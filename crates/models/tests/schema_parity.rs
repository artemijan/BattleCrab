//! Every entity must agree with the schema the migrations build.
//!
//! The two are written from the same source (`dist/db_installer/sql/**`) but by
//! different paths — one through `sea-orm-cli generate entity`, one through
//! `tools/gen_migrations.py`. When they drift, the symptom in production is a
//! decode error on a column nobody touched in months, so it is worth one test.
//!
//! What is checked, per table: the column sets match, and a column the entity
//! calls non-nullable is `NOT NULL` in the database. Primary keys are exempt
//! from the null check — SQLite lets a non-INTEGER key column be nullable, and
//! the entities deliberately type those as non-`Option` (SeaORM cannot build a
//! key out of `Option<_>`).

use std::collections::BTreeMap;

use migration::MigratorTrait;
use models::sea_orm::sea_query::TableCreateStatement;
use models::sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityName, EntityTrait, IdenStatic,
    Iterable, PrimaryKeyToColumn, Schema, Statement,
};

/// `column name -> is NOT NULL`, straight out of the entity definition.
fn entity_columns<E: EntityTrait>(entity: E) -> BTreeMap<String, bool> {
    let schema = Schema::new(DatabaseBackend::Sqlite);
    let statement: TableCreateStatement = schema.create_table_from_entity(entity);
    statement
        .get_columns()
        .iter()
        .map(|col| {
            // `nullable` is three-state: unset means the column definition
            // never said either way, which SeaORM renders as nullable.
            let not_null = col.get_column_spec().nullable == Some(false);
            (col.get_column_name(), not_null)
        })
        .collect()
}

/// `column name -> is NOT NULL`, straight out of the migrated database.
async fn table_columns(db: &DatabaseConnection, table: &str) -> BTreeMap<String, bool> {
    let rows = db
        .query_all_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("PRAGMA table_info(\"{table}\")").as_str(),
        ))
        .await
        .unwrap();
    assert!(!rows.is_empty(), "migrations created no table `{table}`");
    rows.iter()
        .map(|row| {
            (
                row.try_get::<String>("", "name").unwrap(),
                row.try_get::<i32>("", "notnull").unwrap() == 1,
            )
        })
        .collect()
}

async fn check<E: EntityTrait>(db: &DatabaseConnection, entity: E, table: &str) {
    let mut entity_cols = entity_columns(entity);
    // Not a real column: `accounts` keys off SQLite's implicit rowid because
    // its `login` is nullable.
    entity_cols.remove("rowid");
    let db_cols = table_columns(db, table).await;

    let entity_names: Vec<_> = entity_cols.keys().cloned().collect();
    let db_names: Vec<_> = db_cols.keys().cloned().collect();
    assert_eq!(entity_names, db_names, "`{table}`: column sets differ");

    let keys: Vec<String> = E::PrimaryKey::iter()
        .map(|k| k.into_column().as_str().to_string())
        .collect();
    for (name, not_null) in &entity_cols {
        if keys.contains(name) {
            continue;
        }
        assert_eq!(
            *not_null, db_cols[name],
            "`{table}`.`{name}`: entity says NOT NULL = {not_null}, database disagrees"
        );
    }
}

macro_rules! check_all {
    ($db:expr, $($module:ident),* $(,)?) => {
        $(
            check(
                $db,
                models::entity::$module::Entity,
                models::entity::$module::Entity.table_name(),
            )
            .await;
        )*
    };
}

#[tokio::test]
async fn entities_match_the_migrated_schema() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();

    check_all!(
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
}
