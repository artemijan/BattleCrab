//! Team vs Team — the representative event for the G28 gate. Port of
//! `custom/events/TeamVsTeam/TvT.java`. Slices 1–2: the lifecycle, the
//! **registration phase** (manager NPC, register/cancel window), and the
//! **arena stand-up** (coliseum instance, team split + teleport, buffers,
//! scoreboard, the fight-window door/timer chain through a minimal teardown).
//! Per-kill **scoring**, respawn, zone kicks, and winner **rewards** are slices
//! 3–4 (see `docs/PLAN_G28_EVENTS_ENGINE.md`), flagged `TODO(G28)` at the seams.

use commons::util::rnd;
use tracing::warn;

use crate::enums::ChatType;
use crate::game_loop::death::{despawn_npc, introduce_npc, teleport_player};
use crate::game_loop::instances;
use crate::model::Player;
use crate::model::components::{FishingSession, RegionCell};
use crate::model::event::TvtPhase;
use crate::model::npc::spawn_npc_at;
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

    // Enough players — stand the arena up.
    let Some(instance_id) = instances::create_from_template(world, INSTANCE_ID) else {
        warn!("TvT: failed to create coliseum instance {INSTANCE_ID}; canceling.");
        announce(world, "TvT Event: Event was canceled.");
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
        // TODO(G28): leaveParty + party-of-7 / command-channel grouping, and
        //   addDeathListener — both slice 3 (scoring needs a CC primitive we
        //   don't have yet; team membership is tracked in blue/red_team here).
    }

    // The two arena buffers (the manager NPC reused).
    instances::spawn_npc(
        world,
        instance_id,
        MANAGER,
        BLUE_BUFFER.0,
        BLUE_BUFFER.1,
        BLUE_BUFFER.2,
        BLUE_BUFFER.3,
    );
    instances::spawn_npc(
        world,
        instance_id,
        MANAGER,
        RED_BUFFER.0,
        RED_BUFFER.1,
        RED_BUFFER.2,
        RED_BUFFER.3,
    );

    // Initialize the scoreboard (scores already 0 from registration).
    broadcast_scoreboard(world, instance_id, sp::PVP_MATCH_INITIALIZE);

    world.events.tvt.phase = TvtPhase::Warmup;
    world.scheduler.schedule(
        world.tick + WAIT_TIME_MIN * 60 * TICKS_PER_SECOND,
        ScheduledTask::TvtStartFight,
    );
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
    // TODO(G28): the 5..1 warm-up countdown screen messages (cosmetic, slice 3).
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

    instances::open_close_door(world, instance_id, BLUE_DOOR_ID, false);
    instances::open_close_door(world, instance_id, RED_DOOR_ID, false);

    // Freeze participants (invulnerable) and revive any dead one. Java also
    // immobilizes + disables skills (incl. servitors); TODO(G28): no
    // immobilize / skill-lock flag on this port, so the freeze is invul-only.
    for player in world.events.tvt.player_list.clone() {
        set_invul(world, player, true);
        if is_dead(world, player) {
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
        set_invul(world, player, false);
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
        if crate::game_loop::helpers::instance_of(world, player) != instance_id {
            continue;
        }
        firework(world, player);
        broadcast_social(world, player, SOCIAL_WIN);
        if let Some(cid) = crate::game_loop::helpers::client_for_player(world, player) {
            crate::game_loop::quests::give_item_with_earned_message(
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
    if !is_on_event(world, player) || !is_dead(world, player) {
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
    if let Some(skill) = world.data.skill_data.get(GHOST_WALKING, 1).cloned() {
        crate::game_loop::skills::effects::apply_skill_effects(world, player, player, &skill);
    }
    // TODO(G28): resetActivityTimers — the inactivity kick timers are slice 4.
}

fn team_of(world: &World, player: i32) -> u8 {
    world
        .objects
        .get_component::<Player>(&player)
        .map_or(TEAM_NONE, |p| p.team)
}

fn is_dead(world: &World, player: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::Vitals>(&player)
        .is_some_and(|v| v.dead)
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
fn set_invul(world: &mut World, player: i32, value: bool) {
    use crate::model::components::AdminFlags;
    if world.objects.get_component::<AdminFlags>(&player).is_none() {
        if !value {
            return; // absent already means every flag false
        }
        world.objects.add_components(&player, AdminFlags::default());
    }
    if let Some(f) = world.objects.get_component_mut::<AdminFlags>(&player) {
        f.invul = value;
    }
}

/// The winner's firework flourish (Java `broadcastPacket(new MagicSkillUse(...))`).
fn firework(world: &World, player: i32) {
    let Some(pos) = world
        .objects
        .get_component::<crate::model::components::Position>(&player)
        .copied()
    else {
        return;
    };
    let src = (player, pos.x, pos.y, pos.z);
    let pkt = sp::magic_skill_use_raw(src, src, FIREWORK_SKILL, 1, 500);
    crate::game_loop::helpers::broadcast_including_self(world, player, &pkt);
}

/// A player's `SocialAction` to everyone who can see them (Java
/// `broadcastSocialAction`).
fn broadcast_social(world: &World, player: i32, action: i32) {
    let pkt = sp::social_action(player, action);
    crate::game_loop::helpers::broadcast_including_self(world, player, &pkt);
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
    rows.sort_by(|a, b| b.1.cmp(&a.1));
    let refs: Vec<(&str, i32)> = rows.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    let pkt = sp::ex_pvp_match_cc_record(state, &refs);
    instances::broadcast_to_instance(world, instance_id, &pkt);
}

/// Broadcast a top-center screen banner to the arena for `secs` seconds.
fn broadcast_screen(world: &World, instance_id: i32, text: &str, secs: i32) {
    let pkt = sp::ex_show_screen_message(text, TOP_CENTER, secs * 1000);
    instances::broadcast_to_instance(world, instance_id, &pkt);
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
