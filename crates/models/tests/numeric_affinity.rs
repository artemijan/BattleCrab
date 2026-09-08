//! A column an entity reads as `f64` must have REAL affinity in SQLite.
//!
//! SQLite types a *value*, not a column: a `decimal(…)` column has **NUMERIC**
//! affinity, which stores a losslessly-integral value as INTEGER, and sqlx then
//! refuses INTEGER → `f64`. One such value fails the entire `SELECT`, taking
//! every other row of the table with it.
//!
//! That is how GitHub #19 happened. `grandboss_data.currentHP` shipped as
//! `decimal(30,15)`; the `0.0` written when a grand boss dies landed as
//! INTEGER; the boot load then failed outright and the server came up with no
//! grand bosses at all — no Queen Ant in the Ant Nest, no Orfen — and said
//! nothing about it. Declaring the column `double` gives it REAL affinity, so
//! SQLite converts on the way in and the value can never come back an integer.
//!
//! The first test is the general rule; the second is the round trip that
//! actually broke.

use migration::MigratorTrait;
use models::sea_orm::sea_query::{ColumnType, TableCreateStatement};
use models::sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, Database, DatabaseBackend,
    DatabaseConnection, EntityName, EntityTrait, Schema, Statement,
};

/// SQLite's column-affinity rules (datatype.html §3.1), which decide the
/// storage class a value gets on the way in.
fn affinity(declared: &str) -> &'static str {
    let t = declared.to_uppercase();
    if t.contains("INT") {
        "INTEGER"
    } else if t.contains("CHAR") || t.contains("CLOB") || t.contains("TEXT") {
        "TEXT"
    } else if t.is_empty() || t.contains("BLOB") {
        "BLOB"
    } else if t.contains("REAL") || t.contains("FLOA") || t.contains("DOUB") {
        "REAL"
    } else {
        "NUMERIC"
    }
}

/// The columns this entity reads as `f64`.
fn double_columns<E: EntityTrait>(entity: E) -> Vec<String> {
    let statement: TableCreateStatement =
        Schema::new(DatabaseBackend::Sqlite).create_table_from_entity(entity);
    statement
        .get_columns()
        .iter()
        .filter(|col| matches!(col.get_column_type(), Some(ColumnType::Double)))
        .map(|col| col.get_column_name())
        .collect()
}

/// `column name -> declared type`, out of the migrated database.
async fn declared_types(db: &DatabaseConnection, table: &str) -> Vec<(String, String)> {
    db.query_all_raw(Statement::from_string(
        DatabaseBackend::Sqlite,
        format!("PRAGMA table_info(\"{table}\")").as_str(),
    ))
    .await
    .unwrap()
    .iter()
    .map(|row| {
        (
            row.try_get::<String>("", "name").unwrap(),
            row.try_get::<String>("", "type").unwrap(),
        )
    })
    .collect()
}

async fn migrated() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    db
}

/// Every `f64` column in every entity, checked against the schema the
/// migrations build. `grandboss_data` is the one that was wrong; the other two
/// float columns (`npc_respawns`, `mdt_history`) were always `double`, and this
/// keeps a fourth from being added as `decimal`.
#[tokio::test]
async fn f64_columns_have_real_affinity() {
    let db = migrated().await;

    macro_rules! check_all {
        ($($module:ident),* $(,)?) => {{
            let mut checked = 0;
            $(
                let entity = models::entity::$module::Entity;
                let table = entity.table_name();
                let doubles = double_columns(entity);
                if !doubles.is_empty() {
                    let types = declared_types(&db, table).await;
                    for name in doubles {
                        let declared = types
                            .iter()
                            .find(|(c, _)| *c == name)
                            .unwrap_or_else(|| panic!("`{table}` has no column `{name}`"));
                        assert_eq!(
                            affinity(&declared.1),
                            "REAL",
                            "`{table}`.`{name}` is `f64` in the entity but declared `{}`, \
                             which gives it {} affinity — an integral value would be stored \
                             as INTEGER and fail to decode",
                            declared.1,
                            affinity(&declared.1),
                        );
                        checked += 1;
                    }
                }
            )*
            checked
        }};
    }

    let checked = check_all!(grandboss_data, npc_respawns, mdt_history);
    assert_eq!(checked, 5, "expected 5 f64 columns across the three tables");
}

/// The round trip that broke: a grand boss killed by players has `0.0` written
/// to `currentHP`/`currentMP`, and reading the table back must still work.
#[tokio::test]
async fn a_dead_grand_boss_row_still_loads() {
    use models::entity::grandboss_data;

    let db = migrated().await;

    // Queen Ant, alive and wounded to a fractional HP.
    grandboss_data::ActiveModel {
        boss_id: Set(29001),
        loc_x: Set(-21610),
        loc_y: Set(181594),
        loc_z: Set(-5734),
        heading: Set(0),
        respawn_time: Set(0),
        current_hp: Set(229_898.48),
        current_mp: Set(667.776),
        status: Set(0),
    }
    .insert(&db)
    .await
    .unwrap();

    // Orfen, just killed: `on_grand_boss_killed` zeroes the stored vitals.
    // Under NUMERIC affinity this row is what poisoned the whole `SELECT`.
    grandboss_data::ActiveModel {
        boss_id: Set(29014),
        loc_x: Set(55024),
        loc_y: Set(17368),
        loc_z: Set(-5412),
        heading: Set(10126),
        respawn_time: Set(1_700_000_000_000),
        current_hp: Set(0.0),
        current_mp: Set(0.0),
        status: Set(1),
    }
    .insert(&db)
    .await
    .unwrap();

    // Baium's stock HP is an exact integer even before anyone touches him, so
    // a freshly installed database hits this without a single boss dying.
    db.execute_unprepared(
        "INSERT INTO `grandboss_data` VALUES (29020, 116033, 17447, 10107, -25348, 0, 4068372, 39960, 0)",
    )
    .await
    .unwrap();

    let rows = grandboss_data::Entity::find()
        .all(&db)
        .await
        .expect("the whole table must decode, integral vitals included");
    assert_eq!(
        rows.len(),
        3,
        "every boss comes back, not just the wounded one"
    );

    let orfen = rows.iter().find(|r| r.boss_id == 29014).unwrap();
    assert_eq!(orfen.current_hp, 0.0);
    let baium = rows.iter().find(|r| r.boss_id == 29020).unwrap();
    assert_eq!(baium.current_hp, 4_068_372.0);
}
