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

/// Port of `enums/Position` — where the attacker stands relative to the
/// target's facing (drives positional crit/hit bonuses and the proximity
/// damage bonus).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Front,
    Side,
    Back,
}

/// `Position.getPosition(from, to)`: compare the target's heading with the
/// from→to direction. `diff` near 0 (same facing, attacker behind) = BACK,
/// near 0x8000 (facing each other) = FRONT, the quarter-turn bands = SIDE.
pub fn get_position(from_x: i32, from_y: i32, to_x: i32, to_y: i32, to_heading: i32) -> Position {
    let heading_to = calculate_heading((to_x - from_x) as f64, (to_y - from_y) as f64);
    let diff = (to_heading - heading_to).abs();
    if (0x2000..=0x6000).contains(&diff) || ((diff - 0xA000) as u32 as u64) <= 0x4000 {
        Position::Side
    } else if ((diff - 0x2000) as u32 as u64) <= 0xC000 {
        Position::Front
    } else {
        Position::Back
    }
}

/// Advance one `MoveData` by the current tick, returning the interpolated
/// position and whether the move just completed.
fn advance(m: &MoveData, now: u64) -> (i32, i32, i32, bool) {
    let frac = ((now.saturating_sub(m.start_tick)) as f64 / m.total_ticks as f64).min(1.0);
    if frac >= 1.0 {
        (m.dest_x, m.dest_y, m.dest_z, true)
    } else {
        (
            m.start_x + ((m.dest_x - m.start_x) as f64 * frac).round() as i32,
            m.start_y + ((m.dest_y - m.start_y) as f64 * frac).round() as i32,
            m.start_z + ((m.dest_z - m.start_z) as f64 * frac).round() as i32,
            false,
        )
    }
}

/// `Creature.updatePosition`, geodata-free: advance every moving player one
/// tick. Called unconditionally every game-loop iteration (100ms), not gated
/// like the slower fixed-rate systems (e.g. regen) — movement needs to be
/// recomputed every tick to keep the server's authoritative position current.
pub fn tick(world: &mut crate::world::World) {
    let now = world.tick;
    for p in world.players.values_mut() {
        let Some(m) = p.move_data.clone() else { continue };
        let (x, y, z, arrived) = advance(&m, now);
        p.x = x;
        p.y = y;
        p.z = z;
        if arrived {
            p.move_data = None; // arrival needs no broadcast — client self-predicts (see plan).
        }
    }
}

/// The same per-tick advance for moving NPCs (G9 chase/return-home movement).
/// Returns the object ids of NPCs whose position changed, so the caller can
/// fire region re-indexing/visibility deltas (`visibility::update_npc_region`).
pub fn tick_npcs(world: &mut crate::world::World) -> Vec<i32> {
    let now = world.tick;
    let mut moved = Vec::new();
    for npc in world.npcs.values_mut() {
        let Some(m) = npc.move_data.clone() else { continue };
        let (x, y, z, arrived) = advance(&m, now);
        npc.x = x;
        npc.y = y;
        npc.z = z;
        if arrived {
            npc.move_data = None;
        }
        moved.push(npc.object_id);
    }
    moved
}
