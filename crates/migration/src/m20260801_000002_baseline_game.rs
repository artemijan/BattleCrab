//! Baseline: the 96 game-server tables.
//!
//! Transcribed from the Java installer's SQLite DDL (`dist/db_installer`, since
//! removed). Column types are passed through verbatim (`MEDIUMINT`, `TINYINT`,
//! …) on SQLite, so the schema matches the one the Java installer produced; on
//! PostgreSQL `crate::dialect` maps each to the type its entity reads. Applied
//! databases depend on it: change the schema with a new migration, not here.
//!
//! Every statement is `IF NOT EXISTS`, which is what lets `l2r-migrate up`
//! adopt the live production database: it records the migration as applied
//! without touching a single existing table.

use sea_orm_migration::prelude::*;

use crate::dialect::{dflt, ty, ty_loose_f64};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Dropped in reverse order by `down`.
const TABLES: &[&str] = &[
    "account_gsdata",
    "account_premium",
    "airships",
    "announcements",
    "auction_bid",
    "bbs_favorites",
    "bot_reported_char_data",
    "buffer_schemes",
    "buylists",
    "castle",
    "castle_doorupgrade",
    "castle_functions",
    "castle_manor_procure",
    "castle_manor_production",
    "castle_siege_guards",
    "castle_trapupgrade",
    "character_contacts",
    "character_daily_rewards",
    "character_friends",
    "character_hennas",
    "character_instance_time",
    "character_item_reuse_save",
    "character_macroses",
    "character_mentees",
    "character_offline_trade",
    "character_offline_trade_items",
    "character_pet_skills_save",
    "character_premium_items",
    "character_quests",
    "character_recipebook",
    "character_recipeshoplist",
    "character_reco_bonus",
    "character_shortcuts",
    "character_skills",
    "character_skills_save",
    "character_subclasses",
    "character_summon_skills_save",
    "character_summons",
    "character_tpbookmark",
    "character_variables",
    "characters",
    "clan_data",
    "clan_notices",
    "clan_privs",
    "clan_skills",
    "clan_subpledges",
    "clan_variables",
    "clan_wars",
    "clanhall",
    "clanhall_auctions_bidders",
    "commission_items",
    "crests",
    "cursed_weapons",
    "custom_mail",
    "custom_teleport",
    "event_schedulers",
    "fort",
    "fort_doorupgrade",
    "fort_functions",
    "fort_siege_guards",
    "fort_spawnlist",
    "fortsiege_clans",
    "forums",
    "global_tasks",
    "global_variables",
    "grandboss_data",
    "heroes",
    "heroes_diary",
    "item_auction",
    "item_auction_bid",
    "item_elementals",
    "item_variables",
    "item_variations",
    "items",
    "itemsonground",
    "lottery",
    "mdt_bets",
    "mdt_history",
    "merchant_lease",
    "messages",
    "npc_respawns",
    "olympiad_data",
    "olympiad_fights",
    "olympiad_nobles",
    "olympiad_nobles_eom",
    "party_matching_history",
    "petition_feedback",
    "pets",
    "pledge_applicant",
    "pledge_recruit",
    "pledge_waiting_list",
    "posts",
    "punishments",
    "residence_functions",
    "siege_clans",
    "topic",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_account_gsdata(manager).await?;
        create_account_premium(manager).await?;
        create_airships(manager).await?;
        create_announcements(manager).await?;
        create_auction_bid(manager).await?;
        create_bbs_favorites(manager).await?;
        create_bot_reported_char_data(manager).await?;
        create_buffer_schemes(manager).await?;
        create_buylists(manager).await?;
        create_castle(manager).await?;
        create_castle_doorupgrade(manager).await?;
        create_castle_functions(manager).await?;
        create_castle_manor_procure(manager).await?;
        create_castle_manor_production(manager).await?;
        create_castle_siege_guards(manager).await?;
        create_castle_trapupgrade(manager).await?;
        create_character_contacts(manager).await?;
        create_character_daily_rewards(manager).await?;
        create_character_friends(manager).await?;
        create_character_hennas(manager).await?;
        create_character_instance_time(manager).await?;
        create_character_item_reuse_save(manager).await?;
        create_character_macroses(manager).await?;
        create_character_mentees(manager).await?;
        create_character_offline_trade(manager).await?;
        create_character_offline_trade_items(manager).await?;
        create_character_pet_skills_save(manager).await?;
        create_character_premium_items(manager).await?;
        create_character_quests(manager).await?;
        create_character_recipebook(manager).await?;
        create_character_recipeshoplist(manager).await?;
        create_character_reco_bonus(manager).await?;
        create_character_shortcuts(manager).await?;
        create_character_skills(manager).await?;
        create_character_skills_save(manager).await?;
        create_character_subclasses(manager).await?;
        create_character_summon_skills_save(manager).await?;
        create_character_summons(manager).await?;
        create_character_tpbookmark(manager).await?;
        create_character_variables(manager).await?;
        create_characters(manager).await?;
        create_clan_data(manager).await?;
        create_clan_notices(manager).await?;
        create_clan_privs(manager).await?;
        create_clan_skills(manager).await?;
        create_clan_subpledges(manager).await?;
        create_clan_variables(manager).await?;
        create_clan_wars(manager).await?;
        create_clanhall(manager).await?;
        create_clanhall_auctions_bidders(manager).await?;
        create_commission_items(manager).await?;
        create_crests(manager).await?;
        create_cursed_weapons(manager).await?;
        create_custom_mail(manager).await?;
        create_custom_teleport(manager).await?;
        create_event_schedulers(manager).await?;
        create_fort(manager).await?;
        create_fort_doorupgrade(manager).await?;
        create_fort_functions(manager).await?;
        create_fort_siege_guards(manager).await?;
        create_fort_spawnlist(manager).await?;
        create_fortsiege_clans(manager).await?;
        create_forums(manager).await?;
        create_global_tasks(manager).await?;
        create_global_variables(manager).await?;
        create_grandboss_data(manager).await?;
        create_heroes(manager).await?;
        create_heroes_diary(manager).await?;
        create_item_auction(manager).await?;
        create_item_auction_bid(manager).await?;
        create_item_elementals(manager).await?;
        create_item_variables(manager).await?;
        create_item_variations(manager).await?;
        create_items(manager).await?;
        create_itemsonground(manager).await?;
        create_lottery(manager).await?;
        create_mdt_bets(manager).await?;
        create_mdt_history(manager).await?;
        create_merchant_lease(manager).await?;
        create_messages(manager).await?;
        create_npc_respawns(manager).await?;
        create_olympiad_data(manager).await?;
        create_olympiad_fights(manager).await?;
        create_olympiad_nobles(manager).await?;
        create_olympiad_nobles_eom(manager).await?;
        create_party_matching_history(manager).await?;
        create_petition_feedback(manager).await?;
        create_pets(manager).await?;
        create_pledge_applicant(manager).await?;
        create_pledge_recruit(manager).await?;
        create_pledge_waiting_list(manager).await?;
        create_posts(manager).await?;
        create_punishments(manager).await?;
        create_residence_functions(manager).await?;
        create_siege_clans(manager).await?;
        create_topic(manager).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in TABLES.iter().rev() {
            manager
                .drop_table(
                    Table::drop()
                        .table(Alias::new(*table))
                        .if_exists()
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

/// `account_gsdata`
async fn create_account_gsdata(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("account_gsdata"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("account_name"))
                        .custom(ty(manager, "VARCHAR(45)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("var"))
                        .custom(ty(manager, "VARCHAR(255)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("value"))
                        .custom(ty(manager, "TEXT"))
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("account_name"))
                        .col(Alias::new("var")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `account_premium`
async fn create_account_premium(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("account_premium"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("account_name"))
                        .custom(ty(manager, "varchar(45)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("enddate"))
                        .custom(ty(manager, "decimal(20,0)"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(Index::create().col(Alias::new("account_name")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `airships`
async fn create_airships(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("airships"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("owner_id"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("fuel"))
                        .custom(ty(manager, "decimal(5,0)"))
                        .not_null()
                        .default(dflt(manager, "600")),
                )
                .primary_key(Index::create().col(Alias::new("owner_id")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `announcements`
async fn create_announcements(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("announcements"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .integer()
                        .primary_key()
                        .auto_increment(),
                )
                .col(
                    ColumnDef::new(Alias::new("type"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("initial"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("delay"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("repeat"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("author"))
                        .custom(ty(manager, "TEXT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("content"))
                        .custom(ty(manager, "TEXT"))
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `auction_bid`
async fn create_auction_bid(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("auction_bid"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("auctionId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("bidderId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("bidderName"))
                        .custom(ty(manager, "varchar(50)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("clan_name"))
                        .custom(ty(manager, "varchar(50)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("maxBid"))
                        .custom(ty(manager, "BIGINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("time_bid"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("auctionId"))
                        .col(Alias::new("bidderId")),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("id")
                .table(Alias::new("auction_bid"))
                .col(Alias::new("id"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `bbs_favorites`
async fn create_bbs_favorites(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("bbs_favorites"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("favId"))
                        .integer()
                        .primary_key()
                        .auto_increment(),
                )
                .col(
                    ColumnDef::new(Alias::new("playerId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("favTitle"))
                        .custom(ty(manager, "VARCHAR(50)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("favBypass"))
                        .custom(ty(manager, "VARCHAR(127)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("favAddDate"))
                        .custom(ty(manager, "TIMESTAMP"))
                        .not_null()
                        .default(dflt(manager, "CURRENT_TIMESTAMP")),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("favId_playerId")
                .table(Alias::new("bbs_favorites"))
                .unique()
                .col(Alias::new("favId"))
                .col(Alias::new("playerId"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `bot_reported_char_data`
async fn create_bot_reported_char_data(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("bot_reported_char_data"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("botId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("reporterId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("reportDate"))
                        .custom(ty(manager, "BIGINT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("botId"))
                        .col(Alias::new("reporterId")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `buffer_schemes`
async fn create_buffer_schemes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("buffer_schemes"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("object_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("scheme_name"))
                        .custom(ty(manager, "VARCHAR(16)"))
                        .not_null()
                        .default(dflt(manager, "'default'")),
                )
                .col(
                    ColumnDef::new(Alias::new("skills"))
                        .custom(ty(manager, "VARCHAR(200)"))
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("object_id"))
                        .col(Alias::new("scheme_name")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `buylists`
async fn create_buylists(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("buylists"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("buylist_id"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("item_id"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("count"))
                        .custom(ty(manager, "BIGINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("next_restock_time"))
                        .custom(ty(manager, "BIGINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("buylist_id"))
                        .col(Alias::new("item_id")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `castle`
async fn create_castle(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("castle"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("name"))
                        .custom(ty(manager, "varchar(25)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("side"))
                        .custom(ty(manager, "varchar(10)"))
                        .not_null()
                        .default(dflt(manager, "'NEUTRAL'")),
                )
                .col(
                    ColumnDef::new(Alias::new("treasury"))
                        .custom(ty(manager, "BIGINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("siegeDate"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("regTimeOver"))
                        .custom(ty(manager, "varchar(10)"))
                        .not_null()
                        .default(dflt(manager, "'true'")),
                )
                .col(
                    ColumnDef::new(Alias::new("regTimeEnd"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("showNpcCrest"))
                        .custom(ty(manager, "varchar(10)"))
                        .not_null()
                        .default(dflt(manager, "'false'")),
                )
                .col(
                    ColumnDef::new(Alias::new("ticketBuyCount"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(Index::create().col(Alias::new("id")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `castle_doorupgrade`
async fn create_castle_doorupgrade(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("castle_doorupgrade"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("doorId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("ratio"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("castleId"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(Index::create().col(Alias::new("doorId")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `castle_functions`
async fn create_castle_functions(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("castle_functions"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("castle_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("type"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("lvl"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("lease"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("rate"))
                        .custom(ty(manager, "decimal(20,0)"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("endTime"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("castle_id"))
                        .col(Alias::new("type")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `castle_manor_procure`
async fn create_castle_manor_procure(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("castle_manor_procure"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("castle_id"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("crop_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("amount"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("start_amount"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("price"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("reward_type"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("next_period"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null()
                        .default(dflt(manager, "'1'")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("castle_id"))
                        .col(Alias::new("crop_id"))
                        .col(Alias::new("next_period")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `castle_manor_production`
async fn create_castle_manor_production(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("castle_manor_production"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("castle_id"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("seed_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("amount"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("start_amount"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("price"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("next_period"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null()
                        .default(dflt(manager, "'1'")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("castle_id"))
                        .col(Alias::new("seed_id"))
                        .col(Alias::new("next_period")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `castle_siege_guards`
async fn create_castle_siege_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("castle_siege_guards"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("castleId"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .integer()
                        .primary_key()
                        .auto_increment(),
                )
                .col(
                    ColumnDef::new(Alias::new("npcId"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("x"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("y"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("z"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("heading"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("respawnDelay"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("isHired"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'1'")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `castle_trapupgrade`
async fn create_castle_trapupgrade(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("castle_trapupgrade"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("castleId"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("towerIndex"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("level"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("towerIndex"))
                        .col(Alias::new("castleId")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_contacts`
async fn create_character_contacts(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_contacts"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("contactId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("contactId")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_daily_rewards`
async fn create_character_daily_rewards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_daily_rewards"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("rewardId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("status"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "1")),
                )
                .col(
                    ColumnDef::new(Alias::new("progress"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("lastCompleted"))
                        .custom(ty(manager, "bigint"))
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("rewardId")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_friends`
async fn create_character_friends(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_friends"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("friendId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("relation"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("memo"))
                        .custom(ty(manager, "varchar(255)"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("friendId")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_hennas`
async fn create_character_hennas(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_hennas"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("symbol_id"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("slot"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("class_index"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("slot"))
                        .col(Alias::new("class_index")),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_charId_slot_classIndex")
                .table(Alias::new("character_hennas"))
                .col(Alias::new("charId"))
                .col(Alias::new("slot"))
                .col(Alias::new("class_index"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_charId_classIndex")
                .table(Alias::new("character_hennas"))
                .col(Alias::new("charId"))
                .col(Alias::new("class_index"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_instance_time`
async fn create_character_instance_time(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_instance_time"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("instanceId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("time"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("instanceId")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_item_reuse_save`
async fn create_character_item_reuse_save(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_item_reuse_save"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("itemId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("itemObjId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "1")),
                )
                .col(
                    ColumnDef::new(Alias::new("reuseDelay"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("systime"))
                        .custom(ty(manager, "BIGINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("itemId"))
                        .col(Alias::new("itemObjId")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_macroses`
async fn create_character_macroses(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_macroses"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("icon"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("name"))
                        .custom(ty(manager, "VARCHAR(40)"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("descr"))
                        .custom(ty(manager, "VARCHAR(80)"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("acronym"))
                        .custom(ty(manager, "VARCHAR(4)"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("commands"))
                        .custom(ty(manager, "VARCHAR(500)"))
                        .null(),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("id")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_mentees`
async fn create_character_mentees(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_mentees"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("mentorId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_offline_trade`
async fn create_character_offline_trade(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_offline_trade"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("time"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("type"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("title"))
                        .custom(ty(manager, "varchar(50)"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .primary_key(Index::create().col(Alias::new("charId")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_offline_trade_items`
async fn create_character_offline_trade_items(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_offline_trade_items"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("item"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("count"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("price"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("item")
                .table(Alias::new("character_offline_trade_items"))
                .col(Alias::new("item"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("charId")
                .table(Alias::new("character_offline_trade_items"))
                .col(Alias::new("charId"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_pet_skills_save`
async fn create_character_pet_skills_save(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_pet_skills_save"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("petObjItemId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_level"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "1")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_sub_level"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("remaining_time"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("buff_index"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("petObjItemId"))
                        .col(Alias::new("skill_id"))
                        .col(Alias::new("skill_level")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_premium_items`
async fn create_character_premium_items(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_premium_items"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("itemNum"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("itemId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("itemCount"))
                        .custom(ty(manager, "bigint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("itemSender"))
                        .custom(ty(manager, "varchar(50)"))
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("itemId")
                .table(Alias::new("character_premium_items"))
                .col(Alias::new("itemId"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("itemNum")
                .table(Alias::new("character_premium_items"))
                .col(Alias::new("itemNum"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_quests`
async fn create_character_quests(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_quests"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("name"))
                        .custom(ty(manager, "VARCHAR(60)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("var"))
                        .custom(ty(manager, "VARCHAR(20)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("value"))
                        .custom(ty(manager, "VARCHAR(255)"))
                        .null(),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("name"))
                        .col(Alias::new("var")),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_charId_name_var")
                .table(Alias::new("character_quests"))
                .unique()
                .col(Alias::new("charId"))
                .col(Alias::new("name"))
                .col(Alias::new("var"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_charId_var")
                .table(Alias::new("character_quests"))
                .col(Alias::new("charId"))
                .col(Alias::new("var"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_charId_name")
                .table(Alias::new("character_quests"))
                .col(Alias::new("charId"))
                .col(Alias::new("name"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_recipebook`
async fn create_character_recipebook(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_recipebook"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .custom(ty(manager, "decimal(11)"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("classIndex"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("type"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("id"))
                        .col(Alias::new("charId"))
                        .col(Alias::new("classIndex")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_recipeshoplist`
async fn create_character_recipeshoplist(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_recipeshoplist"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("recipeId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("price"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("index"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("recipeId")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_reco_bonus`
async fn create_character_reco_bonus(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_reco_bonus"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("rec_have"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("rec_left"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("time_left"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("charId_unique")
                .table(Alias::new("character_reco_bonus"))
                .unique()
                .col(Alias::new("charId"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_shortcuts`
async fn create_character_shortcuts(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_shortcuts"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("slot"))
                        .custom(ty(manager, "decimal(3)"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("page"))
                        .custom(ty(manager, "decimal(3)"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("type"))
                        .custom(ty(manager, "decimal(3)"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("shortcut_id"))
                        .custom(ty(manager, "decimal(16)"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("level"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("sub_level"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("class_index"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("slot"))
                        .col(Alias::new("page"))
                        .col(Alias::new("class_index")),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("shortcut_id")
                .table(Alias::new("character_shortcuts"))
                .col(Alias::new("shortcut_id"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_skills`
async fn create_character_skills(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_skills"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_level"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "1")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_sub_level"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("class_index"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("skill_id"))
                        .col(Alias::new("class_index")),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_skillId_charId_classIndex")
                .table(Alias::new("character_skills"))
                .col(Alias::new("skill_id"))
                .col(Alias::new("charId"))
                .col(Alias::new("class_index"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_skills_save`
async fn create_character_skills_save(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_skills_save"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_level"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "1")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_sub_level"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("remaining_time"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("reuse_delay"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("systime"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("restore_type"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("class_index"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("buff_index"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("skill_id"))
                        .col(Alias::new("skill_level"))
                        .col(Alias::new("class_index")),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_charId_classIndex_buffIndex")
                .table(Alias::new("character_skills_save"))
                .col(Alias::new("charId"))
                .col(Alias::new("class_index"))
                .col(Alias::new("buff_index"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_subclasses`
async fn create_character_subclasses(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_subclasses"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("class_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("exp"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("sp"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("level"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "40")),
                )
                .col(
                    ColumnDef::new(Alias::new("vitality_points"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("class_index"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("dual_class"))
                        .custom(ty(manager, "BOOLEAN"))
                        .not_null()
                        .default(dflt(manager, "FALSE")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("class_id")),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_charId_classId")
                .table(Alias::new("character_subclasses"))
                .col(Alias::new("charId"))
                .col(Alias::new("class_id"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_summon_skills_save`
async fn create_character_summon_skills_save(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_summon_skills_save"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("ownerId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("ownerClassIndex"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("summonSkillId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_level"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "1")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_sub_level"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("remaining_time"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("buff_index"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("ownerId"))
                        .col(Alias::new("ownerClassIndex"))
                        .col(Alias::new("summonSkillId"))
                        .col(Alias::new("skill_id"))
                        .col(Alias::new("skill_level")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_summons`
async fn create_character_summons(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_summons"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("ownerId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("summonId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("summonSkillId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("curHp"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("curMp"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("time"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("ownerId"))
                        .col(Alias::new("summonId"))
                        .col(Alias::new("summonSkillId")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_tpbookmark`
async fn create_character_tpbookmark(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_tpbookmark"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("Id"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("x"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("y"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("z"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("icon"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("tag"))
                        .custom(ty(manager, "varchar(50)"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("name"))
                        .custom(ty(manager, "varchar(50)"))
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("Id")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `character_variables`
async fn create_character_variables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("character_variables"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("var"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("val"))
                        .custom(ty(manager, "TEXT"))
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_var")
                .table(Alias::new("character_variables"))
                .col(Alias::new("var"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_charId")
                .table(Alias::new("character_variables"))
                .col(Alias::new("charId"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `characters`
async fn create_characters(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("characters"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("account_name"))
                        .custom(ty(manager, "VARCHAR(45)"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("char_name"))
                        .custom(ty(manager, "VARCHAR(35)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("level"))
                        .custom(ty(manager, "TINYINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("maxHp"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("curHp"))
                        .custom(ty_loose_f64(manager, "MEDIUMINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("maxCp"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("curCp"))
                        .custom(ty_loose_f64(manager, "MEDIUMINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("maxMp"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("curMp"))
                        .custom(ty_loose_f64(manager, "MEDIUMINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("face"))
                        .custom(ty(manager, "TINYINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("hairStyle"))
                        .custom(ty(manager, "TINYINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("hairColor"))
                        .custom(ty(manager, "TINYINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("sex"))
                        .custom(ty(manager, "TINYINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("heading"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("x"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("y"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("z"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("exp"))
                        .custom(ty(manager, "BIGINT"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("expBeforeDeath"))
                        .custom(ty(manager, "BIGINT"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("sp"))
                        .custom(ty(manager, "BIGINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("reputation"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("fame"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("raidbossPoints"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("pvpkills"))
                        .custom(ty(manager, "SMALLINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("pkkills"))
                        .custom(ty(manager, "SMALLINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("clanid"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("race"))
                        .custom(ty(manager, "TINYINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("classid"))
                        .custom(ty(manager, "TINYINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("base_class"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("transform_id"))
                        .custom(ty(manager, "SMALLINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("deletetime"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("cancraft"))
                        .custom(ty(manager, "TINYINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("title"))
                        .custom(ty(manager, "VARCHAR(21)"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("title_color"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .not_null()
                        .default(dflt(manager, "0xECF9A2")),
                )
                .col(
                    ColumnDef::new(Alias::new("accesslevel"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("online"))
                        .custom(ty(manager, "TINYINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("onlinetime"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("char_slot"))
                        .custom(ty(manager, "TINYINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("lastAccess"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("clan_privs"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("wantspeace"))
                        .custom(ty(manager, "TINYINT"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("power_grade"))
                        .custom(ty(manager, "TINYINT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("nobless"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("subpledge"))
                        .custom(ty(manager, "SMALLINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("lvl_joined_academy"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("apprentice"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("sponsor"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("clan_join_expiry_time"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("clan_create_expiry_time"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("bookmarkslot"))
                        .custom(ty(manager, "SMALLINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("vitality_points"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("createDate"))
                        .custom(ty(manager, "date"))
                        .not_null()
                        .default(dflt(manager, "'2015-01-01'")),
                )
                .col(
                    ColumnDef::new(Alias::new("language"))
                        .custom(ty(manager, "VARCHAR(2)"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("faction"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("pccafe_points"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(Index::create().col(Alias::new("charId")))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("online")
                .table(Alias::new("characters"))
                .col(Alias::new("online"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("clanid")
                .table(Alias::new("characters"))
                .col(Alias::new("clanid"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("char_name")
                .table(Alias::new("characters"))
                .col(Alias::new("char_name"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("account_name")
                .table(Alias::new("characters"))
                .col(Alias::new("account_name"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_createDate")
                .table(Alias::new("characters"))
                .col(Alias::new("createDate"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_accountName_createDate")
                .table(Alias::new("characters"))
                .col(Alias::new("account_name"))
                .col(Alias::new("createDate"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_account_name")
                .table(Alias::new("characters"))
                .col(Alias::new("account_name"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_char_name")
                .table(Alias::new("characters"))
                .col(Alias::new("char_name"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `clan_data`
async fn create_clan_data(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("clan_data"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("clan_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("clan_name"))
                        .custom(ty(manager, "varchar(45)"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("clan_level"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("reputation_score"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("hasCastle"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("blood_alliance_count"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("blood_oath_count"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("ally_id"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("ally_name"))
                        .custom(ty(manager, "varchar(45)"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("leader_id"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("crest_id"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("crest_large_id"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("ally_crest_id"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("auction_bid_at"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("ally_penalty_expiry_time"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("ally_penalty_type"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("char_penalty_expiry_time"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("dissolving_expiry_time"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("new_leader_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(Index::create().col(Alias::new("clan_id")))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("auction_bid_at")
                .table(Alias::new("clan_data"))
                .col(Alias::new("auction_bid_at"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("leader_id")
                .table(Alias::new("clan_data"))
                .col(Alias::new("leader_id"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ally_id")
                .table(Alias::new("clan_data"))
                .col(Alias::new("ally_id"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `clan_notices`
async fn create_clan_notices(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("clan_notices"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("clan_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("enabled"))
                        .custom(ty(manager, "varchar(10)"))
                        .not_null()
                        .default(dflt(manager, "'false'")),
                )
                .col(
                    ColumnDef::new(Alias::new("notice"))
                        .custom(ty(manager, "TEXT"))
                        .not_null(),
                )
                .primary_key(Index::create().col(Alias::new("clan_id")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `clan_privs`
async fn create_clan_privs(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("clan_privs"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("clan_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("rank"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("party"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("privs"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("clan_id"))
                        .col(Alias::new("rank"))
                        .col(Alias::new("party")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `clan_skills`
async fn create_clan_skills(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("clan_skills"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("clan_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_level"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("skill_name"))
                        .custom(ty(manager, "varchar(26)"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("sub_pledge_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'-2'")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("clan_id"))
                        .col(Alias::new("skill_id"))
                        .col(Alias::new("sub_pledge_id")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `clan_subpledges`
async fn create_clan_subpledges(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("clan_subpledges"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("clan_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("sub_pledge_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("name"))
                        .custom(ty(manager, "varchar(45)"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("leader_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("clan_id"))
                        .col(Alias::new("sub_pledge_id")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `clan_variables`
async fn create_clan_variables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("clan_variables"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("clanId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("var"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("val"))
                        .custom(ty(manager, "TEXT"))
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `clan_wars`
async fn create_clan_wars(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("clan_wars"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("clan1"))
                        .custom(ty(manager, "varchar(35)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("clan2"))
                        .custom(ty(manager, "varchar(35)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("clan1Kill"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("clan2Kill"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("winnerClan"))
                        .custom(ty(manager, "varchar(35)"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("startTime"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("endTime"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("state"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("clan1"))
                        .col(Alias::new("clan2")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `clanhall`
async fn create_clanhall(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("clanhall"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("ownerId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("paidUntil"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(Index::create().col(Alias::new("id")))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ownerId")
                .table(Alias::new("clanhall"))
                .col(Alias::new("ownerId"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `clanhall_auctions_bidders`
async fn create_clanhall_auctions_bidders(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("clanhall_auctions_bidders"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("clanHallId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("clanId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("bid"))
                        .custom(ty(manager, "BIGINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("bidTime"))
                        .custom(ty(manager, "BIGINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("clanHallId"))
                        .col(Alias::new("clanId")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `commission_items`
async fn create_commission_items(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("commission_items"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("commission_id"))
                        .integer()
                        .primary_key()
                        .auto_increment(),
                )
                .col(
                    ColumnDef::new(Alias::new("item_object_id"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("price_per_unit"))
                        .custom(ty(manager, "BIGINT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("start_time"))
                        .custom(ty(manager, "TIMESTAMP"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("duration_in_days"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("discount_in_percentage"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `crests`
async fn create_crests(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("crests"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("crest_id"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("data"))
                        .custom(ty(manager, "VARBINARY(2176)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("type"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null(),
                )
                .primary_key(Index::create().col(Alias::new("crest_id")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `cursed_weapons`
async fn create_cursed_weapons(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("cursed_weapons"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("itemId"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("playerReputation"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("playerPkKills"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("nbKills"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("endTime"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(Index::create().col(Alias::new("itemId")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `custom_mail`
async fn create_custom_mail(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("custom_mail"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("date"))
                        .custom(ty(manager, "TIMESTAMP"))
                        .not_null()
                        .default(dflt(manager, "CURRENT_TIMESTAMP")),
                )
                .col(
                    ColumnDef::new(Alias::new("receiver"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("subject"))
                        .custom(ty(manager, "TINYTEXT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("message"))
                        .custom(ty(manager, "TEXT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("items"))
                        .custom(ty(manager, "TEXT"))
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `custom_teleport`
async fn create_custom_teleport(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("custom_teleport"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("Description"))
                        .custom(ty(manager, "varchar(75)"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("loc_x"))
                        .custom(ty(manager, "mediumint"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("loc_y"))
                        .custom(ty(manager, "mediumint"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("loc_z"))
                        .custom(ty(manager, "mediumint"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("price"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("fornoble"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("itemId"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "'57'")),
                )
                .primary_key(Index::create().col(Alias::new("id")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `event_schedulers`
async fn create_event_schedulers(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("event_schedulers"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .integer()
                        .primary_key()
                        .auto_increment(),
                )
                .col(
                    ColumnDef::new(Alias::new("eventName"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("schedulerName"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("lastRun"))
                        .custom(ty(manager, "timestamp"))
                        .not_null()
                        .default(dflt(manager, "CURRENT_TIMESTAMP")),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("eventName_schedulerName_unique")
                .table(Alias::new("event_schedulers"))
                .unique()
                .col(Alias::new("eventName"))
                .col(Alias::new("schedulerName"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `fort`
async fn create_fort(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("fort"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("name"))
                        .custom(ty(manager, "varchar(25)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("siegeDate"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("lastOwnedTime"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("owner"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("fortType"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("state"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("castleId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("supplyLvL"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(Index::create().col(Alias::new("id")))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_fort_owner")
                .table(Alias::new("fort"))
                .col(Alias::new("owner"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `fort_doorupgrade`
async fn create_fort_doorupgrade(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("fort_doorupgrade"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("doorId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("fortId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("hp"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("pDef"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("mDef"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(Index::create().col(Alias::new("doorId")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `fort_functions`
async fn create_fort_functions(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("fort_functions"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("fort_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("type"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("lvl"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("lease"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("rate"))
                        .custom(ty(manager, "decimal(20,0)"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("endTime"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("fort_id"))
                        .col(Alias::new("type")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `fort_siege_guards`
async fn create_fort_siege_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("fort_siege_guards"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("fortId"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .integer()
                        .primary_key()
                        .auto_increment(),
                )
                .col(
                    ColumnDef::new(Alias::new("npcId"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("x"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("y"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("z"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("heading"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("respawnDelay"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("isHired"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'1'")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `fort_spawnlist`
async fn create_fort_spawnlist(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("fort_spawnlist"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("fortId"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .integer()
                        .primary_key()
                        .auto_increment(),
                )
                .col(
                    ColumnDef::new(Alias::new("npcId"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("x"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("y"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("z"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("heading"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("spawnType"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("castleId"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_fortId")
                .table(Alias::new("fort_spawnlist"))
                .col(Alias::new("fortId"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `fortsiege_clans`
async fn create_fortsiege_clans(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("fortsiege_clans"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("fort_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("clan_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("clan_id"))
                        .col(Alias::new("fort_id")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `forums`
async fn create_forums(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("forums"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("forum_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("forum_name"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("forum_parent"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("forum_post"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("forum_type"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("forum_perm"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("forum_owner_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(Index::create().col(Alias::new("forum_id")))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_forums_owner_id")
                .table(Alias::new("forums"))
                .col(Alias::new("forum_owner_id"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `global_tasks`
async fn create_global_tasks(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("global_tasks"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .integer()
                        .not_null()
                        .primary_key()
                        .auto_increment(),
                )
                .col(
                    ColumnDef::new(Alias::new("task"))
                        .custom(ty(manager, "varchar(50)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("type"))
                        .custom(ty(manager, "varchar(50)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("last_activation"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("param1"))
                        .custom(ty(manager, "varchar(100)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("param2"))
                        .custom(ty(manager, "varchar(100)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("param3"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `global_variables`
async fn create_global_variables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("global_variables"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("var"))
                        .custom(ty(manager, "VARCHAR(255)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("value"))
                        .custom(ty(manager, "VARCHAR(255)"))
                        .null(),
                )
                .primary_key(Index::create().col(Alias::new("var")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `grandboss_data`
async fn create_grandboss_data(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("grandboss_data"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("boss_id"))
                        .custom(ty(manager, "smallint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("loc_x"))
                        .custom(ty(manager, "mediumint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("loc_y"))
                        .custom(ty(manager, "mediumint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("loc_z"))
                        .custom(ty(manager, "mediumint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("heading"))
                        .custom(ty(manager, "mediumint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("respawn_time"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("currentHP"))
                        .custom(ty(manager, "double"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("currentMP"))
                        .custom(ty(manager, "double"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("status"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(Index::create().col(Alias::new("boss_id")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `heroes`
async fn create_heroes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("heroes"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("class_id"))
                        .custom(ty(manager, "decimal(3,0)"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("count"))
                        .custom(ty(manager, "decimal(3,0)"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("played"))
                        .custom(ty(manager, "decimal(1,0)"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("claimed"))
                        .custom(ty(manager, "varchar(5)"))
                        .not_null()
                        .default(dflt(manager, "'false'")),
                )
                .col(
                    ColumnDef::new(Alias::new("message"))
                        .custom(ty(manager, "varchar(300)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .primary_key(Index::create().col(Alias::new("charId")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `heroes_diary`
async fn create_heroes_diary(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("heroes_diary"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("time"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("action"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("param"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `item_auction`
async fn create_item_auction(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("item_auction"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("auctionId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("instanceId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("auctionItemId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("startingTime"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("endingTime"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("auctionStateId"))
                        .custom(ty(manager, "tinyint"))
                        .not_null(),
                )
                .primary_key(Index::create().col(Alias::new("auctionId")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `item_auction_bid`
async fn create_item_auction_bid(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("item_auction_bid"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("auctionId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("playerObjId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("playerBid"))
                        .custom(ty(manager, "bigint"))
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("auctionId"))
                        .col(Alias::new("playerObjId")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `item_elementals`
async fn create_item_elementals(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("item_elementals"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("itemId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("elemType"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "-1")),
                )
                .col(
                    ColumnDef::new(Alias::new("elemValue"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "-1")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("itemId"))
                        .col(Alias::new("elemType")),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_itemId_elemType")
                .table(Alias::new("item_elementals"))
                .col(Alias::new("itemId"))
                .col(Alias::new("elemType"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `item_variables`
async fn create_item_variables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("item_variables"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("var"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("val"))
                        .custom(ty(manager, "TEXT"))
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_id")
                .table(Alias::new("item_variables"))
                .col(Alias::new("id"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `item_variations`
async fn create_item_variations(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("item_variations"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("itemId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("mineralId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("option1"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("option2"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .primary_key(Index::create().col(Alias::new("itemId")))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_itemId")
                .table(Alias::new("item_variations"))
                .col(Alias::new("itemId"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `items`
async fn create_items(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("items"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("owner_id"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("object_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("item_id"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("count"))
                        .custom(ty(manager, "BIGINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("enchant_level"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("loc"))
                        .custom(ty(manager, "VARCHAR(10)"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("loc_data"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("time_of_use"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("custom_type1"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("custom_type2"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("mana_left"))
                        .custom(ty(manager, "decimal(5,0)"))
                        .not_null()
                        .default(dflt(manager, "-1")),
                )
                .col(
                    ColumnDef::new(Alias::new("time"))
                        .custom(ty(manager, "decimal(13)"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(Index::create().col(Alias::new("object_id")))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("time_of_use")
                .table(Alias::new("items"))
                .col(Alias::new("time_of_use"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("loc")
                .table(Alias::new("items"))
                .col(Alias::new("loc"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("item_id")
                .table(Alias::new("items"))
                .col(Alias::new("item_id"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("owner_id")
                .table(Alias::new("items"))
                .col(Alias::new("owner_id"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_owner_id_loc_locdata_enchant")
                .table(Alias::new("items"))
                .col(Alias::new("owner_id"))
                .col(Alias::new("loc"))
                .col(Alias::new("loc_data"))
                .col(Alias::new("enchant_level"))
                .col(Alias::new("item_id"))
                .col(Alias::new("object_id"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_owner_id_loc_locdata")
                .table(Alias::new("items"))
                .col(Alias::new("owner_id"))
                .col(Alias::new("loc"))
                .col(Alias::new("loc_data"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_owner_id_item_id")
                .table(Alias::new("items"))
                .col(Alias::new("owner_id"))
                .col(Alias::new("item_id"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_owner_id_loc")
                .table(Alias::new("items"))
                .col(Alias::new("owner_id"))
                .col(Alias::new("loc"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_owner_id")
                .table(Alias::new("items"))
                .col(Alias::new("owner_id"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_object_id")
                .table(Alias::new("items"))
                .col(Alias::new("object_id"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_item_id")
                .table(Alias::new("items"))
                .col(Alias::new("item_id"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `itemsonground`
async fn create_itemsonground(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("itemsonground"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("object_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("item_id"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("count"))
                        .custom(ty(manager, "BIGINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("enchant_level"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("x"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("y"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("z"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("drop_time"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("equipable"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(Index::create().col(Alias::new("object_id")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `lottery`
async fn create_lottery(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("lottery"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("idnr"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("number1"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("number2"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("prize"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("newprize"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("prize1"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("prize2"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("prize3"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("enddate"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("finished"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("id"))
                        .col(Alias::new("idnr")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `mdt_bets`
async fn create_mdt_bets(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("mdt_bets"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("lane_id"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("bet"))
                        .custom(ty(manager, "bigint"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(Index::create().col(Alias::new("lane_id")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `mdt_history`
async fn create_mdt_history(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("mdt_history"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("race_id"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("first"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("second"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("odd_rate"))
                        .custom(ty(manager, "DOUBLE(10,2)"))
                        .null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(Index::create().col(Alias::new("race_id")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `merchant_lease`
async fn create_merchant_lease(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("merchant_lease"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("merchant_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("player_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("bid"))
                        .custom(ty(manager, "INT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("type"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("player_name"))
                        .custom(ty(manager, "varchar(35)"))
                        .null(),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("merchant_id"))
                        .col(Alias::new("player_id"))
                        .col(Alias::new("type")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `messages`
async fn create_messages(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("messages"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("messageId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("senderId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("receiverId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("subject"))
                        .custom(ty(manager, "TINYTEXT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("content"))
                        .custom(ty(manager, "TEXT"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("expiration"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("reqAdena"))
                        .custom(ty(manager, "BIGINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("hasAttachments"))
                        .custom(ty(manager, "varchar(10)"))
                        .not_null()
                        .default(dflt(manager, "'false'")),
                )
                .col(
                    ColumnDef::new(Alias::new("isUnread"))
                        .custom(ty(manager, "varchar(10)"))
                        .not_null()
                        .default(dflt(manager, "'true'")),
                )
                .col(
                    ColumnDef::new(Alias::new("isDeletedBySender"))
                        .custom(ty(manager, "varchar(10)"))
                        .not_null()
                        .default(dflt(manager, "'false'")),
                )
                .col(
                    ColumnDef::new(Alias::new("isDeletedByReceiver"))
                        .custom(ty(manager, "varchar(10)"))
                        .not_null()
                        .default(dflt(manager, "'false'")),
                )
                .col(
                    ColumnDef::new(Alias::new("isLocked"))
                        .custom(ty(manager, "varchar(10)"))
                        .not_null()
                        .default(dflt(manager, "'false'")),
                )
                .col(
                    ColumnDef::new(Alias::new("sendBySystem"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("isReturned"))
                        .custom(ty(manager, "varchar(10)"))
                        .not_null()
                        .default(dflt(manager, "'false'")),
                )
                .col(
                    ColumnDef::new(Alias::new("itemId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("enchantLvl"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("elementals"))
                        .custom(ty(manager, "VARCHAR(25)"))
                        .null(),
                )
                .primary_key(Index::create().col(Alias::new("messageId")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `npc_respawns`
async fn create_npc_respawns(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("npc_respawns"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("x"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("y"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("z"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("heading"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("respawnTime"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("currentHp"))
                        .custom(ty(manager, "double"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("currentMp"))
                        .custom(ty(manager, "double"))
                        .not_null(),
                )
                .primary_key(Index::create().col(Alias::new("id")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `olympiad_data`
async fn create_olympiad_data(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("olympiad_data"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("current_cycle"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .not_null()
                        .default(dflt(manager, "1")),
                )
                .col(
                    ColumnDef::new(Alias::new("period"))
                        .custom(ty(manager, "MEDIUMINT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("olympiad_end"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("validation_end"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("next_weekly_change"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(Index::create().col(Alias::new("id")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `olympiad_fights`
async fn create_olympiad_fights(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("olympiad_fights"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charOneId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("charTwoId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("charOneClass"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("charTwoClass"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("winner"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("start"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("time"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("classed"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("charTwoId")
                .table(Alias::new("olympiad_fights"))
                .col(Alias::new("charTwoId"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("charOneId")
                .table(Alias::new("olympiad_fights"))
                .col(Alias::new("charOneId"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `olympiad_nobles`
async fn create_olympiad_nobles(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("olympiad_nobles"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("class_id"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("olympiad_points"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("competitions_done"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("competitions_won"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("competitions_lost"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("competitions_drawn"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("competitions_done_week"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(Index::create().col(Alias::new("charId")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `olympiad_nobles_eom`
async fn create_olympiad_nobles_eom(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("olympiad_nobles_eom"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("class_id"))
                        .custom(ty(manager, "tinyint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("olympiad_points"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("competitions_done"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("competitions_won"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("competitions_lost"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("competitions_drawn"))
                        .custom(ty(manager, "smallint"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .primary_key(Index::create().col(Alias::new("charId")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `party_matching_history`
async fn create_party_matching_history(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("party_matching_history"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .integer()
                        .primary_key()
                        .auto_increment(),
                )
                .col(
                    ColumnDef::new(Alias::new("title"))
                        .custom(ty(manager, "VARCHAR(21)"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("leader"))
                        .custom(ty(manager, "VARCHAR(35)"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `petition_feedback`
async fn create_petition_feedback(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("petition_feedback"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charName"))
                        .custom(ty(manager, "VARCHAR(35)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("gmName"))
                        .custom(ty(manager, "VARCHAR(35)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("rate"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null()
                        .default(dflt(manager, "2")),
                )
                .col(
                    ColumnDef::new(Alias::new("message"))
                        .custom(ty(manager, "TEXT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("date"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `pets`
async fn create_pets(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("pets"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("item_obj_id"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("name"))
                        .custom(ty(manager, "varchar(16)"))
                        .null(),
                )
                .col(
                    ColumnDef::new(Alias::new("level"))
                        .custom(ty(manager, "smallint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("curHp"))
                        .custom(ty_loose_f64(manager, "INT"))
                        .null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("curMp"))
                        .custom(ty_loose_f64(manager, "INT"))
                        .null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("exp"))
                        .custom(ty(manager, "bigint"))
                        .null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("sp"))
                        .custom(ty(manager, "bigint"))
                        .null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("fed"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("ownerId"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("restore"))
                        .custom(ty(manager, "varchar(10)"))
                        .not_null()
                        .default(dflt(manager, "'false'")),
                )
                .primary_key(Index::create().col(Alias::new("item_obj_id")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `pledge_applicant`
async fn create_pledge_applicant(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("pledge_applicant"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("charId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("clanId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("karma"))
                        .custom(ty(manager, "tinyint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("message"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("charId"))
                        .col(Alias::new("clanId")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `pledge_recruit`
async fn create_pledge_recruit(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("pledge_recruit"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("clan_id"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("karma"))
                        .custom(ty(manager, "tinyint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("information"))
                        .custom(ty(manager, "varchar(50)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("detailed_information"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("application_type"))
                        .custom(ty(manager, "tinyint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("recruit_type"))
                        .custom(ty(manager, "tinyint"))
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `pledge_waiting_list`
async fn create_pledge_waiting_list(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("pledge_waiting_list"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("char_id"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("karma"))
                        .custom(ty(manager, "tinyint"))
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `posts`
async fn create_posts(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("posts"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("post_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("post_owner_name"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("post_ownerid"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("post_date"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("post_topic_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("post_forum_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("post_txt"))
                        .custom(ty(manager, "TEXT"))
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("post_forum_id")
                .table(Alias::new("posts"))
                .col(Alias::new("post_forum_id"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `punishments`
async fn create_punishments(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("punishments"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .integer()
                        .primary_key()
                        .auto_increment(),
                )
                .col(
                    ColumnDef::new(Alias::new("key"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("affect"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("type"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("expiration"))
                        .custom(ty(manager, "bigint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("reason"))
                        .custom(ty(manager, "TEXT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("punishedBy"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `residence_functions`
async fn create_residence_functions(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("residence_functions"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("level"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("expiration"))
                        .custom(ty(manager, "bigint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("residenceId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("id"))
                        .col(Alias::new("level"))
                        .col(Alias::new("residenceId")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `siege_clans`
async fn create_siege_clans(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("siege_clans"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("castle_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("clan_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "0")),
                )
                .col(
                    ColumnDef::new(Alias::new("type"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .col(
                    ColumnDef::new(Alias::new("castle_owner"))
                        .custom(ty(manager, "INT"))
                        .null()
                        .default(dflt(manager, "NULL")),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("clan_id"))
                        .col(Alias::new("castle_id")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `topic`
async fn create_topic(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("topic"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("topic_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("topic_forum_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("topic_name"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("topic_date"))
                        .custom(ty(manager, "bigint"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("topic_ownername"))
                        .custom(ty(manager, "varchar(255)"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("topic_ownerid"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("topic_type"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("topic_reply"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}
