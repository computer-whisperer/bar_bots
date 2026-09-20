//! H-ARMY-PRESSURE: the first handful of raiders goes at the opponent as soon as it exists, and keeps going while the
//! fight simulator says the fight is won. This is how experienced players open (K-open-early-pawn-pressure-is-standard):
//! five to twelve Pawns at the opponent's base before 2:30, and against BARb medium that is the game. Raiders that find
//! no army at the opponent's base go for its commander and its lab, and the home group is committed after them
//! (H-ARMY-KILL; `docs/design/2026-09-20-rush-benchmark.md`).
//!
//! It replaces H-ARMY-HARASS, which waited for minute 6, took line units only against extractors we had seen, and
//! judged by the duel table; and before it H-ARMY-RAID, whose fast raiders died to the turret BARb puts beside nearly
//! every extractor. With a commander present the ground is its to raid; it is shown the same targets in its report.

use std::collections::HashSet;

use bot_protocol::{Command, OwnUnit, Tick, UnitId, Vec3};

use super::army::CONTACT_RADIUS;
use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};

/// Raiders that leave together; more join as they come out of the lab. One: the experienced player's first Pawn left
/// for the opponent's base the moment it was built (finished at 83 s, 2500 elmos out at 116 s, an extractor hurt at
/// 146 s); a party of five waited for the fifth.
const PARTY: usize = 1;
/// Raiders idle this close to home while a party is out go and join it.
const JOIN_RADIUS: f32 = 1200.0;
/// What counts as standing at a target, for what the party will meet there.
const TARGET_RADIUS: f32 = 600.0;
/// The party goes on while what it would kill outweighs what it would lose by this much, priced by the chase
/// simulator against what is known to stand at the target (`contact.rs`'s safety on our losses applies).
const GO_GAIN: f32 = 0.0;
const REPRICE_FRAMES: i32 = 4 * FRAMES_PER_SECOND;
/// After a party comes home, no new one for this long.
const REST_FRAMES: i32 = 60 * FRAMES_PER_SECOND;
/// H-ARMY-KILL: the party at the opponent's base with no enemy soldier in sight this close offers the kill.
const KILL_RADIUS: f32 = 1000.0;
/// An extractor this close to the enemy's base is the base's business, not a raid's.
const BASE_RADIUS: f32 = 1400.0;
/// The enemy commander's D-gun is not in the simulator (nobody presses the button there) and it kills a Pawn a shot:
/// a party smaller than this much metal does not fight within reach of the commander (rush-smoke2: four Pawns dead to
/// it in six seconds); a second player's twelve Pawns killed BARb's.
const COMMANDER_PARTY_METAL: f32 = 450.0;
const COMMANDER_REACH: f32 = 700.0;

#[derive(Default)]
pub struct Raid {
    members: Vec<UnitId>,
    target: Option<Vec3>,
    last_order_frame: i32,
    priced_at: i32,
    rest_until: i32,
    /// H-ARMY-KILL: the party stands at the opponent's base and nothing armed of theirs is in sight; the home group is
    /// committed after it (`army.rs` reads this).
    pub(super) kill_offered: Option<Vec3>,
    /// H-ARMY-MARCH: members stopped until the body of the party has come up (Pawns 2000 elmos apart met BARb's
    /// commander one at a time in rush-smoke2).
    held: HashSet<UnitId>,
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

    /// Where the party goes: the nearest extractor of theirs we know of, else their base as we presume it. H-ARMY-KILL:
    /// at their base with nothing armed in sight, the commander if we have seen it, else the lab, else any building.
    fn pressure_target(&self, party_at: Vec3, tick: &Tick) -> Option<Vec3> {
        let base = self.enemy_base(party_at);
        let armed_in_sight = tick.snapshot.enemies.iter().any(|e| {
            e.pos.dist2d(party_at) < KILL_RADIUS && e.def.is_none_or(|d| self.world.def(d).is_some_and(|d| d.weapon_count > 0 && d.speed > 0.0))
        });
        if party_at.dist2d(base) < BASE_RADIUS && !armed_in_sight {
            let commander = self.enemy_commander_seen.filter(|(pos, _)| pos.dist2d(base) < BASE_RADIUS).map(|(pos, _)| pos);
            let lab = self.enemy_buildings.values().filter(|(def, _, _)| self.world.def(*def).is_some_and(|d| !d.build_options.is_empty() && d.speed == 0.0)).map(|(_, pos, _)| *pos);
            let any = self.enemy_buildings.values().map(|(_, pos, _)| *pos).min_by(|a, b| a.dist2d(party_at).total_cmp(&b.dist2d(party_at)));
            return commander.or_else(|| lab.min_by(|a, b| a.dist2d(party_at).total_cmp(&b.dist2d(party_at)))).or(any).or(Some(base));
        }
        self.raid_targets().first().copied().or_else(|| self.reachable_on_foot(base).then_some(base))
    }

    /// `soldiers`: finished soldiers no squad has claimed. Returns with the party's orders pushed.
    pub(super) fn run_raid(&mut self, tick: &Tick, kit: &Kit, soldiers: &[&OwnUnit], commands: &mut Vec<Command>) {
        self.raid.kill_offered = None;
        if !self.enabled("H-ARMY-PRESSURE") || self.strategist.is_some() {
            self.raid.members.clear();
            return;
        }
        self.raid.members.retain(|id| soldiers.iter().any(|u| u.id == *id));
        let members = self.raid.members.clone();
        // Raiders only: the party is as fast as its slowest member, and a Mace among Pawns strings it out.
        let raider = |u: &&OwnUnit| !self.army.is_attacker(u.id) && !members.contains(&u.id) && u.def == kit.raider;

        if self.raid.members.is_empty() {
            self.raid.target = None;
            if tick.frame < self.raid.rest_until {
                return;
            }
            let free: Vec<&OwnUnit> = soldiers.iter().copied().filter(raider).collect();
            if free.len() < PARTY {
                return;
            }
            let party: Vec<&OwnUnit> = free[..PARTY].to_vec();
            let Some(centre) = centre(&party) else { return };
            let Some(target) = self.pressure_target(centre, tick) else { return };
            let verdict = self.assault_verdict(&party, target, TARGET_RADIUS, tick);
            if verdict.gain < GO_GAIN {
                return;
            }
            self.raid.members = party.iter().map(|u| u.id).collect();
            self.raid.target = Some(target);
            self.raid.last_order_frame = 0;
            self.raid.priced_at = tick.frame;
            self.fire("H-ARMY-PRESSURE");
            let names: Vec<String> = party.iter().map(|u| format!("{}#{}", self.name(u.def), u.id.0)).collect();
            eprintln!(
                "[ai {}] f={} pressure: party of {} ({}) leaves for ({:.0}, {:.0}), worth {:.0} against what is known there",
                self.ai(), tick.frame, party.len(), names.join(" "), target.x, target.z, verdict.gain
            );
            self.journal.note(tick.frame, "pressure", serde_json::json!({ "party": party.len(), "target": [target.x, target.z] }), serde_json::json!({ "worth": verdict.gain }));
        } else {
            // Raiders coming out of the lab while a party is out go and join it.
            let party_centre = centre(&soldiers.iter().filter(|u| self.raid.contains(u.id)).copied().collect::<Vec<_>>());
            if let Some(at) = party_centre {
                let joining: Vec<&OwnUnit> = soldiers.iter().copied().filter(raider).filter(|u| u.idle && u.pos.dist2d(self.home) < JOIN_RADIUS).collect();
                self.raid.members.extend(joining.iter().map(|u| u.id));
                commands.extend(joining.iter().map(|u| Command::Fight { unit: u.id, to: at, queue: false }));
            }
        }

        let party: Vec<&OwnUnit> = soldiers.iter().filter(|u| self.raid.contains(u.id)).copied().collect();
        let Some(centre) = centre(&party) else { return };
        // The target is gone when we no longer remember an extractor there (seen destroyed, or found missing), or when
        // the party stands on it and sees nothing.
        let arrived = self.raid.target.is_some_and(|t| centre.dist2d(t) < TARGET_RADIUS);
        let still_there = self.raid.target.is_some_and(|t| self.enemy_buildings.values().any(|(_, pos, _)| pos.dist2d(t) < 100.0) || (!arrived && t.dist2d(self.enemy_base(t)) < BASE_RADIUS));
        let target = if still_there { self.raid.target } else { self.pressure_target(centre, tick) };
        // Priced every few seconds against what is in sight of the party and what is known at the target, and every
        // tick while something armed is in sight: four seconds is a fight's length.
        let armed = |e: &bot_protocol::EnemyUnit| e.def.is_none_or(|d| self.world.def(d).is_some_and(|d| d.weapon_count > 0 && d.speed > 0.0));
        let is_commander = |e: &bot_protocol::EnemyUnit| e.def.is_some_and(|d| self.world.def(d).is_some_and(|d| d.name.ends_with("com") && d.build_speed > 0.0));
        let in_sight: Vec<&bot_protocol::EnemyUnit> = tick.snapshot.enemies.iter().filter(|e| e.pos.dist2d(centre) < CONTACT_RADIUS).collect();
        let armed_in_sight = in_sight.iter().any(|e| armed(e) && !is_commander(e));
        let commander_in_sight = in_sight.iter().any(|e| is_commander(e));
        let commander_at = in_sight.iter().find(|e| is_commander(e)).map(|e| e.pos);
        let commander_near = commander_at.is_some_and(|at| at.dist2d(centre) < COMMANDER_REACH);
        let reprice = in_sight.iter().any(|e| armed(e)) || tick.frame - self.raid.priced_at >= REPRICE_FRAMES;
        let party_metal: f32 = party.iter().map(|u| self.world.def(u.def).map_or(0.0, |d| d.metal_cost)).sum();
        let too_small_for_commander = commander_near && party_metal < COMMANDER_PARTY_METAL;
        let outmatched = if reprice {
                self.raid.priced_at = tick.frame;
                let verdict = self.assault_verdict(&party, target.unwrap_or(centre), CONTACT_RADIUS, tick);
                verdict.gain < GO_GAIN
            } else {
                false
            };
        let outmatched = outmatched && !too_small_for_commander;
        match target {
            Some(target) if too_small_for_commander && party.len() >= PARTY => {
                // Too few for the commander: wait for the rest out of its reach, as a player gathers at the edge of a
                // base, rather than walk home and lose the ground already covered.
                self.raid.target = Some(target);
                let Some(commander) = commander_at else { return };
                let (dx, dz) = (centre.x - commander.x, centre.z - commander.z);
                let len = dx.hypot(dz).max(1.0);
                let wait = Vec3 { x: commander.x + dx / len * (COMMANDER_REACH + 300.0), y: 0.0, z: commander.z + dz / len * (COMMANDER_REACH + 300.0) };
                if tick.frame - self.raid.last_order_frame >= REPRICE_FRAMES {
                    self.raid.last_order_frame = tick.frame;
                    commands.extend(party.iter().map(|u| Command::Move { unit: u.id, to: wait, queue: false }));
                }
            }
            Some(target) if !outmatched && party.len() >= PARTY => {
                if self.raid.target != Some(target) {
                    self.raid.last_order_frame = 0;
                }
                self.raid.target = Some(target);
                // H-ARMY-KILL: at their base with no soldier of theirs in sight, and the commander either out of sight
                // or outnumbered, the home group comes to finish it.
                let kill_open = !armed_in_sight && (!commander_in_sight || party_metal >= COMMANDER_PARTY_METAL);
                if centre.dist2d(self.enemy_base(centre)) < BASE_RADIUS && kill_open && self.enabled("H-ARMY-KILL") {
                    self.raid.kill_offered = Some(target);
                }
                let mut held = std::mem::take(&mut self.raid.held);
                if tick.frame - self.raid.last_order_frame >= REPRICE_FRAMES {
                    self.raid.last_order_frame = tick.frame;
                    commands.extend(party.iter().filter(|u| !held.contains(&u.id)).map(|u| Command::Fight { unit: u.id, to: target, queue: false }));
                }
                commands.extend(self.march(&mut held, &party, target, tick.snapshot.enemies.as_slice()));
                self.raid.held = held;
            }
            _ => {
                eprintln!(
                    "[ai {}] f={} pressure: party of {} comes home ({})",
                    self.ai(), tick.frame, party.len(), if outmatched { "outmatched" } else if party.len() < PARTY { "too few left" } else { "no target" }
                );
                let station = self.last_station;
                commands.extend(party.iter().map(|u| Command::Move { unit: u.id, to: station, queue: false }));
                self.raid.members.clear();
                self.raid.held.clear();
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
