//! The turn reports: terse text, and after the first only what changed. A full game of identical JSON dumps
//! teaches a model to answer "no change" by rote; a report that is short when nothing happened keeps its attention
//! on what did. The front of the report describes the game (the score, the trade, the economy, the ground, the
//! opponent) and is the same for the commander and the player; the tail is what each one commands.

use super::shared::{Briefing, Field, Hands, Place, SquadStatus};

/// The hands' `did` lines a player's report carries at most.
const DONE_LINES: usize = 40;

/// What the last report showed, to say only what differs.
#[derive(Default)]
pub struct Seen {
    squads: Vec<String>,
    extractors: Vec<String>,
    turrets: usize,
    pool: String,
    production: String,
    spot_plan: String,
    pressure: String,
    scouting: String,
    hands: Vec<String>,
}

fn counted(items: &[(String, usize)]) -> String {
    if items.is_empty() {
        return "none".into();
    }
    items.iter().map(|(name, n)| format!("{name} {n}")).collect::<Vec<_>>().join(", ")
}

fn clock(seconds: i32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn squad_line(s: &SquadStatus) -> String {
    let at = s.centre.as_ref().map_or("nowhere".to_string(), |c| format!("{} ({}, {})", c.grid, c.x, c.z));
    let post = s.post.as_ref().map_or("no post".to_string(), |(p, r)| format!("post {} ({}, {}) r{r}", p.grid, p.x, p.z));
    let wanted = if s.still_wanted.is_empty() { String::new() } else { format!(", still wants {}", counted(&s.still_wanted)) };
    let engaged = if s.engaged { ", ENGAGED" } else { "" };
    let remark = s.remark.as_ref().map_or(String::new(), |r| format!(" ({r})"));
    format!("{} [{}] {}% at {at}, {post}{wanted}{engaged}{remark}", s.name, counted(&s.composition), s.health_percent)
}

/// The lines every report opens with, changed or not: what is not shown is not weighed.
fn front(briefing: &Briefing, field: &Field, fights: &[String]) -> Vec<String> {
    let mut lines = Vec::new();
    let c = &briefing.counts;
    let s = &field.score;
    lines.push(format!(
        "score: extractors {} (most held {}, no new high for {}; free spots we can walk to {}, nearest on foot: {}; the opponent is known to hold {}) | army {} soldiers worth {} metal, {} of them within 800 of our start | opponent: we see only what our units see. Its soldiers seen in the last 3 min and not seen to die: {} worth {} metal; its army is at least that and may be much more",
        s.extractors,
        s.extractor_peak,
        clock(s.seconds_since_growth),
        s.free_spots,
        if s.next_free.is_empty() { "none".to_string() } else { s.next_free.iter().map(|(n, p, walk, ground)| format!("#{n} {} {walk} {ground}", p.grid)).collect::<Vec<_>>().join(", ") },
        s.enemy_spots_seen,
        s.soldiers,
        s.army_metal,
        s.soldiers_near_home,
        s.enemy_soldiers_seen,
        s.enemy_soldiers_seen_metal
    ));
    lines.push(format!(
        "traded: in the last 3 min we lost {} metal of units and buildings and destroyed {} of theirs that we saw die; whole game {} lost, {} destroyed{}",
        s.traded_3_min.0, s.traded_3_min.1, s.traded.0, s.traded.1,
        s.seconds_since_turn.map_or(String::new(), |t| format!(" | {} of game time since your last turn began", clock(t)))
    ));
    lines.push(format!(
        "eco: metal {:.0} ({:+.1}/-{:.1}), energy {:.0}/{:.0} ({:+.0}) | extractors {} constructors {} labs {} turrets {} converters {}",
        briefing.metal.current, briefing.metal.income, briefing.metal.usage, briefing.energy.current, briefing.energy.storage,
        briefing.energy.income - briefing.energy.usage, c.extractors, c.constructors, c.labs, c.turrets, c.converters
    ));
    let g = &field.ground;
    let listed = |places: &[Place]| if places.is_empty() { "none".to_string() } else { places.iter().map(|p| p.grid.clone()).collect::<Vec<_>>().join(", ") };
    let posts = if g.posts.is_empty() { String::new() } else { format!(" | unclaimed soldiers stand at: {}", listed(&g.posts)) };
    lines.push(format!(
        "ground: free spots on held ground {}, contested {}, theirs {} | our extractors on ground we do not hold: {}{posts} | raided lately: {}",
        g.free_spots.0, g.free_spots.1, g.free_spots.2, listed(&g.extractors_exposed),
        if g.raided.is_empty() { "nowhere".to_string() } else { g.raided.iter().map(|(p, metal)| format!("{} ({metal})", p.grid)).collect::<Vec<_>>().join(", ") }
    ));
    if !field.wreck_fields.is_empty() || field.resurrection_bots > 0 {
        let fields: Vec<String> = field.wreck_fields.iter().take(5).map(|(at, metal, safe)| format!("{} {metal} metal{}", at.grid, if *safe { "" } else { " (not safe)" })).collect();
        lines.push(format!("wrecks: {} | resurrection bots {}", if fields.is_empty() { "none known".to_string() } else { fields.join(", ") }, field.resurrection_bots));
    }
    if briefing.seats.len() > 1 {
        let seats: Vec<String> = briefing
            .seats
            .iter()
            .map(|s| format!("team {} at {}: metal {:.0} ({:+.1}), {} extractors, {} soldiers", s.team, s.home.grid, s.metal_stored, s.metal_income, s.extractors, s.soldiers))
            .collect();
        lines.push(format!("seats: you command {} seats, each with its own economy | {}", seats.len(), seats.join(" | ")));
    }
    if s.enemy_bases.len() > 1 {
        let bases: Vec<String> = s
            .enemy_bases
            .iter()
            .map(|(team, at, found, dead)| {
                let who = team.map_or("an opponent".to_string(), |t| format!("team {t}"));
                let state = if *dead { "razed, no longer a target" } else if *found { "base FOUND" } else { "base unscouted, guessed" };
                format!("{who}: {state} at {} ({}, {})", at.grid, at.x, at.z)
            })
            .collect();
        lines.push(format!("opponents: {}", bases.join(" | ")));
    }
    let places = |list: &[Place]| list.iter().map(|p| format!("{} ({}, {})", p.grid, p.x, p.z)).collect::<Vec<_>>().join("; ");
    lines.push(format!(
        "to win: its commander {}; its factories seen: {}",
        s.enemy_commander.as_ref().map_or("has never been seen".to_string(), |(p, ago)| format!("was last seen at {} ({}, {}) {} ago", p.grid, p.x, p.z, clock(*ago))),
        if let (true, Some(base)) = (s.enemy_factories.is_empty(), &s.enemy_base_found) {
            format!("none standing now; its base is FOUND at {} ({}, {}), where we saw a factory (since destroyed or out of mind); its commander and whatever else it has are most likely still there", base.grid, base.x, base.z)
        } else if s.enemy_factories.is_empty() { format!("none, its base is unscouted (the game's guess is {} ({}, {}), often wrong by 500 or more)", briefing.presumed_enemy_start.grid, briefing.presumed_enemy_start.x, briefing.presumed_enemy_start.z) } else { places(&s.enemy_factories) }
    ));
    if let Some(elsewhere) = &s.guess_disproved {
        lines.push(format!(
            "THE GUESS IS WRONG: our soldiers stand at the guessed enemy start and there is no base in sight. A base is always beside metal; the nearest metal spots we have nobody near: {}. Send scouts or the army to look, nearest first",
            places(elsewhere)
        ));
    }
    if !s.raid_targets.is_empty() {
        let list: Vec<String> = s.raid_targets.iter().map(|(p, turrets)| format!("{} ({}, {}){}", p.grid, p.x, p.z, if *turrets > 0 { format!(" turrets {turrets}m") } else { " no turret seen".into() })).collect();
        lines.push(format!("to raid: its extractors seen outside its base, nearest first: {}", list.join("; ")));
    }
    if !s.trend.is_empty() {
        let then = |pick: &dyn Fn(&(i32, usize, f32, u32)) -> String| s.trend.iter().map(|t| format!("{} ({} min ago)", pick(t), t.0)).collect::<Vec<_>>().join(", ");
        lines.push(format!(
            "curves: extractors {} now, {}; metal income {:.0} now, {}; army metal {} now, {}; extractors lost in the last 3 min: {}",
            s.extractors,
            then(&|t| t.1.to_string()),
            s.metal_income,
            then(&|t| format!("{:.0}", t.2)),
            s.army_metal,
            then(&|t| t.3.to_string()),
            s.extractors_lost_3_min
        ));
    }
    if !fights.is_empty() {
        lines.push(format!("fights since last turn: {}", fights.join(", ")));
    }
    lines
}

/// Enemies in sight and our extractors with enemies near them.
fn contact(briefing: &Briefing, field: &Field) -> Vec<String> {
    let mut lines = Vec::new();
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
    lines
}

/// The commander's report: the front, then its squads, the pool, the extractors, the plan and the mix.
pub fn report(seen: &mut Seen, briefing: &Briefing, field: &Field, fights: &[String], full: bool) -> String {
    let mut lines = front(briefing, field, fights);
    if full || briefing.pressure != seen.pressure {
        lines.push(format!("pressure: {}", briefing.pressure));
    }
    seen.pressure = briefing.pressure.clone();
    if full || briefing.scouting != seen.scouting {
        lines.push(format!("scouted: {}", briefing.scouting));
    }
    seen.scouting = briefing.scouting.clone();
    lines.extend(contact(briefing, field));

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

    extractor_lines(seen, field, full, &mut lines);
    if !field.spot_plan.is_empty() && (full || field.spot_plan != seen.spot_plan) {
        lines.push(format!("expansion plan: {}", field.spot_plan));
    }
    seen.spot_plan = field.spot_plan.clone();
    if full || field.turrets.len() != seen.turrets {
        let turrets: Vec<String> = field.turrets.iter().map(|t| format!("{} ({}, {})", t.grid, t.x, t.z)).collect();
        lines.push(format!("turrets: {}; requests pending {}", if turrets.is_empty() { "none".into() } else { turrets.join("; ") }, field.turret_requests_pending));
    }
    seen.turrets = field.turrets.len();

    let production = if field.production_weights.is_empty() {
        "bot default (constructors while it wants them, then line units with one raider a batch)".to_string()
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

/// Our extractors, in full at first and then as gains and losses.
fn extractor_lines(seen: &mut Seen, field: &Field, full: bool, lines: &mut Vec<String>) {
    let extractors: Vec<String> = field
        .extractors
        .iter()
        .map(|x| format!("{}{} ({}, {}){}", x.spot.map_or(String::new(), |n| format!("#{n} ")), x.at.grid, x.at.x, x.at.z, if x.turret_within_300 { " T" } else { "" }))
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
}

/// The player's report: the front, then its hands: every actor as the picture has it (in full at first, then the
/// ones whose entry changed), Jev's judgements when they are high, and what the hands did since the last turn.
pub fn player_report(seen: &mut Seen, briefing: &Briefing, field: &Field, fights: &[String], hands: &Hands, full: bool) -> String {
    let mut lines = front(briefing, field, fights);
    lines.extend(contact(briefing, field));
    extractor_lines(seen, field, full, &mut lines);
    if full {
        let buildable: Vec<String> = field.buildable.iter().map(|(n, m)| format!("{n} {m}m")).collect();
        lines.push(format!("the lab can build: {}", buildable.join(", ")));
    }
    // Each actor on one line, as the hands see it: the words the instructions have to speak to.
    let mut actors: Vec<String> = Vec::new();
    if let Some(entries) = hands.picture["actors"].as_object() {
        for (name, entry) in entries {
            let field = |key: &str| entry[key].as_str().map(str::to_string);
            let mut parts: Vec<String> = Vec::new();
            if let Some(units) = field("units") {
                parts.push(units);
            }
            if let Some(at) = field("at") {
                parts.push(format!("at {at}"));
            }
            if let Some(doing) = field("doing") {
                parts.push(doing);
            }
            if let Some(health) = field("health").filter(|h| !h.starts_with("full")) {
                parts.push(format!("health {health}"));
            }
            for key in ["allowed", "lane", "progress", "footwork", "enemies_near", "enemies_at_our_extractors", "under_fire", "scouts_out"] {
                match &entry[key] {
                    serde_json::Value::String(text) => parts.push(format!("{key}: {text}")),
                    serde_json::Value::Array(items) => parts.push(format!("{key}: {}", items.iter().filter_map(|i| i.as_str()).collect::<Vec<_>>().join("; "))),
                    _ => {}
                }
            }
            actors.push(format!("{name}: {}", parts.join("; ")));
        }
    }
    let changed: Vec<&String> = actors.iter().filter(|a| full || !seen.hands.contains(a)).collect();
    if !changed.is_empty() {
        lines.push(format!("your hands' actors{}:", if full { "" } else { " (those whose entry changed)" }));
        lines.extend(changed.iter().map(|a| format!("  {a}")));
    }
    let name = |line: &String| line.split(':').next().unwrap_or_default().to_string();
    let gone: Vec<String> = seen.hands.iter().map(name).filter(|n| !actors.iter().any(|a| name(a) == *n)).collect();
    if !gone.is_empty() {
        lines.push(format!("actors gone (dead, merged or split): {}", gone.join(", ")));
    }
    seen.hands = actors;
    let high: Vec<String> = hands.globals.iter().filter(|(_, p)| **p >= 0.5).map(|(q, p)| format!("{q} {p:.2}")).collect();
    if !high.is_empty() {
        lines.push(format!("your hands judge (yes-probability): {}", high.join(", ")));
    }
    if !hands.done.is_empty() {
        let skipped = hands.done.len().saturating_sub(DONE_LINES);
        lines.push(format!("what your hands did since your last turn{}:", if skipped > 0 { format!(" (the last {DONE_LINES} of {})", hands.done.len()) } else { String::new() }));
        lines.extend(hands.done.iter().skip(skipped).map(|d| format!("  {d}")));
    } else if !full {
        lines.push("your hands played nothing new since your last turn (every actor carried on)".into());
    }
    lines.join("\n")
}
