//! Runs the duels of one engine match. Both teams' AI sessions call into the same director, so it sees both
//! sides whole: it spawns the armies, sends them at each other, scores the fight and clears the field.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use bot_protocol::{Command, Event, Hello, Tick, UnitDefId, UnitDefInfo, UnitId, Vec3};

use crate::plan::{Job, Sizing};
use crate::sites::{self, Site};

const FPS: i32 = 30;
/// The armies stand this long after spawning before they are ordered forward.
const SETTLE: i32 = FPS;
/// An army that has not fully appeared by now never will (unknown unit, blocked ground).
const SPAWN_ALLOWANCE: i32 = 10 * FPS;
/// A fight nobody has been hurt in for this long is a stalemate (anti-air against anti-air).
const STALEMATE: i32 = 60 * FPS;
/// After the last unit is gone the field rests this long: explosions finish, wrecks settle.
const REST: i32 = 2 * FPS;
/// Wrecks are blown up by crawling bombs on a grid this fine; the blast (the small one, since neighbours set each
/// other off) reaches 140 elmos.
const SWEEP_SPACING: f32 = 200.0;
/// A self-destruct order still not carried out after this long was cancelled and is given again. (The countdown is
/// 5 s. BAR's "Self-Destruct Resign" gadget cancels the first two attempts of a team to destroy 95% of its units at
/// once, which a big surviving army beside one commander is.)
const DESTRUCT_RETRY: i32 = 8 * FPS;
const SWEEP_BOMBS: [&str; 2] = ["corroach", "armvader"];
/// Self-destruct takes a few seconds, retries and bombing a few more; a site still not clear by now is abandoned.
const CLEANUP_ALLOWANCE: i32 = 120 * FPS;
/// Spawned units are recognised by type and by standing no further than this beyond their formation's furthest
/// place; bombs by standing this near the wreckage.
const CLAIM_MARGIN: f32 = 150.0;

/// What the matches of a batch share: the duels still to run and where results go.
pub struct Batch {
    pub queue: Mutex<VecDeque<Job>>,
    pub results: Mutex<Vec<DuelResult>>,
    pub sizing: Sizing,
    /// Game seconds after which an undecided fight is scored as it stands.
    pub time_limit: i32,
    /// Rounds of bombing after each duel: wrecks take one or two to become heaps, heaps another.
    pub sweep_waves: u32,
    /// Elmos between neighbours in a spawned formation; tight ranks flatter area damage.
    pub spacing: f32,
    pub on_result: Box<dyn Fn(&DuelResult) + Send + Sync>,
}

#[derive(Clone, Debug)]
pub struct DuelResult {
    pub match_index: usize,
    pub site: usize,
    /// Position of this duel in the site's sequence; wrecks of earlier duels lie on the field.
    pub sequence: u32,
    pub job: Job,
    pub count: [u32; 2],
    pub metal: [f32; 2],
    /// "x", "y" or "draw".
    pub winner: &'static str,
    /// "wiped", "timeout", "stalemate" or "spawn_failed".
    pub reason: &'static str,
    pub seconds: f32,
    /// Seconds from the advance order to the first damage.
    pub contact_seconds: Option<f32>,
    pub survivors: [u32; 2],
    /// Surviving metal value, each survivor weighted by its health, as a fraction of the army's value.
    pub value_left: [f32; 2],
    /// Hit points of damage each army took.
    pub damage_taken: [f32; 2],
}

struct Army {
    team: i32,
    def: UnitDefId,
    count: u32,
    metal_each: f32,
    mobile: bool,
    front: Vec3,
    faces_east: bool,
    spawn_ordered: bool,
    destruct_ordered: Option<i32>,
    units: HashSet<UnitId>,
    /// Health as a fraction, per living unit, as of `reported`.
    alive: HashMap<UnitId, f32>,
    /// Where each unit was last seen; a unit that dies leaves its wreck there.
    seen_at: HashMap<UnitId, Vec3>,
    centroid: Vec3,
    damage_taken: f32,
    /// Frame of the last tick this army's team reported on.
    reported: i32,
}

impl Army {
    fn value_left(&self) -> f32 {
        (self.alive.values().sum::<f32>() / self.count as f32).max(0.0)
    }
}

enum Phase {
    Spawning { since: i32 },
    Fighting { advance_at: i32, first_damage: Option<i32>, last_damage: i32 },
    Clearing { since: i32, sweep: Sweep },
}

/// Clearing the field: first the survivors destroy themselves, then bombs are spawned over the wrecks and set off,
/// `waves_left` times, then the field rests.
enum Sweep {
    Survivors,
    Bombs { waves_left: u32, spawned_at: Option<i32>, set_off: HashMap<UnitId, i32> },
    Rest { since: i32 },
    Done,
}

struct Duel {
    job: Job,
    /// `x` then `y`.
    armies: [Army; 2],
    phase: Phase,
    sequence: u32,
    /// Bounding box (min x, min z, max x, max z) of where units have died.
    wreckage: Option<[f32; 4]>,
}

struct Field {
    site: Site,
    duel: Option<Duel>,
    fought: u32,
    abandoned: bool,
}

pub struct Director {
    batch: Arc<Batch>,
    match_index: usize,
    wanted_sites: usize,
    /// No more than this many duels are started in this match (wrecks pile up on the sites).
    duel_budget: u32,
    defs: HashMap<String, UnitDefInfo>,
    hello: Option<Hello>,
    /// First sighting of each team's own units: the commander.
    commanders: HashMap<i32, Vec3>,
    fields: Option<Vec<Field>>,
    pub last_tick: Instant,
    pub finished: bool,
    pub fatal: Option<String>,
}

impl Director {
    pub fn new(batch: Arc<Batch>, match_index: usize, wanted_sites: usize, duel_budget: u32) -> Self {
        Director {
            batch,
            match_index,
            wanted_sites,
            duel_budget,
            defs: HashMap::new(),
            hello: None,
            commanders: HashMap::new(),
            fields: None,
            last_tick: Instant::now(),
            finished: false,
            fatal: None,
        }
    }

    pub fn hello(&mut self, hello: Hello) {
        if self.hello.is_none() {
            self.defs = hello.unit_defs.iter().map(|d| (d.name.clone(), d.clone())).collect();
            self.hello = Some(hello);
        }
    }

    /// Duels that were running when the match stopped; they go back on the queue.
    pub fn unfinished(&mut self) -> Vec<Job> {
        self.fields.iter_mut().flatten().filter_map(|f| f.duel.take()).map(|d| d.job).collect()
    }

    pub fn tick(&mut self, team: i32, tick: &Tick) -> Vec<Command> {
        self.last_tick = Instant::now();
        if self.finished || self.fatal.is_some() {
            return Vec::new();
        }
        if self.fields.is_none() {
            if let Some(unit) = tick.snapshot.own_units.first() {
                self.commanders.entry(team).or_insert(unit.pos);
            }
            if self.commanders.len() < 2 {
                return Vec::new();
            }
            self.lay_out_fields();
            if self.fatal.is_some() {
                return Vec::new();
            }
        }
        let mut commands = Vec::new();
        let mut fields = self.fields.take().unwrap_or_default();
        for (index, field) in fields.iter_mut().enumerate() {
            if field.duel.is_none() && !field.abandoned && self.duel_budget > 0 {
                field.duel = self.next_duel(field);
            }
            let Some(duel) = &mut field.duel else { continue };
            let bomb = SWEEP_BOMBS.iter().find_map(|name| self.defs.get(*name)).map(|def| def.id);
            let sweep = bomb.map(|bomb| (bomb, self.batch.sweep_waves));
            if let Some(result) = advance(duel, team, tick, self.batch.time_limit, sweep, self.batch.spacing, &mut commands) {
                let result = DuelResult { match_index: self.match_index, site: index, ..result };
                (self.batch.on_result)(&result);
                self.batch.results.lock().unwrap().push(result);
            }
            if let Phase::Clearing { since, sweep } = &duel.phase {
                if matches!(sweep, Sweep::Done) {
                    field.duel = None;
                } else if tick.frame - *since > CLEANUP_ALLOWANCE {
                    eprintln!("match {}: site {index} would not clear; abandoned", self.match_index);
                    field.duel = None;
                    field.abandoned = true;
                }
            }
        }
        let idle = fields.iter().all(|f| f.duel.is_none());
        let usable = fields.iter().any(|f| !f.abandoned);
        self.fields = Some(fields);
        if idle && (self.duel_budget == 0 || !usable || self.batch.queue.lock().unwrap().is_empty()) {
            self.finished = true;
        }
        commands
    }

    fn lay_out_fields(&mut self) {
        let hello = self.hello.as_ref().expect("hello precedes ticks");
        let queue = self.batch.queue.lock().unwrap();
        let mut max_slope = 1.0f32;
        for name in queue.iter().flat_map(|job| [&job.x, &job.y]) {
            match self.defs.get(name) {
                None => self.fatal = Some(format!("the game has no unit called {name}")),
                Some(def) => max_slope = max_slope.min(def.move_class.map_or(1.0, |class| class.max_slope)),
            }
        }
        drop(queue);
        let commanders: Vec<Vec3> = self.commanders.values().copied().collect();
        let sites = sites::choose(&hello.terrain, max_slope, &commanders, self.wanted_sites);
        if sites.is_empty() {
            self.fatal = Some(format!("no flat site found on {}", hello.map.name));
        }
        let at: Vec<String> = sites.iter().map(|s| format!("({:.0},{:.0})", s.centre.x, s.centre.z)).collect();
        let commanders: Vec<String> = commanders.iter().map(|c| format!("({:.0},{:.0})", c.x, c.z)).collect();
        eprintln!("match {}: {} site(s) at {}; commanders at {}", self.match_index, sites.len(), at.join(" "), commanders.join(" "));
        self.fields = Some(sites.into_iter().map(|site| Field { site, duel: None, fought: 0, abandoned: false }).collect());
    }

    fn next_duel(&mut self, field: &mut Field) -> Option<Duel> {
        let job = loop {
            let job = self.batch.queue.lock().unwrap().pop_front()?;
            // Two buildings never meet.
            if self.defs[&job.x].speed > 0.0 || self.defs[&job.y].speed > 0.0 {
                break job;
            }
        };
        self.duel_budget -= 1;
        let (def_x, def_y) = (&self.defs[&job.x], &self.defs[&job.y]);
        let (count_x, count_y) = self.batch.sizing.counts(def_x.metal_cost, def_y.metal_cost);
        let army = |def: &UnitDefInfo, count: u32, team: i32, west: bool| Army {
            team,
            def: def.id,
            count,
            metal_each: def.metal_cost,
            mobile: def.speed > 0.0,
            front: field.site.end(west),
            faces_east: west,
            spawn_ordered: false,
            destruct_ordered: None,
            units: HashSet::new(),
            alive: HashMap::new(),
            seen_at: HashMap::new(),
            centroid: field.site.end(west),
            damage_taken: 0.0,
            reported: -1,
        };
        let armies = [
            army(def_x, count_x, job.x_team(), job.x_is_west()),
            army(def_y, count_y, 1 - job.x_team(), !job.x_is_west()),
        ];
        field.fought += 1;
        Some(Duel { job, armies, phase: Phase::Spawning { since: -1 }, sequence: field.fought, wreckage: None })
    }
}

/// One team's tick for one duel. Returns the result on the tick that decides it.
/// `sweep` names the bomb unit and how many rounds of it clear the wrecks afterwards.
fn advance(
    duel: &mut Duel,
    team: i32,
    tick: &Tick,
    time_limit: i32,
    sweep: Option<(UnitDefId, u32)>,
    spacing: f32,
    commands: &mut Vec<Command>,
) -> Option<DuelResult> {
    let frame = tick.frame;
    let side = duel.armies.iter().position(|a| a.team == team)?;
    let enemy_centroid = duel.armies[1 - side].centroid;
    let army = &mut duel.armies[side];

    // What this team's snapshot says about its army.
    if matches!(duel.phase, Phase::Spawning { .. }) {
        // Spawned units are recognised by type and place; the previous duel's are all gone by now.
        let places = sites::formation(army.front, army.faces_east, army.count, spacing);
        let reach = places.iter().map(|p| p.dist2d(army.front)).fold(0.0, f32::max) + CLAIM_MARGIN;
        let arrivals = tick.snapshot.own_units.iter().filter(|u| u.def == army.def && u.pos.dist2d(army.front) < reach);
        for unit in arrivals {
            if army.units.len() < army.count as usize {
                army.units.insert(unit.id);
            }
        }
    }
    army.alive.clear();
    let mut sum = (0.0, 0.0);
    for unit in tick.snapshot.own_units.iter().filter(|u| army.units.contains(&u.id)) {
        army.alive.insert(unit.id, (unit.health / unit.max_health.max(1.0)).clamp(0.0, 1.0));
        army.seen_at.insert(unit.id, unit.pos);
        sum = (sum.0 + unit.pos.x, sum.1 + unit.pos.z);
    }
    let alive = &army.alive;
    for (_, at) in army.seen_at.extract_if(|unit, _| !alive.contains_key(unit)) {
        let [x0, z0, x1, z1] = duel.wreckage.unwrap_or([at.x, at.z, at.x, at.z]);
        duel.wreckage = Some([x0.min(at.x), z0.min(at.z), x1.max(at.x), z1.max(at.z)]);
    }
    if !army.alive.is_empty() {
        let n = army.alive.len() as f32;
        army.centroid = Vec3 { x: sum.0 / n, y: army.front.y, z: sum.1 / n };
    }
    army.reported = frame;
    let mut hurt = false;
    for event in &tick.events {
        if let Event::UnitDamaged { unit, damage, .. } = event
            && army.units.contains(unit)
        {
            army.damage_taken += damage;
            hurt = true;
        }
    }

    match &mut duel.phase {
        Phase::Spawning { since } => {
            if *since < 0 {
                *since = frame;
            }
            if !army.spawn_ordered {
                army.spawn_ordered = true;
                let places = sites::formation(army.front, army.faces_east, army.count, spacing);
                commands.extend(places.into_iter().map(|at| Command::GiveUnit { def: army.def, at }));
            }
            let since = *since;
            if duel.armies.iter().all(|a| a.units.len() == a.count as usize) {
                duel.phase = Phase::Fighting { advance_at: frame + SETTLE, first_damage: None, last_damage: frame + SETTLE };
            } else if frame - since > SPAWN_ALLOWANCE {
                return Some(conclude(duel, frame, frame, None, "spawn_failed"));
            }
            None
        }
        Phase::Fighting { advance_at, first_damage, last_damage } => {
            if hurt {
                first_damage.get_or_insert(frame);
                *last_damage = frame;
            }
            if frame < *advance_at {
                return None;
            }
            if army.mobile {
                // Attack-move at where the enemy is now; idle units (arrived, or never ordered) are sent again.
                let idle = tick.snapshot.own_units.iter().filter(|u| u.idle && army.units.contains(&u.id));
                commands.extend(idle.map(|u| Command::Fight { unit: u.id, to: enemy_centroid, queue: false }));
            }
            let (advance_at, first_damage, last_damage) = (*advance_at, *first_damage, *last_damage);
            // Judge once both teams have reported this frame.
            if duel.armies.iter().any(|a| a.reported != frame) {
                return None;
            }
            let reason = if duel.armies.iter().any(|a| a.alive.is_empty()) {
                "wiped"
            } else if frame - advance_at >= time_limit * FPS {
                "timeout"
            } else if frame - last_damage >= STALEMATE {
                "stalemate"
            } else {
                return None;
            };
            Some(conclude(duel, advance_at, frame, first_damage, reason))
        }
        Phase::Clearing { sweep: stage, .. } => {
            // Not again while the countdown may be running: a second self-destruct order cancels the first.
            if army.destruct_ordered.is_none_or(|at| frame - at >= DESTRUCT_RETRY) {
                army.destruct_ordered = Some(frame);
                commands.extend(army.alive.keys().map(|&unit| Command::SelfDestruct { unit }));
            }
            // The bombs belong to the first army's team, so only its ticks move the sweep along.
            let sweeper = side == 0;
            let front = duel.armies[0].front;
            match stage {
                Sweep::Survivors => {
                    if duel.armies.iter().all(|a| a.destruct_ordered.is_some() && a.alive.is_empty() && a.reported == frame) {
                        *stage = match (sweep, duel.wreckage) {
                            (Some((_, waves @ 1..)), Some(_)) => Sweep::Bombs { waves_left: waves, spawned_at: None, set_off: HashMap::new() },
                            _ => Sweep::Rest { since: frame },
                        };
                    }
                }
                Sweep::Bombs { waves_left, spawned_at, set_off } if sweeper => {
                    let (Some((bomb, _)), Some([x0, z0, x1, z1])) = (sweep, duel.wreckage) else { unreachable!("checked on entry") };
                    let bombs = tick.snapshot.own_units.iter().filter(|u| {
                        u.def == bomb && u.pos.x > x0 - CLAIM_MARGIN && u.pos.x < x1 + CLAIM_MARGIN && u.pos.z > z0 - CLAIM_MARGIN && u.pos.z < z1 + CLAIM_MARGIN
                    });
                    match *spawned_at {
                        None => {
                            *spawned_at = Some(frame);
                            let along = |low: f32, high: f32| {
                                let steps = ((high - low) / SWEEP_SPACING).ceil().max(1.0) as u32;
                                (0..=steps).map(move |i| low + (high - low) * i as f32 / steps as f32)
                            };
                            for x in along(x0, x1) {
                                commands.extend(along(z0, z1).map(|z| Command::GiveUnit { def: bomb, at: Vec3 { x, y: front.y, z } }));
                            }
                        }
                        Some(spawned) => {
                            // Bombs go off at once, so one still standing after the retry time had its order cancelled.
                            let mut standing = 0;
                            for bomb in bombs {
                                standing += 1;
                                if set_off.get(&bomb.id).is_none_or(|at| frame - at >= DESTRUCT_RETRY) {
                                    set_off.insert(bomb.id, frame);
                                    commands.push(Command::SelfDestruct { unit: bomb.id });
                                }
                            }
                            let all_gone = standing == 0 && frame - spawned >= FPS;
                            if all_gone {
                                *waves_left -= 1;
                                *spawned_at = None;
                                if *waves_left == 0 {
                                    *stage = Sweep::Rest { since: frame };
                                }
                            }
                        }
                    }
                }
                Sweep::Bombs { .. } => {}
                Sweep::Rest { since } => {
                    if frame - *since >= REST {
                        *stage = Sweep::Done;
                    }
                }
                Sweep::Done => {}
            }
            None
        }
    }
}

fn conclude(duel: &mut Duel, started: i32, frame: i32, first_damage: Option<i32>, reason: &'static str) -> DuelResult {
    let [x, y] = &duel.armies;
    let survivors = [x.alive.len() as u32, y.alive.len() as u32];
    let winner = match (reason, survivors) {
        ("wiped", [0, 0]) => "draw",
        ("wiped", [_, 0]) => "x",
        ("wiped", [0, _]) => "y",
        _ => "draw",
    };
    let result = DuelResult {
        match_index: 0,
        site: 0,
        sequence: duel.sequence,
        job: duel.job.clone(),
        count: [x.count, y.count],
        metal: [x.count as f32 * x.metal_each, y.count as f32 * y.metal_each],
        winner,
        reason,
        seconds: (frame - started) as f32 / FPS as f32,
        contact_seconds: first_damage.map(|f| (f - started) as f32 / FPS as f32),
        survivors,
        value_left: [x.value_left(), y.value_left()],
        damage_taken: [x.damage_taken, y.damage_taken],
    };
    duel.phase = Phase::Clearing { since: frame, sweep: Sweep::Survivors };
    result
}
