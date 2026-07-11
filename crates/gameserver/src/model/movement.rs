//! Port of `Creature.MoveData`/`Creature.moveToLocation`, narrowed to what a
//! no-geodata milestone needs: straight-line interpolation from a start point
//! to a destination over a precomputed tick count. Everything geodata/
//! pathfinding-related in the Java `MoveData` (`disregardingGeodata`,
//! `onGeodataPathIndex`, `geoPath`, …) is out of scope — see `docs/PROGRESS.md`
//! G7's deferred-TODO note.

/// Java `Creature.MoveData`, geodata fields stripped. `Some` on `Player.move_data`
/// ⇔ the player is currently moving (mirrors Java's nullable `_move`).
#[derive(Debug, Clone, PartialEq)]
pub struct MoveData {
    pub start_x: i32,
    pub start_y: i32,
    pub start_z: i32,
    pub dest_x: i32,
    pub dest_y: i32,
    pub dest_z: i32,
    /// `world.tick` when the move began.
    pub start_tick: u64,
    /// Ticks (100ms each, matching `Formulas.TICKS_PER_SECOND` = `world`'s own
    /// 10-ticks/sec loop) needed to cover the full distance at the speed in
    /// effect when the move started. Fixed for the whole move — no geodata
    /// path corrections to recompute mid-flight.
    pub total_ticks: u64,
}

/// Port of `Util.calculateHeadingFrom(double dx, double dy)`.
pub fn calculate_heading(dx: f64, dy: f64) -> i32 {
    let mut angle_target = dy.atan2(dx).to_degrees();
    if angle_target < 0.0 {
        angle_target += 360.0;
    }
    (angle_target * 182.044444444) as i32
}

/// `Creature.updatePosition`, geodata-free: advance every moving player one
/// tick. Called unconditionally every game-loop iteration (100ms), not gated
/// like the slower fixed-rate systems (e.g. regen) — movement needs to be
/// recomputed every tick to keep the server's authoritative position current.
pub fn tick(world: &mut crate::world::World) {
    let now = world.tick;
    for p in world.players.values_mut() {
        let Some(m) = p.move_data.clone() else { continue };
        let frac = ((now.saturating_sub(m.start_tick)) as f64 / m.total_ticks as f64).min(1.0);
        if frac >= 1.0 {
            p.x = m.dest_x;
            p.y = m.dest_y;
            p.z = m.dest_z;
            p.move_data = None; // arrival needs no broadcast — client self-predicts (see plan).
        } else {
            p.x = m.start_x + ((m.dest_x - m.start_x) as f64 * frac).round() as i32;
            p.y = m.start_y + ((m.dest_y - m.start_y) as f64 * frac).round() as i32;
            p.z = m.start_z + ((m.dest_z - m.start_z) as f64 * frac).round() as i32;
        }
    }
}
