//! Player-facing siege registration (G24) — the `checkIfCanRegister` ladder and
//! the register / approve / remove operations.

use super::*;

use crate::data::siege_data::SiegeScheduleEntry;
use crate::game_loop::siege::{
    RegisterOutcome, approve_defender, check_can_register, is_registration_over, register,
    remove_registration,
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
            show_npc_crest: false,
            id: CASTLE,
            name: "Gludio".into(),
            side: CastleSide::Neutral,
            ticket_buy_count: 0,
            first_mid_victory: false,
            time_registration_over: true,
            siege_time_registration_end: 0,
            siege_date: 0,
            treasury: 0,
        },
        Castle {
            show_npc_crest: false,
            id: OTHER_CASTLE,
            name: "Dion".into(),
            side: CastleSide::Neutral,
            ticket_buy_count: 0,
            first_mid_victory: false,
            time_registration_over: true,
            siege_time_registration_end: 0,
            siege_date: 0,
            treasury: 0,
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
fn world_with_leader() -> (World, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
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

fn sent_opcode(rx: &mut tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>, opcode: u8) -> bool {
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
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
    opcode: u8,
) -> Option<Vec<u8>> {
    let mut found = None;
    while let Ok(p) = rx.try_recv() {
        if p.first() == Some(&opcode) {
            found = Some(p.to_vec());
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

// ---------------------------------------------------------------------------
// Owner set-siege-time (RequestSetCastleSiegeTime, 0xAF)
// ---------------------------------------------------------------------------

/// A `RequestSetCastleSiegeTime` body: castle id + the chosen time in seconds.
fn set_time_body(castle_id: i32, time_secs: i32) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&castle_id.to_le_bytes());
    b.extend_from_slice(&time_secs.to_le_bytes());
    b
}

/// The owner leader of CASTLE with the time-registration window open, ingame.
fn world_with_castle_owner() -> (World, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
    let (mut world, _db, _l) = siege_world();
    world.clans.insert(10, mk_clan(10, 5, CASTLE, 0)); // clan 10 owns CASTLE
    world.clans.get_mut(&10).unwrap().leader_id = LEADER;
    let rx = ingame_player(&mut world, 5, LEADER, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&LEADER)
        .unwrap()
        .clan_id = 10;
    // Open the window (default is closed).
    world
        .castles
        .iter_mut()
        .find(|c| c.id == CASTLE)
        .unwrap()
        .time_registration_over = false;
    (world, rx)
}

/// **The owner picks a valid siege hour** — it's stored, the window closes, and
/// the refreshed `SiegeInfo` (0xC9) goes back.
#[test]
fn the_owner_sets_the_siege_time() {
    use crate::game_loop::siege::next_siege_millis;
    let (mut world, mut rx) = world_with_castle_owner();
    let now = commons::util::now_millis();
    let chosen = next_siege_millis(now, 6, 16); // CASTLE = Sunday 16:00 slot

    crate::game_loop::siege::handle_request_set_castle_siege_time(
        &mut world,
        5,
        &set_time_body(CASTLE, (chosen / 1000) as i32),
    );

    let castle = world.castles.iter().find(|c| c.id == CASTLE).unwrap();
    assert_eq!(castle.siege_date, chosen, "the chosen siege time is stored");
    assert!(castle.time_registration_over, "the window is now closed");
    assert!(
        sent_opcode(&mut rx, 0xC9),
        "the SiegeInfo window was refreshed"
    );
}

/// **A time that isn't one of the `SIEGE_HOUR_LIST` slots is rejected** — nothing
/// is stored and the window stays open.
#[test]
fn an_invalid_siege_time_is_rejected() {
    use crate::game_loop::siege::next_siege_millis;
    let (mut world, _rx) = world_with_castle_owner();
    let now = commons::util::now_millis();
    let bad = next_siege_millis(now, 6, 16) + 123_456; // not on an allowed hour

    crate::game_loop::siege::handle_request_set_castle_siege_time(
        &mut world,
        5,
        &set_time_body(CASTLE, (bad / 1000) as i32),
    );

    let castle = world.castles.iter().find(|c| c.id == CASTLE).unwrap();
    assert_eq!(castle.siege_date, 0, "no time is stored");
    assert!(!castle.time_registration_over, "the window stays open");
}

/// **A player who isn't the owning clan's leader cannot set the time.**
#[test]
fn a_non_owner_cannot_set_the_siege_time() {
    use crate::game_loop::siege::next_siege_millis;
    let (mut world, _rx) = world_with_castle_owner();
    // A different clan owns CASTLE now.
    world.clans.get_mut(&10).unwrap().castle_id = 0;
    world.clans.insert(99, mk_clan(99, 5, CASTLE, 0));
    let now = commons::util::now_millis();
    let chosen = next_siege_millis(now, 6, 16);

    crate::game_loop::siege::handle_request_set_castle_siege_time(
        &mut world,
        5,
        &set_time_body(CASTLE, (chosen / 1000) as i32),
    );

    assert_eq!(
        world
            .castles
            .iter()
            .find(|c| c.id == CASTLE)
            .unwrap()
            .siege_date,
        0,
        "a non-owner set no time"
    );
}

/// **`SiegeInfo` offers the hour list when the window is open** — the packet
/// carries the extra hour entries instead of the single fixed date.
#[test]
fn siege_info_offers_the_hour_list_when_open() {
    use crate::network::server_packets::siege_info;
    let with_hours = siege_info(1, true, 0, "", "", 0, "", 500, 0, &[1000, 2000]);
    let fixed = siege_info(1, true, 0, "", "", 0, "", 500, 0, &[]);
    // Fixed = [date, 0] (2 i32); hours = [0, count, h1, h2] (4 i32): +8 bytes.
    assert_eq!(
        with_hours.len(),
        fixed.len() + 8,
        "two hour options add 2 ints"
    );
}

// ---------------------------------------------------------------------------
// The death window's restart buttons (Java `Die`'s constructor)
// ---------------------------------------------------------------------------

/// **The client only sends a `RequestRestartPoint` for a button it was told
/// exists.** Every flag in the `Die` packet was a hard-coded 0, which made
/// `clanhall_restart_location` and `siege_restart_location` — both fully
/// implemented — unreachable in play: a defender could never pick "to castle",
/// an attacker never "to siege HQ".
#[test]
fn the_death_window_offers_the_siege_restart_buttons() {
    use crate::game_loop::death::die_options;
    use crate::model::siege::{Siege, SiegeClanType};

    const DEF: i32 = 8801;
    const ATK: i32 = 8802;
    // Inside castle 1's siege zone.
    const POS: (i32, i32, i32) = (-17964, 110730, -1000);

    let (mut world, _tx, _db, _l) = test_world();
    world.data.zone_data = crate::data::zone_data::ZoneData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    let (x, y, z) = POS;
    let _d = ingame_player(&mut world, 1, DEF, x, y, z);
    let _a = ingame_player(&mut world, 2, ATK, x, y, z);
    for oid in [DEF, ATK] {
        world
            .objects
            .get_component_mut::<crate::model::components::Position>(&oid)
            .unwrap()
            .z = z;
    }
    world
        .objects
        .get_component_mut::<Player>(&DEF)
        .unwrap()
        .clan_id = 500;
    world
        .objects
        .get_component_mut::<Player>(&ATK)
        .unwrap()
        .clan_id = 700;

    // No siege yet: neither button.
    assert!(!die_options(&world, DEF).to_castle);
    assert!(!die_options(&world, ATK).to_outpost);

    let mut siege = Siege::new(1);
    siege.in_progress = true;
    siege.add_clan(500, SiegeClanType::Defender);
    siege.add_clan(700, SiegeClanType::Attacker);
    world.sieges.insert(1, siege);

    // The defender is offered the castle even though their clan owns none.
    let d = die_options(&world, DEF);
    assert!(d.to_castle, "a registered defender restarts at the castle");
    assert!(!d.to_outpost);

    // The attacker gets the HQ button **only while a flag stands** — Java reads
    // `!siegeClan.getFlag().isEmpty()`, so a razed base camp removes it rather
    // than offering a respawn that would fail.
    assert!(
        !die_options(&world, ATK).to_outpost,
        "no base camp, no button"
    );
    world.sieges.get_mut(&1).unwrap().flags.push((700, 90_001));
    let a = die_options(&world, ATK);
    assert!(a.to_outpost, "with a flag planted, the HQ button appears");
    assert!(!a.to_castle, "and an attacker is not offered the castle");
}

/// The non-siege flags: `to_village` unless a revive is already proposed (the
/// player answers the dialog instead), `to_clan_hall` when the clan owns a
/// hall, `to_castle` when it owns a castle anywhere on the map.
#[test]
fn the_death_window_offers_the_ordinary_restart_buttons() {
    use crate::game_loop::death::die_options;

    const OID: i32 = 8803;
    let (mut world, _tx, _db, _l) = test_world();
    let _p = ingame_player(&mut world, 1, OID, 0, 0, 0);

    let base = die_options(&world, OID);
    assert!(base.to_village, "the village button is the default");
    assert!(!base.to_clan_hall && !base.to_castle);

    // A pending resurrection proposal hides "to village".
    world
        .objects
        .get_component_mut::<Player>(&OID)
        .unwrap()
        .revive_request = Some(crate::model::ReviveRequest {
        reviver: 1,
        restore_percent: 50.0,
        hp_percent: 70,
        mp_percent: 70,
        cp_percent: 0,
        is_pet: false,
    });
    assert!(
        !die_options(&world, OID).to_village,
        "canRevive() && !isPendingRevive()"
    );
    world
        .objects
        .get_component_mut::<Player>(&OID)
        .unwrap()
        .revive_request = None;

    // Owning a castle lights the castle button anywhere on the map.
    world
        .objects
        .get_component_mut::<Player>(&OID)
        .unwrap()
        .clan_id = 500;
    world.clans.insert(500, mk_clan(500, 5, 3, 0));
    assert!(die_options(&world, OID).to_castle);

    // …and owning a clan hall lights that one.
    assert!(!die_options(&world, OID).to_clan_hall);
    let mut halls = crate::data::clan_hall_data::load_clan_halls(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    let first = *halls.keys().next().expect("the dist has clan halls");
    halls.get_mut(&first).unwrap().owner_id = 500;
    world.clan_halls = halls;
    assert!(
        die_options(&world, OID).to_clan_hall,
        "the clan owns a hall, so the hideout button appears"
    );
}

/// **A spoiled corpse advertises itself as sweepable.** Java's
/// `_isSweepable = isAttackable() && isSweepActive()`; the port wrote a hard 0,
/// so the client never knew the loot was there.
#[test]
fn a_spoiled_corpse_is_marked_sweepable_in_its_die_packet() {
    const KILLER: i32 = 8804;
    const MOB: i32 = 0x4000_0777;

    // The `sweepable` dword sits after the opcode, object id and the three
    // village/hall/castle/outpost flags.
    let sweepable_of = |pkts: &[Vec<u8>]| -> Option<i32> {
        pkts.iter()
            .find(|p| p[0] == server_packets::opcodes::DIE)
            .map(|p| i32::from_le_bytes(p[21..25].try_into().unwrap()))
    };

    let die_flag = |spoiled: bool| {
        let (mut world, _tx, _db, _l) = test_world();
        let mut rx = ingame_player(&mut world, 1, KILLER, 0, 0, 0);
        add_test_npc(&mut world, MOB, 20001, "Monster", 5, 0, 0, 0);
        if spoiled {
            world
                .objects
                .get_component_mut::<crate::model::npc::Npc>(&MOB)
                .unwrap()
                .spoiler_object_id = KILLER;
        }
        drain(&mut rx);
        crate::game_loop::death::npc_do_die(&mut world, MOB, KILLER);
        sweepable_of(&drain(&mut rx)).expect("a Die packet was broadcast")
    };

    assert_eq!(die_flag(false), 0, "an unspoiled corpse is not sweepable");
    assert_eq!(die_flag(true), 1, "a spoiled one is");
}

/// `BuildCampSkillCondition`'s last gate: a headquarters may only be planted
/// inside one of the battlefield's marked HQ patches (`castle_hq.xml`, 19 of
/// them across the castles). The zone kind was never parsed, so a base camp
/// could previously go up anywhere on the field.
#[test]
fn a_headquarters_needs_an_hq_zone() {
    use crate::data::zone_data::{Zone, ZoneKind};

    const LEADER: i32 = 8810;
    const CASTLE: i32 = 1;
    const CLAN: i32 = 700;

    fn zone(name: &str, kind: ZoneKind, castle_id: i32, x1: i32, x2: i32) -> Zone {
        Zone {
            id: 0,
            name: name.into(),
            kind,
            territory: crate::data::spawn_data::Territory {
                form: crate::data::spawn_data::ZoneForm::Cuboid {
                    x1,
                    x2,
                    y1: -500,
                    y2: 500,
                },
                min_z: -1000,
                max_z: 1000,
            },
            castle_id,
            clan_hall_id: 0,
            effect: None,
            damage: None,
            swamp: None,
            condition: None,
            mother_tree: None,
        }
    }

    let (mut world, _tx, _db, _l) = test_world();
    world.id_pool = 0x6000_0000..0x6000_1000;
    // The battlefield spans x −500..500; only its right half is HQ ground.
    world
        .data
        .zone_data
        .insert(zone("battlefield", ZoneKind::Siege, CASTLE, -500, 500));
    world
        .data
        .zone_data
        .insert(zone("hq_patch", ZoneKind::Hq, CASTLE, 100, 500));
    // A live siege the clan is registered to attack.
    let mut siege = Siege::new(CASTLE);
    siege.in_progress = true;
    siege.add_clan(CLAN, SiegeClanType::Attacker);
    world.sieges.insert(CASTLE, siege);
    // The HQ flag NPC template.
    let mut t = crate::data::npc_data::default_template(35062);
    t.type_name = "Npc".into();
    t.base_hp_max = 100.0;
    world.data.npc_data.insert_for_test(t);

    let mut rx = ingame_player(&mut world, 1, LEADER, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&LEADER)
        .unwrap()
        .clan_id = CLAN;
    let mut clan = mk_clan(CLAN, 5, 0, 0);
    clan.leader_id = LEADER;
    world.clans.insert(CLAN, clan);
    drain(&mut rx);

    // Outside the HQ patch (x = 0): refused, with the message that names why.
    assert!(
        !crate::game_loop::siege::place_siege_flag(&mut world, LEADER, false),
        "no camp outside an HQ zone"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_CAN_T_BUILD_HEADQUARTERS_HERE),
        "and the client is told why"
    );
    assert_eq!(world.sieges[&CASTLE].flag_count(CLAN), 0);

    // Step onto the patch (x = 200) and it goes up.
    world
        .objects
        .get_component_mut::<crate::model::components::Position>(&LEADER)
        .unwrap()
        .x = 200;
    assert!(crate::game_loop::siege::place_siege_flag(
        &mut world, LEADER, false
    ));
    assert_eq!(world.sieges[&CASTLE].flag_count(CLAN), 1, "camp planted");
}

// ---------------------------------------------------------------------------
// Refusal feedback — the player must be told *why* (G24)
// ---------------------------------------------------------------------------

/// **Every refusal now says something.** Five of Java's registration refusals
/// went out as silence: the window simply did not change, so a player could not
/// tell "the deadline passed" from "you are allied with the owner".
///
/// Two of the five are not plain SystemMessage ids, which is why they lagged —
/// the deadline message carries a **castle-name parameter**, and the NPC-castle
/// refusal is a `sendMessage` free-text line in Java with no id at all.
#[test]
fn each_registration_refusal_sends_its_own_message() {
    use crate::network::server_packets::sm_ids;

    // Defending an NPC-held castle → Java's free-text line, delivered as
    // S1_TEXT because there is no message id for it.
    let (mut world, mut rx) = world_with_leader();
    drain(&mut rx);
    crate::game_loop::siege::handle_request_join_siege(&mut world, 5, &join_body(CASTLE, 0, 1));
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&sm_ids::S1_TEXT),
        "the NPC-castle refusal is a text line, not a message id"
    );

    // An ally of the castle owner, attacking → SM 690.
    let (mut world, mut rx) = world_with_leader();
    world.clans.insert(99, mk_clan(99, 5, CASTLE, 7)); // owner, ally 7
    world.clans.get_mut(&10).unwrap().ally_id = 7; // same alliance
    drain(&mut rx);
    crate::game_loop::siege::handle_request_join_siege(&mut world, 5, &join_body(CASTLE, 1, 1));
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &sm_ids::YOU_CANNOT_REGISTER_AS_AN_ATTACKER_BECAUSE_YOU_ARE_IN_AN_ALLIANCE_WITH_THE_CASTLE_OWNING_CLAN
        ),
        "the ally is told why"
    );

    // A clan inside its dissolution grace period is refused before the ladder
    // is even consulted.
    let (mut world, mut rx) = world_with_leader();
    world.clans.get_mut(&10).unwrap().dissolving_expiry_time =
        commons::util::now_millis() + 86_400_000;
    drain(&mut rx);
    crate::game_loop::siege::handle_request_join_siege(&mut world, 5, &join_body(CASTLE, 1, 1));
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &sm_ids::YOUR_CLAN_MAY_NOT_REGISTER_TO_PARTICIPATE_IN_A_SIEGE_WHILE_UNDER_A_GRACE_PERIOD_OF_THE_CLAN_S_DISSOLUTION
        ),
        "a dissolving clan is told why"
    );
    assert!(
        attackers(&world, CASTLE).is_empty(),
        "and is not registered"
    );
}
