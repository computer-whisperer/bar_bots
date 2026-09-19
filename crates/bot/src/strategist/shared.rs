//! What the brain and the strategist exchange: a briefing going up, directives coming down.

use std::sync::Mutex;

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
        lines
    }
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
}

impl Shared {
    pub fn trigger(&self, text: String) {
        self.triggers.lock().unwrap().push(text);
    }
}
