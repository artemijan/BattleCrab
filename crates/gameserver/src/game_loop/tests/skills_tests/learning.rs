//! Acquiring and losing skills: the learn gates and books, auto-learn, and
//! what a delevel takes away.

use super::*;

/// Delevel skill filtering runs at character *select*, before `from_char`, so
/// the built `Player` folds only the surviving passives and its enter-world
/// `UserInfo` is right the first time (the casting-speed-349 bug). A robe
/// mystic delevelled below 7 loses its getLevel-7 class skill but keeps
/// Spellcraft (getLevel 1), so casting speed stays 499.
///
/// Runs under `StrictDelevelSkillRemoval = true`, which is **not** what ships
/// (see `the_delevel_grace_is_what_ships_and_what_defaults`). The subject here
/// is the *stat refold* after a strip, and a strip is only the trigger — under
/// the shipped 9-level grace a `getLevel`-7 skill can never be stripped at all
/// (the threshold is negative), so the grace would leave this test with no
/// trigger and nothing to assert. Strict is the cheapest way to produce one;
/// the refold path is identical either way.
#[test]
fn delevel_filter_on_select_keeps_passive_stats() {
    const DIST: &str = crate::data::DIST_GAME;
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = dist::player_templates_owned();
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = dist::items_owned();
    data.skill_data = dist::skills_owned();
    data.skill_trees = dist::skill_trees_owned();
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);
    // See the doc comment: the grace can never strip a getLevel-7 skill, so
    // strict is what gives this test something to refold stats after.
    world.cfg.character.strict_delevel_skill_removal = true;

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
    let changes = death::maybe_skill_remove_on_delevel(
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
///
/// Under `StrictDelevelSkillRemoval = true` for the same reason as
/// `delevel_filter_on_select_keeps_passive_stats`: the shipped 9-level grace
/// never strips a getLevel-7 skill, so it would leave this test no trigger.
#[test]
fn live_delevel_removes_passive_and_recomputes_stats() {
    const DIST: &str = crate::data::DIST_GAME;
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = dist::player_templates_owned();
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = dist::items_owned();
    data.skill_data = dist::skills_owned();
    data.skill_trees = dist::skill_trees_owned();
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);
    // See the doc comment — the grace would leave nothing to strip.
    world.cfg.character.strict_delevel_skill_removal = true;

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
    death::check_player_skills(&mut world, 4214);
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
        death::reward_skills(&mut world, 2001);
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
        death::reward_skills(&mut world, 2001);
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
            .get_component_mut::<Player>(&2001)
            .unwrap()
            .level = new_level;
        death::check_player_skills(&mut world, 2001);
        world
            .objects
            .get_component::<SkillBook>(&2001)
            .unwrap()
            .0
            .get(&skill_id)
            .copied()
    };

    // --- Strict mode (StrictDelevelSkillRemoval = true), the port extension. ---
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

    // --- Non-strict: the Java 9-level grace, which is what ships. ---
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

/// **Which of the two branches above actually ships.**
///
/// `StrictDelevelSkillRemoval` is a port extension with no upstream key, so
/// nothing in the reference pins it and the choice lives entirely here. It
/// ships — and defaults — to `false`, the Java-faithful 9-level grace; the
/// strict branch is opt-in.
///
/// Asserted against the real `Character.ini` *and* the code default, because
/// those are two independent ways to end up on the wrong branch: the key going
/// missing from the ini falls back to the default (which
/// `config_boot_warnings` would also catch), and a default flipped in code
/// changes every test world at once.
#[test]
fn the_delevel_grace_is_what_ships_and_what_defaults() {
    let shipped = crate::config::CharacterConfig::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    assert!(
        !shipped.strict_delevel_skill_removal,
        "dist/game/config/Character.ini must ship the retail grace"
    );
    assert!(
        !crate::config::CharacterConfig::default().strict_delevel_skill_removal,
        "and the code default must agree, so a missing key lands on retail too"
    );
    assert!(
        shipped.decrease_skill_level,
        "the grace only means anything while DecreaseSkillOnDelevel is on"
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
        &acquire_skill_body(1001, 1, cp::combat::RequestAcquireSkill::CLASS),
    );
    assert_eq!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::YOU_DO_NOT_MEET_THE_SKILL_LEVEL_REQUIREMENTS],
    );

    handle_request_acquire_skill(
        &mut world,
        1,
        &acquire_skill_body(1002, 1, cp::combat::RequestAcquireSkill::CLASS),
    );
    assert_eq!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_SP_TO_LEARN_THIS_SKILL],
    );

    // Neither gate learned the skill.
    let book = world.objects.get_component::<SkillBook>(&3001).unwrap();
    assert!(!book.0.contains_key(&1001) && !book.0.contains_key(&1002));
}

/// `checkPlayerSkill`'s required-item leg: a book-gated entry (the class trees'
/// `<item id count/>` children) is refused without the book, and consumes it
/// with the disappear message when the player has it.
#[test]
fn skill_acquire_requires_and_consumes_the_book() {
    use crate::data::skill_tree::SkillLearn;
    use model::inventory::Inventory;

    const BOOK: i32 = 8618; // Ancient Book: Divine Inspiration (Modern Language)

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.cfg.character.divine_inspiration_sp_book_needed = true;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().sp = 500;
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
        &acquire_skill_body(1003, 1, cp::combat::RequestAcquireSkill::CLASS),
    );
    assert_eq!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ITEMS_TO_LEARN_THIS_SKILL],
    );
    assert!(
        !world
            .objects
            .get_component::<SkillBook>(&3001)
            .unwrap()
            .0
            .contains_key(&1003),
        "book-gated skill not learned without the book"
    );
    assert_eq!(
        world.objects.get_component::<Player>(&3001).unwrap().sp,
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
        &acquire_skill_body(1003, 1, cp::combat::RequestAcquireSkill::CLASS),
    );
    assert_eq!(
        world
            .objects
            .get_component::<SkillBook>(&3001)
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
        world.objects.get_component::<Player>(&3001).unwrap().sp,
        400,
        "500 SP - levelUpSp(100)"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_DISAPPEARED),
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

    let view = model::PlayerView::of_world(&world, 3001).expect("view");
    let skills = world.objects.get_component::<SkillBook>(&3001).unwrap();
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
    world.objects.get_component_mut::<Player>(&3001).unwrap().sp = 500;
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
            cp::combat::RequestAcquireSkill::CLASS,
        ),
    );
    assert_eq!(
        world
            .objects
            .get_component::<SkillBook>(&3001)
            .unwrap()
            .0
            .get(&DIVINE_INSPIRATION_SKILL_ID),
        Some(&1),
        "learned with no book in the bag"
    );
    assert_eq!(
        world.objects.get_component::<Player>(&3001).unwrap().sp,
        500,
        "Java's early return skips the SP deduction too"
    );

    // The other book-gated skill is still refused.
    drain(&mut rx);
    handle_request_acquire_skill(
        &mut world,
        1,
        &acquire_skill_body(1003, 1, cp::combat::RequestAcquireSkill::CLASS),
    );
    assert_eq!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ITEMS_TO_LEARN_THIS_SKILL],
        "the waiver is keyed to skill 1405 alone"
    );
}
