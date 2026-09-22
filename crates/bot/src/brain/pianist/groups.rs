//! Soldier groups: the pianist's unit of command. A new soldier joins the group standing nearest it or forms one of
//! its own; a group carries one task at a time; between calls the standing orders are kept up (a march arrives
//! together, an engagement follows its party, an arrival becomes a hold).

use std::collections::HashSet;

use bot_protocol::{Command, OwnUnit, Tick, UnitId, Vec3};

use super::super::roster::Kit;
use super::super::{Brain, FRAMES_PER_SECOND};

/// H-HANDS-GROUPS: a new soldier joins the largest group whose centre is this close, else forms a new one; two holding
/// groups whose centres are this close merge, the smaller into the larger (smoke-5: forty one-unit groups round home).
const ADOPT_RADIUS: f32 = 400.0;
/// Farther than that, a newcomer joins the largest group within this that stands in our half, walking to it: fresh
/// Grunts at the lab formed groups of one and two while the ball held 400 to 2,700 away, and died in ones and twos
/// (human-8: eight Grunts lost by 3:14, every one in a group of five or fewer, 380 to 1,231 from the commander).
const ADOPT_FAR: f32 = 1500.0;
const MERGE_RADIUS: f32 = 300.0;
/// A moving group has arrived when its centre is this close to its destination.
const ARRIVED: f32 = 300.0;
/// An engaged group is sent on when its party has moved this far, and no more often than this.
const FOLLOW_DISTANCE: f32 = 150.0;
const FOLLOW_FRAMES: i32 = 2 * FRAMES_PER_SECOND;
/// A party nobody has seen for this long is gone; the group holds where it stands.
const LOST_FRAMES: i32 = 6 * FRAMES_PER_SECOND;
/// H-HANDS-STALL: a moving group whose centre has not come this much closer to its goal in this long is stalled; the
/// picture says so, and an advancing one wakes the player once (pianist-player-2: the ball stood five minutes short
/// of the enemy base with the player reading "under fire" and nothing about the hands).
const PROGRESS_STEP: f32 = 60.0;
const STALL_FRAMES: i32 = 45 * FRAMES_PER_SECOND;
/// An advance that has not got nearer its goal for this long is given up: the group holds and the player is told
/// (pianist-player-7: the ball answered `continue` for 151 s short of an islet spot, then for 103 s short of a mark
/// on the shore, while the player rewrote the packet four times).
const GIVE_UP_FRAMES: i32 = 90 * FRAMES_PER_SECOND;

#[derive(Clone, Debug)]
pub(crate) enum GroupTask {
    /// `committed`: the hold an advance arrives in, still committed to everything there (H-HANDS-GROUPS).
    Hold { since: i32, committed: bool },
    Move { to: Vec3, place: String, fight: bool, since: i32 },
    Engage { party: Vec<UnitId>, at: Vec3, since: i32, last_seen: i32 },
}

impl GroupTask {
    pub(crate) fn busy(&self) -> bool {
        !matches!(self, GroupTask::Hold { .. })
    }
}

#[derive(Debug)]
pub(crate) struct Group {
    pub name: String,
    pub members: Vec<UnitId>,
    pub task: GroupTask,
    /// H-ARMY-MARCH's memory: who is waiting for the body.
    pub held: HashSet<UnitId>,
    pub last_order: i32,
    /// Whether an enemy party stood within reach at the last look: a new one is a reason to ask at once.
    pub enemies_near: bool,
    /// The nearest the centre has been to a moving task's goal, and when it last got nearer (H-HANDS-STALL).
    pub best_to_go: f32,
    pub progressed: i32,
    pub stall_warned: bool,
}

impl Group {
    pub(crate) fn new(name: String, members: Vec<UnitId>, task: GroupTask, frame: i32) -> Group {
        Group { name, members, task, held: HashSet::new(), last_order: frame, enemies_near: false, best_to_go: f32::INFINITY, progressed: frame, stall_warned: false }
    }

    /// Game seconds since a moving group last got nearer its goal; `None` when it is not moving.
    pub(crate) fn stalled_seconds(&self, frame: i32) -> Option<i32> {
        matches!(self.task, GroupTask::Move { .. }).then(|| (frame - self.progressed) / FRAMES_PER_SECOND)
    }

    pub(crate) fn units<'a>(&self, own: &'a [OwnUnit]) -> Vec<&'a OwnUnit> {
        own.iter().filter(|u| self.members.contains(&u.id)).collect()
    }
}

impl Group {
    /// A new task starts the progress clock afresh.
    pub(crate) fn set_task(&mut self, task: GroupTask, frame: i32) {
        self.task = task;
        self.held.clear();
        self.best_to_go = f32::INFINITY;
        self.progressed = frame;
        self.stall_warned = false;
    }
}

pub(crate) fn centre_of(units: &[&OwnUnit]) -> Option<Vec3> {
    if units.is_empty() {
        return None;
    }
    let n = units.len() as f32;
    Some(units.iter().fold(Vec3::default(), |sum, u| Vec3 { x: sum.x + u.pos.x / n, y: 0.0, z: sum.z + u.pos.z / n }))
}

impl Brain {
    /// Every think: membership against what stands, new soldiers adopted, standing orders kept up.
    pub(super) fn keep_groups(&mut self, tick: &Tick, _kit: &Kit, commands: &mut Vec<Command>) {
        let Some(kit) = self.kit else { return };
        let own = &tick.snapshot.own_units;
        let soldiers: Vec<&OwnUnit> = own.iter().filter(|u| !u.being_built && self.is_army(u, &kit)).collect();
        let enemies = &tick.snapshot.enemies;
        let frame = tick.frame;
        let home = self.home;
        let Some(mut pianist) = self.pianist.take() else { return };
        for group in &mut pianist.groups {
            group.members.retain(|id| soldiers.iter().any(|u| u.id == *id));
        }
        pianist.groups.retain(|g| !g.members.is_empty());
        // H-HANDS-GROUPS: newcomers.
        let mut loose: Vec<&OwnUnit> = soldiers.iter().copied().filter(|u| !pianist.groups.iter().any(|g| g.members.contains(&u.id))).collect();
        loose.sort_by_key(|u| u.id.0);
        let enemy_base = self.enemy_base(home);
        for unit in loose {
            let candidates: Vec<(f32, Vec3, usize)> = pianist.groups.iter().enumerate().filter_map(|(i, g)| centre_of(&g.units(own)).map(|c| (c.dist2d(unit.pos), c, i))).collect();
            let near = candidates.iter().filter(|(d, _, _)| *d < ADOPT_RADIUS).max_by(|a, b| pianist.groups[a.2].members.len().cmp(&pianist.groups[b.2].members.len()).then(b.0.total_cmp(&a.0)));
            let far = candidates
                .iter()
                .filter(|(d, c, _)| *d < ADOPT_FAR && c.dist2d(home) < c.dist2d(enemy_base))
                .max_by(|a, b| pianist.groups[a.2].members.len().cmp(&pianist.groups[b.2].members.len()).then(b.0.total_cmp(&a.0)));
            match (near, far) {
                (Some((_, _, i)), _) => pianist.groups[*i].members.push(unit.id),
                (None, Some((_, c, i))) => {
                    pianist.groups[*i].members.push(unit.id);
                    commands.push(Command::Move { unit: unit.id, to: *c, queue: false });
                }
                (None, None) => {
                    let name = pianist.new_group_name();
                    pianist.groups.push(Group::new(name, vec![unit.id], GroupTask::Hold { since: frame, committed: false }, frame));
                }
            }
        }
        // Holding groups standing together are one group.
        let mut merged = true;
        while merged {
            merged = false;
            let centres: Vec<Option<Vec3>> = pianist.groups.iter().map(|g| centre_of(&g.units(own))).collect();
            let pair = (0..pianist.groups.len()).flat_map(|a| (0..pianist.groups.len()).map(move |b| (a, b))).find(|(a, b)| {
                a != b
                    && !pianist.groups[*a].task.busy()
                    && !pianist.groups[*b].task.busy()
                    && pianist.groups[*a].members.len() <= pianist.groups[*b].members.len()
                    && matches!((centres[*a], centres[*b]), (Some(x), Some(y)) if x.dist2d(y) < MERGE_RADIUS)
            });
            if let Some((small, large)) = pair {
                let members = std::mem::take(&mut pianist.groups[small].members);
                pianist.groups[large].members.extend(members);
                pianist.groups.remove(small);
                merged = true;
            }
        }
        // Standing orders, under each group's footwork rules (H-HANDS-LANE).
        let footwork: Vec<crate::strategist::shared::Footwork> = pianist.groups.iter().map(|g| self.footwork_of(&g.name)).collect();
        let marks: Vec<String> = self.strategist.as_ref().map(|s| s.marks.lock().unwrap().keys().cloned().collect()).unwrap_or_default();
        let is_mark_name = |place: &str| place != "home" && place != "enemy_base" && !place.starts_with("spot_") && !place.starts_with("passage_");
        let mut marches: Vec<(usize, Vec3)> = Vec::new();
        let mut stalled: Vec<String> = Vec::new();
        for (index, group) in pianist.groups.iter_mut().enumerate() {
            let units = group.units(own);
            let Some(centre) = centre_of(&units) else { continue };
            match &mut group.task {
                GroupTask::Hold { .. } => {}
                GroupTask::Move { to, fight, place, .. } => {
                    let to_go = centre.dist2d(*to);
                    if to_go < group.best_to_go - PROGRESS_STEP {
                        (group.best_to_go, group.progressed) = (to_go, frame);
                    }
                    let forgotten = is_mark_name(place) && !marks.contains(place);
                    let given_up = frame - group.progressed >= GIVE_UP_FRAMES;
                    if to_go < ARRIVED {
                        group.task = GroupTask::Hold { since: frame, committed: *fight };
                        group.held.clear();
                    } else if forgotten || given_up {
                        stalled.push(if forgotten {
                            format!("group_{} was walking to {place}, which is no longer a marked place: it holds where it is", group.name)
                        } else {
                            format!("group_{} gave up its {} to {place}: it has not got nearer for {} s, {to_go:.0} short of it, and holds where it is", group.name, if *fight { "advance" } else { "walk" }, (frame - group.progressed) / FRAMES_PER_SECOND)
                        });
                        commands.extend(units.iter().map(|u| Command::Stop { unit: u.id }));
                        group.set_task(GroupTask::Hold { since: frame, committed: false }, frame);
                    } else if *fight {
                        if frame - group.progressed >= STALL_FRAMES && !group.stall_warned {
                            group.stall_warned = true;
                            stalled.push(format!("group_{} was told to advance to {place} and has not got nearer for {} s, {to_go:.0} short of it", group.name, (frame - group.progressed) / FRAMES_PER_SECOND));
                        }
                        if footwork[index].march {
                            marches.push((index, *to));
                        }
                    }
                }
                GroupTask::Engage { party, at, last_seen, .. } => {
                    let seen: Vec<&bot_protocol::EnemyUnit> = enemies.iter().filter(|e| party.contains(&e.id)).collect();
                    if let Some(now) = centre_of_enemies(&seen) {
                        *last_seen = frame;
                        if footwork[index].follow && now.dist2d(*at) > FOLLOW_DISTANCE && frame - group.last_order >= FOLLOW_FRAMES {
                            *at = now;
                            group.last_order = frame;
                            commands.extend(units.iter().map(|u| Command::Fight { unit: u.id, to: now, queue: false }));
                        }
                    } else if frame - *last_seen > LOST_FRAMES {
                        group.task = GroupTask::Hold { since: frame, committed: false };
                    }
                }
            }
            let _ = home;
        }
        for text in stalled {
            pianist.note(frame, text.clone());
            pianist.done.push(format!("{} {text}", super::picture::clock(frame)));
            if let Some(shared) = &self.strategist {
                shared.trigger(text);
            }
        }
        self.pianist = Some(pianist);
        for (index, to) in marches {
            let mut held = std::mem::take(&mut self.pianist.as_mut().expect("pianist mode").groups[index].held);
            let members = self.pianist.as_ref().expect("pianist mode").groups[index].members.clone();
            let group: Vec<&OwnUnit> = own.iter().filter(|u| members.contains(&u.id)).collect();
            commands.extend(self.march(&mut held, &group, to, enemies.as_slice()));
            self.pianist.as_mut().expect("pianist mode").groups[index].held = held;
        }
    }
}

pub(crate) fn centre_of_enemies(units: &[&bot_protocol::EnemyUnit]) -> Option<Vec3> {
    if units.is_empty() {
        return None;
    }
    let n = units.len() as f32;
    Some(units.iter().fold(Vec3::default(), |sum, u| Vec3 { x: sum.x + u.pos.x / n, y: 0.0, z: sum.z + u.pos.z / n }))
}
