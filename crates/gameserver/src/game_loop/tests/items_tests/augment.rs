//! Augmenting: making and cancelling one, the options it grants while the
//! item is equipped, and the augment window.

use super::*;

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
    inventory::add_inventory_item(&mut world, 9900, 2551, 1).unwrap();
    inventory::add_inventory_item(&mut world, 9900, 8723, 1).unwrap();
    inventory::add_inventory_item(&mut world, 9900, 2130, 20).unwrap();
    inventory::add_inventory_item(&mut world, 9900, 57, 200_000).unwrap();
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

/// **An augmented weapon's options pump the wearer's stats while it is worn.**
/// Java's equip listener calls `VariationInstance.applyBonus` before the stat
/// recompute, and the unequip listener `removeBonus` — so the two option ids
/// behave like a pair of passive buffs tied to the item.
#[test]
fn augment_options_apply_while_the_item_is_equipped() {
    use crate::data::item_data::SLOT_R_HAND;
    use crate::data::item_data::kinds::{CrystalType, ItemHandler, ItemKind};
    use crate::data::item_data::template::{ItemStats, ItemTemplate};
    use crate::data::option_data::OptionEntry;
    use crate::model::inventory::Inventory;
    use crate::model::skill::effects::StatModifierEffect;
    use crate::model::stats::{Stat, StatModifierType};

    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.id_pool = 0x4200_0000..0x4200_0100;

    // A plain weapon…
    let template = ItemTemplate {
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
        etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
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
            hp_percent: 0,
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
    inventory::add_inventory_item(&mut world, 9910, 2551, 1).unwrap(); // Crimson Sword
    inventory::add_inventory_item(&mut world, 9910, 8723, 1).unwrap(); // Life Stone 46
    inventory::add_inventory_item(&mut world, 9910, 2130, 20).unwrap(); // Gemstone D
    inventory::add_inventory_item(&mut world, 9910, 1458, 1).unwrap(); // Crystal (D)
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
