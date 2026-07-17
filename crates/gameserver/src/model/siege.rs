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

/// A battlefield NPC spawn point — a siege guard (`castle_siege_guards`) or a
/// control/flame tower (`SiegeManager` config).
#[derive(Debug, Clone, Copy)]
pub struct SiegeSpawn {
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

/// A castle's siege (Java `Castle.getSiege()`).
#[derive(Debug, Clone)]
pub struct Siege {
    pub castle_id: i32,
    pub clans: Vec<SiegeClan>,
    /// Java `isInProgress()` — runtime only, not persisted in `siege_clans`.
    pub in_progress: bool,
    /// Java `_firstOwnerClanId` — the castle owner captured at siege start, so
    /// `endSiege` can tell "defender held" from "attacker captured" (0 = NPC).
    pub first_owner_clan_id: i32,
    /// Object ids of the NPCs (siege guards / towers) spawned for this siege,
    /// despawned at `endSiege`.
    pub spawned_npcs: Vec<i32>,
    /// Java `_controlTowerCount` — live control towers. Set on spawn, decremented
    /// as attackers destroy them. Faithful bookkeeping with no gameplay outcome
    /// in Interlude Classic: its only use is picking the *rejection message*
    /// (`ConditionPlayerCanResurrect`) when someone casts a normal resurrection
    /// skill on a corpse during a siege — which is blocked regardless of the
    /// count (the Battlefield Resurrection scroll, a separate condition, always
    /// works). No effect until the resurrection subsystem lands (TODO(G24)); it
    /// does **not** gate the restart-point respawn.
    pub control_tower_count: i32,
    /// Java attacker `SiegeClan.getFlag()` — HQ flags planted during the siege
    /// (owning clan id + flag npc oid). A flag is an attacker's respawn point
    /// (`getTeleToLocation(SIEGEFLAG)`) until a defender destroys it.
    pub flags: Vec<(i32, i32)>,
}

impl Siege {
    pub fn new(castle_id: i32) -> Self {
        Self {
            castle_id,
            clans: Vec::new(),
            in_progress: false,
            first_owner_clan_id: 0,
            spawned_npcs: Vec::new(),
            control_tower_count: 0,
            flags: Vec::new(),
        }
    }

    /// How many HQ flags a clan has planted (`SiegeClan.getNumFlags`).
    pub fn flag_count(&self, clan_id: i32) -> i32 {
        self.flags.iter().filter(|&&(c, _)| c == clan_id).count() as i32
    }

    /// A clan's HQ flag oid, if it has one (`getFlag(clan)` — the respawn point).
    pub fn flag_of(&self, clan_id: i32) -> Option<i32> {
        self.flags.iter().find(|&&(c, _)| c == clan_id).map(|&(_, oid)| oid)
    }

    pub fn add_flag(&mut self, clan_id: i32, npc_oid: i32) {
        self.flags.push((clan_id, npc_oid));
    }

    /// Drop a destroyed flag (`Siege.killedFlag`); returns whether one was removed.
    pub fn remove_flag(&mut self, npc_oid: i32) -> bool {
        let before = self.flags.len();
        self.flags.retain(|&(_, oid)| oid != npc_oid);
        self.flags.len() != before
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
