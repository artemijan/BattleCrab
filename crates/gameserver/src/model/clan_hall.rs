//! Clan halls — Java `model/residences/ClanHall`, the static definition plus the
//! runtime ownership. Loaded from `data/residences/clanHalls/**` (48 halls) and
//! overlaid with the `clanhall` table (id → ownerId, paidUntil) at boot.
//!
//! Scope so far: the residence definition (grade, auction terms, agent NPCs,
//! doors, owner-restart/banish points) and *who owns it*. The auction bidding,
//! the lease/eviction cycle, the function upgrades and the Clan Hall Manager
//! dialog are later slices.

/// Java `ClanHallGrade` (the `_gradeValue` is the client sort weight).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClanHallGrade {
    None,
    D,
    C,
    B,
    A,
    S,
}

impl ClanHallGrade {
    pub fn from_name(name: &str) -> Self {
        match name {
            "GRADE_D" => Self::D,
            "GRADE_C" => Self::C,
            "GRADE_B" => Self::B,
            "GRADE_A" => Self::A,
            "GRADE_S" => Self::S,
            _ => Self::None,
        }
    }
}

/// Java `ClanHallType` (the `_clientVal` wire value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClanHallType {
    Auctionable,
    Siegeable,
    Other,
}

impl ClanHallType {
    pub fn from_name(name: &str) -> Self {
        match name {
            "SIEGEABLE" => Self::Siegeable,
            "OTHER" => Self::Other,
            _ => Self::Auctionable,
        }
    }
}

/// A clan hall — its static definition plus the runtime owner (0 = unowned).
#[derive(Debug, Clone)]
pub struct ClanHall {
    pub id: i32,
    pub name: String,
    pub grade: ClanHallGrade,
    pub hall_type: ClanHallType,
    /// `<auction minBid lease deposit/>` (adena).
    pub min_bid: i64,
    pub lease: i64,
    pub deposit: i64,
    /// The agent NPCs (auctioneer / manager) that belong to this hall.
    pub npcs: Vec<i32>,
    /// The hall's doors (opened/closed with ownership).
    pub doors: Vec<i32>,
    /// `<ownerRestartPoint>` — where the owning clan respawns.
    pub owner_restart: (i32, i32, i32),
    /// `<banishPoint>` — where non-members are ejected to.
    pub banish: (i32, i32, i32),

    // Runtime ownership (from the `clanhall` table).
    /// The owning clan id, or 0 when the hall is free.
    pub owner_id: i32,
    /// Java `paidUntil` — the epoch-millis the current lease is paid through.
    pub paid_until: i64,
}
