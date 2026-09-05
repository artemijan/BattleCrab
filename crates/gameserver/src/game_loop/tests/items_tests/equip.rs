//! Equipping: swapping gear and repainting the paperdoll, and what a piece
//! and its enchant do to combat stats, max HP/MP and attack range.

use super::*;

/// Equipping gear during a cast is deferred to cast end (Java `UseItem`'s
/// `setNextAction(NextAction(EVT_FINISH_CASTING, …))`), silently — no packet
/// at click time, the equip lands when the cast stops.
#[test]
fn equip_click_during_cast_is_deferred_to_cast_end() {
    use crate::model::components::combat::QueuedAction;
    use crate::model::inventory::Inventory;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    world
        .data
        .item_data
        .insert_for_test(crate::data::item_data::template::ItemTemplate {
            trade_flags: Default::default(),
            pre_conditions: Vec::new(),
            is_oly_restricted: false,
            is_event_restricted: false,
            for_npc: false,
            time: -1,
            duration: -1,
            immediate_effect: false,
            ex_immediate_effect: false,
            default_action: crate::data::item_data::kinds::ActionType::Other,
            item_id: 2,
            name: "Test Sword".into(),
            kind: crate::data::item_data::kinds::ItemKind::Weapon,
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
            handler: crate::data::item_data::kinds::ItemHandler::None,
            crystal_type: crate::data::item_data::kinds::CrystalType::None,
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
            etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
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
            .insert_for_test(crate::data::item_data::template::ItemTemplate {
                trade_flags: Default::default(),
                pre_conditions: Vec::new(),
                is_oly_restricted: false,
                is_event_restricted: false,
                for_npc: false,
                time: -1,
                duration: -1,
                immediate_effect: false,
                ex_immediate_effect: false,
                default_action: crate::data::item_data::kinds::ActionType::Other,
                item_id: id,
                name: format!("earring{id}"),
                kind: crate::data::item_data::kinds::ItemKind::Armor,
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
                handler: crate::data::item_data::kinds::ItemHandler::None,
                crystal_type: crate::data::item_data::kinds::CrystalType::None,
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
                etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
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
    use crate::data::item_data::SLOT_R_HAND;
    use crate::data::item_data::kinds::{CrystalType, ItemHandler, ItemKind};
    use crate::data::item_data::template::ItemTemplate;
    use crate::enums::InventorySlot;
    use crate::model::inventory::Inventory;

    const SWORD: i32 = 3029;
    const SWORD_OID: i32 = 9101;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        pre_conditions: Vec::new(),
        is_oly_restricted: false,
        is_event_restricted: false,
        for_npc: false,
        time: -1,
        duration: -1,
        immediate_effect: true,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::kinds::ActionType::Other,
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
        etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
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
    let taken = inventory::take_items(&mut world, 1, 3001, SWORD, -1);
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
    use crate::data::item_data::kinds::{CrystalType, ItemHandler, ItemKind};
    use crate::data::item_data::template::{ItemStats, ItemTemplate};
    use crate::data::item_data::{SLOT_CHEST, SLOT_R_HAND};
    use crate::model::inventory::Inventory;
    use crate::model::stats::Stat;

    let (mut world, ..) = cast_test_world();
    let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let template = |item_id: i32, kind: ItemKind, body_part: i32| ItemTemplate {
        trade_flags: Default::default(),
        pre_conditions: Vec::new(),
        is_oly_restricted: false,
        is_event_restricted: false,
        for_npc: false,
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::kinds::ActionType::Other,
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
        etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
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

/// `P/MEvasionRateFinalizer` end on
/// `validateValue(creature, …, Double.NEGATIVE_INFINITY, Config.MAX_EVASION)` —
/// a **ceiling with no floor**.
///
/// Both halves matter here. Evasion may go **negative**: 309 skills on this dist
/// carry a `PhysicalEvasion` effect and the largest is −60, more than a
/// low-level character's entire base, so a 0 floor would hand them evasion the
/// debuff was supposed to take. And **magic** evasion runs through the same
/// ceiling as the physical one, which a buffed level-80 caster can reach.
#[test]
fn evasion_may_go_negative_but_never_past_the_ceiling() {
    use crate::model::components::stats::StatModifiers;
    use crate::model::stats::Stat;

    let (mut world, ..) = cast_test_world();
    let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let base_evasion = pcs(&world, 3001).evasion;
    let base_magic_evasion = pcs(&world, 3001).magic_evasion;
    assert!(base_evasion > 0, "the fixture starts with some evasion");

    let set_mods = |world: &mut World, evasion: f64, magic: f64| {
        let mut mods = world
            .objects
            .get_component::<StatModifiers>(&3001)
            .cloned()
            .unwrap_or_default();
        mods.add.insert(Stat::EvasionRate, evasion);
        mods.add.insert(Stat::MagicEvasionRate, magic);
        world.objects.add_components(&3001, mods);
        crate::game_loop::helpers::recalculate_player_stats(world, 3001);
    };

    // A debuff bigger than the whole base drives it under zero rather than to it.
    // The exact landing point is ±1 because the *stored* base is truncated
    // toward zero (`as i32`) while the finalizer works in f64, so this asserts
    // the band rather than a single number — the point is the sign.
    set_mods(&mut world, -(base_evasion as f64) - 40.0, 0.0);
    let sunk = pcs(&world, 3001).evasion;
    assert!(
        (-41..=-39).contains(&sunk),
        "no floor — Java's minValue is NEGATIVE_INFINITY, got {sunk}"
    );

    // And both stats stop at `MaxEvasion` (250 on this dist).
    set_mods(&mut world, 10_000.0, 10_000.0);
    assert_eq!(pcs(&world, 3001).evasion, 250, "the physical ceiling holds");
    assert_eq!(
        pcs(&world, 3001).magic_evasion,
        250,
        "and the magic one runs through the same `validateValue`"
    );

    set_mods(&mut world, 0.0, 0.0);
    assert_eq!(pcs(&world, 3001).evasion, base_evasion);
    assert_eq!(pcs(&world, 3001).magic_evasion, base_magic_evasion);
}

/// `IStatFunction.calcEnchantedItemBonus` — **enchanting gear raises its stats**,
/// on a curve that pays triple past +3:
///
/// ```java
/// // calcEnchantedPAtkBonus, S-grade, two-handed non-polearm melee
/// return (6 * enchant) + (12 * Math.max(0, enchant - 3));
/// // calcEnchantDefBonus, every grade Interlude ships
/// return enchant + (3 * Math.max(0, enchant - 3));
/// ```
///
/// Java folds it into the weapon base *before* STR and the level mod, so this
/// asserts the **ratio** against the un-enchanted swing rather than an absolute
/// number — the multipliers cancel and the table is what is left.
#[test]
fn enchanting_gear_raises_attack_and_defence() {
    use crate::data::item_data::kinds::{CrystalType, ItemHandler, ItemKind, WeaponType};
    use crate::data::item_data::template::{ItemStats, ItemTemplate};
    use crate::data::item_data::{SLOT_CHEST, SLOT_LR_HAND};
    use crate::model::inventory::Inventory;
    use crate::model::stats::Stat;

    const SWORD: i32 = 530;
    const ARMOUR: i32 = 531;
    const SWORD_OID: i32 = 9101;
    const ARMOUR_OID: i32 = 9102;
    const WEAPON_P_ATK: f64 = 500.0;
    const ARMOUR_P_DEF: f64 = 300.0;

    let (mut world, ..) = cast_test_world();
    let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let template = |item_id: i32, kind: ItemKind, body_part: i32| ItemTemplate {
        trade_flags: Default::default(),
        pre_conditions: Vec::new(),
        is_oly_restricted: false,
        is_event_restricted: false,
        for_npc: false,
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::kinds::ActionType::Other,
        item_id,
        name: format!("enchant{item_id}"),
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
        // S-grade: the top arm of both weapon tables that Interlude can reach.
        crystal_type: CrystalType::S,
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
        etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
        enchant_enabled: true,
        enchant_limit: 0,
        is_magic_weapon: false,
    };

    world
        .data
        .item_data
        .insert_for_test(template(SWORD, ItemKind::Weapon, SLOT_LR_HAND));
    // A two-handed **sword**: `SLOT_LR_HAND && itemType != POLE` is the arm that
    // pays 6/12 rather than the one-handed 5/10.
    world
        .data
        .item_data
        .set_weapon_type_for_test(SWORD, WeaponType::Sword);
    world.data.item_data.set_item_stats_for_test(
        SWORD,
        ItemStats {
            bonuses: vec![(Stat::PhysicalAttack, WEAPON_P_ATK)],
            ..Default::default()
        },
    );
    world
        .data
        .item_data
        .insert_for_test(template(ARMOUR, ItemKind::Armor, SLOT_CHEST));
    world.data.item_data.set_item_stats_for_test(
        ARMOUR,
        ItemStats {
            bonuses: vec![(Stat::PhysicalDefence, ARMOUR_P_DEF)],
            ..Default::default()
        },
    );
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, SWORD_OID, SWORD, 1);
        inv.add_item(&data.item_data, ARMOUR_OID, ARMOUR, 1);
    }
    items::handle_use_item(&mut world, 1, &use_item_body(SWORD_OID));
    items::handle_use_item(&mut world, 1, &use_item_body(ARMOUR_OID));

    let plain_p_atk = pcs(&world, 3001).p_atk;
    let plain_p_def = pcs(&world, 3001).p_def;
    assert!(plain_p_atk > 0.0 && plain_p_def > 0.0, "the gear is on");

    let enchant_to = |world: &mut World, level: i32| {
        if let Some(inv) = world.objects.get_component_mut::<Inventory>(&3001) {
            inv.set_item_enchant(SWORD_OID, level);
            inv.set_item_enchant(ARMOUR_OID, level);
        }
        crate::game_loop::helpers::recalculate_player_stats(world, 3001);
    };

    // +3 is still on the cheap half of both curves: 6·3 = 18 P.Atk, 3 P.Def.
    enchant_to(&mut world, 3);
    let p_atk_3 = pcs(&world, 3001).p_atk;
    assert!(
        ((p_atk_3 / plain_p_atk) - (WEAPON_P_ATK + 18.0) / WEAPON_P_ATK).abs() < 1e-9,
        "+3 S-grade two-hander adds 6·3 = 18 P.Atk (ratio {})",
        p_atk_3 / plain_p_atk
    );
    assert!(
        ((pcs(&world, 3001).p_def / plain_p_def) - (ARMOUR_P_DEF + 3.0) / ARMOUR_P_DEF).abs()
            < 1e-9,
        "+3 armour adds a flat 3 P.Def"
    );

    // +6 crosses the +3 wall: 6·6 + 12·3 = 72 P.Atk, 6 + 3·3 = 15 P.Def.
    enchant_to(&mut world, 6);
    assert!(
        ((pcs(&world, 3001).p_atk / plain_p_atk) - (WEAPON_P_ATK + 72.0) / WEAPON_P_ATK).abs()
            < 1e-9,
        "past +3 each level pays triple — 6·6 + 12·3 = 72"
    );
    assert!(
        ((pcs(&world, 3001).p_def / plain_p_def) - (ARMOUR_P_DEF + 15.0) / ARMOUR_P_DEF).abs()
            < 1e-9,
        "and the defence curve steps the same way — 6 + 3·3 = 15"
    );

    // `ShotsBonusFinalizer` rides the same enchant: `1 + level·0.003`.
    assert!(
        (pcs(&world, 3001).shots_bonus() - 1.018).abs() < 1e-12,
        "a +6 weapon lifts every soulshot by 1.8 %, got {}",
        pcs(&world, 3001).shots_bonus()
    );

    // Unenchanted again → both fall back exactly.
    enchant_to(&mut world, 0);
    assert!((pcs(&world, 3001).p_atk - plain_p_atk).abs() < 1e-9);
    assert!((pcs(&world, 3001).p_def - plain_p_def).abs() < 1e-9);
    assert_eq!(pcs(&world, 3001).shots_bonus(), 1.0);
}

/// Companion to the combat-stat test: `maxMp` (and `maxHp`) item bonuses live
/// in `Vitals`, computed on a separate path from `recalculate_stats`. Equipping
/// +MP jewelry must raise Max MP; unequipping restores it and clamps current MP.
#[test]
fn equipping_gear_updates_max_hp_mp() {
    use crate::data::item_data::SLOT_NECK;
    use crate::data::item_data::kinds::{CrystalType, ItemHandler, ItemKind};
    use crate::data::item_data::template::{ItemStats, ItemTemplate};
    use crate::model::components::stats::Vitals;
    use crate::model::inventory::Inventory;
    use crate::model::stats::Stat;

    let (mut world, ..) = cast_test_world();
    let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // A necklace granting +100 Max MP.
    world.data.item_data.insert_for_test(ItemTemplate {
        trade_flags: Default::default(),
        pre_conditions: Vec::new(),
        is_oly_restricted: false,
        is_event_restricted: false,
        for_npc: false,
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::kinds::ActionType::Other,
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
        etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
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

    let weapon = crate::db::ItemRow {
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

/// **`enchantHPBonus.xml` was read by nothing**, so an enchanted set was worth
/// exactly its unenchanted stats. `MaxHpFinalizer` adds a flat per-piece figure
/// on top, keyed on the piece's grade and enchant level.
#[test]
fn enchanted_armour_adds_its_max_hp_bonus() {
    use crate::data::item_data::kinds::CrystalType;
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

    inventory::add_inventory_item(&mut world, 8901, CHEST, 1).unwrap();
    let obj = item_oid(&world, 8901, CHEST);
    items::use_equipable_item(&mut world, 1, 8901, obj);

    let max_hp_at = |world: &mut World, enchant: i32| -> f64 {
        if let Some(inv) = world.objects.get_component_mut::<Inventory>(&8901) {
            inv.set_enchant_level(obj, enchant);
        }
        crate::game_loop::helpers::recalculate_player_stats_and_vitals(world, 8901);
        world.objects.get_component::<Vitals>(&8901).unwrap().max_hp as f64
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

    inventory::add_inventory_item(&mut world, 8902, NECKLACE, 1).unwrap();
    let obj = item_oid(&world, 8902, NECKLACE);
    items::use_equipable_item(&mut world, 1, 8902, obj);
    crate::game_loop::helpers::recalculate_player_stats_and_vitals(&mut world, 8902);
    let plain = world.objects.get_component::<Vitals>(&8902).unwrap().max_hp;

    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&8902) {
        inv.set_enchant_level(obj, 12);
    }
    crate::game_loop::helpers::recalculate_player_stats_and_vitals(&mut world, 8902);
    let enchanted = world.objects.get_component::<Vitals>(&8902).unwrap().max_hp;

    assert_eq!(plain, enchanted, "a +12 necklace grants no HP");
}
