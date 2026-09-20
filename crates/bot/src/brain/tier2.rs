//! Tier 2 (docs/knowledge/tier2.md): when to build the advanced bot lab, who helps, and what the advanced
//! constructors do with it. BARb medium starts its own at a median 19.7 minutes in 63 % of games
//! (K-t2-barb-goes-tier-2), and every game of ours that ran past that was tier 1 against tier 2.

use bot_protocol::{OwnUnit, Tick, UnitId, Vec3};

use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};

/// H-T2-GATE: the economy that can carry the 2600-metal lab without stopping everything else (K-t2-gate-thresholds,
/// the softer of the two community rules: we bank nothing, so the lab is paid from income).
const GATE_METAL_INCOME: f32 = 22.0;
const GATE_ENERGY_INCOME: f32 = 450.0;
/// Not while things are dying at home: the lab is three minutes of one constructor's time and all our metal.
const GATE_QUIET_FRAMES: i32 = 45 * FRAMES_PER_SECOND;
/// H-T2-ASSIST: builders that help put the advanced lab up, the commander among them (K-t2-needs-build-power).
pub(super) const LAB_ASSISTANTS: usize = 3;
/// Advanced constructors the lab makes before soldiers.
pub(super) const ADVANCED_CONSTRUCTORS: usize = 2;
/// H-T2-MOHO: no upgrade is started with less energy than this share of storage (20 E/s upkeep each, 7700 to build).
const UPGRADE_ENERGY: f32 = 0.5;

/// Tier-1 constructors helping one upgrade. With no metal banked every consumer gets a share of the income, and an
/// advanced constructor alone took six minutes over an upgrade that repays itself in under two (t2-first-look).
const UPGRADE_HELPERS: usize = 2;
/// The advanced lab makes no soldiers until this many extractors are upgraded, while there are any left to upgrade:
/// the upgrades are what tier 2 is for (K-t2-moho-first), and the lab would take half the metal they need.
pub(super) const UPGRADES_BEFORE_ARMY: usize = 4;

/// What tier 2 asks of a builder right now.
pub(super) enum Advance {
    StartLab,
    Help(UnitId),
    Upgrade(Vec3),
}

impl Brain {
    /// For a tier-1 constructor or the commander: start the advanced lab, or help the one going up.
    pub(super) fn advance_for(&self, builder: &OwnUnit, tick: &Tick, kit: &Kit, planned_labs: usize) -> Option<Advance> {
        if !self.enabled("H-T2-GATE") {
            return None;
        }
        let snapshot = &tick.snapshot;
        let rising = snapshot.own_units.iter().find(|u| u.def == kit.advanced_lab && u.being_built);
        if let Some(lab) = rising {
            return self.enabled("H-T2-ASSIST").then_some(Advance::Help(lab.id));
        }
        let helping = |target: bot_protocol::UnitDefId| {
            self.jobs.iter().filter(|(id, job)| **job == target && snapshot.own_units.iter().any(|u| u.id == **id && u.def != kit.advanced_constructor)).count()
        };
        let upgrade = snapshot
            .own_units
            .iter()
            .filter(|u| u.def == kit.advanced_extractor && u.being_built && u.pos.dist2d(builder.pos) < 1500.0)
            .min_by(|a, b| a.pos.dist2d(builder.pos).total_cmp(&b.pos.dist2d(builder.pos)));
        if let Some(upgrade) = upgrade
            && builder.def != kit.commander
            && self.enabled("H-T2-ASSIST")
            && helping(kit.advanced_extractor) < UPGRADE_HELPERS
        {
            return Some(Advance::Help(upgrade.id));
        }
        let held = self.directives.tier2.map(|d| d.value);
        let economy = snapshot.metal.income >= GATE_METAL_INCOME && snapshot.energy.income >= GATE_ENERGY_INCOME;
        let quiet = tick.frame - self.last_loss_at_home_frame > GATE_QUIET_FRAMES;
        let go = match held {
            Some(true) => true,
            Some(false) => false,
            None => economy && quiet,
        };
        let can = self.world.def(builder.def).is_some_and(|d| d.build_options.contains(&kit.advanced_lab));
        (go && can && planned_labs == 0).then_some(Advance::StartLab)
    }

    /// For an advanced constructor: the tier-1 extractor of ours nearest home that nobody is upgrading yet.
    pub(super) fn upgrade_for(&mut self, builder: &OwnUnit, tick: &Tick, kit: &Kit) -> Option<Advance> {
        let snapshot = &tick.snapshot;
        if !self.enabled("H-T2-MOHO") || snapshot.energy.current < snapshot.energy.storage * UPGRADE_ENERGY {
            return None;
        }
        let own = &snapshot.own_units;
        let upgraded = |at: Vec3| own.iter().any(|u| u.def == kit.advanced_extractor && u.pos.dist2d(at) < 50.0);
        let claimed = |at: Vec3| self.upgrade_claims.iter().any(|(id, spot)| *id != builder.id && spot.dist2d(at) < 50.0);
        let spot = own
            .iter()
            .filter(|u| u.def == kit.extractor && !u.being_built && !upgraded(u.pos) && !claimed(u.pos) && self.ground(u.pos) == super::territory::Ground::Held)
            .map(|u| u.pos)
            .min_by(|a, b| self.walk_from_home(*a).total_cmp(&self.walk_from_home(*b)))?;
        self.upgrade_claims.insert(builder.id, spot);
        Some(Advance::Upgrade(spot))
    }
}
