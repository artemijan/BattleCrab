//! Port of `model/Shortcut`/`ShortCuts` and `model/Macro`/`MacroCmd`/
//! `MacroList` — the shortcut panel and server-stored macros (G9.6, plan:
//! `PLAN_MACROS_SHORTCUTS.md`). The registry logic lives here as methods
//! on the [`Shortcuts`]/[`Macros`] components (declared in
//! `model/components.rs` with the rest of the player component set); DB I/O
//! and packet echoes stay in the handlers (`game_loop/client/shortcuts.rs`).
//!
//! Macro *execution* is client-side (the client replays each command as
//! ordinary packets); the server only stores macros and echoes them back.

use std::collections::BTreeMap;

use enum_ordinalize::Ordinalize;

use super::components::{Macros, Shortcuts};

/// `ShortCuts.MAX_SHORTCUTS_PER_BAR` — the slot/page → storage-key factor and
/// the client wire encoding (`slot + page * 12`).
pub const MAX_SHORTCUTS_PER_BAR: i32 = 12;

/// First macro id handed out (`MacroList._macroId` starts at 1000).
pub const FIRST_MACRO_ID: i32 = 1000;

/// `enums/ShortcutType` — wire value / DB `type` column = Java ordinal.
///
/// `#[repr(i32)]` fixes the ordinal type the client reads; `Ordinalize` derives
/// both directions from the declaration, so a variant's number lives in exactly
/// one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Ordinalize)]
#[repr(i32)]
#[ordinalize(ordinal(pub const fn ordinal, doc = "Java `ordinal()` — the client wire value and the stored `type` column."))]
#[ordinalize(from_ordinal(const fn checked_from_ordinal, doc = "`values()[ordinal]`, `None` for an ordinal no variant has."))]
pub enum ShortcutType {
    None = 0,
    Item = 1,
    Skill = 2,
    Action = 3,
    Macro = 4,
    Recipe = 5,
    Bookmark = 6,
}

impl ShortcutType {
    /// `values()[ordinal]`, out-of-range → `None` (both `RequestShortCutReg`'s
    /// `(typeId < 1) || (typeId > 6) ? 0 : typeId` clamp and a lenient DB
    /// restore land here).
    pub fn from_ordinal(v: i32) -> Self {
        Self::checked_from_ordinal(v).unwrap_or(Self::None)
    }
}

/// `enums/MacroType` — wire value / `commands` encoding = Java ordinal (see
/// [`ShortcutType`] for the derive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Ordinalize)]
#[repr(i32)]
#[ordinalize(ordinal(pub const fn ordinal, doc = "Java `ordinal()` — the wire value and the `commands` encoding."))]
#[ordinalize(from_ordinal(const fn checked_from_ordinal, doc = "`values()[ordinal]`, `None` for an ordinal no variant has."))]
pub enum MacroType {
    None = 0,
    Skill = 1,
    Action = 2,
    Text = 3,
    Shortcut = 4,
    Item = 5,
    Delay = 6,
}

impl MacroType {
    /// `values()[ordinal]`, out-of-range → `None` (`RequestMakeMacro`'s
    /// `(type < 1) || (type > 6) ? 0 : type` clamp).
    pub fn from_ordinal(v: i32) -> Self {
        Self::checked_from_ordinal(v).unwrap_or(Self::None)
    }
}

/// `model/Shortcut` — one panel slot. `sub_level` is omitted (no skill
/// sub-levels in Interlude data; the DB column and packets write 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    /// Slot 0-11 within the page.
    pub slot: i32,
    /// Page 0-9 in the UI (handlers accept 0-19, like Java).
    pub page: i32,
    pub kind: ShortcutType,
    /// Item *object id* / skill id / action id / macro id.
    pub id: i32,
    /// Skill level (SKILL type only).
    pub level: i32,
    /// 1 player, 2 summon (stored, nothing consumes it — no pets).
    pub character_type: i32,
    /// -1 ungrouped; runtime ITEM registration copies the item template's
    /// `shared_reuse_group` (never set in this dist's XMLs, so 0).
    pub shared_reuse_group: i32,
}

impl Shortcut {
    /// The client wire encoding and the `Shortcuts` map key.
    pub fn client_slot(&self) -> i32 {
        self.slot + self.page * MAX_SHORTCUTS_PER_BAR
    }
}

/// `model/MacroCmd` — one command of a macro.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MacroCmd {
    /// Position within the macro (Java `_entry`; only the DB round-trip and
    /// re-numbering use it — the packet writes its own running index).
    pub entry: i32,
    pub kind: MacroType,
    /// Skill id / action id / page (SHORTCUT) / item id / delay seconds.
    pub d1: i32,
    /// Skill level / slot (SHORTCUT).
    pub d2: i32,
    /// The typed command (TEXT), empty otherwise.
    pub cmd: String,
}

/// `model/Macro` — one macro.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Macro {
    pub id: i32,
    pub icon: i32,
    pub name: String,
    pub descr: String,
    pub acronym: String,
    pub commands: Vec<MacroCmd>,
}

/// `enums/MacroUpdateType` — the `SendMacroList` header byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroUpdateType {
    Add,
    List,
    Modify,
    Delete,
}

impl MacroUpdateType {
    pub const fn id(self) -> u8 {
        match self {
            Self::Add | Self::List => 1,
            Self::Modify => 2,
            Self::Delete => 0,
        }
    }
}

impl Shortcuts {
    pub fn get(&self, slot: i32, page: i32) -> Option<&Shortcut> {
        self.0.get(&(slot + page * MAX_SHORTCUTS_PER_BAR))
    }

    /// `ShortCuts.registerShortCut`'s map put (the ITEM inventory check and
    /// DB write live in the handler, which has the inventory/db channel).
    /// Returns the replaced shortcut, if any.
    pub fn put(&mut self, sc: Shortcut) -> Option<Shortcut> {
        self.0.insert(sc.client_slot(), sc)
    }

    /// `ShortCuts.deleteShortCut`'s map remove.
    pub fn remove(&mut self, slot: i32, page: i32) -> Option<Shortcut> {
        self.0.remove(&(slot + page * MAX_SHORTCUTS_PER_BAR))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Shortcut> {
        self.0.values()
    }

    /// The panel slots holding a given macro — `MacroList.deleteMacro`'s
    /// cascade scan.
    pub fn slots_of_macro(&self, macro_id: i32) -> Vec<(i32, i32)> {
        self.0
            .values()
            .filter(|sc| sc.kind == ShortcutType::Macro && sc.id == macro_id)
            .map(|sc| (sc.slot, sc.page))
            .collect()
    }

    pub fn from_list(list: Vec<Shortcut>) -> Self {
        Self(BTreeMap::from_iter(
            list.into_iter().map(|sc| (sc.client_slot(), sc)),
        ))
    }
}

impl Default for Macros {
    fn default() -> Self {
        Self {
            next_id: FIRST_MACRO_ID,
            entries: Vec::new(),
        }
    }
}

impl Macros {
    pub fn get(&self, id: i32) -> Option<&Macro> {
        self.entries.iter().find(|m| m.id == id)
    }

    /// `MacroList.registerMacro`: id 0 = a new macro (allocate the next free
    /// id ≥ 1000) → `Add`; a real id = replace-in-place → `Modify`.
    pub fn register(&mut self, mut macro_: Macro) -> (i32, MacroUpdateType) {
        if macro_.id == 0 {
            while self.entries.iter().any(|m| m.id == self.next_id) {
                self.next_id += 1;
            }
            macro_.id = self.next_id;
            self.next_id += 1;
            let id = macro_.id;
            self.entries.push(macro_);
            (id, MacroUpdateType::Add)
        } else {
            let id = macro_.id;
            match self.entries.iter_mut().find(|m| m.id == id) {
                Some(slot) => *slot = macro_,
                // Java `LinkedHashMap.put` inserts unknown ids too (still
                // reported as MODIFY — the client sent a concrete id).
                None => self.entries.push(macro_),
            }
            (id, MacroUpdateType::Modify)
        }
    }

    /// `MacroList.deleteMacro`'s map remove (shortcut cascade + packets in
    /// the handler).
    pub fn delete(&mut self, id: i32) -> Option<Macro> {
        let idx = self.entries.iter().position(|m| m.id == id)?;
        Some(self.entries.remove(idx))
    }

    /// Restore from DB rows, keeping insertion order (Java `LinkedHashMap`).
    pub fn from_list(entries: Vec<Macro>) -> Self {
        Self {
            next_id: FIRST_MACRO_ID,
            entries,
        }
    }
}

/// `MacroList.registerMacroInDb`'s `commands` column encoding:
/// `type_ordinal,d1,d2[,cmd];` per command, the whole string truncated at 255
/// chars (Java `sb.setLength(255)` — kept for round-trip parity, even though
/// the column holds 500).
pub fn encode_commands(commands: &[MacroCmd]) -> String {
    let mut s = String::with_capacity(300);
    for cmd in commands {
        s.push_str(&format!("{},{},{}", cmd.kind.ordinal(), cmd.d1, cmd.d2));
        if !cmd.cmd.is_empty() {
            s.push(',');
            s.push_str(&cmd.cmd);
        }
        s.push(';');
    }
    if s.len() > 255 {
        s.truncate(255);
    }
    s
}

/// `MacroList.restoreMe`'s tokenizer: split on `;`, then `,`; entries with
/// fewer than 3 fields are skipped; the command text is the 4th `,`-token
/// only (a comma inside a TEXT command truncates it — Java stores the raw
/// text unescaped and re-reads it with the same tokenizer, so this is the
/// same data loss).
pub fn decode_commands(s: &str) -> Vec<MacroCmd> {
    let mut out = Vec::new();
    for part in s.split(';') {
        let tokens: Vec<&str> = part.split(',').collect();
        if tokens.len() < 3 || tokens[..3].iter().any(|t| t.trim().is_empty()) {
            continue;
        }
        let (Ok(kind), Ok(d1), Ok(d2)) = (
            tokens[0].parse::<i32>(),
            tokens[1].parse::<i32>(),
            tokens[2].parse::<i32>(),
        ) else {
            continue;
        };
        let cmd = tokens.get(3).copied().unwrap_or("").to_string();
        out.push(MacroCmd {
            entry: out.len() as i32,
            kind: MacroType::from_ordinal(kind),
            d1,
            d2,
            cmd,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(kind: MacroType, d1: i32, d2: i32, cmd: &str) -> MacroCmd {
        MacroCmd {
            entry: 0,
            kind,
            d1,
            d2,
            cmd: cmd.to_string(),
        }
    }

    /// The ordinals are the client wire values and the stored `type` column, so
    /// they are pinned here against Java's two enums rather than left to the
    /// declaration order: renumbering a variant silently re-labels every saved
    /// shortcut and macro command.
    #[test]
    fn ordinals_match_java_and_round_trip() {
        let shortcuts = [
            (ShortcutType::None, 0),
            (ShortcutType::Item, 1),
            (ShortcutType::Skill, 2),
            (ShortcutType::Action, 3),
            (ShortcutType::Macro, 4),
            (ShortcutType::Recipe, 5),
            (ShortcutType::Bookmark, 6),
        ];
        for (kind, ordinal) in shortcuts {
            assert_eq!(kind.ordinal(), ordinal, "{kind:?}");
            assert_eq!(ShortcutType::from_ordinal(ordinal), kind);
        }
        let macros = [
            (MacroType::None, 0),
            (MacroType::Skill, 1),
            (MacroType::Action, 2),
            (MacroType::Text, 3),
            (MacroType::Shortcut, 4),
            (MacroType::Item, 5),
            (MacroType::Delay, 6),
        ];
        for (kind, ordinal) in macros {
            assert_eq!(kind.ordinal(), ordinal, "{kind:?}");
            assert_eq!(MacroType::from_ordinal(ordinal), kind);
        }
        // Java's `(v < 1) || (v > 6) ? 0 : v` clamp: out of range reads as None.
        for v in [i32::MIN, -1, 7, 100, i32::MAX] {
            assert_eq!(ShortcutType::from_ordinal(v), ShortcutType::None, "{v}");
            assert_eq!(MacroType::from_ordinal(v), MacroType::None, "{v}");
        }
    }

    #[test]
    fn commands_round_trip() {
        let cmds = vec![
            cmd(MacroType::Skill, 1177, 1, ""),
            cmd(MacroType::Delay, 5, 0, ""),
            cmd(MacroType::Text, 0, 0, "/loc"),
        ];
        let encoded = encode_commands(&cmds);
        assert_eq!(encoded, "1,1177,1;6,5,0;3,0,0,/loc;");
        let decoded = decode_commands(&encoded);
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].kind, MacroType::Skill);
        assert_eq!(decoded[0].d1, 1177);
        assert_eq!(decoded[2].cmd, "/loc");
        assert_eq!(decoded[2].entry, 2);
    }

    #[test]
    fn decode_skips_malformed_entries() {
        // Missing fields and junk are skipped, valid entries survive.
        let decoded = decode_commands("1,10;;garbage;2,20,0;");
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].kind, MacroType::Action);
        assert_eq!(decoded[0].d1, 20);
    }

    #[test]
    fn macro_ids_allocate_from_1000_skipping_taken() {
        let mut macros = Macros::from_list(vec![Macro {
            id: 1000,
            icon: 0,
            name: "taken".into(),
            descr: String::new(),
            acronym: String::new(),
            commands: vec![],
        }]);
        let m = Macro {
            id: 0,
            icon: 1,
            name: "new".into(),
            descr: String::new(),
            acronym: String::new(),
            commands: vec![],
        };
        let (id, update) = macros.register(m.clone());
        assert_eq!(id, 1001);
        assert_eq!(update, MacroUpdateType::Add);
        // Re-registering with the real id modifies in place.
        let (id2, update2) = macros.register(Macro {
            id: 1001,
            name: "edited".into(),
            ..m
        });
        assert_eq!(id2, 1001);
        assert_eq!(update2, MacroUpdateType::Modify);
        assert_eq!(macros.entries.len(), 2);
        assert_eq!(macros.get(1001).unwrap().name, "edited");
    }

    #[test]
    fn shortcut_slot_key_and_macro_cascade_scan() {
        let mut shortcuts = Shortcuts::default();
        let sc = Shortcut {
            slot: 3,
            page: 2,
            kind: ShortcutType::Macro,
            id: 1000,
            level: 0,
            character_type: 1,
            shared_reuse_group: -1,
        };
        shortcuts.put(sc);
        assert_eq!(sc.client_slot(), 27);
        assert!(shortcuts.get(3, 2).is_some());
        assert_eq!(shortcuts.slots_of_macro(1000), vec![(3, 2)]);
        assert_eq!(shortcuts.remove(3, 2), Some(sc));
        assert!(shortcuts.get(3, 2).is_none());
    }
}
