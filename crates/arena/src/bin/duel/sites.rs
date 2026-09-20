//! Where duels are fought: flat, dry rectangles every listed unit can cross, far from the commanders and from
//! each other. The long side runs west-east: spawned units always face south (the engine's cheat gives no choice),
//! so on a west-east axis both sides start side-on to the enemy and neither has its back turned.

use bot_protocol::{Terrain, Vec3};

/// Distance between the two armies' front ranks' centres; beyond every tier-1 weapon's range (artillery: ~710).
pub const SEPARATION: f32 = 1100.0;
/// Rectangle checked for flatness: the separation plus room for the ranks behind, and the width of a rank.
const LENGTH: f32 = 1440.0;
const WIDTH: f32 = 640.0;
/// Two sites' rectangles stay this far apart: a team shares sight between its duels, and artillery reaches ~710.
const SITE_GAP: f32 = 900.0;
/// No site this close to a commander, who shoots at what comes near and whose death ends the game; tier-1 units
/// see about 500 elmos.
const COMMANDER_GAP: f32 = 900.0;
/// Most the ground may rise or fall within a site, in elmos (height lends range to ballistic weapons).
const MAX_RELIEF: i32 = 16;
/// Candidate rectangles are tried every this many cells.
const STRIDE: usize = 4;

#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub centre: Vec3,
}

impl Site {
    /// Centre of the front rank at the west (`true`) or east end.
    pub fn end(&self, west: bool) -> Vec3 {
        let dx = if west { -SEPARATION / 2.0 } else { SEPARATION / 2.0 };
        Vec3 { x: self.centre.x + dx, ..self.centre }
    }
}

/// Up to `wanted` sites. `max_slope` is the engine slope value (0-1) the least agile listed unit manages.
pub fn choose(terrain: &Terrain, max_slope: f32, commanders: &[Vec3], wanted: usize) -> Vec<Site> {
    let (width, height) = (terrain.width as usize, terrain.height as usize);
    if width == 0 || terrain.heights.len() != width * height {
        return Vec::new();
    }
    let (length_cells, width_cells) = ((LENGTH / terrain.cell) as usize, (WIDTH / terrain.cell) as usize);
    let slope_limit = (max_slope * 255.0) as u8;
    let mut candidates = Vec::new();
    for z0 in (0..height.saturating_sub(width_cells)).step_by(STRIDE) {
        'candidate: for x0 in (0..width.saturating_sub(length_cells)).step_by(STRIDE) {
            let (mut low, mut high) = (i32::MAX, i32::MIN);
            for z in z0..z0 + width_cells {
                for x in x0..x0 + length_cells {
                    let index = z * width + x;
                    let h = i32::from(terrain.heights[index]);
                    if terrain.slopes[index] > slope_limit || h < 5 {
                        continue 'candidate;
                    }
                    (low, high) = (low.min(h), high.max(h));
                    if high - low > MAX_RELIEF {
                        continue 'candidate;
                    }
                }
            }
            let centre = Vec3 {
                x: (x0 as f32 + length_cells as f32 / 2.0) * terrain.cell,
                y: (low + high) as f32 / 2.0,
                z: (z0 as f32 + width_cells as f32 / 2.0) * terrain.cell,
            };
            if commanders.iter().all(|c| rect_distance(centre, *c) >= COMMANDER_GAP) {
                candidates.push((high - low, centre));
            }
        }
    }
    // Greedy packing finds a different number of sites depending on where it starts, so try a few orders
    // (flattest first, then sweeps across the map) and keep the fullest result.
    let orders: [fn(&(i32, Vec3)) -> f32; 7] = [
        |c| c.0 as f32,
        |c| c.1.x,
        |c| c.1.z,
        |c| -c.1.x,
        |c| -c.1.z,
        |c| c.1.x + c.1.z,
        |c| c.1.x - c.1.z,
    ];
    let mut best: Vec<Site> = Vec::new();
    for key in orders {
        candidates.sort_by(|a, b| key(a).total_cmp(&key(b)));
        let mut sites: Vec<Site> = Vec::new();
        for (_, centre) in &candidates {
            let clear = sites.iter().all(|s| {
                (s.centre.x - centre.x).abs() >= LENGTH + SITE_GAP || (s.centre.z - centre.z).abs() >= WIDTH + SITE_GAP
            });
            if clear && sites.len() < wanted {
                sites.push(Site { centre: *centre });
            }
        }
        if sites.len() > best.len() {
            best = sites;
        }
    }
    best
}

/// Distance from `point` to the site rectangle centred on `centre`.
fn rect_distance(centre: Vec3, point: Vec3) -> f32 {
    let dx = ((point.x - centre.x).abs() - LENGTH / 2.0).max(0.0);
    let dz = ((point.z - centre.z).abs() - WIDTH / 2.0).max(0.0);
    dx.hypot(dz)
}

/// Positions for `count` units: ranks run north-south through `front`, further ranks stack away from the enemy.
pub fn formation(front: Vec3, faces_east: bool, count: u32, spacing: f32) -> Vec<Vec3> {
    const PER_RANK: u32 = 8;
    let back = if faces_east { -spacing } else { spacing };
    (0..count)
        .map(|i| {
            let (rank, file) = (i / PER_RANK, i % PER_RANK);
            let in_rank = (count - rank * PER_RANK).min(PER_RANK);
            Vec3 {
                x: front.x + back * rank as f32,
                y: front.y,
                z: front.z + (file as f32 - (in_rank - 1) as f32 / 2.0) * spacing,
            }
        })
        .collect()
}
