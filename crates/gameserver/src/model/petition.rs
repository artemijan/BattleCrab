//! Petition runtime state (G31) — the World-side counterpart of Java's
//! `model/Petition` + `instancemanager/PetitionManager`. Petitions are entirely
//! **in-memory** (only the post-consultation feedback persists, to
//! `petition_feedback`), so this is pure runtime state that dies with a restart,
//! matching Java.
//!
//! The manager here holds only state and queries; the packet orchestration
//! (notify petitioner/responder/GMs, the HTML list) lives in
//! `game_loop::petition`, because this port's managers can't reach the client
//! sessions.

use std::collections::HashMap;

/// The petition category the client picks (Java `PetitionType`, 1-indexed on the
/// wire). Kept for the GM list display; behaviour doesn't branch on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetitionType {
    Immobility,
    RecoveryRelated,
    BugReport,
    QuestRelated,
    BadUser,
    Suggestions,
    GameTip,
    OperationRelated,
    Other,
}

impl PetitionType {
    /// Java `PetitionType.values()[type - 1]` — the wire value is 1-indexed.
    pub fn from_wire(type_id: i32) -> Option<Self> {
        Some(match type_id {
            1 => PetitionType::Immobility,
            2 => PetitionType::RecoveryRelated,
            3 => PetitionType::BugReport,
            4 => PetitionType::QuestRelated,
            5 => PetitionType::BadUser,
            6 => PetitionType::Suggestions,
            7 => PetitionType::GameTip,
            8 => PetitionType::OperationRelated,
            9 => PetitionType::Other,
            _ => return None,
        })
    }

    /// Java `Petition.getTypeAsString` (enum name with `_`→space).
    pub fn as_label(self) -> &'static str {
        match self {
            PetitionType::Immobility => "IMMOBILITY",
            PetitionType::RecoveryRelated => "RECOVERY RELATED",
            PetitionType::BugReport => "BUG REPORT",
            PetitionType::QuestRelated => "QUEST RELATED",
            PetitionType::BadUser => "BAD USER",
            PetitionType::Suggestions => "SUGGESTIONS",
            PetitionType::GameTip => "GAME TIP",
            PetitionType::OperationRelated => "OPERATION RELATED",
            PetitionType::Other => "OTHER",
        }
    }
}

/// A petition's lifecycle (Java `PetitionState`). The end-states differ only in
/// which notices go out; `Pending` and `InProcess` are the two live states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetitionState {
    Pending,
    ResponderCancel,
    ResponderMissing,
    ResponderReject,
    PetitionerCancel,
    Completed,
    InProcess,
}

/// One petition (Java `model.Petition`). Object ids and names are cached
/// (rather than live refs) so a notice can still address a petitioner/responder
/// who logged out mid-consultation.
pub struct Petition {
    pub id: i32,
    pub ptype: PetitionType,
    pub state: PetitionState,
    pub content: String,
    pub petitioner: i32,
    pub petitioner_name: String,
    pub responder: Option<i32>,
    pub responder_name: Option<String>,
    pub submit_time: i64,
    /// The consultation transcript — serialized `CreatureSay` packets, replayed
    /// to a petitioner who reconnects mid-consultation (Java
    /// `checkPetitionMessages`).
    pub log: Vec<Vec<u8>>,
}

impl Petition {
    /// Whether `object_id` is a live participant (petitioner or responder).
    pub fn involves(&self, object_id: i32) -> bool {
        self.petitioner == object_id || self.responder == Some(object_id)
    }
}

/// The petition registry (Java `PetitionManager`): pending + completed queues
/// and the id allocator. Pure state — the game loop drives the notices.
#[derive(Default)]
pub struct PetitionManager {
    pub pending: HashMap<i32, Petition>,
    pub completed: HashMap<i32, Petition>,
    next_id: i32,
}

impl PetitionManager {
    fn alloc_id(&mut self) -> i32 {
        // Java draws from the global IdManager; a private counter is enough here
        // since petitions never touch the object store or persist.
        self.next_id += 1;
        self.next_id
    }

    /// Java `submitPetition`: register a new pending petition, return its id.
    pub fn submit(
        &mut self,
        petitioner: i32,
        petitioner_name: String,
        content: String,
        ptype: PetitionType,
    ) -> i32 {
        let id = self.alloc_id();
        self.pending.insert(
            id,
            Petition {
                id,
                ptype,
                state: PetitionState::Pending,
                content,
                petitioner,
                petitioner_name,
                responder: None,
                responder_name: None,
                submit_time: commons::util::now_millis(),
                log: Vec::new(),
            },
        );
        id
    }

    /// Java `getPlayerTotalPetitionCount`: pending + completed petitions this
    /// player has filed (the per-day cap counts both).
    pub fn player_total_petition_count(&self, petitioner: i32) -> usize {
        self.pending
            .values()
            .chain(self.completed.values())
            .filter(|p| p.petitioner == petitioner)
            .count()
    }

    /// Java `isPlayerPetitionPending`: the player has a pending (not-yet-ended)
    /// petition.
    pub fn is_player_petition_pending(&self, petitioner: i32) -> bool {
        self.pending.values().any(|p| p.petitioner == petitioner)
    }

    /// The pending petition this player filed, if any (its id).
    pub fn pending_id_of(&self, petitioner: i32) -> Option<i32> {
        self.pending
            .values()
            .find(|p| p.petitioner == petitioner)
            .map(|p| p.id)
    }

    /// Java `isPlayerInConsultation`: the player is a participant in an
    /// IN_PROCESS petition.
    pub fn is_player_in_consultation(&self, object_id: i32) -> bool {
        self.pending
            .values()
            .any(|p| p.state == PetitionState::InProcess && p.involves(object_id))
    }

    /// The active (IN_PROCESS) petition this player participates in, if any.
    pub fn active_id_of(&self, object_id: i32) -> Option<i32> {
        self.pending
            .values()
            .find(|p| p.state == PetitionState::InProcess && p.involves(object_id))
            .map(|p| p.id)
    }

    /// Java `isPetitionInProcess()`: any petition currently under consultation.
    pub fn any_in_process(&self) -> bool {
        self.pending
            .values()
            .any(|p| p.state == PetitionState::InProcess)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Move a petition out of the pending queue (its consultation ended). The
    /// caller records it as completed.
    pub fn take_pending(&mut self, id: i32) -> Option<Petition> {
        self.pending.remove(&id)
    }
}
