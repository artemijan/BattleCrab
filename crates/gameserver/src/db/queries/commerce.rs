//! Offline traders, buy-list stock and item auctions.

use super::super::CharData;
use super::character_load::load_character;
use models::entity;

use models::sea_orm::DatabaseConnection;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use tracing::warn;

/// One row of `character_offline_trade` with its `character_offline_trade_items`
/// lines and the full character behind it.
#[derive(Debug, Clone)]
pub struct OfflineTraderRow {
    pub char: CharData,
    /// `time` — when the shop first went offline.
    pub time: i64,
    /// `type` — a `PrivateStoreType` id.
    pub store_type: i32,
    pub title: String,
    /// `(item, count, price)` — see [`DbCommand::StoreOfflineTrader`].
    pub items: Vec<(i32, i64, i64)>,
}
pub(crate) async fn load_item_auctions(
    db: &DatabaseConnection,
) -> (i32, Vec<crate::model::item_auction::ItemAuction>) {
    use crate::model::item_auction::{AuctionState, ItemAuction, ItemAuctionBid};

    let mut auctions: Vec<ItemAuction> = entity::item_auction::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|row| {
            let state = AuctionState::from_state_id(row.auction_state_id as i8)?;
            Some(ItemAuction::new(
                row.auction_id,
                row.instance_id,
                row.auction_item_id,
                row.starting_time,
                row.ending_time,
                state,
            ))
        })
        .collect();

    // Attach each auction's bids.
    for bid in entity::item_auction_bid::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
    {
        if let Some(a) = auctions.iter_mut().find(|a| a.auction_id == bid.auction_id) {
            a.bids.push(ItemAuctionBid {
                player_obj_id: bid.player_obj_id,
                last_bid: bid.player_bid,
            });
        }
    }

    let next_id = auctions.iter().map(|a| a.auction_id).max().unwrap_or(0) + 1;
    (next_id, auctions)
}
/// `LOAD_OFFLINE_STATUS` + `LOAD_OFFLINE_ITEMS`, joined per trader. A row whose
/// character no longer exists is dropped (Java's `Player.load` returning null
/// lands in its catch block).
pub(crate) async fn load_offline_traders(db: &DatabaseConnection) -> Vec<OfflineTraderRow> {
    let rows = entity::character_offline_trade::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    let mut out = Vec::new();
    for row in rows {
        let items = entity::character_offline_trade_items::Entity::find()
            .filter(entity::character_offline_trade_items::Column::CharId.eq(row.char_id))
            .all(db)
            .await
            .unwrap_or_default();
        let Some(char) = load_character(db, row.char_id).await else {
            warn!(
                "DB thread: offline shop for missing character {}; skipped.",
                row.char_id
            );
            continue;
        };
        out.push(OfflineTraderRow {
            char,
            time: row.time,
            store_type: row.r#type,
            title: row.title.unwrap_or_default(),
            items: items
                .into_iter()
                .map(|i| (i.item, i.count, i.price))
                .collect(),
        });
    }
    out
}
/// `ClanTable`'s boot restore: every `clan_data` row + its member roster
/// from `characters WHERE clanid=?` (Java `Clan.restore`).
/// `SELECT * FROM buylists` — the stock counters `BuyListData.load` restores
/// after parsing the XML. Rows for lists or items the datapack no longer
/// declares are dropped on the game thread, where the lists are.
pub(crate) async fn load_buy_list_stock(db: &DatabaseConnection) -> Vec<(i32, i32, i64, i64)> {
    entity::buylists::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.buylist_id, r.item_id, r.count, r.next_restock_time))
        .collect()
}
