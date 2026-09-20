//! Builders and factories: what to build next, and where.

use std::collections::HashMap;

use bot_protocol::{BuildSite, Command, OwnUnit, Tick, UnitDefId, Vec3};

use super::roster::Kit;
use crate::strategist::shared::Focus;
use super::{Brain, FRAMES_PER_SECOND};

/// The commander never builds farther from home than this.
const COMMANDER_LEASH: f32 = 900.0;
/// H-ECO-EARLY-EXPAND: until this frame the commander's leash is the longer one (no raider that can hurt it is out
/// yet), and until we hold this many extractors constructors take a spot before anything else.
const EARLY_FRAMES: i32 = 5 * 60 * FRAMES_PER_SECOND;
const EARLY_COMMANDER_LEASH: f32 = 1500.0;
const EARLY_EXTRACTORS: usize = 5;
/// H-ECO-NANO: a construction turret per this much metal income, up to this many per factory, placed within reach of it.
const NANO_PER_INCOME: f32 = 8.0;
const NANOS_PER_LAB: usize = 3;
const NANO_REACH: f32 = 260.0;
/// H-ECO-REPAIR: a constructor within this distance mends the commander below this share of its health, else a
/// turret, extractor, factory or construction turret below that one.
const REPAIR_WITHIN: f32 = 1200.0;
const COMMANDER_REPAIR_BELOW: f32 = 0.85;
const BUILDING_REPAIR_BELOW: f32 = 0.7;
/// H-ECO-RECLAIM: with less metal than this banked, a constructor reclaims the wrecks of a recent fight within this
/// walking distance before anything else; one constructor per site.
const RECLAIM_WHEN_METAL_BELOW: f32 = 150.0;
const RECLAIM_RADIUS: f32 = 350.0;
const RECLAIM_WITHIN: f32 = 1800.0;
/// A stationed commander (D-COMMANDER-STATION) builds within this distance of its station and walks back beyond it.
const COMMANDER_STATION_REACH: f32 = 500.0;
const MAX_LABS: usize = 8;
const MAX_TURRETS: usize = 6;
const MAX_CONVERTERS: usize = 40;
/// An extractor beyond this distance from home gets a turret of its own.
/// Extractors farther from home than this want a turret of their own. It was 1200, which left the third and fourth
/// spots of the north-west start (963 and 1199 out, past the base turret line) with none, and they died 3-5 times a game.
const OUTPOST_DISTANCE: f32 = 500.0;
/// H-ECO-HOT-SPOTS: a metal spot where we lost an extractor or a constructor is left alone for this long, unless a
/// turret or soldiers cover it by then.
const HOT_SPOT_FRAMES: i32 = 4 * 60 * 30;
/// Soldiers this close to a spot, at least this many, count as cover.
const COVER_RADIUS: f32 = 500.0;
const COVER_SOLDIERS: usize = 3;
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
/// H-ECO-REACH: constructors take spots within this walking distance of home, plus this much per soldier we have.
const EXPANSION_REACH: f32 = 1500.0;
const EXPANSION_REACH_PER_SOLDIER: f32 = 50.0;
/// Metal spots within this walking distance of the start are built before anything else; farther ones after the lab.
const OPENING_REACH: f32 = 300.0;
/// Generators before the first lab, counting a wind generator as one and a solar as two.
const OPENING_GENERATORS: usize = 2;
/// No orders before this frame: the engine loses them.
const FIRST_ORDER_FRAME: i32 = 60;
/// A builder is not judged idle for this long after an order: the order has to reach it first.
const ORDER_GRACE_FRAMES: i32 = 45;
/// Frames between ticks (the shim's tick interval).
const TICK_FRAMES: i32 = 15;

/// The rules a builder tries once the opening stands and energy is not short.
enum Step {
    Repair,
    Reclaim,
    Nano,
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
    /// A building placed right beside `anchor`: a construction turret has to reach the factory it helps.
    Beside(UnitDefId, Vec3),
    /// Reclaim the wrecks around a place where units died.
    Reclaim(Vec3),
    /// Restore a damaged unit of ours.
    Repair(bot_protocol::UnitId),
}

impl Brain {
    pub(super) fn run_economy(&mut self, tick: &Tick, kit: &Kit, commands: &mut Vec<Command>) {
        let own = &tick.snapshot.own_units;
        // An order takes a few frames to reach the unit (it travels as a network message, longer at game start), so a
        // builder just ordered still reads as idle. Treating it as idle re-planned it, and the new order replaced the
        // old: the opening's two extractors were overwritten by the first generator within 1.5 s in every game.
        let just_ordered = |id: &bot_protocol::UnitId| self.last_orders.get(id).is_some_and(|(frame, _, _)| tick.frame - frame < ORDER_GRACE_FRAMES);
        self.jobs.retain(|id, _| own.iter().any(|u| u.id == *id && (!u.idle || just_ordered(id))));
        self.spot_claims.retain(|_, claimed| tick.frame - *claimed < SPOT_CLAIM_FRAMES);
        self.note_unreachable_sites(tick);

        // The engine drops orders given in the first second of the game; the lost extractor order then held its
        // spot's claim, and the opening went on without it.
        if tick.frame < FIRST_ORDER_FRAME {
            return;
        }
        for unit in own.iter().filter(|u| u.idle && !u.being_built) {
            if self.last_orders.get(&unit.id).is_some_and(|(frame, _, _)| tick.frame - frame < ORDER_GRACE_FRAMES) {
                continue;
            }
            let Some(def) = self.world.def(unit.def) else { continue };
            let (is_builder, is_mobile) = (def.build_speed > 0.0, def.speed > 0.0);
            let stationed_at = self.directives.commander_station.map(|d| self.snap_to_reachable(d.value)).filter(|_| unit.def == kit.commander);
            if let Some(station) = stationed_at
                && unit.pos.dist2d(station) > COMMANDER_STATION_REACH
            {
                self.fire("D-COMMANDER-STATION");
                commands.push(Command::Move { unit: unit.id, to: station, queue: false });
                continue;
            }
            if is_builder && is_mobile {
                let (plan, rule) = self.plan_for(unit, tick, kit);
                // A stationed commander builds where it stands, whatever anchor the rule had in mind.
                let plan = match (plan, stationed_at) {
                    (Plan::Near(def_id, _), Some(station)) => Plan::Near(def_id, station),
                    (plan, _) => plan,
                };
                if let Plan::Repair(target) = plan {
                    self.fire(rule);
                    self.jobs.insert(unit.id, kit.commander);
                    commands.push(Command::Repair { unit: unit.id, target, queue: false });
                    continue;
                }
                if let Plan::Reclaim(site) = plan {
                    self.fire(rule);
                    // Busy, but building nothing: the commander's type stands for "no building" in the job counts.
                    self.jobs.insert(unit.id, kit.commander);
                    commands.push(Command::ReclaimArea { unit: unit.id, centre: site, radius: RECLAIM_RADIUS, queue: false });
                    continue;
                }
                let planned_def = match plan {
                    Plan::Extractor(_) => kit.extractor,
                    Plan::Near(def_id, _) | Plan::Beside(def_id, _) => def_id,
                    Plan::Reclaim(_) | Plan::Repair(_) => unreachable!("handled above"),
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
                    Plan::Beside(def_id, anchor) => (def_id, BuildSite { near: anchor, search_radius: NANO_REACH, min_dist: 2 }),
                    Plan::Reclaim(_) | Plan::Repair(_) => unreachable!("handled above"),
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
            } else if unit.def == kit.nano {
                // H-ECO-NANO: a construction turret guards the nearest factory, which lends it its build power.
                let lab = own.iter().filter(|u| u.def == kit.lab).min_by(|a, b| a.pos.dist2d(unit.pos).total_cmp(&b.pos.dist2d(unit.pos)));
                if let Some(lab) = lab {
                    self.jobs.insert(unit.id, kit.commander);
                    commands.push(Command::Guard { unit: unit.id, target: lab.id });
                }
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

        // H-ECO-OPENING: the first minute is limited by the commander's walking, not by resources (1000 of each in
        // the bank), so the opening is laid out around where the commander stands: the extractors it can reach
        // almost from the start point, generators right beside it, then the lab; a second extractor farther off
        // waits until the lab is going up.
        let lab_started = planned(kit.lab) >= 1;
        // The nearest free spot is the one `claim_spot` hands the commander, so "a free spot near home" decides it.
        let spot_near_home = self.world.hello.metal_spots.iter().enumerate().any(|(i, s)| {
            self.walk_from_home(*s) < OPENING_REACH && !self.spot_claims.contains_key(&i) && !self.spot_taken(*s, snapshot.own_units.as_slice(), kit)
        });
        if planned(kit.extractor) < 2
            && (lab_started || spot_near_home)
            && let Some(spot) = self.claim_spot(builder, snapshot.own_units.as_slice(), kit, tick.frame)
        {
            return (Plan::Extractor(spot), "H-ECO-OPENING");
        }
        // One solar's worth of energy before the lab (a wind generator counts as half): the 1000 energy we start with
        // carries the lab, and four generators first overflowed it and delayed the lab to 0:58 (docs/studies/build-order.md).
        let opening_energy = planned(kit.wind) + 2 * planned(kit.solar);
        if opening_energy < OPENING_GENERATORS {
            // Beside the commander, wherever it is: no walking between the first buildings.
            return (Plan::Beside(small_generator, builder.pos), "H-ECO-OPENING");
        }
        if planned(kit.lab) < 1 {
            return (Plan::Near(kit.lab, yard), "H-ECO-OPENING");
        }
        if !is_commander && can_build(kit.turret) && !self.turret_requests.is_empty() {
            return (Plan::Near(kit.turret, self.turret_requests.remove(0)), "D-TURRET-REQUEST");
        }
        // H-ECO-EARLY-EXPAND: at two extractors all metal is spent as it arrives and nothing else we build helps; the
        // opponent holds four by minute 3. Constructors go for spots first (the commander keeps the energy up), and
        // so does the commander while energy is not short.
        if self.enabled("H-ECO-EARLY-EXPAND")
            && planned(kit.extractor) < EARLY_EXTRACTORS
            && !stalled
            && (!is_commander || !energy_short)
            && let Some(spot) = self.claim_spot(builder, snapshot.own_units.as_slice(), kit, tick.frame)
        {
            return (Plan::Extractor(spot), "H-ECO-EARLY-EXPAND");
        }
        // H-ECO-SPEND: metal in the bank is army we do not have. When it piles up the factory is the bottleneck,
        // whatever the focus says: construction turrets on the labs we have, then another lab. (Commander game 3:
        // 12 extractors against 6 by minute 5, then 1400-1750 banked for five minutes behind one lab, income 28
        // and spending 12, while the opponent's army passed ours.)
        if self.enabled("H-ECO-SPEND") && snapshot.metal.current > FLOATING_METAL && !energy_short {
            let labs: Vec<Vec3> = snapshot.own_units.iter().filter(|u| u.def == kit.lab && !u.being_built).map(|u| u.pos).collect();
            if can_build(kit.nano)
                && planned(kit.nano) < labs.len() * NANOS_PER_LAB
                && let Some(lab) = labs.iter().min_by(|a, b| a.dist2d(builder.pos).total_cmp(&b.dist2d(builder.pos)))
            {
                return (Plan::Beside(kit.nano, *lab), "H-ECO-SPEND");
            }
            if planned(kit.lab) < MAX_LABS && planned(kit.lab) <= labs.len() {
                return (Plan::Near(kit.lab, yard), "H-ECO-SPEND");
            }
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
            Some(Focus::Expand) => &[Step::Repair, Step::Reclaim, Step::Expand, Step::OutpostTurret, Step::FirstTurrets, Step::MoreLabs, Step::Convert, Step::MoreTurrets],
            Some(Focus::Production) => &[Step::Repair, Step::Nano, Step::MoreLabs, Step::FirstTurrets, Step::Expand, Step::OutpostTurret, Step::Convert, Step::MoreTurrets],
            Some(Focus::Defence) => &[Step::Repair, Step::Reclaim, Step::MoreTurrets, Step::OutpostTurret, Step::Expand, Step::MoreLabs, Step::Convert],
            Some(Focus::Energy) | None => &[Step::Repair, Step::Reclaim, Step::FirstTurrets, Step::Nano, Step::MoreLabs, Step::OutpostTurret, Step::Expand, Step::Convert, Step::MoreTurrets],
        };
        if focus.is_some() {
            self.fire("D-ECONOMY-FOCUS");
        }
        // A production focus spends on labs as soon as any metal is banked.
        let floating = if focus == Some(Focus::Production) { FLOATING_METAL / 3.0 } else { FLOATING_METAL };
        for step in order {
            match step {
                Step::Repair if !is_commander && self.enabled("H-ECO-REPAIR") => {
                    if let Some(target) = self.claim_repair(builder, snapshot.own_units.as_slice(), kit, tick.frame) {
                        return (Plan::Repair(target), "H-ECO-REPAIR");
                    }
                }
                Step::Reclaim if !is_commander && snapshot.metal.current < RECLAIM_WHEN_METAL_BELOW && self.enabled("H-ECO-RECLAIM") => {
                    if let Some(site) = self.claim_wreck_site(builder, tick.frame) {
                        return (Plan::Reclaim(site), "H-ECO-RECLAIM");
                    }
                }
                Step::Nano if can_build(kit.nano) && self.enabled("H-ECO-NANO") => {
                    // One turret per NANO_PER_INCOME of metal income, and no more than a factory can use.
                    let labs: Vec<Vec3> = snapshot.own_units.iter().filter(|u| u.def == kit.lab && !u.being_built).map(|u| u.pos).collect();
                    let wanted = ((snapshot.metal.income / NANO_PER_INCOME) as usize).min(labs.len() * NANOS_PER_LAB);
                    if planned(kit.nano) < wanted
                        && let Some(lab) = labs.iter().min_by(|a, b| a.dist2d(builder.pos).total_cmp(&b.dist2d(builder.pos)))
                    {
                        return (Plan::Beside(kit.nano, *lab), "H-ECO-NANO");
                    }
                }
                Step::FirstTurrets if planned(kit.turret) < 2 => return (Plan::Near(kit.turret, front), "H-ECO-BASE-TURRETS"),
                Step::MoreTurrets if planned(kit.turret) < MAX_TURRETS => return (Plan::Near(kit.turret, front), "H-ECO-BASE-TURRETS"),
                // A construction turret is a third of a lab's metal for the same build power: labs come after them.
                Step::MoreLabs
                    if snapshot.metal.current > floating
                        && planned(kit.lab) < MAX_LABS
                        && (!self.enabled("H-ECO-NANO") || planned(kit.nano) >= planned(kit.lab) * NANOS_PER_LAB) =>
                {
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
                // Not with metal in the bank: a converter then buys metal we already cannot spend.
                Step::Convert if energy_rich && planned(kit.converter) < MAX_CONVERTERS && snapshot.metal.current < FLOATING_METAL => {
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
        let commander_station = self.directives.commander_station.map(|d| d.value);
        let expansion_radius = self.directives.expansion_radius.map(|d| d.value);
        if commander_station.is_some() || expansion_radius.is_some() {
            self.fire(if is_commander { "D-COMMANDER-STATION" } else { "D-EXPANSION-RADIUS" });
        }
        let soldiers = own.iter().filter(|u| self.is_army(u, kit)).count();
        let reachable = |spot: Vec3| {
            if is_commander {
                match commander_station {
                    Some(station) => spot.dist2d(station) < COMMANDER_STATION_REACH,
                    None => {
                        let early = frame < EARLY_FRAMES && self.enabled("H-ECO-EARLY-EXPAND");
                        self.walk_from_home(spot) < if early { EARLY_COMMANDER_LEASH } else { COMMANDER_LEASH }
                    }
                }
            } else {
                // H-ECO-REACH: no farther from home than the army can answer for. The far line of spots (2000-2400
                // out) was bought and swept in every game, constructors and all.
                // A commander's radius is the whole rule: it may reach past the half of the map the bot keeps to on its
                // own (in commander game 5 it could not, and sat on 7 extractors with 2.7 times the opponent's army).
                match expansion_radius {
                    Some(radius) => self.reachable_on_foot(spot) && self.walk_from_home(spot) <= radius as f32,
                    None => {
                        let reach = EXPANSION_REACH + EXPANSION_REACH_PER_SOLDIER * soldiers as f32;
                        self.spot_is_ours(spot) && (!self.enabled("H-ECO-REACH") || self.walk_from_home(spot) <= reach)
                    }
                }
            }
        };
        // D-EXPANSION-PLAN: the commander's named spots come first, in its order, wherever they lie and however often
        // they have been raided: which ground to take, retake or give up is its call.
        let free = |i: usize, s: Vec3| !self.spot_claims.contains_key(&i) && self.reachable_on_foot(s) && !self.is_unreachable(s) && !self.spot_taken(s, own, kit);
        if !is_commander {
            let planned = self.spot_priority.iter().copied().find(|i| self.world.hello.metal_spots.get(*i).is_some_and(|s| free(*i, *s)));
            if let Some(index) = planned {
                let spot = self.world.hello.metal_spots[index];
                self.spot_claims.insert(index, frame);
                self.fire("D-EXPANSION-PLAN");
                return Some(Vec3 { y: 0.0, ..spot });
            }
        }
        let (index, spot) = self
            .world
            .hello
            .metal_spots
            .iter()
            .enumerate()
            .filter(|(i, s)| !self.spot_claims.contains_key(i) && reachable(**s) && !self.is_unreachable(**s) && !self.spot_avoid.contains(i))
            .filter(|(_, s)| !self.is_hot(**s, frame) || self.is_covered(**s, own, kit))
            .filter(|(_, s)| !own.iter().any(|u| u.def == kit.extractor && u.pos.dist2d(**s) < SPOT_OCCUPIED_RADIUS))
            .min_by(|(_, a), (_, b)| a.dist2d(builder.pos).total_cmp(&b.dist2d(builder.pos)))?;
        self.spot_claims.insert(index, frame);
        // The engine stores the spot's metal value in `y`.
        Some(Vec3 { y: 0.0, ..*spot })
    }

    /// What this constructor should mend: the commander first (the game ends with it), then the nearest damaged
    /// turret, extractor or factory, each by one constructor at a time.
    fn claim_repair(&mut self, builder: &OwnUnit, own: &[OwnUnit], kit: &Kit, frame: i32) -> Option<bot_protocol::UnitId> {
        const CLAIM_FRAMES: i32 = 20 * 30;
        self.repair_claims.retain(|_, since| frame - *since < CLAIM_FRAMES);
        let hurt = |u: &&OwnUnit, below: f32| !u.being_built && u.max_health > 0.0 && u.health < u.max_health * below;
        let in_reach = |u: &&OwnUnit| u.pos.dist2d(builder.pos) < REPAIR_WITHIN && !self.repair_claims.contains_key(&u.id);
        let commander = own.iter().find(|u| u.def == kit.commander && hurt(u, COMMANDER_REPAIR_BELOW) && in_reach(u));
        let building = || {
            own.iter()
                .filter(|u| [kit.turret, kit.extractor, kit.lab, kit.nano].contains(&u.def) && hurt(u, BUILDING_REPAIR_BELOW) && in_reach(u))
                .min_by(|a, b| a.pos.dist2d(builder.pos).total_cmp(&b.pos.dist2d(builder.pos)))
        };
        let target = commander.or_else(building)?.id;
        self.repair_claims.insert(target, frame);
        Some(target)
    }

    /// The nearest place, on our side of the map, where units died lately and nobody has been sent to reclaim yet.
    fn claim_wreck_site(&mut self, builder: &OwnUnit, frame: i32) -> Option<Vec3> {
        const FRESH_FRAMES: i32 = 4 * 60 * 30;
        self.wreck_sites.retain(|(_, since)| frame - since < FRESH_FRAMES);
        let index = self
            .wreck_sites
            .iter()
            .enumerate()
            .filter(|(_, (site, _))| site.dist2d(builder.pos) < RECLAIM_WITHIN && self.spot_is_ours(*site))
            .min_by(|(_, (a, _)), (_, (b, _))| a.dist2d(builder.pos).total_cmp(&b.dist2d(builder.pos)))
            .map(|(index, _)| index)?;
        Some(self.wreck_sites.swap_remove(index).0)
    }

    fn spot_taken(&self, spot: Vec3, own: &[OwnUnit], kit: &Kit) -> bool {
        own.iter().any(|u| u.def == kit.extractor && u.pos.dist2d(spot) < SPOT_OCCUPIED_RADIUS)
    }

    /// Whether we lost an extractor or a constructor at this metal spot lately.
    fn is_hot(&self, spot: Vec3, frame: i32) -> bool {
        self.hot_spots.iter().any(|(hot, until)| *until > frame && hot.dist2d(spot) < SPOT_OCCUPIED_RADIUS)
    }

    /// A turret beside the spot, or a few soldiers standing by it.
    fn is_covered(&self, spot: Vec3, own: &[OwnUnit], kit: &Kit) -> bool {
        own.iter().any(|u| u.def == kit.turret && !u.being_built && u.pos.dist2d(spot) < OUTPOST_GUARD_RADIUS)
            || own.iter().filter(|u| self.is_army(u, kit) && u.pos.dist2d(spot) < COVER_RADIUS).count() >= COVER_SOLDIERS
    }

    /// Notes the metal spot nearest a lost extractor or constructor as hot.
    pub(super) fn note_hot_spot(&mut self, lost_at: Vec3, frame: i32) {
        self.hot_spots.retain(|(_, until)| *until > frame);
        let nearest = self.world.hello.metal_spots.iter().min_by(|a, b| a.dist2d(lost_at).total_cmp(&b.dist2d(lost_at)));
        if let Some(spot) = nearest.filter(|s| s.dist2d(lost_at) < 400.0) {
            let spot = Vec3 { y: 0.0, ..*spot };
            self.hot_spots.retain(|(hot, _)| hot.dist2d(spot) > 1.0);
            self.hot_spots.push((spot, frame + HOT_SPOT_FRAMES));
        }
    }

    /// The nearest far-flung extractor with no turret beside it; raiders pick those off first.
    fn unguarded_outpost(&self, builder: &OwnUnit, own: &[OwnUnit], kit: &Kit) -> Option<Vec3> {
        let guarded = |pos: Vec3| {
            let turret_near = own.iter().any(|u| u.def == kit.turret && u.pos.dist2d(pos) < OUTPOST_GUARD_RADIUS);
            // Somebody is already building a turret for THIS place. (The test used to ignore the place, so one turret
            // under construction anywhere made every outpost count as guarded and they were built one at a time.)
            let on_its_way = self.jobs.iter().any(|(id, job)| {
                *job == kit.turret
                    && *id != builder.id
                    && self.last_orders.get(id).is_some_and(|(_, _, near)| near.dist2d(pos) < 2.0 * OUTPOST_GUARD_RADIUS)
            });
            turret_near || on_its_way
        };
        // Hot spots count as outposts to be: the turret goes up first, and the spot reopens once it stands.
        let hot = self.hot_spots.iter().map(|(spot, _)| *spot).filter(|spot| self.spot_is_ours(*spot));
        own.iter()
            .filter(|u| u.def == kit.extractor)
            .map(|u| u.pos)
            .chain(hot)
            .filter(|pos| pos.dist2d(self.home) > OUTPOST_DISTANCE && !guarded(*pos))
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
        // By the work there is, not by what we hold: a target that follows our extractor count is lowest exactly
        // when raids have taken them and there is most to rebuild.
        let free_spots = self
            .world
            .hello
            .metal_spots
            .iter()
            .filter(|s| self.spot_is_ours(**s) && !own.iter().any(|u| u.def == kit.extractor && u.pos.dist2d(**s) < SPOT_OCCUPIED_RADIUS))
            .count();
        (2 + extractors / 3 + free_spots / 3).min(MAX_CONSTRUCTORS).max(floor)
    }

    /// What an idle factory queues next.
    fn production_batch(&self, own: &[OwnUnit], kit: &Kit) -> impl Iterator<Item = UnitDefId> + use<> {
        let count = |def: UnitDefId| own.iter().filter(|u| u.def == def).count();
        let wanted_constructors = self.wanted_constructors(own, kit);
        let support = if count(kit.constructor) < wanted_constructors { kit.constructor } else { kit.artillery };
        // Fighters first: early raids arrive before an all-constructor opening pays off.
        // H-PROD-MIX: at equal metal the line unit (Mace, Thug) wins most tier-1 fights and the old staples (Pawn,
        // Rocketeer; Grunt) lose them (docs/data/duels-2026-09-19). One fast raider a batch stays, for responders.
        // H-PROD-BUILDERS-FIRST: the first two units out of the lab are constructors, and a wanted constructor leads
        // its batch. With the constructor third in the batch the first one came at 1:50 and the second after 3:30,
        // and we sat on two extractors until minute 4.
        if self.enabled("H-PROD-BUILDERS-FIRST") && support == kit.constructor {
            let line = if self.enabled("H-PROD-MIX") { kit.line } else { kit.raider };
            let second = if count(kit.constructor) == 0 { kit.constructor } else { line };
            return [kit.constructor, second, line, kit.raider, line].into_iter();
        }
        if self.enabled("H-PROD-MIX") {
            let support = if support == kit.artillery { kit.line } else { support };
            return [kit.line, kit.raider, support, kit.line, kit.second].into_iter();
        }
        [kit.raider, kit.raider, support, kit.skirmisher, kit.skirmisher].into_iter()
    }
}
