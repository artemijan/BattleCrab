//! The scheduled cast phases: launch, finish (per-target effects, witnesses,
//! shot consume) and the cool-down end with its queued-action replay.

use super::apply_cast_consequences;
use super::calc_buff_debuff_reflection;
use super::in_cast_range;
use super::matchup_effects;
use super::stop_casting;
use super::target_state;
use crate::game_loop::common::maybe_distance_too_far;
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers;

use crate::game_loop::skills::effects::apply_skill_effects;
use crate::model::Player;
use crate::model::components::Casting;
use crate::model::components::Position;
use crate::model::components::Vitals;
use crate::model::skill::Skill;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::scheduler::ms_to_ticks;
use crate::world::World;

/// A cast task's `CastState` if it's still the live one (seq matches);
/// stale/aborted tasks resolve to `None` and no-op.
pub(crate) fn live_cast(
    world: &World,
    player_object_id: i32,
    cast_seq: u64,
) -> Option<crate::model::CastState> {
    world
        .objects
        .get_component::<Casting>(&player_object_id)
        .map(|c| c.0.clone())
        .filter(|c| c.seq == cast_seq)
}

/// [`live_cast`] plus the skill row it names — the prologue every scheduled
/// cast phase (channeling tick, launch, finish) opens with. Both halves are
/// stale guards: the phase belongs to one cast generation, and a skill whose
/// row vanished under a reload has nothing left to resolve.
pub(super) fn live_cast_skill(
    world: &World,
    player_object_id: i32,
    cast_seq: u64,
) -> Option<(crate::model::CastState, Skill)> {
    let cast = live_cast(world, player_object_id, cast_seq)?;
    let skill = world
        .data
        .skill_data
        .get_enchanted(cast.skill_id, cast.skill_level, cast.skill_sub_level)
        .cloned()?;
    Some((cast, skill))
}

/// Port of `SkillCaster.launchSkill` (phase 1): re-check `effectRange`
/// (failure → SM 748 + a *quiet* stop, `stopCasting(false)` — Java only
/// sends `MagicSkillCanceled` on explicit aborts), broadcast
/// `MagicSkillLaunched`, mark the cast unabortable, schedule the finish.
pub(crate) fn handle_skill_launch(world: &mut World, player_object_id: i32, cast_seq: u64) {
    let Some((cast, skill)) = live_cast_skill(world, player_object_id, cast_seq) else {
        return;
    };

    // Target gone (logged off / decayed) → quiet stop, like Java's dead-ref
    // return.
    if target_state(world, cast.target_object_id).is_none() {
        stop_casting(world, player_object_id);
        return;
    }

    if skill.effect_range > 0 && cast.target_object_id != player_object_id {
        let Some(caster_pos) = maybe_position(world, player_object_id) else {
            return;
        };
        if !in_cast_range(
            world,
            player_object_id,
            &caster_pos,
            cast.target_object_id,
            skill.effect_range,
            true,
        ) {
            maybe_distance_too_far(world, player_object_id);
            stop_casting(world, player_object_id);
            return;
        }
    }

    helpers::broadcast_including_self(
        world,
        player_object_id,
        &server_packets::magic_skill_launched(
            player_object_id,
            skill.id,
            skill.level,
            &[cast.target_object_id],
        ),
    );

    if let Some(c) = world
        .objects
        .get_component_mut::<Casting>(&player_object_id)
    {
        c.0.launched = true;
    }
    world.scheduler.schedule(
        world.tick + ms_to_ticks(cast.cancel_ms),
        ScheduledTask::SkillFinish {
            player_object_id,
            cast_seq,
        },
    );
}

/// Port of `SkillCaster.finishSkill` + `callSkill` (phase 2): re-check and
/// consume MP/HP (failure → SM + quiet stop, no cancel packet), apply the
/// skill's effects, then either free the cast slot or hold it for
/// `_coolTime`.
pub(crate) fn handle_skill_finish(world: &mut World, player_object_id: i32, cast_seq: u64) {
    use server_packets::sm_ids;

    let Some((cast, skill)) = live_cast_skill(world, player_object_id, cast_seq) else {
        return;
    };
    let client_id = helpers::client_for_player(world, player_object_id);

    // MP/HP re-check at landing (no refund of the initial consume).
    let scaled_mp_consume =
        crate::game_loop::skills::effects::mp_consume_for(world, player_object_id, &skill);
    let Some(v) = world.objects.get_component::<Vitals>(&player_object_id) else {
        return;
    };
    let insufficient_mp = v.cur_mp < scaled_mp_consume as f64;
    let insufficient_hp = v.cur_hp <= skill.hp_consume as f64;
    if insufficient_mp || insufficient_hp {
        if let Some(client_id) = client_id {
            let sm = if insufficient_mp {
                sm_ids::NOT_ENOUGH_MP
            } else {
                sm_ids::NOT_ENOUGH_HP
            };
            helpers::send_sm_and_action_failed(world, client_id, sm, &[]);
        }
        stop_casting(world, player_object_id);
        return;
    }

    let mut updates = Vec::new();
    if let Some(vitals) = world.objects.get_component_mut::<Vitals>(&player_object_id) {
        // Java keys the branch off the *raw* cost (`_skill.getMpConsume() > 0
        // ? getStat().getMpConsume(_skill) : 0`), so a skill that costs
        // nothing stays free however the rates move.
        if skill.mp_consume > 0 {
            vitals.cur_mp = (vitals.cur_mp - scaled_mp_consume as f64).max(0.0);
            updates.push((
                server_packets::status_update_type::CUR_MP,
                vitals.cur_mp as i32,
            ));
        }
        if skill.hp_consume > 0 {
            vitals.cur_hp = (vitals.cur_hp - skill.hp_consume as f64).max(0.0);
            updates.push((
                server_packets::status_update_type::CUR_HP,
                vitals.cur_hp as i32,
            ));
        }
    }
    if !updates.is_empty() {
        if let Some(client_id) = client_id {
            helpers::send_to_client(
                world,
                client_id,
                server_packets::status_update(player_object_id, &updates),
            );
        }
        crate::game_loop::party::notify_party_vitals(world, player_object_id);
    }

    // Java `SkillCaster.finishSkill`'s item consume, between the MP/HP spend and
    // the effects: a `SKILL_REDUCE_ON_SKILL_SUCCESS` item is spent *here*, and
    // a failed spend (someone dropped/traded it mid-cast) aborts the cast with
    // no effects at all — Java's `return false`.
    if cast.trigger_item_object_id != 0 && skill.item_consume_id > 0 && skill.item_consume_count > 0
    {
        let taken = world
            .objects
            .get_component_mut::<crate::model::inventory::Inventory>(&player_object_id)
            .and_then(|inv| {
                inv.remove_by_object_id(
                    cast.trigger_item_object_id,
                    i64::from(skill.item_consume_count),
                )
            });
        let Some(change) = taken else {
            stop_casting(world, player_object_id);
            return;
        };
        crate::game_loop::helpers::send_inventory_update(world, player_object_id, vec![change]);
    }

    // `Skill.forEachTargetAffected` — expand the primary target through the
    // skill's affect scope, then run `callSkill` per affected creature. A
    // single-target skill resolves to exactly the primary target, so this is
    // the old path with one extra element in the loop.
    let affected = if target_state(world, cast.target_object_id).is_some() {
        crate::game_loop::skills::affect::targets_affected(
            world,
            player_object_id,
            cast.target_object_id,
            &skill,
        )
    } else {
        Vec::new()
    };
    // `Npc.onSkillSee` witnesses. Java scans **every NPC within 1000 units of
    // the caster** (`forEachVisibleObjectInRange(player, Npc.class, 1000, …)`),
    // not just the skill's targets — a mob can react to a spell aimed at
    // something else. This narrowed to the target set until 2026-08-05, which
    // happened to satisfy quest 350 (a single-target cast on the mob it
    // watches) and so looked correct.
    const SKILL_SEE_RANGE: f64 = 1000.0;
    let caster_pos = maybe_position(world, player_object_id);
    let caster_region = helpers::region_cell_of(world, player_object_id);
    let skill_see_witnesses: Vec<i32> = match (caster_pos, caster_region) {
        (Some(pos), Some(region)) => world
            .npcs_visible_from(region)
            .into_iter()
            .filter(|oid| {
                world
                    .objects
                    .get_component::<Position>(oid)
                    .is_some_and(|p| pos.distance_2d(p) <= SKILL_SEE_RANGE)
            })
            .collect(),
        _ => Vec::new(),
    };
    // Kept for `OnCreatureSkillFinishCast` below, which resolves its trigger
    // against the cast's own target.
    let first_affected = affected.first().copied();
    // The skill's target list, kept for the support-aggro test below (the
    // effect loop consumes `affected`).
    let affected_for_hate = affected.clone();

    for target_oid in affected {
        // Each target is re-checked: an AoE's effects on an early target can
        // kill or despawn a later one (Java re-resolves through the world too).
        if target_state(world, target_oid).is_none() {
            continue;
        }
        // `Skill.applyEffects`' reflection branch: a debuff can be bounced
        // back onto its own caster by the target's `ReflectSkill` chance.
        if calc_buff_debuff_reflection(world, target_oid, &skill) {
            // Java swaps the roles — `applyEffects(target, caster, …)` — so the
            // debuff lands on the caster with the target as effector.
            apply_skill_effects(world, target_oid, player_object_id, &skill);
        } else {
            apply_skill_effects(world, player_object_id, target_oid, &skill);
        }
        // `EffectScope.PVE`/`PVP` — applied to the same target as GENERAL, but
        // only for the matching matchup:
        //   playable → attackable  ⇒ PVE
        //   playable → playable    ⇒ PVP
        //   otherwise              ⇒ neither
        if let Some(extra) = matchup_effects(world, player_object_id, target_oid, &skill)
            && !extra.is_empty()
        {
            let scoped = Skill {
                self_continuous: false,
                effects: extra,
                ..skill.clone()
            };
            apply_skill_effects(world, player_object_id, target_oid, &scoped);
        }
        // The hate/PvP consequences are unconditional: the caster still *cast*
        // a bad skill at this target, reflected or not.
        apply_cast_consequences(world, player_object_id, target_oid, &skill);
    }

    // `Npc.onSkillSee` for each NPC that saw the cast, plus the support-aggro
    // rule that shares Java's scan.
    for witness in skill_see_witnesses {
        let npc_id = helpers::npc_id_of(world, witness);
        if let Some(npc_id) = npc_id {
            crate::game_loop::quests::notify_skill_see(
                world,
                player_object_id,
                witness,
                npc_id,
                cast.skill_id,
            );
        }
        // Java's "On Skill See logic", in the same loop: a *beneficial* skill
        // (`effectPoint > 0`) cast near an attackable that is already fighting
        // draws hate onto the caster — the reason healing the tank pulls the
        // mob onto the healer. It fires when the mob's current target is one of
        // the skill's targets, or when the mob is itself a target.
        //
        // Hate is `effectPoint * 150 / (level + 7)`, and the *caster* credited
        // is the summon when a summon cast it, otherwise the player.
        // `npcMob.isAttackable() && attackable.getAI().getIntention() ==
        // AI_INTENTION_ATTACK`. The port has no `TargetRef` on NPCs — the aggro
        // list *is* the target, and `npc_ai` already documents `most_hated` as
        // standing in for `getTarget()`.
        let fighting = world
            .objects
            .get_component::<crate::model::npc::NpcAi>(&witness)
            .is_some_and(|ai| ai.intention == crate::model::npc::NpcIntention::Attack)
            && helpers::npc_template(world, witness).is_some_and(|tpl| tpl.is_auto_attackable());
        if skill.effect_point > 0 && fighting {
            let npc_target = world
                .objects
                .get_component::<crate::model::npc::AggroList>(&witness)
                .and_then(|a| a.most_hated());
            let relevant = affected_for_hate
                .iter()
                .any(|&t| Some(t) == npc_target || t == witness);
            if relevant {
                let level = helpers::npc_template(world, witness).map_or(1, |tpl| tpl.level);
                let hate = f64::from(skill.effect_point) * 150.0 / f64::from(level + 7);
                crate::game_loop::minions::add_hate(world, witness, player_object_id, hate);
            }
        }
    }

    // `EffectScope.SELF` — a separate `applyEffects(caster, caster, …)` after
    // the target loop, so a skill can buff its caster while debuffing its
    // target (Blinding Blow 321, Critical Blow 409, Vengeance 368, …). The
    // parser used to read only `<effects>`, so none of these landed.
    if !skill.self_effects.is_empty() {
        let self_skill = Skill {
            self_continuous: false,
            effects: skill.self_effects.clone(),
            ..skill.clone()
        };
        apply_skill_effects(world, player_object_id, player_object_id, &self_skill);
    }

    // Java `OnCreatureSkillFinishCast` — the event `TriggerSkillByMagicType`
    // listens on. Fired once per cast, after the effects have landed, and
    // resolved against the cast's own target (Dance of Shadows' Cancel Shadow
    // Move is what ends the dance's stealth the moment you act).
    crate::game_loop::skills::effects::fire_magic_type_triggers(
        world,
        player_object_id,
        first_affected.unwrap_or(cast.target_object_id),
        skill.magic_type,
    );
    // The augment-option procs Java fires from `SkillCaster` alongside it:
    // `MAGIC` on a magic cast, `ATTACK` on a physical one, nothing on a static
    // skill.
    crate::game_loop::skills::effects::fire_option_cast_triggers(
        world,
        player_object_id,
        first_affected.unwrap_or(cast.target_object_id),
        skill.magic_type,
    );

    // Attack stance is caster-scoped, so it fires once per cast rather than
    // per affected target.
    if skill.is_bad() {
        // Start attack stance (finalizer, right after `callSkill`): a bad skill
        // with an action draws the weapon and starts the 15 s combat timer, so
        // `canLogout` refuses a relogin. Java also excludes `isWithoutAction()`
        // skills and `DOOR_TREASURE` targets; neither is modeled here, so
        // `is_bad()` is the whole gate.
        crate::game_loop::combat::refresh_attack_stance(world, player_object_id);
    }

    // Hold the cast slot for the cool phase (`stopCasting(false)` after
    // `_coolTime`), freeing inline when there's nothing to wait out.
    let cool_ticks = ms_to_ticks(cast.cool_ms);
    if cool_ticks == 0 {
        stop_casting(world, player_object_id);
    } else {
        world.scheduler.schedule(
            world.tick + cool_ticks,
            ScheduledTask::CastEnd {
                player_object_id,
                cast_seq,
            },
        );
    }
}

/// `SkillCaster.run`'s terminal `stopCasting(false)` — the cool phase ended.
pub(crate) fn handle_cast_end(world: &mut World, player_object_id: i32, cast_seq: u64) {
    let Some(cast) = live_cast(world, player_object_id, cast_seq) else {
        return;
    };
    let (skill_id, target) = (cast.skill_id, cast.target_object_id);
    let skill_level = cast.skill_level;
    stop_casting(world, player_object_id);
    // `SkillCaster.finishSkill`'s "Attack target after skill use" block, which
    // is why a Power Strike leaves you swinging rather than standing still.
    //
    // Java gates it on the AI having no queued intention, a real target that
    // is neither the caster nor un-attackable, and — for `ATTACK` only —
    // shift not being held (the port has no shift-cast, so that clause is
    // vacuous). The `CAST` branch is inert here; see `resume_action_after_cast`
    // for why that costs nothing on this dist.
    resume_action_after_cast(world, player_object_id, target, skill_id, skill_level);
    // `EVT_FINISH_CASTING` → script `onSpellFinished`, for NPC casters a
    // script registered (the Primeval Isle Tyrannosaurus's berserk chains).
    let npc_id = helpers::npc_id_of(world, player_object_id);
    if let Some(npc_id) = npc_id {
        crate::game_loop::quests::notify_spell_finished(
            world,
            player_object_id,
            npc_id,
            skill_id,
            target,
        );
    }
}

/// Test hook for [`resume_action_after_cast`], private to this module.
#[cfg(test)]
pub(crate) fn resume_action_after_cast_for_test(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill_id: i32,
    skill_level: i32,
) {
    resume_action_after_cast(world, caster_oid, target_oid, skill_id, skill_level);
}

/// `SkillCaster.finishSkill`'s next-action block — see the call site.
fn resume_action_after_cast(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill_id: i32,
    skill_level: i32,
) {
    use crate::model::skill::NextAction;

    let Some(next) = world
        .data
        .skill_data
        .get(skill_id, skill_level)
        .map(|s| s.next_action)
    else {
        return;
    };
    if next == NextAction::None {
        return;
    }
    // Players only: an NPC's post-cast behaviour is its own AI's business
    // (`npc_ai` re-thinks every tick), and Java routes NPCs through the same
    // `setIntention` this port expresses as `Intent`.
    if !world.objects.has_component::<Player>(&caster_oid) {
        return;
    }
    // `(target != null) && (target != caster) && target.isAutoAttackable(caster)`.
    if target_oid == caster_oid || target_oid == 0 {
        return;
    }
    if target_state(world, target_oid).is_none() {
        return;
    }
    if !crate::game_loop::target::is_auto_attackable(world, caster_oid, target_oid) {
        return;
    }
    match next {
        NextAction::Attack => {
            crate::game_loop::combat::resume_attack_intent(world, caster_oid, target_oid);
        }
        // SKIP(off-chronicle): Java re-queues the same skill through the AI
        // intention queue (`AI_INTENTION_CAST`), which this port does not
        // have — and re-casting inline would loop rather than repeat.
        //
        // It stays inert because nothing on this dist can reach it: exactly 11
        // skills declare `<nextAction>CAST` (11011-11016, 15482-15484,
        // 16355-16356, 19314 — Elemental Spike and friends), every one of them
        // off-chronicle, and none appears in a skill tree or an NPC skill
        // list. Building an intention queue for a branch no skill can enter
        // would be scaffolding, not parity.
        NextAction::Cast | NextAction::None => {}
    }
}
