//! The menus: for every actor that is free or due, one Choice of next actions (always with a no-change option) and,
//! speculatively, the place and the party an action may need; and the global questions. Jev cannot choose what is
//! not offered, so the options are the whole vocabulary of the hands.

use std::collections::BTreeMap;

use bot_protocol::{OwnUnit, Tick, UnitDefId, UnitId, Vec3};
use jev::Question;
use serde_json::{Value, json};

use super::super::roster::Kit;
use super::super::{Brain, FRAMES_PER_SECOND};
use super::picture::{Picture, distance_words};
use super::{REVIEW_FRAMES, Task};

/// Free spots on a builder's menu, nearest by its own walking first.
const NEAR_SPOTS: usize = 6;
/// A holding group is asked this often; a busy one every `REVIEW_FRAMES`.
const HOLD_REVIEW_FRAMES: i32 = 5 * FRAMES_PER_SECOND;
/// A lab that answered "nothing" is not asked again for this long.
const LAB_REVIEW_FRAMES: i32 = 3 * FRAMES_PER_SECOND;
/// Wrecks and things to repair within this of a builder are offered.
const RECLAIM_WITHIN: f32 = 1800.0;
const REPAIR_WITHIN: f32 = 1200.0;
/// A builder farther than this from home is offered the way home.
const AWAY: f32 = 400.0;
/// An enemy party this close to a group is news that gets it asked at once.
const ALARM: f32 = 600.0;

#[derive(Clone, Debug)]
pub(crate) enum Pick {
    Continue,
    Wait,
    /// An extractor at the free spot answered in `where`, else the nearest of the menu's `spots`.
    Extractor,
    /// A building placed as the base layout has it (`place_planned`).
    Building(UnitDefId),
    /// A building at the place answered in `where`.
    BuildingAt(UnitDefId),
    AssistLab(UnitId),
    Reclaim(Vec3),
    Repair(UnitId),
    WalkTo,
    RetreatHome,
    /// Labs.
    Unit(UnitDefId),
    Nothing,
    /// Groups.
    Hold,
    MoveTo { fight: bool },
    Engage,
    Split,
    /// One soldier, a raider if there is one, walks to `where` and stands there.
    Scout,
    Join(String),
    Retreat,
}

#[derive(Clone, Debug)]
pub(crate) enum Actor {
    Builder(UnitId),
    Lab(UnitId),
    Group(String),
    Global,
}

pub(crate) struct Menu {
    pub actor: Actor,
    /// The actor's name in the picture; question ids are `<name>.<what>`.
    pub name: String,
    pub busy: bool,
    pub questions: Vec<(String, Question)>,
    pub options: BTreeMap<String, Pick>,
    /// A builder's free spots, nearest by its own walking first.
    pub spots: Vec<usize>,
}

impl Brain {
    /// The menus for this second. Updates when each actor was last asked.
    pub(super) fn menus(&mut self, tick: &Tick, kit: &Kit, picture: &Picture) -> Vec<Menu> {
        let own = &tick.snapshot.own_units;
        let frame = tick.frame;
        let Some(mut pianist) = self.pianist.take() else { return Vec::new() };
        let mut menus: Vec<Menu> = Vec::new();
        let place_names: Vec<(String, String)> = picture
            .places
            .iter()
            .map(|p| (p.name.clone(), format!("{} ({})", picture.state["places"][&p.name]["what"].as_str().unwrap_or_default(), self.world.grid(p.at))))
            .collect();
        // One place question per kind of action, each with its premise stated: a single "where, if the action needs a
        // place, else home" was answered "home" nearly every time, since it cannot see which action was chosen
        // (pianist-smoke-5: scouts sent home, advances to home, forty one-unit groups).
        let where_question = |premise: &str, spots: &[(usize, f32)], only_spots: bool| {
            let criteria: BTreeMap<String, Value> = place_names
                .iter()
                .filter(|(n, _)| !only_spots || spots.iter().any(|(i, _)| format!("spot_{i}") == *n))
                .map(|(n, d)| {
                    let walk = spots.iter().find(|(i, _)| format!("spot_{i}") == *n).map_or(String::new(), |(_, s)| format!("; {s:.0} s of walking for this builder"));
                    (n.clone(), json!(format!("{d}{walk}")))
                })
                .collect();
            Question::Choice { instructions: json!(premise.to_string()), criteria }
        };
        let e = &tick.snapshot.energy;
        let m = &tick.snapshot.metal;
        let energy_words = picture.state["economy"]["energy"].as_str().unwrap_or_default().to_string();
        let metal_words = picture.state["economy"]["metal"].as_str().unwrap_or_default().to_string();
        let _ = (e, m);

        // Builders.
        for unit in own.iter().filter(|u| !u.being_built && (u.def == kit.commander || u.def == kit.constructor)) {
            let name = self.actor_name(unit.id, kit);
            let task = pianist.tasks.get(&unit.id).cloned();
            let last = pianist.last_asked.get(&name).copied().unwrap_or(i32::MIN / 2);
            let (free, due) = match &task {
                None => (unit.idle || frame - last >= LAB_REVIEW_FRAMES, true),
                Some(task) => (false, frame - last >= REVIEW_FRAMES && frame - task.since() >= LAB_REVIEW_FRAMES),
            };
            if !(free || due) {
                continue;
            }
            let busy = task.is_some();
            let mut options: BTreeMap<String, Pick> = BTreeMap::new();
            let mut criteria: BTreeMap<String, Value> = BTreeMap::new();
            let mut offer = |key: &str, pick: Pick, words: String| {
                options.insert(key.to_string(), pick);
                criteria.insert(key.to_string(), json!(words));
            };
            if busy {
                offer("continue", Pick::Continue, "Carry on with what it is doing now.".into());
            } else {
                offer("wait", Pick::Wait, "Do nothing for now (only when nothing on this list is worth doing).".into());
            }
            let can = |def: UnitDefId| self.world.def(unit.def).is_some_and(|d| d.build_options.contains(&def));
            let taken: Vec<usize> = pianist.tasks.iter().filter_map(|(id, t)| if let (true, Task::Build { spot: Some(i), .. }) = (*id != unit.id, t) { Some(*i) } else { None }).collect();
            let mut spots: Vec<(usize, f32)> = Vec::new();
            if can(kit.extractor) {
                spots = picture
                    .places
                    .iter()
                    .filter_map(|p| p.spot.map(|i| (i, p.at)))
                    .filter(|(i, _)| !taken.contains(i) && !pianist.refused_spots.get(i).is_some_and(|until| *until > frame))
                    .filter(|(_, at)| !own.iter().any(|u| kit.is_extractor(u.def) && u.pos.dist2d(*at) < 100.0))
                    .filter(|(i, _)| picture.state["places"][format!("spot_{i}")]["what"].as_str().is_some_and(|w| w.starts_with("free")))
                    .map(|(i, _)| (i, self.seconds_to_spot(unit.def, i, unit.pos)))
                    .collect();
                spots.sort_by(|a, b| a.1.total_cmp(&b.1));
                spots.truncate(NEAR_SPOTS);
                if !spots.is_empty() {
                    // One option, the spot in `where`: six spot options split the vote and lost to any single
                    // alternative (pianist-smoke-2: constructors helped the lab on four extractors).
                    let list: Vec<String> = spots
                        .iter()
                        .map(|(i, s)| {
                            let place = &picture.state["places"][format!("spot_{i}")];
                            format!(
                                "spot_{i} ({:.0} s of walking, ground {}{})",
                                s, place["ground"].as_str().unwrap_or_default(),
                                place["enemies_near"].as_str().map_or(String::new(), |e| format!(", enemies near: {e}"))
                            )
                        })
                        .collect();
                    offer("extractor", Pick::Extractor, format!("Build a metal extractor (income) at the free spot answered in `where`; the nearest free spots for it: {}.", list.join(", ")));
                }
            }
            let generator = if (self.world.hello.map.wind_min + self.world.hello.map.wind_max) / 2.0 >= 8.0 { kit.wind } else { kit.solar };
            let labs = own.iter().filter(|u| u.def == kit.lab).count();
            let lab_words = match labs {
                0 => "we have no lab yet: nothing makes soldiers or constructors without one".to_string(),
                1 => "we have one lab already; a second doubles production when metal is banking up".to_string(),
                n => format!("we have {n} labs already"),
            };
            for (key, def, words) in [
                ("generator", generator, format!("Build an energy generator beside itself. Our energy now: {energy_words}.")),
                ("lab", kit.lab, format!("Build a bot lab (the factory) in the base yard; 650 metal. {lab_words}. Our metal now: {metal_words}.")),
                ("converter", kit.converter, "Build an energy-to-metal converter beside itself (1150 metal; only with a large energy surplus and no free spots).".to_string()),
                ("advanced_lab", kit.advanced_lab, "Build the advanced (tier 2) bot lab in the base yard: 2600 metal, for a strong economy only.".to_string()),
            ] {
                if can(def) {
                    offer(key, Pick::Building(def), words);
                }
            }
            if can(kit.nano) && own.iter().any(|u| u.def == kit.lab && !u.being_built) {
                offer("construction_turret", Pick::Building(kit.nano), "Build a construction turret beside the nearest lab: adds build power to it (metal must be flowing in faster than the lab spends it).".into());
            }
            if can(kit.turret) {
                offer("turret_at", Pick::BuildingAt(kit.turret), "Build a light laser turret (85 metal) at the place answered in `where`: repels lone raiders at an extractor.".into());
            }
            if can(kit.radar) {
                offer("radar_at", Pick::BuildingAt(kit.radar), "Build a radar tower (60 metal, sees 2000) at the place answered in `where`.".into());
            }
            if let Some(lab) = own.iter().filter(|u| u.def == kit.lab && !u.being_built).min_by(|a, b| a.pos.dist2d(unit.pos).total_cmp(&b.pos.dist2d(unit.pos))) {
                offer("assist_lab", Pick::AssistLab(lab.id), "Help the lab build: adds this builder's build power to whatever it makes.".into());
            }
            if let Some(field) = self.reclaim.fields.iter().filter(|f| f.metal >= 100.0 && f.at.dist2d(unit.pos) < RECLAIM_WITHIN).max_by(|a, b| a.metal.total_cmp(&b.metal)) {
                offer("reclaim", Pick::Reclaim(field.at), format!("Take apart the wrecks at {} ({:.0} metal lying there{}).", self.place_words(&picture.places, field.at), field.metal, if field.safe { "" } else { "; not safe ground" }));
            }
            let hurt = own
                .iter()
                .filter(|u| u.id != unit.id && !u.being_built && u.health < u.max_health * 0.7 && u.pos.dist2d(unit.pos) < REPAIR_WITHIN)
                .filter(|u| u.def == kit.commander || self.world.def(u.def).is_some_and(|d| d.speed == 0.0))
                .min_by(|a, b| a.pos.dist2d(unit.pos).total_cmp(&b.pos.dist2d(unit.pos)));
            if let Some(hurt) = hurt {
                offer("repair", Pick::Repair(hurt.id), format!("Repair our {} at {} ({:.0}% health).", self.name(hurt.def), self.place_words(&picture.places, hurt.pos), hurt.health / hurt.max_health * 100.0));
            }
            offer("walk_to", Pick::WalkTo, "Walk to the place answered in `where` and wait there.".into());
            if unit.pos.dist2d(self.home) > AWAY {
                offer("retreat_home", Pick::RetreatHome, format!("Go home now ({} away, {:.0}); the way home avoids known threats.", distance_words(unit.pos.dist2d(self.home)), unit.pos.dist2d(self.home)));
            }
            let instructions = json!(format!(
                "Given `actors.{name}` and the player's `instructions`, what should {name} do next? Prefer what the instructions say; keep to the plan unless the situation has changed. Energy now: {energy_words}. Metal now: {metal_words}."
            ));
            let mut questions = vec![
                (format!("{name}.do"), Question::Choice { instructions, criteria }),
                (format!("{name}.where"), where_question(&format!("Suppose {name} builds a turret or a radar, or walks somewhere: at which place? Choose the place the instructions and the situation call for."), &spots, false)),
            ];
            if !spots.is_empty() {
                questions.push((format!("{name}.where_extractor"), where_question(&format!("Suppose {name} builds a metal extractor next: at which of these free spots? Nearer is sooner; ground held by us is safer; enemies near a spot get the builder killed."), &spots, true)));
            }
            pianist.last_asked.insert(name.clone(), frame);
            menus.push(Menu {
                actor: Actor::Builder(unit.id),
                questions,
                name,
                busy,
                options,
                spots: spots.iter().map(|(i, _)| *i).collect(),
            });
        }

        // Labs.
        for unit in own.iter().filter(|u| !u.being_built && (u.def == kit.lab || u.def == kit.advanced_lab)) {
            let name = self.actor_name(unit.id, kit);
            let queued = pianist.lab_queue.get(&unit.id).map_or(0, Vec::len);
            let last = pianist.last_asked.get(&name).copied().unwrap_or(i32::MIN / 2);
            if queued >= 2 || frame - last < LAB_REVIEW_FRAMES {
                continue;
            }
            let Some(def) = self.world.def(unit.def) else { continue };
            let extractors = own.iter().filter(|u| !u.being_built && kit.is_extractor(u.def)).count();
            let constructors = own.iter().filter(|u| !u.being_built && (u.def == kit.constructor || u.def == kit.advanced_constructor)).count();
            let soldiers: Vec<&OwnUnit> = own.iter().filter(|u| !u.being_built && self.is_army(u, kit)).collect();
            let army_metal: f32 = soldiers.iter().filter_map(|u| self.world.def(u.def)).map(|d| d.metal_cost).sum();
            let mut options: BTreeMap<String, Pick> = BTreeMap::new();
            let mut criteria: BTreeMap<String, Value> = BTreeMap::new();
            options.insert("nothing".into(), Pick::Nothing);
            criteria.insert("nothing".into(), json!("Build nothing now and save the metal."));
            for buildable in &def.build_options {
                let key = self.name(*buildable).to_string();
                options.insert(key.clone(), Pick::Unit(*buildable));
                // The count in words beside the option: Jev does not count what it has against a plan
                // (pianist-smoke-1: thirty constructors and no soldier by minute nine).
                let have = if *buildable == kit.constructor || *buildable == kit.advanced_constructor {
                    format!(" We have {constructors} constructors already: {}.", super::picture::constructor_words(constructors, extractors))
                } else if self.world.def(*buildable).is_some_and(|d| d.weapon_count > 0 && d.speed > 0.0) {
                    format!(" Our soldiers: {}.", super::picture::soldier_words(soldiers.len(), army_metal))
                } else {
                    String::new()
                };
                criteria.insert(key, json!(format!("Build a {}.{have}", self.unit_words(*buildable, kit))));
            }
            let instructions = json!(format!(
                "Given `actors.{name}`, `ours`, `economy` and the player's `instructions`, which unit should {name} build next? We have {constructors} constructors ({}) and {} soldiers ({}).",
                super::picture::constructor_words(constructors, extractors), soldiers.len(), super::picture::soldier_words(soldiers.len(), army_metal)
            ));
            pianist.last_asked.insert(name.clone(), frame);
            menus.push(Menu { actor: Actor::Lab(unit.id), questions: vec![(format!("{name}.next"), Question::Choice { instructions, criteria })], name, busy: queued > 0, options, spots: Vec::new() });
        }

        // Groups.
        let group_names: Vec<(String, Option<Vec3>)> = pianist.groups.iter().map(|g| (g.name.clone(), super::groups::centre_of(&g.units(own)))).collect();
        let scout_out = pianist.groups.iter().any(|g| g.members.len() == 1 && matches!(g.task, super::GroupTask::Move { fight: false, .. }));
        for group in &mut pianist.groups {
            let name = format!("group_{}", group.name);
            let units = group.units(own);
            let Some(centre) = super::groups::centre_of(&units) else { continue };
            let last = pianist.last_asked.get(&name).copied().unwrap_or(i32::MIN / 2);
            let enemies_near = picture.parties.iter().any(|p| p.at.dist2d(centre) < ALARM);
            let alarm = enemies_near && !group.enemies_near;
            group.enemies_near = enemies_near;
            let review = if group.task.busy() { REVIEW_FRAMES } else { HOLD_REVIEW_FRAMES };
            if !(alarm || frame - last >= review) {
                continue;
            }
            let busy = group.task.busy();
            let mut options: BTreeMap<String, Pick> = BTreeMap::new();
            let mut criteria: BTreeMap<String, Value> = BTreeMap::new();
            let mut offer = |key: &str, pick: Pick, words: String| {
                options.insert(key.to_string(), pick);
                criteria.insert(key.to_string(), json!(words));
            };
            if busy {
                offer("continue", Pick::Continue, "Carry on with what it is doing.".into());
            }
            let enemies = tick.snapshot.enemies.as_slice();
            let parties_words: Vec<String> = picture.parties.iter().map(|p| format!("{} ({}, at {}, {} from this group): {}", p.name, p.composition, self.place_words(&picture.places, p.at), distance_words(p.at.dist2d(centre)), self.odds_words(&units, p, enemies))).collect();
            let base_words = {
                let base = self.enemy_base(centre);
                let force = self.known_enemy_force(base, 1500.0, enemies);
                let ratio = self.odds(&Brain::force_of(&units), &force);
                let turrets = self.enemy_buildings.values().filter(|(def, pos, _)| pos.dist2d(base) < 1500.0 && self.world.def(*def).is_some_and(|d| d.weapon_count > 0)).count();
                format!(
                    "the enemy base as we know it ({turrets} turrets known, its soldiers seen there lately): {}",
                    if self.found_enemy_base().is_none() { "not found yet, what stands there is unknown: scout it first (`scout`)" } else if ratio >= 2.5 { "we outweigh it heavily" } else if ratio >= 1.3 { "we outweigh it" } else if ratio >= 0.8 { "an even fight" } else { "it outweighs us" }
                )
            };
            offer("hold", Pick::Hold, "Stand where it is; fight whatever comes within reach. Nothing beyond reach is protected by this.".into());
            offer("move_to", Pick::MoveTo { fight: false }, "Walk to the place in `where` without stopping to fight on the way (it runs from everything).".into());
            offer("fight_to", Pick::MoveTo { fight: true }, format!("Advance to the place in `where`, arriving together and fighting what it meets. Against {base_words}."));
            if !picture.parties.is_empty() {
                offer("engage", Pick::Engage, format!("Attack the enemy party named in `whom` now and follow it. In sight: {}.", parties_words.join("; ")));
            }
            offer("retreat", Pick::Retreat, "Fall back to our base.".into());
            if units.len() >= 2 {
                offer("split", Pick::Split, "Send a detachment, the number in `how_many` of the nearest soldiers, to advance to the place in `where`; the rest carry on as they were.".into());
                // One scout out at a time (smoke-6: a raider every ten seconds to the enemy base, five dead by 5:00).
                if !scout_out {
                    offer("scout", Pick::Scout, "Send one soldier (a raider if the group has one) to look at the place in `where_scout` and stand there watching; the rest carry on. This is how the enemy base and its army get seen.".into());
                }
            }
            if let Some((other, _)) = group_names.iter().filter(|(n, c)| *n != group.name && c.is_some()).min_by(|a, b| a.1.unwrap().dist2d(centre).total_cmp(&b.1.unwrap().dist2d(centre))) {
                offer(&format!("join_group_{other}"), Pick::Join(other.clone()), format!("Merge into group_{other} and take its task."));
            }
            let instructions = json!(format!("Given `actors.{name}`, `enemy` and the player's `instructions`, what should {name} do next?"));
            let mut questions = vec![
                (format!("{name}.do"), Question::Choice { instructions, criteria }),
                (format!("{name}.where"), where_question(&format!("Suppose {name} advances, moves or sends a detachment: to which place? Choose where the instructions and the situation call for it to stand or fight."), &[], false)),
                (format!("{name}.where_scout"), where_question(&format!("Suppose {name} sends one soldier to look at a place: which place needs looking at? The enemy base if it is not found or not seen lately, else the spots we know least about."), &[], false)),
                (format!("{name}.how_many"), Question::choice(format!("If {name} sends a detachment, how many soldiers go?"), [("2", "two"), ("4", "four"), ("8", "eight"), ("half", "half of the group")])),
            ];
            if !picture.parties.is_empty() {
                let criteria: BTreeMap<String, Value> = picture.parties.iter().map(|p| (p.name.clone(), json!(format!("{} at {}: {}", p.composition, self.place_words(&picture.places, p.at), self.odds_words(&units, p, enemies))))).collect();
                questions.push((format!("{name}.whom"), Question::Choice { instructions: json!(format!("If {name} attacks an enemy party, which one?")), criteria }));
            }
            pianist.last_asked.insert(name.clone(), frame);
            menus.push(Menu { actor: Actor::Group(group.name.clone()), name, busy, questions, options, spots: Vec::new() });
        }

        // Global.
        if !menus.is_empty() {
            let mut questions = vec![
                ("global.base_in_danger".to_string(), Question::noul("Given `enemy` and `places`, is our base or our commander in danger right now?")),
                ("global.attack_coming".to_string(), Question::noul("Given `enemy`, is a large enemy attack on us likely within the next minute or two?")),
            ];
            if self.strategist.is_some() {
                questions.push(("global.needs_player".to_string(), Question::noul("Given everything, does the situation need the player's attention now: something the `instructions` do not cover, or a plan that has stopped fitting the game?")));
            }
            menus.push(Menu { actor: Actor::Global, name: "global".into(), busy: false, questions, options: BTreeMap::new(), spots: Vec::new() });
        }
        self.pianist = Some(pianist);
        menus
    }
}

/// The soldiers of a group nearest a point, for a detachment.
pub(crate) fn nearest_of<'a>(units: &[&'a OwnUnit], to: Vec3, n: usize) -> Vec<&'a OwnUnit> {
    let mut sorted: Vec<&OwnUnit> = units.to_vec();
    sorted.sort_by(|a, b| a.pos.dist2d(to).total_cmp(&b.pos.dist2d(to)));
    sorted.into_iter().take(n).collect()
}
