//! Using an item for its skill: potions and their reuse, non-immediate
//! casts, reduce-on-success, books and dice.

use super::*;

/// `ItemSkills` (the `handlers/itemhandlers/ItemSkillsTemplate` port): a
/// self-targeted potion heals immediately (no cast bar) and consumes one
/// unit from the stack; a second use inside the skill's reuse window is
/// blocked and doesn't consume another.
#[test]
fn item_skill_potion_heals_and_enforces_reuse() {
    use crate::data::item_data::kinds::{ItemHandler, ItemKind};
    use crate::data::item_data::template::ItemTemplate;
    use crate::model::inventory::Inventory;
    use crate::model::skill::effects::SkillEffect;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        without_action: false,
        trait_type: model::skill::traits::TraitType::None,
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
        pre_conditions: Vec::new(),
        is_oly_restricted: false,
        is_event_restricted: false,
        for_npc: false,
        time: -1,
        duration: -1,
        immediate_effect: true,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::kinds::ActionType::SkillReduce,
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
        item_skills: vec![(2031, 1)],
        etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
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
    use crate::data::item_data::kinds::{ItemHandler, ItemKind};
    use crate::data::item_data::template::ItemTemplate;
    use crate::model::components::combat::Casting;
    use crate::model::inventory::Inventory;
    use crate::model::skill::effects::SkillEffect;

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
        trait_type: model::skill::traits::TraitType::None,
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
            dest: model::skill::effects::EscapeDest::Town,
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
        default_action: crate::data::item_data::kinds::ActionType::SkillReduce,
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
        item_skills: vec![(2013, 1)],
        etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
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

/// **A `SKILL_REDUCE_ON_SKILL_SUCCESS` item is spent when the cast lands, not
/// when it is used.** Java's `SkillCaster.finishSkill` destroys the triggering
/// item; the port used to take it up front (a documented safe-side deviation),
/// so an interrupted cast still cost the item. Interlude's pair is 8058/8060
/// (Lockup Research Report / Key of Enigma → skill 2260).
#[test]
fn skill_reduce_on_success_item_is_spent_only_when_the_cast_lands() {
    use crate::data::item_data::kinds::{ItemHandler, ItemKind};
    use crate::data::item_data::template::ItemTemplate;
    use crate::model::components::combat::Casting;
    use crate::model::inventory::Inventory;
    use crate::model::skill::Skill;
    use crate::model::skill::target::{AffectObject, AffectScope, OperateType, TargetType};

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
        trait_type: model::skill::traits::TraitType::None,
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
        pre_conditions: Vec::new(),
        is_oly_restricted: false,
        is_event_restricted: false,
        for_npc: false,
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::kinds::ActionType::SkillReduceOnSkillSuccess,
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
        item_skills: vec![(2260, 1)],
        etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
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
    inventory::add_inventory_item(&mut world, 8801, BOOK, 1).unwrap();
    let obj = item_oid(&world, 8801, BOOK);
    drain(&mut rx);

    items::use_equipable_item(&mut world, 1, 8801, obj);

    let html = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .expect("the page opens");
    assert!(html.len() > 32, "and it carries the file's text");
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&8801)
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
    inventory::add_inventory_item(&mut world, 8802, DIE, 1).unwrap();
    let obj = item_oid(&world, 8802, DIE);
    drain(&mut rx);
    drain(&mut bystander);

    items::use_equipable_item(&mut world, 1, 8802, obj);

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
