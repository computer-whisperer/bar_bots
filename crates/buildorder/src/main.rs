use std::fmt::Write as _;

use buildorder::anneal::{anneal_restarts, Objective, Palette, Search};
use std::sync::Arc;

use buildorder::game::{Game, Ground, Straight};
use buildorder::plan::Plan;
use buildorder::record;
use buildorder::sim::{simulate, Outcome, Sample, Scenario, Wind};

const USAGE: &str = "usage: (the game, that is map, start, faction and unit numbers, comes from a match record's header)
  buildorder optimize  --game RECORD.jsonl [--factory lab|vp] [--objective income|army|mix|tempo] [--minutes 10]
                       [--iterations 40000] [--restarts 8] [--seed 1] [--wind MEAN] [--detour X] [--factories 2] [--constructors 6] [--leash ELMOS]
                       [--no-nano] [--turret llt] [--csv FILE] [--plan-out FILE]      (--detour X: open ground, every walk X straight lines,
                                                                        in place of the map's own ground)
  buildorder simulate  --game RECORD.jsonl --plan FILE [--minutes ..] [--wind ..] [--detour X] [--csv FILE]
  buildorder walks     RECORD.jsonl... [--minutes 6]      (builders' ways between builds: recorded against predicted)
  buildorder calibrate RECORD.jsonl... [--minutes 10] [--detour X] [--constant-wind] [--trace] [--csv-dir DIR]";

struct Args(Vec<String>);

impl Args {
    fn value(&self, flag: &str) -> Option<&str> {
        self.0.iter().position(|a| a == flag).and_then(|i| self.0.get(i + 1)).map(String::as_str)
    }
    fn text(&self, flag: &str, default: &str) -> String {
        self.value(flag).unwrap_or(default).to_string()
    }
    fn number<T: std::str::FromStr>(&self, flag: &str, default: T) -> T {
        self.value(flag).map_or(default, |v| v.parse().unwrap_or_else(|_| die(&format!("bad value for {flag}: {v}"))))
    }
    fn has(&self, flag: &str) -> bool {
        self.0.iter().any(|a| a == flag)
    }
    /// Arguments that are neither flags nor flag values.
    fn positional(&self, flags_without_value: &[&str]) -> Vec<&str> {
        let mut out = Vec::new();
        let mut i = 1;
        while i < self.0.len() {
            let a = self.0[i].as_str();
            if a.starts_with("--") {
                i += if flags_without_value.contains(&a) { 1 } else { 2 };
            } else {
                out.push(a);
                i += 1;
            }
        }
        out
    }
}

fn die(message: &str) -> ! {
    eprintln!("{message}\n{USAGE}");
    std::process::exit(2)
}

/// The game of `--game`'s record, and the search's scenario in it: our half of the spots over open ground.
fn game(args: &Args) -> Game {
    let path = args.value("--game").unwrap_or_else(|| die("--game RECORD.jsonl is required"));
    record::read(path, 60.0).unwrap_or_else(|e| die(&e)).game
}

/// The map's own ground; with `--detour X`, open ground where every walk is X straight lines.
fn ground(game: &Game, args: &Args) -> Arc<dyn Ground> {
    match args.value("--detour") {
        Some(_) => Arc::new(Straight { detour: args.number("--detour", 1.05) }),
        None => game.ground(),
    }
}

fn scenario(game: &Game, args: &Args) -> Scenario {
    let mut scenario = game.scenario(game.own_half(), ground(game, args));
    scenario.wind = Wind::Constant(args.number("--wind", game.mean_wind()));
    scenario.constructors_default_to_extractors = true;
    scenario.commander_leash = args.number("--leash", f64::MAX);
    scenario
}

const CSV_HEADER: &str = "t,metal,energy,metal_income,energy_income,extractors,converters,build_power,factories,constructors,nanos,army_count,army_value,stall,metal_wasted,energy_wasted";

fn csv_row(s: &Sample) -> String {
    format!(
        "{},{:.0},{:.0},{:.2},{:.1},{},{},{:.0},{},{},{},{},{:.0},{:.2},{:.0},{:.0}",
        s.t, s.metal, s.energy, s.metal_income, s.energy_income, s.extractors, s.converters, s.build_power, s.factories,
        s.constructors, s.nanos, s.army_count, s.army_value, s.stall, s.metal_wasted, s.energy_wasted
    )
}

fn write_csv(path: &str, outcome: &Outcome) {
    let mut text = format!("{CSV_HEADER}\n");
    for sample in outcome.samples.iter().filter(|s| (s.t as u32).is_multiple_of(10)) {
        text.push_str(&csv_row(sample));
        text.push('\n');
    }
    std::fs::write(path, text).unwrap_or_else(|e| die(&format!("{path}: {e}")));
}

const MILESTONES: [f64; 4] = [3.0, 5.0, 8.0, 10.0];

/// Markdown rows: one per milestone minute the outcome reaches.
fn milestone_rows(label: &str, outcome: &Outcome) -> String {
    let mut out = String::new();
    for minute in MILESTONES {
        let Some(s) = outcome.samples.iter().find(|s| s.t == minute * 60.0) else { continue };
        let income = outcome.mean_metal_income(s.t, 30.0);
        let stall: f64 = outcome.samples.iter().filter(|x| x.t <= s.t).map(|x| x.stall).sum::<f64>() / s.t;
        let _ = writeln!(
            out,
            "| {label} | {minute:.0} | {} | {income:.1} | {:.0} | {:.0} | {} | {} | {} | {} | {:.0} | {} | {:.2} | {:.0} / {:.0} |",
            s.extractors, s.energy_income, s.build_power, s.factories, s.constructors, s.nanos, s.converters, s.army_value,
            s.army_count, stall, s.metal_wasted, s.energy_wasted
        );
    }
    out
}

const MILESTONE_HEADER: &str = "| run | min | mex | metal/s | energy/s | build power | labs | cons | nanos | conv | army metal | army n | mean stall | wasted M / E |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n";

fn optimize(args: &Args) {
    let game = game(args);
    let units = &game.units;
    let factory = args.text("--factory", "lab");
    let objective = Objective::parse(&args.text("--objective", "mix")).unwrap_or_else(|| die("unknown objective"));
    let minutes: f64 = args.number("--minutes", 10.0);
    let scenario = scenario(&game, args);
    let factory_unit = game.factory(&factory).unwrap_or_else(|| die(&format!("the commander builds no {factory}")));
    let palette = Palette::new(units, game.commander, factory_unit, !args.has("--no-nano"), units.index(&format!("{}{}", game.side(), args.text("--turret", "llt"))));
    let search = Search {
        objective,
        horizon: minutes * 60.0,
        iterations: args.number("--iterations", 40_000),
        seed: args.number("--seed", 1),
        factories: args.number("--factories", 2),
        constructors: args.number("--constructors", 6),
        hot: args.number("--hot", 0.02),
        start: None,
    };
    let found = anneal_restarts(units, &scenario, &palette, &search, args.number("--restarts", 8));
    println!("# {} {factory} from {:.0},{:.0}, objective {} at {minutes} min, score {:.1}", game.side(), game.home.0, game.home.1, objective.name(), found.score);
    print!("{}", found.plan.to_text(units));
    print!("\n{MILESTONE_HEADER}{}", milestone_rows("best", &found.outcome));
    if let Some(path) = args.value("--csv") {
        write_csv(path, &found.outcome);
    }
    if let Some(path) = args.value("--plan-out") {
        std::fs::write(path, found.plan.to_text(units)).unwrap_or_else(|e| die(&format!("{path}: {e}")));
    }
}

fn run_plan(args: &Args) {
    let game = game(args);
    let units = &game.units;
    let path = args.value("--plan").unwrap_or_else(|| die("--plan FILE is required"));
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| die(&format!("{path}: {e}")));
    let plan = Plan::from_text(&text, game.side(), units).unwrap_or_else(|e| die(&e));
    let outcome = simulate(units, &scenario(&game, args), &plan, args.number("--minutes", 10.0) * 60.0);
    print!("{MILESTONE_HEADER}{}", milestone_rows(path, &outcome));
    for done in &outcome.finished {
        println!("{:6.1} s  {}  (queue {})", done.t, units.list[done.unit].name, done.queue);
    }
    if let Some(path) = args.value("--csv") {
        write_csv(path, &outcome);
    }
}

fn calibrate(args: &Args) {
    let minutes: f64 = args.number("--minutes", 10.0);
    let records = args.positional(&["--constant-wind", "--trace"]);
    if records.is_empty() {
        die("calibrate needs at least one record");
    }
    // Per minute: sums of |sim - observed| and of observed. Once over every record, once over the records that had
    // lost at most `QUIET_LOSSES` units by that minute (the simulator knows no losses, so only those test its arithmetic).
    const QUIET_LOSSES: u32 = 2;
    let mut errors = vec![[[0.0f64; 8]; 2]; minutes as usize + 1];
    // Per minute, per record: recorded (extractors alive, metal/s, energy/s, army metal built), for the medians.
    let mut recorded = vec![Vec::<[f64; 4]>::new(); minutes as usize + 1];
    for path in &records {
        let replay = record::read(path, minutes * 60.0).unwrap_or_else(|e| die(&e));
        let (game, units) = (&replay.game, &replay.game.units);
        let mut scenario = game.scenario(game.own_half(), ground(game, args));
        if !args.has("--constant-wind") {
            let mut last = game.mean_wind();
            scenario.wind = Wind::Trace(std::iter::once(last).chain(replay.wind.iter().map(|w| { last = w.unwrap_or(last); last })).collect());
        }
        let outcome = simulate(units, &scenario, &replay.plan, minutes * 60.0);
        println!("\n## {path}\nside {}, home {:.0},{:.0}; builds outside the unit table: {:?}; started but never finished (left out): {}",
            game.side(), game.home.0, game.home.1, replay.unknown, replay.abandoned);
        let sim_factory = outcome.finished.iter().find(|f| units.list[f.unit].role == buildorder::units::Role::Factory).map(|f| f.t);
        println!("first factory finished: recorded {:?} s, simulated {:?} s", replay.first_factory_finished, sim_factory);
        println!("| min | mex alive (built) rec / sim | metal/s rec/sim | energy/s rec/sim | cons rec/sim | army metal built rec/sim | units lost (rec) |\n|---|---|---|---|---|---|---|");
        for minute in 1..=minutes as usize {
            let t = minute as f64 * 60.0;
            let (Some(o), Some(s)) = (replay.observed.iter().find(|o| o.t == t), outcome.samples.iter().find(|s| s.t == t)) else { continue };
            // Both incomes as 30 s means: wind and converters make single seconds noisy.
            let mean = |pick: &dyn Fn(f64) -> Option<f64>| { let v: Vec<f64> = (0..30).filter_map(|k| pick(t - k as f64)).collect(); v.iter().sum::<f64>() / v.len().max(1) as f64 };
            let om = mean(&|at| replay.observed.iter().find(|o| o.t == at).map(|o| o.metal_income));
            let oe = mean(&|at| replay.observed.iter().find(|o| o.t == at).map(|o| o.energy_income));
            let sm = mean(&|at| outcome.samples.iter().find(|s| s.t == at).map(|s| s.metal_income));
            let se = mean(&|at| outcome.samples.iter().find(|s| s.t == at).map(|s| s.energy_income));
            println!("| {minute} | {} ({}) / {} | {om:.1} / {sm:.1} | {oe:.0} / {se:.0} | {} / {} | {:.0} / {:.0} | {} |",
                o.extractors, o.extractors_built, s.extractors, o.constructors, s.constructors, o.army_value, s.army_value, o.losses);
            recorded[minute].push([o.extractors as f64, om, oe, o.army_value]);
            for e in errors[minute].iter_mut().take(if o.losses <= QUIET_LOSSES { 2 } else { 1 }) {
            e[0] += (s.extractors as f64 - o.extractors as f64).abs(); e[1] += o.extractors as f64;
            e[2] += (sm - om).abs(); e[3] += om;
            e[4] += (s.army_value - o.army_value).abs(); e[5] += o.army_value;
            e[6] += sm - om; e[7] += 1.0;
            }
        }
        if args.has("--trace") {
            print!("replayed plan:\n{}", replay.plan.to_text(units));
            for done in &outcome.finished {
                println!("{:6.1} s  {}  (queue {})", done.t, units.list[done.unit].name, done.queue);
            }
        }
        if let Some(dir) = args.value("--csv-dir") {
            let name = path.trim_end_matches("/record-0.jsonl").rsplit('/').take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("-");
            let mut text = String::from("t,rec_metal_income,sim_metal_income,rec_energy_income,sim_energy_income,rec_extractors,sim_extractors,rec_army_value,sim_army_value,rec_losses,rec_metal,sim_metal,rec_energy,sim_energy\n");
            for (o, s) in replay.observed.iter().zip(&outcome.samples).filter(|(o, _)| (o.t as u32).is_multiple_of(10)) {
                let _ = writeln!(text, "{},{:.2},{:.2},{:.1},{:.1},{},{},{:.0},{:.0},{},{:.0},{:.0},{:.0},{:.0}", o.t, o.metal_income, s.metal_income, o.energy_income, s.energy_income, o.extractors, s.extractors, o.army_value, s.army_value, o.losses, o.metal, s.metal, o.energy, s.energy);
            }
            let file = format!("{dir}/calibration-{name}.csv");
            std::fs::write(&file, text).unwrap_or_else(|e| die(&format!("{file}: {e}")));
        }
    }
    println!("\n## What our bot achieved: median over {} records\n| min | mex alive | metal/s | energy/s | army metal built |\n|---|---|---|---|---|", records.len());
    for (minute, rows) in recorded.iter().enumerate().filter(|(_, rows)| !rows.is_empty()) {
        let median = |k: usize| { let mut v: Vec<f64> = rows.iter().map(|r| r[k]).collect(); v.sort_by(f64::total_cmp); (v[(v.len() - 1) / 2] + v[v.len() / 2]) / 2.0 };
        println!("| {minute} | {:.1} | {:.1} | {:.0} | {:.0} |", median(0), median(1), median(2), median(3));
    }
    for (which, title) in ["all records", "only records with at most 2 units lost by that minute"].iter().enumerate() {
    println!("\n## Summary, {title}: mean absolute error (and as a share of the recorded mean); metal/s bias = sim - rec");
    println!("| min | records | mex | metal/s | metal/s bias | army metal built |\n|---|---|---|---|---|---|");
    for (minute, e) in errors.iter().map(|e| e[which]).enumerate().skip(1).filter(|(_, e)| e[7] > 0.0) {
        let n = e[7];
        let share = |err: f64, total: f64| if total > 0.0 { 100.0 * err / total } else { 0.0 };
        println!("| {minute} | {n:.0} | {:.1} ({:.0} %) | {:.1} ({:.0} %) | {:+.1} | {:.0} ({:.0} %) |",
            e[0] / n, share(e[0], e[1]), e[2] / n, share(e[2], e[3]), e[6] / n, e[4] / n, share(e[4], e[5]));
    }
    }
}

/// Every mobile builder's way from one build to the next in the records' first minutes: the seconds it took against
/// what the simulator charges, over the map's own ground and over open ground, by how far the walk was.
fn walks(args: &Args) {
    let minutes: f64 = args.number("--minutes", 6.0);
    const BINS: [(&str, f64, f64); 5] = [("first build of the game", 0.0, 0.0), ("site in reach", 0.0, 0.0), ("walk under 300", 0.0, 300.0), ("walk 300-800", 300.0, 800.0), ("walk over 800", 800.0, f64::MAX)];
    // Per bin: (recorded, predicted on the map's ground, predicted on open ground, walked / straight).
    let mut rows: Vec<Vec<[f64; 4]>> = vec![Vec::new(); BINS.len()];
    for path in args.positional(&[]) {
        let replay = record::read(path, minutes * 60.0).unwrap_or_else(|e| die(&e));
        let game = &replay.game;
        let on_map = game.scenario(Vec::new(), game.ground());
        let open = game.scenario(Vec::new(), Arc::new(Straight { detour: 1.05 }));
        for trip in &replay.trips {
            let unit = &game.units.list[trip.builder];
            let beyond = buildorder::game::distance(trip.from, trip.to) - unit.build_distance - on_map.reach_bonus;
            // Where the builder stood is known only as "within reach of its last site": predict from the site itself.
            let predicted = |sc: &Scenario| sc.trip(trip.from, trip.to, unit.build_distance, unit.speed).0;
            let bin = if trip.first { 0 } else if beyond <= 0.0 { 1 } else { BINS.iter().position(|b| b.2 > 0.0 && beyond >= b.1 && beyond < b.2).unwrap() };
            let ratio = on_map.ground.walk(trip.from, trip.to) / buildorder::game::distance(trip.from, trip.to).max(1.0);
            rows[bin].push([trip.took, predicted(&on_map), predicted(&open), ratio]);
        }
    }
    println!("| way | trips | recorded s (median) | predicted, map's ground | predicted, open ground | median error map / open | walked over straight (median, max) |\n|---|---|---|---|---|---|---|");
    for ((label, _, _), trips) in BINS.iter().zip(&rows).filter(|(_, t)| !t.is_empty()) {
        let median = |pick: &dyn Fn(&[f64; 4]) -> f64| { let mut v: Vec<f64> = trips.iter().map(pick).collect(); v.sort_by(f64::total_cmp); v[v.len() / 2] };
        let longest = trips.iter().map(|t| t[3]).fold(0.0, f64::max);
        println!("| {label} | {} | {:.1} | {:.1} | {:.1} | {:+.1} / {:+.1} | {:.2}, {:.2} |", trips.len(), median(&|t| t[0]), median(&|t| t[1]), median(&|t| t[2]),
            median(&|t| t[1] - t[0]), median(&|t| t[2] - t[0]), median(&|t| t[3]), longest);
    }
}

fn main() {
    let args = Args(std::env::args().skip(1).collect());
    match args.0.first().map(String::as_str) {
        Some("optimize") => optimize(&args),
        Some("simulate") => run_plan(&args),
        Some("calibrate") => calibrate(&args),
        Some("walks") => walks(&args),
        _ => die("missing or unknown command"),
    }
}
