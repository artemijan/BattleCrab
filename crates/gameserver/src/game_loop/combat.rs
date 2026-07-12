//! The auto-attack pipeline (G9): `AttackRequest` handling, the player
//! intent think loops (`PlayerAI.thinkAttack`/`thinkCast` +
//! `CreatureFollowTask` — chase into attack or cast range, then act), the
//! shared swing/hit mechanics (`Creature.doAutoAttack` →
//! `CreatureAttackTaskManager` → `onHitTimeNotDual` → `onHitTarget`), and the
//! combat-stance tracker (`AttackStanceTaskManager`).
//!
//! Scope (see PROGRESS G9): melee swings only — bows/crossbows, dual-weapon
//! split hits, polearm sweeps, soulshots, and shield blocks are all deferred
//! (their formula terms are identity for the actors that exist). PvP
//! auto-attack (force-attacking players) is deferred with the PvP-flag
//! system.

use crate::model::components::{AttackState, Casting, Collision, CombatStats, Intent, Movement, PlayerVitals, Position, RegionCell, Speeds, Vitals};
use crate::model::formulas;
use crate::model::movement::{self, get_position, MoveData};
use crate::model::npc::{AggroList, NpcAi, NpcIntention};
use crate::model::stats::BaseStat;
use crate::model::PlayerIntent;
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, sm_ids, SmParam};
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::{broadcast_including_self, broadcast_near_region, client_for_player, ms_to_ticks};
use super::skills::cast::abort_cast;

/// `AttackStanceTaskManager.COMBAT_TIME` (15 s) in ticks.
pub(crate) const COMBAT_STANCE_TICKS: u64 = 150;

/// NPC object ids live above this base (`model::npc::FIRST_NPC_OBJECT_ID`);
/// everything below is a persistent id (players, items).
pub(crate) fn is_npc_oid(object_id: i32) -> bool {
    object_id >= crate::model::npc::FIRST_NPC_OBJECT_ID
}

/// `Vitals` of any combat actor (one store since the world merge).
pub(crate) fn vitals_of(world: &World, object_id: i32) -> Option<&Vitals> {
    world.objects.get_component::<Vitals>(&object_id)
}

/// The combat-relevant view of a player or NPC — the stat finalizer outputs
/// both kinds of combatant feed into the shared `Formulas` ports. NPC values
/// are derived on demand from the template (same finalizer math the player's
/// `recalculate_stats` runs: base × stat bonus × level mod), since NPCs have
/// no buff state yet.
pub(crate) struct Combatant {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    pub collision_radius: f64,
    pub dead: bool,
    pub p_atk: f64,
    pub p_def: f64,
    pub crit_stat: f64,
    pub accuracy: i32,
    pub evasion: i32,
    pub p_atk_spd: i32,
    pub random_dmg: i32,
    pub atk_range: i32,
}

pub(crate) fn combatant(world: &World, object_id: i32) -> Option<Combatant> {
    // One component-shaped path for both kinds — NPC stats are memoized
    // into `CombatStats` at spawn (`npc::npc_combat_stats`), so the old
    // per-call template derivation is gone.
    let pos = world.objects.get_component::<Position>(&object_id)?;
    let collision = world.objects.get_component::<Collision>(&object_id)?;
    let vitals = world.objects.get_component::<Vitals>(&object_id)?;
    let cs = world.objects.get_component::<CombatStats>(&object_id)?;
    Some(Combatant {
        x: pos.x,
        y: pos.y,
        z: pos.z,
        heading: pos.heading,
        collision_radius: collision.radius,
        dead: vitals.dead,
        p_atk: cs.p_atk,
        p_def: cs.p_def,
        crit_stat: cs.crit_hit,
        accuracy: cs.accuracy,
        evasion: cs.evasion,
        p_atk_spd: cs.p_atk_spd,
        random_dmg: cs.random_dmg,
        atk_range: cs.atk_range,
    })
}

/// 2D center-to-center distance between two combat actors.
fn distance_2d(a: &Combatant, b: &Combatant) -> f64 {
    (((b.x - a.x) as f64).powi(2) + ((b.y - a.y) as f64).powi(2)).sqrt()
}

/// Melee reach: attack range + both collision radii (Java adds collision
/// radii via `Util.checkIfInRange(range, a, b, true)` in the AI range gates).
fn attack_reach(a: &Combatant, b: &Combatant) -> f64 {
    a.atk_range as f64 + a.collision_radius + b.collision_radius
}

// ---------------------------------------------------------------------------
// Combat stance (`AttackStanceTaskManager`)
// ---------------------------------------------------------------------------

/// Put a player into (or refresh) combat stance — `addAttackStanceTask`.
/// Broadcasts `AutoAttackStart` only on the not-in-stance → in-stance edge.
pub(crate) fn refresh_attack_stance(world: &mut World, player_object_id: i32) {
    let now = world.tick;
    let Some(st) = world.objects.get_component_mut::<AttackState>(&player_object_id) else { return };
    let was_in_stance = st.stance_until_tick > now;
    st.stance_until_tick = now + COMBAT_STANCE_TICKS;
    if !was_in_stance {
        broadcast_including_self(world, player_object_id, &server_packets::auto_attack_start(player_object_id));
    }
}

/// The 1 s stance sweep: players whose 15 s ran out leave combat stance
/// (`AutoAttackStop` broadcast).
pub(crate) fn stance_tick(world: &mut World) {
    let now = world.tick;
    let mut expired: Vec<i32> = Vec::new();
    world.objects.for_each_mut::<(&crate::model::Player, &AttackState)>(|(p, st)| {
        if st.stance_until_tick != 0 && st.stance_until_tick <= now {
            expired.push(p.object_id);
        }
    });
    for object_id in expired {
        if let Some(st) = world.objects.get_component_mut::<AttackState>(&object_id) {
            st.stance_until_tick = 0;
        }
        broadcast_including_self(world, object_id, &server_packets::auto_attack_stop(object_id));
    }
}

// ---------------------------------------------------------------------------
// AttackRequest + player attack think
// ---------------------------------------------------------------------------

/// Port of `clientpackets/AttackRequest` + `Player.onActionRequest` →
/// `NpcAction`'s monster branch: clicking your already-selected monster
/// starts the attack loop. A click on something that isn't the current
/// target re-selects instead (Java falls back to `onAction`).
pub(crate) fn handle_attack_request(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::AttackRequest::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    let Some(player) = world.objects.get_component::<crate::model::Player>(&object_id) else { return };

    if world.objects.get_component::<Vitals>(&object_id).is_none_or(|v| v.dead) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }

    let _ = player;
    let current = world.objects.get_component::<crate::model::components::TargetRef>(&object_id).copied().unwrap_or_default().0;
    if current != Some(pkt.object_id) {
        super::target::set_target(world, client_id, object_id, Some(pkt.object_id));
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }

    start_attack_intent(world, client_id, object_id, pkt.object_id);
}

/// Shared entry for "the player wants to auto-attack this NPC" (from
/// `AttackRequest` or the second `Action` click): monsters only — guards/folk
/// don't auto-attack without the PvP/karma systems, and player targets wait
/// for PvP flags.
pub(crate) fn start_attack_intent(world: &mut World, client_id: u32, object_id: i32, target_object_id: i32) {
    let attackable = world
        .objects
        .get_component::<crate::model::npc::Npc>(&target_object_id)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.is_auto_attackable());
    let target_dead = world.objects.get_component::<Vitals>(&target_object_id).is_none_or(|v| v.dead);
    if !attackable || target_dead {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    world.objects.add_components(&object_id, Intent(PlayerIntent::Attack { target_object_id }));
    // Think immediately — first swing shouldn't wait for the next tick.
    player_attack_think(world, object_id);
}

/// Per-tick player combat system: drive every attack/cast intent one step.
/// The sweep is presence-filtered — only intent-holders are visited.
pub(crate) fn player_combat_tick(world: &mut World) {
    let mut ids: Vec<i32> = Vec::new();
    world.objects.for_each_mut::<(&crate::model::Player, &Intent)>(|(p, _)| ids.push(p.object_id));
    for object_id in ids {
        match world.objects.get_component::<Intent>(&object_id).copied() {
            Some(Intent(PlayerIntent::Attack { .. })) => player_attack_think(world, object_id),
            Some(Intent(PlayerIntent::Cast { .. })) => player_cast_think(world, object_id),
            Some(Intent(PlayerIntent::Interact { .. })) => player_interact_think(world, object_id),
            None => {}
        }
    }
}

/// `PlayerAI.thinkAttack`: chase into reach, swing when ready. Runs every
/// tick per intent-holding player; chase re-pathing is throttled to the
/// follow cadence inside `chase_target`.
fn player_attack_think(world: &mut World, object_id: i32) {
    let Some(player) = world.objects.get_component::<crate::model::Player>(&object_id) else { return };
    let Some(Intent(PlayerIntent::Attack { target_object_id })) =
        world.objects.get_component::<Intent>(&object_id).copied()
    else {
        return;
    };

    let dead = world.objects.get_component::<Vitals>(&object_id).is_none_or(|v| v.dead);
    if dead || world.objects.has_component::<Casting>(&object_id) {
        return; // casting pauses the loop (Java: CAST intention), death ends it via do_die.
    }
    // Target gone or dead → drop the intent (Java `checkTargetLostOrDead` →
    // ACTIVE intention).
    let target_alive = vitals_of(world, target_object_id).is_some_and(|v| !v.dead);
    if !target_alive {
        world.objects.remove_component::<Intent>(&object_id);
        return;
    }
    // Mid-swing: wait for the attack period to pass (`isAttackingNow`).
    let _ = player;
    if world
        .objects
        .get_component::<AttackState>(&object_id)
        .is_some_and(|st| st.attack_end_tick > world.tick)
    {
        return;
    }
    // A skill queued during the swing fires before the next swing (Java
    // `thinkAttack`'s queued-skill check). Normally the `AttackFinish` task
    // consumed it already this tick — this is the in-loop backstop. A cast
    // takes over the turn; anything else interleaves with the loop.
    if world.objects.has_component::<crate::model::components::QueuedAction>(&object_id) {
        super::helpers::run_queued_action(world, object_id);
        if world.objects.has_component::<Casting>(&object_id) {
            return;
        }
    }

    let Some(attacker) = combatant(world, object_id) else { return };
    let Some(target) = combatant(world, target_object_id) else { return };
    if distance_2d(&attacker, &target) > attack_reach(&attacker, &target) {
        chase_target(world, object_id, target_object_id, attacker.atk_range);
        return;
    }
    // In reach: stop the chase and swing.
    if world.objects.has_component::<Movement>(&object_id) {
        world.objects.remove_component::<Movement>(&object_id);
        if let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() {
            broadcast_including_self(world, object_id, &server_packets::stop_move(object_id, pos.x, pos.y, pos.z, pos.heading));
        }
    }
    do_auto_attack(world, object_id, target_object_id);
}

/// `Creature.moveToPawn` for a player chasing a pawn (attack target or cast
/// target): walk to `range` + collision radii of it, re-pathed every 5 ticks
/// (Java's 500 ms `CreatureFollowTaskManager.ATTACK_FOLLOW_WEIGHT` cadence).
fn chase_target(world: &mut World, object_id: i32, target_object_id: i32, range: i32) {
    if !world.tick.is_multiple_of(5) {
        // Keep walking on the current path between re-paths.
        if world.objects.has_component::<Movement>(&object_id) {
            return;
        }
    }
    let Some(attacker) = combatant(world, object_id) else { return };
    let Some(target) = combatant(world, target_object_id) else { return };
    let reach = range as f64 + attacker.collision_radius + target.collision_radius;
    let Some((dest_x, dest_y, dest_z, heading)) = pawn_destination(&attacker, &target, reach) else { return };

    let (speed, start) = {
        let Some(speeds) = world.objects.get_component::<Speeds>(&object_id) else { return };
        let pos = world.objects.get_component::<Position>(&object_id).copied().unwrap_or(Position { x: 0, y: 0, z: 0, heading: 0 });
        (speeds.move_speed(), (pos.x, pos.y, pos.z))
    };
    if speed <= 0.0 {
        return;
    }
    let distance = (((dest_x - start.0) as f64).powi(2) + ((dest_y - start.1) as f64).powi(2)).sqrt();
    let total_ticks = ((10.0 * distance / speed).round() as u64).max(1);
    let start_tick = world.tick;
    if let Some(pos) = world.objects.get_component_mut::<Position>(&object_id) {
        pos.heading = heading;
    }
    world.objects.add_components(
        &object_id,
        Movement(MoveData {
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
    let pkt = server_packets::move_to_pawn(
        object_id,
        target_object_id,
        reach as i32,
        start.0,
        start.1,
        start.2,
        target.x,
        target.y,
        target.z,
    );
    broadcast_including_self(world, object_id, &pkt);
}

/// The point at `reach` distance from the target on the mover→target line
/// (`Creature.moveToLocation`'s `offset` handling), plus the facing heading.
/// `None` when already inside reach.
pub(crate) fn pawn_destination(mover: &Combatant, target: &Combatant, reach: f64) -> Option<(i32, i32, i32, i32)> {
    let dx = (target.x - mover.x) as f64;
    let dy = (target.y - mover.y) as f64;
    let distance = (dx * dx + dy * dy).sqrt();
    if distance <= reach {
        return None;
    }
    // Land 5 units inside reach (Java `moveToLocation`: "due to rounding
    // error, we have to move a bit closer to be in range") — aiming at the
    // exact boundary can round to a point just outside it, wedging the
    // chase in an arrive/re-path loop that never satisfies the range gate.
    let frac = (distance - (reach - 5.0).max(0.0)) / distance;
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
    let Some(Intent(PlayerIntent::Cast { skill_id, ctrl, shift, target_object_id })) =
        world.objects.get_component::<Intent>(&object_id).copied()
    else {
        return;
    };
    if world.objects.get_component::<Vitals>(&object_id).is_none_or(|v| v.dead)
        || world.objects.has_component::<Casting>(&object_id)
    {
        return;
    }
    // `checkTargetLost`: a dead or vanished target drops the intention.
    if vitals_of(world, target_object_id).is_none_or(|v| v.dead) {
        world.objects.remove_component::<Intent>(&object_id);
        return;
    }
    let cast_range = world
        .objects
        .get_component::<crate::model::components::SkillBook>(&object_id)
        .and_then(|book| book.0.get(&skill_id))
        .and_then(|&level| world.data.skill_data.get(skill_id, level))
        .map(|s| s.cast_range);
    let Some(cast_range) = cast_range else {
        world.objects.remove_component::<Intent>(&object_id);
        return;
    };
    let Some(caster_pos) = world.objects.get_component::<Position>(&object_id).copied() else { return };
    if !super::skills::cast::in_cast_range(world, object_id, &caster_pos, target_object_id, cast_range, false) {
        chase_target(world, object_id, target_object_id, cast_range);
        return;
    }
    // Arrived: consume the intention, stop the chase leg (`clientStopMoving`
    // in `thinkCast`), and cast.
    world.objects.remove_component::<Intent>(&object_id);
    if world.objects.has_component::<Movement>(&object_id) {
        world.objects.remove_component::<Movement>(&object_id);
        if let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() {
            broadcast_including_self(world, object_id, &server_packets::stop_move(object_id, pos.x, pos.y, pos.z, pos.heading));
        }
    }
    let Some(client_id) = client_for_player(world, object_id) else { return };
    super::skills::cast::use_magic_on(world, client_id, object_id, skill_id, ctrl, shift, Some(target_object_id));
}

/// Shared entry for "the player wants to talk to this NPC but
/// `Npc.canInteract` failed" (out of `target::INTERACTION_DISTANCE`): Java's
/// `NpcAction` sets `AI_INTENTION_INTERACT`, which `CreatureAI.onIntentionInteract`
/// turns into an immediate `moveToPawn` — mirrored here by setting the intent
/// and thinking it once synchronously, same as `start_attack_intent`.
pub(crate) fn start_interact_intent(world: &mut World, object_id: i32, target_object_id: i32) {
    world.objects.add_components(&object_id, Intent(PlayerIntent::Interact { target_object_id }));
    player_interact_think(world, object_id);
}

/// `PlayerAI.thinkInteract`: chase to `maybeMoveToPawn(target, 36)` range,
/// then hand back to `interact_with_npc` for a fully re-validated interaction
/// — Java's `Player.doInteract` re-dispatches `target.onAction(this)`, which
/// re-runs the same click handler now that `canInteract` (250 units) passes
/// comfortably inside this 36-unit arrival range.
fn player_interact_think(world: &mut World, object_id: i32) {
    let Some(Intent(PlayerIntent::Interact { target_object_id })) =
        world.objects.get_component::<Intent>(&object_id).copied()
    else {
        return;
    };
    if world.objects.get_component::<Vitals>(&object_id).is_none_or(|v| v.dead)
        || world.objects.has_component::<Casting>(&object_id)
    {
        return;
    }
    let Some(attacker) = combatant(world, object_id) else { return };
    // Target gone → drop the intention (Java `checkTargetLost`).
    let Some(target) = combatant(world, target_object_id) else {
        world.objects.remove_component::<Intent>(&object_id);
        return;
    };
    const INTERACT_APPROACH_RANGE: i32 = 36;
    let reach = INTERACT_APPROACH_RANGE as f64 + attacker.collision_radius + target.collision_radius;
    if distance_2d(&attacker, &target) > reach {
        chase_target(world, object_id, target_object_id, INTERACT_APPROACH_RANGE);
        return;
    }
    // Arrived: stop the chase leg and re-run the interact click.
    world.objects.remove_component::<Intent>(&object_id);
    if world.objects.has_component::<Movement>(&object_id) {
        world.objects.remove_component::<Movement>(&object_id);
        if let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() {
            broadcast_including_self(world, object_id, &server_packets::stop_move(object_id, pos.x, pos.y, pos.z, pos.heading));
        }
    }
    let Some(client_id) = client_for_player(world, object_id) else { return };
    super::target::interact_with_npc(world, client_id, object_id, target_object_id);
}

// ---------------------------------------------------------------------------
// The swing (`Creature.doAutoAttack` → scheduled hit)
// ---------------------------------------------------------------------------

/// Port of `Creature.doAutoAttack` + `generateAttackTargetData`/`generateHit`
/// for the melee single-hit case, shared by players and NPCs: roll the hit
/// now, broadcast `Attack`, land it at `timeToHit` via the scheduler.
pub(crate) fn do_auto_attack(world: &mut World, attacker_oid: i32, target_oid: i32) {
    let Some(attacker) = combatant(world, attacker_oid) else { return };
    let Some(target) = combatant(world, target_oid) else { return };
    if attacker.dead || target.dead {
        return;
    }

    // GeoData LOS check (`doAutoAttack`).
    if !world.geo.can_see_target(attacker.x, attacker.y, attacker.z, target.x, target.y, target.z) {
        if let Some(client_id) = client_for_player(world, attacker_oid) {
            super::helpers::send_sm_and_action_failed(world, client_id, sm_ids::CANNOT_SEE_TARGET, &[]);
        }
        world.objects.remove_component::<Intent>(&attacker_oid);
        return;
    }

    let time_atk = formulas::calculate_time_between_attacks(attacker.p_atk_spd);
    // Two-handed timing needs the weapon's body part — item kinds are parsed
    // (G5), so check the equipped right hand for SLOT_LR_HAND.
    let two_handed = world.objects.get_component::<crate::model::inventory::Inventory>(&attacker_oid).is_some_and(|inv| {
        let rhand = inv.paperdoll_item_id(crate::model::inventory::PaperdollSlot::RHand);
        rhand != 0
            && world
                .data
                .item_data
                .get(rhand)
                .is_some_and(|t| t.body_part == crate::data::item_data::SLOT_LR_HAND)
    });
    let time_to_hit = formulas::calculate_time_to_hit(time_atk, two_handed);

    // Face the target (Java `setHeading(calculateHeadingFrom(...))`).
    let heading = movement::calculate_heading((target.x - attacker.x) as f64, (target.y - attacker.y) as f64);
    if let Some(pos) = world.objects.get_component_mut::<Position>(&attacker_oid) {
        pos.heading = heading;
    } else if let Some(pos) = world.objects.get_component_mut::<Position>(&attacker_oid) {
        pos.heading = heading;
    }

    // Roll the hit (`generateHit`): miss → everything else skipped.
    let position = get_position(attacker.x, attacker.y, target.x, target.y, target.heading);
    let condition = world.data.hit_condition_bonus.condition_bonus(attacker.z, target.z, position);
    let miss_roll = world.roll(1000);
    let miss = formulas::calc_hit_miss(attacker.accuracy, target.evasion, condition, miss_roll);
    let (crit, damage) = if miss {
        (false, 0)
    } else {
        let crit_roll = world.roll(100);
        let crit = formulas::calc_auto_attack_crit(attacker.crit_stat, position, attacker.z, target.z, crit_roll);
        let r = attacker.random_dmg;
        let rand_roll = if r > 0 { world.roll(2 * r + 1) - r } else { 0 };
        let dmg = formulas::calc_auto_attack_damage(
            attacker.p_atk,
            formulas::random_damage_multiplier(rand_roll),
            position,
            target.p_def,
            crit,
        );
        (crit, dmg as i32)
    };

    let now = world.tick;
    if let Some(st) = world.objects.get_component_mut::<AttackState>(&attacker_oid) {
        st.attack_end_tick = now + ms_to_ticks(time_atk);
    }
    world.scheduler.schedule(
        now + ms_to_ticks(time_to_hit),
        ScheduledTask::AttackHit { attacker: attacker_oid, target: target_oid, damage, miss, crit },
    );
    // Swing-end hook for players (Java's `EVT_READY_TO_ACT` schedule in
    // `doAttack`): fires the action the swing held back, if any.
    if !is_npc_oid(attacker_oid) {
        world
            .scheduler
            .schedule(now + ms_to_ticks(time_atk), ScheduledTask::AttackFinish { object_id: attacker_oid });
    }

    // Broadcast the swing.
    let hit = server_packets::AttackHit { target_object_id: target_oid, damage, miss, crit };
    let pkt = server_packets::attack(attacker_oid, &hit, attacker.x, attacker.y, attacker.z, target.x, target.y, target.z);
    if is_npc_oid(attacker_oid) {
        let Some(region) = world.objects.get_component::<RegionCell>(&attacker_oid).map(|r| r.0) else { return };
        broadcast_near_region(world, region, &pkt);
    } else {
        broadcast_including_self(world, attacker_oid, &pkt);
        refresh_attack_stance(world, attacker_oid);
    }
}

/// `ScheduledTask::AttackHit` — `onHitTimeNotDual` + `onHitTarget`: the swing
/// lands (or misses).
pub(crate) fn handle_attack_hit(world: &mut World, attacker: i32, target: i32, damage: i32, miss: bool, crit: bool) {
    // Attacker died mid-swing → EVT_CANCEL (nothing lands).
    let attacker_alive = vitals_of(world, attacker).map(|v| !v.dead);
    if attacker_alive != Some(true) {
        return;
    }
    // Target dead/gone → skipped (Java's per-hit target check).
    let target_alive = vitals_of(world, target).map(|v| !v.dead);
    if target_alive != Some(true) {
        return;
    }

    if miss {
        // `sendDamageMessage(miss)` + `notifyAttackAvoid`.
        if let Some(client_id) = client_for_player(world, attacker) {
            let name = world.objects.get_component::<crate::model::Player>(&attacker).expect("player").name.clone();
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::system_message_with(sm_ids::C1_S_ATTACK_WENT_ASTRAY, &[SmParam::PlayerName(name)]));
            }
        }
        if let Some(client_id) = client_for_player(world, target) {
            let attacker_name = attacker_display_name(world, attacker);
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::system_message_with(
                    sm_ids::C1_HAS_EVADED_C2_S_ATTACK,
                    &[SmParam::PlayerName(world.objects.get_component::<crate::model::Player>(&target).expect("player").name.clone()), attacker_name],
                ));
            }
            refresh_attack_stance(world, target);
        }
        return;
    }

    // Crit + damage messages (`Player.sendDamageMessage`).
    if let Some(client_id) = client_for_player(world, attacker) {
        let attacker_name = world.objects.get_component::<crate::model::Player>(&attacker).expect("player").name.clone();
        let target_name = target_display_param(world, target);
        if let Some(cs) = world.clients.get(&client_id) {
            if crit {
                cs.send(server_packets::system_message_with(
                    sm_ids::C1_LANDED_A_CRITICAL_HIT,
                    &[SmParam::PlayerName(attacker_name.clone())],
                ));
            }
            cs.send(server_packets::system_message_with(
                sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
                &[SmParam::PlayerName(attacker_name), target_name, SmParam::Int(damage)],
            ));
        }
    }

    apply_physical_damage(world, attacker, target, damage as f64);
}

/// How an attacker shows up in the *victim's* damage messages ($c2).
fn attacker_display_name(world: &World, attacker: i32) -> SmParam {
    if let Some(p) = world.objects.get_component::<crate::model::Player>(&attacker) {
        SmParam::PlayerName(p.name.clone())
    } else if let Some(t) = world.objects.get_component::<crate::model::npc::Npc>(&attacker).and_then(|n| n.template(world)) {
        SmParam::NpcName(t.id)
    } else {
        SmParam::Text(String::new())
    }
}

/// How a target shows up in the *attacker's* damage messages ($c2).
fn target_display_param(world: &World, target: i32) -> SmParam {
    attacker_display_name(world, target)
}

/// Damage application shared by auto-attacks (and reusable by future physical
/// skills): route to the right victim kind, waking NPC AI / breaking player
/// casts / killing at 0 HP.
pub(crate) fn apply_physical_damage(world: &mut World, attacker: i32, target: i32, damage: f64) {
    if is_npc_oid(target) {
        npc_receive_damage(world, target, attacker, damage);
    } else {
        player_receive_damage(world, target, attacker, damage);
    }
}

/// `Attackable.reduceCurrentHp` → `addDamage`/`addDamageHate` + the
/// `onEvtAttacked` AI reaction, then the HP cut and `doDie`.
pub(crate) fn npc_receive_damage(world: &mut World, npc_oid: i32, attacker_oid: i32, damage: f64) {
    if world.objects.get_component::<Vitals>(&npc_oid).is_none_or(|v| v.dead) {
        return;
    }
    let level = match world.objects.get_component::<crate::model::npc::Npc>(&npc_oid) {
        Some(npc) => npc.template(world).map(|t| t.level).unwrap_or(1),
        None => return,
    };
    let now = world.tick;

    let mut became_running = false;
    let mut died = false;
    let (cur_hp, max_hp) = {
        let Some((mut aggro, mut ai, mut vitals, mut speeds)) = world
            .objects
            .get_many_mut::<(&mut AggroList, &mut NpcAi, &mut Vitals, &mut Speeds)>(&npc_oid)
        else {
            return;
        };
        // `addDamage`: hate = damage·100 / (level + 7); `onEvtAttacked`:
        // reset the calm-after-spawn counter, arm the attack timeout, run.
        let hate = damage * 100.0 / (level + 7) as f64;
        let entry = aggro.0.entry(attacker_oid).or_default();
        entry.damage += damage;
        entry.hate += hate;
        if ai.global_aggro < 0 {
            ai.global_aggro = 0;
        }
        ai.attack_timeout_tick = now + ATTACK_TIMEOUT_TICKS;
        if !speeds.running {
            speeds.running = true;
            became_running = true;
        }
        ai.intention = NpcIntention::Attack;

        vitals.cur_hp -= damage;
        if vitals.cur_hp <= 0.0 {
            vitals.cur_hp = 0.0;
            died = true;
        }
        (vitals.cur_hp as i32, vitals.max_hp)
    };
    let Some(region) = world.objects.get_component::<RegionCell>(&npc_oid).map(|r| r.0) else { return };

    if became_running {
        broadcast_near_region(world, region, &server_packets::change_move_type(npc_oid, true));
    }
    if died {
        super::death::npc_do_die(world, npc_oid, attacker_oid);
        return;
    }
    // `broadcastStatusUpdate` — the HP bar for everyone targeting it.
    broadcast_near_region(
        world,
        region,
        &server_packets::status_update(
            npc_oid,
            &[
                (server_packets::status_update_type::MAX_HP, max_hp),
                (server_packets::status_update_type::CUR_HP, cur_hp),
            ],
        ),
    );
}

/// `AttackableAI.MAX_ATTACK_TIMEOUT`: 1200 game ticks (120 s) without combat
/// activity against the target ends the chase.
pub(crate) const ATTACK_TIMEOUT_TICKS: u64 = 1200;

/// `PlayerStatus.reduceHp` for a physical hit: CP absorbs first only against
/// playable attackers (mobs bite straight into HP), casts can break
/// (`Formulas.calcAtkBreak`), 0 HP → `doDie`.
pub(crate) fn player_receive_damage(world: &mut World, player_oid: i32, attacker_oid: i32, damage: f64) {
    let attacker_is_playable = !is_npc_oid(attacker_oid);
    let mut died = false;
    let (cp_after, hp_after) = {
        let Some((mut vitals, mut pvitals)) =
            world.objects.get_many_mut::<(&mut Vitals, &mut PlayerVitals)>(&player_oid)
        else {
            return;
        };
        if vitals.dead {
            return;
        }
        let mut remaining = damage;
        if attacker_is_playable {
            let cp_absorb = remaining.min(pvitals.cur_cp);
            pvitals.cur_cp -= cp_absorb;
            remaining -= cp_absorb;
        }
        vitals.cur_hp -= remaining;
        if vitals.cur_hp <= 0.0 {
            vitals.cur_hp = 0.0;
            died = true;
        }
        (pvitals.cur_cp as i32, vitals.cur_hp as i32)
    };

    // Victim-side damage message + stance.
    if let Some(client_id) = client_for_player(world, player_oid) {
        let attacker_name = attacker_display_name(world, attacker_oid);
        let victim_name = world.objects.get_component::<crate::model::Player>(&player_oid).expect("player").name.clone();
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                sm_ids::C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2,
                &[SmParam::PlayerName(victim_name), attacker_name, SmParam::Int(damage as i32)],
            ));
        }
    }
    if !died {
        refresh_attack_stance(world, player_oid);
    }

    broadcast_including_self(
        world,
        player_oid,
        &server_packets::status_update(
            player_oid,
            &[
                (server_packets::status_update_type::CUR_CP, cp_after),
                (server_packets::status_update_type::CUR_HP, hp_after),
            ],
        ),
    );
    super::party::notify_party_vitals(world, player_oid);

    if died {
        super::death::player_do_die(world, player_oid, attacker_oid);
        return;
    }

    // Cast break on hit (`Formulas.calcAtkBreak`, same roll as the magic
    // damage path).
    let breakable = world
        .objects
        .get_component::<Casting>(&player_oid)
        .is_some_and(|c| !c.0.launched);
    if breakable {
        let men_bonus = {
            let men = world.objects.get_component::<crate::model::components::BaseStats>(&player_oid).map(|b| b.men).unwrap_or(0);
            world.data.stat_bonus.bonus(BaseStat::Men, men)
        };
        let break_roll = world.roll(100);
        if formulas::calc_atk_break(damage, men_bonus, break_roll) {
            abort_cast(world, player_oid);
            if let Some(client_id) = client_for_player(world, player_oid) {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(server_packets::system_message_with(sm_ids::YOUR_CASTING_HAS_BEEN_INTERRUPTED, &[]));
                }
            }
        }
    }
}
