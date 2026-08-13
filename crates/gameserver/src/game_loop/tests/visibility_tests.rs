use super::*;
use crate::game_loop::helpers;

/// Leaving the world (logout here; restart/disconnect share the path)
/// broadcasts `DeleteObject` to everyone watching and drops their target
/// (Java `deleteMe` → `World.removeVisibleObject`).
#[test]
fn leave_world_sends_delete_object_to_watchers() {
    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let _leaver_rx = ingame_player(&mut world, 1, 6301, 0, 0, 0);
    let mut near_rx = ingame_player(&mut world, 2, 6302, 500, 500, 0);
    let mut far_rx = ingame_player(&mut world, 3, 6303, 10_000, 10_000, 0);
    world
        .objects
        .get_component_mut::<TargetRef>(&6302)
        .unwrap()
        .0 = Some(6301);

    handle_logout(&mut world, 1);

    let to_near = drain(&mut near_rx);
    assert_eq!(
        to_near[0][0],
        server_packets::opcodes::TARGET_UNSELECTED,
        "ring released before the delete"
    );
    assert_eq!(delete_object_id(&to_near[1]), 6301);
    assert_eq!(
        world.objects.get_component::<TargetRef>(&6302).unwrap().0,
        None,
        "dangling target dropped"
    );
    assert!(far_rx.try_recv().is_err());
}

/// A clan leader coming into view sends the observer a `RelationChanged` with
/// the `RELATION_LEADER` (0x80) crown bit — even with no siege — because
/// `CharInfo` carries no is-leader field (Java `Player.sendInfo`).
#[test]
fn clan_leader_crown_relation_sent_on_entering_view() {
    use crate::model::Player;
    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let _leader_rx = ingame_player(&mut world, 1, 6401, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&6401).unwrap();
        p.clan_id = 7;
        p.clan_leader = true;
    }
    let mut obs_rx = ingame_player(&mut world, 2, 6402, 200, 0, 0);
    // The observer's knownlist add exchanges CharInfo + RelationChanged.
    visibility::on_enter_world(&world, 2, 6402);
    let saw_crown = drain(&mut obs_rx).iter().any(|p| {
        p[0] == server_packets::opcodes::RELATION_CHANGED
            && i32::from_le_bytes(p[2..6].try_into().unwrap()) == 6401
            && i32::from_le_bytes(p[6..10].try_into().unwrap()) & 0x80 != 0
    });
    assert!(
        saw_crown,
        "leader entering view sends RelationChanged with the 0x80 crown bit"
    );
}

/// `World.player_regions` is what scopes every broadcast now, so the scoping
/// itself is asserted here rather than only implied by the packets other tests
/// happen to observe: a broadcast reaches the 3×3 block around the sender and
/// stops there.
#[test]
fn a_broadcast_reaches_the_surrounding_block_and_nothing_beyond_it() {
    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let _sender_rx = ingame_player(&mut world, 1, 6401, 0, 0, 0);
    // Same cell.
    let mut same_rx = ingame_player(&mut world, 2, 6402, 100, 100, 0);
    // Adjacent cell (region is 2048 units): still inside the 3×3 block.
    let mut adjacent_rx = ingame_player(&mut world, 3, 6403, 3000, 0, 0);
    // Two cells out: outside the block.
    let mut far_rx = ingame_player(&mut world, 4, 6404, 9000, 0, 0);

    drain(&mut same_rx);
    drain(&mut adjacent_rx);
    drain(&mut far_rx);

    let packet = server_packets::action_failed();
    helpers::broadcast_to_others(&world, 6401, &packet);

    assert_eq!(drain(&mut same_rx).len(), 1, "same cell receives");
    assert_eq!(drain(&mut adjacent_rx).len(), 1, "adjacent cell receives");
    assert!(
        drain(&mut far_rx).is_empty(),
        "two cells out is outside the surrounding block"
    );
}

/// Crossing a region boundary has to move the index with the player, or they
/// keep receiving the broadcasts of the cell they left and miss the one they
/// entered. `World::set_player_region` is the only thing that may do it.
#[test]
fn crossing_a_region_boundary_moves_who_hears_a_broadcast() {
    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let _sender_rx = ingame_player(&mut world, 1, 6411, 0, 0, 0);
    // Starts four cells away — out of range of the sender.
    let mut mover_rx = ingame_player(&mut world, 2, 6412, 9000, 0, 0);
    drain(&mut mover_rx);

    let packet = server_packets::action_failed();
    helpers::broadcast_to_others(&world, 6411, &packet);
    assert!(
        drain(&mut mover_rx).is_empty(),
        "out of range to begin with"
    );

    // Walk into the sender's own cell.
    world
        .objects
        .get_component_mut::<Position>(&6412)
        .unwrap()
        .x = 100;
    visibility::update_region(&mut world, 6412);
    drain(&mut mover_rx);

    helpers::broadcast_to_others(&world, 6411, &packet);
    assert_eq!(
        drain(&mut mover_rx).len(),
        1,
        "the index followed them into range"
    );
}

// ---------------------------------------------------------------------------
// `RelationChanged`'s viewer-dependent bits (Java `Player.getRelation(target)`)
// ---------------------------------------------------------------------------

/// Java's party-index `switch` written out, so the port's arithmetic is checked
/// against the table rather than against itself. Slot 0 is the leader flag
/// `0x10`; slots 1..=8 count **down** from `0x8`, which is the part that looks
/// like a bug until you read the switch.
#[test]
fn party_slot_bits_match_javas_switch_table() {
    use crate::game_loop::player_info::party_slot_bits;
    // (index, value) straight off `Player.getRelation`'s cases.
    let java = [
        (0, 0x10), // RELATION_PARTYLEADER
        (1, 0x8),  // PARTY4
        (2, 0x7),  // PARTY3 + PARTY2 + PARTY1
        (3, 0x6),  // PARTY3 + PARTY2
        (4, 0x5),  // PARTY3 + PARTY1
        (5, 0x4),  // PARTY3
        (6, 0x3),  // PARTY2 + PARTY1
        (7, 0x2),  // PARTY2
        (8, 0x1),  // PARTY1
    ];
    for (index, expected) in java {
        assert_eq!(
            party_slot_bits(index),
            expected,
            "party slot {index} encodes as {expected:#x}"
        );
    }
    // Java's switch has no case past 8, so a 10th member contributes nothing.
    assert_eq!(party_slot_bits(9), 0, "no case past 8 in Java's switch");
}

/// Java gates the party block on `party == target.getParty()` — the *same*
/// party, not merely being in one. The port hoisted the relation out of its
/// per-viewer loop, so every bystander was told a player was grouped, and a
/// party member's slot number never arrived at all.
#[test]
fn party_bits_reach_only_the_players_own_party() {
    use crate::game_loop::player_info::relation_to;
    use crate::model::components::PartyRef;
    use crate::model::party::{LootRule, Party};

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let _leader_rx = ingame_player(&mut world, 1, 6501, 0, 0, 0);
    let _member_rx = ingame_player(&mut world, 2, 6502, 100, 0, 0);
    let _loner_rx = ingame_player(&mut world, 3, 6503, 200, 0, 0);
    // A viewer in a *different* party — the case that separates Java's
    // `party == target.getParty()` from a mere "is in a party" test. Without
    // them, weakening the gate to `is_some()` leaves this test green.
    let _other_party_rx = ingame_player(&mut world, 4, 6504, 300, 0, 0);

    let mut party = Party::new(6501, LootRule::FindersKeepers, 0);
    party.members.push(6502);
    world.parties.insert(42, party);
    world.objects.add_components(&6501, PartyRef(42));
    world.objects.add_components(&6502, PartyRef(42));
    world
        .parties
        .insert(43, Party::new(6504, LootRule::FindersKeepers, 0));
    world.objects.add_components(&6504, PartyRef(43));

    const HAS_PARTY: i32 = 0x20;
    const PARTY_LEADER: i32 = 0x10;

    // Leader as seen by their own party member: in a party, and slot 0.
    let seen_by_member = relation_to(&world, 6501, 6502);
    assert_eq!(seen_by_member & HAS_PARTY, HAS_PARTY, "same party");
    assert_eq!(
        seen_by_member & PARTY_LEADER,
        PARTY_LEADER,
        "leader occupies slot 0"
    );

    // The second member, as seen by the leader: slot 1 → 0x8, and NOT 0x10.
    let member_seen = relation_to(&world, 6502, 6501);
    assert_eq!(member_seen & 0xF, 0x8, "slot 1 encodes as 0x8");
    assert_eq!(member_seen & PARTY_LEADER, 0, "a member is not the leader");

    // Neither a partyless bystander nor someone in another party learns
    // anything — the second is the one that matters.
    for outsider in [6503, 6504] {
        assert_eq!(
            relation_to(&world, 6501, outsider) & (HAS_PARTY | PARTY_LEADER | 0xF),
            0,
            "viewer {outsider} is not in this party and sees no party bits"
        );
    }
}

/// `RELATION_CLAN_MATE` (`0x100`) compares the two clans, while
/// `RELATION_CLAN_MEMBER` (`0x40`) only asks whether the subject has one — and
/// `RELATION_ALLY_MEMBER` (`0x10000`) is the trap: it sits inside the same
/// `clan != null` branch but reads only the **subject's** ally, so unlike its
/// neighbours it is identical for every viewer.
#[test]
fn clan_mate_is_viewer_relative_but_ally_member_is_not() {
    use crate::game_loop::player_info::relation_to;
    use crate::model::Player;

    const CLAN_MEMBER: i32 = 0x40;
    const CLAN_MATE: i32 = 0x100;
    const ALLY_MEMBER: i32 = 0x10000;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let _a_rx = ingame_player(&mut world, 1, 6511, 0, 0, 0);
    let _mate_rx = ingame_player(&mut world, 2, 6512, 100, 0, 0);
    let _other_rx = ingame_player(&mut world, 3, 6513, 200, 0, 0);
    for (oid, clan, ally) in [(6511, 7, 3), (6512, 7, 3), (6513, 9, 0)] {
        let p = world.objects.get_component_mut::<Player>(&oid).unwrap();
        p.clan_id = clan;
        p.ally_id = ally;
    }

    let by_mate = relation_to(&world, 6511, 6512);
    let by_other = relation_to(&world, 6511, 6513);

    assert_eq!(by_mate & CLAN_MEMBER, CLAN_MEMBER);
    assert_eq!(by_other & CLAN_MEMBER, CLAN_MEMBER, "not viewer-relative");

    assert_eq!(by_mate & CLAN_MATE, CLAN_MATE, "same clan");
    assert_eq!(by_other & CLAN_MATE, 0, "a different clan is no clan-mate");

    assert_eq!(
        by_mate & ALLY_MEMBER,
        by_other & ALLY_MEMBER,
        "ally membership reads the subject only, so it cannot vary by viewer"
    );
    assert_eq!(by_mate & ALLY_MEMBER, ALLY_MEMBER, "and it is set");

    // No clan at all → none of the three, even with an ally id set.
    world
        .objects
        .get_component_mut::<Player>(&6511)
        .unwrap()
        .clan_id = 0;
    assert_eq!(
        relation_to(&world, 6511, 6512) & (CLAN_MEMBER | CLAN_MATE | ALLY_MEMBER),
        0,
        "Java nests all three inside `clan != null`"
    );
}
