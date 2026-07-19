use super::*;

use crate::data::henna_data::Henna;
use crate::data::item_data::{CrystalType, EtcItemType, ItemHandler, ItemKind, ItemTemplate};
use crate::model::components::{BaseStats, HennaSlots};
use crate::model::inventory::Inventory;

const DYE_ITEM: i32 = 4445;
const DYE_ID: i32 = 1;

fn etc_template(item_id: i32, name: &str) -> ItemTemplate {
    ItemTemplate {
        item_id,
        name: name.into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
        etc_item_type: EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    }
}

/// A dye giving STR +5 / CON -2, allowed for class 0.
fn test_dye() -> Henna {
    Henna {
        dye_id: DYE_ID,
        dye_item_id: DYE_ITEM,
        str_: 5,
        con: -2,
        dex: 0,
        int_: 0,
        men: 0,
        wit: 0,
        wear_count: 3,
        wear_fee: 1000,
        cancel_count: 1,
        cancel_fee: 500,
        wear_classes: vec![0],
    }
}

fn has_sm(out: &[Vec<u8>], id: i16) -> bool {
    out.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE && sm_id(p) == id)
}

fn give(world: &mut World, oid: i32, obj_id: i32, item_id: i32, count: i64) {
    let World { objects, data, .. } = world;
    objects.get_component_mut::<Inventory>(&oid).unwrap().add_item(&data.item_data, obj_id, item_id, count);
}

/// Install a class-0 henna slot (via a SECOND_CLASS_GROUP membership → class
/// level 1 → 2 slots), the test dye, its dye item + adena templates, and a
/// live id pool.
fn install(world: &mut World) {
    world.id_pool = 0x4000_0000..0x4000_0100;
    world.data.categories.insert_for_test("SECOND_CLASS_GROUP", &[0]);
    world.data.hennas.insert_for_test(test_dye());
    world.data.item_data.insert_for_test(etc_template(DYE_ITEM, "Dye"));
    world.data.item_data.insert_for_test(etc_template(57, "Adena"));
}

fn str_of(world: &World, oid: i32) -> i32 {
    world.objects.get_component::<BaseStats>(&oid).unwrap().str_
}
fn con_of(world: &World, oid: i32) -> i32 {
    world.objects.get_component::<BaseStats>(&oid).unwrap().con
}

/// Drawing a dye folds its stat bonus into the base stats, consumes the dyes +
/// fee, fills a slot, and pushes UserInfo + HennaInfo.
#[test]
fn equip_henna_changes_stats_and_consumes() {
    let (mut world, ..) = cast_test_world();
    install(&mut world);
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    give(&mut world, 3001, 9001, DYE_ITEM, 5);
    give(&mut world, 3001, 9002, 57, 5000);
    let (str0, con0) = (str_of(&world, 3001), con_of(&world, 3001));
    drain(&mut rx);

    henna::handle_equip(&mut world, 1, DYE_ID);

    assert_eq!(str_of(&world, 3001), str0 + 5, "STR gained the dye bonus");
    assert_eq!(con_of(&world, 3001), con0 - 2, "CON took the dye penalty");
    assert_eq!(world.objects.get_component::<HennaSlots>(&3001).unwrap().0[0], Some(DYE_ID), "slot filled");
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert_eq!(inv.count_of(DYE_ITEM), 2, "3 dyes consumed");
    assert_eq!(inv.adena(), 4000, "1000 adena fee paid");
    let out = drain(&mut rx);
    assert!(out.iter().any(|p| p[0] == server_packets::opcodes::HENNA_INFO), "HennaInfo sent");
    assert!(has_sm(&out, server_packets::sm_ids::THE_SYMBOL_HAS_BEEN_ADDED));
}

/// Removing a dye reverts the stats, charges the cancel fee, and refunds the
/// cancel count of dyes.
#[test]
fn remove_henna_reverts_and_refunds() {
    let (mut world, ..) = cast_test_world();
    install(&mut world);
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    give(&mut world, 3001, 9001, DYE_ITEM, 5);
    give(&mut world, 3001, 9002, 57, 5000);
    let (str0, con0) = (str_of(&world, 3001), con_of(&world, 3001));
    henna::handle_equip(&mut world, 1, DYE_ID);
    drain(&mut rx);

    henna::handle_remove(&mut world, 1, DYE_ID);

    assert_eq!(str_of(&world, 3001), str0, "STR reverted");
    assert_eq!(con_of(&world, 3001), con0, "CON reverted");
    assert_eq!(world.objects.get_component::<HennaSlots>(&3001).unwrap().0[0], None, "slot cleared");
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    // Left after equip: 2 dyes; remove refunds cancel_count (1) → 3.
    assert_eq!(inv.count_of(DYE_ITEM), 3, "cancel count refunded");
    // After equip: 4000 adena; remove charges cancel_fee 500 → 3500.
    assert_eq!(inv.adena(), 3500, "cancel fee charged");
    let out = drain(&mut rx);
    assert!(has_sm(&out, server_packets::sm_ids::THE_SYMBOL_HAS_BEEN_DELETED));
}

/// A base-class character (no henna slots) is refused with NO_SLOT.
#[test]
fn base_class_has_no_henna_slots() {
    let (mut world, ..) = cast_test_world();
    // Note: no SECOND_CLASS_GROUP membership → class 0 stays level 0 → 0 slots.
    world.id_pool = 0x4000_0000..0x4000_0100;
    world.data.hennas.insert_for_test(test_dye());
    world.data.item_data.insert_for_test(etc_template(DYE_ITEM, "Dye"));
    world.data.item_data.insert_for_test(etc_template(57, "Adena"));
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    give(&mut world, 3001, 9001, DYE_ITEM, 5);
    give(&mut world, 3001, 9002, 57, 5000);
    let str0 = str_of(&world, 3001);
    drain(&mut rx);

    henna::handle_equip(&mut world, 1, DYE_ID);

    assert_eq!(str_of(&world, 3001), str0, "no stat change");
    assert!(world.objects.get_component::<HennaSlots>(&3001).unwrap().0[0].is_none(), "no slot filled");
    let out = drain(&mut rx);
    assert!(has_sm(&out, server_packets::sm_ids::NO_SLOT_EXISTS_TO_DRAW_THE_SYMBOL));
}

/// The dye's stat bonus survives a reload: `from_char` folds a stored dye back
/// into the base stats.
#[test]
fn stored_henna_folds_into_base_stats_on_load() {
    let (mut world, ..) = cast_test_world();
    world.data.hennas.insert_for_test(test_dye());
    let mut chr = dummy_char(3002, "Reload");
    chr.hennas = vec![(1, DYE_ID)]; // slot 1
    let bundle = crate::model::Player::from_char(&world.data, &chr);
    // template class-0 base_str 40 (+5 dye), base_con 43 (-2 dye).
    assert_eq!(bundle.base_stats.str_, 45);
    assert_eq!(bundle.base_stats.con, 41);
    assert_eq!(bundle.henna.0[0], Some(DYE_ID));
}
