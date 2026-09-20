//! The fight itself, frame by frame at the engine's 30 Hz.
//!
//! The model is deliberately thin: positions in 2D, walk until the target is inside the longest weapon's range and
//! then stand and shoot, nearest visible enemy, salvos on the engine's own reload clock, projectiles that take time
//! to arrive and can land beside a target that has moved, area damage with the engine's own falloff, and ground a
//! unit cannot step onto because another unit is on it, so a front can only be so wide. Everything a duel asked
//! for is in here; nothing else is, and `docs/studies/combat-sim.md` lists what that costs.

use crate::field::Flow;
use crate::rng::Rng;
use crate::scenario::{End, Odds, Outcome, Scenario, Vec2};
use crate::units::{FPS, Units, aim_error};

const NONE: u32 = u32::MAX;
/// Targets are picked twice a second, as the engine re-chooses on its own slow clock rather than every frame.
const RETARGET: u32 = 15;
/// Side of a bucket in the neighbour grid, in elmos: wider than any area of effect we care about.
const BUCKET: f32 = 64.0;

/// The simulation's own assumptions, as opposed to the game's numbers. Each one is a knob the validation moved.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    /// Multiplies each weapon's aim cone after the engine's own conversion (`units::aim_error`). 1.0 is the
    /// engine's number; it is here so that a future miss can be blamed on the cone rather than assumed away.
    pub spread: f32,
    /// A unit closes to this share of its range before it stops: the engine's own `maxRange * 0.9`
    /// (`MobileCAI.cpp` `ExecuteAttack`).
    pub stop_at: f32,
    /// Seconds between acquiring a target and the first shot: turret slew and the engine's aiming tolerance.
    /// Zero by default — the duel tables do not ask for one, and every value tried made agreement worse.
    pub aim_seconds: f32,
    /// Units take up ground and cannot stand in each other, at the radius the engine pushes them apart with
    /// (`CGroundMoveType` uses `unit->radius`, the collision volume, which is wider than the build footprint):
    /// the front rank halts at its range and the ranks behind it are stuck there. Off for measuring what it costs.
    pub collide: bool,
}

impl Default for Tuning {
    fn default() -> Tuning {
        Tuning { spread: 1.0, stop_at: 0.9, aim_seconds: 0.0, collide: true }
    }
}

/// A blocked unit tries these deflections either side of where it wanted to go before giving up for the frame;
/// without them a blob jams solid instead of flowing around its own front rank.
const DEFLECTIONS: [f32; 7] = [0.0, 0.5, -0.5, 1.0, -1.0, 1.7, -1.7];

/// One weapon of one unit type, with everything the loop needs already in frames, elmos and radians.
struct Shot {
    range: f32,
    reload: u32,
    burst: u32,
    burst_rate: u32,
    projectiles: u32,
    aoe: f32,
    edge: f32,
    /// Aim cones, as a sine: `salvo` is drawn once per salvo, `spray` once per projectile.
    salvo_spread: f32,
    spray_spread: f32,
    /// How much of the target's speed the shot leads by; see `Weapon::predict_boost`.
    predict_boost: f32,
    lead_limit: f32,
    tracks: bool,
    instant: bool,
    /// Flight time in frames for a given distance, from the weapon's velocity curve.
    velocity: f32,
    start_velocity: f32,
    acceleration: f32,
    energy: f32,
    /// Damage against each unit type, armour class already applied; 0 where the weapon cannot hurt it.
    damage: Vec<f32>,
}

/// Unit types boiled down to what the loop reads, built once and reused by every query.
pub struct Rules {
    pub units: Units,
    pub tuning: Tuning,
    shots: Vec<Vec<Shot>>,
}

impl Default for Rules {
    fn default() -> Rules {
        Rules::new(Units::default(), Tuning::default())
    }
}

impl Rules {
    pub fn new(units: Units, tuning: Tuning) -> Rules {
        let shots = (units.list.iter())
            .map(|unit| {
                (unit.weapons.iter())
                    .filter(|w| w.hits_ground())
                    .map(|w| Shot {
                        range: w.range,
                        reload: (w.reload * FPS).round().max(1.0) as u32,
                        burst: w.burst.max(1),
                        burst_rate: (w.burst_rate * FPS).round().max(1.0) as u32,
                        projectiles: w.projectiles.max(1),
                        aoe: w.aoe.max(1.0),
                        edge: w.edge.clamp(0.0, 1.0),
                        salvo_spread: aim_error(w.accuracy) * tuning.spread,
                        spray_spread: aim_error(w.spray) * tuning.spread,
                        predict_boost: w.predict_boost.clamp(0.0, 1.0),
                        lead_limit: w.lead_limit,
                        tracks: w.tracks,
                        instant: w.kind.instant(),
                        velocity: w.velocity,
                        start_velocity: w.start_velocity,
                        acceleration: w.acceleration,
                        energy: w.energy_per_shot,
                        damage: units.list.iter().map(|target| w.damage_to(&target.armor)).collect(),
                    })
                    .collect()
            })
            .collect();
        Rules { units, tuning, shots }
    }
}

impl Shot {
    fn flight_frames(&self, distance: f32) -> u32 {
        if self.instant || self.velocity <= 0.0 {
            return 0;
        }
        let seconds = if self.acceleration > 0.0 && self.start_velocity > 0.0 && self.start_velocity < self.velocity {
            let ramp = (self.velocity - self.start_velocity) / self.acceleration;
            let covered = (self.start_velocity + self.velocity) / 2.0 * ramp;
            if distance > covered {
                ramp + (distance - covered) / self.velocity
            } else {
                let a = self.acceleration;
                ((self.start_velocity * self.start_velocity + 2.0 * a * distance).sqrt() - self.start_velocity) / a
            }
        } else {
            distance / self.velocity
        };
        (seconds * FPS).round() as u32
    }
}

struct Body {
    def: u32,
    side: u8,
    alive: bool,
    hold: bool,
    spawn: u32,
    pos: Vec2,
    /// Elmos per frame, last frame's step: what a shooter leads the target by.
    vel: Vec2,
    hp: f32,
    max_hp: f32,
    radius: f32,
    speed: f32,
    sight: f32,
    metal: f32,
    reach: f32,
    target: u32,
    /// Frame this unit's weapons may first fire at its current target.
    aimed_at: u32,
}

impl Body {
    /// Alive and already on the field: a group with an arrival delay is counted as the side's value from the
    /// start but is neither shot at nor in the way until it turns up.
    fn here(&self, frame: u32) -> bool {
        self.alive && frame >= self.spawn
    }
}

struct Gun {
    body: u32,
    shot: u32,
    ready: u32,
    salvo: u32,
    next: u32,
    /// How badly this weapon is currently misjudging target speed, redrawn twice a second as the engine does.
    predict_mod: f32,
    /// The aim error the whole salvo shares, as a fraction of the distance to the target.
    salvo_error: Vec2,
}

enum Aim {
    /// Guided or hitscan: wherever the target is when it lands.
    At(u32),
    /// Unguided: a fixed point on the ground, which the target may have left.
    Point(Vec2),
}

struct Impact {
    frame: u32,
    from: u32,
    def: u32,
    shot: u32,
    aim: Aim,
}

/// Run one fight. The same seed on the same scenario always gives the same outcome.
pub fn simulate(rules: &Rules, scenario: &Scenario, seed: u64) -> Outcome {
    Sim::new(rules, scenario, seed).run()
}

/// `reps` fights, seeds 0..reps, summarised. Side 0's point of view.
pub fn odds(rules: &Rules, scenario: &Scenario, reps: u32) -> Odds {
    let mut odds = Odds::default();
    for seed in 0..reps.max(1) {
        let outcome = simulate(rules, scenario, seed as u64);
        match outcome.winner {
            Some(0) => odds.wins += 1,
            Some(_) => odds.losses += 1,
            None => odds.draws += 1,
        }
        odds.mean_margin += outcome.margin;
        odds.mean_seconds += outcome.seconds;
    }
    let reps = reps.max(1) as f32;
    odds.mean_margin /= reps;
    odds.mean_seconds /= reps;
    odds
}

struct Sim<'a> {
    rules: &'a Rules,
    scenario: &'a Scenario,
    rng: Rng,
    bodies: Vec<Body>,
    guns: Vec<Gun>,
    impacts: Vec<Impact>,
    flows: [Option<Flow>; 2],
    /// Enemy centre each side walks at until it can see something to shoot.
    goal: [Vec2; 2],
    /// Whether a body can be seen by the other side, refreshed with targeting.
    seen: Vec<bool>,
    buckets: Vec<Vec<u32>>,
    grid: (i32, i32, i32, i32),
    widest_radius: f32,
    /// Energy in store per side, and the cap it recharges towards.
    energy: [f32; 2],
    energy_cap: [f32; 2],
    metal: [f32; 2],
}

impl<'a> Sim<'a> {
    fn new(rules: &'a Rules, scenario: &'a Scenario, seed: u64) -> Sim<'a> {
        let mut bodies = Vec::new();
        let mut guns = Vec::new();
        let mut metal = [0.0; 2];
        for (side, groups) in scenario.sides.iter().enumerate() {
            for group in groups {
                let unit = &rules.units.list[group.def];
                metal[side] += unit.metal * group.count as f32;
                for place in group.places() {
                    let body = bodies.len() as u32;
                    for shot in 0..rules.shots[group.def].len() {
                        guns.push(Gun { body, shot: shot as u32, ready: 0, salvo: 0, next: 0, predict_mod: 1.0, salvo_error: Vec2::default() });
                    }
                    bodies.push(Body {
                        def: group.def as u32,
                        side: side as u8,
                        alive: true,
                        hold: group.hold || !unit.mobile(),
                        spawn: (group.delay * FPS) as u32,
                        pos: place,
                        vel: Vec2::default(),
                        hp: unit.health,
                        max_hp: unit.health,
                        radius: unit.radius,
                        speed: unit.speed / FPS,
                        sight: unit.sight,
                        metal: unit.metal,
                        reach: unit.reach(),
                        target: NONE,
                        aimed_at: 0,
                    });
                }
            }
        }
        let goal = [centre(&bodies, 1, u32::MAX), centre(&bodies, 0, u32::MAX)];
        let flows = [0, 1].map(|side| {
            let field = scenario.terrain.as_ref()?;
            // One field per side, for the least agile unit on it: a choke that stops the tanks stops the group.
            let mobile = || scenario.sides[side].iter().map(|g| &rules.units.list[g.def]).filter(|u| u.mobile());
            let slope = mobile().map(|u| u.max_slope).min()?;
            let depth = mobile().map(|u| u.max_depth).fold(f32::MAX, f32::min);
            Some(field.flow(field.passable(slope, depth), goal[side]))
        });
        let seen = vec![false; bodies.len()];
        let widest_radius = bodies.iter().map(|b| b.radius).fold(0.0, f32::max);
        Sim {
            rules,
            scenario,
            rng: Rng::new(seed),
            bodies,
            guns,
            impacts: Vec::new(),
            flows,
            goal,
            seen,
            buckets: Vec::new(),
            grid: (0, 0, 0, 0),
            widest_radius,
            energy: [0, 1].map(|s| scenario.energy[s].stored),
            energy_cap: [0, 1].map(|s| scenario.energy[s].stored),
            metal,
        }
    }

    fn run(mut self) -> Outcome {
        let limit = (self.scenario.time_limit * FPS) as u32;
        let stalemate = (self.scenario.stalemate * FPS) as u32;
        let mut last_damage = 0;
        let mut contact = None;
        let mut timeline = Vec::new();
        let mut frame = 0;
        let reason = loop {
            // The grid carries collision as well as damage, so it has to be current every frame.
            self.rebuild_grid(frame);
            if frame % RETARGET == 0 {
                // The engine redraws how far each weapon misjudges target speed on the same slow clock.
                for gun in 0..self.guns.len() {
                    self.guns[gun].predict_mod = self.rng.unit() * 2.0;
                }
                self.retarget(frame);
                self.goal = [centre(&self.bodies, 1, frame), centre(&self.bodies, 0, frame)];
            }
            if frame % (5 * FPS as u32) == 0 {
                timeline.push(self.value_left());
            }
            for side in 0..2 {
                self.energy[side] = (self.energy[side] + self.scenario.energy[side].income / FPS).min(self.energy_cap[side]);
            }
            self.advance(frame);
            self.fire(frame);
            if self.resolve(frame) {
                contact.get_or_insert(frame);
                last_damage = frame;
            }
            frame += 1;
            let left = [self.alive(0), self.alive(1)];
            if left[0] == 0 || left[1] == 0 {
                break End::Wiped;
            }
            if frame >= limit {
                break End::Timeout;
            }
            if frame - last_damage >= stalemate {
                break End::Stalemate;
            }
        };
        let value_left = self.value_left();
        let margin = value_left[0] - value_left[1];
        let mut survivors = [Vec::new(), Vec::new()];
        let mut metal_lost = self.metal;
        for body in self.bodies.iter().filter(|b| b.alive) {
            let counts = &mut survivors[body.side as usize];
            match counts.iter_mut().find(|(def, _)| *def == body.def as usize) {
                Some((_, n)) => *n += 1,
                None => counts.push((body.def as usize, 1)),
            }
            metal_lost[body.side as usize] -= body.metal;
        }
        for counts in &mut survivors {
            counts.sort();
        }
        timeline.push(value_left);
        Outcome {
            winner: if margin.abs() < 1e-3 { None } else { Some(usize::from(margin < 0.0)) },
            reason,
            seconds: frame as f32 / FPS,
            contact_seconds: contact.map(|f| f as f32 / FPS),
            survivors,
            metal_lost,
            value_left,
            margin,
            timeline,
        }
    }

    fn alive(&self, side: usize) -> usize {
        self.bodies.iter().filter(|b| b.alive && b.side as usize == side).count()
    }

    fn value_left(&self) -> [f32; 2] {
        let mut left = [0.0; 2];
        for body in self.bodies.iter().filter(|b| b.alive) {
            left[body.side as usize] += body.metal * (body.hp / body.max_hp).clamp(0.0, 1.0);
        }
        [0, 1].map(|s| if self.metal[s] > 0.0 { left[s] / self.metal[s] } else { 0.0 })
    }

    /// Neighbour buckets over the units that are here now: reinforcements still on their way take up no ground
    /// and cannot be shot at. Damage, crowding and targeting all read these.
    fn rebuild_grid(&mut self, frame: u32) {
        let live = self.bodies.iter().filter(|b| b.here(frame));
        let (mut x0, mut z0, mut x1, mut z1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for body in live {
            (x0, z0) = (x0.min(body.pos.x), z0.min(body.pos.z));
            (x1, z1) = (x1.max(body.pos.x), z1.max(body.pos.z));
        }
        if x0 > x1 {
            (x0, z0, x1, z1) = (0.0, 0.0, 0.0, 0.0);
        }
        // A margin so a unit that walks out of the box before the next rebuild still lands in an edge bucket.
        let (ox, oz) = ((x0 / BUCKET).floor() as i32 - 2, (z0 / BUCKET).floor() as i32 - 2);
        let (w, h) = (((x1 - x0) / BUCKET) as i32 + 5, ((z1 - z0) / BUCKET) as i32 + 5);
        self.grid = (ox, oz, w, h);
        self.buckets.resize((w * h) as usize, Vec::new());
        for bucket in &mut self.buckets {
            bucket.clear();
        }
        for i in 0..self.bodies.len() {
            if self.bodies[i].here(frame) {
                let cell = self.bucket_of(self.bodies[i].pos);
                self.buckets[cell].push(i as u32);
            }
        }
    }

    fn bucket_of(&self, at: Vec2) -> usize {
        let (ox, oz, w, h) = self.grid;
        let x = ((at.x / BUCKET).floor() as i32 - ox).clamp(0, w - 1);
        let z = ((at.z / BUCKET).floor() as i32 - oz).clamp(0, h - 1);
        (z * w + x) as usize
    }

    /// Every living unit within `radius` of `at`, from the buckets.
    fn near(&self, at: Vec2, radius: f32, mut visit: impl FnMut(u32)) {
        let (ox, oz, w, h) = self.grid;
        let reach = (radius / BUCKET).ceil() as i32 + 1;
        let (cx, cz) = ((at.x / BUCKET).floor() as i32 - ox, (at.z / BUCKET).floor() as i32 - oz);
        for z in (cz - reach).max(0)..=(cz + reach).min(h - 1) {
            for x in (cx - reach).max(0)..=(cx + reach).min(w - 1) {
                for &i in &self.buckets[(z * w + x) as usize] {
                    visit(i);
                }
            }
        }
    }

    /// What each side can see, then the nearest visible enemy for everyone who can shoot one.
    fn retarget(&mut self, frame: u32) {
        for i in 0..self.bodies.len() {
            let body = &self.bodies[i];
            if !body.here(frame) {
                self.seen[i] = false;
                continue;
            }
            // Sight is shared across a team, so one scout in front lets the whole line shoot.
            let (pos, side) = (body.pos, body.side);
            self.seen[i] = (self.bodies.iter())
                .any(|o| o.here(frame) && o.side != side && o.pos.dist2(pos) <= o.sight * o.sight);
        }
        for i in 0..self.bodies.len() {
            if !self.bodies[i].here(frame) || self.rules.shots[self.bodies[i].def as usize].is_empty() {
                continue;
            }
            let (pos, side, old) = (self.bodies[i].pos, self.bodies[i].side, self.bodies[i].target);
            let mut best = (f32::MAX, NONE);
            for (j, other) in self.bodies.iter().enumerate() {
                if other.here(frame) && other.side != side && self.seen[j] {
                    let d = pos.dist2(other.pos);
                    if d < best.0 {
                        best = (d, j as u32);
                    }
                }
            }
            self.bodies[i].target = best.1;
            if best.1 != old && best.1 != NONE {
                self.bodies[i].aimed_at = frame + (self.rules.tuning.aim_seconds * FPS) as u32;
            }
        }
    }

    /// Walk: at the target if there is one and it is out of range, otherwise at where the enemy stands. A step
    /// into ground another unit already stands on is refused, not shared, so a blob's rear ranks have to go round.
    fn advance(&mut self, frame: u32) {
        for i in 0..self.bodies.len() {
            let body = &self.bodies[i];
            if !body.here(frame) || body.hold || body.speed <= 0.0 {
                continue;
            }
            let (pos, side, target, reach, speed) = (body.pos, body.side as usize, body.target, body.reach, body.speed);
            let stop = reach * self.rules.tuning.stop_at;
            let goal = if target != NONE { self.bodies[target as usize].pos } else { self.goal[side] };
            if target != NONE && pos.dist(goal) <= stop {
                self.bodies[i].vel = Vec2::default();
                continue;
            }
            // Out of contact the walking field goes round cliffs; once a target is in sight, straight at it.
            let dir = match &self.flows[side] {
                Some(flow) if target == NONE => flow.direction(pos).unwrap_or_else(|| pos.towards(goal)),
                _ => pos.towards(goal),
            };
            let mut moved = Vec2::default();
            for turn in DEFLECTIONS {
                let (sin, cos) = turn.sin_cos();
                let step = Vec2::new(dir.x * cos - dir.z * sin, dir.x * sin + dir.z * cos) * speed;
                if self.open(side, pos + step, i as u32) {
                    moved = step;
                    break;
                }
            }
            self.bodies[i].pos = pos + moved;
            self.bodies[i].vel = moved;
        }
    }

    /// Whether this unit may stand at `at`: walkable ground, and no other unit's footprint already there.
    fn open(&self, side: usize, at: Vec2, me: u32) -> bool {
        if self.flows[side].as_ref().is_some_and(|f| !f.walkable(at)) {
            return false;
        }
        if !self.rules.tuning.collide {
            return true;
        }
        let radius = self.bodies[me as usize].radius;
        let mut clear = true;
        self.near(at, radius + self.widest_radius, |j| {
            if j == me {
                return;
            }
            let other = &self.bodies[j as usize];
            let want = radius + other.radius;
            if at.dist2(other.pos) < want * want {
                clear = false;
            }
        });
        clear
    }

    fn fire(&mut self, frame: u32) {
        for gun in 0..self.guns.len() {
            let (body_index, shot_index) = (self.guns[gun].body as usize, self.guns[gun].shot as usize);
            let body = &self.bodies[body_index];
            if !body.here(frame) || frame < body.aimed_at || body.target == NONE {
                continue;
            }
            let (target, from, def) = (body.target, body.pos, body.def as usize);
            let shot = &self.rules.shots[def][shot_index];
            let goal = &self.bodies[target as usize];
            if !goal.alive || shot.damage[goal.def as usize] <= 0.0 {
                continue;
            }
            let distance = from.dist(goal.pos);
            if distance > shot.range {
                continue;
            }
            let gun_state = &self.guns[gun];
            if gun_state.salvo > 0 {
                if frame < gun_state.next {
                    continue;
                }
                self.guns[gun].salvo -= 1;
                self.guns[gun].next = frame + shot.burst_rate;
            } else {
                if frame < gun_state.ready {
                    continue;
                }
                // A weapon that costs energy holds its shot until the store can pay for the whole salvo.
                let cost = shot.energy * shot.burst as f32 * shot.projectiles as f32;
                let side = body.side as usize;
                if cost > 0.0 {
                    if self.energy[side] < cost {
                        continue;
                    }
                    self.energy[side] -= cost;
                }
                self.guns[gun].ready = frame + shot.reload;
                self.guns[gun].salvo = shot.burst - 1;
                self.guns[gun].next = frame + shot.burst_rate;
                let (ex, ez) = self.rng.in_ball(shot.salvo_spread);
                self.guns[gun].salvo_error = Vec2::new(ex, ez);
            }
            let shot = &self.rules.shots[def][shot_index];
            let flight = shot.flight_frames(distance);
            let (predict_mod, salvo_error) = (self.guns[gun].predict_mod, self.guns[gun].salvo_error);
            for _ in 0..shot.projectiles {
                let aim = if shot.tracks {
                    Aim::At(target)
                } else {
                    // Lead the target as well as this weapon can, then miss by its aim cone.
                    let goal = &self.bodies[target as usize];
                    let mult = predict_mod * (1.0 - shot.predict_boost) + shot.predict_boost;
                    let mut lead = goal.vel * (flight as f32 * mult);
                    if shot.lead_limit >= 0.0 {
                        let length = (lead.x * lead.x + lead.z * lead.z).sqrt();
                        if length > shot.lead_limit {
                            lead = lead * (shot.lead_limit / (length + 0.01));
                        }
                    }
                    let (sx, sz) = self.rng.in_ball(shot.spray_spread);
                    let scatter = (salvo_error + Vec2::new(sx, sz)) * distance;
                    Aim::Point(goal.pos + lead + scatter)
                };
                self.impacts.push(Impact { frame: frame + flight, from: body_index as u32, def: def as u32, shot: shot_index as u32, aim });
            }
        }
    }

    /// Land every shot due this frame. Returns whether anything was hurt.
    fn resolve(&mut self, frame: u32) -> bool {
        let mut hurt = false;
        let mut landed = Vec::new();
        let mut pending = std::mem::take(&mut self.impacts);
        pending.retain(|impact| {
            if impact.frame > frame {
                return true;
            }
            landed.push((impact.from, impact.def, impact.shot, match impact.aim {
                Aim::At(target) => Ok(target),
                Aim::Point(at) => Err(at),
            }));
            false
        });
        self.impacts = pending;
        for (from, def, shot_index, aim) in landed {
            let shot = &self.rules.shots[def as usize][shot_index as usize];
            let at = match aim {
                Ok(target) => {
                    let goal = &self.bodies[target as usize];
                    if !goal.alive {
                        // A guided shot at something already dead is wasted.
                        continue;
                    }
                    goal.pos
                }
                // An unguided shot collides with whatever its path runs into, not only with what it was aimed at.
                Err(point) => {
                    let mut hit = point;
                    let mut best = f32::MAX;
                    self.near(point, self.widest_radius, |i| {
                        let body = &self.bodies[i as usize];
                        let d = body.pos.dist(point);
                        if i != from && d < body.radius && d < best {
                            best = d;
                            hit = body.pos;
                        }
                    });
                    hit
                }
            };
            let (aoe, edge) = (shot.aoe, shot.edge);
            let mut damage: Vec<(u32, f32)> = Vec::new();
            self.near(at, aoe + self.widest_radius, |i| {
                if i == from {
                    return;
                }
                let body = &self.bodies[i as usize];
                // The engine measures to the collision volume's surface, not the unit's centre, and its falloff
                // reaches zero at the rim however large `edgeeffectiveness` is (GameHelper.cpp DoExplosionDamage).
                let d = (body.pos.dist(at) - body.radius).max(0.0);
                if d <= aoe {
                    let falloff =
                        if edge >= 1.0 || d < 1.0 { 1.0 } else { (aoe + 0.001 - d) / (aoe + 0.001 - d * edge) };
                    damage.push((i, shot.damage[body.def as usize] * falloff));
                }
            });
            for (i, amount) in damage {
                let body = &mut self.bodies[i as usize];
                if amount > 0.0 && body.alive {
                    body.hp -= amount;
                    hurt = true;
                    if body.hp <= 0.0 {
                        body.alive = false;
                    }
                }
            }
        }
        hurt
    }
}

fn centre(bodies: &[Body], side: usize, frame: u32) -> Vec2 {
    let mut sum = Vec2::default();
    let mut n = 0.0;
    for body in bodies.iter().filter(|b| b.here(frame) && b.side as usize == side) {
        sum = sum + body.pos;
        n += 1.0;
    }
    if n == 0.0 { Vec2::default() } else { sum * (1.0 / n) }
}
