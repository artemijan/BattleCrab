//! `admin/character.rs`, `admin/editchar.rs`, `admin/vitals.rs`,
//! `admin/points.rs` — editing one character's level, class, stats and vitals.

use super::*;

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
    world.objects.add_components(&6432, TargetRef(Some(6432)));
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
    world.objects.add_components(&7001, TargetRef(Some(7002)));

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
    world.objects.add_components(&7101, TargetRef(Some(7102)));
    on_packet(&mut world, 1, build_admin("res"));

    let v = pvit(&world, 7102);
    assert!(!v.dead, "victim revived");
    assert_eq!(v.cur_hp, v.max_hp as f64, "victim fully restored");
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
    world.objects.add_components(&7301, TargetRef(Some(7301)));
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("add_exp_sp 1000 500"));
    let p = world.objects.get_component::<Player>(&7301).unwrap();
    assert!(p.exp >= 1000, "exp granted");
    assert_eq!(p.sp, 500, "sp granted");
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
            .get_component::<TargetRef>(&7311)
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

/// `AdminLevel`'s accept range for `//set_level`: `1..=ExperienceData
/// .getMaxLevel()`, narrowed to `MaxSubclassLevel` while a subclass is active.
/// A value outside it is refused with the usage line and nothing changes;
/// **inside** it, `setLevel`'s own clamp still applies — which is why asking
/// for the top of the range (81) lands on 80.
#[test]
fn admin_set_level_refuses_past_the_cap_and_clamps_at_it() {
    let (mut world, ..) = admin_world();
    world.data.experience = crate::data::ExperienceData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    let mut gm_rx = ingame_player_access(&mut world, 1, 7306, 100);
    on_packet(&mut world, 1, build_admin("set_level 20"));
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("set_level 999"));
    assert_eq!(
        world.objects.get_component::<Player>(&7306).unwrap().level,
        20,
        "out of range: refused outright, not clamped"
    );
    assert!(
        has_system_message(&drain(&mut gm_rx), server_packets::sm_ids::S1_TEXT),
        "…and said so"
    );

    on_packet(&mut world, 1, build_admin("set_level 81"));
    let p = world.objects.get_component::<Player>(&7306).unwrap();
    assert_eq!(
        p.level, 80,
        "the top of the range is accepted, then clamped by setLevel"
    );
    assert_eq!(
        p.exp,
        world.data.experience.exp_for_level(80),
        "and the exp that goes with the level it settled on, not the one asked for"
    );

    // A subclass lowers the range itself, so the same value is now refused.
    world.cfg.character.max_subclass_level = 75;
    world
        .objects
        .get_component_mut::<Player>(&7306)
        .unwrap()
        .class_index = 1;
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin("set_level 80"));
    assert_eq!(
        world.objects.get_component::<Player>(&7306).unwrap().level,
        80,
        "refused: 80 is past MaxSubclassLevel"
    );
    on_packet(&mut world, 1, build_admin("set_level 70"));
    assert_eq!(
        world.objects.get_component::<Player>(&7306).unwrap().level,
        70,
        "inside the narrowed range"
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

/// `//character_disconnect` disconnects the targeted player.
#[test]
fn admin_character_disconnect_kicks_target() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7504, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7505, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    world.objects.add_components(&7504, TargetRef(Some(7505)));
    on_packet(&mut world, 1, build_admin("character_disconnect"));
    assert!(!world.clients.contains_key(&2), "victim disconnected");
    assert!(
        world.objects.get_component::<Player>(&7505).is_none(),
        "victim despawned"
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
            .get_component::<TargetRef>(&7701)
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
            .get_component::<AdminFlags>(&7801)
            .unwrap()
            .invul
    );

    let hp_before = pvit(&world, 7801).cur_hp;
    combat::player_receive_damage(&mut world, 7801, 12345, 50.0);
    assert_eq!(
        pvit(&world, 7801).cur_hp,
        hp_before,
        "invul: no damage taken"
    );

    // Toggle off → damage lands.
    on_packet(&mut world, 1, build_admin("invul"));
    combat::player_receive_damage(&mut world, 7801, 12345, 50.0);
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
    combat::player_receive_damage(&mut world, 7802, 12345, 100_000.0);
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

    world.objects.add_components(&7803, TargetRef(Some(7804)));
    on_packet(&mut world, 1, build_admin("setinvul"));
    assert!(
        world
            .objects
            .get_component::<AdminFlags>(&7804)
            .unwrap()
            .invul
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
    world.objects.add_components(&8301, TargetRef(Some(8302)));

    let p = |w: &World| w.objects.get_component::<Player>(&8302).unwrap().clone();

    on_packet(&mut world, 1, build_admin("setreputation -500"));
    assert_eq!(p(&world).reputation, -500);
    on_packet(&mut world, 1, build_admin("nokarma"));
    assert_eq!(p(&world).reputation, 0);
    on_packet(&mut world, 1, build_admin("setpk 7"));
    assert_eq!(p(&world).pk_kills, 7);
    on_packet(&mut world, 1, build_admin("setpvp 9"));
    assert_eq!(p(&world).pvp_kills, 9);
    // `//setfame` goes through `Player.setFame`'s clamp like every other fame
    // write, and this dist caps it at 0 — so the ceiling is raised here to test
    // the setter rather than the clamp.
    world.cfg.character.max_personal_fame_points = 1_000_000;
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
    let book = world.objects.get_component::<SkillBook>(&8703).unwrap();
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

/// `//remove_exp_sp <exp> <sp>` subtracts from the targeted player (the GM
/// targets itself, matching Java's required player target).
#[test]
fn admin_remove_exp_sp_reduces() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8901, 100);
    world.objects.add_components(&8901, TargetRef(Some(8901)));
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

/// `//changename` renames the targeted player; a collision with an online name
/// is rejected.
#[test]
fn admin_changename_renames_target() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8903, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 8904, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);
    world.objects.add_components(&8903, TargetRef(Some(8904)));
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
    // The dist runs `EnableVitality = True`; the derived config default is
    // false, and the command now refuses (as Java's does) when it is off.
    world.cfg.character.enable_vitality = true;
    let mut gm_rx = ingame_player_access(&mut world, 1, 8907, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 8908, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);
    world.objects.add_components(&8907, TargetRef(Some(8908)));
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

/// `//setparam pAtk <v>` fixes the target's P.Atk to `v` (Java `addFixedValue`);
/// `//unsetparam pAtk` restores the computed value.
#[test]
fn admin_setparam_fixes_and_clears_a_stat() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8950, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 8951, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);
    world.objects.add_components(&8950, TargetRef(Some(8951)));
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
            .get_component::<model::components::stats::StatModifiers>(&8951)
            .unwrap()
            .fixed
            .get(&Stat::PhysicalAttack),
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
            .get_component::<model::components::stats::StatModifiers>(&8951)
            .unwrap()
            .fixed
            .is_empty()
    );
}

/// Java `setClassId` drops hennas the **new** class may not wear — outright,
/// with no refund, because the character never asked to remove them.
#[test]
fn setclass_drops_a_dye_the_new_class_cannot_wear() {
    use model::components::skills::HennaSlots;
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

// ---------------------------------------------------------------------------
// `//zone_visual` / `//zone_visual_clear` (measured-gaps row 16)
// ---------------------------------------------------------------------------

/// **`//rec` clamps like Java's setter** (GitHub #7). `Player.setRecomHave` is
/// `Math.min(Math.max(value, 0), 255)`, so a GM typing a huge number lands on
/// the cap rather than storing it verbatim.
#[test]
fn rec_clamps_to_the_java_range() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7401, 100);
    let _victim = ingame_player_access(&mut world, 2, 7402, 0);
    drain(&mut gm_rx);
    world.objects.add_components(&7401, TargetRef(Some(7402)));

    for (typed, expected) in [("99999", 255), ("-5", 0), ("42", 42)] {
        on_packet(
            &mut world,
            1,
            [
                vec![cop::SEND_BYPASS_BUILD_CMD],
                build_cmd_body(&format!("rec {typed}")),
            ]
            .concat(),
        );
        assert_eq!(
            world
                .objects
                .get_component::<Player>(&7402)
                .expect("victim")
                .rec_have,
            expected,
            "//rec {typed}"
        );
    }
}

/// **`//set_vitality` on a server with vitality off says so** (GitHub #8's
/// sibling): Java's `AdminVitality` gates on `Config.ENABLE_VITALITY` before it
/// looks at the target at all. The commands themselves work — this is the one
/// clause the port was missing.
#[test]
fn vitality_commands_report_a_disabled_system() {
    let (mut world, ..) = admin_world();
    world.cfg.character.enable_vitality = false;
    let mut gm_rx = ingame_player_access(&mut world, 1, 7405, 100);
    let _victim = ingame_player_access(&mut world, 2, 7406, 0);
    drain(&mut gm_rx);
    world.objects.add_components(&7405, TargetRef(Some(7406)));

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("full_vitality"),
        ]
        .concat(),
    );

    let texts: Vec<String> = drain(&mut gm_rx)
        .iter()
        .filter_map(|p| system_message_text(p))
        .collect();
    assert!(
        texts
            .iter()
            .any(|t| t.contains("Vitality is not enabled on the server!")),
        "the GM is told why nothing happened: {texts:?}"
    );
}

/// **The vitality commands have to say what they did** (GitHub #8). Java's
/// `AdminVitality` is silent on success, so a GM pressing "Vit Set" / "Vit Max"
/// cannot tell a command that worked from one that did nothing — which is what
/// the report described. Every arm now names the state of the system and the
/// outcome: the pool before and after, the clamp when the typed number is out
/// of range, and "nothing changed" when it is already there.
#[test]
fn vitality_commands_report_the_outcome() {
    let (mut world, ..) = admin_world();
    world.cfg.character.enable_vitality = true;
    let mut gm_rx = ingame_player_access(&mut world, 1, 7415, 100);
    let _victim = ingame_player_access(&mut world, 2, 7416, 0);
    drain(&mut gm_rx);
    world.objects.add_components(&7415, TargetRef(Some(7416)));

    let run = |world: &mut World, cmd: &str, rx: &mut _| {
        on_packet(
            world,
            1,
            [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body(cmd)].concat(),
        );
        drain(rx)
            .iter()
            .filter_map(|p| system_message_text(p))
            .collect::<Vec<_>>()
    };

    // A number outside 0..=140000 still succeeds — say so, rather than leave
    // the GM reading the difference as a failure.
    let texts = run(&mut world, "set_vitality 999999", &mut gm_rx);
    assert!(
        texts.iter().any(|t| t.contains("Vitality is enabled")
            && t.contains("set to 140000")
            && t.contains("clamped")),
        "the clamp is reported as a success: {texts:?}"
    );

    // Already there: a distinct line, not silence and not a success line.
    let texts = run(&mut world, "full_vitality", &mut gm_rx);
    assert!(
        texts
            .iter()
            .any(|t| t.contains("Vitality is enabled") && t.contains("nothing changed")),
        "a no-op is reported as a no-op: {texts:?}"
    );

    // And the move back down reports both ends of it.
    let texts = run(&mut world, "empty_vitality", &mut gm_rx);
    assert!(
        texts.iter().any(|t| t.contains("Vitality is enabled")
            && t.contains("set to 0")
            && t.contains("was 140000")),
        "the before/after pair is reported: {texts:?}"
    );

    // Nothing targeted is a refusal, and still names the state of the system.
    world.objects.add_components(&7415, TargetRef(None));
    let texts = run(&mut world, "full_vitality", &mut gm_rx);
    assert!(
        texts
            .iter()
            .any(|t| t.contains("Vitality is enabled") && t.contains("Target not found")),
        "the no-target refusal names the state too: {texts:?}"
    );
}
