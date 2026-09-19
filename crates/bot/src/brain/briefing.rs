//! The brain's side of the strategist link: directives in, briefing, events and triggers out.

use std::collections::BTreeMap;

use bot_protocol::{Event, OwnUnit, Tick, UnitDefId, Vec3};
use serde_json::json;

use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};
use crate::strategist::shared::{Briefing, Counts, EnemyCluster, Group, Place, RememberedBuilding};

const MAX_RECENT_EVENTS: usize = 25;
/// The same kind of trigger wakes the strategist at most this often.
const TRIGGER_COOLDOWN_FRAMES: i32 = 60 * FRAMES_PER_SECOND;

fn clock(frame: i32) -> String {
    let seconds = frame / FRAMES_PER_SECOND;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

impl Brain {
    pub(super) fn read_directives(&mut self, frame: i32) {
        let Some(shared) = &self.strategist else { return };
        let mut directives = shared.directives.lock().unwrap();
        directives.expire(frame);
        self.directives = directives.clone();
    }

    pub(super) fn place(&self, pos: Vec3) -> Place {
        Place { grid: self.world.grid(pos), x: pos.x as i32, z: pos.z as i32 }
    }

    /// Notes something for the strategist's next look.
    pub(super) fn event(&mut self, frame: i32, text: String) {
        if self.strategist.is_none() {
            return;
        }
        if self.recent_events.len() == MAX_RECENT_EVENTS {
            self.recent_events.pop_front();
        }
        self.recent_events.push_back(format!("{} {text}", clock(frame)));
    }

    /// Wakes the strategist now, unless the same kind of trigger fired within the cooldown.
    pub(super) fn trigger(&mut self, kind: &'static str, frame: i32, text: String) {
        let Some(shared) = self.strategist.clone() else { return };
        let last = self.last_trigger_frame.get(kind).copied().unwrap_or(i32::MIN / 2);
        if frame - last < TRIGGER_COOLDOWN_FRAMES {
            return;
        }
        self.last_trigger_frame.insert(kind, frame);
        self.event(frame, text.clone());
        shared.trigger(text);
    }

    /// Notes our own losses by name, and wakes the strategist when extractors go down in numbers.
    pub(super) fn track_losses(&mut self, tick: &Tick, kit: &Kit) {
        const LOSSES_WORTH_WAKING_FOR: usize = 2;
        for event in &tick.events {
            let Event::UnitDestroyed { unit, .. } = event else { continue };
            let Some((def, pos)) = self.known_units.remove(unit) else { continue };
            if self.world.def(def).is_some_and(|d| d.speed > 0.0 && d.build_speed == 0.0) {
                continue; // soldiers die all the time
            }
            let (name, grid) = (self.name(def).to_string(), self.world.grid(pos));
            self.event(tick.frame, format!("lost {name} at {grid}"));
            if def == kit.extractor {
                self.extractor_losses.push_back(tick.frame);
            }
        }
        while self.extractor_losses.front().is_some_and(|f| tick.frame - f > TRIGGER_COOLDOWN_FRAMES) {
            self.extractor_losses.pop_front();
        }
        if self.extractor_losses.len() >= LOSSES_WORTH_WAKING_FOR {
            let lost = self.extractor_losses.len();
            self.trigger("extractors", tick.frame, format!("We lost {lost} extractors in the last minute."));
        }
        for unit in &tick.snapshot.own_units {
            self.known_units.insert(unit.id, (unit.def, unit.pos));
        }
    }

    pub(super) fn track_enemy_buildings(&mut self, tick: &Tick) {
        for event in &tick.events {
            if let Event::EnemyDestroyed { enemy } = event {
                self.enemy_buildings.remove(enemy);
            }
        }
        for enemy in &tick.snapshot.enemies {
            let Some(def) = enemy.def else { continue };
            if self.world.def(def).is_some_and(|d| d.speed == 0.0) {
                self.enemy_buildings.insert(enemy.id, (def, enemy.pos, tick.frame));
            }
        }
    }

    fn group(&self, units: &[&OwnUnit]) -> Group {
        let mut composition: BTreeMap<&str, usize> = BTreeMap::new();
        for unit in units {
            *composition.entry(self.name(unit.def)).or_default() += 1;
        }
        let n = units.len().max(1) as f32;
        let centre = units.iter().fold(Vec3::default(), |sum, u| Vec3 { x: sum.x + u.pos.x / n, y: 0.0, z: sum.z + u.pos.z / n });
        Group {
            size: units.len(),
            idle: units.iter().filter(|u| u.idle).count(),
            centre: (!units.is_empty()).then(|| self.place(centre)),
            composition: composition.into_iter().map(|(name, count)| (name.to_string(), count)).collect(),
        }
    }

    pub(super) fn publish_briefing(&mut self, tick: &Tick, kit: &Kit) {
        let Some(shared) = self.strategist.clone() else { return };
        let snapshot = &tick.snapshot;
        if tick.frame <= super::TICK_FRAMES_HINT {
            *shared.map.lock().unwrap() = self.map_description();
        }
        let count = |def: UnitDefId| snapshot.own_units.iter().filter(|u| u.def == def).count();
        let soldiers: Vec<&OwnUnit> = snapshot.own_units.iter().filter(|u| !u.being_built && self.is_army(u, kit)).collect();
        let (attackers, home_group): (Vec<&OwnUnit>, Vec<&OwnUnit>) =
            soldiers.iter().partition(|u| self.army.is_attacker(u.id));

        let mut cells: BTreeMap<String, (Vec3, BTreeMap<&str, usize>, usize)> = BTreeMap::new();
        for enemy in &snapshot.enemies {
            let cell = cells.entry(self.world.grid(enemy.pos)).or_insert((enemy.pos, BTreeMap::new(), 0));
            *cell.1.entry(enemy.def.map_or("unidentified", |def| self.name(def))).or_default() += 1;
            cell.2 += 1;
        }
        let enemies_visible = cells
            .into_values()
            .map(|(pos, composition, units)| EnemyCluster {
                at: self.place(pos),
                units,
                composition: composition.into_iter().map(|(name, n)| (name.to_string(), n)).collect(),
                distance_from_home: pos.dist2d(self.home) as i32,
            })
            .collect();
        let mut enemy_buildings_remembered: Vec<RememberedBuilding> = self
            .enemy_buildings
            .values()
            .map(|(def, pos, seen)| RememberedBuilding { name: self.name(*def).to_string(), at: self.place(*pos), last_seen: clock(*seen) })
            .collect();
        enemy_buildings_remembered.sort_by(|a, b| a.at.grid.cmp(&b.at.grid).then(a.name.cmp(&b.name)));

        let briefing = Briefing {
            game_time: clock(tick.frame),
            frame: tick.frame,
            metal: snapshot.metal,
            energy: snapshot.energy,
            counts: Counts {
                extractors: count(kit.extractor),
                generators: count(kit.solar) + count(kit.advanced_solar),
                converters: count(kit.converter),
                labs: count(kit.lab),
                turrets: count(kit.turret),
                constructors: count(kit.constructor),
                army: soldiers.len(),
            },
            home: self.place(self.home),
            presumed_enemy_start: self.place(self.enemy_start),
            home_group: self.group(&home_group),
            attackers: self.group(&attackers),
            waves_sent: self.army.waves_sent(),
            army_station: self.place(self.last_station),
            enemies_visible,
            enemy_buildings_remembered,
            recent_events: self.recent_events.iter().cloned().collect(),
            directives_in_force: self.directives.describe(tick.frame),
        };
        *shared.briefing.lock().unwrap() = briefing;
    }

    fn map_description(&self) -> serde_json::Value {
        let map = &self.world.hello.map;
        let spots: Vec<_> = self
            .world
            .hello
            .metal_spots
            .iter()
            .map(|s| json!({ "grid": self.world.grid(*s), "x": s.x as i32, "z": s.z as i32 }))
            .collect();
        json!({
            "name": map.name, "width": map.width, "height": map.height,
            "grid": "8x8 cells; columns A-H run west to east (x), rows 1-8 run north to south (z)",
            "our_start": self.place(self.home), "presumed_enemy_start": self.place(self.enemy_start),
            "metal_spots": spots,
        })
    }
}
