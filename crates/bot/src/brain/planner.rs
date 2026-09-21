//! The rolling-horizon economy planner (`docs/design/2026-09-21-rolling-planner.md`). H-OPEN-SEARCH searches a plan
//! (`buildorder::plan::Plan`: one queue for the commander, one per factory, one per constructor) from a snapshot of
//! the game on its own thread, ten minutes ahead, again every `PLAN_INTERVAL_FRAMES` and whenever a builder or
//! factory of ours comes or goes; H-OPEN-PLAN has each builder work through its queue when idle, ahead of the
//! rules, which take a builder whose queue has run out. The plan ends only with the commander.
//!
//! The first search of a game blocks the tick (the engine takes no orders in the opening seconds anyway); the later
//! ones run beside the game and are re-based on what each builder began while they ran.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};
use std::sync::Arc;

use bot_protocol::{OwnUnit, Tick, UnitDefId, UnitId, Vec3};
use buildorder::anneal::{anneal_within, Contact, Expectations, Objective, Palette, Search};
use buildorder::game::{distance, Game, Ground, Spot};
use buildorder::plan::{Item, Plan, Step};
use buildorder::sim::{simulate, Job, Scenario, Standing, State, StateBuilder};
use buildorder::units::{Role, Units};

use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};

/// How far ahead a plan is priced: three minutes, with the expectations of the clock at the horizon (the user,
/// 2026-09-21: ten minutes was "much too long"; K-plan-ten-minute-horizon-trades-early-army).
const HORIZON_SECONDS: f64 = 180.0;
/// A new plan from a fresh snapshot this often, when nothing forces one sooner; never sooner than the minimum, whatever
/// comes and goes (in the smoke game constructors dying under a raid forced one every twelve seconds).
const PLAN_INTERVAL_FRAMES: i32 = 30 * FRAMES_PER_SECOND;
const PLAN_MIN_INTERVAL_FRAMES: i32 = 10 * FRAMES_PER_SECOND;
/// Metal spots within this of an armed enemy in sight are left out of the search, and a plan's named spot within it
/// is refused: the rules' spot choice (which knows the threat) takes over for that step.
const RAID_RADIUS: f32 = 1000.0;
/// A plan step whose builder is idle again this soon never started (the engine drops orders now and then): once more.
const RETRY_FRAMES: i32 = 3 * FRAMES_PER_SECOND;
const RETRIES: u32 = 2;
/// The search's wall-time budget and threads.
const SEARCH_BUDGET: std::time::Duration = std::time::Duration::from_millis(400);
const SEARCH_THREADS: usize = 2;
/// Queues the search writes beyond the builders standing at the snapshot.
const FUTURE_FACTORIES: usize = 1;
const FUTURE_CONSTRUCTORS: usize = 2;
/// A nanoframe this close to the builder whose job is that type is that job.
const JOB_REACH: f32 = 350.0;
/// Units a factory is given from its queue at once: the one it builds and one behind it, so a re-plan finds at most
/// one unit committed beyond the current one.
const FACTORY_BATCH: usize = 2;
/// H-OPEN-CONTACT: the first fight (K-open-early-pawn-pressure-is-standard) is at 2:30 at the opponent's base; a
/// soldier that can stand there by then is worth this much of its metal on top, falling to nothing over the window.
/// At 8 the search on Quicksilver's north start builds 10 soldiers by minute 3 and 16 by 5 beside 5 extractors and no constructor, near
/// the experienced player's 7 / 15 beside 4 / 6 (`docs/design/2026-09-20-rush-benchmark.md`); at 5 it keeps four
/// constructors and has 6 soldiers at minute 3, at 20 it stalls for soldiers. A 60 s window (rush-4-ab) made the
/// term unreachable and the search built no Pawns at all before minute 8.
const CONTACT_AT: f64 = 150.0;
const CONTACT_WEIGHT: f64 = 6.0;
const CONTACT_WINDOW: f64 = 240.0;
/// Length of one `assist` step, the simulator's `assist_chunk`.
const ASSIST_FRAMES: i32 = 20 * FRAMES_PER_SECOND;

/// A search under way on its own thread, and what has happened since its snapshot.
struct Pending {
    receiver: Receiver<Searched>,
    /// The builders the snapshot listed, in the state's order, each with its rank (commander 0, factory 1,
    /// constructor 2): their queues are 0, then the factories', then the constructors', in this order.
    order: Vec<(UnitId, usize)>,
    standing_factories: usize,
    standing_constructors: usize,
    /// Builds begun since the snapshot, by builder, in order: the plan that comes back skips them.
    begun: Vec<(UnitId, UnitDefId)>,
    /// Whether the tick waits for the answer (the game's first plan).
    blocking: bool,
}

struct Searched {
    plan: Plan,
    /// One line for the log.
    report: String,
}

pub(super) struct Planner {
    plan: Plan,
    /// Per queue of the plan: the step its builder takes next.
    next: Vec<usize>,
    /// Which queue each builder works through: the snapshot's builders by the state's order, later ones in the order
    /// they appear (as the simulator recruits them).
    queue_of: HashMap<UnitId, usize>,
    factories: usize,
    constructors: usize,
    /// Per builder: the frame of its last plan order and how often that step has been given again.
    last: HashMap<UnitId, (i32, u32)>,
    /// Builders on an `assist` step (guarding the first lab), and the frame the step ends at.
    assist_until: HashMap<UnitId, i32>,
    /// What each factory has been given from its queue and the engine has not started yet, oldest first.
    factory_queue: HashMap<UnitId, Vec<UnitDefId>>,
    /// Who made each nanoframe of ours (the engine's creation events), while it is being built: a builder's job.
    nanoframe_by: HashMap<UnitId, UnitId>,
    /// The frame the current plan's snapshot was taken at, and the builders it counted.
    planned_frame: i32,
    planned_builders: usize,
    /// A plan from a file (`WITHIN_REASON_OPENING_PLAN`) is played as given and never searched again.
    given: bool,
    pending: Option<Pending>,
    /// The game as the simulator sees it, and its ground with the walks it has already priced, kept across searches.
    game: Arc<Game>,
    ground: Arc<dyn Ground>,
}

/// What a builder on the plan does next.
pub(super) enum Planned {
    Extractor(Vec3),
    /// With the place the plan names for it, if it names one (a turret at a metal spot).
    Building(UnitDefId, Option<Vec3>),
    /// Guard this lab (add the builder's build power to it) for the assist chunk.
    Assist(UnitId),
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
            // H-OPEN-WIND: plans priced at the engine's process mean; off, at the middle of the range as before.
            wind_override: (!self.enabled("H-OPEN-WIND")).then(|| (hello.map.wind_min as f64 + hello.map.wind_max as f64) / 2.0),
            terrain: hello.terrain.clone(),
        })
    }

    /// Today's rule-made opening written down as a plan: the first search's warm start, and the plan with the search off.
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

    /// The game as it stands now, for the simulator: standing economy buildings, the builders with their jobs, the
    /// stock. Returns the state and the builders in its order, each with its rank.
    fn snapshot(&self, tick: &Tick, kit: &Kit, game: &Game) -> (State, Vec<(UnitId, usize)>) {
        let hello = &self.world.hello;
        let nanoframe_by = self.planner.as_ref().map(|p| &p.nanoframe_by);
        let own = &tick.snapshot.own_units;
        let index = |def: UnitDefId| hello.unit_defs.iter().position(|d| d.id == def);
        let at = |p: Vec3| (p.x as f64, p.z as f64);
        let pays = |unit: usize, pos: Vec3| {
            if game.units.list[unit].extracts_metal == 0.0 {
                return 0.0;
            }
            hello.metal_spots.iter().filter(|s| s.dist2d(pos) < 100.0).map(|s| game.spot_metal(s.y as f64)).next().unwrap_or(0.0)
        };
        let standing: Vec<Standing> = own
            .iter()
            .filter(|u| !u.being_built)
            .filter_map(|u| {
                let unit = index(u.def)?;
                matches!(game.units.list[unit].role, Role::Eco | Role::Turret | Role::Nano).then(|| Standing { unit, site: at(u.pos), pays: pays(unit, u.pos) })
            })
            .collect();
        let rank = |def: UnitDefId| match def {
            d if d == kit.commander => Some(0),
            d if d == kit.lab => Some(1),
            d if d == kit.constructor => Some(2),
            _ => None,
        };
        let mut builders: Vec<(&OwnUnit, usize, usize)> = own.iter().filter(|u| !u.being_built).filter_map(|u| Some((u, rank(u.def)?, index(u.def)?))).collect();
        builders.sort_by_key(|(u, rank, _)| (*rank, u.id.0));
        let job_of = |builder: &OwnUnit| -> Option<Job> {
            // The nanoframe it made and has not finished (the nearest, if several); else the build it was last sent
            // to and has not begun, if it is still on its way there.
            let frame = own
                .iter()
                .filter(|u| u.being_built && nanoframe_by.is_some_and(|by| by.get(&u.id) == Some(&builder.id)) && u.pos.dist2d(builder.pos) < JOB_REACH)
                .min_by(|a, b| a.pos.dist2d(builder.pos).total_cmp(&b.pos.dist2d(builder.pos)));
            match frame {
                Some(f) => {
                    let unit = index(f.def)?;
                    Some(Job { unit, site: at(f.pos), progress: (f.health / f.max_health.max(1.0)).clamp(0.0, 1.0) as f64, pays: pays(unit, f.pos) })
                }
                None => {
                    let job = *self.jobs.get(&builder.id)?;
                    let (_, ordered, near) = self.last_orders.get(&builder.id).copied()?;
                    let unit = index(job)?;
                    (ordered == job && !builder.idle).then(|| Job { unit, site: at(near), progress: 0.0, pays: pays(unit, near) })
                }
            }
        };
        let army_metal: f64 = own.iter().filter(|u| !u.being_built).filter_map(|u| index(u.def)).filter(|i| game.units.list[*i].role == Role::Army).map(|i| game.units.list[i].metal_cost).sum();
        let state = State {
            t0: (tick.frame / FRAMES_PER_SECOND) as f64,
            army_metal,
            metal: tick.snapshot.metal.current as f64,
            energy: tick.snapshot.energy.current as f64,
            metal_storage: tick.snapshot.metal.storage as f64,
            energy_storage: tick.snapshot.energy.storage as f64,
            standing,
            builders: builders.iter().map(|(u, _, unit)| StateBuilder { unit: *unit, place: at(u.pos), job: job_of(u) }).collect(),
        };
        (state, builders.iter().map(|(u, rank, _)| (u.id, *rank)).collect())
    }

    /// The scenario a plan is priced in now: the spots ours on foot, less those an armed enemy in sight stands by.
    fn scenario(&self, tick: &Tick, game: &Game, ground: Arc<dyn Ground>) -> Scenario {
        let raiders: Vec<Vec3> = tick.snapshot.enemies.iter().filter(|e| self.armed(e.def)).map(|e| e.pos).collect();
        let mut spots: Vec<Spot> = self
            .world
            .hello
            .metal_spots
            .iter()
            .filter(|s| self.reachable_on_foot(**s) && self.spot_is_ours(**s) && !raiders.iter().any(|r| r.dist2d(**s) < RAID_RADIUS))
            .map(|s| Spot { at: (s.x as f64, s.z as f64), metal: game.spot_metal(s.y as f64) })
            .collect();
        spots.sort_by(|a, b| distance(a.at, game.home).total_cmp(&distance(b.at, game.home)));
        let mut scenario = game.scenario(spots, ground);
        scenario.constructors_default_to_extractors = true;
        scenario.commander_leash = super::economy::EARLY_COMMANDER_LEASH as f64;
        scenario
    }

    /// Armed and mobile, or unknown: what walks up to the base is not a constructor. Radar contacts count.
    fn armed(&self, def: Option<UnitDefId>) -> bool {
        def.is_none_or(|d| self.world.def(d).is_some_and(|d| d.weapon_count > 0 && d.speed > 0.0))
    }

    /// The current plan as a warm start for a search from `order`'s builders: each builder's steps still ahead, then
    /// the queues nobody has taken up yet, sized to the new search's shape.
    fn rebased(&self, order: &[(UnitId, usize)], factories: usize, constructors: usize) -> Option<Plan> {
        let planner = self.planner.as_ref()?;
        let ahead = |q: usize| planner.plan.queue(q).get(planner.next[q]..).map(|s| s.to_vec()).unwrap_or_default();
        let mut plan = Plan::empty(factories, constructors);
        let (mut nf, mut nc) = (0, 0);
        for (id, _) in order {
            match planner.queue_of.get(id) {
                Some(0) => plan.commander = ahead(0),
                Some(&q) if q <= planner.plan.factories.len() => {
                    if nf < factories {
                        plan.factories[nf] = ahead(q);
                    }
                    nf += 1;
                }
                Some(&q) => {
                    if nc < constructors {
                        plan.constructors[nc] = ahead(q);
                    }
                    nc += 1;
                }
                // A builder that appeared since the plan: nothing ahead for it yet.
                None => {}
            }
        }
        let taken: Vec<usize> = planner.queue_of.values().copied().collect();
        for q in 1..planner.plan.queue_count() {
            if taken.contains(&q) {
                continue;
            }
            if q <= planner.plan.factories.len() {
                if nf < factories {
                    plan.factories[nf] = ahead(q);
                    nf += 1;
                }
            } else if nc < constructors {
                plan.constructors[nc] = ahead(q);
                nc += 1;
            }
        }
        Some(plan)
    }

    /// Starts a search from a snapshot now. `blocking`: the tick waits for it (the game's first plan).
    fn search(&mut self, tick: &Tick, kit: &Kit, blocking: bool) {
        let Some(planner) = self.planner.as_ref() else { return };
        let (game, ground) = (planner.game.clone(), planner.ground.clone());
        let Some(lab) = self.world.hello.unit_defs.iter().position(|d| d.id == kit.lab) else { return };
        let (state, order) = self.snapshot(tick, kit, &game);
        let scenario = self.scenario(tick, &game, ground);
        let (standing_factories, standing_constructors) = (state.standing_factories(&game.units), state.standing_constructors(&game.units));
        let (factories, constructors) = (standing_factories + FUTURE_FACTORIES, standing_constructors + FUTURE_CONSTRUCTORS);
        let start = self.rebased(&order, factories, constructors).unwrap_or_else(|| self.default_opening(&game, kit).unwrap_or_else(|| Plan::empty(factories, constructors)));
        let palette = Palette::new(&game.units, game.commander, lab, true, self.world.hello.unit_defs.iter().position(|d| d.id == kit.turret));
        let contact = self.enabled("H-OPEN-CONTACT").then(|| {
            self.fire("H-OPEN-CONTACT");
            Contact { at: CONTACT_AT, walk: self.walk_from_home(self.enemy_base(self.home)) as f64, weight: CONTACT_WEIGHT, window: CONTACT_WINDOW }
        });
        self.fire("H-OPEN-SEARCH");
        let search = Search { objective: Objective::Expect { exposed: 0.3, contact, expect: Expectations::STANDARD }, horizon: HORIZON_SECONDS, iterations: 0, seed: 1 + tick.frame as u64, factories, constructors, hot: 0.02, start: Some(start.clone()) };
        // `WITHIN_REASON_SEARCH_MS` overrides the budget, to measure what more of it buys.
        let budget = std::env::var("WITHIN_REASON_SEARCH_MS").ok().and_then(|ms| ms.parse().ok()).map_or(SEARCH_BUDGET, std::time::Duration::from_millis);
        let (sender, receiver) = channel();
        let first = blocking;
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let end = state.t0 + HORIZON_SECONDS;
            let warm = simulate(&game.units, &scenario, &state, &start, HORIZON_SECONDS);
            let before = search.objective.score(&game.units, &warm, end);
            let found = anneal_within(&game.units, &scenario, &state, &palette, &search, budget, SEARCH_THREADS);
            let at = |seconds: f64| found.outcome.samples.iter().find(|s| s.t == state.t0 + seconds).map_or((0, 0.0, 0.0), |s| (s.extractors, s.metal_income, s.army_value));
            let warm_at = |seconds: f64| warm.samples.iter().find(|s| s.t == state.t0 + seconds).map_or((0, 0.0, 0.0), |s| (s.extractors, s.metal_income, s.army_value));
            let report = if first {
                // The comparison script (`run/opening_ab.py`) reads this line's shape.
                format!(
                    "opening search: {:.0} ms, score {:.0} from {:.0}, contact walk {:.0}; predicted extractors / metal per s / army metal at 1, 2, 3 min: {:?} {:?} {:?}",
                    started.elapsed().as_secs_f64() * 1000.0, found.score, before, contact.map_or(0.0, |c| c.walk), at(60.0), at(120.0), at(180.0)
                )
            } else {
                let mut report = format!(
                    "planner from {:.0} s ({} standing, {} builders, {} on a job, metal {:.0} energy {:.0}): {:.0} ms, score {:.0} from {:.0}; predicted extractors / metal per s / army metal 1, 2 and 3 min on: {:?} {:?} {:?}; the warm start's: {:?} {:?}",
                    state.t0, state.standing.len(), state.builders.len(), state.builders.iter().filter(|b| b.job.is_some()).count(), state.metal, state.energy,
                    started.elapsed().as_secs_f64() * 1000.0, found.score, before, at(60.0), at(120.0), at(180.0), warm_at(120.0), warm_at(180.0)
                );
                if before < 0.6 * found.score {
                    report.push_str(&format!("\nwarm start (scored {before:.0}):\n{}state: {state:?}", start.to_text(&game.units)));
                }
                report
            };
            let _ = sender.send(Searched { plan: found.plan, report });
        });
        if let Some(planner) = self.planner.as_mut() {
            planner.pending = Some(Pending { receiver, order, standing_factories, standing_constructors, begun: Vec::new(), blocking });
        }
    }

    /// A searched plan comes in: the builders of its snapshot take its queues, and each skips what it began while
    /// the search ran.
    fn install(&mut self, searched: Searched, pending: Pending, tick: &Tick) {
        eprintln!("[ai {}] f={} {}", self.ai(), tick.frame, searched.report);
        let ids: Vec<UnitDefId> = self.world.hello.unit_defs.iter().map(|d| d.id).collect();
        let queued = self.queued.clone();
        let Some(planner) = self.planner.as_mut() else { return };
        let plan = searched.plan;
        let mut queue_of = HashMap::new();
        let (mut nf, mut nc) = (0, 0);
        for (id, rank) in &pending.order {
            let queue = match rank {
                0 => 0,
                1 => {
                    nf += 1;
                    plan.factory_queue(nf - 1)
                }
                _ => {
                    nc += 1;
                    plan.constructor_queue(nc - 1)
                }
            };
            queue_of.insert(*id, queue);
        }
        let mut next = vec![0; plan.queue_count()];
        let mut skip = |builder: UnitId, def: UnitDefId| {
            let Some(&q) = queue_of.get(&builder) else { return };
            if q >= plan.queue_count() {
                return;
            }
            if let Some(Step { item: Item::Build(unit), .. }) = plan.queue(q).get(next[q])
                && ids[*unit] == def
            {
                next[q] += 1;
            }
        };
        for (builder, def) in &pending.begun {
            skip(*builder, *def);
        }
        // Builds given as queued orders and not yet begun run whatever the new plan says: they are past too.
        for (builder, (def, _, _)) in &queued {
            skip(*builder, *def);
        }
        for (factory, defs) in &planner.factory_queue {
            for def in defs {
                skip(*factory, *def);
            }
        }
        planner.plan = plan;
        planner.next = next;
        planner.queue_of = queue_of;
        planner.factories = pending.standing_factories;
        planner.constructors = pending.standing_constructors;
        planner.last.clear();
        planner.planned_frame = tick.frame;
        planner.planned_builders = pending.order.len();
        eprintln!("[ai {}] f={} plan:\n{}", self.ai(), tick.frame, self.planner.as_ref().map(|p| p.plan.to_text(&p.game.units)).unwrap_or_default());
    }

    /// Once a tick: the planner's own bookkeeping. Makes the first plan (blocking), polls a search under way,
    /// starts the next one when due, ends the plan with the commander.
    pub(super) fn run_planner(&mut self, tick: &Tick, kit: &Kit) {
        if self.planner_off || !self.enabled("H-OPEN-PLAN") {
            return;
        }
        if !tick.snapshot.own_units.iter().any(|u| u.def == kit.commander) {
            if self.planner.is_some() {
                eprintln!("[ai {}] f={} plan ends: the commander is gone", self.ai(), tick.frame);
            }
            self.planner = None;
            self.planner_off = true;
            return;
        }
        if self.planner.is_none() {
            let Some(game) = self.buildorder_game(kit) else {
                self.planner_off = true;
                return;
            };
            let ground = game.ground();
            let game = Arc::new(game);
            let Some(default) = self.default_opening(&game, kit) else {
                self.planner_off = true;
                return;
            };
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
            let searching = given.is_none() && self.enabled("H-OPEN-SEARCH");
            let plan = given.unwrap_or(default);
            let queues = plan.queue_count();
            self.planner = Some(Planner {
                plan, next: vec![0; queues], queue_of: HashMap::new(), factories: 0, constructors: 0, last: HashMap::new(), assist_until: HashMap::new(),
                factory_queue: HashMap::new(), nanoframe_by: HashMap::new(), planned_frame: tick.frame, planned_builders: 1, given: !searching, pending: None, game, ground,
            });
            if searching {
                self.search(tick, kit, true);
            } else if let Some(p) = self.planner.as_ref() {
                eprintln!("[ai {}] f={} opening plan:\n{}", self.ai(), tick.frame, p.plan.to_text(&p.game.units));
            }
        }
        // What builders began this tick: for a search under way, and as each builder's job while the nanoframe lasts.
        let begun: Vec<(UnitId, UnitDefId)> = tick
            .events
            .iter()
            .filter_map(|e| match e {
                bot_protocol::Event::UnitCreated { unit, builder: Some(builder) } => tick.snapshot.own_units.iter().find(|u| u.id == *unit).map(|u| (*builder, u.def)),
                _ => None,
            })
            .collect();
        let created: Vec<(UnitId, UnitId)> = tick
            .events
            .iter()
            .filter_map(|e| match e {
                bot_protocol::Event::UnitCreated { unit, builder: Some(builder) } => Some((*unit, *builder)),
                _ => None,
            })
            .collect();
        let Some(planner) = self.planner.as_mut() else { return };
        planner.nanoframe_by.extend(created);
        planner.nanoframe_by.retain(|id, _| tick.snapshot.own_units.iter().any(|u| u.id == *id && u.being_built));
        for (factory, def) in &begun {
            if let Some(queue) = planner.factory_queue.get_mut(factory)
                && let Some(i) = queue.iter().position(|d| d == def)
            {
                queue.remove(i);
            }
        }
        planner.factory_queue.retain(|id, _| tick.snapshot.own_units.iter().any(|u| u.id == *id));
        if let Some(pending) = planner.pending.as_mut() {
            pending.begun.extend(begun);
            let answer = if pending.blocking { pending.receiver.recv().ok() } else { pending.receiver.try_recv().ok() };
            if let Some(searched) = answer
                && let Some(pending) = self.planner.as_mut().and_then(|p| p.pending.take())
            {
                self.install(searched, pending, tick);
            }
            return;
        }
        if planner.given {
            return;
        }
        let builders = tick.snapshot.own_units.iter().filter(|u| !u.being_built && (u.def == kit.commander || u.def == kit.lab || u.def == kit.constructor)).count();
        let since = tick.frame - planner.planned_frame;
        let due = since >= PLAN_INTERVAL_FRAMES || (since >= PLAN_MIN_INTERVAL_FRAMES && builders != planner.planned_builders);
        if due {
            self.search(tick, kit, false);
        }
    }

    /// Everything the plan has left for this factory up to the batch, to be queued: a factory that is never idle
    /// loses no time between units, and one unit beyond the current is all a re-plan finds committed.
    pub(super) fn plan_factory_batch(&mut self, factory: &OwnUnit, tick: &Tick, kit: &Kit) -> Option<Vec<UnitDefId>> {
        // An idle factory's engine queue is empty, whatever it was given.
        if factory.idle && let Some(planner) = self.planner.as_mut() {
            planner.factory_queue.remove(&factory.id);
        }
        let given = self.planner.as_ref().map_or(0, |p| p.factory_queue.get(&factory.id).map_or(0, Vec::len));
        let mut batch = Vec::new();
        while given + batch.len() < FACTORY_BATCH {
            let Some(Planned::Building(def, _)) = self.plan_step(factory, tick, kit) else { break };
            batch.push(def);
        }
        if let Some(planner) = self.planner.as_mut() {
            planner.factory_queue.entry(factory.id).or_default().extend(batch.iter().copied());
        }
        (!batch.is_empty()).then_some(batch)
    }

    /// `builder`'s last step has begun (its nanoframe exists): it is not given again as lost.
    pub(super) fn step_begun(&mut self, builder: UnitId) {
        if let Some(planner) = self.planner.as_mut() {
            planner.last.remove(&builder);
        }
    }

    /// True once for a builder whose assist step has run its chunk: the economy plans it again although it is not
    /// idle (it is still guarding the lab).
    pub(super) fn assist_over(&mut self, builder: UnitId, frame: i32) -> bool {
        let Some(planner) = self.planner.as_mut() else { return false };
        if planner.assist_until.get(&builder).is_some_and(|until| frame >= *until) {
            planner.assist_until.remove(&builder);
            return true;
        }
        false
    }

    /// A step given to `builder` as a queued build that the engine never started: back to it.
    pub(super) fn unqueue_step(&mut self, builder: UnitId) {
        if let Some(planner) = self.planner.as_mut()
            && let Some(queue) = planner.queue_of.get(&builder).copied()
            && planner.next[queue] > 0
        {
            planner.next[queue] -= 1;
            planner.last.remove(&builder);
        }
    }

    /// The next step of `builder`'s queue, if the plan is on and has one for it.
    pub(super) fn plan_step(&mut self, builder: &OwnUnit, tick: &Tick, kit: &Kit) -> Option<Planned> {
        let own = tick.snapshot.own_units.as_slice();
        let raiders: Vec<Vec3> = tick.snapshot.enemies.iter().filter(|e| self.armed(e.def)).map(|e| e.pos).collect();
        let planner = self.planner.as_mut()?;
        let queue = match planner.queue_of.get(&builder.id) {
            Some(queue) => *queue,
            None => {
                let queue = if builder.def == kit.commander {
                    0
                } else if builder.def == kit.lab {
                    planner.factories += 1;
                    planner.plan.factory_queue(planner.factories - 1)
                } else if builder.def == kit.constructor {
                    planner.constructors += 1;
                    planner.plan.constructor_queue(planner.constructors - 1)
                } else {
                    return None;
                };
                planner.queue_of.insert(builder.id, queue);
                queue
            }
        };
        if queue >= planner.plan.queue_count() {
            return None;
        }
        // A step ordered a moment ago and not begun was lost on the way.
        if let Some((frame, tries)) = planner.last.get(&builder.id).copied()
            && builder.def != kit.lab
            && tick.frame - frame < RETRY_FRAMES
            && tries < RETRIES
            && planner.next[queue] > 0
        {
            planner.next[queue] -= 1;
            planner.last.insert(builder.id, (tick.frame, tries + 1));
        } else {
            planner.last.insert(builder.id, (tick.frame, 0));
        }
        loop {
            let planner = self.planner.as_mut()?;
            let step = *planner.plan.queue(queue).get(planner.next[queue])?;
            planner.next[queue] += 1;
            let Item::Build(unit) = step.item else {
                // Assist: guard the first finished lab for the chunk; with no lab standing yet the step is moot.
                let lab = own.iter().find(|u| u.def == kit.lab && !u.being_built);
                let Some(lab) = lab else { continue };
                planner.assist_until.insert(builder.id, tick.frame + ASSIST_FRAMES);
                return Some(Planned::Assist(lab.id));
            };
            let def = self.world.hello.unit_defs[unit].id;
            if !self.world.def(builder.def).is_some_and(|d| d.build_options.contains(&def)) {
                continue;
            }
            if def != kit.extractor {
                return Some(Planned::Building(def, step.site.map(|(x, z)| Vec3 { x: x as f32, y: 0.0, z: z as f32 })));
            }
            // The plan's own spot when it names one that is still free and no raider stands by it, else the spot
            // the rules would hand out.
            let named = step.site.map(|(x, z)| Vec3 { x: x as f32, y: 0.0, z: z as f32 }).and_then(|site| {
                let (i, spot) = self.world.hello.metal_spots.iter().copied().enumerate().min_by(|a, b| a.1.dist2d(site).total_cmp(&b.1.dist2d(site)))?;
                let free = spot.dist2d(site) < 100.0 && !self.spot_claims.contains_key(&i) && !self.spot_taken(spot, own, kit) && !self.is_unreachable(spot);
                let safe = !raiders.iter().any(|r| r.dist2d(spot) < RAID_RADIUS);
                (free && safe).then(|| {
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
