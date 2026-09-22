//! The picture: the game as one JSON state for Jev, written by actor and qualitative first (Jev is documented
//! weak at arithmetic and at comparing quantities, and distracted by unrelated state), with the player's
//! instructions at the top. Places and enemy parties are named here so that the menus' answers can name them back.

use std::collections::BTreeMap;

use bot_protocol::{EnemyUnit, Event, OwnUnit, Tick, UnitDefId, Vec3};
use serde_json::{Value, json};

use super::super::roster::Kit;
use super::super::territory::Ground;
use super::super::{Brain, FRAMES_PER_SECOND};
use super::{GroupTask, Task};

/// Free spots the picture lists, nearest home on foot first.
const FREE_SPOTS: usize = 10;
/// Enemy extractors known, nearest first.
const ENEMY_SPOTS: usize = 6;
const PASSAGES: usize = 3;
/// Enemies this close together are one party.
const PARTY_RADIUS: f32 = 400.0;
/// A party this near a place or an actor is "near" it.
const NEAR: f32 = 800.0;
/// The enemy's soldiers seen within this long count in what we know of its army.
const ARMY_MEMORY: i32 = 3 * 60 * FRAMES_PER_SECOND;

#[derive(Clone, Debug)]
pub(crate) struct Place {
    pub name: String,
    pub at: Vec3,
    pub spot: Option<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct Party {
    pub name: String,
    pub ids: Vec<bot_protocol::UnitId>,
    pub at: Vec3,
    pub metal: f32,
    pub composition: String,
}

pub(crate) struct Picture {
    pub state: Value,
    pub places: Vec<Place>,
    pub parties: Vec<Party>,
}

pub(crate) fn clock(frame: i32) -> String {
    let seconds = frame / FRAMES_PER_SECOND;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn stock_words(current: f32, storage: f32) -> &'static str {
    let share = if storage > 0.0 { current / storage } else { 0.0 };
    if share < 0.1 {
        "nearly empty"
    } else if share < 0.3 {
        "low"
    } else if share < 0.7 {
        "some in store"
    } else if share < 0.95 {
        "plenty in store"
    } else {
        "full: what comes in beyond this is wasted"
    }
}

fn flow_words(income: f32, usage: f32, current: f32, storage: f32) -> &'static str {
    let share = if storage > 0.0 { current / storage } else { 0.0 };
    // The engine caps spending at income once the store is empty, so a stall reads as "in balance" by the numbers.
    if share < 0.05 && usage >= income * 0.9 {
        "STALLING: the store is empty and everything that needs it builds slowly; more income is needed"
    } else if usage > income * 1.15 && share < 0.25 {
        "spending faster than it comes in: stalling, everything builds slowly"
    } else if usage > income * 1.15 {
        "spending faster than it comes in, running the store down"
    } else if income > usage * 1.3 && share > 0.6 {
        "more coming in than is spent: banking it unused"
    } else {
        "in balance"
    }
}

fn health_words(share: f32) -> &'static str {
    if share > 0.95 {
        "full"
    } else if share > 0.7 {
        "lightly hurt"
    } else if share > 0.4 {
        "hurt"
    } else {
        "badly hurt, near death"
    }
}

/// How many constructors we have, against the extractors they serve.
pub(crate) fn constructor_words(constructors: usize, extractors: usize) -> &'static str {
    if constructors == 0 {
        "none"
    } else if constructors <= 2 {
        "a couple, few"
    } else if constructors <= (extractors / 2 + 2).max(4) {
        "enough for the spots we hold"
    } else if constructors <= extractors + 2 {
        "many, more than the spots need"
    } else {
        "far too many: more constructors than extractors, each one metal that is not a soldier"
    }
}

pub(crate) fn soldier_words(soldiers: usize, metal: f32) -> &'static str {
    if soldiers == 0 {
        "none: we have no army at all"
    } else if soldiers < 4 {
        "a handful"
    } else if metal < 1500.0 {
        "a group"
    } else {
        "a real army"
    }
}

pub(crate) fn distance_words(elmos: f32) -> &'static str {
    if elmos < 500.0 {
        "right here"
    } else if elmos < 1200.0 {
        "near"
    } else if elmos < 2500.0 {
        "some way off"
    } else {
        "far"
    }
}

impl Brain {
    /// A unit type in a few words, for the menus and the picture.
    pub(super) fn unit_words(&self, def: UnitDefId, kit: &Kit) -> String {
        let name = self.name(def);
        let metal = self.world.def(def).map_or(0.0, |d| d.metal_cost);
        let role = if def == kit.commander {
            "our commander: the strongest builder, a good fighter; the game is lost if it dies"
        } else if def == kit.constructor {
            "constructor: builds anything tier 1, repairs, reclaims; expands the economy"
        } else if def == kit.raider {
            "raider: fast and light, kills builders, extractors and lone turrets, loses to line units"
        } else if def == kit.line {
            "line unit: wins most tier-1 fights at equal metal, slow"
        } else if def == kit.skirmisher {
            "rocket skirmisher: outranges turrets, loses to anything that closes on it"
        } else if def == kit.second {
            "brawler: tough close fighter"
        } else if def == kit.artillery {
            "artillery: long range, needs something in front of it"
        } else if def == kit.resurrector {
            "resurrection bot: raises and reclaims wrecks, cannot fight"
        } else if def == kit.advanced_constructor {
            "advanced constructor: upgrades extractors, builds tier 2"
        } else if def == kit.extractor {
            "metal extractor"
        } else if def == kit.solar {
            "solar collector: 20 energy a second"
        } else if def == kit.wind {
            "wind generator: energy with the wind"
        } else if def == kit.lab {
            "bot lab: the factory"
        } else if def == kit.turret {
            "light laser turret"
        } else if def == kit.radar {
            "radar tower: sees 2000 around it"
        } else if def == kit.nano {
            "construction turret: adds build power to the lab beside it"
        } else if def == kit.converter {
            "energy-to-metal converter"
        } else if def == kit.advanced_lab {
            "advanced bot lab: tier 2"
        } else {
            "unit"
        };
        format!("{name} ({role}; {metal:.0} metal)")
    }

    fn composition_words(&self, units: &[&OwnUnit]) -> String {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for unit in units {
            *counts.entry(self.name(unit.def)).or_default() += 1;
        }
        counts.iter().map(|(name, n)| format!("{n} {name}")).collect::<Vec<_>>().join(", ")
    }

    /// A type's role in a word or two, for the picture (the internal name means nothing to a reader).
    pub(super) fn short_words(&self, def: UnitDefId, kit: &Kit) -> String {
        let word = if def == kit.commander {
            "commander"
        } else if def == kit.constructor {
            "constructor"
        } else if def == kit.extractor {
            "extractor"
        } else if def == kit.advanced_extractor {
            "advanced extractor"
        } else if def == kit.solar || def == kit.advanced_solar {
            "solar collector"
        } else if def == kit.wind {
            "wind generator"
        } else if def == kit.lab {
            "lab"
        } else if def == kit.advanced_lab {
            "advanced lab"
        } else if def == kit.turret {
            "light turret"
        } else if def == kit.radar {
            "radar"
        } else if def == kit.nano {
            "construction turret"
        } else if def == kit.converter {
            "converter"
        } else if def == kit.raider {
            "raider"
        } else if def == kit.line {
            "line unit"
        } else if def == kit.skirmisher {
            "skirmisher"
        } else if def == kit.second {
            "brawler"
        } else if def == kit.resurrector {
            "resurrection bot"
        } else {
            return self.name(def).to_string();
        };
        format!("{} ({word})", self.name(def))
    }

    /// The player's production whitelist for a lab (H-HANDS-PRODUCE): the lab's own, else `all`, else none.
    pub(super) fn allowed_units(&self, lab: &str) -> Option<Vec<String>> {
        let shared = self.strategist.as_ref()?;
        let allowed = shared.allowed.lock().unwrap();
        allowed.get(lab).or_else(|| allowed.get("all")).cloned()
    }

    /// The named place nearest `pos`, with its grid cell, or the grid cell alone.
    pub(super) fn place_words(&self, places: &[Place], pos: Vec3) -> String {
        let grid = self.world.grid(pos);
        match places.iter().map(|p| (p.at.dist2d(pos), p)).min_by(|a, b| a.0.total_cmp(&b.0)) {
            Some((d, place)) if d < 300.0 => format!("{} ({grid})", place.name),
            Some((d, place)) if d < 900.0 => format!("{:.0} from {} ({grid})", d, place.name),
            _ => format!("({grid})"),
        }
    }

    /// Enemy mobile units in sight, grouped into parties, nearest home first.
    pub(super) fn enemy_parties(&self, enemies: &[EnemyUnit]) -> Vec<Party> {
        let mobile: Vec<&EnemyUnit> = enemies.iter().filter(|e| e.def.is_none_or(|d| self.world.def(d).is_some_and(|d| d.speed > 0.0))).collect();
        let mut party_of: Vec<Option<usize>> = vec![None; mobile.len()];
        let mut parties: Vec<Party> = Vec::new();
        for start in 0..mobile.len() {
            if party_of[start].is_some() {
                continue;
            }
            party_of[start] = Some(parties.len());
            let mut members = vec![start];
            let mut next = 0;
            while next < members.len() {
                let from = mobile[members[next]].pos;
                for other in 0..mobile.len() {
                    if party_of[other].is_none() && mobile[other].pos.dist2d(from) < PARTY_RADIUS {
                        party_of[other] = Some(parties.len());
                        members.push(other);
                    }
                }
                next += 1;
            }
            let n = members.len() as f32;
            let at = members.iter().fold(Vec3::default(), |sum, i| Vec3 { x: sum.x + mobile[*i].pos.x / n, y: 0.0, z: sum.z + mobile[*i].pos.z / n });
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            let mut metal = 0.0;
            for i in &members {
                let name = mobile[*i].def.map_or("unidentified", |d| self.name(d));
                *counts.entry(name).or_default() += 1;
                metal += mobile[*i].def.and_then(|d| self.world.def(d)).map_or(110.0, |d| d.metal_cost);
            }
            let composition = counts.iter().map(|(name, n)| format!("{n} {name}")).collect::<Vec<_>>().join(", ");
            parties.push(Party { name: String::new(), ids: members.iter().map(|i| mobile[*i].id).collect(), at, metal, composition });
        }
        parties.sort_by(|a, b| a.at.dist2d(self.home).total_cmp(&b.at.dist2d(self.home)));
        for (i, party) in parties.iter_mut().enumerate() {
            party.name = format!("party_{}", i + 1);
        }
        parties
    }

    /// The fight simulator's odds of `units` against a party, in words (Jev compares nothing itself).
    pub(super) fn odds_words(&self, units: &[&OwnUnit], party: &Party, enemies: &[EnemyUnit]) -> &'static str {
        let mut theirs = super::super::combat::Force::default();
        for enemy in enemies.iter().filter(|e| party.ids.contains(&e.id)) {
            match enemy.def {
                Some(def) => theirs.add(def),
                None => theirs.unidentified += 1,
            }
        }
        let ours = Brain::force_of(units);
        let ratio = self.odds(&ours, &theirs);
        if units.is_empty() {
            "we have nobody to send"
        } else if ratio >= 2.5 {
            "we outweigh it heavily"
        } else if ratio >= 1.3 {
            "we outweigh it"
        } else if ratio >= 0.8 {
            "an even fight"
        } else {
            "it outweighs us"
        }
    }

    /// Enemy parties at our extractors, for a group's entry: which spot, how far from the group.
    pub(super) fn threats_words(&self, parties: &[Party], places: &[Place], own: &[OwnUnit], kit: &Kit, from: Vec3) -> Vec<String> {
        parties
            .iter()
            .filter_map(|p| {
                let extractor = own.iter().filter(|u| kit.is_extractor(u.def) && u.pos.dist2d(p.at) < NEAR).min_by(|a, b| a.pos.dist2d(p.at).total_cmp(&b.pos.dist2d(p.at)))?;
                Some(format!("{} ({}) at our extractor at {}, {} ({:.0}) from this group", p.name, p.composition, self.place_words(places, extractor.pos), distance_words(p.at.dist2d(from)), p.at.dist2d(from)))
            })
            .collect()
    }

    fn party_words(&self, parties: &[Party], pos: Vec3) -> Option<String> {
        parties
            .iter()
            .map(|p| (p.at.dist2d(pos), p))
            .filter(|(d, _)| *d < NEAR)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(d, p)| format!("{} ({}) {:.0} away", p.name, p.composition, d))
    }

    fn task_words(&self, task: Option<&Task>, unit: &OwnUnit, places: &[Place], frame: i32, kit: &Kit) -> String {
        let ago = |since: i32| format!("{} s ago", (frame - since) / FRAMES_PER_SECOND);
        match task {
            None if unit.idle => "idle, waiting for an order".into(),
            None => "finishing an order".into(),
            Some(Task::Build { def, near, started, ordered, .. }) => {
                let what = self.short_words(*def, kit);
                let walk = unit.pos.dist2d(*near);
                if *started {
                    format!("building a {what} at {}, ordered {}", self.place_words(places, *near), ago(*ordered))
                } else if walk > 120.0 {
                    format!("walking to build a {what} at {}, {walk:.0} to go, ordered {}", self.place_words(places, *near), ago(*ordered))
                } else {
                    format!("about to build a {what} at {}, ordered {}", self.place_words(places, *near), ago(*ordered))
                }
            }
            Some(Task::Assist { lab, since }) => format!("helping lab_{} build, since {}", lab.0, ago(*since)),
            Some(Task::Reclaim { at, since }) => format!("taking apart wrecks at {}, since {}", self.place_words(places, *at), ago(*since)),
            Some(Task::Repair { target, since }) => format!("repairing our {}, since {}", self.known_units.get(target).map_or("unit", |(def, _)| self.name(*def)), ago(*since)),
            Some(Task::Walk { place, to, since }) => format!("walking to {place}, {:.0} to go, since {}", unit.pos.dist2d(*to), ago(*since)),
        }
    }

    /// The whole picture for this second.
    pub(super) fn picture(&self, tick: &Tick, kit: &Kit) -> Picture {
        let pianist = self.pianist.as_ref().expect("pianist mode");
        let snapshot = &tick.snapshot;
        let own = &snapshot.own_units;
        let frame = tick.frame;
        let spots = &self.world.hello.metal_spots;

        // Places.
        let mut places: Vec<Place> = vec![Place { name: "home".into(), at: self.home, spot: None }];
        let enemy_base = self.enemy_base(self.home);
        places.push(Place { name: "enemy_base".into(), at: enemy_base, spot: None });
        let extractor_at = |spot: Vec3| own.iter().find(|u| kit.is_extractor(u.def) && u.pos.dist2d(spot) < 100.0);
        let taken_by_task: Vec<usize> = pianist.tasks.values().filter_map(|t| if let Task::Build { spot: Some(i), .. } = t { Some(*i) } else { None }).collect();
        let mut listed: Vec<usize> = Vec::new();
        for (i, spot) in spots.iter().enumerate() {
            if extractor_at(*spot).is_some() || taken_by_task.contains(&i) {
                listed.push(i);
            }
        }
        let enemy_extractors: Vec<(Vec3, i32)> = self
            .enemy_buildings
            .values()
            .filter(|(def, _, _)| self.world.def(*def).is_some_and(|d| d.extracts_metal > 0.0))
            .map(|(_, pos, seen)| (*pos, *seen))
            .collect();
        let their_spot = |spot: Vec3| enemy_extractors.iter().find(|(pos, _)| pos.dist2d(spot) < 100.0).map(|(_, seen)| *seen);
        let mut free: Vec<(f32, usize)> = spots
            .iter()
            .enumerate()
            .filter(|(i, s)| !listed.contains(i) && their_spot(**s).is_none() && self.reachable_on_foot(**s) && self.spot_open_to_us(*i, **s, frame))
            .map(|(i, s)| (self.walk_from_home(*s), i))
            .collect();
        free.sort_by(|a, b| a.0.total_cmp(&b.0));
        listed.extend(free.iter().take(FREE_SPOTS).map(|(_, i)| *i));
        let mut theirs: Vec<(f32, usize)> = spots.iter().enumerate().filter(|(_, s)| their_spot(**s).is_some()).map(|(i, s)| (s.dist2d(self.home), i)).collect();
        theirs.sort_by(|a, b| a.0.total_cmp(&b.0));
        listed.extend(theirs.iter().take(ENEMY_SPOTS).map(|(_, i)| *i));
        for i in &listed {
            places.push(Place { name: format!("spot_{i}"), at: spots[*i], spot: Some(*i) });
        }
        let passages = self.passages();
        for (n, passage) in passages.iter().take(PASSAGES).enumerate() {
            places.push(Place { name: format!("passage_{}", n + 1), at: passage.at, spot: None });
        }
        // H-HANDS-NAMED-PLACES: every spot and passage the instructions name is a place, however far (pianist-player-5:
        // the player named spot_36 in the south for four turns and it was never on the menu, the list being the
        // nearest free spots and the nearest of theirs), and so is every place the player marked.
        let instructions = self
            .strategist
            .as_ref()
            .map(|s| s.instructions.lock().unwrap().clone())
            .filter(|i| !i.trim().is_empty())
            .unwrap_or_else(|| crate::texts::read(&crate::texts::HANDS_DEFAULT));
        for token in instructions.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
            if let Some(i) = token.strip_prefix("spot_").and_then(|n| n.parse::<usize>().ok()) {
                // An islet spot nobody can walk to is no place to send anyone (pianist-player-7: the ball stood 151 s
                // short of spot_31 on the shore while the player named it).
                if i < spots.len() && !places.iter().any(|p| p.spot == Some(i)) && self.reachable_on_foot(spots[i]) {
                    places.push(Place { name: format!("spot_{i}"), at: spots[i], spot: Some(i) });
                }
            } else if let Some(n) = token.strip_prefix("passage_").and_then(|n| n.parse::<usize>().ok()) {
                if n > PASSAGES && n <= passages.len() && !places.iter().any(|p| p.name == *token) {
                    places.push(Place { name: format!("passage_{n}"), at: passages[n - 1].at, spot: None });
                }
            }
        }
        let marks: BTreeMap<String, (f32, f32)> = self.strategist.as_ref().map(|s| s.marks.lock().unwrap().clone()).unwrap_or_default();
        for (name, (x, z)) in &marks {
            if !places.iter().any(|p| p.name == *name) {
                places.push(Place { name: name.clone(), at: Vec3 { x: *x, y: 0.0, z: *z }, spot: None });
            }
        }
        // H-HANDS-SHELLED: where fire from out of sight likeliest comes from, while it lasts.
        let shelling = self.shelling();
        if let Some(s) = &shelling {
            places.push(Place { name: "shelling".into(), at: s.at, spot: None });
        }
        let parties = self.enemy_parties(&snapshot.enemies);

        let mut place_entries: BTreeMap<String, Value> = BTreeMap::new();
        for place in &places {
            let mut entry = json!({ "grid": self.world.grid(place.at) });
            let walk = self.walk_from_home(place.at);
            entry["from_home"] = json!(format!("{} ({walk:.0} on foot)", distance_words(walk)));
            entry["ground"] = json!(match self.ground(place.at) {
                Ground::Held => "held by us",
                Ground::Contested => "contested",
                Ground::Theirs => "theirs",
            });
            let what = match place.spot {
                Some(i) => {
                    let spot = spots[i];
                    // The extractor on the spot itself (within its radius) is what takes it; the buildings beside it
                    // are said separately (the user, 2026-09-22: a starting spot never taken, every game: the
                    // neighbouring spot's extractor 300 away read as "our extractor" here, and the spot was never free).
                    let on_spot = own.iter().find(|u| kit.is_extractor(u.def) && u.pos.dist2d(spot) < self.world.hello.map.extractor_radius + 10.0);
                    let beside: Vec<String> = own
                        .iter()
                        .filter(|u| !kit.is_extractor(u.def) && u.pos.dist2d(spot) < 200.0 && self.world.def(u.def).is_some_and(|d| d.speed == 0.0))
                        .map(|u| format!("our {}{}", self.short_words(u.def, kit), if u.being_built { " (being built)" } else { "" }))
                        .collect();
                    let beside_words = if beside.is_empty() { String::new() } else { format!(" (beside it: {})", beside.join(", ")) };
                    if let Some(seen) = their_spot(spot) {
                        let turrets = self.enemy_buildings.values().filter(|(def, pos, _)| pos.dist2d(spot) < 500.0 && self.world.def(*def).is_some_and(|d| d.weapon_count > 0)).count();
                        format!("their extractor, seen {} ago{}", clock(frame - seen), if turrets > 0 { format!(", {turrets} turret(s) beside it") } else { String::new() })
                    } else if let Some(u) = on_spot {
                        format!("our extractor{}{beside_words}", if u.being_built { " (being built)" } else { "" })
                    } else {
                        let taker = pianist.tasks.iter().find_map(|(id, t)| matches!(t, Task::Build { spot: Some(s), .. } if *s == i).then_some(*id));
                        match taker {
                            Some(id) => format!("free metal spot, {} is on its way to take it{beside_words}", self.actor_name(id, kit)),
                            None => format!("free metal spot{beside_words}"),
                        }
                    }
                }
                None if place.name == "home" => "our start: the lab and the base stand here".into(),
                None if place.name == "enemy_base" => match self.found_enemy_base() {
                    Some(_) => {
                        let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
                        for (def, pos, _) in self.enemy_buildings.values() {
                            if pos.dist2d(enemy_base) < 1500.0 {
                                *kinds.entry(self.name(*def)).or_default() += 1;
                            }
                        }
                        format!("the enemy base, found: {}", kinds.iter().map(|(n, k)| format!("{k} {n}")).collect::<Vec<_>>().join(", "))
                    }
                    None => "where the enemy is presumed to start; not yet seen".into(),
                },
                None if place.name == "shelling" => {
                    let s = shelling.as_ref().expect("a shelling place has a shelling");
                    format!("where the {} shelling us from out of our sight likeliest stands: its range is {:.0}, {} hits on us in the last 20 s from the {}; advancing a group onto it (fight_to) kills it, a group that stays where it is keeps being hit", self.weapon_words(&s.weapon), s.range, s.hits, super::super::shelling::compass(s.dir))
                }
                None if marks.contains_key(&place.name) => {
                    let walkable = self.snap_to_reachable(place.at);
                    let off = walkable.dist2d(place.at);
                    if off > 150.0 { format!("a place the player marked; our bots cannot walk onto it, the nearest ground they can reach is {off:.0} away (units sent here stop there)") } else { "a place the player marked".into() }
                }
                None => "a narrow passage between the two sides".into(),
            };
            entry["what"] = json!(what);
            if let Some(words) = self.party_words(&parties, place.at) {
                entry["enemies_near"] = json!(words);
            }
            if let Some(i) = place.spot
                && let Some(lost) = self.spot_losses.get(&i)
            {
                entry["raided"] = json!(format!("we lost an extractor here {lost} time(s)"));
            }
            place_entries.insert(place.name.clone(), entry);
        }

        // Economy and what we have.
        let m = &snapshot.metal;
        let e = &snapshot.energy;
        let economy = json!({
            "metal": format!("{:.0} of {:.0} stored ({}); {:.1} a second coming in, {:.1} going out: {}", m.current, m.storage, stock_words(m.current, m.storage), m.income, m.usage, flow_words(m.income, m.usage, m.current, m.storage)),
            "energy": format!("{:.0} of {:.0} stored ({}); {:.0} a second coming in, {:.0} going out: {}", e.current, e.storage, stock_words(e.current, e.storage), e.income, e.usage, flow_words(e.income, e.usage, e.current, e.storage)),
        });
        let count = |f: &dyn Fn(&OwnUnit) -> bool| own.iter().filter(|u| !u.being_built && f(u)).count();
        let soldiers: Vec<&OwnUnit> = own.iter().filter(|u| !u.being_built && self.is_army(u, kit)).collect();
        let army_metal: f32 = soldiers.iter().filter_map(|u| self.world.def(u.def)).map(|d| d.metal_cost).sum();
        let wreck_fields: Vec<String> = self
            .reclaim
            .fields
            .iter()
            .filter(|f| f.metal >= 100.0)
            .take(3)
            .map(|f| format!("{:.0} metal of wrecks at {}{}", f.metal, self.place_words(&places, f.at), if f.safe { "" } else { " (not safe)" }))
            .collect();
        let extractors = count(&|u| kit.is_extractor(u.def));
        let constructors = count(&|u| u.def == kit.constructor || u.def == kit.advanced_constructor);
        let ours = json!({
            "extractors": extractors,
            "labs": count(&|u| u.def == kit.lab || u.def == kit.advanced_lab),
            "constructors": format!("{constructors}: {}", constructor_words(constructors, extractors)),
            "turrets": count(&|u| u.def == kit.turret),
            "radars": count(&|u| u.def == kit.radar),
            "generators": count(&|u| u.def == kit.solar || u.def == kit.wind || u.def == kit.advanced_solar),
            "soldiers": format!("{}: {} worth {:.0} metal ({})", soldier_words(soldiers.len(), army_metal), soldiers.len(), army_metal.max(0.0), self.composition_words(&soldiers)),
            "wrecks": wreck_fields,
        });

        // The enemy.
        let in_sight: Vec<String> = parties
            .iter()
            .map(|p| {
                let heading = {
                    let vel: Vec3 = snapshot.enemies.iter().filter(|e| p.ids.contains(&e.id)).fold(Vec3::default(), |s, e| Vec3 { x: s.x + e.vel.x, y: 0.0, z: s.z + e.vel.z });
                    let speed = vel.x.hypot(vel.z);
                    if speed < 0.3 {
                        "standing still"
                    } else {
                        let toward_home = (self.home.x - p.at.x) * vel.x + (self.home.z - p.at.z) * vel.z > 0.0;
                        if toward_home { "moving toward our side" } else { "moving away from us" }
                    }
                };
                let near_ours = own
                    .iter()
                    .filter(|u| self.world.def(u.def).is_some_and(|d| d.speed == 0.0) || u.def == kit.commander || u.def == kit.constructor)
                    .map(|u| (u.pos.dist2d(p.at), u))
                    .filter(|(d, _)| *d < NEAR)
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(d, u)| format!("; {d:.0} from our {}", self.name(u.def)));
                format!("{}: {} worth {:.0} metal at {}, {} from home, {heading}{}", p.name, p.composition, p.metal, self.place_words(&places, p.at), distance_words(p.at.dist2d(self.home)), near_ours.unwrap_or_default())
            })
            .collect();
        let known_soldiers: Vec<&(UnitDefId, i32)> = self.enemy_soldiers.values().filter(|(_, seen)| frame - seen < ARMY_MEMORY).collect();
        let known_metal: f32 = known_soldiers.iter().filter_map(|(def, _)| self.world.def(*def)).map(|d| d.metal_cost).sum();
        let mut remembered: BTreeMap<String, BTreeMap<&str, usize>> = BTreeMap::new();
        for (def, pos, _) in self.enemy_buildings.values() {
            *remembered.entry(self.world.grid(*pos)).or_default().entry(self.name(*def)).or_default() += 1;
        }
        let remembered: Vec<String> = remembered.iter().map(|(grid, kinds)| format!("{grid}: {}", kinds.iter().map(|(n, k)| format!("{k} {n}")).collect::<Vec<_>>().join(", "))).collect();
        let enemy = json!({
            "in_sight": in_sight,
            "base": match self.found_enemy_base() { Some(at) => format!("found at {}", self.place_words(&places, at)), None => format!("not found; presumed at {}", self.world.grid(enemy_base)) },
            "buildings_seen": remembered,
            "army_known": format!("{} soldiers worth {:.0} metal seen in the last three minutes and not seen to die; it may have much more", known_soldiers.len(), known_metal.max(0.0)),
            "commander": self.enemy_commander_seen.map_or("never seen".to_string(), |(pos, seen)| format!("seen at {} {} ago{}", self.place_words(&places, pos), clock(frame - seen), if self.reachable_on_foot(pos) { "" } else { " (in the water or on ground our bots cannot walk to: it is amphibious, our soldiers are not)" })),
        });

        // Actors.
        let damaged_by: BTreeMap<bot_protocol::UnitId, Vec<String>> = tick.events.iter().fold(BTreeMap::new(), |mut m, e| {
            if let Event::UnitDamaged { unit, attacker, .. } = e {
                let who = attacker.and_then(|id| snapshot.enemies.iter().find(|e| e.id == id)).and_then(|e| e.def).map_or("something unseen".to_string(), |d| self.name(d).to_string());
                m.entry(*unit).or_default().push(who);
            }
            m
        });
        let mut actors: BTreeMap<String, Value> = BTreeMap::new();
        for unit in own.iter().filter(|u| !u.being_built) {
            let builder = unit.def == kit.commander || unit.def == kit.constructor;
            let lab = unit.def == kit.lab || unit.def == kit.advanced_lab;
            if !builder && !lab {
                continue;
            }
            let name = self.actor_name(unit.id, kit);
            let mut entry = json!({
                "is": self.unit_words(unit.def, kit),
                "at": self.place_words(&places, unit.pos),
            });
            if builder {
                entry["health"] = json!(format!("{} ({:.0}%)", health_words(unit.health / unit.max_health), unit.health / unit.max_health * 100.0));
                entry["doing"] = json!(self.task_words(pianist.tasks.get(&unit.id), unit, &places, frame, kit));
                let from_home = unit.pos.dist2d(self.home);
                entry["from_home"] = json!(format!("{} ({from_home:.0}); ground {}", distance_words(from_home), match self.ground(unit.pos) { Ground::Held => "held by us", Ground::Contested => "contested", Ground::Theirs => "theirs" }));
                if let Some(party) = parties.iter().filter(|p| p.at.dist2d(unit.pos) < NEAR).min_by(|a, b| a.at.dist2d(unit.pos).total_cmp(&b.at.dist2d(unit.pos))) {
                    let alone: Vec<&OwnUnit> = vec![unit];
                    entry["enemies_near"] = json!(format!("{} ({}) {:.0} away: against this unit alone, {}", party.name, party.composition, party.at.dist2d(unit.pos), self.odds_words(&alone, party, &snapshot.enemies)));
                }
                if let Some(by) = damaged_by.get(&unit.id) {
                    entry["under_fire"] = json!(format!("yes, hit this second by {}", by.join(", ")));
                }
            } else {
                let queue = pianist.lab_queue.get(&unit.id).map(Vec::as_slice).unwrap_or_default();
                entry["doing"] = json!(if unit.idle && queue.is_empty() {
                    "idle: building nothing".to_string()
                } else {
                    format!("building {}", queue.iter().map(|(def, _)| self.short_words(*def, kit)).collect::<Vec<_>>().join(" then "))
                });
                entry["we_have"] = json!(format!("constructors {constructors} ({}); soldiers {} ({})", constructor_words(constructors, extractors), soldiers.len(), soldier_words(soldiers.len(), army_metal)));
                if let Some(list) = self.allowed_units(&name) {
                    let words: Vec<String> = list.iter().filter_map(|n| self.world.def_named(n)).map(|d| self.short_words(d, kit)).collect();
                    entry["allowed"] = json!(if words.is_empty() { "the player allows nothing this lab can build: it builds anything".to_string() } else { format!("the player allows only: {}", words.join(", ")) });
                }
                entry["health"] = json!(health_words(unit.health / unit.max_health));
            }
            actors.insert(name, entry);
        }
        let scouts: Vec<String> = pianist
            .groups
            .iter()
            .filter(|g| g.members.len() == 1)
            .filter_map(|g| match &g.task {
                GroupTask::Move { place, to, fight: false, .. } => {
                    let unit = g.units(own).first().copied()?;
                    Some(format!("group_{} ({}) walking to look at {place}, {:.0} to go", g.name, self.name(unit.def), unit.pos.dist2d(*to)))
                }
                _ => None,
            })
            .collect();
        for group in &pianist.groups {
            let units = group.units(own);
            let Some(centre) = super::groups::centre_of(&units) else { continue };
            let metal: f32 = units.iter().filter_map(|u| self.world.def(u.def)).map(|d| d.metal_cost).sum();
            let health: f32 = units.iter().map(|u| u.health / u.max_health).sum::<f32>() / units.len().max(1) as f32;
            let ago = |since: i32| format!("{} s", (frame - since) / FRAMES_PER_SECOND);
            let doing = match &group.task {
                GroupTask::Hold { since, committed } => format!("holding for {}{}", ago(*since), if *committed { ", fighting everything here, turrets included, since it arrived by advancing" } else { "" }),
                GroupTask::Move { to, place, fight, since } => format!("{} to {place}, {:.0} to go, for {}", if *fight { "advancing" } else { "walking" }, centre.dist2d(*to), ago(*since)),
                GroupTask::Engage { party, at, since, .. } => {
                    let name = parties.iter().find(|p| p.ids.iter().any(|id| party.contains(id))).map_or("a party now out of sight".to_string(), |p| p.name.clone());
                    format!("attacking {name} at {}, for {}", self.place_words(&places, *at), ago(*since))
                }
            };
            let mut entry = json!({
                "units": format!("{}: {} ({} soldiers worth {metal:.0} metal)", soldier_words(units.len(), metal), self.composition_words(&units), units.len()),
                "at": self.place_words(&places, centre),
                "health": format!("{} on average", health_words(health)),
                "doing": doing,
            });
            if let Some(words) = self.footwork_of(&group.name).words() {
                entry["lane"] = json!(words);
            }
            if let Some(seconds) = group.stalled_seconds(frame).filter(|s| *s >= 20) {
                entry["progress"] = json!(format!("has not got nearer its goal for {seconds} s: stalled"));
            }
            let fleeing = units.iter().filter(|u| self.lane.fleeing(u.id)).count();
            if fleeing > 0 {
                entry["footwork"] = json!(format!("{fleeing} of its {} soldiers are being held back by their own footwork this second: stepping out of a turret's reach they were not sent against, or out of a fight they would die in", units.len()));
            }
            if let Some(words) = self.party_words(&parties, centre) {
                entry["enemies_near"] = json!(words);
            }
            let threats = self.threats_words(&parties, &places, own, kit, centre);
            if !threats.is_empty() {
                entry["enemies_at_our_extractors"] = json!(threats);
            }
            if !scouts.is_empty() && group.members.len() > 1 {
                entry["scouts_out"] = json!(scouts);
            }
            let shooters: Vec<&String> = units.iter().filter_map(|u| damaged_by.get(&u.id)).flatten().collect();
            if !shooters.is_empty() {
                let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
                for who in shooters {
                    *kinds.entry(who.as_str()).or_default() += 1;
                }
                let words: Vec<String> = kinds
                    .iter()
                    .map(|(who, n)| {
                        if *who != "something unseen" {
                            return format!("{who} ({n} hits)");
                        }
                        match &shelling {
                            Some(s) => format!("something out of our sight, {n} hits: a {} with range {:.0} from the {}; its likeliest place is `shelling` at {}", self.weapon_words(&s.weapon), s.range, super::super::shelling::compass(s.dir), self.place_words(&places, s.at)),
                            None => format!("something out of our sight, {n} hits: a turret or artillery that outranges us"),
                        }
                    })
                    .collect();
                entry["under_fire"] = json!(format!("yes, this second, by {}", words.join(", ")));
            }
            actors.insert(format!("group_{}", group.name), entry);
        }

        let wind = (self.world.hello.map.wind_min + self.world.hello.map.wind_max) / 2.0;
        let rules = format!(
            "{}This map's wind averages about {wind:.0}: {}.",
            crate::texts::read(&crate::texts::HANDS_RULES),
            if wind >= 8.0 { "wind generators (40 metal) beat solar collectors here" } else { "solar collectors are the reliable energy here" }
        );
        let state = json!({
            "instructions": instructions,
            "clock": clock(frame),
            "rules": rules,
            "economy": economy,
            "ours": ours,
            "enemy": enemy,
            "places": place_entries,
            "actors": actors,
            "recent": pianist.recent(frame),
        });
        Picture { state, places, parties }
    }

    /// How an actor is named in the picture and the questions.
    pub(super) fn actor_name(&self, unit: bot_protocol::UnitId, kit: &Kit) -> String {
        match self.known_units.get(&unit).map(|(def, _)| *def) {
            Some(def) if def == kit.commander => "commander".into(),
            Some(def) if def == kit.lab || def == kit.advanced_lab => format!("lab_{}", unit.0),
            _ => format!("constructor_{}", unit.0),
        }
    }
}
