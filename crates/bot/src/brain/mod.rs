//! Heuristic brain. One instance per AI; `decide` runs once per tick.
//!
//! The game ends when a commander dies, so the commander builds the opening and then stays
//! home; constructors do the expanding. Builders coordinate through `jobs` so that two of them
//! never pick the same one-off building in the same breath.

mod allies;
mod army;
mod bases;
mod briefing;
mod combat;
mod contact;
mod economy;
pub mod journal;
mod march;
mod micro;
pub mod pianist;
mod planner;
mod raid;
mod scout;
mod reclaim;
mod squads;
mod territory;
mod threat;
mod tier2;
mod wake;
mod roster;
mod routes;

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::Arc;

use bot_protocol::{Command, Event, OwnUnit, Tick, UnitDefId, UnitId, Vec3};

use crate::strategist::shared::{Directives, Shared};
use crate::world::World;
use roster::{Kit, ROSTERS};

const FRAMES_PER_SECOND: i32 = 30;
/// Frames between runs of the whole brain (`think`), 2 Hz: the shim's ticks come more often than that, for the
/// control lane (`micro.rs`), and must divide this (`docs/design/2026-09-20-micro-lane.md`).
pub(crate) const BRAIN_FRAMES: i32 = 15;
/// The static map description is published during the first few seconds, once home is known.
const TICK_FRAMES_HINT: i32 = 10 * FRAMES_PER_SECOND;

pub struct Brain {
    world: World,
    kit: Option<Kit>,
    home: Vec3,
    /// This tick's allied units, where each allied team's commander was first seen, and when each metal spot last
    /// held an allied extractor (`allies.rs`).
    allies: Vec<bot_protocol::AllyUnit>,
    ally_starts: HashMap<i32, Vec3>,
    ally_spot_held: HashMap<usize, i32>,
    /// Shared with the other seats of ours in this game (`team.rs`), and what they posted last tick.
    board: Arc<crate::team::TeamBoard>,
    team_mates: crate::team::Others,
    /// What this seat posts this tick; the army fills in its part.
    team_post: crate::team::Post,
    /// One per enemy seat: guessed, found or dead (`bases.rs`).
    enemy_bases: Vec<bases::EnemyBase>,
    /// What each busy builder was last told to build, so others can plan around it.
    jobs: HashMap<UnitId, UnitDefId>,
    /// Metal spot index to the frame it was claimed at.
    spot_claims: HashMap<usize, i32>,
    /// Walking distances over the terrain, once our faction (and so our movement class) is known.
    routes: Option<routes::Routes>,
    /// Wrecks seen, the fields they lie in and who works them (`reclaim.rs`).
    reclaim: reclaim::Reclaim,
    /// The rolling economy plan (`planner.rs`), and whether planning is over for this game (the commander is gone).
    planner: Option<planner::Planner>,
    planner_off: bool,
    /// The pianist (`pianist/`): Jev plays every actor from the player's instructions; no decision heuristic runs.
    pianist: Option<pianist::Pianist>,
    /// Whose ground is whose (`territory.rs`).
    territory: territory::Territory,
    /// Units a constructor has been sent to repair, and when, so that one goes to each.
    repair_claims: HashMap<UnitId, i32>,
    /// H-T2-MOHO: the extractor each advanced constructor is upgrading.
    upgrade_claims: HashMap<UnitId, Vec3>,
    /// When something of ours last died on our side of the map; no wave leaves while that is fresh.
    last_loss_at_home_frame: i32,
    matchups: combat::Matchups,
    army: army::Army,
    /// H-ARMY-CONTACT: the enemy parties on our ground and who answers each (`contact.rs`).
    contacts: contact::Contacts,
    /// What the bot says in the game chat at its first orders: commit and settings (`main.rs`), once.
    banner: Option<String>,
    /// When each metal spot was last in sight, and the raider out looking (`scout.rs`).
    spots: scout::Spots,
    squads: squads::Squads,
    wake: wake::WakeState,
    /// The commander's unit mix (unit name to weight); empty means the heuristic batch.
    production_weights: std::collections::BTreeMap<String, u32>,
    /// Turrets the commander asked for, oldest first.
    raid: raid::Raid,
    turret_requests: Vec<Vec3>,
    /// D-EXPANSION-PLAN: the commander's spots to take first, in order, and spots to leave alone (indices into the map's list).
    spot_priority: Vec<usize>,
    spot_avoid: Vec<usize>,
    /// How often each heuristic (docs/heuristics.md) acted since the last status line.
    fired: BTreeMap<&'static str, u32>,
    /// Present when a strategist is attached; the brain publishes to it and reads directives from it.
    strategist: Option<Arc<Shared>>,
    /// This tick's unexpired directives; empty without a strategist.
    directives: Directives,
    /// Enemy buildings seen and not known to be destroyed: definition, position, frame last seen.
    enemy_buildings: HashMap<UnitId, (UnitDefId, Vec3, i32)>,
    /// Buildings `forget_razed_buildings` dropped this tick, for the team board.
    razed: Vec<UnitId>,
    /// Every enemy soldier seen and not known dead, with when it was last seen: what we know of their army, a floor.
    enemy_soldiers: HashMap<UnitId, (UnitDefId, i32)>,
    /// Where and when the enemy commander was last seen: killing it wins the game.
    enemy_commander_seen: Option<(Vec3, i32)>,
    recent_events: VecDeque<String>,
    /// Our units as last seen, to name what a destroyed-unit event refers to.
    known_units: HashMap<UnitId, (UnitDefId, Vec3)>,
    /// Our units still being built at the last look, with how far along they were (health share): a `UnitDestroyed`
    /// for one of these with no attacker is a nanoframe its builder abandoned, not a loss to the enemy.
    unfinished: HashMap<UnitId, f32>,
    /// The abandoned frames among this think's `UnitDestroyed` events (`track_losses` fills it; the pianist's
    /// housekeeping runs after and asks).
    abandoned_now: HashMap<UnitId, f32>,
    /// Every enemy unit's type once seen, so a killer that has left sight still has a name.
    enemy_defs: HashMap<UnitId, UnitDefId>,
    /// This minute's fight ledger for the log: "lost X to Y near home" and "killed Y", with counts.
    fight_ledger: std::collections::BTreeMap<String, u32>,
    /// (frame, metal of ours destroyed, metal of theirs we saw destroyed), one entry per death: what the fighting costs
    /// each side, which neither army count shows.
    trade_log: Vec<(i32, f32, f32)>,
    /// Extractors lost so far at each metal spot (its index in the map's list).
    spot_losses: HashMap<usize, u32>,
    /// This minute's soldier move failures by 200-elmo cell.
    stuck_cells: HashMap<(i32, i32), u32>,
    /// Frames at which we lost an extractor, within the trigger cooldown.
    extractor_losses: VecDeque<i32>,
    last_station: Vec3,
    /// Each builder's latest order: frame, what, and near where. For spotting orders that never start.
    last_orders: HashMap<UnitId, (i32, UnitDefId, Vec3)>,
    /// A plan step given to a busy builder as a queued build, until the engine starts it (its job then) or the builder
    /// goes idle without it (wound back): what, near where, since when.
    queued: HashMap<UnitId, (UnitDefId, Vec3, i32)>,
    /// Per builder, the frame its current order produced a nanoframe: the queue pass waits for that, not for time.
    job_started: HashMap<UnitId, i32>,
    /// Metal spots (by index) where an extractor offset toward the builder was refused: the exact centre from then on.
    centre_only: HashSet<usize>,
    dropped_orders: u32,
    move_failures: u32,
    /// Places a builder failed to walk to, with the frame until which to avoid them.
    unreachable: Vec<(Vec3, i32)>,
    /// Heuristic IDs switched off for an ablation run (`WITHIN_REASON_DISABLE=H-A,H-B`).
    disabled: Vec<String>,
    last_trigger_frame: HashMap<&'static str, i32>,
    /// Decisions since `main` last collected them for the match record.
    journal: journal::Journal,
    /// The control lane's state (`micro.rs`): standing orders, commitments, claims, the threat grid.
    lane: micro::Lane,
    /// Events from the ticks since `think` last ran, in order, for its next run.
    carried_events: Vec<Event>,
    /// Ticks that came late (`Tick::late`) and the most frames one waited, since the last per-minute line.
    late_ticks: (u32, i32),
}

impl Brain {
    pub fn new(world: World, strategist: Option<Arc<Shared>>, board: Arc<crate::team::TeamBoard>, banner: String, pianist: Option<pianist::Pianist>) -> Self {
        let h = &world.hello;
        eprintln!(
            "[ai {}] team {} on {} ({}x{}), {} unit defs, {} metal spots",
            h.ai_id, h.team, h.map.name, h.map.width, h.map.height, h.unit_defs.len(), h.metal_spots.len()
        );
        Brain {
            world,
            kit: None,
            home: Vec3::default(),
            board,
            team_mates: Default::default(),
            team_post: Default::default(),
            allies: Vec::new(),
            ally_starts: HashMap::new(),
            ally_spot_held: HashMap::new(),
            enemy_bases: Vec::new(),
            jobs: HashMap::new(),
            spot_claims: HashMap::new(),
            routes: None,
            reclaim: Default::default(),
            planner: None,
            planner_off: false,
            pianist,
            territory: Default::default(),
            repair_claims: HashMap::new(),
            upgrade_claims: HashMap::new(),
            last_loss_at_home_frame: i32::MIN / 2,
            matchups: Default::default(),
            army: army::Army::default(),
            contacts: Default::default(),
            spots: scout::Spots::default(),
            banner: Some(banner),
            squads: Default::default(),
            wake: Default::default(),
            production_weights: Default::default(),
            raid: raid::Raid::default(),
            turret_requests: Vec::new(),
            spot_priority: Vec::new(),
            spot_avoid: Vec::new(),
            fired: BTreeMap::new(),
            strategist,
            directives: Directives::default(),
            enemy_buildings: HashMap::new(),
            razed: Vec::new(),
            enemy_soldiers: HashMap::new(),
            enemy_commander_seen: None,
            recent_events: VecDeque::new(),
            known_units: HashMap::new(),
            unfinished: HashMap::new(),
            abandoned_now: HashMap::new(),
            enemy_defs: HashMap::new(),
            fight_ledger: Default::default(),
            trade_log: Vec::new(),
            spot_losses: HashMap::new(),
            stuck_cells: HashMap::new(),
            extractor_losses: VecDeque::new(),
            last_station: Vec3::default(),
            last_orders: HashMap::new(),
            queued: HashMap::new(),
            job_started: HashMap::new(),
            centre_only: HashSet::new(),
            dropped_orders: 0,
            move_failures: 0,
            unreachable: Vec::new(),
            disabled: std::env::var("WITHIN_REASON_DISABLE").map_or_else(|_| Vec::new(), |ids| ids.split(',').map(str::to_string).collect()),
            last_trigger_frame: HashMap::new(),
            journal: Default::default(),
            lane: Default::default(),
            carried_events: Vec::new(),
            late_ticks: (0, 0),
        }
    }

    /// One tick: the control lane every time, the whole brain (`think`) on the frames due at its own interval, with
    /// every event since it last ran.
    pub fn decide(&mut self, tick: &Tick) -> Vec<Command> {
        if tick.late > 0 {
            self.late_ticks = (self.late_ticks.0 + 1, self.late_ticks.1.max(tick.late));
        }
        if tick.due() % BRAIN_FRAMES != 0 {
            self.carried_events.extend(tick.events.iter().cloned());
            return self.micro(tick);
        }
        let mut commands = if self.carried_events.is_empty() {
            self.think(tick)
        } else {
            let mut events = std::mem::take(&mut self.carried_events);
            events.extend(tick.events.iter().cloned());
            let whole = Tick { frame: tick.frame, late: tick.late, events, snapshot: tick.snapshot.clone() };
            self.think(&whole)
        };
        self.note_standing_orders(&commands, tick.frame);
        self.note_commitments();
        commands.extend(self.micro(tick));
        commands
    }

    fn think(&mut self, tick: &Tick) -> Vec<Command> {
        if self.kit.is_none() {
            self.adopt_faction(&tick.snapshot.own_units);
        }
        let Some(kit) = self.kit else { return Vec::new() };
        self.read_directives(tick.frame);
        self.receive_spot_fields();
        self.note_allies(tick);
        self.track_enemy_buildings(tick);
        self.survey_spots(tick);
        self.track_enemy_bases(tick);
        self.update_territory(tick, &kit);
        self.track_wrecks(tick);
        self.track_losses(tick, &kit);
        let mut commands = Vec::new();
        // The game names AIs at random (ai_namer.lua), so the bot says who it is, once the engine takes orders.
        if tick.frame >= 2 * FRAMES_PER_SECOND
            && let Some(banner) = self.banner.take()
        {
            let side = self.world.hello.teams.iter().find(|t| t.team == self.world.hello.team).map_or("?", |t| t.side.as_str());
            commands.push(Command::Say { text: format!("{{name}} is {banner} | seat ai{} team {} {side}", self.world.hello.ai_id, self.world.hello.team) });
        }
        if self.pianist.is_some() {
            self.run_pianist(tick, &kit, &mut commands);
        } else {
            self.protect_commander(tick, &kit, &mut commands);
            self.run_economy(tick, &kit, &mut commands);
            self.run_army(tick, &kit, &mut commands);
        }
        self.exchange_with_team(tick);
        self.journal_intent();
        self.report(tick, &kit);
        self.publish_briefing(tick, &kit);
        self.wake_commander_if_due(tick, &kit);
        commands
    }

    /// Picks the faction from whichever commander we own; does nothing until one exists.
    fn adopt_faction(&mut self, own: &[OwnUnit]) {
        for roster in &ROSTERS {
            let Some(commander) = self.world.def_named(roster.commander) else { continue };
            let Some(unit) = own.iter().find(|u| u.def == commander) else { continue };
            match roster.resolve(&self.world) {
                Ok(kit) => self.kit = Some(kit),
                Err(missing) => {
                    eprintln!("[ai {}] this game has no unit named {missing}", self.ai());
                    return;
                }
            }
            self.home = unit.pos;
            self.guess_enemy_bases();
            eprintln!("[ai {}] playing {} from ({:.0}, {:.0})", self.ai(), roster.commander, unit.pos.x, unit.pos.z);
            if let Some(kit) = self.kit {
                self.survey(&kit);
            }
            return;
        }
    }

    /// False when an ablation run has switched this heuristic off.
    fn enabled(&self, rule: &str) -> bool {
        !self.disabled.iter().any(|id| id == rule)
    }

    fn fire(&mut self, rule: &'static str) {
        *self.fired.entry(rule).or_default() += 1;
        self.journal.rule(rule);
    }

    /// A point `distance` elmos from home towards the nearest enemy base: along the walking route when we know the terrain, and
    /// always on ground our soldiers can reach. A negative distance is behind home, away from the enemy.
    fn forward_of_home(&self, distance: f32) -> Vec3 {
        if distance > 0.0
            && let Some(point) = self.on_the_way_to(self.enemy_base(self.home), distance)
        {
            return point;
        }
        let enemy = self.enemy_base(self.home);
        let (dx, dz) = (enemy.x - self.home.x, enemy.z - self.home.z);
        let len = dx.hypot(dz).max(1.0);
        self.snap_to_reachable(Vec3 { x: self.home.x + dx / len * distance, y: 0.0, z: self.home.z + dz / len * distance })
    }

    /// A hurt commander away from home walks back; losing it loses the game.
    fn protect_commander(&mut self, tick: &Tick, kit: &Kit, commands: &mut Vec<Command>) {
        const RETREAT_HEALTH: f32 = 0.7;
        const SAFE_RADIUS: f32 = 350.0;
        let damaged = |unit: UnitId| tick.events.iter().any(|e| matches!(e, Event::UnitDamaged { unit: u, .. } if *u == unit));
        for commander in tick.snapshot.own_units.iter().filter(|u| u.def == kit.commander) {
            // Who is hurting the commander: the one fact a lost game's log must hold.
            for event in &tick.events {
                let Event::UnitDamaged { unit, attacker, damage } = *event else { continue };
                if unit != commander.id {
                    continue;
                }
                let enemy = attacker.and_then(|id| tick.snapshot.enemies.iter().find(|e| e.id == id));
                let who = enemy.and_then(|e| e.def).map_or("unseen", |d| self.name(d));
                let range = enemy.map_or(-1.0, |e| e.pos.dist2d(commander.pos));
                eprintln!(
                    "[ai {}] f={} commander hit for {damage:.0} by {who} from {range:.0} away, {:.0} health left",
                    self.ai(), tick.frame, commander.health
                );
            }
            // H-ECO-REPAIR: a badly hurt commander does not wait for a constructor to fall idle; the nearest one drops
            // what it is doing.
            let badly_hurt = commander.health < commander.max_health * 0.5;
            if badly_hurt && self.enabled("H-ECO-REPAIR") && !self.repair_claims.contains_key(&commander.id) {
                let medic = tick
                    .snapshot
                    .own_units
                    .iter()
                    .filter(|u| u.def == kit.constructor && !u.being_built && u.pos.dist2d(commander.pos) < 1200.0)
                    .min_by(|a, b| a.pos.dist2d(commander.pos).total_cmp(&b.pos.dist2d(commander.pos)));
                if let Some(medic) = medic {
                    self.fire("H-ECO-REPAIR");
                    self.repair_claims.insert(commander.id, tick.frame);
                    self.jobs.insert(medic.id, kit.commander);
                    commands.push(Command::Repair { unit: medic.id, target: commander.id, queue: false });
                }
            }
            let hurt = commander.health < commander.max_health * RETREAT_HEALTH;
            if hurt && damaged(commander.id) && commander.pos.dist2d(self.home) > SAFE_RADIUS {
                eprintln!("[ai {}] f={} commander retreats at {:.0} health", self.ai(), tick.frame, commander.health);
                self.fire("H-COM-RETREAT");
                let percent = commander.health / commander.max_health * 100.0;
                self.trigger("commander", tick.frame, format!("Our commander is under fire away from home ({percent:.0}% health)."));
                self.jobs.remove(&commander.id);
                // Round known threats (routing design, the one waypoint case): the safe way's midpoint first when
                // the safe way home parts from the straight one, home queued after it.
                match self.commander_waypoint_home(commander.pos) {
                    Some(waypoint) => {
                        eprintln!("[ai {}] f={} commander retreats round known threats by ({:.0}, {:.0})", self.ai(), tick.frame, waypoint.x, waypoint.z);
                        commands.push(Command::Move { unit: commander.id, to: waypoint, queue: false });
                        commands.push(Command::Move { unit: commander.id, to: self.home, queue: true });
                    }
                    None => commands.push(Command::Move { unit: commander.id, to: self.home, queue: false }),
                }
            }
        }
    }

    fn report(&mut self, tick: &Tick, kit: &Kit) {
        for event in &tick.events {
            match *event {
                Event::BuildSiteNotFound { unit, def } => {
                    eprintln!("[ai {}] f={} no site for {} (builder {})", self.ai(), tick.frame, self.name(def), unit.0);
                }
                Event::CommandRejected { unit, code } => {
                    eprintln!("[ai {}] f={} engine rejected command for unit {} (code {code})", self.ai(), tick.frame, unit.0);
                }
                _ => {}
            }
        }
        if tick.due() % (60 * FRAMES_PER_SECOND) == 0 {
            let s = &tick.snapshot;
            let count = |def: UnitDefId| s.own_units.iter().filter(|u| u.def == def).count();
            eprintln!(
                "[ai {}] f={} ({:.0} min) metal {:.0} (+{:.1}/-{:.1}) energy {:.0}/{:.0} (+{:.0}/-{:.0}) | mex {} labs {} cons {} army {} | enemies visible {}",
                self.ai(), tick.frame, tick.frame as f32 / 1800.0, s.metal.current, s.metal.income, s.metal.usage,
                s.energy.current, s.energy.storage, s.energy.income, s.energy.usage,
                count(kit.extractor) + count(kit.advanced_extractor), count(kit.lab) + count(kit.advanced_lab), count(kit.constructor) + count(kit.advanced_constructor),
                s.own_units.iter().filter(|u| self.is_army(u, kit)).count(), s.enemies.len()
            );
            if !self.stuck_cells.is_empty() {
                let mut cells: Vec<_> = std::mem::take(&mut self.stuck_cells).into_iter().collect();
                cells.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
                let worst: Vec<String> = cells.iter().take(4).map(|((x, z), n)| format!("({}, {}) x{n}", x * 200 + 100, z * 200 + 100)).collect();
                eprintln!("[ai {}] f={} soldiers' moves failed around: {}", self.ai(), tick.frame, worst.join(", "));
            }
            if !self.fight_ledger.is_empty() {
                let ledger: Vec<String> = std::mem::take(&mut self.fight_ledger).into_iter().map(|(what, n)| format!("{what} x{n}")).collect();
                eprintln!("[ai {}] f={} fights: {}", self.ai(), tick.frame, ledger.join(", "));
            }
            eprintln!("[ai {}] f={} dropped build orders so far: {}, move failures: {}", self.ai(), tick.frame, self.dropped_orders, self.move_failures);
            if self.late_ticks.0 > 0 {
                let (count, most) = std::mem::take(&mut self.late_ticks);
                eprintln!("[ai {}] f={} ticks late this minute: {count}, the latest by {most} frames", self.ai(), tick.frame);
            }
            let rules: Vec<String> = self.fired.iter().map(|(rule, n)| format!("{rule}={n}")).collect();
            eprintln!("[ai {}] f={} rules: {}", self.ai(), tick.frame, rules.join(" "));
            self.fired.clear();
        }
    }

    /// Armed, mobile, and not a builder.
    fn is_army(&self, unit: &OwnUnit, kit: &Kit) -> bool {
        unit.def != kit.commander
            && self.world.def(unit.def).is_some_and(|d| d.weapon_count > 0 && d.speed > 0.0 && d.build_speed == 0.0)
    }

    fn ai(&self) -> i32 {
        self.world.hello.ai_id
    }

    fn name(&self, def: UnitDefId) -> &str {
        self.world.def(def).map_or("?", |d| d.name.as_str())
    }
}
