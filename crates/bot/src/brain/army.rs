//! Combat units: defend the base, gather, attack in growing waves.
//!
//! Units are either in the home group or committed attackers. Membership, not position or
//! idleness, decides which: a crowd at the rally point is never all idle at once.

use std::collections::HashSet;

use bot_protocol::{Command, EnemyUnit, OwnUnit, Tick, UnitId, Vec3};

use super::combat::Force;
use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};
use crate::strategist::shared::Stance;

/// BARb medium holds its army until about minute 10 (30 units) while ours left in eights and died (observe-1).
const FIRST_WAVE: usize = 20;
const WAVE_GROWTH: usize = 5;
const MAX_WAVE: usize = 40;
/// Smallest home group an `attack` stance will commit.
const MIN_ORDERED_WAVE: usize = 3;
/// A wave of at least this many that is wiped out is worth waking the strategist for.
const NOTABLE_WAVE: usize = 5;
/// Enemies this close to home are an attack on the base.
pub(super) const BASE_RADIUS: f32 = 1400.0;
/// Enemies this close to one of our extractors are raiding it.
const RAID_RADIUS: f32 = 500.0;
/// The strategist is woken for a base attack of at least this many; lone raiders are routine.
const NOTABLE_INTRUSION: usize = 3;
/// The default station stands this far from the most exposed extractor, on its home side.
const STATION_LEAD: f32 = 250.0;
/// Buildings this close to a target given up as unreachable are skipped too (they share its ledge).
const BAD_TARGET_RADIUS: f32 = 350.0;
/// A candidate station this close to one given up as unreachable is skipped too.
const BAD_STATION_RADIUS: f32 = 200.0;
/// Extractors nearer to home than this are covered by the base itself.
const OUTPOST_DISTANCE: f32 = 600.0;
/// H-ARMY-HOME-GUARD: a wave leaves this many soldiers (those nearest the station) at home.
const HOME_GUARD: usize = 6;
/// H-ARMY-WAVE-GATE: a wave goes only at a target where its predicted odds (`combat.rs`) against the known defenders
/// are at least this. An enemy commander at home counts as a fixed value of turret.
const WAVE_ADVANTAGE: f32 = 1.3;
const COMMANDER_WORTH: f32 = 1500.0;
/// H-ARMY-RETREAT: attackers whose odds against what they can see fall below this break off and go home; they
/// are judged this often, against enemies within this distance of their leading group.
const RETREAT_ODDS: f32 = 0.6;
/// After breaking off, the army masses for this long before it may go again.
const AFTER_RETREAT_FRAMES: i32 = 90 * FRAMES_PER_SECOND;
const RETREAT_CHECK_FRAMES: i32 = 2 * FRAMES_PER_SECOND;
pub(super) const CONTACT_RADIUS: f32 = 1000.0;
/// Responders are added until the odds against the raiders reach this.
pub(super) const RESPONSE_ODDS: f32 = 1.5;
/// Known defenders are the remembered armed buildings and the soldiers in sight this close to the target.
const DEFENDED_RADIUS: f32 = 900.0;
/// No wave leaves within this long of losing something on our side of the map.
const QUIET_BEFORE_WAVE_FRAMES: i32 = 30 * FRAMES_PER_SECOND;
const QUIET_CEILING_FRAMES: i32 = 120 * FRAMES_PER_SECOND;
/// How long the biggest enemy force seen stays in mind.
/// H-ARMY-SCOUT: a raider goes to look at the enemy this often, so the wave gate weighs something it has seen.
const SCOUT_EVERY_FRAMES: i32 = 90 * FRAMES_PER_SECOND;
const ENEMY_ARMY_MEMORY_FRAMES: i32 = 120 * FRAMES_PER_SECOND;
/// H-ARMY-RESPONDERS: raiders are met by the nearest soldiers, at least this many and as many as good odds take,
/// not by the whole home group trailing across the map.
pub(super) const MIN_RESPONDERS: usize = 4;
/// An idle attacker this close to the attack target has arrived and needs a new one.
const ARRIVED_RADIUS: f32 = 400.0;
/// Home-group units farther than this from the rally point are called in.
const RALLY_RADIUS: f32 = 600.0;
/// The station keeps this far from every factory.
const LAB_CLEARANCE: f32 = 450.0;
/// H-ARMY-STAGE: attackers gather this far short of the target before going in together.
const STAGE_DISTANCE: f32 = 1500.0;
/// Attackers this close to the staging point have gathered.
const STAGE_RADIUS: f32 = 500.0;
/// The assault starts once this share of the attackers has gathered, or after this long.
const STAGE_QUORUM: f32 = 0.7;
const STAGE_PATIENCE_FRAMES: i32 = 150 * FRAMES_PER_SECOND;
/// This many enemies at the base call every attacker home.
const RECALL_INTRUDERS: usize = 6;
const DEFEND_REORDER_FRAMES: i32 = 5 * FRAMES_PER_SECOND;

#[derive(Default)]
pub struct Army {
    attackers: HashSet<UnitId>,
    waves_sent: usize,
    /// Where attacks go: a remembered enemy building, else a swept metal spot.
    target: Option<Vec3>,
    /// Next metal spot to sweep when attackers find nothing at their target.
    sweep_index: usize,
    last_defend_order: i32,
    /// Stations given up as unreachable, with the frame until which to avoid them.
    bad_stations: Vec<(Vec3, i32)>,
    /// Attack targets given up as unreachable, with the frame until which to avoid them.
    bad_targets: Vec<(Vec3, i32)>,
    target_failures: u32,
    /// Frame at which the current target was chosen, or last approached.
    target_since: i32,
    station_failures: u32,
    /// When the attackers last broke off; no new wave for a while after.
    last_retreat_frame: i32,
    /// Since when a full wave has been held back only because things were dying at home.
    held_for_losses_since: Option<i32>,
    /// The biggest enemy soldier force seen in one look lately, and when: the opponent's army is one mobile block,
    /// so what stands at a target now says little about what a wave will meet there.
    enemy_army_seen: Option<(Force, i32)>,
    /// The soldier last sent to look at the enemy, and when.
    scout: Option<(UnitId, i32)>,
    /// Where the attackers are gathering before the assault, and since which frame.
    staging: Option<(Vec3, i32)>,
    /// H-ARMY-MARCH: attackers stopped until the body of the wave has come up.
    held: HashSet<UnitId>,
}

impl Army {
    pub fn is_attacker(&self, unit: UnitId) -> bool {
        self.attackers.contains(&unit)
    }

    /// A soldier claimed by a squad is no longer an attacker.
    pub fn release(&mut self, unit: UnitId) {
        self.attackers.remove(&unit);
    }

    pub fn waves_sent(&self) -> usize {
        self.waves_sent
    }

    pub fn target(&self) -> Option<Vec3> {
        self.target
    }

    pub fn staging_point(&self) -> Option<Vec3> {
        self.staging.map(|(point, _)| point)
    }
}

impl Brain {
    /// Where the home group waits: the strategist's station, else just ahead of our most exposed
    /// outpost extractor, else in front of the base.
    pub(super) fn station(&mut self, tick: &Tick, kit: &Kit) -> Vec3 {
        if let Some(ordered) = self.directives.army_station {
            self.fire("D-ARMY-STATION");
            return ordered.value;
        }
        let most_exposed = tick
            .snapshot
            .own_units
            .iter()
            .filter(|u| kit.is_extractor(u.def) && u.pos.dist2d(self.home) > OUTPOST_DISTANCE)
            .min_by(|a, b| a.pos.dist2d(self.enemy_base(a.pos)).total_cmp(&b.pos.dist2d(self.enemy_base(b.pos))));
        // Candidates in order of preference. The outpost station sits on the home side of the extractor, on ground
        // our constructor walked to build it; a point ahead of it towards the enemy was often unreachable, and
        // units that cannot reach their station pile up at the factory exit.
        let outpost = most_exposed.filter(|_| self.enabled("H-ARMY-STATION")).map(|extractor| {
            let (dx, dz) = (self.home.x - extractor.pos.x, self.home.z - extractor.pos.z);
            let len = dx.hypot(dz).max(1.0);
            Vec3 { x: extractor.pos.x + dx / len * STATION_LEAD, y: 0.0, z: extractor.pos.z + dz / len * STATION_LEAD }
        });
        // Never in the lab yard: thirty soldiers parked on the factory exits jam the labs, and production stalls with
        // metal in the bank (v18 match 14: 1500 banked for five minutes, soldiers' moves failing beside the base).
        let labs: Vec<Vec3> = tick.snapshot.own_units.iter().filter(|u| u.def == kit.lab).map(|u| u.pos).collect();
        let clear_of_labs = |point: &Vec3| labs.iter().all(|lab| lab.dist2d(*point) > LAB_CLEARANCE);
        let candidates = [outpost, Some(self.forward_of_home(900.0)), Some(self.forward_of_home(1200.0)), Some(self.forward_of_home(700.0))];
        let usable = |point: &Vec3| {
            !self.army.bad_stations.iter().any(|(bad, until)| *until > tick.frame && bad.dist2d(*point) < BAD_STATION_RADIUS)
        };
        // Never the start point itself: it stands in the middle of the generator field.
        candidates.into_iter().flatten().filter(clear_of_labs).find(usable).unwrap_or(self.forward_of_home(900.0))
    }

    /// Home-group units that cannot reach the station mean the station is a bad place; give it up for a while.
    fn note_station_failures(&mut self, tick: &Tick, kit: &Kit) {
        const FAILURES_TO_GIVE_UP: u32 = 6;
        const GIVE_UP_FRAMES: i32 = 5 * 60 * FRAMES_PER_SECOND;
        let failures = tick
            .events
            .iter()
            .filter(|e| {
                let bot_protocol::Event::UnitMoveFailed { unit } = e else { return false };
                let soldier = tick.snapshot.own_units.iter().find(|u| u.id == *unit).is_some_and(|u| self.is_army(u, kit));
                soldier && !self.army.attackers.contains(unit) && !self.squads.contains(*unit)
            })
            .count() as u32;
        self.army.station_failures += failures;
        if self.army.station_failures >= FAILURES_TO_GIVE_UP {
            self.army.station_failures = 0;
            let station = self.last_station;
            eprintln!("[ai {}] f={} station ({:.0}, {:.0}) is unreachable; trying the next", self.ai(), tick.frame, station.x, station.z);
            self.army.bad_stations.push((station, tick.frame + GIVE_UP_FRAMES));
        }
    }

    /// Attackers whose moves keep failing cannot path to the target (a ledge, an island). Without this the whole
    /// army walks to the nearest reachable point and is re-sent there every tick for the rest of the game.
    fn note_unreachable_targets(&mut self, tick: &Tick, attackers_nearest: Option<f32>) {
        const FAILURES_TO_GIVE_UP: u32 = 10;
        const PATIENCE_FRAMES: i32 = 90 * FRAMES_PER_SECOND;
        /// Somebody got this close: the target is reachable, and the failures are a crowd jostling.
        const CLOSE_ENOUGH: f32 = 600.0;
        const GIVE_UP_FRAMES: i32 = 4 * 60 * FRAMES_PER_SECOND;
        self.army.bad_targets.retain(|(_, until)| *until > tick.frame);
        let failures = tick
            .events
            .iter()
            .filter(|e| matches!(e, bot_protocol::Event::UnitMoveFailed { unit } if self.army.attackers.contains(unit)))
            .count() as u32;
        self.army.target_failures += failures;
        let Some(target) = self.army.target else { return };
        if attackers_nearest.is_some_and(|d| d < CLOSE_ENOUGH) {
            self.army.target_failures = 0;
            self.army.target_since = tick.frame;
        }
        if self.army.target_failures >= FAILURES_TO_GIVE_UP && tick.frame - self.army.target_since > PATIENCE_FRAMES {
            eprintln!("[ai {}] f={} target ({:.0}, {:.0}) is unreachable; choosing another", self.ai(), tick.frame, target.x, target.z);
            self.army.bad_targets.push((target, tick.frame + GIVE_UP_FRAMES));
            self.army.target = None;
        }
    }

    /// What we know stands within `radius` of `place`: remembered armed buildings, soldiers in sight, and the enemy
    /// commander if this is its base. A floor: what we have not seen is not counted.
    pub(super) fn known_enemy_force(&self, place: Vec3, radius: f32, visible: &[EnemyUnit]) -> Force {
        let mut force = Force::default();
        for (def, pos, _) in self.enemy_buildings.values() {
            if pos.dist2d(place) < radius
                && let Some(d) = self.world.def(*def).filter(|d| d.weapon_count > 0)
            {
                force.turret_metal += d.metal_cost;
            }
        }
        for enemy in visible.iter().filter(|e| e.pos.dist2d(place) < radius) {
            let Some(def) = enemy.def else {
                // Radar only. A blip standing where we remember a building is that building, already counted.
                if !self.enemy_buildings.values().any(|(_, pos, _)| pos.dist2d(enemy.pos) < 40.0) {
                    force.unidentified += 1;
                }
                continue;
            };
            if self.world.def(def).is_some_and(|d| d.speed > 0.0 && d.build_speed == 0.0 && d.weapon_count > 0) {
                force.add(def);
            }
        }
        if place.dist2d(self.enemy_base(place)) < radius {
            force.turret_metal += COMMANDER_WORTH / super::combat::TURRET_WORTH;
        }
        force
    }

    pub(super) fn force_of(units: &[&OwnUnit]) -> Force {
        let mut force = Force::default();
        for unit in units {
            force.add(unit.def);
        }
        force
    }

    /// A remembered building that our soldiers are standing next to and cannot see is gone.
    fn forget_razed_buildings(&mut self, soldiers: &[&OwnUnit], visible: &[EnemyUnit]) {
        const IN_PLAIN_SIGHT: f32 = 250.0;
        self.enemy_buildings.retain(|id, (_, pos, _)| {
            let we_are_there = soldiers.iter().any(|u| u.pos.dist2d(*pos) < IN_PLAIN_SIGHT);
            !we_are_there || visible.iter().any(|e| e.id == *id)
        });
    }

    pub(super) fn run_army(&mut self, tick: &Tick, kit: &Kit, commands: &mut Vec<Command>) {
        let snapshot = &tick.snapshot;
        let every_soldier: Vec<&OwnUnit> =
            snapshot.own_units.iter().filter(|u| !u.being_built && self.is_army(u, kit)).collect();
        self.run_squads(tick, kit, &every_soldier, commands);
        let unclaimed: Vec<&OwnUnit> = every_soldier.into_iter().filter(|u| !self.squads.contains(u.id)).collect();
        self.run_raid(tick, kit, &unclaimed, commands);
        let soldiers: Vec<&OwnUnit> = unclaimed.into_iter().filter(|u| !self.raid.contains(u.id)).collect();

        for event in &tick.events {
            let bot_protocol::Event::UnitMoveFailed { unit } = event else { continue };
            self.move_failures += 1;
            if let Some(u) = snapshot.own_units.iter().find(|u| u.id == *unit && self.is_army(u, kit)) {
                // Where soldiers get stuck, in 200-elmo cells, for the per-minute log.
                *self.stuck_cells.entry(((u.pos.x / 200.0) as i32, (u.pos.z / 200.0) as i32)).or_default() += 1;
            }
            if self.move_failures <= 30
                && let Some(u) = snapshot.own_units.iter().find(|u| u.id == *unit)
            {
                eprintln!(
                    "[ai {}] f={} MOVE FAILED: {} at ({:.0}, {:.0}), attacker={}, army target {:?}",
                    self.ai(), tick.frame, self.name(u.def), u.pos.x, u.pos.z, self.army.attackers.contains(unit),
                    self.army.target.map(|t| (t.x as i32, t.z as i32))
                );
            }
        }

        let in_sight = self.known_enemy_force(Vec3::default(), f32::INFINITY, snapshot.enemies.as_slice());
        let in_sight = Force { turret_metal: 0.0, ..in_sight };
        let nothing = Force::default();
        let stale = self.army.enemy_army_seen.as_ref().is_none_or(|(_, seen)| tick.frame - seen > ENEMY_ARMY_MEMORY_FRAMES);
        let bigger = self.army.enemy_army_seen.as_ref().is_none_or(|(army, _)| self.odds(&in_sight, &nothing) >= self.odds(army, &nothing));
        if !in_sight.units.is_empty() && (stale || bigger) {
            self.army.enemy_army_seen = Some((in_sight, tick.frame));
        } else if stale {
            self.army.enemy_army_seen = None;
        }

        let committed = self.army.attackers.len();
        self.army.attackers.retain(|id| soldiers.iter().any(|u| u.id == *id));
        if committed >= NOTABLE_WAVE && self.army.attackers.is_empty() {
            self.trigger("wave-lost", tick.frame, format!("Our attack group of {committed} has been wiped out."));
        }

        let nearest_to = |point: Vec3| {
            snapshot.enemies.iter().min_by(|a, b| a.pos.dist2d(point).total_cmp(&b.pos.dist2d(point)))
        };
        // H-ARMY-TARGET: attack buildings, never whatever unit was seen last. Raiders near our own base used to
        // drag every wave back into our half of the map. Roll the enemy up from the outside: the remembered
        // building nearest to us, else where the enemy presumably started.
        self.forget_razed_buildings(&soldiers, snapshot.enemies.as_slice());
        let previous_target = self.army.target;
        let attackers_nearest = previous_target.and_then(|t| {
            soldiers.iter().filter(|u| self.army.attackers.contains(&u.id)).map(|u| u.pos.dist2d(t)).min_by(f32::total_cmp)
        });
        self.note_unreachable_targets(tick, attackers_nearest);
        let reachable = |point: &Vec3| {
            self.reachable_on_foot(*point) && !self.army.bad_targets.iter().any(|(bad, _)| bad.dist2d(*point) < BAD_TARGET_RADIUS)
        };
        let nearest_building = self
            .enemy_buildings
            .values()
            .map(|(_, pos, _)| *pos)
            .filter(reachable)
            .min_by(|a, b| a.dist2d(self.home).total_cmp(&b.dist2d(self.home)));
        self.army.target = nearest_building.or(self.army.target);
        if self.army.target.zip(previous_target).is_none_or(|(now, before)| now.dist2d(before) > 50.0) {
            self.army.target_since = tick.frame;
            self.army.target_failures = 0;
        }
        // No building known: the nearest live base (`bases.rs`), found or guessed.
        let mut target = nearest_building.or(self.army.target).unwrap_or(self.enemy_base(self.home));
        if let Some(ordered) = self.directives.attack_target {
            self.fire("D-ATTACK-TARGET");
            target = ordered.value;
        }
        let stance = self.directives.army_stance.map(|s| s.value);
        self.note_station_failures(tick, kit);
        let rally = self.station(tick, kit);
        self.last_station = rally;

        if stance == Some(Stance::Defend) && !self.army.attackers.is_empty() {
            // Attackers come home and rejoin the home group.
            self.fire("D-STANCE-DEFEND");
            commands.extend(self.army.attackers.iter().map(|id| Command::Move { unit: *id, to: rally, queue: false }));
            self.army.attackers.clear();
        }
        // H-ARMY-RECALL: a real attack on the base outranks the offensive. The game ends with the commander, and
        // enemy groups have walked in and killed it within seconds of a wave leaving.
        let intruders = snapshot.enemies.iter().filter(|e| e.pos.dist2d(self.home) < BASE_RADIUS).count();
        if self.enabled("H-ARMY-RECALL") && intruders >= RECALL_INTRUDERS && !self.army.attackers.is_empty() {
            self.fire("H-ARMY-RECALL");
            eprintln!("[ai {}] f={} recall: {intruders} enemies at the base, {} attackers called home", self.ai(), tick.frame, self.army.attackers.len());
            self.journal.note(tick.frame, "recall", serde_json::json!({ "intruders": intruders }), serde_json::json!({ "attackers_called_home": self.army.attackers.len() }));
            self.army.attackers.clear();
            self.army.staging = None;
            self.army.last_defend_order = 0;
        }
        let (attackers, home_group): (Vec<&OwnUnit>, Vec<&OwnUnit>) =
            soldiers.iter().partition(|u| self.army.attackers.contains(&u.id));

        // H-ARMY-SCOUT: unscouted, the wave gate knows only the enemy commander, and lets a wave walk into their whole
        // army (v18 match 20: "1500 known" against 3205). One raider at a time goes to look, by way of the target.
        let scouting = self.army.scout.is_some_and(|(id, since)| tick.frame - since < SCOUT_EVERY_FRAMES && soldiers.iter().any(|u| u.id == id));
        if self.enabled("H-ARMY-SCOUT") && !scouting && tick.frame > 3 * 60 * FRAMES_PER_SECOND
            && let Some(scout) = home_group.iter().filter(|u| u.def == kit.raider).min_by(|a, b| a.pos.dist2d(target).total_cmp(&b.pos.dist2d(target)))
        {
            self.fire("H-ARMY-SCOUT");
            self.army.scout = Some((scout.id, tick.frame));
            commands.push(Command::Move { unit: scout.id, to: target, queue: false });
            // On to a base nobody has seen yet, if there is one.
            let unseen = self.enemy_bases.iter().filter(|b| !b.found && !b.dead).map(|b| b.at).min_by(|a, b| a.dist2d(target).total_cmp(&b.dist2d(target)));
            commands.push(Command::Move { unit: scout.id, to: unseen.unwrap_or(self.enemy_base(target)), queue: true });
        }
        let scout_id = self.army.scout.map(|(id, _)| id);
        let home_group: Vec<&OwnUnit> = home_group.into_iter().filter(|u| Some(u.id) != scout_id).collect();

        // Defence first: the home group turns on intruders at the base, then on raiders at any
        // extractor, and no wave leaves meanwhile.
        let at_base = nearest_to(self.home).filter(|e: &&EnemyUnit| e.pos.dist2d(self.home) < BASE_RADIUS);
        let raider = || {
            let extractors = snapshot.own_units.iter().filter(|u| kit.is_extractor(u.def));
            extractors
                .filter_map(|x| nearest_to(x.pos).filter(|e| e.pos.dist2d(x.pos) < RAID_RADIUS))
                .min_by(|a, b| a.pos.dist2d(rally).total_cmp(&b.pos.dist2d(rally)))
        };
        let (intruder, rule) = match at_base {
            Some(enemy) => (Some(enemy), "H-ARMY-DEFEND"),
            None if self.enabled("H-ARMY-DEFEND-OUTPOST") => (raider(), "H-ARMY-DEFEND-OUTPOST"),
            None => (None, "H-ARMY-DEFEND-OUTPOST"),
        };
        if let Some(intruder) = intruder {
            if tick.frame - self.army.last_defend_order >= DEFEND_REORDER_FRAMES {
                self.army.last_defend_order = tick.frame;
                self.fire(rule);
                let count = snapshot.enemies.iter().filter(|e| e.pos.dist2d(self.home) < BASE_RADIUS).count();
                if count >= NOTABLE_INTRUSION {
                    let grid = self.world.grid(intruder.pos);
                    self.trigger("base-attack", tick.frame, format!("Our base is under attack: {count} enemies near {grid}."));
                }
                // An attack on the base is everyone's business; raiders at an outpost are met by the nearest few.
                let mut nearest: Vec<&&OwnUnit> = home_group.iter().collect();
                nearest.sort_by(|a, b| a.pos.dist2d(intruder.pos).total_cmp(&b.pos.dist2d(intruder.pos)));
                let wanted = if rule == "H-ARMY-DEFEND" || !self.enabled("H-ARMY-RESPONDERS") {
                    home_group.len()
                } else {
                    // The nearest soldiers, as many as it takes for good odds against the raiders in sight there.
                    let raiders = self.known_enemy_force(intruder.pos, RAID_RADIUS, snapshot.enemies.as_slice());
                    let enough = (MIN_RESPONDERS..=nearest.len()).find(|&k| {
                        let group: Vec<&OwnUnit> = nearest[..k].iter().map(|u| **u).collect();
                        self.odds(&Self::force_of(&group), &raiders) >= RESPONSE_ODDS
                    });
                    enough.unwrap_or(nearest.len())
                };
                commands.extend(nearest.iter().take(wanted).map(|u| Command::Fight { unit: u.id, to: intruder.pos, queue: false }));
            }
        } else {
            let own_wave_size = (FIRST_WAVE + WAVE_GROWTH * self.army.waves_sent).min(MAX_WAVE);
            let wave_size = match (stance, self.directives.wave_size) {
                (Some(Stance::Attack), _) => MIN_ORDERED_WAVE,
                (_, Some(ordered)) => ordered.value,
                _ => own_wave_size,
            };
            let may_launch = !matches!(stance, Some(Stance::Defend | Stance::Gather));
            // H-ARMY-HOME-GUARD: the soldiers nearest the station stay; the rest are the wave.
            let guard = if self.enabled("H-ARMY-HOME-GUARD") { HOME_GUARD } else { 0 };
            let mut by_station: Vec<&OwnUnit> = home_group.clone();
            by_station.sort_by(|a, b| a.pos.dist2d(rally).total_cmp(&b.pos.dist2d(rally)));
            let wave: Vec<&OwnUnit> = by_station.into_iter().skip(guard).collect();
            // Under continuous raiding it is never quiet; then the clause would keep the army home for good. It may
            // hold a ready wave for QUIET_CEILING_FRAMES at most.
            let overruled = self.army.held_for_losses_since.is_some_and(|since| tick.frame - since > QUIET_CEILING_FRAMES);
            let quiet = (overruled || tick.frame - self.last_loss_at_home_frame >= QUIET_BEFORE_WAVE_FRAMES)
                && (self.army.last_retreat_frame == 0 || tick.frame - self.army.last_retreat_frame >= AFTER_RETREAT_FRAMES);
            // H-ARMY-WAVE-GATE: weigh the wave against what we know stands at the target.
            let mut defenders = self.known_enemy_force(target, DEFENDED_RADIUS, snapshot.enemies.as_slice());
            if let Some((army, _)) = &self.army.enemy_army_seen {
                // Their army will come to the fight wherever it is: count it, unless more than it already stands there.
                if self.odds(army, &Force::default()) > self.odds(&Force { turret_metal: 0.0, ..defenders.clone() }, &Force::default()) {
                    defenders.units = army.units.clone();
                }
            }
            let odds = self.odds(&Self::force_of(&wave), &defenders);
            let outweighs = !self.enabled("H-ARMY-WAVE-GATE") || odds >= WAVE_ADVANTAGE;
            let ordered_attack = stance == Some(Stance::Attack);
            if may_launch && wave.len() >= wave_size && !ordered_attack && (!quiet || !outweighs) && tick.frame % (30 * FRAMES_PER_SECOND) == 0 {
                eprintln!(
                    "[ai {}] f={} wave held: {} soldiers at odds {odds:.2} against what is known at ({:.0}, {:.0}){}",
                    self.ai(), tick.frame, wave.len(), target.x, target.z, if quiet { "" } else { "; losses at home in the last 30 s" }
                );
            }
            let ready = may_launch && wave.len() >= wave_size;
            self.army.held_for_losses_since = match (ready && outweighs && !quiet, self.army.held_for_losses_since) {
                (true, since) => since.or(Some(tick.frame)),
                (false, _) => None,
            };
            let home_group = wave;
            if may_launch && home_group.len() >= wave_size && (ordered_attack || (quiet && outweighs)) {
                self.army.waves_sent += 1;
                self.fire(if wave_size == own_wave_size { "H-ARMY-WAVES" } else { "D-WAVE-LAUNCH" });
                let grid = self.world.grid(target);
                eprintln!(
                    "[ai {}] f={} wave {}: {} units to ({:.0}, {:.0})",
                    self.ai(), tick.frame, self.army.waves_sent, home_group.len(), target.x, target.z
                );
                self.event(tick.frame, format!("wave {} launched: {} units towards {grid}", self.army.waves_sent, home_group.len()));
                // H-ARMY-REINFORCE: a few more soldiers joining a body already out there go to it, and the body carries on.
                // Every wave used to call everyone committed back to a fresh staging point; under an `attack` stance
                // waves of three leave every half minute, and 100 attackers stood at the staging point for three
                // minutes while the commander wondered why (commander game 9, south-east, 32400-37800).
                let body: Vec<&OwnUnit> = soldiers.iter().filter(|u| self.army.attackers.contains(&u.id)).copied().collect();
                if self.enabled("H-ARMY-REINFORCE") && body.len() >= 2 * home_group.len() {
                    self.fire("H-ARMY-REINFORCE");
                    let n = body.len() as f32;
                    let centre = body.iter().fold(Vec3::default(), |sum, u| Vec3 { x: sum.x + u.pos.x / n, y: 0.0, z: sum.z + u.pos.z / n });
                    self.army.attackers.extend(home_group.iter().map(|u| u.id));
                    commands.extend(home_group.iter().map(|u| Command::Fight { unit: u.id, to: centre, queue: false }));
                    commands.extend(home_group.iter().map(|u| Command::Fight { unit: u.id, to: target, queue: true }));
                    return;
                }
                self.army.attackers.extend(home_group.iter().map(|u| u.id));
                // H-ARMY-STAGE: sent straight at the target, a wave arrives fastest-first and dies one by one.
                // Everyone committed, survivors of earlier waves included, gathers short of the target first.
                let approach = self.walk_from_home(target);
                let first_stop = if self.enabled("H-ARMY-STAGE") && approach > 2.0 * STAGE_DISTANCE {
                    self.fire("H-ARMY-STAGE");
                    let t = STAGE_DISTANCE / target.dist2d(self.home).max(1.0);
                    let straight = Vec3 { x: target.x + (self.home.x - target.x) * t, y: 0.0, z: target.z + (self.home.z - target.z) * t };
                    let point = self.on_the_way_to(target, approach - STAGE_DISTANCE).unwrap_or(straight);
                    self.army.staging = Some((point, tick.frame));
                    point
                } else {
                    target
                };
                self.journal_wave(tick.frame, self.army.waves_sent, home_group.len(), target, first_stop);
                let committed = soldiers.iter().filter(|u| self.army.attackers.contains(&u.id));
                commands.extend(committed.map(|u| Command::Fight { unit: u.id, to: first_stop, queue: false }));
            } else {
                // H-ARMY-STATION: wait where raids arrive, not scattered around the labs.
                let stragglers = home_group.iter().filter(|u| u.idle && u.pos.dist2d(rally) > RALLY_RADIUS);
                commands.extend(stragglers.map(|u| Command::Move { unit: u.id, to: rally, queue: false }));
            }
        }

        if tick.frame % (60 * FRAMES_PER_SECOND) == 0 && !attackers.is_empty() {
            let n = attackers.len() as f32;
            let (cx, cz) = attackers.iter().fold((0.0, 0.0), |(x, z), u| (x + u.pos.x / n, z + u.pos.z / n));
            let idle = attackers.iter().filter(|u| u.idle).count();
            eprintln!(
                "[ai {}] f={} attackers {} ({} idle) around ({cx:.0}, {cz:.0}), target ({:.0}, {:.0}), home group {}",
                self.ai(), tick.frame, attackers.len(), idle, target.x, target.z, home_group.len()
            );
        }

        // H-ARMY-RETREAT: attackers facing a fight they are predicted to lose break off before they are spent. Judged
        // from the leading group: the attackers within contact range of the nearest enemy in sight, against every
        // enemy soldier and remembered turret within the same range of it.
        if self.enabled("H-ARMY-RETREAT") && !attackers.is_empty() && tick.frame % RETREAT_CHECK_FRAMES == 0 && stance != Some(Stance::Attack) {
            let contact = snapshot
                .enemies
                .iter()
                .filter(|e| attackers.iter().any(|u| u.pos.dist2d(e.pos) < CONTACT_RADIUS))
                .min_by(|a, b| a.pos.dist2d(self.home).total_cmp(&b.pos.dist2d(self.home)));
            if let Some(contact) = contact {
                let engaged: Vec<&OwnUnit> = attackers.iter().filter(|u| u.pos.dist2d(contact.pos) < CONTACT_RADIUS).copied().collect();
                let theirs = self.known_enemy_force(contact.pos, CONTACT_RADIUS, snapshot.enemies.as_slice());
                let odds = self.odds(&Self::force_of(&engaged), &theirs);
                // The few in contact are judged as what they are, a few; the wave is judged as a whole. A wave of 271
                // used to be sent home a second after it left because six of its fastest had met something (v26,
                // south-east timeouts at four times the opponent's army): if the whole wave wins the fight, the ones
                // in contact fall back on it and the wave keeps coming.
                let whole = self.odds(&Self::force_of(&attackers), &theirs);
                if odds < RETREAT_ODDS && whole >= 1.0 && self.enabled("H-ARMY-REGROUP") {
                    if engaged.len() < attackers.len() {
                        self.fire("H-ARMY-REGROUP");
                        let n = attackers.len() as f32;
                        let body = attackers.iter().fold(Vec3::default(), |sum, u| Vec3 { x: sum.x + u.pos.x / n, y: 0.0, z: sum.z + u.pos.z / n });
                        commands.extend(engaged.iter().map(|u| Command::Move { unit: u.id, to: body, queue: false }));
                    }
                } else if odds < RETREAT_ODDS {
                    self.fire("H-ARMY-RETREAT");
                    eprintln!(
                        "[ai {}] f={} retreat: {} attackers at odds {odds:.2} near ({:.0}, {:.0}); everyone home",
                        self.ai(), tick.frame, engaged.len(), contact.pos.x, contact.pos.z
                    );
                    self.event(tick.frame, format!("attack broken off at {}: odds {odds:.2}", self.world.grid(contact.pos)));
                    commands.extend(attackers.iter().map(|u| Command::Move { unit: u.id, to: rally, queue: false }));
                    self.army.attackers.clear();
                    self.army.staging = None;
                    self.army.last_retreat_frame = tick.frame;
                    return;
                }
            }
        }

        // `attackers` was drawn up before this tick's launch, so a wave launched just now is judged from the next tick.
        let staging = self.army.staging.filter(|(_, since)| *since < tick.frame);
        let mut held = std::mem::take(&mut self.army.held);
        let destination = staging.map(|(point, _)| point).unwrap_or(target);
        commands.extend(self.march(&mut held, &attackers, destination, snapshot.enemies.as_slice()));
        let attackers: Vec<&OwnUnit> = attackers.into_iter().filter(|u| !held.contains(&u.id)).collect();
        self.army.held = held;
        if staging.is_some() && attackers.is_empty() {
            self.army.staging = None;
        } else if let Some((point, since)) = staging {
            let gathered = attackers.iter().filter(|u| u.pos.dist2d(point) < STAGE_RADIUS).count();
            let quorum = gathered as f32 >= attackers.len() as f32 * STAGE_QUORUM;
            if !quorum && tick.frame - since < STAGE_PATIENCE_FRAMES {
                // Still gathering: whoever lost their order on the way is sent on to the staging point.
                let strays = attackers.iter().filter(|u| u.idle && u.pos.dist2d(point) > STAGE_RADIUS);
                commands.extend(strays.map(|u| Command::Fight { unit: u.id, to: point, queue: false }));
                return;
            }
            eprintln!("[ai {}] f={} assault: {gathered} of {} attackers gathered, going in", self.ai(), tick.frame, attackers.len());
            self.journal.note(tick.frame, "assault", serde_json::json!({ "gathered": gathered, "attackers": attackers.len() }), serde_json::Value::Null);
            self.army.staging = None;
            commands.extend(attackers.iter().map(|u| Command::Fight { unit: u.id, to: target, queue: false }));
            return;
        }

        // Attackers that ran out of orders keep the pressure on instead of standing around.
        let idle_attackers: Vec<&&OwnUnit> = attackers.iter().filter(|u| u.idle).collect();
        if idle_attackers.is_empty() {
            return;
        }
        let mut next = target;
        let nothing_here = snapshot.enemies.is_empty() && idle_attackers.iter().any(|u| u.pos.dist2d(target) < ARRIVED_RADIUS);
        if nothing_here && self.directives.attack_target.is_none() && self.enemy_buildings.is_empty() {
            // Sweep metal spots, starting from the enemy's side of the map.
            let mut spots = self.world.hello.metal_spots.clone();
            spots.sort_by(|a, b| a.dist2d(self.enemy_base(*a)).total_cmp(&b.dist2d(self.enemy_base(*b))));
            if !spots.is_empty() {
                next = Vec3 { y: 0.0, ..spots[self.army.sweep_index % spots.len()] };
                self.army.sweep_index += 1;
                self.fire("H-ARMY-SWEEP");
                self.army.target = Some(next);
            }
        }
        self.fire("H-ARMY-TARGET");
        commands.extend(idle_attackers.iter().map(|u| Command::Fight { unit: u.id, to: next, queue: false }));
    }
}
