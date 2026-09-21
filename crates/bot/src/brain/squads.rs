//! Squads: soldiers the field commander has claimed (`DESIGN.md`, "Field commander"). The commander asks by name
//! and unit type; membership, posts and orders are carried out here. Everything unclaimed stays with the heuristics.

use std::collections::{BTreeMap, HashMap, HashSet};

use bot_protocol::{Command, OwnUnit, Tick, UnitId, Vec3};

use super::roster::Kit;
use super::territory::Ground;
use super::{Brain, FRAMES_PER_SECOND};
use crate::strategist::shared::{ExtractorStatus, Field, OrderKind, Post, Score, SquadStatus};

/// A squad's standing orders are re-issued no more often than this.
const REORDER_FRAMES: i32 = 2 * FRAMES_PER_SECOND;
/// Members of a posted squad this far from the post (as a share of its radius) with nothing to fight walk back.
const POST_SLACK: f32 = 0.4;
/// Intruders this close together are one group.
const GROUP_RADIUS: f32 = 400.0;
/// A posted squad meets intruders with its nearest members, at least this many and as many as these odds take.
const MIN_RESPONDERS: usize = 4;
const RESPONSE_ODDS: f32 = 1.5;

#[derive(Default)]
pub struct Squads {
    members: BTreeMap<String, Vec<UnitId>>,
    posts: HashMap<String, Post>,
    last_order_frame: HashMap<String, i32>,
    engaged: HashMap<String, bool>,
    /// What the commander should know about its last post or order for a squad (moved to walkable ground, or refused).
    remarks: HashMap<String, String>,
    /// H-ARMY-MARCH: where a squad under a `fight` order is going, and which of its members are waiting for the rest.
    marches: HashMap<String, (Vec3, HashSet<UnitId>)>,
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

        // Several seats of ours may serve one commander (`strategist/seats.rs`): an order or a release is carried out
        // once by each and dropped when all have; a turret request goes to the seat whose home is nearest.
        let team = self.world.hello.team;
        let homes = shared.seat_homes();
        let everyone_has = |seen: &std::collections::BTreeSet<i32>| homes.iter().all(|(seat, _)| seen.contains(seat));
        let mut one_off: Vec<(String, OrderKind, Vec3)> = Vec::new();
        {
            let mut orders = shared.field_orders.lock().unwrap();
            let (mine, others): (Vec<Vec3>, Vec<Vec3>) =
                orders.turret_requests.drain(..).partition(|at| crate::strategist::seats::nearest_seat(&homes, *at).is_none_or(|seat| seat == team));
            orders.turret_requests = others;
            self.turret_requests.extend(mine);
            self.production_weights = orders.production.clone();
            self.spot_priority = orders.spot_priority.clone();
            self.spot_avoid = orders.spot_avoid.clone();
            let names: Vec<String> = orders.squads.keys().cloned().collect();
            for name in names {
                let request = orders.squads.get_mut(&name).expect("key just listed");
                if request.release {
                    self.squads.members.remove(&name);
                    self.squads.posts.remove(&name);
                    self.squads.marches.remove(&name);
                    request.seen_by.insert(team);
                    if everyone_has(&request.seen_by) {
                        orders.squads.remove(&name);
                    }
                    continue;
                }
                if let Some(post) = request.post {
                    match self.walkable(post.at) {
                        Ok((at, remark)) => {
                            self.squads.posts.insert(name.clone(), Post { at, ..post });
                            self.squads.marches.remove(&name);
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
                let order = request.order.filter(|_| request.seen_by.insert(team));
                if request.order.is_some() && everyone_has(&request.seen_by) {
                    request.order = None;
                    request.seen_by.clear();
                }
                if let Some((kind, to)) = order {
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
            // A withdrawal (`move`) is not slowed down; an advance arrives together.
            match kind {
                OrderKind::Fight => self.squads.marches.insert(name.clone(), (to, HashSet::new())),
                OrderKind::Move => self.squads.marches.remove(&name),
            };
            for id in self.squads.members.get(&name).into_iter().flatten() {
                commands.push(match kind {
                    OrderKind::Move => Command::Move { unit: *id, to, queue: false },
                    OrderKind::Fight => Command::Fight { unit: *id, to, queue: false },
                });
            }
        }

        let enemies = &tick.snapshot.enemies;
        let mut marches = std::mem::take(&mut self.squads.marches);
        for (name, (to, held)) in marches.iter_mut() {
            let members = self.squads.members.get(name).map(Vec::as_slice).unwrap_or_default();
            let group: Vec<&OwnUnit> = soldiers.iter().filter(|u| members.contains(&u.id)).copied().collect();
            commands.extend(self.march(held, &group, *to, enemies.as_slice()));
        }
        self.squads.marches = marches;

        let mut responses: Vec<Command> = Vec::new();
        for (name, members) in &self.squads.members {
            let Some(post) = self.squads.posts.get(name).copied() else { continue };
            let units: Vec<&&OwnUnit> = soldiers.iter().filter(|u| members.contains(&u.id)).collect();
            let Some(centre) = centre_of(&units) else { continue };
            // The biggest group inside the post, the nearest of equals: a squad of 38 used to turn, all of it, on the
            // single nearest intruder every two seconds, and one Grunt walked it 1800 elmos (commander game 7/01).
            let inside: Vec<&bot_protocol::EnemyUnit> = enemies.iter().filter(|e| e.pos.dist2d(post.at) < post.radius).collect();
            let company = |e: &bot_protocol::EnemyUnit| inside.iter().filter(|o| o.pos.dist2d(e.pos) < GROUP_RADIUS).count();
            let intruder = inside
                .iter()
                .copied()
                .max_by(|a, b| company(a).cmp(&company(b)).then(b.pos.dist2d(centre).total_cmp(&a.pos.dist2d(centre))));
            self.squads.engaged.insert(name.clone(), intruder.is_some());
            let due = tick.frame - self.squads.last_order_frame.get(name).copied().unwrap_or(i32::MIN / 2) >= REORDER_FRAMES;
            if !due {
                continue;
            }
            self.squads.last_order_frame.insert(name.clone(), tick.frame);
            match intruder {
                Some(enemy) => {
                    // As many of the nearest as good odds take; the rest keep the post.
                    let mut nearest: Vec<&OwnUnit> = units.iter().map(|u| **u).collect();
                    nearest.sort_by(|a, b| a.pos.dist2d(enemy.pos).total_cmp(&b.pos.dist2d(enemy.pos)));
                    let theirs = self.known_enemy_force(enemy.pos, GROUP_RADIUS, enemies.as_slice());
                    let enough = (MIN_RESPONDERS.min(nearest.len())..=nearest.len())
                        .find(|&k| self.odds(&Self::force_of(&nearest[..k]), &theirs) >= RESPONSE_ODDS)
                        .unwrap_or(nearest.len());
                    responses.extend(nearest.iter().take(enough).map(|u| Command::Fight { unit: u.id, to: enemy.pos, queue: false }));
                    let rest = nearest.iter().skip(enough).filter(|u| u.pos.dist2d(post.at) > post.radius * POST_SLACK);
                    responses.extend(rest.map(|u| Command::Move { unit: u.id, to: post.at, queue: false }));
                }
                None => {
                    let strays = units.iter().filter(|u| u.pos.dist2d(post.at) > post.radius * POST_SLACK);
                    responses.extend(strays.map(|u| Command::Move { unit: u.id, to: post.at, queue: false }));
                }
            }
        }
        commands.extend(responses);
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

    /// The field the commander and the player read, and the wake conditions watch; published by the army rules here
    /// and by the pianist in its mode.
    pub(super) fn publish_field(&self, tick: &Tick, kit: &Kit, soldiers: &[&OwnUnit], shared: &crate::strategist::shared::Shared) {
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
            .filter(|u| kit.is_extractor(u.def))
            .map(|x| ExtractorStatus {
                at: self.place(x.pos),
                spot: self.world.hello.metal_spots.iter().position(|s| s.dist2d(x.pos) < 100.0),
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
        let enemy_extractors: Vec<Vec3> = self
            .enemy_buildings
            .values()
            .filter(|(def, _, _)| self.world.def(*def).is_some_and(|d| d.extracts_metal > 0.0))
            .map(|(_, pos, _)| *pos)
            .collect();
        let our_extractors: Vec<Vec3> = own.iter().filter(|u| kit.is_extractor(u.def)).map(|u| u.pos).collect();
        let held = |spot: Vec3, by: &[Vec3]| by.iter().any(|p| p.dist2d(spot) < 100.0);
        let free: Vec<Vec3> = self
            .world
            .hello
            .metal_spots
            .iter()
            .filter(|s| self.reachable_on_foot(**s))
            .filter(|s| !held(**s, &our_extractors) && !held(**s, &enemy_extractors) && !self.allied_extractor_on(**s))
            .copied()
            .collect();
        let recent = |seen: &i32| tick.frame - seen < 3 * 60 * FRAMES_PER_SECOND;
        let metal = |u: &&OwnUnit| self.world.def(u.def).map_or(0.0, |d| d.metal_cost);
        let traded = |when: &dyn Fn(i32) -> bool| {
            let sum = |pick: &dyn Fn(&(i32, f32, f32)) -> f32| self.trade_log.iter().filter(|t| when(t.0)).map(pick).sum::<f32>() as u32;
            (sum(&|t| t.1), sum(&|t| t.2))
        };
        let score = Score {
            extractors: own.iter().filter(|u| kit.is_extractor(u.def) && !u.being_built).count(),
            extractor_peak: self.wake.extractor_peak,
            seconds_since_growth: (tick.frame - self.wake.growth_frame) / FRAMES_PER_SECOND,
            free_spots: free.len(),
            next_free: {
                let mut nearest: Vec<(usize, Vec3, f32)> = self
                    .world
                    .hello
                    .metal_spots
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| free.iter().any(|f| f.dist2d(**s) < 1.0))
                    .map(|(n, s)| (n, *s, self.walk_from_home(*s)))
                    .collect();
                nearest.sort_by(|a, b| a.2.total_cmp(&b.2));
                nearest.into_iter().take(NEXT_FREE).map(|(n, s, walk)| (n, self.place(s), walk as u32, self.ground(s).word())).collect()
            },
            soldiers: soldiers.len(),
            army_metal: soldiers.iter().map(metal).sum::<f32>() as u32,
            soldiers_near_home: soldiers.iter().filter(|u| u.pos.dist2d(self.home) < SCORE_AT_HOME).count(),
            metal_income: tick.snapshot.metal.income,
            trend: [3, 6].into_iter().filter_map(|m| self.minutes_ago(tick.frame, m).map(|(x, income, army)| (m, x, income, army))).collect(),
            extractors_lost_3_min: self.wake.losses.len(),
            traded_3_min: traded(&|frame| recent(&frame)),
            traded: traded(&|_| true),
            seconds_since_turn: Some(shared.last_turn_frame.load(std::sync::atomic::Ordering::Relaxed)).filter(|at| *at > 0).map(|at| (tick.frame - at) / FRAMES_PER_SECOND),
            enemy_base_found: self.found_enemy_base().map(|pos| self.place(pos)),
            enemy_bases: self.enemy_bases.iter().map(|b| (b.team, self.place(b.at), b.found, b.dead)).collect(),
            enemy_spots_seen: enemy_extractors.len(),
            enemy_factories: self
                .enemy_buildings
                .values()
                .filter(|(def, _, _)| self.world.def(*def).is_some_and(|d| !d.build_options.is_empty()))
                .map(|(_, pos, _)| self.place(*pos))
                .collect(),
            raid_targets: self
                .raid_targets()
                .into_iter()
                .take(6)
                .map(|t| (self.place(t), self.known_enemy_force(t, 500.0, &[]).turret_metal as u32))
                .collect(),
            guess_disproved: {
                let guess = self.enemy_base(self.home);
                let standing_there = soldiers.iter().any(|u| u.pos.dist2d(guess) < 500.0);
                let base_known = self.enemy_buildings.values().any(|(_, pos, _)| pos.dist2d(guess) < 900.0);
                (standing_there && !base_known).then(|| {
                    let mut spots: Vec<Vec3> = self
                        .world
                        .hello
                        .metal_spots
                        .iter()
                        .filter(|s| self.reachable_on_foot(**s) && !soldiers.iter().any(|u| u.pos.dist2d(**s) < 600.0))
                        .copied()
                        .collect();
                    spots.sort_by(|a, b| a.dist2d(guess).total_cmp(&b.dist2d(guess)));
                    spots.into_iter().take(4).map(|s| self.place(s)).collect()
                })
            },
            enemy_commander: self.enemy_commander_seen.map(|(pos, seen)| (self.place(pos), (tick.frame - seen) / FRAMES_PER_SECOND)),
            enemy_soldiers_seen: self.enemy_soldiers.values().filter(|(_, seen)| recent(seen)).count(),
            enemy_soldiers_seen_metal: self.enemy_soldiers.values().filter(|(_, seen)| recent(seen)).map(|(def, _)| self.world.def(*def).map_or(0.0, |d| d.metal_cost)).sum::<f32>() as u32,
        };
        let count = |ground: Ground| free.iter().filter(|s| self.ground(**s) == ground).count();
        let mut raided: Vec<(Vec3, f32)> = self.world.hello.metal_spots.iter().map(|s| (*s, self.territory.raided(*s))).filter(|(_, metal)| *metal >= 100.0).collect();
        raided.sort_by(|a, b| b.1.total_cmp(&a.1));
        raided.dedup_by(|a, b| self.world.grid(a.0) == self.world.grid(b.0));
        let ground = crate::strategist::shared::GroundReport {
            free_spots: (count(Ground::Held), count(Ground::Contested), count(Ground::Theirs)),
            extractors_exposed: own.iter().filter(|u| kit.is_extractor(u.def) && self.ground(u.pos) != Ground::Held).map(|u| self.place(u.pos)).collect(),
            posts: std::iter::once(self.last_station).chain(self.army.detachment_posts()).map(|p| self.place(p)).collect(),
            raided: raided.into_iter().take(4).map(|(at, metal)| (self.place(at), metal as u32)).collect(),
        };
        if shared.lead().is_none_or(|lead| lead == self.world.hello.team) {
            *shared.ground_sketch.lock().unwrap() = self.territory.sketch();
        }
        let mut wreck_fields: Vec<(crate::strategist::shared::Place, u32, bool)> = self.reclaim.fields.iter().map(|f| (self.place(f.at), f.metal as u32, f.safe)).collect();
        wreck_fields.sort_by_key(|f| std::cmp::Reverse(f.1));
        shared.publish_field(self.world.hello.team, Field {
            score,
            ground,
            wreck_fields,
            resurrection_bots: own.iter().filter(|u| kit.is_resurrector(u.def)).count(),
            unassigned: composition(&pool),
            unassigned_centre: centre_of(&pool).map(|c| self.place(c)),
            squads,
            extractors,
            turrets: turrets.iter().map(|t| self.place(*t)).collect(),
            buildable,
            production_weights: self.production_weights.clone().into_iter().collect(),
            turret_requests_pending: self.turret_requests.len(),
            spot_plan: {
                // What taking the spot has cost so far and whether anything of ours stands by it: the commander's list
                // overrides the bot's caution about raided ground, so it is shown what that caution would have said.
                let name = |i: &usize| {
                    let Some(spot) = self.world.hello.metal_spots.get(*i) else { return format!("#{i}") };
                    let lost = self.spot_losses.get(i).map_or(String::new(), |n| format!(", lost here {n} times"));
                    let ground = if held(*spot, &our_extractors) { String::new() } else { format!(", {} ground", self.ground(*spot).word()) };
                    format!("#{i} {}{lost}{ground}", self.world.grid(*spot))
                };
                let list = |spots: &[usize]| spots.iter().map(name).collect::<Vec<_>>().join(", ");
                match (self.spot_priority.is_empty(), self.spot_avoid.is_empty()) {
                    (true, true) => String::new(),
                    _ => format!("take first: {}; leave alone: {}", list(&self.spot_priority), list(&self.spot_avoid)),
                }
            },
        });
    }
}

/// How many of the nearest free spots the score line names.
const NEXT_FREE: usize = 5;
/// Soldiers within this of the start point are "at home" on the score line.
const SCORE_AT_HOME: f32 = 800.0;

fn centre_of(units: &[&&OwnUnit]) -> Option<Vec3> {
    if units.is_empty() {
        return None;
    }
    let n = units.len() as f32;
    Some(units.iter().fold(Vec3::default(), |sum, u| Vec3 { x: sum.x + u.pos.x / n, y: 0.0, z: sum.z + u.pos.z / n }))
}
