//! Small ports of `gameserver/enums` needed so far.

/// Port of `enums/Race` (ordinals matter — sent on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Race {
    Human = 0,
    Elf = 1,
    DarkElf = 2,
    Orc = 3,
    Dwarf = 4,
    Kamael = 5,
    Ertheia = 6,
}

impl Race {
    pub fn ordinal(self) -> i32 {
        self as i32
    }
}
