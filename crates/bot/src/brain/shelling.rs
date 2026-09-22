//! Fire from out of sight (H-HANDS-SHELLED): the engine's damage event carries the weapon and a direction toward the
//! attacker whether or not it is in sight, so a group shelled by something it cannot see still knows what is shelling
//! it, how far that weapon reaches and which way it stands. The likeliest source goes into the picture as a place the
//! hands can advance on, and the player is woken when a group stands under it (the user, watching pianist-player-6:
//! the army "is getting shelled from just out of frame but has no way to push out to try and kill it").

use std::collections::BTreeMap;

use bot_protocol::{Event, Tick, UnitId, Vec3};

use super::pianist::GroupTask;
use super::{Brain, FRAMES_PER_SECOND};

/// Hits older than this are forgotten.
const SHELL_MEMORY: i32 = 20 * FRAMES_PER_SECOND;
/// The player is woken when this many hits have come from out of sight over this long, once a minute.
const WAKE_HITS: usize = 5;
const WAKE_SECONDS: i32 = 15;
const WAKE_COOLDOWN: i32 = 60 * FRAMES_PER_SECOND;
/// With one ray, the source is presumed this far along it, as a share of the weapon's range.
const ALONG_THE_RAY: f32 = 0.8;

/// One hit from out of sight: where it landed, which way the shooter was, and what fired.
#[derive(Clone, Debug)]
pub(super) struct Shell {
    pub at: Vec3,
    pub dir: Vec3,
    pub range: f32,
    pub weapon: String,
    pub unit: UnitId,
    pub frame: i32,
}

/// What the recent hits say together.
pub(super) struct Shelling {
    /// The likeliest place of the shooter.
    pub at: Vec3,
    /// The mean direction of the hits, from us toward it.
    pub dir: Vec3,
    pub weapon: String,
    pub range: f32,
    pub hits: usize,
    pub since: i32,
    pub units: Vec<UnitId>,
}

/// Eight compass words for a direction; north is toward smaller z, as the grid's rows run.
pub(super) fn compass(dir: Vec3) -> &'static str {
    let angle = dir.x.atan2(-dir.z).to_degrees().rem_euclid(360.0);
    const NAMES: [&str; 8] = ["north", "north-east", "east", "south-east", "south", "south-west", "west", "north-west"];
    NAMES[(((angle + 22.5) / 45.0) as usize) % 8]
}

impl Brain {
    /// Remembers this tick's hits from out of sight and wakes the player when a group has stood under them.
    pub(super) fn track_shelling(&mut self, tick: &Tick) {
        let frame = tick.frame;
        self.shelling.retain(|s| frame - s.frame <= SHELL_MEMORY);
        for event in &tick.events {
            let Event::UnitDamaged { unit, attacker: None, from: Some(dir), weapon: Some(weapon), .. } = event else { continue };
            if weapon.range <= 0.0 {
                continue;
            }
            let Some(hit) = tick.snapshot.own_units.iter().find(|u| u.id == *unit) else { continue };
            let len = dir.x.hypot(dir.z);
            if len < 0.1 {
                continue;
            }
            self.shelling.push(Shell { at: hit.pos, dir: Vec3 { x: dir.x / len, y: 0.0, z: dir.z / len }, range: weapon.range, weapon: weapon.name.clone(), unit: *unit, frame });
        }
        let Some(shared) = &self.strategist else { return };
        let Some(s) = self.shelling() else { return };
        if s.hits < WAKE_HITS || frame - s.since < WAKE_SECONDS * FRAMES_PER_SECOND || frame - self.shelling_warned < WAKE_COOLDOWN {
            return;
        }
        // A group already advancing on it needs no waking.
        let advancing = self.pianist.as_ref().is_some_and(|p| p.groups.iter().any(|g| matches!(g.task, GroupTask::Move { fight: true, .. }) && g.members.iter().any(|m| s.units.contains(m))));
        if advancing {
            return;
        }
        self.shelling_warned = frame;
        shared.trigger(format!(
            "our soldiers are being shelled from out of sight by a {} (range {:.0}) from the {}, {} hits in {} s: the picture names the place `shelling` where it likeliest stands",
            s.weapon, s.range, compass(s.dir), s.hits, (frame - s.since) / FRAMES_PER_SECOND
        ));
    }

    /// The recent hits from out of sight, read together: the weapon that hit most, and its likeliest place: the point
    /// nearest every hit's ray when two or more rays cross, else four fifths of the range along the one direction.
    pub(super) fn shelling(&self) -> Option<Shelling> {
        if self.shelling.is_empty() {
            return None;
        }
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for s in &self.shelling {
            *counts.entry(s.weapon.as_str()).or_default() += 1;
        }
        let weapon = counts.iter().max_by_key(|(_, n)| **n).map(|(w, _)| w.to_string())?;
        let shells: Vec<&Shell> = self.shelling.iter().filter(|s| s.weapon == weapon).collect();
        let n = shells.len() as f32;
        let range = shells.iter().map(|s| s.range).fold(0.0, f32::max);
        let origin = shells.iter().fold(Vec3::default(), |sum, s| Vec3 { x: sum.x + s.at.x / n, y: 0.0, z: sum.z + s.at.z / n });
        let sum_dir = shells.iter().fold((0.0f32, 0.0f32), |sum, s| (sum.0 + s.dir.x, sum.1 + s.dir.z));
        let len = sum_dir.0.hypot(sum_dir.1).max(1e-3);
        let dir = Vec3 { x: sum_dir.0 / len, y: 0.0, z: sum_dir.1 / len };
        let along = Vec3 { x: origin.x + dir.x * range * ALONG_THE_RAY, y: 0.0, z: origin.z + dir.z * range * ALONG_THE_RAY };
        // Least squares over the rays: sum over hits of (I - d d^T) p = sum of (I - d d^T) a.
        let (mut a11, mut a12, mut a22, mut b1, mut b2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for s in &shells {
            let (p11, p12, p22) = (1.0 - s.dir.x * s.dir.x, -s.dir.x * s.dir.z, 1.0 - s.dir.z * s.dir.z);
            a11 += p11;
            a12 += p12;
            a22 += p22;
            b1 += p11 * s.at.x + p12 * s.at.z;
            b2 += p12 * s.at.x + p22 * s.at.z;
        }
        let det = a11 * a22 - a12 * a12;
        let at = if shells.len() >= 2 && det.abs() > 1e-2 {
            let p = Vec3 { x: (b1 * a22 - b2 * a12) / det, y: 0.0, z: (a11 * b2 - a12 * b1) / det };
            let ahead = (p.x - origin.x) * dir.x + (p.z - origin.z) * dir.z;
            if ahead > 0.0 && p.dist2d(origin) <= range * 1.1 { p } else { along }
        } else {
            along
        };
        let mut units: Vec<UnitId> = shells.iter().map(|s| s.unit).collect();
        units.sort();
        units.dedup();
        Some(Shelling { at, dir, weapon, range, hits: shells.len(), since: shells.iter().map(|s| s.frame).min().unwrap_or(0), units })
    }
}
