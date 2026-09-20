use std::fmt::Write as _;

use buildorder::anneal::{anneal_restarts, Objective, Palette, Search};
use buildorder::map;
use buildorder::plan::Plan;
use buildorder::record;
use buildorder::sim::{simulate, Outcome, Sample, Scenario, Wind};
use buildorder::units::Units;

const USAGE: &str = "usage:
  buildorder optimize  [--side arm|cor] [--factory lab|vp] [--start nw|se] [--objective income|army|mix] [--minutes 10]
                       [--iterations 40000] [--restarts 8] [--seed 1] [--wind 12.7] [--detour 1.05] [--factories 2] [--constructors 6]
                       [--no-nano] [--csv FILE] [--plan-out FILE]
  buildorder simulate  --plan FILE [--side ..] [--start ..] [--minutes ..] [--wind ..] [--detour ..] [--csv FILE]
  buildorder calibrate RECORD.jsonl... [--minutes 10] [--constant-wind] [--trace] [--csv-dir DIR]
  buildorder study     --out DIR [--iterations ..] [--restarts ..] [--seed ..]   (the grid behind docs/studies/build-order.md)";

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

fn scenario(side: &str, start: &str, wind: f64, detour: f64) -> Scenario {
    let (home, enemy) = match start {
        "nw" => (map::NW_HOME, map::SE_HOME),
        "se" => (map::SE_HOME, map::NW_HOME),
        other => die(&format!("unknown start {other}")),
    };
    let mut scenario = Scenario::new(side, home, map::own_half(home, enemy));
    scenario.wind = Wind::Constant(wind);
    scenario.detour = detour;
    scenario.constructors_default_to_extractors = true;
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
    let units = Units::load();
    let side = args.text("--side", "arm");
    let factory = args.text("--factory", "lab");
    let start = args.text("--start", "nw");
    let objective = Objective::parse(&args.text("--objective", "mix")).unwrap_or_else(|| die("unknown objective"));
    let minutes: f64 = args.number("--minutes", 10.0);
    let scenario = scenario(&side, &start, args.number("--wind", map::WIND_MEAN), args.number("--detour", 1.05));
    let palette = Palette::new(&units, &side, &factory, !args.has("--no-nano"));
    let search = Search {
        objective,
        horizon: minutes * 60.0,
        iterations: args.number("--iterations", 40_000),
        seed: args.number("--seed", 1),
        factories: args.number("--factories", 2),
        constructors: args.number("--constructors", 6),
        hot: args.number("--hot", 0.02),
    };
    let found = anneal_restarts(&units, &scenario, &palette, &search, args.number("--restarts", 8));
    println!("# {side} {factory} {start}, objective {} at {minutes} min, score {:.1}", objective.name(), found.score);
    print!("{}", found.plan.to_text(&units));
    print!("\n{MILESTONE_HEADER}{}", milestone_rows("best", &found.outcome));
    if let Some(path) = args.value("--csv") {
        write_csv(path, &found.outcome);
    }
    if let Some(path) = args.value("--plan-out") {
        std::fs::write(path, found.plan.to_text(&units)).unwrap_or_else(|e| die(&format!("{path}: {e}")));
    }
}

fn run_plan(args: &Args) {
    let units = Units::load();
    let side = args.text("--side", "arm");
    let path = args.value("--plan").unwrap_or_else(|| die("--plan FILE is required"));
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| die(&format!("{path}: {e}")));
    let plan = Plan::from_text(&text, &side, &units).unwrap_or_else(|e| die(&e));
    let scenario = scenario(&side, &args.text("--start", "nw"), args.number("--wind", map::WIND_MEAN), args.number("--detour", 1.05));
    let outcome = simulate(&units, &scenario, &plan, args.number("--minutes", 10.0) * 60.0);
    print!("{MILESTONE_HEADER}{}", milestone_rows(path, &outcome));
    for done in &outcome.finished {
        println!("{:6.1} s  {}  (queue {})", done.t, units.list[done.unit].name, done.queue);
    }
    if let Some(path) = args.value("--csv") {
        write_csv(path, &outcome);
    }
}

fn calibrate(args: &Args) {
    let units = Units::load();
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
        let replay = record::read(path, &units, minutes * 60.0).unwrap_or_else(|e| die(&e));
        let enemy = if map::distance(replay.home, map::NW_HOME) < map::distance(replay.home, map::SE_HOME) { map::SE_HOME } else { map::NW_HOME };
        let mut scenario = Scenario::new(&replay.side, replay.home, map::own_half(replay.home, enemy));
        if !args.has("--constant-wind") {
            let mut last = map::WIND_MEAN;
            scenario.wind = Wind::Trace(std::iter::once(last).chain(replay.wind.iter().map(|w| { last = w.unwrap_or(last); last })).collect());
        }
        let outcome = simulate(&units, &scenario, &replay.plan, minutes * 60.0);
        println!("\n## {path}\nside {}, home {:.0},{:.0}; builds outside the unit table: {:?}; started but never finished (left out): {}",
            replay.side, replay.home.0, replay.home.1, replay.unknown, replay.abandoned);
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
            print!("replayed plan:\n{}", replay.plan.to_text(&units));
            for done in &outcome.finished {
                println!("{:6.1} s  {}  (queue {})", done.t, units.list[done.unit].name, done.queue);
            }
        }
        if let Some(dir) = args.value("--csv-dir") {
            let name = path.trim_end_matches("/record-0.jsonl").rsplit('/').take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("-");
            let mut text = String::from("t,rec_metal_income,sim_metal_income,rec_energy_income,sim_energy_income,rec_extractors,sim_extractors,rec_army_value,sim_army_value,rec_losses\n");
            for (o, s) in replay.observed.iter().zip(&outcome.samples).filter(|(o, _)| (o.t as u32).is_multiple_of(10)) {
                let _ = writeln!(text, "{},{:.2},{:.2},{:.1},{:.1},{},{},{:.0},{:.0},{}", o.t, o.metal_income, s.metal_income, o.energy_income, s.energy_income, o.extractors, s.extractors, o.army_value, s.army_value, o.losses);
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

/// One optimisation of the study grid.
#[derive(Clone, Copy)]
struct Case<'a> {
    side: &'a str,
    factory: &'a str,
    start: &'a str,
    objective: Objective,
    minutes: f64,
    wind: f64,
    detour: f64,
    factories: usize,
    nanos: bool,
}

fn study(args: &Args) {
    let units = Units::load();
    let out = args.value("--out").unwrap_or_else(|| die("--out DIR is required"));
    std::fs::create_dir_all(out).unwrap_or_else(|e| die(&format!("{out}: {e}")));
    let iterations = args.number("--iterations", 40_000);
    let restarts = args.number("--restarts", 8);
    let seed = args.number("--seed", 1);
    let mut report = format!("Generated by `buildorder study --iterations {iterations} --restarts {restarts} --seed {seed}`. Do not edit.\n");
    let mut summary = format!("run,{CSV_HEADER}\n");
    let base = Case { side: "arm", factory: "lab", start: "nw", objective: Objective::Mix, minutes: 10.0, wind: map::WIND_MEAN, detour: 1.05, factories: 2, nanos: true };
    let mut run = |case: &Case| {
        let scenario = scenario(case.side, case.start, case.wind, case.detour);
        let palette = Palette::new(&units, case.side, case.factory, case.nanos);
        let search = Search { objective: case.objective, horizon: case.minutes * 60.0, iterations, seed, factories: case.factories, constructors: 6, hot: 0.02 };
        let found = anneal_restarts(&units, &scenario, &palette, &search, restarts);
        let label = format!(
            "{}-{}-{}-{}-{:.0}min-wind{:.0}-detour{:.2}-{}fac-{}",
            case.side, case.factory, case.start, case.objective.name(), case.minutes, case.wind, case.detour, case.factories,
            if case.nanos { "nano" } else { "nonano" }
        );
        write_csv(&format!("{out}/{label}.csv"), &found.outcome);
        let _ = write!(report, "\n### {label} (score {:.1})\n```\n{}```\n{MILESTONE_HEADER}{}", found.score, found.plan.to_text(&units), milestone_rows("", &found.outcome));
        for minute in MILESTONES {
            if let Some(s) = found.outcome.samples.iter().find(|s| s.t == minute * 60.0) {
                let _ = writeln!(summary, "{label},{}", csv_row(s));
            }
        }
        eprintln!("done {label}");
        found
    };
    // The main line: Armada bot lab from the north-west, every objective and horizon.
    let mut reference = None;
    for objective in [Objective::Income, Objective::Army, Objective::Mix] {
        for minutes in MILESTONES {
            let found = run(&Case { objective, minutes, ..base });
            if objective == Objective::Mix && minutes == 10.0 {
                reference = Some(found);
            }
        }
    }
    // The other start, factions and factory.
    run(&Case { start: "se", ..base });
    for (side, factory) in [("arm", "vp"), ("cor", "lab"), ("cor", "vp")] {
        run(&Case { side, factory, ..base });
    }
    // Build power: one lab or two, with and without construction turrets (the fourth combination is the main line).
    for objective in [Objective::Mix, Objective::Army] {
        for (factories, nanos) in [(1, false), (1, true), (2, false)] {
            run(&Case { objective, factories, nanos, ..base });
        }
    }
    // Harsher worlds: poor wind, and every walk 40 % longer than the straight line.
    run(&Case { wind: 8.0, ..base });
    run(&Case { detour: 1.4, ..base });
    // And the main line's best plan replayed unchanged in those worlds: how brittle is an order tuned to the mean?
    let reference = reference.expect("the main line ran");
    let _ = write!(report, "\n### The arm-lab-nw mix 10 min plan replayed unchanged in harsher worlds\n{MILESTONE_HEADER}");
    for (label, wind, detour) in [("as optimised", map::WIND_MEAN, 1.05), ("wind 8", 8.0, 1.05), ("wind 5", 5.0, 1.05), ("detour 1.4", map::WIND_MEAN, 1.4), ("wind 8, detour 1.4", 8.0, 1.4)] {
        let outcome = simulate(&units, &scenario("arm", "nw", wind, detour), &reference.plan, 600.0);
        report.push_str(&milestone_rows(label, &outcome));
    }
    std::fs::write(format!("{out}/optima.md"), report).unwrap_or_else(|e| die(&format!("{out}/optima.md: {e}")));
    std::fs::write(format!("{out}/summary.csv"), summary).unwrap_or_else(|e| die(&format!("{out}/summary.csv: {e}")));
    println!("wrote {out}/optima.md, {out}/summary.csv and one curve CSV per run");
}

fn main() {
    let args = Args(std::env::args().skip(1).collect());
    match args.0.first().map(String::as_str) {
        Some("optimize") => optimize(&args),
        Some("simulate") => run_plan(&args),
        Some("calibrate") => calibrate(&args),
        Some("study") => study(&args),
        _ => die("missing or unknown command"),
    }
}
