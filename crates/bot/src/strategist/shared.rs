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
    /// Tier 2: `true` starts the advanced lab now whatever the economy, `false` holds it back.
    pub tier2: Option<Timed<bool>>,
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
        lapse(&mut self.tier2, frame);
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
        if let Some(t) = self.tier2 {
            lines.push(format!("tier2={} ({})", if t.value { "go" } else { "hold" }, left(t.expires_frame)));
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
    /// Metal spots by their number in the map's list: taken first and in this order (wherever they are, raided or
    /// not), and never taken.
    pub spot_priority: Vec<usize>,
    pub spot_avoid: Vec<usize>,
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
    /// The expansion plan in force, as spot numbers with their grid cells, for the report.
    pub spot_plan: String,
    pub score: Score,
}

/// How the game stands, in every report: a commander shown only threats plays only defence.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Score {
    pub extractors: usize,
    pub extractor_peak: usize,
    /// Game seconds since our extractor count last reached a new high.
    pub seconds_since_growth: i32,
    /// Metal spots we can walk to that nobody is known to hold: all of them, and those within 2500 walk of home.
    pub free_spots: usize,
    pub free_spots_near: usize,
    /// Spots the opponent is known to hold (its extractors seen and not seen dead).
    pub enemy_spots_seen: usize,
    pub soldiers: usize,
    pub army_metal: u32,
    pub soldiers_near_home: usize,
    /// Opponent extractors seen and not known to be dead: a floor, since we see little of their side.
    pub metal_income: f32,
    /// (minutes ago, extractors, metal income, army metal) for 3 and 6 minutes ago, when the game is that old.
    pub trend: Vec<(i32, usize, f32, u32)>,
    pub extractors_lost_3_min: usize,
    /// Metal of ours destroyed and of theirs we saw destroyed (units and buildings): in the last three minutes, and
    /// in the whole game. Kills out of our sight are not counted.
    pub traded_3_min: (u32, u32),
    pub traded: (u32, u32),
    /// Game seconds since the commander's previous turn began; `None` before its first.
    pub seconds_since_turn: Option<i32>,
    /// Where a factory of the opponent's has been seen, standing or not.
    pub enemy_base_found: Option<Place>,
    /// The opponent's soldiers seen in the last three minutes and not seen to die, and their metal. Older sightings
    /// are left out: most of its soldiers die where we cannot see, and a count that never forgets only grows.
    pub enemy_soldiers_seen: usize,
    pub enemy_soldiers_seen_metal: u32,
    /// Its factories seen and not seen destroyed, and its commander's last sighting with its age in seconds.
    pub enemy_factories: Vec<Place>,
    pub enemy_commander: Option<(Place, i32)>,
    /// Its extractors seen outside its base, nearest to us first, each with the metal of turrets known within 500.
    pub raid_targets: Vec<(Place, u32)>,
    /// Set when our soldiers stand at the guessed enemy start and no enemy building is known near it: the guess is
    /// wrong, and these are the nearest metal spots none of our soldiers is near, where a base could be.
    pub guess_disproved: Option<Vec<Place>>,
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
    /// The spot's number in the map's list, as the `expansion` tool takes it.
    pub spot: Option<usize>,
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
    /// WITHIN_REASON_THINK_PENALTY: the commander's orders take effect this many game seconds late per wall second
    /// it thought (1 = as if the game had kept running while it thought; 0 = at once). Set once at start.
    pub think_penalty: Mutex<f32>,
    /// A turn's orders waiting out their delay: the frame they take effect, and what then becomes live.
    pub delayed: Mutex<Option<(i32, Directives, FieldOrders, Wake)>>,
}

impl Shared {
    pub fn trigger(&self, text: String) {
        self.triggers.lock().unwrap().push(text);
    }

    /// Brain side: ask for a turn and hold the game until it is over. Returns at once when no session can answer.
    pub fn hold_for_turn(&self, reason: String, frame: i32) {
        let before = (self.directives.lock().unwrap().clone(), self.field_orders.lock().unwrap().clone(), self.wake.lock().unwrap().clone());
        let started = std::time::Instant::now();
        {
            let mut gate = self.gate.lock().unwrap();
            if gate.closed {
                return;
            }
            gate.requested = Some(reason);
            self.gate_changed.notify_all();
            let _gate = self.gate_changed.wait_while(gate, |g| !g.closed && (g.requested.is_some() || g.in_progress)).unwrap();
        }
        // The game stood still while the commander thought. With a penalty, what it ordered waits as long (in game
        // time) as it took to decide, and the old orders stand meanwhile: the latency it would have in a live game.
        let penalty = *self.think_penalty.lock().unwrap();
        if penalty > 0.0 {
            let delay = (started.elapsed().as_secs_f32() * penalty * 30.0) as i32;
            let ordered = (
                std::mem::replace(&mut *self.directives.lock().unwrap(), before.0),
                std::mem::replace(&mut *self.field_orders.lock().unwrap(), before.1),
                std::mem::replace(&mut *self.wake.lock().unwrap(), before.2),
            );
            *self.delayed.lock().unwrap() = Some((frame + delay, ordered.0, ordered.1, ordered.2));
        }
    }

    /// Brain side, every tick: puts a delayed turn's orders into force when their time has come. True while one waits.
    pub fn apply_delayed(&self, frame: i32) -> bool {
        let mut delayed = self.delayed.lock().unwrap();
        match delayed.take() {
            Some((at, directives, orders, wake)) if frame >= at => {
                *self.directives.lock().unwrap() = directives;
                *self.field_orders.lock().unwrap() = orders;
                *self.wake.lock().unwrap() = wake;
                false
            }
            waiting => {
                let pending = waiting.is_some();
                *delayed = waiting;
                pending
            }
        }
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
