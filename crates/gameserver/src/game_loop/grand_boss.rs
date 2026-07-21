//! The grand-boss respawn lifecycle — the part every `ai/bosses` script shares.
//!
//! Each script's `onKill` does the same four things: mark the boss **dead**,
//! roll a respawn window, **persist** it so a restart doesn't forget, and arm a
//! timer to bring the boss back. Java repeats that block in every boss file;
//! the port keeps it here once, driven by `GrandBoss.ini`.
//!
//! What stays per-boss is the interesting part — Queen Ant's nurses, Antharas's
//! phases, Baium's sleep — not the bookkeeping.

use commons::util::now_millis;

use crate::db::DbCommand;
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// Java's `ALIVE`/`DEAD` for the simple bosses (Queen Ant, Orfen, Core).
/// Antharas/Valakas/Baium use a wider ladder; this slice only drives the
/// two-state ones.
pub const ALIVE: i32 = 0;
pub const DEAD: i32 = 1;

const TICKS_PER_SECOND: u64 = 10;
const MILLIS_PER_HOUR: i64 = 3_600_000;

/// `GrandBossManager.getStatus`.
pub(crate) fn status(world: &World, boss_id: i32) -> Option<i32> {
    world.grand_bosses.get(&boss_id).map(|b| b.status)
}

/// A grand boss died: mark it dead, roll the respawn window, persist, and arm
/// the timer.
///
/// The window is Java's `(interval + getRandom(-random, random)) * 3600000`, so
/// a boss is never up at a predictable hour — the whole point of the spread.
pub(crate) fn on_grand_boss_killed(world: &mut World, boss_id: i32) {
    let Some(window) = world.cfg.grand_boss.window_for(boss_id) else { return };
    // `getRandom(-r, r)` is inclusive on both ends.
    let spread = if window.random_hours > 0 {
        world.roll(window.random_hours * 2 + 1) - window.random_hours
    } else {
        0
    };
    let hours = (window.interval_hours + spread).max(1);
    let respawn_millis = hours as i64 * MILLIS_PER_HOUR;

    if let Some(b) = world.grand_bosses.get_mut(&boss_id) {
        b.status = DEAD;
        b.respawn_time = now_millis() + respawn_millis;
        // Java stores the *stored* HP/MP as full so a restored boss comes back
        // whole; a dead boss's live HP is meaningless.
        b.current_hp = 0.0;
        b.current_mp = 0.0;
    }
    persist(world, boss_id);

    world.scheduler.schedule(
        world.tick + (respawn_millis / 1000) as u64 * TICKS_PER_SECOND,
        ScheduledTask::GrandBossRespawn { boss_id },
    );
}

/// The `*_unlock` timer firing — the boss comes back.
pub(crate) fn handle_grand_boss_respawn(world: &mut World, boss_id: i32) {
    // Already alive (an admin spawned it, or a duplicate timer): do nothing
    // rather than stacking a second boss.
    if status(world, boss_id) == Some(ALIVE) {
        return;
    }
    if let Some(b) = world.grand_bosses.get_mut(&boss_id) {
        b.status = ALIVE;
        b.respawn_time = 0;
    }
    persist(world, boss_id);
    spawn_from_record(world, boss_id);
}

/// Boot resolution — Java's `onSpawn`/init block, which is a three-way branch
/// and the reason boss state is in a table at all:
///
/// - **alive**: spawn at the stored location with the stored HP/MP.
/// - **dead, timer still running**: arm the remaining time.
/// - **dead, timer already expired while the server was down**: spawn now.
///
/// The third case is the one that is easy to miss and the most visible when
/// missing: a boss whose window elapsed during downtime would otherwise stay
/// dead until someone killed it again, which cannot happen.
pub(crate) fn resolve_at_boot(world: &mut World) {
    let ids: Vec<i32> = world.grand_bosses.keys().copied().collect();
    for boss_id in ids {
        if world.cfg.grand_boss.window_for(boss_id).is_none() {
            continue; // not a boss this slice drives
        }
        match status(world, boss_id) {
            Some(s) if s == DEAD => {
                let remaining = world
                    .grand_bosses
                    .get(&boss_id)
                    .map(|b| b.respawn_time - now_millis())
                    .unwrap_or(0);
                if remaining > 0 {
                    world.scheduler.schedule(
                        world.tick + (remaining / 1000).max(1) as u64 * TICKS_PER_SECOND,
                        ScheduledTask::GrandBossRespawn { boss_id },
                    );
                } else {
                    handle_grand_boss_respawn(world, boss_id);
                }
            }
            Some(_) => spawn_from_record(world, boss_id),
            None => {}
        }
    }
}

/// Place the boss at its stored location with its stored vitals.
fn spawn_from_record(world: &mut World, boss_id: i32) {
    let Some(b) = world.grand_bosses.get(&boss_id).cloned() else { return };
    let Some(oid) = crate::model::npc::spawn_npc_at(world, boss_id, b.loc_x, b.loc_y, b.loc_z, b.heading) else {
        return;
    };
    // Per-boss script hooks. Queen Ant brings out her larva and starts the
    // nurse rotation; the other nine bosses have no script yet.
    if boss_id == crate::game_loop::queen_ant::QUEEN {
        crate::game_loop::queen_ant::on_queen_spawned(world, oid);
    }
    if boss_id == crate::game_loop::core_boss::CORE {
        crate::game_loop::core_boss::on_core_spawned(world);
    }
    if boss_id == crate::game_loop::baium::BAIUM {
        crate::game_loop::baium::on_baium_spawned(world);
    }
    if boss_id == crate::game_loop::antharas::ANTHARAS {
        // The cinematic runs first; its tail starts the minion waves, so a
        // boss that has not yet been engaged is not already spawning adds.
        crate::game_loop::antharas::begin_cinematic(world, oid);
    }

    // A stored HP of 0 means "was never wounded" (a fresh respawn), so only a
    // positive value overrides the template's full vitals.
    if b.current_hp > 0.0 {
        if let Some(v) = world.objects.get_component_mut::<crate::model::components::Vitals>(&oid) {
            v.cur_hp = b.current_hp.min(v.max_hp as f64);
            v.cur_mp = b.current_mp.min(v.max_mp as f64);
        }
    }
}

fn persist(world: &mut World, boss_id: i32) {
    if let Some(b) = world.grand_bosses.get(&boss_id).cloned() {
        let _ = world.db.send(DbCommand::StoreGrandBoss { boss: b });
    }
}
