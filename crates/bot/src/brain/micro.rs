//! The control lane: what a soldier does with its order between the brain's decisions, every tick
//! (`docs/design/2026-09-20-micro-lane.md`). The brain (`think`) decides intent and gives orders; the lane may
//! override a unit's order for as long as a behaviour claims it, and gives the order back when none does. It never
//! chooses a target or re-prices a fight: it spends a unit's order on staying alive.
//!
//! H-MICRO-LANE switches the whole lane off; each behaviour has its own ID under it.

use std::collections::HashMap;

use bot_protocol::{Command, EnemyUnit, OwnUnit, Tick, UnitDefId, UnitId, Vec3};

use super::threat::ThreatGrid;
use super::{Brain, FRAMES_PER_SECOND};

/// Only soldiers with an enemy this close are looked at.
const HORIZON: f32 = 700.0;
/// The unit's next place is its position plus this many frames of its velocity: half a second.
const LOOKAHEAD_FRAMES: f32 = 15.0;
/// Beyond a threat's reach the grid slopes off over this much: what a Pawn walks in a second.
const TAIL: f32 = 90.0;
/// A committed unit leaves when the threat where it is heading would kill it within this long.
const EXPOSURE_SECONDS: f32 = 1.5;
/// A flee is a step to the least threatened cell within this.
const STEP_RADIUS: f32 = 100.0;
/// A fleeing unit's step is re-issued when it has moved this far, and no more often than this.
const REORDER_DISTANCE: f32 = 32.0;
const REORDER_FRAMES: i32 = 6;
/// An armed enemy out of sight is remembered where it was seen for this long, fading.
const MEMORY_FRAMES: i32 = 10 * FRAMES_PER_SECOND;
/// The grid's cell when the engine sent no terrain.
const DEFAULT_CELL: f32 = 16.0;
/// A source fainter than this (a memory nearly faded) stamps the grid but moves nobody.
const FAINT: f32 = 10.0;
/// A claim stands at least this long: a step's reversed velocity cleared the unit's predicted place at once.
const CLAIM_FRAMES: i32 = 30;

/// What a unit's group was priced against, so the lane knows which threats the brain meant it to face.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Commitment {
    /// Nothing: a unit on a Move, a scout, a raider waiting for the rest. It flees any threat.
    #[default]
    None,
    /// Every soldier in sight, these turrets, and the commander when `commander`.
    Priced { turrets: Vec<UnitId>, commander: bool },
    /// Everything (a committed wave, a commander's squad).
    All,
}

impl Commitment {
    fn covers(&self, source: &Source) -> bool {
        match self {
            Commitment::None => false,
            Commitment::All => true,
            Commitment::Priced { turrets, commander } => {
                if source.commander {
                    *commander
                } else if source.mobile {
                    true
                } else {
                    turrets.contains(&source.id)
                }
            }
        }
    }
}

/// One thing that can hurt us this tick.
struct Source {
    id: UnitId,
    pos: Vec3,
    reach: f32,
    /// Damage a second, faded when it is a memory.
    weight: f32,
    mobile: bool,
    commander: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rule {
    Flee,
}

impl Rule {
    fn id(self) -> &'static str {
        match self {
            Rule::Flee => "H-MICRO-FLEE",
        }
    }
}

/// A unit the lane has taken over: which behaviour, where it was last sent and when.
struct Claim {
    rule: Rule,
    sent_to: Vec3,
    frame: i32,
}

#[derive(Default)]
pub struct Lane {
    grid: Option<ThreatGrid>,
    /// Each soldier's last order from the brain, and its frame: the lane's terminal behaviour.
    standing: HashMap<UnitId, (Command, i32)>,
    commitment: HashMap<UnitId, Commitment>,
    claims: HashMap<UnitId, Claim>,
    /// Armed mobile enemies as last seen: type, place, frame.
    seen: HashMap<UnitId, (UnitDefId, Vec3, i32)>,
    /// Per minute, for the log: claims made, orders issued.
    counts: (u32, u32),
}

/// The brain gave the same order again (it re-issues standing orders every few seconds): no reason to let go.
fn same_order(a: &Command, b: &Command) -> bool {
    match (a, b) {
        (Command::Move { unit, to, queue }, Command::Move { unit: u, to: t, queue: q })
        | (Command::Fight { unit, to, queue }, Command::Fight { unit: u, to: t, queue: q }) => unit == u && to.dist2d(*t) < 1.0 && queue == q,
        (Command::Attack { unit, target, queue }, Command::Attack { unit: u, target: t, queue: q }) => unit == u && target == t && queue == q,
        (Command::Stop { unit }, Command::Stop { unit: u }) => unit == u,
        _ => false,
    }
}

fn unit_of(command: &Command) -> Option<UnitId> {
    match *command {
        Command::Move { unit, .. } | Command::Fight { unit, .. } | Command::Attack { unit, .. } | Command::Stop { unit } => Some(unit),
        _ => None,
    }
}

impl Brain {
    /// The brain's orders this tick become the standing orders of the soldiers they went to.
    pub(super) fn note_standing_orders(&mut self, commands: &[Command], frame: i32) {
        let Some(kit) = self.kit else { return };
        for command in commands {
            let Some(unit) = unit_of(command) else { continue };
            let soldier = self.known_units.get(&unit).is_some_and(|(def, _)| *def != kit.commander && self.world.def(*def).is_some_and(|d| d.speed > 0.0 && d.weapon_count > 0 && d.build_speed == 0.0));
            if soldier {
                // A new order from the brain outranks the lane's step (the lane claims again if it must); the same
                // order again does not (micro-flee-debug3: a wounded Pawn held out of a fight was sent back into it
                // by the raid's four-second re-issue, every time).
                if self.lane.standing.get(&unit).is_none_or(|(old, _)| !same_order(old, command)) {
                    self.lane.claims.remove(&unit);
                }
                self.lane.standing.insert(unit, (command.clone(), frame));
            }
        }
    }

    /// What each soldier's group was priced against, from the raid, the contact answers, the waves and the squads.
    pub(super) fn note_commitments(&mut self) {
        let mut commitment: HashMap<UnitId, Commitment> = HashMap::new();
        if self.raid.going() {
            let priced = Commitment::Priced { turrets: self.raid.turrets.clone(), commander: self.raid.fights_commander };
            for id in &self.raid.members {
                commitment.insert(*id, priced.clone());
            }
        }
        // An answer fights the commander only when it is the raid's size for it (the D-gun is not in the simulator;
        // micro-flee-debug: four Pawns answering the commander at our station walked into it and died in six seconds).
        for (members, turrets) in self.response_commitments() {
            let metal: f32 = members.iter().filter_map(|id| self.known_units.get(id)).filter_map(|(def, _)| self.world.def(*def)).map(|d| d.metal_cost).sum();
            let commander = metal >= super::raid::COMMANDER_PARTY_METAL;
            for id in members {
                commitment.insert(id, Commitment::Priced { turrets: turrets.clone(), commander });
            }
        }
        for (id, _) in self.known_units.iter() {
            if self.army.is_attacker(*id) || self.squads.contains(*id) {
                commitment.insert(*id, Commitment::All);
            }
        }
        self.lane.commitment = commitment;
    }

    /// Every tick: the threat grid, then each soldier near an enemy through the behaviours.
    pub(super) fn micro(&mut self, tick: &Tick) -> Vec<Command> {
        let Some(kit) = self.kit else { return Vec::new() };
        if !self.enabled("H-MICRO-LANE") {
            return Vec::new();
        }
        let frame = tick.frame;
        let snapshot = &tick.snapshot;
        let alive = |lane: &mut Lane| {
            lane.standing.retain(|id, _| snapshot.own_units.iter().any(|u| u.id == *id));
            lane.claims.retain(|id, _| snapshot.own_units.iter().any(|u| u.id == *id));
        };
        alive(&mut self.lane);
        let sources = self.threat_sources(snapshot.enemies.as_slice(), frame);
        self.rebuild_grid(&sources);
        let mut commands = Vec::new();
        let debug = std::env::var_os("WITHIN_REASON_MICRO_DEBUG").is_some();

        let soldiers: Vec<&OwnUnit> = snapshot.own_units.iter().filter(|u| !u.being_built && self.is_army(u, &kit)).collect();
        for unit in soldiers {
            let near = sources.iter().any(|s| s.pos.dist2d(unit.pos) < HORIZON + s.reach);
            if !near {
                self.release(unit.id, frame, &mut commands, debug);
                continue;
            }
            let next = Vec3 { x: unit.pos.x + unit.vel.x * LOOKAHEAD_FRAMES, y: 0.0, z: unit.pos.z + unit.vel.z * LOOKAHEAD_FRAMES };
            let commitment = self.lane.commitment.get(&unit.id).cloned().unwrap_or_default();
            let covers = |s: &Source, p: Vec3| s.pos.dist2d(p) < s.reach + TAIL;
            let unpriced_at = |p: Vec3| sources.iter().find(|s| !commitment.covers(s) && s.weight >= FAINT && covers(s, p));
            // Damage a second from soldiers of theirs at `p`: a committed unit leaves a unit fight it would die in,
            // but never a turret dive its group was priced for (a Pawn cannot get out of a tower's reach in time
            // whatever it does, and the dive needs its gun; micro-flee-debug).
            let mobile_threat_at = |p: Vec3| -> f32 {
                sources.iter().filter(|s| s.mobile && covers(s, p)).map(|s| {
                    let d = s.pos.dist2d(p);
                    s.weight * if d <= s.reach { 1.0 } else { (s.reach + TAIL - d) / TAIL }
                }).sum()
            };
            // H-MICRO-FLEE: a threat the brain did not price this unit against, where it is heading (or where it
            // stands, while it has not got out yet); or soldiers of theirs that would kill it before the brain looks again.
            let claimed = self.lane.claims.get(&unit.id).is_some_and(|c| c.rule == Rule::Flee);
            let unpriced = unpriced_at(next).or_else(|| if claimed { unpriced_at(unit.pos) } else { None });
            let lethal = mobile_threat_at(next) * EXPOSURE_SECONDS >= unit.health;
            if let Some(why) = unpriced.map(|s| ("unpriced", s.id)).or(lethal.then_some(("lethal", UnitId(-1)))) {
                let goal = self.standing_goal(unit.id, snapshot.enemies.as_slice()).unwrap_or(self.home);
                let passable = self.passable();
                let Some(step) = self.lane.grid.as_ref().and_then(|g| g.lowest_within(unit.pos, STEP_RADIUS, passable, goal)) else { continue };
                let claim = self.lane.claims.get(&unit.id);
                let fresh = claim.is_none_or(|c| c.rule != Rule::Flee);
                let reorder = claim.is_none_or(|c| c.sent_to.dist2d(step) > REORDER_DISTANCE && frame - c.frame >= REORDER_FRAMES);
                if fresh {
                    self.fire(Rule::Flee.id());
                    self.lane.counts.0 += 1;
                    if debug {
                        eprintln!(
                            "[ai {}] f={frame} micro: {}#{} flees ({why:?}) from ({:.0}, {:.0}) to ({:.0}, {:.0}), threat there {:.0}/s, health {:.0}",
                            self.ai(), self.name(unit.def), unit.id.0, unit.pos.x, unit.pos.z, step.x, step.z,
                            self.lane.grid.as_ref().map_or(0.0, |g| g.at(next)), unit.health
                        );
                    }
                }
                if fresh || reorder {
                    self.lane.counts.1 += 1;
                    self.lane.claims.insert(unit.id, Claim { rule: Rule::Flee, sent_to: step, frame });
                    commands.push(Command::Move { unit: unit.id, to: step, queue: false });
                }
                continue;
            }
            // Out of danger, but its order leads straight back in (the goal, or the way to it): it stands where it
            // is until the brain orders otherwise, rather than walking in and out of the tail every few seconds
            // (micro-flee-debug2: a Pawn released at the tail's edge walked back into a tower's reach three times).
            if claimed
                && let Some(goal) = self.standing_goal(unit.id, snapshot.enemies.as_slice())
                && (sources.iter().any(|s| !commitment.covers(s) && s.weight >= FAINT && super::contact::to_segment(s.pos, unit.pos, goal) < s.reach + TAIL)
                    || mobile_threat_at(goal) * EXPOSURE_SECONDS >= unit.health)
            {
                continue;
            }
            if self.lane.claims.get(&unit.id).is_some_and(|c| frame - c.frame < CLAIM_FRAMES) {
                continue;
            }
            self.release(unit.id, frame, &mut commands, debug);
        }
        if tick.due() % (60 * FRAMES_PER_SECOND) == 0 && self.lane.counts != (0, 0) {
            let (claims, orders) = std::mem::take(&mut self.lane.counts);
            eprintln!("[ai {}] f={frame} micro this minute: {claims} units took over, {orders} steps", self.ai());
        }
        commands
    }

    /// A claimed unit no behaviour wants any more gets its standing order back, once.
    fn release(&mut self, unit: UnitId, frame: i32, commands: &mut Vec<Command>, debug: bool) {
        if self.lane.claims.remove(&unit).is_none() {
            return;
        }
        if let Some((order, _)) = self.lane.standing.get(&unit) {
            if debug {
                eprintln!("[ai {}] f={frame} micro: unit {} released to its order", self.ai(), unit.0);
            }
            commands.push(order.clone());
        }
    }

    /// Where the unit's standing order was taking it: what a flee step prefers among equally safe cells.
    fn standing_goal(&self, unit: UnitId, enemies: &[EnemyUnit]) -> Option<Vec3> {
        match self.lane.standing.get(&unit)?.0 {
            Command::Move { to, .. } | Command::Fight { to, .. } => Some(to),
            Command::Attack { target, .. } => enemies.iter().find(|e| e.id == target).map(|e| e.pos).or_else(|| self.enemy_buildings.get(&target).map(|b| b.1)),
            _ => None,
        }
    }

    /// Everything that can hurt us this tick: armed enemies in sight, remembered armed buildings, and armed
    /// soldiers seen lately at their last place, fading.
    fn threat_sources(&mut self, enemies: &[EnemyUnit], frame: i32) -> Vec<Source> {
        if self.contacts.sim_defs.is_empty() {
            self.survey_sim_defs();
        }
        let rules = self.contacts.rules.clone();
        let stats = |def: UnitDefId| -> Option<(f32, f32)> {
            let unit = &rules.units.list[*self.contacts.sim_defs.get(&def)?];
            (unit.reach() > 0.0).then(|| (unit.reach(), unit.dps()))
        };
        // A radar contact is taken for the soldier of theirs we have seen most, as the pricing does.
        let mut counted: HashMap<UnitDefId, usize> = HashMap::new();
        self.enemy_soldiers.values().for_each(|(def, _)| *counted.entry(*def).or_default() += 1);
        let blip = counted.into_iter().max_by_key(|(def, n)| (*n, def.0)).map(|(def, _)| def).or(self.kit.as_ref().map(|k| k.line));
        let is_commander = |def: UnitDefId| self.world.def(def).is_some_and(|d| d.name.ends_with("com") && d.build_speed > 0.0);
        let mut sources = Vec::new();
        for enemy in enemies {
            let Some(def) = enemy.def.or(blip) else { continue };
            let Some(d) = self.world.def(def) else { continue };
            if d.weapon_count == 0 {
                continue;
            }
            let Some((reach, dps)) = stats(def) else { continue };
            let mobile = d.speed > 0.0;
            if mobile {
                self.lane.seen.insert(enemy.id, (def, enemy.pos, frame));
            }
            sources.push(Source { id: enemy.id, pos: enemy.pos, reach, weight: dps, mobile, commander: is_commander(def) });
        }
        for (id, (def, pos, _)) in &self.enemy_buildings {
            if sources.iter().any(|s| s.id == *id) {
                continue;
            }
            let Some((reach, dps)) = stats(*def) else { continue };
            sources.push(Source { id: *id, pos: *pos, reach, weight: dps, mobile: false, commander: false });
        }
        self.lane.seen.retain(|id, (_, _, at)| frame - *at < MEMORY_FRAMES && self.enemy_soldiers.contains_key(id));
        for (id, (def, pos, at)) in &self.lane.seen {
            if sources.iter().any(|s| s.id == *id) {
                continue;
            }
            let Some((reach, dps)) = stats(*def) else { continue };
            let fade = 1.0 - (frame - at) as f32 / MEMORY_FRAMES as f32;
            sources.push(Source { id: *id, pos: *pos, reach, weight: dps * fade, mobile: true, commander: is_commander(*def) });
        }
        sources
    }

    fn rebuild_grid(&mut self, sources: &[Source]) {
        if self.lane.grid.is_none() {
            let terrain = &self.world.hello.terrain;
            let (cell, width, height) = if terrain.width > 0 && terrain.cell > 0.0 {
                (terrain.cell, terrain.width as usize, terrain.height as usize)
            } else {
                let map = &self.world.hello.map;
                (DEFAULT_CELL, (map.width / DEFAULT_CELL).ceil() as usize, (map.height / DEFAULT_CELL).ceil() as usize)
            };
            self.lane.grid = Some(ThreatGrid::new(cell, width, height));
        }
        let grid = self.lane.grid.as_mut().expect("made above");
        grid.clear();
        for source in sources {
            grid.stamp(source.pos, source.reach, TAIL, source.weight);
        }
    }
}
