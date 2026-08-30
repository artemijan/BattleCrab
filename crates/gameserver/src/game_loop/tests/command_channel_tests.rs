//! Command channels (MPCC): the ask/accept/oust flow, the propagation rules,
//! CC chat, and the MPCC matching rooms.

use super::*;
use crate::game_loop::items::ground_items;
use crate::game_loop::party;
use crate::game_loop::party::command_channel;
use crate::game_loop::party::rooms;
use crate::game_loop::social::chat;
use crate::network::server_packets::opcodes;
use crate::network::server_packets::sm_ids;

const STRATEGY_GUIDE: i32 = 8871;

fn name_body(name: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(name);
    w.into_bytes()
}

fn i32_body(v: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(v);
    w.into_bytes()
}

/// Two two-man parties: A = 3001 (leader) + 3002, B = 3003 (leader) + 3004.
/// Returns the four receivers in that order.
fn two_parties(world: &mut World) -> Vec<UnboundedReceiver<bytes::Bytes>> {
    // Item object ids for the Strategy Guide `add_inventory_item` (the bare
    // `test_world` pool is empty).
    world.id_pool = 0x2000_0000..0x2000_1000;
    let mut rxs = Vec::new();
    for (cid, oid) in [(1u32, 3001), (2, 3002), (3, 3003), (4, 3004)] {
        rxs.push(ingame_player(world, cid, oid, 0, 0, 0));
    }
    make_party(world, &[3001, 3002], LootRule::FindersKeepers);
    make_party(world, &[3003, 3004], LootRule::FindersKeepers);
    for rx in &mut rxs {
        drain(rx);
    }
    rxs
}

/// Invite B (via its member P3004 — the dialog must land on leader P3003) and
/// accept, forming a channel led by 3001.
fn form_channel(world: &mut World) {
    items::add_inventory_item(world, 3001, STRATEGY_GUIDE, 1);
    on_packet(
        world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_ASK_JOIN_MPCC,
            &name_body("P3004"),
        ),
    );
    on_packet(
        world,
        3,
        ex_packet(cp::ex_opcodes::REQUEST_EX_ACCEPT_JOIN_MPCC, &i32_body(1)),
    );
}

#[test]
fn ask_accept_forms_a_channel_with_the_right_packets() {
    let (mut world, ..) = test_world();
    let mut rxs = two_parties(&mut world);

    // Without the forming right: SM 1574, no invite lands.
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_ASK_JOIN_MPCC,
            &name_body("P3003"),
        ),
    );
    assert!(ids_after_opcode(&drain(&mut rxs[0]), opcodes::SYSTEM_MESSAGE).contains(
        &sm_ids::COMMAND_CHANNELS_CAN_ONLY_BE_FORMED_BY_A_PARTY_LEADER_WHO_IS_ALSO_THE_LEADER_OF_A_LEVEL_5_CLAN
    ));
    assert!(drain(&mut rxs[2]).is_empty());

    // A non-leader inviter: SM 1593.
    on_packet(
        &mut world,
        2,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_ASK_JOIN_MPCC,
            &name_body("P3003"),
        ),
    );
    assert!(
        ids_after_opcode(&drain(&mut rxs[1]), opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::YOU_DO_NOT_HAVE_AUTHORITY_TO_INVITE_SOMEONE_TO_THE_COMMAND_CHANNEL)
    );

    // With the Strategy Guide, inviting through *member* P3004: the dialog
    // goes to B's leader P3003.
    items::add_inventory_item(&mut world, 3001, STRATEGY_GUIDE, 1);
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_ASK_JOIN_MPCC,
            &name_body("P3004"),
        ),
    );
    let leader_pkts = drain(&mut rxs[2]);
    assert!(
        leader_pkts
            .iter()
            .any(|p| is_ex(p, opcodes::EX_ASK_JOIN_MPCC)),
        "ExAskJoinMPCC lands on the target party's leader"
    );
    assert!(
        ids_after_opcode(&leader_pkts, opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::C1_IS_INVITING_YOU_TO_A_COMMAND_CHANNEL_DO_YOU_ACCEPT)
    );
    assert!(
        drain(&mut rxs[3]).is_empty(),
        "the clicked member gets nothing"
    );

    // Accept: the channel forms around A, B joins.
    on_packet(
        &mut world,
        3,
        ex_packet(cp::ex_opcodes::REQUEST_EX_ACCEPT_JOIN_MPCC, &i32_body(1)),
    );
    assert_eq!(world.command_channels.len(), 1);
    let cc = world.command_channels.values().next().unwrap();
    assert_eq!(cc.leader, 3001);
    assert_eq!(cc.parties.len(), 2);

    // A's side: formation SM + open window; the joining party's add was
    // announced to the pre-existing channel (party A) via ExMPCCPartyInfoUpdate.
    let a_pkts = drain(&mut rxs[0]);
    assert!(
        ids_after_opcode(&a_pkts, opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::THE_COMMAND_CHANNEL_HAS_BEEN_FORMED)
    );
    assert!(a_pkts.iter().any(|p| is_ex(p, opcodes::EX_OPEN_MPCC)));
    assert!(
        a_pkts
            .iter()
            .any(|p| is_ex(p, opcodes::EX_MPCC_PARTY_INFO_UPDATE))
    );
    // B's side: joined SM + open window.
    let b_pkts = drain(&mut rxs[2]);
    assert!(
        ids_after_opcode(&b_pkts, opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::YOU_HAVE_JOINED_THE_COMMAND_CHANNEL)
    );
    assert!(b_pkts.iter().any(|p| is_ex(p, opcodes::EX_OPEN_MPCC)));

    // Re-inviting an already-channelled party: SM 1594.
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_ASK_JOIN_MPCC,
            &name_body("P3003"),
        ),
    );
    assert!(
        ids_after_opcode(&drain(&mut rxs[0]), opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::C1_S_PARTY_IS_ALREADY_A_MEMBER_OF_THE_COMMAND_CHANNEL)
    );
}

#[test]
fn ousting_the_second_party_disbands_the_channel() {
    let (mut world, ..) = test_world();
    let mut rxs = two_parties(&mut world);
    form_channel(&mut world);
    for rx in &mut rxs {
        drain(rx);
    }

    // A non-CC-leader oust: SM 50.
    on_packet(
        &mut world,
        3,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_OUST_FROM_MPCC,
            &name_body("P3001"),
        ),
    );
    assert!(
        ids_after_opcode(&drain(&mut rxs[2]), opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::YOUR_TARGET_CANNOT_BE_FOUND)
    );
    assert_eq!(world.command_channels.len(), 1);

    // The CC leader ousts B (named by its member): both parties' windows
    // close and the channel dies (fewer than two parties remain).
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_OUST_FROM_MPCC,
            &name_body("P3004"),
        ),
    );
    assert!(world.command_channels.is_empty());
    let b_pkts = drain(&mut rxs[3]);
    assert!(b_pkts.iter().any(|p| is_ex(p, opcodes::EX_CLOSE_MPCC)));
    assert!(
        ids_after_opcode(&b_pkts, opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::YOU_WERE_DISMISSED_FROM_THE_COMMAND_CHANNEL)
    );
    let a_pkts = drain(&mut rxs[0]);
    assert!(
        ids_after_opcode(&a_pkts, opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::THE_COMMAND_CHANNEL_HAS_BEEN_DISBANDED)
    );
    assert!(a_pkts.iter().any(|p| is_ex(p, opcodes::EX_CLOSE_MPCC)));
}

#[test]
fn party_collapse_of_the_leading_party_kills_the_channel() {
    let (mut world, ..) = test_world();
    let mut rxs = two_parties(&mut world);
    form_channel(&mut world);
    for rx in &mut rxs {
        drain(rx);
    }

    // 3002 leaves party A → A collapses (2 members) → the CC leader's party
    // is gone → the whole channel disbands.
    party::remove_party_member(&mut world, 1, 3002, party::LeaveType::Left);
    assert!(world.command_channels.is_empty());
    assert!(
        drain(&mut rxs[2])
            .iter()
            .any(|p| is_ex(p, opcodes::EX_CLOSE_MPCC))
    );
}

#[test]
fn roster_query_answers_with_the_party_members() {
    let (mut world, ..) = test_world();
    let mut rxs = two_parties(&mut world);
    form_channel(&mut world);
    for rx in &mut rxs {
        drain(rx);
    }

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_MPCC_SHOW_PARTY_MEMBERS_INFO,
            &i32_body(3003),
        ),
    );
    let pkts = drain(&mut rxs[0]);
    let info = pkts
        .iter()
        .find(|p| is_ex(p, opcodes::EX_MPCC_SHOW_PARTY_MEMBER_INFO))
        .expect("roster answer");
    let mut r = commons::network::PacketReader::new(&info[3..]);
    assert_eq!(r.read_i32().unwrap(), 2, "two members in party B");
    assert_eq!(r.read_string().unwrap(), "P3003", "leader first");
}

#[test]
fn cc_chat_channels_gate_on_leadership() {
    let (mut world, ..) = test_world();
    let mut rxs = two_parties(&mut world);
    form_channel(&mut world);
    for rx in &mut rxs {
        drain(rx);
    }
    let commander = crate::enums::ChatType::PartyroomCommander.client_id();
    let all = crate::enums::ChatType::PartyroomAll.client_id();

    // Channel 15: only the CC leader; every member of every party hears it.
    chat::handle_say2(&mut world, 1, &say2_body("go", commander, None));
    for rx in &mut rxs {
        assert!(
            drain(rx).iter().any(|p| p[0] == opcodes::SAY2),
            "every CC member hears the commander line"
        );
    }
    chat::handle_say2(&mut world, 3, &say2_body("no", commander, None));
    assert!(
        drain(&mut rxs[0]).is_empty(),
        "a party leader who isn't the CC leader can't use channel 15"
    );

    // Channel 16: any party leader, but not a plain member.
    chat::handle_say2(&mut world, 3, &say2_body("ok", all, None));
    assert!(drain(&mut rxs[1]).iter().any(|p| p[0] == opcodes::SAY2));
    for rx in &mut rxs {
        drain(rx);
    }
    chat::handle_say2(&mut world, 4, &say2_body("no", all, None));
    assert!(drain(&mut rxs[0]).is_empty(), "member may not speak on 16");
}

#[test]
fn mpcc_room_lifecycle() {
    let (mut world, ..) = test_world();
    let mut rxs = two_parties(&mut world);
    form_channel(&mut world);
    let mut solo_rx = ingame_player(&mut world, 5, 3005, 0, 0, 0);
    for rx in &mut rxs {
        drain(rx);
    }
    fn req_party_match_cfg() -> Vec<u8> {
        [vec![cop::REQUEST_PARTY_MATCH_CONFIG], {
            let mut w = PacketWriter::new();
            w.write_i32(1);
            w.write_i32(-1);
            w.write_i32(1);
            w.into_bytes()
        }]
        .concat()
    }
    drain(&mut solo_rx);

    // A channelled non-CC-leader may not use the matching screen.
    on_packet(&mut world, 3, req_party_match_cfg());
    assert!(ids_after_opcode(&drain(&mut rxs[2]), opcodes::SYSTEM_MESSAGE).contains(
        &sm_ids::THE_COMMAND_CHANNEL_AFFILIATED_PARTY_S_PARTY_MEMBER_CANNOT_USE_THE_MATCHING_SCREEN
    ));

    // The CC leader opening the board creates the MPCC room.
    on_packet(&mut world, 1, req_party_match_cfg());
    let leader_pkts = drain(&mut rxs[0]);
    assert!(
        ids_after_opcode(&leader_pkts, opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::THE_COMMAND_CHANNEL_MATCHING_ROOM_WAS_CREATED)
    );
    assert!(
        leader_pkts
            .iter()
            .any(|p| is_ex(p, opcodes::EX_MPCC_ROOM_INFO))
    );
    assert!(
        leader_pkts
            .iter()
            .any(|p| is_ex(p, opcodes::EX_MPCC_ROOM_MEMBER))
    );
    let room_id = world.matching_rooms.room_id_of(3001).expect("room exists");
    assert_eq!(
        world.matching_rooms.get(room_id).unwrap().kind,
        model::matching_room::RoomKind::CommandChannel
    );
    assert_eq!(
        world.matching_rooms.get(room_id).unwrap().max_members,
        50,
        "hardcoded CC-room cap"
    );

    // The CC room is invisible to the party-room browser…
    assert!(
        world
            .matching_rooms
            .find_rooms(-1, model::matching_room::RoomLevelFilter::All, 1, |_| 0)
            .is_empty()
    );

    // …and shows in the MPCC browser (exact-location match).
    let leader_location = rooms::location_of(&world, 3001);
    on_packet(
        &mut world,
        5,
        ex_packet(cp::ex_opcodes::REQUEST_EX_LIST_MPCC_WAITING, &{
            let mut w = PacketWriter::new();
            w.write_i32(1); // page
            w.write_i32(leader_location);
            w.write_i32(1); // level filter value
            w.into_bytes()
        }),
    );
    let list = drain(&mut solo_rx)
        .into_iter()
        .find(|p| is_ex(p, opcodes::EX_LIST_MPCC_WAITING))
        .expect("CC room list");
    let mut r = commons::network::PacketReader::new(&list[3..]);
    assert_eq!(r.read_i32().unwrap(), 1, "one room total");

    // A solo player joins: add-row + SM 3003 to the room, window to joiner.
    on_packet(
        &mut world,
        5,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_JOIN_MPCC_ROOM,
            &i32_body(room_id),
        ),
    );
    let joiner_pkts = drain(&mut solo_rx);
    assert!(
        joiner_pkts
            .iter()
            .any(|p| is_ex(p, opcodes::EX_MPCC_ROOM_INFO))
    );
    assert!(
        joiner_pkts
            .iter()
            .any(|p| is_ex(p, opcodes::EX_MPCC_ROOM_MEMBER))
    );
    let leader_pkts = drain(&mut rxs[0]);
    assert!(
        leader_pkts
            .iter()
            .any(|p| is_ex(p, opcodes::EX_MANAGE_PARTY_ROOM_MEMBER))
    );
    assert!(
        ids_after_opcode(&leader_pkts, opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::C1_ENTERED_THE_COMMAND_CHANNEL_MATCHING_ROOM)
    );

    // The member-type rows: leader = 3 (CC leader), solo joiner = 6 (no party).
    let member_pkt = joiner_pkts
        .iter()
        .find(|p| is_ex(p, opcodes::EX_MPCC_ROOM_MEMBER))
        .unwrap();
    let mut r = commons::network::PacketReader::new(&member_pkt[3..]);
    assert_eq!(r.read_i32().unwrap(), 6, "recipient type: no party");
    assert_eq!(r.read_i32().unwrap(), 2, "two in the room");

    // Withdraw: SM 2997 and back on the waiting list.
    on_packet(
        &mut world,
        5,
        ex_packet(cp::ex_opcodes::REQUEST_EX_WITHDRAW_MPCC_ROOM, &[]),
    );
    assert!(
        ids_after_opcode(&drain(&mut solo_rx), opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::YOU_EXITED_FROM_THE_COMMAND_CHANNEL_MATCHING_ROOM)
    );
    assert!(world.matching_rooms.is_waiting(3005));
    drain(&mut rxs[0]);

    // Dismiss: SM 2994 + ExDissmissMPCCRoom, room gone.
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_EX_DISMISS_MPCC_ROOM, &[]),
    );
    let pkts = drain(&mut rxs[0]);
    assert!(
        ids_after_opcode(&pkts, opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::THE_COMMAND_CHANNEL_MATCHING_ROOM_WAS_CANCELLED)
    );
    assert!(
        pkts.iter()
            .any(|p| is_ex(p, opcodes::EX_DISSMISS_MPCC_ROOM))
    );
    assert!(world.matching_rooms.room_id_of(3001).is_none());
}

/// Register `npc_id` with one guaranteed drop of `drop_item` and spawn it.
fn spawn_dropper(world: &mut World, npc_oid: i32, npc_id: i32, type_name: &str, drop_item: i32) {
    let mut t = crate::data::npc_data::default_template(npc_id);
    t.type_name = type_name.into();
    t.level = 20;
    t.base_hp_max = 100.0;
    t.base_mp_max = 50.0;
    t.drop_groups.push(crate::data::npc_data::DropGroup {
        chance: 100.0,
        items: vec![crate::data::npc_data::DropHolder {
            item_id: drop_item,
            min: 1,
            max: 1,
            chance: 100.0,
        }],
    });
    world.data.npc_data.insert_for_test(t);
    world
        .data
        .item_data
        .insert_for_test(crate::data::item_data::ItemTemplate {
            item_id: drop_item,
            name: "Raid Loot".into(),
            kind: crate::data::item_data::ItemKind::Etc,
            is_stackable: false,
            is_infinite: false,
            is_sellable: true,
            is_freightable: false,
            price: 10,
            ..Default::default()
        });
    add_test_npc(world, npc_oid, npc_id, type_name, 20, 0, 0, 0);
}

fn the_ground_item(world: &World) -> i32 {
    let ids: Vec<i32> = world
        .ground_item_regions
        .values()
        .flat_map(|v| v.iter().copied())
        .collect();
    assert_eq!(ids.len(), 1, "exactly one ground drop expected");
    ids[0]
}

/// A ≥`RaidLootRightsCCSize` command channel that strikes a raid boss first
/// claims looting rights ("You have looting rights!" on channel 16), the kill
/// drops to the ground owned by the CC leader (`AutoLootRaids` off overrides
/// `AutoLoot` on), and only channel members may pick the drop up.
#[test]
fn raid_loot_rights_protect_the_drop_for_the_channel() {
    let (mut world, ..) = test_world();
    world.cfg.character.auto_loot = true;
    world.cfg.character.auto_loot_raids = false;
    world.cfg.character.raid_loot_rights_cc_size = 4;
    let mut rxs = two_parties(&mut world);
    form_channel(&mut world);
    let mut outsider_rx = ingame_player(&mut world, 5, 3005, 0, 0, 0);
    spawn_dropper(&mut world, 9100, 39100, "RaidBoss", 9550);
    for rx in &mut rxs {
        drain(rx);
    }
    drain(&mut outsider_rx);

    // First CC hit: the claim + the announcement to every channel member.
    combat::npc_receive_damage(&mut world, 9100, 3004, 10.0, false);
    assert_eq!(
        command_channel::loot_rights_cc(&world, 9100),
        command_channel::party_id_of(&world, 3001)
            .and_then(|pid| command_channel::cc_id_of_party(&world, pid)),
        "the channel holds the claim"
    );
    for rx in &mut rxs {
        let pkts = drain(rx);
        let say = pkts
            .iter()
            .find(|p| p[0] == opcodes::SAY2)
            .expect("looting-rights announcement");
        let (oid, ty, _, text, _) = parse_creature_say(say);
        assert_eq!((oid, ty), (0, 16), "sender 0 on PARTYROOM_ALL");
        assert_eq!(text, "You have looting rights!");
    }

    // Kill: the drop lands on the ground (AutoLootRaids=False beats
    // AutoLoot=True) owned by the CC leader.
    combat::npc_receive_damage(&mut world, 9100, 3004, 1_000_000.0, false);
    let ground = the_ground_item(&world);
    let g = world
        .objects
        .get_component::<model::components::GroundItem>(&ground)
        .unwrap()
        .clone();
    assert_eq!(g.owner_id, 3001, "owned by the CC leader");
    assert!(g.owner_until_tick > world.tick);
    for rx in &mut rxs {
        drain(rx);
    }

    // An outsider may not take it: ActionFailed + SM 56, item stays.
    ground_items::pickup_ground_item(&mut world, 5, 3005, ground);
    let pkts = drain(&mut outsider_rx);
    assert!(pkts.iter().any(|p| p[0] == opcodes::ACTION_FAIL));
    assert!(
        ids_after_opcode(&pkts, opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::YOU_HAVE_FAILED_TO_PICK_UP_S1)
    );
    assert!(
        world
            .objects
            .get_component::<model::components::GroundItem>(&ground)
            .is_some()
    );

    // A member of the *other* CC party may (isInLooterParty via the channel).
    ground_items::pickup_ground_item(&mut world, 4, 3004, ground);
    assert!(
        world
            .objects
            .get_component::<model::components::GroundItem>(&ground)
            .is_none()
    );
    assert_eq!(count_of_item(&world, 3004, 9550), 1);
}

/// An ordinary kill with `AutoLoot` off drops killer-owned loot: strangers are
/// refused for 15 s, party members and the owner pass, and the protection
/// lapses afterwards.
#[test]
fn ordinary_drop_is_killer_protected_for_15s() {
    let (mut world, ..) = test_world();
    world.cfg.character.auto_loot = false;
    world.id_pool = 0x2100_0000..0x2100_1000;
    let mut killer_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut stranger_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    spawn_dropper(&mut world, 9101, 39101, "Monster", 9551);
    drain(&mut killer_rx);
    drain(&mut stranger_rx);

    combat::npc_receive_damage(&mut world, 9101, 3001, 1_000_000.0, false);
    let ground = the_ground_item(&world);
    let g = world
        .objects
        .get_component::<model::components::GroundItem>(&ground)
        .unwrap()
        .clone();
    assert_eq!(g.owner_id, 3001, "killer-owned");
    assert_eq!(g.owner_until_tick, world.tick + 150, "15 s protection");

    // The stranger is refused while protected…
    ground_items::pickup_ground_item(&mut world, 2, 3002, ground);
    assert!(
        ids_after_opcode(&drain(&mut stranger_rx), opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::YOU_HAVE_FAILED_TO_PICK_UP_S1)
    );
    // …and succeeds once the 15 s lapse.
    advance_ticks(&mut world, 151);
    ground_items::pickup_ground_item(&mut world, 2, 3002, ground);
    assert_eq!(count_of_item(&world, 3002, 9551), 1);
}

/// `AltCommandChannelFriends` — "Same Command Channel are friends". Java places
/// the check immediately after the peace-zone arm of `Player.isAutoAttackable`,
/// *ahead* of the flag/PK arms, so two parties raiding together cannot hit each
/// other even when one of them is a criminal.
///
/// The port had no such check at all: the key is `True` on this dist, so the
/// omission was live.
#[test]
fn same_command_channel_members_cannot_attack_each_other() {
    use crate::game_loop::combat::pvp::is_player_auto_attackable;

    let (mut world, ..) = test_world();
    let _rxs = two_parties(&mut world);

    // Make 3003 a PK so the baseline answer is "attackable" for a reason that
    // has nothing to do with zones or parties.
    world
        .objects
        .get_component_mut::<Player>(&3003)
        .unwrap()
        .reputation = -100;
    assert!(
        is_player_auto_attackable(&world, 3001, 3003),
        "a PK in another party is attackable before the channel forms"
    );

    form_channel(&mut world);
    assert!(
        !is_player_auto_attackable(&world, 3001, 3003),
        "two parties in the same command channel are friends"
    );
    // …and it really is the *channel*, not the PK flag going away.
    world.cfg.character.alt_command_channel_friends = false;
    assert!(
        is_player_auto_attackable(&world, 3001, 3003),
        "turning the key off restores the attack"
    );
}
