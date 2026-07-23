//! Progress reporting from the install worker thread to the UI thread.
//!
//! The worker owns a [`Reporter`]; the UI drains the channel every frame. Messages
//! are cheap and lossy-tolerant — the UI only ever renders the most recent state.

use std::sync::mpsc::{Receiver, Sender};

/// What the worker is currently doing. The UI maps this straight onto its progress bars.
#[derive(Debug, Clone)]
pub enum Phase {
    /// Request sent, waiting on the response headers.
    Connecting,
    /// Streaming the archive to disk. `total` is `None` when the server sends no
    /// `Content-Length` (progress is then indeterminate).
    Downloading {
        done: u64,
        total: Option<u64>,
    },
    /// Unpacking, in *uncompressed* bytes — the 7z index carries the real total, so
    /// this bar reflects actual progress rather than bytes consumed from the archive.
    Extracting {
        done: u64,
        total: u64,
    },
    /// Install finished; the client is playable.
    Ready,
    Failed(String),
}

impl Phase {
    /// Fraction complete in `0.0..=1.0`, or `None` when indeterminate.
    pub fn fraction(&self) -> Option<f32> {
        match self {
            Phase::Downloading {
                done,
                total: Some(total),
            } if *total > 0 => Some(*done as f32 / *total as f32),
            Phase::Extracting { done, total } if *total > 0 => Some(*done as f32 / *total as f32),
            Phase::Ready => Some(1.0),
            _ => None,
        }
    }
}

/// Worker-side handle. Every send also wakes the UI thread, which is otherwise
/// asleep — egui only repaints on input unless asked.
#[derive(Clone)]
pub struct Reporter {
    tx: Sender<Phase>,
    ctx: Option<egui::Context>,
}

impl Reporter {
    pub fn new(tx: Sender<Phase>, ctx: Option<egui::Context>) -> Self {
        Self { tx, ctx }
    }

    pub fn send(&self, phase: Phase) {
        // A closed channel means the window is gone; the worker will notice when it
        // next checks for cancellation, so dropping the error here is fine.
        let _ = self.tx.send(phase);
        if let Some(ctx) = &self.ctx {
            ctx.request_repaint();
        }
    }
}

/// UI-side handle.
pub type ProgressRx = Receiver<Phase>;
