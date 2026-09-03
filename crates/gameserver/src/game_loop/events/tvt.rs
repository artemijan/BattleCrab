//! Team vs Team — the representative event for the G28 gate. Port of
//! `custom/events/TeamVsTeam/TvT.java`. Slices 1–2: the lifecycle, the
//! **registration phase** (manager NPC, register/cancel window), and the
//! **arena stand-up** (coliseum instance, team split + teleport, buffers,
//! scoreboard, the fight-window door/timer chain through a minimal teardown).
//! Slices 3–4 then landed per-kill **scoring** (`on_player_death`, called from
//! `death::player_do_die` for every death), the arena **respawn**, and the
//! winner **rewards** (`reward_team`) — see `PLAN_G28_EVENTS_ENGINE.md`.
//!
//! The once-deferred seams have all closed: the team split leaves old
//! parties and regroups each side into parties of 7 under a per-team command
//! channel (`group_team`), the logout forfeit runs from the disconnect paths
//! (`on_player_logout`), and the freeze applies `Immobilized` +
//! `SkillsDisabled` like Java's `disableAllSkills`.

use crate::game_loop::character::inventory;
use crate::game_loop::space::position::maybe_position;
use crate::game_loop::{helpers, skills};

use crate::game_loop::time::TICKS_PER_SECOND;
use commons::util::rnd;
use tracing::warn;

use crate::game_loop::client::user_commands::in_combat;
use crate::game_loop::combat::death::teleport_player;
use crate::game_loop::net::broadcast;
use crate::game_loop::npc::{despawn_npc_by_oid, introduce_npc, spawn_npc_at};
use crate::game_loop::space::instances;
use crate::model::Player;
use crate::model::components::FishingSession;
use crate::model::event::TvtPhase;
use crate::network::server_packets as sp;
use crate::scheduler::ScheduledTask;
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
const WAIT_TIME_MIN: u64 = 1;
const FIGHT_TIME_MIN: u64 = 20;
const MINIMUM_PARTICIPANT_LEVEL: i32 = 76;
const MAXIMUM_PARTICIPANT_LEVEL: i32 = 200;
const MINIMUM_PARTICIPANT_COUNT: usize = 4;
const MAXIMUM_PARTICIPANT_COUNT: usize = 24; // Scoreboard has 25 slots.

// The coliseum arena (Java `INSTANCE_ID` + door/spawn `Location`s).
const INSTANCE_ID: i32 = 3049;
const BLUE_DOOR_ID: i32 = 24190002;
const RED_DOOR_ID: i32 = 24190003;
const BLUE_SPAWN: (i32, i32, i32) = (147447, 46722, -3416);
const RED_SPAWN: (i32, i32, i32) = (151536, 46722, -3416);
// Buffer NPCs (the manager reused): `(x, y, z, heading)`.
const BLUE_BUFFER: (i32, i32, i32, i32) = (147450, 46913, -3400, 49000);
const RED_BUFFER: (i32, i32, i32, i32) = (151545, 46528, -3400, 16000);

// Java `Team` ordinals (`Creature._team` — 0 none / 1 blue / 2 red).
const TEAM_NONE: u8 = 0;
const TEAM_BLUE: u8 = 1;
const TEAM_RED: u8 = 2;

// `ExShowScreenMessage` positions (Java constants).
const TOP_CENTER: i32 = 2;
const BOTTOM_RIGHT: i32 = 8;

/// The custom Ghost Walking skill (Java `GHOST_WALKING`) — 30s of
/// invulnerability (`DamageBlock` HP/MP) applied on respawn.
const GHOST_WALKING: i32 = 100000;
/// Seconds a killed participant waits before the arena respawns them (Java
/// `startQuestTimer("ResurrectPlayer", 10000)`).
const RESURRECT_DELAY_SECS: u64 = 10;
/// The `ScoreBoard` delay in ticks (Java 3500 ms; 10 ticks/s → 35).
const SCOREBOARD_DELAY_TICKS: u64 = 35;
/// Seconds from `EndFight` to `TeleportOut` (Java 7000 ms).
const TELEPORT_OUT_DELAY_SECS: u64 = 7;
/// Seconds from a forfeit to the early `EndFight` (Java `manageForfeit` 10000 ms).
const FORFEIT_DELAY_SECS: u64 = 10;

/// Winner Adena reward (Java `REWARD = new ItemHolder(57, 100000)`).
const REWARD_ADENA: i64 = 100_000;
/// The firework flourish the winners play (Java `CommonSkill.FIREWORK`).
const FIREWORK_SKILL: i32 = 5965;
/// Social actions: the winners cheer (3), everyone shrugs on a tie (13).
const SOCIAL_WIN: i32 = 3;
const SOCIAL_TIE: i32 = 13;

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

    helpers::announce_to_all_online(
        world,
        &format!("TvT Event: Registration opened for {REGISTRATION_TIME_MIN} minutes."),
    );
    helpers::announce_to_all_online(
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
    // Clear per-participant event state (Java also un-invul / un-immobilize /
    // re-enable skills + servitor reset — that fight-state teardown lands with
    // slice 4, once those disables are applied at `EndFight`).
    for player in world.events.tvt.player_list.clone() {
        set_registered(world, player, false);
        set_on_event(world, player, false);
        set_team(world, player, TEAM_NONE);
    }
    // Tear the arena down if one is up (ousts everyone to their ORIGIN return
    // location, despawns the arena NPCs/doors).
    if let Some(instance_id) = world.events.tvt.world_id.take() {
        instances::destroy(world, instance_id);
    }
    world.events.tvt.reset();
    world.events.active = None;
    helpers::announce_to_all_online(world, "TvT Event: Event was canceled.");
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
        helpers::announce_to_all_online(
            world,
            "TvT Event: Event was canceled, not enough participants.",
        );
        clear_registrations(world);
        world.events.tvt.reset();
        world.events.active = None;
        return;
    }

    // Enough players — stand the arena up.
    let Some(instance_id) = instances::create_from_template(world, INSTANCE_ID) else {
        warn!("TvT: failed to create coliseum instance {INSTANCE_ID}; canceling.");
        helpers::announce_to_all_online(world, "TvT Event: Event was canceled.");
        clear_registrations(world);
        world.events.tvt.reset();
        world.events.active = None;
        return;
    };
    world.events.tvt.world_id = Some(instance_id);
    // The coliseum doors default to closed (coliseum.xml), so no explicit close.

    // Shuffle, then split into teams. Java alternates from a random starting
    // side (`getRandomBoolean`), so the odd player lands on that random side.
    let mut roster = world.events.tvt.player_list.clone();
    rnd::shuffle(&mut roster);
    world.events.tvt.player_list = roster.clone();
    let mut to_blue = rnd::chance(50.0);
    for player in roster {
        set_registered(world, player, false);
        set_on_event(world, player, true);
        // `participant.leaveParty()` — Java's `Player.leaveParty` leaves with
        // `PartyMessageType.DISCONNECTED`, before the arena teleport.
        if let Some(pid) = world
            .objects
            .get_component::<crate::model::components::PartyRef>(&player)
            .map(|r| r.0)
        {
            crate::game_loop::party::remove_party_member(
                world,
                pid,
                player,
                crate::game_loop::party::LeaveType::Disconnected,
            );
        }
        instances::enter(world, player, instance_id);
        if to_blue {
            world.events.tvt.blue_team.push(player);
            set_team(world, player, TEAM_BLUE);
            teleport_player(world, player, BLUE_SPAWN.0, BLUE_SPAWN.1, BLUE_SPAWN.2);
        } else {
            world.events.tvt.red_team.push(player);
            set_team(world, player, TEAM_RED);
            teleport_player(world, player, RED_SPAWN.0, RED_SPAWN.1, RED_SPAWN.2);
        }
        to_blue = !to_blue;
        // (`addDeathListener` is the port's unconditional `on_player_death`
        // hook in `death::player_do_die`; the logout listener is
        // `on_player_logout`, wired from the disconnect paths.)
    }

    // "Make Blue CC." / "Make Red CC." — each team splits into parties of
    // `PARTY_MEMBER_COUNT`, joined into one command channel per team when the
    // team overflows a single party.
    let (blue, red) = (
        world.events.tvt.blue_team.clone(),
        world.events.tvt.red_team.clone(),
    );
    group_team(world, &blue);
    group_team(world, &red);

    // The two arena buffers (the manager NPC reused). Their object ids are kept
    // so the in-arena buff/heal window can be told from the town manager's.
    world.events.tvt.arena_managers = [BLUE_BUFFER, RED_BUFFER]
        .into_iter()
        .filter_map(|(x, y, z, heading)| {
            instances::spawn_npc(world, instance_id, MANAGER, x, y, z, heading)
        })
        .collect();

    // Initialize the scoreboard (scores already 0 from registration).
    broadcast_scoreboard(world, instance_id, sp::PVP_MATCH_INITIALIZE);

    world.events.tvt.phase = TvtPhase::Warmup;
    world.scheduler.schedule(
        world.tick + WAIT_TIME_MIN * 60 * TICKS_PER_SECOND,
        ScheduledTask::TvtStartFight,
    );
    // Java arms "5".."1" against the same `WAIT_TIME` deadline.
    schedule_countdown(world, WAIT_TIME_MIN * 60 * TICKS_PER_SECOND, 5);
}

/// Java `TvT.onEvent("StartFight")`: open the arena doors and start the fight.
pub(crate) fn start_fight(world: &mut World) {
    if world.events.tvt.phase != TvtPhase::Warmup {
        return;
    }
    let Some(instance_id) = world.events.tvt.world_id else {
        return;
    };
    instances::open_close_door(world, instance_id, BLUE_DOOR_ID, true);
    instances::open_close_door(world, instance_id, RED_DOOR_ID, true);
    broadcast_screen(world, instance_id, "The fight has began!", 5);
    world.events.tvt.phase = TvtPhase::Fighting;
    world.scheduler.schedule(
        world.tick + FIGHT_TIME_MIN * 60 * TICKS_PER_SECOND,
        ScheduledTask::TvtEndFight,
    );
    // Java arms ten one-shot timers named "10".."1" at `FIGHT_TIME - Ns`; the
    // port arms the same ten ticks with the number as the payload.
    schedule_countdown(world, FIGHT_TIME_MIN * 60 * TICKS_PER_SECOND, 10);
}

/// Java's `startQuestTimer("<n>", <phase end> - n*1000)` chain: one screen
/// banner per second for the last `from` seconds of a phase.
fn schedule_countdown(world: &mut World, phase_end_ticks: u64, from: i32) {
    let seq = world.events.tvt.countdown_seq;
    for n in 1..=from {
        let at = phase_end_ticks.saturating_sub(n as u64 * TICKS_PER_SECOND);
        world.scheduler.schedule(
            world.tick + at,
            ScheduledTask::TvtCountdown { seconds: n, seq },
        );
    }
}

/// One countdown tick — Java `case "10" … case "1": broadcastScreenMessage`.
/// A tick from a cancelled chain (forfeit, early end) is dropped by the seq.
pub(crate) fn countdown(world: &mut World, seconds: i32, seq: u64) {
    if seq != world.events.tvt.countdown_seq {
        return;
    }
    let Some(instance_id) = world.events.tvt.world_id else {
        return;
    };
    broadcast_screen(world, instance_id, &seconds.to_string(), 4);
}

/// Java `TvT.onEvent("EndFight")`: the fight is over — close the doors, freeze +
/// revive the participants, resolve the winner and reward them, then arm the
/// scoreboard (3.5s) and teleport-out (7s). Runs once (the `Ending` guard also
/// absorbs the original `FIGHT_TIME` timer firing after a forfeit's early end).
pub(crate) fn end_fight(world: &mut World) {
    let Some(instance_id) = world.events.tvt.world_id else {
        return;
    };
    if world.events.tvt.phase == TvtPhase::Ending {
        return;
    }
    world.events.tvt.phase = TvtPhase::Ending;
    // Java `manageForfeit`/`EndFight` cancel every pending "10".."1" timer;
    // bumping the generation drops them all.
    world.events.tvt.countdown_seq += 1;

    instances::open_close_door(world, instance_id, BLUE_DOOR_ID, false);
    instances::open_close_door(world, instance_id, RED_DOOR_ID, false);

    // `EndFight`'s "Disable players" block: invulnerable, immobilised and
    // skill-locked, servitors included — then revive anyone who died so the
    // arena empties with everybody standing.
    for player in world.events.tvt.player_list.clone() {
        set_frozen(world, player, true);
        if helpers::is_dead(world, player) {
            crate::game_loop::death::do_revive(world, player);
        }
    }

    // Resolve the winner.
    let (blue, red) = (world.events.tvt.blue_score, world.events.tvt.red_score);
    if blue > red {
        broadcast_screen(world, instance_id, "Team Blue won the event!", 7);
        reward_team(world, TEAM_BLUE);
    } else if red > blue {
        broadcast_screen(world, instance_id, "Team Red won the event!", 7);
        reward_team(world, TEAM_RED);
    } else {
        broadcast_screen(world, instance_id, "The event ended with a tie!", 7);
        for player in world.events.tvt.player_list.clone() {
            broadcast_social(world, player, SOCIAL_TIE);
        }
    }

    world.scheduler.schedule(
        world.tick + SCOREBOARD_DELAY_TICKS,
        ScheduledTask::TvtScoreBoard,
    );
    world.scheduler.schedule(
        world.tick + TELEPORT_OUT_DELAY_SECS * TICKS_PER_SECOND,
        ScheduledTask::TvtTeleportOut,
    );
}

/// Java `TvT.onEvent("ScoreBoard")`: the final scoreboard.
pub(crate) fn score_board(world: &mut World) {
    if let Some(instance_id) = world.events.tvt.world_id {
        broadcast_scoreboard(world, instance_id, sp::PVP_MATCH_FINISH);
    }
}

/// Java `TvT.onEvent("TeleportOut")`: unfreeze participants, clear their event
/// state, and destroy the arena (ousting everyone to their ORIGIN return
/// location). Idempotent — a stale timer after `event_stop` finds nothing.
pub(crate) fn teleport_out(world: &mut World) {
    for player in world.events.tvt.player_list.clone() {
        set_on_event(world, player, false);
        set_team(world, player, TEAM_NONE);
        set_frozen(world, player, false);
    }
    if let Some(instance_id) = world.events.tvt.world_id.take() {
        instances::destroy(world, instance_id);
    }
    world.events.tvt.reset();
    world.events.active = None;
}

/// Give every still-present member of the winning team the firework flourish,
/// the cheer social action, and the adena reward (Java's `EndFight` winner loop,
/// which skips anyone no longer in `PVP_WORLD`).
fn reward_team(world: &mut World, team: u8) {
    let Some(instance_id) = world.events.tvt.world_id else {
        return;
    };
    let members = if team == TEAM_BLUE {
        world.events.tvt.blue_team.clone()
    } else {
        world.events.tvt.red_team.clone()
    };
    for player in members {
        if helpers::instance_of(world, player) != instance_id {
            continue;
        }
        firework(world, player);
        broadcast_social(world, player, SOCIAL_WIN);
        if let Some(cid) = helpers::client_for_player(world, player) {
            inventory::give_item_with_earned_message(
                world,
                cid,
                player,
                crate::data::item_data::ADENA_ID,
                REWARD_ADENA,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Scoring & respawn (Java `onPlayerDeath` + `"ResurrectPlayer"`)
// ---------------------------------------------------------------------------

/// A participant died. A cross-team kill scores for the killer's side (score
/// message + `ExPVPMatchCCRecord::UPDATE`); the victim is queued for a timed
/// arena respawn. Called from `death::player_do_die` for **every** player death;
/// no-ops off-event. `killer` is already the acting player.
pub(crate) fn on_player_death(world: &mut World, victim: i32, killer: i32) {
    // Only participants in a live arena (Java's death listener only exists
    // between teleport-in and teardown).
    let Some(instance_id) = world.events.tvt.world_id else {
        return;
    };
    if !is_on_event(world, victim) {
        return;
    }

    // Score a cross-team kill (Java: BLUE kills RED, RED kills BLUE).
    let scored = match (team_of(world, killer), team_of(world, victim)) {
        (TEAM_BLUE, TEAM_RED) => {
            world.events.tvt.blue_score += 1;
            true
        }
        (TEAM_RED, TEAM_BLUE) => {
            world.events.tvt.red_score += 1;
            true
        }
        _ => false,
    };
    if scored {
        *world.events.tvt.scores.entry(killer).or_insert(0) += 1;
        broadcast_score_message(world, instance_id);
        broadcast_scoreboard(world, instance_id, sp::PVP_MATCH_UPDATE);
    }

    // Queue the respawn (Java arms it for the killed player regardless of who
    // scored). A stale timer no-ops via `resurrect_player`'s guards.
    world.scheduler.schedule(
        world.tick + RESURRECT_DELAY_SECS * TICKS_PER_SECOND,
        ScheduledTask::TvtResurrect { player: victim },
    );
}

/// The queued respawn fires: if the victim is still dead and still in the event,
/// revive them at their team spawn behind the Ghost Walking invulnerability
/// (Java `"ResurrectPlayer"`).
pub(crate) fn resurrect_player(world: &mut World, player: i32) {
    if !is_on_event(world, player) || !helpers::is_dead(world, player) {
        return;
    }
    let spawn = match team_of(world, player) {
        TEAM_BLUE => BLUE_SPAWN,
        TEAM_RED => RED_SPAWN,
        _ => return,
    };
    teleport_player(world, player, spawn.0, spawn.1, spawn.2);
    crate::game_loop::death::do_revive(world, player);
    // Ghost Walking: 30s of DamageBlock (HP/MP) invulnerability + speed.
    if let Some(skill) = skills::skill_by_id(world, GHOST_WALKING, 1) {
        skills::effects::apply_skill_effects(world, player, player, &skill);
    }
    // Java resets the clock here too — a player who died *inside* their own
    // headquarters never crosses the zone edge on respawn, so the enter hook
    // would not re-arm it.
    reset_activity_timers(world, player);
}

/// Java's `player.isOnEvent() && !player.isOnSoloEvent() && (player.getTeam() == target.getTeam())`
/// — two players on the **same** event team. `TEAM_NONE` is the not-in-an-event
/// answer, so two bystanders never read as team-mates.
pub(crate) fn same_team(world: &World, a: i32, b: i32) -> bool {
    let ta = team_of(world, a);
    ta != TEAM_NONE && ta == team_of(world, b)
}

fn team_of(world: &World, player: i32) -> u8 {
    world
        .objects
        .get_component::<Player>(&player)
        .map_or(TEAM_NONE, |p| p.team)
}

/// Java `broadcastScoreMessage()` — the running "Blue: X - Red: Y" tally in the
/// arena's bottom-right corner.
fn broadcast_score_message(world: &World, instance_id: i32) {
    let text = format!(
        "Blue: {} - Red: {}",
        world.events.tvt.blue_score, world.events.tvt.red_score
    );
    let pkt = sp::ex_show_screen_message(&text, BOTTOM_RIGHT, 15_000);
    instances::broadcast_to_instance(world, instance_id, &pkt);
}

// ---------------------------------------------------------------------------
// Forfeit & logout (Java `onPlayerLogout` + `manageForfeit`)
// ---------------------------------------------------------------------------

/// A participant logged out (or dropped): drop them from every list, and if that
/// empties one team mid-arena, forfeit the match to the other. Called from the
/// logout / disconnect paths for **every** player; no-ops off-event. Java's
/// `onPlayerLogout` `ON_PLAYER_LOGOUT` listener.
pub(crate) fn on_player_logout(world: &mut World, player: i32) {
    if world.events.active.is_none() {
        return;
    }
    if !world.events.tvt.player_list.contains(&player) {
        return;
    }
    world.events.tvt.player_list.retain(|&p| p != player);
    world.events.tvt.scores.remove(&player);
    world.events.tvt.blue_team.retain(|&p| p != player);
    world.events.tvt.red_team.retain(|&p| p != player);

    // Forfeit only mid-arena (a live instance), when exactly one team is now
    // empty (Java's `(blueEmpty && !redEmpty) || (redEmpty && !blueEmpty)`).
    if world.events.tvt.world_id.is_some()
        && (world.events.tvt.blue_team.is_empty() != world.events.tvt.red_team.is_empty())
    {
        manage_forfeit(world);
    }
}

/// Java `manageForfeit`: end the match early for the surviving team. We can't
/// cancel the original `FIGHT_TIME` `EndFight` timer, but `end_fight`'s `Ending`
/// guard makes the later firing a no-op, so a second, earlier `EndFight` is safe.
fn manage_forfeit(world: &mut World) {
    if world.events.tvt.phase == TvtPhase::Ending {
        return;
    }
    if let Some(instance_id) = world.events.tvt.world_id {
        broadcast_screen(world, instance_id, "Enemy team forfeit!", 7);
    }
    world.scheduler.schedule(
        world.tick + FORFEIT_DELAY_SECS * TICKS_PER_SECOND,
        ScheduledTask::TvtEndFight,
    );
}

// ---------------------------------------------------------------------------
// Manager talk flow (Java `onFirstTalk` / `onEvent`)
// ---------------------------------------------------------------------------

/// Java `TvT.onFirstTalk(npc, player)`: the manager's chat window during
/// registration. Returns the html to show (`None` shows nothing, as when the
/// event isn't active).
pub(crate) fn on_manager_first_talk(world: &World, player: i32, npc: i32) -> Option<String> {
    if !world.events.tvt.is_active() {
        return None;
    }
    if world.events.tvt.player_list.contains(&player) {
        // Java: the manager copy standing *inside* the arena offers the buff/
        // heal service instead of the cancel window.
        let in_arena = world.events.tvt.arena_managers.contains(&npc);
        if in_arena {
            return Some("manager-buffheal.html".to_string());
        }
        return Some(count_page(world, player, "manager-cancel.html"));
    }
    Some(count_page(world, player, "manager-register.html"))
}

/// Java `TvT.loadConfig()`: the `<schedule pattern="…"/>` entries of
/// `data/scripts/custom/events/TeamVsTeam/config.xml`. **This dist ships them
/// commented out**, so the list is normally empty and nothing auto-starts.
pub(crate) fn load_schedule(root: &str) -> Vec<String> {
    let path = format!("{root}data/scripts/custom/events/TeamVsTeam/config.xml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut reader = quick_xml::Reader::from_str(&text);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(e)) | Ok(quick_xml::events::Event::Empty(e)) => {
                if e.name().as_ref() == b"schedule"
                    && let Some(pattern) = e
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == b"pattern")
                        .and_then(|a| String::from_utf8(a.value.to_vec()).ok())
                {
                    out.push(pattern);
                }
            }
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

// ---------------------------------------------------------------------------
// Headquarters zones — Java `onEnterZone` / `onExitZone` (row 10)
// ---------------------------------------------------------------------------

/// Java `INACTIVITY_TIME` — minutes a participant may idle in their own
/// headquarters before being kicked; the warning lands at half that.
const INACTIVITY_TIME_MIN: u64 = 2;

/// Java `TvT.onEnterZone`/`onExitZone` for `colosseum_peace1|2`, fired from
/// [`crate::game_loop::space::zones::revalidate_zone`] when the named zone changes.
///
/// - Walking into the **enemy** headquarters bounces you back to your own spawn
///   with a screen message ("Entering the enemy headquarters is prohibited!").
/// - Standing in **your own** starts the inactivity clock; leaving cancels it.
pub(crate) fn on_hq_zone_change(world: &mut World, player: i32, from: u8, to: u8) {
    if !is_on_event(world, player) {
        return;
    }
    // Exit: cancel the clock (Java also strips the respawn invulnerability,
    // which this port models as the `set_invul` flag).
    if from != 0 && to == 0 {
        cancel_inactivity(world, player);
        set_invul(world, player, false);
        return;
    }
    if to == 0 {
        return;
    }
    let team = team_of(world, player);
    if team == 0 {
        return;
    }
    if to != team {
        // Enemy headquarters — bounce them home.
        let spawn = if team == 1 { BLUE_SPAWN } else { RED_SPAWN };
        teleport_player(world, player, spawn.0, spawn.1, spawn.2);
        send_screen(
            world,
            player,
            "Entering the enemy headquarters is prohibited!",
            10,
        );
        return;
    }
    // Own headquarters — (re)start the inactivity clock.
    reset_activity_timers(world, player);
}

/// Java `resetActivityTimers`: cancel the pending pair and arm a fresh one. The
/// clock is longer while the arena doors are still shut (the warm-up), exactly
/// as Java adds `WAIT_TIME` in that branch.
pub(crate) fn reset_activity_timers(world: &mut World, player: i32) {
    let seq = {
        let e = world.events.tvt.inactivity_seq.entry(player).or_insert(0);
        *e += 1;
        *e
    };
    let warmup_extra = if world.events.tvt.phase == TvtPhase::Fighting {
        0
    } else {
        WAIT_TIME_MIN * 60 * TICKS_PER_SECOND
    };
    let kick_at = INACTIVITY_TIME_MIN * 60 * TICKS_PER_SECOND + warmup_extra;
    let warn_at = (INACTIVITY_TIME_MIN / 2) * 60 * TICKS_PER_SECOND + warmup_extra;
    world.scheduler.schedule(
        world.tick + warn_at,
        ScheduledTask::TvtInactivity {
            player,
            warning: true,
            seq,
        },
    );
    world.scheduler.schedule(
        world.tick + kick_at,
        ScheduledTask::TvtInactivity {
            player,
            warning: false,
            seq,
        },
    );
}

/// Retire this player's inactivity pair (Java's two `cancelQuestTimer`s).
fn cancel_inactivity(world: &mut World, player: i32) {
    *world.events.tvt.inactivity_seq.entry(player).or_insert(0) += 1;
}

/// One inactivity tick — the warning banner, or the kick itself.
pub(crate) fn inactivity_tick(world: &mut World, player: i32, warning: bool, seq: u64) {
    if world.events.tvt.inactivity_seq.get(&player) != Some(&seq) {
        return; // a re-arm (or a cancel) retired this pair
    }
    if !is_on_event(world, player) || world.events.tvt.world_id.is_none() {
        return;
    }
    if warning {
        send_screen(world, player, "You have been marked as inactive!", 10);
        return;
    }
    // Kick: strip the participant, oust them from the arena, and either forfeit
    // the match (their team is now empty) or announce the kick.
    let name = helpers::player_name_or_empty(world, player);
    set_team(world, player, 0);
    instances::exit(world, player);
    world.events.tvt.player_list.retain(|&p| p != player);
    world.events.tvt.scores.remove(&player);
    world.events.tvt.blue_team.retain(|&p| p != player);
    world.events.tvt.red_team.retain(|&p| p != player);
    set_on_event(world, player, false);
    send_player_message(world, player, "You have been kicked for been inactive.");

    let (blue_empty, red_empty) = (
        world.events.tvt.blue_team.is_empty(),
        world.events.tvt.red_team.is_empty(),
    );
    if blue_empty != red_empty {
        manage_forfeit(world);
    } else if let Some(instance_id) = world.events.tvt.world_id {
        broadcast_screen(
            world,
            instance_id,
            &format!("Player {name} was kicked for been inactive!"),
            7,
        );
    }
}

/// A screen banner for one player (Java `sendScreenMessage`).
fn send_screen(world: &World, player: i32, text: &str, secs: i32) {
    helpers::send_to_player(
        world,
        player,
        sp::ex_show_screen_message(text, TOP_CENTER, secs * 1000),
    );
}

/// Java `player.sendMessage(...)` — the plain white chat line.
fn send_player_message(world: &World, player: i32, text: &str) {
    helpers::send_sm_to_player(
        world,
        player,
        sp::sm_ids::S1_TEXT,
        &[sp::SmParam::Text(text.to_string())],
    );
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
                // `AntiFeedManager.tryAddPlayer(L2EVENT_ID, player, max)` — the
                // dualbox cap, with its own refusal page.
                if !ip_slot_available(world, player) {
                    return Some("registration-ip.html".to_string());
                }
                world.events.tvt.player_list.push(player);
                world.events.tvt.scores.insert(player, 0);
                set_registered(world, player, true);
                // (`addLogoutListener` — the port hooks every disconnect
                // through `on_player_logout` instead of per-player listeners.)
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
            buff_heal(world, player);
            None
        }
        _ => None,
    }
}

/// Java `TvT.onEvent("BuffHeal")` — the in-arena manager's service: the
/// class-appropriate buff set plus a full HP/MP/CP top-up, refused in combat
/// (`manager-combat.html`). Available to participants and GMs.
fn buff_heal(world: &mut World, player: i32) {
    if !is_on_event(world, player)
        && !world
            .objects
            .get_component::<Player>(&player)
            .is_some_and(|p| p.is_gm(&world.data))
    {
        return;
    }
    if in_combat(world, player) {
        return;
    }
    let Some(manager) = arena_manager_near(world, player) else {
        return;
    };
    let class_id = world
        .objects
        .get_component::<Player>(&player)
        .map_or(0, |p| p.class_id);
    let is_mage = world.data.categories.contains("BEGINNER_MAGE", class_id);
    for &skill in if is_mage { MAGE_BUFFS } else { FIGHTER_BUFFS } {
        crate::game_loop::npc::support_magic::cast_from_npc(world, manager, player, skill);
    }
    // `setCurrentHp/Mp/Cp(max)` — the heal half. CP lives on its own
    // player-only component.
    let mut updates = Vec::new();
    if let Some(vitals) = world
        .objects
        .get_component_mut::<crate::model::components::Vitals>(&player)
    {
        vitals.cur_hp = f64::from(vitals.max_hp);
        vitals.cur_mp = f64::from(vitals.max_mp);
        updates.push((sp::status_update_type::CUR_HP, vitals.max_hp));
        updates.push((sp::status_update_type::CUR_MP, vitals.max_mp));
    }
    if let Some(pv) = world
        .objects
        .get_component_mut::<crate::model::components::PlayerVitals>(&player)
    {
        pv.cur_cp = f64::from(pv.max_cp);
        updates.push((sp::status_update_type::CUR_CP, pv.max_cp));
    }
    helpers::send_to_player(world, player, sp::status_update(player, &updates));
    crate::game_loop::party::notify_party_vitals(world, player);
}

/// The in-arena manager copy nearest the player — the NPC whose buffs these
/// are (Java passes the clicked `npc` straight through).
fn arena_manager_near(world: &World, player: i32) -> Option<i32> {
    world
        .objects
        .get_component::<crate::model::components::LastFolkNpc>(&player)
        .map(|&crate::model::components::LastFolkNpc(npc)| npc)
}

/// Java's `FIGHTER_BUFFS` / `MAGE_BUFFS` (the event manager's service set).
const FIGHTER_BUFFS: &[(i32, i32)] = &[
    (4322, 1), // Wind Walk
    (4323, 1), // Shield
    (5637, 1), // Magic Barrier
    (4324, 1), // Bless the Body
    (4325, 1), // Vampiric Rage
    (4326, 1), // Regeneration
    (5632, 1), // Haste
];
const MAGE_BUFFS: &[(i32, i32)] = &[
    (4322, 1), // Wind Walk
    (4323, 1), // Shield
    (5637, 1), // Magic Barrier
    (4328, 1), // Bless the Soul
    (4329, 1), // Acumen
    (4330, 1), // Concentration
    (4331, 1), // Empower
];

// ---------------------------------------------------------------------------
// Registration eligibility (Java `canRegister`)
// ---------------------------------------------------------------------------

/// Java `TvT.canRegister(player)` — every gate ported.
fn can_register(world: &mut World, client_id: u32, player: i32) -> bool {
    if world.events.tvt.player_list.contains(&player) {
        helpers::send_message(
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
        helpers::send_message(world, client_id, "Your level is too low to participate.");
        return false;
    }
    if level > MAXIMUM_PARTICIPANT_LEVEL {
        helpers::send_message(world, client_id, "Your level is too high to participate.");
        return false;
    }
    if already_reg {
        helpers::send_message(world, client_id, "You are already registered on an event.");
        return false;
    }
    if world.events.tvt.player_list.len() >= MAXIMUM_PARTICIPANT_COUNT {
        helpers::send_message(
            world,
            client_id,
            "There are too many players registered on the event.",
        );
        return false;
    }
    if cursed || reputation < 0 {
        helpers::send_message(
            world,
            client_id,
            "People with bad reputation can't register.",
        );
        return false;
    }
    if world.olympiad.is_registered(player) {
        helpers::send_message(
            world,
            client_id,
            "You cannot participate while registered on the Olympiad.",
        );
        return false;
    }
    if is_fishing(world, player) {
        helpers::send_message(world, client_id, "You cannot register while fishing.");
        return false;
    }
    // `isInOlympiadMode()` — Java ORs this with the registration check above;
    // a noble already *fighting* a bout is not in the waiting list.
    if world.olympiad.in_competition.contains(&player) {
        helpers::send_message(
            world,
            client_id,
            "You cannot participate while registered on the Olympiad.",
        );
        return false;
    }
    let (flying, transformed) = world
        .objects
        .get_component::<Player>(&player)
        .map_or((false, false), |p| (p.is_flying(), p.transform_id != 0));
    if flying {
        helpers::send_message(
            world,
            client_id,
            "You cannot register on the event while flying.",
        );
        return false;
    }
    if transformed {
        helpers::send_message(
            world,
            client_id,
            "You cannot register on the event while on a transformed state.",
        );
        return false;
    }
    if crate::game_loop::combat::duel::is_in_duel(world, player) {
        helpers::send_message(world, client_id, "You cannot register while on a duel.");
        return false;
    }
    // `isInInstance()` — the overworld is instance 0.
    if helpers::instance_of(world, player) != 0 {
        helpers::send_message(
            world,
            client_id,
            "You cannot register while in an instance.",
        );
        return false;
    }
    // `isInSiege() || isInsideZone(SIEGE)` — Java checks both, and they are not
    // the same question: the first is "a siege I take part in is running", the
    // second is "I am standing on castle ground" even in peacetime.
    let in_siege_zone = world
        .objects
        .get_component::<crate::model::components::ZoneFlags>(&player)
        .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Siege));
    if crate::game_loop::combat::pvp::is_in_siege(world, player) || in_siege_zone {
        helpers::send_message(world, client_id, "You cannot register while on a siege.");
        return false;
    }
    if !crate::game_loop::stats::weight::is_inventory_under_80(world, player) {
        helpers::send_message(
            world,
            client_id,
            "There are too many items in your inventory.",
        );
        helpers::send_message(world, client_id, "Try removing some items.");
        return false;
    }
    // `getWeightPenalty() != 0` — *any* penalty band, not just overloaded.
    if crate::game_loop::stats::weight::current_penalty(world, player) != 0 {
        helpers::send_message(
            world,
            client_id,
            "Your invetory weight has exceeded the normal limit.",
        );
        helpers::send_message(world, client_id, "Try removing some items.");
        return false;
    }
    true
}

/// Java's per-team grouping in `StartFight`: parties of `PARTY_MEMBER_COUNT`
/// (7) under FINDERS_KEEPERS, and — when the team is bigger than one party —
/// a command channel formed around the first party with every later party
/// added to it.
fn group_team(world: &mut World, team: &[i32]) {
    /// Java `PARTY_MEMBER_COUNT`.
    const PARTY_MEMBER_COUNT: usize = 7;
    if team.len() < 2 {
        return;
    }
    let mut cc_id: Option<u32> = None;
    let mut current_party: Option<u32> = None;
    for (i, &member) in team.iter().enumerate() {
        if i % PARTY_MEMBER_COUNT == 0 {
            let party_id = world.next_party_id;
            world.next_party_id += 1;
            let seq = world.next_request_seq();
            world.parties.insert(
                party_id,
                crate::model::party::Party::new(
                    member,
                    crate::model::party::LootRule::FindersKeepers,
                    seq,
                ),
            );
            world
                .objects
                .add_components(&member, crate::model::components::PartyRef(party_id));
            current_party = Some(party_id);
            if team.len() > PARTY_MEMBER_COUNT {
                match cc_id {
                    None => {
                        cc_id = Some(crate::game_loop::party::command_channel::create_channel(
                            world, member, party_id,
                        ));
                    }
                    Some(cc) => {
                        crate::game_loop::party::command_channel::add_party_to_channel(
                            world, cc, party_id,
                        );
                    }
                }
            }
        } else if let Some(pid) = current_party {
            crate::game_loop::party::add_party_member(world, pid, member);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn set_registered(world: &mut World, player: i32, value: bool) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&player) {
        p.registered_on_event = value;
    }
}

fn set_on_event(world: &mut World, player: i32, value: bool) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&player) {
        p.on_event = value;
    }
}

fn set_team(world: &mut World, player: i32, team: u8) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&player) {
        p.team = team;
    }
}

/// Toggle a participant's invulnerability (Java `setInvul`) — the end-of-match
/// freeze. Presence-based `AdminFlags`, added on first use.
/// `EndFight`'s freeze and the "Enable players" thaw, as one call: Java's
/// `setInvul` + `setImmobilized` + `disable/enableAllSkills`, applied to the
/// participant **and their servitor**.
///
/// **Deviation from Java, deliberate.** Java's thaw block unfreezes the player
/// but re-runs `setInvul(true)`/`setImmobilized(true)`/`disableAllSkills()` on
/// the servitor — plainly a copy-paste of the freeze block, since nothing ever
/// undoes it. A pet that survived a TvT event would stay invulnerable and
/// unable to move or cast for the rest of the session. Recorded in
/// `docs/CUSTOM_DIST_DEVIATIONS.md`.
fn set_frozen(world: &mut World, player: i32, frozen: bool) {
    use crate::model::components::{Immobilized, SkillsDisabled};

    let mut targets = vec![player];
    targets.extend(crate::game_loop::servitor::servitor_of(world, player));
    for oid in targets {
        set_invul(world, oid, frozen);
        if frozen {
            world.objects.add_components(&oid, Immobilized);
            world.objects.add_components(&oid, SkillsDisabled);
        } else {
            world.objects.remove_component::<Immobilized>(&oid);
            world.objects.remove_component::<SkillsDisabled>(&oid);
        }
    }
}

fn set_invul(world: &mut World, player: i32, value: bool) {
    helpers::update_admin_flags(world, player, |f| f.invul = value);
}

/// The winner's firework flourish (Java `broadcastPacket(new MagicSkillUse(...))`).
fn firework(world: &World, player: i32) {
    let Some(pos) = maybe_position(world, player) else {
        return;
    };
    let src = (player, pos.x, pos.y, pos.z);
    let pkt = sp::magic_skill_use_raw(src, src, FIREWORK_SKILL, 1, 500);
    broadcast::broadcast_including_self(world, player, &pkt);
}

/// A player's `SocialAction` to everyone who can see them (Java
/// `broadcastSocialAction`).
fn broadcast_social(world: &World, player: i32, action: i32) {
    let pkt = sp::social_action(player, action);
    broadcast::broadcast_including_self(world, player, &pkt);
}

/// Build the score rows (name, score) sorted by score descending — Java
/// `Util.sortByValue(PLAYER_SCORES, true)` — and broadcast `ExPVPMatchCCRecord`
/// to the arena.
fn broadcast_scoreboard(world: &mut World, instance_id: i32, state: i32) {
    let mut rows: Vec<(String, i32)> = world
        .events
        .tvt
        .scores
        .iter()
        .filter_map(|(&oid, &score)| {
            world
                .objects
                .get_component::<Player>(&oid)
                .map(|p| (p.name.clone(), score))
        })
        .collect();
    rows.sort_by_key(|b| std::cmp::Reverse(b.1));
    let refs: Vec<(&str, i32)> = rows.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    let pkt = sp::ex_pvp_match_cc_record(state, &refs);
    instances::broadcast_to_instance(world, instance_id, &pkt);
}

/// Broadcast a top-center screen banner to the arena for `secs` seconds.
fn broadcast_screen(world: &World, instance_id: i32, text: &str, secs: i32) {
    let pkt = sp::ex_show_screen_message(text, TOP_CENTER, secs * 1000);
    instances::broadcast_to_instance(world, instance_id, &pkt);
}

pub(crate) fn is_on_event(world: &World, player: i32) -> bool {
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
    despawn_npc_by_oid(world, oid);
}

/// Load a manager page and substitute `%player_numbers%` (Java builds these two
/// via `html.replace`). Returned content starts with `<html>`, so the quest
/// framework renders it inline.
fn count_page(world: &World, viewer_oid: i32, file: &str) -> String {
    manager_html(world, viewer_oid, file).replace(
        "%player_numbers%",
        &world.events.tvt.player_list.len().to_string(),
    )
}

fn manager_html(world: &World, viewer_oid: i32, file: &str) -> String {
    let root = &world.data.root;
    crate::data::htm_cache::read_htm_for(
        world,
        viewer_oid,
        format!("{root}data/scripts/custom/events/TeamVsTeam/{file}"),
    )
    .unwrap_or_default()
}

/// `AntiFeedManager.tryAddPlayer(L2EVENT_ID, player, DUALBOX_CHECK_MAX_L2EVENT_
/// PARTICIPANTS_PER_IP)` — is there room for one more entrant from this
/// player's IP? `0` means unlimited and Java skips the call entirely.
///
/// Java keeps its own per-event IP counter, incremented here and decremented on
/// `CancelParticipation`; the port instead **counts the live roster**, which
/// cannot drift out of step with it (Java's counter leaks a slot when a
/// registrant disconnects without cancelling). An offline or session-less
/// player contributes nothing, as it does not in Java either.
fn ip_slot_available(world: &World, player: i32) -> bool {
    let max = world.cfg.dualbox.max_event_participants_per_ip;
    if max <= 0 {
        return true;
    }
    let Some(ip) = player_ip(world, player) else {
        // Java: `tryAddClient` returns false with no client — no IP, no entry.
        return false;
    };
    let taken = world
        .events
        .tvt
        .player_list
        .iter()
        .filter(|&&p| player_ip(world, p).as_deref() == Some(ip.as_str()))
        .count();
    (taken as i32) < world.cfg.dualbox.event_limit_for(&ip)
}

/// A registered player's live client IP, or `None` if they have no session.
fn player_ip(world: &World, player: i32) -> Option<String> {
    helpers::client_for_player(world, player)
        .and_then(|cid| world.clients.get(&cid))
        .map(|cs| cs.addr().ip().to_string())
}
