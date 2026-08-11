use crate::model::components::Position;
use crate::world::World;

pub fn position_of(world: &World, oid: i32) -> Option<(i32, i32, i32)> {
    world
        .objects
        .get_component::<Position>(&oid)
        .map(|p| (p.x, p.y, p.z))
}
pub fn distance_2d(world: &World, a: i32, b: i32) -> Option<f64> {
    let (ax, ay, _) = position_of(world, a)?;
    let (bx, by, _) = position_of(world, b)?;
    Some(distance_2d_xy(ax, ay, bx, by))
}

/// `Util.calculateDistance(…, includeZAxis = true)` — what an NPC's cast-range
/// gate measures.
pub fn distance_3d(world: &World, a: i32, b: i32) -> Option<f64> {
    let (ax, ay, az) = position_of(world, a)?;
    let (bx, by, bz) = position_of(world, b)?;
    Some(dist3d_xyz(ax, ay, az, bx, by, bz))
}

/// `Util.checkIfInRange(range, a, b, includeZAxis = false)` — the flat-range
/// gate, **inclusive** of `range`.
///
/// `false` when either object has left the world, which every caller wants: a
/// departed object is not in range of anything.
///
/// Note the axis: this deliberately ignores z, so a target directly overhead
/// counts as near. Reach for [`within_3d`] where height matters.
pub fn within_2d(world: &World, a: i32, b: i32, range: f64) -> bool {
    distance_2d(world, a, b).is_some_and(|d| d <= range)
}

/// `Util.checkIfInRange(range, a, b, includeZAxis = true)` — the [`within_2d`]
/// counterpart that measures height too, inclusive of `range`.
///
/// Callers needing a *strict* `<` keep their own comparison: Java is not
/// consistent about the boundary, and `death::resurrect` in particular ports
/// `calculateDistance3D(this) < ALT_PARTY_RANGE`.
pub fn within_3d(world: &World, a: i32, b: i32, range: f64) -> bool {
    distance_3d(world, a, b).is_some_and(|d| d <= range)
}

/// [`within_2d`] measured against a bare point instead of a second object —
/// the "everyone standing within `range` of this spot" gate that radius sweeps
/// (admin effects, boss aggro counts, loot distribution) each spelled out by
/// hand.
///
/// `false` when the object has left the world, same as [`within_2d`].
pub fn within_2d_xy(world: &World, oid: i32, x: i32, y: i32, range: f64) -> bool {
    position_of(world, oid).is_some_and(|(ox, oy, _)| distance_2d_xy(ox, oy, x, y) <= range)
}

/// [`within_3d`] measured against a bare point — the height-aware twin of
/// [`within_2d_xy`], for the sweeps whose origin is a captured `Position`
/// rather than a live object.
///
/// `false` when the object has left the world, same as [`within_3d`].
pub fn within_3d_xyz(world: &World, oid: i32, x: i32, y: i32, z: i32, range: f64) -> bool {
    position_of(world, oid).is_some_and(|(ox, oy, oz)| dist3d_xyz(ox, oy, oz, x, y, z) <= range)
}

pub fn dist3d_xyz(x1: i32, y1: i32, z1: i32, x2: i32, y2: i32, z2: i32) -> f64 {
    (((x2 - x1) as f64).powi(2) + ((y2 - y1) as f64).powi(2) + ((z2 - z1) as f64).powi(2)).sqrt()
}

pub fn distance_2d_xy(tx: i32, ty: i32, cx: i32, cy: i32) -> f64 {
    (((tx - cx) as f64).powi(2) + ((ty - cy) as f64).powi(2)).sqrt()
}
