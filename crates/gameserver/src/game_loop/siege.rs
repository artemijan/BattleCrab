//! Castle siege lifecycle — Java `Siege.startSiege`/`endSiege`, the timed-event
//! skeleton: announce the start to everyone, set the in-progress flag, and
//! schedule the auto-end after the siege length; the auto-end announces the
//! finish and clears the flag.
//!
//! The battlefield itself — teleport, control/flame towers, castle doors, siege
//! guards, the siege-zone PvP, and the winner/ownership change — is a later
//! milestone (TODO(G24) at the call sites).

use crate::db::DbCommand;
use crate::model::components::Position;
use crate::model::door::Door;
use crate::model::siege::SiegeClanType;
use crate::model::Player;
use crate::network::server_packets::{self, sm_ids, SmParam};
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::ms_to_ticks;

/// `SiegeManager.getSiegeLength()` — `SiegeLength = 120` (minutes) in Siege.ini.
const SIEGE_LENGTH_MIN: i32 = 120;

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

    // "The <castle> siege has started." + the siege sound, to everyone.
    broadcast_sm(world, sm_ids::THE_S1_SIEGE_HAS_STARTED, castle_id);
    broadcast_to_all(world, &server_packets::play_sound("systemmsg_eu.17"));

    // Auto-end after the siege length (Java `ScheduleEndSiegeTask`).
    let fire_at = world.tick + ms_to_ticks(SIEGE_LENGTH_MIN * 60 * 1000);
    world.scheduler.schedule(fire_at, ScheduledTask::SiegeEnd { castle_id });

    // `teleportPlayer(NotOwner, TOWN)` — clear the battlefield of everyone but
    // the owning clan. TODO(G24): attackers/defenders re-enter through their
    // siege HQ / flags (unported), so for now they're simply evicted too.
    teleport_non_owners(world, castle_id);

    // `_castle.spawnDoor()` — close the castle gates at full HP for the battle.
    spawn_castle_doors(world, castle_id, false);

    // TODO(G24): updatePlayerSiegeStateFlags, spawn control/flame towers + siege
    // guards (Castle.getZone().setActive is modelled by the in-progress flag the
    // siege-zone PvP check reads).
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

    broadcast_sm(world, sm_ids::THE_S1_SIEGE_HAS_FINISHED, castle_id);
    broadcast_to_all(world, &server_packets::play_sound("systemmsg_eu.18"));

    // The winner is whoever owns the castle at the end (an attacker only owns it
    // if they captured it mid-siege via `capture`).
    match world.clans.values().find(|c| c.castle_id == castle_id).map(|c| (c.id, c.name.clone())) {
        Some((owner_id, owner_name)) => {
            // "Clan <owner> is victorious over <castle>'s castle siege!"
            let pkt = server_packets::system_message_with(
                sm_ids::CLAN_S1_IS_VICTORIOUS_OVER_S2_S_CASTLE_SIEGE,
                &[SmParam::Text(owner_name), SmParam::CastleName(castle_id)],
            );
            broadcast_to_all(world, &pkt);
            // owner_id == first_owner → the defender held; otherwise an attacker
            // captured it. TODO(G24): increaseBloodAllianceCount (unchanged) /
            // setTicketBuyCount(0) + Hero.setCastleTaken (captured) — the clan
            // blood-alliance count, castle ticket count and nobles aren't modelled.
            let _ = (owner_id, first_owner);
        }
        None => broadcast_sm(world, sm_ids::THE_SIEGE_OF_S1_HAS_ENDED_IN_A_DRAW, castle_id),
    }

    // `teleportPlayer(NotOwner, TOWN)` — clear the battlefield at the end too.
    teleport_non_owners(world, castle_id);
    // `_castle.spawnDoor()` — restore the gates to full HP + closed.
    spawn_castle_doors(world, castle_id, false);
    // TODO(G24): despawn control/flame towers + siege guards.
}

/// Java `Castle.setOwner` (from the throne-room artifact) + `Siege.midVictory`
/// core: an attacker captures the castle mid-siege. Ownership transfers to
/// `new_clan_id`; the old owner/defenders become attackers and the captor
/// becomes the OWNER defender.
///
/// TODO(G24): the trigger (the artifact NPC), teleport-attackers-to-flag, the
/// weakened-door respawn, tower removal, residential skills and crests.
// The throne-room artifact that calls this is an unported spawn, so nothing in
// production reaches `capture` yet — the engine + its end-to-end test are ready
// for when it lands.
#[allow(dead_code)]
pub(crate) fn capture(world: &mut World, castle_id: i32, new_clan_id: i32) {
    if !world.sieges.get(&castle_id).is_some_and(|s| s.in_progress) {
        return;
    }
    // Transfer ownership: the old owner loses `hasCastle`, the captor gains it.
    if let Some(old) = owner_clan_id_opt(world, castle_id) {
        if let Some(c) = world.clans.get_mut(&old) {
            c.castle_id = 0;
        }
        let _ = world.db.send(DbCommand::UpdateClanCastle { clan_id: old, castle_id: 0 });
    }
    if let Some(c) = world.clans.get_mut(&new_clan_id) {
        c.castle_id = castle_id;
    }
    let _ = world.db.send(DbCommand::UpdateClanCastle { clan_id: new_clan_id, castle_id });

    // Reshuffle siege roles: every other side becomes an attacker, the captor
    // becomes the OWNER; then re-persist the changed rows.
    let changed: Vec<(i32, i32)> = match world.sieges.get_mut(&castle_id) {
        Some(siege) => {
            for sc in siege.clans.iter_mut() {
                if sc.clan_id != new_clan_id
                    && matches!(
                        sc.kind,
                        SiegeClanType::Owner | SiegeClanType::Defender | SiegeClanType::DefenderPending
                    )
                {
                    sc.kind = SiegeClanType::Attacker;
                }
            }
            match siege.clans.iter_mut().find(|c| c.clan_id == new_clan_id) {
                Some(sc) => sc.kind = SiegeClanType::Owner,
                None => siege.add_clan(new_clan_id, SiegeClanType::Owner),
            }
            siege.clans.iter().map(|c| (c.clan_id, c.kind.as_db())).collect()
        }
        None => Vec::new(),
    };
    for (clan_id, kind) in changed {
        let _ = world.db.send(DbCommand::SaveSiegeClan { castle_id, clan_id, kind });
    }

    // `_castle.spawnDoor(true)` — respawn the (now the captor's) gates at 50% HP.
    spawn_castle_doors(world, castle_id, true);
}

/// The object ids of a castle's doors — the doors standing inside its siege
/// zone (Java `Door.getCastle()` is region-based; the siege-zone polygon is the
/// port's proxy for the castle grounds).
fn castle_door_oids(world: &World, castle_id: i32) -> Vec<i32> {
    world
        .door_regions
        .values()
        .flatten()
        .copied()
        .filter(|&oid| {
            world
                .objects
                .get_component::<Position>(&oid)
                .and_then(|p| world.data.zone_data.siege_castle_at(p.x, p.y, p.z))
                == Some(castle_id)
        })
        .collect()
}

/// Java `Castle.spawnDoor(isDoorWeak)`: revive breached doors to full (or half)
/// HP and close any open ones. Called at siege start/end (full) and on capture
/// (weak, 50%).
fn spawn_castle_doors(world: &mut World, castle_id: i32, weak: bool) {
    for oid in castle_door_oids(world, castle_id) {
        let (door_id, dead) = match world.objects.get_component::<Door>(&oid) {
            Some(d) => (d.door_id, d.current_hp <= 0),
            None => continue,
        };
        if dead {
            let max_hp = world.data.door_data.get(door_id).map(|t| t.hp_max).unwrap_or(1);
            if let Some(d) = world.objects.get_component_mut::<Door>(&oid) {
                d.current_hp = if weak { (max_hp / 2).max(1) } else { max_hp };
            }
        }
        if world.geo.doors.is_open(door_id) {
            super::doors::close_door(world, oid);
        }
    }
}

/// Apply siege damage to a castle door; at 0 HP it's breached (opens). Returns
/// whether it broke on this hit. TODO(G24): the attack trigger — `DoorAction`
/// click-to-target + the melee/skill path against a door — is unported, so
/// nothing reaches this in production yet.
#[allow(dead_code)]
pub(crate) fn damage_door(world: &mut World, door_oid: i32, damage: i32) -> bool {
    let breached = {
        let Some(d) = world.objects.get_component_mut::<Door>(&door_oid) else { return false };
        if d.current_hp <= 0 {
            return false; // already breached
        }
        d.current_hp = (d.current_hp - damage).max(0);
        d.current_hp == 0
    };
    if breached {
        super::doors::open_door(world, door_oid); // breach — the gate swings open
        // TODO(G24): broadcast the reduced HP too (DoorStatusUpdate showHp).
    }
    breached
}

/// The clan id owning `castle_id` (0 = NPC/none).
fn owner_clan_id(world: &World, castle_id: i32) -> i32 {
    owner_clan_id_opt(world, castle_id).unwrap_or(0)
}

fn owner_clan_id_opt(world: &World, castle_id: i32) -> Option<i32> {
    world.clans.values().find(|c| c.castle_id == castle_id).map(|c| c.id)
}

/// `Siege.teleportPlayer(NotOwner, TOWN)`: send every player standing in the
/// castle's siege zone who isn't in the owning clan (nor a GM) to their nearest
/// town.
fn teleport_non_owners(world: &mut World, castle_id: i32) {
    let owner_clan_id = world.clans.values().find(|c| c.castle_id == castle_id).map(|c| c.id).unwrap_or(0);
    // Collect first — teleporting mutates the world and re-runs visibility.
    let targets: Vec<i32> = world
        .clients
        .values()
        .filter_map(|cs| match cs {
            ClientSession::InGame(s) => Some(s.player_object_id()),
            _ => None,
        })
        .filter(|&oid| {
            let Some(p) = world.objects.get_component::<Player>(&oid) else { return false };
            // Owner clan stays; GMs (canOverrideCond CASTLE_CONDITIONS) stay.
            if (owner_clan_id != 0 && p.clan_id == owner_clan_id) || p.is_gm(&world.data) {
                return false;
            }
            world
                .objects
                .get_component::<Position>(&oid)
                .and_then(|pos| world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z))
                == Some(castle_id)
        })
        .collect();

    for oid in targets {
        let Some(pos) = world.objects.get_component::<Position>(&oid).copied() else { continue };
        let race = world
            .objects
            .get_component::<Player>(&oid)
            .and_then(|p| crate::enums::Race::from_ordinal(p.race))
            .unwrap_or(crate::enums::Race::Human);
        if let Some((x, y, z)) = world.data.map_region.town_respawn(pos.x, pos.y, pos.z, race, 0) {
            super::death::teleport_player(world, oid, x, y, z);
        }
    }
}

/// Broadcast `SystemMessage(id, castleName = castle_id)` to every online player.
fn broadcast_sm(world: &World, message_id: i16, castle_id: i32) {
    let pkt = server_packets::system_message_with(message_id, &[SmParam::CastleName(castle_id)]);
    broadcast_to_all(world, &pkt);
}

/// Java `Broadcast.toAllOnlinePlayers`.
fn broadcast_to_all(world: &World, pkt: &[u8]) {
    for cs in world.clients.values() {
        if let ClientSession::InGame(_) = cs {
            cs.send(pkt.to_vec());
        }
    }
}
