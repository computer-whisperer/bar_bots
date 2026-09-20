//! H-ARMY-MARCH: a group on its way somewhere arrives together.
//!
//! Sent with one order, a group strings out by speed and meets the enemy a few at a time: commander game 8c's Centurions
//! reached the enemy base 350 elmos ahead of its Hammers and 3000 metal was gone in 17 seconds; the heuristic waves lose
//! their fastest the same way. The ones out in front stop until the body has come up. Nobody is held while an enemy is
//! near (a stopped unit under fire is a target), and a few stragglers hold nobody up.

use std::collections::HashSet;

use bot_protocol::{Command, EnemyUnit, OwnUnit, UnitId, Vec3};

/// Smaller groups are not worth slowing down.
const MIN_GROUP: usize = 4;
/// The body is measured at this share of the group, counted from the front: the rearmost few hold nobody up.
const BODY_SHARE: f32 = 0.6;
/// A unit this much nearer the destination than the body stops; it goes on when the body is within half of it.
const LEAD: f32 = 350.0;
/// With the body this close to the destination the march is over.
const ARRIVED: f32 = 500.0;
/// An enemy this close to anyone ends the waiting: everybody goes on to the fight.
const ENEMY_NEAR: f32 = 900.0;

impl super::Brain {
    /// Orders for a group walking to `to`: the leaders it stops and the held units it lets go (`held` is the group's own
    /// memory of who is waiting). The caller has given the group its order already and must leave held units alone.
    pub(super) fn march(&mut self, held: &mut HashSet<UnitId>, group: &[&OwnUnit], to: Vec3, enemies: &[EnemyUnit]) -> Vec<Command> {
        held.retain(|id| group.iter().any(|u| u.id == *id));
        let go = |unit: UnitId| Command::Fight { unit, to, queue: false };
        let mut distances: Vec<f32> = group.iter().map(|u| u.pos.dist2d(to)).collect();
        distances.sort_by(f32::total_cmp);
        let body = distances.get(((group.len() as f32 * BODY_SHARE) as usize).min(group.len().saturating_sub(1))).copied().unwrap_or(0.0);
        let contact = enemies.iter().any(|e| group.iter().any(|u| u.pos.dist2d(e.pos) < ENEMY_NEAR));
        if !self.enabled("H-ARMY-MARCH") || group.len() < MIN_GROUP || contact || body < ARRIVED {
            return held.drain().map(go).collect();
        }
        let mut commands = Vec::new();
        for unit in group {
            let lead = body - unit.pos.dist2d(to);
            if lead > LEAD && held.insert(unit.id) {
                self.fire("H-ARMY-MARCH");
                commands.push(Command::Stop { unit: unit.id });
            } else if lead < LEAD / 2.0 && held.remove(&unit.id) {
                commands.push(go(unit.id));
            }
        }
        commands
    }
}
