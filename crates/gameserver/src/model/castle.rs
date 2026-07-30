//! Castles — Java `model/residences/Castle` + `CastleManager`, the display/
//! ownership slice. On this Interlude dist there are nine (Gludio…Schuttgart),
//! loaded from the `castle` table at boot. Ownership lives on the owning clan
//! (`clan_data.hasCastle`), resolved against `World.clans`.
//!
//! Scope: the `//castlemanage` admin surface — the castle roster, owner/side
//! display, the ownership actions (set/take owner, switch side) — plus the
//! **treasury** (Java `Castle._treasury` / `castle.treasury`) and the tax
//! percent its side implies. The functions, residential skills and crests are
//! later milestones (TODO(G24) at their sites). The treasury's arithmetic and
//! persistence live in [`crate::game_loop::castle`].

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
    /// Java `Castle._isTimeRegistrationOver` (`castle.regTimeOver`): while this
    /// is `false` the owner may pick the siege hour (`RequestSetCastleSiegeTime`);
    /// it defaults `true`, so the feature is dormant until an operator opens the
    /// window. Picking an hour closes it again.
    pub time_registration_over: bool,
    /// Java `Castle._siegeDate` (`castle.siegeDate`): the owner-chosen siege
    /// time (epoch-millis), 0 when none has been set — then the fixed
    /// `SiegeSchedule.xml` slot is used.
    pub siege_date: i64,
    /// Java `Castle._treasury` (`castle.treasury`): the castle vault, in adena.
    /// Fed by the tax on purchases made inside the castle's tax zone, by manor
    /// seed sales and by the owner's chamberlain deposits; drained by
    /// chamberlain withdrawals and the manor's period costs. Only ever moved
    /// through [`crate::game_loop::castle::add_to_treasury_no_tax`], which
    /// persists every change like Java's per-call `UPDATE castle SET treasury`.
    pub treasury: i64,
}

/// Java `enums/TaxType`. `SELL` has no caller anywhere in this Java build — the
/// sell-side keys exist in `Feature.ini` and are read by nothing — so it is
/// carried for completeness, not because a path uses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaxType {
    Buy,
    #[allow(
        dead_code,
        reason = "no Java path reads the sell tax either; kept so the config keys have a home"
    )]
    Sell,
}
