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

pub fn dist3d_xyz(x1: i32, y1: i32, z1: i32, x2: i32, y2: i32, z2: i32) -> f64 {
    (((x2 - x1) as f64).powi(2) + ((y2 - y1) as f64).powi(2) + ((z2 - z1) as f64).powi(2)).sqrt()
}

pub fn distance_2d_xy(tx: i32, ty: i32, cx: i32, cy: i32) -> f64 {
    (((tx - cx) as f64).powi(2) + ((ty - cy) as f64).powi(2)).sqrt()
}
