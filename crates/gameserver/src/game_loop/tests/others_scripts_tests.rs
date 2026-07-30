//! `ai/others` slice 1 — the `multisell` NPC bypass and the three wandering
//! Mammon merchants.

use super::*;

use crate::data::MultisellData;
use crate::data::zone_data::ZoneData;
use crate::game_loop::area_npcs::{
    self, BLACKSMITH_OF_MAMMON, MERCHANT_OF_MAMMON, PRIEST_OF_MAMMON,
};
use crate::model::components::ActiveMultisell;

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

/// The Blacksmith of Mammon's first exchange list — `<npcs><npc>31126</npc>`,
/// i.e. openable *only* from him.
const BLACKSMITH_LIST: i32 = 31126001;

fn load_real_multisell_data(world: &mut World) {
    world.data.item_data = crate::data::ItemData::load_from(DIST);
    world.data.multisells = MultisellData::load_from(DIST, &world.data.item_data);
}

/// `bypass -h npc_<oid>_multisell 31126001` on the Blacksmith of Mammon opens
/// the window: the list is npc-restricted, so this only works because the
/// bypass passes the NPC through to `separateAndSend`.
#[test]
fn multisell_bypass_opens_an_npc_restricted_list() {
    let (mut world, ..) = test_world();
    load_real_multisell_data(&mut world);
    add_test_npc(
        &mut world,
        NPC_OID,
        BLACKSMITH_OF_MAMMON,
        "Merchant",
        70,
        100,
        0,
        0,
    );
    let mut rx = ingame_player(&mut world, 1, 8801, 60, 0, 0);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_multisell {BLACKSMITH_LIST}")),
    );

    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::MULTI_SELL_LIST),
        "the MultiSellList window is sent"
    );
    assert_eq!(
        world
            .objects
            .get_component::<ActiveMultisell>(&8801)
            .map(|a| a.list_id),
        Some(BLACKSMITH_LIST),
        "the open list is recorded for the follow-up MultiSellChoose"
    );
}

/// The same list from the npc-less community-board path stays closed — the
/// `<npcs>` allow-list is what the bypass has to satisfy, so this is the case
/// that fails if the npc is dropped on the way in.
#[test]
fn npc_restricted_list_is_refused_without_an_npc() {
    let (mut world, ..) = test_world();
    load_real_multisell_data(&mut world);
    let mut rx = ingame_player(&mut world, 1, 8802, 0, 0, 0);
    drain(&mut rx);

    crate::game_loop::multisell::separate_and_send(
        &mut world,
        1,
        8802,
        None,
        BLACKSMITH_LIST,
        false,
    );

    assert!(
        !drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MULTI_SELL_LIST),
        "an npc-only list opens for nobody without an npc"
    );
    assert!(
        world
            .objects
            .get_component::<ActiveMultisell>(&8802)
            .is_none(),
        "and nothing is recorded as open"
    );
}

/// A list opened from the *wrong* NPC is refused as well (Java's
/// `!isNpcAllowed(npc.getId())`).
#[test]
fn multisell_bypass_refuses_a_foreign_npc() {
    let (mut world, ..) = test_world();
    load_real_multisell_data(&mut world);
    // The Merchant of Mammon is not on 31126001's allow-list.
    add_test_npc(
        &mut world,
        NPC_OID,
        MERCHANT_OF_MAMMON,
        "Merchant",
        70,
        100,
        0,
        0,
    );
    let mut rx = ingame_player(&mut world, 1, 8803, 60, 0, 0);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_multisell {BLACKSMITH_LIST}")),
    );

    assert!(
        !drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MULTI_SELL_LIST),
        "the Blacksmith's list does not open from the Merchant"
    );
}

// --- The Mammons -----------------------------------------------------------

fn mammon_positions(world: &mut World, npc_id: i32) -> Vec<(i32, i32, i32)> {
    let mut out = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &Position)>(|(n, p)| {
            if n.npc_id == npc_id {
                out.push((p.x, p.y, p.z));
            }
        });
    out
}

fn register_mammon_templates(world: &mut World) {
    for npc_id in [MERCHANT_OF_MAMMON, BLACKSMITH_OF_MAMMON, PRIEST_OF_MAMMON] {
        let mut t = crate::data::npc_data::default_template(npc_id);
        t.type_name = "Merchant".into();
        world.data.npc_data.insert_for_test(t);
    }
}

/// Boot places exactly one of each Mammon, and the 30-minute beat moves them
/// instead of piling up copies (Java deletes `_lastSpawn` first).
#[test]
fn mammons_spawn_at_boot_and_relocate_without_duplicating() {
    let (mut world, _db, _l) = combat_test_world();
    register_mammon_templates(&mut world);

    area_npcs::spawn_at_boot(&mut world);
    for npc_id in [MERCHANT_OF_MAMMON, BLACKSMITH_OF_MAMMON, PRIEST_OF_MAMMON] {
        assert_eq!(
            mammon_positions(&mut world, npc_id).len(),
            1,
            "exactly one {npc_id} after boot"
        );
    }

    for _ in 0..5 {
        area_npcs::relocate_mammon(&mut world, MERCHANT_OF_MAMMON);
        assert_eq!(
            mammon_positions(&mut world, MERCHANT_OF_MAMMON).len(),
            1,
            "relocation never duplicates the Merchant"
        );
    }
}

/// The Priest of Mammon (33511) also has seven *static* spawns in the dist.
/// The script must delete the copy it placed — tracked in `World.mammon_spawns`
/// — not whichever 33511 it finds, or every relocation would eat a town NPC.
#[test]
fn relocating_the_priest_leaves_static_spawns_alone() {
    let (mut world, _db, _l) = combat_test_world();
    register_mammon_templates(&mut world);
    // A "static" Priest, as the spawn data would place him.
    add_test_npc(
        &mut world,
        NPC_OID,
        PRIEST_OF_MAMMON,
        "Merchant",
        70,
        111_385,
        220_888,
        -3536,
    );

    area_npcs::spawn_at_boot(&mut world);
    assert_eq!(
        mammon_positions(&mut world, PRIEST_OF_MAMMON).len(),
        2,
        "the static Priest plus the script's own"
    );

    for _ in 0..3 {
        area_npcs::relocate_mammon(&mut world, PRIEST_OF_MAMMON);
        assert_eq!(
            mammon_positions(&mut world, PRIEST_OF_MAMMON).len(),
            2,
            "still the static Priest plus exactly one script copy"
        );
    }
    assert!(
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&NPC_OID)
            .is_some(),
        "the static Priest is never the one deleted"
    );
}

/// `findNearestCastle` over the real siege zones: each Priest haunt names the
/// town it stands in.
#[test]
fn nearest_castle_resolves_the_mammon_haunts() {
    let zones = ZoneData::load_from(DIST);
    for (x, y, z, expected) in [
        (146882, 29665, -2264, 5), // Aden
        (81284, 150155, -3528, 3), // Giran
        (42784, -41236, -2192, 8), // Rune
    ] {
        assert_eq!(
            zones.nearest_castle_at(x, y, z),
            Some(expected),
            "({x}, {y}) belongs to castle {expected}"
        );
    }
}

/// With `AnnounceMammonSpawn` on, each relocation shouts the merchant's line
/// naming the castle it landed near. The roll is forced onto the Giran haunt.
#[test]
fn mammon_spawn_announces_the_nearest_castle() {
    let (mut world, _db, _l) = combat_test_world();
    register_mammon_templates(&mut world);
    world.data.zone_data = ZoneData::load_from(DIST);
    world.castles = vec![castle_row(3, "Giran"), castle_row(5, "Aden")];
    world.cfg.npc.announce_mammon_spawn = true;
    let mut rx = ingame_player(&mut world, 1, 8804, 0, 0, 0);
    drain(&mut rx);

    // Haunt index 1 = Giran.
    world.forced_rolls.push_back(1);
    area_npcs::relocate_mammon(&mut world, PRIEST_OF_MAMMON);

    let said: Vec<Vec<u8>> = drain(&mut rx)
        .into_iter()
        .filter(|p| p[0] == server_packets::opcodes::SAY2)
        .collect();
    assert_eq!(said.len(), 1, "one announcement per relocation");
    assert!(
        contains_utf16(
            &said[0],
            "Priest of Mammon has been spawned in Town of Giran."
        ),
        "the line names the castle nearest the haunt he picked"
    );

    // And with the config off, nothing is said.
    world.cfg.npc.announce_mammon_spawn = false;
    world.forced_rolls.push_back(1);
    area_npcs::relocate_mammon(&mut world, PRIEST_OF_MAMMON);
    assert!(
        !drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SAY2),
        "AnnounceMammonSpawn=False keeps the relocation quiet"
    );
}

fn castle_row(id: i32, name: &str) -> crate::model::castle::Castle {
    crate::model::castle::Castle {
        id,
        name: name.into(),
        side: Default::default(),
        ticket_buy_count: 0,
        time_registration_over: true,
        siege_date: 0,
    }
}
