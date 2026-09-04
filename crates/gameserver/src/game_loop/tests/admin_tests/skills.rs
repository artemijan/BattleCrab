//! `admin/skills.rs` and `admin/hero.rs` — granting and removing skills and
//! buffs, and the hero status toggle.

use super::*;

/// Hero glow resolves from config: a GM gets it only when `GMHeroAura` is on;
/// a normal player never does. (`isHero()` is always false — no Olympiad yet.)
#[test]
fn hero_aura_resolves_from_gm_config() {
    let (mut world, ..) = admin_world();
    world.data.gm.hero_aura = true;

    // Master GM (level 100) with the aura on → hero glow.
    let _gm = ingame_player_access(&mut world, 1, 6401, 100);
    assert!(
        world
            .objects
            .get_component::<Player>(&6401)
            .unwrap()
            .hero_aura
    );

    // Normal player, aura on → still no glow (not a GM).
    let _user = ingame_player_access(&mut world, 2, 6402, 0);
    assert!(
        !world
            .objects
            .get_component::<Player>(&6402)
            .unwrap()
            .hero_aura
    );

    // Same GM with the aura off → no glow.
    world.data.gm.hero_aura = false;
    let _gm2 = ingame_player_access(&mut world, 3, 6403, 100);
    assert!(
        !world
            .objects
            .get_component::<Player>(&6403)
            .unwrap()
            .hero_aura
    );
}

/// `GMGiveSpecialSkills` / `GMGiveSpecialAuraSkills` — the GM convenience
/// kits from `gameMasterSkillTree.xml` and its aura twin.
///
/// The part worth pinning is that they are **session-only**. Java grants them
/// with `addSkill(skill, false)`, so a GM who logs in once must not carry
/// Super Haste in `character_skills` afterwards — least of all after the
/// config is turned back off.
#[test]
fn gm_special_skills_are_granted_but_never_persisted() {
    /// Super Haste, the first row of `gameMasterSkillTree.xml`.
    const SUPER_HASTE: i32 = 7029;

    let (mut world, ..) = admin_world();
    world.data.skill_trees = dist::skill_trees_owned();
    assert!(
        world.data.skill_trees.gm_skills(false).len() > 1
            && !world.data.skill_trees.gm_skills(true).is_empty(),
        "sanity: both GM trees really loaded from the dist"
    );

    // Off by default: no kit.
    let mut rx = ingame_player_access(&mut world, 1, 6431, 100);
    drain(&mut rx);
    admin::apply_gm_startup(&mut world, 1, 6431);
    assert!(
        !world
            .objects
            .get_component::<SkillBook>(&6431)
            .unwrap()
            .0
            .contains_key(&SUPER_HASTE),
        "no kit while the config is off"
    );

    // On: the kit lands in the live book.
    world.data.gm.give_special_skills = true;
    world.data.gm.give_special_aura_skills = true;
    let mut rx = ingame_player_access(&mut world, 2, 6432, 100);
    drain(&mut rx);
    admin::apply_gm_startup(&mut world, 2, 6432);
    let book = world
        .objects
        .get_component::<SkillBook>(&6432)
        .unwrap()
        .0
        .clone();
    assert!(book.contains_key(&SUPER_HASTE), "the kit is granted");

    // …and none of it reaches what would be written. This reads the real
    // save payload rather than re-asserting the predicate, so a filter that
    // stopped being applied would fail here even though `is_gm_skill` still
    // answered correctly.
    let saved = build_save_data(&world, 6432).expect("save data");
    for (id, ..) in &saved.skills {
        assert!(
            !world.data.skill_trees.is_gm_skill(*id),
            "GM skill {id} must never become a learned row"
        );
    }
    assert!(
        book.contains_key(&SUPER_HASTE) && !saved.skills.iter().any(|(id, ..)| *id == SUPER_HASTE),
        "granted in the live book, absent from the save — the whole point"
    );
}

/// `//add_skill <id> <lvl>` puts the skill in the target's book and refreshes
/// their SkillList; `//remove_skill` takes it back out.
#[test]
fn admin_add_and_remove_skill() {
    let (mut world, ..) = admin_world();
    world.data.skill_data = dist::skills_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8001, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("add_skill 1177 1"));
    assert_eq!(
        world
            .objects
            .get_component::<SkillBook>(&8001)
            .unwrap()
            .0
            .get(&1177),
        Some(&1),
        "skill added to the book"
    );
    assert!(
        drain(&mut gm_rx).iter().any(|p| p[0] == 0x5F),
        "SkillList refresh sent"
    );

    on_packet(&mut world, 1, build_admin("remove_skill 1177"));
    assert!(
        !world
            .objects
            .get_component::<SkillBook>(&8001)
            .unwrap()
            .0
            .contains_key(&1177),
        "skill removed"
    );
}

/// `//add_skill` with an unknown id is refused.
#[test]
fn admin_add_skill_rejects_unknown() {
    let (mut world, ..) = admin_world();
    world.data.skill_data = dist::skills_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8002, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("add_skill 99999999 1"));
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "does-not-exist line"
    );
}

/// `//buff <id>` applies the skill's effects (a buff) to the GM.
#[test]
fn admin_buff_applies_skill_to_self() {
    let (mut world, ..) = admin_world();
    world.data.skill_data = dist::skills_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8201, 100);
    drain(&mut gm_rx);

    let before = pbuffs(&world, 8201);
    on_packet(&mut world, 1, build_admin("buff 1068 1")); // Might
    assert!(pbuffs(&world, 8201) > before, "//buff applied a buff");
}

/// `//buff` with an unknown skill is refused.
#[test]
fn admin_buff_rejects_unknown_skill() {
    let (mut world, ..) = admin_world();
    world.data.skill_data = dist::skills_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8202, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("buff 99999999 1"));
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "does-not-exist line"
    );
}

/// `//getbuffs` lists the target's active buffs (header + one line per buff).
#[test]
fn admin_getbuffs_lists_active_buffs() {
    let (mut world, ..) = admin_world();
    world.data.skill_data = dist::skills_owned();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8401, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("buff 1068 1")); // Might
    drain(&mut gm_rx);
    // Java `showBuffs` renders the `getbuffs.htm` window with a per-buff row +
    // an `X` cancel button (not sysmessage lines).
    on_packet(&mut world, 1, build_admin("getbuffs"));
    let html = drain(&mut gm_rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("getbuffs html");
    assert!(!html.contains("My text is missing"), "getbuffs.htm found");
    assert!(
        html.contains("admin_stopbuff 8401 1068"),
        "buff row carries a cancel button"
    );
    // One buff is a single page, so Java leaves `%pages%` empty rather than
    // drawing a one-button pager.
    assert!(
        !html.contains("admin_getbuffs "),
        "no pager for a single page: {html}"
    );
}

/// `//getbuffs` pages at 3 buffs (Java `PageBuilder.newBuilder(effects, 3, …)`),
/// and the pager links carry the target's name so a page button works even when
/// the window was opened off a selection.
#[test]
fn admin_getbuffs_pages_at_three() {
    let (mut world, ..) = admin_world();
    world.data.skill_data = dist::skills_owned();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8402, 100);
    drain(&mut gm_rx);

    // Four buffs → two pages.
    for id in [1068, 1204, 1085, 1077] {
        on_packet(&mut world, 1, build_admin(&format!("buff {id} 1")));
    }
    drain(&mut gm_rx);

    let page = |world: &mut World, rx: &mut _, arg: &str| -> String {
        on_packet(world, 1, build_admin(arg));
        drain(rx)
            .iter()
            .find_map(|p| decode_npc_html(p))
            .expect("getbuffs html")
    };

    let first = page(&mut world, &mut gm_rx, "getbuffs");
    let rows = |h: &str| h.matches("admin_stopbuff").count();
    assert_eq!(rows(&first), 3, "page one holds three buffs: {first}");
    assert!(
        first.contains("admin_getbuffs "),
        "a pager appears past one page"
    );

    // Page two holds the remainder. The link the pager builds is what a GM
    // would click, so drive it rather than a hand-made command.
    let second = page(&mut world, &mut gm_rx, "getbuffs P8402 1");
    assert_eq!(rows(&second), 1, "page two holds the fourth: {second}");
}

/// `//stopbuff <id>` removes that one buff.
#[test]
fn admin_stopbuff_removes_one() {
    let (mut world, ..) = admin_world();
    world.data.skill_data = dist::skills_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8501, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("buff 1068 1"));
    let has = |w: &World| {
        w.objects
            .get_component::<Buffs>(&8501)
            .is_some_and(|b| b.0.iter().any(|x| x.skill_id == 1068))
    };
    assert!(has(&world), "Might applied");
    on_packet(&mut world, 1, build_admin("stopbuff 1068"));
    assert!(!has(&world), "Might removed by //stopbuff");
}

/// `//stopallbuffs` prompts (confirmDlg) and clears every buff on confirm.
#[test]
fn admin_stopallbuffs_clears_after_confirm() {
    let (mut world, ..) = admin_world();
    world.data.skill_data = dist::skills_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8502, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("buff 1068 1"));
    assert!(pbuffs(&world, 8502) >= 1, "a buff is active");

    // confirmDlg="true": prompts first, no clear yet.
    on_packet(&mut world, 1, build_admin("stopallbuffs"));
    assert!(pbuffs(&world, 8502) >= 1, "not cleared until confirmed");

    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    assert_eq!(pbuffs(&world, 8502), 0, "all buffs cleared after confirm");
}

/// `//setskill <id> <lvl>` grants the skill to the GM themselves.
#[test]
fn admin_setskill_adds_to_self() {
    let (mut world, ..) = admin_world();
    world.data.skill_data = dist::skills_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8902, 100);
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin("setskill 1177 1"));
    assert_eq!(
        world
            .objects
            .get_component::<SkillBook>(&8902)
            .unwrap()
            .0
            .get(&1177),
        Some(&1),
        "skill added to the GM"
    );
}

/// `//sethero` toggles hero status on the target: grants/removes the hero skill
/// tree and flips the aura; `//givehero` can't claim without an Olympiad-crowned
/// hero list. Port of AdminAdmin's hero commands.
#[test]
fn admin_sethero_toggles_status_skills_and_aura() {
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, ..) = admin_world();
    world.data.root = ROOT.to_string();
    world.data.skill_trees = dist::skill_trees_owned();
    let mut rx = ingame_player_access(&mut world, 1, 7301, 100);
    drain(&mut rx);
    // Target self (a player) so sethero applies to the GM.
    world.objects.add_components(&7301, TargetRef(Some(7301)));
    assert!(
        !world.data.skill_trees.hero_skills().is_empty(),
        "hero skill tree loaded from XML"
    );

    // //sethero → hero on: flag, aura, and the hero skills granted.
    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("sethero")].concat(),
    );
    let p = world.objects.get_component::<Player>(&7301).unwrap();
    assert!(p.is_hero && p.hero_aura, "hero status + aura on");
    assert!(
        world
            .objects
            .get_component::<SkillBook>(&7301)
            .unwrap()
            .0
            .contains_key(&395),
        "Heroic Miracle granted"
    );

    // //sethero again → hero off, skills removed.
    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("sethero")].concat(),
    );
    let p = world.objects.get_component::<Player>(&7301).unwrap();
    assert!(!p.is_hero, "hero status off");
    assert!(
        !world
            .objects
            .get_component::<SkillBook>(&7301)
            .unwrap()
            .0
            .contains_key(&395),
        "hero skill removed"
    );

    // //givehero (confirmDlg) → yes → cannot claim (no Olympiad hero list).
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("givehero")].concat(),
    );
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    let msgs: Vec<String> = drain(&mut rx)
        .iter()
        .filter_map(|p| sysmsg_text(p))
        .collect();
    assert!(
        msgs.iter()
            .any(|t| t.contains("cannot claim the hero status")),
        "givehero blocked without a crowned hero"
    );
}

/// `//give_clan_skills` end-to-end through the admin dispatch: a GM (access 100)
/// targeting a clan leader grants the clan its pledge skills, applies them, and
/// persists them (Java `AdminSkill.adminGiveClanSkills`).
#[test]
fn admin_give_clan_skills_command_grants_targeted_clan() {
    use crate::data::pledge_skill_tree::PledgeSkillLearn;
    use model::clan::{Clan, ClanMember};
    use model::components::combat::TargetRef;
    use model::components::skills::ClanSkills;

    let (mut world, _tx, mut db_rx, _link) = admin_world();
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(370));
    world.data.pledge_skill_trees.insert_for_test(
        PledgeSkillLearn {
            skill_id: 370,
            skill_level: 1,
            get_level: 3,
            social_class: Some(3),
            residencial: false,
            residence_ids: Vec::new(),
            level_up_sp: 0,
        },
        false,
    );

    let mut rx = ingame_player_access(&mut world, 1, 6500, 100);
    let clan_id = 0x3000_0077;
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "GmClan".into(),
            leader_id: 6500,
            level: 8,
            reputation_score: 0,
            castle_id: 0,
            members: vec![ClanMember {
                char_id: 6500,
                name: "P6500".into(),
                level: 80,
                class_id: 0,
                sex: 0,
                race: 0,
                power_grade: 5,
                title: String::new(),
                pledge_type: 0,
                apprentice: 0,
                sponsor: 0,
            }],
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    world
        .objects
        .get_component_mut::<Player>(&6500)
        .unwrap()
        .clan_id = clan_id;
    world.objects.add_components(&6500, TargetRef(Some(6500)));
    drain(&mut rx);
    drain_db(&mut db_rx);

    admin::use_admin_command(&mut world, 1, "admin_give_clan_skills", false);

    assert_eq!(
        world.clans[&clan_id].skills.get(&370),
        Some(&1),
        "clan learned the pledge skill"
    );
    assert!(
        world
            .objects
            .get_component::<ClanSkills>(&6500)
            .is_some_and(|c| c.0.contains_key(&370)),
        "skill applied to the online leader"
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::SaveClanSkill { skill_id: 370, .. })),
        "clan skill persisted"
    );
}

/// **`//getbuffs` follows an NPC target** (Java's gate is `isCreature()`, and
/// the `Buffs` button on the shift-click NPC window is the reason it exists).
/// Resolving through `target_player` — player target, else self — meant a GM
/// with a mob selected got their *own* buff list. The `<playername>` argument
/// form lands with it.
#[test]
fn getbuffs_follows_an_npc_target_and_a_name_argument() {
    let (mut world, ..) = admin_world();
    world.data.skill_data = dist::skills_owned();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut t = crate::data::npc_data::default_template(30001);
    t.name = "Buffed Mob".into();
    t.type_name = "Monster".into();
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30001, "Monster", 5, 100, 0, 0);
    let mut gm_rx = ingame_player_access(&mut world, 1, 8410, 100);
    // A buff on the GM (so a self-resolution would be visibly wrong) and one
    // on the mob.
    on_packet(&mut world, 1, build_admin("buff 1068 1")); // Might, on self
    world.objects.add_components(
        &NPC_OID,
        Buffs(vec![model::skill::active_buff::ActiveBuff {
            skill_id: 1204, // Wind Walk
            abnormal_type: "WIND_WALK".into(),
            abnormal_level: 1,
            expires_at_tick: world.tick + 1000,
            ..test_buff()
        }]),
    );
    handle_action(&mut world, 1, &action_body(NPC_OID, 0)); // target the mob
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("getbuffs"));

    let html = drain(&mut gm_rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("getbuffs html");
    assert!(
        html.contains("Buffed Mob"),
        "the window names the NPC target, got: {html}"
    );
    assert!(
        html.contains(&format!("admin_stopbuff {NPC_OID} 1204")),
        "the NPC's buff is listed with its cancel button, got: {html}"
    );
    assert!(
        !html.contains("admin_stopbuff 8410 1068"),
        "the GM's own buffs must not be what an NPC target shows"
    );

    // `//getbuffs <playername>` wins over the target (Java's first branch).
    let gm_name = world
        .objects
        .get_component::<Player>(&8410)
        .unwrap()
        .name
        .clone();
    on_packet(&mut world, 1, build_admin(&format!("getbuffs {gm_name}")));
    let html = drain(&mut gm_rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("getbuffs by name");
    assert!(
        html.contains("admin_stopbuff 8410 1068"),
        "the named player's buffs, not the target's, got: {html}"
    );
}

/// `//give_clan_skills` refuses in Java's order and with Java's two distinct
/// messages: no player target → INVALID_TARGET, a clanless target →
/// THE_TARGET_MUST_BE_A_CLAN_MEMBER. The two ids are the whole point of the
/// guard — the refusal paths had no cover before, so a swapped id was silent.
#[test]
fn give_clan_skills_refuses_with_javas_two_distinct_messages() {
    use model::components::combat::TargetRef;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7821, 100);
    let _clanless_rx = ingame_player_access(&mut world, 2, 7822, 0);
    drain(&mut gm_rx);

    // Nothing targeted → INVALID_TARGET.
    on_packet(&mut world, 1, build_admin("give_clan_skills"));
    let pkts = drain(&mut gm_rx);
    assert!(
        has_system_message(&pkts, server_packets::sm_ids::INVALID_TARGET),
        "no target → INVALID_TARGET"
    );
    assert!(
        !has_system_message(
            &pkts,
            server_packets::sm_ids::THE_TARGET_MUST_BE_A_CLAN_MEMBER
        ),
        "and NOT the clan-member message"
    );

    // A targeted player with no clan → THE_TARGET_MUST_BE_A_CLAN_MEMBER.
    world.objects.add_components(&7821, TargetRef(Some(7822)));
    on_packet(&mut world, 1, build_admin("give_clan_skills"));
    let pkts = drain(&mut gm_rx);
    assert!(
        has_system_message(
            &pkts,
            server_packets::sm_ids::THE_TARGET_MUST_BE_A_CLAN_MEMBER
        ),
        "clanless target → THE_TARGET_MUST_BE_A_CLAN_MEMBER"
    );
    assert!(
        !has_system_message(&pkts, server_packets::sm_ids::INVALID_TARGET),
        "and NOT INVALID_TARGET — a resolved player is a valid target"
    );
}

/// `Creature.stopAllEffects()` keeps passives — the single invariant that three
/// call sites (`//stopallbuffs`, `//areacancel`, the olympiad's pre-match
/// strip) now share through `expire_active_buffs`.
///
/// Passives carry grade penalties and the clan/residence pumps, which Java
/// never clears here. Dropping the filter passed the whole suite before this
/// test existed, so it is pinned at the shared helper's most dangerous edge.
#[test]
fn stop_all_buffs_clears_timed_buffs_and_keeps_passives() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7831, 100);
    let entry = |skill_id: i32, passive: bool| model::skill::active_buff::ActiveBuff {
        displayed: !passive,
        skill_id,
        abnormal_type: format!("T{skill_id}"),
        abnormal_level: 1,
        expires_at_tick: world.tick + 1000,
        passive,
        ..test_buff()
    };
    world.objects.add_components(
        &7831,
        Buffs(vec![
            entry(1204, false), // Wind Walk — timed
            entry(1078, false), // Concentration — timed
            entry(313, true),   // a passive, must survive
        ]),
    );
    drain(&mut gm_rx);

    // confirmDlg="true" in the datapack: the command only prompts, and the
    // DlgAnswer is what actually runs it.
    on_packet(&mut world, 1, build_admin("stopallbuffs"));
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );

    let left: Vec<(i32, bool)> = world
        .objects
        .get_component::<Buffs>(&7831)
        .map(|b| b.0.iter().map(|x| (x.skill_id, x.passive)).collect())
        .unwrap_or_default();
    assert_eq!(
        left,
        vec![(313, true)],
        "only the passive survives //stopallbuffs"
    );
}

/// `//remove_skills` is a *generated* per-character page in Java, not a file:
/// every row is a `bypass -h admin_remove_skill <id>` for a skill that
/// character actually knows. The port used to serve the static `skills.htm`,
/// from which a GM could not pick anything.
#[test]
fn admin_remove_skills_generates_the_targets_own_skill_list() {
    use model::components::combat::TargetRef;
    use model::components::skills::SkillBook;
    let (mut world, ..) = admin_world();
    world.data.skill_data = dist::skills_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7130, 100);
    let _victim_rx = ingame_player(&mut world, 2, 7131, 0, 0, 0);
    world.objects.add_components(&7130, TargetRef(Some(7131)));
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&7131) {
        book.0.clear();
        book.0.insert(1177, 1); // Wind Strike
    }
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("remove_skills"));
    let html = drain(&mut gm_rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .map(|p| String::from_utf8_lossy(&p).replace('\0', ""))
        .expect("an html page was sent");
    assert!(
        html.contains("admin_remove_skill 1177"),
        "the page offers the skill the target actually knows: {html:.400}"
    );
    assert!(
        html.contains("Wind Strike"),
        "and names it, so the GM can tell what they are clicking"
    );
}

/// `//skill_test <id>` plays a skill animation from the target (or the GM)
/// aimed at the GM. With no target it answers with the usage line, because
/// Java's null target throws into the same `catch`.
#[test]
fn admin_skill_test_plays_the_animation_at_the_gm() {
    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 7405, 100);
    add_test_npc(&mut world, NPC_OID, 20002, "Monster", 20, 100, 0, 0);
    // The animation needs a real skill to read `hitTime` off.
    world.data.skill_data.insert_for_test(Skill {
        id: 1177,
        level: 1,
        hit_time: 1500,
        ..Skill::default()
    });
    drain(&mut rx);

    // No target: the usage line, no packet.
    on_packet(&mut world, 1, build_admin("skill_test 1177"));
    let pkts = drain(&mut rx);
    assert_eq!(count_system_messages(&pkts), 1, "usage line");
    assert!(!has_opcode(&pkts, server_packets::opcodes::MAGIC_SKILL_USE));

    world
        .objects
        .add_components(&7405, TargetRef(Some(NPC_OID)));
    on_packet(&mut world, 1, build_admin("skill_test 1177"));
    assert!(
        has_opcode(&drain(&mut rx), server_packets::opcodes::MAGIC_SKILL_USE),
        "the targeted NPC casts it at the GM"
    );

    // An unknown skill id is the usage line again, not a packet.
    on_packet(&mut world, 1, build_admin("skill_test 999999"));
    let pkts = drain(&mut rx);
    assert!(!has_opcode(&pkts, server_packets::opcodes::MAGIC_SKILL_USE));
    assert_eq!(count_system_messages(&pkts), 1);
}
