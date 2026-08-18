//! Boot-time world population and the shutdown flush — the ordered subsystem
//! registry `run` used to inline.

use super::*;

/// The boot sequence: place the static world content and arm the self-
/// rescheduling cycles before accepting anyone in.
pub(super) fn boot(world: &mut World) {
    // Java `GameServer`: SpawnData.getInstance().init() — place the static
    // world content before accepting anyone in.
    crate::model::npc::spawn_all(world);
    // DoorData's boot spawn (entities + BY_TIME cycles; the collision grid
    // was registered into the GeoEngine in main.rs, before it was shared).
    crate::model::door::spawn_doors(world);
    doors::start_time_cycles(world);
    crate::model::static_object::spawn_static_objects(world);
    // Java `DailyTaskManager`: the daily 06:30 reset (recommends + vitality
    // refill). Scheduled once here; the task reschedules itself every 24 h.
    daily_tasks::schedule_initial_daily_reset(world);
    bot_report::schedule_initial_points_reset(world);
    restart::schedule_server_restart(world);
    // Grand bosses spawn/respawn once their data lands — the `grandboss_data`
    // table arrives asynchronously as `DbEvent::GrandBossesLoaded`, so
    // `grand_boss::resolve_at_boot` (and `dr_chaos`) run from that handler, not
    // here where `world.grand_bosses` is still empty.
    boats::spawn_boats(world);
    // Script-owned area NPCs (Toma is not in the spawn data — his script
    // places and relocates him).
    area_npcs::spawn_at_boot(world);
    // Each event's `config.xml` cron schedule (Java's per-event `loadConfig`).
    events::schedule_at_boot(world);
    // Java `CreatureSeeTaskManager`: the 1 s creature-see scan behind
    // `addCreatureSeeId` (G22). Armed once; the sweep re-arms itself.
    world
        .scheduler
        .schedule(world.tick + 10, ScheduledTask::CreatureSeeSweep);
    // The Monster Race (like the Lottery) starts from its DB-load event
    // (`DbEvent::MdtLoaded` → `monster_race::on_mdt_loaded`), which seeds the
    // race number from the loaded history before beginning the cycle.
}

/// The ordered shutdown saves (Java `Shutdown` save-all): everything queued
/// here drains ahead of the `DbCommand::Shutdown` `main` sends after this
/// thread joins.
pub(super) fn shutdown_flush(world: &mut World) {
    // Persist every still-online player so level/exp/position survive the
    // restart (Java `Shutdown` save-all). These `StorePlayer` commands queue
    // ahead of the `DbCommand::Shutdown` `main` sends only after this thread
    // joins, so the DB thread drains them first.
    net::save_all_players(world);
    // `Shutdown` → `OfflineTraderTable.storeOffliners()` — only meaningful with
    // realtime storing off; with it on (this dist) the rows are already current
    // and Java skips the sweep entirely.
    offline_trade::store_offliners(world);
    // `Shutdown` → `BotReportTable.saveReportedCharData()`.
    bot_report::save_reports(world);
    // `DBSpawnManager.updateDb` — every living raid boss's current HP/MP, so a
    // restart mid-fight resumes at the HP the boss was left on.
    boss_respawn::save_all_bosses(world);
    // `Olympiad.saveOlympiadStatus` — the period row + every noble's points.
    olympiad::save_all(world);
    // `Shutdown` → `CursedWeaponsManager.saveData()`: every live weapon's row.
    // Only `activate`/`increaseKills` write it during play, so without this the
    // kill tally and the time already burned off a wielded weapon are lost on
    // restart — it would come back with its count and deadline as of pickup.
    cursed_weapon::save_all(world);
    // `Shutdown`: "Save all manor data", guarded by `!ALT_MANOR_SAVE_ALL_ACTIONS`
    // exactly as Java guards it — with per-action saving on, the rows are
    // already current and the sweep is redundant.
    manor::save_all_on_shutdown(world);
    // `Shutdown` → `ItemsOnGroundManager.saveInDb()`: the ground set is
    // in-memory between periodic writes, so without this every item dropped
    // since the last one is lost. A no-op while `SaveDroppedItem` is off.
    ground_items::store_all(world);
}
