//! Event-engine runtime state — the World-side counterpart of the
//! `Event`-derived event scripts (`Event extends Quest`). Only Team vs Team
//! exists this milestone (G28); its static Java fields (`PLAYER_LIST`,
//! `PLAYER_SCORES`, `BLUE_TEAM`, `EVENT_ACTIVE`, …) live here as [`TvtState`],
//! driven by `game_loop/events/`.

use std::collections::HashMap;

/// The TvT lifecycle phase. Java tracks this implicitly through `EVENT_ACTIVE`
/// plus which quest timers are armed; the port makes it explicit so the
/// registration-close handler and (later) the fight handlers can guard on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TvtPhase {
    /// No event running.
    #[default]
    Inactive,
    /// The manager NPC is spawned and the registration window is open.
    Registration,
    /// Teams are in the arena behind closed doors; the fight hasn't started
    /// (Java's `WAIT_TIME` countdown between teleport and `StartFight`).
    Warmup,
    /// The doors are open and the fight is on (`StartFight` → `EndFight`).
    Fighting,
    /// The fight is over: winner resolved and rewarded, players frozen, waiting
    /// on the scoreboard + teleport-out timers (`EndFight` → `TeleportOut`).
    Ending,
}

/// TvT runtime — port of the static fields in `custom/events/TeamVsTeam/TvT.java`.
#[derive(Debug, Default)]
pub struct TvtState {
    pub phase: TvtPhase,
    /// Registered participants, in join order (Java `PLAYER_LIST`, a set; the
    /// Vec de-dups via `contains`). Object ids.
    pub player_list: Vec<i32>,
    /// Per-player kill score (Java `PLAYER_SCORES`). Object id → kills.
    pub scores: HashMap<i32, i32>,
    /// Team rosters, filled at teleport-to-arena (Java `BLUE_TEAM`/`RED_TEAM`).
    pub blue_team: Vec<i32>,
    pub red_team: Vec<i32>,
    /// Team kill totals (Java `BLUE_SCORE`/`RED_SCORE`).
    pub blue_score: i32,
    pub red_score: i32,
    /// The spawned event-manager NPC (Giran), despawned when registration
    /// closes (Java `MANAGER_NPC_INSTANCE`).
    pub manager_oid: Option<i32>,
    /// The PvP instance world id, once the arena stands up (Java `PVP_WORLD`;
    /// slice 2).
    pub world_id: Option<i32>,
}

impl TvtState {
    pub fn is_active(&self) -> bool {
        self.phase != TvtPhase::Inactive
    }

    /// Java's `PLAYER_LIST.clear()` + friends at `eventStart`/end.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// The event lifecycle manager. Java has no single "EventManager" class — each
/// `Event` script drives itself off its schedule — so this is the port's thin
/// adaptor: it names the running event (for the re-entry guard and the admin
/// panel) and owns its runtime state. `game_loop/events::{start,stop}` dispatch
/// by name.
#[derive(Debug, Default)]
pub struct EventManager {
    /// Name of the running event, `None` when idle (mirrors Java `EVENT_ACTIVE`
    /// but carries identity).
    pub active: Option<&'static str>,
    pub tvt: TvtState,
}
