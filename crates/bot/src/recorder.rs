//! Match record: what the bot saw and decided, as JSON Lines, for `viewer/` (`docs/harness/record-format.md`).
//!
//! Switched on with `WITHIN_REASON_RECORD=1`; writes `record-<ai_id>.jsonl` into the log directory. The file is
//! append-only and flushed every tick, so a match that is killed is viewable up to its last tick.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use bot_protocol::{Command, Event, Hello, Resource, Tick, UnitDefId, UnitId, Vec3};
use serde_json::{Value, json};

use crate::brain::journal::{Intent, Journal, Milling};

pub const FORMAT_VERSION: u32 = 1;
/// A state sample once a game second: the viewer interpolates between samples, and a 40-minute game stays
/// around ten megabytes (measured; see the format document).
const SAMPLE_FRAMES: i32 = 30;

const FLAG_BEING_BUILT: u8 = 1;
const FLAG_IDLE: u8 = 2;
/// The brain's role bits (`journal::ROLE_*`) sit above the unit's own.
const ROLE_SHIFT: u8 = 2;

pub struct Recorder {
    out: File,
    /// The line being assembled; one write per tick.
    buffer: String,
    /// Engine definition id to index in the header's table.
    def_index: HashMap<UnitDefId, usize>,
    header: Option<Value>,
    /// Units of either side as last seen, so an event about a unit that is gone still has a type and a place.
    known: HashMap<UnitId, (Option<UnitDefId>, Vec3)>,
    next_sample: i32,
    /// Since the last sample: heuristic firings, damage taken per unit, slowest `decide`.
    rules: BTreeMap<&'static str, u32>,
    /// The control lane's milling counters since the last sample.
    milling: Milling,
    damage: BTreeMap<UnitId, f32>,
    slowest_decide_ms: f32,
    /// The most frames a tick of the interval arrived late by (`Tick::late`).
    latest_tick: i32,
    intent: Option<Intent>,
}

impl Recorder {
    /// `None` unless `WITHIN_REASON_RECORD` is set to something other than `0`.
    /// `session`: a Claude Code session writes `strategist-<ai>.jsonl` beside the record; `pianist`: `jev-<ai>.jsonl`.
    pub fn from_env(dir: &Path, hello: &Hello, mode: &str, session: bool, pianist: bool) -> Option<Recorder> {
        if std::env::var("WITHIN_REASON_RECORD").map_or(true, |v| v.is_empty() || v == "0") {
            return None;
        }
        let path = dir.join(format!("record-{}.jsonl", hello.ai_id));
        if let Err(e) = write_terrain(&dir.join(terrain_file(hello)), hello) {
            eprintln!("[ai {}] no terrain file: {e}", hello.ai_id);
        }
        // Appending: a bot restarted mid-match continues the same record with a second header line.
        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(out) => Some(Recorder {
                out,
                buffer: String::new(),
                def_index: hello.unit_defs.iter().enumerate().map(|(i, d)| (d.id, i)).collect(),
                header: Some(header(hello, mode, session, pianist)),
                known: HashMap::new(),
                next_sample: 0,
                rules: BTreeMap::new(),
                milling: Milling::default(),
                damage: BTreeMap::new(),
                slowest_decide_ms: 0.0,
                latest_tick: 0,
                intent: None,
            }),
            Err(e) => {
                eprintln!("[ai {}] no match record: {}: {e}", hello.ai_id, path.display());
                None
            }
        }
    }

    /// Records one tick: the events, the brain's journal, the commands it answered with, and a sample when due.
    pub fn tick(&mut self, tick: &Tick, journal: Journal, role: impl Fn(UnitId) -> u8, commands: &[Command], decide_ms: f32) -> io::Result<()> {
        if let Some(mut header) = self.header.take() {
            // The header waits for the first tick because only a unit tells which faction we play.
            let first = tick.snapshot.own_units.first().and_then(|u| self.def_index.get(&u.def));
            let side = first.and_then(|&i| header["unit_defs"][i]["name"].as_str()).and_then(|name| name.get(..3)).map(str::to_string);
            header["side"] = json!(side);
            header["first_tick_frame"] = json!(tick.frame);
            self.line(&header);
        }
        for unit in &tick.snapshot.own_units {
            self.known.insert(unit.id, (Some(unit.def), unit.pos));
        }
        for enemy in &tick.snapshot.enemies {
            let def = enemy.def.or_else(|| self.known.get(&enemy.id).and_then(|k| k.0));
            self.known.insert(enemy.id, (def, enemy.pos));
        }
        for event in &tick.events {
            self.event(tick.frame, event);
        }
        for (rule, n) in journal.rules {
            *self.rules.entry(rule).or_default() += n;
        }
        self.milling += journal.milling;
        for note in journal.notes {
            self.line(&json!({ "t": "d", "f": note.frame, "source": note.source, "kind": note.kind, "inputs": note.inputs, "outputs": note.outputs }));
        }
        if self.intent != Some(journal.intent) {
            self.intent = Some(journal.intent);
            let at = |p: Vec3| json!([p.x as i32, p.z as i32]);
            let i = journal.intent;
            self.line(&json!({
                "t": "intent", "f": tick.frame, "home": at(i.home), "enemy_start": at(i.enemy_start), "station": at(i.station),
                "target": i.target.map(at), "staging": i.staging.map(at),
            }));
        }
        self.commands(tick.frame, commands);
        self.slowest_decide_ms = self.slowest_decide_ms.max(decide_ms);
        self.latest_tick = self.latest_tick.max(tick.late);
        if tick.frame >= self.next_sample {
            self.next_sample = tick.frame - tick.frame % SAMPLE_FRAMES + SAMPLE_FRAMES;
            self.sample(tick, role);
        }
        self.out.write_all(self.buffer.as_bytes())?;
        self.buffer.clear();
        Ok(())
    }

    fn line(&mut self, value: &Value) {
        let _ = writeln!(self.buffer, "{value}");
    }

    fn def(&self, def: Option<UnitDefId>) -> i64 {
        def.and_then(|d| self.def_index.get(&d)).map_or(-1, |&i| i as i64)
    }

    fn event(&mut self, frame: i32, event: &Event) {
        // `who` is the unit the event is about, with its type and last known place.
        let about = |this: &Self, kind: &str, who: UnitId| {
            let (def, pos) = this.known.get(&who).copied().unwrap_or_default();
            json!({ "t": "ev", "f": frame, "k": kind, "u": who.0, "d": this.def(def), "x": pos.x as i32, "z": pos.z as i32 })
        };
        let record = match *event {
            // Idle is a unit flag in the samples; damage is summed into the next sample.
            Event::UnitIdle { .. } => return,
            Event::UnitDamaged { unit, damage, .. } => {
                *self.damage.entry(unit).or_default() += damage;
                return;
            }
            Event::UnitCreated { unit, builder } => {
                let mut r = about(self, "created", unit);
                r["by"] = json!(builder.map(|b| b.0));
                r
            }
            Event::UnitFinished { unit } => about(self, "finished", unit),
            Event::UnitMoveFailed { unit } => about(self, "move_failed", unit),
            Event::UnitDestroyed { unit, attacker } => {
                let mut r = about(self, "destroyed", unit);
                r["by"] = json!(attacker.map(|a| a.0));
                r["by_d"] = json!(self.def(attacker.and_then(|a| self.known.get(&a)).and_then(|k| k.0)));
                self.known.remove(&unit);
                r
            }
            Event::EnemyEnterLos { enemy } => about(self, "enemy_seen", enemy),
            Event::EnemyLeaveLos { enemy } => about(self, "enemy_lost", enemy),
            Event::EnemyDestroyed { enemy } => {
                let r = about(self, "enemy_destroyed", enemy);
                self.known.remove(&enemy);
                r
            }
            Event::BuildSiteNotFound { unit, def } => {
                let mut r = about(self, "no_site", unit);
                r["what"] = json!(self.def(Some(def)));
                r
            }
            Event::CommandRejected { unit, code } => {
                let mut r = about(self, "rejected", unit);
                r["code"] = json!(code);
                r
            }
        };
        self.line(&record);
    }

    fn commands(&mut self, frame: i32, commands: &[Command]) {
        if commands.is_empty() {
            return;
        }
        let list: Vec<Value> = commands
            .iter()
            .map(|command| match *command {
                Command::Build { unit, def, site, .. } => match site {
                    Some(site) => json!(["build", unit.0, self.def(Some(def)), site.near.x as i32, site.near.z as i32]),
                    None => json!(["build", unit.0, self.def(Some(def))]),
                },
                Command::Move { unit, to, .. } => json!(["move", unit.0, to.x as i32, to.z as i32]),
                Command::Fight { unit, to, .. } => json!(["fight", unit.0, to.x as i32, to.z as i32]),
                Command::Stop { unit } => json!(["stop", unit.0]),
                Command::SetRepeat { unit, repeat } => json!(["repeat", unit.0, repeat as i32]),
                Command::Guard { unit, target } => json!(["guard", unit.0, target.0]),
                Command::Repair { unit, target, .. } => json!(["repair", unit.0, target.0]),
                Command::ReclaimFeature { unit, feature, .. } => json!(["reclaim_feature", unit.0, feature.0]),
                Command::Resurrect { unit, feature, .. } => json!(["resurrect", unit.0, feature.0]),
                Command::Say { ref text } => json!(["say", text]),
                Command::Attack { unit, target, .. } => json!(["attack", unit.0, target.0]),
                Command::GiveUnit { def, at } => json!(["give", self.def(Some(def)), at.x as i32, at.z as i32]),
                Command::SelfDestruct { unit } => json!(["selfdestruct", unit.0]),
            })
            .collect();
        self.line(&json!({ "t": "cmd", "f": frame, "c": list }));
    }

    /// Written by hand: this is nine tenths of the file, and `[id,def,x,z,health%,flags]` rows keep it small.
    fn sample(&mut self, tick: &Tick, role: impl Fn(UnitId) -> u8) {
        let s = &tick.snapshot;
        let resource = |r: Resource| format!("[{:.0},{:.1},{:.1},{:.0}]", r.current, r.income, r.usage, r.storage);
        let _ = write!(self.buffer, "{{\"t\":\"s\",\"f\":{},\"m\":{},\"e\":{},\"own\":[", tick.frame, resource(s.metal), resource(s.energy));
        for (i, u) in s.own_units.iter().enumerate() {
            let flags = u8::from(u.being_built) * FLAG_BEING_BUILT | u8::from(u.idle) * FLAG_IDLE | role(u.id) << ROLE_SHIFT;
            let health = (u.health / u.max_health.max(1.0) * 100.0).round() as i32;
            let comma = if i == 0 { "" } else { "," };
            let _ = write!(self.buffer, "{comma}[{},{},{},{},{health},{flags}]", u.id.0, self.def(Some(u.def)), u.pos.x as i32, u.pos.z as i32);
        }
        self.buffer.push_str("],\"en\":[");
        for (i, e) in s.enemies.iter().enumerate() {
            let comma = if i == 0 { "" } else { "," };
            let def = self.def(self.known.get(&e.id).and_then(|k| k.0));
            let _ = write!(self.buffer, "{comma}[{},{def},{},{},{:.0}]", e.id.0, e.pos.x as i32, e.pos.z as i32, e.health);
        }
        self.buffer.push_str("],\"al\":[");
        for (i, a) in s.allies.iter().enumerate() {
            let comma = if i == 0 { "" } else { "," };
            let _ = write!(self.buffer, "{comma}[{},{},{},{},{},{}]", a.id.0, self.def(Some(a.def)), a.pos.x as i32, a.pos.z as i32, a.team, a.being_built as i32);
        }
        self.buffer.push_str("],\"dmg\":[");
        for (i, (unit, damage)) in std::mem::take(&mut self.damage).into_iter().enumerate() {
            let comma = if i == 0 { "" } else { "," };
            let _ = write!(self.buffer, "{comma}[{},{damage:.0}]", unit.0);
        }
        let _ = write!(self.buffer, "],\"ms\":{:.2},\"late\":{}", std::mem::take(&mut self.slowest_decide_ms), std::mem::take(&mut self.latest_tick));
        let milling = std::mem::take(&mut self.milling);
        if milling.claims > 0 {
            let _ = write!(self.buffer, ",\"lane\":[{},{:.0},{:.0},{}]", milling.claims, milling.path, milling.net, milling.reversals);
        }
        self.buffer.push_str("}\n");
        if !self.rules.is_empty() {
            let rules = std::mem::take(&mut self.rules);
            self.line(&json!({ "t": "d", "f": tick.frame, "source": "heuristic", "kind": "rules", "inputs": null, "outputs": rules }));
        }
    }
}

fn terrain_file(hello: &Hello) -> String {
    format!("terrain-{}.bin", hello.ai_id)
}

/// The terrain grid as raw bytes beside the record: every height as a little-endian i16, then every slope as a u8.
/// The record's header says how to read it.
fn write_terrain(path: &Path, hello: &Hello) -> io::Result<()> {
    let terrain = &hello.terrain;
    let mut bytes: Vec<u8> = terrain.heights.iter().flat_map(|h| h.to_le_bytes()).collect();
    bytes.extend_from_slice(&terrain.slopes);
    std::fs::write(path, bytes)
}

/// The distinct ways our side's land units move, for the viewer's "cannot go here" layers.
fn move_classes(hello: &Hello) -> Vec<Value> {
    let mut classes: Vec<((String, i32, i32), Value, usize)> = Vec::new();
    for def in &hello.unit_defs {
        let Some(class) = def.move_class else { continue };
        let kind = format!("{:?}", class.kind).to_lowercase();
        let key = (kind.clone(), (class.max_slope * 1000.0) as i32, class.depth as i32);
        match classes.iter_mut().find(|(k, _, _)| *k == key) {
            Some((_, _, units)) => *units += 1,
            None => classes.push((key, json!({ "kind": kind, "max_slope": class.max_slope, "depth": class.depth, "example": def.name }), 1)),
        }
    }
    // `units` tells the ordinary class of a kind (most unit types) from the specialists (amphibians, climbers).
    classes.into_iter().map(|(_, mut class, units)| { class["units"] = json!(units); class }).collect()
}

fn header(hello: &Hello, mode: &str, session: bool, pianist: bool) -> Value {
    let defs: Vec<Value> = hello
        .unit_defs
        .iter()
        .map(|d| {
            json!({
                "id": d.id.0, "name": d.name, "class": class(d), "metal": d.metal_cost, "energy": d.energy_cost,
                "speed": d.speed, "weapons": d.weapon_count,
                "build_time": d.build_time, "build_speed": d.build_speed, "build_distance": d.build_distance,
                "builds": d.build_options.iter().map(|o| o.0).collect::<Vec<_>>(),
                "extracts_metal": d.extracts_metal, "metal_make": d.metal_make, "energy_make": d.energy_make,
                "energy_upkeep": d.energy_upkeep, "wind_cap": d.wind_cap, "metal_storage": d.metal_storage,
                "energy_storage": d.energy_storage,
                "radar_range": d.radar_range,
                "converter": d.converter.map(|c| json!([c.capacity, c.efficiency])),
                "move": d.move_class.map(|m| json!([format!("{:?}", m.kind).to_lowercase(), m.max_slope, m.depth, m.slope_mod])),
            })
        })
        .collect();
    let spots: Vec<Value> = hello.metal_spots.iter().map(|s| json!([s.x as i32, s.z as i32, s.y])).collect();
    let map = &hello.map;
    // The Claude Code session beside the brain keeps its own transcript (`strategist/transcript.rs`); the pianist its
    // own log of every call (`brain/pianist/mod.rs`).
    let mut decision_logs: Vec<String> = session.then(|| format!("strategist-{}.jsonl", hello.ai_id)).into_iter().collect();
    if pianist {
        decision_logs.push(format!("jev-{}.jsonl", hello.ai_id));
    }
    json!({
        "t": "header", "format": "within-reason-record", "version": FORMAT_VERSION,
        "ai_id": hello.ai_id, "team": hello.team, "ally_team": hello.ally_team, "start_frame": hello.frame,
        "frames_per_second": 30, "sample_frames": SAMPLE_FRAMES, "tick_frames": hello.tick_frames, "mode": mode,
        "wall_start": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()),
        "map": { "name": map.name, "width": map.width, "height": map.height, "wind_min": map.wind_min, "wind_max": map.wind_max, "extractor_radius": map.extractor_radius },
        "grid": { "columns": 8, "rows": 8 },
        "metal_spots": spots,
        "terrain": {
            "file": terrain_file(hello), "cell": hello.terrain.cell, "width": hello.terrain.width, "height": hello.terrain.height,
            "layout": "heights as little-endian i16 (elmos, water level 0), row-major north to south; then slopes as u8 (engine slope x 255)",
            "move_classes": move_classes(hello),
        },
        "unit_defs": defs,
        "siblings": {
            "bot_log": "bot.log", "engine_log": "engine.log", "replay": "demos/*.sdfz",
            "decision_logs": decision_logs,
        },
    })
}

/// A coarse class from the definition's numbers alone, so it holds for every faction and for enemy units too.
fn class(d: &bot_protocol::UnitDefInfo) -> &'static str {
    let mobile = d.speed > 0.0;
    match () {
        _ if d.name.ends_with("com") && mobile && d.build_speed > 0.0 => "commander",
        _ if d.extracts_metal > 0.0 => "extractor",
        _ if !mobile && !d.build_options.is_empty() => "factory",
        _ if !mobile && d.weapon_count > 0 => "turret",
        _ if !mobile => "building",
        _ if d.build_speed > 0.0 => "builder",
        _ if d.weapon_count > 0 => "army",
        _ => "other",
    }
}
