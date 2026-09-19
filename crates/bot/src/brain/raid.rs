//! Raid squads: a few fast units that hunt enemy extractors away from the enemy's defences.
//!
//! The main army marches as one group at one target, so its raiders' speed is wasted, and the enemy's outlying
//! extractors live all game (K-army-we-never-raid). Squads form from raiders standing in the home group and are
//! then no part of it: they are not counted for waves, not recalled, and not stationed.

use std::collections::HashMap;

use bot_protocol::{Command, OwnUnit, Tick, UnitId, Vec3};

use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};

const SQUAD_SIZE: usize = 3;
const MAX_SQUADS: usize = 3;
/// No raids on anything this close to an armed enemy building we remember.
const TURRET_DANGER: f32 = 550.0;
/// A squad member this close to its target has arrived: attack what is there, or find the spot empty.
const ARRIVED: f32 = 300.0;
/// A spot found empty, or given up on, is left alone for this long.
const REVISIT_FRAMES: i32 = 3 * 60 * FRAMES_PER_SECOND;
/// A squad that has not arrived after this long gives the target up (unreachable, or too well guarded).
const PATIENCE_FRAMES: i32 = 2 * 60 * FRAMES_PER_SECOND;

#[derive(Default)]
pub struct Raids {
    squads: Vec<Squad>,
    /// Places not worth going to until the given frame.
    stale: Vec<(Vec3, i32)>,
}

struct Squad {
    members: Vec<UnitId>,
    target: Option<(Vec3, i32)>,
    /// What each member was last told (where, and whether to fight there), so orders are not repeated every tick.
    ordered: HashMap<UnitId, (Vec3, bool)>,
}

impl Raids {
    pub fn is_raider(&self, unit: UnitId) -> bool {
        self.squads.iter().any(|s| s.members.contains(&unit))
    }
}

impl Brain {
    /// `pool` is the home group: finished soldiers that are neither attackers nor already raiding.
    pub(super) fn run_raids(&mut self, tick: &Tick, kit: &Kit, pool: &[&OwnUnit], commands: &mut Vec<Command>) {
        if !self.enabled("H-ARMY-RAID") {
            return;
        }
        let own = &tick.snapshot.own_units;
        for squad in &mut self.raids.squads {
            squad.members.retain(|id| own.iter().any(|u| u.id == *id));
        }
        self.raids.squads.retain(|s| !s.members.is_empty());
        self.raids.stale.retain(|(_, until)| *until > tick.frame);

        let recruits: Vec<UnitId> = pool.iter().filter(|u| u.def == kit.raider).map(|u| u.id).collect();
        if recruits.len() >= SQUAD_SIZE && self.raids.squads.len() < MAX_SQUADS {
            let members = recruits[..SQUAD_SIZE].to_vec();
            eprintln!("[ai {}] f={} raid squad {} formed", self.ai(), tick.frame, self.raids.squads.len() + 1);
            self.raids.squads.push(Squad { members, target: None, ordered: HashMap::new() });
        }

        for index in 0..self.raids.squads.len() {
            let positions: Vec<Vec3> = {
                let members = &self.raids.squads[index].members;
                own.iter().filter(|u| members.contains(&u.id)).map(|u| u.pos).collect()
            };
            // Arrived and nothing to shoot, or out of patience: the place goes stale and the squad moves on.
            if let Some((target, since)) = self.raids.squads[index].target {
                let arrived = positions.iter().any(|p| p.dist2d(target) < ARRIVED);
                let prey_here = self.enemy_buildings.values().any(|(_, pos, _)| pos.dist2d(target) < ARRIVED);
                if (arrived && !prey_here) || tick.frame - since > PATIENCE_FRAMES {
                    self.raids.stale.push((target, tick.frame + REVISIT_FRAMES));
                    self.raids.squads[index].target = None;
                }
            }
            if self.raids.squads[index].target.is_none() {
                let taken: Vec<Vec3> = self.raids.squads.iter().filter_map(|s| s.target.map(|(t, _)| t)).collect();
                let from = positions.first().copied().unwrap_or(self.home);
                if let Some(target) = self.raid_target(from, &taken) {
                    self.fire("H-ARMY-RAID");
                    self.raids.squads[index].target = Some((target, tick.frame));
                }
            }
            let squad = &mut self.raids.squads[index];
            let Some((target, _)) = squad.target else { continue };
            for unit in own.iter().filter(|u| squad.members.contains(&u.id)) {
                // Walk past whatever is on the way; fight only once there.
                let near = unit.pos.dist2d(target) < 2.0 * ARRIVED;
                let told = squad.ordered.get(&unit.id).is_some_and(|(t, n)| t.dist2d(target) < 1.0 && *n == near);
                if !told || unit.idle {
                    squad.ordered.insert(unit.id, (target, near));
                    commands.push(if near {
                        Command::Fight { unit: unit.id, to: target, queue: false }
                    } else {
                        Command::Move { unit: unit.id, to: target, queue: false }
                    });
                }
            }
        }
    }

    /// The nearest place worth raiding: a remembered enemy extractor away from remembered turrets, else a metal
    /// spot on the enemy's half that we have not looked at lately.
    fn raid_target(&self, from: Vec3, taken: &[Vec3]) -> Option<Vec3> {
        let armed: Vec<Vec3> = self
            .enemy_buildings
            .values()
            .filter(|(def, _, _)| self.world.def(*def).is_some_and(|d| d.weapon_count > 0))
            .map(|(_, pos, _)| *pos)
            .collect();
        let safe = |p: &Vec3| !armed.iter().any(|a| a.dist2d(*p) < TURRET_DANGER);
        let fresh = |p: &Vec3| !self.raids.stale.iter().any(|(s, _)| s.dist2d(*p) < ARRIVED);
        let free = |p: &Vec3| !taken.iter().any(|t| t.dist2d(*p) < ARRIVED);
        let nearest = |a: &Vec3, b: &Vec3| a.dist2d(from).total_cmp(&b.dist2d(from));
        let extractors = self
            .enemy_buildings
            .values()
            .filter(|(def, _, _)| self.world.def(*def).is_some_and(|d| d.extracts_metal > 0.0))
            .map(|(_, pos, _)| *pos);
        if let Some(known) = extractors.filter(safe).filter(fresh).filter(free).min_by(nearest) {
            return Some(known);
        }
        let theirs = |p: &Vec3| p.dist2d(self.enemy_start) < p.dist2d(self.home);
        let spots = self.world.hello.metal_spots.iter().map(|s| Vec3 { y: 0.0, ..*s });
        spots.filter(theirs).filter(safe).filter(fresh).filter(free).min_by(nearest)
    }
}
