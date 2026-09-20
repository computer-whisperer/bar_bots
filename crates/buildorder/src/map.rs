//! Quicksilver Remake 1.24: metal spots and the two corner starts.
//!
//! The spot list is what the engine reports to our bot (the `metal_spots` of a match record's header, e.g.
//! `run/matches/1789866799-v15-terrain-medium/00/record-0.jsonl`). Every spot we have seen an extractor finish on paid
//! 2.0 metal/s (income steps in 12 recorded games, home spots exactly 2.0 in 18 of 18), so the simulator uses that
//! constant rather than reading the map's metal image.

pub const SPOT_METAL: f64 = 2.0;
pub const WIND_MIN: f64 = 3.0;
pub const WIND_MAX: f64 = 17.0;
/// Mean wind per turbine inferred from energy income over 12 recorded games (12.8); the map site lists 12.7.
pub const WIND_MEAN: f64 = 12.7;

pub const NW_HOME: (f64, f64) = (2032.0, 1188.0);
pub const SE_HOME: (f64, f64) = (5136.0, 5980.0);

pub const METAL_SPOTS: [(f64, f64); 44] = [
    (3224.0, 520.0), (6184.0, 520.0), (2152.0, 808.0), (6632.0, 984.0), (1992.0, 1000.0), (5096.0, 1336.0),
    (4296.0, 1416.0), (4600.0, 1672.0), (3608.0, 1880.0), (5848.0, 2040.0), (3864.0, 2088.0), (2136.0, 2136.0),
    (4136.0, 2216.0), (2312.0, 2344.0), (4056.0, 2776.0), (1352.0, 2808.0), (2936.0, 2808.0), (5224.0, 2808.0),
    (2296.0, 2936.0), (6056.0, 3208.0), (3336.0, 3256.0), (4056.0, 3624.0), (1240.0, 3768.0), (3352.0, 3864.0),
    (2328.0, 4200.0), (4168.0, 4264.0), (5336.0, 4536.0), (1272.0, 4760.0), (3352.0, 4840.0), (4520.0, 4920.0),
    (5976.0, 4968.0), (5448.0, 5240.0), (5640.0, 5352.0), (3320.0, 5368.0), (2440.0, 5448.0), (1752.0, 5576.0),
    (3352.0, 5704.0), (2456.0, 5816.0), (3656.0, 5816.0), (6216.0, 6088.0), (504.0, 6184.0), (6072.0, 6248.0),
    (936.0, 6664.0), (4632.0, 6664.0),
];

pub fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// The spots nearer to `home` than to `enemy`, nearest first: the ones a build order may count on.
pub fn own_half(home: (f64, f64), enemy: (f64, f64)) -> Vec<(f64, f64)> {
    let mut spots: Vec<_> = METAL_SPOTS.iter().copied().filter(|s| distance(*s, home) < distance(*s, enemy)).collect();
    spots.sort_by(|a, b| distance(*a, home).total_cmp(&distance(*b, home)));
    spots
}
