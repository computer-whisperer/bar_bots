//! Where the commander starts: the first genuine game input (the user's ruling, 2026-09-20,
//! `docs/design/2026-09-20-opening-search.md`). A human places an AI through the lobby, so in a game with people the
//! start is **forced** and the search plans from it; the arena can place us, so there the start is **open** and the
//! search chooses it inside the box: the point from which the best opening plan scores best. No walk is added beyond
//! what a plan's own steps say.

use std::sync::Arc;
use std::time::Duration;

use crate::anneal::{anneal_within, Found, Palette, Search};
use crate::game::{distance, Game, Ground, Spot};
use crate::sim::{Scenario, State};

/// A start box, in elmos.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Rect {
    pub fn contains(&self, p: (f64, f64)) -> bool {
        p.0 >= self.left && p.0 <= self.right && p.1 >= self.top && p.1 <= self.bottom
    }

    pub fn centre(&self) -> (f64, f64) {
        ((self.left + self.right) / 2.0, (self.top + self.bottom) / 2.0)
    }

    fn clamp(&self, p: (f64, f64)) -> (f64, f64) {
        (p.0.clamp(self.left, self.right), p.1.clamp(self.top, self.bottom))
    }
}

/// The starts worth trying inside `rect`: beside each metal spot in it, between each pair of spots the commander could
/// reach both of from one place (the experienced player's choice: two extractors without a step), and the box's
/// centre. Reach is the commander's build distance plus an extractor's half-footprint.
pub fn candidates(game: &Game, rect: Rect) -> Vec<(f64, f64)> {
    let reach = game.units.list[game.commander].build_distance + 60.0;
    let spots: Vec<(f64, f64)> = game.spots.iter().map(|(at, _)| *at).filter(|at| rect.contains(*at)).collect();
    let mut points: Vec<(f64, f64)> = vec![rect.centre()];
    for (i, a) in spots.iter().enumerate() {
        // Beside the spot, not on it: the extractor goes there.
        points.push(rect.clamp((a.0, a.1 + reach * 0.8)));
        for b in &spots[i + 1..] {
            if distance(*a, *b) <= 2.0 * reach {
                points.push(rect.clamp(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0)));
            }
        }
    }
    points.dedup_by(|p, q| distance(*p, *q) < 30.0);
    // The user's rule of thumb (2026-09-20): in most games the right place is within reach of two extractor spots
    // with a short walk to a third. Ranked by spots in reach (more first), then by the walk to the next one.
    let all: Vec<(f64, f64)> = game.spots.iter().map(|(at, _)| *at).collect();
    let rank = |p: &(f64, f64)| {
        let mut walks: Vec<f64> = all.iter().map(|s| distance(*p, *s)).collect();
        walks.sort_by(f64::total_cmp);
        let in_reach = walks.iter().filter(|w| **w <= reach).count();
        (std::cmp::Reverse(in_reach), walks.get(in_reach).copied().unwrap_or(f64::MAX))
    };
    points.sort_by(|a, b| rank(a).partial_cmp(&rank(b)).unwrap_or(std::cmp::Ordering::Equal));
    points.truncate(MOST_CANDIDATES);
    points
}

/// Starts the search spends its budget on, the best-ranked first.
pub const MOST_CANDIDATES: usize = 6;

/// The best start inside `rect` and the opening found from it: `budget` split over the candidates, `scenario_for`
/// building the scenario each is judged in (spots, ground, leash) from the candidate start.
pub fn choose(
    game: &Game,
    rect: Rect,
    palette: &Palette,
    search: &Search,
    budget: Duration,
    threads: usize,
    scenario_for: &dyn Fn((f64, f64)) -> Scenario,
) -> ((f64, f64), Found) {
    let points = candidates(game, rect);
    let each = budget / points.len().max(1) as u32;
    let mut best: Option<((f64, f64), Found)> = None;
    for start in points {
        let scenario = scenario_for(start);
        let found = anneal_within(&game.units, &scenario, &State::start(&scenario), palette, search, each, threads);
        let at = |minute: f64| found.outcome.samples.iter().find(|s| s.t == minute * 60.0).map_or((0, 0.0, 0.0), |s| (s.extractors, s.metal_income, s.army_value));
        eprintln!("candidate start ({:.0}, {:.0}): score {:.0}; predicted extractors / metal per s / army metal at 5 and 10 min: {:?} {:?}", start.0, start.1, found.score, at(5.0), at(10.0));
        if best.as_ref().is_none_or(|(_, b)| found.score > b.score) {
            best = Some((start, found));
        }
    }
    best.expect("a start box has at least its centre")
}

/// The scenario for a start: the game's, with the commander standing at `start`.
pub fn scenario_from(game: &Game, start: (f64, f64), spots: Vec<Spot>, ground: Arc<dyn Ground>) -> Scenario {
    Scenario::new(game.commander, start, spots, ground, game.mean_wind())
}
