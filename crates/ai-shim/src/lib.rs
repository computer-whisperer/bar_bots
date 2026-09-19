//! Skirmish AI library for the Recoil engine's C AI interface.
//!
//! The engine loads this as `libSkirmishAI.so` (renamed by `run/install_ai.sh`) and calls the three exports below
//! (`rts/ExternalAI/Interface/SSkirmishAILibrary.h`). For now this only counts
//! and logs the events it receives; it issues no commands.

use std::collections::BTreeMap;
use std::ffi::{c_int, c_void};
use std::sync::Mutex;

/// Event topics from `rts/ExternalAI/Interface/AISEvents.h`, indexed by topic id.
const EVENT_NAMES: [&str; 28] = [
    "NULL", "INIT", "RELEASE", "UPDATE", "MESSAGE", "UNIT_CREATED", "UNIT_FINISHED", "UNIT_IDLE",
    "UNIT_MOVE_FAILED", "UNIT_DAMAGED", "UNIT_DESTROYED", "UNIT_GIVEN", "UNIT_CAPTURED",
    "ENEMY_ENTER_LOS", "ENEMY_LEAVE_LOS", "ENEMY_ENTER_RADAR", "ENEMY_LEAVE_RADAR",
    "ENEMY_DAMAGED", "ENEMY_DESTROYED", "WEAPON_FIRED", "PLAYER_COMMAND", "SEISMIC_PING",
    "COMMAND_FINISHED", "LOAD", "SAVE", "ENEMY_CREATED", "ENEMY_FINISHED", "LUA_MESSAGE",
];
const EVENT_UPDATE: c_int = 3;

/// `struct SUpdateEvent` from AISEvents.h.
#[repr(C)]
struct SUpdateEvent {
    frame: c_int,
}

#[derive(Default)]
struct Instance {
    counts: BTreeMap<c_int, u64>,
    last_frame: c_int,
}

/// One entry per AI instance; the engine may run several from one library.
static INSTANCES: Mutex<BTreeMap<c_int, Instance>> = Mutex::new(BTreeMap::new());

fn event_name(topic: c_int) -> &'static str {
    usize::try_from(topic).ok().and_then(|i| EVENT_NAMES.get(i)).copied().unwrap_or("UNKNOWN")
}

#[unsafe(no_mangle)]
pub extern "C" fn init(skirmish_ai_id: c_int, _callback: *const c_void) -> c_int {
    eprintln!("[bar_bots ai={skirmish_ai_id}] init");
    INSTANCES.lock().unwrap().insert(skirmish_ai_id, Instance::default());
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn release(skirmish_ai_id: c_int) -> c_int {
    if let Some(instance) = INSTANCES.lock().unwrap().remove(&skirmish_ai_id) {
        let summary: Vec<String> =
            instance.counts.iter().map(|(topic, n)| format!("{}={n}", event_name(*topic))).collect();
        eprintln!(
            "[bar_bots ai={skirmish_ai_id}] release at frame {}: {}",
            instance.last_frame,
            summary.join(" ")
        );
    }
    0
}

/// # Safety
/// `data` must point to the event struct matching `topic`, as the engine guarantees.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn handleEvent(skirmish_ai_id: c_int, topic: c_int, data: *const c_void) -> c_int {
    let mut instances = INSTANCES.lock().unwrap();
    let Some(instance) = instances.get_mut(&skirmish_ai_id) else {
        return -1;
    };
    *instance.counts.entry(topic).or_default() += 1;
    if topic == EVENT_UPDATE && !data.is_null() {
        instance.last_frame = unsafe { (*data.cast::<SUpdateEvent>()).frame };
    } else {
        eprintln!("[bar_bots ai={skirmish_ai_id}] f={} {}", instance.last_frame, event_name(topic));
    }
    0
}
