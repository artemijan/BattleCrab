//! Persistence: per-action saves, the autosave chain, shutdown flush and
//! the clan-leader notification.

use super::castle_owner_clan_id;
use crate::game_loop::helpers::send_sm_bare_to_player;
use crate::game_loop::time::TICKS_PER_SECOND;
/// `AltManorSaveAllActions` — write this castle's rows now if the operator
/// asked for per-action persistence. With it off (this dist) the setup rides
/// in memory until [`handle_autosave`] or the shutdown sweep, which is what
/// Java does too.
use crate::scheduler::ScheduledTask;
use crate::world::World;
pub(super) fn save_after_action(world: &World, castle_id: i32) {
    if world.cfg.general.alt_manor_save_all_actions {
        store_manor(world, castle_id);
    }
}

/// The autosave timer (`ThreadPool.scheduleAtFixedRate(this::storeMe, rate,
/// rate)`): persist every castle's manor, then re-arm.
///
/// Java's `storeMe` walks all castles, not just the one that changed — a
/// per-castle save here would leave any castle whose owner set up during the
/// previous window unwritten.
pub(crate) fn handle_autosave(world: &mut World) {
    for castle_id in world.castles.iter().map(|c| c.id).collect::<Vec<_>>() {
        store_manor(world, castle_id);
    }
    arm_autosave(world);
}

/// Arm the autosave, if the config wants one. Called at boot and after each
/// run. Java only schedules this when per-action saving is **off**.
pub(crate) fn arm_autosave(world: &mut World) {
    if world.cfg.general.alt_manor_save_all_actions {
        return;
    }
    let hours = world.cfg.general.alt_manor_save_period_rate.max(1) as u64;
    let delay = hours * 3600 * TICKS_PER_SECOND;
    world
        .scheduler
        .schedule(world.tick + delay, ScheduledTask::ManorAutosave);
}

/// `Shutdown`: "Save all manor data", guarded by `!ALT_MANOR_SAVE_ALL_ACTIONS`.
pub(crate) fn save_all_on_shutdown(world: &World) {
    if world.cfg.general.alt_manor_save_all_actions {
        return; // already written per action
    }
    for castle in &world.castles {
        store_manor(world, castle.id);
    }
}

/// `CastleManorManager.storeMe` for one castle — write all four period lists.
pub(super) fn store_manor(world: &World, castle_id: i32) {
    let mut production = Vec::new();
    let mut procure = Vec::new();
    for next_period in [false, true] {
        production.extend(
            world
                .manor
                .seed_production(castle_id, next_period)
                .iter()
                .map(|sp| crate::db::ManorProductionRow {
                    castle_id,
                    seed_id: sp.seed_id,
                    amount: sp.amount,
                    start_amount: sp.start_amount,
                    price: sp.price,
                    next_period,
                }),
        );
        procure.extend(
            world
                .manor
                .crop_procure(castle_id, next_period)
                .iter()
                .map(|cp| crate::db::ManorProcureRow {
                    castle_id,
                    crop_id: cp.crop_id,
                    amount: cp.amount,
                    start_amount: cp.start_amount,
                    price: cp.price,
                    reward_type: cp.reward_type,
                    next_period,
                }),
        );
    }
    let _ = world.db.send(crate::db::DbCommand::StoreManor {
        castle_id,
        production,
        procure,
    });
}

/// Java's `clanLeader.isOnline()` notification — send `message_id` to the owner
/// clan's leader if they are logged in.
pub(super) fn notify_leader(world: &World, castle_id: i32, message_id: i16) {
    let Some(leader_oid) = castle_owner_clan_id(world, castle_id)
        .and_then(|clan_id| world.clans.get(&clan_id))
        .map(|clan| clan.leader_id)
        .filter(|&oid| oid != 0)
    else {
        return;
    };
    send_sm_bare_to_player(world, leader_oid, message_id);
}
