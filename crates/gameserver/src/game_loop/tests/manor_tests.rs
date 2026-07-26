//! Castle manor (G26) — the chamberlain's manor menu entry and the
//! `manor_menu_select` display bypass. Seven Signs is removed from this dist;
//! the manor is config-disabled (`AllowManor=False`) but fully wired so an
//! operator can enable it.

use super::*;

use crate::data::item_data::ADENA_ID;
use crate::data::manor_data::Seed;
use crate::model::clan::Clan;
use crate::model::components::LastFolkNpc;
use crate::model::inventory::Inventory;
use crate::model::manor::{CropProcure, ManorMode, SeedProduction};
use crate::model::Player;

/// Register + place a Manor Manager (a Merchant with a `manor_id` param) and
/// make it the player's last folk NPC (so the trader gate passes).
fn add_manor_manager(world: &mut World, oid: i32, npc_id: i32, manor_id: i32) {
    let mut t = crate::data::npc_data::default_template(npc_id);
    t.type_name = "Merchant".into();
    t.level = 75;
    t.base_hp_max = 100.0;
    t.base_mp_max = 50.0;
    t.ai_params.insert("manor_id".into(), manor_id.to_string());
    world.data.npc_data.insert_for_test(t);
    add_test_npc(world, oid, npc_id, "Merchant", 75, 0, 0, 0);
    world.objects.add_components(&100, LastFolkNpc(oid));
}

fn inv_count(world: &World, item_id: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&100)
        .map_or(0, |i| i.count_of(item_id))
}

/// A player buys seeds at a Manor Manager: adena leaves, the seeds arrive, and
/// the manor's current-period stock drops by the amount bought.
#[test]
fn buy_seed_trades_adena_for_seeds_and_decrements_stock() {
    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    // Gludio Manor Manager (35103), manor_id 1. Seed 5016 is a real item.
    add_manor_manager(&mut world, 702, 35103, 1);
    world.data.manor.insert_for_test(seed(1, 5016, 5073, 10));
    world.manor.set_seed_production(
        1,
        false,
        vec![SeedProduction {
            seed_id: 5016,
            amount: 500,
            price: 10,
            start_amount: 500,
        }],
    );
    super::items::add_inventory_item(&mut world, 100, ADENA_ID, 1_000);

    // Buy 5 of seed 5016 (price 10 → 50 adena).
    let mut w = PacketWriter::new();
    w.write_i32(1); // manor id
    w.write_i32(1); // count
    w.write_i32(5016);
    w.write_i64(5);
    crate::game_loop::manor::handle_request_buy_seed(&mut world, 1, &w.into_bytes());

    assert_eq!(inv_count(&world, 5016), 5, "the buyer received 5 seeds");
    assert_eq!(inv_count(&world, ADENA_ID), 950, "50 adena was taken");
    assert_eq!(
        world.manor.seed_product(1, 5016, false).unwrap().amount,
        495,
        "the manor's stock dropped by 5"
    );
}

/// The purchase is refused (no adena taken, no stock change) when the buyer
/// can't afford it.
#[test]
fn buy_seed_refused_without_adena() {
    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    add_manor_manager(&mut world, 702, 35103, 1);
    world.data.manor.insert_for_test(seed(1, 5016, 5073, 10));
    world.manor.set_seed_production(
        1,
        false,
        vec![SeedProduction {
            seed_id: 5016,
            amount: 500,
            price: 10,
            start_amount: 500,
        }],
    );
    super::items::add_inventory_item(&mut world, 100, ADENA_ID, 10); // far short of 50

    let mut w = PacketWriter::new();
    w.write_i32(1);
    w.write_i32(1);
    w.write_i32(5016);
    w.write_i64(5);
    crate::game_loop::manor::handle_request_buy_seed(&mut world, 1, &w.into_bytes());

    assert_eq!(inv_count(&world, 5016), 0, "no seeds delivered");
    assert_eq!(inv_count(&world, ADENA_ID), 10, "no adena taken");
    assert_eq!(
        world.manor.seed_product(1, 5016, false).unwrap().amount,
        500,
        "stock unchanged"
    );
}

/// Buying more than the manor stocks is refused outright (Java's
/// `sp.getAmount() < count` guard).
#[test]
fn buy_seed_refused_when_overdrawing_stock() {
    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    add_manor_manager(&mut world, 702, 35103, 1);
    world.data.manor.insert_for_test(seed(1, 5016, 5073, 10));
    world.manor.set_seed_production(
        1,
        false,
        vec![SeedProduction {
            seed_id: 5016,
            amount: 3,
            price: 10,
            start_amount: 500,
        }],
    );
    super::items::add_inventory_item(&mut world, 100, ADENA_ID, 1_000);

    let mut w = PacketWriter::new();
    w.write_i32(1);
    w.write_i32(1);
    w.write_i32(5016);
    w.write_i64(10); // only 3 in stock
    crate::game_loop::manor::handle_request_buy_seed(&mut world, 1, &w.into_bytes());

    assert_eq!(inv_count(&world, 5016), 0, "no seeds delivered on overdraw");
    assert_eq!(inv_count(&world, ADENA_ID), 1_000, "no adena taken");
    assert_eq!(
        world.manor.seed_product(1, 5016, false).unwrap().amount,
        3,
        "stock unchanged"
    );
}

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

/// **Requests 3/4 send the castle's live seed-production / crop-procure state.**
/// The "Seed Purchase" view (request 3) → `ExShowSeedInfo`; "Crop Sales"
/// (request 4) → `ExShowCropInfo`, each carrying the runtime `ManorState` list.
#[test]
fn manor_menu_select_requests_3_and_4_send_live_state() {
    let (mut world, mut rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    // Catalogue (for level/reward resolution) + live state for Gludio.
    world.data.manor.insert_for_test(seed(1, 5016, 5073, 10));
    world.manor.set_seed_production(
        1,
        false,
        vec![SeedProduction {
            seed_id: 5016,
            amount: 700,
            price: 3,
            start_amount: 1000,
        }],
    );
    world.manor.set_crop_procure(
        1,
        false,
        vec![CropProcure {
            crop_id: 5073,
            amount: 40,
            price: 9,
            start_amount: 50,
            reward_type: 1,
        }],
    );
    world.objects.add_components(&100, LastFolkNpc(701));

    // Request 3 → ExShowSeedInfo (0x23), one seed line.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("manor_menu_select?ask=3&state=-1&time=0"),
    );
    let pkt = ex_packet(&mut rx, 0x23).expect("ExShowSeedInfo sent");
    // [0xFE][0x23 0x00][hide][manorId i32][unknown i32][count i32]…
    assert_eq!(i32::from_le_bytes(pkt[12..16].try_into().unwrap()), 1);
    assert_eq!(
        i32::from_le_bytes(pkt[16..20].try_into().unwrap()),
        5016,
        "the seed id"
    );

    // Request 4 → ExShowCropInfo (0x24), one crop line.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("manor_menu_select?ask=4&state=-1&time=0"),
    );
    let pkt = ex_packet(&mut rx, 0x24).expect("ExShowCropInfo sent");
    assert_eq!(i32::from_le_bytes(pkt[12..16].try_into().unwrap()), 1);
    assert_eq!(
        i32::from_le_bytes(pkt[16..20].try_into().unwrap()),
        5073,
        "the crop id"
    );
}

/// **The manor state loads at boot, grouped by castle/period and filtered to
/// known ids** (Java `CastleManorManager.loadDb`'s "don't load unknown"). An
/// unknown seed row and unknown crop row are dropped.
#[test]
fn manor_state_loads_at_boot() {
    let (mut world, _db, _l) = quest_test_world();
    world.data.manor.insert_for_test(seed(1, 5016, 5073, 10));

    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(crate::db::DbEvent::ManorLoaded {
        production: vec![
            // Known seed, current period.
            crate::db::ManorProductionRow {
                castle_id: 1,
                seed_id: 5016,
                amount: 500,
                start_amount: 500,
                price: 3,
                next_period: false,
            },
            // Unknown seed → dropped.
            crate::db::ManorProductionRow {
                castle_id: 1,
                seed_id: 999_999,
                amount: 1,
                start_amount: 1,
                price: 1,
                next_period: false,
            },
        ],
        procure: vec![
            // Known crop, next period.
            crate::db::ManorProcureRow {
                castle_id: 1,
                crop_id: 5073,
                amount: 20,
                start_amount: 20,
                price: 9,
                reward_type: 1,
                next_period: true,
            },
            // Unknown crop → dropped.
            crate::db::ManorProcureRow {
                castle_id: 1,
                crop_id: 999_998,
                amount: 1,
                start_amount: 1,
                price: 1,
                reward_type: 0,
                next_period: true,
            },
        ],
    })
    .unwrap();
    drop(tx);
    crate::game_loop::net::drain_db(&mut world, &rx);

    // The known seed is in the current period; the unknown one was dropped.
    let prod = world.manor.seed_production(1, false);
    assert_eq!(prod.len(), 1, "one known seed loaded, unknown dropped");
    assert_eq!(prod[0].seed_id, 5016);
    assert_eq!(prod[0].amount, 500);
    // The crop was a next-period row → current period is empty.
    assert!(world.manor.crop_procure(1, false).is_empty());
    let proc = world.manor.crop_procure(1, true);
    assert_eq!(proc.len(), 1, "one known crop loaded, unknown dropped");
    assert_eq!(proc[0].crop_id, 5073);
    assert_eq!(proc[0].reward_type, 1);
}

/// **The daily mode cycle advances and rolls an owned castle's production.**
/// APPROVED → MAINTENANCE promotes the next-period setup to current; the cycle
/// then continues MAINTENANCE → MODIFIABLE → APPROVED.
#[test]
fn mode_cycle_rolls_owned_castle_production() {
    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    world
        .data
        .manor
        .insert_for_test(seed_full(1, 90001, 91001, 10, 8100, 8100));
    own_castle(&mut world, 1); // clan 500 owns Gludio
                               // Owner set up next-period seed production; current is empty.
    world.manor.set_next_seed_production(
        1,
        vec![SeedProduction {
            seed_id: 90001,
            amount: 500,
            price: 3,
            start_amount: 500,
        }],
    );
    // Mode starts Approved (the settled daytime period).
    world.manor.set_mode(ManorMode::Approved);
    assert!(
        world.manor.seed_production(1, false).is_empty(),
        "current empty before roll"
    );

    // APPROVED → MAINTENANCE: the next-period setup rolls into current.
    crate::game_loop::manor::advance_manor_mode(&mut world);
    assert_eq!(world.manor.mode(), ManorMode::Maintenance);
    let cur = world.manor.seed_production(1, false);
    assert_eq!(cur.len(), 1, "the owner's setup is now the current period");
    assert_eq!(cur[0].seed_id, 90001);

    // The cycle continues.
    crate::game_loop::manor::advance_manor_mode(&mut world);
    assert_eq!(world.manor.mode(), ManorMode::Modifiable);
    crate::game_loop::manor::advance_manor_mode(&mut world);
    assert_eq!(world.manor.mode(), ManorMode::Approved);
}

/// **An unowned castle's manor does not roll.** Java skips castles with no owner
/// in `changeMode`, so a next-period setup on an ownerless castle stays put.
#[test]
fn mode_cycle_skips_unowned_castle() {
    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    world
        .data
        .manor
        .insert_for_test(seed_full(1, 90001, 91001, 10, 8100, 8100));
    // No clan owns castle 1.
    world.manor.set_next_seed_production(
        1,
        vec![SeedProduction {
            seed_id: 90001,
            amount: 500,
            price: 3,
            start_amount: 500,
        }],
    );
    world.manor.set_mode(ManorMode::Approved);

    crate::game_loop::manor::advance_manor_mode(&mut world);
    assert!(
        world.manor.seed_production(1, false).is_empty(),
        "an unowned castle's manor is not rolled"
    );
}

fn seed(castle_id: i32, seed_id: i32, crop_id: i32, level: i32) -> Seed {
    seed_full(castle_id, seed_id, crop_id, level, 0, 0)
}

fn seed_full(
    castle_id: i32,
    seed_id: i32,
    crop_id: i32,
    level: i32,
    limit_seeds: i32,
    limit_crops: i32,
) -> Seed {
    Seed {
        castle_id,
        seed_id,
        crop_id,
        mature_id: 0,
        level,
        reward1: 1864,
        reward2: 1878,
        alternative: false,
        limit_seeds,
        limit_crops,
    }
}

/// **The owner setup views (requests 7/8) are gated to the modifiable period.**
/// During the settled (`Approved`) period nothing is shown; once the manor is
/// `Modifiable` the seed/crop setup windows are sent.
#[test]
fn manor_setup_views_gated_by_period() {
    let (mut world, mut rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    // Seed id 90001 is not a real item ⇒ reference price defaults to 1.
    world
        .data
        .manor
        .insert_for_test(seed_full(1, 90001, 91001, 10, 8100, 8100));
    world.objects.add_components(&100, LastFolkNpc(701));

    // Approved (the default) → no setup window for request 7 or 8.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("manor_menu_select?ask=7&state=-1&time=0"),
    );
    assert!(
        ex_packet(&mut rx, 0x26).is_none(),
        "no seed setting during approved period"
    );

    // Modifiable → the seed setup (0x26) and crop setup (0x2B) are sent.
    world.manor.set_mode(ManorMode::Modifiable);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("manor_menu_select?ask=7&state=-1&time=0"),
    );
    let pkt = ex_packet(&mut rx, 0x26).expect("ExShowSeedSetting sent when modifiable");
    // [0xFE][0x26 0x00][manorId i32][size i32]…
    assert_eq!(
        i32::from_le_bytes(pkt[7..11].try_into().unwrap()),
        1,
        "one seed line"
    );

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("manor_menu_select?ask=8&state=-1&time=0"),
    );
    assert!(
        ex_packet(&mut rx, 0x2B).is_some(),
        "ExShowCropSetting sent when modifiable"
    );
}

/// **RequestSetSeed writes the owner's next-period seed setup, filtering bad
/// lines.** A valid seed within its limit/price band is stored; an unknown
/// seed and an over-limit sale are dropped.
#[test]
fn request_set_seed_writes_next_period() {
    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    world.manor.set_mode(ManorMode::Modifiable);
    world
        .data
        .manor
        .insert_for_test(seed_full(1, 90001, 91001, 10, 8100, 8100));
    own_castle(&mut world, 1);
    world.objects.add_components(&100, LastFolkNpc(701));

    // Three lines: a valid seed, an over-limit valid seed, and an unknown seed.
    let mut w = PacketWriter::new();
    w.write_i32(1); // manor id
    w.write_i32(3); // count
    w.write_i32(90001);
    w.write_i64(500); // sales within the 8100 limit
    w.write_i64(3); // price within [0, 10]
    w.write_i32(90001);
    w.write_i64(999_999); // over the limit → dropped
    w.write_i64(3);
    w.write_i32(88888); // unknown seed → dropped
    w.write_i64(10);
    w.write_i64(3);
    crate::game_loop::manor::handle_request_set_seed(&mut world, 1, &w.into_bytes());

    let next = world.manor.seed_production(1, true);
    assert_eq!(next.len(), 1, "only the valid in-limit seed is stored");
    assert_eq!(next[0].seed_id, 90001);
    assert_eq!(next[0].start_amount, 500);
    assert_eq!(next[0].amount, 500);
    assert_eq!(next[0].price, 3);
}

/// **RequestSetSeed is refused outside the modifiable period.** In the settled
/// period the write is dropped (Java's `!isModifiablePeriod` → ActionFailed).
#[test]
fn request_set_seed_refused_when_not_modifiable() {
    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    // Mode stays Approved (the default).
    world
        .data
        .manor
        .insert_for_test(seed_full(1, 90001, 91001, 10, 8100, 8100));
    own_castle(&mut world, 1);
    world.objects.add_components(&100, LastFolkNpc(701));

    let mut w = PacketWriter::new();
    w.write_i32(1);
    w.write_i32(1);
    w.write_i32(90001);
    w.write_i64(500);
    w.write_i64(3);
    crate::game_loop::manor::handle_request_set_seed(&mut world, 1, &w.into_bytes());

    assert!(
        world.manor.seed_production(1, true).is_empty(),
        "no write outside the modifiable period"
    );
}

/// Find the `ExShowManorDefaultInfo` packet (EX 0xFE, sub-op 0x25) among the
/// drained output.
fn default_info_packet(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) -> Option<Vec<u8>> {
    ex_packet(rx, 0x25)
}

/// Find an EX packet (0xFE) with the given single-byte sub-op among the drained
/// output.
fn ex_packet(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>, subop: u8) -> Option<Vec<u8>> {
    drain(rx)
        .into_iter()
        .find(|p| p.len() >= 8 && p[0] == 0xFE && p[1] == subop && p[2] == 0x00)
}
