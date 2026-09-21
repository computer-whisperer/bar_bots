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
use super::scout::{LIKELY_BASE, LIKELY_BOX, TURRET_BERTH};
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
/// After a party is lost, no new one for this long.
const REST_FRAMES: i32 = 60 * FRAMES_PER_SECOND;
/// Outmatched at its target, the party tries this many other extractors of theirs, nearest first, before waiting.
const ELSEWHERE_TRIES: usize = 3;
/// How far from the nearest threat the party waits for reinforcements when nothing is worth attacking.
const WAIT_OFF: f32 = 1000.0;
/// With nothing of theirs known worth going for, the party feels round the base's perimeter for unguarded
/// structures: points on a ring this far from the presumed base, each remembered as probed for this long.
const PERIMETER: f32 = 900.0;
const PERIMETER_POINTS: usize = 8;
const PROBE_MEMORY: i32 = 120 * FRAMES_PER_SECOND;
/// An alternative target (a structure, a spot, a ring point) holds this long unless reached: the party's mind was
/// changing every few seconds as the "still there" test failed for a spot or a ring point (rush-24).
const ALTERNATIVE_FRAMES: i32 = 30 * FRAMES_PER_SECOND;
/// "Round the base" for the party's alternatives: the base cluster, not the next one over (1800 kept a cluster
/// 1580 away in; the user counts that as another cluster, the scouts' business).
const PARTY_VICINITY: f32 = 1200.0;
/// H-ARMY-KILL: the party at the opponent's base with no enemy soldier in sight this close offers the kill.
const KILL_RADIUS: f32 = 1000.0;
/// An extractor this close to the enemy's base is the base's business, not a raid's.
const BASE_RADIUS: f32 = 1400.0;
/// The enemy commander's D-gun is not in the simulator (nobody presses the button there) and it kills a Pawn a shot:
/// a party smaller than this much metal does not fight within reach of the commander (rush-smoke2: four Pawns dead to
/// it in six seconds); a second player's twelve Pawns killed BARb's.
const COMMANDER_PARTY_METAL: f32 = 450.0;
const COMMANDER_REACH: f32 = 700.0;
/// The party's body: members within this of the one nearest the target. Raiders still on their way from home are
/// members too, but the march holds the leaders for the body only, and the body is what stands at the target, sees
/// and is priced (rush-8: the front crawled at a third of a Pawn's speed for four minutes waiting for joiners
/// trickling out of the lab, and its centre lay 2000 elmos behind the Pawn walking into the commander's D-gun).
const BODY_BAND: f32 = 1500.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Mode {
    #[default]
    Idle,
    Going,
    TooSmall,
    Elsewhere,
    Waiting,
}

#[derive(Default)]
pub struct Raid {
    pub(super) members: Vec<UnitId>,
    pub(super) target: Option<Vec3>,
    /// Parties sent this game.
    pub(super) sorties: usize,
    last_order_frame: i32,
    priced_at: i32,
    rest_until: i32,
    /// H-ARMY-KILL: the party stands at the opponent's base and nothing armed of theirs is in sight; the home group is
    /// committed after it (`army.rs` reads this).
    pub(super) kill_offered: Option<Vec3>,
    /// H-ARMY-MARCH: members stopped until the body of the party has come up (Pawns 2000 elmos apart met BARb's
    /// commander one at a time in rush-smoke2).
    held: HashSet<UnitId>,
    /// Waiting out of reach for reinforcements (logged once).
    pub(super) waiting: bool,
    /// What the party was doing last tick; a change means new orders at once, not at the next 4 s slot (rush-16:
    /// the first Pawn kept its fight order for 3 s after sighting the commander and died).
    mode: Mode,
    /// Their turrets the last pricing included, nearest first: the party kills them first, deliberately.
    turrets: Vec<UnitId>,
    /// Armed buildings of theirs known to bear on the target at the last pricing: a new one means a price now.
    turrets_known: usize,
    /// Perimeter points the party has stood at, with when (`perimeter_probe`).
    probed: Vec<(Vec3, i32)>,
    /// Until when the current alternative target holds (`ALTERNATIVE_FRAMES`).
    hold_until: i32,
    /// The wait point in force while waiting: chosen once, moved only when the threat comes within reach of it.
    wait_at: Option<Vec3>,
    /// The last pricing's verdict, held until the next: between pricings the party was "not outmatched" and went
    /// back at the target for four seconds, then retreated for four (rush-11 to 13: parties oscillating at the base).
    pub(super) outmatched: bool,
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

    /// Finding the base: spots round the presumed base or in the enemy start boxes nobody has looked at lately
    /// (`scout.rs`), nearest `from` first. The presumed base is a guess and the opponent may be anywhere in its box:
    /// in Comet Catcher's strips BARb spawns at an end, 2000 elmos from the centre, and the party of rush-7 stood
    /// at an empty spot for five minutes while the home group was committed to it.
    fn unscouted_box_spots(&self, from: Vec3, frame: i32) -> Vec<Vec3> {
        // Round the presumed base; the rest of the box only when nothing round the base is left to look at (the
        // far clusters are the scouts' business: rush-20, the whole party thrashing between two clusters).
        let base = self.enemy_base(from);
        let mut spots: Vec<Vec3> = self.spots_to_look_at(from, frame, 0.0, LIKELY_BASE).into_iter().filter(|s| s.dist2d(base) < PARTY_VICINITY).collect();
        if spots.is_empty() {
            spots = self.spots_to_look_at(from, frame, 0.0, LIKELY_BOX);
        }
        // Not within a small party's death of the commander where it was last seen (its laser reaches 300 and its
        // D-gun kills a Pawn a shot; rush-16: five of eight first Pawns died within six seconds of sighting it).
        spots.retain(|s| !self.commander_ground(*s));
        // Round the presumed base first (rush-15: the first Pawn went to a stale spot in the middle of the box).
        spots.sort_by(|a, b| self.spot_likelihood(*b).total_cmp(&self.spot_likelihood(*a)).then(a.dist2d(from).total_cmp(&b.dist2d(from))));
        spots
    }

    /// Whether `at` lies within the enemy commander's reach where it was last seen, for a party too small for it.
    fn commander_ground(&self, at: Vec3) -> bool {
        self.enemy_commander_seen.is_some_and(|(pos, _)| pos.dist2d(at) < COMMANDER_REACH + 300.0)
    }

    /// Whether `t` is one of the spots nobody has looked at lately (`unscouted_box_spots`).
    fn is_unscouted(&self, t: Vec3, frame: i32) -> bool {
        self.spots_to_look_at(t, frame, 0.0, 1.0).iter().any(|s| s.dist2d(t) < 1.0)
    }

    /// Where the party goes: the nearest extractor of theirs we know of, else a spot in their box nobody has looked
    /// at, else their base as we presume it. H-ARMY-KILL: at their base, known by a building of theirs standing
    /// there, with nothing armed in sight, the commander if we have seen it, else the lab, else any building.
    fn pressure_target(&self, party_at: Vec3, tick: &Tick) -> Option<Vec3> {
        let base = self.enemy_base(party_at);
        let armed_in_sight = tick.snapshot.enemies.iter().any(|e| {
            e.pos.dist2d(party_at) < KILL_RADIUS && e.def.is_none_or(|d| self.world.def(d).is_some_and(|d| d.weapon_count > 0 && d.speed > 0.0))
        });
        let base_known = self.enemy_buildings.values().any(|(_, pos, _)| pos.dist2d(party_at) < BASE_RADIUS);
        if party_at.dist2d(base) < BASE_RADIUS && base_known && !armed_in_sight {
            let commander = self.enemy_commander_seen.filter(|(pos, _)| pos.dist2d(base) < BASE_RADIUS).map(|(pos, _)| pos);
            let lab = self.enemy_buildings.values().filter(|(def, _, _)| self.world.def(*def).is_some_and(|d| !d.build_options.is_empty() && d.speed == 0.0)).map(|(_, pos, _)| *pos);
            let any = self.enemy_buildings.values().map(|(_, pos, _)| *pos).min_by(|a, b| a.dist2d(party_at).total_cmp(&b.dist2d(party_at)));
            return commander.or_else(|| lab.min_by(|a, b| a.dist2d(party_at).total_cmp(&b.dist2d(party_at)))).or(any).or(Some(base));
        }
        if let Some(extractor) = self.raid_targets().iter().find(|t| !self.commander_ground(**t)) {
            return Some(*extractor);
        }
        if self.found_enemy_base().is_none()
            && let Some(spot) = self.unscouted_box_spots(party_at, tick.frame).first()
        {
            return Some(*spot);
        }
        self.reachable_on_foot(base).then_some(base)
    }

    /// Outmatched at `target`: another extractor of theirs, in its base or out of it, or a metal spot in its box
    /// nobody has looked at, that nothing armed and mobile stands at (within 600) and the body is priced to win at
    /// against what stands that close, nearest first. The experienced players' Pawns meet BARb's commander out front
    /// and go round it to the extractors it is not standing on (it is slow); ours went home and rested a minute
    /// (rush-10-qs-place 08). On Quicksilver every extractor of BARb's lies within 1400 of its start, so the raid
    /// list of extractors outside the base is empty there, and on arriving the party knows two of them, one under
    /// the commander and one under an LLT (rush-12): the spots it has not seen are where the rest are.
    fn harass_elsewhere(&mut self, body: &[&OwnUnit], target: Vec3, centre: Vec3, tick: &Tick) -> Option<Vec3> {
        let armed: Vec<Vec3> = tick.snapshot.enemies.iter()
            .filter(|e| e.def.is_none_or(|d| self.world.def(d).is_some_and(|d| d.weapon_count > 0 && d.speed > 0.0)))
            .map(|e| e.pos)
            .collect();
        // Any structure of theirs round the base is a target when unguarded (the user: feel round the perimeter
        // for unguarded structures), the extractors and the rest alike; then spots round the base nobody has looked at.
        let base = self.enemy_base(centre);
        let mut candidates: Vec<Vec3> = self.enemy_buildings.values()
            .filter(|(def, pos, _)| self.world.def(*def).is_some_and(|d| d.weapon_count == 0) && pos.dist2d(base) < PARTY_VICINITY)
            .map(|(_, pos, _)| *pos)
            .chain(self.unscouted_box_spots(centre, tick.frame))
            // Not where the party already stands: a candidate within arrival distance of the centre counted as
            // reached the next tick and was dropped for the next (raid-debug: every alternative "arrived" at once).
            .filter(|t| t.dist2d(target) > TARGET_RADIUS && t.dist2d(centre) > TARGET_RADIUS && !armed.iter().any(|a| a.dist2d(*t) < TARGET_RADIUS) && !self.commander_ground(*t) && self.reachable_on_foot(*t))
            .collect();
        candidates.sort_by(|a, b| a.dist2d(centre).total_cmp(&b.dist2d(centre)));
        // Priced as the party's target will be, over the same radius (rush-16: a target priced won at 600 and lost at
        // 1000 had the party go and retreat every four seconds under a turret).
        let found = candidates.into_iter().take(ELSEWHERE_TRIES).find(|t| self.assault_verdict(body, *t, CONTACT_RADIUS, tick).gain >= GO_GAIN);
        found.or_else(|| self.perimeter_probe(centre, tick.frame))
    }

    /// The nearest point on a ring round the presumed base the party has not stood at lately, off known turrets'
    /// ground and the commander's: walking it shows what stands round the base and where it is unguarded.
    fn perimeter_probe(&mut self, centre: Vec3, frame: i32) -> Option<Vec3> {
        self.raid.probed.retain(|(_, at)| frame - at < PROBE_MEMORY);
        let base = self.enemy_base(centre);
        let armed: Vec<Vec3> = self.enemy_buildings.values().filter(|(def, _, _)| self.world.def(*def).is_some_and(|d| d.weapon_count > 0)).map(|(_, pos, _)| *pos).collect();
        (0..PERIMETER_POINTS)
            .map(|k| {
                let angle = k as f32 / PERIMETER_POINTS as f32 * std::f32::consts::TAU;
                Vec3 { x: base.x + angle.cos() * PERIMETER, y: 0.0, z: base.z + angle.sin() * PERIMETER }
            })
            .filter(|p| self.reachable_on_foot(*p) && !self.commander_ground(*p) && !armed.iter().any(|a| a.dist2d(*p) < TURRET_BERTH))
            .filter(|p| !self.raid.probed.iter().any(|(q, _)| q.dist2d(*p) < 1.0) && p.dist2d(centre) > TARGET_RADIUS)
            .min_by(|a, b| a.dist2d(centre).total_cmp(&b.dist2d(centre)))
    }

    /// `soldiers`: finished soldiers no squad has claimed. Returns with the party's orders pushed.
    pub(super) fn run_raid(&mut self, tick: &Tick, kit: &Kit, soldiers: &[&OwnUnit], commands: &mut Vec<Command>) {
        self.raid.kill_offered = None;
        if !self.enabled("H-ARMY-PRESSURE") || self.directives.pressure.is_some_and(|p| !p.value) {
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
            self.raid.outmatched = false;
            self.raid.sorties += 1;
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
        let front = self.raid.target.map(|t| party.iter().map(|u| u.pos.dist2d(t)).fold(f32::INFINITY, f32::min));
        let body: Vec<&OwnUnit> = match (front, self.raid.target) {
            (Some(front), Some(t)) => party.iter().copied().filter(|u| u.pos.dist2d(t) < front + BODY_BAND).collect(),
            _ => party.clone(),
        };
        let Some(centre) = centre(&body) else { return };
        // The target is gone when we no longer remember an extractor there (seen destroyed, or found missing), or when
        // the party stands on it and sees nothing. A box spot the party reaches is scouted, whatever it found.
        let arrived = self.raid.target.is_some_and(|t| centre.dist2d(t) < TARGET_RADIUS);
        if arrived && let Some(t) = self.raid.target && t.dist2d(self.enemy_base(t)) > PERIMETER - 1.0 && t.dist2d(self.enemy_base(t)) < PERIMETER + 1.0 {
            self.raid.probed.push((t, tick.frame));
        }
        let unscouted = self.raid.target.is_some_and(|t| self.is_unscouted(t, tick.frame));
        let held = !arrived && tick.frame < self.raid.hold_until;
        let still_there = self.raid.target.is_some_and(|t| held || self.enemy_buildings.values().any(|(_, pos, _)| pos.dist2d(t) < 100.0) || (!arrived && (unscouted || t.dist2d(self.enemy_base(t)) < BASE_RADIUS)));
        let target = if still_there { self.raid.target } else { self.pressure_target(centre, tick) };
        if std::env::var_os("WITHIN_REASON_RAID_DEBUG").is_some() && let Some(t) = self.raid.target && target != Some(t) {
            eprintln!("[ai {}] f={} raid-debug: target {:?} dropped: arrived={arrived} held={held} hold_until={} unscouted={unscouted} mode={:?} -> {:?}", self.ai(), tick.frame, (t.x as i32, t.z as i32), self.raid.hold_until, self.raid.mode, target.map(|p| (p.x as i32, p.z as i32)));
        }
        // Priced every few seconds against what is in sight of the party and what is known at the target, and every
        // tick while something armed is in sight: four seconds is a fight's length.
        let armed = |e: &bot_protocol::EnemyUnit| e.def.is_none_or(|d| self.world.def(d).is_some_and(|d| d.weapon_count > 0 && d.speed > 0.0));
        let is_commander = |e: &bot_protocol::EnemyUnit| e.def.is_some_and(|d| self.world.def(d).is_some_and(|d| d.name.ends_with("com") && d.build_speed > 0.0));
        let in_sight: Vec<&bot_protocol::EnemyUnit> = tick.snapshot.enemies.iter().filter(|e| e.pos.dist2d(centre) < CONTACT_RADIUS).collect();
        let armed_in_sight = in_sight.iter().any(|e| armed(e) && !is_commander(e));
        let commander_at = in_sight.iter().find(|e| is_commander(e)).map(|e| e.pos);
        let armed_in_sight_at: Vec<Vec3> = in_sight.iter().filter(|e| armed(e)).map(|e| e.pos).collect();
        // In sight at all is near enough: the first sighting comes at 200-300 elmos (rush-16), inside its laser.
        let commander_near = commander_at.is_some();
        let turrets_known = target.map_or(0, |t| self.turrets_bearing(centre, t, CONTACT_RADIUS).len());
        let reprice = in_sight.iter().any(|e| armed(e)) || tick.frame - self.raid.priced_at >= REPRICE_FRAMES || turrets_known != self.raid.turrets_known;
        let party_metal: f32 = body.iter().map(|u| self.world.def(u.def).map_or(0.0, |d| d.metal_cost)).sum();
        let too_small_for_commander = commander_near && party_metal < COMMANDER_PARTY_METAL;
        if reprice {
            self.raid.priced_at = tick.frame;
            let verdict = self.assault_verdict(&body, target.unwrap_or(centre), CONTACT_RADIUS, tick);
            self.raid.outmatched = verdict.gain < GO_GAIN;
            self.raid.turrets = verdict.turrets;
            self.raid.turrets_known = turrets_known;
        }
        let outmatched = self.raid.outmatched && !too_small_for_commander;
        let mode = match target {
            Some(_) if too_small_for_commander && party.len() >= PARTY => Mode::TooSmall,
            Some(_) if !outmatched && party.len() >= PARTY => Mode::Going,
            Some(_) if party.len() >= PARTY => if self.raid.waiting { Mode::Waiting } else { Mode::Elsewhere },
            _ => Mode::Idle,
        };
        if mode != self.raid.mode {
            self.raid.mode = mode;
            self.raid.last_order_frame = 0;
            if mode != Mode::Waiting {
                self.raid.wait_at = None;
            }
        }
        match target {
            Some(target) if too_small_for_commander && party.len() >= PARTY => {
                // Too few for the commander: wait for the rest out of its reach, as a player gathers at the edge of a
                // base, rather than walk home and lose the ground already covered.
                self.raid.target = Some(target);
                let Some(commander) = commander_at.or(self.enemy_commander_seen.map(|(pos, _)| pos)) else { return };
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
                self.raid.waiting = false;
                // H-ARMY-KILL: at their base with no soldier of theirs in sight, and the commander either out of sight
                // or outnumbered, the home group comes to finish it.
                // A party too small for the commander is no kill, in sight of it or not: it is usually a step away
                // (cmd-harness-smoke: the commander was woken nine times by one to three Pawns at the base).
                let kill_open = !armed_in_sight && party_metal >= COMMANDER_PARTY_METAL;
                let at_their_base = centre.dist2d(self.enemy_base(centre)) < BASE_RADIUS && self.enemy_buildings.values().any(|(_, pos, _)| pos.dist2d(centre) < BASE_RADIUS);
                if at_their_base && kill_open && self.enabled("H-ARMY-KILL") {
                    self.raid.kill_offered = Some(target);
                }
                let mut held = std::mem::take(&mut self.raid.held);
                if tick.frame - self.raid.last_order_frame >= REPRICE_FRAMES {
                    self.raid.last_order_frame = tick.frame;
                    // A turret priced in is killed first, deliberately, by everybody; then the target.
                    let turret = self.raid.turrets.iter().find(|id| self.enemy_buildings.contains_key(id)).copied();
                    for u in party.iter().filter(|u| !held.contains(&u.id)) {
                        if let Some(turret) = turret {
                            commands.push(Command::Attack { unit: u.id, target: turret, queue: false });
                            commands.push(Command::Fight { unit: u.id, to: target, queue: true });
                        } else {
                            commands.push(Command::Fight { unit: u.id, to: target, queue: false });
                        }
                    }
                }
                commands.extend(self.march(&mut held, &body, target, tick.snapshot.enemies.as_slice()));
                self.raid.held = held;
            }
            Some(target) if party.len() >= PARTY => {
                // Outmatched here: an extractor of theirs elsewhere, or wait out of reach of the nearest threat for
                // the Pawns still coming. Home is for a party with nobody left.
                if let Some(next) = self.harass_elsewhere(&body, target, centre, tick) {
                    if self.raid.target.is_none_or(|t| t.dist2d(next) > 1.0) {
                        eprintln!("[ai {}] f={} pressure: party of {} outmatched at ({:.0}, {:.0}), goes for ({:.0}, {:.0}) instead", self.ai(), tick.frame, body.len(), target.x, target.z, next.x, next.z);
                    }
                    self.raid.mode = Mode::Elsewhere;
                    self.raid.target = Some(next);
                    self.raid.hold_until = tick.frame + ALTERNATIVE_FRAMES;
                    self.raid.last_order_frame = tick.frame;
                    self.raid.waiting = false;
                    self.raid.outmatched = false;
                    self.raid.priced_at = 0;
                    self.raid.held.clear();
                    commands.extend(party.iter().map(|u| Command::Move { unit: u.id, to: next, queue: false }));
                } else {
                    let threats = armed_in_sight_at.iter().copied()
                        .chain(self.enemy_buildings.values().filter(|(def, pos, _)| pos.dist2d(target) < CONTACT_RADIUS && self.world.def(*def).is_some_and(|d| d.weapon_count > 0)).map(|(_, pos, _)| *pos));
                    let threat = threats.min_by(|a, b| a.dist2d(centre).total_cmp(&b.dist2d(centre))).unwrap_or(target);
                    // Held where the party first fell back to; moved only when the threat comes within reach of it
                    // (rush-20: a wait point recomputed as their Pawns advanced walked the party home).
                    let wait = match self.raid.wait_at {
                        Some(at) if at.dist2d(threat) > COMMANDER_REACH => at,
                        _ => {
                            let (dx, dz) = (centre.x - threat.x, centre.z - threat.z);
                            let len = dx.hypot(dz).max(1.0);
                            let at = Vec3 { x: threat.x + dx / len * WAIT_OFF, y: 0.0, z: threat.z + dz / len * WAIT_OFF };
                            self.raid.wait_at = Some(at);
                            self.raid.last_order_frame = 0;
                            at
                        }
                    };
                    if !self.raid.waiting {
                        eprintln!("[ai {}] f={} pressure: party of {} outmatched at ({:.0}, {:.0}), waits at ({:.0}, {:.0}) for more", self.ai(), tick.frame, body.len(), target.x, target.z, wait.x, wait.z);
                        self.raid.waiting = true;
                        self.raid.mode = Mode::Waiting;
                        self.raid.last_order_frame = 0;
                    }
                    if tick.frame - self.raid.last_order_frame >= REPRICE_FRAMES {
                        self.raid.last_order_frame = tick.frame;
                        commands.extend(party.iter().map(|u| Command::Move { unit: u.id, to: wait, queue: false }));
                    }
                }
            }
            _ => {
                eprintln!("[ai {}] f={} pressure: party of {} comes home ({})", self.ai(), tick.frame, party.len(), if party.len() < PARTY { "too few left" } else { "no target" });
                let station = self.last_station;
                commands.extend(party.iter().map(|u| Command::Move { unit: u.id, to: station, queue: false }));
                self.raid.members.clear();
                self.raid.held.clear();
                self.raid.waiting = false;
                self.raid.outmatched = false;
                self.raid.mode = Mode::Idle;
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
