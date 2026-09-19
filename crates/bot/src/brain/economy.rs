//! Builders and factories: what to build next, and where.

use std::collections::HashMap;

use bot_protocol::{BuildSite, Command, OwnUnit, Tick, UnitDefId, Vec3};

use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};

/// The commander never builds farther from home than this.
const COMMANDER_LEASH: f32 = 900.0;
const MAX_LABS: usize = 4;
const MAX_TURRETS: usize = 6;
const MAX_CONVERTERS: usize = 40;
/// An extractor beyond this distance from home gets a turret of its own.
const OUTPOST_DISTANCE: f32 = 1200.0;
const OUTPOST_GUARD_RADIUS: f32 = 350.0;
const MAX_CONSTRUCTORS: usize = 6;
/// Stored metal above which the base is under-spending and wants another lab.
const FLOATING_METAL: f32 = 500.0;
/// Energy income beyond which solar collectors are too small to keep up.
const ADVANCED_SOLAR_INCOME: f32 = 250.0;
/// How long a metal spot stays reserved for a builder that was sent to it.
const SPOT_CLAIM_FRAMES: i32 = 60 * FRAMES_PER_SECOND;
/// A metal extractor this close to a spot occupies it.
const SPOT_OCCUPIED_RADIUS: f32 = 60.0;

/// What one builder should do next.
enum Plan {
    Extractor(Vec3),
    /// A building placed near `anchor`.
    Near(UnitDefId, Vec3),
}

impl Brain {
    pub(super) fn run_economy(&mut self, tick: &Tick, kit: &Kit, commands: &mut Vec<Command>) {
        let own = &tick.snapshot.own_units;
        self.jobs.retain(|id, _| own.iter().any(|u| u.id == *id && !u.idle));
        self.spot_claims.retain(|_, claimed| tick.frame - *claimed < SPOT_CLAIM_FRAMES);

        for unit in own.iter().filter(|u| u.idle && !u.being_built) {
            let Some(def) = self.world.def(unit.def) else { continue };
            let (is_builder, is_mobile) = (def.build_speed > 0.0, def.speed > 0.0);
            if is_builder && is_mobile {
                let plan = self.plan_for(unit, tick, kit);
                let (def_id, site) = match plan {
                    Plan::Extractor(spot) => (kit.extractor, BuildSite { near: spot, search_radius: 100.0, min_dist: 0 }),
                    Plan::Near(def_id, anchor) => (def_id, BuildSite { near: anchor, search_radius: 1000.0, min_dist: 3 }),
                };
                self.jobs.insert(unit.id, def_id);
                commands.push(Command::Build { unit: unit.id, def: def_id, site: Some(site), queue: false });
            } else if is_builder {
                commands.extend(self.production_batch(own, kit).map(|def_id| Command::Build {
                    unit: unit.id,
                    def: def_id,
                    site: None,
                    queue: false,
                }));
            }
        }
    }

    /// The most urgent thing this builder can safely do.
    fn plan_for(&mut self, builder: &OwnUnit, tick: &Tick, kit: &Kit) -> Plan {
        let snapshot = &tick.snapshot;
        let is_commander = builder.def == kit.commander;
        // Existing (finished or not) plus what other builders are already on their way to build.
        let mut counts: HashMap<UnitDefId, usize> = HashMap::new();
        let others_jobs = self.jobs.iter().filter(|(id, _)| **id != builder.id).map(|(_, job)| *job);
        for def in snapshot.own_units.iter().map(|u| u.def).chain(others_jobs) {
            *counts.entry(def).or_default() += 1;
        }
        let planned = |def: UnitDefId| counts.get(&def).copied().unwrap_or(0);
        let energy = snapshot.energy;
        let energy_short = energy.current < energy.storage * 0.3 || energy.income < energy.usage + 20.0;
        let energy_rich = energy.current > energy.storage * 0.8 && energy.income > energy.usage + 60.0;
        let generator = if energy.income > ADVANCED_SOLAR_INCOME && !is_commander { kit.advanced_solar } else { kit.solar };
        let base = self.home;
        let front = self.forward_of_home(450.0);

        if planned(kit.extractor) < 2
            && let Some(spot) = self.claim_spot(builder, snapshot.own_units.as_slice(), kit, tick.frame)
        {
            return Plan::Extractor(spot);
        }
        if planned(kit.solar) < 2 {
            return Plan::Near(kit.solar, base);
        }
        if planned(kit.lab) < 1 {
            return Plan::Near(kit.lab, base);
        }
        if energy_short && planned(generator) < snapshot.own_units.iter().filter(|u| u.def == generator).count() + 2 {
            return Plan::Near(generator, base);
        }
        if planned(kit.turret) < 2 {
            return Plan::Near(kit.turret, front);
        }
        if snapshot.metal.current > FLOATING_METAL && planned(kit.lab) < MAX_LABS {
            return Plan::Near(kit.lab, base);
        }
        if energy_rich && planned(kit.converter) < MAX_CONVERTERS {
            return Plan::Near(kit.converter, base);
        }
        if !is_commander
            && !energy_short
            && let Some(outpost) = self.unguarded_outpost(builder, snapshot.own_units.as_slice(), kit)
        {
            return Plan::Near(kit.turret, outpost);
        }
        if let Some(spot) = self.claim_spot(builder, snapshot.own_units.as_slice(), kit, tick.frame) {
            return Plan::Extractor(spot);
        }
        if planned(kit.turret) < MAX_TURRETS && !energy_short {
            return Plan::Near(kit.turret, front);
        }
        Plan::Near(generator, base)
    }

    /// Reserves the nearest free metal spot this builder may go to: inside the leash for the
    /// commander, on our half of the map for constructors.
    fn claim_spot(&mut self, builder: &OwnUnit, own: &[OwnUnit], kit: &Kit, frame: i32) -> Option<Vec3> {
        let is_commander = builder.def == kit.commander;
        let (home, enemy_start) = (self.home, self.enemy_start);
        let reachable = |spot: Vec3| {
            if is_commander { spot.dist2d(home) < COMMANDER_LEASH } else { spot.dist2d(home) < spot.dist2d(enemy_start) }
        };
        let (index, spot) = self
            .world
            .hello
            .metal_spots
            .iter()
            .enumerate()
            .filter(|(i, s)| !self.spot_claims.contains_key(i) && reachable(**s))
            .filter(|(_, s)| !own.iter().any(|u| u.def == kit.extractor && u.pos.dist2d(**s) < SPOT_OCCUPIED_RADIUS))
            .min_by(|(_, a), (_, b)| a.dist2d(builder.pos).total_cmp(&b.dist2d(builder.pos)))?;
        self.spot_claims.insert(index, frame);
        // The engine stores the spot's metal value in `y`.
        Some(Vec3 { y: 0.0, ..*spot })
    }

    /// The nearest far-flung extractor with no turret beside it; raiders pick those off first.
    fn unguarded_outpost(&self, builder: &OwnUnit, own: &[OwnUnit], kit: &Kit) -> Option<Vec3> {
        let guarded = |pos: Vec3| {
            let turret_near = own.iter().any(|u| u.def == kit.turret && u.pos.dist2d(pos) < OUTPOST_GUARD_RADIUS);
            let on_its_way = self.jobs.iter().any(|(id, job)| *job == kit.turret && *id != builder.id);
            turret_near || on_its_way
        };
        own.iter()
            .filter(|u| u.def == kit.extractor && u.pos.dist2d(self.home) > OUTPOST_DISTANCE && !guarded(u.pos))
            .map(|u| u.pos)
            .min_by(|a, b| a.dist2d(builder.pos).total_cmp(&b.dist2d(builder.pos)))
    }

    /// What an idle factory queues next.
    fn production_batch(&self, own: &[OwnUnit], kit: &Kit) -> impl Iterator<Item = UnitDefId> + use<> {
        let count = |def: UnitDefId| own.iter().filter(|u| u.def == def).count();
        let wanted_constructors = (2 + count(kit.extractor) / 4).min(MAX_CONSTRUCTORS);
        let support = if count(kit.constructor) < wanted_constructors { kit.constructor } else { kit.artillery };
        // Fighters first: early raids arrive before an all-constructor opening pays off.
        [kit.raider, kit.raider, support, kit.skirmisher, kit.skirmisher].into_iter()
    }
}
