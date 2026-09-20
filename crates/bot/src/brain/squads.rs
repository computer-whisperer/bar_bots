//! Squads: soldiers the field commander has claimed (`DESIGN.md`, "Field commander"). The commander asks by name
//! and unit type; membership, posts and orders are carried out here. Everything unclaimed stays with the heuristics.

use std::collections::{BTreeMap, HashMap};

use bot_protocol::{Command, OwnUnit, Tick, UnitId, Vec3};

use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};
use crate::strategist::shared::{ExtractorStatus, Field, OrderKind, Post, Score, SquadStatus};

/// A squad's standing orders are re-issued no more often than this.
const REORDER_FRAMES: i32 = 2 * FRAMES_PER_SECOND;
/// Members of a posted squad this far from the post (as a share of its radius) with nothing to fight walk back.
const POST_SLACK: f32 = 0.4;

#[derive(Default)]
pub struct Squads {
    members: BTreeMap<String, Vec<UnitId>>,
    posts: HashMap<String, Post>,
    last_order_frame: HashMap<String, i32>,
    engaged: HashMap<String, bool>,
    /// What the commander should know about its last post or order for a squad (moved to walkable ground, or refused).
    remarks: HashMap<String, String>,
}

impl Squads {
    pub fn contains(&self, unit: UnitId) -> bool {
        self.members.values().any(|m| m.contains(&unit))
    }
}

impl Brain {
    /// `soldiers` is every finished soldier, squad members included.
    pub(super) fn run_squads(&mut self, tick: &Tick, kit: &Kit, soldiers: &[&OwnUnit], commands: &mut Vec<Command>) {
        let Some(shared) = self.strategist.clone() else { return };
        for members in self.squads.members.values_mut() {
            members.retain(|id| soldiers.iter().any(|u| u.id == *id));
        }

        let mut one_off: Vec<(String, OrderKind, Vec3)> = Vec::new();
        {
            let mut orders = shared.field_orders.lock().unwrap();
            self.turret_requests.append(&mut orders.turret_requests);
            self.production_weights = orders.production.clone();
            let names: Vec<String> = orders.squads.keys().cloned().collect();
            for name in names {
                let request = orders.squads.get_mut(&name).expect("key just listed");
                if request.release {
                    self.squads.members.remove(&name);
                    self.squads.posts.remove(&name);
                    orders.squads.remove(&name);
                    continue;
                }
                if let Some(post) = request.post {
                    match self.walkable(post.at) {
                        Ok((at, remark)) => {
                            self.squads.posts.insert(name.clone(), Post { at, ..post });
                            self.squads.remarks.extend(remark.map(|r| (name.clone(), format!("post {r}"))));
                        }
                        Err(problem) => {
                            request.post = None;
                            self.squads.remarks.insert(name.clone(), format!("post refused: {problem}"));
                        }
                    }
                    // Taken as given from here on, so a moved post is not re-examined (and re-remarked) every tick.
                    request.post = request.post.and(self.squads.posts.get(&name).copied());
                }
                let anchor = request.near.or(request.post.map(|p| p.at)).unwrap_or(self.home);
                for (unit_name, wanted) in request.take.iter_mut() {
                    let Some(def) = self.world.def_named(unit_name) else { continue };
                    let mut pool: Vec<&&OwnUnit> =
                        soldiers.iter().filter(|u| u.def == def && !self.squads.contains(u.id)).collect();
                    pool.sort_by(|a, b| a.pos.dist2d(anchor).total_cmp(&b.pos.dist2d(anchor)));
                    for unit in pool.into_iter().take(*wanted) {
                        self.army.release(unit.id);
                        self.squads.members.entry(name.clone()).or_default().push(unit.id);
                        *wanted -= 1;
                    }
                }
                request.take.retain(|_, wanted| *wanted > 0);
                if let Some((kind, to)) = request.order.take() {
                    // A one-off order ends the standing post; the commander posts the squad again when it wants.
                    match self.walkable(to) {
                        Ok((to, remark)) => {
                            self.squads.posts.remove(&name);
                            self.squads.remarks.extend(remark.map(|r| (name.clone(), format!("order {r}"))));
                            one_off.push((name.clone(), kind, to));
                        }
                        Err(problem) => {
                            self.squads.remarks.insert(name.clone(), format!("order refused: {problem}"));
                        }
                    }
                }
            }
        }

        for (name, kind, to) in one_off {
            self.fire("D-SQUAD-ORDER");
            for id in self.squads.members.get(&name).into_iter().flatten() {
                commands.push(match kind {
                    OrderKind::Move => Command::Move { unit: *id, to, queue: false },
                    OrderKind::Fight => Command::Fight { unit: *id, to, queue: false },
                });
            }
        }

        let enemies = &tick.snapshot.enemies;
        for (name, members) in &self.squads.members {
            let Some(post) = self.squads.posts.get(name).copied() else { continue };
            let units: Vec<&&OwnUnit> = soldiers.iter().filter(|u| members.contains(&u.id)).collect();
            let Some(centre) = centre_of(&units) else { continue };
            let intruder = enemies
                .iter()
                .filter(|e| e.pos.dist2d(post.at) < post.radius)
                .min_by(|a, b| a.pos.dist2d(centre).total_cmp(&b.pos.dist2d(centre)));
            self.squads.engaged.insert(name.clone(), intruder.is_some());
            let due = tick.frame - self.squads.last_order_frame.get(name).copied().unwrap_or(i32::MIN / 2) >= REORDER_FRAMES;
            if !due {
                continue;
            }
            self.squads.last_order_frame.insert(name.clone(), tick.frame);
            match intruder {
                Some(enemy) => {
                    commands.extend(units.iter().map(|u| Command::Fight { unit: u.id, to: enemy.pos, queue: false }));
                }
                None => {
                    let strays = units.iter().filter(|u| u.pos.dist2d(post.at) > post.radius * POST_SLACK);
                    commands.extend(strays.map(|u| Command::Move { unit: u.id, to: post.at, queue: false }));
                }
            }
        }
        if !self.squads.posts.is_empty() {
            self.fire("D-SQUAD-POST");
        }
        self.track_growth(tick, kit);
        self.publish_field(tick, kit, soldiers, &shared);
    }

    /// Where a commander's point really is for our bots: itself, the walkable ground nearest it (with a remark saying
    /// how far that is), or nowhere.
    fn walkable(&self, at: Vec3) -> Result<(Vec3, Option<String>), String> {
        if !self.reachable_on_foot(at) {
            return Err(format!("our bots cannot walk to ({:.0}, {:.0}) or anywhere near it", at.x, at.z));
        }
        let snapped = self.snap_to_reachable(at);
        let moved = snapped.dist2d(at);
        let remark = (moved > 48.0).then(|| format!("moved {moved:.0} to walkable ground at ({:.0}, {:.0})", snapped.x, snapped.z));
        Ok((snapped, remark))
    }

    fn publish_field(&self, tick: &Tick, kit: &Kit, soldiers: &[&OwnUnit], shared: &crate::strategist::shared::Shared) {
        let own = &tick.snapshot.own_units;
        let composition = |units: &[&&OwnUnit]| {
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for u in units {
                *counts.entry(self.name(u.def).to_string()).or_default() += 1;
            }
            counts.into_iter().collect::<Vec<_>>()
        };
        let pool: Vec<&&OwnUnit> = soldiers.iter().filter(|u| !self.squads.contains(u.id)).collect();
        let wanted = shared.field_orders.lock().unwrap().squads.clone();
        // Every squad asked for, manned or not: an empty one still has things to say (still wanted, post refused).
        let mut names: Vec<&String> = self.squads.members.keys().chain(wanted.keys()).collect();
        names.sort();
        names.dedup();
        let nobody = Vec::new();
        let squads = names
            .into_iter()
            .map(|name| {
                let members = self.squads.members.get(name).unwrap_or(&nobody);
                let units: Vec<&&OwnUnit> = soldiers.iter().filter(|u| members.contains(&u.id)).collect();
                let (health, max): (f32, f32) = units.iter().fold((0.0, 0.0), |(h, m), u| (h + u.health, m + u.max_health));
                SquadStatus {
                    name: name.clone(),
                    composition: composition(&units),
                    health_percent: if max > 0.0 { (health / max * 100.0) as u32 } else { 0 },
                    centre: centre_of(&units).map(|c| self.place(c)),
                    post: self.squads.posts.get(name).map(|p| (self.place(p.at), p.radius as u32)),
                    still_wanted: wanted.get(name).map(|r| r.take.clone().into_iter().collect()).unwrap_or_default(),
                    engaged: self.squads.engaged.get(name).copied().unwrap_or(false),
                    remark: self.squads.remarks.get(name).cloned(),
                }
            })
            .collect();
        let turrets: Vec<Vec3> = own.iter().filter(|u| u.def == kit.turret).map(|u| u.pos).collect();
        let extractors = own
            .iter()
            .filter(|u| u.def == kit.extractor)
            .map(|x| ExtractorStatus {
                at: self.place(x.pos),
                enemies_within_600: tick.snapshot.enemies.iter().filter(|e| e.pos.dist2d(x.pos) < 600.0).count(),
                turret_within_300: turrets.iter().any(|t| t.dist2d(x.pos) < 300.0),
            })
            .collect();
        let buildable = self
            .world
            .def(kit.lab)
            .map(|lab| {
                lab.build_options
                    .iter()
                    .filter_map(|id| self.world.def(*id))
                    .map(|d| (d.name.clone(), d.metal_cost as u32))
                    .collect()
            })
            .unwrap_or_default();
        let free: Vec<Vec3> = self
            .world
            .hello
            .metal_spots
            .iter()
            .filter(|s| self.spot_is_ours(**s) && !own.iter().any(|u| u.def == kit.extractor && u.pos.dist2d(**s) < 100.0))
            .copied()
            .collect();
        let metal = |u: &&OwnUnit| self.world.def(u.def).map_or(0.0, |d| d.metal_cost);
        let score = Score {
            extractors: own.iter().filter(|u| u.def == kit.extractor && !u.being_built).count(),
            extractor_peak: self.wake.extractor_peak,
            seconds_since_growth: (tick.frame - self.wake.growth_frame) / FRAMES_PER_SECOND,
            free_spots_ours: free.len(),
            free_spots_near: free.iter().filter(|s| self.walk_from_home(**s) < SCORE_NEAR).count(),
            soldiers: soldiers.len(),
            army_metal: soldiers.iter().map(metal).sum::<f32>() as u32,
            soldiers_near_home: soldiers.iter().filter(|u| u.pos.dist2d(self.home) < SCORE_AT_HOME).count(),
            enemy_extractors_seen: self.enemy_buildings.values().filter(|(def, _, _)| self.world.def(*def).is_some_and(|d| d.extracts_metal > 0.0)).count(),
            enemy_soldiers_seen_metal: self.enemy_soldiers.values().map(|(def, _)| self.world.def(*def).map_or(0.0, |d| d.metal_cost)).sum::<f32>() as u32,
            enemy_soldiers_seen_lately: self.enemy_soldiers.values().filter(|(_, seen)| tick.frame - seen < 2 * 60 * FRAMES_PER_SECOND).count(),
            enemy_soldiers_seen: self.enemy_soldiers.len(),
            enemy_army_typical: typical_enemy_army(tick.frame),
        };
        *shared.field.lock().unwrap() = Field {
            score,
            unassigned: composition(&pool),
            unassigned_centre: centre_of(&pool).map(|c| self.place(c)),
            squads,
            extractors,
            turrets: turrets.iter().map(|t| self.place(*t)).collect(),
            buildable,
            production_weights: self.production_weights.clone().into_iter().collect(),
            turret_requests_pending: self.turret_requests.len(),
        };
    }
}

/// The scoreboard's "near": free spots within this walk of home, soldiers within this of the start point.
const SCORE_NEAR: f32 = 2500.0;
const SCORE_AT_HOME: f32 = 800.0;

/// BARb medium's mean army value by minute over the 24 north-west games of v20 and v22 (opponent ground truth,
/// `run/batch_curves.py`). What we see of its army is a fragment; this is what to assume until scouting says otherwise.
const TYPICAL_ENEMY_ARMY: [(i32, f32); 8] = [(2, 70.0), (4, 520.0), (6, 1250.0), (10, 2800.0), (12, 3800.0), (15, 4700.0), (20, 5400.0), (30, 8000.0)];

fn typical_enemy_army(frame: i32) -> u32 {
    let minute = frame as f32 / (60 * FRAMES_PER_SECOND) as f32;
    let table = TYPICAL_ENEMY_ARMY;
    let after = table.iter().position(|(m, _)| *m as f32 >= minute).unwrap_or(table.len() - 1);
    if after == 0 {
        return (table[0].1 * minute / table[0].0 as f32) as u32;
    }
    let ((m0, v0), (m1, v1)) = (table[after - 1], table[after]);
    let share = ((minute - m0 as f32) / (m1 - m0) as f32).clamp(0.0, 1.0);
    (v0 + (v1 - v0) * share) as u32
}

fn centre_of(units: &[&&OwnUnit]) -> Option<Vec3> {
    if units.is_empty() {
        return None;
    }
    let n = units.len() as f32;
    Some(units.iter().fold(Vec3::default(), |sum, u| Vec3 { x: sum.x + u.pos.x / n, y: 0.0, z: sum.z + u.pos.z / n }))
}
