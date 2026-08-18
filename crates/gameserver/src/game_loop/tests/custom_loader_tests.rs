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

/// `HideBypassRemoval` (**True** here) strips `-h` from exactly three bypasses
/// as the file is read. It is safe to do to content because the *client*
/// consumes the flag — Java's `RequestBypassToServer` never sees a `-h` — so
/// this changes what the chat box shows, not what the server parses.
///
/// The negative half matters as much as the positive: an unrelated `-h` bypass
/// must survive, or every other link in the datapack silently changes meaning.
#[test]
fn hide_bypass_removal_strips_only_the_three_named_bypasses() {
    let on = HtmlSettings {
        hide_bypass_removal: true,
        check_encoding: false,
    };
    let html = "<a action=\"bypass -h npc_%objectId%_Chat 1\">a</a>\
                <a action=\"bypass -h npc_%objectId%_Quest\">b</a>\
                <a action=\"bypass -h npc_%objectId%_showTeleports\">c</a>\
                <a action=\"bypass -h npc_%objectId%_Buy 3\">d</a>\
                <a action=\"bypass -h admin_help\">e</a>";
    let out = strip_htm_with(html, on, "");
    assert!(out.contains("bypass npc_%objectId%_Chat 1"));
    assert!(out.contains("bypass npc_%objectId%_Quest\""));
    assert!(out.contains("bypass npc_%objectId%_showTeleports"));
    assert!(
        out.contains("bypass -h npc_%objectId%_Buy 3"),
        "an unlisted npc bypass keeps its -h"
    );
    assert!(
        out.contains("bypass -h admin_help"),
        "an admin bypass keeps its -h"
    );

    // Off: nothing is touched.
    let off = HtmlSettings {
        hide_bypass_removal: false,
        check_encoding: false,
    };
    let out = strip_htm_with(html, off, "");
    assert_eq!(
        out.matches("bypass -h ").count(),
        5,
        "with the key off every -h survives"
    );
}

/// The replacements run **after** the comment/whitespace strip, as Java's do,
/// and the newline case is where that is observable.
///
/// Datapack authors wrap long tags across lines. Java removes `[\t\n]` first,
/// which *joins* `…_Chat` and ` 1` into the literal the replacement looks for;
/// run the replacement first and the same file keeps its `-h`. A commented-out
/// link cannot show this — it is deleted either way — which is what the first
/// version of this test asserted, and why it survived reversing the order.
#[test]
fn the_bypass_replacement_runs_after_the_whitespace_strip() {
    let on = HtmlSettings {
        hide_bypass_removal: true,
        check_encoding: false,
    };
    let wrapped = "<a action=\"bypass -h npc_%objectId%_Chat\n 1\">ask</a>";
    let out = strip_htm_with(wrapped, on, "");
    assert_eq!(
        out, "<a action=\"bypass npc_%objectId%_Chat 1\">ask</a>",
        "the newline is removed first, so the joined text matches and loses -h"
    );

    // And the plainer property: a commented-out link is gone entirely.
    let out = strip_htm_with(
        "<!-- <a action=\"bypass -h npc_%objectId%_Chat 9\">hidden</a> -->keep",
        on,
        "",
    );
    assert_eq!(out, "keep");
}

/// `CheckHtmlEncoding` is a **diagnostic**: a non-ASCII file still loads and is
/// still served. Pinned because the obvious wrong implementation — refusing the
/// file — would blank every dialog an operator localised.
#[test]
fn the_encoding_check_does_not_change_the_content() {
    let on = HtmlSettings {
        hide_bypass_removal: false,
        check_encoding: true,
    };
    let html = "<html><body>Grüße</body></html>";
    assert_eq!(
        strip_htm_with(html, on, "dist/game/data/html/x.htm"),
        html,
        "the file is served unchanged; the key only logs"
    );
}
