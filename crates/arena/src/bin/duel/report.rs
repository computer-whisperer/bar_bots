//! `duels.csv` (one row per duel) and the tables made from it.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use crate::director::DuelResult;

pub const HEADER: &str = "match,site,sequence,x,y,rep,x_end,x_team,n_x,n_y,metal_x,metal_y,winner,reason,seconds,\
contact_seconds,survivors_x,survivors_y,value_left_x,value_left_y,damage_taken_x,damage_taken_y";

pub fn row(r: &DuelResult) -> String {
    format!(
        "{},{},{},{},{},{},{},{},{},{},{:.0},{:.0},{},{},{:.1},{},{},{},{:.3},{:.3},{:.0},{:.0}",
        r.match_index, r.site, r.sequence, r.job.x, r.job.y, r.job.rep,
        if r.job.x_is_west() { "west" } else { "east" }, r.job.x_team(),
        r.count[0], r.count[1], r.metal[0], r.metal[1], r.winner, r.reason, r.seconds,
        r.contact_seconds.map_or(String::new(), |s| format!("{s:.1}")),
        r.survivors[0], r.survivors[1], r.value_left[0], r.value_left[1], r.damage_taken[0], r.damage_taken[1],
    )
}

/// One pairing seen from its first unit's side.
#[derive(Default, Clone)]
struct Tally {
    duels: u32,
    wins: u32,
    losses: u32,
    /// Sum over duels of (own value left - the other's value left).
    margin: f32,
    seconds: f32,
    own_count: u32,
    other_count: u32,
}

/// Reads raw duel rows (several files may be merged) and writes `pairs.csv`, `matrix.csv` and `matrix.md` to `out`.
pub fn write(inputs: &[&Path], out: &Path) -> io::Result<()> {
    let mut tallies: BTreeMap<(String, String), Tally> = BTreeMap::new();
    let mut units: Vec<String> = Vec::new();
    for input in inputs {
        let text = fs::read_to_string(input)?;
        let mut lines = text.lines();
        let header: Vec<&str> = lines.next().unwrap_or_default().split(',').collect();
        let column = |name: &str| header.iter().position(|h| *h == name).ok_or_else(|| io::Error::other(format!("{}: no column {name}", input.display())));
        let (x, y, n_x, n_y) = (column("x")?, column("y")?, column("n_x")?, column("n_y")?);
        let (winner, reason, seconds) = (column("winner")?, column("reason")?, column("seconds")?);
        let (left_x, left_y) = (column("value_left_x")?, column("value_left_y")?);
        for line in lines {
            let f: Vec<&str> = line.split(',').collect();
            if f.len() != header.len() || f[reason] == "spawn_failed" {
                continue;
            }
            let number = |i: usize| f[i].parse::<f32>().unwrap_or(0.0);
            let margin = number(left_x) - number(left_y);
            for name in [f[x], f[y]] {
                if !units.iter().any(|u| u == name) {
                    units.push(name.to_string());
                }
            }
            // Each duel counts once from either side; a unit against itself only once.
            let views = [(f[x], f[y], margin, "x", n_x, n_y), (f[y], f[x], -margin, "y", n_y, n_x)];
            for (own, other, margin, won_as, own_n, other_n) in views.into_iter().take(if f[x] == f[y] { 1 } else { 2 }) {
                let tally = tallies.entry((own.to_string(), other.to_string())).or_default();
                tally.duels += 1;
                tally.wins += u32::from(f[winner] == won_as);
                tally.losses += u32::from(f[winner] != won_as && f[winner] != "draw");
                tally.margin += margin;
                tally.seconds += number(seconds);
                tally.own_count = number(own_n) as u32;
                tally.other_count = number(other_n) as u32;
            }
        }
    }

    let mut pairs = String::from("unit,against,n_unit,n_against,duels,wins,losses,draws,mean_margin,mean_seconds\n");
    for ((own, other), t) in &tallies {
        let _ = writeln!(
            pairs, "{own},{other},{},{},{},{},{},{},{:.3},{:.1}",
            t.own_count, t.other_count, t.duels, t.wins, t.losses, t.duels - t.wins - t.losses,
            t.margin / t.duels as f32, t.seconds / t.duels as f32
        );
    }
    fs::write(out.join("pairs.csv"), pairs)?;

    // Rows fight columns: +100 means the row unit won untouched, -100 that it died without scratching the column unit.
    let cell = |own: &str, other: &str| {
        let own_view = tallies.get(&(own.to_string(), other.to_string()));
        own_view.map(|t| (100.0 * t.margin / t.duels as f32).round() as i32)
    };
    let mut csv = format!("unit,{}\n", units.join(","));
    let mut md = format!("| row vs column | {} | mean |\n|---|{}---|\n", units.join(" | "), "---|".repeat(units.len()));
    for own in &units {
        let cells: Vec<Option<i32>> = units.iter().map(|other| cell(own, other)).collect();
        let text: Vec<String> = cells.iter().map(|c| c.map_or(String::new(), |v| v.to_string())).collect();
        let known: Vec<i32> = cells.iter().flatten().copied().collect();
        let mean = known.iter().sum::<i32>() as f32 / known.len().max(1) as f32;
        let _ = writeln!(csv, "{own},{}", text.join(","));
        let _ = writeln!(md, "| **{own}** | {} | {mean:.0} |", text.join(" | "));
    }
    fs::write(out.join("matrix.csv"), csv)?;
    fs::write(out.join("matrix.md"), md)
}
