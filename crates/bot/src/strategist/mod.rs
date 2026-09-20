//! The Claude Code session beside the brain: it watches the game through our MCP server and pulls the levers the
//! brain exposes. Never in the control loop; see `DESIGN.md`. Two ways to run it:
//!
//! - **Strategist**: Opus, a turn every 45 s of game time or on a trigger, standing directives only.
//! - **Commander**: Sonnet, turns back to back at game speed 1, with squads, the unit mix and turret requests.

mod mcp;
mod report;
pub mod shared;
mod transcript;

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use mcp::McpServer;
use shared::Shared;
use transcript::Transcript;

/// `claude --effort` unless `WITHIN_REASON_EFFORT` (the arena's `--effort`) says otherwise. The game is held still during a
/// turn, so thinking costs wall time only; against people in real time it will want to be `low`.
const DEFAULT_EFFORT: &str = "high";

/// Sessions run on this subscription unless `WITHIN_REASON_CLAUDE_CONFIG_DIR` says otherwise: it has extra usage
/// (paid credits) disabled, so it can be blocked but never charged (`docs/harness/claude-p.md`).
const DEFAULT_CLAUDE_CONFIG_DIR: &str = ".claude2";
/// Game time between the strategist's routine turns when nothing triggers one sooner.
const ROUTINE_INTERVAL_FRAMES: i32 = 45 * 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Strategist,
    Commander,
}

impl Mode {
    fn model(self) -> &'static str {
        match self {
            Mode::Strategist => "claude-opus-5",
            Mode::Commander => "claude-sonnet-5",
        }
    }

    fn system_prompt(self) -> &'static str {
        match self {
            Mode::Strategist => include_str!("prompt.md"),
            // The role, then what the project knows (`docs/README.md`: the brief is rewritten from the knowledge base).
            Mode::Commander => concat!(include_str!("commander.md"), include_str!("../../../../docs/briefs/commander.md")),
        }
    }

    /// Turns after which the session is replaced by a fresh one that is handed the notes, to bound its context.
    fn turns_per_session(self) -> usize {
        match self {
            Mode::Strategist => usize::MAX,
            Mode::Commander => 40,
        }
    }
}

pub struct Strategist {
    pub shared: Arc<Shared>,
    _server: McpServer,
    stop: Arc<AtomicBool>,
}

/// What it takes to start a session, kept so the driver can start the next one.
struct Launch {
    mode: Mode,
    cwd: PathBuf,
    config_dir: PathBuf,
    /// `claude --effort`: stated, never inherited, so a transcript can be compared with another.
    effort: String,
    mcp_config: String,
    transcript: Arc<Transcript>,
}

struct Session {
    child: Child,
    stdin: ChildStdin,
    turn_done: Receiver<()>,
}

impl Strategist {
    /// Starts the MCP server and the Claude Code session. `dir` receives `strategist-N.jsonl`.
    pub fn start(dir: &Path, ai_id: i32, mode: Mode) -> std::io::Result<Self> {
        let shared = Arc::new(Shared::default());
        shared.lockstep.store(mode == Mode::Commander, Ordering::Relaxed);
        // How late the commander's orders land, in game seconds per wall second of thought (arena `--think-penalty`).
        *shared.think_penalty.lock().unwrap() = std::env::var("WITHIN_REASON_THINK_PENALTY").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let transcript = Arc::new(Transcript::create(&dir.join(format!("strategist-{ai_id}.jsonl")))?);
        let server = McpServer::start(shared.clone(), transcript.clone())?;
        // An empty working directory: nothing for the session to discover.
        let cwd = dir.join(format!("strategist-{ai_id}-cwd"));
        std::fs::create_dir_all(&cwd)?;
        let mcp_config = json!({ "mcpServers": { "wreason": { "type": "http", "url": format!("http://127.0.0.1:{}/mcp", server.port) } } });
        let config_dir = std::env::var_os("WITHIN_REASON_CLAUDE_CONFIG_DIR").map_or_else(
            || PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(DEFAULT_CLAUDE_CONFIG_DIR),
            Into::into,
        );
        let effort = std::env::var("WITHIN_REASON_EFFORT").ok().filter(|e| !e.is_empty()).unwrap_or_else(|| DEFAULT_EFFORT.into());
        let launch = Launch { mode, cwd, config_dir, effort, mcp_config: mcp_config.to_string(), transcript };
        let session = launch.spawn()?;
        eprintln!(
            "[ai {ai_id}] {mode:?} started ({}, effort {}, account {}, MCP on port {})",
            mode.model(), launch.effort, launch.config_dir.display(), server.port
        );
        let stop = Arc::new(AtomicBool::new(false));
        let (driver_shared, driver_stop) = (shared.clone(), stop.clone());
        std::thread::spawn(move || drive(launch, session, &driver_shared, &driver_stop, ai_id));
        Ok(Strategist { shared, _server: server, stop })
    }
}

impl Drop for Strategist {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Launch {
    fn spawn(&self) -> std::io::Result<Session> {
        let mut child = Command::new("claude")
            .current_dir(&self.cwd)
            .env("CLAUDE_CONFIG_DIR", &self.config_dir)
            .args(["-p", "--model", self.mode.model(), "--tools", "", "--strict-mcp-config", "--mcp-config"])
            .arg(&self.mcp_config)
            .args(["--allowedTools", "mcp__wreason__*", "--permission-mode", "dontAsk", "--setting-sources", ""])
            .args(["--effort", &self.effort])
            .args(["--system-prompt", self.mode.system_prompt()])
            .args(["--input-format", "stream-json", "--output-format", "stream-json", "--verbose"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let (turn_done_tx, turn_done) = channel();
        let transcript = self.transcript.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
                let kind = message["type"].as_str().unwrap_or_default();
                if matches!(kind, "assistant" | "result" | "rate_limit_event") {
                    transcript.record(json!({ "kind": kind, "message": message }));
                }
                // We spend weekly allotments only: the first sign of paid overage, or of any limit, ends the session.
                let limit = &message["rate_limit_info"];
                if kind == "rate_limit_event"
                    && (limit["isUsingOverage"].as_bool() == Some(true) || limit["status"].as_str().is_some_and(|s| s != "allowed"))
                {
                    eprintln!("[strategist] STOPPING: rate limit event {limit}");
                    transcript.record(json!({ "kind": "stopped", "reason": limit }));
                    break;
                }
                if kind == "result" && turn_done_tx.send(()).is_err() {
                    break;
                }
            }
        });
        Ok(Session { child, stdin, turn_done })
    }
}

impl Session {
    fn end(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Sends one turn at a time. The strategist is called on a timer and on triggers while the game runs on; the
/// commander is called when the brain asks (its wake conditions), and the brain holds the game until the turn ends.
fn drive(launch: Launch, mut session: Session, shared: &Shared, stop: &AtomicBool, ai_id: i32) {
    let mode = launch.mode;
    let mut last_turn_frame = i32::MIN / 2;
    let mut turns_this_session = 0;
    let mut seen = report::Seen::default();
    let mut owed_result = false;
    while !stop.load(Ordering::Relaxed) {
        let headline = match mode {
            Mode::Commander => match shared.next_turn_request(stop) {
                Some(reason) => reason,
                None => break,
            },
            Mode::Strategist => {
                std::thread::sleep(Duration::from_millis(200));
                let frame = shared.briefing.lock().unwrap().frame;
                let triggers = std::mem::take(&mut *shared.triggers.lock().unwrap());
                if frame == 0 || (triggers.is_empty() && frame - last_turn_frame < ROUTINE_INTERVAL_FRAMES) {
                    continue;
                }
                if triggers.is_empty() { "Routine check.".to_string() } else { triggers.join(" ") }
            }
        };
        if turns_this_session >= mode.turns_per_session() {
            session.end();
            session = match launch.spawn() {
                Ok(next) => next,
                Err(e) => {
                    eprintln!("[ai {ai_id}] could not restart the session: {e}; heuristics carry on alone");
                    shared.close_gate();
                    return;
                }
            };
            turns_this_session = 0;
        }
        // The last turn ended at its `wait`; the session may still be writing its closing words, and takes no new
        // prompt until it has reported that response finished.
        if owed_result {
            owed_result = false;
            let arrived = loop {
                match session.turn_done.recv_timeout(Duration::from_millis(250)) {
                    Ok(()) => break true,
                    Err(RecvTimeoutError::Timeout) if !stop.load(Ordering::Relaxed) => {}
                    Err(_) => break false,
                }
            };
            if !arrived {
                shared.end_turn();
                break;
            }
        }
        let (frame, game_time) = {
            let briefing = shared.briefing.lock().unwrap();
            (briefing.frame, briefing.game_time.clone())
        };
        let prompt = match mode {
            Mode::Strategist => format!("Game time {game_time}. {headline}"),
            Mode::Commander => commander_prompt(&game_time, &headline, shared, &mut seen, turns_this_session == 0),
        };
        last_turn_frame = frame;
        turns_this_session += 1;
        let started = Instant::now();
        launch.transcript.record(json!({ "kind": "turn", "frame": frame, "prompt": prompt }));
        let line = json!({ "type": "user", "message": { "role": "user", "content": prompt } });
        let sent = writeln!(session.stdin, "{line}").and_then(|()| session.stdin.flush()).is_ok();
        let finished = sent
            && loop {
                match session.turn_done.recv_timeout(Duration::from_millis(50)) {
                    Ok(()) => break true,
                    // The commander called `wait`: the game is running again, the response's tail is owed.
                    Err(RecvTimeoutError::Timeout) if mode == Mode::Commander && !shared.turn_in_progress() => {
                        owed_result = true;
                        break true;
                    }
                    Err(RecvTimeoutError::Timeout) if !stop.load(Ordering::Relaxed) => {}
                    Err(_) => break false,
                }
            };
        launch.transcript.record(json!({ "kind": "turn_end", "wall_seconds": started.elapsed().as_secs_f32(), "ended_by": if owed_result { "wait" } else { "response" } }));
        shared.end_turn();
        if !finished {
            if !stop.load(Ordering::Relaxed) {
                eprintln!("[ai {ai_id}] {mode:?} session ended; heuristics carry on alone");
            }
            break;
        }
    }
    shared.close_gate();
    session.end();
}

/// The commander is shown the picture outright (a tool call to look would double its turn), in full at the start of
/// a session and as changes afterwards.
fn commander_prompt(game_time: &str, headline: &str, shared: &Shared, seen: &mut report::Seen, fresh_session: bool) -> String {
    let briefing = shared.briefing.lock().unwrap().clone();
    let field = shared.field.lock().unwrap().clone();
    let fights: Vec<String> =
        std::mem::take(&mut *shared.fights.lock().unwrap()).into_iter().map(|(what, n)| format!("{what} x{n}")).collect();
    let mut prompt = String::new();
    if fresh_session {
        let notes = shared.notes.lock().unwrap();
        if !notes.is_empty() {
            prompt += &format!("You are taking over mid-game from an earlier session of yourself. Its notes:\n{}\n\n", notes.join("\n"));
        }
        prompt += &format!("Map: {}\n\n", shared.map.lock().unwrap());
    }
    let wake = serde_json::to_string(&*shared.wake.lock().unwrap()).unwrap_or_default();
    prompt += &format!(
        "[{game_time}] Woken because: {headline}\n{}\nwake conditions in force: {wake}",
        report::report(seen, &briefing, &field, &fights, fresh_session)
    );
    prompt
}
