//! Combat units: defend the base, gather, attack in growing waves.
//!
//! Units are either in the home group or committed attackers. Membership, not position or
//! idleness, decides which: a crowd at the rally point is never all idle at once.

use std::collections::HashSet;

use bot_protocol::{Command, EnemyUnit, OwnUnit, Tick, UnitId, Vec3};

use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};

const FIRST_WAVE: usize = 8;
const WAVE_GROWTH: usize = 4;
const MAX_WAVE: usize = 40;
/// Enemies this close to home are an attack on the base.
const BASE_RADIUS: f32 = 1400.0;
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

impl Brain {
    pub(super) fn run_army(&mut self, tick: &Tick, kit: &Kit, commands: &mut Vec<Command>) {
        let snapshot = &tick.snapshot;
        let soldiers: Vec<&OwnUnit> =
            snapshot.own_units.iter().filter(|u| !u.being_built && self.is_army(u, kit)).collect();
        self.army.attackers.retain(|id| soldiers.iter().any(|u| u.id == *id));
        let nearest_to = |point: Vec3| {
            snapshot.enemies.iter().min_by(|a, b| a.pos.dist2d(point).total_cmp(&b.pos.dist2d(point)))
        };
        if let Some(enemy) = nearest_to(self.enemy_start) {
            self.army.target = Some(enemy.pos);
        }
        let target = self.army.target.unwrap_or(self.enemy_start);
        let (attackers, home_group): (Vec<&OwnUnit>, Vec<&OwnUnit>) =
            soldiers.iter().partition(|u| self.army.attackers.contains(&u.id));

        // Defence first: the home group turns on intruders and no wave leaves meanwhile.
        let intruder = nearest_to(self.home).filter(|e: &&EnemyUnit| e.pos.dist2d(self.home) < BASE_RADIUS);
        if let Some(intruder) = intruder {
            if tick.frame - self.army.last_defend_order >= DEFEND_REORDER_FRAMES {
                self.army.last_defend_order = tick.frame;
                commands.extend(home_group.iter().map(|u| Command::Fight { unit: u.id, to: intruder.pos, queue: false }));
            }
        } else {
            let wave_size = (FIRST_WAVE + WAVE_GROWTH * self.army.waves_sent).min(MAX_WAVE);
            if home_group.len() >= wave_size {
                self.army.waves_sent += 1;
                eprintln!(
                    "[ai {}] f={} wave {}: {} units to ({:.0}, {:.0})",
                    self.ai(), tick.frame, self.army.waves_sent, home_group.len(), target.x, target.z
                );
                self.army.attackers.extend(home_group.iter().map(|u| u.id));
                commands.extend(home_group.iter().map(|u| Command::Fight { unit: u.id, to: target, queue: false }));
            } else {
                // Wait where the base's turrets are, not scattered around the labs.
                let rally = self.forward_of_home(500.0);
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
        if snapshot.enemies.is_empty() && idle_attackers.iter().any(|u| u.pos.dist2d(target) < ARRIVED_RADIUS) {
            // Nothing here: sweep metal spots, starting from the enemy's side of the map.
            let mut spots = self.world.hello.metal_spots.clone();
            spots.sort_by(|a, b| a.dist2d(self.enemy_start).total_cmp(&b.dist2d(self.enemy_start)));
            if !spots.is_empty() {
                next = Vec3 { y: 0.0, ..spots[self.army.sweep_index % spots.len()] };
                self.army.sweep_index += 1;
                self.army.target = Some(next);
            }
        }
        commands.extend(idle_attackers.iter().map(|u| Command::Fight { unit: u.id, to: next, queue: false }));
    }
}
