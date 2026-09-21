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
/// A wounded unit leaves a unit fight only when their fire on it beats our fire round it by this factor.
const ODDS_TO_STAY: f32 = 1.2;
/// A claim stands at least this long: a step's reversed velocity cleared the unit's predicted place at once.
const CLAIM_FRAMES: i32 = 30;
/// A target this far beyond a shooter's reach still counts as in it (the engine closes the last few elmos).
const FOCUS_SLACK: f32 = 20.0;
/// Shooters this close together choose targets together.
const FOCUS_GROUP: f32 = 300.0;
/// Shooters are put on a target until their fire would kill it within this long; the rest take the next.
const OVERKILL_SECONDS: f32 = 1.0;
/// A unit kites what it out-reaches by this much or more.
const KITE_MARGIN: f32 = 20.0;
/// Where a kiting unit keeps its enemy: this far inside its reach.
const KITE_EDGE: f32 = 15.0;
/// How far a kiting unit steps back while reloading, beyond what restores the edge.
const KITE_STEP: f32 = 40.0;
/// A commander's D-gun line is spaced out to this far beyond its reach: three seconds of a Pawn's approach. The
/// shot comes as the party enters the reach (matt-raidprice and matt-fan: every D-gun death was a unit closing at
/// full speed, dead at 240 to 280 from the commander, in a file with the rest), so a zone the size of the reach
/// itself catches a unit half a second before it dies (matt-fan: the rule fired ten times in twelve games).
const DGUN_MARGIN: f32 = 260.0;
/// The width of a D-gun shot: units of ours killed by one shot lay within 20 elmos either side of its line
/// (matt-plan, matt-raidprice, matt-fan2: six shots of two to six). Two lines to the commander that pass closer than
/// this at the edge of its reach are one line to the shot.
const DGUN_SHADOW: f32 = 48.0;
/// The gap a shadowed unit opens between its line and its friend's, at the edge of the reach.
const FAN_CLEAR: f32 = 64.0;
/// The most a fan step is, however far out the unit is.
const FAN_STEP_MAX: f32 = 150.0;

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
    /// The reach of its D-gun (a `command_fire` weapon); 0 for everything but a commander.
    dgun: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rule {
    Flee,
    Fan,
    Focus,
    Kite,
}

impl Rule {
    fn id(self) -> &'static str {
        match self {
            Rule::Flee => "H-MICRO-FLEE",
            Rule::Fan => "H-MICRO-FAN",
            Rule::Focus => "H-MICRO-FOCUS",
            Rule::Kite => "H-MICRO-KITE",
        }
    }
}

/// A standing order as the thing to do after a shot: the same order, queued.
fn queued(order: &Command) -> Command {
    match order.clone() {
        Command::Move { unit, to, .. } => Command::Move { unit, to, queue: true },
        Command::Fight { unit, to, .. } => Command::Fight { unit, to, queue: true },
        Command::Attack { unit, target, .. } => Command::Attack { unit, target, queue: true },
        other => other,
    }
}

/// A unit the lane has taken over: which behaviour, where it was last sent and when.
struct Claim {
    rule: Rule,
    sent_to: Vec3,
    /// What it was told to shoot (focus, or a kite's shot).
    target: Option<UnitId>,
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

    /// Every tick: the threat grid, then each soldier near an enemy through the behaviours, in order: flee, fan,
    /// focus, kite, the standing order.
    pub(super) fn micro(&mut self, tick: &Tick) -> Vec<Command> {
        let Some(kit) = self.kit else { return Vec::new() };
        if !self.enabled("H-MICRO-LANE") {
            return Vec::new();
        }
        let frame = tick.frame;
        let snapshot = &tick.snapshot;
        self.lane.standing.retain(|id, _| snapshot.own_units.iter().any(|u| u.id == *id));
        self.lane.claims.retain(|id, _| snapshot.own_units.iter().any(|u| u.id == *id));
        let sources = self.threat_sources(snapshot.enemies.as_slice(), frame);
        self.rebuild_grid(&sources);
        let mut commands = Vec::new();
        let debug = std::env::var_os("WITHIN_REASON_MICRO_DEBUG").is_some();

        let soldiers: Vec<&OwnUnit> = snapshot.own_units.iter().filter(|u| !u.being_built && self.is_army(u, &kit)).collect();
        let mut owned: HashMap<UnitId, Rule> = HashMap::new();
        let mut free: Vec<&OwnUnit> = Vec::new();
        for unit in &soldiers {
            let near = sources.iter().any(|s| s.pos.dist2d(unit.pos) < HORIZON + s.reach);
            if !near {
                self.release(unit.id, frame, &mut commands, debug);
                continue;
            }
            // Our fire round the unit, this one's included: what a unit fight there is worth staying in.
            let friends_dps: f32 = soldiers.iter().filter(|f| f.pos.dist2d(unit.pos) < FOCUS_GROUP).filter_map(|f| self.sim_stats(f.def)).map(|(_, dps, _)| dps).sum();
            match self.flee(unit, &sources, friends_dps, frame, &mut commands, debug) {
                Some(rule) => {
                    owned.insert(unit.id, rule);
                }
                None => free.push(unit),
            }
        }
        let fanned = self.fan(&free, &soldiers, &sources, frame, &mut commands, debug);
        owned.extend(fanned.iter().map(|id| (*id, Rule::Fan)));
        free.retain(|u| !fanned.contains(&u.id));
        let focused = self.focus(&free, snapshot.enemies.as_slice(), frame, &mut commands, debug);
        owned.extend(focused.iter().map(|id| (*id, Rule::Focus)));
        free.retain(|u| !focused.contains(&u.id));
        for unit in &free {
            if self.kite(unit, snapshot.enemies.as_slice(), frame, &mut commands, debug) {
                owned.insert(unit.id, Rule::Kite);
            } else if self.lane.claims.get(&unit.id).is_some_and(|c| frame - c.frame < CLAIM_FRAMES) {
                // A claim stands at least this long.
            } else {
                self.release(unit.id, frame, &mut commands, debug);
            }
        }
        if tick.due() % (60 * FRAMES_PER_SECOND) == 0 && self.lane.counts != (0, 0) {
            let (claims, orders) = std::mem::take(&mut self.lane.counts);
            eprintln!("[ai {}] f={frame} micro this minute: {claims} units took over, {orders} orders", self.ai());
        }
        commands
    }

    /// H-MICRO-FLEE for one unit: `Some` when it owns the unit this tick (a step, or a hold at the edge).
    fn flee(&mut self, unit: &OwnUnit, sources: &[Source], friends_dps: f32, frame: i32, commands: &mut Vec<Command>, debug: bool) -> Option<Rule> {
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
        // A threat the brain did not price this unit against, where it is heading (or where it stands, while it has
        // not got out yet); or soldiers of theirs that would kill it before the brain looks again.
        let claimed = self.lane.claims.get(&unit.id).is_some_and(|c| c.rule == Rule::Flee);
        let unpriced = unpriced_at(next).or_else(|| if claimed { unpriced_at(unit.pos) } else { None });
        // A unit fight it would die in, and one its side is losing where it stands: with friends' fire around it
        // beating theirs, a wounded unit stays and shoots (micro-flee-ab: fleeing every fight three Pawns could win
        // halved what the army killed by minute 10 and won a game fewer; K-army-withdrawing-a-hurt-soldier-...).
        let theirs = mobile_threat_at(next);
        let outgunned = theirs > friends_dps * ODDS_TO_STAY;
        let lethal = theirs * EXPOSURE_SECONDS >= unit.health && outgunned;
        let enemies: Vec<EnemyUnit> = Vec::new();
        if let Some(why) = unpriced.map(|s| ("unpriced", s.id)).or(lethal.then_some(("lethal", UnitId(-1)))) {
            let goal = self.standing_goal(unit.id, &enemies).unwrap_or(self.home);
            let passable = self.passable();
            let step = self.lane.grid.as_ref().and_then(|g| g.lowest_within(unit.pos, STEP_RADIUS, passable, goal))?;
            let claim = self.lane.claims.get(&unit.id);
            let fresh = !claimed;
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
                self.lane.claims.insert(unit.id, Claim { rule: Rule::Flee, sent_to: step, target: None, frame });
                commands.push(Command::Move { unit: unit.id, to: step, queue: false });
            }
            return Some(Rule::Flee);
        }
        // Out of danger, but its order leads straight back in (the goal, or the way to it), or back into a fight it
        // would die in: it stands where it is until the brain orders otherwise, rather than walking in and out of
        // the tail every few seconds (micro-flee-debug2, -debug3).
        if claimed
            && let Some(goal) = self.standing_goal(unit.id, &enemies)
            && (sources.iter().any(|s| !commitment.covers(s) && s.weight >= FAINT && super::contact::to_segment(s.pos, unit.pos, goal) < s.reach + TAIL)
                || (mobile_threat_at(goal) * EXPOSURE_SECONDS >= unit.health && mobile_threat_at(goal) > friends_dps * ODDS_TO_STAY))
        {
            return Some(Rule::Flee);
        }
        None
    }

    /// H-MICRO-FAN: inside a commander's D-gun reach plus three seconds of approach, a unit whose line to the
    /// commander would pass within a shot's width of a nearer friend's line at the edge of the reach steps across
    /// its line, to the side with fewer of ours, far enough to open a gap there: the D-gun's shot passes through
    /// everything on its line (matt-plan 07 and 08: seven Pawns to one shot, twice), and lines to one point
    /// converge, so a gap here is a smaller one at the reach (matt-fan2: 48 elmos of sidestep at 500 out left the
    /// party 24 apart where the shot came, and one shot still took three). The unit nearest the commander on a line
    /// never steps, so the party fans out rather than withdrawing; commitment is not consulted, a sidestep leaves no
    /// fight. Returns who stepped or holds a fan claim inside the zone.
    fn fan(&mut self, free: &[&OwnUnit], soldiers: &[&OwnUnit], sources: &[Source], frame: i32, commands: &mut Vec<Command>, debug: bool) -> Vec<UnitId> {
        let mut owned = Vec::new();
        for commander in sources.iter().filter(|s| s.dgun > 0.0 && s.weight >= FAINT) {
            let zone = commander.dgun + DGUN_MARGIN;
            let inside: Vec<&OwnUnit> = soldiers.iter().copied().filter(|u| u.pos.dist2d(commander.pos) < zone).collect();
            for unit in free.iter().filter(|u| inside.iter().any(|i| i.id == u.id)) {
                let (dx, dz) = (unit.pos.x - commander.pos.x, unit.pos.z - commander.pos.z);
                let along = dx.hypot(dz).max(1.0);
                let (ux, uz) = (dx / along, dz / along);
                // Signed distance across the line; and what a gap here between two lines to the commander is at the
                // edge of its reach (the same when the unit is inside it).
                let across = |f: &OwnUnit| -> f32 { (f.pos.x - commander.pos.x) * -uz + (f.pos.z - commander.pos.z) * ux };
                let at_edge = |f: &OwnUnit| -> f32 { (commander.dgun / f.pos.dist2d(commander.pos).max(1.0)).min(1.0) };
                let shadowed = inside.iter().any(|f| f.id != unit.id && f.pos.dist2d(commander.pos) < along && across(f).abs() * at_edge(f) < DGUN_SHADOW);
                let claim = self.lane.claims.get(&unit.id);
                if !shadowed {
                    if claim.is_some_and(|c| c.rule == Rule::Fan && frame - c.frame < CLAIM_FRAMES) {
                        owned.push(unit.id);
                    }
                    continue;
                }
                // Across by enough to open the gap at the reach, to the side with fewer of ours within that.
                let step_len = (FAN_CLEAR / at_edge(unit)).min(FAN_STEP_MAX);
                let left = inside.iter().filter(|f| f.id != unit.id && f.pos.dist2d(unit.pos) < 2.0 * step_len && across(f) > across(unit)).count();
                let right = inside.iter().filter(|f| f.id != unit.id && f.pos.dist2d(unit.pos) < 2.0 * step_len && across(f) < across(unit)).count();
                let side = if left <= right { 1.0 } else { -1.0 };
                let step = self.snap_to_reachable(Vec3 { x: unit.pos.x + -uz * step_len * side, y: 0.0, z: unit.pos.z + ux * step_len * side });
                let fresh = claim.is_none_or(|c| c.rule != Rule::Fan);
                let reorder = claim.is_none_or(|c| c.sent_to.dist2d(step) > REORDER_DISTANCE && frame - c.frame >= REORDER_FRAMES);
                if fresh {
                    self.fire(Rule::Fan.id());
                    self.lane.counts.0 += 1;
                    if debug {
                        eprintln!(
                            "[ai {}] f={frame} micro: {}#{} fans out of the D-gun line of commander {} ({:.0} away) from ({:.0}, {:.0}) to ({:.0}, {:.0})",
                            self.ai(), self.name(unit.def), unit.id.0, commander.id.0, along, unit.pos.x, unit.pos.z, step.x, step.z
                        );
                    }
                }
                if fresh || reorder {
                    self.lane.counts.1 += 1;
                    self.lane.claims.insert(unit.id, Claim { rule: Rule::Fan, sent_to: step, target: None, frame });
                    commands.push(Command::Move { unit: unit.id, to: step, queue: false });
                }
                owned.push(unit.id);
            }
        }
        owned
    }

    /// H-MICRO-FOCUS: soldiers standing together with enemies inside their reach shoot one target at a time, the
    /// dearest for the time it takes, as many of them as it takes to kill it in a second, the rest the next. Returns
    /// who was given a target. Only targets already in a shooter's reach: an Attack order on anything else is a chase.
    fn focus(&mut self, free: &[&OwnUnit], enemies: &[EnemyUnit], frame: i32, commands: &mut Vec<Command>, debug: bool) -> Vec<UnitId> {
        let mut given = Vec::new();
        let shooters: Vec<(&OwnUnit, f32, f32)> = free.iter().filter_map(|u| {
            let (reach, dps, _) = self.sim_stats(u.def)?;
            (reach > 0.0 && dps > 0.0).then_some((*u, reach, dps))
        }).collect();
        // Targets: enemies of known type and worth inside some shooter's reach.
        let targets: Vec<(&EnemyUnit, f32)> = enemies.iter().filter_map(|e| {
            let def = e.def?;
            let metal = self.world.def(def)?.metal_cost;
            shooters.iter().any(|(u, reach, _)| u.pos.dist2d(e.pos) < reach + FOCUS_SLACK).then_some((e, metal))
        }).collect();
        if targets.is_empty() {
            return given;
        }
        // Groups: shooters with a target in reach, chained within the group radius.
        let mut engaged: Vec<usize> = (0..shooters.len()).filter(|i| targets.iter().any(|(e, _)| shooters[*i].0.pos.dist2d(e.pos) < shooters[*i].1 + FOCUS_SLACK)).collect();
        let mut assignment: HashMap<UnitId, UnitId> = HashMap::new();
        while let Some(seed) = engaged.pop() {
            let mut group = vec![seed];
            let mut i = 0;
            while i < group.len() {
                let at = shooters[group[i]].0.pos;
                let (near, far): (Vec<usize>, Vec<usize>) = engaged.drain(..).partition(|j| shooters[*j].0.pos.dist2d(at) < FOCUS_GROUP);
                group.extend(near);
                engaged = far;
                i += 1;
            }
            // Dearest target per second of the group's fire on it, until each is covered; a shooter goes to the
            // first target it can reach in that order.
            let mut unassigned: Vec<usize> = group.clone();
            // What shoots comes before what does not (raid-price-debug2: the dearest-per-second rule put Pawns on a
            // base's winds while its tower killed them; the user's rule: combatants first unless the brain has said
            // otherwise), and what the brain has told a member to attack comes first of all.
            let ordered: Vec<UnitId> = group.iter().filter_map(|j| match self.lane.standing.get(&shooters[*j].0.id)?.0 { Command::Attack { target, .. } => Some(target), _ => None }).collect();
            let mut order: Vec<(&EnemyUnit, f32, f32)> = targets.iter().map(|(e, metal)| {
                let dps: f32 = group.iter().filter(|j| shooters[**j].0.pos.dist2d(e.pos) < shooters[**j].1 + FOCUS_SLACK).map(|j| shooters[*j].2).sum();
                let score = if dps > 0.0 { metal / (e.health / dps).max(0.1) } else { 0.0 };
                let shoots = e.def.and_then(|d| self.sim_stats(d)).is_some_and(|(reach, dps, _)| reach > 0.0 && dps > 0.0);
                let rank = if ordered.contains(&e.id) { 2.0 } else if shoots { 1.0 } else { 0.0 };
                (*e, *metal, if score > 0.0 { score + rank * 1.0e6 } else { 0.0 })
            }).collect();
            order.retain(|(_, _, score)| *score > 0.0);
            order.sort_by(|a, b| b.2.total_cmp(&a.2));
            for (target, _, _) in order {
                if unassigned.is_empty() {
                    break;
                }
                let mut covered = 0.0;
                let mut i = 0;
                while i < unassigned.len() && covered < target.health / OVERKILL_SECONDS {
                    let j = unassigned[i];
                    if shooters[j].0.pos.dist2d(target.pos) < shooters[j].1 + FOCUS_SLACK {
                        assignment.insert(shooters[j].0.id, target.id);
                        covered += shooters[j].2;
                        unassigned.remove(i);
                    } else {
                        i += 1;
                    }
                }
            }
        }
        for (unit, target) in assignment {
            given.push(unit);
            let claim = self.lane.claims.get(&unit);
            if claim.is_some_and(|c| c.rule == Rule::Focus && c.target == Some(target)) {
                continue;
            }
            if claim.is_none_or(|c| c.rule != Rule::Focus) {
                self.fire(Rule::Focus.id());
                self.lane.counts.0 += 1;
            }
            self.lane.counts.1 += 1;
            if debug {
                eprintln!("[ai {}] f={frame} micro: unit {} focuses enemy {}", self.ai(), unit.0, target.0);
            }
            let pos = free.iter().find(|u| u.id == unit).map_or(Vec3::default(), |u| u.pos);
            self.lane.claims.insert(unit, Claim { rule: Rule::Focus, sent_to: pos, target: Some(target), frame });
            commands.push(Command::Attack { unit, target, queue: false });
            if let Some((order, _)) = self.lane.standing.get(&unit) {
                commands.push(queued(order));
            }
        }
        given
    }

    /// H-MICRO-KITE for one unit: the nearest soldier of theirs that can hurt it is one it outranges and is no
    /// slower than, inside its reach: while its weapon reloads it steps back to keep the enemy at the edge of its
    /// reach; when the weapon is ready it shoots it. True when it owns the unit.
    fn kite(&mut self, unit: &OwnUnit, enemies: &[EnemyUnit], frame: i32, commands: &mut Vec<Command>, debug: bool) -> bool {
        let Some((reach, dps, speed)) = self.sim_stats(unit.def) else { return false };
        if reach <= 0.0 || dps <= 0.0 {
            return false;
        }
        let nearest = enemies.iter().filter_map(|e| {
            let def = e.def?;
            let d = self.world.def(def)?;
            let (their_reach, their_dps, their_speed) = self.sim_stats(def)?;
            (d.speed > 0.0 && their_dps > 0.0 && d.build_speed == 0.0).then_some((e, their_reach, their_speed))
        }).min_by(|a, b| a.0.pos.dist2d(unit.pos).total_cmp(&b.0.pos.dist2d(unit.pos)));
        let Some((enemy, their_reach, their_speed)) = nearest else { return false };
        let distance = enemy.pos.dist2d(unit.pos);
        if !(reach >= their_reach + KITE_MARGIN && speed >= their_speed && distance < reach + FOCUS_SLACK) {
            return false;
        }
        // What the claim says now, copied out: the unit is claimed again below whatever it was.
        let claim: Option<(Rule, Vec3, Option<UnitId>, i32)> = self.lane.claims.get(&unit.id).map(|c| (c.rule, c.sent_to, c.target, c.frame));
        let fresh = claim.is_none_or(|c| c.0 != Rule::Kite);
        if fresh {
            self.fire(Rule::Kite.id());
            self.lane.counts.0 += 1;
            if debug {
                eprintln!("[ai {}] f={frame} micro: {}#{} kites enemy {} ({:.0} away, reach {reach:.0} against {their_reach:.0})", self.ai(), self.name(unit.def), unit.id.0, enemy.id.0, distance);
            }
        }
        let ready = unit.reload_frame <= frame + 1;
        if ready {
            if claim.is_some_and(|c| c.0 == Rule::Kite && c.2 == Some(enemy.id)) {
                return true;
            }
            self.lane.counts.1 += 1;
            self.lane.claims.insert(unit.id, Claim { rule: Rule::Kite, sent_to: unit.pos, target: Some(enemy.id), frame });
            commands.push(Command::Attack { unit: unit.id, target: enemy.id, queue: false });
            return true;
        }
        // Reloading: back to the edge of the reach, away from the enemy.
        let (dx, dz) = (unit.pos.x - enemy.pos.x, unit.pos.z - enemy.pos.z);
        let len = dx.hypot(dz).max(1.0);
        let back = (reach - KITE_EDGE - distance).max(0.0) + KITE_STEP;
        let step = self.snap_to_reachable(Vec3 { x: unit.pos.x + dx / len * back, y: 0.0, z: unit.pos.z + dz / len * back });
        if claim.is_some_and(|c| c.0 == Rule::Kite && c.2.is_none() && c.1.dist2d(step) < REORDER_DISTANCE && frame - c.3 < REORDER_FRAMES) {
            return true;
        }
        self.lane.counts.1 += 1;
        self.lane.claims.insert(unit.id, Claim { rule: Rule::Kite, sent_to: step, target: None, frame });
        commands.push(Command::Move { unit: unit.id, to: step, queue: false });
        true
    }

    /// Reach, damage a second and speed (elmos a second) of a unit type from the simulator's table.
    fn sim_stats(&self, def: UnitDefId) -> Option<(f32, f32, f32)> {
        let unit = &self.contacts.rules.units.list[*self.contacts.sim_defs.get(&def)?];
        Some((unit.reach(), unit.dps(), unit.speed))
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
        let stats = |def: UnitDefId| -> Option<(f32, f32, f32)> {
            let unit = &rules.units.list[*self.contacts.sim_defs.get(&def)?];
            let dgun = unit.weapons.iter().filter(|w| w.command_fire && !w.paralyzer).map(|w| w.range).fold(0.0, f32::max);
            (unit.reach() > 0.0).then(|| (unit.reach(), unit.dps(), dgun))
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
            let Some((reach, dps, dgun)) = stats(def) else { continue };
            let mobile = d.speed > 0.0;
            if mobile {
                self.lane.seen.insert(enemy.id, (def, enemy.pos, frame));
            }
            sources.push(Source { id: enemy.id, pos: enemy.pos, reach, weight: dps, mobile, commander: is_commander(def), dgun });
        }
        for (id, (def, pos, _)) in &self.enemy_buildings {
            if sources.iter().any(|s| s.id == *id) {
                continue;
            }
            let Some((reach, dps, _)) = stats(*def) else { continue };
            sources.push(Source { id: *id, pos: *pos, reach, weight: dps, mobile: false, commander: false, dgun: 0.0 });
        }
        self.lane.seen.retain(|id, (_, _, at)| frame - *at < MEMORY_FRAMES && self.enemy_soldiers.contains_key(id));
        for (id, (def, pos, at)) in &self.lane.seen {
            if sources.iter().any(|s| s.id == *id) {
                continue;
            }
            let Some((reach, dps, dgun)) = stats(*def) else { continue };
            let fade = 1.0 - (frame - at) as f32 / MEMORY_FRAMES as f32;
            sources.push(Source { id: *id, pos: *pos, reach, weight: dps * fade, mobile: true, commander: is_commander(*def), dgun });
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
