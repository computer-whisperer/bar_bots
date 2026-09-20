//! Batch evaluation harness. Runs headless matches of our bot against BARb, several at a time,
//! and reports win rates. Each match gets its own directory under `run/matches/<batch>/` holding
//! the start script, engine log, bot log and replay.
//!
//! usage: arena [--matches N] [--parallel N] [--speed N] [--profile easy|medium|hard|hard_aggressive]
//!              [--map NAME] [--max-minutes N] [--label TEXT] [--mirror] [--swap-corners]
//!              [--side armada|cortex] [--corner nw|se]   (default: alternate)
//!              [--bot PATH]   (bot binary from another build, for A/B runs)
//!              [--disable H-ID,H-ID]   (ablation: switch heuristics off by registry ID)
//!              [--ab-disable H-ID,H-ID]   (interleaved A/B: arm B also switches these off; blocks of four matches)
//!              [--claude-config-dir DIR]   (subscription for --strategist sessions; default ~/.claude2)
//!              [--base-port N]   (default 9100; match i uses N+2i and N+2i+1, so a second arena needs another range)
//!              [--strategist]   (Claude Code strategist per match; use with --speed 2 and few matches)
//!              [--commander]    (Sonnet field commander per match; the game is held still during its turns, so any --speed)

mod record;
mod script;

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use arena::autohost::{Autohost, Event};
use arena::harness::{
    ENGINE_MEMORY_GB, GAME_TAG, LOAD_ALLOWANCE, REPO, available_memory_gb, copy_tree, git_commit, last_frame,
    refresh_cache_template, resolve_game, stop,
};
use script::MatchSetup;

const BASE_PORT: u16 = 9100;
/// Backstop for a match whose game clock stops advancing.
const STALL_ALLOWANCE: Duration = Duration::from_secs(120);

struct Options {
    matches: usize,
    parallel: usize,
    speed: u32,
    profile: String,
    map: String,
    max_minutes: u32,
    label: String,
    mirror: bool,
    swap_corners: bool,
    strategist: bool,
    commander: bool,
    /// Play every match as this faction instead of alternating.
    side: Option<&'static str>,
    /// Fixes our start corner; otherwise it alternates.
    corner: Option<bool>,
    /// Bot binary to run instead of this workspace's, for A/B runs against an older build.
    bot: Option<std::path::PathBuf>,
    /// Comma-separated heuristic IDs the bot should switch off (ablation).
    disable: String,
    /// Interleaved A/B: arm B additionally switches these heuristics off. Arms alternate in blocks
    /// of four matches, so each arm meets every corner and faction under the same machine conditions.
    ab_disable: Option<String>,
    /// Claude Code config dir for strategist sessions (which subscription they run on).
    claude_config_dir: Option<String>,
    /// First of the UDP ports the matches use (two each); a second arena on the same machine needs its own range.
    base_port: u16,
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
    /// "A" or "B" in an interleaved A/B batch, else empty.
    arm: &'static str,
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
            "max_minutes": options.max_minutes, "mirror": options.mirror, "swap_corners": options.swap_corners, "strategist": options.strategist, "commander": options.commander, "side": options.side, "corner": options.corner.map(|first| if first { "NW" } else { "SE" }), "bot": options.bot, "disable": options.disable, "ab_disable": options.ab_disable,
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
                        MatchResult { index, arm: "", outcome: Outcome::Aborted, our_side: "?", our_corner: "?", game_minutes: 0.0, wall_seconds: 0.0 }
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
    if let Some(extra) = &options.ab_disable {
        for arm in ["A", "B"] {
            let tally = |outcome| results.iter().filter(|r| r.arm == arm && r.outcome == outcome).count();
            println!(
                "arm {arm} ({}): {} wins, {} losses, {} timeouts",
                if arm == "A" { "as configured".to_string() } else { format!("also without {extra}") },
                tally(Outcome::Win), tally(Outcome::Loss), tally(Outcome::Timeout)
            );
        }
    }
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
    let host_port = options.base_port + 2 * index as u16;
    let game = resolve_game(repo, GAME_TAG)?;
    let setup = MatchSetup {
        game: &game,
        map: &options.map,
        opponent_profile: &options.profile,
        host_port,
        autohost_port: host_port + 1,
        seed: index as u32 + 1,
        // Alternate corner and faction so neither start nor side biases the batch.
        // `--corner` names the corner; which team slot that means depends on `--swap-corners`.
        we_are_first: options.corner.map_or(index.is_multiple_of(2), |north_west| north_west != options.swap_corners),
        our_side: options.side.unwrap_or(if (index / 2).is_multiple_of(2) { "Armada" } else { "Cortex" }),
        mirror: options.mirror,
        swap_corners: options.swap_corners,
    };
    copy_tree(&repo.join("run/match-template"), &dir)?;
    let cache_template = repo.join("run/cache-template");
    if cache_template.is_dir() {
        copy_tree(&cache_template, &dir.join("cache"))?;
    }
    let script_path = dir.join("script.txt");
    fs::write(&script_path, setup.render())?;

    let mut autohost = Autohost::bind(setup.autohost_port)?;
    // Unix socket paths are limited to ~108 bytes, so the socket cannot live in the match directory.
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, Into::into);
    let socket = runtime_dir.join(format!("wreason-arena-{}-{index}.sock", std::process::id()));
    let arm = match &options.ab_disable {
        None => "",
        Some(_) if (index / 4).is_multiple_of(2) => "A",
        Some(_) => "B",
    };
    let disable = match (&options.ab_disable, arm) {
        (Some(extra), "B") if options.disable.is_empty() => extra.clone(),
        (Some(extra), "B") => format!("{},{extra}", options.disable),
        _ => options.disable.clone(),
    };
    let mut bot = Command::new(options.bot.clone().unwrap_or_else(|| repo.join("target/release/bot")))
        .args(options.strategist.then_some("--strategist"))
        .args(options.commander.then_some("--commander"))
        .env("WITHIN_REASON_SOCKET", &socket)
        .env("WITHIN_REASON_LOG_DIR", &dir)
        .env("WITHIN_REASON_DISABLE", &disable)
        // Every match leaves a record for `run/view_match.py`: 0.1-0.3 MB per game minute (docs/harness/record-format.md).
        .env("WITHIN_REASON_RECORD", "1")
        .envs(options.claude_config_dir.as_ref().map(|dir| ("WITHIN_REASON_CLAUDE_CONFIG_DIR", dir)))
        .stderr(File::create(dir.join("bot.log"))?)
        .spawn()?;
    let log = File::create(dir.join("engine.log"))?;
    let mut engine = Command::new(repo.join("run/engine/spring-headless"))
        .args(["--isolation", "--write-dir"])
        .arg(&dir)
        .arg(&script_path)
        .env("SPRING_DATADIR", repo.join("run/data"))
        // The commander takes its turns with the game held still: the shim waits for each of the bot's answers.
        .envs(options.commander.then_some(("WITHIN_REASON_LOCKSTEP", "1")))
        .env("WITHIN_REASON_TRACE_BUILDS", "1")
        .env("WITHIN_REASON_SOCKET", &socket)
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?;

    let started = Instant::now();
    let result = referee(&mut autohost, &mut engine, &mut bot, options, &setup, &dir.join("engine.log"));
    stop(&mut engine, &mut autohost);
    let _ = bot.kill();
    let _ = bot.wait();
    let _ = fs::remove_file(&socket);
    let outcome = result?;
    let game_minutes = last_frame(&dir.join("engine.log")) as f32 / (30.0 * 60.0);
    let result = MatchResult {
        index,
        arm,
        outcome,
        our_side: setup.our_side,
        our_corner: if setup.we_are_first != setup.swap_corners { "NW" } else { "SE" },
        game_minutes,
        wall_seconds: started.elapsed().as_secs_f32(),
    };
    if let Err(e) = record::finish(&dir, &result, &options.profile) {
        eprintln!("match {index}: could not close the match record: {e}");
    }
    Ok(result)
}

/// Follows one match over the autohost channel until it is decided or out of time.
fn referee(
    autohost: &mut Autohost,
    engine: &mut Child,
    bot: &mut Child,
    options: &Options,
    setup: &MatchSetup,
    engine_log: &Path,
) -> io::Result<Outcome> {
    let mut playing = false;
    // The game clock stands still during the commander's turns, and the log only shows it once a game minute.
    let stall_allowance = if options.commander { 10 * STALL_ALLOWANCE } else { STALL_ALLOWANCE };
    let mut deadline = Instant::now() + LOAD_ALLOWANCE;
    let frame_limit = options.max_minutes * 60 * 30;
    let mut last_seen_frame = 0;
    loop {
        if engine.try_wait()?.is_some() {
            return Ok(Outcome::Aborted);
        }
        if let Some(status) = bot.try_wait()? {
            // Without its bot our team stands idle; the result would be meaningless.
            return Err(io::Error::other(format!("bot process exited ({status}); see bot.log")));
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
                deadline = Instant::now() + stall_allowance;
            }
        }
        match autohost.receive(Duration::from_millis(500))? {
            Some(Event::StartPlaying) => {
                // Raising the minimum forces the server's speed up; the maximum has to allow it first.
                autohost.send(&format!("/setmaxspeed {}", options.speed))?;
                autohost.send(&format!("/setminspeed {}", options.speed))?;
                playing = true;
                deadline = Instant::now() + stall_allowance;
            }
            Some(Event::GameOver { winning_ally_teams }) => {
                let won = winning_ally_teams.contains(&setup.our_ally_team());
                return Ok(if won { Outcome::Win } else { Outcome::Loss });
            }
            Some(Event::Other) | None => {}
        }
    }
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
        swap_corners: false,
        strategist: false,
        commander: false,
        side: None,
        corner: None,
        bot: None,
        disable: String::new(),
        ab_disable: None,
        claude_config_dir: None,
        base_port: BASE_PORT,
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--mirror" => {
                options.mirror = true;
                continue;
            }
            "--swap-corners" => {
                options.swap_corners = true;
                continue;
            }
            "--commander" => {
                options.commander = true;
                continue;
            }
            "--strategist" => {
                options.strategist = true;
                continue;
            }
            _ => {}
        }
        let mut value = || args.next().unwrap_or_else(|| usage(&format!("{flag} needs a value")));
        match flag.as_str() {
            "--matches" => options.matches = value().parse().unwrap_or_else(|_| usage("--matches")),
            "--parallel" => options.parallel = value().parse().unwrap_or_else(|_| usage("--parallel")),
            "--base-port" => options.base_port = value().parse().unwrap_or_else(|_| usage("--base-port")),
            "--speed" => options.speed = value().parse().unwrap_or_else(|_| usage("--speed")),
            "--max-minutes" => options.max_minutes = value().parse().unwrap_or_else(|_| usage("--max-minutes")),
            "--profile" => options.profile = value(),
            "--bot" => options.bot = Some(value().into()),
            "--disable" => options.disable = value(),
            "--ab-disable" => options.ab_disable = Some(value()),
            "--claude-config-dir" => options.claude_config_dir = Some(value()),
            "--side" => {
                options.side = Some(match value().to_lowercase().as_str() {
                    "armada" => "Armada",
                    "cortex" => "Cortex",
                    _ => usage("--side takes armada or cortex"),
                })
            }
            "--corner" => {
                options.corner = Some(match value().to_lowercase().as_str() {
                    "nw" => true,
                    "se" => false,
                    _ => usage("--corner takes nw or se"),
                })
            }
            "--map" => options.map = value(),
            "--label" => options.label = value(),
            _ => usage(&format!("unknown argument {flag}")),
        }
    }
    options
}

fn usage(problem: &str) -> ! {
    eprintln!("{problem}\nusage: arena [--matches N] [--parallel N] [--speed N] [--profile NAME] [--map NAME] [--max-minutes N] [--label TEXT] [--mirror] [--swap-corners] [--strategist | --commander] [--side armada|cortex] [--corner nw|se] [--bot PATH] [--disable H-ID,H-ID] [--ab-disable H-ID,H-ID] [--claude-config-dir DIR] [--base-port N]");
    std::process::exit(2)
}

