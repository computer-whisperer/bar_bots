//! The pianist (`docs/design/2026-09-21-pianist.md`): Opus plays the game, Jev plays the keyboard. Once a game
//! second the brain writes a picture of the game (`picture.rs`), offers a menu for every actor that is free or due
//! for review (`menu.rs`), asks Jev once for all of them, and plays the answers through the actuators (`hands.rs`,
//! `groups.rs`). No decision heuristic runs beneath it; the control lane (`micro.rs`) and the tracking do.
//!
//! In lockstep the call holds the game (the arena's shim waits for the reply); against a live engine it costs a
//! late tick a second. The call fails safe: every actor keeps its task until the next answer.

mod groups;
mod hands;
mod menu;
mod picture;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs::File;
use std::io::Write as _;
use std::path::Path;

use bot_protocol::{Command, Event, Tick, UnitDefId, UnitId, Vec3};
use serde_json::json;

use super::economy::FIRST_ORDER_FRAME;
use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};
pub(super) use groups::{Group, GroupTask};
pub(super) use picture::{Party, Place};

/// Game seconds between calls (`WITHIN_REASON_JEV_INTERVAL` overrides).
const INTERVAL_SECONDS: f32 = 1.0;
/// A busy actor is asked again this often, with "continue" on its menu.
pub(super) const REVIEW_FRAMES: i32 = 10 * FRAMES_PER_SECOND;
/// H-HANDS-SWITCH: a busy actor changes course only when the winning option beats "continue" by this much probability.
pub(super) const SWITCH_MARGIN: f64 = 0.15;
/// Events the picture shows, and for how long.
const RECENT_EVENTS: usize = 12;
const RECENT_FRAMES: i32 = 90 * FRAMES_PER_SECOND;
/// H-HANDS-REFUSED: a spot where the engine refused an extractor is left off every menu for this long (smoke-4: the
/// commander asked for the same refused spot thirty times running beside the enemy base, and died there).
const REFUSED_FRAMES: i32 = 90 * FRAMES_PER_SECOND;
/// H-HANDS-PLAYER-WAKE: the player is woken when Jev says the game needs it for this many calls running, at most this
/// often.
const NEEDS_PLAYER_PROBABILITY: f64 = 0.8;
const NEEDS_PLAYER_CALLS: u32 = 3;
const NEEDS_PLAYER_COOLDOWN: i32 = 60 * FRAMES_PER_SECOND;

/// What a builder or a lab is committed to.
#[derive(Clone, Debug)]
pub(super) enum Task {
    /// A build order: the type, the site asked for, the spot if an extractor, when it was ordered, whether the
    /// engine has started it (a nanoframe by this builder since the order).
    Build { def: UnitDefId, near: Vec3, spot: Option<usize>, ordered: i32, started: bool },
    Assist { lab: UnitId, since: i32 },
    Reclaim { at: Vec3, since: i32 },
    Repair { target: UnitId, since: i32 },
    Walk { to: Vec3, place: String, since: i32 },
}

impl Task {
    fn since(&self) -> i32 {
        match self {
            Task::Build { ordered, .. } => *ordered,
            Task::Assist { since, .. } | Task::Reclaim { since, .. } | Task::Repair { since, .. } | Task::Walk { since, .. } => *since,
        }
    }
}

#[derive(Default)]
struct Stats {
    calls: u32,
    errors: u32,
    latencies_ms: Vec<f32>,
    tokens: u64,
    questions: u32,
    switches: u32,
    kept: u32,
}

pub struct Pianist {
    client: jev::Client,
    interval_frames: i32,
    last_ask_frame: i32,
    /// Builders' and labs' tasks, by unit.
    pub(super) tasks: HashMap<UnitId, Task>,
    /// When each builder, lab or group (by name) was last asked.
    last_asked: HashMap<String, i32>,
    /// Units a lab has been told to build and not yet started, oldest first.
    pub(super) lab_queue: HashMap<UnitId, Vec<(UnitDefId, i32)>>,
    pub(super) groups: Vec<Group>,
    next_group: usize,
    /// Spots where the engine refused an extractor, and until when they are left off the menus (H-HANDS-REFUSED).
    pub(super) refused_spots: HashMap<usize, i32>,
    /// What the last picture named, so an answer's place or party can be looked up.
    pub(super) places: Vec<Place>,
    pub(super) parties: Vec<Party>,
    /// Things worth telling: (frame, text).
    recent: VecDeque<(i32, String)>,
    /// What the hands did this call, for the player's report (`Shared.hands`).
    pub(super) done: Vec<String>,
    needs_player_run: u32,
    last_player_wake: i32,
    /// The whole request and answer per call, when `WITHIN_REASON_JEV_LOG` is set (`docs/harness/record-format.md`,
    /// "The pianist's log").
    log: Option<File>,
    /// The instructions as last written to the log: a call carries them only when they changed.
    logged_instructions: String,
    /// What the hands did with this call's answers (`hands.rs`), for the log line.
    pub(super) played: Vec<serde_json::Value>,
    stats: Stats,
}

/// The pianist's log format version (`docs/harness/record-format.md`).
const LOG_VERSION: u32 = 1;

impl Pianist {
    /// The client from the environment (the key file or `TYPESAFE_API_KEY`); `Err` says why there is none.
    pub fn from_env(log_dir: &Path, ai_id: i32) -> Result<Pianist, String> {
        let client = jev::Client::from_env().map_err(|e| e.to_string())?;
        let seconds: f32 = std::env::var("WITHIN_REASON_JEV_INTERVAL").ok().and_then(|v| v.parse().ok()).unwrap_or(INTERVAL_SECONDS);
        let log = match std::env::var("WITHIN_REASON_JEV_LOG").ok().filter(|v| !v.is_empty() && v != "0") {
            Some(_) => File::create(log_dir.join(format!("jev-{ai_id}.jsonl"))).ok(),
            None => None,
        };
        Ok(Pianist {
            client,
            interval_frames: ((seconds * FRAMES_PER_SECOND as f32) as i32).max(super::BRAIN_FRAMES),
            last_ask_frame: i32::MIN / 2,
            tasks: HashMap::new(),
            last_asked: HashMap::new(),
            lab_queue: HashMap::new(),
            groups: Vec::new(),
            next_group: 0,
            refused_spots: HashMap::new(),
            places: Vec::new(),
            parties: Vec::new(),
            recent: VecDeque::new(),
            done: Vec::new(),
            needs_player_run: 0,
            last_player_wake: i32::MIN / 2,
            log,
            logged_instructions: String::new(),
            played: Vec::new(),
            stats: Stats::default(),
        })
    }

    /// The log's first line: what every call shares.
    pub fn log_header(&mut self, ai_id: i32, rules: &str) {
        if let Some(log) = &mut self.log {
            let line = json!({
                "t": "header", "format": "within-reason-jev", "version": LOG_VERSION, "ai_id": ai_id, "model": self.client.model(),
                "interval_frames": self.interval_frames, "rules": rules,
            });
            let _ = writeln!(log, "{line}");
        }
    }

    pub fn model(&self) -> &str {
        self.client.model()
    }

    pub(super) fn note(&mut self, frame: i32, text: String) {
        if self.recent.len() == RECENT_EVENTS {
            self.recent.pop_front();
        }
        self.recent.push_back((frame, text));
    }

    pub(super) fn recent(&self, frame: i32) -> Vec<String> {
        self.recent.iter().filter(|(f, _)| frame - f < RECENT_FRAMES).map(|(f, text)| format!("{} {text}", picture::clock(*f))).collect()
    }

    /// A new group's name: A, B, C, ...
    pub(super) fn new_group_name(&mut self) -> String {
        let n = self.next_group;
        self.next_group += 1;
        let letter = (b'A' + (n % 26) as u8) as char;
        if n < 26 { letter.to_string() } else { format!("{letter}{}", n / 26) }
    }
}

impl Brain {
    /// The pianist's whole turn of the brain: bookkeeping every think, a call to Jev when one is due.
    pub(super) fn run_pianist(&mut self, tick: &Tick, kit: &Kit, commands: &mut Vec<Command>) {
        self.pianist_housekeeping(tick, kit);
        self.keep_groups(tick, kit, commands);
        // The growth history (the curves and the stagnation wake) and the field the player's report and wake
        // conditions read; the heuristic brain keeps them inside its army rules.
        self.track_growth(tick, kit);
        if let Some(shared) = self.strategist.clone() {
            let soldiers: Vec<&bot_protocol::OwnUnit> = tick.snapshot.own_units.iter().filter(|u| !u.being_built && self.is_army(u, kit)).collect();
            self.publish_field(tick, kit, &soldiers, &shared);
        }
        if tick.frame < FIRST_ORDER_FRAME {
            return;
        }
        let due = {
            let pianist = self.pianist.as_ref().expect("pianist mode");
            tick.frame - pianist.last_ask_frame >= pianist.interval_frames
        };
        if !due {
            return;
        }
        self.pianist.as_mut().expect("pianist mode").last_ask_frame = tick.frame;
        let picture = self.picture(tick, kit);
        let menus = self.menus(tick, kit, &picture);
        if menus.is_empty() {
            return;
        }
        let mut questions: BTreeMap<String, jev::Question> = BTreeMap::new();
        for menu in &menus {
            for (id, question) in &menu.questions {
                questions.insert(id.clone(), question.clone());
            }
        }
        let request = jev::Request { state: picture.state.clone(), questions };
        let (response, ai) = {
            let pianist = self.pianist.as_mut().expect("pianist mode");
            if pianist.stats.calls == 0 && pianist.logged_instructions.is_empty() {
                let rules = picture.state["rules"].as_str().unwrap_or_default().to_string();
                pianist.log_header(self.world.hello.ai_id, &rules);
            }
            pianist.places = picture.places.clone();
            pianist.parties = picture.parties.clone();
            pianist.stats.calls += 1;
            pianist.stats.questions += request.questions.len() as u32;
            pianist.played.clear();
            (pianist.client.ask(&request), self.world.hello.ai_id)
        };
        match response {
            Ok(response) => {
                {
                    let pianist = self.pianist.as_mut().expect("pianist mode");
                    pianist.stats.latencies_ms.push(response.latency.as_secs_f32() * 1000.0);
                    pianist.stats.tokens += response.usage["input_tokens"].as_u64().unwrap_or(0);
                }
                self.play(tick, kit, &picture, menus, &response.answers, commands);
                self.pianist_globals(tick, &response.answers);
                self.publish_hands(&picture, &response.answers);
                self.log_call(tick, &request, &response);
            }
            Err(e) => {
                let pianist = self.pianist.as_mut().expect("pianist mode");
                pianist.stats.errors += 1;
                if let Some(log) = &mut pianist.log {
                    let _ = writeln!(log, "{}", json!({ "t": "error", "f": tick.frame, "error": e.to_string() }));
                }
                eprintln!("[ai {ai}] f={} pianist: {e}; every actor keeps its task", tick.frame);
            }
        }
        if tick.due() % (60 * FRAMES_PER_SECOND) < self.pianist.as_ref().expect("pianist mode").interval_frames {
            self.pianist_status_line(tick.frame);
        }
    }

    /// One line of the log per call: the request (the instructions only when they changed, the rules never: they are
    /// in the header), the answers, what the hands played, and the groups, places and parties by name so a reader can
    /// draw them.
    fn log_call(&mut self, tick: &Tick, request: &jev::Request, response: &jev::Response) {
        let own = &tick.snapshot.own_units;
        let Some(pianist) = self.pianist.as_mut() else { return };
        if pianist.log.is_none() {
            return;
        }
        let mut state = request.state.clone();
        let instructions = state["instructions"].as_str().unwrap_or_default().to_string();
        if let Some(fields) = state.as_object_mut() {
            fields.remove("instructions");
            fields.remove("rules");
        }
        let changed = instructions != pianist.logged_instructions;
        if changed {
            pianist.logged_instructions = instructions.clone();
        }
        let groups: Vec<serde_json::Value> = pianist
            .groups
            .iter()
            .map(|g| {
                let units = g.units(own);
                let centre = groups::centre_of(&units);
                let task = match &g.task {
                    GroupTask::Hold { .. } => json!({ "kind": "hold" }),
                    GroupTask::Move { to, place, fight, .. } => json!({ "kind": if *fight { "fight_to" } else { "move_to" }, "place": place, "to": [to.x as i32, to.z as i32] }),
                    GroupTask::Engage { at, .. } => json!({ "kind": "engage", "to": [at.x as i32, at.z as i32] }),
                };
                json!({ "name": g.name, "members": g.members.iter().map(|id| id.0).collect::<Vec<_>>(), "at": centre.map(|c| [c.x as i32, c.z as i32]), "task": task })
            })
            .collect();
        let places: Vec<serde_json::Value> = pianist.places.iter().map(|p| json!({ "name": p.name, "x": p.at.x as i32, "z": p.at.z as i32, "spot": p.spot })).collect();
        let parties: Vec<serde_json::Value> = pianist.parties.iter().map(|p| json!({ "name": p.name, "ids": p.ids.iter().map(|id| id.0).collect::<Vec<_>>(), "x": p.at.x as i32, "z": p.at.z as i32, "metal": p.metal as i32, "composition": p.composition })).collect();
        let mut line = json!({
            "t": "call", "f": tick.frame, "ms": (response.latency.as_secs_f32() * 1000.0) as u32, "model": response.model, "usage": response.usage,
            "retries": response.retries, "state": state, "questions": request.questions, "answers": response.answers,
            "played": std::mem::take(&mut pianist.played), "groups": groups, "places": places, "parties": parties,
        });
        if changed {
            line["instructions"] = json!(instructions);
        }
        if let Some(log) = &mut pianist.log {
            let _ = writeln!(log, "{line}");
        }
    }

    /// Tasks and queues against what the engine says: builds started or refused, labs' units begun, units gone.
    fn pianist_housekeeping(&mut self, tick: &Tick, kit: &Kit) {
        let own = &tick.snapshot.own_units;
        let frame = tick.frame;
        let names: Vec<(UnitId, String, Vec3)> = own.iter().map(|u| (u.id, self.name(u.def).to_string(), u.pos)).collect();
        let mut notes: Vec<String> = Vec::new();
        let mut refused: Vec<(UnitId, UnitDefId, Vec3)> = Vec::new();
        let Some(mut pianist) = self.pianist.take() else { return };
        pianist.tasks.retain(|id, _| own.iter().any(|u| u.id == *id));
        pianist.lab_queue.retain(|id, _| own.iter().any(|u| u.id == *id));
        for event in &tick.events {
            match *event {
                Event::UnitCreated { unit, builder: Some(builder) } => {
                    if let Some(Task::Build { started, .. }) = pianist.tasks.get_mut(&builder) {
                        *started = true;
                    }
                    if let Some(queue) = pianist.lab_queue.get_mut(&builder) {
                        let made = own.iter().find(|u| u.id == unit).map(|u| u.def);
                        if let Some(i) = queue.iter().position(|(def, _)| Some(*def) == made) {
                            queue.remove(i);
                        } else if !queue.is_empty() {
                            queue.remove(0);
                        }
                    }
                }
                Event::UnitFinished { unit } => {
                    if let Some((_, name, pos)) = names.iter().find(|(id, _, _)| *id == unit)
                        && let Some(def) = own.iter().find(|u| u.id == unit).map(|u| u.def)
                        && self.world.def(def).is_some_and(|d| d.speed == 0.0)
                    {
                        notes.push(format!("finished {name} at {}", self.world.grid(*pos)));
                    }
                }
                Event::UnitDestroyed { unit, attacker } => {
                    if let Some((def, pos)) = self.known_units.get(&unit) {
                        let building = self.world.def(*def).is_some_and(|d| d.speed == 0.0 || *def == kit.commander || d.build_speed > 0.0);
                        if let Some(share) = self.abandoned(unit, attacker) {
                            notes.push(format!("abandoned an unfinished {} at {} ({:.0}% built): its builder was sent elsewhere and the frame decayed", self.name(*def), self.world.grid(*pos), share * 100.0));
                        } else if building {
                            let killer = attacker.and_then(|id| self.enemy_defs.get(&id)).map_or("something unseen".to_string(), |d| self.name(*d).to_string());
                            notes.push(format!("lost our {} at {} to {killer}", self.name(*def), self.world.grid(*pos)));
                        }
                    }
                }
                Event::EnemyDestroyed { enemy } => {
                    if let Some(def) = self.enemy_defs.get(&enemy) {
                        notes.push(format!("killed their {}", self.name(*def)));
                    }
                }
                _ => {}
            }
        }
        // A build the engine never started: the builder is idle again after the order's grace with no nanoframe.
        for unit in own.iter().filter(|u| u.idle && !u.being_built) {
            match pianist.tasks.get(&unit.id) {
                Some(Task::Build { def, near, ordered, started: false, .. }) if frame - ordered > super::economy::ORDER_GRACE_FRAMES => {
                    refused.push((unit.id, *def, *near));
                }
                Some(Task::Build { started: true, ordered, .. }) if frame - ordered > super::economy::ORDER_GRACE_FRAMES => {
                    pianist.tasks.remove(&unit.id);
                }
                Some(Task::Reclaim { since, .. }) | Some(Task::Repair { since, .. }) if frame - since > super::economy::ORDER_GRACE_FRAMES => {
                    pianist.tasks.remove(&unit.id);
                }
                Some(Task::Walk { to, since, .. }) if unit.pos.dist2d(*to) < 150.0 || frame - since > 40 * FRAMES_PER_SECOND => {
                    pianist.tasks.remove(&unit.id);
                }
                _ => {}
            }
        }
        for text in notes {
            pianist.note(frame, text);
        }
        self.pianist = Some(pianist);
        for (unit, def, near) in refused {
            self.dropped_orders += 1;
            // An extractor refused off its centre: that spot takes the exact centre from now on (as the ladder does).
            if def == kit.extractor
                && let Some(i) = self.world.hello.metal_spots.iter().position(|s| s.dist2d(near) < super::economy::MEX_PATCH + 1.0 && s.dist2d(near) > 1.0)
            {
                self.centre_only.insert(i);
            }
            let name = self.name(def).to_string();
            let refused_spot = (def == kit.extractor).then(|| self.world.hello.metal_spots.iter().position(|s| s.dist2d(near) < super::economy::MEX_PATCH + 1.0)).flatten();
            let pianist = self.pianist.as_mut().expect("pianist mode");
            pianist.tasks.remove(&unit);
            if let Some(i) = refused_spot {
                pianist.refused_spots.insert(i, frame + REFUSED_FRAMES);
            }
            pianist.note(frame, format!("the engine refused a {name} at {}: the site was bad", self.world.grid(near)));
            eprintln!("[ai {}] f={frame} pianist: {name} order for unit {} near ({:.0}, {:.0}) never started", self.world.hello.ai_id, unit.0, near.x, near.z);
        }
    }

    /// What the player's session reads (`Shared.hands`): the picture without the instructions and rules, the `did`
    /// lines since its last turn, the groups that began an engagement this call, the global probabilities.
    fn publish_hands(&mut self, picture: &picture::Picture, answers: &BTreeMap<String, jev::Answer>) {
        let Some(shared) = &self.strategist else { return };
        let pianist = self.pianist.as_mut().expect("pianist mode");
        let mut state = picture.state.clone();
        if let Some(fields) = state.as_object_mut() {
            fields.remove("instructions");
            fields.remove("rules");
        }
        let mut hands = shared.hands.lock().unwrap();
        hands.picture = state;
        hands.done.append(&mut pianist.done);
        hands.engaged = pianist.played.iter().filter(|p| p["did"].as_str().is_some_and(|d| d.starts_with("attack "))).filter_map(|p| p["actor"].as_str().map(str::to_string)).collect();
        hands.globals = answers.iter().filter_map(|(id, a)| id.strip_prefix("global.").map(|q| (q.to_string(), a.probability_of("yes")))).collect();
    }

    /// The global answers: the player's wake (H-HANDS-PLAYER-WAKE).
    fn pianist_globals(&mut self, tick: &Tick, answers: &BTreeMap<String, jev::Answer>) {
        let needs = answers.get("global.needs_player").map_or(0.0, |a| a.probability_of("yes"));
        let pianist = self.pianist.as_mut().expect("pianist mode");
        pianist.needs_player_run = if needs >= NEEDS_PLAYER_PROBABILITY { pianist.needs_player_run + 1 } else { 0 };
        if pianist.needs_player_run >= NEEDS_PLAYER_CALLS && tick.frame - pianist.last_player_wake >= NEEDS_PLAYER_COOLDOWN {
            pianist.last_player_wake = tick.frame;
            pianist.needs_player_run = 0;
            if let Some(shared) = &self.strategist {
                shared.trigger(format!("your hands say the situation needs you (probability {needs:.2} for three seconds running)"));
            }
        }
    }

    fn pianist_status_line(&mut self, frame: i32) {
        let pianist = self.pianist.as_mut().expect("pianist mode");
        let stats = std::mem::take(&mut pianist.stats);
        if stats.calls == 0 {
            return;
        }
        let mut latencies = stats.latencies_ms;
        latencies.sort_by(f32::total_cmp);
        let median = latencies.get(latencies.len() / 2).copied().unwrap_or(0.0);
        let max = latencies.last().copied().unwrap_or(0.0);
        eprintln!(
            "[ai {}] f={frame} pianist this minute: {} calls ({} failed), median {median:.0} ms, longest {max:.0}, {} tokens in, {} questions; busy actors changed course {} times, kept {}; groups {}, tasks {}",
            self.world.hello.ai_id, stats.calls, stats.errors, stats.tokens, stats.questions, stats.switches, stats.kept, pianist.groups.len(), pianist.tasks.len()
        );
    }
}
