//! `General.ini`'s `Custom*Load` family and the two `HtmCache.loadFile`
//! content keys.
//!
//! The loader half is asserted against the **real dist** rather than fixtures:
//! the whole question is what the shipped datapack contains under `custom/`,
//! which a synthetic tree cannot answer. That is also how the gap survived —
//! `tvt_tests` registers the manager template by hand, so nothing ever asked
//! whether the loader supplied it.

use crate::data::htm_cache::{HtmlSettings, strip_htm_with};

/// The TvT event manager, defined **only** in `stats/npcs/custom/tvt_event.xml`.
const TVT_MANAGER: i32 = 70010;
/// Kadmos the Noblesse Master, defined only in `stats/npcs/custom/`.
const NOBLESS_MASTER: i32 = 1_003_000;

/// **The bug this cluster was opened on.** `NpcData` read `stats/npcs` without
/// its `custom/` subdirectory, so 14 templates never loaded — and two of them
/// belong to features the port implements.
///
/// `spawn_npc_at` returns `None` without a template, so `//event_start TvT`
/// spawned no manager at all and `//spawn 1003000` failed. Asserted against the
/// real datapack, because a fixture that inserts the template is exactly what
/// hid this.
#[test]
fn the_custom_npc_directory_is_loaded() {
    let npcs = crate::data::dist::npcs();
    assert!(
        npcs.get(TVT_MANAGER).is_some(),
        "the TvT event manager must have a template, or the event spawns nothing"
    );
    assert!(
        npcs.get(NOBLESS_MASTER).is_some(),
        "Kadmos must be reachable through //spawn"
    );
    // …and the main tree is still there.
    assert!(npcs.get(31324).is_some(), "a retail NPC still loads");
}

/// `CustomNpcData = False` leaves the retail tree intact and drops only the
/// overlay — the two halves are separate directories, not a merged read.
#[test]
fn custom_npc_data_off_drops_only_the_overlay() {
    let off = crate::data::NpcData::load_from_with(crate::data::DIST_GAME, false);
    assert!(
        off.get(TVT_MANAGER).is_none(),
        "the overlay is gone with the key off"
    );
    assert!(off.get(31324).is_some(), "the retail tree is untouched");
}

/// `CustomSkillsLoad` — one file here, and it belongs to the same event as the
/// NPC above: Ghost Walking (100000) is TvT's spawn-protection buff.
#[test]
fn the_custom_skill_directory_is_loaded() {
    let skills = crate::data::dist::skills();
    assert!(
        skills.get(100_000, 1).is_some(),
        "Ghost Walking is TvT's spawn protection and lives in skills/custom/"
    );
}

/// `CheckHtmlEncoding` is a **diagnostic**: a non-ASCII file still loads and is
/// still served. Pinned because the obvious wrong implementation — refusing the
/// file — would blank every dialog an operator localised.
#[test]
fn the_encoding_check_does_not_change_the_content() {
    let on = HtmlSettings {
        check_encoding: true,
    };
    let html = "<html><body>Grüße</body></html>";
    assert_eq!(
        strip_htm_with(html, on, "dist/game/data/html/x.htm"),
        html,
        "the file is served unchanged; the key only logs"
    );
}
