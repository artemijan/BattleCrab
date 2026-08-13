//! Phase 0: `SkillCaster.startCasting` — and `stop_casting`, where every
//! cast-stop path funnels.

use super::*;
/// Port of `SkillCaster.startCasting` (phase 0). Narrowing: no skill mastery,
/// no `MAGIC_REUSE_RATE` stat (reuse = the skill's `reuseDelay`), no fame/
/// clan-rep consumes (item reagents ARE consumed — see below), no
/// `stopEffectsOnAction`, no `MoveToPawn` cosmetic (only `ExRotation` for
/// target facing).
pub(crate) fn start_casting(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    skill: &Skill,
    target_oid: i32,
) {
    use server_packets::{SmParam, sm_ids};

    let Some(player) = world.objects.get_component::<Player>(&object_id) else {
        return;
    };
    let Some(base) = world
        .objects
        .get_component::<crate::model::components::BaseStats>(&object_id)
    else {
        return;
    };
    let Some(mods) = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&object_id)
    else {
        return;
    };
    let Some(combat) = world
        .objects
        .get_component::<crate::model::components::CombatStats>(&object_id)
    else {
        return;
    };
    let (hit_ms, cancel_ms, cool_ms) =
        formulas::calc_cast_times(player, base, mods, combat, &world.data, skill);
    let displayed_cast_time = hit_ms + cancel_ms;

    // `SkillCaster`: recharge shots before the cast so the spiritshot bonus is
    // in effect when the effects land (`rechargeShots(useSoulShot, useSpiritShot)`
    // — magic skills use the spiritshot; physical-skill soulshots aren't wired,
    // only the melee auto-attack path charges soulshots).
    if skill.magic_type == 1 {
        crate::game_loop::items::recharge_shots(world, object_id, false, true, false);
    }

    // Register the reuse (skipped when trivially short, like Java's `> 10`),
    // under the shared group id when the skill has one.
    set_skill_reuse(world, object_id, skill);

    // Reagent consume (`SkillCaster.startCasting`): bad skills and pure
    // reagents (Java `defaultAction == NONE` — this port's `ActionType::Other`
    // covers NONE plus values `checkConsume` never branches on) pay at cast
    // start; usable items pay in their own handler, so scrolls don't consume
    // twice. Volcano's Magic Symbol 8876 lands here.
    if skill.item_consume_id > 0 && skill.item_consume_count > 0 {
        let is_reagent = skill.is_bad()
            || world
                .data
                .item_data
                .get(skill.item_consume_id)
                .is_none_or(|t| t.default_action == crate::data::item_data::ActionType::Other);
        if is_reagent {
            crate::game_loop::quests::take_items(
                world,
                client_id,
                object_id,
                skill.item_consume_id,
                skill.item_consume_count as i64,
            );
        }
    }

    // The post-cast raid-curse scan (Java runs it at the tail of the cast, so
    // the skill itself goes off first). Catches a high-level player *helping*
    // a low-level raid party from outside the fight, which the damage-side
    // check never sees.
    crate::game_loop::raid_curse::on_skill_cast_near_raid(world, object_id, skill.is_bad());

    // A new cast wipes the queue slot — Java clears `_queuedSkill` on every
    // successful `useMagic`, `changeIntention` drops `_nextIntention` for
    // offensive skills, and `setIntention(CAST)` cancels a pending equip.
    world.objects.remove_component::<QueuedAction>(&object_id);

    // Stop movement (`clientStopMoving`) — the client freezes on its own; the
    // broadcast pins the position for everyone else. The interrupted manual
    // move is NOT resumed: `PlayerAI.changeIntention` does save the MOVE_TO
    // as `_nextIntention` when a good-skill CAST comes in, but
    // `startCasting` immediately follows with `setIntention(IDLE)`, whose
    // `changeIntention(IDLE)` wipes `_nextIntention` ("Also replace other
    // intentions with idle. (Mainly done for AI_INTENTION_MOVE_TO)"). An
    // attack loop's chase leg is different: the surviving `Intent` component
    // resumes the loop (and its chase) by itself.
    stop_movement(world, object_id);

    // Face the target (Java: `setHeading` + broadcast `ExRotation`).
    if target_oid != object_id {
        let Some((tx, ty, _, _)) = target_state(world, target_oid) else {
            return;
        };
        let heading = {
            let Some(pos) = world.objects.get_component::<Position>(&object_id) else {
                return;
            };
            crate::model::movement::calculate_heading((tx - pos.x) as f64, (ty - pos.y) as f64)
        };
        if let Some(pos) = world.objects.get_component_mut::<Position>(&object_id) {
            pos.heading = heading;
        }
        broadcast_including_self(
            world,
            object_id,
            &crate::network::enter_world::ex_rotation(object_id, heading),
        );
    }

    // Initial MP consume + StatusUpdate (re-checked here in Java too).
    let mut mp_update = None;
    if skill.mp_initial_consume > 0 {
        let Some(vitals) = world.objects.get_component_mut::<Vitals>(&object_id) else {
            return;
        };
        if vitals.cur_mp < skill.mp_initial_consume as f64 {
            send_sm_and_action_failed(world, client_id, sm_ids::NOT_ENOUGH_MP, &[]);
            return;
        }
        vitals.cur_mp -= skill.mp_initial_consume as f64;
        mp_update = Some(vitals.cur_mp as i32);
    }
    if let Some(mp) = mp_update {
        send_to_client(
            world,
            client_id,
            server_packets::status_update(
                object_id,
                &[(server_packets::status_update_type::CUR_MP, mp)],
            ),
        );
        crate::game_loop::party::notify_party_vitals(world, object_id);
    }

    // Broadcast the cast start, then the caster-only YOU_USE_S1 + cast bar.
    {
        let Some((tx, ty, tz, _)) = target_state(world, target_oid) else {
            return;
        };
        let caster = &world
            .objects
            .get_component::<Player>(&object_id)
            .expect("player");
        let Some(caster_pos) = world.objects.get_component::<Position>(&object_id) else {
            return;
        };
        broadcast_including_self(
            world,
            object_id,
            &server_packets::magic_skill_use(
                caster,
                caster_pos,
                (target_oid, tx, ty, tz),
                skill.id,
                skill.level,
                displayed_cast_time,
                skill.reuse_delay_group,
                skill.reuse_delay,
            ),
        );
    }
    send_sm_to_client(
        world,
        client_id,
        sm_ids::YOU_USE_S1,
        &[SmParam::SkillName {
            id: skill.id,
            level: skill.level,
        }],
    );
    send_to_client(
        world,
        client_id,
        server_packets::setup_gauge(object_id, 0, displayed_cast_time),
    );

    let cast_seq = {
        let Some(player) = world.objects.get_component_mut::<Player>(&object_id) else {
            return;
        };
        player.cast_seq += 1;
        player.cast_seq
    };
    world.objects.add_components(
        &object_id,
        Casting(crate::model::CastState {
            skill_id: skill.id,
            skill_level: skill.level,
            skill_sub_level: skill.sub_level,
            target_object_id: target_oid,
            seq: cast_seq,
            launched: false,
            cancel_ms,
            cool_ms,
            trigger_item_object_id: 0,
        }),
    );
    world.scheduler.schedule(
        world.tick + ms_to_ticks(hit_ms),
        ScheduledTask::SkillLaunch {
            player_object_id: object_id,
            cast_seq,
        },
    );

    // `SkillCaster.startCasting`'s channeling hook: the `SkillChannelizer`
    // fixed-rate task — first fire after `channelingStart`, then every
    // `channelingTickInterval` (the tick handler re-schedules itself while
    // the cast lives; `stop_casting` removing `Casting` is Java's
    // `stopChanneling`, on completion and abort alike).
    if skill.operate_type == OperateType::Channeling && skill.channeling_tick_ms > 0 {
        world.scheduler.schedule(
            world.tick + ms_to_ticks(skill.channeling_start_ms),
            ScheduledTask::ChannelingTick {
                player_object_id: object_id,
                cast_seq,
            },
        );
    }
}

/// Every cast-stop path funnels here — Java `SkillCaster.stopCasting`: free
/// the casting slot, then fire whatever the cast held back (the queued skill
/// `useMagic` replay, or `EVT_FINISH_CASTING` → the saved MOVE_TO / pending
/// equip). The dead don't replay (guarded in `run_queued_action`; the slot is
/// also cleared in `player_do_die`).
pub(crate) fn stop_casting(world: &mut World, object_id: i32) {
    world.objects.remove_component::<Casting>(&object_id);
    // Java `stopChanneling()` runs off the same stop path.
    stop_channelizing(world, object_id);
    run_queued_action(world, object_id);
}
