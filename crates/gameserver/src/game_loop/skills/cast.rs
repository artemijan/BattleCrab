//! The casting pipeline: `RequestMagicSkillUse` validation, target
//! resolution, and the three scheduled phases (launch → finish → cool-down
//! end), plus cast aborts.

use crate::game_loop::helpers::{
    broadcast_including_self, client_for_player, ms_to_ticks, send_sm_and_action_failed,
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

/// Port of `Skill.getTarget` + the `targethandlers/{Self,Target,Enemy,
/// EnemyOnly}.java` scripts as a static match over players *and* NPCs (G9).
/// `Err(sm_id)` is the system message the caller sends alongside
/// `ActionFailed` (Java: the handlers' `sendMessage` path) — SM 109 for an
/// invalid target, SM 181 when geodata blocks line of sight.
pub(crate) fn resolve_cast_target(world: &World, caster: &Player, skill: &Skill, ctrl: bool) -> Result<i32, i16> {
    use server_packets::sm_ids;

    let resolved = match skill.target_type {
        TargetType::Self_ => return Ok(caster.object_id),
        // `Target.java`: the selected target, friend or foe; self allowed
        // (and self skips the LOS check — "you can always target yourself").
        TargetType::Target => {
            let t = caster.target.ok_or(sm_ids::INVALID_TARGET)?;
            if t == caster.object_id {
                return Ok(t);
            }
            t
        }
        // `Enemy.java`/`EnemyOnly.java`: not self, and `isAutoAttackable ||
        // forceUse` — monsters are auto-attackable; players carry no PvP
        // flag/karma yet, so hitting one still needs ctrl (force-use).
        TargetType::Enemy | TargetType::EnemyOnly => {
            let t = caster.target.ok_or(sm_ids::INVALID_TARGET)?;
            if t == caster.object_id {
                return Err(sm_ids::INVALID_TARGET);
            }
            let auto_attackable = world
                .npcs
                .get(&t)
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
    if !world.geo.can_see_target(caster.x, caster.y, caster.z, tx, ty, tz) {
        return Err(sm_ids::CANNOT_SEE_TARGET);
    }
    Ok(resolved)
}

/// Position + liveness of a castable target, whichever registry it lives in
/// (plus its collision radius for the range gates).
pub(crate) fn target_state(world: &World, object_id: i32) -> Option<(i32, i32, i32, bool)> {
    if let Some(p) = world.players.get(&object_id) {
        return Some((p.x, p.y, p.z, p.dead));
    }
    let n = world.npcs.get(&object_id)?;
    Some((n.x, n.y, n.z, n.dead))
}

/// `Util.checkIfInRange` over any two castable actors.
fn in_cast_range(world: &World, caster: &Player, target_oid: i32, range: i32, include_z: bool) -> bool {
    let Some((tx, ty, tz, _)) = target_state(world, target_oid) else { return false };
    let target_radius = world
        .npcs
        .get(&target_oid)
        .and_then(|n| n.template(world))
        .map(|t| t.collision_radius)
        .or_else(|| world.players.get(&target_oid).map(|p| p.collision_radius))
        .unwrap_or(0.0);
    let (dx, dy, dz) = ((tx - caster.x) as f64, (ty - caster.y) as f64, (tz - caster.z) as f64);
    let d2 = dx * dx + dy * dy + if include_z { dz * dz } else { 0.0 };
    let reach = range as f64 + caster.collision_radius + target_radius;
    d2 <= reach * reach
}


/// Port of `clientpackets/RequestMagicSkillUse.runImpl` + `Player.useMagic`'s
/// guards + `SkillCaster.castSkill`/`checkUseConditions`. Narrowing: no
/// queued skills, no follow-into-range (an out-of-range cast just fails), no
/// mute/sit/fake-death states (none exist), toggles and non-single targeting
/// still silently ignored.
pub(crate) fn handle_request_magic_skill_use(world: &mut World, client_id: u32, body: &[u8]) {
    use server_packets::{sm_ids, SmParam};

    let Some(pkt) = cp::RequestMagicSkillUse::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();

    let Some(player) = world.players.get(&object_id) else { return };
    // The dead can't cast (`checkUseConditions` → `isDead`).
    if player.dead {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    // Unknown skill → ActionFailed (RequestMagicSkillUse.runImpl).
    let Some(&skill_level) = player.skills.get(&pkt.magic_id) else {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    };
    let Some(skill) = world.data.skill_data.get(pkt.magic_id, skill_level).cloned() else { return };

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
    if let Some(&crate::model::SkillReuse { until_tick, total_ms, .. }) = player.reuses.get(&skill.reuse_key()) {
        if until_tick > world.tick {
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
            return;
        }
    }

    // Single NORMAL casting slot busy (`checkUseConditions`).
    if player.cast.is_some() {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }

    // MP/HP prechecks (`checkUseConditions`).
    if player.cur_mp < (skill.mp_initial_consume + skill.mp_consume) as f64 {
        send_sm_and_action_failed(world, client_id, sm_ids::NOT_ENOUGH_MP, &[]);
        return;
    }
    if player.cur_hp <= skill.hp_consume as f64 {
        send_sm_and_action_failed(world, client_id, sm_ids::NOT_ENOUGH_HP, &[]);
        return;
    }

    let target_oid = match resolve_cast_target(world, player, &skill, pkt.ctrl_pressed) {
        Ok(oid) => oid,
        Err(sm_id) => {
            send_sm_and_action_failed(world, client_id, sm_id, &[]);
            return;
        }
    };

    // Cast-range gate (`SkillCaster.castSkill`). Java returns null and lets
    // the AI walk into range; there's no follow-to-cast yet, so just unstick
    // the client (narrowing note).
    if skill.cast_range > 0 && target_oid != object_id && !in_cast_range(world, player, target_oid, skill.cast_range, false)
    {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }

    start_casting(world, client_id, object_id, &skill, target_oid);
}

/// Port of `SkillCaster.startCasting` (phase 0). Narrowing: no skill mastery,
/// no `MAGIC_REUSE_RATE` stat (reuse = the skill's `reuseDelay`), no item/
/// fame/clan-rep consumes, no `stopEffectsOnAction`, no `MoveToPawn`
/// cosmetic (only `ExRotation` for target facing).
pub(crate) fn start_casting(world: &mut World, client_id: u32, object_id: i32, skill: &Skill, target_oid: i32) {
    use server_packets::{sm_ids, SmParam};

    let Some(player) = world.players.get(&object_id) else { return };
    let (hit_ms, cancel_ms, cool_ms) = formulas::calc_cast_times(player, &world.data, skill);
    let displayed_cast_time = hit_ms + cancel_ms;

    // Register the reuse (skipped when trivially short, like Java's `> 10`),
    // under the shared group id when the skill has one.
    if skill.reuse_delay > 10 {
        let until_tick = world.tick + ms_to_ticks(skill.reuse_delay);
        if let Some(player) = world.players.get_mut(&object_id) {
            player.reuses.insert(
                skill.reuse_key(),
                crate::model::SkillReuse { skill_level: skill.level, until_tick, total_ms: skill.reuse_delay },
            );
        }
    }

    // Stop movement (`clientStopMoving`) — the client freezes on its own; the
    // broadcast pins the position for everyone else.
    let was_moving = world.players.get(&object_id).is_some_and(|p| p.move_data.is_some());
    if was_moving {
        if let Some(player) = world.players.get_mut(&object_id) {
            player.move_data = None;
        }
        let p = &world.players[&object_id];
        broadcast_including_self(world, object_id, &server_packets::stop_move(object_id, p.x, p.y, p.z, p.heading));
    }

    // Face the target (Java: `setHeading` + broadcast `ExRotation`).
    if target_oid != object_id {
        let Some((tx, ty, _, _)) = target_state(world, target_oid) else { return };
        let (dx, dy) = {
            let p = &world.players[&object_id];
            ((tx - p.x) as f64, (ty - p.y) as f64)
        };
        let heading = crate::model::movement::calculate_heading(dx, dy);
        if let Some(player) = world.players.get_mut(&object_id) {
            player.heading = heading;
        }
        let p = &world.players[&object_id];
        broadcast_including_self(world, object_id, &crate::network::enter_world::ex_rotation(p));
    }

    // Initial MP consume + StatusUpdate (re-checked here in Java too).
    let mut mp_update = None;
    if let Some(player) = world.players.get_mut(&object_id) {
        if skill.mp_initial_consume > 0 {
            if player.cur_mp < skill.mp_initial_consume as f64 {
                send_sm_and_action_failed(world, client_id, sm_ids::NOT_ENOUGH_MP, &[]);
                return;
            }
            player.cur_mp -= skill.mp_initial_consume as f64;
            mp_update = Some(player.cur_mp as i32);
        }
    }
    if let Some(mp) = mp_update {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::status_update(object_id, &[(server_packets::status_update_type::CUR_MP, mp)]));
        }
    }

    // Broadcast the cast start, then the caster-only YOU_USE_S1 + cast bar.
    {
        let Some((tx, ty, tz, _)) = target_state(world, target_oid) else { return };
        let caster = &world.players[&object_id];
        broadcast_including_self(
            world,
            object_id,
            &server_packets::magic_skill_use(
                caster,
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
            &[SmParam::SkillName { id: skill.id, level: skill.level }],
        ));
        cs.send(server_packets::setup_gauge(object_id, 0, displayed_cast_time));
    }

    let cast_seq = {
        let Some(player) = world.players.get_mut(&object_id) else { return };
        player.cast_seq += 1;
        player.cast = Some(crate::model::CastState {
            skill_id: skill.id,
            skill_level: skill.level,
            target_object_id: target_oid,
            seq: player.cast_seq,
            launched: false,
            cancel_ms,
            cool_ms,
        });
        player.cast_seq
    };
    world
        .scheduler
        .schedule(world.tick + ms_to_ticks(hit_ms), ScheduledTask::SkillLaunch { player_object_id: object_id, cast_seq });
}

/// A cast task's `CastState` if it's still the live one (seq matches);
/// stale/aborted tasks resolve to `None` and no-op.
pub(crate) fn live_cast(world: &World, player_object_id: i32, cast_seq: u64) -> Option<crate::model::CastState> {
    world
        .players
        .get(&player_object_id)?
        .cast
        .clone()
        .filter(|c| c.seq == cast_seq)
}

/// Port of `SkillCaster.launchSkill` (phase 1): re-check `effectRange`
/// (failure → SM 748 + a *quiet* stop, `stopCasting(false)` — Java only
/// sends `MagicSkillCanceled` on explicit aborts), broadcast
/// `MagicSkillLaunched`, mark the cast unabortable, schedule the finish.
pub(crate) fn handle_skill_launch(world: &mut World, player_object_id: i32, cast_seq: u64) {
    use server_packets::sm_ids;

    let Some(cast) = live_cast(world, player_object_id, cast_seq) else { return };
    let Some(skill) = world.data.skill_data.get(cast.skill_id, cast.skill_level).cloned() else { return };

    // Target gone (logged off / decayed) → quiet stop, like Java's dead-ref
    // return.
    if target_state(world, cast.target_object_id).is_none() {
        if let Some(player) = world.players.get_mut(&player_object_id) {
            player.cast = None;
        }
        return;
    }

    if skill.effect_range > 0 && cast.target_object_id != player_object_id {
        let caster = &world.players[&player_object_id];
        if !in_cast_range(world, caster, cast.target_object_id, skill.effect_range, true) {
            if let Some(client_id) = client_for_player(world, player_object_id) {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(server_packets::system_message_with(sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED, &[]));
                }
            }
            if let Some(player) = world.players.get_mut(&player_object_id) {
                player.cast = None;
            }
            return;
        }
    }

    broadcast_including_self(
        world,
        player_object_id,
        &server_packets::magic_skill_launched(player_object_id, skill.id, skill.level, &[cast.target_object_id]),
    );

    if let Some(player) = world.players.get_mut(&player_object_id) {
        if let Some(c) = player.cast.as_mut() {
            c.launched = true;
        }
    }
    world.scheduler.schedule(
        world.tick + ms_to_ticks(cast.cancel_ms),
        ScheduledTask::SkillFinish { player_object_id, cast_seq },
    );
}

/// Port of `SkillCaster.finishSkill` + `callSkill` (phase 2): re-check and
/// consume MP/HP (failure → SM + quiet stop, no cancel packet), apply the
/// skill's effects, then either free the cast slot or hold it for
/// `_coolTime`.
pub(crate) fn handle_skill_finish(world: &mut World, player_object_id: i32, cast_seq: u64) {
    use server_packets::sm_ids;

    let Some(cast) = live_cast(world, player_object_id, cast_seq) else { return };
    let Some(skill) = world.data.skill_data.get(cast.skill_id, cast.skill_level).cloned() else { return };
    let client_id = client_for_player(world, player_object_id);

    // MP/HP re-check at landing (no refund of the initial consume).
    let insufficient_mp = world.players[&player_object_id].cur_mp < skill.mp_consume as f64;
    let insufficient_hp = world.players[&player_object_id].cur_hp <= skill.hp_consume as f64;
    if insufficient_mp || insufficient_hp {
        if let Some(client_id) = client_id {
            let sm = if insufficient_mp { sm_ids::NOT_ENOUGH_MP } else { sm_ids::NOT_ENOUGH_HP };
            send_sm_and_action_failed(world, client_id, sm, &[]);
        }
        if let Some(player) = world.players.get_mut(&player_object_id) {
            player.cast = None;
        }
        return;
    }

    let mut updates = Vec::new();
    if let Some(player) = world.players.get_mut(&player_object_id) {
        if skill.mp_consume > 0 {
            player.cur_mp = (player.cur_mp - skill.mp_consume as f64).max(0.0);
            updates.push((server_packets::status_update_type::CUR_MP, player.cur_mp as i32));
        }
        if skill.hp_consume > 0 {
            player.cur_hp = (player.cur_hp - skill.hp_consume as f64).max(0.0);
            updates.push((server_packets::status_update_type::CUR_HP, player.cur_hp as i32));
        }
    }
    if !updates.is_empty() {
        if let Some(client_id) = client_id {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::status_update(player_object_id, &updates));
            }
        }
    }

    // `callSkill` → effect application, if the target is still around.
    if target_state(world, cast.target_object_id).is_some() {
        apply_skill_effects(world, player_object_id, cast.target_object_id, &skill);
    }

    // Hold the cast slot for the cool phase (`stopCasting(false)` after
    // `_coolTime`), freeing inline when there's nothing to wait out.
    let cool_ticks = ms_to_ticks(cast.cool_ms);
    if cool_ticks == 0 {
        if let Some(player) = world.players.get_mut(&player_object_id) {
            player.cast = None;
        }
    } else {
        world
            .scheduler
            .schedule(world.tick + cool_ticks, ScheduledTask::CastEnd { player_object_id, cast_seq });
    }
}

/// `SkillCaster.run`'s terminal `stopCasting(false)` — the cool phase ended.
pub(crate) fn handle_cast_end(world: &mut World, player_object_id: i32, cast_seq: u64) {
    if live_cast(world, player_object_id, cast_seq).is_none() {
        return;
    }
    if let Some(player) = world.players.get_mut(&player_object_id) {
        player.cast = None;
    }
}

/// Port of `Creature.abortCast` → `stopCasting(aborted == true)`: only casts
/// that haven't launched can be aborted; broadcast `MagicSkillCanceled` (self
/// included, to stop the animation) + `ActionFailed` to the caster. The
/// already-scheduled phase tasks go stale via the seq mismatch.
pub(crate) fn abort_cast(world: &mut World, object_id: i32) {
    let abortable = world.players.get(&object_id).is_some_and(|p| p.cast.as_ref().is_some_and(|c| !c.launched));
    if !abortable {
        return;
    }
    if let Some(player) = world.players.get_mut(&object_id) {
        player.cast = None;
    }
    broadcast_including_self(world, object_id, &server_packets::magic_skill_canceld(object_id));
    if let Some(client_id) = client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
    }
}

