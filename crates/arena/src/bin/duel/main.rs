//! Unit-against-unit duels: spawns two armies on flat ground in a headless match where both teams are our AI,
//! sends them at each other and records who is left. Many duels run per engine start, at several sites at once.
//! This process is the bot for both teams: the shims connect to it instead of `bot`. See `docs/harness/duels.md`.
//!
//! usage: duel (--units a,b,c | --ours a,b --theirs c,d | --pairs a:b,c:d) [--reps N] [--budget METAL | --count N]
//!             [--parallel N] [--sites N] [--duels-per-match N] [--time-limit SECONDS] [--sweep-waves N] [--spacing ELMOS] [--speed N] [--map NAME]
//!             [--label TEXT] [--base-port N]
//!        duel --report DIR [duels.csv ...]   (rebuild the tables in DIR, from its own duels.csv or the files named)

mod director;
mod plan;
mod report;
mod script;
mod sites;

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use arena::autohost::{Autohost, Event};
use arena::harness::{
    ENGINE_MEMORY_GB, GAME_TAG, LOAD_ALLOWANCE, REPO, available_memory_gb, copy_tree, git_commit, refresh_cache_template,
    resolve_game, stop,
};
use bot_protocol::{Commands, FrameReader, ToBot, write_frame};

use director::{Batch, Director};
use plan::{Job, Sizing};

/// A match whose director hears nothing for this long has hung.
const STALL_ALLOWANCE: Duration = Duration::from_secs(120);
/// A duel is given up after its match aborted this many times under it.
const MAX_ATTEMPTS: u32 = 2;

struct Options {
    pairs: Vec<(String, String)>,
    reps: u32,
    sizing: Sizing,
    parallel: usize,
    sites: usize,
    duels_per_match: u32,
    time_limit: i32,
    sweep_waves: u32,
    spacing: f32,
    speed: u32,
    map: String,
    label: String,
    base_port: u16,
}

fn main() -> io::Result<()> {
    let options = parse_args()?;
    let repo = fs::canonicalize(REPO)?;
    let status = Command::new(repo.join("run/install_ai.sh")).stdout(Stdio::null()).status()?;
    if !status.success() {
        return Err(io::Error::other("install_ai.sh failed"));
    }
    let needed_gb = options.parallel as u64 * ENGINE_MEMORY_GB;
    let available_gb = available_memory_gb()?;
    if available_gb < needed_gb {
        return Err(io::Error::other(format!("{} engines need about {needed_gb} GB, only {available_gb} GB available", options.parallel)));
    }

    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let batch_dir = repo.join(format!("run/matches/{stamp}-duel-{}", options.label));
    fs::create_dir_all(&batch_dir)?;
    let jobs = plan::jobs(&options.pairs, options.reps);
    println!("{} duels ({} pairings x {}) on {} -> {}", jobs.len(), options.pairs.len(), options.reps, options.map, batch_dir.display());
    fs::write(
        batch_dir.join("batch.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "label": options.label, "commit": git_commit(&repo), "map": options.map, "pairings": options.pairs.len(),
            "reps": options.reps, "sizing": format!("{:?}", options.sizing), "time_limit": options.time_limit,
            "speed": options.speed, "sweep_waves": options.sweep_waves, "spacing": options.spacing, "sites": options.sites, "duels_per_match": options.duels_per_match,
        }))?,
    )?;

    let csv_path = batch_dir.join("duels.csv");
    fs::write(&csv_path, format!("{}\n", report::HEADER))?;
    let csv = Mutex::new(OpenOptions::new().append(true).open(&csv_path)?);
    let total = jobs.len();
    let done = AtomicUsize::new(0);
    let batch = Arc::new(Batch {
        queue: Mutex::new(VecDeque::from(jobs)),
        results: Mutex::new(Vec::new()),
        sizing: options.sizing,
        time_limit: options.time_limit,
        sweep_waves: options.sweep_waves,
        spacing: options.spacing,
        on_result: Box::new(move |result| {
            // Written as they finish, so an interrupted batch keeps what it has.
            let _ = writeln!(csv.lock().unwrap(), "{}", report::row(result));
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            println!(
                "[{n}/{total}] {}x{} vs {}x{}: {} ({}) {:.0}s, left {:.0}% / {:.0}%",
                result.count[0], result.job.x, result.count[1], result.job.y, result.winner, result.reason,
                result.seconds, 100.0 * result.value_left[0], 100.0 * result.value_left[1]
            );
        }),
    });

    let started = Instant::now();
    let options = Arc::new(options);
    let next_match = Arc::new(AtomicUsize::new(0));
    let workers: Vec<_> = (0..options.parallel)
        .map(|_| {
            let (batch, options, repo, batch_dir, next_match) =
                (batch.clone(), options.clone(), repo.clone(), batch_dir.clone(), next_match.clone());
            std::thread::spawn(move || {
                let mut failures = 0;
                while !batch.queue.lock().unwrap().is_empty() && failures < 3 {
                    let index = next_match.fetch_add(1, Ordering::Relaxed);
                    match run_match(&repo, &batch_dir, &options, &batch, index) {
                        Ok(()) => failures = 0,
                        Err(e) => {
                            eprintln!("match {index}: {e}");
                            failures += 1;
                        }
                    }
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    refresh_cache_template(&repo, &batch_dir);

    let left = batch.queue.lock().unwrap().len();
    let ran = batch.results.lock().unwrap().len();
    println!("== {ran} duels in {:.0}s over {} matches; {left} not run ==", started.elapsed().as_secs_f32(), next_match.load(Ordering::Relaxed));
    report::write(&[&csv_path], &batch_dir)?;
    println!("{}\n{}", csv_path.display(), batch_dir.join("matrix.md").display());
    Ok(())
}

/// One engine start: duels run until the queue is empty or the match has had its share.
fn run_match(repo: &Path, batch_dir: &Path, options: &Options, batch: &Arc<Batch>, index: usize) -> io::Result<()> {
    let dir = batch_dir.join(format!("{index:02}"));
    fs::create_dir_all(&dir)?;
    let host_port = options.base_port + 2 * index as u16;
    copy_tree(&repo.join("run/match-template"), &dir)?;
    let cache_template = repo.join("run/cache-template");
    if cache_template.is_dir() {
        copy_tree(&cache_template, &dir.join("cache"))?;
    }
    let script_path = dir.join("script.txt");
    fs::write(&script_path, script::render(&resolve_game(repo, GAME_TAG)?, &options.map, host_port, host_port + 1, index as u32 + 1))?;

    let mut autohost = Autohost::bind(host_port + 1)?;
    // Unix socket paths are limited to ~108 bytes, so the socket cannot live in the match directory.
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, Into::into);
    let socket = runtime_dir.join(format!("wreason-duel-{}-{index}.sock", std::process::id()));
    let _ = fs::remove_file(&socket);
    let director = Arc::new(Mutex::new(Director::new(batch.clone(), index, options.sites, options.duels_per_match)));
    let closing = Arc::new(AtomicBool::new(false));
    let listener = serve(&socket, director.clone(), closing.clone())?;

    let log = File::create(dir.join("engine.log"))?;
    let mut engine = Command::new(repo.join("run/engine/spring-headless"))
        .args(["--isolation", "--write-dir"])
        .arg(&dir)
        .arg(&script_path)
        .env("SPRING_DATADIR", repo.join("run/data"))
        .env("WITHIN_REASON_SOCKET", &socket)
        // The shim waits for the director's answer to every tick, so orders land on the frame they were decided
        // for however fast the game runs.
        .env("WITHIN_REASON_LOCKSTEP", "1")
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?;

    let mut playing = false;
    let load_deadline = Instant::now() + LOAD_ALLOWANCE;
    let outcome = loop {
        if engine.try_wait()?.is_some() {
            break Err(io::Error::other("engine exited; see engine.log"));
        }
        {
            let director = director.lock().unwrap();
            if let Some(problem) = &director.fatal {
                // Nothing a retry would cure: drop the plan so the workers stop.
                batch.queue.lock().unwrap().clear();
                break Err(io::Error::other(problem.clone()));
            }
            if director.finished {
                break Ok(());
            }
            if playing && director.last_tick.elapsed() > STALL_ALLOWANCE {
                break Err(io::Error::other("no tick from the engine for two minutes"));
            }
        }
        if !playing && Instant::now() > load_deadline {
            break Err(io::Error::other("engine did not start playing"));
        }
        match autohost.receive(Duration::from_millis(200))? {
            Some(Event::StartPlaying) => {
                // Raising the minimum forces the server's speed up; the maximum has to allow it first.
                autohost.send(&format!("/setmaxspeed {}", options.speed))?;
                autohost.send(&format!("/setminspeed {}", options.speed))?;
                playing = true;
                director.lock().unwrap().last_tick = Instant::now();
            }
            Some(Event::GameOver { .. }) => break Err(io::Error::other("the game ended under the duels (a commander died?)")),
            Some(Event::Other) | None => {}
        }
    };
    stop(&mut engine, &mut autohost);
    closing.store(true, Ordering::Relaxed);
    let _ = listener.join();
    let _ = fs::remove_file(&socket);
    let mut queue = batch.queue.lock().unwrap();
    for job in director.lock().unwrap().unfinished() {
        if job.attempts + 1 < MAX_ATTEMPTS {
            queue.push_back(Job { attempts: job.attempts + 1, ..job });
        } else {
            eprintln!("gave up on {} vs {} (rep {})", job.x, job.y, job.rep);
        }
    }
    outcome
}

/// Accepts the two shims' connections and answers their ticks from the director until `closing` is set.
fn serve(socket: &Path, director: Arc<Mutex<Director>>, closing: Arc<AtomicBool>) -> io::Result<std::thread::JoinHandle<()>> {
    let listener = UnixListener::bind(socket)?;
    listener.set_nonblocking(true)?;
    Ok(std::thread::spawn(move || {
        while !closing.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let director = director.clone();
                    std::thread::spawn(move || {
                        // Ends with an error when the engine goes away, which is how every session ends.
                        let _ = session(stream, &director);
                    });
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => return eprintln!("duel socket: {e}"),
            }
        }
    }))
}

fn session(mut stream: UnixStream, director: &Mutex<Director>) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    let mut input = stream.try_clone()?;
    let mut reader = FrameReader::default();
    let mut next = move || reader.read::<ToBot>(&mut input).map(|m| m.expect("blocking socket"));
    let ToBot::Hello(hello) = next()? else {
        return Err(io::Error::other("expected Hello first"));
    };
    let team = hello.team;
    director.lock().unwrap().hello(hello);
    write_frame(&mut stream, &Commands::default())?;
    loop {
        let ToBot::Tick(tick) = next()? else {
            return Err(io::Error::other("unexpected second Hello"));
        };
        let commands = director.lock().unwrap().tick(team, &tick);
        write_frame(&mut stream, &Commands(commands))?;
    }
}

fn parse_args() -> io::Result<Options> {
    let mut options = Options {
        pairs: Vec::new(),
        reps: 4,
        sizing: Sizing::EqualMetal(1200.0),
        parallel: 2,
        sites: 3,
        duels_per_match: 45,
        time_limit: 240,
        sweep_waves: 3,
        spacing: 56.0,
        speed: 50,
        map: "Quicksilver Remake 1.24".into(),
        label: "batch".into(),
        base_port: 9500,
    };
    let list = |text: String| text.split(',').map(str::to_string).collect::<Vec<_>>();
    let (mut ours, mut theirs) = (Vec::new(), Vec::new());
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == "--report" {
            let dir = PathBuf::from(args.next().unwrap_or_else(|| usage("--report needs a directory")));
            let named: Vec<PathBuf> = args.map(PathBuf::from).collect();
            let inputs = if named.is_empty() { vec![dir.join("duels.csv")] } else { named };
            report::write(&inputs.iter().map(PathBuf::as_path).collect::<Vec<_>>(), &dir)?;
            std::process::exit(0);
        }
        let mut value = || args.next().unwrap_or_else(|| usage(&format!("{flag} needs a value")));
        let number = |text: String| text.parse::<u32>().unwrap_or_else(|_| usage(&format!("{flag} takes a number")));
        match flag.as_str() {
            "--units" => options.pairs.extend(plan::all_pairs(&list(value()))),
            "--ours" => ours = list(value()),
            "--theirs" => theirs = list(value()),
            "--pairs" => {
                for pair in list(value()) {
                    let (x, y) = pair.split_once(':').unwrap_or_else(|| usage("--pairs takes x:y,x:y"));
                    options.pairs.push((x.to_string(), y.to_string()));
                }
            }
            "--reps" => options.reps = number(value()),
            "--budget" => options.sizing = Sizing::EqualMetal(number(value()) as f32),
            "--count" => options.sizing = Sizing::EqualCount(number(value())),
            "--parallel" => options.parallel = number(value()) as usize,
            "--sites" => options.sites = number(value()) as usize,
            "--duels-per-match" => options.duels_per_match = number(value()),
            "--time-limit" => options.time_limit = number(value()) as i32,
            "--sweep-waves" => options.sweep_waves = number(value()),
            "--spacing" => options.spacing = number(value()) as f32,
            "--speed" => options.speed = number(value()),
            "--base-port" => options.base_port = number(value()) as u16,
            "--map" => options.map = value(),
            "--label" => options.label = value(),
            _ => usage(&format!("unknown argument {flag}")),
        }
    }
    options.pairs.extend(plan::cross(&ours, &theirs));
    if options.pairs.is_empty() {
        usage("no pairings: give --units, --ours with --theirs, or --pairs");
    }
    Ok(options)
}

fn usage(problem: &str) -> ! {
    eprintln!("{problem}\nusage: duel (--units a,b,c | --ours a,b --theirs c,d | --pairs a:b,c:d) [--reps N] [--budget METAL | --count N] [--parallel N] [--sites N] [--duels-per-match N] [--time-limit SECONDS] [--sweep-waves N] [--spacing ELMOS] [--speed N] [--map NAME] [--label TEXT] [--base-port N]\n       duel --report DIR [duels.csv ...]");
    std::process::exit(2)
}
