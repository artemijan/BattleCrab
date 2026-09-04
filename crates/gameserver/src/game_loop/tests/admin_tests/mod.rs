//! GM command tests, split to mirror the `game_loop::admin` modules they
//! exercise; the helpers more than one area needs live here.

mod castle;
mod character;
mod cursed_weapons;
mod effects;
mod guard;
mod items;
mod skills;
mod spawn;
mod teleport;
mod transforms;
mod world;

use super::*;
use crate::game_loop;
use crate::game_loop::admin;
use crate::game_loop::character::inventory;
use crate::game_loop::space::position::set_position;

/// The html body of the most recent `NpcHtmlMessage` in `packets`, if any.
fn last_admin_html(packets: &[Vec<u8>]) -> Option<String> {
    let pkt = packets
        .iter()
        .rev()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)?;
    let mut r = commons::network::PacketReader::new(&pkt[1..]);
    r.read_i32()?; // object id (0 for admin pages)
    r.read_string()
}

fn scan_npc(world: &mut World, oid: i32, gm: i32, dx: i32, dy: i32, dz: i32) {
    const SCAN_MOB: i32 = 47000;
    if world.data.npc_data.get(SCAN_MOB).is_none() {
        let mut t = crate::data::npc_data::default_template(SCAN_MOB);
        t.type_name = "Monster".into();
        t.name = "Scan Target".into();
        world.data.npc_data.insert_for_test(t);
    }
    let pos = world
        .objects
        .get_component::<Position>(&gm)
        .copied()
        .unwrap();
    add_test_npc(
        world,
        oid,
        SCAN_MOB,
        "Monster",
        20,
        pos.x + dx,
        pos.y + dy,
        pos.z + dz,
    );
}

fn has_admin_html(pkts: &[Vec<u8>]) -> bool {
    pkts.iter()
        .any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
}
