//! The other teams on our side: a human, another AI, or another seat of ours. We see their units and never order them.
//!
//! H-TEAM-ALLIED-SPOTS: an allied extractor holds its spot. H-TEAM-ALLY-GROUND: metal nearer to an ally's start than
//! to ours is theirs to take first; we build there only once it has stood empty for [`ALLY_GROUND_FRAMES`] (an ally
//! expands where it likes, and we do not race it at its own door). H-TEAM-ALLIED-COVER: allied soldiers and turrets
//! hold ground as ours do (`territory.rs`), and allied soldiers in a fight are counted into its odds.
//!
//! Seats of ours also talk through the team board (`crate::team`): H-TEAM-BOARD pools spot claims and enemy buildings,
//! H-TEAM-WAVES makes them attack one target and weigh a wave with the others' soldiers beside it.

use bot_protocol::{Tick, UnitDefId, Vec3};

use super::FRAMES_PER_SECOND;

const ALLY_GROUND_FRAMES: i32 = 3 * 60 * FRAMES_PER_SECOND;
/// An extractor this close to a spot is on it (as `economy.rs` has it for our own).
const ON_SPOT: f32 = 100.0;

impl super::Brain {
    pub(super) fn is_commander_def(&self, def: UnitDefId) -> bool {
        self.world.def(def).is_some_and(|d| d.name.ends_with("com") && d.speed > 0.0 && d.build_speed > 0.0)
    }

    pub(super) fn note_allies(&mut self, tick: &Tick) {
        self.allies = tick.snapshot.allies.clone();
        for ally in &tick.snapshot.allies {
            if !self.ally_starts.contains_key(&ally.team) && self.is_commander_def(ally.def) {
                eprintln!("[ai {}] f={} ally team {} starts at ({:.0}, {:.0})", self.ai(), tick.frame, ally.team, ally.pos.x, ally.pos.z);
                self.ally_starts.insert(ally.team, ally.pos);
            }
        }
        let held: Vec<usize> = (0..self.world.hello.metal_spots.len()).filter(|i| self.allied_extractor_on(self.world.hello.metal_spots[*i])).collect();
        for index in held {
            self.ally_spot_held.insert(index, tick.frame);
        }
    }

    pub(super) fn allied_extractor_on(&self, spot: Vec3) -> bool {
        self.allies.iter().any(|a| a.pos.dist2d(spot) < ON_SPOT.max(self.spot_occupied_radius()) && self.world.def(a.def).is_some_and(|d| d.extracts_metal > 0.0))
    }

    /// False for a spot at an ally's door that the ally has not had three minutes to take (or retake).
    pub(super) fn spot_open_to_us(&self, index: usize, spot: Vec3, frame: i32) -> bool {
        let theirs = self.enabled("H-TEAM-ALLY-GROUND") && self.ally_starts.values().any(|start| start.dist2d(spot) < self.home.dist2d(spot));
        !theirs || frame - self.ally_spot_held.get(&index).copied().unwrap_or(0) > ALLY_GROUND_FRAMES
    }

    /// Posts this seat's tick to the team board and reads the other seats'.
    pub(super) fn exchange_with_team(&mut self, tick: &Tick) {
        let mut post = std::mem::take(&mut self.team_post);
        self.team_post.launched = post.launched;
        post.frame = tick.frame;
        post.spot_claims = self.spot_claims.keys().copied().collect();
        self.team_mates = if self.enabled("H-TEAM-BOARD") { self.board.exchange(self.world.hello.team, post) } else { Default::default() };
    }

    /// The types of the allied soldiers within `radius` of `pos`.
    pub(super) fn allied_soldiers_near(&self, pos: Vec3, radius: f32) -> Vec<UnitDefId> {
        let soldier = |def: UnitDefId| self.world.def(def).is_some_and(|d| d.weapon_count > 0 && d.speed > 0.0 && d.build_speed == 0.0);
        self.allies.iter().filter(|a| !a.being_built && a.pos.dist2d(pos) < radius && soldier(a.def)).map(|a| a.def).collect()
    }
}
