//! The match runtime: the game-manager sweep, match-making draws, the
//! pre-fight countdown ceremony, the match poll and its resolution.

use super::*;
use crate::scheduler::ms_to_ticks;
/// Java `Player.isInOlympiadMode()` — the player is *in a running match*, not
/// merely registered or spectating.
///
/// Distinct from the composite `offline_trade` builds, which also refuses a
/// registered or observing player from going offline. Effects that ask
/// "is this an olympiad fight?" want only the match.
pub(crate) fn in_match(world: &World, object_id: i32) -> bool {
    world
        .olympiad
        .matches
        .iter()
        .any(|m| m.player_a == object_id || m.player_b == object_id)
}

/// `OlympiadGameManager.run`: while the window is open, fill the free stadiums
/// with 1v1 matches drawn from the non-class queue, then reschedule. Stops when
/// the window closes.
pub(crate) fn handle_game_manager(world: &mut World) {
    if !world.olympiad.in_comp_period {
        return; // window closed — the sweep stops until it reopens
    }
    make_matches(world);
    world.scheduler.schedule(
        fire_at(world, GAME_MANAGER_PERIOD_MS),
        ScheduledTask::OlympiadGameManager,
    );
}

/// Pair waiting nobles into the free stadiums. Only runs once the non-class
/// queue is large enough (Java `hasEnoughRegisteredNonClassed`); then each free
/// arena takes a 2-player game until the queue runs dry.
fn make_matches(world: &mut World) {
    if world.olympiad.non_class_registers.len() < world.cfg.olympiad.nonclassed_participants {
        return;
    }
    // The stadium slots already busy with a running match.
    let mut busy: Vec<bool> = vec![false; NUM_ARENAS];
    for m in &world.olympiad.matches {
        if let Some(slot) = busy.get_mut(m.arena) {
            *slot = true;
        }
    }
    for (arena, &is_busy) in busy.iter().enumerate() {
        if is_busy {
            continue;
        }
        let Some((player_a, player_b)) = draw_pair(world) else {
            break; // not enough online players left in the queue
        };
        world.olympiad.in_competition.insert(player_a);
        world.olympiad.in_competition.insert(player_b);
        start_match(world, arena, player_a, player_b);
    }
}

/// The grassy-arena spawn points (`zones/olympiad_stadium.xml`), for player
/// one and player two. Every match shares these coordinates — isolation comes
/// from the per-match private instance `start_match` creates (fighters and
/// observers are scoped to it, so concurrent bouts never see each other; the
/// dist's four arena templates all point at the grassy arena's geometry
/// anyway, three of them explicitly commented "Use Grassy Arena").
const ARENA_SPAWN_A: (i32, i32, i32) = (-89597, -252841, -3320);
const ARENA_SPAWN_B: (i32, i32, i32) = (-86544, -252846, -3320);
/// `AltOlyBattle` — the battle length (5 min); an undecided fight is a draw.
/// How often a running match is polled for a result.
const MATCH_POLL_MS: i64 = 1000;
/// The outcome of a match once it resolves.
enum MatchResult {
    Win { winner: i32, loser: i32 },
    Draw,
}

/// One step of the pre-fight ceremony (Java `OlympiadGameTask`'s countdowns).
enum CountdownStep {
    /// Announce "moved to the stadium / match starts in `secs` seconds" (the
    /// `sm` id) to both fighters.
    Say { sm: i16, secs: i32 },
    /// The teleport countdown reached zero: port both into the arena + strip.
    Enter,
    /// The battle countdown reached zero: the fight begins.
    Fight,
}
use CountdownStep::{Enter, Fight, Say};

/// `(delay_ms_since_previous_step, step)`. First the 120 s teleport countdown
/// (`YOU_WILL_BE_MOVED…` 1492 at `AltOlyWaitTime`'s checkpoints), then the
/// teleport, then the 60 s battle countdown (`THE_MATCH_WILL_START…` 1495).
const COUNTDOWN: &[(u64, CountdownStep)] = &[
    (
        0,
        Say {
            sm: 1492,
            secs: 120,
        },
    ),
    (60_000, Say { sm: 1492, secs: 60 }),
    (30_000, Say { sm: 1492, secs: 30 }),
    (15_000, Say { sm: 1492, secs: 15 }),
    (5_000, Say { sm: 1492, secs: 10 }),
    (5_000, Say { sm: 1492, secs: 5 }),
    (1_000, Say { sm: 1492, secs: 4 }),
    (1_000, Say { sm: 1492, secs: 3 }),
    (1_000, Say { sm: 1492, secs: 2 }),
    (1_000, Say { sm: 1492, secs: 1 }),
    (1_000, Enter),
    (0, Say { sm: 1495, secs: 60 }),
    (5_000, Say { sm: 1495, secs: 55 }),
    (5_000, Say { sm: 1495, secs: 50 }),
    (10_000, Say { sm: 1495, secs: 40 }),
    (10_000, Say { sm: 1495, secs: 30 }),
    (10_000, Say { sm: 1495, secs: 20 }),
    (10_000, Say { sm: 1495, secs: 10 }),
    (5_000, Say { sm: 1495, secs: 5 }),
    (1_000, Say { sm: 1495, secs: 4 }),
    (1_000, Say { sm: 1495, secs: 3 }),
    (1_000, Say { sm: 1495, secs: 2 }),
    (1_000, Say { sm: 1495, secs: 1 }),
    (1_000, Fight),
];

/// Register a match and start its pre-fight ceremony (Java `OlympiadGameTask`
/// from `BEGIN`): the fighters are announced, teleported in + buff-stripped
/// after the wait countdown, and the fight begins after the battle countdown.
pub(crate) fn start_match(world: &mut World, arena: usize, player_a: i32, player_b: i32) {
    // A private instance so concurrent bouts sharing arena coords stay isolated.
    let instance_id = world.instances.create(0);
    // Observers already watching this stadium slot follow it to the new bout
    // (Java's per-slot instance is permanent; ours is per-match, so the
    // spectators must be re-scoped or they'd be stranded in the dead one).
    let mut watching: Vec<i32> = Vec::new();
    world
        .objects
        .for_each_mut::<(&Player, &OlympiadObserver)>(|(p, o)| {
            if o.arena == arena as i32 {
                watching.push(p.object_id);
            }
        });
    for oid in watching {
        world
            .objects
            .add_components(&oid, crate::model::components::InstanceId(instance_id));
    }
    world.olympiad.matches.push(OlympiadMatch {
        arena,
        player_a,
        player_b,
        instance_id,
        deadline_tick: 0, // set when the battle actually begins
        // Both are in the world to have been matched, so the origin fallback
        // is unreachable; it only keeps the return location total.
        return_a: pos_of(world, player_a).unwrap_or((0, 0, 0)),
        return_b: pos_of(world, player_b).unwrap_or((0, 0, 0)),
    });
    tracing::info!("Olympiad: match in arena {arena}: {player_a} vs {player_b}.");
    world.scheduler.schedule(
        world.tick,
        ScheduledTask::OlympiadCountdown { arena, step: 0 },
    );
}

/// Run one ceremony step and schedule the next.
pub(crate) fn handle_countdown(world: &mut World, arena: usize, step: usize) {
    let Some(m) = world
        .olympiad
        .matches
        .iter()
        .find(|m| m.arena == arena)
        .cloned()
    else {
        return; // the match was resolved/aborted
    };
    let Some((_, action)) = COUNTDOWN.get(step) else {
        return;
    };
    match action {
        Say { sm, secs } => {
            for oid in [m.player_a, m.player_b] {
                send_sm_int(world, oid, *sm, *secs);
            }
        }
        Enter => {
            for (oid, spawn) in [(m.player_a, ARENA_SPAWN_A), (m.player_b, ARENA_SPAWN_B)] {
                world
                    .objects
                    .add_components(&oid, crate::model::components::InstanceId(m.instance_id));
                crate::game_loop::death::teleport_player(world, oid, spawn.0, spawn.1, spawn.2);
                strip_buffs(world, oid);
            }
        }
        Fight => {
            // `AltOlyBattle` — the match's own time limit.
            let battle_ms = world.cfg.olympiad.battle_ms;
            let now_tick = world.tick;
            if let Some(mm) = world.olympiad.matches.iter_mut().find(|x| x.arena == arena) {
                mm.deadline_tick = now_tick + ms_to_ticks(battle_ms);
            }
            world.scheduler.schedule(
                fire_at(world, MATCH_POLL_MS),
                ScheduledTask::OlympiadMatchTick { arena },
            );
            return; // the fight is on — the match tick takes over
        }
    }
    if let Some((next_delay, _)) = COUNTDOWN.get(step + 1) {
        world.scheduler.schedule(
            fire_at(world, *next_delay as i64),
            ScheduledTask::OlympiadCountdown {
                arena,
                step: step + 1,
            },
        );
    }
}

/// `AbstractOlympiadGame.removeBuffs`: drop every active (non-passive) buff so
/// nobody enters the arena pre-buffed.
pub(crate) fn strip_buffs(world: &mut World, object_id: i32) {
    crate::game_loop::skills::effects::expire_active_buffs(world, object_id);
}

/// `OlympiadGameTask` poll: a fighter who died or vanished loses; both surviving
/// past the battle deadline is a draw. Otherwise keep watching.
pub(crate) fn handle_match_tick(world: &mut World, arena: usize) {
    let Some(m) = world
        .olympiad
        .matches
        .iter()
        .find(|m| m.arena == arena)
        .cloned()
    else {
        return; // already resolved
    };
    let a_gone = !is_online(world, m.player_a);
    let b_gone = !is_online(world, m.player_b);
    let a_dead = a_gone || is_dead(world, m.player_a);
    let b_dead = b_gone || is_dead(world, m.player_b);

    let result = match (a_dead, b_dead) {
        (false, false) if world.tick < m.deadline_tick => {
            // Battle still on — keep polling.
            world.scheduler.schedule(
                fire_at(world, MATCH_POLL_MS),
                ScheduledTask::OlympiadMatchTick { arena },
            );
            return;
        }
        (true, true) => MatchResult::Draw, // both down (or timeout with both alive → below)
        (true, false) => MatchResult::Win {
            winner: m.player_b,
            loser: m.player_a,
        },
        (false, true) => MatchResult::Win {
            winner: m.player_a,
            loser: m.player_b,
        },
        _ => MatchResult::Draw, // deadline reached, both alive
    };
    resolve_match(world, &m, &result);
}

/// Apply the result (Java `validateWinner`), port both fighters back, and clear
/// the match.
fn resolve_match(world: &mut World, m: &OlympiadMatch, result: &MatchResult) {
    // Java's `olympiad` logger (`AbstractOlympiadGame`/`Olympiad`). Ungated,
    // like accounting: match outcomes decide hero status, so they are kept
    // regardless of the diagnostic settings.
    {
        let name_of = |oid: i32| {
            world
                .objects
                .get_component::<Player>(&oid)
                .map(|p| p.name.clone())
        };
        let (outcome, winner, loser) = match result {
            MatchResult::Win { winner, loser } => ("win", Some(*winner), Some(*loser)),
            MatchResult::Draw => ("draw", None, None),
        };
        commons::audit::record(
            commons::audit::Category::Olympiad,
            serde_json::json!({
                "outcome": outcome,
                "player_a": name_of(m.player_a),
                "player_a_oid": m.player_a,
                "player_b": name_of(m.player_b),
                "player_b_oid": m.player_b,
                "winner": winner.and_then(name_of),
                "loser": loser.and_then(name_of),
            }),
        );
    }

    match result {
        MatchResult::Win { winner, loser } => {
            let diff = point_transfer(world, *winner, *loser);
            update_noble(world, *winner, |n| {
                n.points += diff;
                n.comp_won += 1;
            });
            update_noble(world, *loser, |n| {
                n.points = (n.points - diff).max(0);
                n.comp_lost += 1;
            });
            send_sm(world, *winner, sm_ids::CONGRATULATIONS_C1_YOU_WIN_THE_MATCH);
        }
        MatchResult::Draw => {
            update_noble(world, m.player_a, |n| n.comp_drawn += 1);
            update_noble(world, m.player_b, |n| n.comp_drawn += 1);
        }
    }
    // Both played a match this week (Java increments COMP_DONE / COMP_DONE_WEEK
    // for both regardless of outcome).
    for oid in [m.player_a, m.player_b] {
        update_noble(world, oid, |n| {
            n.comp_done += 1;
            n.comp_done_week += 1;
        });
    }

    // Port the fighters back to the overworld and free them.
    for (oid, ret) in [(m.player_a, m.return_a), (m.player_b, m.return_b)] {
        world.olympiad.in_competition.remove(&oid);
        world
            .objects
            .remove_component::<crate::model::components::InstanceId>(&oid);
        if is_online(world, oid) {
            crate::game_loop::death::teleport_player(world, oid, ret.0, ret.1, ret.2);
        }
    }
    world.olympiad.matches.retain(|x| x.arena != m.arena);
    world.instances.destroy(m.instance_id);
    save_all(world);
}

/// `validateWinner`'s `pointDiff`: `min(winnerPts, loserPts) / divider`,
/// clamped to `[1, ALT_OLY_MAX_POINTS]`.
fn point_transfer(world: &World, winner: i32, loser: i32) -> i32 {
    let wp = world.olympiad.nobles.get(&winner).map_or(0, |n| n.points);
    let lp = world.olympiad.nobles.get(&loser).map_or(0, |n| n.points);
    // Java picks the divider by queue kind; both are 5 on this dist, and the
    // port runs only the non-class queue.
    let o = &world.cfg.olympiad;
    (wp.min(lp) / o.divider_nonclassed.max(1)).clamp(1, o.max_points)
}

fn update_noble(world: &mut World, object_id: i32, f: impl FnOnce(&mut NobleStats)) {
    if let Some(n) = world.olympiad.nobles.get_mut(&object_id) {
        f(n);
    }
}

/// `OlympiadGameNormal.createListOfParticipants`: draw two distinct **online**
/// players at random from the non-class queue, removing them. Offline entries
/// are dropped. Returns `None` if fewer than two online players remain.
fn draw_pair(world: &mut World) -> Option<(i32, i32)> {
    let first = draw_online(world)?;
    match draw_online(world) {
        Some(second) => Some((first, second)),
        None => {
            // No valid opponent — put the first player back (Java re-adds it).
            world.olympiad.non_class_registers.insert(first);
            None
        }
    }
}

/// Remove and return a random online player from the non-class queue, discarding
/// offline entries along the way. `None` when the queue empties.
fn draw_online(world: &mut World) -> Option<i32> {
    loop {
        let len = world.olympiad.non_class_registers.len();
        if len == 0 {
            return None;
        }
        let idx = world.roll(len as i32) as usize;
        let oid = *world.olympiad.non_class_registers.iter().nth(idx)?;
        world.olympiad.non_class_registers.remove(&oid);
        if is_online(world, oid) {
            return Some(oid);
        }
        // Offline: dropped from the queue, keep drawing.
    }
}

pub(super) fn is_online(world: &World, object_id: i32) -> bool {
    world.objects.get_component::<Player>(&object_id).is_some()
}
