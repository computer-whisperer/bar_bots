//! Batch evaluation harness. Runs headless matches of our bot against BARb, several at a time,
//! and reports win rates. Each match gets its own directory under `run/matches/<batch>/` holding
//! the start script, engine log, bot log and replay.
//!
//! usage: arena [--matches N] [--parallel N] [--speed N] [--profile easy|medium|hard|hard_aggressive]
//!              [--map NAME] [--max-minutes N] [--label TEXT] [--mirror]

mod autohost;
mod script;

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use autohost::{Autohost, Event};
use script::MatchSetup;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const BASE_PORT: u16 = 9100;
/// Wall-clock allowance for engine start-up and map loading.
const LOAD_ALLOWANCE: Duration = Duration::from_secs(90);
/// Backstop for a match whose game clock stops advancing.
const STALL_ALLOWANCE: Duration = Duration::from_secs(120);
/// Resident memory one engine needs with the match template's settings, with headroom.
const ENGINE_MEMORY_GB: u64 = 4;

struct Options {
    matches: usize,
    parallel: usize,
    speed: u32,
    profile: String,
    map: String,
    max_minutes: u32,
    label: String,
    mirror: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
enum Outcome {
    Win,
    Loss,
    /// Nobody had won after `--max-minutes` of game time, or the game clock stalled.
    Timeout,
    /// The engine exited without reporting a result.
    Aborted,
}

#[derive(Debug, Serialize)]
struct MatchResult {
    index: usize,
    outcome: Outcome,
    our_side: &'static str,
    our_corner: &'static str,
    game_minutes: f32,
    wall_seconds: f32,
}

fn main() -> io::Result<()> {
    let options = parse_args();
    let repo = fs::canonicalize(REPO)?;
    let status = Command::new(repo.join("run/install_ai.sh")).stdout(Stdio::null()).status()?;
    if !status.success() {
        return Err(io::Error::other("install_ai.sh failed"));
    }

    let available_gb = available_memory_gb()?;
    let needed_gb = options.parallel as u64 * ENGINE_MEMORY_GB;
    if available_gb < needed_gb {
        return Err(io::Error::other(format!(
            "{} parallel engines need about {needed_gb} GB, only {available_gb} GB available; lower --parallel",
            options.parallel
        )));
    }

    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let batch_dir = repo.join(format!("run/matches/{stamp}-{}", options.label));
    fs::create_dir_all(&batch_dir)?;
    println!(
        "{} matches vs BARb {} on {}, speed {}, {} parallel -> {}",
        options.matches, options.profile, options.map, options.speed, options.parallel, batch_dir.display()
    );

    let commit = git_commit(&repo);
    fs::write(
        batch_dir.join("batch.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "label": options.label, "commit": commit, "opponent": format!("BARb {}", options.profile),
            "map": options.map, "matches": options.matches, "parallel": options.parallel, "speed": options.speed,
            "max_minutes": options.max_minutes, "mirror": options.mirror,
        }))?,
    )?;

    let queue = Arc::new(Mutex::new((0..options.matches).collect::<Vec<_>>()));
    let results = Arc::new(Mutex::new(Vec::new()));
    let options = Arc::new(options);
    let workers: Vec<_> = (0..options.parallel)
        .map(|_| {
            let (queue, results, options, repo, batch_dir) =
                (queue.clone(), results.clone(), options.clone(), repo.clone(), batch_dir.clone());
            std::thread::spawn(move || {
                loop {
                    let Some(index) = queue.lock().unwrap().pop() else { break };
                    let result = run_match(&repo, &batch_dir, &options, index).unwrap_or_else(|e| {
                        eprintln!("match {index}: {e}");
                        MatchResult { index, outcome: Outcome::Aborted, our_side: "?", our_corner: "?", game_minutes: 0.0, wall_seconds: 0.0 }
                    });
                    println!(
                        "match {:>2}: {:<7} {} {} {:>5.1} game-min in {:>4.0}s",
                        result.index, format!("{:?}", result.outcome), result.our_side, result.our_corner,
                        result.game_minutes, result.wall_seconds
                    );
                    results.lock().unwrap().push(result);
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    refresh_cache_template(&repo, &batch_dir);

    let mut results = Arc::into_inner(results).unwrap().into_inner().unwrap();
    results.sort_by_key(|r| r.index);
    let mut file = File::create(batch_dir.join("results.jsonl"))?;
    for result in &results {
        writeln!(file, "{}", serde_json::to_string(result)?)?;
    }
    let count = |outcome| results.iter().filter(|r| r.outcome == outcome).count();
    println!(
        "== {} wins, {} losses, {} timeouts, {} aborted ==",
        count(Outcome::Win), count(Outcome::Loss), count(Outcome::Timeout), count(Outcome::Aborted)
    );
    print_rule_comparison(&batch_dir, &results);
    println!(
        "ledger row for docs/experiments.md:\n| {} | {} | {}{} | {} | {}-{}-{}{} | <what this tested> | <what it showed> |",
        options.label, commit, options.profile, if options.mirror { ", mirror" } else { "" }, results.len(),
        count(Outcome::Win), count(Outcome::Loss), count(Outcome::Timeout),
        match count(Outcome::Aborted) { 0 => String::new(), n => format!(" ({n})") }
    );
    Ok(())
}

fn run_match(repo: &Path, batch_dir: &Path, options: &Options, index: usize) -> io::Result<MatchResult> {
    let dir = batch_dir.join(format!("{index:02}"));
    fs::create_dir_all(&dir)?;
    let host_port = BASE_PORT + 2 * index as u16;
    let setup = MatchSetup {
        map: &options.map,
        opponent_profile: &options.profile,
        host_port,
        autohost_port: host_port + 1,
        seed: index as u32 + 1,
        // Alternate corner and faction so neither start nor side biases the batch.
        we_are_first: index.is_multiple_of(2),
        our_side: if (index / 2).is_multiple_of(2) { "Armada" } else { "Cortex" },
        mirror: options.mirror,
    };
    copy_tree(&repo.join("run/match-template"), &dir)?;
    let cache_template = repo.join("run/cache-template");
    if cache_template.is_dir() {
        copy_tree(&cache_template, &dir.join("cache"))?;
    }
    let script_path = dir.join("script.txt");
    fs::write(&script_path, setup.render())?;

    let mut autohost = Autohost::bind(setup.autohost_port)?;
    let socket = dir.join("bot.sock");
    let mut bot = Command::new(repo.join("target/release/bot"))
        .env("BAR_BOTS_SOCKET", &socket)
        .stderr(File::create(dir.join("bot.log"))?)
        .spawn()?;
    let log = File::create(dir.join("engine.log"))?;
    let mut engine = Command::new(repo.join("run/engine/spring-headless"))
        .args(["--isolation", "--write-dir"])
        .arg(&dir)
        .arg(&script_path)
        .env("SPRING_DATADIR", repo.join("run/data"))
        .env("BAR_BOTS_SOCKET", &socket)
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?;

    let started = Instant::now();
    let result = referee(&mut autohost, &mut engine, options, &setup, &dir.join("engine.log"));
    stop(&mut engine, &mut autohost);
    let _ = bot.kill();
    let _ = bot.wait();
    let outcome = result?;
    let game_minutes = last_frame(&dir.join("engine.log")) as f32 / (30.0 * 60.0);
    Ok(MatchResult {
        index,
        outcome,
        our_side: setup.our_side,
        our_corner: if setup.we_are_first { "NW" } else { "SE" },
        game_minutes,
        wall_seconds: started.elapsed().as_secs_f32(),
    })
}

/// Follows one match over the autohost channel until it is decided or out of time.
fn referee(
    autohost: &mut Autohost,
    engine: &mut Child,
    options: &Options,
    setup: &MatchSetup,
    engine_log: &Path,
) -> io::Result<Outcome> {
    let mut playing = false;
    let mut deadline = Instant::now() + LOAD_ALLOWANCE;
    let frame_limit = options.max_minutes * 60 * 30;
    let mut last_seen_frame = 0;
    loop {
        if engine.try_wait()?.is_some() {
            return Ok(Outcome::Aborted);
        }
        if Instant::now() > deadline {
            return Ok(if playing { Outcome::Timeout } else { Outcome::Aborted });
        }
        if playing {
            // Game time comes from the shim's heartbeat lines; the deadline only catches a stalled clock.
            let frame = last_frame(engine_log);
            if frame >= frame_limit {
                return Ok(Outcome::Timeout);
            }
            if frame > last_seen_frame {
                last_seen_frame = frame;
                deadline = Instant::now() + STALL_ALLOWANCE;
            }
        }
        match autohost.receive(Duration::from_millis(500))? {
            Some(Event::StartPlaying) => {
                // Raising the minimum forces the server's speed up; the maximum has to allow it first.
                autohost.send(&format!("/setmaxspeed {}", options.speed))?;
                autohost.send(&format!("/setminspeed {}", options.speed))?;
                playing = true;
                deadline = Instant::now() + STALL_ALLOWANCE;
            }
            Some(Event::GameOver { winning_ally_teams }) => {
                let won = winning_ally_teams.contains(&setup.our_ally_team());
                return Ok(if won { Outcome::Win } else { Outcome::Loss });
            }
            Some(Event::Other) | None => {}
        }
    }
}

/// Latest game frame in the engine log: the engine prefixes its lines with `[f=NNNNNNN]` and our
/// shim prints `heartbeat f=N` once per game minute. Reads only the tail, so it is cheap to poll.
fn last_frame(engine_log: &Path) -> u32 {
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

fn available_memory_gb() -> io::Result<u64> {
    let meminfo = fs::read_to_string("/proc/meminfo")?;
    let kb = meminfo
        .lines()
        .find_map(|line| line.strip_prefix("MemAvailable:")?.trim().strip_suffix("kB")?.trim().parse::<u64>().ok())
        .ok_or_else(|| io::Error::other("no MemAvailable in /proc/meminfo"))?;
    Ok(kb / (1024 * 1024))
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
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
fn refresh_cache_template(repo: &Path, batch_dir: &Path) {
    let source = batch_dir.join("00/cache");
    let template = repo.join("run/cache-template");
    if source.join("ArchiveCache22.lua").is_file() {
        let _ = fs::remove_dir_all(&template);
        if let Err(e) = copy_tree(&source, &template) {
            eprintln!("could not refresh {}: {e}", template.display());
        }
    }
}

fn stop(engine: &mut Child, autohost: &mut Autohost) {
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
fn git_commit(repo: &Path) -> String {
    let git = |args: &[&str]| Command::new("git").arg("-C").arg(repo).args(args).output().ok();
    let hash = git(&["rev-parse", "--short", "HEAD"]).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|o| !o.stdout.is_empty());
    format!("{}{}", hash.unwrap_or_else(|| "unknown".into()), if dirty { "+" } else { "" })
}

/// Mean firings per match of each heuristic (the `rules:` lines in bot.log), wins against losses.
fn print_rule_comparison(batch_dir: &Path, results: &[MatchResult]) {
    let mut totals: std::collections::BTreeMap<String, [f32; 2]> = Default::default();
    let mut matches = [0f32; 2];
    for result in results {
        let side = match result.outcome {
            Outcome::Win => 0,
            Outcome::Loss => 1,
            _ => continue,
        };
        matches[side] += 1.0;
        let log = fs::read_to_string(batch_dir.join(format!("{:02}/bot.log", result.index))).unwrap_or_default();
        for counts in log.lines().filter_map(|line| line.split_once(" rules: ")).map(|(_, counts)| counts) {
            for (rule, n) in counts.split_whitespace().filter_map(|pair| pair.split_once('=')) {
                totals.entry(rule.to_string()).or_default()[side] += n.parse::<f32>().unwrap_or(0.0);
            }
        }
    }
    if matches[0] == 0.0 || matches[1] == 0.0 {
        return;
    }
    println!("heuristic firings per match      wins   losses");
    for (rule, [wins, losses]) in totals {
        println!("  {rule:<28} {:>6.1} {:>8.1}", wins / matches[0], losses / matches[1]);
    }
}

fn parse_args() -> Options {
    let mut options = Options {
        matches: 4,
        parallel: 8,
        speed: 50,
        profile: "easy".into(),
        map: "Quicksilver Remake 1.24".into(),
        max_minutes: 40,
        label: "batch".into(),
        mirror: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == "--mirror" {
            options.mirror = true;
            continue;
        }
        let mut value = || args.next().unwrap_or_else(|| usage(&format!("{flag} needs a value")));
        match flag.as_str() {
            "--matches" => options.matches = value().parse().unwrap_or_else(|_| usage("--matches")),
            "--parallel" => options.parallel = value().parse().unwrap_or_else(|_| usage("--parallel")),
            "--speed" => options.speed = value().parse().unwrap_or_else(|_| usage("--speed")),
            "--max-minutes" => options.max_minutes = value().parse().unwrap_or_else(|_| usage("--max-minutes")),
            "--profile" => options.profile = value(),
            "--map" => options.map = value(),
            "--label" => options.label = value(),
            _ => usage(&format!("unknown argument {flag}")),
        }
    }
    options
}

fn usage(problem: &str) -> ! {
    eprintln!("{problem}\nusage: arena [--matches N] [--parallel N] [--speed N] [--profile NAME] [--map NAME] [--max-minutes N] [--label TEXT] [--mirror]");
    std::process::exit(2)
}

