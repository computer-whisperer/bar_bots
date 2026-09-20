//! The ground a build order is played on: where metal is and how far a builder walks.

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use bot_protocol::{MoveClass, Terrain, Vec3};
use terrain::Field;

use crate::sim::Scenario;
use crate::units::{Role, Units};

pub fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub at: (f64, f64),
    /// Metal per second the scenario's extractor draws from it.
    pub metal: f64,
}

/// Walking distance in elmos between two points, for a builder on foot.
pub trait Ground: Debug + Send + Sync {
    fn walk(&self, from: (f64, f64), to: (f64, f64)) -> f64;
}

/// Flat open ground: the straight line times `detour`. 1.05 fits 565 builder trips in 12 recorded games on Quicksilver
/// (walks over 1000 elmos took 8 % longer than the straight line).
#[derive(Clone, Copy, Debug)]
pub struct Straight {
    pub detour: f64,
}

impl Ground for Straight {
    fn walk(&self, from: (f64, f64), to: (f64, f64)) -> f64 {
        distance(from, to) * self.detour
    }
}

/// The map's own ground for one movement class: distances on foot, one field per place walked to (few: metal spots
/// and base sites), made when first asked for.
pub struct Walked {
    terrain: Terrain,
    passable: Vec<bool>,
    fields: Mutex<HashMap<(i32, i32), Option<Arc<Field>>>>,
}

/// Where no way on foot is found the builder is charged this many straight lines, not eternity: a plan stuck for
/// good on one bad site would tell the search nothing about the rest of it.
const NO_WAY_DETOUR: f64 = 3.0;

impl Walked {
    /// `None` without terrain data.
    pub fn new(terrain: &Terrain, class: MoveClass) -> Option<Walked> {
        let passable = terrain::passable(terrain, class);
        (terrain.width > 0 && passable.len() == (terrain.width * terrain.height) as usize)
            .then(|| Walked { terrain: terrain.clone(), passable, fields: Mutex::default() })
    }
}

impl Debug for Walked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Walked({}x{} cells)", self.terrain.width, self.terrain.height)
    }
}

impl Ground for Walked {
    fn walk(&self, from: (f64, f64), to: (f64, f64)) -> f64 {
        let at = |p: (f64, f64)| Vec3 { x: p.0 as f32, y: 0.0, z: p.1 as f32 };
        let key = ((to.0 / self.terrain.cell as f64) as i32, (to.1 / self.terrain.cell as f64) as i32);
        let field = self.fields.lock().unwrap().entry(key).or_insert_with(|| Field::from(&self.terrain, &self.passable, at(to)).map(Arc::new)).clone();
        field.and_then(|f| f.distance(at(from))).map_or(distance(from, to) * NO_WAY_DETOUR, |d| (d as f64).max(distance(from, to)))
    }
}

/// One game as a build order sees it: built from the bot's `Hello` in a game, from a match record's header offline.
pub struct Game {
    pub units: Units,
    /// Our commander's unit type.
    pub commander: usize,
    pub home: (f64, f64),
    /// Map extent in elmos.
    pub size: (f64, f64),
    /// Every metal spot with the amount the engine reports for it.
    pub spots: Vec<((f64, f64), f64)>,
    pub wind: (f64, f64),
    pub terrain: Terrain,
}

impl Game {
    /// Mean wind: the engine draws the wind speed evenly between the map's bounds.
    pub fn mean_wind(&self) -> f64 {
        (self.wind.0 + self.wind.1) / 2.0
    }

    /// What our tier-1 extractor draws from a spot of this amount.
    pub fn spot_metal(&self, amount: f64) -> f64 {
        let extractor = self.units.extractor(self.commander).expect("the commander builds an extractor");
        amount * self.units.list[extractor].extracts_metal
    }

    /// The spots nearer to `home` than to its mirror image through the map's centre (where a lone opponent starts on
    /// a two-player map), nearest first: the ones a build order may count on when nothing more is known.
    pub fn own_half(&self) -> Vec<Spot> {
        let mirror = (self.size.0 - self.home.0, self.size.1 - self.home.1);
        let mut spots: Vec<Spot> = self
            .spots
            .iter()
            .filter(|(at, _)| distance(*at, self.home) < distance(*at, mirror))
            .map(|(at, amount)| Spot { at: *at, metal: self.spot_metal(*amount) })
            .collect();
        spots.sort_by(|a, b| distance(a.at, self.home).total_cmp(&distance(b.at, self.home)));
        spots
    }

    /// The map's ground as the commander walks it (constructors are charged the same ways: a vehicle's differ
    /// where slopes are steep); open ground when the game came without terrain data.
    pub fn ground(&self) -> Arc<dyn Ground> {
        match self.units.list[self.commander].move_class.and_then(|class| Walked::new(&self.terrain, class)) {
            Some(walked) => Arc::new(walked),
            None => Arc::new(Straight { detour: 1.05 }),
        }
    }

    pub fn scenario(&self, spots: Vec<Spot>, ground: Arc<dyn Ground>) -> Scenario {
        Scenario::new(self.commander, self.home, spots, ground, self.mean_wind())
    }

    /// The factory of this name our commander builds (`lab`, `vp`: the name without the faction prefix).
    pub fn factory(&self, name: &str) -> Option<usize> {
        let units = &self.units;
        units.list[self.commander].builds.iter().copied().find(|u| units.list[*u].role == Role::Factory && units.list[*u].name.get(3..) == Some(name))
    }

    /// Faction prefix of our units' names.
    pub fn side(&self) -> &str {
        &self.units.list[self.commander].name[..3]
    }
}
