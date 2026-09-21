//! Walking distances for the brain's geometry: what is reachable, what is ours, which way is forward.
//! Everything falls back to straight lines when the engine sent no terrain.

use std::collections::HashMap;

use bot_protocol::{UnitDefId, UnitId, Vec3};

use super::Brain;
use super::roster::Kit;
use terrain::Field;

/// The enemy-side field is rebuilt when our estimate of where an enemy lives has moved this far.
const ENEMY_MOVED: f32 = 600.0;
/// A place this near a metal spot borrows the spot's field for its routes.
const SPOT_PROXY: f32 = 400.0;

/// The movement classes we field: our soldiers and constructors walk as the lab's raider does, the commander as
/// itself (`COMMANDERBOT` crosses different ground, and slower on slopes). Vehicles when a game has them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Walker {
    Bots,
    Commander,
}

pub struct Routes {
    from_home: Field,
    /// Effective elmos from home for the commander (its own class and slopes), for its leash in seconds.
    commander_from_home: Option<Field>,
    /// Distance from the nearest live enemy base.
    from_enemy: Option<Field>,
    enemy_origins: Vec<Vec3>,
    passable: Vec<bool>,
    /// One field per metal spot (the map's order) per class, in effective elmos (slopes priced by the engine's
    /// law), so that "nearest spot" means travel time from wherever a unit stands (the user, 2026-09-20: the
    /// commander walked the cliffs behind the Quicksilver base to spots a straight line called near). Built on a
    /// thread at the survey (10 ms a field); empty until they arrive.
    spot_fields: HashMap<Walker, Vec<Option<Field>>>,
    spot_fields_pending: Option<std::sync::mpsc::Receiver<HashMap<Walker, Vec<Option<Field>>>>>,
    /// The commander's costs, for the safe field home.
    commander_costs: Option<Vec<u32>>,
    /// The commander's way home round every known armed building's reach and their commander's ground (the safe
    /// flavour): the one place a waypoint is given, for deliberate avoidance (the user, 2026-09-20). Rebuilt on
    /// the thread when the known threats change; `None` (the plain way) until it arrives.
    safe_home: Option<Field>,
    safe_home_pending: Option<std::sync::mpsc::Receiver<Option<Field>>>,
    /// The threats the safe field was last built against: armed building ids and the commander's place.
    safe_home_against: (Vec<UnitId>, Option<(i32, i32)>),
}

impl Brain {
    /// Fields for our soldiers' movement class (the lab's raider stands for all of them).
    pub(super) fn survey(&mut self, kit: &Kit) {
        let terrain = &self.world.hello.terrain;
        let Some(class) = self.world.def(kit.raider).and_then(|d| d.move_class) else { return };
        let passable = terrain::passable(terrain, class);
        let Some(from_home) = Field::from(terrain, &passable, self.home) else {
            eprintln!("[ai {}] no terrain data; distances are straight lines", self.ai());
            return;
        };
        // H-MAP-ENEMY-START: the mirror image of our start is only where the enemy would be on a symmetric map. On
        // Quicksilver it is a beach across the water from the real base, 700 elmos off, and armies sent "to the enemy
        // start" stood there looking at the sea (the spot we can walk to nearest it is 140 from the real start).
        if self.enabled("H-MAP-ENEMY-START") {
            self.snap_guesses_to_metal(|spot| from_home.distance(spot).is_some());
        }
        let terrain = &self.world.hello.terrain;
        let enemy_origins = self.live_enemy_bases();
        let from_enemy = Field::from_many(terrain, &passable, &enemy_origins);
        let spots = &self.world.hello.metal_spots;
        let reachable = spots.iter().filter(|s| from_home.distance(**s).is_some()).count();
        let ours = spots.iter().filter(|s| is_ours(&from_home, from_enemy.as_ref(), **s)).count();
        eprintln!(
            "[ai {}] terrain: {} of {} metal spots reachable on foot, {} nearer to us than to the enemy; enemy start {:.0} away on foot, {:.0} in a straight line",
            self.ai(), reachable, spots.len(), ours,
            from_home.distance(self.enemy_base(self.home)).unwrap_or(f32::INFINITY), self.home.dist2d(self.enemy_base(self.home))
        );
        let cut_off: Vec<String> =
            spots.iter().filter(|s| from_home.distance(**s).is_none()).map(|s| format!("({:.0}, {:.0})", s.x, s.z)).collect();
        eprintln!("[ai {}] terrain: spots we cannot walk to: {}", self.ai(), cut_off.join(" "));
        let commander_class = self.world.def(kit.commander).and_then(|d| d.move_class);
        let bot_costs = terrain::costs(terrain, class);
        let commander_costs = commander_class.map(|c| terrain::costs(terrain, c));
        let commander_from_home = commander_costs.as_ref().and_then(|costs| Field::from_costs(terrain, costs, &[self.home]));
        let (sender, receiver) = std::sync::mpsc::channel();
        {
            let terrain = terrain.clone();
            let spots = spots.clone();
            std::thread::spawn(move || {
                let mut fields = HashMap::new();
                fields.insert(Walker::Bots, spots.iter().map(|spot| Field::from_costs(&terrain, &bot_costs, &[*spot])).collect());
                if let Some(costs) = commander_costs {
                    fields.insert(Walker::Commander, spots.iter().map(|spot| Field::from_costs(&terrain, &costs, &[*spot])).collect());
                }
                let _ = sender.send(fields);
            });
        }
        self.routes = Some(Routes {
            from_home, commander_from_home, from_enemy, enemy_origins, passable, spot_fields: HashMap::new(), spot_fields_pending: Some(receiver),
            commander_costs: commander_class.map(|c| terrain::costs(terrain, c)), safe_home: None, safe_home_pending: None, safe_home_against: (Vec::new(), None),
        });
        for p in self.passages() {
            eprintln!("[ai {}] terrain: passage at {} ({:.0}, {:.0}), {:.0} wide, {:.0} % of the way to the enemy", self.ai(), self.world.grid(p.at), p.at.x, p.at.z, p.width, p.along * 100.0);
        }
        if let Some(sketch) = self.terrain_sketch() {
            for row in sketch["rows"].as_array().into_iter().flatten() {
                eprintln!("[ai {}] terrain: {}", self.ai(), row.as_str().unwrap_or_default());
            }
        }
    }

    /// Call when an enemy base may have moved, died or come back.
    pub(super) fn resurvey_enemy(&mut self) {
        let origins = self.live_enemy_bases();
        let terrain = &self.world.hello.terrain;
        if let Some(routes) = &mut self.routes
            && (routes.enemy_origins.len() != origins.len() || routes.enemy_origins.iter().zip(&origins).any(|(a, b)| a.dist2d(*b) > ENEMY_MOVED))
        {
            routes.from_enemy = Field::from_many(terrain, &routes.passable, &origins);
            routes.enemy_origins = origins;
        }
    }

    /// Takes the spot fields once the thread has built them; call every think tick. Also keeps the commander's safe
    /// way home current against the known threats.
    pub(super) fn receive_spot_fields(&mut self) {
        let Some(routes) = &mut self.routes else { return };
        if let Some(receiver) = &routes.spot_fields_pending
            && let Ok(fields) = receiver.try_recv()
        {
            routes.spot_fields = fields;
            routes.spot_fields_pending = None;
        }
        if let Some(receiver) = &routes.safe_home_pending
            && let Ok(field) = receiver.try_recv()
        {
            routes.safe_home = field;
            routes.safe_home_pending = None;
        }
        self.refresh_safe_home();
    }

    /// Rebuilds the commander's safe field home on the thread when the known armed buildings or their commander's
    /// place have changed since the last build, and none is in progress.
    fn refresh_safe_home(&mut self) {
        let rules = self.contacts.rules.clone();
        let mut walls: Vec<(UnitId, Vec3, f32)> = self.enemy_buildings.iter().filter_map(|(id, (def, pos, _))| {
            let d = self.world.def(*def)?;
            (d.weapon_count > 0).then_some(())?;
            let reach = rules.units.list[*self.contacts.sim_defs.get(def)?].reach();
            Some((*id, *pos, reach + super::contact::TURRET_MARGIN))
        }).collect();
        walls.sort_by_key(|(id, _, _)| *id);
        let commander = self.enemy_commander_seen.map(|(p, _)| (p.x as i32, p.z as i32));
        let against = (walls.iter().map(|(id, _, _)| *id).collect::<Vec<_>>(), commander);
        let Some(routes) = &mut self.routes else { return };
        if routes.safe_home_pending.is_some() || routes.safe_home_against == against {
            return;
        }
        let Some(base) = routes.commander_costs.clone() else { return };
        routes.safe_home_against = against;
        let terrain = self.world.hello.terrain.clone();
        let home = self.home;
        let mut zones: Vec<(Vec3, f32)> = walls.into_iter().map(|(_, pos, reach)| (pos, reach)).collect();
        if let Some((pos, _)) = self.enemy_commander_seen {
            zones.push((pos, super::raid::COMMANDER_GROUND));
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        routes.safe_home_pending = Some(receiver);
        std::thread::spawn(move || {
            let mut costs = base;
            let (cell, width) = (terrain.cell, terrain.width as usize);
            for (index, cost) in costs.iter_mut().enumerate() {
                let centre = Vec3 { x: ((index % width) as f32 + 0.5) * cell, y: 0.0, z: ((index / width) as f32 + 0.5) * cell };
                if zones.iter().any(|(at, radius)| at.dist2d(centre) < *radius) {
                    *cost = terrain::IMPASSABLE;
                }
            }
            let _ = sender.send(Field::from_costs(&terrain, &costs, &[home]));
        });
    }

    /// The commander's way home round known threats, as one waypoint: the first place the safe way turns by more
    /// than a right angle's third from the straight line home, if the safe way parts from the straight one at
    /// all. `None`: go straight home.
    pub(super) fn commander_waypoint_home(&self, from: Vec3) -> Option<Vec3> {
        let field = self.routes.as_ref()?.safe_home.as_ref()?;
        let route = field.route(from);
        if route.len() < 3 {
            return None;
        }
        let (dx, dz) = (self.home.x - from.x, self.home.z - from.z);
        let straight = dz.atan2(dx);
        // The route's first cell is where the commander stands; a turning is where the way's direction from the
        // start leaves the straight line's by more than 30 degrees.
        for point in route.iter().skip(2) {
            let heading = (point.z - from.z).atan2(point.x - from.x);
            let off = (heading - straight).abs().min(std::f32::consts::TAU - (heading - straight).abs());
            if off > 30f32.to_radians() && point.dist2d(from) > 100.0 {
                // The waypoint is the farthest point on the way that is still within a straight-line clear of
                // the walls' ground: the route itself is safe, so the point halfway along it serves.
                let midway = route[route.len() / 2];
                return Some(midway);
            }
        }
        None
    }

    /// Which class a unit of this type walks as.
    pub(super) fn walker_of(&self, def: UnitDefId) -> Walker {
        if self.kit.is_some_and(|k| k.commander == def) { Walker::Commander } else { Walker::Bots }
    }

    /// Effective elmos (elmos at full speed, slopes priced) from `from` to the metal spot with this index for a
    /// class; the straight line until the fields are built or where the spot cannot be walked to.
    pub(super) fn walk_to_spot_as(&self, walker: Walker, index: usize, from: Vec3) -> f32 {
        let spot = self.world.hello.metal_spots.get(index).copied().unwrap_or(from);
        self.routes.as_ref().and_then(|r| r.spot_fields.get(&walker)?.get(index)?.as_ref()?.distance(from)).unwrap_or_else(|| spot.dist2d(from))
    }

    /// As our soldiers walk.
    pub(super) fn walk_to_spot(&self, index: usize, from: Vec3) -> f32 {
        self.walk_to_spot_as(Walker::Bots, index, from)
    }

    /// Seconds a unit of this type takes from `from` to the spot (its speed over the effective elmos).
    pub(super) fn seconds_to_spot(&self, def: UnitDefId, index: usize, from: Vec3) -> f32 {
        let speed = self.world.def(def).map_or(1.0, |d| d.speed.max(1.0));
        self.walk_to_spot_as(self.walker_of(def), index, from) / speed
    }

    /// The way our soldiers would walk from `from` to `to`, read off the field of the metal spot nearest `to` when
    /// one lies within `SPOT_PROXY` of it (targets round a base are extractors, spots and ring points beside them);
    /// `None` when there is no such field yet or `from` cannot reach it. Coarse, for accounting, never for orders
    /// (the user, 2026-09-20).
    pub(super) fn route_to(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        let (index, _) = self.world.hello.metal_spots.iter().enumerate().map(|(i, s)| (i, s.dist2d(to))).filter(|(_, d)| *d < SPOT_PROXY).min_by(|a, b| a.1.total_cmp(&b.1))?;
        let field = self.routes.as_ref()?.spot_fields.get(&Walker::Bots)?.get(index)?.as_ref()?;
        let route = field.route(from);
        (!route.is_empty()).then_some(route)
    }

    /// Seconds a unit of this type takes from `from` to `site`: over the field of the metal spot at `site` when one
    /// lies within 100 of it (extractors, radars and turrets at spots), else the straight line at its speed.
    pub(super) fn seconds_to_site(&self, def: UnitDefId, from: Vec3, site: Vec3) -> f32 {
        let speed = self.world.def(def).map_or(1.0, |d| d.speed.max(1.0));
        match self.world.hello.metal_spots.iter().position(|s| s.dist2d(site) < 100.0) {
            Some(index) => self.seconds_to_spot(def, index, from),
            None => site.dist2d(from) / speed,
        }
    }

    /// Seconds the commander takes from home to `pos`, on its own ground; the straight line without terrain.
    pub(super) fn commander_seconds_from_home(&self, pos: Vec3) -> f32 {
        let speed = self.kit.and_then(|k| self.world.def(k.commander)).map_or(1.0, |d| d.speed.max(1.0));
        let effective = self.routes.as_ref().and_then(|r| r.commander_from_home.as_ref()?.distance(pos)).unwrap_or_else(|| pos.dist2d(self.home));
        effective / speed
    }

    /// The index of the metal spot at `pos`, if one lies there.
    pub(super) fn spot_index(&self, pos: Vec3) -> Option<usize> {
        self.world.hello.metal_spots.iter().position(|s| s.dist2d(pos) < 1.0)
    }

    /// Whether our soldiers can walk from home to (next to) `pos`. True when we cannot tell.
    pub(super) fn reachable_on_foot(&self, pos: Vec3) -> bool {
        self.routes.as_ref().is_none_or(|r| r.from_home.distance(pos).is_some())
    }

    /// Walking distance from home, else the straight line.
    pub(super) fn walk_from_home(&self, pos: Vec3) -> f32 {
        self.routes.as_ref().and_then(|r| r.from_home.distance(pos)).unwrap_or_else(|| pos.dist2d(self.home))
    }

    /// Walking distance from the nearest live enemy base; `None` when it cannot walk here or we have no terrain.
    pub(super) fn walk_from_enemy(&self, pos: Vec3) -> Option<f32> {
        match &self.routes {
            Some(routes) => routes.from_enemy.as_ref().and_then(|f| f.distance(pos)),
            None => Some(pos.dist2d(self.enemy_base(pos))),
        }
    }

    /// A metal spot we can walk to that is nearer to us than to the enemy, on foot.
    pub(super) fn spot_is_ours(&self, spot: Vec3) -> bool {
        match &self.routes {
            Some(routes) => is_ours(&routes.from_home, routes.from_enemy.as_ref(), spot),
            None => spot.dist2d(self.home) < spot.dist2d(self.enemy_base(spot)),
        }
    }

    /// The point `along` elmos from home on the way to `goal`, on foot.
    pub(super) fn on_the_way_to(&self, goal: Vec3, along: f32) -> Option<Vec3> {
        self.routes.as_ref()?.from_home.towards(goal, along)
    }

    /// The map in text for the language model, with a legend; `None` without terrain data.
    pub(super) fn terrain_sketch(&self) -> Option<serde_json::Value> {
        const SIZE: usize = 32;
        let routes = self.routes.as_ref()?;
        let rows = terrain::sketch(&self.world.hello.terrain, &routes.passable, &routes.from_home, SIZE);
        // Four characters to a grid column, four rows to a grid row, so A1..H8 can be read off the picture.
        let mut lines = vec!["   A   B   C   D   E   F   G   H".to_string()];
        lines.extend(rows.iter().enumerate().map(|(i, row)| {
            let label = if i % 4 == 0 { format!("{} ", i / 4 + 1) } else { "  ".to_string() };
            format!("{label}{row}")
        }));
        Some(serde_json::json!({
            "legend": "north is up; 4x4 characters per grid cell. ~ water, # cliff or slope our bots cannot cross, x ground we cannot walk to from our start, . o O walkable ground (low, middle, high)",
            "rows": lines,
        }))
    }

    /// The narrow places on the ways between home and the nearest enemy bases, the narrowest first.
    pub(super) fn passages(&self) -> Vec<terrain::Passage> {
        self.routes.as_ref().and_then(|r| r.from_enemy.as_ref().map(|enemy| terrain::passages(&r.from_home, enemy))).unwrap_or_default()
    }

    /// Where our soldiers can stand, one flag per terrain cell (`Terrain` order); `None` without terrain data.
    pub(super) fn passable(&self) -> Option<&[bool]> {
        self.routes.as_ref().map(|r| r.passable.as_slice())
    }

    /// The reachable ground nearest `pos`; `pos` itself when we cannot tell.
    pub(super) fn snap_to_reachable(&self, pos: Vec3) -> Vec3 {
        self.routes.as_ref().and_then(|r| r.from_home.snap(pos)).unwrap_or(pos)
    }
}

fn is_ours(from_home: &Field, from_enemy: Option<&Field>, spot: Vec3) -> bool {
    let Some(ours) = from_home.distance(spot) else { return false };
    from_enemy.and_then(|f| f.distance(spot)).is_none_or(|theirs| ours < theirs)
}
