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
    broadcast_to_all(world, &server_packets::play_sound("systemmsg_eu.17"));

    // Auto-end after the siege length (Java `ScheduleEndSiegeTask`).
    let fire_at = world.tick + ms_to_ticks(SIEGE_LENGTH_MIN * 60 * 1000);
    world
        .scheduler
        .schedule(fire_at, ScheduledTask::SiegeEnd { castle_id });

    // `teleportPlayer(NotOwner, TOWN)` — clear the battlefield of everyone but
    // the owning clan. TODO(G24): attackers/defenders re-enter through their
    // siege HQ / flags (unported), so for now they're simply evicted too.
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
            broadcast_to_all(world, &pkt);
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
}

/// `SiegeManager.getBloodAllianceReward()` — `Siege.ini BloodAllianceReward = 0`
/// on this dist, so holding a castle awards nothing in Interlude Classic. Kept
/// as the single knob: raising it lights up the whole [`increase_blood_alliance`]
/// path without any other change.
pub(crate) const BLOOD_ALLIANCE_REWARD: i32 = 0;

/// Java `Clan.increaseBloodAllianceCount` — the owner held its castle through
/// the siege, so bump (and persist) its blood-alliance count by the reward.
fn increase_blood_alliance(world: &mut World, clan_id: i32) {
    let Some(clan) = world.clans.get_mut(&clan_id) else {
        return;
    };
    clan.blood_alliance_count += BLOOD_ALLIANCE_REWARD;
    let count = clan.blood_alliance_count;
    let _ = world
        .db
        .send(DbCommand::UpdateClanBloodAlliance { clan_id, count });
}

/// Java `Castle.setTicketBuyCount(0)` — the castle changed hands, so the former
/// owner's placed-mercenary count is cleared. A no-op (and no DB write) when it
/// was already 0, which it always is until the mercenary system lands.
fn reset_castle_ticket_count(world: &mut World, castle_id: i32) {
    let Some(castle) = world.castles.iter_mut().find(|c| c.id == castle_id) else {
        return;
    };
    if castle.ticket_buy_count == 0 {
        return;
    }
    castle.ticket_buy_count = 0;
    let _ = world.db.send(DbCommand::UpdateCastleTicketCount {
        castle_id,
        count: 0,
    });
}

/// `Hero.ACTION_CASTLE_TAKEN`.
const HERO_ACTION_CASTLE_TAKEN: i32 = 3;

/// Java `endSiege`'s `Hero.setCastleTaken` loop: every online **noble** member
/// of the capturing clan gets a `heroes_diary` "castle taken" entry (the
/// hero-eligibility record). A player's object id is their character id, so it
/// keys the diary row directly. The in-memory hero-diary display (only for a
/// currently-crowned hero) isn't modelled, so only the persistent row is written.
fn record_castle_taken_for_nobles(world: &mut World, clan_id: i32, castle_id: i32) {
    let now = commons::util::now_millis();
    let nobles: Vec<i32> = super::clans::online_members(world, clan_id)
        .into_iter()
        .filter(|oid| {
            world
                .objects
                .get_component::<Player>(oid)
                .is_some_and(|p| p.is_noble)
        })
        .collect();
    for char_id in nobles {
        let _ = world.db.send(DbCommand::SaveHeroDiary {
            char_id,
            time: now,
            action: HERO_ACTION_CASTLE_TAKEN,
            param: castle_id,
        });
    }
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
/// removal, and castle crests.
pub(crate) fn capture(world: &mut World, castle_id: i32, new_clan_id: i32) {
    if !world.sieges.get(&castle_id).is_some_and(|s| s.in_progress) {
        return;
    }
    let old_owner = owner_clan_id_opt(world, castle_id);
    // Transfer ownership: the old owner loses `hasCastle`, the captor gains it.
    if let Some(old) = old_owner {
        if let Some(c) = world.clans.get_mut(&old) {
            c.castle_id = 0;
        }
        let _ = world.db.send(DbCommand::UpdateClanCastle {
            clan_id: old,
            castle_id: 0,
        });
    }
    if let Some(c) = world.clans.get_mut(&new_clan_id) {
        c.castle_id = castle_id;
    }
    let _ = world.db.send(DbCommand::UpdateClanCastle {
        clan_id: new_clan_id,
        castle_id,
    });

    // Java `Castle.setOwner`: strip the castle's residential skills from the
    // former owner's online members, and grant them to the captor's.
    if let Some(old) = old_owner {
        for member in super::clans::online_members(world, old) {
            super::clans::remove_residential_skills(world, member, castle_id);
        }
    }
    for member in super::clans::online_members(world, new_clan_id) {
        super::clans::give_residential_skills(world, member, castle_id, new_clan_id);
    }

    // Reshuffle siege roles: every other side becomes an attacker, the captor
    // becomes the OWNER; then re-persist the changed rows.
    let changed: Vec<(i32, i32)> = match world.sieges.get_mut(&castle_id) {
        Some(siege) => {
            for sc in siege.clans.iter_mut() {
                if sc.clan_id != new_clan_id
                    && matches!(
                        sc.kind,
                        SiegeClanType::Owner
                            | SiegeClanType::Defender
                            | SiegeClanType::DefenderPending
                    )
                {
                    sc.kind = SiegeClanType::Attacker;
                }
            }
            match siege.clans.iter_mut().find(|c| c.clan_id == new_clan_id) {
                Some(sc) => sc.kind = SiegeClanType::Owner,
                None => siege.add_clan(new_clan_id, SiegeClanType::Owner),
            }
            siege
                .clans
                .iter()
                .map(|c| (c.clan_id, c.kind.as_db()))
                .collect()
        }
        None => Vec::new(),
    };
    for (clan_id, kind) in changed {
        let _ = world.db.send(DbCommand::SaveSiegeClan {
            castle_id,
            clan_id,
            kind,
        });
    }

    // `_castle.spawnDoor(true)` — respawn the (now the captor's) gates at 50% HP.
    spawn_castle_doors(world, castle_id, true);
}

/// Spawn a set of siege NPCs (the stationed guards / control + flame towers)
/// onto the battlefield, tracking their object ids on the siege for despawn at
/// the end. NPCs carry their template AI, so aggressive guards engage attackers.
fn spawn_siege_npcs(world: &mut World, castle_id: i32, spawns: &[SiegeSpawn]) {
    for s in spawns {
        if let Some(oid) =
            crate::model::npc::spawn_npc_at(world, s.npc_id, s.x, s.y, s.z, s.heading)
        {
            super::death::introduce_npc(world, oid);
            // Java `spawnControlTower` counts the live control towers.
            let is_control_tower = world
                .data
                .npc_data
                .get(s.npc_id)
                .is_some_and(|t| t.type_name == "ControlTower");
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
    let Some(pos) = world.objects.get_component::<Position>(&npc_oid) else {
        return false;
    };
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
    let Some(pos) = world.objects.get_component::<Position>(&npc_oid).copied() else {
        return;
    };
    let Some(castle_id) = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z) else {
        return;
    };
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
    let Some(clan_id) = world
        .objects
        .get_component::<Player>(&player_oid)
        .map(|p| p.clan_id)
    else {
        return false;
    };
    let Some((x, y, z, heading)) = world
        .objects
        .get_component::<Position>(&player_oid)
        .map(|p| (p.x, p.y, p.z, p.heading))
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
    let Some(castle_id) = world.data.zone_data.siege_castle_at(x, y, z) else {
        return false;
    };
    let ok = world.sieges.get(&castle_id).is_some_and(|s| {
        s.in_progress
            && s.clans
                .iter()
                .any(|c| c.clan_id == clan_id && c.kind == SiegeClanType::Attacker)
            && s.flag_count(clan_id) < FLAG_MAX_COUNT // getNumFlags < MaxFlags
    });
    if !ok {
        return false;
    }
    // Plant it at z+50 (Java `spawnMe(x, y, z + 50)`) and register it.
    let Some(oid) = crate::model::npc::spawn_npc_at(world, HQ_NPC_ID, x, y, z + 50, heading) else {
        return false;
    };
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
    world
        .sieges
        .values()
        .any(|s| s.in_progress && s.flags.iter().any(|&(_, oid)| oid == npc_oid))
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
    let Some(clan_id) = world
        .objects
        .get_component::<Player>(&player_oid)
        .map(|p| p.clan_id)
    else {
        return false;
    };
    if clan_id == 0 {
        return false;
    }
    if world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.castle_id == castle_id)
    {
        return true;
    }
    world.sieges.get(&castle_id).is_some_and(|s| {
        s.clans.iter().any(|c| {
            c.clan_id == clan_id && matches!(c.kind, SiegeClanType::Owner | SiegeClanType::Defender)
        })
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
    world
        .sieges
        .get(&castle_id)
        .filter(|s| s.in_progress)
        .map(|_| castle_id)
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
    let oids = world
        .sieges
        .get_mut(&castle_id)
        .map(|s| std::mem::take(&mut s.spawned_npcs))
        .unwrap_or_default();
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
            let max_hp = world
                .data
                .door_data
                .get(door_id)
                .map(|t| t.hp_max)
                .unwrap_or(1);
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
    let Some(pos) = world.objects.get_component::<Position>(&door_oid) else {
        return false;
    };
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
        let Some(d) = world.objects.get_component_mut::<Door>(&door_oid) else {
            return false;
        };
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
    let Some(pos) = world
        .objects
        .get_component::<Position>(&artifact_oid)
        .copied()
    else {
        return;
    };
    let Some(castle_id) = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z) else {
        return;
    };
    if !world.sieges.get(&castle_id).is_some_and(|s| s.in_progress) {
        return;
    }
    let clan_id = world
        .objects
        .get_component::<Player>(&player_oid)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    if clan_id == 0 {
        return;
    }
    // Only a registered attacker can seize the castle.
    let is_attacker = world.sieges.get(&castle_id).is_some_and(|s| {
        s.clans
            .iter()
            .any(|c| c.clan_id == clan_id && c.kind == SiegeClanType::Attacker)
    });
    if is_attacker {
        capture(world, castle_id, clan_id);
    }
}

/// The clan id owning `castle_id` (0 = NPC/none).
fn owner_clan_id(world: &World, castle_id: i32) -> i32 {
    owner_clan_id_opt(world, castle_id).unwrap_or(0)
}

fn owner_clan_id_opt(world: &World, castle_id: i32) -> Option<i32> {
    world
        .clans
        .values()
        .find(|c| c.castle_id == castle_id)
        .map(|c| c.id)
}

/// `Siege.teleportPlayer(NotOwner, TOWN)`: send every player standing in the
/// castle's siege zone who isn't in the owning clan (nor a GM) to their nearest
/// town.
fn teleport_non_owners(world: &mut World, castle_id: i32) {
    let owner_clan_id = world
        .clans
        .values()
        .find(|c| c.castle_id == castle_id)
        .map(|c| c.id)
        .unwrap_or(0);
    // Collect first — teleporting mutates the world and re-runs visibility.
    let targets: Vec<i32> = world
        .clients
        .values()
        .filter_map(|cs| match cs {
            ClientSession::InGame(s) => Some(s.player_object_id()),
            _ => None,
        })
        .filter(|&oid| {
            let Some(p) = world.objects.get_component::<Player>(&oid) else {
                return false;
            };
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
        let Some(pos) = world.objects.get_component::<Position>(&oid).copied() else {
            continue;
        };
        let race = world
            .objects
            .get_component::<Player>(&oid)
            .and_then(|p| crate::enums::Race::from_ordinal(p.race))
            .unwrap_or(crate::enums::Race::Human);
        if let Some((x, y, z)) = world
            .data
            .map_region
            .town_respawn(pos.x, pos.y, pos.z, race, 0)
        {
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
    world.scheduler.schedule(
        world.tick + delay_ticks,
        ScheduledTask::SiegeStart { castle_id },
    );
}

/// The castle's siege time (epoch-millis): the owner-chosen date when one is set
/// for the future, else the next fixed `SiegeSchedule.xml` slot. 0 when neither.
fn effective_siege_millis(world: &World, castle_id: i32, now_millis: i64) -> i64 {
    let chosen = world
        .castles
        .iter()
        .find(|c| c.id == castle_id)
        .map(|c| c.siege_date)
        .unwrap_or(0);
    if chosen > now_millis {
        return chosen;
    }
    world
        .data
        .siege_schedule
        .get(&castle_id)
        .filter(|e| e.enabled)
        .map(|e| next_siege_millis(now_millis, e.weekday, e.hour))
        .unwrap_or(0)
}

/// Whether the castle owner may still pick the siege hour (Java
/// `!isTimeRegistrationOver`).
fn can_pick_siege_time(world: &World, castle_id: i32) -> bool {
    world
        .castles
        .iter()
        .find(|c| c.id == castle_id)
        .is_some_and(|c| !c.time_registration_over)
}

/// A scheduled siege's start time arrived: begin it, and re-arm next week so
/// the timer perpetuates itself (whether or not this siege actually runs —
/// a castle with no registered attackers just holds, as in Java).
pub(crate) fn handle_scheduled_siege_start(world: &mut World, castle_id: i32) {
    start_siege(world, castle_id);
    // The owner's one-off chosen time is spent; clear it so the SiegeInfo window
    // and registration cut-off revert to the fixed schedule for the next cycle
    // (Java reopens time registration for the next siege).
    if let Some(c) = world.castles.iter_mut().find(|c| c.id == castle_id) {
        if c.siege_date != 0 {
            c.siege_date = 0;
            let _ = world.db.send(DbCommand::UpdateCastleSiegeTime {
                castle_id,
                siege_date: 0,
                time_registration_over: c.time_registration_over,
            });
        }
    }
    // TODO(G24): the auto-start still fires at the fixed `SiegeSchedule.xml`
    // hour, not the owner's chosen one — honoring the chosen hour in the timer
    // needs task cancellation the scheduler doesn't have yet. The choice is
    // reflected in the SiegeInfo window and the registration cut-off.
    if let Some(e) = world.data.siege_schedule.get(&castle_id).copied() {
        if e.enabled {
            arm_next_siege(
                world,
                castle_id,
                e.weekday,
                e.hour,
                commons::util::now_millis(),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Player-facing registration (Java `Siege.registerAttacker/registerDefender/
// approveSiegeDefenderClan/removeSiegeClan` + `checkIfCanRegister`)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Reachability — `RequestJoinSiege` (0xAD) and the `SiegeInfo` response
// ---------------------------------------------------------------------------

/// Send a SystemMessage to one client.
fn send_sm_to(world: &World, client_id: u32, id: i16) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::enter_world::system_message(id));
    }
}

/// The SystemMessage for a refusal, or `None` when nothing is said (success, or
/// a refusal whose Interlude message id isn't ported yet — the unchanged window
/// is the feedback).
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
        // Ported message id pending (window-only feedback): RegistrationOver,
        // SiegeInProgress, AllianceWithOwner, AlreadyRegisteredSameDay,
        // DefendingNpcCastle. TODO(G24).
        _ => None,
    }
}

/// `RequestJoinSiege` (0xAD): a `CS_MANAGE_SIEGE` clan leader registers as
/// attacker/defender (`isJoining==1`) or cancels (`isJoining==0`) for a castle
/// siege, then gets the refreshed `SiegeInfo` window (Java `listRegisterClan`).
pub(crate) fn handle_request_join_siege(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();

    let mut r = commons::network::PacketReader::new(body);
    let (Some(castle_id), Some(is_attacker), Some(is_joining)) =
        (r.read_i32(), r.read_i32(), r.read_i32())
    else {
        return;
    };

    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    let privs = p.clan_privs;
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
            return; // Java sends SM then returns; that id isn't ported. TODO(G24).
        }
        let outcome = register(world, castle_id, clan_id, is_attacker == 1, now);
        if let Some(sm) = outcome_sm(outcome) {
            send_sm_to(world, client_id, sm);
        }
    } else {
        remove_registration(world, castle_id, clan_id);
    }

    send_siege_info(world, client_id, castle_id, clan_id, player, now);
}

/// Java `Siege.listRegisterClan` → `new SiegeInfo(castle, player)`.
fn send_siege_info(
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
                SIEGE_HOUR_LIST
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
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(pkt);
    }
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
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();

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
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
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
    let Some(p) = world.objects.get_component::<crate::model::Player>(&player) else {
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
    if !SIEGE_HOUR_LIST
        .iter()
        .any(|&h| next_siege_millis(now, weekday, h) == chosen_millis)
    {
        return;
    }

    // Set the date, close the window, persist, and re-arm the auto-start.
    if let Some(c) = world.castles.iter_mut().find(|c| c.id == castle_id) {
        c.siege_date = chosen_millis;
        c.time_registration_over = true;
    }
    let _ = world.db.send(DbCommand::UpdateCastleSiegeTime {
        castle_id,
        siege_date: chosen_millis,
        time_registration_over: true,
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
    if owner_id != 0 {
        if let Some(e) = defender_entry(world, owner_id, 1) {
            entries.push(e);
        }
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
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(pkt);
    }
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
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(pkt);
    }
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
    let leader_name = clan
        .members
        .iter()
        .find(|m| m.char_id == clan.leader_id)
        .map(|m| m.name.clone())
        .unwrap_or_default();
    Some(server_packets::AttackerEntry {
        clan_id,
        name: clan.name.clone(),
        leader_name,
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
    let leader_name = clan
        .members
        .iter()
        .find(|m| m.char_id == clan.leader_id)
        .map(|m| m.name.clone())
        .unwrap_or_default();
    // The ally leader clan shares the ally id (Java: the leader clan's own id).
    let ally_leader_name = if clan.ally_id != 0 {
        world
            .clans
            .get(&clan.ally_id)
            .and_then(|a| a.members.iter().find(|m| m.char_id == a.leader_id))
            .map(|m| m.name.clone())
            .unwrap_or_default()
    } else {
        String::new()
    };
    Some(server_packets::DefenderEntry {
        clan_id,
        name: clan.name.clone(),
        leader_name,
        crest_id: clan.crest_id,
        type_value,
        ally_id: clan.ally_id,
        ally_name: clan.ally_name.clone(),
        ally_leader_name,
        ally_crest_id: clan.ally_crest_id,
    })
}
