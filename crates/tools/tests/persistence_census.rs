//! **Persistence census** — what Java stores that this port does not.
//!
//! The parity axes before this one compare *behaviour while the server is up*.
//! This one is about what survives it going down, and it is a set-difference
//! rather than a reading exercise: every Java `INSERT`/`UPDATE` names its table
//! and its columns, and the port's writes all live under
//! `crates/gameserver/src/db/`. Subtracting one from the other gives a residue,
//! and the residue is the point — a table or column that is *deliberately* not
//! persisted has to be listed here with the reason, and one that quietly starts
//! being persisted fails this test until somebody takes it off the list.
//!
//! The same shape as `deferral_markers_match_the_recorded_inventory`: an
//! inventory as an assertion rather than a paragraph.
//!
//! **The Java side of the difference is not re-derived here.** It comes from a
//! scan of the Java tree, which lives outside this repo; the command is in
//! `docs/PORTING_STATUS.md`. What this test holds is the *port* side: that
//! everything listed below is still absent.

use std::path::Path;

/// Tables Java writes and the game server never does, each with why. The port
/// is the login server's peer for `accounts`/`gameservers`, which *are* written
/// (through `crates/models/src/repo`), so they are not residue.
const UNWRITTEN_TABLES: &[(&str, &str)] = &[
    ("airships", "Gracia airships — off-chronicle"),
    (
        "announcements",
        "server announcements are config-driven here, not stored",
    ),
    ("character_contacts", "the contact list is post-Interlude"),
    (
        "character_item_reuse_save",
        "item reuse rides the skill-reuse map here and persists through \
         character_skills_save; Java keeps a second, item-keyed table",
    ),
    ("character_mentees", "mentoring is post-Interlude"),
    (
        "character_pet_skills_save",
        "a pet's own skill cooldowns; pets persist, their reuses do not",
    ),
    (
        "character_premium_items",
        "premium item delivery is post-Interlude",
    ),
    (
        "character_tpbookmark",
        "teleport bookmarks — the handler is null in this build (row 15)",
    ),
    ("clan_variables", "the port has no clan variable store"),
    ("commission_items", "the commission house is post-Interlude"),
    (
        "fort",
        "fortresses are off-chronicle per the ROADMAP scope gate",
    ),
    ("fort_doorupgrade", "fortresses — as above"),
    ("fortsiege_clans", "fortresses — as above"),
    (
        "forums",
        "the BBS forum tables; this dist's board is the custom one",
    ),
    (
        "global_tasks",
        "DailyTaskManager's per-task stamps; the port keeps the same state in \
         global_variables",
    ),
    ("item_variables", "the port has no per-item variable store"),
    ("mods_wedding", "the wedding mod is custom content"),
    (
        "olympiad_fights",
        "per-match history rows; the port keeps no fight log",
    ),
    (
        "party_matching_history",
        "party-matching history is post-Interlude",
    ),
    ("posts", "BBS forums — as above"),
    ("topic", "BBS forums — as above"),
];

/// Columns of tables the port *does* write, that it leaves alone.
const UNWRITTEN_COLUMNS: &[(&str, &str, &str)] = &[
    (
        "characters",
        "bookmarkslot",
        "teleport bookmarks — not portable",
    ),
    (
        "characters",
        "cancraft",
        "dead in Java too: written by the UPDATE, read by nothing",
    ),
    (
        "characters",
        "faction",
        "the faction system is post-Interlude",
    ),
    ("characters", "fame", "fame points arrive after Interlude"),
    (
        "characters",
        "language",
        "client language selection is post-Interlude",
    ),
    (
        "characters",
        "onlinetime",
        "feeds ClanRewardType.MEMBERS_ONLINE, whose tiers in \
         config/ClanReward.xml grant skills 55168-55171 — none of which exists \
         in this dist's skill data, so the bonus resolves to nothing",
    ),
    (
        "characters",
        "title_color",
        "recomputed at login from the access level and the PvP title ladder \
         rather than stored",
    ),
    (
        "characters",
        "wantspeace",
        "dead in Java too: restored and read, but nothing ever sets it",
    ),
    (
        "clan_data",
        "auction_bid_at",
        "the port tracks clan-hall bids in clanhall_auctions_bidders",
    ),
    (
        "messages",
        "itemId",
        "the commission-mail item preview — post-Interlude",
    ),
    ("messages", "enchantLvl", "commission mail — as above"),
    ("messages", "elementals", "commission mail — as above"),
];

/// Every `.rs` under the game server's persistence layer, which is where all of
/// its writes live.
fn db_layer_sources() -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../gameserver/src/db");
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(std::fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    assert!(!out.is_empty(), "found no db-layer sources to scan");
    out
}

fn to_camel(column: &str) -> String {
    column
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

/// The recorded residue is still residue: nothing on either list is written.
#[test]
fn unpersisted_state_matches_the_recorded_inventory() {
    let sources = db_layer_sources();
    let mentions = |needle: &str| sources.iter().any(|s| s.contains(needle));

    for (table, why) in UNWRITTEN_TABLES {
        let needle = format!("entity::{table}::");
        assert!(
            !mentions(&needle),
            "`{table}` is on the unwritten list ({why}) but the db layer now \
             references it — persist it and take it off the list, or keep the \
             list honest"
        );
    }

    for (table, column, why) in UNWRITTEN_COLUMNS {
        // sea-orm gives each column both spellings: the `Column::` variant and
        // the `ActiveModel` field. Neither may appear.
        let camel = format!("Column::{}", to_camel(&column.to_lowercase()));
        let field = format!("{}: Set(", column.to_lowercase());
        assert!(
            !mentions(&camel) && !mentions(&field),
            "`{table}.{column}` is on the unwritten list ({why}) but the db \
             layer now writes it — take it off the list"
        );
    }
}

/// The inventory is a set, not a bag: a duplicate row would let one entry go
/// stale behind another.
#[test]
fn the_inventory_has_no_duplicates() {
    let mut tables: Vec<&str> = UNWRITTEN_TABLES.iter().map(|&(t, _)| t).collect();
    tables.sort_unstable();
    let before = tables.len();
    tables.dedup();
    assert_eq!(before, tables.len(), "duplicate table in the inventory");

    let mut columns: Vec<(&str, &str)> =
        UNWRITTEN_COLUMNS.iter().map(|&(t, c, _)| (t, c)).collect();
    columns.sort_unstable();
    let before = columns.len();
    columns.dedup();
    assert_eq!(before, columns.len(), "duplicate column in the inventory");
}
