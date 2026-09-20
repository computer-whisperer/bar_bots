//! Ground truth: the engine duel tables of 2026-09-19, and the same fights set up for the simulator.
//!
//! The geometry is the harness's (`docs/harness/duels.md`): two armies of about 1200 metal each, front ranks 1100
//! elmos apart on a west-east axis, ranks of eight, flat ground, both sides attack-moving at the other's centre.
//! Replaying exactly that is the only way a difference between table and simulation means anything.

use crate::scenario::{Energy, Group, Scenario, Vec2};
use crate::sim::{Rules, odds};

/// Distance between the two front ranks, beyond every tier-1 weapon.
pub const APART: f32 = 1100.0;

/// One ordered pairing of a duel table: what was fought and how it went.
#[derive(Clone, Debug)]
pub struct Pair {
    pub unit: String,
    pub against: String,
    pub n_unit: u32,
    pub n_against: u32,
    pub duels: u32,
    /// Mean of `value_left(unit) - value_left(against)` over the duels, -1..1.
    pub mean_margin: f32,
    pub mean_seconds: f32,
}

/// Ranks 56 elmos apart, eight duels a pairing.
pub fn tight() -> Vec<Pair> {
    parse(include_str!("../../../docs/data/duels-2026-09-19/tight-pairs.csv"))
}

/// Ranks 100 elmos apart, four duels a pairing.
pub fn wide() -> Vec<Pair> {
    parse(include_str!("../../../docs/data/duels-2026-09-19/wide-pairs.csv"))
}

fn parse(csv: &str) -> Vec<Pair> {
    let mut lines = csv.lines();
    lines.next();
    lines
        .filter_map(|line| {
            let f: Vec<&str> = line.split(',').collect();
            if f.len() < 10 {
                return None;
            }
            Some(Pair {
                unit: f[0].to_string(),
                against: f[1].to_string(),
                n_unit: f[2].parse().ok()?,
                n_against: f[3].parse().ok()?,
                duels: f[4].parse().ok()?,
                mean_margin: f[8].parse().ok()?,
                mean_seconds: f[9].parse().ok()?,
            })
        })
        .collect()
}

/// The pairing set up as the harness set it up. Side 0 is `pair.unit`, at the west end.
pub fn scenario(rules: &Rules, pair: &Pair, spacing: f32, energy: Energy) -> Option<Scenario> {
    let (west, east) = (rules.units.index(&pair.unit)?, rules.units.index(&pair.against)?);
    let mut scenario = Scenario::new();
    scenario.energy = [energy; 2];
    let group = |def, count, x, facing| {
        let mut group = Group::new(def, count, Vec2::new(x, 0.0), facing);
        group.spacing = spacing;
        group
    };
    scenario.sides[0] = vec![group(west, pair.n_unit, 0.0, Vec2::new(1.0, 0.0))];
    scenario.sides[1] = vec![group(east, pair.n_against, APART, Vec2::new(-1.0, 0.0))];
    Some(scenario)
}

/// How well the simulation reproduces a table.
#[derive(Clone, Debug, Default)]
pub struct Agreement {
    pub pairings: usize,
    /// Pairings the table calls decisive (|margin| >= `DECISIVE`), and how many the simulation puts on the same side.
    pub decisive: usize,
    pub same_sign: usize,
    pub mean_absolute_error: f32,
    pub correlation: f32,
    /// Table margin against simulated margin, per pairing, worst first.
    pub misses: Vec<(String, String, f32, f32)>,
}

/// A margin this small is noise in the table too, so it is not counted as a sign to agree with.
pub const DECISIVE: f32 = 0.10;

pub fn validate(rules: &Rules, pairs: &[Pair], spacing: f32, reps: u32, energy: Energy) -> Agreement {
    let mut points: Vec<(f32, f32)> = Vec::new();
    let mut agreement = Agreement::default();
    for pair in pairs {
        let Some(scenario) = scenario(rules, pair, spacing, energy) else { continue };
        let got = odds(rules, &scenario, reps).mean_margin;
        points.push((pair.mean_margin, got));
        agreement.pairings += 1;
        if pair.mean_margin.abs() >= DECISIVE {
            agreement.decisive += 1;
            agreement.same_sign += usize::from(pair.mean_margin.signum() == got.signum());
        }
        agreement.misses.push((pair.unit.clone(), pair.against.clone(), pair.mean_margin, got));
    }
    let n = points.len().max(1) as f32;
    agreement.mean_absolute_error = points.iter().map(|(a, b)| (a - b).abs()).sum::<f32>() / n;
    agreement.correlation = correlation(&points);
    agreement.misses.sort_by(|a, b| (b.2 - b.3).abs().total_cmp(&(a.2 - a.3).abs()));
    agreement
}

fn correlation(points: &[(f32, f32)]) -> f32 {
    let n = points.len() as f32;
    if n < 2.0 {
        return 0.0;
    }
    let (mx, my) = (points.iter().map(|p| p.0).sum::<f32>() / n, points.iter().map(|p| p.1).sum::<f32>() / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (x, y) in points {
        sxy += (x - mx) * (y - my);
        sxx += (x - mx) * (x - mx);
        syy += (y - my) * (y - my);
    }
    if sxx <= 0.0 || syy <= 0.0 { 0.0 } else { sxy / (sxx * syy).sqrt() }
}
