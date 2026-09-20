//! H-ARMY-HARASS: a small party that burns the opponent's outlying extractors while the main army waits.
//!
//! The first raiding rule (H-ARMY-RAID, retired) sent fast raiders at anything; they stuck on ledges and died to the
//! turret BARb puts beside nearly every extractor. This one picks its fights: line units, which beat a light turret at
//! equal metal (K-units-duel-range-vs-turrets), against an extractor we have seen, only at good odds against what we
//! know stands there, and home again when the odds turn. With a commander present the ground is its to raid; it is
//! shown the same targets in its report.

use bot_protocol::{Command, OwnUnit, Tick, UnitId, Vec3};

use super::army::CONTACT_RADIUS;
use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};

const FIRST_RAID_FRAME: i32 = 6 * 60 * FRAMES_PER_SECOND;
const PARTY: usize = 5;
/// Soldiers that stay at home when a party leaves.
const RESERVE: usize = 6;
/// An extractor this close to the enemy's base is the main army's business.
const BASE_RADIUS: f32 = 1400.0;
/// What counts as standing at a target, for the odds.
const TARGET_RADIUS: f32 = 500.0;
const GO_ODDS: f32 = 1.5;
const FLEE_ODDS: f32 = 0.7;
const REORDER_FRAMES: i32 = 4 * FRAMES_PER_SECOND;
/// After a party is lost or flees, no new one for this long.
const REST_FRAMES: i32 = 90 * FRAMES_PER_SECOND;

#[derive(Default)]
pub struct Raid {
    members: Vec<UnitId>,
    target: Option<Vec3>,
    last_order_frame: i32,
    rest_until: i32,
}

impl Raid {
    pub fn contains(&self, unit: UnitId) -> bool {
        self.members.contains(&unit)
    }
}

impl Brain {
    /// Enemy extractors seen and not seen dead, outside its base, that we can walk to: nearest to us first.
    pub(super) fn raid_targets(&self) -> Vec<Vec3> {
        let mut targets: Vec<Vec3> = self
            .enemy_buildings
            .values()
            .filter(|(def, _, _)| self.world.def(*def).is_some_and(|d| d.extracts_metal > 0.0))
            .map(|(_, pos, _)| *pos)
            .filter(|pos| pos.dist2d(self.enemy_base(*pos)) > BASE_RADIUS && self.reachable_on_foot(*pos))
            .collect();
        targets.sort_by(|a, b| self.walk_from_home(*a).total_cmp(&self.walk_from_home(*b)));
        targets
    }

    /// `soldiers`: finished soldiers no squad has claimed. Returns with the party's orders pushed.
    pub(super) fn run_raid(&mut self, tick: &Tick, kit: &Kit, soldiers: &[&OwnUnit], commands: &mut Vec<Command>) {
        if !self.enabled("H-ARMY-HARASS") || self.strategist.is_some() {
            self.raid.members.clear();
            return;
        }
        self.raid.members.retain(|id| soldiers.iter().any(|u| u.id == *id));
        let enemies = tick.snapshot.enemies.as_slice();
        let targets = self.raid_targets();

        if self.raid.members.is_empty() {
            self.raid.target = None;
            let free: Vec<&&OwnUnit> = soldiers.iter().filter(|u| !self.army.is_attacker(u.id) && u.def == kit.line).collect();
            let at_home = soldiers.iter().filter(|u| !self.army.is_attacker(u.id)).count();
            if tick.frame < FIRST_RAID_FRAME.max(self.raid.rest_until) || free.len() < PARTY || at_home < PARTY + RESERVE {
                return;
            }
            let party: Vec<&OwnUnit> = free.iter().take(PARTY).map(|u| **u).collect();
            let ours = Self::force_of(&party);
            let Some(target) = targets.iter().copied().find(|t| self.odds(&ours, &self.known_enemy_force(*t, TARGET_RADIUS, enemies)) >= GO_ODDS) else {
                return;
            };
            self.raid.members = party.iter().map(|u| u.id).collect();
            self.raid.target = Some(target);
            self.raid.last_order_frame = 0;
            self.fire("H-ARMY-HARASS");
            eprintln!("[ai {}] f={} raid party of {} leaves for the extractor at ({:.0}, {:.0})", self.ai(), tick.frame, party.len(), target.x, target.z);
        }

        let party: Vec<&OwnUnit> = soldiers.iter().filter(|u| self.raid.contains(u.id)).copied().collect();
        let Some(centre) = centre(&party) else { return };
        let near = self.known_enemy_force(centre, CONTACT_RADIUS, enemies);
        let outmatched = self.odds(&Self::force_of(&party), &near) < FLEE_ODDS;
        // The target is gone when we no longer remember an extractor there (seen destroyed, or found missing).
        let target = self.raid.target.filter(|t| targets.iter().any(|seen| seen.dist2d(*t) < 50.0)).or_else(|| {
            let ours = Self::force_of(&party);
            targets.iter().copied().find(|t| self.odds(&ours, &self.known_enemy_force(*t, TARGET_RADIUS, enemies)) >= GO_ODDS)
        });
        match target {
            Some(target) if !outmatched && party.len() >= 2 => {
                self.raid.target = Some(target);
                if tick.frame - self.raid.last_order_frame >= REORDER_FRAMES {
                    self.raid.last_order_frame = tick.frame;
                    commands.extend(party.iter().map(|u| Command::Fight { unit: u.id, to: target, queue: false }));
                }
            }
            _ => {
                // Nothing left to burn, or the odds have turned: home, and back into the home group.
                eprintln!(
                    "[ai {}] f={} raid party of {} comes home ({})",
                    self.ai(), tick.frame, party.len(), if outmatched { "outmatched" } else if party.len() < 2 { "too few left" } else { "no target" }
                );
                let station = self.home;
                commands.extend(party.iter().map(|u| Command::Move { unit: u.id, to: station, queue: false }));
                self.raid.members.clear();
                self.raid.target = None;
                self.raid.rest_until = tick.frame + REST_FRAMES;
            }
        }
    }
}

fn centre(units: &[&OwnUnit]) -> Option<Vec3> {
    if units.is_empty() {
        return None;
    }
    let n = units.len() as f32;
    Some(units.iter().fold(Vec3::default(), |sum, u| Vec3 { x: sum.x + u.pos.x / n, y: 0.0, z: sum.z + u.pos.z / n }))
}
