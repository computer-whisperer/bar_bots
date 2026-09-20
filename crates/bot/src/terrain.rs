//! The ground as units experience it: where a movement class can go, and how far places are by foot rather than
//! as the crow flies. Quicksilver has cliffs and water between points a straight line joins; every rule that says
//! "nearer", "forward" or "our half" means walking distance (K-maps-terrain-not-straight-lines).

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use bot_protocol::{MoveClass, MoveKind, Terrain, Vec3};

/// Walking distances from the nearest of its origins for one movement class, over the terrain grid.
pub struct Field {
    cell: f32,
    width: usize,
    height: usize,
    /// Tenths of a cell; `u32::MAX` where the class cannot get to.
    cost: Vec<u32>,
}

const UNREACHABLE: u32 = u32::MAX;
/// A position is judged by the best cell this close to it: buildings and spots sit on ledges' edges and shorelines.
const SNAP_CELLS: i32 = 6;

pub fn passable(terrain: &Terrain, class: MoveClass) -> Vec<bool> {
    let max_slope = (class.max_slope * 255.0).round() as i32;
    terrain
        .heights
        .iter()
        .zip(&terrain.slopes)
        .map(|(&height, &slope)| {
            let height = f32::from(height);
            match class.kind {
                MoveKind::Tank | MoveKind::Bot => i32::from(slope) <= max_slope && height >= -class.depth,
                MoveKind::Hover => i32::from(slope) <= max_slope || height < 0.0,
                MoveKind::Ship => height <= -class.depth,
            }
        })
        .collect()
}

impl Field {
    /// Distances from `origin` for a class that can stand where `passable` says. `None` without terrain data.
    pub fn from(terrain: &Terrain, passable: &[bool], origin: Vec3) -> Option<Field> {
        Field::from_many(terrain, passable, &[origin])
    }

    /// Distances from whichever of `origins` is nearest on foot. `None` when none of them is near passable ground.
    pub fn from_many(terrain: &Terrain, passable: &[bool], origins: &[Vec3]) -> Option<Field> {
        let (width, height) = (terrain.width as usize, terrain.height as usize);
        if width == 0 || passable.len() != width * height {
            return None;
        }
        let mut field = Field { cell: terrain.cell, width, height, cost: vec![UNREACHABLE; width * height] };
        let mut queue = BinaryHeap::new();
        for origin in origins {
            if let Some(start) = field.nearest(*origin, |index| passable[index]) {
                field.cost[start] = 0;
                queue.push(Reverse((0u32, start)));
            }
        }
        if queue.is_empty() {
            return None;
        }
        while let Some(Reverse((cost, index))) = queue.pop() {
            if cost > field.cost[index] {
                continue;
            }
            let (x, z) = ((index % width) as i32, (index / width) as i32);
            for (dx, dz, step) in [(1, 0, 10), (-1, 0, 10), (0, 1, 10), (0, -1, 10), (1, 1, 14), (1, -1, 14), (-1, 1, 14), (-1, -1, 14)] {
                let (nx, nz) = (x + dx, z + dz);
                if nx < 0 || nz < 0 || nx >= width as i32 || nz >= height as i32 {
                    continue;
                }
                let next = nz as usize * width + nx as usize;
                // No cutting corners between two blocked cells.
                let corner_clear = dx == 0 || dz == 0 || (passable[z as usize * width + nx as usize] && passable[nz as usize * width + x as usize]);
                if passable[next] && corner_clear && cost + step < field.cost[next] {
                    field.cost[next] = cost + step;
                    queue.push(Reverse((cost + step, next)));
                }
            }
        }
        Some(field)
    }

    fn cell_of(&self, pos: Vec3) -> (i32, i32) {
        ((pos.x / self.cell) as i32, (pos.z / self.cell) as i32)
    }

    fn centre(&self, index: usize) -> Vec3 {
        Vec3 { x: ((index % self.width) as f32 + 0.5) * self.cell, y: 0.0, z: ((index / self.width) as f32 + 0.5) * self.cell }
    }

    /// The cell nearest `pos`, within the snap distance, that `accept`s.
    fn nearest(&self, pos: Vec3, accept: impl Fn(usize) -> bool) -> Option<usize> {
        let (cx, cz) = self.cell_of(pos);
        let mut best: Option<(i32, usize)> = None;
        for dz in -SNAP_CELLS..=SNAP_CELLS {
            for dx in -SNAP_CELLS..=SNAP_CELLS {
                let (x, z) = (cx + dx, cz + dz);
                if x < 0 || z < 0 || x >= self.width as i32 || z >= self.height as i32 {
                    continue;
                }
                let index = z as usize * self.width + x as usize;
                let apart = dx * dx + dz * dz;
                if accept(index) && best.is_none_or(|(d, _)| apart < d) {
                    best = Some((apart, index));
                }
            }
        }
        best.map(|(_, index)| index)
    }

    /// Walking distance from the origin to (the reachable ground nearest) `pos`, in elmos.
    pub fn distance(&self, pos: Vec3) -> Option<f32> {
        let index = self.nearest(pos, |i| self.cost[i] != UNREACHABLE)?;
        Some(self.cost[index] as f32 / 10.0 * self.cell)
    }

    /// The reachable ground nearest `pos`, if any is close.
    pub fn snap(&self, pos: Vec3) -> Option<Vec3> {
        self.nearest(pos, |i| self.cost[i] != UNREACHABLE).map(|index| self.centre(index))
    }

    /// The point `along` elmos from the origin on the way to `goal`; the goal itself if it is nearer than that.
    pub fn towards(&self, goal: Vec3, along: f32) -> Option<Vec3> {
        let mut index = self.nearest(goal, |i| self.cost[i] != UNREACHABLE)?;
        let wanted = (along / self.cell * 10.0) as u32;
        while self.cost[index] > wanted {
            let (x, z) = ((index % self.width) as i32, (index / self.width) as i32);
            let downhill = (-1..=1)
                .flat_map(|dz| (-1..=1).map(move |dx| (x + dx, z + dz)))
                .filter(|&(nx, nz)| nx >= 0 && nz >= 0 && nx < self.width as i32 && nz < self.height as i32)
                .map(|(nx, nz)| nz as usize * self.width + nx as usize)
                .min_by_key(|&next| self.cost[next])?;
            if self.cost[downhill] >= self.cost[index] {
                break;
            }
            index = downhill;
        }
        Some(self.centre(index))
    }
}

/// A coarse picture of the map in text, `size` characters square, for a reader that cannot see it: `~` water,
/// `#` ground this class cannot stand on (cliffs), `x` ground it could stand on but cannot walk to from the field's
/// origin, and `.` `o` `O` walkable ground by height (low, middle, high thirds).
pub fn sketch(terrain: &Terrain, passable: &[bool], field: &Field, size: usize) -> Vec<String> {
    let (width, height) = (terrain.width as usize, terrain.height as usize);
    let top = terrain.heights.iter().copied().max().unwrap_or(1).max(1) as f32;
    let (step_x, step_z) = (width.div_ceil(size), height.div_ceil(size));
    (0..size)
        .map(|row| {
            (0..size)
                .map(|column| {
                    let (mut water, mut blocked, mut cut_off, mut walkable, mut height_sum) = (0, 0, 0, 0, 0.0);
                    for z in (row * step_z)..((row + 1) * step_z).min(height) {
                        for x in (column * step_x)..((column + 1) * step_x).min(width) {
                            let index = z * width + x;
                            if terrain.heights[index] < 0 {
                                water += 1;
                            } else if !passable[index] {
                                blocked += 1;
                            } else if field.cost[index] == UNREACHABLE {
                                cut_off += 1;
                            } else {
                                walkable += 1;
                                height_sum += f32::from(terrain.heights[index]);
                            }
                        }
                    }
                    // A cell is what most of it is, except that any real share of cliff shows: cliffs are thin.
                    let land = blocked + cut_off + walkable;
                    if water > land {
                        '~'
                    } else if blocked * 3 >= land {
                        '#'
                    } else if cut_off > walkable {
                        'x'
                    } else {
                        match height_sum / walkable.max(1) as f32 / top {
                            t if t < 0.33 => '.',
                            t if t < 0.66 => 'o',
                            _ => 'O',
                        }
                    }
                })
                .collect()
        })
        .collect()
}

