//! Castle siege lifecycle — Java `Siege.startSiege`/`endSiege`, the timed-event
//! skeleton: announce the start to everyone, set the in-progress flag, and
//! schedule the auto-end after the siege length; the auto-end announces the
//! finish and clears the flag.
//!
//! The battlefield landed with G24 and lives here too: control/flame towers and
//! siege guards (`spawn_siege_npcs`), the siege zone and its PvP
//! (`zones::refresh_siege_zone_for_all`), siege flags, and the winner's
//! ownership change (`capture`).
//!
//! What is still deferred is narrow and marked at its own site: castle crests,
//! `Castle.removeUpgrade()` (castle *functions* — the chamberlain's door/trap
//! tiers — are not modelled at all, so there is nothing to strip), the
//! members-inside-the-zone fame task (see `update_player_siege_state_flags`),
//! and two registration refusal messages.

mod battlefield;
mod capture;
mod doors;
mod mercenaries;
// Unlike the tick drivers in `game_loop/mod.rs`, this re-export is load-bearing:
// `game_loop::tests` is a **sibling** of `siege`, not a descendant, so it cannot
// name `siege::mercenaries` while that module is private. Deleting this line
// does not compile.
#[cfg(test)]
pub(crate) use mercenaries::clear_castle as clear_castle_mercenaries;
pub(crate) use mercenaries::handle_confirm as handle_mercenary_confirm;
pub(crate) use mercenaries::use_ticket as use_mercenary_ticket;
mod packets;
mod registration;
mod schedule;
pub(crate) mod treasury;

use crate::game_loop::combat::pvp;
use crate::game_loop::helpers::{is_dead, send_sm_to_player};
use crate::model::Player;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::scheduler::ScheduledTask;
use crate::scheduler::ms_to_ticks;
use crate::world::World;
use battlefield::spawn_siege_npcs;
pub(crate) use battlefield::{
    active_siege_castle_at, active_siege_guard_castle, attackable_siege_flag,
    attackable_siege_guard, attackable_siege_tower, is_siege_defender, killed_control_tower,
    killed_siege_flag, place_siege_flag,
};
#[cfg(test)]
pub(crate) use capture::BLOOD_ALLIANCE_REWARD;
#[cfg(test)]
pub(crate) use capture::spawn_towers_for_test;
pub(crate) use capture::{capture, set_show_npc_crest};
use capture::{
    increase_blood_alliance, record_castle_taken_for_nobles, reopen_time_registration,
    reset_castle_ticket_count, teleport_all_to_town,
};
pub(crate) use doors::{attackable_door, damage_door, oust_all_players, try_capture_artifact};
use doors::{despawn_siege_npcs, spawn_castle_doors, teleport_non_owners};
pub(crate) use packets::{
    handle_request_confirm_siege_waiting_list, handle_request_join_siege,
    handle_request_set_castle_siege_time, handle_request_siege_attacker_list,
    handle_request_siege_defender_list, list_register_clan, send_siege_info,
};
#[cfg(test)]
pub(crate) use registration::check_can_register;
pub(crate) use registration::{
    RegisterOutcome, approve_defender, is_registration_over, register, remove_registration,
};
#[cfg(test)]
pub(crate) use schedule::run_auto_task;
use schedule::{can_pick_siege_time, effective_siege_millis};
pub(crate) use schedule::{handle_scheduled_siege_start, next_siege_millis, schedule_all_at_boot};

/// `SiegeManager.getSiegeLength()` — `SiegeLength = 120` (minutes) in Siege.ini.
const SIEGE_LENGTH_MIN: i32 = 120;

/// `Siege.startSiege` (lifecycle slice). Called only with a registered attacker
/// (the admin path guards that).
/// `Player.setFame`: every fame write is clamped to
/// `[0, MaxPersonalFamePoints]`.
///
/// The ceiling is **0** on this dist, which does not read as "no limit" — it
/// disables fame, because each award is clamped straight back to zero. The
/// port used to add the castle-zone award unclamped, so players banked fame
/// that Java would never have let them keep.
pub(crate) fn set_fame_clamped(world: &mut World, player_oid: i32, f: impl Fn(i32) -> i32) {
    let ceiling = world.cfg.character.max_personal_fame_points;
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.fame = f(p.fame).clamp(0, ceiling.max(0));
    }
}

pub(crate) fn start_siege(world: &mut World, castle_id: i32) {
    // The castle owner at start — `endSiege` compares against it (Java
    // `_firstOwnerClanId = _castle.getOwnerId()`).
    let first_owner = owner_clan_id(world, castle_id);
    match world.sieges.get_mut(&castle_id) {
        Some(siege) if !siege.in_progress => {
            siege.in_progress = true;
            siege.first_owner_clan_id = first_owner;
        }
        _ => return, // unknown castle or already in progress
    }

    // Java `updatePlayerSiegeStateFlags(false)` + `updateZoneStatusForCharacters
    // Inside`: now that the siege is active, participants standing in the zone
    // gain the in-siege crown (UserInfo 0x80) + attackable icon. Runs before the
    // teleport below, matching Java's order.
    super::space::zones::refresh_siege_zone_for_all(world);

    // "The <castle> siege has started." + the siege sound, to everyone.
    broadcast_sm(world, sm_ids::THE_S1_SIEGE_HAS_STARTED, castle_id);
    world.broadcast_to_all_online(&server_packets::play_sound("systemmsg_eu.17"));

    // Auto-end after the siege length (Java `ScheduleEndSiegeTask`).
    let fire_at = world.tick + ms_to_ticks(SIEGE_LENGTH_MIN * 60 * 1000);
    world
        .scheduler
        .schedule(fire_at, ScheduledTask::SiegeEnd { castle_id });

    // `teleportPlayer(NotOwner, TOWN)` — Java clears the battlefield of
    // everyone but the owning clan at the bell, attackers included; they walk
    // back in and an attacker leader plants a headquarters
    // (`build_headquarters`) to respawn there instead of in town.
    teleport_non_owners(world, castle_id);

    // `_castle.spawnDoor()` — close the castle gates at full HP for the battle.
    spawn_castle_doors(world, castle_id, false);
    // `spawnControlTower()` / `spawnFlameTower()` + `spawnSiegeGuard()`.
    let towers = world
        .data
        .siege_towers
        .get(&castle_id)
        .cloned()
        .unwrap_or_default();
    spawn_siege_npcs(world, castle_id, &towers);
    let guards = world
        .siege_guards
        .get(&castle_id)
        .cloned()
        .unwrap_or_default();
    spawn_siege_npcs(world, castle_id, &guards);
    // …and the mercenaries the owner posted with tickets between sieges.
    mercenaries::spawn_hired(world, castle_id);

    // `updatePlayerSiegeStateFlags(false)` — every online member of a
    // registered clan learns which side they are on.
    update_player_siege_state_flags(world, castle_id, false);

    // The control-tower consequences are both live now: the guardian-tower
    // resurrection refusal (`death::siege_resurrect_refusal`) and the mass
    // gatekeeper's 8-minute evacuation delay once the towers are gone.
    // `Castle.getZone().setActive` is modelled by the in-progress flag the
    // siege-zone PvP check reads.
}

/// Port of `Siege.updatePlayerSiegeStateFlags(clear)`: stamp (or wipe) the
/// per-member siege side on every **online** member of every registered clan,
/// then re-push their appearance so nearby clients recolour them.
///
/// The side is a *clan* property projected onto its members, and it is
/// deliberately **not** "is standing in the siege zone": a registered attacker
/// carries state 1 wherever they are, which is what makes two attacker clans
/// unable to fight each other anywhere on the map for the duration.
///
/// The zone's *fame task* is separate — see [`arm_fame_task`], which
/// `zones::revalidate_zone` drives off siege-zone entry the way Java drives it
/// off `SiegeZone.onEnter`.
/// `SiegeZone.onEnter` → `player.startFameTask(fameFrequency * 1000,
/// fameAmount)`, and `onExit` → `stopFameTask()`.
///
/// Java holds a cancellable `ScheduledFuture` per player. This scheduler has no
/// cancel, so the task instead **re-arms itself** only while the player still
/// qualifies: leaving the zone, unregistering, or logging out for good simply
/// means the next firing does not schedule another. `siege_fame_armed` is what
/// keeps a second task from being armed while one is already running — without
/// it, every zone revalidation inside the zone would stack another earner.
///
/// Inert on this dist, where `CastleZoneFameAquirePoints = 0`: Java arms the
/// task on `giveFame() && frequency > 0` — the *amount* is not part of that
/// gate — so a 0 amount still runs the task and pays nothing. Ported as
/// written rather than short-circuited, because an operator raising the amount
/// should get the behaviour without a code change.
pub(crate) fn arm_fame_task(world: &mut World, player_oid: i32) {
    if world.cfg.character.castle_zone_fame_task_frequency <= 0
        || !world.siege_fame_armed.insert(player_oid)
    {
        return;
    }
    let delay = world.cfg.character.castle_zone_fame_task_frequency as u64 * 10;
    world
        .scheduler
        .schedule(world.tick + delay, ScheduledTask::SiegeFame { player_oid });
}

/// One firing of the fame task: pay, then decide whether there is a next one.
///
/// The three refusals are Java's `FameTask.run`, in Java's order. Note the
/// first two only skip *this* payment — Java's task keeps ticking for a corpse
/// in the zone and pays again once it stands up — while leaving the zone is
/// what actually ends it.
pub(crate) fn handle_siege_fame(world: &mut World, player_oid: i32) {
    if !crate::game_loop::combat::pvp::is_in_siege(world, player_oid) {
        // `stopFameTask()`: out of the zone, or no longer a participant.
        world.siege_fame_armed.remove(&player_oid);
        return;
    }
    let dead = is_dead(world, player_oid);
    let detached = crate::game_loop::helpers::client_for_player(world, player_oid).is_none();
    let paid = !(dead && !world.cfg.character.fame_for_dead_players)
        && !(detached && !world.cfg.offline_trade.fame);
    if paid {
        let amount = world.cfg.character.castle_zone_fame_acquire_points;
        set_fame_clamped(world, player_oid, |cur| cur + amount);
        send_sm_to_player(
            world,
            player_oid,
            sm_ids::YOU_HAVE_ACQUIRED_S1_FAME,
            &[SmParam::Int(amount)],
        );
        crate::game_loop::character::player_info::broadcast_user_info(world, player_oid);
    }
    // Still in the zone, so it keeps ticking either way.
    let delay = world.cfg.character.castle_zone_fame_task_frequency as u64 * 10;
    world
        .scheduler
        .schedule(world.tick + delay, ScheduledTask::SiegeFame { player_oid });
}

pub(crate) fn update_player_siege_state_flags(world: &mut World, castle_id: i32, clear: bool) {
    let Some(siege) = world.sieges.get(&castle_id) else {
        return;
    };
    // (clan_id, side) for every clan with a side — owner/defender = 2,
    // attacker = 1. Pending defenders have no side, as in Java.
    let sides: Vec<(i32, u8)> = siege
        .clans
        .iter()
        .map(|c| (c.clan_id, siege.side_of(c.clan_id)))
        .filter(|&(_, side)| side != 0)
        .collect();
    let mut touched = Vec::new();
    for (clan_id, side) in sides {
        for member in super::clans::online_members(world, clan_id) {
            if let Some(p) = world.objects.get_component_mut::<Player>(&member) {
                p.siege_state = if clear { 0 } else { side };
                p.siege_side = if clear { 0 } else { castle_id };
            }
            touched.push(member);
        }
    }
    // `member.updateUserInfo()` + the `RelationChanged` sweep: the port's
    // `broadcast_user_info` carries both (UserInfo to self, CharInfo to the
    // neighbours), and the relation refresh rides the same path.
    for member in touched {
        super::character::player_info::broadcast_user_info(world, member);
        pvp::broadcast_siege_relation(world, member);
    }
}

/// `Siege.endSiege` — announce the finish, declare the winner (or a draw), and
/// clear the battlefield.
/// `Castle.updateClansReputation` — the siege's reputation settlement, run
/// once as the siege closes.
///
/// Three outcomes, and the middle one is the easy one to get wrong: the captor
/// gains `TakeCastlePoints` **capped by what the former owner actually had**
/// (Java's `min(TAKE_CASTLE_POINTS, maxreward)`, where `maxreward` is the
/// former owner's score *before* it is docked), so taking a castle off a
/// bankrupt clan pays nothing. A castle with no former owner pays the full
/// amount uncapped, and a successful defence pays `CastleDefendedPoints`.
fn update_clans_reputation(world: &mut World, former_owner: i32, new_owner: Option<i32>) {
    let f = &world.cfg.feature;
    let (take, defended, loose) = (
        f.take_castle_points,
        f.castle_defended_points,
        f.loose_castle_points,
    );
    if former_owner == 0 {
        // `_formerOwner == null` — an unowned castle taken: the captor gets the
        // full amount with no cap.
        if let Some(new) = new_owner {
            crate::game_loop::clans::add_clan_reputation(world, new, take);
        }
        return;
    }
    match new_owner {
        Some(new) if new != former_owner => {
            let max_reward = world
                .clans
                .get(&former_owner)
                .map_or(0, |c| c.reputation_score)
                .max(0);
            crate::game_loop::clans::add_clan_reputation(world, former_owner, -loose);
            crate::game_loop::clans::add_clan_reputation(world, new, take.min(max_reward));
        }
        // Held it, or nobody took it: Java's `else` branch is "the owner is the
        // former owner", which covers the draw too.
        _ => {
            crate::game_loop::clans::add_clan_reputation(world, former_owner, defended);
        }
    }
}

pub(crate) fn end_siege(world: &mut World, castle_id: i32) {
    let first_owner = match world.sieges.get_mut(&castle_id) {
        Some(siege) if siege.in_progress => {
            siege.in_progress = false;
            siege.first_owner_clan_id
        }
        _ => return, // unknown castle or not in progress
    };

    // Java `updatePlayerSiegeStateFlags(true)`: the siege is over, so clear the
    // side off every registered clan's members (and the in-siege crown/icon
    // from everyone on the now-inactive field). Runs **before** the ownership
    // bookkeeping below, while the roster still names the clans that fought.
    update_player_siege_state_flags(world, castle_id, true);
    // `endSiege` then walks both rosters calling `checkItemRestriction()` on
    // every online member: the siege that let a `<cond SiegeZone>` item be
    // worn has just stopped being one.
    for member in registered_online_members(world, castle_id) {
        crate::game_loop::items::check_item_restriction(world, member);
    }
    super::space::zones::refresh_siege_zone_for_all(world);
    // `_castle.setFirstMidVictory(false)`.
    if let Some(c) = world.castle_mut(castle_id) {
        c.first_mid_victory = false;
    }

    broadcast_sm(world, sm_ids::THE_S1_SIEGE_HAS_FINISHED, castle_id);
    world.broadcast_to_all_online(&server_packets::play_sound("systemmsg_eu.18"));

    // The winner is whoever owns the castle at the end (an attacker only owns it
    // if they captured it mid-siege via `capture`).
    match world
        .clans
        .values()
        .find(|c| c.castle_id == castle_id)
        .map(|c| (c.id, c.name.clone()))
    {
        Some((owner_id, owner_name)) => {
            // "Clan <owner> is victorious over <castle>'s castle siege!"
            let pkt = server_packets::system_message_with(
                sm_ids::CLAN_S1_IS_VICTORIOUS_OVER_S2_S_CASTLE_SIEGE,
                &[SmParam::Text(owner_name), SmParam::CastleName(castle_id)],
            );
            world.broadcast_to_all_online(&pkt);
            // Java: owner unchanged (`clan.getId() == _firstOwnerClanId`) → the
            // defenders held → blood-alliance reward; owner changed → an attacker
            // captured it → the castle's mercenary ticket count is cleared.
            if owner_id == first_owner {
                increase_blood_alliance(world, owner_id);
            } else {
                reset_castle_ticket_count(world, castle_id);
                record_castle_taken_for_nobles(world, owner_id, castle_id);
            }
            update_clans_reputation(world, first_owner, Some(owner_id));
        }
        None => {
            broadcast_sm(
                world,
                sm_ids::THE_SIEGE_OF_S1_HAS_ENDED_IN_A_DRAW,
                castle_id,
            );
            update_clans_reputation(world, first_owner, None);
        }
    }

    // `teleportPlayer(NotOwner, TOWN)` — clear the battlefield at the end too.
    teleport_non_owners(world, castle_id);
    // `_castle.spawnDoor()` — restore the gates to full HP + closed.
    spawn_castle_doors(world, castle_id, false);
    // Despawn the siege guards (+ any towers) from the battlefield.
    despawn_siege_npcs(world, castle_id);
    // Java `saveCastleSiege()` — reopen the owner's hour-picking window.
    reopen_time_registration(world, castle_id);
}

/// The clan id owning `castle_id` (0 = NPC/none).
fn owner_clan_id(world: &World, castle_id: i32) -> i32 {
    owner_clan_id_opt(world, castle_id).unwrap_or(0)
}

pub(crate) fn owner_clan_id_opt(world: &World, castle_id: i32) -> Option<i32> {
    world
        .clans
        .values()
        .find(|c| c.castle_id == castle_id)
        .map(|c| c.id)
}

/// Broadcast `SystemMessage(id, castleName = castle_id)` to every online player.
fn broadcast_sm(world: &World, message_id: i16, castle_id: i32) {
    let pkt = server_packets::system_message_with(message_id, &[SmParam::CastleName(castle_id)]);
    world.broadcast_to_all_online(&pkt);
}

/// Every online member of every clan registered on `castle_id`'s siege — the
/// attacker and defender loops `Siege.endSiege` runs back to back.
fn registered_online_members(world: &World, castle_id: i32) -> Vec<i32> {
    let Some(siege) = world.sieges.get(&castle_id) else {
        return Vec::new();
    };
    let clan_ids: Vec<i32> = siege.clans.iter().map(|c| c.clan_id).collect();
    clan_ids
        .into_iter()
        .flat_map(|clan_id| super::clans::online_members(world, clan_id))
        .collect()
}
