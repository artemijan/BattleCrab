//! Castles — Java `model/residences/Castle` + `CastleManager`, the display/
//! ownership slice. On this Interlude dist there are nine (Gludio…Schuttgart),
//! loaded from the `castle` table at boot. Ownership lives on the owning clan
//! (`clan_data.hasCastle`), resolved against `World.clans`.
//!
//! Scope: the `//castlemanage` admin surface — the castle roster, owner/side
//! display, the ownership actions (set/take owner, switch side) — plus the
//! **treasury** (Java `Castle._treasury` / `castle.treasury`) and the tax
//! percent its side implies. Residential skills landed with G24
//! (`clans::grant_residential_skills_to_clan`); castle **functions** (the
//! chamberlain's door/trap upgrade tiers) and crests are still deferred, marked
//! at their sites. The treasury's arithmetic and
//! persistence live in [`crate::game_loop::siege::treasury`].

/// Java `enums/CastleSide`.
///
/// `Serialize`/`Deserialize` because an item `<cond isOnSide="LIGHT">` parses
/// into one ([`crate::data::item_cond::Cond::IsOnSide`]) and the item
/// catalogue is bincode-snapshotted — which is also why `model/castle.rs` is
/// on `snapshot::LAYOUT_SOURCES`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum CastleSide {
    #[default]
    Neutral,
    Light,
    Dark,
}

impl CastleSide {
    /// Parse the `castle.side` column / the `setOwner` argument (case-insensitive).
    pub fn from_string(s: &str) -> Option<Self> {
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
    /// Java `Castle._showNpcCrest` (`castle.showNpcCrest`): whether tax-zone
    /// NPCs fly the owner clan's crest. Nothing in the Java tree ever sets it
    /// true — an operator flips the DB column (or `ShowCrestWithoutQuest` in
    /// `NPC.ini`) to turn the display on.
    pub show_npc_crest: bool,
    /// Java `Castle._ticketBuyCount` (`castle.ticketBuyCount`): how many
    /// mercenary tickets the owner has placed; reset to 0 when the castle
    /// changes hands at a siege end. (The mercenary-placement system itself is
    /// a later milestone, so nothing increments it yet.)
    pub ticket_buy_count: i32,
    /// Java `Castle._isFirstMidVictory` — set when an attacker first engraves
    /// the castle mid-siege (`Siege.midVictory`), cleared at `endSiege`. It is
    /// the *only* thing that lets two attacker clans fight each other, and it
    /// makes every side a "siege friend" while still false.
    ///
    /// Runtime-only (Java never persists it). **Nothing sets it true yet**: the
    /// engrave skill (`Siege.midVictory`) is unported, so the port behaves as a
    /// siege where no one has engraved — which is Java's own behaviour up to
    /// that moment, not a divergence.
    pub first_mid_victory: bool,
    /// Java `Castle._isTimeRegistrationOver` (`castle.regTimeOver`): while this
    /// is `false` the owner may pick the siege hour (`RequestSetCastleSiegeTime`);
    /// it defaults `true`, so the feature is dormant until an operator opens the
    /// window. Picking an hour closes it again.
    pub time_registration_over: bool,
    /// Java `Castle._siegeTimeRegistrationEndDate` (`castle.regTimeEnd`): the
    /// deadline for the owner to pick the siege hour, stamped `now + 1 day` when
    /// the previous siege ended. Only consulted while `time_registration_over`
    /// is false — that flag is the gate, this is the countdown behind it.
    pub siege_time_registration_end: i64,
    /// Java `Castle._siegeDate` (`castle.siegeDate`): the owner-chosen siege
    /// time (epoch-millis), 0 when none has been set — then the fixed
    /// `SiegeSchedule.xml` slot is used.
    pub siege_date: i64,
    /// Java `Castle._treasury` (`castle.treasury`): the castle vault, in adena.
    /// Fed by the tax on purchases made inside the castle's tax zone, by manor
    /// seed sales and by the owner's chamberlain deposits; drained by
    /// chamberlain withdrawals and the manor's period costs. Only ever moved
    /// through [`crate::game_loop::siege::treasury::add_to_treasury_no_tax`], which
    /// persists every change like Java's per-call `UPDATE castle SET treasury`.
    pub treasury: i64,
}

/// Java `Castle.FUNC_*` — the five rentable castle functions.
pub const FUNC_TELEPORT: i32 = 1;
pub const FUNC_RESTORE_HP: i32 = 2;
pub const FUNC_RESTORE_MP: i32 = 3;
pub const FUNC_RESTORE_EXP: i32 = 4;
pub const FUNC_SUPPORT: i32 = 5;

/// One active castle function (Java `Castle.CastleFunction`): the rented
/// level, its per-period fee, the period, and the absolute next-charge stamp.
#[derive(Debug, Clone, Copy)]
pub struct CastleFunc {
    pub level: i32,
    pub lease: i64,
    /// The rental period in milliseconds (`_rate`).
    pub rate_ms: i64,
    /// Absolute unix-millis of the next renewal charge (`_endDate`).
    pub end_time: i64,
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
