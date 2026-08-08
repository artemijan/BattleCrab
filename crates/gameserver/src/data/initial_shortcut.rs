//! Port of `data/xml/InitialShortcutData` — the shortcut panel a freshly
//! created character starts with (`data/stats/initialShortcuts.xml`): global
//! pages, per-classId pages, and the macro presets MACRO slots reference.
//! Applied at character creation (`game_loop/lobby.rs`); persistence only —
//! there's no in-world session to echo packets to at that point.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

use crate::data::xml::{attr_i32, attr_str};
use crate::model::shortcut::{Macro, MacroCmd, MacroType, ShortcutType};

pub const INITIAL_SHORTCUTS_FILE: &str = "data/stats/initialShortcuts.xml";

/// One `<slot>` line — like Java's reuse of the `Shortcut` DTO but keeping
/// the item *id* semantics explicit (ITEM entries hold an item id until
/// creation resolves the created item's object id).
#[derive(Debug, Clone, Copy)]
pub struct InitialShortcut {
    pub slot: i32,
    pub page: i32,
    pub kind: ShortcutType,
    pub id: i32,
    pub level: i32,
    pub character_type: i32,
}

pub struct InitialShortcutData {
    global: Vec<InitialShortcut>,
    by_class: HashMap<i32, Vec<InitialShortcut>>,
    macro_presets: HashMap<i32, Macro>,
}

impl InitialShortcutData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut data = Self::empty();
        let full_path = format!("{file_path}{INITIAL_SHORTCUTS_FILE}");
        if let Ok(content) = std::fs::read_to_string(&full_path) {
            data.parse(&content);
        }
        info!(
            "InitialShortcutData: Loaded {} global, {} class shortcut list(s), {} macro preset(s).",
            data.global.len(),
            data.by_class.len(),
            data.macro_presets.len()
        );
        data
    }

    fn parse(&mut self, content: &str) {
        let mut reader = Reader::from_str(content);
        // <shortcuts> scope: Some(None) = global, Some(Some(id)) = class list.
        let mut cur_class: Option<Option<i32>> = None;
        let mut cur_page = 0;
        // In-flight <macro>: (macro, enabled).
        let mut cur_macro: Option<(Macro, bool)> = None;
        // In-flight <command …> waiting for its text content.
        let mut cur_cmd: Option<MacroCmd> = None;
        loop {
            let event = reader.read_event();
            match event {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    // A self-closing element gets no End event — flush the
                    // in-flight command at the bottom of this arm.
                    let self_closing = matches!(event, Ok(Event::Empty(_)));
                    match e.name().as_ref() {
                        b"shortcuts" => cur_class = Some(attr_i32(e, b"classId")),
                        b"page" => cur_page = attr_i32(e, b"pageId").unwrap_or(0),
                        b"slot" => {
                            let Some(scope) = &cur_class else { continue };
                            let Some(kind) =
                                attr_str(e, b"shortcutType").map(|s| shortcut_type_of(&s))
                            else {
                                continue;
                            };
                            let sc = InitialShortcut {
                                slot: attr_i32(e, b"slotId").unwrap_or(0),
                                page: cur_page,
                                kind,
                                id: attr_i32(e, b"shortcutId").unwrap_or(0),
                                level: attr_i32(e, b"shortcutLevel").unwrap_or(0),
                                character_type: attr_i32(e, b"characterType").unwrap_or(0),
                            };
                            match scope {
                                Some(class_id) => {
                                    self.by_class.entry(*class_id).or_default().push(sc)
                                }
                                None => self.global.push(sc),
                            }
                        }
                        b"macro" => {
                            let enabled = attr_str(e, b"enabled").as_deref() != Some("false");
                            cur_macro = Some((
                                Macro {
                                    id: attr_i32(e, b"macroId").unwrap_or(0),
                                    icon: attr_i32(e, b"icon").unwrap_or(0),
                                    name: attr_str(e, b"name").unwrap_or_default(),
                                    descr: attr_str(e, b"description").unwrap_or_default(),
                                    acronym: attr_str(e, b"acronym").unwrap_or_default(),
                                    commands: Vec::new(),
                                },
                                enabled,
                            ));
                        }
                        b"command" => {
                            let Some((m, _)) = &mut cur_macro else {
                                continue;
                            };
                            let kind = attr_str(e, b"type")
                                .map(|s| macro_type_of(&s))
                                .unwrap_or(MacroType::None);
                            // The Java `parseMacros` d1/d2 switch.
                            let (d1, d2) = match kind {
                                MacroType::Skill => (
                                    attr_i32(e, b"skillId").unwrap_or(0),
                                    attr_i32(e, b"skillLevel").unwrap_or(0),
                                ),
                                MacroType::Action => (attr_i32(e, b"actionId").unwrap_or(0), 0),
                                MacroType::Shortcut => (
                                    attr_i32(e, b"page").unwrap_or(0),
                                    attr_i32(e, b"slot").unwrap_or(0),
                                ),
                                MacroType::Item => (attr_i32(e, b"itemId").unwrap_or(0), 0),
                                MacroType::Delay => (attr_i32(e, b"delay").unwrap_or(0), 0),
                                MacroType::Text | MacroType::None => (0, 0),
                            };
                            let cmd = MacroCmd {
                                entry: m.commands.len() as i32,
                                kind,
                                d1,
                                d2,
                                cmd: String::new(),
                            };
                            cur_cmd = Some(cmd);
                        }
                        _ => {}
                    }
                    if self_closing
                        && e.name().as_ref() == b"command"
                        && let (Some(cmd), Some((m, _))) = (cur_cmd.take(), &mut cur_macro)
                    {
                        m.commands.push(cmd);
                    }
                }
                Ok(Event::Text(t)) => {
                    if let Some(cmd) = &mut cur_cmd {
                        cmd.cmd = t.unescape().map(|s| s.into_owned()).unwrap_or_default();
                    }
                }
                Ok(Event::End(e)) => match e.name().as_ref() {
                    b"shortcuts" => cur_class = None,
                    b"macro" => {
                        if let Some((m, enabled)) = cur_macro.take()
                            && enabled
                        {
                            self.macro_presets.insert(m.id, m);
                        }
                    }
                    b"command" => {
                        if let (Some(cmd), Some((m, _))) = (cur_cmd.take(), &mut cur_macro) {
                            m.commands.push(cmd);
                        }
                    }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
        }
    }

    pub fn global(&self) -> &[InitialShortcut] {
        &self.global
    }

    pub fn for_class(&self, class_id: i32) -> &[InitialShortcut] {
        self.by_class
            .get(&class_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn macro_preset(&self, macro_id: i32) -> Option<&Macro> {
        self.macro_presets.get(&macro_id)
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            global: Vec::new(),
            by_class: HashMap::new(),
            macro_presets: HashMap::new(),
        }
    }
}

fn shortcut_type_of(s: &str) -> ShortcutType {
    match s {
        "ITEM" => ShortcutType::Item,
        "SKILL" => ShortcutType::Skill,
        "ACTION" => ShortcutType::Action,
        "MACRO" => ShortcutType::Macro,
        "RECIPE" => ShortcutType::Recipe,
        "BOOKMARK" => ShortcutType::Bookmark,
        _ => ShortcutType::None,
    }
}

fn macro_type_of(s: &str) -> MacroType {
    match s {
        "SKILL" => MacroType::Skill,
        "ACTION" => MacroType::Action,
        "TEXT" => MacroType::Text,
        "SHORTCUT" => MacroType::Shortcut,
        "ITEM" => MacroType::Item,
        "DELAY" => MacroType::Delay,
        _ => MacroType::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real() -> InitialShortcutData {
        InitialShortcutData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"))
    }

    #[test]
    fn loads_global_page_actions() {
        let data = real();
        // Page 0: Attack (action 2) in slot 0, Sit/Stand (action 0) in slot 10.
        let attack = data
            .global()
            .iter()
            .find(|s| s.page == 0 && s.slot == 0)
            .expect("attack slot");
        assert_eq!(attack.kind, ShortcutType::Action);
        assert_eq!(attack.id, 2);
        let sit = data
            .global()
            .iter()
            .find(|s| s.page == 0 && s.slot == 10)
            .expect("sit slot");
        assert_eq!(sit.id, 0);
        // The page-1 example MACRO slot parses too (its preset is disabled —
        // creation drops it, `resolve_initial_shortcuts`).
        let macro_slot = data
            .global()
            .iter()
            .find(|s| s.kind == ShortcutType::Macro)
            .expect("macro slot");
        assert_eq!(macro_slot.id, 10000);
    }

    #[test]
    fn loads_class_skill_pages() {
        let data = real();
        // Human Mystic (10): Wind Strike (1177) slot 1, Self Heal (1216) slot 10.
        let mystic = data.for_class(10);
        let ws = mystic.iter().find(|s| s.slot == 1).expect("wind strike");
        assert_eq!(ws.kind, ShortcutType::Skill);
        assert_eq!(ws.id, 1177);
        assert_eq!(ws.level, 1);
        assert!(mystic.iter().any(|s| s.slot == 10 && s.id == 1216));
    }

    #[test]
    fn disabled_macro_preset_is_skipped() {
        let data = real();
        // The stock preset 10000 ships enabled="false" — Java's parseMacros
        // skips it, so the MACRO slot referencing it resolves to nothing.
        assert!(data.macro_preset(10000).is_none());
    }
}
