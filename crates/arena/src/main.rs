//! Batch evaluation harness. Runs headless matches of our bot against BARb, several at a time,
//! and reports win rates. Each match gets its own directory under `run/matches/<batch>/` holding
//! the start script, engine log, bot log and replay.
//!
//! usage: arena [--matches N] [--parallel N] [--speed N] [--profile easy|medium|hard|hard_aggressive]
//!              [--map NAME] [--max-minutes N] [--label TEXT] [--mirror] [--place] [--swap-corners] [--play-out]
//!              [--side armada|cortex] [--corner nw|se]   (default: alternate; `nw` is the first start box of the layout, W on Comet)
//!              [--ours N] [--allies N] [--enemies N] [--ffa]   (seats of ours, allied BARb seats, enemy BARb seats, default 1 0 1;
//!                                                               --ffa: every enemy seat is its own team)
//!              [--boxes standard|corners|north-south|west-east]   (where the ally teams start; default the lobby's boxes for the map)
//!              [--bot PATH]   (bot binary from another build, for A/B runs)
//!              [--disable H-ID,H-ID]   (ablation: switch heuristics off by registry ID)
//!              [--ab-disable H-ID,H-ID]   (interleaved A/B: arm B also switches these off; blocks of four matches)
//!              [--claude-config-dir DIR]   (subscription for --strategist sessions; default ~/.claude2)
//!              [--effort low|medium|high|xhigh|max]   (the LLM session's `claude --effort`; default high)
//!              [--opponent-opening any|bots|vehicles]   (pins BARb's first factory by disabling the other; default any)
//!              [--think-penalty X]   (commander: its orders land X game seconds late per wall second it thought; 1 = as in a live game, default 0)
//!              [--seed-base N]   (default 1; match i plays seed N+i, for the engine and for BARb: a fresh N is a fresh set of games)
//!              [--opening-plan PATH]   (the bot plays this plan text instead of searching one; run/replay_plan.py writes one from a replay)
//!              [--base-port N]   (default 9100; match i uses N+2i and N+2i+1, so a second arena needs another range)
//!              [--strategist]   (Claude Code strategist per match; use with --speed 2 and few matches)
//!              [--commander]    (Sonnet field commander per match, one for all our seats; the game is held still during its turns, so any --speed)
//!              [--commander-each]   (a commander of its own for every seat of ours)
//!              [--commander-model ID]   (the session's model instead of the role's usual one, e.g. claude-opus-5)

mod place;
mod record;
mod script;

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
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
use script::{Boxes, MatchSetup};

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
    /// `--place`: the arena chooses our commander's start inside its box by the opening search (`place.rs`).
    place: bool,
    swap_corners: bool,
    /// End a game once it is settled (see [`Settled`]); `--play-out` turns it off.
    call_settled: bool,
    strategist: bool,
    commander: bool,
    /// With `commander`: every seat of ours gets its own session instead of one for the team.
    commander_each: bool,
    pianist: bool,
    commander_model: Option<String>,
    /// Play every match as this faction instead of alternating.
    side: Option<&'static str>,
    /// Fixes our start corner; otherwise it alternates.
    corner: Option<bool>,
    ours: usize,
    allies: usize,
    enemies: usize,
    free_for_all: bool,
    boxes: Boxes,
    /// Bot binary to run instead of this workspace's, for A/B runs against an older build.
    bot: Option<std::path::PathBuf>,
    /// Comma-separated heuristic IDs the bot should switch off (ablation).
    disable: String,
    /// Interleaved A/B: arm B additionally switches these heuristics off. Arms alternate in blocks
    /// of four matches, so each arm meets every corner and faction under the same machine conditions.
    ab_disable: Option<String>,
    /// Claude Code config dir for strategist sessions (which subscription they run on).
    claude_config_dir: Option<String>,
    seed_base: u32,
    /// `--opening-plan PATH`: the bot plays this plan (`buildorder::plan::Plan` text) instead of searching one.
    opening_plan: Option<String>,
    /// `bots`, `vehicles` or `any` (BARb's own choice, about 70 % bots on Quicksilver).
    opponent_opening: String,
    effort: Option<String>,
    think_penalty: Option<String>,
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
    our_corner: String,
    game_minutes: f32,
    wall_seconds: f32,
    /// The referee ended the game because it was settled (`Win` or `Loss` then says for whom), not the engine.
    called: bool,
    /// The engine's and the opponent's random seed.
    seed: u32,
    /// The opponent's first factory, from its ground truth (`WITHIN_REASON_OBSERVE=1`), else empty.
    opponent_first_factory: String,
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
            "max_minutes": options.max_minutes, "mirror": options.mirror, "place": options.place, "call_settled": options.call_settled, "swap_corners": options.swap_corners, "strategist": options.strategist, "commander": options.commander, "commander_each": options.commander_each, "commander_model": options.commander_model, "effort": options.effort, "think_penalty": options.think_penalty, "seed_base": options.seed_base, "opening_plan": options.opening_plan, "opponent_opening": options.opponent_opening, "side": options.side, "corner": options.corner.map(|first| if first { "first box" } else { "second box" }), "ours": options.ours, "allies": options.allies, "enemies": options.enemies, "ffa": options.free_for_all, "boxes": format!("{:?}", options.boxes), "bot": options.bot, "disable": options.disable, "ab_disable": options.ab_disable,
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
                        MatchResult { index, arm: "", outcome: Outcome::Aborted, our_side: "?", our_corner: "?".into(), game_minutes: 0.0, wall_seconds: 0.0, called: false, seed: 0, opponent_first_factory: String::new() }
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
        "== {} wins, {} losses, {} timeouts, {} aborted ({} of the wins and {} of the losses called by the referee) ==",
        count(Outcome::Win), count(Outcome::Loss), count(Outcome::Timeout), count(Outcome::Aborted),
        results.iter().filter(|r| r.called && r.outcome == Outcome::Win).count(),
        results.iter().filter(|r| r.called && r.outcome == Outcome::Loss).count(),
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
        // Its first factory decides the kind of game (K-barb-opening-varies); taking the other away pins it.
        opponent_disabled_units: match options.opponent_opening.as_str() {
            "bots" => "armvp+corvp",
            "vehicles" => "armlab+corlab",
            _ => "",
        },
        host_port,
        autohost_port: host_port + 1,
        // The engine's and the opponent's dice. Match i has the same seed in every batch, and in an A/B batch the two
        // arms meet the same seeds (match i of arm A and match i + 4 of arm B, which also share corner and faction).
        seed: options.seed_base + if options.ab_disable.is_some() { (index / 8 * 4 + index % 4) as u32 } else { index as u32 },
        // Alternate corner and faction so neither start nor side biases the batch.
        // `--corner` names the corner; which team slot that means depends on `--swap-corners`.
        we_are_first: options.corner.map_or(index.is_multiple_of(2), |north_west| north_west != options.swap_corners),
        our_side: options.side.unwrap_or(if (index / 2).is_multiple_of(2) { "Armada" } else { "Cortex" }),
        mirror: options.mirror,
        starts: Vec::new(),
        swap_corners: options.swap_corners,
        ours: options.ours,
        allies: options.allies,
        enemies: options.enemies,
        free_for_all: options.free_for_all,
        boxes: options.boxes,
    };
    let mut setup = setup;
    // `--place`: our spawn is the opening search's choice inside our box (a 1v1 only: the opponent keeps the spawn it
    // had in the earlier match the map is read from). In team order, which is ally-team order.
    if options.place && options.ours == 1 && options.allies == 0 && options.enemies == 1 {
        match place::choose_starts(repo, &options.map, setup.our_rect(), setup.their_rect(), &setup.our_side.to_lowercase()[..3]) {
            Ok((ours, theirs)) => setup.starts = if setup.we_are_first { vec![ours, theirs] } else { vec![theirs, ours] },
            Err(why) => eprintln!("match {index}: not placed: {why}"),
        }
    }
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
        .args(options.commander.then_some(if options.commander_each { "--commander-each" } else { "--commander" }))
        .args(options.pianist.then_some("--pianist"))
        // The pianist's every request and answer, for study (`docs/design/2026-09-21-pianist.md`).
        .envs(options.pianist.then_some(("WITHIN_REASON_JEV_LOG", "1")))
        .env("WITHIN_REASON_SOCKET", &socket)
        .env("WITHIN_REASON_LOG_DIR", &dir)
        .env("WITHIN_REASON_DISABLE", &disable)
        .envs(options.opening_plan.as_ref().map(|path| ("WITHIN_REASON_OPENING_PLAN", path)))
        // Every match leaves a record for `run/view_match.py`: 0.1-0.3 MB per game minute (docs/harness/record-format.md).
        .env("WITHIN_REASON_RECORD", "1")
        .envs(options.claude_config_dir.as_ref().map(|dir| ("WITHIN_REASON_CLAUDE_CONFIG_DIR", dir)))
        .envs(options.effort.as_ref().map(|effort| ("WITHIN_REASON_EFFORT", effort)))
        .envs(options.commander_model.as_ref().map(|model| ("WITHIN_REASON_MODEL", model)))
        .envs(options.think_penalty.as_ref().map(|penalty| ("WITHIN_REASON_THINK_PENALTY", penalty)))
        .stderr(File::create(dir.join("bot.log"))?)
        .spawn()?;
    let log = File::create(dir.join("engine.log"))?;
    let mut engine = Command::new(repo.join("run/engine/spring-headless"))
        .args(["--isolation", "--write-dir"])
        .arg(&dir)
        .arg(&script_path)
        .env("SPRING_DATADIR", repo.join("run/data"))
        // The shim waits for each of the bot's answers. At arena speed a frame is under a millisecond, so any thinking
        // the bot does (a commander's turn, the opening search's few hundred milliseconds) would otherwise cost game
        // time it does not cost in a game played at speed 1.
        .env("WITHIN_REASON_LOCKSTEP", "1")
        .env("WITHIN_REASON_TRACE_BUILDS", "1")
        // Every match leaves the opponent's ground truth and a census for study (the shim reads them with cheat access
        // for the length of the query; the bot never sees them). It used to take the caller's environment to switch on,
        // and a game launched without it cannot be studied afterwards.
        .env("WITHIN_REASON_OBSERVE", "1")
        .env("WITHIN_REASON_TRUTH_DIR", &dir)
        .envs(options.call_settled.then_some(("WITHIN_REASON_BALANCE", "1")))
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
    let (outcome, called) = result?;
    let game_minutes = last_frame(&dir.join("engine.log")) as f32 / (30.0 * 60.0);
    let result = MatchResult {
        index,
        arm,
        outcome,
        our_side: setup.our_side,
        our_corner: setup.our_box(),
        game_minutes,
        wall_seconds: started.elapsed().as_secs_f32(),
        called,
        seed: setup.seed,
        opponent_first_factory: first_factory(&dir),
    };
    if let Err(e) = record::finish(&dir, &result, &options.profile) {
        eprintln!("match {index}: could not close the match record: {e}");
    }
    Ok(result)
}

/// The first factory in the opponent's ground-truth log, by name; empty without a log.
fn first_factory(dir: &Path) -> String {
    const FACTORIES: [&str; 10] = ["armlab", "armvp", "armap", "armsy", "armhp", "corlab", "corvp", "corap", "corsy", "corhp"];
    let Some(truth) = fs::read_dir(dir).ok().and_then(|entries| {
        entries.flatten().map(|e| e.path()).find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("truth-")))
    }) else {
        return String::new();
    };
    let text = fs::read_to_string(truth).unwrap_or_default();
    text.lines()
        .find_map(|line| FACTORIES.iter().find(|name| line.contains(&format!("\"{name}\""))))
        .map_or(String::new(), |name| name.to_string())
}

/// Follows one match over the autohost channel until it is decided or out of time.
fn referee(
    autohost: &mut Autohost,
    engine: &mut Child,
    bot: &mut Child,
    options: &Options,
    setup: &MatchSetup,
    engine_log: &Path,
) -> io::Result<(Outcome, bool)> {
    let mut playing = false;
    let mut settled = Settled::default();
    // The game clock stands still during the commander's turns, and the log only shows it once a game minute.
    let stall_allowance = if options.commander { 10 * STALL_ALLOWANCE } else { STALL_ALLOWANCE };
    let mut deadline = Instant::now() + LOAD_ALLOWANCE;
    let frame_limit = options.max_minutes * 60 * 30;
    let mut last_seen_frame = 0;
    loop {
        if engine.try_wait()?.is_some() {
            return Ok((Outcome::Aborted, false));
        }
        if let Some(status) = bot.try_wait()? {
            // Without its bot our team stands idle; the result would be meaningless.
            return Err(io::Error::other(format!("bot process exited ({status}); see bot.log")));
        }
        if Instant::now() > deadline {
            return Ok((if playing { Outcome::Timeout } else { Outcome::Aborted }, false));
        }
        if playing {
            // Game time comes from the shim's heartbeat lines; the deadline only catches a stalled clock.
            let frame = last_frame(engine_log);
            if frame >= frame_limit {
                return Ok((Outcome::Timeout, false));
            }
            if options.call_settled && let Some(outcome) = settled.judge(engine_log) {
                return Ok((outcome, true));
            }
            // The hand-operated kill switch (`run/stop_match.py`): a file `stop` in the match directory ends the match
            // the way every other match ends, the engine told to quit and given time to write its replay.
            if let Ok(verdict) = fs::read_to_string(engine_log.with_file_name("stop")) {
                let outcome = match verdict.trim() {
                    "win" => Outcome::Win,
                    "loss" => Outcome::Loss,
                    _ => Outcome::Timeout,
                };
                return Ok((outcome, true));
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
                return Ok((if won { Outcome::Win } else { Outcome::Loss }, false));
            }
            Some(Event::Other) | None => {}
        }
    }
}

/// Calls a game whose outcome is no longer in doubt, from the shim's `balance` lines (both sides' soldiers' metal
/// and extractors, every 30 game seconds). A game is settled when one side has led on army AND extractors by the
/// ratios of its [`Lead`] for long enough. Results carry `called: true`, so called games can be told from played-out
/// ones; `--play-out` turns the referee's calls off.
#[derive(Default)]
struct Settled {
    /// Frame since which the side has been that far ahead without a break; `true` is us.
    ahead_since: Option<(bool, u32)>,
    last_frame: u32,
}

/// What "hopelessly ahead" means for one side: `army` and `extractors` are ratios over the other side, held for
/// `for_frames`, not before `from_frame`.
struct Lead {
    army: f32,
    extractors: f32,
    for_frames: u32,
    from_frame: u32,
}

/// Their lead over us. Replayed over the 24 recorded games of v17-truth-medium it called 10 of the 14 losses, 1 to 16
/// minutes before the end, and nothing that was not a loss.
const THEIR_LEAD: Lead = Lead { army: 3.0, extractors: 3.0, for_frames: 120 * 30, from_frame: 6 * 60 * 30 };
/// Our lead over them has to be bigger and last longer: BARb comes back. In that batch a game we led 14 extractors to 2
/// at minute 9 was level again at minute 24 and ended a 40-minute timeout.
const OUR_LEAD: Lead = Lead { army: 5.0, extractors: 3.0, for_frames: 300 * 30, from_frame: 12 * 60 * 30 };

impl Settled {
    fn judge(&mut self, engine_log: &Path) -> Option<Outcome> {
        let (frame, ours, theirs) = last_balance(engine_log)?;
        if frame <= self.last_frame {
            return None;
        }
        self.last_frame = frame;
        // An army under 300 metal and a pair of extractors count as that, so ratios against nothing stay finite.
        let ahead = |lead: &Lead, a: (f32, f32), b: (f32, f32)| a.0 >= lead.army * b.0.max(300.0) && a.1 >= lead.extractors * b.1.max(2.0);
        let leader = if ahead(&OUR_LEAD, ours, theirs) { Some(true) } else if ahead(&THEIR_LEAD, theirs, ours) { Some(false) } else { None };
        self.ahead_since = match (leader, self.ahead_since) {
            (Some(side), Some((since_side, since))) if side == since_side => Some((side, since)),
            (Some(side), _) => Some((side, frame)),
            (None, _) => None,
        };
        let (side, since) = self.ahead_since?;
        let lead = if side { &OUR_LEAD } else { &THEIR_LEAD };
        (frame >= lead.from_frame && frame - since >= lead.for_frames).then_some(if side { Outcome::Win } else { Outcome::Loss })
    }
}

/// The newest `balance f=N ours=ARMY/EXTRACTORS theirs=ARMY/EXTRACTORS` line of the engine log.
fn last_balance(engine_log: &Path) -> Option<(u32, (f32, f32), (f32, f32))> {
    const TAIL: u64 = 64 * 1024;
    let mut file = File::open(engine_log).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(TAIL))).ok()?;
    let mut tail = Vec::new();
    file.read_to_end(&mut tail).ok()?;
    let tail = String::from_utf8_lossy(&tail);
    let line = tail.lines().rev().find(|l| l.contains("balance f="))?;
    let field = |key: &str| line.split_whitespace().find_map(|word| word.strip_prefix(key));
    let pair = |text: &str| {
        let (army, extractors) = text.split_once('/')?;
        Some((army.parse::<f32>().ok()?, extractors.parse::<f32>().ok()?))
    };
    Some((field("f=")?.parse().ok()?, pair(field("ours=")?)?, pair(field("theirs=")?)?))
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
        place: false,
        swap_corners: false,
        call_settled: true,
        strategist: false,
        commander: false,
        commander_each: false,
        pianist: false,
        commander_model: None,
        side: None,
        corner: None,
        ours: 1,
        allies: 0,
        enemies: 1,
        free_for_all: false,
        boxes: Boxes::Standard,
        bot: None,
        disable: String::new(),
        ab_disable: None,
        claude_config_dir: None,
        seed_base: 1,
        opening_plan: None,
        opponent_opening: "any".into(),
        effort: None,
        think_penalty: None,
        base_port: BASE_PORT,
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--place" => {
                options.place = true;
                continue;
            }
            "--mirror" => {
                options.mirror = true;
                continue;
            }
            "--play-out" => {
                options.call_settled = false;
                continue;
            }
            "--ffa" => {
                options.free_for_all = true;
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
            "--commander-each" => {
                (options.commander, options.commander_each) = (true, true);
                continue;
            }
            "--strategist" => {
                options.strategist = true;
                continue;
            }
            "--pianist" => {
                options.pianist = true;
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
            "--effort" => options.effort = Some(value()),
            "--commander-model" => options.commander_model = Some(value()),
            "--think-penalty" => options.think_penalty = Some(value()),
            "--opponent-opening" => {
                options.opponent_opening = value();
                if !["any", "bots", "vehicles"].contains(&options.opponent_opening.as_str()) {
                    usage("--opponent-opening");
                }
            }
            "--seed-base" => options.seed_base = value().parse().unwrap_or_else(|_| usage("--seed-base")),
            "--opening-plan" => options.opening_plan = Some(std::fs::canonicalize(value()).unwrap_or_else(|_| usage("--opening-plan")).to_string_lossy().into_owned()),
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
            "--ours" => options.ours = value().parse().ok().filter(|n| *n >= 1).unwrap_or_else(|| usage("--ours takes 1 or more")),
            "--allies" => options.allies = value().parse().unwrap_or_else(|_| usage("--allies")),
            "--enemies" => options.enemies = value().parse().ok().filter(|n| *n >= 1).unwrap_or_else(|| usage("--enemies takes 1 or more")),
            "--boxes" => {
                options.boxes = match value().as_str() {
                    "standard" => Boxes::Standard,
                    "corners" => Boxes::Corners,
                    "north-south" => Boxes::NorthSouth,
                    "west-east" => Boxes::WestEast,
                    _ => usage("--boxes takes standard, corners, north-south or west-east"),
                }
            }
            "--map" => options.map = value(),
            "--label" => options.label = value(),
            _ => usage(&format!("unknown argument {flag}")),
        }
    }
    let ally_teams = 1 + if options.free_for_all { options.enemies } else { 1 };
    if options.boxes.rects(&options.map, ally_teams).is_none() {
        usage(&format!("no start boxes for {ally_teams} ally teams on {} with --boxes {:?}: the lobby has none saved for that count, or the layout has no room (--ffa takes at most 3 enemies, with corners)", options.map, options.boxes));
    }
    options
}

fn usage(problem: &str) -> ! {
    eprintln!("{problem}\nusage: arena [--matches N] [--parallel N] [--speed N] [--profile NAME] [--map NAME] [--max-minutes N] [--label TEXT] [--mirror] [--place] [--swap-corners] [--play-out] [--strategist | --commander | --commander-each] [--pianist] [--commander-model ID] [--side armada|cortex] [--corner nw|se] [--ours N] [--allies N] [--enemies N] [--ffa] [--boxes standard|corners|north-south|west-east] [--bot PATH] [--disable H-ID,H-ID] [--ab-disable H-ID,H-ID] [--claude-config-dir DIR] [--effort LEVEL] [--think-penalty X] [--opponent-opening any|bots|vehicles] [--seed-base N] [--opening-plan PATH] [--base-port N]");
    std::process::exit(2)
}

