#!/usr/bin/env python3
"""Transcribe the authoritative dist DDL into a SeaORM baseline migration.

Reads a throwaway SQLite database built from
`dist/db_installer/sql/sqlite/**` and emits `Table::create()` /
`Index::create()` statements — one Rust function per table, column types kept
verbatim through `ColumnDef::custom` so the migrated schema is byte-identical
to the one the Java installer produces (docs/PLAN_ORM_MIGRATION.md §6).

The output is committed; this script exists so the transcription is
reproducible and so a dist DDL change can be re-applied mechanically. The
`dist_parity` test is what proves the result correct.

Usage:
    python3 tools/gen_migrations.py <schema.db> <out.rs> <migration-name> <table>...
"""

import re
import sqlite3
import sys
from pathlib import Path

HEADER = '''//! {title}
//!
//! Transcribed from `dist/db_installer/sql/sqlite/**` by
//! `tools/gen_migrations.py` — do not hand-edit. Column types are passed
//! through verbatim (`MEDIUMINT`, `TINYINT`, …) so the schema matches the one
//! the Java installer produces; `crates/migration/tests/dist_parity.rs` proves
//! it column by column.
//!
//! Every statement is `IF NOT EXISTS`, which is what lets `l2r-migrate up`
//! adopt the live production database: it records the migration as applied
//! without touching a single existing table.

use sea_orm_migration::prelude::*;{extra_imports}

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Dropped in reverse order by `down`.
const TABLES: &[&str] = &[
{table_list}
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {{
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {{
{up_calls}
        Ok(())
    }}

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {{
        for table in TABLES.iter().rev() {{
            manager
                .drop_table(Table::drop().table(Alias::new(*table)).if_exists().to_owned())
                .await?;
        }}
        Ok(())
    }}
}}
'''


def emit_column(col, autoincrement_pk: str | None, unique: set[str]) -> str:
    cid, name, decl, notnull, default, pk = col
    parts = [f'ColumnDef::new(Alias::new("{name}"))']

    if name == autoincrement_pk:
        # SQLite only allows AUTOINCREMENT on `INTEGER PRIMARY KEY`, which is
        # exactly what sea-query renders for integer + primary_key + auto.
        parts.append(".integer()")
        if notnull:
            # `INTEGER PRIMARY KEY` is implicitly NOT NULL in SQLite, but some
            # dist tables say so explicitly and PRAGMA reports the difference.
            parts.append(".not_null()")
        parts.append(".primary_key()")
        parts.append(".auto_increment()")
        return "                    " + "\n                    ".join(parts)

    parts.append(f'.custom(Alias::new("{decl}"))')
    parts.append(".not_null()" if notnull else ".null()")
    if default is not None:
        escaped = default.replace("\\", "\\\\").replace('"', '\\"')
        parts.append(f'.default(Expr::cust("{escaped}"))')
    if name in unique:
        parts.append(".unique_key()")
    return "                    " + "\n                    ".join(parts)


def emit_table(db: sqlite3.Connection, table: str) -> str:
    cols = list(db.execute(f'PRAGMA table_info("{table}")'))
    create_sql = db.execute(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name=?", (table,)
    ).fetchone()[0]
    auto = "AUTOINCREMENT" in create_sql.upper()
    pk_cols = [c[1] for c in sorted((c for c in cols if c[5]), key=lambda c: c[5])]
    auto_pk = pk_cols[0] if auto and len(pk_cols) == 1 else None

    # A UNIQUE column constraint shows up as an index SQLite created itself
    # (origin 'u'). Re-emit it as `.unique_key()` on the column so the rebuilt
    # table carries the constraint rather than a stray index.
    unique_cols: set[str] = set()
    for row in db.execute(f'PRAGMA index_list("{table}")'):
        if row[3] != "u":
            continue
        idx_cols = [r[2] for r in db.execute(f'PRAGMA index_info("{row[1]}")')]
        if len(idx_cols) == 1:
            unique_cols.add(idx_cols[0])
        else:
            raise SystemExit(
                f"{table}: multi-column UNIQUE constraint needs hand-written support"
            )

    body = [
        "    manager",
        "        .create_table(",
        "            Table::create()",
        f'                .table(Alias::new("{table}"))',
        "                .if_not_exists()",
    ]
    for col in cols:
        body.append("                .col(")
        body.append(emit_column(col, auto_pk, unique_cols))
        body.append("                )")
    if pk_cols and not auto_pk:
        key = "".join(f'.col(Alias::new("{c}"))' for c in pk_cols)
        body.append(f"                .primary_key(Index::create(){key})")
    body.append("                .to_owned(),")
    body.append("        )")
    body.append("        .await?;")

    # Explicit indexes only: `origin = 'c'`. The ones SQLite creates itself for
    # PRIMARY KEY / UNIQUE constraints come with the table.
    for idx_name, unique, origin, partial in (
        (r[1], r[2], r[3], r[4]) for r in db.execute(f'PRAGMA index_list("{table}")')
    ):
        if origin != "c":
            continue
        if partial:
            # sea-query cannot express `WHERE …` (or `COLLATE …`) on an index;
            # the dist statement goes through as written.
            sql = db.execute(
                "SELECT sql FROM sqlite_master WHERE type='index' AND name=?", (idx_name,)
            ).fetchone()[0]
            sql = sql.replace("CREATE UNIQUE INDEX ", "CREATE UNIQUE INDEX IF NOT EXISTS ", 1)
            sql = sql.replace("IF NOT EXISTS IF NOT EXISTS ", "IF NOT EXISTS ", 1)
            body.append("    manager")
            body.append("        .get_connection()")
            body.append(f'        .execute_unprepared("{sql.replace(chr(34), chr(92) + chr(34))}")')
            body.append("        .await?;")
            continue
        idx_cols = [r[2] for r in db.execute(f'PRAGMA index_info("{idx_name}")')]
        body.append("    manager")
        body.append("        .create_index(")
        body.append("            Index::create()")
        body.append("                .if_not_exists()")
        body.append(f'                .name("{idx_name}")')
        body.append(f'                .table(Alias::new("{table}"))')
        if unique:
            body.append("                .unique()")
        for c in idx_cols:
            body.append(f'                .col(Alias::new("{c}"))')
        body.append("                .to_owned(),")
        body.append("        )")
        body.append("        .await?;")

    fn = re.sub(r"\W", "_", table)
    return (
        f"/// `{table}`\n"
        f"async fn create_{fn}(manager: &SchemaManager<'_>) -> Result<(), DbErr> {{\n"
        + "\n".join(body)
        + "\n    Ok(())\n}\n"
    )


def main() -> None:
    schema, out, title, *tables = sys.argv[1:]
    db = sqlite3.connect(f"file:{schema}?mode=ro", uri=True)
    fns = [emit_table(db, t) for t in tables]
    needs_conn = any("execute_unprepared" in fn for fn in fns)
    text = HEADER.format(
        title=title,
        extra_imports=(
            "\nuse sea_orm_migration::sea_orm::ConnectionTrait;" if needs_conn else ""
        ),
        table_list="\n".join(f'    "{t}",' for t in tables),
        up_calls="\n".join(
            f"        create_{re.sub(r'\\W', '_', t)}(manager).await?;" for t in tables
        ),
    )
    text += "\n" + "\n".join(fns)
    Path(out).write_text(text)
    print(f"wrote {out}: {len(tables)} tables, {len(text.splitlines())} lines")


if __name__ == "__main__":
    main()
