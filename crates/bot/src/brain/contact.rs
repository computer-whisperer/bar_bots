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

use super::army::CONTACT_RADIUS;

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
    /// Their turrets the answer was priced against, nearest the party first: the answer kills them first.
    turrets: Vec<UnitId>,
    /// How many armed buildings of theirs were known within reach of the party at the last pricing: a new one
    /// means a new price now, not in four seconds.
    turrets_known: usize,
}

/// The price of one of our parties attacking a place: what it kills (soldiers and turrets) less what it loses, with the
/// same safety on our losses as a contact answer.
pub(super) struct Assault {
    pub gain: f32,
    /// Their turrets priced in, nearest the party first: what the party kills first.
    pub turrets: Vec<UnitId>,
    pub verdict: Verdict,
}

pub(super) struct Contacts {
    pub(super) rules: Arc<Rules>,
    /// The simulator's unit type for each of the game's, by name; for a unit it lacks, the nearest in metal of the
    /// same kind (mobile or not, armed or not). Filled on first use.
    pub(super) sim_defs: HashMap<UnitDefId, usize>,
    responses: Vec<Response>,
    /// Simulator time this minute, for the log: (questions, total ms, longest ms).
    spent: (u32, f64, f64),
}

impl Default for Contacts {
    fn default() -> Self {
        // Their commander presses its D-gun (K-barb-commander-dgun-beats-a-pawn-party); ours never does in a scene.
        let tuning = combatsim::sim::Tuning { dgun: true, ..Default::default() };
        Contacts { rules: Arc::new(Rules::new(combatsim::units::Units::default(), tuning)), sim_defs: HashMap::new(), responses: Vec::new(), spent: (0, 0.0, 0.0) }
    }
}

fn flat(pos: Vec3) -> Vec2 {
    Vec2::new(pos.x, pos.z)
}

/// Distance from `p` to the segment `a`-`b`.
pub(super) fn to_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let (dx, dz) = (b.x - a.x, b.z - a.z);
    let len2 = dx * dx + dz * dz;
    let t = if len2 <= 0.0 { 0.0 } else { (((p.x - a.x) * dx + (p.z - a.z) * dz) / len2).clamp(0.0, 1.0) };
    (p.x - (a.x + t * dx)).hypot(p.z - (a.z + t * dz))
}

/// A turret has this much beyond its range for the approach: a unit walking past at the edge of its range is shot.
pub(super) const TURRET_MARGIN: f32 = 100.0;
/// A raid on a building of theirs burns what stands within this of it, and is priced against the turrets there.
const RAID_ASSETS: f32 = 600.0;
/// The raid's safety on our losses: metal for metal (Matt paid seven Pawns, 378, for 1,270 of base; the contact
/// answer's 1.7 is for pursuits whose losses the simulator under-predicts).
const RAID_SAFETY: f32 = 1.0;
/// A base's worth of assets in the raid scene (the contact answer's 8 is for our outposts).
const RAID_MAX_ASSETS: usize = 12;
/// What a base of BARb's holds that we have not seen, priced in when the target lies at its base: the lab and the
/// extractors and winds round it (K-barb-medium-observed-build; its true state at 3:00-5:00 in the players' and our
/// games: 3-4 extractors, 4-7 winds, the lab), and its towers by the clock (1 by 2:00, 2 by 3:00, 3 by 4:00, 4 by
/// 5:00). Estimated the way a player does, because a party priced only against what it has seen walked into
/// three towers it had not (raid-price-debug: fifteen Pawns in a minute at "lose 0").
const EXPECTED_BASE_EXTRACTORS: usize = 3;
const EXPECTED_BASE_WINDS: usize = 4;
fn expected_towers(frame: i32) -> usize {
    ((frame as f32 / (60.0 * FRAMES_PER_SECOND as f32)) - 1.0).clamp(0.0, 4.0) as usize
}

impl Brain {
    pub(super) fn survey_sim_defs(&mut self) {
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
        let chase = Chase { pursuers, party: party.units.clone(), at: flat(party.at), intent, assets: assets.to_vec(), party_buildings: party_buildings.to_vec(), pursuer_buildings: Vec::new(), seconds: SECONDS };
        let started = std::time::Instant::now();
        let verdict = chase.verdict(&rules, REPS);
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let spent = &mut self.contacts.spent;
        *spent = (spent.0 + 1, spent.1 + ms, spent.2.max(ms));
        verdict
    }

    /// Their armed buildings that bear on `at` (within `radius` of it) or on the way there from `from` (within their
    /// own range and a margin of the line), with their simulator index, nearest `from` first. Only the ones we have
    /// seen: BARb puts a turret beside nearly every extractor, and an unseen one is priced the moment it is seen.
    pub(super) fn turrets_bearing(&self, from: Vec3, at: Vec3, radius: f32) -> Vec<(UnitId, usize, Vec3)> {
        let rules = &self.contacts.rules;
        let mut turrets: Vec<(UnitId, usize, Vec3)> = self
            .enemy_buildings
            .iter()
            .filter_map(|(id, (def, pos, _))| {
                let d = self.world.def(*def)?;
                if d.weapon_count == 0 {
                    return None;
                }
                let index = *self.contacts.sim_defs.get(def)?;
                let reach = rules.units.list[index].reach();
                (pos.dist2d(at) < radius || to_segment(*pos, from, at) < reach + TURRET_MARGIN).then_some((*id, index, *pos))
            })
            .collect();
        turrets.sort_by(|a, b| a.2.dist2d(from).total_cmp(&b.2.dist2d(from)));
        turrets
    }

    /// Per contact answer: its members and the turrets of theirs it was priced against (the control lane's
    /// commitments, `micro.rs`).
    pub(super) fn response_commitments(&self) -> Vec<(Vec<UnitId>, Vec<UnitId>)> {
        self.contacts.responses.iter().map(|r| (r.members.clone(), r.turrets.clone())).collect()
    }

    /// What our `party` raiding `target` (a building of theirs) comes to: the chase the other way round
    /// (`docs/design/2026-09-20-base-raid-pricing.md`). Their soldiers in sight within `CONTACT_RADIUS` of the
    /// target pursue from where they stand, their turrets bearing on the target or the approach hold, every unarmed
    /// building of theirs remembered within `RAID_ASSETS` of the target is an asset, and our party fights: it kills
    /// what shoots and then burns the rest, which is what Matt's twelve did (the simulator's `Fight` with assets
    /// present: six Pawns for two towers and everything behind them, as he paid seven). Their commander is left out
    /// of the scene: a party that fights it loses whatever else stands there, and the control lane keeps the
    /// party out of its reach until the kill gate opens. Gain: their metal burned and killed, less the safety
    /// times ours lost.
    pub(super) fn raid_verdict(&mut self, party: &[&OwnUnit], target: Vec3, tick: &Tick) -> Assault {
        if self.contacts.sim_defs.is_empty() {
            self.survey_sim_defs();
        }
        let n = party.len().max(1) as f32;
        let from = party.iter().fold(Vec3::default(), |sum, u| Vec3 { x: sum.x + u.pos.x / n, y: 0.0, z: sum.z + u.pos.z / n });
        let bearing = self.turrets_bearing(from, target, RAID_ASSETS);
        let mut turrets: Vec<(usize, Vec2)> = bearing.iter().map(|(_, index, pos)| (*index, flat(*pos))).collect();
        let known: Vec<(UnitDefId, Vec3)> = self.enemy_buildings.values()
            .filter(|(def, pos, _)| pos.dist2d(target) < RAID_ASSETS && self.world.def(*def).is_some_and(|d| d.weapon_count == 0))
            .map(|(def, pos, _)| (*def, *pos))
            .collect();
        let mut assets: Vec<(usize, Vec2)> = known.iter().filter_map(|(def, pos)| Some((*self.contacts.sim_defs.get(def)?, flat(*pos)))).collect();
        // At their base, what we have not seen of it yet, priced as it usually stands.
        let base = self.enemy_base(target);
        if base.dist2d(target) < super::army::BASE_RADIUS && let Some(kit) = self.kit {
            let their = |ours: UnitDefId| {
                // Their faction's counterpart of our unit by name suffix, else ours (the simulator prices by type).
                let suffix = self.world.def(ours).map(|d| d.name[3..].to_string()).unwrap_or_default();
                self.enemy_buildings.values().map(|(d, _, _)| *d).chain(self.enemy_soldiers.values().map(|(d, _)| *d))
                    .find(|d| self.world.def(*d).is_some_and(|x| x.name.ends_with(&suffix)))
                    .unwrap_or(ours)
            };
            let counted = |ours: UnitDefId| known.iter().filter(|(d, _)| self.world.def(*d).is_some_and(|x| self.world.def(ours).is_some_and(|o| x.name[3..] == o.name[3..]))).count();
            let place = |def: UnitDefId, n: usize, list: &mut Vec<(usize, Vec2)>| {
                let Some(index) = self.contacts.sim_defs.get(&their(def)).copied() else { return };
                for i in 0..n {
                    let angle = i as f32 * 2.4;
                    list.push((index, Vec2::new(base.x + angle.cos() * 120.0, base.z + angle.sin() * 120.0)));
                }
            };
            place(kit.lab, 1usize.saturating_sub(counted(kit.lab)), &mut assets);
            place(kit.extractor, EXPECTED_BASE_EXTRACTORS.saturating_sub(counted(kit.extractor)), &mut assets);
            place(kit.wind, EXPECTED_BASE_WINDS.saturating_sub(counted(kit.wind)), &mut assets);
            // BARb puts a tower beside nearly every extractor (K-barb-medium-observed-build): the first unseen tower
            // stands by the target when the target is an extractor, the rest at the base.
            let known_towers = bearing.len();
            let mut unseen = expected_towers(tick.frame).saturating_sub(known_towers);
            let target_is_extractor = known.iter().any(|(d, p)| p.dist2d(target) < 50.0 && self.world.def(*d).is_some_and(|x| x.extracts_metal > 0.0));
            if unseen > 0 && target_is_extractor && let Some(index) = self.contacts.sim_defs.get(&their(kit.turret)).copied() {
                turrets.push((index, Vec2::new(target.x + 60.0, target.z + 60.0)));
                unseen -= 1;
            }
            place(kit.turret, unseen, &mut turrets);
        }
        assets.truncate(RAID_MAX_ASSETS);
        // Their soldiers (the commander among them) as pursuers from where they stand, grouped as ours are.
        let mut seen: HashMap<UnitDefId, usize> = HashMap::new();
        self.enemy_soldiers.values().for_each(|(def, _)| *seen.entry(*def).or_default() += 1);
        let blip = seen.into_iter().max_by_key(|(def, n)| (*n, def.0)).map(|(def, _)| def).or(self.kit.as_ref().map(|k| k.line));
        let mut groups: HashMap<(usize, i32, i32), (u32, f32, f32)> = HashMap::new();
        for enemy in tick.snapshot.enemies.iter().filter(|e| e.pos.dist2d(target) < CONTACT_RADIUS) {
            let Some(def) = enemy.def.or(blip) else { continue };
            let Some(d) = self.world.def(def) else { continue };
            if !(d.speed > 0.0 && d.weapon_count > 0 && d.move_class.is_some()) || (d.name.ends_with("com") && d.build_speed > 0.0) {
                continue;
            }
            let Some(index) = self.contacts.sim_defs.get(&def) else { continue };
            let group = groups.entry((*index, (enemy.pos.x / 300.0) as i32, (enemy.pos.z / 300.0) as i32)).or_default();
            *group = (group.0 + 1, group.1 + enemy.pos.x, group.2 + enemy.pos.z);
        }
        let mut pursuers: Vec<(usize, u32, Vec2)> = groups.into_iter().map(|((def, _, _), (k, x, z))| (def, k, Vec2::new(x / k as f32, z / k as f32))).collect();
        pursuers.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)).then(a.2.x.total_cmp(&b.2.x)));
        let mut ours: HashMap<usize, u32> = HashMap::new();
        for unit in party {
            if let Some(index) = self.contacts.sim_defs.get(&unit.def) {
                *ours.entry(*index).or_default() += 1;
            }
        }
        let mut ours: Vec<(usize, u32)> = ours.into_iter().collect();
        ours.sort();
        let chase = Chase { pursuers, party: ours, at: flat(from), intent: Intent::Fight, assets, party_buildings: Vec::new(), pursuer_buildings: turrets, seconds: SECONDS };
        let rules = self.contacts.rules.clone();
        let started = std::time::Instant::now();
        let verdict = chase.verdict(&rules, REPS);
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let spent = &mut self.contacts.spent;
        *spent = (spent.0 + 1, spent.1 + ms, spent.2.max(ms));
        let gain = verdict.assets_lost + verdict.pursuers_lost - RAID_SAFETY * verdict.party_killed;
        Assault { gain, turrets: bearing.into_iter().map(|(id, _, _)| id).collect(), verdict }
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
                None => Response { party: HashSet::new(), at: party.at, members: Vec::new(), priced_at: i32::MIN / 2, misses: 0, last_seen: tick.frame, ordered_to: Vec3::default(), turrets: Vec::new(), turrets_known: 0 },
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
        // Due: four seconds since the last price, or a turret of theirs newly seen bearing on the party.
        let turrets_known: Vec<usize> = parties.iter().map(|p| self.turrets_bearing(p.at, p.at, 0.0).len()).collect();
        let mut due: Vec<usize> = (0..parties.len()).filter(|i| tick.frame - responses[*i].priced_at >= REPRICE_FRAMES || turrets_known[*i] != responses[*i].turrets_known).collect();
        due.sort_by(|a, b| stake(*b).total_cmp(&stake(*a)));
        for index in due.into_iter().take(PRICED_PER_TICK) {
            let party = &parties[index];
            let taken: HashSet<UnitId> = responses.iter().enumerate().filter(|(i, _)| *i != index).flat_map(|(_, r)| r.members.iter().copied()).collect();
            // Arrival by travel time over the ground (routing design): the party is usually at an extractor, so
            // the spot's field prices the walk; the straight line otherwise.
            let eta = |u: &OwnUnit| self.seconds_to_site(u.def, u.pos, party.at);
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

            // Their turrets whose range covers the party: an answer under them is priced under them, and kills them first.
            let bearing = self.turrets_bearing(party.at, party.at, 0.0);
            let their_turrets: Vec<(usize, Vec2)> = bearing.iter().map(|(_, index, pos)| (*index, flat(*pos))).collect();
            let nobody = self.chase_verdict(party, &assets, &[], &their_turrets);
            let gain = |v: &Verdict| (v.party_killed - nobody.party_killed) - LOSS_SAFETY * (v.pursuers_lost - nobody.pursuers_lost) - ASSET_WORTH * (v.assets_lost - nobody.assets_lost);
            // Whoever is on it stays on it: only more, or everybody off.
            let least = members.len().max(1);
            let mut sizes: Vec<usize> = SIZES.iter().copied().chain([least, pool.len().min(SIZES[SIZES.len() - 1])]).filter(|k| *k >= least && *k <= pool.len()).collect();
            sizes.sort();
            sizes.dedup();
            let mut best: Option<(usize, f32, Verdict)> = None;
            for size in sizes {
                let verdict = self.chase_verdict(party, &assets, &pool[..size], &their_turrets);
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
            (response.turrets, response.turrets_known) = (bearing.iter().map(|(id, _, _)| *id).collect(), turrets_known[index]);
        }

        for response in &mut responses {
            let moved = response.ordered_to.dist2d(response.at) > REORDER_DISTANCE;
            let orders = free.iter().filter(|u| response.members.contains(&u.id) && (moved || u.idle));
            // A turret over the party is killed first, deliberately, then the party.
            let turret = response.turrets.iter().find(|id| self.enemy_buildings.contains_key(id)).copied();
            for u in orders {
                if let Some(turret) = turret {
                    commands.push(Command::Attack { unit: u.id, target: turret, queue: false });
                    commands.push(Command::Fight { unit: u.id, to: response.at, queue: true });
                } else {
                    commands.push(Command::Fight { unit: u.id, to: response.at, queue: false });
                }
            }
            if moved {
                response.ordered_to = response.at;
            }
        }
        if tick.due() % (60 * FRAMES_PER_SECOND) == 0 && self.contacts.spent.0 > 0 {
            let (questions, total, longest) = std::mem::take(&mut self.contacts.spent);
            eprintln!("[ai {}] f={} contact simulator this minute: {questions} questions, {total:.0} ms, longest {longest:.1} ms", self.ai(), tick.frame);
        }
        let answering = responses.iter().flat_map(|r| r.members.iter().copied()).collect();
        self.contacts.responses = responses;
        answering
    }
}
