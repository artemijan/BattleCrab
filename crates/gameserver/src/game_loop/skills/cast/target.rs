//! Target resolution: `Skill.getTarget` + the targethandler scripts as a
//! static match, plus the target-state and cast-range checks.

use super::*;
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
            if !world
                .geo
                .can_see_target(caster_pos.x, caster_pos.y, caster_pos.z, gp.x, gp.y, gp.z)
            {
                return Err(sm_ids::CANNOT_SEE_TARGET);
            }
            // `ZoneRegion.checkEffectRangeInsidePeaceZone`: a bad ground cast
            // is refused when its effect circle would clip a peace zone —
            // Java samples five points (centre + N/S/E/W at `effectRange`).
            if skill.is_bad() {
                let r = skill.effect_range;
                let clips_peace =
                    [(0, 0), (0, r), (0, -r), (r, 0), (-r, 0)]
                        .iter()
                        .any(|&(ox, oy)| {
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
            let is_monster = npc_template(world, t).is_some_and(|tm| tm.is_auto_attackable());
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
        // `NpcBody.java`: a dead NPC corpse. The target handler itself only
        // requires a dead NPC — the spoil/owner gate is `OpSweeper`, a
        // skill-condition that on this dist only Sweeper 42 carries (Sweeper
        // Festival 444 has it stripped and is in no skill tree anyway). The
        // learnable corpse skills sharing this target type — Life Scavenge 46,
        // Corpse Plague 103, Corpse Life Drain 1151, Corpse Burst 1155 — carry
        // no condition and must cast on any dead NPC, spoiled or not. Since
        // there's no condition-handler layer, the `OpSweeper` gate keys off
        // the `Sweeper` effect (same carrier set on this dist) so a failed
        // sweep still blocks the whole cast (no MP spent, no `ConsumeBody`
        // corpse-decay), matching Java's `canUse` refusal.
        TargetType::NpcBody => {
            let t = caster_target.ok_or(sm_ids::INVALID_TARGET)?;
            let is_dead_npc = world
                .objects
                .get_component::<crate::model::npc::Npc>(&t)
                .is_some()
                && world
                    .objects
                    .get_component::<Vitals>(&t)
                    .is_some_and(|v| v.dead);
            if !is_dead_npc {
                return Err(sm_ids::INVALID_TARGET);
            }
            let is_sweep = skill
                .effects
                .iter()
                .any(|e| matches!(e, SkillEffect::Sweeper));
            if is_sweep {
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
                // `isOldCorpse(sweeper, CORPSE_CONSUME_SKILL_ALLOWED_TIME_BEFORE_DECAY)`
                // — a corpse with less than that long left to live is too far
                // gone to sweep, so a sweeper who arrives at the last moment
                // is refused rather than paying MP for nothing.
                let decay_at = world
                    .objects
                    .get_component::<crate::model::npc::Npc>(&t)
                    .map(|n| n.decay_at_tick)
                    .unwrap_or(0);
                // `DecayTaskManager.getRemainingTime` answers `Long.MAX_VALUE`
                // for a creature it has no schedule for, so a corpse with no
                // decay pending is never "old". `0` is that same "unscheduled"
                // state here.
                let ticks_left = if decay_at == 0 {
                    u64::MAX
                } else {
                    decay_at.saturating_sub(world.tick)
                };
                let min_ticks = (world
                    .cfg
                    .npc
                    .corpse_consume_skill_allowed_time_before_decay
                    .max(0)
                    / 100) as u64;
                if ticks_left < min_ticks {
                    return Err(sm_ids::THE_CORPSE_IS_TOO_OLD_THE_SKILL_CANNOT_BE_USED);
                }
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
                || world
                    .objects
                    .has_component::<crate::model::components::PetOf>(&t);
            let is_dead = world
                .objects
                .get_component::<Vitals>(&t)
                .is_some_and(|v| v.dead);
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
        TargetType::Summon => crate::game_loop::servitor::servitor_of(world, caster.object_id)
            .ok_or(sm_ids::INVALID_TARGET)?,
        // `targethandlers/Others.java` — the selection, with one rule: it may
        // not be the caster, and Java refuses with its own message rather than
        // the generic invalid-target one.
        TargetType::Others => {
            let t = caster_target.ok_or(sm_ids::THAT_IS_AN_INCORRECT_TARGET)?;
            if t == caster.object_id {
                return Err(sm_ids::YOU_CANNOT_USE_THIS_ON_YOURSELF);
            }
            t
        }
        // `targethandlers/DoorTreasure.java` — the selection itself is the
        // whole validation: a door or a chest passes, anything else (including
        // no selection) is `THAT_IS_AN_INCORRECT_TARGET`. Unlock is the only
        // learnable skill that uses it.
        TargetType::DoorTreasure => {
            let t = caster_target.ok_or(sm_ids::THAT_IS_AN_INCORRECT_TARGET)?;
            let is_door = world.objects.has_component::<crate::model::door::Door>(&t);
            if !is_door && !target_is_chest(world, t) {
                return Err(sm_ids::THAT_IS_AN_INCORRECT_TARGET);
            }
            t
        }
        // `targethandlers/OwnerPet.java` — `creature.getActingPlayer()`, which
        // for a **player** caster is the player themselves. No skill on this
        // dist puts OWNER_PET in a player's hands (every carrier is a pet
        // skill, resolved on the servitor path in `npc_cast`), but Java would
        // self-target here rather than refuse, so this does too.
        TargetType::OwnerPet => caster.object_id,
        TargetType::Other => return Err(sm_ids::INVALID_TARGET),
    };
    let (.., target_dead) = target_state(world, resolved).ok_or(sm_ids::INVALID_TARGET)?;
    // A corpse (`NPC_BODY`) is *supposed* to be dead; `EnemyNot` explicitly
    // "works on dead targets... as well" (a heal landing on a fresh corpse
    // ahead of a resurrection); every other target type rejects the dead.
    if target_dead
        && !matches!(
            skill.target_type,
            TargetType::NpcBody | TargetType::PcBody | TargetType::EnemyNot
        )
    {
        return Err(sm_ids::INVALID_TARGET);
    }
    finalize_target(world, caster.object_id, resolved, skill)
}

/// The closing gates of `Target.java`/`Enemy.java`/`EnemyOnly.java`, shared
/// verbatim by the player resolver above and the NPC resolver
/// (`npc::cast::resolve_npc_cast_target`):
///
/// - "Geodata check when character is within range" — `GeoEngine.canSeeTarget`
///   → CANNOT_SEE_TARGET, short-circuited to pass when the target is a door
///   (`target.isDoor() || canSeeTarget(...)`): a closed siege gate occludes
///   the ray to its own centre, so without this a gate could never be nuked.
/// - `Enemy`/`EnemyOnly`'s peace-zone refusal — "cannot be used by playables
///   on playables in peace zone" — SM 2167, after the LOS check, matching the
///   handlers' order.
///
/// `Err` carries the player path's system message; the NPC path drops it with
/// `.ok()`.
pub(crate) fn finalize_target(
    world: &World,
    caster_oid: i32,
    resolved: i32,
    skill: &Skill,
) -> Result<i32, i16> {
    use server_packets::sm_ids;
    let (Some(from), Some(to)) = (
        maybe_position(world, caster_oid),
        maybe_position(world, resolved),
    ) else {
        return Err(sm_ids::CANNOT_SEE_TARGET);
    };
    let target_is_door = world
        .objects
        .has_component::<crate::model::door::Door>(&resolved);
    if !target_is_door
        && !world
            .geo
            .can_see_target(from.x, from.y, from.z, to.x, to.y, to.z)
    {
        return Err(sm_ids::CANNOT_SEE_TARGET);
    }
    if matches!(skill.target_type, TargetType::Enemy | TargetType::EnemyOnly)
        && crate::game_loop::zones::is_inside_peace_zone(world, caster_oid, resolved)
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
    if let Some(door) = world
        .objects
        .get_component::<crate::model::door::Door>(&object_id)
    {
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
            if world
                .objects
                .has_component::<crate::model::door::Door>(&target_oid)
            {
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
