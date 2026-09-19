//! Bot process. Listens on the socket the AI shims connect to and runs one session per AI.

mod brain;
mod strategist;
mod world;

use std::io;
use std::os::unix::net::{UnixListener, UnixStream};

use bot_protocol::{Commands, FrameReader, ToBot, socket_path, write_frame};

use brain::Brain;
use strategist::Strategist;
use world::World;

/// usage: bot [--strategist]
/// `--strategist` gives every AI session a Claude Code strategist (see `DESIGN.md`); transcripts go to
/// `$WITHIN_REASON_LOG_DIR`, else the current directory.
fn main() -> io::Result<()> {
    let with_strategist = match std::env::args().nth(1).as_deref() {
        None => false,
        Some("--strategist") => true,
        Some(other) => return Err(io::Error::other(format!("unknown argument {other}; usage: bot [--strategist]"))),
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
            if let Err(e) = session(stream, with_strategist) {
                eprintln!("session ended: {e}");
            }
        });
    }
    Ok(())
}

fn log_dir() -> std::path::PathBuf {
    std::env::var_os("WITHIN_REASON_LOG_DIR").map_or_else(|| ".".into(), Into::into)
}

fn session(mut stream: UnixStream, with_strategist: bool) -> io::Result<()> {
    let mut input = stream.try_clone()?;
    let mut reader = FrameReader::default();
    let mut next = move || reader.read::<ToBot>(&mut input).map(|m| m.expect("blocking socket"));

    let ToBot::Hello(hello) = next()? else {
        return Err(io::Error::other("expected Hello first"));
    };
    // A strategist that fails to start is not fatal: the heuristics play alone.
    let strategist = with_strategist
        .then(|| Strategist::start(&log_dir(), hello.ai_id))
        .and_then(|started| started.inspect_err(|e| eprintln!("strategist failed to start: {e}")).ok());
    let mut brain = Brain::new(World::new(hello), strategist.as_ref().map(|s| s.shared.clone()));
    write_frame(&mut stream, &Commands::default())?;
    loop {
        let ToBot::Tick(tick) = next()? else {
            return Err(io::Error::other("unexpected second Hello"));
        };
        let commands = brain.decide(&tick);
        write_frame(&mut stream, &Commands(commands))?;
    }
}
