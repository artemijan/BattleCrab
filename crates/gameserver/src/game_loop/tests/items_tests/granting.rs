//! Items that produce other items: extractable packs and the `GiveItem` /
//! `GiveItemRandom` item skills, including the weighted groups and the
//! enchant rolled onto what they create.

use super::*;

/// The bug this guards: `UseItem` on a non-equipable `EtcItem` used to be a
/// silent no-op (`is_equipable() == false` → early return before any handler
/// dispatch existed), so pack/box items like "Mage Class Equipment Set"
/// never unpacked in-game. `ExtractableItems` should destroy the pack and
/// grant its `<capsuled_items>` contents.
#[test]
fn extractable_pack_item_unpacks_into_its_contents() {
    use crate::data::item_data::kinds::{ItemHandler, ItemKind};
    use crate::data::item_data::template::{CapsuledItem, ItemTemplate};
    use crate::model::inventory::Inventory;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

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
        crystal_type: crate::data::item_data::kinds::CrystalType::None,
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
        etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
    for item_id in [15230, 15270] {
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
    use crate::data::item_data;
    use crate::data::item_data::kinds::{ItemHandler, ItemKind};
    use crate::data::item_data::template::{CapsuledItem, ItemTemplate};
    use crate::model::inventory::Inventory;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

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
        default_action: item_data::kinds::ActionType::Other,
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
        crystal_type: item_data::kinds::CrystalType::None,
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
        etc_item_type: item_data::kinds::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
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
        default_action: item_data::kinds::ActionType::Other,
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
        crystal_type: item_data::kinds::CrystalType::None,
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
        etc_item_type: item_data::kinds::EtcItemType::Other,
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
    use crate::data::item_data::kinds::{ItemHandler, ItemKind};
    use crate::data::item_data::template::{CapsuledItem, ItemTemplate};
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
        pre_conditions: Vec::new(),
        is_oly_restricted: false,
        is_event_restricted: false,
        for_npc: false,
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::kinds::ActionType::Other,
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
        crystal_type: crate::data::item_data::kinds::CrystalType::None,
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
        etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
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

/// The bug this guards: a `Restoration`-effect skill (e.g. the "Mysterious
/// Blessed Spiritshot Pack" line, item 22599 → skill 22490) used to parse
/// with an empty effect list — `SkillEffect::GiveItem`/`GiveItemRandom`
/// didn't exist yet — so `use_item_skills` still consumed the pack (a skill
/// was found and "cast") but granted nothing: the pack just disappeared.
#[test]
fn item_skill_give_item_grants_reward_and_consumes_pack() {
    use crate::data::item_data::kinds::{ItemHandler, ItemKind};
    use crate::data::item_data::template::ItemTemplate;
    use crate::model::inventory::Inventory;
    use crate::model::skill::effects::SkillEffect;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        without_action: false,
        trait_type: model::skill::traits::TraitType::None,
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
        pre_conditions: Vec::new(),
        is_oly_restricted: false,
        is_event_restricted: false,
        for_npc: false,
        time: -1,
        duration: -1,
        immediate_effect: true,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::kinds::ActionType::SkillReduce,
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
        item_skills: vec![(22490, 5)],
        etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    });
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
    use crate::data::item_data::kinds::{ItemHandler, ItemKind};
    use crate::data::item_data::template::ItemTemplate;
    use crate::model::inventory::Inventory;
    use crate::model::skill::effects::{RestorationGroup, RestorationItem, SkillEffect};
    use crate::model::skill::target::{AffectObject, AffectScope};

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
        trait_type: model::skill::traits::TraitType::None,
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
        pre_conditions: Vec::new(),
        is_oly_restricted: false,
        is_event_restricted: false,
        for_npc: false,
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::kinds::ActionType::Other,
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
        default_action: crate::data::item_data::kinds::ActionType::SkillReduce,
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
        item_skills: vec![(323, 1)],
        etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
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
    use crate::data::item_data::kinds::{ItemHandler, ItemKind};
    use crate::data::item_data::template::ItemTemplate;
    use crate::model::inventory::Inventory;
    use crate::model::skill::effects::{RestorationGroup, RestorationItem, SkillEffect};

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
        trait_type: model::skill::traits::TraitType::None,
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
        pre_conditions: Vec::new(),
        is_oly_restricted: false,
        is_event_restricted: false,
        for_npc: false,
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::kinds::ActionType::Other,
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
        default_action: crate::data::item_data::kinds::ActionType::SkillReduce,
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
        item_skills: vec![(324, 1)],
        etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
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
