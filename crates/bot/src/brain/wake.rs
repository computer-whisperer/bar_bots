//! When to wake the field commander: it names the conditions (`wait` tool), the brain watches for them each tick
//! and holds the game while the commander takes its turn (`DESIGN.md`, "Field commander").

use std::sync::atomic::Ordering;

use bot_protocol::Tick;

use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};

/// Turns are at least this far apart in game time, however many conditions fire.
const MIN_GAP_FRAMES: i32 = 5 * FRAMES_PER_SECOND;
const THREAT_RADIUS: f32 = 600.0;

#[derive(Default)]
pub struct WakeState {
    last_turn_frame: i32,
    threatened_extractors: usize,
    squads_engaged: usize,
    pool_met: bool,
    /// Reasons that fired while a turn could not be taken yet.
    pending: Vec<String>,
}

impl Brain {
    pub(super) fn wake_commander_if_due(&mut self, tick: &Tick, kit: &Kit) {
        let Some(shared) = self.strategist.clone() else { return };
        if !shared.lockstep.load(Ordering::Relaxed) {
            return;
        }
        let wake = shared.wake.lock().unwrap().clone();
        let field = shared.field.lock().unwrap().clone();
        let mut reasons: Vec<String> = std::mem::take(&mut self.wake.pending);
        reasons.append(&mut shared.triggers.lock().unwrap());

        // Conditions wake on their rising edge: a raid is news when it starts, not every tick it lasts.
        let threatened: Vec<&str> =
            field.extractors.iter().filter(|x| x.enemies_within_600 > 0).map(|x| x.at.grid.as_str()).collect();
        if wake.enemy_near_extractor && threatened.len() > self.wake.threatened_extractors {
            reasons.push(format!("enemies within {THREAT_RADIUS:.0} of our extractors at {}", threatened.join(", ")));
        }
        self.wake.threatened_extractors = threatened.len();
        let engaged: Vec<&str> = field.squads.iter().filter(|s| s.engaged).map(|s| s.name.as_str()).collect();
        if wake.squad_engaged && engaged.len() > self.wake.squads_engaged {
            reasons.push(format!("squad engaged: {}", engaged.join(", ")));
        }
        self.wake.squads_engaged = engaged.len();
        if wake.extractor_lost && self.extractor_losses.back() == Some(&tick.frame) {
            reasons.push("an extractor was destroyed".into());
        }
        let pool_met = !wake.pool_reaches.is_empty()
            && wake.pool_reaches.iter().all(|(name, n)| field.unassigned.iter().any(|(have, count)| have == name && count >= n));
        if pool_met && !self.wake.pool_met {
            reasons.push("the unassigned soldiers you were waiting for are ready".into());
        }
        self.wake.pool_met = pool_met;

        let since = tick.frame - self.wake.last_turn_frame;
        if reasons.is_empty() && since >= wake.max_seconds as i32 * FRAMES_PER_SECOND {
            reasons.push(format!("{} s have passed", since / FRAMES_PER_SECOND));
        }
        let first_turn = self.wake.last_turn_frame == 0 && tick.snapshot.own_units.iter().any(|u| u.def == kit.lab);
        if first_turn {
            reasons.push("our first factory is up".into());
        }
        let too_soon = if self.wake.last_turn_frame == 0 { !first_turn } else { since < MIN_GAP_FRAMES };
        if reasons.is_empty() || too_soon {
            // Keep the newest word on each subject: "enemies within ... at C3" then "... at C3, C4" is one piece of news.
            let subject = |r: &String| r.split(|c: char| c == ':' || c.is_ascii_digit()).next().unwrap_or_default().to_string();
            let mut kept: Vec<String> = Vec::new();
            for reason in reasons.into_iter().rev() {
                if !kept.iter().any(|k| subject(k) == subject(&reason)) {
                    kept.push(reason);
                }
            }
            kept.reverse();
            let reasons = kept;
            self.wake.pending = reasons;
            return;
        }
        self.wake.last_turn_frame = tick.frame;
        shared.hold_for_turn(reasons.join("; "));
    }
}
