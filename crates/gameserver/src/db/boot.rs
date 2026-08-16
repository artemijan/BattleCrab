use super::*;

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
pub(crate) async fn send_boot_events(db: &DatabaseConnection, event_tx: &EventTx) {
    // `GlobalVariablesManager.restoreMe()` — small, and read by boot code that
    // runs before the world is up, so it goes first.
    let _ = event_tx.send(DbEvent::GlobalVariablesLoaded {
        entries: load_global_variables(db).await,
    });

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
        block_lists: crate::db::queries::load_all_block_lists(db).await,
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
