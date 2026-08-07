//! The `Say2` chat filters — `General.ini`'s `UseChatFilter` /
//! `ChatFilterChars` / `BanChatChannels`, plus the word list in
//! `config/chatfilter.txt`.
//!
//! Two unrelated things share this module because Java reads them from the same
//! block of `Config.java`:
//!
//! * the **say filter**, which rewrites matched words in an outgoing line
//!   (`Say2.checkText`), and
//! * **`BanChatChannels`**, which decides only which channels a chat-*banned*
//!   player gets the prohibition message on. Java's ban itself covers every
//!   channel — the `return` in `Say2` is unconditional — so this list changes
//!   what the player is *told*, not what gets through.
//!
//! The say filter ships **off** on this dist (`UseChatFilter = False`); it is
//! ported anyway, per the standing rule that a disabled flag is not a reason to
//! skip a port.

use std::collections::HashSet;

use commons::config::PropertiesParser;
use regex::Regex;

use crate::enums::ChatType;

/// Java `Config.CHAT_FILTER_FILE`.
pub const CHAT_FILTER_FILE: &str = "config/chatfilter.txt";

#[derive(Debug, Clone, Default)]
pub struct ChatFilterConfig {
    /// `UseChatFilter` (Java's field is `USE_SAY_FILTER` — the key and the
    /// field disagree upstream; the *key* is what an operator sets).
    pub use_say_filter: bool,
    /// `ChatFilterChars` — what a matched word is replaced with (`^_^` here).
    pub filter_chars: String,
    /// The compiled `chatfilter.txt` patterns, in file order.
    ///
    /// Java compiles each line per message, inside `String.replaceAll("(?i)" +
    /// pattern, chars)`. Compiling once at load is the same match semantics,
    /// and it turns a malformed pattern from "every chat line by every player
    /// throws" into one warning at boot — the one deliberate deviation here.
    pub patterns: Vec<Regex>,
    /// `BanChatChannels`.
    pub ban_chat_channels: HashSet<ChatType>,
}

impl ChatFilterConfig {
    pub fn load_from(root: &str) -> Self {
        let p = PropertiesParser::load_rel(root, super::general::GENERAL_CONFIG_FILE);
        let words =
            std::fs::read_to_string(format!("{root}{CHAT_FILTER_FILE}")).unwrap_or_else(|e| {
                tracing::warn!("Error while loading chat filter words: {e}");
                String::new()
            });
        Self::from_parts(&p, &words)
    }

    pub fn from_parts(p: &PropertiesParser, filter_file: &str) -> Self {
        let patterns = compile_patterns(filter_file);
        if !patterns.is_empty() {
            tracing::info!("Loaded {} Filter Words.", patterns.len());
        }
        Self {
            use_say_filter: p.get_bool("UseChatFilter", false),
            filter_chars: p.get_string("ChatFilterChars", "^_^"),
            patterns,
            ban_chat_channels: parse_ban_channels(&p.get_string("BanChatChannels", "")),
        }
    }

    /// Java `Say2.checkText`: replace every match of every pattern, in file
    /// order, case-insensitively. Returns `None` when nothing changed, so the
    /// common case allocates nothing.
    pub fn filter(&self, text: &str) -> Option<String> {
        if !self.use_say_filter || self.patterns.is_empty() {
            return None;
        }
        let mut out = std::borrow::Cow::Borrowed(text);
        for pattern in &self.patterns {
            if let std::borrow::Cow::Owned(replaced) =
                pattern.replace_all(&out, self.filter_chars.as_str())
            {
                out = std::borrow::Cow::Owned(replaced);
            }
        }
        match out {
            std::borrow::Cow::Owned(s) => Some(s),
            std::borrow::Cow::Borrowed(_) => None,
        }
    }
}

/// Java: trim, drop blanks and `#` comments, keep the rest as regexes.
fn compile_patterns(body: &str) -> Vec<Regex> {
    body.lines()
        .map(str::trim)
        // A UTF-8 BOM on the first line would otherwise make it a non-comment.
        .map(|l| l.trim_start_matches('\u{feff}'))
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|word| match Regex::new(&format!("(?i){word}")) {
            Ok(re) => Some(re),
            Err(e) => {
                tracing::warn!("chatfilter.txt: skipping invalid pattern '{word}': {e}");
                None
            }
        })
        .collect()
}

/// `GENERAL;SHOUT;WORLD;…` — Java `Enum.valueOf(ChatType.class, name)`.
///
/// A name this port has no variant for is skipped with a warning rather than
/// aborting the load: the shipped list names `WORLD`, a channel that exists in
/// Java's cross-chronicle `ChatType` but not in Interlude.
fn parse_ban_channels(raw: &str) -> HashSet<ChatType> {
    raw.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|name| match chat_type_by_java_name(name) {
            Some(t) => Some(t),
            None => {
                tracing::warn!("BanChatChannels: no such chat channel '{name}' — ignored.");
                None
            }
        })
        .collect()
}

/// Java's `ChatType` constant names, which are what the ini spells.
///
/// **This must cover every name the shipped ini uses.** Java resolves them with
/// `Enum.valueOf(ChatType.class, chatId)` under a `catch (NumberFormatException)`
/// — which does *not* catch the `IllegalArgumentException` a bad name throws —
/// so upstream an unknown channel aborts config loading outright. Warning and
/// skipping is the gentler deviation, but it means a name missing from this map
/// silently drops a channel from the ban list instead of failing loudly:
/// `WORLD` was missing until the world-chat port, so a chat-banned player got
/// no prohibition notice on it.
fn chat_type_by_java_name(name: &str) -> Option<ChatType> {
    Some(match name {
        "GENERAL" => ChatType::General,
        "SHOUT" => ChatType::Shout,
        "WHISPER" | "TELL" => ChatType::Whisper,
        "PARTY" => ChatType::Party,
        "CLAN" => ChatType::Clan,
        "PETITION_PLAYER" => ChatType::PetitionPlayer,
        "PETITION_GM" => ChatType::PetitionGm,
        "TRADE" => ChatType::Trade,
        "ALLIANCE" => ChatType::Alliance,
        "ANNOUNCEMENT" => ChatType::Announcement,
        "BOAT" => ChatType::Boat,
        "PARTYMATCH_ROOM" => ChatType::PartyMatchRoom,
        "PARTYROOM_COMMANDER" => ChatType::PartyroomCommander,
        "PARTYROOM_ALL" => ChatType::PartyroomAll,
        "HERO_VOICE" => ChatType::HeroVoice,
        "WORLD" => ChatType::World,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(ini: &str, words: &str) -> ChatFilterConfig {
        ChatFilterConfig::from_parts(&PropertiesParser::from_content("General.ini", ini), words)
    }

    /// The dist's own word list and replacement string.
    #[test]
    fn matched_words_are_replaced_case_insensitively() {
        let c = cfg(
            "UseChatFilter = True\nChatFilterChars = ^_^\n",
            "# comment\nsuck\nfuck\n",
        );
        assert_eq!(c.filter("you suck").as_deref(), Some("you ^_^"));
        assert_eq!(
            c.filter("SUCK and FuCk").as_deref(),
            Some("^_^ and ^_^"),
            "Java prefixes every pattern with (?i)"
        );
        assert_eq!(c.filter("perfectly fine"), None, "no match → no allocation");
    }

    /// Java matches a bare word anywhere in the line, substring included —
    /// `replaceAll` is not word-anchored. Pinned because it is surprising and
    /// an operator's list depends on it.
    #[test]
    fn a_pattern_matches_inside_a_longer_word() {
        let c = cfg("UseChatFilter = True\n", "ass\n");
        assert_eq!(c.filter("classic").as_deref(), Some("cl^_^ic"));
    }

    /// `UseChatFilter = False` (what this dist ships) leaves every line alone,
    /// even with a loaded word list.
    #[test]
    fn the_filter_is_inert_while_disabled() {
        let c = cfg("UseChatFilter = False\n", "suck\n");
        assert!(!c.use_say_filter);
        assert_eq!(c.filter("you suck"), None);
    }

    /// Comment and blank lines are not patterns; a BOM must not turn the first
    /// comment into one.
    #[test]
    fn comments_blanks_and_a_bom_are_skipped() {
        let c = cfg("UseChatFilter = True\n", "\u{feff}# header\n\n  \nreal\n");
        assert_eq!(c.patterns.len(), 1);
        assert_eq!(c.filter("a real word").as_deref(), Some("a ^_^ word"));
    }

    /// One bad pattern must not take the rest of the list with it.
    #[test]
    fn an_invalid_pattern_is_skipped_not_fatal() {
        let c = cfg("UseChatFilter = True\n", "good\n[unclosed\nalso_good\n");
        assert_eq!(c.patterns.len(), 2);
        assert_eq!(c.filter("good").as_deref(), Some("^_^"));
    }

    /// The whole shipped list resolves — `WORLD` included.
    ///
    /// This test used to assert the opposite, on the belief that "WORLD has no
    /// Interlude variant". Java's `ChatType` has `WORLD(25)`, the channel has a
    /// live handler in the datapack, and the dist ships it enabled; the missing
    /// variant was the port's, not the chronicle's. The wrong claim survived
    /// here because the assertion tested the port against itself — a *count*
    /// derived from the same map that was dropping the name.
    #[test]
    fn every_shipped_ban_channel_resolves() {
        let c = cfg(
            "BanChatChannels = GENERAL;SHOUT;WORLD;TRADE;HERO_VOICE;WHISPER\n",
            "",
        );
        for ty in [
            ChatType::General,
            ChatType::Shout,
            ChatType::World,
            ChatType::Trade,
            ChatType::HeroVoice,
            ChatType::Whisper,
        ] {
            assert!(c.ban_chat_channels.contains(&ty), "missing {ty:?}");
        }
        assert_eq!(c.ban_chat_channels.len(), 6, "and nothing was dropped");
        assert!(!c.ban_chat_channels.contains(&ChatType::Party));
    }

    /// A name with no `ChatType` really is skipped rather than fatal — the
    /// deviation from Java, which throws an uncaught `IllegalArgumentException`
    /// out of `Enum.valueOf`. Kept as its own case so that behaviour stays
    /// covered now that the shipped list no longer exercises it.
    #[test]
    fn an_unknown_channel_name_is_skipped_not_fatal() {
        let c = cfg("BanChatChannels = GENERAL;NOT_A_CHANNEL;TRADE\n", "");
        assert_eq!(c.ban_chat_channels.len(), 2);
        assert!(c.ban_chat_channels.contains(&ChatType::General));
        assert!(c.ban_chat_channels.contains(&ChatType::Trade));
    }
}
