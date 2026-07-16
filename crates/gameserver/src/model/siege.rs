//! Castle sieges — Java `model/siege/Siege`, the registration/state slice. Each
//! castle has a `Siege` holding the clans registered as attackers / defenders /
//! pending defenders and an in-progress flag. Loaded from `siege_clans` at boot.
//!
//! Scope: what the `//castlemanage` siege actions touch — registration
//! (add/remove siege clans) and the start/stop state transition. The actual
//! siege combat — control towers, siege flags, siege guards, the siege zone/
//! PvP, teleport-to-siege, the scheduled 2h window and ownership-on-victory —
//! is a later milestone (TODO(G24) at the call sites).

/// Java `Siege`'s `byte` type constants (the `siege_clans.type` column).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiegeClanType {
    Owner,
    Defender,
    Attacker,
    /// Java `DEFENDER_NOT_APPROVED` — a defender awaiting the owner's approval.
    DefenderPending,
}

impl SiegeClanType {
    pub fn as_db(self) -> i32 {
        match self {
            Self::Owner => -1,
            Self::Defender => 0,
            Self::Attacker => 1,
            Self::DefenderPending => 2,
        }
    }

    pub fn from_db(v: i32) -> Option<Self> {
        match v {
            -1 => Some(Self::Owner),
            0 => Some(Self::Defender),
            1 => Some(Self::Attacker),
            2 => Some(Self::DefenderPending),
            _ => None,
        }
    }
}

/// One `siege_clans` row.
#[derive(Debug, Clone, Copy)]
pub struct SiegeClan {
    pub clan_id: i32,
    pub kind: SiegeClanType,
}

/// A castle's siege (Java `Castle.getSiege()`).
#[derive(Debug, Clone)]
pub struct Siege {
    pub castle_id: i32,
    pub clans: Vec<SiegeClan>,
    /// Java `isInProgress()` — runtime only, not persisted in `siege_clans`.
    pub in_progress: bool,
}

impl Siege {
    pub fn new(castle_id: i32) -> Self {
        Self { castle_id, clans: Vec::new(), in_progress: false }
    }

    /// Any clan registered as an ATTACKER (`getAttackerClans().isEmpty()`).
    pub fn has_attackers(&self) -> bool {
        self.clans.iter().any(|c| c.kind == SiegeClanType::Attacker)
    }

    /// Whether `clan_id` is registered for this siege in any role
    /// (`SiegeManager.checkIsRegistered`, narrowed to this castle).
    pub fn is_registered(&self, clan_id: i32) -> bool {
        self.clans.iter().any(|c| c.clan_id == clan_id)
    }

    pub fn add_clan(&mut self, clan_id: i32, kind: SiegeClanType) {
        self.clans.push(SiegeClan { clan_id, kind });
    }

    /// Remove a clan from the siege; returns whether anything was removed.
    pub fn remove_clan(&mut self, clan_id: i32) -> bool {
        let before = self.clans.len();
        self.clans.retain(|c| c.clan_id != clan_id);
        self.clans.len() != before
    }

    /// A human-readable roster for the (unported) registration window.
    pub fn summary(&self) -> String {
        let count = |k: SiegeClanType| self.clans.iter().filter(|c| c.kind == k).count();
        format!(
            "attackers: {}, defenders: {}, pending defenders: {}",
            count(SiegeClanType::Attacker),
            count(SiegeClanType::Defender),
            count(SiegeClanType::DefenderPending),
        )
    }
}
