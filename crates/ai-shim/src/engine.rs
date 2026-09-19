//! Safe wrapper over the engine's callback table, exposing only what the shim needs.
//!
//! Every method must be called on the engine thread, from inside one of the library's exports.

use std::ffi::{CStr, c_int, c_void};

use bot_protocol::{
    BuildSite, Command, EnemyUnit, Hello, MapInfo, OwnUnit, Resource, Snapshot, UnitDefId,
    UnitDefInfo, UnitId, Vec3,
};
use recoil_ai_sys as sys;

/// Heightmap squares to elmos.
const SQUARE_SIZE: f32 = 8.0;
const MAX_UNITS: usize = 32_000;

pub struct Engine {
    ai_id: c_int,
    callback: *const sys::SSkirmishAICallback,
    metal: c_int,
    energy: c_int,
    id_buf: Vec<c_int>,
}

/// Calls a callback-table entry, which always takes the AI id first.
macro_rules! call {
    ($engine:expr, $name:ident ( $($arg:expr),* )) => {
        unsafe { ((*$engine.callback).$name.expect(stringify!($name)))($engine.ai_id $(, $arg)*) }
    };
}

impl Engine {
    /// # Safety
    /// `callback` must be the table the engine passed to `init` for `ai_id`, and stay valid
    /// until `release`.
    pub unsafe fn new(ai_id: c_int, callback: *const sys::SSkirmishAICallback) -> Self {
        let mut engine = Engine { ai_id, callback, metal: -1, energy: -1, id_buf: vec![0; MAX_UNITS] };
        engine.metal = call!(engine, getResourceByName(c"Metal".as_ptr()));
        engine.energy = call!(engine, getResourceByName(c"Energy".as_ptr()));
        engine
    }

    pub fn hello(&mut self, frame: i32) -> Hello {
        let map = MapInfo {
            name: self.string(call!(self, Map_getName())),
            width: call!(self, Map_getWidth()) as f32 * SQUARE_SIZE,
            height: call!(self, Map_getHeight()) as f32 * SQUARE_SIZE,
            wind_min: call!(self, Map_getMinWind()),
            wind_max: call!(self, Map_getMaxWind()),
        };
        let def_count = call!(self, getUnitDefs(std::ptr::null_mut(), 0));
        let mut def_ids = vec![0; def_count.max(0) as usize];
        call!(self, getUnitDefs(def_ids.as_mut_ptr(), def_count));
        let unit_defs = def_ids.into_iter().map(|id| self.unit_def(id)).collect();

        let float_count = call!(self, Map_getResourceMapSpotsPositions(self.metal, std::ptr::null_mut(), 0));
        let mut floats = vec![0f32; float_count.max(0) as usize];
        call!(self, Map_getResourceMapSpotsPositions(self.metal, floats.as_mut_ptr(), float_count));
        let metal_spots =
            floats.chunks_exact(3).map(|s| Vec3 { x: s[0], y: s[1], z: s[2] }).collect();

        Hello {
            ai_id: self.ai_id,
            team: call!(self, SkirmishAI_getTeamId()),
            ally_team: call!(self, Game_getMyAllyTeam()),
            frame,
            map,
            unit_defs,
            metal_spots,
        }
    }

    fn unit_def(&mut self, id: c_int) -> UnitDefInfo {
        let option_count = call!(self, UnitDef_getBuildOptions(id, std::ptr::null_mut(), 0));
        let mut options = vec![0; option_count.max(0) as usize];
        call!(self, UnitDef_getBuildOptions(id, options.as_mut_ptr(), option_count));
        UnitDefInfo {
            id: UnitDefId(id),
            name: self.string(call!(self, UnitDef_getName(id))),
            metal_cost: call!(self, UnitDef_getCost(id, self.metal)),
            energy_cost: call!(self, UnitDef_getCost(id, self.energy)),
            speed: call!(self, UnitDef_getSpeed(id)),
            build_speed: call!(self, UnitDef_getBuildSpeed(id)),
            extracts_metal: call!(self, UnitDef_getExtractsResource(id, self.metal)),
            weapon_count: call!(self, UnitDef_getWeaponMounts(id)),
            build_options: options.into_iter().map(UnitDefId).collect(),
        }
    }

    pub fn snapshot(&mut self) -> Snapshot {
        let max = self.id_buf.len() as c_int;
        let own_count = call!(self, getTeamUnits(self.id_buf.as_mut_ptr(), max)).max(0) as usize;
        let own_units = self.id_buf[..own_count]
            .iter()
            .map(|&id| OwnUnit {
                id: UnitId(id),
                def: UnitDefId(call!(self, Unit_getDef(id))),
                pos: self.unit_pos(id),
                health: call!(self, Unit_getHealth(id)),
                max_health: call!(self, Unit_getMaxHealth(id)),
                being_built: call!(self, Unit_isBeingBuilt(id)),
                idle: call!(self, Unit_getCurrentCommands(id)) == 0,
            })
            .collect();

        let enemy_count = call!(self, getEnemyUnitsInRadarAndLos(self.id_buf.as_mut_ptr(), max)).max(0) as usize;
        let enemies = self.id_buf[..enemy_count]
            .iter()
            .map(|&id| {
                let def = call!(self, Unit_getDef(id));
                EnemyUnit {
                    id: UnitId(id),
                    def: (def >= 0).then_some(UnitDefId(def)),
                    pos: self.unit_pos(id),
                    health: call!(self, Unit_getHealth(id)),
                }
            })
            .collect();

        Snapshot { metal: self.resource(self.metal), energy: self.resource(self.energy), own_units, enemies }
    }

    fn resource(&self, id: c_int) -> Resource {
        Resource {
            current: call!(self, Economy_getCurrent(id)),
            income: call!(self, Economy_getIncome(id)),
            usage: call!(self, Economy_getUsage(id)),
            storage: call!(self, Economy_getStorage(id)),
        }
    }

    fn unit_pos(&self, unit: c_int) -> Vec3 {
        let mut pos = [0f32; 3];
        call!(self, Unit_getPos(unit, pos.as_mut_ptr()));
        Vec3 { x: pos[0], y: pos[1], z: pos[2] }
    }

    /// Closest legal position for `def` near `site`, if the map has one.
    pub fn find_build_site(&self, def: UnitDefId, site: BuildSite) -> Option<Vec3> {
        let mut near = [site.near.x, site.near.y, site.near.z];
        let mut found = [0f32; 3];
        call!(self, Map_findClosestBuildSite(
            def.0, near.as_mut_ptr(), site.search_radius, site.min_dist,
            sys::UNIT_COMMAND_BUILD_NO_FACING, found.as_mut_ptr()
        ));
        // The engine reports failure as x == -1.
        (found[0] >= 0.0).then_some(Vec3 { x: found[0], y: found[1], z: found[2] })
    }

    /// Issues a command; `Err` carries the engine's non-zero result code.
    /// [`Command::Build`] must already have a concrete position in `site.near`.
    pub fn issue(&self, command: &Command) -> Result<(), i32> {
        let options = |queue: bool| if queue { sys::UNIT_COMMAND_OPTION_SHIFT_KEY as i16 } else { 0 };
        const NO_GROUP: c_int = -1;
        const NO_TIMEOUT: c_int = c_int::MAX;
        match *command {
            Command::Build { unit, def, site, queue } => {
                let mut pos = site.map_or([0.0; 3], |s| [s.near.x, s.near.y, s.near.z]);
                self.handle(sys::COMMAND_UNIT_BUILD, &mut sys::SBuildUnitCommand {
                    unitId: unit.0,
                    groupId: NO_GROUP,
                    // For factories the shift bit means "build five" (FactoryCAI.cpp), not "append".
                    options: options(queue && site.is_some()),
                    timeOut: NO_TIMEOUT,
                    toBuildUnitDefId: def.0,
                    buildPos_posF3: pos.as_mut_ptr(),
                    facing: sys::UNIT_COMMAND_BUILD_NO_FACING,
                })
            }
            Command::Move { unit, to, queue } => {
                let mut pos = [to.x, to.y, to.z];
                self.handle(sys::COMMAND_UNIT_MOVE, &mut sys::SMoveUnitCommand {
                    unitId: unit.0,
                    groupId: NO_GROUP,
                    options: options(queue),
                    timeOut: NO_TIMEOUT,
                    toPos_posF3: pos.as_mut_ptr(),
                })
            }
            Command::Fight { unit, to, queue } => {
                let mut pos = [to.x, to.y, to.z];
                self.handle(sys::COMMAND_UNIT_FIGHT, &mut sys::SFightUnitCommand {
                    unitId: unit.0,
                    groupId: NO_GROUP,
                    options: options(queue),
                    timeOut: NO_TIMEOUT,
                    toPos_posF3: pos.as_mut_ptr(),
                })
            }
            Command::Stop { unit } => self.handle(sys::COMMAND_UNIT_STOP, &mut sys::SStopUnitCommand {
                unitId: unit.0,
                groupId: NO_GROUP,
                options: 0,
                timeOut: NO_TIMEOUT,
            }),
            Command::SetRepeat { unit, repeat } => {
                self.handle(sys::COMMAND_UNIT_SET_REPEAT, &mut sys::SSetRepeatUnitCommand {
                    unitId: unit.0,
                    groupId: NO_GROUP,
                    options: 0,
                    timeOut: NO_TIMEOUT,
                    repeat,
                })
            }
        }
    }

    fn handle<T>(&self, topic: sys::CommandTopic, data: &mut T) -> Result<(), i32> {
        const NO_COMMAND_ID: c_int = -1;
        let code = call!(self, Engine_handleCommand(
            sys::COMMAND_TO_ID_ENGINE, NO_COMMAND_ID, topic as c_int, std::ptr::from_mut(data).cast::<c_void>()
        ));
        if code == 0 { Ok(()) } else { Err(code) }
    }

    fn string(&self, ptr: *const std::ffi::c_char) -> String {
        if ptr.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
    }
}
