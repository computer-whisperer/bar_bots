//! Static game data from `Hello`, indexed for lookup.

use std::collections::HashMap;

use bot_protocol::{Hello, UnitDefId, UnitDefInfo, Vec3};

pub struct World {
    pub hello: Hello,
    defs: HashMap<UnitDefId, usize>,
    names: HashMap<String, UnitDefId>,
}

impl World {
    pub fn new(hello: Hello) -> Self {
        let defs = hello.unit_defs.iter().enumerate().map(|(i, d)| (d.id, i)).collect();
        let names = hello.unit_defs.iter().map(|d| (d.name.clone(), d.id)).collect();
        World { hello, defs, names }
    }

    pub fn def(&self, id: UnitDefId) -> Option<&UnitDefInfo> {
        self.defs.get(&id).map(|&i| &self.hello.unit_defs[i])
    }

    pub fn def_named(&self, name: &str) -> Option<UnitDefId> {
        self.names.get(name).copied()
    }

    /// Name of the cell containing `pos` on an 8x8 grid: columns A-H west to east, rows 1-8 north to south.
    pub fn grid(&self, pos: Vec3) -> String {
        let cell = |value: f32, extent: f32| ((value / extent * 8.0) as i32).clamp(0, 7) as u8;
        let (column, row) = (cell(pos.x, self.hello.map.width), cell(pos.z, self.hello.map.height));
        format!("{}{}", (b'A' + column) as char, row + 1)
    }

    /// The point opposite `pos` through the map centre: a first guess at where the enemy starts.
    pub fn mirrored(&self, pos: Vec3) -> Vec3 {
        Vec3 { x: self.hello.map.width - pos.x, y: 0.0, z: self.hello.map.height - pos.z }
    }
}
