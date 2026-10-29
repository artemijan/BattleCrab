use super::*;

use crate::data::henna_data::Henna;
use crate::data::item_data::kinds::{CrystalType, EtcItemType, ItemHandler, ItemKind};
use crate::data::item_data::template::ItemTemplate;
use crate::game_loop::character::henna;
use crate::model::components::skills::HennaSlots;
use crate::model::components::stats::BaseStats;
use crate::model::inventory::Inventory;

const DYE_ITEM: i32 = 4445;
const DYE_ID: i32 = 1;

fn etc_template(item_id: i32, name: &str) -> ItemTemplate {
    ItemTemplate {
        trade_flags: Default::default(),
        pre_conditions: Vec::new(),
        is_oly_restricted: false,
        is_event_restricted: false,
        for_npc: false,
        change_weapon_id: 0,
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::kinds::ActionType::Other,
        item_id,
        name: name.into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        is_infinite: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: CrystalType::None,
        blessed: false,
        is_dragon_weapon: false,
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
        luc: 0,
        cha: 0,
        wear_count: 3,
        wear_fee: 1000,
        cancel_count: 1,
        cancel_fee: 500,
        wear_classes: vec![0],
        duration: -1,
    }
}

/// Install a class-0 henna slot (via a SECOND_CLASS_GROUP membership → class
/// level 1 → 2 slots), the test dye, its dye item + adena templates, and a
/// live id pool.
fn install(world: &mut World) {
    world.id_pool = 0x4000_0000..0x4000_0100;
    world
        .data
        .categories
        .insert_for_test("SECOND_CLASS_GROUP", &[0]);
    world.data.hennas.insert_for_test(test_dye());
    world
        .data
        .item_data
        .insert_for_test(etc_template(DYE_ITEM, "Dye"));
    world
        .data
        .item_data
        .insert_for_test(etc_template(57, "Adena"));
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
    give_item(&mut world, 3001, 9001, DYE_ITEM, 5);
    give_item(&mut world, 3001, 9002, 57, 5000);
    let (str0, con0) = (str_of(&world, 3001), con_of(&world, 3001));
    drain(&mut rx);

    henna::handle_equip(&mut world, 1, DYE_ID);

    assert_eq!(str_of(&world, 3001), str0 + 5, "STR gained the dye bonus");
    assert_eq!(con_of(&world, 3001), con0 - 2, "CON took the dye penalty");
    assert_eq!(
        world.objects.get_component::<HennaSlots>(&3001).unwrap().0[0],
        Some(DYE_ID),
        "slot filled"
    );
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert_eq!(inv.count_of(DYE_ITEM), 2, "3 dyes consumed");
    assert_eq!(inv.adena(), 4000, "1000 adena fee paid");
    let out = drain(&mut rx);
    assert!(
        out.iter()
            .any(|p| p[0] == server_packets::opcodes::HENNA_INFO),
        "HennaInfo sent"
    );
    assert!(has_sm(
        &out,
        server_packets::sm_ids::THE_SYMBOL_HAS_BEEN_ADDED
    ));
}

/// Removing a dye reverts the stats, charges the cancel fee, and refunds the
/// cancel count of dyes.
#[test]
fn remove_henna_reverts_and_refunds() {
    let (mut world, ..) = cast_test_world();
    install(&mut world);
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    give_item(&mut world, 3001, 9001, DYE_ITEM, 5);
    give_item(&mut world, 3001, 9002, 57, 5000);
    let (str0, con0) = (str_of(&world, 3001), con_of(&world, 3001));
    henna::handle_equip(&mut world, 1, DYE_ID);
    drain(&mut rx);

    henna::handle_remove(&mut world, 1, DYE_ID);

    assert_eq!(str_of(&world, 3001), str0, "STR reverted");
    assert_eq!(con_of(&world, 3001), con0, "CON reverted");
    assert_eq!(
        world.objects.get_component::<HennaSlots>(&3001).unwrap().0[0],
        None,
        "slot cleared"
    );
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    // Left after equip: 2 dyes; remove refunds cancel_count (1) → 3.
    assert_eq!(inv.count_of(DYE_ITEM), 3, "cancel count refunded");
    // After equip: 4000 adena; remove charges cancel_fee 500 → 3500.
    assert_eq!(inv.adena(), 3500, "cancel fee charged");
    let out = drain(&mut rx);
    assert!(has_sm(
        &out,
        server_packets::sm_ids::THE_SYMBOL_HAS_BEEN_DELETED
    ));
}

/// A base-class character (no henna slots) is refused with NO_SLOT.
#[test]
fn base_class_has_no_henna_slots() {
    let (mut world, ..) = cast_test_world();
    // Note: no SECOND_CLASS_GROUP membership → class 0 stays level 0 → 0 slots.
    world.id_pool = 0x4000_0000..0x4000_0100;
    world.data.hennas.insert_for_test(test_dye());
    world
        .data
        .item_data
        .insert_for_test(etc_template(DYE_ITEM, "Dye"));
    world
        .data
        .item_data
        .insert_for_test(etc_template(57, "Adena"));
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    give_item(&mut world, 3001, 9001, DYE_ITEM, 5);
    give_item(&mut world, 3001, 9002, 57, 5000);
    let str0 = str_of(&world, 3001);
    drain(&mut rx);

    henna::handle_equip(&mut world, 1, DYE_ID);

    assert_eq!(str_of(&world, 3001), str0, "no stat change");
    assert!(
        world.objects.get_component::<HennaSlots>(&3001).unwrap().0[0].is_none(),
        "no slot filled"
    );
    let out = drain(&mut rx);
    assert!(has_sm(
        &out,
        server_packets::sm_ids::NO_SLOT_EXISTS_TO_DRAW_THE_SYMBOL
    ));
}

/// The dye's stat bonus survives a reload: `from_char` folds a stored dye back
/// into the base stats.
#[test]
fn stored_henna_folds_into_base_stats_on_load() {
    let (mut world, ..) = cast_test_world();
    world.data.hennas.insert_for_test(test_dye());
    let mut chr = dummy_char(3002, "Reload");
    chr.hennas = vec![(1, DYE_ID)]; // slot 1
    let bundle = Player::from_char(&world.data, &chr);
    // template class-0 base_str 40 (+5 dye), base_con 43 (-2 dye).
    assert_eq!(bundle.base_stats.str_, 45);
    assert_eq!(bundle.base_stats.con, 41);
    assert_eq!(bundle.henna.0[0], Some(DYE_ID));
}

/// `<duration time>` is read off the dist (Salvation's premium dyes last a
/// week, 10080 minutes), and a dye without one keeps Java's -1.
#[test]
fn dist_dye_durations_load() {
    let hennas = crate::data::henna_data::HennaData::load_from(crate::data::DIST_GAME);
    assert_eq!(hennas.get(500).map(|h| h.duration), Some(10080));
    assert_eq!(hennas.get(1).map(|h| h.duration), Some(-1));
}

/// The protocol-140 draw window adds the dye's duration to each line; the
/// remove window replaces the class flag with two zero ints.
#[test]
fn draw_and_remove_windows_carry_the_salvation_fields() {
    use commons::network::PacketReader;

    let (mut world, ..) = cast_test_world();
    install(&mut world);
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    give_item(&mut world, 3001, 9001, DYE_ITEM, 5);
    give_item(&mut world, 3001, 9002, 57, 5000);
    drain(&mut rx);

    henna::handle_item_list(&mut world, 1);
    let out = drain(&mut rx);
    let list = out
        .iter()
        .find(|p| p[0] == server_packets::opcodes::HENNA_EQUIP_LIST)
        .expect("HennaEquipList");
    let mut r = PacketReader::new(&list[1..]);
    r.read_bytes(8 + 4); // adena, slots
    assert_eq!(r.read_i32(), Some(1));
    r.read_bytes(4 + 4 + 8 + 8 + 4);
    assert_eq!(r.read_i32(), Some(-1), "a permanent dye's duration");
    assert_eq!(r.remaining(), 0);

    henna::handle_equip(&mut world, 1, DYE_ID);
    drain(&mut rx);
    henna::handle_remove_list(&mut world, 1);
    let out = drain(&mut rx);
    let list = out
        .iter()
        .find(|p| p[0] == server_packets::opcodes::HENNA_UNEQUIP_LIST)
        .expect("HennaRemoveList");
    let mut r = PacketReader::new(&list[1..]);
    r.read_bytes(8);
    assert_eq!(r.read_i32(), Some(3));
    assert_eq!(r.read_i32(), Some(1), "one dye worn");
    assert_eq!(r.read_i32(), Some(DYE_ID));
    r.read_bytes(4 + 8 + 8);
    assert_eq!(r.read_i32_array::<2>(), Some([0, 0]));
    assert_eq!(r.remaining(), 0);
}

/// A LUC/CHA dye raises the two stats like any other, and `HennaInfo` writes
/// its totals after WIT.
#[test]
fn a_luc_cha_dye_reaches_the_stats_and_henna_info() {
    use commons::network::PacketReader;

    let (mut world, ..) = cast_test_world();
    install(&mut world);
    world.data.hennas.insert_for_test(Henna {
        str_: 0,
        con: 0,
        luc: 2,
        cha: 1,
        ..test_dye()
    });
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    give_item(&mut world, 3001, 9001, DYE_ITEM, 5);
    give_item(&mut world, 3001, 9002, 57, 5000);
    let before = *world.objects.get_component::<BaseStats>(&3001).unwrap();
    drain(&mut rx);

    henna::handle_equip(&mut world, 1, DYE_ID);

    let after = *world.objects.get_component::<BaseStats>(&3001).unwrap();
    assert_eq!((after.luc, after.cha), (before.luc + 2, before.cha + 1));
    let out = drain(&mut rx);
    let info = out
        .iter()
        .find(|p| p[0] == server_packets::opcodes::HENNA_INFO)
        .expect("HennaInfo");
    let mut r = PacketReader::new(&info[1..]);
    r.read_bytes(6 * 2); // INT … WIT
    assert_eq!(r.read_i16(), Some(2), "LUC");
    assert_eq!(r.read_i16(), Some(1), "CHA");
}

/// Java's preview columns are `getX() ± getHennaValue(X)`: the worn dyes'
/// total, whichever dye is being previewed. With the STR +5 / CON -2 dye worn,
/// previewing a dye with no STR still shows STR +5 in the draw window and
/// STR -5 in the remove window.
#[test]
fn dye_previews_use_the_worn_total() {
    use commons::network::PacketReader;

    let (mut world, ..) = cast_test_world();
    install(&mut world);
    world.data.hennas.insert_for_test(Henna {
        dye_id: DYE_ID + 1,
        str_: 0,
        con: 0,
        wit: 1,
        ..test_dye()
    });
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    give_item(&mut world, 3001, 9001, DYE_ITEM, 5);
    give_item(&mut world, 3001, 9002, 57, 5000);
    henna::handle_equip(&mut world, 1, DYE_ID);
    let (str_, con) = (str_of(&world, 3001), con_of(&world, 3001));
    drain(&mut rx);

    // INT, STR, CON pairs of (current i32, equip i16) after the fixed header.
    let columns = |pkt: &[u8], header: usize| {
        let mut r = PacketReader::new(&pkt[1..]);
        r.read_bytes(header);
        let mut pair = || (r.read_i32().unwrap(), r.read_i16().unwrap());
        let _int = pair();
        (pair(), pair())
    };
    // dye id, item id, count, fee, allowed, adena
    let header = 4 + 4 + 8 + 8 + 4 + 8;

    henna::handle_item_info(&mut world, 1, DYE_ID + 1);
    let out = drain(&mut rx);
    let draw = out
        .iter()
        .find(|p| p[0] == server_packets::opcodes::HENNA_ITEM_INFO)
        .expect("HennaItemDrawInfo");
    assert_eq!(
        columns(draw, header),
        ((str_, (str_ + 5) as i16), (con, (con - 2) as i16))
    );

    henna::handle_item_remove_info(&mut world, 1, DYE_ID + 1);
    let out = drain(&mut rx);
    let remove = out
        .iter()
        .find(|p| p[0] == server_packets::opcodes::HENNA_UNEQUIP_INFO)
        .expect("HennaItemRemoveInfo");
    assert_eq!(
        columns(remove, header),
        ((str_, (str_ - 5) as i16), (con, (con + 2) as i16))
    );
}
