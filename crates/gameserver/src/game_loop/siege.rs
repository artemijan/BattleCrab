//! Castle siege lifecycle — Java `Siege.startSiege`/`endSiege`, the timed-event
//! skeleton: announce the start to everyone, set the in-progress flag, and
//! schedule the auto-end after the siege length; the auto-end announces the
//! finish and clears the flag.
//!
//! The battlefield itself — teleport, control/flame towers, castle doors, siege
//! guards, the siege-zone PvP, and the winner/ownership change — is a later
//! milestone (TODO(G24) at the call sites).

use crate::db::DbCommand;
use crate::model::components::{Position, RegionCell};
use crate::model::door::Door;
use crate::model::siege::{SiegeClanType, SiegeSpawn};
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

    // Java `updatePlayerSiegeStateFlags(false)` + `updateZoneStatusForCharacters
    // Inside`: now that the siege is active, participants standing in the zone
    // gain the in-siege crown (UserInfo 0x80) + attackable icon. Runs before the
    // teleport below, matching Java's order.
    super::zones::refresh_siege_zone_for_all(world);

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
    // `spawnControlTower()` / `spawnFlameTower()` + `spawnSiegeGuard()`.
    let towers = world.data.siege_towers.get(&castle_id).cloned().unwrap_or_default();
    spawn_siege_npcs(world, castle_id, &towers);
    let guards = world.siege_guards.get(&castle_id).cloned().unwrap_or_default();
    spawn_siege_npcs(world, castle_id, &guards);

    // TODO(G24): updatePlayerSiegeStateFlags; the control-tower destruction
    // mechanic (destroying towers weakens the defenders' respawn). The
    // Castle.getZone().setActive is modelled by the in-progress flag the
    // siege-zone PvP check reads.
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
    // in-siege crown/icon from everyone who was on the (now inactive) field.
    super::zones::refresh_siege_zone_for_all(world);

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
    // Despawn the siege guards (+ any towers) from the battlefield.
    despawn_siege_npcs(world, castle_id);
}

/// Java `Castle.setOwner` (from the throne-room artifact) + `Siege.midVictory`
/// core: an attacker captures the castle mid-siege. Ownership transfers to
/// `new_clan_id`; the old owner/defenders become attackers and the captor
/// becomes the OWNER defender.
///
/// Reached in production: the Holy Artifact (type `Artefact`, e.g. Gludio's
/// 35063) is a permanent castle spawn, so an attacker touching it during an
/// active siege calls [`try_capture_artifact`] → here.
///
/// TODO(G24): teleport-attackers-to-flag, the weakened-door respawn, tower
/// removal, residential skills and crests.
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

/// Spawn a set of siege NPCs (the stationed guards / control + flame towers)
/// onto the battlefield, tracking their object ids on the siege for despawn at
/// the end. NPCs carry their template AI, so aggressive guards engage attackers.
fn spawn_siege_npcs(world: &mut World, castle_id: i32, spawns: &[SiegeSpawn]) {
    for s in spawns {
        if let Some(oid) = crate::model::npc::spawn_npc_at(world, s.npc_id, s.x, s.y, s.z, s.heading) {
            super::death::introduce_npc(world, oid);
            // Java `spawnControlTower` counts the live control towers.
            let is_control_tower = world.data.npc_data.get(s.npc_id).is_some_and(|t| t.type_name == "ControlTower");
            if let Some(siege) = world.sieges.get_mut(&castle_id) {
                siege.spawned_npcs.push(oid);
                if is_control_tower {
                    siege.control_tower_count += 1;
                }
            }
        }
    }
}

/// Whether an NPC is a siege tower (control / flame) standing in an active
/// siege zone — attackable so attackers can tear it down.
pub(crate) fn attackable_siege_tower(world: &World, npc_oid: i32) -> bool {
    let is_tower = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .and_then(|n| n.template(world))
        .is_some_and(|t| matches!(t.type_name.as_str(), "ControlTower" | "FlameTower"));
    if !is_tower {
        return false;
    }
    let Some(pos) = world.objects.get_component::<Position>(&npc_oid) else { return false };
    match world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z) {
        Some(castle_id) => world.sieges.get(&castle_id).is_some_and(|s| s.in_progress),
        None => false,
    }
}

/// Java `Siege.killedCT` — a control tower fell; decrement its castle's live
/// count. At 0 the defenders lose their castle respawn.
pub(crate) fn killed_control_tower(world: &mut World, npc_oid: i32) {
    let is_ct = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.type_name == "ControlTower");
    if !is_ct {
        return;
    }
    let Some(pos) = world.objects.get_component::<Position>(&npc_oid).copied() else { return };
    let Some(castle_id) = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z) else { return };
    if let Some(siege) = world.sieges.get_mut(&castle_id) {
        siege.control_tower_count = (siege.control_tower_count - 1).max(0);
        // The count has no gameplay outcome in Interlude Classic (see the field
        // doc on `Siege.control_tower_count`): a normal resurrection is blocked
        // in a siege regardless, and the count only selects the rejection
        // message. TODO(G24): honor that message once resurrection is ported.
    }
}

/// Siege.ini `MaxFlags` (dist default) — HQ flags a clan may plant at once.
const FLAG_MAX_COUNT: i32 = 1;
/// Java `HeadquarterCreate.HQ_NPC_ID` — the "Headquarters" siege flag NPC.
const HQ_NPC_ID: i32 = 35062;

/// Java `HeadquarterCreate.instant` + `BuildCampSkillCondition`: the leader of an
/// attacker clan plants an HQ flag in the siege zone, becoming the clan's respawn
/// point until a defender destroys it. Returns whether a flag was placed.
pub(crate) fn place_siege_flag(world: &mut World, player_oid: i32) -> bool {
    let Some(clan_id) = world.objects.get_component::<Player>(&player_oid).map(|p| p.clan_id) else { return false };
    let Some((x, y, z, heading)) = world.objects.get_component::<Position>(&player_oid).map(|p| (p.x, p.y, p.z, p.heading))
    else {
        return false;
    };
    // `HeadquarterCreate`: caster must be a clan leader (leaderId == objectId;
    // a player's object id is its char id).
    if clan_id == 0 || world.clans.get(&clan_id).map(|c| c.leader_id) != Some(player_oid) {
        return false;
    }
    // `BuildCampSkillCondition`: an active siege at this spot where the clan is
    // registered as an attacker. TODO(G24): also require the `ZoneId.HQ` sub-zone.
    let Some(castle_id) = world.data.zone_data.siege_castle_at(x, y, z) else { return false };
    let ok = world.sieges.get(&castle_id).is_some_and(|s| {
        s.in_progress
            && s.clans.iter().any(|c| c.clan_id == clan_id && c.kind == SiegeClanType::Attacker)
            && s.flag_count(clan_id) < FLAG_MAX_COUNT // getNumFlags < MaxFlags
    });
    if !ok {
        return false;
    }
    // Plant it at z+50 (Java `spawnMe(x, y, z + 50)`) and register it.
    let Some(oid) = crate::model::npc::spawn_npc_at(world, HQ_NPC_ID, x, y, z + 50, heading) else { return false };
    super::death::introduce_npc(world, oid);
    if let Some(siege) = world.sieges.get_mut(&castle_id) {
        siege.add_flag(clan_id, oid);
        // Tracked for cleanup too, so a flag still standing at siege end is
        // despawned with the rest (`removeFlags`).
        siege.spawned_npcs.push(oid);
    }
    true
}

/// Whether an NPC is a registered HQ flag standing in an active siege —
/// attackable so defenders can destroy it (Java `SiegeFlag.isAutoAttackable`).
pub(crate) fn attackable_siege_flag(world: &World, npc_oid: i32) -> bool {
    world.sieges.values().any(|s| s.in_progress && s.flags.iter().any(|&(_, oid)| oid == npc_oid))
}

/// Java `Siege.killedFlag` — a defender destroyed an attacker's HQ flag; drop it
/// so the attacker loses that respawn point.
pub(crate) fn killed_siege_flag(world: &mut World, npc_oid: i32) {
    for siege in world.sieges.values_mut() {
        if siege.remove_flag(npc_oid) {
            break;
        }
    }
}

/// Whether `player_oid`'s clan defends `castle_id` — the castle owner or a
/// registered defender clan (Java `Siege.checkIsDefender`, which counts the
/// owner). Non-players and clanless players are never defenders.
pub(crate) fn is_siege_defender(world: &World, castle_id: i32, player_oid: i32) -> bool {
    let Some(clan_id) = world.objects.get_component::<Player>(&player_oid).map(|p| p.clan_id) else {
        return false;
    };
    if clan_id == 0 {
        return false;
    }
    if world.clans.get(&clan_id).is_some_and(|c| c.castle_id == castle_id) {
        return true;
    }
    world.sieges.get(&castle_id).is_some_and(|s| {
        s.clans
            .iter()
            .any(|c| c.clan_id == clan_id && matches!(c.kind, SiegeClanType::Owner | SiegeClanType::Defender))
    })
}

/// The castle whose active siege a stationed guard (`Defender`) is standing in,
/// if any — the guard's employer.
pub(crate) fn active_siege_guard_castle(world: &World, guard_oid: i32) -> Option<i32> {
    let is_guard = world
        .objects
        .get_component::<crate::model::npc::Npc>(&guard_oid)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.type_name == "Defender");
    if !is_guard {
        return None;
    }
    let pos = world.objects.get_component::<Position>(&guard_oid)?;
    let castle_id = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z)?;
    world.sieges.get(&castle_id).filter(|s| s.in_progress).map(|_| castle_id)
}

/// Whether a stationed siege guard (`Defender`) is attackable by `attacker_oid`:
/// the guard stands in an active siege zone and the attacker is not one of that
/// castle's defenders (Java `Defender.isAutoAttackable` — attackable during the
/// siege by anyone who isn't a registered defender of this field). The same
/// predicate decides who a guard aggros (its enemies).
pub(crate) fn attackable_siege_guard(world: &World, guard_oid: i32, attacker_oid: i32) -> bool {
    match active_siege_guard_castle(world, guard_oid) {
        Some(castle_id) => !is_siege_defender(world, castle_id, attacker_oid),
        None => false,
    }
}

/// Despawn every NPC spawned for this siege (Java `removeSiegeGuards` + the
/// control/flame towers — the latter unported yet).
fn despawn_siege_npcs(world: &mut World, castle_id: i32) {
    let oids = world.sieges.get_mut(&castle_id).map(|s| std::mem::take(&mut s.spawned_npcs)).unwrap_or_default();
    for oid in oids {
        if let Some(region) = world.objects.get_component::<RegionCell>(&oid).map(|r| r.0) {
            super::death::despawn_npc(world, oid, region);
        }
    }
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

/// Whether `door_oid` is a castle door standing in a siege zone whose siege is
/// in progress — i.e. currently attackable/breachable (Java `Door.isAttackable`
/// during a siege).
pub(crate) fn attackable_door(world: &World, door_oid: i32) -> bool {
    if !world.objects.has_component::<Door>(&door_oid) {
        return false;
    }
    let Some(pos) = world.objects.get_component::<Position>(&door_oid) else { return false };
    match world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z) {
        Some(castle_id) => world.sieges.get(&castle_id).is_some_and(|s| s.in_progress),
        None => false,
    }
}

/// Apply siege damage to a castle door; at 0 HP it's breached (opens). Returns
/// whether it broke on this hit. Driven by the melee path against a targeted
/// door (`combat::attack_door`).
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

/// The throne-room Holy Artifact capture (Java `Artefact.onAction` →
/// `Castle.setOwner` → `Siege.midVictory`): an attacker clan member touching the
/// artifact during an active siege takes the castle. No-op otherwise.
pub(crate) fn try_capture_artifact(world: &mut World, player_oid: i32, artifact_oid: i32) {
    let Some(pos) = world.objects.get_component::<Position>(&artifact_oid).copied() else { return };
    let Some(castle_id) = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z) else { return };
    if !world.sieges.get(&castle_id).is_some_and(|s| s.in_progress) {
        return;
    }
    let clan_id = world.objects.get_component::<Player>(&player_oid).map(|p| p.clan_id).unwrap_or(0);
    if clan_id == 0 {
        return;
    }
    // Only a registered attacker can seize the castle.
    let is_attacker = world
        .sieges
        .get(&castle_id)
        .is_some_and(|s| s.clans.iter().any(|c| c.clan_id == clan_id && c.kind == SiegeClanType::Attacker));
    if is_attacker {
        capture(world, castle_id, clan_id);
    }
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

// ---------------------------------------------------------------------------
// The automatic weekly schedule (`SiegeSchedule.xml`). G24 slice 1.
// ---------------------------------------------------------------------------

const MILLIS_PER_DAY: i64 = 86_400_000;
const MILLIS_PER_HOUR: i64 = 3_600_000;
const TICKS_PER_SECOND: u64 = 10;

/// The next `weekday`@`hour`:00 **UTC** strictly after `now_millis` (Java
/// `SiegeScheduleDate` + `Calendar` next-occurrence, computed in UTC — Rust std
/// has no timezone, so this differs from Java's server-local time by the
/// deployment's UTC offset; the weekly cadence itself is exact).
///
/// `weekday` is `Mon=0..Sun=6`. 1970-01-01 (epoch day 0) was a Thursday, so
/// `weekday_of(day) = (day + 3) % 7`.
pub(crate) fn next_siege_millis(now_millis: i64, weekday: u32, hour: u32) -> i64 {
    let now_day = now_millis.div_euclid(MILLIS_PER_DAY);
    let now_weekday = (now_day + 3).rem_euclid(7) as u32;
    let mut delta = (weekday as i64 - now_weekday as i64).rem_euclid(7);
    let mut candidate = (now_day + delta) * MILLIS_PER_DAY + hour as i64 * MILLIS_PER_HOUR;
    if candidate <= now_millis {
        delta += 7;
        candidate = (now_day + delta) * MILLIS_PER_DAY + hour as i64 * MILLIS_PER_HOUR;
    }
    candidate
}

/// Arm each enabled castle's next scheduled siege. Called once the per-castle
/// `Siege`s exist (the `SiegesLoaded` boot handler).
pub(crate) fn schedule_all_at_boot(world: &mut World) {
    let now = commons::util::now_millis();
    let entries: Vec<(i32, u32, u32)> = world
        .data
        .siege_schedule
        .iter()
        .filter(|(_, e)| e.enabled)
        .map(|(&id, e)| (id, e.weekday, e.hour))
        .collect();
    for (castle_id, weekday, hour) in entries {
        arm_next_siege(world, castle_id, weekday, hour, now);
    }
}

fn arm_next_siege(world: &mut World, castle_id: i32, weekday: u32, hour: u32, now: i64) {
    let at_millis = next_siege_millis(now, weekday, hour);
    let delay_ticks = ((at_millis - now).max(0) / 1000) as u64 * TICKS_PER_SECOND;
    world.scheduler.schedule(world.tick + delay_ticks, ScheduledTask::SiegeStart { castle_id });
}

/// A scheduled siege's start time arrived: begin it, and re-arm next week so
/// the timer perpetuates itself (whether or not this siege actually runs —
/// a castle with no registered attackers just holds, as in Java).
pub(crate) fn handle_scheduled_siege_start(world: &mut World, castle_id: i32) {
    start_siege(world, castle_id);
    if let Some(e) = world.data.siege_schedule.get(&castle_id).copied() {
        if e.enabled {
            arm_next_siege(world, castle_id, e.weekday, e.hour, commons::util::now_millis());
        }
    }
}
