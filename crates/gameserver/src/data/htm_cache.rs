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
//! We keep the per-interaction disk read (no cache; see the TODO in
//! `game_loop::target`) but route it through [`read_htm`] so the normalization
//! is applied in exactly one place.

/// Apply the `HtmCache.loadFile` text normalization: strip html comments, then
/// tabs and newlines. Carriage returns are left alone, matching Java's
/// character class.
pub fn strip_htm(content: &str) -> String {
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
    std::fs::read_to_string(path).ok().map(|c| strip_htm(&c))
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
