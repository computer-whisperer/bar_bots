//! H-OPEN-PLAN: the opening as a plan (`buildorder::plan::Plan`: one queue for the commander, one per factory, one per
//! constructor) that builders work through in place of the opening rules, each taking its next step when it falls
//! idle. A builder whose queue has run out goes back to the ordinary rules, and so does everybody the moment reality
//! leaves the plan (`docs/design/2026-09-20-opening-search.md`).

use std::collections::HashMap;

use bot_protocol::{OwnUnit, Tick, UnitDefId, UnitId, Vec3};
use buildorder::anneal::{anneal_within, Objective, Palette, Search};
use buildorder::game::{distance, Game, Spot};
use buildorder::plan::{Item, Plan, Step};
use buildorder::sim::simulate;
use buildorder::units::Units;

use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};

/// The plan is for the first minutes; after this the ordinary rules have everything.
const HORIZON_FRAMES: i32 = 5 * 60 * FRAMES_PER_SECOND;
/// An enemy soldier this close to home ends the plan: the rules know how to answer a raid and the plan does not.
const RAID_RADIUS: f32 = 1000.0;
/// A plan step whose builder is idle again this soon never started (the engine drops orders now and then): once more.
const RETRY_FRAMES: i32 = 3 * FRAMES_PER_SECOND;
const RETRIES: u32 = 2;
/// The search's wall-time budget and threads: spent once, in the opening seconds in which the engine takes no orders.
const SEARCH_BUDGET: std::time::Duration = std::time::Duration::from_millis(500);
const SEARCH_THREADS: usize = 2;
/// Constructor queues the search may write.
const SEARCHED_CONSTRUCTORS: usize = 4;

pub(super) struct Opening {
    plan: Plan,
    /// Per queue of the plan: the step its builder takes next.
    next: Vec<usize>,
    /// Which queue each builder works through, given out in the order builders appear (as the simulator does).
    queue_of: HashMap<UnitId, usize>,
    factories: usize,
    constructors: usize,
    /// Per builder: the frame of its last plan order and how often that step has been given again.
    last: HashMap<UnitId, (i32, u32)>,
}

/// What a builder on the plan does next.
pub(super) enum Planned {
    Extractor(Vec3),
    /// With the place the plan names for it, if it names one (a turret at a metal spot).
    Building(UnitDefId, Option<Vec3>),
}

impl Brain {
    /// The game as the build-order simulator sees it, from what the engine told us at the start.
    pub(super) fn buildorder_game(&self, kit: &Kit) -> Option<Game> {
        let hello = &self.world.hello;
        let units = Units::new(&hello.unit_defs);
        let commander = hello.unit_defs.iter().position(|d| d.id == kit.commander)?;
        Some(Game {
            units,
            commander,
            home: (self.home.x as f64, self.home.z as f64),
            size: (hello.map.width as f64, hello.map.height as f64),
            spots: hello.metal_spots.iter().map(|s| ((s.x as f64, s.z as f64), s.y as f64)).collect(),
            wind: (hello.map.wind_min as f64, hello.map.wind_max as f64),
            terrain: hello.terrain.clone(),
        })
    }

    /// Today's rule-made opening written down as a plan, the executor's first test: played through the plan it should
    /// come out as the rules play it.
    fn default_opening(&self, game: &Game, kit: &Kit) -> Option<Plan> {
        let index = |def: UnitDefId| self.world.hello.unit_defs.iter().position(|d| d.id == def);
        let step = |def: UnitDefId| index(def).map(Step::build);
        let (mex, wind, solar, lab) = (step(kit.extractor)?, step(kit.wind)?, step(kit.solar)?, step(kit.lab)?);
        let (constructor, line, raider) = (step(kit.constructor)?, step(kit.line)?, step(kit.raider)?);
        let windy = game.mean_wind() >= 9.0;
        let generator = if windy { wind } else { solar };
        let mut plan = Plan::empty(1, 2);
        plan.commander = vec![mex, mex, generator, generator, lab, mex, generator, solar, solar, generator, solar];
        plan.factories[0] = vec![constructor, constructor, line, raider, line, constructor, line, line];
        plan.constructors[0] = vec![mex, mex, mex];
        plan.constructors[1] = vec![mex, mex, mex];
        Some(plan)
    }

    /// The opening for this start: the best plan an anytime search finds from the rule-made one within its budget,
    /// by the simulator's measure (H-OPEN-SEARCH; with it off, the rule-made one as it stands).
    pub(super) fn start_opening(&mut self, tick: &Tick, kit: &Kit) {
        if self.opening_tried || !self.enabled("H-OPEN-PLAN") {
            return;
        }
        self.opening_tried = true;
        let Some(game) = self.buildorder_game(kit) else { return };
        let Some(mut plan) = self.default_opening(&game, kit) else { return };
        let lab = self.world.hello.unit_defs.iter().position(|d| d.id == kit.lab);
        if self.enabled("H-OPEN-SEARCH") && let Some(lab) = lab {
            // Ours to count on: the spots nearer to us than to the enemy on foot.
            let mut spots: Vec<Spot> = self.world.hello.metal_spots.iter().filter(|s| self.reachable_on_foot(**s) && self.spot_is_ours(**s))
                .map(|s| Spot { at: (s.x as f64, s.z as f64), metal: game.spot_metal(s.y as f64) }).collect();
            spots.sort_by(|a, b| distance(a.at, game.home).total_cmp(&distance(b.at, game.home)));
            let mut scenario = game.scenario(spots, game.ground());
            scenario.constructors_default_to_extractors = true;
            scenario.commander_leash = super::economy::EARLY_COMMANDER_LEASH as f64;
            let palette = Palette::new(&game.units, game.commander, lab, false, self.world.hello.unit_defs.iter().position(|d| d.id == kit.turret));
            let horizon = (HORIZON_FRAMES / FRAMES_PER_SECOND) as f64;
            let search = Search { objective: Objective::Tempo { army: 1.0, exposed: 0.3 }, horizon, iterations: 0, seed: 1, factories: 1, constructors: SEARCHED_CONSTRUCTORS, hot: 0.02, start: Some(plan.clone()) };
            let started = std::time::Instant::now();
            let before = search.objective.score(&simulate(&game.units, &scenario, &plan, horizon), horizon);
            let found = anneal_within(&game.units, &scenario, &palette, &search, SEARCH_BUDGET, SEARCH_THREADS);
            let at = |minute: f64| found.outcome.samples.iter().find(|s| s.t == minute * 60.0).map_or((0, 0.0, 0.0), |s| (s.extractors, s.metal_income, s.army_value));
            eprintln!(
                "[ai {}] f={} opening search: {:.0} ms, score {:.0} from {:.0}; predicted extractors / metal per s / army metal at 2, 3, 5 min: {:?} {:?} {:?}",
                self.ai(), tick.frame, started.elapsed().as_secs_f64() * 1000.0, found.score, before, at(2.0), at(3.0), at(5.0)
            );
            plan = found.plan;
        }
        eprintln!("[ai {}] f={} opening plan:\n{}", self.ai(), tick.frame, plan.to_text(&game.units));
        let queues = plan.queue_count();
        self.opening = Some(Opening { plan, next: vec![0; queues], queue_of: HashMap::new(), factories: 0, constructors: 0, last: HashMap::new() });
    }

    /// Ends the plan for everybody when the game has left it.
    fn check_opening(&mut self, tick: &Tick, kit: &Kit) {
        if self.opening.is_none() {
            return;
        }
        // Radar contacts count: what walks up to the base in the first minutes is not a constructor.
        let armed = |def: Option<UnitDefId>| def.is_none_or(|d| self.world.def(d).is_some_and(|d| d.weapon_count > 0 && d.speed > 0.0));
        let raided = tick.snapshot.enemies.iter().any(|e| armed(e.def) && e.pos.dist2d(self.home) < RAID_RADIUS);
        let commander_lost = !tick.snapshot.own_units.iter().any(|u| u.def == kit.commander);
        let why = match () {
            _ if tick.frame > HORIZON_FRAMES => "the horizon is reached",
            _ if raided => "enemy soldiers at home",
            _ if commander_lost => "the commander is gone",
            _ => return,
        };
        eprintln!("[ai {}] f={} opening plan ends: {why}", self.ai(), tick.frame);
        self.opening = None;
    }

    /// Everything the plan has left for this factory, to be queued in one go: a factory that is never idle loses no
    /// time between units.
    pub(super) fn opening_factory_batch(&mut self, factory: &OwnUnit, tick: &Tick, kit: &Kit) -> Option<Vec<UnitDefId>> {
        let mut batch = Vec::new();
        while let Some(Planned::Building(def, _)) = self.opening_step(factory, tick, kit) {
            batch.push(def);
        }
        (!batch.is_empty()).then_some(batch)
    }

    /// The next step of `builder`'s queue, if the plan is on and has one for it.
    pub(super) fn opening_step(&mut self, builder: &OwnUnit, tick: &Tick, kit: &Kit) -> Option<Planned> {
        self.start_opening(tick, kit);
        self.check_opening(tick, kit);
        let own = tick.snapshot.own_units.as_slice();
        let opening = self.opening.as_mut()?;
        let queue = match opening.queue_of.get(&builder.id) {
            Some(queue) => *queue,
            None => {
                let queue = if builder.def == kit.commander {
                    0
                } else if builder.def == kit.lab {
                    opening.factories += 1;
                    opening.plan.factory_queue(opening.factories - 1)
                } else if builder.def == kit.constructor {
                    opening.constructors += 1;
                    opening.plan.constructor_queue(opening.constructors - 1)
                } else {
                    return None;
                };
                opening.queue_of.insert(builder.id, queue);
                queue
            }
        };
        if queue >= opening.plan.queue_count() {
            return None;
        }
        // A step ordered a moment ago and not begun was lost on the way.
        if let Some((frame, tries)) = opening.last.get(&builder.id).copied()
            && builder.def != kit.lab
            && tick.frame - frame < RETRY_FRAMES
            && tries < RETRIES
            && opening.next[queue] > 0
        {
            opening.next[queue] -= 1;
            opening.last.insert(builder.id, (tick.frame, tries + 1));
        } else {
            opening.last.insert(builder.id, (tick.frame, 0));
        }
        loop {
            let opening = self.opening.as_mut()?;
            let step = *opening.plan.queue(queue).get(opening.next[queue])?;
            opening.next[queue] += 1;
            let Item::Build(unit) = step.item else { continue };
            let def = self.world.hello.unit_defs[unit].id;
            if !self.world.def(builder.def).is_some_and(|d| d.build_options.contains(&def)) {
                continue;
            }
            if def != kit.extractor {
                return Some(Planned::Building(def, step.site.map(|(x, z)| Vec3 { x: x as f32, y: 0.0, z: z as f32 })));
            }
            // The plan's own spot when it names one that is still free, else the spot the rules would hand out.
            let named = step.site.map(|(x, z)| Vec3 { x: x as f32, y: 0.0, z: z as f32 }).and_then(|site| {
                let (i, spot) = self.world.hello.metal_spots.iter().copied().enumerate().min_by(|a, b| a.1.dist2d(site).total_cmp(&b.1.dist2d(site)))?;
                let free = spot.dist2d(site) < 100.0 && !self.spot_claims.contains_key(&i) && !self.spot_taken(spot, own, kit) && !self.is_unreachable(spot);
                free.then(|| {
                    self.spot_claims.insert(i, tick.frame);
                    spot
                })
            });
            if let Some(spot) = named.or_else(|| self.claim_spot(builder, own, kit, tick.frame)) {
                return Some(Planned::Extractor(spot));
            }
        }
    }
}
