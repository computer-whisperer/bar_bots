//! "Who wins if this force meets that one here?" — a fight simulated from the game's own unit numbers instead of
//! looked up in a table of duels. The matchup table the brain uses (`crates/bot/src/brain/combat.rs`) knows one
//! situation: two single-type blobs of equal metal charging each other on flat ground. This knows range, speed,
//! arrival order, turrets, mixed forces and the shape of the ground, at the cost of being a model.
//!
//! What it is judged by: the duel tables in `docs/data/duels-2026-09-19/`, replayed by `combatsim validate`.
//! `docs/studies/combat-sim.md` has the numbers and what it still gets wrong.

pub mod duels;
pub mod field;
pub mod rng;
pub mod scenario;
pub mod sim;
pub mod units;

pub use field::Field;
pub use scenario::{End, Focus, Group, Micro, Odds, Outcome, Scenario, Vec2};
pub use sim::{Rules, Tuning, odds, simulate};
pub use units::Units;
