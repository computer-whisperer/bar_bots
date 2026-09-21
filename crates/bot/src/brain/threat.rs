//! The threat grid: how much damage a second the enemy can put on each cell of the map, rebuilt every tick for the
//! control lane (`micro.rs`, `docs/design/2026-09-20-micro-lane.md`). Every armed enemy stamps its damage rate over
//! its reach, full inside it and falling to nothing over a tail beyond, so that the grid slopes away from danger
//! outside the range as well as in it.

use bot_protocol::Vec3;

/// How much more than its weight a threat costs at its own position than at the edge of its reach.
const INNER_SLOPE: f32 = 0.5;

pub struct ThreatGrid {
    cell: f32,
    width: usize,
    height: usize,
    cost: Vec<f32>,
}

impl ThreatGrid {
    pub fn new(cell: f32, width: usize, height: usize) -> ThreatGrid {
        ThreatGrid { cell, width, height, cost: vec![0.0; width * height] }
    }

    pub fn clear(&mut self) {
        self.cost.fill(0.0);
    }

    /// Adds `weight` (damage a second) to every cell within `reach` of `at`, rising to half as much again at the
    /// centre so that the grid slopes toward safety inside the reach too (a flat disc gave a unit under a tower no
    /// way to tell out from in, and its steps went deeper), and a share of it falling linearly to nothing at
    /// `reach + tail`.
    pub fn stamp(&mut self, at: Vec3, reach: f32, tail: f32, weight: f32) {
        if weight <= 0.0 {
            return;
        }
        let outer = reach + tail;
        let (cx, cz) = (at.x / self.cell, at.z / self.cell);
        let r = outer / self.cell;
        let (x0, x1) = (((cx - r).floor() as i32).max(0), ((cx + r).ceil() as i32).min(self.width as i32 - 1));
        let (z0, z1) = (((cz - r).floor() as i32).max(0), ((cz + r).ceil() as i32).min(self.height as i32 - 1));
        for z in z0..=z1 {
            for x in x0..=x1 {
                let d = ((x as f32 + 0.5 - cx).hypot(z as f32 + 0.5 - cz)) * self.cell;
                let share = if d <= reach { 1.0 + INNER_SLOPE * (1.0 - d / reach.max(1.0)) } else if d < outer { (outer - d) / tail.max(1.0) } else { continue };
                self.cost[z as usize * self.width + x as usize] += weight * share;
            }
        }
    }

    fn index(&self, pos: Vec3) -> Option<usize> {
        let (x, z) = ((pos.x / self.cell) as i32, (pos.z / self.cell) as i32);
        (x >= 0 && z >= 0 && x < self.width as i32 && z < self.height as i32).then(|| z as usize * self.width + x as usize)
    }

    /// Damage a second on the cell under `pos`; 0 off the map.
    pub fn at(&self, pos: Vec3) -> f32 {
        self.index(pos).map_or(0.0, |i| self.cost[i])
    }

    /// The centre of the cell within `radius` of `pos` with the least threat, among those our units can stand on
    /// (`passable`, one flag a cell in the grid's order, or none known); ties go to the cell nearest `prefer`.
    pub fn lowest_within(&self, pos: Vec3, radius: f32, passable: Option<&[bool]>, prefer: Vec3) -> Option<Vec3> {
        let (cx, cz) = ((pos.x / self.cell) as i32, (pos.z / self.cell) as i32);
        let r = (radius / self.cell).ceil() as i32;
        let mut best: Option<(f32, f32, Vec3)> = None;
        for z in (cz - r).max(0)..=(cz + r).min(self.height as i32 - 1) {
            for x in (cx - r).max(0)..=(cx + r).min(self.width as i32 - 1) {
                let centre = Vec3 { x: (x as f32 + 0.5) * self.cell, y: 0.0, z: (z as f32 + 0.5) * self.cell };
                if centre.dist2d(pos) > radius {
                    continue;
                }
                let i = z as usize * self.width + x as usize;
                if passable.is_some_and(|p| !p[i]) {
                    continue;
                }
                let key = (self.cost[i], centre.dist2d(prefer));
                if best.is_none_or(|(c, d, _)| key.0 < c || (key.0 == c && key.1 < d)) {
                    best = Some((key.0, key.1, centre));
                }
            }
        }
        best.map(|(_, _, centre)| centre)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, z: f32) -> Vec3 {
        Vec3 { x, y: 0.0, z }
    }

    #[test]
    fn a_stamp_is_full_inside_the_reach_and_slopes_off_over_the_tail() {
        let mut grid = ThreatGrid::new(16.0, 64, 64);
        grid.stamp(at(512.0, 512.0), 160.0, 80.0, 100.0);
        assert!(grid.at(at(512.0, 512.0)) > 140.0, "steepest at the source");
        let inside = grid.at(at(512.0 + 150.0, 512.0));
        assert!((100.0..110.0).contains(&inside), "{inside}");
        let on_the_tail = grid.at(at(512.0 + 200.0, 512.0));
        assert!(on_the_tail > 0.0 && on_the_tail < 100.0, "{on_the_tail}");
        assert_eq!(grid.at(at(512.0 + 300.0, 512.0)), 0.0);
    }

    #[test]
    fn the_lowest_cell_nearby_is_away_from_the_threat_and_toward_the_goal() {
        let mut grid = ThreatGrid::new(16.0, 64, 64);
        grid.stamp(at(512.0, 512.0), 160.0, 80.0, 100.0);
        // Standing at the edge of the reach, east of the threat, wanting to go north-east.
        let step = grid.lowest_within(at(660.0, 512.0), 150.0, None, at(900.0, 300.0)).unwrap();
        assert!(step.x > 700.0, "steps away from the threat: {step:?}");
        assert!(step.z < 512.0, "and toward the goal: {step:?}");
        assert_eq!(grid.at(step), 0.0);
    }

    #[test]
    fn inside_the_reach_the_step_goes_outward() {
        let mut grid = ThreatGrid::new(16.0, 64, 64);
        grid.stamp(at(512.0, 512.0), 300.0, 90.0, 100.0);
        // Deep inside, wanting to go on through the threat: the step still goes away from it.
        let step = grid.lowest_within(at(700.0, 512.0), 100.0, None, at(300.0, 512.0)).unwrap();
        assert!(step.x > 760.0, "{step:?}");
    }

    #[test]
    fn impassable_cells_are_never_a_destination() {
        let mut grid = ThreatGrid::new(16.0, 8, 8);
        grid.stamp(at(8.0, 8.0), 40.0, 40.0, 10.0);
        let mut passable = vec![false; 64];
        passable[7 * 8 + 7] = true;
        let step = grid.lowest_within(at(64.0, 64.0), 200.0, Some(&passable), at(0.0, 0.0)).unwrap();
        assert_eq!((step.x, step.z), (120.0, 120.0));
    }
}
