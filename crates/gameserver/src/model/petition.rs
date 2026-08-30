//! Petition runtime state (G31) — the World-side counterpart of Java's
//! `model/Petition` + `instancemanager/PetitionManager`. Petitions are entirely
//! **in-memory** (only the post-consultation feedback persists, to
//! `petition_feedback`), so this is pure runtime state that dies with a restart,
//! matching Java.
//!
//! The manager here holds only state and queries; the packet orchestration
//! (notify petitioner/responder/GMs, the HTML list) lives in
//! `game_loop::moderation::petition`, because this port's managers can't reach the client
//! sessions.

use std::collections::HashMap;

use enum_ordinalize::Ordinalize;

/// The petition category the client picks (Java `PetitionType`, 1-indexed on the
/// wire). Kept for the GM list display; behaviour doesn't branch on it.
///
/// `Ordinalize` derives the lookup from the declaration (see `enums::Race`);
/// the `- 1` that turns the wire value into the ordinal stays in
/// [`from_wire`](Self::from_wire), which is the only place the off-by-one
/// belongs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ordinalize)]
#[repr(i32)]
#[ordinalize(from_ordinal(const fn from_ordinal, doc = "`values()[ordinal]` — note the wire value is this **plus one**."))]
pub enum PetitionType {
    Immobility = 0,
    RecoveryRelated = 1,
    BugReport = 2,
    QuestRelated = 3,
    BadUser = 4,
    Suggestions = 5,
    GameTip = 6,
    OperationRelated = 7,
    Other = 8,
}

impl PetitionType {
    /// Java `PetitionType.values()[type - 1]` — the wire value is 1-indexed.
    pub fn from_wire(type_id: i32) -> Option<Self> {
        Self::from_ordinal(type_id.checked_sub(1)?)
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

    /// The pending petition this player participates in, as `(id, is_petitioner)`
    /// — `false` means they are the responding GM.
    pub fn participation_of(&self, object_id: i32) -> Option<(i32, bool)> {
        self.pending.values().find_map(|p| {
            if p.petitioner == object_id {
                Some((p.id, true))
            } else if p.responder == Some(object_id) {
                Some((p.id, false))
            } else {
                None
            }
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Java reads the category with `values()[type - 1]`, so the wire value is
    /// the ordinal *plus one*. Pinned here because getting the off-by-one wrong
    /// mislabels every petition in the GM list by exactly one category.
    #[test]
    fn petition_type_wire_values_are_one_indexed() {
        let expected = [
            (PetitionType::Immobility, 1),
            (PetitionType::RecoveryRelated, 2),
            (PetitionType::BugReport, 3),
            (PetitionType::QuestRelated, 4),
            (PetitionType::BadUser, 5),
            (PetitionType::Suggestions, 6),
            (PetitionType::GameTip, 7),
            (PetitionType::OperationRelated, 8),
            (PetitionType::Other, 9),
        ];
        for (kind, wire) in expected {
            assert_eq!(PetitionType::from_wire(wire), Some(kind), "{kind:?}");
        }
        // 0 is Java's `values()[-1]` — an ArrayIndexOutOfBounds there, `None`
        // here — and the subtraction must not wrap at `i32::MIN`.
        for wire in [i32::MIN, -1, 0, 10, i32::MAX] {
            assert_eq!(PetitionType::from_wire(wire), None, "{wire}");
        }
    }
}
