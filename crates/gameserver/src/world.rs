//! `org.l2jmobius.gameserver.model.World` — the single owner of all mutable
//! game state. Exactly one thread (the game thread) ever touches it, so it holds
//! no locks (CONCURRENCY_MODEL §2, challenge #2).
//!
//! G0 is a placeholder: it carries the tick counter and the scheduler so the
//! loop is real. Object registries, the region grid, and managers land in the
//! world/enter-world milestones (G3–G5).

use crate::scheduler::{ScheduledTask, Scheduler};

pub struct World {
    /// Monotonic tick counter (10 ticks/s). This *is* `GameTimeTaskManager` —
    /// no dedicated game-time thread (CONCURRENCY_MODEL §2.4).
    pub tick: u64,
    pub scheduler: Scheduler,
}

impl World {
    pub fn new() -> Self {
        Self { tick: 0, scheduler: Scheduler::new() }
    }

    /// Run every task the scheduler says is due this tick. Dead-id tasks are
    /// no-ops (handled per-variant as real tasks are added).
    pub fn run_due_tasks(&mut self) {
        for task in self.scheduler.drain_due(self.tick) {
            match task {
                ScheduledTask::Noop { .. } => {}
            }
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
