//! A crude simulator of Beyond All Reason's tier-1 economy and a simulated-annealing search for build orders over it.
//! A study tool: see `docs/studies/build-order.md` for what it models, what it ignores and how far off it is.

pub mod anneal;
pub mod game;
pub mod plan;
pub mod record;
pub mod sim;
pub mod units;
