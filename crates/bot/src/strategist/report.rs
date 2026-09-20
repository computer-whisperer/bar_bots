//! The commander's turn reports: terse text, and after the first only what changed. A full game of identical
//! JSON dumps teaches a model to answer "no change" by rote; a report that is short when nothing happened keeps
//! its attention on what did.

use super::shared::{Briefing, Field, SquadStatus};

/// What the last report showed, to say only what differs.
#[derive(Default)]
pub struct Seen {
    squads: Vec<String>,
    extractors: Vec<String>,
    turrets: usize,
    pool: String,
    production: String,
}

fn counted(items: &[(String, usize)]) -> String {
    if items.is_empty() {
        return "none".into();
    }
    items.iter().map(|(name, n)| format!("{name} {n}")).collect::<Vec<_>>().join(", ")
}

fn squad_line(s: &SquadStatus) -> String {
    let at = s.centre.as_ref().map_or("nowhere".to_string(), |c| format!("{} ({}, {})", c.grid, c.x, c.z));
    let post = s.post.as_ref().map_or("no post".to_string(), |(p, r)| format!("post {} ({}, {}) r{r}", p.grid, p.x, p.z));
    let wanted = if s.still_wanted.is_empty() { String::new() } else { format!(", still wants {}", counted(&s.still_wanted)) };
    let engaged = if s.engaged { ", ENGAGED" } else { "" };
    let remark = s.remark.as_ref().map_or(String::new(), |r| format!(" ({r})"));
    format!("{} [{}] {}% at {at}, {post}{wanted}{engaged}{remark}", s.name, counted(&s.composition), s.health_percent)
}

pub fn report(seen: &mut Seen, briefing: &Briefing, field: &Field, fights: &[String], full: bool) -> String {
    let mut lines = Vec::new();
    let c = &briefing.counts;
    lines.push(format!(
        "eco: metal {:.0} ({:+.1}/-{:.1}), energy {:.0}/{:.0} ({:+.0}) | extractors {} constructors {} labs {} turrets {} converters {}",
        briefing.metal.current, briefing.metal.income, briefing.metal.usage, briefing.energy.current, briefing.energy.storage,
        briefing.energy.income - briefing.energy.usage, c.extractors, c.constructors, c.labs, c.turrets, c.converters
    ));
    if !fights.is_empty() {
        lines.push(format!("fights since last turn: {}", fights.join(", ")));
    }
    for cluster in &briefing.enemies_visible {
        lines.push(format!(
            "enemy in sight: {} at {} ({}, {}), {} from home: {}",
            cluster.units, cluster.at.grid, cluster.at.x, cluster.at.z, cluster.distance_from_home, counted(&cluster.composition)
        ));
    }
    let threatened: Vec<String> = field
        .extractors
        .iter()
        .filter(|x| x.enemies_within_600 > 0)
        .map(|x| format!("{} ({} enemies{})", x.at.grid, x.enemies_within_600, if x.turret_within_300 { ", turret" } else { ", NO turret" }))
        .collect();
    if !threatened.is_empty() {
        lines.push(format!("extractors under threat: {}", threatened.join("; ")));
    }

    let pool = format!("{} around {}", counted(&field.unassigned), field.unassigned_centre.as_ref().map_or("-", |p| p.grid.as_str()));
    if full || pool != seen.pool {
        lines.push(format!(
            "unassigned soldiers (the bot's): {pool}; home group {} attackers {}",
            briefing.home_group.size, briefing.attackers.size
        ));
    }
    seen.pool = pool;

    let squads: Vec<String> = field.squads.iter().map(squad_line).collect();
    for line in &squads {
        if full || !seen.squads.contains(line) {
            lines.push(format!("squad {line}"));
        }
    }
    let names = |lines: &[String]| lines.iter().map(|l| l.split(' ').next().unwrap_or_default().to_string()).collect::<Vec<_>>();
    for gone in names(&seen.squads).iter().filter(|n| !names(&squads).contains(n)) {
        lines.push(format!("squad {gone} is gone (wiped out or released)"));
    }
    seen.squads = squads;

    let extractors: Vec<String> = field
        .extractors
        .iter()
        .map(|x| format!("{} ({}, {}){}", x.at.grid, x.at.x, x.at.z, if x.turret_within_300 { " T" } else { "" }))
        .collect();
    if full {
        lines.push(format!("our extractors (T = turret within 300): {}", extractors.join("; ")));
    } else {
        let gained: Vec<&String> = extractors.iter().filter(|x| !seen.extractors.contains(x)).collect();
        let lost: Vec<&String> = seen.extractors.iter().filter(|x| !extractors.contains(x)).collect();
        if !gained.is_empty() {
            lines.push(format!("extractors new or changed: {}", gained.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("; ")));
        }
        if !lost.is_empty() {
            lines.push(format!("extractors gone or changed: {}", lost.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("; ")));
        }
    }
    seen.extractors = extractors;
    if full || field.turrets.len() != seen.turrets {
        let turrets: Vec<String> = field.turrets.iter().map(|t| format!("{} ({}, {})", t.grid, t.x, t.z)).collect();
        lines.push(format!("turrets: {}; requests pending {}", if turrets.is_empty() { "none".into() } else { turrets.join("; ") }, field.turret_requests_pending));
    }
    seen.turrets = field.turrets.len();

    let production = if field.production_weights.is_empty() {
        "bot default (2 raiders, 1 constructor-or-artillery, 2 skirmishers)".to_string()
    } else {
        field.production_weights.iter().map(|(n, w)| format!("{n} {w}")).collect::<Vec<_>>().join(", ")
    };
    if full || production != seen.production {
        lines.push(format!("production mix: {production}"));
    }
    seen.production = production;
    if full {
        let buildable: Vec<String> = field.buildable.iter().map(|(n, m)| format!("{n} {m}m")).collect();
        lines.push(format!("factories can build: {}", buildable.join(", ")));
        if !briefing.directives_in_force.is_empty() {
            lines.push(format!("directives in force: {}", briefing.directives_in_force.join("; ")));
        }
    }
    lines.join("\n")
}
