#!/usr/bin/env python3
"""Post-process `sea-orm-cli generate entity` output for this schema.

The generator infers Rust types from SQLite's *storage classes*, which throws
away everything the dist DDL actually says: every integer column comes back as
`i64`, `TINYINT` as `i8`, `DECIMAL(20,0)` as `rust_decimal::Decimal`, and a
`VARBINARY` blob as an `ignore`d `String`. This script re-types every field from
the declared column type instead, so the entities line up with the `i32`-shaped
game code (see docs/PLAN_ORM_MIGRATION.md §8).

Usage:
    python3 tools/normalize_entities.py <schema.db> <generated-dir> <out-dir>

`schema.db` is a throwaway SQLite database built from the authoritative DDL —
see docs/DATABASE.md for the two commands that produce it.
"""

import re
import sqlite3
import sys
from pathlib import Path

# Columns the DDL declares as integers but that actually hold floating-point
# values: Java writes doubles into them and SQLite's type affinity keeps the
# fraction. They get `LooseF64`, which accepts either storage class — a plain
# `f64` field fails to decode the rows SQLite filed as INTEGER (most of them),
# and an `i32` field would truncate a wounded character's HP on every load.
# `npc_respawns` and `grandboss_data` are declared `double` / `decimal(30,15)`,
# so SQLite's REAL affinity normalises them and they need no override.
FLOAT_OVERRIDES = {
    ("characters", "curHp"),
    ("characters", "curCp"),
    ("characters", "curMp"),
    ("pets", "curHp"),
    ("pets", "curMp"),
}

BINARY_OVERRIDES = {("crests", "data")}

# Columns the DDL declares 32-bit that the game code reads as `i64`. SQLite
# stores whatever fits regardless of the declared type, so a lottery prize or a
# race bet that grew past 2^31 would decode-fail against an `i32` field. Where
# the Rust side already treats a column as 64-bit, the entity follows it.
INT64_OVERRIDES = {
    ("lottery", "prize"),
    ("lottery", "newprize"),
    ("lottery", "prize1"),
    ("lottery", "prize2"),
    ("lottery", "prize3"),
    ("mdt_bets", "bet"),
}

# Tables whose only candidate key is nullable, so the entity keys off SQLite's
# implicit `rowid` instead. `accounts.login` is NULL on a dashboard master
# account (PLAN_DASHBOARD.md §15) and therefore cannot be a primary key; the
# admin list already orders by `rowid` for the same reason.
ROWID_PK = {"accounts"}

ROWID_FIELD = """    #[sea_orm(primary_key, auto_increment = true)]
    pub rowid: i32,"""

# SeaORM requires a primary key on every entity; 16 dist tables declare none.
# These are logical keys — they change nothing about the schema (migrations are
# transcribed from the DDL, not derived from entities), they only give the ORM
# something to address a row by.
MISSING_PK = {
    "accounts_ipauth": ["login", "ip"],
    "character_mentees": ["charId", "mentorId"],
    "character_offline_trade_items": ["charId", "item"],
    "character_premium_items": ["charId", "itemNum"],
    "character_reco_bonus": ["charId"],
    "character_variables": ["charId", "var"],
    "clan_variables": ["clanId", "var"],
    "custom_mail": ["date", "receiver"],
    "heroes_diary": ["charId", "time"],
    "item_variables": ["id", "var"],
    "olympiad_fights": ["charOneId", "charTwoId", "start"],
    "petition_feedback": ["charName", "gmName", "date"],
    "pledge_recruit": ["clan_id"],
    "pledge_waiting_list": ["char_id"],
    "posts": ["post_id", "post_topic_id", "post_forum_id"],
    "topic": ["topic_id", "topic_forum_id"],
}


def rust_type(decl: str) -> str:
    """Declared SQL type -> Rust scalar."""
    t = decl.strip().upper()
    base = re.sub(r"\(.*\)", "", t).strip()

    if base in ("DOUBLE", "FLOAT", "REAL"):
        return "f64"
    if base == "DECIMAL":
        m = re.match(r"DECIMAL\((\d+)(?:\s*,\s*(\d+))?\)", t)
        precision, scale = (int(m.group(1)), int(m.group(2) or 0)) if m else (10, 0)
        if scale > 0:
            return "f64"
        return "i64" if precision >= 10 else "i32"
    if base == "BIGINT":
        return "i64"
    if base in ("INT", "INTEGER", "MEDIUMINT", "SMALLINT", "TINYINT", "BOOLEAN"):
        return "i32"
    if base.startswith("VARBINARY") or base in ("BLOB", "BINARY"):
        return "Vec<u8>"
    # VARCHAR/CHAR/TEXT/TINYTEXT and the date-ish columns, which the game code
    # reads as the raw string SQLite stores.
    return "String"


def columns(db: sqlite3.Connection, table: str) -> dict[str, str]:
    return {r[1]: r[2] for r in db.execute(f'PRAGMA table_info("{table}")')}


def autoincrements(db: sqlite3.Connection, table: str) -> bool:
    """True when the table's key is `INTEGER PRIMARY KEY AUTOINCREMENT`.

    The generator always writes `auto_increment = false`, which would make the
    ORM send an explicit NULL-less id on insert for the two tables that expect
    SQLite to allocate one.
    """
    row = db.execute(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name=?", (table,)
    ).fetchone()
    return bool(row) and "AUTOINCREMENT" in (row[0] or "").upper()


FIELD = re.compile(r"^(\s*)pub (r#)?([a-z0-9_]+): (Option<)?([^,]+?)(>)?,$")
COLUMN_NAME = re.compile(r'column_name\s*=\s*"([^"]+)"')


def normalize(path: Path, db: sqlite3.Connection) -> str:
    src = path.read_text()
    m = re.search(r'table_name\s*=\s*"([^"]+)"', src)
    if not m:
        return src
    table = m.group(1)
    cols = columns(db, table)
    missing_pk = MISSING_PK.get(table, [])
    auto_inc = autoincrements(db, table)

    out: list[str] = []
    pending_attr: list[str] = []
    rowid_pending = table in ROWID_PK
    has_float = False
    lines = src.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        field = FIELD.match(line)
        if not field:
            # Buffer attribute lines (possibly multi-line) so a field can rewrite
            # or drop its own attributes.
            if line.strip().startswith("#[sea_orm("):
                attr = [line]
                while not line.rstrip().endswith(")]"):
                    i += 1
                    line = lines[i]
                    attr.append(line)
                pending_attr = attr
                i += 1
                continue
            out.extend(pending_attr)
            pending_attr = []
            out.append(line)
            i += 1
            continue

        indent, raw, name, opt, _ty, _close = field.groups()
        attr_text = "\n".join(pending_attr)
        col = m2.group(1) if (m2 := COLUMN_NAME.search(attr_text)) else name
        decl = cols.get(col)
        if decl is None:  # not a real column (shouldn't happen)
            out.extend(pending_attr)
            pending_attr = []
            out.append(line)
            i += 1
            continue

        if (table, col) in FLOAT_OVERRIDES:
            new_ty = "crate::value::LooseF64"
        elif (table, col) in INT64_OVERRIDES:
            new_ty = "i64"
        elif (table, col) in BINARY_OVERRIDES:
            new_ty = "Vec<u8>"
        else:
            new_ty = rust_type(decl)
        has_float |= new_ty in ("f64", "crate::value::LooseF64")

        keep: list[str] = []
        is_pk = "primary_key" in attr_text
        if m2:
            keep.append(f'column_name = "{col}"')
        if is_pk:
            keep.append("primary_key")
            keep.append(
                "auto_increment = true" if auto_inc else "auto_increment = false"
            )
        if new_ty == "Vec<u8>":
            keep.append('column_type = "Binary(2176)"')
        if col in missing_pk:
            keep.append("primary_key")
            keep.append("auto_increment = false")

        if rowid_pending:
            out.append(ROWID_FIELD)
            rowid_pending = False
        if keep:
            out.append(f"{indent}#[sea_orm({', '.join(keep)})]")
        # A primary-key column is never NULL in practice, and SeaORM cannot
        # build a key type out of `Option<_>`. SQLite lets a non-INTEGER PK
        # column be nullable, which is how 16 dist tables end up here.
        nullable = bool(opt) and not (is_pk or col in missing_pk)
        ty = f"Option<{new_ty}>" if nullable else new_ty
        out.append(f"{indent}pub {raw or ''}{name}: {ty},")
        pending_attr = []
        i += 1

    out.extend(pending_attr)
    text = "\n".join(out) + "\n"
    if has_float:
        # `Eq` cannot be derived alongside an `f64` field.
        text = text.replace(
            "#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]",
            "#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]",
        )
    return text


def main() -> None:
    schema, gen_dir, out_dir = (Path(a) for a in sys.argv[1:4])
    db = sqlite3.connect(f"file:{schema}?mode=ro", uri=True)
    out_dir.mkdir(parents=True, exist_ok=True)
    for path in sorted(gen_dir.glob("*.rs")):
        (out_dir / path.name).write_text(
            path.read_text() if path.name in ("mod.rs", "prelude.rs") else normalize(path, db)
        )
    print(f"normalized {len(list(gen_dir.glob('*.rs')))} files into {out_dir}")


if __name__ == "__main__":
    main()
