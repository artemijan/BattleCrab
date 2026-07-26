//! Team vs Team — the representative event for the G28 gate. Port of
//! `custom/events/TeamVsTeam/TvT.java`. This slice (1) covers the lifecycle
//! skeleton and the **registration phase**: the manager NPC at Giran, the
//! open registration window, the register/cancel talk flow, and the
//! window-close handler (cancel for too few players). Standing the arena up,
//! the fight, scoring and rewards are slices 2–4 (see
//! `docs/PLAN_G28_EVENTS_ENGINE.md`), flagged `TODO(G28)` at the seams.

use tracing::warn;

use crate::enums::ChatType;
use crate::game_loop::death::{despawn_npc, introduce_npc};
use crate::model::components::{FishingSession, RegionCell};
use crate::model::event::TvtPhase;
use crate::model::npc::spawn_npc_at;
use crate::model::Player;
use crate::network::server_packets as sp;
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

/// The event's registry name (Java `params.set("Name", "Team Vs Team")`; the
/// script owning the manager is registered under this too, so the html buttons'
/// `bypass -h Quest TvT <event>` route to it).
pub(crate) const NAME: &str = "TvT";

/// The Giran event-manager NPC (Java `MANAGER = 70010`).
pub(crate) const MANAGER: i32 = 70010;

// Java `MANAGER_SPAWN_LOC = new Location(83425, 148585, -3406, 32938)`.
const MANAGER_X: i32 = 83425;
const MANAGER_Y: i32 = 148585;
const MANAGER_Z: i32 = -3406;
const MANAGER_HEADING: i32 = 32938;

// Java `Settings`.
const REGISTRATION_TIME_MIN: u64 = 10;
const MINIMUM_PARTICIPANT_LEVEL: i32 = 76;
const MAXIMUM_PARTICIPANT_LEVEL: i32 = 200;
const MINIMUM_PARTICIPANT_COUNT: usize = 4;
const MAXIMUM_PARTICIPANT_COUNT: usize = 24; // Scoreboard has 25 slots.

/// Game-loop ticks per second (matches the rest of `game_loop`).
const TICKS_PER_SECOND: u64 = 10;

// ---------------------------------------------------------------------------
// Lifecycle (Java `eventStart` / `eventStop`)
// ---------------------------------------------------------------------------

/// Java `TvT.eventStart(eventMaker)`: open registration. Returns `false` if an
/// event is already running (Java's `EVENT_ACTIVE` re-entry guard).
pub(crate) fn event_start(world: &mut World) -> bool {
    if world.events.tvt.is_active() {
        return false;
    }
    world.events.tvt.reset();
    world.events.tvt.phase = TvtPhase::Registration;
    world.events.active = Some(NAME);

    // Spawn the event manager at Giran (Java `addSpawn(MANAGER, MANAGER_SPAWN_LOC,
    // false, REGISTRATION_TIME * 60000)` — the despawn coincides with the
    // registration-close handler below, so we delete it there rather than arm a
    // second timer).
    if let Some(oid) = spawn_npc_at(
        world,
        MANAGER,
        MANAGER_X,
        MANAGER_Y,
        MANAGER_Z,
        MANAGER_HEADING,
    ) {
        introduce_npc(world, oid);
        world.events.tvt.manager_oid = Some(oid);
    }

    // Java `startQuestTimer("TeleportToArena", REGISTRATION_TIME * 60000)`.
    world.scheduler.schedule(
        world.tick + REGISTRATION_TIME_MIN * 60 * TICKS_PER_SECOND,
        ScheduledTask::TvtTeleportToArena,
    );

    announce(
        world,
        &format!("TvT Event: Registration opened for {REGISTRATION_TIME_MIN} minutes."),
    );
    announce(
        world,
        "TvT Event: You can register at Giran TvT Event Manager.",
    );
    true
}

/// Java `TvT.eventStop()`: force-cancel a running event. Returns `false` when
/// nothing was running.
pub(crate) fn event_stop(world: &mut World) -> bool {
    if !world.events.tvt.is_active() {
        return false;
    }
    despawn_manager(world);
    // Java clears the registration flag on every participant (the fight-state
    // teardown — team/invul/immobilize/servitors — lands with slice 4).
    for player in world.events.tvt.player_list.clone() {
        set_registered(world, player, false);
        // TODO(G28): setOnEvent(false), team=NONE, un-invul/un-immobilize +
        //   servitor reset (slice 4, once the fight state exists).
    }
    // TODO(G28): PVP_WORLD.destroy() — no instance stands up until slice 2.
    world.events.tvt.reset();
    world.events.active = None;
    announce(world, "TvT Event: Event was canceled.");
    true
}

/// Java `TvT.onEvent("TeleportToArena")`: registration closed. Prune offline
/// registrants, then either stand up the arena (slice 2) or cancel for too few
/// players.
pub(crate) fn teleport_to_arena(world: &mut World) {
    if world.events.tvt.phase != TvtPhase::Registration {
        return;
    }
    // The manager's `addSpawn` despawn deadline is this same moment.
    despawn_manager(world);
    prune_offline(world);

    if world.events.tvt.player_list.len() < MINIMUM_PARTICIPANT_COUNT {
        announce(
            world,
            "TvT Event: Event was canceled, not enough participants.",
        );
        clear_registrations(world);
        world.events.tvt.reset();
        world.events.active = None;
        return;
    }

    // TODO(G28) slice 2: create the coliseum instance (template 3049), close
    // doors, shuffle + split BLUE/RED, teleport participants in, form
    // parties/command-channels, spawn the two buffers, broadcast
    // ExPVPMatchCCRecord::INITIALIZE, then arm the StartFight countdown.
    // Until that lands, end the event cleanly rather than strand registrants.
    warn!(
        "TvT: {} participants registered — arena stand-up is slice 2 (TODO(G28)); ending event.",
        world.events.tvt.player_list.len()
    );
    clear_registrations(world);
    world.events.tvt.reset();
    world.events.active = None;
}

// ---------------------------------------------------------------------------
// Manager talk flow (Java `onFirstTalk` / `onEvent`)
// ---------------------------------------------------------------------------

/// Java `TvT.onFirstTalk(npc, player)`: the manager's chat window during
/// registration. Returns the html to show (`None` shows nothing, as when the
/// event isn't active).
pub(crate) fn on_manager_first_talk(world: &World, player: i32) -> Option<String> {
    if !world.events.tvt.is_active() {
        return None;
    }
    if world.events.tvt.player_list.contains(&player) {
        // TODO(G28): when the manager is the in-arena copy (npc in the PVP
        //   instance), Java shows "manager-buffheal.html" — slice 3.
        return Some(count_page(world, "manager-cancel.html"));
    }
    Some(count_page(world, "manager-register.html"))
}

/// Java `TvT.onEvent(event, npc, player)` for the manager's bypass buttons.
pub(crate) fn on_manager_event(
    world: &mut World,
    client_id: u32,
    player: i32,
    event: &str,
) -> Option<String> {
    if !world.events.tvt.is_active() {
        return None;
    }
    match event {
        "Participate" => {
            if can_register(world, client_id, player) {
                // TODO(G28): AntiFeedManager dualbox-per-IP cap
                //   (registration-ip.html) — needs the IP plumbing (G31).
                world.events.tvt.player_list.push(player);
                world.events.tvt.scores.insert(player, 0);
                set_registered(world, player, true);
                // TODO(G28): addLogoutListener(player) — forfeit-on-logout is
                //   slice 4.
                Some("registration-success.html".to_string())
            } else {
                Some("registration-failed.html".to_string())
            }
        }
        "CancelParticipation" => {
            // Java: can't cancel once inside the fight.
            if is_on_event(world, player) {
                return None;
            }
            world.events.tvt.player_list.retain(|&p| p != player);
            world.events.tvt.scores.remove(&player);
            set_registered(world, player, false);
            Some("registration-canceled.html".to_string())
        }
        "BuffHeal" => {
            // TODO(G28): in-arena buff + full heal (slice 3 — needs the fight
            //   state and SkillCaster.triggerCast on the manager).
            None
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Registration eligibility (Java `canRegister`)
// ---------------------------------------------------------------------------

/// Java `TvT.canRegister(player)`. Ported gates use state that exists on this
/// port; the rest are `TODO(G28)` at the site.
fn can_register(world: &mut World, client_id: u32, player: i32) -> bool {
    if world.events.tvt.player_list.contains(&player) {
        send_player_message(
            world,
            client_id,
            "You are already registered on this event.",
        );
        return false;
    }
    let Some((level, reputation, cursed, already_reg)) =
        world.objects.get_component::<Player>(&player).map(|p| {
            (
                p.level,
                p.reputation,
                p.cursed_weapon_equipped_id != 0,
                p.registered_on_event,
            )
        })
    else {
        return false;
    };
    if level < MINIMUM_PARTICIPANT_LEVEL {
        send_player_message(world, client_id, "Your level is too low to participate.");
        return false;
    }
    if level > MAXIMUM_PARTICIPANT_LEVEL {
        send_player_message(world, client_id, "Your level is too high to participate.");
        return false;
    }
    if already_reg {
        send_player_message(world, client_id, "You are already registered on an event.");
        return false;
    }
    if world.events.tvt.player_list.len() >= MAXIMUM_PARTICIPANT_COUNT {
        send_player_message(
            world,
            client_id,
            "There are too many players registered on the event.",
        );
        return false;
    }
    if cursed || reputation < 0 {
        send_player_message(
            world,
            client_id,
            "People with bad reputation can't register.",
        );
        return false;
    }
    if world.olympiad.is_registered(player) {
        send_player_message(
            world,
            client_id,
            "You cannot participate while registered on the Olympiad.",
        );
        return false;
    }
    if is_fishing(world, player) {
        send_player_message(world, client_id, "You cannot register while fishing.");
        return false;
    }
    // TODO(G28): remaining Java gates whose state isn't wired yet —
    //   isFlyingMounted, isTransformed, isInventoryUnder80 + weightPenalty,
    //   isInDuel, isInInstance, isInSiege/inside a SIEGE zone. Add each as its
    //   subsystem exposes the query.
    true
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn set_registered(world: &mut World, player: i32, value: bool) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&player) {
        p.registered_on_event = value;
    }
}

fn is_on_event(world: &World, player: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&player)
        .is_some_and(|p| p.on_event)
}

fn is_fishing(world: &World, player: i32) -> bool {
    world
        .objects
        .get_component::<FishingSession>(&player)
        .is_some_and(|f| f.is_fishing)
}

/// Drop the registration flag on every current registrant (Java's
/// `participant.setRegisteredOnEvent(false)` cleanup loops).
fn clear_registrations(world: &mut World) {
    for player in world.events.tvt.player_list.clone() {
        set_registered(world, player, false);
    }
}

/// Remove registrants who logged out during the window (Java's offline sweep at
/// the top of `TeleportToArena`).
fn prune_offline(world: &mut World) {
    let offline: Vec<i32> = world
        .events
        .tvt
        .player_list
        .iter()
        .copied()
        .filter(|p| world.objects.get_component::<Player>(p).is_none())
        .collect();
    for p in offline {
        world.events.tvt.player_list.retain(|&x| x != p);
        world.events.tvt.scores.remove(&p);
    }
}

fn despawn_manager(world: &mut World) {
    let Some(oid) = world.events.tvt.manager_oid.take() else {
        return;
    };
    let Some(region) = world.objects.get_component::<RegionCell>(&oid).map(|r| r.0) else {
        return;
    };
    despawn_npc(world, oid, region);
}

/// Load a manager page and substitute `%player_numbers%` (Java builds these two
/// via `html.replace`). Returned content starts with `<html>`, so the quest
/// framework renders it inline.
fn count_page(world: &World, file: &str) -> String {
    manager_html(world, file).replace(
        "%player_numbers%",
        &world.events.tvt.player_list.len().to_string(),
    )
}

fn manager_html(world: &World, file: &str) -> String {
    let root = &world.data.root;
    crate::data::htm_cache::read_htm(format!(
        "{root}data/scripts/custom/events/TeamVsTeam/{file}"
    ))
    .unwrap_or_default()
}

/// Java `player.sendMessage(String)` — a `$s1` system-message line.
fn send_player_message(world: &World, client_id: u32, text: &str) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(sp::system_message_with(
            sp::sm_ids::S1_TEXT,
            &[sp::SmParam::Text(text.to_string())],
        ));
    }
}

/// Java `Broadcast.toAllOnlinePlayers(String)` — a yellow announcement line to
/// every in-game player.
fn announce(world: &World, text: &str) {
    let pkt = sp::creature_say(0, ChatType::Announcement, "", text, None);
    for cs in world.clients.values() {
        if let ClientSession::InGame(_) = cs {
            cs.send(pkt.clone());
        }
    }
}
