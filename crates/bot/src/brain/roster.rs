//! Which unit fills which role, per faction.

use bot_protocol::UnitDefId;

use crate::world::World;

/// Unit names for one faction.
pub struct Roster {
    pub commander: &'static str,
    extractor: &'static str,
    solar: &'static str,
    wind: &'static str,
    advanced_solar: &'static str,
    converter: &'static str,
    lab: &'static str,
    turret: &'static str,
    nano: &'static str,
    radar: &'static str,
    constructor: &'static str,
    /// Tier 2 (docs/knowledge/tier2.md): the advanced bot lab, its constructor, the extractor that one builds over a
    /// tier-1 extractor, and the two units the lab makes until a commander says otherwise.
    advanced_lab: &'static str,
    advanced_constructor: &'static str,
    advanced_extractor: &'static str,
    advanced_line: &'static str,
    advanced_second: &'static str,
    raider: &'static str,
    skirmisher: &'static str,
    artillery: &'static str,
    /// The unit the line is made of, and a second one to go with it (K-units-duel-*).
    line: &'static str,
    second: &'static str,
}

pub const ROSTERS: [Roster; 2] = [
    Roster {
        commander: "armcom", extractor: "armmex", solar: "armsolar", wind: "armwin", advanced_solar: "armadvsol",
        converter: "armmakr", lab: "armlab", turret: "armllt", nano: "armnanotc", radar: "armrad", constructor: "armck",
        advanced_lab: "armalab", advanced_constructor: "armack", advanced_extractor: "armmoho", advanced_line: "armzeus", advanced_second: "armfido",
        raider: "armpw", skirmisher: "armrock", artillery: "armham", line: "armham", second: "armwar",
    },
    Roster {
        commander: "corcom", extractor: "cormex", solar: "corsolar", wind: "corwin", advanced_solar: "coradvsol",
        converter: "cormakr", lab: "corlab", turret: "corllt", nano: "cornanotc", radar: "corrad", constructor: "corck",
        advanced_lab: "coralab", advanced_constructor: "corack", advanced_extractor: "cormoho", advanced_line: "corcan", advanced_second: "corcan",
        raider: "corak", skirmisher: "corstorm", artillery: "corthud", line: "corthud", second: "corstorm",
    },
];

/// A roster resolved against the running game's unit definitions.
#[derive(Clone, Copy)]
pub struct Kit {
    pub commander: UnitDefId,
    pub extractor: UnitDefId,
    pub solar: UnitDefId,
    pub wind: UnitDefId,
    pub advanced_solar: UnitDefId,
    pub converter: UnitDefId,
    pub lab: UnitDefId,
    pub turret: UnitDefId,
    /// Construction turret: a fixed builder that adds its build power to a factory it stands beside.
    pub nano: UnitDefId,
    pub radar: UnitDefId,
    pub constructor: UnitDefId,
    pub advanced_lab: UnitDefId,
    pub advanced_constructor: UnitDefId,
    pub advanced_extractor: UnitDefId,
    pub advanced_line: UnitDefId,
    pub advanced_second: UnitDefId,
    pub raider: UnitDefId,
    pub skirmisher: UnitDefId,
    pub artillery: UnitDefId,
    pub line: UnitDefId,
    pub second: UnitDefId,
}

impl Kit {
    /// Either tier: what holds a metal spot for us.
    pub fn is_extractor(&self, def: UnitDefId) -> bool {
        def == self.extractor || def == self.advanced_extractor
    }
}

impl Roster {
    /// `Err` names the first unit this game does not define.
    pub fn resolve(&self, world: &World) -> Result<Kit, &'static str> {
        let id = |name: &'static str| world.def_named(name).ok_or(name);
        Ok(Kit {
            commander: id(self.commander)?,
            extractor: id(self.extractor)?,
            solar: id(self.solar)?,
            wind: id(self.wind)?,
            advanced_solar: id(self.advanced_solar)?,
            converter: id(self.converter)?,
            lab: id(self.lab)?,
            turret: id(self.turret)?,
            nano: id(self.nano)?,
            radar: id(self.radar)?,
            constructor: id(self.constructor)?,
            advanced_lab: id(self.advanced_lab)?,
            advanced_constructor: id(self.advanced_constructor)?,
            advanced_extractor: id(self.advanced_extractor)?,
            advanced_line: id(self.advanced_line)?,
            advanced_second: id(self.advanced_second)?,
            raider: id(self.raider)?,
            skirmisher: id(self.skirmisher)?,
            artillery: id(self.artillery)?,
            line: id(self.line)?,
            second: id(self.second)?,
        })
    }
}
