//! Combat units: defend the base, gather, attack in growing waves.
//!
//! Units are either in the home group or committed attackers. Membership, not position or
//! idleness, decides which: a crowd at the rally point is never all idle at once.

use std::collections::HashSet;

use bot_protocol::{Command, EnemyUnit, OwnUnit, Tick, UnitId, Vec3};

use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};
use crate::strategist::shared::Stance;

const FIRST_WAVE: usize = 8;
const WAVE_GROWTH: usize = 4;
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
const OUTPOST_DISTANCE: f32 = 1200.0;
/// An idle attacker this close to the attack target has arrived and needs a new one.
const ARRIVED_RADIUS: f32 = 400.0;
/// Home-group units farther than this from the rally point are called in.
const RALLY_RADIUS: f32 = 600.0;
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
    /// Where the attackers are gathering before the assault, and since which frame.
    staging: Option<(Vec3, i32)>,
}

impl Army {
    pub fn is_attacker(&self, unit: UnitId) -> bool {
        self.attackers.contains(&unit)
    }

    pub fn waves_sent(&self) -> usize {
        self.waves_sent
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
            .filter(|u| u.def == kit.extractor && u.pos.dist2d(self.home) > OUTPOST_DISTANCE)
            .min_by(|a, b| a.pos.dist2d(self.enemy_start).total_cmp(&b.pos.dist2d(self.enemy_start)));
        // Candidates in order of preference. The outpost station sits on the home side of the extractor, on ground
        // our constructor walked to build it; a point ahead of it towards the enemy was often unreachable, and
        // units that cannot reach their station pile up at the factory exit.
        let outpost = most_exposed.filter(|_| self.enabled("H-ARMY-STATION")).map(|extractor| {
            let (dx, dz) = (self.home.x - extractor.pos.x, self.home.z - extractor.pos.z);
            let len = dx.hypot(dz).max(1.0);
            Vec3 { x: extractor.pos.x + dx / len * STATION_LEAD, y: 0.0, z: extractor.pos.z + dz / len * STATION_LEAD }
        });
        let candidates = [outpost, Some(self.forward_of_home(500.0)), Some(self.forward_of_home(150.0))];
        let usable = |point: &Vec3| {
            !self.army.bad_stations.iter().any(|(bad, until)| *until > tick.frame && bad.dist2d(*point) < BAD_STATION_RADIUS)
        };
        // Never the start point itself: it stands in the middle of the generator field.
        candidates.into_iter().flatten().find(usable).unwrap_or(self.forward_of_home(150.0))
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
                soldier && !self.army.attackers.contains(unit)
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
        let soldiers: Vec<&OwnUnit> =
            snapshot.own_units.iter().filter(|u| !u.being_built && self.is_army(u, kit) && !self.raids.is_raider(u.id)).collect();

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
        let reachable = |point: &Vec3| !self.army.bad_targets.iter().any(|(bad, _)| bad.dist2d(*point) < BAD_TARGET_RADIUS);
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
        // Seen buildings say where the enemy really is; the mirrored start is only a first guess.
        if self.enemy_buildings.len() >= 3 {
            let n = self.enemy_buildings.len() as f32;
            self.enemy_start = self.enemy_buildings.values().fold(Vec3::default(), |sum, (_, pos, _)| Vec3 {
                x: sum.x + pos.x / n,
                y: 0.0,
                z: sum.z + pos.z / n,
            });
        }
        let mut target = nearest_building.or(self.army.target).unwrap_or(self.enemy_start);
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
            self.army.attackers.clear();
            self.army.staging = None;
            self.army.last_defend_order = 0;
        }
        let (attackers, home_group): (Vec<&OwnUnit>, Vec<&OwnUnit>) =
            soldiers.iter().partition(|u| self.army.attackers.contains(&u.id));

        let pool: Vec<&OwnUnit> = home_group.clone();
        self.run_raids(tick, kit, &pool, commands);
        let home_group: Vec<&OwnUnit> = home_group.into_iter().filter(|u| !self.raids.is_raider(u.id)).collect();

        // Defence first: the home group turns on intruders at the base, then on raiders at any
        // extractor, and no wave leaves meanwhile.
        let at_base = nearest_to(self.home).filter(|e: &&EnemyUnit| e.pos.dist2d(self.home) < BASE_RADIUS);
        let raider = || {
            let extractors = snapshot.own_units.iter().filter(|u| u.def == kit.extractor);
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
                commands.extend(home_group.iter().map(|u| Command::Fight { unit: u.id, to: intruder.pos, queue: false }));
            }
        } else {
            let own_wave_size = (FIRST_WAVE + WAVE_GROWTH * self.army.waves_sent).min(MAX_WAVE);
            let wave_size = match (stance, self.directives.wave_size) {
                (Some(Stance::Attack), _) => MIN_ORDERED_WAVE,
                (_, Some(ordered)) => ordered.value,
                _ => own_wave_size,
            };
            let may_launch = !matches!(stance, Some(Stance::Defend | Stance::Gather));
            if may_launch && home_group.len() >= wave_size {
                self.army.waves_sent += 1;
                self.fire(if wave_size == own_wave_size { "H-ARMY-WAVES" } else { "D-WAVE-LAUNCH" });
                let grid = self.world.grid(target);
                eprintln!(
                    "[ai {}] f={} wave {}: {} units to ({:.0}, {:.0})",
                    self.ai(), tick.frame, self.army.waves_sent, home_group.len(), target.x, target.z
                );
                self.event(tick.frame, format!("wave {} launched: {} units towards {grid}", self.army.waves_sent, home_group.len()));
                self.army.attackers.extend(home_group.iter().map(|u| u.id));
                // H-ARMY-STAGE: sent straight at the target, a wave arrives fastest-first and dies one by one.
                // Everyone committed, survivors of earlier waves included, gathers short of the target first.
                let approach = target.dist2d(self.home);
                let first_stop = if self.enabled("H-ARMY-STAGE") && approach > 2.0 * STAGE_DISTANCE {
                    self.fire("H-ARMY-STAGE");
                    let t = STAGE_DISTANCE / approach;
                    let point = Vec3 { x: target.x + (self.home.x - target.x) * t, y: 0.0, z: target.z + (self.home.z - target.z) * t };
                    self.army.staging = Some((point, tick.frame));
                    point
                } else {
                    target
                };
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

        // `attackers` was drawn up before this tick's launch, so a wave launched just now is judged from the next tick.
        let staging = self.army.staging.filter(|(_, since)| *since < tick.frame);
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
            spots.sort_by(|a, b| a.dist2d(self.enemy_start).total_cmp(&b.dist2d(self.enemy_start)));
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
