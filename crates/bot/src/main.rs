//! Bot process. Listens on the socket the AI shims connect to and runs one session per AI.

mod brain;
mod recorder;
mod strategist;
mod team;
mod world;

use std::io;
use std::os::unix::net::{UnixListener, UnixStream};

use bot_protocol::{Commands, FrameReader, ToBot, socket_path, write_frame};

use brain::Brain;
use strategist::{Mode, Strategist};
use world::World;

/// usage: bot [--strategist | --commander | --commander-each] [--pianist]
/// A Claude Code session beside the brains (see `DESIGN.md`): the Opus strategist with standing directives, or the
/// Sonnet field commander with squads and the unit mix. One session serves every seat we play on a team
/// (`strategist/seats.rs`); `--commander-each` gives each seat a commander of its own. `--pianist`: Jev plays every
/// unit from the player's instructions in place of the decision heuristics (`docs/design/2026-09-21-pianist.md`).
/// Transcripts go to `$WITHIN_REASON_LOG_DIR`, else the current directory.
fn main() -> io::Result<()> {
    let mut mode = None;
    let mut pianist = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--strategist" => mode = Some((Mode::Strategist, false)),
            "--commander" => mode = Some((Mode::Commander, false)),
            "--commander-each" => mode = Some((Mode::Commander, true)),
            "--pianist" => pianist = true,
            other => return Err(io::Error::other(format!("unknown argument {other}; usage: bot [--strategist | --commander | --commander-each] [--pianist]"))),
        }
    }
    let path = socket_path();
    // A previous run may have left its socket file behind; nothing can be listening on it.
    if UnixStream::connect(&path).is_err() {
        let _ = std::fs::remove_file(&path);
    }
    let listener = UnixListener::bind(&path)?;
    eprintln!("listening on {}", path.display());
    for stream in listener.incoming() {
        let stream = stream?;
        std::thread::spawn(move || {
            if let Err(e) = session(stream, mode, pianist) {
                eprintln!("session ended: {e}");
            }
        });
    }
    Ok(())
}

fn log_dir() -> std::path::PathBuf {
    std::env::var_os("WITHIN_REASON_LOG_DIR").map_or_else(|| ".".into(), Into::into)
}

/// `mode`: the kind of LLM session, and whether each seat gets its own (else one per team).
fn session(mut stream: UnixStream, mode: Option<(Mode, bool)>, pianist: bool) -> io::Result<()> {
    let mut input = stream.try_clone()?;
    let mut reader = FrameReader::default();
    let mut next = move || reader.read::<ToBot>(&mut input).map(|m| m.expect("blocking socket"));

    let ToBot::Hello(hello) = next()? else {
        return Err(io::Error::other("expected Hello first"));
    };
    // A strategist that fails to start is not fatal: the heuristics play alone.
    let board = team::TeamBoard::of(&hello);
    let strategist = mode
        .map(|(mode, each)| {
            let start = || Strategist::start(&log_dir(), hello.ai_id, mode).map(std::sync::Arc::new);
            if each { start() } else { board.strategist(start) }
        })
        .and_then(|started| started.inspect_err(|e| eprintln!("strategist failed to start: {e}")).ok());
    let mode_name = match (mode, pianist) {
        (_, true) => "pianist",
        (None, _) => "heuristic",
        (Some((Mode::Strategist, _)), _) => "strategist",
        (Some((Mode::Commander, _)), _) => "commander",
    };
    // No key, no pianist: the process says so and the seat is not played at all rather than by the heuristics,
    // which would pass for the pianist in the ledger.
    let pianist = match pianist {
        false => None,
        true => match brain::pianist::Pianist::from_env(&log_dir(), hello.ai_id) {
            Ok(pianist) => {
                eprintln!("[ai {}] pianist: Jev ({}) plays from the instructions", hello.ai_id, pianist.model());
                Some(pianist)
            }
            Err(e) => return Err(io::Error::other(format!("pianist mode asked for but {e}"))),
        },
    };
    let mut recorder = recorder::Recorder::from_env(&log_dir(), &hello, mode_name);
    let setting = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let mut banner = format!("Within Reason {} | {mode_name}", env!("WITHIN_REASON_COMMIT"));
    if strategist.is_some() {
        banner += &format!(" ({}{})", setting("WITHIN_REASON_MODEL").unwrap_or_else(|| "default model".into()), setting("WITHIN_REASON_EFFORT").map_or(String::new(), |e| format!(", effort {e}")));
    }
    if let Some(disabled) = setting("WITHIN_REASON_DISABLE") {
        banner += &format!(" | off: {disabled}");
    }
    if pianist.is_some() {
        banner += &format!(" | pianist {}", pianist.as_ref().map_or("", |p| p.model()));
    }
    let mut brain = Brain::new(World::new(hello), strategist.as_ref().map(|s| s.shared.clone()), board, banner, pianist);
    write_frame(&mut stream, &Commands::default())?;
    loop {
        let ToBot::Tick(tick) = next()? else {
            return Err(io::Error::other("unexpected second Hello"));
        };
        let started = std::time::Instant::now();
        let commands = brain.decide(&tick);
        let decide_ms = started.elapsed().as_secs_f32() * 1000.0;
        let journal = brain.take_journal();
        if let Some(r) = &mut recorder
            && let Err(e) = r.tick(&tick, journal, |unit| brain.role(unit), &commands, decide_ms)
        {
            // A full disk must not lose the match.
            eprintln!("match record abandoned: {e}");
            recorder = None;
        }
        write_frame(&mut stream, &Commands(commands))?;
    }
}
