//! Where the opponents live: one base per enemy seat (H-MAP-ENEMY-BASES).
//!
//! A base starts as a guess (its ally team's start box, else the mirror image of our start) and moves to the seat's
//! factories once one is seen. Every rule that used to ask for "the enemy start" asks for the live base nearest the
//! place in question; with two opponents a mean over both would be a point between them where nobody lives.

use std::collections::HashMap;

use bot_protocol::{Event, Tick, UnitId, Vec3};

/// A factory with no team on it belongs to the base this near, else to the nearest.
const BASE_RADIUS: f32 = 1500.0;
/// A found base with no factory left and nothing remembered standing this near is dead.
const RAZED_RADIUS: f32 = 1200.0;

pub struct EnemyBase {
    /// The seat, when the script named the teams; `None` for a lone mirror guess.
    pub team: Option<i32>,
    pub at: Vec3,
    /// A factory of this seat has been seen (it may be gone since).
    pub found: bool,
    /// Found, and then razed to the ground: no longer a target nor a claim on ground.
    pub dead: bool,
    factories: HashMap<UnitId, Vec3>,
}

impl super::Brain {
    /// First guesses, once our own start is known.
    pub(super) fn guess_enemy_bases(&mut self) {
        let hello = &self.world.hello;
        let mut bases: Vec<EnemyBase> = Vec::new();
        let enemy_teams: Vec<(i32, i32)> = hello.teams.iter().filter(|t| t.ally_team != hello.ally_team).map(|t| (t.team, t.ally_team)).collect();
        for (index, (team, ally_team)) in enemy_teams.iter().enumerate() {
            let sharing: Vec<i32> = enemy_teams.iter().filter(|(_, a)| a == ally_team).map(|(t, _)| *t).collect();
            let at = match hello.start_boxes.iter().find(|b| b.ally_team == *ally_team) {
                // Seats sharing a box are spread along its longer side.
                Some(b) => {
                    let share = (sharing.iter().position(|t| t == team).unwrap_or(0) as f32 + 0.5) / sharing.len() as f32;
                    if b.right - b.left >= b.bottom - b.top {
                        Vec3 { x: b.left + (b.right - b.left) * share, ..b.centre() }
                    } else {
                        Vec3 { z: b.top + (b.bottom - b.top) * share, ..b.centre() }
                    }
                }
                None if index == 0 => self.world.mirrored(self.home),
                None => continue,
            };
            bases.push(EnemyBase { team: Some(*team), at, found: false, dead: false, factories: HashMap::new() });
        }
        if bases.is_empty() {
            bases.push(EnemyBase { team: None, at: self.world.mirrored(self.home), found: false, dead: false, factories: HashMap::new() });
        }
        self.enemy_bases = bases;
    }

    /// H-MAP-ENEMY-START: a start is always beside metal, and a box centre or a mirror point may be a beach or a cliff
    /// top; each guess moves to the metal spot nearest it that we can walk to.
    pub(super) fn snap_guesses_to_metal(&mut self, reachable: impl Fn(Vec3) -> bool) {
        let spots = &self.world.hello.metal_spots;
        for base in self.enemy_bases.iter_mut().filter(|b| !b.found) {
            if let Some(spot) = spots.iter().filter(|s| reachable(**s)).min_by(|a, b| a.dist2d(base.at).total_cmp(&b.dist2d(base.at))) {
                base.at = Vec3 { y: 0.0, ..*spot };
            }
        }
    }

    /// Moves each base to its seat's factories in sight or remembered, and calls a razed base dead.
    pub(super) fn track_enemy_bases(&mut self, tick: &Tick) {
        if !self.enabled("H-MAP-ENEMY-BASE") {
            return;
        }
        for event in &tick.events {
            if let Event::EnemyDestroyed { enemy } = event {
                self.enemy_bases.iter_mut().for_each(|b| { b.factories.remove(enemy); });
            }
        }
        for enemy in &tick.snapshot.enemies {
            let is_factory = enemy.def.and_then(|d| self.world.def(d)).is_some_and(|d| d.speed == 0.0 && !d.build_options.is_empty());
            if !is_factory || self.enemy_bases.iter().any(|b| b.factories.contains_key(&enemy.id)) {
                continue;
            }
            let by_team = enemy.team.and_then(|team| self.enemy_bases.iter().position(|b| b.team == Some(team)));
            let nearest = || {
                (0..self.enemy_bases.len()).min_by(|a, b| {
                    let key = |i: &usize| (self.enemy_bases[*i].dead, self.enemy_bases[*i].at.dist2d(enemy.pos));
                    key(a).0.cmp(&key(b).0).then(key(a).1.total_cmp(&key(b).1))
                })
            };
            let Some(index) = by_team.or_else(nearest) else { continue };
            // A teamless factory far from every base is a base of its own (a seat the script did not tell us of).
            if by_team.is_none() && self.enemy_bases[index].found && self.enemy_bases[index].at.dist2d(enemy.pos) > BASE_RADIUS {
                self.enemy_bases.push(EnemyBase { team: enemy.team, at: enemy.pos, found: true, dead: false, factories: HashMap::from([(enemy.id, enemy.pos)]) });
                continue;
            }
            self.enemy_bases[index].factories.insert(enemy.id, enemy.pos);
        }
        // Factories we stood beside and could not see are gone (`forget_razed_buildings`).
        let remembered = &self.enemy_buildings;
        let mut moved = false;
        for base in &mut self.enemy_bases {
            base.factories.retain(|id, _| remembered.contains_key(id));
            if !base.factories.is_empty() {
                let n = base.factories.len() as f32;
                let at = base.factories.values().fold(Vec3::default(), |sum, p| Vec3 { x: sum.x + p.x / n, y: 0.0, z: sum.z + p.z / n });
                moved |= at.dist2d(base.at) > 1.0;
                (base.at, base.found, base.dead) = (at, true, false);
            } else if base.found {
                let standing = remembered.values().any(|(_, pos, _)| pos.dist2d(base.at) < RAZED_RADIUS);
                if base.dead == standing {
                    eprintln!("[ai {}] f={} enemy base at ({:.0}, {:.0}) is {}", self.world.hello.ai_id, tick.frame, base.at.x, base.at.z, if standing { "alive after all" } else { "razed" });
                    moved = true;
                }
                base.dead = !standing;
            }
        }
        if moved {
            self.resurvey_enemy();
        }
    }

    /// Bases that still count; all of them when every one is dead (something must be "the enemy's side").
    pub(super) fn live_enemy_bases(&self) -> Vec<Vec3> {
        let live: Vec<Vec3> = self.enemy_bases.iter().filter(|b| !b.dead).map(|b| b.at).collect();
        if live.is_empty() { self.enemy_bases.iter().map(|b| b.at).collect() } else { live }
    }

    /// The live enemy base nearest `from`, as the crow flies.
    pub(super) fn enemy_base(&self, from: Vec3) -> Vec3 {
        self.live_enemy_bases().into_iter().min_by(|a, b| a.dist2d(from).total_cmp(&b.dist2d(from))).unwrap_or_else(|| self.world.mirrored(self.home))
    }

    /// The found, live base nearest home, if any base has been found.
    pub(super) fn found_enemy_base(&self) -> Option<Vec3> {
        self.enemy_bases.iter().filter(|b| b.found && !b.dead).map(|b| b.at).min_by(|a, b| a.dist2d(self.home).total_cmp(&b.dist2d(self.home)))
    }
}
