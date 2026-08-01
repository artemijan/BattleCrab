//! Bow / crossbow attacks (G20): ammunition, MP upkeep and the reload delay.

use super::*;

use crate::data::item_data::{CrystalType, EtcItemType, ItemKind, ItemTemplate, WeaponType};
use crate::model::components::RangedReload;
use crate::model::inventory::{Inventory, PaperdollSlot};

const ARCHER: i32 = 2001;
const CID: u32 = 1;
const BOW_ID: i32 = 8100;
const ARROW_ID: i32 = 8101;
const WRONG_GRADE_ARROW: i32 = 8102;

fn template(item_id: i32, name: &str, kind: ItemKind, body_part: i32) -> ItemTemplate {
    ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::Other,
        item_id,
        name: name.into(),
        kind,
        crystal_type: CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        body_part,
        weight: 0,
        is_stackable: kind == ItemKind::Etc,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: crate::data::item_data::ItemHandler::None,
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

/// A world with a bow (mp_consume 1, no grade) and matching arrows registered.
fn bow_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = cast_test_world();

    let mut bow = template(
        BOW_ID,
        "Test Bow",
        ItemKind::Weapon,
        crate::data::item_data::SLOT_LR_HAND,
    );
    bow.mp_consume = 1;
    world.data.item_data.insert_for_test(bow);
    world
        .data
        .item_data
        .set_weapon_type_for_test(BOW_ID, WeaponType::Bow);

    let mut arrow = template(
        ARROW_ID,
        "Test Arrow",
        ItemKind::Etc,
        crate::data::item_data::SLOT_L_HAND,
    );
    arrow.etc_item_type = EtcItemType::Arrow;
    world.data.item_data.insert_for_test(arrow);

    // Same kind, *different* grade — must not be picked up for a no-grade bow.
    let mut wrong = template(
        WRONG_GRADE_ARROW,
        "B-grade Arrow",
        ItemKind::Etc,
        crate::data::item_data::SLOT_L_HAND,
    );
    wrong.etc_item_type = EtcItemType::Arrow;
    wrong.crystal_type = CrystalType::B;
    world.data.item_data.insert_for_test(wrong);

    (world, db, l)
}

/// Equip the bow and give `arrows` of `arrow_id`.
fn arm_archer(world: &mut World, arrows: i64, arrow_id: i32) {
    let bow_obj = give(world, ARCHER, BOW_ID, 1);
    // Disjoint-field split, the same one the equip paths use.
    let (data, objects) = (&world.data, &mut world.objects);
    if let Some(inv) = objects.get_component_mut::<Inventory>(&ARCHER) {
        inv.equip_item(&data.item_data, bow_obj);
    }
    if arrows > 0 {
        give(world, ARCHER, arrow_id, arrows);
    }
}

/// Add an item straight to the inventory with an explicit object id —
/// `cast_test_world` seeds no id pool, so `items::add_inventory_item` would bail
/// on `alloc_object_id`.
fn give(world: &mut World, oid: i32, item_id: i32, count: i64) -> i32 {
    let obj_id = 0x5000_0000 + item_id;
    let World { objects, data, .. } = world;
    objects
        .get_component_mut::<Inventory>(&oid)
        .expect("player has an inventory")
        .add_item(&data.item_data, obj_id, item_id, count);
    obj_id
}

fn arrow_count(world: &World, arrow_id: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&ARCHER)
        .map(|i| i.count_of(arrow_id))
        .unwrap_or(0)
}

fn shoot(world: &mut World, target: i32) {
    crate::game_loop::combat::do_auto_attack(world, ARCHER, target);
}

// ---------------------------------------------------------------------------

/// A bow shot auto-equips a matching arrow, spends one, and costs MP —
/// **the G20 gate line "a bow attack consumes an arrow"**.
#[test]
fn bow_shot_consumes_an_arrow_and_mp() {
    let (mut world, _db, _l) = bow_world();
    let mut out = ingame_caster(&mut world, CID, ARCHER, 0, 0);
    arm_archer(&mut world, 10, ARROW_ID);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 300, 0, 0);
    let mp_before = pvit(&world, ARCHER).cur_mp;
    drain(&mut out);

    shoot(&mut world, NPC_OID);

    assert_eq!(arrow_count(&world, ARROW_ID), 9, "one arrow spent");
    assert!(pvit(&world, ARCHER).cur_mp < mp_before, "the shot cost MP");
    // The arrow was auto-equipped into the left hand (checkAndEquipAmmunition).
    let lhand = world
        .objects
        .get_component::<Inventory>(&ARCHER)
        .unwrap()
        .paperdoll_item_id(PaperdollSlot::LHand);
    assert_eq!(lhand, ARROW_ID, "arrows are equipped in the left hand");
}

/// Firing arms a reload delay, and a second shot inside it is refused (the
/// arrow is not spent twice).
#[test]
fn reload_delay_blocks_the_next_shot() {
    let (mut world, _db, _l) = bow_world();
    let mut out = ingame_caster(&mut world, CID, ARCHER, 0, 0);
    arm_archer(&mut world, 10, ARROW_ID);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 300, 0, 0);
    drain(&mut out);

    shoot(&mut world, NPC_OID);
    assert_eq!(arrow_count(&world, ARROW_ID), 9);
    let ready_at = world
        .objects
        .get_component::<RangedReload>(&ARCHER)
        .expect("reload armed")
        .ready_at_tick;
    assert!(ready_at > world.tick, "the reload delay is in the future");

    // Immediately again: refused, no arrow spent.
    shoot(&mut world, NPC_OID);
    assert_eq!(
        arrow_count(&world, ARROW_ID),
        9,
        "no second arrow while reloading"
    );

    // Past the delay it fires again.
    world.tick = ready_at + 1;
    shoot(&mut world, NPC_OID);
    assert_eq!(
        arrow_count(&world, ARROW_ID),
        8,
        "fires again once reloaded"
    );
}

/// With no arrows the shot is refused with "You have run out of arrows" and the
/// attack intention is dropped.
#[test]
fn out_of_arrows_cancels_the_attack() {
    let (mut world, _db, _l) = bow_world();
    let mut out = ingame_caster(&mut world, CID, ARCHER, 0, 0);
    arm_archer(&mut world, 0, ARROW_ID); // bow, no ammunition
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 300, 0, 0);
    drain(&mut out);

    shoot(&mut world, NPC_OID);

    let pkts = drain(&mut out);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::YOU_HAVE_RUN_OUT_OF_ARROWS),
        "the player is told they are out of arrows"
    );
    assert!(
        !world
            .objects
            .has_component::<crate::model::components::Intent>(&ARCHER),
        "the attack intention is dropped"
    );
}

/// Ammunition must match the bow's **grade** — a B-grade arrow is not picked up
/// for a no-grade bow (`findArrowForBow`'s crystal-type test).
#[test]
fn ammunition_must_match_the_bow_grade() {
    let (mut world, _db, _l) = bow_world();
    let mut out = ingame_caster(&mut world, CID, ARCHER, 0, 0);
    arm_archer(&mut world, 10, WRONG_GRADE_ARROW);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 300, 0, 0);
    drain(&mut out);

    shoot(&mut world, NPC_OID);

    assert_eq!(
        arrow_count(&world, WRONG_GRADE_ARROW),
        10,
        "the wrong-grade stack is untouched"
    );
    let pkts = drain(&mut out);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::YOU_HAVE_RUN_OUT_OF_ARROWS),
        "and the shot is refused as if there were none"
    );
}

/// Too little MP refuses the shot without spending an arrow.
#[test]
fn not_enough_mp_refuses_the_shot() {
    let (mut world, _db, _l) = bow_world();
    let mut out = ingame_caster(&mut world, CID, ARCHER, 0, 0);
    arm_archer(&mut world, 10, ARROW_ID);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 300, 0, 0);
    world
        .objects
        .get_component_mut::<Vitals>(&ARCHER)
        .unwrap()
        .cur_mp = 0.0;
    drain(&mut out);

    shoot(&mut world, NPC_OID);

    assert_eq!(
        arrow_count(&world, ARROW_ID),
        10,
        "no arrow spent when the shot is refused"
    );
    let pkts = drain(&mut out);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::NOT_ENOUGH_MP),
        "the player is told they lack MP"
    );
}

/// A melee weapon is untouched by any of this — no ammunition, no reload.
#[test]
fn melee_attacks_are_unaffected() {
    let (mut world, _db, _l) = bow_world();
    let mut out = ingame_caster(&mut world, CID, ARCHER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 40, 0, 0);
    drain(&mut out);

    // Unarmed (no bow equipped) — the ranged gate must not engage.
    shoot(&mut world, NPC_OID);
    assert!(
        !world.objects.has_component::<RangedReload>(&ARCHER),
        "a melee swing arms no reload delay"
    );
}
