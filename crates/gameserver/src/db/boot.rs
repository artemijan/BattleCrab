use super::DbEvent;
use super::EventTx;
use super::clear_ground_items;
use super::load_all_block_lists;
use super::load_bot_reports;
use super::load_buffer_schemes;
use super::load_buy_list_stock;
use super::load_castles;
use super::load_char_ids_by_name;
use super::load_clan_hall_bidders;
use super::load_clan_hall_owners;
use super::load_clan_notices;
use super::load_clan_wars;
use super::load_clans;
use super::load_crests;
use super::load_cursed_weapons;
use super::load_favorites;
use super::load_global_variables;
use super::load_grandboss_data;
use super::load_ground_items;
use super::load_hero_diary;
use super::load_heroes;
use super::load_hired_siege_guards;
use super::load_item_auctions;
use super::load_lottery;
use super::load_lottery_draws;
use super::load_mail;
use super::load_manor_procure;
use super::load_manor_production;
use super::load_mdt_bets;
use super::load_mdt_history;
use super::load_npc_respawns;
use super::load_offline_traders;
use super::load_olympiad;
use super::load_premium;
use super::load_punishments;
use super::load_recruit_applicants;
use super::load_recruit_clans;
use super::load_recruit_waiting;
use super::load_residence_functions;
use super::load_siege_clans;
use super::load_siege_guards;
use models::sea_orm::ConnectionTrait;
use models::sea_orm::DatabaseConnection;
use tracing::info;
use tracing::warn;
/// The three `General.ini` keys `ItemsOnGroundManager`'s constructor reads.
/// Passed to the DB thread rather than read there, because the thread has no
/// `Config`.
#[derive(Debug, Clone, Copy)]
pub struct GroundItemBootConfig {
    pub save_dropped_item: bool,
    pub clear_dropped_item_table: bool,
    pub empty_dropped_item_table_after_load: bool,
}

/// Confirms the pool actually points at a game database.
///
/// `characters` and `accounts` are the two tables the server cannot run without
/// and that no other database on the box would have together, which makes them
/// a cheap and unambiguous fingerprint.
pub(crate) async fn verify_schema(db: &DatabaseConnection) -> Result<(), String> {
    let mut missing = Vec::new();
    for table in ["characters", "accounts"] {
        let found = db
            .query_one_raw(models::sea_orm::Statement::from_sql_and_values(
                models::sea_orm::DatabaseBackend::Sqlite,
                "SELECT name FROM sqlite_master WHERE type='table' AND name = ?",
                [table.into()],
            ))
            .await
            .map_err(|e| format!("cannot inspect database schema: {e}"))?;
        if found.is_none() {
            missing.push(table);
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    Err(format!(
        "database is missing required table(s): {}",
        missing.join(", ")
    ))
}

/// The unprompted boot loads: every table the world needs before it comes up,
/// pushed to the game thread as `DbEvent`s. The game loop cannot ask for these —
/// it does not know the DB thread is up yet — so they are sent on our own
/// initiative, in dependency order.
///
/// The tail order is load-bearing: `ClansLoaded` must be sent **last**, because
/// the game loop releases the login link when it arrives.
pub(crate) async fn send_boot_events(
    db: &DatabaseConnection,
    cfg: &GroundItemBootConfig,
    event_tx: &EventTx,
) {
    // `GlobalVariablesManager.restoreMe()` — small, and read by boot code that
    // runs before the world is up, so it goes first.
    let _ = event_tx.send(DbEvent::GlobalVariablesLoaded {
        entries: load_global_variables(db).await,
    });

    // `ItemsOnGroundManager` construction, in Java's own order: the
    // clear-on-disabled case first, then the load, then the empty-after-load.
    if !cfg.save_dropped_item {
        // "may want to delete all items previously stored to avoid add old
        // items on reactivate" — only when the operator asked for it.
        if cfg.clear_dropped_item_table {
            clear_ground_items(db).await;
        }
    } else {
        let items = load_ground_items(db).await;
        info!("ItemsOnGroundManager: loaded {} items.", items.len());
        let _ = event_tx.send(DbEvent::GroundItemsLoaded { items });
        if cfg.empty_dropped_item_table_after_load {
            clear_ground_items(db).await;
        }
    }

    // Premium table cache, before clans so `ClansLoaded` stays the last boot
    // event (the game loop releases the login link on it).
    let _ = event_tx.send(DbEvent::PremiumLoaded {
        entries: load_premium(db).await,
    });

    // `SchemeBufferTable.load` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::BufferSchemesLoaded {
        entries: load_buffer_schemes(db).await,
    });

    // Last lottery round (Java `Lottery.startLottery`'s restore) + the drawn
    // rounds cache — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::LotteryLoaded {
        row: load_lottery(db).await,
        draws: load_lottery_draws(db).await,
    });

    // Monster Race history + lane bets (Java `MonsterRace` constructor) —
    // likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::MdtLoaded {
        history: load_mdt_history(db).await,
        bets: load_mdt_bets(db).await,
    });

    // Item auctions + bids (Java `ItemAuctionManager` boot load, G30.5) —
    // likewise unprompted, before `ClansLoaded`.
    let (next_auction_id, auctions) = load_item_auctions(db).await;
    let _ = event_tx.send(DbEvent::ItemAuctionsLoaded {
        next_auction_id,
        auctions,
    });

    // Mail + attachments + the offline name->id table (Java `MailManager.load`
    // and `CharInfoTable`, G30) — likewise unprompted, before `ClansLoaded`.
    let (messages, attachments) = load_mail(db).await;
    let _ = event_tx.send(DbEvent::MailLoaded {
        messages,
        attachments,
        char_ids_by_name: load_char_ids_by_name(db).await,
        block_lists: load_all_block_lists(db).await,
    });

    // Active punishments (Java `PunishmentManager.load`, G31) — likewise
    // unprompted, before `ClansLoaded`.
    let (next_punishment_id, punishments) = load_punishments(db).await;
    let _ = event_tx.send(DbEvent::PunishmentsLoaded {
        next_id: next_punishment_id,
        punishments,
    });

    // Java `BotReportTable`'s constructor load — likewise unprompted.
    let _ = event_tx.send(DbEvent::BotReportsLoaded {
        rows: load_bot_reports(db).await,
    });

    // `FavoriteBoard` favorites cache — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::FavoritesLoaded {
        entries: load_favorites(db).await,
    });

    // `DBSpawnManager.load` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::NpcRespawnsLoaded {
        rows: load_npc_respawns(db).await,
    });

    // `OfflineTraderTable.restoreOfflineTraders` — the stored shops. The rows
    // are always read (the config lives on the game thread), which also means a
    // server that turned the feature off still gets to clear them.
    let _ = event_tx.send(DbEvent::OfflineTradersLoaded {
        traders: load_offline_traders(db).await,
    });

    // `GrandBossManager.init` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::GrandBossesLoaded {
        bosses: load_grandboss_data(db).await,
    });

    // `CursedWeaponsManager.restore` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::CursedWeaponsLoaded {
        rows: load_cursed_weapons(db).await,
    });

    // `CastleManager.load` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::CastlesLoaded {
        castles: load_castles(db).await,
    });

    // `Siege.loadSiegeClan` — after castles (the game loop keys sieges off them).
    let _ = event_tx.send(DbEvent::SiegesLoaded {
        rows: load_siege_clans(db).await,
    });

    // `CastleManorManager.loadDb` — the manor production/procure state.
    let _ = event_tx.send(DbEvent::ManorLoaded {
        production: load_manor_production(db).await,
        procure: load_manor_procure(db).await,
    });

    // Clan-hall ownership — overlaid onto the static hall defs at boot.
    let _ = event_tx.send(DbEvent::ClanHallsLoaded {
        rows: load_clan_hall_owners(db).await,
    });

    // Clan-hall auction bids — restored so escrowed adena stays accounted for.
    let _ = event_tx.send(DbEvent::ClanHallBiddersLoaded {
        rows: load_clan_hall_bidders(db).await,
    });

    // Active clan-hall function upgrades.
    let _ = event_tx.send(DbEvent::ResidenceFunctionsLoaded {
        rows: load_residence_functions(db).await,
    });

    // `Olympiad.load` — the period/cycle row + every noble's record.
    let _ = event_tx.send(load_olympiad(db).await);

    // `Hero.init` — the currently-crowned heroes (`played = 1`) + their diaries.
    let _ = event_tx.send(DbEvent::HeroesLoaded {
        heroes: load_heroes(db).await,
        diary: load_hero_diary(db).await,
    });

    // `BuyListData.load`'s second half: the merchant stock that survived the
    // last shutdown, and how long each has left before it refills.
    let _ = event_tx.send(DbEvent::BuyListStockLoaded {
        rows: load_buy_list_stock(db).await,
    });

    // `SiegeGuardManager` — the stationed siege guards, spawned at siege start,
    // and the mercenaries the owning clans hired between sieges.
    let _ = event_tx.send(DbEvent::SiegeGuardsLoaded {
        guards: load_siege_guards(db).await,
    });
    let _ = event_tx.send(DbEvent::MercenariesLoaded {
        guards: load_hired_siege_guards(db).await,
    });

    // `ClanTable`'s boot restore, likewise unprompted.
    let _ = event_tx.send(DbEvent::ClansLoaded {
        clans: load_clans(db).await,
        wars: load_clan_wars(db).await,
        crests: load_crests(db).await,
        recruit_clans: load_recruit_clans(db).await,
        recruit_waiting: load_recruit_waiting(db).await,
        recruit_applicants: load_recruit_applicants(db).await,
        notices: load_clan_notices(db).await,
    });
}

/// Java `IdManager`'s `Config.DATABASE_CLEAN_UP` block: delete every child row
/// whose owner is gone. Runs once at boot, before the used-id walk.
///
/// Java spells out 50 statements; they are four shapes over 43 tables, so they
/// are a table here instead. Each entry is
/// `(table, column, parent_table, parent_column)` and becomes
/// `DELETE FROM t WHERE t.c NOT IN (SELECT pc FROM pt)`. **Every one of Java's
/// 43 target tables exists in this schema**, so nothing is skipped; the four
/// statements that do not fit the shape are spelled out below it.
///
/// Deleting nothing is the expected outcome on a healthy database. It is worth
/// running anyway because the rows it removes are invisible: an orphaned
/// `items` row still consumes its object id, which is exactly what the walk
/// after this reads.
pub(crate) async fn clean_up_database(db: &DatabaseConnection) {
    use models::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    /// `(table, column, parent table, parent column)`.
    const ORPHANS: &[(&str, &str, &str, &str)] = &[
        (
            "account_gsdata",
            "account_name",
            "characters",
            "account_name",
        ),
        ("character_contacts", "charId", "characters", "charId"),
        ("character_contacts", "contactId", "characters", "charId"),
        ("character_friends", "charId", "characters", "charId"),
        ("character_friends", "friendId", "characters", "charId"),
        ("character_hennas", "charId", "characters", "charId"),
        ("character_macroses", "charId", "characters", "charId"),
        ("character_quests", "charId", "characters", "charId"),
        ("character_recipebook", "charId", "characters", "charId"),
        ("character_recipeshoplist", "charId", "characters", "charId"),
        ("character_shortcuts", "charId", "characters", "charId"),
        ("character_skills", "charId", "characters", "charId"),
        ("character_skills_save", "charId", "characters", "charId"),
        ("character_subclasses", "charId", "characters", "charId"),
        ("character_instance_time", "charId", "characters", "charId"),
        ("item_auction_bid", "playerObjId", "characters", "charId"),
        ("item_variations", "itemId", "items", "object_id"),
        ("item_elementals", "itemId", "items", "object_id"),
        ("item_variables", "id", "items", "object_id"),
        ("cursed_weapons", "charId", "characters", "charId"),
        ("heroes", "charId", "characters", "charId"),
        ("olympiad_nobles", "charId", "characters", "charId"),
        ("olympiad_nobles_eom", "charId", "characters", "charId"),
        ("pets", "item_obj_id", "items", "object_id"),
        ("merchant_lease", "player_id", "characters", "charId"),
        ("character_reco_bonus", "charId", "characters", "charId"),
        ("clan_data", "leader_id", "characters", "charId"),
        ("clan_data", "clan_id", "characters", "clanid"),
        ("olympiad_fights", "charOneId", "characters", "charId"),
        ("olympiad_fights", "charTwoId", "characters", "charId"),
        ("heroes_diary", "charId", "characters", "charId"),
        ("character_offline_trade", "charId", "characters", "charId"),
        (
            "character_offline_trade_items",
            "charId",
            "characters",
            "charId",
        ),
        ("character_tpbookmark", "charId", "characters", "charId"),
        ("character_variables", "charId", "characters", "charId"),
        ("bot_reported_char_data", "botId", "characters", "charId"),
        ("clan_privs", "clan_id", "clan_data", "clan_id"),
        ("clan_skills", "clan_id", "clan_data", "clan_id"),
        ("clan_subpledges", "clan_id", "clan_data", "clan_id"),
        ("clan_wars", "clan1", "clan_data", "clan_id"),
        ("clan_wars", "clan2", "clan_data", "clan_id"),
        ("siege_clans", "clan_id", "clan_data", "clan_id"),
        ("clan_notices", "clan_id", "clan_data", "clan_id"),
        ("auction_bid", "bidderId", "clan_data", "clan_id"),
        ("posts", "post_forum_id", "forums", "forum_id"),
        ("topic", "topic_forum_id", "forums", "forum_id"),
    ];

    /// The four Java statements that are not a plain single-parent orphan test.
    const IRREGULAR: &[&str] = &[
        // An item belongs to a character *or* a clan warehouse; `-1` is the
        // mail/auction holding owner and is exempt.
        "DELETE FROM items WHERE items.owner_id NOT IN (SELECT charId FROM characters) \
         AND items.owner_id NOT IN (SELECT clan_id FROM clan_data) AND items.owner_id != -1",
        // …and the `-1` ones are orphaned only when their mail is gone.
        "DELETE FROM items WHERE items.owner_id = -1 AND loc LIKE 'MAIL' \
         AND loc_data NOT IN (SELECT messageId FROM messages WHERE senderId = -1)",
        // A forum's owner is a clan or a character depending on `forum_parent`.
        "DELETE FROM forums WHERE forums.forum_owner_id NOT IN (SELECT clan_id FROM clan_data) \
         AND forums.forum_parent=2",
        "DELETE FROM forums WHERE forums.forum_owner_id NOT IN (SELECT charId FROM characters) \
         AND forums.forum_parent=3",
    ];

    let started = std::time::Instant::now();
    let mut cleaned = 0u64;
    let exec = async |sql: String| -> u64 {
        match db
            .execute_raw(Statement::from_string(DatabaseBackend::Sqlite, sql.clone()))
            .await
        {
            Ok(r) => r.rows_affected(),
            Err(e) => {
                // A missing table is not fatal: Java logs and carries on too,
                // and an operator on a trimmed schema should still boot.
                warn!("DB cleanup: {sql} failed: {e}");
                0
            }
        }
    };
    for (table, column, parent, parent_column) in ORPHANS {
        cleaned += exec(format!(
            "DELETE FROM {table} WHERE {table}.{column} NOT IN \
             (SELECT {parent_column} FROM {parent})"
        ))
        .await;
    }
    for sql in IRREGULAR {
        cleaned += exec((*sql).to_string()).await;
    }
    info!(
        "DB cleanup: removed {cleaned} orphaned row(s) in {} ms.",
        started.elapsed().as_millis()
    );
}
