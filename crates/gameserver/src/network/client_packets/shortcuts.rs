//! Shortcut-bar and macro packets.

use commons::network::PacketReader;

/// Port of `clientpackets/RequestShortCutReg` — drag something onto a panel
/// slot. The combined slot int decodes to `slot % 12` / `page = slot / 12`;
/// the type id is clamped to NONE outside 1-6, both like Java.
pub struct RequestShortCutReg {
    pub kind: crate::model::shortcut::ShortcutType,
    pub slot: i32,
    pub page: i32,
    pub id: i32,
    pub level: i32,
    pub sub_level: i32,
    pub character_type: i32,
}

impl RequestShortCutReg {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let kind = crate::model::shortcut::ShortcutType::from_ordinal(r.read_i32()?);
        let slot_raw = r.read_i32()?;
        let id = r.read_i32()?;
        let level = r.read_i16()? as i32;
        let sub_level = r.read_i16()? as i32;
        let character_type = r.read_i32()?;
        Some(Self {
            kind,
            slot: slot_raw % 12,
            page: slot_raw / 12,
            id,
            level,
            sub_level,
            character_type,
        })
    }
}

/// Port of `clientpackets/RequestShortCutDel` — clear a panel slot.
pub struct RequestShortCutDel {
    pub slot: i32,
    pub page: i32,
}

impl RequestShortCutDel {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let slot_raw = r.read_i32()?;
        Some(Self {
            slot: slot_raw % 12,
            page: slot_raw / 12,
        })
    }
}

/// Port of `clientpackets/RequestMakeMacro` — create or edit a macro (macro
/// id 0 = create). The command count is hard-capped at 12
/// (`MAX_MACRO_LENGTH`); `commands_length` accumulates the command strings'
/// lengths for the 255-char validity gate.
pub struct RequestMakeMacro {
    pub macro_: crate::model::shortcut::Macro,
    pub commands_length: usize,
}

impl RequestMakeMacro {
    pub const MAX_MACRO_LENGTH: u8 = 12;

    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        use crate::model::shortcut::{Macro, MacroCmd, MacroType};
        let mut r = PacketReader::new(body_after_opcode);
        let id = r.read_i32()?;
        let name = r.read_string()?;
        let descr = r.read_string()?;
        let acronym = r.read_string()?;
        let icon = r.read_i32()?;
        let count = r.read_u8()?.min(Self::MAX_MACRO_LENGTH);
        let mut commands = Vec::with_capacity(count as usize);
        let mut commands_length = 0;
        for _ in 0..count {
            let entry = r.read_u8()? as i32;
            let kind = MacroType::from_ordinal(r.read_u8()? as i32);
            let d1 = r.read_i32()?;
            let d2 = r.read_u8()? as i32;
            let cmd = r.read_string()?;
            commands_length += cmd.chars().count();
            commands.push(MacroCmd {
                entry,
                kind,
                d1,
                d2,
                cmd,
            });
        }
        Some(Self {
            macro_: Macro {
                id,
                icon,
                name,
                descr,
                acronym,
                commands,
            },
            commands_length,
        })
    }
}

/// Port of `clientpackets/RequestDeleteMacro`.
pub struct RequestDeleteMacro {
    pub id: i32,
}

impl RequestDeleteMacro {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self { id: r.read_i32()? })
    }
}
