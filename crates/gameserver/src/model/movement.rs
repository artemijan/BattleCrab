//! Port of `Creature.MoveData`/`Creature.moveToLocation`: straight-line
//! interpolation from a start point to a destination over a precomputed tick
//! count, plus multi-segment route following when the move carries a geodata
//! path (`geoPath`/`onGeodataPathIndex`/`moveToNextRoutePoint`). Java's
//! `disregardingGeodata` flag has no equivalent — the conditions it tracks
//! (inactive region, flying, water, vehicles) don't exist yet.

/// The geodata route attached to a pathfound move — Java `MoveData.geoPath` +
/// `onGeodataPathIndex`/`geoPathAccurateTx`/`geoPathAccurateTy`/`geoPathGtx`/
/// `geoPathGty` flattened into one struct (present ⇔ the Java index != -1).
#[derive(Debug, Clone, PartialEq)]
pub struct GeoPath {
    /// Route points in world coordinates (`CellPathFinding.findPath` output).
    pub points: Vec<(i32, i32, i32)>,
    /// Index of the point the current segment is heading to.
    pub index: usize,
    /// The player-requested destination — the final segment aims here rather
    /// than at the last cell-centre route point.
    pub accurate_tx: i32,
    pub accurate_ty: i32,
    /// Geo cell of the requested destination, for the "re-click on the cell
    /// we are already pathing to" ignore check.
    pub gtx: i32,
    pub gty: i32,
}

impl GeoPath {
    /// Java `isOnGeodataPath()`: false once the final route point is the
    /// active segment — arrival there ends the move.
    pub fn has_next(&self) -> bool {
        self.index != self.points.len() - 1
    }
}

/// Java `Creature.MoveData`. `Some` on the `Movement` component ⇔ the
/// creature is currently moving (mirrors Java's nullable `_move`).
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
    /// effect when the move started. Fixed for one segment — a route advance
    /// recomputes it at the speed in effect then.
    pub total_ticks: u64,
    /// The remaining pathfound route, `None` for plain straight moves.
    pub geo_path: Option<GeoPath>,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// What `tick` observed, so the caller can fire the follow-up packets/events.
#[derive(Debug, Default)]
pub struct TickOutcome {
    /// NPCs whose position changed (region re-index/visibility deltas).
    pub moved_npcs: Vec<i32>,
    /// Movers that finished a route segment and started the next one — the
    /// caller broadcasts `MoveToLocation` for the new segment (Java
    /// `moveToNextRoutePoint` → `broadcastMoveToLocation`).
    pub route_advanced: Vec<i32>,
    /// Movers that reached their destination this tick — Java's
    /// `CreatureAI.onEvtArrived`. Only the `AI_INTENTION_MOVE_TO` → `ACTIVE`
    /// reset is ported off it (see `visibility::movement_tick`).
    pub arrived: Vec<i32>,
}

/// `Creature.updatePosition`: advance **every** mover — player or NPC — one
/// tick in a single presence-filtered sweep (only entities carrying
/// `Movement` are visited; arrivals are collected and their component
/// removed after the iteration). Called unconditionally every game-loop
/// iteration (100 ms), not gated like the slower fixed-rate systems —
/// movement must keep the server's authoritative position current.
/// A mover that finishes a segment of a geodata path advances to the next
/// route point instead of arriving (`Creature.moveToNextRoutePoint`: new
/// destination, ticks recomputed at the current speed, heading updated); the
/// final segment aims at the accurate requested destination. Plain arrivals
/// need no broadcast — the client self-predicts.
pub fn tick(world: &mut crate::world::World) -> TickOutcome {
    use crate::model::components::{Movement, Position, Speeds};
    let now = world.tick;
    let mut out = TickOutcome::default();
    let mut arrived: Vec<i32> = Vec::new();
    world.objects.for_each_mut::<(
        &mut Movement,
        &mut Position,
        Option<&Speeds>,
        Option<&crate::model::Player>,
        Option<&crate::model::npc::Npc>,
    )>(|(mut m, mut pos, speeds, player, npc)| {
        let (x, y, z, done) = advance(&m.0, now);
        pos.x = x;
        pos.y = y;
        pos.z = z;
        let object_id = player.map(|p| p.object_id).or(npc.map(|n| n.object_id));
        if let Some(npc) = npc {
            out.moved_npcs.push(npc.object_id);
        }
        if done {
            let speed = speeds.map(Speeds::move_speed).unwrap_or(0.0);
            if let Some(id) = object_id {
                if advance_route(&mut m.0, &mut pos, speed, now) {
                    out.route_advanced.push(id);
                } else {
                    arrived.push(id);
                }
            }
        }
    });
    for &id in &arrived {
        world.objects.remove_component::<Movement>(&id);
    }
    out.arrived = arrived;
    out
}

/// `Creature.moveToNextRoutePoint`, minus the packet send (the caller's job):
/// rewrite `m` in place for the next segment of its geodata path. Returns
/// false — final arrival — when there is no next route point or the mover
/// can no longer move (speed ≤ 0).
fn advance_route(
    m: &mut MoveData,
    pos: &mut crate::model::components::Position,
    speed: f64,
    now: u64,
) -> bool {
    let Some(path) = &mut m.geo_path else { return false };
    if !path.has_next() || speed <= 0.0 {
        return false;
    }
    path.index += 1;
    // The last segment heads to the accurate (requested) destination, not
    // the cell-centre route point.
    let (dest_x, dest_y, dest_z) = if path.index == path.points.len() - 1 {
        (path.accurate_tx, path.accurate_ty, path.points[path.index].2)
    } else {
        path.points[path.index]
    };
    m.start_x = pos.x;
    m.start_y = pos.y;
    m.start_z = pos.z;
    m.dest_x = dest_x;
    m.dest_y = dest_y;
    m.dest_z = dest_z;
    m.start_tick = now;
    let dx = (dest_x - pos.x) as f64;
    let dy = (dest_y - pos.y) as f64;
    let distance = (dx * dx + dy * dy).sqrt();
    m.total_ticks = ((10.0 * distance / speed).round() as u64).max(1);
    if distance != 0.0 {
        pos.heading = calculate_heading(dx, dy);
    }
    true
}
