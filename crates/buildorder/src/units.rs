//! The unit table: the numbers the engine reports for this game's unit types, as the bot's `Hello` or a match
//! record's header carries them. Nothing is compiled in.

use std::collections::HashMap;

use bot_protocol::{MoveClass, UnitDefInfo};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Commander,
    /// Economy building: extractor, generator, converter, storage.
    Eco,
    Factory,
    /// Mobile constructor: anything that moves and has a build menu.
    Builder,
    Nano,
    Turret,
    Army,
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub name: String,
    pub role: Role,
    /// Indices into `Units::list` of what it builds.
    pub builds: Vec<usize>,
    pub metal_cost: f64,
    pub energy_cost: f64,
    pub build_time: f64,
    pub worker_time: f64,
    pub build_distance: f64,
    pub speed: f64,
    pub move_class: Option<MoveClass>,
    pub metal_make: f64,
    /// Net constant energy production: `energy_make - energy_upkeep`.
    pub energy_make: f64,
    pub extracts_metal: f64,
    pub wind_cap: f64,
    pub metal_storage: f64,
    pub energy_storage: f64,
    pub radar_range: f64,
    pub conv_capacity: f64,
    pub conv_efficiency: f64,
}

pub struct Units {
    pub list: Vec<Unit>,
    by_name: HashMap<String, usize>,
}

fn role(n: &UnitDefInfo) -> Role {
    let (moves, menu) = (n.speed > 0.0, !n.build_options.is_empty());
    match () {
        _ if moves && menu && n.name.ends_with("com") => Role::Commander,
        _ if moves && menu => Role::Builder,
        _ if menu => Role::Factory,
        _ if !moves && n.build_speed > 0.0 => Role::Nano,
        _ if !moves && n.weapon_count > 0 => Role::Turret,
        _ if moves && n.weapon_count > 0 => Role::Army,
        _ => Role::Eco,
    }
}

impl Units {
    pub fn new(defs: &[UnitDefInfo]) -> Units {
        let index_of: HashMap<_, usize> = defs.iter().enumerate().map(|(i, n)| (n.id, i)).collect();
        let list: Vec<Unit> = defs
            .iter()
            .map(|n| Unit {
                name: n.name.clone(),
                role: role(n),
                builds: n.build_options.iter().filter_map(|id| index_of.get(id).copied()).collect(),
                metal_cost: n.metal_cost as f64,
                energy_cost: n.energy_cost as f64,
                build_time: n.build_time as f64,
                worker_time: n.build_speed as f64,
                build_distance: n.build_distance as f64,
                speed: n.speed as f64,
                move_class: n.move_class,
                metal_make: n.metal_make as f64,
                energy_make: (n.energy_make - n.energy_upkeep) as f64,
                extracts_metal: n.extracts_metal as f64,
                wind_cap: n.wind_cap as f64,
                metal_storage: n.metal_storage as f64,
                energy_storage: n.energy_storage as f64,
                radar_range: n.radar_range as f64,
                conv_capacity: n.converter.map_or(0.0, |c| c.capacity as f64),
                conv_efficiency: n.converter.map_or(0.0, |c| c.efficiency as f64),
            })
            .collect();
        let by_name = list.iter().enumerate().map(|(i, u)| (u.name.clone(), i)).collect();
        Units { list, by_name }
    }

    pub fn index(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    pub fn get(&self, name: &str) -> &Unit {
        &self.list[self.index(name).unwrap_or_else(|| panic!("no unit type {name} in this game"))]
    }

    /// The cheapest thing `builder` builds that passes `test`.
    pub fn cheapest(&self, builder: usize, test: impl Fn(&Unit) -> bool) -> Option<usize> {
        self.list[builder].builds.iter().copied().filter(|u| test(&self.list[*u])).min_by(|a, b| self.list[*a].metal_cost.total_cmp(&self.list[*b].metal_cost))
    }

    pub fn extractor(&self, builder: usize) -> Option<usize> {
        self.cheapest(builder, |u| u.extracts_metal > 0.0)
    }
}
