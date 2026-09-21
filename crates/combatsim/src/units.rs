//! The unit table: what a unit is made of and what its weapons do, derived from the game's own definition files by
//! `tools/extract_units.py`. Everything here is the engine's number, unchanged; the simulation's own assumptions
//! live in `sim.rs` so that a wrong result can be blamed on one or the other.

use std::collections::HashMap;

use serde::Deserialize;

/// Engine frames per second. Reloads, flight and the whole simulation are counted in frames, so a run is exactly
/// reproducible and the engine's own whole-frame reload rounding carries over.
pub const FPS: f32 = 30.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum Kind {
    Cannon,
    MissileLauncher,
    StarburstLauncher,
    BeamLaser,
    LaserCannon,
    LightningCannon,
    Flame,
    DGun,
    #[serde(other)]
    Other,
}

impl Kind {
    /// Hitscan weapons reach the target the frame they are fired and cannot be dodged.
    pub fn instant(self) -> bool {
        matches!(self, Kind::BeamLaser | Kind::LightningCannon | Kind::LaserCannon)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Weapon {
    pub kind: Kind,
    pub range: f32,
    /// Seconds between the starts of two salvos.
    pub reload: f32,
    /// Projectiles in a salvo, and the seconds between them.
    pub burst: u32,
    pub burst_rate: f32,
    pub projectiles: u32,
    /// Damage by armour class; `default` is the fallback the standard class uses.
    pub damage: HashMap<String, f32>,
    pub aoe: f32,
    pub edge: f32,
    pub velocity: f32,
    pub start_velocity: f32,
    pub acceleration: f32,
    pub tracks: bool,
    /// Aim error in the engine's raw units, turned into an angle by `aim_error`; `accuracy` is drawn once a
    /// salvo, `spray` once a projectile.
    pub accuracy: f32,
    pub spray: f32,
    /// 0 (the engine's default) means the shot leads a moving target by 0 to 2x the right amount; 1 means exactly.
    pub predict_boost: f32,
    /// Elmos of lead the weapon will not exceed; negative is unlimited.
    pub lead_limit: f32,
    pub energy_per_shot: f32,
    pub only_targets: String,
    /// Ways a weapon takes no part in a stand-up land fight. The simulation has no stun, no stockpile, no water
    /// and nobody to press the D-Gun button, so each of these would otherwise be scored as ordinary damage.
    #[serde(default)]
    pub paralyzer: bool,
    #[serde(default)]
    pub stockpile: bool,
    #[serde(default)]
    pub water_only: bool,
    #[serde(default)]
    pub command_fire: bool,
}

/// The engine's own conversion from a weapondef's aim-error number to an angle, `WeaponDef.cpp`'s `AccuracyToSin`.
pub fn aim_error(raw: f32) -> f32 {
    (raw * std::f32::consts::PI / 45055.0).sin()
}

impl Weapon {
    /// Damage to a unit of this armour class.
    pub fn damage_to(&self, armor: &str) -> f32 {
        self.damage.get(armor).or_else(|| self.damage.get("default")).copied().unwrap_or(0.0)
    }

    /// Whether this weapon fights in a dry land battle at all. Anti-air (`onlytargetcategory` VTOL) does not
    /// exist as far as a land fight is concerned; nor do paralysers (no stun model), stockpiled launchers
    /// (nothing built to fire), underwater weapons, or the manually fired D-Gun.
    pub fn hits_ground(&self) -> bool {
        self.only_targets != "VTOL"
            && !self.paralyzer
            && !self.stockpile
            && !self.water_only
            && !self.command_fire
            && self.damage_to("standard") > 0.0
    }

    /// Seconds for a shot to cover `distance`. Missiles start slow and accelerate to `velocity`; everything else
    /// flies at `velocity` (the engine's ballistic arc is longer than the straight line, which this ignores).
    pub fn flight_time(&self, distance: f32) -> f32 {
        if self.kind.instant() || self.velocity <= 0.0 {
            return 0.0;
        }
        if self.acceleration > 0.0 && self.start_velocity > 0.0 && self.start_velocity < self.velocity {
            // Accelerate from `start_velocity` to `velocity`, then cruise.
            let ramp = (self.velocity - self.start_velocity) / self.acceleration;
            let covered = (self.start_velocity + self.velocity) / 2.0 * ramp;
            if distance > covered {
                return ramp + (distance - covered) / self.velocity;
            }
            let a = self.acceleration;
            return ((self.start_velocity * self.start_velocity + 2.0 * a * distance).sqrt() - self.start_velocity) / a;
        }
        distance / self.velocity
    }
}

fn one() -> u32 {
    1
}

#[derive(Clone, Debug, Deserialize)]
pub struct Unit {
    pub metal: f32,
    pub energy: f32,
    pub health: f32,
    /// Elmos per second.
    pub speed: f32,
    pub sight: f32,
    /// Collision radius, from the unit's collision volume: a shot that lands within it hits the unit, and two
    /// units cannot get closer than the sum of two of these.
    pub radius: f32,
    pub armor: String,
    pub air: bool,
    pub builder: bool,
    /// The unit file's `customparams.techlevel`: 1 unless the file says otherwise.
    #[serde(default = "one")]
    pub tech: u32,
    /// The steepest ground this unit's movement class can stand on, in the engine's slope units (the terrain
    /// grid's own scale), and the deepest water it can wade. Both come from `gamedata/movedefs.lua`, not from the
    /// unit file's legacy `maxslope`.
    pub max_slope: i32,
    pub max_depth: f32,
    pub weapons: Vec<Weapon>,
}

impl Unit {
    pub fn mobile(&self) -> bool {
        self.speed > 0.0
    }

    /// Longest reach against ground; 0 for a unit that cannot fight one.
    pub fn reach(&self) -> f32 {
        self.weapons.iter().filter(|w| w.hits_ground()).map(|w| w.range).fold(0.0, f32::max)
    }

    /// Damage a second against a standard-armour ground unit with every ground weapon firing as fast as it reloads,
    /// every shot landing: the weight a threat map gives this unit.
    pub fn dps(&self) -> f32 {
        self.weapons
            .iter()
            .filter(|w| w.hits_ground() && w.reload > 0.0)
            .map(|w| w.damage_to("standard") * w.burst.max(1) as f32 * w.projectiles.max(1) as f32 / w.reload)
            .sum()
    }
}

#[derive(Deserialize)]
struct Table {
    units: HashMap<String, Unit>,
}

/// Every unit type the simulation knows, indexed for the hot loop.
pub struct Units {
    pub list: Vec<Unit>,
    pub names: Vec<String>,
    by_name: HashMap<String, usize>,
}

impl Default for Units {
    fn default() -> Self {
        Units::load(include_str!("../data/units.json"))
    }
}

impl Units {
    pub fn load(json: &str) -> Units {
        let table: Table = serde_json::from_str(json).expect("data/units.json");
        let mut names: Vec<String> = table.units.keys().cloned().collect();
        names.sort();
        let list = names.iter().map(|n| table.units[n].clone()).collect();
        let by_name = names.iter().enumerate().map(|(i, n)| (n.clone(), i)).collect();
        Units { list, names, by_name }
    }

    pub fn index(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    pub fn get(&self, name: &str) -> &Unit {
        &self.list[self.index(name).unwrap_or_else(|| panic!("unit {name} is not in data/units.json"))]
    }
}
