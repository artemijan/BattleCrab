//! `DbEvent` — the DB→game half of the thread protocol. Drained by the game
//! loop each tick, so a read's continuation runs the moment the row arrives.

use super::super::CharData;
use super::super::ItemRow;
use super::super::NpcRespawnRow;
use super::super::OfflineTraderRow;
use super::CreateResult;
use super::rows::{
    BirthdayMatch, ClanHallBidRow, ClanHallRow, CursedWeaponRow, CustomMailRow, GroundItemRow,
    HeroRow, ManorProcureRow, ManorProductionRow, OlympiadEomRow, OlympiadNobleRow,
    ResidenceFunctionRow, SiegeClanRow,
};

/// DB thread → game thread (drained in tick step 2).
pub enum DbEvent {
    /// `GlobalVariablesManager.restoreMe()` — the whole table, at boot.
    GlobalVariablesLoaded { entries: Vec<(String, String)> },
    /// `ItemsOnGroundManager.load()` — every row of `itemsonground`, sent only
    /// when `SaveDroppedItem` is on (Java returns early otherwise).
    GroundItemsLoaded { items: Vec<GroundItemRow> },
    /// `send_list` = push a fresh `CharSelectionInfo` to the client (login,
    /// delete, restore). After character creation it is false — Java only caches
    /// the list (`setCharSelection`) and does not re-send it.
    CharactersLoaded {
        client_id: u32,
        account: String,
        chars: Vec<CharData>,
        send_list: bool,
    },
    CharacterCreated {
        client_id: u32,
        result: CreateResult,
    },
    CharCount {
        account: String,
        count: u8,
        del_times: Vec<i64>,
    },
    /// `ExIsCharNameCreatable` result: -1 = creatable, else a failure code.
    NameCreatable { client_id: u32, result: i32 },
    /// A reserved object-id block `[start, end)` for the game thread's
    /// runtime allocations (loot items). One is pushed unprompted at boot.
    IdBlock { start: i64, end: i64 },
    /// The full clan table (`ClanTable` boot load), pushed unprompted at
    /// boot like the first `IdBlock`.
    ClansLoaded {
        clans: Vec<crate::model::clan::Clan>,
        wars: Vec<crate::model::clan::ClanWar>,
        crests: Vec<crate::model::clan::Crest>,
        recruit_clans: Vec<crate::model::clan_entry::PledgeRecruitInfo>,
        recruit_waiting: Vec<crate::model::clan_entry::PledgeWaitingInfo>,
        recruit_applicants: Vec<crate::model::clan_entry::PledgeApplicantInfo>,
        /// `clan_notices` rows as `clan_id → (enabled, notice)`.
        notices: Vec<(i32, bool, String)>,
    },
    /// The whole `npc_respawns` table (Java `DBSpawnManager.load`), pushed
    /// unprompted at boot. See [`NpcRespawnRow`].
    NpcRespawnsLoaded { rows: Vec<NpcRespawnRow> },
    /// `OfflineTraderTable.restoreOfflineTraders`' two queries, already joined:
    /// every stored shop with its full character and its item lines. Pushed
    /// unprompted at boot (before `ClansLoaded`), like the other restores; the
    /// game thread applies the config gates and `OfflineMaxDays`.
    OfflineTradersLoaded { traders: Vec<OfflineTraderRow> },
    /// The whole `account_premium` table (Java `PremiumManager` cache),
    /// pushed unprompted at boot. `(account_name lowercase, enddate millis)`.
    PremiumLoaded { entries: Vec<(String, i64)> },
    /// The most recent `lottery` row (Java `Lottery.SELECT_LAST_LOTTERY`) for the
    /// lifecycle, plus every finished round's draw result (round id →
    /// [`DrawnRound`](crate::model::lottery::DrawnRound)) for offline prize
    /// claim. Pushed unprompted at boot; `row` is `None` on a first-ever boot.
    LotteryLoaded {
        row: Option<crate::model::lottery::LotteryRow>,
        draws: Vec<(i32, crate::model::lottery::DrawnRound)>,
    },
    /// The persisted (offline) sold tickets of `round` for a draw — the reply to
    /// [`super::command::DbCommand::LoadCustomMail`] — the pending rows, in table order.
    CustomMailLoaded { rows: Vec<CustomMailRow> },
    /// The reply to [`super::command::DbCommand::LoadBirthdays`]: one entry per character
    /// whose creation day matches a day that was asked for. The name comes
    /// from the same row rather than from a second lookup — Java reads it back
    /// out of `CharInfoTable`, which is that table's own index.
    BirthdaysLoaded { rows: Vec<BirthdayMatch> },
    /// [`super::command::DbCommand::LoadLotteryTickets`]. `(object_id, enchant, custom_type2)`
    /// per ticket item 4442; the draw dedupes these against online inventories.
    LotteryTicketsLoaded {
        round: i32,
        rows: Vec<(i32, i32, i32)>,
    },
    /// The Monster Race history + current lane bets (Java `MonsterRace
    /// .loadHistory`/`loadBets`), pushed unprompted at boot.
    MdtLoaded {
        history: Vec<crate::model::monster_race::HistoryInfo>,
        bets: Vec<(i32, i64)>,
    },
    /// The persisted item auctions + their bids + the next auction id (Java
    /// `ItemAuctionManager` boot load, G30.5), pushed unprompted at boot.
    ItemAuctionsLoaded {
        next_auction_id: i32,
        auctions: Vec<crate::model::item_auction::ItemAuction>,
    },
    /// Every mail message + its attachments, and the offline character
    /// name -> id table mail needs to address them (Java `MailManager.load`
    /// + `CharInfoTable`, G30). Pushed unprompted at boot.
    MailLoaded {
        messages: Vec<crate::model::mail::Message>,
        attachments: Vec<(i32, Vec<ItemRow>)>,
        char_ids_by_name: Vec<(String, i32)>,
        /// Every character's ignore list (Java `BlockList`). Rides this event
        /// because it is wanted at the same moment and for the same reason as
        /// the name table: mail must be filtered against an addressee who need
        /// not be online.
        block_lists: Vec<(i32, std::collections::HashSet<i32>)>,
    },
    /// The active punishments (Java `PunishmentManager.load`, G31), pushed
    /// unprompted at boot. Already-expired rows are filtered out here; `next_id`
    /// seeds the game-thread id allocator past the highest loaded id.
    PunishmentsLoaded {
        next_id: i32,
        punishments: Vec<crate::model::punishment::Punishment>,
    },
    /// The whole `bot_reported_char_data` table (Java
    /// `BotReportTable.loadReportedCharData`), pushed unprompted at boot as
    /// `(bot_id, reporter_id, report_date)`.
    BotReportsLoaded { rows: Vec<(i32, i32, i64)> },
    /// The whole `buffer_schemes` table (Java `SchemeBufferTable.load`), pushed
    /// unprompted at boot. `(object_id, scheme_name, skill_ids)`; skills not in
    /// the available-buff table are filtered on the game thread.
    BufferSchemesLoaded {
        entries: Vec<(i32, String, Vec<i32>)>,
    },
    /// The whole `bbs_favorites` table (Java `FavoriteBoard` loads per-player on
    /// demand; this port caches all rows at boot like `buffer_schemes`), pushed
    /// unprompted at boot. `(player_id, fav_id, title, bypass, add_date)`,
    /// newest first.
    FavoritesLoaded {
        entries: Vec<(i32, i32, String, String, String)>,
    },
    /// The `grandboss_data` table (Java `GrandBossManager.init`), pushed
    /// unprompted at boot. Filtered to known NPC templates on the game thread.
    GrandBossesLoaded {
        bosses: Vec<crate::model::grand_boss::GrandBoss>,
    },
    /// The `cursed_weapons` state table (Java `CursedWeaponsManager.restore`),
    /// pushed unprompted at boot; overlaid onto the XML config on the game thread.
    CursedWeaponsLoaded { rows: Vec<CursedWeaponRow> },
    /// The `castle` table (Java `CastleManager.load`), pushed unprompted at boot.
    CastlesLoaded {
        castles: Vec<crate::model::castle::Castle>,
    },
    /// The `siege_clans` table (Java `Siege.loadSiegeClan`), pushed unprompted at
    /// boot after `CastlesLoaded`. Grouped into per-castle sieges on the game thread.
    SiegesLoaded { rows: Vec<SiegeClanRow> },
    /// The `clanhall` table (Java `ClanHall` ownership load) — id → owner/paidUntil.
    /// Overlaid onto the static hall defs on the game thread.
    ClanHallsLoaded { rows: Vec<ClanHallRow> },
    /// The `clanhall_auctions_bidders` table (Java `ClanHallAuction.loadBidder`) —
    /// the live bids per hall, restored at boot.
    ClanHallBiddersLoaded { rows: Vec<ClanHallBidRow> },
    /// The `residence_functions` table — active hall function upgrades, restored
    /// at boot.
    ResidenceFunctionsLoaded { rows: Vec<ResidenceFunctionRow> },
    /// `olympiad_data` (the single id=0 row) + all `olympiad_nobles`
    /// (Java `Olympiad.load`), loaded once at boot.
    OlympiadLoaded {
        current_cycle: i32,
        period: i32,
        olympiad_end: i64,
        validation_end: i64,
        next_weekly_change: i64,
        nobles: Vec<OlympiadNobleRow>,
        /// The last completed cycle's snapshot (`olympiad_nobles_eom`) — what
        /// the Olympiad Manager's class leaderboard shows.
        eom: Vec<OlympiadEomRow>,
    },
    /// The current heroes (`heroes` rows with `played = 1`) + every hero-diary
    /// entry (`heroes_diary`, `(charId, time, action, param)`), loaded at boot.
    HeroesLoaded {
        heroes: Vec<HeroRow>,
        diary: Vec<(i32, i64, i8, i32)>,
    },
    /// The `castle_siege_guards` table (the stationed garrison, `isHired=0`),
    /// pushed unprompted at boot. `(castle_id, spawn)`; grouped by castle on the
    /// game thread.
    SiegeGuardsLoaded {
        guards: Vec<(i32, crate::model::siege::SiegeSpawn)>,
    },
    /// The same table's `isHired = 1` rows — the mercenaries the owning clans
    /// posted between sieges. Pushed unprompted at boot beside the garrison.
    MercenariesLoaded {
        guards: Vec<(i32, crate::model::siege::SiegeSpawn)>,
    },
    /// The `buylists` table — the remaining stock of every limited-stock
    /// product that has been sold since its last restock. `BuyListData.load`
    /// reads it right after parsing the XML; pushed unprompted at boot here.
    /// `(list_id, item_id, count, next_restock_time)`.
    BuyListStockLoaded { rows: Vec<(i32, i32, i64, i64)> },
    /// The `castle_manor_production` + `castle_manor_procure` tables (Java
    /// `CastleManorManager.loadDb`), pushed unprompted at boot. Filtered to
    /// known seeds/crops and grouped by castle/period on the game thread.
    ManorLoaded {
        production: Vec<ManorProductionRow>,
        procure: Vec<ManorProcureRow>,
    },
}
