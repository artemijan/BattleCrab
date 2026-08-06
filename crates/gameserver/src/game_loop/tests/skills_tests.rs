use super::*;
use commons::system_messages::generated::{
    C1_HAS_RESISTED_S2_CHANCE_WAS_S3, S1_LANDED_ON_C2_CHANCE_WAS_S3,
};

/// Arm a test player with `item_id` in the right hand.
///
/// **G34 S1 made this necessary.** Every warrior/dagger skill in the dist
/// carries an `<condition name="EquipWeapon">`, which Java enforces and this
/// port ignored until the condition engine landed — so these fixtures used to
/// cast Sonic Blaster and Lethal Blow bare-handed. Java refuses that, and now
/// so do we; the fixture has to hold the weapon the skill demands.
fn arm(world: &mut World, object_id: i32, item_id: i32) {
    // `world.data` is borrowed immutably while the inventory is borrowed
    // mutably, so the catalog has to come out of the ECS first.
    let mut inv = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&object_id)
        .expect("test player has an inventory")
        .clone();
    let oid = inv.add_item(&world.data.item_data, 0x5000_0001, item_id, 1);
    inv.equip_item(&world.data.item_data, oid);
    world.objects.add_components(&object_id, inv);
}

/// G6 cast-pipeline gate: learn a class skill (SP spend + level gate),
/// cast it, watch the buff land (P.Def +8%) and the right packet sequence
/// go out, then fast-forward the scheduler past `abnormalTime` and watch
/// it expire and P.Def come back down. Runs entirely against a synthetic
/// `World` (no sockets) driven by manually advancing `world.tick` — real
/// time would mean actually waiting out the buff's 20 in-game seconds,
/// which a unit test shouldn't do (PLAN_GAME_SERVER.md §8.5: tick systems
/// are tested against synthetic `World` state, not real time).
#[test]
fn learn_and_cast_buff_skill_applies_and_expires() {
    use crate::model::skill::{AffectObject, AffectScope, Skill, SkillEffect, StatModifierEffect};
    use crate::model::stats::{Stat, StatModifierType};

    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();

    let mut hp_table = vec![0.0; 90];
    let mut mp_table = vec![0.0; 90];
    let mut cp_table = vec![0.0; 90];
    hp_table[5] = 100.0;
    mp_table[5] = 50.0;
    cp_table[5] = 20.0;
    let template = crate::data::player_template::PlayerTemplate {
        class_id: 0,
        base_str: 40,
        base_dex: 30,
        base_con: 43,
        base_int: 21,
        base_wit: 11,
        base_men: 25,
        hp_table,
        mp_table,
        cp_table,
        base_p_def: 80, // naked P.Def, matches the real HumanFighter.xml sum
        ..Default::default()
    };

    let mut data = GameData::for_test();
    data.player_templates = crate::data::PlayerTemplateData::from_vec(vec![template]);
    data.skill_trees.insert_for_test(
        0,
        crate::data::skill_tree::SkillLearn {
            skill_id: 91,
            skill_level: 1,
            name: "Defense Aura".into(),
            get_level: 5,
            level_up_sp: 100,
            auto_get: false,
            required_items: Vec::new(),
        },
    );
    data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        basic_property: crate::model::skill::BasicProperty::None,
        conditions: Vec::new(),
        target_conditions: Vec::new(),
        passive_conditions: Vec::new(),
        without_action: false,
        icon: String::from("icon.skill0000"),
        trait_type: crate::model::skill::TraitType::None,
        static_reuse: false,
        item_consume_id: 0,
        item_consume_count: 0,
        id: 91,
        level: 1,
        name: "Defense Aura".into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Self_,
        magic_type: 1,
        magic_level: 0,
        effect_point: 0,
        cast_range: 0,
        effect_range: 0,
        hit_time: 400,
        next_action: Default::default(),
        abnormal_resists: Vec::new(),
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 2000,
        reuse_delay_group: -1,
        mp_consume: 4,
        mp_initial_consume: 1,
        hp_consume: 0,
        abnormal_time: 20,
        abnormal_level: 1,
        abnormal_type: "PD_UP".into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        fan_range: [0; 4],
        attribute_type: None,
        sub_level: 0,
        attribute_value: 0,
        end_effects: Vec::new(),
        channeling_effects: Vec::new(),
        mp_per_channeling: 0,
        channeling_skill_id: 0,
        channeling_tick_ms: 0,
        channeling_start_ms: 0,
        can_be_dispelled: true,
        is_debuff: false,
        shared_with_summon: true,
        stay_after_death: false,
        removed_on_damage: false,
        self_effects: Vec::new(),
        pve_effects: Vec::new(),
        pvp_effects: Vec::new(),
        effects: vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::PhysicalDefence,
            mode: StatModifierType::Per,
            amount: 8.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
        })],
    });

    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

    // A level-5 character with 200 SP, walked straight to `InGame` (same
    // `Session` transition chain `handle_enter_world` uses in production).
    let mut chr = dummy_char(2001, "Def");
    chr.level = 5;
    chr.sp = 200;
    chr.cur_mp = 50.0;
    let bundle = Player::from_char(&world.data, &chr);
    // Naked P.Def = base(80) × levelMod((5+89)/100 = 0.94) = 75.2 (no gear,
    // so no slot subtraction); stored unrounded, the display truncates to 75.
    assert!(
        (bundle.combat.p_def - 75.2).abs() < 1e-9,
        "naked P.Def before any buff: {}",
        bundle.combat.p_def
    );

    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel();
    let s = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![])
        .into_entering(bundle);
    let (session, bundle) = s.into_ingame();
    bundle.spawn_into(&mut world);
    world.clients.insert(1, ClientSession::InGame(session));

    // --- Learn: RequestAcquireSkill(id=91, level=1, type=CLASS). ---
    let mut w = PacketWriter::new();
    w.write_i32(91);
    w.write_i32(1);
    w.write_i32(cp::RequestAcquireSkill::CLASS);
    handle_request_acquire_skill(&mut world, 1, &w.into_bytes());

    assert_eq!(
        world
            .objects
            .get_component::<SkillBook>(&2001)
            .unwrap()
            .0
            .get(&91),
        Some(&1)
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::Player>(&2001)
            .expect("player")
            .sp,
        100,
        "200 SP - levelUpSp(100)"
    );
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACQUIRE_SKILL_DONE
    );
    assert_eq!(out_rx.try_recv().unwrap()[0], 0x5F); // SkillList
    let _ = out_rx.try_recv().unwrap(); // AcquireSkillList
    let _ = out_rx.try_recv().unwrap(); // UserInfo

    // --- Cast: RequestMagicSkillUse(91). ---
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));

    assert!(world.objects.has_component::<Casting>(&2001));
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    ); // initial MP consume
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_USE
    );
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::SYSTEM_MESSAGE
    ); // YOU_USE_S1
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::SETUP_GAUGE
    );
    assert_eq!(pvit(&world, 2001).cur_mp, 49.0, "50 - mpInitialConsume(1)");

    // --- Launch: hit = max(400/factor(1.0) − cancel(500), 0) = 0 ms, so
    // the launch task is already due; the finish follows 500 ms later.
    apply_due_tasks(&mut world);
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_LAUNCHED
    );
    assert!(
        world
            .objects
            .get_component::<Casting>(&2001)
            .is_some_and(|c| c.0.launched)
    );

    world.tick += 5;
    apply_due_tasks(&mut world);
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    ); // final MP consume
    assert_eq!(out_rx.try_recv().unwrap()[0], 0x85); // AbnormalStatusUpdate
    let _ = out_rx.try_recv().unwrap(); // UserInfo (buff changed pDef → broadcastUserInfo)

    {
        assert!(
            !world.objects.has_component::<Casting>(&2001),
            "coolTime 0 frees the cast slot inline"
        );
        assert_eq!(pbuffs(&world, 2001), 1);
        assert!(
            (pcs(&world, 2001).p_def - 75.2 * 1.08).abs() < 1e-9,
            "75.2 × 1.08 (PhysicalDefence +8%): {}",
            pcs(&world, 2001).p_def
        );
    }
    assert_eq!(pvit(&world, 2001).cur_mp, 45.0, "49 - mpConsume(4)");

    // --- Advance past expiry (abnormalTime 20 s = 200 ticks) and drain again. ---
    world.tick += 200;
    apply_due_tasks(&mut world);

    let _ = out_rx.try_recv().unwrap(); // UserInfo (buff removal reverted pDef → broadcastUserInfo)
    let expired = out_rx.try_recv().unwrap();
    assert_eq!(expired[0], 0x85);
    assert_eq!(
        &expired[1..3],
        &[0, 0],
        "AbnormalStatusUpdate count = 0 once expired"
    );

    assert_eq!(pbuffs(&world, 2001), 0);
    assert!(
        (pcs(&world, 2001).p_def - 75.2).abs() < 1e-9,
        "P.Def restored after the buff expired: {}",
        pcs(&world, 2001).p_def
    );
}

/// Real-data stat parity: a level-1 Human Mystic loaded with the *real* class
/// starting gear (`initialEquipment.xml`, replayed through the equip-slot logic)
/// and *all* the class's level-1 autoGet skills (`skillTrees`), computed the
/// same way enter-world does, must show exactly the numbers the Java client
/// draws — including the Spellcraft-boosted casting speed of 499. Locks in the
/// finalizer fixes (pDef levelMod + slot-sub, mDef MEN×levelMod, RunSpeedBoost,
/// `(int)` truncation) *and* the armor-conditioned passives end to end.
#[test]
fn human_mystic_lvl1_full_loadout_matches_java_client() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    data.skill_trees = crate::data::skill_tree::SkillTreeData::load_from(DIST);
    data.initial_equipment = crate::data::initial_equipment::InitialEquipmentData::load_from(DIST);

    let class_id = 10; // Human Mystic

    // Replay the class starting equipment through the real equip-slot logic
    // (mirrors `resolve_initial_items`), then hand the resolved paperdoll to
    // `from_char` as stored `ItemRow`s.
    let mut inv = crate::model::inventory::Inventory::new();
    let mut next_oid = 1000;
    for entry in data.initial_equipment.get(class_id) {
        let oid = next_oid;
        next_oid += 1;
        inv.add_item(&data.item_data, oid, entry.item_id, entry.count);
        if entry.equipped {
            inv.equip_item(&data.item_data, oid);
        }
    }
    let items: Vec<crate::character::ItemRow> = inv
        .items()
        .iter()
        .map(|it| {
            let slot = inv.paperdoll_slot_of(it.object_id);
            crate::character::ItemRow {
                object_id: it.object_id,
                item_id: it.item_id,
                count: it.count,
                enchant_level: 0,
                loc: if slot.is_some() {
                    "PAPERDOLL".into()
                } else {
                    "INVENTORY".into()
                },
                loc_data: slot.map(|s| s as i32).unwrap_or(0),
                custom_type1: 0,
                custom_type2: 0,
                mana_left: -1,
                time: 0,
                augment_mineral: 0,
                augment_option1: 0,
                augment_option2: 0,
            }
        })
        .collect();

    let mut chr = dummy_char(4212, "Mystic");
    chr.class_id = class_id;
    chr.base_class_id = class_id;
    chr.items = items;
    chr.skills = data
        .skill_trees
        .initial_skills(class_id)
        .into_iter()
        .map(|(id, lvl)| (id, lvl, 0))
        .collect(); // 118, 163, 214, 1177, 1216

    let b = Player::from_char(&data, &chr);
    let c = &b.combat;
    // Displayed via `(int)`/`as i32` truncation, matching the Java client panel.
    assert_eq!(c.p_atk as i32, 2, "p.atk");
    assert_eq!(c.m_atk as i32, 8, "m.atk");
    assert_eq!(c.p_def as i32, 52, "p.def");
    assert_eq!(c.accuracy, 31, "p.accuracy");
    assert_eq!(c.evasion, 23, "p.evasion");
    assert_eq!(c.crit_hit as i32, 60, "p.critical");
    assert_eq!(c.p_atk_spd, 384, "atk speed");
    assert_eq!(b.speeds.run_spd as i32, 159, "run speed");
    assert_eq!(c.m_def as i32, 54, "m.def");
    assert_eq!(c.magic_accuracy, 15, "m.accuracy");
    assert_eq!(c.magic_evasion, 15, "m.evasion");
    assert_eq!(c.m_crit_hit as i32, 50, "m.critical");
    assert_eq!(
        c.m_atk_spd, 499,
        "cast speed (333 × Spellcraft 1.5 in a robe)"
    );

    // --- Now drive the real enter-world refresh tail (expertise + conditioned
    // passives, in the order `handle_enter_world` runs them) and confirm the
    // in-world stats still match — this is where the reported 349 shows up. ---
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);
    b.spawn_into(&mut world);
    super::expertise::refresh_expertise_penalty(&mut world, 4212);
    super::passive_skills::refresh_conditioned_passives(&mut world, 4212);
    assert_eq!(
        pcs(&world, 4212).m_atk_spd,
        499,
        "cast speed after enter-world refresh tail"
    );
    assert_eq!(
        pcs(&world, 4212).p_atk as i32,
        2,
        "p.atk after enter-world refresh tail"
    );
}

/// The armor-conditioned passives close the last gap: Spellcraft (163) multiplies
/// a robe mystic's casting speed by 1.5 (333 → 499), while Magician's Movement
/// (118) stays inert (its −20% atk-speed penalty is gated to non-robe armor).
#[test]
fn spellcraft_passive_raises_mystic_cast_speed_in_a_robe() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let paperdoll = |object_id, item_id, slot| crate::character::ItemRow {
        object_id,
        item_id,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: slot,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    };
    let mut chr = dummy_char(4211, "Robe");
    chr.class_id = 10;
    chr.base_class_id = 10;
    chr.items = vec![
        paperdoll(1001, 6, 5),
        paperdoll(1002, 425, 6),
        paperdoll(1003, 461, 11),
    ];
    // The two autoGet mystic passives.
    chr.skills = vec![(163, 1, 0), (118, 1, 0)];
    let bundle = Player::from_char(&world.data, &chr);
    // `from_char` (Java `restoreCharData`/`addSkill`) already folds the robe
    // passives in: Spellcraft's MAGIC branch (+50%) applies, while Magician's
    // Movement stays inert (its −20% atk-speed penalty is gated to non-robe).
    assert_eq!(
        bundle.combat.m_atk_spd, 499,
        "Spellcraft: 333 × 1.5 in a robe"
    );
    assert_eq!(
        bundle.combat.p_atk_spd, 384,
        "Magician's Movement stays inert in a robe"
    );
    bundle.spawn_into(&mut world);

    // Take the robe legs off: the MAGIC condition now fails (bare legs read as
    // NONE), so `refresh_conditioned_passives` drops Spellcraft's bonus.
    world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&4211)
        .unwrap()
        .unequip_item(1003);
    super::passive_skills::refresh_conditioned_passives(&mut world, 4211);
    assert_eq!(
        pcs(&world, 4211).m_atk_spd,
        333,
        "no robe → Spellcraft bonus gone"
    );
}

/// Reproduction of the reported "casting speed 349 at level 7" bug: a Human
/// Mystic learns Weapon Mastery (249) at getLevel 7, whose `-30%
/// MagicalAttackSpeed` is gated to `<weaponType>BOW/POLE`. Wielding a (non
/// bow/pole) staff in a no-grade robe, that effect must NOT apply, so casting
/// speed stays Spellcraft's 499 — but before the `<weaponType>` gate was
/// honored it dropped to 349 (499 × 0.7). Driven through the real relogin path
/// (delevel filter → `from_char` → enter-world refresh tail); the no-grade robe
/// keeps the armor grade-penalty out of it, isolating the weapon-condition bug.
#[test]
fn human_mystic_lvl7_weapon_mastery_does_not_slow_staff_casting() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    data.skill_trees = crate::data::skill_tree::SkillTreeData::load_from(DIST);
    data.initial_equipment = crate::data::initial_equipment::InitialEquipmentData::load_from(DIST);

    let class_id = 10; // Human Mystic

    // No-grade MAGIC robe (chest/legs/gloves → Spellcraft applies, no grade
    // penalty) plus a D-grade BLUNT staff (15149) — a weapon that is NOT
    // bow/pole, equipped through the real slot logic.
    let mut inv = crate::model::inventory::Inventory::new();
    let mut next_oid = 2000;
    for item_id in [6, 425, 461, 15149] {
        let oid = next_oid;
        next_oid += 1;
        inv.add_item(&data.item_data, oid, item_id, 1);
        inv.equip_item(&data.item_data, oid);
    }
    let items: Vec<crate::character::ItemRow> = inv
        .items()
        .iter()
        .map(|it| {
            let slot = inv.paperdoll_slot_of(it.object_id);
            crate::character::ItemRow {
                object_id: it.object_id,
                item_id: it.item_id,
                count: it.count,
                enchant_level: 0,
                loc: if slot.is_some() {
                    "PAPERDOLL".into()
                } else {
                    "INVENTORY".into()
                },
                loc_data: slot.map(|s| s as i32).unwrap_or(0),
                custom_type1: 0,
                custom_type2: 0,
                mana_left: -1,
                time: 0,
                augment_mineral: 0,
                augment_option1: 0,
                augment_option2: 0,
            }
        })
        .collect();

    let mut chr = dummy_char(4213, "Mystic7");
    chr.class_id = class_id;
    chr.base_class_id = class_id;
    chr.level = 7;
    chr.items = items;
    // Every skill a level-7 mystic can reach (autoGet + learnable), i.e. what the
    // character would have after "reaching level 7 and getting skills".
    chr.skills = data
        .skill_trees
        .all_available_skills(class_id, 7, &std::collections::HashMap::new(), true, true)
        .into_iter()
        .map(|(id, lvl)| (id, lvl, 0))
        .collect();
    assert!(
        chr.skills.iter().any(|&(id, _, _)| id == 163),
        "level-7 mystic has Spellcraft (163)"
    );
    assert!(
        chr.skills.iter().any(|&(id, _, _)| id == 249),
        "level-7 mystic has Weapon Mastery (249)"
    );

    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

    // 1. Character select: the delevel filter (`filter_skills_on_select` →
    // `maybe_skill_remove_on_delevel`), replicated on `chr.skills`.
    let skills_before = chr.skills.len();
    {
        let mut skills_map: std::collections::HashMap<i32, i32> =
            chr.skills.iter().map(|&(id, lvl, _)| (id, lvl)).collect();
        super::death::maybe_skill_remove_on_delevel(
            &world,
            chr.object_id,
            chr.class_id,
            chr.level,
            &mut skills_map,
        );
        chr.skills = skills_map
            .into_iter()
            .map(|(id, lvl)| (id, lvl, 0))
            .collect();
    }
    assert!(
        chr.skills.iter().any(|&(id, _, _)| id == 163),
        "delevel filter kept Spellcraft (163)"
    );
    assert_eq!(
        chr.skills.len(),
        skills_before,
        "delevel filter removed no skills at level 7"
    );

    // 2. Build the player from the (filtered) select data.
    let b = Player::from_char(&world.data, &chr);
    assert_eq!(
        b.combat.m_atk_spd, 499,
        "cast speed after from_char (Spellcraft ×1.5 in a robe)"
    );
    b.spawn_into(&mut world);

    // 3. Enter-world refresh tail, in `handle_enter_world` order.
    super::expertise::refresh_expertise_penalty(&mut world, 4213);
    assert_eq!(
        pcs(&world, 4213).m_atk_spd,
        499,
        "cast speed after expertise refresh"
    );
    super::passive_skills::refresh_conditioned_passives(&mut world, 4213);
    assert_eq!(
        pcs(&world, 4213).m_atk_spd,
        499,
        "cast speed after conditioned-passive refresh"
    );
}

/// Delevel skill filtering runs at character *select*, before `from_char`, so
/// the built `Player` folds only the surviving passives and its enter-world
/// `UserInfo` is right the first time (the casting-speed-349 bug). A robe
/// mystic delevelled below 7 loses its getLevel-7 class skill but keeps
/// Spellcraft (getLevel 1), so casting speed stays 499.
#[test]
fn delevel_filter_on_select_keeps_passive_stats() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    data.skill_trees = crate::data::skill_tree::SkillTreeData::load_from(DIST);
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let paperdoll = |object_id, item_id, slot| crate::character::ItemRow {
        object_id,
        item_id,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: slot,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    };
    let mut chr = dummy_char(4213, "Robe");
    chr.class_id = 10;
    chr.base_class_id = 10;
    chr.level = 5; // below the getLevel-7 skills
    chr.items = vec![
        paperdoll(1001, 6, 5),
        paperdoll(1002, 425, 6),
        paperdoll(1003, 461, 11),
    ];
    // Spellcraft (163, getLevel 1) + Magician's Movement (118, getLevel 1) +
    // Shield (1040, getLevel 7) that a level-5 delevel strips.
    chr.skills = vec![(163, 1, 0), (118, 1, 0), (1040, 1, 0)];

    // The select-time filter (what `filter_skills_on_select` runs).
    let mut skills: std::collections::HashMap<i32, i32> =
        chr.skills.iter().map(|&(id, lvl, _)| (id, lvl)).collect();
    let changes = super::death::maybe_skill_remove_on_delevel(
        &world,
        chr.object_id,
        chr.class_id,
        chr.level,
        &mut skills,
    );
    assert!(
        changes.iter().any(|&(id, a)| id == 1040 && a.is_none()),
        "Shield stripped at level 5"
    );
    chr.skills = skills.into_iter().map(|(id, lvl)| (id, lvl, 0)).collect();

    // `from_char` on the corrected skills: Shield gone, Spellcraft kept, so the
    // casting-speed bonus is folded in and the first UserInfo is 499 (not 349).
    let bundle = Player::from_char(&world.data, &chr);
    assert!(
        !bundle.skills.0.contains_key(&1040),
        "Shield removed from the book"
    );
    assert!(bundle.skills.0.contains_key(&163), "Spellcraft survives");
    assert_eq!(
        bundle.combat.m_atk_spd, 499,
        "Spellcraft's casting-speed bonus intact"
    );
}

/// A live level-down (`check_player_skills`) removes a now-too-high passive and
/// re-folds the stat block: Weapon Mastery (249, getLevel 7, +m.atk) is stripped
/// at level 5, lowering m.atk, while Spellcraft (getLevel 1) stays and keeps
/// casting speed at 499. Only passive skills move stats — step 4.
#[test]
fn live_delevel_removes_passive_and_recomputes_stats() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    data.skill_trees = crate::data::skill_tree::SkillTreeData::load_from(DIST);
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let paperdoll = |object_id, item_id, slot| crate::character::ItemRow {
        object_id,
        item_id,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: slot,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    };
    let mut chr = dummy_char(4214, "Mage");
    chr.class_id = 10;
    chr.base_class_id = 10;
    chr.level = 5;
    chr.items = vec![
        paperdoll(1001, 6, 5),
        paperdoll(1002, 425, 6),
        paperdoll(1003, 461, 11),
    ];
    // Spellcraft (163, getLevel 1) + Weapon Mastery (249, getLevel 7, passive +m.atk).
    chr.skills = vec![(163, 1, 0), (249, 1, 0)];
    let bundle = Player::from_char(&world.data, &chr);
    let m_atk_with_mastery = bundle.combat.m_atk;
    bundle.spawn_into(&mut world);

    // Level-down check strips Weapon Mastery (5 < 7) and re-folds the stats.
    super::death::check_player_skills(&mut world, 4214);
    assert!(
        !world
            .objects
            .get_component::<SkillBook>(&4214)
            .unwrap()
            .0
            .contains_key(&249),
        "Weapon Mastery removed"
    );
    assert!(
        world
            .objects
            .get_component::<SkillBook>(&4214)
            .unwrap()
            .0
            .contains_key(&163),
        "Spellcraft kept"
    );
    // Weapon Mastery's +m.atk is gone; Spellcraft's casting-speed bonus (499)
    // is now un-corrupted by 249 and correctly folded from the reduced book.
    assert!(
        pcs(&world, 4214).m_atk < m_atk_with_mastery,
        "removing Weapon Mastery lowered m.atk"
    );
    assert_eq!(
        pcs(&world, 4214).m_atk_spd,
        499,
        "recompute re-folds only the surviving passives"
    );
}

/// `AutoLearnSkills`: `rewardSkills` must grant every reachable class skill,
/// not just autoGet ones — and only autoGet ones when the flag is off.
#[test]
fn auto_learn_grants_all_reachable_class_skills() {
    use crate::data::skill_tree::SkillLearn;

    let mk_data = || {
        let mut data = GameData::for_test();
        data.player_templates =
            crate::data::PlayerTemplateData::from_vec(vec![human_fighter_template()]);
        // Class 0: a level-1 autoGet skill + a non-autoGet class skill (id 91,
        // levels 1@getLevel5 and 2@getLevel10).
        data.skill_trees.insert_for_test(
            0,
            SkillLearn {
                skill_id: 1000,
                skill_level: 1,
                name: "Auto".into(),
                get_level: 1,
                level_up_sp: 0,
                auto_get: true,
                required_items: Vec::new(),
            },
        );
        data.skill_trees.insert_for_test(
            0,
            SkillLearn {
                skill_id: 91,
                skill_level: 1,
                name: "Class1".into(),
                get_level: 5,
                level_up_sp: 100,
                auto_get: false,
                required_items: Vec::new(),
            },
        );
        data.skill_trees.insert_for_test(
            0,
            SkillLearn {
                skill_id: 91,
                skill_level: 2,
                name: "Class2".into(),
                get_level: 10,
                level_up_sp: 200,
                auto_get: false,
                required_items: Vec::new(),
            },
        );
        data
    };

    let spawn_level_5 = |world: &mut World| {
        let mut chr = dummy_char(2001, "Al");
        chr.level = 5;
        let bundle = Player::from_char(&world.data, &chr);
        let (link_out, _r) = tokio::sync::mpsc::unbounded_channel();
        let s = Session::new(1, link_out, "127.0.0.1:1".parse().unwrap())
            .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
            .into_lobby(vec![])
            .into_entering(bundle);
        let (_session, bundle) = s.into_ingame();
        bundle.spawn_into(world);
    };

    // Flag ON: the class skill (id 91 @ level 1, the max reachable at char
    // level 5) is auto-learned alongside the autoGet skill.
    {
        let (link_tx, _l) = tokio::sync::mpsc::unbounded_channel();
        let (db_tx, _d) = tokio::sync::mpsc::unbounded_channel();
        let mut world = World::new(link_tx, 7, 3, 0, mk_data(), db_tx);
        world.cfg.character.auto_learn_skills = true;
        spawn_level_5(&mut world);
        super::death::reward_skills(&mut world, 2001);
        let book = &world.objects.get_component::<SkillBook>(&2001).unwrap().0;
        assert_eq!(book.get(&1000), Some(&1), "autoGet skill granted");
        assert_eq!(
            book.get(&91),
            Some(&1),
            "class skill auto-learned at level 5"
        );
    }

    // Flag OFF: only the autoGet skill; the class skill stays unlearned.
    {
        let (link_tx, _l) = tokio::sync::mpsc::unbounded_channel();
        let (db_tx, _d) = tokio::sync::mpsc::unbounded_channel();
        let mut world = World::new(link_tx, 7, 3, 0, mk_data(), db_tx);
        assert!(!world.cfg.character.auto_learn_skills, "default is off");
        spawn_level_5(&mut world);
        super::death::reward_skills(&mut world, 2001);
        let book = &world.objects.get_component::<SkillBook>(&2001).unwrap().0;
        assert_eq!(book.get(&1000), Some(&1), "autoGet skill granted");
        assert_eq!(
            book.get(&91),
            None,
            "class skill NOT auto-learned when flag is off"
        );
    }
}

/// `Player.checkPlayerSkills` on delevel: a skill above the `(level − 9)` grace
/// is downgraded to the highest still-reachable level, then removed once even
/// level 1 is out of range — and kept untouched when `DecreaseSkillOnDelevel`
/// is off.
#[test]
fn delevel_downgrades_then_removes_skills() {
    use crate::data::skill_tree::SkillLearn;

    let mk_data = || {
        let mut data = GameData::for_test();
        data.player_templates =
            crate::data::PlayerTemplateData::from_vec(vec![human_fighter_template()]);
        // Skill 91: level 1 @ getLevel 20, level 2 @ getLevel 40.
        data.skill_trees.insert_for_test(
            0,
            SkillLearn {
                skill_id: 91,
                skill_level: 1,
                name: "S1".into(),
                get_level: 20,
                level_up_sp: 100,
                auto_get: false,
                required_items: Vec::new(),
            },
        );
        data.skill_trees.insert_for_test(
            0,
            SkillLearn {
                skill_id: 91,
                skill_level: 2,
                name: "S2".into(),
                get_level: 40,
                level_up_sp: 200,
                auto_get: false,
                required_items: Vec::new(),
            },
        );
        // Skill 92: a single level @ getLevel 7 — used to show the strict flag
        // vs the 9-level grace at low character levels.
        data.skill_trees.insert_for_test(
            0,
            SkillLearn {
                skill_id: 92,
                skill_level: 1,
                name: "S3".into(),
                get_level: 7,
                level_up_sp: 100,
                auto_get: false,
                required_items: Vec::new(),
            },
        );
        data
    };

    // Spawn a level-40 character who knows the skills, then force the level down
    // (a delevel already applied to the model) and run the check.
    let run = |decrease_flag: bool, strict: bool, new_level: i32, skill_id: i32| -> Option<i32> {
        let (link_tx, _l) = tokio::sync::mpsc::unbounded_channel();
        let (db_tx, _d) = tokio::sync::mpsc::unbounded_channel();
        let mut world = World::new(link_tx, 7, 3, 0, mk_data(), db_tx);
        world.cfg.character.decrease_skill_level = decrease_flag;
        world.cfg.character.strict_delevel_skill_removal = strict;

        let mut chr = dummy_char(2001, "Al");
        chr.level = 40;
        chr.skills = vec![(91, 2, 0), (92, 1, 0)];
        let bundle = Player::from_char(&world.data, &chr);
        let (link_out, _r) = tokio::sync::mpsc::unbounded_channel();
        let s = Session::new(1, link_out, "127.0.0.1:1".parse().unwrap())
            .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
            .into_lobby(vec![])
            .into_entering(bundle);
        let (_session, bundle) = s.into_ingame();
        bundle.spawn_into(&mut world);

        world
            .objects
            .get_component_mut::<crate::model::Player>(&2001)
            .unwrap()
            .level = new_level;
        super::death::check_player_skills(&mut world, 2001);
        world
            .objects
            .get_component::<SkillBook>(&2001)
            .unwrap()
            .0
            .get(&skill_id)
            .copied()
    };

    // --- Default strict mode (StrictDelevelSkillRemoval = true). ---
    // 40 → 30: skill 91 @ level 2 (getLevel 40) is out of range → downgrade to
    // the highest reachable level (1, getLevel 20).
    assert_eq!(
        run(true, true, 30, 91),
        Some(1),
        "downgraded to the highest reachable level"
    );
    // 40 → 5: even level 1 (getLevel 20) is out of range → removed.
    assert_eq!(
        run(true, true, 5, 91),
        None,
        "removed when no level is reachable"
    );
    // Skill 92 (getLevel 7) at level 1: strict strips it (1 < 7)…
    assert_eq!(
        run(true, true, 1, 92),
        None,
        "strict removes a getLevel-7 skill at level 1"
    );

    // --- Non-strict (Java 9-level grace). ---
    // …but the 9-level grace keeps it (1 ≥ 7 − 9).
    assert_eq!(
        run(true, false, 1, 92),
        Some(1),
        "grace keeps a getLevel-7 skill at level 1"
    );

    // Flag off: kept despite being out of range, regardless of strictness.
    assert_eq!(
        run(false, true, 5, 91),
        Some(2),
        "kept when DecreaseSkillOnDelevel is off"
    );
}

/// The full happy path of an offensive cast on another player, phase by
/// phase, plus the reuse gate on an immediate re-cast: exact
/// Formulas.calcMagicDam damage, CP absorbed before HP, the SM
/// 2261/2262 damage messages, and every broadcast reaching the target.
#[test]
fn cast_enemy_nuke_deals_damage_and_enforces_reuse() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);

    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Without ctrl an unflagged player is not a valid enemy target.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::INVALID_TARGET
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(!world.objects.has_component::<Casting>(&3001));

    // With ctrl: ExRotation (face target) + initial-MP StatusUpdate +
    // MagicSkillUse to everyone, YOU_USE_S1 + SetupGauge to the caster.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::EX);
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    );
    let msu = a_rx.try_recv().unwrap();
    assert_eq!(msu[0], server_packets::opcodes::MAGIC_SKILL_USE);
    assert_eq!(
        i32::from_le_bytes(msu[25..29].try_into().unwrap()),
        -1,
        "ungrouped skill must send reuse group -1 (0 greys every icon client-side)"
    );
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::YOU_USE_S1
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::SETUP_GAUGE
    );
    assert!(a_rx.try_recv().is_err());
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::EX);
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_USE
    );
    assert!(b_rx.try_recv().is_err());
    assert_eq!(pvit(&world, 3001).cur_mp, 48.0, "50 - mpInitialConsume(2)");

    // Launch at hit = 4000/1.0 − 500 = 3500 ms = 35 ticks.
    world.tick += 35;
    apply_due_tasks(&mut world);
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_LAUNCHED
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_LAUNCHED
    );

    // Finish 500 ms later: MP consume, damage, messages, status updates.
    // Pin the two rolls the finish consumes: the magic crit (d1000) and the
    // `MagicFailures` success roll. PvP takes `calcMagicSuccess`' magic-accuracy
    // branch (both sides players, neither `isAttackable()`), which is only a
    // 98 % rate — left unforced, the nuke would be resisted ~2 % of runs and the
    // exact-damage assertions below would flake.
    world.forced_rolls.extend([999, 0]);
    world.tick += 5;
    apply_due_tasks(&mut world);

    let m_atk = pcs(&world, 3001).m_atk;
    let m_def = pcs(&world, 3002).m_def;
    let damage = formulas::calc_magic_dam(
        m_atk,
        m_def,
        12.0,
        false,
        2.0,
        1.0,
        formulas::MagicFailure::None,
    );
    assert!(
        damage > 100.0,
        "sanity: the nuke must overflow B's CP ({damage})"
    );
    {
        let b = pvit(&world, 3002);
        let bcp = pcp(&world, 3002);
        assert_eq!(bcp.cur_cp, 0.0, "CP absorbs first");
        assert!(
            (b.cur_hp - (100.0 - (damage - 100.0))).abs() < 1e-9,
            "HP takes the rest"
        );
    }
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    ); // MP consume
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2
    );
    // Being hit puts B in combat stance (CreatureAI.onEvtAttacked ->
    // clientStartAutoAttack broadcast), then B's CP/HP status.
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::AUTO_ATTACK_START
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    ); // B's CP/HP
    // Nuking a player flags the caster (SkillCaster: bad skill on a playable →
    // updatePvPStatus(target)): a PVP_FLAG StatusUpdate for object 3001, then
    // the caster's own stance — both broadcast, object 3001.
    let a_flag = a_rx.try_recv().unwrap();
    assert_eq!(a_flag[0], server_packets::opcodes::STATUS_UPDATE);
    assert_eq!(
        i32::from_le_bytes(a_flag[1..5].try_into().unwrap()),
        3001,
        "caster's own pvp-flag update"
    );
    let a_stance = a_rx.try_recv().unwrap();
    assert_eq!(a_stance[0], server_packets::opcodes::AUTO_ATTACK_START);
    assert_eq!(
        i32::from_le_bytes(a_stance[1..5].try_into().unwrap()),
        3001,
        "caster's own stance"
    );
    assert!(a_rx.try_recv().is_err());
    assert_eq!(
        sm_id(&b_rx.try_recv().unwrap()),
        server_packets::sm_ids::C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::AUTO_ATTACK_START
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    );
    // B also sees A's flag: the PVP_FLAG StatusUpdate + a RelationChanged.
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE,
        "B sees A's pvp-flag update"
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::RELATION_CHANGED,
        "B sees A's relation change"
    );
    let b_sees_a = b_rx.try_recv().unwrap();
    assert_eq!(b_sees_a[0], server_packets::opcodes::AUTO_ATTACK_START);
    assert_eq!(
        i32::from_le_bytes(b_sees_a[1..5].try_into().unwrap()),
        3001,
        "B sees the caster's stance"
    );
    assert!(b_rx.try_recv().is_err());
    assert!(
        world
            .objects
            .get_component::<crate::model::components::AttackState>(&3001)
            .is_some_and(|st| st.stance_until_tick > world.tick),
        "caster is in combat stance → canLogout refuses relogin"
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::PvpState>(&3001)
            .unwrap()
            .flag,
        1,
        "caster is now flagged for attacking a player"
    );
    assert!(
        !world.objects.has_component::<Casting>(&3001),
        "coolTime 0 frees the slot"
    );

    // Immediate re-cast: 10 s reuse still has 6 s left → SM 2303 + fail.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::S2_SECONDS_REMAINING_FOR_REUSE
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert!(b_rx.try_recv().is_err(), "rejected cast must not broadcast");
}

/// A shift-click cast out of range (Java `dontMove`) is cancelled with
/// SM 748 — no walk-into-range, nothing announced.
#[test]
fn shift_cast_out_of_range_cancelled_without_moving() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 700, 0); // castRange 600
    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);
    drain(&mut b_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body_shift(1177, true));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(a_rx.try_recv().is_err());
    assert!(b_rx.try_recv().is_err());
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert!(
        !world.objects.has_component::<Intent>(&3001),
        "dontMove must not start a walk-to-cast"
    );
    assert!(!world.objects.has_component::<Movement>(&3001));
}

/// A lethal nuke kills (G9): HP hits 0, the victim is dead, and `Die` with
/// the to-village flag reaches both sides.
#[test]
fn nuke_kills_at_zero_hp() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    world
        .objects
        .get_component_mut::<PlayerVitals>(&3002)
        .unwrap()
        .cur_cp = 0.0;
    world
        .objects
        .get_component_mut::<Vitals>(&3002)
        .unwrap()
        .cur_hp = 5.0;
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    advance_ticks(&mut world, 45);
    let b = pvit(&world, 3002);
    assert_eq!(b.cur_hp, 0.0);
    assert!(b.dead);
    let a_packets = drain(&mut a_rx);
    let b_packets = drain(&mut b_rx);
    for packets in [&a_packets, &b_packets] {
        let die = packets
            .iter()
            .find(|p| {
                p[0] == server_packets::opcodes::DIE
                    && i32::from_le_bytes(p[1..5].try_into().unwrap()) == 3002
            })
            .expect("Die packet for B");
        assert_eq!(
            i32::from_le_bytes(die[5..9].try_into().unwrap()),
            1,
            "to-village flag"
        );
    }
}

/// An offensive skill lands on a siege gate: `resolve_cast_target` accepts the
/// door (siege-attackable), the LOS check is skipped for a door target (Java
/// `canSeeTarget` short-circuit), the pipeline resolves the door's position,
/// and the magic damage routes to the gate's HP instead of the creature path.
#[test]
fn cast_nuke_damages_siege_door() {
    use crate::data::door_data::DoorOpenMethod;
    use crate::model::door::Door;
    use crate::model::siege::Siege;
    let (mut world, ..) = cast_test_world();
    insert_siege_zone(&mut world, 3, 0, 1000, -1000, 1000); // covers the gate at (100, 0)
    let mut siege = Siege::new(3);
    siege.in_progress = true;
    world.sieges.insert(3, siege);
    let door = crate::model::door::spawn_door_for_test(
        &mut world,
        test_door(24190001, DoorOpenMethod::None),
    );
    world
        .objects
        .get_component_mut::<Door>(&door)
        .unwrap()
        .current_hp = 100_000;
    let mut rx = ingame_caster(&mut world, 1, 3001, 150, 0); // within Wind Strike's 600 cast range

    // Ctrl-cast Wind Strike (1177, EnemyOnly) at the gate.
    handle_action(&mut world, 1, &action_body(door, 0));
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "the door is a valid enemy target"
    );
    advance_ticks(&mut world, 45); // launch (35) + finish (5) with margin

    assert!(
        world
            .objects
            .get_component::<Door>(&door)
            .unwrap()
            .current_hp
            < 100_000,
        "the nuke damaged the gate"
    );
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::STATUS_UPDATE
                && i32::from_le_bytes(p[1..5].try_into().unwrap()) == door),
        "the gate's HP bar is refreshed for onlookers",
    );
}

/// Esc aborts a pre-launch cast: `MagicSkillCanceled` broadcast (self
/// included) + `ActionFailed`, the stale phase tasks no-op, the reuse
/// registered at cast start still stands (Java semantics), and once it
/// runs out the caster can cast again.
#[test]
fn esc_aborts_cast_and_stale_tasks_noop() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    drain(&mut a_rx);
    drain(&mut b_rx);
    let mp_after_start = pvit(&world, 3001).cur_mp;

    // Esc (targetLost=false: abort only, keep the target).
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(false));
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_CANCELED
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_CANCELED
    );

    // The scheduled launch is stale: nothing fires, nothing lands.
    world.tick += 40;
    apply_due_tasks(&mut world);
    assert!(a_rx.try_recv().is_err());
    assert!(b_rx.try_recv().is_err());
    assert_eq!(
        pvit(&world, 3001).cur_mp,
        mp_after_start,
        "no finish consume after abort"
    );
    assert_eq!(pvit(&world, 3002).cur_hp, 100.0);

    // Reuse (registered at cast start) still blocks, then expires.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::S2_SECONDS_REMAINING_FOR_REUSE
    );
    drain(&mut a_rx);
    world.tick += 60;
    apply_due_tasks(&mut world);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "castable again after reuse expiry"
    );
}

/// The launch-phase `effectRange` re-check: a target who got away between
/// start and launch cancels the cast quietly (SM 748, no cancel packet —
/// Java `stopCasting(false)`).
#[test]
fn effect_range_recheck_cancels_when_target_moves_away() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    drain(&mut a_rx);
    drain(&mut b_rx);

    world
        .objects
        .get_component_mut::<Position>(&3002)
        .unwrap()
        .x = 5000; // > effectRange 1100

    world.tick += 40;
    apply_due_tasks(&mut world);
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED
    );
    assert!(
        a_rx.try_recv().is_err(),
        "no MagicSkillLaunched, no cancel packet"
    );
    assert!(b_rx.try_recv().is_err());
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert_eq!(pvit(&world, 3002).cur_hp, 100.0);
}

/// A buff cast on another player lands on the *target*: their stats pump,
/// their client gets the AbnormalStatusUpdate, and the expiry restores.
#[test]
fn buff_on_other_player_lands_on_target() {
    let (mut world, ..) = cast_test_world();
    let mut _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut b_rx);
    let base_p_atk = pcs(&world, 3002).p_atk;

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, false));
    advance_ticks(&mut world, 10);

    {
        assert_eq!(pbuffs(&world, 3002), 1);
        assert!(
            pcs(&world, 3002).p_atk > base_p_atk,
            "P.Atk pumped by Might (+8%)"
        );
    }
    let b_packets = drain(&mut b_rx);
    assert!(
        b_packets.iter().any(|p| p[0] == 0x85),
        "target's client gets the AbnormalStatusUpdate"
    );
    assert_eq!(pbuffs(&world, 3001), 0, "nothing lands on the caster");

    advance_ticks(&mut world, 200);
    assert_eq!(pbuffs(&world, 3002), 0);
    assert_eq!(pcs(&world, 3002).p_atk, base_p_atk, "restored after expiry");
}

/// Finish-phase MP shortfall stops the cast quietly: SM 24 +
/// ActionFailed to the caster, but no `MagicSkillCanceled` (Java
/// `stopCasting(false)`), and no effects land.
#[test]
fn finish_phase_mp_shortfall_aborts_quietly() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    drain(&mut a_rx);
    drain(&mut b_rx);

    world
        .objects
        .get_component_mut::<Vitals>(&3001)
        .unwrap()
        .cur_mp = 0.0;

    advance_ticks(&mut world, 45);
    // Launch fires normally (range fine), then the finish fails on MP.
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_LAUNCHED
    );
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::NOT_ENOUGH_MP
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(a_rx.try_recv().is_err());
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_LAUNCHED
    );
    assert!(b_rx.try_recv().is_err(), "no cancel packet on a quiet stop");
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert_eq!(pvit(&world, 3002).cur_hp, 100.0, "no damage landed");
}

/// `RequestSkillCoolTime` reports the remaining reuse of a just-cast
/// skill.
#[test]
fn skill_cool_time_lists_remaining_reuse() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));
    drain(&mut a_rx);

    on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_COOL_TIME]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::SKILL_COOL_TIME);
    assert_eq!(
        i32::from_le_bytes(pkt[1..5].try_into().unwrap()),
        0,
        "Slow Aura has no reuse delay"
    );

    // A reuse with 6 s left is reported with its total and remainder.
    world
        .objects
        .get_component_mut::<Reuses>(&3001)
        .unwrap()
        .0
        .insert(
            1177,
            crate::model::SkillReuse {
                skill_level: 1,
                until_tick: world.tick + 60,
                total_ms: 10_000,
            },
        );
    on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_COOL_TIME]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::SKILL_COOL_TIME);
    assert_eq!(i32::from_le_bytes(pkt[1..5].try_into().unwrap()), 1);
    assert_eq!(i32::from_le_bytes(pkt[5..9].try_into().unwrap()), 1177);
    assert_eq!(
        i32::from_le_bytes(pkt[9..13].try_into().unwrap()),
        1,
        "known level"
    );
    assert_eq!(
        i32::from_le_bytes(pkt[13..17].try_into().unwrap()),
        10,
        "total seconds"
    );
    assert_eq!(
        i32::from_le_bytes(pkt[17..21].try_into().unwrap()),
        6,
        "remaining seconds"
    );
}

/// Skills sharing a positive `reuseDelayGroup` share one cooldown entry
/// keyed by the group id: the `MagicSkillUse` broadcast carries the group,
/// casting one blocks the sibling (SM 48 — short reuse), and
/// `SkillCoolTime` reports the group id with the cast level.
#[test]
fn shared_reuse_group_blocks_sibling_skill() {
    let (mut world, ..) = cast_test_world();

    // Two quick self-skills in shared group 9000 (potion-style), cloned
    // off Slow Aura (91) so only the reuse fields differ.
    let base = world.data.skill_data.get(91, 1).unwrap().clone();
    for id in [7001, 7002] {
        world.data.skill_data.insert_for_test(Skill {
            self_continuous: false,
            id,
            hit_time: 400,
            reuse_delay: 2000,
            reuse_delay_group: 9000,
            ..base.clone()
        });
    }

    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let skills = &mut world
        .objects
        .get_component_mut::<SkillBook>(&3001)
        .unwrap()
        .0;
    skills.insert(7001, 1);
    skills.insert(7002, 1);

    // Cast the first: MagicSkillUse carries group 9000 + the 2000 ms
    // delay, and the reuse lands under the group key, not the skill id.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(7001, false));
    let msu = drain(&mut a_rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE)
        .expect("MagicSkillUse broadcast");
    assert_eq!(
        i32::from_le_bytes(msu[25..29].try_into().unwrap()),
        9000,
        "reuse group"
    );
    assert_eq!(
        i32::from_le_bytes(msu[29..33].try_into().unwrap()),
        2000,
        "reuse delay"
    );
    let reuses = &world.objects.get_component::<Reuses>(&3001).unwrap().0;
    assert!(reuses.contains_key(&9000) && !reuses.contains_key(&7001));

    // The sibling is blocked by the shared cooldown (reuse gate fires
    // before the busy-casting-slot check, same as Java's useMagic order).
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(7002, false));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::S1_IS_NOT_AVAILABLE_REUSE
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );

    // SkillCoolTime reports the group id, cast level, 2 s total/remaining.
    on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_COOL_TIME]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::SKILL_COOL_TIME);
    assert_eq!(i32::from_le_bytes(pkt[1..5].try_into().unwrap()), 1);
    assert_eq!(
        i32::from_le_bytes(pkt[5..9].try_into().unwrap()),
        9000,
        "group id, not skill id"
    );
    assert_eq!(
        i32::from_le_bytes(pkt[9..13].try_into().unwrap()),
        1,
        "cast level"
    );
    assert_eq!(
        i32::from_le_bytes(pkt[13..17].try_into().unwrap()),
        2,
        "total seconds"
    );
    assert_eq!(
        i32::from_le_bytes(pkt[17..21].try_into().unwrap()),
        2,
        "remaining seconds"
    );
}

/// Incoming magic damage can break a victim's pre-launch cast
/// (`Formulas.calcAtkBreak`): `MagicSkillCanceled` broadcast + SM 27 to
/// the victim, and their stale launch task no-ops.
#[test]
fn incoming_magic_damage_can_break_precast() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);

    // B starts a slow self-cast (hit = 9500 ms = 95 ticks).
    handle_request_magic_skill_use(&mut world, 2, &magic_skill_use_body(91, false));
    assert!(world.objects.has_component::<Casting>(&3002));

    // A nukes B; the nuke lands at 40 ticks, well before B's launch.
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Force the rolls: crit d1000 (rate 0 → miss regardless), the magic-success
    // d100 (PvP accuracy branch, rate 98 → 0 lands, so damage is unreduced),
    // then the atk-break d100 → 0 always breaks (rate ≥ 1).
    world.forced_rolls.extend([999, 0, 0]);

    advance_ticks(&mut world, 45);

    assert!(
        !world.objects.has_component::<Casting>(&3002),
        "victim's cast broken"
    );
    let b_packets = drain(&mut b_rx);
    assert!(
        b_packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_CANCELED)
    );
    assert!(
        b_packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::YOUR_CASTING_HAS_BEEN_INTERRUPTED)
    );
    let a_packets = drain(&mut a_rx);
    assert!(
        a_packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_CANCELED)
    );

    // B's stale launch task fires and no-ops: no buff ever lands.
    advance_ticks(&mut world, 60);
    assert_eq!(pbuffs(&world, 3002), 0);
}

/// Casting any skill while running stops the move for good — Java's
/// `PlayerAI.changeIntention` saves the MOVE_TO as `_nextIntention` for a
/// good-skill cast, but `SkillCaster.startCasting` immediately replaces the
/// intention with IDLE, wiping the saved move; a bad skill clears it in
/// `changeIntention` itself. Either way the player stands where the cast
/// began and does not resume walking.
#[test]
fn cast_discards_inflight_move() {
    use crate::model::components::QueuedAction;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    world
        .objects
        .get_component_mut::<Speeds>(&3001)
        .unwrap()
        .run_spd = 100.0;
    world
        .objects
        .get_component_mut::<Speeds>(&3001)
        .unwrap()
        .running = true;

    handle_move_backward_to_location(&mut world, 1, &move_body((600, 0, 0), (0, 0, 0), 1));
    assert!(world.objects.has_component::<Movement>(&3001));
    drain(&mut a_rx);

    // Slow Aura (good, self): the move stops and its destination is dropped.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));
    assert!(world.objects.has_component::<Casting>(&3001));
    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "cast stops the move"
    );
    assert!(
        !world.objects.has_component::<QueuedAction>(&3001),
        "good skill forgets the move (startCasting sets IDLE)"
    );

    // hit 9500 ms (95 ticks) + finish 5 ticks later: still standing.
    advance_ticks(&mut world, 101);
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "move must not resume after the cast"
    );

    // An offensive cast forgets the interrupted move the same way.
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    handle_move_backward_to_location(&mut world, 1, &move_body((600, 0, 0), (0, 0, 0), 1));
    assert!(world.objects.has_component::<Movement>(&3001));
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "cast stops the move"
    );
    assert!(
        !world.objects.has_component::<QueuedAction>(&3001),
        "bad skill forgets the move"
    );
    advance_ticks(&mut world, 45);
    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "nothing resumes after a nuke"
    );
}

/// A skill clicked during a cast is queued (`Player._queuedSkill`) and fires
/// when the cast stops, resolved against the player's *current* target — so
/// re-targeting mid-cast redirects the queued skill (Java `stopCasting` →
/// `useMagic`, which re-resolves the target).
#[test]
fn skill_queued_during_cast_replays_on_current_target() {
    use crate::model::components::QueuedAction;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    let _c_rx = ingame_caster(&mut world, 3, 3003, 150, 0);
    world
        .objects
        .get_component_mut::<Vitals>(&3003)
        .unwrap()
        .cur_hp = 50.0;

    // A nukes B (hit 3500 + finish 500 ms = 40 ticks).
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
    drain(&mut a_rx);

    // Mid-cast: select C, then click Battle Heal → rejected but queued.
    handle_action(&mut world, 1, &action_body(3003, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(a_rx.try_recv().is_err(), "nothing else while the cast runs");
    assert!(
        matches!(
            world.objects.get_component::<QueuedAction>(&3001),
            Some(QueuedAction::Skill { skill_id: 1015, .. })
        ),
        "skill click parked in the queue slot"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Casting>(&3001)
            .unwrap()
            .0
            .skill_id,
        1177,
        "the running cast is untouched"
    );

    // The nuke finishes → the queued heal starts by itself, aimed at C.
    advance_ticks(&mut world, 45);
    let cast = world
        .objects
        .get_component::<Casting>(&3001)
        .expect("queued skill cast started");
    assert_eq!(cast.0.skill_id, 1015);
    assert_eq!(
        cast.0.target_object_id, 3003,
        "replay resolves the mid-cast re-target"
    );
    assert!(
        !world.objects.has_component::<QueuedAction>(&3001),
        "queue consumed"
    );

    // Heal phases (hit 500 + finish 500 ms): C's HP goes up.
    advance_ticks(&mut world, 12);
    assert!(
        pvit(&world, 3003).cur_hp > 50.0,
        "heal landed on the new target"
    );
}

/// A Ctrl-click (force attack) mid-cast on a *new* target must record the
/// attack as the next intention, so the swing starts once the cast ends —
/// Java's `onForcedAttack` → `setIntention(ATTACK)` (deferred to
/// `_nextIntention` while casting). Regression for the "it changes the target
/// but forgets to put the next intention, so when the cast finishes it doesn't
/// start a new action" report: a single ctrl-click used to only select.
#[test]
fn force_attack_mid_cast_engages_new_target_after_cast() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    // Nuke victim + the mob we force-attack next (in melee reach at x=20).
    add_test_npc(&mut world, NPC_OID + 90, 45001, "Monster", 5, 60, 0, 0);
    add_test_npc(&mut world, NPC_OID + 91, 45002, "Monster", 5, 20, 0, 0);
    let cast_target = NPC_OID + 90;
    let next = NPC_OID + 91;

    // Start a nuke on the first monster.
    handle_action(&mut world, 1, &action_body(cast_target, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "nuke is casting"
    );
    drain(&mut a_rx);

    // A SINGLE Ctrl-click on the second monster mid-cast: switches target AND
    // parks the attack as the intention (it can't swing yet — still casting).
    on_packet(
        &mut world,
        1,
        [vec![cop::ATTACK], attack_request_body(next)].concat(),
    );
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(next),
        "target switched to the ctrl-clicked mob"
    );
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(crate::model::PlayerIntent::Attack { target_object_id })) if *target_object_id == next
        ),
        "the force-attack is remembered as the next intention"
    );
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "the running nuke is untouched"
    );

    // When the nuke finishes, the parked attack engages the new mob.
    let hp_before = nvit(&world, next).cur_hp;
    world
        .forced_rolls
        .extend(std::iter::repeat_n([0i32, 99, 10], 12).flatten());
    advance_world(&mut world, 55);
    assert!(
        nvit(&world, next).cur_hp < hp_before,
        "the new target took melee damage after the cast"
    );
}

/// A skill clicked mid-swing (`isAttackingNow`) queues and fires when the
/// swing period ends (Java `thinkAttack`'s queued-skill check /
/// `EVT_READY_TO_ACT`), leaving the attack intent alive to resume after.
#[test]
fn skill_mid_swing_is_queued_until_swing_end() {
    use crate::model::components::QueuedAction;

    let (mut world, ..) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 20;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 100_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    // Swing rolls: hit, no crit, ±0 random damage.
    world.forced_rolls.extend([0, 99, 10]);
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    drain(&mut a_rx);
    let swing_end = world
        .objects
        .get_component::<crate::model::components::AttackState>(&3001)
        .unwrap()
        .attack_end_tick;
    assert!(swing_end > world.tick, "swing in flight");

    // Mid-swing skill click: rejected, queued, intent intact.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(
        !world.objects.has_component::<Casting>(&3001),
        "no cast mid-swing"
    );
    assert!(matches!(
        world.objects.get_component::<QueuedAction>(&3001),
        Some(QueuedAction::Skill { skill_id: 91, .. })
    ));
    assert!(
        world.objects.has_component::<Intent>(&3001),
        "skill click keeps the attack intent"
    );

    // Swing period over (`AttackFinish`): the queued cast starts.
    let remaining = swing_end - world.tick;
    advance_ticks(&mut world, remaining);
    let cast = world
        .objects
        .get_component::<Casting>(&3001)
        .expect("queued skill fired at swing end");
    assert_eq!(cast.0.skill_id, 91);
    assert!(
        world.objects.has_component::<Intent>(&3001),
        "attack resumes after the cast"
    );
}

/// The target-handler geodata check: a wall between caster and target
/// fails the cast with SM 181 (`CANNOT_SEE_TARGET`); with the target on
/// the caster's side the same cast starts normally.
#[test]
fn cast_blocked_by_wall_sends_cannot_see_target() {
    let (mut world, ..) = cast_test_world();
    install_wall_region(&mut world);
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 8, 8);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 328, 8); // across the wall

    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::CANNOT_SEE_TARGET
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(!world.objects.has_component::<Casting>(&3001));

    // Same side of the wall: the cast starts.
    world
        .objects
        .get_component_mut::<Position>(&3002)
        .unwrap()
        .x = 72; // cell 4
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
}

/// Broadcasts only reach players whose region cell is adjacent to the
/// broadcaster's (Java `broadcastPacket` over `forEachVisibleObject`).
#[test]
fn broadcast_is_scoped_to_surrounding_regions() {
    let (mut world, ..) = test_world();
    let _mover_rx = ingame_player(&mut world, 1, 6101, 0, 0, 0);
    let mut near_rx = ingame_player(&mut world, 2, 6102, 500, 500, 0);
    let mut far_rx = ingame_player(&mut world, 3, 6103, 10_000, 10_000, 0);
    world
        .objects
        .get_component_mut::<Speeds>(&6101)
        .unwrap()
        .run_spd = 100.0;

    handle_move_backward_to_location(&mut world, 1, &move_body((1000, 0, 0), (0, 0, 0), 1));

    assert_eq!(
        near_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MOVE_TO_LOCATION
    );
    assert!(
        far_rx.try_recv().is_err(),
        "far player must not see the move"
    );
}

/// A mob that dies **mid-chase** must broadcast `StopMove` (Java `doDie` →
/// `stopMove(null)`) so the client freezes the corpse at the death spot instead
/// of sliding it toward its last move destination — the lingering selection/
/// target decal "where the mob died". The `StopMove` carries the mob's current
/// position and comes before the `Die` broadcast.
#[test]
fn moving_mob_death_broadcasts_stop_move() {
    use crate::model::components::Movement;
    use crate::model::movement::MoveData;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    add_test_npc(&mut world, npc_oid, 40001, "Monster", 5, 40, 0, 0);
    // Give it an in-flight chase move (client is interpolating it toward 400,0).
    world.objects.add_components(
        &npc_oid,
        Movement(MoveData {
            start_x: 40,
            start_y: 0,
            start_z: 0,
            dest_x: 400,
            dest_y: 0,
            dest_z: 0,
            start_tick: world.tick,
            total_ticks: 100,
            geo_path: None,
        }),
    );
    drain(&mut a_rx);

    death::npc_do_die(&mut world, npc_oid, 3001);

    let packets = drain(&mut a_rx);
    let stop_idx = packets
        .iter()
        .position(|p| {
            p[0] == server_packets::opcodes::STOP_MOVE
                && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid
        })
        .expect("StopMove broadcast for the dying mob");
    // Frozen at the death spot (40,0), not the move destination (400,0).
    let stop = &packets[stop_idx];
    assert_eq!(
        i32::from_le_bytes(stop[5..9].try_into().unwrap()),
        40,
        "StopMove at death x"
    );
    assert_eq!(
        i32::from_le_bytes(stop[9..13].try_into().unwrap()),
        0,
        "StopMove at death y"
    );
    // Ordering: StopMove precedes Die (Java doDie order).
    let die_idx = packets
        .iter()
        .position(|p| {
            p[0] == server_packets::opcodes::DIE
                && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid
        })
        .expect("Die broadcast");
    assert!(stop_idx < die_idx, "StopMove is sent before Die");
}

/// An out-of-range cast walks the caster into cast range (Java `useMagic` →
/// CAST intention → `thinkCast`/`maybeMoveToPawn`) and only then starts the
/// cast at the snapshotted target.
#[test]
fn cast_out_of_range_walks_into_range_then_casts() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    // 700 away — castRange 600 + collision 9 + 10 leaves ~81 units to walk.
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 700);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "walks toward the cast target"
    );
    assert!(
        !packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "no cast before range"
    );
    assert!(world.objects.has_component::<Intent>(&3001));
    assert!(!world.objects.has_component::<Casting>(&3001));

    // ~81 units at run speed 115 ⇒ in range in ~8 ticks.
    advance_world(&mut world, 15);
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "cast starts on arrival"
    );
    assert!(
        !world.objects.has_component::<Intent>(&3001),
        "the walk-to-cast intent is consumed"
    );
    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "chase leg stopped before casting"
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE)
    );

    // Launch (35 ticks) + finish (5): the nuke lands on the walked-to monster.
    advance_world(&mut world, 45);
    assert!(
        nvit(&world, npc_oid).cur_hp < 5000.0,
        "nuke landed after the walk"
    );
}

/// Bug fix: casting a beneficial (`Target`-type) skill on a monster requires
/// Ctrl (force). Without it the cast is refused (INVALID_TARGET); with it, it
/// proceeds.
#[test]
fn buff_on_monster_requires_ctrl() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 20;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50);

    // No Ctrl → refused, no cast.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, false));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::INVALID_TARGET
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(
        !world.objects.has_component::<Casting>(&3001),
        "no cast on a mob without force"
    );

    // Ctrl (force) → the cast starts.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, true));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "ctrl force-targets the mob"
    );
}

/// Bug fix: a buff cast on a monster modifies the mob's stats (like on a
/// character) and reverts on expiry.
#[test]
fn buff_on_monster_modifies_stats_and_reverts() {
    use crate::model::components::{Buffs, CombatStats};
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 21;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50);
    let base_p_atk = world
        .objects
        .get_component::<CombatStats>(&npc_oid)
        .unwrap()
        .p_atk;
    assert!(base_p_atk > 0.0, "sanity: the mob has a base pAtk");

    // Might (+8% pAtk), forced onto the mob; lands after hit_time (10 ticks).
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, true));
    advance_ticks(&mut world, 12);
    let buffed = world
        .objects
        .get_component::<CombatStats>(&npc_oid)
        .unwrap()
        .p_atk;
    assert!(
        (buffed - base_p_atk * 1.08).abs() < 1e-6,
        "Might raises the mob pAtk 8% ({base_p_atk} -> {buffed})"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Buffs>(&npc_oid)
            .unwrap()
            .0
            .len(),
        1,
        "buff tracked on the mob"
    );

    // abnormal_time 20 s = 200 ticks → expiry reverts the stat.
    advance_ticks(&mut world, 205);
    let reverted = world
        .objects
        .get_component::<CombatStats>(&npc_oid)
        .unwrap()
        .p_atk;
    assert!(
        (reverted - base_p_atk).abs() < 1e-6,
        "expiry reverts the mob pAtk"
    );
    assert!(
        world
            .objects
            .get_component::<Buffs>(&npc_oid)
            .unwrap()
            .0
            .is_empty(),
        "buff removed on expiry"
    );
}

/// Bug fix: a buff cast on a monster is shown in the target window of players
/// who have it selected (`ExAbnormalStatusUpdateFromTarget`, 0xFE:0xE6).
#[test]
fn buff_on_monster_shows_in_target_window() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 22;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50); // caster now targets the mob

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, true));
    advance_ticks(&mut world, 12);

    let pkt = drain(&mut a_rx)
        .into_iter()
        .find(|p| p.len() >= 13 && p[0] == 0xFE && p[1] == 0xE6 && p[2] == 0x00)
        .expect("ExAbnormalStatusUpdateFromTarget sent to the observer");
    assert_eq!(
        i32::from_le_bytes(pkt[3..7].try_into().unwrap()),
        npc_oid,
        "for the buffed mob"
    );
    assert_eq!(
        i16::from_le_bytes(pkt[7..9].try_into().unwrap()),
        1,
        "one buff shown"
    );
    assert_eq!(
        i32::from_le_bytes(pkt[9..13].try_into().unwrap()),
        1068,
        "Might listed in the target window"
    );
}

/// Nuking a monster with a skill wakes its AI exactly like a melee hit and
/// kills through the same death path (the "kill a monster with a skill"
/// half of the G9 gate).
#[test]
fn nuke_kills_monster_and_rewards() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 11;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 100, 0, 0, 100, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);

    world
        .objects
        .get_component_mut::<crate::model::Player>(&3001)
        .unwrap()
        .exp = 4000; // level 5 on the test table
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);

    // Monsters are valid Enemy targets without ctrl.
    let exp_before = world
        .objects
        .get_component::<crate::model::Player>(&3001)
        .expect("player")
        .exp;
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "cast accepted without force-use"
    );
    // Roll order at cast finish: magic crit (d1000, 999_999 → no crit), the
    // `MagicFailures` success roll (d100, 0 → lands at full damage against a
    // level-5 mob), then the drop roll at death (999_999 → fails, so no loot
    // noise in this test).
    world.forced_rolls.extend([999_999, 0, 999_999]);
    advance_world(&mut world, 45);

    assert!(nvit(&world, npc_oid).dead, "the nuke killed it");
    assert!(
        world
            .objects
            .get_component::<crate::model::Player>(&3001)
            .expect("player")
            .exp
            > exp_before,
        "XP rewarded through the same death path"
    );
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::DIE));
}

/// A nuke against a far-higher-level monster is resisted down to 1 damage —
/// `Formulas.calcMagicDam`'s `ALT_GAME_MAGICFAILURES` branch. `calcMagicSuccess`
/// scales the failure term by `1.3^(targetLevel - skillMagicLevel)`, so at a
/// ~55-level gap the rate is far below 0 and *both* rolls fail whatever they
/// land on, floating the damage to 1. Until this was wired up, a level-5
/// character's Wind Strike killed a level-60 mob at full damage.
#[test]
fn nuke_on_a_far_higher_level_monster_is_resisted_to_one_damage() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::Player>(&3001)
        .unwrap()
        .exp = 4000; // level 5

    let npc_oid = NPC_OID + 31;
    add_test_npc(&mut world, npc_oid, 40099, "Monster", 60, 100, 0, 0);
    let hp_before = nvit(&world, npc_oid).cur_hp;

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    // Only the crit roll needs pinning — a magic crit would double the floored
    // damage to 2. The two success rolls fail on any value at this gap.
    world.forced_rolls.extend([999]);
    advance_world(&mut world, 45);

    // The next regen tick is at 60, past the cast, so nothing heals the 1 back.
    let dealt = hp_before - nvit(&world, npc_oid).cur_hp;
    assert!(
        (dealt - 1.0).abs() < 1e-9,
        "a resisted nuke deals exactly 1 damage, dealt {dealt}"
    );
    assert!(
        !nvit(&world, npc_oid).dead,
        "1 damage can't kill a 100 HP mob"
    );

    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::C1_HAS_RESISTED_YOUR_S2),
        "the caster is told the target resisted"
    );
}

/// The same nuke against a same-level monster is unaffected: the failure roll
/// is a 97 % proposition at a 4-level gap, so the damage is the full MDAM
/// figure. Guards the penalty against over-firing on normal-level content.
#[test]
fn nuke_on_a_same_level_monster_deals_full_damage() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::Player>(&3001)
        .unwrap()
        .exp = 4000; // level 5

    let npc_oid = NPC_OID + 32;
    add_test_npc(&mut world, npc_oid, 40098, "Monster", 5, 100, 0, 0);
    let m_atk = pcs(&world, 3001).m_atk;
    let m_def = pcs(&world, npc_oid).m_def; // `pcs` reads any object's CombatStats
    let unresisted = formulas::calc_magic_dam(
        m_atk,
        m_def,
        12.0,
        false,
        2.0,
        1.0,
        formulas::MagicFailure::None,
    );
    assert!(
        unresisted > 100.0,
        "sanity: an unresisted nuke overkills a 100 HP mob ({unresisted})"
    );

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    // Crit misses; the success roll of 0 lands against the 97 % rate.
    world.forced_rolls.extend([999, 0]);
    advance_world(&mut world, 45);

    // Full damage overkills, so the mob dies — the exact figure is pinned by
    // `magic_dam_matches_java_formula`; what matters here is that no level
    // penalty bit at a 4-level gap (contrast the level-60 case above, which
    // survives on 1 damage).
    assert!(
        nvit(&world, npc_oid).dead,
        "an unpenalized nuke kills a same-level mob"
    );
}

/// Dagger blows deal damage (Mortal Blow, a FatalBlow), and Backstab only
/// lands from a flank: behind the mob it hits, from the front it silently fails.
#[test]
fn dagger_blows_deal_damage_and_backstab_requires_flank() {
    use crate::model::components::{CombatStats, Position, Vitals};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0); // caster at (0,0)
    let npc_oid = NPC_OID + 16;
    // NPC at (40,0). Heading 0 (faces +x, east) → caster to its west is BEHIND.
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    // Deterministic land roll: crit rate > 0 so the blow can land, no random spread.
    {
        let c = world
            .objects
            .get_component_mut::<CombatStats>(&3001)
            .unwrap();
        c.crit_hit = 1000.0;
        c.random_dmg = 0;
    }
    drain(&mut a_rx);

    // Mortal Blow (FatalBlow) — lands from behind, deals damage.
    let mortal = world
        .data
        .skill_data
        .get(16, 1)
        .expect("Mortal Blow")
        .clone();
    let hp0 = nvit(&world, npc_oid).cur_hp;
    world.forced_rolls.extend([999_999, 0, 999_999]); // top magic roll; success lands; crit-double fails
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &mortal);
    assert!(
        nvit(&world, npc_oid).cur_hp < hp0,
        "FatalBlow dealt damage (was a no-op before)"
    );
    world
        .objects
        .get_component_mut::<Vitals>(&npc_oid)
        .unwrap()
        .cur_hp = hp0;

    // Backstab from behind — lands.
    let backstab = world.data.skill_data.get(30, 1).expect("Backstab").clone();
    world.forced_rolls.extend([999_999, 0, 999_999]);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &backstab);
    assert!(
        nvit(&world, npc_oid).cur_hp < hp0,
        "Backstab from the flank landed"
    );
    world
        .objects
        .get_component_mut::<Vitals>(&npc_oid)
        .unwrap()
        .cur_hp = hp0;

    // Turn the mob to face the caster (heading 0x8000 = west) → caster is now in
    // front → Backstab silently fails, dealing no damage.
    world
        .objects
        .get_component_mut::<Position>(&npc_oid)
        .unwrap()
        .heading = 0x8000;
    world.forced_rolls.extend([999_999, 0, 999_999]);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &backstab);
    assert_eq!(
        nvit(&world, npc_oid).cur_hp,
        hp0,
        "front Backstab dealt no damage"
    );
}

/// Vampiric Touch (1147, HpDrain) deals magic damage to the mob and heals the
/// caster by 40% of the HP drained — the regression guard for the whole
/// `HpDrain` family, which used to cast but deal (and drain) nothing.
#[test]
fn vampiric_touch_deals_damage_and_heals_caster() {
    use crate::model::components::Vitals;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 15;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    // Wound the caster (with ample headroom) so the lifesteal isn't
    // overheal-clamped away.
    {
        let v = world.objects.get_component_mut::<Vitals>(&3001).unwrap();
        v.max_hp = 100_000;
        v.cur_hp = 1.0;
    }
    let npc_hp_before = nvit(&world, npc_oid).cur_hp;
    let caster_hp_before = world.objects.get_component::<Vitals>(&3001).unwrap().cur_hp;
    drain(&mut a_rx);

    let skill = world
        .data
        .skill_data
        .get(1147, 1)
        .expect("Vampiric Touch")
        .clone();
    // magic-crit roll fails, then the `MagicFailures` success roll lands (0).
    world.forced_rolls.extend([999_999, 0]);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

    let dmg = npc_hp_before - nvit(&world, npc_oid).cur_hp;
    assert!(
        dmg > 0.0,
        "Vampiric Touch dealt damage (was a silent no-op before)"
    );
    let healed = world.objects.get_component::<Vitals>(&3001).unwrap().cur_hp - caster_hp_before;
    assert!(
        (healed - 0.40 * dmg).abs() < 1.0,
        "caster healed {healed}, expected 40% of {dmg}"
    );
}

/// Spawn the level-5 test mob (40001) targeted for a debuff cast and drain the
/// spawn/target chatter, returning its object id.
fn spawn_debuff_target(
    world: &mut World,
    a_rx: &mut tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
) -> i32 {
    let npc_oid = NPC_OID + 14;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    drain(a_rx);
    npc_oid
}

/// A single-target debuff (Decrease Speed 1160) that passes its landing roll
/// slows the mob server-side (base 120 × 0.80 = 96) and shows the caster the
/// computed landing chance. Against the level-5 test mob the rate constrains to
/// the 90 cap; the forced roll (0) is below it, so the debuff lands. The first
/// forced value feeds the unconditional magic-crit roll, the second the land roll.
#[test]
fn single_target_debuff_lands_and_reports_chance() {
    use crate::model::components::Speeds;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);

    let skill = world
        .data
        .skill_data
        .get(1160, 1)
        .expect("Decrease Speed")
        .clone();
    assert!(skill.is_bad() && skill.affect_scope == AffectScope::Single);
    world.forced_rolls.extend([0, 0]); // magic-crit roll, then land roll (0 < 90 → lands)
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

    // Debuff applied to the mob: run speed recomputed to base 120 × 0.80 = 96.
    let speed = world
        .objects
        .get_component::<Speeds>(&npc_oid)
        .unwrap()
        .run_spd;
    assert!(
        (speed - 96.0).abs() < 1e-6,
        "run speed debuffed to 96, got {speed}"
    );

    // The caster sees the landed-outcome line (single-target only). It is now
    // this server's own message 9001, so the assertion is on which message was
    // sent rather than on a sentence we formatted.
    let msgs = drain(&mut a_rx);
    assert!(
        msgs.iter()
            .any(|p| sysmsg_id(p) == Some(S1_LANDED_ON_C2_CHANCE_WAS_S3::ID as i16)),
        "caster received the debuff-landed message",
    );
}

/// The same cast that fails its landing roll leaves the mob unslowed and sends
/// the caster the "<target> has resisted <skill>: X%" line. The land roll is
/// forced to 90, which is not below the 90 rate, so it resists.
#[test]
fn single_target_debuff_resisted_leaves_target_and_reports() {
    use crate::model::components::Speeds;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);

    let skill = world
        .data
        .skill_data
        .get(1160, 1)
        .expect("Decrease Speed")
        .clone();
    world.forced_rolls.extend([0, 90]); // magic-crit roll, then land roll (90 >= 90 → resisted)
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

    // No debuff: run speed stays at the mob's base 120.
    let speed = world
        .objects
        .get_component::<Speeds>(&npc_oid)
        .unwrap()
        .run_spd;
    assert!(
        (speed - 120.0).abs() < 1e-6,
        "run speed unchanged on resist, got {speed}"
    );

    let msgs = drain(&mut a_rx);
    // The resisted-outcome line carries the target, skill and computed chance
    // as typed parameters (message 9000).
    assert!(
        msgs.iter()
            .any(|p| sysmsg_id(p) == Some(C1_HAS_RESISTED_S2_CHANCE_WAS_S3::ID as i16)),
        "caster received the debuff-resisted message",
    );
}

/// **The debuff's `<trait>` meets the target's `DefenceTrait`.** The dist's
/// stuns (Stun Attack 100, Shield Bash 352, …) declare `<trait>SHOCK`, which
/// the parse test above pins; here the fixture debuff borrows that trait so the
/// *consumption* side is visible. Against an unprotected mob the rate
/// constrains to the 90 cap, but a target made invulnerable to SHOCK drags the
/// same cast down to **0** — invulnerability skips the clamp, so the reported
/// chance drops all the way out of the retail range. That number is the proof
/// that `calcGeneralTraitBonus` reaches the landing roll.
#[test]
fn a_shock_debuff_is_scaled_by_the_targets_shock_defence() {
    use crate::game_loop::skills::effects::merge_defence_traits;
    use crate::model::skill::TraitType;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);

    let mut skill = world
        .data
        .skill_data
        .get(1160, 1)
        .expect("Decrease Speed")
        .clone();
    skill.trait_type = TraitType::Shock;

    // Unprotected: (35 - 5 + 3)·30 + 80 + 30 clamps to the 90 cap.
    world.forced_rolls.extend([0, 95]); // magic-crit roll, then a losing land roll
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let msgs = drain(&mut a_rx);
    assert!(
        msgs.iter().any(|p| {
            sysmsg_id(p) == Some(C1_HAS_RESISTED_S2_CHANCE_WAS_S3::ID as i16)
                && sysmsg_int(p) == Some(90)
        }),
        "unprotected, the stun is offered at the 90 cap",
    );

    // Invulnerable to SHOCK: the same cast is offered at the 10 floor.
    merge_defence_traits(&mut world, npc_oid, &[(TraitType::Shock, 1.0)]);
    world.forced_rolls.extend([0, 95]);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let msgs = drain(&mut a_rx);
    assert!(
        msgs.iter().any(|p| {
            sysmsg_id(p) == Some(C1_HAS_RESISTED_S2_CHANCE_WAS_S3::ID as i16)
                && sysmsg_int(p) == Some(0)
        }),
        "SHOCK invulnerability refuses the debuff outright",
    );
}

/// **A resisted DoT does not burst.** Java puts the magic-crit burst in
/// `DamOverTime.onStart`, which `EffectList.add(info)` only reaches when
/// `calcEffectSuccess` passed — so a resisted poison deals nothing at all,
/// crit or no crit. The port used to apply the burst in the *instant* effect
/// pass, before the land roll, so a resisted debuff still hit for `power * 10`.
///
/// (Java carries an inline `// TODO: M.Crit can occur even if this skill is
/// resisted` at that exact spot. It is aspirational — the shipped code does
/// not do it, and neither does this.)
#[test]
fn a_magic_crit_dot_bursts_only_when_the_debuff_lands() {
    let dot_skill = |world: &World| {
        let mut s = world.data.skill_data.get(1160, 1).expect("fixture").clone();
        s.id = 9610;
        s.name = "Test Poison".into();
        s.magic_type = 1;
        s.effects = vec![crate::model::skill::SkillEffect::DamOverTime {
            power: 5.0,
            ticks: 5,
            can_kill: false,
        }];
        s
    };
    let hp = |w: &World, oid: i32| {
        w.objects
            .get_component::<crate::model::components::Vitals>(&oid)
            .unwrap()
            .cur_hp
    };
    // `Npc::for_test` seeds a 1 000 000 HP pool, but the damage path's stat
    // recalculation clamps it to template 40001's real 100 — which alone would
    // read as 999 900 "damage". Start at that real max so the before/after
    // difference is the burst and nothing else.
    let normalise = |w: &mut World, oid: i32| {
        w.objects
            .get_component_mut::<crate::model::components::Vitals>(&oid)
            .unwrap()
            .cur_hp = 100.0;
    };
    // The crit roll reads the *caster's* `m_crit_hit`; the fixture player has
    // none, so nothing would ever crit.
    let make_critter = |w: &mut World| {
        w.objects
            .get_component_mut::<crate::model::components::CombatStats>(&3001)
            .unwrap()
            .m_crit_hit = 200.0;
    };

    // --- resisted: crit rolled, land roll lost ---
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);
    let skill = dot_skill(&world);
    make_critter(&mut world);
    normalise(&mut world, npc_oid);
    let before = hp(&world, npc_oid);
    world.forced_rolls.extend([0, 90]); // crit, then 90 >= the 90 rate -> resisted
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    assert_eq!(
        hp(&world, npc_oid),
        before,
        "a resisted DoT deals no burst, even on a magic crit"
    );

    // --- landed: the same crit now bursts for power * 10 ---
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);
    let skill = dot_skill(&world);
    make_critter(&mut world);
    normalise(&mut world, npc_oid);
    let before = hp(&world, npc_oid);
    world.forced_rolls.extend([0, 0]); // crit, then 0 < 90 -> lands
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    assert_eq!(
        before - hp(&world, npc_oid),
        50.0,
        "power 5 x 10 on the magic crit"
    );
}

/// Regression: casting a *bad* skill at a monster must aggro it — the mob's AI
/// wakes and switches to the attack intention — **even when the debuff is
/// resisted**. Java `SkillCaster.callSkill` runs `addDamageHate(caster, 0,
/// -effectPoint)` + `notifyEvent(EVT_ATTACKED)` for every bad skill on an
/// attackable, right after `activateSkill` and independent of whether the
/// effects landed. The port used to wake the mob only from the damage/spoil
/// effect handlers, so a pure or resisted debuff never made the monster
/// retaliate ("when using a debuff and it doesn't land, the monster doesn't
/// attack back"). This drives the full network cast path (where the fix lives,
/// in `handle_skill_finish`) and forces the land roll to fail.
#[test]
fn resisted_debuff_still_aggros_monster() {
    use crate::model::components::SkillBook;
    use crate::model::npc::{AggroList, NpcAi, NpcIntention};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);

    // Teach the caster Decrease Speed (1160) so the network cast path accepts it.
    world
        .objects
        .get_component_mut::<SkillBook>(&3001)
        .unwrap()
        .0
        .insert(1160, 1);

    // Target the mob, then cast the debuff, forcing the resist (crit roll 0,
    // land roll 90 ≥ the 90 rate → resisted, as in `single_target_debuff_
    // resisted_leaves_target_and_reports`).
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);
    world.forced_rolls.extend([0, 90]);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1160, false));

    // Run the cast to completion (launch + finish phases).
    advance_ticks(&mut world, 60);

    // The debuff resisted (the resist line fired above), yet the mob is now
    // attacking the caster: `callSkill` woke its AI + added hate regardless.
    let ai = world.objects.get_component::<NpcAi>(&npc_oid).unwrap();
    assert_eq!(
        ai.intention,
        NpcIntention::Attack,
        "resisted debuff still wakes the mob"
    );
    let aggro = world.objects.get_component::<AggroList>(&npc_oid).unwrap();
    assert!(
        aggro.0.contains_key(&3001),
        "the caster is on the mob's aggro list"
    );
}

/// G19 hate-manipulation effects (`GetAgro`/`AddHate`/`DeleteHate`/
/// `DeleteHateOfMe`): before this slice all four effect names fell through
/// unregistered, so Aggression/Charm/Peace/Trick — and every other skill on
/// the same 6-effect family — cast but did nothing to the target's aggro
/// list. The underlying `AggroList`/`NpcAi` primitives were already ported
/// (used by combat/`faction_call`); these effects are thin wiring onto them.
mod hate_effects {
    use super::*;
    use crate::model::npc::{AggroList, NpcAi, NpcIntention};
    use crate::model::skill::SkillEffect;

    const DECOY: i32 = 90001;

    /// Build a synthetic instant skill by cloning a known-good fixture skill
    /// (avoids repeating `Skill`'s ~35 fields) and swapping in the id/effect
    /// under test.
    fn hate_skill(world: &World, id: i32, name: &str, effect: SkillEffect) -> Skill {
        let mut skill = world
            .data
            .skill_data
            .get(1160, 1)
            .expect("fixture base")
            .clone();
        skill.id = id;
        skill.name = name.into();
        skill.effects = vec![effect];
        skill
    }

    /// `GetAgro` (Aggression 28/Aggression Aura 18/Judgment 401/Tribunal 400):
    /// the effected NPC intends to attack the caster, and the caster's hate
    /// becomes dominant over whoever it was already fighting — the ported
    /// AI re-derives its attack target from `AggroList::most_hated` every
    /// think tick, so "force intend-attack" has to mean "become the top
    /// entry," not just flipping the intention flag.
    #[test]
    fn get_agro_forces_the_npc_onto_the_caster() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);
        // A decoy already has strong hate — the NPC is mid-fight with someone else.
        world
            .objects
            .get_component_mut::<AggroList>(&npc_oid)
            .unwrap()
            .0
            .insert(
                DECOY,
                crate::model::npc::AggroInfo {
                    hate: 500.0,
                    damage: 500.0,
                },
            );

        let skill = hate_skill(&world, 28, "Aggression", SkillEffect::GetAgro);
        crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

        let ai = world.objects.get_component::<NpcAi>(&npc_oid).unwrap();
        assert_eq!(
            ai.intention,
            NpcIntention::Attack,
            "the mob intends to attack"
        );
        let aggro = world.objects.get_component::<AggroList>(&npc_oid).unwrap();
        let caster_hate = aggro.0.get(&3001).map(|i| i.hate).unwrap_or(0.0);
        let decoy_hate = aggro.0.get(&DECOY).map(|i| i.hate).unwrap_or(0.0);
        assert!(
            caster_hate > decoy_hate,
            "caster hate ({caster_hate}) must outrank the decoy ({decoy_hate})"
        );
    }

    /// `AddHate` (Charm 15/Lure 51): a flat hate change with no damage.
    /// Positive raises hate and wakes the AI; negative (unused on this dist,
    /// but Java supports it) lowers it, floored at 0.
    #[test]
    fn add_hate_raises_then_lowers_caster_hate() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);

        let up = hate_skill(&world, 15, "Charm", SkillEffect::AddHate { power: 500.0 });
        crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &up);
        assert_eq!(
            world
                .objects
                .get_component::<AggroList>(&npc_oid)
                .unwrap()
                .0[&3001]
                .hate,
            500.0
        );
        assert_eq!(
            world
                .objects
                .get_component::<NpcAi>(&npc_oid)
                .unwrap()
                .intention,
            NpcIntention::Attack,
            "positive power wakes the AI"
        );

        let down = hate_skill(&world, 15, "Charm", SkillEffect::AddHate { power: -800.0 });
        crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &down);
        assert_eq!(
            world
                .objects
                .get_component::<AggroList>(&npc_oid)
                .unwrap()
                .0[&3001]
                .hate,
            0.0,
            "floored at 0, not negative"
        );
    }

    /// **An aggro-shedding skill must not wake the mob it just calmed.** Java
    /// gates `callSkill`'s `EVT_ATTACKED` notify on
    /// `!skill.hasEffectType(HATE)`; the port used to skip the gate (its TODO
    /// claimed no HATE effect was modelled, which stopped being true when
    /// `DeleteHate`/`DeleteHateOfMe` landed). The result was Bluff and Forget
    /// re-aggroing the mob on the same cast that made it forget you.
    ///
    /// The hate *addition* is not gated — Bluff really does carry
    /// `effectPoint -1`, so 1 hate is still added. Only the wake is skipped.
    #[test]
    fn a_hate_shedding_skill_does_not_wake_the_mob() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);

        // An idle mob, and a Bluff-shaped skill: DeleteHateOfMe + effectPoint -1.
        let mut bluff = hate_skill(
            &world,
            9600,
            "Bluff",
            SkillEffect::DeleteHateOfMe { chance: 0 },
        );
        bluff.effect_point = -1;
        assert!(bluff.has_hate_effect());
        assert!(bluff.is_bad(), "still a bad skill, so it reaches the gate");

        crate::game_loop::skills::cast::apply_bad_skill_aggro_for_test(
            &mut world, 3001, npc_oid, &bluff,
        );

        let ai = world.objects.get_component::<NpcAi>(&npc_oid).unwrap();
        assert_ne!(
            ai.intention,
            NpcIntention::Attack,
            "the mob was not woken by the skill that calmed it"
        );
        // The -effectPoint hate still landed.
        assert_eq!(
            world
                .objects
                .get_component::<AggroList>(&npc_oid)
                .unwrap()
                .0[&3001]
                .hate,
            1.0,
            "only the AI wake is suppressed, not the hate"
        );
    }

    /// The control: the very same call with an ordinary debuff *does* wake it.
    #[test]
    fn an_ordinary_bad_skill_still_wakes_the_mob() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);

        let mut plain = hate_skill(&world, 9601, "Plain Debuff", SkillEffect::Root);
        plain.effect_point = -1;
        assert!(!plain.has_hate_effect());

        crate::game_loop::skills::cast::apply_bad_skill_aggro_for_test(
            &mut world, 3001, npc_oid, &plain,
        );
        assert_eq!(
            world
                .objects
                .get_component::<NpcAi>(&npc_oid)
                .unwrap()
                .intention,
            NpcIntention::Attack,
        );
    }

    /// `DeleteHate` (Eva's Serenade 1273/Peace 1075/Repose 1034): a
    /// chance-rolled effect that wipes the target's *entire* aggro list and
    /// disengages its AI, even though only the caster cast the skill —
    /// whoever else was fighting it gets forgotten too (Java's own
    /// behaviour, not an approximation).
    #[test]
    fn delete_hate_wipes_the_whole_list_and_disengages() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);
        {
            let aggro = world
                .objects
                .get_component_mut::<AggroList>(&npc_oid)
                .unwrap();
            aggro.0.insert(
                3001,
                crate::model::npc::AggroInfo {
                    hate: 50.0,
                    damage: 50.0,
                },
            );
            aggro.0.insert(
                DECOY,
                crate::model::npc::AggroInfo {
                    hate: 900.0,
                    damage: 900.0,
                },
            );
            let ai = world.objects.get_component_mut::<NpcAi>(&npc_oid).unwrap();
            ai.intention = NpcIntention::Attack;
        }
        // The first roll is `apply_skill_effects`' unconditional magic-crit
        // roll (999_999 → no crit, irrelevant here); the second is the
        // effect's own chance roll (0, well under the 80/100 chance).
        world.forced_rolls.extend([999_999, 0]);

        let skill = hate_skill(
            &world,
            1273,
            "Eva's Serenade",
            SkillEffect::DeleteHate { chance: 80 },
        );
        crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

        assert!(
            world
                .objects
                .get_component::<AggroList>(&npc_oid)
                .unwrap()
                .0
                .is_empty(),
            "the whole list is wiped, not just the caster's entry"
        );
        assert_eq!(
            world
                .objects
                .get_component::<NpcAi>(&npc_oid)
                .unwrap()
                .intention,
            NpcIntention::Active,
            "the mob disengages"
        );
    }

    /// `DeleteHateOfMe` (Bluff 358/Forget 1156/Trick 11): chance-rolled,
    /// zeroes only the caster's own aggro entry — but, matching Java
    /// exactly, still disengages the AI wholesale even though the decoy's
    /// hate is untouched and still in the list.
    #[test]
    fn delete_hate_of_me_clears_only_the_casters_entry() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);
        {
            let aggro = world
                .objects
                .get_component_mut::<AggroList>(&npc_oid)
                .unwrap();
            aggro.0.insert(
                3001,
                crate::model::npc::AggroInfo {
                    hate: 50.0,
                    damage: 50.0,
                },
            );
            aggro.0.insert(
                DECOY,
                crate::model::npc::AggroInfo {
                    hate: 900.0,
                    damage: 900.0,
                },
            );
            let ai = world.objects.get_component_mut::<NpcAi>(&npc_oid).unwrap();
            ai.intention = NpcIntention::Attack;
        }
        world.forced_rolls.extend([999_999, 0]);

        let skill = hate_skill(
            &world,
            358,
            "Bluff",
            SkillEffect::DeleteHateOfMe { chance: 80 },
        );
        crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

        let aggro = world.objects.get_component::<AggroList>(&npc_oid).unwrap();
        assert_eq!(
            aggro.0[&3001].hate, 0.0,
            "only the caster's own hate is zeroed"
        );
        assert_eq!(aggro.0[&DECOY].hate, 900.0, "the decoy's hate is untouched");
        assert_eq!(
            world
                .objects
                .get_component::<NpcAi>(&npc_oid)
                .unwrap()
                .intention,
            NpcIntention::Active,
            "the AI still disengages wholesale, matching Java"
        );
    }
}

/// Cure Poison (1012) cleanses a POISON debuff via `DispelBySlot`: it removes a
/// landed Poison (129) DoT whose `abnormalLevel` is at or below the cure's
/// dispel level, and leaves a higher-level poison alone. Before the fix
/// `DispelBySlot` fell through the effect registry and the cure was a silent
/// no-op (the poison kept ticking).
#[test]
fn cure_poison_dispels_matching_poison_debuff() {
    use crate::model::components::Buffs;
    use crate::model::skill::{OperateType, Skill, SkillEffect, TargetType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 31;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50);

    // The test world builds skills by hand (no real XML) — mirror the dist
    // values: Poison 129 (abnormalType POISON, abnormalLevel 3 @ lvl 1 / 7 @
    // lvl 4, a DamOverTime debuff) and Cure Poison 1012 (DispelBySlot POISON,3).
    let poison = |level: i32, abnormal_level: i32| Skill {
        self_continuous: false,
        basic_property: crate::model::skill::BasicProperty::None,
        conditions: Vec::new(),
        target_conditions: Vec::new(),
        passive_conditions: Vec::new(),
        without_action: false,
        icon: String::from("icon.skill0000"),
        trait_type: crate::model::skill::TraitType::None,
        static_reuse: false,
        item_consume_id: 0,
        item_consume_count: 0,
        id: 129,
        level,
        name: "Poison".into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::EnemyOnly,
        magic_type: 1,
        magic_level: 20,
        effect_point: -204,
        cast_range: 600,
        effect_range: 1100,
        hit_time: 3000,
        next_action: Default::default(),
        abnormal_resists: Vec::new(),
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 2000,
        reuse_delay_group: -1,
        mp_consume: 8,
        mp_initial_consume: 2,
        hp_consume: 0,
        abnormal_time: 30,
        abnormal_level,
        abnormal_type: "POISON".into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        fan_range: [0; 4],
        attribute_type: None,
        sub_level: 0,
        attribute_value: 0,
        end_effects: Vec::new(),
        channeling_effects: Vec::new(),
        mp_per_channeling: 0,
        channeling_skill_id: 0,
        channeling_tick_ms: 0,
        channeling_start_ms: 0,
        can_be_dispelled: true,
        is_debuff: false,
        shared_with_summon: true,
        stay_after_death: false,
        removed_on_damage: false,
        self_effects: Vec::new(),
        pve_effects: Vec::new(),
        pvp_effects: Vec::new(),
        effects: vec![SkillEffect::DamOverTime {
            power: 24.0,
            ticks: 5,
            can_kill: false,
        }],
    };
    world.data.skill_data.insert_for_test(poison(1, 3));
    world.data.skill_data.insert_for_test(poison(4, 7));
    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        basic_property: crate::model::skill::BasicProperty::None,
        conditions: Vec::new(),
        target_conditions: Vec::new(),
        passive_conditions: Vec::new(),
        without_action: false,
        icon: String::from("icon.skill0000"),
        trait_type: crate::model::skill::TraitType::None,
        static_reuse: false,
        item_consume_id: 0,
        item_consume_count: 0,
        id: 1012,
        level: 1,
        name: "Cure Poison".into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Target,
        magic_type: 1,
        magic_level: 7,
        effect_point: 121,
        cast_range: 600,
        effect_range: 1100,
        hit_time: 4000,
        next_action: Default::default(),
        abnormal_resists: Vec::new(),
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 4000,
        reuse_delay_group: -1,
        mp_consume: 8,
        mp_initial_consume: 2,
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
        fan_range: [0; 4],
        attribute_type: None,
        sub_level: 0,
        attribute_value: 0,
        end_effects: Vec::new(),
        channeling_effects: Vec::new(),
        mp_per_channeling: 0,
        channeling_skill_id: 0,
        channeling_tick_ms: 0,
        channeling_start_ms: 0,
        can_be_dispelled: true,
        is_debuff: false,
        shared_with_summon: true,
        stay_after_death: false,
        removed_on_damage: false,
        self_effects: Vec::new(),
        pve_effects: Vec::new(),
        pvp_effects: Vec::new(),
        effects: vec![SkillEffect::DispelBySlot {
            dispel: vec![("POISON".into(), 3)],
        }],
    });

    let poison1 = world.data.skill_data.get(129, 1).unwrap().clone();
    let poison4 = world.data.skill_data.get(129, 4).unwrap().clone();
    let cure = world.data.skill_data.get(1012, 1).unwrap().clone();

    // Land Poison lvl 1 (abnormalLevel 3) on the mob.
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &poison1);
    assert_eq!(
        world
            .objects
            .get_component::<Buffs>(&npc_oid)
            .unwrap()
            .0
            .len(),
        1,
        "poison landed"
    );

    // Cure Poison lvl 1 dispels POISON up to level 3 → the debuff is removed.
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &cure);
    assert!(
        world
            .objects
            .get_component::<Buffs>(&npc_oid)
            .unwrap()
            .0
            .is_empty(),
        "poison cured"
    );

    // A higher-level poison (lvl 4, abnormalLevel 7) is above Cure Poison lvl 1's
    // reach (POISON,3) and survives the cleanse.
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &poison4);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &cure);
    assert_eq!(
        world
            .objects
            .get_component::<Buffs>(&npc_oid)
            .unwrap()
            .0
            .len(),
        1,
        "a poison above the cure's dispel level is not removed",
    );
}

/// G19 `DispelByCategory` (the "Cancel" family: Cancellation, Cleanse,
/// Purification Field, Touch of Death) — unlike `DispelBySlot`/
/// `DispelBySlotProbability` (a fixed abnormal-type list) this steals
/// *whatever* is up. Before this slice the effect name fell through
/// unregistered and every skill in the family cast but stripped nothing.
mod dispel_by_category {
    use super::*;
    use crate::model::components::Buffs;
    use crate::model::skill::{
        AffectObject, AffectScope, DispelSlot, OperateType, Skill, SkillEffect, StatModifierEffect,
        TargetType,
    };
    use crate::model::stats::{Stat, StatModifierType};

    /// A minimal continuous skill — override `id`/`magic_type`/`effect_point`/
    /// `can_be_dispelled`/`is_debuff`/`effects` per case.
    fn base_skill(id: i32, name: &str) -> Skill {
        Skill {
            self_continuous: false,
            basic_property: crate::model::skill::BasicProperty::None,
            conditions: Vec::new(),
            target_conditions: Vec::new(),
            passive_conditions: Vec::new(),
            without_action: false,
            icon: String::from("icon.skill0000"),
            trait_type: crate::model::skill::TraitType::None,
            static_reuse: false,
            item_consume_id: 0,
            item_consume_count: 0,
            id,
            level: 1,
            name: name.into(),
            operate_type: OperateType::Active,
            is_continuous: true,
            target_type: TargetType::Target,
            magic_type: 1,
            magic_level: 20,
            effect_point: 100,
            cast_range: 600,
            effect_range: 900,
            hit_time: 1000,
            next_action: Default::default(),
            abnormal_resists: Vec::new(),
            hit_cancel_time: 0.0,
            cool_time: 0,
            reuse_delay: 0,
            reuse_delay_group: -1,
            mp_consume: 0,
            mp_initial_consume: 0,
            hp_consume: 0,
            abnormal_time: 120,
            abnormal_level: 1,
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
            fan_range: [0; 4],
            attribute_type: None,
            sub_level: 0,
            attribute_value: 0,
            end_effects: Vec::new(),
            channeling_effects: Vec::new(),
            mp_per_channeling: 0,
            channeling_skill_id: 0,
            channeling_tick_ms: 0,
            channeling_start_ms: 0,
            can_be_dispelled: true,
            is_debuff: false,
            shared_with_summon: true,
            stay_after_death: false,
            removed_on_damage: false,
            self_effects: Vec::new(),
            pve_effects: Vec::new(),
            pvp_effects: Vec::new(),
            effects: Vec::new(),
        }
    }

    fn stat_buff(stat: Stat, amount: f64) -> SkillEffect {
        SkillEffect::StatModifier(StatModifierEffect {
            stat,
            mode: StatModifierType::Diff,
            amount,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
        })
    }

    /// `BUFF` slot: dances are tried before ordinary buffs (Java's
    /// `getDances()` walked before `getBuffs()`, both in reverse cast order),
    /// and `can_be_dispelled=false` is respected.
    #[test]
    fn buff_slot_prefers_dances_and_respects_cant_dispel() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = NPC_OID + 40;
        spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50);

        let mut buff = base_skill(9001, "Regular Buff");
        buff.target_type = TargetType::Target;
        buff.effects = vec![stat_buff(Stat::MaxHp, 100.0)];

        let mut undispellable = base_skill(9002, "Undispellable Buff");
        undispellable.target_type = TargetType::Target;
        undispellable.can_be_dispelled = false;
        undispellable.effects = vec![stat_buff(Stat::MaxMp, 100.0)];

        let mut dance = base_skill(9003, "A Dance");
        dance.target_type = TargetType::Target;
        dance.magic_type = 3; // isMagic==3 -> Dance slot
        dance.effects = vec![stat_buff(Stat::MaxCp, 100.0)];

        let mut cancel = base_skill(1056, "Cancellation");
        cancel.target_type = TargetType::Target;
        cancel.magic_level = 40; // higher than the buffs' 20 so calcCancelSuccess isn't needed at rate=100
        cancel.effects = vec![SkillEffect::DispelByCategory {
            slot: DispelSlot::Buff,
            rate: 100,
            max: 1,
        }];

        for s in [&buff, &undispellable, &dance] {
            world.data.skill_data.insert_for_test(s.clone());
            crate::game_loop::skills::effects::apply_continuous_effects(
                &mut world, 3001, npc_oid, s, None,
            );
        }
        assert_eq!(
            world
                .objects
                .get_component::<Buffs>(&npc_oid)
                .unwrap()
                .0
                .len(),
            3,
            "all three landed"
        );

        crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &cancel);

        let remaining: Vec<i32> = world
            .objects
            .get_component::<Buffs>(&npc_oid)
            .unwrap()
            .0
            .iter()
            .map(|b| b.skill_id)
            .collect();
        assert_eq!(
            remaining,
            vec![9001, 9002],
            "the dance (9003) is stolen first, max=1 stops there"
        );
    }

    /// `DEBUFF` slot (Cleanse/Purification Field, rate 100): strips debuffs
    /// only, leaving positive buffs on the same target untouched.
    #[test]
    fn debuff_slot_strips_only_debuffs() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = NPC_OID + 41;
        spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50);

        let mut buff = base_skill(9001, "Regular Buff");
        buff.target_type = TargetType::Target;
        buff.effects = vec![stat_buff(Stat::MaxHp, 100.0)];

        let mut debuff = base_skill(9010, "A Debuff");
        debuff.target_type = TargetType::Target;
        debuff.effect_point = -50;
        debuff.is_debuff = true;
        debuff.effects = vec![stat_buff(Stat::PhysicalDefence, -20.0)];

        let mut cleanse = base_skill(1409, "Cleanse");
        cleanse.target_type = TargetType::Target;
        cleanse.effects = vec![SkillEffect::DispelByCategory {
            slot: DispelSlot::Debuff,
            rate: 100,
            max: 10,
        }];

        world.data.skill_data.insert_for_test(buff.clone());
        world.data.skill_data.insert_for_test(debuff.clone());
        crate::game_loop::skills::effects::apply_continuous_effects(
            &mut world, 3001, npc_oid, &buff, None,
        );
        crate::game_loop::skills::effects::apply_continuous_effects(
            &mut world, 3001, npc_oid, &debuff, None,
        );
        assert_eq!(
            world
                .objects
                .get_component::<Buffs>(&npc_oid)
                .unwrap()
                .0
                .len(),
            2,
            "both landed"
        );

        crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &cleanse);

        let remaining: Vec<i32> = world
            .objects
            .get_component::<Buffs>(&npc_oid)
            .unwrap()
            .0
            .iter()
            .map(|b| b.skill_id)
            .collect();
        assert_eq!(
            remaining,
            vec![9001],
            "the debuff is stripped, the buff stays"
        );
    }
}

/// `RequestAcquireSkill.checkPlayerSkill` gates: an under-level request sends
/// `YOU_DO_NOT_MEET_THE_SKILL_LEVEL_REQUIREMENTS`, an unaffordable one sends
/// `YOU_DO_NOT_HAVE_ENOUGH_SP_TO_LEARN_THIS_SKILL` — instead of silently dropping.
#[test]
fn skill_acquire_gates_send_system_messages() {
    use crate::data::skill_tree::SkillLearn;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0); // dummy_char: class 0, level 1, sp 0
    drain(&mut rx);

    // Under-level: get_level 10 > player level 1.
    world.data.skill_trees.insert_for_test(
        0,
        SkillLearn {
            skill_id: 1001,
            skill_level: 1,
            name: "Too High".into(),
            get_level: 10,
            level_up_sp: 0,
            auto_get: false,
            required_items: Vec::new(),
        },
    );
    // Reachable level, but costs more SP than the player has (sp 0).
    world.data.skill_trees.insert_for_test(
        0,
        SkillLearn {
            skill_id: 1002,
            skill_level: 1,
            name: "Too Pricey".into(),
            get_level: 1,
            level_up_sp: 100,
            auto_get: false,
            required_items: Vec::new(),
        },
    );

    handle_request_acquire_skill(
        &mut world,
        1,
        &acquire_skill_body(1001, 1, cp::RequestAcquireSkill::CLASS),
    );
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        vec![server_packets::sm_ids::YOU_DO_NOT_MEET_THE_SKILL_LEVEL_REQUIREMENTS],
    );

    handle_request_acquire_skill(
        &mut world,
        1,
        &acquire_skill_body(1002, 1, cp::RequestAcquireSkill::CLASS),
    );
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        vec![server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_SP_TO_LEARN_THIS_SKILL],
    );

    // Neither gate learned the skill.
    let book = world
        .objects
        .get_component::<crate::model::components::SkillBook>(&3001)
        .unwrap();
    assert!(!book.0.contains_key(&1001) && !book.0.contains_key(&1002));
}

/// `checkPlayerSkill`'s required-item leg: a book-gated entry (the class trees'
/// `<item id count/>` children) is refused without the book, and consumes it
/// with the disappear message when the player has it.
#[test]
fn skill_acquire_requires_and_consumes_the_book() {
    use crate::data::skill_tree::SkillLearn;
    use crate::model::inventory::Inventory;

    const BOOK: i32 = 8618; // Ancient Book: Divine Inspiration (Modern Language)

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.cfg.character.divine_inspiration_sp_book_needed = true;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::Player>(&3001)
        .unwrap()
        .sp = 500;
    world.data.skill_trees.insert_for_test(
        0,
        SkillLearn {
            skill_id: 1003,
            skill_level: 1,
            name: "Book Gated".into(),
            get_level: 1,
            level_up_sp: 100,
            auto_get: false,
            required_items: vec![(BOOK, 1)],
        },
    );
    drain(&mut rx);

    // No book in the bag → `YOU_DO_NOT_HAVE_ENOUGH_ITEMS_TO_LEARN_THIS_SKILL`,
    // and neither the skill nor the SP moves.
    handle_request_acquire_skill(
        &mut world,
        1,
        &acquire_skill_body(1003, 1, cp::RequestAcquireSkill::CLASS),
    );
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        vec![server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ITEMS_TO_LEARN_THIS_SKILL],
    );
    assert!(
        !world
            .objects
            .get_component::<crate::model::components::SkillBook>(&3001)
            .unwrap()
            .0
            .contains_key(&1003),
        "book-gated skill not learned without the book"
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::Player>(&3001)
            .unwrap()
            .sp,
        500,
        "SP untouched when the item gate refuses"
    );

    // With the book: learned, book destroyed, `S1_DISAPPEARED` (count 1), SP paid.
    let World { objects, data, .. } = &mut world;
    objects
        .get_component_mut::<Inventory>(&3001)
        .unwrap()
        .add_item(&data.item_data, 9100, BOOK, 1);
    handle_request_acquire_skill(
        &mut world,
        1,
        &acquire_skill_body(1003, 1, cp::RequestAcquireSkill::CLASS),
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::SkillBook>(&3001)
            .unwrap()
            .0
            .get(&1003),
        Some(&1)
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(BOOK),
        0,
        "the book is consumed"
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::Player>(&3001)
            .unwrap()
            .sp,
        400,
        "500 SP - levelUpSp(100)"
    );
    assert!(
        sm_ids_of(&drain(&mut rx)).contains(&server_packets::sm_ids::S1_DISAPPEARED),
        "the disappear message for the consumed book"
    );
}

/// `AcquireSkillList`'s per-entry required-item block (Java writes
/// `getRequiredItems()` as `count` then `(int id, long count)` each) — it was a
/// hard-coded zero, so the client never showed the book beside the skill.
#[test]
fn acquire_skill_list_carries_the_required_book() {
    use crate::data::skill_tree::SkillLearn;

    const BOOK: i32 = 8618;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.data.skill_trees.insert_for_test(
        0,
        SkillLearn {
            skill_id: 1003,
            skill_level: 1,
            name: "Book Gated".into(),
            get_level: 1,
            level_up_sp: 100,
            auto_get: false,
            required_items: vec![(BOOK, 2)],
        },
    );

    let view = crate::model::PlayerView::of_world(&world, 3001).expect("view");
    let skills = world
        .objects
        .get_component::<crate::model::components::SkillBook>(&3001)
        .unwrap();
    let pkt = crate::network::enter_world::acquire_skill_list(view.p, skills, &world.data);

    // 0x90, i16 entry count, then: i32 id, i16 level, i64 sp, u8 getLevel,
    // u8 dualClass, u8 reqCount, (i32 itemId, i64 count)…, u8 removeCount.
    assert_eq!(pkt[0], 0x90);
    assert_eq!(
        i16::from_le_bytes([pkt[1], pkt[2]]),
        1,
        "one learnable skill"
    );
    assert_eq!(i32::from_le_bytes(pkt[3..7].try_into().unwrap()), 1003);
    assert_eq!(pkt[19], 1, "one required item");
    assert_eq!(i32::from_le_bytes(pkt[20..24].try_into().unwrap()), BOOK);
    assert_eq!(i64::from_le_bytes(pkt[24..32].try_into().unwrap()), 2);
    assert_eq!(pkt[32], 0, "no remove-skills");
    assert_eq!(pkt.len(), 33);
}

/// `DivineInspirationSpBookNeeded = False` (this dist): `checkPlayerSkill`
/// returns early for skill 1405, so it needs no book — and because that `return`
/// sits above Java's SP deduction, no SP either. Only 1405 is waived.
#[test]
fn divine_inspiration_book_waiver_also_waives_sp() {
    use crate::data::skill_tree::{DIVINE_INSPIRATION_SKILL_ID, SkillLearn};

    const BOOK: i32 = 8618;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.cfg.character.divine_inspiration_sp_book_needed = false;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::Player>(&3001)
        .unwrap()
        .sp = 500;
    world.data.skill_trees.insert_for_test(
        0,
        SkillLearn {
            skill_id: DIVINE_INSPIRATION_SKILL_ID,
            skill_level: 1,
            name: "Divine Inspiration".into(),
            get_level: 1,
            level_up_sp: 100,
            auto_get: false,
            required_items: vec![(BOOK, 1)],
        },
    );
    // A second book-gated skill that is *not* Divine Inspiration — the waiver is
    // keyed to skill 1405, not to "has required items".
    world.data.skill_trees.insert_for_test(
        0,
        SkillLearn {
            skill_id: 1003,
            skill_level: 1,
            name: "Book Gated".into(),
            get_level: 1,
            level_up_sp: 100,
            auto_get: false,
            required_items: vec![(BOOK, 1)],
        },
    );
    drain(&mut rx);

    handle_request_acquire_skill(
        &mut world,
        1,
        &acquire_skill_body(
            DIVINE_INSPIRATION_SKILL_ID,
            1,
            cp::RequestAcquireSkill::CLASS,
        ),
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::SkillBook>(&3001)
            .unwrap()
            .0
            .get(&DIVINE_INSPIRATION_SKILL_ID),
        Some(&1),
        "learned with no book in the bag"
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::Player>(&3001)
            .unwrap()
            .sp,
        500,
        "Java's early return skips the SP deduction too"
    );

    // The other book-gated skill is still refused.
    drain(&mut rx);
    handle_request_acquire_skill(
        &mut world,
        1,
        &acquire_skill_body(1003, 1, cp::RequestAcquireSkill::CLASS),
    );
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        vec![server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ITEMS_TO_LEARN_THIS_SKILL],
        "the waiver is keyed to skill 1405 alone"
    );
}

/// `StoreSkillCooltime` round-trip: a live cooldown is captured into the save
/// (as an absolute wall-clock end time) and, on relog, `restore_reuses` re-arms
/// it against the current game tick — the cooldown survives the trip.
#[test]
fn skill_reuse_cooldown_survives_relog() {
    use crate::model::SkillReuse;
    use crate::model::components::Reuses;

    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    // A cooldown on reuse-key 1177, ending 500 ticks (50 s) out.
    world
        .objects
        .get_component_mut::<Reuses>(&3001)
        .unwrap()
        .0
        .insert(
            1177,
            SkillReuse {
                skill_level: 3,
                until_tick: world.tick + 500,
                total_ms: 300_000,
            },
        );

    // The save captures it (config default = on) as an absolute systime.
    let save = super::net::build_save_data(&world, 3001).expect("save data");
    assert_eq!(save.skill_reuses.len(), 1);
    let row = save.skill_reuses[0];
    assert_eq!(
        (row.reuse_key, row.skill_level, row.reuse_delay),
        (1177, 3, 300_000)
    );

    // Relog: a fresh bundle from a CharData carrying that row, restored against
    // the current tick + wall clock.
    let mut chr = dummy_char(3002, "Relog");
    chr.skill_reuses = vec![row];
    let mut bundle = Player::from_char(&world.data, &chr);
    bundle.restore_reuses(&chr, world.tick, commons::util::now_millis());

    let restored = bundle.reuses.0.get(&1177).expect("cooldown restored");
    assert_eq!((restored.skill_level, restored.total_ms), (3, 300_000));
    let remaining = restored.until_tick - world.tick;
    assert!(
        (498..=500).contains(&remaining),
        "≈500 ticks left, got {remaining}"
    );

    // With the config off, nothing is persisted (and the DB rows get cleared).
    world.cfg.character.store_skill_cooltime = false;
    assert!(
        super::net::build_save_data(&world, 3001)
            .unwrap()
            .skill_reuses
            .is_empty()
    );
}

/// A live buff is captured into the save as its **remaining seconds** and comes
/// back on relog with that time intact (Java `storeEffect`/`restoreEffects`,
/// buff half). The countdown is frozen while offline: the restored buff gets
/// its full stored duration measured off the *new* tick, however long the
/// character was away.
#[test]
fn buff_survives_relog_without_offline_countdown() {
    use crate::game_loop::skills::effects::{apply_skill_effects, restore_persisted_buffs};

    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // `synthetic_buff` has `abnormal_time` 100 s; burn 30 s of it.
    let buff = synthetic_buff(9500, 2, "RELOG", 1, 1);
    world.data.skill_data.insert_for_test(buff.clone());
    apply_skill_effects(&mut world, 3001, 3001, &buff);
    world.tick += 300;

    // The save captures the *remaining* 70 s, not the skill's full duration.
    let save = super::net::build_save_data(&world, 3001).expect("save data");
    assert_eq!(save.skill_buffs.len(), 1, "the live buff is captured");
    let row = save.skill_buffs[0];
    assert_eq!(
        (row.skill_id, row.skill_level, row.remaining_time_secs),
        (9500, 2, 70)
    );

    // Relog after a long "offline" gap — modelled by advancing the tick well past
    // where the buff would have expired had it kept counting down.
    world.tick += 100_000;
    let _rx2 = ingame_caster(&mut world, 2, 3002, 0, 0);
    restore_persisted_buffs(&mut world, 3002, &[row]);

    let restored = world
        .objects
        .get_component::<Buffs>(&3002)
        .and_then(|b| b.0.iter().find(|x| x.skill_id == 9500).cloned())
        .expect("buff restored");
    assert_eq!(
        restored.skill_level, 2,
        "the stored level came back, not the skill's level 1"
    );
    // 70 s off the *current* tick: the offline gap consumed none of the buff.
    assert_eq!(restored.expires_at_tick - world.tick, 700);

    // With the config off, buffs aren't persisted (and the DB rows get cleared).
    world.cfg.character.store_skill_cooltime = false;
    assert!(
        super::net::build_save_data(&world, 3001)
            .unwrap()
            .skill_buffs
            .is_empty()
    );
}

/// Java `storeEffect`'s skip list: a dance/song is dropped at logout unless
/// `AltStoreDances`, and a toggle (no expiry) is never stored at all.
#[test]
fn dances_and_toggles_are_not_stored_by_default() {
    use crate::game_loop::skills::effects::apply_skill_effects;

    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // magic_type 3 = dance/song.
    let dance = synthetic_buff(9600, 1, "DANCE", 1, 3);
    // A toggle-ish buff: 0 `abnormal_time` → no expiry (the `u64::MAX` sentinel).
    let mut toggle = synthetic_buff(9601, 1, "TOGGLE", 1, 1);
    toggle.abnormal_time = 0;
    apply_skill_effects(&mut world, 3001, 3001, &dance);
    apply_skill_effects(&mut world, 3001, 3001, &toggle);
    assert_eq!(pbuffs(&world, 3001), 2, "both landed");

    world.cfg.character.alt_store_dances = false;
    let save = super::net::build_save_data(&world, 3001).expect("save data");
    assert!(
        save.skill_buffs.is_empty(),
        "dance dropped, toggle never stored"
    );

    // AltStoreDances=True (this dist) keeps the dance — but still not the toggle.
    world.cfg.character.alt_store_dances = true;
    let save = super::net::build_save_data(&world, 3001).expect("save data");
    assert_eq!(save.skill_buffs.len(), 1);
    assert_eq!(
        save.skill_buffs[0].skill_id, 9600,
        "only the dance came through"
    );
}

// --- Buff-slot stacking & count caps (Java `EffectList.addActive`) -----------

/// A synthetic self-buff with a `PhysicalDefence +8%` modifier so it lands (a
/// non-empty effect list), tagged with the given abnormal type/level and
/// magic type (3 = dance/song).
fn synthetic_buff(
    id: i32,
    level: i32,
    abnormal_type: &str,
    abnormal_level: i32,
    magic_type: i32,
) -> Skill {
    use crate::model::skill::{Skill, SkillEffect, StatModifierEffect};
    use crate::model::stats::{Stat, StatModifierType};
    Skill {
        self_continuous: false,
        basic_property: crate::model::skill::BasicProperty::None,
        conditions: Vec::new(),
        target_conditions: Vec::new(),
        passive_conditions: Vec::new(),
        without_action: false,
        icon: String::from("icon.skill0000"),
        trait_type: crate::model::skill::TraitType::None,
        static_reuse: false,
        item_consume_id: 0,
        item_consume_count: 0,
        id,
        level,
        name: format!("Buff{id}"),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Self_,
        magic_type,
        magic_level: 0,
        activate_rate: -1,
        lvl_bonus_rate: 0,
        effect_point: 100, // >= 0 → not a debuff
        cast_range: 0,
        effect_range: 0,
        hit_time: 0,
        next_action: Default::default(),
        abnormal_resists: Vec::new(),
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 100,
        abnormal_level,
        abnormal_type: abnormal_type.into(),
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        fan_range: [0; 4],
        attribute_type: None,
        sub_level: 0,
        attribute_value: 0,
        end_effects: Vec::new(),
        channeling_effects: Vec::new(),
        mp_per_channeling: 0,
        channeling_skill_id: 0,
        channeling_tick_ms: 0,
        channeling_start_ms: 0,
        can_be_dispelled: true,
        is_debuff: false,
        shared_with_summon: true,
        stay_after_death: false,
        removed_on_damage: false,
        self_effects: Vec::new(),
        pve_effects: Vec::new(),
        pvp_effects: Vec::new(),
        effects: vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::PhysicalDefence,
            mode: StatModifierType::Per,
            amount: 8.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
        })],
    }
}

fn buff_skill_level(world: &World, oid: i32, skill_id: i32) -> i32 {
    world
        .objects
        .get_component::<Buffs>(&oid)
        .and_then(|b| b.0.iter().find(|x| x.skill_id == skill_id))
        .map(|x| x.skill_level)
        .unwrap_or(0)
}

fn has_buff(world: &World, oid: i32, skill_id: i32) -> bool {
    world
        .objects
        .get_component::<Buffs>(&oid)
        .is_some_and(|b| b.0.iter().any(|x| x.skill_id == skill_id))
}

/// Same abnormal type: a lower-level cast is refused (no downgrade, no second
/// slot); an equal or higher level replaces in place.
#[test]
fn buff_same_abnormal_type_level_stacking() {
    use crate::game_loop::skills::effects::apply_skill_effects;
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let lvl1 = synthetic_buff(9001, 1, "MYBUFF", 1, 1);
    let lvl3 = synthetic_buff(9001, 3, "MYBUFF", 3, 1);

    // Land level 3.
    apply_skill_effects(&mut world, 3001, 3001, &lvl3);
    assert_eq!(pbuffs(&world, 3001), 1, "level 3 landed");
    assert_eq!(buff_skill_level(&world, 3001, 9001), 3);

    // A lower level is refused — no duplicate, still level 3.
    apply_skill_effects(&mut world, 3001, 3001, &lvl1);
    assert_eq!(
        pbuffs(&world, 3001),
        1,
        "lower level does not stack a second slot"
    );
    assert_eq!(
        buff_skill_level(&world, 3001, 9001),
        3,
        "lower level did not downgrade the buff"
    );

    // Re-casting the same level replaces in place (refresh) — still one slot.
    apply_skill_effects(&mut world, 3001, 3001, &lvl3);
    assert_eq!(pbuffs(&world, 3001), 1, "re-cast same level does not stack");
}

/// A different skill sharing the abnormal type also can't stack; the higher
/// level overrides the lower.
#[test]
fn buff_higher_level_overrides_same_type() {
    use crate::game_loop::skills::effects::apply_skill_effects;
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // Two *different* skills, same abnormal type.
    let weak = synthetic_buff(9101, 1, "SHARED", 1, 1);
    let strong = synthetic_buff(9102, 1, "SHARED", 4, 1);

    apply_skill_effects(&mut world, 3001, 3001, &weak);
    assert!(has_buff(&world, 3001, 9101), "weak buff landed");

    // Higher abnormal level overrides — the weak one is removed.
    apply_skill_effects(&mut world, 3001, 3001, &strong);
    assert_eq!(pbuffs(&world, 3001), 1, "same abnormal type never stacks");
    assert!(has_buff(&world, 3001, 9102), "strong buff present");
    assert!(!has_buff(&world, 3001, 9101), "weak buff overridden");
}

// --- RequestDispel (alt+click buff-cancel, ex 0xD0:0x0048) -------------------

/// The `RequestDispel` ex body after the sub-opcode: objectId, skillId,
/// skillLevel (short), skillSubLevel (short).
fn dispel_body(object_id: i32, skill_id: i32, skill_level: i32, skill_sub_level: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(object_id);
    w.write_i32(skill_id);
    w.write_i16(skill_level as i16);
    w.write_i16(skill_sub_level as i16);
    w.into_bytes()
}

/// Alt+clicking a normal self-buff strips it: the buff leaves and its stat
/// contribution (P.Def +8%) reverts.
#[test]
fn dispel_removes_self_buff() {
    use crate::game_loop::skills::{effects::apply_skill_effects, handle_request_dispel};
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let buff = synthetic_buff(9200, 1, "MYBUFF", 1, 1);
    world.data.skill_data.insert_for_test(buff.clone());
    apply_skill_effects(&mut world, 3001, 3001, &buff);
    assert!(has_buff(&world, 3001, 9200), "buff landed");
    assert_eq!(pbuffs(&world, 3001), 1);

    handle_request_dispel(&mut world, 1, &dispel_body(3001, 9200, 1, 0));
    assert!(!has_buff(&world, 3001, 9200), "alt+click removed the buff");
    assert_eq!(pbuffs(&world, 3001), 0, "buff slot freed after dispel");
}

/// A debuff can't be self-dispelled (Java `skill.isDebuff()` guard), even though
/// it sits in the buff list.
#[test]
fn dispel_refuses_debuff() {
    use crate::game_loop::skills::{effects::apply_skill_effects, handle_request_dispel};
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let mut debuff = synthetic_buff(9201, 1, "MYDEBUFF", 1, 1);
    debuff.is_debuff = true;
    world.data.skill_data.insert_for_test(debuff.clone());
    apply_skill_effects(&mut world, 3001, 3001, &debuff);
    assert!(has_buff(&world, 3001, 9201), "debuff landed");

    handle_request_dispel(&mut world, 1, &dispel_body(3001, 9201, 1, 0));
    assert!(
        has_buff(&world, 3001, 9201),
        "debuff cannot be alt+click dispelled"
    );
}

/// A skill flagged `canBeDispelled=false` survives an alt+click.
#[test]
fn dispel_refuses_undispellable_buff() {
    use crate::game_loop::skills::{effects::apply_skill_effects, handle_request_dispel};
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let mut buff = synthetic_buff(9202, 1, "MYBUFF", 1, 1);
    buff.can_be_dispelled = false;
    world.data.skill_data.insert_for_test(buff.clone());
    apply_skill_effects(&mut world, 3001, 3001, &buff);
    assert!(has_buff(&world, 3001, 9202), "buff landed");

    handle_request_dispel(&mut world, 1, &dispel_body(3001, 9202, 1, 0));
    assert!(
        has_buff(&world, 3001, 9202),
        "undispellable buff survives alt+click"
    );
}

/// A dance/song (`magic_type == 3`) is only strippable when `DanceCancelBuff`
/// is on — this dist's Character.ini sets it True.
#[test]
fn dispel_dance_gated_by_config() {
    use crate::game_loop::skills::{effects::apply_skill_effects, handle_request_dispel};
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let dance = synthetic_buff(9203, 1, "MYDANCE", 1, 3);
    world.data.skill_data.insert_for_test(dance.clone());

    // Config off: the dance is not removed.
    world.cfg.character.dance_cancel_buff = false;
    apply_skill_effects(&mut world, 3001, 3001, &dance);
    assert!(has_buff(&world, 3001, 9203), "dance landed");
    handle_request_dispel(&mut world, 1, &dispel_body(3001, 9203, 1, 0));
    assert!(
        has_buff(&world, 3001, 9203),
        "dance kept while DanceCancelBuff is off"
    );

    // Config on (this dist's default): the dance is removed.
    world.cfg.character.dance_cancel_buff = true;
    handle_request_dispel(&mut world, 1, &dispel_body(3001, 9203, 1, 0));
    assert!(
        !has_buff(&world, 3001, 9203),
        "dance removed while DanceCancelBuff is on"
    );
}

/// A dispel aimed at a foreign object id (not the player's own, nor their
/// summon) is a no-op for the player's buffs.
#[test]
fn dispel_wrong_object_id_ignored() {
    use crate::game_loop::skills::{effects::apply_skill_effects, handle_request_dispel};
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let buff = synthetic_buff(9204, 1, "MYBUFF", 1, 1);
    world.data.skill_data.insert_for_test(buff.clone());
    apply_skill_effects(&mut world, 3001, 3001, &buff);
    assert!(has_buff(&world, 3001, 9204), "buff landed");

    handle_request_dispel(&mut world, 1, &dispel_body(9999, 9204, 1, 0));
    assert!(
        has_buff(&world, 3001, 9204),
        "dispel on a foreign object id leaves the player's buff"
    );
}

/// **Alt+click dispel on a summon's buff strips it off the summon** (Java's
/// `getPet()` / `getServitor(_objectId)` branch), leaving the player's own buff.
#[test]
fn dispel_strips_a_summon_buff() {
    use crate::game_loop::servitor::summon_servitor;
    use crate::game_loop::skills::{effects::apply_skill_effects, handle_request_dispel};
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    // Register a servitor template and summon one.
    let mut t = crate::data::npc_data::default_template(14799);
    t.type_name = "Servitor".into();
    t.base_hp_max = 400.0;
    t.base_mp_max = 200.0;
    world.data.npc_data.insert_for_test(t);
    let servitor = summon_servitor(&mut world, 3001, 14799, 283, 1200, 0, 0).expect("summoned");

    // Put the same buff on both the owner and the servitor.
    let buff = synthetic_buff(9210, 1, "MYBUFF", 1, 1);
    world.data.skill_data.insert_for_test(buff.clone());
    apply_skill_effects(&mut world, 3001, 3001, &buff);
    apply_skill_effects(&mut world, 3001, servitor, &buff);
    assert!(has_buff(&world, 3001, 9210) && has_buff(&world, servitor, 9210));

    // Alt+click the servitor's buff → removed from the servitor only.
    handle_request_dispel(&mut world, 1, &dispel_body(servitor, 9210, 1, 0));
    assert!(
        !has_buff(&world, servitor, 9210),
        "the summon's buff was stripped"
    );
    assert!(
        has_buff(&world, 3001, 9210),
        "the owner's own buff is untouched"
    );
}

/// The good-buff slot cap (`MaxBuffAmount`) drops the oldest buff to make room;
/// dances count against their own cap, not the buff cap.
#[test]
fn buff_slot_cap_drops_oldest() {
    use crate::game_loop::skills::effects::apply_skill_effects;
    let (mut world, ..) = cast_test_world();
    world.data.combat_caps.max_buff_count = 2; // shrink the cap for the test
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // Three distinct-type buffs; the cap is 2.
    apply_skill_effects(&mut world, 3001, 3001, &synthetic_buff(9201, 1, "A", 1, 1));
    apply_skill_effects(&mut world, 3001, 3001, &synthetic_buff(9202, 1, "B", 1, 1));
    assert_eq!(pbuffs(&world, 3001), 2, "at the buff cap");

    apply_skill_effects(&mut world, 3001, 3001, &synthetic_buff(9203, 1, "C", 1, 1));
    assert_eq!(pbuffs(&world, 3001), 2, "cap holds after a third");
    assert!(!has_buff(&world, 3001, 9201), "oldest buff (A) dropped");
    assert!(has_buff(&world, 3001, 9203), "newest buff (C) present");

    // A dance uses its own pool, so it lands on top of the 2 buffs.
    apply_skill_effects(&mut world, 3001, 3001, &synthetic_buff(9301, 1, "D", 1, 3));
    assert_eq!(
        pbuffs(&world, 3001),
        3,
        "the dance is counted separately, not against the buff cap"
    );
    assert!(has_buff(&world, 3001, 9301), "dance landed");
}

/// Repro for "cast on a monster, mid-cast select a far monster and click the
/// same skill again → the click is forgotten": the queued skill must replay at
/// cast end against the new target and, being out of range, start the
/// walk-to-cast leg (Java `stopCasting` → `useMagic` → CAST intention →
/// `thinkCast`/`maybeMoveToPawn`).
#[test]
fn queued_skill_on_far_retarget_walks_into_range_after_cast() {
    use crate::model::components::QueuedAction;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let near = NPC_OID + 70;
    let far = NPC_OID + 71;
    spawn_targeted_monster(&mut world, &mut a_rx, near, 100);
    // The far monster: outside castRange 600, spawned untargeted.
    let (npc, extra) = crate::model::npc::Npc::for_test(far, 40001, 900, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1.0).or_default().push(far);
    world.objects.spawn(far, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&far, cs);

    // Nuke the near monster (hit 3500 + finish 500 ms = 40 ticks).
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "first cast running"
    );

    // Mid-cast, past the real Wind Strike's 1200 ms reuse (the test skill's
    // 10 s reuse is dropped to model the dist timing, where the reuse expires
    // while the 4 s cast is still running): select the far monster and click
    // the same skill again.
    advance_world(&mut world, 15);
    if let Some(reuses) = world
        .objects
        .get_component_mut::<crate::model::components::Reuses>(&3001)
    {
        reuses.0.clear();
    }
    handle_action(&mut world, 1, &action_body(far, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        matches!(
            world.objects.get_component::<QueuedAction>(&3001),
            Some(QueuedAction::Skill { skill_id: 1177, .. })
        ),
        "second click parked in the queue slot"
    );
    drain(&mut a_rx);

    // Cast end → replay → out of range → walk-to-cast toward the far monster.
    advance_world(&mut world, 30);
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(crate::model::PlayerIntent::Cast { target_object_id, .. })) if *target_object_id == far
        ),
        "replayed click walks to the far monster (got intent {:?}, queued {:?}, casting {:?})",
        world.objects.get_component::<Intent>(&3001),
        world.objects.get_component::<QueuedAction>(&3001),
        world.objects.get_component::<Casting>(&3001)
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "MoveToPawn broadcast for the walk"
    );

    // The nuked monster has 5000 HP, so it survived and has been meleeing the
    // caster since the nuke landed — and now that a mob swings the instant it
    // closes (`EVT_ARRIVED` → `onEvtThink`) that is enough damage to kill the
    // 100 HP test caster during the walk below. This test is about the
    // queued-skill replay, not about retaliation: call the monster off and
    // top the caster back up so the walk-to-cast is what is being measured.
    world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&near)
        .unwrap()
        .0
        .clear();
    world
        .objects
        .get_component_mut::<crate::model::npc::NpcAi>(&near)
        .unwrap()
        .intention = crate::model::npc::NpcIntention::Active;
    {
        let v = world
            .objects
            .get_component_mut::<crate::model::components::Vitals>(&3001)
            .unwrap();
        v.cur_hp = v.max_hp as f64;
    }

    // ~300 units at run speed ⇒ in range, then the cast starts on the far mob.
    // 40 ticks is the bare arrival time, with no slack for a tick lost to the
    // walk's start, so allow a couple more — the cast has 40 ticks of its own
    // to run, which leaves plenty of window to observe it.
    advance_world(&mut world, 45);
    let cast = world
        .objects
        .get_component::<Casting>(&3001)
        .expect("cast started after the walk");
    assert_eq!(
        cast.0.target_object_id, far,
        "cast aimed at the far monster"
    );
}

/// Same scenario through the real client packet sequence: switching targets
/// sends `RequestTargetCanceld` (aborting the running cast) before the
/// `Action` click, so the second skill click must start the walk-to-cast
/// immediately.
#[test]
fn far_retarget_after_target_cancel_walks_into_range() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let near = NPC_OID + 72;
    let far = NPC_OID + 73;
    spawn_targeted_monster(&mut world, &mut a_rx, near, 100);
    let (npc, extra) = crate::model::npc::Npc::for_test(far, 40001, 900, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1.0).or_default().push(far);
    world.objects.spawn(far, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&far, cs);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "first cast running"
    );

    // Client target switch: TargetCanceld (aborts the cast) + Action(far).
    advance_world(&mut world, 15);
    if let Some(reuses) = world
        .objects
        .get_component_mut::<crate::model::components::Reuses>(&3001)
    {
        reuses.0.clear();
    }
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(false));
    assert!(
        !world.objects.has_component::<Casting>(&3001),
        "cast aborted by the switch"
    );
    handle_action(&mut world, 1, &action_body(far, 0));
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(crate::model::PlayerIntent::Cast { target_object_id, .. })) if *target_object_id == far
        ),
        "second click walks to the far monster (got intent {:?})",
        world.objects.get_component::<Intent>(&3001)
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "MoveToPawn broadcast for the walk"
    );

    advance_world(&mut world, 40);
    let cast = world
        .objects
        .get_component::<Casting>(&3001)
        .expect("cast started after the walk");
    assert_eq!(
        cast.0.target_object_id, far,
        "cast aimed at the far monster"
    );
}

/// The same "queue on a far retarget" flow against the real datapack: real
/// Wind Strike (4 s cast, 1.2 s reuse — the reuse expires while the cast is
/// still running, so the mid-cast second click must reach the queue slot).
#[test]
fn queued_far_retarget_with_real_datapack_timings() {
    use crate::model::components::QueuedAction;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    // The first cast's damage makes the (passive, level-1) Gremlin retaliate,
    // and a melee hit on a still-abortable cast rolls `Formulas.calcAtkBreak`
    // — `15 + sqrt(13 * dmg)`, ~20 % at this damage — which then decides the
    // *last* assertion below. That made this test fail ~2 % of the time for a
    // reason it does not cover. `isInvul` returns out of `reduce_hp` ahead of
    // the break roll, so the retaliation lands no damage and rolls nothing,
    // leaving the queue/retarget flow under test untouched.
    world.objects.add_components(
        &3001,
        crate::model::components::AdminFlags {
            invul: true,
            ..Default::default()
        },
    );
    let near = NPC_OID + 74;
    let far = NPC_OID + 75;
    // Real-datapack monsters (Gremlin, 20001) at 100 and 900 units.
    for (oid, x) in [(near, 100), (far, 900)] {
        let (npc, extra) = crate::model::npc::Npc::for_test(oid, 20001, x, 0, 0, 5000, 30);
        world.npc_regions.entry(extra.1.0).or_default().push(oid);
        world.objects.spawn(oid, (npc, extra));
        let cs = crate::model::npc::npc_combat_stats(
            world.data.npc_data.get(20001).unwrap(),
            &world.data.stat_bonus,
        );
        world.objects.add_components(&oid, cs);
    }
    handle_action(&mut world, 1, &action_body(near, 0));
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "first cast running"
    );
    drain(&mut a_rx);

    // 2 s in: reuse (1.2 s) expired, cast (~4 s) still running. Select the far
    // monster and click the same skill again.
    advance_world(&mut world, 20);
    handle_action(&mut world, 1, &action_body(far, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        matches!(
            world.objects.get_component::<QueuedAction>(&3001),
            Some(QueuedAction::Skill { skill_id: 1177, .. })
        ),
        "second click parked in the queue slot (casting {:?}, reuses {:?})",
        world
            .objects
            .get_component::<Casting>(&3001)
            .map(|c| c.0.skill_id),
        world
            .objects
            .get_component::<crate::model::components::Reuses>(&3001)
    );
    drain(&mut a_rx);

    // Cast end → replay → walk-to-cast toward the far monster.
    advance_world(&mut world, 40);
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(crate::model::PlayerIntent::Cast { target_object_id, .. })) if *target_object_id == far
        ) || world
            .objects
            .get_component::<Casting>(&3001)
            .is_some_and(|c| c.0.target_object_id == far),
        "replayed click acts on the far monster (intent {:?}, queued {:?}, casting {:?})",
        world.objects.get_component::<Intent>(&3001),
        world.objects.get_component::<QueuedAction>(&3001),
        world.objects.get_component::<Casting>(&3001)
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "MoveToPawn broadcast for the walk"
    );

    advance_world(&mut world, 40);
    let cast = world
        .objects
        .get_component::<Casting>(&3001)
        .expect("cast started after the walk");
    assert_eq!(
        cast.0.target_object_id, far,
        "cast aimed at the far monster"
    );
}

/// G19 `Transformation` effect: casting "Transform Doom Wraith" (618, real
/// dist data → `transformationId` 2) polymorphs the caster — durable
/// transform id + display id, the transform template's granted skill (586,
/// Rolling Attack) in the `SkillBook` — exactly like `//transform` (the admin
/// runtime `admin_ride_bike_transforms_and_reverts` covers), but reached
/// through the ordinary cast pipeline instead of the GM command. Re-casting
/// while transformed is refused (`ConditionPlayerCanTransform`'s
/// already-polymorphed leg); `handle_buff_expire` on the `TRANSFORM` buff
/// reverts everything, matching the buff's `BuffExpire`/dispel/death path.
#[test]
fn transformation_skill_polymorphs_and_reverts_on_expiry() {
    let (mut world, ..) = test_world();
    // The full real datapack, not just `transforms`/`skill_data`: `checkUseConditions`'
    // MP/HP prechecks need a real class template's hp/mp tables (`for_test`'s
    // `player_templates` is empty, so a level-1 dummy char would compute 0 max HP).
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    let mut rx = ingame_player_access(&mut world, 1, 5001, 0);
    drain(&mut rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5001)
        .unwrap()
        .0
        .insert(618, 1);
    // A second, independent transform skill: Doom Wraith's own 4 h reuse means
    // re-clicking *it* is refused by the reuse gate long before any condition
    // runs (Java checks `isSkillDisabled` at `useMagic`'s top, well ahead of
    // `checkCondition`), so the already-polymorphed refusal below has to come
    // from a skill that is not on cooldown.
    world
        .objects
        .get_component_mut::<SkillBook>(&5001)
        .unwrap()
        .0
        .insert(617, 1);
    let base_run = world
        .objects
        .get_component::<Speeds>(&5001)
        .unwrap()
        .run_spd;

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(618, false));
    advance_world(&mut world, 40); // hitTime 2500 ms + finish, well inside 40 × 100 ms ticks

    {
        let p = world.objects.get_component::<Player>(&5001).unwrap();
        assert_eq!(
            p.transform_id, 2,
            "transformed into Doom Wraith (transformationId 2)"
        );
        assert_eq!(p.transform_display_id, 2, "display id == id on this dist");
    }
    assert!(
        world
            .objects
            .get_component::<SkillBook>(&5001)
            .unwrap()
            .0
            .contains_key(&586),
        "transform's granted skill (Rolling Attack) present"
    );
    assert_ne!(
        world
            .objects
            .get_component::<Speeds>(&5001)
            .unwrap()
            .run_spd,
        base_run,
        "run speed overridden by the transform template"
    );
    assert_eq!(
        pbuffs(&world, 5001),
        1,
        "lands as one TRANSFORM buff (drives the expiry-based revert)"
    );

    // Transforming *again* while transformed is refused by the `CanTransform`
    // skill condition (G34 S1 — it used to be an inline block in `cast.rs`).
    // Java's `Skill.checkCondition` sends the handler's own message **and** the
    // generic "cannot be used due to unsuitable terms"; the inline version sent
    // only the first, which is the behaviour change this assertion pins.
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(617, false));
    let refused = drain(&mut rx);
    assert!(
        has_system_message(
            &refused,
            server_packets::sm_ids::YOU_ALREADY_POLYMORPHED_AND_CANNOT_POLYMORPH_AGAIN
        ),
        "already-polymorphed refusal sent"
    );
    assert!(
        has_system_message(
            &refused,
            server_packets::sm_ids::S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS
        ),
        "…followed by the generic condition refusal, as Java sends both"
    );
    assert!(
        !world.objects.has_component::<Casting>(&5001),
        "the refused click never starts a cast"
    );

    // Expiry (natural `BuffExpire`, dispel, or death all route through this).
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, 5001, 618);
    // A TvT entrant cannot transform at all — Java's `isRegisteredOnEvent()`
    // leg, which sends a plain text line rather than a SystemMessage.
    {
        let p = world.objects.get_component::<Player>(&5001).unwrap();
        assert_eq!(p.transform_id, 0, "reverted before the event check");
    }
    // Clear the reuse the first cast left, or the refusal below would be the
    // cooldown talking rather than the event gate (it was, on the first
    // attempt at this test — the sabotage caught it).
    if let Some(r) = world
        .objects
        .get_component_mut::<crate::model::components::Reuses>(&5001)
    {
        r.0.clear();
    }
    world.events.tvt.player_list.push(5001);
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(618, false));
    assert!(
        !world.objects.has_component::<Casting>(&5001),
        "an event entrant's transform click never starts a cast"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5001)
            .unwrap()
            .transform_id,
        0,
        "…and they stay themselves"
    );
    world.events.tvt.player_list.clear();
    let p = world.objects.get_component::<Player>(&5001).unwrap();
    assert_eq!(p.transform_id, 0, "reverted");
    assert_eq!(p.transform_display_id, 0, "display cleared");
    assert!(
        !world
            .objects
            .get_component::<SkillBook>(&5001)
            .unwrap()
            .0
            .contains_key(&586),
        "transform skill removed"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&5001)
            .unwrap()
            .run_spd,
        base_run,
        "run speed restored"
    );
    assert_eq!(pbuffs(&world, 5001), 0, "buff cleared");

    // Mounted refusal: a strider rider casting the scroll gets SM 2063
    // (`ConditionPlayerCanTransform`'s `isMounted()` leg — a real mount sets
    // `mount_type`, not `transform_id`, so the polymorph leg alone misses it).
    crate::game_loop::admin::mounts::mount_player(&mut world, 5001, 12526, 1);
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(618, false));
    let refused = drain(&mut rx);
    assert!(
        has_system_message(
            &refused,
            server_packets::sm_ids::YOU_CANNOT_TRANSFORM_WHILE_RIDING_A_PET
        ),
        "mounted refusal sent"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5001)
            .unwrap()
            .transform_id,
        0,
        "no transform applied while mounted"
    );
    crate::game_loop::admin::mounts::dismount(&mut world, 5001);

    // Sitting refusal — note *which* message. `Player.useMagic` turns away
    // every skill from a seated caster with SM 31, and it does so long before
    // it reaches `usedSkill.checkCondition`, so `ConditionPlayerCanTransform`'s
    // own `isSitting()` leg (SM 2283) is unreachable down the cast path in Java
    // too; it only answers for transforms that skip `useMagic` (the admin
    // `//transform`, gated separately). This assertion named 2283 while the
    // blanket seated-cast gate was still missing from the port.
    world
        .objects
        .get_component_mut::<Player>(&5001)
        .unwrap()
        .sitting = true;
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(618, false));
    let refused = drain(&mut rx);
    assert!(
        has_system_message(
            &refused,
            server_packets::sm_ids::YOU_CANNOT_USE_ACTIONS_AND_SKILLS_WHILE_THE_CHARACTER_IS_SITTING
        ),
        "sitting refusal sent"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5001)
            .unwrap()
            .transform_id,
        0,
        "no transform applied while sitting"
    );
}

/// G19 `MpConsumePerLevel` effect: the fighter-toggle upkeep half of
/// "Accuracy" (256, real dist data — `<effects>` carries both a real
/// `+3 Accuracy` `StatModifier`, already landing before this slice, and an
/// `MpConsumePerLevel` that previously fell through unrecognised, so the
/// toggle was a free buff). Toggling it on lands the stat *and* starts a
/// periodic MP drain; running the MP pool dry switches the toggle back off
/// (Java's `false`-return-cancels-a-toggle path, SM 140) and reverts the
/// stat, exactly like the DoT/ManaDamOverTime tick chain this effect shares.
#[test]
fn mp_consume_per_level_toggle_drains_mp_and_self_deactivates() {
    let (mut world, ..) = test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    let mut rx = ingame_player_access(&mut world, 1, 5101, 0);
    drain(&mut rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5101)
        .unwrap()
        .0
        .insert(256, 1);
    let base_accuracy = pcs(&world, 5101).accuracy;
    let mp_before = pvit(&world, 5101).cur_mp;

    // Toggle on: instant (no cast bar) — `+3 Accuracy` lands immediately.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(256, false));
    assert_eq!(pbuffs(&world, 5101), 1, "toggle landed as one buff");
    assert_eq!(
        pcs(&world, 5101).accuracy,
        base_accuracy + 3,
        "Accuracy +3 (DIFF) applied"
    );
    assert_eq!(
        pvit(&world, 5101).cur_mp,
        mp_before,
        "no MP deducted at cast time (pre-existing gap, not this slice)"
    );

    // One upkeep tick: `power(0.4) * ticksMultiplier(5 × 666 / 1000 = 3.33) ≈ 1.332` MP.
    advance_world(&mut world, 40); // interval = (5 × 666) / 100 = 33 ticks
    let mp_after_one_tick = pvit(&world, 5101).cur_mp;
    assert!(
        (mp_before - mp_after_one_tick - 1.332).abs() < 1e-6,
        "first tick drained ~1.332 MP: {mp_before} -> {mp_after_one_tick}"
    );
    assert_eq!(
        pbuffs(&world, 5101),
        1,
        "toggle still up (MP not exhausted yet)"
    );

    // Drain the rest of the pool: the toggle self-deactivates the moment a
    // tick's drain would exceed current MP (Java's `false` return).
    drain(&mut rx);
    advance_world(
        &mut world,
        40 * (mp_after_one_tick / 1.332).ceil() as u64 + 40,
    );
    assert_eq!(
        pbuffs(&world, 5101),
        0,
        "toggle switched itself off once MP ran dry"
    );
    assert_eq!(
        pcs(&world, 5101).accuracy,
        base_accuracy,
        "Accuracy reverted"
    );
    let packets = drain(&mut rx);
    assert!(
        has_system_message(
            &packets,
            server_packets::sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP
        ),
        "deactivation SystemMessage sent"
    );
}

/// G19 `ShieldDefence`/`ShieldDefenceRate` effects: Shield Mastery (153, a
/// widely-learned shield-user passive) level 4 pumps both `+60% ShieldDefence`
/// and `+100% ShieldDefenceRate` (`PER` mode) on top of the equipped shield's
/// own `sDef`/`rShld` — previously silently dropped (`ShieldDefenceRate` was
/// already parsed but never folded into `game_loop::combat::shield_stats`,
/// which read the raw item stat directly; `ShieldDefence` wasn't even parsed).
/// Passives fold into `StatModifiers` at `Player::from_char`, so this checks
/// `combat::combatant`'s finalized `shield_def`/`shield_rate` directly rather
/// than going through the cast pipeline.
#[test]
fn shield_mastery_passive_raises_shield_block_stats() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let paperdoll = |object_id, item_id, slot| crate::character::ItemRow {
        object_id,
        item_id,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: slot,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    };
    // Item 628 "Hoplon" (sDef 128, rShld 20) in LHand (slot 7).
    let mut bare = dummy_char(5201, "Bare");
    bare.items = vec![paperdoll(1, 628, 7)];
    let bare_bundle = Player::from_char(&world.data, &bare);
    bare_bundle.spawn_into(&mut world);
    let bare_shield = crate::game_loop::combat::combatant(&world, 5201).expect("bare combatant");
    assert_eq!(
        bare_shield.shield_def, 128.0,
        "no skill: raw sDef unchanged"
    );

    let mut masted = dummy_char(5202, "Masted");
    masted.items = vec![paperdoll(2, 628, 7)];
    masted.skills = vec![(153, 4, 0)];
    let masted_bundle = Player::from_char(&world.data, &masted);
    masted_bundle.spawn_into(&mut world);
    let masted_shield =
        crate::game_loop::combat::combatant(&world, 5202).expect("masted combatant");
    assert_eq!(
        masted_shield.shield_def,
        128.0 * 1.6,
        "Shield Mastery lvl4: sDef × 1.6 (+60% PER)"
    );
    assert!(
        (masted_shield.shield_rate - bare_shield.shield_rate * 2.0).abs() < 1e-9,
        "Shield Mastery lvl4: rShld × 2.0 (+100% PER), CON bonus cancels in the ratio: {} vs {}",
        masted_shield.shield_rate,
        bare_shield.shield_rate
    );
}

/// G19 `PhysicalAttackRange`: Archery (431, real dist data — `<weaponType>
/// BOW</weaponType>`-conditioned, `+50 DIFF`) raises the reach of a bow past
/// its own `pAtkRange` (item 14 "Bow", 500). Before this slice the effect
/// name fell through to nothing (`combat.atk_range` read the equipped
/// weapon's raw range with no stat modifier applied at all — the same gap
/// `ShieldDefenceRate` had before an earlier slice). The weapon condition
/// gate is also checked: unarmed, Archery must be inert.
#[test]
fn archery_passive_raises_bow_attack_range() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let paperdoll = |object_id, item_id, slot| crate::character::ItemRow {
        object_id,
        item_id,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: slot,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    };
    // Item 14 "Bow" (pAtkRange 500) in RHand (slot 5, two-handed).
    let mut bare = dummy_char(5401, "Bare Bow");
    bare.items = vec![paperdoll(1, 14, 5)];
    let bare_bundle = Player::from_char(&world.data, &bare);
    assert_eq!(
        bare_bundle.combat.atk_range, 500,
        "no skill: raw bow range unchanged"
    );

    let mut archer = dummy_char(5402, "Archer");
    archer.items = vec![paperdoll(2, 14, 5)];
    archer.skills = vec![(431, 1, 0)]; // Archery
    let archer_bundle = Player::from_char(&world.data, &archer);
    assert_eq!(
        archer_bundle.combat.atk_range, 550,
        "Archery: +50 bow range"
    );

    // The `<weaponType>BOW</weaponType>` condition: unarmed, Archery must not
    // leak its bonus onto the bare-fist range.
    let unarmed_bare = dummy_char(5403, "Unarmed Bare");
    let unarmed_bare_bundle = Player::from_char(&world.data, &unarmed_bare);
    let mut unarmed_archer = dummy_char(5404, "Unarmed Archer");
    unarmed_archer.skills = vec![(431, 1, 0)];
    let unarmed_archer_bundle = Player::from_char(&world.data, &unarmed_archer);
    assert_eq!(
        unarmed_archer_bundle.combat.atk_range, unarmed_bare_bundle.combat.atk_range,
        "Archery is inert without a bow equipped"
    );
}

/// G19 `FatalBlowRate`: Assassination (432, real dist data, unconditioned
/// `PER +3`) raises `Stat::BlowRate`, the caster-side multiplier
/// `formulas::calc_blow_success` folds into the Backstab/Lethal-Blow-style
/// landing roll. Before this slice `Stat::BlowRate` didn't exist and the
/// formula had no term for it at all — the skill was a passive that did
/// nothing. Checked at the `StatModifiers` level (the formula's own boundary
/// shift is covered by `formulas::tests::blow_success_rate_cap_and_threshold`).
#[test]
fn assassination_passive_raises_blow_rate_stat() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let bare = dummy_char(5501, "Bare");
    let bare_bundle = Player::from_char(&world.data, &bare);
    assert_eq!(
        bare_bundle
            .stat_modifiers
            .mul
            .get(&crate::model::stats::Stat::BlowRate),
        None,
        "no skill: no modifier at all"
    );

    let mut assassin = dummy_char(5502, "Assassin");
    assassin.skills = vec![(432, 1, 0)]; // Assassination
    let assassin_bundle = Player::from_char(&world.data, &assassin);
    let mul = assassin_bundle
        .stat_modifiers
        .mul
        .get(&crate::model::stats::Stat::BlowRate)
        .copied()
        .unwrap_or(0.0);
    assert!(
        (mul - 1.03).abs() < 1e-9,
        "Assassination: +3% PER -> ×1.03, got {mul}"
    );
}

/// G19 `EnlargeSlot`: Expand Inventory (1372, real dist data, no `<type>` so
/// it defaults to `Stat::InventoryNormal`) raises the inventory-slot cap
/// `UserInfo`'s INVENTORY_LIMIT block reports. Passives fold into
/// `StatModifiers` at `Player::from_char`, matching the `ShieldDefence` test
/// above; before this slice the effect fell through and the skill did
/// nothing (the client always showed the bare race-based cap).
#[test]
fn enlarge_slot_expand_inventory_raises_reported_cap() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);
    let cfg = world.cfg.character.clone();

    let mut bare = dummy_char(5301, "Bare");
    bare.race = 0; // human, not a dwarf
    let bare_bundle = Player::from_char(&world.data, &bare);
    let bare_view = bare_bundle.view();
    let bare_limit = crate::model::finalize(
        bare_view.mods,
        crate::model::stats::Stat::InventoryNormal,
        cfg.inventory_limit(0) as f64,
    ) as i32;
    assert_eq!(
        bare_limit,
        cfg.inventory_limit(0),
        "no skill: unmodified race base"
    );

    let mut expanded = dummy_char(5302, "Expanded");
    expanded.race = 0;
    expanded.skills = vec![(1372, 3, 0)]; // Expand Inventory lvl3, real dist: +18
    let expanded_bundle = Player::from_char(&world.data, &expanded);
    let expanded_view = expanded_bundle.view();
    let expanded_limit = crate::model::finalize(
        expanded_view.mods,
        crate::model::stats::Stat::InventoryNormal,
        cfg.inventory_limit(0) as f64,
    ) as i32;
    assert_eq!(
        expanded_limit,
        cfg.inventory_limit(0) + 18,
        "Expand Inventory lvl3: +18 slots"
    );
}

/// `ExStorageMaxCount`'s field order. The protocol-110 client reads the
/// quest-tab capacity from the **9th** int and the belt/`EnlargeSlot` bonus
/// from the 10th; stock L2J Mobius writes those two the other way round, so
/// the client picked up the always-zero belt field and the inventory's Quest
/// Items tab reported "N/0" while the real limit landed in a field it ignores.
/// The Java reference tree carries the same swap.
///
/// Also pins the three capacity numbers that `Character.ini` owns —
/// `MaximumSlotsForNoDwarf`, `MaximumSlotsForGMPlayer`, and
/// `MaximumSlotsForQuestItems` — so raising a key in the ini moves what the
/// client is told.
#[test]
fn ex_storage_max_count_reports_the_configured_capacities() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);
    let cfg = world.cfg.character.clone();

    // `ex(0x2F)` = one opcode byte + the two-byte ex id, then 12 ints.
    let fields = |bytes: &[u8]| -> Vec<i32> {
        bytes[3..]
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    };

    let mut bare = dummy_char(5303, "Bare");
    bare.race = 0; // human, not a dwarf
    let bare_bundle = Player::from_char(&world.data, &bare);
    let f = fields(&crate::network::enter_world::ex_storage_max_count(
        0,
        false,
        &cfg,
        bare_bundle.view().mods,
    ));
    assert_eq!(f.len(), 12, "12 ints, as Java writes");
    assert_eq!(f[0], cfg.inventory_limit(0), "inventory slots");
    assert_eq!(
        f[8], cfg.inventory_max_quest_items,
        "quest-items capacity is the 9th int, not the 10th — a 0 here is the \
         Quest Items tab showing N/0"
    );
    assert_eq!(f[9], 0, "no belt, no Expand Inventory: no extra slots");

    // A GM gets `MaximumSlotsForGMPlayer` instead of the race base, exactly as
    // `Player.getInventoryLimit()` orders its branches.
    let gm = fields(&crate::network::enter_world::ex_storage_max_count(
        0,
        true,
        &cfg,
        bare_bundle.view().mods,
    ));
    assert_eq!(gm[0], cfg.inventory_max_gm, "GM bag");
    assert_ne!(cfg.inventory_max_gm, cfg.inventory_limit(0));

    // Expand Inventory lvl3 (+18): the total grows *and* the bonus is reported
    // on its own, which is what Java's `_inventoryExtraSlots` carries.
    let mut expanded = dummy_char(5304, "Expanded");
    expanded.race = 0;
    expanded.skills = vec![(1372, 3, 0)];
    let expanded_bundle = Player::from_char(&world.data, &expanded);
    let e = fields(&crate::network::enter_world::ex_storage_max_count(
        0,
        false,
        &cfg,
        expanded_bundle.view().mods,
    ));
    assert_eq!(
        e[0],
        cfg.inventory_limit(0) + 18,
        "total includes the bonus"
    );
    assert_eq!(e[9], 18, "and the bonus is reported separately");
    assert_eq!(
        e[8], cfg.inventory_max_quest_items,
        "quest tab is untouched"
    );
}

/// G19 `HealPercent` effect: "Revival" (181, real dist data — a self-target,
/// 100%-power heal) restores HP on cast. Before this slice every
/// `HealPercent` skill — including the priest staples Miracle, Benediction,
/// Restore Life, Touch of Life — parsed to an empty effect list, so the cast
/// landed but healed nothing. Self-cast rather than on another player only
/// because that's what this particular skill is — see
/// `enemy_not_targets_a_friendly_player_but_refuses_a_hostile_one` for
/// Restore Life healing someone else (its `targetType ENEMY_NOT`).
#[test]
fn heal_percent_restores_a_share_of_max_hp() {
    let (mut world, ..) = test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    let mut rx = ingame_player_access(&mut world, 1, 5301, 0);
    drain(&mut rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5301)
        .unwrap()
        .0
        .insert(181, 1);

    let max_hp = pvit(&world, 5301).max_hp as f64;
    // Revival's own `<condition name="RemainHpPer">` is `LESS 10` on the
    // caster — it is the emergency self-heal, and Java refuses it above 10 %.
    // The fixture used to sit at 20 % and cast anyway, because no condition
    // was enforced (G34 S1).
    let low = max_hp * 0.05;
    world
        .objects
        .get_component_mut::<Vitals>(&5301)
        .unwrap()
        .cur_hp = low;

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(181, false));
    advance_world(&mut world, 40); // hitTime 1500 ms, well inside 40 × 100 ms ticks

    assert!(
        (pvit(&world, 5301).cur_hp - max_hp).abs() < 1e-6,
        "Revival (power 100) fully restores HP: {} (max {})",
        pvit(&world, 5301).cur_hp,
        max_hp
    );
    let packets = drain(&mut rx);
    assert!(
        has_system_message(&packets, server_packets::sm_ids::S1_HP_HAS_BEEN_RESTORED),
        "self-cast heal SystemMessage sent"
    );
}

/// G19 `TargetType::EnemyNot` — the "any friendly selected target" gate
/// `targethandlers/EnemyNot.java` backs (34 instances, 4 learnable, including
/// "Restore Life" itself), found unmodeled while testing `HealPercent`: it
/// fell through to `Other`, and `use_magic_on` silently no-ops on that (no
/// packet, no cast). Restore Life (1258, real dist data, level 1 heals 15%
/// of max HP) now lands on a friendly player.
#[test]
fn enemy_not_targets_a_friendly_player() {
    let (mut world, ..) = test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    // Restore Life is `isMagic`, so its cast time scales by the caster's
    // *magic* casting speed — a level-1 default (Human Fighter, class 0) has
    // a near-zero one, stretching an 8 s cast into minutes. Use a Mystic
    // (class 10, as the real-data spellcraft test does) for a sane cast time.
    let mut chr = dummy_char(5401, "Healer");
    chr.class_id = 10;
    chr.base_class_id = 10;
    chr.skills = vec![(1258, 1, 0)];
    let bundle = Player::from_char(&world.data, &chr);
    let (out_tx, mut a_rx) = tokio::sync::mpsc::unbounded_channel();
    let s = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![])
        .into_entering(bundle);
    let (session, bundle) = s.into_ingame();
    bundle.spawn_into(&mut world);
    world.clients.insert(1, ClientSession::InGame(session));
    // `chr.cur_mp` gets clamped to the class's computed max MP at spawn (59
    // for a level-1 Mystic) — below level-1 Restore Life's 80 MP cost, so
    // bump it directly rather than fighting the clamp through `CharData`.
    world
        .objects
        .get_component_mut::<Vitals>(&5401)
        .unwrap()
        .cur_mp = 200.0;

    let mut b_rx = ingame_player_access(&mut world, 2, 5402, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    // Distinct position from the caster's default (1, 2, 3) — same-position
    // casters/targets aren't exercised elsewhere in this suite.
    world
        .objects
        .get_component_mut::<crate::model::components::Position>(&5402)
        .unwrap()
        .x = 50;

    let max_hp = pvit(&world, 5402).max_hp as f64;
    let half = max_hp * 0.5;
    world
        .objects
        .get_component_mut::<Vitals>(&5402)
        .unwrap()
        .cur_hp = half;

    handle_action(&mut world, 1, &action_body(5402, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1258, false));
    advance_world(&mut world, 200); // hitTime 8000 ms at a Mystic's casting speed

    let expected = half + max_hp * 0.15; // level 1 power = 15%, no overheal clamp hit
    assert!(
        (pvit(&world, 5402).cur_hp - expected).abs() < 1e-6,
        "healed a friendly player 15% of max HP: {} (expected {})",
        pvit(&world, 5402).cur_hp,
        expected
    );
    let b_packets = drain(&mut b_rx);
    assert!(
        has_system_message(
            &b_packets,
            server_packets::sm_ids::S2_HP_HAS_BEEN_RESTORED_BY_C1
        ),
        "target sees the heal SystemMessage"
    );
}

/// The other half of `TargetType::EnemyNot`: the exact inverse of `Enemy`'s
/// gate, so a hostile target is refused outright (no ctrl/force-use override,
/// unlike `Enemy`).
#[test]
fn enemy_not_refuses_a_hostile_target() {
    let (mut world, ..) = test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    let mut a_rx = ingame_player_access(&mut world, 1, 5411, 0);
    drain(&mut a_rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5411)
        .unwrap()
        .0
        .insert(1258, 1);
    world
        .objects
        .get_component_mut::<Vitals>(&5411)
        .unwrap()
        .cur_mp = 200.0;

    // A real dist monster (20001 Gremlin) is auto-attackable.
    let npc_oid = NPC_OID + 1;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 20001, 50, 0, 0, 1000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(
        world.data.npc_data.get(20001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1258, false));
    assert!(
        !world.objects.has_component::<Casting>(&5411),
        "refused: no cast against a hostile target"
    );
}

/// G19 `FocusMomentum` effect: Sonic Focus (8, real dist data, level 1 grants
/// max 1 charge) builds "Force" — previously silently dropped, so the
/// warrior Force-builder skills did nothing. First cast lands (0 → 1,
/// already at the level-1 cap) with SM 324 ("reached maximum capacity");
/// recasting at the cap is refused outright (no further gain, same SM).
#[test]
fn focus_momentum_builds_force_and_refuses_past_the_cap() {
    let (mut world, ..) = test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    let mut rx = ingame_player_access(&mut world, 1, 5501, 0);
    drain(&mut rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5501)
        .unwrap()
        .0
        .insert(8, 1);
    // `EquipWeapon` DUAL/DUALBLUNT/SWORD/BLUNT — Long Sword satisfies it.
    arm(&mut world, 5501, 2);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(8, false));
    advance_world(&mut world, 20); // hitTime 900 ms
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5501)
            .unwrap()
            .charges,
        1,
        "0 -> 1, the level-1 cap"
    );
    let packets = drain(&mut rx);
    assert!(
        has_system_message(
            &packets,
            server_packets::sm_ids::YOUR_FORCE_HAS_REACHED_MAXIMUM_CAPACITY
        ),
        "reached-cap SystemMessage on the capping cast"
    );

    // Off cooldown or not, the skill is castable again; already at the cap,
    // it refuses outright (no charge change, same SM, no further gain).
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(8, false));
    advance_world(&mut world, 20);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5501)
            .unwrap()
            .charges,
        1,
        "still capped at 1"
    );
}

/// Java's ten-minute Force decay (`ResetChargesTask`): charges clear on their
/// own, the clock restarts on every gain, and it stops when the pool empties.
///
/// The port cannot cancel a scheduled task, so "restart" is a generation
/// counter — a stale task fires and does nothing. Each leg below fails
/// differently, so they are asserted separately.
#[test]
fn force_charges_decay_after_ten_minutes_and_the_clock_restarts_on_a_gain() {
    /// 600 000 ms at 100 ms a tick.
    const DECAY: u64 = 6_000;

    let charges = |w: &World| w.objects.get_component::<Player>(&5541).unwrap().charges;
    let gain = |w: &mut World| {
        handle_request_magic_skill_use(w, 1, &magic_skill_use_body(8, false));
        advance_world(w, 20);
    };
    let build = || {
        let (mut world, ..) = test_world();
        world.data = crate::data::GameData::load_from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/"
        ));
        let rx = ingame_player_access(&mut world, 1, 5541, 0);
        world
            .objects
            .get_component_mut::<SkillBook>(&5541)
            .unwrap()
            .0
            .insert(8, 1);
        arm(&mut world, 5541, 2);
        (world, rx)
    };

    // A charge sitting untouched for ten minutes clears itself.
    let (mut world, _rx) = build();
    gain(&mut world);
    assert_eq!(charges(&world), 1, "sanity: Sonic Focus charged");
    advance_world(&mut world, DECAY - 100);
    assert_eq!(charges(&world), 1, "still there just before the deadline");
    advance_world(&mut world, 200);
    assert_eq!(charges(&world), 0, "and gone just after it");

    // A second gain restarts the clock: the *first* task still fires at its
    // original deadline and must do nothing, or the pool empties early.
    let (mut world, _rx) = build();
    gain(&mut world);
    advance_world(&mut world, DECAY / 2);
    world
        .objects
        .get_component_mut::<Player>(&5541)
        .unwrap()
        .charges = 0; // clear the cap so the next cast really charges
    gain(&mut world);
    assert_eq!(charges(&world), 1, "recharged");
    advance_world(&mut world, DECAY / 2 + 100);
    assert_eq!(
        charges(&world),
        1,
        "the first task's deadline passed, but it was superseded"
    );
    advance_world(&mut world, DECAY / 2);
    assert_eq!(charges(&world), 0, "the second one still expires on time");
}

/// G19 `EnergyAttack` effect: Sonic Blaster (6, real dist data, level 1:
/// power 369, criticalChance 15, `chargeConsume` 2 — a *skill-level* tag)
/// spends Force for bonus physical damage — previously silently dropped, so
/// every "Sonic"/"Force" attack skill (Double Sonic Slash, Sonic Blaster,
/// Sonic Buster, Force Burst/Storm/Blaster, …) did nothing on cast.
#[test]
fn energy_attack_spends_charges_for_bonus_damage() {
    let (mut world, ..) = test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    let mut a_rx = ingame_player_access(&mut world, 1, 5511, 0);
    drain(&mut a_rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5511)
        .unwrap()
        .0
        .insert(6, 1);
    // Pre-set Force rather than grinding Sonic Focus casts (level 1 only
    // grants 1, below Sonic Blaster's own chargeConsume of 2) — this effect
    // only cares about the charges already on hand, not how they got there.
    world
        .objects
        .get_component_mut::<Player>(&5511)
        .unwrap()
        .charges = 5;
    // `EquipWeapon` DUAL/SWORD/BLUNT/DUALBLUNT (the `EnergySaved` 2 is already
    // satisfied by the 5 charges above).
    arm(&mut world, 5511, 2);

    let npc_oid = NPC_OID + 2;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 20001, 50, 0, 0, 100_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(
        world.data.npc_data.get(20001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    let npc_hp_before = pvit(&world, npc_oid).cur_hp;
    let p_atk = pcs(&world, 5511).p_atk;
    let p_def = crate::game_loop::combat::combatant(&world, npc_oid)
        .unwrap()
        .p_def;

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(6, false));
    advance_world(&mut world, 40); // hitTime 1900 ms

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5511)
            .unwrap()
            .charges,
        3,
        "5 - chargeConsume(2)"
    );
    // `77 * ((pAtk * levelMod) + power) / pDef * energyChargesBoost(1 + 2×0.1)`,
    // times 2 on a crit. The crit roll isn't pinned here: `advance_world`
    // also runs the spawned NPC's periodic AI think tick, which draws from
    // the same `forced_rolls` queue as the cast's own crit check, so which
    // one actually consumes a pushed value depends on tick timing rather
    // than cast order — tolerate either outcome instead of fighting that.
    let level = world.objects.get_component::<Player>(&5511).unwrap().level;
    let level_mod = crate::model::formulas::level_mod(level);
    let base = (77.0 * ((p_atk * level_mod) + 369.0) / p_def.max(1.0)) * 1.2;
    let actual_damage = npc_hp_before - pvit(&world, npc_oid).cur_hp;
    assert!(
        (actual_damage - base).abs() < 1e-6 || (actual_damage - base * 2.0).abs() < 1e-6,
        "Sonic Blaster damage with the Force bonus: {actual_damage} (expected {base} or {} on a crit)",
        base * 2.0
    );
}

/// G19 `Lethal` effect: Lethal Blow (344, real dist data — pairs `FatalBlow`
/// with `fullLethal` 0 / `halfLethal` 15) sets the target's CP to 1 on a
/// landed half-kill — previously dropped, so Backstab/Lethal Blow/Deadly
/// Blow/Critical Blow/Lethal Shot dealt their (already-ported) damage but
/// never rolled the bonus kill chance. Force-targets a second player (`ctrl`)
/// so the assertion (CP → 1) is decoupled from FatalBlow's own HP damage,
/// which lands first in the same effect list. Every `world.roll` is flooded
/// with `0` — not just the half-kill roll, since `FatalBlow`'s own land/crit
/// rolls (and the spawned NPC's periodic AI think tick, present in other
/// tests in this file) also draw from the same queue ahead of it.
#[test]
fn lethal_half_kill_sets_player_cp_to_1() {
    let (mut world, ..) = test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    let mut a_rx = ingame_player_access(&mut world, 1, 5601, 0);
    let mut b_rx = ingame_player_access(&mut world, 2, 5602, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    world
        .objects
        .get_component_mut::<crate::model::components::Position>(&5602)
        .unwrap()
        .x = 30;
    world
        .objects
        .get_component_mut::<SkillBook>(&5601)
        .unwrap()
        .0
        .insert(344, 1);
    // `EquipWeapon` DAGGER/DUALDAGGER — Bone Dagger satisfies it.
    arm(&mut world, 5601, 11);
    world
        .objects
        .get_component_mut::<Vitals>(&5601)
        .unwrap()
        .cur_mp = 200.0;
    // A level-1 default has a tiny naked CP pool (possibly already ≤ 1) —
    // give the target real headroom so "drained to 1" is an observable drop.
    {
        let pv = world
            .objects
            .get_component_mut::<crate::model::components::PlayerVitals>(&5602)
            .unwrap();
        pv.max_cp = 50;
        pv.cur_cp = 50.0;
    }

    handle_action(&mut world, 1, &action_body(5602, 0));
    drain(&mut a_rx);
    world.forced_rolls.extend(std::iter::repeat_n(0, 30));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(344, true)); // ctrl: force a clean player target
    advance_world(&mut world, 30); // hitTime 1080 ms

    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::PlayerVitals>(&5602)
            .unwrap()
            .cur_cp,
        1.0,
        "half-kill drains CP to 1"
    );
    let b_packets = drain(&mut b_rx);
    assert!(
        has_system_message(&b_packets, server_packets::sm_ids::HALF_KILL)
            && has_system_message(&b_packets, server_packets::sm_ids::YOUR_CP_WAS_DRAINED_BECAUSE_YOU_WERE_HIT_WITH_A_HALF_KILL_SKILL),
        "target sees both Half-Kill SystemMessages"
    );
}

/// `Lethal.instant`'s closing `calcCounterAttack` — "No matter if lethal
/// succeeded or not, its reflected", in Java's own words.
///
/// Java has exactly two `calcCounterAttack` call sites: `reduceCurrentHp`
/// (once per damaging skill) and `Lethal.instant`. Every lethal carrier on
/// this dist pairs Lethal with a damage effect, so both fire and the victim
/// counters **twice** for one cast. That double is the observable, and it is
/// Java's behaviour, not a duplicate to suppress — before this, only one
/// counter fired.
#[test]
fn a_lethal_cast_counters_twice_because_java_rolls_it_from_both_sites() {
    let (mut world, ..) = test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    let mut a_rx = ingame_player_access(&mut world, 1, 5621, 0);
    let mut b_rx = ingame_player_access(&mut world, 2, 5622, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    world
        .objects
        .get_component_mut::<crate::model::components::Position>(&5622)
        .unwrap()
        .x = 30;
    world
        .objects
        .get_component_mut::<SkillBook>(&5621)
        .unwrap()
        .0
        .insert(344, 1);
    arm(&mut world, 5621, 11);
    world
        .objects
        .get_component_mut::<Vitals>(&5621)
        .unwrap()
        .cur_mp = 200.0;
    // Survive the blow: the counter bails on a dead target, so a one-shot
    // victim would silently lose the second roll and look like a missing
    // call site.
    {
        let v = world.objects.get_component_mut::<Vitals>(&5622).unwrap();
        v.max_hp = 100_000;
        v.cur_hp = 100_000.0;
    }
    // The counter needs a real p_atk behind it or the damage rounds to 0 and
    // the whole thing bails before sending anything.
    if let Some(cs) = world
        .objects
        .get_component_mut::<crate::model::components::CombatStats>(&5622)
    {
        cs.p_atk = 500.0;
    }
    // `VENGEANCE_SKILL_PHYSICAL_DAMAGE` at 100 — the counter always rolls.
    {
        let mut mods = world
            .objects
            .get_component::<crate::model::components::StatModifiers>(&5622)
            .cloned()
            .unwrap_or_default();
        mods.add.insert(
            crate::model::stats::Stat::VengeanceSkillPhysicalDamage,
            100.0,
        );
        world.objects.add_components(&5622, mods);
    }

    handle_action(&mut world, 1, &action_body(5622, 0));
    drain(&mut a_rx);
    drain(&mut b_rx);
    world.forced_rolls.extend(std::iter::repeat_n(0, 40));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(344, true));
    advance_world(&mut world, 30);

    let counters = drain(&mut b_rx)
        .iter()
        .filter(|p| {
            has_system_message(
                std::slice::from_ref(*p),
                server_packets::sm_ids::YOU_COUNTERED_C1_S_ATTACK,
            )
        })
        .count();
    assert_eq!(
        counters, 2,
        "one counter from the damage effect and one from Lethal — Java's two \
         call sites; a single counter means the Lethal one is missing"
    );
}

/// The other half of `Lethal`: raid bosses are immune (`isLethalable()`),
/// mirroring the same raid-immunity check `Mute`'s cast-interrupt already
/// has. A real dist raid boss (3404 "Tracker Captain Sharuk", level 23 — well
/// under Lethal Blow's magicLevel 76, so the separate level gate doesn't
/// interfere) takes `FatalBlow`'s damage but keeps its Force/CP-equivalent
/// untouched: HP drops from the blow, but never gets forced to 1 or halved
/// again on top of that by a landed Lethal.
#[test]
fn lethal_spares_a_raid_boss() {
    let (mut world, ..) = test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    let mut a_rx = ingame_player_access(&mut world, 1, 5611, 0);
    drain(&mut a_rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5611)
        .unwrap()
        .0
        .insert(344, 1);
    // `EquipWeapon` DAGGER/DUALDAGGER — Bone Dagger satisfies it.
    arm(&mut world, 5611, 11);
    world
        .objects
        .get_component_mut::<Vitals>(&5611)
        .unwrap()
        .cur_mp = 200.0;

    let npc_oid = NPC_OID + 10;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 3404, 30, 0, 0, 1_000_000, 100);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(
        world.data.npc_data.get(3404).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    assert!(
        world.data.npc_data.get(3404).unwrap().is_raid(),
        "sanity: 3404 really is a RaidBoss template"
    );
    let hp_before = pvit(&world, npc_oid).cur_hp;

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);
    world.forced_rolls.extend(std::iter::repeat_n(0, 30));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(344, false));
    advance_world(&mut world, 30); // hitTime 1080 ms

    let hp_after = pvit(&world, npc_oid).cur_hp;
    assert!(
        hp_after < hp_before,
        "sanity: FatalBlow's own damage still landed"
    );
    assert!(
        hp_after > hp_before * 0.4,
        "a landed Lethal half-kill would have halved *whatever HP FatalBlow left* on top \
         of the blow's own damage — well below 50% of the pre-cast HP; immunity keeps it \
         from ever compounding like that: {hp_before} -> {hp_after}"
    );
}

/// G19 `AttackTrait` effect: "Detect Beast Weakness" (80, real dist data,
/// self-target, `abnormalTime` 600 s) lands as an icon-only buff — before
/// this slice the effect name wasn't recognized, so the whole "Detect
/// <Category> Weakness" family (Insect/Beast/Animal/Dragon/Plant, Eye of
/// Hunter/Slayer) fell through the empty-effects guard and never landed at
/// all (no icon, nothing). It's a peculiar effect to port: even on the real
/// Java server it's functionally inert (see the doc comment on
/// `SkillEffect::AttackTrait` — `Formulas.calcWeaknessBonus` needs a
/// matching NPC-side `DefenceTrait`, and nothing in this datapack ever sets
/// one), so this test only checks that the buff *lands and expires*
/// correctly, not any damage change.
#[test]
fn attack_trait_lands_as_an_icon_only_buff() {
    let (mut world, ..) = test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    let mut rx = ingame_player_access(&mut world, 1, 5701, 0);
    drain(&mut rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5701)
        .unwrap()
        .0
        .insert(80, 1);
    world
        .objects
        .get_component_mut::<Vitals>(&5701)
        .unwrap()
        .cur_mp = 200.0;

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(80, false));
    advance_world(&mut world, 30); // hitTime 1500 ms
    assert_eq!(
        pbuffs(&world, 5701),
        1,
        "Detect Beast Weakness lands as one buff"
    );

    world.tick += 6000; // abnormalTime 600 s
    apply_due_tasks(&mut world);
    assert_eq!(pbuffs(&world, 5701), 0, "expires after abnormalTime");
}

/// G19 `DamageBlock` effect: Celestial Shield (1418, real dist data,
/// `isMagic`, self-targetable via `targetType TARGET`) sets `HP_BLOCK` +
/// `MP_BLOCK` — previously silently dropped, so this and the other four
/// short invulnerability shields (Flames of Invincibility, Dance of Medusa,
/// Sonic/Force Barrier) did nothing on cast. `HP_BLOCK`'s real consumer is
/// `game_loop::combat::apply_physical_damage`'s new choke-point gate: a huge
/// non-DoT hit does nothing while the shield is up, but a DoT tick (Java's
/// one exemption besides a skill's own HP cost) still lands.
#[test]
fn damage_block_refuses_incoming_hp_damage_except_a_dot() {
    let (mut world, ..) = test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    // isMagic: needs a real magic casting speed, like the EnemyNot slice's
    // own test found — a level-1 default (Fighter, class 0) stretches the
    // nominal 4 s cast into minutes.
    let mut chr = dummy_char(5801, "Shielded");
    chr.class_id = 10;
    chr.base_class_id = 10;
    chr.skills = vec![(1418, 1, 0)];
    let bundle = Player::from_char(&world.data, &chr);
    let (out_tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let s = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![])
        .into_entering(bundle);
    let (session, bundle) = s.into_ingame();
    bundle.spawn_into(&mut world);
    world.clients.insert(1, ClientSession::InGame(session));
    world
        .objects
        .get_component_mut::<Vitals>(&5801)
        .unwrap()
        .cur_mp = 200.0;
    drain(&mut rx);

    handle_action(&mut world, 1, &action_body(5801, 0)); // self-target
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1418, false));
    advance_world(&mut world, 60); // hitTime 4000 ms

    assert_eq!(
        pbuffs(&world, 5801),
        1,
        "Celestial Shield lands as one buff"
    );
    assert_eq!(
        world.objects.get_component::<Buffs>(&5801).unwrap().0[0].effect_flags
            & (crate::model::skill::effect_flag::HP_BLOCK
                | crate::model::skill::effect_flag::MP_BLOCK),
        crate::model::skill::effect_flag::HP_BLOCK | crate::model::skill::effect_flag::MP_BLOCK,
        "both HP_BLOCK and MP_BLOCK set"
    );

    // Zero CP so a landed hit reduces HP directly, not absorbed by CP first
    // (the synthetic attacker oid below reads as "playable", which triggers
    // that absorb branch in `player_receive_damage`).
    world
        .objects
        .get_component_mut::<crate::model::components::PlayerVitals>(&5801)
        .unwrap()
        .cur_cp = 0.0;
    let hp_before = pvit(&world, 5801).cur_hp;
    // A huge non-DoT hit: refused outright.
    crate::game_loop::combat::apply_physical_damage(
        &mut world, 90001, 5801, 999_999.0, false, false,
    );
    assert_eq!(
        pvit(&world, 5801).cur_hp,
        hp_before,
        "HP_BLOCK refuses a normal hit"
    );
    // A DoT tick: Java's one exemption besides a skill's own HP cost.
    crate::game_loop::combat::apply_physical_damage(&mut world, 90001, 5801, 5.0, true, false);
    assert_eq!(
        pvit(&world, 5801).cur_hp,
        hp_before - 5.0,
        "a DoT tick still lands through HP_BLOCK"
    );
}

/// **A trait resistance makes you harder to execute.** Java's `Lethal` scales
/// both kill chances by `calcAttributeBonus * calcGeneralTraitBonus(…, false)`;
/// the port applied only the attribute half, with a comment saying the trait
/// half "stays unported with the trait system". The trait system landed, and
/// nothing existed to make that claim fail.
#[test]
fn a_trait_resistance_lowers_the_lethal_chance() {
    use crate::model::skill::{SkillEffect, TraitType};

    let lethal = |resist: bool| {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);
        let max = world
            .objects
            .get_component::<Vitals>(&npc_oid)
            .unwrap()
            .max_hp as f64;
        world
            .objects
            .get_component_mut::<Vitals>(&npc_oid)
            .unwrap()
            .cur_hp = max;

        let mut skill = world.data.skill_data.get(1160, 1).expect("fixture").clone();
        skill.id = 9900;
        skill.magic_type = 0;
        skill.activate_rate = -1;
        skill.magic_level = 80; // well above the mob, so the level gate passes
        skill.trait_type = TraitType::Shock;
        skill.effects = vec![SkillEffect::Lethal {
            full_lethal: 100.0,
            half_lethal: 0.0,
        }];
        if resist {
            crate::game_loop::skills::effects::merge_defence_traits(
                &mut world,
                npc_oid,
                &[(TraitType::Shock, 0.5)],
            );
        }
        // magic-crit throwaway, then the full-lethal roll: 60 is under the
        // unresisted 100 but over the halved 50.
        world.forced_rolls.extend([0, 60]);
        crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
        world
            .objects
            .get_component::<Vitals>(&npc_oid)
            .unwrap()
            .cur_hp
    };

    assert_eq!(lethal(false), 1.0, "unresisted, the lethal lands");
    assert!(
        lethal(true) > 1.0,
        "a 50% SHOCK resistance halves the kill chance and the same roll misses"
    );
}

/// G34 S4 sub-slice 2 — `PHYSICAL_SKILL_POWER` / `MAGICAL_SKILL_POWER`, the
/// last multiplier a skill's damage passes through. Focus Skill Mastery (334)
/// is the learnable physical source; the magical one is item-only here.
///
/// Java applies the physical stat from each `PhysicalAttack`-family *effect
/// handler* but the magical one from **inside `calcMagicDam`** — so every
/// caller of that function gets it, HpDrain included, even though HpDrain's own
/// handler never mentions the stat. Both damage paths are asserted for that
/// reason.
///
/// **One world, four measurements.** An earlier version built a fresh dist-
/// loaded world per case; four `GameData::load_from` calls made it the slowest
/// test in the suite and it started timing out under parallel load. The mob's
/// HP is reset between measurements instead, and its pool is far deeper than
/// any hit under test — otherwise the clamp, not the multiplier, is what the
/// assertion measures.
#[test]
fn the_skill_power_stats_scale_finished_skill_damage() {
    use crate::model::components::StatModifiers;
    use crate::model::stats::Stat;

    const POWER_STRIKE: i32 = 3;
    const WIND_STRIKE: i32 = 1177;
    const CASTER: i32 = 6401;
    let npc = crate::model::npc::FIRST_NPC_OBJECT_ID + 7801;

    let (mut world, ..) = test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let _rx = ingame_player_access(&mut world, 1, CASTER, 0);
    add_test_npc(&mut world, npc, 20001, "Monster", 20, 0, 0, 0);

    let measure = |world: &mut World, skill_id: i32, stat: Option<(Stat, f64)>| -> f64 {
        let mut mods = world
            .objects
            .get_component::<StatModifiers>(&CASTER)
            .cloned()
            .expect("modifiers");
        mods.mul.remove(&Stat::PhysicalSkillPower);
        mods.mul.remove(&Stat::MagicalSkillPower);
        if let Some((s, v)) = stat {
            mods.mul.insert(s, v);
        }
        world.objects.add_components(&CASTER, mods);
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&npc) {
            v.max_hp = 1_000_000;
            v.cur_hp = 1_000_000.0;
        }
        let skill = world
            .data
            .skill_data
            .get(skill_id, 1)
            .cloned()
            .expect("skill");
        world.forced_rolls.clear();
        world.forced_rolls.extend([50; 12]);
        crate::game_loop::skills::effects::apply_skill_effects(world, CASTER, npc, &skill);
        1_000_000.0 - pvit_npc_hp(world, npc)
    };

    let plain = measure(&mut world, POWER_STRIKE, None);
    let boosted = measure(
        &mut world,
        POWER_STRIKE,
        Some((Stat::PhysicalSkillPower, 2.0)),
    );
    assert!(plain > 0.0, "the skill deals damage at all: {plain}");
    assert!(
        (boosted - plain * 2.0).abs() < 1.0,
        "PHYSICAL_SKILL_POWER ×2 doubles it ({plain} → {boosted})"
    );

    let plain_m = measure(&mut world, WIND_STRIKE, None);
    let boosted_m = measure(
        &mut world,
        WIND_STRIKE,
        Some((Stat::MagicalSkillPower, 2.0)),
    );
    assert!(plain_m > 0.0, "the nuke deals damage at all: {plain_m}");
    assert!(
        (boosted_m - plain_m * 2.0).abs() < 1.0,
        "MAGICAL_SKILL_POWER ×2 doubles it ({plain_m} → {boosted_m})"
    );
}

fn pvit_npc_hp(world: &World, oid: i32) -> f64 {
    world
        .objects
        .get_component::<Vitals>(&oid)
        .map(|v| v.cur_hp)
        .unwrap_or(0.0)
}

/// **A beneficial skill cast near a fighting mob pulls it onto the caster** —
/// Java's "On Skill See logic", the rule that makes healing the tank aggro the
/// healer.
///
/// Two halves are being checked. The witness scan is Java's
/// `forEachVisibleObjectInRange(player, Npc.class, 1000, …)`, so the mob reacts
/// to a cast it was never a target of; and the hate is
/// `effectPoint * 150 / (level + 7)` credited to the caster. Until 2026-08-05
/// the port only notified the skill's own targets, so neither happened.
#[test]
fn healing_beside_a_fighting_mob_draws_its_hate_onto_the_healer() {
    use crate::model::npc::{AggroList, NpcAi, NpcIntention};

    let (mut world, ..) = cast_test_world();
    // The healer, the tank it heals, and a mob already fighting the tank.
    let mut healer_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let _tank_rx = ingame_caster(&mut world, 2, 3002, 60, 0);
    let mob = NPC_OID;
    add_test_npc(&mut world, mob, 20001, "Monster", 10, 80, 0, 0);
    world
        .objects
        .get_component_mut::<NpcAi>(&mob)
        .unwrap()
        .intention = NpcIntention::Attack;
    // The mob's target is the tank — never the healer.
    crate::game_loop::minions::add_hate(&mut world, mob, 3002, 500.0);
    assert_eq!(
        world
            .objects
            .get_component::<AggroList>(&mob)
            .and_then(|a| a.most_hated()),
        Some(3002),
        "the mob is on the tank to begin with"
    );
    let hate_on_healer = |w: &World| -> f64 {
        w.objects
            .get_component::<AggroList>(&mob)
            .and_then(|a| a.0.get(&3001).map(|i| i.hate))
            .unwrap_or(0.0)
    };
    assert_eq!(
        hate_on_healer(&world),
        0.0,
        "the healer has drawn no hate yet"
    );

    // Heal the tank. The mob is not a target of the cast at all.
    set_target(&mut world, 1, 3001, Some(3002));
    drain(&mut healer_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
    advance_ticks(&mut world, 45);

    let after = hate_on_healer(&world);
    assert!(
        after > 0.0,
        "the mob noticed the heal and now hates the healer ({after})"
    );
}

/// **A non-combat transform cannot walk into range to cast** — Java's "while
/// flying there is no move to cast" (`checkTransformed(t -> !t.isCombat())` →
/// SM 748 + `ActionFailed`, `maybeMoveToPawn` returning true).
///
/// The discrimination is the point: a COMBAT form walks as normal. Asserting
/// only the refusal would pass just as well if the gate ignored the flag and
/// refused everyone.
#[test]
fn a_non_combat_transform_is_refused_a_walk_to_cast() {
    use crate::model::Player;

    let refused_for = |transform_id: i32| -> bool {
        let (mut world, ..) = cast_test_world();
        world.data.transforms = crate::data::TransformData::load_from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/"
        ));
        let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        // A target far enough that the cast needs a walk.
        let _target_rx = ingame_caster(&mut world, 2, 3002, 5000, 0);
        world
            .objects
            .get_component_mut::<Player>(&3001)
            .unwrap()
            .transform_id = transform_id;
        set_target(&mut world, 1, 3001, Some(3002));
        drain(&mut rx);
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
        sm_ids_of(&drain(&mut rx)).contains(
            &crate::network::server_packets::sm_ids::THE_DISTANCE_IS_TOO_FAR_AND_SO_THE_CASTING_HAS_BEEN_CANCELLED,
        )
    };

    // Transform 101 is NON_COMBAT on this dist; 1 is COMBAT.
    assert!(
        refused_for(101),
        "a non-combat form is refused the walk-to-cast"
    );
    assert!(
        !refused_for(1),
        "a COMBAT form walks as normal — the gate reads the flag, not merely 'is transformed'"
    );
    assert!(!refused_for(0), "and an untransformed player is unaffected");
}

/// A **passive** carrying a rate effect (`MagicMpCost` / `Reuse`) has to reach
/// the rate tables.
///
/// `conditioned_passive_buffs` keeps only `StatModifier` effects, so a passive
/// whose *only* effect is a rate produced no buff and reached no table at all
/// — Inner Rhythm (428) advertised "−10 % MP for songs and dances" and did
/// exactly nothing. Six more learnable passives were inert the same way
/// (164, 435, 436, 615, 945, 1527), plus the Clarity/Apella/boss-jewel item
/// skills.
#[test]
fn a_passive_rate_skill_actually_discounts_the_skill_it_names() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    /// Inner Rhythm: `MagicMpCost -10 PER`, `magicType 3` (songs and dances).
    const INNER_RHYTHM: i32 = 428;
    /// Champion Song — `magicType 3`, `mpConsume 60`.
    const SONG: i32 = 364;
    /// Wind Strike — an ordinary magic skill, a different `magicType`.
    const WIND_STRIKE: i32 = 1177;

    let (mut world, ..) = test_world();
    world.data = crate::data::GameData::load_from(DIST);
    let _rx = ingame_player_access(&mut world, 1, 7711, 0);

    let song = world.data.skill_data.get(SONG, 1).expect("song").clone();
    let nuke = world
        .data
        .skill_data
        .get(WIND_STRIKE, 1)
        .expect("wind strike")
        .clone();
    let mp = |w: &World, s: &crate::model::skill::Skill| {
        crate::game_loop::skills::effects::mp_consume_for(w, 7711, s)
    };

    let (song_before, nuke_before) = (mp(&world, &song), mp(&world, &nuke));
    assert_eq!(song_before, 60, "sanity: the dist still prices it at 60");

    world
        .objects
        .get_component_mut::<SkillBook>(&7711)
        .unwrap()
        .0
        .insert(INNER_RHYTHM, 1);
    crate::game_loop::passive_skills::refresh_conditioned_passives(&mut world, 7711);

    assert_eq!(
        mp(&world, &song),
        54,
        "−10 % of 60 — the discount the skill description promises"
    );
    // The bucket is the *skill's* magicType, so a nuke is untouched: a rate
    // that leaked across buckets would look like a working discount too.
    assert_eq!(
        mp(&world, &nuke),
        nuke_before,
        "a skill outside magicType 3 is unaffected"
    );

    // Unlearning restores it — the rebuild is wholesale, not a one-way merge.
    world
        .objects
        .get_component_mut::<SkillBook>(&7711)
        .unwrap()
        .0
        .remove(&INNER_RHYTHM);
    crate::game_loop::passive_skills::refresh_conditioned_passives(&mut world, 7711);
    assert_eq!(mp(&world, &song), song_before, "and it comes back off");
}

/// End to end, the way a GM actually tests it: `//add_skill 428 1`, then cast
/// a real dance and measure the MP that actually left the bar.
///
/// The unit-level test above calls `mp_consume_for` directly, one layer below
/// the cast path — this one goes through the admin command and
/// `RequestMagicSkillUse` so nothing between them can quietly skip the rate.
#[test]
fn inner_rhythm_discounts_a_real_cast_driven_through_the_admin_command() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    const CHAMPION_SONG: i32 = 364;
    const OID: i32 = 7721;

    let cast_cost = |learn_inner_rhythm: bool| -> f64 {
        let (mut world, ..) = test_world();
        world.data = crate::data::GameData::load_from(DIST);
        let _rx = ingame_player_access(&mut world, 1, OID, 100);
        // A real MP pool rather than a hand-set one: `//add_skill` recomputes
        // max vitals, and a level-1 character's ~40 MP would be clamped back
        // under the song's 60 anyway.
        crate::game_loop::death::set_level(&mut world, OID, 78);
        {
            let max = world.objects.get_component::<Vitals>(&OID).unwrap().max_mp;
            let v = world.objects.get_component_mut::<Vitals>(&OID).unwrap();
            v.cur_mp = max as f64;
        }
        crate::game_loop::admin::use_admin_command(
            &mut world,
            1,
            &format!("admin_add_skill {CHAMPION_SONG} 1"),
            false,
        );
        if learn_inner_rhythm {
            crate::game_loop::admin::use_admin_command(
                &mut world,
                1,
                "admin_add_skill 428 1",
                false,
            );
        }
        let before = world.objects.get_component::<Vitals>(&OID).unwrap().cur_mp;
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(CHAMPION_SONG, false));
        advance_world(&mut world, 40); // hitTime 2500 ms
        let after = world.objects.get_component::<Vitals>(&OID).unwrap().cur_mp;
        before - after
    };

    let plain = cast_cost(false);
    assert_eq!(
        plain, 60.0,
        "sanity: the song really costs its listed 60 MP"
    );
    assert_eq!(
        cast_cost(true),
        54.0,
        "Inner Rhythm takes 10 % off the MP that actually leaves the bar"
    );
}

/// `NpcBody` targeting: the `OpSweeper` spoil gate belongs to the Sweeper
/// family alone — on this dist only Sweeper 42 carries the condition. A
/// corpse skill without the `Sweeper` effect (Corpse Burst 1155, Corpse Life
/// Drain 1151, Life Scavenge 46, Corpse Plague 103 — all learnable) casts on
/// any dead NPC, spoiled or not; Sweeper is still refused at cast time on an
/// unspoiled corpse and passes on the caster's own spoil.
#[test]
fn npc_body_spoil_gate_only_for_sweeper() {
    use crate::model::components::Position;
    use crate::model::skill::{Skill, SkillEffect, TargetType};
    use crate::network::server_packets::sm_ids;

    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    add_test_npc(&mut world, npc_oid, 40778, "Monster", 5, 50, 0, 0);
    world
        .objects
        .get_component_mut::<Vitals>(&npc_oid)
        .unwrap()
        .dead = true;

    let corpse_burst = Skill {
        id: 1155,
        target_type: TargetType::NpcBody,
        effects: vec![SkillEffect::MagicalAttack { power: 10.0 }],
        ..Default::default()
    };
    let sweeper = Skill {
        id: 42,
        target_type: TargetType::NpcBody,
        effects: vec![SkillEffect::Sweeper, SkillEffect::ConsumeBody],
        ..Default::default()
    };
    let caster = world
        .objects
        .get_component::<crate::model::Player>(&3001)
        .unwrap();
    let pos = *world.objects.get_component::<Position>(&3001).unwrap();

    assert_eq!(
        skills::cast::resolve_cast_target(
            &world,
            caster,
            &pos,
            Some(npc_oid),
            &corpse_burst,
            false,
            false
        ),
        Ok(npc_oid),
        "a corpse skill without the Sweeper effect casts on an unspoiled corpse"
    );
    assert_eq!(
        skills::cast::resolve_cast_target(
            &world,
            caster,
            &pos,
            Some(npc_oid),
            &sweeper,
            false,
            false
        ),
        Err(sm_ids::SWEEPER_FAILED_TARGET_NOT_SPOILED),
        "Sweeper is still refused on an unspoiled corpse at cast time"
    );
    world
        .objects
        .get_component_mut::<crate::model::npc::Npc>(&npc_oid)
        .unwrap()
        .spoiler_object_id = 3001;
    let caster = world
        .objects
        .get_component::<crate::model::Player>(&3001)
        .unwrap();
    assert_eq!(
        skills::cast::resolve_cast_target(
            &world,
            caster,
            &pos,
            Some(npc_oid),
            &sweeper,
            false,
            false
        ),
        Ok(npc_oid),
        "the caster's own spoil passes the Sweeper gate"
    );
}
