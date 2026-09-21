//! The economy simulator. Fixed time step; every assumption is listed in `docs/studies/build-order.md`.
//!
//! Per step: builders without a task take the next step of their queue and walk to the site; every builder on a site
//! asks for `build power / buildtime` of its target's cost per second; if the team cannot pay, every build slows by the
//! same factor (the scarcer resource decides); converters then burn stored energy above 75 % of storage; what does not
//! fit in storage is lost.

use std::sync::Arc;

use crate::game::{distance, Ground, Spot};
use crate::plan::{Item, Plan, Step};
use crate::units::{Role, Units};

#[derive(Clone, Debug)]
pub enum Wind {
    Constant(f64),
    /// One value per second of game time; the last one holds afterwards.
    Trace(Vec<f64>),
}

impl Wind {
    fn at(&self, t: f64) -> f64 {
        match self {
            Wind::Constant(v) => *v,
            Wind::Trace(values) => values.get(t as usize).or(values.last()).copied().unwrap_or(0.0),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Scenario {
    /// The commander's unit type (index into the unit table): the first builder, standing at `home`.
    pub commander: usize,
    pub home: (f64, f64),
    /// Metal spots an extractor without an explicit site may take.
    pub spots: Vec<Spot>,
    pub ground: Arc<dyn Ground>,
    pub wind: Wind,
    pub start_metal: f64,
    pub start_energy: f64,
    /// Team storage before any building adds to it.
    pub base_storage: f64,
    pub dt: f64,
    /// Elmos added to every builder's build distance: the engine measures reach to the target's edge (its model radius),
    /// not its centre. One constant for all buildings; the table carries no radii.
    pub reach_bonus: f64,
    /// Seconds a mobile builder loses per build besides walking (order latency, opening the nano spray). This and
    /// `walk_overhead` fit 565 builder trips in 12 recorded games on Quicksilver (median time beyond the walk itself:
    /// 1.2-1.5 s when the site was in reach, 2.4-3.4 s when not).
    pub mobile_overhead: f64,
    /// Further seconds lost on a walk (turning, accelerating, stopping); a walk shorter than this only doubles.
    pub walk_overhead: f64,
    /// Seconds a factory loses between units (the finished unit clearing the pad).
    pub factory_overhead: f64,
    /// The commander takes no metal spot farther from home than this on foot: the game ends with it.
    pub commander_leash: f64,
    /// An extractor farther from home than this with no turret within `turret_cover` is exposed: raiders take it.
    pub exposed_beyond: f64,
    pub turret_cover: f64,
    /// A builder within this of home puts economy buildings beside itself (H-OPEN-PLAN places them so).
    pub base_radius: f64,
    /// Length of one `Item::Assist`.
    pub assist_chunk: f64,
    /// Stored-energy fraction above which converters run (`mmLevel`, game_energy_conversion.lua).
    pub converter_level: f64,
    /// A constructor whose queue has run out keeps taking the nearest free metal spot. On for the search (a new
    /// constructor is then worth something before the search has written it a queue), off for replays.
    pub constructors_default_to_extractors: bool,
}

impl Scenario {
    pub fn new(commander: usize, home: (f64, f64), spots: Vec<Spot>, ground: Arc<dyn Ground>, wind: f64) -> Scenario {
        Scenario {
            commander,
            home,
            spots,
            ground,
            wind: Wind::Constant(wind),
            start_metal: 1000.0,
            start_energy: 1000.0,
            base_storage: 1000.0,
            dt: 0.5,
            reach_bonus: 40.0,
            mobile_overhead: 3.5,
            walk_overhead: 0.0,
            factory_overhead: 1.0,
            commander_leash: f64::MAX,
            exposed_beyond: 900.0,
            turret_cover: 450.0,
            base_radius: 600.0,
            assist_chunk: 20.0,
            converter_level: 0.75,
            constructors_default_to_extractors: false,
        }
    }
}

impl Scenario {
    /// A mobile builder's way to a build: the seconds before the build's first frame, and where it then stands
    /// (as far from the site as it reaches).
    pub fn trip(&self, place: (f64, f64), site: (f64, f64), range: f64, speed: f64) -> (f64, (f64, f64)) {
        let gap = distance(place, site);
        let reach = range + self.reach_bonus;
        if gap <= reach {
            return (self.mobile_overhead, place);
        }
        let walk = (self.ground.walk(place, site) - reach).max(0.0) / speed;
        let keep = reach / gap;
        (walk + walk.min(self.walk_overhead) + self.mobile_overhead, (site.0 + (place.0 - site.0) * keep, site.1 + (place.1 - site.1) * keep))
    }
}

/// State at the end of a whole game second.
#[derive(Clone, Debug, Default)]
pub struct Sample {
    pub t: f64,
    pub metal: f64,
    pub energy: f64,
    /// Commander + extractors + converters, per second, this second.
    pub metal_income: f64,
    /// Commander and generators, per second. Extractor upkeep is charged but, as in the engine's books, not netted here.
    pub energy_income: f64,
    pub extractors: u32,
    pub converters: u32,
    /// Sum of worker time over commander, constructors, factories and construction turrets.
    pub build_power: f64,
    pub factories: u32,
    pub constructors: u32,
    pub nanos: u32,
    pub army_count: u32,
    /// Metal cost of every combat unit finished so far (nothing ever dies here).
    pub army_value: f64,
    /// Share of the asked-for build progress that resources allowed, averaged over the second (1 = no stall).
    pub stall: f64,
    pub metal_wasted: f64,
    pub energy_wasted: f64,
    pub metal_spent: f64,
}

#[derive(Clone, Debug)]
pub struct Finished {
    pub t: f64,
    pub unit: usize,
    pub queue: usize,
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub samples: Vec<Sample>,
    pub finished: Vec<Finished>,
    /// Per queue, the steps its builder actually took up, in order. Steps it had to pass over are absent (no free
    /// metal spot, a unit this builder cannot build, a factory beyond the plan's factory queues), and so is the tail
    /// it never reached; extractors built under `Scenario::constructors_default_to_extractors` are present.
    pub effective: Vec<Vec<Step>>,
    /// Metal already sunk into combat units still on the pad at the end.
    pub army_in_progress: f64,
    /// Metal per second from extractors standing at the end farther than `Scenario::exposed_beyond` from home with no
    /// turret of ours within `Scenario::turret_cover`.
    pub exposed_income: f64,
}

impl Outcome {
    pub fn last(&self) -> &Sample {
        self.samples.last().expect("at least one second simulated")
    }

    /// Mean metal income over the `window` seconds before `t`.
    pub fn mean_metal_income(&self, t: f64, window: f64) -> f64 {
        let picked: Vec<f64> = self.samples.iter().filter(|s| s.t > t - window && s.t <= t).map(|s| s.metal_income).collect();
        picked.iter().sum::<f64>() / picked.len().max(1) as f64
    }
}

/// A finished unit of a modelled role standing when the plan is priced.
#[derive(Clone, Debug, PartialEq)]
pub struct Standing {
    pub unit: usize,
    pub site: (f64, f64),
    /// Metal per second, for an extractor.
    pub pays: f64,
}

/// What a builder is on when the plan is priced.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub unit: usize,
    pub site: (f64, f64),
    /// Share of the build done, 0 for one not yet begun (the builder then walks to the site first).
    pub progress: f64,
    /// Metal per second once it stands, for an extractor.
    pub pays: f64,
}

/// A builder when the plan is priced: the commander, a factory or a constructor.
#[derive(Clone, Debug, PartialEq)]
pub struct StateBuilder {
    pub unit: usize,
    pub place: (f64, f64),
    pub job: Option<Job>,
}

/// The game as it stands when a plan is priced: the empty start (`State::start`), or a snapshot of a game in
/// progress (`docs/design/2026-09-21-rolling-planner.md`). Time is absolute: samples, finishes and the wind read
/// `t0` onwards.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    /// Seconds into the game; taken to the nearest whole second.
    pub t0: f64,
    pub metal: f64,
    pub energy: f64,
    pub metal_storage: f64,
    pub energy_storage: f64,
    pub standing: Vec<Standing>,
    /// In queue order: the commander, then the standing factories, then the standing constructors. A plan's
    /// queues follow this order; the queues of factories and constructors not yet built come after, in the order
    /// they finish.
    pub builders: Vec<StateBuilder>,
}

impl State {
    /// The empty start: the commander at home with the scenario's starting stock.
    pub fn start(scenario: &Scenario) -> State {
        State {
            t0: 0.0,
            metal: scenario.start_metal,
            energy: scenario.start_energy,
            metal_storage: scenario.base_storage,
            energy_storage: scenario.base_storage,
            standing: Vec::new(),
            builders: vec![StateBuilder { unit: scenario.commander, place: scenario.home, job: None }],
        }
    }

    /// How many of the state's builders are factories, and how many constructors: where the plan's queues for
    /// builders yet to come begin.
    pub fn standing_factories(&self, units: &Units) -> usize {
        self.builders.iter().filter(|b| units.list[b.unit].role == Role::Factory).count()
    }

    pub fn standing_constructors(&self, units: &Units) -> usize {
        self.builders.iter().filter(|b| units.list[b.unit].role == Role::Builder).count()
    }

    /// The plan's queue count a state needs at least: one per builder it lists.
    pub fn plan_shape(&self, units: &Units, factories: usize, constructors: usize) -> (usize, usize) {
        (factories.max(self.standing_factories(units)), constructors.max(self.standing_constructors(units)))
    }
}

enum Doing {
    Idle,
    Travel { left: f64, then: Option<(usize, (f64, f64), f64)> },
    /// `pays`: metal per second once it stands, for an extractor.
    Build { unit: usize, site: (f64, f64), pays: f64, progress: f64 },
    Assist { left: f64 },
    Done,
}

impl Doing {
    /// On the way to a build, or already building when there is no way to go.
    fn begin(left: f64, unit: usize, site: (f64, f64), pays: f64) -> Doing {
        if left > 1e-9 { Doing::Travel { left, then: Some((unit, site, pays)) } } else { Doing::Build { unit, site, pays, progress: 0.0 } }
    }
}

struct Builder {
    queue: usize,
    next: usize,
    power: f64,
    speed: f64,
    range: f64,
    place: (f64, f64),
    /// Its own unit type.
    unit: usize,
    is_factory: bool,
    state: Doing,
}

/// Where the n-th building without an explicit site goes: a golden-angle spiral around home, so that base buildings
/// cost a little walking, as they do in the recorded games (consecutive wind turbines start 1.5-4.5 s apart).
fn base_site(home: (f64, f64), n: usize) -> (f64, f64) {
    let radius = 120.0 + 28.0 * n as f64;
    let angle = 2.4 * n as f64;
    (home.0 + radius * angle.cos(), home.1 + radius * angle.sin())
}

/// `plan` is played from `state` for `seconds`. The plan's queues follow the state's builders (`State::builders`),
/// then the factories and constructors the plan has still to build.
pub fn simulate(units: &Units, scenario: &Scenario, state: &State, plan: &Plan, seconds: f64) -> Outcome {
    let sc = scenario;
    let extractor = units.extractor(sc.commander).expect("the commander builds an extractor");
    // An extractor on a site outside the scenario's spots (a replay's) pays what the scenario's spots do on average.
    let mean_spot = sc.spots.iter().map(|s| s.metal).sum::<f64>() / sc.spots.len().max(1) as f64;
    let t0 = state.t0.round();
    // Spots taken: by a standing extractor, or by one a builder is on (begun or not).
    let taken = |at: (f64, f64)| {
        state.standing.iter().any(|u| units.list[u.unit].extracts_metal > 0.0 && distance(u.site, at) < 100.0)
            || state.builders.iter().any(|b| b.job.as_ref().is_some_and(|j| units.list[j.unit].extracts_metal > 0.0 && distance(j.site, at) < 100.0))
    };
    let mut claimed: Vec<bool> = sc.spots.iter().map(|s| taken(s.at)).collect();
    let mut first_factory: Option<usize> = None;
    let (mut factories, mut constructors, mut nanos) = (0usize, 0usize, 0u32);
    let mut builders: Vec<Builder> = Vec::new();
    for (i, b) in state.builders.iter().enumerate() {
        let def = &units.list[b.unit];
        let is_factory = def.role == Role::Factory;
        let queue = match def.role {
            Role::Factory => {
                factories += 1;
                first_factory.get_or_insert(i);
                plan.factory_queue(factories - 1)
            }
            Role::Builder => {
                constructors += 1;
                plan.constructor_queue(constructors - 1)
            }
            _ => 0,
        };
        let (place, doing) = match &b.job {
            None => (b.place, Doing::Idle),
            Some(job) if job.progress > 0.0 || is_factory => (b.place, Doing::Build { unit: job.unit, site: job.site, pays: job.pays, progress: job.progress }),
            Some(job) => {
                let (left, stands) = sc.trip(b.place, job.site, def.build_distance, def.speed);
                (stands, Doing::begin(left, job.unit, job.site, job.pays))
            }
        };
        builders.push(Builder { queue, next: 0, power: def.worker_time, speed: def.speed, range: def.build_distance, place, unit: b.unit, is_factory, state: doing });
    }
    let mut base_sites = state.standing.iter().filter(|u| units.list[u.unit].extracts_metal == 0.0).count();

    let (mut metal, mut energy) = (state.metal, state.energy);
    let (mut metal_storage, mut energy_storage) = (state.metal_storage, state.energy_storage);
    let mut steady_metal: f64 = state.builders.iter().map(|b| units.list[b.unit].metal_make).sum();
    let mut steady_energy: f64 = state.builders.iter().map(|b| units.list[b.unit].energy_make).sum();
    // Extractors pay upkeep; with no energy to pay it they stand still (every recorded game's income dips when stored
    // energy reaches zero: open-cal2, 2026-09-20).
    let (mut upkeep, mut extractor_metal) = (0.0, 0.0);
    let (mut extractors, mut converters) = (0u32, 0u32);
    let mut wind_caps: Vec<f64> = Vec::new();
    let mut nano_power = 0.0;
    let (mut conv_capacity, mut conv_efficiency) = (0.0, 0.0);
    let (mut army_count, mut army_value) = (0u32, 0.0);
    let (mut metal_wasted, mut energy_wasted, mut metal_spent) = (0.0, 0.0, 0.0);
    let (mut extractor_sites, mut turret_sites): (Vec<((f64, f64), f64)>, Vec<(f64, f64)>) = (Vec::new(), Vec::new());
    for u in &state.standing {
        let def = &units.list[u.unit];
        steady_metal += def.metal_make;
        steady_energy += def.energy_make.max(0.0);
        upkeep += (-def.energy_make).max(0.0);
        if def.extracts_metal > 0.0 {
            extractors += 1;
            extractor_metal += u.pays;
            extractor_sites.push((u.site, u.pays));
        }
        if def.wind_cap > 0.0 {
            wind_caps.push(def.wind_cap);
        }
        if def.conv_capacity > 0.0 {
            converters += 1;
            conv_capacity += def.conv_capacity;
            conv_efficiency = def.conv_efficiency;
        }
        match def.role {
            Role::Nano => {
                nanos += 1;
                nano_power += def.worker_time;
            }
            Role::Turret => turret_sites.push(u.site),
            _ => {}
        }
    }

    let mut outcome = Outcome { samples: Vec::new(), finished: Vec::new(), effective: vec![Vec::new(); plan.queue_count()], army_in_progress: 0.0, exposed_income: 0.0 };
    let (mut second_metal, mut second_energy, mut second_stall, mut second_steps) = (0.0, 0.0, 0.0, 0u32);
    let steps = (seconds / sc.dt).round() as usize;

    for step in 0..steps {
        let t = t0 + step as f64 * sc.dt;

        // 1. Idle builders take their next step.
        for b in 0..builders.len() {
            while matches!(builders[b].state, Doing::Idle) {
                let builder = &builders[b];
                let default_step = Step::build(extractor);
                let next = match plan.queue(builder.queue).get(builder.next) {
                    Some(next) => next,
                    None if sc.constructors_default_to_extractors && builder.queue >= plan.constructor_queue(0) => &default_step,
                    None => {
                        builders[b].state = Doing::Done;
                        break;
                    }
                };
                let queue = builder.queue;
                builders[b].next += 1;
                let builder = &builders[b];
                match next.item {
                    Item::Assist => {
                        if builder.is_factory {
                            continue;
                        }
                        let walk = (sc.ground.walk(builder.place, sc.home) - builder.range - sc.reach_bonus).max(0.0) / builder.speed;
                        builders[b].state = Doing::Travel { left: walk, then: None };
                        outcome.effective[queue].push(*next);
                    }
                    Item::Build(unit) => {
                        let def = &units.list[unit];
                        if !units.list[builder.unit].builds.contains(&unit) {
                            continue;
                        }
                        if builder.is_factory {
                            let site = builder.place;
                            builders[b].state = Doing::begin(sc.factory_overhead, unit, site, 0.0);
                            outcome.effective[queue].push(*next);
                            continue;
                        }
                        if def.role == Role::Factory && factories_planned(&builders, units) >= plan.factories.len() {
                            continue;
                        }
                        // The leash holds for whatever the commander is sent to build.
                        if queue == 0 && next.site.is_some_and(|site| sc.commander_leash < f64::MAX && sc.ground.walk(sc.home, site) > sc.commander_leash) {
                            continue;
                        }
                        let mut pays = 0.0;
                        let site = if def.extracts_metal > 0.0 {
                            pays = mean_spot;
                            let from = next.site.unwrap_or(builder.place);
                            let leashed = queue == 0 && sc.commander_leash < f64::MAX;
                            let free = (0..sc.spots.len())
                                .filter(|i| !claimed[*i] && !(leashed && sc.ground.walk(sc.home, sc.spots[*i].at) > sc.commander_leash))
                                .min_by(|a, b| distance(sc.spots[*a].at, from).total_cmp(&distance(sc.spots[*b].at, from)));
                            match free {
                                Some(i) if next.site.is_none() || distance(sc.spots[i].at, from) < 100.0 => {
                                    claimed[i] = true;
                                    pays = sc.spots[i].metal;
                                    sc.spots[i].at
                                }
                                _ => match next.site {
                                    // The named spot is taken already, or too far for the commander.
                                    Some(site) if sc.spots.iter().any(|s| distance(s.at, site) < 100.0) => continue,
                                    None if leashed => continue,
                                    // A replayed extractor on a spot outside the scenario's list.
                                    Some(site) => site,
                                    None if builder.next > plan.queue(queue).len() => {
                                        // The default extractor found no spot: this constructor is finished.
                                        builders[b].state = Doing::Done;
                                        break;
                                    }
                                    None => continue,
                                },
                            }
                        } else {
                            // Economy buildings go up beside a builder that is about the base anyway (no walk);
                            // everything else, and everything a builder far afield is asked for, goes to the base.
                            let beside = def.role == Role::Eco && distance(builder.place, sc.home) < sc.base_radius;
                            next.site.unwrap_or_else(|| {
                                if beside {
                                    return builder.place;
                                }
                                base_sites += 1;
                                base_site(sc.home, base_sites - 1)
                            })
                        };
                        let builder = &builders[b];
                        let (left, stands) = sc.trip(builder.place, site, builder.range, builder.speed);
                        builders[b].place = stands;
                        builders[b].state = Doing::begin(left, unit, site, pays);
                        outcome.effective[queue].push(*next);
                    }
                }
            }
        }

        // 2. What every build asks for this step.
        let assisting: f64 = builders.iter().filter(|b| matches!(b.state, Doing::Assist { .. })).map(|b| b.power).sum();
        let mut asks: Vec<(usize, f64)> = Vec::new(); // (builder, progress wanted this step)
        let (mut want_metal, mut want_energy) = (0.0, 0.0);
        for (b, builder) in builders.iter().enumerate() {
            if let Doing::Build { unit, progress, .. } = builder.state {
                let def = &units.list[unit];
                let mut power = builder.power;
                if Some(b) == first_factory {
                    power += nano_power + assisting;
                }
                let gain = (power / def.build_time * sc.dt).min(1.0 - progress);
                want_metal += gain * def.metal_cost;
                want_energy += gain * def.energy_cost;
                asks.push((b, gain));
            }
        }

        // 3. Income, then pay for as much of it as the stock allows.
        let wind = sc.wind.at(t);
        let wind_energy: f64 = wind_caps.iter().map(|cap| wind.min(*cap)).sum();
        let energy_rate = steady_energy + wind_energy;
        let share = |have: f64, want: f64| if want > have { have / want } else { 1.0 };
        // Upkeep stands in the same queue for energy as the builds do: the engine gives it no priority.
        energy += energy_rate * sc.dt;
        want_energy += upkeep * sc.dt;
        let running = share(energy, want_energy);
        energy -= upkeep * sc.dt * running;
        want_energy -= upkeep * sc.dt * running;
        let mut metal_rate = steady_metal + extractor_metal * running;
        metal += metal_rate * sc.dt;
        // Each build is held back by the resources it needs only: a solar collector (no energy in its cost) goes up at
        // full speed through an energy stall, as it does in the engine.
        let (metal_share, energy_share) = (share(metal, want_metal), running.min(share(energy, want_energy)));
        let held = |unit: usize| {
            let def = &units.list[unit];
            (if def.metal_cost > 0.0 { metal_share } else { 1.0 }).min(if def.energy_cost > 0.0 { energy_share } else { 1.0 })
        };
        let mut factor = 0.0;
        for (b, gain) in &asks {
            let Doing::Build { unit, .. } = builders[*b].state else { unreachable!() };
            let def = &units.list[unit];
            metal -= gain * held(unit) * def.metal_cost;
            energy -= gain * held(unit) * def.energy_cost;
            metal_spent += gain * held(unit) * def.metal_cost;
            factor += held(unit) / asks.len() as f64;
        }
        if asks.is_empty() {
            factor = 1.0;
        }
        (metal, energy) = (metal.max(0.0), energy.max(0.0));

        // 4. Converters take what is above the level, up to their capacity.
        let surplus = energy - sc.converter_level * energy_storage;
        if surplus > 0.0 && conv_capacity > 0.0 {
            let burned = surplus.min(conv_capacity * sc.dt);
            energy -= burned;
            metal += burned * conv_efficiency;
            metal_rate += burned * conv_efficiency / sc.dt;
        }
        if metal > metal_storage {
            metal_wasted += metal - metal_storage;
            metal = metal_storage;
        }
        if energy > energy_storage {
            energy_wasted += energy - energy_storage;
            energy = energy_storage;
        }

        // 5. Progress and completions.
        for (b, gain) in asks {
            let Doing::Build { unit, site, pays, progress } = &mut builders[b].state else { unreachable!() };
            *progress += gain * held(*unit);
            if *progress < 1.0 - 1e-9 {
                continue;
            }
            let (unit, site, pays) = (*unit, *site, *pays);
            let queue = builders[b].queue;
            builders[b].state = Doing::Idle;
            outcome.finished.push(Finished { t: t + sc.dt, unit, queue });
            let def = &units.list[unit];
            steady_metal += def.metal_make;
            steady_energy += def.energy_make.max(0.0);
            upkeep += (-def.energy_make).max(0.0);
            metal_storage += def.metal_storage;
            energy_storage += def.energy_storage;
            if def.extracts_metal > 0.0 {
                extractors += 1;
                extractor_metal += pays;
                extractor_sites.push((site, pays));
            }
            if def.wind_cap > 0.0 {
                wind_caps.push(def.wind_cap);
            }
            if def.conv_capacity > 0.0 {
                converters += 1;
                conv_capacity += def.conv_capacity;
                conv_efficiency = def.conv_efficiency;
            }
            let recruit = |queue: usize, is_factory: bool| Builder {
                queue,
                next: 0,
                power: def.worker_time,
                speed: def.speed,
                range: def.build_distance,
                place: site,
                unit,
                is_factory,
                state: Doing::Idle,
            };
            match def.role {
                Role::Factory => {
                    let queue = plan.factory_queue(factories);
                    factories += 1;
                    first_factory.get_or_insert(builders.len());
                    builders.push(recruit(queue, true));
                }
                Role::Builder => {
                    // Constructors beyond the plan's queues still exist (and cost), they just stand idle.
                    let queue = plan.constructor_queue(constructors);
                    constructors += 1;
                    if queue < plan.queue_count() {
                        builders.push(recruit(queue, false));
                    }
                }
                Role::Nano => {
                    nanos += 1;
                    nano_power += def.worker_time;
                }
                Role::Army => {
                    army_count += 1;
                    army_value += def.metal_cost;
                }
                Role::Turret => turret_sites.push(site),
                Role::Commander | Role::Eco => {}
            }
        }

        // 6. Clocks of walking and assisting builders.
        for builder in &mut builders {
            match &mut builder.state {
                Doing::Travel { left, then } => {
                    *left -= sc.dt;
                    if *left <= 1e-9 {
                        builder.state = match then {
                            Some((unit, site, pays)) => Doing::Build { unit: *unit, site: *site, pays: *pays, progress: 0.0 },
                            None => Doing::Assist { left: sc.assist_chunk },
                        };
                    }
                }
                Doing::Assist { left } => {
                    *left -= sc.dt;
                    if *left <= 1e-9 {
                        builder.state = Doing::Idle;
                    }
                }
                _ => {}
            }
        }

        // 7. One sample per whole second.
        second_metal += metal_rate;
        second_energy += energy_rate;
        second_stall += factor;
        second_steps += 1;
        let now = t + sc.dt;
        if (now - now.round()).abs() < 1e-6 {
            let n = second_steps as f64;
            outcome.samples.push(Sample {
                t: now.round(),
                metal,
                energy,
                metal_income: second_metal / n,
                energy_income: second_energy / n,
                extractors,
                converters,
                build_power: builders.iter().map(|b| b.power).sum::<f64>() + nano_power,
                factories: factories as u32,
                constructors: constructors as u32,
                nanos,
                army_count,
                army_value,
                stall: second_stall / n,
                metal_wasted,
                energy_wasted,
                metal_spent,
            });
            (second_metal, second_energy, second_stall, second_steps) = (0.0, 0.0, 0.0, 0);
        }
    }

    outcome.exposed_income = extractor_sites
        .iter()
        .filter(|(at, _)| distance(*at, sc.home) > sc.exposed_beyond && !turret_sites.iter().any(|t| distance(*t, *at) < sc.turret_cover))
        .map(|(_, pays)| pays)
        .sum();
    outcome.army_in_progress = builders
        .iter()
        .filter_map(|b| match b.state {
            Doing::Build { unit, progress, .. } if units.list[unit].role == Role::Army => Some(progress * units.list[unit].metal_cost),
            _ => None,
        })
        .sum();
    outcome
}

/// Factories standing or under way, so a plan cannot start more than it has queues for.
fn factories_planned(builders: &[Builder], units: &Units) -> usize {
    builders
        .iter()
        .filter(|b| {
            b.is_factory
                || matches!(b.state, Doing::Travel { then: Some((unit, _, _)), .. } | Doing::Build { unit, .. } if units.list[unit].role == Role::Factory)
        })
        .count()
}
