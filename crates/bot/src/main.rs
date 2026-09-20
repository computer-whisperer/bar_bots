//! Bot process. Listens on the socket the AI shims connect to and runs one session per AI.

mod brain;
mod recorder;
mod strategist;
mod world;

use std::io;
use std::os::unix::net::{UnixListener, UnixStream};

use bot_protocol::{Commands, FrameReader, ToBot, socket_path, write_frame};

use brain::Brain;
use strategist::{Mode, Strategist};
use world::World;

/// usage: bot [--strategist | --commander]
/// Either flag gives every AI session a Claude Code session beside the brain (see `DESIGN.md`): the Opus strategist
/// with standing directives, or the Sonnet field commander with squads and the unit mix. Transcripts go to
/// `$WITHIN_REASON_LOG_DIR`, else the current directory.
fn main() -> io::Result<()> {
    let mode = match std::env::args().nth(1).as_deref() {
        None => None,
        Some("--strategist") => Some(Mode::Strategist),
        Some("--commander") => Some(Mode::Commander),
        Some(other) => return Err(io::Error::other(format!("unknown argument {other}; usage: bot [--strategist | --commander]"))),
    };
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
            if let Err(e) = session(stream, mode) {
                eprintln!("session ended: {e}");
            }
        });
    }
    Ok(())
}

fn log_dir() -> std::path::PathBuf {
    std::env::var_os("WITHIN_REASON_LOG_DIR").map_or_else(|| ".".into(), Into::into)
}

fn session(mut stream: UnixStream, mode: Option<Mode>) -> io::Result<()> {
    let mut input = stream.try_clone()?;
    let mut reader = FrameReader::default();
    let mut next = move || reader.read::<ToBot>(&mut input).map(|m| m.expect("blocking socket"));

    let ToBot::Hello(hello) = next()? else {
        return Err(io::Error::other("expected Hello first"));
    };
    // A strategist that fails to start is not fatal: the heuristics play alone.
    let strategist = mode
        .map(|mode| Strategist::start(&log_dir(), hello.ai_id, mode))
        .and_then(|started| started.inspect_err(|e| eprintln!("strategist failed to start: {e}")).ok());
    let mode_name = match mode {
        None => "heuristic",
        Some(Mode::Strategist) => "strategist",
        Some(Mode::Commander) => "commander",
    };
    let mut recorder = recorder::Recorder::from_env(&log_dir(), &hello, mode_name);
    let mut brain = Brain::new(World::new(hello), strategist.as_ref().map(|s| s.shared.clone()));
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
