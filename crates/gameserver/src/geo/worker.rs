//! The path-worker service (CONCURRENCY_MODEL §2.4): pathfinding runs off
//! the game thread on one dedicated worker that shares the read-only geodata
//! (`Arc<GeoEngine>`). Java calls `CellPathFinding.findPath` synchronously
//! inside `Creature.moveToLocation`; here the game thread sends a
//! [`PathRequest`] and picks the [`PathEvent`] up on a later tick — same
//! request → next-tick-result split as the DB thread (`db.rs`).

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;

use tracing::info;

use super::GeoEngine;
use super::path::{PathConfig, find_path};

/// One `findPath` call. `seq` pairs the reply with the newest request for an
/// object — the game thread bumps it per request and drops stale replies
/// (the player may have clicked elsewhere while the search ran).
#[derive(Debug)]
pub struct PathRequest {
    pub seq: u64,
    pub client_id: u32,
    pub object_id: i32,
    pub from: (i32, i32, i32),
    /// The original (pre-clamp) destination — Java passes
    /// `originalX/Y/Z` to `findPath`.
    pub to: (i32, i32, i32),
    /// Java `playable`: full postfilter for players, one pass for AI.
    pub playable: bool,
}

/// The reply: the request context echoed back plus the route (world
/// coordinates), `None` when no path exists.
#[derive(Debug)]
pub struct PathEvent {
    pub seq: u64,
    pub client_id: u32,
    pub object_id: i32,
    pub to: (i32, i32, i32),
    pub path: Option<Vec<(i32, i32, i32)>>,
}

pub type PathReqTx = Sender<PathRequest>;
pub type PathReqRx = Receiver<PathRequest>;
pub type PathEventTx = Sender<PathEvent>;
pub type PathEventRx = Receiver<PathEvent>;

/// Spawn the path-worker thread. It exits when every request sender is gone
/// (the game thread dropping `World` on shutdown closes the channel); replies
/// to an already-stopped game thread are silently dropped.
pub fn spawn(
    geo: Arc<GeoEngine>,
    cfg: PathConfig,
    req_rx: PathReqRx,
    event_tx: PathEventTx,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("path-worker".to_string())
        .spawn(move || {
            info!("PathWorker: started.");
            while let Ok(req) = req_rx.recv() {
                let PathRequest {
                    seq,
                    client_id,
                    object_id,
                    from,
                    to,
                    playable,
                } = req;
                let path = find_path(&geo, &cfg, from, to, playable);
                if event_tx
                    .send(PathEvent {
                        seq,
                        client_id,
                        object_id,
                        to,
                        path,
                    })
                    .is_err()
                {
                    break;
                }
            }
            info!("PathWorker: stopped.");
        })
        .expect("failed to spawn path worker")
}
