//! The "player clicked a skill" pipeline: the two `RequestMagicSkillUse`
//! packets and the `use_magic_on` validation gauntlet.

use super::check_skill_reuse;
use super::in_cast_range;
use super::known_skill_level;
use super::resolve_cast_target;
use super::set_skill_reuse;
use super::start_casting;
use super::target_state;
use crate::game_loop::helpers;
use crate::game_loop::helpers::maybe_position;
use crate::model::components;

use crate::game_loop::skills::effects::apply_skill_effects;
use crate::model::Player;

use crate::model::skill::OperateType;
use crate::model::skill::TargetType;
use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::world::World;
/// Port of `clientpackets/RequestMagicSkillUse.runImpl`: parse and hand to
/// `use_magic`.
pub(crate) fn handle_request_magic_skill_use(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestMagicSkillUse::read(body) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    use_magic(
        world,
        client_id,
        object_id,
        pkt.magic_id,
        pkt.ctrl_pressed,
        pkt.shift_pressed,
    );
}

/// `World.getVisibleObjectsInRange(caster, Npc.class, range)` filtered to the
/// condition's id list — the sweep half of `OpExistNpcSkillCondition`.
pub(crate) fn op_exist_npc_around(
    world: &World,
    caster_oid: i32,
    cond: &crate::model::skill::OpExistNpcCondition,
) -> bool {
    let Some(region) = helpers::region_cell_of(world, caster_oid) else {
        return false;
    };
    let Some(origin) = maybe_position(world, caster_oid) else {
        return false;
    };
    let range = cond.range as f64;
    world.npcs_visible_from(region).into_iter().any(|oid| {
        let listed = world
            .objects
            .get_component::<crate::model::npc::Npc>(&oid)
            .is_some_and(|n| cond.npc_ids.contains(&n.npc_id));
        listed
            && crate::geo::distance::within_3d_xyz(world, oid, origin.x, origin.y, origin.z, range)
    })
}

/// Port of `RequestExMagicSkillUseGround` (ex 0x41): store the aimed world
/// position (Java `Player._currentSkillWorldPosition` — never cleared, only
/// overwritten), face it ("normally magicskilluse packet turns char client
/// side but for these skills, it doesn't"), then enter the normal `useMagic`
/// path — the `Ground.java` target leg resolves the caster as sentinel from
/// there.
pub(crate) fn handle_request_magic_skill_use_ground(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(pkt) = cp::RequestExMagicSkillUseGround::read(ex_body) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    world.objects.add_components(
        &object_id,
        crate::model::components::GroundSkillTarget {
            x: pkt.x,
            y: pkt.y,
            z: pkt.z,
        },
    );
    if let Some(pos) = world
        .objects
        .get_component_mut::<components::Position>(&object_id)
    {
        pos.heading = crate::model::movement::calculate_heading(
            (pkt.x - pos.x) as f64,
            (pkt.y - pos.y) as f64,
        );
    }
    if let Some(pos) = maybe_position(world, object_id) {
        // `Broadcast.toKnownPlayers(player, new ValidateLocation(player))` —
        // bystanders only, the caster's own client already turned.
        crate::game_loop::helpers::broadcast_to_others(
            world,
            object_id,
            &server_packets::validate_location(object_id, pos.x, pos.y, pos.z, pos.heading),
        );
    }
    use_magic(
        world,
        client_id,
        object_id,
        pkt.skill_id,
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

    // The dead can't cast (`checkUseConditions` → `isDead`).
    if helpers::is_dead(world, object_id) {
        helpers::send_action_failed(world, client_id);
        return;
    }
    // `Creature.isAllSkillsDisabled()` — `_allSkillsDisabled ||
    // hasBlockActions()`: no casting while stunned/asleep/paralyzed, or while
    // a script has locked skills outright (the TvT freeze). Checked before the
    // skill lookup, like Java's `useMagic` guard order.
    if crate::game_loop::abnormal::all_skills_disabled(world, object_id) {
        helpers::send_action_failed(world, client_id);
        return;
    }
    // Unknown skill → ActionFailed (RequestMagicSkillUse.runImpl).
    //
    // Java asks `getKnownSkill`, which is one map holding both the learned
    // skills and everything granted with `addSkill(…, false)`. This port keeps
    // the transient grants apart so they cannot be persisted, so the lookup has
    // to consult both: without the `OptionSkills` fallback an augment's active
    // skill appears on the bar (it is in the `SkillList`) and then answers
    // every click with `ActionFailed`.
    let Some(skill_level) = known_skill_level(world, object_id, magic_id) else {
        helpers::send_action_failed(world, client_id);
        return;
    };
    // An enchanted skill resolves to its sub-level variant (Java's known
    // skill IS the enchanted instance); the plain instance backs sub 0 and
    // any sub the data lacks.
    let sub_level = world
        .objects
        .get_component::<crate::model::components::SkillEnchants>(&object_id)
        .and_then(|e| e.0.get(&magic_id).copied())
        .unwrap_or(0);
    let Some(skill) = world
        .data
        .skill_data
        .get_enchanted(magic_id, skill_level, sub_level)
        .or_else(|| world.data.skill_data.get(magic_id, skill_level))
        .cloned()
    else {
        return;
    };

    // Passive → ActionFailed (useMagic); toggles/unsupported targeting are
    // not castable yet and are consumed silently, same as before.
    if skill.operate_type == OperateType::Passive {
        helpers::send_action_failed(world, client_id);
        return;
    }
    // `Player.useMagic`: "Check if the caster is sitting" — a seated player may
    // cast nothing at all, passive-or-not, toggle-or-not, and is told so.
    // This gate is *not* the 2.5 s `SitBlock` animation block checked above:
    // that one expires while the character stays seated, which is why a sitting
    // player could cast freely a couple of seconds after sitting down.
    //
    // Java runs this check one step later, after the reuse-timer message, so a
    // seated player whose skill is also on cooldown hears about the cooldown
    // there and the chair here; ours always names the chair. Every other
    // ordering (passive first, toggles after) matches.
    if crate::game_loop::sit_stand::is_resting(world, object_id) {
        helpers::send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::YOU_CANNOT_USE_ACTIONS_AND_SKILLS_WHILE_THE_CHARACTER_IS_SITTING,
            &[],
        );
        return;
    }
    // `Player.useMagic`'s toggle branch, ahead of every other check: recasting
    // a live toggle switches it **off** and casts nothing; otherwise a toggle
    // in a group first stops the others in that group, then casts normally.
    //
    // Java's `isNecessaryToggle()` exemption (a toggle that may never be
    // switched off) is not ported. **Ten** skills on this dist do set it —
    // the Borna and elemental/Holy/Dark Stances — but every one is 11xxx /
    // 19xxx / 23xxx, i.e. post-Interlude, so none is reachable here. Re-check
    // the carriers, not this sentence, before relying on it.
    if skill.operate_type == OperateType::Toggle {
        let already_on = world
            .objects
            .get_component::<crate::model::components::Buffs>(&object_id)
            .is_some_and(|b| b.0.iter().any(|x| x.skill_id == skill.id));
        if already_on {
            crate::game_loop::skills::effects::handle_buff_expire(world, object_id, skill.id);
            helpers::send_action_failed(world, client_id);
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
                crate::game_loop::skills::effects::handle_buff_expire(world, object_id, sibling);
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
            world.objects.get_component::<Player>(&object_id),
            maybe_position(world, object_id),
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
            helpers::broadcast_including_self(world, object_id, &pkt);
        }
        apply_skill_effects(world, object_id, object_id, &skill);
        set_skill_reuse(world, object_id, &skill);
        helpers::send_action_failed(world, client_id);
        return;
    } else if !matches!(
        skill.operate_type,
        OperateType::Active | OperateType::Channeling
    ) {
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
        helpers::send_action_failed(world, client_id);
        return;
    }
    // `SkillCaster.checkDoCastConditions`' mute checks: a magic skill is
    // refused while silenced, a non-magic one while physically muted. Static
    // skills (`magic_type == 2`) bypass both, as in Java's `!skill.isStatic()`
    // guard.
    if skill.magic_type != 2 {
        let muted = if skill.magic_type == 1 {
            crate::game_loop::abnormal::is_muted(world, object_id)
        } else {
            crate::game_loop::abnormal::is_physical_muted(world, object_id)
        };
        if muted {
            helpers::send_action_failed(world, client_id);
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
    let Some(caster_pos) = maybe_position(world, object_id) else {
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
    // `PlayableAI.onIntentionCast`: a bad skill aimed at a playable runs the
    // Blessing of Protection pair before anything else about the cast.
    if skill.is_bad()
        && let Some(t) = caster_target
        && (world.objects.has_component::<Player>(&t)
            || world
                .objects
                .has_component::<crate::model::components::ServitorOf>(&t))
        && crate::game_loop::combat::pvp::protection_blessing_blocks(world, object_id, t)
    {
        helpers::send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::THAT_IS_AN_INCORRECT_TARGET,
            &[],
        );
        return;
    }
    // Fetched here rather than at the top of the function: the toggle branch
    // above needs `&mut world`, so this borrow must start after it.
    let Some(player) = world.objects.get_component::<Player>(&object_id) else {
        return;
    };
    let target_oid = match resolve_cast_target(
        world,
        player,
        &caster_pos,
        caster_target,
        &skill,
        ctrl,
        shift,
    ) {
        Ok(oid) => oid,
        Err(sm_id) => {
            helpers::send_sm_and_action_failed(world, client_id, sm_id, &[]);
            return;
        }
    };

    // `Player.useMagic`: `skill.checkCondition(this, target)` — every
    // `<conditions>`/`<targetConditions>` entry, evaluated here because the
    // target-scoped ones need the *resolved* target (G34 S1). The inline
    // `OpExistNpc` and transform gates that used to sit further up are now two
    // of these; the ordering change is Java's own.
    if let Err(refusal) =
        crate::game_loop::skills::conditions::check_cast(world, object_id, &skill, target_oid)
    {
        crate::game_loop::skills::conditions::send_refusal(
            world, client_id, object_id, &skill, target_oid, &refusal,
        );
        return;
    }

    // Busy — mid-cast or mid-swing (`useMagic`'s `isAttackingNow() ||
    // isCastingNow()` branch): park the click in the queue slot instead of
    // dropping it; it replays with full re-validation when the cast stops
    // (`stop_casting`) or the swing ends (`AttackFinish`/`thinkAttack`).
    // Java checks MP only after this, so a low-MP click still queues.
    let mid_swing = world
        .objects
        .get_component::<components::AttackState>(&object_id)
        .is_some_and(|st| st.attack_end_tick > world.tick);
    if mid_swing
        || world
            .objects
            .has_component::<components::Casting>(&object_id)
    {
        world.objects.add_components(
            &object_id,
            components::QueuedAction::Skill {
                skill_id: magic_id,
                ctrl,
                shift,
            },
        );
        helpers::send_action_failed(world, client_id);
        return;
    }

    // MP/HP prechecks (`checkUseConditions`).
    // Java `checkUseConditions`: `getMpConsume(skill) + getMpInitialConsume(skill)`
    // — the consume is rate-scaled, the *initial* consume is not.
    let scaled_mp_consume =
        crate::game_loop::skills::effects::mp_consume_for(world, object_id, &skill);
    let Some(v) = world
        .objects
        .get_component::<components::Vitals>(&object_id)
    else {
        return;
    };
    if v.cur_mp < (skill.mp_initial_consume + scaled_mp_consume) as f64 {
        helpers::send_sm_and_action_failed(world, client_id, sm_ids::NOT_ENOUGH_MP, &[]);
        return;
    }
    if v.cur_hp <= skill.hp_consume as f64 {
        helpers::send_sm_and_action_failed(world, client_id, sm_ids::NOT_ENOUGH_HP, &[]);
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
            helpers::send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::THERE_ARE_NOT_ENOUGH_NECESSARY_ITEMS_TO_USE_THE_SKILL,
                &[],
            );
            return;
        }
    }

    // Shift-click is Java's `dontMove`, and the target handlers test it with a
    // *different* metric from the walk gate below: `if (dontMove &&
    // (creature.calculateDistance2D(target) > skill.getCastRange()))` — raw
    // centre-to-centre 2D, **no collision radii**. So the refusal is strictly
    // tighter than the range that would have been walked into, and a shift-cast
    // that survives it never wants to move.
    if shift
        && skill.cast_range > 0
        && target_oid != object_id
        && let Some((tx, ty, _, _)) = target_state(world, target_oid)
    {
        let (dx, dy) = ((tx - caster_pos.x) as f64, (ty - caster_pos.y) as f64);
        if dx * dx + dy * dy > (skill.cast_range as f64).powi(2) {
            helpers::send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED,
                &[],
            );
            return;
        }
    }

    // Cast-range gate (`SkillCaster.castSkill` returning null → the AI walks
    // into range via `thinkCast`/`maybeMoveToPawn`). This one *does* carry the
    // collision radii — `Util.checkIfInRange` adds both.
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

    // Past every reject: this click is now the player's order — a walk-to-cast
    // still in flight is superseded (Java: each accepted `useMagic` sets a
    // fresh CAST intention; a rejected one leaves the old intention running).
    if matches!(
        world
            .objects
            .get_component::<components::Intent>(&object_id),
        Some(components::Intent(crate::model::PlayerIntent::Cast { .. }))
    ) {
        world
            .objects
            .remove_component::<components::Intent>(&object_id);
    }

    if out_of_range {
        world.objects.add_components(
            &object_id,
            components::Intent(crate::model::PlayerIntent::Cast {
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
