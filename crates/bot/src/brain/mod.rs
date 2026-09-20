//! Heuristic brain. One instance per AI; `decide` runs once per tick.
//!
//! The game ends when a commander dies, so the commander builds the opening and then stays
//! home; constructors do the expanding. Builders coordinate through `jobs` so that two of them
//! never pick the same one-off building in the same breath.

mod army;
mod briefing;
mod combat;
mod economy;
pub mod journal;
mod squads;
mod wake;
mod roster;
mod routes;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;

use bot_protocol::{Command, Event, OwnUnit, Tick, UnitDefId, UnitId, Vec3};

use crate::strategist::shared::{Directives, Shared};
use crate::world::World;
use roster::{Kit, ROSTERS};

const FRAMES_PER_SECOND: i32 = 30;
/// The static map description is published during the first few ticks, once home is known.
const TICK_FRAMES_HINT: i32 = 10 * FRAMES_PER_SECOND;

pub struct Brain {
    world: World,
    kit: Option<Kit>,
    home: Vec3,
    /// Where the enemy is presumed to have started.
    enemy_start: Vec3,
    /// What each busy builder was last told to build, so others can plan around it.
    jobs: HashMap<UnitId, UnitDefId>,
    /// Metal spot index to the frame it was claimed at.
    spot_claims: HashMap<usize, i32>,
    /// Walking distances over the terrain, once our faction (and so our movement class) is known.
    routes: Option<routes::Routes>,
    /// Where our units died lately, for H-ECO-RECLAIM: place and frame (enemy wrecks lie in the same places). Deaths close together are one site.
    wreck_sites: Vec<(Vec3, i32)>,
    /// Metal spots where an extractor or a constructor of ours died, and until which frame they stay closed.
    hot_spots: Vec<(Vec3, i32)>,
    /// Units a constructor has been sent to repair, and when, so that one goes to each.
    repair_claims: HashMap<UnitId, i32>,
    /// When something of ours last died on our side of the map; no wave leaves while that is fresh.
    last_loss_at_home_frame: i32,
    matchups: combat::Matchups,
    army: army::Army,
    squads: squads::Squads,
    wake: wake::WakeState,
    /// The commander's unit mix (unit name to weight); empty means the heuristic batch.
    production_weights: std::collections::BTreeMap<String, u32>,
    /// Turrets the commander asked for, oldest first.
    turret_requests: Vec<Vec3>,
    /// How often each heuristic (docs/heuristics.md) acted since the last status line.
    fired: BTreeMap<&'static str, u32>,
    /// Present when a strategist is attached; the brain publishes to it and reads directives from it.
    strategist: Option<Arc<Shared>>,
    /// This tick's unexpired directives; empty without a strategist.
    directives: Directives,
    /// Enemy buildings seen and not known to be destroyed: definition, position, frame last seen.
    enemy_buildings: HashMap<UnitId, (UnitDefId, Vec3, i32)>,
    recent_events: VecDeque<String>,
    /// Our units as last seen, to name what a destroyed-unit event refers to.
    known_units: HashMap<UnitId, (UnitDefId, Vec3)>,
    /// Every enemy unit's type once seen, so a killer that has left sight still has a name.
    enemy_defs: HashMap<UnitId, UnitDefId>,
    /// This minute's fight ledger for the log: "lost X to Y near home" and "killed Y", with counts.
    fight_ledger: std::collections::BTreeMap<String, u32>,
    /// This minute's soldier move failures by 200-elmo cell.
    stuck_cells: HashMap<(i32, i32), u32>,
    /// Frames at which we lost an extractor, within the trigger cooldown.
    extractor_losses: VecDeque<i32>,
    last_station: Vec3,
    /// Each builder's latest order: frame, what, and near where. For spotting orders that never start.
    last_orders: HashMap<UnitId, (i32, UnitDefId, Vec3)>,
    dropped_orders: u32,
    move_failures: u32,
    /// Places a builder failed to walk to, with the frame until which to avoid them.
    unreachable: Vec<(Vec3, i32)>,
    /// Heuristic IDs switched off for an ablation run (`WITHIN_REASON_DISABLE=H-A,H-B`).
    disabled: Vec<String>,
    last_trigger_frame: HashMap<&'static str, i32>,
    /// Decisions since `main` last collected them for the match record.
    journal: journal::Journal,
}

impl Brain {
    pub fn new(world: World, strategist: Option<Arc<Shared>>) -> Self {
        let h = &world.hello;
        eprintln!(
            "[ai {}] team {} on {} ({}x{}), {} unit defs, {} metal spots",
            h.ai_id, h.team, h.map.name, h.map.width, h.map.height, h.unit_defs.len(), h.metal_spots.len()
        );
        Brain {
            world,
            kit: None,
            home: Vec3::default(),
            enemy_start: Vec3::default(),
            jobs: HashMap::new(),
            spot_claims: HashMap::new(),
            routes: None,
            wreck_sites: Vec::new(),
            hot_spots: Vec::new(),
            repair_claims: HashMap::new(),
            last_loss_at_home_frame: i32::MIN / 2,
            matchups: Default::default(),
            army: army::Army::default(),
            squads: Default::default(),
            wake: Default::default(),
            production_weights: Default::default(),
            turret_requests: Vec::new(),
            fired: BTreeMap::new(),
            strategist,
            directives: Directives::default(),
            enemy_buildings: HashMap::new(),
            recent_events: VecDeque::new(),
            known_units: HashMap::new(),
            enemy_defs: HashMap::new(),
            fight_ledger: Default::default(),
            stuck_cells: HashMap::new(),
            extractor_losses: VecDeque::new(),
            last_station: Vec3::default(),
            last_orders: HashMap::new(),
            dropped_orders: 0,
            move_failures: 0,
            unreachable: Vec::new(),
            disabled: std::env::var("WITHIN_REASON_DISABLE").map_or_else(|_| Vec::new(), |ids| ids.split(',').map(str::to_string).collect()),
            last_trigger_frame: HashMap::new(),
            journal: Default::default(),
        }
    }

    pub fn decide(&mut self, tick: &Tick) -> Vec<Command> {
        if self.kit.is_none() {
            self.adopt_faction(&tick.snapshot.own_units);
        }
        let Some(kit) = self.kit else { return Vec::new() };
        self.read_directives(tick.frame);
        self.track_enemy_buildings(tick);
        self.track_losses(tick, &kit);
        let mut commands = Vec::new();
        self.protect_commander(tick, &kit, &mut commands);
        self.run_economy(tick, &kit, &mut commands);
        self.run_army(tick, &kit, &mut commands);
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
            self.enemy_start = self.world.mirrored(unit.pos);
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

    /// A point `distance` elmos from home towards the enemy: along the walking route when we know the terrain, and
    /// always on ground our soldiers can reach. A negative distance is behind home, away from the enemy.
    fn forward_of_home(&self, distance: f32) -> Vec3 {
        if distance > 0.0
            && let Some(point) = self.on_the_way_to(self.enemy_start, distance)
        {
            return point;
        }
        let (dx, dz) = (self.enemy_start.x - self.home.x, self.enemy_start.z - self.home.z);
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
                commands.push(Command::Move { unit: commander.id, to: self.home, queue: false });
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
        if tick.frame % (60 * FRAMES_PER_SECOND) == 0 {
            let s = &tick.snapshot;
            let count = |def: UnitDefId| s.own_units.iter().filter(|u| u.def == def).count();
            eprintln!(
                "[ai {}] f={} ({:.0} min) metal {:.0} (+{:.1}/-{:.1}) energy {:.0}/{:.0} (+{:.0}/-{:.0}) | mex {} labs {} cons {} army {} | enemies visible {}",
                self.ai(), tick.frame, tick.frame as f32 / 1800.0, s.metal.current, s.metal.income, s.metal.usage,
                s.energy.current, s.energy.storage, s.energy.income, s.energy.usage,
                count(kit.extractor), count(kit.lab), count(kit.constructor),
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
