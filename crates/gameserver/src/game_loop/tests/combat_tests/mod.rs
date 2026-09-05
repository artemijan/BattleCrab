mod attack;
mod death;
mod npc_ai;
mod pvp;
mod siege;
mod siege_capture;
mod siege_defence;

use super::*;
use crate::game_loop;
use crate::game_loop::npc;

/// A `RequestRestartPoint` body for the given point type.
fn restart_to(point_type: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(point_type);
    w.into_bytes()
}
