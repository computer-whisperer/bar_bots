//! The team board: what the seats we play on one ally team of one game tell each other. Each seat is its own session
//! with its own brain; without the board two of them walk to the same free metal spot, know different halves of the
//! enemy, and send two half-armies at two targets.
//!
//! The board holds facts, not decisions: each brain posts its own and reads the others' on its tick.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};

use bot_protocol::{Hello, UnitDefId, UnitId, Vec3};

/// A seat's post older than this is not counted: the seat is dead or its session gone.
const FRESH_FRAMES: i32 = 90;

/// An enemy building seen and not known destroyed: type, place, frame last seen.
pub type Building = (UnitDefId, Vec3, i32);

#[derive(Default)]
pub struct TeamBoard {
    inner: Mutex<Board>,
}

#[derive(Default)]
struct Board {
    seats: BTreeMap<i32, Post>,
    buildings: HashMap<UnitId, Building>,
}

/// What one seat says of itself, renewed every tick.
#[derive(Clone, Default)]
pub struct Post {
    pub frame: i32,
    /// Metal spots (indices into the map's list) its builders are on their way to.
    pub spot_claims: HashSet<usize>,
    /// Where its army would go, or is going.
    pub target: Option<Vec3>,
    /// Soldiers it would send now if the odds allowed (empty unless a wave is ready and its home is quiet).
    pub offer: Vec<UnitDefId>,
    /// Soldiers it has out attacking.
    pub committed: Vec<UnitDefId>,
    /// When it last launched a wave.
    pub launched: Option<i32>,
}

/// What the other seats say, as one seat reads it.
#[derive(Default)]
pub struct Others {
    pub spot_claims: HashSet<usize>,
    /// The target of the lowest-numbered other seat that has one and outranks the reader (a lower team number).
    pub lead_target: Option<Vec3>,
    /// Every other seat's offered and committed soldiers.
    pub with_us: Vec<UnitDefId>,
    /// The committed ones alone: out there with our attackers.
    pub attacking: Vec<UnitDefId>,
    /// The latest wave launch by another seat.
    pub launched: Option<i32>,
}

static BOARDS: Mutex<Vec<(u64, i32, Weak<TeamBoard>)>> = Mutex::new(Vec::new());

impl TeamBoard {
    /// The board of this seat's game and ally team, shared with every other session of this process that plays it.
    pub fn of(hello: &Hello) -> Arc<TeamBoard> {
        let mut boards = BOARDS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        boards.retain(|(_, _, board)| board.strong_count() > 0);
        if let Some(board) = boards.iter().find(|(game, ally, _)| (*game, *ally) == (hello.game_id, hello.ally_team)).and_then(|(_, _, b)| b.upgrade()) {
            return board;
        }
        let board = Arc::new(TeamBoard::default());
        boards.push((hello.game_id, hello.ally_team, Arc::downgrade(&board)));
        board
    }

    /// Posts `team`'s state and returns what the other live seats have posted.
    pub fn exchange(&self, team: i32, post: Post) -> Others {
        let mut board = self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let frame = post.frame;
        board.seats.insert(team, post);
        let mut others = Others::default();
        for (seat, post) in board.seats.iter().filter(|(seat, post)| **seat != team && frame - post.frame < FRESH_FRAMES) {
            others.spot_claims.extend(&post.spot_claims);
            if *seat < team && others.lead_target.is_none() {
                others.lead_target = post.target;
            }
            others.with_us.extend(post.offer.iter().chain(&post.committed));
            others.attacking.extend(&post.committed);
            others.launched = others.launched.max(post.launched);
        }
        others
    }

    /// Pools enemy buildings: `seen` are this seat's sightings of this tick, `gone` what it knows destroyed or razed.
    /// Returns everything the team remembers.
    pub fn pool_buildings(&self, seen: &[(UnitId, Building)], gone: &[UnitId]) -> HashMap<UnitId, Building> {
        let mut board = self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for id in gone {
            board.buildings.remove(id);
        }
        board.buildings.extend(seen.iter().copied());
        board.buildings.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seat_reads_the_others_and_follows_only_a_lower_seats_target() {
        let board = TeamBoard::default();
        let at = |x: f32| Vec3 { x, y: 0.0, z: 0.0 };
        let post = |frame, target: f32, claim: usize| Post { frame, spot_claims: HashSet::from([claim]), target: Some(at(target)), offer: vec![UnitDefId(1)], committed: vec![UnitDefId(2)], launched: None };
        assert!(board.exchange(3, post(15, 30.0, 7)).with_us.is_empty());
        let second = board.exchange(5, post(15, 50.0, 9));
        assert_eq!(second.lead_target.map(|t| t.x), Some(30.0));
        assert_eq!(second.spot_claims, HashSet::from([7]));
        assert_eq!(second.with_us.len(), 2);
        assert!(board.exchange(3, post(30, 30.0, 7)).lead_target.is_none());
        // A seat that has stopped posting drops out.
        assert!(board.exchange(3, post(300, 30.0, 7)).with_us.is_empty());
    }
}
