//! The event engine — the Rust counterpart of the `Event`-derived scripts
//! (`Event extends Quest`, adding `eventStart`/`eventStop`/`eventBypass`) plus
//! their schedule. Java has no central engine class; each event drives itself.
//! The port centralizes the *lifecycle* (start/stop a named event) so the admin
//! trigger — and, later, a cron schedule — can drive any registered event,
//! while each event's logic and state live in its own module.
//!
//! Only **Team vs Team** ([`tvt`]) exists this milestone (G28). Events start
//! either from the cron schedule in their `config.xml` ([`schedule_at_boot`],
//! Java's `loadConfig` + `Schedule<n>` timers) or by GM trigger
//! (`//event_start`, `game_loop/admin/events.rs`). **On this dist the TvT
//! schedule ships commented out**, so nothing auto-starts until an operator
//! uncomments it — the loader is live either way.

pub(crate) mod tvt;

use crate::world::World;

/// The events the engine knows, for the admin panel / listing.
pub(crate) const EVENT_NAMES: &[&str] = &[tvt::NAME];

/// `Event.eventStart(eventMaker)` for the named event. Returns `false` when the
/// name is unknown or the event refused to start (already running).
pub(crate) fn start(world: &mut World, name: &str) -> bool {
    match name {
        tvt::NAME => tvt::event_start(world),
        _ => false,
    }
}

/// `Event.eventStop()` for the named event. Returns `false` when the name is
/// unknown or the event was not running.
pub(crate) fn stop(world: &mut World, name: &str) -> bool {
    match name {
        tvt::NAME => tvt::event_stop(world),
        _ => false,
    }
}

/// Java's per-event `loadConfig()`: read each event's `config.xml`, parse its
/// `<schedule pattern="…">` entries, and arm the first firing of each. Called
/// once at boot; every firing re-arms itself.
pub(crate) fn schedule_at_boot(world: &mut World) {
    let patterns = tvt::load_schedule(&world.data.root);
    for (index, pattern) in patterns.into_iter().enumerate() {
        arm_schedule(world, index, &pattern);
    }
}

/// Arm one schedule slot's next firing (Java `getTimers().addTimer("Schedule<n>",
/// …, schedulingPattern.getDelayToNextFromNow())`).
pub(crate) fn arm_schedule(world: &mut World, index: usize, pattern: &str) {
    let Some(parsed) = commons::cron::SchedulingPattern::parse(pattern) else {
        tracing::warn!("Events: bad schedule pattern [{pattern}], ignored.");
        return;
    };
    let now = commons::util::now_millis();
    let Some(delay_ms) = parsed.delay_from(now) else {
        tracing::warn!("Events: schedule pattern [{pattern}] never fires, ignored.");
        return;
    };
    let ticks = (delay_ms.max(0) / 1000) as u64 * 10;
    tracing::info!(
        "Events: {} scheduled in {} s (pattern [{pattern}]).",
        tvt::NAME,
        delay_ms / 1000
    );
    world.scheduler.schedule(
        world.tick + ticks,
        crate::scheduler::ScheduledTask::EventSchedule {
            index,
            pattern: pattern.to_string(),
        },
    );
}

/// A schedule slot fired: start the event and re-arm for the next occurrence
/// (Java re-adds the timer with `getDelayToNextFromNow() + 1000`).
pub(crate) fn on_schedule_fired(world: &mut World, index: usize, pattern: String) {
    start(world, tvt::NAME);
    arm_schedule(world, index, &pattern);
}
