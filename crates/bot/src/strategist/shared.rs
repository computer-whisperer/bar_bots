//! What the brain and the strategist exchange: a briefing going up, directives coming down.

use std::collections::BTreeMap;
use std::sync::{Condvar, Mutex};

use bot_protocol::{Resource, Vec3};
use serde::{Deserialize, Serialize};

/// The brain's summary of the game, published every tick for the strategist to read.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Briefing {
    pub game_time: String,
    pub frame: i32,
    pub metal: Resource,
    pub energy: Resource,
    pub counts: Counts,
    pub home: Place,
    pub presumed_enemy_start: Place,
    pub home_group: Group,
    pub attackers: Group,
    pub waves_sent: usize,
    /// Where the home group waits: ahead of our most exposed extractors unless a directive says otherwise.
    pub army_station: Place,
    /// Enemies in sight or radar right now, grouped by map grid cell.
    pub enemies_visible: Vec<EnemyCluster>,
    /// Enemy buildings seen earlier and not known to be destroyed.
    pub enemy_buildings_remembered: Vec<RememberedBuilding>,
    /// Newest last.
    pub recent_events: Vec<String>,
    pub directives_in_force: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Counts {
    pub extractors: usize,
    pub generators: usize,
    pub converters: usize,
    pub labs: usize,
    pub turrets: usize,
    pub constructors: usize,
    pub army: usize,
}

/// A position with its grid cell name, so it can be talked about.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Place {
    pub grid: String,
    pub x: i32,
    pub z: i32,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Group {
    pub size: usize,
    pub idle: usize,
    pub centre: Option<Place>,
    /// Unit name to count.
    pub composition: Vec<(String, usize)>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EnemyCluster {
    pub at: Place,
    pub units: usize,
    /// Unit name to count; radar-only contacts are "unidentified".
    pub composition: Vec<(String, usize)>,
    pub distance_from_home: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct RememberedBuilding {
    pub name: String,
    pub at: Place,
    pub last_seen: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    /// Keep the whole army at home.
    Defend,
    /// Keep building the home group; launch nothing.
    Gather,
    /// Commit the home group now, whatever its size.
    Attack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Focus {
    Expand,
    Energy,
    Production,
    Defence,
}

/// A directive value that lapses, so a silent strategist hands control back to the heuristics.
#[derive(Clone, Copy, Debug)]
pub struct Timed<T> {
    pub value: T,
    pub expires_frame: i32,
}

/// The strategist's standing orders. Every field is "directive, else the heuristic's default".
#[derive(Clone, Debug, Default)]
pub struct Directives {
    pub army_stance: Option<Timed<Stance>>,
    pub attack_target: Option<Timed<Vec3>>,
    pub wave_size: Option<Timed<usize>>,
    pub economy_focus: Option<Timed<Focus>>,
    pub army_station: Option<Timed<Vec3>>,
    pub min_constructors: Option<Timed<usize>>,
    pub min_converters: Option<Timed<usize>>,
    /// Constructors take no metal spot farther than this from home, on foot.
    pub expansion_radius: Option<Timed<usize>>,
    /// Where the commander stands and builds, instead of roaming its leash around home.
    pub commander_station: Option<Timed<Vec3>>,
}

impl Directives {
    pub fn expire(&mut self, frame: i32) {
        fn lapse<T>(slot: &mut Option<Timed<T>>, frame: i32) {
            if slot.as_ref().is_some_and(|t| t.expires_frame <= frame) {
                *slot = None;
            }
        }
        lapse(&mut self.army_stance, frame);
        lapse(&mut self.attack_target, frame);
        lapse(&mut self.wave_size, frame);
        lapse(&mut self.economy_focus, frame);
        lapse(&mut self.army_station, frame);
        lapse(&mut self.min_constructors, frame);
        lapse(&mut self.min_converters, frame);
        lapse(&mut self.expansion_radius, frame);
        lapse(&mut self.commander_station, frame);
    }

    pub fn describe(&self, frame: i32) -> Vec<String> {
        let left = |expires: i32| format!("{}s left", (expires - frame).max(0) / 30);
        let mut lines = Vec::new();
        if let Some(t) = self.army_stance {
            lines.push(format!("army_stance={:?} ({})", t.value, left(t.expires_frame)));
        }
        if let Some(t) = self.attack_target {
            lines.push(format!("attack_target=({:.0}, {:.0}) ({})", t.value.x, t.value.z, left(t.expires_frame)));
        }
        if let Some(t) = self.wave_size {
            lines.push(format!("wave_size={} ({})", t.value, left(t.expires_frame)));
        }
        if let Some(t) = self.economy_focus {
            lines.push(format!("economy_focus={:?} ({})", t.value, left(t.expires_frame)));
        }
        if let Some(t) = self.army_station {
            lines.push(format!("army_station=({:.0}, {:.0}) ({})", t.value.x, t.value.z, left(t.expires_frame)));
        }
        if let Some(t) = self.min_constructors {
            lines.push(format!("min_constructors={} ({})", t.value, left(t.expires_frame)));
        }
        if let Some(t) = self.min_converters {
            lines.push(format!("min_converters={} ({})", t.value, left(t.expires_frame)));
        }
        if let Some(t) = self.expansion_radius {
            lines.push(format!("expansion_radius={} ({})", t.value, left(t.expires_frame)));
        }
        if let Some(t) = self.commander_station {
            lines.push(format!("commander_station=({:.0}, {:.0}) ({})", t.value.x, t.value.z, left(t.expires_frame)));
        }
        lines
    }
}

/// A standing defensive position: stand at `at`, engage what comes within `radius`, go back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Post {
    pub at: Vec3,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderKind {
    Move,
    Fight,
}

/// What the commander wants of one squad. The brain owns the membership; this is the request side.
#[derive(Clone, Debug, Default)]
pub struct SquadRequest {
    /// Unit name to how many more to draw from the unassigned pool; emptied by the brain as it fills them.
    pub take: BTreeMap<String, usize>,
    /// Draw the units nearest to this point (else to the post, else to home).
    pub near: Option<Vec3>,
    pub post: Option<Post>,
    /// A one-off order, taken by the brain when issued.
    pub order: Option<(OrderKind, Vec3)>,
    /// Hand every member back to the heuristics and forget the squad.
    pub release: bool,
}

/// The field commander's levers (see `DESIGN.md`, "Field commander").
#[derive(Clone, Debug, Default)]
pub struct FieldOrders {
    pub squads: BTreeMap<String, SquadRequest>,
    /// Unit name to weight. Empty: the heuristic batch.
    pub production: BTreeMap<String, u32>,
    pub turret_requests: Vec<Vec3>,
}

/// What the commander is shown each turn, beyond the briefing.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Field {
    /// Soldiers no squad has claimed, by unit name; the heuristics command these.
    pub unassigned: Vec<(String, usize)>,
    pub unassigned_centre: Option<Place>,
    pub squads: Vec<SquadStatus>,
    pub extractors: Vec<ExtractorStatus>,
    pub turrets: Vec<Place>,
    /// What our factories can build, with metal cost.
    pub buildable: Vec<(String, u32)>,
    pub production_weights: Vec<(String, u32)>,
    pub turret_requests_pending: usize,
    pub score: Score,
}

/// How the game stands, in every report: a commander shown only threats plays only defence.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Score {
    pub extractors: usize,
    pub extractor_peak: usize,
    /// Game seconds since our extractor count last reached a new high.
    pub seconds_since_growth: i32,
    /// Free metal spots nearer to us than to the opponent on foot, and how many of them lie within 2500 of home.
    pub free_spots_ours: usize,
    pub free_spots_near: usize,
    pub soldiers: usize,
    pub army_metal: u32,
    pub soldiers_near_home: usize,
    /// Opponent extractors seen and not known to be dead: a floor, since we see little of their side.
    pub enemy_extractors_seen: usize,
    /// The biggest opponent army seen in one look lately, in metal, and how many seconds ago.
    pub enemy_army_seen: Option<(u32, i32)>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SquadStatus {
    pub name: String,
    pub composition: Vec<(String, usize)>,
    pub health_percent: u32,
    pub centre: Option<Place>,
    pub post: Option<(Place, u32)>,
    pub still_wanted: Vec<(String, usize)>,
    pub engaged: bool,
    /// What became of the last post or order: moved to walkable ground, or refused.
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExtractorStatus {
    pub at: Place,
    pub enemies_within_600: usize,
    pub turret_within_300: bool,
}

/// When the commander wants to be woken. It sets these itself (`wait` tool); they hold until changed.
#[derive(Clone, Debug, Serialize)]
pub struct Wake {
    /// Game seconds after which it is woken whatever happens.
    pub max_seconds: u32,
    /// Enemies appear within 600 of an extractor that had none near it.
    pub enemy_near_extractor: bool,
    /// A posted squad starts fighting.
    pub squad_engaged: bool,
    pub extractor_lost: bool,
    /// Unit name to count: woken when that many of the type stand unassigned.
    pub pool_reaches: BTreeMap<String, usize>,
}

impl Default for Wake {
    fn default() -> Self {
        Wake { max_seconds: 30, enemy_near_extractor: true, squad_engaged: true, extractor_lost: true, pool_reaches: BTreeMap::new() }
    }
}

/// Lockstep turns: the brain asks for a turn and holds the game (its reply to the engine) until the turn is over.
#[derive(Default)]
pub struct Gate {
    /// Why the commander is being woken; taken by the driver.
    pub requested: Option<String>,
    pub in_progress: bool,
    /// The session is gone: never hold the game again.
    pub closed: bool,
}

/// State shared between the brain's thread, the MCP server and the strategist driver.
#[derive(Default)]
pub struct Shared {
    pub briefing: Mutex<Briefing>,
    pub directives: Mutex<Directives>,
    /// Things worth waking the strategist for, drained by the driver.
    pub triggers: Mutex<Vec<String>>,
    /// Static map description, filled once at game start.
    pub map: Mutex<serde_json::Value>,
    pub field: Mutex<Field>,
    pub field_orders: Mutex<FieldOrders>,
    /// Losses and kills since the commander last looked ("lost armpw to corak in our half" to count).
    pub fights: Mutex<BTreeMap<String, u32>>,
    /// The commander's own notes, carried across session restarts.
    pub notes: Mutex<Vec<String>>,
    pub wake: Mutex<Wake>,
    /// True when turns are taken in lockstep with the game (the field commander).
    pub lockstep: std::sync::atomic::AtomicBool,
    pub gate: Mutex<Gate>,
    pub gate_changed: Condvar,
}

impl Shared {
    pub fn trigger(&self, text: String) {
        self.triggers.lock().unwrap().push(text);
    }

    /// Brain side: ask for a turn and hold the game until it is over. Returns at once when no session can answer.
    pub fn hold_for_turn(&self, reason: String) {
        let mut gate = self.gate.lock().unwrap();
        if gate.closed {
            return;
        }
        gate.requested = Some(reason);
        self.gate_changed.notify_all();
        let _gate = self.gate_changed.wait_while(gate, |g| !g.closed && (g.requested.is_some() || g.in_progress)).unwrap();
    }

    /// Driver side: wait for the brain to ask for a turn. `None` when `give_up` turns true.
    pub fn next_turn_request(&self, give_up: &std::sync::atomic::AtomicBool) -> Option<String> {
        let mut gate = self.gate.lock().unwrap();
        loop {
            if give_up.load(std::sync::atomic::Ordering::Relaxed) {
                return None;
            }
            if let Some(reason) = gate.requested.take() {
                gate.in_progress = true;
                return Some(reason);
            }
            gate = self.gate_changed.wait_timeout(gate, std::time::Duration::from_millis(250)).unwrap().0;
        }
    }

    pub fn end_turn(&self) {
        self.gate.lock().unwrap().in_progress = false;
        self.gate_changed.notify_all();
    }

    pub fn close_gate(&self) {
        let mut gate = self.gate.lock().unwrap();
        gate.closed = true;
        gate.in_progress = false;
        self.gate_changed.notify_all();
    }
}
