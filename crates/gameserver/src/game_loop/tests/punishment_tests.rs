//! Punishment / jail (G31 slice 1): the jail effect (teleport + persist), the
//! release path (`//unjail` + timed expiry), the login re-apply, and the
//! JailZone keep-in.

use super::*;

use crate::data::spawn_data::{Territory, ZoneForm};
use crate::data::zone_data::{Zone, ZoneKind};
use crate::db::DbCommand;
use crate::game_loop::moderation::punishment;
use crate::model::components::Position;
use crate::model::punishment as punishment_models;
use crate::scheduler::ScheduledTask;

// Java `JailZone` locations.
const JAIL_IN: (i32, i32) = (-114356, -249645);
const JAIL_OUT: (i32, i32) = (17836, 170178);

/// `JailZone` teleports through Java's *scattering* flavour
/// (`new TeleportTask(player, JAIL_IN_LOC)` → `randomOffset = true`), so the
/// arrival lands within `MaxOffsetOnTeleport` of the cell rather than on it.
/// Asserting the exact tile would be asserting the bug.
fn assert_near_jail(world: &World, oid: i32) {
    let (x, y) = pos_xy(world, oid);
    let offset = world.cfg.character.teleport_offset();
    assert!(
        (x - JAIL_IN.0).abs() <= offset && (y - JAIL_IN.1).abs() <= offset,
        "expected to land within {offset} of {JAIL_IN:?}, got ({x}, {y})"
    );
}

/// Register a jail zone around the jail-in point so `in_jail_zone` is meaningful
/// (the test `GameData` ships no zones).
fn add_jail_zone(world: &mut World) {
    world.data.zone_data.insert(Zone {
        id: 0,
        name: "test_jail".into(),
        kind: ZoneKind::Jail,
        territory: Territory {
            form: ZoneForm::Cuboid {
                x1: JAIL_IN.0 - 2000,
                x2: JAIL_IN.0 + 2000,
                y1: JAIL_IN.1 - 2000,
                y2: JAIL_IN.1 + 2000,
            },
            min_z: -4000,
            max_z: 0,
        },
        castle_id: 0,
        clan_hall_id: 0,
        effect: None,
        damage: None,
        swamp: None,
        condition: None,
        mother_tree: None,
    });
}

fn pos_xy(world: &World, oid: i32) -> (i32, i32) {
    let p = world.objects.get_component::<Position>(&oid).unwrap();
    (p.x, p.y)
}

fn store_punishment_cmds(cmds: &[DbCommand]) -> usize {
    cmds.iter()
        .filter(|c| matches!(c, DbCommand::StorePunishment { .. }))
        .count()
}

#[test]
fn jail_teleports_marks_and_persists() {
    let (mut world, _tx, mut db_rx, _link) = test_world();
    let _rx = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);

    let applied = punishment::jail_character(&mut world, 3001, 0, "r".into(), "gm".into());
    assert!(applied);

    // Flag set, teleported into the prison.
    assert!(world.objects.get_component::<Player>(&3001).unwrap().jailed);
    assert_near_jail(&world, 3001);

    // Registered and persisted.
    assert!(world.punishments.has_punishment(
        "3001",
        punishment_models::PunishmentAffect::Character,
        punishment_models::PunishmentType::Jail
    ));
    let cmds = drain_db(&mut db_rx);
    assert_eq!(store_punishment_cmds(&cmds), 1, "one StorePunishment sent");
    assert!(cmds.iter().any(|c| matches!(
        c,
        DbCommand::StorePunishment { key, affect, ptype, .. }
            if key == "3001" && affect == "CHARACTER" && ptype == "JAIL"
    )));
}

#[test]
fn jailing_an_already_jailed_player_is_rejected() {
    let (mut world, _tx, _rx, _link) = test_world();
    let _out = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);

    assert!(punishment::jail_character(
        &mut world,
        3001,
        0,
        "r".into(),
        "gm".into()
    ));
    // Second jail on the same character → Java's "already affected" guard.
    assert!(!punishment::jail_character(
        &mut world,
        3001,
        0,
        "r".into(),
        "gm".into()
    ));
}

#[test]
fn unjail_releases_teleports_out_and_deletes() {
    let (mut world, _tx, mut db_rx, _link) = test_world();
    let _out = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);
    punishment::jail_character(&mut world, 3001, 0, "r".into(), "gm".into());
    drain_db(&mut db_rx);

    assert!(punishment::unjail_character(&mut world, 3001));
    assert!(!world.objects.get_component::<Player>(&3001).unwrap().jailed);
    assert_eq!(pos_xy(&world, 3001), JAIL_OUT);
    assert!(!world.punishments.has_punishment(
        "3001",
        punishment_models::PunishmentAffect::Character,
        punishment_models::PunishmentType::Jail
    ));
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter()
            .any(|c| matches!(c, DbCommand::DeletePunishment { .. })),
        "DeletePunishment sent on release"
    );

    // Releasing a non-jailed player is a no-op false.
    assert!(!punishment::unjail_character(&mut world, 3001));
}

#[test]
fn a_timed_jail_expires_and_releases_the_player() {
    let (mut world, _tx, mut db_rx, _link) = test_world();
    let _out = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);

    // One minute → an expiry timer is armed.
    punishment::jail_character(&mut world, 3001, 1, "r".into(), "gm".into());
    assert!(world.objects.get_component::<Player>(&3001).unwrap().jailed);
    drain_db(&mut db_rx);

    // 60 s = 600 ticks, plus a margin, firing tasks each tick.
    advance_ticks(&mut world, 700);

    assert!(!world.objects.get_component::<Player>(&3001).unwrap().jailed);
    assert_eq!(pos_xy(&world, 3001), JAIL_OUT);
    assert!(!world.punishments.has_punishment(
        "3001",
        punishment_models::PunishmentAffect::Character,
        punishment_models::PunishmentType::Jail
    ));
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, DbCommand::DeletePunishment { .. }))
    );
}

#[test]
fn keep_in_teleports_a_wanderer_back_but_leaves_an_inmate() {
    let (mut world, _tx, _rx, _link) = test_world();
    add_jail_zone(&mut world);
    let _out = ingame_player(&mut world, 1, 3001, JAIL_IN.0, JAIL_IN.1, -2984);
    // Mark jailed directly (jail_character would teleport; we want to place them).
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .jailed = true;

    // Standing inside the jail zone: keep-in leaves them put.
    punishment::enforce_jail_keep_in(&mut world, 3001);
    assert_near_jail(&world, 3001);

    // Wander far outside the zone, then re-check: teleported straight back.
    {
        let p = world.objects.get_component_mut::<Position>(&3001).unwrap();
        p.x = 50_000;
        p.y = 50_000;
    }
    punishment::enforce_jail_keep_in(&mut world, 3001);
    assert_near_jail(&world, 3001);
}

#[test]
fn keep_in_ignores_a_free_player() {
    let (mut world, _tx, _rx, _link) = test_world();
    add_jail_zone(&mut world);
    let _out = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);
    // Not jailed, standing outside the zone → keep-in does nothing.
    punishment::enforce_jail_keep_in(&mut world, 3001);
    assert_eq!(pos_xy(&world, 3001), (50_000, 50_000));
}

#[test]
fn boot_load_registers_and_re_arms_a_timed_jail() {
    let (mut world, _tx, _rx, _link) = test_world();
    let now = commons::util::now_millis();
    let task = punishment_models::Punishment {
        id: 7,
        key: "3001".into(),
        affect: punishment_models::PunishmentAffect::Character,
        ptype: punishment_models::PunishmentType::Jail,
        expiration: now + 60_000,
        reason: "r".into(),
        punished_by: "gm".into(),
    };
    punishment::on_loaded(&mut world, 8, vec![task]);

    // Registered, allocator seeded, and an expiry timer queued.
    assert!(world.punishments.has_punishment(
        "3001",
        punishment_models::PunishmentAffect::Character,
        punishment_models::PunishmentType::Jail
    ));
    assert_eq!(world.punishments.next_id, 8);
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .iter()
            .any(|t| matches!(t, ScheduledTask::PunishmentExpire { punishment_id: 7 }))
    );
}

#[test]
fn on_enter_world_reapplies_jail_to_a_returning_inmate() {
    let (mut world, _tx, _rx, _link) = test_world();
    add_jail_zone(&mut world);
    // A persisted jail for char 3001, but the player logs in out in the world.
    let now = commons::util::now_millis();
    punishment::on_loaded(
        &mut world,
        1,
        vec![punishment_models::Punishment {
            id: 1,
            key: "3001".into(),
            affect: punishment_models::PunishmentAffect::Character,
            ptype: punishment_models::PunishmentType::Jail,
            expiration: now + 3_600_000,
            reason: "r".into(),
            punished_by: "gm".into(),
        }],
    );
    let _out = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);

    punishment::on_enter_world(&mut world, 1, 3001);
    assert!(world.objects.get_component::<Player>(&3001).unwrap().jailed);
    assert_near_jail(&world, 3001);
}

// --- Slice 2: ban / chat-ban / party-ban -----------------------------------
use crate::model::components::PendingRequest;

fn ban(
    world: &mut World,
    key: &str,
    affect: punishment_models::PunishmentAffect,
    ptype: punishment_models::PunishmentType,
) -> bool {
    punishment::start_punishment(
        world,
        key.to_string(),
        affect,
        ptype,
        0,
        "test".into(),
        "gm".into(),
    )
}

#[test]
fn start_punishment_guards_duplicates_and_stop_lifts() {
    let (mut world, _tx, _rx, _link) = test_world();
    assert!(ban(
        &mut world,
        "acc1",
        punishment_models::PunishmentAffect::Account,
        punishment_models::PunishmentType::Ban
    ));
    // Same (key, affect, type) again → Java's "already affected" guard.
    assert!(!ban(
        &mut world,
        "acc1",
        punishment_models::PunishmentAffect::Account,
        punishment_models::PunishmentType::Ban
    ));
    assert!(world.punishments.has_punishment(
        "acc1",
        punishment_models::PunishmentAffect::Account,
        punishment_models::PunishmentType::Ban
    ));
    assert!(punishment::stop_punishment(
        &mut world,
        "acc1",
        punishment_models::PunishmentAffect::Account,
        punishment_models::PunishmentType::Ban
    ));
    assert!(!world.punishments.has_punishment(
        "acc1",
        punishment_models::PunishmentAffect::Account,
        punishment_models::PunishmentType::Ban
    ));
}

#[test]
fn ban_disconnects_the_online_player_and_flags_them() {
    let (mut world, _tx, _rx, _link) = test_world();
    let _out = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    ban(
        &mut world,
        "3001",
        punishment_models::PunishmentAffect::Character,
        punishment_models::PunishmentType::Ban,
    );

    // Java `BanHandler.onStart` → Disconnection: session dropped + player despawned.
    assert!(world.clients.get(&1).is_none(), "session dropped");
    assert!(
        !world.objects.has_component::<Player>(&3001),
        "player despawned"
    );
    // The gate reads the char id (== object id); account/IP/HWID irrelevant here.
    assert!(punishment::is_banned(&world, 3001, "acc", "1.2.3.4", None));
}

#[test]
fn character_select_refuses_a_banned_character() {
    let (mut world, _tx, _rx, _link) = test_world();
    let mut out_rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    handle_request_restart(&mut world, 1);
    on_characters_loaded(
        &mut world,
        1,
        "bob".into(),
        vec![dummy_char(5001, "P5001")],
        true,
    );
    while out_rx.try_recv().is_ok() {}

    ban(
        &mut world,
        "5001",
        punishment_models::PunishmentAffect::Character,
        punishment_models::PunishmentType::Ban,
    );

    let mut w = PacketWriter::new();
    w.write_i32(0); // slot
    handle_character_select(&mut world, 1, &w.into_bytes());

    assert!(
        out_rx.try_recv().is_err(),
        "no CharSelected for a banned character"
    );
    assert!(world.clients.get(&1).is_none(), "connection closed");
}

#[test]
fn chat_ban_blocks_chat_but_a_dot_command_slips_through() {
    let (mut world, _tx, _rx, _link) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 500, 0, 0);
    ban(
        &mut world,
        "3001",
        punishment_models::PunishmentAffect::Character,
        punishment_models::PunishmentType::ChatBan,
    );
    assert!(punishment::is_chat_banned(&world, 3001));
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Ordinary general chat is swallowed — the in-range bystander hears nothing.
    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body("hello", 0, None)].concat(),
    );
    assert!(drain(&mut b_rx).is_empty(), "chat-banned speech is blocked");

    // A `.`-prefixed message bypasses the ban (Java `_text.charAt(0) != '.'`).
    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body(".hi", 0, None)].concat(),
    );
    assert!(
        !drain(&mut b_rx).is_empty(),
        "a dot command is not chat-blocked"
    );

    // Lifting the chat-ban restores speech.
    assert!(punishment::stop_punishment(
        &mut world,
        "3001",
        punishment_models::PunishmentAffect::Character,
        punishment_models::PunishmentType::ChatBan
    ));
    assert!(!punishment::is_chat_banned(&world, 3001));
    drain(&mut a_rx);
    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body("free now", 0, None)].concat(),
    );
    assert!(!drain(&mut b_rx).is_empty(), "speech works after unban");
}

#[test]
fn party_ban_blocks_a_banned_requestor_and_a_banned_target() {
    // Banned requestor can't invite.
    let (mut world, _tx, _rx, _link) = test_world();
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    ban(
        &mut world,
        "3001",
        punishment_models::PunishmentAffect::Character,
        punishment_models::PunishmentType::PartyBan,
    );
    assert!(punishment::is_party_banned(&world, 3001));

    let mut w = PacketWriter::new();
    w.write_string("P3002");
    w.write_i32(0);
    crate::game_loop::party::handle_request_join_party(&mut world, 1, &w.into_bytes());
    assert!(
        !world.objects.has_component::<PendingRequest>(&3002),
        "a party-banned requestor cannot invite"
    );

    // Banned target can't be invited.
    let (mut world, _tx, _rx, _link) = test_world();
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    ban(
        &mut world,
        "3002",
        punishment_models::PunishmentAffect::Character,
        punishment_models::PunishmentType::PartyBan,
    );
    let mut w = PacketWriter::new();
    w.write_string("P3002");
    w.write_i32(0);
    crate::game_loop::party::handle_request_join_party(&mut world, 1, &w.into_bytes());
    assert!(
        !world.objects.has_component::<PendingRequest>(&3002),
        "a party-banned target cannot be invited"
    );
}

// --- Illegal player actions (Java `Util.handleIllegalPlayerAction` +
// `IllegalPlayerActionTask`, G35) -------------------------------------------

#[test]
fn illegal_action_kick_fires_after_the_five_second_delay() {
    let (mut world, _tx, _db_rx, _link) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);
    drain(&mut rx);

    punishment::handle_illegal_player_action(
        &mut world,
        3001,
        "test kick",
        punishment_models::IllegalActionPunishment::Kick,
    );
    // The warning is immediate, the kick is not — Java schedules the task 5 s
    // out and the offender stays connected until it runs.
    assert!(!drain(&mut rx).is_empty(), "the warning line is immediate");
    assert!(
        world.clients.contains_key(&1),
        "still connected at call time"
    );
    advance_ticks(&mut world, 51);
    assert!(
        !world.clients.contains_key(&1),
        "kicked when the task fires"
    );
}

#[test]
fn illegal_action_jail_books_a_timed_jail_punishment() {
    let (mut world, _tx, _db_rx, _link) = test_world();
    add_jail_zone(&mut world);
    let _rx = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);
    world.cfg.general.default_punish_param = 60;

    punishment::handle_illegal_player_action(
        &mut world,
        3001,
        "test jail",
        punishment_models::IllegalActionPunishment::Jail,
    );
    assert!(
        !world.objects.get_component::<Player>(&3001).unwrap().jailed,
        "not jailed until the task fires"
    );
    advance_ticks(&mut world, 51);
    assert!(world.objects.get_component::<Player>(&3001).unwrap().jailed);
    assert_near_jail(&world, 3001);
    assert!(world.punishments.has_punishment(
        "3001",
        punishment_models::PunishmentAffect::Character,
        punishment_models::PunishmentType::Jail
    ));
}

#[test]
fn illegal_action_kickban_drops_access_bans_and_disconnects() {
    let (mut world, _tx, mut db_rx, link_rx) = test_world();
    let _rx = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);
    drain_db(&mut db_rx);

    punishment::handle_illegal_player_action(
        &mut world,
        3001,
        "test kickban",
        punishment_models::IllegalActionPunishment::KickBan,
    );
    // The access-level drop is immediate (Java does it in the constructor):
    // character to −1 (persisted) and the account relayed to the login server.
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .access_level,
        -1
    );
    assert!(drain_db(&mut db_rx).iter().any(|c| matches!(
        c,
        DbCommand::SetAccessLevel {
            char_id: 3001,
            level: -1
        }
    )));
    let mut link_rx = link_rx;
    assert!(matches!(
        link_rx.try_recv(),
        Ok(crate::loginlink::LoginLinkCommand::SetAccountAccessLevel { level: -1, .. })
    ));

    advance_ticks(&mut world, 51);
    assert!(world.punishments.has_punishment(
        "3001",
        punishment_models::PunishmentAffect::Character,
        punishment_models::PunishmentType::Ban
    ));
    assert!(!world.clients.contains_key(&1), "kicked with the ban");
}

#[test]
fn illegal_action_spares_a_gm_from_the_punishment() {
    let (mut world, _tx, _db_rx, _link) = admin_world();
    let _rx = ingame_player_access(&mut world, 1, 3001, 70);

    punishment::handle_illegal_player_action(
        &mut world,
        3001,
        "gm tripped a guard",
        punishment_models::IllegalActionPunishment::Kick,
    );
    advance_ticks(&mut world, 51);
    assert!(
        world.clients.contains_key(&1),
        "the audit record is written but a GM is never punished"
    );
}

/// End-to-end through a wired guard: a `RequestDestroyItem` with a negative
/// count is the classic packet-tool signature — the offender is kicked
/// (`DefaultPunish` = KICK) five seconds later.
#[test]
fn destroy_item_negative_count_trips_the_default_punish() {
    let (mut world, _tx, _db_rx, _link) = test_world();
    let _rx = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);

    let mut body = Vec::new();
    body.extend_from_slice(&999_i32.to_le_bytes()); // any object id
    body.extend_from_slice(&(-5_i64).to_le_bytes()); // negative count
    items::handle_request_destroy_item(&mut world, 1, &body);

    assert!(world.clients.contains_key(&1));
    advance_ticks(&mut world, 51);
    assert!(
        !world.clients.contains_key(&1),
        "kicked for the exploit probe"
    );
}
