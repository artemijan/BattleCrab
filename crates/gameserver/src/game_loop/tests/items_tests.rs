use super::*;

/// A heal on another player: Heal.java's `power + sqrt(2·mAtk)` amount,
/// overheal-clamped, SM 1067 to the healed target.
#[test]
fn heal_on_other_restores_hp_with_formula() {
    let (mut world, ..) = cast_test_world();
    let mut _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    world.objects.get_component_mut::<Vitals>(&3002).unwrap().cur_hp = 50.0;
    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut b_rx);

    // TARGET-type skills need no ctrl.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
    assert!(world.objects.has_component::<Casting>(&3001));
    drain(&mut b_rx); // ExRotation + MagicSkillUse

    advance_ticks(&mut world, 10); // hit 500 ms + cancel 500 ms

    let heal = formulas::calc_heal(83.0, pcs(&world, 3001).m_atk, false, false, false, 0, false);
    assert!(heal > 50.0, "sanity: heal ({heal}) overflows the missing 50 HP");
    assert_eq!(pvit(&world, 3002).cur_hp, 100.0, "overheal clamped at max HP");
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);
    assert_eq!(sm_id(&b_rx.try_recv().unwrap()), server_packets::sm_ids::S2_HP_HAS_BEEN_RESTORED_BY_C1);
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE);
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
    world.data.item_data.insert_for_test(crate::data::item_data::ItemTemplate {
        item_id: 2,
        name: "Test Sword".into(),
        kind: crate::data::item_data::ItemKind::Weapon,
        body_part: crate::data::item_data::SLOT_R_HAND,
        weight: 0,
        is_stackable: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        price: 0,
        handler: crate::data::item_data::ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
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
    assert!(a_rx.try_recv().is_err(), "no packet at click time (Java sends none)");
    assert!(matches!(
        world.objects.get_component::<QueuedAction>(&3001),
        Some(QueuedAction::UseItem { item_object_id: 9001 })
    ));
    {
        let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
        assert!(inv.paperdoll_slot_of(9001).is_none(), "not equipped mid-cast");
    }

    // Cast ends (hit 9500 + finish 500 ms): the equip fires.
    advance_ticks(&mut world, 101);
    assert!(!world.objects.has_component::<QueuedAction>(&3001), "queue consumed");
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(inv.paperdoll_slot_of(9001).is_some(), "sword equipped at cast end");
    let packets = drain(&mut a_rx);
    assert!(!packets.is_empty(), "InventoryUpdate/UserInfo sent with the deferred equip");
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
        world.data.item_data.insert_for_test(crate::data::item_data::ItemTemplate {
            item_id: id,
            name: format!("earring{id}"),
            kind: crate::data::item_data::ItemKind::Armor,
            body_part: crate::data::item_data::SLOT_L_EAR | crate::data::item_data::SLOT_R_EAR,
            weight: 0,
            is_stackable: false,
            type1: 0,
            type2: 0,
            is_quest_item: false,
            price: 0,
            handler: crate::data::item_data::ItemHandler::None,
            crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
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
    assert_eq!((rear_oid, lear_oid, lear_iid), (0, 9001, 501), "first earring lands in LEar");

    // Second earring: fills the free REar slot, LEar untouched.
    items::handle_use_item(&mut world, 1, &use_item_body(9002));
    let packets = drain(&mut a_rx);
    let (rear_oid, rear_iid, lear_oid, lear_iid) = ear_slots(&packets);
    assert_eq!((rear_oid, rear_iid, lear_oid, lear_iid), (9002, 502, 9001, 501), "second earring lands in REar, first stays put");

    // Clicking an *already-equipped* earring toggles it back off. Java
    // resolves this via `getSlotFromItem` (the single-bit slot the item
    // currently occupies), not the item's raw (combined, for ears/fingers)
    // template body part — passing the latter used to silently no-op.
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    let packets = drain(&mut a_rx);
    assert!(!packets.is_empty(), "unequip-via-click must send packets, not silently no-op");
    let (rear_oid, rear_iid, lear_oid, _lear_iid) = ear_slots(&packets);
    assert_eq!((rear_oid, rear_iid, lear_oid), (9002, 502, 0), "LEar cleared, REar untouched");
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(inv.paperdoll_slot_of(9001).is_none(), "first earring actually unequipped");
}

/// The bug this guards: equipping gear moved the paperdoll but never recomputed
/// combat stats, so a freshly-equipped weapon's P.Atk / armor's P.Def never
/// reached the client's stat panel. `finish_equip_change` now reruns
/// `recalculate_stats`, and the weapon's stat *replaces* the naked base while
/// armor's *sums* on top (matching the Java finalizers).
#[test]
fn equipping_gear_updates_combat_stats() {
    use crate::data::item_data::{CrystalType, ItemHandler, ItemKind, ItemStats, ItemTemplate, SLOT_CHEST, SLOT_R_HAND};
    use crate::model::inventory::Inventory;
    use crate::model::stats::Stat;

    let (mut world, ..) = cast_test_world();
    let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let template = |item_id: i32, kind: ItemKind, body_part: i32| ItemTemplate {
        item_id,
        name: format!("gear{item_id}"),
        kind,
        body_part,
        weight: 0,
        is_stackable: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: CrystalType::None, crystal_count: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    };
    // Weapon P.Atk 500 (well above the class base of 100, so equip must raise
    // P.Atk); chest armor P.Def 30 (class base P.Def is 0, so it must appear).
    world.data.item_data.insert_for_test(template(500, ItemKind::Weapon, SLOT_R_HAND));
    world.data.item_data.set_item_stats_for_test(500, ItemStats { bonuses: vec![(Stat::PhysicalAttack, 500.0)], ..Default::default() });
    world.data.item_data.insert_for_test(template(510, ItemKind::Armor, SLOT_CHEST));
    world.data.item_data.set_item_stats_for_test(510, ItemStats { bonuses: vec![(Stat::PhysicalDefence, 30.0)], ..Default::default() });
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
    assert!(pcs(&world, 3001).p_atk > base_p_atk, "equipping a weapon must raise P.Atk (was {base_p_atk}, now {})", pcs(&world, 3001).p_atk);

    // Equip the armor → P.Def rises by its contribution, P.Atk unchanged.
    let after_weapon_p_atk = pcs(&world, 3001).p_atk;
    items::handle_use_item(&mut world, 1, &use_item_body(9002));
    assert!(pcs(&world, 3001).p_def > base_p_def, "equipping armor must raise P.Def (was {base_p_def}, now {})", pcs(&world, 3001).p_def);
    assert_eq!(pcs(&world, 3001).p_atk, after_weapon_p_atk, "armor doesn't touch P.Atk");

    // Unequip the weapon → P.Atk falls back to the naked value.
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    assert_eq!(pcs(&world, 3001).p_atk, base_p_atk, "unequipping the weapon restores naked P.Atk");
}

/// Companion to the combat-stat test: `maxMp` (and `maxHp`) item bonuses live
/// in `Vitals`, computed on a separate path from `recalculate_stats`. Equipping
/// +MP jewelry must raise Max MP; unequipping restores it and clamps current MP.
#[test]
fn equipping_gear_updates_max_hp_mp() {
    use crate::data::item_data::{CrystalType, ItemHandler, ItemKind, ItemStats, ItemTemplate, SLOT_NECK};
    use crate::model::components::Vitals;
    use crate::model::inventory::Inventory;
    use crate::model::stats::Stat;

    let (mut world, ..) = cast_test_world();
    let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // A necklace granting +100 Max MP.
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 520,
        name: "MP Necklace".into(),
        kind: ItemKind::Armor,
        body_part: SLOT_NECK,
        weight: 0,
        is_stackable: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: CrystalType::None, crystal_count: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    });
    world.data.item_data.set_item_stats_for_test(520, ItemStats { bonuses: vec![(Stat::MaxMp, 100.0)], ..Default::default() });
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
    assert!(v.cur_mp <= v.max_mp as f64, "current MP clamped to the lowered max");
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
        item_id: 15195,
        name: "Mage Class Equipment Set".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::ExtractableItems,
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: vec![
            CapsuledItem { item_id: 15230, min: 1, max: 1, chance: 100_000 },
            CapsuledItem { item_id: 15270, min: 1, max: 1, chance: 100_000 },
        ],
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    });
    for item_id in [15230, 15270] {
        world.data.item_data.insert_for_test(ItemTemplate {
            item_id,
            name: format!("Pack Content {item_id}"),
            kind: ItemKind::Etc,
            body_part: 0,
            weight: 0,
            is_stackable: false,
            type1: 4,
            type2: 5,
            is_quest_item: false,
            price: 0,
            handler: ItemHandler::None,
            crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
        });
    }
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 15195, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(inv.items().iter().all(|i| i.item_id != 15195), "pack consumed");
    assert!(inv.items().iter().any(|i| i.item_id == 15230), "first capsule granted");
    assert!(inv.items().iter().any(|i| i.item_id == 15270), "second capsule granted");

    let packets = drain(&mut rx);
    let obtained_count = sm_ids_of(&packets).into_iter().filter(|&id| id == server_packets::sm_ids::YOU_HAVE_OBTAINED_S1).count();
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
        item_id: 15274,
        name: "Jewelry Pack (A-grade)".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::ExtractableItems,
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: vec![CapsuledItem { item_id: 14966, min: 2, max: 2, chance: 100_000 }],
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 14966,
        name: "Majestic Earring of Fortune".into(),
        kind: ItemKind::Armor,
        body_part: item_data::SLOT_LR_EAR,
        weight: 0,
        is_stackable: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 15274, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let earring_oids: Vec<i32> = {
        let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
        inv.items().iter().filter(|i| i.item_id == 14966).map(|i| i.object_id).collect()
    };
    assert_eq!(earring_oids.len(), 2, "two separate earring instances, not one instance with count 2");
    {
        let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
        for oid in &earring_oids {
            assert_eq!(inv.items().iter().find(|i| i.object_id == *oid).unwrap().count, 1, "each instance is a single unit");
        }
    }

    let packets = drain(&mut rx);
    let obtained_two = sm_ids_of(&packets).into_iter().any(|id| id == server_packets::sm_ids::YOU_HAVE_OBTAINED_S2_S1);
    assert!(obtained_two, "message reports the pair as a count-2 grant");

    // Equipping one instance must not touch (or vanish) the other.
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.equip_item(&data.item_data, earring_oids[0]);
    }
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(inv.items().iter().any(|i| i.object_id == earring_oids[1]), "second earring still in the bag, not vanished");
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
    assert_eq!(world.cfg.character.inventory_max_no_dwarf, 80, "test assumes the default 80-slot cap");

    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 15195,
        name: "Mage Class Equipment Set".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::ExtractableItems,
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: vec![CapsuledItem { item_id: 15230, min: 1, max: 1, chance: 100_000 }],
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
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
    assert!(inv.items().iter().any(|i| i.item_id == 15195), "pack not consumed when inventory is full");
    assert!(inv.items().iter().all(|i| i.item_id != 15230), "no capsule granted when inventory is full");

    let packets = drain(&mut rx);
    let full_count = sm_ids_of(&packets).into_iter().filter(|&id| id == server_packets::sm_ids::YOUR_INVENTORY_IS_FULL).count();
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
        id: 2031,
        level: 1,
        name: "Lesser Healing Potion".into(),
        operate_type: OperateType::Active,
        target_type: TargetType::Self_,
        magic_type: 1,
        magic_level: 0,        effect_point: 100,
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
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        effects: vec![SkillEffect::Heal { power: 30.0 }],
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 9910,
        name: "Lesser Healing Potion".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::ItemSkills,
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(2031, 1)], etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
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
        let potion = inv.items().iter().find(|i| i.item_id == 9910).expect("one potion left");
        assert_eq!(potion.count, 1, "one unit consumed");
    }
    let packets = drain(&mut rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::STATUS_UPDATE),
        "heal must push an HP StatusUpdate"
    );
    // Memory-first: no per-use DB write; the remaining stack lives in the
    // Inventory component (asserted below) and persists on the next flush.

    // Second use, same tick: reuse still active, no extra heal or consume.
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    assert_eq!(pvit(&world, 3001).cur_hp, 40.0, "reuse blocks a second heal");
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    let potion = inv.items().iter().find(|i| i.item_id == 9910).expect("still one potion left");
    assert_eq!(potion.count, 1, "reuse blocks a second consume");
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
        id: 22490,
        level: 5,
        name: "Mysterious Spiritshot d 5000".into(),
        operate_type: OperateType::Active,
        target_type: TargetType::Self_,
        magic_type: 2,
        magic_level: 0,        effect_point: 0,
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
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        effects: vec![SkillEffect::GiveItem { item_id: 21852, item_count: 5000, item_enchant_level: 0 }],
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 22599,
        name: "Mysterious Blessed Spiritshot Pack (5000) (D-grade)".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 1000,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::ItemSkills,
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(22490, 5)], etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 21852,
        name: "Blessed Spiritshot: D-grade".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 22599, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(inv.items().iter().all(|i| i.item_id != 22599), "pack consumed");
    let shots = inv.items().iter().find(|i| i.item_id == 21852).expect("5000 Blessed Spiritshots granted, not lost");
    assert_eq!(shots.count, 5000);

    let packets = drain(&mut rx);
    assert!(
        sm_ids_of(&packets).into_iter().any(|id| id == server_packets::sm_ids::YOU_HAVE_OBTAINED_S2_S1),
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
    use crate::model::skill::{AffectObject, AffectScope, RestorationGroup, RestorationItem, SkillEffect};

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    // `apply_skill_effects` rolls a magic-crit check unconditionally before
    // walking the effect list (unused here since this isn't a
    // `MagicalAttack`) — force it out of the queue first, then force the
    // roulette roll: `roll_f64` reads a forced value `v` as `v / 1_000_000`,
    // so 600_000 -> 0.6 -> `100 * 0.6 = 60`, landing in the second slice
    // (30..80) below.
    world.forced_rolls.push_back(0);
    world.forced_rolls.push_back(600_000);

    world.data.skill_data.insert_for_test(Skill {
        id: 323,
        level: 1,
        name: "Quiver of Arrow".into(),
        operate_type: OperateType::Active,
        target_type: TargetType::Self_,
        magic_type: 2,
        magic_level: 0,        effect_point: 0,
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
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        effects: vec![SkillEffect::GiveItemRandom {
            groups: vec![
                RestorationGroup {
                    chance: 30.0,
                    items: vec![RestorationItem { item_id: 1344, count: 700, min_enchant: 0, max_enchant: 0 }],
                },
                RestorationGroup {
                    chance: 50.0,
                    items: vec![RestorationItem { item_id: 1344, count: 1400, min_enchant: 0, max_enchant: 0 }],
                },
                RestorationGroup {
                    chance: 20.0,
                    items: vec![RestorationItem { item_id: 1344, count: 2800, min_enchant: 0, max_enchant: 0 }],
                },
            ],
        }],
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 1344,
        name: "Mithril Arrow".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 9999,
        name: "Quiver of Arrow scroll".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::ItemSkills,
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(323, 1)], etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 9999, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    let arrows = inv.items().iter().find(|i| i.item_id == 1344).expect("arrows granted");
    assert_eq!(arrows.count, 1400, "roll 60 lands in the 30..80 (second) slice");
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
    world.forced_rolls.push_back(0);
    world.forced_rolls.push_back(500_000);
    world.forced_rolls.push_back(1);

    world.data.skill_data.insert_for_test(Skill {
        id: 324,
        level: 1,
        name: "Enchanted Reward".into(),
        operate_type: OperateType::Active,
        target_type: TargetType::Self_,
        magic_type: 2,
        magic_level: 0,        effect_point: 0,
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
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        effects: vec![SkillEffect::GiveItemRandom {
            groups: vec![RestorationGroup {
                chance: 100.0,
                items: vec![RestorationItem { item_id: 6001, count: 1, min_enchant: 3, max_enchant: 5 }],
            }],
        }],
    });
    // The reward is a non-stackable weapon so it carries an enchant.
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 6001,
        name: "Enchanted Blade".into(),
        kind: ItemKind::Weapon,
        body_part: 0,
        weight: 0,
        is_stackable: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 9998,
        name: "Enchanted Reward scroll".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::ItemSkills,
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(324, 1)], etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 9998, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    let blade = inv.items().iter().find(|i| i.item_id == 6001).expect("blade granted");
    assert_eq!(blade.enchant_level, 4, "Rnd.get(3, 5) with forced roll 1 -> +4");
    assert!(
        sm_ids_of(&drain(&mut rx)).contains(&server_packets::sm_ids::YOU_HAVE_OBTAINED_A_S1_S2),
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
        let p = world.objects.get_component_mut::<crate::model::Player>(&5001).unwrap();
        p.exp = 1234;
    }
    world.objects.get_component_mut::<Position>(&5001).unwrap().x = 777;

    handle_request_restart(&mut world, 1);

    // storeMe: the snapshot carries the live (not the loaded) state, and
    // is queued before the character-list reload.
    let save = expect_store_player(&mut db_rx);
    assert_eq!((save.base.object_id, save.base.exp, save.base.x), (5001, 1234, 777));
    match db_rx.try_recv() {
        Ok(db::DbCommand::LoadCharacters { client_id, account }) => {
            assert_eq!((client_id, account.as_str()), (1, "bob"));
        }
        _ => panic!("expected a LoadCharacters DB command after the store"),
    }

    // deleteMe + setConnectionState(AUTHENTICATED) + RestartResponse.TRUE.
    assert_eq!(world.objects.count::<Player>(), 0);
    assert!(matches!(world.clients.get(&1), Some(ClientSession::Authenticated(_))));
    let pkt = out_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::RESTART_RESPONSE);
    assert_eq!(pkt[1], 1, "RestartResponse.TRUE");

    // The reload result lands like any character-list load: InLobby +
    // CharSelectionInfo.
    on_characters_loaded(&mut world, 1, "bob".into(), vec![dummy_char(5001, "P5001")], true);
    assert!(matches!(world.clients.get(&1), Some(ClientSession::InLobby(_))));
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::CHARACTER_SELECTION_INFO);
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
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::LOG_OUT_OK);
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
    let tabs: Vec<_> = pkts.iter().filter(|p| is_ex(p, crate::network::trade::EX_BUY_SELL_LIST)).collect();
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
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{}_Buy 3", NPC_OID + 1)));
    let pkts = drain(&mut rx);
    assert!(!pkts.iter().any(|p| is_ex(p, crate::network::trade::EX_BUY_SELL_LIST)));
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
    world.data.item_data.insert_for_test(shot_template(1463, CrystalType::D, ItemHandler::SoulShots, 2150));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    grant_and_equip(&mut world, 3001, 1, 9500);
    let shot_oid = super::items::add_inventory_item(&mut world, 3001, 1463, 10).unwrap()[0];
    drain(&mut a_rx);

    super::items::use_equipable_item(&mut world, 1, 3001, shot_oid);

    assert!(world.objects.get_component::<Player>(&3001).unwrap().is_charged_shot(ShotType::Soulshots), "soulshot charged");
    assert_eq!(world.objects.get_component::<Inventory>(&3001).unwrap().count_of(1463), 8, "weapon.soulShotCount (2) consumed");
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE), "enable message sent");
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE
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
    world.data.item_data.insert_for_test(shot_template(1464, CrystalType::C, ItemHandler::SoulShots, 2151));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    grant_and_equip(&mut world, 3001, 1, 9500);
    let shot_oid = super::items::add_inventory_item(&mut world, 3001, 1464, 10).unwrap()[0];
    drain(&mut a_rx);

    super::items::use_equipable_item(&mut world, 1, 3001, shot_oid);

    assert!(!world.objects.get_component::<Player>(&3001).unwrap().is_charged_shot(ShotType::Soulshots), "wrong-grade shot not charged");
    assert_eq!(world.objects.get_component::<crate::model::inventory::Inventory>(&3001).unwrap().count_of(1464), 10, "nothing consumed");
}

/// A charged soulshot is spent on the next non-miss melee swing, doubles its
/// damage, and sets the `SHOT_USED` flag (`generateHit`).
#[test]
fn soulshot_consumed_on_hit_doubles_melee_damage() {
    use crate::model::{Player, ShotType};

    fn attack_damage_and_flags(packets: &[Vec<u8>]) -> (i32, i32) {
        let atk = packets.iter().find(|p| p[0] == server_packets::opcodes::ATTACK).expect("Attack broadcast");
        (
            i32::from_le_bytes(atk[13..17].try_into().unwrap()),
            i32::from_le_bytes(atk[17..21].try_into().unwrap()),
        )
    }

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // A non-miss swing consumes five rolls in order: miss(1000), shield-rate(100),
    // shield-perfect(100), crit(100), random-damage(2r+1). Force them so both
    // swings are identical: hit, no shield (the NPC has none anyway), no crit,
    // and rand roll 10 → `rand_roll = 10 - 10 = 0` → random multiplier 1.0.
    const SWING_ROLLS: [i32; 5] = [0, 0, 0, 99, 10];

    // Control swing (no shot): plain hit, no crit.
    world.forced_rolls.extend(SWING_ROLLS);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    let (base_dmg, base_flags) = attack_damage_and_flags(&drain(&mut a_rx));
    assert_eq!(base_flags & 0x08, 0, "no soulshot flag without a charge");

    // Charged swing: identical rolls → exactly double, flag set, shot spent.
    world.objects.get_component_mut::<Player>(&3001).unwrap().charge_shot(ShotType::Soulshots);
    world.forced_rolls.extend(SWING_ROLLS);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    let (ss_dmg, ss_flags) = attack_damage_and_flags(&drain(&mut a_rx));

    assert_eq!(ss_dmg, base_dmg * 2, "soulshot doubles the swing");
    assert_ne!(ss_flags & 0x08, 0, "SHOT_USED flag set");
    assert!(!world.objects.get_component::<Player>(&3001).unwrap().is_charged_shot(ShotType::Soulshots), "shot consumed");
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
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    let skill = world.data.skill_data.get(1177, 1).expect("Wind Strike").clone();
    assert_eq!(skill.magic_type, 1, "test skill must be magic");
    drain(&mut a_rx);

    let start_hp = nvit(&world, npc_oid).cur_hp;
    // Control cast (no shot), non-crit.
    world.forced_rolls.push_back(999_999);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let base = start_hp - nvit(&world, npc_oid).cur_hp;
    assert!(base > 0.0, "control nuke dealt damage");
    world.objects.get_component_mut::<Vitals>(&npc_oid).unwrap().cur_hp = start_hp;

    // Charged spiritshot cast, identical crit roll.
    world.objects.get_component_mut::<Player>(&3001).unwrap().charge_shot(ShotType::Spiritshots);
    world.forced_rolls.push_back(999_999);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let ss = start_hp - nvit(&world, npc_oid).cur_hp;

    assert!((ss - base * 2.0).abs() < 1e-6, "spiritshot doubles magic damage ({ss} vs {base})");
    assert!(!world.objects.get_component::<Player>(&3001).unwrap().is_charged_shot(ShotType::Spiritshots), "spiritshot consumed");
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
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    let skill = world.data.skill_data.get(3, 1).expect("Power Strike").clone();
    assert_eq!(skill.magic_type, 0, "test skill must be physical");
    // Zero the weapon random-damage spread so only the crit roll is consumed
    // and the damage is deterministic.
    world.objects.get_component_mut::<CombatStats>(&3001).unwrap().random_dmg = 0;
    drain(&mut a_rx);

    let start_hp = nvit(&world, npc_oid).cur_hp;
    // Two forced high rolls per cast: the unconditional top-of-cast magic-crit
    // roll (unused for a physical skill) then the physical-skill crit roll —
    // both fail, so damage is the non-crit base.
    // Control cast (no shot).
    world.forced_rolls.extend([999_999, 999_999]);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let base = start_hp - nvit(&world, npc_oid).cur_hp;
    assert!(base > 0.0, "physical skill dealt damage (was a silent no-op before)");
    world.objects.get_component_mut::<Vitals>(&npc_oid).unwrap().cur_hp = start_hp;

    // Charged soulshot cast, identical (failed) crit rolls.
    world.objects.get_component_mut::<Player>(&3001).unwrap().charge_shot(ShotType::Soulshots);
    world.forced_rolls.extend([999_999, 999_999]);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let ss = start_hp - nvit(&world, npc_oid).cur_hp;

    assert!((ss - base * 2.0).abs() < 1e-6, "soulshot doubles physical skill damage ({ss} vs {base})");
    assert!(!world.objects.get_component::<Player>(&3001).unwrap().is_charged_shot(ShotType::Soulshots), "soulshot consumed");
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
    world.data.item_data.insert_for_test(shot_template(1463, CrystalType::D, ItemHandler::SoulShots, 2150));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    grant_and_equip(&mut world, 3001, 1, 9500);
    super::items::add_inventory_item(&mut world, 3001, 1463, 10);
    drain(&mut a_rx);

    // itemId=1463, enable=1, type=0.
    let mut body = Vec::new();
    body.extend_from_slice(&1463i32.to_le_bytes());
    body.extend_from_slice(&1i32.to_le_bytes());
    body.extend_from_slice(&0i32.to_le_bytes());
    super::items::handle_request_auto_soul_shot(&mut world, 1, &body);

    assert!(world.objects.get_component::<Player>(&3001).unwrap().auto_shots.contains(&1463), "item recorded for auto-use");
    assert!(world.objects.get_component::<Player>(&3001).unwrap().is_charged_shot(ShotType::Soulshots), "charged on activation");
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::EX && i16::from_le_bytes(p[1..3].try_into().unwrap()) == server_packets::opcodes::EX_AUTO_SOUL_SHOT),
        "ExAutoSoulShot ack sent"
    );

    // The charge is spent on a hit, and the next attack auto-recharges it.
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // Swing 1 spends the activation charge (no item, just the flag).
    world.forced_rolls.extend([0, 99, 10]);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    drain(&mut a_rx);
    // Swing 2 finds no charge, auto-recharges (spends an item), then spends it:
    // the `SHOT_USED` flag on this swing proves the recharge fed it.
    world.forced_rolls.extend([0, 99, 10]);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    let atk = drain(&mut a_rx).into_iter().find(|p| p[0] == server_packets::opcodes::ATTACK).expect("Attack");
    assert_ne!(i32::from_le_bytes(atk[17..21].try_into().unwrap()) & 0x08, 0, "auto-shot re-charged and was spent on the 2nd swing");
    assert_eq!(
        world.objects.get_component::<crate::model::inventory::Inventory>(&3001).unwrap().count_of(1463),
        8,
        "activation + one auto-recharge consumed two shots"
    );
}

/// `RequestDestroyItem` (0x60) removes `count` of a stackable item and sends an
/// `InventoryUpdate`; a bad object id is a no-op.
#[test]
fn destroy_item_removes_from_inventory() {
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9100, 0);
    drain(&mut rx);

    super::items::add_inventory_item(&mut world, 9100, 57, 1000).expect("adena added");
    let inv = |w: &World| w.objects.get_component::<crate::model::inventory::Inventory>(&9100).unwrap().count_of(57);
    let adena_oid = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&9100)
        .unwrap()
        .items()
        .iter()
        .find(|it| it.item_id == 57)
        .unwrap()
        .object_id;

    let destroy = |oid: i32, count: i64| -> Vec<u8> {
        let mut w = PacketWriter::new();
        w.write_u8(cop::REQUEST_DESTROY_ITEM);
        w.write_i32(oid);
        w.write_i64(count);
        w.into_bytes()
    };

    on_packet(&mut world, 1, destroy(adena_oid, 400));
    assert_eq!(inv(&world), 600, "400 adena destroyed");
    assert!(drain(&mut rx).iter().any(|p| p[0] == 0x21), "InventoryUpdate sent");

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
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9100, 0);
    drain(&mut rx);

    crate::game_loop::quests::give_item_with_earned_message(&mut world, 1, 9100, 57, 100_000);

    let pkts = drain(&mut rx);
    assert!(pkts.iter().any(|p| is_ex(p, 0x13E)), "ExAdenaInvenCount (status-bar adena) sent");
    assert!(pkts.iter().any(|p| is_ex(p, 0x166)), "ExUserInfoInvenWeight sent");
    assert_eq!(
        world.objects.get_component::<crate::model::inventory::Inventory>(&9100).unwrap().adena(),
        100_000,
        "adena actually added"
    );
}

/// Drop → the item leaves the inventory and becomes a `GroundItem` world entity
/// (DropItem broadcast); a click (`Action`) picks it back up.
#[test]
fn drop_and_pickup_ground_item() {
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9200, 0);
    drain(&mut rx);
    super::items::add_inventory_item(&mut world, 9200, 57, 1000).expect("adena");
    let count_of = |w: &World| w.objects.get_component::<crate::model::inventory::Inventory>(&9200).unwrap().count_of(57);
    let adena_oid = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&9200)
        .unwrap()
        .items()
        .iter()
        .find(|it| it.item_id == 57)
        .unwrap()
        .object_id;

    // Drop 400 adena.
    let item_oid = world.next_npc_object_id;
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_DROP_ITEM);
    w.write_i32(adena_oid);
    w.write_i64(400);
    w.write_i32(100);
    w.write_i32(200);
    w.write_i32(-3000);
    on_packet(&mut world, 1, w.into_bytes());

    assert_eq!(count_of(&world), 600, "400 left the inventory");
    let g = world.objects.get_component::<crate::model::components::GroundItem>(&item_oid).expect("ground item spawned");
    assert_eq!((g.item_id, g.count), (57, 400));
    assert!(drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::DROP_ITEM), "DropItem broadcast");

    // Click the ground item to pick it up (Action: objectId + origin xyz + action).
    let mut a = PacketWriter::new();
    a.write_u8(cop::ACTION);
    a.write_i32(item_oid);
    a.write_i32(0);
    a.write_i32(0);
    a.write_i32(0);
    a.write_u8(0);
    on_packet(&mut world, 1, a.into_bytes());

    assert_eq!(count_of(&world), 1000, "adena back in the inventory");
    assert!(!world.objects.has_component::<crate::model::components::GroundItem>(&item_oid), "ground item removed");
    assert!(
        !world.ground_item_regions.values().flatten().any(|&id| id == item_oid),
        "ground item de-indexed"
    );
}

/// Give `count` adena to `player_oid` and drop it via `RequestDropItem` at a
/// fixed spot; returns the resulting ground-item object id.
fn drop_adena(world: &mut World, client_id: u32, player_oid: i32, count: i64) -> i32 {
    super::items::add_inventory_item(world, player_oid, 57, count).expect("adena");
    let adena_oid = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&player_oid)
        .unwrap()
        .items()
        .iter()
        .find(|it| it.item_id == 57)
        .unwrap()
        .object_id;
    let item_oid = world.next_npc_object_id;
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_DROP_ITEM);
    w.write_i32(adena_oid);
    w.write_i64(count);
    w.write_i32(100);
    w.write_i32(200);
    w.write_i32(-3000);
    on_packet(world, client_id, w.into_bytes());
    item_oid
}

/// A ground item left un-picked-up auto-destroys after its lifetime
/// (`ItemsOnGroundManager` cleanup) — when General.ini enables it.
#[test]
fn ground_item_decays_after_lifetime() {
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    // Enable player-drop auto-destroy (General.ini `AutoDestroyDroppedItemAfter`
    // + `DestroyPlayerDroppedItem`); the dist default keeps player drops.
    world.cfg.general.autodestroy_item_after = 600;
    world.cfg.general.destroy_dropped_player_item = true;
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let item_oid = drop_adena(&mut world, 1, 9300, 100);
    assert!(world.objects.has_component::<crate::model::components::GroundItem>(&item_oid), "dropped");

    // Jump past the 600 s lifetime and fire the scheduled decay.
    world.tick += 600 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(!world.objects.has_component::<crate::model::components::GroundItem>(&item_oid), "decayed");
    assert!(!world.ground_item_regions.values().flatten().any(|&id| id == item_oid), "de-indexed");
}

/// General.ini parity: with `DestroyPlayerDroppedItem = False` (the dist
/// value), a player's drop is **never** auto-destroyed even when
/// `AutoDestroyDroppedItemAfter > 0` — it persists until pickup/restart.
#[test]
fn player_ground_item_persists_when_destroy_player_dropped_off() {
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.cfg.general.autodestroy_item_after = 600;
    world.cfg.general.destroy_dropped_player_item = false; // dist default
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let item_oid = drop_adena(&mut world, 1, 9300, 100);

    world.tick += 600 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(world.objects.has_component::<crate::model::components::GroundItem>(&item_oid), "player drop persists");
}

/// An NPC drop auto-destroys whenever `AutoDestroyDroppedItemAfter > 0`,
/// independent of the player-drop flag (Java `Npc.dropItem`).
#[test]
fn npc_ground_item_decays_regardless_of_player_flag() {
    use crate::game_loop::ground_items::{spawn_ground_item, DropSource};
    let (mut world, ..) = admin_world();
    world.cfg.general.autodestroy_item_after = 600;
    world.cfg.general.destroy_dropped_player_item = false;
    world.id_pool = 0x4000_0000..0x4000_0100;
    let item_oid = spawn_ground_item(&mut world, 57, 100, 0, 100, 200, -3000, 0, DropSource::Npc);
    assert!(world.objects.has_component::<crate::model::components::GroundItem>(&item_oid), "npc drop on ground");

    world.tick += 600 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(!world.objects.has_component::<crate::model::components::GroundItem>(&item_oid), "npc drop decays");
}

/// Warehouse deposit → the item moves inventory→warehouse; withdraw moves it
/// back; and the save gathers both containers with the right `loc`s (so a
/// deposit survives relog).
#[test]
fn warehouse_deposit_withdraw_and_persist() {
    use crate::model::inventory::{Inventory, Warehouse};
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9400, 0);
    drain(&mut rx);
    super::items::add_inventory_item(&mut world, 9400, 57, 1000).expect("adena");
    let inv_adena = |w: &World| w.objects.get_component::<Inventory>(&9400).unwrap().count_of(57);
    let wh_adena = |w: &World| w.objects.get_component::<Warehouse>(&9400).unwrap().0.count_of(57);
    let adena_oid = world.objects.get_component::<Inventory>(&9400).unwrap().items().iter().find(|it| it.item_id == 57).unwrap().object_id;

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
    let save = super::net::build_save_data(&world, 9400).expect("save");
    let inv_row = save.items.iter().find(|r| r.item_id == 57 && r.loc == "INVENTORY").expect("inv adena row");
    let wh_row = save.items.iter().find(|r| r.item_id == 57 && r.loc == "WAREHOUSE").expect("wh adena row");
    assert_eq!((inv_row.count, wh_row.count), (600, 400));

    // Withdraw 150 back.
    let wh_oid = world.objects.get_component::<Warehouse>(&9400).unwrap().0.items().iter().find(|it| it.item_id == 57).unwrap().object_id;
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
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.id_pool = 0x4000_0000..0x4000_0100;
    // Leather Boots (40): D-grade with a crystal count.
    let cc = world.data.item_data.get(40).unwrap().crystal_count;
    assert!(cc > 0, "test item is crystallizable");

    let mut rx = ingame_player_access(&mut world, 1, 9500, 0);
    drain(&mut rx);
    super::items::add_inventory_item(&mut world, 9500, 40, 1).expect("boots");
    let boots_oid = world.objects.get_component::<Inventory>(&9500).unwrap().items().iter().find(|it| it.item_id == 40).unwrap().object_id;
    let crystallize = |oid: i32| -> Vec<u8> {
        let mut w = PacketWriter::new();
        w.write_u8(cop::REQUEST_CRYSTALLIZE_ITEM);
        w.write_i32(oid);
        w.write_i64(1);
        w.into_bytes()
    };

    // No skill → refused, boots keep.
    on_packet(&mut world, 1, crystallize(boots_oid));
    assert_eq!(world.objects.get_component::<Inventory>(&9500).unwrap().count_of(40), 1, "no skill, no crystallize");

    // Grant Crystallize (248) level 1, then crystallize.
    world.objects.get_component_mut::<crate::model::components::SkillBook>(&9500).unwrap().0.insert(248, 1);
    on_packet(&mut world, 1, crystallize(boots_oid));
    let inv = world.objects.get_component::<Inventory>(&9500).unwrap();
    assert_eq!(inv.count_of(40), 0, "boots crystallized away");
    assert_eq!(inv.count_of(1458), cc as i64, "got crystal_count Crystal (D-grade)");
}

/// A private sell store: the owner sets a list (store activates + store byte),
/// and a buyer purchases — items move seller→buyer, adena buyer→seller.
#[test]
fn private_store_sell_and_buy() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut seller_rx = ingame_player_access(&mut world, 1, 9600, 0);
    let mut buyer_rx = ingame_player_access(&mut world, 2, 9601, 0);
    drain(&mut seller_rx);
    drain(&mut buyer_rx);
    // Seller has 10 Crystal (D); buyer has 1000 adena.
    super::items::add_inventory_item(&mut world, 9600, 1458, 10).unwrap();
    super::items::add_inventory_item(&mut world, 9601, 57, 1000).unwrap();
    let crystal_oid = world.objects.get_component::<Inventory>(&9600).unwrap().items().iter().find(|it| it.item_id == 1458).unwrap().object_id;

    // Seller sets the store: sell 4 crystals at 100 adena each.
    let mut w = PacketWriter::new();
    w.write_u8(cop::SET_PRIVATE_STORE_LIST_SELL);
    w.write_i32(0); // not package
    w.write_i32(1); // one line
    w.write_i32(crystal_oid);
    w.write_i64(4);
    w.write_i64(100);
    on_packet(&mut world, 1, w.into_bytes());
    assert_eq!(world.objects.get_component::<crate::model::Player>(&9600).unwrap().store_type, 1, "store active");
    assert_eq!(world.objects.get_component::<crate::model::components::PrivateStore>(&9600).unwrap().items.len(), 1);

    // Buyer buys all 4.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PRIVATE_STORE_BUY);
    w.write_i32(9600); // seller
    w.write_i32(1);
    w.write_i32(crystal_oid);
    w.write_i64(4);
    w.write_i64(100);
    on_packet(&mut world, 2, w.into_bytes());

    assert_eq!(world.objects.get_component::<Inventory>(&9601).unwrap().count_of(1458), 4, "buyer got 4 crystals");
    assert_eq!(world.objects.get_component::<Inventory>(&9601).unwrap().count_of(57), 600, "buyer paid 400");
    assert_eq!(world.objects.get_component::<Inventory>(&9600).unwrap().count_of(1458), 6, "seller has 6 left");
    assert_eq!(world.objects.get_component::<Inventory>(&9600).unwrap().count_of(57), 400, "seller earned 400");
    // Store emptied of its offered stock → closed.
    assert_eq!(world.objects.get_component::<crate::model::Player>(&9600).unwrap().store_type, 0, "store closed when sold out");
}

/// A full player-to-player trade: request → accept → both add items → both
/// confirm → the offered items swap.
#[test]
fn player_trade_swaps_items() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut a_rx = ingame_player_access(&mut world, 1, 9700, 0);
    let mut b_rx = ingame_player_access(&mut world, 2, 9701, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    super::items::add_inventory_item(&mut world, 9700, 1458, 10).unwrap(); // A: Crystal D
    super::items::add_inventory_item(&mut world, 9701, 1459, 10).unwrap(); // B: Crystal C
    let a_oid = world.objects.get_component::<Inventory>(&9700).unwrap().items().iter().find(|it| it.item_id == 1458).unwrap().object_id;
    let b_oid = world.objects.get_component::<Inventory>(&9701).unwrap().items().iter().find(|it| it.item_id == 1459).unwrap().object_id;
    let one_int = |op: u8, v: i32| { let mut w = PacketWriter::new(); w.write_u8(op); w.write_i32(v); w.into_bytes() };
    let add = |oid: i32, n: i64| { let mut w = PacketWriter::new(); w.write_u8(cop::ADD_TRADE_ITEM); w.write_i32(0); w.write_i32(oid); w.write_i64(n); w.into_bytes() };

    // A requests, B accepts → both in a trade.
    on_packet(&mut world, 1, one_int(cop::TRADE_REQUEST, 9701));
    assert_eq!(world.objects.get_component::<crate::model::components::PendingTrade>(&9701).map(|p| p.from), Some(9700));
    on_packet(&mut world, 2, one_int(cop::ANSWER_TRADE_REQUEST, 1));
    assert_eq!(world.objects.get_component::<crate::model::components::Trade>(&9700).unwrap().partner, 9701);

    // A offers 4 Crystal D, B offers 3 Crystal C.
    on_packet(&mut world, 1, add(a_oid, 4));
    on_packet(&mut world, 2, add(b_oid, 3));
    assert_eq!(world.objects.get_component::<crate::model::components::Trade>(&9700).unwrap().items[0].count, 4);

    // Both confirm → swap.
    on_packet(&mut world, 1, one_int(cop::TRADE_DONE, 1));
    on_packet(&mut world, 2, one_int(cop::TRADE_DONE, 1));

    let a_inv = |w: &World, id: i32| w.objects.get_component::<Inventory>(&9700).unwrap().count_of(id);
    let b_inv = |w: &World, id: i32| w.objects.get_component::<Inventory>(&9701).unwrap().count_of(id);
    assert_eq!((a_inv(&world, 1458), a_inv(&world, 1459)), (6, 3), "A: -4 D, +3 C");
    assert_eq!((b_inv(&world, 1458), b_inv(&world, 1459)), (4, 7), "B: +4 D, -3 C");
    assert!(!world.objects.has_component::<crate::model::components::Trade>(&9700), "trade closed");
    assert!(!world.objects.has_component::<crate::model::components::Trade>(&9701), "trade closed");
}

/// Full enchant flow with real data: use scroll → add scroll → put target →
/// enchant. Success bumps +1; a forced failure at +4 destroys the weapon and
/// returns crystals.
#[test]
fn enchant_scroll_success_and_failure() {
    use crate::model::components::EnchantRequest;
    use crate::model::inventory::Inventory;
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (mut world, ..) = admin_world();
    world.data.item_data = crate::data::ItemData::load_from(DIST);
    world.data.enchant = crate::data::EnchantData::load_from(DIST);
    world.id_pool = 0x4000_0000..0x4000_0200;

    // Scroll: Enchant Weapon (D-grade) 955; Bastard Sword 69 (D weapon, enchantable).
    let sword_cc = world.data.item_data.get(69).unwrap().crystal_count;
    let crystal_id = world.data.item_data.get(69).unwrap().crystal_type.crystal_item_id().unwrap();

    let mut rx = ingame_player_access(&mut world, 1, 9800, 0);
    drain(&mut rx);
    super::items::add_inventory_item(&mut world, 9800, 955, 5).unwrap();
    super::items::add_inventory_item(&mut world, 9800, 69, 1).unwrap();
    let find = |w: &World, item: i32| w.objects.get_component::<Inventory>(&9800).unwrap().items().iter().find(|it| it.item_id == item).map(|it| it.object_id);
    let scroll_oid = find(&world, 955).unwrap();
    let sword_oid = find(&world, 69).unwrap();

    // Use the scroll → opens the enchant request.
    let use_item = { let mut w = PacketWriter::new(); w.write_u8(cop::USE_ITEM); w.write_i32(scroll_oid); w.write_i32(0); w.into_bytes() };
    on_packet(&mut world, 1, use_item);
    assert!(world.objects.has_component::<EnchantRequest>(&9800), "enchant window opened");

    let add_scroll = { let mut w = PacketWriter::new(); w.write_i32(scroll_oid); w.write_i32(sword_oid); w.into_bytes() };
    on_packet(&mut world, 1, ex_packet(cp::ex_opcodes::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM, &add_scroll));
    let put_target = { let mut w = PacketWriter::new(); w.write_i32(sword_oid); w.into_bytes() };
    on_packet(&mut world, 1, ex_packet(cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM, &put_target));

    // +0 weapon is a guaranteed (100%) success → +1.
    let do_enchant = |oid: i32| { let mut w = PacketWriter::new(); w.write_u8(cop::REQUEST_ENCHANT_ITEM); w.write_i32(oid); w.write_i32(0); w.into_bytes() };
    world.forced_rolls.push_back(0); // roll_f64 = 0.0 < 100
    on_packet(&mut world, 1, do_enchant(sword_oid));
    let level = |w: &World| w.objects.get_component::<Inventory>(&9800).unwrap().items().iter().find(|it| it.object_id == sword_oid).map(|it| it.enchant_level);
    assert_eq!(level(&world), Some(1), "success: +0 → +1");
    assert_eq!(world.objects.get_component::<Inventory>(&9800).unwrap().count_of(955), 4, "one scroll consumed");

    // Bump to +4 (66.67% group chance), then force a failing roll (90%) →
    // weapon destroyed, crystals returned.
    world.objects.get_component_mut::<Inventory>(&9800).unwrap().set_item_enchant(sword_oid, 4);
    on_packet(&mut world, 1, ex_packet(cp::ex_opcodes::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM, &add_scroll));
    on_packet(&mut world, 1, ex_packet(cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM, &put_target));
    world.forced_rolls.push_back(900_000); // roll_f64 = 90.0 > 66.67 → fail
    on_packet(&mut world, 1, do_enchant(sword_oid));
    let inv = world.objects.get_component::<Inventory>(&9800).unwrap();
    assert_eq!(inv.count_of(69), 0, "failed enchant destroyed the sword");
    let expected_crystals = (sword_cc - (sword_cc + 1) / 2).max(0) as i64;
    assert_eq!(inv.count_of(crystal_id), expected_crystals, "crystals returned on break");
    assert_eq!(inv.count_of(955), 3, "second scroll consumed");
}

/// Enchant with a support item: its +20 bonus rate flips a roll that would miss
/// the bare 66.67% group chance at +3, and the support is consumed.
#[test]
fn enchant_support_item_bonus_and_consume() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.data.enchant =
        crate::data::EnchantData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9850, 0);
    drain(&mut rx);

    // Bastard Sword 69 (D weapon), Enchant Weapon D scroll 955, and the D-grade
    // weapon support "Lucky Enchant Stone" 12362 (+20 bonus, valid at +3..9).
    super::items::add_inventory_item(&mut world, 9850, 955, 1).unwrap();
    super::items::add_inventory_item(&mut world, 9850, 69, 1).unwrap();
    super::items::add_inventory_item(&mut world, 9850, 12362, 1).unwrap();
    let find = |w: &World, item: i32| w.objects.get_component::<Inventory>(&9850).unwrap().items().iter().find(|it| it.item_id == item).map(|it| it.object_id);
    let (scroll, sword, support) = (find(&world, 955).unwrap(), find(&world, 69).unwrap(), find(&world, 12362).unwrap());
    // The support requires the target already at +3.
    world.objects.get_component_mut::<Inventory>(&9850).unwrap().set_item_enchant(sword, 3);

    let use_scroll = { let mut w = PacketWriter::new(); w.write_u8(cop::USE_ITEM); w.write_i32(scroll); w.write_i32(0); w.into_bytes() };
    on_packet(&mut world, 1, use_scroll);
    let add_scroll = { let mut w = PacketWriter::new(); w.write_i32(scroll); w.write_i32(sword); w.into_bytes() };
    on_packet(&mut world, 1, ex_packet(cp::ex_opcodes::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM, &add_scroll));
    let put_target = { let mut w = PacketWriter::new(); w.write_i32(sword); w.into_bytes() };
    on_packet(&mut world, 1, ex_packet(cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM, &put_target));
    // Support: body is (supportObjId, enchantObjId).
    let put_support = { let mut w = PacketWriter::new(); w.write_i32(support); w.write_i32(sword); w.into_bytes() };
    drain(&mut rx);
    on_packet(&mut world, 1, ex_packet(cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_SUPPORT_ITEM, &put_support));
    let put_out = drain(&mut rx);
    assert!(put_out.iter().any(|p| p.len() >= 3 && p[0] == 0xFE && i16::from_le_bytes([p[1], p[2]]) == server_packets::opcodes::EX_PUT_ENCHANT_SUPPORT_ITEM_RESULT), "support accepted");

    // Roll 80%: bare chance 66.67 would fail, but +20 support → 86.67 succeeds.
    world.forced_rolls.push_back(800_000);
    let enchant = { let mut w = PacketWriter::new(); w.write_u8(cop::REQUEST_ENCHANT_ITEM); w.write_i32(sword); w.write_i32(support); w.into_bytes() };
    on_packet(&mut world, 1, enchant);

    let inv = world.objects.get_component::<Inventory>(&9850).unwrap();
    let level = inv.items().iter().find(|it| it.object_id == sword).unwrap().enchant_level;
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
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9600, 0);
    drain(&mut rx);

    // Seed 300 adena into the freight (as if sent by another character).
    let fr_oid = world.alloc_object_id().unwrap();
    {
        let World { objects, data, .. } = &mut world;
        objects.get_component_mut::<Freight>(&9600).unwrap().0.add_item(&data.item_data, fr_oid, 57, 300);
    }

    // package_withdraw → active = freight, window opens.
    super::warehouse::open_freight_withdraw(&mut world, 1);
    let withdraw = { let mut w = PacketWriter::new(); w.write_u8(cop::SEND_WARE_HOUSE_WITH_DRAW_LIST); w.write_i32(1); w.write_i32(fr_oid); w.write_i64(120); w.into_bytes() };
    on_packet(&mut world, 1, withdraw);

    assert_eq!(world.objects.get_component::<Freight>(&9600).unwrap().0.count_of(57), 180, "180 left in freight");
    assert_eq!(world.objects.get_component::<Inventory>(&9600).unwrap().count_of(57), 120, "120 withdrawn to inventory");

    // Persisted with its own loc alongside inventory + warehouse.
    let save = super::net::build_save_data(&world, 9600).expect("save");
    let fr_row = save.items.iter().find(|r| r.item_id == 57 && r.loc == "FREIGHT").expect("freight row");
    assert_eq!(fr_row.count, 180);
    assert!(save.items.iter().any(|r| r.item_id == 57 && r.loc == "INVENTORY" && r.count == 120), "inventory row");
}

/// Augmentation: confirm the life stone, refine (roll + consume + stamp), then
/// cancel for the adena fee.
#[test]
fn augment_make_and_cancel() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.data.variations =
        crate::data::VariationData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut rx = ingame_player_access(&mut world, 1, 9900, 0);
    drain(&mut rx);

    // Crimson Sword (2551, augmentable D weapon), Life Stone Lv.46 (8723),
    // Gemstone D (2130) ×20, and adena for the cancel fee (95000).
    super::items::add_inventory_item(&mut world, 9900, 2551, 1).unwrap();
    super::items::add_inventory_item(&mut world, 9900, 8723, 1).unwrap();
    super::items::add_inventory_item(&mut world, 9900, 2130, 20).unwrap();
    super::items::add_inventory_item(&mut world, 9900, 57, 200_000).unwrap();
    let oid = |w: &World, item: i32| w.objects.get_component::<Inventory>(&9900).unwrap().items().iter().find(|it| it.item_id == item).unwrap().object_id;
    let (weapon, lifestone, gem) = (oid(&world, 2551), oid(&world, 8723), oid(&world, 2130));

    // Confirm the refiner → the make window echoes the gemstone fee.
    let mut confirm = PacketWriter::new(); confirm.write_i32(weapon); confirm.write_i32(lifestone);
    drain(&mut rx);
    on_packet(&mut world, 1, ex_packet(cp::ex_opcodes::REQUEST_CONFIRM_REFINER_ITEM, &confirm.into_bytes()));
    let confirm_out = drain(&mut rx);
    assert!(confirm_out.iter().any(|p| p.len() >= 3 && p[0] == 0xFE && i16::from_le_bytes([p[1], p[2]]) == server_packets::opcodes::EX_PUT_INTENSIVE_RESULT_FOR_VARIATION_MAKE), "confirm echoes fee");

    // Refine: force low rolls so the augment always resolves.
    world.forced_rolls.extend(std::iter::repeat(0).take(8));
    let mut refine = PacketWriter::new(); refine.write_i32(weapon); refine.write_i32(lifestone); refine.write_i32(gem); refine.write_i64(20);
    on_packet(&mut world, 1, ex_packet(cp::ex_opcodes::REQUEST_REFINE, &refine.into_bytes()));

    let inv = world.objects.get_component::<Inventory>(&9900).unwrap();
    assert!(inv.is_augmented(weapon), "weapon augmented");
    assert_eq!(inv.count_of(8723), 0, "life stone consumed");
    assert_eq!(inv.count_of(2130), 0, "20 gemstones consumed");
    let (o1, o2) = inv.augmentation_of(weapon).unwrap();
    assert!(o1 != 0 && o2 != 0, "two options rolled");

    // Persistence round-trip: the augment rides the item rows (→ item_variations)
    // and restores through `from_rows`.
    let save = super::net::build_save_data(&world, 9900).expect("save");
    let wrow = save.items.iter().find(|r| r.object_id == weapon).expect("weapon row");
    assert_eq!((wrow.augment_mineral, wrow.augment_option1, wrow.augment_option2), (8723, o1, o2), "augment persisted on the row");
    let restored = crate::model::inventory::Inventory::from_rows(&save.items);
    assert_eq!(restored.augmentation_of(weapon), Some((o1, o2)), "augment restored on reload");

    // Cancel: pays the adena fee and strips the augment.
    let mut cancel = PacketWriter::new(); cancel.write_i32(weapon);
    on_packet(&mut world, 1, ex_packet(cp::ex_opcodes::REQUEST_REFINE_CANCEL, &cancel.into_bytes()));
    let inv = world.objects.get_component::<Inventory>(&9900).unwrap();
    assert!(!inv.is_augmented(weapon), "augment removed");
    assert_eq!(inv.count_of(57), 200_000 - 95_000, "adena cancel fee charged");
}
