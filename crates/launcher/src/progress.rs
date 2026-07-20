//! Progress reporting from the install worker thread to the UI thread.
//!
//! The worker owns a [`Reporter`]; the UI drains the channel every frame. Messages
//! are cheap and lossy-tolerant — the UI only ever renders the most recent state.

use std::io::Read;
use std::sync::mpsc::{Receiver, Sender};

/// What the worker is currently doing. The UI maps this straight onto its progress bars.
#[derive(Debug, Clone)]
pub enum Phase {
    /// Fetching the remote manifest to decide what needs downloading.
    CheckingManifest,
    /// Streaming the archive to disk. `total` is `None` when the server sends no
    /// `Content-Length` (progress is then indeterminate).
    Downloading { done: u64, total: Option<u64> },
    /// Unpacking. Measured in *compressed* bytes consumed, so `total` is the archive
    /// size — the uncompressed size is not known up front.
    Extracting { done: u64, total: u64 },
    /// Install finished; the client is playable.
    Ready,
    Failed(String),
}

impl Phase {
    /// Fraction complete in `0.0..=1.0`, or `None` when indeterminate.
    pub fn fraction(&self) -> Option<f32> {
        match self {
            Phase::Downloading { done, total: Some(total) } if *total > 0 => {
                Some(*done as f32 / *total as f32)
            }
            Phase::Extracting { done, total } if *total > 0 => {
                Some(*done as f32 / *total as f32)
            }
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

/// Wraps a reader and reports how many bytes have passed through it.
///
/// Used to drive the extraction bar: we count bytes pulled *out of the compressed
/// archive*, because the decompressed total is unknown until we finish.
pub struct ProgressReader<R> {
    inner: R,
    read: u64,
    total: u64,
    reporter: Reporter,
    /// Only report every ~4 MB — reporting per `read()` call floods the channel and
    /// costs more than the copy itself.
    last_reported: u64,
}

const REPORT_INTERVAL: u64 = 4 * 1024 * 1024;

impl<R: Read> ProgressReader<R> {
    pub fn new(inner: R, total: u64, reporter: Reporter) -> Self {
        Self { inner, read: 0, total, reporter, last_reported: 0 }
    }
}

impl<R: Read> Read for ProgressReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read += n as u64;
        if self.read - self.last_reported >= REPORT_INTERVAL || n == 0 {
            self.last_reported = self.read;
            self.reporter
                .send(Phase::Extracting { done: self.read, total: self.total });
        }
        Ok(n)
    }
}
