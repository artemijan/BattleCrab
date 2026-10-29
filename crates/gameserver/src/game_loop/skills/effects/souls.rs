//! Souls — the Kamael resource (`Player._souls`): absorbed by `SoulEating`
//! from exp gains, granted by `FocusSouls`, spent by a cast's
//! `soulMaxConsumeCount` and read by `SoulBlow`, `SoulSaved` and `OpSoulMax`.

use crate::game_loop::World;
use crate::game_loop::helpers;
use crate::model::Player;
use crate::model::skill::effects::SkillEffect;
use crate::model::stats::Stat;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::scheduler::ScheduledTask;

/// `(int) getStat().getValue(Stat.MAX_SOULS, 0)` — base 0, raised only by
/// `SoulEating`'s pump.
pub(crate) fn max_souls(world: &World, player_oid: i32) -> i32 {
    world
        .objects
        .get_component::<crate::model::components::stats::StatModifiers>(&player_oid)
        .map_or(0.0, |m| {
            crate::model::stat_finalize::finalize(m, Stat::MaxSouls, 0.0)
        }) as i32
}

pub(crate) fn charged_souls(world: &World, player_oid: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&player_oid)
        .map_or(0, |p| p.souls)
}

fn set_souls(world: &mut World, player_oid: i32, souls: i32) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.souls = souls;
    }
}

fn send_etc_status_update(world: &World, player_oid: i32) {
    if let Some(client_id) = helpers::client_for_player(world, player_oid) {
        helpers::send_etc_status_update(world, client_id, player_oid);
    }
}

/// `restartSoulTask` / `stopSoulTask`: the bump invalidates the pending
/// `ResetSoulsTask`; a new one is armed only when `restart`.
fn arm_soul_decay(world: &mut World, player_oid: i32, restart: bool) {
    /// `ThreadPool.schedule(new ResetSoulsTask(this), 600000)`.
    const SOUL_DECAY_TICKS: u64 = 6_000;

    let Some(seq) = world
        .objects
        .get_component_mut::<Player>(&player_oid)
        .map(|p| {
            p.souls_seq = p.souls_seq.wrapping_add(1);
            p.souls_seq
        })
    else {
        return;
    };
    if restart {
        world.scheduler.schedule(
            world.tick + SOUL_DECAY_TICKS,
            ScheduledTask::ResetSouls { player_oid, seq },
        );
    }
}

/// `Player.increaseSouls`.
pub(crate) fn increase_souls(world: &mut World, player_oid: i32, count: i32) {
    let souls = charged_souls(world, player_oid) + count;
    set_souls(world, player_oid, souls);
    if let Some(client_id) = helpers::client_for_player(world, player_oid) {
        helpers::send_sm_to_client(
            world,
            client_id,
            sm_ids::YOUR_SOUL_COUNT_HAS_INCREASED_BY_S1_IT_IS_NOW_AT_S2,
            &[SmParam::Int(count), SmParam::Int(souls)],
        );
    }
    arm_soul_decay(world, player_oid, true);
    send_etc_status_update(world, player_oid);
}

/// `Player.decreaseSouls` — floors at 0 and, unlike `decreaseCharges`, never
/// refuses.
pub(crate) fn decrease_souls(world: &mut World, player_oid: i32, count: i32) {
    let souls = (charged_souls(world, player_oid) - count).max(0);
    set_souls(world, player_oid, souls);
    arm_soul_decay(world, player_oid, souls != 0);
    send_etc_status_update(world, player_oid);
}

/// `Player.clearSouls`.
pub(crate) fn clear_souls(world: &mut World, player_oid: i32) {
    set_souls(world, player_oid, 0);
    arm_soul_decay(world, player_oid, false);
    send_etc_status_update(world, player_oid);
}

/// `ResetSoulsTask.run` → `clearSouls`, unless a later change replaced it.
pub(crate) fn reset_souls(world: &mut World, player_oid: i32, seq: u64) {
    let stale = world
        .objects
        .get_component::<Player>(&player_oid)
        .is_none_or(|p| p.souls_seq != seq);
    if !stale {
        clear_souls(world, player_oid);
    }
}

/// `FocusSouls.instant`: `charge` more souls, up to `MAX_SOULS`.
pub(crate) fn focus_souls(world: &mut World, target_oid: i32, charge: i32) {
    if !world.objects.has_component::<Player>(&target_oid)
        || helpers::is_alike_dead(world, target_oid)
    {
        return;
    }
    let max = max_souls(world, target_oid);
    if max <= 0 {
        return;
    }
    let current = charged_souls(world, target_oid);
    if current < max {
        let count = if current + charge <= max {
            charge
        } else {
            max - current
        };
        increase_souls(world, target_oid, count);
    } else if let Some(client_id) = helpers::client_for_player(world, target_oid) {
        helpers::send_sm_bare_to_client(world, client_id, sm_ids::SOUL_CANNOT_BE_INCREASED_ANYMORE);
    }
}

/// `SoulEating.onExperienceReceived` — Java's `ON_PLAYABLE_EXP_CHANGED`
/// listener, registered while the passive is in effect. `exp` is the gain
/// `PlayableStat.addExp` reports (`newExp - oldExp`).
pub(crate) fn on_exp_gained(world: &mut World, player_oid: i32, exp: i64) {
    let Some(exp_needed) = soul_eating_exp_needed(world, player_oid) else {
        return;
    };
    if exp < exp_needed {
        return;
    }
    if charged_souls(world, player_oid) >= max_souls(world, player_oid) {
        if let Some(client_id) = helpers::client_for_player(world, player_oid) {
            helpers::send_sm_bare_to_client(
                world,
                client_id,
                sm_ids::SOUL_CANNOT_BE_ABSORBED_ANYMORE,
            );
        }
        return;
    }
    increase_souls(world, player_oid, 1);
    if let Some(npc_oid) = crate::game_loop::combat::target::current(world, player_oid)
        .filter(|&t| helpers::is_npc(world, t))
    {
        crate::game_loop::net::broadcast::broadcast_including_self(
            world,
            player_oid,
            &server_packets::ex_spawn_emitter(npc_oid, player_oid),
        );
    }
}

/// The `expNeeded` of the learned `SoulEating` passive, if any.
fn soul_eating_exp_needed(world: &World, player_oid: i32) -> Option<i64> {
    let book = world
        .objects
        .get_component::<crate::model::components::skills::SkillBook>(&player_oid)?;
    book.0.iter().find_map(|(&id, &level)| {
        world
            .data
            .skill_data
            .get(id, level)?
            .effects
            .iter()
            .find_map(|e| match e {
                SkillEffect::SoulEating { exp_needed } => Some(*exp_needed),
                _ => None,
            })
    })
}
