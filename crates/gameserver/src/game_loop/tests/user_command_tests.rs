//! The `/command` bar (`BypassUserCmd`) — the G15.5 sweep that wired every
//! handler this Java build registers. `/loc` and `/unstuck` have their own
//! coverage in `teleport_cmds_tests`.

use super::*;
use crate::game_loop::user_commands;

use crate::model::Player;
use crate::model::clan::Clan;
use crate::model::party::LootRule;

fn cmd(world: &mut World, client_id: u32, id: i32) {
    user_commands::handle_bypass_user_cmd(world, client_id, &user_cmd_body(id));
}

fn mk_clan(id: i32, name: &str, ally_id: i32, ally_name: &str) -> Clan {
    Clan {
        id,
        name: name.into(),
        leader_id: 0,
        level: 5,
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
        ally_id,
        ally_name: ally_name.into(),
        ally_penalty_expiry_time: 0,
        ally_penalty_type: 0,
        crest_id: 0,
        crest_large_id: 0,
        ally_crest_id: 0,
        blood_alliance_count: 0,
    }
}

/// **`/time` reports the in-game clock**, with the night variant of the message
/// after dark. The clock is a pure function of wall time, so the test asserts
/// the pairing rather than a fixed hour.
#[test]
fn time_reports_the_game_clock() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    cmd(&mut world, 1, 77);

    let ids = ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE);
    assert_eq!(ids.len(), 1, "one message");
    assert!(
        ids[0] == server_packets::sm_ids::THE_CURRENT_TIME_IS_S1_S2
            || ids[0] == server_packets::sm_ids::THE_CURRENT_TIME_IS_S1_S2_NIGHT,
        "…and it is one of the two clock strings"
    );

    // The day/night split and the clock math, on fixed instants: an in-game day
    // is 4 real hours from the epoch, and night is in-game 00:00–06:00.
    const IG_DAY_MS: i64 = 14_400_000;
    let (day_id, hour, minute) =
        crate::game_loop::user_commands::time_message(IG_DAY_MS / 2 + 610_000);
    assert_eq!(day_id, server_packets::sm_ids::THE_CURRENT_TIME_IS_S1_S2);
    assert_eq!((hour.as_str(), minute.as_str()), ("13", "01"));
    let (night_id, hour, _) = crate::game_loop::user_commands::time_message(60_000);
    assert_eq!(
        night_id,
        server_packets::sm_ids::THE_CURRENT_TIME_IS_S1_S2_NIGHT,
        "just after in-game midnight it is night"
    );
    assert_eq!(hour, "0");
}

/// **`/partyinfo` names the loot rule only when in a party.** Solo it is just
/// the header and Java's trailing blank line.
#[test]
fn party_info_reports_the_loot_rule() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    cmd(&mut world, 1, 81);
    assert_eq!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![
            server_packets::sm_ids::PARTY_INFORMATION,
            server_packets::sm_ids::EMPTY_3
        ],
        "solo: header + blank line"
    );

    // In a random-loot party the rule line appears between them.
    let party_id = 7;
    world.parties.insert(
        party_id,
        crate::model::party::Party::new(3001, LootRule::Random, 0),
    );
    world
        .objects
        .add_components(&3001, crate::model::components::PartyRef(party_id));
    cmd(&mut world, 1, 81);
    assert_eq!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![
            server_packets::sm_ids::PARTY_INFORMATION,
            server_packets::sm_ids::LOOTING_METHOD_RANDOM,
            server_packets::sm_ids::EMPTY_3
        ],
    );
}

/// **The clan-war lists show only *one-directional* wars.** Java's SQL excludes
/// the clans that declared back (those belong to the mutual list, whose id 90 is
/// shadowed by `InstanceZone` in this build). A clanless player is told so.
#[test]
fn clan_war_lists_exclude_mutual_wars() {
    use crate::model::clan::{ClanWar, ClanWarState};

    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    // No clan → "not joined in any clan".
    cmd(&mut world, 1, 88);
    assert_eq!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::NOT_JOINED_IN_ANY_CLAN]
    );

    world.clans.insert(10, mk_clan(10, "Mine", 0, ""));
    world.clans.insert(11, mk_clan(11, "Victim", 0, ""));
    world.clans.insert(12, mk_clan(12, "Mutual", 3, "TheAlly"));
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_id = 10;
    let war = |a: i32, b: i32| ClanWar {
        attacker_id: a,
        attacked_id: b,
        state: ClanWarState::Declaration,
        winner_id: 0,
        start_time: 0,
        end_time: 0,
        attacker_kills: 0,
        attacked_kills: 0,
    };
    world.clan_wars.push(war(10, 11)); // one-directional
    world.clan_wars.push(war(10, 12)); // …and declared back below
    world.clan_wars.push(war(12, 10));

    cmd(&mut world, 1, 88);
    let ids = ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE);
    assert_eq!(
        ids,
        vec![
            server_packets::sm_ids::CLANS_YOU_VE_DECLARED_WAR_ON,
            server_packets::sm_ids::S1_NO_ALLIANCE_EXISTS,
            server_packets::sm_ids::EMPTY_3
        ],
        "only the clan that hasn't declared back is listed"
    );
}

/// **`/clanpenalty` lists the live penalties**, or says there are none.
#[test]
fn clan_penalty_lists_active_penalties() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    cmd(&mut world, 1, 100);
    let page = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("the penalty page");
    assert!(page.contains("No penalty is imposed."), "{page}");

    // A live re-join penalty shows with its expiry date.
    let expiry = commons::util::now_millis() + 86_400_000;
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_join_expiry_time = expiry;
    cmd(&mut world, 1, 100);
    let page = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("the penalty page");
    assert!(page.contains("Unable to join a clan."), "{page}");
    assert!(
        page.contains(&commons::util::format_date(expiry)),
        "the expiry date is shown: {page}"
    );
}

/// **`/mybirthday` reports the character's creation date**, unpadded like Java.
#[test]
fn my_birthday_reports_the_creation_date() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .create_date = "2026-07-05".to_string();
    drain(&mut rx);

    cmd(&mut world, 1, 126);

    assert_eq!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::C1_S_BIRTHDAY_IS_S3_S4_S2]
    );
}

/// **`/olympiadstat` needs a 2nd-class target.** Without one the client is told
/// to complete the transfer first; with one it gets the record + the weekly
/// allowance.
#[test]
fn olympiad_stat_needs_a_second_class_target() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    // No target at all.
    cmd(&mut world, 1, 109);
    assert_eq!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::COMMAND_AVAILABLE_AFTER_THE_2ND_CLASS_TRANSFER]
    );

    // Target a 2nd-class player. Java's `ClassId.level() >= 2` is the port's
    // `THIRD_CLASS_GROUP` membership (base 0 → 1st transfer 1 → 2nd 2).
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[88]);
    let _rx2 = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .class_id = 88;
    world
        .objects
        .add_components(&3001, crate::model::components::TargetRef(Some(3002)));
    cmd(&mut world, 1, 109);
    assert_eq!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![
            server_packets::sm_ids::FOR_THE_CURRENT_OLYMPIAD_YOU_HAVE_PARTICIPATED,
            server_packets::sm_ids::THE_MATCHES_THIS_WEEK_ARE_ALL_CLASS_BATTLES
        ],
    );
}

/// **`/siegestatus` is for noble clan leaders in a running siege.** Everyone
/// else — including a noble leader with no siege — gets the refusal.
#[test]
fn siege_status_needs_a_noble_leader_in_a_siege() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    cmd(&mut world, 1, 99);
    assert_eq!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::ONLY_A_NOBLE_CLAN_LEADER_CAN_VIEW_THE_SIEGE_STATUS],
        "a plain player is refused"
    );

    // Give the clan a running siege it is attacking.
    world.clans.insert(10, mk_clan(10, "Mine", 0, ""));
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut siege = crate::model::siege::Siege::new(1);
    siege.in_progress = true;
    siege.clans.push(crate::model::siege::SiegeClan {
        clan_id: 10,
        kind: crate::model::siege::SiegeClanType::Attacker,
    });
    world.sieges.insert(1, siege);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.clan_id = 10;
    }

    // In the clan, in the siege — but not a noble leader: still refused.
    cmd(&mut world, 1, 99);
    assert_eq!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::ONLY_A_NOBLE_CLAN_LEADER_CAN_VIEW_THE_SIEGE_STATUS],
        "the noble + leader gate still refuses"
    );

    // As a noble leader the report page comes back, listing the member.
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.is_noble = true;
        p.clan_leader = true;
    }
    cmd(&mut world, 1, 99);
    let page = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("the siege status page");
    assert!(page.contains("Siege Status Report"), "{page}");
    assert!(
        page.contains("Not in the siege zone"),
        "the online member is listed, outside the zone: {page}"
    );
}
