//! Which duels to run: pairings, repetitions, and how many units make a fair fight.

/// One fight to run: `x` against `y`. The repetition number decides who stands where.
#[derive(Clone, Debug)]
pub struct Job {
    pub x: String,
    pub y: String,
    pub rep: u32,
    /// Times this job was handed out before (a match that aborts gives its running duels back).
    pub attempts: u32,
}

impl Job {
    /// Even repetitions put `x` at the west end, odd ones at the east end.
    pub fn x_is_west(&self) -> bool {
        self.rep.is_multiple_of(2)
    }

    /// Repetitions 0-1 give `x` to team 0, 2-3 to team 1, and so on.
    pub fn x_team(&self) -> i32 {
        ((self.rep / 2) % 2) as i32
    }
}

/// Every unordered pair from `units`, a unit against itself included.
pub fn all_pairs(units: &[String]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for (i, x) in units.iter().enumerate() {
        for y in &units[i..] {
            pairs.push((x.clone(), y.clone()));
        }
    }
    pairs
}

/// Every `ours` against every `theirs`, without repeating a pair that appears both ways round.
pub fn cross(ours: &[String], theirs: &[String]) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for x in ours {
        for y in theirs {
            if !pairs.iter().any(|(a, b)| (a == x && b == y) || (a == y && b == x)) {
                pairs.push((x.clone(), y.clone()));
            }
        }
    }
    pairs
}

/// `reps` jobs per pair, in an order that spreads a pair's repetitions over sites and matches.
pub fn jobs(pairs: &[(String, String)], reps: u32) -> Vec<Job> {
    let mut jobs: Vec<Job> = (0..reps)
        .flat_map(|rep| pairs.iter().map(move |(x, y)| Job { x: x.clone(), y: y.clone(), rep, attempts: 0 }))
        .collect();
    // A fixed shuffle (the same plan every run): long fights and short ones mix, so sites stay evenly loaded.
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    for i in (1..jobs.len()).rev() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        jobs.swap(i, (state >> 33) as usize % (i + 1));
    }
    jobs
}

/// How many of each unit fight.
#[derive(Clone, Copy, Debug)]
pub enum Sizing {
    /// About this much metal a side, the two sides' totals as close as whole units allow.
    EqualMetal(f32),
    EqualCount(u32),
}

impl Sizing {
    pub fn counts(self, cost_x: f32, cost_y: f32) -> (u32, u32) {
        match self {
            Sizing::EqualCount(n) => (n, n),
            Sizing::EqualMetal(budget) => {
                // Among armies worth 60-125% of the budget, take the one whose two totals differ least (in steps
                // of 2%); on a tie, the one nearest the budget. A unit dearer than that fights alone.
                let mut best = (1, (cost_x / cost_y).round().max(1.0) as u32);
                let mut best_score = (f32::INFINITY, f32::INFINITY);
                for n in 1..=400u32 {
                    let value = n as f32 * cost_x;
                    let m = (value / cost_y).round().max(1.0);
                    let other = m * cost_y;
                    if value.min(other) < 0.6 * budget || value.max(other) > 1.25 * budget {
                        continue;
                    }
                    let mismatch = ((value - other).abs() / value.max(other) * 50.0).round();
                    let score = (mismatch, (value - budget).abs());
                    if score < best_score {
                        best_score = score;
                        best = (n, m as u32);
                    }
                }
                best
            }
        }
    }
}
