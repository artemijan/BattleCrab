//! `General.ini`'s `SkillCheckEnable`/`Remove`/`GM` — the `restoreSkills`
//! validation of every `character_skills` row against the skill trees.
//!
//! The allow-list itself ([`SkillTreeData::is_skill_allowed`]) is tested
//! against the **real dist trees** in `data::skill_tree`'s own module, because
//! what it does turns on what the shipped data contains. What is tested here is
//! the other half: that the check runs at load, obeys its three keys, and hands
//! the finding to the login path.

use super::*;
use crate::data::SkillCheckSettings;
use crate::model::components::skills::SkillBook;

/// A skill id no tree on this dist grants at any level.
const ILLEGAL_SKILL: i32 = 999_001;

/// Build a world whose class 0 tree teaches exactly one skill, so "allowed"
/// and "illegal" are both unambiguous.
fn world_with_one_class_skill(check: SkillCheckSettings) -> World {
    let (mut world, ..) = test_world();
    world.data.skill_check = check;
    world.data.skill_trees.insert_for_test(
        0,
        crate::data::skill_tree::SkillLearn {
            skill_id: 3,
            skill_level: 1,
            name: "Power Strike".into(),
            get_level: 1,
            level_up_sp: 0,
            auto_get: false,
            required_items: Vec::new(),
        },
    );
    world
}

fn char_with_skills(object_id: i32, skills: &[(i32, i32, i32)]) -> CharData {
    let mut chr = dummy_char(object_id, "Checked");
    chr.class_id = 0;
    chr.base_class_id = 0;
    chr.skills = skills.to_vec();
    chr
}

fn book_of(bundle: &crate::model::PlayerData) -> Vec<(i32, i32)> {
    let mut v: Vec<(i32, i32)> = bundle.skills.0.iter().map(|(&i, &l)| (i, l)).collect();
    v.sort_unstable();
    v
}

/// The default path on this dist: check on, remove on. The illegal row is
/// dropped from the book **and** recorded; the legitimate one is untouched.
///
/// Both halves matter — a check that removed everything would satisfy the first
/// assertion on its own, which is why the class skill is in the same fixture.
#[test]
fn an_illegal_row_is_removed_and_recorded_while_a_class_skill_survives() {
    let world = world_with_one_class_skill(SkillCheckSettings {
        enable: true,
        remove: true,
        gm: true,
    });
    let chr = char_with_skills(4101, &[(3, 1, 0), (ILLEGAL_SKILL, 1, 0)]);
    let bundle = crate::model::Player::from_char(&world.data, &chr);

    assert_eq!(
        book_of(&bundle),
        vec![(3, 1)],
        "only the class skill is kept"
    );
    assert_eq!(
        bundle.illegal_skills,
        vec![(ILLEGAL_SKILL, 1)],
        "and the removal is reported"
    );
}

/// `SkillCheckRemove` is a **separate** key from `SkillCheckEnable`, and with
/// it off the check is a pure audit: the finding is still made, the skill still
/// kept.
///
/// This is the configuration an operator uses to see what a check *would* do
/// before letting it delete anything, so collapsing the two keys into one —
/// the obvious simplification — destroys the only safe way to turn the feature
/// on.
#[test]
fn removal_is_a_separate_key_from_the_check_itself() {
    let world = world_with_one_class_skill(SkillCheckSettings {
        enable: true,
        remove: false,
        gm: true,
    });
    let chr = char_with_skills(4102, &[(3, 1, 0), (ILLEGAL_SKILL, 1, 0)]);
    let bundle = crate::model::Player::from_char(&world.data, &chr);

    assert_eq!(
        book_of(&bundle),
        vec![(3, 1), (ILLEGAL_SKILL, 1)],
        "nothing is removed"
    );
    assert_eq!(
        bundle.illegal_skills,
        vec![(ILLEGAL_SKILL, 1)],
        "but the audit still reports it"
    );
}

/// `SkillCheckEnable = False` — nothing runs at all, not even the audit.
#[test]
fn the_check_is_off_by_default() {
    let world = world_with_one_class_skill(SkillCheckSettings::default());
    let chr = char_with_skills(4103, &[(ILLEGAL_SKILL, 1, 0)]);
    let bundle = crate::model::Player::from_char(&world.data, &chr);

    assert_eq!(book_of(&bundle), vec![(ILLEGAL_SKILL, 1)]);
    assert!(bundle.illegal_skills.is_empty());
}

/// **`SkillCheckGM` reads backwards, and this is the test that says so.**
///
/// Java's guard is
/// `(!canOverrideCond(SKILL_CONDITIONS) || Config.SKILL_CHECK_GM)`, so the key
/// being **False** — the dist value — *exempts* a character holding the
/// override, and True subjects them to the check like anyone else. Reading the
/// name as "should GMs be checked" and wiring it as written gives exactly the
/// opposite behaviour on both branches, and no other test in this file would
/// notice.
///
/// The override reaches a GM by way of `Player.restore`'s default of
/// `getAllExceptionsMask()`, which is why an access level is all this fixture
/// sets.
#[test]
fn skill_check_gm_false_exempts_a_gm_and_true_checks_them() {
    for (gm_key, expect_checked) in [(false, false), (true, true)] {
        let mut world = world_with_one_class_skill(SkillCheckSettings {
            enable: true,
            remove: true,
            gm: gm_key,
        });
        // `test_world`'s access table is empty, so nothing is a GM; load the
        // real one, where 100 is.
        world.data.admin = crate::data::AdminData::load_from(crate::data::DIST_GAME);
        let mut chr = char_with_skills(4104, &[(ILLEGAL_SKILL, 1, 0)]);
        chr.access_level = 100;
        let bundle = crate::model::Player::from_char(&world.data, &chr);

        assert_eq!(
            !bundle.illegal_skills.is_empty(),
            expect_checked,
            "SkillCheckGM = {gm_key}: a GM should {} be checked",
            if expect_checked { "" } else { "not" }
        );
        assert_eq!(
            book_of(&bundle).is_empty(),
            expect_checked,
            "SkillCheckGM = {gm_key}: and the skill should {} be removed",
            if expect_checked { "" } else { "not" }
        );
    }
}

/// A non-GM never holds the override, so the key does not reach them at all —
/// pinned so a future short-circuit ("skip the check when `!skill_check.gm`")
/// cannot quietly disable the whole feature.
#[test]
fn an_ordinary_character_is_checked_whatever_skill_check_gm_says() {
    for gm_key in [false, true] {
        let world = world_with_one_class_skill(SkillCheckSettings {
            enable: true,
            remove: true,
            gm: gm_key,
        });
        let chr = char_with_skills(4105, &[(ILLEGAL_SKILL, 1, 0)]);
        let bundle = crate::model::Player::from_char(&world.data, &chr);
        assert_eq!(
            bundle.illegal_skills,
            vec![(ILLEGAL_SKILL, 1)],
            "SkillCheckGM = {gm_key} must not affect a plain character"
        );
    }
}

/// Java's first arm: `skill.isExcludedFromCheck()`, read off the **skill**, not
/// any tree. The subclass certification families are in no tree at all and are
/// legitimate anyway; without this arm the check would delete every one of them
/// the moment a dist shipped a `subClassSkillTree`.
#[test]
fn an_excluded_from_check_skill_is_kept_though_no_tree_grants_it() {
    let mut world = world_with_one_class_skill(SkillCheckSettings {
        enable: true,
        remove: true,
        gm: true,
    });
    let mut skill = passive_clan_test_skill(ILLEGAL_SKILL);
    skill.excluded_from_check = true;
    world.data.skill_data.insert_for_test(skill);

    let chr = char_with_skills(4106, &[(ILLEGAL_SKILL, 1, 0)]);
    let bundle = crate::model::Player::from_char(&world.data, &chr);
    assert_eq!(book_of(&bundle), vec![(ILLEGAL_SKILL, 1)]);
    assert!(bundle.illegal_skills.is_empty());
}

/// **Noble skills are re-derived from the `nobless` column at load and are not
/// persisted** — Java `Player.restore`'s `setNoble(...)`, which grants them
/// with `addSkill(skill, false)`.
///
/// Three things are asserted together because they only make sense together:
/// the column grants them, a stored row for one is illegal, and the check does
/// not take the granted skill away with the row.
///
/// The order is the subtle part, and it is the opposite of the intuitive one.
/// Java's check reads its **`ResultSet`**, so it only ever judges stored rows;
/// the port runs it in the same place, over the DB rows, *before* the derived
/// grants. Running it over the finished book instead — which looks equivalent —
/// removes the noble skill the line above just granted, because noble skills
/// are (correctly) in no allow-list arm.
///
/// One consequence worth knowing: a nobless carrying legacy rows logs in, has
/// them reported and removed, and keeps the skills anyway. The audit line is
/// accurate — the rows really should not be there — and it stops after the
/// first login, because the flush no longer writes them.
#[test]
fn noble_skills_come_from_the_column_and_survive_the_check() {
    let mut world = world_with_one_class_skill(SkillCheckSettings {
        enable: true,
        remove: true,
        gm: true,
    });
    world
        .data
        .skill_trees
        .set_noble_skills_for_test(vec![(1323, 1)]);

    let mut chr = char_with_skills(4107, &[(1323, 1, 0)]);
    chr.noble = true;
    let bundle = crate::model::Player::from_char(&world.data, &chr);
    assert_eq!(
        book_of(&bundle),
        vec![(1323, 1)],
        "the nobless keeps Noblesse Blessing"
    );

    // …and a character who is *not* a nobless does not, however the row got
    // into their table.
    let chr = char_with_skills(4108, &[(1323, 1, 0)]);
    let bundle = crate::model::Player::from_char(&world.data, &chr);
    assert!(
        book_of(&bundle).is_empty(),
        "a non-nobless holding the row loses it"
    );
    assert_eq!(bundle.illegal_skills, vec![(1323, 1)]);
}

/// The persistence half of the same rule: hero and noble skills must not reach
/// `character_skills`, because both are derived on the way in and a stored row
/// would outlive the status.
#[test]
fn hero_and_noble_skills_are_filtered_out_of_the_flush() {
    let (mut world, ..) = test_world();
    world
        .data
        .skill_trees
        .set_noble_skills_for_test(vec![(1323, 1)]);
    world
        .data
        .skill_trees
        .set_hero_skills_for_test(vec![(395, 1)]);
    let _rx = ingame_player(&mut world, 1, 4109, 0, 0, 0);
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&4109) {
        book.0.insert(1323, 1);
        book.0.insert(395, 1);
        book.0.insert(3, 1);
    }

    let save = crate::game_loop::net::build_save_data(&world, 4109).expect("save data");
    let stored: Vec<i32> = save.skills.iter().map(|&(id, _, _)| id).collect();
    assert_eq!(
        stored,
        vec![3],
        "only the learned class skill is written; hero 395 and noble 1323 are not"
    );
}
