//! What the brain decided, in structured form, for the match record (`docs/harness/record-format.md`).
//!
//! The brain only notes; `main` drains the journal after every tick and the recorder writes it out. Notes are
//! cheap (a counter bump, or one small value per wave) so they are taken whether or not a record is being written.

use std::collections::BTreeMap;

use bot_protocol::{UnitId, Vec3};
use serde_json::{Value, json};

use super::Brain;

/// One decision worth a line of its own: a wave launched, a recall, a note for the strategist.
pub struct Note {
    /// Which layer decided: `heuristic` today; an LLM's or Jev's decisions made inside the bot would name themselves.
    pub source: &'static str,
    pub frame: i32,
    pub kind: &'static str,
    /// What the decision was made on; `Null` when the text says it all.
    pub inputs: Value,
    pub outputs: Value,
}

/// Where the army stands in the brain's mind this tick.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Intent {
    pub home: Vec3,
    pub enemy_start: Vec3,
    pub station: Vec3,
    pub target: Option<Vec3>,
    pub staging: Option<Vec3>,
}

#[derive(Default)]
pub struct Journal {
    /// Heuristic firings (docs/heuristics.md) since the last drain.
    pub rules: BTreeMap<&'static str, u32>,
    pub notes: Vec<Note>,
    pub intent: Intent,
}

impl Journal {
    pub fn rule(&mut self, rule: &'static str) {
        *self.rules.entry(rule).or_default() += 1;
    }

    /// A decision of the heuristic brain.
    pub fn note(&mut self, frame: i32, kind: &'static str, inputs: Value, outputs: Value) {
        self.notes.push(Note { source: "heuristic", frame, kind, inputs, outputs });
    }
}

/// How a unit is employed, as bits in a sample's unit flags.
pub const ROLE_ATTACKER: u8 = 1;
pub const ROLE_SQUAD: u8 = 2;

impl Brain {
    /// Everything noted since the last call; the intent is carried over.
    pub fn take_journal(&mut self) -> Journal {
        let intent = self.journal.intent;
        std::mem::replace(&mut self.journal, Journal { intent, ..Journal::default() })
    }

    pub fn role(&self, unit: UnitId) -> u8 {
        if self.army.is_attacker(unit) {
            ROLE_ATTACKER
        } else if self.squads.contains(unit) {
            ROLE_SQUAD
        } else {
            0
        }
    }

    pub(super) fn journal_intent(&mut self) {
        self.journal.intent = Intent {
            home: self.home,
            enemy_start: self.enemy_base(self.home),
            station: self.last_station,
            target: self.army.target(),
            staging: self.army.staging_point(),
        };
    }

    pub(super) fn journal_wave(&mut self, frame: i32, number: usize, units: usize, target: Vec3, first_stop: Vec3) {
        let at = |p: Vec3| json!({ "grid": self.world.grid(p), "x": p.x as i32, "z": p.z as i32 });
        let (inputs, outputs) = (json!({ "home_group": units }), json!({ "wave": number, "target": at(target), "first_stop": at(first_stop) }));
        self.journal.note(frame, "wave", inputs, outputs);
    }
}
