//! Passive skills — the weapon and armour masteries, cast speed, the
//! conditional wounded-only bonuses, and the storage caps.

use super::*;

/// Real-data stat parity: a level-1 Human Mystic loaded with the *real* class
/// starting gear (`initialEquipment.xml`, replayed through the equip-slot logic)
/// and *all* the class's level-1 autoGet skills (`skillTrees`), computed the
/// same way enter-world does, must show exactly the numbers the Java client
/// draws — including the Spellcraft-boosted casting speed of 499. Locks in the
/// finalizer fixes (pDef levelMod + slot-sub, mDef MEN×levelMod, RunSpeedBoost,
/// `(int)` truncation) *and* the armor-conditioned passives end to end.
#[test]
fn human_mystic_lvl1_full_loadout_matches_java_client() {
    const DIST: &str = crate::data::DIST_GAME;
    let mut data = GameData::for_test();
    data.player_templates = dist::player_templates_owned();
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = dist::items_owned();
    data.skill_data = dist::skills_owned();
    data.skill_trees = dist::skill_trees_owned();
    data.initial_equipment = crate::data::initial_equipment::InitialEquipmentData::load_from(DIST);

    let class_id = 10; // Human Mystic

    // Replay the class starting equipment through the real equip-slot logic
    // (mirrors `resolve_initial_items`), then hand the resolved paperdoll to
    // `from_char` as stored `ItemRow`s.
    let mut inv = Inventory::new();
    for (i, entry) in data.initial_equipment.get(class_id).iter().enumerate() {
        let oid = 1000 + i as i32;
        inv.add_item(&data.item_data, oid, entry.item_id, entry.count);
        if entry.equipped {
            inv.equip_item(&data.item_data, oid);
        }
    }
    let items: Vec<crate::db::ItemRow> = inv
        .items()
        .iter()
        .map(|it| {
            let slot = inv.paperdoll_slot_of(it.object_id);
            crate::db::ItemRow {
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
    expertise::refresh_expertise_penalty(&mut world, 4212);
    passive_skills::refresh_conditioned_passives(&mut world, 4212);
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
    const DIST: &str = crate::data::DIST_GAME;
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = dist::player_templates_owned();
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = dist::items_owned();
    data.skill_data = dist::skills_owned();
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let paperdoll = |object_id, item_id, slot| crate::db::ItemRow {
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
        .get_component_mut::<Inventory>(&4211)
        .unwrap()
        .unequip_item(1003);
    passive_skills::refresh_conditioned_passives(&mut world, 4211);
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
    const DIST: &str = crate::data::DIST_GAME;
    let mut data = GameData::for_test();
    data.player_templates = dist::player_templates_owned();
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = dist::items_owned();
    data.skill_data = dist::skills_owned();
    data.skill_trees = dist::skill_trees_owned();
    data.initial_equipment = crate::data::initial_equipment::InitialEquipmentData::load_from(DIST);

    let class_id = 10; // Human Mystic

    // No-grade MAGIC robe (chest/legs/gloves → Spellcraft applies, no grade
    // penalty) plus a D-grade BLUNT staff (15149) — a weapon that is NOT
    // bow/pole, equipped through the real slot logic.
    let mut inv = Inventory::new();
    for (i, item_id) in [6, 425, 461, 15149].into_iter().enumerate() {
        let oid = 2000 + i as i32;
        inv.add_item(&data.item_data, oid, item_id, 1);
        inv.equip_item(&data.item_data, oid);
    }
    let items: Vec<crate::db::ItemRow> = inv
        .items()
        .iter()
        .map(|it| {
            let slot = inv.paperdoll_slot_of(it.object_id);
            crate::db::ItemRow {
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
        death::maybe_skill_remove_on_delevel(
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
    expertise::refresh_expertise_penalty(&mut world, 4213);
    assert_eq!(
        pcs(&world, 4213).m_atk_spd,
        499,
        "cast speed after expertise refresh"
    );
    passive_skills::refresh_conditioned_passives(&mut world, 4213);
    assert_eq!(
        pcs(&world, 4213).m_atk_spd,
        499,
        "cast speed after conditioned-passive refresh"
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
    const DIST: &str = crate::data::DIST_GAME;
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = dist::player_templates_owned();
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = dist::items_owned();
    data.skill_data = dist::skills_owned();
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let paperdoll = |object_id, item_id, slot| crate::db::ItemRow {
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
    let bare_shield = combat::combatant(&world, 5201).expect("bare combatant");
    assert_eq!(
        bare_shield.shield_def, 128.0,
        "no skill: raw sDef unchanged"
    );

    let mut masted = dummy_char(5202, "Masted");
    masted.items = vec![paperdoll(2, 628, 7)];
    masted.skills = vec![(153, 4, 0)];
    let masted_bundle = Player::from_char(&world.data, &masted);
    masted_bundle.spawn_into(&mut world);
    let masted_shield = combat::combatant(&world, 5202).expect("masted combatant");
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
    const DIST: &str = crate::data::DIST_GAME;
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = dist::player_templates_owned();
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = dist::items_owned();
    data.skill_data = dist::skills_owned();
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let paperdoll = |object_id, item_id, slot| crate::db::ItemRow {
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
/// `formulas::physical::calc_blow_success` folds into the Backstab/Lethal-Blow-style
/// landing roll. Before this slice `Stat::BlowRate` didn't exist and the
/// formula had no term for it at all — the skill was a passive that did
/// nothing. Checked at the `StatModifiers` level (the formula's own boundary
/// shift is covered by `formulas::tests::blow_success_rate_cap_and_threshold`).
#[test]
fn assassination_passive_raises_blow_rate_stat() {
    const DIST: &str = crate::data::DIST_GAME;
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = dist::player_templates_owned();
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = dist::items_owned();
    data.skill_data = dist::skills_owned();
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let bare = dummy_char(5501, "Bare");
    let bare_bundle = Player::from_char(&world.data, &bare);
    assert_eq!(
        bare_bundle.stat_modifiers.mul.get(&Stat::BlowRate),
        None,
        "no skill: no modifier at all"
    );

    let mut assassin = dummy_char(5502, "Assassin");
    assassin.skills = vec![(432, 1, 0)]; // Assassination
    let assassin_bundle = Player::from_char(&world.data, &assassin);
    let mul = assassin_bundle
        .stat_modifiers
        .mul
        .get(&Stat::BlowRate)
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
    const DIST: &str = crate::data::DIST_GAME;
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = dist::player_templates_owned();
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = dist::items_owned();
    data.skill_data = dist::skills_owned();
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);
    let cfg = world.cfg.character.clone();

    let mut bare = dummy_char(5301, "Bare");
    bare.race = 0; // human, not a dwarf
    let bare_bundle = Player::from_char(&world.data, &bare);
    let bare_view = bare_bundle.view();
    let bare_limit = model::stat_finalize::finalize(
        bare_view.mods,
        Stat::InventoryNormal,
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
    let expanded_limit = model::stat_finalize::finalize(
        expanded_view.mods,
        Stat::InventoryNormal,
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
    const DIST: &str = crate::data::DIST_GAME;
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = dist::player_templates_owned();
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = dist::items_owned();
    data.skill_data = dist::skills_owned();
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);
    let cfg = world.cfg.character.clone();

    // `ex(0x2F)` = one opcode byte + the two-byte ex id, then 12 ints.
    let fields = |bytes: &[u8]| -> Vec<i32> {
        bytes[3..]
            .as_chunks::<4>()
            .0
            .iter()
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
    /// Inner Rhythm: `MagicMpCost -10 PER`, `magicType 3` (songs and dances).
    const INNER_RHYTHM: i32 = 428;
    /// Champion Song — `magicType 3`, `mpConsume 60`.
    const SONG: i32 = 364;
    /// Wind Strike — an ordinary magic skill, a different `magicType`.
    const WIND_STRIKE: i32 = 1177;

    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();
    let _rx = ingame_player_access(&mut world, 1, 7711, 0);

    let song = world.data.skill_data.get(SONG, 1).expect("song").clone();
    let nuke = world
        .data
        .skill_data
        .get(WIND_STRIKE, 1)
        .expect("wind strike")
        .clone();
    let mp = |w: &World, s: &Skill| effects::mp_consume_for(w, 7711, s);

    let (song_before, nuke_before) = (mp(&world, &song), mp(&world, &nuke));
    assert_eq!(song_before, 60, "sanity: the dist still prices it at 60");

    world
        .objects
        .get_component_mut::<SkillBook>(&7711)
        .unwrap()
        .0
        .insert(INNER_RHYTHM, 1);
    passive_skills::refresh_conditioned_passives(&mut world, 7711);

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
    passive_skills::refresh_conditioned_passives(&mut world, 7711);
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
    const CHAMPION_SONG: i32 = 364;
    const OID: i32 = 7721;

    let cast_cost = |learn_inner_rhythm: bool| -> f64 {
        let (mut world, ..) = test_world();
        world.data = dist::game_data_owned();
        let _rx = ingame_player_access(&mut world, 1, OID, 100);
        // A real MP pool rather than a hand-set one: `//add_skill` recomputes
        // max vitals, and a level-1 character's ~40 MP would be clamped back
        // under the song's 60 anyway.
        death::set_level(&mut world, OID, 78);
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

/// `AbstractConditionalHpEffect` — a stat effect that counts **only while the
/// wearer's HP is at or below `<hpPercent>`**:
///
/// ```java
/// public boolean canPump(Creature effector, Creature effected, Skill skill)
/// {
///     return (_hpPercent <= 0) || (effected.getCurrentHpPercent() <= _hpPercent);
/// }
/// ```
///
/// Four handlers extend it (`PAtk`, `PhysicalDefence`, `PhysicalEvasion`,
/// `CriticalRate`) and two learnable skills on this dist use it: **Final Frenzy
/// (290)**, +P.Atk below 30 % HP, and **Final Fortress (291)**, +P.Def. The port
/// parsed the effects but not the condition, so both bonuses were up
/// permanently — a flat damage and defence inflation for the classes that learn
/// them.
///
/// The fixture reads its numbers off the **real** skill so it cannot drift from
/// what it is modelling, and drives the whole thing through the same
/// `refresh_conditioned_passives` the equip path uses.
#[test]
fn a_below_thirty_percent_passive_only_counts_while_wounded() {
    use model::components::skills::SkillBook;
    use model::components::stats::Vitals;

    const FINAL_FRENZY: i32 = 290;
    let (mut world, ..) = cast_test_world();
    world.data.skill_data = dist::skills_owned();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // The real skill's own numbers: a DIFF `PAtk` bonus gated at 30 %.
    let frenzy = world
        .data
        .skill_data
        .get(FINAL_FRENZY, 1)
        .expect("Final Frenzy 290 on the dist");
    let (bonus, threshold) = frenzy
        .effects
        .iter()
        .find_map(|e| match e {
            model::skill::effects::SkillEffect::StatModifier(m)
                if m.stat == Stat::PhysicalAttack && m.hp_percent > 0 =>
            {
                Some((m.amount, m.hp_percent))
            }
            _ => None,
        })
        .expect("a PAtk effect carrying an hpPercent");
    assert_eq!(threshold, 30, "Java's `<hpPercent>30</hpPercent>`");
    assert!(bonus > 0.0, "and a positive P.Atk bonus");

    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&3001) {
        book.0.insert(FINAL_FRENZY, 1);
    }

    let set_hp = |world: &mut World, percent: f64| {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&3001) {
            v.max_hp = 1000;
            v.cur_hp = 1000.0 * percent / 100.0;
        }
        passive_skills::refresh_conditioned_passives(world, 3001);
        pcs(world, 3001).p_atk
    };

    let healthy = set_hp(&mut world, 100.0);
    let wounded = set_hp(&mut world, 25.0);
    assert!(
        (wounded - healthy - bonus).abs() < 1e-9,
        "below 30 % the bonus is up: {wounded} vs {healthy} (+{bonus})"
    );

    // Java's test is `<=`, and `getCurrentHpPercent()` truncates, so 30 % is
    // inside the band and 31 % is not.
    assert!(
        (set_hp(&mut world, 30.0) - healthy - bonus).abs() < 1e-9,
        "exactly 30 % still counts — Java compares with `<=`"
    );
    assert!(
        (set_hp(&mut world, 31.0) - healthy).abs() < 1e-9,
        "31 % does not"
    );

    // And healing back out drops it again, which is the half a one-way hook
    // would miss.
    assert!(
        (set_hp(&mut world, 100.0) - healthy).abs() < 1e-9,
        "back above the band, the bonus is gone"
    );
}
