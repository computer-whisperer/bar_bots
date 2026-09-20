//! Several seats, one commander: every seat of ours publishes its own briefing and field, and what the commander is
//! shown is the merge over the live ones. One seat merges to itself.

use std::collections::BTreeMap;

use bot_protocol::{Resource, Vec3};
use serde::Serialize;

use super::shared::{Briefing, Field, Group, Place, Shared, SquadStatus};

/// A seat that has not published for this long is dead or gone.
const LIVE_FRAMES: i32 = 90;

#[derive(Clone, Default)]
pub struct SeatView {
    pub frame: i32,
    pub home: Vec3,
    pub briefing: Briefing,
    pub field: Field,
}

/// One seat in the commander's report: the economies are separate, and so is what each seat can build.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SeatLine {
    pub team: i32,
    pub home: Place,
    pub metal_income: f32,
    pub metal_stored: f32,
    pub extractors: usize,
    pub soldiers: usize,
}

impl Shared {
    pub fn publish_briefing(&self, team: i32, home: Vec3, briefing: Briefing) {
        let mut seats = self.seats.lock().unwrap();
        let seat = seats.entry(team).or_default();
        (seat.frame, seat.home, seat.briefing) = (briefing.frame, home, briefing);
    }

    pub fn publish_field(&self, team: i32, field: Field) {
        self.seats.lock().unwrap().entry(team).or_default().field = field;
    }

    /// Live seats in team order; the first is the lead.
    fn live(&self) -> Vec<(i32, SeatView)> {
        let seats = self.seats.lock().unwrap();
        let newest = seats.values().map(|s| s.frame).max().unwrap_or(0);
        seats.iter().filter(|(_, s)| newest - s.frame < LIVE_FRAMES).map(|(team, s)| (*team, s.clone())).collect()
    }

    pub fn live_seats(&self) -> Vec<i32> {
        self.live().into_iter().map(|(team, _)| team).collect()
    }

    /// The seat that asks for the commander's turns: the live seat with the lowest team number.
    pub fn lead(&self) -> Option<i32> {
        self.live_seats().first().copied()
    }

    pub fn seat_homes(&self) -> Vec<(i32, Vec3)> {
        self.live().into_iter().map(|(team, seat)| (team, seat.home)).collect()
    }

    /// The live seat whose home is nearest `pos`: the one a place-bound request goes to.
    pub fn nearest_seat(&self, pos: Vec3) -> Option<i32> {
        nearest_seat(&self.seat_homes(), pos)
    }

    pub fn briefing(&self) -> Briefing {
        let live = self.live();
        let Some((_, lead)) = live.first() else { return Briefing::default() };
        let mut merged = lead.briefing.clone();
        merged.seats = live
            .iter()
            .map(|(team, s)| SeatLine {
                team: *team,
                home: s.briefing.home.clone(),
                metal_income: s.briefing.metal.income,
                metal_stored: s.briefing.metal.current,
                extractors: s.briefing.counts.extractors,
                soldiers: s.briefing.counts.army,
            })
            .collect();
        for (_, seat) in &live[1..] {
            let b = &seat.briefing;
            merged.metal = add(merged.metal, b.metal);
            merged.energy = add(merged.energy, b.energy);
            let (c, o) = (&mut merged.counts, &b.counts);
            c.extractors += o.extractors;
            c.generators += o.generators;
            c.converters += o.converters;
            c.labs += o.labs;
            c.turrets += o.turrets;
            c.constructors += o.constructors;
            c.army += o.army;
            merged.home_group = join(&merged.home_group, &b.home_group);
            merged.attackers = join(&merged.attackers, &b.attackers);
            merged.waves_sent += b.waves_sent;
            merged.recent_events.extend(b.recent_events.iter().cloned());
        }
        // Events carry their game time in front ("12:34 ..."); what two seats both noted is said once.
        merged.recent_events.sort();
        merged.recent_events.dedup();
        merged
    }

    pub fn field(&self) -> Field {
        let live = self.live();
        let Some((_, lead)) = live.first() else { return Field::default() };
        let mut merged = lead.field.clone();
        let mut biggest_pool: usize = merged.unassigned.iter().map(|(_, n)| n).sum();
        for (_, seat) in &live[1..] {
            let f = &seat.field;
            let pool: usize = f.unassigned.iter().map(|(_, n)| n).sum();
            if pool > biggest_pool {
                (biggest_pool, merged.unassigned_centre) = (pool, f.unassigned_centre.clone());
            }
            merged.unassigned = sum_by_name(&merged.unassigned, &f.unassigned);
            for squad in &f.squads {
                match merged.squads.iter_mut().find(|s| s.name == squad.name) {
                    Some(have) => *have = join_squads(have, squad),
                    None => merged.squads.push(squad.clone()),
                }
            }
            merged.extractors.extend(f.extractors.iter().cloned());
            merged.turrets.extend(f.turrets.iter().cloned());
            for item in &f.buildable {
                if !merged.buildable.contains(item) {
                    merged.buildable.push(item.clone());
                }
            }
            merged.turret_requests_pending += f.turret_requests_pending;
            let (s, o) = (&mut merged.score, &f.score);
            s.extractors += o.extractors;
            s.extractor_peak += o.extractor_peak;
            s.seconds_since_growth = s.seconds_since_growth.min(o.seconds_since_growth);
            s.free_spots = s.free_spots.min(o.free_spots);
            // Each free spot with its walk from the nearest of our homes.
            for (n, place, walk) in &o.next_free {
                match s.next_free.iter_mut().find(|(have, ..)| have == n) {
                    Some(have) => have.2 = have.2.min(*walk),
                    None => s.next_free.push((*n, place.clone(), *walk)),
                }
            }
            s.next_free.sort_by_key(|(_, _, walk)| *walk);
            s.next_free.truncate(lead.field.score.next_free.len().max(o.next_free.len()));
            s.enemy_spots_seen = s.enemy_spots_seen.max(o.enemy_spots_seen);
            s.soldiers += o.soldiers;
            s.army_metal += o.army_metal;
            s.soldiers_near_home += o.soldiers_near_home;
            s.metal_income += o.metal_income;
            for (minutes, extractors, income, army) in &mut s.trend {
                if let Some((_, x, i, a)) = o.trend.iter().find(|(m, ..)| m == minutes) {
                    (*extractors, *income, *army) = (*extractors + x, *income + i, *army + a);
                }
            }
            s.extractors_lost_3_min += o.extractors_lost_3_min;
            s.traded_3_min = (s.traded_3_min.0 + o.traded_3_min.0, s.traded_3_min.1 + o.traded_3_min.1);
            s.traded = (s.traded.0 + o.traded.0, s.traded.1 + o.traded.1);
            s.enemy_soldiers_seen = s.enemy_soldiers_seen.max(o.enemy_soldiers_seen);
            s.enemy_soldiers_seen_metal = s.enemy_soldiers_seen_metal.max(o.enemy_soldiers_seen_metal);
        }
        merged
    }
}

pub fn nearest_seat(homes: &[(i32, Vec3)], pos: Vec3) -> Option<i32> {
    homes.iter().min_by(|a, b| a.1.dist2d(pos).total_cmp(&b.1.dist2d(pos))).map(|(team, _)| *team)
}

fn add(a: Resource, b: Resource) -> Resource {
    Resource { current: a.current + b.current, income: a.income + b.income, usage: a.usage + b.usage, storage: a.storage + b.storage }
}

fn sum_by_name(a: &[(String, usize)], b: &[(String, usize)]) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = a.iter().cloned().collect();
    for (name, n) in b {
        *counts.entry(name.clone()).or_default() += n;
    }
    counts.into_iter().collect()
}

/// Two seats' parts of one group; it stands where the bigger part does.
fn join(a: &Group, b: &Group) -> Group {
    Group {
        size: a.size + b.size,
        idle: a.idle + b.idle,
        centre: if b.size > a.size { b.centre.clone() } else { a.centre.clone().or(b.centre.clone()) },
        composition: sum_by_name(&a.composition, &b.composition),
    }
}

fn join_squads(a: &SquadStatus, b: &SquadStatus) -> SquadStatus {
    let size = |s: &SquadStatus| s.composition.iter().map(|(_, n)| n).sum::<usize>();
    let (na, nb) = (size(a), size(b));
    SquadStatus {
        name: a.name.clone(),
        composition: sum_by_name(&a.composition, &b.composition),
        health_percent: if na + nb == 0 { 0 } else { ((a.health_percent as usize * na + b.health_percent as usize * nb) / (na + nb)) as u32 },
        centre: if nb > na { b.centre.clone() } else { a.centre.clone().or(b.centre.clone()) },
        post: a.post.clone().or(b.post.clone()),
        still_wanted: a.still_wanted.clone(),
        engaged: a.engaged || b.engaged,
        remark: a.remark.clone().or(b.remark.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_seats_merge_and_a_silent_one_drops_out() {
        let shared = Shared::default();
        let briefing = |frame, army, income| {
            let mut b = Briefing { frame, ..Default::default() };
            b.counts.army = army;
            b.metal.income = income;
            b.home_group = Group { size: army, idle: 0, centre: None, composition: vec![("armpw".into(), army)] };
            b
        };
        shared.publish_briefing(4, Vec3::default(), briefing(300, 5, 10.0));
        shared.publish_briefing(2, Vec3 { x: 1000.0, y: 0.0, z: 0.0 }, briefing(300, 7, 12.0));
        let merged = shared.briefing();
        assert_eq!((shared.lead(), merged.counts.army, merged.metal.income, merged.seats.len()), (Some(2), 12, 22.0, 2));
        assert_eq!(merged.home_group.composition, vec![("armpw".to_string(), 12)]);
        assert_eq!(shared.nearest_seat(Vec3 { x: 900.0, y: 0.0, z: 0.0 }), Some(2));
        shared.publish_briefing(4, Vec3::default(), briefing(600, 5, 10.0));
        assert_eq!((shared.lead(), shared.briefing().counts.army), (Some(4), 5));
    }
}
