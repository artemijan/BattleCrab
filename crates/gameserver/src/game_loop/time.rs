//! Shared time-unit constants for the 100 ms tick world — previously
//! re-declared file-locally by two dozen modules.

/// Game-loop ticks per second (the loop runs at [`super::TICK`], 100 ms).
pub(crate) const TICKS_PER_SECOND: u64 = 10;
pub(crate) const MILLIS_PER_MINUTE: i64 = 60_000;
pub(crate) const MILLIS_PER_HOUR: i64 = 3_600_000;
pub(crate) const MILLIS_PER_DAY: i64 = 86_400_000;

/// Schedule `task` to fire `delay_ms` from now at whole-second resolution —
/// the arming idiom the wall-clock schedulers (siege, manor, lottery,
/// punishments, auctions, events, cursed weapons) each hand-rolled. A
/// past-due delay fires on the next timer sweep.
pub(crate) fn schedule_in_ms(
    world: &mut crate::world::World,
    delay_ms: i64,
    task: crate::scheduler::ScheduledTask,
) {
    let delay_ticks = (delay_ms.max(0) / 1000) as u64 * TICKS_PER_SECOND;
    world.scheduler.schedule(world.tick + delay_ticks, task);
}
