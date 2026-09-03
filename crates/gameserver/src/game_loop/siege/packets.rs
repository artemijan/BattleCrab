//! The siege packet handlers (`RequestJoinSiege` family) and the
//! attacker/defender list builders.

use super::RegisterOutcome;
use super::approve_defender;
use super::broadcast_sm;
use super::can_pick_siege_time;
use super::effective_siege_millis;
use super::is_registration_over;
use super::next_siege_millis;
use super::owner_clan_id_opt;
use super::register;
use super::remove_registration;
use crate::db::DbCommand;
use crate::game_loop::clans;
use crate::game_loop::clans::clan_of_or_zero;
use crate::game_loop::helpers::send_sm_bare_to_client as send_sm_to;
use crate::game_loop::helpers::send_sm_to_client;
use crate::game_loop::helpers::send_to_client;
use crate::model::Player;
use crate::model::siege::SiegeClanType;
use crate::network::server_packets;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::world::World;
/// The SystemMessage for a refusal, or `None` when nothing is said.
///
/// Two outcomes are absent on purpose. `Approved` says nothing (Java's success
/// path sends the updated window, not a message), and `DefendingNpcCastle` is
/// not a SystemMessage at all: Java uses `player.sendMessage(...)` with a
/// hand-built string naming the castle, which [`register_refusal_text`]
/// reproduces.
fn outcome_sm(outcome: RegisterOutcome) -> Option<i16> {
    use RegisterOutcome::*;
    match outcome {
        ClanTooLow => Some(sm_ids::ONLY_CLANS_OF_LEVEL_3_OR_ABOVE_MAY_REGISTER_FOR_A_CASTLE_SIEGE),
        OwnerAutoRegistered => {
            Some(sm_ids::CASTLE_OWNING_CLANS_ARE_AUTOMATICALLY_REGISTERED_ON_THE_DEFENDING_SIDE)
        }
        AlreadyRegistered => Some(sm_ids::YOU_HAVE_ALREADY_REQUESTED_A_CASTLE_SIEGE),
        OwnsAnotherCastle => {
            Some(sm_ids::A_CLAN_THAT_OWNS_A_CASTLE_CANNOT_PARTICIPATE_IN_ANOTHER_SIEGE)
        }
        AttackerSideFull => {
            Some(sm_ids::NO_MORE_REGISTRATIONS_MAY_BE_ACCEPTED_FOR_THE_ATTACKER_SIDE)
        }
        DefenderSideFull => {
            Some(sm_ids::NO_MORE_REGISTRATIONS_MAY_BE_ACCEPTED_FOR_THE_DEFENDER_SIDE)
        }
        SiegeInProgress => Some(sm_ids::THIS_IS_NOT_THE_TIME_FOR_SIEGE_REGISTRATION),
        AllianceWithOwner => Some(
            sm_ids::YOU_CANNOT_REGISTER_AS_AN_ATTACKER_BECAUSE_YOU_ARE_IN_AN_ALLIANCE_WITH_THE_CASTLE_OWNING_CLAN,
        ),
        AlreadyRegisteredSameDay => Some(
            sm_ids::YOUR_APPLICATION_HAS_BEEN_DENIED_BECAUSE_YOU_HAVE_ALREADY_SUBMITTED_A_REQUEST_FOR_ANOTHER_CASTLE_SIEGE,
        ),
        // `RegistrationOver` carries a castle-name parameter, so it goes out
        // through `send_register_outcome` rather than this id-only path.
        RegistrationOver | DefendingNpcCastle | Approved => None,
    }
}

/// Deliver a registration outcome, including the two Java does not express as
/// a bare SystemMessage id.
fn send_register_outcome(world: &World, client_id: u32, castle_id: i32, outcome: RegisterOutcome) {
    use RegisterOutcome::*;
    match outcome {
        // `sm.addCastleId(residenceId)` — the client resolves the castle's name
        // from the id, so this needs the parameterised writer.
        RegistrationOver => {
            send_to_client(
                world,
                client_id,
                server_packets::system_message_with(
                    sm_ids::THE_DEADLINE_TO_REGISTER_FOR_THE_SIEGE_OF_S1_HAS_PASSED,
                    &[SmParam::CastleName(castle_id)],
                ),
            );
        }
        // Java: `player.sendMessage("You cannot register as a defender because
        // " + castle.getName() + " is owned by NPC.")` — a plain line, not a
        // SystemMessage, so there is no id to look up.
        DefendingNpcCastle => {
            let name = world
                .castles
                .iter()
                .find(|c| c.id == castle_id)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            send_sm_to_client(
                world,
                client_id,
                sm_ids::S1_TEXT,
                &[SmParam::Text(format!(
                    "You cannot register as a defender because {name} is owned by NPC."
                ))],
            );
        }
        other => {
            if let Some(sm) = outcome_sm(other) {
                send_sm_to(world, client_id, sm);
            }
        }
    }
}

/// `RequestJoinSiege` (0xAD): a `CS_MANAGE_SIEGE` clan leader registers as
/// attacker/defender (`isJoining==1`) or cancels (`isJoining==0`) for a castle
/// siege, then gets the refreshed `SiegeInfo` window (Java `listRegisterClan`).
pub(crate) fn handle_request_join_siege(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };

    let mut r = commons::network::PacketReader::new(body);
    let (Some(castle_id), Some(is_attacker), Some(is_joining)) =
        (r.read_i32(), r.read_i32(), r.read_i32())
    else {
        return;
    };

    let Some((clan_id, privs)) = clans::clan_and_privs(world, player) else {
        return;
    };
    if clan_id == 0 {
        return;
    }

    // `hasClanPrivilege(CS_MANAGE_SIEGE)`.
    let authorized = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.has_privilege(player, privs, crate::model::clan::CS_MANAGE_SIEGE));
    if !authorized {
        send_sm_to(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    }
    // Unknown castle → ignore (Java's `castle != null`).
    if !world.castles.iter().any(|c| c.id == castle_id) {
        return;
    }

    let now = commons::util::now_millis();
    if is_joining == 1 {
        // A clan under a dissolution grace period may not register.
        let grace = world
            .clans
            .get(&clan_id)
            .map(|c| c.dissolving_expiry_time)
            .unwrap_or(0);
        if now < grace {
            // Java refuses before even reaching `checkIfCanRegister`.
            send_sm_to(
                world,
                client_id,
                sm_ids::YOUR_CLAN_MAY_NOT_REGISTER_TO_PARTICIPATE_IN_A_SIEGE_WHILE_UNDER_A_GRACE_PERIOD_OF_THE_CLAN_S_DISSOLUTION,
            );
            return;
        }
        let outcome = register(world, castle_id, clan_id, is_attacker == 1, now);
        send_register_outcome(world, client_id, castle_id, outcome);
    } else {
        remove_registration(world, castle_id, clan_id);
    }

    send_siege_info(world, client_id, castle_id, clan_id, player, now);
}

/// Java `Siege.listRegisterClan(player)` — the same window, opened by talking
/// to a castle's Siege Manager NPC as a non-owner (`ai/others/
/// CastleSiegeManager`). Resolves the caller's clan itself.
pub(crate) fn list_register_clan(world: &World, client_id: u32, player: i32, castle_id: i32) {
    let clan_id = clan_of_or_zero(world, player);
    send_siege_info(
        world,
        client_id,
        castle_id,
        clan_id,
        player,
        commons::util::now_millis(),
    );
}

/// Java `Siege.listRegisterClan` → `new SiegeInfo(castle, player)`.
pub(crate) fn send_siege_info(
    world: &World,
    client_id: u32,
    castle_id: i32,
    my_clan_id: i32,
    player: i32,
    now_millis: i64,
) {
    let owner_id = owner_clan_id_opt(world, castle_id).unwrap_or(0);
    let owner = world.clans.get(&owner_id);
    let owner_name = owner.map(|c| c.name.as_str()).unwrap_or("");
    let owner_leader_id = owner.map(|c| c.leader_id).unwrap_or(0);
    let owner_leader = owner
        .and_then(|c| c.members.iter().find(|m| m.char_id == owner_leader_id))
        .map(|m| m.name.as_str())
        .unwrap_or("");
    let owner_ally_id = owner.map(|c| c.ally_id).unwrap_or(0);
    let owner_ally_name = owner.map(|c| c.ally_name.as_str()).unwrap_or("");

    // `(ownerId == player.getClanId()) && player.isClanLeader()`.
    let can_set_time = owner_id != 0 && owner_id == my_clan_id && owner_leader_id == player;

    // Java: when the owner-leader may still set the time (`!isTimeRegistrationOver`),
    // send the selectable `SIEGE_HOUR_LIST` slots instead of the fixed date. The
    // day is the castle's scheduled weekday; only the hour varies.
    let weekday = world
        .data
        .siege_schedule
        .get(&castle_id)
        .filter(|e| e.enabled)
        .map(|e| e.weekday);
    let hour_options: Vec<i32> = if can_set_time && can_pick_siege_time(world, castle_id) {
        weekday
            .map(|wd| {
                world
                    .cfg
                    .feature
                    .siege_hour_list
                    .iter()
                    .map(|&h| (next_siege_millis(now_millis, wd, h) / 1000) as i32)
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let siege_date_secs = (effective_siege_millis(world, castle_id, now_millis) / 1000) as i32;

    let pkt = server_packets::siege_info(
        castle_id,
        can_set_time,
        owner_id,
        owner_name,
        owner_leader,
        owner_ally_id,
        owner_ally_name,
        (now_millis / 1000) as i32,
        siege_date_secs,
        &hour_options,
    );
    send_to_client(world, client_id, pkt);
}

/// A clan's role in a castle's siege, if registered.
fn siege_clan_kind(world: &World, castle_id: i32, clan_id: i32) -> Option<SiegeClanType> {
    world
        .sieges
        .get(&castle_id)?
        .clans
        .iter()
        .find(|c| c.clan_id == clan_id)
        .map(|c| c.kind)
}

/// `RequestConfirmSiegeWaitingList` (0xAE): the castle owner's clan leader
/// approves (`approved==1`) a pending defender or rejects/removes a
/// pending-or-confirmed defender, then gets the refreshed defender list.
pub(crate) fn handle_request_confirm_siege_waiting_list(
    world: &mut World,
    client_id: u32,
    body: &[u8],
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };

    let mut r = commons::network::PacketReader::new(body);
    let (Some(castle_id), Some(clan_id), Some(approved)) =
        (r.read_i32(), r.read_i32(), r.read_i32())
    else {
        return;
    };

    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let my_clan = p.clan_id;
    if my_clan == 0 {
        return;
    }
    if !world.castles.iter().any(|c| c.id == castle_id) {
        return;
    }
    // Only the owning clan's leader may manage the defender list.
    let owner = owner_clan_id_opt(world, castle_id);
    let is_leader = world
        .clans
        .get(&my_clan)
        .is_some_and(|c| c.leader_id == player);
    if owner != Some(my_clan) || !is_leader {
        return;
    }
    // The target clan must exist.
    if !world.clans.contains_key(&clan_id) {
        return;
    }

    let now = commons::util::now_millis();
    if !is_registration_over(world, castle_id, now) {
        let kind = siege_clan_kind(world, castle_id, clan_id);
        if approved == 1 {
            if kind == Some(SiegeClanType::DefenderPending) {
                approve_defender(world, castle_id, clan_id);
            } else {
                return; // Java returns without sending the list
            }
        } else if matches!(
            kind,
            Some(SiegeClanType::DefenderPending) | Some(SiegeClanType::Defender)
        ) {
            remove_registration(world, castle_id, clan_id);
        }
    }

    send_defender_list(world, client_id, castle_id, now);
}

/// Java `RequestSetCastleSiegeTime` (client 0xAF): the castle owner's clan leader
/// picks the siege hour from `SIEGE_HOUR_LIST`, closing the time-registration
/// window. Announces it to everyone and refreshes the viewer's `SiegeInfo`.
pub(crate) fn handle_request_set_castle_siege_time(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = commons::network::PacketReader::new(body);
    let (Some(castle_id), Some(time_secs)) = (r.read_i32(), r.read_i32()) else {
        return;
    };
    let chosen_millis = time_secs as i64 * 1000;

    // `getCastleById == null → return`.
    if !world.castles.iter().any(|c| c.id == castle_id) {
        return;
    }
    let owner_id = owner_clan_id_opt(world, castle_id).unwrap_or(0);
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    let is_leader = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.leader_id == player);
    // Gates: the owner's clan (or an unowned castle) + clan leader + the window
    // still open (Java's warnings on each failed branch, which we just drop).
    if (owner_id != 0 && owner_id != clan_id) || !is_leader {
        return;
    }
    if !can_pick_siege_time(world, castle_id) {
        return;
    }

    // `isSiegeTimeValid`: the chosen time must be one of the `SIEGE_HOUR_LIST`
    // slots on the castle's scheduled day.
    let now = commons::util::now_millis();
    let Some(weekday) = world
        .data
        .siege_schedule
        .get(&castle_id)
        .filter(|e| e.enabled)
        .map(|e| e.weekday)
    else {
        return;
    };
    if !world
        .cfg
        .feature
        .siege_hour_list
        .iter()
        .any(|&h| next_siege_millis(now, weekday, h) == chosen_millis)
    {
        return;
    }

    // Set the date, close the window, persist, and re-arm the auto-start.
    if let Some(c) = world.castle_mut(castle_id) {
        c.siege_date = chosen_millis;
        c.time_registration_over = true;
    }
    let _ = world.db.send(DbCommand::UpdateCastleSiegeTime {
        castle_id,
        siege_date: chosen_millis,
        time_registration_over: true,
        siege_time_registration_end: None,
    });

    // "S1 has announced the next castle siege time." to everyone, then refresh.
    broadcast_sm(
        world,
        sm_ids::S1_HAS_ANNOUNCED_THE_NEXT_CASTLE_SIEGE_TIME,
        castle_id,
    );
    send_siege_info(world, client_id, castle_id, clan_id, player, now);
}

/// Java `new SiegeDefenderList(castle)`: the owner clan first, then confirmed
/// defenders, then pending defenders.
fn send_defender_list(world: &World, client_id: u32, castle_id: i32, now_millis: i64) {
    let owner_id = owner_clan_id_opt(world, castle_id).unwrap_or(0);
    let mut entries: Vec<server_packets::DefenderEntry> = Vec::new();

    // Owner (type 1), if any.
    if owner_id != 0
        && let Some(e) = defender_entry(world, owner_id, 1)
    {
        entries.push(e);
    }
    // Confirmed defenders (type 3), then pending (type 2) — skipping the owner.
    if let Some(siege) = world.sieges.get(&castle_id) {
        for &(kind, type_value) in &[
            (SiegeClanType::Defender, 3),
            (SiegeClanType::DefenderPending, 2),
        ] {
            for c in siege.clans.iter().filter(|c| c.kind == kind) {
                if c.clan_id == owner_id {
                    continue;
                }
                if let Some(e) = defender_entry(world, c.clan_id, type_value) {
                    entries.push(e);
                }
            }
        }
    }

    let valid_registration = owner_id != 0 && is_registration_over(world, castle_id, now_millis);
    let pkt = server_packets::siege_defender_list(castle_id, valid_registration, &entries);
    send_to_client(world, client_id, pkt);
}

/// Java `RequestSiegeAttackerList` (client 0xAB): send the castle's registered
/// attacker clans to the viewer (Java gates on nothing — any in-game player).
pub(crate) fn handle_request_siege_attacker_list(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(castle_id) = commons::network::PacketReader::new(body).read_i32() else {
        return;
    };
    // Java `getCastleById(castleId) == null → return`.
    if !world.castles.iter().any(|c| c.id == castle_id) {
        return;
    }
    let mut entries: Vec<server_packets::AttackerEntry> = Vec::new();
    if let Some(siege) = world.sieges.get(&castle_id) {
        for c in siege
            .clans
            .iter()
            .filter(|c| c.kind == SiegeClanType::Attacker)
        {
            if let Some(e) = attacker_entry(world, c.clan_id) {
                entries.push(e);
            }
        }
    }
    let pkt = server_packets::siege_attacker_list(castle_id, &entries);
    send_to_client(world, client_id, pkt);
}

/// Java `RequestSiegeDefenderList` (client 0xAC): send the castle's owner +
/// defender roster to the viewer (reusing the owner-approval list builder).
pub(crate) fn handle_request_siege_defender_list(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(castle_id) = commons::network::PacketReader::new(body).read_i32() else {
        return;
    };
    if !world.castles.iter().any(|c| c.id == castle_id) {
        return;
    }
    send_defender_list(world, client_id, castle_id, commons::util::now_millis());
}

/// Build one attacker row from a clan (Java `SiegeAttackerList` — no type byte,
/// and the ally-leader name is always written empty).
fn attacker_entry(world: &World, clan_id: i32) -> Option<server_packets::AttackerEntry> {
    let clan = world.clans.get(&clan_id)?;
    Some(server_packets::AttackerEntry {
        clan_id,
        name: clan.name.clone(),
        leader_name: clan.leader_name().to_string(),
        crest_id: clan.crest_id,
        ally_id: clan.ally_id,
        ally_name: clan.ally_name.clone(),
        ally_crest_id: clan.ally_crest_id,
    })
}

/// Build one defender row from a clan.
fn defender_entry(
    world: &World,
    clan_id: i32,
    type_value: i32,
) -> Option<server_packets::DefenderEntry> {
    let clan = world.clans.get(&clan_id)?;
    // The ally leader clan shares the ally id (Java: the leader clan's own id).
    let ally_leader_name = if clan.ally_id != 0 {
        world
            .clans
            .get(&clan.ally_id)
            .map(|a| a.leader_name().to_string())
            .unwrap_or_default()
    } else {
        String::new()
    };
    Some(server_packets::DefenderEntry {
        clan_id,
        name: clan.name.clone(),
        leader_name: clan.leader_name().to_string(),
        crest_id: clan.crest_id,
        type_value,
        ally_id: clan.ally_id,
        ally_name: clan.ally_name.clone(),
        ally_leader_name,
        ally_crest_id: clan.ally_crest_id,
    })
}
