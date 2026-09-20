//! The economy simulator. Fixed time step; every assumption is listed in `docs/studies/build-order.md`.
//!
//! Per step: builders without a task take the next step of their queue and walk to the site; every builder on a site
//! asks for `build power / buildtime` of its target's cost per second; if the team cannot pay, every build slows by the
//! same factor (the scarcer resource decides); converters then burn stored energy above 75 % of storage; what does not
//! fit in storage is lost.

use crate::map::distance;
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
    /// Faction prefix: `arm` or `cor`.
    pub side: String,
    pub home: (f64, f64),
    /// Metal spots an extractor without an explicit site may take.
    pub spots: Vec<(f64, f64)>,
    pub spot_metal: f64,
    pub wind: Wind,
    pub start_metal: f64,
    pub start_energy: f64,
    /// Team storage before any building adds to it.
    pub base_storage: f64,
    pub dt: f64,
    /// Walked distance = straight line times this. 1.05 and the two overheads below fit 565 builder trips in 12
    /// recorded games on Quicksilver (median time beyond the straight-line walk: 1.2-1.5 s when the site was in reach,
    /// 2.4-3.4 s when not; walks over 1000 elmos took 8 % longer than the straight line).
    pub detour: f64,
    /// Elmos added to every builder's build distance: the engine measures reach to the target's edge (its model radius),
    /// not its centre. One constant for all buildings; the table carries no radii.
    pub reach_bonus: f64,
    /// Seconds a mobile builder loses per build besides walking (order latency, opening the nano spray).
    pub mobile_overhead: f64,
    /// Further seconds lost on a walk (turning, accelerating, stopping); a walk shorter than this only doubles.
    pub walk_overhead: f64,
    /// Seconds a factory loses between units (the finished unit clearing the pad).
    pub factory_overhead: f64,
    /// Length of one `Item::Assist`.
    pub assist_chunk: f64,
    /// Stored-energy fraction above which converters run (`mmLevel`, game_energy_conversion.lua).
    pub converter_level: f64,
    /// A constructor whose queue has run out keeps taking the nearest free metal spot. On for the search (a new
    /// constructor is then worth something before the search has written it a queue), off for replays.
    pub constructors_default_to_extractors: bool,
}

impl Scenario {
    pub fn new(side: &str, home: (f64, f64), spots: Vec<(f64, f64)>) -> Scenario {
        Scenario {
            side: side.to_string(),
            home,
            spots,
            spot_metal: crate::map::SPOT_METAL,
            wind: Wind::Constant(crate::map::WIND_MEAN),
            start_metal: 1000.0,
            start_energy: 1000.0,
            base_storage: 1000.0,
            dt: 0.5,
            detour: 1.05,
            reach_bonus: 40.0,
            mobile_overhead: 1.5,
            walk_overhead: 1.5,
            factory_overhead: 1.0,
            assist_chunk: 20.0,
            converter_level: 0.75,
            constructors_default_to_extractors: false,
        }
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

enum State {
    Idle,
    Travel { left: f64, then: Option<(usize, (f64, f64))> },
    Build { unit: usize, site: (f64, f64), progress: f64 },
    Assist { left: f64 },
    Done,
}

impl State {
    /// On the way to a build, or already building when there is no way to go.
    fn begin(left: f64, unit: usize, site: (f64, f64)) -> State {
        if left > 1e-9 { State::Travel { left, then: Some((unit, site)) } } else { State::Build { unit, site, progress: 0.0 } }
    }
}

struct Builder {
    queue: usize,
    next: usize,
    power: f64,
    speed: f64,
    range: f64,
    place: (f64, f64),
    /// For factories: the unit index of the factory itself. `None` for mobile builders.
    factory: Option<usize>,
    state: State,
}

/// Where the n-th building without an explicit site goes: a golden-angle spiral around home, so that base buildings
/// cost a little walking, as they do in the recorded games (consecutive wind turbines start 1.5-4.5 s apart).
fn base_site(home: (f64, f64), n: usize) -> (f64, f64) {
    let radius = 120.0 + 28.0 * n as f64;
    let angle = 2.4 * n as f64;
    (home.0 + radius * angle.cos(), home.1 + radius * angle.sin())
}

pub fn simulate(units: &Units, scenario: &Scenario, plan: &Plan, seconds: f64) -> Outcome {
    let sc = scenario;
    let commander = units.get(&format!("{}com", sc.side));
    let extractor = units.index(&format!("{}mex", sc.side)).expect("extractor in the unit table");
    let mut builders = vec![Builder {
        queue: 0,
        next: 0,
        power: commander.worker_time,
        speed: commander.speed,
        range: commander.build_distance,
        place: sc.home,
        factory: None,
        state: State::Idle,
    }];
    let mut first_factory: Option<usize> = None;
    let (mut factories, mut constructors, mut nanos) = (0usize, 0usize, 0u32);
    let mut nano_power = 0.0;
    let mut claimed = vec![false; sc.spots.len()];
    let mut base_sites = 0usize;

    let (mut metal, mut energy) = (sc.start_metal, sc.start_energy);
    let (mut metal_storage, mut energy_storage) = (sc.base_storage, sc.base_storage);
    let mut steady_metal = commander.metal_make;
    let mut steady_energy = commander.energy_make;
    let mut upkeep = 0.0;
    let (mut extractors, mut converters) = (0u32, 0u32);
    let mut wind_caps: Vec<f64> = Vec::new();
    let (mut conv_capacity, mut conv_efficiency) = (0.0, 0.0);
    let (mut army_count, mut army_value) = (0u32, 0.0);
    let (mut metal_wasted, mut energy_wasted, mut metal_spent) = (0.0, 0.0, 0.0);

    let mut outcome = Outcome { samples: Vec::new(), finished: Vec::new(), effective: vec![Vec::new(); plan.queue_count()], army_in_progress: 0.0 };
    let (mut second_metal, mut second_energy, mut second_stall, mut second_steps) = (0.0, 0.0, 0.0, 0u32);
    let steps = (seconds / sc.dt).round() as usize;

    for step in 0..steps {
        let t = step as f64 * sc.dt;

        // 1. Idle builders take their next step.
        for b in 0..builders.len() {
            while matches!(builders[b].state, State::Idle) {
                let builder = &builders[b];
                let default_step = Step::build(extractor);
                let next = match plan.queue(builder.queue).get(builder.next) {
                    Some(next) => next,
                    None if sc.constructors_default_to_extractors && builder.queue >= plan.constructor_queue(0) => &default_step,
                    None => {
                        builders[b].state = State::Done;
                        break;
                    }
                };
                let queue = builder.queue;
                builders[b].next += 1;
                let builder = &builders[b];
                match next.item {
                    Item::Assist => {
                        if builder.factory.is_some() {
                            continue;
                        }
                        let walk = (distance(builder.place, sc.home) * sc.detour - builder.range - sc.reach_bonus).max(0.0) / builder.speed;
                        builders[b].state = State::Travel { left: walk, then: None };
                        outcome.effective[queue].push(*next);
                    }
                    Item::Build(unit) => {
                        let def = &units.list[unit];
                        if let Some(factory) = builder.factory {
                            if def.factory.is_empty() || !units.list[factory].name.ends_with(&def.factory) {
                                continue;
                            }
                            let site = builder.place;
                            builders[b].state = State::begin(sc.factory_overhead, unit, site);
                            outcome.effective[queue].push(*next);
                            continue;
                        }
                        let surplus_factory = def.role == Role::Factory && factories_planned(&builders, units) >= plan.factories.len();
                        if !def.factory.is_empty() || def.role == Role::Commander || surplus_factory {
                            continue;
                        }
                        let site = if def.extracts_metal > 0.0 {
                            let from = next.site.unwrap_or(builder.place);
                            let free = (0..sc.spots.len())
                                .filter(|i| !claimed[*i])
                                .min_by(|a, b| distance(sc.spots[*a], from).total_cmp(&distance(sc.spots[*b], from)));
                            match free {
                                Some(i) if next.site.is_none() || distance(sc.spots[i], from) < 100.0 => {
                                    claimed[i] = true;
                                    sc.spots[i]
                                }
                                _ => match next.site {
                                    // A replayed extractor on a spot outside the scenario's list.
                                    Some(site) => site,
                                    None if builder.next > plan.queue(queue).len() => {
                                        // The default extractor found no spot: this constructor is finished.
                                        builders[b].state = State::Done;
                                        break;
                                    }
                                    None => continue,
                                },
                            }
                        } else {
                            next.site.unwrap_or_else(|| {
                                base_sites += 1;
                                base_site(sc.home, base_sites - 1)
                            })
                        };
                        let builder = &builders[b];
                        let gap = distance(builder.place, site);
                        let reach = builder.range + sc.reach_bonus;
                        let walked = (gap * sc.detour - reach).max(0.0);
                        if gap > reach {
                            let keep = reach / gap;
                            builders[b].place = (site.0 + (builder.place.0 - site.0) * keep, site.1 + (builder.place.1 - site.1) * keep);
                        }
                        let walk = walked / builders[b].speed;
                        let left = walk + walk.min(sc.walk_overhead) + sc.mobile_overhead;
                        builders[b].state = State::begin(left, unit, site);
                        outcome.effective[queue].push(*next);
                    }
                }
            }
        }

        // 2. What every build asks for this step.
        let assisting: f64 = builders.iter().filter(|b| matches!(b.state, State::Assist { .. })).map(|b| b.power).sum();
        let mut asks: Vec<(usize, f64)> = Vec::new(); // (builder, progress wanted this step)
        let (mut want_metal, mut want_energy) = (0.0, 0.0);
        for (b, builder) in builders.iter().enumerate() {
            if let State::Build { unit, progress, .. } = builder.state {
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
        let mut metal_rate = steady_metal;
        metal += steady_metal * sc.dt;
        energy = (energy + (energy_rate - upkeep) * sc.dt).max(0.0);
        let share = |have: f64, want: f64| if want > have { have / want } else { 1.0 };
        let factor = share(metal, want_metal).min(share(energy, want_energy));
        metal -= want_metal * factor;
        energy -= want_energy * factor;
        metal_spent += want_metal * factor;

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
            let State::Build { unit, site, progress } = &mut builders[b].state else { unreachable!() };
            *progress += gain * factor;
            if *progress < 1.0 - 1e-9 {
                continue;
            }
            let (unit, site) = (*unit, *site);
            let queue = builders[b].queue;
            builders[b].state = State::Idle;
            outcome.finished.push(Finished { t: t + sc.dt, unit, queue });
            let def = &units.list[unit];
            steady_metal += def.metal_make;
            steady_energy += def.energy_make.max(0.0);
            upkeep += (-def.energy_make).max(0.0);
            metal_storage += def.metal_storage;
            energy_storage += def.energy_storage;
            if def.extracts_metal > 0.0 {
                extractors += 1;
                steady_metal += sc.spot_metal;
            }
            if def.wind_cap > 0.0 {
                wind_caps.push(def.wind_cap);
            }
            if def.conv_capacity > 0.0 {
                converters += 1;
                conv_capacity += def.conv_capacity;
                conv_efficiency = def.conv_efficiency;
            }
            let recruit = |queue: usize, factory: Option<usize>| Builder {
                queue,
                next: 0,
                power: def.worker_time,
                speed: def.speed,
                range: def.build_distance,
                place: site,
                factory,
                state: State::Idle,
            };
            match def.role {
                Role::Factory => {
                    let queue = plan.factory_queue(factories);
                    factories += 1;
                    first_factory.get_or_insert(builders.len());
                    builders.push(recruit(queue, Some(unit)));
                }
                Role::Builder => {
                    // Constructors beyond the plan's queues still exist (and cost), they just stand idle.
                    let queue = plan.constructor_queue(constructors);
                    constructors += 1;
                    if queue < plan.queue_count() {
                        builders.push(recruit(queue, None));
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
                Role::Commander | Role::Eco | Role::Turret => {}
            }
        }

        // 6. Clocks of walking and assisting builders.
        for builder in &mut builders {
            match &mut builder.state {
                State::Travel { left, then } => {
                    *left -= sc.dt;
                    if *left <= 1e-9 {
                        builder.state = match then {
                            Some((unit, site)) => State::Build { unit: *unit, site: *site, progress: 0.0 },
                            None => State::Assist { left: sc.assist_chunk },
                        };
                    }
                }
                State::Assist { left } => {
                    *left -= sc.dt;
                    if *left <= 1e-9 {
                        builder.state = State::Idle;
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

    outcome.army_in_progress = builders
        .iter()
        .filter_map(|b| match b.state {
            State::Build { unit, progress, .. } if units.list[unit].role == Role::Army => Some(progress * units.list[unit].metal_cost),
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
            b.factory.is_some()
                || matches!(b.state, State::Travel { then: Some((unit, _)), .. } | State::Build { unit, .. } if units.list[unit].role == Role::Factory)
        })
        .count()
}
