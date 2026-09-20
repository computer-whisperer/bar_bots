//! H-ARMY-CONTACT: enemies on our ground are grouped into parties, and every party gets its own answer in the same
//! tick: the soldiers the chase simulator (`combatsim::chase`) says are worth sending, or nobody. An answer is
//! worth what it kills and saves over sending nobody, less what it loses; the simulator knows who can catch whom.
//! Whoever answers a party keeps it until it is dead, gone, or the price has turned
//! (`docs/design/2026-09-20-army-response.md`, section 2).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bot_protocol::{Command, EnemyUnit, OwnUnit, Tick, UnitDefId, UnitId, Vec3};
use combatsim::chase::{Chase, Verdict};
use combatsim::scenario::{Intent, Vec2};
use combatsim::sim::Rules;

use super::army::BASE_RADIUS;
use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};

/// An enemy this close to a building of ours is on our ground.
const AREA_RADIUS: f32 = 900.0;
/// Enemies this close to each other are one party.
const PARTY_RADIUS: f32 = 400.0;
/// What a party may burn where it is: our buildings (and the commander) this close to it, the nearest few.
const ASSET_RADIUS: f32 = 700.0;
const MAX_ASSETS: usize = 8;
/// Nobody farther from a party than this is asked about: from 1500 an answer already arrives after the raid
/// (K-army-distance-decides-a-raid-response).
const POOL_RADIUS: f32 = 2500.0;
/// The answers priced: this many of the soldiers who would be there first.
const SIZES: [usize; 6] = [2, 4, 7, 12, 20, 32];
const SECONDS: f32 = 45.0;
const REPS: u32 = 2;
/// The simulator sees the party and not what follows it: over 1106 recorded raids it under-predicted our answer's
/// losses by 40-80 %.
const LOSS_SAFETY: f32 = 1.7;
/// A building burned costs its metal again, the builder's walk, and its income meanwhile. A guess, to be fitted.
const ASSET_WORTH: f32 = 3.0;
/// An answer goes for at least this much gained, in metal: below it the simulator's seeds decide.
const MIN_GAIN: f32 = 20.0;
/// A party at the lab or the commander is answered whatever the price.
const FORCED_RADIUS: f32 = 700.0;
const REPRICE_FRAMES: i32 = 4 * FRAMES_PER_SECOND;
/// A party nobody has seen for this long is gone.
const LOST_FRAMES: i32 = 6 * FRAMES_PER_SECOND;
/// Responders are called off when their answer has priced below [`MIN_GAIN`] this many times running: a party between
/// two extractors has nothing to burn for a moment, and one price is no reason to turn round.
const MISSES_TO_RELEASE: u32 = 2;
const PRICED_PER_TICK: usize = 3;
/// Responders are sent on when their party has moved this far from where they were sent.
const REORDER_DISTANCE: f32 = 150.0;
/// The strategist is woken for a party of at least this many at the base; lone raiders are routine.
const NOTABLE_INTRUSION: usize = 3;

struct Party {
    ids: HashSet<UnitId>,
    /// Simulator unit type and count.
    units: Vec<(usize, u32)>,
    at: Vec3,
    metal: f32,
}

struct Response {
    party: HashSet<UnitId>,
    at: Vec3,
    members: Vec<UnitId>,
    priced_at: i32,
    /// Prices running at which the members' answer was not worth having.
    misses: u32,
    last_seen: i32,
    ordered_to: Vec3,
}

/// The price of one of our parties attacking a place: what it kills (soldiers and turrets) less what it loses, with the
/// same safety on our losses as a contact answer.
pub(super) struct Assault {
    pub gain: f32,
    #[allow(dead_code)]
    pub verdict: Verdict,
}

pub(super) struct Contacts {
    rules: Arc<Rules>,
    /// The simulator's unit type for each of the game's, by name; for a unit it lacks, the nearest in metal of the
    /// same kind (mobile or not, armed or not). Filled on first use.
    sim_defs: HashMap<UnitDefId, usize>,
    responses: Vec<Response>,
    /// Simulator time this minute, for the log: (questions, total ms, longest ms).
    spent: (u32, f64, f64),
}

impl Default for Contacts {
    fn default() -> Self {
        Contacts { rules: Arc::new(Rules::default()), sim_defs: HashMap::new(), responses: Vec::new(), spent: (0, 0.0, 0.0) }
    }
}

fn flat(pos: Vec3) -> Vec2 {
    Vec2::new(pos.x, pos.z)
}

impl Brain {
    fn survey_sim_defs(&mut self) {
        let rules = self.contacts.rules.clone();
        let table = &rules.units;
        for def in &self.world.hello.unit_defs {
            // Aircraft have no place in a ground chase.
            if def.speed > 0.0 && def.move_class.is_none() {
                continue;
            }
            let kind = |mobile: bool, armed: bool| (mobile, armed) == (def.speed > 0.0, def.weapon_count > 0);
            let stand_in = || {
                let like = table.list.iter().enumerate().filter(|(_, u)| !u.air && kind(u.mobile(), u.reach() > 0.0));
                like.min_by(|a, b| (a.1.metal - def.metal_cost).abs().total_cmp(&(b.1.metal - def.metal_cost).abs())).map(|(i, _)| i)
            };
            if let Some(index) = table.index(&def.name).or_else(stand_in) {
                self.contacts.sim_defs.insert(def.id, index);
            }
        }
    }

    /// The enemy's mobile ground units on our ground, grouped.
    fn parties(&self, tick: &Tick) -> Vec<Party> {
        let snapshot = &tick.snapshot;
        let buildings: Vec<Vec3> = snapshot.own_units.iter().filter(|u| self.world.def(u.def).is_some_and(|d| d.speed == 0.0)).map(|u| u.pos).collect();
        let ally_bases: Vec<Vec3> = if self.enabled("H-TEAM-DEFEND") { self.ally_starts.values().copied().collect() } else { Vec::new() };
        let ours = |p: Vec3| {
            p.dist2d(self.home) < BASE_RADIUS || ally_bases.iter().any(|b| b.dist2d(p) < BASE_RADIUS) || buildings.iter().any(|b| b.dist2d(p) < AREA_RADIUS)
        };
        // A radar contact is taken for the soldier of theirs we have seen most, or for one like our own.
        let mut seen: HashMap<UnitDefId, usize> = HashMap::new();
        self.enemy_soldiers.values().for_each(|(def, _)| *seen.entry(*def).or_default() += 1);
        let blip = seen.into_iter().max_by_key(|(def, n)| (*n, def.0)).map(|(def, _)| def).or(self.kit.as_ref().map(|k| k.line));
        let walks = |e: &&EnemyUnit| e.def.is_none_or(|d| self.world.def(d).is_some_and(|d| d.speed > 0.0 && d.move_class.is_some()));
        let contacts: Vec<&EnemyUnit> = snapshot.enemies.iter().filter(walks).filter(|e| ours(e.pos)).collect();

        let mut party_of: Vec<Option<usize>> = vec![None; contacts.len()];
        let mut parties = Vec::new();
        for start in 0..contacts.len() {
            if party_of[start].is_some() {
                continue;
            }
            party_of[start] = Some(parties.len());
            let mut members = vec![start];
            let mut next = 0;
            while next < members.len() {
                let from = contacts[members[next]].pos;
                for other in 0..contacts.len() {
                    if party_of[other].is_none() && contacts[other].pos.dist2d(from) < PARTY_RADIUS {
                        party_of[other] = Some(parties.len());
                        members.push(other);
                    }
                }
                next += 1;
            }
            let n = members.len() as f32;
            let at = members.iter().fold(Vec3::default(), |sum, i| Vec3 { x: sum.x + contacts[*i].pos.x / n, y: 0.0, z: sum.z + contacts[*i].pos.z / n });
            let mut units: HashMap<usize, u32> = HashMap::new();
            let mut metal = 0.0;
            for i in &members {
                let Some(def) = contacts[*i].def.or(blip) else { continue };
                metal += self.world.def(def).map_or(0.0, |d| d.metal_cost);
                if let Some(index) = self.contacts.sim_defs.get(&def) {
                    *units.entry(*index).or_default() += 1;
                }
            }
            let mut units: Vec<(usize, u32)> = units.into_iter().collect();
            units.sort();
            parties.push(Party { ids: members.iter().map(|i| contacts[*i].id).collect(), units, at, metal });
        }
        parties
    }

    /// What sending `pursuers` after `party` comes to.
    fn chase_verdict(&mut self, party: &Party, assets: &[(usize, Vec2)], pursuers: &[&OwnUnit], party_buildings: &[(usize, Vec2)]) -> Verdict {
        // Soldiers of one type standing together are one group of the simulator's.
        let mut groups: HashMap<(usize, i32, i32), (u32, f32, f32)> = HashMap::new();
        for unit in pursuers {
            let Some(def) = self.contacts.sim_defs.get(&unit.def) else { continue };
            let group = groups.entry((*def, (unit.pos.x / 300.0) as i32, (unit.pos.z / 300.0) as i32)).or_default();
            *group = (group.0 + 1, group.1 + unit.pos.x, group.2 + unit.pos.z);
        }
        let mut pursuers: Vec<(usize, u32, Vec2)> = groups.into_iter().map(|((def, _, _), (n, x, z))| (def, n, Vec2::new(x / n as f32, z / n as f32))).collect();
        pursuers.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)).then(a.2.x.total_cmp(&b.2.x)));
        let rules = self.contacts.rules.clone();
        let burns = assets.iter().any(|(def, _)| rules.units.list[*def].reach() == 0.0);
        let intent = if burns { Intent::Raid { then: flat(self.enemy_base(party.at)) } } else { Intent::Fight };
        let chase = Chase { pursuers, party: party.units.clone(), at: flat(party.at), intent, assets: assets.to_vec(), party_buildings: party_buildings.to_vec(), seconds: SECONDS };
        let started = std::time::Instant::now();
        let verdict = chase.verdict(&rules, REPS);
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let spent = &mut self.contacts.spent;
        *spent = (spent.0 + 1, spent.1 + ms, spent.2.max(ms));
        verdict
    }

    /// What `party` of ours attacking `at` comes to, against what is known to stand within `radius` of it: remembered
    /// armed buildings, and soldiers in sight (radar contacts taken for the enemy's usual soldier). H-ARMY-PRESSURE
    /// prices its raids by this; the same simulator and the same safety on our losses as the contact response.
    pub(super) fn assault_verdict(&mut self, party: &[&OwnUnit], at: Vec3, radius: f32, tick: &Tick) -> Assault {
        if self.contacts.sim_defs.is_empty() {
            self.survey_sim_defs();
        }
        let mut seen: HashMap<UnitDefId, usize> = HashMap::new();
        self.enemy_soldiers.values().for_each(|(def, _)| *seen.entry(*def).or_default() += 1);
        let blip = seen.into_iter().max_by_key(|(def, n)| (*n, def.0)).map(|(def, _)| def).or(self.kit.as_ref().map(|k| k.line));
        let mut theirs: HashMap<usize, u32> = HashMap::new();
        let mut turrets: Vec<(usize, Vec2)> = Vec::new();
        let mut metal = 0.0;
        for (def, pos, _) in self.enemy_buildings.values() {
            if pos.dist2d(at) < radius
                && self.world.def(*def).is_some_and(|d| d.weapon_count > 0)
                && let Some(index) = self.contacts.sim_defs.get(def)
            {
                turrets.push((*index, flat(*pos)));
            }
        }
        for enemy in tick.snapshot.enemies.iter().filter(|e| e.pos.dist2d(at) < radius) {
            let Some(def) = enemy.def.or(blip) else { continue };
            let Some(d) = self.world.def(def) else { continue };
            if d.speed > 0.0 && d.weapon_count > 0 && d.move_class.is_some()
                && let Some(index) = self.contacts.sim_defs.get(&def)
            {
                *theirs.entry(*index).or_default() += 1;
                metal += d.metal_cost;
            }
        }
        let mut units: Vec<(usize, u32)> = theirs.into_iter().collect();
        units.sort();
        let their_party = Party { ids: HashSet::new(), units, at, metal };
        let verdict = self.chase_verdict(&their_party, &[], party, &turrets);
        let gain = verdict.party_killed - LOSS_SAFETY * verdict.pursuers_lost;
        Assault { gain, verdict }
    }

    /// Answers every party on our ground from `free` (the home group) and returns who is answering one.
    pub(super) fn run_contacts(&mut self, tick: &Tick, kit: &Kit, free: &[&OwnUnit], commands: &mut Vec<Command>) -> HashSet<UnitId> {
        if !self.enabled("H-ARMY-CONTACT") {
            self.contacts.responses.clear();
            return HashSet::new();
        }
        if self.contacts.sim_defs.is_empty() {
            self.survey_sim_defs();
        }
        let snapshot = &tick.snapshot;
        let parties = self.parties(tick);
        let at_base = parties.iter().filter(|p| p.at.dist2d(self.home) < BASE_RADIUS).max_by_key(|p| p.ids.len());
        if let Some(party) = at_base.filter(|p| p.ids.len() >= NOTABLE_INTRUSION) {
            self.trigger("base-attack", tick.frame, format!("Our base is under attack: {} enemies near {}.", party.ids.len(), self.world.grid(party.at)));
        }

        // Each response goes on with the party that holds most of its units, or stands nearest to where it was.
        let mut previous = std::mem::take(&mut self.contacts.responses);
        let mut responses: Vec<Response> = Vec::new();
        for party in &parties {
            let shared = |r: &Response| r.party.intersection(&party.ids).count();
            let by_units = (0..previous.len()).filter(|i| shared(&previous[*i]) > 0).max_by_key(|i| shared(&previous[*i]));
            let by_place = || (0..previous.len()).filter(|i| previous[*i].at.dist2d(party.at) < PARTY_RADIUS).min_by(|a, b| previous[*a].at.dist2d(party.at).total_cmp(&previous[*b].at.dist2d(party.at)));
            let mut response = match by_units.or_else(by_place) {
                Some(index) => previous.swap_remove(index),
                None => Response { party: HashSet::new(), at: party.at, members: Vec::new(), priced_at: i32::MIN / 2, misses: 0, last_seen: tick.frame, ordered_to: Vec3::default() },
            };
            (response.party, response.at, response.last_seen) = (party.ids.clone(), party.at, tick.frame);
            response.members.retain(|id| free.iter().any(|u| u.id == *id));
            responses.push(response);
        }
        // Out of sight for a moment is not gone: its responders carry on to where it was.
        for mut response in previous {
            response.members.retain(|id| free.iter().any(|u| u.id == *id));
            if tick.frame - response.last_seen < LOST_FRAMES && !response.members.is_empty() {
                responses.push(response);
            } else {
                commands.extend(response.members.iter().map(|id| Command::Move { unit: *id, to: self.last_station, queue: false }));
            }
        }

        // The parties due a price, the dearest first; the rest wait a tick.
        let stake = |index: usize| parties.get(index).map_or(0.0, |p| p.metal);
        let mut due: Vec<usize> = (0..parties.len()).filter(|i| tick.frame - responses[*i].priced_at >= REPRICE_FRAMES).collect();
        due.sort_by(|a, b| stake(*b).total_cmp(&stake(*a)));
        for index in due.into_iter().take(PRICED_PER_TICK) {
            let party = &parties[index];
            let taken: HashSet<UnitId> = responses.iter().enumerate().filter(|(i, _)| *i != index).flat_map(|(_, r)| r.members.iter().copied()).collect();
            let eta = |u: &OwnUnit| u.pos.dist2d(party.at) / self.world.def(u.def).map_or(1.0, |d| d.speed.max(1.0));
            let members = &responses[index].members;
            let mut pool: Vec<&OwnUnit> = free.iter().copied().filter(|u| members.contains(&u.id)).collect();
            let mut others: Vec<&OwnUnit> =
                free.iter().copied().filter(|u| !members.contains(&u.id) && !taken.contains(&u.id) && u.pos.dist2d(party.at) < POOL_RADIUS).collect();
            others.sort_by(|a, b| eta(a).total_cmp(&eta(b)));
            pool.extend(others);

            let mut near: Vec<&OwnUnit> = snapshot
                .own_units
                .iter()
                .filter(|u| (u.def == kit.commander || self.world.def(u.def).is_some_and(|d| d.speed == 0.0)) && u.pos.dist2d(party.at) < ASSET_RADIUS)
                .collect();
            near.sort_by(|a, b| a.pos.dist2d(party.at).total_cmp(&b.pos.dist2d(party.at)));
            let forced = near.iter().any(|u| (u.def == kit.commander || u.def == kit.lab) && u.pos.dist2d(party.at) < FORCED_RADIUS);
            let mut assets: Vec<(usize, Vec2)> = near.iter().take(MAX_ASSETS).filter_map(|u| Some((*self.contacts.sim_defs.get(&u.def)?, flat(u.pos)))).collect();
            let allied = self.allies.iter().filter(|a| self.world.def(a.def).is_some_and(|d| d.speed == 0.0) && a.pos.dist2d(party.at) < ASSET_RADIUS);
            let allied: Vec<(usize, Vec2)> = allied.filter_map(|a| Some((*self.contacts.sim_defs.get(&a.def)?, flat(a.pos)))).collect();
            assets.extend(allied.into_iter().take(MAX_ASSETS.saturating_sub(assets.len())));

            let nobody = self.chase_verdict(party, &assets, &[], &[]);
            let gain = |v: &Verdict| (v.party_killed - nobody.party_killed) - LOSS_SAFETY * (v.pursuers_lost - nobody.pursuers_lost) - ASSET_WORTH * (v.assets_lost - nobody.assets_lost);
            // Whoever is on it stays on it: only more, or everybody off.
            let least = members.len().max(1);
            let mut sizes: Vec<usize> = SIZES.iter().copied().chain([least, pool.len().min(SIZES[SIZES.len() - 1])]).filter(|k| *k >= least && *k <= pool.len()).collect();
            sizes.sort();
            sizes.dedup();
            let mut best: Option<(usize, f32, Verdict)> = None;
            for size in sizes {
                let verdict = self.chase_verdict(party, &assets, &pool[..size], &[]);
                let worth = gain(&verdict);
                if best.is_none_or(|(_, most, _)| worth > most) {
                    best = Some((size, worth, verdict));
                } else if verdict.survived == 0.0 {
                    // The party dies to fewer: more soldiers only walk farther.
                    break;
                }
            }
            let chosen = best.filter(|(_, worth, _)| forced || *worth >= MIN_GAIN);
            let before = responses[index].members.len();
            let misses = if chosen.is_some() { 0 } else { responses[index].misses + 1 };
            let sent: Vec<UnitId> = match chosen {
                Some((size, _, _)) => pool[..size].iter().map(|u| u.id).collect(),
                None if before > 0 && misses < MISSES_TO_RELEASE => responses[index].members.clone(),
                None => Vec::new(),
            };
            if sent.is_empty() {
                commands.extend(responses[index].members.iter().map(|id| Command::Move { unit: *id, to: self.last_station, queue: false }));
            }
            if sent.len() != before {
                let (worth, caught) = chosen.map_or((best.map_or(0.0, |b| b.1), 0.0), |(_, worth, v)| (worth, v.caught));
                eprintln!(
                    "[ai {}] f={} contact: {} units ({:.0} metal) at ({:.0}, {:.0}){}: {} answer (were {before}) of {} within reach, worth {worth:.0}, caught {caught:.1}; nobody: {:.0} burned",
                    self.ai(), tick.frame, party.ids.len(), party.metal, party.at.x, party.at.z, if forced { ", at the lab or the commander" } else { "" },
                    sent.len(), pool.len(), nobody.assets_lost
                );
                self.journal.note(
                    tick.frame,
                    "contact",
                    serde_json::json!({ "party": party.ids.len(), "metal": party.metal, "at": [party.at.x, party.at.z], "forced": forced, "within_reach": pool.len(), "burned_if_nobody": nobody.assets_lost }),
                    serde_json::json!({ "sent": sent.len(), "worth": worth, "caught": caught }),
                );
            }
            if !sent.is_empty() {
                self.fire("H-ARMY-CONTACT");
            }
            let response = &mut responses[index];
            (response.members, response.priced_at, response.misses, response.ordered_to) = (sent, tick.frame, misses, Vec3::default());
        }

        for response in &mut responses {
            let moved = response.ordered_to.dist2d(response.at) > REORDER_DISTANCE;
            let orders = free.iter().filter(|u| response.members.contains(&u.id) && (moved || u.idle));
            commands.extend(orders.map(|u| Command::Fight { unit: u.id, to: response.at, queue: false }));
            if moved {
                response.ordered_to = response.at;
            }
        }
        if tick.frame % (60 * FRAMES_PER_SECOND) == 0 && self.contacts.spent.0 > 0 {
            let (questions, total, longest) = std::mem::take(&mut self.contacts.spent);
            eprintln!("[ai {}] f={} contact simulator this minute: {questions} questions, {total:.0} ms, longest {longest:.1} ms", self.ai(), tick.frame);
        }
        let answering = responses.iter().flat_map(|r| r.members.iter().copied()).collect();
        self.contacts.responses = responses;
        answering
    }
}
