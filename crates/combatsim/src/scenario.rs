//! What a query is and what it answers: two sides, each a list of groups, on optional terrain.

use crate::field::Field;
use crate::units::Units;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Vec2 {
    pub x: f32,
    pub z: f32,
}

impl Vec2 {
    pub fn new(x: f32, z: f32) -> Vec2 {
        Vec2 { x, z }
    }

    pub fn dist(self, other: Vec2) -> f32 {
        self.dist2(other).sqrt()
    }

    pub fn dist2(self, other: Vec2) -> f32 {
        let (dx, dz) = (self.x - other.x, self.z - other.z);
        dx * dx + dz * dz
    }

    /// Unit vector towards `other`; zero when they coincide.
    pub fn towards(self, other: Vec2) -> Vec2 {
        let (dx, dz) = (other.x - self.x, other.z - self.z);
        let len = (dx * dx + dz * dz).sqrt();
        if len < 1e-4 { Vec2::default() } else { Vec2::new(dx / len, dz / len) }
    }
}

impl std::ops::Add for Vec2 {
    type Output = Vec2;
    fn add(self, other: Vec2) -> Vec2 {
        Vec2::new(self.x + other.x, self.z + other.z)
    }
}

impl std::ops::Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, by: f32) -> Vec2 {
        Vec2::new(self.x * by, self.z * by)
    }
}

/// One block of identical units: where they stand, how tightly, and when they turn up.
#[derive(Clone, Debug)]
pub struct Group {
    /// Index into `Units::list`.
    pub def: usize,
    pub count: u32,
    /// Centre of the front rank.
    pub front: Vec2,
    /// Which way the formation faces; further ranks stack away from it. Normalised on use.
    pub facing: Vec2,
    pub spacing: f32,
    pub per_rank: u32,
    /// Seconds before these units exist (reinforcements arriving late).
    pub delay: f32,
    /// Stand and fight where placed instead of advancing. Buildings always hold.
    pub hold: bool,
}

impl Group {
    pub fn new(def: usize, count: u32, front: Vec2, facing: Vec2) -> Group {
        Group { def, count, front, facing, spacing: 56.0, per_rank: 8, delay: 0.0, hold: false }
    }

    /// Where each unit of the group stands: ranks across the facing, further ranks behind. This is the duel
    /// harness's formation (`crates/arena/src/bin/duel/sites.rs`), so a duel can be replayed here.
    pub fn places(&self) -> Vec<Vec2> {
        let ahead = Vec2::default().towards(self.facing);
        let across = Vec2::new(-ahead.z, ahead.x);
        (0..self.count)
            .map(|i| {
                let (rank, file) = (i / self.per_rank, i % self.per_rank);
                let in_rank = (self.count - rank * self.per_rank).min(self.per_rank);
                let back = ahead * (-self.spacing * rank as f32);
                let side = across * (self.spacing * (file as f32 - (in_rank - 1) as f32 / 2.0));
                self.front + back + side
            })
            .collect()
    }
}

/// What a side has to pay its laser towers with. Weapons with an `energypershot` simply do not fire when the
/// store is empty (K-units-laser-towers-need-energy), and in a fight with no economy behind it that is most of
/// the time: fourteen light towers want 600 energy a second and a commander makes thirty.
#[derive(Clone, Copy, Debug)]
pub struct Energy {
    pub stored: f32,
    /// Per second.
    pub income: f32,
}

impl Default for Energy {
    /// What a duel team has: a commander's 30 a second, and a store part-refilled since the previous duel drained
    /// it. The duel rows bracket it — Mace against Sentry spent exactly the income (52 shots in 34.5 s, an empty
    /// store), Pawn against Sentry spent income plus a full 1500 — and agreement is flat between 400 and 750.
    fn default() -> Energy {
        Energy { stored: 500.0, income: 30.0 }
    }
}

/// Which enemy a unit shoots at, when it has a choice.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Focus {
    /// The engine's own answer, near enough: whatever is nearest.
    #[default]
    Nearest,
    /// The one with the fewest hit points left, among those this unit can already reach.
    Weakest,
    /// The one with the most damage a second per hit point left: kill what hurts most per second spent.
    Threat,
}

impl Focus {
    pub fn parse(name: &str) -> Option<Focus> {
        match name {
            "nearest" => Some(Focus::Nearest),
            "weakest" => Some(Focus::Weakest),
            "threat" => Some(Focus::Threat),
            _ => None,
        }
    }
}

/// What a side's soldiers do beyond walking at the enemy and shooting the nearest thing: one field per policy,
/// each off at its default, so `Micro::default()` is plain attack-move and a policy can be priced on its own.
///
/// These are *policies*, not orders. Whether the bot can express one with `Move`/`Fight`/`Stop` at its 0.5 s tick
/// is a separate question, answered in `docs/studies/micro-combat.md`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Micro {
    /// Keep at least this many elmos from the nearest friend while advancing (0 is off). An army that arrives
    /// loose is a different army as far as area damage is concerned (K-units-duel-spacing-decides-area-damage).
    pub spread: f32,
    /// A unit under this share of its health turns round and walks away from the enemy (0 is off). It keeps
    /// shooting whatever comes into range on the way, as a unit under a move order does.
    pub withdraw_below: f32,
    /// Units that out-range their target back off when it closes, instead of letting it walk into their face.
    pub kite: bool,
    /// Whether a unit that out-ranges its target but cannot outrun it kites anyway.
    pub kite_when_slower: bool,
    pub focus: Focus,
    /// Do not walk after a target that is faster than us and out of reach: stand and let it come.
    pub no_chase: bool,
}

impl Micro {
    pub fn is_default(&self) -> bool {
        self.spread == 0.0 && self.withdraw_below == 0.0 && !self.kite && self.focus == Focus::Nearest && !self.no_chase
    }
}

#[derive(Clone, Debug, Default)]
pub struct Scenario {
    pub sides: [Vec<Group>; 2],
    pub energy: [Energy; 2],
    /// Per-side unit micro; both sides plain attack-move by default.
    pub micro: [Micro; 2],
    pub terrain: Option<Field>,
    /// Game seconds after which an undecided fight is scored as it stands; the duel harness uses 240.
    pub time_limit: f32,
    /// A fight nobody has been hurt in for this long is over (anti-air against anti-air).
    pub stalemate: f32,
}

impl Scenario {
    pub fn new() -> Scenario {
        Scenario { time_limit: 240.0, stalemate: 60.0, ..Scenario::default() }
    }

    pub fn metal(&self, units: &Units, side: usize) -> f32 {
        self.sides[side].iter().map(|g| units.list[g.def].metal * g.count as f32).sum()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum End {
    /// One side has nothing left.
    Wiped,
    Timeout,
    /// Nobody could hurt anybody for a minute.
    Stalemate,
}

#[derive(Clone, Debug)]
pub struct Outcome {
    /// The side with more left, or `None` when the two are within a hair of each other.
    pub winner: Option<usize>,
    pub reason: End,
    pub seconds: f32,
    /// Seconds from the start to the first damage.
    pub contact_seconds: Option<f32>,
    /// Living units per side, by unit index.
    pub survivors: [Vec<(usize, u32)>; 2],
    pub metal_lost: [f32; 2],
    /// Surviving share of the side's metal, each survivor weighted by its health — the duel harness's score.
    pub value_left: [f32; 2],
    /// Side 0's `value_left` less side 1's: +1 a flawless win, -1 a wipe without a scratch.
    pub margin: f32,
    /// Value left per side every few seconds, for a caller that wants to see the shape of the fight.
    pub timeline: Vec<[f32; 2]>,
}

/// What `reps` seeds say about a scenario.
#[derive(Clone, Debug, Default)]
pub struct Odds {
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub mean_margin: f32,
    pub mean_seconds: f32,
}

impl Odds {
    pub fn win_probability(&self) -> f32 {
        let total = self.wins + self.losses + self.draws;
        if total == 0 { 0.5 } else { (self.wins as f32 + self.draws as f32 / 2.0) / total as f32 }
    }
}
