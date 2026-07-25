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
        },
        Castle {
            id: OTHER_CASTLE,
            name: "Dion".into(),
            side: CastleSide::Neutral,
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
