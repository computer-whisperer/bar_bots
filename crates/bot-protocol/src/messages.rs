use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UnitId(pub i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UnitDefId(pub i32);

/// A map feature: a wreck, a rock, a tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FeatureId(pub i32);

/// Map position in elmos; `y` is height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    /// Horizontal distance; height is ignored.
    pub fn dist2d(self, other: Vec3) -> f32 {
        (self.x - other.x).hypot(self.z - other.z)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToBot {
    Hello(Hello),
    Tick(Tick),
}

/// Static game data, sent once per connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hello {
    pub ai_id: i32,
    pub team: i32,
    pub ally_team: i32,
    /// The same for every AI of one game and different from game to game (a hash of the start script): how the
    /// bot process tells which of its sessions play together.
    pub game_id: u64,
    /// Every team in the game, ours included.
    pub teams: Vec<TeamInfo>,
    /// Where each ally team may start, for the ally teams whose box the start script gives.
    pub start_boxes: Vec<StartBox>,
    pub frame: i32,
    pub map: MapInfo,
    pub unit_defs: Vec<UnitDefInfo>,
    pub metal_spots: Vec<Vec3>,
    pub terrain: Terrain,
}

/// One seat: a team has one commander and one economy; teams on one ally team fight together.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TeamInfo {
    pub team: i32,
    pub ally_team: i32,
    /// Faction name as the start script gives it; may be empty.
    pub side: String,
}

/// An ally team's start box, in elmos. The engine tells nobody where another team actually started.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct StartBox {
    pub ally_team: i32,
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl StartBox {
    pub fn centre(&self) -> Vec3 {
        Vec3 { x: (self.left + self.right) / 2.0, y: 0.0, z: (self.top + self.bottom) / 2.0 }
    }

    pub fn contains(&self, pos: Vec3) -> bool {
        pos.x >= self.left && pos.x <= self.right && pos.z >= self.top && pos.z <= self.bottom
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapInfo {
    pub name: String,
    /// Extent in elmos.
    pub width: f32,
    pub height: f32,
    /// Wind speed range; a wind generator produces the current wind speed in energy, up to its cap.
    pub wind_min: f32,
    pub wind_max: f32,
}

/// The ground, at the engine's slope-map resolution. Rows run north to south (z), cells west to east (x).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Terrain {
    /// Edge of one cell in elmos.
    pub cell: f32,
    pub width: u32,
    pub height: u32,
    /// Ground height in elmos; water level is 0, so negative is under water.
    pub heights: Vec<i16>,
    /// The engine's slope value (1 - the ground normal's y; 0 is flat), scaled by 255. Comparable with
    /// [`MoveClass::max_slope`] after the same scaling.
    pub slopes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveKind {
    Tank,
    Bot,
    Hover,
    Ship,
}

/// How a mobile ground or sea unit moves; aircraft and buildings have none.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MoveClass {
    pub kind: MoveKind,
    /// Steepest ground it crosses, as the engine's slope value.
    pub max_slope: f32,
    /// Deepest water a land unit wades; for a ship, the shallowest it floats in.
    pub depth: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitDefInfo {
    pub id: UnitDefId,
    pub name: String,
    pub metal_cost: f32,
    pub energy_cost: f32,
    pub speed: f32,
    pub build_speed: f32,
    /// Work one unit of `build_speed` does on it for one second adds `1 / build_time` of it.
    pub build_time: f32,
    /// How far from itself a builder builds, measured to the target's edge.
    pub build_distance: f32,
    pub extracts_metal: f32,
    /// Made every second whatever happens.
    pub metal_make: f32,
    pub energy_make: f32,
    /// Energy used every second; negative for what produces this way (solar collectors).
    pub energy_upkeep: f32,
    /// A wind generator makes the current wind speed in energy up to this.
    pub wind_cap: f32,
    pub metal_storage: f32,
    pub energy_storage: f32,
    /// For an energy converter (the game's `energyconv_*` custom parameters).
    pub converter: Option<Converter>,
    pub weapon_count: i32,
    pub build_options: Vec<UnitDefId>,
    pub move_class: Option<MoveClass>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Converter {
    /// Energy per second it can take.
    pub capacity: f32,
    /// Metal returned per energy.
    pub efficiency: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tick {
    pub frame: i32,
    /// Everything that happened since the previous tick, in order.
    pub events: Vec<Event>,
    pub snapshot: Snapshot,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub metal: Resource,
    pub energy: Resource,
    pub own_units: Vec<OwnUnit>,
    /// Units of the other teams on our ally team: seen, never commanded.
    pub allies: Vec<AllyUnit>,
    /// Enemies currently in line of sight or radar.
    pub enemies: Vec<EnemyUnit>,
    /// Reclaimable features in sight worth the walk (wrecks mostly), the richest first, capped. `None` on the ticks the
    /// shim did not look (it looks every few seconds): what was sent last still stands.
    pub wrecks: Option<Vec<Wreck>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wreck {
    pub id: FeatureId,
    pub pos: Vec3,
    /// Metal left in it.
    pub metal: f32,
    /// The unit a resurrection bot can raise from it.
    pub resurrects_into: Option<UnitDefId>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Resource {
    pub current: f32,
    pub income: f32,
    pub usage: f32,
    pub storage: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OwnUnit {
    pub id: UnitId,
    pub def: UnitDefId,
    pub pos: Vec3,
    pub health: f32,
    pub max_health: f32,
    pub being_built: bool,
    /// Command queue is empty.
    pub idle: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AllyUnit {
    pub id: UnitId,
    pub def: UnitDefId,
    pub pos: Vec3,
    pub team: i32,
    pub being_built: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnemyUnit {
    pub id: UnitId,
    /// Unknown for radar-only contacts.
    pub def: Option<UnitDefId>,
    pub pos: Vec3,
    pub health: f32,
    /// Whose it is, when the engine tells (it does for units in sight).
    pub team: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    UnitCreated { unit: UnitId, builder: Option<UnitId> },
    UnitFinished { unit: UnitId },
    UnitIdle { unit: UnitId },
    UnitMoveFailed { unit: UnitId },
    UnitDamaged { unit: UnitId, attacker: Option<UnitId>, damage: f32 },
    UnitDestroyed { unit: UnitId, attacker: Option<UnitId> },
    EnemyEnterLos { enemy: UnitId },
    EnemyLeaveLos { enemy: UnitId },
    EnemyDestroyed { enemy: UnitId },
    /// A [`Command::Build`] was dropped because the shim found no legal site.
    BuildSiteNotFound { unit: UnitId, def: UnitDefId },
    /// The engine rejected a command.
    CommandRejected { unit: UnitId, code: i32 },
}

/// The bot's reply to a tick; also the credit that lets the shim send the next one.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Commands(pub Vec<Command>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    /// `site` is `None` for factories, which build in place and always append to their build
    /// queue; `queue` is ignored for them (the engine reads that option bit as "build five").
    Build { unit: UnitId, def: UnitDefId, site: Option<BuildSite>, queue: bool },
    Move { unit: UnitId, to: Vec3, queue: bool },
    Fight { unit: UnitId, to: Vec3, queue: bool },
    Stop { unit: UnitId },
    SetRepeat { unit: UnitId, repeat: bool },
    /// Follow and help `target`: a builder guarding a factory adds its build power to whatever the factory makes.
    Guard { unit: UnitId, target: UnitId },
    /// Reclaim every wreck and rock within `radius` of `centre` for their metal.
    ReclaimArea { unit: UnitId, centre: Vec3, radius: f32, queue: bool },
    /// Take one feature apart for its metal.
    ReclaimFeature { unit: UnitId, feature: FeatureId, queue: bool },
    /// Raise the unit a wreck was (resurrection bots only); it costs energy and time, no metal.
    Resurrect { unit: UnitId, feature: FeatureId, queue: bool },
    /// Restore `target`'s health (a builder's job; it also finishes a stalled construction).
    Repair { unit: UnitId, target: UnitId, queue: bool },
    /// Cheat: a finished unit of type `def` appears at `at`, owned by this AI's team. For the duel harness
    /// (`docs/harness/duels.md`); the engine honours it only in a game hosted locally with a single player.
    GiveUnit { def: UnitDefId, at: Vec3 },
    /// Starts the unit's self-destruct countdown (a second order cancels it).
    SelfDestruct { unit: UnitId },
}

/// The shim resolves this to the closest legal build position, since only it can query the map.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BuildSite {
    pub near: Vec3,
    pub search_radius: f32,
    /// Minimum gap to other buildings, in build squares.
    pub min_dist: i32,
}
