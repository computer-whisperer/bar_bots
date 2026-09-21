//! The control lane: what a unit does with its order between the brain's decisions, every tick
//! (`docs/design/2026-09-20-micro-lane.md`). Part 1 of the arc: nothing yet.

use bot_protocol::{Command, Tick};

use super::Brain;

impl Brain {
    pub(super) fn micro(&mut self, _tick: &Tick) -> Vec<Command> {
        Vec::new()
    }
}
