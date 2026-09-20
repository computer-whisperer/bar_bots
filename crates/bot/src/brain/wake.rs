//! When to wake the field commander: it names the conditions (`wait` tool), the brain watches for them each tick
//! and holds the game while the commander takes its turn (`DESIGN.md`, "Field commander").

use std::sync::atomic::Ordering;

use bot_protocol::Tick;

use super::roster::Kit;
use super::{Brain, FRAMES_PER_SECOND};

/// Turns are at least this far apart in game time, however many conditions fire.
const MIN_GAP_FRAMES: i32 = 5 * FRAMES_PER_SECOND;
const THREAT_RADIUS: f32 = 600.0;
/// The commander is woken when our extractor count has made no new high for this long, and again this long after.
const STAGNATION_FRAMES: i32 = 4 * 60 * FRAMES_PER_SECOND;

#[derive(Default)]
pub struct WakeState {
    threatened_extractors: usize,
    squads_engaged: usize,
    pool_met: bool,
    /// The most extractors we have held, and when we first held that many.
    pub(super) extractor_peak: usize,
    pub(super) growth_frame: i32,
    last_stagnation_wake: i32,
    /// Once a minute: (frame, extractors, metal income, army metal). The commander is shown the curve, not only the level.
    pub(super) history: Vec<(i32, usize, f32, u32)>,
    /// Frames at which we lost an extractor, for the last few minutes (the brain's own list forgets after one).
    pub(super) losses: Vec<i32>,
    /// Reasons that fired while a turn could not be taken yet.
    pending: Vec<String>,
}

impl Brain {
    pub(super) fn track_growth(&mut self, tick: &Tick, kit: &Kit) {
        let extractors = tick.snapshot.own_units.iter().filter(|u| kit.is_extractor(u.def) && !u.being_built).count();
        if extractors > self.wake.extractor_peak {
            self.wake.extractor_peak = extractors;
            self.wake.growth_frame = tick.frame;
        }
        let lost_now = self.extractor_losses.iter().filter(|f| **f == tick.frame).count();
        self.wake.losses.extend(std::iter::repeat_n(tick.frame, lost_now));
        self.wake.losses.retain(|f| tick.frame - f < 3 * 60 * FRAMES_PER_SECOND);
        if self.wake.history.last().is_none_or(|(frame, ..)| tick.frame - frame >= 60 * FRAMES_PER_SECOND) {
            let army: f32 = tick.snapshot.own_units.iter().filter(|u| self.is_army(u, kit)).filter_map(|u| self.world.def(u.def)).map(|d| d.metal_cost).sum();
            self.wake.history.push((tick.frame, extractors, tick.snapshot.metal.income, army as u32));
        }
    }

    /// The history sample nearest to `minutes` ago, if the game is that old: (extractors, metal income, army metal).
    pub(super) fn minutes_ago(&self, frame: i32, minutes: i32) -> Option<(usize, f32, u32)> {
        let then = frame - minutes * 60 * FRAMES_PER_SECOND;
        if then < 0 {
            return None;
        }
        self.wake.history.iter().min_by_key(|(f, ..)| (f - then).abs()).map(|(_, x, income, army)| (*x, *income, *army))
    }

    pub(super) fn wake_commander_if_due(&mut self, tick: &Tick, kit: &Kit) {
        let Some(shared) = self.strategist.clone() else { return };
        if !shared.lockstep.load(Ordering::Relaxed) {
            return;
        }
        // Of several seats under one commander only the lead asks for turns (held, it holds the engine and so every
        // seat); the others hand it what is theirs alone to know.
        if shared.lead().is_some_and(|lead| lead != self.world.hello.team) {
            if shared.wake.lock().unwrap().extractor_lost && self.extractor_losses.back() == Some(&tick.frame) {
                shared.trigger("an extractor was destroyed".into());
            }
            return;
        }
        let last_turn_frame = shared.last_turn_frame.load(Ordering::Relaxed);
        // A commander still "thinking" (its last orders not yet in force) cannot be asked again; what happens meanwhile
        // is kept for its next turn.
        let busy = shared.apply_delayed(tick.frame);
        let wake = shared.wake.lock().unwrap().clone();
        let field = shared.field();
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
        // Not the commander's to switch off: every other condition is a threat, and a commander woken only by threats
        // defends four extractors for half an hour.
        let stagnant = tick.frame - self.wake.growth_frame.max(self.wake.last_stagnation_wake);
        if stagnant >= STAGNATION_FRAMES && field.score.free_spots > 0 {
            self.wake.last_stagnation_wake = tick.frame;
            reasons.push(format!(
                "no growth: we have not held more than {} extractors for {} min, with {} free spots we can walk to",
                self.wake.extractor_peak,
                (tick.frame - self.wake.growth_frame) / (60 * FRAMES_PER_SECOND),
                field.score.free_spots
            ));
        }

        let since = tick.frame - last_turn_frame;
        if reasons.is_empty() && since >= wake.max_seconds as i32 * FRAMES_PER_SECOND {
            reasons.push(format!("{} s have passed", since / FRAMES_PER_SECOND));
        }
        let first_turn = last_turn_frame == 0 && tick.snapshot.own_units.iter().any(|u| u.def == kit.lab);
        if first_turn {
            reasons.push("our first factory is up".into());
        }
        let too_soon = if last_turn_frame == 0 { !first_turn } else { since < MIN_GAP_FRAMES };
        if reasons.is_empty() || too_soon || busy {
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
        shared.last_turn_frame.store(tick.frame, Ordering::Relaxed);
        shared.hold_for_turn(reasons.join("; "), tick.frame);
    }
}
