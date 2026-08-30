//! Shadow Weapon Exchange Coupons end-to-end: the village-master desk that
//! turns a class-transfer coupon into a shadow weapon
//! ([`crate::scripts::shadow_weapons`]), and the mana clock that takes the
//! weapon away again ([`crate::game_loop::items::item_mana`]).

use super::*;
use crate::data::multisell_data::MultisellData;
use crate::model::inventory::Inventory;

const DIST: &str = crate::data::DIST_GAME;

/// Shadow Item Exchange Coupon (D-Grade) / (C-Grade).
const COUPON_D: i32 = 8869;
const COUPON_C: i32 = 8870;
/// Shadow Item: Two-handed Sword — the first entry of multisell 306893001,
/// `duration=90`.
const SHADOW_TWO_HANDED_SWORD: i32 = 8821;
/// Rains (30288), an Elf/Human first-class Grand Master on the script's list.
const MASTER_NPC_ID: i32 = 30288;
const NPC_OID: i32 = 5001;
const PLAYER_OID: i32 = 3001;

/// The real item catalog + multisell lists — the exchange is data, so a
/// synthetic catalog would test nothing. (The loader validates every
/// ingredient/product against a template, so this also proves the three
/// restored lists resolve.)
fn shadow_test_world() -> (World, db::CmdRx) {
    let (mut world, db_rx, _link_rx) = quest_test_world();
    world.data.item_data = dist::items_owned();
    world.data.multisells = MultisellData::load_from(DIST, &world.data.item_data, true, true);
    world.id_pool = 0x7000_0000..0x7000_1000;
    add_test_npc(&mut world, NPC_OID, MASTER_NPC_ID, "Rains", 70, 100, 0, 0);
    (world, db_rx)
}

/// Talk the desk: `bypass -h npc_<oid>_Quest ShadowWeapons`.
fn talk_shadow_desk(world: &mut World) {
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ShadowWeapons")),
    );
}

fn mana_of(world: &World, player: i32, item_id: i32) -> Option<i32> {
    inv_item(world, player, item_id).map(|it| it.mana_left)
}

// ---------------------------------------------------------------------------
// The desk: which page each coupon holding opens
// ---------------------------------------------------------------------------

/// Java `ShadowWeapons.onTalk`, all four branches. The page matters because it
/// carries the multisell link — the wrong page offers the wrong grade.
#[test]
fn the_desk_offers_the_page_matching_the_coupons_held() {
    for (d, c, expect_list) in [
        (0, 0, None),
        (15, 0, Some("306893001")),
        (0, 15, Some("306893002")),
        (15, 15, Some("306893003")),
    ] {
        let (mut world, _db_rx) = shadow_test_world();
        let mut rx = ingame_player(&mut world, 1, PLAYER_OID, 0, 0, 0);
        if d > 0 {
            items::add_inventory_item(&mut world, PLAYER_OID, COUPON_D, d);
        }
        if c > 0 {
            items::add_inventory_item(&mut world, PLAYER_OID, COUPON_C, c);
        }
        drain(&mut rx);

        talk_shadow_desk(&mut world);
        let html = drain(&mut rx)
            .iter()
            .find_map(|p| decode_npc_html(p))
            .unwrap_or_else(|| panic!("no html for ({d} D, {c} C)"));

        match expect_list {
            Some(list) => assert!(
                html.contains(&format!("multisell {list}")),
                "({d} D, {c} C) should link multisell {list}, got: {html}"
            ),
            None => assert!(
                html.contains("don't have a Shadow weapon exchange coupon"),
                "({d} D, {c} C) should show the no-coupon page, got: {html}"
            ),
        }
        // The npc object id is substituted, or the link would be inert.
        assert!(
            !html.contains("%objectId%"),
            "the html's %objectId% is replaced"
        );
    }
}

// ---------------------------------------------------------------------------
// The exchange itself
// ---------------------------------------------------------------------------

/// One coupon buys one shadow weapon, and the weapon arrives already charged
/// with its template's `duration` — the 90 minutes that make it temporary.
#[test]
fn a_coupon_buys_a_shadow_weapon_charged_with_its_duration() {
    let (mut world, _db_rx) = shadow_test_world();
    let mut rx = ingame_player(&mut world, 1, PLAYER_OID, 0, 0, 0);
    items::add_inventory_item(&mut world, PLAYER_OID, COUPON_D, 15);
    drain(&mut rx);

    talk_shadow_desk(&mut world);
    drain(&mut rx);
    // The page's link — the same bypass the client sends when clicked.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_multisell 306893001")),
    );
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MULTI_SELL_LIST),
        "the exchange window opens"
    );

    crate::game_loop::multisell::handle_multi_sell_choose(
        &mut world,
        1,
        &multisell_choose_body(306893001, 1, 1),
    );
    drain(&mut rx);

    assert_eq!(
        item_count(&world, PLAYER_OID, COUPON_D),
        14,
        "exactly one coupon spent"
    );
    assert_eq!(
        item_count(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        1,
        "the shadow weapon was granted"
    );
    assert_eq!(
        mana_of(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        Some(90),
        "born with the template's `duration` as mana (Java `Item._mana`)"
    );
}

/// The C-grade list refuses a D-grade coupon: the ingredient is 8870, so a
/// player holding only 8869 pays nothing and gets nothing.
#[test]
fn the_c_grade_exchange_refuses_a_d_grade_coupon() {
    let (mut world, _db_rx) = shadow_test_world();
    let mut rx = ingame_player(&mut world, 1, PLAYER_OID, 0, 0, 0);
    items::add_inventory_item(&mut world, PLAYER_OID, COUPON_D, 15);
    drain(&mut rx);

    crate::game_loop::multisell::separate_and_send(
        &mut world,
        1,
        PLAYER_OID,
        Some(NPC_OID),
        306893002,
        false,
    );
    drain(&mut rx);
    crate::game_loop::multisell::handle_multi_sell_choose(
        &mut world,
        1,
        &multisell_choose_body(306893002, 1, 1),
    );

    assert_eq!(
        item_count(&world, PLAYER_OID, COUPON_D),
        15,
        "the D-grade coupons are untouched"
    );
    let inv = world
        .objects
        .get_component::<Inventory>(&PLAYER_OID)
        .unwrap();
    assert_eq!(inv.items().len(), 1, "nothing was produced");
}

// ---------------------------------------------------------------------------
// The mana clock
// ---------------------------------------------------------------------------

/// Give the player a shadow weapon straight from the catalog and equip it.
/// Returns its object id.
fn equip_shadow_weapon(world: &mut World, rx: &mut UnboundedReceiver<bytes::Bytes>) -> i32 {
    let oid = items::add_inventory_item(world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD, 1)
        .expect("granted")[0];
    items::use_equipable_item(world, 1, PLAYER_OID, oid);
    drain(rx);
    oid
}

/// Equipping burns the first point (Java `Player.useEquipableItem`) and arms
/// the 60 s beat; each beat burns one more while it stays on.
#[test]
fn a_worn_shadow_weapon_burns_one_mana_per_minute() {
    let (mut world, _db_rx) = shadow_test_world();
    let mut rx = ingame_player(&mut world, 1, PLAYER_OID, 0, 0, 0);
    drain(&mut rx);
    let item_oid = equip_shadow_weapon(&mut world, &mut rx);

    assert_eq!(
        mana_of(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        Some(89),
        "one point spent the moment it goes on"
    );
    assert!(
        world.item_mana_consuming.contains_key(&item_oid),
        "the beat is armed (Java `_consumingMana`)"
    );

    // One minute of wear.
    advance_world(
        &mut world,
        crate::game_loop::items::item_mana::MANA_CONSUMPTION_TICKS,
    );
    assert_eq!(
        mana_of(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        Some(88),
        "the 60 s beat spent another point"
    );
    // …and re-armed itself.
    advance_world(
        &mut world,
        crate::game_loop::items::item_mana::MANA_CONSUMPTION_TICKS,
    );
    assert_eq!(
        mana_of(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        Some(87),
        "the beat re-arms while worn"
    );
}

/// A weapon that is *not* worn never starts a beat at all — mana only burns
/// while equipped, which is what makes the 90 minutes usage time rather than
/// wall-clock time.
#[test]
fn an_unworn_shadow_weapon_does_not_burn_mana() {
    let (mut world, _db_rx) = shadow_test_world();
    let mut rx = ingame_player(&mut world, 1, PLAYER_OID, 0, 0, 0);
    items::add_inventory_item(&mut world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD, 1);
    drain(&mut rx);

    advance_world(
        &mut world,
        crate::game_loop::items::item_mana::MANA_CONSUMPTION_TICKS * 5,
    );
    assert_eq!(
        mana_of(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        Some(90),
        "untouched in the bag"
    );
    assert!(world.item_mana_consuming.is_empty(), "no beat was armed");
}

/// At zero the item unequips itself, is destroyed, and the player is told —
/// the whole point of a *shadow* weapon.
#[test]
fn at_zero_mana_the_shadow_weapon_unequips_and_disappears() {
    let (mut world, _db_rx) = shadow_test_world();
    let mut rx = ingame_player(&mut world, 1, PLAYER_OID, 0, 0, 0);
    drain(&mut rx);
    let item_oid = equip_shadow_weapon(&mut world, &mut rx);
    // Fast-forward to the last point.
    world
        .objects
        .get_component_mut::<Inventory>(&PLAYER_OID)
        .unwrap()
        .set_mana_left(item_oid, 1);
    drain(&mut rx);

    advance_world(
        &mut world,
        crate::game_loop::items::item_mana::MANA_CONSUMPTION_TICKS,
    );
    let pkts = drain(&mut rx);

    assert_eq!(
        item_count(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        0,
        "the item is gone"
    );
    assert!(
        world
            .objects
            .get_component::<Inventory>(&PLAYER_OID)
            .unwrap()
            .paperdoll_slot_of(item_oid)
            .is_none(),
        "and no longer on the paperdoll"
    );
    assert!(
        ids_after_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::S1_S_REMAINING_MANA_IS_NOW_0_AND_THE_ITEM_HAS_DISAPPEARED
        ),
        "the disappearance message is sent"
    );
    assert!(
        !world.item_mana_consuming.contains_key(&item_oid),
        "the beat is dropped with the item"
    );
}

/// The 10/5/1 warnings (Java's `switch (_mana)`), each sent exactly once as
/// the counter passes it.
#[test]
fn the_mana_countdown_warns_at_ten_five_and_one() {
    let (mut world, _db_rx) = shadow_test_world();
    let mut rx = ingame_player(&mut world, 1, PLAYER_OID, 0, 0, 0);
    drain(&mut rx);
    let item_oid = equip_shadow_weapon(&mut world, &mut rx);
    world
        .objects
        .get_component_mut::<Inventory>(&PLAYER_OID)
        .unwrap()
        .set_mana_left(item_oid, 11);
    drain(&mut rx);

    let mut seen = Vec::new();
    for _ in 0..10 {
        advance_world(
            &mut world,
            crate::game_loop::items::item_mana::MANA_CONSUMPTION_TICKS,
        );
        seen.extend(ids_after_opcode(
            &drain(&mut rx),
            server_packets::opcodes::SYSTEM_MESSAGE,
        ));
    }
    for id in [
        server_packets::sm_ids::S1_S_REMAINING_MANA_IS_NOW_10,
        server_packets::sm_ids::S1_S_REMAINING_MANA_IS_NOW_5,
        server_packets::sm_ids::S1_S_REMAINING_MANA_IS_NOW_1_IT_WILL_DISAPPEAR_SOON,
    ] {
        assert_eq!(
            seen.iter().filter(|&&s| s == id).count(),
            1,
            "warning {id} sent exactly once"
        );
    }
}

/// A shadow item can't be melted down for free crystals (Java
/// `RequestCrystallizeItem`'s `isShadowItem()` guard) — otherwise a coupon
/// would be a crystal printer.
///
/// The shadow *weapons* declare no `crystal_count`, so they are already
/// refused by the "cannot be crystallized" branch and would pass this test
/// for the wrong reason. The subject here is therefore a Bastard Sword (69,
/// 122 D-grade crystals) with a `duration` stamped onto its template: the
/// shadow guard is then the only thing standing between the player and 122
/// free crystals.
#[test]
fn a_shadow_weapon_cannot_be_crystallized() {
    const BASTARD_SWORD: i32 = 69;
    const CRYSTAL_D: i32 = 1458;
    let (mut world, _db_rx) = shadow_test_world();
    let mut template = world.data.item_data.get(BASTARD_SWORD).unwrap().clone();
    assert!(template.crystal_count > 0, "the subject is crystallizable");
    template.duration = 90; // …and now also a shadow item
    world.data.item_data.insert_for_test(template);

    let mut rx = ingame_player(&mut world, 1, PLAYER_OID, 0, 0, 0);
    let item_oid =
        items::add_inventory_item(&mut world, PLAYER_OID, BASTARD_SWORD, 1).expect("granted")[0];
    // The Crystallize skill above the D-grade requirement, so only the shadow
    // guard can refuse this.
    world.objects.add_components(
        &PLAYER_OID,
        SkillBook(std::collections::HashMap::from([(248, 5)])),
    );
    drain(&mut rx);

    let mut w = PacketWriter::new();
    w.write_i32(item_oid);
    w.write_i64(1);
    items::handle_request_crystallize_item(&mut world, 1, &w.into_bytes());

    assert_eq!(
        item_count(&world, PLAYER_OID, BASTARD_SWORD),
        1,
        "the shadow weapon survives"
    );
    assert_eq!(
        item_count(&world, PLAYER_OID, CRYSTAL_D),
        0,
        "no D-grade crystals were minted"
    );
}

/// Java `Player.useEquipableItem` spends one point **per equip** and none at
/// all when the item comes off — `decreaseMana(false)` sits inside the
/// `if (item.isEquipped())` half of the branch. Toggling a shadow weapon
/// therefore does cost mana on the way on, and that is upstream behaviour, but
/// it must cost exactly one point and only in that direction.
#[test]
fn each_equip_spends_one_point_and_taking_it_off_spends_none() {
    let (mut world, _db_rx) = shadow_test_world();
    let mut rx = ingame_player(&mut world, 1, PLAYER_OID, 0, 0, 0);
    drain(&mut rx);
    let item_oid = equip_shadow_weapon(&mut world, &mut rx);
    assert_eq!(
        mana_of(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        Some(89),
        "the first equip spent one point"
    );

    // Off again — Java's unequip branch never reaches `decreaseMana`.
    items::use_equipable_item(&mut world, 1, PLAYER_OID, item_oid);
    drain(&mut rx);
    assert_eq!(
        mana_of(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        Some(89),
        "taking it off is free"
    );

    // …and back on: one more point, not two.
    items::use_equipable_item(&mut world, 1, PLAYER_OID, item_oid);
    drain(&mut rx);
    assert_eq!(
        mana_of(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        Some(88),
        "the second equip spent exactly one more"
    );
}

/// `finish_equip_change` is the shared tail of far more than the equip click —
/// an enchant refreshing a worn item's glow, an augment re-applying its
/// options, `//mount` stripping a weapon all end there. Java charges mana in
/// `useEquipableItem` alone, so none of those may cost the wearer a point.
#[test]
fn refreshing_a_worn_shadow_weapon_spends_no_mana() {
    let (mut world, _db_rx) = shadow_test_world();
    let mut rx = ingame_player(&mut world, 1, PLAYER_OID, 0, 0, 0);
    drain(&mut rx);
    let item_oid = equip_shadow_weapon(&mut world, &mut rx);
    let after_equip = mana_of(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD);

    // Exactly what `enchant::apply_success` does to a worn item.
    for _ in 0..3 {
        items::finish_equip_change(&mut world, 1, PLAYER_OID, &[item_oid]);
    }
    drain(&mut rx);

    assert_eq!(
        mana_of(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        after_equip,
        "a paperdoll refresh is not an equip"
    );
}

/// Java's `_consumingMana` is a field on the `Item`, so a logout throws it away
/// and the next `EnterWorld` re-arms the beat. Ours is keyed by an object id
/// that comes straight back out of the `items` table, so the flag has to be
/// cleared by hand — and the beat the old session left in flight has to be
/// dropped, or the weapon ends up ticking twice a minute.
#[test]
fn a_relog_inside_the_beat_window_leaves_exactly_one_beat_running() {
    let (mut world, _db_rx) = shadow_test_world();
    let mut rx = ingame_player(&mut world, 1, PLAYER_OID, 0, 0, 0);
    drain(&mut rx);
    let item_oid = equip_shadow_weapon(&mut world, &mut rx);
    let armed_at = crate::game_loop::items::item_mana::MANA_CONSUMPTION_TICKS;
    assert_eq!(
        world.item_mana_consuming.get(&item_oid),
        Some(&armed_at),
        "the first beat is due 60 s out"
    );

    // Half a minute in, the player logs out and straight back in.
    advance_world(&mut world, armed_at / 2);
    crate::game_loop::items::item_mana::on_player_leave_world(&mut world, PLAYER_OID);
    assert!(
        !world.item_mana_consuming.contains_key(&item_oid),
        "the flag does not outlive the session (Java's `Item` is discarded)"
    );
    crate::game_loop::items::item_mana::on_enter_world(&mut world, PLAYER_OID);
    drain(&mut rx);
    assert_eq!(
        mana_of(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        Some(88),
        "`EnterWorld`'s sweep spends its point"
    );
    assert_eq!(
        world.item_mana_consuming.get(&item_oid),
        Some(&(world.tick + armed_at)),
        "and re-arms the beat — the whole point of clearing the flag"
    );

    // The old session's beat comes due first and must be ignored.
    advance_world(&mut world, armed_at / 2);
    assert_eq!(
        mana_of(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        Some(88),
        "the orphaned beat spends nothing"
    );

    // The new one lands on schedule, and re-arms itself once, not twice.
    advance_world(&mut world, armed_at / 2);
    assert_eq!(
        mana_of(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        Some(87),
        "the surviving beat burns a point"
    );
    advance_world(&mut world, armed_at);
    assert_eq!(
        mana_of(&world, PLAYER_OID, SHADOW_TWO_HANDED_SWORD),
        Some(86),
        "one point per minute, not two"
    );
}
