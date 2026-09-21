//! Wrecks: metal lying on the ground, and units that can be raised again.
//!
//! H-REC-FIELDS: wrecks in sight are remembered and grouped into fields; a field is safe when it lies on ground we hold
//! (`territory.rs`) with no enemy soldier in sight near it. H-REC-CREW: resurrection bots are built in proportion to the
//! metal lying in safe fields and work the richest field for the walk. H-REC-RESURRECT: a wreck of a soldier worth
//! having is raised when energy is plentiful; everything else is taken apart. Constructors short of metal go to the
//! same fields (H-ECO-RECLAIM) and, while we have a resurrection bot, leave it the wrecks worth raising.

use std::collections::HashMap;

use bot_protocol::{Command, FeatureId, OwnUnit, Tick, UnitId, Vec3, Wreck};

use super::roster::Kit;
use super::territory::Ground;
use super::{Brain, FRAMES_PER_SECOND};

/// Wrecks this close together are one field.
const FIELD_RADIUS: f32 = 400.0;
/// A field poorer than this is not worth a walk.
const FIELD_MIN_METAL: f32 = 100.0;
/// An unseen wreck is believed in for this long.
const MEMORY_FRAMES: i32 = 2 * 60 * FRAMES_PER_SECOND;
/// A wreck we stand this close to and are not told of is gone.
const IN_PLAIN_SIGHT: f32 = 250.0;
const ENEMY_NEAR: f32 = 700.0;
/// H-REC-CREW: one resurrection bot per this much metal in safe fields, up to this many.
const METAL_PER_BOT: f32 = 600.0;
const MAX_CREW: usize = 6;
/// A field takes one worker per this much metal (at least one).
const METAL_PER_WORKER: f32 = 300.0;
/// Wrecks queued in one order.
const QUEUE: usize = 5;
/// H-REC-RESURRECT: soldiers worth at least this much metal, with stored energy above this share of storage. Raising
/// costs energy and no metal; a test on banked metal stood here and failed whenever the metal was being spent well
/// (the commander games, 2026-09-20: 869 wrecks taken apart for 89 raised).
const RAISE_MIN_METAL: f32 = 100.0;
const RAISE_ENERGY: f32 = 0.5;

pub struct WreckField {
    pub at: Vec3,
    pub metal: f32,
    pub safe: bool,
    wrecks: Vec<FeatureId>,
}

#[derive(Default)]
pub struct Reclaim {
    wrecks: HashMap<FeatureId, (Wreck, i32)>,
    pub fields: Vec<WreckField>,
    /// Who is working which field (by its place), and since when.
    workers: HashMap<UnitId, (Vec3, i32)>,
    /// Units just raised and the bot that raised them: a raised unit arrives with a twentieth of its health (rec-2:
    /// 48-102 of 755-1890), and the bot that raised it mends it before anything else.
    pub(super) to_mend: Vec<(UnitId, UnitId)>,
    /// Resurrection bots we have.
    crew: usize,
}

impl Brain {
    pub(super) fn track_wrecks(&mut self, tick: &Tick) {
        // What a resurrection gives us, for the log: the unit and the health it arrives with.
        for event in &tick.events {
            let bot_protocol::Event::UnitCreated { unit, builder: Some(builder) } = event else { continue };
            let raised_by_crew = self.known_units.get(builder).is_some_and(|(def, _)| self.kit.is_some_and(|kit| kit.is_resurrector(*def)));
            if let Some(raised) = tick.snapshot.own_units.iter().find(|u| u.id == *unit).filter(|_| raised_by_crew) {
                eprintln!("[ai {}] f={} raised {} at {:.0} of {:.0} health", self.ai(), tick.frame, self.name(raised.def), raised.health, raised.max_health);
                self.reclaim.to_mend.push((*builder, *unit));
            }
        }
        self.reclaim.crew = tick.snapshot.own_units.iter().filter(|u| !u.being_built && self.kit.is_some_and(|kit| kit.is_resurrector(u.def))).count();
        let Some(seen) = &tick.snapshot.wrecks else { return };
        let listed: std::collections::HashSet<FeatureId> = seen.iter().map(|w| w.id).collect();
        for wreck in seen {
            self.reclaim.wrecks.insert(wreck.id, (wreck.clone(), tick.frame));
        }
        let own = &tick.snapshot.own_units;
        self.reclaim.wrecks.retain(|id, (wreck, last_seen)| {
            let looked_at = own.iter().any(|u| u.pos.dist2d(wreck.pos) < IN_PLAIN_SIGHT);
            listed.contains(id) || (!looked_at && tick.frame - *last_seen < MEMORY_FRAMES)
        });
        // Fields: the richest wreck not yet in one, with everything near it.
        let mut left: Vec<&Wreck> = self.reclaim.wrecks.values().map(|(w, _)| w).collect();
        left.sort_by(|a, b| b.metal.total_cmp(&a.metal).then(a.id.cmp(&b.id)));
        let mut fields = Vec::new();
        while let Some(seed) = left.first().copied() {
            let (inside, outside): (Vec<&Wreck>, Vec<&Wreck>) = left.into_iter().partition(|w| w.pos.dist2d(seed.pos) < FIELD_RADIUS);
            left = outside;
            let metal: f32 = inside.iter().map(|w| w.metal).sum();
            if metal < FIELD_MIN_METAL {
                continue;
            }
            let at = inside.iter().fold(Vec3::default(), |sum, w| Vec3 { x: sum.x + w.pos.x * w.metal / metal, y: 0.0, z: sum.z + w.pos.z * w.metal / metal });
            let enemy_near = tick.snapshot.enemies.iter().any(|e| e.pos.dist2d(at) < ENEMY_NEAR);
            let safe = self.ground(at) == Ground::Held && !enemy_near && self.reachable_on_foot(at);
            fields.push(WreckField { at, metal, safe, wrecks: inside.iter().map(|w| w.id).collect() });
        }
        self.reclaim.fields = fields;
        if tick.due() % (60 * FRAMES_PER_SECOND) < 3 * FRAMES_PER_SECOND {
            let safe: f32 = self.reclaim.fields.iter().filter(|f| f.safe).map(|f| f.metal).sum();
            let all: f32 = self.reclaim.fields.iter().map(|f| f.metal).sum();
            eprintln!(
                "[ai {}] f={} wrecks: {} known in {} fields, {all:.0} metal ({safe:.0} safe to work), {} at work",
                self.ai(), tick.frame, self.reclaim.wrecks.len(), self.reclaim.fields.len(), self.reclaim.workers.len()
            );
        }
        self.reclaim.workers.retain(|id, _| own.iter().any(|u| u.id == *id && !u.idle));
    }

    /// H-REC-CREW: how many resurrection bots the metal on the ground is worth.
    pub(super) fn wanted_crew(&self) -> usize {
        if !self.enabled("H-REC-CREW") {
            return 0;
        }
        let safe: f32 = self.reclaim.fields.iter().filter(|f| f.safe).map(|f| f.metal).sum();
        ((safe / METAL_PER_BOT).ceil() as usize).min(MAX_CREW)
    }

    /// The safe field most worth this unit's walk that still has room for a worker.
    fn field_for(&self, unit: &OwnUnit, within: f32) -> Option<usize> {
        let room = |field: &WreckField| {
            let working = self.reclaim.workers.values().filter(|(at, _)| at.dist2d(field.at) < FIELD_RADIUS).count();
            working < ((field.metal / METAL_PER_WORKER).ceil() as usize).max(1)
        };
        (0..self.reclaim.fields.len())
            .filter(|i| self.reclaim.fields[*i].safe && room(&self.reclaim.fields[*i]) && self.reclaim.fields[*i].at.dist2d(unit.pos) < within)
            .max_by(|a, b| {
                let worth = |i: &usize| self.reclaim.fields[*i].metal / (self.reclaim.fields[*i].at.dist2d(unit.pos) + 500.0);
                worth(a).total_cmp(&worth(b))
            })
    }

    /// A soldier's wreck worth having back.
    fn worth_raising(&self, wreck: &Wreck) -> bool {
        wreck.resurrects_into.and_then(|def| self.world.def(def)).is_some_and(|d| d.weapon_count > 0 && d.speed > 0.0 && d.build_speed == 0.0 && d.metal_cost >= RAISE_MIN_METAL)
    }

    /// The wrecks of the field at `at` a constructor takes apart, the nearest first: not the ones worth raising while
    /// there is a resurrection bot to raise them. An area order took everything, and 71-91 % of those orders were given
    /// over a field where a soldier worth raising had just died (the commander games, 2026-09-20).
    pub(super) fn wrecks_to_take(&self, at: Vec3, builder: &OwnUnit) -> Vec<FeatureId> {
        let Some(field) = self.reclaim.fields.iter().find(|f| f.at.dist2d(at) < 1.0) else { return Vec::new() };
        let mut wrecks: Vec<&Wreck> = field.wrecks.iter().filter_map(|id| self.reclaim.wrecks.get(id)).map(|(w, _)| w).collect();
        wrecks.retain(|w| self.reclaim.crew == 0 || !self.worth_raising(w));
        wrecks.sort_by(|a, b| a.pos.dist2d(builder.pos).total_cmp(&b.pos.dist2d(builder.pos)));
        wrecks.into_iter().take(QUEUE).map(|w| w.id).collect()
    }

    /// H-ECO-RECLAIM: where a constructor short of metal goes to take wrecks apart.
    pub(super) fn claim_wreck_field(&mut self, builder: &OwnUnit, within: f32, frame: i32) -> Option<Vec3> {
        let at = self.reclaim.fields[self.field_for(builder, within)?].at;
        if self.wrecks_to_take(at, builder).is_empty() {
            return None;
        }
        self.reclaim.workers.insert(builder.id, (at, frame));
        Some(at)
    }

    /// Orders for an idle resurrection bot.
    pub(super) fn work_wrecks(&mut self, unit: &OwnUnit, tick: &Tick, commands: &mut Vec<Command>) {
        let Some(index) = self.field_for(unit, f32::INFINITY) else {
            // Nothing to do: wait with the army, where the next wrecks will be.
            if unit.pos.dist2d(self.last_station) > 600.0 {
                commands.push(Command::Move { unit: unit.id, to: self.last_station, queue: false });
            }
            return;
        };
        let energy = &tick.snapshot.energy;
        let allowed = self.directives.resurrect.is_none_or(|d| d.value) && self.enabled("H-REC-RESURRECT");
        let can_raise = allowed && energy.current > RAISE_ENERGY * energy.storage;
        let field = &self.reclaim.fields[index];
        let mut wrecks: Vec<&Wreck> = field.wrecks.iter().filter_map(|id| self.reclaim.wrecks.get(id)).map(|(w, _)| w).collect();
        wrecks.sort_by(|a, b| a.pos.dist2d(unit.pos).total_cmp(&b.pos.dist2d(unit.pos)));
        let mut raised = 0;
        for (n, wreck) in wrecks.iter().take(QUEUE).enumerate() {
            if can_raise && self.worth_raising(wreck) {
                raised += 1;
                commands.push(Command::Resurrect { unit: unit.id, feature: wreck.id, queue: n > 0 });
            } else {
                commands.push(Command::ReclaimFeature { unit: unit.id, feature: wreck.id, queue: n > 0 });
            }
        }
        let at = field.at;
        self.fire(if raised > 0 { "H-REC-RESURRECT" } else { "H-REC-CREW" });
        self.reclaim.workers.insert(unit.id, (at, tick.frame));
    }
}

impl Kit {
    /// A resurrection bot: it raises wrecks and takes them apart, and builds nothing.
    pub fn is_resurrector(&self, def: bot_protocol::UnitDefId) -> bool {
        def == self.resurrector
    }
}
