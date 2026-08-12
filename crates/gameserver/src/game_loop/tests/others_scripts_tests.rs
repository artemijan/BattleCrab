//! `ai/others` slice 1 — the `multisell` NPC bypass and the three wandering
//! Mammon merchants.

use super::*;

use crate::data::zone_data::ZoneData;
use crate::game_loop::area_npcs::{
    self, BLACKSMITH_OF_MAMMON, MERCHANT_OF_MAMMON, PRIEST_OF_MAMMON,
};
use crate::model::components::ActiveMultisell;

const DIST: &str = crate::data::DIST_GAME;

/// The Blacksmith of Mammon's first exchange list — `<npcs><npc>31126</npc>`,
/// i.e. openable *only* from him.
const BLACKSMITH_LIST: i32 = 31126001;

/// `bypass -h npc_<oid>_multisell 31126001` on the Blacksmith of Mammon opens
/// the window: the list is npc-restricted, so this only works because the
/// bypass passes the NPC through to `separateAndSend`.
#[test]
fn multisell_bypass_opens_an_npc_restricted_list() {
    let (mut world, ..) = test_world();
    load_real_multisell_data(&mut world, DIST);
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
    load_real_multisell_data(&mut world, DIST);
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
    load_real_multisell_data(&mut world, DIST);
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
            insert_positions_for(&mut world, npc_id).len(),
            1,
            "exactly one {npc_id} after boot"
        );
    }

    for _ in 0..5 {
        area_npcs::relocate_mammon(&mut world, MERCHANT_OF_MAMMON);
        assert_eq!(
            insert_positions_for(&mut world, MERCHANT_OF_MAMMON).len(),
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
        insert_positions_for(&mut world, PRIEST_OF_MAMMON).len(),
        2,
        "the static Priest plus the script's own"
    );

    for _ in 0..3 {
        area_npcs::relocate_mammon(&mut world, PRIEST_OF_MAMMON);
        assert_eq!(
            insert_positions_for(&mut world, PRIEST_OF_MAMMON).len(),
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

// ---------------------------------------------------------------------------
// Slice 2 — the castle staff
// ---------------------------------------------------------------------------

use crate::model::clan::{CS_MERCENARIES, CS_OPEN_DOOR, Clan};
use crate::model::siege::{Siege, SiegeClanType};

/// Gludio's castle staff, at their real dist spawn points (inside the castle,
/// so `nearest_castle_at` resolves castle 1).
const GLUDIO_BLACKSMITH: i32 = 35098;
const GLUDIO_WAREHOUSE: i32 = 35099;
const GLUDIO_MERC_MANAGER: i32 = 35102;
const GLUDIO_DOORMAN_OUTER: i32 = 35096;
const BLACKSMITH_POS: (i32, i32, i32) = (-17680, 109519, -2656);
const DOORMAN_POS: (i32, i32, i32) = (-18452, 113261, -2750);
const GLUDIO: i32 = 1;

/// A world with the real castle zones + htmls, one castle, and a clan that owns
/// it whose leader is the test player.
fn castle_world(
    npc_id: i32,
    npc_pos: (i32, i32, i32),
) -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = quest_test_world();
    world.data.zone_data = ZoneData::load_from(DIST);
    world.castles = vec![castle_row(GLUDIO, "Gludio")];
    world.sieges.insert(GLUDIO, Siege::new(GLUDIO));
    add_test_npc(
        &mut world, NPC_OID, npc_id, "Folk", 70, npc_pos.0, npc_pos.1, npc_pos.2,
    );
    (world, db, l)
}

/// The lord of the castle: clan 77 owns Gludio and `player` leads it.
fn make_castle_lord(world: &mut World, player: i32) {
    let mut clan = mk_test_clan(77, player);
    clan.castle_id = GLUDIO;
    world.clans.insert(77, clan);
    let p = world
        .objects
        .get_component_mut::<crate::model::Player>(&player)
        .unwrap();
    p.clan_id = 77;
    p.clan_leader = true; // Java `isClanLeader()` = clan.leaderId == objectId
    p.clan_privs = 0; // the leader holds every privilege regardless
}

fn mk_test_clan(id: i32, leader_id: i32) -> Clan {
    Clan {
        id,
        name: format!("Clan{id}"),
        leader_id,
        level: 5,
        reputation_score: 0,
        castle_id: 0,
        members: Vec::new(),
        skills: Default::default(),
        warehouse: Default::default(),
        char_penalty_expiry_time: 0,
        dissolving_expiry_time: 0,
        rank_privs: Default::default(),
        new_leader_id: 0,
        sub_pledges: Default::default(),
        ally_id: 0,
        ally_name: String::new(),
        ally_penalty_expiry_time: 0,
        ally_penalty_type: 0,
        crest_id: 0,
        crest_large_id: 0,
        ally_crest_id: 0,
        blood_alliance_count: 0,
    }
}

/// Click an NPC (Java's target-then-interact pair) and return the html it sent.
fn talk_to_npc(world: &mut World, client_id: u32, rx: &mut PktRx) -> String {
    handle_action(world, client_id, &action_body(NPC_OID, 0));
    handle_action(world, client_id, &action_body(NPC_OID, 0));
    drain(rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .unwrap_or_default()
}

type PktRx = tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>;

/// The blacksmith serves the castle's lord and nobody else (Java `hasRights`).
#[test]
fn castle_blacksmith_answers_only_the_castle_lord() {
    let (mut world, _db, _l) = castle_world(GLUDIO_BLACKSMITH, BLACKSMITH_POS);
    let mut rx = ingame_player(
        &mut world,
        1,
        8810,
        BLACKSMITH_POS.0 + 40,
        BLACKSMITH_POS.1,
        BLACKSMITH_POS.2,
    );
    drain(&mut rx);

    // A clanless passer-by is turned away.
    let html = talk_to_npc(&mut world, 1, &mut rx);
    assert!(
        !html.is_empty() && !html.contains("Quest CastleBlacksmith"),
        "the refusal page has no console buttons: {html}"
    );

    // The castle's lord gets the console.
    make_castle_lord(&mut world, 8810);
    let html = talk_to_npc(&mut world, 1, &mut rx);
    assert!(
        html.contains("Quest CastleBlacksmith"),
        "the lord sees the console: {html}"
    );
}

/// The doorman works the gates for the owning clan, refuses everyone else, and
/// freezes while the castle is under siege.
#[test]
fn castle_doorman_opens_the_gates_except_during_a_siege() {
    use crate::data::door_data::DoorOpenMethod;
    let (mut world, _db, _l) = castle_world(GLUDIO_DOORMAN_OUTER, DOORMAN_POS);
    // Gludio's outer gate pair, named by the doorman's template parameters.
    let (door1, door2) = (19_210_001, 19_210_002);
    crate::model::door::spawn_door_for_test(&mut world, test_door(door1, DoorOpenMethod::None));
    crate::model::door::spawn_door_for_test(&mut world, test_door(door2, DoorOpenMethod::None));
    {
        let mut t = crate::data::npc_data::default_template(GLUDIO_DOORMAN_OUTER);
        t.type_name = "Folk".into();
        t.ai_params.insert("DoorId1".to_string(), door1.to_string());
        t.ai_params.insert("DoorId2".to_string(), door2.to_string());
        world.data.npc_data.insert_for_test(t);
    }
    let mut rx = ingame_player(
        &mut world,
        1,
        8811,
        DOORMAN_POS.0 + 40,
        DOORMAN_POS.1,
        DOORMAN_POS.2,
    );
    drain(&mut rx);

    // Not the owner: the gates stay shut.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest CastleDoorManager manageDoors 1"
        )),
    );
    assert!(!world.geo.doors.is_open(door1), "a stranger opens nothing");

    // The owning clan opens both gates.
    make_castle_lord(&mut world, 8811);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest CastleDoorManager manageDoors 1"
        )),
    );
    assert!(world.geo.doors.is_open(door1), "gate 1 opened");
    assert!(world.geo.doors.is_open(door2), "gate 2 opened");

    // Closing works the same way.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest CastleDoorManager manageDoors 0"
        )),
    );
    assert!(!world.geo.doors.is_open(door1), "gate 1 closed");

    // With the siege running the console refuses instead of toggling.
    world.sieges.get_mut(&GLUDIO).unwrap().in_progress = true;
    drain(&mut rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest CastleDoorManager manageDoors 1"
        )),
    );
    assert!(
        !world.geo.doors.is_open(door1),
        "the gates are frozen during a siege"
    );
}

/// The doorman's first-talk page needs `CS_OPEN_DOOR`, which the leader holds
/// implicitly but a plain member must be granted.
#[test]
fn doorman_first_talk_needs_the_open_door_privilege() {
    let (mut world, _db, _l) = castle_world(GLUDIO_DOORMAN_OUTER, DOORMAN_POS);
    let mut rx = ingame_player(
        &mut world,
        1,
        8812,
        DOORMAN_POS.0 + 40,
        DOORMAN_POS.1,
        DOORMAN_POS.2,
    );
    drain(&mut rx);
    // A member of the owning clan (not its leader) with no privileges.
    let mut clan = mk_test_clan(78, 9999);
    clan.castle_id = GLUDIO;
    world.clans.insert(78, clan);
    {
        let p = world
            .objects
            .get_component_mut::<crate::model::Player>(&8812)
            .unwrap();
        p.clan_id = 78;
        p.clan_privs = 0;
    }
    let refused = talk_to_npc(&mut world, 1, &mut rx);
    assert!(
        !refused.contains("manageDoors"),
        "no gate controls without CS_OPEN_DOOR: {refused}"
    );

    world
        .objects
        .get_component_mut::<crate::model::Player>(&8812)
        .unwrap()
        .clan_privs = CS_OPEN_DOOR;
    let allowed = talk_to_npc(&mut world, 1, &mut rx);
    assert!(
        allowed.contains("manageDoors"),
        "the privilege opens the console: {allowed}"
    );
}

/// The mercenary manager's limit page names the castle through the client
/// string `1001000 + residenceId` (Java's `%feud_name%` replacement).
#[test]
fn mercenary_manager_limit_page_names_the_castle() {
    let (mut world, _db, _l) = castle_world(GLUDIO_MERC_MANAGER, BLACKSMITH_POS);
    let mut rx = ingame_player(
        &mut world,
        1,
        8813,
        BLACKSMITH_POS.0 + 40,
        BLACKSMITH_POS.1,
        BLACKSMITH_POS.2,
    );
    make_castle_lord(&mut world, 8813);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest CastleMercenaryManager limit")),
    );
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("limit page");
    assert!(
        html.contains("1001001"),
        "Gludio's feud name (1001000 + 1): {html}"
    );
    assert!(!html.contains("%feud_name%"), "no leftover placeholder");
}

/// A plain member without `CS_MERCENARIES` is refused the console.
#[test]
fn mercenary_manager_needs_its_privilege() {
    let (mut world, _db, _l) = castle_world(GLUDIO_MERC_MANAGER, BLACKSMITH_POS);
    let mut rx = ingame_player(
        &mut world,
        1,
        8814,
        BLACKSMITH_POS.0 + 40,
        BLACKSMITH_POS.1,
        BLACKSMITH_POS.2,
    );
    let mut clan = mk_test_clan(79, 9999);
    clan.castle_id = GLUDIO;
    world.clans.insert(79, clan);
    {
        let p = world
            .objects
            .get_component_mut::<crate::model::Player>(&8814)
            .unwrap();
        p.clan_id = 79;
        p.clan_privs = 0;
    }
    drain(&mut rx);

    let refused = talk_to_npc(&mut world, 1, &mut rx);
    assert!(
        !refused.contains("Quest CastleMercenaryManager"),
        "no console without CS_MERCENARIES: {refused}"
    );

    world
        .objects
        .get_component_mut::<crate::model::Player>(&8814)
        .unwrap()
        .clan_privs = CS_MERCENARIES;
    let allowed = talk_to_npc(&mut world, 1, &mut rx);
    assert!(
        allowed.contains("Quest CastleMercenaryManager"),
        "the privilege opens the console: {allowed}"
    );
}

/// The warehouse keeper hands the castle's lord the Blood Alliances the clan
/// earned defending it — once; the counter is reset with the payout.
#[test]
fn castle_warehouse_pays_out_blood_alliances_once() {
    const BLOOD_ALLIANCE: i32 = 9911;
    let (mut world, mut db, _l) = castle_world(GLUDIO_WAREHOUSE, BLACKSMITH_POS);
    world.data.item_data = dist::items_owned();
    let mut rx = ingame_player(
        &mut world,
        1,
        8815,
        BLACKSMITH_POS.0 + 40,
        BLACKSMITH_POS.1,
        BLACKSMITH_POS.2,
    );
    make_castle_lord(&mut world, 8815);
    world.clans.get_mut(&77).unwrap().blood_alliance_count = 2;
    drain(&mut rx);
    drain_db(&mut db);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest CastleWarehouse Receive")),
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&8815)
            .map(|i| i.count_of(BLOOD_ALLIANCE))
            .unwrap_or(0),
        2,
        "both Blood Alliances handed over"
    );
    assert_eq!(
        world.clans.get(&77).unwrap().blood_alliance_count,
        0,
        "the clan's counter is spent"
    );

    // A second claim finds nothing left.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest CastleWarehouse Receive")),
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&8815)
            .map(|i| i.count_of(BLOOD_ALLIANCE))
            .unwrap_or(0),
        2,
        "no second payout"
    );
}

/// The 9 `ResidenceTeleportZone`s parse with their castle ids and oust points.
#[test]
fn residence_teleport_zones_load_for_every_castle() {
    let zones = ZoneData::load_from(DIST);
    for castle_id in 1..=9 {
        let zone = zones
            .residence_teleport_zone(castle_id)
            .unwrap_or_else(|| panic!("castle {castle_id} has a teleport zone"));
        assert!(zone.castle_id == castle_id, "the zone knows its castle");
        assert!(
            !zones.residence_teleport_spawns(castle_id).is_empty(),
            "castle {castle_id}'s zone carries its oust point"
        );
    }
}

/// `MASS_TELEPORT`: everyone standing in the owner-restart territory is pulled
/// to the inner castle; players outside it are left alone.
#[test]
fn mass_teleport_ousts_only_the_restart_territory() {
    let (mut world, _db, _l) = castle_world(35095, BLACKSMITH_POS);
    let zone_point = (-16700, 109300, -1700); // inside Gludio's restart polygon
    let outside = (-20000, 120000, -2000);
    let spawn = world.data.zone_data.residence_teleport_spawns(GLUDIO)[0];

    let mut rx_in = ingame_player(
        &mut world,
        1,
        8816,
        zone_point.0,
        zone_point.1,
        zone_point.2,
    );
    let mut rx_out = ingame_player(&mut world, 2, 8817, outside.0, outside.1, outside.2);
    drain(&mut rx_in);
    drain(&mut rx_out);

    crate::game_loop::area_npcs::handle_castle_mass_teleport(&mut world, NPC_OID);

    let pos_in = *world
        .objects
        .get_component::<Position>(&8816)
        .expect("player position");
    assert_eq!(
        (pos_in.x, pos_in.y),
        (spawn.0, spawn.1),
        "the player inside the territory was pulled to the oust point"
    );
    let pos_out = *world
        .objects
        .get_component::<Position>(&8817)
        .expect("player position");
    assert_eq!(
        (pos_out.x, pos_out.y),
        (outside.0, outside.1),
        "the player outside it stayed put"
    );
}

/// A castle teleporter only serves the owning clan's defenders **while their
/// siege runs** (Java's `getSiegeState() == 2`).
#[test]
fn castle_teleporter_serves_defenders_during_a_siege() {
    let (mut world, _db, _l) = castle_world(35092, BLACKSMITH_POS);
    let mut rx = ingame_player(
        &mut world,
        1,
        8818,
        BLACKSMITH_POS.0 + 40,
        BLACKSMITH_POS.1,
        BLACKSMITH_POS.2,
    );
    make_castle_lord(&mut world, 8818);
    drain(&mut rx);

    // Peacetime: the owner still gets the refusal page (no siege state).
    let peace = talk_to_npc(&mut world, 1, &mut rx);
    assert!(
        !peace.contains("teleportMe"),
        "no battlefield teleports outside a siege: {peace}"
    );

    // Siege running with the clan registered as the owner-defender.
    {
        let siege = world.sieges.get_mut(&GLUDIO).unwrap();
        siege.in_progress = true;
        siege.add_clan(77, SiegeClanType::Owner);
    }
    let at_war = talk_to_npc(&mut world, 1, &mut rx);
    assert!(
        at_war.contains("teleportMe"),
        "defenders get the posts: {at_war}"
    );
}

fn castle_row(id: i32, name: &str) -> crate::model::castle::Castle {
    crate::model::castle::Castle {
        show_npc_crest: false,
        id,
        name: name.into(),
        side: Default::default(),
        ticket_buy_count: 0,
        first_mid_victory: false,
        time_registration_over: true,
        siege_time_registration_end: 0,
        siege_date: 0,
        treasury: 0,
    }
}

// ---------------------------------------------------------------------------
// Slice 3 — the small combat behaviours
// ---------------------------------------------------------------------------

use crate::game_loop::quests;

/// Count the NPCs of a given template id in the world.
fn npc_count(world: &mut World, npc_id: i32) -> usize {
    let mut n = 0;
    world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &Position)>(|(npc, _)| {
            if npc.npc_id == npc_id {
                n += 1;
            }
        });
    n
}

/// An Ol Mahum Transcender sheds into its next stage on the chance roll, and
/// the new form is already hating whoever was hitting the old one.
#[test]
fn a_wounded_mob_polymorphs_into_its_next_form() {
    const TRANSCENDER_1: i32 = 21261;
    const TRANSCENDER_2: i32 = 21262;
    let (mut world, _db, _l) = combat_test_world();
    {
        let mut t = crate::data::npc_data::default_template(TRANSCENDER_2);
        t.type_name = "Monster".into();
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, TRANSCENDER_1, "Monster", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 8830, 60, 0, 0);

    // 20% chance → a roll of 0 fires it; the bark group rolls next.
    world.forced_rolls.extend([0, 0]);
    quests::notify_attack(&mut world, 8830, NPC_OID, TRANSCENDER_1, None, false);

    assert_eq!(
        npc_count(&mut world, TRANSCENDER_1),
        0,
        "the wounded form is gone"
    );
    assert_eq!(
        npc_count(&mut world, TRANSCENDER_2),
        1,
        "the next stage took its place"
    );
    // The newcomer inherited the fight.
    let mut hating = false;
    let mut new_oid = 0;
    world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &Position)>(|(npc, _)| {
            if npc.npc_id == TRANSCENDER_2 {
                new_oid = npc.object_id;
            }
        });
    if let Some(aggro) = world
        .objects
        .get_component::<crate::model::npc::AggroList>(&new_oid)
    {
        hating = aggro.0.iter().any(|(oid, _)| *oid == 8830);
    }
    assert!(hating, "the new form is set on the attacker");
}

/// The same swing with a failed chance roll changes nothing — the morph is a
/// chance, not a threshold.
#[test]
fn a_failed_chance_roll_leaves_the_mob_alone() {
    const TRANSCENDER_1: i32 = 21261;
    let (mut world, _db, _l) = combat_test_world();
    add_test_npc(&mut world, NPC_OID, TRANSCENDER_1, "Monster", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 8831, 60, 0, 0);

    // chance is 20 — a roll of 20 misses.
    world.forced_rolls.push_back(20);
    quests::notify_attack(&mut world, 8831, NPC_OID, TRANSCENDER_1, None, false);

    assert_eq!(
        npc_count(&mut world, TRANSCENDER_1),
        1,
        "the mob is still itself"
    );
    assert_eq!(npc_count(&mut world, 21262), 0, "nothing was spawned");
}

/// Killing one angel raises its twin on the corpse.
#[test]
fn killing_an_angel_raises_its_twin() {
    const ANGEL: i32 = 20830;
    const TWIN: i32 = 20859;
    let (mut world, _db, _l) = combat_test_world();
    {
        let mut t = crate::data::npc_data::default_template(TWIN);
        t.type_name = "Monster".into();
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, ANGEL, "Monster", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 8832, 60, 0, 0);

    quests::notify_kill(&mut world, 8832, NPC_OID, ANGEL, false);

    assert_eq!(npc_count(&mut world, TWIN), 1, "the twin rose");
}

/// The Timak leader calls in one private per successful roll, and stops at
/// three (Java's `countSpawnedMinions() < 3`).
#[test]
fn timak_leader_calls_privates_one_at_a_time() {
    const LEADER: i32 = 20767;
    const PRIVATE_A: i32 = 20768;
    const PRIVATE_B: i32 = 20769;
    let (mut world, _db, _l) = combat_test_world();
    {
        let mut t = crate::data::npc_data::default_template(LEADER);
        t.type_name = "Monster".into();
        t.ai_params
            .insert("SummonPrivateRate".to_string(), "100".to_string());
        for npc_id in [PRIVATE_A, PRIVATE_B] {
            t.minions.push(crate::data::npc_data::MinionHolder {
                npc_id,
                count: 1,
                group: "Privates".to_string(),
            });
        }
        world.data.npc_data.insert_for_test(t);
        for npc_id in [PRIVATE_A, PRIVATE_B] {
            let mut m = crate::data::npc_data::default_template(npc_id);
            m.type_name = "Monster".into();
            world.data.npc_data.insert_for_test(m);
        }
    }
    add_test_npc(&mut world, NPC_OID, LEADER, "Monster", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 8833, 60, 0, 0);

    // rate 100 always passes; the second roll picks the bark.
    world.forced_rolls.extend([0, 0]);
    quests::notify_attack(&mut world, 8833, NPC_OID, LEADER, None, false);
    assert_eq!(npc_count(&mut world, PRIVATE_A), 1, "one private answered");
    assert_eq!(
        npc_count(&mut world, PRIVATE_B),
        0,
        "only one per swing — not the whole group"
    );

    world.forced_rolls.extend([0, 0]);
    quests::notify_attack(&mut world, 8833, NPC_OID, LEADER, None, false);
    assert_eq!(
        npc_count(&mut world, PRIVATE_B),
        1,
        "the next swing calls the other private"
    );
}

/// An Elpy runs directly away from whoever hit it, roughly 500 units, and
/// commits to the walk (`MOVE_TO`) so the AI doesn't drag it back.
#[test]
fn an_elpy_flees_from_its_attacker() {
    const ELPY: i32 = 20432;
    let (mut world, _db, _l) = combat_test_world();
    add_test_npc(&mut world, NPC_OID, ELPY, "Monster", 20, 100, 0, 0);
    // The attacker stands to the west, so the mob should run east.
    let _rx = ingame_player(&mut world, 1, 8834, -100, 0, 0);

    quests::notify_attack(&mut world, 8834, NPC_OID, ELPY, None, false);

    let intention = world
        .objects
        .get_component::<crate::model::npc::NpcAi>(&NPC_OID)
        .map(|ai| ai.intention);
    assert_eq!(
        intention,
        Some(crate::model::npc::NpcIntention::MoveTo),
        "the mob commits to the flight"
    );
    let dest = world
        .objects
        .get_component::<crate::model::components::Movement>(&NPC_OID)
        .map(|m| (m.0.dest_x, m.0.dest_y));
    let (dx, _dy) = dest.expect("the mob is walking somewhere");
    assert!(
        dx > 100,
        "it ran away from the attacker, not towards them: {dest:?}"
    );
}

/// Felling a fairy tree from close up releases 20 guardians — on top of the
/// 20 that quest 421 (`Little Wing's Big Adventure`) swarms the killer with,
/// since Java registers **both** scripts on the same kill and this port now
/// does too. Beyond 1500 units neither reacts.
#[test]
fn a_felled_fairy_tree_releases_its_guardians() {
    const FAIRY_TREE: i32 = 27185;
    const SOUL_GUARDIAN: i32 = 27189;
    let (mut world, _db, _l) = combat_test_world();
    {
        let mut t = crate::data::npc_data::default_template(SOUL_GUARDIAN);
        t.type_name = "Monster".into();
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, FAIRY_TREE, "Monster", 40, 0, 0, 0);
    let _rx = ingame_player(&mut world, 1, 8835, 100, 0, 0);

    quests::notify_kill(&mut world, 8835, NPC_OID, FAIRY_TREE, false);
    assert_eq!(
        npc_count(&mut world, SOUL_GUARDIAN),
        40,
        "20 guardians from this script + quest 421's own 20"
    );
}

#[test]
fn a_fairy_tree_felled_from_afar_stays_quiet() {
    const FAIRY_TREE: i32 = 27186;
    const SOUL_GUARDIAN: i32 = 27189;
    let (mut world, _db, _l) = combat_test_world();
    {
        let mut t = crate::data::npc_data::default_template(SOUL_GUARDIAN);
        t.type_name = "Monster".into();
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, FAIRY_TREE, "Monster", 40, 0, 0, 0);
    let _rx = ingame_player(&mut world, 1, 8836, 2000, 0, 0);

    quests::notify_kill(&mut world, 8836, NPC_OID, FAIRY_TREE, false);
    assert_eq!(
        npc_count(&mut world, SOUL_GUARDIAN),
        0,
        "out of range (>1500), no revenge"
    );
}

/// A fairy tree is rooted where it stands (Java `setImmobilized(true)` +
/// `setRandomWalking(false)`).
#[test]
fn a_fairy_tree_is_immobile() {
    const FAIRY_TREE: i32 = 27187;
    let (mut world, _db, _l) = combat_test_world();
    {
        let mut t = crate::data::npc_data::default_template(FAIRY_TREE);
        t.type_name = "Monster".into();
        world.data.npc_data.insert_for_test(t);
    }
    let oid = crate::model::npc::spawn_npc_at(&mut world, FAIRY_TREE, 0, 0, 0, 0).expect("spawned");
    assert!(
        crate::game_loop::abnormal::is_movement_disabled(&world, oid),
        "the tree cannot move"
    );
}

/// The siege Headquarters is exempt from lethal blows — its script marks it,
/// and the `Lethal` effect honours the mark.
#[test]
fn the_siege_headquarters_ignores_a_lethal_blow() {
    const HEADQUARTERS: i32 = 35062;
    let (mut world, _db, _l) = combat_test_world();
    {
        let mut t = crate::data::npc_data::default_template(HEADQUARTERS);
        t.type_name = "Npc".into();
        world.data.npc_data.insert_for_test(t);
    }
    let oid =
        crate::model::npc::spawn_npc_at(&mut world, HEADQUARTERS, 0, 0, 0, 0).expect("spawned");
    assert!(
        world
            .objects
            .has_component::<crate::model::components::NotLethalable>(&oid),
        "the spawn hook marks it non-lethalable"
    );
}

// ---------------------------------------------------------------------------
// Slice 4 — day/night spawn groups + NoRandomActivity
// ---------------------------------------------------------------------------

use crate::data::spawn_data::{NpcSpawnDef, SpawnGroup, SpawnTemplate};
use crate::game_loop::spawn_scripts;

/// The dist really does ship the two spawn-script families this slice serves.
#[test]
fn the_dist_ships_day_night_templates_and_non_default_groups() {
    let spawns = dist::spawns();
    let day_night = spawns
        .spawns
        .iter()
        .filter(|t| t.ai.as_deref() == Some("DayNightSpawns"))
        .count();
    assert_eq!(day_night, 50, "DayNightSpawns templates");
    // Not 2 groups apiece: the Interlude map tiles ship their day and night
    // halves in separate files (one group each), and Devil's Isle has a
    // template with two `dayTime` groups. `manageSpawns` works per group, so
    // both shapes are fine.
    let phase_groups = spawns
        .spawns
        .iter()
        .flat_map(|t| &t.groups)
        .filter(|g| matches!(g.name.as_deref(), Some("dayTime") | Some("nightTime")))
        .count();
    assert_eq!(phase_groups, 94, "dayTime + nightTime groups");
    let non_default = spawns
        .spawns
        .iter()
        .flat_map(|t| &t.groups)
        .filter(|g| !g.spawn_by_default)
        .count();
    assert_eq!(
        non_default, 95,
        "groups a script owns rather than the boot pass"
    );
    assert!(
        spawns
            .spawns
            .iter()
            .any(|t| t.ai.as_deref() == Some("NoRandomActivity")
                && t.parameters.get("disableRandomWalk").map(String::as_str) == Some("true")),
        "the Chapel Guards template keeps its parameters"
    );
}

/// Build a template with the two phase groups, both `spawnByDefault=false`.
fn day_night_test_template(day_npc: i32, night_npc: i32) -> SpawnTemplate {
    let line = |npc_id: i32| NpcSpawnDef {
        npc_id,
        count: 1,
        loc: Some(crate::data::spawn_data::FixedLoc {
            x: 100,
            y: 100,
            z: 0,
            heading: 0,
        }),
        respawn_secs: 60,
        respawn_random_secs: 0,
        chase_range: 0,
        db_save: false,
    };
    SpawnTemplate {
        file: "test/day-night.xml".to_string(),
        name: Some("test-day-night".to_string()),
        ai: Some("DayNightSpawns".to_string()),
        parameters: Default::default(),
        territories: Vec::new(),
        groups: vec![
            SpawnGroup {
                name: Some("dayTime".to_string()),
                spawn_by_default: false,
                territories: Vec::new(),
                npcs: vec![line(day_npc)],
            },
            SpawnGroup {
                name: Some("nightTime".to_string()),
                spawn_by_default: false,
                territories: Vec::new(),
                npcs: vec![line(night_npc)],
            },
        ],
    }
}

fn register_monster(world: &mut World, npc_id: i32) {
    let mut t = crate::data::npc_data::default_template(npc_id);
    t.type_name = "Monster".into();
    world.data.npc_data.insert_for_test(t);
}

/// The phase swap: exactly one half stands at a time, and a transition
/// replaces it rather than stacking.
#[test]
fn only_the_in_phase_half_of_a_day_night_template_stands() {
    const DAY_MOB: i32 = 24052;
    const NIGHT_MOB: i32 = 24055;
    let (mut world, _db, _l) = combat_test_world();
    register_monster(&mut world, DAY_MOB);
    register_monster(&mut world, NIGHT_MOB);
    world
        .data
        .spawn_data
        .spawns
        .push(day_night_test_template(DAY_MOB, NIGHT_MOB));

    // Nightfall: the night half spawns, the day half is absent.
    spawn_scripts::on_day_night_change(&mut world, true);
    assert_eq!(npc_count(&mut world, NIGHT_MOB), 1, "night mob is out");
    assert_eq!(npc_count(&mut world, DAY_MOB), 0, "day mob is not");

    // Daybreak: they trade places — no stacking.
    spawn_scripts::on_day_night_change(&mut world, false);
    assert_eq!(npc_count(&mut world, DAY_MOB), 1, "day mob took over");
    assert_eq!(npc_count(&mut world, NIGHT_MOB), 0, "night mob went away");

    // And back again, still one apiece.
    spawn_scripts::on_day_night_change(&mut world, true);
    assert_eq!(npc_count(&mut world, NIGHT_MOB), 1);
    assert_eq!(npc_count(&mut world, DAY_MOB), 0);
}

/// The boot pass leaves both halves alone — before this slice it placed *both*,
/// so every day/night map stood with a double population.
#[test]
fn the_boot_pass_skips_groups_a_script_owns() {
    const DAY_MOB: i32 = 24052;
    const NIGHT_MOB: i32 = 24055;
    let (mut world, _db, _l) = combat_test_world();
    register_monster(&mut world, DAY_MOB);
    register_monster(&mut world, NIGHT_MOB);
    world
        .data
        .spawn_data
        .spawns
        .push(day_night_test_template(DAY_MOB, NIGHT_MOB));

    crate::model::npc::spawn_all(&mut world);

    assert_eq!(npc_count(&mut world, DAY_MOB), 0, "day half waits");
    assert_eq!(npc_count(&mut world, NIGHT_MOB), 0, "night half waits");
}

/// A mob killed just before its phase ended does not climb back out during the
/// other half: the scheduled respawn is refused while out of phase.
#[test]
fn an_out_of_phase_respawn_is_refused() {
    const DAY_MOB: i32 = 24052;
    const NIGHT_MOB: i32 = 24055;
    let (mut world, _db, _l) = combat_test_world();
    register_monster(&mut world, DAY_MOB);
    register_monster(&mut world, NIGHT_MOB);
    world
        .data
        .spawn_data
        .spawns
        .push(day_night_test_template(DAY_MOB, NIGHT_MOB));
    let template_idx = world.data.spawn_data.spawns.len() - 1;

    // Which half is in phase depends on the wall clock, so ask it.
    let night = crate::game_loop::game_time::is_night_at(commons::util::now_millis());
    let (in_phase_group, out_of_phase_group) = if night { (1, 0) } else { (0, 1) };
    let (in_phase_mob, out_of_phase_mob) = if night {
        (NIGHT_MOB, DAY_MOB)
    } else {
        (DAY_MOB, NIGHT_MOB)
    };

    crate::game_loop::death::handle_npc_respawn(&mut world, template_idx, out_of_phase_group, 0);
    assert_eq!(
        npc_count(&mut world, out_of_phase_mob),
        0,
        "the out-of-phase respawn is dropped"
    );

    crate::game_loop::death::handle_npc_respawn(&mut world, template_idx, in_phase_group, 0);
    assert_eq!(
        npc_count(&mut world, in_phase_mob),
        1,
        "the in-phase one still respawns"
    );
}

/// `NoRandomActivity`: the template's `disableRandomWalk` overrides the NPC
/// template's own flag, per spawned NPC.
#[test]
fn no_random_activity_pins_its_npcs_down() {
    const GUARD: i32 = 22138;
    let (mut world, _db, _l) = combat_test_world();
    {
        let mut t = crate::data::npc_data::default_template(GUARD);
        t.type_name = "Monster".into();
        t.random_walk = true;
        t.random_animation = true;
        world.data.npc_data.insert_for_test(t);
    }
    let mut template = day_night_test_template(GUARD, GUARD);
    template.ai = Some("NoRandomActivity".to_string());
    template
        .parameters
        .insert("disableRandomWalk".to_string(), "true".to_string());
    template.groups[0].spawn_by_default = true;
    world.data.spawn_data.spawns.push(template);
    let template_idx = world.data.spawn_data.spawns.len() - 1;

    let oid = crate::model::npc::spawn_one(&mut world, template_idx, 0, 0).expect("spawned");
    assert!(
        !spawn_scripts::random_walk_enabled(&world, oid, true),
        "the guard does not wander"
    );
    assert!(
        spawn_scripts::random_animation_enabled(&world, oid, true),
        "animations were not disabled, so the template flag stands"
    );
}

// ---------------------------------------------------------------------------
// Slice 5 — the talk/utility tail
// ---------------------------------------------------------------------------

const ADENA: i32 = 57;

fn adena_of(world: &mut World, player: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&player)
        .map(|i| i.count_of(ADENA))
        .unwrap_or(0)
}

fn give_adena(world: &mut World, player: i32, count: i64) {
    world.data.item_data = dist::items_owned();
    give_test_item(world, player, ADENA, count);
}

fn give_test_item(world: &mut World, player: i32, item_id: i32, count: i64) {
    let obj = world.next_npc_object_id;
    world.next_npc_object_id += 1;
    let World { objects, data, .. } = world;
    objects
        .get_component_mut::<Inventory>(&player)
        .unwrap()
        .add_item(&data.item_data, obj, item_id, count);
}

/// The arena attendant charges for the recovery up front and casts it two
/// seconds later; a broke customer gets the refusal and keeps their adena.
#[test]
fn arena_manager_charges_for_cp_recovery() {
    const ARENA_MANAGER: i32 = 31226;
    let (mut world, _db, _l) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, ARENA_MANAGER, "Folk", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 8850, 60, 0, 0);
    drain(&mut rx);

    // Broke: nothing is taken.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ArenaManager CPrecovery")),
    );
    assert_eq!(adena_of(&mut world, 8850), 0, "nothing to take");

    give_adena(&mut world, 8850, 5_000);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ArenaManager CPrecovery")),
    );
    assert_eq!(
        adena_of(&mut world, 8850),
        4_000,
        "1000 adena for the CP recovery"
    );
}

/// The buff package costs 2000 adena and casts all six arena buffs.
#[test]
fn arena_manager_sells_the_buff_package() {
    const ARENA_MANAGER: i32 = 31225;
    let (mut world, _db, _l) = quest_test_world();
    world.data.skill_data = dist::skills_owned();
    add_test_npc(&mut world, NPC_OID, ARENA_MANAGER, "Folk", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 8851, 60, 0, 0);
    give_adena(&mut world, 8851, 5_000);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ArenaManager Buff")),
    );
    assert_eq!(
        adena_of(&mut world, 8851),
        3_000,
        "2000 adena for the package"
    );
    let casts = drain(&mut rx)
        .iter()
        .filter(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE)
        .count();
    assert!(casts >= 6, "all six buffs are cast, saw {casts}");
}

/// The Tower of Insolence vortex eats one dimension stone per ride, and turns
/// the rider away when they have none.
#[test]
fn toi_vortex_trades_a_stone_for_a_ride() {
    const VORTEX: i32 = 30952;
    const GREEN_STONE: i32 = 4404;
    let (mut world, _db, _l) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, VORTEX, "Folk", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 8852, 60, 0, 0);
    world.data.item_data = dist::items_owned();
    drain(&mut rx);

    // No stone: the refusal page, and no teleport.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ToIVortex 1")),
    );
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .unwrap_or_default();
    assert!(!html.is_empty(), "the no-stones page is served");
    let pos = *world.objects.get_component::<Position>(&8852).unwrap();
    assert_eq!((pos.x, pos.y), (60, 0), "still standing at the vortex");

    // With a stone: it is spent and the rider lands on floor 1.
    give_test_item(&mut world, 8852, GREEN_STONE, 1);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ToIVortex 1")),
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&8852)
            .map(|i| i.count_of(GREEN_STONE))
            .unwrap_or(0),
        0,
        "the stone is spent"
    );
    let pos = *world.objects.get_component::<Position>(&8852).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (114356, 13423),
        "teleported to the first floor"
    );
}

/// The stone counter sells one green stone for 100k adena.
#[test]
fn toi_vortex_sells_dimension_stones() {
    const KEPLON: i32 = 30949;
    const GREEN_STONE: i32 = 4404;
    let (mut world, _db, _l) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, KEPLON, "Folk", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 8853, 60, 0, 0);
    give_adena(&mut world, 8853, 150_000);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ToIVortex GREEN")),
    );
    assert_eq!(adena_of(&mut world, 8853), 50_000, "100k for a stone");
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&8853)
            .map(|i| i.count_of(GREEN_STONE))
            .unwrap_or(0),
        1,
        "and the stone is handed over"
    );
}

/// The dye NPC's window opens at all — its `Draw`/`Remove` buttons were wired
/// long ago, but nothing served the page that carries them.
#[test]
fn symbol_maker_opens_the_dye_window() {
    const MARSDEN: i32 = 31046;
    let (mut world, _db, _l) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, MARSDEN, "Folk", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 8854, 60, 0, 0);
    drain(&mut rx);

    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("symbol maker window");
    assert!(
        html.contains("Draw") || html.contains("symbol_maker"),
        "the dye page, not the default chat window: {html}"
    );
}

/// A village guard strolls around its post and keeps the beat going. (`Guard`
/// NPCs have random walking off by default, which is why the script exists.)
#[test]
fn village_guards_stroll_around_their_post() {
    const GUARD: i32 = 31032;
    let (mut world, _db, _l) = combat_test_world();
    {
        let mut t = crate::data::npc_data::default_template(GUARD);
        t.type_name = "Guard".into();
        t.can_move = true;
        t.base_run_spd = 120.0;
        world.data.npc_data.insert_for_test(t);
    }
    let oid = crate::model::npc::spawn_npc_at(&mut world, GUARD, 0, 0, 0, 0).expect("spawned");

    // The spawn hook armed the first stroll; fire it.
    crate::game_loop::area_npcs::handle_guard_random_walk(&mut world, oid);
    assert!(
        world
            .objects
            .has_component::<crate::model::components::Movement>(&oid),
        "the guard set off"
    );
    // And the beat re-armed itself: the spawn hook queued one, firing it
    // queues the next (counting, because the first is still in the heap).
    let armed = world
        .scheduler
        .pending_tasks_for_test()
        .iter()
        .filter(|t| **t == crate::scheduler::ScheduledTask::GuardRandomWalk { npc_oid: oid })
        .count();
    assert_eq!(armed, 2, "the stroll beat keeps going");
}

// --- The Mammon economy: inventory-only (`exc_multisell`) exchange windows ---

use crate::game_loop::multisell::handle_multi_sell_choose;
use crate::model::inventory::Inventory;

/// `MultiSellChoose` body, with the enchant level the client echoes back for an
/// item-paired row.
fn choose_body(list_id: i32, entry_id: i32, amount: i64, enchant: i16) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(list_id);
    w.write_i32(entry_id);
    w.write_i64(amount);
    w.write_i16(enchant);
    w.write_i32(0); // augment 1
    w.write_i32(0); // augment 2
    for _ in 0..8 {
        w.write_i16(0);
    }
    w.into_bytes()
}

/// A world with the real catalogue and the Blacksmith of Mammon in front of
/// player 8801.
fn mammon_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = test_world();
    load_real_multisell_data(&mut world, DIST);
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
    world.id_pool = 0x7000_0000..0x7000_1000;
    let rx = ingame_player(&mut world, 1, 8801, 60, 0, 0);
    (world, rx)
}

fn rows_of(world: &World, player: i32) -> Vec<crate::model::components::PreparedRow> {
    world
        .objects
        .get_component::<ActiveMultisell>(&player)
        .map(|a| a.rows.clone())
        .unwrap_or_default()
}

/// **An `exc_multisell` window shows only what the player is carrying.** The
/// Blacksmith's SA-removal list has hundreds of entries; a player holding one
/// SA weapon (Stormbringer — Critical Anger, 4681) sees exactly the row for it,
/// carrying that instance's enchant level. The plain `multisell` verb still
/// opens the whole list.
#[test]
fn an_exchange_window_lists_only_the_players_own_items() {
    let (mut world, mut rx) = mammon_world();
    let oids = super::items::add_inventory_item(&mut world, 8801, 4681, 1).expect("SA weapon");
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&8801) {
        inv.set_item_enchant(oids[0], 5);
    }
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_exc_multisell {BLACKSMITH_LIST}")),
    );

    let rows = rows_of(&world, 8801);
    assert_eq!(rows.len(), 1, "only the held weapon's row is shown");
    assert_eq!(rows[0].item_object_id, oids[0], "paired to that instance");
    assert_eq!(rows[0].enchant_level, 5, "with its enchant level");

    // The unfiltered verb still opens everything.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_multisell {BLACKSMITH_LIST}")),
    );
    assert!(
        rows_of(&world, 8801).len() > 100,
        "`multisell` is unfiltered"
    );
}

/// **An equipped item is not offered for exchange** (Java skips
/// `item.isEquipped()`), so a player wearing their only SA weapon sees an empty
/// window.
#[test]
fn an_equipped_item_is_not_offered() {
    let (mut world, mut rx) = mammon_world();
    let oids = super::items::add_inventory_item(&mut world, 8801, 4681, 1).expect("SA weapon");
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&8801).unwrap();
        inv.equip_item(&data.item_data, oids[0]);
    }
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_exc_multisell {BLACKSMITH_LIST}")),
    );

    assert!(
        rows_of(&world, 8801).is_empty(),
        "an equipped weapon is not exchangeable"
    );
}

/// **The exchange consumes the exact instance the row was paired with.** With
/// two Stormbringers of different enchant in the bag, taking the row for the +0
/// one leaves the +5 alone — the case that fails if the ingredient is taken by
/// item id.
#[test]
fn the_exchange_consumes_the_paired_instance() {
    let (mut world, mut rx) = mammon_world();
    let plain = super::items::add_inventory_item(&mut world, 8801, 4681, 1).expect("first")[0];
    let enchanted = super::items::add_inventory_item(&mut world, 8801, 4681, 1).expect("second")[0];
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&8801) {
        inv.set_item_enchant(enchanted, 5);
    }
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_exc_multisell {BLACKSMITH_LIST}")),
    );
    let rows = rows_of(&world, 8801);
    assert_eq!(rows.len(), 2, "one row per instance");
    let plain_row = rows.iter().position(|r| r.item_object_id == plain).unwrap();

    handle_multi_sell_choose(
        &mut world,
        1,
        &choose_body(BLACKSMITH_LIST, plain_row as i32 + 1, 1, 0),
    );

    let inv = world.objects.get_component::<Inventory>(&8801).unwrap();
    assert!(
        inv.item_by_object_id(plain).is_none(),
        "the chosen instance is gone"
    );
    assert!(
        inv.item_by_object_id(enchanted).is_some(),
        "the other one is untouched"
    );
    assert_eq!(inv.count_of(72), 1, "and the plain Stormbringer arrived");
}

/// **A forged choose on an item-paired row is refused.** Java compares the
/// client's echoed stats against the paired item and drops the window; an amount
/// above 1 is refused the same way. Neither takes anything.
#[test]
fn a_mismatched_echo_is_refused() {
    let (mut world, mut rx) = mammon_world();
    let oid = super::items::add_inventory_item(&mut world, 8801, 4681, 1).expect("SA weapon")[0];
    drain(&mut rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_exc_multisell {BLACKSMITH_LIST}")),
    );

    // The row's item is +0; claim +9.
    handle_multi_sell_choose(&mut world, 1, &choose_body(BLACKSMITH_LIST, 1, 1, 9));
    assert!(
        world
            .objects
            .get_component::<Inventory>(&8801)
            .unwrap()
            .item_by_object_id(oid)
            .is_some(),
        "nothing was taken"
    );
    assert!(
        world
            .objects
            .get_component::<ActiveMultisell>(&8801)
            .is_none(),
        "and the window is dropped"
    );
}

/// **`maintainEnchantment` carries the enchant onto the new item.** List 1005
/// (the town blacksmiths' weapon-SA exchange) declares it, so a +7 Bow of
/// Peril comes back out as a +7 SA version.
#[test]
fn maintain_enchantment_carries_the_enchant_over() {
    let (mut world, ..) = test_world();
    load_real_multisell_data(&mut world, DIST);
    // Pinter (30298) is on list 1005's `<npcs>` allow-list.
    add_test_npc(&mut world, NPC_OID, 30298, "Merchant", 70, 100, 0, 0);
    world.id_pool = 0x7000_0000..0x7000_1000;
    let mut rx2 = ingame_player(&mut world, 1, 8802, 60, 0, 0);

    // The list's first entry: find an ingredient the player can hold.
    let (ingredient_id, product_id) = {
        let list = world.data.multisells.get(1005).expect("list 1005");
        let entry = &list.entries[0];
        (entry.ingredients[0].id, entry.products[0].id)
    };
    let oid = super::items::add_inventory_item(&mut world, 8802, ingredient_id, 1)
        .expect("ingredient")[0];
    // Everything else the entry wants (adena and the like).
    let extras: Vec<(i32, i64)> = {
        let list = world.data.multisells.get(1005).expect("list 1005");
        list.entries[0]
            .ingredients
            .iter()
            .skip(1)
            .map(|i| (i.id, i.count * 10))
            .collect()
    };
    for (id, count) in extras {
        super::items::add_inventory_item(&mut world, 8802, id, count);
    }
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&8802) {
        inv.set_item_enchant(oid, 7);
    }
    drain(&mut rx2);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_exc_multisell 1005")),
    );
    let rows = rows_of(&world, 8802);
    let row = rows
        .iter()
        .position(|r| r.item_object_id == oid)
        .expect("the held weapon has a row");

    handle_multi_sell_choose(&mut world, 1, &choose_body(1005, row as i32 + 1, 1, 7));

    let inv = world.objects.get_component::<Inventory>(&8802).unwrap();
    let produced = inv
        .items()
        .iter()
        .find(|i| i.item_id == product_id)
        .expect("the exchanged weapon");
    assert_eq!(
        produced.enchant_level, 7,
        "the enchant level came along (maintainEnchantment)"
    );
}

/// `ItemAction`'s mercenary-ticket refusal: a posting ticket lying inside a
/// castle's siege zone can only be picked up by an owning-clan member with
/// `CS_MERCENARIES` — everyone else gets SM 654 and no pickup walk. The gate
/// has no siege-active requirement, as in Java.
#[test]
fn mercenary_ticket_pickup_needs_the_privilege() {
    use crate::game_loop::ground_items::{DropSource, spawn_ground_item};
    use crate::model::components::Intent;

    let (mut world, _db, _l, _link) = test_world();
    insert_siege_zone(&mut world, GLUDIO, -500, 500, -500, 500);
    world.data.castle_siege_guards.insert_for_test(3960, GLUDIO);
    let mut rx = ingame_player(&mut world, 1, 8815, 0, 0, 0);
    let ticket = spawn_ground_item(&mut world, 3960, 1, 0, 50, 0, 0, 8815, DropSource::Player);
    drain(&mut rx);

    // A clanless passer-by: refused, no walk starts.
    on_packet(
        &mut world,
        1,
        [vec![cop::ACTION], action_body(ticket, 0)].concat(),
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(&654),
        "SM 654: no authority to cancel mercenary positioning"
    );
    assert!(
        !world.objects.has_component::<Intent>(&8815),
        "no pickup intent for the unprivileged"
    );

    // Owning-clan member with CS_MERCENARIES: the pickup walk starts.
    let mut clan = mk_test_clan(79, 9999);
    clan.castle_id = GLUDIO;
    world.clans.insert(79, clan);
    {
        let p = world
            .objects
            .get_component_mut::<crate::model::Player>(&8815)
            .unwrap();
        p.clan_id = 79;
        p.clan_privs = CS_MERCENARIES;
    }
    on_packet(
        &mut world,
        1,
        [vec![cop::ACTION], action_body(ticket, 0)].concat(),
    );
    assert!(
        world.objects.has_component::<Intent>(&8815),
        "the privileged owner walks to the ticket"
    );
}
