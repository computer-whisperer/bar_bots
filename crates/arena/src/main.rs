//! Batch evaluation harness. Runs headless matches of our bot against BARb, several at a time,
//! and reports win rates. Each match gets its own directory under `run/matches/<batch>/` holding
//! the start script, engine log, bot log and replay.
//!
//! usage: arena [--matches N] [--parallel N] [--speed N] [--profile easy|medium|hard|hard_aggressive]
//!              [--map NAME] [--max-minutes N] [--label TEXT]

mod autohost;
mod script;

use std::fs::{self, File};
use std::io::{self, Write};
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

struct Options {
    matches: usize,
    parallel: usize,
    speed: u32,
    profile: String,
    map: String,
    max_minutes: u32,
    label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
enum Outcome {
    Win,
    Loss,
    /// Nobody had won when the game-time limit ran out.
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

    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let batch_dir = repo.join(format!("run/matches/{stamp}-{}", options.label));
    fs::create_dir_all(&batch_dir)?;
    println!(
        "{} matches vs BARb {} on {}, speed {}, {} parallel -> {}",
        options.matches, options.profile, options.map, options.speed, options.parallel, batch_dir.display()
    );

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
        we_are_first: index % 2 == 0,
        our_side: if (index / 2) % 2 == 0 { "Armada" } else { "Cortex" },
    };
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
    let result = referee(&mut autohost, &mut engine, options, &setup);
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
fn referee(autohost: &mut Autohost, engine: &mut Child, options: &Options, setup: &MatchSetup) -> io::Result<Outcome> {
    let mut playing = false;
    let mut deadline = Instant::now() + LOAD_ALLOWANCE;
    let game_limit = Duration::from_secs_f32(options.max_minutes as f32 * 60.0 / options.speed as f32);
    loop {
        if engine.try_wait()?.is_some() {
            return Ok(Outcome::Aborted);
        }
        if Instant::now() > deadline {
            return Ok(if playing { Outcome::Timeout } else { Outcome::Aborted });
        }
        match autohost.receive(Duration::from_millis(500))? {
            Some(Event::StartPlaying) => {
                // Raising the minimum forces the server's speed up; the maximum has to allow it first.
                autohost.send(&format!("/setmaxspeed {}", options.speed))?;
                autohost.send(&format!("/setminspeed {}", options.speed))?;
                playing = true;
                // Generous: the engine may not sustain the requested speed.
                deadline = Instant::now() + game_limit * 3;
            }
            Some(Event::GameOver { winning_ally_teams }) => {
                let won = winning_ally_teams.contains(&setup.our_ally_team());
                return Ok(if won { Outcome::Win } else { Outcome::Loss });
            }
            Some(Event::Other) | None => {}
        }
    }
}

/// The engine prefixes log lines with `[f=NNNNNNN]`; the log is complete once the engine has exited.
fn last_frame(engine_log: &Path) -> u32 {
    let log = fs::read(engine_log).unwrap_or_default();
    let log = String::from_utf8_lossy(&log);
    log.rmatch_indices("[f=").find_map(|(at, _)| log[at + 3..].split(']').next()?.parse().ok()).unwrap_or(0)
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

fn parse_args() -> Options {
    let mut options = Options {
        matches: 4,
        parallel: 4,
        speed: 20,
        profile: "easy".into(),
        map: "Quicksilver Remake 1.24".into(),
        max_minutes: 40,
        label: "batch".into(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
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
    eprintln!("{problem}\nusage: arena [--matches N] [--parallel N] [--speed N] [--profile NAME] [--map NAME] [--max-minutes N] [--label TEXT]");
    std::process::exit(2)
}

