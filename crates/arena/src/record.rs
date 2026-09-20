//! Closes the bot's match records (`docs/harness/record-format.md`) with what only the arena knows:
//! who won, and where the engine put its replay.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use serde_json::json;

use crate::MatchResult;

/// Appends a `result` line to every `record-*.jsonl` in the match directory. The bot has exited by now.
pub fn finish(dir: &Path, result: &MatchResult, opponent_profile: &str) -> io::Result<()> {
    let replay = fs::read_dir(dir.join("demos")).ok().and_then(|entries| {
        let name = entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).find(|n| n.ends_with(".sdfz"))?;
        Some(format!("demos/{name}"))
    });
    for entry in fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.starts_with("record-") && name.ends_with(".jsonl")) {
            continue;
        }
        let line = json!({ "t": "result", "result": result, "opponent": format!("BARb {opponent_profile}"), "replay": replay });
        // A killed bot can leave half a line behind; start on a fresh one.
        let ragged = fs::read(entry.path())?.last().is_some_and(|&b| b != b'\n');
        let mut file = OpenOptions::new().append(true).open(entry.path())?;
        write!(file, "{}{line}\n", if ragged { "\n" } else { "" })?;
    }
    Ok(())
}
