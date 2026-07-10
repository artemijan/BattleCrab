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

use tracing::{info, warn};

use crate::world::World;

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
/// final tick (drain + save) before exiting.
pub fn spawn(shutdown: Shutdown) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("game-thread".to_string())
        .spawn(move || run(World::new(), shutdown))
        .expect("failed to spawn game thread")
}

fn run(mut world: World, shutdown: Shutdown) {
    info!("GameLoop: started ({} ms tick).", TICK.as_millis());

    while !shutdown.is_requested() {
        let tick_start = Instant::now();

        // 1. Client packets — added in G1/G4.
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
