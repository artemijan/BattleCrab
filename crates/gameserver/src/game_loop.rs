//! The game thread and its 100 ms tick loop (CONCURRENCY_MODEL §2.2).
//!
//! Runs on one dedicated OS thread that owns [`World`]. The base tick is 100 ms,
//! matching Java's `GameTimeTaskManager` and high-priority task-manager rate.
//! G0 wires the loop shell: drain timers, run (no) systems, sleep to cadence,
//! and warn on tick overrun. Packet drain, service results, and tick systems
//! slot into the numbered steps as later milestones add them.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tracing::{debug, info, warn};

use crate::network::{NetEvent, NetEventRx};
use crate::world::{ClientHandle, World};

/// Base tick period. Slower Java rates (1 s, 5 s…) become `world.tick % N == 0`
/// systems on top of this.
pub const TICK: Duration = Duration::from_millis(100);

/// A tick that runs longer than this is the failure mode of the single-thread
/// design, so it must be visible from day one (CONCURRENCY_MODEL §2.6 rule 4).
const TICK_OVERRUN_WARN: Duration = Duration::from_millis(50);

/// Signal shared with the async side (ctrl-c / scheduled restart) to stop the
/// loop after the current tick finishes.
#[derive(Clone, Default)]
pub struct Shutdown(Arc<AtomicBool>);

impl Shutdown {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_requested(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Spawn the game thread. Returns its join handle so `main` can wait for the
/// final tick (drain + save) before exiting. `net_rx` delivers connection
/// lifecycle + inbound packets from the network runtime.
pub fn spawn(shutdown: Shutdown, net_rx: NetEventRx) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("game-thread".to_string())
        .spawn(move || run(World::new(), shutdown, net_rx))
        .expect("failed to spawn game thread")
}

fn run(mut world: World, shutdown: Shutdown, net_rx: NetEventRx) {
    info!("GameLoop: started ({} ms tick).", TICK.as_millis());

    while !shutdown.is_requested() {
        let tick_start = Instant::now();

        // 1. Network events: connects, disconnects, and inbound packets.
        drain_network(&mut world, &net_rx);
        // 2. Service results (DB / path / login-link) — added in G2+.

        // 3. One-shot timers due this tick.
        world.run_due_tasks();

        // 4. Fixed-rate tick systems (movement, AI, attack…) — added in G4+.
        // 5. Flush outbound packets / DB commands — added in G1+/G3+.

        let elapsed = tick_start.elapsed();
        if elapsed > TICK_OVERRUN_WARN {
            warn!("GameLoop: tick {} ran {} ms (budget {} ms).", world.tick, elapsed.as_millis(), TICK.as_millis());
        }
        if let Some(remaining) = TICK.checked_sub(elapsed) {
            std::thread::sleep(remaining);
        }

        world.tick += 1;
    }

    info!("GameLoop: stopped after {} ticks.", world.tick);
    // Final drain + save-all lands with the DB thread (G3).
}

/// Bounded, non-blocking drain of the network→game channel (step 1 of the tick).
fn drain_network(world: &mut World, net_rx: &NetEventRx) {
    while let Ok(event) = net_rx.try_recv() {
        match event {
            NetEvent::Connected { client_id, out, addr } => {
                world.clients.insert(client_id, ClientHandle { out, addr });
                debug!("GameLoop: client {client_id} connected from {addr} ({} online).", world.clients.len());
            }
            NetEvent::Received { client_id, data } => {
                // Opcode dispatch against &mut World lands in G2 (AuthLogin) on.
                debug!("GameLoop: client {client_id} sent {} bytes (opcode 0x{:02x}), unhandled in G1.", data.len(), data.first().copied().unwrap_or(0));
            }
            NetEvent::Disconnected { client_id } => {
                world.clients.remove(&client_id);
                debug!("GameLoop: client {client_id} disconnected ({} online).", world.clients.len());
            }
        }
    }
}
