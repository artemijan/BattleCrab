//! Shortcut-panel and macro-list packets.

use commons::network::PacketWriter;

use super::opcodes;

/// The ITEM-arm tail shared by `ShortCutInit` and `ShortCutRegister`:
/// augmentation option 1/2 + visual id — all 0, our `ItemInstance` carries
/// neither (augmentation/appearance are later milestones; Java reads them
/// off the item and writes the same zeros for a plain one).
fn write_shortcut_item_tail(w: &mut PacketWriter) {
    w.write_i32(0); // augmentation option 1
    w.write_i32(0); // augmentation option 2
    w.write_i32(0); // visual id
}

/// Port of `serverpackets/ShortCutInit` — the full panel, sent on enter
/// world and after every deletion (there's no per-slot delete packet; Java
/// re-sends the whole panel).
pub fn shortcut_init(shortcuts: &crate::model::components::Shortcuts) -> Vec<u8> {
    use crate::model::shortcut::ShortcutType;
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SHORT_CUT_INIT);
    w.write_i32(shortcuts.0.len() as i32);
    for sc in shortcuts.0.values() {
        w.write_i32(sc.kind.ordinal());
        w.write_i32(sc.client_slot());
        match sc.kind {
            ShortcutType::Item => {
                w.write_i32(sc.id);
                w.write_i32(1); // enabled
                w.write_i32(sc.shared_reuse_group);
                w.write_i32(0);
                w.write_i32(0);
                write_shortcut_item_tail(&mut w);
            }
            ShortcutType::Skill => {
                w.write_i32(sc.id);
                w.write_i16(sc.level as i16);
                w.write_i16(0); // sub-level
                w.write_i32(sc.shared_reuse_group);
                w.write_u8(0); // C5
                w.write_i32(1); // C6
            }
            ShortcutType::Action
            | ShortcutType::Macro
            | ShortcutType::Recipe
            | ShortcutType::Bookmark => {
                w.write_i32(sc.id);
                w.write_i32(1); // C6
            }
            // Java's switch has no NONE arm — nothing more is written.
            ShortcutType::None => {}
        }
    }
    w.into_bytes()
}

/// Port of `serverpackets/ShortCutRegister` — the echo for one registered
/// slot.
pub fn shortcut_register(sc: &crate::model::shortcut::Shortcut) -> Vec<u8> {
    use crate::model::shortcut::ShortcutType;
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SHORT_CUT_REGISTER);
    w.write_i32(sc.kind.ordinal());
    w.write_i32(sc.client_slot());
    match sc.kind {
        ShortcutType::Item => {
            w.write_i32(sc.id);
            w.write_i32(sc.character_type);
            w.write_i32(sc.shared_reuse_group);
            w.write_i32(0); // unknown
            w.write_i32(0); // unknown
            write_shortcut_item_tail(&mut w);
        }
        ShortcutType::Skill => {
            w.write_i32(sc.id);
            w.write_i16(sc.level as i16);
            w.write_i16(0); // sub-level
            w.write_i32(sc.shared_reuse_group);
            w.write_u8(0); // C5
            w.write_i32(sc.character_type);
            w.write_i32(0); // if 1 - can't use
            w.write_i32(0); // reuse delay ?
        }
        ShortcutType::Action
        | ShortcutType::Macro
        | ShortcutType::Recipe
        | ShortcutType::Bookmark => {
            w.write_i32(sc.id);
            w.write_i32(sc.character_type);
        }
        ShortcutType::None => {}
    }
    w.into_bytes()
}

/// Port of `serverpackets/SendMacroList`. `count` is the header's macro
/// count: the running total for `List` bursts, 1 for `Add`/`Modify`, 0 for
/// `Delete` (matching the Java call sites). The body is omitted for `Delete`
/// and for the has-no-macros `List` (macro `None`).
pub fn send_macro_list(
    count: i32,
    macro_: Option<&crate::model::shortcut::Macro>,
    update: crate::model::shortcut::MacroUpdateType,
) -> Vec<u8> {
    use crate::model::shortcut::MacroUpdateType;
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MACRO_LIST);
    w.write_u8(update.id());
    // Modified, created or deleted macro's id; 0 for LIST entries.
    w.write_i32(if update != MacroUpdateType::List {
        macro_.map_or(0, |m| m.id)
    } else {
        0
    });
    w.write_u8(count as u8);
    w.write_u8(macro_.is_some() as u8);
    if let Some(m) = macro_
        && update != MacroUpdateType::Delete
    {
        w.write_i32(m.id);
        w.write_string(&m.name);
        w.write_string(&m.descr);
        w.write_string(&m.acronym);
        w.write_i32(m.icon);
        w.write_u8(m.commands.len() as u8);
        for (i, cmd) in m.commands.iter().enumerate() {
            w.write_u8((i + 1) as u8);
            w.write_u8(cmd.kind.ordinal() as u8);
            w.write_i32(cmd.d1);
            w.write_u8(cmd.d2 as u8);
            w.write_string(&cmd.cmd);
        }
    }
    w.into_bytes()
}

/// `MacroList.sendAllMacros` — the enter-world macro burst: one packet per
/// macro carrying the total count, or a single empty LIST packet when the
/// player has none.
pub fn send_all_macros(macros: &crate::model::components::Macros) -> Vec<Vec<u8>> {
    use crate::model::shortcut::MacroUpdateType;
    if macros.entries.is_empty() {
        vec![send_macro_list(0, None, MacroUpdateType::List)]
    } else {
        let count = macros.entries.len() as i32;
        macros
            .entries
            .iter()
            .map(|m| send_macro_list(count, Some(m), MacroUpdateType::List))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    // ---- G9.6 shortcut/macro packet layouts (hand-computed against the
    // Java writeImpls — no client capture available, same approach as the
    // NpcInfo test). ----

    use commons::network::PacketWriter;

    use crate::model::components::{Macros, Shortcuts};
    use crate::model::shortcut::{
        Macro, MacroCmd, MacroType, MacroUpdateType, Shortcut, ShortcutType,
    };

    fn skill_shortcut() -> Shortcut {
        Shortcut {
            slot: 1,
            page: 0,
            kind: ShortcutType::Skill,
            id: 1177,
            level: 2,
            character_type: 1,
            shared_reuse_group: -1,
        }
    }

    #[test]
    fn shortcut_init_layout_per_type() {
        let shortcuts = Shortcuts(std::collections::BTreeMap::from_iter(
            [
                Shortcut {
                    slot: 0,
                    page: 0,
                    kind: ShortcutType::Action,
                    id: 2,
                    level: 0,
                    character_type: 1,
                    shared_reuse_group: -1,
                },
                skill_shortcut(),
                Shortcut {
                    slot: 0,
                    page: 1,
                    kind: ShortcutType::Item,
                    id: 0x1000_0005,
                    level: 0,
                    character_type: 1,
                    shared_reuse_group: 0,
                },
            ]
            .into_iter()
            .map(|sc| (sc.client_slot(), sc)),
        ));
        let mut w = PacketWriter::new();
        w.write_u8(0x45);
        w.write_i32(3);
        // Slot 0: ACTION (Attack).
        w.write_i32(3); // ShortcutType.ACTION ordinal
        w.write_i32(0);
        w.write_i32(2);
        w.write_i32(1); // C6
        // Slot 1: SKILL.
        w.write_i32(2);
        w.write_i32(1);
        w.write_i32(1177);
        w.write_i16(2); // level
        w.write_i16(0); // sub-level
        w.write_i32(-1); // shared reuse group
        w.write_u8(0); // C5
        w.write_i32(1); // C6
        // Slot 12 (page 1 slot 0): ITEM.
        w.write_i32(1);
        w.write_i32(12);
        w.write_i32(0x1000_0005);
        w.write_i32(1); // enabled
        w.write_i32(0); // shared reuse group (template default)
        w.write_i32(0);
        w.write_i32(0);
        w.write_i32(0); // augment 1
        w.write_i32(0); // augment 2
        w.write_i32(0); // visual id
        assert_eq!(super::shortcut_init(&shortcuts), w.into_bytes());
    }

    #[test]
    fn shortcut_register_skill_layout() {
        let mut w = PacketWriter::new();
        w.write_u8(0x44);
        w.write_i32(2); // SKILL
        w.write_i32(1); // slot + page*12
        w.write_i32(1177);
        w.write_i16(2);
        w.write_i16(0);
        w.write_i32(-1);
        w.write_u8(0); // C5
        w.write_i32(1); // character type
        w.write_i32(0); // if 1 - can't use
        w.write_i32(0); // reuse delay ?
        assert_eq!(super::shortcut_register(&skill_shortcut()), w.into_bytes());
    }

    fn test_macro() -> Macro {
        Macro {
            id: 1000,
            icon: 3,
            name: "aa".into(),
            descr: "bb".into(),
            acronym: "cc".into(),
            commands: vec![MacroCmd {
                entry: 0,
                kind: MacroType::Skill,
                d1: 1177,
                d2: 1,
                cmd: String::new(),
            }],
        }
    }

    #[test]
    fn send_macro_list_add_layout() {
        let m = test_macro();
        let mut w = PacketWriter::new();
        w.write_u8(0xE8);
        w.write_u8(1); // ADD
        w.write_i32(1000); // the created macro's id
        w.write_u8(1); // count
        w.write_u8(1); // has macro
        w.write_i32(1000);
        w.write_string("aa");
        w.write_string("bb");
        w.write_string("cc");
        w.write_i32(3); // icon
        w.write_u8(1); // command count
        w.write_u8(1); // running index (1-based)
        w.write_u8(1); // MacroType.SKILL ordinal
        w.write_i32(1177);
        w.write_u8(1); // d2
        w.write_string("");
        assert_eq!(
            super::send_macro_list(1, Some(&m), MacroUpdateType::Add),
            w.into_bytes()
        );
    }

    #[test]
    fn send_macro_list_delete_has_no_body() {
        let m = test_macro();
        let mut w = PacketWriter::new();
        w.write_u8(0xE8);
        w.write_u8(0); // DELETE
        w.write_i32(1000); // the deleted macro's id
        w.write_u8(0); // count
        w.write_u8(1); // macro non-null, body still omitted
        assert_eq!(
            super::send_macro_list(0, Some(&m), MacroUpdateType::Delete),
            w.into_bytes()
        );
    }

    #[test]
    fn send_all_macros_empty_and_counted() {
        // No macros: a single empty LIST packet.
        let empty = super::send_all_macros(&Macros::default());
        let mut w = PacketWriter::new();
        w.write_u8(0xE8);
        w.write_u8(1); // LIST
        w.write_i32(0); // no macro id in LIST entries
        w.write_u8(0); // count
        w.write_u8(0); // no macro
        assert_eq!(empty, vec![w.into_bytes()]);

        // Two macros: one packet each, both carrying the total count.
        let m2 = Macro {
            id: 1001,
            ..test_macro()
        };
        let macros = Macros {
            next_id: 1002,
            entries: vec![test_macro(), m2],
        };
        let pkts = super::send_all_macros(&macros);
        assert_eq!(pkts.len(), 2);
        for pkt in &pkts {
            assert_eq!(pkt[0], 0xE8);
            assert_eq!(pkt[1], 1); // LIST
            assert_eq!(i32::from_le_bytes([pkt[2], pkt[3], pkt[4], pkt[5]]), 0);
            assert_eq!(pkt[6], 2); // total count
            assert_eq!(pkt[7], 1); // has macro
        }
    }
}
