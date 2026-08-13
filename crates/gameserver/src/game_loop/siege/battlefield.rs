//! The battlefield NPCs: control/flame towers, siege guards, siege flags
//! and the advanced headquarter.

use super::*;
/// Spawn a set of siege NPCs (the stationed guards / control + flame towers)
/// onto the battlefield, tracking their object ids on the siege for despawn at
/// the end. NPCs carry their template AI, so aggressive guards engage attackers.
pub(super) fn spawn_siege_npcs(world: &mut World, castle_id: i32, spawns: &[SiegeSpawn]) {
    for s in spawns {
        if let Some(oid) =
            crate::model::npc::spawn_npc_at(world, s.npc_id, s.x, s.y, s.z, s.heading)
        {
            crate::game_loop::death::introduce_npc(world, oid);
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

/// The castle whose siege is **currently running** over `(x, y, z)` — the siege
/// zone lookup plus the `in_progress` test every "is this spot a live
/// battlefield" check needs.
pub(crate) fn active_siege_castle_at(world: &World, x: i32, y: i32, z: i32) -> Option<i32> {
    let castle_id = world.data.zone_data.siege_castle_at(x, y, z)?;
    world
        .sieges
        .get(&castle_id)
        .filter(|s| s.in_progress)
        .map(|_| castle_id)
}

/// Whether an NPC is a siege tower (control / flame) standing in an active
/// siege zone — attackable so attackers can tear it down.
pub(crate) fn attackable_siege_tower(world: &World, npc_oid: i32) -> bool {
    npc_template(world, npc_oid)
        .is_some_and(|t| matches!(t.type_name.as_str(), "ControlTower" | "FlameTower"))
        && crate::game_loop::pvp::active_siege_castle(world, npc_oid).is_some()
}

/// Java `Siege.killedCT` — a control tower fell; decrement its castle's live
/// count. At 0 the defenders lose their castle respawn.
pub(crate) fn killed_control_tower(world: &mut World, npc_oid: i32) {
    let is_ct = npc_template(world, npc_oid).is_some_and(|t| t.type_name == "ControlTower");
    if !is_ct {
        return;
    }
    let Some(pos) = maybe_position(world, npc_oid) else {
        return;
    };
    let Some(castle_id) = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z) else {
        return;
    };
    if let Some(siege) = world.sieges.get_mut(&castle_id) {
        siege.control_tower_count = (siege.control_tower_count - 1).max(0);
        // The count picks the *rejection message* a normal resurrection gets
        // during a siege — every branch of `ConditionPlayerCanResurrect`
        // refuses, so the count never decides whether the revive happens, only
        // what the caster is told (`death::siege_resurrect_refusal`). It does
        // **not** gate the restart-point respawn; the tower consequence that
        // does bite is the mass gatekeeper's 8-minute delay
        // (`scripts::castle_services`).
    }
}

/// Siege.ini `MaxFlags` (dist default) — HQ flags a clan may plant at once.
const FLAG_MAX_COUNT: i32 = 1;
/// Java `HeadquarterCreate.HQ_NPC_ID` — the "Headquarters" siege flag NPC.
const HQ_NPC_ID: i32 = 35062;

/// Java `HeadquarterCreate.instant` + `BuildCampSkillCondition`: the leader of an
/// attacker clan plants an HQ flag in the siege zone, becoming the clan's respawn
/// point until a defender destroys it. Returns whether a flag was placed.
pub(crate) fn place_siege_flag(world: &mut World, player_oid: i32, advanced: bool) -> bool {
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
    // registered as an attacker.
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
    // …and its **last** gate, which is the one with its own message: the camp
    // may only go up inside the battlefield's marked headquarters areas
    // (`isInsideZone(ZoneId.HQ)`, `castle_hq.xml` — 19 patches across the
    // castles). Without it a base camp could be planted in the courtyard.
    if world.data.zone_data.hq_castle_at(x, y, z) != Some(castle_id) {
        send_sm_bare_to_player(world, player_oid, sm_ids::YOU_CAN_T_BUILD_HEADQUARTERS_HERE);
        return false;
    }
    // Plant it at z+50 (Java `spawnMe(x, y, z + 50)`) and register it.
    let Some(oid) = crate::model::npc::spawn_npc_at(world, HQ_NPC_ID, x, y, z + 50, heading) else {
        return false;
    };
    crate::game_loop::death::introduce_npc(world, oid);
    // `new SiegeFlag(player, template, isAdvanced)` — skill 326's flag is the
    // same NPC (35062) but takes half damage. See `AdvancedHeadquarter` and
    // docs/CUSTOM_DIST_DEVIATIONS.md for why this halves rather than
    // reproducing Java's arithmetic.
    if advanced {
        world.objects.add_components(&oid, AdvancedHeadquarter);
    }
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
    // No siege state anywhere (the overwhelming majority of uptime) answers
    // every guard with one map probe, and the `Defender` type test reads the
    // fact memoized on the `Npc` core instead of the template.
    if world.sieges.is_empty() {
        return None;
    }
    let is_guard = world
        .objects
        .get_component::<crate::model::npc::Npc>(&guard_oid)
        .is_some_and(|n| n.is_defender(world));
    if !is_guard {
        return None;
    }
    let pos = world.objects.get_component::<Position>(&guard_oid)?;
    active_siege_castle_at(world, pos.x, pos.y, pos.z)
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
