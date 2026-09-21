//! The hands: an answer becomes orders through the existing actuators (build sites, walking, the march, fight
//! orders), a task is remembered, and the decision goes into the record.

use std::collections::{BTreeMap, HashSet};

use bot_protocol::{BuildSite, Command, OwnUnit, Tick, UnitId, Vec3};
use jev::Answer;
use serde_json::json;

use super::super::economy::Plan;
use super::super::roster::Kit;
use super::super::{Brain, FRAMES_PER_SECOND};
use super::menu::{Actor, Menu, Pick, nearest_of};
use super::picture::Picture;
use super::{Group, GroupTask, SWITCH_MARGIN, Task};

impl Brain {
    /// Plays every menu's answer.
    pub(super) fn play(&mut self, tick: &Tick, kit: &Kit, picture: &Picture, menus: Vec<Menu>, answers: &BTreeMap<String, Answer>, commands: &mut Vec<Command>) {
        let frame = tick.frame;
        for menu in menus {
            let Actor::Global = menu.actor else {
                self.play_one(tick, kit, picture, menu, answers, commands);
                continue;
            };
            let outputs: BTreeMap<&str, f64> = menu.questions.iter().filter_map(|(id, _)| answers.get(id).map(|a| (id.as_str(), a.probability_of("yes")))).collect();
            self.journal.note_from("jev", frame, "global", serde_json::Value::Null, json!(outputs));
        }
    }

    fn play_one(&mut self, tick: &Tick, kit: &Kit, picture: &Picture, menu: Menu, answers: &BTreeMap<String, Answer>, commands: &mut Vec<Command>) {
        let frame = tick.frame;
        let own = &tick.snapshot.own_units;
        let name = menu.name.clone();
        let question = if matches!(menu.actor, Actor::Lab(_)) { format!("{name}.next") } else { format!("{name}.do") };
        let Some(Answer::Choice { choice, probabilities, confidence }) = answers.get(&question) else { return };
        let mut chosen = choice.clone();
        let p = |option: &str| probabilities.get(option).copied().unwrap_or(0.0);
        // H-HANDS-SWITCH: a busy actor changes course only for a clear winner.
        let mut kept = false;
        if menu.busy && chosen != "continue" && p(&chosen) - p("continue") < SWITCH_MARGIN {
            chosen = "continue".into();
            kept = true;
        }
        let answered = |q: &str| answers.get(&format!("{name}.{q}")).and_then(|a| if let Answer::Choice { choice, .. } = a { Some(choice.clone()) } else { None });
        let where_ = answered("where");
        let where_extractor = answered("where_extractor");
        let where_scout = answered("where_scout");
        let whom = answers.get(&format!("{name}.whom")).and_then(|a| if let Answer::Choice { choice, .. } = a { Some(choice.clone()) } else { None });
        let how_many = answers.get(&format!("{name}.how_many")).and_then(|a| if let Answer::Choice { choice, .. } = a { Some(choice.clone()) } else { None });
        let place = |named: &Option<String>| named.as_ref().and_then(|n| picture.places.iter().find(|p| p.name == *n)).cloned();
        let Some(pick) = menu.options.get(&chosen).cloned() else { return };
        let mut did: Option<String> = None;
        match menu.actor {
            Actor::Builder(id) => {
                let Some(unit) = own.iter().find(|u| u.id == id) else { return };
                let mut task: Option<Task> = None;
                let mut build = |plan: Plan, spot: Option<usize>| -> Option<String> {
                    let (def, site) = self.build_site_for(&plan, unit, kit)?;
                    commands.push(Command::Build { unit: id, def, site: Some(site), queue: false });
                    task = Some(Task::Build { def, near: site.near, spot, ordered: frame, started: false });
                    Some(format!("build a {} at {}", self.name(def), self.place_words(&picture.places, site.near)))
                };
                match pick {
                    Pick::Continue | Pick::Wait => {}
                    Pick::Extractor => {
                        // The spot answered in `where` when it is one of the free ones offered, else the nearest.
                        let asked = place(&where_extractor).and_then(|p| p.spot).filter(|i| menu.spots.contains(i));
                        if let Some(i) = asked.or_else(|| menu.spots.first().copied()) {
                            did = build(Plan::Extractor(self.world.hello.metal_spots[i]), Some(i));
                        }
                    }
                    Pick::Building(def) => {
                        let plan = self.place_planned(def, unit, own, kit);
                        did = build(plan, None);
                    }
                    Pick::BuildingAt(def) => {
                        let at = place(&where_).map_or(unit.pos, |p| p.at);
                        did = build(Plan::Near(def, self.snap_to_reachable(at)), None);
                    }
                    Pick::AssistLab(lab) => {
                        commands.push(Command::Guard { unit: id, target: lab });
                        task = Some(Task::Assist { lab, since: frame });
                        did = Some("help the lab".into());
                    }
                    Pick::Reclaim(at) => {
                        let wrecks = self.wrecks_to_take(at, unit);
                        if !wrecks.is_empty() {
                            commands.extend(wrecks.into_iter().enumerate().map(|(n, feature)| Command::ReclaimFeature { unit: id, feature, queue: n > 0 }));
                            task = Some(Task::Reclaim { at, since: frame });
                            did = Some(format!("reclaim wrecks at {}", self.place_words(&picture.places, at)));
                        }
                    }
                    Pick::Repair(target) => {
                        commands.push(Command::Repair { unit: id, target, queue: false });
                        task = Some(Task::Repair { target, since: frame });
                        did = Some("repair".into());
                    }
                    Pick::WalkTo => {
                        if let Some(p) = place(&where_) {
                            let to = self.snap_to_reachable(p.at);
                            commands.push(Command::Move { unit: id, to, queue: false });
                            task = Some(Task::Walk { to, place: p.name.clone(), since: frame });
                            did = Some(format!("walk to {}", p.name));
                        }
                    }
                    Pick::RetreatHome => {
                        match (unit.def == kit.commander).then(|| self.commander_waypoint_home(unit.pos)).flatten() {
                            Some(waypoint) => {
                                commands.push(Command::Move { unit: id, to: waypoint, queue: false });
                                commands.push(Command::Move { unit: id, to: self.home, queue: true });
                            }
                            None => commands.push(Command::Move { unit: id, to: self.home, queue: false }),
                        }
                        task = Some(Task::Walk { to: self.home, place: "home".into(), since: frame });
                        did = Some("go home".into());
                    }
                    _ => {}
                }
                if let Some(task) = task {
                    self.pianist.as_mut().expect("pianist mode").tasks.insert(id, task);
                } else if matches!(pick, Pick::Wait) {
                    self.pianist.as_mut().expect("pianist mode").tasks.remove(&id);
                }
            }
            Actor::Lab(id) => {
                if let Pick::Unit(def) = pick {
                    commands.push(Command::Build { unit: id, def, site: None, queue: false });
                    self.pianist.as_mut().expect("pianist mode").lab_queue.entry(id).or_default().push((def, frame));
                    did = Some(format!("build a {}", self.name(def)));
                }
            }
            Actor::Group(ref group_name) => {
                did = self.play_group(tick, kit, picture, group_name, pick, &where_, &where_scout, &whom, &how_many, commands);
            }
            Actor::Global => {}
        }
        let pianist = self.pianist.as_mut().expect("pianist mode");
        if kept {
            pianist.stats.kept += 1;
        } else if menu.busy && chosen != "continue" {
            pianist.stats.switches += 1;
        }
        if let Some(did) = &did {
            pianist.done.push(format!("{} {name}: {did}", super::picture::clock(frame)));
        }
        let kind: &'static str = match menu.actor {
            Actor::Builder(_) => "builder",
            Actor::Lab(_) => "lab",
            Actor::Group(_) => "group",
            Actor::Global => "global",
        };
        let inputs = json!({ "actor": name, "options": menu.options.keys().collect::<Vec<_>>(), "busy": menu.busy });
        let outputs = json!({ "choice": choice, "played": chosen, "probability": p(choice), "confidence": confidence, "where": where_, "where_extractor": where_extractor, "where_scout": where_scout, "whom": whom, "how_many": how_many, "did": did });
        self.journal.note_from("jev", frame, kind, inputs, outputs);
    }

    #[allow(clippy::too_many_arguments)]
    fn play_group(&mut self, tick: &Tick, kit: &Kit, picture: &Picture, group_name: &str, pick: Pick, where_: &Option<String>, where_scout: &Option<String>, whom: &Option<String>, how_many: &Option<String>, commands: &mut Vec<Command>) -> Option<String> {
        let frame = tick.frame;
        let own = &tick.snapshot.own_units;
        let home = self.home;
        let place = |named: &Option<String>| named.as_ref().and_then(|n| picture.places.iter().find(|p| p.name == *n)).cloned();
        let Some(mut pianist) = self.pianist.take() else { return None };
        let Some(index) = pianist.groups.iter().position(|g| g.name == group_name) else {
            self.pianist = Some(pianist);
            return None;
        };
        let units: Vec<&OwnUnit> = pianist.groups[index].units(own);
        let ids: Vec<UnitId> = units.iter().map(|u| u.id).collect();
        let centre = super::groups::centre_of(&units);
        let mut did: Option<String> = None;
        match pick {
            Pick::Continue => {}
            Pick::Hold => {
                let group = &mut pianist.groups[index];
                if group.task.busy() {
                    commands.extend(ids.iter().map(|id| Command::Stop { unit: *id }));
                }
                group.task = GroupTask::Hold { since: frame };
                group.held.clear();
                did = Some("hold".into());
            }
            Pick::MoveTo { fight } => {
                if let Some(p) = place(where_) {
                    let to = self.snap_to_reachable(p.at);
                    let group = &mut pianist.groups[index];
                    commands.extend(ids.iter().map(|id| if fight { Command::Fight { unit: *id, to, queue: false } } else { Command::Move { unit: *id, to, queue: false } }));
                    group.task = GroupTask::Move { to, place: p.name.clone(), fight, since: frame };
                    group.held.clear();
                    group.last_order = frame;
                    did = Some(format!("{} to {}", if fight { "advance" } else { "walk" }, p.name));
                }
            }
            Pick::Engage => {
                if let Some(party) = whom.as_ref().and_then(|n| picture.parties.iter().find(|p| p.name == *n)) {
                    let group = &mut pianist.groups[index];
                    commands.extend(ids.iter().map(|id| Command::Fight { unit: *id, to: party.at, queue: false }));
                    group.task = GroupTask::Engage { party: party.ids.clone(), at: party.at, since: frame, last_seen: frame };
                    group.held.clear();
                    group.last_order = frame;
                    did = Some(format!("attack {} ({})", party.name, party.composition));
                }
            }
            Pick::Retreat => {
                let group = &mut pianist.groups[index];
                commands.extend(ids.iter().map(|id| Command::Move { unit: *id, to: home, queue: false }));
                group.task = GroupTask::Move { to: home, place: "home".into(), fight: false, since: frame };
                group.held.clear();
                group.last_order = frame;
                did = Some("fall back home".into());
            }
            Pick::Split => {
                if let Some(p) = place(where_) {
                    let n = match how_many.as_deref() {
                        Some("2") => 2,
                        Some("4") => 4,
                        Some("8") => 8,
                        _ => units.len() / 2,
                    }
                    .clamp(1, units.len().saturating_sub(1).max(1));
                    let to = self.snap_to_reachable(p.at);
                    let detached: Vec<UnitId> = nearest_of(&units, to, n).iter().map(|u| u.id).collect();
                    pianist.groups[index].members.retain(|id| !detached.contains(id));
                    commands.extend(detached.iter().map(|id| Command::Fight { unit: *id, to, queue: false }));
                    let name = pianist.new_group_name();
                    did = Some(format!("send {} soldiers as group_{name} to {}", detached.len(), p.name));
                    pianist.groups.push(Group { name, members: detached, task: GroupTask::Move { to, place: p.name.clone(), fight: true, since: frame }, held: HashSet::new(), last_order: frame, enemies_near: false });
                }
            }
            Pick::Scout => {
                // A scout to where the group stands looks at nothing.
                if let Some(p) = place(where_scout).filter(|p| centre.is_none_or(|c| c.dist2d(p.at) > 600.0)) {
                    let to = self.snap_to_reachable(p.at);
                    let scout = units.iter().copied().filter(|u| u.def == kit.raider).min_by(|a, b| a.pos.dist2d(to).total_cmp(&b.pos.dist2d(to))).or_else(|| nearest_of(&units, to, 1).first().copied());
                    if let Some(scout) = scout {
                        pianist.groups[index].members.retain(|id| *id != scout.id);
                        commands.push(Command::Move { unit: scout.id, to, queue: false });
                        let name = pianist.new_group_name();
                        did = Some(format!("send a {} as group_{name} to look at {}", self.name(scout.def), p.name));
                        pianist.groups.push(Group { name, members: vec![scout.id], task: GroupTask::Move { to, place: p.name.clone(), fight: false, since: frame }, held: HashSet::new(), last_order: frame, enemies_near: false });
                    }
                }
            }
            Pick::Join(other) => {
                if let Some(target) = pianist.groups.iter().position(|g| g.name == other) {
                    let members = std::mem::take(&mut pianist.groups[index].members);
                    let to = super::groups::centre_of(&pianist.groups[target].units(own)).or(centre).unwrap_or(home);
                    commands.extend(members.iter().map(|id| Command::Move { unit: *id, to, queue: false }));
                    pianist.groups[target].members.extend(members);
                    pianist.groups.remove(index);
                    did = Some(format!("join group_{other}"));
                }
            }
            _ => {}
        }
        let _ = FRAMES_PER_SECOND;
        self.pianist = Some(pianist);
        did
    }
}

/// A site exactly at a point, for what the shim places without searching.
#[allow(dead_code)]
pub(crate) fn exact(at: Vec3) -> BuildSite {
    BuildSite { near: at, search_radius: 0.0, min_dist: 0 }
}
