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
//!   threads, plus the plain row structs the queries return.
//! - `boot` — the schema check and the unprompted boot loads (`ClansLoaded`
//!   must be sent last; the game loop releases the login link on it).
//! - `commands` — the thread's main loop: receive a [`DbCommand`], run it,
//!   emit any [`DbEvent`].
//! - `queries` — the `load_*` readers and the handful of writers they sit
//!   beside; the only place that speaks sea-orm.

use std::thread::JoinHandle;

use models::entity::{
    account_gsdata, account_premium, bbs_favorites, bot_reported_char_data, buffer_schemes, castle,
    castle_manor_procure, castle_manor_production, castle_siege_guards, character_friends,
    character_hennas, character_macroses, character_offline_trade, character_offline_trade_items,
    character_quests, character_recipebook, character_reco_bonus, character_shortcuts,
    character_skills, character_skills_save, character_subclasses, character_summon_skills_save,
    character_summons, character_variables, characters, clan_data, clan_privs, clan_skills,
    clan_subpledges, clan_wars, clanhall, clanhall_auctions_bidders, crests, cursed_weapons,
    custom_mail, global_variables, grandboss_data, heroes, heroes_diary, item_auction,
    item_auction_bid, item_variations, items, lottery, mdt_bets, mdt_history, messages,
    npc_respawns, olympiad_data, olympiad_nobles, olympiad_nobles_eom, petition_feedback, pets,
    pledge_applicant, pledge_recruit, pledge_waiting_list, punishments, residence_functions,
    siege_clans,
};
use models::sea_orm::ActiveValue::{NotSet, Set, Unchanged};
use models::sea_orm::Condition;
use models::sea_orm::sea_query::{CaseStatement, Expr, OnConflict};
use models::sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use tracing::{error, info, warn};

use crate::character::{CharData, ItemRow};
use commons::util::now_millis;

mod boot;
mod commands;
mod queries;
mod types;

pub(crate) use boot::*;
pub(crate) use commands::*;
pub use queries::*;
pub use types::*;

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
            rt.block_on(run(url, max_connections, max_characters, cmd_rx, event_tx));
        })
        .expect("failed to spawn db thread")
}
