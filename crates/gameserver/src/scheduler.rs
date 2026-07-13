//! One-shot timer scheduler — the Rust replacement for the ~300
//! `ThreadPool.schedule(runnable, delay)` call sites (CONCURRENCY_MODEL §2.2).
//!
//! Entries are keyed by the tick they fire on and **capture object IDs, never
//! references**. When a timer fires and its target is already gone, the task is
//! a no-op — exactly the effect Java gets from the `isDead()`/null checks inside
//! each runnable.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// A scheduled unit of work. Grows one variant per Java `schedule(...)` site as
/// milestones land; for now it only carries the id-capturing shape and a test
/// hook, so the loop and heap can be exercised before any real tasks exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduledTask {
    /// Placeholder used by tests and as the template for real tasks.
    Noop { object_id: i32 },
    /// `SkillCaster.run` phase 1 (`launchSkill`), fires `_hitTime` ms after
    /// `startCasting`. The skill/target live in the player's `CastState`;
    /// `cast_seq` must match it or the task is stale (aborted/replaced cast)
    /// and no-ops — the heap-free abort mechanism, same dead-id contract as
    /// everything else here.
    SkillLaunch { player_object_id: i32, cast_seq: u64 },
    /// `SkillCaster.run` phase 2 (`finishSkill`), fires `_cancelTime` ms
    /// after the launch: consume MP/HP, apply effects.
    SkillFinish { player_object_id: i32, cast_seq: u64 },
    /// `SkillCaster.run` end (`stopCasting(false)`), fires `_coolTime` ms
    /// after the finish: the cast slot frees up.
    CastEnd { player_object_id: i32, cast_seq: u64 },
    /// `BuffFinishTask`: an active buff's `abnormalTime` has elapsed.
    BuffExpire { player_object_id: i32, skill_id: i32 },
    /// `CreatureAttackTaskManager.onHitTimeNotDual` — one auto-attack swing
    /// landing. The hit was rolled at swing start (Java
    /// `generateAttackTargetData`) and rides along; attacker/target death or
    /// disappearance before it fires makes it a no-op (Java's `isDead` /
    /// dead-ref checks inside the task).
    AttackHit { attacker: i32, target: i32, damage: i32, miss: bool, crit: bool },
    /// The Rust `EVT_READY_TO_ACT`: a player's swing period ended
    /// (`attack_end_tick`), releasing whatever action the swing held back
    /// (`run_queued_action`); a no-op when nothing is queued.
    AttackFinish { object_id: i32 },
    /// `DecayTaskManager` firing for a dead NPC: the corpse disappears.
    NpcDecay { npc_object_id: i32 },
    /// `RespawnTaskManager` → `Spawn.respawnNpc`: re-run the spawn line the
    /// dead NPC came from (indices into `GameData.spawn_data`).
    NpcRespawn { spawn_idx: usize, group_idx: usize, npc_idx: usize },
    /// A party/friend invite went unanswered (Java `PartyRequest.
    /// scheduleTimeout` / `_requestExpireTime`): clear the player's
    /// `PendingRequest` if `seq` still matches.
    RequestTimeout { object_id: i32, seq: u64 },
    /// The 12 s `PartyMemberPosition` broadcast (Java's per-party
    /// `_positionBroadcastTask`); reschedules itself while the party lives
    /// and `seq` matches.
    PartyPositionBroadcast { party_id: u32, seq: u64 },
    /// The 15 s loot-rule-change window elapsed without unanimous approval
    /// (`Party.PARTY_DISTRIBUTION_TYPE_REQUEST_TIMEOUT`).
    PartyLootChangeTimeout { party_id: u32, seq: u64 },
    /// A `Quest.startQuestTimer` firing → `quest.notifyEvent(name, …)` →
    /// `on_timer`. `seq` is checked against the player's `QuestTimerSeqs`
    /// entry for `(quest, name)` — cancelling a timer is bumping that seq
    /// (the cast_seq pattern). `npc` is 0 when the timer has no NPC.
    QuestTimer { quest: &'static str, name: String, player: i32, npc: i32, seq: u64 },
    /// `Door.AutoClose`: a script-opened door's `closeTime` elapsed. Stale
    /// (superseded by a newer open/close → `auto_close_seq` mismatch) = no-op.
    DoorAutoClose { door_object_id: i32, seq: u64 },
    /// `Door.TimerOpen`: a BY_TIME door's cycle toggle; reschedules itself.
    DoorTimerToggle { door_object_id: i32 },
}

struct Entry {
    fire_at: u64,
    seq: u64,
    task: ScheduledTask,
}

// Min-heap on `fire_at`, FIFO within the same tick via `seq`. `BinaryHeap` is a
// max-heap, so all comparisons are reversed.
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.fire_at == other.fire_at && self.seq == other.seq
    }
}
impl Eq for Entry {}
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        other.fire_at.cmp(&self.fire_at).then_with(|| other.seq.cmp(&self.seq))
    }
}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Default)]
pub struct Scheduler {
    heap: BinaryHeap<Entry>,
    next_seq: u64,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Java: `ThreadPool.schedule(task, delayMs)`. `fire_at` is an absolute tick.
    pub fn schedule(&mut self, fire_at: u64, task: ScheduledTask) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.heap.push(Entry { fire_at, seq, task });
    }

    /// Pop every task due at or before `now`, in (fire_at, insertion) order.
    /// The caller runs each against `&mut World`.
    pub fn drain_due(&mut self, now: u64) -> Vec<ScheduledTask> {
        let mut due = Vec::new();
        while let Some(entry) = self.heap.peek() {
            if entry.fire_at > now {
                break;
            }
            due.push(self.heap.pop().unwrap().task);
        }
        due
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_in_tick_then_insertion_order() {
        let mut s = Scheduler::new();
        s.schedule(5, ScheduledTask::Noop { object_id: 2 });
        s.schedule(3, ScheduledTask::Noop { object_id: 1 });
        s.schedule(5, ScheduledTask::Noop { object_id: 3 });

        assert!(s.drain_due(2).is_empty());
        assert_eq!(s.drain_due(3), vec![ScheduledTask::Noop { object_id: 1 }]);
        assert_eq!(
            s.drain_due(10),
            vec![ScheduledTask::Noop { object_id: 2 }, ScheduledTask::Noop { object_id: 3 }]
        );
        assert!(s.is_empty());
    }
}
