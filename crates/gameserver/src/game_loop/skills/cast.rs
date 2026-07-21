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
use crate::model::skill::{OperateType, Skill, SkillEffect, TargetType};
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
    // Players are given `Reuses` at load; **NPCs were not**, so this write was
    // a silent no-op for them and `npc_cast`'s check — which treats an absent
    // component as "ready" — always passed. NPC skill cooldowns therefore
    // never applied at all: a mob could re-cast as fast as its AI ticked.
    //
    // Attached on first use rather than at spawn, so only NPCs that actually
    // cast pay for the map (this world holds ~34.9k of them, the vast majority
    // of which never cast anything).
    if world.objects.get_component::<crate::model::components::Reuses>(&object_id).is_none() {
        world.objects.add_components(&object_id, crate::model::components::Reuses::default());
    }
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
    shift: bool,
) -> Result<i32, i16> {
    use server_packets::sm_ids;

    let resolved = match skill.target_type {
        // `None.java`: returns the caster outright. Unlike `Self`, there is no
        // peace-zone gate — a toggle is not an attack on anyone.
        TargetType::None_ => return Ok(caster.object_id),
        // `Ground.java`: validate the stored ex-0x41 world position and return
        // the caster as sentinel — the POINT_BLANK sweep re-reads the point.
        TargetType::Ground => {
            // Player-only in Java (`creature.isPlayer()`); NPC casters fall
            // through to null there, and no NPC cast path reaches here.
            let Some(gp) = world
                .objects
                .get_component::<crate::model::components::GroundSkillTarget>(&caster.object_id)
                .copied()
            else {
                // `use_magic_on` already refused the no-position case; this
                // guards the tick's quiet re-resolve.
                return Err(sm_ids::INVALID_TARGET);
            };
            // `dontMove`: shift-click refuses beyond `castRange + collision
            // radius` (2D) instead of walking — Java returns null with no
            // message; the port sends the same SM 748 the shift-refusal of a
            // creature target uses (visible refusal beats a silent one, and
            // the general out-of-range branch below never fires for a
            // self-sentinel target).
            if shift {
                let reach = skill.cast_range as f64
                    + world
                        .objects
                        .get_component::<Collision>(&caster.object_id)
                        .map(|c| c.radius)
                        .unwrap_or(0.0);
                let (dx, dy) = ((gp.x - caster_pos.x) as f64, (gp.y - caster_pos.y) as f64);
                if dx * dx + dy * dy > reach * reach {
                    return Err(sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED);
                }
            }
            if !world.geo.can_see_target(caster_pos.x, caster_pos.y, caster_pos.z, gp.x, gp.y, gp.z) {
                return Err(sm_ids::CANNOT_SEE_TARGET);
            }
            // `ZoneRegion.checkEffectRangeInsidePeaceZone`: a bad ground cast
            // is refused when its effect circle would clip a peace zone —
            // Java samples five points (centre + N/S/E/W at `effectRange`).
            if skill.is_bad() {
                let r = skill.effect_range;
                let clips_peace = [(0, 0), (0, r), (0, -r), (r, 0), (-r, 0)].iter().any(|&(ox, oy)| {
                    world
                        .data
                        .zone_data
                        .zones_at(gp.x + ox, gp.y + oy, gp.z)
                        .any(|z| z.kind == crate::data::zone_data::ZoneKind::Peace)
                });
                if clips_peace {
                    return Err(sm_ids::YOU_CANNOT_USE_SKILLS_THAT_MAY_HARM_OTHER_PLAYERS_IN_HERE);
                }
            }
            return Ok(caster.object_id);
        }
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
            // Casting on a monster requires force (Ctrl). Java's `Target.java`
            // is permissive and leans on the client to demand Ctrl for a good
            // skill on a hostile creature; we enforce it server-side so buffing
            // a mob needs a deliberate force-pick, matching the real client.
            let is_monster = world
                .objects
                .get_component::<crate::model::npc::Npc>(&t)
                .and_then(|n| n.template(world))
                .is_some_and(|tm| tm.is_auto_attackable());
            if is_monster && !ctrl {
                return Err(sm_ids::INVALID_TARGET);
            }
            t
        }
        // `Enemy.java`/`EnemyOnly.java`: not self, and `isAutoAttackable ||
        // forceUse`. Monsters are always auto-attackable; a player is only when
        // flagged/PK (`isAutoAttackable` relation), so hitting a clean player
        // still needs Ctrl (force-use), but a flagged one doesn't.
        TargetType::Enemy | TargetType::EnemyOnly => {
            let t = caster_target.ok_or(sm_ids::INVALID_TARGET)?;
            if t == caster.object_id {
                return Err(sm_ids::INVALID_TARGET);
            }
            // `isAutoAttackable` over players, monsters, and — during a siege —
            // castle doors, towers, HQ flags and stationed guards. A clean
            // target still needs Ctrl (force-use).
            let auto_attackable =
                crate::game_loop::target::is_auto_attackable(world, caster.object_id, t);
            if !auto_attackable && !ctrl {
                return Err(sm_ids::INVALID_TARGET);
            }
            t
        }
        // `EnemyNot.java`: "any friendly selected target" — the exact inverse
        // of `Enemy`/`EnemyOnly`'s gate, self always allowed, and **no**
        // force-use (ctrl) override; a hostile target is refused outright.
        TargetType::EnemyNot => {
            let t = caster_target.ok_or(sm_ids::INVALID_TARGET)?;
            if t == caster.object_id {
                return Ok(t);
            }
            if crate::game_loop::target::is_auto_attackable(world, caster.object_id, t) {
                return Err(sm_ids::INVALID_TARGET);
            }
            t
        }
        // `NpcBody.java`: a dead NPC corpse. Used by the Sweeper family, which
        // also carries the `OpSweeper` skill-condition — since there's no
        // condition-handler layer yet, that gate (dead + spoiled + owner) is
        // enforced here so a failed sweep blocks the whole cast (no MP spent,
        // no `ConsumeBody` corpse-decay), matching Java's `canUse` refusal.
        // TODO(G19): move this into a real skill-condition layer if other
        // `NPC_BODY`/corpse skills (Harvest, corpse-consume) land.
        TargetType::NpcBody => {
            let t = caster_target.ok_or(sm_ids::INVALID_TARGET)?;
            let is_dead_npc = world
                .objects
                .get_component::<crate::model::npc::Npc>(&t)
                .is_some()
                && world.objects.get_component::<Vitals>(&t).is_some_and(|v| v.dead);
            if !is_dead_npc {
                return Err(sm_ids::INVALID_TARGET);
            }
            let spoiler = world
                .objects
                .get_component::<crate::model::npc::Npc>(&t)
                .map(|n| n.spoiler_object_id)
                .unwrap_or(0);
            if spoiler == 0 {
                return Err(sm_ids::SWEEPER_FAILED_TARGET_NOT_SPOILED);
            }
            if spoiler != caster.object_id
                && !crate::game_loop::party::same_party(world, caster.object_id, spoiler)
            {
                return Err(sm_ids::THERE_ARE_NO_PRIORITY_RIGHTS_ON_A_SWEEPER);
            }
            t
        }
        // `targethandlers/PcBody.java`: `if (!selectedTarget.isPlayer() &&
        // !selectedTarget.isPet())` — a dead player **or a dead pet**, which
        // is what a resurrection is cast on. The pet branch was missing, so a
        // dead pet could not be targeted at all.
        TargetType::PcBody => {
            let t = caster_target.ok_or(sm_ids::INVALID_TARGET)?;
            let is_revivable = world.objects.has_component::<Player>(&t)
                || world.objects.has_component::<crate::model::components::PetOf>(&t);
            let is_dead = world.objects.get_component::<Vitals>(&t).is_some_and(|v| v.dead);
            if !is_revivable || !is_dead {
                return Err(sm_ids::INVALID_TARGET);
            }
            t
        }
        // `targethandlers/Summon.java` — the caster's own servitor, whatever
        // they currently have selected. This is the whole Summoner support kit
        // (Servitor Heal/Recharge/shields/Haste/Wind Walk/Magic Boost and the
        // four class servitor buffs): 18 learnable skills that resolved to
        // `INVALID_TARGET` before this arm existed.
        TargetType::Summon => {
            super::super::servitor::servitor_of(world, caster.object_id).ok_or(sm_ids::INVALID_TARGET)?
        }
        TargetType::Other => return Err(sm_ids::INVALID_TARGET),
    };
    let (tx, ty, tz, target_dead) = target_state(world, resolved).ok_or(sm_ids::INVALID_TARGET)?;
    // A corpse (`NPC_BODY`) is *supposed* to be dead; `EnemyNot` explicitly
    // "works on dead targets... as well" (a heal landing on a fresh corpse
    // ahead of a resurrection); every other target type rejects the dead.
    if target_dead && !matches!(skill.target_type, TargetType::NpcBody | TargetType::PcBody | TargetType::EnemyNot) {
        return Err(sm_ids::INVALID_TARGET);
    }
    // "Geodata check when character is within range" — every non-self
    // handler ends with `GeoEngine.canSeeTarget` → CANNOT_SEE_TARGET. Java's
    // `canSeeTarget(asker, target)` short-circuits to `true` when the target
    // is a door (GeoEngine.java: `target.isDoor() || canSeeTarget(...)`) — a
    // closed siege gate occludes the ray to its own centre, so without this a
    // gate could never be nuked.
    let target_is_door = world.objects.has_component::<crate::model::door::Door>(&resolved);
    if !target_is_door
        && !world
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
    // Doors carry no `Vitals`/`Npc` — their HP lives on the `Door` component;
    // a breached gate (0 HP) counts as dead, so a skill can't re-hit it.
    if let Some(door) = world.objects.get_component::<crate::model::door::Door>(&object_id) {
        let pos = world.objects.get_component::<Position>(&object_id)?;
        return Some((pos.x, pos.y, pos.z, door.current_hp <= 0));
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
    // A siege door has no `Collision` component — its extent lives in the
    // `DOOR_COLLISION_RADIUS` stand-in that the melee reach/chase geometry
    // (`combatant`) already uses. Match it here so the cast range gate and the
    // walk-to-cast (`chase_target`, also radius 80) agree; otherwise the gate
    // demands the door's polygon *centre* and the chase overshoots almost onto
    // the gate before casting.
    let target_radius = world
        .objects
        .get_component::<Collision>(&target_oid)
        .map(|c| c.radius)
        .unwrap_or_else(|| {
            if world.objects.has_component::<crate::model::door::Door>(&target_oid) {
                crate::game_loop::combat::DOOR_COLLISION_RADIUS
            } else {
                0.0
            }
        });
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

/// Port of `RequestExMagicSkillUseGround` (ex 0x41): store the aimed world
/// position (Java `Player._currentSkillWorldPosition` — never cleared, only
/// overwritten), face it ("normally magicskilluse packet turns char client
/// side but for these skills, it doesn't"), then enter the normal `useMagic`
/// path — the `Ground.java` target leg resolves the caster as sentinel from
/// there.
pub(crate) fn handle_request_magic_skill_use_ground(world: &mut World, client_id: u32, ex_body: &[u8]) {
    let Some(pkt) = cp::RequestExMagicSkillUseGround::read(ex_body) else {
        return;
    };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();
    world.objects.add_components(
        &object_id,
        crate::model::components::GroundSkillTarget { x: pkt.x, y: pkt.y, z: pkt.z },
    );
    if let Some(pos) = world.objects.get_component_mut::<Position>(&object_id) {
        pos.heading = crate::model::movement::calculate_heading(
            (pkt.x - pos.x) as f64,
            (pkt.y - pos.y) as f64,
        );
    }
    if let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() {
        // `Broadcast.toKnownPlayers(player, new ValidateLocation(player))` —
        // bystanders only, the caster's own client already turned.
        crate::game_loop::helpers::broadcast_to_others(
            world,
            object_id,
            &server_packets::validate_location(object_id, pos.x, pos.y, pos.z, pos.heading),
        );
    }
    use_magic(world, client_id, object_id, pkt.skill_id, pkt.ctrl_pressed, pkt.shift_pressed);
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
    // `Creature.isAllSkillsDisabled()` → `hasBlockActions()`: no casting while
    // stunned/asleep/paralyzed. Checked before the skill lookup, like Java's
    // `useMagic` guard order.
    if super::super::abnormal::is_blocked_from_actions(world, object_id) {
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
    // `Player.useMagic`'s toggle branch, ahead of every other check: recasting
    // a live toggle switches it **off** and casts nothing; otherwise a toggle
    // in a group first stops the others in that group, then casts normally.
    //
    // Java's `isNecessaryToggle()` exemption (a toggle that may never be
    // switched off) is not ported — no skill on this dist sets it.
    if skill.operate_type == OperateType::Toggle {
        let already_on = world
            .objects
            .get_component::<crate::model::components::Buffs>(&object_id)
            .is_some_and(|b| b.0.iter().any(|x| x.skill_id == skill.id));
        if already_on {
            super::effects::handle_buff_expire(world, object_id, skill.id);
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::action_failed());
            }
            return;
        }
        if skill.toggle_group_id > 0 {
            // `EffectList.stopAllTogglesOfGroup` — the group is mutually
            // exclusive, so switching this one on drops its siblings.
            let siblings: Vec<i32> = world
                .objects
                .get_component::<crate::model::components::Buffs>(&object_id)
                .map(|b| {
                    b.0.iter()
                        .map(|x| x.skill_id)
                        .filter(|&id| {
                            world
                                .data
                                .skill_data
                                .get(id, 1)
                                .is_some_and(|s| s.toggle_group_id == skill.toggle_group_id)
                        })
                        .collect()
                })
                .unwrap_or_default();
            for sibling in siblings {
                super::effects::handle_buff_expire(world, object_id, sibling);
            }
        }
        // `SkillCaster.run`'s instant-cast short circuit: a toggle is never
        // "launched" — no cast bar, no launch/finish phases, no `Casting`
        // component. It goes straight to `triggerCast`, so the effect is live
        // the moment the client asks for it.
        //
        // Toggles are `targetType NONE` (the caster) on this dist, so the
        // target resolution the phased path does is just `object_id`.
        if !check_skill_reuse(world, client_id, object_id, &skill) {
            return;
        }
        // `MagicSkillUse` with a 0 cast time — the client plays the toggle's
        // animation without drawing a cast bar.
        if let (Some(caster), Some(pos)) = (
            world.objects.get_component::<crate::model::Player>(&object_id),
            world.objects.get_component::<Position>(&object_id).copied(),
        ) {
            let pkt = server_packets::magic_skill_use(
                caster,
                &pos,
                (object_id, pos.x, pos.y, pos.z),
                skill.id,
                skill.level,
                0,
                skill.reuse_delay_group,
                skill.reuse_delay,
            );
            broadcast_including_self(world, object_id, &pkt);
        }
        super::effects::apply_skill_effects(world, object_id, object_id, &skill);
        set_skill_reuse(world, object_id, &skill);
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    } else if !matches!(skill.operate_type, OperateType::Active | OperateType::Channeling) {
        return;
    }
    if skill.target_type == TargetType::Other {
        return;
    }
    // `useMagic`: a GROUND cast with no stored world position (the client
    // always sends ex 0x41 first, which stores one) is refused with a bare
    // ActionFailed — no system message.
    if skill.target_type == TargetType::Ground
        && !world
            .objects
            .has_component::<crate::model::components::GroundSkillTarget>(&object_id)
    {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }

    // `SkillCaster.checkDoCastConditions`' mute checks: a magic skill is
    // refused while silenced, a non-magic one while physically muted. Static
    // skills (`magic_type == 2`) bypass both, as in Java's `!skill.isStatic()`
    // guard.
    if skill.magic_type != 2 {
        let muted = if skill.magic_type == 1 {
            super::super::abnormal::is_muted(world, object_id)
        } else {
            super::super::abnormal::is_physical_muted(world, object_id)
        };
        if muted {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::action_failed());
            }
            return;
        }
    }

    // `ConditionPlayerCanTransform` — the cast-time gate on a `Transformation`
    // skill (the "Transform <Monster>" scroll family). Java also refuses while
    // sitting or registered on an event; neither state is modeled on this port
    // yet (TODO(G19)/TODO(G28)), so only the legs backed by modeled state are
    // ported: already transformed (this port also represents a horse/bike
    // mount as a transform, so `transform_id != 0` covers Java's separate
    // `isMounted()` leg too), in water, and cursed-weapon-equipped.
    if skill.effects.iter().any(|e| matches!(e, SkillEffect::Transform { .. })) {
        use server_packets::sm_ids;
        let (transform_id, cursed_weapon) = world
            .objects
            .get_component::<Player>(&object_id)
            .map_or((0, 0), |p| (p.transform_id, p.cursed_weapon_equipped_id));
        let in_water = world
            .objects
            .get_component::<crate::model::components::Speeds>(&object_id)
            .is_some_and(|s| s.swimming);
        // Java sends a SystemMessage for the transformed/water legs but not the
        // cursed-weapon one (`ConditionPlayerCanTransform`'s final `else`
        // branches fall through with no packet) — cast just silently fails.
        if transform_id != 0 {
            send_sm_and_action_failed(world, client_id, sm_ids::YOU_ALREADY_POLYMORPHED_AND_CANNOT_POLYMORPH_AGAIN, &[]);
            return;
        }
        if in_water {
            send_sm_and_action_failed(world, client_id, sm_ids::YOU_CANNOT_POLYMORPH_INTO_THE_DESIRED_FORM_IN_WATER, &[]);
            return;
        }
        if cursed_weapon != 0 {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::action_failed());
            }
            return;
        }
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
    // Fetched here rather than at the top of the function: the toggle branch
    // above needs `&mut world`, so this borrow must start after it.
    let Some(player) = world.objects.get_component::<crate::model::Player>(&object_id) else {
        return;
    };
    let target_oid =
        match resolve_cast_target(world, player, &caster_pos, caster_target, &skill, ctrl, shift) {
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
    // Reagent gate (`SkillCaster.checkUseConditions`): the skill's
    // `itemConsumeId × itemConsumeCount` must be in inventory — SM 2156.
    // (Java uses a different message for SUMMON-effect skills; that path is
    // G29's.) The consume itself happens at cast start (`start_casting`).
    if skill.item_consume_id > 0 && skill.item_consume_count > 0 {
        let have = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&object_id)
            .map(|inv| inv.count_of(skill.item_consume_id))
            .unwrap_or(0);
        if have < skill.item_consume_count as i64 {
            send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::THERE_ARE_NOT_ENOUGH_NECESSARY_ITEMS_TO_USE_THE_SKILL,
                &[],
            );
            return;
        }
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

    // `SkillCaster`: recharge shots before the cast so the spiritshot bonus is
    // in effect when the effects land (`rechargeShots(useSoulShot, useSpiritShot)`
    // — magic skills use the spiritshot; physical-skill soulshots aren't wired,
    // only the melee auto-attack path charges soulshots).
    if skill.magic_type == 1 {
        crate::game_loop::items::recharge_shots(world, object_id, false, true);
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
    super::super::raid_curse::on_skill_cast_near_raid(world, object_id, skill.is_bad());

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

/// One `SkillChannelizer.run()` tick: MP upkeep (starvation → SM 140 + abort),
/// re-resolve the target and **re-sweep the affect scope** (a mob that walked
/// into the volcano mid-channel burns; one that left stops), then apply the
/// CHANNELING effect scope per target behind Java's `effectRange` + LOS gate.
/// The `channelingSkillId > 0` branch (stacking "channelized" buffs — hero
/// stances 426/427) is TODO(G19); no reachable channeler on this dist uses it.
pub(crate) fn handle_channeling_tick(world: &mut World, player_object_id: i32, cast_seq: u64) {
    use server_packets::sm_ids;

    // Stale guard, like every scheduled cast phase: the tick belongs to one
    // specific cast generation.
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

    // MP upkeep. Java: not enough → SM 140 + `abortCast()`, no reschedule.
    if skill.mp_per_channeling > 0 {
        let Some(vitals) = world.objects.get_component_mut::<Vitals>(&player_object_id) else {
            return;
        };
        if vitals.cur_mp < skill.mp_per_channeling as f64 {
            if let Some(cid) = client_id {
                if let Some(cs) = world.clients.get(&cid) {
                    cs.send(server_packets::system_message_with(
                        sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP,
                        &[],
                    ));
                }
            }
            abort_cast(world, player_object_id);
            return;
        }
        vitals.cur_mp -= skill.mp_per_channeling as f64;
        let mp = vitals.cur_mp as i32;
        if let Some(cid) = client_id {
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(server_packets::status_update(
                    player_object_id,
                    &[(server_packets::status_update_type::CUR_MP, mp)],
                ));
            }
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
        let Some(pos) = world.objects.get_component::<Position>(&player_object_id).copied() else {
            return;
        };
        match resolve_cast_target(world, player, &pos, Some(cast.target_object_id), &skill, false, false) {
            Ok(oid) => oid,
            Err(_) => return, // quiet: skip this tick, keep channeling
        }
    };
    let affected = super::affect::targets_affected(world, player_object_id, target_oid, &skill);
    if affected.is_empty() || skill.channeling_effects.is_empty() {
        return;
    }
    let Some(caster_pos) = world.objects.get_component::<Position>(&player_object_id).copied() else {
        return;
    };
    let scoped = Skill {
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
        let Some(pos) = world.objects.get_component::<Position>(&target).copied() else {
            continue;
        };
        if skill.effect_range > 0 {
            let (dx, dy, dz) = (
                (pos.x - caster_pos.x) as f64,
                (pos.y - caster_pos.y) as f64,
                (pos.z - caster_pos.z) as f64,
            );
            let r = skill.effect_range as f64;
            if dx * dx + dy * dy + dz * dz > r * r {
                continue;
            }
        }
        if !world
            .geo
            .can_see_target(caster_pos.x, caster_pos.y, caster_pos.z, pos.x, pos.y, pos.z)
        {
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
        crate::game_loop::items::recharge_shots(world, player_object_id, false, true);
    }
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

    // `Skill.forEachTargetAffected` — expand the primary target through the
    // skill's affect scope, then run `callSkill` per affected creature. A
    // single-target skill resolves to exactly the primary target, so this is
    // the old path with one extra element in the loop.
    let affected = if target_state(world, cast.target_object_id).is_some() {
        super::affect::targets_affected(world, player_object_id, cast.target_object_id, &skill)
    } else {
        Vec::new()
    };

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
        if let Some(extra) = matchup_effects(world, player_object_id, target_oid, &skill) {
            if !extra.is_empty() {
                let scoped = Skill { effects: extra, ..skill.clone() };
                apply_skill_effects(world, player_object_id, target_oid, &scoped);
            }
        }
        // The hate/PvP consequences are unconditional: the caster still *cast*
        // a bad skill at this target, reflected or not.
        apply_cast_consequences(world, player_object_id, target_oid, &skill);
    }

    // `EffectScope.SELF` — a separate `applyEffects(caster, caster, …)` after
    // the target loop, so a skill can buff its caster while debuffing its
    // target (Blinding Blow 321, Critical Blow 409, Vengeance 368, …). The
    // parser used to read only `<effects>`, so none of these landed.
    if !skill.self_effects.is_empty() {
        let self_skill = Skill { effects: skill.self_effects.clone(), ..skill.clone() };
        apply_skill_effects(world, player_object_id, player_object_id, &self_skill);
    }

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

/// The per-target half of Java's `callSkill`: PvP flagging, monster hate and
/// the AI wake. Split out of [`handle_skill_finish`] when affect scopes landed
/// so every creature an AoE touches gets the same treatment the single target
/// used to get.
/// `Formulas.calcBuffDebuffReflection` — the chance that `target` bounces this
/// skill back at its caster.
///
/// ```java
/// if (!skill.isDebuff() || (skill.getActivateRate() == -1)) return false;
/// return target.getStat().getValue(skill.isMagic() ? REFLECT_SKILL_MAGIC : REFLECT_SKILL_PHYSIC, 0) > Rnd.get(100);
/// ```
///
/// Two gates before the roll: the skill must be a **debuff**, and it must
/// declare an `activateRate` (a skill with the default `-1` — i.e. one that
/// always lands — is never reflected). Which of the two stats is read depends
/// on the *incoming skill's* `isMagic`, not on the defender.
fn calc_buff_debuff_reflection(world: &mut World, target_oid: i32, skill: &Skill) -> bool {
    use crate::model::stats::Stat;
    if !skill.is_debuff || skill.activate_rate == -1 {
        return false;
    }
    let stat = if skill.magic_type == 1 { Stat::ReflectSkillMagic } else { Stat::ReflectSkillPhysic };
    let chance = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&target_oid)
        .and_then(|m| m.add.get(&stat).copied())
        .unwrap_or(0.0);
    if chance <= 0.0 {
        return false;
    }
    chance > world.roll(100) as f64
}

/// Java's PVE/PVP scope selector:
///
/// ```java
/// effector.isPlayable() && effected.isAttackable() ? PVE
///   : effector.isPlayable() && effected.isPlayable() ? PVP : null
/// ```
///
/// Returns `None` when neither applies (an NPC caster, or a player hitting
/// something that is neither attackable nor playable).
fn matchup_effects(world: &World, caster_oid: i32, target_oid: i32, skill: &Skill) -> Option<Vec<SkillEffect>> {
    // `isPlayable()` — a player (summons are TODO(G29)).
    if !world.objects.has_component::<Player>(&caster_oid) {
        return None;
    }
    if world.objects.has_component::<Player>(&target_oid) {
        return Some(skill.pvp_effects.clone());
    }
    let attackable = world
        .objects
        .get_component::<crate::model::npc::Npc>(&target_oid)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.is_attackable_class());
    attackable.then(|| skill.pve_effects.clone())
}

fn apply_cast_consequences(world: &mut World, player_object_id: i32, target_oid: i32, skill: &Skill) {
    let target_is_player = world.objects.has_component::<Player>(&target_oid);
    // Monster proxy: an NPC whose template is auto-attackable (same test the
    // targeting code uses for "is this a monster").
    let target_is_monster = !target_is_player
        && world
            .objects
            .get_component::<crate::model::npc::Npc>(&target_oid)
            .and_then(|n| n.template(world))
            .is_some_and(|t| t.is_auto_attackable());
    if skill.is_bad() {
        // Bad skill on a player → flag the caster against that target
        // (`updatePvPStatus(target)`). Monsters take hate + an AI wake, no flag.
        if target_is_player {
            crate::game_loop::pvp::update_pvp_status_target(world, player_object_id, target_oid);
        } else if target_is_monster {
            // `callSkill`: a bad skill on an attackable *always* adds hate
            // (`addDamageHate(caster, 0, -effectPoint)`) and wakes its AI
            // (`notifyEvent(EVT_ATTACKED, caster)`), right after `activateSkill`
            // and **independent of whether the effects landed**. That's why a
            // resisted or otherwise no-op debuff still makes the mob retaliate:
            // the wake belongs here in the `callSkill` analog, not in the effect
            // handlers (those only wake on damage / spoil, so a pure or resisted
            // debuff would never aggro). `npc_wake_on_attacked` is the EVT_ATTACKED
            // primitive (hate += 1 + switch to the attack intention); the explicit
            // `-effectPoint` hate is added on top, matching `addDamageHate`
            // (`-effect_point` is positive since a bad skill has `effect_point < 0`).
            // TODO(G16): Java skips the wake when the skill `hasEffectType(HATE)`
            // (aggro-reduction skills manage their own hate); no HATE effect is
            // modeled yet, so every bad skill wakes.
            crate::game_loop::combat::npc_wake_on_attacked(world, target_oid, player_object_id);
            let hate = (-skill.effect_point) as f64;
            if hate != 0.0 {
                if let Some(aggro) =
                    world.objects.get_component_mut::<crate::model::npc::AggroList>(&target_oid)
                {
                    aggro.0.entry(player_object_id).or_default().hate += hate;
                }
            }
        }
    } else if target_oid != player_object_id {
        // Good/support skill (not self-cast): "supporting monsters or players
        // results in pvpflag" — buffing a monster, or a flagged/PK player,
        // self-flags the caster (`updatePvPStatus()`).
        let target_is_flagged = world
            .objects
            .get_component::<crate::model::components::PvpState>(&target_oid)
            .is_some_and(|s| s.flag > 0)
            || world
                .objects
                .get_component::<Player>(&target_oid)
                .is_some_and(|p| p.reputation < 0);
        let flag_self = (skill.effect_point > 0 && target_is_monster)
            || (target_is_player && target_is_flagged);
        if flag_self {
            crate::game_loop::pvp::update_pvp_status(world, player_object_id);
        }
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

/// The `abortCast()` inside `Creature.teleToLocation`, which resolves its
/// caster through `SkillCaster.canAbortCast` — and that is *not* the phase
/// check its comment claims. It is literally
/// `getCaster().getTarget() == null` (`SkillCaster.java:940`), so a teleport
/// cancels the cast exactly while the caster has nothing selected.
///
/// [`abort_cast`]'s `!launched` guard models the other abort paths and is
/// deliberately not reused: a teleport effect (`Escape`, `Recall`) fires from
/// the *finish* phase, when `launched` is already true, so that guard would
/// swallow the `MagicSkillCanceled`. That packet is the only thing that stops
/// the cast animation client-side — without it the escape FX keeps playing at
/// the destination until the client's own skill duration elapses (5 minutes
/// for skill 2099), long after `/unstuck` already teleported the player.
pub(crate) fn abort_cast_on_teleport(world: &mut World, object_id: i32) {
    if !world.objects.has_component::<Casting>(&object_id) {
        return;
    }
    let has_target = world
        .objects
        .get_component::<crate::model::components::TargetRef>(&object_id)
        .is_some_and(|t| t.0.is_some());
    if has_target {
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
