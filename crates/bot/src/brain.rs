//! MVP heuristic: basic economy, one lab, a stream of cheap units, attack in waves.
//! Deliberately weak; it exists to exercise the shim/bot loop end to end (see `DESIGN.md`).

use std::collections::HashMap;

use bot_protocol::{BuildSite, Command, Event, OwnUnit, Tick, UnitDefId, Vec3};

use crate::world::World;

const FRAMES_PER_SECOND: i32 = 30;
const WAVE_SIZE: usize = 6;
const MAX_CONSTRUCTORS: usize = 3;
const MAX_ENERGY_BUILDINGS: usize = 12;
const MAX_LABS: usize = 4;
/// Stored metal above which the base is under-spending and wants another lab.
const FLOATING_METAL: f32 = 600.0;
/// How long a metal spot stays reserved for a builder that was sent to it.
const SPOT_CLAIM_FRAMES: i32 = 40 * FRAMES_PER_SECOND;
/// A metal extractor this close to a spot occupies it.
const SPOT_OCCUPIED_RADIUS: f32 = 60.0;
/// An idle army unit this close to its target has arrived and needs a new one.
const ARRIVED_RADIUS: f32 = 400.0;

/// Unit names for one faction.
struct Roster {
    commander: &'static str,
    extractor: &'static str,
    energy: &'static str,
    lab: &'static str,
    raider: &'static str,
    constructor: &'static str,
}

const ROSTERS: [Roster; 2] = [
    Roster { commander: "armcom", extractor: "armmex", energy: "armsolar", lab: "armlab", raider: "armpw", constructor: "armck" },
    Roster { commander: "corcom", extractor: "cormex", energy: "corsolar", lab: "corlab", raider: "corak", constructor: "corck" },
];

/// A roster resolved against the running game's unit definitions.
#[derive(Clone, Copy)]
struct Kit {
    extractor: UnitDefId,
    energy: UnitDefId,
    lab: UnitDefId,
    raider: UnitDefId,
    constructor: UnitDefId,
}

pub struct Brain {
    world: World,
    kit: Option<Kit>,
    home: Vec3,
    /// Metal spot index to the frame it was claimed at.
    spot_claims: HashMap<usize, i32>,
    attack_target: Vec3,
    /// Next metal spot (by distance from the enemy's presumed start) to sweep when the army finds nothing.
    sweep_index: usize,
}

impl Brain {
    pub fn new(world: World) -> Self {
        let h = &world.hello;
        eprintln!(
            "[ai {}] team {} on {} ({}x{}), {} unit defs, {} metal spots",
            h.ai_id, h.team, h.map.name, h.map.width, h.map.height, h.unit_defs.len(), h.metal_spots.len()
        );
        Brain {
            world,
            kit: None,
            home: Vec3::default(),
            spot_claims: HashMap::new(),
            attack_target: Vec3::default(),
            sweep_index: 0,
        }
    }

    pub fn decide(&mut self, tick: &Tick) -> Vec<Command> {
        self.report(tick);
        if self.kit.is_none() {
            self.adopt_faction(&tick.snapshot.own_units);
        }
        let Some(kit) = self.kit else { return Vec::new() };
        let own = &tick.snapshot.own_units;
        let count = |def: UnitDefId| own.iter().filter(|u| u.def == def).count();
        let (extractors, energy, labs, constructors) =
            (count(kit.extractor), count(kit.energy), count(kit.lab), count(kit.constructor));

        if let Some(enemy) = tick.snapshot.enemies.first() {
            self.attack_target = enemy.pos;
        }

        let mut commands = Vec::new();
        let mut planned_extractors = extractors;
        let mut planned_energy = energy;
        let mut planned_labs = labs;
        let mut idle_army = Vec::new();
        for unit in own.iter().filter(|u| u.idle && !u.being_built) {
            let Some(def) = self.world.def(unit.def) else { continue };
            let (is_builder, is_mobile, is_armed) = (def.build_speed > 0.0, def.speed > 0.0, def.weapon_count > 0);
            if is_builder && is_mobile {
                // What the base is shortest of, in opening order.
                let want = if planned_extractors < 2 {
                    kit.extractor
                } else if planned_energy < 2 {
                    kit.energy
                } else if planned_labs < 1 || (tick.snapshot.metal.current > FLOATING_METAL && planned_labs < MAX_LABS) {
                    kit.lab
                } else if planned_extractors <= planned_energy || planned_energy >= MAX_ENERGY_BUILDINGS {
                    kit.extractor
                } else {
                    kit.energy
                };
                let site = if want == kit.extractor {
                    self.claim_spot(unit.pos, own, kit.extractor, tick.frame)
                } else {
                    None
                };
                let (def_id, site) = match (want == kit.extractor, site) {
                    (true, Some(spot)) => {
                        planned_extractors += 1;
                        (want, BuildSite { near: spot, search_radius: 100.0, min_dist: 0 })
                    }
                    // No free metal spot left: energy is never wasted.
                    (true, None) => {
                        planned_energy += 1;
                        (kit.energy, BuildSite { near: self.home, search_radius: 1200.0, min_dist: 3 })
                    }
                    (false, _) => {
                        if want == kit.lab { planned_labs += 1 } else { planned_energy += 1 }
                        (want, BuildSite { near: self.home, search_radius: 1200.0, min_dist: 3 })
                    }
                };
                eprintln!("[ai {}] f={} {} builds {}", self.ai(), tick.frame, self.name(unit.def), self.name(def_id));
                commands.push(Command::Build { unit: unit.id, def: def_id, site: Some(site), queue: false });
            } else if is_builder {
                let batch = if constructors < MAX_CONSTRUCTORS { [kit.constructor, kit.raider] } else { [kit.raider; 2] };
                for def_id in batch.into_iter().chain([kit.raider; 3]) {
                    commands.push(Command::Build { unit: unit.id, def: def_id, site: None, queue: true });
                }
            } else if is_armed && is_mobile {
                idle_army.push(unit);
            }
        }

        self.command_army(&idle_army, tick, &mut commands);
        commands
    }

    fn command_army(&mut self, idle_army: &[&OwnUnit], tick: &Tick, commands: &mut Vec<Command>) {
        if idle_army.is_empty() {
            return;
        }
        let arrived = idle_army.iter().any(|u| u.pos.dist2d(self.attack_target) < ARRIVED_RADIUS);
        if arrived && tick.snapshot.enemies.is_empty() {
            // Nothing here: sweep metal spots, starting from the enemy's side of the map.
            let enemy_start = self.world.mirrored(self.home);
            let mut spots = self.world.hello.metal_spots.clone();
            spots.sort_by(|a, b| a.dist2d(enemy_start).total_cmp(&b.dist2d(enemy_start)));
            if !spots.is_empty() {
                self.attack_target = spots[self.sweep_index % spots.len()];
                self.sweep_index += 1;
            }
        }
        if idle_army.len() < WAVE_SIZE {
            return;
        }
        let to = Vec3 { y: 0.0, ..self.attack_target };
        eprintln!("[ai {}] f={} attack: {} units to ({:.0}, {:.0})", self.ai(), tick.frame, idle_army.len(), to.x, to.z);
        commands.extend(idle_army.iter().map(|u| Command::Fight { unit: u.id, to, queue: false }));
    }

    /// Picks the faction from whichever commander we own; does nothing until one exists.
    fn adopt_faction(&mut self, own: &[OwnUnit]) {
        for roster in &ROSTERS {
            let Some(commander) = self.world.def_named(roster.commander) else { continue };
            let Some(unit) = own.iter().find(|u| u.def == commander) else { continue };
            let resolve = |name: &str| self.world.def_named(name);
            let (Some(extractor), Some(energy), Some(lab), Some(raider), Some(constructor)) = (
                resolve(roster.extractor), resolve(roster.energy), resolve(roster.lab),
                resolve(roster.raider), resolve(roster.constructor),
            ) else {
                eprintln!("[ai {}] roster for {} has names this game lacks", self.ai(), roster.commander);
                return;
            };
            self.kit = Some(Kit { extractor, energy, lab, raider, constructor });
            self.home = unit.pos;
            self.attack_target = self.world.mirrored(unit.pos);
            eprintln!("[ai {}] playing {} from ({:.0}, {:.0})", self.ai(), roster.commander, unit.pos.x, unit.pos.z);
            return;
        }
    }

    /// Reserves the nearest metal spot that has no extractor of ours and no recent claim.
    fn claim_spot(&mut self, from: Vec3, own: &[OwnUnit], extractor: UnitDefId, frame: i32) -> Option<Vec3> {
        self.spot_claims.retain(|_, claimed| frame - *claimed < SPOT_CLAIM_FRAMES);
        let (index, spot) = self
            .world
            .hello
            .metal_spots
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.spot_claims.contains_key(i))
            .filter(|(_, s)| !own.iter().any(|u| u.def == extractor && u.pos.dist2d(**s) < SPOT_OCCUPIED_RADIUS))
            .min_by(|(_, a), (_, b)| a.dist2d(from).total_cmp(&b.dist2d(from)))?;
        self.spot_claims.insert(index, frame);
        // The engine stores the spot's metal value in `y`.
        Some(Vec3 { y: 0.0, ..*spot })
    }

    fn report(&self, tick: &Tick) {
        for event in &tick.events {
            match *event {
                Event::UnitFinished { unit } => {
                    if let Some(u) = tick.snapshot.own_units.iter().find(|u| u.id == unit) {
                        eprintln!("[ai {}] f={} finished {}", self.ai(), tick.frame, self.name(u.def));
                    }
                }
                Event::BuildSiteNotFound { unit, def } => {
                    eprintln!("[ai {}] f={} no site for {} (builder {})", self.ai(), tick.frame, self.name(def), unit.0);
                }
                Event::CommandRejected { unit, code } => {
                    eprintln!("[ai {}] f={} engine rejected command for unit {} (code {code})", self.ai(), tick.frame, unit.0);
                }
                _ => {}
            }
        }
        if tick.frame % (30 * FRAMES_PER_SECOND) == 0 {
            let s = &tick.snapshot;
            eprintln!(
                "[ai {}] f={} metal {:.0} (+{:.1}/-{:.1}) energy {:.0} (+{:.0}/-{:.0}) units {} enemies visible {}",
                self.ai(), tick.frame, s.metal.current, s.metal.income, s.metal.usage,
                s.energy.current, s.energy.income, s.energy.usage, s.own_units.len(), s.enemies.len()
            );
        }
    }

    fn ai(&self) -> i32 {
        self.world.hello.ai_id
    }

    fn name(&self, def: UnitDefId) -> &str {
        self.world.def(def).map_or("?", |d| d.name.as_str())
    }
}
