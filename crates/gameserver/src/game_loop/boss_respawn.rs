//! Raid-boss persistence — port of `DBSpawnManager` (the `npc_respawns` table).
//!
//! A `dbSave="true"` spawn's boss keeps its **live HP/MP** and its **pending
//! respawn time** across a server restart. 225 spawns on this dist carry the
//! flag, all raid bosses in `RaidbossSpawns.xml`; before this they were placed
//! like ordinary static spawns, so every restart handed players a fresh
//! full-HP boss and wiped any respawn timer.
//!
//! **Ownership split, straight from Java.** `NpcSpawnTemplate.spawnNpc` sends a
//! `dbSave` spawn to `DBSpawnManager.addNewSpawn` instead of spawning it, and
//! only when `!DBSpawnManager.isDefined(id)` — i.e. the DB owns these NPCs and
//! the static spawn pass defers to it. The port mirrors that: [`spawn_all`]
//! collects `db_save` definitions into `World.pending_boss_spawns` rather than
//! placing them, and [`resolve_boot`] settles them once the `npc_respawns` rows
//! arrive from the DB thread. That keeps boot asynchronous (no blocking wait on
//! the DB) while preserving the "DB wins" rule.
//!
//! [`spawn_all`]: crate::model::npc::spawn_all

use crate::db::{DbCommand, NpcRespawnRow};
use crate::model::components::{Position, Vitals};
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// Java stores absolute unix millis in `npc_respawns.respawnTime`, with 0
/// meaning "alive". The world clock is a tick counter, so the two are bridged
/// here at boot (and only here).
fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// `DBSpawnManager.load` + the deferred half of `NpcSpawnTemplate.spawnNpc`:
/// settle every `dbSave` spawn against its stored row.
///
/// Three cases per boss:
/// - **row says it's still dead** (`respawnTime` in the future) → don't spawn;
///   schedule the spawn for when it's due.
/// - **row says it's alive** → spawn and restore the stored HP/MP, so a boss
///   left at 12 % comes back at 12 %.
/// - **no row** (fresh DB, or a boss newly flagged `dbSave`) → spawn full and
///   insert a row, matching Java's `addNewSpawn(..., storeInDb = true)`.
pub(crate) fn resolve_boot(world: &mut World, rows: Vec<NpcRespawnRow>) {
    let pending = std::mem::take(&mut world.pending_boss_spawns);
    let now = now_millis();
    let mut spawned = 0usize;
    let mut scheduled = 0usize;

    for spawn_ref in pending {
        let Some(npc_id) = world
            .data
            .spawn_data
            .spawns
            .get(spawn_ref.0)
            .and_then(|t| t.groups.get(spawn_ref.1))
            .and_then(|g| g.npcs.get(spawn_ref.2))
            .map(|d| d.npc_id)
        else {
            continue;
        };
        let row = rows.iter().find(|r| r.npc_id == npc_id).copied();

        match row {
            // Still dead: put it back on the clock instead of on the map.
            Some(r) if r.respawn_time > now => {
                let delay_ticks = ((r.respawn_time - now) / 100).max(1) as u64;
                world
                    .scheduler
                    .schedule(world.tick + delay_ticks, ScheduledTask::BossRespawn { spawn_ref });
                world.boss_spawn_refs.insert(npc_id, spawn_ref);
                scheduled += 1;
            }
            // Alive (or due): place it, restoring stored vitals when we have them.
            _ => {
                if let Some(oid) = crate::model::npc::spawn_one(world, spawn_ref.0, spawn_ref.1, spawn_ref.2) {
                    if let Some(r) = row {
                        restore_vitals(world, oid, r.cur_hp, r.cur_mp);
                    }
                    world.boss_spawn_refs.insert(npc_id, spawn_ref);
                    persist_alive(world, npc_id, oid);
                    spawned += 1;
                }
            }
        }
    }

    if spawned + scheduled > 0 {
        tracing::info!("DBSpawnManager: {spawned} raid bosses spawned, {scheduled} awaiting respawn.");
    }
}

/// Clamp the stored vitals onto a freshly spawned boss. A stored value above
/// the template maximum (a datapack buff since the row was written) clamps
/// down rather than over-filling the bar.
fn restore_vitals(world: &mut World, oid: i32, cur_hp: f64, cur_mp: f64) {
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) {
        // A row written for a *dead* boss holds 0 HP; spawning it at 0 would
        // kill it on the next tick, so only a positive value is restored.
        if cur_hp > 0.0 {
            v.cur_hp = cur_hp.min(v.max_hp as f64);
        }
        if cur_mp > 0.0 {
            v.cur_mp = cur_mp.min(v.max_mp as f64);
        }
    }
}

/// `DBSpawnManager.updateStatus` for a living boss: `respawnTime = 0` plus its
/// current vitals and position.
pub(crate) fn persist_alive(world: &World, npc_id: i32, oid: i32) {
    let Some(pos) = world.objects.get_component::<Position>(&oid).copied() else { return };
    let (cur_hp, cur_mp) = world
        .objects
        .get_component::<Vitals>(&oid)
        .map(|v| (v.cur_hp, v.cur_mp))
        .unwrap_or((0.0, 0.0));
    let _ = world.db.send(DbCommand::StoreNpcRespawn {
        npc_id,
        x: pos.x,
        y: pos.y,
        z: pos.z,
        heading: pos.heading,
        respawn_time: 0,
        cur_hp,
        cur_mp,
    });
}

/// `DBSpawnManager.updateStatus` for a killed boss: bank the absolute time it's
/// due back, so a restart in between doesn't reset the wait.
pub(crate) fn persist_death_at(world: &World, npc_id: i32, pos: Position, respawn_delay_secs: i32) {
    let _ = world.db.send(DbCommand::StoreNpcRespawn {
        npc_id,
        x: pos.x,
        y: pos.y,
        z: pos.z,
        heading: pos.heading,
        respawn_time: now_millis() + respawn_delay_secs as i64 * 1000,
        cur_hp: 0.0,
        cur_mp: 0.0,
    });
}

/// Is this NPC one the DB owns? (`NpcSpawnDef.db_save` on its spawn line.)
pub(crate) fn is_db_saved(world: &World, spawn_ref: (usize, usize, usize)) -> bool {
    world
        .data
        .spawn_data
        .spawns
        .get(spawn_ref.0)
        .and_then(|t| t.groups.get(spawn_ref.1))
        .and_then(|g| g.npcs.get(spawn_ref.2))
        .is_some_and(|d| d.db_save)
}

/// `ScheduledTask::BossRespawn` — a boss whose stored respawn time came due
/// while the server was running (scheduled at boot by [`resolve_boot`]).
pub(crate) fn handle_boss_respawn(world: &mut World, spawn_ref: (usize, usize, usize)) {
    if let Some(oid) = crate::model::npc::spawn_one(world, spawn_ref.0, spawn_ref.1, spawn_ref.2) {
        if let Some(npc_id) = world.objects.get_component::<crate::model::npc::Npc>(&oid).map(|n| n.npc_id) {
            persist_alive(world, npc_id, oid);
        }
    }
}

/// Shutdown flush (`DBSpawnManager.updateDb`): write every living boss's
/// current HP/MP so the restart picks up where the fight left off.
pub(crate) fn save_all_bosses(world: &mut World) {
    // Collect first: the store can't be queried while a write borrow is out,
    // and `persist_alive` reads it again per boss.
    let mut live: Vec<(i32, i32)> = Vec::new();
    world.objects.for_each_mut::<(&crate::model::npc::Npc, &Vitals)>(|(n, v)| {
        if !v.dead {
            live.push((n.npc_id, n.object_id));
        }
    });
    for (npc_id, oid) in live {
        if world.boss_spawn_refs.contains_key(&npc_id) {
            persist_alive(world, npc_id, oid);
        }
    }
}
