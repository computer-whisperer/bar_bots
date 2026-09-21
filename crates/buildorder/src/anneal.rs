//! Simulated annealing over `Plan`s. Deterministic for a given seed.

use crate::plan::{Item, Plan, QueueKind, Step};
use crate::sim::{simulate, Outcome, Scenario, State};
use crate::units::{Role, Unit, Units};

/// splitmix64: small, seedable, good enough for annealing.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn bits(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.bits() % n.max(1) as u64) as usize
    }

    pub fn unit(&mut self) -> f64 {
        (self.bits() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Objective {
    /// Metal income (commander, extractors, converters), mean of the last 30 s before the horizon.
    Income,
    /// Metal cost of all combat units finished by the horizon.
    Army,
    /// Army value plus `MIX_INCOME_SECONDS` of the final metal income.
    Mix,
    /// The opening's measure (`docs/design/2026-09-20-opening-search.md`), in metal: all metal made over the horizon,
    /// plus `TEMPO_INCOME_SECONDS` of the income it ends on (what stands at the horizon goes on paying), plus the
    /// army built times `army` per metal, less `TEMPO_STALL_METAL` for every builder-second lost to a stall.
    /// Income from exposed extractors (`Outcome::exposed_income`) counts `exposed` of its worth at the horizon: in
    /// open-search-2-ab an opening with no turret held 8.8 extractors at minute 6 and 5.8 at minute 10. With a
    /// `contact`, a soldier that can stand at the opponent's base by the first-contact time is worth `contact.weight`
    /// of its metal on top (`docs/design/2026-09-20-rush-benchmark.md`).
    Tempo { army: f64, exposed: f64, contact: Option<Contact> },
    /// A short horizon with expectations at it (the user, 2026-09-21: "try it with 3 minutes and use the utility
    /// function to incentivise expected extractor numbers etc."): metal made over the horizon, plus
    /// `TEMPO_INCOME_SECONDS` of the income it ends on (exposed as in `Tempo`), plus what stands at the horizon
    /// against what is expected of the clock (`Expectations`): each extractor up to the expected count is worth
    /// `EXPECTED_EXTRACTOR`, beyond it `EXTRA_EXTRACTOR`; each constructor `EXPECTED_CONSTRUCTOR` / `EXTRA_CONSTRUCTOR`;
    /// army metal up to the expected `EXPECTED_ARMY` times, beyond `EXTRA_ARMY`; the contact term as in `Tempo`;
    /// less the stall penalty.
    Expect { exposed: f64, contact: Option<Contact>, expect: Expectations },
}

/// What a game is expected to have by each second of the clock: a piecewise-linear curve per quantity. The standard
/// curves (`Expectations::STANDARD`) follow the experienced players' openings (K-open-early-pawn-pressure-is-standard:
/// 4 / 6 extractors beside 7 / 15 soldiers at 3 / 5 min), the searched openings (5 / 8 / 11 extractors at 3 / 4 / 5
/// min, open-search-1-ab) and commander game 5 (9 at 6:00, 11 at 12:00, 19 at 33:00), and are the first thing the
/// commander's requirements will set (`docs/design/2026-09-21-rolling-planner.md`, step 3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Expectations {
    pub extractors: &'static [(f64, f64)],
    pub constructors: &'static [(f64, f64)],
    /// Metal cost of the soldiers built so far.
    pub army_metal: &'static [(f64, f64)],
}

impl Expectations {
    pub const STANDARD: Expectations = Expectations {
        extractors: &[(0.0, 0.0), (60.0, 2.0), (120.0, 4.0), (180.0, 6.0), (240.0, 8.0), (300.0, 10.0), (420.0, 12.0), (600.0, 15.0), (900.0, 18.0), (1500.0, 22.0)],
        constructors: &[(0.0, 0.0), (90.0, 1.0), (150.0, 2.0), (240.0, 3.0), (360.0, 4.0), (600.0, 5.0), (900.0, 6.0)],
        army_metal: &[(0.0, 0.0), (120.0, 200.0), (180.0, 650.0), (300.0, 1400.0), (420.0, 2100.0), (600.0, 3200.0), (900.0, 5200.0), (1500.0, 9000.0)],
    };

    /// The curve's value at `t`, held flat beyond its ends.
    pub fn at(curve: &[(f64, f64)], t: f64) -> f64 {
        let Some(first) = curve.first() else { return 0.0 };
        if t <= first.0 {
            return first.1;
        }
        for pair in curve.windows(2) {
            let ((t0, v0), (t1, v1)) = (pair[0], pair[1]);
            if t <= t1 {
                return v0 + (v1 - v0) * (t - t0) / (t1 - t0).max(1e-9);
            }
        }
        curve.last().map_or(0.0, |l| l.1)
    }
}

/// Worth, in metal, of what stands at the horizon against the expectation (`Objective::Expect`).
pub const EXPECTED_EXTRACTOR: f64 = 400.0;
pub const EXTRA_EXTRACTOR: f64 = 120.0;
pub const EXPECTED_CONSTRUCTOR: f64 = 250.0;
/// A constructor beyond the expectation costs its metal: rewarded at 40 the planner kept nine at 5:00 and twenty at
/// 15:00 (planner-smoke-4), each worth its simulated extractors on spots the game did not have.
pub const EXTRA_CONSTRUCTOR: f64 = -120.0;
pub const EXPECTED_ARMY: f64 = 2.0;
pub const EXTRA_ARMY: f64 = 0.5;

/// Worth of `have` against `expected`: full up to the expectation, `extra` per unit beyond it.
fn against(have: f64, expected: f64, full: f64, extra: f64) -> f64 {
    full * have.min(expected) + extra * (have - expected).max(0.0)
}

/// When the first fight is, and how far away: experienced players' raiders are at the opponent's base before 2:30
/// (K-open-early-pawn-pressure-is-standard). A soldier finished at t with speed v stands there at t + walk / v.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// Seconds into the game.
    pub at: f64,
    /// Elmos from home to the opponent's base, on foot.
    pub walk: f64,
    /// Worth of a soldier there in time, per metal, on top of the plain army weight.
    pub weight: f64,
    /// Seconds over which that worth falls to nothing for a soldier that arrives late.
    pub window: f64,
}

impl Contact {
    /// The share of `weight` a soldier of this speed, finished at `t`, earns: whole when there by the contact time,
    /// falling to nothing over the window (long: the players' Pawns keep coming to minute 5), times the square of the
    /// unit's speed over a raider's for anything slower (a Rocketeer at the opponent's base is not the pressure a Pawn
    /// is; with metal alone the search led with Rocketeers, rush-2-ab).
    pub fn presence(self, t: f64, speed: f64) -> f64 {
        let arrives = t + self.walk / speed.max(1.0);
        (1.0 - (arrives - self.at) / self.window).clamp(0.0, 1.0) * (speed / RAIDER_SPEED).min(1.0).powi(2)
    }
}

pub const TEMPO_INCOME_SECONDS: f64 = 90.0;
/// A tier-1 raider's speed (Pawn 87, Grunt 81), the pace the first contact is made at.
pub const RAIDER_SPEED: f64 = 80.0;
pub const TEMPO_STALL_METAL: f64 = 3.0;

/// In `Objective::Mix` one metal/s of income at the horizon counts as this much army metal.
pub const MIX_INCOME_SECONDS: f64 = 120.0;

impl Objective {
    pub fn parse(name: &str) -> Option<Objective> {
        match name {
            "income" => Some(Objective::Income),
            "army" => Some(Objective::Army),
            "mix" => Some(Objective::Mix),
            "tempo" => Some(Objective::Tempo { army: 1.0, exposed: 0.3, contact: None }),
            "expect" => Some(Objective::Expect { exposed: 0.3, contact: None, expect: Expectations::STANDARD }),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Objective::Income => "income",
            Objective::Army => "army",
            Objective::Mix => "mix",
            Objective::Tempo { .. } => "tempo",
            Objective::Expect { .. } => "expect",
        }
    }

    /// Higher is better. Small shaping terms (metal put to use, half-built soldiers) give the search a slope on
    /// plateaus; they are three orders of magnitude below the terms that matter.
    pub fn score(self, units: &Units, outcome: &Outcome, horizon: f64) -> f64 {
        let income = outcome.mean_metal_income(horizon, 30.0);
        let army = outcome.last().army_value + 0.5 * outcome.army_in_progress;
        let shaping = 1e-3 * outcome.last().metal_spent;
        match self {
            Objective::Income => income + 1e-3 * shaping,
            Objective::Army => army + shaping,
            Objective::Mix => army + MIX_INCOME_SECONDS * income + shaping,
            Objective::Tempo { army: weight, exposed, contact } => {
                let made: f64 = outcome.samples.iter().map(|s| s.metal_income).sum();
                let stalled: f64 = outcome.samples.iter().map(|s| 1.0 - s.stall).sum();
                let there = Self::there(units, outcome, contact);
                made + TEMPO_INCOME_SECONDS * (income - (1.0 - exposed) * outcome.exposed_income) + weight * army + there - TEMPO_STALL_METAL * stalled + shaping
            }
            Objective::Expect { exposed, contact, expect } => {
                let made: f64 = outcome.samples.iter().map(|s| s.metal_income).sum();
                let stalled: f64 = outcome.samples.iter().map(|s| 1.0 - s.stall).sum();
                let last = outcome.last();
                let extractors = against(last.extractors as f64, Expectations::at(expect.extractors, horizon), EXPECTED_EXTRACTOR, EXTRA_EXTRACTOR);
                let constructors = against(last.constructors as f64, Expectations::at(expect.constructors, horizon), EXPECTED_CONSTRUCTOR, EXTRA_CONSTRUCTOR);
                let soldiers = against(army, Expectations::at(expect.army_metal, horizon), EXPECTED_ARMY, EXTRA_ARMY);
                made + TEMPO_INCOME_SECONDS * (income - (1.0 - exposed) * outcome.exposed_income) + extractors + constructors + soldiers + Self::there(units, outcome, contact) - TEMPO_STALL_METAL * stalled + shaping
            }
        }
    }
}

impl Objective {
    /// The contact term: soldiers that can stand at the opponent's base by the contact time.
    fn there(units: &Units, outcome: &Outcome, contact: Option<Contact>) -> f64 {
        contact.map_or(0.0, |contact| {
            let soldiers = outcome.finished.iter().map(|f| &units.list[f.unit]).zip(outcome.finished.iter()).filter(|(u, _)| u.role == Role::Army);
            soldiers.map(|(u, f)| contact.weight * u.metal_cost * contact.presence(f.t, u.speed)).sum()
        })
    }
}

/// What the search may put in a queue.
#[derive(Clone)]
pub struct Palette {
    pub mobile: Vec<Item>,
    pub factory: Vec<Item>,
    pub constructor: usize,
    pub factory_unit: usize,
    /// The turrets on offer: the search may stand one at a metal spot.
    pub turrets: Vec<usize>,
}

/// Not offered to the search: anti-air (worthless without aircraft) and the scouts, which are not fighters and would
/// only pad the army count.
const NOT_FIGHTERS: [&str; 7] = ["jeth", "crash", "sam", "mist", "flea", "armfav", "corfav"];

impl Palette {
    /// `commander`, `factory`: unit types. `turret`: the ground turret on offer, by the caller's word (the numbers do
    /// not tell an anti-air launcher from a laser tower). `nanos`: whether construction turrets are on offer. What goes on offer is
    /// chosen by what a unit does, not by its name: the cheapest extractor, wind generator, steady generator, converter
    /// and energy storage in the commander's or the constructor's menu.
    pub fn new(units: &Units, commander: usize, factory: usize, nanos: bool, turret: Option<usize>) -> Palette {
        let constructor = units
            .cheapest(factory, |u| u.role == Role::Builder && u.builds.contains(&factory))
            .unwrap_or_else(|| panic!("{} builds no constructor", units.list[factory].name));
        let offer = |test: &dyn Fn(&Unit) -> bool| units.cheapest(commander, test).or_else(|| units.cheapest(constructor, test));
        let plain = |u: &Unit| u.role == Role::Eco && u.extracts_metal == 0.0 && u.conv_capacity == 0.0;
        let mut mobile: Vec<Item> = [
            offer(&|u| u.extracts_metal > 0.0),
            offer(&|u| u.wind_cap > 0.0),
            offer(&|u| plain(u) && u.wind_cap == 0.0 && u.energy_make > 0.0),
            offer(&|u| u.conv_capacity > 0.0),
            offer(&|u| plain(u) && u.energy_storage >= 1000.0),
            // Eyes: both experienced players' constructors put a radar up by minute 3 (K-open-early-pawn-pressure-is-standard).
            offer(&|u| plain(u) && u.radar_range > 0.0 && u.energy_make <= 0.0),
            Some(factory),
            offer(&|u| nanos && u.role == Role::Nano),
            turret,
        ]
        .into_iter()
        .flatten()
        .map(Item::Build)
        .collect();
        assert!(mobile.len() >= 3, "the commander of this game builds neither an extractor nor a wind generator");
        mobile.push(Item::Assist);
        let mut from_factory = vec![Item::Build(constructor)];
        for unit in &units.list[factory].builds {
            let def = &units.list[*unit];
            // Soldiers, and the mobile builders without a menu (resurrection bots): the players' labs make those too.
            let mobile_eco = def.role == Role::Eco && def.speed > 0.0;
            if (def.role == Role::Army || mobile_eco) && !NOT_FIGHTERS.iter().any(|s| def.name.ends_with(s)) {
                from_factory.push(Item::Build(*unit));
            }
        }
        let turrets = turret.into_iter().collect();
        Palette { mobile, factory: from_factory, constructor, factory_unit: factory, turrets }
    }

    fn pick(&self, kind: QueueKind, rng: &mut Rng) -> Step {
        let list = if kind == QueueKind::Factory { &self.factory } else { &self.mobile };
        Step { item: list[rng.below(list.len())], site: None }
    }

    /// A plain opening to start the search from.
    pub fn seed_plan(&self, factories: usize, constructors: usize) -> Plan {
        let mut plan = Plan::empty(factories, constructors);
        let (mex, wind) = (self.mobile[0], self.mobile[1]);
        let factory = Item::Build(self.factory_unit);
        for item in [mex, mex, wind, wind, factory, wind, wind, mex] {
            plan.commander.push(Step { item, site: None });
        }
        plan.factories[0].push(Step::build(self.constructor));
        let soldier = *self.factory.get(1).unwrap_or(&self.factory[0]);
        plan.factories[0].extend(std::iter::repeat_n(Step { item: soldier, site: None }, 4));
        plan.constructors[0].extend(std::iter::repeat_n(Step { item: mex, site: None }, 3));
        plan
    }
}

/// `plan` must be an effective plan (`Outcome::effective`): every step in it was reached, so every position matters.
/// `live[q]`: whether queue `q` has a builder yet; writing into a queue nobody will read is a wasted move.
fn mutate(plan: &mut Plan, live: &[usize], palette: &Palette, spots: &[(f64, f64)], rng: &mut Rng) {
    let reach = |plan: &Plan, q: usize| plan.queue(q).len();
    for _ in 0..20 {
        let q = live[rng.below(live.len())];
        let kind = plan.kind(q);
        let len = reach(plan, q);
        match rng.below(6) {
            0 if len >= 2 => {
                let (a, b) = (rng.below(len), rng.below(len));
                if plan.queue(q)[a] == plan.queue(q)[b] {
                    continue;
                }
                plan.queue_mut(q).swap(a, b);
            }
            1 => {
                let at = rng.below(len + 1).min(plan.queue(q).len());
                let step = palette.pick(kind, rng);
                plan.queue_mut(q).insert(at, step);
            }
            2 if len >= 1 => {
                plan.queue_mut(q).remove(rng.below(len));
            }
            3 if len >= 1 => {
                let at = rng.below(len);
                let step = palette.pick(kind, rng);
                if plan.queue(q)[at] == step {
                    continue;
                }
                plan.queue_mut(q)[at] = step;
            }
            4 if len >= 1 => {
                // Move a step to another queue of a builder that can do the same work.
                let mobile = |k: QueueKind| k != QueueKind::Factory;
                let others: Vec<usize> = live.iter().copied().filter(|o| *o != q && mobile(plan.kind(*o)) == mobile(kind)).collect();
                if others.is_empty() {
                    continue;
                }
                let to = others[rng.below(others.len())];
                let step = plan.queue_mut(q).remove(rng.below(len));
                let at = rng.below(reach(plan, to) + 1).min(plan.queue(to).len());
                plan.queue_mut(to).insert(at, step);
            }
            5 if len >= 1 && kind != QueueKind::Factory && !spots.is_empty() => {
                // Name the spot an extractor goes to (or leave it to "the nearest free one" again): how the search
                // sends a builder on a walk the greedy choice would not take.
                let at = rng.below(len);
                let Item::Build(unit) = plan.queue(q)[at].item else { continue };
                if plan.queue(q)[at].item != palette.mobile[0] && !palette.turrets.contains(&unit) {
                    continue;
                }
                let site = (rng.below(4) > 0).then(|| spots[rng.below(spots.len())]);
                if plan.queue(q)[at].site == site {
                    continue;
                }
                plan.queue_mut(q)[at].site = site;
            }
            _ => continue,
        }
        return;
    }
}

#[derive(Clone)]
pub struct Search {
    pub objective: Objective,
    /// Seconds.
    pub horizon: f64,
    pub iterations: usize,
    pub seed: u64,
    pub factories: usize,
    pub constructors: usize,
    /// Starting temperature as a share of the score; it falls geometrically to a hundredth of this.
    pub hot: f64,
    /// Where to start from; the palette's plain opening when there is none. The answer is never worse than this.
    pub start: Option<Plan>,
}

pub struct Found {
    pub plan: Plan,
    pub score: f64,
    pub outcome: Outcome,
}

/// The best plan found from `state` (`docs/design/2026-09-21-rolling-planner.md`): the plan's queues follow the
/// state's builders, and the search writes at least as many factory and constructor queues as the state has.
pub fn anneal(units: &Units, scenario: &Scenario, state: &State, palette: &Palette, search: &Search) -> Found {
    let mut rng = Rng::new(search.seed);
    let end = state.t0 + search.horizon;
    let evaluate = |plan: &Plan| {
        let outcome = simulate(units, scenario, state, plan, search.horizon);
        (search.objective.score(units, &outcome, end), outcome)
    };
    let (factories, constructors) = state.plan_shape(units, search.factories, search.constructors);
    let mut current = search.start.clone().unwrap_or_else(|| palette.seed_plan(factories, constructors));
    current.factories.resize(factories, Vec::new());
    current.constructors.resize(constructors, Vec::new());
    let spots: Vec<(f64, f64)> = scenario.spots.iter().map(|s| s.at).collect();
    let current_queue_count = current.queue_count();
    let (mut current_score, outcome) = evaluate(&current);
    let effective = |outcome: &Outcome| Plan {
        commander: outcome.effective[0].clone(),
        factories: outcome.effective[1..=factories].to_vec(),
        constructors: outcome.effective[1 + factories..].to_vec(),
    };
    // Queues whose builder exists by the horizon, plus the next constructor's.
    let live = |outcome: &Outcome| -> Vec<usize> {
        let last = outcome.last();
        let (standing_factories, standing_constructors) = (last.factories as usize, last.constructors as usize);
        (0..current_queue_count).filter(|q| *q == 0 || (*q <= factories && *q <= standing_factories.max(1)) || (*q > factories && *q - factories <= standing_constructors + 1)).collect()
    };
    let mut alive = live(&outcome);
    current = effective(&outcome);
    let mut best = Found { plan: current.clone(), score: current_score, outcome };
    // Temperature as a share of the best score so far, so one schedule serves objectives of any scale.
    let (hot, cold) = (search.hot, search.hot * 1e-2);
    for i in 0..search.iterations {
        let temperature = best.score.abs().max(1.0) * hot * (cold / hot).powf(i as f64 / search.iterations as f64);
        let mut candidate = current.clone();
        mutate(&mut candidate, &alive, palette, &spots, &mut rng);
        let (score, outcome) = evaluate(&candidate);
        if score >= current_score || rng.unit() < ((score - current_score) / temperature).exp() {
            // Continue from what the builders actually did: no skipped steps, no unreached tail, default extractors
            // written out. Same score, and every later mutation lands on a step that matters.
            alive = live(&outcome);
            current = effective(&outcome);
            current_score = score;
            if score > best.score {
                best = Found { plan: current.clone(), score, outcome };
            }
        }
    }
    best
}

/// Independent restarts on separate threads (seeds `seed`, `seed + 1`, ...); the best one wins, ties to the lower seed.
pub fn anneal_restarts(units: &Units, scenario: &Scenario, state: &State, palette: &Palette, search: &Search, restarts: usize) -> Found {
    let mut found: Vec<Found> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..restarts)
            .map(|r| {
                let one = Search { seed: search.seed + r as u64, ..search.clone() };
                scope.spawn(move || anneal(units, scenario, state, palette, &one))
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("annealing thread")).collect()
    });
    let mut best = found.remove(0);
    for other in found {
        if other.score > best.score {
            best = other;
        }
    }
    best
}

/// The best plan found in about `budget` of wall time on `threads` threads: the iteration count is set from how long
/// this scenario's simulations take on this machine.
pub fn anneal_within(units: &Units, scenario: &Scenario, state: &State, palette: &Palette, search: &Search, budget: std::time::Duration, threads: usize) -> Found {
    let started = std::time::Instant::now();
    let mut best = anneal(units, scenario, state, palette, &Search { iterations: 30, ..search.clone() });
    // Wall time an iteration costs, measured again after every round: the probe's figure carries the first round's
    // set-up, and a budget spent by it alone used a third of itself (rush-budget-500: 320 ms of 500).
    let mut per_iteration = started.elapsed().as_secs_f64() / 31.0;
    for round in 1.. {
        let left = budget.as_secs_f64() - started.elapsed().as_secs_f64();
        let iterations = (left / per_iteration.max(1e-6)) as usize;
        if iterations < 100 {
            return best;
        }
        let began = std::time::Instant::now();
        let again = Search { iterations, seed: search.seed + round, start: Some(best.plan.clone()), ..search.clone() };
        let found = anneal_restarts(units, scenario, state, palette, &again, threads.max(1));
        per_iteration = began.elapsed().as_secs_f64() / iterations as f64;
        if found.score > best.score {
            best = found;
        }
    }
    best
}
