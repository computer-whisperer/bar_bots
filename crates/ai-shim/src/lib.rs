//! Skirmish AI library for the Recoil engine's C AI interface.
//!
//! The engine loads this as `libSkirmishAI.so` (renamed by `run/install_ai.sh`) and calls the
//! three exports below (`SSkirmishAILibrary.h`). The shim holds no strategy: it relays events
//! and state snapshots to the bot process and applies the commands it gets back. See `DESIGN.md`.

mod engine;
mod link;
mod script;

use std::collections::BTreeMap;
use std::ffi::{c_int, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

use bot_protocol::{BuildSite, Command, Commands, Event, Tick, ToBot, UnitDefId, UnitId, Vec3};
use recoil_ai_sys as sys;

use engine::Engine;
use link::Link;

/// Frames between ticks sent to the bot (the sim runs 30 per second).
const TICK_INTERVAL: i32 = 15;
const RECONNECT_INTERVAL: i32 = 30;
/// Frames between heartbeat lines in the engine log; the arena reads game time from them.
const HEARTBEAT_INTERVAL: i32 = 30 * 30;
/// Frames between census lines (`WITHIN_REASON_OBSERVE`): what the opponent owns, for studying how it plays.
const CENSUS_INTERVAL: i32 = 60 * 30;
/// Frames between ground-truth samples of the opponent's units (`WITHIN_REASON_OBSERVE` with `WITHIN_REASON_TRUTH_DIR`).
const TRUTH_INTERVAL: i32 = 2 * 30;
/// Frames between looks at the map's features (wrecks); a multiple of the tick interval.
const WRECKS_INTERVAL: i32 = 3 * 30;

struct Instance {
    ai_id: c_int,
    engine: Engine,
    link: Option<Link>,
    /// The bot has answered the previous message, so it may be sent another.
    has_credit: bool,
    lockstep: bool,
    /// Where the opponent's true state is written for post-game analysis; the bot never sees it.
    truth: Option<std::io::BufWriter<std::fs::File>>,
    events: Vec<Event>,
    /// Units to create once the export has let go of the instance table (see `engine::Spawner`).
    spawns: Vec<(UnitDefId, Vec3)>,
}

// SAFETY: the engine only calls the exports from its own thread; the raw callback pointer
// inside `Engine` never leaves that thread. The mutex only guards the instance table.
unsafe impl Send for Instance {}

/// One entry per AI instance; the engine may run several from one library.
static INSTANCES: Mutex<BTreeMap<c_int, Instance>> = Mutex::new(BTreeMap::new());

/// A panic in one export must not take the other instances down with it, so poisoning is ignored.
fn instances() -> std::sync::MutexGuard<'static, BTreeMap<c_int, Instance>> {
    INSTANCES.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Instance {
    fn log(&self, text: impl std::fmt::Display) {
        eprintln!("[wreason ai={}] {text}", self.ai_id);
    }

    fn update(&mut self, frame: i32) {
        if frame % HEARTBEAT_INTERVAL == 0 {
            self.log(format_args!("heartbeat f={frame}"));
            // For the arena's referee, which may call a settled game (`WITHIN_REASON_BALANCE`).
            if std::env::var_os("WITHIN_REASON_BALANCE").is_some() {
                let ((army, extractors), (their_army, their_extractors)) = self.engine.balance();
                self.log(format_args!("balance f={frame} ours={army:.0}/{extractors} theirs={their_army:.0}/{their_extractors}"));
            }
        }
        if frame % TRUTH_INTERVAL == 0 && let Some(truth) = &mut self.truth {
            use std::io::Write;
            let units = self.engine.enemy_truth();
            let _ = writeln!(truth, "{{\"f\":{frame},\"enemy\":{units}}}").and_then(|()| truth.flush());
        }
        if frame % CENSUS_INTERVAL == 0 && std::env::var_os("WITHIN_REASON_OBSERVE").is_some() {
            let census = self.engine.enemy_census();
            self.log(format_args!("census f={frame} enemy {census}"));
            let census = self.engine.own_census();
            self.log(format_args!("census f={frame} own {census}"));
        }
        if self.link.is_none() && frame % RECONNECT_INTERVAL == 0 {
            self.connect(frame);
        }
        let Some(link) = &mut self.link else { return };
        match link.poll() {
            Ok(Some(commands)) => {
                self.has_credit = true;
                self.apply(commands);
            }
            Ok(None) => {}
            Err(e) => return self.disconnect(e),
        }
        if self.has_credit && frame % TICK_INTERVAL == 0 {
            let mut snapshot = self.engine.snapshot();
            if frame % WRECKS_INTERVAL == 0 {
                snapshot.wrecks = Some(self.engine.wrecks());
            }
            let tick = Tick { frame, events: std::mem::take(&mut self.events), snapshot };
            self.send(ToBot::Tick(tick));
            // Lockstep (`WITHIN_REASON_LOCKSTEP`, headless study runs only): the engine waits here for the answer,
            // so a bot that holds its reply while a language model thinks has in effect paused the game.
            if self.lockstep && let Some(link) = &mut self.link {
                match link.wait() {
                    Ok(commands) => {
                        self.has_credit = true;
                        self.apply(commands);
                    }
                    Err(e) => self.disconnect(e),
                }
            }
        }
    }

    fn connect(&mut self, frame: i32) {
        // A missing bot is the normal idle state, not worth a log line every second.
        let Ok(link) = Link::connect() else { return };
        self.link = Some(link);
        self.log(format_args!("connected to bot at frame {frame}"));
        let hello = self.engine.hello(frame);
        self.send(ToBot::Hello(hello));
    }

    fn send(&mut self, message: ToBot) {
        self.has_credit = false;
        let Some(link) = &mut self.link else { return };
        if let Err(e) = link.send(&message) {
            self.disconnect(e);
        }
    }

    fn disconnect(&mut self, error: std::io::Error) {
        self.log(format_args!("lost bot connection: {error}"));
        self.link = None;
        self.has_credit = false;
    }

    fn apply(&mut self, commands: Commands) {
        for mut command in commands.0 {
            if let Command::Build { unit, def, site: Some(site), .. } = &mut command {
                match self.engine.find_build_site(*def, *site) {
                    Some(pos) => {
                        if std::env::var_os("WITHIN_REASON_TRACE_BUILDS").is_some() {
                            self.log(format_args!(
                                "build unit={} def={} wanted=({:.0},{:.0}) placed=({:.0},{:.0})",
                                unit.0, def.0, site.near.x, site.near.z, pos.x, pos.z
                            ));
                        }
                        *site = BuildSite { near: pos, ..*site }
                    }
                    None => {
                        if std::env::var_os("WITHIN_REASON_TRACE_BUILDS").is_some() {
                            let what = self.engine.describe_site(*def, site.near, 64.0);
                            self.log(format_args!(
                                "no site unit={} def={} wanted=({:.0},{:.0}) {what}", unit.0, def.0, site.near.x, site.near.z
                            ));
                        }
                        self.events.push(Event::BuildSiteNotFound { unit: *unit, def: *def });
                        continue;
                    }
                }
            }
            if let Command::GiveUnit { def, at } = command {
                self.spawns.push((def, at));
                continue;
            }
            if let Err(code) = self.engine.issue(&command) {
                let (Command::Build { unit, .. }
                | Command::Move { unit, .. }
                | Command::Fight { unit, .. }
                | Command::Stop { unit }
                | Command::SetRepeat { unit, .. }
                | Command::Guard { unit, .. }
                | Command::ReclaimArea { unit, .. }
                | Command::ReclaimFeature { unit, .. }
                | Command::Resurrect { unit, .. }
                | Command::Repair { unit, .. }
                | Command::SelfDestruct { unit }) = command
                else {
                    continue;
                };
                self.events.push(Event::CommandRejected { unit, code });
            }
        }
    }

    /// # Safety
    /// `data` must point to the event struct matching `topic`.
    unsafe fn handle_event(&mut self, topic: sys::EventTopic, data: *const c_void) {
        /// The engine passes -1 for "no such unit".
        fn unit(id: c_int) -> Option<UnitId> {
            (id >= 0).then_some(UnitId(id))
        }
        macro_rules! event {
            ($ty:ident) => {
                unsafe { &*data.cast::<sys::$ty>() }
            };
        }
        let event = match topic {
            sys::EVENT_UPDATE => return self.update(event!(SUpdateEvent).frame),
            sys::EVENT_UNIT_CREATED => {
                let e = event!(SUnitCreatedEvent);
                Event::UnitCreated { unit: UnitId(e.unit), builder: unit(e.builder) }
            }
            sys::EVENT_UNIT_FINISHED => Event::UnitFinished { unit: UnitId(event!(SUnitFinishedEvent).unit) },
            sys::EVENT_UNIT_IDLE => Event::UnitIdle { unit: UnitId(event!(SUnitIdleEvent).unit) },
            sys::EVENT_UNIT_MOVE_FAILED => {
                Event::UnitMoveFailed { unit: UnitId(event!(SUnitMoveFailedEvent).unit) }
            }
            sys::EVENT_UNIT_DAMAGED => {
                let e = event!(SUnitDamagedEvent);
                Event::UnitDamaged { unit: UnitId(e.unit), attacker: unit(e.attacker), damage: e.damage }
            }
            sys::EVENT_UNIT_DESTROYED => {
                let e = event!(SUnitDestroyedEvent);
                Event::UnitDestroyed { unit: UnitId(e.unit), attacker: unit(e.attacker) }
            }
            sys::EVENT_ENEMY_ENTER_LOS => Event::EnemyEnterLos { enemy: UnitId(event!(SEnemyEnterLOSEvent).enemy) },
            sys::EVENT_ENEMY_LEAVE_LOS => Event::EnemyLeaveLos { enemy: UnitId(event!(SEnemyLeaveLOSEvent).enemy) },
            sys::EVENT_ENEMY_DESTROYED => {
                Event::EnemyDestroyed { enemy: UnitId(event!(SEnemyDestroyedEvent).enemy) }
            }
            _ => return,
        };
        self.events.push(event);
    }
}

/// Runs `body` so that a panic cannot unwind into the engine.
fn guarded(ai_id: c_int, body: impl FnOnce() -> c_int) -> c_int {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or_else(|_| {
        eprintln!("[wreason ai={ai_id}] panic in AI library");
        -1
    })
}

/// # Safety
/// `callback` must be the engine's callback table for `skirmish_ai_id`, valid until `release`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn init(skirmish_ai_id: c_int, callback: *const sys::SSkirmishAICallback) -> c_int {
    guarded(skirmish_ai_id, || {
        let instance = Instance {
            ai_id: skirmish_ai_id,
            engine: unsafe { Engine::new(skirmish_ai_id, callback) },
            link: None,
            has_credit: false,
            lockstep: std::env::var_os("WITHIN_REASON_LOCKSTEP").is_some(),
            truth: std::env::var_os("WITHIN_REASON_OBSERVE")
                .and(std::env::var_os("WITHIN_REASON_TRUTH_DIR"))
                .and_then(|dir| std::fs::File::create(std::path::Path::new(&dir).join(format!("truth-{skirmish_ai_id}.jsonl"))).ok())
                .map(std::io::BufWriter::new),
            events: Vec::new(),
            spawns: Vec::new(),
        };
        instance.log("init");
        instances().insert(skirmish_ai_id, instance);
        0
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn release(skirmish_ai_id: c_int) -> c_int {
    guarded(skirmish_ai_id, || {
        if let Some(instance) = instances().remove(&skirmish_ai_id) {
            instance.log("release");
        }
        0
    })
}

/// # Safety
/// `data` must point to the event struct matching `topic`, as the engine guarantees.
#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub unsafe extern "C" fn handleEvent(skirmish_ai_id: c_int, topic: c_int, data: *const c_void) -> c_int {
    guarded(skirmish_ai_id, || {
        let (spawner, spawns) = {
            let mut instances = instances();
            let Some(instance) = instances.get_mut(&skirmish_ai_id) else { return -1 };
            if !data.is_null() {
                unsafe { instance.handle_event(topic as sys::EventTopic, data) };
            }
            (instance.engine.spawner(), std::mem::take(&mut instance.spawns))
        };
        // The table is unlocked here: the engine re-enters this function with the new unit's events.
        for (def, at) in spawns {
            if let Err(code) = spawner.give(def, at) {
                eprintln!("[wreason ai={skirmish_ai_id}] give unit def={} at ({:.0},{:.0}) refused: {code}", def.0, at.x, at.z);
            }
        }
        0
    })
}
