//! `admin/guard.rs`, `admin/menu.rs`, `admin/moderation.rs`,
//! `admin/premium.rs` — who may run a command, the confirm dialog, the admin
//! menu, and the punishment console.

use super::*;

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

    // In AdminCommands.xml (admin_fakechat, level 100) but no body: it drives
    // the FakePlayers subsystem, which this dist disables outright → the
    // not-implemented path, which must answer rather than crash. (The command
    // has to be one **without** `confirmDlg`, or the reply is a ConfirmDlg
    // rather than a system message.)
    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("fakechat")].concat(),
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
    assert_eq!(user.player.name_color, model::DEFAULT_NAME_COLOR);
    assert_eq!(user.player.title_color, model::DEFAULT_TITLE_COLOR);
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
        let view = model::PlayerView::of(&world.objects, oid).unwrap();
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

/// `//kill` on a targeted player kills them (Java `doDie` path).
#[test]
fn admin_kill_slays_targeted_player() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7003, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7004, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    world.objects.add_components(&7003, TargetRef(Some(7004)));
    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("kill")].concat(),
    );

    assert!(pvit(&world, 7004).dead, "victim is dead after //kill");
}

/// **`//kill` on a monster has to pay out.** Java's `AdminKill.kill` deals
/// `maxHp + 1` *as damage* (`reduceCurrentHp(…, activeChar, null)`), so the GM
/// lands in the victim's aggro list and the reward split — which reads exactly
/// that list — finds a damage dealer. The port used to call the death path
/// straight, so a GM killing a mob with a full drop table got no exp and no
/// loot, which is the one thing `//kill` is used to check.
#[test]
fn admin_kill_on_a_monster_awards_its_drops() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7103, 100);
    drain(&mut gm_rx);

    // `AutoLoot` is on in this dist, which is what puts the drop straight in
    // the killer's inventory instead of on the ground — and the loot needs the
    // real item catalogue to become an inventory row.
    world.cfg.character.auto_loot = true;
    world.data.item_data = dist::items_owned();
    // Auto-loot mints an item instance, which needs object ids to hand out.
    world.id_pool = 0x4300_0000..0x4300_0100;

    // A monster whose whole drop list is one guaranteed line, at the GM's feet.
    let npc_oid = NPC_OID + 41;
    let npc_id = 90101;
    let mut template = crate::data::npc_data::default_template(npc_id);
    template.type_name = "Monster".into();
    template.level = 1;
    template.exp = 100.0;
    template.sp = 10.0;
    template.drop_list_death = vec![crate::data::npc_data::DropHolder {
        item_id: 57,
        min: 100,
        max: 100,
        chance: 100.0,
    }];
    world.data.npc_data.insert_for_test(template);
    // At the GM's feet: the reward split drops any dealer outside
    // `RewardRange` of the corpse, so the fixture has to stand next to it.
    let gm_pos = *world
        .objects
        .get_component::<crate::model::components::space::Position>(&7103)
        .expect("gm position");
    add_test_npc(
        &mut world, npc_oid, npc_id, "Monster", 1, gm_pos.x, gm_pos.y, gm_pos.z,
    );

    // `dummy_char` spawns with an empty HP bar, and a dead earner is skipped
    // by the reward loop (Java's `if (!attacker.isDead())`).
    {
        let v = world
            .objects
            .get_component_mut::<Vitals>(&7103)
            .expect("gm vitals");
        v.max_hp = 1000;
        v.cur_hp = 1000.0;
        v.dead = false;
    }
    let before = world
        .objects
        .get_component::<Player>(&7103)
        .expect("gm")
        .exp;
    world
        .objects
        .add_components(&7103, TargetRef(Some(npc_oid)));
    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("kill")].concat(),
    );

    // Death consequences run on the tick after the blow.
    advance_world(&mut world, 2);

    assert!(nvit(&world, npc_oid).dead, "the monster died");
    // The GM landed in the aggro list, which is the whole mechanism: the
    // reward split reads it, and reads nothing else.
    let damage_dealt = world
        .objects
        .get_component::<crate::model::npc::AggroList>(&npc_oid)
        .and_then(|a| a.0.get(&7103).map(|info| info.damage))
        .unwrap_or(0.0);
    assert!(
        damage_dealt > 0.0,
        "the admin kill registered as damage from the GM"
    );
    let _ = before;
    let adena = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&7103)
        .expect("inventory")
        .count_of(57);
    assert!(
        adena >= 100,
        "the guaranteed adena line was auto-looted ({adena})"
    );
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

/// `//add_exp_sp_to_character` opens the real `expsp.htm` window
/// (`NpcHtmlMessage`) for the targeted player with its level/xp/sp filled in —
/// not chat text — matching Java's `addExpSp`.
#[test]
fn admin_add_exp_sp_to_character_opens_menu() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7301, 100);
    world.objects.add_components(&7301, TargetRef(Some(7301)));
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

/// **The AdminPunishment console round-trips.** `//punishment` renders with
/// the type/affect combos filled; `//punishment_add` starts a real punishment
/// through the generic engine (a jail actually confines); `//punishment info`
/// lists it; `//punishment_remove` lifts it.
#[test]
fn punishment_console_add_info_remove() {
    use model::punishment::{PunishmentAffect, PunishmentType};
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
            .any(|p| p[0] == server_packets::opcodes::QUEST_LIST),
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
            .get_component::<model::components::social::Quests>(&7822)
            .unwrap();
        let st = q.0.get("Q00101_SwordOfSolidarity").expect("state created");
        assert_eq!(st.vars.get("cond").map(String::as_str), Some("3"));
        assert_eq!(st.state, model::quest::state::STARTED);
    }
    drain(&mut gm_rx);
    // Java's menu is three pages deep, and the bare command lands on the first:
    // buttons for CREATED/STARTED/COMPLETED/All, no quest list yet.
    on_packet(&mut world, 1, build_admin(&format!("charquestmenu {name}")));
    let html = drain(&mut gm_rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("quest panel served");
    assert!(
        html.contains("Quest Menu for") && html.contains("admin_charquestmenu"),
        "the landing menu, got: {html}"
    );

    // "All" (`3`) lists the quests, each linking to its own editor.
    on_packet(
        &mut world,
        1,
        build_admin(&format!("charquestmenu {name} 3")),
    );
    let html = drain(&mut gm_rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("quest list served");
    assert!(
        html.contains("Full Quest List") && html.contains("Q00101_SwordOfSolidarity"),
        "the quest is listed, got: {html}"
    );

    // The quest's own page carries its state and every var with Set/Del.
    on_packet(
        &mut world,
        1,
        build_admin(&format!("charquestmenu {name} Q00101_SwordOfSolidarity")),
    );
    let html = drain(&mut gm_rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("quest editor served");
    assert!(
        html.contains("State: <font color=\"LEVEL\">STARTED")
            && html.contains("<td>cond</td><td>3</td>")
            && html.contains("admin_setcharquest")
            && html.contains("Quest Complete"),
        "the editor shows state, vars and the action buttons, got: {html}"
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
            .get_component::<model::components::social::Quests>(&7822)
            .unwrap()
            .0
            .contains_key("Q00101_SwordOfSolidarity"),
        "state removed"
    );
}

/// **`//exceptions`/`//set_exception` toggle cond-override bits, and
/// SEE_ALL_PLAYERS lets its holder be described a hidden GM.**
///
/// The watcher starts by *disabling* everything. That step used to be
/// unnecessary — the port gave every character 0 overrides at load — but Java
/// defaults a **GM** to `getAllExceptionsMask()` in `Player.restore`, and the
/// port now does too, so an access-100 character already holds SEE_ALL_PLAYERS
/// before anything is toggled. Testing a toggle means starting from a known
/// state, and for a GM that state has to be set rather than assumed.
#[test]
fn cond_overrides_and_see_all_players() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7911, 100);
    let mut watcher_rx = ingame_player_access(&mut world, 2, 7912, 100);
    assert!(
        world
            .objects
            .get_component::<Player>(&7912)
            .unwrap()
            .can_override_cond(13),
        "a GM logs in overriding everything (Java's `Player.restore` default)"
    );
    on_packet(&mut world, 2, build_admin("set_exception disable_all"));
    // Admin commands land behind a confirm dialog, same as the enable below.
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
        !world
            .objects
            .get_component::<Player>(&7912)
            .unwrap()
            .can_override_cond(13),
        "…and `disable_all` clears the lot"
    );
    drain(&mut gm_rx);
    drain(&mut watcher_rx);

    // GM 7911 hides; watcher 7912 (no override) re-enters — no CharInfo.
    on_packet(&mut world, 1, build_admin("hide"));
    drain(&mut watcher_rx);
    visibility::on_enter_world(&world, 2, 7912);
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
    visibility::on_enter_world(&world, 2, 7912);
    assert!(
        drain(&mut watcher_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::CHAR_INFO),
        "SEE_ALL_PLAYERS holder is described the hidden GM"
    );
}

/// **The premium commands fall back to the target** (GitHub #5). Java takes an
/// account name and nothing else; with a character selected and no argument —
/// which is what the menu's own buttons send — the target's account is used.
#[test]
fn premium_commands_use_the_target_when_no_account_is_given() {
    let (mut world, ..) = admin_world();
    world.cfg.premium.enabled = true;
    let mut gm_rx = ingame_player_access(&mut world, 1, 7403, 100);
    let _victim = ingame_player_access(&mut world, 2, 7404, 0);
    drain(&mut gm_rx);
    let account = world
        .objects
        .get_component::<Player>(&7404)
        .expect("victim")
        .account
        .clone();
    world.objects.add_components(&7403, TargetRef(Some(7404)));

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("premium_add1"),
        ]
        .concat(),
    );
    // `admin_premium_add1` is `confirmDlg="true"` in AdminCommands.xml, so the
    // command is held until the GM answers the dialog.
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
        crate::game_loop::admin::premium::has_premium_status(&world, 7404),
        "the targeted character's account ({account}) got the premium month"
    );
}
