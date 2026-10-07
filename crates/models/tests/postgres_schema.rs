//! Every entity must be able to read the PostgreSQL schema the migrations build.
//!
//! SQLite forgives a mismatch between a column's declared type and the field
//! reading it; PostgreSQL's driver does not. An `i32` field cannot decode a
//! `BIGINT`, an `i64` cannot decode an `INTEGER`, and a `String` cannot decode a
//! `TIMESTAMP` — each fails the whole query at runtime. The migrations pick
//! PostgreSQL types to suit the entities (`migration::dialect`); this checks
//! the pick, column by column, plus the column sets and nullability the SQLite
//! parity test checks.
//!
//! Runs only when `L2R_TEST_DATABASE_URL` names a server it may create a
//! scratch database on, e.g. `postgres://l2:l2@localhost:55432/l2`
//! (`commons::db::testing`).

use std::collections::BTreeMap;

use migration::MigratorTrait;
use models::sea_orm::sea_query::{ColumnType, TableCreateStatement};
use models::sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityName, EntityTrait, IdenStatic,
    Iterable, PrimaryKeyToColumn, Schema, Statement,
};

/// The PostgreSQL types a field of this column type can decode.
fn decodable(ty: &ColumnType) -> &'static [&'static str] {
    match ty {
        ColumnType::Integer => &["int4"],
        ColumnType::BigInteger => &["int8"],
        ColumnType::Double => &["float8"],
        ColumnType::String(_) | ColumnType::Text | ColumnType::Char(_) => &["varchar", "text"],
        ColumnType::Binary(_) | ColumnType::VarBinary(_) | ColumnType::Blob => &["bytea"],
        other => panic!("no PostgreSQL expectation for entity column type {other:?}"),
    }
}

/// `column name -> (type, is NOT NULL)`, out of the entity definition.
fn entity_columns<E: EntityTrait>(entity: E) -> BTreeMap<String, (ColumnType, bool)> {
    let statement: TableCreateStatement =
        Schema::new(DatabaseBackend::Postgres).create_table_from_entity(entity);
    statement
        .get_columns()
        .iter()
        .map(|col| {
            (
                col.get_column_name(),
                (
                    col.get_column_type().expect("typed column").clone(),
                    col.get_column_spec().nullable == Some(false),
                ),
            )
        })
        .collect()
}

/// `column name -> (udt name, is NOT NULL)`, out of the migrated database.
async fn table_columns(db: &DatabaseConnection, table: &str) -> BTreeMap<String, (String, bool)> {
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT column_name::text AS name, udt_name::text AS udt, is_nullable::text AS nullable \
             FROM information_schema.columns \
             WHERE table_schema = 'public' AND table_name = $1",
            [table.into()],
        ))
        .await
        .unwrap();
    assert!(!rows.is_empty(), "migrations created no table `{table}`");
    rows.iter()
        .map(|row| {
            (
                row.try_get::<String>("", "name").unwrap(),
                (
                    row.try_get::<String>("", "udt").unwrap(),
                    row.try_get::<String>("", "nullable").unwrap() == "NO",
                ),
            )
        })
        .collect()
}

async fn check<E: EntityTrait>(db: &DatabaseConnection, entity: E, table: &str) -> Vec<String> {
    let entity_cols = entity_columns(entity);
    let db_cols = table_columns(db, table).await;
    let mut problems = Vec::new();

    let entity_names: Vec<_> = entity_cols.keys().cloned().collect();
    let db_names: Vec<_> = db_cols.keys().cloned().collect();
    if entity_names != db_names {
        problems.push(format!(
            "`{table}`: column sets differ\n  entity:   {entity_names:?}\n  database: {db_names:?}"
        ));
        return problems;
    }

    let keys: Vec<String> = E::PrimaryKey::iter()
        .map(|k| k.into_column().as_str().to_string())
        .collect();
    for (name, (ty, not_null)) in &entity_cols {
        let (udt, db_not_null) = &db_cols[name];
        if !decodable(ty).contains(&udt.as_str()) {
            problems.push(format!(
                "`{table}`.`{name}`: entity reads {ty:?}, column is {udt}"
            ));
        }
        if !keys.contains(name) && not_null != db_not_null {
            problems.push(format!(
                "`{table}`.`{name}`: entity says NOT NULL = {not_null}, database says {db_not_null}"
            ));
        }
    }
    problems
}

macro_rules! check_all {
    ($db:expr, $($module:ident),* $(,)?) => {{
        let mut problems = Vec::new();
        $(
            problems.extend(
                check(
                    $db,
                    models::entity::$module::Entity,
                    models::entity::$module::Entity.table_name(),
                )
                .await,
            );
        )*
        problems
    }};
}

#[tokio::test]
async fn entities_can_read_the_migrated_postgres_schema() {
    if std::env::var(commons::db::testing::ENV).is_err() {
        eprintln!(
            "skipped: set {} to run against PostgreSQL",
            commons::db::testing::ENV
        );
        return;
    }
    let scratch = commons::db::testing::TestDb::new("schema").await;
    let db = commons::db::connect(&scratch.url, 1).await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();

    let problems = check_all!(
        &db,
        account_data,
        account_gsdata,
        account_premium,
        accounts,
        accounts_ipauth,
        airships,
        announcements,
        auction_bid,
        bbs_favorites,
        bot_reported_char_data,
        buffer_schemes,
        buylists,
        castle,
        castle_doorupgrade,
        castle_functions,
        castle_manor_procure,
        castle_manor_production,
        castle_siege_guards,
        castle_trapupgrade,
        character_contacts,
        character_daily_rewards,
        character_friends,
        character_hennas,
        character_instance_time,
        character_item_reuse_save,
        character_macroses,
        character_mentees,
        character_offline_trade,
        character_offline_trade_items,
        character_pet_skills_save,
        character_premium_items,
        character_quests,
        character_recipebook,
        character_recipeshoplist,
        character_reco_bonus,
        character_shortcuts,
        character_skills,
        character_skills_save,
        character_subclasses,
        character_summon_skills_save,
        character_summons,
        character_tpbookmark,
        character_variables,
        characters,
        clan_data,
        clan_notices,
        clan_privs,
        clan_skills,
        clan_subpledges,
        clan_variables,
        clan_wars,
        clanhall,
        clanhall_auctions_bidders,
        commission_items,
        crests,
        cursed_weapons,
        custom_mail,
        custom_teleport,
        event_schedulers,
        fort,
        fort_doorupgrade,
        fort_functions,
        fort_siege_guards,
        fort_spawnlist,
        fortsiege_clans,
        forums,
        gameservers,
        global_tasks,
        global_variables,
        grandboss_data,
        heroes,
        heroes_diary,
        ip_bans,
        item_auction,
        item_auction_bid,
        item_elementals,
        item_variables,
        item_variations,
        items,
        itemsonground,
        lottery,
        mdt_bets,
        mdt_history,
        merchant_lease,
        messages,
        npc_respawns,
        olympiad_data,
        olympiad_fights,
        olympiad_nobles,
        olympiad_nobles_eom,
        party_matching_history,
        petition_feedback,
        pets,
        pledge_applicant,
        pledge_recruit,
        pledge_waiting_list,
        posts,
        punishments,
        residence_functions,
        siege_clans,
        topic,
    );
    db.close().await.unwrap();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
