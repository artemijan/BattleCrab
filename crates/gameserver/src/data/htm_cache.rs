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
//! **The cache.** Every read goes through [`read_htm`], so the normalization
//! is applied in exactly one place and the file is read from disk at most once.
//! `General.ini`'s `HtmCache` picks Java's branch ([`install_cache`], at boot):
//!
//! - **True — eager.** The whole `data/` tree's `.htm`/`.html` (~9.8k files,
//!   ~6 MB) is loaded before the game thread starts, and a miss means "no such
//!   file": a dialog open never touches the disk, which is what
//!   THREADING_MODEL rule 1 (no file syscalls on the game thread) asks for.
//!   The cache is the existence oracle, as in Java — a file added after boot
//!   stays invisible until `//reload html`.
//! - **False — lazy.** A miss reads the disk once and remembers the hit (Java's
//!   `ConcurrentHashMap` branch); misses are not remembered, so a fallback
//!   chain still probes the disk for files that do not exist.
//!
//! Either way an edited `.htm` needs `//reload html` to show. With no cache
//! installed — every unit test — reads go straight to disk, uncached, so a
//! fixture that rewrites a file sees its own write.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

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

static HTML_SETTINGS: OnceLock<HtmlSettings> = OnceLock::new();

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

/// The process-wide html cache — Java's `HTML_CACHE`. Keyed by the path the
/// callers build (`{root}data/html/…`); `Path`'s equality and hash compare
/// components, so the boot walk's `{root}data` + joins match them exactly.
///
/// After boot only the game thread reads it, and the only writer besides the
/// lazy branch's own inserts is the `//reload html` swap, so the lock is never
/// held across I/O and never contended in practice.
struct Cache {
    eager: bool,
    files: RwLock<HashMap<PathBuf, String>>,
}

static CACHE: OnceLock<Cache> = OnceLock::new();

/// What a (re)load put in memory — Java's `Cache[HTML]: %.3f megabytes on %d
/// files loaded.`
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CacheStats {
    pub files: usize,
    /// Raw file bytes, as Java counts them (`bis.available()`).
    pub bytes: u64,
}

impl CacheStats {
    pub fn megabytes(&self) -> f64 {
        self.bytes as f64 / 1_048_576.0
    }
}

/// Install the cache once at boot, after [`set_html_settings`] (the loads
/// normalize with them). `eager` is `General.ini`'s `HtmCache`; on true the
/// whole `{root}data` tree is loaded here. A second call is ignored.
pub fn install_cache(root: &str, eager: bool) -> CacheStats {
    let (files, stats) = if eager {
        load_tree(&data_dir(root))
    } else {
        (HashMap::new(), CacheStats::default())
    };
    let _ = CACHE.set(Cache {
        eager,
        files: RwLock::new(files),
    });
    stats
}

/// `HtmCache.reload()`: eager rebuilds the whole tree (read without the lock,
/// then one swap — files deleted since boot drop out too), lazy forgets
/// everything so the next read of each file goes to disk. With no cache
/// installed there is nothing to reload.
pub fn reload_cache(root: &str) -> CacheStats {
    let Some(cache) = CACHE.get() else {
        return CacheStats::default();
    };
    if !cache.eager {
        write(cache).clear();
        return CacheStats::default();
    }
    let (files, stats) = load_tree(&data_dir(root));
    *write(cache) = files;
    stats
}

/// `HtmCache.reload(File)`: re-read one file, or a directory's html, into the
/// cache — the `//reload html <path>` form. `None` when the path does not
/// exist.
pub fn reload_cache_path(path: &Path) -> Option<CacheStats> {
    if !path.exists() {
        return None;
    }
    let Some(cache) = CACHE.get() else {
        return Some(CacheStats::default());
    };
    let (files, stats) = if path.is_dir() {
        load_tree(path)
    } else {
        let mut one = HashMap::new();
        let mut stats = CacheStats::default();
        if is_html(path)
            && let Some((text, bytes)) = load_file(path)
        {
            one.insert(path.to_path_buf(), text);
            stats = CacheStats { files: 1, bytes };
        }
        (one, stats)
    };
    write(cache).extend(files);
    Some(stats)
}

fn data_dir(root: &str) -> PathBuf {
    PathBuf::from(format!("{root}data"))
}

fn write(cache: &Cache) -> std::sync::RwLockWriteGuard<'_, HashMap<PathBuf, String>> {
    cache.files.write().unwrap_or_else(|e| e.into_inner())
}

/// Java's `HTML_FILTER`: the name ends `.htm`/`.html`, any case.
fn is_html(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("htm") || e.eq_ignore_ascii_case("html"))
}

/// `HtmCache.parseDir`: every html file under `dir`, normalized.
fn load_tree(dir: &Path) -> (HashMap<PathBuf, String>, CacheStats) {
    let mut files = HashMap::new();
    let mut stats = CacheStats::default();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(t) if t.is_dir() => pending.push(path),
                Ok(_) if is_html(&path) => {
                    if let Some((text, bytes)) = load_file(&path) {
                        stats.files += 1;
                        stats.bytes += bytes;
                        files.insert(path, text);
                    }
                }
                _ => {}
            }
        }
    }
    (files, stats)
}

/// `HtmCache.loadFile`: read and normalize one file; the raw byte length rides
/// along for the stats line.
fn load_file(path: &Path) -> Option<(String, u64)> {
    let raw = std::fs::read_to_string(path).ok()?;
    let shown = path.to_string_lossy();
    Some((strip_htm_with(&raw, settings(), &shown), raw.len() as u64))
}

/// Read a datapack html file and normalize it like Java's `HtmCache`.
/// Returns `None` when the file is missing, so callers keep their existing
/// fallback chains (`.or_else(…)`, "text is missing" stubs).
pub fn read_htm(path: impl AsRef<Path>) -> Option<String> {
    let p = path.as_ref();
    let Some(cache) = CACHE.get() else {
        return load_file(p).map(|(text, _)| text);
    };
    if let Some(hit) = cache.files.read().unwrap_or_else(|e| e.into_inner()).get(p) {
        return Some(hit.clone());
    }
    // Eager: everything was loaded at boot, so a miss is a missing file.
    if cache.eager {
        return None;
    }
    let (text, _) = load_file(p)?;
    write(cache).insert(p.to_path_buf(), text.clone());
    Some(text)
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

    /// A scratch datapack root (`…/` with a trailing slash, like
    /// `DATAPACK_ROOT`) holding `data/html/…`.
    fn scratch_root(name: &str) -> String {
        let dir = std::env::temp_dir().join(format!("htm_cache_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("data/html/default")).unwrap();
        std::fs::create_dir_all(dir.join("data/scripts/quests/Q1")).unwrap();
        format!("{}/", dir.display())
    }

    /// `parseDir`: every `.htm`/`.html` under `data/`, any case, nested, and
    /// nothing else — normalized on the way in, raw bytes counted.
    #[test]
    fn tree_load_takes_html_only_and_normalizes() {
        let root = scratch_root("tree");
        let raw = "<html>\n<!-- gone -->hi</html>";
        std::fs::write(format!("{root}data/html/default/30001.htm"), raw).unwrap();
        std::fs::write(format!("{root}data/scripts/quests/Q1/start.HTML"), "x").unwrap();
        std::fs::write(format!("{root}data/html/default/notes.txt"), "no").unwrap();
        std::fs::write(format!("{root}data/Routes.xml"), "<list/>").unwrap();

        let (files, stats) = load_tree(&data_dir(&root));
        assert_eq!(stats.files, 2);
        assert_eq!(stats.bytes, raw.len() as u64 + 1);
        assert_eq!(files.len(), 2);
        // Looked up by the path a caller builds — the key must match it.
        let key = PathBuf::from(format!("{root}data/html/default/30001.htm"));
        assert_eq!(files.get(&key).map(String::as_str), Some("<html>hi</html>"));
        assert!(files.contains_key(&PathBuf::from(format!(
            "{root}data/scripts/quests/Q1/start.HTML"
        ))));
        let _ = std::fs::remove_dir_all(root);
    }

    /// `Path` equality is per component, so a caller's doubled separator
    /// (`{root}` already ends in `/`, then a `/` in the format string) still
    /// hits the entry the walk stored.
    #[test]
    fn cache_keys_ignore_doubled_separators() {
        let root = scratch_root("keys");
        std::fs::write(format!("{root}data/html/default/1.htm"), "a").unwrap();
        let (files, _) = load_tree(&data_dir(&root));
        let sloppy = PathBuf::from(format!("{root}/data//html/default/1.htm"));
        assert!(files.contains_key(&sloppy));
        let _ = std::fs::remove_dir_all(root);
    }

    /// The real tree: the preload finds the dist's html, and the key a dialog
    /// builds (`show_chat_window`'s `{root}data/html/<dir>/<id>.htm`) hits.
    #[test]
    fn preloads_the_dist_tree_and_serves_dialog_paths() {
        let root = crate::data::DIST_GAME;
        let started = std::time::Instant::now();
        let (files, stats) = load_tree(&data_dir(root));
        eprintln!(
            "dist html preload: {} files, {:.3} MB, {:?}",
            stats.files,
            stats.megabytes(),
            started.elapsed()
        );
        assert!(stats.files > 9000, "dist html present? {}", stats.files);
        let page = PathBuf::from(format!("{root}data/html/villagemaster/30026.htm"));
        let html = files.get(&page).expect("villagemaster 30026 preloaded");
        assert!(!html.contains('\n') && !html.contains("<!--"));
    }

    #[test]
    fn html_filter_is_extension_based_and_case_blind() {
        assert!(is_html(Path::new("a/b.htm")));
        assert!(is_html(Path::new("a/b.HTML")));
        assert!(!is_html(Path::new("a/b.htm.bak")));
        assert!(!is_html(Path::new("a/htm")));
    }
}
