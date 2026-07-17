use super::*;

/// A GM's `//serverinfo` runs and answers with server-info text lines.
#[test]
fn admin_serverinfo_runs_for_gm() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("serverinfo")].concat());
    let pkts = drain(&mut gm_rx);
    assert_eq!(count_system_messages(&pkts), 3, "three server-info lines");
}

/// A non-GM issuing an admin command is silently ignored (Java `isGM` gate).
#[test]
fn admin_command_ignored_for_non_gm() {
    let (mut world, ..) = admin_world();
    let mut user_rx = ingame_player_access(&mut world, 1, 5002, 0);
    drain(&mut user_rx);

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("serverinfo")].concat());
    assert!(drain(&mut user_rx).is_empty(), "non-GM gets no reply at all");
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

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("serverinfo")].concat());
    let pkts = drain(&mut rx);
    // One system message: the "no access rights" refusal, not the 3 info lines.
    assert_eq!(count_system_messages(&pkts), 1, "single refusal line, command not run");
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
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("totally_made_up")].concat());
    assert_eq!(count_system_messages(&drain(&mut rx)), 1, "does-not-exist line");

    // In AdminCommands.xml (admin_instance, level 100, no confirm) but no body
    // yet (the instance system is not ported) → not-implemented path, does not
    // crash.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("instance")].concat());
    assert_eq!(count_system_messages(&drain(&mut rx)), 1, "not-implemented line");
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
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("givehero")].concat());
    let pkts = drain(&mut rx);
    assert_eq!(pkts.len(), 1, "only the ConfirmDlg is sent");
    assert_eq!(pkts[0][0], server_packets::opcodes::CONFIRM_DLG, "it's a ConfirmDlg");
    assert_eq!(count_system_messages(&pkts), 0, "command did not execute yet");

    // Answer "yes" → the stored command re-runs and reaches dispatch (givehero
    // has no body yet → the not-implemented reply proves re-execution).
    on_packet(&mut world, 1, [vec![cop::DLG_ANSWER], dlg_answer_body(S1_3, 1, 0)].concat());
    assert_eq!(count_system_messages(&drain(&mut rx)), 1, "re-ran on confirm");

    // A second "yes" does nothing — the pending command was consumed.
    on_packet(&mut world, 1, [vec![cop::DLG_ANSWER], dlg_answer_body(S1_3, 1, 0)].concat());
    assert!(drain(&mut rx).is_empty(), "no pending command to re-run");
}

/// Answering "no" to the confirm drops the command without executing it.
#[test]
fn admin_confirm_dialog_declined() {
    const S1_3: i32 = server_packets::S1_3_MESSAGE_ID;

    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 5006, 100);
    drain(&mut rx);

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("givehero")].concat());
    drain(&mut rx);
    on_packet(&mut world, 1, [vec![cop::DLG_ANSWER], dlg_answer_body(S1_3, 0, 0)].concat());
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
    assert!(world.objects.get_component::<Player>(&6401).unwrap().hero_aura);

    // Normal player, aura on → still no glow (not a GM).
    let _user = ingame_player_access(&mut world, 2, 6402, 0);
    assert!(!world.objects.get_component::<Player>(&6402).unwrap().hero_aura);

    // Same GM with the aura off → no glow.
    world.data.gm.hero_aura = false;
    let _gm2 = ingame_player_access(&mut world, 3, 6403, 100);
    assert!(!world.objects.get_component::<Player>(&6403).unwrap().hero_aura);
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
    super::admin::apply_gm_startup(&mut world, 1, 6411);

    let f = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&6411)
        .copied()
        .unwrap_or_default();
    assert!(f.invul, "GMStartupInvulnerable applied");
    assert!(f.hidden, "GMStartupInvisible applied");
    assert!(!f.silence && !f.diet, "unset startup flags stay off");
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
    super::admin::apply_gm_startup(&mut world, 1, 6421);

    let f = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&6421)
        .copied()
        .unwrap_or_default();
    assert!(f.hidden, "builder hide set");
    assert!(!f.invul, "builder hide broke before the invul flag");
    assert_eq!(count_system_messages(&drain(&mut rx)), 3, "three builder notices");
}

/// `//admin` opens the main menu page — the real `main_menu.htm` is served (not
/// the missing-file placeholder) through an `NpcHtmlMessage`.
#[test]
fn admin_menu_serves_main_page() {
    let (mut world, ..) = admin_world();
    // Point the datapack root at dist/game so the html file resolves.
    world.data.root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/").to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6431, 100);
    drain(&mut rx);

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("admin")].concat());
    let pkts = drain(&mut rx);
    let html = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .expect("an NpcHtmlMessage was sent");

    // Decode: object_id (0) then the UTF-16 html string.
    let mut r = commons::network::PacketReader::new(&html[1..]);
    assert_eq!(r.read_i32().unwrap(), 0, "admin menu is not NPC-scoped");
    let content = r.read_string().unwrap();
    assert!(!content.contains("My text is missing"), "main_menu.htm was found");
    assert!(content.contains("admin_admin"), "menu links back through the admin_ bypass");
}

/// `//show_characters` and `//character_info` render HTML windows (Java
/// `listCharacters`/`showCharacterInfo`), not text lines: the regression the
/// user flagged.
#[test]
fn admin_editchar_info_commands_use_html() {
    let (mut world, ..) = admin_world();
    world.data.root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/").to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6432, 100);
    drain(&mut rx);

    // //show_characters → charlist.htm (with the caller's own row).
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("show_characters")].concat());
    let pkts = drain(&mut rx);
    let list = pkts.iter().find_map(|p| decode_npc_html(p)).expect("charlist html");
    assert!(!list.contains("My text is missing"), "charlist.htm found");
    assert!(list.contains("Character Selection"), "charlist body");
    assert!(list.contains("admin_character_info P6432"), "roster links to character_info");
    assert!(!has_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE), "no sysmessage fallback");

    // //character_info (self via target) → charinfo.htm filled with the name.
    world.objects.add_components(&6432, crate::model::components::TargetRef(Some(6432)));
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("character_info")].concat());
    let info = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("charinfo html");
    assert!(!info.contains("My text is missing"), "charinfo.htm found");
    assert!(info.contains("P6432"), "charinfo shows the character name");
    assert!(!info.contains("%name%") && !info.contains("%level%"), "charinfo tokens replaced");
}

/// `//grandboss` opens the boss menu; `//grandboss <id>` shows one boss's live
/// status/respawn from `world.grand_bosses`; the per-boss action buttons hit the
/// unported boss AI (Java NPEs on the null AI, reproduced here). Port of
/// `AdminGrandBoss`.
#[test]
fn admin_grandboss_status_panel_and_actions() {
    use crate::model::grand_boss::GrandBoss;
    let (mut world, ..) = admin_world();
    world.data.root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/").to_string();
    // Queen Ant alive (status 0); Antharas dead (status 3) with a known respawn.
    world.grand_bosses.insert(
        29001,
        GrandBoss { boss_id: 29001, loc_x: 0, loc_y: 0, loc_z: 0, heading: 0, respawn_time: 0, current_hp: 1.0, current_mp: 1.0, status: 0 },
    );
    world.grand_bosses.insert(
        29068,
        GrandBoss { boss_id: 29068, loc_x: 0, loc_y: 0, loc_z: 0, heading: 0, respawn_time: 1_700_000_000_000, current_hp: 1.0, current_mp: 1.0, status: 3 },
    );
    let mut rx = ingame_player_access(&mut world, 1, 6440, 100);
    drain(&mut rx);

    // Menu: the six-boss list.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("grandboss")].concat());
    let menu = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("grandboss menu html");
    assert!(!menu.contains("My text is missing"), "grandboss.htm found");
    assert!(menu.contains("admin_grandboss 29001"), "menu links to each boss");

    // Queen Ant: alive → green, not-yet-respawned label.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("grandboss 29001")].concat());
    let qa = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("queenant html");
    assert!(qa.contains("Alive") && qa.contains("00FF00"), "alive status + green color");
    assert!(qa.contains("Already respawned!"), "alive boss is not awaiting respawn");

    // Antharas: dead → red, formatted respawn date (UTC), zone count unported.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("grandboss 29068")].concat());
    let an = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("antharas html");
    assert!(an.contains("Dead") && an.contains("FF0000"), "dead status + red color");
    assert!(an.contains("2023-11-14 22:13:20"), "formatted respawn time");
    assert!(an.contains("Zone not found!"), "boss-zone player count unported (G21)");

    // Action buttons: no arg → Usage; unsupported id → Wrong ID; supported id →
    // the dist's null-AI NPE, with no status page.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("grandboss_skip")].concat());
    let m = drain(&mut rx);
    assert!(m.iter().filter_map(|p| sysmsg_text(p)).any(|t| t == "Usage: //grandboss_skip Id"));

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("grandboss_skip 29014")].concat());
    let m = drain(&mut rx);
    assert!(m.iter().filter_map(|p| sysmsg_text(p)).any(|t| t == "Wrong ID!"), "skip is Antharas-only");

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("grandboss_skip 29068")].concat());
    let m = drain(&mut rx);
    assert!(m.iter().filter_map(|p| sysmsg_text(p)).any(|t| t.contains("NullPointerException")), "unported AI reproduces the dist NPE");
    assert!(!has_opcode(&m, server_packets::opcodes::NPC_HTML_MESSAGE), "NPE path shows no status page");
}

/// `//cw_info` lists both cursed weapons; `//cw_add` activates one on the GM
/// (item + karma swap + cursed-weapon flag + DB persist + world announce);
/// `//cw_remove` reverses it. Port of `AdminCursedWeapons`.
#[test]
fn admin_cursed_weapons_info_add_remove() {
    const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
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
            cw.skill_max_level =
                (1..=100).take_while(|l| world.data.skill_data.get(cw.skill_id, *l).is_some()).last().unwrap_or(1);
            cw
        })
        .collect();
    assert_eq!(world.cursed_weapons.len(), 2, "Zariche + Akamanah loaded from XML");

    world.id_pool = 0x3000_0000..0x3000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 7001, 100);
    drain(&mut rx);
    let original_rep = world.objects.get_component::<Player>(&7001).unwrap().reputation;

    // //cw_info — both weapons inactive.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("cw_info")].concat());
    let info: Vec<String> = drain(&mut rx).iter().filter_map(|p| sysmsg_text(p)).collect();
    assert!(info.iter().any(|t| t.contains("Demonic Sword Zariche (8190)")), "lists Zariche");
    assert!(info.iter().any(|t| t.contains("Don't exist in the world.")), "inactive status");

    // //cw_add 8190 — Java marks it confirmDlg, so it prompts first; the "yes"
    // reply then activates it on the GM (no target).
    drain_db(&mut db_rx);
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("cw_add 8190")].concat());
    let prompt = drain(&mut rx);
    assert_eq!(prompt.iter().filter(|p| p[0] == server_packets::opcodes::CONFIRM_DLG).count(), 1, "confirm prompt");
    on_packet(&mut world, 1, [vec![cop::DLG_ANSWER], dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0)].concat());
    let add_pkts = drain(&mut rx);
    let p = world.objects.get_component::<Player>(&7001).unwrap();
    assert_eq!(p.cursed_weapon_equipped_id, 8190, "cursed-weapon flag set");
    assert_eq!(p.reputation, -9_999_999, "karma slammed to the cursed value");
    let cw = world.cursed_weapons.iter().find(|c| c.item_id == 8190).unwrap();
    assert!(cw.is_activated && cw.player_id == 7001, "weapon activated on the wielder");
    assert_eq!(cw.player_reputation, original_rep, "saved the wielder's real karma");
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::StoreCursedWeapon { item_id: 8190, char_id: 7001, .. })), "persisted");
    assert!(
        sm_ids_of(&add_pkts).contains(&server_packets::sm_ids::THE_OWNER_OF_S2_HAS_APPEARED_IN_THE_S1_REGION),
        "appearance announced"
    );

    // //cw_info now shows the wielder.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("cw_info")].concat());
    let info: Vec<String> = drain(&mut rx).iter().filter_map(|p| sysmsg_text(p)).collect();
    assert!(info.iter().any(|t| t.contains("Player holding: P7001")), "shows the holder");

    // //cw_remove 8190 — end of life restores the wielder + resets state.
    drain_db(&mut db_rx);
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("cw_remove 8190")].concat());
    let rm_pkts = drain(&mut rx);
    let p = world.objects.get_component::<Player>(&7001).unwrap();
    assert_eq!(p.cursed_weapon_equipped_id, 0, "flag cleared");
    assert_eq!(p.reputation, original_rep, "karma restored");
    let cw = world.cursed_weapons.iter().find(|c| c.item_id == 8190).unwrap();
    assert!(!cw.is_active(), "weapon reset to not-in-world");
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::RemoveCursedWeapon { item_id: 8190 })), "db row dropped");
    assert!(sm_ids_of(&rm_pkts).contains(&server_packets::sm_ids::S1_HAS_DISAPPEARED), "disappearance announced");
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
    assert_eq!(isgm_byte(&world, 6461, "P6461"), 1, "GM UserInfo enables the //command bar");

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
    world.data.root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/").to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6471, 100);
    drain(&mut rx);

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("silence")].concat());
    let pkts = drain(&mut rx);
    assert!(world.objects.get_component::<AdminFlags>(&6471).unwrap().silence, "silence on");
    assert!(has_system_message(&pkts, 177), "MESSAGE_REFUSAL_MODE");
    assert_eq!(etc_status_mask(&pkts).map(|m| m & 1), Some(1), "EtcStatusUpdate refusal bit set");

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("silence")].concat());
    let pkts = drain(&mut rx);
    assert!(!world.objects.get_component::<AdminFlags>(&6471).unwrap().silence, "silence off");
    assert!(has_system_message(&pkts, 178), "MESSAGE_ACCEPTANCE_MODE");
    assert_eq!(etc_status_mask(&pkts).map(|m| m & 1), Some(0), "EtcStatusUpdate refusal bit cleared");
}

/// `//hide` sends the GM's own client an `ExUserInfoAbnormalVisualEffect` with
/// the STEALTH effect present (so the invisible state renders), and clears it
/// on unhide.
#[test]
fn admin_hide_sends_stealth_visual() {
    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 6491, 100);
    drain(&mut rx);

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("hide")].concat());
    assert_eq!(ave_effect_count(&drain(&mut rx)), Some(1), "STEALTH present when hidden");

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("hide")].concat());
    assert_eq!(ave_effect_count(&drain(&mut rx)), Some(0), "no effects when visible again");
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
    world.objects.add_components(&7001, crate::model::components::TargetRef(Some(7002)));

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("heal")].concat());

    let v = pvit(&world, 7002);
    assert_eq!(v.cur_hp, v.max_hp as f64, "victim fully healed");
    assert!(
        drain(&mut victim_rx).iter().any(|p| p[0] == server_packets::opcodes::STATUS_UPDATE),
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

    world.objects.add_components(&7003, crate::model::components::TargetRef(Some(7004)));
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("kill")].concat());

    assert!(pvit(&world, 7004).dead, "victim is dead after //kill");
}

/// `//kill` with no target tells the GM to select one and kills nothing.
#[test]
fn admin_kill_without_target_warns() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7005, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("kill")].concat());
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "one 'select a target' line");
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
    world.objects.add_components(&7101, crate::model::components::TargetRef(Some(7102)));
    on_packet(&mut world, 1, build_admin("res"));

    let v = pvit(&world, 7102);
    assert!(!v.dead, "victim revived");
    assert_eq!(v.cur_hp, v.max_hp as f64, "victim fully restored");
}

/// `//gmspeed N` sets the move multiplier to `1 + N` (0 resets) and rebroadcasts
/// UserInfo.
#[test]
fn admin_gmspeed_sets_move_multiplier() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7103, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("gmspeed 3"));
    assert_eq!(
        world.objects.get_component::<crate::model::components::Speeds>(&7103).unwrap().move_multiplier,
        4.0,
        "1 + boost"
    );
    assert!(drain(&mut gm_rx).iter().any(|p| p[0] == 0x32), "UserInfo (0x32) rebroadcast");

    on_packet(&mut world, 1, build_admin("gmspeed 0"));
    assert_eq!(
        world.objects.get_component::<crate::model::components::Speeds>(&7103).unwrap().move_multiplier,
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
        world.objects.get_component::<crate::model::components::Speeds>(&7107).unwrap().move_multiplier,
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
    let pos = *world.objects.get_component::<crate::model::components::Position>(&7104).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (100, 200, 305), "moved to coords (z lifted by 5)");
    assert!(
        drain(&mut gm_rx).iter().any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION),
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

    if let Some(p) = world.objects.get_component_mut::<crate::model::components::Position>(&7105) {
        p.x = 500;
        p.y = 600;
        p.z = 700;
    }
    on_packet(&mut world, 1, build_admin("recall P7106"));
    let pos = *world.objects.get_component::<crate::model::components::Position>(&7106).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (500, 600, 705), "recalled to GM position + 5 collision adjustment");
}

/// `//create_item 57 1000` puts 1000 adena in the GM's inventory.
#[test]
fn admin_create_item_adds_to_gm_inventory() {
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut gm_rx = ingame_player_access(&mut world, 1, 7201, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("create_item 57 1000"));
    assert_eq!(
        world.objects.get_component::<crate::model::inventory::Inventory>(&7201).unwrap().count_of(57),
        1000,
        "1000 adena created"
    );
}

/// `//create_item` with a bogus id answers "does not exist" and adds nothing.
#[test]
fn admin_create_item_rejects_unknown_id() {
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 7204, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("create_item 99999999 5"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "does-not-exist line");
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
        [vec![cop::DLG_ANSWER], dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0)].concat(),
    );
    assert!(!world.clients.contains_key(&2), "victim session removed after confirm");
    assert!(world.objects.get_component::<Player>(&7203).is_none(), "victim despawned");
}

/// `//add_exp_sp <exp> <sp>` grants exp and sp to the targeted player (driving
/// level-up). Java requires a player target, so the GM targets itself here.
#[test]
fn admin_add_exp_sp_grants_to_target() {
    let (mut world, ..) = admin_world();
    world.data.experience =
        crate::data::ExperienceData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 7301, 100);
    world.objects.add_components(&7301, crate::model::components::TargetRef(Some(7301)));
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
    world.data.root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/").to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7301, 100);
    world.objects.add_components(&7301, crate::model::components::TargetRef(Some(7301)));
    if let Some(p) = world.objects.get_component_mut::<Player>(&7301) {
        p.level = 20;
        p.exp = 123456;
        p.sp = 789;
    }
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("add_exp_sp_to_character"));
    let pkts = drain(&mut gm_rx);
    let html = pkts.iter().find_map(|p| decode_npc_html(p)).expect("expsp.htm sent as NpcHtmlMessage");
    assert!(html.contains("admin_add_exp_sp"), "the Add/Remove button bypasses are present");
    assert!(html.contains("123456"), "the player's xp is filled in");
    assert!(html.contains("789"), "the player's sp is filled in");
    assert!(!html.contains("%xp%") && !html.contains("%sp%"), "placeholders substituted");
}

/// `//add_exp_sp` with no player target is refused (Java `INVALID_TARGET`),
/// not silently applied to the GM.
#[test]
fn admin_add_exp_sp_without_target_is_invalid() {
    let (mut world, ..) = admin_world();
    world.data.experience =
        crate::data::ExperienceData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 7301, 100);
    let exp_before = world.objects.get_component::<Player>(&7301).unwrap().exp;
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("add_exp_sp 1000 500"));
    assert_eq!(world.objects.get_component::<Player>(&7301).unwrap().exp, exp_before, "no self-grant without a target");
    let pkts = drain(&mut gm_rx);
    assert!(
        pkts.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::INVALID_TARGET),
        "INVALID_TARGET sent",
    );
}

/// `//set_level N` sets the target's level; `//add_level N` adds to it.
#[test]
fn admin_set_and_add_level() {
    let (mut world, ..) = admin_world();
    world.data.experience =
        crate::data::ExperienceData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 7305, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("set_level 20"));
    assert_eq!(world.objects.get_component::<Player>(&7305).unwrap().level, 20, "set to 20");

    on_packet(&mut world, 1, build_admin("add_level 5"));
    assert_eq!(world.objects.get_component::<Player>(&7305).unwrap().level, 25, "added 5");
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
    assert!(drain(&mut gm1).iter().any(|p| p[0] == say), "sender GM sees it");
    assert!(drain(&mut gm2).iter().any(|p| p[0] == say), "other GM sees it");
    assert!(drain(&mut user).iter().all(|p| p[0] != say), "normal player does not");
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
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::SetAccessLevel { char_id: 7402, level: 70 })),
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
    assert_eq!(world.objects.get_component::<Player>(&7404).unwrap().access_level, 100, "unchanged");
}

/// `//gm` deactivates the caller's own GM access for the session (not persisted).
#[test]
fn admin_gm_deactivates_own_access() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7403, 100);
    drain(&mut gm_rx);
    assert!(world.objects.get_component::<Player>(&7403).unwrap().is_gm(&world.data));

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
    assert_eq!(count_system_messages(&drain(&mut u1)), 1, "player 1 got the announce");
    assert_eq!(count_system_messages(&drain(&mut u2)), 1, "player 2 got the announce");
}

/// `//character_disconnect` disconnects the targeted player.
#[test]
fn admin_character_disconnect_kicks_target() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7504, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7505, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    world.objects.add_components(&7504, crate::model::components::TargetRef(Some(7505)));
    on_packet(&mut world, 1, build_admin("character_disconnect"));
    assert!(!world.clients.contains_key(&2), "victim disconnected");
    assert!(world.objects.get_component::<Player>(&7505).is_none(), "victim despawned");
}

/// `//delete` despawns the targeted NPC and broadcasts DeleteObject.
#[test]
fn admin_delete_despawns_targeted_npc() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7601, 100);
    drain(&mut gm_rx);

    let npc_oid = crate::model::npc::FIRST_NPC_OBJECT_ID + 1;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 1, 2, 3, 100, 50);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    world.objects.add_components(&7601, crate::model::components::TargetRef(Some(npc_oid)));

    on_packet(&mut world, 1, build_admin("delete"));
    assert!(
        world.objects.get_component::<crate::model::npc::Npc>(&npc_oid).is_none(),
        "npc despawned by //delete"
    );
    assert!(
        drain(&mut gm_rx).iter().any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT),
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
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "select-an-npc line");
}

/// `//spawn` with an unknown NPC id is refused.
#[test]
fn admin_spawn_rejects_unknown_npc() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7602, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("spawn 99999"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "does-not-exist line");
}

/// `//spawn <npcId>` creates the NPC at the GM's location and shows it to them.
#[test]
fn admin_spawn_creates_npc_at_gm() {
    let (mut world, ..) = admin_world();
    world.data.npc_data =
        crate::data::NpcData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 7604, 100);
    drain(&mut gm_rx);
    if let Some(p) = world.objects.get_component_mut::<crate::model::components::Position>(&7604) {
        p.x = 100;
        p.y = 200;
        p.z = 300;
    }

    let npc_oid = world.next_npc_object_id;
    on_packet(&mut world, 1, build_admin("spawn 30001")); // Lector, a Merchant (non-monster)
    assert_eq!(world.next_npc_object_id, npc_oid + 1, "one NPC spawned");
    let npc = world.objects.get_component::<crate::model::npc::Npc>(&npc_oid).expect("npc entity exists");
    assert_eq!(npc.npc_id, 30001);
    let pos = world.objects.get_component::<crate::model::components::Position>(&npc_oid).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (100, 200, 300), "spawned at the GM");
    assert!(
        drain(&mut gm_rx).iter().any(|p| p[0] == server_packets::opcodes::NPC_INFO),
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
        world.objects.get_component::<crate::model::components::TargetRef>(&7701).and_then(|t| t.0),
        Some(7702),
        "GM now targets the named player"
    );
    assert!(
        drain(&mut gm_rx).iter().any(|p| p[0] == server_packets::opcodes::MY_TARGET_SELECTED),
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
    assert!(world.objects.get_component::<crate::model::components::AdminFlags>(&7801).unwrap().invul);

    let hp_before = pvit(&world, 7801).cur_hp;
    super::combat::player_receive_damage(&mut world, 7801, 12345, 50.0);
    assert_eq!(pvit(&world, 7801).cur_hp, hp_before, "invul: no damage taken");

    // Toggle off → damage lands.
    on_packet(&mut world, 1, build_admin("invul"));
    super::combat::player_receive_damage(&mut world, 7801, 12345, 50.0);
    assert!(pvit(&world, 7801).cur_hp < hp_before, "damage applies once invul is off");
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

    world.objects.add_components(&7803, crate::model::components::TargetRef(Some(7804)));
    on_packet(&mut world, 1, build_admin("setinvul"));
    assert!(world.objects.get_component::<crate::model::components::AdminFlags>(&7804).unwrap().invul);
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
    assert!(world.objects.get_component::<crate::model::components::AdminFlags>(&7901).unwrap().hidden);
    assert!(
        drain(&mut obs_rx).iter().any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT
            && i32::from_le_bytes([p[1], p[2], p[3], p[4]]) == 7901),
        "observer got DeleteObject for the hidden GM"
    );

    on_packet(&mut world, 1, build_admin("hide"));
    assert!(!world.objects.get_component::<crate::model::components::AdminFlags>(&7901).unwrap().hidden);
    assert!(
        drain(&mut obs_rx).iter().any(|p| p[0] == server_packets::opcodes::CHAR_INFO),
        "observer got CharInfo when the GM reappeared"
    );
}

/// `//add_skill <id> <lvl>` puts the skill in the target's book and refreshes
/// their SkillList; `//remove_skill` takes it back out.
#[test]
fn admin_add_and_remove_skill() {
    let (mut world, ..) = admin_world();
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8001, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("add_skill 1177 1"));
    assert_eq!(
        world.objects.get_component::<crate::model::components::SkillBook>(&8001).unwrap().0.get(&1177),
        Some(&1),
        "skill added to the book"
    );
    assert!(drain(&mut gm_rx).iter().any(|p| p[0] == 0x5F), "SkillList refresh sent");

    on_packet(&mut world, 1, build_admin("remove_skill 1177"));
    assert!(
        !world.objects.get_component::<crate::model::components::SkillBook>(&8001).unwrap().0.contains_key(&1177),
        "skill removed"
    );
}

/// `//add_skill` with an unknown id is refused.
#[test]
fn admin_add_skill_rejects_unknown() {
    let (mut world, ..) = admin_world();
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8002, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("add_skill 99999999 1"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "does-not-exist line");
}

/// `//setew <n>` sets the enchant level of the equipped weapon.
#[test]
fn admin_setew_enchants_equipped_weapon() {
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
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
    world.objects.add_components(&8101, crate::model::inventory::Inventory::from_rows(&[weapon]));

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
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "no-item-in-slot line");
}

/// `//buff <id>` applies the skill's effects (a buff) to the GM.
#[test]
fn admin_buff_applies_skill_to_self() {
    let (mut world, ..) = admin_world();
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
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
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8202, 100);
    drain(&mut gm_rx);

    let base_spd = world.objects.get_component::<Speeds>(&8202).unwrap().run_spd;
    on_packet(&mut world, 1, build_admin("superhaste 2"));

    // The buff is present, permanent, and raised run speed.
    let buff = world.objects.get_component::<Buffs>(&8202).unwrap().0.iter().find(|b| b.skill_id == 7029).cloned();
    let buff = buff.expect("super-haste buff applied");
    assert_eq!(buff.expires_at_tick, u64::MAX, "toggle buff is permanent");
    assert!(world.objects.get_component::<Speeds>(&8202).unwrap().run_spd > base_spd, "run speed increased");

    // No BuffExpire was scheduled, so advancing the world keeps it.
    world.tick += 100;
    crate::game_loop::apply_due_tasks(&mut world);
    assert!(world.objects.get_component::<Buffs>(&8202).unwrap().0.iter().any(|b| b.skill_id == 7029), "still active after ticks");

    // //superhaste 0 clears it.
    on_packet(&mut world, 1, build_admin("superhaste 0"));
    assert!(!world.objects.get_component::<Buffs>(&8202).unwrap().0.iter().any(|b| b.skill_id == 7029), "cleared by level 0");
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
        running: true,
        swimming: false,
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
    let mut c = CombatStats { p_atk_spd: 300, ..Default::default() };
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
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8202, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("buff 99999999 1"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "does-not-exist line");
}

/// The `//editchar` field setters mutate the targeted player and broadcast.
#[test]
fn admin_editchar_field_setters() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8301, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 8302, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);
    world.objects.add_components(&8301, crate::model::components::TargetRef(Some(8302)));

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
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.data.root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/").to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8401, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("buff 1068 1")); // Might
    drain(&mut gm_rx);
    // Java `showBuffs` renders the `getbuffs.htm` window with a per-buff row +
    // an `X` cancel button (not sysmessage lines).
    on_packet(&mut world, 1, build_admin("getbuffs"));
    let html = drain(&mut gm_rx).iter().find_map(|p| decode_npc_html(p)).expect("getbuffs html");
    assert!(!html.contains("My text is missing"), "getbuffs.htm found");
    assert!(html.contains("admin_stopbuff 8401 1068"), "buff row carries a cancel button");
}

/// `//stopbuff <id>` removes that one buff.
#[test]
fn admin_stopbuff_removes_one() {
    let (mut world, ..) = admin_world();
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
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
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
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
        [vec![cop::DLG_ANSWER], dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0)].concat(),
    );
    assert_eq!(pbuffs(&world, 8502), 0, "all buffs cleared after confirm");
}

/// `//setclass <id>` changes the target's class and recomputes their template.
#[test]
fn admin_setclass_changes_class() {
    let (mut world, ..) = admin_world();
    world.data.player_templates =
        crate::data::PlayerTemplateData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8701, 100);
    drain(&mut gm_rx);
    assert_eq!(world.objects.get_component::<Player>(&8701).unwrap().class_id, 0);

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
    const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (mut world, ..) = admin_world();
    world.data.player_templates = crate::data::PlayerTemplateData::load_from(ROOT);
    world.data.skill_trees = crate::data::SkillTreeData::load_from(ROOT);
    world.cfg.character.auto_learn_skills = true;
    let mut gm_rx = ingame_player_access(&mut world, 1, 8703, 100);
    drain(&mut gm_rx);
    if let Some(p) = world.objects.get_component_mut::<Player>(&8703) {
        p.level = 40; // Warlord's 2nd-class skills gate at getLevel 40.
    }

    on_packet(&mut world, 1, build_admin("setclass 3")); // Warlord (2nd class)

    let p = world.objects.get_component::<Player>(&8703).unwrap();
    assert_eq!(p.class_id, 3);
    let book = world.objects.get_component::<crate::model::components::SkillBook>(&8703).unwrap();
    assert!(book.0.contains_key(&36), "gained Warlord's Whirlwind (36)");
    assert!(book.0.contains_key(&239), "gained common Expertise (239) via the complete tree");
}

/// `//setclass` with an unknown class id is refused.
#[test]
fn admin_setclass_rejects_unknown() {
    let (mut world, ..) = admin_world();
    world.data.player_templates =
        crate::data::PlayerTemplateData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8702, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("setclass 99999"));
    assert_eq!(world.objects.get_component::<Player>(&8702).unwrap().class_id, 0, "unchanged");
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
    assert_eq!(i32::from_le_bytes(social[1..5].try_into().unwrap()), 8801, "on the GM");
    assert_eq!(i32::from_le_bytes(social[5..9].try_into().unwrap()), 3, "action id 3");
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
        !pkts.iter().any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION),
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
    let pos = *world.objects.get_component::<crate::model::components::Position>(&8803).unwrap();
    if let Some(p) = world.objects.get_component_mut::<crate::model::components::Position>(&8804) {
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
        drain(&mut gm_rx).iter().any(|p| p[0] == server_packets::opcodes::EARTHQUAKE),
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
        drain(&mut other_rx).iter().any(|p| p[0] == server_packets::opcodes::SUN_RISE),
        "SunRise reached an unrelated online player"
    );
}

/// `//play_sound <name>` plays the sound and confirms to the GM.
#[test]
fn admin_play_sound_plays() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8808, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("play_sound ItemSound.quest_middle"));
    let pkts = drain(&mut gm_rx);
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::PLAY_SOUND), "PlaySound sent");
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
    assert_eq!(i32::from_le_bytes(msu[5..9].try_into().unwrap()), 8809, "GM is the animation source");
}

/// `//remove_exp_sp <exp> <sp>` subtracts from the targeted player (the GM
/// targets itself, matching Java's required player target).
#[test]
fn admin_remove_exp_sp_reduces() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8901, 100);
    world.objects.add_components(&8901, crate::model::components::TargetRef(Some(8901)));
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
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8902, 100);
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin("setskill 1177 1"));
    assert_eq!(
        world.objects.get_component::<SkillBook>(&8902).unwrap().0.get(&1177),
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
    world.objects.add_components(&8903, crate::model::components::TargetRef(Some(8904)));
    on_packet(&mut world, 1, build_admin("changename Renamed"));
    assert_eq!(world.objects.get_component::<Player>(&8904).unwrap().name, "Renamed");
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
    world.objects.add_components(&8907, crate::model::components::TargetRef(Some(8908)));
    on_packet(&mut world, 1, build_admin("set_vitality 5000"));
    assert_eq!(world.objects.get_component::<Player>(&8908).unwrap().vitality_points, 5000);
    on_packet(&mut world, 1, build_admin("full_vitality"));
    assert_eq!(world.objects.get_component::<Player>(&8908).unwrap().vitality_points, 140_000, "clamped to max");
}

/// `//gonorth <offset>` moves the GM north (-y) by the offset.
#[test]
fn admin_gonorth_moves_gm() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8909, 100);
    drain(&mut gm_rx);
    let y0 = world.objects.get_component::<Position>(&8909).unwrap().y;
    on_packet(&mut world, 1, build_admin("gonorth 200"));
    assert_eq!(world.objects.get_component::<Position>(&8909).unwrap().y, y0 - 200);
}

/// `//geo_pos` with no geodata loaded answers the "no geodata" line (does not
/// crash on the empty geo engine).
#[test]
fn admin_geo_pos_no_geodata() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8910, 100);
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin("geo_pos"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "one geo status line");
}

/// `//create_coin adena <n>` gives adena (item 57) to the GM.
#[test]
fn admin_create_coin_gives_adena() {
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut gm_rx = ingame_player_access(&mut world, 1, 8911, 100);
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin("create_coin adena 100"));
    let inv = world.objects.get_component::<crate::model::inventory::Inventory>(&8911).unwrap();
    assert_eq!(inv.count_of(57), 100, "adena added");
}

/// `//spawnat <id> <x> <y> <z>` spawns an NPC at explicit coordinates.
#[test]
fn admin_spawnat_creates_npc_at_coords() {
    let (mut world, ..) = admin_world();
    world.data.npc_data =
        crate::data::NpcData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8912, 100);
    drain(&mut gm_rx);
    let npc_oid = world.next_npc_object_id;
    on_packet(&mut world, 1, build_admin("spawnat 30001 -84000 244000 -3700"));
    assert_eq!(world.next_npc_object_id, npc_oid + 1, "one NPC spawned");
    let pos = world.objects.get_component::<crate::model::components::Position>(&npc_oid).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (-84000, 244000, -3700), "spawned at the coords");
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
        drain(&mut gm_rx).iter().any(|pk| pk[0] == server_packets::opcodes::RIDE),
        "Ride broadcast sent"
    );

    // Re-riding while mounted is refused (Java "already have a summon").
    on_packet(&mut world, 1, build_admin("ride_wolf"));
    assert_eq!(world.objects.get_component::<Player>(&8920).unwrap().mount_type, 1, "still on the strider");

    on_packet(&mut world, 1, build_admin("unride"));
    assert_eq!(world.objects.get_component::<Player>(&8920).unwrap().mount_type, 0, "dismounted");
}

/// `//ride_bike` transforms the GM (transform 20001): durable transform id +
/// display id, the run speed overridden to the template's, and the transform's
/// skills granted; `//unride` reverts all of it.
#[test]
fn admin_ride_bike_transforms_and_reverts() {
    let (mut world, ..) = admin_world();
    world.data.transforms =
        crate::data::TransformData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    // Jet bike (20001) exists in the dist with run=170 + a Dismount skill.
    let bike = world.data.transforms.get(20001).expect("jet bike transform loaded");
    let bike_run = bike.template(false).run_spd.expect("bike has a run speed");
    let bike_skill = bike.template(false).skills.first().map(|(id, _)| *id).expect("bike grants a skill");

    let mut gm_rx = ingame_player_access(&mut world, 1, 8930, 100);
    drain(&mut gm_rx);
    let base_run = world.objects.get_component::<Speeds>(&8930).unwrap().run_spd;

    on_packet(&mut world, 1, build_admin("ride_bike"));
    {
        let p = world.objects.get_component::<Player>(&8930).unwrap();
        assert_eq!(p.transform_id, 20001, "transformed into the bike");
        assert_eq!(p.transform_display_id, 20001, "display id == id on this dist");
    }
    assert_eq!(world.objects.get_component::<Speeds>(&8930).unwrap().run_spd, bike_run, "run speed overridden by the transform");
    assert!(world.objects.get_component::<SkillBook>(&8930).unwrap().0.contains_key(&bike_skill), "transform skill granted");

    // Re-transforming while transformed is refused (Java polymorph message).
    on_packet(&mut world, 1, build_admin("ride_horse"));
    assert_eq!(world.objects.get_component::<Player>(&8930).unwrap().transform_id, 20001, "still the bike");

    on_packet(&mut world, 1, build_admin("unride"));
    let p = world.objects.get_component::<Player>(&8930).unwrap();
    assert_eq!(p.transform_id, 0, "reverted");
    assert_eq!(p.transform_display_id, 0, "display cleared");
    assert_eq!(world.objects.get_component::<Speeds>(&8930).unwrap().run_spd, base_run, "run speed restored");
    assert!(!world.objects.get_component::<SkillBook>(&8930).unwrap().0.contains_key(&bike_skill), "transform skill removed");
}

/// `//mobgroup` lifecycle: create → spawn (members tagged Controllable) →
/// set a state → invul → kill → remove.
#[test]
fn admin_mobgroup_lifecycle() {
    let (mut world, ..) = admin_world();
    world.data.npc_data =
        crate::data::NpcData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8940, 100);
    drain(&mut gm_rx);

    // create (no spawn yet)
    on_packet(&mut world, 1, build_admin("mobgroup_create 1 20001 3"));
    assert_eq!(world.mob_groups.get(&1).map(|g| g.max_count), Some(3), "group registered");
    assert!(world.mob_groups.get(&1).unwrap().members.is_empty(), "not spawned yet");

    // spawn at the GM → 3 Controllable NPCs
    on_packet(&mut world, 1, build_admin("mobgroup_spawn 1"));
    let members: Vec<i32> = world.mob_groups.get(&1).unwrap().members.clone();
    assert_eq!(members.len(), 3, "three mobs spawned");
    for &m in &members {
        assert_eq!(
            world.objects.get_component::<crate::model::mob_group::Controllable>(&m).map(|c| c.group_id),
            Some(1),
            "member tagged Controllable"
        );
    }

    // state: follow the GM
    on_packet(&mut world, 1, build_admin("mobgroup_follow 1"));
    assert!(matches!(world.mob_groups.get(&1).unwrap().state, crate::model::mob_group::MobGroupState::Follow(8940)));

    // invul on → each member gets the invul flag
    on_packet(&mut world, 1, build_admin("mobgroup_invul 1 on"));
    assert!(world.mob_groups.get(&1).unwrap().invul, "group invul set");
    assert!(world.objects.get_component::<AdminFlags>(&members[0]).is_some_and(|f| f.invul), "member invul");

    // kill → members become corpses (dead)
    on_packet(&mut world, 1, build_admin("mobgroup_kill 1"));
    assert!(
        members.iter().all(|m| world.objects.get_component::<Vitals>(m).is_some_and(|v| v.dead)),
        "all members killed"
    );

    // remove → group gone, members despawned
    on_packet(&mut world, 1, build_admin("mobgroup_remove 1"));
    assert!(!world.mob_groups.contains_key(&1), "group removed");
    assert!(members.iter().all(|m| !world.objects.has_component::<crate::model::npc::Npc>(m)), "members despawned");
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
    world.objects.add_components(&8950, crate::model::components::TargetRef(Some(8951)));
    let base_p_atk = world.objects.get_component::<CombatStats>(&8951).unwrap().p_atk;

    on_packet(&mut world, 1, build_admin("setparam pAtk 9999"));
    assert_eq!(world.objects.get_component::<CombatStats>(&8951).unwrap().p_atk, 9999.0, "P.Atk fixed");
    assert_eq!(
        world.objects.get_component::<crate::model::components::StatModifiers>(&8951).unwrap().fixed.get(&crate::model::stats::Stat::PhysicalAttack),
        Some(&9999.0)
    );

    on_packet(&mut world, 1, build_admin("unsetparam pAtk"));
    assert_eq!(world.objects.get_component::<CombatStats>(&8951).unwrap().p_atk, base_p_atk, "P.Atk restored");

    // An unknown stat name is rejected without touching the overrides.
    on_packet(&mut world, 1, build_admin("setparam bogus 5"));
    assert!(world.objects.get_component::<crate::model::components::StatModifiers>(&8951).unwrap().fixed.is_empty());
}

/// `//sethero` toggles hero status on the target: grants/removes the hero skill
/// tree and flips the aura; `//givehero` can't claim without an Olympiad-crowned
/// hero list. Port of AdminAdmin's hero commands.
#[test]
fn admin_sethero_toggles_status_skills_and_aura() {
    const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (mut world, ..) = admin_world();
    world.data.root = ROOT.to_string();
    world.data.skill_trees = crate::data::SkillTreeData::load_from(ROOT);
    let mut rx = ingame_player_access(&mut world, 1, 7301, 100);
    drain(&mut rx);
    // Target self (a player) so sethero applies to the GM.
    world.objects.add_components(&7301, TargetRef(Some(7301)));
    assert!(!world.data.skill_trees.hero_skills().is_empty(), "hero skill tree loaded from XML");

    // //sethero → hero on: flag, aura, and the hero skills granted.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("sethero")].concat());
    let p = world.objects.get_component::<Player>(&7301).unwrap();
    assert!(p.is_hero && p.hero_aura, "hero status + aura on");
    assert!(world.objects.get_component::<SkillBook>(&7301).unwrap().0.contains_key(&395), "Heroic Miracle granted");

    // //sethero again → hero off, skills removed.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("sethero")].concat());
    let p = world.objects.get_component::<Player>(&7301).unwrap();
    assert!(!p.is_hero, "hero status off");
    assert!(!world.objects.get_component::<SkillBook>(&7301).unwrap().0.contains_key(&395), "hero skill removed");

    // //givehero (confirmDlg) → yes → cannot claim (no Olympiad hero list).
    drain(&mut rx);
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("givehero")].concat());
    on_packet(&mut world, 1, [vec![cop::DLG_ANSWER], dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0)].concat());
    let msgs: Vec<String> = drain(&mut rx).iter().filter_map(|p| sysmsg_text(p)).collect();
    assert!(msgs.iter().any(|t| t.contains("cannot claim the hero status")), "givehero blocked without a crowned hero");
}

/// `//castlemanage` shows a castle's page; `setOwner` assigns the targeted
/// clanned player's clan + side, `switchSide` flips it, `takeCastle` strips it;
/// siege actions report unavailable. Port of AdminCastle.
#[test]
fn admin_castlemanage_ownership_and_side() {
    use crate::model::castle::{Castle, CastleSide};
    use crate::model::clan::{Clan, ClanMember};
    const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (mut world, _db_tx, mut db_rx, _link) = admin_world();
    world.data.root = ROOT.to_string();
    world.castles = vec![Castle { id: 3, name: "Giran".into(), side: CastleSide::Neutral }];
    world.clans.insert(
        500,
        Clan {
            id: 500,
            name: "Owners".into(),
            leader_id: 8002,
            level: 5,
            reputation_score: 0,
            castle_id: 0,
            members: vec![ClanMember { char_id: 8002, name: "P8002".into(), level: 40, class_id: 0, sex: 0, race: 0 }],
            warehouse: Default::default(),
        },
    );
    let mut rx = ingame_player_access(&mut world, 1, 8001, 100);
    let _t = ingame_player_access(&mut world, 2, 8002, 0);
    world.objects.get_component_mut::<Player>(&8002).unwrap().clan_id = 500;
    world.objects.add_components(&8001, TargetRef(Some(8002)));
    drain(&mut rx);

    // //castlemanage 3 → the page, unowned + neutral.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("castlemanage 3")].concat());
    let page = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("castle page");
    assert!(page.contains("Giran") && page.contains("NPC"), "unowned castle shows NPC");

    // //castlemanage 3 setOwner LIGHT → clan 500 owns Giran on the light side.
    drain_db(&mut db_rx);
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("castlemanage 3 setOwner LIGHT")].concat());
    let page = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("castle page");
    assert_eq!(world.castles[0].side, CastleSide::Light, "side set");
    assert_eq!(world.clans[&500].castle_id, 3, "clan owns the castle");
    assert!(page.contains("Owners") && page.contains("Light"), "owner + side displayed");
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::UpdateClanCastle { clan_id: 500, castle_id: 3 })), "persisted");

    // //castlemanage 3 switchSide → Dark.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("castlemanage 3 switchSide")].concat());
    drain(&mut rx);
    assert_eq!(world.castles[0].side, CastleSide::Dark, "side switched");

    // //castlemanage 3 takeCastle → unowned again.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("castlemanage 3 takeCastle")].concat());
    drain(&mut rx);
    assert_eq!(world.clans[&500].castle_id, 0, "ownership removed");

    // //castlemanage 3 startSiege → no attackers registered.
    world.sieges.insert(3, crate::model::siege::Siege::new(3));
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("castlemanage 3 startSiege")].concat());
    let msgs: Vec<String> = drain(&mut rx).iter().filter_map(|p| sysmsg_text(p)).collect();
    assert!(msgs.iter().any(|t| t.contains("not registered any clan")), "siege needs an attacker");
}

/// The `//castlemanage <id>` siege actions: register/remove attackers &
/// defenders (`siege_clans`), and the start/stop state transition. Port of
/// AdminCastle's siege branch over the model/siege slice.
#[test]
fn admin_castlemanage_siege_registration_and_state() {
    use crate::model::castle::{Castle, CastleSide};
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::siege::Siege;
    const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (mut world, _db_tx, mut db_rx, _link) = admin_world();
    world.data.root = ROOT.to_string();
    world.castles = vec![Castle { id: 3, name: "Giran".into(), side: CastleSide::Neutral }];
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
            members: vec![ClanMember { char_id: 8102, name: "P8102".into(), level: 40, class_id: 0, sex: 0, race: 0 }],
            warehouse: Default::default(),
        },
    );
    let mut rx = ingame_player_access(&mut world, 1, 8101, 100);
    let _t = ingame_player_access(&mut world, 2, 8102, 0);
    world.objects.get_component_mut::<Player>(&8102).unwrap().clan_id = 700;
    world.objects.add_components(&8101, TargetRef(Some(8102)));
    drain(&mut rx);

    // addAttacker → clan 700 registered + persisted.
    drain_db(&mut db_rx);
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("castlemanage 3 addAttacker")].concat());
    assert!(world.sieges[&3].has_attackers() && world.sieges[&3].is_registered(700), "attacker registered");
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::SaveSiegeClan { castle_id: 3, clan_id: 700, kind: 1 })), "persisted attacker");

    // addAttacker again → already requested.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("castlemanage 3 addAttacker")].concat());
    assert!(
        sm_ids_of(&drain(&mut rx)).contains(&server_packets::sm_ids::YOU_HAVE_ALREADY_REQUESTED_A_CASTLE_SIEGE),
        "duplicate registration refused"
    );

    // startSiege → in progress + "siege has started" announced to everyone.
    drain(&mut rx);
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("castlemanage 3 startSiege")].concat());
    assert!(world.sieges[&3].in_progress, "siege started");
    assert!(sm_ids_of(&drain(&mut rx)).contains(&server_packets::sm_ids::THE_S1_SIEGE_HAS_STARTED), "start announced");
    // stopSiege → ended + "siege has finished".
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("castlemanage 3 stopSiege")].concat());
    assert!(!world.sieges[&3].in_progress, "siege stopped");
    assert!(sm_ids_of(&drain(&mut rx)).contains(&server_packets::sm_ids::THE_S1_SIEGE_HAS_FINISHED), "end announced");

    // Re-start, then let the scheduled auto-end fire (Siege.ScheduleEndSiegeTask).
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("castlemanage 3 startSiege")].concat());
    assert!(world.sieges[&3].in_progress);
    drain(&mut rx);
    world.tick += 120 * 60 * 10 + 1; // past the 120-minute window (100 ms ticks)
    apply_due_tasks(&mut world);
    assert!(!world.sieges[&3].in_progress, "auto-ended after the siege window");
    assert!(sm_ids_of(&drain(&mut rx)).contains(&server_packets::sm_ids::THE_S1_SIEGE_HAS_FINISHED), "auto-end announced");

    // removeDeffender strips the target's clan (Java quirk) + persists.
    drain(&mut rx);
    drain_db(&mut db_rx);
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("castlemanage 3 removeDeffender")].concat());
    assert!(!world.sieges[&3].is_registered(700), "clan removed from the siege");
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::RemoveSiegeClan { castle_id: 3, clan_id: 700 })), "persisted removal");
}
