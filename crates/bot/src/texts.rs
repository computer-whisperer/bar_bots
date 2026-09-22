//! The texts the models read: the player's, commander's and strategist's prompts, the briefs behind them, and the
//! hands' rules and default instructions. Each is a file in the repository, read at every use, so an edit takes
//! effect without a rebuild (the user, 2026-09-22): the hands' rules on their next call, a session prompt on the
//! next session, which the driver starts as soon as the prompt on disk has changed. The copy compiled in is the
//! fallback for a bot run away from its checkout; `WITHIN_REASON_TEXTS` names the checkout explicitly.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub struct Text {
    /// Relative to the repository root.
    pub path: &'static str,
    pub compiled: &'static str,
}

pub const STRATEGIST_PROMPT: Text = Text { path: "crates/bot/src/strategist/prompt.md", compiled: include_str!("strategist/prompt.md") };
pub const COMMANDER_PROMPT: Text = Text { path: "crates/bot/src/strategist/commander.md", compiled: include_str!("strategist/commander.md") };
pub const PLAYER_PROMPT: Text = Text { path: "crates/bot/src/strategist/player.md", compiled: include_str!("strategist/player.md") };
pub const COMMANDER_BRIEF: Text = Text { path: "docs/briefs/commander.md", compiled: include_str!("../../../docs/briefs/commander.md") };
pub const PLAYER_BRIEF: Text = Text { path: "docs/briefs/player.md", compiled: include_str!("../../../docs/briefs/player.md") };
pub const HANDS_RULES: Text = Text { path: "crates/bot/src/brain/pianist/rules.md", compiled: include_str!("brain/pianist/rules.md") };
pub const HANDS_DEFAULT: Text = Text { path: "crates/bot/src/brain/pianist/default.md", compiled: include_str!("brain/pianist/default.md") };

/// The file that marks a checkout.
const MARKER: &str = "crates/bot/src/strategist/player.md";

/// The checkout the texts are read from: `WITHIN_REASON_TEXTS`, else the ancestor of the running binary that holds
/// the marker (`target/release/bot` is two below it), else none and every text is its compiled copy.
fn root() -> Option<&'static PathBuf> {
    static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();
    ROOT.get_or_init(|| {
        if let Some(dir) = std::env::var_os("WITHIN_REASON_TEXTS") {
            return Some(PathBuf::from(dir));
        }
        let exe = std::env::current_exe().ok()?;
        exe.ancestors().find(|a| a.join(MARKER).is_file()).map(PathBuf::from)
    })
    .as_ref()
}

/// The text as it is on disk now, else as compiled. Says once per file where it reads from, and once per file when a
/// read fails.
pub fn read(text: &Text) -> String {
    static SAID: Mutex<Option<HashSet<String>>> = Mutex::new(None);
    let say_once = |key: String, line: String| {
        let mut said = SAID.lock().unwrap();
        if said.get_or_insert_with(HashSet::new).insert(key) {
            eprintln!("[texts] {line}");
        }
    };
    let Some(root) = root() else {
        say_once("root".into(), "no checkout found (set WITHIN_REASON_TEXTS): the prompts and rules are the compiled copies".into());
        return text.compiled.to_string();
    };
    match std::fs::read_to_string(root.join(text.path)) {
        Ok(contents) => {
            say_once(format!("read {}", text.path), format!("{} is read from {}", text.path, root.display()));
            contents
        }
        Err(e) => {
            say_once(format!("fail {}", text.path), format!("{}: {e}; the compiled copy is used", text.path));
            text.compiled.to_string()
        }
    }
}

/// A hash of a text, for noticing an edit between sessions.
pub fn digest(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_checkout_is_found_from_the_test_binary_and_its_files_match_the_compiled_copies() {
        assert!(super::root().is_some(), "the test binary runs under target/, below the checkout");
        for text in [&super::PLAYER_PROMPT, &super::PLAYER_BRIEF, &super::HANDS_RULES, &super::HANDS_DEFAULT] {
            assert_eq!(super::read(text), text.compiled, "{} on disk differs from the copy compiled into this test binary", text.path);
        }
    }
}
