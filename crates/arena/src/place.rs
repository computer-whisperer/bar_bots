//! Where our commander spawns when the arena may choose (`--place`): the start the opening search likes best inside
//! our box, from the map as an earlier match on it recorded it (its record header carries the spots and the unit
//! numbers, its `terrain-*.bin` the ground). The opponent keeps the spawn it had in that match. Chosen once per box
//! and side and written into the start script (`StartPosType=3`), so the game begins where the search says; in a
//! game with people the lobby places us and the search plans from there (the user's ruling, 2026-09-20).

use std::path::Path;
use std::time::Duration;

use buildorder::anneal::{Contact, Objective, Palette, Search};
use buildorder::game::{distance, Spot};
use buildorder::start::{choose, scenario_from, Rect};

const BUDGET: Duration = Duration::from_secs(4);
const THREADS: usize = 4;
/// Ten minutes: at five with 90 s of terminal income the search took a start with six extractors at 5:00 over one
/// with eleven for a Pawn party more (commander games 6 and 7, `docs/design/2026-09-21-rolling-planner.md`).
const HORIZON: f64 = 600.0;

/// The latest record of a match on `map` that has its terrain beside it and whose opponent started inside `theirs`
/// (an earlier match may have had the corners the other way round), with that start.
fn latest_record(repo: &Path, map: &str, theirs: &dyn Fn((f64, f64), (f64, f64)) -> bool) -> Option<(String, (f64, f64))> {
    let mut candidates: Vec<(String, String)> = Vec::new();
    for batch in std::fs::read_dir(repo.join("run/matches")).ok()?.flatten() {
        for m in std::fs::read_dir(batch.path()).into_iter().flatten().flatten() {
            let Ok(entries) = std::fs::read_dir(m.path()) else { continue };
            for f in entries.flatten() {
                let name = f.file_name().to_string_lossy().to_string();
                if name.starts_with("record-") && name.ends_with(".jsonl") {
                    candidates.push((m.path().to_string_lossy().to_string(), name));
                }
            }
        }
    }
    candidates.sort();
    for (dir, name) in candidates.into_iter().rev() {
        let path = format!("{dir}/{name}");
        let Ok(file) = std::fs::File::open(&path) else { continue };
        let mut first = String::new();
        use std::io::BufRead;
        if std::io::BufReader::new(file).read_line(&mut first).is_err() {
            continue;
        }
        let Ok(header) = serde_json::from_str::<serde_json::Value>(&first) else { continue };
        if header["map"]["name"].as_str() != Some(map) || header["terrain"]["file"].as_str().is_none() {
            continue;
        }
        let truth = std::fs::read_dir(&dir).ok()?.flatten().map(|f| f.path()).find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("truth-")));
        let Some(truth) = truth else { continue };
        let Ok(text) = std::fs::read_to_string(&truth) else { continue };
        let Some(line) = text.lines().next() else { continue };
        let Ok(row) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let commander = row["enemy"].as_array().and_then(|units| units.iter().find(|u| u[1].as_str().is_some_and(|n| n.ends_with("com"))));
        let Some(c) = commander else { continue };
        let start = (c[2].as_f64().unwrap_or(0.0), c[3].as_f64().unwrap_or(0.0));
        let size = (header["map"]["width"].as_f64().unwrap_or(0.0), header["map"]["height"].as_f64().unwrap_or(0.0));
        if !theirs(start, size) {
            continue;
        }
        return Some((path, start));
    }
    None
}

/// Our start and the opponent's for a 1v1 in which our box is `ours` and theirs `theirs` (fractions of the map),
/// or none with a reason when the arena cannot choose.
pub fn choose_starts(repo: &Path, map: &str, ours: [f32; 4], theirs: [f32; 4], side: &str) -> Result<((f32, f32), (f32, f32)), String> {
    let inside = |start: (f64, f64), size: (f64, f64)| {
        Rect { left: theirs[0] as f64 * size.0, top: theirs[1] as f64 * size.1, right: theirs[2] as f64 * size.0, bottom: theirs[3] as f64 * size.1 }.contains(start)
    };
    let (record, enemy) = latest_record(repo, map, &inside).ok_or("no earlier match on this map with a record, its terrain, a truth file and the opponent in its box")?;
    let replay = buildorder::record::read(&record, 60.0)?;
    let mut game = replay.game;
    // The record's commander is its side's; ours may be the other faction.
    let commander = game.units.list.iter().position(|u| u.name == format!("{side}com")).ok_or(format!("no {side} commander in the record"))?;
    game.commander = commander;
    let rect = |r: [f32; 4]| Rect { left: r[0] as f64 * game.size.0, top: r[1] as f64 * game.size.1, right: r[2] as f64 * game.size.0, bottom: r[3] as f64 * game.size.1 };
    let (our_rect, their_rect) = (rect(ours), rect(theirs));
    let lab = game.factory("lab").ok_or("the commander builds no lab")?;
    let palette = Palette::new(&game.units, game.commander, lab, true, game.units.index(&format!("{side}llt")));
    let their_centre = their_rect.centre();
    let search = Search {
        objective: Objective::Tempo { army: 1.0, exposed: 0.3, contact: Some(Contact { at: 150.0, walk: distance(our_rect.centre(), their_centre), weight: 6.0, window: 240.0 }) },
        horizon: HORIZON, iterations: 0, seed: 1, factories: 1, constructors: 4, hot: 0.02, start: None,
    };
    let ground = game.ground();
    let scenario_for = |start: (f64, f64)| {
        // Ours to count on from this start: the spots nearer to it than to the opponent's box.
        let mut spots: Vec<Spot> = game.spots.iter().filter(|(at, _)| distance(*at, start) < distance(*at, their_centre)).map(|(at, amount)| Spot { at: *at, metal: game.spot_metal(*amount) }).collect();
        spots.sort_by(|a, b| distance(a.at, start).total_cmp(&distance(b.at, start)));
        let mut scenario = scenario_from(&game, start, spots, ground.clone());
        scenario.constructors_default_to_extractors = true;
        scenario.commander_leash = 1500.0;
        scenario
    };
    let (start, found) = choose(&game, our_rect, &palette, &search, BUDGET, THREADS, &scenario_for);
    eprintln!("placed: our commander at ({:.0}, {:.0}) in the {side} box (score {:.0}; from {record}); the opponent at ({:.0}, {:.0}) as before", start.0, start.1, found.score, enemy.0, enemy.1);
    Ok(((start.0 as f32, start.1 as f32), (enemy.0 as f32, enemy.1 as f32)))
}
