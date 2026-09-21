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
    /// The builder's class's cost a cell: slopes priced by the engine's own law (`terrain::costs`), so that a plan
    /// walks the commander over its own ground at its own pace (routing design, 2026-09-20).
    costs: Vec<u32>,
    fields: Mutex<HashMap<(i32, i32), Option<Arc<Field>>>>,
}

/// Where no way on foot is found the builder is charged this many straight lines, not eternity: a plan stuck for
/// good on one bad site would tell the search nothing about the rest of it.
const NO_WAY_DETOUR: f64 = 3.0;

impl Walked {
    /// `None` without terrain data.
    pub fn new(terrain: &Terrain, class: MoveClass) -> Option<Walked> {
        let costs = terrain::costs(terrain, class);
        (terrain.width > 0 && costs.len() == (terrain.width * terrain.height) as usize)
            .then(|| Walked { terrain: terrain.clone(), costs, fields: Mutex::default() })
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
        let field = self.fields.lock().unwrap().entry(key).or_insert_with(|| Field::from_costs(&self.terrain, &self.costs, &[at(to)]).map(Arc::new)).clone();
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
    /// A wind to price plans at instead of the engine's process mean (the bot's H-OPEN-WIND off: the middle of the range).
    pub wind_override: Option<f64>,
    pub terrain: Terrain,
}

/// The mean wind the engine's law produces between `min` and `max`: every 15 s the wind vector takes a step of up to
/// half the maximum on each axis and its length is clamped to the bounds (`rts/Sim/Misc/Wind.cpp`), a clamped random
/// walk that sits well above the middle of the range (Quicksilver 3-17: 12.7, measured 12.8 in games, against 10 for the
/// middle). Simulated here with a fixed seed; it decorrelates within a minute, so what a game shows early predicts
/// nothing about the rest (correlation 0.09 between the first two minutes and the next eight), and the game average
/// spreads only from 10.5 to 13.9 (10th to 90th percentile). The blend between updates is taken as its mean.
pub fn process_mean_wind(min: f64, max: f64) -> f64 {
    if max <= 0.0 {
        return 0.0;
    }
    // A small linear congruential generator: the same answer every time, no dependency.
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut x, mut z) = (0.0f64, 0.0f64);
    let (mut sum, mut n) = (0.0, 0);
    for _ in 0..20_000 {
        let (ox, oz) = (x, z);
        let mut s;
        loop {
            x -= (next() - 0.5) * max;
            z -= (next() - 0.5) * max;
            s = x.hypot(z);
            if s > 0.0 {
                break;
            }
        }
        let clamped = s.clamp(min, max);
        (x, z) = (x / s * clamped, z / s * clamped);
        for i in 0..15 {
            let t = (i as f64 + 0.5) / 15.0;
            let m = t * t * (3.0 - 2.0 * t);
            let (cx, cz) = (ox + (x - ox) * m, oz + (z - oz) * m);
            sum += cx.hypot(cz).clamp(min, max);
            n += 1;
        }
    }
    sum / n as f64
}

impl Game {
    /// The wind a plan is priced at: the mean of the engine's law for this map's bounds (`process_mean_wind`).
    pub fn mean_wind(&self) -> f64 {
        self.wind_override.unwrap_or_else(|| process_mean_wind(self.wind.0, self.wind.1))
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

#[cfg(test)]
mod wind_tests {
    use super::process_mean_wind;

    #[test]
    fn the_engines_wind_sits_above_the_middle_of_the_range() {
        let quicksilver = process_mean_wind(3.0, 17.0);
        assert!((12.2..13.2).contains(&quicksilver), "{quicksilver}");
        let comet = process_mean_wind(1.0, 4.0);
        assert!((2.6..3.4).contains(&comet), "{comet}");
        assert_eq!(process_mean_wind(0.0, 0.0), 0.0);
    }
}
