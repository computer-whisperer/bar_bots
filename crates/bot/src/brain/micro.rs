//! H-MICRO-SPREAD: a wave attacks as a loose block instead of as a blob.
//!
//! Every attack order the army gives names one point, so the whole wave walks at it in a column and fights
//! shoulder to shoulder. The tier-1 line units and skirmishers on both sides are area-damage weapons — Mace and
//! Thug 104 damage over 36 elmos, Rocketeer and Aggravator 157 over 48 — and a blob hands every one of their
//! shells two or three units. The engine duel tables measure the difference and it is the largest single lever in
//! them: Pawn against Mace is -0.45 at 56 elmos of spacing and +0.19 at 160
//! (K-units-duel-spacing-decides-area-damage).
//!
//! This is the same order, aimed at each unit's own place in a block around the destination rather than at the
//! destination itself, so the wave arrives loose without arriving late. It is a rewrite of the orders the rest of
//! the brain has already decided on: what to attack and when to go are not this rule's business.
//!
//! Measured in `docs/studies/micro-combat.md`.

use std::collections::HashMap;

use bot_protocol::{Command, Tick, UnitId, Vec3};

use super::Brain;

/// Elmos between neighbours. The blasts being dodged reach 36 and 48 elmos and the engine measures to the
/// collision volume's surface (radius 11-15 for a tier-1 bot), so 110 puts a neighbour clear of any of them with
/// room left for the engine's own jostling.
const GAP: f32 = 110.0;
/// Smaller groups are not worth shaping: the block would be no wider than the crowd already is.
const MIN_GROUP: usize = 6;
/// Orders aimed this close together are one order and share one block.
const SAME_PLACE: f32 = 300.0;
/// The block is this many times wider across the approach than it is deep, so that it still arrives together
/// (H-ARMY-MARCH exists because a wave that arrives strung out dies in pieces).
const WIDTH_BIAS: f32 = 2.0;
/// However many there are, the block is never more than this many files wide: a hundred attackers in one rank
/// would be a mile of front, walking through whatever the ground holds.
const MAX_FILES: usize = 8;

impl Brain {
    /// Re-aims the attack orders of committed attackers at their own place in a loose block. Called on the whole
    /// tick's commands, so it catches every path the army takes to a `Fight` order without a hook in each.
    pub(super) fn spread_attack_orders(&mut self, tick: &Tick, commands: &mut [Command]) {
        if !self.enabled("H-MICRO-SPREAD") {
            return;
        }
        let at: HashMap<UnitId, Vec3> = tick.snapshot.own_units.iter().map(|u| (u.id, u.pos)).collect();
        // Which commands this rule may touch, and where each is aimed.
        let mut wanted: Vec<(usize, UnitId, Vec3)> = Vec::new();
        for (index, command) in commands.iter().enumerate() {
            let Command::Fight { unit, to, .. } = command else { continue };
            if self.army.is_attacker(*unit) && at.contains_key(unit) {
                wanted.push((index, *unit, *to));
            }
        }
        // One block per place being attacked: a tick can hold orders to a staging point and to the target at once.
        let mut groups: Vec<(Vec3, Vec<(usize, UnitId)>)> = Vec::new();
        for (index, unit, to) in wanted {
            match groups.iter_mut().find(|(place, _)| place.dist2d(to) < SAME_PLACE) {
                Some((_, members)) => members.push((index, unit)),
                None => groups.push((to, vec![(index, unit)])),
            }
        }
        // The march to the staging point is left alone. Spread is worth something where the shooting is, and
        // H-ARMY-STAGE counts an attacker as gathered by its distance to the staging point itself: a wave told to
        // stand in a block around it would never reach its quorum and would wait out the whole 150 s of patience.
        let staging = self.army.staging_point();
        for (place, members) in groups {
            if members.len() < MIN_GROUP || staging.is_some_and(|point| point.dist2d(place) < SAME_PLACE) {
                continue;
            }
            self.fire("H-MICRO-SPREAD");
            let positions: Vec<(UnitId, Vec3)> = members.iter().map(|(_, unit)| (*unit, at[unit])).collect();
            let from = centre(&positions);
            for ((index, unit), point) in members.iter().zip(loose_block(&positions, from, place, GAP)) {
                let point = if self.reachable_on_foot(point) { point } else { self.snap_to_reachable(point) };
                debug_assert!(matches!(commands[*index], Command::Fight { unit: u, .. } if u == *unit));
                if let Command::Fight { to, .. } = &mut commands[*index] {
                    *to = point;
                }
            }
        }
    }
}

fn centre(units: &[(UnitId, Vec3)]) -> Vec3 {
    let n = units.len().max(1) as f32;
    units.iter().fold(Vec3::default(), |sum, (_, p)| Vec3 { x: sum.x + p.x / n, y: 0.0, z: sum.z + p.z / n })
}

/// Where each unit is sent so that the group stands in a block around `to` instead of on it: files across the
/// approach from `from`, ranks behind. Units keep their left-to-right order, so nobody crosses anybody's path.
/// Returned in the order `units` was given in.
fn loose_block(units: &[(UnitId, Vec3)], from: Vec3, to: Vec3, gap: f32) -> Vec<Vec3> {
    let n = units.len();
    let (dx, dz) = (to.x - from.x, to.z - from.z);
    let len = dx.hypot(dz).max(1.0);
    let ahead = (dx / len, dz / len);
    let across = (-ahead.1, ahead.0);
    let files = ((n as f32 * WIDTH_BIAS).sqrt().ceil() as usize).clamp(1, n.min(MAX_FILES));
    let ranks = n.div_ceil(files);
    // Left to right: the leftmost `ranks` units fill the leftmost file, and so on.
    let mut order: Vec<usize> = (0..n).collect();
    let sideways = |i: &usize| units[*i].1.x * across.0 + units[*i].1.z * across.1;
    order.sort_by(|a, b| sideways(a).total_cmp(&sideways(b)));
    let mut points = vec![Vec3::default(); n];
    for (place, unit) in order.into_iter().enumerate() {
        let (file, rank) = (place / ranks, place % ranks);
        let side = (file as f32 - (files as f32 - 1.0) / 2.0) * gap;
        let back = (rank as f32 - (ranks as f32 - 1.0) / 2.0) * gap;
        points[unit] = Vec3 {
            x: to.x + across.0 * side - ahead.0 * back,
            y: to.y,
            z: to.z + across.1 * side - ahead.1 * back,
        };
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, z: f32) -> Vec3 {
        Vec3 { x, y: 0.0, z }
    }

    #[test]
    fn a_block_is_wider_than_it_is_deep_and_holds_its_order() {
        // Twelve units in a heap at the origin, attacking eastwards.
        let units: Vec<(UnitId, Vec3)> = (0..12).map(|i| (UnitId(i), at(i as f32 * 5.0, i as f32 * 3.0))).collect();
        let points = loose_block(&units, at(0.0, 0.0), at(2000.0, 0.0), 100.0);
        let width = |pick: fn(&Vec3) -> f32| {
            let values: Vec<f32> = points.iter().map(pick).collect();
            values.iter().copied().fold(f32::MIN, f32::max) - values.iter().copied().fold(f32::MAX, f32::min)
        };
        // Approach along x, so the block is spread across z and shallow along x.
        assert!(width(|p| p.z) > width(|p| p.x), "{:?}", points);
        assert!(width(|p| p.z) >= 300.0, "twelve units should stand at least four files wide: {:?}", points);
        // Nobody is told to stand on anybody else.
        for (i, a) in points.iter().enumerate() {
            for b in points.iter().skip(i + 1) {
                assert!(a.dist2d(*b) > 50.0, "{a:?} and {b:?} are the same place");
            }
        }
        // The one that started furthest to the south (-z is across when going east) keeps that end of the line.
        let southmost = points.iter().enumerate().min_by(|a, b| a.1.z.total_cmp(&b.1.z)).unwrap().0;
        assert_eq!(southmost, 0);
    }

    #[test]
    fn the_block_is_centred_on_where_the_order_pointed() {
        let units: Vec<(UnitId, Vec3)> = (0..9).map(|i| (UnitId(i), at(i as f32 * 10.0, 0.0))).collect();
        let points = loose_block(&units, at(0.0, 0.0), at(1000.0, 1000.0), 100.0);
        let middle = centre(&points.iter().map(|p| (UnitId(0), *p)).collect::<Vec<_>>());
        assert!(middle.dist2d(at(1000.0, 1000.0)) < 60.0, "{middle:?}");
    }
}
