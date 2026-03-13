use std::collections::{HashMap, HashSet};
use glam::Vec2;

const CELL_SIZE: f32 = 512.0;

fn cell(p: Vec2) -> (i32, i32) {
    (p.x.div_euclid(CELL_SIZE) as i32, p.y.div_euclid(CELL_SIZE) as i32)
}

pub struct SpatialGrid {
    cells: HashMap<(i32, i32), Vec<u64>>,
}

impl SpatialGrid {
    pub fn new() -> Self {
        Self { cells: HashMap::new() }
    }

    pub fn clear(&mut self) {
        self.cells.clear();
    }

    pub fn insert(&mut self, id: u64, aabb_min: Vec2, aabb_max: Vec2) {
        let (c0x, c0y) = cell(aabb_min);
        let (c1x, c1y) = cell(aabb_max);
        for cx in c0x..=c1x {
            for cy in c0y..=c1y {
                self.cells.entry((cx, cy)).or_default().push(id);
            }
        }
    }

    /// Returns unique IDs overlapping the given AABB of world-space mins/maxes.
    pub fn query(&self, aabb_min: Vec2, aabb_max: Vec2) -> HashSet<u64> {
        let (c0x, c0y) = cell(aabb_min);
        let (c1x, c1y) = cell(aabb_max);
        let mut result: HashSet<u64> = HashSet::new();
        for cx in c0x..=c1x {
            for cy in c0y..=c1y {
                if let Some(ids) = self.cells.get(&(cx, cy)) {
                    for &id in ids {
                        result.insert(id);
                    }
                }
            }
        }
        result
    }
}
