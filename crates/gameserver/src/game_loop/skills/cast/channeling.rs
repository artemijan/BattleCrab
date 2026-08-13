//! Ground-channeling: the per-tick re-affect, channeled-skill application
//! and `stopChanneling`.

use super::*;

/// One `SkillChannelizer.run()` tick: MP upkeep (starvation → SM 140 + abort),
/// re-resolve the target and **re-sweep the affect scope** (a mob that walked
/// into the volcano mid-channel burns; one that left stops), then apply the
/// CHANNELING effect scope per target behind Java's `effectRange` + LOS gate.
///
/// The `channelingSkillId > 0` branch applies a *named* skill to each target
/// instead of channeling effects, at a level equal to how many casters are
/// channeling it — see [`apply_channeled_skill`]. Battle Stance 426 (→ Battle
/// Force 5104) and Spell Stance 427 (→ 5105) are the reachable carriers; the
/// 3600-range Capture states and 14559 Soul Drain are off-chronicle.
pub(crate) fn handle_channeling_tick(world: &mut World, player_object_id: i32, cast_seq: u64) {
    use server_packets::sm_ids;

    // Stale guard, like every scheduled cast phase: the tick belongs to one
    // specific cast generation.
    let Some((cast, skill)) = live_cast_skill(world, player_object_id, cast_seq) else {
        return;
    };
    let client_id = client_for_player(world, player_object_id);

    // MP upkeep. Java: not enough → SM 140 + `abortCast()`, no reschedule.
    if skill.mp_per_channeling > 0 {
        let Some(vitals) = world.objects.get_component_mut::<Vitals>(&player_object_id) else {
            return;
        };
        if vitals.cur_mp < skill.mp_per_channeling as f64 {
            send_sm_bare_to_player(
                world,
                player_object_id,
                sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP,
            );
            abort_cast(world, player_object_id);
            return;
        }
        vitals.cur_mp -= skill.mp_per_channeling as f64;
        let mp = vitals.cur_mp as i32;
        if let Some(cid) = client_id {
            send_to_client(
                world,
                cid,
                server_packets::status_update(
                    player_object_id,
                    &[(server_packets::status_update_type::CUR_MP, mp)],
                ),
            );
        }
        crate::game_loop::party::notify_party_vitals(world, player_object_id);
    }

    // Re-schedule first (Java's task is fixed-rate): an empty tick — target
    // gone, nobody in the area — keeps ticking; only the MP abort above and a
    // finished/aborted cast (the stale guard) end the series.
    world.scheduler.schedule(
        world.tick + ms_to_ticks(skill.channeling_tick_ms),
        ScheduledTask::ChannelingTick {
            player_object_id,
            cast_seq,
        },
    );

    // Re-resolve the target quietly (`skill.getTarget(_channelizer, false,
    // false, false)`) and fan back out over the scope.
    let target_oid = {
        let Some(player) = world.objects.get_component::<Player>(&player_object_id) else {
            return;
        };
        let Some(pos) = maybe_position(world, player_object_id) else {
            return;
        };
        match resolve_cast_target(
            world,
            player,
            &pos,
            Some(cast.target_object_id),
            &skill,
            false,
            false,
        ) {
            Ok(oid) => oid,
            Err(_) => return, // quiet: skip this tick, keep channeling
        }
    };
    let affected = crate::game_loop::skills::affect::targets_affected(
        world,
        player_object_id,
        target_oid,
        &skill,
    );
    // A `channelingSkillId` skill carries **no** `<channelingEffects>` — it
    // applies a named skill instead — so the emptiness check must not gate it.
    // Battle Stance 426 / Spell Stance 427 are exactly this shape, and bailing
    // here is what left them channeling MP upkeep while applying nothing.
    let channels_a_skill = skill.channeling_skill_id > 0;
    if affected.is_empty() || (skill.channeling_effects.is_empty() && !channels_a_skill) {
        return;
    }
    // Java registers the channelizer inside `forEachTargetAffected`, i.e. for
    // every affected target — *before* the per-target range/LOS filter below,
    // which only gates application.
    if channels_a_skill {
        for &target in &affected {
            if target == player_object_id {
                continue;
            }
            world
                .channelized
                .entry(target)
                .or_default()
                .entry(skill.channeling_skill_id)
                .or_default()
                .insert(player_object_id);
        }
    }
    let Some(caster_pos) = maybe_position(world, player_object_id) else {
        return;
    };
    let scoped = Skill {
        self_continuous: false,
        effects: skill.channeling_effects.clone(),
        ..skill.clone()
    };
    for target in affected {
        if target == player_object_id {
            continue; // the ground sentinel, never a victim of its own cast
        }
        if target_state(world, target).is_none() {
            continue;
        }
        // Java's per-target gates: `checkIfInRange(effectRange, …, true)` +
        // `canSeeTarget(channelizer, creature)`.
        let Some(pos) = maybe_position(world, target) else {
            continue;
        };
        if skill.effect_range > 0
            && crate::geo::distance::dist3d_xyz(
                pos.x,
                pos.y,
                pos.z,
                caster_pos.x,
                caster_pos.y,
                caster_pos.z,
            ) > skill.effect_range as f64
        {
            continue;
        }
        if !world.geo.can_see_target(
            caster_pos.x,
            caster_pos.y,
            caster_pos.z,
            pos.x,
            pos.y,
            pos.z,
        ) {
            continue;
        }
        if channels_a_skill {
            apply_channeled_skill(world, player_object_id, target, &skill);
            continue;
        }
        // Just `applyChannelingEffects` — Java's simple path runs **no**
        // per-tick `callSkill` consequences (no flat `-effectPoint` hate, no
        // PvP flag; those fire once, at cast finish, through the normal
        // pipeline). NPC aggro still happens: the damage handler itself wakes
        // whatever it hurts.
        apply_skill_effects(world, player_object_id, target, &scoped);
    }

    // Java uncharges + recharges shots each tick; the port's shot model is
    // recharge-only, so mirror the cast-start call for magic skills.
    if skill.magic_type == 1 {
        crate::game_loop::items::recharge_shots(world, player_object_id, false, true, false);
    }
}

pub(crate) fn buff_level(world: &World, oid: i32, skill_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<crate::model::components::Buffs>(&oid)
        .and_then(|b| {
            b.0.iter()
                .find(|a| a.skill_id == skill_id)
                .map(|a| a.skill_level)
        })
}
/// Java's `channelingSkillId > 0` branch of `SkillChannelizer.run`.
///
/// The level is **how many distinct channelers** are holding this skill on the
/// target, capped at the channeled skill's max level: one Warcryer on Battle
/// Stance gives Battle Force 5104 level 1, two give level 2. That is the entire
/// mechanic — the registry size *is* the level.
///
/// Re-applied only when the target has no such buff or a weaker one, so a
/// steady two-channeler stack refreshes at level 2 rather than flickering.
fn apply_channeled_skill(world: &mut World, channelizer: i32, target: i32, skill: &Skill) {
    let channeled_id = skill.channeling_skill_id;
    let count = world
        .channelized
        .get(&target)
        .and_then(|m| m.get(&channeled_id))
        .map_or(0, |s| s.len()) as i32;
    let level = count.min(world.data.skill_data.max_level(channeled_id));
    if level <= 0 {
        return;
    }
    // `getBuffInfoBySkillId(...)` — skip while an equal-or-stronger stack is up.
    let current = buff_level(world, target, channeled_id);
    if current.is_some_and(|lvl| lvl >= level) {
        return;
    }
    let Some(channeled) = skill_by_id(world, channeled_id, level) else {
        // Java logs and aborts the cast on a missing channeled skill.
        tracing::warn!(
            "Channeling: skill {} names a non-existent channeling skill {channeled_id}.",
            skill.id
        );
        abort_cast(world, channelizer);
        return;
    };
    // Java `updatePvPStatus(creature)` before the apply, when both are playable.
    if world.objects.has_component::<Player>(&target) {
        crate::game_loop::pvp::update_pvp_status_target(world, channelizer, target);
    }
    apply_skill_effects(world, channelizer, target, &channeled);
}

/// Java `SkillChannelizer.stopChanneling` → `removeChannelizer`: drop this
/// caster from every target's registry, so the stack it was contributing to
/// shrinks by one (and the channeled buff re-lands a level lower on the next
/// surviving channeler's tick, or simply expires).
pub(crate) fn stop_channelizing(world: &mut World, channelizer: i32) {
    world.channelized.retain(|_, per_skill| {
        per_skill.retain(|_, set| {
            set.remove(&channelizer);
            !set.is_empty()
        });
        !per_skill.is_empty()
    });
}
