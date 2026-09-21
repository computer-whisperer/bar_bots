//! Walking distances for the brain's geometry: what is reachable, what is ours, which way is forward.
//! Everything falls back to straight lines when the engine sent no terrain.

use std::collections::HashMap;

use bot_protocol::{UnitDefId, Vec3};

use super::Brain;
use super::roster::Kit;
use terrain::Field;

/// The enemy-side field is rebuilt when our estimate of where an enemy lives has moved this far.
const ENEMY_MOVED: f32 = 600.0;

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
        self.routes = Some(Routes { from_home, commander_from_home, from_enemy, enemy_origins, passable, spot_fields: HashMap::new(), spot_fields_pending: Some(receiver) });
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

    /// Takes the spot fields once the thread has built them; call every think tick.
    pub(super) fn receive_spot_fields(&mut self) {
        let Some(routes) = &mut self.routes else { return };
        if let Some(receiver) = &routes.spot_fields_pending
            && let Ok(fields) = receiver.try_recv()
        {
            routes.spot_fields = fields;
            routes.spot_fields_pending = None;
        }
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
