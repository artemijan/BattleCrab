//! Buying, selling and the player-run shops: NPC buy/sell/refund, private
//! store and manufacture lists, warehouse and trade.

use super::items::read_item_lines;
use commons::network::PacketReader;

/// One purchase line of `RequestBuyItem`.
pub struct BuyLine {
    pub item_id: i32,
    pub count: i64,
}

/// Port of `clientpackets/RequestBuyItem.readImpl`: list id + item lines;
/// any non-positive id/count invalidates the whole request (Java nulls
/// `_items` and the handler answers ActionFailed — here the packet just
/// fails to parse, same net effect as the guards re-run in the handler).
pub struct RequestBuyItem {
    pub list_id: i32,
    pub items: Vec<BuyLine>,
}

impl RequestBuyItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let list_id = r.read_i32()?;
        let size = r.read_i32()?;
        // Java: `(size > 500) || ((size * 12) != remaining)` drops the packet.
        if size <= 0 || size > 500 {
            return None;
        }
        let mut items = Vec::with_capacity(size as usize);
        for _ in 0..size {
            let item_id = r.read_i32()?;
            let count = r.read_i64()?;
            if item_id < 1 || count < 1 {
                return None;
            }
            items.push(BuyLine { item_id, count });
        }
        Some(Self { list_id, items })
    }
}

/// Port of `SendWareHouseDepositList` / `SendWareHouseWithDrawList` (`d[dq]`):
/// a count-prefixed list of `(object_id, count)` pairs — the items to move into
/// or out of the warehouse.
pub struct WarehouseItemList {
    pub items: Vec<(i32, i64)>,
}

impl WarehouseItemList {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let count = r.read_i32()?;
        if count <= 0 || count > 500 {
            return None;
        }
        let items = read_item_lines(&mut r, count)?;
        Some(Self { items })
    }
}

/// Port of `clientpackets/AddTradeItem` (`ddq`): the trade id, the inventory
/// item object id, and how many to add.
pub struct AddTradeItem {
    pub object_id: i32,
    pub count: i64,
}

impl AddTradeItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        r.read_i32()?; // trade id — unused (one active trade per player)
        let object_id = r.read_i32()?;
        let count = r.read_i64()?;
        Some(Self { object_id, count })
    }
}

/// Port of `SetPrivateStoreListSell` (`dd [dqq]`): the items to offer —
/// `(object_id, count, price)`. `RequestPrivateStoreBuy` (`dd [dqq]`) shares the
/// same trailing layout but leads with the seller's object id.
/// One offered line of `RequestPrivateStoreSell`.
pub struct StoreSellLine {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub price: i64,
}

/// `RequestPrivateStoreSell` — a customer filling someone's buy store.
pub struct StoreSellRequest {
    pub store_player: i32,
    pub items: Vec<StoreSellLine>,
}

/// One line of a buy store's wanted list (`SetPrivateStoreListBuy`).
pub struct WantedLine {
    pub item_id: i32,
    pub enchant: i32,
    pub count: i64,
    pub price: i64,
}

pub struct PrivateStoreItemList {
    /// `RequestPrivateStoreBuy` only: the seller's object id (`0` for a set-list).
    pub target_object_id: i32,
    pub items: Vec<(i32, i64, i64)>,
}

impl PrivateStoreItemList {
    /// `SetPrivateStoreListSell`: `packageSale(int)` then the item lines.
    /// Returns the leading **package-sale** flag alongside the lines: `1` opens
    /// a `PACKAGE_SELL` store (Java `SetPrivateStoreListSell._packageSale`).
    pub fn read_set_list(body_after_opcode: &[u8]) -> Option<(bool, Self)> {
        let mut r = PacketReader::new(body_after_opcode);
        let packaged = r.read_i32()? == 1;
        Self::read_lines(&mut r, 0).map(|lines| (packaged, lines))
    }

    /// `RequestPrivateStoreBuy`: `storePlayerId(int)` then the item lines.
    pub fn read_buy(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let seller = r.read_i32()?;
        Self::read_lines(&mut r, seller)
    }

    /// `SetPrivateStoreListBuy`: the wanted lines, keyed by **item id** (the
    /// owner doesn't own them yet) with the client's enchant/augment/element
    /// tail that this port ignores.
    pub fn read_set_list_buy(body_after_opcode: &[u8]) -> Option<Vec<WantedLine>> {
        let mut r = PacketReader::new(body_after_opcode);
        let count = r.read_i32()?;
        if !(1..=500).contains(&count) {
            return None;
        }
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let item_id = r.read_i32()?;
            let enchant = r.read_i16()? as i32;
            let _unknown = r.read_i16()?;
            let cnt = r.read_i64()?;
            let price = r.read_i64()?;
            let _option1 = r.read_i32()?;
            let _option2 = r.read_i32()?;
            // attack element (id + power) then the six defence elements.
            for _ in 0..8 {
                r.read_i16()?;
            }
            let _visual_id = r.read_i32()?;
            if item_id < 1 || cnt < 1 || price < 0 {
                return None;
            }
            items.push(WantedLine {
                item_id,
                enchant,
                count: cnt,
                price,
            });
        }
        Some(items)
    }

    /// `RequestPrivateStoreSell`: the store owner's object id, then the lines
    /// the customer offers — inventory object id, item id, count and the price
    /// the client believes the store pays, plus a soul-crystal/SA tail this
    /// port skips.
    pub fn read_store_sell(body_after_opcode: &[u8]) -> Option<StoreSellRequest> {
        let mut r = PacketReader::new(body_after_opcode);
        let store_player = r.read_i32()?;
        let count = r.read_i32()?;
        if !(1..=500).contains(&count) {
            return None;
        }
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let object_id = r.read_i32()?;
            let item_id = r.read_i32()?;
            let _enchant = r.read_i16()?;
            let _unknown = r.read_i16()?;
            let cnt = r.read_i64()?;
            let price = r.read_i64()?;
            let _visual = r.read_i32()?;
            let _option1 = r.read_i32()?;
            let _option2 = r.read_i32()?;
            // Two length-prefixed tails (soul-crystal options, SA effects).
            for _ in 0..2 {
                let extra = r.read_u8()? as i32;
                for _ in 0..extra {
                    r.read_i32()?;
                }
            }
            if item_id < 1 || cnt < 1 || price < 0 {
                return None;
            }
            items.push(StoreSellLine {
                object_id,
                item_id,
                count: cnt,
                price,
            });
        }
        Some(StoreSellRequest {
            store_player,
            items,
        })
    }

    fn read_lines(r: &mut PacketReader, target: i32) -> Option<Self> {
        let count = r.read_i32()?;
        if !(1..=500).contains(&count) {
            return None;
        }
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let object_id = r.read_i32()?;
            let cnt = r.read_i64()?;
            let price = r.read_i64()?;
            if object_id < 1 || cnt < 1 || price < 0 {
                return None;
            }
            items.push((object_id, cnt, price));
        }
        Some(Self {
            target_object_id: target,
            items,
        })
    }
}

/// Port of `clientpackets/RequestSellItem` (`dd [dq]`... actually `ddd q` per
/// entry): the buy-list id and the items to sell — `(object_id, item_id, count)`.
pub struct RequestSellItem {
    pub list_id: i32,
    pub items: Vec<(i32, i32, i64)>,
}

impl RequestSellItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let list_id = r.read_i32()?;
        let size = r.read_i32()?;
        if size <= 0 || size > 500 {
            return None;
        }
        let mut items = Vec::with_capacity(size as usize);
        for _ in 0..size {
            let object_id = r.read_i32()?;
            let item_id = r.read_i32()?;
            let count = r.read_i64()?;
            if object_id < 1 || item_id < 1 || count < 1 {
                return None;
            }
            items.push((object_id, item_id, count));
        }
        Some(Self { list_id, items })
    }
}

/// Port of `clientpackets/RequestRefundItem` (ex 0x72): buy back items from
/// the refund tab — the buy-list id and the refund-list positions to reclaim.
pub struct RequestRefundItem {
    #[allow(dead_code)] // Java validates it against BuyListData; we don't (yet).
    pub list_id: i32,
    pub indexes: Vec<i32>,
}

impl RequestRefundItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let list_id = r.read_i32()?;
        let count = r.read_i32()?;
        if count <= 0 || count > 500 {
            return None;
        }
        let mut indexes = Vec::with_capacity(count as usize);
        for _ in 0..count {
            indexes.push(r.read_i32()?);
        }
        Some(Self { list_id, indexes })
    }
}

/// One line of a `RequestRecipeShopListSet` manufacture list: recipe-list id +
/// adena cost.
#[derive(Debug, Clone, Copy)]
pub struct ManufactureLine {
    pub recipe_id: i32,
    pub cost: i64,
}

/// `RequestRecipeShopListSet` (0xBB): the manufacture recipes + prices the
/// seller set. Java: `count(int)` then `count × (id:int, cost:long)`; a
/// negative cost aborts the whole read (Java nulls `_items`).
pub fn read_recipe_shop_list_set(body: &[u8]) -> Option<Vec<ManufactureLine>> {
    let mut r = PacketReader::new(body);
    let count = r.read_i32()?;
    if !(0..=500).contains(&count) {
        return None;
    }
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let recipe_id = r.read_i32()?;
        let cost = r.read_i64()?;
        if cost < 0 {
            return None;
        }
        items.push(ManufactureLine { recipe_id, cost });
    }
    Some(items)
}

/// `RequestRecipeShopMakeItem` (0xBF): `manufacturerId(int)`, `recipeId(int)`,
/// then an unused long.
pub fn read_recipe_shop_make_item(body: &[u8]) -> Option<(i32, i32)> {
    let mut r = PacketReader::new(body);
    let manufacturer = r.read_i32()?;
    let recipe_id = r.read_i32()?;
    let _unknown = r.read_i64()?;
    Some((manufacturer, recipe_id))
}

/// `RequestRecipeShopMakeInfo` (0xBE): `playerObjectId(int)`, `recipeId(int)`.
pub fn read_recipe_shop_make_info(body: &[u8]) -> Option<(i32, i32)> {
    let mut r = PacketReader::new(body);
    Some((r.read_i32()?, r.read_i32()?))
}

/// The single-int recipe packets (`RequestRecipeBookDestroy` 0xB6,
/// `RequestRecipeItemMakeInfo` 0xB7, `RequestRecipeItemMakeSelf` 0xB8): one int.
pub fn read_recipe_single_int(body: &[u8]) -> Option<i32> {
    PacketReader::new(body).read_i32()
}

/// `RequestRecipeShopMessageSet` (0xBA): the store title string.
pub fn read_recipe_shop_message_set(body: &[u8]) -> Option<String> {
    PacketReader::new(body).read_string()
}
