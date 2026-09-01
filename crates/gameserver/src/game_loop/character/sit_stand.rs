//! Sitting — `Player.sitDown()` / `standUp()` plus the `SitStand` player
//! action (`ActionData.xml` id 0, the `/sit` and `/stand` commands).
//!
//! The port had no seated state at all, which left a trail of TODOs across five
//! milestones: the regen bonus (`MoveType::Sitting`), the `/mount` refusal, the
//! crafting `sitDown()`, the transform condition's sitting leg and the offline
//! shop's. This is the one mechanism behind all of them.
//!
//! **Sitting is two-phase, and the two phases are not the same predicate.**
//! Java's `sitDown` flips `_waitTypeSitting` *immediately* and blocks actions
//! for 2.5 s while the animation plays; `standUp` broadcasts first and only
//! clears the flag 2.5 s later, from `StandUpTask`. So there is a window where
//! a character is seated but already un-blocked, and another where they are
//! standing up but still count as sitting. Anything reading "is seated" (regen,
//! the cast/mount refusals) follows the **flag**; anything asking "may they
//! act" follows the block.
//!
//! Not ported: the **chair** branch (`ChairSit` on a type-1 `StaticObject`) —
//! `SitStand` only takes it when the player has such an object targeted and in
//! range, and no throne/bench is interactive on this dist; and the queued
//! sit-on-arrival `NextAction`, which needs an AI next-action slot.

use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::send_sm_to_player;
use crate::game_loop::space::position::maybe_position;
use crate::model::Player;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// `ChangeWaitType.WT_SITTING` / `WT_STANDING`.
const WT_SITTING: i32 = 0;
const WT_STANDING: i32 = 1;

/// Java schedules both `SitDownTask` and `StandUpTask` 2500 ms out; the loop
/// runs at 10 ticks/s.
const ANIMATION_TICKS: u64 = 25;

/// Java `Player.isSitting()`.
pub(crate) fn is_sitting(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.sitting)
}

/// Java's `AI_INTENTION_REST` gate — true while the player's AI intention is
/// REST, i.e. from `sitDown` until `StandUpTask` sets IDLE 2.5 s after the
/// stand-up broadcast. That window is exactly the seated flag's lifetime
/// (`sitDown` sets both, `StandUpTask` clears both), so the flag stands in for
/// the intention the port doesn't model.
///
/// `CreatureAI` refuses `MOVE_TO`, `FOLLOW`, `PICK_UP`, `INTERACT` and (magic)
/// `CAST` outright while resting, each with a bare `clientActionFailed()`;
/// `PlayableAI.onIntentionAttack` refuses `ATTACK` on `isSitting()` with **no**
/// packet at all; and `Player.useMagic` refuses every skill on
/// `_waitTypeSitting` with a system message. Without these the port let a
/// seated player cast, swing and run the moment the 2.5 s `SitBlock` animation
/// block expired.
pub(crate) fn is_resting(world: &World, object_id: i32) -> bool {
    is_sitting(world, object_id)
}

/// The `SitStand` player action (id 0) — Java's `useSit`, minus the chair
/// branch: toggle between seated and standing.
pub(crate) fn handle_sit_stand(world: &mut World, object_id: i32) {
    // `if (player.getMountType() != MountType.NONE) return false;` — a rider
    // cannot sit, and the toggle does nothing at all rather than dismounting.
    if world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(Player::is_mounted)
    {
        return;
    }
    // `if (player.isFakeDeath()) stopEffects(FAKE_DEATH)` — the toggle is how
    // a rogue gets up off the floor, and it does *not* also sit them down.
    if crate::game_loop::abnormal::flags_of(world, object_id)
        & crate::model::skill::effect_flag::FAKE_DEATH
        != 0
    {
        crate::game_loop::skills::effects::stop_fake_death(world, object_id);
        return;
    }
    if is_sitting(world, object_id) {
        stand_up(world, object_id);
    } else {
        sit_down(world, object_id);
    }
}

/// Java `Player.sitDown(checkCast = true)`.
pub(crate) fn sit_down(world: &mut World, object_id: i32) {
    // "Cannot sit while casting." — a plain message, not a SystemMessage.
    if world
        .objects
        .has_component::<crate::model::components::Casting>(&object_id)
    {
        send_sm_to_player(
            world,
            object_id,
            server_packets::sm_ids::S1_TEXT,
            &[server_packets::SmParam::Text(
                "Cannot sit while casting.".into(),
            )],
        );
        return;
    }
    // `!_waitTypeSitting && !isAttackDisabled() && !isControlBlocked() &&
    // !isImmobilized() && !isFishing()`.
    //
    // `isAttackDisabled()` is `isAttackingNow() || isDisabled()`, and only the
    // second half was ported: mid-swing counts too, so the request is dropped
    // until the swing lands rather than sitting the attacker down out from
    // under it. `isImmobilized()` reads Java's `_isImmobilized` flag, which
    // nothing sets on a player (the port's `Immobilized` component is
    // NPC-only — Core, Baium, Sailren), so it has no leg here.
    let mid_swing = world
        .objects
        .get_component::<crate::model::components::AttackState>(&object_id)
        .is_some_and(|st| st.attack_end_tick > world.tick);
    if is_sitting(world, object_id)
        || mid_swing
        || crate::game_loop::abnormal::is_blocked_from_actions(world, object_id)
        || crate::game_loop::abnormal::is_control_blocked(world, object_id)
        || world
            .objects
            .has_component::<crate::model::components::FishingSession>(&object_id)
    {
        return;
    }

    // `breakAttack()` — sitting cancels the swing you were winding up, and the
    // standing intention with it. It calls `abortAttack()`, which ends *the
    // current swing only*: the 15 s combat stance in `AttackStanceTaskManager`
    // is a separate map that `sitDown` never touches, so a player who sits
    // mid-fight keeps their sword drawn until the stance times out normally.
    //
    // Clearing `attack_end_tick` rather than dropping the whole `AttackState`
    // is load-bearing: that component *also* carries `stance_until_tick`, and
    // removing it took the player out of the stance sweep for good — the 15 s
    // `AutoAttackStop` never fired (stance stuck on the client forever) and
    // `refresh_attack_stance`'s `if let Some` write silently no-opped from
    // then on, so no later fight could put them back in stance either.
    if let Some(st) = world
        .objects
        .get_component_mut::<crate::model::components::AttackState>(&object_id)
    {
        st.attack_end_tick = 0;
    }
    world
        .objects
        .remove_component::<crate::model::components::Intent>(&object_id);
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.sitting = true;
    }
    // `AI_INTENTION_REST`, which for a player is `PlayerAI.onIntentionRest`:
    // `changeIntention(REST)` + `setTarget(null)` + `clientStopMoving(null)`.
    // The port has no intention slot for REST, so the seated flag *is* the
    // intention: `is_resting` below reads it, and every intention Java refuses
    // while resting is gated on it.
    world
        .objects
        .remove_component::<crate::model::components::Movement>(&object_id);
    crate::game_loop::combat::target::drop_target_notify(world, object_id);
    broadcast_wait_type(world, object_id, WT_SITTING);
    // `setBlockActions(true)` for the 2.5 s animation.
    set_action_block(world, object_id, true);
    world.scheduler.schedule(
        world.tick + ANIMATION_TICKS,
        ScheduledTask::SitDownFinish { object_id },
    );
}

/// Java `Player.standUp()`.
pub(crate) fn stand_up(world: &mut World, object_id: i32) {
    // `_waitTypeSitting && !isInStoreMode() && !isAlikeDead()` — a private
    // store keeps its owner seated; closing the store is what stands them up.
    let refuses = !is_sitting(world, object_id)
        || world
            .objects
            .get_component::<Player>(&object_id)
            .is_some_and(|p| p.store_type != 0)
        || is_dead(world, object_id);
    if refuses {
        return;
    }
    // Java `stopEffects(EffectFlag.RELAXING)` — standing up ends the Relax
    // toggle (226), which only runs while seated. The tick's own "not sitting"
    // gate would also catch this, but not until the next tick fires: without
    // this the player keeps paying MP upkeep for up to a full interval after
    // standing, and the buff icon lingers that long too.
    stop_relaxing(world, object_id);
    broadcast_wait_type(world, object_id, WT_STANDING);
    // The flag clears only when the animation finishes, not now.
    world.scheduler.schedule(
        world.tick + ANIMATION_TICKS,
        ScheduledTask::StandUpFinish { object_id },
    );
}

/// End any active Relax-style toggle (Java's `EffectFlag.RELAXING`).
///
/// The port has no effect-flag index over live buffs, so membership is decided
/// the same way the tick does: by looking for the effect on the buff's skill.
fn stop_relaxing(world: &mut World, object_id: i32) {
    crate::game_loop::skills::effects::expire_buffs_where(world, object_id, |world, buff| {
        !buff.passive
            && world
                .data
                .skill_data
                .get(buff.skill_id, buff.skill_level)
                .is_some_and(|s| {
                    s.effects.iter().any(|e| {
                        // Java stops everything carrying `EffectFlag.RELAXING`,
                        // which is `Relax` and `ChameleonRest` — the latter also
                        // carries SILENT_MOVE, but it is the RELAXING half that
                        // standing up cancels.
                        matches!(
                            e,
                            crate::model::skill::SkillEffect::Relax { .. }
                                | crate::model::skill::SkillEffect::ChameleonRest { .. }
                        )
                    })
                })
    });
}

/// `SitDownTask` — the animation is over, actions unblock. The character stays
/// seated.
pub(crate) fn handle_sit_down_finish(world: &mut World, object_id: i32) {
    set_action_block(world, object_id, false);
}

/// `StandUpTask` — now they are really up.
pub(crate) fn handle_stand_up_finish(world: &mut World, object_id: i32) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.sitting = false;
    }
    // Standing changes the regen move-type, so the client's status needs a
    // refresh (Java's AI intention change broadcasts one).
    super::player_info::broadcast_user_info(world, object_id);
}

/// Java `Player.setBlockActions` — the port keeps the seated block as its own
/// flag rather than an abnormal, since no buff grants it.
fn set_action_block(world: &mut World, object_id: i32, blocked: bool) {
    use crate::model::components::SitBlock;
    if blocked {
        world.objects.add_components(&object_id, SitBlock);
    } else {
        world.objects.remove_component::<SitBlock>(&object_id);
    }
}

fn broadcast_wait_type(world: &World, object_id: i32, wait_type: i32) {
    let Some(pos) = maybe_position(world, object_id) else {
        return;
    };
    let pkt = server_packets::change_wait_type(object_id, wait_type, pos.x, pos.y, pos.z);
    crate::game_loop::helpers::broadcast_including_self(world, object_id, &pkt);
}
