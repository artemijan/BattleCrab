use super::Combatant;
use super::combatant;
use super::distance_2d;
use super::do_auto_attack;
use super::refresh_attack_stance;
use super::target_is_dead;
use super::wields_two_handed;
use crate::game_loop::helpers;
use crate::game_loop::net::broadcast;
use crate::game_loop::space::position::maybe_position;
use crate::model::components;

use crate::game_loop::skills::effects::target_p_def;
use crate::game_loop::space::position;
use crate::model::PlayerIntent;

use crate::model::formulas;
use crate::model::movement;
use crate::model::movement::MoveData;
use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::network::server_packets::sm_ids;
use crate::scheduler::ScheduledTask;
use crate::scheduler::ms_to_ticks;
use crate::world::World;

/// Port of `clientpackets/AttackRequest` + `Player.onActionRequest` →
/// `NpcAction`'s monster branch: clicking your already-selected monster
/// starts the attack loop. A click on something that isn't the current
/// target re-selects instead (Java falls back to `onAction`).
pub(crate) fn handle_attack_request(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::AttackRequest::read(body) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    let Some(player) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
    else {
        return;
    };

    if helpers::is_dead(world, object_id) {
        helpers::send_action_failed(world, client_id);
        return;
    }

    let _ = player;
    // `Creature.isAttackDisabled()` → `isDisabled()` → `hasBlockActions()`: a
    // stunned/asleep/paralyzed attacker is refused outright. Java's same
    // predicate also ORs `isPhysicalAttackMuted()` — the auto-attack lock,
    // distinct from `PHYSICAL_MUTED`'s skill lock (G34 S3).
    // Java `AttackRequest`: `if ((!target.isTargetable() ||
    // player.isTargetingDisabled()) && !canOverrideCond(TARGET_ALL))` — the
    // force-attack path has the same targeting gate as `Action`.
    if crate::game_loop::abnormal::is_blocked_from_actions(world, object_id)
        || crate::game_loop::abnormal::is_physical_attack_muted(world, object_id)
        || crate::game_loop::abnormal::is_targeting_disabled(world, object_id)
        || crate::game_loop::abnormal::is_untargetable(world, pkt.object_id)
    {
        helpers::send_action_failed(world, client_id);
        return;
    }
    // A Ctrl-click (force attack) both selects *and* engages the target. When
    // switching target the client may send only this packet — no preceding
    // `Action` — so selecting without engaging drops the "attack this next"
    // order (Java gets the select via a separate `Action` first, then
    // `onForcedAttack`; we can't rely on that ordering). While casting or
    // mid-swing, `start_attack_intent` parks the attack as the intention that
    // fires when the cast/swing ends — Java's `onForcedAttack` →
    // `setIntention(ATTACK)`, deferred to `_nextIntention` while busy.
    let current = world
        .objects
        .get_component::<crate::model::components::TargetRef>(&object_id)
        .copied()
        .unwrap_or_default()
        .0;
    if current != Some(pkt.object_id) {
        super::target::set_target(world, client_id, object_id, Some(pkt.object_id));
    }

    // `pkt.shift` is deliberately dropped — Java's `AttackRequest._attackId`
    // ("0 for simple click 1 for shift-click") is read and never used.
    start_attack_intent(world, client_id, object_id, pkt.object_id);
}

/// Shared entry for "the player wants to auto-attack this target" (from
/// `AttackRequest` or the second `Action` click): monsters, siege
/// towers/flags/guards and siege gates, plus flagged players. Clean players
/// need Ctrl (enforced client-side) and plain folk aren't attackable without
/// the karma system. Out of reach the click starts a chase
/// (`player_attack_think` → `maybe_move_to_pawn`).
///
/// There is **no `dontMove` for melee**: `AttackRequest` reads the shift byte
/// into a field Java marks `@SuppressWarnings("unused")` and never looks at
/// again, so a shift-click walks the target down exactly like a plain one.
/// (`Action` case 1 doesn't reach here at all — it routes to `onActionShift`.)
/// The cast path's `dontMove` is real and lives in the target handlers.
pub(crate) fn start_attack_intent(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    target_object_id: i32,
) {
    // `PlayableAI.onIntentionAttack`'s very first line: `if
    // (getActingPlayer().isSitting()) return;`. A seated player never engages,
    // and Java sends **nothing** back — not even `clientActionFailed()` — so
    // the click is swallowed whole. (The `Action` click path does end with an
    // unconditional `ActionFailed` of its own; the `AttackRequest` one doesn't.)
    if crate::game_loop::character::sit_stand::is_resting(world, object_id) {
        return;
    }
    // `PlayableAI.onIntentionAttack`'s Blessing of Protection pair: refused
    // with INCORRECT_TARGET + ActionFailed for a playable target.
    let target_is_playable = world
        .objects
        .has_component::<crate::model::Player>(&target_object_id)
        || world
            .objects
            .has_component::<crate::model::components::ServitorOf>(&target_object_id);
    if target_is_playable
        && crate::game_loop::combat::pvp::protection_blessing_blocks(
            world,
            object_id,
            target_object_id,
        )
    {
        crate::game_loop::helpers::send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::THAT_IS_AN_INCORRECT_TARGET,
            &[],
        );
        return;
    }
    let target_is_player = world
        .objects
        .has_component::<crate::model::Player>(&target_object_id);
    let target_dead = target_is_dead(world, target_object_id);
    if target_is_player {
        // `Creature.onForcedAttack` (the Ctrl/force melee path): the client only
        // sends an AttackRequest against a player when it means to — either
        // Ctrl-forced or a target it already knows is attackable (from our
        // RelationChanged). The server just refuses inside a peace zone; the
        // clean-player "needs Ctrl" gate is enforced client-side.
        if target_dead {
            helpers::send_action_failed(world, client_id);
            return;
        }
        if crate::game_loop::space::zones::is_inside_peace_zone(world, object_id, target_object_id)
        {
            if let Some(client_id) = helpers::client_for_player(world, object_id) {
                crate::game_loop::helpers::send_sm_and_action_failed(
                    world,
                    client_id,
                    sm_ids::YOU_MAY_NOT_ATTACK_THIS_TARGET_IN_A_PEACEFUL_ZONE,
                    &[],
                );
            }
            return;
        }
    } else {
        // NPCs: monsters (auto-attackable template) — plus siege towers/flags,
        // which combatants tear down during a siege, and the stationed guards,
        // which attackers (anyone but a defender) may attack. Other folk aren't
        // attackable without the karma system.
        let attackable = super::target::is_auto_attackable(world, object_id, target_object_id);
        if !attackable || target_dead {
            helpers::send_action_failed(world, client_id);
            return;
        }
    }
    world.objects.add_components(
        &object_id,
        components::Intent(PlayerIntent::Attack { target_object_id }),
    );
    // Think immediately — first swing shouldn't wait for the next tick.
    player_attack_think(world, object_id);
}

/// `setIntention(AI_INTENTION_ATTACK, target)` from a *finished cast* — the
/// `nextAction` continuation. Distinct from [`start_attack_intent`], which is
/// the click path and re-runs the click-time gates (peace zone, the refusal
/// packets); by the time a cast has landed those have already been decided, so
/// this only sets the intent and thinks once.
pub(crate) fn resume_attack_intent(world: &mut World, object_id: i32, target_object_id: i32) {
    world.objects.add_components(
        &object_id,
        components::Intent(PlayerIntent::Attack { target_object_id }),
    );
    player_attack_think(world, object_id);
}

/// A player's melee swing against a targeted siege gate — the in-reach half of
/// the `DoorAction` attack path, called from `player_attack_think` once the
/// chase (`chase_target`) has closed the distance. Doors don't roll
/// miss/crit/shield and have no AI, so this is a straight pAtk-vs-pDef hit
/// (front, no shot); paced by the attacker's swing period so the loop
/// auto-repeats until the gate breaches, and the damage lands at the swing's
/// `timeToHit` through the same queued-hit machinery as `do_auto_attack`
/// (so an abort mid-swing drops it).
fn do_door_swing(world: &mut World, attacker_oid: i32, door_oid: i32) {
    // Re-check the siege gate (the loop can outlive the siege ending).
    if !crate::game_loop::siege::attackable_door(world, door_oid) {
        world
            .objects
            .remove_component::<components::Intent>(&attacker_oid);
        return;
    }
    let Some(attacker) = combatant(world, attacker_oid) else {
        return;
    };
    let Some(dpos) = maybe_position(world, door_oid) else {
        return;
    };

    // Damage: pAtk vs the door's pDef (front, no crit, no shot).
    let door_pdef = target_p_def(world, door_oid);
    let damage = formulas::physical::calc_auto_attack_damage(
        attacker.p_atk,
        1.0,
        movement::Position::Front,
        door_pdef,
        false,
        // A door swing never crits, so the crit stats are never read.
        formulas::physical::CritDamage::default(),
        false,
        // No shot is spent on a door, so `SHOTS_BONUS` never multiplies
        // anything — Java's `ssBonus` is a flat 1 on the `ss == false` arm.
        1.0,
        // Doors are hit with whatever is in hand; the weapon mod still
        // applies, so a bow batters a gate on 154 like it does anything else.
        super::ranged::is_ranged(
            super::ranged::equipped_weapon_type(world, attacker_oid).unwrap_or_default(),
        ),
        // A door is not a creature: it carries no traits, no elements and no
        // pvp/pve side, so all three multipliers are their identity.
        1.0,
        1.0,
        1.0,
    ) as i32;

    // Face the gate (Java `doAttack` `setHeading`).
    let heading =
        movement::calculate_heading((dpos.x - attacker.x) as f64, (dpos.y - attacker.y) as f64);
    if let Some(pos) = world
        .objects
        .get_component_mut::<components::Position>(&attacker_oid)
    {
        pos.heading = heading;
    }

    // Pace the loop: hold the next swing for the attacker's attack period and
    // fire the swing-end hook (queued action), exactly like `do_auto_attack`.
    let time_atk = formulas::timing::calculate_time_between_attacks(attacker.p_atk_spd);
    let now = world.tick;
    let mut swing_seq = 0;
    if let Some(st) = world
        .objects
        .get_component_mut::<components::AttackState>(&attacker_oid)
    {
        st.attack_end_tick = now + ms_to_ticks(time_atk);
        swing_seq = st.swing_seq;
    }
    world.scheduler.schedule(
        now + ms_to_ticks(time_atk),
        ScheduledTask::AttackFinish {
            object_id: attacker_oid,
        },
    );
    // Land the damage at `timeToHit`, like a creature swing (Java `doAttack`
    // schedules `onHitTimeNotDual`); the shared `AttackHit` task carries the
    // door branch, and the seq guard drops it if the swing is aborted.
    let two_handed = wields_two_handed(world, attacker_oid);
    let time_to_hit = formulas::timing::calculate_time_to_hit(time_atk, two_handed);
    world.scheduler.schedule(
        now + ms_to_ticks(time_to_hit),
        ScheduledTask::AttackHit {
            attacker: attacker_oid,
            target: door_oid,
            damage,
            miss: false,
            crit: false,
            swing_seq,
        },
    );

    // Broadcast the swing.
    let hit = server_packets::AttackHit {
        target_object_id: door_oid,
        damage,
        miss: false,
        crit: false,
        soulshot: false,
        ss_grade: 0,
    };
    let pkt = server_packets::attack(
        attacker_oid,
        std::slice::from_ref(&hit),
        attacker.x,
        attacker.y,
        attacker.z,
        dpos.x,
        dpos.y,
        dpos.z,
    );
    broadcast::broadcast_including_self(world, attacker_oid, &pkt);
    refresh_attack_stance(world, attacker_oid);
}

/// Apply damage to a siege door's HP and push its refreshed HP bar to nearby
/// clients (`StatusUpdate`); a breach opens the gate (`siege::damage_door`).
/// Shared by the melee swing (`do_door_swing`) and offensive skills
/// (`skills::effects::apply_skill_damage`).
pub(crate) fn apply_door_damage(world: &mut World, door_oid: i32, damage: i32) {
    crate::game_loop::siege::damage_door(world, door_oid, damage);
    let (cur_hp, max_hp) = {
        let d = world
            .objects
            .get_component::<crate::model::door::Door>(&door_oid);
        (
            d.map(|d| d.current_hp).unwrap_or(0),
            d.and_then(|d| world.data.door_data.get(d.door_id))
                .map(|t| t.hp_max)
                .unwrap_or(1),
        )
    };
    if let Some(region) = position::region_cell_of(world, door_oid) {
        broadcast::broadcast_near_region_in(
            world,
            region,
            helpers::instance_of(world, door_oid),
            &server_packets::status_update(
                door_oid,
                &[
                    (server_packets::status_update_type::MAX_HP, max_hp),
                    (server_packets::status_update_type::CUR_HP, cur_hp),
                ],
            ),
        );
    }
}

/// Per-tick player combat system: drive every attack/cast intent one step.
/// The sweep is presence-filtered — only intent-holders are visited.
pub(crate) fn player_combat_tick(world: &mut World) {
    // `AbstractAI.changeIntention` calls `stopFollow()` on every intention
    // switch, and `clientNotifyDead`/`onIntentionIdle` do the same. Rather than
    // chase every one of those call sites, the invariant is enforced here: a
    // follow latch only outlives the intent that started it by one tick.
    let mut orphaned: Vec<i32> = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::Player, &components::Following)>(|(p, _)| {
            orphaned.push(p.object_id)
        });
    for object_id in orphaned {
        if !world
            .objects
            .has_component::<components::Intent>(&object_id)
        {
            stop_follow(world, object_id);
        }
    }

    let mut ids: Vec<i32> = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::Player, &components::Intent)>(|(p, _)| {
            ids.push(p.object_id)
        });
    for object_id in ids {
        match world
            .objects
            .get_component::<components::Intent>(&object_id)
            .copied()
        {
            Some(components::Intent(PlayerIntent::Attack { .. })) => {
                player_attack_think(world, object_id)
            }
            Some(components::Intent(PlayerIntent::Cast { .. })) => {
                player_cast_think(world, object_id)
            }
            Some(components::Intent(PlayerIntent::Interact { .. })) => {
                player_interact_think(world, object_id)
            }
            Some(components::Intent(PlayerIntent::PickUp { .. })) => {
                player_pickup_think(world, object_id)
            }
            None => {}
        }
    }
}

/// `PlayerAI.thinkAttack`: chase into reach, swing when ready. Runs every
/// tick per intent-holding player; chase re-pathing is throttled to the
/// follow cadence inside `chase_target`.
fn player_attack_think(world: &mut World, object_id: i32) {
    let Some(player) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
    else {
        return;
    };
    let Some(components::Intent(PlayerIntent::Attack { target_object_id })) = world
        .objects
        .get_component::<components::Intent>(&object_id)
        .copied()
    else {
        return;
    };

    let dead = helpers::is_dead(world, object_id);
    if dead
        || world
            .objects
            .has_component::<components::Casting>(&object_id)
    {
        return; // casting pauses the loop (Java: CAST intention), death ends it via do_die.
    }
    // Target gone or dead → drop the intent (Java `checkTargetLostOrDead` →
    // ACTIVE intention). A breached siege gate counts as dead.
    if target_is_dead(world, target_object_id) {
        world
            .objects
            .remove_component::<components::Intent>(&object_id);
        return;
    }
    // Mid-swing: wait for the attack period to pass (`isAttackingNow`).
    let _ = player;
    if world
        .objects
        .get_component::<components::AttackState>(&object_id)
        .is_some_and(|st| st.attack_end_tick > world.tick)
    {
        return;
    }
    // A skill queued during the swing fires before the next swing (Java
    // `thinkAttack`'s queued-skill check). Normally the `AttackFinish` task
    // consumed it already this tick — this is the in-loop backstop. A cast
    // takes over the turn; anything else interleaves with the loop.
    if world
        .objects
        .has_component::<crate::model::components::QueuedAction>(&object_id)
    {
        run_queued_action(world, object_id);
        if world
            .objects
            .has_component::<components::Casting>(&object_id)
        {
            return;
        }
    }

    let Some(attacker) = combatant(world, object_id) else {
        return;
    };
    let Some(target) = combatant(world, target_object_id) else {
        return;
    };
    // `maybeMoveToPawn(target, getPhysicalAttackRange())` — the weapon's range,
    // so a bow's 500 flows through the exact same gate a dagger's 40 does.
    if maybe_move_to_pawn(
        world,
        object_id,
        target_object_id,
        &target,
        attacker.atk_range,
    ) {
        return;
    }
    // In reach: stop the chase and swing.
    position::stop_movement(world, object_id);
    // A siege door takes damage through the gate path (no miss/crit/shield/AI);
    // everything else goes through the shared creature swing.
    if world
        .objects
        .has_component::<crate::model::door::Door>(&target_object_id)
    {
        do_door_swing(world, object_id, target_object_id);
    } else {
        do_auto_attack(world, object_id, target_object_id);
    }
}

/// Java widens the range gate by this much *while a follow is running*
/// (`maybeMoveToPawn`: "allow larger hit range when the target is moving —
/// check is run only once per second"). Without it a chase after anything that
/// keeps walking re-paths forever and never gets to swing or cast, because the
/// strict gate is re-evaluated faster than the target can be caught.
const FOLLOW_ENGAGE_SLACK: f64 = 100.0;

/// How much closer Java aims when the pawn is on the move
/// (`if (target.isMoving()) offset -= 100`), floored at
/// [`FOLLOW_MIN_OFFSET`]. Aiming *past* the reach point is what makes the
/// chase converge on a runner instead of trailing it at exactly reach.
const FOLLOW_MOVING_OFFSET: i32 = 100;

/// `maybeMoveToPawn`'s `if (offset < 5) offset = 5` floor.
const FOLLOW_MIN_OFFSET: i32 = 5;

/// `AbstractAI.moveToPawn`'s own, higher floor: `if (offset < 10) offset = 10`.
const MOVE_TO_PAWN_MIN_OFFSET: i32 = 10;

/// `_moveToPawnTimeout += 1000 / GameTimeTaskManager.MILLIS_IN_TICK` — the 1 s
/// window during which `moveToPawn` refuses to re-path toward the same pawn at
/// the same offset. Java's game tick is 100 ms, and so is ours.
const MOVE_TO_PAWN_TIMEOUT_TICKS: u64 = 10;

/// `else if (_actor.isOnGeodataPath() && (gameTicks < (_moveToPawnTimeout +
/// 10)))` — a *changed* offset still waits a second longer while the current
/// path came out of the pathfinder ("minimum time to calculate new route is 2
/// seconds").
const GEODATA_REPATH_EXTRA_TICKS: u64 = 10;

/// `CreatureFollowTaskManager.follow`: past this the follow gives up entirely
/// ("if the target is too far — maybe also teleported") and the intention goes
/// idle, rather than starting a cross-map walk.
const FOLLOW_ABANDON_DISTANCE: f64 = 3000.0;

/// `AbstractAI.stopFollow()` — drop the actor out of the follow registry.
fn stop_follow(world: &mut World, object_id: i32) {
    world
        .objects
        .remove_component::<components::Following>(&object_id);
}

/// Port of `CreatureAI.maybeMoveToPawn(target, offsetValue)` — the one helper
/// Java runs for `thinkAttack`, `thinkCast`, `thinkInteract` and `thinkPickUp`
/// alike. Only `offsetValue` differs between them (`getPhysicalAttackRange()`,
/// `getMagicalAttackRange(skill)` = the skill's cast range, or the flat 36 of
/// the two interaction paths), which is exactly why short-range and bow
/// engagements — and attacks versus casts — must not drift apart: they are the
/// same code with a different number.
///
/// Returns `true` when a movement must be done, i.e. the caller should return
/// and retry on a later tick; `false` when the actor is in range and should act
/// now.
///
/// `target` is the caller's already-resolved pawn (the pick-up path chases a
/// ground item, which is a plain `WorldObject` with no collision radius of its
/// own — Java's `offsetWithCollision` only adds the target's radius
/// `if (target.isCreature())`).
fn maybe_move_to_pawn(
    world: &mut World,
    object_id: i32,
    target_object_id: i32,
    target: &Combatant,
    offset_value: i32,
) -> bool {
    if offset_value < 0 {
        return false; // skill radius -1: unlimited, never walk
    }
    let Some(actor) = combatant(world, object_id) else {
        return false;
    };
    let reach = offset_value as f64 + actor.collision_radius + target.collision_radius;
    let distance = distance_2d(&actor, target);
    if distance <= reach {
        stop_follow(world, object_id);
        return false;
    }

    // `target.isCreature() && !target.isDoor()` — only a live creature is
    // followed. A siege gate or a ground item is walked to with a plain
    // `moveToPawn`, so it never enters the follow registry and therefore never
    // earns the hysteresis below (`isFollowing()` is false for it).
    let followable = world
        .objects
        .has_component::<components::Vitals>(&target_object_id)
        && !world
            .objects
            .has_component::<crate::model::door::Door>(&target_object_id);

    // `if (isFollowing())`: a chase already in flight engages anywhere inside
    // `reach + 100`, and stops following as it does. The next think starts from
    // the strict gate again — which is Java's actual behaviour against a
    // fleeing target: swing, chase, swing, rather than chase forever.
    let following = world
        .objects
        .get_component::<components::Following>(&object_id)
        .copied()
        .filter(|f| f.target_object_id == target_object_id);
    if followable && let Some(follow) = following {
        if distance > reach + FOLLOW_ENGAGE_SLACK {
            follow_step(world, object_id, target_object_id, target, follow.offset);
            return true;
        }
        stop_follow(world, object_id);
        return false;
    }

    // `isMovementDisabled() || getMoveSpeed() <= 0` — rooted, paralyzed,
    // overloaded, or simply unable to move. Java is deliberately asymmetric
    // here: an ATTACK intention gives up (`setIntention(AI_INTENTION_IDLE)`),
    // every other intention keeps standing there waiting for the block to lift.
    if crate::game_loop::abnormal::is_movement_disabled(world, object_id)
        || world
            .objects
            .get_component::<components::Speeds>(&object_id)
            .is_none_or(|s| s.move_speed() <= 0.0)
    {
        if matches!(
            world
                .objects
                .get_component::<components::Intent>(&object_id),
            Some(components::Intent(PlayerIntent::Attack { .. }))
        ) {
            world
                .objects
                .remove_component::<components::Intent>(&object_id);
        }
        stop_follow(world, object_id);
        return true;
    }

    // "while flying there is no move to cast": a player who would have to walk
    // into range to finish a cast is refused outright while transformed into a
    // **non-combat** form, rather than walked there. `checkTransformed(t ->
    // !t.isCombat())` in Java — so an untransformed player, or one in a COMBAT
    // form (89 of the 174 templates on this dist), is unaffected.
    if matches!(
        world
            .objects
            .get_component::<components::Intent>(&object_id),
        Some(components::Intent(PlayerIntent::Cast { .. }))
    ) {
        let non_combat_form = world
            .objects
            .get_component::<crate::model::Player>(&object_id)
            .filter(|p| p.transform_id != 0)
            .and_then(|p| world.data.transforms.get(p.transform_id))
            .is_some_and(|tr| !tr.combat);
        if non_combat_form {
            if let Some(cid) = helpers::client_for_player(world, object_id) {
                crate::game_loop::helpers::send_sm_and_action_failed(
                    world,
                    cid,
                    sm_ids::THE_DISTANCE_IS_TOO_FAR_AND_SO_THE_CASTING_HAS_BEEN_CANCELLED,
                    &[],
                );
            }
            return true;
        }
    }

    let mut offset = offset_value;
    if followable {
        // `startFollow(target, offset)`. A moving pawn is chased 100 units
        // deeper so the walk actually catches it.
        if world
            .objects
            .has_component::<components::Movement>(&target_object_id)
        {
            offset -= FOLLOW_MOVING_OFFSET;
        }
        offset = offset.max(FOLLOW_MIN_OFFSET);
        world.objects.add_components(
            &object_id,
            components::Following {
                target_object_id,
                offset,
            },
        );
        // `addAttackFollow` runs `follow()` once immediately, so the walk
        // starts on this think rather than waiting for the next task period.
        follow_step(world, object_id, target_object_id, target, offset);
    } else {
        stop_follow(world, object_id);
        chase_pawn(world, object_id, target_object_id, target, offset);
    }
    true
}

/// One pass of `CreatureFollowTaskManager.follow(creature, range)` — the 500 ms
/// task that does the actual walking for a registered follow, while
/// `maybeMoveToPawn` only decides whether to engage.
///
/// Its range test is deliberately unlike the engage gate's: **3D**, and
/// **centre to centre** with no collision radii. Past
/// [`FOLLOW_ABANDON_DISTANCE`] it gives the intention up rather than start a
/// cross-map walk — Java's "if the target is too far (maybe also teleported)".
fn follow_step(
    world: &mut World,
    object_id: i32,
    target_object_id: i32,
    target: &Combatant,
    follow_range: i32,
) {
    let Some(actor) = combatant(world, object_id) else {
        return;
    };
    let (dx, dy, dz) = (
        (target.x - actor.x) as f64,
        (target.y - actor.y) as f64,
        (target.z - actor.z) as f64,
    );
    let distance_3d = (dx * dx + dy * dy + dz * dz).sqrt();
    if distance_3d <= follow_range as f64 {
        return; // already inside the follow range: the task does nothing
    }
    if distance_3d > FOLLOW_ABANDON_DISTANCE {
        world
            .objects
            .remove_component::<components::Intent>(&object_id);
        stop_follow(world, object_id);
        return;
    }
    chase_pawn(world, object_id, target_object_id, target, follow_range);
}

/// [`maybe_move_to_pawn`] with the pawn resolved from the store — the shape
/// every caller but the ground-item pick-up path wants.
fn maybe_move_to_pawn_oid(
    world: &mut World,
    object_id: i32,
    target_object_id: i32,
    offset_value: i32,
) -> bool {
    let Some(target) = combatant(world, target_object_id) else {
        return false;
    };
    maybe_move_to_pawn(world, object_id, target_object_id, &target, offset_value)
}

/// `AbstractAI.moveToPawn(pawn, offsetValue)` — walk to `offset_value` of the
/// pawn's **centre** (collision radii belong to the range *test*, not to the
/// destination) and broadcast `MoveToPawn` carrying that same raw offset.
///
/// The NPC chase (`AttackableAI.thinkAttack`) reaches `moveToLocation` through
/// its own call and deliberately *does* pass a radii-inclusive range, so the
/// radii question lives at the call sites, not here.
fn chase_pawn(
    world: &mut World,
    object_id: i32,
    target_object_id: i32,
    target: &Combatant,
    offset_value: i32,
) {
    // `if (offset < 10) offset = 10;` — `moveToPawn`'s own floor, one notch
    // above `maybeMoveToPawn`'s 5.
    let offset = offset_value.max(MOVE_TO_PAWN_MIN_OFFSET);

    // "prevent possible extra calls to this function, also don't send
    // movetopawn packets too often": while already walking toward this same
    // pawn, a re-path at an unchanged offset waits out the 1 s timeout, and a
    // *changed* offset still waits 2 s if the current path came from geodata.
    if world
        .objects
        .has_component::<components::Movement>(&object_id)
        && let Some(state) = world
            .objects
            .get_component::<components::MoveToPawnState>(&object_id)
            .copied()
        && state.target_object_id == target_object_id
    {
        let on_geodata_path = world
            .objects
            .get_component::<components::Movement>(&object_id)
            .is_some_and(|components::Movement(m)| m.geo_path.is_some());
        if state.offset == offset {
            if world.tick < state.timeout_tick {
                return;
            }
        } else if on_geodata_path && world.tick < state.timeout_tick + GEODATA_REPATH_EXTRA_TICKS {
            return;
        }
    }

    let Some(attacker) = combatant(world, object_id) else {
        return;
    };
    let Some((dest_x, dest_y, dest_z, heading)) =
        pawn_destination(&attacker, target, offset as f64)
    else {
        return;
    };

    let (speed, start) = {
        let Some(speeds) = world
            .objects
            .get_component::<components::Speeds>(&object_id)
        else {
            return;
        };
        let pos = maybe_position(world, object_id).unwrap_or(components::Position {
            x: 0,
            y: 0,
            z: 0,
            heading: 0,
        });
        (speeds.move_speed(), (pos.x, pos.y, pos.z))
    };
    if speed <= 0.0 {
        return;
    }
    let distance =
        (((dest_x - start.0) as f64).powi(2) + ((dest_y - start.1) as f64).powi(2)).sqrt();
    let total_ticks = ((10.0 * distance / speed).round() as u64).max(1);
    let start_tick = world.tick;
    if let Some(pos) = world
        .objects
        .get_component_mut::<components::Position>(&object_id)
    {
        pos.heading = heading;
    }
    world.objects.add_components(
        &object_id,
        components::Movement(MoveData {
            start_x: start.0,
            start_y: start.1,
            start_z: start.2,
            dest_x,
            dest_y,
            dest_z,
            start_tick,
            total_ticks,
            geo_path: None,
        }),
    );
    // `_moveToPawnTimeout = gameTicks + 1000 / MILLIS_IN_TICK` — Java's game
    // tick is 100 ms, the same as ours, so the 1 s window is 10 ticks either
    // way.
    world.objects.add_components(
        &object_id,
        components::MoveToPawnState {
            target_object_id,
            offset,
            timeout_tick: world.tick + MOVE_TO_PAWN_TIMEOUT_TICKS,
        },
    );
    let pkt = server_packets::move_to_pawn(
        object_id,
        target_object_id,
        offset,
        start.0,
        start.1,
        start.2,
        target.x,
        target.y,
        target.z,
    );
    broadcast::broadcast_including_self(world, object_id, &pkt);
}

/// `Creature.moveToLocation`'s offset handling: the point `offset_value − 5`
/// from the target on the mover→target line, plus the facing heading. `None`
/// when the walk is already over — Java notifies `EVT_ARRIVED` and returns
/// without moving whenever `distance − offset <= 0`.
///
/// `offset_value` is a distance from the target's **centre**; whether collision
/// radii were folded into it is the caller's business (`maybeMoveToPawn` keeps
/// them out, `AttackableAI.thinkAttack` puts them in).
pub(crate) fn pawn_destination(
    mover: &Combatant,
    target: &Combatant,
    offset_value: f64,
) -> Option<(i32, i32, i32, i32)> {
    let dx = (target.x - mover.x) as f64;
    let dy = (target.y - mover.y) as f64;
    let dz = (target.z - mover.z) as f64;
    let distance = (dx * dx + dy * dy).sqrt();
    // "approximation for moving closer when z coordinates are different" — a
    // target up a slope is walked to more tightly, since part of the offset is
    // spent on the height difference the 2D geometry can't see.
    let offset = (offset_value - dz.abs()).max(5.0);
    if distance < 1.0 || distance - offset <= 0.0 {
        return None;
    }
    // Land 5 units inside the offset (Java: "due to rounding error, we have to
    // move a bit closer to be in range") — aiming at the exact boundary can
    // round to a point just outside it, wedging the chase in an arrive/re-path
    // loop that never satisfies the range gate.
    let frac = (distance - (offset - 5.0)) / distance;
    let dest_x = mover.x + (dx * frac).round() as i32;
    let dest_y = mover.y + (dy * frac).round() as i32;
    let heading = movement::calculate_heading(dx, dy);
    Some((dest_x, dest_y, target.z, heading))
}

/// `PlayerAI.thinkCast` for the walk-to-cast leg: chase into the skill's cast
/// range (`maybeMoveToPawn(target, getMagicalAttackRange(skill))`), then hand
/// back to `use_magic_on` for a fully re-validated cast (LOS from the arrival
/// spot, MP, reuse) at the target snapshotted in the intent — Java casts at
/// the intention's cast target even if the player re-targeted mid-walk.
pub(crate) fn player_cast_think(world: &mut World, object_id: i32) {
    let Some(components::Intent(PlayerIntent::Cast {
        skill_id,
        ctrl,
        shift,
        target_object_id,
    })) = world
        .objects
        .get_component::<components::Intent>(&object_id)
        .copied()
    else {
        return;
    };
    if helpers::is_dead(world, object_id)
        || world
            .objects
            .has_component::<components::Casting>(&object_id)
    {
        return;
    }
    // `checkTargetLost`: a dead or vanished target drops the intention. A
    // siege door carries no `Vitals` (its HP lives on the `Door` component), so
    // use the door-aware `target_is_dead` — otherwise `vitals_of` reads `None`
    // for a door and the walk-to-cast is abandoned before it starts.
    if target_is_dead(world, target_object_id) {
        world
            .objects
            .remove_component::<components::Intent>(&object_id);
        return;
    }
    let cast_range = world
        .objects
        .get_component::<crate::model::components::SkillBook>(&object_id)
        .and_then(|book| book.0.get(&skill_id))
        .and_then(|&level| world.data.skill_data.get(skill_id, level))
        .map(|s| s.cast_range);
    let Some(cast_range) = cast_range else {
        world
            .objects
            .remove_component::<components::Intent>(&object_id);
        return;
    };
    // `maybeMoveToPawn(target, getMagicalAttackRange(skill))` — the same helper
    // `thinkAttack` uses, with the skill's cast range in place of the weapon's
    // attack range. Its reach (`castRange` + both collision radii, 2D) is the
    // one `in_cast_range` applies, so arriving here means the cast's own gate
    // in `use_magic_on` will pass too.
    if maybe_move_to_pawn_oid(world, object_id, target_object_id, cast_range) {
        return;
    }
    // Arrived: consume the intention, stop the chase leg (`clientStopMoving`
    // in `thinkCast`), and cast.
    world
        .objects
        .remove_component::<components::Intent>(&object_id);
    position::stop_movement(world, object_id);
    let Some(client_id) = helpers::client_for_player(world, object_id) else {
        return;
    };
    crate::game_loop::skills::cast::use_magic_on(
        world,
        client_id,
        object_id,
        skill_id,
        ctrl,
        shift,
        Some(target_object_id),
    );
}

/// Shared entry for "the player wants to talk to this NPC but
/// `Npc.canInteract` failed" (out of `target::INTERACTION_DISTANCE`): Java's
/// `NpcAction` sets `AI_INTENTION_INTERACT`, which `CreatureAI.onIntentionInteract`
/// turns into an immediate `moveToPawn` — mirrored here by setting the intent
/// and thinking it once synchronously, same as `start_attack_intent`.
pub(crate) fn start_interact_intent(world: &mut World, object_id: i32, target_object_id: i32) {
    // `CreatureAI.onIntentionInteract`'s REST branch — a seated player doesn't
    // walk over. Paired with the sitting leg of `target::can_interact`
    // (`Npc.canInteract`), this reproduces Java exactly: sitting fails
    // `canInteract`, so the click falls through to `AI_INTENTION_INTERACT`,
    // which REST then refuses. Net effect — clicking an NPC while seated does
    // nothing at all, near or far.
    if crate::game_loop::character::sit_stand::is_resting(world, object_id) {
        return;
    }
    world.objects.add_components(
        &object_id,
        components::Intent(PlayerIntent::Interact { target_object_id }),
    );
    player_interact_think(world, object_id);
}

/// `PlayerAI.thinkInteract`: chase to `maybeMoveToPawn(target, 36)` range,
/// then hand back to `interact_with_npc` for a fully re-validated interaction
/// — Java's `Player.doInteract` re-dispatches `target.onAction(this)`, which
/// re-runs the same click handler now that `canInteract` (250 units) passes
/// comfortably inside this 36-unit arrival range.
fn player_interact_think(world: &mut World, object_id: i32) {
    let Some(components::Intent(PlayerIntent::Interact { target_object_id })) = world
        .objects
        .get_component::<components::Intent>(&object_id)
        .copied()
    else {
        return;
    };
    if helpers::is_dead(world, object_id)
        || world
            .objects
            .has_component::<components::Casting>(&object_id)
    {
        return;
    }
    // Target gone → drop the intention (Java `checkTargetLost`).
    let Some(target) = combatant(world, target_object_id) else {
        world
            .objects
            .remove_component::<components::Intent>(&object_id);
        return;
    };
    const INTERACT_APPROACH_RANGE: i32 = 36;
    if maybe_move_to_pawn(
        world,
        object_id,
        target_object_id,
        &target,
        INTERACT_APPROACH_RANGE,
    ) {
        return;
    }
    // Arrived: stop the chase leg and re-run the interact click.
    world
        .objects
        .remove_component::<components::Intent>(&object_id);
    position::stop_movement(world, object_id);
    let Some(client_id) = helpers::client_for_player(world, object_id) else {
        return;
    };
    // Re-entry after walking into interaction range: only the chat/interact
    // branch reaches here (attackable targets chase via the attack loop, not
    // this walk-to-interact path), so the dontMove flag is moot.
    super::target::interact_with_npc(world, client_id, object_id, target_object_id);
}

/// `maybeMoveToPawn`'s offset for the pick-up/interact think loops — Java
/// `PlayerAI.thinkPickUp` passes 36, on top of which `maybeMoveToPawn` adds the
/// actor's collision radius (a ground item is not a `Creature`, so it
/// contributes none of its own).
const PICKUP_APPROACH_RANGE: i32 = 36;

/// `CreatureAI.onIntentionPickUp` — the ground-item click. Java never picks up
/// on the spot: the click only *sets* `AI_INTENTION_PICK_UP` and fires
/// `moveToPawn(object, 20)`; the lift happens later in `thinkPickUp`, once
/// `maybeMoveToPawn(target, 36)` reports the player has arrived.
pub(crate) fn start_pickup_intent(world: &mut World, object_id: i32, item_object_id: i32) {
    // `if (getIntention() == AI_INTENTION_REST) { clientActionFailed(); return; }`
    // — loot stays on the floor until the player stands up.
    if crate::game_loop::character::sit_stand::is_resting(world, object_id) {
        if let Some(client_id) = helpers::client_for_player(world, object_id) {
            helpers::send_action_failed(world, client_id);
        }
        return;
    }
    // `if (_actor.isAllSkillsDisabled() || _actor.isCastingNow())` — same
    // refusal, same bare ActionFailed.
    if world
        .objects
        .has_component::<components::Casting>(&object_id)
    {
        if let Some(client_id) = helpers::client_for_player(world, object_id) {
            helpers::send_action_failed(world, client_id);
        }
        return;
    }
    // `changeIntention(AI_INTENTION_PICK_UP, object)` replaces whatever was
    // running, so an attack loop in progress ends here. (Java's preceding
    // `clientStopAutoAttack()` sends nothing for a player — it only tops up the
    // attack-stance task — so there is no packet to port.)
    world.objects.add_components(
        &object_id,
        components::Intent(PlayerIntent::PickUp { item_object_id }),
    );
    // Java's `moveToPawn(object, 20)` fires from the intention itself; thinking
    // once synchronously reaches the same place via `thinkPickUp`, and also
    // lifts the item immediately when the player already stands on it.
    player_pickup_think(world, object_id);
}

/// `PlayerAI.thinkPickUp`: chase to `maybeMoveToPawn(target, 36)` range, then
/// `setIntention(AI_INTENTION_IDLE)` + `Player.doPickupItem`.
fn player_pickup_think(world: &mut World, object_id: i32) {
    let Some(components::Intent(PlayerIntent::PickUp { item_object_id })) = world
        .objects
        .get_component::<components::Intent>(&object_id)
        .copied()
    else {
        return;
    };
    // `Player.doPickupItem`'s `isAlikeDead()` guard plus thinkPickUp's own
    // `isAllSkillsDisabled() || isCastingNow()` bail (which does *not* drop the
    // intention — the walk resumes once the cast ends).
    if helpers::is_dead(world, object_id)
        || world
            .objects
            .has_component::<components::Casting>(&object_id)
    {
        return;
    }
    // `checkTargetLost` — someone else got there first, or it decayed.
    let (Some(item_pos), true) = (
        maybe_position(world, item_object_id),
        world
            .objects
            .has_component::<crate::model::components::GroundItem>(&item_object_id),
    ) else {
        world
            .objects
            .remove_component::<components::Intent>(&object_id);
        return;
    };
    // The item is a plain `WorldObject`, so `maybeMoveToPawn` adds only the
    // picker's collision radius to the 36-unit offset — and never follows it,
    // which is why a pick-up gets none of the moving-target slack a creature
    // chase does.
    let item = stationary_pawn(item_pos);
    if maybe_move_to_pawn(
        world,
        object_id,
        item_object_id,
        &item,
        PICKUP_APPROACH_RANGE,
    ) {
        return;
    }
    // Arrived: `setIntention(AI_INTENTION_IDLE)` then `doPickupItem`, which
    // itself sends the `StopMove` that ends the walk client-side.
    world
        .objects
        .remove_component::<components::Intent>(&object_id);
    position::stop_movement(world, object_id);
    let Some(client_id) = helpers::client_for_player(world, object_id) else {
        return;
    };
    crate::game_loop::items::ground_items::pickup_ground_item(
        world,
        client_id,
        object_id,
        item_object_id,
    );
}

/// A zero-extent `Combatant` standing at a point — lets the shared chase/reach
/// geometry (`distance_2d`/`pawn_destination`/`chase_pawn`) run against a plain
/// `WorldObject` such as a ground item, which carries none of the combat
/// components. Every stat field is inert; only the coordinates are read.
pub(crate) fn stationary_pawn(pos: components::Position) -> Combatant {
    Combatant {
        x: pos.x,
        y: pos.y,
        z: pos.z,
        heading: pos.heading,
        collision_radius: 0.0,
        dead: false,
        p_atk: 0.0,
        p_def: 0.0,
        crit_stat: 0.0,
        accuracy: 0,
        evasion: 0,
        p_atk_spd: 0,
        random_dmg: 0,
        atk_range: 0,
        shield_def: 0.0,
        shield_rate: 0.0,
        con_bonus: 1.0,
    }
}

// ---------------------------------------------------------------------------
// The swing (`Creature.doAutoAttack` → scheduled hit)
// ---------------------------------------------------------------------------

// Moved from helpers: the post-cast queued-action replay.
/// Fire the held-back action — the tail of Java `SkillCaster.stopCasting`
/// (queued skill → `useMagic`, else `EVT_FINISH_CASTING` → the saved MOVE_TO)
/// and of `EVT_READY_TO_ACT` at swing end. Each replay re-enters the normal
/// handler pipeline, so it re-validates everything exactly like a fresh
/// click. No-op while still busy (casting or mid-swing) or dead — the slot
/// stays for the later stop.
pub(crate) fn run_queued_action(world: &mut World, object_id: i32) {
    use crate::model::components::{AttackState, Casting, QueuedAction};
    let Some(&action) = world.objects.get_component::<QueuedAction>(&object_id) else {
        return;
    };
    if world.objects.has_component::<Casting>(&object_id)
        || world
            .objects
            .get_component::<AttackState>(&object_id)
            .is_some_and(|st| st.attack_end_tick > world.tick)
        || helpers::is_dead(world, object_id)
    {
        return;
    }
    world.objects.remove_component::<QueuedAction>(&object_id);
    let Some(client_id) = helpers::client_for_player(world, object_id) else {
        return;
    };
    match action {
        QueuedAction::Move { x, y, z } => {
            let Some(cur) = maybe_position(world, object_id) else {
                return;
            };
            crate::game_loop::space::position::intention_move_to(
                world,
                client_id,
                object_id,
                cur,
                (x, y, z),
            );
        }
        QueuedAction::Skill {
            skill_id,
            ctrl,
            shift,
        } => {
            crate::game_loop::skills::cast::use_magic(
                world, client_id, object_id, skill_id, ctrl, shift,
            );
        }
        QueuedAction::UseItem { item_object_id } => {
            crate::game_loop::items::use_equipable_item(
                world,
                client_id,
                object_id,
                item_object_id,
            );
        }
    }
}
