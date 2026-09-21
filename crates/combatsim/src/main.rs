//! `combatsim` — ask the simulator a fight and read the answer, or replay the duel tables against it.

use std::collections::HashMap;

mod policies;

use combatsim::duels::{self, DECISIVE};
use combatsim::field::Field;
use combatsim::chase::Chase;
use combatsim::scenario::{Energy, Focus, Group, Intent, Micro, Odds, Scenario, Vec2};
use combatsim::sim::{Rules, Tuning, odds, simulate};
use combatsim::units::Units;

const USAGE: &str = "\
combatsim --a <type:count[@delay][,...]> --b <type:count[@delay][,...]>
          [--spacing 56] [--apart 1100] [--reps 1] [--seed 0] [--delay-a S] [--delay-b S]
          [--hold-a] [--hold-b] [--terrain FILE:W:H] [--at X,Z] [--from X,Z] [--no-collide] [--verbose]
          [--stored 500] [--income 30] (both sides; --stored-a/--income-b for one side only)
          [--micro-a POLICY] [--micro-b POLICY]
combatsim validate [--spacing 56|100|both] [--reps 4] [--worst 15] [--no-collide] [--stored E] [--income E]
combatsim micro [--reps 8] [--budget 1200] [--ratios 0.7,1,1.4] [--policies LIST] [--detail] [--spacing 56]
combatsim speed [--reps 200]
combatsim chase --pursuers <type:count,...> --party <type:count,...> [--distance 1500] [--assets armmex:3] [--defences armllt:2] [--dgun] [--intent raid|fight|flee]
          [--seconds 60] [--reps 8]    (the party stands among the assets, the pursuers start --distance away; a fleeing
          party runs directly away from them; printed beside the same contact with nobody sent)

Unit names are the game's internal ones (armham, corllt). Side A stands in the west, B in the east.
A side's types are laid out front to back in the order listed, so `--b corllt:12,corthud:20` is a tower line
with the army behind it, and `--a armham:10,armrock:6` is Rocketeers screened by Maces.
--terrain reads the bot's terrain-<ai>.bin (width and height in 16-elmo cells, from the record header);
--from and --at then place A and B on it instead of the default west-east line.

A micro policy is a comma-separated list of `none`, `spread=N`, `withdraw=F`, `kite`, `kite-slow`,
`focus=weakest|threat` and `no-chase`; `combatsim micro` prices each of them across the tier-1 matchups.";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flags = Flags::parse(&args);
    let tuning = Tuning {
        collide: !flags.has("no-collide"),
        dgun: flags.has("dgun"),
        spread: flags.num("spread", Tuning::default().spread),
        stop_at: flags.num("stop-at", Tuning::default().stop_at),
        aim_seconds: flags.num("aim", Tuning::default().aim_seconds),
    };
    let rules = Rules::new(Units::default(), tuning);
    match args.first().map(String::as_str) {
        Some("validate") => validate(&rules, &flags),
        Some("micro") => policies::sweep(&rules, &flags),
        Some("speed") => speed(&rules, &flags),
        Some("chase") => chase(&rules, &flags),
        Some("chase-file") => chase_file(&rules, &args[1], flags.num("reps", 4.0) as u32),
        Some("--help" | "-h") | None => println!("{USAGE}"),
        _ => query(&rules, &flags),
    }
}

/// `spread=120,withdraw=0.35` and so on; `none` (or nothing) is plain attack-move.
pub fn micro(spec: &str) -> Micro {
    let mut micro = Micro::default();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty() && *p != "none") {
        let (name, value) = part.split_once('=').unwrap_or((part, ""));
        match name {
            "spread" => micro.spread = value.parse().expect("spread=<elmos>"),
            "withdraw" => micro.withdraw_below = value.parse().expect("withdraw=<share of health>"),
            "kite" => micro.kite = true,
            "kite-slow" => (micro.kite, micro.kite_when_slower) = (true, true),
            "focus" => micro.focus = Focus::parse(value).expect("focus=nearest|weakest|threat"),
            "no-chase" => micro.no_chase = true,
            _ => panic!("unknown micro policy {name}"),
        }
    }
    micro
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

/// The groups of one side, laid out one behind the other: the first listed type stands at the front and each
/// later one falls in a formation-depth further back. Stacking them on the same spot instead would put a
/// screen inside the unit it is meant to screen, so a mixed force is written front-to-back, `armham:10,armrock:6`
/// being Maces with Rocketeers behind them.
fn force(rules: &Rules, spec: &str, front: Vec2, facing: Vec2, flags: &Flags, side: &str) -> Vec<Group> {
    let spacing = flags.num("spacing", 56.0);
    let back = Vec2::default().towards(facing) * -spacing;
    let mut depth = 0.0;
    let mut groups = Vec::new();
    for part in spec.split(',').filter(|part| !part.is_empty()) {
        // `armham:6@12` is six Maces turning up twelve seconds in.
        let (part, late) = part.split_once('@').unwrap_or((part, ""));
        let (name, count) = part.split_once(':').unwrap_or((part, "1"));
        let def = rules.units.index(name).unwrap_or_else(|| panic!("unknown unit {name}"));
        let count: u32 = count.parse().expect("count");
        let mut group = Group::new(def, count, front + back * depth, facing);
        group.spacing = spacing;
        group.delay = late.parse().unwrap_or_else(|_| flags.num(&format!("delay-{side}"), 0.0));
        group.hold = flags.has(&format!("hold-{side}"));
        depth += count.div_ceil(group.per_rank) as f32;
        groups.push(group);
    }
    groups
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
    scenario.energy = [energy(flags, "a"), energy(flags, "b")];
    scenario.micro = [micro(flags.get("micro-a").unwrap_or("")), micro(flags.get("micro-b").unwrap_or(""))];
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
    println!(
        "{} after {:.1}s ({:?}), first damage at {}",
        outcome.winner.map_or("draw".to_string(), |s| ["A", "B"][s].to_string()),
        outcome.seconds,
        outcome.reason,
        outcome.contact_seconds.map_or("never".to_string(), |s| format!("{s:.1}s"))
    );
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

/// One side's energy: `--stored`/`--income` for both, `--stored-a`/`--income-b` and so on to give the two sides
/// different economies, which is the usual case once a base is involved (a tower line is fed by a real grid).
fn energy(flags: &Flags, side: &str) -> Energy {
    let per_side = |name: &str, shared: f32| flags.num(&format!("{name}-{side}"), flags.num(name, shared));
    Energy { stored: per_side("stored", Energy::default().stored), income: per_side("income", Energy::default().income) }
}

fn validate(rules: &Rules, flags: &Flags) {
    let reps: u32 = flags.num("reps", 4);
    let worst: usize = flags.num("worst", 15);
    let wanted = flags.get("spacing").unwrap_or("both");
    let tables: Vec<(&str, f32, Vec<duels::Pair>)> = [("tight", 56.0, duels::tight()), ("wide", 100.0, duels::wide())]
        .into_iter()
        .filter(|(name, spacing, _)| wanted == "both" || wanted == *name || wanted.parse() == Ok(*spacing))
        .collect();
    for (name, spacing, pairs) in tables {
        let started = std::time::Instant::now();
        // Both duel teams have the same economy, so the shared `--stored`/`--income` is the whole story here.
        let agreement = duels::validate(rules, &pairs, spacing, reps, energy(flags, "a"));
        println!(
            "{name} (spacing {spacing:.0}, {} pairings, {reps} seeds each, {:.1}s):\n  \
             sign agreement {}/{} decisive (|margin| >= {DECISIVE:.2}) = {:.0}%, mean |error| {:.3}, correlation {:.3}, slope {:.2}",
            agreement.pairings,
            started.elapsed().as_secs_f32(),
            agreement.same_sign,
            agreement.decisive,
            agreement.same_sign as f32 / agreement.decisive.max(1) as f32 * 100.0,
            agreement.mean_absolute_error,
            agreement.correlation,
            agreement.slope,
        );
        println!("  worst misses (table -> simulated):");
        for (unit, against, table, got) in agreement.misses.iter().take(worst) {
            println!("    {unit:>9} vs {against:<9} {table:+.2} -> {got:+.2}");
        }
    }
}

/// Queries a second, by fight size, on this machine.
/// One chase a line, as `run/raid_episodes.py` writes them: `pursuers` [[name, count, x, z]], `party` [[name, count]],
/// `at` [x, z], `then` [x, z], `assets` [[name, x, z]], `seconds`. Prints one verdict a line; units the table lacks
/// are left out and counted in `unknown`.
fn chase_file(rules: &Rules, path: &str, reps: u32) {
    let text = std::fs::read_to_string(path).expect("episode file");
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let e: serde_json::Value = serde_json::from_str(line).expect("episode json");
        let mut unknown = 0;
        let point = |v: &serde_json::Value, i: usize| Vec2::new(v[i].as_f64().unwrap_or(0.0) as f32, v[i + 1].as_f64().unwrap_or(0.0) as f32);
        let mut def = |v: &serde_json::Value| { let d = rules.units.index(v[0].as_str().unwrap_or("")); unknown += usize::from(d.is_none()); d };
        let pursuers = e["pursuers"].as_array().unwrap().iter().filter_map(|v| Some((def(v)?, v[1].as_u64()? as u32, point(v, 2)))).collect();
        let party = e["party"].as_array().unwrap().iter().filter_map(|v| Some((def(v)?, v[1].as_u64()? as u32))).collect();
        let assets = e["assets"].as_array().unwrap().iter().filter_map(|v| Some((def(v)?, point(v, 1)))).collect();
        let chase = Chase { pursuers, party, at: point(&e["at"], 0), intent: Intent::Raid { then: point(&e["then"], 0) }, assets, party_buildings: Vec::new(), pursuer_buildings: Vec::new(), seconds: e["seconds"].as_f64().unwrap_or(60.0) as f32 };
        let v = chase.verdict(rules, reps);
        println!("{{\"caught\":{},\"caught_after\":{},\"party_killed\":{},\"pursuers_lost\":{},\"assets_lost\":{},\"survived\":{},\"unknown\":{unknown}}}", v.caught, v.caught_after, v.party_killed, v.pursuers_lost, v.assets_lost, v.survived);
    }
}

fn chase(rules: &Rules, flags: &Flags) {
    let pairs = |spec: String| -> Vec<(usize, u32)> {
        spec.split(',').filter(|p| !p.is_empty()).map(|part| {
            let (name, count) = part.split_once(':').unwrap_or((part, "1"));
            (rules.units.index(name).unwrap_or_else(|| panic!("unknown unit {name}")), count.parse().expect("count"))
        }).collect()
    };
    let distance: f32 = flags.num("distance", 1500.0);
    let at = Vec2::new(0.0, 0.0);
    let intent = match flags.get("intent").unwrap_or("raid").to_string().as_str() {
        "raid" => Intent::Raid { then: Vec2::new(4000.0, 0.0) },
        "fight" => Intent::Fight,
        "flee" => Intent::Flee(Vec2::new(4000.0, 0.0)),
        other => panic!("unknown intent {other}"),
    };
    // The buildings in a row beside the party, 250 apart: an outpost cluster.
    let assets: Vec<(usize, Vec2)> = pairs(flags.get("assets").unwrap_or("armmex:3").to_string()).into_iter()
        .flat_map(|(def, count)| (0..count).map(move |i| (def, Vec2::new(150.0, 250.0 * (i as f32 - (count - 1) as f32 / 2.0))))).collect();
    let from = Vec2::new(-distance, 0.0);
    // `--defences armllt:2`: the pursuers' turrets in a row 200 beyond the assets, holding (the raid question the
    // other way round: `--party` is then ours, raiding; `--dgun` arms their commander).
    let defences: Vec<(usize, Vec2)> = pairs(flags.get("defences").unwrap_or("").to_string()).into_iter()
        .flat_map(|(def, count)| (0..count).map(move |i| (def, Vec2::new(350.0, 300.0 * (i as f32 - (count - 1) as f32 / 2.0))))).collect();
    let sent = Chase { pursuers: pairs(flags.get("pursuers").unwrap_or("").to_string()).into_iter().map(|(d, n)| (d, n, from)).collect(), party: pairs(flags.get("party").unwrap_or("").to_string()), at, intent, assets, party_buildings: Vec::new(), pursuer_buildings: defences, seconds: flags.num("seconds", 60.0) };
    let reps = flags.num("reps", 8.0) as u32;
    let started = std::time::Instant::now();
    let with = sent.verdict(rules, reps);
    let each = started.elapsed().as_secs_f64() * 1000.0 / reps as f64;
    let without = Chase { pursuers: Vec::new(), ..sent.clone() }.verdict(rules, reps);
    for (label, v) in [("sent", &with), ("nobody sent", &without)] {
        println!("{label:12} caught in {:3.0} % of runs after {:4.1} s; party metal killed {:5.0}, pursuers' lost {:5.0}, buildings lost {:5.0}, survived {:3.0} %",
            v.caught * 100.0, v.caught_after, v.party_killed, v.pursuers_lost, v.assets_lost, v.survived * 100.0);
    }
    println!("sending them is worth {:+.0} metal ({each:.2} ms a run)", with.gain_over(&without));
}

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
