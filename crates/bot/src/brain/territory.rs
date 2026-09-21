//! H-MAP-TERRITORY: whose ground is whose. Ground is ours when we can answer for it sooner and harder than the
//! opponent can reach it. One coarse grid holds, per cell, the force each side can bring there within
//! [`REACH_SECONDS`]; everything that used to ask a local question with a clock (a raided spot is closed four minutes;
//! no farther from home than N; stand by the extractor nearest the enemy) asks this instead.
//!
//! `ours`: a standing claim that falls off with walking distance from home, our and our allies' soldiers (each
//! discounted by its travel time to the cell) and turrets (in range only). `theirs`: the same claim around every live
//! enemy base, enemy soldiers in sight, soldiers seen lately (fading over [`MEMORY_FRAMES`], and spread as far as they
//! may have walked since), remembered armed buildings, and the places where something of ours died lately.
//!
//! Two memories, because two questions are asked. Whether a constructor may go somewhere is about who is there NOW:
//! an extractor pays for itself in half a minute, and with every soldier seen in three minutes counted, one visit by the
//! opponent's army closed most of our half and nothing was rebuilt (terr-1-nw: 4-5 extractors from minute 7 against
//! 6-7 without the grid). The ground's class forgets in [`PRESENCE_FRAMES`]. Where soldiers and turrets should stand is
//! about where raids keep coming from: `threat` and `raided` remember for [`MEMORY_FRAMES`].
//! Straight-line travel for units; the standing claims use walking distance, which is what cliffs and water change most.

use std::collections::HashMap;

use bot_protocol::{Event, Tick, UnitId, Vec3};

use super::combat::TURRET_WORTH;
use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};

pub const CELL: f32 = 256.0;
const UPDATE_FRAMES: i32 = 2 * FRAMES_PER_SECOND;
/// A raid kills an extractor in 15-20 s: force that arrives later than this does not hold the ground.
const REACH_SECONDS: f32 = 20.0;
const MEMORY_FRAMES: i32 = 3 * 60 * FRAMES_PER_SECOND;
const PRESENCE_FRAMES: i32 = 60 * FRAMES_PER_SECOND;
/// An unseen soldier is taken to be within this of where it was seen, at most.
const MAX_SPREAD: f32 = 1200.0;
/// The standing claim at a start point, in metal of soldiers: about a first raiding party.
const CLAIM: f32 = 600.0;
/// One side holds a cell when it brings this many times the other's force.
const HOLD_RATIO: f32 = 1.5;
const TURRET_RANGE: f32 = 450.0;
/// A death of ours with no killer in sight still marks the ground: at least this much, where it happened.
const LOSS_MARK: f32 = 200.0;
const SLOWEST: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    Held,
    Contested,
    Theirs,
}

impl Ground {
    pub fn word(self) -> &'static str {
        match self {
            Ground::Held => "held",
            Ground::Contested => "contested",
            Ground::Theirs => "theirs",
        }
    }
}

/// A mobile force somewhere: where and when last seen, its metal, its speed in elmos a second.
struct Sighting {
    at: Vec3,
    frame: i32,
    metal: f32,
    speed: f32,
}

#[derive(Default)]
pub struct Territory {
    width: usize,
    height: usize,
    ours: Vec<f32>,
    /// What the opponent can bring, from the long memory (posts, turrets) and from the short one (the ground's class).
    theirs: Vec<f32>,
    theirs_now: Vec<f32>,
    claim_ours: Vec<f32>,
    claim_theirs: Vec<f32>,
    /// The enemy bases the standing claims were computed for.
    claimed_for: Vec<Vec3>,
    updated: i32,
    enemy_soldiers: HashMap<UnitId, Sighting>,
    /// Where something of ours died: a force nobody may have seen.
    losses: Vec<Sighting>,
}

impl Territory {
    fn index(&self, pos: Vec3) -> Option<usize> {
        if self.width == 0 {
            return None;
        }
        let (x, z) = (((pos.x / CELL) as usize).min(self.width - 1), ((pos.z / CELL) as usize).min(self.height - 1));
        Some(z * self.width + x)
    }

    fn centre(&self, index: usize) -> Vec3 {
        Vec3 { x: ((index % self.width) as f32 + 0.5) * CELL, y: 0.0, z: ((index / self.width) as f32 + 0.5) * CELL }
    }

    pub fn ground(&self, pos: Vec3) -> Ground {
        let Some(index) = self.index(pos) else { return Ground::Held };
        let (ours, theirs) = (self.ours[index], self.theirs_now[index]);
        if ours >= HOLD_RATIO * theirs {
            Ground::Held
        } else if theirs >= HOLD_RATIO * ours {
            Ground::Theirs
        } else {
            Ground::Contested
        }
    }

    /// The force the opponent can bring to `pos`, standing claim included: how exposed a thing standing there is.
    pub fn threat(&self, pos: Vec3) -> f32 {
        self.index(pos).map_or(0.0, |index| self.theirs[index])
    }

    /// The part of the threat that comes from what was seen or lost lately, not from where the bases are.
    pub fn raided(&self, pos: Vec3) -> f32 {
        self.index(pos).map_or(0.0, |index| self.theirs[index] - self.claim_theirs[index])
    }

    /// One character per cell, rows north to south: `+` held, `?` contested, `-` theirs.
    pub fn sketch(&self) -> Vec<String> {
        (0..self.height)
            .map(|z| {
                (0..self.width)
                    .map(|x| match self.ground(self.centre(z * self.width + x)) {
                        Ground::Held => '+',
                        Ground::Contested => '?',
                        Ground::Theirs => '-',
                    })
                    .collect()
            })
            .collect()
    }
}

/// What a force of `metal` at `at`, last seen `age` seconds ago and moving at `speed`, brings to `cell`, for a memory
/// that forgets in `memory` frames.
fn brought(cell: Vec3, at: Vec3, metal: f32, speed: f32, age: f32, memory: i32) -> f32 {
    let speed = speed.max(SLOWEST);
    let spread = (speed * age).min(MAX_SPREAD);
    let travel = (cell.dist2d(at) - spread).max(0.0) / speed;
    let fresh = 1.0 - age * FRAMES_PER_SECOND as f32 / memory as f32;
    metal * (1.0 - travel / REACH_SECONDS).max(0.0) * fresh.max(0.0)
}

impl Brain {
    pub(super) fn ground(&self, pos: Vec3) -> Ground {
        self.territory.ground(pos)
    }

    pub(super) fn update_territory(&mut self, tick: &Tick, kit: &Kit) {
        let is_soldier = |d: &bot_protocol::UnitDefInfo| d.weapon_count > 0 && d.speed > 0.0 && d.build_speed == 0.0;
        // Sightings are kept every tick; the grid is redrawn every two seconds.
        for event in &tick.events {
            match *event {
                Event::EnemyDestroyed { enemy } => {
                    self.territory.enemy_soldiers.remove(&enemy);
                }
                Event::UnitDestroyed { unit, .. } => {
                    if let Some((def, pos)) = self.known_units.get(&unit) {
                        let metal = self.world.def(*def).map_or(0.0, |d| d.metal_cost).max(LOSS_MARK);
                        self.territory.losses.push(Sighting { at: *pos, frame: tick.frame, metal, speed: 0.0 });
                    }
                }
                _ => {}
            }
        }
        for enemy in &tick.snapshot.enemies {
            if let Some(def) = enemy.def.and_then(|d| self.world.def(d)).filter(|d| is_soldier(d)) {
                self.territory.enemy_soldiers.insert(enemy.id, Sighting { at: enemy.pos, frame: tick.frame, metal: def.metal_cost, speed: def.speed });
            }
        }
        if tick.frame - self.territory.updated < UPDATE_FRAMES && self.territory.width > 0 {
            return;
        }
        self.territory.updated = tick.frame;
        if tick.due() % (60 * FRAMES_PER_SECOND) < UPDATE_FRAMES && self.territory.width > 0 {
            let spots = &self.world.hello.metal_spots;
            let count = |ground: Ground| spots.iter().filter(|s| self.ground(**s) == ground).count();
            eprintln!(
                "[ai {}] f={} ground: metal spots held {}, contested {}, theirs {}; sightings remembered {}, losses {}",
                self.ai(), tick.frame, count(Ground::Held), count(Ground::Contested), count(Ground::Theirs),
                self.territory.enemy_soldiers.len(), self.territory.losses.len()
            );
        }
        self.territory.enemy_soldiers.retain(|_, s| tick.frame - s.frame < MEMORY_FRAMES);
        self.territory.losses.retain(|s| tick.frame - s.frame < MEMORY_FRAMES);
        self.redraw_claims();

        let own: Vec<(Vec3, f32, f32)> = tick
            .snapshot
            .own_units
            .iter()
            .filter(|u| !u.being_built && self.is_army(u, kit))
            .filter_map(|u| self.world.def(u.def).map(|d| (u.pos, d.metal_cost, d.speed)))
            .chain(self.allies.iter().filter(|a| !a.being_built).filter_map(|a| self.world.def(a.def).filter(|d| is_soldier(d)).map(|d| (a.pos, d.metal_cost, d.speed))))
            .collect();
        let is_turret = |d: &bot_protocol::UnitDefInfo| d.weapon_count > 0 && d.speed == 0.0;
        let own_turrets: Vec<(Vec3, f32)> = tick
            .snapshot
            .own_units
            .iter()
            .filter(|u| !u.being_built)
            .map(|u| (u.pos, u.def))
            .chain(self.allies.iter().filter(|a| !a.being_built).map(|a| (a.pos, a.def)))
            .filter_map(|(pos, def)| self.world.def(def).filter(|d| is_turret(d)).map(|d| (pos, d.metal_cost * TURRET_WORTH)))
            .collect();
        let their_turrets: Vec<(Vec3, f32)> = self
            .enemy_buildings
            .values()
            .filter_map(|(def, pos, _)| self.world.def(*def).filter(|d| is_turret(d)).map(|d| (*pos, d.metal_cost * TURRET_WORTH)))
            .collect();
        let age = |s: &Sighting| (tick.frame - s.frame) as f32 / FRAMES_PER_SECOND as f32;
        let t = &mut self.territory;
        for index in 0..t.ours.len() {
            let cell = t.centre(index);
            let in_range = |turrets: &[(Vec3, f32)]| turrets.iter().filter(|(pos, _)| pos.dist2d(cell) < TURRET_RANGE).map(|(_, worth)| worth).sum::<f32>();
            t.ours[index] = t.claim_ours[index] + own.iter().map(|(at, metal, speed)| brought(cell, *at, *metal, *speed, 0.0, MEMORY_FRAMES)).sum::<f32>() + in_range(&own_turrets);
            let seen = |memory: i32| t.enemy_soldiers.values().chain(&t.losses).map(|s| brought(cell, s.at, s.metal, s.speed, age(s), memory)).sum::<f32>();
            let standing = t.claim_theirs[index] + in_range(&their_turrets);
            t.theirs[index] = standing + seen(MEMORY_FRAMES);
            t.theirs_now[index] = standing + seen(PRESENCE_FRAMES);
        }
    }

    /// The standing claims: ours falls off with walking distance from home (and from each ally's start), theirs from the
    /// nearest live enemy base, both to nothing at the distance between home and that base. Redone when a base moves.
    fn redraw_claims(&mut self) {
        let bases = self.live_enemy_bases();
        let same = self.territory.width > 0
            && bases.len() == self.territory.claimed_for.len()
            && bases.iter().zip(&self.territory.claimed_for).all(|(a, b)| a.dist2d(*b) < CELL);
        if same {
            return;
        }
        let map = &self.world.hello.map;
        let (width, height) = ((map.width / CELL).ceil().max(1.0) as usize, (map.height / CELL).ceil().max(1.0) as usize);
        let between = self.walk_from_home(self.enemy_base(self.home)).max(CELL);
        let mut t = Territory { width, height, claimed_for: bases, ..std::mem::take(&mut self.territory) };
        let claim = |walk: Option<f32>| walk.map_or(0.0, |walk| CLAIM * (1.0 - walk / between).max(0.0));
        let cells = width * height;
        t.claim_ours = (0..cells)
            .map(|index| {
                let cell = t.centre(index);
                let from_home = self.reachable_on_foot(cell).then(|| self.walk_from_home(cell));
                let from_ally = self.ally_starts.values().map(|start| start.dist2d(cell)).min_by(f32::total_cmp);
                claim(from_home).max(claim(from_ally))
            })
            .collect();
        t.claim_theirs = (0..cells).map(|index| claim(self.walk_from_enemy(t.centre(index)))).collect();
        t.ours = t.claim_ours.clone();
        t.theirs = t.claim_theirs.clone();
        t.theirs_now = t.claim_theirs.clone();
        self.territory = t;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_fades_with_travel_time_and_with_age_and_spreads_while_unseen() {
        let at = Vec3::default();
        let cell = |x: f32| Vec3 { x, y: 0.0, z: 0.0 };
        assert_eq!(brought(cell(0.0), at, 100.0, 50.0, 0.0, MEMORY_FRAMES), 100.0);
        assert_eq!(brought(cell(500.0), at, 100.0, 50.0, 0.0, MEMORY_FRAMES), 50.0);
        assert_eq!(brought(cell(1000.0), at, 100.0, 50.0, 0.0, MEMORY_FRAMES), 0.0);
        // Seen 10 s ago, it may have walked 500 since: as good as there, less what 10 s of memory fades.
        assert!((brought(cell(500.0), at, 100.0, 50.0, 10.0, MEMORY_FRAMES) - 100.0 * (1.0 - 10.0 / 180.0)).abs() < 0.01);
        assert_eq!(brought(cell(0.0), at, 100.0, 50.0, 180.0, MEMORY_FRAMES), 0.0);
    }
}
