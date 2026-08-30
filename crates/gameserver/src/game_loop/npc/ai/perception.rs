//! Perception and target selection: `checkTarget`, the reconsider paths,
//! aggro-range candidate scans, `isAggressiveTowards` and guard PK scans.

use super::movement_disabled;
use crate::game_loop::combat;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::is_raid_npc;
use crate::game_loop::helpers::maybe_position;
use crate::game_loop::helpers::npc_template;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::target;
use crate::model::components::Position;
use crate::model::components::Vitals;
use crate::model::npc::AggroList;
use crate::world::World;
/// `AttackableAI.checkTarget` — is this still something worth walking to?
///
/// Alive, and (for an **immobilised** mob only) inside physical attack reach
/// with line of sight, and auto-attackable. The `isMovementDisabled()` gate is
/// load-bearing: a mob that *can* move is allowed to chase a target it cannot
/// currently see, which is what lets it walk around a corner after you.
pub(super) fn check_target(world: &World, npc_oid: i32, target_oid: i32) -> bool {
    if is_dead(world, target_oid) {
        return false;
    }
    if movement_disabled(world, npc_oid) {
        let (Some(me), Some(target)) = (
            combat::combatant(world, npc_oid),
            combat::combatant(world, target_oid),
        ) else {
            return false;
        };
        let reach = me.atk_range as f64 + me.collision_radius + target.collision_radius;
        let (dx, dy) = ((target.x - me.x) as f64, (target.y - me.y) as f64);
        if dx * dx + dy * dy > reach * reach {
            return false;
        }
        if !world
            .geo
            .can_see_target(me.x, me.y, me.z, target.x, target.y, target.z)
        {
            return false;
        }
    }
    target::is_auto_attackable(world, npc_oid, target_oid)
}

/// `AttackableAI.targetReconsider(false)` — the most hated attacker that still
/// passes [`check_target`], falling back to the first valid creature inside the
/// aggro range when the mob is aggressive and its whole list has gone stale.
pub(super) fn target_reconsider(world: &mut World, npc_oid: i32) -> Option<i32> {
    let candidates: Vec<(i32, f64)> = world
        .objects
        .get_component::<AggroList>(&npc_oid)
        .map(|a| a.0.iter().map(|(&oid, i)| (oid, i.hate)).collect())
        .unwrap_or_default();
    let best = candidates
        .iter()
        .filter(|&&(oid, _)| check_target(world, npc_oid, oid))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|&(oid, _)| oid);
    if best.is_some() {
        return best;
    }
    aggro_range_candidates(world, npc_oid)
        .into_iter()
        .find(|&oid| check_target(world, npc_oid, oid))
}

/// `AttackableAI.targetReconsider(true)` — any valid attacker at random, plus
/// (for an aggressive mob) anyone standing inside the aggro range.
pub(super) fn target_reconsider_random(world: &mut World, npc_oid: i32) -> Option<i32> {
    let mut valid: Vec<i32> = world
        .objects
        .get_component::<AggroList>(&npc_oid)
        .map(|a| a.0.keys().copied().collect())
        .unwrap_or_default();
    valid.extend(aggro_range_candidates(world, npc_oid));
    valid.retain(|&oid| check_target(world, npc_oid, oid));
    if valid.is_empty() {
        return None;
    }
    Some(valid[world.roll(valid.len() as i32) as usize])
}

/// The "if npc is aggressive, add characters within aggro range too" leg of
/// both `targetReconsider` arms. Empty for a passive mob.
pub(super) fn aggro_range_candidates(world: &mut World, npc_oid: i32) -> Vec<i32> {
    let Some(template) = npc_template(world, npc_oid) else {
        return Vec::new();
    };
    if !template.is_aggressive {
        return Vec::new();
    }
    let range = template.aggro_range as f64;
    let (Some(pos), Some(region)) = (
        maybe_position(world, npc_oid),
        region_cell_of(world, npc_oid),
    ) else {
        return Vec::new();
    };
    // Index-derived like the aggro scan, but deliberately without the LOS and
    // liveness filters the scan applies — this candidate list feeds
    // `target_reconsider`, which does its own checks.
    let range_sq = range * range;
    let mut out = Vec::new();
    for pid in world.players_visible_from(region) {
        let Some(ppos) = world.objects.get_component::<Position>(&pid) else {
            continue;
        };
        let (dx, dy, dz) = (
            (ppos.x - pos.x) as f64,
            (ppos.y - pos.y) as f64,
            (ppos.z - pos.z) as f64,
        );
        if dx * dx + dy * dy + dz * dz <= range_sq {
            out.push(pid);
        }
    }
    out
}

/// `Creature.setRunning()` for an NPC: flip the move type and tell everyone
/// watching (`ChangeMoveType`). Idempotent — Java guards every call site with
/// `if (!me.isRunning())` and so does this.
///
/// Every path that puts a monster into the attack loop has to come through
/// here. [`think_active`] already did it inline when it promoted its own
/// target, but the two paths that seed hate from *outside* the think — the
/// `AttackableAI.isAggressiveTowards`'s playable-state gates — whether this NPC
/// notices `target_oid` at all.
///
/// Two effect flags hide a player from an aggro scan, and Java checks them on
/// adjacent lines of the same method:
///
/// - **`SILENT_MOVE`** (Silent Move 221, Stealth 411, Dance of Shadows 366):
///   `!me.isRaid() && !me.canSeeThroughSilentMove() && target.isSilentMovingAffected()`.
///   Raid bosses see through stealth; `canSeeThroughSilentMove` is always false
///   on this dist (`setSeeThroughSilentMove` has no callers in the whole Java
///   tree), so only the raid exemption is ported.
/// - **`FAKE_DEATH`** via `isAlikeDead()`, which `Player` overrides to include
///   it — the very first check in the method.
///
/// Java's third gate here, `player.isRecentFakeDeath()` (a grace window after
/// standing up), is inert on this dist: `PlayerFakeDeathUpProtection = 0`.
pub(crate) fn notices_target(world: &World, npc_oid: i32, target_oid: i32) -> bool {
    use crate::game_loop::abnormal;
    use crate::model::skill::effect_flag;
    // `//invis`: an invisible GM is never noticed — Java's `AttackableAI`
    // drops invisible targets and `OnCreatureSee` never fires for them
    // (no raid exemption, unlike SILENT_MOVE below).
    if world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&target_oid)
        .is_some_and(|f| f.hidden)
    {
        return false;
    }
    // `Attackable.getHating`'s `act.isSpawnProtected()` arm: a character
    // inside their entry grace period is dropped from the aggro list, so a
    // monster neither takes an interest nor keeps one.
    if crate::game_loop::spawn_protection::is_protected(world, target_oid) {
        return false;
    }
    let flags = abnormal::flags_of(world, target_oid);
    // `isAlikeDead()` — a fake-dead player is, for aggro purposes, a corpse.
    if flags & effect_flag::FAKE_DEATH != 0 {
        return false;
    }
    if flags & effect_flag::SILENT_MOVE != 0 {
        let is_raid = is_raid_npc(world, npc_oid);
        if !is_raid {
            return false;
        }
    }
    true
}

/// The shared body of the AI proximity scans: live players within `range`
/// (3D — `World.forEachVisibleObjectInRange` uses `calculateDistance3D`, so a
/// player a floor above is outside a ground mob's aggro sphere) of (nx,ny,nz)
/// with geodata line of sight, drawn from the `player_regions` index (≤9
/// cells) instead of a full player-table sweep. Like Java's knownlist it
/// includes unattended shops — they are `Player` objects in the region index.
pub(super) fn players_in_range_los(
    world: &World,
    region: (i32, i32),
    nx: i32,
    ny: i32,
    nz: i32,
    range: f64,
) -> Vec<i32> {
    let range_sq = range * range;
    let mut out = Vec::new();
    for pid in world.players_visible_from(region) {
        let (Some(pos), Some(v)) = (
            world.objects.get_component::<Position>(&pid),
            world.objects.get_component::<Vitals>(&pid),
        ) else {
            continue;
        };
        if !v.dead
            && ((pos.x - nx) as f64).powi(2)
                + ((pos.y - ny) as f64).powi(2)
                + ((pos.z - nz) as f64).powi(2)
                <= range_sq
            && world.geo.can_see_target(nx, ny, nz, pos.x, pos.y, pos.z)
        {
            out.push(pid);
        }
    }
    out
}

/// Java `AttackableAI.isAggressiveTowards`, `me instanceof Guard` branch.
/// Guards seed hate on nearby **PKs** (reputation < 0) so the ordinary attack
/// loop takes over from there.
///
/// The 500 is Java's literal (`GUARD_ATTACK_RANGE` in spirit — the source has
/// the bare constant with a "Make sure how guards behave towards players"
/// note beside it), deliberately not the template `aggroRange`.
const GUARD_AGGRO_RANGE: f64 = 500.0;

pub(super) fn set_hate_for(world: &mut World, npc_oid: i32, in_range: Vec<i32>) {
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        for player_oid in in_range {
            let entry = aggro.0.entry(player_oid).or_default();
            if entry.hate == 0.0 {
                entry.hate = 1.0;
            }
        }
    }
}
pub(super) fn guard_aggro_scan(world: &mut World, npc_oid: i32, region: (i32, i32)) {
    let (nx, ny, nz) = {
        let Some(pos) = maybe_position(world, npc_oid) else {
            return;
        };
        (pos.x, pos.y, pos.z)
    };
    let mut pks = players_in_range_los(world, region, nx, ny, nz, GUARD_AGGRO_RANGE);
    // `getReputation() < 0` is the whole test: a clean player walks past a
    // guard untouched no matter how close.
    pks.retain(|&pid| {
        world
            .objects
            .get_component::<crate::model::Player>(&pid)
            .is_some_and(|p| p.reputation < 0)
    });
    // Guards run the same `isAggressiveTowards` (Java `Guard extends
    // Attackable`), so stealth and fake death hide a PK from them too.
    pks.retain(|&pid| notices_target(world, npc_oid, pid));
    set_hate_for(world, npc_oid, pks);
}
