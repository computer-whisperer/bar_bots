//! "Can these catch and kill those within this long, or should they not bother?" A party of the other side is at
//! our buildings with an intent; some of ours set off after it from somewhere else. The answer is what each side
//! loses, what burns meanwhile, and when (if ever) the pursuers land their first hit
//! (`docs/design/2026-09-20-army-response.md`).

use crate::scenario::{Group, Intent, Scenario, Vec2};
use crate::sim::{Rules, simulate};

#[derive(Clone, Debug)]
pub struct Chase {
    /// Ours that go after the party: unit type, count, and where that group starts from.
    pub pursuers: Vec<(usize, u32, Vec2)>,
    /// The party (unit type, count), where it is and what it is there to do.
    pub party: Vec<(usize, u32)>,
    pub at: Vec2,
    pub intent: Intent,
    /// Our unarmed buildings within the party's reach of mischief: unit type and place.
    pub assets: Vec<(usize, Vec2)>,
    /// The party's own buildings standing with it (its turrets, when the question is our raid on its ground): they
    /// hold and shoot, and count in what the party loses.
    pub party_buildings: Vec<(usize, Vec2)>,
    /// The pursuers' turrets, holding where they stand: the question asked the other way round, our party raiding
    /// their base (`docs/design/2026-09-20-base-raid-pricing.md`): their soldiers pursue, their towers hold, their
    /// unarmed buildings are the assets, and ours is the party with a raid intent.
    pub pursuer_buildings: Vec<(usize, Vec2)>,
    /// How long the question runs.
    pub seconds: f32,
}

/// Means over the seeds asked for. Metal throughout.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Verdict {
    /// Share of runs in which a pursuer hurt one of the party at all, and the mean second it first did.
    pub caught: f32,
    pub caught_after: f32,
    pub party_killed: f32,
    pub pursuers_lost: f32,
    pub assets_lost: f32,
    /// Share of the party's units alive at the end, gone from the field or still on it.
    pub survived: f32,
}

impl Verdict {
    /// What sending the pursuers is worth against `other` (the same contact answered another way, or not at all):
    /// more of theirs killed, less of ours burned or lost.
    pub fn gain_over(&self, other: &Verdict) -> f32 {
        (self.party_killed - other.party_killed) - (self.pursuers_lost - other.pursuers_lost) - (self.assets_lost - other.assets_lost)
    }
}

impl Chase {
    pub fn scenario(&self) -> Scenario {
        let mut scenario = Scenario::new();
        (scenario.time_limit, scenario.stalemate) = (self.seconds, self.seconds);
        for (def, count, from) in &self.pursuers {
            scenario.sides[0].push(Group::new(*def, *count, *from, from.towards(self.at)));
        }
        for (def, place) in &self.assets {
            scenario.sides[0].push(Group::new(*def, 1, *place, Vec2::new(1.0, 0.0)));
        }
        for (def, place) in &self.pursuer_buildings {
            let mut group = Group::new(*def, 1, *place, place.towards(self.at));
            group.hold = true;
            scenario.sides[0].push(group);
        }
        let threat = self.pursuers.first().map_or(Vec2::new(self.at.x - 1.0, self.at.z), |p| p.2);
        for (def, place) in &self.party_buildings {
            let mut group = Group::new(*def, 1, *place, place.towards(threat));
            group.hold = true;
            scenario.sides[1].push(group);
        }
        for (def, count) in &self.party {
            let mut group = Group::new(*def, *count, self.at, self.at.towards(threat));
            group.intent = self.intent;
            scenario.sides[1].push(group);
        }
        scenario
    }

    pub fn verdict(&self, rules: &Rules, reps: u32) -> Verdict {
        let scenario = self.scenario();
        let party_count: u32 = self.party.iter().map(|(_, n)| n).sum();
        let mut verdict = Verdict::default();
        let mut caught_runs = 0.0;
        for seed in 0..reps.max(1) {
            let outcome = simulate(rules, &scenario, seed as u64);
            if let Some(after) = outcome.first_hurt[1] {
                caught_runs += 1.0;
                verdict.caught_after += after;
            }
            verdict.party_killed += outcome.metal_lost[1];
            verdict.pursuers_lost += outcome.metal_lost[0] - outcome.assets_lost[0];
            verdict.assets_lost += outcome.assets_lost[0];
            let alive: u32 = outcome.survivors[1].iter().map(|(_, n)| n).sum();
            verdict.survived += alive as f32 / party_count.max(1) as f32;
        }
        let n = reps.max(1) as f32;
        verdict.caught = caught_runs / n;
        verdict.caught_after = if caught_runs > 0.0 { verdict.caught_after / caught_runs } else { 0.0 };
        for field in [&mut verdict.party_killed, &mut verdict.pursuers_lost, &mut verdict.assets_lost, &mut verdict.survived] {
            *field /= n;
        }
        verdict
    }
}
