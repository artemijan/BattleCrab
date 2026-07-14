//! The casting pipeline: `RequestMagicSkillUse` validation, target
//! resolution, and the three scheduled phases (launch → finish → cool-down
//! end), plus cast aborts.

use crate::game_loop::common::maybe_distance_too_far;
use crate::game_loop::helpers::{
    broadcast_including_self, client_for_player, ms_to_ticks, run_queued_action,
    send_sm_and_action_failed,
};
use crate::model::components::{
    AttackState, Casting, Collision, Intent, Movement, Position, QueuedAction, Vitals,
};
use crate::model::formulas;
use crate::model::skill::{OperateType, Skill, TargetType};
use crate::model::Player;
use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

use super::effects::apply_skill_effects;

/// Reuse gate shared by `use_magic_on` and the `ItemSkills` item handler
/// (Java `Player.isSkillDisabled`/`getSkillRemainingReuseTime`), keyed by the
/// shared reuse group when the skill has one. `true` means the skill is off
/// cooldown; a still-cooling skill sends the h/m/s breakdown (or SM 48 for
/// short reuses) plus `ActionFailed` and returns `false`.
pub(crate) fn check_skill_reuse(world: &World, client_id: u32, object_id: i32, skill: &Skill) -> bool {
    use server_packets::{sm_ids, SmParam};

    let Some(&crate::model::SkillReuse { until_tick, total_ms, .. }) = world
        .objects
        .get_component::<crate::model::components::Reuses>(&object_id)
        .and_then(|r| r.0.get(&skill.reuse_key()))
    else {
        return true;
    };
    if until_tick <= world.tick {
        return true;
    }
    let name_param = SmParam::SkillName { id: skill.id, level: skill.level };
    if total_ms > 3000 {
        let remaining_ms = (until_tick - world.tick) * 100;
        let hours = (remaining_ms / 3_600_000) as i32;
        let minutes = ((remaining_ms % 3_600_000) / 60_000) as i32;
        let seconds = ((remaining_ms / 1000) % 60) as i32;
        if hours > 0 {
            send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::S2_HOURS_S3_MINUTES_S4_SECONDS_REMAINING_FOR_REUSE,
                &[name_param, SmParam::Int(hours), SmParam::Int(minutes), SmParam::Int(seconds)],
            );
        } else if minutes > 0 {
            send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::S2_MINUTES_S3_SECONDS_REMAINING_FOR_REUSE,
                &[name_param, SmParam::Int(minutes), SmParam::Int(seconds)],
            );
        } else {
            send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::S2_SECONDS_REMAINING_FOR_REUSE,
                &[name_param, SmParam::Int(seconds)],
            );
        }
    } else {
        send_sm_and_action_failed(world, client_id, sm_ids::S1_IS_NOT_AVAILABLE_REUSE, &[name_param]);
    }
    false
}

/// Registers a skill's cooldown (Java `Player.addTimeStamp`), skipped when
/// trivially short (`> 10` ms, like Java). Shared by `start_casting` and the
/// `ItemSkills` item handler (immediate-effect items never enter
/// `start_casting`).
pub(crate) fn set_skill_reuse(world: &mut World, object_id: i32, skill: &Skill) {
    if skill.reuse_delay <= 10 {
        return;
    }
    let until_tick = world.tick + ms_to_ticks(skill.reuse_delay);
    if let Some(reuses) = world.objects.get_component_mut::<crate::model::components::Reuses>(&object_id) {
        reuses.0.insert(
            skill.reuse_key(),
            crate::model::SkillReuse { skill_level: skill.level, until_tick, total_ms: skill.reuse_delay },
        );
    }
}

/// Port of `Skill.getTarget` + the `targethandlers/{Self,Target,Enemy,
/// EnemyOnly}.java` scripts as a static match over players *and* NPCs (G9).
/// `Err(sm_id)` is the system message the caller sends alongside
/// `ActionFailed` (Java: the handlers' `sendMessage` path) — SM 109 for an
/// invalid target, SM 181 when geodata blocks line of sight.
pub(crate) fn resolve_cast_target(
    world: &World,
    caster: &Player,
    caster_pos: &Position,
    caster_target: Option<i32>,
    skill: &Skill,
    ctrl: bool,
) -> Result<i32, i16> {
    use server_packets::sm_ids;

    let resolved = match skill.target_type {
        // `Self.java`: a bad (offensive) self-target skill is refused inside
        // a peace zone — SM 2167.
        TargetType::Self_ => {
            let in_peace = world
                .objects
                .get_component::<crate::model::components::ZoneFlags>(&caster.object_id)
                .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Peace));
            if in_peace && skill.is_bad() {
                return Err(sm_ids::YOU_CANNOT_USE_SKILLS_THAT_MAY_HARM_OTHER_PLAYERS_IN_HERE);
            }
            return Ok(caster.object_id);
        }
        // `Target.java`: the selected target, friend or foe; self allowed
        // (and self skips the LOS check — "you can always target yourself").
        TargetType::Target => {
            let t = caster_target.ok_or(sm_ids::INVALID_TARGET)?;
            if t == caster.object_id {
                return Ok(t);
            }
            t
        }
        // `Enemy.java`/`EnemyOnly.java`: not self, and `isAutoAttackable ||
        // forceUse` — monsters are auto-attackable; players carry no PvP
        // flag/karma yet, so hitting one still needs ctrl (force-use).
        TargetType::Enemy | TargetType::EnemyOnly => {
            let t = caster_target.ok_or(sm_ids::INVALID_TARGET)?;
            if t == caster.object_id {
                return Err(sm_ids::INVALID_TARGET);
            }
            let auto_attackable = world
                .objects
                .get_component::<crate::model::npc::Npc>(&t)
                .and_then(|n| n.template(world))
                .is_some_and(|tm| tm.is_auto_attackable());
            if !auto_attackable && !ctrl {
                return Err(sm_ids::INVALID_TARGET);
            }
            t
        }
        TargetType::Other => return Err(sm_ids::INVALID_TARGET),
    };
    let (tx, ty, tz, target_dead) = target_state(world, resolved).ok_or(sm_ids::INVALID_TARGET)?;
    if target_dead {
        return Err(sm_ids::INVALID_TARGET);
    }
    // "Geodata check when character is within range" — every non-self
    // handler ends with `GeoEngine.canSeeTarget` → CANNOT_SEE_TARGET.
    if !world
        .geo
        .can_see_target(caster_pos.x, caster_pos.y, caster_pos.z, tx, ty, tz)
    {
        return Err(sm_ids::CANNOT_SEE_TARGET);
    }
    // `Enemy`/`EnemyOnly.java`: "Skills with this target type cannot be used
    // by playables on playables in peace zone, but can be used by and on
    // NPCs" — SM 2167 (after the LOS check, matching the handlers' order).
    if matches!(skill.target_type, TargetType::Enemy | TargetType::EnemyOnly)
        && crate::game_loop::zones::is_inside_peace_zone(world, caster.object_id, resolved)
    {
        return Err(sm_ids::YOU_CANNOT_USE_SKILLS_THAT_MAY_HARM_OTHER_PLAYERS_IN_HERE);
    }
    Ok(resolved)
}

/// Position + liveness of a castable target, whichever registry it lives in
/// (plus its collision radius for the range gates).
pub(crate) fn target_state(world: &World, object_id: i32) -> Option<(i32, i32, i32, bool)> {
    if world.objects.get_component::<Player>(&object_id).is_some() {
        let pos = world.objects.get_component::<Position>(&object_id)?;
        let vitals = world.objects.get_component::<Vitals>(&object_id)?;
        return Some((pos.x, pos.y, pos.z, vitals.dead));
    }
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&object_id)?;
    let pos = world.objects.get_component::<Position>(&object_id)?;
    let vitals = world.objects.get_component::<Vitals>(&object_id)?;
    Some((pos.x, pos.y, pos.z, vitals.dead))
}

/// `Util.checkIfInRange` over any two castable actors.
pub(crate) fn in_cast_range(
    world: &World,
    caster_oid: i32,
    caster_pos: &Position,
    target_oid: i32,
    range: i32,
    include_z: bool,
) -> bool {
    let Some((tx, ty, tz, _)) = target_state(world, target_oid) else {
        return false;
    };
    let target_radius = world
        .objects
        .get_component::<Collision>(&target_oid)
        .map(|c| c.radius)
        .unwrap_or(0.0);
    let caster_radius = world
        .objects
        .get_component::<Collision>(&caster_oid)
        .map(|c| c.radius)
        .unwrap_or(0.0);
    let (dx, dy, dz) = (
        (tx - caster_pos.x) as f64,
        (ty - caster_pos.y) as f64,
        (tz - caster_pos.z) as f64,
    );
    let d2 = dx * dx + dy * dy + if include_z { dz * dz } else { 0.0 };
    let reach = range as f64 + caster_radius + target_radius;
    d2 <= reach * reach
}

/// Port of `clientpackets/RequestMagicSkillUse.runImpl`: parse and hand to
/// `use_magic`.
pub(crate) fn handle_request_magic_skill_use(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestMagicSkillUse::read(body) else {
        return;
    };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();
    use_magic(
        world,
        client_id,
        object_id,
        pkt.magic_id,
        pkt.ctrl_pressed,
        pkt.shift_pressed,
    );
}

/// Port of `Player.useMagic`'s guards + `SkillCaster.castSkill`/
/// `checkUseConditions`, entered from the packet handler and from the
/// queued-skill replay (`run_queued_action`). Narrowing: no mute/sit/
/// fake-death states (none exist), toggles and non-single targeting still
/// silently ignored.
pub(crate) fn use_magic(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    magic_id: i32,
    ctrl: bool,
    shift: bool,
) {
    use_magic_on(world, client_id, object_id, magic_id, ctrl, shift, None);
}

/// `use_magic` with an optional pre-resolved target: the walk-to-cast think
/// (`player_cast_think`) re-enters here with the intent's snapshotted target
/// so a mid-walk re-target can't redirect the cast; everything else about the
/// click is re-validated from scratch.
pub(crate) fn use_magic_on(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    magic_id: i32,
    ctrl: bool,
    shift: bool,
    forced_target: Option<i32>,
) {
    use server_packets::sm_ids;

    let Some(player) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
    else {
        return;
    };
    // The dead can't cast (`checkUseConditions` → `isDead`).
    if world
        .objects
        .get_component::<Vitals>(&object_id)
        .is_none_or(|v| v.dead)
    {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    // Unknown skill → ActionFailed (RequestMagicSkillUse.runImpl).
    let Some(&skill_level) = world
        .objects
        .get_component::<crate::model::components::SkillBook>(&object_id)
        .and_then(|book| book.0.get(&magic_id))
    else {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    };
    let Some(skill) = world.data.skill_data.get(magic_id, skill_level).cloned() else {
        return;
    };

    // Passive → ActionFailed (useMagic); toggles/unsupported targeting are
    // not castable yet and are consumed silently, same as before.
    if skill.operate_type == OperateType::Passive {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    if skill.operate_type != OperateType::Active || skill.target_type == TargetType::Other {
        return;
    }

    // Reuse gate (`Player.useMagic`'s `isSkillDisabled` branch), keyed by the
    // shared reuse group when the skill has one: timestamp reuses (> 3000 ms)
    // get the remaining h/m/s breakdown, short ones SM 48.
    if !check_skill_reuse(world, client_id, object_id, &skill) {
        return;
    }

    // Target validity first, like Java (`useMagic` resolves and checks the
    // target before the queue/MP decisions).
    let Some(caster_pos) = world.objects.get_component::<Position>(&object_id).copied() else {
        return;
    };
    let caster_target = forced_target.or_else(|| {
        world
            .objects
            .get_component::<crate::model::components::TargetRef>(&object_id)
            .copied()
            .unwrap_or_default()
            .0
    });
    let target_oid =
        match resolve_cast_target(world, player, &caster_pos, caster_target, &skill, ctrl) {
            Ok(oid) => oid,
            Err(sm_id) => {
                send_sm_and_action_failed(world, client_id, sm_id, &[]);
                return;
            }
        };

    // Busy — mid-cast or mid-swing (`useMagic`'s `isAttackingNow() ||
    // isCastingNow()` branch): park the click in the queue slot instead of
    // dropping it; it replays with full re-validation when the cast stops
    // (`stop_casting`) or the swing ends (`AttackFinish`/`thinkAttack`).
    // Java checks MP only after this, so a low-MP click still queues.
    let mid_swing = world
        .objects
        .get_component::<AttackState>(&object_id)
        .is_some_and(|st| st.attack_end_tick > world.tick);
    if mid_swing || world.objects.has_component::<Casting>(&object_id) {
        world.objects.add_components(
            &object_id,
            QueuedAction::Skill {
                skill_id: magic_id,
                ctrl,
                shift,
            },
        );
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }

    // MP/HP prechecks (`checkUseConditions`).
    let Some(v) = world.objects.get_component::<Vitals>(&object_id) else {
        return;
    };
    if v.cur_mp < (skill.mp_initial_consume + skill.mp_consume) as f64 {
        send_sm_and_action_failed(world, client_id, sm_ids::NOT_ENOUGH_MP, &[]);
        return;
    }
    if v.cur_hp <= skill.hp_consume as f64 {
        send_sm_and_action_failed(world, client_id, sm_ids::NOT_ENOUGH_HP, &[]);
        return;
    }

    // Cast-range gate (`SkillCaster.castSkill` returning null → the AI walks
    // into range via `thinkCast`/`maybeMoveToPawn`). Shift-click is Java's
    // `dontMove`: the target handlers reject with SM 748 instead of moving.
    let out_of_range = skill.cast_range > 0
        && target_oid != object_id
        && !in_cast_range(
            world,
            object_id,
            &caster_pos,
            target_oid,
            skill.cast_range,
            false,
        );
    if out_of_range && shift {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED,
            &[],
        );
        return;
    }

    // Past every reject: this click is now the player's order — a walk-to-cast
    // still in flight is superseded (Java: each accepted `useMagic` sets a
    // fresh CAST intention; a rejected one leaves the old intention running).
    if matches!(
        world.objects.get_component::<Intent>(&object_id),
        Some(Intent(crate::model::PlayerIntent::Cast { .. }))
    ) {
        world.objects.remove_component::<Intent>(&object_id);
    }

    if out_of_range {
        world.objects.add_components(
            &object_id,
            Intent(crate::model::PlayerIntent::Cast {
                skill_id: magic_id,
                ctrl,
                shift,
                target_object_id: target_oid,
            }),
        );
        // Start walking immediately — the first leg shouldn't wait a tick.
        crate::game_loop::combat::player_cast_think(world, object_id);
        return;
    }

    start_casting(world, client_id, object_id, &skill, target_oid);
}

/// Port of `SkillCaster.startCasting` (phase 0). Narrowing: no skill mastery,
/// no `MAGIC_REUSE_RATE` stat (reuse = the skill's `reuseDelay`), no item/
/// fame/clan-rep consumes, no `stopEffectsOnAction`, no `MoveToPawn`
/// cosmetic (only `ExRotation` for target facing).
pub(crate) fn start_casting(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    skill: &Skill,
    target_oid: i32,
) {
    use server_packets::{sm_ids, SmParam};

    let Some(player) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
    else {
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

    // Register the reuse (skipped when trivially short, like Java's `> 10`),
    // under the shared group id when the skill has one.
    set_skill_reuse(world, object_id, skill);

    // A new cast wipes the queue slot — Java clears `_queuedSkill` on every
    // successful `useMagic`, `changeIntention` drops `_nextIntention` for
    // offensive skills, and `setIntention(CAST)` cancels a pending equip.
    world.objects.remove_component::<QueuedAction>(&object_id);

    // Stop movement (`clientStopMoving`) — the client freezes on its own; the
    // broadcast pins the position for everyone else. A good-skill cast saves
    // an interrupted *manual* move to resume after (the current MOVE_TO
    // intention becomes the next intention in `changeIntention`) — but not a
    // chase leg: while attacking, Java's current intention is ATTACK, and the
    // surviving `Intent` component already resumes the loop (and its chase)
    // by itself.
    if let Some(mv) = world.objects.get_component::<Movement>(&object_id).cloned() {
        if !skill.is_bad() && !world.objects.has_component::<Intent>(&object_id) {
            let (x, y, z) = match &mv.0.geo_path {
                Some(gp) => (
                    gp.accurate_tx,
                    gp.accurate_ty,
                    gp.points[gp.points.len() - 1].2,
                ),
                None => (mv.0.dest_x, mv.0.dest_y, mv.0.dest_z),
            };
            world
                .objects
                .add_components(&object_id, QueuedAction::Move { x, y, z });
        }
        world.objects.remove_component::<Movement>(&object_id);
        if let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() {
            broadcast_including_self(
                world,
                object_id,
                &server_packets::stop_move(object_id, pos.x, pos.y, pos.z, pos.heading),
            );
        }
    }

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
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::status_update(
                object_id,
                &[(server_packets::status_update_type::CUR_MP, mp)],
            ));
        }
        crate::game_loop::party::notify_party_vitals(world, object_id);
    }

    // Broadcast the cast start, then the caster-only YOU_USE_S1 + cast bar.
    {
        let Some((tx, ty, tz, _)) = target_state(world, target_oid) else {
            return;
        };
        let caster = &world
            .objects
            .get_component::<crate::model::Player>(&object_id)
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
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::system_message_with(
            sm_ids::YOU_USE_S1,
            &[SmParam::SkillName {
                id: skill.id,
                level: skill.level,
            }],
        ));
        cs.send(server_packets::setup_gauge(
            object_id,
            0,
            displayed_cast_time,
        ));
    }

    let cast_seq = {
        let Some(player) = world
            .objects
            .get_component_mut::<crate::model::Player>(&object_id)
        else {
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
            target_object_id: target_oid,
            seq: cast_seq,
            launched: false,
            cancel_ms,
            cool_ms,
        }),
    );
    world.scheduler.schedule(
        world.tick + ms_to_ticks(hit_ms),
        ScheduledTask::SkillLaunch {
            player_object_id: object_id,
            cast_seq,
        },
    );
}

/// Every cast-stop path funnels here — Java `SkillCaster.stopCasting`: free
/// the casting slot, then fire whatever the cast held back (the queued skill
/// `useMagic` replay, or `EVT_FINISH_CASTING` → the saved MOVE_TO / pending
/// equip). The dead don't replay (guarded in `run_queued_action`; the slot is
/// also cleared in `player_do_die`).
pub(crate) fn stop_casting(world: &mut World, object_id: i32) {
    world.objects.remove_component::<Casting>(&object_id);
    run_queued_action(world, object_id);
}

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

/// Port of `SkillCaster.launchSkill` (phase 1): re-check `effectRange`
/// (failure → SM 748 + a *quiet* stop, `stopCasting(false)` — Java only
/// sends `MagicSkillCanceled` on explicit aborts), broadcast
/// `MagicSkillLaunched`, mark the cast unabortable, schedule the finish.
pub(crate) fn handle_skill_launch(world: &mut World, player_object_id: i32, cast_seq: u64) {
    let Some(cast) = live_cast(world, player_object_id, cast_seq) else {
        return;
    };
    let Some(skill) = world
        .data
        .skill_data
        .get(cast.skill_id, cast.skill_level)
        .cloned()
    else {
        return;
    };

    // Target gone (logged off / decayed) → quiet stop, like Java's dead-ref
    // return.
    if target_state(world, cast.target_object_id).is_none() {
        stop_casting(world, player_object_id);
        return;
    }

    if skill.effect_range > 0 && cast.target_object_id != player_object_id {
        let Some(caster_pos) = world
            .objects
            .get_component::<Position>(&player_object_id)
            .copied()
        else {
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

    broadcast_including_self(
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

    let Some(cast) = live_cast(world, player_object_id, cast_seq) else {
        return;
    };
    let Some(skill) = world
        .data
        .skill_data
        .get(cast.skill_id, cast.skill_level)
        .cloned()
    else {
        return;
    };
    let client_id = client_for_player(world, player_object_id);

    // MP/HP re-check at landing (no refund of the initial consume).
    let Some(v) = world.objects.get_component::<Vitals>(&player_object_id) else {
        return;
    };
    let insufficient_mp = v.cur_mp < skill.mp_consume as f64;
    let insufficient_hp = v.cur_hp <= skill.hp_consume as f64;
    if insufficient_mp || insufficient_hp {
        if let Some(client_id) = client_id {
            let sm = if insufficient_mp {
                sm_ids::NOT_ENOUGH_MP
            } else {
                sm_ids::NOT_ENOUGH_HP
            };
            send_sm_and_action_failed(world, client_id, sm, &[]);
        }
        stop_casting(world, player_object_id);
        return;
    }

    let mut updates = Vec::new();
    if let Some(vitals) = world.objects.get_component_mut::<Vitals>(&player_object_id) {
        if skill.mp_consume > 0 {
            vitals.cur_mp = (vitals.cur_mp - skill.mp_consume as f64).max(0.0);
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
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::status_update(player_object_id, &updates));
            }
        }
        crate::game_loop::party::notify_party_vitals(world, player_object_id);
    }

    // `callSkill` → effect application, if the target is still around.
    if target_state(world, cast.target_object_id).is_some() {
        apply_skill_effects(world, player_object_id, cast.target_object_id, &skill);
    }

    // Start attack stance (`SkillCaster` finalizer, right after `callSkill`): a
    // bad skill with an action draws the caster's weapon and starts the 15 s
    // combat timer, exactly like a melee swing — so `canLogout` refuses a
    // relogin for 15 s after nuking/debuffing a mob. Java also excludes
    // `isWithoutAction()` skills and `DOOR_TREASURE` targets; neither is modeled
    // in the cast pipeline (every bad skill it resolves has an action and
    // targets a creature), so `is_bad()` is the whole gate here.
    if skill.is_bad() {
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
    if live_cast(world, player_object_id, cast_seq).is_none() {
        return;
    }
    stop_casting(world, player_object_id);
}

/// Port of `Creature.abortCast` → `stopCasting(aborted == true)`: only casts
/// that haven't launched can be aborted; broadcast `MagicSkillCanceled` (self
/// included, to stop the animation) + `ActionFailed` to the caster. The
/// already-scheduled phase tasks go stale via the seq mismatch.
pub(crate) fn abort_cast(world: &mut World, object_id: i32) {
    let abortable = world
        .objects
        .get_component::<Casting>(&object_id)
        .is_some_and(|c| !c.0.launched);
    if !abortable {
        return;
    }
    broadcast_including_self(
        world,
        object_id,
        &server_packets::magic_skill_canceld(object_id),
    );
    if let Some(client_id) = client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
    }
    // Java `stopCasting(true)` also ends with `EVT_FINISH_CASTING`, so an
    // interrupted cast still releases the click it held back.
    stop_casting(world, object_id);
}

/// Port of `Creature.breakCast`: a cast broken by *incoming damage* (as opposed
/// to a self-initiated `abortCast`). It performs the same abort — `MagicSkillCanceled`
/// + `ActionFailed`, only for a not-yet-launched cast — and then, if the victim
/// is a player, additionally sends the `YOUR_CASTING_HAS_BEEN_INTERRUPTED`
/// system message. That extra message is the sole difference from [`abort_cast`],
/// which is why the movement/self-abort call sites keep using `abort_cast`.
pub(crate) fn break_cast(world: &mut World, object_id: i32) {
    let breakable = world
        .objects
        .get_component::<Casting>(&object_id)
        .is_some_and(|c| !c.0.launched);
    if !breakable {
        return;
    }
    abort_cast(world, object_id);
    if let Some(client_id) = client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                server_packets::sm_ids::YOUR_CASTING_HAS_BEEN_INTERRUPTED,
                &[],
            ));
        }
    }
}
