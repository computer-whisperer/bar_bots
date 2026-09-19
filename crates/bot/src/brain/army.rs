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
const BASE_RADIUS: f32 = 1400.0;
/// Enemies this close to one of our extractors are raiding it.
const RAID_RADIUS: f32 = 500.0;
/// The strategist is woken for a base attack of at least this many; lone raiders are routine.
const NOTABLE_INTRUSION: usize = 3;
/// The default station stands this far ahead of the most exposed extractor, towards the enemy.
const STATION_LEAD: f32 = 250.0;
/// Extractors nearer to home than this are covered by the base itself.
const OUTPOST_DISTANCE: f32 = 1200.0;
/// An idle attacker this close to the attack target has arrived and needs a new one.
const ARRIVED_RADIUS: f32 = 400.0;
/// Home-group units farther than this from the rally point are called in.
const RALLY_RADIUS: f32 = 600.0;
const DEFEND_REORDER_FRAMES: i32 = 5 * FRAMES_PER_SECOND;

#[derive(Default)]
pub struct Army {
    attackers: HashSet<UnitId>,
    waves_sent: usize,
    /// Where attacks go: the most recently seen enemy nearest its presumed start.
    target: Option<Vec3>,
    /// Next metal spot to sweep when attackers find nothing at their target.
    sweep_index: usize,
    last_defend_order: i32,
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
        match most_exposed.filter(|_| self.enabled("H-ARMY-STATION")) {
            Some(extractor) => {
                let (dx, dz) = (self.enemy_start.x - extractor.pos.x, self.enemy_start.z - extractor.pos.z);
                let len = dx.hypot(dz).max(1.0);
                Vec3 { x: extractor.pos.x + dx / len * STATION_LEAD, y: 0.0, z: extractor.pos.z + dz / len * STATION_LEAD }
            }
            None => self.forward_of_home(500.0),
        }
    }

    pub(super) fn run_army(&mut self, tick: &Tick, kit: &Kit, commands: &mut Vec<Command>) {
        let snapshot = &tick.snapshot;
        let soldiers: Vec<&OwnUnit> =
            snapshot.own_units.iter().filter(|u| !u.being_built && self.is_army(u, kit)).collect();

        let committed = self.army.attackers.len();
        self.army.attackers.retain(|id| soldiers.iter().any(|u| u.id == *id));
        if committed >= NOTABLE_WAVE && self.army.attackers.is_empty() {
            self.trigger("wave-lost", tick.frame, format!("Our attack group of {committed} has been wiped out."));
        }

        let nearest_to = |point: Vec3| {
            snapshot.enemies.iter().min_by(|a, b| a.pos.dist2d(point).total_cmp(&b.pos.dist2d(point)))
        };
        if let Some(enemy) = nearest_to(self.enemy_start) {
            self.army.target = Some(enemy.pos);
        }
        let mut target = self.army.target.unwrap_or(self.enemy_start);
        if let Some(ordered) = self.directives.attack_target {
            self.fire("D-ATTACK-TARGET");
            target = ordered.value;
        }
        let stance = self.directives.army_stance.map(|s| s.value);
        let rally = self.station(tick, kit);
        self.last_station = rally;

        if stance == Some(Stance::Defend) && !self.army.attackers.is_empty() {
            // Attackers come home and rejoin the home group.
            self.fire("D-STANCE-DEFEND");
            commands.extend(self.army.attackers.iter().map(|id| Command::Move { unit: *id, to: rally, queue: false }));
            self.army.attackers.clear();
        }
        let (attackers, home_group): (Vec<&OwnUnit>, Vec<&OwnUnit>) =
            soldiers.iter().partition(|u| self.army.attackers.contains(&u.id));

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
                commands.extend(home_group.iter().map(|u| Command::Fight { unit: u.id, to: target, queue: false }));
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

        // Attackers that ran out of orders keep the pressure on instead of standing around.
        let idle_attackers: Vec<&&OwnUnit> = attackers.iter().filter(|u| u.idle).collect();
        if idle_attackers.is_empty() {
            return;
        }
        let mut next = target;
        let nothing_here = snapshot.enemies.is_empty() && idle_attackers.iter().any(|u| u.pos.dist2d(target) < ARRIVED_RADIUS);
        if nothing_here && self.directives.attack_target.is_none() {
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
