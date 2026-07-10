//! `org.l2jmobius.gameserver.model.World` — the single owner of all mutable
//! game state. Exactly one thread (the game thread) ever touches it, so it holds
//! no locks (CONCURRENCY_MODEL §2, challenge #2).
//!
//! G0/G1 are a placeholder: it carries the tick counter, the scheduler, and the
//! connected-client registry. Object registries, the region grid, and managers
//! land in the world/enter-world milestones (G3–G5).

use std::collections::HashMap;
use std::net::SocketAddr;

use crate::network::OutboundTx;
use crate::scheduler::{ScheduledTask, Scheduler};

/// The game thread's handle to a connected client: the outbound queue (send a
/// serialized packet body; the connection task encrypts + frames it) and its
/// address. The gameplay-facing binding (which `Player`/account) is added in G2.
pub struct ClientHandle {
    pub out: OutboundTx,
    pub addr: SocketAddr,
}

impl ClientHandle {
    /// Queue a serialized packet body for this client. Fails silently if the
    /// connection task is gone (it will be reaped via `Disconnected`).
    pub fn send(&self, body: Vec<u8>) {
        let _ = self.out.send(body);
    }
}

pub struct World {
    /// Monotonic tick counter (10 ticks/s). This *is* `GameTimeTaskManager` —
    /// no dedicated game-time thread (CONCURRENCY_MODEL §2.4).
    pub tick: u64,
    pub scheduler: Scheduler,
    /// Connected clients keyed by network id.
    pub clients: HashMap<u32, ClientHandle>,
}

impl World {
    pub fn new() -> Self {
        Self { tick: 0, scheduler: Scheduler::new(), clients: HashMap::new() }
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
