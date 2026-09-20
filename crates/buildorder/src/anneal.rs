//! Simulated annealing over `Plan`s. Deterministic for a given seed.

use crate::plan::{Item, Plan, QueueKind, Step};
use crate::sim::{simulate, Outcome, Scenario};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Objective {
    /// Metal income (commander, extractors, converters), mean of the last 30 s before the horizon.
    Income,
    /// Metal cost of all combat units finished by the horizon.
    Army,
    /// Army value plus `MIX_INCOME_SECONDS` of the final metal income.
    Mix,
}

/// In `Objective::Mix` one metal/s of income at the horizon counts as this much army metal.
pub const MIX_INCOME_SECONDS: f64 = 120.0;

impl Objective {
    pub fn parse(name: &str) -> Option<Objective> {
        match name {
            "income" => Some(Objective::Income),
            "army" => Some(Objective::Army),
            "mix" => Some(Objective::Mix),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Objective::Income => "income",
            Objective::Army => "army",
            Objective::Mix => "mix",
        }
    }

    /// Higher is better. Small shaping terms (metal put to use, half-built soldiers) give the search a slope on
    /// plateaus; they are three orders of magnitude below the terms that matter.
    pub fn score(self, outcome: &Outcome, horizon: f64) -> f64 {
        let income = outcome.mean_metal_income(horizon, 30.0);
        let army = outcome.last().army_value + 0.5 * outcome.army_in_progress;
        let shaping = 1e-3 * outcome.last().metal_spent;
        match self {
            Objective::Income => income + 1e-3 * shaping,
            Objective::Army => army + shaping,
            Objective::Mix => army + MIX_INCOME_SECONDS * income + shaping,
        }
    }
}

/// What the search may put in a queue.
pub struct Palette {
    pub mobile: Vec<Item>,
    pub factory: Vec<Item>,
    pub constructor: usize,
    pub factory_unit: usize,
}

/// Not offered to the search: anti-air (worthless without aircraft) and the scouts, which are not fighters and would
/// only pad the army count.
const NOT_FIGHTERS: [&str; 7] = ["jeth", "crash", "sam", "mist", "flea", "armfav", "corfav"];

impl Palette {
    /// `commander`, `factory`: unit types. `nanos`: whether construction turrets are on offer. What goes on offer is
    /// chosen by what a unit does, not by its name: the cheapest extractor, wind generator, steady generator, converter
    /// and energy storage in the commander's or the constructor's menu.
    pub fn new(units: &Units, commander: usize, factory: usize, nanos: bool) -> Palette {
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
            Some(factory),
            offer(&|u| nanos && u.role == Role::Nano),
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
            if def.role == Role::Army && !NOT_FIGHTERS.iter().any(|s| def.name.ends_with(s)) {
                from_factory.push(Item::Build(*unit));
            }
        }
        Palette { mobile, factory: from_factory, constructor, factory_unit: factory }
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
fn mutate(plan: &mut Plan, live: &[usize], palette: &Palette, rng: &mut Rng) {
    let reach = |plan: &Plan, q: usize| plan.queue(q).len();
    for _ in 0..20 {
        let q = live[rng.below(live.len())];
        let kind = plan.kind(q);
        let len = reach(plan, q);
        match rng.below(5) {
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
            _ => continue,
        }
        return;
    }
}

#[derive(Clone, Copy)]
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
}

pub struct Found {
    pub plan: Plan,
    pub score: f64,
    pub outcome: Outcome,
}

pub fn anneal(units: &Units, scenario: &Scenario, palette: &Palette, search: &Search) -> Found {
    let mut rng = Rng::new(search.seed);
    let evaluate = |plan: &Plan| {
        let outcome = simulate(units, scenario, plan, search.horizon);
        (search.objective.score(&outcome, search.horizon), outcome)
    };
    let mut current = palette.seed_plan(search.factories, search.constructors);
    let current_queue_count = current.queue_count();
    let (mut current_score, outcome) = evaluate(&current);
    let effective = |outcome: &Outcome| Plan {
        commander: outcome.effective[0].clone(),
        factories: outcome.effective[1..=search.factories].to_vec(),
        constructors: outcome.effective[1 + search.factories..].to_vec(),
    };
    // Queues whose builder exists by the horizon, plus the next constructor's.
    let live = |outcome: &Outcome| -> Vec<usize> {
        let last = outcome.last();
        let (factories, constructors) = (last.factories as usize, last.constructors as usize);
        (0..current_queue_count).filter(|q| *q == 0 || (*q <= search.factories && *q <= factories.max(1)) || (*q > search.factories && *q - search.factories <= constructors + 1)).collect()
    };
    let mut alive = live(&outcome);
    current = effective(&outcome);
    let mut best = Found { plan: current.clone(), score: current_score, outcome };
    // Temperature as a share of the best score so far, so one schedule serves objectives of any scale.
    let (hot, cold) = (search.hot, search.hot * 1e-2);
    for i in 0..search.iterations {
        let temperature = best.score.abs().max(1.0) * hot * (cold / hot).powf(i as f64 / search.iterations as f64);
        let mut candidate = current.clone();
        mutate(&mut candidate, &alive, palette, &mut rng);
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
pub fn anneal_restarts(units: &Units, scenario: &Scenario, palette: &Palette, search: &Search, restarts: usize) -> Found {
    let mut found: Vec<Found> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..restarts)
            .map(|r| {
                let one = Search { seed: search.seed + r as u64, ..*search };
                scope.spawn(move || anneal(units, scenario, palette, &one))
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("annealing thread")).collect()
    });
    let scores: Vec<String> = found.iter().map(|f| format!("{:.0}", f.score)).collect();
    eprintln!("restart scores: {}", scores.join(" "));
    let mut best = found.remove(0);
    for other in found {
        if other.score > best.score {
            best = other;
        }
    }
    best
}
