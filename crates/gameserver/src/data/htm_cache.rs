//! Port of `org.l2jmobius.gameserver.cache.HtmCache`.
//!
//! Java preloads every datapack `.htm`/`.html` at startup and normalizes the
//! text once, on the way into the cache (`HtmCache.loadFile`):
//!
//! ```java
//! content = content.replaceAll("(?s)<!--.*?-->", ""); // Remove html comments.
//! content = content.replaceAll("[\\t\\n]", "");       // Remove tabs and new lines.
//! ```
//!
//! Every html the client ever sees has been through that filter, so datapack
//! authors comment out buttons freely (see `html/default/31076.htm`, the
//! Talking Island Harbor Newbie Guide). The L2 client does not understand
//! `<!-- -->`: it eats `<!-- <Button …>` as one unknown tag and renders the
//! trailing `-->` as literal text in the dialog. Reading these files raw is
//! therefore not "close enough" — it is visibly wrong.
//!
//! **No cache, by choice.** Java streams these through `HtmCache`, which loads
//! every file at boot; this port reads per interaction and routes it through
//! [`read_htm`] so the normalization is applied in exactly one place. The
//! rendered output is identical either way — the difference is a disk read on
//! a dialog open, against a boot-time load of the whole `html/` tree — and
//! reading per interaction has the small operational advantage that an edited
//! `.htm` takes effect without a restart.
//!
//! Not a parity gap and not owed work. Revisit only if a profile shows dialog
//! opens costing measurable time.

/// The `General.ini` keys `HtmCache.loadFile` applies to every file it reads.
///
/// A module-level setting rather than a parameter because `read_htm` has ~40
/// call sites and Java holds the same values in `Config` statics. Installed
/// once at boot; the derived `Default` is Java's own code defaults, so an
/// uninitialised test still behaves like a stock server.
#[derive(Debug, Clone, Copy)]
pub struct HtmlSettings {
    /// `HideBypassRemoval` — strip `-h` from the three named bypasses.
    pub hide_bypass_removal: bool,
    /// `CheckHtmlEncoding` — warn when a file is not pure ASCII.
    pub check_encoding: bool,
}

impl Default for HtmlSettings {
    fn default() -> Self {
        Self {
            hide_bypass_removal: true,
            check_encoding: true,
        }
    }
}

static HTML_SETTINGS: std::sync::OnceLock<HtmlSettings> = std::sync::OnceLock::new();

/// Install the boot-time settings. Ignored if called twice — the second call
/// would be a second `Config` load, which cannot happen on this server.
pub fn set_html_settings(settings: HtmlSettings) {
    let _ = HTML_SETTINGS.set(settings);
}

fn settings() -> HtmlSettings {
    HTML_SETTINGS.get().copied().unwrap_or_default()
}

/// Apply the `HtmCache.loadFile` text normalization: strip html comments, then
/// tabs and newlines. Carriage returns are left alone, matching Java's
/// character class.
///
/// Then `HideBypassRemoval`'s three replacements, in Java's order and on the
/// already-stripped text — which matters, because a `-h` inside an html comment
/// is gone by the time this runs.
pub fn strip_htm(content: &str) -> String {
    strip_htm_with(content, settings(), "")
}

/// [`strip_htm`] with explicit settings and a path for the encoding warning —
/// the form the tests drive, so neither behaviour depends on boot order.
pub fn strip_htm_with(content: &str, settings: HtmlSettings, path: &str) -> String {
    let out = strip_htm_inner(content);
    let out = if settings.hide_bypass_removal {
        // Java's three literal replacements, verbatim. Note the trailing space
        // on the first: `_Chat ` takes an argument, the other two do not.
        out.replace(
            "bypass -h npc_%objectId%_Chat ",
            "bypass npc_%objectId%_Chat ",
        )
        .replace(
            "bypass -h npc_%objectId%_Quest",
            "bypass npc_%objectId%_Quest",
        )
        .replace(
            "bypass -h npc_%objectId%_showTeleports",
            "bypass npc_%objectId%_showTeleports",
        )
    } else {
        out
    };
    // `Config.CHECK_HTML_ENCODING && !filePath.startsWith("data/lang")` — a
    // load-time diagnostic, not a refusal: the file is served either way.
    if settings.check_encoding && !path.is_empty() && !out.is_ascii() {
        let rel = match path.find("data/") {
            Some(i) => &path[i..],
            None => path,
        };
        if !rel.starts_with("data/lang") {
            tracing::warn!("HTML encoding check: File {rel} contains non ASCII content.");
        }
    }
    out
}

fn strip_htm_inner(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut rest = content;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        // An unterminated comment swallows the remainder, as `(?s)<!--.*?-->`
        // simply fails to match and Java leaves it — but a dangling `<!--`
        // with no `-->` is malformed either way; drop it so nothing leaks.
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + "-->".len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out.retain(|c| c != '\t' && c != '\n');
    out
}

/// Read a datapack html file and normalize it like Java's `HtmCache`.
/// Returns `None` when the file is missing, so callers keep their existing
/// fallback chains (`.or_else(…)`, "text is missing" stubs).
pub fn read_htm(path: impl AsRef<std::path::Path>) -> Option<String> {
    let p = path.as_ref();
    let shown = p.to_string_lossy().into_owned();
    std::fs::read_to_string(p)
        .ok()
        .map(|c| strip_htm_with(&c, settings(), &shown))
}

/// [`read_htm`] for a file being served **to a player** — Java's
/// `HtmCache.getHtm(player, path)`, as opposed to the `getHtm(null, path)` the
/// loaders and scans use.
///
/// The recipient is the whole difference: it carries `Config.GM_DEBUG_HTML_PATHS`
/// (**True** on this dist), which sends a GM the path of every html the server
/// hands them. It is how a GM answers "which file is this dialog?" without
/// grepping the datapack, and it is why the parameter exists in Java's
/// signature at all.
///
/// Java prints `newPath.substring(5)`, dropping the leading `data/` — the
/// path as the datapack author would write it.
pub fn read_htm_for(
    world: &crate::world::World,
    player_object_id: i32,
    path: impl AsRef<std::path::Path>,
) -> Option<String> {
    let content = read_htm(&path);
    if world.cfg.general.gm_debug_html_paths
        && crate::game_loop::helpers::is_gm(world, player_object_id)
    {
        let shown = path.as_ref().to_string_lossy().into_owned();
        // The port reads under the datapack root rather than Java's cache key,
        // so strip whatever prefix precedes `data/` instead of a fixed 5.
        let shown = match shown.find("data/") {
            Some(i) => shown[i + "data/".len()..].to_string(),
            None => shown,
        };
        crate::game_loop::helpers::send_to_player(
            world,
            player_object_id,
            crate::network::server_packets::system_message_with(
                crate::network::server_packets::sm_ids::S1_TEXT,
                &[crate::network::server_packets::SmParam::Text(shown)],
            ),
        );
    }
    content
}

/// [`read_htm_for`] keyed by client id, for the many handlers that hold one
/// rather than an object id. A client with no in-game player reads the file
/// with no debug line, which is Java's `getHtm(null, path)`.
pub fn read_htm_for_client(
    world: &crate::world::World,
    client_id: u32,
    path: impl AsRef<std::path::Path>,
) -> Option<String> {
    match world.player_oid(client_id) {
        Some(oid) => read_htm_for(world, oid, path),
        None => read_htm(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_commented_out_button() {
        // Verbatim from data/html/default/31076.htm (Newbie Guide).
        let html = "<html><body>Newbie Guide:<br>\n\
                    <Button action=\"bypass -h npc_%objectId%_Chat 1\">Ask for advice.</Button>\n\
                    <!-- <Button action=\"bypass -h npc_%objectId%_Chat 2\">Novices</Button> -->\n\
                    <Button action=\"bypass -h Quest\">Quest</Button>\n</body></html>";
        let out = strip_htm(html);
        assert!(!out.contains("-->"), "comment terminator leaked: {out}");
        assert!(!out.contains("Novices"));
        assert!(out.contains("Ask for advice."));
        assert!(out.contains("Quest"));
    }

    #[test]
    fn strips_tabs_and_newlines_but_keeps_text() {
        assert_eq!(
            strip_htm("<html>\n\t<body>hi</body>\n</html>"),
            "<html><body>hi</body></html>"
        );
    }

    #[test]
    fn handles_multiline_and_multiple_comments() {
        assert_eq!(strip_htm("a<!-- one\ntwo -->b<!--x-->c"), "abc");
    }

    #[test]
    fn drops_unterminated_comment() {
        assert_eq!(strip_htm("keep<!-- dangling"), "keep");
    }
}
