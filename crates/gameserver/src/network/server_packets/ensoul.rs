//! Soul crystal (ensoul) server packets: the two NPC windows and the two
//! results.

use super::ex;
use super::opcodes;

/// `ExShowEnsoulWindow` — open the soul crystal window. Empty body.
pub fn ex_show_ensoul_window() -> Vec<u8> {
    ex(opcodes::EX_SHOW_ENSOUL_WINDOW).into_bytes()
}

/// `ExShowEnsoulExtractionWindow` — open the extraction window. Empty body.
pub fn ex_show_ensoul_extraction_window() -> Vec<u8> {
    ex(opcodes::EX_ENSOUL_EXTRACTION_SHOW).into_bytes()
}

/// `ExEnsoulResult` — the success byte, then the item's regular and special
/// options, each list a count byte and the ids.
pub fn ex_ensoul_result(success: bool, regular: &[i32], special: &[i32]) -> Vec<u8> {
    let mut w = ex(opcodes::EX_ENSOUL_RESULT);
    w.write_u8(u8::from(success));
    write_options(&mut w, regular, special);
    w.into_bytes()
}

/// `ExEnSoulExtractionResult` — the success byte; only a success carries the
/// item's two option lists.
pub fn ex_ensoul_extraction_result(success: bool, regular: &[i32], special: &[i32]) -> Vec<u8> {
    let mut w = ex(opcodes::EX_ENSOUL_EXTRACTION_RESULT);
    w.write_u8(u8::from(success));
    if success {
        write_options(&mut w, regular, special);
    }
    w.into_bytes()
}

fn write_options(w: &mut commons::network::PacketWriter, regular: &[i32], special: &[i32]) {
    for list in [regular, special] {
        w.write_u8(list.len() as u8);
        for &id in list {
            w.write_i32(id);
        }
    }
}
