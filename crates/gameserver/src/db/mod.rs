//! The DB thread (CONCURRENCY_MODEL §2.4). A dedicated OS thread owns the SQLite
//! pool; the game thread never blocks on the database — it sends [`DbCommand`]s
//! and drains [`DbEvent`]s each tick. Character id allocation lives here too
//! (a minimal `IdManager`).
//!
//! [`spawn`] is the entry point and lives here; the rest is split by role and
//! re-exported, so callers keep saying `db::DbCommand` / `db::load_characters`
//! and never name a submodule:
//!
//! - `types` — the [`DbCommand`] / [`DbEvent`] protocol between the two
//!   threads, plus the payloads each half carries and the plain row structs
//!   the queries return.
//! - `character` — the character-load bundle ([`CharData`] and the rows it
//!   carries), restored for the selection screen and enter-world.
//! - `boot` — the schema check and the unprompted boot loads (`ClansLoaded`
//!   must be sent last; the game loop releases the login link on it).
//! - `commands` — the thread's main loop: receive a [`DbCommand`], run it,
//!   emit any [`DbEvent`].
//! - `queries` — the `load_*` readers and the handful of writers they sit
//!   beside; the only place that speaks sea-orm, split by the same domains
//!   as `commands`.

use std::thread::JoinHandle;

mod boot;
mod character;
mod commands;
mod queries;
mod types;

pub use boot::GroundItemBootConfig;
pub(crate) use boot::{clean_up_database, send_boot_events, verify_schema};
pub use character::{CharData, FriendInfo, ItemRow};
pub(crate) use commands::run;
pub(crate) use queries::account::{load_buffer_schemes, load_favorites, load_premium};
pub(crate) use queries::character_load::{load_char_ids_by_name, reload};
pub(crate) use queries::character_store::{
    count_characters, create_character, delete_char, name_exists, store_player,
};
pub(crate) use queries::clans::{
    load_clan_notices, load_clan_wars, load_clans, load_crests, load_recruit_applicants,
    load_recruit_clans, load_recruit_waiting,
};
pub use queries::commerce::OfflineTraderRow;
pub(crate) use queries::commerce::{load_buy_list_stock, load_item_auctions, load_offline_traders};
pub(crate) use queries::minigames::{
    load_lottery, load_lottery_draws, load_mdt_bets, load_mdt_history,
};
pub(crate) use queries::olympiad::{load_hero_diary, load_heroes, load_olympiad};
pub(crate) use queries::residences::{
    load_castles, load_clan_hall_bidders, load_clan_hall_owners, load_cursed_weapons,
    load_hired_siege_guards, load_manor_procure, load_manor_production, load_residence_functions,
    load_siege_clans, load_siege_guards,
};
pub(crate) use queries::social::{
    BLOCK_RELATION, load_all_block_lists, load_bot_reports, load_mail, load_punishments,
};
pub use queries::world::NpcRespawnRow;
pub(crate) use queries::world::{
    clear_ground_items, load_global_variables, load_grandboss_data, load_ground_items,
    load_next_id, load_npc_respawns, store_ground_items,
};
pub(crate) use queries::{item_row_model, warn_err};
pub use types::command::DbCommand;
pub use types::event::DbEvent;
pub use types::rows::{
    BirthdayDay, BirthdayMatch, ClanHallBidRow, ClanHallRow, CursedWeaponRow, CustomMailRow,
    FreightItemRow, GroundItemRow, HeroRow, MailRow, ManorProcureRow, ManorProductionRow,
    OlympiadEomRow, OlympiadNobleRow, PetRow, ResidenceFunctionRow, SiegeClanRow, SkillBuffRow,
    SkillReuseRow, SummonRow,
};
pub use types::save::{NewCharacter, NewItem, NewShortcut, PlayerSaveData, PlayerSnapshot};
pub use types::{CmdTx, CreateResult};

/// First object id handed out by `IdManager` (Java `FIRST_OID`). Shared by
/// every world-object type (characters, items, …) — Java's `IdManager` is a
/// single pool, not one per type.
pub(crate) const FIRST_OID: i64 = 0x10000000;

/// How many object ids each `IdBlock` reservation hands the game thread.
pub const ID_BLOCK_SIZE: i64 = 5000;

pub type CmdRx = tokio::sync::mpsc::UnboundedReceiver<DbCommand>;

/// Sender facade for the DB thread's share of the unified service→game
/// channel ([`crate::events::GameEvent`]); a send wakes the sleeping game
/// loop, which is what lets a mid-handler read's continuation run the moment
/// the row arrives instead of at the next tick boundary.
#[derive(Clone)]
pub struct EventTx(pub crate::events::GameEventTx);

impl EventTx {
    /// An `Err` means the game thread is gone — callers treat it as shutdown.
    pub fn send(&self, event: DbEvent) -> Result<(), std::sync::mpsc::SendError<()>> {
        self.0
            .send(crate::events::GameEvent::Db(event))
            .map_err(|_| std::sync::mpsc::SendError(()))
    }
}

/// Spawn the DB thread. It creates and owns the pool on its own runtime.
pub fn spawn(
    url: String,
    max_connections: u32,
    max_characters: i32,
    clean_up: bool,
    ground_items: GroundItemBootConfig,
    cmd_rx: CmdRx,
    event_tx: EventTx,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("db-thread".to_string())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("db thread runtime");
            rt.block_on(run(
                url,
                max_connections,
                max_characters,
                clean_up,
                ground_items,
                cmd_rx,
                event_tx,
            ));
        })
        .expect("failed to spawn db thread")
}
