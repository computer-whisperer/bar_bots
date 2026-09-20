//! Builders and factories: what to build next, and where.

use std::collections::HashMap;

use bot_protocol::{BuildSite, Command, OwnUnit, Tick, UnitDefId, Vec3};

use super::roster::Kit;
use crate::strategist::shared::Focus;
use super::{Brain, FRAMES_PER_SECOND};

/// The commander never builds farther from home than this.
const COMMANDER_LEASH: f32 = 900.0;
const MAX_LABS: usize = 8;
const MAX_TURRETS: usize = 6;
const MAX_CONVERTERS: usize = 40;
/// An extractor beyond this distance from home gets a turret of its own.
const OUTPOST_DISTANCE: f32 = 1200.0;
const OUTPOST_GUARD_RADIUS: f32 = 350.0;
/// BARb medium runs 4-6 constructors by minute 10 and 10-20 later; we ran 2-4 and never rebuilt what raids took (observe-2).
const MAX_CONSTRUCTORS: usize = 10;
/// Constructors die with the outposts they build; the count must not shrink with the extractor count.
const MIN_CONSTRUCTORS: usize = 3;
/// Stored metal above which the base is under-spending and wants another lab.
const FLOATING_METAL: f32 = 500.0;
/// Energy income beyond which solar collectors are too small to keep up.
const ADVANCED_SOLAR_INCOME: f32 = 250.0;
/// Average wind speed from which wind generators replace solar collectors (solar: 20 energy for 155 metal; wind: the
/// wind speed in energy for 40 metal).
const WINDY_AVERAGE: f32 = 8.0;
/// Stored energy, as a fraction of storage, below which nothing that costs energy to build gets started.
const STALLED_ENERGY: f32 = 0.15;
/// How long a metal spot stays reserved for a builder that was sent to it.
const SPOT_CLAIM_FRAMES: i32 = 60 * FRAMES_PER_SECOND;
/// A metal extractor this close to a spot occupies it.
const SPOT_OCCUPIED_RADIUS: f32 = 60.0;
/// Gaps between base buildings, in 8-elmo build squares. Three squares made a maze the army could not leave.
const BUILDING_GAP: i32 = 5;
const LAB_GAP: i32 = 8;
/// Distances from the start point along the line to the enemy: generators behind, labs ahead, turrets beyond them.
const BACK_FIELD: f32 = 150.0;
const LAB_YARD: f32 = 350.0;
const TURRET_LINE: f32 = 650.0;
/// A site a builder failed to reach is avoided, with everything this close to it, for this long.
const UNREACHABLE_RADIUS: f32 = 120.0;
const UNREACHABLE_FRAMES: i32 = 5 * 60 * FRAMES_PER_SECOND;
/// Frames between ticks (the shim's tick interval).
const TICK_FRAMES: i32 = 15;

/// The rules a builder tries once the opening stands and energy is not short.
enum Step {
    FirstTurrets,
    MoreTurrets,
    MoreLabs,
    OutpostTurret,
    Expand,
    Convert,
}

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
        self.note_unreachable_sites(tick);

        for unit in own.iter().filter(|u| u.idle && !u.being_built) {
            let Some(def) = self.world.def(unit.def) else { continue };
            let (is_builder, is_mobile) = (def.build_speed > 0.0, def.speed > 0.0);
            if is_builder && is_mobile {
                let (plan, rule) = self.plan_for(unit, tick, kit);
                let planned_def = match plan {
                    Plan::Extractor(_) => kit.extractor,
                    Plan::Near(def_id, _) => def_id,
                };
                let buildable = self.world.def(unit.def).is_some_and(|d| d.build_options.contains(&planned_def));
                if !buildable {
                    // A rule chose something this builder cannot make; say so rather than issue a doomed order.
                    eprintln!("[ai {}] f={} {rule} chose {} which {} cannot build", self.ai(), tick.frame, self.name(planned_def), self.name(unit.def));
                    continue;
                }
                self.fire(rule);
                let (def_id, site) = match plan {
                    // The game rejects an extractor that is not exactly on its spot (cmd_mex_denier.lua), and the shim
                    // places extractors exactly at `near`, searching nowhere.
                    Plan::Extractor(spot) => (kit.extractor, BuildSite { near: spot, search_radius: 0.0, min_dist: 0 }),
                    // Where the builder stands is reachable by definition; fall back to it when the usual anchor is not.
                    Plan::Near(def_id, anchor) if self.is_unreachable(anchor) => {
                        (def_id, BuildSite { near: unit.pos, search_radius: 500.0, min_dist: self.gap_around(def_id, kit) })
                    }
                    Plan::Near(def_id, anchor) => {
                        (def_id, BuildSite { near: anchor, search_radius: 1000.0, min_dist: self.gap_around(def_id, kit) })
                    }
                };
                // An order whose builder is idle again within two ticks never started: count it and say where.
                if let Some((frame, earlier, near)) = self.last_orders.insert(unit.id, (tick.frame, def_id, site.near))
                    && tick.frame - frame <= 2 * TICK_FRAMES
                {
                    self.dropped_orders += 1;
                    if self.dropped_orders <= 40 {
                        eprintln!(
                            "[ai {}] f={} DROPPED order: {} (unit {}) at ({:.0}, {:.0}) was to build {} near ({:.0}, {:.0}), {:.0} away",
                            self.ai(), tick.frame, self.name(unit.def), unit.id.0, unit.pos.x, unit.pos.z,
                            self.name(earlier), near.x, near.z, unit.pos.dist2d(near)
                        );
                    }
                }
                self.jobs.insert(unit.id, def_id);
                commands.push(Command::Build { unit: unit.id, def: def_id, site: Some(site), queue: false });
            } else if is_builder && let Some(def_id) = self.weighted_production(unit, own, kit) {
                self.fire("D-PRODUCTION-MIX");
                commands.push(Command::Build { unit: unit.id, def: def_id, site: None, queue: false });
            } else if is_builder {
                self.fire("H-PROD-BATCH");
                commands.extend(self.production_batch(own, kit).map(|def_id| Command::Build {
                    unit: unit.id,
                    def: def_id,
                    site: None,
                    queue: false,
                }));
            }
        }
    }

    /// A builder whose move failed could not reach its site. Remember that, or it is sent there again at once,
    /// fails again, and spends the game walking into a cliff. Spots and base sites are avoided for a while.
    fn note_unreachable_sites(&mut self, tick: &Tick) {
        self.unreachable.retain(|(_, until)| *until > tick.frame);
        for event in &tick.events {
            let bot_protocol::Event::UnitMoveFailed { unit } = event else { continue };
            let Some((_, def, near)) = self.last_orders.get(unit).copied() else { continue };
            if !self.unreachable.iter().any(|(bad, _)| bad.dist2d(near) < UNREACHABLE_RADIUS) {
                eprintln!("[ai {}] f={} cannot reach ({:.0}, {:.0}) to build {}; avoiding it", self.ai(), tick.frame, near.x, near.z, self.name(def));
                self.unreachable.push((near, tick.frame + UNREACHABLE_FRAMES));
            }
        }
    }

    /// The gap, in build squares, a new building keeps from its neighbours: wide enough for units to walk through.
    fn gap_around(&self, def: UnitDefId, kit: &Kit) -> i32 {
        if def == kit.lab { LAB_GAP } else { BUILDING_GAP }
    }

    fn is_unreachable(&self, point: Vec3) -> bool {
        self.unreachable.iter().any(|(bad, _)| bad.dist2d(point) < UNREACHABLE_RADIUS)
    }

    /// The most urgent thing this builder can safely do, and the heuristic (docs/heuristics.md) that chose it.
    fn plan_for(&mut self, builder: &OwnUnit, tick: &Tick, kit: &Kit) -> (Plan, &'static str) {
        let snapshot = &tick.snapshot;
        let is_commander = builder.def == kit.commander;
        // H-ECO-JOBS: existing (finished or not) plus what other builders are already on their way to build.
        let mut counts: HashMap<UnitDefId, usize> = HashMap::new();
        let others_jobs = self.jobs.iter().filter(|(id, _)| **id != builder.id).map(|(_, job)| *job);
        for def in snapshot.own_units.iter().map(|u| u.def).chain(others_jobs) {
            *counts.entry(def).or_default() += 1;
        }
        let planned = |def: UnitDefId| counts.get(&def).copied().unwrap_or(0);
        let energy = snapshot.energy;
        // Judge energy by what is stored, not by income against usage: converters soak up any
        // surplus, so usage always catches up with income and would read as a permanent shortage.
        let energy_short = energy.current < energy.storage * 0.4;
        let energy_rich = energy.current > energy.storage * 0.8;
        // Builders differ in what they can build (a commander cannot build an advanced solar); an order outside
        // the builder's options is dropped by the engine without a word, leaving the builder idle forever.
        let options = self.world.def(builder.def).map(|d| d.build_options.clone()).unwrap_or_default();
        let can_build = |def: UnitDefId| options.contains(&def);
        // H-ECO-WIND: on a windy map a wind generator gives about twice a solar's energy per metal.
        let map = &self.world.hello.map;
        let windy = (map.wind_min + map.wind_max) / 2.0 >= WINDY_AVERAGE && self.enabled("H-ECO-WIND");
        // A wind generator costs energy to build and a solar collector none, so an energy stall is dug out of with solars.
        let stalled = energy.current < energy.storage * STALLED_ENERGY;
        let small_generator = if windy && !stalled && can_build(kit.wind) { kit.wind } else { kit.solar };
        let generator = if energy.income > ADVANCED_SOLAR_INCOME && can_build(kit.advanced_solar) { kit.advanced_solar } else { small_generator };
        // H-ECO-BASE-LAYOUT: labs in a yard out front, generators and converters behind the start, turrets beyond
        // the yard. Everything used to go around the start point, and the army jammed in the maze that made.
        let base = self.forward_of_home(-BACK_FIELD);
        let yard = self.forward_of_home(LAB_YARD);
        let front = self.forward_of_home(TURRET_LINE);

        if planned(kit.extractor) < 2
            && let Some(spot) = self.claim_spot(builder, snapshot.own_units.as_slice(), kit, tick.frame)
        {
            return (Plan::Extractor(spot), "H-ECO-OPENING");
        }
        // Two solars' worth of energy before the lab; a wind generator counts as half a solar.
        let opening_energy = planned(kit.wind) + 2 * planned(kit.solar);
        if opening_energy < 4 {
            return (Plan::Near(small_generator, base), "H-ECO-OPENING");
        }
        if planned(kit.lab) < 1 {
            return (Plan::Near(kit.lab, yard), "H-ECO-OPENING");
        }
        if !is_commander && can_build(kit.turret) && !self.turret_requests.is_empty() {
            return (Plan::Near(kit.turret, self.turret_requests.remove(0)), "D-TURRET-REQUEST");
        }
        let focus = self.directives.economy_focus.map(|f| f.value);
        if energy_short || (focus == Some(Focus::Energy) && energy.current < energy.storage * 0.9) {
            return (Plan::Near(generator, base), if energy_short { "H-ECO-ENERGY-BY-STORAGE" } else { "D-FOCUS-ENERGY" });
        }
        if let Some(ordered) = self.directives.min_converters
            && planned(kit.converter) < ordered.value
        {
            return (Plan::Near(kit.converter, base), "D-MIN-CONVERTERS");
        }
        // Once the opening stands, the remaining rules run in an order the strategist can change.
        let order: &[Step] = match focus {
            Some(Focus::Expand) => &[Step::Expand, Step::OutpostTurret, Step::FirstTurrets, Step::MoreLabs, Step::Convert, Step::MoreTurrets],
            Some(Focus::Production) => &[Step::MoreLabs, Step::FirstTurrets, Step::Expand, Step::OutpostTurret, Step::Convert, Step::MoreTurrets],
            Some(Focus::Defence) => &[Step::MoreTurrets, Step::OutpostTurret, Step::Expand, Step::MoreLabs, Step::Convert],
            Some(Focus::Energy) | None => &[Step::FirstTurrets, Step::MoreLabs, Step::OutpostTurret, Step::Expand, Step::Convert, Step::MoreTurrets],
        };
        if focus.is_some() {
            self.fire("D-ECONOMY-FOCUS");
        }
        // A production focus spends on labs as soon as any metal is banked.
        let floating = if focus == Some(Focus::Production) { FLOATING_METAL / 3.0 } else { FLOATING_METAL };
        for step in order {
            match step {
                Step::FirstTurrets if planned(kit.turret) < 2 => return (Plan::Near(kit.turret, front), "H-ECO-BASE-TURRETS"),
                Step::MoreTurrets if planned(kit.turret) < MAX_TURRETS => return (Plan::Near(kit.turret, front), "H-ECO-BASE-TURRETS"),
                Step::MoreLabs if snapshot.metal.current > floating && planned(kit.lab) < MAX_LABS => {
                    return (Plan::Near(kit.lab, yard), "H-ECO-MORE-LABS");
                }
                Step::OutpostTurret if !is_commander => {
                    if let Some(outpost) = self.unguarded_outpost(builder, snapshot.own_units.as_slice(), kit) {
                        return (Plan::Near(kit.turret, outpost), "H-ECO-OUTPOST-TURRET");
                    }
                }
                Step::Expand => {
                    if let Some(spot) = self.claim_spot(builder, snapshot.own_units.as_slice(), kit, tick.frame) {
                        return (Plan::Extractor(spot), "H-ECO-EXPAND");
                    }
                }
                Step::Convert if energy_rich && planned(kit.converter) < MAX_CONVERTERS => {
                    return (Plan::Near(kit.converter, base), "H-ECO-CONVERT-SURPLUS");
                }
                _ => {}
            }
        }
        (Plan::Near(generator, base), "H-ECO-FALLBACK-ENERGY")
    }

    /// Reserves the nearest free metal spot this builder may go to: inside the leash for the
    /// commander (H-COM-LEASH), on our half of the map for constructors (H-ECO-OWN-HALF).
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
            .filter(|(i, s)| !self.spot_claims.contains_key(i) && reachable(**s) && !self.is_unreachable(**s))
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

    /// The commander's unit mix: the type furthest below its share of what is alive, one unit at a time. The
    /// constructor floor stays ours. `None` without a mix, or when nothing in it can be built here.
    fn weighted_production(&self, factory: &OwnUnit, own: &[OwnUnit], kit: &Kit) -> Option<UnitDefId> {
        if self.production_weights.is_empty() {
            return None;
        }
        let count = |def: UnitDefId| own.iter().filter(|u| u.def == def).count();
        let options = &self.world.def(factory.def)?.build_options;
        if count(kit.constructor) < self.wanted_constructors(own, kit) && options.contains(&kit.constructor) {
            // Every other unit until the floor is met, so the army is not starved by it.
            let army = own.iter().filter(|u| self.is_army(u, kit)).count();
            if army % 2 == 1 {
                return Some(kit.constructor);
            }
        }
        let mix: Vec<(UnitDefId, f32)> = self
            .production_weights
            .iter()
            .filter_map(|(name, weight)| Some((self.world.def_named(name)?, *weight as f32)))
            .filter(|(def, weight)| options.contains(def) && *weight > 0.0)
            .collect();
        let (total_weight, total_alive) = (mix.iter().map(|(_, w)| w).sum::<f32>(), mix.iter().map(|(d, _)| count(*d)).sum::<usize>());
        let deficit = |(def, weight): &(UnitDefId, f32)| weight / total_weight - count(*def) as f32 / (total_alive.max(1)) as f32;
        mix.iter().max_by(|a, b| deficit(a).total_cmp(&deficit(b))).map(|(def, _)| *def)
    }

    fn wanted_constructors(&self, own: &[OwnUnit], kit: &Kit) -> usize {
        let extractors = own.iter().filter(|u| u.def == kit.extractor).count();
        // H-PROD-CONSTRUCTOR-FLOOR
        let own_floor = if self.enabled("H-PROD-CONSTRUCTOR-FLOOR") { MIN_CONSTRUCTORS } else { 0 };
        let floor = self.directives.min_constructors.map_or(own_floor, |d| d.value);
        (3 + extractors / 2).min(MAX_CONSTRUCTORS).max(floor)
    }

    /// What an idle factory queues next.
    fn production_batch(&self, own: &[OwnUnit], kit: &Kit) -> impl Iterator<Item = UnitDefId> + use<> {
        let count = |def: UnitDefId| own.iter().filter(|u| u.def == def).count();
        let wanted_constructors = self.wanted_constructors(own, kit);
        let support = if count(kit.constructor) < wanted_constructors { kit.constructor } else { kit.artillery };
        // Fighters first: early raids arrive before an all-constructor opening pays off.
        [kit.raider, kit.raider, support, kit.skirmisher, kit.skirmisher].into_iter()
    }
}
