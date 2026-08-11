use super::*;
use crate::game_loop::party;

/// General chat reaches the speaker and players within 1250 units, but not a
/// region-adjacent player standing further away.
#[test]
fn general_chat_is_scoped_to_1250_units() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 1000, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 2000, 0, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body("hello there", 0, None)].concat(),
    );

    let a_pkts = drain(&mut a_rx);
    assert_eq!(a_pkts.len(), 1, "speaker gets exactly the echo");
    let (oid, ty, name, text, tail) = parse_creature_say(&a_pkts[0]);
    assert_eq!((oid, ty), (3001, 0));
    assert_eq!(name, "P3001");
    assert_eq!(text, "hello there");
    assert!(tail.is_none(), "no whisper tail on general chat");

    let b_pkts = drain(&mut b_rx);
    assert_eq!(b_pkts.len(), 1, "in-range bystander hears it");
    assert!(
        drain(&mut c_rx).is_empty(),
        "1250+ units away hears nothing"
    );
}

/// Whisper: case-insensitive name lookup, receiver gets the message with the
/// relation-mask tail (mask 0 + sender level), sender gets the `->Name` echo;
/// whispering to a name that isn't online answers SM 145.
#[test]
fn whisper_delivers_echoes_and_rejects_offline() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 500, 0, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body("psst", 2, Some("p3002"))].concat(),
    );

    let b_pkts = drain(&mut b_rx);
    assert_eq!(b_pkts.len(), 1);
    let (oid, ty, name, text, tail) = parse_creature_say(&b_pkts[0]);
    assert_eq!((oid, ty), (3001, 2));
    assert_eq!(name, "P3001");
    assert_eq!(text, "psst");
    assert_eq!(tail, Some((0, 1)), "mask 0 + sender level 1");

    let a_pkts = drain(&mut a_rx);
    assert_eq!(a_pkts.len(), 1);
    let (_, _, echo_name, _, echo_tail) = parse_creature_say(&a_pkts[0]);
    assert_eq!(echo_name, "->P3002");
    assert_eq!(echo_tail, Some((0, 1)));

    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body("psst", 2, Some("nobody"))].concat(),
    );
    let a_pkts = drain(&mut a_rx);
    assert_eq!(a_pkts.len(), 1);
    assert_eq!(
        sm_id(&a_pkts[0]),
        server_packets::sm_ids::THAT_PLAYER_IS_NOT_ONLINE
    );
}

/// Shout/trade use map-region buckets; with no map regions loaded everyone
/// shares Java's fallback bucket, so even a far player hears it. Party/clan
/// chat without the group answers the "you are not in a …" SMs, and an
/// over-long line gets the spam warning.
#[test]
fn shout_reaches_region_bucket_and_groupless_chats_reject() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 60_000, 0, 0);
    drain(&mut a_rx);
    drain(&mut c_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body("WTS stuff", 1, None)].concat(),
    );
    let c_pkts = drain(&mut c_rx);
    assert_eq!(c_pkts.len(), 1, "same (empty) map-region bucket");
    let (_, ty, _, _, _) = parse_creature_say(&c_pkts[0]);
    assert_eq!(ty, 1);
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body("anyone?", 3, None)].concat(),
    );
    let a_pkts = drain(&mut a_rx);
    assert_eq!(
        sm_id(&a_pkts[0]),
        server_packets::sm_ids::YOU_ARE_NOT_IN_A_PARTY
    );

    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body("hi clan", 4, None)].concat(),
    );
    let a_pkts = drain(&mut a_rx);
    assert_eq!(
        sm_id(&a_pkts[0]),
        server_packets::sm_ids::YOU_ARE_NOT_IN_A_CLAN
    );

    let long = "x".repeat(106);
    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body(&long, 0, None)].concat(),
    );
    let a_pkts = drain(&mut a_rx);
    assert_eq!(
        sm_id(&a_pkts[0]),
        server_packets::sm_ids::KEYBOARD_INPUT_SPAM_WARNING
    );
    assert!(
        drain(&mut c_rx).is_empty(),
        "rejected line is not broadcast"
    );
}

/// `UserInfo.calculateRelation` (via `party::calculate_relation`): the party
/// and clan bits, driven off the `PartyRef` component and the `Player`'s clan
/// fields. The siege bit (0x80) is unported, so it never sets.
#[test]
fn relation_reflects_party_and_clan() {
    let (mut world, ..) = test_world();
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    let snapshot = |w: &World, oid: i32| w.objects.get_component::<Player>(&oid).unwrap().clone();

    // Solo, clanless → 0.
    assert_eq!(
        party::calculate_relation(&world, &snapshot(&world, 3001)),
        0
    );

    // Clan member + leader → 0x20 | 0x40.
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.clan_id = 7;
        p.clan_leader = true;
    }
    assert_eq!(
        party::calculate_relation(&world, &snapshot(&world, 3001)),
        0x20 | 0x40
    );

    // Clan member, not leader → 0x20.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_leader = false;
    assert_eq!(
        party::calculate_relation(&world, &snapshot(&world, 3001)),
        0x20
    );

    // Party leader (3001 first) → adds 0x08 | 0x10; the non-leader member
    // (3002, clanless) gets 0x08 only.
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    assert_eq!(
        party::calculate_relation(&world, &snapshot(&world, 3001)),
        0x20 | 0x08 | 0x10
    );
    assert_eq!(
        party::calculate_relation(&world, &snapshot(&world, 3002)),
        0x08
    );
}

/// The invite → accept happy path: SM 105 + AskJoinParty, then JoinParty(1),
/// the window packets on both sides, the joined SMs, and a live party with
/// the leader first.
#[test]
fn party_invite_accept_builds_party_and_windows() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::REQUEST_JOIN_PARTY], join_party_body("p3002", 1)].concat(),
    );
    let a_pkts = drain(&mut a_rx);
    assert_eq!(
        sm_ids_of(&a_pkts),
        vec![server_packets::sm_ids::C1_HAS_BEEN_INVITED_TO_THE_PARTY]
    );
    let b_pkts = drain(&mut b_rx);
    assert_eq!(b_pkts.len(), 1);
    assert_eq!(b_pkts[0][0], server_packets::opcodes::ASK_JOIN_PARTY);
    {
        let mut r = commons::network::PacketReader::new(&b_pkts[0][1..]);
        assert_eq!(r.read_string().unwrap(), "P3001");
        assert_eq!(r.read_i32().unwrap(), 1, "Random loot rule echoed");
    }
    assert!(world.objects.has_component::<PendingRequest>(&3001));
    assert!(world.objects.has_component::<PendingRequest>(&3002));

    on_packet(
        &mut world,
        2,
        [vec![cop::REQUEST_ANSWER_JOIN_PARTY], int_body(1)].concat(),
    );

    let a_pkts = drain(&mut a_rx);
    assert!(
        has_opcode(&a_pkts, server_packets::opcodes::JOIN_PARTY),
        "JoinParty echo"
    );
    assert!(
        has_opcode(&a_pkts, server_packets::opcodes::PARTY_SMALL_WINDOW_ADD),
        "leader window gains the member"
    );
    assert!(sm_ids_of(&a_pkts).contains(&server_packets::sm_ids::C1_HAS_JOINED_THE_PARTY));

    let b_pkts = drain(&mut b_rx);
    let all = b_pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::PARTY_SMALL_WINDOW_ALL)
        .expect("window all");
    {
        let mut r = commons::network::PacketReader::new(&all[1..]);
        assert_eq!(r.read_i32().unwrap(), 3001, "leader object id");
        assert_eq!(r.read_u8().unwrap(), 1, "loot rule byte");
        assert_eq!(r.read_u8().unwrap(), 1, "one other member");
        assert_eq!(r.read_i32().unwrap(), 3001, "the leader's entry");
    }
    let b_sms = sm_ids_of(&b_pkts);
    assert!(b_sms.contains(&server_packets::sm_ids::YOU_HAVE_JOINED_S1_S_PARTY));
    assert!(b_sms.contains(&server_packets::sm_ids::C1_HAS_JOINED_THE_PARTY));

    assert_eq!(world.parties.len(), 1);
    let party = world.parties.values().next().unwrap();
    assert_eq!(party.members, vec![3001, 3002]);
    assert!(!party.pending_invitation);
    let a_ref = world
        .objects
        .get_component::<PartyRef>(&3001)
        .copied()
        .unwrap();
    let b_ref = world
        .objects
        .get_component::<PartyRef>(&3002)
        .copied()
        .unwrap();
    assert_eq!(a_ref, b_ref);
    assert!(
        !world.objects.has_component::<PendingRequest>(&3001),
        "request cleared"
    );
}

/// Declining the first invite answers JoinParty(0) and dissolves the embryo
/// party; guards: target busy (SM 153 via second inviter), target already in
/// a party (SM 160), non-leader invites (SM 154).
#[test]
fn party_invite_decline_and_guards() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 200, 0, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    // A invites B; C tries to invite the busy B → SM 153 after the SM 105.
    on_packet(
        &mut world,
        1,
        [vec![cop::REQUEST_JOIN_PARTY], join_party_body("P3002", 0)].concat(),
    );
    on_packet(
        &mut world,
        3,
        [vec![cop::REQUEST_JOIN_PARTY], join_party_body("P3002", 0)].concat(),
    );
    let c_sms = sm_ids_of(&drain(&mut c_rx));
    assert!(
        c_sms.contains(&server_packets::sm_ids::WAITING_FOR_ANOTHER_REPLY),
        "busy target: {c_sms:?}"
    );

    // B declines: A gets JoinParty(0), the embryo party dies.
    drain(&mut a_rx);
    on_packet(
        &mut world,
        2,
        [vec![cop::REQUEST_ANSWER_JOIN_PARTY], int_body(0)].concat(),
    );
    let a_pkts = drain(&mut a_rx);
    let jp = a_pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::JOIN_PARTY)
        .expect("JoinParty");
    assert_eq!(
        i32::from_le_bytes(jp[1..5].try_into().unwrap()),
        0,
        "declined"
    );
    assert!(world.parties.is_empty(), "embryo party dissolved");
    assert!(!world.objects.has_component::<PartyRef>(&3001));

    // Formed party: B (not leader) inviting → SM 154; inviting a partied
    // player → SM 160.
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    drain(&mut b_rx);
    on_packet(
        &mut world,
        2,
        [vec![cop::REQUEST_JOIN_PARTY], join_party_body("P3003", 0)].concat(),
    );
    let b_sms = sm_ids_of(&drain(&mut b_rx));
    assert!(
        b_sms.contains(&server_packets::sm_ids::ONLY_THE_LEADER_CAN_GIVE_OUT_INVITATIONS),
        "{b_sms:?}"
    );
    drain(&mut c_rx);
    on_packet(
        &mut world,
        3,
        [vec![cop::REQUEST_JOIN_PARTY], join_party_body("P3002", 0)].concat(),
    );
    let c_sms = sm_ids_of(&drain(&mut c_rx));
    assert!(
        c_sms.contains(
            &server_packets::sm_ids::C1_IS_A_MEMBER_OF_ANOTHER_PARTY_AND_CANNOT_BE_INVITED
        ),
        "{c_sms:?}"
    );
}

/// An unanswered invite times out after 30 s: both request slots clear and
/// the embryo party is dropped.
#[test]
fn party_invite_timeout_drops_embryo_party() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::REQUEST_JOIN_PARTY], join_party_body("P3002", 0)].concat(),
    );
    assert_eq!(world.parties.len(), 1);
    advance_ticks(&mut world, 301);
    assert!(!world.objects.has_component::<PendingRequest>(&3001));
    assert!(!world.objects.has_component::<PendingRequest>(&3002));
    assert!(world.parties.is_empty(), "unanswered embryo party dropped");
}

/// Leaving a 2-member party disbands it (SM 203 + window clear on both).
#[test]
fn party_withdrawal_two_members_disbands() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(&mut world, 2, vec![cop::REQUEST_WITH_DRAWAL_PARTY]);
    for rx in [&mut a_rx, &mut b_rx] {
        let pkts = drain(rx);
        assert!(sm_ids_of(&pkts).contains(&server_packets::sm_ids::THE_PARTY_HAS_DISPERSED));
        assert!(has_opcode(
            &pkts,
            server_packets::opcodes::PARTY_SMALL_WINDOW_DELETE_ALL
        ));
    }
    assert!(world.parties.is_empty());
    assert!(!world.objects.has_component::<PartyRef>(&3001));
    assert!(!world.objects.has_component::<PartyRef>(&3002));
}

/// A 3-member party: the leader disconnecting transfers leadership (SM 1384 +
/// window rebuild), ousting sends SM 202/201 + the delete entry, and
/// `RequestChangePartyLeader` swaps slot 0.
#[test]
fn party_leadership_oust_and_change() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 200, 0, 0);
    let party_id = make_party(&mut world, &[3001, 3002, 3003], LootRule::FindersKeepers);
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    // Leader disconnects → B becomes leader.
    world.clients.remove(&1);
    store_and_remove_player(&mut world, 3001);
    let b_pkts = drain(&mut b_rx);
    assert!(sm_ids_of(&b_pkts).contains(&server_packets::sm_ids::C1_HAS_BECOME_THE_PARTY_LEADER));
    assert!(
        has_opcode(&b_pkts, server_packets::opcodes::PARTY_SMALL_WINDOW_ALL),
        "window rebuilt"
    );
    assert_eq!(world.parties[&party_id].members, vec![3002, 3003]);

    // New leader B ousts C → 2-member party disbands instead (the 2-left
    // rule), C sees the expelled SM.
    on_packet(
        &mut world,
        2,
        [vec![cop::REQUEST_OUST_PARTY_MEMBER], name_body("P3003")].concat(),
    );
    let c_pkts = drain(&mut c_rx);
    assert!(sm_ids_of(&c_pkts).contains(&server_packets::sm_ids::THE_PARTY_HAS_DISPERSED));
    assert!(world.parties.is_empty());

    // Fresh 3-member party exercises oust + leader change proper.
    let mut a2_rx = ingame_player(&mut world, 1, 3004, 0, 0, 0);
    let party_id = make_party(&mut world, &[3004, 3002, 3003], LootRule::FindersKeepers);
    drain(&mut a2_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);
    on_packet(
        &mut world,
        1,
        [vec![cop::REQUEST_OUST_PARTY_MEMBER], name_body("P3002")].concat(),
    );
    let b_pkts = drain(&mut b_rx);
    assert!(
        sm_ids_of(&b_pkts).contains(&server_packets::sm_ids::YOU_HAVE_BEEN_EXPELLED_FROM_THE_PARTY)
    );
    assert!(has_opcode(
        &b_pkts,
        server_packets::opcodes::PARTY_SMALL_WINDOW_DELETE_ALL
    ));
    let c_pkts = drain(&mut c_rx);
    assert!(sm_ids_of(&c_pkts).contains(&server_packets::sm_ids::C1_WAS_EXPELLED_FROM_THE_PARTY));
    assert!(has_opcode(
        &c_pkts,
        server_packets::opcodes::PARTY_SMALL_WINDOW_DELETE
    ));
    assert_eq!(world.parties[&party_id].members, vec![3004, 3003]);

    // Change leader to C; a repeat naming the (new) leader → SM 1401 quirk
    // (sent to the requestor, who is no longer leader → silently ignored, so
    // name the current leader from the current leader instead).
    on_packet(&mut world, 1, ex_packet(0x0C, &name_body("P3003")));
    assert_eq!(world.parties[&party_id].members, vec![3003, 3004]);
    drain(&mut c_rx);
    on_packet(&mut world, 3, ex_packet(0x0C, &name_body("P3003")));
    let c_sms = sm_ids_of(&drain(&mut c_rx));
    assert!(
        c_sms.contains(&server_packets::sm_ids::SLOW_DOWN_YOU_ARE_ALREADY_THE_PARTY_LEADER),
        "{c_sms:?}"
    );
}

/// Loot-rule voting: unanimous yes applies the rule (ExSetPartyLooting(1) +
/// SM 3138), the 15 s timeout cancels (ExSetPartyLooting(0) + SM 3137).
#[test]
fn party_loot_change_vote_and_timeout() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 200, 0, 0);
    let party_id = make_party(&mut world, &[3001, 3002, 3003], LootRule::FindersKeepers);
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    // Leader proposes Random (1): members get the FE:C0 ask, leader SM 3135.
    on_packet(&mut world, 1, ex_packet(0x75, &int_body(1)));
    assert!(ex_subs_of(&drain(&mut b_rx)).contains(&0xC0));
    assert!(
        sm_ids_of(&drain(&mut a_rx))
            .contains(&server_packets::sm_ids::REQUESTING_APPROVAL_FOR_CHANGING_PARTY_LOOT_TO_S1)
    );

    // Both members agree → applied everywhere.
    on_packet(&mut world, 2, ex_packet(0x76, &int_body(1)));
    on_packet(&mut world, 3, ex_packet(0x76, &int_body(1)));
    let a_pkts = drain(&mut a_rx);
    assert!(ex_subs_of(&a_pkts).contains(&0xC1), "ExSetPartyLooting");
    assert!(sm_ids_of(&a_pkts).contains(&server_packets::sm_ids::PARTY_LOOT_WAS_CHANGED_TO_S1));
    assert_eq!(world.parties[&party_id].distribution, LootRule::Random);

    // Second proposal times out → cancelled, rule unchanged.
    on_packet(&mut world, 1, ex_packet(0x75, &int_body(3)));
    drain(&mut a_rx);
    advance_ticks(&mut world, 151);
    let a_pkts = drain(&mut a_rx);
    assert!(sm_ids_of(&a_pkts).contains(&server_packets::sm_ids::PARTY_LOOT_CHANGE_WAS_CANCELLED));
    assert_eq!(world.parties[&party_id].distribution, LootRule::Random);
}

/// Party chat reaches exactly the members (speaker echo included).
#[test]
fn party_chat_reaches_members_only() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 200, 0, 0);
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body("inc mob", 3, None)].concat(),
    );
    let (_, ty, _, text, _) = parse_creature_say(&drain(&mut b_rx)[0]);
    assert_eq!((ty, text.as_str()), (3, "inc mob"));
    assert_eq!(drain(&mut a_rx).len(), 1, "speaker echo");
    assert!(drain(&mut c_rx).is_empty(), "non-member hears nothing");
}

/// A party member taking damage pushes `PartySmallWindowUpdate` (vitals
/// flags) to the other members, not to themselves.
#[test]
fn party_vitals_piggyback_on_damage() {
    let (mut world, ..) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    drain(&mut a_rx);
    drain(&mut b_rx);

    combat::player_receive_damage(&mut world, 3002, 3001, 30.0);
    let a_pkts = drain(&mut a_rx);
    let upd = a_pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::PARTY_SMALL_WINDOW_UPDATE)
        .expect("window update");
    let mut r = commons::network::PacketReader::new(&upd[1..]);
    assert_eq!(r.read_i32().unwrap(), 3002, "the damaged member's entry");
    assert_eq!(
        r.read_i16().unwrap() as u16,
        server_packets::party_window_flags::VITALS
    );
    assert!(
        !has_opcode(
            &drain(&mut b_rx),
            server_packets::opcodes::PARTY_SMALL_WINDOW_UPDATE
        ),
        "not echoed to self"
    );
}

/// The 12 s position broadcast reaches every member and keeps rescheduling
/// while the party lives.
#[test]
fn party_position_broadcast_ticks() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    // Through the real flow so the broadcast task starts.
    on_packet(
        &mut world,
        1,
        [vec![cop::REQUEST_JOIN_PARTY], join_party_body("P3002", 0)].concat(),
    );
    on_packet(
        &mut world,
        2,
        [vec![cop::REQUEST_ANSWER_JOIN_PARTY], int_body(1)].concat(),
    );
    drain(&mut a_rx);
    drain(&mut b_rx);

    advance_ticks(&mut world, 61); // initial delay = period/2 = 6 s
    let a_pkts = drain(&mut a_rx);
    let pos = a_pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::PARTY_MEMBER_POSITION)
        .expect("positions");
    let mut r = commons::network::PacketReader::new(&pos[1..]);
    assert_eq!(r.read_i32().unwrap(), 2, "both members listed");

    advance_ticks(&mut world, 120);
    assert!(
        has_opcode(
            &drain(&mut b_rx),
            server_packets::opcodes::PARTY_MEMBER_POSITION
        ),
        "keeps ticking"
    );

    // Disband kills the task.
    on_packet(&mut world, 2, vec![cop::REQUEST_WITH_DRAWAL_PARTY]);
    drain(&mut a_rx);
    advance_ticks(&mut world, 240);
    assert!(
        !has_opcode(
            &drain(&mut a_rx),
            server_packets::opcodes::PARTY_MEMBER_POSITION
        ),
        "task died with the party"
    );
}

/// A party kill splits XP/SP with Java's math: both level-5 members in range,
/// killer deals all damage → base 2000 XP × 1.3 party bonus, level²-weighted
/// (equal levels → 1300 each); SP likewise (100 × 1.3 / 2 = 65).
#[test]
fn party_kill_splits_xp_and_sp() {
    let (mut world, mut _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    for oid in [3001, 3002] {
        world
            .objects
            .get_component_mut::<crate::model::Player>(&oid)
            .unwrap()
            .exp = 4000;
    }

    let npc_oid = NPC_OID + 21;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 100, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);
    world.forced_rolls.extend([0, 99, 10]); // hit, no crit, ±0 damage
    // Kill the drop roll chances deterministically: level-gap gate passes
    // (roll 0), drop chance fails (roll ~1.0 impossible via forced_rolls —
    // use the f64 hook by clearing the drop list instead).
    {
        let mut t = world.data.npc_data.get(40001).unwrap().clone();
        t.drop_list_death.clear();
        world.data.npc_data.insert_for_test(t);
    }
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    advance_world(&mut world, 12); // swing lands

    assert!(
        pvit(&world, npc_oid).dead || world.objects.get_component::<Vitals>(&npc_oid).is_none(),
        "monster died"
    );
    let a_exp = world
        .objects
        .get_component::<crate::model::Player>(&3001)
        .unwrap()
        .exp;
    let b_exp = world
        .objects
        .get_component::<crate::model::Player>(&3002)
        .unwrap()
        .exp;
    assert_eq!(a_exp, 4000 + 1300, "killer: 2000 × 1.3 bonus × 25/50");
    assert_eq!(
        b_exp,
        4000 + 1300,
        "idle member gets the same equal-level share"
    );
    let b_sp = world
        .objects
        .get_component::<crate::model::Player>(&3002)
        .unwrap()
        .sp;
    assert_eq!(b_sp, 65, "SP: 100 × 1.3 / 2");
}

/// `Party.distributeItem`: adena splits evenly among in-range members;
/// BY_TURN rotates the looter, skipping the out-of-range member.
#[test]
fn party_loot_split_and_rotation() {
    let (mut world, ..) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    let mut c_rx = ingame_caster(&mut world, 3, 3003, 99_000, 0); // out of range
    let party_id = make_party(&mut world, &[3001, 3002, 3003], LootRule::ByTurn);
    world
        .data
        .item_data
        .insert_for_test(crate::data::item_data::ItemTemplate {
            trade_flags: Default::default(),
            time: -1,
            duration: -1,
            immediate_effect: false,
            ex_immediate_effect: false,
            default_action: crate::data::item_data::ActionType::Other,
            item_id: 1234,
            name: "Test Loot".into(),
            kind: crate::data::item_data::ItemKind::Etc,
            body_part: 0,
            weight: 0,
            is_stackable: false,
            is_infinite: false,
            type1: 4,
            type2: 5,
            is_quest_item: false,
            is_sellable: true,
            is_freightable: false,
            price: 0,
            handler: crate::data::item_data::ItemHandler::None,
            crystal_type: crate::data::item_data::CrystalType::None,
            crystal_count: 0,
            attack_radius: 40,
            attack_angle: 0,
            mp_consume: 0,
            reduced_mp_consume: 0,
            reduced_mp_consume_chance: 0,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(),
            etc_item_type: crate::data::item_data::EtcItemType::Other,
            enchant_enabled: false,
            enchant_limit: 0,
            is_magic_weapon: false,
        });
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    // Adena: 100 split across the 2 in-range members → 50 each.
    party::distribute_item(&mut world, party_id, 3001, 57, 100, (0, 0));
    let count_of = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&oid)
            .map(|inv| {
                inv.items()
                    .iter()
                    .filter(|i| i.item_id == 57)
                    .map(|i| i.count)
                    .sum::<i64>()
            })
            .unwrap_or(0)
    };
    assert_eq!(count_of(&world, 3001), 50);
    assert_eq!(count_of(&world, 3002), 50);
    assert_eq!(
        count_of(&world, 3003),
        0,
        "out-of-range member gets nothing"
    );

    // BY_TURN: cursor starts at 0 → first item to member index 1 (3002),
    // next wraps past out-of-range 3003 back to 3001.
    party::distribute_item(&mut world, party_id, 3001, 1234, 1, (0, 0));
    party::distribute_item(&mut world, party_id, 3001, 1234, 1, (0, 0));
    let has_item = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&oid)
            .is_some_and(|inv| inv.items().iter().any(|i| i.item_id == 1234))
    };
    assert!(has_item(&world, 3002), "first by-turn item");
    assert!(has_item(&world, 3001), "rotation skipped the far member");
    assert!(!has_item(&world, 3003));
    // The non-looting members saw the "C1 has obtained" line.
    assert!(sm_ids_of(&drain(&mut b_rx)).contains(&server_packets::sm_ids::C1_HAS_OBTAINED_S2));
}

/// Invite → accept: FriendAddRequest popup, both sides' SMs +
/// FriendAddRequestResult, both lists updated, one DB pair insert; the
/// whisper relation mask then carries the friend bit.
#[test]
fn friend_invite_accept_and_whisper_mask() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::REQUEST_FRIEND_INVITE], name_body("p3002")].concat(),
    );
    assert!(
        sm_ids_of(&drain(&mut a_rx))
            .contains(&server_packets::sm_ids::YOU_VE_REQUESTED_C1_TO_BE_ON_YOUR_FRIENDS_LIST)
    );
    assert!(has_opcode(
        &drain(&mut b_rx),
        server_packets::opcodes::FRIEND_ADD_REQUEST
    ));

    on_packet(
        &mut world,
        2,
        [
            vec![cop::REQUEST_ANSWER_FRIEND_INVITE],
            friend_answer_body(1),
        ]
        .concat(),
    );
    let a_pkts = drain(&mut a_rx);
    let a_sms = sm_ids_of(&a_pkts);
    assert!(
        a_sms.contains(&server_packets::sm_ids::FRIEND_ADDED_SUCCESSFULLY),
        "{a_sms:?}"
    );
    assert!(a_sms.contains(&server_packets::sm_ids::S1_HAS_BEEN_ADDED_TO_YOUR_FRIENDS_LIST));
    assert!(has_opcode(
        &a_pkts,
        server_packets::opcodes::FRIEND_ADD_REQUEST_RESULT
    ));
    let b_pkts = drain(&mut b_rx);
    assert!(
        sm_ids_of(&b_pkts)
            .contains(&server_packets::sm_ids::S1_HAS_BEEN_ADDED_TO_YOUR_FRIENDS_LIST_2)
    );
    assert!(has_opcode(
        &b_pkts,
        server_packets::opcodes::FRIEND_ADD_REQUEST_RESULT
    ));

    let a_friends = world.objects.get_component::<Friends>(&3001).unwrap();
    assert_eq!(a_friends.0.len(), 1);
    assert_eq!(
        (a_friends.0[0].char_id, a_friends.0[0].name.as_str()),
        (3002, "P3002")
    );
    assert_eq!(
        world.objects.get_component::<Friends>(&3002).unwrap().0[0].char_id,
        3001
    );

    let mut saw_insert = false;
    while let Ok(cmd) = db_rx.try_recv() {
        if let db::DbCommand::InsertFriendPair { a, b } = cmd {
            assert_eq!((a, b), (3001, 3002));
            saw_insert = true;
        }
    }
    assert!(saw_insert, "friendship persisted");

    // Whisper now carries the friend relation bit (receiver's view).
    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body("hey", 2, Some("P3002"))].concat(),
    );
    let (_, _, _, _, tail) = parse_creature_say(&drain(&mut b_rx)[0]);
    assert_eq!(tail, Some((0x01, 1)), "friend bit set");
}

/// Delete by name updates both sides' lists and rows; unknown names answer
/// SM 171. Friend messages deliver only when the receiver friended the
/// sender.
#[test]
fn friend_delete_and_messages() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    seed_friendship(&mut world, 3001, 3002);
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Friend message A → B (B has A friended).
    let mut msg = PacketWriter::new();
    msg.write_string("meet at giran");
    msg.write_string("P3002");
    on_packet(
        &mut world,
        1,
        [vec![cop::REQUEST_SEND_FRIEND_MSG], msg.into_bytes()].concat(),
    );
    let b_pkts = drain(&mut b_rx);
    let say = b_pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::L2_FRIEND_SAY)
        .expect("friend say");
    let mut r = commons::network::PacketReader::new(&say[1..]);
    r.read_i32().unwrap();
    assert_eq!(r.read_string().unwrap(), "P3002");
    assert_eq!(r.read_string().unwrap(), "P3001");
    assert_eq!(r.read_string().unwrap(), "meet at giran");

    // Delete: both lists + both clients + the DB pair.
    on_packet(
        &mut world,
        1,
        [vec![cop::REQUEST_FRIEND_DEL], name_body("p3002")].concat(),
    );
    let a_pkts = drain(&mut a_rx);
    assert!(
        sm_ids_of(&a_pkts)
            .contains(&server_packets::sm_ids::S1_HAS_BEEN_REMOVED_FROM_YOUR_FRIENDS_LIST_2)
    );
    assert!(has_opcode(&a_pkts, server_packets::opcodes::FRIEND_REMOVE));
    assert!(has_opcode(
        &drain(&mut b_rx),
        server_packets::opcodes::FRIEND_REMOVE
    ));
    assert!(
        world
            .objects
            .get_component::<Friends>(&3001)
            .unwrap()
            .0
            .is_empty()
    );
    assert!(
        world
            .objects
            .get_component::<Friends>(&3002)
            .unwrap()
            .0
            .is_empty()
    );
    let mut saw_delete = false;
    while let Ok(cmd) = db_rx.try_recv() {
        if let db::DbCommand::DeleteFriendPair { a, b } = cmd {
            assert_eq!((a, b), (3001, 3002));
            saw_delete = true;
        }
    }
    assert!(saw_delete);

    // Now strangers: the message bounces, delete answers SM 171.
    let mut msg = PacketWriter::new();
    msg.write_string("hello?");
    msg.write_string("P3002");
    on_packet(
        &mut world,
        1,
        [vec![cop::REQUEST_SEND_FRIEND_MSG], msg.into_bytes()].concat(),
    );
    assert!(
        sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::THAT_PLAYER_IS_NOT_ONLINE)
    );
    on_packet(
        &mut world,
        1,
        [vec![cop::REQUEST_FRIEND_DEL], name_body("P3002")].concat(),
    );
    assert!(
        sm_ids_of(&drain(&mut a_rx))
            .contains(&server_packets::sm_ids::C1_IS_NOT_ON_YOUR_FRIEND_LIST)
    );
}

/// Enter world sends the real `L2FriendList` and pings online friends with
/// SM 503 + `FriendStatus(ONLINE)`; leaving pings `FriendStatus(OFFLINE)`.
#[test]
fn friend_login_logout_notifications() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        // A has B friended (for display); B's own list drives the pings.
        let p = FriendInfo {
            char_id: 3002,
            name: "P3002".into(),
            level: 1,
            class_id: 0,
        };
        world
            .objects
            .get_component_mut::<Friends>(&3001)
            .unwrap()
            .0
            .push(p);
    }
    drain(&mut a_rx);

    // B enters the world with A on their friend list.
    let mut chr = dummy_char(3002, "P3002");
    chr.x = 100;
    chr.friends = vec![FriendInfo {
        char_id: 3001,
        name: "P3001".into(),
        level: 1,
        class_id: 0,
    }];
    let bundle = Player::from_char(&world.data, &chr);
    let (out_tx, mut b_rx) = tokio::sync::mpsc::unbounded_channel();
    let s = Session::new(2, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![])
        .into_entering(bundle);
    world.clients.insert(2, ClientSession::Entering(s));
    on_packet(&mut world, 2, vec![cop::ENTER_WORLD]);

    let b_pkts = drain(&mut b_rx);
    let list = b_pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::L2_FRIEND_LIST)
        .expect("L2FriendList");
    let mut r = commons::network::PacketReader::new(&list[1..]);
    assert_eq!(r.read_i32().unwrap(), 1, "one friend");
    assert_eq!(r.read_i32().unwrap(), 3001);
    assert_eq!(r.read_string().unwrap(), "P3001");
    assert_eq!(r.read_i32().unwrap(), 1, "online");

    let a_pkts = drain(&mut a_rx);
    assert!(sm_ids_of(&a_pkts).contains(&server_packets::sm_ids::YOUR_FRIEND_S1_JUST_LOGGED_IN));
    let status = a_pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::FRIEND_STATUS)
        .expect("FriendStatus");
    assert_eq!(
        i32::from_le_bytes(status[1..5].try_into().unwrap()),
        1,
        "MODE_ONLINE"
    );

    // B logs out → A gets the offline ping.
    on_packet(&mut world, 2, vec![cop::LOGOUT]);
    let a_pkts = drain(&mut a_rx);
    let status = a_pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::FRIEND_STATUS)
        .expect("offline ping");
    assert_eq!(
        i32::from_le_bytes(status[1..5].try_into().unwrap()),
        0,
        "MODE_OFFLINE"
    );
}

/// A whisper to a player in silence mode is refused — the sender gets the
/// refusal notice and nothing is delivered to the receiver.
#[test]
fn whisper_to_silenced_player_is_refused() {
    let (mut world, ..) = admin_world();
    let mut sender_rx = ingame_player_access(&mut world, 1, 6481, 0);
    let mut recv_rx = ingame_player_access(&mut world, 2, 6482, 100);
    drain(&mut sender_rx);
    drain(&mut recv_rx);
    let mut f = world
        .objects
        .get_component::<AdminFlags>(&6482)
        .copied()
        .unwrap_or_default();
    f.silence = true;
    world.objects.add_components(&6482, f);

    // Whisper (chat type 2) to "P6482".
    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body("hi", 2, Some("P6482"))].concat(),
    );
    let sender_pkts = drain(&mut sender_rx);
    assert!(
        has_system_message(&sender_pkts, 176),
        "sender told: person in refusal mode"
    );
    assert!(
        drain(&mut recv_rx)
            .iter()
            .all(|p| p[0] != server_packets::opcodes::SAY2),
        "silenced receiver got no whisper"
    );
}

// ---------------------------------------------------------------------------
// `Custom/*.ini` voiced commands + gates (the G33 audit's cheap slice)
// ---------------------------------------------------------------------------

/// `.online` reports the in-game population, with Java's singular/plural split.
/// Gated on `EnableOnlineCommand`: with the flag off the line is *said* instead,
/// because Java's `MasterHandler` never registers the handler.
#[test]
fn the_online_command_counts_players() {
    let (mut world, ..) = test_world();
    world.cfg.custom_misc.online_command = true;
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body(".online", 0, None)].concat(),
    );
    let out = drain(&mut a_rx);
    assert_eq!(
        out.iter().filter_map(|p| sysmsg_text(p)).next().as_deref(),
        Some("There is 1 player online!"),
        "one player → singular"
    );
    assert!(
        out.iter().all(|p| p[0] != server_packets::opcodes::SAY2),
        "a voiced command is never broadcast as chat"
    );

    // A second player flips it to the plural form.
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body(".online", 0, None)].concat(),
    );
    assert_eq!(
        drain(&mut a_rx)
            .iter()
            .filter_map(|p| sysmsg_text(p))
            .next()
            .as_deref(),
        Some("There are 2 players online!")
    );

    // Flag off → not a command any more, so it goes out as ordinary chat.
    world.cfg.custom_misc.online_command = false;
    drain(&mut a_rx);
    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body(".online", 0, None)].concat(),
    );
    assert!(
        drain(&mut a_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SAY2),
        "an unregistered voiced command is just chat"
    );
}

/// `.deposit` / `.withdraw` swap adena for goldbars at the configured rate, and
/// say so when the player is short.
#[test]
fn banking_swaps_adena_for_goldbars() {
    use crate::model::inventory::Inventory;
    const ADENA: i32 = 57;
    const GOLDBAR: i32 = 3470;

    let (mut world, ..) = test_world();
    world.id_pool = 0x7000_0000..0x7000_1000;
    // Both currencies must be stackable, or a million adena becomes a million
    // item instances.
    for (id, name) in [(ADENA, "Adena"), (GOLDBAR, "Goldbar")] {
        let mut t = crate::data::item_data::ItemTemplate::default();
        t.item_id = id;
        t.name = name.into();
        t.is_stackable = true;
        world.data.item_data.insert_for_test(t);
    }
    world.cfg.banking.enabled = true;
    world.cfg.banking.adena = 1_000_000;
    world.cfg.banking.goldbars = 1;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let count = |w: &World, id: i32| {
        w.objects
            .get_component::<Inventory>(&3001)
            .map_or(0, |i| i.count_of(id))
    };

    // Broke: the shortfall is explained, nothing moves.
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body(".deposit", 0, None)].concat(),
    );
    assert!(
        drain(&mut rx)
            .iter()
            .filter_map(|p| sysmsg_text(p))
            .any(|t| t.contains("do not have enough Adena")),
        "the shortfall is explained"
    );
    assert_eq!(count(&world, GOLDBAR), 0);

    // With the adena: one goldbar out, the adena gone.
    {
        let World { objects, data, .. } = &mut world;
        objects
            .get_component_mut::<Inventory>(&3001)
            .unwrap()
            .add_item(&data.item_data, 0x7000_0500, ADENA, 1_000_000);
    }
    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body(".deposit", 0, None)].concat(),
    );
    assert_eq!(count(&world, GOLDBAR), 1, "a goldbar was minted");
    assert_eq!(count(&world, ADENA), 0, "and the adena spent");

    // …and back again.
    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body(".withdraw", 0, None)].concat(),
    );
    assert_eq!(count(&world, GOLDBAR), 0);
    assert_eq!(
        count(&world, ADENA),
        1_000_000,
        "the round trip is lossless"
    );
}

/// `Config.L2WALKER_PROTECTION`: a **whisper** opening with an L2Walker verb
/// gets its sender kicked. Ordinary chat with the same text does not — Java
/// gates the check on `ChatType.WHISPER`.
#[test]
fn a_walker_whisper_kicks_the_sender() {
    let (mut world, ..) = test_world();
    world.cfg.custom_misc.walker_protection = true;
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    drain(&mut a_rx);

    // General chat with a bot verb is left alone.
    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body("USESKILL 1177", 0, None)].concat(),
    );
    assert!(world.clients.contains_key(&1), "still connected");

    // The same text whispered is an emulator giveaway — the punish task warns
    // at once and kicks 5 seconds later (Java `handleIllegalPlayerAction`).
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SAY2],
            say2_body("USESKILL 1177", 2, Some("P3002")),
        ]
        .concat(),
    );
    assert!(world.clients.contains_key(&1), "warned but not yet kicked");
    advance_ticks(&mut world, 51);
    assert!(!world.clients.contains_key(&1), "kicked");
}

/// `Custom/BossAnnouncements.ini` — a raid boss spawning is announced to
/// everyone, in chat and on screen. An ordinary monster is not, and a boss
/// spawned as somebody's **minion** is a reinforcement rather than an event.
#[test]
fn a_boss_spawn_is_announced_server_wide() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    for (id, ty, name) in [
        (25001, "RaidBoss", "Test Raid Boss"),
        (20001, "Monster", "Plain Mob"),
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = ty.into();
        t.name = name.into();
        t.base_hp_max = 100.0;
        world.data.npc_data.insert_for_test(t);
    }
    world.cfg.boss_announcements.raidboss_spawn = true;
    drain(&mut rx);

    // An ordinary mob is silent.
    crate::model::npc::spawn_npc_at(&mut world, 20001, 0, 0, 0, 0);
    assert!(
        drain(&mut rx).iter().all(|p| sysmsg_text(p).is_none()),
        "no line for a plain monster"
    );

    // The raid boss announces itself twice: chat + on screen.
    let boss = crate::model::npc::spawn_npc_at(&mut world, 25001, 0, 0, 0, 0).expect("spawned");
    let out = drain(&mut rx);
    assert!(
        out.iter().any(|p| p[0] == server_packets::opcodes::SAY2),
        "the chat line"
    );
    assert!(
        out.iter().any(|p| p[0] == server_packets::opcodes::EX),
        "and the on-screen one"
    );

    // A boss spawned **as a minion** stays quiet — suppression lives at the
    // call site, because `MinionOf` can only be attached after the entity
    // exists (see `spawn_minion_npc_at`).
    let _ = boss;
    drain(&mut rx);
    crate::model::npc::spawn_minion_npc_at(&mut world, 25001, 0, 0, 0, 0).expect("spawned");
    assert!(
        drain(&mut rx).iter().all(|p| sysmsg_text(p).is_none()),
        "a minion is a reinforcement, not an event"
    );

    world.cfg.boss_announcements.raidboss_spawn = false;
    drain(&mut rx);
    crate::model::npc::spawn_npc_at(&mut world, 25001, 0, 0, 0, 0);
    assert!(
        drain(&mut rx).iter().all(|p| sysmsg_text(p).is_none()),
        "the flag gates it"
    );
}

/// `Custom/PrivateStoreRange.ini` — a store may not open within
/// `ShopMinRangeFromNpc` of an NPC, nor within `ShopMinRangeFromPlayer` of
/// another **seated** player (Java's `getMinShopDistance` is zero unless
/// sitting, so a passer-by never blocks).
#[test]
fn a_private_store_needs_room_around_it() {
    use crate::game_loop::private_store::can_open_private_store;

    let (mut world, ..) = test_world();
    world.cfg.custom_misc.shop_min_range_from_npc = 100;
    world.cfg.custom_misc.shop_min_range_from_player = 50;
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    assert!(can_open_private_store(&world, 1, 3001), "open ground");

    // An NPC 60 units away is inside the 100-unit NPC spacing.
    add_test_npc(&mut world, 4001, 20001, "Npc", 5, 60, 0, 0);
    assert!(
        !can_open_private_store(&world, 1, 3001),
        "too close to an NPC"
    );

    // Move it out of range and the store is fine again.
    world
        .objects
        .get_component_mut::<crate::model::components::Position>(&4001)
        .unwrap()
        .x = 500;
    assert!(can_open_private_store(&world, 1, 3001), "NPC moved away");

    // A standing player 30 units away does not block…
    let _rx2 = ingame_player(&mut world, 2, 3002, 30, 0, 0);
    assert!(
        can_open_private_store(&world, 1, 3001),
        "a passer-by is not a shop"
    );
    // …but a seated one (i.e. one with a store up) does.
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .sitting = true;
    assert!(
        !can_open_private_store(&world, 1, 3001),
        "shops have to be spaced apart"
    );
}

/// `Custom/AllowedPlayerRaces.ini` — every race is allowed on this dist, so the
/// gate is inert; flipping one off refuses creation with
/// `CharCreateFail.REASON_CREATION_FAILED`.
#[test]
fn a_disallowed_race_cannot_be_created() {
    let (mut world, ..) = test_world();
    // Dark elf (race 2) barred; humans (0) still fine.
    world.cfg.allowed_races =
        crate::config::AllowedRacesConfig::with_allowed_for_test([true, true, false, true, true]);
    assert!(world.cfg.allowed_races.allows(0));
    assert!(!world.cfg.allowed_races.allows(2));
}

/// `Custom/PvpRewardItem.ini` — the killer is paid on a PvP kill (the victim
/// was flagged) and, separately, on a PK. Both rewards share one guard: nothing
/// is paid inside a PvP zone or an instance.
#[test]
fn a_pvp_kill_pays_the_killer() {
    use crate::model::components::{PvpState, ZoneFlags};
    use crate::model::inventory::Inventory;
    const ADENA: i32 = 57;
    const KILLER: i32 = 3001;
    const VICTIM: i32 = 3002;

    let (mut world, ..) = test_world();
    world.id_pool = 0x8000_0000..0x8000_0100;
    let mut t = crate::data::item_data::ItemTemplate::default();
    t.item_id = ADENA;
    t.name = "Adena".into();
    t.is_stackable = true;
    world.data.item_data.insert_for_test(t);
    world.cfg.pvp_reward.reward_pvp = true;
    world.cfg.pvp_reward.pvp_item_id = ADENA;
    world.cfg.pvp_reward.pvp_item_amount = 300_000;
    world.cfg.pvp_reward.reward_pk = false;
    let _a = ingame_player(&mut world, 1, KILLER, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, VICTIM, 50, 0, 0);
    let paid = |w: &World| {
        w.objects
            .get_component::<Inventory>(&KILLER)
            .map_or(0, |i| i.count_of(ADENA))
    };

    // Unflagged victim → this is a PK, and the PK arm ships off.
    crate::game_loop::pvp::pay_kill_reward(&mut world, KILLER, VICTIM);
    assert_eq!(paid(&world), 0, "no PK reward configured");

    // Flag the victim: now it is a PvP kill and the reward lands.
    world.objects.add_components(
        &VICTIM,
        PvpState {
            flag: 1,
            ..Default::default()
        },
    );
    crate::game_loop::pvp::pay_kill_reward(&mut world, KILLER, VICTIM);
    assert_eq!(paid(&world), 300_000, "the PvP reward");

    // …but not inside a PvP zone. This guard is only reachable because the
    // reward is a *sibling* of the reputation update rather than part of it —
    // that block returns early in a PvP zone, which would have made
    // `DisableRewardsInPvpZones` dead config.
    world.cfg.pvp_reward.disable_in_pvp_zones = true;
    world
        .objects
        .get_component_mut::<ZoneFlags>(&VICTIM)
        .unwrap()
        .mask |= crate::data::zone_data::ZoneKind::Pvp.bit();
    crate::game_loop::pvp::pay_kill_reward(&mut world, KILLER, VICTIM);
    assert_eq!(paid(&world), 300_000, "no reward inside a PvP zone");

    // With the guard off, the same kill pays again — proving the zone check is
    // what stopped it, not the zone itself.
    world.cfg.pvp_reward.disable_in_pvp_zones = false;
    crate::game_loop::pvp::pay_kill_reward(&mut world, KILLER, VICTIM);
    assert_eq!(
        paid(&world),
        600_000,
        "the guard, not the zone, was the block"
    );
}

/// `Custom/PvpTitleColor.ini` — crossing a rung renames and recolours the
/// player. Below the first rung nothing is touched, so an ordinary player keeps
/// the title they chose.
#[test]
fn the_pvp_ladder_retitles_the_killer() {
    let (mut world, ..) = test_world();
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.cfg.pvp_title_color = crate::config::PvpTitleColorConfig {
        enabled: true,
        ranks: vec![
            crate::config::custom_pvp::PvpRank {
                kills: 2,
                color: 0x9C_9C_9C,
                title: "Sergeant".into(),
            },
            crate::config::custom_pvp::PvpRank {
                kills: 5,
                color: 0x69_69_69,
                title: "Lieutenant".into(),
            },
        ],
    };
    let set_title = |w: &mut World, t: &str| {
        w.objects.get_component_mut::<Player>(&3001).unwrap().title = t.to_string();
    };
    let title_of = |w: &World| {
        w.objects
            .get_component::<Player>(&3001)
            .unwrap()
            .title
            .clone()
    };

    set_title(&mut world, "Mine");
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .pvp_kills = 1;
    crate::game_loop::pvp::update_pvp_title_and_color(&mut world, 3001, false);
    assert_eq!(title_of(&world), "Mine", "below the first rung, untouched");

    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .pvp_kills = 2;
    crate::game_loop::pvp::update_pvp_title_and_color(&mut world, 3001, false);
    assert_eq!(title_of(&world), "® Sergeant ®");
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .title_color,
        0x9C_9C_9C
    );

    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .pvp_kills = 99;
    crate::game_loop::pvp::update_pvp_title_and_color(&mut world, 3001, false);
    assert_eq!(
        title_of(&world),
        "® Lieutenant ®",
        "the top rung is open-ended"
    );
}

/// `Custom/RandomSpawns.ini` — a monster's spawn point is jittered; a raid, a
/// listed id and a quest monster keep their exact coordinates.
#[test]
fn random_spawns_jitter_only_ordinary_monsters() {
    let (mut world, ..) = test_world();
    world.cfg.random_spawns.enabled = true;
    world.cfg.random_spawns.max_range = 100;
    world.cfg.random_spawns.never_random.insert(20003);
    for (id, ty, title) in [
        (20001, "Monster", ""),
        (20002, "Monster", "Quest Monster"),
        (20003, "Monster", ""),
        (25001, "RaidBoss", ""),
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = ty.into();
        t.title = title.into();
        t.base_hp_max = 100.0;
        world.data.npc_data.insert_for_test(t);
    }

    // Exercised directly: only the *datapack* spawn path jitters. A script or
    // admin spawn keeps its coordinates, because Java's `AbstractScript.addSpawn`
    // explicitly restores them for a scripted monster.
    let spawn_at = |world: &mut World, id: i32| -> (i32, i32) {
        crate::model::npc::randomize_spawn_point(world, id, 1000, 1000, 0, 0)
    };

    // An ordinary monster moves (over many spawns, at least one lands off the
    // exact point — a single roll can legitimately come up (0, 0)).
    let moved = (0..40).any(|_| spawn_at(&mut world, 20001) != (1000, 1000));
    assert!(moved, "an ordinary monster is jittered");

    // The exclusions never move, however many times they spawn.
    for id in [20002, 20003, 25001] {
        assert!(
            (0..40).all(|_| spawn_at(&mut world, id) == (1000, 1000)),
            "npc {id} keeps its exact spawn point"
        );
    }

    // And the master flag turns the whole thing off.
    world.cfg.random_spawns.enabled = false;
    assert!(
        (0..40).all(|_| spawn_at(&mut world, 20001) == (1000, 1000)),
        "disabled → nothing moves"
    );
}
