//! Java `Quest.getTimers()` — the per-script `TimerExecutor<String>`: one-shot
//! and repeating timers keyed by `(event, npc, player)`, fired into
//! `onTimerEvent`. Separate from the older `startQuestTimer` (per-player
//! `QuestTimerSeqs`), which Salvation scripts still use alongside it.
//!
//! Each live holder is an entry in [`ScriptTimers`] carrying the seq its
//! scheduled task was armed with. Cancelling removes the entry; Java's
//! `stopTimerHolders` (NPC decay, player logout) only kills the task and
//! leaves the holder in the executor's map, so that path re-keys the entry
//! to a fresh seq instead — `hasTimer` keeps answering true for it exactly
//! as Java's does, and the armed task no-ops.

use super::QuestCtx;
use crate::game_loop::helpers;
use crate::scheduler::{ScheduledTask, ms_to_ticks};
use crate::world::World;
use std::collections::HashMap;

/// `TimerHolder.isEqual`'s identity: script, event, npc (0 = none), player
/// (0 = none).
type TimerKey = (&'static str, String, i32, i32);

#[derive(Default)]
pub struct ScriptTimers(HashMap<TimerKey, u64>);

impl ScriptTimers {
    /// Holders in the executor, live or stopped.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
}

/// `TimerExecutor.addTimer` / `addRepeatingTimer`: a same-key holder is
/// cancelled first, then the new one is armed. `period` is the repeat
/// interval (`scheduleAtFixedRate(this, time, time)`), `None` for one-shot.
fn add(
    world: &mut World,
    script: &'static str,
    event: &str,
    delay_ms: u64,
    npc: i32,
    player: i32,
    repeating: bool,
) {
    let seq = world.next_request_seq();
    world
        .script_timers
        .0
        .insert((script, event.to_string(), npc, player), seq);
    let ticks = ms_to_ticks(delay_ms);
    world.scheduler.schedule(
        world.tick + ticks,
        ScheduledTask::ScriptTimer {
            script,
            event: event.to_string(),
            npc,
            player,
            seq,
            period: repeating.then_some(ticks),
        },
    );
}

/// `TimerHolder.cancelTask` for every holder bound to `oid` as its NPC or
/// its player (`Npc.stopTimerHolders` on decay, `Player.deleteMe`'s loop).
pub(crate) fn stop_timer_holders(world: &mut World, oid: i32) {
    let keys: Vec<TimerKey> = world
        .script_timers
        .0
        .keys()
        .filter(|(_, _, npc, player)| *npc == oid || *player == oid)
        .cloned()
        .collect();
    for key in keys {
        let seq = world.next_request_seq();
        world.script_timers.0.insert(key, seq);
    }
}

/// `TimerHolder.run`: `onTimerPostExecute` (a one-shot leaves the map; a
/// repeating one stays and re-arms) then `onTimerEvent`. The script runs with
/// the timer's NPC and player — either may be 0.
pub(crate) fn handle_script_timer(
    world: &mut World,
    script: &'static str,
    event: String,
    npc: i32,
    player: i32,
    seq: u64,
    period: Option<u64>,
) {
    let key = (script, event, npc, player);
    if world.script_timers.0.get(&key) != Some(&seq) {
        return; // cancelled, superseded or its task was stopped
    }
    match period {
        Some(ticks) => world.scheduler.schedule(
            world.tick + ticks.max(1),
            ScheduledTask::ScriptTimer {
                script,
                event: key.1.clone(),
                npc,
                player,
                seq,
                period,
            },
        ),
        None => {
            world.script_timers.0.remove(&key);
        }
    }
    let event = key.1;
    let registry = world.quests.clone();
    let Some(quest) = registry.by_name(script) else {
        return;
    };
    let client_id = helpers::client_for_player(world, player).unwrap_or(0);
    let mut ctx = QuestCtx::new(world, client_id, player, npc, quest.clone());
    quest.on_timer_event(&mut ctx, &event);
}

impl QuestCtx<'_> {
    /// `getTimers().addTimer(event, time, npc, player)`.
    pub fn add_timer(&mut self, event: &str, delay_ms: u64, npc: i32, player: i32) {
        if self.simulated {
            return;
        }
        add(
            self.world,
            self.script.name(),
            event,
            delay_ms,
            npc,
            player,
            false,
        );
    }

    /// `getTimers().addRepeatingTimer(event, time, npc, player)`.
    pub fn add_repeating_timer(&mut self, event: &str, period_ms: u64, npc: i32, player: i32) {
        if self.simulated {
            return;
        }
        add(
            self.world,
            self.script.name(),
            event,
            period_ms,
            npc,
            player,
            true,
        );
    }

    /// `getTimers().cancelTimer(event, npc, player)`.
    pub fn cancel_timer(&mut self, event: &str, npc: i32, player: i32) {
        if self.simulated {
            return;
        }
        self.world
            .script_timers
            .0
            .remove(&(self.script.name(), event.to_string(), npc, player));
    }

    /// `getTimers().hasTimer(event, npc, player)`.
    pub fn has_timer(&self, event: &str, npc: i32, player: i32) -> bool {
        self.world.script_timers.0.contains_key(&(
            self.script.name(),
            event.to_string(),
            npc,
            player,
        ))
    }
}
