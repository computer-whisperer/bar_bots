//! `combatsim micro`: what each unit-micro policy is worth, matchup by matchup.
//!
//! A cell is one of our tier-1 forces against one of the opponent's, at a metal ratio. Every policy is run on the
//! same seeds as the baseline (plain attack-move), so the difference is paired: the spread quoted is the spread of
//! that paired difference, not of the two fights separately. The two numbers reported are the duel harness's
//! margin (own share of value left less theirs) and the ledger's own measure, metal killed per metal lost.

use combatsim::duels;
use combatsim::scenario::{Group, Micro, Scenario, Vec2};
use combatsim::sim::{Rules, simulate};

use crate::Flags;

/// Our tier-1 bot lines (K-units-t1-bot-roster), each against the opponent's of the other faction: the arena
/// alternates faction by match index and the opponent always gets the other one, so we are Armada exactly when
/// BARb is Cortex.
const ARMADA_OURS: [&str; 5] = ["armpw", "armham", "armrock", "armwar", "armham:2+armrock:1"];
const CORTEX_OURS: [&str; 4] = ["corak", "corthud", "corstorm", "corthud:2+corstorm:1"];
/// What BARb medium fields from a tier-1 bot lab: raiders (armpw / corak are its `raider` role), the line unit,
/// the skirmisher, a mixed group of the first two, and the light tower it puts behind everything
/// (docs/knowledge/opponents-barb.md).
const CORTEX_THEIRS: [&str; 5] = ["corak", "corthud", "corstorm", "corak:1+corthud:1", "corllt"];
const ARMADA_THEIRS: [&str; 5] = ["armpw", "armham", "armrock", "armpw:1+armham:1", "armllt"];

/// The policies priced by default, baseline first.
const POLICIES: [&str; 8] = [
    "none",
    "spread=100",
    "spread=160",
    "withdraw=0.35",
    "kite",
    "kite-slow",
    "focus=weakest",
    "no-chase",
];

/// One side of a cell: unit types and the share of the metal budget each takes.
struct Force {
    spec: String,
    parts: Vec<(usize, f32)>,
}

impl Force {
    /// `armham:2+armrock:1` is two metal of Maces to one of Rocketeers; a bare name takes the whole budget.
    fn parse(rules: &Rules, spec: &str) -> Force {
        let parts = spec
            .split('+')
            .map(|part| {
                let (name, share) = part.split_once(':').unwrap_or((part, "1"));
                let def = rules.units.index(name).unwrap_or_else(|| panic!("unknown unit {name}"));
                (def, share.parse::<f32>().expect("share"))
            })
            .collect();
        Force { spec: spec.to_string(), parts }
    }

    /// The groups this force fields for `budget` metal, laid out front to back from `front` like the duel
    /// harness's ranks. Counts are whole units, so the metal actually fielded is reported beside them.
    fn groups(&self, rules: &Rules, budget: f32, front: Vec2, facing: Vec2, spacing: f32) -> (Vec<Group>, f32) {
        let total: f32 = self.parts.iter().map(|(_, share)| share).sum();
        let back = Vec2::default().towards(facing) * -spacing;
        let mut depth = 0.0;
        let mut groups = Vec::new();
        let mut metal = 0.0;
        for (def, share) in &self.parts {
            let unit = &rules.units.list[*def];
            let count = (budget * share / total / unit.metal).round().max(1.0) as u32;
            metal += count as f32 * unit.metal;
            let mut group = Group::new(*def, count, front + back * depth, facing);
            group.spacing = spacing;
            depth += count.div_ceil(group.per_rank) as f32;
            groups.push(group);
        }
        (groups, metal)
    }
}

/// One policy's result in one cell, against the baseline's on the same seeds.
#[derive(Clone, Copy, Default)]
struct Scored {
    margin: f32,
    /// Metal destroyed over all the cell's seeds, theirs and ours: the ledger's "army metal killed per metal
    /// lost" is the ratio of the two, and summing before dividing keeps one seed where we lost almost nothing
    /// from swamping the mean.
    killed: f32,
    lost: f32,
    /// Mean of the per-seed differences from the baseline, and its standard error.
    gain: f32,
    gain_error: f32,
}

impl Scored {
    fn efficiency(&self) -> f32 {
        self.killed / self.lost.max(1.0)
    }
}

fn mean(values: &[f32]) -> f32 {
    if values.is_empty() { 0.0 } else { values.iter().sum::<f32>() / values.len() as f32 }
}

/// Standard error of the mean.
fn spread(values: &[f32]) -> f32 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    let variance = values.iter().map(|v| (v - m) * (v - m)).sum::<f32>() / (values.len() - 1) as f32;
    (variance / values.len() as f32).sqrt()
}

pub fn sweep(rules: &Rules, flags: &Flags) {
    let reps: u32 = flags.num("reps", 8);
    let budget: f32 = flags.num("budget", 1200.0);
    let spacing: f32 = flags.num("spacing", 56.0);
    let ratios: Vec<f32> = flags
        .get("ratios")
        .unwrap_or("0.7,1,1.4")
        .split(',')
        .map(|r| r.trim().parse().expect("ratio"))
        .collect();
    let names: Vec<String> = match flags.get("policies") {
        Some(list) => list.split(';').map(str::to_string).collect(),
        None => POLICIES.iter().map(|p| p.to_string()).collect(),
    };
    let policies: Vec<Micro> = names.iter().map(|spec| crate::micro(spec)).collect();

    let mut cells: Vec<(String, String, f32, Vec<Scored>)> = Vec::new();
    let started = std::time::Instant::now();
    for (ours_list, theirs_list) in [(&ARMADA_OURS[..], &CORTEX_THEIRS[..]), (&CORTEX_OURS[..], &ARMADA_THEIRS[..])] {
        for ours in ours_list {
            let ours = Force::parse(rules, ours);
            for theirs in theirs_list {
                let theirs = Force::parse(rules, theirs);
                for &ratio in &ratios {
                    let scored = price(rules, &ours, &theirs, budget, ratio, spacing, &policies, reps);
                    cells.push((ours.spec.clone(), theirs.spec.clone(), ratio, scored));
                }
            }
        }
    }
    report(&names, &cells, reps, budget, &ratios, started.elapsed().as_secs_f32(), flags.has("detail"));
}

/// Every policy in one cell, on the same seeds, against the baseline (the first policy listed).
fn price(
    rules: &Rules,
    ours: &Force,
    theirs: &Force,
    budget: f32,
    ratio: f32,
    spacing: f32,
    policies: &[Micro],
    reps: u32,
) -> Vec<Scored> {
    let (from, at) = (Vec2::new(0.0, 0.0), Vec2::new(duels::APART, 0.0));
    let (our_groups, _) = ours.groups(rules, budget * ratio, from, from.towards(at), spacing);
    let (their_groups, _) = theirs.groups(rules, budget, at, at.towards(from), spacing);
    let mut base = Scenario::new();
    base.sides = [our_groups, their_groups];
    // Long enough for a fight that nobody is winning to be scored as it stands, short enough that a policy which
    // simply walks away does not cost four minutes of simulation per seed.
    base.time_limit = 180.0;

    let mut margins: Vec<Vec<f32>> = Vec::new();
    let mut traded: Vec<(f32, f32)> = Vec::new();
    for policy in policies {
        let mut scenario = base.clone();
        scenario.micro[0] = *policy;
        let mut margin = Vec::new();
        let mut trade = (0.0, 0.0);
        for seed in 0..reps {
            let outcome = simulate(rules, &scenario, seed as u64);
            margin.push(outcome.margin);
            trade = (trade.0 + outcome.metal_lost[1], trade.1 + outcome.metal_lost[0]);
        }
        margins.push(margin);
        traded.push(trade);
    }
    (0..policies.len())
        .map(|p| {
            let gains: Vec<f32> = margins[p].iter().zip(&margins[0]).map(|(a, b)| a - b).collect();
            Scored {
                margin: mean(&margins[p]),
                killed: traded[p].0,
                lost: traded[p].1,
                gain: mean(&gains),
                gain_error: spread(&gains),
            }
        })
        .collect()
}

fn report(
    names: &[String],
    cells: &[(String, String, f32, Vec<Scored>)],
    reps: u32,
    budget: f32,
    ratios: &[f32],
    seconds: f32,
    detail: bool,
) {
    println!(
        "{} cells ({} matchups x {} metal ratios), {reps} seeds each, {budget:.0} metal a side at ratio 1, {seconds:.1}s",
        cells.len(),
        cells.len() / ratios.len(),
        ratios.len()
    );
    println!("\nWhat each policy is worth overall (paired with the baseline, seed by seed):");
    println!("{:<16} {:>9} {:>9} {:>11} {:>9} {:>9}", "policy", "margin", "gain", "+- (cells)", "kill/loss", "better");
    for (p, name) in names.iter().enumerate() {
        let gains: Vec<f32> = cells.iter().map(|(_, _, _, s)| s[p].gain).collect();
        let margin = mean(&cells.iter().map(|(_, _, _, s)| s[p].margin).collect::<Vec<_>>());
        let killed: f32 = cells.iter().map(|(_, _, _, s)| s[p].killed).sum();
        let lost: f32 = cells.iter().map(|(_, _, _, s)| s[p].lost).sum();
        let efficiency = killed / lost.max(1.0);
        let better = gains.iter().filter(|g| **g > 0.02).count();
        println!(
            "{name:<16} {margin:>+9.3} {:>+9.3} {:>11} {efficiency:>9.2} {better:>4}/{}",
            mean(&gains),
            format!("{:.3}", spread(&gains)),
            gains.len()
        );
    }

    println!("\nGain in margin by what we are fighting (mean over our units and metal ratios):");
    let mut opponents: Vec<&str> = cells.iter().map(|(_, t, _, _)| t.as_str()).collect();
    opponents.sort();
    opponents.dedup();
    print!("{:<16}", "policy");
    for them in &opponents {
        print!("{them:>11}");
    }
    println!();
    for (p, name) in names.iter().enumerate().skip(1) {
        print!("{name:<16}");
        for them in &opponents {
            let gains: Vec<f32> = cells.iter().filter(|(_, t, _, _)| t == them).map(|(_, _, _, s)| s[p].gain).collect();
            print!("{:>+11.3}", mean(&gains));
        }
        println!();
    }

    println!("\nBy metal ratio (ours to theirs): gain in margin, and metal killed per metal lost:");
    print!("{:<16}", "policy");
    for ratio in ratios {
        print!("{:>20}", format!("{ratio:.2}x"));
    }
    println!();
    for (p, name) in names.iter().enumerate() {
        print!("{name:<16}");
        for ratio in ratios {
            let here = || cells.iter().filter(|(_, _, r, _)| (r - ratio).abs() < 1e-6);
            let gains: Vec<f32> = here().map(|(_, _, _, s)| s[p].gain).collect();
            let killed: f32 = here().map(|(_, _, _, s)| s[p].killed).sum();
            let lost: f32 = here().map(|(_, _, _, s)| s[p].lost).sum();
            print!("{:>13}{:>7.2}", format!("{:+.3}", mean(&gains)), killed / lost.max(1.0));
        }
        println!();
    }

    println!("\nGain in margin by our own unit:");
    let mut mine: Vec<&str> = cells.iter().map(|(o, _, _, _)| o.as_str()).collect();
    mine.sort();
    mine.dedup();
    print!("{:<16}", "policy");
    for ours in &mine {
        print!("{:>19}", ours);
    }
    println!();
    for (p, name) in names.iter().enumerate().skip(1) {
        print!("{name:<16}");
        for ours in &mine {
            let gains: Vec<f32> = cells.iter().filter(|(o, _, _, _)| o == ours).map(|(_, _, _, s)| s[p].gain).collect();
            print!("{:>19.3}", mean(&gains));
        }
        println!();
    }

    if !detail {
        println!("\n(--detail prints every cell)");
        return;
    }
    println!("\nEvery cell: margin (gain +- standard error) per policy");
    for (ours, theirs, ratio, scored) in cells {
        println!("  {ours} vs {theirs} at {ratio:.2}x metal");
        for (p, name) in names.iter().enumerate() {
            println!(
                "    {name:<16} margin {:>+6.3}  gain {:>+6.3} +- {:.3}  killed {:>6.0} lost {:>6.0} = {:.2}",
                scored[p].margin, scored[p].gain, scored[p].gain_error, scored[p].killed, scored[p].lost, scored[p].efficiency()
            );
        }
    }
}
