//! The strategist: a `claude -p` session that watches the game through our MCP server and sets
//! directives for the brain. Never in the control loop; see `DESIGN.md`.

mod mcp;
pub mod shared;
mod transcript;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use serde_json::{Value, json};

use mcp::McpServer;
use shared::Shared;
use transcript::Transcript;

/// Game time between routine calls when nothing triggers one sooner.
const ROUTINE_INTERVAL_FRAMES: i32 = 45 * 30;
const MODEL: &str = "claude-opus-5";
/// Strategist sessions run on this subscription unless `WITHIN_REASON_CLAUDE_CONFIG_DIR` says otherwise: it has
/// extra usage (paid credits) disabled, so it can be blocked but never charged (`docs/harness/claude-p.md`).
const DEFAULT_CLAUDE_CONFIG_DIR: &str = ".claude2";

pub struct Strategist {
    pub shared: Arc<Shared>,
    _server: McpServer,
    child: Child,
}

impl Strategist {
    /// Starts the MCP server and the Claude Code session. `dir` receives `strategist.jsonl`.
    pub fn start(dir: &Path, ai_id: i32) -> std::io::Result<Self> {
        let shared = Arc::new(Shared::default());
        let transcript = Arc::new(Transcript::create(&dir.join(format!("strategist-{ai_id}.jsonl")))?);
        let server = McpServer::start(shared.clone(), transcript.clone())?;
        // An empty working directory: nothing for the session to discover.
        let cwd = dir.join(format!("strategist-{ai_id}-cwd"));
        std::fs::create_dir_all(&cwd)?;
        let mcp_config = json!({ "mcpServers": { "wreason": { "type": "http", "url": format!("http://127.0.0.1:{}/mcp", server.port) } } });
        let config_dir = std::env::var_os("WITHIN_REASON_CLAUDE_CONFIG_DIR").map_or_else(
            || std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(DEFAULT_CLAUDE_CONFIG_DIR),
            Into::into,
        );
        let mut child = Command::new("claude")
            .current_dir(&cwd)
            .env("CLAUDE_CONFIG_DIR", &config_dir)
            .args(["-p", "--model", MODEL, "--tools", "", "--strict-mcp-config", "--mcp-config"])
            .arg(mcp_config.to_string())
            .args(["--allowedTools", "mcp__wreason__*", "--permission-mode", "dontAsk", "--setting-sources", ""])
            .args(["--system-prompt", include_str!("prompt.md")])
            .args(["--input-format", "stream-json", "--output-format", "stream-json", "--verbose"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");

        let (turn_done_tx, turn_done) = channel();
        let reader_transcript = transcript.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
                let kind = message["type"].as_str().unwrap_or_default();
                if matches!(kind, "assistant" | "result" | "rate_limit_event") {
                    reader_transcript.record(json!({ "kind": kind, "message": message }));
                }
                // We spend weekly allotments only: the first sign of paid overage, or of any limit, ends the session.
                let limit = &message["rate_limit_info"];
                if kind == "rate_limit_event"
                    && (limit["isUsingOverage"].as_bool() == Some(true) || limit["status"].as_str().is_some_and(|s| s != "allowed"))
                {
                    eprintln!("[strategist] STOPPING: rate limit event {limit}");
                    reader_transcript.record(json!({ "kind": "stopped", "reason": limit }));
                    break;
                }
                if kind == "result" && turn_done_tx.send(()).is_err() {
                    break;
                }
            }
        });
        let driver_shared = shared.clone();
        std::thread::spawn(move || drive(stdin, turn_done, &driver_shared, &transcript, ai_id));
        eprintln!("[ai {ai_id}] strategist started ({MODEL}, account {}, MCP on port {})", config_dir.display(), server.port);
        Ok(Strategist { shared, _server: server, child })
    }
}

impl Drop for Strategist {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Sends one turn at a time: routinely, and at once when the brain raises a trigger. Triggers
/// that arrive while a turn is in flight are folded into the next one.
fn drive(mut stdin: ChildStdin, turn_done: Receiver<()>, shared: &Shared, transcript: &Transcript, ai_id: i32) {
    let mut last_turn_frame = i32::MIN / 2;
    loop {
        std::thread::sleep(Duration::from_millis(200));
        let (frame, game_time) = {
            let briefing = shared.briefing.lock().unwrap();
            (briefing.frame, briefing.game_time.clone())
        };
        if frame == 0 {
            continue;
        }
        let triggers = std::mem::take(&mut *shared.triggers.lock().unwrap());
        if triggers.is_empty() && frame - last_turn_frame < ROUTINE_INTERVAL_FRAMES {
            continue;
        }
        last_turn_frame = frame;
        let prompt = if triggers.is_empty() {
            format!("Game time {game_time}. Routine check.")
        } else {
            format!("Game time {game_time}. {}", triggers.join(" "))
        };
        transcript.record(json!({ "kind": "turn", "frame": frame, "prompt": prompt }));
        let line = json!({ "type": "user", "message": { "role": "user", "content": prompt } });
        if writeln!(stdin, "{line}").and_then(|()| stdin.flush()).is_err() || turn_done.recv().is_err() {
            eprintln!("[ai {ai_id}] strategist session ended; heuristics carry on alone");
            return;
        }
    }
}
