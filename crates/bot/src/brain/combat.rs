//! Combat prediction: who wins if these two forces fight it out, from metal value and the duel table
//! (`docs/harness/duels.md`). Checked against 273 decisive engagements of recorded games
//! (`run/predict_check.py`): the side given the higher power lost the smaller share of its force in 89 % of them, 94 %
//! when the ratio was beyond 2:1. Plain metal value does nearly as well (88 %); the matchup weighting matters when
//! the two sides field different kinds of unit. What it cannot know is what we have not seen.

use std::collections::HashMap;

use bot_protocol::UnitDefId;

use super::Brain;

/// A turret is worth more in a fight than its metal: it is tough for its cost and does not need to walk anywhere.
pub const TURRET_WORTH: f32 = 1.5;

/// `margin[(unit, against)]` at equal metal, -1..1, by unit name.
pub struct Matchups(HashMap<(String, String), f32>);

impl Default for Matchups {
    fn default() -> Self {
        let rows = include_str!("../../data/matchups.csv").lines().filter(|l| !l.starts_with('#'));
        Matchups(
            rows.filter_map(|line| {
                let mut fields = line.split(',');
                Some(((fields.next()?.to_string(), fields.next()?.to_string()), fields.next()?.parse().ok()?))
            })
            .collect(),
        )
    }
}

impl Matchups {
    /// What one metal of `unit` is worth against one metal of `against`: the Lanchester square law run backwards
    /// from the duel margin (equal forces, winner keeps a share m of its value: effectiveness ratio 1 / (1 - m^2)).
    fn effectiveness(&self, unit: &str, against: &str) -> f32 {
        let margin = self.0.get(&(unit.to_string(), against.to_string())).copied().unwrap_or(0.0).clamp(-0.95, 0.95);
        if margin >= 0.0 { 1.0 / (1.0 - margin * margin) } else { 1.0 - margin * margin }
    }
}

/// A force: how many of each mobile unit type, and the metal of the turrets standing with it.
#[derive(Default, Clone)]
pub struct Force {
    pub units: HashMap<UnitDefId, usize>,
    pub turret_metal: f32,
}

impl Force {
    pub fn add(&mut self, def: UnitDefId) {
        *self.units.entry(def).or_default() += 1;
    }
}

impl Brain {
    /// Fighting power of `force` against `other`, in metal-equivalents.
    fn power(&self, force: &Force, other: &Force) -> f32 {
        let metal = |def: UnitDefId| self.world.def(def).map_or(0.0, |d| d.metal_cost);
        let other_total: f32 = other.units.iter().map(|(def, n)| metal(*def) * *n as f32).sum();
        let soldiers: f32 = force
            .units
            .iter()
            .map(|(def, n)| {
                let effectiveness = if other_total > 0.0 {
                    other.units.iter().map(|(against, k)| metal(*against) * *k as f32 / other_total * self.matchups.effectiveness(self.name(*def), self.name(*against))).sum()
                } else {
                    1.0
                };
                metal(*def) * *n as f32 * effectiveness.sqrt()
            })
            .sum();
        soldiers + TURRET_WORTH * force.turret_metal
    }

    /// Our power over theirs if `ours` fights `theirs`: above 1 we should win, and by the square law a ratio r leaves
    /// the winner about sqrt(1 - 1/r^2) of its force.
    pub(super) fn odds(&self, ours: &Force, theirs: &Force) -> f32 {
        self.power(ours, theirs) / self.power(theirs, ours).max(1.0)
    }
}
