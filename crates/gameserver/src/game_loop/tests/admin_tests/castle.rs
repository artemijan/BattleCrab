//! `admin/castle.rs`, `admin/pledge.rs`, `admin/grand_boss.rs` — the
//! residence, clan and grand-boss management panels.

use super::*;

/// `//grandboss` opens the boss menu; `//grandboss <id>` shows one boss's live
/// status/respawn from `world.grand_bosses`; the per-boss action buttons hit the
/// unported boss AI (Java NPEs on the null AI, reproduced here). Port of
/// `AdminGrandBoss`.
#[test]
fn admin_grandboss_status_panel_and_actions() {
    use model::grand_boss::GrandBoss;
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

/// `//castlemanage` shows a castle's page; `setOwner` assigns the targeted
/// clanned player's clan + side, `switchSide` flips it, `takeCastle` strips it;
/// siege actions report unavailable. Port of AdminCastle.
#[test]
fn admin_castlemanage_ownership_and_side() {
    use model::castle::{Castle, CastleSide};
    use model::clan::{Clan, ClanMember};
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
    world.sieges.insert(3, model::siege::Siege::new(3));
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
    use model::castle::{Castle, CastleSide};
    use model::clan::{Clan, ClanMember};
    use model::siege::Siege;
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

/// **`//clan_changeleader` swaps the leader immediately** — clan record,
/// both players' leader flags/privileges, and the clan-wide SM.
#[test]
fn clan_changeleader_swaps_leader() {
    use model::components::combat::TargetRef;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7601, 100);
    let mut old_rx = ingame_player_access(&mut world, 2, 7602, 0);
    let mut new_rx = ingame_player_access(&mut world, 3, 7603, 0);
    // Clan 600: 7602 leads, 7603 is a member.
    world.clans.insert(
        600,
        Clan {
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

/// Insert a minimal clan led by `leader_id` and enrol every `members` player.
fn seed_clan(world: &mut World, clan_id: i32, leader_id: i32, members: &[i32]) {
    world.clans.insert(
        clan_id,
        Clan {
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

/// `AdminPledge` re-shows the Game panel after every branch **except** the one
/// where `Integer.parseInt` throws past it into `AdminCommandHandler`. Both
/// halves are asserted together: a bad level prints the exception line and
/// leaves the panel closed, while a merely out-of-range level prints "Level
/// incorrect." and still re-opens it. The pair is what keeps a refactor from
/// quietly collapsing the two exits into one.
#[test]
fn pledge_setlevel_reopens_the_panel_except_when_the_parse_throws() {
    use model::components::combat::TargetRef;
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
