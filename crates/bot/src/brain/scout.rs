//! Scouting (`docs/design/2026-09-20-scouting.md`): when each metal spot was last in sight (H-SCOUT-SPOTS), what is
//! worth a look from where, and one raider at a time sent round a route of such spots (H-SCOUT-ROUTE). The
//! enemy's extractors cluster round its start, so the spots round a presumed or found base come first, the enemy
//! start boxes next, the rest of the map eventually.

use bot_protocol::{Command, OwnUnit, Tick, UnitDefId, UnitId, Vec3};

use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};

/// Sight of a unit type the simulator's table does not know.
const DEFAULT_SIGHT: f32 = 350.0;
/// Spots are surveyed against every unit's sight this often.
const SURVEY_FRAMES: i32 = FRAMES_PER_SECOND / 2;
/// A spot seen this recently needs no looking at.
const FRESH_FRAMES: i32 = 60 * FRAMES_PER_SECOND;
/// A spot never seen counts as this stale.
const NEVER_SEEN_FRAMES: i32 = 600 * FRAMES_PER_SECOND;
/// Round a live enemy base its extractors are likely: this far from the base's point.
const BASE_VICINITY: f32 = 1800.0;
/// Likelihood of something of theirs at a spot: round a base, in an enemy start box, anywhere else.
const LIKELY_BASE: f32 = 3.0;
const LIKELY_BOX: f32 = 1.0;
const LIKELY_ELSEWHERE: f32 = 0.3;
/// The walk a spot's worth is divided by is at least this, so the next spot over does not always win.
const MIN_WALK: f32 = 300.0;
/// A scout keeps this far from a known armed building.
pub(super) const TURRET_BERTH: f32 = 450.0;
/// A route is this many spots, the best first, then greedily the nearest of this many candidates from each point on.
const ROUTE_SPOTS: usize = 4;
const ROUTE_CANDIDATES: usize = 8;
/// No scout before this: the first raider is the pressure party's.
const SCOUT_FROM: i32 = 120 * FRAMES_PER_SECOND;
/// A route is given up on after this long, and a new one sent at most this often when nothing is stale.
const ROUTE_FRAMES: i32 = 90 * FRAMES_PER_SECOND;

#[derive(Default)]
pub struct Spots {
    /// Per metal spot (the hello's order), the last frame it was within an own unit's sight.
    seen: Vec<Option<i32>>,
    last_survey: i32,
    /// The raider on a route, and since when.
    scout: Option<(UnitId, i32)>,
    /// The `scout_at` directive already served, by its expiry frame.
    served_scout_at: Option<i32>,
}

impl Brain {
    /// Sight of a unit type: the simulator's table, else a default.
    fn sight_of(&self, def: UnitDefId) -> f32 {
        self.contacts.sim_defs.get(&def).map_or(DEFAULT_SIGHT, |&index| self.contacts.rules.units.list[index].sight)
    }

    /// H-SCOUT-SPOTS: marks every spot within some unit of ours' sight as seen now.
    pub(super) fn survey_spots(&mut self, tick: &Tick) {
        let count = self.world.hello.metal_spots.len();
        if self.spots.seen.len() != count {
            self.spots.seen = vec![None; count];
        }
        if tick.frame - self.spots.last_survey < SURVEY_FRAMES {
            return;
        }
        self.spots.last_survey = tick.frame;
        if self.contacts.sim_defs.is_empty() {
            self.survey_sim_defs();
        }
        let spots = &self.world.hello.metal_spots;
        let sights: Vec<(Vec3, f32)> = tick.snapshot.own_units.iter().map(|u| (u.pos, self.sight_of(u.def))).collect();
        for (index, spot) in spots.iter().enumerate() {
            if sights.iter().any(|(pos, sight)| pos.dist2d(*spot) <= *sight) {
                self.spots.seen[index] = Some(tick.frame);
            }
        }
    }

    /// Frames since the spot was in sight; never seen counts as long ago.
    fn spot_staleness(&self, index: usize, frame: i32) -> i32 {
        self.spots.seen.get(index).copied().flatten().map_or(NEVER_SEEN_FRAMES, |seen| (frame - seen).min(NEVER_SEEN_FRAMES))
    }

    /// How likely something of theirs stands at `spot`.
    pub(super) fn spot_likelihood(&self, spot: Vec3) -> f32 {
        if self.live_enemy_bases().iter().any(|base| base.dist2d(spot) < BASE_VICINITY) {
            LIKELY_BASE
        } else if self.world.hello.start_boxes.iter().any(|b| b.ally_team != self.world.hello.ally_team && b.contains(spot)) {
            LIKELY_BOX
        } else {
            LIKELY_ELSEWHERE
        }
    }

    /// Spots worth a look from `from`, best first: staleness times likelihood over the walk there. Fresh and
    /// unreachable spots are left out, and those within `berth` of a known armed building. `at_least`: the lowest
    /// likelihood taken.
    pub(super) fn spots_to_look_at(&self, from: Vec3, frame: i32, berth: f32, at_least: f32) -> Vec<Vec3> {
        let armed: Vec<Vec3> = self.enemy_buildings.values()
            .filter(|(def, _, _)| self.world.def(*def).is_some_and(|d| d.weapon_count > 0))
            .map(|(_, pos, _)| *pos)
            .collect();
        let mut scored: Vec<(f32, Vec3)> = self.world.hello.metal_spots.iter().enumerate()
            .filter(|(index, _)| self.spot_staleness(*index, frame) >= FRESH_FRAMES)
            .map(|(index, spot)| (index, Vec3 { y: 0.0, ..*spot }))
            .filter(|(_, spot)| self.reachable_on_foot(*spot) && !armed.iter().any(|a| a.dist2d(*spot) < berth))
            .filter_map(|(index, spot)| {
                let likelihood = self.spot_likelihood(spot);
                (likelihood >= at_least).then(|| (self.spot_staleness(index, frame) as f32 / FRAMES_PER_SECOND as f32 * likelihood / spot.dist2d(from).max(MIN_WALK), spot))
            })
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored.into_iter().map(|(_, spot)| spot).collect()
    }

    /// H-SCOUT-ROUTE: one raider from `home_group` on a route of spots worth a look. Returns the scout, which the
    /// caller keeps out of the home group.
    pub(super) fn run_scout(&mut self, tick: &Tick, kit: &Kit, home_group: &[&OwnUnit], commands: &mut Vec<Command>) -> Option<UnitId> {
        if !self.enabled("H-SCOUT-ROUTE") {
            self.spots.scout = None;
            return None;
        }
        if let Some((id, since)) = self.spots.scout {
            let alive = tick.snapshot.own_units.iter().any(|u| u.id == id);
            if alive && tick.frame - since < ROUTE_FRAMES {
                return Some(id);
            }
            self.spots.scout = None;
        }
        if tick.frame < SCOUT_FROM {
            return None;
        }
        let base_found = self.found_enemy_base().is_some();
        let base = self.enemy_base(self.home);
        let freshest_round_base = self.world.hello.metal_spots.iter().enumerate()
            .filter(|(_, spot)| spot.dist2d(base) < BASE_VICINITY)
            .map(|(index, _)| self.spot_staleness(index, tick.frame))
            .min()
            .unwrap_or(NEVER_SEEN_FRAMES);
        // The commander's `scout_at`: once per directive, the route starts round its point.
        let asked = self.directives.scout_at.filter(|t| self.spots.served_scout_at != Some(t.expires_frame));
        let due = asked.is_some() || !base_found || freshest_round_base >= FRESH_FRAMES || tick.frame - self.spots.last_route() >= ROUTE_FRAMES;
        if !due {
            return None;
        }
        let around = asked.map_or(base, |t| t.value);
        let candidates = self.spots_to_look_at(around, tick.frame, TURRET_BERTH, 0.0);
        let first = candidates.first().copied().or_else(|| asked.map(|t| t.value))?;
        let scout = home_group.iter().filter(|u| u.def == kit.raider && !u.being_built).min_by(|a, b| a.pos.dist2d(first).total_cmp(&b.pos.dist2d(first)))?;
        // The route: the best spot from the scout, then the nearest of the best few from each point on.
        let mut route: Vec<Vec3> = Vec::new();
        let mut from = scout.pos;
        if asked.is_some() {
            route.push(first);
            from = first;
        }
        while route.len() < ROUTE_SPOTS {
            let next = self.spots_to_look_at(from, tick.frame, TURRET_BERTH, 0.0).into_iter()
                .filter(|s| !route.iter().any(|r| r.dist2d(*s) < 1.0))
                .take(if route.is_empty() { 1 } else { ROUTE_CANDIDATES })
                .min_by(|a, b| a.dist2d(from).total_cmp(&b.dist2d(from)));
            let Some(next) = next else { break };
            route.push(next);
            from = next;
        }
        if route.is_empty() {
            return None;
        }
        self.fire("H-SCOUT-ROUTE");
        self.spots.scout = Some((scout.id, tick.frame));
        if let Some(t) = asked {
            self.spots.served_scout_at = Some(t.expires_frame);
        }
        let squares: Vec<String> = route.iter().map(|s| self.world.grid(*s)).collect();
        eprintln!("[ai {}] f={} scout: {}#{} looks at {} ({} spots; base {})", self.ai(), tick.frame, self.name(scout.def), scout.id.0, squares.join(" "), route.len(), if base_found { "found" } else { "presumed" });
        self.journal.note(tick.frame, "scout", serde_json::json!({ "unit": scout.id.0, "route": route.iter().map(|s| [s.x as i32, s.z as i32]).collect::<Vec<_>>() }), serde_json::json!({ "squares": squares }));
        commands.extend(route.iter().enumerate().map(|(n, to)| Command::Move { unit: scout.id, to: *to, queue: n > 0 }));
        Some(scout.id)
    }
}

impl Spots {
    fn last_route(&self) -> i32 {
        self.scout.map_or(i32::MIN / 2, |(_, since)| since)
    }
}

impl Brain {
    /// The commander's `scouted` line: the spots round the enemy base by age, the box's never-seen squares, the scout out.
    pub(super) fn scouting_line(&self, frame: i32) -> String {
        let base = self.enemy_base(self.home);
        let spots = &self.world.hello.metal_spots;
        let round: Vec<usize> = (0..spots.len()).filter(|i| spots[*i].dist2d(base) < BASE_VICINITY).collect();
        let age = |i: &usize| self.spot_staleness(*i, frame);
        let unseen_round = round.iter().filter(|i| self.spots.seen.get(**i).copied().flatten().is_none()).count();
        let oldest = round.iter().filter(|i| self.spots.seen.get(**i).copied().flatten().is_some()).map(age).max();
        let mut never: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for (i, spot) in spots.iter().enumerate() {
            if self.spots.seen.get(i).copied().flatten().is_none() && !round.contains(&i) && self.spot_likelihood(*spot) >= LIKELY_BOX {
                *never.entry(self.world.grid(*spot)).or_default() += 1;
            }
        }
        let scout = self.spots.scout.map_or("no scout out".to_string(), |(id, since)| format!("scout #{} out for {}s", id.0, (frame - since) / FRAMES_PER_SECOND));
        format!(
            "round the enemy base ({}) {} spots: {} never seen, the rest last seen up to {}s ago | its box's spots never seen: {} | {scout}",
            self.world.grid(base), round.len(), unseen_round,
            oldest.map_or("-".to_string(), |a| (a / FRAMES_PER_SECOND).to_string()),
            if never.is_empty() { "none".to_string() } else { never.iter().map(|(g, n)| format!("{g} x{n}")).collect::<Vec<_>>().join(", ") }
        )
    }
}
