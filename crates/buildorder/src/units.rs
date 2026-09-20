//! The unit table: numbers derived from the game's unit definition files by `tools/extract_units.py`.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Commander,
    /// Economy building: extractor, generator, converter, storage.
    Eco,
    Factory,
    /// Mobile constructor (also resurrection bots and minelayers: anything a factory builds that has build power).
    Builder,
    Nano,
    Turret,
    Army,
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub name: String,
    pub role: Role,
    /// "lab" or "vp" for units built there, empty otherwise.
    pub factory: String,
    pub metal_cost: f64,
    pub energy_cost: f64,
    pub build_time: f64,
    pub worker_time: f64,
    pub build_distance: f64,
    /// Elmos per second.
    pub speed: f64,
    pub metal_make: f64,
    /// Net constant energy production: `energymake - energyupkeep` (solars produce through a negative upkeep).
    pub energy_make: f64,
    pub extracts_metal: f64,
    pub wind_cap: f64,
    pub metal_storage: f64,
    pub energy_storage: f64,
    /// Energy per second a converter can take, and metal returned per energy.
    pub conv_capacity: f64,
    pub conv_efficiency: f64,
}

pub struct Units {
    pub list: Vec<Unit>,
    by_name: HashMap<String, usize>,
}

const TABLE: &str = include_str!("../data/units.csv");

impl Units {
    pub fn load() -> Units {
        let mut lines = TABLE.lines().filter(|l| !l.starts_with('#'));
        let header: Vec<&str> = lines.next().expect("header").split(',').collect();
        let col = |name: &str| header.iter().position(|h| *h == name).unwrap_or_else(|| panic!("column {name}"));
        let (c_name, c_role, c_factory) = (col("name"), col("role"), col("factory"));
        let num = |cells: &[&str], name: &str| cells[col(name)].parse::<f64>().unwrap_or(0.0);
        let mut list = Vec::new();
        for line in lines.filter(|l| !l.is_empty()) {
            let cells: Vec<&str> = line.split(',').collect();
            let role = match cells[c_role] {
                "com" => Role::Commander,
                "eco" => Role::Eco,
                "factory" => Role::Factory,
                "builder" => Role::Builder,
                "nano" => Role::Nano,
                "turret" => Role::Turret,
                "army" => Role::Army,
                other => panic!("role {other}"),
            };
            list.push(Unit {
                name: cells[c_name].to_string(),
                role,
                factory: cells[c_factory].to_string(),
                metal_cost: num(&cells, "metalcost"),
                energy_cost: num(&cells, "energycost"),
                build_time: num(&cells, "buildtime"),
                worker_time: num(&cells, "workertime"),
                build_distance: num(&cells, "builddistance"),
                speed: num(&cells, "speed"),
                metal_make: num(&cells, "metalmake"),
                energy_make: num(&cells, "energymake") - num(&cells, "energyupkeep"),
                extracts_metal: num(&cells, "extractsmetal"),
                wind_cap: num(&cells, "windgenerator"),
                metal_storage: num(&cells, "metalstorage"),
                energy_storage: num(&cells, "energystorage"),
                conv_capacity: num(&cells, "energyconv_capacity"),
                conv_efficiency: num(&cells, "energyconv_efficiency"),
            });
        }
        let by_name = list.iter().enumerate().map(|(i, u)| (u.name.clone(), i)).collect();
        Units { list, by_name }
    }

    pub fn index(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    pub fn get(&self, name: &str) -> &Unit {
        &self.list[self.index(name).unwrap_or_else(|| panic!("unit {name} is not in data/units.csv"))]
    }
}
