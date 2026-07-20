use super::*;

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
            requires_item: false,
        },
    );
    data.skill_data.insert_for_test(Skill {
        without_action: false,
        item_consume_id: 0,
        item_consume_count: 0,
        id: 91,
        level: 1,
        name: "Defense Aura".into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Self_,
        magic_type: 1,
        magic_level: 0,        effect_point: 0,
        cast_range: 0,
        effect_range: 0,
        hit_time: 400,
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
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects: vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::PhysicalDefence,
            mode: StatModifierType::Per,
            amount: 8.0,
            armor_condition: 0,
            weapon_condition: 0,
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
    assert!((bundle.combat.p_def - 75.2).abs() < 1e-9, "naked P.Def before any buff: {}", bundle.combat.p_def);

    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel();
    let s = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![])
        .into_entering(bundle);
    let (session, bundle) = s.into_ingame();
    bundle.spawn_into(&mut world.objects);
    world.clients.insert(1, ClientSession::InGame(session));

    // --- Learn: RequestAcquireSkill(id=91, level=1, type=CLASS). ---
    let mut w = PacketWriter::new();
    w.write_i32(91);
    w.write_i32(1);
    w.write_i32(cp::RequestAcquireSkill::CLASS);
    handle_request_acquire_skill(&mut world, 1, &w.into_bytes());

    assert_eq!(world.objects.get_component::<SkillBook>(&2001).unwrap().0.get(&91), Some(&1));
    assert_eq!(world.objects.get_component::<crate::model::Player>(&2001).expect("player").sp, 100, "200 SP - levelUpSp(100)");
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::ACQUIRE_SKILL_DONE);
    assert_eq!(out_rx.try_recv().unwrap()[0], 0x5F); // SkillList
    let _ = out_rx.try_recv().unwrap(); // AcquireSkillList
    let _ = out_rx.try_recv().unwrap(); // UserInfo

    // --- Cast: RequestMagicSkillUse(91). ---
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));

    assert!(world.objects.has_component::<Casting>(&2001));
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE); // initial MP consume
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_USE);
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::SYSTEM_MESSAGE); // YOU_USE_S1
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::SETUP_GAUGE);
    assert_eq!(pvit(&world, 2001).cur_mp, 49.0, "50 - mpInitialConsume(1)");

    // --- Launch: hit = max(400/factor(1.0) − cancel(500), 0) = 0 ms, so
    // the launch task is already due; the finish follows 500 ms later.
    apply_due_tasks(&mut world);
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);
    assert!(world.objects.get_component::<Casting>(&2001).is_some_and(|c| c.0.launched));

    world.tick += 5;
    apply_due_tasks(&mut world);
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE); // final MP consume
    assert_eq!(out_rx.try_recv().unwrap()[0], 0x85); // AbnormalStatusUpdate
    let _ = out_rx.try_recv().unwrap(); // UserInfo (buff changed pDef → broadcastUserInfo)

    {
        assert!(!world.objects.has_component::<Casting>(&2001), "coolTime 0 frees the cast slot inline");
        assert_eq!(pbuffs(&world, 2001), 1);
        assert!((pcs(&world, 2001).p_def - 75.2 * 1.08).abs() < 1e-9, "75.2 × 1.08 (PhysicalDefence +8%): {}", pcs(&world, 2001).p_def);
    }
    assert_eq!(pvit(&world, 2001).cur_mp, 45.0, "49 - mpConsume(4)");

    // --- Advance past expiry (abnormalTime 20 s = 200 ticks) and drain again. ---
    world.tick += 200;
    apply_due_tasks(&mut world);

    let _ = out_rx.try_recv().unwrap(); // UserInfo (buff removal reverted pDef → broadcastUserInfo)
    let expired = out_rx.try_recv().unwrap();
    assert_eq!(expired[0], 0x85);
    assert_eq!(&expired[1..3], &[0, 0], "AbnormalStatusUpdate count = 0 once expired");

    assert_eq!(pbuffs(&world, 2001), 0);
    assert!((pcs(&world, 2001).p_def - 75.2).abs() < 1e-9, "P.Def restored after the buff expired: {}", pcs(&world, 2001).p_def);
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
                loc: if slot.is_some() { "PAPERDOLL".into() } else { "INVENTORY".into() },
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
    chr.skills = data.skill_trees.initial_skills(class_id); // 118, 163, 214, 1177, 1216

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
    assert_eq!(c.m_atk_spd, 499, "cast speed (333 × Spellcraft 1.5 in a robe)");

    // --- Now drive the real enter-world refresh tail (expertise + conditioned
    // passives, in the order `handle_enter_world` runs them) and confirm the
    // in-world stats still match — this is where the reported 349 shows up. ---
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);
    b.spawn_into(&mut world.objects);
    super::expertise::refresh_expertise_penalty(&mut world, 4212);
    super::passive_skills::refresh_conditioned_passives(&mut world, 4212);
    assert_eq!(pcs(&world, 4212).m_atk_spd, 499, "cast speed after enter-world refresh tail");
    assert_eq!(pcs(&world, 4212).p_atk as i32, 2, "p.atk after enter-world refresh tail");
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
    chr.items = vec![paperdoll(1001, 6, 5), paperdoll(1002, 425, 6), paperdoll(1003, 461, 11)];
    // The two autoGet mystic passives.
    chr.skills = vec![(163, 1), (118, 1)];
    let bundle = Player::from_char(&world.data, &chr);
    // `from_char` (Java `restoreCharData`/`addSkill`) already folds the robe
    // passives in: Spellcraft's MAGIC branch (+50%) applies, while Magician's
    // Movement stays inert (its −20% atk-speed penalty is gated to non-robe).
    assert_eq!(bundle.combat.m_atk_spd, 499, "Spellcraft: 333 × 1.5 in a robe");
    assert_eq!(bundle.combat.p_atk_spd, 384, "Magician's Movement stays inert in a robe");
    bundle.spawn_into(&mut world.objects);

    // Take the robe legs off: the MAGIC condition now fails (bare legs read as
    // NONE), so `refresh_conditioned_passives` drops Spellcraft's bonus.
    world.objects.get_component_mut::<crate::model::inventory::Inventory>(&4211).unwrap().unequip_item(1003);
    super::passive_skills::refresh_conditioned_passives(&mut world, 4211);
    assert_eq!(pcs(&world, 4211).m_atk_spd, 333, "no robe → Spellcraft bonus gone");
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
                loc: if slot.is_some() { "PAPERDOLL".into() } else { "INVENTORY".into() },
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
    chr.skills = data.skill_trees.all_available_skills(class_id, 7, &std::collections::HashMap::new(), true, true);
    assert!(chr.skills.iter().any(|&(id, _)| id == 163), "level-7 mystic has Spellcraft (163)");
    assert!(chr.skills.iter().any(|&(id, _)| id == 249), "level-7 mystic has Weapon Mastery (249)");

    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

    // 1. Character select: the delevel filter (`filter_skills_on_select` →
    // `maybe_skill_remove_on_delevel`), replicated on `chr.skills`.
    let skills_before = chr.skills.len();
    {
        let mut skills_map: std::collections::HashMap<i32, i32> = chr.skills.iter().copied().collect();
        super::death::maybe_skill_remove_on_delevel(&world, chr.object_id, chr.class_id, chr.level, &mut skills_map);
        chr.skills = skills_map.into_iter().collect();
    }
    assert!(chr.skills.iter().any(|&(id, _)| id == 163), "delevel filter kept Spellcraft (163)");
    assert_eq!(chr.skills.len(), skills_before, "delevel filter removed no skills at level 7");

    // 2. Build the player from the (filtered) select data.
    let b = Player::from_char(&world.data, &chr);
    assert_eq!(b.combat.m_atk_spd, 499, "cast speed after from_char (Spellcraft ×1.5 in a robe)");
    b.spawn_into(&mut world.objects);

    // 3. Enter-world refresh tail, in `handle_enter_world` order.
    super::expertise::refresh_expertise_penalty(&mut world, 4213);
    assert_eq!(pcs(&world, 4213).m_atk_spd, 499, "cast speed after expertise refresh");
    super::passive_skills::refresh_conditioned_passives(&mut world, 4213);
    assert_eq!(pcs(&world, 4213).m_atk_spd, 499, "cast speed after conditioned-passive refresh");
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
    chr.items = vec![paperdoll(1001, 6, 5), paperdoll(1002, 425, 6), paperdoll(1003, 461, 11)];
    // Spellcraft (163, getLevel 1) + Magician's Movement (118, getLevel 1) +
    // Shield (1040, getLevel 7) that a level-5 delevel strips.
    chr.skills = vec![(163, 1), (118, 1), (1040, 1)];

    // The select-time filter (what `filter_skills_on_select` runs).
    let mut skills: std::collections::HashMap<i32, i32> = chr.skills.iter().copied().collect();
    let changes = super::death::maybe_skill_remove_on_delevel(&world, chr.object_id, chr.class_id, chr.level, &mut skills);
    assert!(changes.iter().any(|&(id, a)| id == 1040 && a.is_none()), "Shield stripped at level 5");
    chr.skills = skills.into_iter().collect();

    // `from_char` on the corrected skills: Shield gone, Spellcraft kept, so the
    // casting-speed bonus is folded in and the first UserInfo is 499 (not 349).
    let bundle = Player::from_char(&world.data, &chr);
    assert!(!bundle.skills.0.contains_key(&1040), "Shield removed from the book");
    assert!(bundle.skills.0.contains_key(&163), "Spellcraft survives");
    assert_eq!(bundle.combat.m_atk_spd, 499, "Spellcraft's casting-speed bonus intact");
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
    chr.items = vec![paperdoll(1001, 6, 5), paperdoll(1002, 425, 6), paperdoll(1003, 461, 11)];
    // Spellcraft (163, getLevel 1) + Weapon Mastery (249, getLevel 7, passive +m.atk).
    chr.skills = vec![(163, 1), (249, 1)];
    let bundle = Player::from_char(&world.data, &chr);
    let m_atk_with_mastery = bundle.combat.m_atk;
    bundle.spawn_into(&mut world.objects);

    // Level-down check strips Weapon Mastery (5 < 7) and re-folds the stats.
    super::death::check_player_skills(&mut world, 4214);
    assert!(!world.objects.get_component::<SkillBook>(&4214).unwrap().0.contains_key(&249), "Weapon Mastery removed");
    assert!(world.objects.get_component::<SkillBook>(&4214).unwrap().0.contains_key(&163), "Spellcraft kept");
    // Weapon Mastery's +m.atk is gone; Spellcraft's casting-speed bonus (499)
    // is now un-corrupted by 249 and correctly folded from the reduced book.
    assert!(pcs(&world, 4214).m_atk < m_atk_with_mastery, "removing Weapon Mastery lowered m.atk");
    assert_eq!(pcs(&world, 4214).m_atk_spd, 499, "recompute re-folds only the surviving passives");
}

/// `AutoLearnSkills`: `rewardSkills` must grant every reachable class skill,
/// not just autoGet ones — and only autoGet ones when the flag is off.
#[test]
fn auto_learn_grants_all_reachable_class_skills() {
    use crate::data::skill_tree::SkillLearn;

    let mk_data = || {
        let mut data = GameData::for_test();
        data.player_templates = crate::data::PlayerTemplateData::from_vec(vec![human_fighter_template()]);
        // Class 0: a level-1 autoGet skill + a non-autoGet class skill (id 91,
        // levels 1@getLevel5 and 2@getLevel10).
        data.skill_trees.insert_for_test(0, SkillLearn { skill_id: 1000, skill_level: 1, name: "Auto".into(), get_level: 1, level_up_sp: 0, auto_get: true, requires_item: false });
        data.skill_trees.insert_for_test(0, SkillLearn { skill_id: 91, skill_level: 1, name: "Class1".into(), get_level: 5, level_up_sp: 100, auto_get: false, requires_item: false });
        data.skill_trees.insert_for_test(0, SkillLearn { skill_id: 91, skill_level: 2, name: "Class2".into(), get_level: 10, level_up_sp: 200, auto_get: false, requires_item: false });
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
        bundle.spawn_into(&mut world.objects);
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
        assert_eq!(book.get(&91), Some(&1), "class skill auto-learned at level 5");
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
        assert_eq!(book.get(&91), None, "class skill NOT auto-learned when flag is off");
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
        data.player_templates = crate::data::PlayerTemplateData::from_vec(vec![human_fighter_template()]);
        // Skill 91: level 1 @ getLevel 20, level 2 @ getLevel 40.
        data.skill_trees.insert_for_test(0, SkillLearn { skill_id: 91, skill_level: 1, name: "S1".into(), get_level: 20, level_up_sp: 100, auto_get: false, requires_item: false });
        data.skill_trees.insert_for_test(0, SkillLearn { skill_id: 91, skill_level: 2, name: "S2".into(), get_level: 40, level_up_sp: 200, auto_get: false, requires_item: false });
        // Skill 92: a single level @ getLevel 7 — used to show the strict flag
        // vs the 9-level grace at low character levels.
        data.skill_trees.insert_for_test(0, SkillLearn { skill_id: 92, skill_level: 1, name: "S3".into(), get_level: 7, level_up_sp: 100, auto_get: false, requires_item: false });
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
        chr.skills = vec![(91, 2), (92, 1)];
        let bundle = Player::from_char(&world.data, &chr);
        let (link_out, _r) = tokio::sync::mpsc::unbounded_channel();
        let s = Session::new(1, link_out, "127.0.0.1:1".parse().unwrap())
            .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
            .into_lobby(vec![])
            .into_entering(bundle);
        let (_session, bundle) = s.into_ingame();
        bundle.spawn_into(&mut world.objects);

        world.objects.get_component_mut::<crate::model::Player>(&2001).unwrap().level = new_level;
        super::death::check_player_skills(&mut world, 2001);
        world.objects.get_component::<SkillBook>(&2001).unwrap().0.get(&skill_id).copied()
    };

    // --- Default strict mode (StrictDelevelSkillRemoval = true). ---
    // 40 → 30: skill 91 @ level 2 (getLevel 40) is out of range → downgrade to
    // the highest reachable level (1, getLevel 20).
    assert_eq!(run(true, true, 30, 91), Some(1), "downgraded to the highest reachable level");
    // 40 → 5: even level 1 (getLevel 20) is out of range → removed.
    assert_eq!(run(true, true, 5, 91), None, "removed when no level is reachable");
    // Skill 92 (getLevel 7) at level 1: strict strips it (1 < 7)…
    assert_eq!(run(true, true, 1, 92), None, "strict removes a getLevel-7 skill at level 1");

    // --- Non-strict (Java 9-level grace). ---
    // …but the 9-level grace keeps it (1 ≥ 7 − 9).
    assert_eq!(run(true, false, 1, 92), Some(1), "grace keeps a getLevel-7 skill at level 1");

    // Flag off: kept despite being out of range, regardless of strictness.
    assert_eq!(run(false, true, 5, 91), Some(2), "kept when DecreaseSkillOnDelevel is off");
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
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::INVALID_TARGET);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(!world.objects.has_component::<Casting>(&3001));

    // With ctrl: ExRotation (face target) + initial-MP StatusUpdate +
    // MagicSkillUse to everyone, YOU_USE_S1 + SetupGauge to the caster.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::EX);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE);
    let msu = a_rx.try_recv().unwrap();
    assert_eq!(msu[0], server_packets::opcodes::MAGIC_SKILL_USE);
    assert_eq!(
        i32::from_le_bytes(msu[25..29].try_into().unwrap()),
        -1,
        "ungrouped skill must send reuse group -1 (0 greys every icon client-side)"
    );
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::YOU_USE_S1);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::SETUP_GAUGE);
    assert!(a_rx.try_recv().is_err());
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::EX);
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_USE);
    assert!(b_rx.try_recv().is_err());
    assert_eq!(pvit(&world, 3001).cur_mp, 48.0, "50 - mpInitialConsume(2)");

    // Launch at hit = 4000/1.0 − 500 = 3500 ms = 35 ticks.
    world.tick += 35;
    apply_due_tasks(&mut world);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);

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
    let damage = formulas::calc_magic_dam(m_atk, m_def, 12.0, false, 1.0, formulas::MagicFailure::None);
    assert!(damage > 100.0, "sanity: the nuke must overflow B's CP ({damage})");
    {
        let b = pvit(&world, 3002);
        let bcp = pcp(&world, 3002);
        assert_eq!(bcp.cur_cp, 0.0, "CP absorbs first");
        assert!((b.cur_hp - (100.0 - (damage - 100.0))).abs() < 1e-9, "HP takes the rest");
    }
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE); // MP consume
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2);
    // Being hit puts B in combat stance (CreatureAI.onEvtAttacked ->
    // clientStartAutoAttack broadcast), then B's CP/HP status.
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::AUTO_ATTACK_START);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE); // B's CP/HP
    // Nuking a player flags the caster (SkillCaster: bad skill on a playable →
    // updatePvPStatus(target)): a PVP_FLAG StatusUpdate for object 3001, then
    // the caster's own stance — both broadcast, object 3001.
    let a_flag = a_rx.try_recv().unwrap();
    assert_eq!(a_flag[0], server_packets::opcodes::STATUS_UPDATE);
    assert_eq!(i32::from_le_bytes(a_flag[1..5].try_into().unwrap()), 3001, "caster's own pvp-flag update");
    let a_stance = a_rx.try_recv().unwrap();
    assert_eq!(a_stance[0], server_packets::opcodes::AUTO_ATTACK_START);
    assert_eq!(i32::from_le_bytes(a_stance[1..5].try_into().unwrap()), 3001, "caster's own stance");
    assert!(a_rx.try_recv().is_err());
    assert_eq!(sm_id(&b_rx.try_recv().unwrap()), server_packets::sm_ids::C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2);
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::AUTO_ATTACK_START);
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE);
    // B also sees A's flag: the PVP_FLAG StatusUpdate + a RelationChanged.
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE, "B sees A's pvp-flag update");
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::RELATION_CHANGED, "B sees A's relation change");
    let b_sees_a = b_rx.try_recv().unwrap();
    assert_eq!(b_sees_a[0], server_packets::opcodes::AUTO_ATTACK_START);
    assert_eq!(i32::from_le_bytes(b_sees_a[1..5].try_into().unwrap()), 3001, "B sees the caster's stance");
    assert!(b_rx.try_recv().is_err());
    assert!(world.objects.get_component::<crate::model::components::AttackState>(&3001).is_some_and(|st| st.stance_until_tick > world.tick), "caster is in combat stance → canLogout refuses relogin");
    assert_eq!(world.objects.get_component::<crate::model::components::PvpState>(&3001).unwrap().flag, 1, "caster is now flagged for attacking a player");
    assert!(!world.objects.has_component::<Casting>(&3001), "coolTime 0 frees the slot");

    // Immediate re-cast: 10 s reuse still has 6 s left → SM 2303 + fail.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::S2_SECONDS_REMAINING_FOR_REUSE);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
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
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(a_rx.try_recv().is_err());
    assert!(b_rx.try_recv().is_err());
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert!(!world.objects.has_component::<Intent>(&3001), "dontMove must not start a walk-to-cast");
    assert!(!world.objects.has_component::<Movement>(&3001));
}

/// A lethal nuke kills (G9): HP hits 0, the victim is dead, and `Die` with
/// the to-village flag reaches both sides.
#[test]
fn nuke_kills_at_zero_hp() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    world.objects.get_component_mut::<PlayerVitals>(&3002).unwrap().cur_cp = 0.0;
    world.objects.get_component_mut::<Vitals>(&3002).unwrap().cur_hp = 5.0;
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
            .find(|p| p[0] == server_packets::opcodes::DIE && i32::from_le_bytes(p[1..5].try_into().unwrap()) == 3002)
            .expect("Die packet for B");
        assert_eq!(i32::from_le_bytes(die[5..9].try_into().unwrap()), 1, "to-village flag");
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
    let door = crate::model::door::spawn_door_for_test(&mut world, test_door(24190001, DoorOpenMethod::None));
    world.objects.get_component_mut::<Door>(&door).unwrap().current_hp = 100_000;
    let mut rx = ingame_caster(&mut world, 1, 3001, 150, 0); // within Wind Strike's 600 cast range

    // Ctrl-cast Wind Strike (1177, EnemyOnly) at the gate.
    handle_action(&mut world, 1, &action_body(door, 0));
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001), "the door is a valid enemy target");
    advance_ticks(&mut world, 45); // launch (35) + finish (5) with margin

    assert!(world.objects.get_component::<Door>(&door).unwrap().current_hp < 100_000, "the nuke damaged the gate");
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter().any(|p| p[0] == server_packets::opcodes::STATUS_UPDATE
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
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_CANCELED);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_CANCELED);

    // The scheduled launch is stale: nothing fires, nothing lands.
    world.tick += 40;
    apply_due_tasks(&mut world);
    assert!(a_rx.try_recv().is_err());
    assert!(b_rx.try_recv().is_err());
    assert_eq!(pvit(&world, 3001).cur_mp, mp_after_start, "no finish consume after abort");
    assert_eq!(pvit(&world, 3002).cur_hp, 100.0);

    // Reuse (registered at cast start) still blocks, then expires.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::S2_SECONDS_REMAINING_FOR_REUSE);
    drain(&mut a_rx);
    world.tick += 60;
    apply_due_tasks(&mut world);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001), "castable again after reuse expiry");
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

    world.objects.get_component_mut::<Position>(&3002).unwrap().x = 5000; // > effectRange 1100

    world.tick += 40;
    apply_due_tasks(&mut world);
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED);
    assert!(a_rx.try_recv().is_err(), "no MagicSkillLaunched, no cancel packet");
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
        assert!(pcs(&world, 3002).p_atk > base_p_atk, "P.Atk pumped by Might (+8%)");
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

    world.objects.get_component_mut::<Vitals>(&3001).unwrap().cur_mp = 0.0;

    advance_ticks(&mut world, 45);
    // Launch fires normally (range fine), then the finish fails on MP.
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::NOT_ENOUGH_MP);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(a_rx.try_recv().is_err());
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);
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
    assert_eq!(i32::from_le_bytes(pkt[1..5].try_into().unwrap()), 0, "Slow Aura has no reuse delay");

    // A reuse with 6 s left is reported with its total and remainder.
    world.objects.get_component_mut::<Reuses>(&3001).unwrap().0.insert(
        1177,
        crate::model::SkillReuse { skill_level: 1, until_tick: world.tick + 60, total_ms: 10_000 },
    );
    on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_COOL_TIME]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::SKILL_COOL_TIME);
    assert_eq!(i32::from_le_bytes(pkt[1..5].try_into().unwrap()), 1);
    assert_eq!(i32::from_le_bytes(pkt[5..9].try_into().unwrap()), 1177);
    assert_eq!(i32::from_le_bytes(pkt[9..13].try_into().unwrap()), 1, "known level");
    assert_eq!(i32::from_le_bytes(pkt[13..17].try_into().unwrap()), 10, "total seconds");
    assert_eq!(i32::from_le_bytes(pkt[17..21].try_into().unwrap()), 6, "remaining seconds");
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
            id,
            hit_time: 400,
            reuse_delay: 2000,
            reuse_delay_group: 9000,
            ..base.clone()
        });
    }

    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let skills = &mut world.objects.get_component_mut::<SkillBook>(&3001).unwrap().0;
    skills.insert(7001, 1);
    skills.insert(7002, 1);

    // Cast the first: MagicSkillUse carries group 9000 + the 2000 ms
    // delay, and the reuse lands under the group key, not the skill id.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(7001, false));
    let msu = drain(&mut a_rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE)
        .expect("MagicSkillUse broadcast");
    assert_eq!(i32::from_le_bytes(msu[25..29].try_into().unwrap()), 9000, "reuse group");
    assert_eq!(i32::from_le_bytes(msu[29..33].try_into().unwrap()), 2000, "reuse delay");
    let reuses = &world.objects.get_component::<Reuses>(&3001).unwrap().0;
    assert!(reuses.contains_key(&9000) && !reuses.contains_key(&7001));

    // The sibling is blocked by the shared cooldown (reuse gate fires
    // before the busy-casting-slot check, same as Java's useMagic order).
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(7002, false));
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::S1_IS_NOT_AVAILABLE_REUSE);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);

    // SkillCoolTime reports the group id, cast level, 2 s total/remaining.
    on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_COOL_TIME]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::SKILL_COOL_TIME);
    assert_eq!(i32::from_le_bytes(pkt[1..5].try_into().unwrap()), 1);
    assert_eq!(i32::from_le_bytes(pkt[5..9].try_into().unwrap()), 9000, "group id, not skill id");
    assert_eq!(i32::from_le_bytes(pkt[9..13].try_into().unwrap()), 1, "cast level");
    assert_eq!(i32::from_le_bytes(pkt[13..17].try_into().unwrap()), 2, "total seconds");
    assert_eq!(i32::from_le_bytes(pkt[17..21].try_into().unwrap()), 2, "remaining seconds");
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

    assert!(!world.objects.has_component::<Casting>(&3002), "victim's cast broken");
    let b_packets = drain(&mut b_rx);
    assert!(b_packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_CANCELED));
    assert!(b_packets
        .iter()
        .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::YOUR_CASTING_HAS_BEEN_INTERRUPTED));
    let a_packets = drain(&mut a_rx);
    assert!(a_packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_CANCELED));

    // B's stale launch task fires and no-ops: no buff ever lands.
    advance_ticks(&mut world, 60);
    assert_eq!(pbuffs(&world, 3002), 0);
}

/// Casting a good skill while running pauses the move and resumes it toward
/// the original destination after the cast; an offensive skill forgets it —
/// Java `PlayerAI.changeIntention`'s save/clear of the interrupted intention.
#[test]
fn good_skill_cast_pauses_and_resumes_inflight_move() {
    use crate::model::components::QueuedAction;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    world.objects.get_component_mut::<Speeds>(&3001).unwrap().run_spd = 100.0;
    world.objects.get_component_mut::<Speeds>(&3001).unwrap().running = true;

    handle_move_backward_to_location(&mut world, 1, &move_body((600, 0, 0), (0, 0, 0), 1));
    assert!(world.objects.has_component::<Movement>(&3001));
    drain(&mut a_rx);

    // Slow Aura (good, self): the move stops but its destination is saved.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));
    assert!(world.objects.has_component::<Casting>(&3001));
    assert!(!world.objects.has_component::<Movement>(&3001), "cast stops the move");
    match world.objects.get_component::<QueuedAction>(&3001) {
        Some(&QueuedAction::Move { x, y, z }) => assert_eq!((x, y, z), (600, 0, 0)),
        other => panic!("interrupted move not saved: {other:?}"),
    }

    // hit 9500 ms (95 ticks) + finish 5 ticks later: the move resumes.
    advance_ticks(&mut world, 101);
    assert!(!world.objects.has_component::<Casting>(&3001));
    let mv = world.objects.get_component::<Movement>(&3001).expect("move resumed after the cast");
    assert_eq!((mv.0.dest_x, mv.0.dest_y), (600, 0));

    // An offensive cast instead forgets the interrupted move.
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
    assert!(!world.objects.has_component::<Movement>(&3001), "cast stops the move");
    assert!(!world.objects.has_component::<QueuedAction>(&3001), "bad skill forgets the move");
    advance_ticks(&mut world, 45);
    assert!(!world.objects.has_component::<Movement>(&3001), "nothing resumes after a nuke");
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
    world.objects.get_component_mut::<Vitals>(&3003).unwrap().cur_hp = 50.0;

    // A nukes B (hit 3500 + finish 500 ms = 40 ticks).
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
    drain(&mut a_rx);

    // Mid-cast: select C, then click Battle Heal → rejected but queued.
    handle_action(&mut world, 1, &action_body(3003, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(a_rx.try_recv().is_err(), "nothing else while the cast runs");
    assert!(
        matches!(world.objects.get_component::<QueuedAction>(&3001), Some(QueuedAction::Skill { skill_id: 1015, .. })),
        "skill click parked in the queue slot"
    );
    assert_eq!(
        world.objects.get_component::<Casting>(&3001).unwrap().0.skill_id,
        1177,
        "the running cast is untouched"
    );

    // The nuke finishes → the queued heal starts by itself, aimed at C.
    advance_ticks(&mut world, 45);
    let cast = world.objects.get_component::<Casting>(&3001).expect("queued skill cast started");
    assert_eq!(cast.0.skill_id, 1015);
    assert_eq!(cast.0.target_object_id, 3003, "replay resolves the mid-cast re-target");
    assert!(!world.objects.has_component::<QueuedAction>(&3001), "queue consumed");

    // Heal phases (hit 500 + finish 500 ms): C's HP goes up.
    advance_ticks(&mut world, 12);
    assert!(pvit(&world, 3003).cur_hp > 50.0, "heal landed on the new target");
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
    assert!(world.objects.has_component::<Casting>(&3001), "nuke is casting");
    drain(&mut a_rx);

    // A SINGLE Ctrl-click on the second monster mid-cast: switches target AND
    // parks the attack as the intention (it can't swing yet — still casting).
    on_packet(&mut world, 1, [vec![cop::ATTACK], attack_request_body(next)].concat());
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
    assert!(world.objects.has_component::<Casting>(&3001), "the running nuke is untouched");

    // When the nuke finishes, the parked attack engages the new mob.
    let hp_before = nvit(&world, next).cur_hp;
    world.forced_rolls.extend(std::iter::repeat([0i32, 99, 10]).take(12).flatten());
    advance_world(&mut world, 55);
    assert!(nvit(&world, next).cur_hp < hp_before, "the new target took melee damage after the cast");
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
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    // Swing rolls: hit, no crit, ±0 random damage.
    world.forced_rolls.extend([0, 99, 10]);
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    drain(&mut a_rx);
    let swing_end = world.objects.get_component::<crate::model::components::AttackState>(&3001).unwrap().attack_end_tick;
    assert!(swing_end > world.tick, "swing in flight");

    // Mid-swing skill click: rejected, queued, intent intact.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(!world.objects.has_component::<Casting>(&3001), "no cast mid-swing");
    assert!(matches!(world.objects.get_component::<QueuedAction>(&3001), Some(QueuedAction::Skill { skill_id: 91, .. })));
    assert!(world.objects.has_component::<Intent>(&3001), "skill click keeps the attack intent");

    // Swing period over (`AttackFinish`): the queued cast starts.
    let remaining = swing_end - world.tick;
    advance_ticks(&mut world, remaining);
    let cast = world.objects.get_component::<Casting>(&3001).expect("queued skill fired at swing end");
    assert_eq!(cast.0.skill_id, 91);
    assert!(world.objects.has_component::<Intent>(&3001), "attack resumes after the cast");
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
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::CANNOT_SEE_TARGET);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(!world.objects.has_component::<Casting>(&3001));

    // Same side of the wall: the cast starts.
    world.objects.get_component_mut::<Position>(&3002).unwrap().x = 72; // cell 4
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
    world.objects.get_component_mut::<Speeds>(&6101).unwrap().run_spd = 100.0;

    handle_move_backward_to_location(&mut world, 1, &move_body((1000, 0, 0), (0, 0, 0), 1));

    assert_eq!(near_rx.try_recv().unwrap()[0], server_packets::opcodes::MOVE_TO_LOCATION);
    assert!(far_rx.try_recv().is_err(), "far player must not see the move");
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
        .position(|p| p[0] == server_packets::opcodes::STOP_MOVE
            && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid)
        .expect("StopMove broadcast for the dying mob");
    // Frozen at the death spot (40,0), not the move destination (400,0).
    let stop = &packets[stop_idx];
    assert_eq!(i32::from_le_bytes(stop[5..9].try_into().unwrap()), 40, "StopMove at death x");
    assert_eq!(i32::from_le_bytes(stop[9..13].try_into().unwrap()), 0, "StopMove at death y");
    // Ordering: StopMove precedes Die (Java doDie order).
    let die_idx = packets
        .iter()
        .position(|p| p[0] == server_packets::opcodes::DIE
            && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid)
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
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN), "walks toward the cast target");
    assert!(!packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE), "no cast before range");
    assert!(world.objects.has_component::<Intent>(&3001));
    assert!(!world.objects.has_component::<Casting>(&3001));

    // ~81 units at run speed 115 ⇒ in range in ~8 ticks.
    advance_world(&mut world, 15);
    assert!(world.objects.has_component::<Casting>(&3001), "cast starts on arrival");
    assert!(!world.objects.has_component::<Intent>(&3001), "the walk-to-cast intent is consumed");
    assert!(!world.objects.has_component::<Movement>(&3001), "chase leg stopped before casting");
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE));

    // Launch (35 ticks) + finish (5): the nuke lands on the walked-to monster.
    advance_world(&mut world, 45);
    assert!(nvit(&world, npc_oid).cur_hp < 5000.0, "nuke landed after the walk");
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
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::INVALID_TARGET);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(!world.objects.has_component::<Casting>(&3001), "no cast on a mob without force");

    // Ctrl (force) → the cast starts.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, true));
    assert!(world.objects.has_component::<Casting>(&3001), "ctrl force-targets the mob");
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
    let base_p_atk = world.objects.get_component::<CombatStats>(&npc_oid).unwrap().p_atk;
    assert!(base_p_atk > 0.0, "sanity: the mob has a base pAtk");

    // Might (+8% pAtk), forced onto the mob; lands after hit_time (10 ticks).
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, true));
    advance_ticks(&mut world, 12);
    let buffed = world.objects.get_component::<CombatStats>(&npc_oid).unwrap().p_atk;
    assert!((buffed - base_p_atk * 1.08).abs() < 1e-6, "Might raises the mob pAtk 8% ({base_p_atk} -> {buffed})");
    assert_eq!(world.objects.get_component::<Buffs>(&npc_oid).unwrap().0.len(), 1, "buff tracked on the mob");

    // abnormal_time 20 s = 200 ticks → expiry reverts the stat.
    advance_ticks(&mut world, 205);
    let reverted = world.objects.get_component::<CombatStats>(&npc_oid).unwrap().p_atk;
    assert!((reverted - base_p_atk).abs() < 1e-6, "expiry reverts the mob pAtk");
    assert!(world.objects.get_component::<Buffs>(&npc_oid).unwrap().0.is_empty(), "buff removed on expiry");
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
    assert_eq!(i32::from_le_bytes(pkt[3..7].try_into().unwrap()), npc_oid, "for the buffed mob");
    assert_eq!(i16::from_le_bytes(pkt[7..9].try_into().unwrap()), 1, "one buff shown");
    assert_eq!(i32::from_le_bytes(pkt[9..13].try_into().unwrap()), 1068, "Might listed in the target window");
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
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    world.objects.get_component_mut::<crate::model::Player>(&3001).unwrap().exp = 4000; // level 5 on the test table
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);

    // Monsters are valid Enemy targets without ctrl.
    let exp_before = world.objects.get_component::<crate::model::Player>(&3001).expect("player").exp;
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Casting>(&3001), "cast accepted without force-use");
    // Roll order at cast finish: magic crit (d1000, 999_999 → no crit), the
    // `MagicFailures` success roll (d100, 0 → lands at full damage against a
    // level-5 mob), then the drop roll at death (999_999 → fails, so no loot
    // noise in this test).
    world.forced_rolls.extend([999_999, 0, 999_999]);
    advance_world(&mut world, 45);

    assert!(nvit(&world, npc_oid).dead, "the nuke killed it");
    assert!(world.objects.get_component::<crate::model::Player>(&3001).expect("player").exp > exp_before, "XP rewarded through the same death path");
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
    world.objects.get_component_mut::<crate::model::Player>(&3001).unwrap().exp = 4000; // level 5

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
    assert!((dealt - 1.0).abs() < 1e-9, "a resisted nuke deals exactly 1 damage, dealt {dealt}");
    assert!(!nvit(&world, npc_oid).dead, "1 damage can't kill a 100 HP mob");

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
    world.objects.get_component_mut::<crate::model::Player>(&3001).unwrap().exp = 4000; // level 5

    let npc_oid = NPC_OID + 32;
    add_test_npc(&mut world, npc_oid, 40098, "Monster", 5, 100, 0, 0);
    let m_atk = pcs(&world, 3001).m_atk;
    let m_def = pcs(&world, npc_oid).m_def; // `pcs` reads any object's CombatStats
    let unresisted = formulas::calc_magic_dam(m_atk, m_def, 12.0, false, 1.0, formulas::MagicFailure::None);
    assert!(unresisted > 100.0, "sanity: an unresisted nuke overkills a 100 HP mob ({unresisted})");

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
    assert!(nvit(&world, npc_oid).dead, "an unpenalized nuke kills a same-level mob");
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
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    // Deterministic land roll: crit rate > 0 so the blow can land, no random spread.
    {
        let c = world.objects.get_component_mut::<CombatStats>(&3001).unwrap();
        c.crit_hit = 1000.0;
        c.random_dmg = 0;
    }
    drain(&mut a_rx);

    // Mortal Blow (FatalBlow) — lands from behind, deals damage.
    let mortal = world.data.skill_data.get(16, 1).expect("Mortal Blow").clone();
    let hp0 = nvit(&world, npc_oid).cur_hp;
    world.forced_rolls.extend([999_999, 0, 999_999]); // top magic roll; success lands; crit-double fails
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &mortal);
    assert!(nvit(&world, npc_oid).cur_hp < hp0, "FatalBlow dealt damage (was a no-op before)");
    world.objects.get_component_mut::<Vitals>(&npc_oid).unwrap().cur_hp = hp0;

    // Backstab from behind — lands.
    let backstab = world.data.skill_data.get(30, 1).expect("Backstab").clone();
    world.forced_rolls.extend([999_999, 0, 999_999]);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &backstab);
    assert!(nvit(&world, npc_oid).cur_hp < hp0, "Backstab from the flank landed");
    world.objects.get_component_mut::<Vitals>(&npc_oid).unwrap().cur_hp = hp0;

    // Turn the mob to face the caster (heading 0x8000 = west) → caster is now in
    // front → Backstab silently fails, dealing no damage.
    world.objects.get_component_mut::<Position>(&npc_oid).unwrap().heading = 0x8000;
    world.forced_rolls.extend([999_999, 0, 999_999]);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &backstab);
    assert_eq!(nvit(&world, npc_oid).cur_hp, hp0, "front Backstab dealt no damage");
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
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
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

    let skill = world.data.skill_data.get(1147, 1).expect("Vampiric Touch").clone();
    // magic-crit roll fails, then the `MagicFailures` success roll lands (0).
    world.forced_rolls.extend([999_999, 0]);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

    let dmg = npc_hp_before - nvit(&world, npc_oid).cur_hp;
    assert!(dmg > 0.0, "Vampiric Touch dealt damage (was a silent no-op before)");
    let healed = world.objects.get_component::<Vitals>(&3001).unwrap().cur_hp - caster_hp_before;
    assert!((healed - 0.40 * dmg).abs() < 1.0, "caster healed {healed}, expected 40% of {dmg}");
}

/// Spawn the level-5 test mob (40001) targeted for a debuff cast and drain the
/// spawn/target chatter, returning its object id.
fn spawn_debuff_target(world: &mut World, a_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) -> i32 {
    let npc_oid = NPC_OID + 14;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
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

    let skill = world.data.skill_data.get(1160, 1).expect("Decrease Speed").clone();
    assert!(skill.is_bad() && skill.affect_scope == AffectScope::Single);
    world.forced_rolls.extend([0, 0]); // magic-crit roll, then land roll (0 < 90 → lands)
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

    // Debuff applied to the mob: run speed recomputed to base 120 × 0.80 = 96.
    let speed = world.objects.get_component::<Speeds>(&npc_oid).unwrap().run_spd;
    assert!((speed - 96.0).abs() < 1e-6, "run speed debuffed to 96, got {speed}");

    // The caster sees the landed-outcome line (single-target only).
    let msgs = drain(&mut a_rx);
    assert!(
        msgs.iter()
            .any(|p| sysmsg_text(p).as_deref() == Some("Decrease Speed landed with 90% chance on Test Gremlin")),
        "caster received the debuff landed S1_TEXT line",
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

    let skill = world.data.skill_data.get(1160, 1).expect("Decrease Speed").clone();
    world.forced_rolls.extend([0, 90]); // magic-crit roll, then land roll (90 >= 90 → resisted)
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

    // No debuff: run speed stays at the mob's base 120.
    let speed = world.objects.get_component::<Speeds>(&npc_oid).unwrap().run_spd;
    assert!((speed - 120.0).abs() < 1e-6, "run speed unchanged on resist, got {speed}");

    let msgs = drain(&mut a_rx);
    // The resisted-outcome line carries the target, skill, and computed chance.
    assert!(
        msgs.iter()
            .any(|p| sysmsg_text(p).as_deref() == Some("Test Gremlin has resisted Decrease Speed: 90%")),
        "caster received the debuff resisted S1_TEXT line",
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
    world.objects.get_component_mut::<SkillBook>(&3001).unwrap().0.insert(1160, 1);

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
    assert_eq!(ai.intention, NpcIntention::Attack, "resisted debuff still wakes the mob");
    let aggro = world.objects.get_component::<AggroList>(&npc_oid).unwrap();
    assert!(aggro.0.contains_key(&3001), "the caster is on the mob's aggro list");
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
        without_action: false,
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
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects: vec![SkillEffect::DamOverTime { power: 24.0, ticks: 5, can_kill: false }],
    };
    world.data.skill_data.insert_for_test(poison(1, 3));
    world.data.skill_data.insert_for_test(poison(4, 7));
    world.data.skill_data.insert_for_test(Skill {
        without_action: false,
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
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects: vec![SkillEffect::DispelBySlot { dispel: vec![("POISON".into(), 3)] }],
    });

    let poison1 = world.data.skill_data.get(129, 1).unwrap().clone();
    let poison4 = world.data.skill_data.get(129, 4).unwrap().clone();
    let cure = world.data.skill_data.get(1012, 1).unwrap().clone();

    // Land Poison lvl 1 (abnormalLevel 3) on the mob.
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &poison1);
    assert_eq!(world.objects.get_component::<Buffs>(&npc_oid).unwrap().0.len(), 1, "poison landed");

    // Cure Poison lvl 1 dispels POISON up to level 3 → the debuff is removed.
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &cure);
    assert!(world.objects.get_component::<Buffs>(&npc_oid).unwrap().0.is_empty(), "poison cured");

    // A higher-level poison (lvl 4, abnormalLevel 7) is above Cure Poison lvl 1's
    // reach (POISON,3) and survives the cleanse.
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &poison4);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &cure);
    assert_eq!(
        world.objects.get_component::<Buffs>(&npc_oid).unwrap().0.len(),
        1,
        "a poison above the cure's dispel level is not removed",
    );
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
    world.data.skill_trees.insert_for_test(0, SkillLearn {
        skill_id: 1001,
        skill_level: 1,
        name: "Too High".into(),
        get_level: 10,
        level_up_sp: 0,
        auto_get: false,
        requires_item: false,
    });
    // Reachable level, but costs more SP than the player has (sp 0).
    world.data.skill_trees.insert_for_test(0, SkillLearn {
        skill_id: 1002,
        skill_level: 1,
        name: "Too Pricey".into(),
        get_level: 1,
        level_up_sp: 100,
        auto_get: false,
        requires_item: false,
    });

    handle_request_acquire_skill(&mut world, 1, &acquire_skill_body(1001, 1, cp::RequestAcquireSkill::CLASS));
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        vec![server_packets::sm_ids::YOU_DO_NOT_MEET_THE_SKILL_LEVEL_REQUIREMENTS],
    );

    handle_request_acquire_skill(&mut world, 1, &acquire_skill_body(1002, 1, cp::RequestAcquireSkill::CLASS));
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        vec![server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_SP_TO_LEARN_THIS_SKILL],
    );

    // Neither gate learned the skill.
    let book = world.objects.get_component::<crate::model::components::SkillBook>(&3001).unwrap();
    assert!(!book.0.contains_key(&1001) && !book.0.contains_key(&1002));
}

/// `StoreSkillCooltime` round-trip: a live cooldown is captured into the save
/// (as an absolute wall-clock end time) and, on relog, `restore_reuses` re-arms
/// it against the current game tick — the cooldown survives the trip.
#[test]
fn skill_reuse_cooldown_survives_relog() {
    use crate::model::components::Reuses;
    use crate::model::SkillReuse;

    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    // A cooldown on reuse-key 1177, ending 500 ticks (50 s) out.
    world.objects.get_component_mut::<Reuses>(&3001).unwrap().0.insert(
        1177,
        SkillReuse { skill_level: 3, until_tick: world.tick + 500, total_ms: 300_000 },
    );

    // The save captures it (config default = on) as an absolute systime.
    let save = super::net::build_save_data(&world, 3001).expect("save data");
    assert_eq!(save.skill_reuses.len(), 1);
    let row = save.skill_reuses[0];
    assert_eq!((row.reuse_key, row.skill_level, row.reuse_delay), (1177, 3, 300_000));

    // Relog: a fresh bundle from a CharData carrying that row, restored against
    // the current tick + wall clock.
    let mut chr = dummy_char(3002, "Relog");
    chr.skill_reuses = vec![row];
    let mut bundle = Player::from_char(&world.data, &chr);
    bundle.restore_reuses(&chr, world.tick, commons::util::now_millis());

    let restored = bundle.reuses.0.get(&1177).expect("cooldown restored");
    assert_eq!((restored.skill_level, restored.total_ms), (3, 300_000));
    let remaining = restored.until_tick - world.tick;
    assert!((498..=500).contains(&remaining), "≈500 ticks left, got {remaining}");

    // With the config off, nothing is persisted (and the DB rows get cleared).
    world.cfg.character.store_skill_cooltime = false;
    assert!(super::net::build_save_data(&world, 3001).unwrap().skill_reuses.is_empty());
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
    assert_eq!((row.skill_id, row.skill_level, row.remaining_time_secs), (9500, 2, 70));

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
    assert_eq!(restored.skill_level, 2, "the stored level came back, not the skill's level 1");
    // 70 s off the *current* tick: the offline gap consumed none of the buff.
    assert_eq!(restored.expires_at_tick - world.tick, 700);

    // With the config off, buffs aren't persisted (and the DB rows get cleared).
    world.cfg.character.store_skill_cooltime = false;
    assert!(super::net::build_save_data(&world, 3001).unwrap().skill_buffs.is_empty());
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
    assert!(save.skill_buffs.is_empty(), "dance dropped, toggle never stored");

    // AltStoreDances=True (this dist) keeps the dance — but still not the toggle.
    world.cfg.character.alt_store_dances = true;
    let save = super::net::build_save_data(&world, 3001).expect("save data");
    assert_eq!(save.skill_buffs.len(), 1);
    assert_eq!(save.skill_buffs[0].skill_id, 9600, "only the dance came through");
}

// --- Buff-slot stacking & count caps (Java `EffectList.addActive`) -----------

/// A synthetic self-buff with a `PhysicalDefence +8%` modifier so it lands (a
/// non-empty effect list), tagged with the given abnormal type/level and
/// magic type (3 = dance/song).
fn synthetic_buff(id: i32, level: i32, abnormal_type: &str, abnormal_level: i32, magic_type: i32) -> Skill {
    use crate::model::skill::{Skill, SkillEffect, StatModifierEffect};
    use crate::model::stats::{Stat, StatModifierType};
    Skill {
        without_action: false,
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
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects: vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::PhysicalDefence,
            mode: StatModifierType::Per,
            amount: 8.0,
            armor_condition: 0,
            weapon_condition: 0,
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
    assert_eq!(pbuffs(&world, 3001), 1, "lower level does not stack a second slot");
    assert_eq!(buff_skill_level(&world, 3001, 9001), 3, "lower level did not downgrade the buff");

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
    assert!(has_buff(&world, 3001, 9201), "debuff cannot be alt+click dispelled");
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
    assert!(has_buff(&world, 3001, 9202), "undispellable buff survives alt+click");
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
    assert!(has_buff(&world, 3001, 9203), "dance kept while DanceCancelBuff is off");

    // Config on (this dist's default): the dance is removed.
    world.cfg.character.dance_cancel_buff = true;
    handle_request_dispel(&mut world, 1, &dispel_body(3001, 9203, 1, 0));
    assert!(!has_buff(&world, 3001, 9203), "dance removed while DanceCancelBuff is on");
}

/// A dispel aimed at another object id (not the player's own) is a no-op for the
/// player's buffs — the pet/servitor branch is out of scope (TODO(G29)).
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
    assert!(has_buff(&world, 3001, 9204), "dispel on a foreign object id leaves the player's buff");
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
    assert_eq!(pbuffs(&world, 3001), 3, "the dance is counted separately, not against the buff cap");
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
    world.npc_regions.entry(extra.1 .0).or_default().push(far);
    world.objects.spawn(far, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&far, cs);

    // Nuke the near monster (hit 3500 + finish 500 ms = 40 ticks).
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Casting>(&3001), "first cast running");

    // Mid-cast, past the real Wind Strike's 1200 ms reuse (the test skill's
    // 10 s reuse is dropped to model the dist timing, where the reuse expires
    // while the 4 s cast is still running): select the far monster and click
    // the same skill again.
    advance_world(&mut world, 15);
    if let Some(reuses) = world.objects.get_component_mut::<crate::model::components::Reuses>(&3001) {
        reuses.0.clear();
    }
    handle_action(&mut world, 1, &action_body(far, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        matches!(world.objects.get_component::<QueuedAction>(&3001), Some(QueuedAction::Skill { skill_id: 1177, .. })),
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
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN), "MoveToPawn broadcast for the walk");

    // ~300 units at run speed ⇒ in range, then the cast starts on the far mob.
    advance_world(&mut world, 40);
    let cast = world.objects.get_component::<Casting>(&3001).expect("cast started after the walk");
    assert_eq!(cast.0.target_object_id, far, "cast aimed at the far monster");
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
    world.npc_regions.entry(extra.1 .0).or_default().push(far);
    world.objects.spawn(far, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&far, cs);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Casting>(&3001), "first cast running");

    // Client target switch: TargetCanceld (aborts the cast) + Action(far).
    advance_world(&mut world, 15);
    if let Some(reuses) = world.objects.get_component_mut::<crate::model::components::Reuses>(&3001) {
        reuses.0.clear();
    }
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(false));
    assert!(!world.objects.has_component::<Casting>(&3001), "cast aborted by the switch");
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
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN), "MoveToPawn broadcast for the walk");

    advance_world(&mut world, 40);
    let cast = world.objects.get_component::<Casting>(&3001).expect("cast started after the walk");
    assert_eq!(cast.0.target_object_id, far, "cast aimed at the far monster");
}

/// The same "queue on a far retarget" flow against the real datapack: real
/// Wind Strike (4 s cast, 1.2 s reuse — the reuse expires while the cast is
/// still running, so the mid-cast second click must reach the queue slot).
#[test]
fn queued_far_retarget_with_real_datapack_timings() {
    use crate::model::components::QueuedAction;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    world.data = crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let near = NPC_OID + 74;
    let far = NPC_OID + 75;
    // Real-datapack monsters (Gremlin, 20001) at 100 and 900 units.
    for (oid, x) in [(near, 100), (far, 900)] {
        let (npc, extra) = crate::model::npc::Npc::for_test(oid, 20001, x, 0, 0, 5000, 30);
        world.npc_regions.entry(extra.1 .0).or_default().push(oid);
        world.objects.spawn(oid, (npc, extra));
        let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(20001).unwrap(), &world.data.stat_bonus);
        world.objects.add_components(&oid, cs);
    }
    handle_action(&mut world, 1, &action_body(near, 0));
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Casting>(&3001), "first cast running");
    drain(&mut a_rx);

    // 2 s in: reuse (1.2 s) expired, cast (~4 s) still running. Select the far
    // monster and click the same skill again.
    advance_world(&mut world, 20);
    handle_action(&mut world, 1, &action_body(far, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        matches!(world.objects.get_component::<QueuedAction>(&3001), Some(QueuedAction::Skill { skill_id: 1177, .. })),
        "second click parked in the queue slot (casting {:?}, reuses {:?})",
        world.objects.get_component::<Casting>(&3001).map(|c| c.0.skill_id),
        world.objects.get_component::<crate::model::components::Reuses>(&3001)
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
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN), "MoveToPawn broadcast for the walk");

    advance_world(&mut world, 40);
    let cast = world.objects.get_component::<Casting>(&3001).expect("cast started after the walk");
    assert_eq!(cast.0.target_object_id, far, "cast aimed at the far monster");
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
    world.data = crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    let mut rx = ingame_player_access(&mut world, 1, 5001, 0);
    drain(&mut rx);
    world.objects.get_component_mut::<SkillBook>(&5001).unwrap().0.insert(618, 1);
    let base_run = world.objects.get_component::<Speeds>(&5001).unwrap().run_spd;

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(618, false));
    advance_world(&mut world, 40); // hitTime 2500 ms + finish, well inside 40 × 100 ms ticks

    {
        let p = world.objects.get_component::<Player>(&5001).unwrap();
        assert_eq!(p.transform_id, 2, "transformed into Doom Wraith (transformationId 2)");
        assert_eq!(p.transform_display_id, 2, "display id == id on this dist");
    }
    assert!(world.objects.get_component::<SkillBook>(&5001).unwrap().0.contains_key(&586), "transform's granted skill (Rolling Attack) present");
    assert_ne!(world.objects.get_component::<Speeds>(&5001).unwrap().run_spd, base_run, "run speed overridden by the transform template");
    assert_eq!(pbuffs(&world, 5001), 1, "lands as one TRANSFORM buff (drives the expiry-based revert)");

    // Re-casting while transformed is refused (Java's polymorph SystemMessage).
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(618, false));
    let refused = drain(&mut rx);
    assert!(
        has_system_message(&refused, server_packets::sm_ids::YOU_ALREADY_POLYMORPHED_AND_CANNOT_POLYMORPH_AGAIN),
        "already-polymorphed refusal sent"
    );
    assert!(!world.objects.has_component::<Casting>(&5001), "the refused click never starts a cast");

    // Expiry (natural `BuffExpire`, dispel, or death all route through this).
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, 5001, 618);
    let p = world.objects.get_component::<Player>(&5001).unwrap();
    assert_eq!(p.transform_id, 0, "reverted");
    assert_eq!(p.transform_display_id, 0, "display cleared");
    assert!(!world.objects.get_component::<SkillBook>(&5001).unwrap().0.contains_key(&586), "transform skill removed");
    assert_eq!(world.objects.get_component::<Speeds>(&5001).unwrap().run_spd, base_run, "run speed restored");
    assert_eq!(pbuffs(&world, 5001), 0, "buff cleared");
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
    world.data = crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    let mut rx = ingame_player_access(&mut world, 1, 5101, 0);
    drain(&mut rx);
    world.objects.get_component_mut::<SkillBook>(&5101).unwrap().0.insert(256, 1);
    let base_accuracy = pcs(&world, 5101).accuracy;
    let mp_before = pvit(&world, 5101).cur_mp;

    // Toggle on: instant (no cast bar) — `+3 Accuracy` lands immediately.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(256, false));
    assert_eq!(pbuffs(&world, 5101), 1, "toggle landed as one buff");
    assert_eq!(pcs(&world, 5101).accuracy, base_accuracy + 3, "Accuracy +3 (DIFF) applied");
    assert_eq!(pvit(&world, 5101).cur_mp, mp_before, "no MP deducted at cast time (pre-existing gap, not this slice)");

    // One upkeep tick: `power(0.4) * ticksMultiplier(5 × 666 / 1000 = 3.33) ≈ 1.332` MP.
    advance_world(&mut world, 40); // interval = (5 × 666) / 100 = 33 ticks
    let mp_after_one_tick = pvit(&world, 5101).cur_mp;
    assert!(
        (mp_before - mp_after_one_tick - 1.332).abs() < 1e-6,
        "first tick drained ~1.332 MP: {mp_before} -> {mp_after_one_tick}"
    );
    assert_eq!(pbuffs(&world, 5101), 1, "toggle still up (MP not exhausted yet)");

    // Drain the rest of the pool: the toggle self-deactivates the moment a
    // tick's drain would exceed current MP (Java's `false` return).
    drain(&mut rx);
    advance_world(&mut world, 40 * (mp_after_one_tick / 1.332).ceil() as u64 + 40);
    assert_eq!(pbuffs(&world, 5101), 0, "toggle switched itself off once MP ran dry");
    assert_eq!(pcs(&world, 5101).accuracy, base_accuracy, "Accuracy reverted");
    let packets = drain(&mut rx);
    assert!(
        has_system_message(&packets, server_packets::sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP),
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
    bare_bundle.spawn_into(&mut world.objects);
    let bare_shield = crate::game_loop::combat::combatant(&world, 5201).expect("bare combatant");
    assert_eq!(bare_shield.shield_def, 128.0, "no skill: raw sDef unchanged");

    let mut masted = dummy_char(5202, "Masted");
    masted.items = vec![paperdoll(2, 628, 7)];
    masted.skills = vec![(153, 4)];
    let masted_bundle = Player::from_char(&world.data, &masted);
    masted_bundle.spawn_into(&mut world.objects);
    let masted_shield = crate::game_loop::combat::combatant(&world, 5202).expect("masted combatant");
    assert_eq!(masted_shield.shield_def, 128.0 * 1.6, "Shield Mastery lvl4: sDef × 1.6 (+60% PER)");
    assert!(
        (masted_shield.shield_rate - bare_shield.shield_rate * 2.0).abs() < 1e-9,
        "Shield Mastery lvl4: rShld × 2.0 (+100% PER), CON bonus cancels in the ratio: {} vs {}",
        masted_shield.shield_rate,
        bare_shield.shield_rate
    );
}

/// G19 `HealPercent` effect: "Revival" (181, real dist data — a self-target,
/// 100%-power heal) restores HP on cast. Before this slice every
/// `HealPercent` skill — including the priest staples Miracle, Benediction,
/// Restore Life, Touch of Life — parsed to an empty effect list, so the cast
/// landed but healed nothing. (Self-cast rather than on another player: 1258
/// "Restore Life"'s `targetType ENEMY_NOT` hits an unrelated, pre-existing
/// gap — `TargetType::EnemyNot` isn't modeled and falls through to `Other`,
/// which `use_magic_on` silently no-ops — out of scope for this slice.)
#[test]
fn heal_percent_restores_a_share_of_max_hp() {
    let (mut world, ..) = test_world();
    world.data = crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

    let mut rx = ingame_player_access(&mut world, 1, 5301, 0);
    drain(&mut rx);
    world.objects.get_component_mut::<SkillBook>(&5301).unwrap().0.insert(181, 1);

    let max_hp = pvit(&world, 5301).max_hp as f64;
    let low = max_hp * 0.2;
    world.objects.get_component_mut::<Vitals>(&5301).unwrap().cur_hp = low;

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
