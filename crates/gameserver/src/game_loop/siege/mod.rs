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
mod packets;
mod registration;
mod schedule;

use crate::db::DbCommand;
use crate::game_loop::guard::clan_of_or_zero;
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::npc_template;
use crate::game_loop::helpers::send_to_client;
use crate::model::Player;
use crate::model::components::{AdvancedHeadquarter, Position};
use crate::model::door::Door;
use crate::model::siege::{SiegeClanType, SiegeSpawn};
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::scheduler::ScheduledTask;
use crate::world::World;
pub(crate) use battlefield::*;
pub(crate) use capture::*;
pub(crate) use doors::*;
pub(crate) use packets::*;
pub(crate) use registration::*;
pub(crate) use schedule::*;

use super::helpers::send_sm_bare_to_client as send_sm_to;
use super::helpers::{ms_to_ticks, send_sm_bare_to_player, send_sm_to_client};

/// `SiegeManager.getSiegeLength()` — `SiegeLength = 120` (minutes) in Siege.ini.
const SIEGE_LENGTH_MIN: i32 = 120;

/// `Config.SIEGE_HOUR_LIST` — `Feature.ini`'s `SiegeHourList = 16,20`, the hours
/// a castle owner may choose from for their siege (via `RequestSetCastleSiegeTime`).
const SIEGE_HOUR_LIST: &[u32] = &[16, 20];

/// `Siege.startSiege` (lifecycle slice). Called only with a registered attacker
/// (the admin path guards that).
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
    super::zones::refresh_siege_zone_for_all(world);

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
    world.scheduler.schedule(
        world.tick + delay,
        crate::scheduler::ScheduledTask::SiegeFame { player_oid },
    );
}

/// One firing of the fame task: pay, then decide whether there is a next one.
///
/// The three refusals are Java's `FameTask.run`, in Java's order. Note the
/// first two only skip *this* payment — Java's task keeps ticking for a corpse
/// in the zone and pays again once it stands up — while leaving the zone is
/// what actually ends it.
pub(crate) fn handle_siege_fame(world: &mut World, player_oid: i32) {
    if !crate::game_loop::pvp::is_in_siege(world, player_oid) {
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
        if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
            p.fame += amount;
        }
        crate::game_loop::clans::send_sm_with(
            world,
            player_oid,
            crate::network::server_packets::sm_ids::YOU_HAVE_ACQUIRED_S1_FAME,
            &[crate::network::server_packets::SmParam::Int(amount)],
        );
        crate::game_loop::player_info::broadcast_user_info(world, player_oid);
    }
    // Still in the zone, so it keeps ticking either way.
    let delay = world.cfg.character.castle_zone_fame_task_frequency as u64 * 10;
    world.scheduler.schedule(
        world.tick + delay,
        crate::scheduler::ScheduledTask::SiegeFame { player_oid },
    );
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
        super::player_info::broadcast_user_info(world, member);
        super::pvp::broadcast_siege_relation(world, member);
    }
}

/// `Siege.endSiege` — announce the finish, declare the winner (or a draw), and
/// clear the battlefield.
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
    super::zones::refresh_siege_zone_for_all(world);
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
        }
        None => broadcast_sm(
            world,
            sm_ids::THE_SIEGE_OF_S1_HAS_ENDED_IN_A_DRAW,
            castle_id,
        ),
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
