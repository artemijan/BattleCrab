//! Dr. Chaos (`ai/bosses/DrChaos`) — the paranoia transformation.
//!
//! Dr. Chaos (32033) is a small paranoid NPC who **becomes** the Gigantic
//! Chaos Golem (25512, the tracked grand boss) when players linger near him or
//! talk to him: a `pissed_off` timer starts at 30 and drains 1 per nearby
//! living player per second (1–5 more on a talk); at ≤0 he transforms. The
//! golem despawns after 30 idle minutes (back to Dr. Chaos) or, on death,
//! arms a `(36 ± 24)h` reset that respawns Dr. Chaos.
//!
//! The golem has **no config respawn window**, so the shared grand-boss
//! lifecycle skips 25512 entirely — this module owns its status, kill and
//! boot. The status field (on the golem's `grand_bosses` record) is DrChaos's
//! own three-state ladder, distinct from the two-/four-state ones elsewhere.

use crate::game_loop::helpers::maybe_position;
use crate::game_loop::time::{MILLIS_PER_HOUR, TICKS_PER_SECOND};
use crate::geo::distance::within_2d_xy;
use crate::model::components::{DrChaosGolem, DrChaosState, Vitals};
use crate::scheduler::ScheduledTask;
use crate::world::World;

pub const DOCTOR_CHAOS: i32 = 32033;
pub const CHAOS_GOLEM: i32 = 25512;

/// DrChaos's own status ladder, stored on the golem's `grand_bosses` record.
pub const NORMAL: i32 = 0; // Dr. Chaos NPC is up; entry to the fight is possible.
pub const CRAZY: i32 = 1; // The golem is up.
pub const DEAD: i32 = 2; // The golem was killed, awaiting reset.

/// `addSpawn(DOCTOR_CHAOS, 96320, -110912, -3328, 8191, …)`.
const DR_CHAOS_SPAWN: (i32, i32, i32, i32) = (96_320, -110_912, -3_328, 8_191);
/// The golem's spawn point on transform (`addSpawn(CHAOS_GOLEM, 96080, …)`).
const GOLEM_SPAWN: (i32, i32, i32) = (96_080, -110_822, -3_343);
/// Dr. Chaos's cosmetic walk-to-grotto target before the golem appears.
const GROTTO: (i32, i32, i32) = (95_928, -110_671, -3_340);

/// `_lastAttackVsGolem + 1800000` — 30 idle minutes despawns the golem.
const GOLEM_IDLE_TICKS: u64 = 30 * 60 * TICKS_PER_SECOND;
/// `paranoia_activity` / `golem_despawn` cadence.
const PARANOIA_TICKS: u64 = TICKS_PER_SECOND;
const DESPAWN_CHECK_TICKS: u64 = 60 * TICKS_PER_SECOND;
/// The paranoia proximity radius (`getVisibleObjectsInRange(npc, …, 500)`).
const PARANOIA_RANGE: f64 = 500.0;

fn status(world: &World) -> Option<i32> {
    world.grand_bosses.get(&CHAOS_GOLEM).map(|b| b.status)
}

/// Boot resolution (Java's ctor branch). Runs after the shared grand-boss boot
/// — the golem carries no config window, so this is its only spawner.
pub(crate) fn resolve_at_boot(world: &mut World) {
    // Only if 25512 is a tracked boss on this dist.
    let Some(st) = status(world) else { return };
    match st {
        CRAZY => {
            // The golem was up when the server stopped: bring it back with its
            // stored vitals and re-arm the idle clock.
            respawn_golem_from_record(world);
        }
        DEAD => {
            let remaining = world
                .grand_bosses
                .get(&CHAOS_GOLEM)
                .map(|b| b.respawn_time - commons::util::now_millis())
                .unwrap_or(0);
            if remaining > 0 {
                world.scheduler.schedule(
                    world.tick + (remaining / 1000).max(1) as u64 * TICKS_PER_SECOND,
                    ScheduledTask::DrChaosReset,
                );
            } else {
                // The window elapsed while the server was down — Dr. Chaos
                // returns now (the boss-lifecycle "elapsed during downtime"
                // trap: miss it and he never comes back).
                do_reset(world);
            }
        }
        // NORMAL (or a fresh record): Dr. Chaos stands.
        _ => spawn_dr_chaos(world),
    }
}

/// `addSpawn(DOCTOR_CHAOS, …)` + `onSpawn` (arm the paranoia timer at 30).
fn spawn_dr_chaos(world: &mut World) {
    let Some(oid) = crate::game_loop::npc::spawn_npc_at(
        world,
        DOCTOR_CHAOS,
        DR_CHAOS_SPAWN.0,
        DR_CHAOS_SPAWN.1,
        DR_CHAOS_SPAWN.2,
        DR_CHAOS_SPAWN.3,
    ) else {
        return;
    };
    crate::game_loop::death::introduce_npc(world, oid);
    world
        .objects
        .add_components(&oid, DrChaosState { pissed_off: 30 });
    world.scheduler.schedule(
        world.tick + PARANOIA_TICKS,
        ScheduledTask::DrChaosParanoia { dr_chaos_oid: oid },
    );
}

/// `paranoia_activity` — drain the timer by one per nearby living player, bark
/// the warning at exactly 15, transform at ≤0. Reschedules while NORMAL.
pub(crate) fn handle_paranoia(world: &mut World, dr_chaos_oid: i32) {
    if status(world) != Some(NORMAL) {
        return; // stopped (transformed or reset)
    }
    // Dr. Chaos may have been removed/replaced; bail if the timer is gone.
    if world
        .objects
        .get_component::<DrChaosState>(&dr_chaos_oid)
        .is_none()
    {
        return;
    }
    let nearby = living_players_near(world, dr_chaos_oid, PARANOIA_RANGE);
    let mut transformed = false;
    for _ in 0..nearby {
        let timer = {
            let Some(st) = world
                .objects
                .get_component_mut::<DrChaosState>(&dr_chaos_oid)
            else {
                break;
            };
            st.pissed_off -= 1;
            st.pissed_off
        };
        if timer == 15 {
            crate::game_loop::npc::say::npc_say_text(
                world,
                dr_chaos_oid,
                "How dare you trespass into my territory! Have you no fear?",
            );
        }
        if timer <= 0 {
            become_angry(world, dr_chaos_oid);
            transformed = true;
            break;
        }
    }
    if !transformed {
        world.scheduler.schedule(
            world.tick + PARANOIA_TICKS,
            ScheduledTask::DrChaosParanoia { dr_chaos_oid },
        );
    }
}

/// `crazyMidgetBecomesAngry` — status CRAZY, the "Fools!" bark, the walk to the
/// grotto (a teleport here), and the five transformation beats.
fn become_angry(world: &mut World, dr_chaos_oid: i32) {
    if status(world) != Some(NORMAL) {
        return;
    }
    crate::game_loop::grand_boss::set_status(world, CHAOS_GOLEM, CRAZY);
    world
        .objects
        .remove_component::<DrChaosState>(&dr_chaos_oid);
    // `setIntention(MOVE_TO, grotto)` — cosmetic; teleport rather than model
    // the walk (scripted bosses teleport elsewhere in the port too).
    crate::game_loop::death::relocate_npc(world, dr_chaos_oid, GROTTO.0, GROTTO.1, GROTTO.2, 0);
    crate::game_loop::npc::say::npc_say_text(
        world,
        dr_chaos_oid,
        "Fools! Why haven't you fled yet? Prepare to learn a lesson!",
    );
    for (step, delay_ms) in [
        (1u8, 2_000u64),
        (2, 4_000),
        (3, 6_500),
        (4, 12_500),
        (5, 17_000),
    ] {
        world.scheduler.schedule(
            world.tick + delay_ms / 1000 * TICKS_PER_SECOND,
            ScheduledTask::DrChaosTransform { dr_chaos_oid, step },
        );
    }
}

/// One transformation beat. Beats 1–4 are Dr. Chaos animations/cameras; beat 5
/// deletes him and spawns the golem.
pub(crate) fn handle_transform(world: &mut World, dr_chaos_oid: i32, step: u8) {
    use crate::network::server_packets::{social_action, special_camera};
    match step {
        1 => {
            crate::game_loop::helpers::broadcast_from(
                world,
                dr_chaos_oid,
                &social_action(dr_chaos_oid, 2),
            );
            crate::game_loop::helpers::broadcast_from(
                world,
                dr_chaos_oid,
                &special_camera(dr_chaos_oid, 1, -200, 15, 5500, 1000, 13500, 0, 0, 0, 0, 0),
            );
        }
        2 => crate::game_loop::helpers::broadcast_from(
            world,
            dr_chaos_oid,
            &social_action(dr_chaos_oid, 3),
        ),
        3 => crate::game_loop::helpers::broadcast_from(
            world,
            dr_chaos_oid,
            &social_action(dr_chaos_oid, 1),
        ),
        4 => {
            crate::game_loop::helpers::broadcast_from(
                world,
                dr_chaos_oid,
                &special_camera(dr_chaos_oid, 1, -150, 10, 3500, 1000, 5000, 0, 0, 0, 0, 0),
            );
            crate::game_loop::death::relocate_npc(
                world,
                dr_chaos_oid,
                GROTTO.0,
                GROTTO.1,
                GROTTO.2,
                0,
            );
        }
        5 => {
            // Delete Dr. Chaos, spawn the golem with its intro.
            crate::game_loop::death::despawn_npc_by_oid(world, dr_chaos_oid);
            spawn_golem(world, GOLEM_SPAWN.0, GOLEM_SPAWN.1, GOLEM_SPAWN.2, false);
        }
        _ => {}
    }
}

/// Spawn the golem, wire its idle clock, and (for a fresh transform) play its
/// intro cinematic. `restore` skips the intro and uses stored HP (boot's CRAZY
/// branch).
fn spawn_golem(world: &mut World, x: i32, y: i32, z: i32, restore: bool) -> Option<i32> {
    use crate::network::server_packets::{play_sound, social_action, special_camera};
    let oid = crate::game_loop::npc::spawn_npc_at(world, CHAOS_GOLEM, x, y, z, 0)?;
    crate::game_loop::death::introduce_npc(world, oid);
    world.objects.add_components(
        &oid,
        DrChaosGolem {
            last_attack_tick: world.tick,
        },
    );
    if !restore {
        crate::game_loop::helpers::broadcast_from(
            world,
            oid,
            &special_camera(oid, 30, 200, 20, 6000, 700, 8000, 0, 0, 0, 0, 0),
        );
        crate::game_loop::helpers::broadcast_from(world, oid, &social_action(oid, 1));
        crate::game_loop::helpers::broadcast_from(world, oid, &play_sound("Rm03_A"));
    }
    world.scheduler.schedule(
        world.tick + DESPAWN_CHECK_TICKS,
        ScheduledTask::DrChaosGolemDespawn { golem_oid: oid },
    );
    Some(oid)
}

/// Boot's CRAZY branch: respawn the golem at its stored location/vitals.
fn respawn_golem_from_record(world: &mut World) {
    let Some(b) = world.grand_bosses.get(&CHAOS_GOLEM).cloned() else {
        return;
    };
    if let Some(oid) = spawn_golem(world, b.loc_x, b.loc_y, b.loc_z, true)
        && b.current_hp > 0.0
        && let Some(v) = world.objects.get_component_mut::<Vitals>(&oid)
    {
        v.cur_hp = b.current_hp.min(v.max_hp as f64);
        v.cur_mp = b.current_mp.min(v.max_mp as f64);
    }
}

/// `golem_despawn` — 30 idle minutes reverts to Dr. Chaos; else reschedule.
pub(crate) fn handle_golem_despawn(world: &mut World, golem_oid: i32) {
    let Some(g) = world
        .objects
        .get_component::<DrChaosGolem>(&golem_oid)
        .copied()
    else {
        return;
    };
    if world.tick.saturating_sub(g.last_attack_tick) >= GOLEM_IDLE_TICKS {
        crate::game_loop::death::despawn_npc_by_oid(world, golem_oid);
        crate::game_loop::grand_boss::set_status(world, CHAOS_GOLEM, NORMAL);
        spawn_dr_chaos(world);
    } else {
        world.scheduler.schedule(
            world.tick + DESPAWN_CHECK_TICKS,
            ScheduledTask::DrChaosGolemDespawn { golem_oid },
        );
    }
}

/// `onAttack` (golem): refresh the idle clock and, `rnd(300) < 3`, a taunt.
pub(crate) fn on_golem_attacked(world: &mut World, golem_oid: i32) {
    if let Some(g) = world.objects.get_component_mut::<DrChaosGolem>(&golem_oid) {
        g.last_attack_tick = world.tick;
    }
    let chance = world.roll(300);
    if chance < 3 {
        let line = match chance {
            0 => "Bwah-ha-ha! Your doom is at hand! Behold the Ultra Secret Super Weapon!",
            1 => "Foolish, insignificant creatures! How dare you challenge me!",
            _ => "I see that none will challenge me now!",
        };
        crate::game_loop::npc::say::npc_say_text(world, golem_oid, line);
    }
}

/// `onKill` (golem): DEAD, a `(36 ± 24)h` reset, and the parting bark.
pub(crate) fn on_golem_killed(world: &mut World, golem_oid: i32) {
    crate::game_loop::npc::say::npc_say_text(
        world,
        golem_oid,
        "Urggh! You will pay dearly for this insult.",
    );
    // `(36 + Rnd.get(-24, 24))` hours — a 12..=60 h window.
    let hours = 36 + (world.roll(49) - 24);
    let respawn_millis = hours.max(1) as i64 * MILLIS_PER_HOUR;
    crate::game_loop::grand_boss::set_status(world, CHAOS_GOLEM, DEAD);
    if let Some(b) = world.grand_bosses.get_mut(&CHAOS_GOLEM) {
        b.respawn_time = commons::util::now_millis() + respawn_millis;
        b.current_hp = 0.0;
        b.current_mp = 0.0;
    }
    crate::game_loop::grand_boss::persist(world, CHAOS_GOLEM);
    world.scheduler.schedule(
        world.tick + (respawn_millis / 1000) as u64 * TICKS_PER_SECOND,
        ScheduledTask::DrChaosReset,
    );
}

/// `reset_drchaos` — the window elapsed: Dr. Chaos returns at NORMAL.
pub(crate) fn handle_reset(world: &mut World) {
    // Only if still dead (a GM could have spawned him meanwhile).
    if status(world) == Some(DEAD) {
        do_reset(world);
    }
}

fn do_reset(world: &mut World) {
    crate::game_loop::grand_boss::set_status(world, CHAOS_GOLEM, NORMAL);
    if let Some(b) = world.grand_bosses.get_mut(&CHAOS_GOLEM) {
        b.respawn_time = 0;
    }
    crate::game_loop::grand_boss::persist(world, CHAOS_GOLEM);
    spawn_dr_chaos(world);
}

/// First-talk (Java `onFirstTalk`): drain 1–5, then a paranoia html by band,
/// or the transform at ≤0. Returns the inline html.
pub(crate) fn on_first_talk(world: &mut World, dr_chaos_oid: i32) -> Option<String> {
    if status(world) != Some(NORMAL) {
        return None;
    }
    let drain = 1 + world.roll(5);
    let timer = {
        let st = world
            .objects
            .get_component_mut::<DrChaosState>(&dr_chaos_oid)?;
        st.pissed_off -= drain;
        st.pissed_off
    };
    if timer > 20 {
        Some("<html><body>Doctor Chaos:<br>What?! Who are you? How did you come here?<br>You really look suspicious... Aren't those filthy members of Black Anvil guild send you? No? Mhhhhh... I don't trust you!</body></html>".into())
    } else if timer > 10 {
        Some("<html><body>Doctor Chaos:<br>Why are you standing here? Don't you see it's a private propertie? Don't look at him with those eyes... Did you smile?! Don't make fun of me! He will ... destroy ... you ... if you continue!</body></html>".into())
    } else if timer > 0 {
        Some("<html><body>Doctor Chaos:<br>I know why you are here, traitor! He discovered your plans! You are assassin ... sent by the Black Anvil guild! But you won't kill the Emperor of Evil!</body></html>".into())
    } else {
        become_angry(world, dr_chaos_oid);
        None
    }
}

// -- helpers ----------------------------------------------------------------

fn living_players_near(world: &World, oid: i32, range: f64) -> usize {
    let Some(origin) = maybe_position(world, oid) else {
        return 0;
    };
    world
        .in_game_player_oids()
        .filter(|p| {
            let alive = world
                .objects
                .get_component::<Vitals>(p)
                .is_some_and(|v| !v.dead);
            alive && within_2d_xy(world, *p, origin.x, origin.y, range)
        })
        .count()
}
