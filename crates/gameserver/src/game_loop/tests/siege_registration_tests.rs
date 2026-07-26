//! Player-facing siege registration (G24) — the `checkIfCanRegister` ladder and
//! the register / approve / remove operations.

use super::*;

use crate::data::siege_data::SiegeScheduleEntry;
use crate::game_loop::siege::{
    approve_defender, check_can_register, is_registration_over, register, remove_registration,
    RegisterOutcome,
};
use crate::model::castle::{Castle, CastleSide};
use crate::model::clan::Clan;
use crate::model::siege::{Siege, SiegeClanType};

const DAY_MS: i64 = 86_400_000;
const HOUR_MS: i64 = 3_600_000;
/// Epoch is Thursday(3); a Sunday(6)@16:00 slot is ~3.7 days out, so with
/// `now = 0` registration is comfortably open.
const OPEN_NOW: i64 = 0;

const CASTLE: i32 = 1;
const OTHER_CASTLE: i32 = 2;

fn mk_clan(id: i32, level: i32, castle_id: i32, ally_id: i32) -> Clan {
    Clan {
        id,
        name: format!("Clan{id}"),
        leader_id: id * 10,
        level,
        reputation_score: 0,
        castle_id,
        members: Vec::new(),
        skills: Default::default(),
        warehouse: Default::default(),
        char_penalty_expiry_time: 0,
        dissolving_expiry_time: 0,
        rank_privs: Default::default(),
        new_leader_id: 0,
        sub_pledges: Default::default(),
        ally_id,
        ally_name: String::new(),
        ally_penalty_expiry_time: 0,
        ally_penalty_type: 0,
        crest_id: 0,
        crest_large_id: 0,
        ally_crest_id: 0,
        blood_alliance_count: 0,
    }
}

/// A world with two Sunday@16:00 castles and their (empty) sieges.
fn siege_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    world.castles = vec![
        Castle {
            id: CASTLE,
            name: "Gludio".into(),
            side: CastleSide::Neutral,
            ticket_buy_count: 0,
        },
        Castle {
            id: OTHER_CASTLE,
            name: "Dion".into(),
            side: CastleSide::Neutral,
            ticket_buy_count: 0,
        },
    ];
    for id in [CASTLE, OTHER_CASTLE] {
        world.sieges.insert(id, Siege::new(id));
        world.data.siege_schedule.insert(
            id,
            SiegeScheduleEntry {
                weekday: 6,
                hour: 16,
                enabled: true,
            },
        );
    }
    (world, db, l)
}

fn attackers(world: &World, castle_id: i32) -> Vec<i32> {
    world.sieges[&castle_id]
        .clans
        .iter()
        .filter(|c| c.kind == SiegeClanType::Attacker)
        .map(|c| c.clan_id)
        .collect()
}

/// **A qualified clan registers as an attacker.** It lands on the siege as an
/// `Attacker` — the happy path the whole ladder guards.
#[test]
fn a_qualified_clan_registers_as_attacker() {
    let (mut world, _db, _l) = siege_world();
    world.clans.insert(10, mk_clan(10, 5, 0, 0));

    let outcome = register(&mut world, CASTLE, 10, true, OPEN_NOW);

    assert_eq!(outcome, RegisterOutcome::Approved);
    assert_eq!(
        attackers(&world, CASTLE),
        vec![10],
        "added to the attackers"
    );
}

/// A clan below the minimum level (3) is turned away.
#[test]
fn a_low_level_clan_is_refused() {
    let (mut world, _db, _l) = siege_world();
    world.clans.insert(10, mk_clan(10, 2, 0, 0));

    assert_eq!(
        register(&mut world, CASTLE, 10, true, OPEN_NOW),
        RegisterOutcome::ClanTooLow
    );
    assert!(attackers(&world, CASTLE).is_empty(), "nothing registered");
}

/// The castle owner is already on the defence — it can't register.
#[test]
fn the_castle_owner_is_auto_registered() {
    let (mut world, _db, _l) = siege_world();
    world.clans.insert(10, mk_clan(10, 5, CASTLE, 0)); // owns this castle

    assert_eq!(
        check_can_register(&world, CASTLE, 10, true, OPEN_NOW),
        RegisterOutcome::OwnerAutoRegistered
    );
}

/// A clan that owns *another* castle can't join a second siege.
#[test]
fn a_castle_owner_cannot_besiege_another() {
    let (mut world, _db, _l) = siege_world();
    world.clans.insert(10, mk_clan(10, 5, OTHER_CASTLE, 0));

    assert_eq!(
        check_can_register(&world, CASTLE, 10, true, OPEN_NOW),
        RegisterOutcome::OwnsAnotherCastle
    );
}

/// Registering twice for the same siege is refused.
#[test]
fn registering_twice_is_refused() {
    let (mut world, _db, _l) = siege_world();
    world.clans.insert(10, mk_clan(10, 5, 0, 0));
    register(&mut world, CASTLE, 10, true, OPEN_NOW);

    assert_eq!(
        register(&mut world, CASTLE, 10, true, OPEN_NOW),
        RegisterOutcome::AlreadyRegistered
    );
    assert_eq!(attackers(&world, CASTLE).len(), 1, "still just the one");
}

/// **A clan may only take part in one siege per day.** Registered for the other
/// Sunday castle, it is refused here.
#[test]
fn same_day_registration_is_refused() {
    let (mut world, _db, _l) = siege_world();
    world.clans.insert(10, mk_clan(10, 5, 0, 0));
    register(&mut world, OTHER_CASTLE, 10, true, OPEN_NOW);

    assert_eq!(
        register(&mut world, CASTLE, 10, true, OPEN_NOW),
        RegisterOutcome::AlreadyRegisteredSameDay
    );
}

/// An ally of the castle owner cannot register as an attacker.
#[test]
fn an_ally_of_the_owner_cannot_attack() {
    let (mut world, _db, _l) = siege_world();
    world.clans.insert(99, mk_clan(99, 5, CASTLE, 7)); // owner, ally 7
    world.clans.insert(10, mk_clan(10, 5, 0, 7)); // same ally

    assert_eq!(
        check_can_register(&world, CASTLE, 10, true, OPEN_NOW),
        RegisterOutcome::AllianceWithOwner
    );
}

/// You can't defend a castle held by an NPC — there is no owner to side with.
#[test]
fn defending_an_npc_castle_is_refused() {
    let (mut world, _db, _l) = siege_world();
    world.clans.insert(10, mk_clan(10, 5, 0, 0)); // no clan owns CASTLE

    assert_eq!(
        check_can_register(&world, CASTLE, 10, false, OPEN_NOW),
        RegisterOutcome::DefendingNpcCastle
    );
}

/// **Registration closes 24 h before the siege.** Open a day and a half out,
/// shut half a day out.
#[test]
fn registration_closes_24h_before_the_siege() {
    let (world, _db, _l) = siege_world();
    let slot = 3 * DAY_MS + 16 * HOUR_MS; // the first Sunday@16:00 after epoch

    assert!(
        !is_registration_over(&world, CASTLE, slot - 25 * HOUR_MS),
        "open 25 h before"
    );
    assert!(
        is_registration_over(&world, CASTLE, slot - 12 * HOUR_MS),
        "closed 12 h before"
    );
}

/// The owner promotes a pending defender to a full one.
#[test]
fn the_owner_approves_a_pending_defender() {
    let (mut world, _db, _l) = siege_world();
    world.clans.insert(99, mk_clan(99, 5, CASTLE, 0)); // owner (so defence is open)
    world.clans.insert(10, mk_clan(10, 5, 0, 0));

    assert_eq!(
        register(&mut world, CASTLE, 10, false, OPEN_NOW),
        RegisterOutcome::Approved
    );
    // A fresh defender is pending, not yet a full defender.
    let kind_of = |w: &World| {
        w.sieges[&CASTLE]
            .clans
            .iter()
            .find(|c| c.clan_id == 10)
            .map(|c| c.kind)
    };
    assert_eq!(kind_of(&world), Some(SiegeClanType::DefenderPending));

    assert!(approve_defender(&mut world, CASTLE, 10), "promoted");
    assert_eq!(kind_of(&world), Some(SiegeClanType::Defender));
}

/// Cancelling a registration removes the clan.
#[test]
fn cancelling_removes_the_registration() {
    let (mut world, _db, _l) = siege_world();
    world.clans.insert(10, mk_clan(10, 5, 0, 0));
    register(&mut world, CASTLE, 10, true, OPEN_NOW);
    assert_eq!(attackers(&world, CASTLE), vec![10]);

    assert!(remove_registration(&mut world, CASTLE, 10));
    assert!(attackers(&world, CASTLE).is_empty(), "gone");
}

// ---------------------------------------------------------------------------
// Reachability — the RequestJoinSiege (0xAD) packet handler
// ---------------------------------------------------------------------------

const LEADER: i32 = 8100;

/// Build a `RequestJoinSiege` body: castleId, isAttacker, isJoining.
fn join_body(castle_id: i32, attacker: i32, joining: i32) -> Vec<u8> {
    let mut w = commons::network::PacketWriter::new();
    w.write_i32(castle_id);
    w.write_i32(attacker);
    w.write_i32(joining);
    w.into_bytes()
}

/// Disable the schedules so `is_registration_over` (which reads the real
/// wall-clock in the packet handlers) is deterministically **open** — the 24 h
/// window is exercised separately by `registration_closes_24h_before_the_siege`.
fn keep_registration_open(world: &mut World) {
    for e in world.data.siege_schedule.values_mut() {
        e.enabled = false;
    }
}

/// A clan leader with `world`, a `SiegeInfo`-capable clan and an ingame session.
fn world_with_leader() -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, _db, _l) = siege_world();
    keep_registration_open(&mut world);
    world.clans.insert(10, mk_clan(10, 5, 0, 0));
    world.clans.get_mut(&10).unwrap().leader_id = LEADER; // the player is the leader
    let rx = ingame_player(&mut world, 5, LEADER, 0, 0, 0);
    let p = world.objects.get_component_mut::<Player>(&LEADER).unwrap();
    p.clan_id = 10;
    p.clan_privs = 0; // leader, so privileges are implicit
                      // `_db`/`_l` drop here; the world's db sends then no-op (best-effort, as in
                      // the production path).
    (world, rx)
}

fn sent_opcode(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>, opcode: u8) -> bool {
    let mut found = false;
    while let Ok(p) = rx.try_recv() {
        if p.first() == Some(&opcode) {
            found = true;
        }
    }
    found
}

/// **A clan leader registers through the packet** — the whole point of the
/// slice: `RequestJoinSiege` lands the clan on the siege and the refreshed
/// `SiegeInfo` window (0xC9) goes back.
#[test]
fn a_leader_registers_through_the_packet() {
    let (mut world, mut rx) = world_with_leader();

    crate::game_loop::siege::handle_request_join_siege(&mut world, 5, &join_body(CASTLE, 1, 1));

    assert_eq!(
        attackers(&world, CASTLE),
        vec![10],
        "registered as attacker"
    );
    assert!(
        sent_opcode(&mut rx, 0xC9),
        "the SiegeInfo window was sent back"
    );
}

/// Cancelling through the packet (`isJoining == 0`) removes the clan.
#[test]
fn cancelling_through_the_packet_removes_it() {
    let (mut world, _rx) = world_with_leader();
    crate::game_loop::siege::handle_request_join_siege(&mut world, 5, &join_body(CASTLE, 1, 1));
    assert_eq!(attackers(&world, CASTLE), vec![10]);

    crate::game_loop::siege::handle_request_join_siege(&mut world, 5, &join_body(CASTLE, 1, 0));

    assert!(attackers(&world, CASTLE).is_empty(), "cancelled");
}

/// A member without `CS_MANAGE_SIEGE` (not the leader, no privilege bit) is
/// refused and nothing is registered.
#[test]
fn a_member_without_the_privilege_is_refused() {
    let (mut world, _rx) = world_with_leader();
    // Demote the actor: no longer the leader, and no siege-manage bit.
    world.clans.get_mut(&10).unwrap().leader_id = 9999;

    crate::game_loop::siege::handle_request_join_siege(&mut world, 5, &join_body(CASTLE, 1, 1));

    assert!(
        attackers(&world, CASTLE).is_empty(),
        "an unauthorized member registered nothing"
    );
}

// ---------------------------------------------------------------------------
// Reachability — RequestConfirmSiegeWaitingList (0xAE), owner approval
// ---------------------------------------------------------------------------

/// Build a `RequestConfirmSiegeWaitingList` body: castleId, clanId, approved.
fn confirm_body(castle_id: i32, clan_id: i32, approved: i32) -> Vec<u8> {
    let mut w = commons::network::PacketWriter::new();
    w.write_i32(castle_id);
    w.write_i32(clan_id);
    w.write_i32(approved);
    w.into_bytes()
}

fn kind_of(world: &World, castle_id: i32, clan_id: i32) -> Option<SiegeClanType> {
    world.sieges[&castle_id]
        .clans
        .iter()
        .find(|c| c.clan_id == clan_id)
        .map(|c| c.kind)
}

/// **The owner's leader approves a pending defender through the packet.** The
/// pending clan becomes a full defender and the defender list (0xCB) is sent.
#[test]
fn the_owner_approves_a_pending_defender_through_the_packet() {
    let (mut world, _db, _l) = siege_world();
    keep_registration_open(&mut world);
    // The owner clan, led by the acting player.
    world.clans.insert(20, mk_clan(20, 5, CASTLE, 0));
    world.clans.get_mut(&20).unwrap().leader_id = LEADER;
    // A pending defender.
    world.clans.insert(10, mk_clan(10, 5, 0, 0));
    world
        .sieges
        .get_mut(&CASTLE)
        .unwrap()
        .add_clan(10, SiegeClanType::DefenderPending);
    let mut rx = ingame_player(&mut world, 5, LEADER, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&LEADER)
        .unwrap()
        .clan_id = 20;

    crate::game_loop::siege::handle_request_confirm_siege_waiting_list(
        &mut world,
        5,
        &confirm_body(CASTLE, 10, 1),
    );

    assert_eq!(
        kind_of(&world, CASTLE, 10),
        Some(SiegeClanType::Defender),
        "promoted to a full defender"
    );
    assert!(sent_opcode(&mut rx, 0xCB), "the defender list was sent");
}

/// Rejecting (approved==0) removes the pending defender.
#[test]
fn the_owner_rejects_a_pending_defender_through_the_packet() {
    let (mut world, _db, _l) = siege_world();
    keep_registration_open(&mut world);
    world.clans.insert(20, mk_clan(20, 5, CASTLE, 0));
    world.clans.get_mut(&20).unwrap().leader_id = LEADER;
    world.clans.insert(10, mk_clan(10, 5, 0, 0));
    world
        .sieges
        .get_mut(&CASTLE)
        .unwrap()
        .add_clan(10, SiegeClanType::DefenderPending);
    let _rx = ingame_player(&mut world, 5, LEADER, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&LEADER)
        .unwrap()
        .clan_id = 20;

    crate::game_loop::siege::handle_request_confirm_siege_waiting_list(
        &mut world,
        5,
        &confirm_body(CASTLE, 10, 0),
    );

    assert_eq!(
        kind_of(&world, CASTLE, 10),
        None,
        "the pending clan was removed"
    );
}

/// A non-owner leader can't manage the defender list.
#[test]
fn a_non_owner_cannot_approve_defenders() {
    let (mut world, _db, _l) = siege_world();
    keep_registration_open(&mut world);
    // The acting clan does NOT own CASTLE.
    world.clans.insert(20, mk_clan(20, 5, 0, 0));
    world.clans.get_mut(&20).unwrap().leader_id = LEADER;
    world.clans.insert(10, mk_clan(10, 5, 0, 0));
    world
        .sieges
        .get_mut(&CASTLE)
        .unwrap()
        .add_clan(10, SiegeClanType::DefenderPending);
    let _rx = ingame_player(&mut world, 5, LEADER, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&LEADER)
        .unwrap()
        .clan_id = 20;

    crate::game_loop::siege::handle_request_confirm_siege_waiting_list(
        &mut world,
        5,
        &confirm_body(CASTLE, 10, 1),
    );

    assert_eq!(
        kind_of(&world, CASTLE, 10),
        Some(SiegeClanType::DefenderPending),
        "still pending — a non-owner changed nothing"
    );
}

// ---------------------------------------------------------------------------
// Reachability — RequestSiegeAttackerList (0xAB) / RequestSiegeDefenderList (0xAC)
// ---------------------------------------------------------------------------

/// A single-i32 castle-id body (both list requests read just the castle id).
fn list_body(castle_id: i32) -> Vec<u8> {
    castle_id.to_le_bytes().to_vec()
}

/// Drain `rx` and return the last packet with the given opcode, if any.
fn take_packet(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    opcode: u8,
) -> Option<Vec<u8>> {
    let mut found = None;
    while let Ok(p) = rx.try_recv() {
        if p.first() == Some(&opcode) {
            found = Some(p);
        }
    }
    found
}

/// **The attacker list (0xCA) answers a `RequestSiegeAttackerList` (0xAB)** with
/// the castle's registered attacker clans.
#[test]
fn the_attacker_list_is_sent_on_request() {
    let (mut world, mut rx) = world_with_leader();
    register(&mut world, CASTLE, 10, true, OPEN_NOW); // clan 10 attacks CASTLE
    let _ = take_packet(&mut rx, 0); // flush login/register chatter

    crate::game_loop::siege::handle_request_siege_attacker_list(&mut world, 5, &list_body(CASTLE));

    let pkt = take_packet(&mut rx, 0xCA).expect("attacker list sent");
    // Layout: opcode(1) + castleId,0,1,0 (16) → the clan count at offset 17.
    let count = i32::from_le_bytes(pkt[17..21].try_into().unwrap());
    assert_eq!(count, 1, "the one registered attacker is listed");
}

/// **The defender list (0xCB) answers a `RequestSiegeDefenderList` (0xAC)** with
/// the owner and the registered defenders.
#[test]
fn the_defender_list_is_sent_on_request() {
    let (mut world, mut rx) = world_with_leader();
    world.clans.get_mut(&10).unwrap().castle_id = CASTLE; // clan 10 owns CASTLE
    world.clans.insert(20, mk_clan(20, 5, 0, 0));
    register(&mut world, CASTLE, 20, false, OPEN_NOW); // clan 20 pending defender
    let _ = take_packet(&mut rx, 0);

    crate::game_loop::siege::handle_request_siege_defender_list(&mut world, 5, &list_body(CASTLE));

    let pkt = take_packet(&mut rx, 0xCB).expect("defender list sent");
    // Layout: opcode(1) + castleId,0,valid,0 (16) → the clan count at offset 17.
    let count = i32::from_le_bytes(pkt[17..21].try_into().unwrap());
    assert_eq!(count, 2, "the owner plus the one pending defender");
}

/// A request for a castle that does not exist sends nothing (Java's
/// `getCastleById == null → return`).
#[test]
fn a_list_request_for_an_unknown_castle_is_ignored() {
    let (mut world, mut rx) = world_with_leader();
    crate::game_loop::siege::handle_request_siege_attacker_list(&mut world, 5, &list_body(99));
    assert!(
        take_packet(&mut rx, 0xCA).is_none(),
        "no attacker list for an unknown castle"
    );
}
