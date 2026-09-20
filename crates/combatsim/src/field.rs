//! Ground a fight happens on: which cells a unit can stand in, and which way to walk from each of them.
//!
//! The grid is the bot's terrain format (`docs/harness/record-format.md`): heights as i16 and slopes as u8 at
//! 16-elmo cells. The walking field is the same Dijkstra sweep as `crates/terrain`, run backwards from
//! the goal so units can read a direction straight out of it; a choke then limits how many units get through
//! without any pathfinding in the hot loop.

use crate::scenario::Vec2;

pub const UNREACHABLE: u32 = u32::MAX;

#[derive(Clone, Debug)]
pub struct Field {
    pub cell: f32,
    pub width: usize,
    pub height: usize,
    pub heights: Vec<i16>,
    pub slopes: Vec<u8>,
}

/// Where one side wants to go, for one slope tolerance: tenths of a cell of walking from every cell to the goal.
pub struct Flow {
    width: usize,
    height: usize,
    cell: f32,
    cost: Vec<u32>,
    passable: Vec<bool>,
}

impl Field {
    /// Reads the bot's `terrain-<ai>.bin`: every height as a little-endian i16, then every slope as a u8.
    pub fn from_bytes(bytes: &[u8], width: usize, height: usize, cell: f32) -> Option<Field> {
        let cells = width * height;
        if bytes.len() < cells * 3 {
            return None;
        }
        let heights = bytes[..cells * 2].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
        let slopes = bytes[cells * 2..cells * 3].to_vec();
        Some(Field { cell, width, height, heights, slopes })
    }

    /// A bare rectangle of flat ground: the duels' site, and the default when a query names no terrain.
    pub fn flat(width: usize, height: usize, cell: f32) -> Field {
        Field { cell, width, height, heights: vec![0; width * height], slopes: vec![0; width * height] }
    }

    fn index(&self, at: Vec2) -> Option<usize> {
        let (x, z) = ((at.x / self.cell) as i32, (at.z / self.cell) as i32);
        if x < 0 || z < 0 || x >= self.width as i32 || z >= self.height as i32 {
            return None;
        }
        Some(z as usize * self.width + x as usize)
    }

    /// Cells a unit of this movement class can stand in: gentle enough, and not deeper than it can wade.
    pub fn passable(&self, max_slope: i32, max_depth: f32) -> Vec<bool> {
        (self.heights.iter().zip(&self.slopes))
            .map(|(&height, &slope)| i32::from(slope) <= max_slope && f32::from(height) >= -max_depth)
            .collect()
    }

    /// Walking costs to `goal` over `passable` ground.
    pub fn flow(&self, passable: Vec<bool>, goal: Vec2) -> Flow {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        let mut flow = Flow {
            width: self.width,
            height: self.height,
            cell: self.cell,
            cost: vec![UNREACHABLE; self.width * self.height],
            passable,
        };
        let Some(start) = self.index(goal).filter(|&i| flow.passable[i]) else { return flow };
        let mut queue = BinaryHeap::new();
        flow.cost[start] = 0;
        queue.push(Reverse((0u32, start)));
        while let Some(Reverse((cost, index))) = queue.pop() {
            if cost > flow.cost[index] {
                continue;
            }
            let (x, z) = ((index % self.width) as i32, (index / self.width) as i32);
            for (dx, dz, step) in NEIGHBOURS {
                let (nx, nz) = (x + dx, z + dz);
                if nx < 0 || nz < 0 || nx >= self.width as i32 || nz >= self.height as i32 {
                    continue;
                }
                let next = nz as usize * self.width + nx as usize;
                // No cutting corners between two blocked cells.
                let clear = dx == 0
                    || dz == 0
                    || (flow.passable[z as usize * self.width + nx as usize] && flow.passable[nz as usize * self.width + x as usize]);
                if flow.passable[next] && clear && cost + step < flow.cost[next] {
                    flow.cost[next] = cost + step;
                    queue.push(Reverse((cost + step, next)));
                }
            }
        }
        flow
    }
}

const NEIGHBOURS: [(i32, i32, u32); 8] =
    [(1, 0, 10), (-1, 0, 10), (0, 1, 10), (0, -1, 10), (1, 1, 14), (1, -1, 14), (-1, 1, 14), (-1, -1, 14)];

impl Flow {
    fn cell_index(&self, at: Vec2) -> Option<usize> {
        let (x, z) = ((at.x / self.cell) as i32, (at.z / self.cell) as i32);
        if x < 0 || z < 0 || x >= self.width as i32 || z >= self.height as i32 {
            return None;
        }
        Some(z as usize * self.width + x as usize)
    }

    pub fn walkable(&self, at: Vec2) -> bool {
        self.cell_index(at).is_some_and(|i| self.passable[i])
    }

    /// The way downhill in walking cost from `at`, or `None` where the goal cannot be reached (the caller then
    /// walks straight at it and is stopped by whatever blocks the way).
    pub fn direction(&self, at: Vec2) -> Option<Vec2> {
        let index = self.cell_index(at)?;
        let here = self.cost[index];
        if here == UNREACHABLE {
            return None;
        }
        let (x, z) = ((index % self.width) as i32, (index / self.width) as i32);
        let mut best = (here, None);
        for (dx, dz, _) in NEIGHBOURS {
            let (nx, nz) = (x + dx, z + dz);
            if nx < 0 || nz < 0 || nx >= self.width as i32 || nz >= self.height as i32 {
                continue;
            }
            let next = nz as usize * self.width + nx as usize;
            if self.cost[next] < best.0 {
                best = (self.cost[next], Some(Vec2::new(dx as f32, dz as f32)));
            }
        }
        best.1.map(|d| Vec2::default().towards(d))
    }
}
