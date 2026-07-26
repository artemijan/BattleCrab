//! Castles — Java `model/residences/Castle` + `CastleManager`, the display/
//! ownership slice. On this Interlude dist there are nine (Gludio…Schuttgart),
//! loaded from the `castle` table at boot. Ownership lives on the owning clan
//! (`clan_data.hasCastle`), resolved against `World.clans`.
//!
//! Scope: the `//castlemanage` admin surface — the castle roster, owner/side
//! display, and the ownership actions (set/take owner, switch side). The siege
//! engine, taxes, functions, residential skills and crests are later milestones
//! (TODO(G24) at their sites).

/// Java `enums/CastleSide`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CastleSide {
    #[default]
    Neutral,
    Light,
    Dark,
}

impl CastleSide {
    /// Parse the `castle.side` column / the `setOwner` argument (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "NEUTRAL" => Some(Self::Neutral),
            "LIGHT" => Some(Self::Light),
            "DARK" => Some(Self::Dark),
            _ => None,
        }
    }

    /// The stored/wire form (`castle.side`).
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Neutral => "NEUTRAL",
            Self::Light => "LIGHT",
            Self::Dark => "DARK",
        }
    }

    /// Java `CommonUtil.capitalizeFirst(side.toString().toLowerCase())` — the
    /// `%castleSide%` display value.
    pub fn display(self) -> &'static str {
        match self {
            Self::Neutral => "Neutral",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

/// One `castle` row, narrowed to the display/ownership slice.
#[derive(Debug, Clone)]
pub struct Castle {
    /// Java `getResidenceId()` — 1..=9 on this dist.
    pub id: i32,
    pub name: String,
    pub side: CastleSide,
    /// Java `Castle._ticketBuyCount` (`castle.ticketBuyCount`): how many
    /// mercenary tickets the owner has placed; reset to 0 when the castle
    /// changes hands at a siege end. (The mercenary-placement system itself is
    /// a later milestone, so nothing increments it yet.)
    pub ticket_buy_count: i32,
}
