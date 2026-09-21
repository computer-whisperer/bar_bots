//! Reads a match record (`docs/harness/record-format.md`) and turns the opening our bot played into a `Plan` with the
//! sites it used, plus the curves it actually achieved, so the simulator can be checked against a real game.

use std::collections::HashMap;

use bot_protocol::{Converter, MoveClass, MoveKind, Terrain, UnitDefId, UnitDefInfo};
use serde_json::Value;

use crate::game::{distance, Game};
use crate::plan::{Item, Plan, Step};
use crate::sim::{Job, Scenario, Standing, State, StateBuilder};
use crate::units::{Role, Units};

/// What the record says about one game second.
#[derive(Clone, Debug, Default)]
pub struct Observed {
    pub t: f64,
    pub metal: f64,
    pub energy: f64,
    pub metal_storage: f64,
    pub energy_storage: f64,
    pub metal_income: f64,
    pub energy_income: f64,
    pub extractors: u32,
    /// Extractors finished so far, dead or alive.
    pub extractors_built: u32,
    pub constructors: u32,
    /// Combat units finished so far, by metal cost, dead or alive (what the simulator counts).
    pub army_value: f64,
    /// Own units lost so far, buildings included.
    pub losses: u32,
}

/// One walk of a mobile builder between two builds, as the record shows it.
#[derive(Clone, Copy, Debug)]
pub struct Trip {
    /// The builder's unit type.
    pub builder: usize,
    /// Where it stood when it became free (the site of its previous build, or where it was made) and where it built next.
    pub from: (f64, f64),
    pub to: (f64, f64),
    /// Seconds from becoming free to the next build's first frame.
    pub took: f64,
    /// The game's first build: the engine drops orders given in the opening seconds.
    pub first: bool,
}

/// One build the record shows started and finished: who started it, when, what and where.
#[derive(Clone, Copy, Debug)]
pub struct Started {
    pub at: f64,
    pub builder: u64,
    pub unit: usize,
    pub site: (f64, f64),
}

/// One own unit in a sample: id, type, place, health as a share of its maximum, whether it is still being built.
#[derive(Clone, Copy, Debug)]
pub struct Seen {
    pub id: u64,
    pub unit: usize,
    pub at: (f64, f64),
    pub health: f64,
    pub being_built: bool,
}

pub struct Replay {
    pub game: Game,
    pub trips: Vec<Trip>,
    pub plan: Plan,
    /// Every finished build in the window, in the order started (the plan's steps with their times and builders).
    pub steps: Vec<Started>,
    /// Who built each unit (unit id to builder id), for everything the record shows created by a builder.
    pub built_by: HashMap<u64, u64>,
    /// When each unit finished (unit id to seconds).
    pub finished_at: HashMap<u64, f64>,
    /// Each unit's type (unit id to index in the unit table), for everything created in the window.
    pub types: HashMap<u64, usize>,
    /// Our units as each whole second's sample lists them.
    pub seen: Vec<(f64, Vec<Seen>)>,
    pub observed: Vec<Observed>,
    /// Wind per turbine, per second, inferred from energy income; `None` where no turbine stood.
    pub wind: Vec<Option<f64>>,
    /// Builds the record shows that the unit table does not cover, by name.
    pub unknown: Vec<String>,
    /// Builds that were started and never finished (killed or abandoned); left out of the plan.
    pub abandoned: usize,
    pub first_factory_finished: Option<f64>,
}

fn pair(v: &Value) -> (f64, f64) {
    (v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0))
}

/// The unit table of a record's header. Records written before 2026-09-20 lack the numbers.
fn unit_table(header: &Value) -> Result<Units, String> {
    let defs = header["unit_defs"].as_array().ok_or("no unit_defs")?;
    if defs.first().is_none_or(|d| d["build_time"].is_null()) {
        return Err("this record's header lacks the units' build numbers (written before 2026-09-20): play the game again".to_string());
    }
    let number = |d: &Value, key: &str| d[key].as_f64().unwrap_or(0.0) as f32;
    let kind = |name: &str| match name {
        "tank" => Some(MoveKind::Tank),
        "bot" => Some(MoveKind::Bot),
        "hover" => Some(MoveKind::Hover),
        "ship" => Some(MoveKind::Ship),
        _ => None,
    };
    let defs: Vec<UnitDefInfo> = defs
        .iter()
        .map(|d| UnitDefInfo {
            id: UnitDefId(d["id"].as_i64().unwrap_or(-1) as i32),
            name: d["name"].as_str().unwrap_or("").to_string(),
            metal_cost: number(d, "metal"),
            energy_cost: number(d, "energy"),
            speed: number(d, "speed"),
            build_speed: number(d, "build_speed"),
            build_time: number(d, "build_time"),
            build_distance: number(d, "build_distance"),
            extracts_metal: number(d, "extracts_metal"),
            metal_make: number(d, "metal_make"),
            energy_make: number(d, "energy_make"),
            energy_upkeep: number(d, "energy_upkeep"),
            wind_cap: number(d, "wind_cap"),
            metal_storage: number(d, "metal_storage"),
            energy_storage: number(d, "energy_storage"),
            radar_range: number(d, "radar_range"),
            converter: d["converter"].as_array().map(|c| Converter { capacity: c[0].as_f64().unwrap_or(0.0) as f32, efficiency: c[1].as_f64().unwrap_or(0.0) as f32 }),
            weapon_count: d["weapons"].as_i64().unwrap_or(0) as i32,
            build_options: d["builds"].as_array().map(|b| b.iter().filter_map(|id| id.as_i64().map(|id| UnitDefId(id as i32))).collect()).unwrap_or_default(),
            move_class: d["move"].as_array().and_then(|m| {
                // `slope_mod` is in records from 2026-09-20 night on; older ones walk flat ground at full speed everywhere.
                Some(MoveClass { kind: kind(m[0].as_str()?)?, max_slope: m[1].as_f64()? as f32, depth: m[2].as_f64()? as f32, slope_mod: m.get(3).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32 })
            }),
        })
        .collect();
    Ok(Units::new(&defs))
}

/// The terrain grid in the file beside the record (`docs/harness/record-format.md`); empty when it is missing.
fn terrain(path: &str, header: &Value) -> Terrain {
    let t = &header["terrain"];
    let (width, height) = (t["width"].as_u64().unwrap_or(0) as u32, t["height"].as_u64().unwrap_or(0) as u32);
    let cells = (width * height) as usize;
    let file = std::path::Path::new(path).with_file_name(t["file"].as_str().unwrap_or(""));
    match std::fs::read(file) {
        Ok(bytes) if cells > 0 && bytes.len() == cells * 3 => Terrain {
            cell: t["cell"].as_f64().unwrap_or(0.0) as f32,
            width,
            height,
            heights: bytes[..cells * 2].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect(),
            slopes: bytes[cells * 2..].to_vec(),
        },
        _ => Terrain::default(),
    }
}

pub fn read(path: &str, seconds: f64) -> Result<Replay, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    // A killed match may end in a partial line; skip whatever does not parse.
    let mut lines = text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok());
    let header = lines.next().ok_or("empty record")?;
    let names: Vec<String> = header["unit_defs"].as_array().ok_or("no unit_defs")?.iter().map(|d| d["name"].as_str().unwrap_or("").to_string()).collect();
    let table = unit_table(&header)?;
    let units = &table;
    let spots: Vec<((f64, f64), f64)> =
        header["metal_spots"].as_array().ok_or("no metal_spots")?.iter().map(|s| (pair(s), s[2].as_f64().unwrap_or(0.0))).collect();
    let map = &header["map"];
    let number = |v: &Value| v.as_f64().unwrap_or(0.0);
    let records: Vec<Value> = lines.take_while(|r| r["f"].as_f64().unwrap_or(0.0) <= seconds * 30.0).collect();

    let finished_ids: HashMap<u64, f64> = records
        .iter()
        .filter(|r| r["t"] == "ev" && r["k"] == "finished")
        .map(|r| (r["u"].as_u64().unwrap_or(0), r["f"].as_f64().unwrap_or(0.0) / 30.0))
        .collect();
    let destroyed_ids: HashMap<u64, f64> = records
        .iter()
        .filter(|r| r["t"] == "ev" && r["k"] == "destroyed")
        .map(|r| (r["u"].as_u64().unwrap_or(0), r["f"].as_f64().unwrap_or(0.0) / 30.0))
        .collect();

    // Queues: who started what, in order. Builders are numbered in the order they finished, like the simulator does.
    let mut plan = Plan::empty(0, 0);
    enum Slot {
        Commander,
        Factory(usize),
        Constructor(usize),
    }
    let mut slot_of: HashMap<u64, Slot> = HashMap::new(); // builder unit id -> its queue
    let mut pending: HashMap<u64, Vec<Step>> = HashMap::new();
    // Mobile builders between builds: unit id -> (its type, where and when it became free); and who builds what.
    let mut free: HashMap<u64, (usize, (f64, f64), f64)> = HashMap::new();
    let mut built_by: HashMap<u64, u64> = HashMap::new();
    let mut every_builder: HashMap<u64, u64> = HashMap::new();
    let mut types: HashMap<u64, usize> = HashMap::new();
    let mut steps: Vec<Started> = Vec::new();
    let mut trips = Vec::new();
    let (mut home, mut commander, mut unknown, mut abandoned, mut first_factory_finished) = (None, None, Vec::new(), 0, None);
    for r in records.iter().filter(|r| r["t"] == "ev") {
        let id = r["u"].as_u64().unwrap_or(0);
        let name = names.get(r["d"].as_i64().unwrap_or(-1) as usize).cloned().unwrap_or_default();
        let site = (r["x"].as_f64().unwrap_or(0.0), r["z"].as_f64().unwrap_or(0.0));
        match r["k"].as_str() {
            Some("created") => {
                if let Some(unit) = units.index(&name) {
                    types.insert(id, unit);
                }
                let Some(by) = r["by"].as_u64() else {
                    home.get_or_insert(site);
                    commander = commander.or(units.index(&name));
                    continue;
                };
                every_builder.insert(id, by);
                if let Some((builder, from, since)) = free.remove(&by) {
                    trips.push(Trip { builder, from, to: site, took: r["f"].as_f64().unwrap_or(0.0) / 30.0 - since, first: trips.is_empty() });
                    built_by.insert(id, by);
                }
                let still_building = !finished_ids.contains_key(&id) && !destroyed_ids.contains_key(&id);
                if !finished_ids.contains_key(&id) && !still_building {
                    abandoned += 1;
                    continue;
                }
                match units.index(&name) {
                    Some(unit) => {
                        pending.entry(by).or_default().push(Step { item: Item::Build(unit), site: Some(site) });
                        steps.push(Started { at: r["f"].as_f64().unwrap_or(0.0) / 30.0, builder: by, unit, site });
                    }
                    None => unknown.push(name),
                }
            }
            Some("finished") => {
                let now = r["f"].as_f64().unwrap_or(0.0) / 30.0;
                // Whoever built this is free again, standing at it; a new commander or constructor is free where it is.
                if let Some(builder) = built_by.remove(&id) {
                    if let Some(unit) = records.iter().find(|c| c["t"] == "ev" && c["k"] == "created" && c["u"].as_u64() == Some(builder)).and_then(|c| units.index(names.get(c["d"].as_i64().unwrap_or(-1) as usize)?)) {
                        free.insert(builder, (unit, site, now));
                    }
                }
                if let Some(unit) = units.index(&name).filter(|u| matches!(units.list[*u].role, Role::Commander | Role::Builder)) {
                    free.insert(id, (unit, site, if units.list[unit].role == Role::Commander { 0.0 } else { now }));
                }
                match units.index(&name).map(|u| units.list[u].role) {
                Some(Role::Commander) => {
                    slot_of.insert(id, Slot::Commander);
                }
                Some(Role::Factory) => {
                    first_factory_finished.get_or_insert(r["f"].as_f64().unwrap_or(0.0) / 30.0);
                    slot_of.insert(id, Slot::Factory(plan.factories.len()));
                    plan.factories.push(Vec::new());
                }
                Some(Role::Builder) => {
                    slot_of.insert(id, Slot::Constructor(plan.constructors.len()));
                    plan.constructors.push(Vec::new());
                }
                _ => {}
                }
            }
            _ => {}
        }
    }
    for (builder, steps) in pending {
        match slot_of.get(&builder) {
            Some(Slot::Commander) => plan.commander = steps,
            Some(Slot::Factory(nth)) => plan.factories[*nth] = steps,
            Some(Slot::Constructor(nth)) => plan.constructors[*nth] = steps,
            None => {} // built by something that never finished inside the window
        }
    }

    // Curves. Counts come from the events, the economy from the samples.
    let cost = |name: &str| units.index(name).map(|u| &units.list[u]);
    let mut alive: HashMap<u64, String> = HashMap::new();
    let (mut army_value, mut losses, mut extractors_built) = (0.0, 0u32, 0u32);
    let (mut observed, mut wind, mut seen) = (Vec::new(), Vec::new(), Vec::new());
    for r in &records {
        let id = r["u"].as_u64().unwrap_or(0);
        match (r["t"].as_str(), r["k"].as_str()) {
            (Some("ev"), Some("finished")) => {
                let name = names.get(r["d"].as_i64().unwrap_or(-1) as usize).cloned().unwrap_or_default();
                if let Some(def) = cost(&name).filter(|d| d.role == Role::Army) {
                    army_value += def.metal_cost;
                }
                if cost(&name).is_some_and(|d| d.extracts_metal > 0.0) {
                    extractors_built += 1;
                }
                alive.insert(id, name);
            }
            (Some("ev"), Some("destroyed")) => {
                alive.remove(&id);
                losses += 1;
            }
            (Some("s"), _) => {
                let frame = r["f"].as_f64().unwrap_or(0.0);
                if frame == 0.0 || frame % 30.0 != 0.0 {
                    continue;
                }
                let defs: Vec<_> = alive.values().filter_map(|n| cost(n)).collect();
                let turbines = defs.iter().filter(|d| d.wind_cap > 0.0).count();
                // Upkeep is usage, not negative income, in the engine's books; generators' constant output is income.
                let steady: f64 = defs.iter().map(|d| d.energy_make.max(0.0)).sum();
                let energy_income = r["e"][1].as_f64().unwrap_or(0.0);
                wind.push((turbines > 0).then(|| ((energy_income - steady) / turbines as f64).clamp(0.0, 25.0)));
                let listed: Vec<Seen> = r["own"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|u| {
                        let unit = units.index(names.get(u[1].as_i64()? as usize)?)?;
                        let flags = u[5].as_i64().unwrap_or(0);
                        Some(Seen { id: u[0].as_u64()?, unit, at: (u[2].as_f64()?, u[3].as_f64()?), health: u[4].as_f64().unwrap_or(100.0) / 100.0, being_built: flags & 1 != 0 })
                    })
                    .collect();
                seen.push((frame / 30.0, listed));
                observed.push(Observed {
                    t: frame / 30.0,
                    metal: r["m"][0].as_f64().unwrap_or(0.0),
                    energy: r["e"][0].as_f64().unwrap_or(0.0),
                    metal_storage: r["m"][3].as_f64().unwrap_or(0.0),
                    energy_storage: r["e"][3].as_f64().unwrap_or(0.0),
                    metal_income: r["m"][1].as_f64().unwrap_or(0.0),
                    energy_income,
                    extractors: defs.iter().filter(|d| d.extracts_metal > 0.0).count() as u32,
                    extractors_built,
                    constructors: defs.iter().filter(|d| d.role == Role::Builder).count() as u32,
                    army_value,
                    losses,
                });
            }
            _ => {}
        }
    }
    let game = Game {
        commander: commander.ok_or("no commander in the record")?,
        home: home.ok_or("no commander in the record")?,
        size: (number(&map["width"]), number(&map["height"])),
        spots,
        wind: (number(&map["wind_min"]), number(&map["wind_max"])),
        wind_override: None,
        units: table,
        terrain: terrain(path, &header),
    };
    Ok(Replay { game, trips, plan, steps, built_by: every_builder, finished_at: finished_ids, types, seen, observed, wind, unknown, abandoned, first_factory_finished })
}

impl Replay {
    /// The game as it stood at second `t` of the record, and the plan of what its builders started after that: the
    /// check of the snapshot simulation (`docs/design/2026-09-21-rolling-planner.md`). Builders alive at `t` come
    /// first in the state's order; factories and constructors finished later get the queues after them, in the
    /// order they finished. A nanoframe at `t` is the job of whoever started it, its progress its health.
    pub fn state_at(&self, t: f64, scenario: &Scenario) -> Option<(State, Plan)> {
        let units = &self.game.units;
        let (_, listed) = self.seen.iter().min_by(|a, b| (a.0 - t).abs().total_cmp(&(b.0 - t).abs())).filter(|(at, _)| (at - t).abs() < 1.0)?;
        let observed = self.observed.iter().find(|o| (o.t - t).abs() < 1e-6)?;
        let pays = |unit: usize, site: (f64, f64)| {
            if units.list[unit].extracts_metal == 0.0 {
                return 0.0;
            }
            scenario.spots.iter().find(|s| distance(s.at, site) < 100.0).map_or(0.0, |s| s.metal)
        };
        let standing: Vec<Standing> = listed
            .iter()
            .filter(|u| !u.being_built && matches!(units.list[u.unit].role, Role::Eco | Role::Turret | Role::Nano))
            .map(|u| Standing { unit: u.unit, site: u.at, pays: pays(u.unit, u.at) })
            .collect();
        let mut builders: Vec<Seen> = listed.iter().copied().filter(|u| !u.being_built && matches!(units.list[u.unit].role, Role::Commander | Role::Factory | Role::Builder)).collect();
        let rank = |role: Role| match role {
            Role::Commander => 0,
            Role::Factory => 1,
            _ => 2,
        };
        builders.sort_by_key(|b| (rank(units.list[b.unit].role), b.id));
        let job_of = |builder: u64| {
            listed.iter().find(|u| u.being_built && self.built_by.get(&u.id) == Some(&builder)).map(|u| Job { unit: u.unit, site: u.at, progress: u.health, pays: pays(u.unit, u.at) })
        };
        let state = State {
            t0: t,
            metal: observed.metal,
            energy: observed.energy,
            metal_storage: observed.metal_storage,
            energy_storage: observed.energy_storage,
            standing,
            builders: builders.iter().map(|b| StateBuilder { unit: b.unit, place: b.at, job: job_of(b.id) }).collect(),
        };
        // Queues: the standing builders' in the state's order, then those of the factories and constructors that
        // finished after t, in finish order.
        let mut later: Vec<(f64, u64, Role)> = self
            .finished_at
            .iter()
            .filter(|(_, at)| **at > t)
            .filter_map(|(id, at)| {
                let role = units.list[*self.types.get(id)?].role;
                matches!(role, Role::Factory | Role::Builder).then_some((*at, *id, role))
            })
            .collect();
        later.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let queue_for = |builder: u64| -> Vec<Step> { self.steps.iter().filter(|s| s.at > t && s.builder == builder).map(|s| Step { item: Item::Build(s.unit), site: Some(s.site) }).collect() };
        let mut plan = Plan::empty(0, 0);
        for b in &builders {
            match units.list[b.unit].role {
                Role::Commander => plan.commander = queue_for(b.id),
                Role::Factory => plan.factories.push(queue_for(b.id)),
                _ => plan.constructors.push(queue_for(b.id)),
            }
        }
        for (_, id, role) in later {
            match role {
                Role::Factory => plan.factories.push(queue_for(id)),
                _ => plan.constructors.push(queue_for(id)),
            }
        }
        Some((state, plan))
    }
}
