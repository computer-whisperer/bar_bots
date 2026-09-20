//! `combatsim` — ask the simulator a fight and read the answer, or replay the duel tables against it.

use std::collections::HashMap;

use combatsim::duels::{self, DECISIVE};
use combatsim::field::Field;
use combatsim::scenario::{Energy, Group, Odds, Scenario, Vec2};
use combatsim::sim::{Rules, Tuning, odds, simulate};
use combatsim::units::Units;

const USAGE: &str = "\
combatsim --a <type:count[,...]> --b <type:count[,...]>
          [--spacing 56] [--apart 1100] [--reps 1] [--seed 0] [--delay-a S] [--delay-b S]
          [--hold-a] [--hold-b] [--terrain FILE:W:H] [--at X,Z] [--from X,Z] [--no-collide] [--verbose]
combatsim validate [--spacing 56|100|both] [--reps 4] [--worst 15] [--no-collide]
combatsim speed [--reps 200]

Unit names are the game's internal ones (armham, corllt). Side A stands in the west, B in the east.
--terrain reads the bot's terrain-<ai>.bin (width and height in 16-elmo cells, from the record header);
--from and --at then place A and B on it instead of the default west-east line.";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flags = Flags::parse(&args);
    let tuning = Tuning {
        collide: !flags.has("no-collide"),
        spread: flags.num("spread", Tuning::default().spread),
        stop_at: flags.num("stop-at", Tuning::default().stop_at),
        aim_seconds: flags.num("aim", Tuning::default().aim_seconds),
    };
    let rules = Rules::new(Units::default(), tuning);
    match args.first().map(String::as_str) {
        Some("validate") => validate(&rules, &flags),
        Some("speed") => speed(&rules, &flags),
        Some("--help" | "-h") | None => println!("{USAGE}"),
        _ => query(&rules, &flags),
    }
}

/// `--name value` and bare `--name` switches.
struct Flags(HashMap<String, String>);

impl Flags {
    fn parse(args: &[String]) -> Flags {
        let mut flags = HashMap::new();
        let mut i = 0;
        while i < args.len() {
            if let Some(name) = args[i].strip_prefix("--") {
                let value = args.get(i + 1).filter(|v| !v.starts_with("--"));
                flags.insert(name.to_string(), value.cloned().unwrap_or_default());
                i += 1 + usize::from(value.is_some());
            } else {
                i += 1;
            }
        }
        Flags(flags)
    }

    fn has(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str).filter(|v| !v.is_empty())
    }

    fn num<T: std::str::FromStr>(&self, name: &str, fallback: T) -> T {
        self.get(name).and_then(|v| v.parse().ok()).unwrap_or(fallback)
    }

    fn point(&self, name: &str) -> Option<Vec2> {
        let value = self.get(name)?;
        let (x, z) = value.split_once(',')?;
        Some(Vec2::new(x.trim().parse().ok()?, z.trim().parse().ok()?))
    }
}

fn force(rules: &Rules, spec: &str, front: Vec2, facing: Vec2, flags: &Flags, side: &str) -> Vec<Group> {
    spec.split(',')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (name, count) = part.split_once(':').unwrap_or((part, "1"));
            let def = rules.units.index(name).unwrap_or_else(|| panic!("unknown unit {name}"));
            let mut group = Group::new(def, count.parse().expect("count"), front, facing);
            group.spacing = flags.num("spacing", 56.0);
            group.delay = flags.num(&format!("delay-{side}"), 0.0);
            group.hold = flags.has(&format!("hold-{side}"));
            group
        })
        .collect()
}

fn query(rules: &Rules, flags: &Flags) {
    let (Some(a), Some(b)) = (flags.get("a"), flags.get("b")) else {
        println!("{USAGE}");
        return;
    };
    let apart: f32 = flags.num("apart", duels::APART);
    let (from, at) = (flags.point("from").unwrap_or(Vec2::new(0.0, 0.0)), flags.point("at"));
    let at = at.unwrap_or(Vec2::new(from.x + apart, from.z));
    let mut scenario = Scenario::new();
    scenario.sides[0] = force(rules, a, from, from.towards(at), flags, "a");
    scenario.sides[1] = force(rules, b, at, at.towards(from), flags, "b");
    if let Some(spec) = flags.get("terrain") {
        scenario.terrain = Some(terrain(spec));
    }
    let reps: u32 = flags.num("reps", 1);
    if reps > 1 {
        let odds = odds(rules, &scenario, reps);
        report_odds(&odds, reps);
        return;
    }
    let outcome = simulate(rules, &scenario, flags.num("seed", 0));
    let name = |def: usize| rules.units.names[def].clone();
    println!("{:?} after {:.1}s ({:?})", outcome.winner.map_or("draw".to_string(), |s| ["A", "B"][s].to_string()), outcome.seconds, outcome.reason);
    for (side, label) in ["A", "B"].iter().enumerate() {
        let left: Vec<String> = outcome.survivors[side].iter().map(|(def, n)| format!("{}x{n}", name(*def))).collect();
        println!(
            "  {label}: {:.0} metal lost, {:.0}% of its value left, survivors {}",
            outcome.metal_lost[side],
            outcome.value_left[side] * 100.0,
            if left.is_empty() { "none".to_string() } else { left.join(" ") }
        );
    }
    println!("  margin {:+.2} (A's share left minus B's)", outcome.margin);
    if flags.has("verbose") {
        let line: Vec<String> = outcome.timeline.iter().map(|v| format!("{:.2}/{:.2}", v[0], v[1])).collect();
        println!("  every 5 s: {}", line.join(" "));
    }
}

fn report_odds(odds: &Odds, reps: u32) {
    println!(
        "A wins {:.0}% of {reps} ({} / {} / {} draw), mean margin {:+.2}, mean {:.0}s",
        odds.win_probability() * 100.0,
        odds.wins,
        odds.losses,
        odds.draws,
        odds.mean_margin,
        odds.mean_seconds
    );
}

fn terrain(spec: &str) -> Field {
    let parts: Vec<&str> = spec.split(':').collect();
    let (path, width, height) = (parts[0], parts[1].parse().expect("width"), parts[2].parse().expect("height"));
    let bytes = std::fs::read(path).expect("terrain file");
    Field::from_bytes(&bytes, width, height, 16.0).expect("terrain file too short")
}

fn validate(rules: &Rules, flags: &Flags) {
    let reps: u32 = flags.num("reps", 4);
    let worst: usize = flags.num("worst", 15);
    let energy = Energy { stored: flags.num("stored", Energy::default().stored), income: flags.num("income", 30.0) };
    let wanted = flags.get("spacing").unwrap_or("both");
    let tables: Vec<(&str, f32, Vec<duels::Pair>)> = [("tight", 56.0, duels::tight()), ("wide", 100.0, duels::wide())]
        .into_iter()
        .filter(|(name, spacing, _)| wanted == "both" || wanted == *name || wanted.parse() == Ok(*spacing))
        .collect();
    for (name, spacing, pairs) in tables {
        let started = std::time::Instant::now();
        let agreement = duels::validate(rules, &pairs, spacing, reps, energy);
        println!(
            "{name} (spacing {spacing:.0}, {} pairings, {reps} seeds each, {:.1}s):\n  \
             sign agreement {}/{} decisive (|margin| >= {DECISIVE:.2}) = {:.0}%, mean |error| {:.3}, correlation {:.3}",
            agreement.pairings,
            started.elapsed().as_secs_f32(),
            agreement.same_sign,
            agreement.decisive,
            agreement.same_sign as f32 / agreement.decisive.max(1) as f32 * 100.0,
            agreement.mean_absolute_error,
            agreement.correlation,
        );
        println!("  worst misses (table -> simulated):");
        for (unit, against, table, got) in agreement.misses.iter().take(worst) {
            println!("    {unit:>9} vs {against:<9} {table:+.2} -> {got:+.2}");
        }
    }
}

/// Queries a second, by fight size, on this machine.
fn speed(rules: &Rules, flags: &Flags) {
    let reps: u32 = flags.num("reps", 200);
    let (ham, ak) = (rules.units.index("armham").unwrap(), rules.units.index("corak").unwrap());
    for count in [5, 20, 40] {
        let mut scenario = Scenario::new();
        scenario.sides[0] = vec![Group::new(ham, count, Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0))];
        scenario.sides[1] = vec![Group::new(ak, count, Vec2::new(duels::APART, 0.0), Vec2::new(-1.0, 0.0))];
        let started = std::time::Instant::now();
        let mut seconds = 0.0;
        for seed in 0..reps {
            seconds += simulate(rules, &scenario, seed as u64).seconds;
        }
        let elapsed = started.elapsed().as_secs_f64();
        println!(
            "{count} v {count}: {:.0} queries/s ({:.2} ms each, {:.0} game seconds a fight)",
            reps as f64 / elapsed,
            elapsed / reps as f64 * 1000.0,
            seconds / reps as f32
        );
    }
}
