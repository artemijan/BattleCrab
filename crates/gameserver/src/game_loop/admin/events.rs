//! `//event_start` / `//event_stop` — the GM trigger for the event engine
//! (G28). This dist ships the TvT `config.xml` schedule commented out, so
//! there is no auto-start; a GM opens and cancels the event by hand. There is
//! no direct Java analogue command (Java relies on the schedule), so this is a
//! port-side operator convenience over [`crate::game_loop::events`].

use super::send_message;
use crate::game_loop::events;
use crate::world::World;

/// `//event_start [name]` — open the named event (defaults to `TvT`, the only
/// one this milestone).
pub(super) fn admin_event_start(world: &mut World, client_id: u32, args: &[&str]) {
    let name = args.first().copied().unwrap_or(events::tvt::NAME);
    if !events::EVENT_NAMES.contains(&name) {
        send_message(
            world,
            client_id,
            &format!(
                "Unknown event '{name}'. Known: {}.",
                events::EVENT_NAMES.join(", ")
            ),
        );
        return;
    }
    if events::start(world, name) {
        send_message(world, client_id, &format!("Event '{name}' started."));
    } else {
        send_message(
            world,
            client_id,
            &format!("Event '{name}' could not start (already running?)."),
        );
    }
}

/// `//event_stop [name]` — cancel the named event (defaults to `TvT`).
pub(super) fn admin_event_stop(world: &mut World, client_id: u32, args: &[&str]) {
    let name = args.first().copied().unwrap_or(events::tvt::NAME);
    if events::stop(world, name) {
        send_message(world, client_id, &format!("Event '{name}' stopped."));
    } else {
        send_message(
            world,
            client_id,
            &format!("Event '{name}' was not running."),
        );
    }
}
