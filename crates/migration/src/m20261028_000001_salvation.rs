//! Salvation's game schema on top of the Interlude baseline
//! (`L2J_Mobius_05.0_Salvation/dist/sql/game`).
//!
//! - **Six new tables:** `character_offline_play`,
//!   `character_offline_play_group`, `character_transmogs`,
//!   `item_special_abilities`, `mods_wedding` and `underground_coliseum`.
//!   They are created now so the schema matches. Their systems arrive later
//!   (ensoul in S10, the others when they come into scope).
//! - **`buffer_schemes.skills`** widens from `VARCHAR(200)` to `VARCHAR(512)`.
//! - **`character_shortcuts.level`** becomes `NOT NULL DEFAULT 0`. A `NULL`
//!   already stored is written as 0, which is what Java's `rs.getInt` read it
//!   as anyway.
//! - **`grandboss_data`** gets Salvation's 13 seed rows, `INSERT IGNORE`-style.
//!   A row already present (a live boss's state) is kept as it is. Interlude's
//!   extra rows (Zaken 29022, Dr. Chaos 25512) are left alone. They are inert,
//!   because only an id with a `GrandBoss.ini` window is ever spawned.
//!
//! `olympiad_fights.classed` needs no change: the baseline already declares it
//! `tinyint NOT NULL DEFAULT 0`, as Salvation does.
//!
//! SQLite cannot alter a column, so both column changes use the rebuild from
//! [`super::m20260908_000001_grandboss_real_hp`]. Each is skipped when the
//! column already has the new shape, which keeps `up` idempotent.
//!
//! `down` drops the six tables only. The wider column, the `NOT NULL` and the
//! seed rows are all readable by the old code, and narrowing `skills` again
//! could truncate a stored scheme.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

use crate::dialect::{dflt, ty};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Dropped in reverse order by `down`.
const NEW_TABLES: &[&str] = &[
    "character_offline_play",
    "character_offline_play_group",
    "character_transmogs",
    "item_special_abilities",
    "mods_wedding",
    "underground_coliseum",
];

/// `grandboss_data.sql`'s seed: `(boss_id, x, y, z, heading, hp, mp)`.
const GRAND_BOSSES: &[(i32, i32, i32, i32, i32, f64, f64)] = &[
    (29001, -21610, 181594, -5734, 0, 229898.48, 667.776), // Queen Ant
    (29006, 17726, 108915, -6480, 0, 622493.58388, 3793.536), // Core
    (29014, 55024, 17368, -5412, 10126, 622493.58388, 3793.536), // Orfen
    (29020, 116033, 17447, 10107, -25348, 4068372.0, 39960.0), // Baium
    (29068, 185708, 114298, -8221, 32768, 62802301.0, 1998000.0), // Antharas
    (29028, -105200, -253104, -15264, 0, 62041918.0, 2248572.0), // Valakas
    (29240, 0, 0, 0, 0, 288282589.0, 47100.0),             // Lindvior
    (29197, 81208, -182095, -9895, 0, 512402016.0, 47100.0), // Trasken
    (29118, 0, 0, 0, 0, 4109288.0, 1220547.0),             // Beleth
    (25286, 185080, -12613, -5499, 16550, 556345880.0, 86847.0), // Anakim
    (25283, 185062, -9605, -5499, 15640, 486021997.0, 79600.0), // Lilith
    (26124, 0, 0, 0, 0, 13945521.0, 50920.0),              // Kelbim
    (29305, 0, 0, 0, 0, 589355368.0, 51696.0),             // Helios
];

fn col(name: &str) -> Alias {
    Alias::new(name)
}

async fn create_new_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let int_zero = |name: &str, declared: &str| {
        ColumnDef::new(col(name))
            .custom(ty(manager, declared))
            .not_null()
            .default(dflt(manager, "0"))
            .to_owned()
    };

    manager
        .create_table(
            Table::create()
                .table(col("character_offline_play"))
                .if_not_exists()
                .col(int_zero("charId", "INT"))
                .col(int_zero("type", "TINYINT"))
                .col(int_zero("id", "INT"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("character_offline_play_charId")
                .table(col("character_offline_play"))
                .col(col("charId"))
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(col("character_offline_play_group"))
                .if_not_exists()
                .col(int_zero("leaderId", "INT"))
                .col(int_zero("charId", "INT"))
                .col(int_zero("type", "TINYINT"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("character_offline_play_group_leaderId")
                .table(col("character_offline_play_group"))
                .col(col("leaderId"))
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(col("character_transmogs"))
                .if_not_exists()
                .col(
                    ColumnDef::new(col("owner"))
                        .custom(ty(manager, "VARCHAR(45)"))
                        .not_null(),
                )
                .col(int_zero("itemId", "INT"))
                .primary_key(Index::create().col(col("owner")).col(col("itemId")))
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(col("item_special_abilities"))
                .if_not_exists()
                .col(
                    ColumnDef::new(col("objectId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(col("type"))
                        .custom(ty(manager, "TINYINT"))
                        .not_null()
                        .default(dflt(manager, "1")),
                )
                .col(
                    ColumnDef::new(col("optionId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(int_zero("position", "TINYINT"))
                .primary_key(Index::create().col(col("objectId")).col(col("optionId")))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_objectId")
                .table(col("item_special_abilities"))
                .col(col("objectId"))
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(col("mods_wedding"))
                .if_not_exists()
                .col(
                    ColumnDef::new(col("id"))
                        .integer()
                        .primary_key()
                        .auto_increment(),
                )
                .col(
                    ColumnDef::new(col("player1Id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(col("player2Id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(col("married"))
                        .custom(ty(manager, "VARCHAR(5)"))
                        .null(),
                )
                .col(
                    ColumnDef::new(col("affianceDate"))
                        .custom(ty(manager, "decimal(20,0)"))
                        .null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(col("weddingDate"))
                        .custom(ty(manager, "decimal(20,0)"))
                        .null()
                        .default(dflt(manager, "'0'")),
                )
                .to_owned(),
        )
        .await?;
    for player in ["player1Id", "player2Id"] {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(format!("mods_wedding_{player}"))
                    .table(col("mods_wedding"))
                    .col(col(player))
                    .to_owned(),
            )
            .await?;
    }

    manager
        .create_table(
            Table::create()
                .table(col("underground_coliseum"))
                .if_not_exists()
                .col(
                    ColumnDef::new(col("arenaId"))
                        .custom(ty(manager, "INT"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(col("leader"))
                        .custom(ty(manager, "VARCHAR(16)"))
                        .not_null(),
                )
                .col(int_zero("wins", "INT"))
                .primary_key(Index::create().col(col("arenaId")))
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `(declared type, NOT NULL)` of `table.column` on SQLite, or `None` if
/// the table or the column is not there.
async fn sqlite_column(
    manager: &SchemaManager<'_>,
    table: &str,
    column: &str,
) -> Result<Option<(String, bool)>, DbErr> {
    let rows = manager
        .get_connection()
        .query_all_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("PRAGMA table_info(\"{table}\")"),
        ))
        .await?;
    for row in rows {
        if row.try_get::<String>("", "name")? == column {
            return Ok(Some((
                row.try_get::<String>("", "type")?,
                row.try_get::<i32>("", "notnull")? == 1,
            )));
        }
    }
    Ok(None)
}

/// Swap `{table}_new` into place for `table`, copying `columns` with each
/// `select` expression.
async fn swap_in(
    manager: &SchemaManager<'_>,
    table: &str,
    columns: &[&str],
    select: impl Fn(&str) -> String,
) -> Result<(), DbErr> {
    let new = format!("{table}_new");
    let cols = columns
        .iter()
        .map(|c| format!("`{c}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let selected = columns
        .iter()
        .map(|c| select(c))
        .collect::<Vec<_>>()
        .join(", ");
    manager
        .get_connection()
        .execute_unprepared(&format!(
            "INSERT INTO `{new}` ({cols}) SELECT {selected} FROM `{table}`"
        ))
        .await?;
    manager
        .drop_table(Table::drop().table(col(table)).to_owned())
        .await?;
    manager
        .rename_table(Table::rename().table(col(&new), col(table)).to_owned())
        .await?;
    Ok(())
}

async fn widen_scheme_skills(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if manager.get_database_backend() == DatabaseBackend::Postgres {
        return manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE \"buffer_schemes\" ALTER COLUMN \"skills\" TYPE VARCHAR(512)",
            )
            .await
            .map(|_| ());
    }
    match sqlite_column(manager, "buffer_schemes", "skills").await? {
        Some((ty, _)) if !ty.eq_ignore_ascii_case("VARCHAR(512)") => {}
        _ => return Ok(()),
    }
    manager
        .create_table(
            Table::create()
                .table(col("buffer_schemes_new"))
                .col(
                    ColumnDef::new(col("object_id"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(col("scheme_name"))
                        .custom(ty(manager, "VARCHAR(16)"))
                        .not_null()
                        .default(dflt(manager, "'default'")),
                )
                .col(
                    ColumnDef::new(col("skills"))
                        .custom(ty(manager, "VARCHAR(512)"))
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(col("object_id"))
                        .col(col("scheme_name")),
                )
                .to_owned(),
        )
        .await?;
    swap_in(
        manager,
        "buffer_schemes",
        &["object_id", "scheme_name", "skills"],
        |c| format!("`{c}`"),
    )
    .await
}

async fn shortcut_level_not_null(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if manager.get_database_backend() == DatabaseBackend::Postgres {
        let db = manager.get_connection();
        for sql in [
            "UPDATE \"character_shortcuts\" SET \"level\" = 0 WHERE \"level\" IS NULL",
            "ALTER TABLE \"character_shortcuts\" ALTER COLUMN \"level\" SET DEFAULT 0",
            "ALTER TABLE \"character_shortcuts\" ALTER COLUMN \"level\" SET NOT NULL",
        ] {
            db.execute_unprepared(sql).await?;
        }
        return Ok(());
    }
    match sqlite_column(manager, "character_shortcuts", "level").await? {
        Some((_, false)) => {}
        _ => return Ok(()),
    }
    let decimal = |name: &str, declared: &str| {
        ColumnDef::new(col(name))
            .custom(ty(manager, declared))
            .not_null()
            .default(dflt(manager, "0"))
            .to_owned()
    };
    manager
        .create_table(
            Table::create()
                .table(col("character_shortcuts_new"))
                .col(decimal("charId", "INT"))
                .col(decimal("slot", "decimal(3)"))
                .col(decimal("page", "decimal(3)"))
                .col(
                    ColumnDef::new(col("type"))
                        .custom(ty(manager, "decimal(3)"))
                        .null(),
                )
                .col(
                    ColumnDef::new(col("shortcut_id"))
                        .custom(ty(manager, "decimal(16)"))
                        .null(),
                )
                .col(decimal("level", "MEDIUMINT"))
                .col(
                    ColumnDef::new(col("sub_level"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(col("class_index"))
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .primary_key(
                    Index::create()
                        .col(col("charId"))
                        .col(col("slot"))
                        .col(col("page"))
                        .col(col("class_index")),
                )
                .to_owned(),
        )
        .await?;
    swap_in(
        manager,
        "character_shortcuts",
        &[
            "charId",
            "slot",
            "page",
            "type",
            "shortcut_id",
            "level",
            "sub_level",
            "class_index",
        ],
        |c| match c {
            "level" => "COALESCE(`level`, 0)".to_string(),
            _ => format!("`{c}`"),
        },
    )
    .await?;
    // The rebuilt table lost the baseline's index along with the old table.
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("shortcut_id")
                .table(col("character_shortcuts"))
                .col(col("shortcut_id"))
                .to_owned(),
        )
        .await?;
    Ok(())
}

async fn seed_grand_bosses(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let mut insert = Query::insert();
    insert.into_table(col("grandboss_data")).columns([
        col("boss_id"),
        col("loc_x"),
        col("loc_y"),
        col("loc_z"),
        col("heading"),
        col("currentHP"),
        col("currentMP"),
    ]);
    for &(id, x, y, z, heading, hp, mp) in GRAND_BOSSES {
        insert.values_panic([
            id.into(),
            x.into(),
            y.into(),
            z.into(),
            heading.into(),
            hp.into(),
            mp.into(),
        ]);
    }
    insert.on_conflict(OnConflict::column(col("boss_id")).do_nothing().to_owned());
    manager.exec_stmt(insert).await
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// One transaction on every backend. On SQLite, sea-orm-migration would
    /// otherwise run each statement on whichever pooled connection is free,
    /// and a connection that hasn't seen the `DROP` fails the rename
    /// ("there is already another table … named buffer_schemes"). The
    /// transaction also makes each rebuild atomic, so a crash mid-copy
    /// cannot lose the table.
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_new_tables(manager).await?;
        widen_scheme_skills(manager).await?;
        shortcut_level_not_null(manager).await?;
        seed_grand_bosses(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in NEW_TABLES.iter().rev() {
            manager
                .drop_table(Table::drop().table(col(table)).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
