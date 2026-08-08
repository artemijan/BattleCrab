//! The attribute readers every datapack XML loader in this module opens with.
//!
//! Each loader used to carry its own copy — 26 identical bodies for the string
//! read alone, half of them as a `let attr = |key|` closure over the current
//! element and half as a private `fn attr_str`. They are here instead.
//!
//! What is deliberately *not* here: the loaders whose readers only look alike.
//! `npc_data` unescapes XML entities (`unescape_value`), `pledge_skill_tree`
//! parses strictly and drops an element on invalid UTF-8 rather than replacing
//! the bytes, `route_data` matches attribute names case-insensitively, and
//! `instance_data` / `item_auction_data` trim before parsing. Those differences
//! decide what a datapack file means, so each keeps its own reader.

use quick_xml::events::BytesStart;

/// The raw text of attribute `key`, or `None` when the element does not carry
/// it. Invalid UTF-8 is replaced rather than rejected (`from_utf8_lossy`):
/// the datapack is authored as UTF-8, and a mangled character in one name is
/// not worth dropping the whole entry over.
///
/// Entity references are **not** expanded — see the module note.
pub(crate) fn attr_str(e: &BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

/// The raw text of attribute `key`, named as a `&str` and read **strictly**:
/// invalid UTF-8 gives `None`, so the caller drops the entry rather than
/// storing a mangled one. The counterpart to [`attr_str`] for the loaders that
/// would rather lose an entry than keep a corrupt one.
pub(crate) fn attr_strict(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key.as_bytes())
        .and_then(|a| String::from_utf8(a.value.into_owned()).ok())
}

/// Attribute `key` parsed as `i32` — `None` if absent *or* unparseable, which
/// every caller treats the same way: fall back to a default, or skip the entry.
pub(crate) fn attr_i32(e: &BytesStart, key: &[u8]) -> Option<i32> {
    attr_str(e, key).and_then(|v| v.parse().ok())
}

/// Attribute `key` parsed as `i32` after trimming. Kept apart from
/// [`attr_i32`] on purpose: `instances.xml` and `ItemAuction.xml` pad some
/// numeric attributes with whitespace, and a plain `parse()` rejects those —
/// while trimming everywhere would quietly start accepting padded values in
/// files that have never contained any.
pub(crate) fn attr_i32_trimmed(e: &BytesStart, key: &[u8]) -> Option<i32> {
    attr_str(e, key).and_then(|v| v.trim().parse().ok())
}

/// Attribute `key` parsed as `i64` — the [`attr_i32`] counterpart for counts and
/// timestamps that outgrow 32 bits.
pub(crate) fn attr_i64(e: &BytesStart, key: &[u8]) -> Option<i64> {
    attr_str(e, key).and_then(|v| v.parse().ok())
}

/// Attribute `key` parsed as `f64` — chances, multipliers and stat values.
pub(crate) fn attr_f64(e: &BytesStart, key: &[u8]) -> Option<f64> {
    attr_str(e, key).and_then(|v| v.parse().ok())
}
