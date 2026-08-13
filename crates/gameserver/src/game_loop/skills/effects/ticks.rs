use super::*;
use crate::game_loop::abnormal::has_buff;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::send_sm_bare_to_player;
use crate::game_loop::helpers::send_to_player;
use crate::game_loop::helpers::skill_by_id;
use crate::game_loop::helpers::spend_mp;

/// `DamOverTime.onActionTime` — one poison/bleed tick. Deals
/// `power * getTicksMultiplier()` from `caster` to `target` for each of the
/// skill's DoT effects, then reschedules itself. The chain stops (Java's
/// fixed-rate task cancelled by `BuffFinishTask`) when the buff is no longer
/// present — its `BuffExpire` removes it at `abnormalTime` — or the target is
/// dead. `can_kill == false` clamps each tick to leave the target at 1 HP
/// (Java: "Fix for players dying by DOTs"). A non-toggle DoT never
/// self-cancels on the tick's own return value (`BuffInfo.onTick` only cancels
/// toggles), so the reschedule is unconditional while the buff lives.
pub(crate) fn handle_dam_over_time_tick(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill_id: i32,
    skill_level: i32,
) {
    // Buff gone (expired / removed / dispelled) → end the tick chain.
    let buff_present = has_buff(world, target_oid, skill_id);
    if !buff_present {
        return;
    }
    // Dead target → stop (Java `onActionTime`: `isDead()` bails).
    if is_dead(world, target_oid) {
        return;
    }
    let Some(skill) = skill_by_id(world, skill_id, skill_level) else {
        return;
    };
    // Effector name for the damage message (`Player.sendDamageMessage`); empty
    // for an NPC effector (no client to message — the base no-op).
    let caster_name = player_name_or_empty(world, caster_oid);

    let mut interval = 0;
    // Set when a tick returns Java's `false` for a *toggle*, which cancels it
    // (`BuffInfo.onTick` only honours the return value for toggles).
    let mut deactivate_toggle = false;
    let is_toggle = skill.operate_type == crate::model::skill::OperateType::Toggle;

    for effect in &skill.effects {
        match effect {
            // `HealOverTime.onActionTime`. `power` is negative for the upkeep
            // toggles, so this both heals and drains.
            SkillEffect::HealOverTime { power, ticks } if *ticks > 0 => {
                interval = dot_interval_ticks(*ticks);
                let Some(v) = world.objects.get_component::<Vitals>(&target_oid).copied() else { continue };
                let max_hp = v.max_hp as f64;
                // Java's early bails: at full HP a healing tick is skipped, and
                // a draining one is skipped when it would take the target to 0.
                // (With a negative power the second test is `hp + |power| <= 0`,
                // which never fires — ported as written rather than "fixed".)
                if *power > 0.0 {
                    if v.cur_hp >= max_hp {
                        deactivate_toggle |= is_toggle;
                        continue;
                    }
                } else if v.cur_hp - *power <= 0.0 {
                    deactivate_toggle |= is_toggle;
                    continue;
                }
                let mut hp = v.cur_hp + dot_tick_damage(*power, *ticks);
                // Cap at max when healing, floor at 1 when draining — a HoT
                // upkeep never kills its owner.
                hp = if *power > 0.0 { hp.min(max_hp) } else { hp.max(1.0) };
                if let Some(vit) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                    vit.cur_hp = hp;
                }
                broadcast_vitals(world, target_oid);
            }
            // `ManaDamOverTime.onActionTime` — MP upkeep. Shares this arm with
            // `MpConsumePerLevel` (the fighter-toggle upkeep effect): Java's
            // formula for the latter is `power * getTicksMultiplier()` whenever
            // the skill has no `abnormalTime`, which is every instance in this
            // datapack (all 19 are toggles/`AU` skills), so the two are
            // computed identically here. Split them out if a skill ever pairs
            // `MpConsumePerLevel` with an `abnormalTime` (the level-scaled
            // `((level-1)/7.5) * base * abnormalTime` branch) — no skill in
            // this datapack does, so that branch is unreachable rather than
            // pending.
            // `Fear.onActionTime` — keep running. Java passes `null` for the
            // effector here (not the caster it had at `onStart`), so every
            // repeat steers by the victim's current heading: they keep going
            // the way the first shove threw them instead of being re-aimed
            // away from a caster who may be dead, gone or long out of range.
            SkillEffect::Fear { ticks } if *ticks > 0 => {
                interval = dot_interval_ticks(*ticks);
                fear_action(world, None, target_oid);
            }
            // `ChameleonRest.onActionTime` — Relax's stand-up and out-of-MP
            // stops, **without** its HP-full stop: you are resting to hide,
            // not to heal, so a full HP bar does not retire it.
            SkillEffect::ChameleonRest { power, ticks } if *ticks > 0 => {
                interval = dot_interval_ticks(*ticks);
                if world.objects.has_component::<crate::model::Player>(&target_oid)
                    && !crate::game_loop::sit_stand::is_sitting(world, target_oid)
                {
                    deactivate_toggle |= is_toggle;
                    continue;
                }
                let Some(v) = world.objects.get_component::<Vitals>(&target_oid).copied() else {
                    continue;
                };
                let drain = dot_tick_damage(*power, *ticks);
                // Java compares before spending and bails on `>`, so a tick that
                // costs exactly the remaining MP still runs.
                if drain > v.cur_mp {
                    send_sm_bare_to_player(world, target_oid, server_packets::sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP);
                    deactivate_toggle = true;
                    continue;
                }
                spend_mp(world, target_oid, drain);
                broadcast_vitals(world, target_oid);
            }
            // `ManaHealOverTime.onActionTime` — the mirror of the drain arm
            // below. Java's two early-outs are asymmetric: a **positive** power
            // stops once MP is already full, a negative one stops when the tick
            // would take MP to zero or below, and the write floors at 1 rather
            // than 0 — a drain wearing this handler can never empty the pool.
            SkillEffect::ManaHealOverTime { power, ticks } if *ticks > 0 => {
                interval = dot_interval_ticks(*ticks);
                let Some(v) = world.objects.get_component::<Vitals>(&target_oid).copied() else {
                    continue;
                };
                if v.dead {
                    continue;
                }
                // `getMaxRecoverableMp()` — `MAX_RECOVERABLE_MP` over `maxMp`.
                // `LimitMp`'s two carriers are unreachable here (see
                // `restore_mp`), so the ceiling is plain `maxMp`.
                let ceiling = v.max_mp as f64;
                if *power > 0.0 {
                    if v.cur_mp >= ceiling {
                        continue;
                    }
                } else if v.cur_mp - *power <= 0.0 {
                    continue;
                }
                let delta = dot_tick_damage(*power, *ticks);
                let restored = if *power > 0.0 {
                    (v.cur_mp + delta).min(ceiling)
                } else {
                    (v.cur_mp + delta).max(1.0)
                };
                if let Some(vit) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                    vit.cur_mp = restored;
                }
                broadcast_vitals(world, target_oid);
            }
            // `Relax.onActionTime` — the MP upkeep above, plus the two extra
            // stop conditions the plain upkeep effects do not have.
            SkillEffect::Relax { power, ticks } if *ticks > 0 => {
                interval = dot_interval_ticks(*ticks);
                // "the holder stood up" — Java returns `false` outright, which
                // cancels the toggle. Standing is how a player turns Relax off.
                if world.objects.has_component::<crate::model::Player>(&target_oid)
                    && !crate::game_loop::sit_stand::is_sitting(world, target_oid)
                {
                    deactivate_toggle |= is_toggle;
                    continue;
                }
                let Some(v) = world.objects.get_component::<Vitals>(&target_oid).copied() else {
                    continue;
                };
                // Java's `(curHp + 1) > maxRecoverableHp`: the point of Relax is
                // to regenerate, so it retires itself once there is nothing left
                // to heal — with its own message, distinct from running dry.
                if v.cur_hp + 1.0 > v.max_hp as f64 && is_toggle {
                    send_sm_bare_to_player(world, target_oid, server_packets::sm_ids::THAT_SKILL_HAS_BEEN_DE_ACTIVATED_AS_HP_WAS_FULLY_RECOVERED);
                    deactivate_toggle = true;
                    continue;
                }
                let drain = dot_tick_damage(*power, *ticks);
                if drain > v.cur_mp && is_toggle {
                    send_sm_bare_to_player(world, target_oid, server_packets::sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP);
                    deactivate_toggle = true;
                    continue;
                }
                spend_mp(world, target_oid, drain);
                broadcast_vitals(world, target_oid);
            }
            SkillEffect::ManaDamOverTime { power, ticks }
            | SkillEffect::MpConsumePerLevel { power, ticks }
            // `FakeDeath.onActionTime` is the same `power * getTicksMultiplier()`
            // MP drain, with the same toggle self-deactivate on empty MP.
            | SkillEffect::FakeDeath { power, ticks }
                if *ticks > 0 =>
            {
                interval = dot_interval_ticks(*ticks);
                let Some(v) = world.objects.get_component::<Vitals>(&target_oid).copied() else { continue };
                let drain = dot_tick_damage(*power, *ticks);
                if drain > v.cur_mp && is_toggle {
                    // Out of MP: the toggle switches itself off.
                    send_sm_bare_to_player(world, target_oid, server_packets::sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP);
                    deactivate_toggle = true;
                    continue;
                }
                spend_mp(world, target_oid, drain);
                broadcast_vitals(world, target_oid);
            }
            _ => {}
        }

        let SkillEffect::DamOverTime {
            power,
            ticks,
            can_kill,
        } = effect
        else {
            continue;
        };
        if *ticks <= 0 {
            continue;
        }
        interval = dot_interval_ticks(*ticks);
        let mut damage = dot_tick_damage(*power, *ticks);
        // `!canKill`: a tick may never drop the target below 1 HP.
        if !*can_kill {
            let cur_hp = world
                .objects
                .get_component::<Vitals>(&target_oid)
                .map(|v| v.cur_hp)
                .unwrap_or(0.0);
            if cur_hp <= 1.0 {
                continue;
            }
            if damage >= cur_hp - 1.0 {
                damage = cur_hp - 1.0;
            }
        }
        if damage > 0.0 {
            // Java `effector.doAttack(damage, effected, skill, isDOT=true, …,
            // critical=false, …)`: no crit line; reuses the shared victim-side
            // path (CP soak / NPC hate / AI wake / death).
            apply_skill_damage(
                world,
                caster_oid,
                target_oid,
                SkillHit {
                    damage,
                    is_magic: skill.magic_type == 1,
                    caster_name: &caster_name,
                    is_dot: true,
                    skill_id: skill.id,
                    ..Default::default()
                },
            );
            // A `canKill` tick can kill outright — stop then.
            if is_dead(world, target_oid) {
                return;
            }
        }
    }
    if deactivate_toggle {
        // Java's `false` return cancels a toggle's effect outright; the tick
        // chain then ends with the buff.
        handle_buff_expire(world, target_oid, skill_id);
        return;
    }
    if interval > 0 {
        world.scheduler.schedule(
            world.tick + interval,
            ScheduledTask::DamOverTimeTick {
                caster: caster_oid,
                target: target_oid,
                skill_id,
                skill_level,
            },
        );
    }
}

/// Java `EffectList.stopEffects(Predicate)` — take off every buff the predicate
/// selects, and return how many were removed.
///
/// Eight call sites had open-coded the same three steps: read `Buffs`, collect
/// the matching `skill_id`s into a `Vec`, then loop [`handle_buff_expire`] over
/// it. Only the predicate ever differed, and the `Vec` is not an incidental
/// detail — it is what ends the immutable borrow of `Buffs` before the loop
/// starts mutating the world, so open-coding it invites someone to "simplify"
/// the collect away into a borrow error, or worse, to iterate a list that the
/// expiry is concurrently editing.
///
/// The predicate takes `&World` because several callers decide from the skill
/// table (`removed_on_damage`, `stay_after_death`, `operate_type`) rather than
/// from the buff row alone.
pub(crate) fn expire_buffs_where(
    world: &mut World,
    object_id: i32,
    matches: impl Fn(&World, &crate::model::skill::ActiveBuff) -> bool,
) -> usize {
    let skill_ids: Vec<i32> = buffs_snapshot(world, object_id, |buff| {
        matches(world, buff).then_some(buff.skill_id)
    });
    let count = skill_ids.len();
    for skill_id in skill_ids {
        handle_buff_expire(world, object_id, skill_id);
    }
    count
}

/// Java `Creature.stopAllEffects()` — drop every *timed* buff, keeping passives.
///
/// The passive filter is the whole point: passives carry grade penalties and
/// clan/residence pumps that Java never clears here, so a caller that forgets
/// it silently strips them.
pub(crate) fn expire_active_buffs(world: &mut World, object_id: i32) -> usize {
    expire_buffs_where(world, object_id, |_, buff| !buff.passive)
}

/// `BuffFinishTask`, fired when a buff's `abnormalTime` elapses
/// (`ScheduledTask::BuffExpire`). A buff already gone (re-cast/replaced) is a
/// no-op, matching the scheduler's dead-id contract.
/// Java `EffectList.remove` — take the buff off and run everything that hangs
/// off its removal, ending with `applyEffectScope(EffectScope.END, …)`.
///
/// The END scope is applied *here* rather than inside the removal body because
/// that body has several early exits (the NPC path returns before the player
/// broadcasts); hanging the end-effects off the wrapper means every removal
/// route fires them exactly once, which is what Java's single call site does.
pub(crate) fn handle_buff_expire(world: &mut World, player_object_id: i32, skill_id: i32) {
    // Read before the removal — the buff has to still be there to know whether
    // this call is the one that actually took it off.
    let was_active = has_buff(world, player_object_id, skill_id);
    let end_effects = world
        .data
        .skill_data
        .get(skill_id, 1)
        .map(|s| s.end_effects.clone())
        .unwrap_or_default();

    handle_buff_expire_inner(world, player_object_id, skill_id);

    // Anchor (1170) is the learnable carrier: its first stage holds the body
    // rigid and this fires skill 6091 for the paralysis its own description
    // promises. Applied after the removal, so a called skill that re-buffs the
    // same target cannot race it.
    if was_active && !end_effects.is_empty() {
        let called = Skill {
            self_continuous: false,
            effects: end_effects,
            ..skill_by_id(world, skill_id, 1).unwrap_or_default()
        };
        apply_skill_effects(world, player_object_id, player_object_id, &called);
    }
}

fn handle_buff_expire_inner(world: &mut World, player_object_id: i32, skill_id: i32) {
    // Forced/unconditional removal — also used by dispel/cure, which strip a
    // buff before its timer. The natural-timeout path gates on `expires_at_tick`
    // at the scheduler dispatch so a stale `BuffExpire` from a re-cast can't drop
    // the refreshed buff early.
    let still_active = has_buff(world, player_object_id, skill_id);
    if !still_active {
        return;
    }
    // `Grow.onExit` — put the normal collision cylinder back. Runs on every
    // removal path (timeout, dispel, death), which is what Java's `onExit`
    // does too: the swell must not outlive the buff that caused it.
    let expiring_level = buff_level(world, player_object_id, skill_id);
    if world
        .data
        .skill_data
        .get(skill_id, expiring_level)
        .is_some_and(|s| s.effects.iter().any(|e| matches!(e, SkillEffect::Grow)))
    {
        super::continuous::set_collision_grown(world, player_object_id, false);
    }

    // `ResurrectionSpecial.onExit` — the auto-resurrect. The buff does nothing
    // while it is up; what fires it is being *stripped*, which is what death
    // does.
    //
    // Java refuses in an olympiad match (`isInOlympiadMode()`), which matters:
    // an auto-resurrect inside a duel-to-the-death would decide the match. The
    // `instanceId` allow-list is the other half and stays unmodelled — no
    // carrier on this dist declares one, so the list is empty everywhere.
    if crate::game_loop::olympiad::in_match(world, player_object_id) {
        return;
    }
    if let Some(res) = world.data.skill_data.get(skill_id, 1).and_then(|s| {
        s.effects.iter().find_map(|e| match e {
            SkillEffect::ResurrectionSpecial {
                power,
                hp_percent,
                mp_percent,
                cp_percent,
            } => Some((*power, *hp_percent, *mp_percent, *cp_percent)),
            _ => None,
        })
    }) {
        let (power, hp, mp, cp) = res;
        // Java's effector is the caster; these are self-buffs, so the bearer
        // proposes their own revive.
        crate::game_loop::death::revive_request(
            world,
            player_object_id,
            player_object_id,
            power,
            hp,
            mp,
            cp,
            skill_id,
            0, // no affectRange bypass — this is a self-revive, not a mass one
        );
    }
    // `SkillEvasion.onExit` — `removeSkillEvasionTypeValue(magicType, amount)`.
    // Merged onto a per-bucket map rather than a `Stat`, so it needs its own
    // unmerge; without it Ultimate Evasion's 40 % dodge would be permanent.
    if let Some(evasions) = world.data.skill_data.get(skill_id, 1).map(|s| {
        s.effects
            .iter()
            .filter_map(|e| match e {
                SkillEffect::SkillEvasion { magic_type, amount } => Some((*magic_type, *amount)),
                _ => None,
            })
            .collect::<Vec<_>>()
    }) && !evasions.is_empty()
        && let Some(mods) = world
            .objects
            .get_component_mut::<crate::model::components::StatModifiers>(&player_object_id)
    {
        for (magic_type, amount) in evasions {
            let entry = mods.skill_evasion.entry(magic_type).or_insert(0.0);
            *entry = (*entry - amount).max(0.0);
        }
    }
    // The bot-report punishments' `onExit`: each undoes what its `onStart`
    // began. Java's handlers stop the punishment / clear the PvP flag; the buff
    // itself is the timer, so this is the only thing that ends them.
    if let Some(effects) = world
        .data
        .skill_data
        .get(skill_id, 1)
        .map(|s| s.effects.clone())
    {
        for effect in &effects {
            match effect {
                SkillEffect::BlockChat => stop_bot_report_punishment(
                    world,
                    player_object_id,
                    crate::model::punishment::PunishmentType::ChatBan,
                ),
                SkillEffect::BlockParty => stop_bot_report_punishment(
                    world,
                    player_object_id,
                    crate::model::punishment::PunishmentType::PartyBan,
                ),
                SkillEffect::BlockAction { blocked_actions } => {
                    if blocked_actions
                        .contains(&crate::game_loop::bot_report::PARTY_ACTION_BLOCK_ID)
                    {
                        stop_bot_report_punishment(
                            world,
                            player_object_id,
                            crate::model::punishment::PunishmentType::PartyBan,
                        );
                    }
                    if blocked_actions.contains(&crate::game_loop::bot_report::CHAT_BLOCK_ID) {
                        stop_bot_report_punishment(
                            world,
                            player_object_id,
                            crate::model::punishment::PunishmentType::ChatBan,
                        );
                    }
                }
                // `Flag.onExit` → `updatePvPFlag(0)`.
                SkillEffect::PvpFlag => {
                    crate::game_loop::pvp::update_pvp_flag(world, player_object_id, 0);
                }
                _ => {}
            }
        }
    }
    // `TargetMe.onExit` — `setLockedTarget(null)`. The lock is what stops the
    // victim clicking a different NPC ("Failed to change enmity"), so it must
    // go the moment the taunt does (G34 S4).
    if world
        .data
        .skill_data
        .get(skill_id, 1)
        .is_some_and(|s| s.effects.iter().any(|e| matches!(e, SkillEffect::TargetMe)))
    {
        world
            .objects
            .remove_component::<crate::model::components::LockedTarget>(&player_object_id);
    }
    // `DefenceTrait.onExit` — unmerge before the buff row goes, while the skill
    // is still resolvable. Covers the NPC branch below as well as the player
    // one, and every removal route (timeout, dispel, death) funnels here.
    if let Some(effects) = world
        .data
        .skill_data
        .get(skill_id, buff_level(world, player_object_id, skill_id))
        .map(|s| s.effects.clone())
    {
        for effect in &effects {
            match effect {
                SkillEffect::DefenceTrait { traits } => {
                    remove_defence_traits(world, player_object_id, traits)
                }
                SkillEffect::AttackTrait { traits } => {
                    remove_attack_traits(world, player_object_id, traits)
                }
                _ => {}
            }
        }
    }
    // `MagicMpCost.onExit` / `Reuse.onExit`.
    if let Some(skill) = skill_by_id(
        world,
        skill_id,
        buff_level(world, player_object_id, skill_id),
    ) {
        remove_skill_rates(world, player_object_id, &skill);
    }
    // Did the buff about to go carry a visual? If not, the set can't change and
    // no `ExUserInfoAbnormalVisualEffect` is due (Java's same rule).
    let had_visuals = world
        .objects
        .get_component::<Buffs>(&player_object_id)
        .is_some_and(|b| {
            b.0.iter()
                .any(|x| x.skill_id == skill_id && !x.abnormal_visuals.is_empty())
        });
    fn is_effect(world: &World, object_id: i32, skill_id: i32, effect: u32) -> bool {
        world
            .objects
            .get_component::<Buffs>(&object_id)
            .is_some_and(|b| {
                b.0.iter()
                    .any(|x| x.skill_id == skill_id && x.effect_flags & effect != 0)
            })
    }
    // NPC: drop the buff and recompute from the template (no icons/broadcast).
    if crate::game_loop::combat::is_npc_oid(player_object_id) {
        // `Fear.onExit`: `if (!effected.isPlayer()) notifyEvent(EVT_THINK)` —
        // a mob left mid-flight is still on `MOVE_TO`, whose think arm does
        // nothing, so without this it would keep walking out its last leg
        // before ever re-engaging. Reading the flag *before* the buff is
        // dropped is what makes this specific to fear rather than to any
        // expiring NPC buff.
        let was_afraid = is_effect(
            world,
            player_object_id,
            skill_id,
            crate::model::skill::effect_flag::FEAR,
        );
        if let Some(b) = world.objects.get_component_mut::<Buffs>(&player_object_id) {
            b.0.retain(|x| x.skill_id != skill_id);
        }
        if was_afraid
            && let Some(ai) = world
                .objects
                .get_component_mut::<crate::model::npc::NpcAi>(&player_object_id)
            && ai.intention == crate::model::npc::NpcIntention::MoveTo
        {
            ai.intention = crate::model::npc::NpcIntention::Active;
        }
        recompute_npc_buffed_stats(world, player_object_id);
        broadcast_target_buffs(world, player_object_id);
        // The expiry has to reach the client too, or the summon keeps showing
        // the buffed speed after the buff is gone.
        refresh_summon_info(world, player_object_id);
        return;
    }
    // `Transformation` buffs carry no stat modifier — `remove_buff` below is a
    // no-op for them — so the revert lives here: drop the display id/collision/
    // granted skills before the generic removal, and defer the extra self
    // packets (AVE + SkillList) to piggyback on the `broadcast_user_info` call
    // a few lines down rather than sending a second `UserInfo`.
    let skill_level = maybe_buff_level(world, player_object_id, skill_id);
    let is_transform = skill_level.is_some_and(|lvl| {
        world.data.skill_data.get(skill_id, lvl).is_some_and(|s| {
            s.effects
                .iter()
                .any(|e| matches!(e, SkillEffect::Transform { .. }))
        })
    });
    if is_transform {
        crate::game_loop::admin::transforms::remove_transform_state(world, player_object_id);
    }
    // `FakeDeath.onExit` — stand back up. Read the flag off the *expiring buff*
    // (not the skill template) so this fires only for fake death, and keeps
    // working for a buff whose skill row is no longer loadable — the same
    // source `Fear`'s own `onExit` and `break_fake_death_on_damage` use.
    let was_fake_dead = is_effect(
        world,
        player_object_id,
        skill_id,
        crate::model::skill::effect_flag::FAKE_DEATH,
    );
    if was_fake_dead {
        stop_fake_death(world, player_object_id);
    }
    crate::game_loop::stat_ctx::with_stat_ctx(world, player_object_id, |ctx| ctx.remove(skill_id));
    // Reverting a MaxHp/MaxMp/MaxCp buff shrinks the bar (and clamps current).
    recompute_max_vitals(world, player_object_id);
    let now = world.tick;
    // Removing the buff reverted its stat contribution — rebroadcast so the
    // client (and nearby players, for speed) see the stats return to normal.
    crate::game_loop::player_info::broadcast_user_info(world, player_object_id);
    if is_transform {
        crate::game_loop::admin::transforms::refresh_transform_visuals(world, player_object_id);
    }
    if had_visuals {
        refresh_abnormal_visuals(world, player_object_id);
    }
    if let Some(buffs) = world.objects.get_component::<Buffs>(&player_object_id) {
        send_to_player(
            world,
            player_object_id,
            crate::network::enter_world::abnormal_status_update(buffs, now),
        );
    }
}
