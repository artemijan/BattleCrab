//! Castle manor (G26) — the chamberlain's manor menu entry and the
//! `manor_menu_select` display bypass. Seven Signs is removed from this dist;
//! the manor is config-disabled (`AllowManor=False`) but fully wired so an
//! operator can enable it.

use super::*;
use crate::game_loop::character::inventory;

use crate::data::item_data::ADENA_ID;
use crate::data::manor_data::Seed;
use crate::model::Player;
use crate::model::components::LastFolkNpc;
use crate::model::manor::{CropProcure, ManorMode, SeedProduction};

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

/// Put Gludio (castle 1) on the world so the manor sale has a vault to pay.
fn add_gludio(world: &mut World) {
    world.castles = vec![model::castle::Castle {
        show_npc_crest: false,
        id: 1,
        name: "Gludio".into(),
        side: model::castle::CastleSide::Neutral,
        ticket_buy_count: 0,
        first_mid_victory: false,
        time_registration_over: true,
        siege_time_registration_end: 0,
        siege_date: 0,
        treasury: 0,
    }];
}

/// Register + place a sowable monster (a `canBeSown` Monster) at the origin.
/// Uses `NPC_OID` so `is_npc_oid` (which range-checks the object id) accepts it.
fn add_sowable_mob(world: &mut World, npc_id: i32, level: i32) {
    let mut t = crate::data::npc_data::default_template(npc_id);
    t.type_name = "Monster".into();
    t.level = level;
    t.base_hp_max = 100.0;
    t.base_mp_max = 30.0;
    t.can_be_sown = true;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(world, NPC_OID, npc_id, "Monster", level, 0, 0, 0);
}

/// **Sow a seed on a monster, then harvest the crop from its corpse.** A
/// successful `Sow` marks the mob seeded and stashes the crop; a successful
/// `Harvesting` on the dead mob hands it over (and clears it).
#[test]
fn sow_then_harvest_yields_the_crop() {
    use crate::game_loop::skills::effects::{apply_harvesting, apply_sow};
    use crate::model::components::Vitals;
    use crate::model::npc::Npc;

    let (mut world, mut rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    world.data.manor.insert_for_test(seed(1, 5016, 5073, 10)); // seed 5016 → crop 5073, lvl 10
    add_stackable_item(&mut world, 5073, 50);
    add_sowable_mob(&mut world, 45001, 10);

    // The Seed item handler flags the mob (seed + seeder) before the Sow skill.
    {
        let npc = world.objects.get_component_mut::<Npc>(&NPC_OID).unwrap();
        npc.seed_id = 5016;
        npc.seeder_object_id = 100;
    }
    // Sow: a forced roll of 0 is under any positive chance → success.
    world.force_roll(0);
    drain(&mut rx);
    apply_sow(&mut world, 100, NPC_OID);
    {
        // Java's success leg: the item-get sound, then the result message —
        // sent solo here because this sower has no party.
        let pkts = drain(&mut rx);
        assert!(
            ids_after_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE)
                .contains(&server_packets::sm_ids::THE_SEED_WAS_SUCCESSFULLY_SOWN),
            "the sower is told the seed took"
        );
        assert!(
            pkts.iter()
                .any(|p| p[0] == server_packets::opcodes::PLAY_SOUND),
            "ITEMSOUND_QUEST_ITEMGET accompanies a successful sow"
        );
    }
    {
        let npc = world.objects.get_component::<Npc>(&NPC_OID).unwrap();
        assert!(npc.seeded, "the mob is now seeded");
        assert_eq!(
            npc.harvest_item,
            Some((5073, 1)),
            "one crop stashed (RateDropManor = 1)"
        );
    }

    // The mob dies; the seeder harvests its corpse.
    world
        .objects
        .get_component_mut::<Vitals>(&NPC_OID)
        .unwrap()
        .dead = true;
    world.force_roll(0);
    apply_harvesting(&mut world, 100, NPC_OID);
    assert_eq!(
        inv_count(&world, 5073),
        1,
        "the harvester received the crop"
    );
    assert!(
        world
            .objects
            .get_component::<Npc>(&NPC_OID)
            .unwrap()
            .harvest_item
            .is_none(),
        "the crop can't be harvested twice"
    );
}

/// **Only the seeder may harvest.** A different player's `Harvesting` cast on a
/// sown corpse yields nothing and leaves the crop stashed.
#[test]
fn harvest_refused_when_not_the_seeder() {
    use crate::game_loop::skills::effects::apply_harvesting;
    use crate::model::components::Vitals;
    use crate::model::npc::Npc;

    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    world.data.manor.insert_for_test(seed(1, 5016, 5073, 10));
    add_stackable_item(&mut world, 5073, 50);
    add_sowable_mob(&mut world, 45001, 10);
    // Already sown+seeded by player 100, and dead.
    {
        let npc = world.objects.get_component_mut::<Npc>(&NPC_OID).unwrap();
        npc.seed_id = 5016;
        npc.seeder_object_id = 100;
        npc.seeded = true;
        npc.harvest_item = Some((5073, 1));
    }
    world
        .objects
        .get_component_mut::<Vitals>(&NPC_OID)
        .unwrap()
        .dead = true;

    // A different, real player (999, not the seeder) tries to harvest.
    let mut rx2 = ingame_player(&mut world, 2, 999, 0, 0, 0);
    world.force_roll(0);
    drain(&mut rx2);
    apply_harvesting(&mut world, 999, NPC_OID);
    assert_eq!(inv_count(&world, 5073), 0, "a non-seeder harvests nothing");
    assert!(
        ids_after_opcode(&drain(&mut rx2), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_HARVEST),
        "Java tells the interloper why nothing happened"
    );
    assert!(
        world
            .objects
            .get_component::<Npc>(&NPC_OID)
            .unwrap()
            .harvest_item
            .is_some(),
        "the crop is left for the seeder"
    );
}

/// The test world's `item_data` is `empty()`; register a stackable Etc item
/// (cloned from the always-present Adena template) so crops/rewards stack and
/// carry a reference price.
fn add_stackable_item(world: &mut World, item_id: i32, price: i64) {
    let mut t = world
        .data
        .item_data
        .get(ADENA_ID)
        .cloned()
        .expect("adena template present");
    t.item_id = item_id;
    t.name = format!("TestItem{item_id}");
    t.price = price;
    t.is_stackable = true;
    world.data.item_data.insert_for_test(t);
}

/// A player buys seeds at a Manor Manager: adena leaves, the seeds arrive, and
/// the manor's current-period stock drops by the amount bought.
#[test]
fn buy_seed_trades_adena_for_seeds_and_decrements_stock() {
    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    // The sale needs a castle with an owner for the vault to exist.
    add_gludio(&mut world);
    own_castle(&mut world, 1);
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
    inventory::add_inventory_item(&mut world, 100, ADENA_ID, 1_000);

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
    assert_eq!(
        crate::game_loop::siege::treasury::treasury(&world, 1),
        50,
        "and the sale went into the castle's vault (addToTreasuryNoTax)"
    );
}

/// **Seed money paid at a castle nobody owns leaves the economy.** Java's
/// `addToTreasuryNoTax` returns early on `_ownerId <= 0`, so the buyer is still
/// charged and the stock still drops — the adena just goes nowhere.
#[test]
fn buy_seed_at_an_unowned_castle_banks_nothing() {
    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    add_gludio(&mut world); // …but no owning clan
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
    inventory::add_inventory_item(&mut world, 100, ADENA_ID, 1_000);

    let mut w = PacketWriter::new();
    w.write_i32(1);
    w.write_i32(1);
    w.write_i32(5016);
    w.write_i64(5);
    crate::game_loop::manor::handle_request_buy_seed(&mut world, 1, &w.into_bytes());

    assert_eq!(inv_count(&world, ADENA_ID), 950, "the buyer still paid");
    assert_eq!(crate::game_loop::siege::treasury::treasury(&world, 1), 0);
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
    inventory::add_inventory_item(&mut world, 100, ADENA_ID, 10); // far short of 50

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
    inventory::add_inventory_item(&mut world, 100, ADENA_ID, 1_000);

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

/// A player sells crops at the crop's own Manor Manager: the reward item
/// arrives, the crops leave the inventory, the procurement stock drops, and
/// **no** cross-manor fee is charged.
#[test]
fn sell_crop_same_manor_pays_reward_without_fee() {
    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    add_manor_manager(&mut world, 702, 35103, 1); // manager's castle = manor 1
    // Catalogue: crop 5073 yields reward item 1864 (reward type 1).
    world.data.manor.insert_for_test(seed(1, 5016, 5073, 10));
    add_stackable_item(&mut world, 5073, 50); // the crop
    add_stackable_item(&mut world, 1864, 20); // the reward
    world.manor.set_crop_procure(
        1,
        false,
        vec![CropProcure {
            crop_id: 5073,
            amount: 100,
            price: 1_000_000,
            start_amount: 100,
            reward_type: 1,
        }],
    );
    let crop_oid = inventory::add_inventory_item(&mut world, 100, 5073, 20).unwrap()[0];
    inventory::add_inventory_item(&mut world, 100, ADENA_ID, 10_000);

    // Sell 10 crops registered at manor 1.
    let mut w = PacketWriter::new();
    w.write_i32(1); // count
    w.write_i32(crop_oid);
    w.write_i32(5073);
    w.write_i32(1); // item's manor
    w.write_i64(10);
    crate::game_loop::manor::handle_request_procure_crop_list(&mut world, 1, &w.into_bytes());

    assert_eq!(inv_count(&world, 5073), 10, "10 crops were sold");
    assert!(inv_count(&world, 1864) > 0, "the reward item was paid out");
    assert_eq!(
        inv_count(&world, ADENA_ID),
        10_000,
        "no fee at the crop's own manor"
    );
    assert_eq!(
        world.manor.crop_procure_for(1, 5073, false).unwrap().amount,
        90,
        "procurement stock dropped by 10"
    );
}

/// Selling a crop at a *different* castle's Manor Manager charges the Java 5 %
/// adena fee.
#[test]
fn sell_crop_cross_manor_charges_five_percent_fee() {
    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    add_manor_manager(&mut world, 702, 35103, 1); // manager's castle = manor 1
    world.data.manor.insert_for_test(seed(1, 5016, 5073, 10));
    add_stackable_item(&mut world, 5073, 50);
    add_stackable_item(&mut world, 1864, 20);
    // The crop's procurement is registered at manor 2, not the manager's 1.
    world.manor.set_crop_procure(
        2,
        false,
        vec![CropProcure {
            crop_id: 5073,
            amount: 100,
            price: 1_000_000,
            start_amount: 100,
            reward_type: 1,
        }],
    );
    let crop_oid = inventory::add_inventory_item(&mut world, 100, 5073, 20).unwrap()[0];
    inventory::add_inventory_item(&mut world, 100, ADENA_ID, 1_000_000);

    // Sell 10 crops registered at manor 2 → price 10,000,000 → fee 500,000.
    let mut w = PacketWriter::new();
    w.write_i32(1);
    w.write_i32(crop_oid);
    w.write_i32(5073);
    w.write_i32(2); // item's manor (≠ the manager's)
    w.write_i64(10);
    crate::game_loop::manor::handle_request_procure_crop_list(&mut world, 1, &w.into_bytes());

    assert_eq!(inv_count(&world, 5073), 10, "10 crops were sold");
    assert!(inv_count(&world, 1864) > 0, "the reward item was paid out");
    assert_eq!(
        inv_count(&world, ADENA_ID),
        500_000,
        "the 5% cross-manor fee (500,000) was taken"
    );
    assert_eq!(
        world.manor.crop_procure_for(2, 5073, false).unwrap().amount,
        90,
        "manor 2's procurement dropped by 10"
    );
}

/// A sell for crops the player doesn't hold is rejected outright (no stock
/// change), matching Java's item-validation `ActionFailed` + return.
#[test]
fn sell_crop_rejected_when_item_missing() {
    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    add_manor_manager(&mut world, 702, 35103, 1);
    world.data.manor.insert_for_test(seed(1, 5016, 5073, 10));
    world.manor.set_crop_procure(
        1,
        false,
        vec![CropProcure {
            crop_id: 5073,
            amount: 100,
            price: 1_000_000,
            start_amount: 100,
            reward_type: 1,
        }],
    );
    // The player holds no crops; object id 999999 is bogus.
    let mut w = PacketWriter::new();
    w.write_i32(1);
    w.write_i32(999_999);
    w.write_i32(5073);
    w.write_i32(1);
    w.write_i64(10);
    crate::game_loop::manor::handle_request_procure_crop_list(&mut world, 1, &w.into_bytes());

    assert_eq!(inv_count(&world, 1864), 0, "no reward paid on a bogus sell");
    assert_eq!(
        world.manor.crop_procure_for(1, 5073, false).unwrap().amount,
        100,
        "procurement stock unchanged"
    );
}

/// Gludio's Chamberlain of Light (35100) at the origin, plus an in-game player
/// standing on it. Returns the world and the player's packet receiver.
fn chamberlain_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, _db, _link) = quest_test_world();
    add_test_npc(&mut world, 701, 35100, "Merchant", 75, 0, 0, 0);
    let rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    (world, rx)
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

    handle_db_event(
        &mut world,
        DbEvent::ManorLoaded {
            production: vec![
                // Known seed, current period.
                db::ManorProductionRow {
                    castle_id: 1,
                    seed_id: 5016,
                    amount: 500,
                    start_amount: 500,
                    price: 3,
                    next_period: false,
                },
                // Unknown seed → dropped.
                db::ManorProductionRow {
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
                db::ManorProcureRow {
                    castle_id: 1,
                    crop_id: 5073,
                    amount: 20,
                    start_amount: 20,
                    price: 9,
                    reward_type: 1,
                    next_period: true,
                },
                // Unknown crop → dropped.
                db::ManorProcureRow {
                    castle_id: 1,
                    crop_id: 999_998,
                    amount: 1,
                    start_amount: 1,
                    price: 1,
                    reward_type: 0,
                    next_period: true,
                },
            ],
        },
    );

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
fn default_info_packet(rx: &mut UnboundedReceiver<bytes::Bytes>) -> Option<Vec<u8>> {
    ex_packet(rx, 0x25)
}

/// Find an EX packet (0xFE) with the given single-byte sub-op among the drained
/// output.
fn ex_packet(rx: &mut UnboundedReceiver<bytes::Bytes>, subop: u8) -> Option<Vec<u8>> {
    drain(rx)
        .into_iter()
        .find(|p| p.len() >= 8 && p[0] == 0xFE && p[1] == subop && p[2] == 0x00)
}

/// **A seed may only be sown inside its own castle's territory.** Java's Seed
/// item handler refuses when the target's `TaxZone` doesn't name the seed's
/// castle (`THIS_SEED_MAY_NOT_BE_SOWN_HERE`); the mob is left unflagged, so the
/// Sow skill never runs. Inside the right territory the same use flags it.
#[test]
fn sowing_is_gated_on_the_seeds_own_territory() {
    use crate::model::npc::Npc;

    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    world.data.manor.insert_for_test(seed(1, 5016, 5073, 10)); // a Gludio seed
    add_sowable_mob(&mut world, 45001, 10);
    // The seed item itself, carrying Java's `Seed` item handler.
    add_stackable_item(&mut world, 5016, 10);
    {
        let mut t = world.data.item_data.get(5016).cloned().unwrap();
        t.handler = crate::data::item_data::kinds::ItemHandler::Seed;
        world.data.item_data.insert_for_test(t);
    }
    // Give the player the seed item and target the mob.
    let seed_oid =
        inventory::add_inventory_item(&mut world, 100, 5016, 1).expect("the seed was added")[0];
    world.objects.add_components(&100, TargetRef(Some(NPC_OID)));

    // The mob stands in *Dion's* tax territory — wrong castle, refused.
    insert_tax_zone_for(&mut world, 2);
    items::handle_use_item(&mut world, 1, &use_item_body(seed_oid));
    assert_eq!(
        world
            .objects
            .get_component::<Npc>(&NPC_OID)
            .map(|n| n.seed_id),
        Some(0),
        "the mob was never flagged with the seed"
    );

    // Re-home the zone to Gludio: the same use now flags the mob.
    world.data.zone_data = crate::data::zone_data::ZoneData::empty();
    insert_tax_zone_for(&mut world, 1);
    items::handle_use_item(&mut world, 1, &use_item_body(seed_oid));
    assert_eq!(
        world
            .objects
            .get_component::<Npc>(&NPC_OID)
            .map(|n| n.seed_id),
        Some(5016),
        "sown inside its own castle's territory"
    );
}

/// A `TaxZone` around the origin paying `castle_id`.
fn insert_tax_zone_for(world: &mut World, castle_id: i32) {
    world.data.zone_data.insert(crate::data::zone_data::Zone {
        id: 0,
        name: format!("test_tax_{castle_id}"),
        kind: crate::data::zone_data::ZoneKind::Tax,
        territory: test_territory(),
        castle_id,
        clan_hall_id: 0,
        effect: None,
        damage: None,
        swamp: None,
        condition: None,
        mother_tree: None,
    });
}

// --- The rollover settlement (Java `CastleManorManager.changeMode`) ----------

const MATURE_ID: i32 = 91101;

/// A seed line whose crop matures into [`MATURE_ID`].
fn seed_with_mature(castle_id: i32, seed_id: i32, crop_id: i32) -> Seed {
    Seed {
        mature_id: MATURE_ID,
        ..seed_full(castle_id, seed_id, crop_id, 10, 8100, 8100)
    }
}

/// A world holding Gludio (owned by player 100's clan), the mature-crop item
/// template, and a closing period whose crops were partly sold.
fn settlement_world(sold: i64, left: i64, price: i64) -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    add_gludio(&mut world);
    own_castle(&mut world, 1);
    world
        .data
        .manor
        .insert_for_test(seed_with_mature(1, 90001, 91001));
    add_stackable_item(&mut world, MATURE_ID, 1);
    world.manor.set_crop_procure(
        1,
        false,
        vec![CropProcure {
            crop_id: 91001,
            amount: left,
            price,
            start_amount: sold + left,
            reward_type: 1,
        }],
    );
    world.manor.set_mode(ManorMode::Approved);
    (world, rx)
}

fn clan_wh_count(world: &World, clan_id: i32, item_id: i32) -> i64 {
    world
        .clans
        .get(&clan_id)
        .map_or(0, |c| c.warehouse.0.count_of(item_id))
}

/// **The closing period pays the owner and refunds the vault.** Crops players
/// sold are paid into the clan warehouse as *mature* crops at 90 %, and the
/// adena still reserved for crops nobody sold goes back to the treasury.
#[test]
fn rollover_pays_crops_to_the_warehouse_and_refunds_the_treasury() {
    // 60 of 100 crops sold at 7 adena, 40 still reserved.
    let (mut world, _rx) = settlement_world(60, 40, 7);

    crate::game_loop::manor::advance_manor_mode(&mut world);

    assert_eq!(
        clan_wh_count(&world, 500, MATURE_ID),
        54,
        "60 sold × 0.9 matured crops reached the clan warehouse"
    );
    assert_eq!(
        crate::game_loop::siege::treasury::treasury(&world, 1),
        40 * 7,
        "the unspent reservation went back to the vault"
    );
}

/// **A payout that rounds to nothing is Java's 90 % consolation item.** One crop
/// sold gives `(1 × 0.9) = 0`, which becomes 1 when `Rnd.get(99) < 90` — and
/// stays 0 on the other roll.
#[test]
fn a_rounded_down_payout_is_one_item_ninety_percent_of_the_time() {
    let (mut world, _rx) = settlement_world(1, 0, 7);
    world.force_roll(0); // < 90 → consolation item
    crate::game_loop::manor::advance_manor_mode(&mut world);
    assert_eq!(clan_wh_count(&world, 500, MATURE_ID), 1, "consolation item");

    let (mut world, _rx) = settlement_world(1, 0, 7);
    world.force_roll(95); // ≥ 90 → nothing at all
    crate::game_loop::manor::advance_manor_mode(&mut world);
    assert_eq!(clan_wh_count(&world, 500, MATURE_ID), 0, "no payout");
}

/// **A line that was never set up is skipped whole** (Java's
/// `startAmount > 0` guard) — no payout, no refund.
#[test]
fn an_unset_crop_line_settles_nothing() {
    let (mut world, _rx) = settlement_world(0, 0, 7);
    crate::game_loop::manor::advance_manor_mode(&mut world);
    assert_eq!(clan_wh_count(&world, 500, MATURE_ID), 0);
    assert_eq!(crate::game_loop::siege::treasury::treasury(&world, 1), 0);
}

/// **The next period is wiped when the treasury can't cover the one just
/// promoted.** With a full vault the setup survives the rollover.
#[test]
fn the_next_period_is_gated_on_the_treasury() {
    // A next-period crop setup worth 100 × 7 = 700 adena.
    let setup = |world: &mut World| {
        world.manor.set_crop_procure(
            1,
            true,
            vec![CropProcure {
                crop_id: 91001,
                amount: 100,
                price: 7,
                start_amount: 100,
                reward_type: 1,
            }],
        );
    };

    // Empty vault (the closing period refunds nothing): next is cleared.
    let (mut world, _rx) = settlement_world(0, 0, 7);
    setup(&mut world);
    crate::game_loop::manor::advance_manor_mode(&mut world);
    assert!(
        world.manor.crop_procure(1, true).is_empty(),
        "a castle that can't afford the promoted period loses its next setup"
    );

    // Same setup, but the vault covers the promoted period's 700 adena.
    let (mut world, _rx) = settlement_world(0, 0, 7);
    setup(&mut world);
    crate::game_loop::siege::treasury::add_to_treasury_no_tax(&mut world, 1, 700);
    crate::game_loop::manor::advance_manor_mode(&mut world);
    assert_eq!(
        world.manor.crop_procure(1, true).len(),
        1,
        "an affordable period keeps its next setup"
    );
}

/// **The rollover is written through.** Java `storeMe()`s after the APPROVED
/// transition; the port sends one `StoreManor` per rolled castle carrying both
/// periods.
#[test]
fn the_rollover_persists_the_manor() {
    let (mut world, _db, _link) = quest_test_world();
    world.cfg.general.allow_manor = true;
    add_test_npc(&mut world, 701, 35100, "Merchant", 75, 0, 0, 0);
    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    add_gludio(&mut world);
    own_castle(&mut world, 1);
    world
        .data
        .manor
        .insert_for_test(seed_with_mature(1, 90001, 91001));
    world.manor.set_next_seed_production(
        1,
        vec![SeedProduction {
            seed_id: 90001,
            amount: 500,
            price: 3,
            start_amount: 500,
        }],
    );
    crate::game_loop::siege::treasury::add_to_treasury_no_tax(&mut world, 1, 10_000_000);
    world.manor.set_mode(ManorMode::Approved);
    let mut db = _db;
    drain_db(&mut db);

    crate::game_loop::manor::advance_manor_mode(&mut world);

    let stored = drain_db(&mut db).into_iter().find_map(|c| match c {
        db::DbCommand::StoreManor {
            castle_id: 1,
            production,
            ..
        } => Some(production),
        _ => None,
    });
    let production = stored.expect("the rolled castle was stored");
    assert_eq!(production.len(), 2, "both periods are written");
    assert!(
        production.iter().any(|r| !r.next_period) && production.iter().any(|r| r.next_period),
        "one current row and one next row"
    );
}

/// **Maintenance tells the owner's online leader the manor was updated**
/// (SM 884).
#[test]
fn maintenance_notifies_the_online_clan_leader() {
    let (mut world, mut rx) = settlement_world(0, 0, 7);
    world.manor.set_mode(ManorMode::Maintenance);
    drain(&mut rx);

    crate::game_loop::manor::advance_manor_mode(&mut world);

    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_MANOR_INFORMATION_HAS_BEEN_UPDATED),
        "the leader is told the manor information has been updated"
    );
}

/// **Approving the new period charges its cost to the treasury.**
#[test]
fn approving_charges_the_manor_cost() {
    let (mut world, _rx) = settlement_world(0, 0, 7);
    world.manor.set_crop_procure(
        1,
        true,
        vec![CropProcure {
            crop_id: 91001,
            amount: 100,
            price: 7,
            start_amount: 100,
            reward_type: 1,
        }],
    );
    crate::game_loop::siege::treasury::add_to_treasury_no_tax(&mut world, 1, 1_000);
    world.manor.set_mode(ManorMode::Modifiable);

    crate::game_loop::manor::advance_manor_mode(&mut world);

    assert_eq!(world.manor.mode(), ManorMode::Approved);
    assert_eq!(
        crate::game_loop::siege::treasury::treasury(&world, 1),
        300,
        "1000 − the period's 700 adena cost"
    );
    assert_eq!(
        world.manor.crop_procure(1, true).len(),
        1,
        "the setup stands"
    );
}

/// **A castle that can neither pay nor store loses the period and is warned.**
/// Java's gate is `!validateCapacity(slots) && treasury < cost` — *both* must
/// fail, so the warehouse is filled to its slot ceiling here.
#[test]
fn a_castle_that_cannot_pay_or_store_loses_its_setup() {
    let (mut world, mut rx) = settlement_world(0, 0, 7);
    world.manor.set_crop_procure(
        1,
        true,
        vec![CropProcure {
            crop_id: 91001,
            amount: 100,
            price: 7,
            start_amount: 100,
            reward_type: 1,
        }],
    );
    // Vault can't cover the 700, and the warehouse is at its ceiling.
    world.cfg.character.warehouse_slots_clan = 0;
    world.manor.set_mode(ManorMode::Modifiable);
    drain(&mut rx);

    crate::game_loop::manor::advance_manor_mode(&mut world);

    assert!(
        world.manor.crop_procure(1, true).is_empty(),
        "the unaffordable setup is cleared"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::NOT_ENOUGH_FUNDS_IN_CLAN_WAREHOUSE_FOR_MANOR),
        "and the leader is warned"
    );
}

/// **`//manor` shows the period state and each castle's bill.** Java's
/// `AdminManor` is a read-only page; the parts worth pinning are that the two
/// costs come from the *current* and *next* period lists (they differ), the
/// castles are listed in id order, and the mode name is the bare enum constant.
#[test]
fn the_manor_admin_page_reports_the_period_and_the_costs() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    // The page is a real dist html, so the datapack root has to be set.
    world.data.root = crate::data::DIST_GAME.to_string();
    world.cfg.general.allow_manor = true;
    // Two castles, deliberately inserted out of id order.
    let castle = |id: i32, name: &str| model::castle::Castle {
        show_npc_crest: false,
        id,
        name: name.into(),
        side: model::castle::CastleSide::Neutral,
        ticket_buy_count: 0,
        first_mid_victory: false,
        time_registration_over: true,
        siege_time_registration_end: 0,
        siege_date: 0,
        treasury: 0,
    };
    world.castles = vec![castle(5, "Aden"), castle(1, "Gludio")];
    // Seed id 90001 is not a real item ⇒ its reference price defaults to 1, so
    // the cost is just the start amount — 30 now, 70 next period. The two must
    // not be read off the same list.
    world.data.manor.insert_for_test(seed(1, 90001, 91001, 10));
    world.manor.set_seed_production(
        1,
        false,
        vec![SeedProduction {
            seed_id: 90001,
            amount: 30,
            start_amount: 30,
            price: 1,
        }],
    );
    world.manor.set_seed_production(
        1,
        true,
        vec![SeedProduction {
            seed_id: 90001,
            amount: 70,
            start_amount: 70,
            price: 1,
        }],
    );
    world.manor.set_mode(ManorMode::Approved);
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("manor")].concat(),
    );

    let page = drain(&mut gm_rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .map(|p| String::from_utf8_lossy(&p).replace('\0', ""))
        .expect("the manor page");
    assert!(
        page.contains("APPROVED"),
        "the bare enum name, as Java sends"
    );
    assert!(page.contains("30 Adena"), "the current period's cost");
    assert!(
        page.contains("70 Adena"),
        "and the next period's, separately"
    );
    assert!(page.contains("Gludio") && page.contains("Aden"));
    assert!(
        page.find("Gludio").unwrap() < page.find("Aden").unwrap(),
        "castles are listed in id order, not in map order"
    );
    // The next-change stamp is `dd/MM HH:mm:ss` — no year (Java's format).
    assert!(
        page.contains(&commons::util::format_day_month_time(
            crate::game_loop::manor::next_mode_change_at(&world, commons::util::now_millis())
        )),
        "the page carries the scheduled next mode change"
    );
}

/// **An overweight buyer is refused before the adena check.** Java validates
/// weight, then slots, then adena, and the order is visible: a player who is
/// both overloaded and broke is told about the weight.
#[test]
fn buy_seed_refused_when_it_would_exceed_the_weight_limit() {
    let (mut world, mut rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    add_manor_manager(&mut world, 702, 35103, 1);
    world.data.manor.insert_for_test(seed(1, 5016, 5073, 10));
    world.manor.set_seed_production(
        1,
        false,
        vec![SeedProduction {
            seed_id: 5016,
            amount: 500,
            price: 1,
            start_amount: 500,
        }],
    );
    // Give the seed a real weight and the buyer plenty of adena, so weight is
    // the only thing that can refuse the purchase.
    {
        let mut t = world.data.item_data.get(ADENA_ID).unwrap().clone();
        t.item_id = 5016;
        t.name = "Seed".into();
        t.weight = 10_000;
        t.is_stackable = true;
        world.data.item_data.insert_for_test(t);
    }
    inventory::add_inventory_item(&mut world, 100, ADENA_ID, 1_000_000);
    drain(&mut rx);

    let mut w = PacketWriter::new();
    w.write_i32(1);
    w.write_i32(1);
    w.write_i32(5016);
    w.write_i64(500);
    crate::game_loop::manor::handle_request_buy_seed(&mut world, 1, &w.into_bytes());

    assert_eq!(inv_count(&world, 5016), 0, "no seeds delivered");
    assert_eq!(
        world.manor.seed_product(1, 5016, false).unwrap().amount,
        500,
        "stock unchanged"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_HAVE_EXCEEDED_THE_WEIGHT_LIMIT),
        "the buyer is told it is the weight, not the money"
    );
}

/// **The manor autosave is armed at boot and re-arms itself**, which is how the
/// owner's setup reaches the database at all with `AltManorSaveAllActions` off
/// (this dist). With it on, Java never schedules the timer.
#[test]
fn manor_autosave_is_armed_only_when_per_action_saving_is_off() {
    use crate::scheduler::ScheduledTask;

    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    world.cfg.general.alt_manor_save_all_actions = false;
    world.cfg.general.alt_manor_save_period_rate = 2;
    crate::game_loop::manor::schedule_manor_at_boot(&mut world);
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .into_iter()
            .any(|t| matches!(t, ScheduledTask::ManorAutosave)),
        "autosave armed when per-action saving is off"
    );

    let (mut world, _rx) = chamberlain_world();
    world.cfg.general.allow_manor = true;
    world.cfg.general.alt_manor_save_all_actions = true;
    crate::game_loop::manor::schedule_manor_at_boot(&mut world);
    assert!(
        !world
            .scheduler
            .pending_tasks_for_test()
            .into_iter()
            .any(|t| matches!(t, ScheduledTask::ManorAutosave)),
        "per-action saving replaces the timer, as in Java's load()"
    );
}
