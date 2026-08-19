//! Faction (clan) help calls: dragging nearby clan-mates into a fight, on
//! engagement and on a clan-mate death, plus the script-event dispatch.

use super::set_running;
use crate::game_loop::combat::ATTACK_TIMEOUT_TICKS;
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::hp_pair;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::pvp;
use crate::model::components::Casting;
use crate::model::npc::AggroList;
use crate::model::npc::NpcAi;
use crate::model::npc::NpcIntention;
use crate::world::World;
/// Java `AttackableAI.thinkAttack`'s faction block: an engaged NPC drags its
/// nearby clan-mates into the fight.
///
/// The gate that is easy to drop: **only if the target actually attacked *this*
/// NPC.** Java checks `getAttackByList`; the port's proxy is a non-zero `damage`
/// entry in the aggro list. Without it, walking up to one mob of a faction and
/// hitting *nothing* would still pull the whole camp. The rest of the scan lives
/// in [`faction_recruits`].
///
/// This runs from the think tick, so it never fires for a mob that dies before
/// its first think — [`faction_call_on_kill`] is the site that covers that.
///
/// Java routes the recruit through `EVT_AGGRESSION` (whose `Summon`-aware leg
/// never applies — a faction recruit is always an `Attackable`) and fires the
/// `OnAttackableFactionCall` script event; the port seeds hate directly and
/// dispatches the event's two listeners via [`on_faction_call_script`].
pub(super) fn faction_call(world: &mut World, npc_oid: i32, target_oid: i32) {
    let Some((npc_id, help_range, collision)) = npc_id_of(world, npc_oid).and_then(|id| {
        world
            .data
            .npc_data
            .get(id)
            .map(|t| (id, t.clan_help_range, t.collision_radius))
    }) else {
        return;
    };
    if help_range <= 0
        || world
            .data
            .npc_data
            .get(npc_id)
            .is_none_or(|t| t.clans.is_empty())
    {
        return;
    }

    // Gate 1: this NPC must actually have been attacked by the target.
    let was_attacked = world
        .objects
        .get_component::<AggroList>(&npc_oid)
        .and_then(|a| a.0.get(&target_oid))
        .is_some_and(|info| info.damage > 0.0);
    if !was_attacked {
        return;
    }

    let hate = world
        .objects
        .get_component::<AggroList>(&npc_oid)
        .and_then(|a| a.0.get(&target_oid))
        .map(|i| i.hate)
        .unwrap_or(1.0);
    let target_is_player = world
        .objects
        .has_component::<crate::model::Player>(&target_oid);
    let Some(target_pos) = maybe_position(world, target_oid) else {
        return;
    };

    // `thinkAttack` widens the scan by the caller's collision radius and honours
    // `ignoreClanNpcIds`; `doDie` (see [`faction_call_on_kill`]) does neither.
    let recruits = faction_recruits(
        world,
        npc_oid,
        help_range as f64 + collision,
        target_pos.z,
        true,
    );

    for other in recruits {
        // Java: a *playable* target gets `EVT_AGGRESSION … 1` (a nudge — the
        // recruit picks its own target), anything else inherits the caller's
        // full hate. Either way the recruit switches to the attack loop.
        let added = if target_is_player { 1.0 } else { hate };
        recruit_to_attack(world, other, target_oid, added);
        let attacker = pvp::acting_player(world, target_oid);
        on_faction_call_script(world, other, npc_oid, attacker);
    }
}

/// Java `Creature.doDie`'s "Clan help range aggro on kill" block: the clan-mates
/// of a monster that just *died* aggro its killer.
///
/// This is the second of Java's two faction-call sites and is not a nicety — it
/// is the only one that fires when a player one-shots a mob. [`faction_call`]
/// runs from the AI think tick, so a monster killed before its first think in
/// `AI_INTENTION_ATTACK` never gets to call anyone; without this block a
/// high-level character farming low-level `[G]` mobs (Cave Blade Spiders, say)
/// would never pull the pack, which reads as group aggro being broken.
///
/// Java's version deliberately differs from `thinkAttack`'s in three ways, all
/// mirrored here: the killer must be a **non-GM playable**, the scan range is
/// the bare `clanHelpRange` (no collision radius added), and
/// `ignoreClanNpcIds` is *not* consulted.
///
/// As with [`faction_call`], the recruit is seeded directly (Java's
/// `EVT_AGGRESSION`) and the script event's listeners run via
/// [`on_faction_call_script`].
pub(crate) fn faction_call_on_kill(world: &mut World, npc_oid: i32, killer_oid: i32) {
    // `killer.isPlayable()` — a player or a summon, not another NPC.
    let killer_is_playable = world
        .objects
        .has_component::<crate::model::Player>(&killer_oid)
        || world
            .objects
            .has_component::<crate::model::components::ServitorOf>(&killer_oid);
    if !killer_is_playable {
        return;
    }
    // `!killer.getActingPlayer().isGM()` — a GM cleaning up a spawn is ignored.
    let actor = pvp::acting_player(world, killer_oid);
    if world
        .objects
        .get_component::<crate::model::Player>(&actor)
        .is_some_and(|p| p.is_gm(&world.data))
    {
        return;
    }

    let Some(help_range) = npc_id_of(world, npc_oid)
        .and_then(|id| world.data.npc_data.get(id))
        .filter(|t| !t.clans.is_empty())
        .map(|t| t.clan_help_range)
    else {
        return;
    };
    if help_range <= 0 {
        return;
    }

    let Some(killer_pos) = maybe_position(world, killer_oid) else {
        return;
    };

    // Java: `notifyEvent(EVT_AGGRESSION, killer, 1)` — hate on the *killer*
    // object (a summon aggroes the pack on itself, exactly as in Java).
    for other in faction_recruits(world, npc_oid, help_range as f64, killer_pos.z, false) {
        recruit_to_attack(world, other, killer_oid, 1.0);
        let attacker = pvp::acting_player(world, killer_oid);
        on_faction_call_script(world, other, npc_oid, attacker);
    }
}

/// The recruit scan shared by Java's two faction-call sites.
///
/// Returns the clan-mates of `caller_oid` that would answer a call about a
/// target at `target_z`. Three gates are easy to drop and each one matters:
/// alive-and-in-range, **only idle/active clan-mates answer** (one already
/// attacking is left alone, so a fight doesn't continually re-target everyone
/// in it), and same-faction. `honor_ignore_list` covers `ignoreClanNpcIds` —
/// 82 templates on this dist refuse calls from specific faction-mates — which
/// only `thinkAttack` consults.
fn faction_recruits(
    world: &World,
    caller_oid: i32,
    range: f64,
    target_z: i32,
    honor_ignore_list: bool,
) -> Vec<i32> {
    let (Some(caller_id), Some(pos), Some(region)) = (
        npc_id_of(world, caller_oid),
        maybe_position(world, caller_oid),
        region_cell_of(world, caller_oid),
    ) else {
        return Vec::new();
    };

    // Candidate clan-mates: NPCs in this and the neighbouring regions.
    let nearby: Vec<i32> = world
        .npcs_visible_from(region)
        .into_iter()
        .filter(|&other| other != caller_oid)
        .collect();

    let mut recruits: Vec<i32> = Vec::new();
    for other in nearby {
        let Some(opos) = maybe_position(world, other) else {
            continue;
        };
        if is_dead(world, other) {
            continue;
        }
        // 3D range around the *caller* (`forEachVisibleObjectInRange`), plus
        // Java's explicit ±600 z band against the *target* — a helper on
        // another tower level never answers a call about a target it could
        // only reach by crossing floors.
        let dist_sq = ((opos.x - pos.x) as f64).powi(2)
            + ((opos.y - pos.y) as f64).powi(2)
            + ((opos.z - pos.z) as f64).powi(2);
        if dist_sq > range * range || (opos.z - target_z).abs() > 600 {
            continue;
        }
        // Only the uncommitted answer.
        if world
            .objects
            .get_component::<NpcAi>(&other)
            .is_none_or(|ai| ai.intention == NpcIntention::Attack)
        {
            continue;
        }
        // Same faction, and not on the recruit's ignore list.
        let Some(other_id) = npc_id_of(world, other) else {
            continue;
        };
        let (Some(mine), Some(theirs)) = (
            world.data.npc_data.get(caller_id),
            world.data.npc_data.get(other_id),
        ) else {
            continue;
        };
        if !mine.shares_clan_with(theirs)
            || (honor_ignore_list && theirs.ignore_clan_npc_ids.contains(&caller_id))
        {
            continue;
        }
        recruits.push(other);
    }
    recruits
}

/// The port's `OnAttackableFactionCall`. Java fires the script event at each
/// recruit from exactly its two faction-call sites (`AttackableAI.thinkAttack`
/// and `Creature.doDie`); on this dist only two scripts listen — Queen Ant
/// (`addFactionCallId(NURSE)`) and Orfen (`registerMobs`) — so the dispatch is
/// a direct match on the recruit's npc id rather than a listener registry.
/// Every listener starts by bailing while the recruit is mid-cast.
fn on_faction_call_script(world: &mut World, recruit_oid: i32, caller_oid: i32, attacker_oid: i32) {
    /// Queen Ant's healer minion; heals the hurt caller with Recovery (4020,1).
    const NURSE: i32 = 29003;
    /// Orfen's melee minion; 1-in-20 to open with Blow (4067,4) on the attacker.
    const RAIKEL_LEOS: i32 = 29016;
    /// Orfen Heal (4516,1) at a half-dead caller: 9-in-10 for Orfen herself,
    /// 1-in-10 for anyone else (never for a fellow Riba Iren).
    const ORFEN_HEAL: (i32, i32) = (4516, 1);
    const QA_HEAL: (i32, i32) = (4020, 1);
    const BLOW: (i32, i32) = (4067, 4);

    let recruit_id = npc_id_of(world, recruit_oid);
    let riba = crate::game_loop::orfen::RIBA_IREN;
    let Some(recruit_id) = recruit_id else { return };
    if !(recruit_id == NURSE || recruit_id == RAIKEL_LEOS || recruit_id == riba) {
        return;
    }
    if world.objects.has_component::<Casting>(&recruit_oid) {
        return;
    }
    let caller_hp = hp_pair(world, caller_oid);
    let cast = |world: &mut World, target: i32, (id, lvl): (i32, i32)| {
        crate::game_loop::npc::cast::cast_skill(world, recruit_oid, target, id, lvl);
    };
    match recruit_id {
        NURSE => {
            // `caller.getCurrentHp() < caller.getMaxHp()` — any wound at all.
            if caller_hp.is_some_and(|(cur, max)| cur < max) {
                cast(world, caller_oid, QA_HEAL);
            }
        }
        RAIKEL_LEOS => {
            if world.roll(20) == 0 {
                cast(world, attacker_oid, BLOW);
            }
        }
        id if id == riba => {
            let caller_id = npc_id_of(world, caller_oid);
            let chance = if caller_id == Some(crate::game_loop::orfen::ORFEN) {
                9
            } else {
                1
            };
            if caller_id != Some(riba)
                && caller_hp.is_some_and(|(cur, max)| cur < max / 2.0)
                && world.roll(10) < chance
            {
                cast(world, caller_oid, ORFEN_HEAL);
            }
        }
        _ => {}
    }
}

/// Test hook.
#[cfg(test)]
pub(crate) fn on_faction_call_script_for_test(
    world: &mut World,
    recruit_oid: i32,
    caller_oid: i32,
    attacker_oid: i32,
) {
    on_faction_call_script(world, recruit_oid, caller_oid, attacker_oid);
}

/// A faction-mate answering a call: seed hate on the target and switch it into
/// the attack loop.
fn recruit_to_attack(world: &mut World, recruit_oid: i32, target_oid: i32, hate: f64) {
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&recruit_oid) {
        let entry = aggro.0.entry(target_oid).or_default();
        entry.hate += hate;
    }
    // `onEvtAggression`: run **before** switching to the attack intention.
    set_running(world, recruit_oid);
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&recruit_oid) {
        ai.intention = NpcIntention::Attack;
        ai.attack_timeout_tick = world.tick + ATTACK_TIMEOUT_TICKS;
    }
}
