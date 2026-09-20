//! Chores of running a headless engine from a match directory, shared by the arena and the duel runner.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use crate::autohost::Autohost;

pub const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
/// The game version matches are played on, as a rapid tag; resolved to its full name for the start script.
pub const GAME_TAG: &str = "byar:test";
/// Wall-clock allowance for engine start-up and map loading.
pub const LOAD_ALLOWANCE: Duration = Duration::from_secs(90);
/// Resident memory one engine needs with the match template's settings, with headroom.
pub const ENGINE_MEMORY_GB: u64 = 4;

/// The full name a rapid tag stands for ("byar:test" -> "Beyond All Reason test-NNNNN-hash"), from the rapid
/// index in our data directory. A start script may name either, but the name ends up in the replay's header, and
/// the BAR lobby offers to download a game called "byar:test" for ever instead of opening the replay.
pub fn resolve_game(repo: &Path, tag: &str) -> io::Result<String> {
    let rapid = repo.join("run/data/rapid");
    let repository = tag.split(':').next().unwrap_or_default();
    for host in fs::read_dir(&rapid)?.flatten() {
        let Ok(file) = File::open(host.path().join(repository).join("versions.gz")) else { continue };
        let mut index = String::new();
        flate2::read::GzDecoder::new(file).read_to_string(&mut index)?;
        // tag,package hash,dependencies,full name
        let name = index.lines().find_map(|line| {
            let mut fields = line.split(',');
            (fields.next() == Some(tag)).then(|| fields.nth(2)).flatten()
        });
        if let Some(name) = name {
            return Ok(name.to_string());
        }
    }
    Err(io::Error::other(format!("rapid tag {tag} not found under {}", rapid.display())))
}

/// Latest game frame in the engine log: the engine prefixes its lines with `[f=NNNNNNN]` and our
/// shim prints `heartbeat f=N` once per game minute. Reads only the tail, so it is cheap to poll.
pub fn last_frame(engine_log: &Path) -> u32 {
    const TAIL: u64 = 64 * 1024;
    let Ok(mut file) = File::open(engine_log) else { return 0 };
    let len = file.metadata().map_or(0, |m| m.len());
    let mut tail = Vec::new();
    if file.seek(SeekFrom::Start(len.saturating_sub(TAIL))).is_err() || file.read_to_end(&mut tail).is_err() {
        return 0;
    }
    let tail = String::from_utf8_lossy(&tail);
    let number_after = |marker: &str| {
        tail.rmatch_indices(marker).find_map(|(at, _)| {
            let digits: String = tail[at + marker.len()..].chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<u32>().ok()
        })
    };
    number_after("[f=").max(number_after("heartbeat f=")).unwrap_or(0)
}

pub fn available_memory_gb() -> io::Result<u64> {
    let meminfo = fs::read_to_string("/proc/meminfo")?;
    let kb = meminfo
        .lines()
        .find_map(|line| line.strip_prefix("MemAvailable:")?.trim().strip_suffix("kB")?.trim().parse::<u64>().ok())
        .ok_or_else(|| io::Error::other("no MemAvailable in /proc/meminfo"))?;
    Ok(kb / (1024 * 1024))
}

pub fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// The engine checksums every archive unless the write dir already holds a cache that matches
/// their paths and mtimes. Keep a copy of the newest one for the next batch's match dirs.
pub fn refresh_cache_template(repo: &Path, batch_dir: &Path) {
    let source = batch_dir.join("00/cache");
    let template = repo.join("run/cache-template");
    if source.join("ArchiveCache22.lua").is_file() {
        let _ = fs::remove_dir_all(&template);
        if let Err(e) = copy_tree(&source, &template) {
            eprintln!("could not refresh {}: {e}", template.display());
        }
    }
}

pub fn stop(engine: &mut Child, autohost: &mut Autohost) {
    let _ = autohost.send("/kill");
    let give_up = Instant::now() + Duration::from_secs(30);
    while Instant::now() < give_up {
        if matches!(engine.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let _ = engine.kill();
    let _ = engine.wait();
}

/// Short commit hash, with `+` when the working tree has uncommitted changes to tracked files.
pub fn git_commit(repo: &Path) -> String {
    let git = |args: &[&str]| Command::new("git").arg("-C").arg(repo).args(args).output().ok();
    let hash = git(&["rev-parse", "--short", "HEAD"]).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|o| !o.stdout.is_empty());
    format!("{}{}", hash.unwrap_or_else(|| "unknown".into()), if dirty { "+" } else { "" })
}

