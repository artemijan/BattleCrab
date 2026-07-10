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
    /// `SkillCaster.launchSkill`, fires `hit_time` ms after
    /// `RequestMagicSkillUse` started the cast. Sends `MagicSkillLaunched`
    /// then runs `finishSkill` (MP/HP consume + apply effects) inline — G6's
    /// self-only cast pipeline has no separate travel/cancel-time phase to
    /// wait out between launch and landing (see the G6 plan's scope notes).
    SkillLaunch { player_object_id: i32, skill_id: i32, skill_level: i32 },
    /// `BuffFinishTask`: an active buff's `abnormalTime` has elapsed.
    BuffExpire { player_object_id: i32, skill_id: i32 },
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
