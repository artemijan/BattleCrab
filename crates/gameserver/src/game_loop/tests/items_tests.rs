use super::*;
use crate::game_loop::warehouse;

/// A heal on another player: Heal.java's `power + sqrt(2·mAtk)` amount,
/// overheal-clamped, SM 1067 to the healed target.
#[test]
fn heal_on_other_restores_hp_with_formula() {
    let (mut world, ..) = cast_test_world();
    let mut _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    world
        .objects
        .get_component_mut::<Vitals>(&3002)
        .unwrap()
        .cur_hp = 50.0;
    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut b_rx);

    // TARGET-type skills need no ctrl.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
    assert!(world.objects.has_component::<Casting>(&3001));
    drain(&mut b_rx); // ExRotation + MagicSkillUse

    advance_ticks(&mut world, 10); // hit 500 ms + cancel 500 ms

    let heal = formulas::calc_heal(83.0, pcs(&world, 3001).m_atk, false, false, false, 0, false);
    assert!(
        heal > 50.0,
        "sanity: heal ({heal}) overflows the missing 50 HP"
    );
    assert_eq!(
        pvit(&world, 3002).cur_hp,
        100.0,
        "overheal clamped at max HP"
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_LAUNCHED
    );
    assert_eq!(
        sm_id(&b_rx.try_recv().unwrap()),
        server_packets::sm_ids::S2_HP_HAS_BEEN_RESTORED_BY_C1
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    );
}

/// Equipping gear during a cast is deferred to cast end (Java `UseItem`'s
/// `setNextAction(NextAction(EVT_FINISH_CASTING, …))`), silently — no packet
/// at click time, the equip lands when the cast stops.
#[test]
fn equip_click_during_cast_is_deferred_to_cast_end() {
    use crate::model::components::QueuedAction;
    use crate::model::inventory::Inventory;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    world
        .data
        .item_data
        .insert_for_test(crate::data::item_data::ItemTemplate {
            trade_flags: Default::default(),
            time: -1,
            duration: -1,
            immediate_effect: false,
            ex_immediate_effect: false,
            default_action: crate::data::item_data::ActionType::Other,
            item_id: 2,
            name: "Test Sword".into(),
            kind: crate::data::item_data::ItemKind::Weapon,
            body_part: crate::data::item_data::SLOT_R_HAND,
            weight: 0,
            is_stackable: false,
            is_infinite: false,
            type1: 0,
            type2: 0,
            is_quest_item: false,
            is_sellable: true,
            is_freightable: false,
            price: 0,
            handler: crate::data::item_data::ItemHandler::None,
            crystal_type: crate::data::item_data::CrystalType::None,
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
            etc_item_type: crate::data::item_data::EtcItemType::Other,
            enchant_enabled: false,
            enchant_limit: 0,
            is_magic_weapon: false,
        });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 2, 1);
    }

    // Slow self-cast, then the equip click mid-cast: swallowed silently.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));
    drain(&mut a_rx);
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    assert!(
        a_rx.try_recv().is_err(),
        "no packet at click time (Java sends none)"
    );
    assert!(matches!(
        world.objects.get_component::<QueuedAction>(&3001),
        Some(QueuedAction::UseItem {
            item_object_id: 9001
        })
    ));
    {
        let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
        assert!(
            inv.paperdoll_slot_of(9001).is_none(),
            "not equipped mid-cast"
        );
    }

    // Cast ends (hit 9500 + finish 500 ms): the equip fires.
    advance_ticks(&mut world, 101);
    assert!(
        !world.objects.has_component::<QueuedAction>(&3001),
        "queue consumed"
    );
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(
        inv.paperdoll_slot_of(9001).is_some(),
        "sword equipped at cast end"
    );
    let packets = drain(&mut a_rx);
    assert!(
        !packets.is_empty(),
        "InventoryUpdate/UserInfo sent with the deferred equip"
    );
}

/// End-to-end guard for the ring/earring paperdoll bug: equipping, then
/// swapping, a dual-slot item (earring) must resend `ExUserInfoEquipSlot`
/// (Ex 0x156) — the packet that actually paints the client's own paperdoll —
/// with the correct REar/LEar object ids on *every* click, not just at
/// enter-world.
#[test]
fn equip_swap_resends_ex_user_info_equip_slot_with_correct_slots() {
    use crate::enums::InventorySlot;
    use crate::model::inventory::Inventory;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    for id in [501, 502] {
        world
            .data
            .item_data
            .insert_for_test(crate::data::item_data::ItemTemplate {
                trade_flags: Default::default(),
                time: -1,
                duration: -1,
                immediate_effect: false,
                ex_immediate_effect: false,
                default_action: crate::data::item_data::ActionType::Other,
                item_id: id,
                name: format!("earring{id}"),
                kind: crate::data::item_data::ItemKind::Armor,
                body_part: crate::data::item_data::SLOT_L_EAR | crate::data::item_data::SLOT_R_EAR,
                weight: 0,
                is_stackable: false,
                is_infinite: false,
                type1: 0,
                type2: 0,
                is_quest_item: false,
                is_sellable: true,
                is_freightable: false,
                price: 0,
                handler: crate::data::item_data::ItemHandler::None,
                crystal_type: crate::data::item_data::CrystalType::None,
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
                etc_item_type: crate::data::item_data::EtcItemType::Other,
                enchant_enabled: false,
                enchant_limit: 0,
                is_magic_weapon: false,
            });
    }
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 501, 1);
        inv.add_item(&data.item_data, 9002, 502, 1);
    }

    // Extract (object_id, item_id) for a given InventorySlot from the most
    // recent ExUserInfoEquipSlot packet in `packets`, panicking if absent.
    fn ear_slots(packets: &[Vec<u8>]) -> (i32, i32, i32, i32) {
        let pkt = packets
            .iter()
            .rev()
            .find(|p| p.len() > 2 && p[0] == 0xFE && u16::from_le_bytes([p[1], p[2]]) == 0x156)
            .expect("ExUserInfoEquipSlot not sent");
        let mut offset = 14usize;
        let (mut rear, mut lear) = ((0, 0), (0, 0));
        for slot in InventorySlot::VALUES {
            let block_len = u16::from_le_bytes([pkt[offset], pkt[offset + 1]]) as usize;
            let obj_id = i32::from_le_bytes(pkt[offset + 2..offset + 6].try_into().unwrap());
            let item_id = i32::from_le_bytes(pkt[offset + 6..offset + 10].try_into().unwrap());
            match slot {
                InventorySlot::REar => rear = (obj_id, item_id),
                InventorySlot::LEar => lear = (obj_id, item_id),
                _ => {}
            }
            offset += block_len;
        }
        (rear.0, rear.1, lear.0, lear.1)
    }

    // First earring: fills LEar (equip_item fills left-then-right).
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    let packets = drain(&mut a_rx);
    let (rear_oid, _rear_iid, lear_oid, lear_iid) = ear_slots(&packets);
    assert_eq!(
        (rear_oid, lear_oid, lear_iid),
        (0, 9001, 501),
        "first earring lands in LEar"
    );

    // Second earring: fills the free REar slot, LEar untouched.
    items::handle_use_item(&mut world, 1, &use_item_body(9002));
    let packets = drain(&mut a_rx);
    let (rear_oid, rear_iid, lear_oid, lear_iid) = ear_slots(&packets);
    assert_eq!(
        (rear_oid, rear_iid, lear_oid, lear_iid),
        (9002, 502, 9001, 501),
        "second earring lands in REar, first stays put"
    );

    // Clicking an *already-equipped* earring toggles it back off. Java
    // resolves this via `getSlotFromItem` (the single-bit slot the item
    // currently occupies), not the item's raw (combined, for ears/fingers)
    // template body part — passing the latter used to silently no-op.
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    let packets = drain(&mut a_rx);
    assert!(
        !packets.is_empty(),
        "unequip-via-click must send packets, not silently no-op"
    );
    let (rear_oid, rear_iid, lear_oid, _lear_iid) = ear_slots(&packets);
    assert_eq!(
        (rear_oid, rear_iid, lear_oid),
        (9002, 502, 0),
        "LEar cleared, REar untouched"
    );
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(
        inv.paperdoll_slot_of(9001).is_none(),
        "first earring actually unequipped"
    );
}

/// Destroying a *worn* item must repaint the client's paperdoll, not just the
/// inventory list. Java gets this for free — `Inventory.removeItem` unequips
/// whatever it takes out of the bag, and `setPaperdollItem` pushes
/// `ExUserInfoEquipSlot` — but here the paperdoll lives in a data component
/// that cannot reach the client, so `quests::take_items` has to drive it.
///
/// Reported against Q229 `Test of Witchcraft`: the Sword of Seal (3029) is a
/// registered quest item *and* a weapon, so the hand-in's `exitQuest` sweep
/// destroys it straight out of the player's hand. Without the unequip the
/// client kept rendering the sword — `UserInfo` carries only the right-hand
/// *enchant level*, never the paperdoll item ids — while the inventory window
/// correctly showed nothing equipped.
#[test]
fn destroying_an_equipped_quest_item_repaints_the_paperdoll() {
    use crate::data::item_data::{CrystalType, ItemHandler, ItemKind, ItemTemplate, SLOT_R_HAND};
    use crate::enums::InventorySlot;
    use crate::model::inventory::Inventory;

    const SWORD: i32 = 3029;
    const SWORD_OID: i32 = 9101;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: true,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::Other,
        item_id: SWORD,
        name: "Sword of Seal".to_string(),
        kind: ItemKind::Weapon,
        body_part: SLOT_R_HAND,
        weight: 1200,
        is_stackable: false,
        is_infinite: false,
        type1: 0,
        type2: 0,
        // Registered as a quest item by Q229 — and still equippable.
        is_quest_item: true,
        is_sellable: false,
        is_freightable: false,
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
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, SWORD_OID, SWORD, 1);
    }

    // The object id the latest ExUserInfoEquipSlot reports for the right hand.
    fn rhand_oid(packets: &[Vec<u8>]) -> i32 {
        let pkt = packets
            .iter()
            .rev()
            .find(|p| p.len() > 2 && p[0] == 0xFE && u16::from_le_bytes([p[1], p[2]]) == 0x156)
            .expect("ExUserInfoEquipSlot not sent");
        let mut offset = 14usize;
        for slot in InventorySlot::VALUES {
            let block_len = u16::from_le_bytes([pkt[offset], pkt[offset + 1]]) as usize;
            if slot == InventorySlot::RHand {
                return i32::from_le_bytes(pkt[offset + 2..offset + 6].try_into().unwrap());
            }
            offset += block_len;
        }
        panic!("no RHand block in ExUserInfoEquipSlot");
    }

    items::handle_use_item(&mut world, 1, &use_item_body(SWORD_OID));
    let packets = drain(&mut a_rx);
    assert_eq!(rhand_oid(&packets), SWORD_OID, "sword equipped in RHand");

    // `exitQuest`'s registered-quest-item sweep: destroy every one of them.
    let taken = crate::game_loop::quests::take_items(&mut world, 1, 3001, SWORD, -1);
    assert!(taken, "the sword was destroyed");

    let packets = drain(&mut a_rx);
    assert_eq!(
        rhand_oid(&packets),
        0,
        "destroying the worn sword must clear the client's RHand slot"
    );
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(
        inv.paperdoll_slot_of(SWORD_OID).is_none() && inv.count_of(SWORD) == 0,
        "sword gone from both the paperdoll and the bag"
    );
}

/// The bug this guards: equipping gear moved the paperdoll but never recomputed
/// combat stats, so a freshly-equipped weapon's P.Atk / armor's P.Def never
/// reached the client's stat panel. `finish_equip_change` now reruns
/// `recalculate_stats`, and the weapon's stat *replaces* the naked base while
/// armor's *sums* on top (matching the Java finalizers).
#[test]
fn equipping_gear_updates_combat_stats() {
    use crate::data::item_data::{
        CrystalType, ItemHandler, ItemKind, ItemStats, ItemTemplate, SLOT_CHEST, SLOT_R_HAND,
    };
    use crate::model::inventory::Inventory;
    use crate::model::stats::Stat;

    let (mut world, ..) = cast_test_world();
    let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let template = |item_id: i32, kind: ItemKind, body_part: i32| ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::Other,
        item_id,
        name: format!("gear{item_id}"),
        kind,
        body_part,
        weight: 0,
        is_stackable: false,
        is_infinite: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
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
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    };
    // Weapon P.Atk 500 (well above the class base of 100, so equip must raise
    // P.Atk); chest armor P.Def 30 (class base P.Def is 0, so it must appear).
    world
        .data
        .item_data
        .insert_for_test(template(500, ItemKind::Weapon, SLOT_R_HAND));
    world.data.item_data.set_item_stats_for_test(
        500,
        ItemStats {
            bonuses: vec![(Stat::PhysicalAttack, 500.0)],
            ..Default::default()
        },
    );
    world
        .data
        .item_data
        .insert_for_test(template(510, ItemKind::Armor, SLOT_CHEST));
    world.data.item_data.set_item_stats_for_test(
        510,
        ItemStats {
            bonuses: vec![(Stat::PhysicalDefence, 30.0)],
            ..Default::default()
        },
    );
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 500, 1);
        inv.add_item(&data.item_data, 9002, 510, 1);
    }

    let base_p_atk = pcs(&world, 3001).p_atk;
    let base_p_def = pcs(&world, 3001).p_def;

    // Equip the weapon → P.Atk jumps (weapon base 500 replaces the naked 100).
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    assert!(
        pcs(&world, 3001).p_atk > base_p_atk,
        "equipping a weapon must raise P.Atk (was {base_p_atk}, now {})",
        pcs(&world, 3001).p_atk
    );

    // Equip the armor → P.Def rises by its contribution, P.Atk unchanged.
    let after_weapon_p_atk = pcs(&world, 3001).p_atk;
    items::handle_use_item(&mut world, 1, &use_item_body(9002));
    assert!(
        pcs(&world, 3001).p_def > base_p_def,
        "equipping armor must raise P.Def (was {base_p_def}, now {})",
        pcs(&world, 3001).p_def
    );
    assert_eq!(
        pcs(&world, 3001).p_atk,
        after_weapon_p_atk,
        "armor doesn't touch P.Atk"
    );

    // Unequip the weapon → P.Atk falls back to the naked value.
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    assert_eq!(
        pcs(&world, 3001).p_atk,
        base_p_atk,
        "unequipping the weapon restores naked P.Atk"
    );
}

/// Companion to the combat-stat test: `maxMp` (and `maxHp`) item bonuses live
/// in `Vitals`, computed on a separate path from `recalculate_stats`. Equipping
/// +MP jewelry must raise Max MP; unequipping restores it and clamps current MP.
#[test]
fn equipping_gear_updates_max_hp_mp() {
    use crate::data::item_data::{
        CrystalType, ItemHandler, ItemKind, ItemStats, ItemTemplate, SLOT_NECK,
    };
    use crate::model::components::Vitals;
    use crate::model::inventory::Inventory;
    use crate::model::stats::Stat;

    let (mut world, ..) = cast_test_world();
    let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // A necklace granting +100 Max MP.
    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::Other,
        item_id: 520,
        name: "MP Necklace".into(),
        kind: ItemKind::Armor,
        body_part: SLOT_NECK,
        weight: 0,
        is_stackable: false,
        is_infinite: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
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
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    world.data.item_data.set_item_stats_for_test(
        520,
        ItemStats {
            bonuses: vec![(Stat::MaxMp, 100.0)],
            ..Default::default()
        },
    );
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9003, 520, 1);
    }

    let base_max_mp = world.objects.get_component::<Vitals>(&3001).unwrap().max_mp;

    // Equip → Max MP rises by exactly the item's flat bonus.
    items::handle_use_item(&mut world, 1, &use_item_body(9003));
    assert_eq!(
        world.objects.get_component::<Vitals>(&3001).unwrap().max_mp,
        base_max_mp + 100,
        "equipping +100 MP jewelry raises Max MP by 100"
    );

    // Unequip → Max MP falls back, and current MP is clamped to the new max.
    items::handle_use_item(&mut world, 1, &use_item_body(9003));
    let v = world.objects.get_component::<Vitals>(&3001).unwrap();
    assert_eq!(v.max_mp, base_max_mp, "unequipping restores base Max MP");
    assert!(
        v.cur_mp <= v.max_mp as f64,
        "current MP clamped to the lowered max"
    );
}

/// The bug this guards: `UseItem` on a non-equipable `EtcItem` used to be a
/// silent no-op (`is_equipable() == false` → early return before any handler
/// dispatch existed), so pack/box items like "Mage Class Equipment Set"
/// never unpacked in-game. `ExtractableItems` should destroy the pack and
/// grant its `<capsuled_items>` contents.
#[test]
fn extractable_pack_item_unpacks_into_its_contents() {
    use crate::data::item_data::{CapsuledItem, ItemHandler, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::Other,
        item_id: 15195,
        name: "Mage Class Equipment Set".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: false,
        is_infinite: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::ExtractableItems,
        crystal_type: crate::data::item_data::CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        capsuled_items: vec![
            CapsuledItem {
                item_id: 15230,
                min: 1,
                max: 1,
                chance: 100_000,
            },
            CapsuledItem {
                item_id: 15270,
                min: 1,
                max: 1,
                chance: 100_000,
            },
        ],
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    for item_id in [15230, 15270] {
        world.data.item_data.insert_for_test(ItemTemplate {
            trade_flags: Default::default(),
            time: -1,
            duration: -1,
            immediate_effect: false,
            ex_immediate_effect: false,
            default_action: crate::data::item_data::ActionType::Other,
            item_id,
            name: format!("Pack Content {item_id}"),
            kind: ItemKind::Etc,
            body_part: 0,
            weight: 0,
            is_stackable: false,
            is_infinite: false,
            type1: 4,
            type2: 5,
            is_quest_item: false,
            is_sellable: true,
            is_freightable: false,
            price: 0,
            handler: ItemHandler::None,
            crystal_type: crate::data::item_data::CrystalType::None,
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
            etc_item_type: crate::data::item_data::EtcItemType::Other,
            enchant_enabled: false,
            enchant_limit: 0,
            is_magic_weapon: false,
        });
    }
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 15195, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(
        inv.items().iter().all(|i| i.item_id != 15195),
        "pack consumed"
    );
    assert!(
        inv.items().iter().any(|i| i.item_id == 15230),
        "first capsule granted"
    );
    assert!(
        inv.items().iter().any(|i| i.item_id == 15270),
        "second capsule granted"
    );

    let packets = drain(&mut rx);
    let obtained_count = ids_after_opcode(&packets, server_packets::opcodes::SYSTEM_MESSAGE)
        .into_iter()
        .filter(|&id| id == server_packets::sm_ids::YOU_HAVE_OBTAINED_S1)
        .count();
    assert_eq!(obtained_count, 2, "one obtained-message per capsule item");

    // Memory-first: the consumed pack instance (object 9001) is gone from the
    // Inventory component (asserted above as "pack consumed"); it persists as a
    // deletion on the next flush, not per use — so no per-action DB write.
}

/// The bug this guards: a capsule entry with `min == max == 2` on a
/// non-stackable, equipable item (e.g. the real "Jewelry Pack"'s Majestic
/// Earring/Ring, `min="2" max="2"` in `15200-15299.xml`) used to be granted
/// as a single item instance with `count == 2` — a state the paperdoll can't
/// represent. The client showed "you obtained 2" but only one icon in the
/// bag, and equipping it made the whole pair disappear (one unit moved to
/// the paperdoll, the other had no object id of its own to remain behind
/// with). `ItemContainer.addItem` in Java splits any non-stackable count
/// into one instance per unit; this asserts the Rust port now does too.
#[test]
fn extractable_pack_item_splits_non_stackable_multi_count_capsule() {
    use crate::data::item_data::{self, CapsuledItem, ItemHandler, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: item_data::ActionType::Other,
        item_id: 15274,
        name: "Jewelry Pack (A-grade)".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: false,
        is_infinite: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::ExtractableItems,
        crystal_type: item_data::CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        capsuled_items: vec![CapsuledItem {
            item_id: 14966,
            min: 2,
            max: 2,
            chance: 100_000,
        }],
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
        etc_item_type: item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: item_data::ActionType::Other,
        item_id: 14966,
        name: "Majestic Earring of Fortune".into(),
        kind: ItemKind::Armor,
        body_part: item_data::SLOT_LR_EAR,
        weight: 0,
        is_stackable: false,
        is_infinite: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: item_data::CrystalType::None,
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
        etc_item_type: item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 15274, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let earring_oids: Vec<i32> = {
        let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
        inv.items()
            .iter()
            .filter(|i| i.item_id == 14966)
            .map(|i| i.object_id)
            .collect()
    };
    assert_eq!(
        earring_oids.len(),
        2,
        "two separate earring instances, not one instance with count 2"
    );
    {
        let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
        for oid in &earring_oids {
            assert_eq!(
                inv.items()
                    .iter()
                    .find(|i| i.object_id == *oid)
                    .unwrap()
                    .count,
                1,
                "each instance is a single unit"
            );
        }
    }

    let packets = drain(&mut rx);
    let obtained_two = ids_after_opcode(&packets, server_packets::opcodes::SYSTEM_MESSAGE)
        .into_iter()
        .any(|id| id == server_packets::sm_ids::YOU_HAVE_OBTAINED_S2_S1);
    assert!(obtained_two, "message reports the pair as a count-2 grant");

    // Equipping one instance must not touch (or vanish) the other.
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.equip_item(&data.item_data, earring_oids[0]);
    }
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(
        inv.items().iter().any(|i| i.object_id == earring_oids[1]),
        "second earring still in the bag, not vanished"
    );
}

/// The bug this guards: `extract_item` used to grant capsule rewards with no
/// capacity check at all, so a full inventory would silently overflow.
/// `ExtractableItems.useItem` refuses (leaving the box untouched) once
/// non-quest item count reaches 80% of the inventory cap
/// (`Player.isInventoryUnder80(false)`).
#[test]
fn extractable_pack_item_blocked_when_inventory_is_over_80_percent() {
    use crate::data::item_data::{CapsuledItem, ItemHandler, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    assert_eq!(
        world.cfg.character.inventory_max_no_dwarf, 80,
        "test assumes the default 80-slot cap"
    );

    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::Other,
        item_id: 15195,
        name: "Mage Class Equipment Set".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: false,
        is_infinite: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::ExtractableItems,
        crystal_type: crate::data::item_data::CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        capsuled_items: vec![CapsuledItem {
            item_id: 15230,
            min: 1,
            max: 1,
            chance: 100_000,
        }],
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });

    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        // 65 items (> 80% of the 80-slot cap), the pack itself included.
        for i in 0..64 {
            inv.add_item(&data.item_data, 9100 + i, 20000 + i, 1);
        }
        inv.add_item(&data.item_data, 9001, 15195, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(
        inv.items().iter().any(|i| i.item_id == 15195),
        "pack not consumed when inventory is full"
    );
    assert!(
        inv.items().iter().all(|i| i.item_id != 15230),
        "no capsule granted when inventory is full"
    );

    let packets = drain(&mut rx);
    let full_count = ids_after_opcode(&packets, server_packets::opcodes::SYSTEM_MESSAGE)
        .into_iter()
        .filter(|&id| id == server_packets::sm_ids::YOUR_INVENTORY_IS_FULL)
        .count();
    assert_eq!(full_count, 1, "YOUR_INVENTORY_IS_FULL sent");
}

/// `ItemSkills` (the `handlers/itemhandlers/ItemSkillsTemplate` port): a
/// self-targeted potion heals immediately (no cast bar) and consumes one
/// unit from the stack; a second use inside the skill's reuse window is
/// blocked and doesn't consume another.
#[test]
fn item_skill_potion_heals_and_enforces_reuse() {
    use crate::data::item_data::{ItemHandler, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;
    use crate::model::skill::SkillEffect;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        without_action: false,
        trait_type: model::skill::TraitType::None,
        item_consume_id: 0,
        item_consume_count: 0,
        id: 2031,
        level: 1,
        name: "Lesser Healing Potion".into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Self_,
        magic_type: 1,
        magic_level: 0,
        effect_point: 100,
        cast_range: 0,
        effect_range: 0,
        hit_time: 0,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 6000,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 0,
        abnormal_level: 0,
        abnormal_type: "NONE".into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects: vec![SkillEffect::Heal { power: 30.0 }],
        ..Default::default()
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: true,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::SkillReduce,
        item_id: 9910,
        name: "Lesser Healing Potion".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        is_infinite: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::ItemSkills,
        crystal_type: crate::data::item_data::CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(2031, 1)],
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    if let Some(vitals) = world.objects.get_component_mut::<Vitals>(&3001) {
        vitals.max_hp = 100;
        vitals.cur_hp = 10.0;
    }
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 9910, 2);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    assert_eq!(pvit(&world, 3001).cur_hp, 40.0, "10 + Heal(30)");
    {
        let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
        let potion = inv
            .items()
            .iter()
            .find(|i| i.item_id == 9910)
            .expect("one potion left");
        assert_eq!(potion.count, 1, "one unit consumed");
    }
    let packets = drain(&mut rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::STATUS_UPDATE),
        "heal must push an HP StatusUpdate"
    );
    // Memory-first: no per-use DB write; the remaining stack lives in the
    // Inventory component (asserted below) and persists on the next flush.

    // Second use, same tick: reuse still active, no extra heal or consume.
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    assert_eq!(
        pvit(&world, 3001).cur_hp,
        40.0,
        "reuse blocks a second heal"
    );
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    let potion = inv
        .items()
        .iter()
        .find(|i| i.item_id == 9910)
        .expect("still one potion left");
    assert_eq!(potion.count, 1, "reuse blocks a second consume");
}

/// `ItemSkillsTemplate` only fires an item's skills instantly on the
/// `triggerCast` branch (`withoutAction`, or the item's `immediate_effect`/
/// `ex_immediate_effect`); everything else goes through `useMagic` and gets a
/// real cast bar. This models the Scroll of Escape (item 736 → skill 2013,
/// `hitTime` 20000, `SKILL_REDUCE`, no `immediate_effect`): using it must
/// *start a 20 s cast* rather than teleport on the spot, and the scroll is
/// spent up front because `checkConsume` returns `hasConsumeSkill` for a
/// `SKILL_REDUCE` item whose skill declares an `itemConsumeId`.
#[test]
fn non_immediate_item_skill_casts_instead_of_firing_instantly() {
    use crate::data::item_data::{ItemHandler, ItemKind, ItemTemplate};
    use crate::model::components::Casting;
    use crate::model::inventory::Inventory;
    use crate::model::skill::SkillEffect;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.data.map_region =
        crate::data::MapRegionData::from_regions(vec![crate::data::map_region::MapRegion {
            name: "test_town".into(),
            loc_id: 924,
            bbs: 0,
            respawn_points: vec![(5000, 6000, -30)],
            tiles: vec![(20, 18)],
        }]);
    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        id: 2013,
        level: 1,
        name: "Scroll of Escape".into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Self_,
        magic_type: 2, // static: hitTime used verbatim
        magic_level: 0,
        effect_point: 0,
        cast_range: 0,
        effect_range: 0,
        hit_time: 20_000,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        without_action: false,
        trait_type: model::skill::TraitType::None,
        item_consume_id: 9909,
        item_consume_count: 1,
        abnormal_time: 0,
        abnormal_level: 0,
        abnormal_type: "NONE".into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects: vec![SkillEffect::Escape {
            dest: model::skill::EscapeDest::Town,
        }],
        ..Default::default()
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::SkillReduce,
        item_id: 9909,
        name: "Scroll of Escape".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        is_infinite: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::ItemSkills,
        crystal_type: crate::data::item_data::CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(2013, 1)],
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    if let Some(vitals) = world.objects.get_component_mut::<Vitals>(&3001) {
        vitals.cur_hp = 100.0;
    }
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9002, 9909, 2);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9002));

    // The cast started — it did NOT resolve inline.
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "20 s cast in progress"
    );
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "cast bar shown"
    );
    assert!(
        !pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION),
        "must not teleport before the cast lands"
    );
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y), (0, 0), "still at the origin mid-cast");
    // Spent at cast start, like Java's `successfulUse` -> `checkConsume`.
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert_eq!(
        inv.items()
            .iter()
            .find(|i| i.item_id == 9909)
            .expect("scroll")
            .count,
        1,
        "one consumed up front"
    );

    // 20 s later (200 ticks) + the finish floor: now it teleports.
    advance_ticks(&mut world, 210);
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (5000, 6000, -25),
        "escaped once the cast landed"
    );
}

/// The bug this guards: a `Restoration`-effect skill (e.g. the "Mysterious
/// Blessed Spiritshot Pack" line, item 22599 → skill 22490) used to parse
/// with an empty effect list — `SkillEffect::GiveItem`/`GiveItemRandom`
/// didn't exist yet — so `use_item_skills` still consumed the pack (a skill
/// was found and "cast") but granted nothing: the pack just disappeared.
#[test]
fn item_skill_give_item_grants_reward_and_consumes_pack() {
    use crate::data::item_data::{ItemHandler, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;
    use crate::model::skill::SkillEffect;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        without_action: false,
        trait_type: model::skill::TraitType::None,
        item_consume_id: 0,
        item_consume_count: 0,
        id: 22490,
        level: 5,
        name: "Mysterious Spiritshot d 5000".into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Self_,
        magic_type: 2,
        magic_level: 0,
        effect_point: 0,
        cast_range: 0,
        effect_range: 0,
        hit_time: 0,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 0,
        abnormal_level: 0,
        abnormal_type: "NONE".into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects: vec![SkillEffect::GiveItem {
            item_id: 21852,
            item_count: 5000,
            item_enchant_level: 0,
        }],
        ..Default::default()
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: true,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::SkillReduce,
        item_id: 22599,
        name: "Mysterious Blessed Spiritshot Pack (5000) (D-grade)".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 1000,
        is_stackable: true,
        is_infinite: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::ItemSkills,
        crystal_type: crate::data::item_data::CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(22490, 5)],
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::Other,
        item_id: 21852,
        name: "Blessed Spiritshot: D-grade".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        is_infinite: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None,
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
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 22599, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(
        inv.items().iter().all(|i| i.item_id != 22599),
        "pack consumed"
    );
    let shots = inv
        .items()
        .iter()
        .find(|i| i.item_id == 21852)
        .expect("5000 Blessed Spiritshots granted, not lost");
    assert_eq!(shots.count, 5000);

    let packets = drain(&mut rx);
    assert!(
        ids_after_opcode(&packets, server_packets::opcodes::SYSTEM_MESSAGE)
            .into_iter()
            .any(|id| id == server_packets::sm_ids::YOU_HAVE_OBTAINED_S2_S1),
        "reward message sent"
    );
}

/// `RestorationRandom` (e.g. "Quiver of Arrow"-shaped skills): exactly one
/// weighted group is picked and its items granted together, matching Java's
/// `100 * Rnd.nextDouble()` roulette roll against the raw 0-100 `chance`
/// values.
#[test]
fn item_skill_give_item_random_grants_one_weighted_group() {
    use crate::data::item_data::{ItemHandler, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;
    use crate::model::skill::{
        AffectObject, AffectScope, RestorationGroup, RestorationItem, SkillEffect,
    };

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    // `apply_skill_effects` rolls a magic-crit check unconditionally before
    // walking the effect list (unused here since this isn't a
    // `MagicalAttack`) — force it out of the queue first, then force the
    // roulette roll: `roll_f64` reads a forced value `v` as `v / 1_000_000`,
    // so 600_000 -> 0.6 -> `100 * 0.6 = 60`, landing in the second slice
    // (30..80) below.
    world.force_roll(0);
    world.force_roll(600_000);

    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        without_action: false,
        trait_type: model::skill::TraitType::None,
        item_consume_id: 0,
        item_consume_count: 0,
        id: 323,
        level: 1,
        name: "Quiver of Arrow".into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Self_,
        magic_type: 2,
        magic_level: 0,
        effect_point: 0,
        cast_range: 0,
        effect_range: 0,
        hit_time: 0,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 0,
        abnormal_level: 0,
        abnormal_type: "NONE".into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects: vec![SkillEffect::GiveItemRandom {
            groups: vec![
                RestorationGroup {
                    chance: 30.0,
                    items: vec![RestorationItem {
                        item_id: 1344,
                        count: 700,
                        min_enchant: 0,
                        max_enchant: 0,
                    }],
                },
                RestorationGroup {
                    chance: 50.0,
                    items: vec![RestorationItem {
                        item_id: 1344,
                        count: 1400,
                        min_enchant: 0,
                        max_enchant: 0,
                    }],
                },
                RestorationGroup {
                    chance: 20.0,
                    items: vec![RestorationItem {
                        item_id: 1344,
                        count: 2800,
                        min_enchant: 0,
                        max_enchant: 0,
                    }],
                },
            ],
        }],
        ..Default::default()
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::Other,
        item_id: 1344,
        name: "Mithril Arrow".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        is_infinite: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None,
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
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: true,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::SkillReduce,
        item_id: 9999,
        name: "Quiver of Arrow scroll".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        is_infinite: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::ItemSkills,
        crystal_type: crate::data::item_data::CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(323, 1)],
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 9999, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    let arrows = inv
        .items()
        .iter()
        .find(|i| i.item_id == 1344)
        .expect("arrows granted");
    assert_eq!(
        arrows.count, 1400,
        "roll 60 lands in the 30..80 (second) slice"
    );
    let _ = &mut rx;
}

/// `RestorationRandom` with `maxEnchant > 0` rolls `Rnd.get(min, max)` (inclusive)
/// onto the created non-stackable item and sends the "obtained a +S1 S2" message.
#[test]
fn item_skill_give_item_random_rolls_enchant_on_created_item() {
    use crate::data::item_data::{ItemHandler, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;
    use crate::model::skill::{RestorationGroup, RestorationItem, SkillEffect};

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);
    // Forced rolls, in consumption order: crit check (0), roulette `roll_f64`
    // (500_000 -> 0.5 -> 50, inside the single 0..100 slice), then the enchant
    // `roll(max-min+1)` = `roll(3)`; forcing 1 -> enchant = min(3) + 1 = 4.
    world.force_roll(0);
    world.force_roll(500_000);
    world.force_roll(1);

    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        without_action: false,
        trait_type: model::skill::TraitType::None,
        item_consume_id: 0,
        item_consume_count: 0,
        id: 324,
        level: 1,
        name: "Enchanted Reward".into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Self_,
        magic_type: 2,
        magic_level: 0,
        effect_point: 0,
        cast_range: 0,
        effect_range: 0,
        hit_time: 0,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 0,
        abnormal_level: 0,
        abnormal_type: "NONE".into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects: vec![SkillEffect::GiveItemRandom {
            groups: vec![RestorationGroup {
                chance: 100.0,
                items: vec![RestorationItem {
                    item_id: 6001,
                    count: 1,
                    min_enchant: 3,
                    max_enchant: 5,
                }],
            }],
        }],
        ..Default::default()
    });
    // The reward is a non-stackable weapon so it carries an enchant.
    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::Other,
        item_id: 6001,
        name: "Enchanted Blade".into(),
        kind: ItemKind::Weapon,
        body_part: 0,
        weight: 0,
        is_stackable: false,
        is_infinite: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None,
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
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: true,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::SkillReduce,
        item_id: 9998,
        name: "Enchanted Reward scroll".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        is_infinite: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::ItemSkills,
        crystal_type: crate::data::item_data::CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(324, 1)],
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 9998, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    let blade = inv
        .items()
        .iter()
        .find(|i| i.item_id == 6001)
        .expect("blade granted");
    assert_eq!(
        blade.enchant_level, 4,
        "Rnd.get(3, 5) with forced roll 1 -> +4"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_HAVE_OBTAINED_A_S1_S2),
        "enchanted single grant uses the +S1 S2 message",
    );
}

/// RequestRestart: the player is stored + removed, the client gets
/// `RestartResponse(true)`, drops back to `Authenticated`, and the reloaded
/// character list flows through the normal lobby path.
#[test]
fn restart_stores_player_and_returns_to_lobby() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut out_rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&5001).unwrap();
        p.exp = 1234;
    }
    world
        .objects
        .get_component_mut::<Position>(&5001)
        .unwrap()
        .x = 777;

    handle_request_restart(&mut world, 1);

    // storeMe: the snapshot carries the live (not the loaded) state, and
    // is queued before the character-list reload.
    let save = expect_store_player(&mut db_rx);
    assert_eq!(
        (save.base.object_id, save.base.exp, save.base.x),
        (5001, 1234, 777)
    );
    match db_rx.try_recv() {
        Ok(db::DbCommand::LoadCharacters { client_id, account }) => {
            assert_eq!((client_id, account.as_str()), (1, "bob"));
        }
        _ => panic!("expected a LoadCharacters DB command after the store"),
    }

    // deleteMe + setConnectionState(AUTHENTICATED) + RestartResponse.TRUE.
    assert_eq!(world.objects.count::<Player>(), 0);
    assert!(matches!(
        world.clients.get(&1),
        Some(ClientSession::Authenticated(_))
    ));
    let pkt = out_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::RESTART_RESPONSE);
    assert_eq!(pkt[1], 1, "RestartResponse.TRUE");

    // The reload result lands like any character-list load: InLobby +
    // CharSelectionInfo.
    on_characters_loaded(
        &mut world,
        1,
        "bob".into(),
        vec![dummy_char(5001, "P5001")],
        true,
    );
    assert!(matches!(
        world.clients.get(&1),
        Some(ClientSession::InLobby(_))
    ));
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::CHARACTER_SELECTION_INFO
    );
}

/// Logout: the player is stored + removed and the client gets `LeaveWorld`;
/// dropping the session is what closes the socket.
#[test]
fn logout_stores_player_and_sends_leave_world() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut out_rx = ingame_player(&mut world, 1, 5002, 100, 200, 0);

    handle_logout(&mut world, 1);

    assert_eq!(expect_store_player(&mut db_rx).base.object_id, 5002);
    assert_eq!(world.objects.count::<Player>(), 0);
    assert!(world.clients.is_empty(), "session dropped → socket closes");
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::LOG_OUT_OK
    );
}

/// An unexpected disconnect while in game persists the player too (Java
/// `GameClient.onDisconnection` → `Disconnection.storeMe().deleteMe()`).
#[test]
fn disconnect_stores_ingame_player() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let _out_rx = ingame_player(&mut world, 1, 5003, 100, 200, 0);

    on_disconnect(&mut world, 1);

    assert_eq!(expect_store_player(&mut db_rx).base.object_id, 5003);
    assert_eq!(world.objects.count::<Player>(), 0);
    assert!(world.clients.is_empty());
}

/// The `Buy <listId>` bypass opens the buy window: the BUY tab (type 0,
/// list id + adena + both products) and the SELL tab (type 1).
#[test]
fn buy_bypass_opens_buy_and_sell_tabs() {
    let (mut world, _db_rx, mut rx) = shop_world();
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Buy 3")));
    let pkts = drain(&mut rx);
    let tabs: Vec<_> = pkts
        .iter()
        .filter(|p| is_ex(p, crate::network::trade::EX_BUY_SELL_LIST))
        .collect();
    assert_eq!(tabs.len(), 2, "buy + sell tab");
    // BUY tab: type 0, money 1000, list id 3, then the product table.
    let buy = tabs[0];
    assert_eq!(i32::from_le_bytes(buy[3..7].try_into().unwrap()), 0);
    assert_eq!(i64::from_le_bytes(buy[7..15].try_into().unwrap()), 1000);
    assert_eq!(i32::from_le_bytes(buy[15..19].try_into().unwrap()), 3);
    // SELL tab leads with type 1.
    assert_eq!(i32::from_le_bytes(tabs[1][3..7].try_into().unwrap()), 1);

    // A non-merchant NPC refuses the same bypass.
    add_test_npc(&mut world, NPC_OID + 1, 30002, "Folk", 5, 120, 0, 0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Buy 3", NPC_OID + 1)),
    );
    let pkts = drain(&mut rx);
    assert!(
        !pkts
            .iter()
            .any(|p| is_ex(p, crate::network::trade::EX_BUY_SELL_LIST))
    );
}

/// Using a soulshot with a matching-grade weapon charges the shot, consumes
/// `weapon.soulShotCount` from the stack, and plays the shot's `<skills>`
/// visual (`SoulShots.useItem`).
#[test]
fn soulshot_charges_consumes_and_plays_visual() {
    use crate::data::item_data::{CrystalType, ItemHandler};
    use crate::model::inventory::Inventory;
    use crate::model::{Player, ShotType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    shot_weapon(&mut world, 9500, CrystalType::D, 2, 2);
    world.data.item_data.insert_for_test(shot_template(
        1463,
        CrystalType::D,
        ItemHandler::SoulShots,
        2150,
    ));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    grant_and_equip(&mut world, 3001, 1, 9500);
    let shot_oid = items::add_inventory_item(&mut world, 3001, 1463, 10).unwrap()[0];
    drain(&mut a_rx);

    items::use_equipable_item(&mut world, 1, 3001, shot_oid);

    assert!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_charged_shot(ShotType::Soulshots),
        "soulshot charged"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(1463),
        8,
        "weapon.soulShotCount (2) consumed"
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE),
        "enable message sent"
    );
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE
                && i32::from_le_bytes(p[13..17].try_into().unwrap()) == 2150),
        "shot visual (skill 2150) broadcast"
    );
}

/// A soulshot whose grade doesn't match the equipped weapon is refused.
#[test]
fn soulshot_wrong_grade_is_refused() {
    use crate::data::item_data::{CrystalType, ItemHandler};
    use crate::model::{Player, ShotType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    shot_weapon(&mut world, 9500, CrystalType::D, 2, 2);
    // A C-grade soulshot on a D-grade weapon.
    world.data.item_data.insert_for_test(shot_template(
        1464,
        CrystalType::C,
        ItemHandler::SoulShots,
        2151,
    ));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    grant_and_equip(&mut world, 3001, 1, 9500);
    let shot_oid = items::add_inventory_item(&mut world, 3001, 1464, 10).unwrap()[0];
    drain(&mut a_rx);

    items::use_equipable_item(&mut world, 1, 3001, shot_oid);

    assert!(
        !world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_charged_shot(ShotType::Soulshots),
        "wrong-grade shot not charged"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(1464),
        10,
        "nothing consumed"
    );
}

/// A charged soulshot is spent on the next non-miss melee swing, doubles its
/// damage, and sets the `SHOT_USED` flag (`generateHit`).
#[test]
fn soulshot_consumed_on_hit_doubles_melee_damage() {
    use crate::model::{Player, ShotType};

    fn attack_damage_and_flags(packets: &[Vec<u8>]) -> (i32, i32) {
        let atk = packets
            .iter()
            .find(|p| p[0] == server_packets::opcodes::ATTACK)
            .expect("Attack broadcast");
        (
            i32::from_le_bytes(atk[13..17].try_into().unwrap()),
            i32::from_le_bytes(atk[17..21].try_into().unwrap()),
        )
    }

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = model::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // A non-miss swing consumes five rolls in order: miss(1000), shield-rate(100),
    // shield-perfect(100), crit(100), random-damage(2r+1). Force them so both
    // swings are identical: hit, no shield (the NPC has none anyway), no crit,
    // and rand roll 10 → `rand_roll = 10 - 10 = 0` → random multiplier 1.0.
    const SWING_ROLLS: [i32; 5] = [0, 0, 0, 99, 10];

    // Control swing (no shot): plain hit, no crit.
    world.force_rolls(SWING_ROLLS);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    let (base_dmg, base_flags) = attack_damage_and_flags(&drain(&mut a_rx));
    assert_eq!(base_flags & 0x08, 0, "no soulshot flag without a charge");

    // Charged swing: identical rolls → exactly double, flag set, shot spent.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .charge_shot(ShotType::Soulshots);
    world.force_rolls(SWING_ROLLS);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    let (ss_dmg, ss_flags) = attack_damage_and_flags(&drain(&mut a_rx));

    assert_eq!(ss_dmg, base_dmg * 2, "soulshot doubles the swing");
    assert_ne!(ss_flags & 0x08, 0, "SHOT_USED flag set");
    assert!(
        !world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_charged_shot(ShotType::Soulshots),
        "shot consumed"
    );
}

/// A charged spiritshot doubles a magic attack's damage and is spent
/// (`calcMagicDam` `sps` bonus + `Skill` uncharge).
#[test]
fn spiritshot_doubles_magic_damage_and_is_consumed() {
    use crate::model::components::Vitals;
    use crate::model::{Player, ShotType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = model::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    let skill = world
        .data
        .skill_data
        .get(1177, 1)
        .expect("Wind Strike")
        .clone();
    assert_eq!(skill.magic_type, 1, "test skill must be magic");
    drain(&mut a_rx);

    let start_hp = nvit(&world, npc_oid).cur_hp;
    // Control cast (no shot), non-crit. The trailing 0 pins the `MagicFailures`
    // success roll — unforced it resists ~3 % of the time against this mob, and
    // the halved damage reads as "the shot did nothing".
    world.force_rolls([999_999, 0]);
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let base = start_hp - nvit(&world, npc_oid).cur_hp;
    assert!(base > 0.0, "control nuke dealt damage");
    world
        .objects
        .get_component_mut::<Vitals>(&npc_oid)
        .unwrap()
        .cur_hp = start_hp;

    // Charged spiritshot cast, identical crit + success rolls.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .charge_shot(ShotType::Spiritshots);
    world.force_rolls([999_999, 0]);
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let ss = start_hp - nvit(&world, npc_oid).cur_hp;

    assert!(
        (ss - base * 2.0).abs() < 1e-6,
        "spiritshot doubles magic damage ({ss} vs {base})"
    );
    assert!(
        !world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_charged_shot(ShotType::Spiritshots),
        "spiritshot consumed"
    );
}

/// A `PhysicalAttack` skill (Power Strike 3) deals damage end-to-end — the
/// regression guard for the whole family of physical skills that used to cast
/// but no-op — and a charged soulshot doubles it and is spent.
#[test]
fn physical_skill_damages_monster_and_soulshot_doubles() {
    use crate::model::components::{CombatStats, Vitals};
    use crate::model::{Player, ShotType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 13;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = model::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    let skill = world
        .data
        .skill_data
        .get(3, 1)
        .expect("Power Strike")
        .clone();
    assert_eq!(skill.magic_type, 0, "test skill must be physical");
    // Zero the weapon random-damage spread so only the crit roll is consumed
    // and the damage is deterministic.
    world
        .objects
        .get_component_mut::<CombatStats>(&3001)
        .unwrap()
        .random_dmg = 0;
    drain(&mut a_rx);

    let start_hp = nvit(&world, npc_oid).cur_hp;
    // **Four** forced high rolls per cast, in the order the path draws them:
    // the unconditional top-of-cast magic-crit roll (unused for a physical
    // skill), the two `calcShldUse` rolls, then the physical-skill crit roll.
    // All fail, so damage is the non-crit, unblocked base.
    //
    // The shield pair arrived with the G20 shield slice and silently shifted
    // this queue: with only two values forced, the crit roll fell through to
    // the real RNG and the two casts could disagree — the test then failed
    // about two full-suite runs in three while still passing in isolation.
    // Control cast (no shot).
    world.force_rolls([999_999, 999_999, 999_999, 999_999]);
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let base = start_hp - nvit(&world, npc_oid).cur_hp;
    assert!(
        base > 0.0,
        "physical skill dealt damage (was a silent no-op before)"
    );
    world
        .objects
        .get_component_mut::<Vitals>(&npc_oid)
        .unwrap()
        .cur_hp = start_hp;

    // Charged soulshot cast, identical (failed) crit rolls.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .charge_shot(ShotType::Soulshots);
    world.force_rolls([999_999, 999_999, 999_999, 999_999]);
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let ss = start_hp - nvit(&world, npc_oid).cur_hp;

    assert!(
        (ss - base * 2.0).abs() < 1e-6,
        "soulshot doubles physical skill damage ({ss} vs {base})"
    );
    assert!(
        !world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_charged_shot(ShotType::Soulshots),
        "soulshot consumed"
    );
}

/// Toggling auto-use (`RequestAutoSoulShot`) with a matching weapon activates
/// the shot: `ExAutoSoulShot` ack, the auto-set records the item, and it's
/// charged immediately; a following attack keeps it topped up.
#[test]
fn auto_soulshot_toggle_activates_and_recharges() {
    use crate::data::item_data::{CrystalType, ItemHandler};
    use crate::model::{Player, ShotType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    shot_weapon(&mut world, 9500, CrystalType::D, 1, 1);
    world.data.item_data.insert_for_test(shot_template(
        1463,
        CrystalType::D,
        ItemHandler::SoulShots,
        2150,
    ));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    grant_and_equip(&mut world, 3001, 1, 9500);
    items::add_inventory_item(&mut world, 3001, 1463, 10);
    drain(&mut a_rx);

    // itemId=1463, enable=1, type=0.
    let mut body = Vec::new();
    body.extend_from_slice(&1463i32.to_le_bytes());
    body.extend_from_slice(&1i32.to_le_bytes());
    body.extend_from_slice(&0i32.to_le_bytes());
    items::handle_request_auto_soul_shot(&mut world, 1, &body);

    assert!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .auto_shots
            .contains(&1463),
        "item recorded for auto-use"
    );
    assert!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_charged_shot(ShotType::Soulshots),
        "charged on activation"
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::EX
            && i16::from_le_bytes(p[1..3].try_into().unwrap())
                == server_packets::opcodes::EX_AUTO_SOUL_SHOT),
        "ExAutoSoulShot ack sent"
    );

    // The charge is spent on a hit, and the next attack auto-recharges it.
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = model::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // Swing 1 spends the activation charge (no item, just the flag).
    world.force_rolls([0, 99, 10]);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    drain(&mut a_rx);
    // Swing 2 finds no charge, auto-recharges (spends an item), then spends it:
    // the `SHOT_USED` flag on this swing proves the recharge fed it.
    world.force_rolls([0, 99, 10]);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    let atk = drain(&mut a_rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::ATTACK)
        .expect("Attack");
    assert_ne!(
        i32::from_le_bytes(atk[17..21].try_into().unwrap()) & 0x08,
        0,
        "auto-shot re-charged and was spent on the 2nd swing"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(1463),
        8,
        "activation + one auto-recharge consumed two shots"
    );
}

/// `RequestDestroyItem` (0x60) removes `count` of a stackable item and sends an
/// `InventoryUpdate`; a bad object id is a no-op.
#[test]
fn destroy_item_removes_from_inventory() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9100, 0);
    drain(&mut rx);

    items::add_inventory_item(&mut world, 9100, 57, 1000).expect("adena added");
    let inv = |w: &World| {
        w.objects
            .get_component::<Inventory>(&9100)
            .unwrap()
            .count_of(57)
    };
    let adena_oid = item_oid(&world, 9100, 57);

    let destroy = |oid: i32, count: i64| -> Vec<u8> {
        let mut w = PacketWriter::new();
        w.write_u8(cop::REQUEST_DESTROY_ITEM);
        w.write_i32(oid);
        w.write_i64(count);
        w.into_bytes()
    };

    on_packet(&mut world, 1, destroy(adena_oid, 400));
    assert_eq!(inv(&world), 600, "400 adena destroyed");
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == 0x21),
        "InventoryUpdate sent"
    );

    // A bogus object id changes nothing.
    on_packet(&mut world, 1, destroy(0x7fff_ffff, 1));
    assert_eq!(inv(&world), 600, "unchanged");

    // Destroy the rest.
    on_packet(&mut world, 1, destroy(adena_oid, 600));
    assert_eq!(inv(&world), 0, "all adena gone");
}

/// Giving adena (the item-creation menu's "Create Coin", quest rewards) sends
/// the adena counter (`ExAdenaInvenCount` 0x13E) and weight bar
/// (`ExUserInfoInvenWeight` 0x166) alongside the `InventoryUpdate`, matching
/// Java `Player.sendInventoryUpdate` — so the status-bar adena refreshes. The
/// bare-InventoryUpdate path left it stale.
#[test]
fn giving_adena_refreshes_the_adena_counter() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9100, 0);
    drain(&mut rx);

    crate::game_loop::quests::give_item_with_earned_message(&mut world, 1, 9100, 57, 100_000);

    let pkts = drain(&mut rx);
    assert!(
        pkts.iter().any(|p| is_ex(p, 0x13E)),
        "ExAdenaInvenCount (status-bar adena) sent"
    );
    assert!(
        pkts.iter().any(|p| is_ex(p, 0x166)),
        "ExUserInfoInvenWeight sent"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9100)
            .unwrap()
            .adena(),
        100_000,
        "adena actually added"
    );
}

/// Drop → the item leaves the inventory and becomes a `GroundItem` world entity
/// (DropItem broadcast); a click (`Action`) picks it back up.
#[test]
fn drop_and_pickup_ground_item() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9200, 0);
    drain(&mut rx);
    items::add_inventory_item(&mut world, 9200, 57, 1000).expect("adena");
    let count_of = |w: &World| {
        w.objects
            .get_component::<Inventory>(&9200)
            .unwrap()
            .count_of(57)
    };
    let adena_oid = item_oid(&world, 9200, 57);

    // Drop 400 adena.
    let item_oid = world.next_npc_object_id;
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_DROP_ITEM);
    w.write_i32(adena_oid);
    w.write_i64(400);
    w.write_i32(DROP_AT.0);
    w.write_i32(DROP_AT.1);
    w.write_i32(DROP_AT.2);
    on_packet(&mut world, 1, w.into_bytes());

    assert_eq!(count_of(&world), 600, "400 left the inventory");
    let g = world
        .objects
        .get_component::<model::components::GroundItem>(&item_oid)
        .expect("ground item spawned");
    assert_eq!((g.item_id, g.count), (57, 400));
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::DROP_ITEM),
        "DropItem broadcast"
    );

    // Click the ground item to pick it up (Action: objectId + origin xyz + action).
    let mut a = PacketWriter::new();
    a.write_u8(cop::ACTION);
    a.write_i32(item_oid);
    a.write_i32(0);
    a.write_i32(0);
    a.write_i32(0);
    a.write_u8(0);
    on_packet(&mut world, 1, a.into_bytes());
    // The stack landed where the drop asked (~90 units out), not underfoot, so
    // the click only starts the approach — `thinkPickUp` lifts it on arrival.
    advance_world(&mut world, 300);

    assert_eq!(count_of(&world), 1000, "adena back in the inventory");
    assert!(
        !world
            .objects
            .has_component::<model::components::GroundItem>(&item_oid),
        "ground item removed"
    );
    assert!(
        !world
            .ground_item_regions
            .values()
            .flatten()
            .any(|&id| id == item_oid),
        "ground item de-indexed"
    );
}

/// An enchanted item keeps its `+N` across drop → pickup.
///
/// Java gets this for free: both sides move the same `Item` instance between
/// containers. This port mints a fresh instance on the give path, so the level
/// has to be carried across explicitly — and until it was, dropping a `+7`
/// weapon and picking it straight back up silently returned it at `+0`.
///
/// The assertion is on the *enchant of the instance in the bag*, not merely on
/// the item being back: the old behaviour returned the item too.
#[test]
fn a_dropped_item_keeps_its_enchant_when_picked_back_up() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9200, 0);
    drain(&mut rx);

    // Short Sword — non-stackable (a distinct instance) and genuinely
    // dropable. The starter Squire's Sword is `is_dropable="false"`, so a drop
    // of it is correctly refused and would make this test vacuous.
    const SWORD: i32 = 1;
    items::add_inventory_item(&mut world, 9200, SWORD, 1).expect("sword");
    let sword_oid = item_oid(&world, 9200, SWORD);
    world
        .objects
        .get_component_mut::<Inventory>(&9200)
        .unwrap()
        .set_enchant_level(sword_oid, 7);

    let ground_oid = world.next_npc_object_id;
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_DROP_ITEM);
    w.write_i32(sword_oid);
    w.write_i64(1);
    w.write_i32(DROP_AT.0);
    w.write_i32(DROP_AT.1);
    w.write_i32(DROP_AT.2);
    on_packet(&mut world, 1, w.into_bytes());

    assert_eq!(
        world
            .objects
            .get_component::<model::components::GroundItem>(&ground_oid)
            .expect("ground item spawned")
            .enchant,
        7,
        "the drop side records the enchant"
    );

    let mut a = PacketWriter::new();
    a.write_u8(cop::ACTION);
    a.write_i32(ground_oid);
    a.write_i32(0);
    a.write_i32(0);
    a.write_i32(0);
    a.write_u8(0);
    on_packet(&mut world, 1, a.into_bytes());
    advance_world(&mut world, 300);

    let picked_enchant = inv_item(&world, 9200, SWORD)
        .expect("sword back in the bag")
        .enchant_level;
    assert_eq!(
        picked_enchant, 7,
        "and the pickup restores it — not a fresh +0 instance"
    );
}

/// Where the drop tests aim: within `RequestDropItem`'s 150/50 box of the
/// dummy character's `(1, 2, 3)`, the way a real client's cursor position is.
const DROP_AT: (i32, i32, i32) = (61, 72, 13);

/// Give `count` adena to `player_oid` and drop it via `RequestDropItem` at a
/// fixed spot; returns the resulting ground-item object id.
fn drop_adena(world: &mut World, client_id: u32, player_oid: i32, count: i64) -> i32 {
    items::add_inventory_item(world, player_oid, 57, count).expect("adena");
    let adena_oid = item_oid(world, player_oid, 57);
    let item_oid = world.next_npc_object_id;
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_DROP_ITEM);
    w.write_i32(adena_oid);
    w.write_i64(count);
    w.write_i32(DROP_AT.0);
    w.write_i32(DROP_AT.1);
    w.write_i32(DROP_AT.2);
    on_packet(world, client_id, w.into_bytes());
    item_oid
}

/// Build a `RequestDropItem` body for `item_oid` at an explicit location.
fn drop_item_packet(item_oid: i32, count: i64, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_DROP_ITEM);
    w.write_i32(item_oid);
    w.write_i64(count);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.into_bytes()
}

/// Give the player adena and return its inventory object id.
fn give_adena(world: &mut World, player_oid: i32, count: i64) -> i32 {
    items::add_inventory_item(world, player_oid, 57, count).expect("adena");
    item_oid(world, player_oid, 57)
}

/// The dropped stack lands **where the client asked**, not at the player's
/// feet: Java reads `_x/_y/_z` off the packet and hands them to
/// `Player.dropItem` → `Item.dropMe`. Dropping at the character's own position
/// is what the port used to do, and it made every discarded stack pile up
/// under the character instead of scattering where it was dragged.
#[test]
fn dropped_item_lands_at_the_requested_location() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let adena_oid = give_adena(&mut world, 9300, 100);
    let ground_oid = world.next_npc_object_id;

    on_packet(
        &mut world,
        1,
        drop_item_packet(adena_oid, 100, DROP_AT.0, DROP_AT.1, DROP_AT.2),
    );

    let pos = world
        .objects
        .get_component::<Position>(&ground_oid)
        .expect("the stack reached the ground");
    assert_eq!(
        (pos.x, pos.y, pos.z),
        DROP_AT,
        "the ground item sits at the requested drop point"
    );
    let player_pos = *world.objects.get_component::<Position>(&9300).unwrap();
    assert_ne!(
        (pos.x, pos.y),
        (player_pos.x, player_pos.y),
        "sanity: the requested point is not the player's own position"
    );
}

/// `!player.isInsideRadius2D(_x, _y, 0, 150)` — a drop aimed further than 150
/// units away is refused with SM 151 and the stack stays in the inventory.
/// Without this a client could post items across the map from where it stands.
#[test]
fn drop_beyond_150_units_is_refused() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let adena_oid = give_adena(&mut world, 9300, 100);
    let ground_oid = world.next_npc_object_id;

    // (1, 2, 3) → (401, 2, 3) is 400 units out.
    on_packet(&mut world, 1, drop_item_packet(adena_oid, 100, 401, 2, 3));

    assert!(
        !world
            .objects
            .has_component::<model::components::GroundItem>(&ground_oid),
        "nothing reached the ground"
    );
    assert_eq!(
        item_count(&world, 9300, 57),
        100,
        "the adena stays in the inventory"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_CANNOT_DISCARD_SOMETHING_THAT_FAR_AWAY_FROM_YOU),
        "the client is told the spot is too far away"
    );
}

/// The same guard's second half: `Math.abs(_z - player.getZ()) > 50`. The 2D
/// distance is fine here — only the height differs — so a port that checked
/// distance in 3D, or skipped z entirely, would let this through.
#[test]
fn drop_more_than_50_units_below_is_refused() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let adena_oid = give_adena(&mut world, 9300, 100);
    let ground_oid = world.next_npc_object_id;

    on_packet(
        &mut world,
        1,
        drop_item_packet(adena_oid, 100, 11, 12, -300),
    );

    assert!(
        !world
            .objects
            .has_component::<model::components::GroundItem>(&ground_oid),
        "nothing reached the ground"
    );
    assert_eq!(item_count(&world, 9300, 57), 100, "adena kept");
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_CANNOT_DISCARD_SOMETHING_THAT_FAR_AWAY_FROM_YOU),
        "the client is told the spot is too far away"
    );
    // …while the same request at the player's own height is accepted, so the
    // refusal is the z test and not something else in the chain.
    let ground_oid = world.next_npc_object_id;
    on_packet(&mut world, 1, drop_item_packet(adena_oid, 100, 11, 12, 13));
    assert!(
        world
            .objects
            .has_component::<model::components::GroundItem>(&ground_oid),
        "the in-range drop goes through"
    );
}

/// `player.isInsideZone(ZoneId.NO_ITEM_DROP)` — inside a `ConditionZone` that
/// declares `NoItemDrop` (`no_drop_item.xml`: the bascule bridge, the
/// Underground Coliseum floors) nothing may be discarded at all.
#[test]
fn drop_inside_a_no_item_drop_zone_is_refused() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let adena_oid = give_adena(&mut world, 9300, 100);
    world.data.zone_data.insert(crate::data::zone_data::Zone {
        id: 0,
        name: "test_no_drop".into(),
        kind: crate::data::zone_data::ZoneKind::Condition,
        territory: Territory {
            form: crate::data::spawn_data::ZoneForm::Cuboid {
                x1: -1000,
                x2: 1000,
                y1: -1000,
                y2: 1000,
            },
            min_z: -1000,
            max_z: 1000,
        },
        castle_id: 0,
        clan_hall_id: 0,
        effect: None,
        damage: None,
        swamp: None,
        condition: Some(crate::data::zone_data::ConditionZoneParams {
            no_item_drop: true,
            no_bookmark: false,
        }),
        mother_tree: None,
    });
    let ground_oid = world.next_npc_object_id;

    on_packet(
        &mut world,
        1,
        drop_item_packet(adena_oid, 100, DROP_AT.0, DROP_AT.1, DROP_AT.2),
    );

    assert!(
        !world
            .objects
            .has_component::<model::components::GroundItem>(&ground_oid),
        "nothing reached the ground inside the zone"
    );
    assert_eq!(item_count(&world, 9300, 57), 100, "adena kept");
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THAT_ITEM_CANNOT_BE_DISCARDED),
        "the client is told the item cannot be discarded"
    );
}

/// "Do not drop items when casting known skills to avoid exploits." Java
/// refuses mid-cast with `"You cannot drop an item while casting " +
/// skill.getName() + "."` — the **named** skill, so the player can tell which
/// cast is holding their inventory. `SkillData` now keeps `<skill name="…">`
/// per id to say it.
#[test]
fn drop_while_casting_a_known_skill_is_refused_by_name() {
    const WIND_STRIKE: i32 = 1177;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    world
        .data
        .skill_data
        .insert_name_for_test(WIND_STRIKE, "Wind Strike");
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let adena_oid = give_adena(&mut world, 9300, 100);
    // The character knows the skill *and* is casting it.
    world
        .objects
        .get_component_mut::<SkillBook>(&9300)
        .unwrap()
        .0
        .insert(WIND_STRIKE, 1);
    world.objects.add_components(
        &9300,
        Casting(model::CastState {
            skill_id: WIND_STRIKE,
            skill_level: 1,
            skill_sub_level: 0,
            target_object_id: 0,
            seq: 0,
            launched: false,
            cancel_ms: 0,
            cool_ms: 0,
            trigger_item_object_id: 0,
        }),
    );
    let ground_oid = world.next_npc_object_id;

    on_packet(
        &mut world,
        1,
        drop_item_packet(adena_oid, 100, DROP_AT.0, DROP_AT.1, DROP_AT.2),
    );

    assert!(
        !world
            .objects
            .has_component::<model::components::GroundItem>(&ground_oid),
        "nothing reached the ground mid-cast"
    );
    assert_eq!(item_count(&world, 9300, 57), 100, "adena kept");
    let pkts = drain(&mut rx);
    let needle: Vec<u8> = "You cannot drop an item while casting Wind Strike."
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert!(
        pkts.iter()
            .any(|p| p.windows(needle.len()).any(|w| w == needle)),
        "the refusal names the skill being cast"
    );
}

/// `_count > item.getCount()` refuses outright (Java sends
/// `THAT_ITEM_CANNOT_BE_DISCARDED`) rather than clamping — a forged count must
/// not walk away with the whole stack under a partial-drop request.
#[test]
fn drop_of_more_than_is_held_is_refused() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let adena_oid = give_adena(&mut world, 9300, 100);
    let ground_oid = world.next_npc_object_id;

    on_packet(
        &mut world,
        1,
        drop_item_packet(adena_oid, 500, DROP_AT.0, DROP_AT.1, DROP_AT.2),
    );

    assert!(
        !world
            .objects
            .has_component::<model::components::GroundItem>(&ground_oid),
        "nothing reached the ground"
    );
    assert_eq!(
        item_count(&world, 9300, 57),
        100,
        "the whole stack is still held"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THAT_ITEM_CANNOT_BE_DISCARDED),
        "the client is told the item cannot be discarded"
    );
}

/// Datapack parity: *Mage Class Equipment Set (10-day)* (15195) declares
/// `is_dropable="false"`, so `RequestDropItem` must refuse it with
/// `THAT_ITEM_CANNOT_BE_DISCARDED` and leave it in the inventory — Java's
/// first guard in `RequestDropItem.runImpl`.
#[test]
fn bound_item_cannot_be_discarded() {
    const BOUND_BOX: i32 = 15195;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);

    items::add_inventory_item(&mut world, 9300, BOUND_BOX, 1).expect("bound box");
    let box_oid = item_oid(&world, 9300, BOUND_BOX);
    let would_be_ground_oid = world.next_npc_object_id;

    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_DROP_ITEM);
    w.write_i32(box_oid);
    w.write_i64(1);
    w.write_i32(DROP_AT.0);
    w.write_i32(DROP_AT.1);
    w.write_i32(DROP_AT.2);
    on_packet(&mut world, 1, w.into_bytes());

    assert!(
        world
            .objects
            .get_component::<Inventory>(&9300)
            .unwrap()
            .items()
            .iter()
            .any(|it| it.item_id == BOUND_BOX),
        "the bound box stays in the inventory"
    );
    assert!(
        !world
            .objects
            .has_component::<model::components::GroundItem>(&would_be_ground_oid),
        "nothing reached the ground"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THAT_ITEM_CANNOT_BE_DISCARDED),
        "the client is told the item cannot be discarded"
    );
}

/// Java never lifts a ground item on the click itself: `ItemAction` only sets
/// `AI_INTENTION_PICK_UP`, `CreatureAI.onIntentionPickUp` fires `moveToPawn`,
/// and `Player.doPickupItem` runs later from `PlayerAI.thinkPickUp` — once
/// `maybeMoveToPawn(target, 36)` reports the walk has arrived. So clicking loot
/// across the field must walk the character over, not teleport the item into
/// the bag.
#[test]
fn distant_ground_item_is_walked_to_before_pickup() {
    use crate::game_loop::ground_items::{DropSource, spawn_ground_item};
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9400, 0);
    let start = *world.objects.get_component::<Position>(&9400).unwrap();
    drain(&mut rx);

    // 500 units away — far outside `maybeMoveToPawn`'s 36 + collision radius.
    let item_oid = spawn_ground_item(
        &mut world,
        57,
        400,
        0,
        start.x + 500,
        start.y,
        start.z,
        0,
        DropSource::Npc,
    );
    let held = |w: &World| {
        w.objects
            .get_component::<Inventory>(&9400)
            .unwrap()
            .count_of(57)
    };
    assert_eq!(held(&world), 0, "sanity: no adena to start with");
    drain(&mut rx);

    // The click starts the approach and nothing else.
    handle_action(&mut world, 1, &action_body(item_oid, 0));
    assert_eq!(held(&world), 0, "the click alone must not pick it up");
    assert!(
        world
            .objects
            .has_component::<model::components::GroundItem>(&item_oid),
        "the item is still on the ground"
    );
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&9400).copied(),
            Some(Intent(crate::model::PlayerIntent::PickUp { item_object_id })) if item_object_id == item_oid
        ),
        "AI_INTENTION_PICK_UP is set"
    );
    assert!(
        world.objects.has_component::<Movement>(&9400),
        "and the character is walking to it (moveToPawn)"
    );
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "MoveToPawn broadcast"
    );

    // Walk it out: `thinkPickUp` lifts the item on the tick it arrives.
    advance_world(&mut world, 300);
    assert_eq!(held(&world), 400, "picked up on arrival");
    assert!(
        !world
            .objects
            .has_component::<model::components::GroundItem>(&item_oid),
        "ground item removed"
    );
    assert!(
        !world.objects.has_component::<Intent>(&9400),
        "thinkPickUp's setIntention(AI_INTENTION_IDLE) clears the intention"
    );
    let end = *world.objects.get_component::<Position>(&9400).unwrap();
    assert!(
        ((end.x - start.x) as f64).hypot((end.y - start.y) as f64) > 400.0,
        "the character actually travelled to the loot"
    );
}

/// `CreatureAI.onIntentionPickUp`'s REST branch: a seated player's click on
/// loot is refused outright with a bare `ActionFailed` — no walk is started
/// and the item stays put.
#[test]
fn seated_player_cannot_pick_up() {
    use crate::game_loop::ground_items::{DropSource, spawn_ground_item};
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9401, 0);
    let start = *world.objects.get_component::<Position>(&9401).unwrap();
    world
        .objects
        .get_component_mut::<Player>(&9401)
        .unwrap()
        .sitting = true;
    // At the player's feet, so only the REST gate can refuse it.
    let item_oid = spawn_ground_item(
        &mut world,
        57,
        400,
        0,
        start.x,
        start.y,
        start.z,
        0,
        DropSource::Npc,
    );
    drain(&mut rx);

    handle_action(&mut world, 1, &action_body(item_oid, 0));
    assert!(
        world
            .objects
            .has_component::<model::components::GroundItem>(&item_oid),
        "loot stays on the floor while seated"
    );
    assert!(
        !world.objects.has_component::<Intent>(&9401),
        "no pick-up intention is started"
    );
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ACTION_FAIL),
        "clientActionFailed"
    );
}

/// A ground item left un-picked-up auto-destroys after its lifetime
/// (`ItemsOnGroundManager` cleanup) — when General.ini enables it.
#[test]
fn ground_item_decays_after_lifetime() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    // Enable player-drop auto-destroy (General.ini `AutoDestroyDroppedItemAfter`
    // + `DestroyPlayerDroppedItem`); the dist default keeps player drops.
    world.cfg.general.autodestroy_item_after = 600;
    world.cfg.general.destroy_dropped_player_item = true;
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let item_oid = drop_adena(&mut world, 1, 9300, 100);
    assert!(
        world
            .objects
            .has_component::<model::components::GroundItem>(&item_oid),
        "dropped"
    );

    // Jump past the 600 s lifetime and fire the scheduled decay.
    world.tick += 600 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(
        !world
            .objects
            .has_component::<model::components::GroundItem>(&item_oid),
        "decayed"
    );
    assert!(
        !world
            .ground_item_regions
            .values()
            .flatten()
            .any(|&id| id == item_oid),
        "de-indexed"
    );
}

/// General.ini parity: with `DestroyPlayerDroppedItem = False` (the dist
/// value), a player's drop is **never** auto-destroyed even when
/// `AutoDestroyDroppedItemAfter > 0` — it persists until pickup/restart.
#[test]
fn player_ground_item_persists_when_destroy_player_dropped_off() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.cfg.general.autodestroy_item_after = 600;
    world.cfg.general.destroy_dropped_player_item = false; // dist default
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let item_oid = drop_adena(&mut world, 1, 9300, 100);

    world.tick += 600 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(
        world
            .objects
            .has_component::<model::components::GroundItem>(&item_oid),
        "player drop persists"
    );
}

/// An NPC drop auto-destroys whenever `AutoDestroyDroppedItemAfter > 0`,
/// independent of the player-drop flag (Java `Npc.dropItem`).
#[test]
fn npc_ground_item_decays_regardless_of_player_flag() {
    use crate::game_loop::ground_items::{DropSource, spawn_ground_item};
    let (mut world, ..) = admin_world();
    world.cfg.general.autodestroy_item_after = 600;
    world.cfg.general.destroy_dropped_player_item = false;
    world.id_pool = 0x4000_0000..0x4000_0100;
    let item_oid = spawn_ground_item(&mut world, 57, 100, 0, 100, 200, -3000, 0, DropSource::Npc);
    assert!(
        world
            .objects
            .has_component::<model::components::GroundItem>(&item_oid),
        "npc drop on ground"
    );

    world.tick += 600 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(
        !world
            .objects
            .has_component::<model::components::GroundItem>(&item_oid),
        "npc drop decays"
    );
}

/// Warehouse deposit → the item moves inventory→warehouse; withdraw moves it
/// back; and the save gathers both containers with the right `loc`s (so a
/// deposit survives relog).
#[test]
fn warehouse_deposit_withdraw_and_persist() {
    use crate::model::inventory::{Inventory, Warehouse};
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9400, 0);
    drain(&mut rx);
    items::add_inventory_item(&mut world, 9400, 57, 1000).expect("adena");
    let inv_adena = |w: &World| {
        w.objects
            .get_component::<Inventory>(&9400)
            .unwrap()
            .count_of(57)
    };
    let wh_adena = |w: &World| {
        w.objects
            .get_component::<Warehouse>(&9400)
            .unwrap()
            .0
            .count_of(57)
    };
    let adena_oid = item_oid(&world, 9400, 57);

    // Deposit 400.
    let mut w = PacketWriter::new();
    w.write_u8(cop::SEND_WARE_HOUSE_DEPOSIT_LIST);
    w.write_i32(1);
    w.write_i32(adena_oid);
    w.write_i64(400);
    on_packet(&mut world, 1, w.into_bytes());
    assert_eq!(inv_adena(&world), 600, "400 left inventory");
    assert_eq!(wh_adena(&world), 400, "400 in warehouse");

    // The whole persisted set carries both containers with distinct locs.
    let save = build_save_data(&world, 9400).expect("save");
    let inv_row = save
        .items
        .iter()
        .find(|r| r.item_id == 57 && r.loc == "INVENTORY")
        .expect("inv adena row");
    let wh_row = save
        .items
        .iter()
        .find(|r| r.item_id == 57 && r.loc == "WAREHOUSE")
        .expect("wh adena row");
    assert_eq!((inv_row.count, wh_row.count), (600, 400));

    // Withdraw 150 back.
    let wh_oid = world
        .objects
        .get_component::<Warehouse>(&9400)
        .unwrap()
        .0
        .items()
        .iter()
        .find(|it| it.item_id == 57)
        .unwrap()
        .object_id;
    let mut w = PacketWriter::new();
    w.write_u8(cop::SEND_WARE_HOUSE_WITH_DRAW_LIST);
    w.write_i32(1);
    w.write_i32(wh_oid);
    w.write_i64(150);
    on_packet(&mut world, 1, w.into_bytes());
    assert_eq!(wh_adena(&world), 250, "150 withdrawn");
    assert_eq!(inv_adena(&world), 750, "back in inventory");
}

/// Crystallizing a D-grade item destroys it and yields its `crystal_count` of
/// Crystal (D-grade) (1458) — but only with the Crystallize skill.
#[test]
fn crystallize_item_yields_crystals_when_skilled() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    // Leather Boots (40): D-grade with a crystal count.
    let cc = world.data.item_data.get(40).unwrap().crystal_count;
    assert!(cc > 0, "test item is crystallizable");

    let mut rx = ingame_player_access(&mut world, 1, 9500, 0);
    drain(&mut rx);
    items::add_inventory_item(&mut world, 9500, 40, 1).expect("boots");
    let boots_oid = item_oid(&world, 9500, 40);
    let crystallize = |oid: i32| -> Vec<u8> {
        let mut w = PacketWriter::new();
        w.write_u8(cop::REQUEST_CRYSTALLIZE_ITEM);
        w.write_i32(oid);
        w.write_i64(1);
        w.into_bytes()
    };

    // No skill → refused, boots keep.
    on_packet(&mut world, 1, crystallize(boots_oid));
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9500)
            .unwrap()
            .count_of(40),
        1,
        "no skill, no crystallize"
    );

    // Grant Crystallize (248) level 1, then crystallize.
    world
        .objects
        .get_component_mut::<SkillBook>(&9500)
        .unwrap()
        .0
        .insert(248, 1);
    on_packet(&mut world, 1, crystallize(boots_oid));
    let inv = world.objects.get_component::<Inventory>(&9500).unwrap();
    assert_eq!(inv.count_of(40), 0, "boots crystallized away");
    assert_eq!(
        inv.count_of(1458),
        cc as i64,
        "got crystal_count Crystal (D-grade)"
    );
}

/// A private sell store: the owner sets a list (store activates + store byte),
/// and a buyer purchases — items move seller→buyer, adena buyer→seller.
#[test]
fn private_store_sell_and_buy() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut seller_rx = ingame_player_access(&mut world, 1, 9600, 0);
    let mut buyer_rx = ingame_player_access(&mut world, 2, 9601, 0);
    drain(&mut seller_rx);
    drain(&mut buyer_rx);
    // Seller has 10 Crystal (D); buyer has 1000 adena.
    items::add_inventory_item(&mut world, 9600, 1458, 10).unwrap();
    items::add_inventory_item(&mut world, 9601, 57, 1000).unwrap();
    let crystal_oid = item_oid(&world, 9600, 1458);

    // Seller sets the store: sell 4 crystals at 100 adena each.
    let mut w = PacketWriter::new();
    w.write_u8(cop::SET_PRIVATE_STORE_LIST_SELL);
    w.write_i32(0); // not package
    w.write_i32(1); // one line
    w.write_i32(crystal_oid);
    w.write_i64(4);
    w.write_i64(100);
    on_packet(&mut world, 1, w.into_bytes());
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9600)
            .unwrap()
            .store_type,
        1,
        "store active"
    );
    assert_eq!(
        world
            .objects
            .get_component::<model::components::PrivateStore>(&9600)
            .unwrap()
            .items
            .len(),
        1
    );

    // Buyer buys all 4.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PRIVATE_STORE_BUY);
    w.write_i32(9600); // seller
    w.write_i32(1);
    w.write_i32(crystal_oid);
    w.write_i64(4);
    w.write_i64(100);
    on_packet(&mut world, 2, w.into_bytes());

    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9601)
            .unwrap()
            .count_of(1458),
        4,
        "buyer got 4 crystals"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9601)
            .unwrap()
            .count_of(57),
        600,
        "buyer paid 400"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9600)
            .unwrap()
            .count_of(1458),
        6,
        "seller has 6 left"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9600)
            .unwrap()
            .count_of(57),
        400,
        "seller earned 400"
    );
    // Store emptied of its offered stock → closed.
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9600)
            .unwrap()
            .store_type,
        0,
        "store closed when sold out"
    );
}

/// A full player-to-player trade: request → accept → both add items → both
/// confirm → the offered items swap.
#[test]
fn player_trade_swaps_items() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut a_rx = ingame_player_access(&mut world, 1, 9700, 0);
    let mut b_rx = ingame_player_access(&mut world, 2, 9701, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    items::add_inventory_item(&mut world, 9700, 1458, 10).unwrap(); // A: Crystal D
    items::add_inventory_item(&mut world, 9701, 1459, 10).unwrap(); // B: Crystal C
    let a_oid = item_oid(&world, 9700, 1458);
    let b_oid = item_oid(&world, 9701, 1459);
    let one_int = |op: u8, v: i32| {
        let mut w = PacketWriter::new();
        w.write_u8(op);
        w.write_i32(v);
        w.into_bytes()
    };
    let add = |oid: i32, n: i64| {
        let mut w = PacketWriter::new();
        w.write_u8(cop::ADD_TRADE_ITEM);
        w.write_i32(0);
        w.write_i32(oid);
        w.write_i64(n);
        w.into_bytes()
    };

    // A requests, B accepts → both in a trade.
    on_packet(&mut world, 1, one_int(cop::TRADE_REQUEST, 9701));
    assert_eq!(
        world
            .objects
            .get_component::<model::components::PendingTrade>(&9701)
            .map(|p| p.from),
        Some(9700)
    );
    on_packet(&mut world, 2, one_int(cop::ANSWER_TRADE_REQUEST, 1));
    assert_eq!(
        world
            .objects
            .get_component::<model::components::Trade>(&9700)
            .unwrap()
            .partner,
        9701
    );

    // A offers 4 Crystal D, B offers 3 Crystal C.
    on_packet(&mut world, 1, add(a_oid, 4));
    on_packet(&mut world, 2, add(b_oid, 3));
    assert_eq!(
        world
            .objects
            .get_component::<model::components::Trade>(&9700)
            .unwrap()
            .items[0]
            .count,
        4
    );

    // Both confirm → swap.
    on_packet(&mut world, 1, one_int(cop::TRADE_DONE, 1));
    on_packet(&mut world, 2, one_int(cop::TRADE_DONE, 1));

    let a_inv = |w: &World, id: i32| {
        w.objects
            .get_component::<Inventory>(&9700)
            .unwrap()
            .count_of(id)
    };
    let b_inv = |w: &World, id: i32| {
        w.objects
            .get_component::<Inventory>(&9701)
            .unwrap()
            .count_of(id)
    };
    assert_eq!(
        (a_inv(&world, 1458), a_inv(&world, 1459)),
        (6, 3),
        "A: -4 D, +3 C"
    );
    assert_eq!(
        (b_inv(&world, 1458), b_inv(&world, 1459)),
        (4, 7),
        "B: +4 D, -3 C"
    );
    assert!(
        !world
            .objects
            .has_component::<model::components::Trade>(&9700),
        "trade closed"
    );
    assert!(
        !world
            .objects
            .has_component::<model::components::Trade>(&9701),
        "trade closed"
    );
}

/// Full enchant flow with real data: use scroll → add scroll → put target →
/// enchant. Success bumps +1; a forced failure at +4 destroys the weapon and
/// returns crystals.
#[test]
fn enchant_scroll_success_and_failure() {
    use crate::model::components::EnchantRequest;
    use crate::model::inventory::Inventory;
    const DIST: &str = crate::data::DIST_GAME;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.data.enchant = crate::data::EnchantData::load_from(DIST);
    world.id_pool = 0x4000_0000..0x4000_0200;

    // Scroll: Enchant Weapon (D-grade) 955; Bastard Sword 69 (D weapon, enchantable).
    let sword_cc = world.data.item_data.get(69).unwrap().crystal_count;
    let crystal_id = world
        .data
        .item_data
        .get(69)
        .unwrap()
        .crystal_type
        .crystal_item_id()
        .unwrap();

    let mut rx = ingame_player_access(&mut world, 1, 9800, 0);
    drain(&mut rx);
    items::add_inventory_item(&mut world, 9800, 955, 5).unwrap();
    items::add_inventory_item(&mut world, 9800, 69, 1).unwrap();
    let scroll_oid = item_oid(&world, 9800, 955);
    let sword_oid = item_oid(&world, 9800, 69);

    // Use the scroll → opens the enchant request.
    let use_item = {
        let mut w = PacketWriter::new();
        w.write_u8(cop::USE_ITEM);
        w.write_i32(scroll_oid);
        w.write_i32(0);
        w.into_bytes()
    };
    on_packet(&mut world, 1, use_item);
    assert!(
        world.objects.has_component::<EnchantRequest>(&9800),
        "enchant window opened"
    );

    let add_scroll = {
        let mut w = PacketWriter::new();
        w.write_i32(scroll_oid);
        w.write_i32(sword_oid);
        w.into_bytes()
    };
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM,
            &add_scroll,
        ),
    );
    let put_target = {
        let mut w = PacketWriter::new();
        w.write_i32(sword_oid);
        w.into_bytes()
    };
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM,
            &put_target,
        ),
    );

    // +0 weapon is a guaranteed (100%) success → +1.
    let do_enchant = |oid: i32| {
        let mut w = PacketWriter::new();
        w.write_u8(cop::REQUEST_ENCHANT_ITEM);
        w.write_i32(oid);
        w.write_i32(0);
        w.into_bytes()
    };
    // Java's anti-autoenchant guard punishes an Enchant pressed within 2 s of
    // the last window interaction, so the window has to age before the press.
    world.tick += 20;
    world.force_roll(0); // roll_f64 = 0.0 < 100
    on_packet(&mut world, 1, do_enchant(sword_oid));
    let level = |w: &World| {
        w.objects
            .get_component::<Inventory>(&9800)
            .unwrap()
            .by_object_id(sword_oid)
            .map(|it| it.enchant_level)
    };
    assert_eq!(level(&world), Some(1), "success: +0 → +1");
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9800)
            .unwrap()
            .count_of(955),
        4,
        "one scroll consumed"
    );

    // Bump to +4 (66.67% group chance), then force a failing roll (90%) →
    // weapon destroyed, crystals returned.
    world
        .objects
        .get_component_mut::<Inventory>(&9800)
        .unwrap()
        .set_item_enchant(sword_oid, 4);
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM,
            &add_scroll,
        ),
    );
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM,
            &put_target,
        ),
    );
    world.tick += 20;
    world.force_roll(900_000); // roll_f64 = 90.0 > 66.67 → fail
    on_packet(&mut world, 1, do_enchant(sword_oid));
    let inv = world.objects.get_component::<Inventory>(&9800).unwrap();
    assert_eq!(inv.count_of(69), 0, "failed enchant destroyed the sword");
    let expected_crystals = (sword_cc - (sword_cc + 1) / 2).max(0) as i64;
    assert_eq!(
        inv.count_of(crystal_id),
        expected_crystals,
        "crystals returned on break"
    );
    assert_eq!(inv.count_of(955), 3, "second scroll consumed");
}

/// Enchant with a support item: its +20 bonus rate flips a roll that would miss
/// the bare 66.67% group chance at +3, and the support is consumed.
#[test]
fn enchant_support_item_bonus_and_consume() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.data.enchant = crate::data::EnchantData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9850, 0);
    drain(&mut rx);

    // Bastard Sword 69 (D weapon), Enchant Weapon D scroll 955, and the D-grade
    // weapon support "Lucky Enchant Stone" 12362 (+20 bonus, valid at +3..9).
    items::add_inventory_item(&mut world, 9850, 955, 1).unwrap();
    items::add_inventory_item(&mut world, 9850, 69, 1).unwrap();
    items::add_inventory_item(&mut world, 9850, 12362, 1).unwrap();
    let (scroll, sword, support) = (
        item_oid(&world, 9850, 955),
        item_oid(&world, 9850, 69),
        item_oid(&world, 9850, 12362),
    );
    // The support requires the target already at +3.
    world
        .objects
        .get_component_mut::<Inventory>(&9850)
        .unwrap()
        .set_item_enchant(sword, 3);

    let use_scroll = {
        let mut w = PacketWriter::new();
        w.write_u8(cop::USE_ITEM);
        w.write_i32(scroll);
        w.write_i32(0);
        w.into_bytes()
    };
    on_packet(&mut world, 1, use_scroll);
    let add_scroll = {
        let mut w = PacketWriter::new();
        w.write_i32(scroll);
        w.write_i32(sword);
        w.into_bytes()
    };
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM,
            &add_scroll,
        ),
    );
    let put_target = {
        let mut w = PacketWriter::new();
        w.write_i32(sword);
        w.into_bytes()
    };
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM,
            &put_target,
        ),
    );
    // Support: body is (supportObjId, enchantObjId).
    let put_support = {
        let mut w = PacketWriter::new();
        w.write_i32(support);
        w.write_i32(sword);
        w.into_bytes()
    };
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_SUPPORT_ITEM,
            &put_support,
        ),
    );
    let put_out = drain(&mut rx);
    assert!(
        put_out.iter().any(|p| is_ex(
            p,
            server_packets::opcodes::EX_PUT_ENCHANT_SUPPORT_ITEM_RESULT
        )),
        "support accepted"
    );

    // Roll 80%: bare chance 66.67 would fail, but +20 support → 86.67 succeeds.
    // Age the window past Java's 2 s anti-autoenchant guard first.
    world.tick += 20;
    world.force_roll(800_000);
    let enchant = {
        let mut w = PacketWriter::new();
        w.write_u8(cop::REQUEST_ENCHANT_ITEM);
        w.write_i32(sword);
        w.write_i32(support);
        w.into_bytes()
    };
    on_packet(&mut world, 1, enchant);

    let inv = world.objects.get_component::<Inventory>(&9850).unwrap();
    let level = inv.by_object_id(sword).unwrap().enchant_level;
    assert_eq!(level, 4, "support bonus carried the +3 → +4 enchant");
    assert_eq!(inv.count_of(12362), 0, "support consumed");
    assert_eq!(inv.count_of(955), 0, "scroll consumed");
}

/// Freight (the account-package warehouse): the `package_withdraw` half. Seed
/// the container as if another character had sent items, withdraw part of it,
/// and confirm it persists with `loc="FREIGHT"`.
#[test]
fn freight_withdraw_and_persist() {
    use crate::model::inventory::{Freight, Inventory};
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9600, 0);
    drain(&mut rx);

    // Seed 300 adena into the freight (as if sent by another character).
    let fr_oid = world.alloc_object_id().unwrap();
    {
        let World { objects, data, .. } = &mut world;
        objects
            .get_component_mut::<Freight>(&9600)
            .unwrap()
            .0
            .add_item(&data.item_data, fr_oid, 57, 300);
    }

    // package_withdraw → active = freight, window opens.
    warehouse::open_freight_withdraw(&mut world, 1);
    let withdraw = {
        let mut w = PacketWriter::new();
        w.write_u8(cop::SEND_WARE_HOUSE_WITH_DRAW_LIST);
        w.write_i32(1);
        w.write_i32(fr_oid);
        w.write_i64(120);
        w.into_bytes()
    };
    on_packet(&mut world, 1, withdraw);

    assert_eq!(
        world
            .objects
            .get_component::<Freight>(&9600)
            .unwrap()
            .0
            .count_of(57),
        180,
        "180 left in freight"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9600)
            .unwrap()
            .count_of(57),
        120,
        "120 withdrawn to inventory"
    );

    // Persisted with its own loc alongside inventory + warehouse.
    let save = build_save_data(&world, 9600).expect("save");
    let fr_row = save
        .items
        .iter()
        .find(|r| r.item_id == 57 && r.loc == "FREIGHT")
        .expect("freight row");
    assert_eq!(fr_row.count, 180);
    assert!(
        save.items
            .iter()
            .any(|r| r.item_id == 57 && r.loc == "INVENTORY" && r.count == 120),
        "inventory row"
    );
}

/// Augmentation: confirm the life stone, refine (roll + consume + stamp), then
/// cancel for the adena fee.
#[test]
fn augment_make_and_cancel() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.data.variations = crate::data::VariationData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut rx = ingame_player_access(&mut world, 1, 9900, 0);
    drain(&mut rx);

    // Crimson Sword (2551, augmentable D weapon), Life Stone Lv.46 (8723),
    // Gemstone D (2130) ×20, and adena for the cancel fee (95000).
    items::add_inventory_item(&mut world, 9900, 2551, 1).unwrap();
    items::add_inventory_item(&mut world, 9900, 8723, 1).unwrap();
    items::add_inventory_item(&mut world, 9900, 2130, 20).unwrap();
    items::add_inventory_item(&mut world, 9900, 57, 200_000).unwrap();
    let (weapon, lifestone, gem) = (
        item_oid(&world, 9900, 2551),
        item_oid(&world, 9900, 8723),
        item_oid(&world, 9900, 2130),
    );

    // Confirm the refiner → the make window echoes the gemstone fee.
    let mut confirm = PacketWriter::new();
    confirm.write_i32(weapon);
    confirm.write_i32(lifestone);
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_CONFIRM_REFINER_ITEM,
            &confirm.into_bytes(),
        ),
    );
    let confirm_out = drain(&mut rx);
    assert!(
        confirm_out.iter().any(|p| is_ex(
            p,
            server_packets::opcodes::EX_PUT_INTENSIVE_RESULT_FOR_VARIATION_MAKE
        )),
        "confirm echoes fee"
    );

    // Refine: force low rolls so the augment always resolves.
    world.force_rolls(std::iter::repeat_n(0, 8));
    let mut refine = PacketWriter::new();
    refine.write_i32(weapon);
    refine.write_i32(lifestone);
    refine.write_i32(gem);
    refine.write_i64(20);
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_REFINE, &refine.into_bytes()),
    );

    let inv = world.objects.get_component::<Inventory>(&9900).unwrap();
    assert!(inv.is_augmented(weapon), "weapon augmented");
    assert_eq!(inv.count_of(8723), 0, "life stone consumed");
    assert_eq!(inv.count_of(2130), 0, "20 gemstones consumed");
    let (o1, o2) = inv.augmentation_of(weapon).unwrap();
    assert!(o1 != 0 && o2 != 0, "two options rolled");

    // Persistence round-trip: the augment rides the item rows (→ item_variations)
    // and restores through `from_rows`.
    let save = build_save_data(&world, 9900).expect("save");
    let wrow = save
        .items
        .iter()
        .find(|r| r.object_id == weapon)
        .expect("weapon row");
    assert_eq!(
        (
            wrow.augment_mineral,
            wrow.augment_option1,
            wrow.augment_option2
        ),
        (8723, o1, o2),
        "augment persisted on the row"
    );
    let restored = Inventory::from_rows(&save.items);
    assert_eq!(
        restored.augmentation_of(weapon),
        Some((o1, o2)),
        "augment restored on reload"
    );

    // Cancel: pays the adena fee and strips the augment.
    let mut cancel = PacketWriter::new();
    cancel.write_i32(weapon);
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_REFINE_CANCEL, &cancel.into_bytes()),
    );
    let inv = world.objects.get_component::<Inventory>(&9900).unwrap();
    assert!(!inv.is_augmented(weapon), "augment removed");
    assert_eq!(
        inv.count_of(57),
        200_000 - 95_000,
        "adena cancel fee charged"
    );
}

/// Build a `SetPrivateStoreListBuy` body: the wanted lines, keyed by item id
/// with the client's enchant/augment/element tail.
fn set_buy_list(lines: &[(i32, i64, i64)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(cop::SET_PRIVATE_STORE_LIST_BUY);
    w.write_i32(lines.len() as i32);
    for &(item_id, count, price) in lines {
        w.write_i32(item_id);
        w.write_i16(0); // enchant
        w.write_i16(0); // unknown
        w.write_i64(count);
        w.write_i64(price);
        w.write_i32(0); // augment option 1
        w.write_i32(0); // augment option 2
        for _ in 0..8 {
            w.write_i16(0); // attack element + six defences
        }
        w.write_i32(0); // visual id
    }
    w.into_bytes()
}

/// Build a `RequestPrivateStoreSell` body: the store owner and the offered
/// lines, with the soul-crystal/SA tails empty.
fn store_sell_body(store_player: i32, lines: &[(i32, i32, i64, i64)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PRIVATE_STORE_SELL);
    w.write_i32(store_player);
    w.write_i32(lines.len() as i32);
    for &(object_id, item_id, count, price) in lines {
        w.write_i32(object_id);
        w.write_i32(item_id);
        w.write_i16(0); // enchant
        w.write_i16(0); // unknown
        w.write_i64(count);
        w.write_i64(price);
        w.write_i32(0); // visual
        w.write_i32(0); // option 1
        w.write_i32(0); // option 2
        w.write_u8(0); // soul-crystal options
        w.write_u8(0); // SA effects
    }
    w.into_bytes()
}

/// A private **buy** store: the owner posts what they want, a customer sells
/// into it — items customer→owner, adena owner→customer.
#[test]
fn private_buy_store_takes_items_and_pays_out() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut owner_rx = ingame_player_access(&mut world, 1, 9610, 0);
    let mut seller_rx = ingame_player_access(&mut world, 2, 9611, 0);
    drain(&mut owner_rx);
    drain(&mut seller_rx);
    // The buyer has 1000 adena to spend; the seller has 10 D-grade crystals.
    items::add_inventory_item(&mut world, 9610, 57, 1000).unwrap();
    items::add_inventory_item(&mut world, 9611, 1458, 10).unwrap();
    let crystal_oid = item_oid(&world, 9611, 1458);

    // Wanted: 4 crystals at 100 adena each (400 total, affordable).
    on_packet(&mut world, 1, set_buy_list(&[(1458, 4, 100)]));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9610)
            .unwrap()
            .store_type,
        3,
        "the buy store is open"
    );

    // The customer offers ten, but only four are wanted — the rest stay put.
    on_packet(
        &mut world,
        2,
        store_sell_body(9610, &[(crystal_oid, 1458, 10, 100)]),
    );
    {
        let seller_inv = world.objects.get_component::<Inventory>(&9611).unwrap();
        assert_eq!(
            seller_inv.count_of(1458),
            6,
            "only the four wanted changed hands"
        );
        assert_eq!(
            seller_inv.count_of(57),
            400,
            "and were paid for at 100 each"
        );
    }
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9610)
            .unwrap()
            .store_type,
        0,
        "a filled buy store closes"
    );

    // Re-open a smaller store and fill it in two goes.
    on_packet(&mut world, 1, set_buy_list(&[(1458, 4, 100)]));
    on_packet(
        &mut world,
        2,
        store_sell_body(9610, &[(crystal_oid, 1458, 3, 100)]),
    );
    let seller_inv = world.objects.get_component::<Inventory>(&9611).unwrap();
    assert_eq!(
        seller_inv.count_of(1458),
        3,
        "three more crystals handed over"
    );
    assert_eq!(seller_inv.count_of(57), 700, "paid 300 more adena");
    let owner_inv = world.objects.get_component::<Inventory>(&9610).unwrap();
    assert_eq!(owner_inv.count_of(1458), 7, "the owner received them");
    assert_eq!(owner_inv.count_of(57), 300, "and spent 300 more");
    // One line still wanted, so the store stays open.
    assert_eq!(
        world
            .objects
            .get_component::<model::components::PrivateBuyStore>(&9610)
            .unwrap()
            .items[0]
            .count,
        1,
        "one crystal still wanted"
    );

    // Filling the last one closes the store.
    on_packet(
        &mut world,
        2,
        store_sell_body(9610, &[(crystal_oid, 1458, 1, 100)]),
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9610)
            .unwrap()
            .store_type,
        0,
        "a filled buy store closes"
    );
}

/// A buy store may not ask for more than the owner can pay for.
#[test]
fn private_buy_store_refuses_an_unaffordable_list() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut rx = ingame_player_access(&mut world, 1, 9612, 0);
    drain(&mut rx);
    items::add_inventory_item(&mut world, 9612, 57, 100).unwrap();

    // 10 × 100 = 1000 adena wanted, but only 100 in the purse.
    on_packet(&mut world, 1, set_buy_list(&[(1458, 10, 100)]));

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9612)
            .unwrap()
            .store_type,
        0,
        "the store never opened"
    );
    assert!(
        has_system_message(
            &drain(&mut rx),
            server_packets::sm_ids::THE_PURCHASE_PRICE_IS_HIGHER_THAN_YOUR_MONEY
        ),
        "and the client is told why"
    );
}

/// The wanted list is capped by `MaxPvtStoreBuySlots*` (4 for a non-Dwarf).
#[test]
fn private_buy_store_enforces_the_slot_limit() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut rx = ingame_player_access(&mut world, 1, 9613, 0);
    drain(&mut rx);
    items::add_inventory_item(&mut world, 9613, 57, 1_000_000).unwrap();

    let five = [
        (1458, 1, 100),
        (1459, 1, 100),
        (1460, 1, 100),
        (1461, 1, 100),
        (1462, 1, 100),
    ];
    on_packet(&mut world, 1, set_buy_list(&five));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9613)
            .unwrap()
            .store_type,
        0,
        "five lines is one over the limit"
    );

    on_packet(&mut world, 1, set_buy_list(&five[..4]));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9613)
            .unwrap()
            .store_type,
        3,
        "four lines is fine"
    );
}

/// **An augmented weapon's options pump the wearer's stats while it is worn.**
/// Java's equip listener calls `VariationInstance.applyBonus` before the stat
/// recompute, and the unequip listener `removeBonus` — so the two option ids
/// behave like a pair of passive buffs tied to the item.
#[test]
fn augment_options_apply_while_the_item_is_equipped() {
    use crate::data::item_data::{
        CrystalType, ItemHandler, ItemKind, ItemStats, ItemTemplate, SLOT_R_HAND,
    };
    use crate::data::option_data::OptionEntry;
    use crate::model::inventory::Inventory;
    use crate::model::skill::StatModifierEffect;
    use crate::model::stats::{Stat, StatModifierType};

    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.id_pool = 0x4200_0000..0x4200_0100;

    // A plain weapon…
    let template = ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::Other,
        item_id: 600,
        name: "Augmented Blade".into(),
        kind: ItemKind::Weapon,
        body_part: SLOT_R_HAND,
        weight: 0,
        is_stackable: false,
        is_infinite: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
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
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    };
    world.data.item_data.insert_for_test(template);
    world.data.item_data.set_item_stats_for_test(
        600,
        ItemStats {
            bonuses: vec![(Stat::PhysicalAttack, 100.0)],
            ..Default::default()
        },
    );
    // …and two options: +200 P.Def flat, and +100 P.Atk flat.
    let option = |id: i32, stat: Stat, amount: f64| OptionEntry {
        id,
        effects: vec![StatModifierEffect {
            stat,
            mode: StatModifierType::Diff,
            amount,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
        }],
        ..Default::default()
    };
    world
        .data
        .options
        .insert_for_test(option(4001, Stat::PhysicalDefence, 200.0));
    world
        .data
        .options
        .insert_for_test(option(4002, Stat::PhysicalAttack, 100.0));
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9500, 600, 1);
        inv.set_augmentation(9500, 8723, 4001, 4002);
    }
    drain(&mut rx);

    let base_p_def = pcs(&world, 3001).p_def;
    let base_p_atk = pcs(&world, 3001).p_atk;

    // Equip: both options land.
    items::handle_use_item(&mut world, 1, &use_item_body(9500));
    let equipped_p_def = pcs(&world, 3001).p_def;
    let equipped_p_atk = pcs(&world, 3001).p_atk;
    assert!(
        equipped_p_def >= base_p_def + 200.0,
        "the +200 P.Def option applied (was {base_p_def}, now {equipped_p_def})"
    );
    assert!(
        equipped_p_atk >= base_p_atk + 100.0,
        "…and the +100 P.Atk one (was {base_p_atk}, now {equipped_p_atk})"
    );

    // Unequip: both come back off, exactly.
    items::handle_use_item(&mut world, 1, &use_item_body(9500));
    assert_eq!(
        pcs(&world, 3001).p_def,
        base_p_def,
        "P.Def returns to its unaugmented value"
    );
    assert_eq!(pcs(&world, 3001).p_atk, base_p_atk, "and so does P.Atk");
}

/// **A package store sells its whole list as one lot.** `/packagesale` (player
/// action 61) opens the manage window in package mode; the store then reports
/// `PACKAGE_SELL` (8), and a buyer who asks for fewer lines than it holds is
/// refused outright — Java's anti-bot check. Taking every line goes through.
#[test]
fn package_store_is_all_or_nothing() {
    use crate::model::components::PrivateStore;
    use crate::model::inventory::Inventory;

    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4300_0000..0x4300_0200;
    let mut seller_rx = ingame_player_access(&mut world, 1, 9700, 0);
    let mut buyer_rx = ingame_player_access(&mut world, 2, 9701, 0);
    // Two distinct items so the package has two lines.
    items::add_inventory_item(&mut world, 9700, 1458, 5).unwrap(); // Crystal (D)
    items::add_inventory_item(&mut world, 9700, 1459, 5).unwrap(); // Crystal (C)
    items::add_inventory_item(&mut world, 9701, 57, 10_000).unwrap();
    let (a, b) = (item_oid(&world, 9700, 1458), item_oid(&world, 9700, 1459));
    drain(&mut seller_rx);
    drain(&mut buyer_rx);

    // `/packagesale` → the manage window opens with the package flag set.
    // The press dispatches through `ActionData.xml`'s handler table, which the
    // fixture world ships empty: without the row the packet finds no handler
    // and no window opens at all.
    world
        .data
        .action_data
        .insert_row_for_test(61, "PrivateStore", 8);
    let mut act = PacketWriter::new();
    act.write_u8(cop::REQUEST_ACTION_USE);
    act.write_i32(61);
    act.write_i32(0);
    act.write_u8(0);
    on_packet(&mut world, 1, act.into_bytes());
    let manage = drain(&mut seller_rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::PRIVATE_STORE_MANAGE_LIST)
        .expect("the manage window");
    assert_eq!(
        i32::from_le_bytes([manage[5], manage[6], manage[7], manage[8]]),
        1,
        "the window is flagged as a package sale"
    );

    // Open the package store with both lines.
    let mut w = PacketWriter::new();
    w.write_u8(cop::SET_PRIVATE_STORE_LIST_SELL);
    w.write_i32(1); // package sale
    w.write_i32(2);
    w.write_i32(a);
    w.write_i64(5);
    w.write_i64(100);
    w.write_i32(b);
    w.write_i64(5);
    w.write_i64(200);
    on_packet(&mut world, 1, w.into_bytes());
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9700)
            .unwrap()
            .store_type,
        8,
        "PACKAGE_SELL"
    );
    assert!(
        world
            .objects
            .get_component::<PrivateStore>(&9700)
            .unwrap()
            .packaged
    );

    // Buying only one of the two lines is refused — nothing moves.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PRIVATE_STORE_BUY);
    w.write_i32(9700);
    w.write_i32(1);
    w.write_i32(a);
    w.write_i64(5);
    w.write_i64(100);
    on_packet(&mut world, 2, w.into_bytes());
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9701)
            .unwrap()
            .count_of(57),
        10_000,
        "a partial package purchase pays nothing"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9701)
            .unwrap()
            .count_of(1458),
        0,
        "…and delivers nothing"
    );

    // Taking the whole package works: 5×100 + 5×200 = 1500 adena.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PRIVATE_STORE_BUY);
    w.write_i32(9700);
    w.write_i32(2);
    w.write_i32(a);
    w.write_i64(5);
    w.write_i64(100);
    w.write_i32(b);
    w.write_i64(5);
    w.write_i64(200);
    on_packet(&mut world, 2, w.into_bytes());

    let buyer = world.objects.get_component::<Inventory>(&9701).unwrap();
    assert_eq!(buyer.count_of(1458), 5, "first line delivered");
    assert_eq!(buyer.count_of(1459), 5, "second line delivered");
    assert_eq!(
        buyer.count_of(57),
        10_000 - 1_500,
        "and paid for as one lot"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9700)
            .unwrap()
            .store_type,
        0,
        "the emptied store closes"
    );
}

/// **`SetPrivateStoreWholeMsg` (ex 0x47) titles the package store** and echoes
/// `ExPrivateStoreSetWholeMsg` back — the package-sell counterpart of
/// `PrivateStoreMsgSell`, which was missing entirely.
#[test]
fn package_store_title_round_trips() {
    use crate::model::components::PrivateStore;

    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 9702, 0);
    drain(&mut rx);

    let mut body = PacketWriter::new();
    body.write_string("Whole lot!");
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::SET_PRIVATE_STORE_WHOLE_MSG,
            &body.into_bytes(),
        ),
    );

    assert_eq!(
        world
            .objects
            .get_component::<PrivateStore>(&9702)
            .map(|s| s.title.clone()),
        Some("Whole lot!".to_string())
    );
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| is_ex(p, server_packets::opcodes::EX_PRIVATE_STORE_WHOLE_MSG)),
        "the title is echoed back"
    );
}

/// **Freighting items to another character on the account.** `package_deposit`
/// offers the account's other characters, the send window lists only
/// `is_freightable` items, and the send itself charges `FreightPrice` per slot
/// and writes the items to the (offline) recipient's freight rows.
#[test]
fn freight_send_delivers_to_an_offline_character() {
    use crate::model::components::LastFolkNpc;
    use crate::model::inventory::Inventory;

    let (mut world, mut db, _link) = quest_test_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4400_0000..0x4400_0200;

    // The sender, with a second character (9902 "Alt") on the account.
    let chr = dummy_char(9901, "Sender");
    let bundle = Player::from_char(&world.data, &chr);
    let (out_tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let session = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![dummy_char(9901, "Sender"), dummy_char(9902, "Alt")])
        .into_entering(bundle);
    let (session, bundle) = session.into_ingame();
    bundle.spawn_into(&mut world);
    world.clients.insert(1, ClientSession::InGame(session));

    // A freight manager in range (the send checks the last folk NPC).
    add_test_npc(&mut world, NPC_OID, 30001, "Warehouse", 70, 0, 0, 0);
    world.objects.add_components(&9901, LastFolkNpc(NPC_OID));

    // **No item below id 10000 declares `is_freightable` on this dist** — every
    // one of the 3416 that do is later-chronicle (10649+). Java's gate is the
    // same, so the freight can only ever carry those; 10649 (Feather of
    // Blessing) is the lowest and stands in for the mechanism here.
    const FREIGHTABLE: i32 = 10649;
    assert!(
        world
            .data
            .item_data
            .get(FREIGHTABLE)
            .unwrap()
            .is_freightable,
        "fixture assumption: 10649 is freightable"
    );
    items::add_inventory_item(&mut world, 9901, FREIGHTABLE, 10).unwrap();
    items::add_inventory_item(&mut world, 9901, 57, 5_000).unwrap();
    let crystal = item_oid(&world, 9901, FREIGHTABLE);
    drain(&mut rx);

    // `package_deposit` → the account's other characters.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_package_deposit")),
    );
    let list = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::PACKAGE_TO_LIST)
        .expect("the send-to list");
    assert_eq!(
        i32::from_le_bytes([list[1], list[2], list[3], list[4]]),
        1,
        "one other character on the account"
    );

    // The send window lists the freightable item.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PACKAGE_SENDABLE_ITEM_LIST);
    w.write_i32(9902);
    on_packet(&mut world, 1, w.into_bytes());
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::PACKAGE_SENDABLE_LIST),
        "the sendable-item window opens"
    );

    // Send 4 crystals: they leave the inventory, the 1000-adena slot fee is
    // charged, and the delivery is written to the offline recipient's rows.
    drain_db(&mut db);
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PACKAGE_SEND);
    w.write_i32(9902);
    w.write_i32(1);
    w.write_i32(crystal);
    w.write_i64(4);
    on_packet(&mut world, 1, w.into_bytes());

    let inv = world.objects.get_component::<Inventory>(&9901).unwrap();
    assert_eq!(inv.count_of(FREIGHTABLE), 6, "4 items left the sender");
    assert_eq!(inv.count_of(57), 4_000, "the 1000-adena fee was charged");
    let delivered = drain_db(&mut db).into_iter().find_map(|c| match c {
        db::DbCommand::AddFreightItems {
            owner_id: 9902,
            items,
        } => Some(items),
        _ => None,
    });
    let items = delivered.expect("the freight rows were written");
    assert_eq!(items.len(), 1);
    assert_eq!((items[0].item_id, items[0].count), (FREIGHTABLE, 4));
}

/// **The freight refuses what it may not carry.** A non-freightable item and a
/// recipient who isn't on the account both leave everything where it is.
#[test]
fn freight_send_refuses_bad_items_and_strangers() {
    use crate::model::components::LastFolkNpc;
    use crate::model::inventory::Inventory;

    let (mut world, _db, _link) = quest_test_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4500_0000..0x4500_0200;
    let chr = dummy_char(9903, "Sender");
    let bundle = Player::from_char(&world.data, &chr);
    let (out_tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let session = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![dummy_char(9903, "Sender"), dummy_char(9904, "Alt")])
        .into_entering(bundle);
    let (session, bundle) = session.into_ingame();
    bundle.spawn_into(&mut world);
    world.clients.insert(1, ClientSession::InGame(session));
    add_test_npc(&mut world, NPC_OID, 30001, "Warehouse", 70, 0, 0, 0);
    world.objects.add_components(&9903, LastFolkNpc(NPC_OID));

    // Adena — like every other Interlude-range item on this dist — is not
    // freightable; 10649 is, and stands in for a legal cargo below.
    items::add_inventory_item(&mut world, 9903, 57, 5_000).unwrap();
    items::add_inventory_item(&mut world, 9903, 10649, 5).unwrap();
    assert!(
        !world.data.item_data.get(1458).unwrap().is_freightable,
        "Crystal (D) — an Interlude item — may not be freighted"
    );
    let (adena_oid, crystal) = (item_oid(&world, 9903, 57), item_oid(&world, 9903, 10649));
    drain(&mut rx);

    // A non-freightable line aborts the whole send.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PACKAGE_SEND);
    w.write_i32(9904);
    w.write_i32(1);
    w.write_i32(adena_oid);
    w.write_i64(100);
    on_packet(&mut world, 1, w.into_bytes());
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9903)
            .unwrap()
            .count_of(57),
        5_000,
        "a non-freightable item is refused, fee included"
    );

    // A recipient who isn't on the account is refused too.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PACKAGE_SEND);
    w.write_i32(7777);
    w.write_i32(1);
    w.write_i32(crystal);
    w.write_i64(5);
    on_packet(&mut world, 1, w.into_bytes());
    let inv = world.objects.get_component::<Inventory>(&9903).unwrap();
    assert_eq!(inv.count_of(10649), 5, "nothing was sent to a stranger");
    assert_eq!(inv.count_of(57), 5_000, "and no fee was taken");
}

/// **A `SKILL_REDUCE_ON_SKILL_SUCCESS` item is spent when the cast lands, not
/// when it is used.** Java's `SkillCaster.finishSkill` destroys the triggering
/// item; the port used to take it up front (a documented safe-side deviation),
/// so an interrupted cast still cost the item. Interlude's pair is 8058/8060
/// (Lockup Research Report / Key of Enigma → skill 2260).
#[test]
fn skill_reduce_on_success_item_is_spent_only_when_the_cast_lands() {
    use crate::data::item_data::{ItemHandler, ItemKind, ItemTemplate};
    use crate::model::components::Casting;
    use crate::model::inventory::Inventory;
    use crate::model::skill::{AffectObject, AffectScope, OperateType, Skill, TargetType};

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4600_0000..0x4600_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        id: 2260,
        level: 1,
        name: "Key of Enigma".into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Self_,
        magic_type: 2, // static: hitTime used verbatim
        magic_level: 0,
        effect_point: 0,
        cast_range: 0,
        effect_range: 0,
        hit_time: 5_000,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        without_action: false,
        trait_type: model::skill::TraitType::None,
        item_consume_id: 8058,
        item_consume_count: 1,
        abnormal_time: 0,
        abnormal_level: 0,
        abnormal_type: "NONE".into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects: Vec::new(),
        ..Default::default()
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::SkillReduceOnSkillSuccess,
        item_id: 8058,
        name: "Lockup Research Report".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        is_infinite: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::ItemSkills,
        crystal_type: crate::data::item_data::CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(2260, 1)],
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    // The finish phase re-checks HP (`cur_hp <= hp_consume` aborts), and the
    // bare fixture player starts at 0.
    if let Some(vitals) = world.objects.get_component_mut::<Vitals>(&3001) {
        vitals.cur_hp = 100.0;
    }
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9100, 8058, 2);
    }
    drain(&mut rx);

    // --- Interrupted cast: the item survives. ---
    items::handle_use_item(&mut world, 1, &use_item_body(9100));
    assert!(world.objects.has_component::<Casting>(&3001), "casting");
    assert_eq!(
        count_of_item(&world, 3001, 8058),
        2,
        "nothing is taken at use time"
    );
    abort_cast(&mut world, 3001);
    advance_ticks(&mut world, 60);
    assert_eq!(
        count_of_item(&world, 3001, 8058),
        2,
        "an aborted cast costs nothing"
    );

    // --- Completed cast: exactly one is spent. ---
    drain(&mut rx);
    items::handle_use_item(&mut world, 1, &use_item_body(9100));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "casting again"
    );
    advance_ticks(&mut world, 60); // 5 s hit time + the finish floor
    assert!(
        !world.objects.has_component::<Casting>(&3001),
        "the cast finished"
    );
    assert_eq!(
        count_of_item(&world, 3001, 8058),
        1,
        "one spent when the cast landed"
    );
}

// ---------------------------------------------------------------------------
// Row 11 — the augment window's confirm steps (ex 0x26 / 0x28 / 0x3F)
// ---------------------------------------------------------------------------

/// **The augment window's three confirm steps echo what the player dropped in**
/// — the weapon, the gemstone fee, and (in the cancel window) the augmented
/// item with its options and price. An unsuitable item is refused instead.
#[test]
fn the_augment_window_confirms_each_slot() {
    use crate::model::inventory::Inventory;

    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.data.variations = crate::data::VariationData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.id_pool = 0x4700_0000..0x4700_0200;
    let mut rx = ingame_player_access(&mut world, 1, 9910, 0);
    items::add_inventory_item(&mut world, 9910, 2551, 1).unwrap(); // Crimson Sword
    items::add_inventory_item(&mut world, 9910, 8723, 1).unwrap(); // Life Stone 46
    items::add_inventory_item(&mut world, 9910, 2130, 20).unwrap(); // Gemstone D
    items::add_inventory_item(&mut world, 9910, 1458, 1).unwrap(); // Crystal (D)
    let (weapon, lifestone, gem, crystal) = (
        item_oid(&world, 9910, 2551),
        item_oid(&world, 9910, 8723),
        item_oid(&world, 9910, 2130),
        item_oid(&world, 9910, 1458),
    );
    drain(&mut rx);

    // (1) target item: an augmentable weapon echoes back.
    let mut w = PacketWriter::new();
    w.write_i32(weapon);
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_CONFIRM_TARGET_ITEM, &w.into_bytes()),
    );
    assert!(
        drain(&mut rx).iter().any(|p| is_ex(
            p,
            server_packets::opcodes::EX_PUT_ITEM_RESULT_FOR_VARIATION_MAKE
        )),
        "the weapon is accepted"
    );

    // …a Crystal is not a weapon: refused with the system message, no echo.
    let mut w = PacketWriter::new();
    w.write_i32(crystal);
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_CONFIRM_TARGET_ITEM, &w.into_bytes()),
    );
    let pkts = drain(&mut rx);
    assert!(
        !pkts.iter().any(|p| is_ex(
            p,
            server_packets::opcodes::EX_PUT_ITEM_RESULT_FOR_VARIATION_MAKE
        )),
        "an unsuitable item is not echoed"
    );
    assert!(
        ids_after_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THIS_IS_NOT_A_SUITABLE_ITEM)
    );

    // (2) gemstone: the fee the refiner step quoted is echoed back.
    let mut w = PacketWriter::new();
    w.write_i32(weapon);
    w.write_i32(lifestone);
    w.write_i32(gem);
    w.write_i64(20);
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_CONFIRM_GEMSTONE, &w.into_bytes()),
    );
    assert!(
        drain(&mut rx).iter().any(|p| is_ex(
            p,
            server_packets::opcodes::EX_PUT_COMMISSION_RESULT_FOR_VARIATION_MAKE
        )),
        "the gemstone fee is accepted"
    );

    // (3) cancel window: an unaugmented item is refused…
    let mut w = PacketWriter::new();
    w.write_i32(weapon);
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_CONFIRM_CANCEL_ITEM, &w.into_bytes()),
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::AUGMENTATION_REMOVAL_ONLY_ON_AN_AUGMENTED_ITEM)
    );

    // …and an augmented one echoes with its options.
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&9910) {
        inv.set_augmentation(weapon, 8723, 4001, 4002);
    }
    let mut w = PacketWriter::new();
    w.write_i32(weapon);
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_CONFIRM_CANCEL_ITEM, &w.into_bytes()),
    );
    let echo = drain(&mut rx)
        .into_iter()
        .find(|p| {
            is_ex(
                p,
                server_packets::opcodes::EX_PUT_ITEM_RESULT_FOR_VARIATION_CANCEL,
            )
        })
        .expect("the cancel echo");
    assert_eq!(
        i32::from_le_bytes([echo[11], echo[12], echo[13], echo[14]]),
        4001,
        "…carrying the first option id"
    );
}

/// **The client's key layout survives a relogin.** `RequestSaveKeyMapping`
/// stores the blob in a player variable (Java's `UI_KEY_MAPPING`), and
/// `RequestKeyMapping` replays it verbatim.
#[test]
fn the_saved_key_mapping_round_trips() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    // Nothing saved yet: Java's empty payload.
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_KEY_MAPPING, &[]),
    );
    let empty = drain(&mut rx)
        .into_iter()
        .find(|p| is_ex(p, server_packets::opcodes::EX_UI_SETTING))
        .expect("the UI setting packet");
    assert_eq!(
        i32::from_le_bytes([empty[3], empty[4], empty[5], empty[6]]),
        0,
        "no stored layout"
    );

    // Save three bytes, then ask for them back.
    let mut w = PacketWriter::new();
    w.write_i32(3);
    for b in [7u8, 0, 200] {
        w.write_u8(b);
    }
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_SAVE_KEY_MAPPING, &w.into_bytes()),
    );
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_KEY_MAPPING, &[]),
    );
    let stored = drain(&mut rx)
        .into_iter()
        .find(|p| is_ex(p, server_packets::opcodes::EX_UI_SETTING))
        .expect("the UI setting packet");
    assert_eq!(
        i32::from_le_bytes([stored[3], stored[4], stored[5], stored[6]]),
        3,
        "three bytes come back"
    );
    assert_eq!(&stored[7..10], &[7, 0, 200], "…verbatim, high bytes intact");
}

/// **Herbs run their own auto-destroy clock.** Java's gate is an *either/or*:
/// `(AUTODESTROY_ITEM_AFTER > 0 && !hasExImmediateEffect()) ||
/// (HERB_AUTO_DESTROY_TIME > 0 && hasExImmediateEffect())`. So a herb vanishes
/// on `AutoDestroyHerbTime` (60 s) rather than the ordinary 600 s — and it is
/// scheduled even when the ordinary destroyer is switched off entirely.
#[test]
fn herbs_decay_on_their_own_shorter_clock() {
    use crate::game_loop::ground_items::{DropSource, spawn_ground_item};
    use crate::model::components::GroundItem;

    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.cfg.general.autodestroy_item_after = 600;
    world.cfg.general.herb_auto_destroy_time = 60;
    world.id_pool = 0x4000_0000..0x4000_0100;
    let _rx = ingame_player_access(&mut world, 1, 9300, 0);

    // 8600 "Herb of Life" carries `ex_immediate_effect`; 57 adena does not.
    let herb = spawn_ground_item(&mut world, 8600, 1, 0, 100, 200, 0, 0, DropSource::Npc);
    let coin = spawn_ground_item(&mut world, 57, 100, 0, 100, 200, 0, 0, DropSource::Npc);

    // Past 60 s: the herb is gone, the coin is not.
    world.tick += 60 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(
        !world.objects.has_component::<GroundItem>(&herb),
        "the herb is swept on the 60 s herb clock"
    );
    assert!(
        world.objects.has_component::<GroundItem>(&coin),
        "…while an ordinary drop still has its 600 s"
    );

    // Past 600 s the coin goes too.
    world.tick += 600 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(!world.objects.has_component::<GroundItem>(&coin));
}

/// The herb clock is gated **independently** of the ordinary one: with
/// `AutoDestroyDroppedItemAfter = 0` a herb is still swept, because Java's two
/// conditions are alternatives rather than nested.
#[test]
fn herbs_decay_even_with_the_ordinary_destroyer_off() {
    use crate::game_loop::ground_items::{DropSource, spawn_ground_item};
    use crate::model::components::GroundItem;

    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.cfg.general.autodestroy_item_after = 0; // ordinary destroyer off
    world.cfg.general.herb_auto_destroy_time = 60;
    world.id_pool = 0x4000_0000..0x4000_0100;
    let _rx = ingame_player_access(&mut world, 1, 9300, 0);

    let herb = spawn_ground_item(&mut world, 8600, 1, 0, 100, 200, 0, 0, DropSource::Npc);
    let coin = spawn_ground_item(&mut world, 57, 100, 0, 100, 200, 0, 0, DropSource::Npc);
    world.tick += 60 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(
        !world.objects.has_component::<GroundItem>(&herb),
        "the herb clock stands on its own"
    );
    assert!(
        world.objects.has_component::<GroundItem>(&coin),
        "and the coin is never scheduled at all"
    );
}

/// `UserInfo`'s STATS block leads with the character's physical attack range —
/// Java's `getActiveWeaponItem() != null ? 40 : 20`. The client walks to that
/// distance before swinging, so an armed player reporting the unarmed 20 closes
/// further than they need to.
///
/// It was hard-coded to 20. The branch is weapon *presence*, not type.
///
/// Asserted by **diffing** the armed and unarmed packets rather than seeking a
/// byte offset: a first draft searched for the STATS block length (56) and
/// matched a coincidental `[56, 0]` in an earlier field, reading 51.
#[test]
fn the_user_info_stats_block_reports_weapon_attack_range() {
    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, 1, 8300, 0, 0, 0);

    let packet = |world: &World| -> Vec<u8> {
        let view = model::PlayerView::of(&world.objects, 8300).unwrap();
        crate::network::user_info::user_info(&view, &world.data, &world.cfg.character, 0)
    };

    let unarmed = packet(&world);

    let weapon = crate::character::ItemRow {
        object_id: 8_300_001,
        item_id: 1, // a Short Sword; any right-hand item takes the branch
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: model::inventory::PaperdollSlot::RHand as i32,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    };
    world
        .objects
        .add_components(&8300, Inventory::from_rows(&[weapon]));
    let armed = packet(&world);

    assert_eq!(unarmed.len(), armed.len(), "same layout either way");
    let diffs: Vec<usize> = (0..unarmed.len())
        .filter(|&i| unarmed[i] != armed[i])
        .collect();
    // Equipping also fills the paperdoll and enchant blocks, so isolate the
    // one i16 that flips 20 -> 40.
    let range_at = diffs
        .iter()
        .copied()
        .find(|&i| {
            i + 1 < unarmed.len()
                && i16::from_le_bytes([unarmed[i], unarmed[i + 1]]) == 20
                && i16::from_le_bytes([armed[i], armed[i + 1]]) == 40
        })
        .expect("an i16 that reads 20 unarmed and 40 armed");
    assert_eq!(
        i16::from_le_bytes([armed[range_at], armed[range_at + 1]]),
        40,
        "the armed attack range reaches the client"
    );
}

/// **Opening a shop suppresses inventory refreshes for 1500 ms** (Java
/// `Player.setInventoryBlockingStatus` + `InventoryEnableTask`).
///
/// The client fires its own `RequestItemList` while a buy window is coming up;
/// answering it redraws the inventory over the window the player just asked
/// for. Java ignores those requests for 1.5 s, and so does this now.
#[test]
fn a_shop_window_suppresses_item_list_refreshes_briefly() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    // Not blocked to begin with: a plain request is answered.
    drain(&mut rx);
    items::handle_request_item_list(&mut world, 1);
    assert!(
        !drain(&mut rx).is_empty(),
        "an unblocked item-list request is answered"
    );

    // Block, as opening a shop/warehouse does.
    crate::game_loop::helpers::block_inventory(&mut world, 3001);
    items::handle_request_item_list(&mut world, 1);
    assert!(
        drain(&mut rx).is_empty(),
        "the request is dropped while the window is opening"
    );

    // 1500 ms later the scheduled task lifts it. Java's task clears the flag
    // unconditionally, so a second window opened inside the window is
    // unblocked by the *first* task rather than extending the block.
    advance_ticks(&mut world, 16); // 1.6 s at 10 ticks/s
    assert!(
        !world.inventory_blocked.contains(&3001),
        "InventoryEnableTask lifted the block"
    );
    items::handle_request_item_list(&mut world, 1);
    assert!(
        !drain(&mut rx).is_empty(),
        "refreshes are answered again once the window has settled"
    );
}

/// `randomEnchantMin`/`Max` on the **scroll** — the success step is a roll over
/// an inclusive range, not a flat `+1`.
///
/// Java `RequestEnchantItem`'s SUCCESS arm is
/// `Rnd.get(randomEnchantMin, randomEnchantMax)` capped at `maxEnchant`. The
/// port had this on the support side only and hard-coded the scroll side to
/// `+1`, which is correct for every scroll that omits the attributes (Java
/// defaults min to 1 and max to min) and wrong for the 20 that carry them.
///
/// Driven with 33808 "Giant's Scroll: Enchant Weapon (B-grade)" — `min 1 max 3`
/// — because it is the one a player can actually obtain here: Q375 Whisper of
/// Dreams Part 2 rewards it, and this port ships that quest.
#[test]
fn a_scroll_with_a_random_range_rolls_its_enchant_step() {
    use crate::model::inventory::Inventory;
    const DIST: &str = crate::data::DIST_GAME;
    const SCROLL: i32 = 33808; // Giant's Scroll: Enchant Weapon (B-grade)
    const SWORD: i32 = 78; // Great Sword — B-grade weapon
    const PLAYER: i32 = 9801;

    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.data.enchant = crate::data::EnchantData::load_from(DIST);
    world.id_pool = 0x4100_0000..0x4100_0200;

    // The range really is in the dist, and really is a range — a scroll whose
    // min == max would make every assertion below pass for the wrong reason.
    let tpl = world.data.enchant.scroll(SCROLL).expect("33808 loaded");
    assert_eq!((tpl.random_min, tpl.random_max), (1, 3));

    let mut rx = ingame_player_access(&mut world, 1, PLAYER, 0);
    drain(&mut rx);
    items::add_inventory_item(&mut world, PLAYER, SCROLL, 5).unwrap();
    items::add_inventory_item(&mut world, PLAYER, SWORD, 1).unwrap();
    let scroll_oid = item_oid(&world, PLAYER, SCROLL);
    let sword_oid = item_oid(&world, PLAYER, SWORD);
    let level = |w: &World| {
        w.objects
            .get_component::<Inventory>(&PLAYER)
            .unwrap()
            .by_object_id(sword_oid)
            .map(|it| it.enchant_level)
            .unwrap()
    };

    let arm = |world: &mut World| {
        let mut w = PacketWriter::new();
        w.write_u8(cop::USE_ITEM);
        w.write_i32(scroll_oid);
        w.write_i32(0);
        on_packet(world, 1, w.into_bytes());
        let mut w = PacketWriter::new();
        w.write_i32(scroll_oid);
        w.write_i32(sword_oid);
        on_packet(
            world,
            1,
            ex_packet(
                cp::ex_opcodes::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM,
                &w.into_bytes(),
            ),
        );
        let mut w = PacketWriter::new();
        w.write_i32(sword_oid);
        on_packet(
            world,
            1,
            ex_packet(
                cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM,
                &w.into_bytes(),
            ),
        );
    };
    let do_enchant = |world: &mut World| {
        let mut w = PacketWriter::new();
        w.write_u8(cop::REQUEST_ENCHANT_ITEM);
        w.write_i32(sword_oid);
        w.write_i32(0);
        on_packet(world, 1, w.into_bytes());
    };

    // Roll order per attempt: the success check (`roll_f64`) consumes one
    // forced value, then the step roll consumes the next. `roll(3)` returns an
    // index in 0..3, so the step is `min + index`.
    arm(&mut world);
    // Java's anti-autoenchant guard punishes an Enchant pressed within 2 s of
    // the last window interaction, so the window has to age before the press.
    world.tick += 20;
    world.force_roll(0); // success
    world.force_roll(2); // index 2 → step 1 + 2 = 3
    do_enchant(&mut world);
    assert_eq!(level(&world), 3, "the top of the range is +3, not +1");

    arm(&mut world);
    world.tick += 20;
    world.force_roll(0); // success
    world.force_roll(0); // index 0 → step 1
    do_enchant(&mut world);
    assert_eq!(level(&world), 4, "the bottom of the range is +1");

    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&PLAYER)
            .unwrap()
            .count_of(SCROLL),
        3,
        "one scroll per attempt"
    );
}

/// Java's anti-autoenchant guard: pressing Enchant within 2 s of the last
/// window interaction is treated as a bot — punished, and the attempt consumes
/// nothing.
///
/// The heuristic is coarse and deliberately so: `RequestEnchantItem` compares
/// against `AbstractRequest._timestamp`, which the four `RequestEx*Enchant*`
/// packets stamp on their success path, so it measures "time since the player
/// last touched the window" rather than anything about the enchant itself.
#[test]
fn pressing_enchant_within_two_seconds_is_punished_and_costs_nothing() {
    use crate::model::components::EnchantRequest;
    use crate::model::inventory::Inventory;
    const DIST: &str = crate::data::DIST_GAME;
    const PLAYER: i32 = 9805;

    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.data.enchant = crate::data::EnchantData::load_from(DIST);
    world.id_pool = 0x4200_0000..0x4200_0200;
    // Start well clear of tick 0 so "stamped at tick 0" and "never stamped"
    // cannot be confused — the bug this test caught in the first place.
    world.tick = 500;

    let mut rx = ingame_player_access(&mut world, 1, PLAYER, 0);
    drain(&mut rx);
    items::add_inventory_item(&mut world, PLAYER, 955, 3).unwrap();
    items::add_inventory_item(&mut world, PLAYER, 69, 1).unwrap();
    let scroll_oid = item_oid(&world, PLAYER, 955);
    let sword_oid = item_oid(&world, PLAYER, 69);
    let level = |w: &World| {
        w.objects
            .get_component::<Inventory>(&PLAYER)
            .unwrap()
            .by_object_id(sword_oid)
            .map(|it| it.enchant_level)
            .unwrap()
    };
    let scrolls_left = |w: &World| {
        w.objects
            .get_component::<Inventory>(&PLAYER)
            .unwrap()
            .count_of(955)
    };

    let arm = |world: &mut World| {
        let mut w = PacketWriter::new();
        w.write_u8(cop::USE_ITEM);
        w.write_i32(scroll_oid);
        w.write_i32(0);
        on_packet(world, 1, w.into_bytes());
        let mut w = PacketWriter::new();
        w.write_i32(scroll_oid);
        w.write_i32(sword_oid);
        on_packet(
            world,
            1,
            ex_packet(
                cp::ex_opcodes::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM,
                &w.into_bytes(),
            ),
        );
        let mut w = PacketWriter::new();
        w.write_i32(sword_oid);
        on_packet(
            world,
            1,
            ex_packet(
                cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM,
                &w.into_bytes(),
            ),
        );
    };
    let press = |world: &mut World| {
        let mut w = PacketWriter::new();
        w.write_u8(cop::REQUEST_ENCHANT_ITEM);
        w.write_i32(sword_oid);
        w.write_i32(0);
        on_packet(world, 1, w.into_bytes());
    };

    // Straight from arming the window to pressing Enchant: 0 ticks elapsed.
    arm(&mut world);
    drain(&mut rx);
    world.force_roll(0); // would be a guaranteed success
    press(&mut world);

    assert_eq!(level(&world), 0, "the enchant never happened");
    assert_eq!(scrolls_left(&world), 3, "and cost no scroll");
    assert!(
        !world.objects.has_component::<EnchantRequest>(&PLAYER),
        "Java drops the request on this branch, unlike a plain validation error"
    );
    assert!(
        !drain(&mut rx).is_empty(),
        "the punishment's warning line goes out"
    );
    // The forced roll was never reached — the guard returns before the roll.
    assert_eq!(
        world.forced_rolls_len(),
        1,
        "the guard bails before the success roll is drawn"
    );
    world.clear_forced_rolls();

    // One tick short of the window is still a bot. This is what pins the
    // threshold at 2 s rather than "some delay": with only the 0-tick and
    // 20-tick cases below, a guard that fired at 100 ms would pass too.
    arm(&mut world);
    world.tick += 19;
    press(&mut world);
    assert_eq!(level(&world), 0, "19 ticks (1.9 s) is inside the window");
    assert_eq!(scrolls_left(&world), 3, "still no scroll spent");

    // Wait the window out and the identical sequence succeeds, which is what
    // makes the assertions above about the guard rather than about the setup.
    arm(&mut world);
    world.tick += 20;
    world.force_roll(0);
    press(&mut world);
    assert_eq!(level(&world), 1, "past the 2 s window it enchants normally");
    assert_eq!(scrolls_left(&world), 2, "and now a scroll is consumed");
}

// ---------------------------------------------------------------------------
// Item handlers restored with row 6 (`Book`, `RollingDice`, `PetFood`)
// ---------------------------------------------------------------------------

/// `handlers/itemhandlers/Book` — a readable book opens
/// `data/html/help/<itemId>.htm` and is **not** consumed. 31 of the 50 book
/// items on this dist are sold in shops; before this they were eaten in
/// silence.
#[test]
fn a_book_opens_its_help_page_and_survives() {
    let (mut world, ..) = test_world();
    world.data.item_data = dist::items_owned();
    world.data.root = crate::data::DIST_GAME.to_string();
    world.id_pool = 0x4400_0000..0x4400_0200;
    let mut rx = ingame_player(&mut world, 1, 8801, 0, 0, 0);
    const BOOK: i32 = 7100; // "Importance of Strain"
    items::add_inventory_item(&mut world, 8801, BOOK, 1).unwrap();
    let obj = item_oid(&world, 8801, BOOK);
    drain(&mut rx);

    crate::game_loop::items::use_equipable_item(&mut world, 1, 8801, obj);

    let html = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .expect("the page opens");
    assert!(html.len() > 32, "and it carries the file's text");
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&8801)
            .unwrap()
            .count_of(BOOK),
        1,
        "reading a book does not consume it"
    );
}

/// `handlers/itemhandlers/RollingDice` — the die lands in front of the roller
/// and everyone nearby sees the number.
#[test]
fn rolling_a_die_broadcasts_the_result() {
    let (mut world, ..) = test_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4500_0000..0x4500_0200;
    let mut rx = ingame_player(&mut world, 1, 8802, 0, 0, 0);
    let mut bystander = ingame_player(&mut world, 2, 8803, 60, 0, 0);
    const DIE: i32 = 4625; // Dice (Heart)
    items::add_inventory_item(&mut world, 8802, DIE, 1).unwrap();
    let obj = item_oid(&world, 8802, DIE);
    drain(&mut rx);
    drain(&mut bystander);

    crate::game_loop::items::use_equipable_item(&mut world, 1, 8802, obj);

    let rolled = drain(&mut rx);
    let dice = rolled
        .iter()
        .find(|p| p[0] == server_packets::opcodes::DICE)
        .expect("the die is thrown");
    // objectId, itemId, number, x, y, z
    let number = i32::from_le_bytes([dice[9], dice[10], dice[11], dice[12]]);
    assert!((1..=6).contains(&number), "a six-sided die, got {number}");
    assert!(
        rolled
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE),
        "and the roller is told the number"
    );
    assert!(
        drain(&mut bystander)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::DICE),
        "bystanders see it land"
    );
}

// ---------------------------------------------------------------------------
// Enchanted armour's max-HP bonus (measured-gaps row 11)
// ---------------------------------------------------------------------------

/// **`enchantHPBonus.xml` was read by nothing**, so an enchanted set was worth
/// exactly its unenchanted stats. `MaxHpFinalizer` adds a flat per-piece figure
/// on top, keyed on the piece's grade and enchant level.
#[test]
fn enchanted_armour_adds_its_max_hp_bonus() {
    use crate::data::item_data::CrystalType;
    let (mut world, ..) = test_world();
    world.data.item_data = dist::items_owned();
    world.data.enchant_hp_bonus =
        crate::data::EnchantHpBonusData::load_from(crate::data::DIST_GAME);
    world.id_pool = 0x4700_0000..0x4700_0200;
    let _rx = ingame_player(&mut world, 1, 8901, 0, 0, 0);

    // Dark Crystal Leather Armor — an A-grade chest piece: neither jewellery
    // nor a one-piece suit, so it takes the plain arm of the bonus. The grade
    // is read rather than assumed; the point of the test is that the piece's
    // *own* grade row is what gets paid.
    const CHEST: i32 = 2385;
    let (grade, body_part) = {
        let t = world.data.item_data.get(CHEST).expect("a chest piece");
        (t.crystal_type, t.body_part)
    };
    assert_ne!(
        grade,
        CrystalType::None,
        "a graded piece, or there is no row"
    );
    assert_eq!(
        body_part,
        crate::data::item_data::SLOT_CHEST,
        "the plain arm, not the full-armour one"
    );

    items::add_inventory_item(&mut world, 8901, CHEST, 1).unwrap();
    let obj = item_oid(&world, 8901, CHEST);
    crate::game_loop::items::use_equipable_item(&mut world, 1, 8901, obj);

    let max_hp_at = |world: &mut World, enchant: i32| -> f64 {
        if let Some(inv) = world
            .objects
            .get_component_mut::<crate::model::inventory::Inventory>(&8901)
        {
            inv.set_enchant_level(obj, enchant);
        }
        crate::game_loop::helpers::recalculate_player_stats_and_vitals(world, 8901);
        world
            .objects
            .get_component::<crate::model::components::Vitals>(&8901)
            .unwrap()
            .max_hp as f64
    };

    let plain = max_hp_at(&mut world, 0);
    let plus4 = max_hp_at(&mut world, 4);
    let plus12 = max_hp_at(&mut world, 12);

    let expected4 = world.data.enchant_hp_bonus.bonus(grade, 4, body_part);
    assert!(expected4 > 0.0, "the shipped table has a +4 B-grade figure");
    assert_eq!(
        plus4 - plain,
        expected4,
        "a +4 piece is worth exactly its table row"
    );
    assert!(plus12 > plus4, "and +12 more than +4");
}

/// Java excludes necklace, earrings and rings by **body part** — `ItemKind`
/// calls them armour too, so testing the kind alone would pay a bonus on a
/// +12 ring.
#[test]
fn enchanted_jewellery_pays_no_hp_bonus() {
    let (mut world, ..) = test_world();
    world.data.item_data = dist::items_owned();
    world.data.enchant_hp_bonus =
        crate::data::EnchantHpBonusData::load_from(crate::data::DIST_GAME);
    world.id_pool = 0x4800_0000..0x4800_0200;
    let _rx = ingame_player(&mut world, 1, 8902, 0, 0, 0);

    // Necklace of Mermaid — B-grade, `SLOT_NECK`.
    const NECKLACE: i32 = 916;
    let neck_slot = world
        .data
        .item_data
        .get(NECKLACE)
        .expect("a B-grade neck")
        .body_part;
    assert_eq!(
        neck_slot,
        crate::data::item_data::SLOT_NECK,
        "slot assumption"
    );

    items::add_inventory_item(&mut world, 8902, NECKLACE, 1).unwrap();
    let obj = item_oid(&world, 8902, NECKLACE);
    crate::game_loop::items::use_equipable_item(&mut world, 1, 8902, obj);
    crate::game_loop::helpers::recalculate_player_stats_and_vitals(&mut world, 8902);
    let plain = world
        .objects
        .get_component::<crate::model::components::Vitals>(&8902)
        .unwrap()
        .max_hp;

    if let Some(inv) = world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&8902)
    {
        inv.set_enchant_level(obj, 12);
    }
    crate::game_loop::helpers::recalculate_player_stats_and_vitals(&mut world, 8902);
    let enchanted = world
        .objects
        .get_component::<crate::model::components::Vitals>(&8902)
        .unwrap()
        .max_hp;

    assert_eq!(plain, enchanted, "a +12 necklace grants no HP");
}
