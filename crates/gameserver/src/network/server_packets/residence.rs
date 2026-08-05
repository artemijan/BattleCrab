//! Castle / fortress residence packets — the world map's ownership overlay.
//! Ported from `serverpackets/{ExShowCastleInfo,ExShowFortressInfo}`.
//!
//! The castle overlay carries live data: owner, tax, siege schedule and side.
//! The **fortress** overlay is still the static all-unowned list, and that is
//! not a deferral — fort sieges are an explicit scope-out (PORTING_STATUS.md's
//! out-of-scope table: off-chronicle for this build), so no fort on this dist
//! can ever have an owner. The wire shape matches a fresh Java DB.

use commons::network::PacketWriter;

use super::opcodes;
use crate::model::castle::TaxType;
use crate::world::World;

/// The twenty-one fortresses (`fort` table ids 101..=121).
const FORT_IDS: std::ops::RangeInclusive<i32> = 101..=121;

/// Port of `serverpackets/ExShowCastleInfo` — per castle: id, owner clan
/// name, buy-tax percent, siege date (epoch seconds), siege-in-progress and
/// castle-side bytes.
///
/// Java iterates `CastleManager.getCastles()`, so the count and order are the
/// server's castle list rather than a hard-coded 1..=9 range — this port's
/// `world.castles` is that list.
///
/// Two details worth keeping: the owner name is looked up through the clan
/// table and Java writes an **empty string** (with a warning) when the id has
/// no clan behind it, so a dangling owner id must not drop the entry or shift
/// the fields after it; and the siege date goes out in **seconds**, not the
/// milliseconds it is stored in.
pub fn ex_show_castle_info(world: &World) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SHOW_CASTLE_INFO);
    w.write_i32(world.castles.len() as i32);
    for castle in &world.castles {
        w.write_i32(castle.id);
        // `ClanTable.getClan(castle.getOwnerId()).getName()`, via the clan's
        // own `castle_id` back-reference. Java's else-branch writes "" rather
        // than skipping, so an ownerless castle still occupies its slot.
        let owner = world
            .clans
            .values()
            .find(|c| c.castle_id == castle.id)
            .map(|c| c.name.as_str())
            .unwrap_or("");
        w.write_string(owner);
        w.write_i32(crate::game_loop::castle::tax_percent(
            world,
            castle.id,
            TaxType::Buy,
        ));
        // `getSiegeDate().getTimeInMillis() / 1000`.
        w.write_i32((castle.siege_date / 1000) as i32);
        w.write_u8(u8::from(
            world.sieges.get(&castle.id).is_some_and(|s| s.in_progress),
        ));
        w.write_u8(castle.side as u8);
    }
    w.into_bytes()
}

/// Port of `serverpackets/ExShowFortressInfo` — per fort: id, owner clan
/// name, siege-in-progress int, owned-time seconds.
pub fn ex_show_fortress_info() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SHOW_FORTRESS_INFO);
    w.write_i32(FORT_IDS.count() as i32);
    for id in FORT_IDS {
        w.write_i32(id);
        w.write_string(""); // owner clan name (unowned)
        w.write_i32(0); // siege in progress
        w.write_i32(0); // owned time
    }
    w.into_bytes()
}
