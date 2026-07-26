//! Castle manor (G26) — the chamberlain's manor menu entry and the
//! `manor_menu_select` display bypass. Seven Signs is removed from this dist;
//! the manor is config-disabled (`AllowManor=False`) but fully wired so an
//! operator can enable it.

use super::*;

use crate::data::manor_data::Seed;
use crate::model::clan::Clan;
use crate::model::components::LastFolkNpc;
use crate::model::Player;

/// Gludio's Chamberlain of Light (35100) at the origin, plus an in-game player
/// standing on it. Returns the world and the player's packet receiver.
fn chamberlain_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, _db, _link) = quest_test_world();
    add_test_npc(&mut world, 701, 35100, "Merchant", 75, 0, 0, 0);
    let rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    (world, rx)
}

/// Make player 100 the leader of a clan that owns `castle_id` (so `isOwner` and
/// the `CS_MANOR_ADMIN` privilege both hold — the leader has every privilege).
fn own_castle(world: &mut World, castle_id: i32) {
    let clan_id = 500;
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "Owners".into(),
            leader_id: 100,
            level: 5,
            reputation_score: 0,
            castle_id,
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
        },
    );
    let p = world.objects.get_component_mut::<Player>(&100).unwrap();
    p.clan_id = clan_id;
}

fn served_html(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) -> Option<String> {
    drain(rx).iter().find_map(|p| decode_npc_html(p))
}

/// **The chamberlain's manor button gates on castle ownership specifically.**
/// A clan *leader* (who holds every privilege) who owns a *different* castle is
/// still refused at Gludio's chamberlain — it's the ownership check, not the
/// privilege check, doing the gating. The Gludio owner gets the console.
#[test]
fn manor_button_gates_on_ownership() {
    let (mut world, mut rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;

    // Leader of a clan that owns Dion (castle 2), not Gludio (castle 1) → the
    // privilege check passes (leader) but ownership does not → refusal page.
    own_castle(&mut world, 2);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest CastleChamberlain manor"),
    );
    let html = served_html(&mut rx).expect("a page is served to a non-owner");
    assert!(
        html.contains("not authorized"),
        "the wrong castle's owner sees chamberlain-21 (not authorized), got: {html}"
    );

    // Now that same clan owns Gludio (castle 1 — the chamberlain 35100's
    // castle) → the console.
    world.clans.get_mut(&500).unwrap().castle_id = 1;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest CastleChamberlain manor"),
    );
    let html = served_html(&mut rx).expect("a page is served to the owner");
    assert!(
        html.contains("manor_menu_select"),
        "the owner sees manor.html (its buttons send manor_menu_select), got: {html}"
    );
}

/// **The manor button also gates on the `CS_MANOR_ADMIN` privilege.** A clan
/// member who owns the castle but lacks the manor-admin privilege is refused.
#[test]
fn manor_button_gates_on_privilege() {
    let (mut world, mut rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    own_castle(&mut world, 1);
    // Demote player 100 from leader to a privilege-less member: ownership holds
    // but `CS_MANOR_ADMIN` does not.
    world.clans.get_mut(&500).unwrap().leader_id = 999;
    let p = world.objects.get_component_mut::<Player>(&100).unwrap();
    p.clan_privs = 0;

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest CastleChamberlain manor"),
    );
    let html = served_html(&mut rx).expect("a page is served");
    assert!(
        html.contains("not authorized"),
        "an owner without CS_MANOR_ADMIN is refused, got: {html}"
    );
}

/// **When the manor is disabled the button only chats "deactivated".** No
/// console page is served (Java's `player.sendMessage` branch).
#[test]
fn manor_button_deactivated_when_disabled() {
    let (mut world, mut rx) = chamberlain_world();
    world.cfg.general.allow_manor = false; // the dist default
    own_castle(&mut world, 1);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest CastleChamberlain manor"),
    );
    // No manor/console html is served — the manor branch returns nothing.
    assert!(
        served_html(&mut rx).is_none(),
        "a disabled manor serves no console page"
    );
}

/// **The manor "Seeds/Crops status" button sends the reference table.** Request
/// 5 (`manor_menu_select?ask=5`) → `ExShowManorDefaultInfo` with one line per
/// distinct crop id (Java `CastleManorManager.getCrops`).
#[test]
fn manor_menu_select_request5_sends_default_info() {
    let (mut world, mut rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    // Two distinct crops (5073, 5074) plus a duplicate of 5073 in another
    // castle — the reference table dedupes by crop id.
    world.data.manor.insert_for_test(seed(1, 5016, 5073, 10));
    world.data.manor.insert_for_test(seed(1, 5017, 5074, 11));
    world.data.manor.insert_for_test(seed(2, 5018, 5073, 10)); // dup crop 5073
                                                               // The manor menu resolves its NPC through the last folk NPC (the
                                                               // chamberlain the player just clicked).
    world.objects.add_components(&100, LastFolkNpc(701));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("manor_menu_select?ask=5&state=-1&time=0"),
    );
    let pkt = default_info_packet(&mut rx).expect("ExShowManorDefaultInfo was sent");
    // [0xFE][0x25 0x00][hideButtons][count i32 LE]…
    let count = i32::from_le_bytes(pkt[4..8].try_into().unwrap());
    assert_eq!(count, 2, "two distinct crops in the reference table");
}

/// **A disabled manor sends no reference table.** The `manor_menu_select`
/// bypass no-ops when `AllowManor=False`.
#[test]
fn manor_menu_select_gated_when_disabled() {
    let (mut world, mut rx) = chamberlain_world();
    world.cfg.general.allow_manor = false;
    world.data.manor.insert_for_test(seed(1, 5016, 5073, 10));
    world.objects.add_components(&100, LastFolkNpc(701));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("manor_menu_select?ask=5&state=-1&time=0"),
    );
    assert!(
        default_info_packet(&mut rx).is_none(),
        "no manor packet when the manor is disabled"
    );
}

fn seed(castle_id: i32, seed_id: i32, crop_id: i32, level: i32) -> Seed {
    Seed {
        castle_id,
        seed_id,
        crop_id,
        mature_id: 0,
        level,
        reward1: 1864,
        reward2: 1878,
        alternative: false,
        limit_seeds: 0,
        limit_crops: 0,
    }
}

/// Find the `ExShowManorDefaultInfo` packet (EX 0xFE, sub-op 0x25) among the
/// drained output.
fn default_info_packet(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) -> Option<Vec<u8>> {
    drain(rx)
        .into_iter()
        .find(|p| p.len() >= 8 && p[0] == 0xFE && p[1] == 0x25 && p[2] == 0x00)
}
