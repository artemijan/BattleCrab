//! Registration rules: who may register on which side, the caps, and the
//! same-day exclusivity check.

use super::effective_siege_millis;
use super::owner_clan_id_opt;
use crate::db::DbCommand;
use crate::model::siege::SiegeClanType;
use crate::world::World;
/// `dist/game/config/Siege.ini` (Interlude): a clan must be at least level 3,
/// and each side holds up to 500 clans.
const SIEGE_CLAN_MIN_LEVEL: i32 = 3;
const ATTACKER_MAX_CLANS: usize = 500;
const DEFENDER_MAX_CLANS: usize = 500;
/// Java closes registration 24 h before the siege (`getSiegeDate - 86400000`).
const REGISTRATION_CLOSE_BEFORE_MS: i64 = 86_400_000;

/// The result of a clan trying to register for a siege — each variant is one
/// branch of Java's `checkIfCanRegister` (plus the two side-specific pre-checks
/// in `registerAttacker`/`registerDefender`), so the caller can pick the right
/// SystemMessage without re-deriving why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterOutcome {
    /// The clan may register (and, from [`register`], now has).
    Approved,
    /// Attacker-only: the clan is allied with the castle owner.
    AllianceWithOwner,
    /// Defender-only: the castle is held by an NPC, so there is nothing to defend.
    DefendingNpcCastle,
    /// The registration deadline (24 h before the siege) has passed.
    RegistrationOver,
    /// The siege is under way — no registering mid-fight.
    SiegeInProgress,
    /// Below the minimum clan level.
    ClanTooLow,
    /// The castle owner is registered on the defence automatically.
    OwnerAutoRegistered,
    /// The clan already owns a castle, so it can't join another siege.
    OwnsAnotherCastle,
    /// Already registered for *this* siege.
    AlreadyRegistered,
    /// Already registered for another siege on the same weekday.
    AlreadyRegisteredSameDay,
    /// The attacker side is full.
    AttackerSideFull,
    /// The defender side is full.
    DefenderSideFull,
}

/// Has this castle's registration window closed? True once we are within 24 h of
/// the next scheduled siege. A castle with no (or a disabled) schedule never
/// auto-closes — it is GM-driven, so registration stays open until the siege
/// runs (the `in_progress` check covers that case separately).
pub(crate) fn is_registration_over(world: &World, castle_id: i32, now_millis: i64) -> bool {
    // No schedule and no chosen date → GM-driven, never auto-closes.
    let next = effective_siege_millis(world, castle_id, now_millis);
    if next == 0 {
        return false;
    }
    next - now_millis <= REGISTRATION_CLOSE_BEFORE_MS
}

/// Java `checkIfAlreadyRegisteredForSameDay`: is the clan registered (in any
/// role) for a *different* castle whose siege falls on the same weekday? A clan
/// can only take part in one siege per day.
fn registered_same_day(world: &World, this_castle: i32, clan_id: i32) -> bool {
    let Some(this_weekday) = world
        .data
        .siege_schedule
        .get(&this_castle)
        .map(|e| e.weekday)
    else {
        return false;
    };
    world.sieges.iter().any(|(&cid, siege)| {
        cid != this_castle
            && world
                .data
                .siege_schedule
                .get(&cid)
                .is_some_and(|e| e.weekday == this_weekday)
            && siege.clans.iter().any(|c| {
                c.clan_id == clan_id
                    && matches!(
                        c.kind,
                        SiegeClanType::Attacker
                            | SiegeClanType::Defender
                            | SiegeClanType::DefenderPending
                    )
            })
    })
}

/// Java `checkIfCanRegister` plus the side-specific pre-checks of
/// `registerAttacker`/`registerDefender`, in Java's order. Pure — decides only,
/// changes nothing.
pub(crate) fn check_can_register(
    world: &World,
    castle_id: i32,
    clan_id: i32,
    attacker: bool,
    now_millis: i64,
) -> RegisterOutcome {
    let owner = owner_clan_id_opt(world, castle_id);

    // Side-specific pre-checks (Java runs these before checkIfCanRegister).
    if attacker {
        // Allied with the owner? A castle owner's allies can't besiege it.
        if let Some(owner_id) = owner {
            let owner_ally = world.clans.get(&owner_id).map(|c| c.ally_id).unwrap_or(0);
            let my_ally = world.clans.get(&clan_id).map(|c| c.ally_id).unwrap_or(0);
            if owner_ally != 0 && my_ally == owner_ally {
                return RegisterOutcome::AllianceWithOwner;
            }
        }
    } else if owner.is_none() {
        // Nothing to defend on an NPC-held castle.
        return RegisterOutcome::DefendingNpcCastle;
    }

    // The common ladder.
    if is_registration_over(world, castle_id, now_millis) {
        return RegisterOutcome::RegistrationOver;
    }
    if world.sieges.get(&castle_id).is_some_and(|s| s.in_progress) {
        return RegisterOutcome::SiegeInProgress;
    }
    if world
        .clans
        .get(&clan_id)
        .is_none_or(|c| c.level < SIEGE_CLAN_MIN_LEVEL)
    {
        return RegisterOutcome::ClanTooLow;
    }
    if owner == Some(clan_id) {
        return RegisterOutcome::OwnerAutoRegistered;
    }
    if world.clans.get(&clan_id).is_some_and(|c| c.castle_id > 0) {
        return RegisterOutcome::OwnsAnotherCastle;
    }
    if world
        .sieges
        .get(&castle_id)
        .is_some_and(|s| s.is_registered(clan_id))
    {
        return RegisterOutcome::AlreadyRegistered;
    }
    if registered_same_day(world, castle_id, clan_id) {
        return RegisterOutcome::AlreadyRegisteredSameDay;
    }

    let (attackers, defenders) = side_counts(world, castle_id);
    if attacker {
        if attackers >= ATTACKER_MAX_CLANS {
            return RegisterOutcome::AttackerSideFull;
        }
    } else if defenders >= DEFENDER_MAX_CLANS {
        return RegisterOutcome::DefenderSideFull;
    }

    RegisterOutcome::Approved
}

/// `(attacker count, defender+pending count)` for the side-limit checks.
fn side_counts(world: &World, castle_id: i32) -> (usize, usize) {
    let Some(siege) = world.sieges.get(&castle_id) else {
        return (0, 0);
    };
    let attackers = siege
        .clans
        .iter()
        .filter(|c| c.kind == SiegeClanType::Attacker)
        .count();
    let defenders = siege
        .clans
        .iter()
        .filter(|c| {
            matches!(
                c.kind,
                SiegeClanType::Defender | SiegeClanType::DefenderPending
            )
        })
        .count();
    (attackers, defenders)
}

/// Register a clan for a siege (Java `registerAttacker`/`registerDefender`). On
/// [`RegisterOutcome::Approved`] the clan is added — attackers as `Attacker`,
/// defenders as `DefenderPending` (awaiting the owner's approval) — and the row
/// is persisted. Any other outcome changes nothing.
pub(crate) fn register(
    world: &mut World,
    castle_id: i32,
    clan_id: i32,
    attacker: bool,
    now_millis: i64,
) -> RegisterOutcome {
    let outcome = check_can_register(world, castle_id, clan_id, attacker, now_millis);
    if outcome != RegisterOutcome::Approved {
        return outcome;
    }
    let kind = if attacker {
        SiegeClanType::Attacker
    } else {
        SiegeClanType::DefenderPending
    };
    if let Some(siege) = world.sieges.get_mut(&castle_id) {
        siege.add_clan(clan_id, kind);
    }
    let _ = world.db.send(DbCommand::SaveSiegeClan {
        castle_id,
        clan_id,
        kind: kind.as_db(),
    });
    RegisterOutcome::Approved
}

/// Java `approveSiegeDefenderClan`: the owner promotes a pending defender to a
/// full defender. Returns whether a pending row was found and promoted.
pub(crate) fn approve_defender(world: &mut World, castle_id: i32, clan_id: i32) -> bool {
    let promoted = world.sieges.get_mut(&castle_id).is_some_and(|siege| {
        siege
            .clans
            .iter_mut()
            .find(|c| c.clan_id == clan_id && c.kind == SiegeClanType::DefenderPending)
            .map(|c| c.kind = SiegeClanType::Defender)
            .is_some()
    });
    if promoted {
        let _ = world.db.send(DbCommand::SaveSiegeClan {
            castle_id,
            clan_id,
            kind: SiegeClanType::Defender.as_db(),
        });
    }
    promoted
}

/// Java `removeSiegeClan`: a clan cancels its registration. Returns whether it
/// was registered.
pub(crate) fn remove_registration(world: &mut World, castle_id: i32, clan_id: i32) -> bool {
    if clan_id <= 0 {
        return false;
    }
    let removed = world
        .sieges
        .get_mut(&castle_id)
        .is_some_and(|s| s.remove_clan(clan_id));
    if removed {
        let _ = world
            .db
            .send(DbCommand::RemoveSiegeClan { castle_id, clan_id });
    }
    removed
}
