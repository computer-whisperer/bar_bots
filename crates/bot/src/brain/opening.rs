//! H-OPEN-PLAN: the opening as a plan (`buildorder::plan::Plan`: one queue for the commander, one per factory, one per
//! constructor) that builders work through in place of the opening rules, each taking its next step when it falls
//! idle. A builder whose queue has run out goes back to the ordinary rules, and so does everybody the moment reality
//! leaves the plan (`docs/design/2026-09-20-opening-search.md`).

use std::collections::HashMap;

use bot_protocol::{OwnUnit, Tick, UnitDefId, UnitId, Vec3};
use buildorder::anneal::{anneal_within, Contact, Objective, Palette, Search};
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
/// H-OPEN-CONTACT: the first fight (K-open-early-pawn-pressure-is-standard) is at 2:30 at the opponent's base; a
/// soldier that can stand there by then is worth this much of its metal on top, falling to nothing over the window.
/// At 8 the search on Quicksilver's north start builds 10 soldiers by minute 3 and 16 by 5 beside 5 extractors and no constructor, near
/// the experienced player's 7 / 15 beside 4 / 6 (`docs/design/2026-09-20-rush-benchmark.md`); at 5 it keeps four
/// constructors and has 6 soldiers at minute 3, at 20 it stalls for soldiers. A 60 s window (rush-4-ab) made the
/// term unreachable and the search built no Pawns at all before minute 8.
const CONTACT_AT: f64 = 150.0;
const CONTACT_WEIGHT: f64 = 6.0;
const CONTACT_WINDOW: f64 = 240.0;

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
    /// Builders on an `assist` step (guarding the first lab), and the frame the step ends at. Until 2026-09-20
    /// night the executor skipped assist steps, so the commander never lent the lab its build power although the
    /// searched plans asked for it three times over.
    assist_until: HashMap<UnitId, i32>,
}

/// What a builder on the plan does next.
pub(super) enum Planned {
    Extractor(Vec3),
    /// With the place the plan names for it, if it names one (a turret at a metal spot).
    Building(UnitDefId, Option<Vec3>),
    /// Guard this lab (add the builder's build power to it) for the assist chunk.
    Assist(UnitId),
}

/// Length of one `assist` step, the simulator's `assist_chunk`.
const ASSIST_FRAMES: i32 = 20 * FRAMES_PER_SECOND;

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
            // H-OPEN-WIND: plans priced at the engine's process mean; off, at the middle of the range as before.
            wind_override: (!self.enabled("H-OPEN-WIND")).then(|| (hello.map.wind_min as f64 + hello.map.wind_max as f64) / 2.0),
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
        // `WITHIN_REASON_OPENING_PLAN`: a plan from a file (`Plan::from_text`, the form the log prints) instead of the
        // search: a player's transcribed order (`run/replay_plan.py`), or one written by hand, played as given.
        let given = std::env::var_os("WITHIN_REASON_OPENING_PLAN").and_then(|path| {
            let side = self.world.def(kit.commander).map_or("arm".to_string(), |d| d.name.chars().take(3).collect());
            match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|text| Plan::from_text(&text, &side, &game.units)) {
                Ok(plan) => {
                    eprintln!("[ai {}] f={} opening plan from {}", self.ai(), tick.frame, path.to_string_lossy());
                    Some(plan)
                }
                Err(problem) => {
                    eprintln!("[ai {}] f={} opening plan file {} not usable ({problem}); searching instead", self.ai(), tick.frame, path.to_string_lossy());
                    None
                }
            }
        });
        if let Some(given) = given {
            plan = given;
        } else if self.enabled("H-OPEN-SEARCH") && let Some(lab) = lab {
            // Ours to count on: the spots nearer to us than to the enemy on foot.
            let mut spots: Vec<Spot> = self.world.hello.metal_spots.iter().filter(|s| self.reachable_on_foot(**s) && self.spot_is_ours(**s))
                .map(|s| Spot { at: (s.x as f64, s.z as f64), metal: game.spot_metal(s.y as f64) }).collect();
            spots.sort_by(|a, b| distance(a.at, game.home).total_cmp(&distance(b.at, game.home)));
            let mut scenario = game.scenario(spots, game.ground());
            scenario.constructors_default_to_extractors = true;
            scenario.commander_leash = super::economy::EARLY_COMMANDER_LEASH as f64;
            let palette = Palette::new(&game.units, game.commander, lab, true, self.world.hello.unit_defs.iter().position(|d| d.id == kit.turret));
            let horizon = (HORIZON_FRAMES / FRAMES_PER_SECOND) as f64;
            let contact = self.enabled("H-OPEN-CONTACT").then(|| {
                self.fire("H-OPEN-CONTACT");
                Contact { at: CONTACT_AT, walk: self.walk_from_home(self.enemy_base(self.home)) as f64, weight: CONTACT_WEIGHT, window: CONTACT_WINDOW }
            });
            let search = Search { objective: Objective::Tempo { army: 1.0, exposed: 0.3, contact }, horizon, iterations: 0, seed: 1, factories: 1, constructors: SEARCHED_CONSTRUCTORS, hot: 0.02, start: Some(plan.clone()) };
            let started = std::time::Instant::now();
            let before = search.objective.score(&game.units, &simulate(&game.units, &scenario, &plan, horizon), horizon);
            // `WITHIN_REASON_SEARCH_MS` overrides the budget, to measure what more of it buys.
            let budget = std::env::var("WITHIN_REASON_SEARCH_MS").ok().and_then(|ms| ms.parse().ok()).map_or(SEARCH_BUDGET, std::time::Duration::from_millis);
            let found = anneal_within(&game.units, &scenario, &palette, &search, budget, SEARCH_THREADS);
            let at = |minute: f64| found.outcome.samples.iter().find(|s| s.t == minute * 60.0).map_or((0, 0.0, 0.0), |s| (s.extractors, s.metal_income, s.army_value));
            eprintln!(
                "[ai {}] f={} opening search: {:.0} ms, score {:.0} from {:.0}, contact walk {:.0}; predicted extractors / metal per s / army metal at 2, 3, 5 min: {:?} {:?} {:?}",
                self.ai(), tick.frame, started.elapsed().as_secs_f64() * 1000.0, found.score, before, contact.map_or(0.0, |c| c.walk), at(2.0), at(3.0), at(5.0)
            );
            plan = found.plan;
        }
        eprintln!("[ai {}] f={} opening plan:\n{}", self.ai(), tick.frame, plan.to_text(&game.units));
        let queues = plan.queue_count();
        self.opening = Some(Opening { plan, next: vec![0; queues], queue_of: HashMap::new(), factories: 0, constructors: 0, last: HashMap::new(), assist_until: HashMap::new() });
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

    /// `builder`'s last step has begun (its nanoframe exists): it is not given again as lost.
    pub(super) fn step_begun(&mut self, builder: UnitId) {
        if let Some(opening) = self.opening.as_mut() {
            opening.last.remove(&builder);
        }
    }

    /// A step given to `builder` as a queued build that the engine never started: back to it.
    /// True once for a builder whose assist step has run its chunk: the economy plans it again although it is not
    /// idle (it is still guarding the lab).
    pub(super) fn assist_over(&mut self, builder: UnitId, frame: i32) -> bool {
        let Some(opening) = self.opening.as_mut() else { return false };
        if opening.assist_until.get(&builder).is_some_and(|until| frame >= *until) {
            opening.assist_until.remove(&builder);
            return true;
        }
        false
    }

    pub(super) fn unqueue_step(&mut self, builder: UnitId) {
        if let Some(opening) = self.opening.as_mut()
            && let Some(queue) = opening.queue_of.get(&builder).copied()
            && opening.next[queue] > 0
        {
            opening.next[queue] -= 1;
            opening.last.remove(&builder);
        }
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
            let Item::Build(unit) = step.item else {
                // Assist: guard the first finished lab for the chunk; with no lab standing yet the step is moot.
                let lab = own.iter().find(|u| u.def == kit.lab && !u.being_built);
                let Some(lab) = lab else { continue };
                opening.assist_until.insert(builder.id, tick.frame + ASSIST_FRAMES);
                return Some(Planned::Assist(lab.id));
            };
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
