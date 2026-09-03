//! Inventory packets: using, dropping and destroying an item, the order
//! the client wants it listed in, augmentation and multisell.

use commons::network::PacketReader;

/// Port of `clientpackets/UseItem` (`cdc`): the target item's object id, plus
/// a ctrl-pressed flag (used for split-stack prompts — not needed while gear
/// is the only thing `UseItem` acts on).
pub struct UseItem {
    pub object_id: i32,
    pub ctrl_pressed: bool,
}

impl UseItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let object_id = r.read_i32()?;
        let ctrl_pressed = r.read_i32()? != 0;
        Some(Self {
            object_id,
            ctrl_pressed,
        })
    }
}

/// Port of `clientpackets/RequestSaveInventoryOrder` (`d[dd]`): the client's
/// custom inventory arrangement — one `(object_id, order)` pair per grid slot.
/// `order` is the slot index the client wants that item stored at (`items.
/// loc_data` for `INVENTORY`-located items). Java caps the count at `LIMIT`
/// (125) and silently drops the overflow.
pub struct RequestSaveInventoryOrder {
    pub order: Vec<(i32, i32)>,
}

impl RequestSaveInventoryOrder {
    const LIMIT: usize = 125;

    pub fn read(ex_body: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(ex_body);
        let count = (r.read_i32()? as usize).min(Self::LIMIT);
        let mut order = Vec::with_capacity(count);
        for _ in 0..count {
            let object_id = r.read_i32()?;
            let slot = r.read_i32()?;
            order.push((object_id, slot));
        }
        Some(Self { order })
    }
}

/// Port of `clientpackets/RequestDestroyItem` (`dq`): the inventory item object
/// id and the count to destroy.
pub struct RequestDestroyItem {
    pub object_id: i32,
    pub count: i64,
}

impl RequestDestroyItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let object_id = r.read_i32()?;
        let count = r.read_i64()?;
        Some(Self { object_id, count })
    }
}

/// Read `count` `(object id, count)` pairs — the item-list body every
/// item-moving request carries (warehouse deposit and withdraw, freight, mail
/// attachments).
///
/// A malformed line rejects the **whole** packet rather than being skipped, as
/// Java does. That is the part worth stating: taking the valid prefix of a
/// tampered list is how a client talks the server into a partial warehouse
/// transfer, and a partial transfer is a duplication bug.
///
/// The per-request cap on `count` stays at the call site — 500 for a warehouse
/// list, `MAX_ATTACHMENTS * 4` for mail — because those are different limits
/// with different system messages behind them. The preallocation is clamped
/// regardless, so a caller that forgets to check cannot turn a four-byte header
/// into a large allocation.
pub(crate) fn read_item_lines(r: &mut PacketReader, count: i32) -> Option<Vec<(i32, i64)>> {
    let mut items = Vec::with_capacity(count.clamp(0, 512) as usize);
    for _ in 0..count {
        let object_id = r.read_i32()?;
        let cnt = r.read_i64()?;
        if object_id < 1 || cnt < 0 {
            return None;
        }
        items.push((object_id, cnt));
    }
    Some(items)
}

/// Port of Java's `AbstractRefinePacket` body (`dddq`) — the four fields every
/// augment-window request carries: the weapon being augmented, the life stone,
/// the gemstone offered as the fee, and how many.
///
/// `RequestRefine` and `RequestConfirmGemStone` send exactly this and nothing
/// else, which is why Java gives them a shared base class rather than two
/// readers. The two handlers then diverge completely — one rolls the augment,
/// the other only echoes the fee back — so what is shared is the body, not the
/// behaviour.
///
/// Note this is *not* the shape of every `dddq` in the protocol: the manor's
/// crop-sale lines read the same four widths and mean something entirely
/// different, and they validate ranges this does not.
pub struct RefineRequest {
    /// The item being augmented.
    pub target_obj: i32,
    /// The life stone.
    pub mineral_obj: i32,
    /// The gemstone offered against the fee.
    pub fee_obj: i32,
    pub fee_count: i64,
}

impl RefineRequest {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            target_obj: r.read_i32()?,
            mineral_obj: r.read_i32()?,
            fee_obj: r.read_i32()?,
            fee_count: r.read_i64()?,
        })
    }
}

/// Port of `clientpackets/MultiSellChoose`: the item exchange click. Reads the
/// full retail body (enchant/augment/elemental stats follow the amount);
/// `enchant_level` feeds the `maintainEnchantment` validation in
/// `multisell::handle_multi_sell_choose` (the echoed level must match the
/// paired inventory row, and `amount` must be 1 for such lists).
pub struct MultiSellChoose {
    pub list_id: i32,
    pub entry_id: i32,
    pub amount: i64,
    /// The enchant level the client echoes back for the row it clicked. Java
    /// refuses the exchange when it disagrees with the item the inventory-only
    /// window paired with that row.
    pub enchant_level: i32,
}

impl MultiSellChoose {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let list_id = r.read_i32()?;
        let entry_id = r.read_i32()?;
        let amount = r.read_i64()?;
        let enchant_level = i32::from(r.read_i16()?);
        // augment1(int), augment2(int), attackAttribute(short), attributePower
        // (short) and six elemental defence shorts — read to keep the reader
        // honest; augments/attributes aren't compared on this path (no dist
        // multisell carries them as ingredients).
        let _augment1 = r.read_i32()?;
        let _augment2 = r.read_i32()?;
        for _ in 0..8 {
            let _ = r.read_i16()?;
        }
        Some(Self {
            list_id,
            entry_id,
            amount,
            enchant_level,
        })
    }
}

/// Port of `clientpackets/RequestDropItem` (`dqddd`): item object id, count,
/// and the requested drop location.
pub struct RequestDropItem {
    pub object_id: i32,
    pub count: i64,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl RequestDropItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let object_id = r.read_i32()?;
        let count = r.read_i64()?;
        let x = r.read_i32()?;
        let y = r.read_i32()?;
        let z = r.read_i32()?;
        Some(Self {
            object_id,
            count,
            x,
            y,
            z,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commons::network::PacketWriter;

    fn save_order_body(count: i32, pairs: &[(i32, i32)]) -> Vec<u8> {
        let mut w = PacketWriter::new();
        w.write_i32(count);
        for &(object_id, order) in pairs {
            w.write_i32(object_id);
            w.write_i32(order);
        }
        w.into_bytes()
    }

    #[test]
    fn save_inventory_order_reads_pairs() {
        let pairs = [(1000, 0), (1001, 2), (1002, 1)];
        let body = save_order_body(pairs.len() as i32, &pairs);
        let pkt = RequestSaveInventoryOrder::read(&body).expect("parses");
        assert_eq!(pkt.order, pairs);
    }

    #[test]
    fn save_inventory_order_caps_at_limit() {
        // A count above LIMIT reads exactly LIMIT pairs; trailing pairs the
        // client sent past the cap are ignored (matches Java's `Math.min`).
        let pairs: Vec<(i32, i32)> = (0..RequestSaveInventoryOrder::LIMIT as i32 + 10)
            .map(|i| (2000 + i, i))
            .collect();
        let body = save_order_body(pairs.len() as i32, &pairs);
        let pkt = RequestSaveInventoryOrder::read(&body).expect("parses");
        assert_eq!(pkt.order.len(), RequestSaveInventoryOrder::LIMIT);
        assert_eq!(pkt.order, pairs[..RequestSaveInventoryOrder::LIMIT]);
    }

    #[test]
    fn save_inventory_order_rejects_truncated() {
        // Claims two pairs but only supplies one.
        let body = save_order_body(2, &[(1000, 0)]);
        assert!(RequestSaveInventoryOrder::read(&body).is_none());
    }
}
