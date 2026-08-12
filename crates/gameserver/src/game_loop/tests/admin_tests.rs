use super::*;
use crate::game_loop::admin;
use crate::game_loop::helpers::set_position;

/// A GM's `//serverinfo` runs and answers with server-info text lines.
#[test]
fn admin_serverinfo_runs_for_gm() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("serverinfo"),
        ]
        .concat(),
    );
    let pkts = drain(&mut gm_rx);
    assert_eq!(count_system_messages(&pkts), 3, "three server-info lines");
}

/// A non-GM issuing an admin command is silently ignored (Java `isGM` gate).
#[test]
fn admin_command_ignored_for_non_gm() {
    let (mut world, ..) = admin_world();
    let mut user_rx = ingame_player_access(&mut world, 1, 5002, 0);
    drain(&mut user_rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("serverinfo"),
        ]
        .concat(),
    );
    assert!(
        drain(&mut user_rx).is_empty(),
        "non-GM gets no reply at all"
    );
}

/// A GM whose tier lacks the required access level is refused with the Java
/// message. We synthesize a right the master tier's childAccess cannot reach by
/// using a real command but a mid-tier GM: `admin_serverinfo` needs level 100,
/// and a level-70 Admin's chain descends (never ascends) so it is denied.
#[test]
fn admin_command_access_denied_for_insufficient_level() {
    let (mut world, ..) = admin_world();
    // Level 70 ("Admin") is a GM (isGM=true) but its childAccess chain runs
    // 70→60→…→0, never reaching 100, so a level-100 command is refused.
    let mut rx = ingame_player_access(&mut world, 1, 5003, 70);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("serverinfo"),
        ]
        .concat(),
    );
    let pkts = drain(&mut rx);
    // One system message: the "no access rights" refusal, not the 3 info lines.
    assert_eq!(
        count_system_messages(&pkts),
        1,
        "single refusal line, command not run"
    );
}

/// An unknown command answers "does not exist"; a known-but-unimplemented
/// command (gated in AdminCommands.xml, no body yet — G13.C) answers the
/// not-implemented path. Both for a master GM.
#[test]
fn admin_unknown_vs_unimplemented() {
    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 5004, 100);
    drain(&mut rx);

    // Not in AdminCommands.xml → "does not exist".
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("totally_made_up"),
        ]
        .concat(),
    );
    assert_eq!(
        count_system_messages(&drain(&mut rx)),
        1,
        "does-not-exist line"
    );

    // In AdminCommands.xml (admin_instancezone, level 100) but no body yet (the
    // per-player instance-reuse view is deferred with reuse-time tracking) →
    // not-implemented path, does not crash.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("instancezone"),
        ]
        .concat(),
    );
    assert_eq!(
        count_system_messages(&drain(&mut rx)),
        1,
        "not-implemented line"
    );
}

/// A GM's name/title color comes from the access-level table; a normal player
/// keeps the client defaults.
#[test]
fn access_level_colors_applied() {
    let (world, ..) = admin_world();
    // Level 70 "Admin": nameColor/titleColor 0FF000 in AccessLevels.xml.
    let mut chr = dummy_char(6001, "Gm");
    chr.access_level = 70;
    let gm = Player::from_char(&world.data, &chr);
    assert_eq!(gm.player.name_color, 0x0F_F000);
    assert_eq!(gm.player.title_color, 0x0F_F000);

    // Level 0 keeps the client defaults (real-capture parity).
    let user = Player::from_char(&world.data, &dummy_char(6002, "Joe"));
    assert_eq!(user.player.name_color, crate::model::DEFAULT_NAME_COLOR);
    assert_eq!(user.player.title_color, crate::model::DEFAULT_TITLE_COLOR);
}

/// A `confirmDlg` command (admin_givehero) prompts with a ConfirmDlg and does
/// NOT execute; the DlgAnswer "yes" re-runs it (reaching dispatch — here the
/// not-implemented path), while "no" drops it silently.
#[test]
fn admin_confirm_dialog_round_trip() {
    const S1_3: i32 = server_packets::S1_3_MESSAGE_ID;

    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 5005, 100);
    drain(&mut rx);

    // //givehero → a single ConfirmDlg (0xF3), no execution yet.
    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("givehero")].concat(),
    );
    let pkts = drain(&mut rx);
    assert_eq!(pkts.len(), 1, "only the ConfirmDlg is sent");
    assert_eq!(
        pkts[0][0],
        server_packets::opcodes::CONFIRM_DLG,
        "it's a ConfirmDlg"
    );
    assert_eq!(
        count_system_messages(&pkts),
        0,
        "command did not execute yet"
    );

    // Answer "yes" → the stored command re-runs and reaches dispatch (givehero
    // has no body yet → the not-implemented reply proves re-execution).
    on_packet(
        &mut world,
        1,
        [vec![cop::DLG_ANSWER], dlg_answer_body(S1_3, 1, 0)].concat(),
    );
    assert_eq!(
        count_system_messages(&drain(&mut rx)),
        1,
        "re-ran on confirm"
    );

    // A second "yes" does nothing — the pending command was consumed.
    on_packet(
        &mut world,
        1,
        [vec![cop::DLG_ANSWER], dlg_answer_body(S1_3, 1, 0)].concat(),
    );
    assert!(drain(&mut rx).is_empty(), "no pending command to re-run");
}

/// Answering "no" to the confirm drops the command without executing it.
#[test]
fn admin_confirm_dialog_declined() {
    const S1_3: i32 = server_packets::S1_3_MESSAGE_ID;

    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 5006, 100);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("givehero")].concat(),
    );
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        [vec![cop::DLG_ANSWER], dlg_answer_body(S1_3, 0, 0)].concat(),
    );
    assert!(drain(&mut rx).is_empty(), "declined command does not run");
}

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

/// The GM startup block (`EnterWorld` GM branch) sets invul + invisible from
/// config, each gated by the `admin_invul`/`admin_invisible` access right.
#[test]
fn gm_startup_applies_invul_and_invisible() {
    let (mut world, ..) = admin_world();
    world.data.gm.startup_invulnerable = true;
    world.data.gm.startup_invisible = true;

    let mut rx = ingame_player_access(&mut world, 1, 6411, 100);
    drain(&mut rx);
    admin::apply_gm_startup(&mut world, 1, 6411);

    let f = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&6411)
        .copied()
        .unwrap_or_default();
    assert!(f.invul, "GMStartupInvulnerable applied");
    assert!(f.hidden, "GMStartupInvisible applied");
    assert!(!f.silence && !f.diet, "unset startup flags stay off");
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
            .get_component::<crate::model::components::SkillBook>(&6431)
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
        .get_component::<crate::model::components::SkillBook>(&6432)
        .unwrap()
        .0
        .clone();
    assert!(book.contains_key(&SUPER_HASTE), "the kit is granted");

    // …and none of it reaches what would be written. This reads the real
    // save payload rather than re-asserting the predicate, so a filter that
    // stopped being applied would fail here even though `is_gm_skill` still
    // answered correctly.
    let saved = crate::game_loop::net::build_save_data(&world, 6432).expect("save data");
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

/// `GMStartupBuilderHide` hides the GM and **breaks** the startup process, so
/// the invul/invisible/silence/diet flags below the break are not applied
/// (Java's `break gmStartupProcess`). The three "…default for builder" notices
/// are sent.
#[test]
fn gm_startup_builder_hide_short_circuits() {
    let (mut world, ..) = admin_world();
    world.data.gm.startup_builder_hide = true;
    world.data.gm.startup_invulnerable = true; // would apply if not short-circuited

    let mut rx = ingame_player_access(&mut world, 1, 6421, 100);
    drain(&mut rx);
    admin::apply_gm_startup(&mut world, 1, 6421);

    let f = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&6421)
        .copied()
        .unwrap_or_default();
    assert!(f.hidden, "builder hide set");
    assert!(!f.invul, "builder hide broke before the invul flag");
    assert_eq!(
        count_system_messages(&drain(&mut rx)),
        3,
        "three builder notices"
    );
}

/// `//admin` opens the main menu page — the real `main_menu.htm` is served (not
/// the missing-file placeholder) through an `NpcHtmlMessage`.
#[test]
fn admin_menu_serves_main_page() {
    let (mut world, ..) = admin_world();
    // Point the datapack root at dist/game so the html file resolves.
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6431, 100);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("admin")].concat(),
    );
    let pkts = drain(&mut rx);
    let html = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .expect("an NpcHtmlMessage was sent");

    // Decode: object_id (0) then the UTF-16 html string.
    let mut r = commons::network::PacketReader::new(&html[1..]);
    assert_eq!(r.read_i32().unwrap(), 0, "admin menu is not NPC-scoped");
    let content = r.read_string().unwrap();
    assert!(
        !content.contains("My text is missing"),
        "main_menu.htm was found"
    );
    assert!(
        content.contains("admin_admin"),
        "menu links back through the admin_ bypass"
    );
}

/// `//instancelist id=<t>` (G27) serves the real detail html with the live
/// instances of that template, each carrying teleport/destroy bypasses.
#[test]
fn admin_instance_detail_lists_live_instances() {
    use crate::data::instance_data::{ExitType, InstanceTemplate};
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();

    // A template with an empty default group (no NPC data needed) + a live copy.
    world
        .data
        .instance_templates
        .insert_for_test(InstanceTemplate {
            id: 900,
            name: Some("Test Arena".into()),
            max_worlds: -1,
            duration_min: 30,
            empty_destroy_min: 5,
            enter: Some((100, 200, 300)),
            exit: ExitType::Origin,
            doors: vec![],
            groups: vec![],
        });
    let iid = crate::game_loop::instances::create_from_template(&mut world, 900).expect("template");

    let mut rx = ingame_player_access(&mut world, 1, 6440, 100);
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("instancelist id=900"),
        ]
        .concat(),
    );

    let pkts = drain(&mut rx);
    let html = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .expect("NpcHtmlMessage");
    let mut r = commons::network::PacketReader::new(&html[1..]);
    r.read_i32().unwrap();
    let content = r.read_string().unwrap();
    assert!(
        !content.contains("My text is missing"),
        "the real detail htm was served"
    );
    assert!(
        content.contains("Test Arena (900)"),
        "template name + id shown"
    );
    assert!(
        content.contains(&format!("admin_instanceteleport {iid}")),
        "a Teleport button targets the live instance"
    );
    assert!(
        content.contains(&format!("admin_instancedestroy {iid}")),
        "a Destroy button targets the live instance"
    );
}

/// `//instancecreate <t>` builds the instance and moves the GM into it (Alone).
#[test]
fn admin_instancecreate_enters_the_gm() {
    use crate::data::instance_data::{ExitType, InstanceTemplate};
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    world
        .data
        .instance_templates
        .insert_for_test(InstanceTemplate {
            id: 901,
            name: Some("Solo".into()),
            max_worlds: -1,
            duration_min: 0,
            empty_destroy_min: 0,
            enter: Some((1000, 2000, 300)),
            exit: ExitType::Origin,
            doors: vec![],
            groups: vec![],
        });
    let mut rx = ingame_player_access(&mut world, 1, 6441, 100);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("instancecreate 901"),
        ]
        .concat(),
    );

    let iid = crate::game_loop::helpers::instance_of(&world, 6441);
    assert!(iid >= 1, "the GM entered a freshly-created instance");
    assert_eq!(
        world.instances.member_count(iid),
        1,
        "GM is the sole member"
    );
}

/// `//show_characters` and `//character_info` render HTML windows (Java
/// `listCharacters`/`showCharacterInfo`), not text lines: the regression the
/// user flagged.
#[test]
fn admin_editchar_info_commands_use_html() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6432, 100);
    drain(&mut rx);

    // //show_characters → charlist.htm (with the caller's own row).
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("show_characters"),
        ]
        .concat(),
    );
    let pkts = drain(&mut rx);
    let list = pkts
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("charlist html");
    assert!(!list.contains("My text is missing"), "charlist.htm found");
    assert!(list.contains("Character Selection"), "charlist body");
    assert!(
        list.contains("admin_character_info P6432"),
        "roster links to character_info"
    );
    assert!(
        !has_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE),
        "no sysmessage fallback"
    );

    // //character_info (self via target) → charinfo.htm filled with the name.
    world
        .objects
        .add_components(&6432, crate::model::components::TargetRef(Some(6432)));
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("character_info"),
        ]
        .concat(),
    );
    let info = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("charinfo html");
    assert!(!info.contains("My text is missing"), "charinfo.htm found");
    assert!(info.contains("P6432"), "charinfo shows the character name");
    assert!(
        !info.contains("%name%") && !info.contains("%level%"),
        "charinfo tokens replaced"
    );
    // Java `gatherCharacterInfo` shows the target's live client IP (the fixture
    // connects everyone from 127.0.0.1) — the port used to hardcode "N/A".
    assert!(
        info.contains("127.0.0.1"),
        "charinfo shows the client IP, got: {info}"
    );
    assert!(
        info.contains("admin_find_ip 127.0.0.1"),
        "the IP is a working find_ip link"
    );
    // `%protocol%` — Java's `client.getProtocolVersion()`. It lives on the
    // connection, so the game thread only has it because the handshake
    // forwards a `NetEvent::ProtocolVersion`; the port used to hardcode 0.
    world.protocol_versions.insert(1, 746);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("character_info"),
        ]
        .concat(),
    );
    let info = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("charinfo html");
    assert!(
        info.contains("746"),
        "charinfo shows the client protocol version, got: {info}"
    );
}

/// `//grandboss` opens the boss menu; `//grandboss <id>` shows one boss's live
/// status/respawn from `world.grand_bosses`; the per-boss action buttons hit the
/// unported boss AI (Java NPEs on the null AI, reproduced here). Port of
/// `AdminGrandBoss`.
#[test]
fn admin_grandboss_status_panel_and_actions() {
    use crate::model::grand_boss::GrandBoss;
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    // Queen Ant alive (status 0); Antharas dead (status 3) with a known respawn.
    world.grand_bosses.insert(
        29001,
        GrandBoss {
            boss_id: 29001,
            loc_x: 0,
            loc_y: 0,
            loc_z: 0,
            heading: 0,
            respawn_time: 0,
            current_hp: 1.0,
            current_mp: 1.0,
            status: 0,
        },
    );
    world.grand_bosses.insert(
        29068,
        GrandBoss {
            boss_id: 29068,
            loc_x: 0,
            loc_y: 0,
            loc_z: 0,
            heading: 0,
            respawn_time: 1_700_000_000_000,
            current_hp: 1.0,
            current_mp: 1.0,
            status: 3,
        },
    );
    let mut rx = ingame_player_access(&mut world, 1, 6440, 100);
    drain(&mut rx);

    // Menu: the six-boss list.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("grandboss"),
        ]
        .concat(),
    );
    let menu = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("grandboss menu html");
    assert!(!menu.contains("My text is missing"), "grandboss.htm found");
    assert!(
        menu.contains("admin_grandboss 29001"),
        "menu links to each boss"
    );

    // Queen Ant: alive → green, not-yet-respawned label.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("grandboss 29001"),
        ]
        .concat(),
    );
    let qa = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("queenant html");
    assert!(
        qa.contains("Alive") && qa.contains("00FF00"),
        "alive status + green color"
    );
    assert!(
        qa.contains("Already respawned!"),
        "alive boss is not awaiting respawn"
    );

    // Antharas: dead → red, formatted respawn date (UTC), zone count unported.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("grandboss 29068"),
        ]
        .concat(),
    );
    let an = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("antharas html");
    assert!(
        an.contains("Dead") && an.contains("FF0000"),
        "dead status + red color"
    );
    assert!(an.contains("2023-11-14 22:13:20"), "formatted respawn time");
    // Antharas has a nest zone (`NoRestartZone` 70050), so the panel shows a
    // **count** — 0 here, with nobody in the lair. The "Zone not found!"
    // fallback is for the four panel bosses Java pairs with no zone at all.
    assert!(
        an.contains("<font color=FF9900>0</font>") || an.contains(">0<"),
        "the nest's occupancy is counted, not stubbed"
    );
    assert!(
        !an.contains("Zone not found!"),
        "…and the not-found fallback is gone for a boss that has a zone"
    );

    // Action buttons: no arg → Usage; unsupported id → Wrong ID; supported id →
    // the dist's null-AI NPE, with no status page.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("grandboss_skip"),
        ]
        .concat(),
    );
    let m = drain(&mut rx);
    assert!(
        m.iter()
            .filter_map(|p| sysmsg_text(p))
            .any(|t| t == "Usage: //grandboss_skip Id")
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("grandboss_skip 29014"),
        ]
        .concat(),
    );
    let m = drain(&mut rx);
    assert!(
        m.iter()
            .filter_map(|p| sysmsg_text(p))
            .any(|t| t == "Wrong ID!"),
        "skip is Antharas-only"
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("grandboss_skip 29068"),
        ]
        .concat(),
    );
    let m = drain(&mut rx);
    assert!(
        m.iter()
            .filter_map(|p| sysmsg_text(p))
            .any(|t| t.contains("NullPointerException")),
        "unported AI reproduces the dist NPE"
    );
    assert!(
        !has_opcode(&m, server_packets::opcodes::NPC_HTML_MESSAGE),
        "NPE path shows no status page"
    );
}

/// `//cw_info` lists both cursed weapons; `//cw_add` activates one on the GM
/// (item + karma swap + cursed-weapon flag + DB persist + world announce);
/// `//cw_remove` reverses it. Port of `AdminCursedWeapons`.
#[test]
fn admin_cursed_weapons_info_add_remove() {
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, _db_tx, mut db_rx, _link) = admin_world();
    world.data.root = ROOT.to_string();
    // Boot-equivalent: load config, then build the runtime list (as net.rs does).
    world.data.cursed_weapons = crate::data::CursedWeaponData::load_from(ROOT);
    world.cursed_weapons = world
        .data
        .cursed_weapons
        .weapons
        .iter()
        .cloned()
        .map(|mut cw| {
            cw.skill_max_level = (1..=100)
                .take_while(|l| world.data.skill_data.get(cw.skill_id, *l).is_some())
                .last()
                .unwrap_or(1);
            cw
        })
        .collect();
    assert_eq!(
        world.cursed_weapons.len(),
        2,
        "Zariche + Akamanah loaded from XML"
    );

    world.id_pool = 0x3000_0000..0x3000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 7001, 100);
    drain(&mut rx);
    let original_rep = world
        .objects
        .get_component::<Player>(&7001)
        .unwrap()
        .reputation;

    // //cw_info — both weapons inactive.
    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("cw_info")].concat(),
    );
    let info: Vec<String> = drain(&mut rx)
        .iter()
        .filter_map(|p| sysmsg_text(p))
        .collect();
    assert!(
        info.iter()
            .any(|t| t.contains("Demonic Sword Zariche (8190)")),
        "lists Zariche"
    );
    assert!(
        info.iter().any(|t| t.contains("Don't exist in the world.")),
        "inactive status"
    );

    // //cw_add 8190 — Java marks it confirmDlg, so it prompts first; the "yes"
    // reply then activates it on the GM (no target).
    drain_db(&mut db_rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("cw_add 8190"),
        ]
        .concat(),
    );
    let prompt = drain(&mut rx);
    assert_eq!(
        prompt
            .iter()
            .filter(|p| p[0] == server_packets::opcodes::CONFIRM_DLG)
            .count(),
        1,
        "confirm prompt"
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
    let add_pkts = drain(&mut rx);
    let p = world.objects.get_component::<Player>(&7001).unwrap();
    assert_eq!(p.cursed_weapon_equipped_id, 8190, "cursed-weapon flag set");
    assert_eq!(
        p.reputation, -9_999_999,
        "karma slammed to the cursed value"
    );
    let cw = world
        .cursed_weapons
        .iter()
        .find(|c| c.item_id == 8190)
        .unwrap();
    assert!(
        cw.is_activated && cw.player_id == 7001,
        "weapon activated on the wielder"
    );
    assert_eq!(
        cw.player_reputation, original_rep,
        "saved the wielder's real karma"
    );
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            db::DbCommand::StoreCursedWeapon {
                item_id: 8190,
                char_id: 7001,
                ..
            }
        )),
        "persisted"
    );
    assert!(
        ids_after_opcode(&add_pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_OWNER_OF_S2_HAS_APPEARED_IN_THE_S1_REGION),
        "appearance announced"
    );

    // //cw_info now shows the wielder.
    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("cw_info")].concat(),
    );
    let info: Vec<String> = drain(&mut rx)
        .iter()
        .filter_map(|p| sysmsg_text(p))
        .collect();
    assert!(
        info.iter().any(|t| t.contains("Player holding: P7001")),
        "shows the holder"
    );

    // //cw_remove 8190 — end of life restores the wielder + resets state.
    drain_db(&mut db_rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("cw_remove 8190"),
        ]
        .concat(),
    );
    let rm_pkts = drain(&mut rx);
    let p = world.objects.get_component::<Player>(&7001).unwrap();
    assert_eq!(p.cursed_weapon_equipped_id, 0, "flag cleared");
    assert_eq!(p.reputation, original_rep, "karma restored");
    let cw = world
        .cursed_weapons
        .iter()
        .find(|c| c.item_id == 8190)
        .unwrap();
    assert!(!cw.is_active(), "weapon reset to not-in-world");
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter()
            .any(|c| matches!(c, db::DbCommand::RemoveCursedWeapon { item_id: 8190 })),
        "db row dropped"
    );
    assert!(
        ids_after_opcode(&rm_pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_HAS_DISAPPEARED),
        "disappearance announced"
    );
}

/// The `cwinfo.htm` panel draws its buttons from the weapon's live state —
/// "Give to Target" while it is nowhere, "Remove"/"Go" once it is live. Java
/// returns from `//cw_add` without touching the window, so the page kept
/// offering "Give to Target" and could not remove the sword it had just handed
/// out; the GM had to back out and re-enter `//cw_info_menu`. Both commands now
/// redraw the panel from the state they just changed.
#[test]
fn cursed_weapon_panel_redraws_after_give_and_remove() {
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, _db_tx, _db_rx, _link) = admin_world();
    world.data.root = ROOT.to_string();
    world.data.cursed_weapons = crate::data::CursedWeaponData::load_from(ROOT);
    world.cursed_weapons = world
        .data
        .cursed_weapons
        .weapons
        .iter()
        .cloned()
        .map(|mut cw| {
            cw.skill_max_level = (1..=100)
                .take_while(|l| world.data.skill_data.get(cw.skill_id, *l).is_some())
                .last()
                .unwrap_or(1);
            cw
        })
        .collect();
    world.id_pool = 0x3000_0000..0x3000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 7003, 100);
    drain(&mut rx);

    // The panel as the GM first sees it: nothing in the world, so the only
    // button on offer is "Give to Target".
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("cw_info_menu"),
        ]
        .concat(),
    );
    let first = last_admin_html(&drain(&mut rx)).expect("the panel opens");
    assert!(
        first.contains("admin_cw_add 8190") && !first.contains("admin_cw_remove 8190"),
        "a weapon that is nowhere offers only Give to Target"
    );

    // "Give to Target" → confirm → the weapon is live on the GM.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("cw_add 8190"),
        ]
        .concat(),
    );
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    let after_add = last_admin_html(&drain(&mut rx))
        .expect("the give redraws the panel instead of leaving it stale");
    assert!(
        after_add.contains("admin_cw_remove 8190"),
        "the redrawn page can remove what it just gave out"
    );
    assert!(
        after_add.contains("Weilder:"),
        "and shows the wielder row rather than the not-in-world one"
    );

    // "Remove" → the row goes back to offering the weapon.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("cw_remove 8190"),
        ]
        .concat(),
    );
    let after_remove = last_admin_html(&drain(&mut rx)).expect("the remove redraws the panel too");
    assert!(
        after_remove.contains("admin_cw_add 8190")
            && !after_remove.contains("admin_cw_remove 8190"),
        "back to Give to Target once the weapon is gone"
    );
}

/// The html body of the most recent `NpcHtmlMessage` in `packets`, if any.
fn last_admin_html(packets: &[Vec<u8>]) -> Option<String> {
    let pkt = packets
        .iter()
        .rev()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)?;
    let mut r = commons::network::PacketReader::new(&pkt[1..]);
    r.read_i32()?; // object id (0 for admin pages)
    r.read_string()
}

/// UserInfo's BASIC_INFO `isGM` byte is `player.isGM()` (Java `UserInfo` L147).
/// This is what tells the client to enable the `//command` bar — with a
/// hardcoded 0 the client never sends `SendBypassBuildCmd`, so no `//` command
/// ever reaches the server. A GM's UserInfo must carry `isGM=1`.
#[test]
fn user_info_isgm_byte_reflects_access_level() {
    let (mut world, ..) = admin_world();

    // Offset to the isGM byte: opcode(1) + object_id(4) + init_size(4) +
    // mask-count(2) + mask(3) + relation(4) + basic_info-len(2) +
    // name-len(2) + name(2·units) — the next byte is isGM.
    let isgm_byte = |world: &World, oid: i32, name: &str| -> u8 {
        let view = crate::model::PlayerView::of(&world.objects, oid).unwrap();
        let pkt = crate::network::user_info::user_info(&view, &world.data, &world.cfg.character, 0);
        let off = 1 + 4 + 4 + 2 + 3 + 4 + 2 + 2 + name.encode_utf16().count() * 2;
        pkt[off]
    };

    // Master GM (level 100) → isGM = 1.
    let _gm = ingame_player_access(&mut world, 1, 6461, 100);
    assert_eq!(
        isgm_byte(&world, 6461, "P6461"),
        1,
        "GM UserInfo enables the //command bar"
    );

    // Normal player (level 0) → isGM = 0.
    let _user = ingame_player_access(&mut world, 2, 6462, 0);
    assert_eq!(isgm_byte(&world, 6462, "P6462"), 0, "non-GM stays isGM=0");
}

/// `//silence` toggles message-refusal mode: on → MESSAGE_REFUSAL_MODE (177),
/// flag set, and an `EtcStatusUpdate` with the refusal bit so the client draws
/// the chat-block icon; a second toggle → MESSAGE_ACCEPTANCE_MODE (178), flag
/// cleared, and the bit cleared.
#[test]
fn admin_silence_toggles_refusal_mode() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6471, 100);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("silence")].concat(),
    );
    let pkts = drain(&mut rx);
    assert!(
        world
            .objects
            .get_component::<AdminFlags>(&6471)
            .unwrap()
            .silence,
        "silence on"
    );
    assert!(has_system_message(&pkts, 177), "MESSAGE_REFUSAL_MODE");
    assert_eq!(
        etc_status_mask(&pkts).map(|m| m & 1),
        Some(1),
        "EtcStatusUpdate refusal bit set"
    );

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("silence")].concat(),
    );
    let pkts = drain(&mut rx);
    assert!(
        !world
            .objects
            .get_component::<AdminFlags>(&6471)
            .unwrap()
            .silence,
        "silence off"
    );
    assert!(has_system_message(&pkts, 178), "MESSAGE_ACCEPTANCE_MODE");
    assert_eq!(
        etc_status_mask(&pkts).map(|m| m & 1),
        Some(0),
        "EtcStatusUpdate refusal bit cleared"
    );
}

/// `//hide` sends the GM's own client an `ExUserInfoAbnormalVisualEffect` with
/// the STEALTH effect present (so the invisible state renders), and clears it
/// on unhide.
#[test]
fn admin_hide_sends_stealth_visual() {
    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 6491, 100);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("hide")].concat(),
    );
    assert_eq!(
        ave_effect_count(&drain(&mut rx)),
        Some(1),
        "STEALTH present when hidden"
    );

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("hide")].concat(),
    );
    assert_eq!(
        ave_effect_count(&drain(&mut rx)),
        Some(0),
        "no effects when visible again"
    );
}

/// `//heal` on a targeted, damaged player fully restores HP/MP/CP and pushes a
/// StatusUpdate to that player.
#[test]
fn admin_heal_restores_targeted_player() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7001, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7002, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    if let Some(v) = world.objects.get_component_mut::<Vitals>(&7002) {
        v.cur_hp = 1.0;
    }
    world
        .objects
        .add_components(&7001, crate::model::components::TargetRef(Some(7002)));

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("heal")].concat(),
    );

    let v = pvit(&world, 7002);
    assert_eq!(v.cur_hp, v.max_hp as f64, "victim fully healed");
    assert!(
        drain(&mut victim_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::STATUS_UPDATE),
        "victim got a StatusUpdate"
    );
}

/// `//kill` on a targeted player kills them (Java `doDie` path).
#[test]
fn admin_kill_slays_targeted_player() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7003, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7004, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    world
        .objects
        .add_components(&7003, crate::model::components::TargetRef(Some(7004)));
    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("kill")].concat(),
    );

    assert!(pvit(&world, 7004).dead, "victim is dead after //kill");
}

/// `//kill` with no target tells the GM to select one and kills nothing.
#[test]
fn admin_kill_without_target_warns() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7005, 100);
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("kill")].concat(),
    );
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "one 'select a target' line"
    );
}

/// `//res` revives a dead targeted player and fully restores them.
#[test]
fn admin_res_revives_and_restores_target() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7101, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7102, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    if let Some(v) = world.objects.get_component_mut::<Vitals>(&7102) {
        v.cur_hp = 0.0;
        v.dead = true;
    }
    world
        .objects
        .add_components(&7101, crate::model::components::TargetRef(Some(7102)));
    on_packet(&mut world, 1, build_admin("res"));

    let v = pvit(&world, 7102);
    assert!(!v.dead, "victim revived");
    assert_eq!(v.cur_hp, v.max_hp as f64, "victim fully restored");
}

/// `//gmspeed N` sets the move multiplier to **N** (0 resets) and rebroadcasts
/// UserInfo.
///
/// Java names the argument `runSpeedBoost` but feeds it to `addFixedValue`, and
/// a fixed value is an *override* — `CreatureStat.getValue` returns it and never
/// calls the finalizer. So the speed becomes `base * N`, not `base * (1 + N)`,
/// and `//gmspeed 1` is a no-op. This asserted `1 + N`, one whole multiple of
/// base speed too fast at every setting except 0.
#[test]
fn admin_gmspeed_sets_move_multiplier() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7103, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("gmspeed 3"));
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::Speeds>(&7103)
            .unwrap()
            .move_multiplier,
        3.0,
        "the argument is the multiplier itself"
    );

    // The witness for the distinction: at 1 the fixed value equals the base, so
    // nothing moves. Under `1 + N` this would read 2.0.
    on_packet(&mut world, 1, build_admin("gmspeed 1"));
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::Speeds>(&7103)
            .unwrap()
            .move_multiplier,
        1.0,
        "//gmspeed 1 is a no-op, not double speed"
    );
    assert!(
        drain(&mut gm_rx).iter().any(|p| p[0] == 0x32),
        "UserInfo (0x32) rebroadcast"
    );

    on_packet(&mut world, 1, build_admin("gmspeed 0"));
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::Speeds>(&7103)
            .unwrap()
            .move_multiplier,
        1.0,
        "boost 0 resets"
    );
}

/// `//gmspeed` out of range answers the usage line and changes nothing.
#[test]
fn admin_gmspeed_rejects_out_of_range() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7107, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("gmspeed 99"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "usage line");
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::Speeds>(&7107)
            .unwrap()
            .move_multiplier,
        1.0,
        "unchanged"
    );
}

/// `//teleport x y z` moves the GM to those coordinates and broadcasts a
/// TeleportToLocation.
#[test]
fn admin_teleport_moves_gm_to_coords() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7104, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("teleport 100 200 300"));
    let pos = *world
        .objects
        .get_component::<crate::model::components::Position>(&7104)
        .unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (100, 200, 305),
        "moved to coords (z lifted by 5)"
    );
    assert!(
        drain(&mut gm_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION),
        "TeleportToLocation broadcast"
    );
}

/// `//recall <name>` brings the named online player to the GM's location.
#[test]
fn admin_recall_brings_player_to_gm() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7105, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 7106, 0);
    drain(&mut gm_rx);
    drain(&mut other_rx);

    set_position(&mut world, 7105, (500, 600, 700));
    on_packet(&mut world, 1, build_admin("recall P7106"));
    let pos = *world
        .objects
        .get_component::<crate::model::components::Position>(&7106)
        .unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (500, 600, 705),
        "recalled to GM position + 5 collision adjustment"
    );
}

/// The Character panel's "Go To" button (`admin_goto_char_menu <name>`) sends
/// the GM to the character already picked on the previous page — it must resolve
/// the name argument (Java `World.getPlayer(command.substring(21))`) and never
/// demand a live target, which is what the `//teleto` alias used to do.
#[test]
fn admin_goto_char_menu_uses_the_named_character_not_the_target() {
    use crate::model::components::Position;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7305, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 7306, 0);
    drain(&mut gm_rx);
    drain(&mut other_rx);

    set_position(&mut world, 7306, (1500, 1600, 1700));
    // Nothing selected on the GM: the button follows the name, not a target.
    assert!(
        world
            .objects
            .get_component::<crate::model::components::TargetRef>(&7305)
            .is_none_or(|t| t.0.is_none()),
        "GM has no target selected"
    );
    on_packet(&mut world, 1, build_admin("goto_char_menu P7306"));
    let pos = *world.objects.get_component::<Position>(&7305).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (1500, 1600, 1705),
        "GM teleported to the named character (+5 collision adjustment)"
    );

    // A stale target must not win over the name argument either.
    set_position(&mut world, 7306, (2500, 2600, 2700));
    world
        .objects
        .add_components(&7305, crate::model::components::TargetRef(Some(7305)));
    on_packet(&mut world, 1, build_admin("goto_char_menu P7306"));
    let pos = *world.objects.get_component::<Position>(&7305).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (2500, 2600, 2705),
        "the name argument beats the GM's own selection"
    );
}

/// `//create_item 57 1000` puts 1000 adena in the GM's inventory.
#[test]
fn admin_create_item_adds_to_gm_inventory() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut gm_rx = ingame_player_access(&mut world, 1, 7201, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("create_item 57 1000"));
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&7201)
            .unwrap()
            .count_of(57),
        1000,
        "1000 adena created"
    );
}

/// `//delete_item <objectId> [count]` trims a stack by the item's object id,
/// and a count of 0 destroys the whole stack (Java's `numval == 0`).
#[test]
fn admin_delete_item_trims_a_stack_by_object_id() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0200..0x4000_0300;
    let mut gm_rx = ingame_player_access(&mut world, 1, 7211, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("create_item 57 1000"));
    fn inv(w: &World) -> &crate::model::inventory::Inventory {
        w.objects
            .get_component::<crate::model::inventory::Inventory>(&7211)
            .unwrap()
    }
    let adena_oid = inv(&world)
        .items()
        .iter()
        .find(|it| it.item_id == 57)
        .expect("adena stack")
        .object_id;

    // Partial: 400 off the 1000.
    on_packet(
        &mut world,
        1,
        build_admin(&format!("delete_item {adena_oid} 400")),
    );
    assert_eq!(inv(&world).count_of(57), 600, "400 adena destroyed");

    // Count 0 means the whole remaining stack.
    on_packet(
        &mut world,
        1,
        build_admin(&format!("delete_item {adena_oid} 0")),
    );
    assert_eq!(inv(&world).count_of(57), 0, "stack destroyed outright");
}

/// `//delete_item` on an object id nobody online owns reports it and changes
/// nothing (Java's "Item doesn't have owner." / "Player is not online.").
#[test]
fn admin_delete_item_rejects_unowned_object_id() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0300..0x4000_0400;
    let mut gm_rx = ingame_player_access(&mut world, 1, 7212, 100);
    on_packet(&mut world, 1, build_admin("create_item 57 50"));
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("delete_item 123456789 1"));
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "one message, no destruction"
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&7212)
            .unwrap()
            .count_of(57),
        50,
        "inventory untouched"
    );
}

/// `//delete_quest_item <itemId> [count] [charName]`: no count clears the lot,
/// a count trims, and a trailing name overrides the target.
#[test]
fn admin_delete_quest_item_by_template_id() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0400..0x4000_0500;
    let mut gm_rx = ingame_player_access(&mut world, 1, 7213, 100);
    let _p_rx = ingame_player_access(&mut world, 2, 7214, 0);
    let pname = world
        .objects
        .get_component::<Player>(&7214)
        .unwrap()
        .name
        .clone();
    world
        .objects
        .add_components(&7213, crate::model::components::TargetRef(Some(7214)));
    crate::game_loop::items::add_inventory_item(&mut world, 7214, 57, 10);
    drain(&mut gm_rx);

    let held = |w: &World, oid: i32| {
        w.objects
            .get_component::<crate::model::inventory::Inventory>(&oid)
            .map(|i| i.count_of(57))
            .unwrap_or(0)
    };
    assert_eq!(held(&world, 7214), 10, "target stocked");

    // A count trims the target's stack.
    on_packet(&mut world, 1, build_admin("delete_quest_item 57 4"));
    assert_eq!(held(&world, 7214), 6, "4 destroyed off the target");

    // No count clears whatever is left.
    on_packet(&mut world, 1, build_admin("delete_quest_item 57"));
    assert_eq!(held(&world, 7214), 0, "no count = all of it");

    // A trailing name wins over the target: stock the GM, aim at the player.
    crate::game_loop::items::add_inventory_item(&mut world, 7213, 57, 8);
    assert_eq!(held(&world, 7213), 8, "GM stocked");
    on_packet(
        &mut world,
        1,
        build_admin(&format!("delete_quest_item 57 3 {pname}")),
    );
    assert_eq!(held(&world, 7213), 8, "named player, not the GM");
    on_packet(&mut world, 1, build_admin("delete_quest_item 57 3"));
    assert_eq!(held(&world, 7213), 8, "still the target, not the GM");

    // An unheld id reports and destroys nothing.
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin("delete_quest_item 2716"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "one message");
    assert_eq!(held(&world, 7213), 8, "nothing destroyed");
}

/// `//create_item` with a bogus id answers "does not exist" and adds nothing.
#[test]
fn admin_create_item_rejects_unknown_id() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7204, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("create_item 99999999 5"));
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "does-not-exist line"
    );
}

/// `//kick <name>` persists + despawns the target and drops their session.
#[test]
fn admin_kick_disconnects_target() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7202, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7203, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    // admin_kick carries confirmDlg="true", so it prompts first; answer "yes".
    on_packet(&mut world, 1, build_admin("kick P7203"));
    assert!(world.clients.contains_key(&2), "not kicked until confirmed");
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    assert!(
        !world.clients.contains_key(&2),
        "victim session removed after confirm"
    );
    assert!(
        world.objects.get_component::<Player>(&7203).is_none(),
        "victim despawned"
    );
}

/// `//add_exp_sp <exp> <sp>` grants exp and sp to the targeted player (driving
/// level-up). Java requires a player target, so the GM targets itself here.
#[test]
fn admin_add_exp_sp_grants_to_target() {
    let (mut world, ..) = admin_world();
    world.data.experience = crate::data::ExperienceData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    let mut gm_rx = ingame_player_access(&mut world, 1, 7301, 100);
    world
        .objects
        .add_components(&7301, crate::model::components::TargetRef(Some(7301)));
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("add_exp_sp 1000 500"));
    let p = world.objects.get_component::<Player>(&7301).unwrap();
    assert!(p.exp >= 1000, "exp granted");
    assert_eq!(p.sp, 500, "sp granted");
}

/// `//add_exp_sp_to_character` opens the real `expsp.htm` window
/// (`NpcHtmlMessage`) for the targeted player with its level/xp/sp filled in —
/// not chat text — matching Java's `addExpSp`.
#[test]
fn admin_add_exp_sp_to_character_opens_menu() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7301, 100);
    world
        .objects
        .add_components(&7301, crate::model::components::TargetRef(Some(7301)));
    if let Some(p) = world.objects.get_component_mut::<Player>(&7301) {
        p.level = 20;
        p.exp = 123456;
        p.sp = 789;
    }
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("add_exp_sp_to_character"));
    let pkts = drain(&mut gm_rx);
    let html = pkts
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("expsp.htm sent as NpcHtmlMessage");
    assert!(
        html.contains("admin_add_exp_sp"),
        "the Add/Remove button bypasses are present"
    );
    assert!(html.contains("123456"), "the player's xp is filled in");
    assert!(html.contains("789"), "the player's sp is filled in");
    assert!(
        !html.contains("%xp%") && !html.contains("%sp%"),
        "placeholders substituted"
    );
}

/// Picking a name off the `//show_characters` roster leaves the GM *targeting*
/// that character (Java `showCharacterInfo`'s `activeChar.setTarget(player)`),
/// so the `charinfo.htm` buttons that follow — `Lv/Exp/Sp` first among them —
/// act on him instead of answering INVALID_TARGET.
#[test]
fn admin_character_info_by_name_sets_target() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7311, 100);
    let _victim_rx = ingame_player_access(&mut world, 2, 7312, 0);
    if let Some(p) = world.objects.get_component_mut::<Player>(&7312) {
        p.level = 33;
        p.exp = 555111;
        p.sp = 4242;
    }
    drain(&mut gm_rx);

    // The roster link — `admin_character_info P7312` — with nothing targeted.
    on_packet(&mut world, 1, build_admin("character_info P7312"));
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::TargetRef>(&7311)
            .and_then(|t| t.0),
        Some(7312),
        "the listed character becomes the GM's target"
    );
    drain(&mut gm_rx);

    // The `Lv/Exp/Sp` button on charinfo.htm: no name argument, target only.
    on_packet(&mut world, 1, build_admin("add_exp_sp_to_character"));
    let pkts = drain(&mut gm_rx);
    assert!(
        !pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::INVALID_TARGET),
        "no INVALID_TARGET after picking the character from the list"
    );
    let html = pkts
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("expsp.htm sent for the listed character");
    assert!(
        html.contains("555111") && html.contains("4242"),
        "the menu carries the listed character's xp/sp, not the GM's"
    );
}

/// `//add_exp_sp` with no player target is refused (Java `INVALID_TARGET`),
/// not silently applied to the GM.
#[test]
fn admin_add_exp_sp_without_target_is_invalid() {
    let (mut world, ..) = admin_world();
    world.data.experience = crate::data::ExperienceData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    let mut gm_rx = ingame_player_access(&mut world, 1, 7301, 100);
    let exp_before = world.objects.get_component::<Player>(&7301).unwrap().exp;
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("add_exp_sp 1000 500"));
    assert_eq!(
        world.objects.get_component::<Player>(&7301).unwrap().exp,
        exp_before,
        "no self-grant without a target"
    );
    let pkts = drain(&mut gm_rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::INVALID_TARGET),
        "INVALID_TARGET sent",
    );
}

/// `//set_level N` sets the target's level; `//add_level N` adds to it.
#[test]
fn admin_set_and_add_level() {
    let (mut world, ..) = admin_world();
    world.data.experience = crate::data::ExperienceData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    let mut gm_rx = ingame_player_access(&mut world, 1, 7305, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("set_level 20"));
    assert_eq!(
        world.objects.get_component::<Player>(&7305).unwrap().level,
        20,
        "set to 20"
    );

    on_packet(&mut world, 1, build_admin("add_level 5"));
    assert_eq!(
        world.objects.get_component::<Player>(&7305).unwrap().level,
        25,
        "added 5"
    );
}

/// `//gmchat` reaches every online GM (including the sender) but no normal
/// player.
#[test]
fn admin_gmchat_broadcasts_to_gms_only() {
    let (mut world, ..) = admin_world();
    let mut gm1 = ingame_player_access(&mut world, 1, 7302, 100);
    let mut gm2 = ingame_player_access(&mut world, 2, 7303, 100);
    let mut user = ingame_player_access(&mut world, 3, 7304, 0);
    drain(&mut gm1);
    drain(&mut gm2);
    drain(&mut user);

    on_packet(&mut world, 1, build_admin("gmchat hello gms"));
    let say = server_packets::opcodes::SAY2;
    assert!(
        drain(&mut gm1).iter().any(|p| p[0] == say),
        "sender GM sees it"
    );
    assert!(
        drain(&mut gm2).iter().any(|p| p[0] == say),
        "other GM sees it"
    );
    assert!(
        drain(&mut user).iter().all(|p| p[0] != say),
        "normal player does not"
    );
}

/// `//changelvl <name> <level>` promotes a player, updates colors/is_gm, and
/// queues the persisting DB update.
#[test]
fn admin_changelvl_sets_access_and_persists() {
    let (mut world, _db_tx, mut db_rx, _link) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7401, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7402, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    on_packet(&mut world, 1, build_admin("changelvl P7402 70"));
    let p = world.objects.get_component::<Player>(&7402).unwrap();
    assert_eq!(p.access_level, 70, "promoted to 70");
    assert!(p.is_gm(&world.data), "now a GM");
    assert_eq!(p.name_color, 0x0F_F000, "tier color applied");
    assert!(
        drain_db(&mut db_rx).iter().any(|c| matches!(
            c,
            db::DbCommand::SetAccessLevel {
                char_id: 7402,
                level: 70
            }
        )),
        "access-level UPDATE queued"
    );
}

/// `//changelvl` to an undefined level is refused and changes nothing.
#[test]
fn admin_changelvl_rejects_unknown_level() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7404, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("changelvl 55"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&7404)
            .unwrap()
            .access_level,
        100,
        "unchanged"
    );
}

/// `//gm` deactivates the caller's own GM access for the session (not persisted).
#[test]
fn admin_gm_deactivates_own_access() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7403, 100);
    drain(&mut gm_rx);
    assert!(
        world
            .objects
            .get_component::<Player>(&7403)
            .unwrap()
            .is_gm(&world.data)
    );

    on_packet(&mut world, 1, build_admin("gm"));
    let p = world.objects.get_component::<Player>(&7403).unwrap();
    assert_eq!(p.access_level, 0, "demoted to user");
    assert!(!p.is_gm(&world.data), "no longer GM");
}

/// `//announce` reaches every online player.
#[test]
fn admin_announce_reaches_all_players() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7501, 100);
    let mut u1 = ingame_player_access(&mut world, 2, 7502, 0);
    let mut u2 = ingame_player_access(&mut world, 3, 7503, 0);
    drain(&mut gm_rx);
    drain(&mut u1);
    drain(&mut u2);

    on_packet(&mut world, 1, build_admin("announce server restart soon"));
    assert_eq!(
        count_system_messages(&drain(&mut u1)),
        1,
        "player 1 got the announce"
    );
    assert_eq!(
        count_system_messages(&drain(&mut u2)),
        1,
        "player 2 got the announce"
    );
}

/// `//character_disconnect` disconnects the targeted player.
#[test]
fn admin_character_disconnect_kicks_target() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7504, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7505, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    world
        .objects
        .add_components(&7504, crate::model::components::TargetRef(Some(7505)));
    on_packet(&mut world, 1, build_admin("character_disconnect"));
    assert!(!world.clients.contains_key(&2), "victim disconnected");
    assert!(
        world.objects.get_component::<Player>(&7505).is_none(),
        "victim despawned"
    );
}

/// `//delete` despawns the targeted NPC and broadcasts DeleteObject.
#[test]
fn admin_delete_despawns_targeted_npc() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7601, 100);
    drain(&mut gm_rx);

    let npc_oid = crate::model::npc::FIRST_NPC_OBJECT_ID + 1;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 1, 2, 3, 100, 50);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    world
        .objects
        .add_components(&7601, crate::model::components::TargetRef(Some(npc_oid)));

    on_packet(&mut world, 1, build_admin("delete"));
    assert!(
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
            .is_none(),
        "npc despawned by //delete"
    );
    assert!(
        drain(&mut gm_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT),
        "GM got DeleteObject"
    );
}

/// `//delete` with a non-NPC target (or none) warns and deletes nothing.
#[test]
fn admin_delete_without_npc_target_warns() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7603, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("delete"));
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "select-an-npc line"
    );
}

/// `//spawn` with an unknown NPC id is refused.
#[test]
fn admin_spawn_rejects_unknown_npc() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7602, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("spawn 99999"));
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "does-not-exist line"
    );
}

/// `//spawn <npcId>` creates the NPC at the GM's location and shows it to them.
#[test]
fn admin_spawn_creates_npc_at_gm() {
    let (mut world, ..) = admin_world();
    world.data.npc_data = dist::npcs_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7604, 100);
    drain(&mut gm_rx);
    set_position(&mut world, 7604, (100, 200, 300));

    let npc_oid = world.next_npc_object_id;
    on_packet(&mut world, 1, build_admin("spawn 30001")); // Lector, a Merchant (non-monster)
    assert_eq!(world.next_npc_object_id, npc_oid + 1, "one NPC spawned");
    let npc = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .expect("npc entity exists");
    assert_eq!(npc.npc_id, 30001);
    let pos = world
        .objects
        .get_component::<crate::model::components::Position>(&npc_oid)
        .unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (100, 200, 300), "spawned at the GM");
    assert!(
        drain(&mut gm_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_INFO),
        "GM was shown the NPC"
    );
}

/// `//target <name>` selects that player (MyTargetSelected + TargetRef set).
#[test]
fn admin_target_selects_named_player() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7701, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7702, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    on_packet(&mut world, 1, build_admin("target P7702"));
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::TargetRef>(&7701)
            .and_then(|t| t.0),
        Some(7702),
        "GM now targets the named player"
    );
    assert!(
        drain(&mut gm_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MY_TARGET_SELECTED),
        "GM got MyTargetSelected"
    );
}

/// `//invul` toggles invulnerability; incoming damage is ignored while on.
#[test]
fn admin_invul_blocks_damage() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7801, 100);
    drain(&mut gm_rx);
    // The synthetic template has no HP table; give the player real HP.
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&7801) {
        v.max_hp = 1000;
        v.cur_hp = 1000.0;
        v.dead = false;
    }

    on_packet(&mut world, 1, build_admin("invul"));
    assert!(
        world
            .objects
            .get_component::<crate::model::components::AdminFlags>(&7801)
            .unwrap()
            .invul
    );

    let hp_before = pvit(&world, 7801).cur_hp;
    super::combat::player_receive_damage(&mut world, 7801, 12345, 50.0);
    assert_eq!(
        pvit(&world, 7801).cur_hp,
        hp_before,
        "invul: no damage taken"
    );

    // Toggle off → damage lands.
    on_packet(&mut world, 1, build_admin("invul"));
    super::combat::player_receive_damage(&mut world, 7801, 12345, 50.0);
    assert!(
        pvit(&world, 7801).cur_hp < hp_before,
        "damage applies once invul is off"
    );
}

/// `//undying` lets damage apply but never kills — HP floors at 1.
#[test]
fn admin_undying_floors_hp_at_one() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7802, 100);
    drain(&mut gm_rx);
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&7802) {
        v.max_hp = 1000;
        v.cur_hp = 1000.0;
        v.dead = false;
    }

    on_packet(&mut world, 1, build_admin("undying"));
    super::combat::player_receive_damage(&mut world, 7802, 12345, 100_000.0);
    let v = pvit(&world, 7802);
    assert_eq!(v.cur_hp, 1.0, "undying floors HP at 1");
    assert!(!v.dead, "undying player does not die");
}

/// `//setinvul` toggles invulnerability on the targeted player.
#[test]
fn admin_setinvul_targets_a_player() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7803, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 7804, 0);
    drain(&mut gm_rx);
    drain(&mut other_rx);

    world
        .objects
        .add_components(&7803, crate::model::components::TargetRef(Some(7804)));
    on_packet(&mut world, 1, build_admin("setinvul"));
    assert!(
        world
            .objects
            .get_component::<crate::model::components::AdminFlags>(&7804)
            .unwrap()
            .invul
    );
}

/// `//hide` removes the GM from nearby players' view (DeleteObject) and toggling
/// it off re-introduces them (CharInfo).
#[test]
fn admin_hide_toggles_visibility() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7901, 100);
    let mut obs_rx = ingame_player_access(&mut world, 2, 7902, 0);
    drain(&mut gm_rx);
    drain(&mut obs_rx);

    on_packet(&mut world, 1, build_admin("hide"));
    assert!(
        world
            .objects
            .get_component::<crate::model::components::AdminFlags>(&7901)
            .unwrap()
            .hidden
    );
    assert!(
        drain(&mut obs_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT
                && i32::from_le_bytes([p[1], p[2], p[3], p[4]]) == 7901),
        "observer got DeleteObject for the hidden GM"
    );

    on_packet(&mut world, 1, build_admin("hide"));
    assert!(
        !world
            .objects
            .get_component::<crate::model::components::AdminFlags>(&7901)
            .unwrap()
            .hidden
    );
    assert!(
        drain(&mut obs_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::CHAR_INFO),
        "observer got CharInfo when the GM reappeared"
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
            .get_component::<crate::model::components::SkillBook>(&8001)
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
            .get_component::<crate::model::components::SkillBook>(&8001)
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

/// `//setew <n>` sets the enchant level of the equipped weapon.
#[test]
fn admin_setew_enchants_equipped_weapon() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8101, 100);
    drain(&mut gm_rx);
    // Equip a weapon (item 1, the starter gloves aside — any weapon id) in RHand.
    let weapon = crate::character::ItemRow {
        object_id: 50000,
        item_id: 1,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: crate::model::inventory::PaperdollSlot::RHand as i32,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    };
    world.objects.add_components(
        &8101,
        crate::model::inventory::Inventory::from_rows(&[weapon]),
    );

    on_packet(&mut world, 1, build_admin("setew 10"));
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&8101)
            .unwrap()
            .paperdoll_enchant_level(crate::model::inventory::PaperdollSlot::RHand),
        10,
        "weapon enchanted to +10"
    );
}

/// `//setew` with no weapon equipped warns.
#[test]
fn admin_setew_without_weapon_warns() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8102, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("setew 10"));
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "no-item-in-slot line"
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

/// `//superhaste` applies and **persists**: Super Haste (7029) is a toggle with
/// no `abnormalTime`, so the buff must be permanent (Java `EffectList` never
/// schedules its stop) — it previously expired the same tick it landed.
#[test]
fn admin_superhaste_applies_and_persists() {
    use crate::model::components::{Buffs, Speeds};
    let (mut world, ..) = admin_world();
    // Full datapack, not just `skill_data`: Super Haste also carries
    // `MpConsumePerLevel` (G19) — Java's `AdminSuperHaste` casts it through
    // the real `applyEffects` path (`superHasteSkill.applyEffects(player,
    // player, true, time)`), so it drains MP like any other toggle. The drain
    // is negligible (`power` 0.0001) but still needs a real MP pool: with
    // `for_test()`'s empty `player_templates` a level-1 dummy char computes 0
    // max MP, and the very first tick would exceed it and cancel the toggle.
    world.data = dist::game_data_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8202, 100);
    drain(&mut gm_rx);

    let base_spd = world
        .objects
        .get_component::<Speeds>(&8202)
        .unwrap()
        .run_spd;
    on_packet(&mut world, 1, build_admin("superhaste 2"));

    // The buff is present, permanent, and raised run speed.
    let buff = world
        .objects
        .get_component::<Buffs>(&8202)
        .unwrap()
        .0
        .iter()
        .find(|b| b.skill_id == 7029)
        .cloned();
    let buff = buff.expect("super-haste buff applied");
    assert_eq!(buff.expires_at_tick, u64::MAX, "toggle buff is permanent");
    assert!(
        world
            .objects
            .get_component::<Speeds>(&8202)
            .unwrap()
            .run_spd
            > base_spd,
        "run speed increased"
    );

    // No BuffExpire was scheduled, so advancing the world keeps it.
    world.tick += 100;
    crate::game_loop::apply_due_tasks(&mut world);
    assert!(
        world
            .objects
            .get_component::<Buffs>(&8202)
            .unwrap()
            .0
            .iter()
            .any(|b| b.skill_id == 7029),
        "still active after ticks"
    );

    // //superhaste 0 clears it.
    on_packet(&mut world, 1, build_admin("superhaste 0"));
    assert!(
        !world
            .objects
            .get_component::<Buffs>(&8202)
            .unwrap()
            .0
            .iter()
            .any(|b| b.skill_id == 7029),
        "cleared by level 0"
    );
}

/// `Speeds::client_move_multiplier` is Java's `getMovementSpeedMultiplier`
/// (moveSpeed ÷ raw template base) — the leg-animation rate. A stat speed buff
/// raises `run_spd` and must raise the multiplier proportionally; a bare
/// `move_multiplier` there left buffed characters gliding with base-cadence legs
/// (the reported Super Haste "slow legs" symptom). `//gmspeed` keeps working
/// because it scales through `move_multiplier`, which folds in via `move_speed`.
#[test]
fn client_move_multiplier_tracks_speed_buffs() {
    use crate::model::components::Speeds;
    // base template run 132, +35 RunSpeedBoost folded into run_spd → 167 at rest.
    let mut s = Speeds {
        run_spd: 167.0,
        walk_spd: 90.0,
        swim_run_spd: 0.0,
        swim_walk_spd: 0.0,
        move_multiplier: 1.0,
        base_run_spd: 132.0,
        base_walk_spd: 88.0,
        base_swim_run_spd: 50.0,
        base_swim_walk_spd: 50.0,
        running: true,
        swimming: false,
        swamp_multiplier: 1.0,
    };
    // At rest it matches Java exactly: 167 / 132.
    assert!((s.client_move_multiplier() - 167.0 / 132.0).abs() < 1e-9);
    // Super Haste ×4 on run_spd → the multiplier quadruples with it.
    s.run_spd = 668.0;
    assert!((s.client_move_multiplier() - 668.0 / 132.0).abs() < 1e-9);
    // //gmspeed (move_multiplier) still folds through move_speed().
    s.run_spd = 167.0;
    s.move_multiplier = 4.0;
    assert!((s.client_move_multiplier() - 167.0 * 4.0 / 132.0).abs() < 1e-9);
    // Unknown base (0) is a safe no-op multiplier.
    s.base_run_spd = 0.0;
    assert_eq!(s.client_move_multiplier(), 1.0);
}

/// `CombatStats::client_atk_speed_multiplier` is Java's `getAttackSpeedMultiplier`
/// (`Formulas.calcAtkSpdMultiplier` = `pAtkSpd / 333`) — the swing-animation rate,
/// the haste counterpart of the move multiplier. Super Haste ×4 on `p_atk_spd`
/// must scale the swing animation with it; the old hardcoded `1.0` left it at
/// base cadence.
#[test]
fn client_atk_speed_multiplier_tracks_haste() {
    use crate::model::components::CombatStats;
    let mut c = CombatStats {
        p_atk_spd: 300,
        ..Default::default()
    };
    // Base p_atk_spd 300 → 300 / 333 (matches Java calcAtkSpdMultiplier).
    assert!((c.client_atk_speed_multiplier() - 300.0 / 333.0).abs() < 1e-9);
    // Super Haste ×4 on p_atk_spd → the multiplier quadruples with it.
    c.p_atk_spd = 1200;
    assert!((c.client_atk_speed_multiplier() - 1200.0 / 333.0).abs() < 1e-9);
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

/// The `//editchar` field setters mutate the targeted player and broadcast.
#[test]
fn admin_editchar_field_setters() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8301, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 8302, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);
    world
        .objects
        .add_components(&8301, crate::model::components::TargetRef(Some(8302)));

    let p = |w: &World| w.objects.get_component::<Player>(&8302).unwrap().clone();

    on_packet(&mut world, 1, build_admin("setreputation -500"));
    assert_eq!(p(&world).reputation, -500);
    on_packet(&mut world, 1, build_admin("nokarma"));
    assert_eq!(p(&world).reputation, 0);
    on_packet(&mut world, 1, build_admin("setpk 7"));
    assert_eq!(p(&world).pk_kills, 7);
    on_packet(&mut world, 1, build_admin("setpvp 9"));
    assert_eq!(p(&world).pvp_kills, 9);
    on_packet(&mut world, 1, build_admin("setfame 42"));
    assert_eq!(p(&world).fame, 42);
    on_packet(&mut world, 1, build_admin("settitle Hello World"));
    assert_eq!(p(&world).title, "Hello World");
    on_packet(&mut world, 1, build_admin("setcolor FF0000"));
    assert_eq!(p(&world).name_color, 0xFF_0000);
    assert!(!p(&world).is_female);
    on_packet(&mut world, 1, build_admin("setsex"));
    assert!(p(&world).is_female, "gender flipped");
}

/// `//set_hp <n>` sets the caster's current HP (clamped to max).
#[test]
fn admin_set_hp_sets_current_hp() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8303, 100);
    drain(&mut gm_rx);
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&8303) {
        v.max_hp = 500;
        v.cur_hp = 500.0;
        v.dead = false;
    }

    on_packet(&mut world, 1, build_admin("set_hp 100"));
    assert_eq!(pvit(&world, 8303).cur_hp, 100.0, "HP set to 100");
    // Clamps above max.
    on_packet(&mut world, 1, build_admin("set_hp 99999"));
    assert_eq!(pvit(&world, 8303).cur_hp, 500.0, "clamped to max");
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
            .get_component::<crate::model::components::Buffs>(&8501)
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

/// `//setclass <id>` changes the target's class and recomputes their template.
#[test]
fn admin_setclass_changes_class() {
    let (mut world, ..) = admin_world();
    world.data.player_templates = dist::player_templates_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8701, 100);
    drain(&mut gm_rx);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8701)
            .unwrap()
            .class_id,
        0
    );

    on_packet(&mut world, 1, build_admin("setclass 1"));
    let p = world.objects.get_component::<Player>(&8701).unwrap();
    assert_eq!(p.class_id, 1, "class changed to 1");
    assert_eq!(p.base_class_id, 1);
}

/// `//setclass` to an advanced class recalculates the skill set: with
/// `AutoLearnSkills`, the target gains the new class's reachable skills —
/// including ancestor-tier and common (Expertise) skills through the complete
/// class tree — not just the base-class ones.
#[test]
fn admin_setclass_grants_advanced_class_skills() {
    let (mut world, ..) = admin_world();
    world.data.player_templates = dist::player_templates_owned();
    world.data.skill_trees = dist::skill_trees_owned();
    world.cfg.character.auto_learn_skills = true;
    let mut gm_rx = ingame_player_access(&mut world, 1, 8703, 100);
    drain(&mut gm_rx);
    if let Some(p) = world.objects.get_component_mut::<Player>(&8703) {
        p.level = 40; // Warlord's 2nd-class skills gate at getLevel 40.
    }

    on_packet(&mut world, 1, build_admin("setclass 3")); // Warlord (2nd class)

    let p = world.objects.get_component::<Player>(&8703).unwrap();
    assert_eq!(p.class_id, 3);
    let book = world
        .objects
        .get_component::<crate::model::components::SkillBook>(&8703)
        .unwrap();
    assert!(book.0.contains_key(&36), "gained Warlord's Whirlwind (36)");
    assert!(
        book.0.contains_key(&239),
        "gained common Expertise (239) via the complete tree"
    );
}

/// `//setclass` with an unknown class id is refused.
#[test]
fn admin_setclass_rejects_unknown() {
    let (mut world, ..) = admin_world();
    world.data.player_templates = dist::player_templates_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8702, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("setclass 99999"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8702)
            .unwrap()
            .class_id,
        0,
        "unchanged"
    );
}

/// `//social <id>` broadcasts a `SocialAction` on the GM (self, no target).
#[test]
fn admin_social_broadcasts_gesture() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8801, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("social 3"));
    let pkts = drain(&mut gm_rx);
    let social = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION)
        .expect("a SocialAction was broadcast");
    assert_eq!(
        i32::from_le_bytes(social[1..5].try_into().unwrap()),
        8801,
        "on the GM"
    );
    assert_eq!(
        i32::from_le_bytes(social[5..9].try_into().unwrap()),
        3,
        "action id 3"
    );
}

/// A player-invalid social id (< 2) is rejected with `NOTHING_HAPPENED` and no
/// gesture is sent.
#[test]
fn admin_social_rejects_out_of_range_action() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8802, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("social 1"));
    let pkts = drain(&mut gm_rx);
    assert!(
        !pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION),
        "no gesture for an out-of-range action"
    );
    assert!(count_system_messages(&pkts) >= 1, "NOTHING_HAPPENED sent");
}

/// `//social <id> <radius>` affects other creatures within the radius.
#[test]
fn admin_social_radius_affects_nearby_player() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8803, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 8804, 0);
    drain(&mut gm_rx);
    drain(&mut other_rx);
    // Place both at the same spot so the other is in range and region-adjacent.
    let pos = *world
        .objects
        .get_component::<crate::model::components::Position>(&8803)
        .unwrap();
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::Position>(&8804)
    {
        *p = pos;
    }

    on_packet(&mut world, 1, build_admin("social 3 500"));
    assert!(
        drain(&mut other_rx).iter().any(|p| {
            p[0] == server_packets::opcodes::SOCIAL_ACTION
                && i32::from_le_bytes(p[1..5].try_into().unwrap()) == 8804
        }),
        "the nearby player got the gesture"
    );
}

/// `//earthquake <intensity> <duration>` broadcasts an Earthquake to the GM.
#[test]
fn admin_earthquake_broadcasts() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8805, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("earthquake 20 10"));
    assert!(
        drain(&mut gm_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::EARTHQUAKE),
        "Earthquake broadcast"
    );
}

/// `//atmosphere sky day` sends `SunRise` to every online player.
#[test]
fn admin_atmosphere_broadcasts_to_all() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8806, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 8807, 0);
    drain(&mut gm_rx);
    drain(&mut other_rx);

    on_packet(&mut world, 1, build_admin("atmosphere sky day 0"));
    assert!(
        drain(&mut other_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SUN_RISE),
        "SunRise reached an unrelated online player"
    );
}

/// `//play_sound <name>` plays the sound and confirms to the GM.
#[test]
fn admin_play_sound_plays() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8808, 100);
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        build_admin("play_sound ItemSound.quest_middle"),
    );
    let pkts = drain(&mut gm_rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::PLAY_SOUND),
        "PlaySound sent"
    );
    assert!(count_system_messages(&pkts) >= 1, "confirmation line");
}

/// `//effect <skill>` broadcasts a cosmetic `MagicSkillUse` (self animation).
#[test]
fn admin_effect_broadcasts_msu() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8809, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("effect 1177 1 1"));
    let pkts = drain(&mut gm_rx);
    let msu = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE)
        .expect("MagicSkillUse broadcast");
    // caster object id is at [5..9] (after the leading casting-bar int at [1..5]).
    assert_eq!(
        i32::from_le_bytes(msu[5..9].try_into().unwrap()),
        8809,
        "GM is the animation source"
    );
}

/// `//remove_exp_sp <exp> <sp>` subtracts from the targeted player (the GM
/// targets itself, matching Java's required player target).
#[test]
fn admin_remove_exp_sp_reduces() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8901, 100);
    world
        .objects
        .add_components(&8901, crate::model::components::TargetRef(Some(8901)));
    drain(&mut gm_rx);
    if let Some(p) = world.objects.get_component_mut::<Player>(&8901) {
        p.exp = 1000;
        p.sp = 500;
    }
    on_packet(&mut world, 1, build_admin("remove_exp_sp 400 200"));
    let p = world.objects.get_component::<Player>(&8901).unwrap();
    assert_eq!(p.exp, 600, "exp reduced");
    assert_eq!(p.sp, 300, "sp reduced");
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

/// `//changename` renames the targeted player; a collision with an online name
/// is rejected.
#[test]
fn admin_changename_renames_target() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8903, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 8904, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);
    world
        .objects
        .add_components(&8903, crate::model::components::TargetRef(Some(8904)));
    on_packet(&mut world, 1, build_admin("changename Renamed"));
    assert_eq!(
        world.objects.get_component::<Player>(&8904).unwrap().name,
        "Renamed"
    );
}

/// `//kick_non_gm` disconnects every non-GM but leaves the GM connected.
#[test]
fn admin_kick_non_gm_disconnects_players() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8905, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 8906, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);
    on_packet(&mut world, 1, build_admin("kick_non_gm"));
    assert!(!world.clients.contains_key(&2), "non-GM disconnected");
    assert!(world.clients.contains_key(&1), "GM stays connected");
}

/// `//set_vitality <n>` sets the targeted player's vitality points (clamped).
#[test]
fn admin_set_vitality_sets_points() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8907, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 8908, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);
    world
        .objects
        .add_components(&8907, crate::model::components::TargetRef(Some(8908)));
    on_packet(&mut world, 1, build_admin("set_vitality 5000"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8908)
            .unwrap()
            .vitality_points,
        5000
    );
    on_packet(&mut world, 1, build_admin("full_vitality"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8908)
            .unwrap()
            .vitality_points,
        140_000,
        "clamped to max"
    );
}

/// `//gonorth <offset>` moves the GM north (-y) by the offset.
#[test]
fn admin_gonorth_moves_gm() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8909, 100);
    drain(&mut gm_rx);
    let y0 = world.objects.get_component::<Position>(&8909).unwrap().y;
    on_packet(&mut world, 1, build_admin("gonorth 200"));
    assert_eq!(
        world.objects.get_component::<Position>(&8909).unwrap().y,
        y0 - 200
    );
}

/// The "Additional Movement Options" button on `teleports.htm` fires
/// `bypass -h admin_tele`, which Java answers with `showTeleportWindow` →
/// `html/admin/move.htm`: the nudge pad, the click-to-move mode row, the GM
/// speed row and the tele/walk box. The port used to alias `admin_tele` onto
/// the *coordinate* teleport, so the button answered "Usage: //teleport <x> <y>
/// <z>" and the window never opened.
#[test]
fn admin_tele_opens_the_additional_movement_options_window() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8920, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("tele"));
    let out = drain(&mut gm_rx);
    let html = last_admin_html(&out).expect("a page came back");
    assert!(
        html.contains("Teleport Menu") && html.contains("admin_instant_move"),
        "move.htm, not teleports.htm: {html}"
    );
    // The old aliasing answered with the usage line instead of a page.
    assert_eq!(
        count_system_messages(&out),
        0,
        "the button opens a window, it does not complain about coordinates"
    );
}

/// The "Move:" row of `move.htm` arms `Player.setTeleMode(...)`; the click that
/// follows is consumed by `MoveBackwardToLocation`. Each of the three armed
/// modes announces itself with Java's exact line; "Normal mode" (`//teleto
/// end`) disarms silently.
#[test]
fn teleto_mode_words_arm_the_click_to_move_latch() {
    use crate::enums::AdminTeleportType;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8921, 100);
    drain(&mut gm_rx);
    let mode = |w: &World| w.objects.get_component::<Player>(&8921).unwrap().tele_mode;

    assert_eq!(mode(&world), AdminTeleportType::Normal, "off by default");

    on_packet(&mut world, 1, build_admin("instant_move"));
    assert_eq!(mode(&world), AdminTeleportType::Demonic);
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "ready line");

    on_packet(&mut world, 1, build_admin("teleto sayune"));
    assert_eq!(mode(&world), AdminTeleportType::Sayune);
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1);

    on_packet(&mut world, 1, build_admin("teleto charge"));
    assert_eq!(mode(&world), AdminTeleportType::Charge);
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1);

    on_packet(&mut world, 1, build_admin("teleto end"));
    assert_eq!(mode(&world), AdminTeleportType::Normal, "disarmed");
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        0,
        "Java's `admin_teleto end` arm sends no line"
    );
}

/// A bare `//teleto` keeps its teleport-to-target meaning — only the three mode
/// words are latches, so the alias the char-management pages use is not
/// swallowed by the new arm.
#[test]
fn bare_teleto_still_teleports_to_the_target() {
    use crate::enums::AdminTeleportType;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8922, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 8923, 0);
    drain(&mut gm_rx);
    drain(&mut other_rx);
    set_position(&mut world, 8923, (2500, 2600, 2700));
    world
        .objects
        .add_components(&8922, crate::model::components::TargetRef(Some(8923)));

    on_packet(&mut world, 1, build_admin("teleto"));
    let pos = *world.objects.get_component::<Position>(&8922).unwrap();
    assert_eq!((pos.x, pos.y), (2500, 2600), "teleported to the target");
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8922)
            .unwrap()
            .tele_mode,
        AdminTeleportType::Normal,
        "no latch armed"
    );
}

/// `//walk <x> <y> <z>` — the "Walk" button beside "Tele" on `move.htm`. Java
/// sets `AI_INTENTION_MOVE_TO`, so the GM *walks* there; the port used to alias
/// it onto the coordinate teleport, which made the two buttons identical.
#[test]
fn admin_walk_walks_instead_of_teleporting() {
    use crate::model::components::{Movement, Speeds};
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8924, 100);
    {
        let speeds = world.objects.get_component_mut::<Speeds>(&8924).unwrap();
        speeds.run_spd = 120.0;
        speeds.running = true;
    }
    set_position(&mut world, 8924, (1000, 1000, 0));
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("walk 1300 1000 0"));

    assert!(
        world.objects.has_component::<Movement>(&8924),
        "a walk is in flight"
    );
    assert_eq!(
        world.objects.get_component::<Position>(&8924).unwrap().x,
        1000,
        "still at the start — it walks there, it does not jump"
    );
    let out = drain(&mut gm_rx);
    assert!(
        !out.iter()
            .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION),
        "no teleport"
    );
    assert!(
        out.iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_LOCATION),
        "a MoveToLocation instead"
    );
}

/// `//geo_pos` with no geodata loaded answers the "no geodata" line (does not
/// crash on the empty geo engine).
#[test]
fn admin_geo_pos_no_geodata() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8910, 100);
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin("geo_pos"));
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "one geo status line"
    );
}

/// `//create_coin adena <n>` gives adena (item 57) to the GM.
#[test]
fn admin_create_coin_gives_adena() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut gm_rx = ingame_player_access(&mut world, 1, 8911, 100);
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin("create_coin adena 100"));
    let inv = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&8911)
        .unwrap();
    assert_eq!(inv.count_of(57), 100, "adena added");
}

/// `//spawnat <id> <x> <y> <z>` spawns an NPC at explicit coordinates.
#[test]
fn admin_spawnat_creates_npc_at_coords() {
    let (mut world, ..) = admin_world();
    world.data.npc_data = dist::npcs_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8912, 100);
    drain(&mut gm_rx);
    let npc_oid = world.next_npc_object_id;
    on_packet(
        &mut world,
        1,
        build_admin("spawnat 30001 -84000 244000 -3700"),
    );
    assert_eq!(world.next_npc_object_id, npc_oid + 1, "one NPC spawned");
    let pos = world
        .objects
        .get_component::<crate::model::components::Position>(&npc_oid)
        .unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (-84000, 244000, -3700),
        "spawned at the coords"
    );
}

/// `//ride_strider` mounts the GM (durable `mount_type`/`mount_npc_id` + a Ride
/// broadcast); `//unride` clears it.
#[test]
fn admin_ride_and_unride() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8920, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("ride_strider"));
    let p = world.objects.get_component::<Player>(&8920).unwrap();
    assert_eq!(p.mount_type, 1, "strider = MountType 1");
    assert_eq!(p.mount_npc_id, 12526, "strider npc id");
    assert!(
        drain(&mut gm_rx)
            .iter()
            .any(|pk| pk[0] == server_packets::opcodes::RIDE),
        "Ride broadcast sent"
    );

    // Re-riding while mounted is refused (Java "already have a summon").
    on_packet(&mut world, 1, build_admin("ride_wolf"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8920)
            .unwrap()
            .mount_type,
        1,
        "still on the strider"
    );

    on_packet(&mut world, 1, build_admin("unride"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8920)
            .unwrap()
            .mount_type,
        0,
        "dismounted"
    );
}

/// Java `AdminRide`'s `isMounted() || hasSummon()` gate runs before *every*
/// `//ride_*` branch — including the transform-based `//ride_horse` — and
/// `AdminTransform` refuses a mounted target with SM 2063: a strider rider
/// can't stack a horse or a polymorph on top of the mount.
#[test]
fn admin_mounted_blocks_horse_and_transform() {
    let (mut world, ..) = admin_world();
    world.data.transforms = crate::data::TransformData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8925, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("ride_strider"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8925)
            .unwrap()
            .mount_type,
        1,
        "on the strider"
    );

    on_packet(&mut world, 1, build_admin("ride_horse"));
    {
        let p = world.objects.get_component::<Player>(&8925).unwrap();
        assert_eq!(p.transform_id, 0, "horse refused while mounted");
        assert_eq!(p.mount_type, 1, "still on the strider");
    }

    on_packet(&mut world, 1, build_admin("transform 106"));
    let p = world.objects.get_component::<Player>(&8925).unwrap();
    assert_eq!(p.transform_id, 0, "//transform refused while mounted");
    assert_eq!(p.mount_type, 1, "mount untouched");
}

/// `//transform` refuses **in water** (Java `player.isInWater()` → SM 2060).
///
/// The gate had no reader until `position::is_in_water` landed with the
/// water/swim work; the marker outlived its own blocker.
#[test]
fn admin_transform_refused_in_water() {
    let (mut world, ..) = admin_world();
    world.data.transforms = crate::data::TransformData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    let mut rx = ingame_player_access(&mut world, 1, 8925, 100);
    drain(&mut rx);

    world.cfg.general.allow_water = true;
    // A water zone over the GM, then the revalidation Java runs on movement —
    // `checkWaterState` is what starts the drowning task, and that task (not
    // the zone) is what `Player.isInWater()` reports.
    insert_zone(
        &mut world,
        crate::data::zone_data::ZoneKind::Water,
        -1000,
        1000,
        -1000,
        1000,
    );
    // Go through `revalidate_zone`, not `check_water_state` directly: since
    // the hot-paths work the latter reads the cached `ZoneFlags` mask that
    // revalidation writes, rather than walking the zone grid itself.
    crate::game_loop::zones::revalidate_zone(&mut world, 8925, true);
    assert!(
        crate::game_loop::water::is_drowning_task_active(&world, 8925),
        "fixture must actually be drowning for this to mean anything"
    );
    drain(&mut rx);

    on_packet(&mut world, 1, build_admin("transform 106"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8925)
            .unwrap()
            .transform_id,
        0,
        "//transform refused in water"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &crate::network::server_packets::sm_ids::YOU_CANNOT_POLYMORPH_INTO_THE_DESIRED_FORM_IN_WATER
        ),
        "and says why"
    );
}

/// `//ride_bike` transforms the GM (transform 20001): durable transform id +
/// display id, the run speed overridden to the template's, and the transform's
/// skills granted; `//unride` reverts all of it.
#[test]
fn admin_ride_bike_transforms_and_reverts() {
    let (mut world, ..) = admin_world();
    world.data.transforms = crate::data::TransformData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.data.skill_data = dist::skills_owned();
    // Jet bike (20001) exists in the dist with run=170 + a Dismount skill.
    let bike = world
        .data
        .transforms
        .get(20001)
        .expect("jet bike transform loaded");
    let bike_run = bike.template(false).run_spd.expect("bike has a run speed");
    let bike_skill = bike
        .template(false)
        .skills
        .first()
        .map(|(id, _)| *id)
        .expect("bike grants a skill");

    let mut gm_rx = ingame_player_access(&mut world, 1, 8930, 100);
    drain(&mut gm_rx);
    let base_run = world
        .objects
        .get_component::<Speeds>(&8930)
        .unwrap()
        .run_spd;

    on_packet(&mut world, 1, build_admin("ride_bike"));
    {
        let p = world.objects.get_component::<Player>(&8930).unwrap();
        assert_eq!(p.transform_id, 20001, "transformed into the bike");
        assert_eq!(
            p.transform_display_id, 20001,
            "display id == id on this dist"
        );
    }
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&8930)
            .unwrap()
            .run_spd,
        bike_run,
        "run speed overridden by the transform"
    );
    assert!(
        world
            .objects
            .get_component::<SkillBook>(&8930)
            .unwrap()
            .0
            .contains_key(&bike_skill),
        "transform skill granted"
    );

    // Re-transforming while transformed is refused (Java polymorph message).
    on_packet(&mut world, 1, build_admin("ride_horse"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8930)
            .unwrap()
            .transform_id,
        20001,
        "still the bike"
    );

    on_packet(&mut world, 1, build_admin("unride"));
    let p = world.objects.get_component::<Player>(&8930).unwrap();
    assert_eq!(p.transform_id, 0, "reverted");
    assert_eq!(p.transform_display_id, 0, "display cleared");
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&8930)
            .unwrap()
            .run_spd,
        base_run,
        "run speed restored"
    );
    assert!(
        !world
            .objects
            .get_component::<SkillBook>(&8930)
            .unwrap()
            .0
            .contains_key(&bike_skill),
        "transform skill removed"
    );
}

/// The transform-granted Dismount skill (839, `DispelBySlot TRANSFORM,-1` in
/// the dist) reverts a GM `//ride_bike` transform even though no buff backs it
/// — Java's `DispelBySlot` dispels "transformations (buff and by GM)" via
/// `stopTransformation`, and that skill is the only in-client revert path for
/// a ride transform. Before the fix the dispel only swept the buff list, so
/// clicking "transform back" was a silent no-op and the player stayed a bike.
#[test]
fn dismount_skill_reverts_gm_ride_transform() {
    let (mut world, ..) = admin_world();
    world.data.transforms = crate::data::TransformData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.data.skill_data = dist::skills_owned();

    let mut gm_rx = ingame_player_access(&mut world, 1, 8935, 100);
    drain(&mut gm_rx);
    let base_run = world
        .objects
        .get_component::<Speeds>(&8935)
        .unwrap()
        .run_spd;

    on_packet(&mut world, 1, build_admin("ride_bike"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8935)
            .unwrap()
            .transform_id,
        20001,
        "transformed into the bike"
    );

    // The dist parses 839 into a DispelBySlot with the TRANSFORM,-1 entry.
    let dismount = world
        .data
        .skill_data
        .get(839, 1)
        .expect("Dismount 839 parsed from dist")
        .clone();
    assert!(
        dismount.effects.iter().any(|e| matches!(
            e,
            crate::model::skill::SkillEffect::DispelBySlot { dispel }
                if dispel.iter().any(|(ty, lvl)| ty == "TRANSFORM" && *lvl < 0)
        )),
        "Dismount carries DispelBySlot TRANSFORM,-1, got {:?}",
        dismount.effects
    );

    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 8935, 8935, &dismount);
    let p = world.objects.get_component::<Player>(&8935).unwrap();
    assert_eq!(p.transform_id, 0, "transform dispelled");
    assert_eq!(p.transform_display_id, 0, "display cleared");
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&8935)
            .unwrap()
            .run_spd,
        base_run,
        "run speed restored"
    );
}

/// Transform-granted skills are session-only (Java `_transformSkills`, which
/// `storeSkills` never writes): a flush while transformed must not persist
/// them, and rows a pre-filter flush already leaked into `character_skills`
/// are dropped on restore. Before the fix an autosave during `//ride_bike`
/// wrote Dismount 839 + Dissonance 5437 as learned rows, and 5437's passive
/// (Accuracy -50, P./M. Atk -95%) then followed the character across every
/// relog.
#[test]
fn transform_skills_never_persist() {
    let (mut world, ..) = admin_world();
    world.data.transforms = crate::data::TransformData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.data.skill_data = dist::skills_owned();
    let bike_skills: Vec<i32> = world
        .data
        .transforms
        .get(20001)
        .expect("jet bike transform loaded")
        .template(false)
        .skills
        .iter()
        .map(|&(id, _)| id)
        .collect();
    assert!(!bike_skills.is_empty(), "bike grants skills");

    let mut gm_rx = ingame_player_access(&mut world, 1, 8945, 100);
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin("ride_bike"));
    let book = world.objects.get_component::<SkillBook>(&8945).unwrap();
    for id in &bike_skills {
        assert!(
            book.0.contains_key(id),
            "skill {id} granted while transformed"
        );
    }

    // Flush mid-transform: the snapshot must not carry the transform skills.
    let save = build_save_data(&world, 8945).expect("save data");
    for id in &bike_skills {
        assert!(
            !save.skills.iter().any(|&(sid, _, _)| sid == *id),
            "transform skill {id} must not reach character_skills"
        );
    }

    // Restore with rows a pre-filter flush leaked: they're dropped, learned
    // skills survive.
    let mut chr = dummy_char(8946, "Poisoned");
    chr.skills = vec![(839, 1, 0), (5437, 2, 0), (1177, 1, 0)];
    Player::from_char(&world.data, &chr).spawn_into(&mut world);
    let book = world.objects.get_component::<SkillBook>(&8946).unwrap();
    assert!(
        !book.0.contains_key(&839),
        "stale Dismount dropped on restore"
    );
    assert!(
        !book.0.contains_key(&5437),
        "stale Dissonance dropped on restore"
    );
    assert!(book.0.contains_key(&1177), "learned skill survives restore");
}

/// `//mobgroup` lifecycle: create → spawn (members tagged Controllable) →
/// set a state → invul → kill → remove.
#[test]
fn admin_mobgroup_lifecycle() {
    let (mut world, ..) = admin_world();
    world.data.npc_data = dist::npcs_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8940, 100);
    drain(&mut gm_rx);

    // create (no spawn yet)
    on_packet(&mut world, 1, build_admin("mobgroup_create 1 20001 3"));
    assert_eq!(
        world.mob_groups.get(&1).map(|g| g.max_count),
        Some(3),
        "group registered"
    );
    assert!(
        world.mob_groups.get(&1).unwrap().members.is_empty(),
        "not spawned yet"
    );

    // spawn at the GM → 3 Controllable NPCs
    on_packet(&mut world, 1, build_admin("mobgroup_spawn 1"));
    let members: Vec<i32> = world.mob_groups.get(&1).unwrap().members.clone();
    assert_eq!(members.len(), 3, "three mobs spawned");
    for &m in &members {
        assert_eq!(
            world
                .objects
                .get_component::<crate::model::mob_group::Controllable>(&m)
                .map(|c| c.group_id),
            Some(1),
            "member tagged Controllable"
        );
    }

    // state: follow the GM
    on_packet(&mut world, 1, build_admin("mobgroup_follow 1"));
    assert!(matches!(
        world.mob_groups.get(&1).unwrap().state,
        crate::model::mob_group::MobGroupState::Follow(8940)
    ));

    // invul on → each member gets the invul flag
    on_packet(&mut world, 1, build_admin("mobgroup_invul 1 on"));
    assert!(world.mob_groups.get(&1).unwrap().invul, "group invul set");
    assert!(
        world
            .objects
            .get_component::<AdminFlags>(&members[0])
            .is_some_and(|f| f.invul),
        "member invul"
    );

    // kill → members become corpses (dead)
    on_packet(&mut world, 1, build_admin("mobgroup_kill 1"));
    assert!(
        members.iter().all(|m| world
            .objects
            .get_component::<Vitals>(m)
            .is_some_and(|v| v.dead)),
        "all members killed"
    );

    // remove → group gone, members despawned
    on_packet(&mut world, 1, build_admin("mobgroup_remove 1"));
    assert!(!world.mob_groups.contains_key(&1), "group removed");
    assert!(
        members
            .iter()
            .all(|m| !world.objects.has_component::<crate::model::npc::Npc>(m)),
        "members despawned"
    );
}

/// `//setparam pAtk <v>` fixes the target's P.Atk to `v` (Java `addFixedValue`);
/// `//unsetparam pAtk` restores the computed value.
#[test]
fn admin_setparam_fixes_and_clears_a_stat() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8950, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 8951, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);
    world
        .objects
        .add_components(&8950, crate::model::components::TargetRef(Some(8951)));
    let base_p_atk = world
        .objects
        .get_component::<CombatStats>(&8951)
        .unwrap()
        .p_atk;

    on_packet(&mut world, 1, build_admin("setparam pAtk 9999"));
    assert_eq!(
        world
            .objects
            .get_component::<CombatStats>(&8951)
            .unwrap()
            .p_atk,
        9999.0,
        "P.Atk fixed"
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::StatModifiers>(&8951)
            .unwrap()
            .fixed
            .get(&crate::model::stats::Stat::PhysicalAttack),
        Some(&9999.0)
    );

    on_packet(&mut world, 1, build_admin("unsetparam pAtk"));
    assert_eq!(
        world
            .objects
            .get_component::<CombatStats>(&8951)
            .unwrap()
            .p_atk,
        base_p_atk,
        "P.Atk restored"
    );

    // An unknown stat name is rejected without touching the overrides.
    on_packet(&mut world, 1, build_admin("setparam bogus 5"));
    assert!(
        world
            .objects
            .get_component::<crate::model::components::StatModifiers>(&8951)
            .unwrap()
            .fixed
            .is_empty()
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

/// `//castlemanage` shows a castle's page; `setOwner` assigns the targeted
/// clanned player's clan + side, `switchSide` flips it, `takeCastle` strips it;
/// siege actions report unavailable. Port of AdminCastle.
#[test]
fn admin_castlemanage_ownership_and_side() {
    use crate::model::castle::{Castle, CastleSide};
    use crate::model::clan::{Clan, ClanMember};
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, _db_tx, mut db_rx, _link) = admin_world();
    world.data.root = ROOT.to_string();
    world.castles = vec![Castle {
        show_npc_crest: false,
        id: 3,
        name: "Giran".into(),
        side: CastleSide::Neutral,
        ticket_buy_count: 0,
        first_mid_victory: false,
        time_registration_over: true,
        siege_time_registration_end: 0,
        siege_date: 0,
        treasury: 0,
    }];
    world.clans.insert(
        500,
        Clan {
            id: 500,
            name: "Owners".into(),
            leader_id: 8002,
            level: 5,
            reputation_score: 0,
            castle_id: 0,
            members: vec![ClanMember {
                char_id: 8002,
                name: "P8002".into(),
                level: 40,
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
    let mut rx = ingame_player_access(&mut world, 1, 8001, 100);
    let _t = ingame_player_access(&mut world, 2, 8002, 0);
    world
        .objects
        .get_component_mut::<Player>(&8002)
        .unwrap()
        .clan_id = 500;
    world.objects.add_components(&8001, TargetRef(Some(8002)));
    drain(&mut rx);

    // //castlemanage 3 → the page, unowned + neutral.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("castlemanage 3"),
        ]
        .concat(),
    );
    let page = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("castle page");
    assert!(
        page.contains("Giran") && page.contains("NPC"),
        "unowned castle shows NPC"
    );

    // //castlemanage 3 setOwner LIGHT → clan 500 owns Giran on the light side.
    drain_db(&mut db_rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("castlemanage 3 setOwner LIGHT"),
        ]
        .concat(),
    );
    let page = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("castle page");
    assert_eq!(world.castles[0].side, CastleSide::Light, "side set");
    assert_eq!(world.clans[&500].castle_id, 3, "clan owns the castle");
    assert!(
        page.contains("Owners") && page.contains("Light"),
        "owner + side displayed"
    );
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            db::DbCommand::UpdateClanCastle {
                clan_id: 500,
                castle_id: 3
            }
        )),
        "persisted"
    );

    // //castlemanage 3 switchSide → Dark.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("castlemanage 3 switchSide"),
        ]
        .concat(),
    );
    drain(&mut rx);
    assert_eq!(world.castles[0].side, CastleSide::Dark, "side switched");

    // //castlemanage 3 takeCastle → unowned + reverted to NEUTRAL (Java removeOwner).
    drain_db(&mut db_rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("castlemanage 3 takeCastle"),
        ]
        .concat(),
    );
    drain(&mut rx);
    assert_eq!(world.clans[&500].castle_id, 0, "ownership removed");
    assert_eq!(
        world.castles[0].side,
        CastleSide::Neutral,
        "side reverts to neutral on take"
    );
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::UpdateCastleSide { castle_id: 3, side } if side == "NEUTRAL")), "neutral side persisted");

    // //castlemanage 3 startSiege → no attackers registered.
    world.sieges.insert(3, crate::model::siege::Siege::new(3));
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("castlemanage 3 startSiege"),
        ]
        .concat(),
    );
    let msgs: Vec<String> = drain(&mut rx)
        .iter()
        .filter_map(|p| sysmsg_text(p))
        .collect();
    assert!(
        msgs.iter().any(|t| t.contains("not registered any clan")),
        "siege needs an attacker"
    );
}

/// The `//castlemanage <id>` siege actions: register/remove attackers &
/// defenders (`siege_clans`), and the start/stop state transition. Port of
/// AdminCastle's siege branch over the model/siege slice.
#[test]
fn admin_castlemanage_siege_registration_and_state() {
    use crate::model::castle::{Castle, CastleSide};
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::siege::Siege;
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, _db_tx, mut db_rx, _link) = admin_world();
    world.data.root = ROOT.to_string();
    world.castles = vec![Castle {
        show_npc_crest: false,
        id: 3,
        name: "Giran".into(),
        side: CastleSide::Neutral,
        ticket_buy_count: 0,
        first_mid_victory: false,
        time_registration_over: true,
        siege_time_registration_end: 0,
        siege_date: 0,
        treasury: 0,
    }];
    world.sieges.insert(3, Siege::new(3));
    world.clans.insert(
        700,
        Clan {
            id: 700,
            name: "Besiegers".into(),
            leader_id: 8102,
            level: 5,
            reputation_score: 0,
            castle_id: 0,
            members: vec![ClanMember {
                char_id: 8102,
                name: "P8102".into(),
                level: 40,
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
    let mut rx = ingame_player_access(&mut world, 1, 8101, 100);
    let _t = ingame_player_access(&mut world, 2, 8102, 0);
    world
        .objects
        .get_component_mut::<Player>(&8102)
        .unwrap()
        .clan_id = 700;
    world.objects.add_components(&8101, TargetRef(Some(8102)));
    drain(&mut rx);

    // addAttacker → clan 700 registered + persisted.
    drain_db(&mut db_rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("castlemanage 3 addAttacker"),
        ]
        .concat(),
    );
    assert!(
        world.sieges[&3].has_attackers() && world.sieges[&3].is_registered(700),
        "attacker registered"
    );
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            db::DbCommand::SaveSiegeClan {
                castle_id: 3,
                clan_id: 700,
                kind: 1
            }
        )),
        "persisted attacker"
    );

    // addAttacker again → already requested.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("castlemanage 3 addAttacker"),
        ]
        .concat(),
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_HAVE_ALREADY_REQUESTED_A_CASTLE_SIEGE),
        "duplicate registration refused"
    );

    // startSiege → in progress + "siege has started" announced to everyone.
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("castlemanage 3 startSiege"),
        ]
        .concat(),
    );
    assert!(world.sieges[&3].in_progress, "siege started");
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_S1_SIEGE_HAS_STARTED),
        "start announced"
    );
    // stopSiege → ended + "siege has finished".
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("castlemanage 3 stopSiege"),
        ]
        .concat(),
    );
    assert!(!world.sieges[&3].in_progress, "siege stopped");
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_S1_SIEGE_HAS_FINISHED),
        "end announced"
    );

    // Re-start, then let the scheduled auto-end fire (Siege.ScheduleEndSiegeTask).
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("castlemanage 3 startSiege"),
        ]
        .concat(),
    );
    assert!(world.sieges[&3].in_progress);
    drain(&mut rx);
    world.tick += 120 * 60 * 10 + 1; // past the 120-minute window (100 ms ticks)
    apply_due_tasks(&mut world);
    assert!(
        !world.sieges[&3].in_progress,
        "auto-ended after the siege window"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_S1_SIEGE_HAS_FINISHED),
        "auto-end announced"
    );

    // removeDeffender strips the target's clan (Java quirk) + persists.
    drain(&mut rx);
    drain_db(&mut db_rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("castlemanage 3 removeDeffender"),
        ]
        .concat(),
    );
    assert!(
        !world.sieges[&3].is_registered(700),
        "clan removed from the siege"
    );
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            db::DbCommand::RemoveSiegeClan {
                castle_id: 3,
                clan_id: 700
            }
        )),
        "persisted removal"
    );
}

/// `//give_clan_skills` end-to-end through the admin dispatch: a GM (access 100)
/// targeting a clan leader grants the clan its pledge skills, applies them, and
/// persists them (Java `AdminSkill.adminGiveClanSkills`).
#[test]
fn admin_give_clan_skills_command_grants_targeted_clan() {
    use crate::data::pledge_skill_tree::PledgeSkillLearn;
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::components::{ClanSkills, TargetRef};

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

/// `//ave_abnormal <NAME>` toggles a GM-pinned abnormal visual on the target
/// (self when untargeted), and rejects an unknown effect name. The pinned set
/// is folded alongside buff-derived visuals by `abnormal::visual_effects`.
#[test]
fn admin_ave_abnormal_toggles_a_pinned_visual() {
    use crate::game_loop::abnormal::visual_effects;

    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6481, 100);
    drain(&mut rx);

    assert!(
        visual_effects(&world, 6481).is_empty(),
        "nothing pinned to begin with"
    );

    // BIG_HEAD is client id 14.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("ave_abnormal BIG_HEAD"),
        ]
        .concat(),
    );
    assert!(visual_effects(&world, 6481).contains(&14), "pinned on");
    drain(&mut rx);

    // Toggling the same name removes it.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("ave_abnormal BIG_HEAD"),
        ]
        .concat(),
    );
    assert!(!visual_effects(&world, 6481).contains(&14), "pinned off");
    drain(&mut rx);

    // An unknown name changes nothing.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("ave_abnormal NOT_REAL"),
        ]
        .concat(),
    );
    assert!(
        visual_effects(&world, 6481).is_empty(),
        "an unknown effect name is rejected"
    );
}

// ---------------------------------------------------------------------------
// AdminEffects' G19 tail: //setteam, //para, //settargetable, //event_trigger,
// //playmovie, //bighead.
// ---------------------------------------------------------------------------

/// `//setteam blue` colors the aura (self when untargeted); `//setteam none`
/// clears it; a bad color is refused with usage text.
#[test]
fn admin_setteam_sets_the_aura_color() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6491, 100);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("setteam blue"),
        ]
        .concat(),
    );
    assert_eq!(
        world.objects.get_component::<Player>(&6491).unwrap().team,
        1
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("setteam red"),
        ]
        .concat(),
    );
    assert_eq!(
        world.objects.get_component::<Player>(&6491).unwrap().team,
        2
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("setteam purple"),
        ]
        .concat(),
    );
    assert_eq!(
        world.objects.get_component::<Player>(&6491).unwrap().team,
        2,
        "bad color leaves it"
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("setteam none"),
        ]
        .concat(),
    );
    assert_eq!(
        world.objects.get_component::<Player>(&6491).unwrap().team,
        0
    );
}

/// `//para` freezes the target — the block-actions and movement gates both
/// hold, and the PARALYZE visual (11) is pinned — and `//unpara` releases.
#[test]
fn admin_para_blocks_actions_until_unpara() {
    use crate::game_loop::abnormal::{
        is_blocked_from_actions, is_movement_disabled, visual_effects,
    };

    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6492, 100);
    drain(&mut rx);

    assert!(!is_blocked_from_actions(&world, 6492));
    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("para")].concat(),
    );
    assert!(
        is_blocked_from_actions(&world, 6492),
        "GM paralysis blocks actions"
    );
    assert!(is_movement_disabled(&world, 6492), "and movement");
    assert!(
        visual_effects(&world, 6492).contains(&11),
        "PARALYZE visual pinned"
    );

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("unpara")].concat(),
    );
    assert!(!is_blocked_from_actions(&world, 6492), "released");
    assert!(
        !visual_effects(&world, 6492).contains(&11),
        "visual unpinned"
    );
}

/// `//settargetable` makes the GM unselectable: another player's click no
/// longer sets their target; toggling back restores it.
#[test]
fn admin_settargetable_blocks_selection() {
    use crate::model::components::TargetRef;

    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6493, 100);
    let mut rx2 = ingame_player_access(&mut world, 2, 6494, 0);
    drain(&mut rx);
    drain(&mut rx2);

    let click_gm = {
        let mut w = PacketWriter::new();
        w.write_i32(6493);
        w.write_i32(0);
        w.write_i32(0);
        w.write_i32(0);
        w.write_u8(0);
        w.into_bytes()
    };
    crate::game_loop::target::handle_action(&mut world, 2, &click_gm);
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&6494)
            .copied()
            .unwrap_or_default()
            .0,
        Some(6493),
        "targetable by default"
    );
    crate::game_loop::target::set_target(&mut world, 2, 6494, None);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("settargetable"),
        ]
        .concat(),
    );
    crate::game_loop::target::handle_action(&mut world, 2, &click_gm);
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&6494)
            .copied()
            .unwrap_or_default()
            .0,
        None,
        "untargetable GM can't be selected"
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("settargetable"),
        ]
        .concat(),
    );
    crate::game_loop::target::handle_action(&mut world, 2, &click_gm);
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&6494)
            .copied()
            .unwrap_or_default()
            .0,
        Some(6493),
        "toggled back"
    );
}

/// `//event_trigger` fans the 0xCF packet out (self included); `//playmovie`
/// sends the cinematic starter to the GM.
#[test]
fn admin_event_trigger_and_playmovie_send_their_packets() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6495, 100);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("event_trigger 21170110 true"),
        ]
        .concat(),
    );
    let got = drain(&mut rx);
    assert!(
        got.iter()
            .any(|p| p.first() == Some(&0xCF) && p[1..5] == 21170110i32.to_le_bytes() && p[5] == 1),
        "OnEventTrigger 0xCF with the id and enabled byte"
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("playmovie 101"),
        ]
        .concat(),
    );
    let got = drain(&mut rx);
    assert!(
        got.iter().any(|p| p.first() == Some(&0xFE)
            && p[1..3] == 0x9Au16.to_le_bytes()
            && p[3..7] == 101i32.to_le_bytes()),
        "ExStartScenePlayer with the movie id"
    );
}

/// `//announce_screen <msg>` puts the text on every player's screen as an
/// `ExShowScreenMessage` (top-centre, free text); `//announce_crit` stays a
/// plain system-message line, not a banner.
#[test]
fn admin_announce_screen_broadcasts_a_banner() {
    let (mut world, ..) = admin_world();
    let mut gm = ingame_player_access(&mut world, 1, 7601, 100);
    let mut user = ingame_player_access(&mut world, 2, 7602, 0);
    drain(&mut gm);
    drain(&mut user);

    /// Decode `ExShowScreenMessage`: the 11-int field block, then the text.
    fn decode_screen(pkt: &[u8]) -> Option<(i32, i32, i32, String)> {
        if pkt[0] != server_packets::opcodes::EX
            || i16::from_le_bytes([pkt[1], pkt[2]])
                != server_packets::opcodes::EX_SHOW_SCREEN_MESSAGE
        {
            return None;
        }
        let mut r = commons::network::PacketReader::new(&pkt[3..]);
        let msg_type = r.read_i32()?;
        r.read_i32()?; // sysMessageId
        let position = r.read_i32()?;
        for _ in 0..7 {
            r.read_i32()?; // unk1, size, unk2, unk3, effect, time, fade
        }
        let npc_string = r.read_i32()?;
        Some((msg_type, position, npc_string, r.read_string()?))
    }

    on_packet(&mut world, 1, build_admin("announce_screen hello world"));
    let (msg_type, position, npc_string, text) = drain(&mut user)
        .iter()
        .find_map(|p| decode_screen(p))
        .expect("screen message");
    assert_eq!(text, "hello world", "banner text broadcast");
    assert_eq!(msg_type, 2, "the (text, time) constructor's type");
    assert_eq!(position, 2, "TOP_CENTER");
    assert_eq!(npc_string, -1, "free text, no NpcString");

    // //announce_crit is the ordinary text line, not a screen banner.
    drain(&mut user);
    on_packet(&mut world, 1, build_admin("announce_crit red alert"));
    assert!(
        drain(&mut user).iter().all(|p| decode_screen(p).is_none()),
        "crit does not put a banner on screen"
    );
}

/// `//spawn` with no arguments must not panic (it used to index `args[0]` on an
/// empty token list, killing the game thread) — it answers with the spawns menu
/// and the "doesnt exist" message like an unknown id does.
#[test]
fn admin_spawn_without_args_does_not_panic() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("spawn"));
    let pkts = drain(&mut gm_rx);
    assert!(
        pkts.iter().any(|p| contains_utf16(p, "doesnt exist")),
        "GM is told the (missing) NPC doesnt exist instead of the server dying"
    );
}

// --- `//scan` (AdminScan) ---------------------------------------------------

/// Spawn a scan-target NPC with a real name at an offset from the GM.
fn scan_world() -> (
    World,
    db::CmdTx,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, a, b, c) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    (world, a, b, c)
}

fn scan_npc(world: &mut World, oid: i32, gm: i32, dx: i32, dy: i32, dz: i32) {
    const SCAN_MOB: i32 = 47000;
    if world.data.npc_data.get(SCAN_MOB).is_none() {
        let mut t = crate::data::npc_data::default_template(SCAN_MOB);
        t.type_name = "Monster".into();
        t.name = "Scan Target".into();
        world.data.npc_data.insert_for_test(t);
    }
    let pos = world
        .objects
        .get_component::<crate::model::components::Position>(&gm)
        .copied()
        .unwrap();
    add_test_npc(
        world,
        oid,
        SCAN_MOB,
        "Monster",
        20,
        pos.x + dx,
        pos.y + dy,
        pos.z + dz,
    );
}

fn scan_html(pkts: &[Vec<u8>]) -> String {
    pkts.iter()
        .find_map(|p| decode_npc_html(p))
        .expect("scan html")
}

/// `//scan`'s range is a 3D sphere (Java `getVisibleObjectsInRange` measures
/// `calculateDistance3D`, default radius 1000): an NPC on a floor 2000 z away
/// is horizontally on top of the GM yet out of range — the Tower of Insolence
/// stairs case, where Java returns an empty list while the old Rust port
/// dumped every stacked floor into one client-crashing html.
#[test]
fn scan_range_is_a_3d_sphere() {
    let (mut world, ..) = scan_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    scan_npc(&mut world, NPC_OID, 5001, 300, 0, 0); // 3D 300: in
    scan_npc(&mut world, NPC_OID + 1, 5001, 200, 0, 2000); // 3D ~2010: out
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("scan")].concat(),
    );
    let html = scan_html(&drain(&mut gm_rx));
    assert_eq!(
        html.matches("admin_move_to").count(),
        1,
        "only the same-floor NPC is listed: {html}"
    );
    assert!(html.contains("Scan Target"), "{html}");
    assert!(
        html.contains(&format!("objectId={NPC_OID}")),
        "delete link carries the object id: {html}"
    );
}

/// With nothing in range the list is empty — no rows at all (what the Java
/// version shows on the ToI 13F stairs).
#[test]
fn scan_with_nothing_in_range_is_empty() {
    let (mut world, ..) = scan_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    scan_npc(&mut world, NPC_OID, 5001, 3000, 0, 0); // beyond default 1000
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("scan")].concat(),
    );
    let html = scan_html(&drain(&mut gm_rx));
    assert_eq!(html.matches("admin_move_to").count(), 0, "{html}");
}

/// The list pages at 15 rows (Java `PageBuilder`): 20 NPCs in range render 15
/// rows and a pager on the first page, and the remaining 5 on `page=1`. This
/// (with the radius) is what keeps the dialog under the client's html limit.
#[test]
fn scan_paginates_at_fifteen_rows() {
    let (mut world, ..) = scan_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    for i in 0..20 {
        scan_npc(&mut world, NPC_OID + i, 5001, 100 + i, 0, 0);
    }
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("scan")].concat(),
    );
    let html = scan_html(&drain(&mut gm_rx));
    assert_eq!(html.matches("admin_move_to").count(), 15, "{html}");
    assert!(html.contains("Page: 1/"), "pager rendered: {html}");
    assert!(
        html.contains("admin_scan page=1"),
        "next-page bypass: {html}"
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("scan page=1"),
        ]
        .concat(),
    );
    let html = scan_html(&drain(&mut gm_rx));
    assert_eq!(
        html.matches("admin_move_to").count(),
        5,
        "second page holds the remainder: {html}"
    );
}

/// `AbstractHtmlPacket.setHtml`'s guard, ported to the packet builder: an
/// oversized html is clipped to 17 200 chars instead of crashing the client.
#[test]
fn oversized_html_is_clipped_to_java_limit() {
    let big = "a".repeat(20_000);
    let pkt = server_packets::npc_html_message_item(0, 1, &big);
    let decoded = decode_npc_html(&pkt).expect("html packet");
    assert_eq!(decoded.chars().count(), 17_200);

    let small = "b".repeat(100);
    let pkt = server_packets::npc_html_message_item(0, 1, &small);
    assert_eq!(decode_npc_html(&pkt).unwrap(), small);
}

// ---------------------------------------------------------------------------
// GM invisibility (`admin_invis` family) + the Debug panel
// ---------------------------------------------------------------------------

/// **The gm_menu "Invis" button works end-to-end.** `admin_invis_menu` was
/// undispatched ("not implemented yet"): it must toggle invisibility — the
/// observer's selection drops (TargetUnselected) before the DeleteObject —
/// re-serve `gm_menu.htm`, suppress CharInfo rebroadcasts while hidden (the
/// old `broadcast_user_info` leaked the GM back onto nearby clients), and
/// re-describe the GM on the second press.
#[test]
fn admin_invis_menu_hides_and_reserves_panel() {
    use crate::model::components::{AdminFlags, TargetRef};
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7101, 100);
    let mut obs_rx = ingame_player_access(&mut world, 2, 7102, 0);
    world.objects.add_components(&7102, TargetRef(Some(7101)));
    drain(&mut gm_rx);
    drain(&mut obs_rx);

    on_packet(&mut world, 1, build_admin("invis_menu"));
    assert!(
        world
            .objects
            .get_component::<AdminFlags>(&7101)
            .is_some_and(|f| f.hidden),
        "GM hidden after the Invis button"
    );
    let obs = drain(&mut obs_rx);
    assert!(
        obs.iter()
            .any(|p| p[0] == server_packets::opcodes::TARGET_UNSELECTED),
        "observer's selection dropped"
    );
    assert!(
        obs.iter()
            .any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT),
        "GM removed from the observer's client"
    );
    assert!(
        drain(&mut gm_rx)
            .iter()
            .filter_map(|p| decode_npc_html(p))
            .any(|h| h.contains("admin_invis_menu")),
        "gm_menu.htm re-served to keep the panel up"
    );

    // While hidden, a UserInfo broadcast must not leak CharInfo to others.
    crate::game_loop::party::broadcast_user_info(&mut world, 7101);
    assert!(
        !drain(&mut obs_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::CHAR_INFO),
        "no CharInfo leak to the observer while hidden"
    );

    on_packet(&mut world, 1, build_admin("invis_menu"));
    assert!(
        !world
            .objects
            .get_component::<AdminFlags>(&7101)
            .unwrap()
            .hidden,
        "second press unhides"
    );
    assert!(
        drain(&mut obs_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::CHAR_INFO),
        "CharInfo re-sent to the observer on unhide"
    );
}

/// **`//vis` sets visible, never toggles.** The old alias collapsed the whole
/// family onto the `//hide` toggle, so `//vis` while visible *hid* you.
/// `//invis` is likewise an idempotent set.
#[test]
fn vis_and_invis_are_sets_not_toggles() {
    use crate::model::components::AdminFlags;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7111, 100);
    drain(&mut gm_rx);
    let hidden = |world: &World| {
        world
            .objects
            .get_component::<AdminFlags>(&7111)
            .is_some_and(|f| f.hidden)
    };

    on_packet(&mut world, 1, build_admin("vis"));
    assert!(!hidden(&world), "//vis while visible stays visible");

    on_packet(&mut world, 1, build_admin("invis"));
    assert!(hidden(&world), "//invis hides");
    on_packet(&mut world, 1, build_admin("invis"));
    assert!(hidden(&world), "//invis is idempotent");

    on_packet(&mut world, 1, build_admin("visible"));
    assert!(!hidden(&world), "//visible unhides");
}

/// **`//setinvis` acts on the *target*, not the GM.** The old alias hid the
/// GM themself.
#[test]
fn setinvis_toggles_the_targeted_player() {
    use crate::model::components::{AdminFlags, TargetRef};
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7121, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7122, 0);
    world.objects.add_components(&7121, TargetRef(Some(7122)));
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    on_packet(&mut world, 1, build_admin("setinvis"));
    assert!(
        world
            .objects
            .get_component::<AdminFlags>(&7122)
            .is_some_and(|f| f.hidden),
        "the targeted player is hidden"
    );
    assert!(
        !world
            .objects
            .get_component::<AdminFlags>(&7121)
            .is_some_and(|f| f.hidden),
        "the GM themself stays visible"
    );
}

/// **Mobs don't notice an invisible GM** (Java `AttackableAI` drops invisible
/// targets; the aggro scan must skip them, with no raid exemption).
#[test]
fn npc_aggro_ignores_hidden_gm() {
    use crate::model::components::AdminFlags;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7131, 100);
    drain(&mut gm_rx);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 10, 100, 0, 0);
    assert!(
        crate::game_loop::ai::notices_target(&world, NPC_OID, 7131),
        "a visible player is noticed"
    );
    let mut flags = world
        .objects
        .get_component::<AdminFlags>(&7131)
        .copied()
        .unwrap_or_default();
    flags.hidden = true;
    world.objects.add_components(&7131, flags);
    assert!(
        !crate::game_loop::ai::notices_target(&world, NPC_OID, 7131),
        "a hidden GM is never noticed"
    );
}

/// **The Debug button opens the real Debug panel.** `admin_debug` used to
/// dump chat text; Java serves `debug.htm` with every `%…_status%` token
/// substituted. The packets toggle round-trips through `World::debug_packets`
/// and re-renders the panel with the flipped label.
#[test]
fn debug_menu_renders_and_packet_toggle_works() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7141, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("debug"));
    let html = drain(&mut gm_rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("the Debug panel is served");
    assert!(html.contains("Debug Menu"), "debug.htm served, got: {html}");
    assert!(
        !html.contains('%'),
        "every %token% substituted, got: {html}"
    );
    assert!(
        html.contains("admin_debug packets on menu"),
        "packets button offers enabling"
    );

    on_packet(&mut world, 1, build_admin("debug packets on menu"));
    assert!(world.debug_packets, "packet debugging enabled");
    let html = drain(&mut gm_rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("panel re-rendered");
    assert!(
        html.contains("admin_debug packets off menu"),
        "packets button now offers disabling"
    );

    on_packet(&mut world, 1, build_admin("debug packets off"));
    assert!(!world.debug_packets, "packet debugging disabled again");
}

// ---------------------------------------------------------------------------
// Category-4 sweep: punishment console, clan leader override, spawn controls
// ---------------------------------------------------------------------------

/// **The AdminPunishment console round-trips.** `//punishment` renders with
/// the type/affect combos filled; `//punishment_add` starts a real punishment
/// through the generic engine (a jail actually confines); `//punishment info`
/// lists it; `//punishment_remove` lifts it.
#[test]
fn punishment_console_add_info_remove() {
    use crate::model::punishment::{PunishmentAffect, PunishmentType};
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7501, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7502, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    on_packet(&mut world, 1, build_admin("punishment"));
    let html = drain(&mut gm_rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("punishment.htm served");
    assert!(
        html.contains("CHAT_BAN") && html.contains("HWID"),
        "type/affect combos substituted, got: {html}"
    );

    let victim_name = world
        .objects
        .get_component::<Player>(&7502)
        .unwrap()
        .name
        .clone();
    on_packet(
        &mut world,
        1,
        build_admin(&format!(
            "punishment_add {victim_name} CHARACTER JAIL 0 testing"
        )),
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
    assert!(
        world
            .punishments
            .has_punishment("7502", PunishmentAffect::Character, PunishmentType::Jail),
        "the jail punishment is registered under the char id"
    );

    on_packet(
        &mut world,
        1,
        build_admin(&format!("punishment info {victim_name} CHARACTER")),
    );
    let html = drain(&mut gm_rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("punishment-info.htm served");
    assert!(html.contains("JAIL"), "the active jail is listed: {html}");

    on_packet(
        &mut world,
        1,
        build_admin("punishment_remove 7502 CHARACTER JAIL"),
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
    assert!(
        !world.punishments.has_punishment(
            "7502",
            PunishmentAffect::Character,
            PunishmentType::Jail
        ),
        "the punishment is lifted"
    );
}

/// **`//clan_changeleader` swaps the leader immediately** — clan record,
/// both players' leader flags/privileges, and the clan-wide SM.
#[test]
fn clan_changeleader_swaps_leader() {
    use crate::model::components::TargetRef;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7601, 100);
    let mut old_rx = ingame_player_access(&mut world, 2, 7602, 0);
    let mut new_rx = ingame_player_access(&mut world, 3, 7603, 0);
    // Clan 600: 7602 leads, 7603 is a member.
    world.clans.insert(
        600,
        crate::model::clan::Clan {
            id: 600,
            name: "Swap".into(),
            leader_id: 7602,
            level: 3,
            reputation_score: 0,
            castle_id: 0,
            members: Vec::new(),
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
    for oid in [7602, 7603] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .unwrap()
            .clan_id = 600;
    }
    world
        .objects
        .get_component_mut::<Player>(&7602)
        .unwrap()
        .clan_leader = true;
    world.objects.add_components(&7601, TargetRef(Some(7603)));
    drain(&mut gm_rx);
    drain(&mut old_rx);
    drain(&mut new_rx);

    // `admin_clan_changeleader` is confirmDlg-gated — answer "yes".
    on_packet(&mut world, 1, build_admin("clan_changeleader"));
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    assert_eq!(
        world.clans.get(&600).unwrap().leader_id,
        7603,
        "clan record"
    );
    assert!(
        world
            .objects
            .get_component::<Player>(&7603)
            .unwrap()
            .clan_leader,
        "new leader flagged"
    );
    assert!(
        !world
            .objects
            .get_component::<Player>(&7602)
            .unwrap()
            .clan_leader,
        "old leader unflagged"
    );
}

/// **`//unspawnall` clears every NPC and `//respawnall` puts the world
/// back** through the boot spawn pass.
#[test]
fn unspawnall_and_respawnall() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7701, 100);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 10, 100, 0, 0);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("unspawnall"));
    assert!(
        !world
            .objects
            .has_component::<crate::model::npc::Npc>(&NPC_OID),
        "all NPCs despawned"
    );

    // The synthetic test world has an empty spawn table — respawnall reports 0.
    on_packet(&mut world, 1, build_admin("respawnall"));
    let msgs = drain(&mut gm_rx);
    assert!(!msgs.is_empty(), "respawnall answers");
}

// ---------------------------------------------------------------------------
// Server control, olympiad manual commands, quest admin
// ---------------------------------------------------------------------------

/// **`//server_shutdown` runs a real countdown** — announce on start, marks
/// while ticking, and the final beat requests the game-thread stop
/// (`shutdown_signal`; a test world's `None` just skips the request).
/// `//server_abort` cancels a pending countdown.
#[test]
fn server_shutdown_countdown_and_abort() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7801, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("server_shutdown 30"));
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    assert!(world.pending_shutdown.is_some(), "countdown armed");
    assert!(
        count_system_messages(&drain(&mut gm_rx)) >= 1,
        "start announcement"
    );

    // Run past the 10s / 5..1s marks and the deadline.
    advance_ticks(&mut world, 320);
    assert!(
        count_system_messages(&drain(&mut gm_rx)) >= 5,
        "mark announcements fired while ticking"
    );

    on_packet(&mut world, 1, build_admin("server_shutdown 60"));
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    on_packet(&mut world, 1, build_admin("server_abort"));
    assert!(
        world.pending_shutdown.is_none(),
        "abort clears the countdown"
    );
}

/// **`//server_gm_only` pushes a `ServerStatus` over the login link.**
#[test]
fn server_gm_only_sends_server_status() {
    use crate::loginlink::LoginLinkCommand;
    let (mut world, _db, _db_rx, mut link_rx) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7811, 100);
    drain(&mut gm_rx);
    while link_rx.try_recv().is_ok() {}

    on_packet(&mut world, 1, build_admin("server_gm_only"));
    let mut got = false;
    while let Ok(cmd) = link_rx.try_recv() {
        if matches!(cmd, LoginLinkCommand::ServerStatus { .. }) {
            got = true;
        }
    }
    assert!(got, "ServerStatus command reached the login link");
}

/// **`//setcharquest` edits a quest state var and `//charquestmenu` lists
/// it**; `state DELETE` removes the state.
#[test]
fn setcharquest_and_menu_roundtrip() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7821, 100);
    let mut p_rx = ingame_player_access(&mut world, 2, 7822, 0);
    let name = world
        .objects
        .get_component::<Player>(&7822)
        .unwrap()
        .name
        .clone();
    drain(&mut gm_rx);
    drain(&mut p_rx);

    on_packet(
        &mut world,
        1,
        build_admin(&format!(
            "setcharquest {name} Q00101_SwordOfSolidarity cond 3"
        )),
    );
    // Java closes setQuestVar with QuestList + ExShowQuestMark on the edited
    // player: the journal must move without a relog.
    let to_target = drain(&mut p_rx);
    assert!(
        to_target
            .iter()
            .any(|p| p[0] == crate::network::server_packets::opcodes::QUEST_LIST),
        "QuestList pushed to the edited player"
    );
    on_packet(
        &mut world,
        1,
        build_admin(&format!(
            "setcharquest {name} Q00101_SwordOfSolidarity state STARTED"
        )),
    );
    {
        let q = world
            .objects
            .get_component::<crate::model::components::Quests>(&7822)
            .unwrap();
        let st = q.0.get("Q00101_SwordOfSolidarity").expect("state created");
        assert_eq!(st.vars.get("cond").map(String::as_str), Some("3"));
        assert_eq!(st.state, crate::model::quest::state::STARTED);
    }
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin(&format!("charquestmenu {name}")));
    let html = drain(&mut gm_rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("quest panel served");
    assert!(
        html.contains("Q00101_SwordOfSolidarity") && html.contains("cond=3"),
        "quest + var listed, got: {html}"
    );

    on_packet(
        &mut world,
        1,
        build_admin(&format!(
            "setcharquest {name} Q00101_SwordOfSolidarity state DELETE"
        )),
    );
    assert!(
        !world
            .objects
            .get_component::<crate::model::components::Quests>(&7822)
            .unwrap()
            .0
            .contains_key("Q00101_SwordOfSolidarity"),
        "state removed"
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
        crate::model::components::Buffs(vec![crate::model::skill::ActiveBuff {
            displayed: true,
            skill_id: 1204, // Wind Walk
            skill_level: 1,
            abnormal_type_client_id: 0,
            abnormal_type: "WIND_WALK".into(),
            abnormal_level: 1,
            slot: crate::model::skill::BuffSlot::Buff,
            expires_at_tick: world.tick + 1000,
            passive: false,
            effect_flags: 0,
            abnormal_visuals: Vec::new(),
            blocked_abnormals: Vec::new(),
            effects: Vec::new(),
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

/// **`//show_quests` is `AdminQuest`'s NPC listing, not `//charquestmenu`.**
/// The two were aliased to the player quest-state editor, so the `Quests`
/// button on the shift-click admin NPC window answered `INVALID_TARGET`
/// instead of listing the scripts registered on the target NPC.
#[test]
fn show_quests_lists_the_target_npcs_scripts() {
    use crate::game_loop::quests;

    struct NpcQuestScript;
    impl quests::QuestScript for NpcQuestScript {
        fn id(&self) -> i32 {
            -42
        }
        fn name(&self) -> &'static str {
            "TestNpcQuest"
        }
        fn html_dir(&self) -> &'static str {
            ""
        }
        fn start_npcs(&self) -> &[i32] {
            &[30001]
        }
        fn talk_npcs(&self) -> &[i32] {
            &[30001]
        }
        fn on_talk(&self, _ctx: &mut quests::QuestCtx) -> Option<String> {
            None
        }
    }

    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    world.quests = std::sync::Arc::new(quests::QuestRegistry::new(vec![std::sync::Arc::new(
        NpcQuestScript,
    )]));
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut gm_rx = ingame_player_access(&mut world, 1, 7830, 100);
    // Select the NPC — Java reads `activeChar.getTarget()`, ignoring the
    // template id the html passes as an argument.
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("show_quests 30001"));

    let html = drain(&mut gm_rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("npc-quests.htm served");
    assert!(
        html.contains("TestNpcQuest") && html.contains("admin_quest_info TestNpcQuest"),
        "the NPC's script is listed and links into //quest_info, got: {html}"
    );
    // The player-menu columns must NOT be what this button opens.
    assert!(
        !html.contains("CREATED") && !html.contains("STARTED"),
        "this is the NPC listing, not the player quest-state editor"
    );
}

// ---------------------------------------------------------------------------
// Tail polish: tradeoff, cond overrides, reload
// ---------------------------------------------------------------------------

/// **`//tradeoff on` refuses incoming trade requests** (Java
/// `getTradeRefusal` in `TradeRequest`).
#[test]
fn tradeoff_refuses_trade_requests() {
    use crate::model::components::TargetRef;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7901, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 7902, 0);
    world.objects.add_components(&7902, TargetRef(Some(7901)));
    drain(&mut gm_rx);
    drain(&mut other_rx);

    on_packet(&mut world, 1, build_admin("tradeoff on"));
    assert!(
        world
            .objects
            .get_component::<Player>(&7901)
            .unwrap()
            .trade_refusal
    );

    // 7902 asks 7901 to trade — refused, no pending request lands.
    let mut body = Vec::new();
    body.extend_from_slice(&7901i32.to_le_bytes());
    crate::game_loop::trade::handle_request(&mut world, 2, &body);
    assert!(
        !world
            .objects
            .has_component::<crate::model::components::PendingTrade>(&7901),
        "no trade request while refusing"
    );
    assert!(
        count_system_messages(&drain(&mut other_rx)) >= 1,
        "requester told about refusal mode"
    );
}

/// **`//exceptions`/`//set_exception` toggle cond-override bits, and
/// SEE_ALL_PLAYERS lets its holder be described a hidden GM.**
#[test]
fn cond_overrides_and_see_all_players() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7911, 100);
    let mut watcher_rx = ingame_player_access(&mut world, 2, 7912, 100);
    drain(&mut gm_rx);
    drain(&mut watcher_rx);

    // GM 7911 hides; watcher 7912 (no override) re-enters — no CharInfo.
    on_packet(&mut world, 1, build_admin("hide"));
    drain(&mut watcher_rx);
    crate::game_loop::visibility::on_enter_world(&world, 2, 7912);
    assert!(
        !drain(&mut watcher_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::CHAR_INFO),
        "hidden GM not described without the override"
    );

    // The watcher enables SEE_ALL_PLAYERS (ordinal 13) — now described.
    on_packet(&mut world, 2, build_admin("set_exception 13"));
    on_packet(
        &mut world,
        2,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    assert!(
        world
            .objects
            .get_component::<Player>(&7912)
            .unwrap()
            .can_override_cond(13),
        "override bit set"
    );
    drain(&mut watcher_rx);
    crate::game_loop::visibility::on_enter_world(&world, 2, 7912);
    assert!(
        drain(&mut watcher_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::CHAR_INFO),
        "SEE_ALL_PLAYERS holder is described the hidden GM"
    );
}

/// **`//reload config` re-reads the ini values from disk.**
#[test]
fn reload_config_rereads_ini() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7921, 100);
    drain(&mut gm_rx);

    world.cfg.feature.allow_ride_wyvern_always = true; // drift from the ini
    on_packet(&mut world, 1, build_admin("reload config"));
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    assert!(
        !world.cfg.feature.allow_ride_wyvern_always,
        "ini value (False) restored by the reload"
    );
}

// ---------------------------------------------------------------------------
// Debug panel drawing toggles
// ---------------------------------------------------------------------------

/// **All four Debug-panel toggles are live.** The geodata toggle draws the
/// NSWE arrow grid as `ExServerPrimitive` (FE:11) packets and redraws after
/// the GM moves; toggling off erases; the panel reflects each state.
#[test]
fn debug_panel_geodata_toggle_draws_grid() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7951, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("debug geodata on menu"));
    let pkts = drain(&mut gm_rx);
    let prim_count = pkts
        .iter()
        .filter(|p| {
            p[0] == 0xFE && p.len() > 2 && i16::from_le_bytes(p[1..3].try_into().unwrap()) == 0x11
        })
        .count();
    assert!(
        prim_count >= 42,
        "41×41 cells / 40 per packet → 43 ExServerPrimitive frames, got {prim_count}"
    );
    assert!(
        pkts.iter()
            .filter_map(|p| decode_npc_html(p))
            .any(|h| h.contains("geodata off")),
        "panel shows the toggle as Disable"
    );
    assert!(
        crate::game_loop::admin::debug_draw::flags(&world, 7951).1,
        "geo flag set"
    );

    // Moving > 15 units redraws on the next beat (15 ticks).
    world
        .objects
        .get_component_mut::<crate::model::components::Position>(&7951)
        .unwrap()
        .x += 100;
    drain(&mut gm_rx);
    advance_ticks(&mut world, 16);
    assert!(
        drain(&mut gm_rx).iter().any(|p| p[0] == 0xFE),
        "grid redrawn after moving"
    );

    on_packet(&mut world, 1, build_admin("debug geodata off"));
    assert!(
        !crate::game_loop::admin::debug_draw::flags(&world, 7951).1,
        "geo flag cleared"
    );
    assert!(
        drain(&mut gm_rx).iter().filter(|p| p[0] == 0xFE).count() >= 42,
        "erase frames sent for every grid packet"
    );
}

/// **`//geogrid` draws the NSWE grid, `//geogrid off` erases it.** Java's
/// `AdminGeodata.admin_geogrid` is one-shot (`GeoUtils.debugGrid` /
/// `hideDebugGrid`): it arms no redraw beat and leaves the Debug panel's
/// `geodata` flag untouched. Draw frames carry 40 cells × 16 arrow lines;
/// the erase frames carry one zero-length line, so the two are told apart by
/// packet size, not just by count.
#[test]
fn admin_geogrid_draws_and_erases_grid() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7955, 100);
    drain(&mut gm_rx);

    let prims = |pkts: &[Vec<u8>]| -> Vec<usize> {
        pkts.iter()
            .filter(|p| {
                p[0] == 0xFE
                    && p.len() > 2
                    && i16::from_le_bytes(p[1..3].try_into().unwrap()) == 0x11
            })
            .map(|p| p.len())
            .collect()
    };

    on_packet(&mut world, 1, build_admin("geogrid"));
    let drawn = prims(&drain(&mut gm_rx));
    assert!(
        drawn.len() >= 42,
        "41×41 cells / 40 per packet → 43 ExServerPrimitive frames, got {}",
        drawn.len()
    );
    assert!(
        drawn.iter().take(drawn.len() - 1).all(|&n| n > 1000),
        "full grid frames carry 640 arrow lines: {drawn:?}"
    );
    assert!(
        !crate::game_loop::admin::debug_draw::flags(&world, 7955).1,
        "one-shot draw must not set the Debug panel's geodata flag"
    );

    // No redraw loop: moving and letting the geo beat (15 ticks) pass is quiet.
    world
        .objects
        .get_component_mut::<crate::model::components::Position>(&7955)
        .unwrap()
        .x += 500;
    advance_ticks(&mut world, 20);
    assert!(
        prims(&drain(&mut gm_rx)).is_empty(),
        "//geogrid arms no redraw task (Java's is one-shot)"
    );

    on_packet(&mut world, 1, build_admin("geogrid off"));
    let erased = prims(&drain(&mut gm_rx));
    assert!(
        erased.len() >= 42,
        "erase frame per grid packet, got {}",
        erased.len()
    );
    assert!(
        erased.iter().all(|&n| n < 200),
        "erase frames are a single zero-length black line: {erased:?}"
    );
}

/// **The movement toggle draws the walk line.** Enabling while standing is
/// clean; once the GM walks, the beat sends the green destination line.
#[test]
fn debug_panel_movement_toggle_draws_walk_line() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7961, 100);
    {
        let speeds = world.objects.get_component_mut::<Speeds>(&7961).unwrap();
        speeds.run_spd = 100.0;
        speeds.running = true;
    }
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("debug movement on"));
    drain(&mut gm_rx);

    handle_move_backward_to_location(&mut world, 1, &move_body((500, 0, 0), (0, 0, 0), 1));
    // The test world doesn't interpolate movement — walk the position
    // forward by hand so the beat sees >15 units from its anchor.
    world
        .objects
        .get_component_mut::<crate::model::components::Position>(&7961)
        .unwrap()
        .x += 100;
    advance_ticks(&mut world, 3);
    assert!(
        drain(&mut gm_rx).iter().any(|p| {
            p[0] == 0xFE && p.len() > 2 && i16::from_le_bytes(p[1..3].try_into().unwrap()) == 0x11
        }),
        "movement line drawn while walking"
    );
}

/// **`//ave_abnormal` with no argument opens the effect list.** Java's
/// `AdminEffects` treats a missing (or numeric) first token as "show the menu"
/// and pages `AbnormalVisualEffect.values()` 100 at a time into
/// `data/html/admin/ave_abnormal.htm`; only a non-numeric token toggles an
/// effect. The port only printed a usage line, so the Game panel's "Abnormal
/// Visual Effects" button opened nothing.
#[test]
fn ave_abnormal_without_args_serves_the_paged_effect_list() {
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, ..) = admin_world();
    world.data.root = ROOT.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 7501, 100);
    drain(&mut rx);

    on_packet(&mut world, 1, build_admin("ave_abnormal"));
    let page0 = drain(&mut rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("the menu html is served");
    assert!(
        page0.contains("Abnormal Visual Effects"),
        "ave_abnormal.htm was loaded"
    );
    assert!(
        page0.contains("bypass admin_ave_abnormal STUN") && page0.contains("STUN(7)"),
        "each effect is a button that re-enters the command by name"
    );
    assert!(
        !page0.contains("bypass admin_ave_abnormal YOGI\""),
        "YOGI sits at enum index 100, so it opens page 2 (100 per page)"
    );

    // The pager's links carry a bare page number (Java's DefaultFormatter),
    // which the command parses as a page rather than an effect name.
    assert!(
        page0.contains("bypass -h admin_ave_abnormal 1"),
        "next-page link is `<bypass> <page>`"
    );
    on_packet(&mut world, 1, build_admin("ave_abnormal 1"));
    let page1 = drain(&mut rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("page 2 html");
    assert!(
        page1.contains("bypass admin_ave_abnormal YOGI\""),
        "page 2 holds the next 100 effects"
    );
    assert!(
        !page1.contains("bypass admin_ave_abnormal STUN"),
        "and not the first page's"
    );

    // A name still toggles, so the buttons work.
    world
        .objects
        .add_components(&7501, crate::model::components::TargetRef(Some(7501)));
    on_packet(&mut world, 1, build_admin("ave_abnormal AURA_BUFF"));
    assert!(
        crate::game_loop::abnormal::visual_effects(&world, 7501).contains(&57),
        "clicking a button applies the effect"
    );
}

/// **The Effects panel's buttons come back with a page.** Java ends
/// `AdminEffects.useAdminCommand` with `if (command.contains("menu"))
/// showMainPage(...)`, which re-serves `effects_menu.htm` — or `social.htm`
/// for the social commands, the panel's own sub-page. The port ran the action
/// and sent nothing, so every button press dropped the panel and "Social"
/// opened nothing at all. `//transform_menu` belongs to `AdminTransform` and
/// has its own page (`transform.htm`), not the main GM menu the port served.
#[test]
fn effects_panel_menu_commands_reserve_their_pages() {
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, ..) = admin_world();
    world.data.root = ROOT.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 7510, 100);
    world
        .objects
        .add_components(&7510, crate::model::components::TargetRef(Some(7510)));
    drain(&mut rx);

    let click = |world: &mut World,
                 rx: &mut tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
                 cmd: &str|
     -> String {
        on_packet(world, 1, build_admin(cmd));
        drain(rx)
            .iter()
            .filter_map(|p| decode_npc_html(p))
            .next_back()
            .unwrap_or_default()
    };

    assert!(
        click(&mut world, &mut rx, "social_menu 2").contains("Social Menu"),
        "social lands on social.htm"
    );
    assert!(
        click(&mut world, &mut rx, "effect_menu").contains("Effects Menu"),
        "effect_menu serves the panel"
    );
    for cmd in [
        "para_menu",
        "unpara_menu",
        "para_all_menu",
        "unpara_all_menu",
    ] {
        assert!(
            click(&mut world, &mut rx, cmd).contains("Effects Menu"),
            "{cmd} leaves the panel up"
        );
    }
    assert!(
        click(&mut world, &mut rx, "earthquake_menu 20 10").contains("Effects Menu"),
        "earthquake_menu leaves the panel up"
    );
    assert!(
        click(&mut world, &mut rx, "transform_menu").contains("Transform"),
        "transform_menu opens the transform sub-page, not gm_menu"
    );
    // A non-menu command must NOT drag a page in.
    on_packet(&mut world, 1, build_admin("para"));
    assert!(
        drain(&mut rx)
            .iter()
            .filter_map(|p| decode_npc_html(p))
            .next()
            .is_none(),
        "the bare command sends no html, as in Java"
    );
}

/// **The pager is `PageBuilder`'s default, numbered one.** `AdminEffects` never
/// calls `pageHandler()`, so `//ave_abnormal` gets `DefaultPageHandler` +
/// `ButtonsStyle`: a numbered strip whose current page is plain text and whose
/// others are buttons. The port had rendered `NextPrevPageHandler`'s
/// `First | Prev | Page: x/y | Next | Last` strip, which this page never uses.
#[test]
fn ave_menu_pager_is_the_numbered_default_handler() {
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, ..) = admin_world();
    world.data.root = ROOT.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 7511, 100);
    drain(&mut rx);

    on_packet(&mut world, 1, build_admin("ave_abnormal"));
    let page0 = drain(&mut rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("menu html");
    // 206 effects at 100 per page = 3 pages, all three linked from page one
    // (`DefaultPageHandler`'s window is the current page ± 2).
    assert!(
        page0.contains("<td>1</td>"),
        "the current page is plain text, not a button: {page0}"
    );
    for page in ["1", "2"] {
        assert!(
            page0.contains(&format!(
                "<button action=\"bypass -h admin_ave_abnormal {page}\" value=\"{}\" ",
                page.parse::<i32>().unwrap() + 1
            )),
            "page {page} is a numbered button"
        );
    }
    assert!(
        !page0.contains("admin_ave_abnormal 3"),
        "no link past the last page (index 2)"
    );
    assert!(
        !page0.contains("Page: 1/") && !page0.contains("value=\"Last\""),
        "not the next/prev strip"
    );
    // The fullest page must stay under the ~17k the client chokes on.
    assert!(
        page0.len() < 17_000,
        "page html is {} chars — over the client's limit",
        page0.len()
    );

    // The final page is reachable and populated.
    on_packet(&mut world, 1, build_admin("ave_abnormal 2"));
    let last = drain(&mut rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("last page html");
    assert!(
        last.contains("bypass admin_ave_abnormal BR_Y_3_ACCESSORY_NECKRACE"),
        "the last page holds the tail of the enum"
    );
    assert!(
        last.contains("<td>3</td>") && last.contains("admin_ave_abnormal 0"),
        "and pages back to the first"
    );
}

/// `//cw_add 8689` then `//cw_remove 8689` — the Akamanah passive (3629) has to
/// be gone from the live book *and* from every list the next flush writes.
///
/// The live half always worked; the flush half did not. `PlayerSaveData` ships
/// both the live `SkillBook` **and** `Player.skills_by_index`, a login-time
/// snapshot that still carried an entry for the class being played, and
/// `store_player` inserts both. So after a relog while cursed (which is what
/// puts 3629 into `character_skills`, and therefore into the banked map at the
/// next login), removing the weapon dropped the `MaxCp` pump live but the
/// banked row put 3629 straight back on the next flush — the reported "max CP
/// returns to the cursed value after a relog".
#[test]
fn cursed_weapon_skill_not_persisted_after_removal() {
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, _db_tx, mut db_rx, _link) = admin_world();
    world.data.root = ROOT.to_string();
    world.data.skill_data = dist::skills_owned();
    world.data.transforms = crate::data::TransformData::load_from(ROOT);
    world.data.cursed_weapons = crate::data::CursedWeaponData::load_from(ROOT);
    world.cursed_weapons = world
        .data
        .cursed_weapons
        .weapons
        .iter()
        .cloned()
        .map(|mut cw| {
            cw.skill_max_level = (1..=100)
                .take_while(|l| world.data.skill_data.get(cw.skill_id, *l).is_some())
                .last()
                .unwrap_or(1);
            cw
        })
        .collect();
    world.id_pool = 0x3000_0000..0x3000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 7009, 100);
    drain(&mut rx);
    drain_db(&mut db_rx);

    let max_cp_before = world
        .objects
        .get_component::<crate::model::components::PlayerVitals>(&7009)
        .unwrap()
        .max_cp;

    // //cw_add 8689 (+ confirm).
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("cw_add 8689"),
        ]
        .concat(),
    );
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    drain(&mut rx);

    let book_has = |w: &World| {
        w.objects
            .get_component::<crate::model::components::SkillBook>(&7009)
            .unwrap()
            .0
            .contains_key(&3629)
    };
    assert!(book_has(&world), "cursed passive granted");
    let max_cp_cursed = world
        .objects
        .get_component::<crate::model::components::PlayerVitals>(&7009)
        .unwrap()
        .max_cp;
    assert!(
        max_cp_cursed > max_cp_before,
        "curse pumps MaxCp ({max_cp_before} -> {max_cp_cursed})"
    );

    // An autosave while cursed writes 3629 to `character_skills`.
    let cursed_save = build_save_data(&world, 7009).expect("snapshot");
    assert!(
        cursed_save.skills.iter().any(|(id, _, _)| *id == 3629),
        "while cursed, the flush carries 3629"
    );

    // A relog *while cursed* is the state that matters: the character loads
    // with 3629 already in `character_skills`, so `Player.skills_by_index`
    // banks it for the active class index. Seed exactly that.
    {
        let live: Vec<(i32, i32, i32)> = world
            .objects
            .get_component::<crate::model::components::SkillBook>(&7009)
            .unwrap()
            .0
            .iter()
            .map(|(id, lvl)| (*id, *lvl, 0))
            .collect();
        let p = world.objects.get_component_mut::<Player>(&7009).unwrap();
        let ci = p.class_index;
        p.skills_by_index.insert(ci, live);
    }

    // //cw_remove 8689.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("cw_remove 8689"),
        ]
        .concat(),
    );
    drain(&mut rx);

    assert!(!book_has(&world), "cursed passive dropped from the book");
    let max_cp_after = world
        .objects
        .get_component::<crate::model::components::PlayerVitals>(&7009)
        .unwrap()
        .max_cp;
    assert_eq!(max_cp_after, max_cp_before, "MaxCp back to normal live");

    // The flush that follows must not carry it, or the relog reloads it.
    let clean_save = build_save_data(&world, 7009).expect("snapshot");
    let ids: Vec<i32> = clean_save.skills.iter().map(|(id, _, _)| *id).collect();
    let by_index: Vec<(i32, Vec<i32>)> = clean_save
        .skills_by_index
        .iter()
        .map(|(i, v)| (*i, v.iter().map(|(id, _, _)| *id).collect()))
        .collect();
    assert!(
        !ids.contains(&3629),
        "3629 still in the active flush list: {ids:?}"
    );
    assert!(
        !by_index.iter().any(|(_, v)| v.contains(&3629)),
        "3629 still in a banked per-index flush list: {by_index:?}"
    );
}

/// **`//world_missing_htmls`** lists talkable NPCs with no dialog page of
/// their own — the datapack audit a builder runs before shipping content.
///
/// The three exclusions are the point, and each is a different reason to skip:
/// a **monster** is not folk, a **non-talkable** NPC has no chat window to
/// miss, and an NPC whose chat window is supplied by a **script**
/// (`ON_NPC_FIRST_TALK`) needs no file at all. A sweep that only checked "is
/// there a .htm" would report all three as broken.
#[test]
fn missing_htmls_reports_folk_without_a_page_and_skips_the_three_exclusions() {
    let (mut world, ..) = admin_world();

    // A talkable Folk with no `data/html/default/<id>.htm` — the real finding.
    // 90501 is synthetic, so no dist file can exist for it.
    add_test_npc(&mut world, 7001, 90501, "Folk", 20, 100, 0, 0);
    // A monster: excluded regardless of html.
    add_test_npc(&mut world, 7002, 90502, "Monster", 20, 120, 0, 0);
    // A non-talkable Folk: nothing to open.
    add_test_npc(&mut world, 7003, 90503, "Folk", 20, 140, 0, 0);
    if let Some(t) = world.data.npc_data.get(90503).cloned() {
        let mut t = t;
        t.talkable = false;
        world.data.npc_data.insert_for_test(t);
    }

    let found: Vec<i32> = crate::game_loop::admin::missing_htmls::scan_for_test(&mut world, None)
        .into_iter()
        .map(|(id, ..)| id)
        .collect();

    assert!(
        found.contains(&90501),
        "the talkable folk with no page is reported: {found:?}"
    );
    assert!(
        !found.contains(&90502),
        "a monster is not folk and is skipped: {found:?}"
    );
    assert!(
        !found.contains(&90503),
        "a non-talkable NPC has no window to miss: {found:?}"
    );
}

/// The geomap-scoped sweep only reports NPCs inside the GM's own geodata tile
/// — that is the whole difference between it and the world sweep.
#[test]
fn geomap_missing_htmls_is_scoped_to_the_tile() {
    let (mut world, ..) = admin_world();
    add_test_npc(&mut world, 7010, 90511, "Folk", 20, 100, 0, 0);

    // A box around the near NPC includes it; a far-away box does not.
    let near = crate::game_loop::admin::missing_htmls::scan_for_test(
        &mut world,
        Some((-1000, -1000, 1000, 1000)),
    );
    let far = crate::game_loop::admin::missing_htmls::scan_for_test(
        &mut world,
        Some((500_000, 500_000, 600_000, 600_000)),
    );

    assert!(
        near.iter().any(|&(id, ..)| id == 90511),
        "inside the tile it is reported"
    );
    assert!(
        !far.iter().any(|&(id, ..)| id == 90511),
        "outside it is not"
    );
}

/// **`//forge_send sc`** puts the forged bytes on the GM's own socket — the
/// whole point of the tool, and the half that unit tests of the encoder cannot
/// see.
///
/// `$oid` is resolved before the operand is written, so the packet carries the
/// GM's object id rather than the literal token.
#[test]
fn forge_send_sc_writes_the_forged_packet_to_the_gm() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    drain(&mut gm_rx);

    // opcode 0x2F, one dword operand: the GM's own object id.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("forge_send sc 0x2F d $oid"),
        ]
        .concat(),
    );

    let pkts = drain(&mut gm_rx);
    let forged = pkts
        .iter()
        .find(|p| p.len() == 5 && p[0] == 0x2F)
        .expect("the forged packet reached the GM");
    assert_eq!(
        i32::from_le_bytes(forged[1..5].try_into().unwrap()),
        5001,
        "$oid was substituted, not written literally"
    );
}

/// `cs` refuses rather than forging an inbound packet — matching Java, whose
/// branch throws `UnsupportedOperationException`. The refusal is the ported
/// behaviour, so it must be visible rather than silent.
#[test]
fn forge_send_cs_refuses_like_java() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("forge_send cs 0x2F"),
        ]
        .concat(),
    );

    let pkts = drain(&mut gm_rx);
    assert!(
        count_system_messages(&pkts) > 0,
        "the refusal is reported to the GM"
    );
    assert!(
        !pkts.iter().any(|p| p.len() == 1 && p[0] == 0x2F),
        "and nothing is forged"
    );
}

/// `//playmovie` carries Java's `MovieHolder` bookkeeping: the state is
/// remembered, a second movie is refused while one plays, `EndScenePlayer`
/// only clears on the matching id, Esc (`RequestExEscapeScene`) ends an
/// escapable movie with `ExStopScenePlayer` but is ignored for a
/// non-escapable one, and an id outside the `Movie` table is refused
/// outright (Java's `findByClientId` → catch → usage).
#[test]
fn playmovie_movie_holder_bookkeeping() {
    use crate::model::components::InMovie;

    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 6496, 100);
    drain(&mut rx);

    let play = |world: &mut World, id: &str| {
        on_packet(
            world,
            1,
            [
                vec![cop::SEND_BYPASS_BUILD_CMD],
                build_cmd_body(&format!("playmovie {id}")),
            ]
            .concat(),
        );
    };
    let in_movie = |world: &World| {
        world
            .objects
            .get_component::<InMovie>(&6496)
            .map(|m| (m.movie_id, m.escapable))
    };
    let stop_sent = |pkts: &[Vec<u8>], id: i32| {
        pkts.iter().any(|p| {
            p.first() == Some(&0xFE)
                && p[1..3] == 0xE7u16.to_le_bytes()
                && p[3..7] == id.to_le_bytes()
        })
    };

    // 39 is a hole in the Movie enum — refused, no state.
    play(&mut world, "39");
    assert_eq!(in_movie(&world), None, "an unknown id never starts a movie");

    play(&mut world, "101"); // SI_ILLUSION_01_QUE, escapable
    assert_eq!(in_movie(&world), Some((101, true)));
    drain(&mut rx);

    // A second movie while one is playing is Java's `_movieHolder != null`
    // refusal.
    play(&mut world, "102");
    assert_eq!(in_movie(&world), Some((101, true)), "still the first movie");

    // The end notice must echo the running movie's id to count.
    on_packet(&mut world, 1, ex_packet(0x58, &102i32.to_le_bytes()));
    assert_eq!(in_movie(&world), Some((101, true)), "wrong id ignored");
    on_packet(&mut world, 1, ex_packet(0x58, &101i32.to_le_bytes()));
    assert_eq!(in_movie(&world), None, "matching id ends the movie");

    // Esc ends an escapable movie (single viewer: the vote passes at once)…
    play(&mut world, "101");
    drain(&mut rx);
    on_packet(&mut world, 1, ex_packet(0x90, &[]));
    assert_eq!(in_movie(&world), None, "Esc ended the escapable movie");
    assert!(
        stop_sent(&drain(&mut rx), 101),
        "ExStopScenePlayer answers the escape"
    );

    // …but a non-escapable one ignores it (15 = SC_BOSS_FREYA_OPENING).
    play(&mut world, "15");
    assert_eq!(in_movie(&world), Some((15, false)));
    on_packet(&mut world, 1, ex_packet(0x90, &[]));
    assert_eq!(
        in_movie(&world),
        Some((15, false)),
        "Esc is ignored for a non-escapable movie"
    );
}

/// `//instancedestroy` warns everyone inside with the "destroyed by Game
/// Master" screen banner before the teleport-out, like Java's AdminInstance.
#[test]
fn admin_instancedestroy_warns_the_players_inside() {
    use crate::data::instance_data::{ExitType, InstanceTemplate};
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    world
        .data
        .instance_templates
        .insert_for_test(InstanceTemplate {
            id: 902,
            name: Some("Doomed Arena".into()),
            max_worlds: -1,
            duration_min: 60,
            empty_destroy_min: 5,
            enter: Some((100, 200, 300)),
            exit: ExitType::Origin,
            doors: vec![],
            groups: vec![],
        });
    let iid = crate::game_loop::instances::create_from_template(&mut world, 902).expect("template");
    let mut gm = ingame_player_access(&mut world, 1, 6441, 100);
    let mut inhabitant = ingame_player_access(&mut world, 2, 6442, 0);
    crate::game_loop::instances::enter(&mut world, 6442, iid);
    drain(&mut gm);
    drain(&mut inhabitant);

    // `admin_instancedestroy` carries `confirmDlg="true"` — answer it.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body(&format!("instancedestroy {iid}")),
        ]
        .concat(),
    );
    const S1_3: i32 = server_packets::S1_3_MESSAGE_ID;
    on_packet(
        &mut world,
        1,
        [vec![cop::DLG_ANSWER], dlg_answer_body(S1_3, 1, 0)].concat(),
    );

    let pkts = drain(&mut inhabitant);
    let warned = pkts.iter().any(|p| {
        p[0] == server_packets::opcodes::EX
            && i16::from_le_bytes([p[1], p[2]]) == server_packets::opcodes::EX_SHOW_SCREEN_MESSAGE
    });
    assert!(warned, "the inhabitant saw the Game Master banner");
    assert!(world.instances.get(iid).is_none(), "the instance is gone");
}

/// Insert a minimal clan led by `leader_id` and enrol every `members` player.
fn seed_clan(world: &mut World, clan_id: i32, leader_id: i32, members: &[i32]) {
    world.clans.insert(
        clan_id,
        crate::model::clan::Clan {
            id: clan_id,
            name: "Tail".into(),
            leader_id,
            level: 3,
            reputation_score: 0,
            castle_id: 0,
            members: Vec::new(),
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
    for oid in members {
        world
            .objects
            .get_component_mut::<Player>(oid)
            .unwrap()
            .clan_id = clan_id;
    }
}

fn has_admin_html(pkts: &[Vec<u8>]) -> bool {
    pkts.iter()
        .any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
}

/// `AdminPledge` re-shows the Game panel after every branch **except** the one
/// where `Integer.parseInt` throws past it into `AdminCommandHandler`. Both
/// halves are asserted together: a bad level prints the exception line and
/// leaves the panel closed, while a merely out-of-range level prints "Level
/// incorrect." and still re-opens it. The pair is what keeps a refactor from
/// quietly collapsing the two exits into one.
#[test]
fn pledge_setlevel_reopens_the_panel_except_when_the_parse_throws() {
    use crate::model::components::TargetRef;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7801, 100);
    let _member_rx = ingame_player_access(&mut world, 2, 7802, 0);
    seed_clan(&mut world, 700, 7802, &[7802]);
    world.objects.add_components(&7801, TargetRef(Some(7802)));
    drain(&mut gm_rx);

    // Non-numeric level → Java's NumberFormatException path: message, no panel.
    on_packet(&mut world, 1, build_admin("pledge setlevel abc"));
    let pkts = drain(&mut gm_rx);
    assert!(
        count_system_messages(&pkts) >= 1,
        "the exception line is printed"
    );
    assert!(
        !has_admin_html(&pkts),
        "the throw unwinds past showMainPage — no Game panel"
    );

    // Numeric but out of range → ordinary refusal: message AND the panel.
    on_packet(&mut world, 1, build_admin("pledge setlevel 99"));
    let pkts = drain(&mut gm_rx);
    assert!(
        count_system_messages(&pkts) >= 1,
        "\"Level incorrect.\" is printed"
    );
    assert!(
        has_admin_html(&pkts),
        "an ordinary refusal still re-opens the Game panel"
    );
    assert_eq!(
        world.clans.get(&700).unwrap().level,
        3,
        "neither refusal changed the clan"
    );
}

/// `AdminMenu.teleportToCharacter` reopens `charmanage.htm` on every path but
/// the unresolved target, which returns straight out. The self-target case is
/// the counterexample that stops the tail from being read as "on success only".
#[test]
fn goto_char_reopens_the_page_except_on_an_unresolved_target() {
    use crate::model::components::TargetRef;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7811, 100);
    drain(&mut gm_rx);

    // Nothing targeted → INVALID_TARGET and no page.
    on_packet(&mut world, 1, build_admin("goto_char_menu"));
    let pkts = drain(&mut gm_rx);
    assert!(
        has_system_message(&pkts, server_packets::sm_ids::INVALID_TARGET),
        "INVALID_TARGET"
    );
    assert!(!has_admin_html(&pkts), "no char-manage page");

    // Targeting yourself is refused by message, but the page still re-opens.
    world.objects.add_components(&7811, TargetRef(Some(7811)));
    on_packet(&mut world, 1, build_admin("goto_char_menu"));
    let pkts = drain(&mut gm_rx);
    assert!(
        has_system_message(
            &pkts,
            server_packets::sm_ids::YOU_CANNOT_USE_THIS_ON_YOURSELF
        ),
        "YOU_CANNOT_USE_THIS_ON_YOURSELF"
    );
    assert!(
        has_admin_html(&pkts),
        "the self-target refusal still re-opens charmanage.htm"
    );
}

/// `//give_clan_skills` refuses in Java's order and with Java's two distinct
/// messages: no player target → INVALID_TARGET, a clanless target →
/// THE_TARGET_MUST_BE_A_CLAN_MEMBER. The two ids are the whole point of the
/// guard — the refusal paths had no cover before, so a swapped id was silent.
#[test]
fn give_clan_skills_refuses_with_javas_two_distinct_messages() {
    use crate::model::components::TargetRef;
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
    let entry = |skill_id: i32, passive: bool| crate::model::skill::ActiveBuff {
        displayed: !passive,
        skill_id,
        skill_level: 1,
        abnormal_type_client_id: 0,
        abnormal_type: format!("T{skill_id}"),
        abnormal_level: 1,
        slot: crate::model::skill::BuffSlot::Buff,
        expires_at_tick: world.tick + 1000,
        passive,
        effect_flags: 0,
        abnormal_visuals: Vec::new(),
        blocked_abnormals: Vec::new(),
        effects: Vec::new(),
    };
    world.objects.add_components(
        &7831,
        crate::model::components::Buffs(vec![
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
        .get_component::<crate::model::components::Buffs>(&7831)
        .map(|b| b.0.iter().map(|x| (x.skill_id, x.passive)).collect())
        .unwrap_or_default();
    assert_eq!(
        left,
        vec![(313, true)],
        "only the passive survives //stopallbuffs"
    );
}

/// `//cw_goto` tries the holder first, then the dropped item, and only reports
/// "isn't in the World" when neither has a position.
///
/// The fall-through is the whole behaviour: a cursed weapon can be flagged
/// activated while its holder carries no position (offline, mid-teleport), and
/// Java still checks the ground item before giving up. Pinned because the
/// command has no other test and the branch order is easy to flatten.
#[test]
fn cw_goto_falls_through_from_the_holder_to_the_dropped_item() {
    use crate::model::components::Position;
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, ..) = admin_world();
    world.data.root = ROOT.to_string();
    world.data.cursed_weapons = crate::data::CursedWeaponData::load_from(ROOT);
    world.cursed_weapons = world.data.cursed_weapons.weapons.clone();
    let item_id = world.cursed_weapons[0].item_id;

    let mut gm_rx = ingame_player_access(&mut world, 1, 9101, 100);
    // Both anchors must be *registered* objects — components attached to an
    // unknown object id are silently dropped by the store.
    const HOLDER: i32 = 9102;
    const GROUND_ITEM: i32 = 9103;
    let _holder_rx = ingame_player_access(&mut world, 2, HOLDER, 0);
    let _item_rx = ingame_player_access(&mut world, 3, GROUND_ITEM, 0);
    // The holder is flagged but has NO position — the case that must fall through.
    world.objects.remove_component::<Position>(&HOLDER);
    set_position(&mut world, GROUND_ITEM, (84_000, 148_000, -3400));
    world.cursed_weapons[0].is_activated = true;
    world.cursed_weapons[0].player_id = HOLDER;
    world.cursed_weapons[0].is_dropped = true;
    world.cursed_weapons[0].dropped_item_oid = GROUND_ITEM;
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin(&format!("cw_goto {item_id}")));

    let at = world
        .objects
        .get_component::<Position>(&9101)
        .copied()
        .expect("gm position");
    assert_eq!(
        (at.x, at.y),
        (84_000, 148_000),
        "the GM landed on the dropped item after the position-less holder"
    );

    // With neither anchor placed, the command reports the weapon as absent.
    world.cursed_weapons[0].is_dropped = false;
    world.cursed_weapons[0].dropped_item_oid = 0;
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin(&format!("cw_goto {item_id}")));
    let after = world
        .objects
        .get_component::<Position>(&9101)
        .copied()
        .expect("gm position");
    assert_eq!((after.x, after.y), (at.x, at.y), "no anchor, no teleport");
    assert!(
        count_system_messages(&drain(&mut gm_rx)) >= 1,
        "the \"isn't in the World\" line is sent"
    );
}

/// Every `ExBasicActionList` (0xFE 0x60 00) in `pkts`, decoded back to its id
/// list, so a test can compare against the template it expects.
fn basic_action_lists(pkts: &[Vec<u8>]) -> Vec<Vec<i32>> {
    let mut out = Vec::new();
    for p in pkts {
        if p.len() < 7 || p[0] != 0xFE || p[1] != 0x60 || p[2] != 0x00 {
            continue;
        }
        let rd = |o: usize| i32::from_le_bytes([p[o], p[o + 1], p[o + 2], p[o + 3]]);
        let count = rd(3) as usize;
        if p.len() < 7 + count * 4 {
            continue;
        }
        out.push((0..count).map(|i| rd(7 + i * 4)).collect());
    }
    out
}

/// Java `Transform.onTransform` sends `ExBasicActionList(template.actions)`,
/// and `onUntransform` sends `ExBasicActionList.STATIC_PACKET` — the client's
/// action bar becomes the form's own and is restored on the way out. All 174
/// templates on this dist carry an `<actions>` block, so the swap is the half
/// of the transform data a GM can reach on every single one of them.
#[test]
fn admin_transform_swaps_and_restores_the_action_bar() {
    let (mut world, ..) = admin_world();
    world.data.transforms = crate::data::TransformData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.data.skill_data = dist::skills_owned();
    // The fixture world ships an *empty* ActionData, which would make the
    // restore leg below compare an empty bar against an empty bar and pass
    // while proving nothing. Load the real one.
    world.data.action_data = crate::data::ActionData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));

    // Transform 105 is one of the two forms a *player* can actually enter on
    // this dist (the Rabbits event casts it), which is why it is the one worth
    // pinning here rather than an admin-only id.
    let expected = world
        .data
        .transforms
        .get(105)
        .expect("transform 105 loaded")
        .template(false)
        .actions
        .clone();
    assert!(
        !expected.is_empty(),
        "the dist template carries an <actions> block"
    );
    let default_bar = world.data.action_data.action_ids().to_vec();
    assert_ne!(
        expected, default_bar,
        "the form's bar must differ from the default, or this test proves nothing"
    );

    let mut gm_rx = ingame_player_access(&mut world, 1, 8931, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("transform 105"));
    let bars = basic_action_lists(&drain(&mut gm_rx));
    assert_eq!(
        bars.last(),
        Some(&expected),
        "transforming swaps the action bar for the template's <actions>"
    );

    on_packet(&mut world, 1, build_admin("untransform"));
    let bars = basic_action_lists(&drain(&mut gm_rx));
    assert_eq!(
        bars.last(),
        Some(&default_bar),
        "untransforming restores ExBasicActionList.STATIC_PACKET"
    );
}

/// Java `IStatFunction.calcWeaponBaseValue`: the transform's `<base>` values
/// replace the equipped weapon's for every form *except* `COMBAT` and
/// `MODE_CHANGE`, which keep the weapon. Both forms a player can enter on this
/// dist are on the transform-wins side of that line (105 = NON_COMBAT,
/// 20008 = RIDING_MODE).
#[test]
fn transform_base_replaces_the_weapon_only_for_non_combat_forms() {
    let (mut world, ..) = admin_world();
    world.data.transforms = crate::data::TransformData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.data.skill_data = dist::skills_owned();
    // The fixture's single synthetic class template does not cover the test
    // player's class, so `recalculate_stats` would fall back to
    // `PlayerTemplate::default()` — every class base 0, which makes a ratio
    // assertion meaningless. Load the real templates.
    world.data.player_templates = dist::player_templates_owned();

    let non_combat = world.data.transforms.get(105).expect("105 loaded");
    assert!(
        !non_combat.kind.weapon_overrides_base(),
        "105 is NON_COMBAT — the transform's base wins"
    );
    let tf_p_atk = non_combat
        .template(false)
        .base
        .as_ref()
        .and_then(|b| b.p_atk)
        .expect("105 carries <base pAtk=…>");

    // A COMBAT form is the control: Java hands the weapon branch back to it.
    let combat = world.data.transforms.get(1).expect("1 loaded");
    assert!(
        combat.kind.weapon_overrides_base(),
        "transform 1 is COMBAT — the weapon wins"
    );

    let mut gm_rx = ingame_player_access(&mut world, 1, 8932, 100);
    drain(&mut gm_rx);
    let naked_p_atk = world
        .objects
        .get_component::<CombatStats>(&8932)
        .unwrap()
        .p_atk;
    // The finalizer is `base * STR bonus * levelMod` and only `base` moves, so
    // the expected total scales by exactly the ratio of the two bases. Deriving
    // it from the class template keeps the assertion honest whichever way the
    // numbers happen to fall.
    let class_base_p_atk = {
        let (class_id, base_class_id) = {
            let p = world.objects.get_component::<Player>(&8932).unwrap();
            (p.class_id, p.base_class_id)
        };
        // The same lookup `recalculate_stats` does, fallback included.
        world
            .data
            .player_templates
            .get(class_id)
            .or_else(|| world.data.player_templates.get(base_class_id))
            .expect("class template loaded")
            .base_p_atk as f64
    };
    assert!(
        class_base_p_atk > 0.0 && class_base_p_atk != tf_p_atk,
        "the two bases must differ, or this test proves nothing \
         (class {class_base_p_atk}, transform {tf_p_atk})"
    );

    on_packet(&mut world, 1, build_admin("transform 105"));
    let transformed = world
        .objects
        .get_component::<CombatStats>(&8932)
        .unwrap()
        .p_atk;
    let expected = naked_p_atk * tf_p_atk / class_base_p_atk;
    assert!(
        (transformed - expected).abs() < 1e-6,
        "the NON_COMBAT form's <base pAtk={tf_p_atk}> displaces the class base \
         {class_base_p_atk}: expected {expected}, got {transformed}"
    );

    on_packet(&mut world, 1, build_admin("untransform"));
    assert_eq!(
        world
            .objects
            .get_component::<CombatStats>(&8932)
            .unwrap()
            .p_atk,
        naked_p_atk,
        "reverting restores the untransformed base"
    );

    on_packet(&mut world, 1, build_admin("transform 1"));
    assert_eq!(
        world
            .objects
            .get_component::<CombatStats>(&8932)
            .unwrap()
            .p_atk,
        naked_p_atk,
        "a COMBAT form ignores <base> and keeps the weapon/class value"
    );
}

/// Java's `//gmspeed` target is any **Creature**, not just a player — an NPC
/// can be sped up too, and it gets `broadcastInfo()` rather than `UserInfo`.
#[test]
fn admin_gmspeed_scales_a_targeted_npc() {
    use crate::model::components::{Speeds, TargetRef};
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7110, 100);
    scan_npc(&mut world, NPC_OID, 7110, 50, 0, 0);
    world
        .objects
        .add_components(&7110, TargetRef(Some(NPC_OID)));
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("gmspeed 5"));
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&NPC_OID)
            .unwrap()
            .move_multiplier,
        5.0,
        "the NPC target is scaled"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&7110)
            .unwrap()
            .move_multiplier,
        1.0,
        "and the GM is left alone"
    );
}

/// `//teleportto <name>` sends the GM to a *named* player. Java's two guards:
/// an unknown name answers `INVALID_TARGET`, and your own name answers
/// `YOU_CANNOT_USE_THIS_ON_YOURSELF` — neither moves anybody.
#[test]
fn admin_teleportto_moves_the_gm_to_a_named_player() {
    use crate::model::components::Position;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7120, 100);
    let _target_rx = ingame_player(&mut world, 2, 7121, 50_000, 60_000, -3000);
    {
        let p = world.objects.get_component_mut::<Player>(&7121).unwrap();
        p.name = "Wanda".into();
    }
    let gm_pos = |w: &World| {
        let p = w.objects.get_component::<Position>(&7120).unwrap();
        (p.x, p.y)
    };
    let start = gm_pos(&world);
    drain(&mut gm_rx);

    // The assertions below check system-message *ids*, not just a count: a
    // self-teleport is positionally invisible (you land where you already are),
    // so the refusal message is the only witness that the guard fired at all.

    // Unknown name: INVALID_TARGET, nobody moves.
    on_packet(&mut world, 1, build_admin("teleportto Nobody"));
    assert_eq!(
        ids_after_opcode(&drain(&mut gm_rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::INVALID_TARGET],
        "an unknown name answers INVALID_TARGET"
    );
    assert_eq!(gm_pos(&world), start, "and moves nothing");

    // Own name: refused with Java's own message, not the success line.
    let gm_name = world
        .objects
        .get_component::<Player>(&7120)
        .unwrap()
        .name
        .clone();
    on_packet(&mut world, 1, build_admin(&format!("teleportto {gm_name}")));
    assert_eq!(
        ids_after_opcode(&drain(&mut gm_rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::YOU_CANNOT_USE_THIS_ON_YOURSELF],
        "targeting yourself is refused — and refusing is only observable here, \
         since teleporting to yourself would not move you"
    );
    assert_eq!(gm_pos(&world), start, "still put");

    // A real name: the GM lands on them.
    on_packet(&mut world, 1, build_admin("teleportto Wanda"));
    assert_eq!(
        gm_pos(&world),
        (50_000, 60_000),
        "the GM is moved onto the named player"
    );
}

/// `//remove_skills` is a *generated* per-character page in Java, not a file:
/// every row is a `bypass -h admin_remove_skill <id>` for a skill that
/// character actually knows. The port used to serve the static `skills.htm`,
/// from which a GM could not pick anything.
#[test]
fn admin_remove_skills_generates_the_targets_own_skill_list() {
    use crate::model::components::{SkillBook, TargetRef};
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

/// Java `setClassId` drops hennas the **new** class may not wear — outright,
/// with no refund, because the character never asked to remove them.
#[test]
fn setclass_drops_a_dye_the_new_class_cannot_wear() {
    use crate::model::components::HennaSlots;
    let (mut world, ..) = admin_world();
    world.data.hennas = crate::data::HennaData::load_from(crate::data::DIST_GAME);
    world.data.player_templates = dist::player_templates_owned();
    let _rx = ingame_player_access(&mut world, 1, 7140, 100);

    // Find a dye this dist restricts, and a class that may not wear it. Driven
    // off `list_for_class`: a dye that some class can wear and another cannot is
    // exactly the case the removal exists for.
    let (dye_id, forbidden_class) = (0..88)
        .filter(|c| world.data.player_templates.get(*c).is_some())
        .find_map(|allowed_class| {
            let dye = world
                .data
                .hennas
                .list_for_class(allowed_class)
                .first()?
                .dye_id;
            let h = world.data.hennas.get(dye)?;
            let bad = (0..88).find(|c| {
                world.data.player_templates.get(*c).is_some() && !h.is_allowed_class(*c)
            })?;
            Some((dye, bad))
        })
        .expect("some dye is class-restricted on this dist");

    world
        .objects
        .add_components(&7140, HennaSlots([Some(dye_id), None, None]));
    on_packet(
        &mut world,
        1,
        build_admin(&format!("setclass {forbidden_class}")),
    );
    assert_eq!(
        world.objects.get_component::<HennaSlots>(&7140).unwrap().0[0],
        None,
        "the dye the new class cannot wear came off"
    );
}
