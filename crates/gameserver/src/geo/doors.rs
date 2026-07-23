//! Door collision — the port of `DoorData.checkIfDoorsBetween`, the check
//! Java runs at the head of `GeoEngine.canSeeTarget` / `getValidLocation` /
//! `canMoveToTarget` (doors don't carve the geodata; every geo query just
//! also tests the segment against nearby closed doors' collision polygons).
//!
//! The grid lives *inside* [`super::GeoEngine`] like Java, so the path
//! worker's `canMoveToTarget` postfilter sees doors for free. Geometry is
//! registered once at boot (`&mut` before the engine is `Arc`-wrapped);
//! the only mutable state — each door's open flag — is an `AtomicBool` the
//! game thread flips through the shared handle (`set_open`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

/// One door's collision state (Java `Door`'s geometry accessors + `_open`).
pub struct DoorShape {
    pub door_id: i32,
    node_x: [i32; 4],
    node_y: [i32; 4],
    z_min: i32,
    z_max: i32,
    open: AtomicBool,
}

#[derive(Default)]
pub struct DoorGrid {
    doors: Vec<DoorShape>,
    by_id: HashMap<i32, u32>,
    /// Visibility-region cell (`world::REGION_SHIFT`) → door indexes. Doors
    /// register into their own cell *and* the 8 surrounding ones (Java
    /// `WorldRegion.addVisibleObject` pushes a door into every surrounding
    /// region's `_doors`), so a lookup at a segment's start cell finds every
    /// door near it.
    by_region: HashMap<(i32, i32), Vec<u32>>,
}

impl DoorGrid {
    pub fn is_empty(&self) -> bool {
        self.doors.is_empty()
    }

    /// Register a door's collision polygon (boot / tests; `&mut` — geometry
    /// is fixed once the engine is shared).
    pub fn register(
        &mut self,
        door_id: i32,
        node_x: [i32; 4],
        node_y: [i32; 4],
        z_min: i32,
        z_max: i32,
        open: bool,
    ) {
        let idx = self.doors.len() as u32;
        self.doors.push(DoorShape {
            door_id,
            node_x,
            node_y,
            z_min,
            z_max,
            open: AtomicBool::new(open),
        });
        self.by_id.insert(door_id, idx);
        let region = (
            self.doors[idx as usize].node_x.iter().sum::<i32>() / 4 >> crate::world::REGION_SHIFT,
            self.doors[idx as usize].node_y.iter().sum::<i32>() / 4 >> crate::world::REGION_SHIFT,
        );
        for dx in -1..=1 {
            for dy in -1..=1 {
                self.by_region
                    .entry((region.0 + dx, region.1 + dy))
                    .or_default()
                    .push(idx);
            }
        }
    }

    /// Flip a door's open state (game thread, through the shared engine).
    pub fn set_open(&self, door_id: i32, open: bool) {
        if let Some(&idx) = self.by_id.get(&door_id) {
            self.doors[idx as usize].open.store(open, Ordering::Relaxed);
        }
    }

    pub fn is_open(&self, door_id: i32) -> bool {
        self.by_id
            .get(&door_id)
            .is_some_and(|&idx| self.doors[idx as usize].open.load(Ordering::Relaxed))
    }

    /// Port of `DoorData.checkIfDoorsBetween`: does the segment cross a
    /// *closed* door's collision polygon within its z band? With
    /// `double_face_check` (the LOS variant) a single face crossing is
    /// tolerated — you can see "into" a doorway but not through both faces.
    pub fn check_doors_between(
        &self,
        x: i32,
        y: i32,
        z: i32,
        tx: i32,
        ty: i32,
        tz: i32,
        double_face_check: bool,
    ) -> bool {
        if self.doors.is_empty() {
            return false;
        }
        let region = (
            x >> crate::world::REGION_SHIFT,
            y >> crate::world::REGION_SHIFT,
        );
        let Some(nearby) = self.by_region.get(&region) else {
            return false;
        };

        for &idx in nearby {
            let door = &self.doors[idx as usize];
            if door.open.load(Ordering::Relaxed) {
                continue;
            }
            let mut intersect_face = false;
            for i in 0..4usize {
                let j = (i + 1) % 4;
                // Segment-segment intersection via the two line-equation
                // multipliers (integer denominator like Java — 0 = parallel).
                let denominator = ((ty - y) as i64 * (door.node_x[i] - door.node_x[j]) as i64)
                    - ((tx - x) as i64 * (door.node_y[i] - door.node_y[j]) as i64);
                if denominator == 0 {
                    continue;
                }
                let multiplier1 = (((door.node_x[j] - door.node_x[i]) as i64
                    * (y - door.node_y[i]) as i64)
                    - ((door.node_y[j] - door.node_y[i]) as i64 * (x - door.node_x[i]) as i64))
                    as f64
                    / denominator as f64;
                let multiplier2 = (((tx - x) as i64 * (y - door.node_y[i]) as i64)
                    - ((ty - y) as i64 * (x - door.node_x[i]) as i64))
                    as f64
                    / denominator as f64;
                if (0.0..=1.0).contains(&multiplier1) && (0.0..=1.0).contains(&multiplier2) {
                    let intersect_z = (z as f64 + (multiplier1 * (tz - z) as f64)).round() as i32;
                    if intersect_z > door.z_min && intersect_z < door.z_max {
                        if !double_face_check || intersect_face {
                            return true;
                        }
                        intersect_face = true;
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_with_door(open: bool) -> DoorGrid {
        let mut g = DoorGrid::default();
        // A door across the x axis: a thin wall segment from (100,-50) to
        // (100,50), z band [-100, 100].
        g.register(1, [98, 102, 102, 98], [-50, -50, 50, 50], -100, 100, open);
        g
    }

    #[test]
    fn closed_door_blocks_crossing_segment() {
        let g = grid_with_door(false);
        // Straight through the door plane (crosses both faces).
        assert!(g.check_doors_between(0, 0, 0, 200, 0, 0, false));
        assert!(g.check_doors_between(0, 0, 0, 200, 0, 0, true));
        // Segment ending before the door.
        assert!(!g.check_doors_between(0, 0, 0, 90, 0, 0, false));
        // Segment passing beside the door.
        assert!(!g.check_doors_between(0, 100, 0, 200, 100, 0, false));
        // Above the door's z band.
        assert!(!g.check_doors_between(0, 0, 500, 200, 0, 500, false));
    }

    #[test]
    fn open_door_blocks_nothing_and_flag_flips() {
        let g = grid_with_door(true);
        assert!(!g.check_doors_between(0, 0, 0, 200, 0, 0, false));
        assert!(g.is_open(1));
        g.set_open(1, false);
        assert!(!g.is_open(1));
        assert!(g.check_doors_between(0, 0, 0, 200, 0, 0, false));
    }

    #[test]
    fn double_face_check_tolerates_single_face() {
        let g = grid_with_door(false);
        // A segment ending *inside* the door box crosses only one face:
        // the movement variant blocks, the LOS variant does not.
        assert!(g.check_doors_between(0, 0, 0, 100, 0, 0, false));
        assert!(!g.check_doors_between(0, 0, 0, 100, 0, 0, true));
    }

    /// The engine-level wiring: a closed door blocks `can_see_target` /
    /// `can_move_to_target` and pins `get_valid_location` to the origin,
    /// even with no geodata loaded (`NullRegion` everywhere); opening it
    /// restores all three.
    #[test]
    fn geo_engine_queries_respect_doors() {
        let mut engine = crate::geo::GeoEngine::empty();
        engine
            .doors
            .register(7, [98, 102, 102, 98], [-50, -50, 50, 50], -100, 100, false);

        assert!(!engine.can_see_target(0, 0, 0, 200, 0, 0));
        assert!(!engine.can_move_to_target(0, 0, 0, 200, 0, 0));
        assert_eq!(engine.get_valid_location(0, 0, 0, 200, 0, 0), (0, 0, 0));

        engine.doors.set_open(7, true);
        assert!(engine.can_see_target(0, 0, 0, 200, 0, 0));
        assert!(engine.can_move_to_target(0, 0, 0, 200, 0, 0));
        assert_eq!(engine.get_valid_location(0, 0, 0, 200, 0, 0), (200, 0, 0));
    }
}
