//! The event engine — the Rust counterpart of the `Event`-derived scripts
//! (`Event extends Quest`, adding `eventStart`/`eventStop`/`eventBypass`) plus
//! their schedule. Java has no central engine class; each event drives itself.
//! The port centralizes the *lifecycle* (start/stop a named event) so the admin
//! trigger — and, later, a cron schedule — can drive any registered event,
//! while each event's logic and state live in its own module.
//!
//! Only **Team vs Team** ([`tvt`]) exists this milestone (G28). On this dist the
//! TvT `config.xml` schedule ships commented out, so the event is GM-triggered
//! via `//event_start` (`game_loop/admin/events.rs`); the cron auto-start is a
//! later slice.

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
