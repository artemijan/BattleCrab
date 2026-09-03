//! Trading rows: offline traders, freight, buy-list stock, the manor and
//! item auctions.

use super::super::FreightItemRow;
use super::super::ItemRow;
use super::super::ManorProcureRow;
use super::super::ManorProductionRow;
use super::super::item_row_model;
use super::super::warn_err;
use models::entity;
use models::sea_orm::ActiveValue::Set;
use models::sea_orm::DatabaseConnection;
use models::sea_orm::sea_query::OnConflict;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

pub(super) async fn save_buy_list_stock(
    db: &DatabaseConnection,
    list_id: i32,
    item_id: i32,
    count: i64,
    next_restock_time: i64,
) {
    warn_err(
        entity::buylists::Entity::insert(entity::buylists::ActiveModel {
            buylist_id: Set(list_id),
            item_id: Set(item_id),
            count: Set(count),
            next_restock_time: Set(next_restock_time),
        })
        .on_conflict(
            OnConflict::columns([
                entity::buylists::Column::BuylistId,
                entity::buylists::Column::ItemId,
            ])
            .update_columns([
                entity::buylists::Column::Count,
                entity::buylists::Column::NextRestockTime,
            ])
            .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn add_freight_items(
    db: &DatabaseConnection,
    owner_id: i32,
    items: Vec<FreightItemRow>,
) {
    for it in &items {
        warn_err(
            entity::items::Entity::insert(entity::items::ActiveModel {
                owner_id: Set(Some(owner_id)),
                object_id: Set(it.object_id),
                item_id: Set(Some(it.item_id)),
                count: Set(it.count),
                enchant_level: Set(Some(it.enchant_level)),
                loc: Set(Some("FREIGHT".to_string())),
                loc_data: Set(Some(0)),
                custom_type1: Set(Some(0)),
                custom_type2: Set(Some(0)),
                mana_left: Set(-1),
                time: Set(0),
                ..Default::default()
            })
            .exec(db)
            .await,
        );
    }
}

pub(super) async fn store_offline_trader(
    db: &DatabaseConnection,
    char_id: i32,
    time: i64,
    store_type: i32,
    title: String,
    items: Vec<(i32, i64, i64)>,
) {
    // Java rewrites both tables for this trader (`onTransaction`
    // clears the item rows first, then re-inserts).
    warn_err(
        entity::character_offline_trade_items::Entity::delete_many()
            .filter(entity::character_offline_trade_items::Column::CharId.eq(char_id))
            .exec(db)
            .await,
    );
    warn_err(
        entity::character_offline_trade::Entity::delete_many()
            .filter(entity::character_offline_trade::Column::CharId.eq(char_id))
            .exec(db)
            .await,
    );
    warn_err(
        entity::character_offline_trade::Entity::insert(
            entity::character_offline_trade::ActiveModel {
                char_id: Set(char_id),
                time: Set(time),
                r#type: Set(store_type),
                title: Set(Some(title)),
            },
        )
        .exec(db)
        .await,
    );
    for (item, count, price) in &items {
        warn_err(
            entity::character_offline_trade_items::Entity::insert(
                entity::character_offline_trade_items::ActiveModel {
                    char_id: Set(char_id),
                    item: Set(*item),
                    count: Set(*count),
                    price: Set(*price),
                },
            )
            .exec(db)
            .await,
        );
    }
}

pub(super) async fn clear_offline_trader(db: &DatabaseConnection, char_id: i32) {
    warn_err(
        entity::character_offline_trade_items::Entity::delete_many()
            .filter(entity::character_offline_trade_items::Column::CharId.eq(char_id))
            .exec(db)
            .await,
    );
    warn_err(
        entity::character_offline_trade::Entity::delete_many()
            .filter(entity::character_offline_trade::Column::CharId.eq(char_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn store_manor(
    db: &DatabaseConnection,
    castle_id: i32,
    production: Vec<ManorProductionRow>,
    procure: Vec<ManorProcureRow>,
) {
    warn_err(
        entity::castle_manor_production::Entity::delete_many()
            .filter(entity::castle_manor_production::Column::CastleId.eq(castle_id))
            .exec(db)
            .await,
    );
    for r in &production {
        warn_err(
            entity::castle_manor_production::Entity::insert(
                entity::castle_manor_production::ActiveModel {
                    castle_id: Set(r.castle_id),
                    seed_id: Set(r.seed_id),
                    amount: Set(r.amount as i32),
                    start_amount: Set(r.start_amount as i32),
                    price: Set(r.price as i32),
                    next_period: Set(i32::from(r.next_period)),
                },
            )
            .exec(db)
            .await,
        );
    }
    warn_err(
        entity::castle_manor_procure::Entity::delete_many()
            .filter(entity::castle_manor_procure::Column::CastleId.eq(castle_id))
            .exec(db)
            .await,
    );
    for r in &procure {
        warn_err(
            entity::castle_manor_procure::Entity::insert(
                entity::castle_manor_procure::ActiveModel {
                    castle_id: Set(r.castle_id),
                    crop_id: Set(r.crop_id),
                    amount: Set(r.amount as i32),
                    start_amount: Set(r.start_amount as i32),
                    price: Set(r.price as i32),
                    reward_type: Set(r.reward_type),
                    next_period: Set(i32::from(r.next_period)),
                },
            )
            .exec(db)
            .await,
        );
    }
}

pub(super) async fn store_offline_warehouse_items(
    db: &DatabaseConnection,
    owner_id: i32,
    items: Vec<ItemRow>,
) {
    for it in &items {
        warn_err(
            entity::items::Entity::insert(item_row_model(owner_id, it, Some(("WAREHOUSE", 0))))
                .on_conflict(
                    OnConflict::column(entity::items::Column::ObjectId)
                        .update_columns([
                            entity::items::Column::OwnerId,
                            entity::items::Column::ItemId,
                            entity::items::Column::Count,
                            entity::items::Column::EnchantLevel,
                            entity::items::Column::Loc,
                            entity::items::Column::LocData,
                            entity::items::Column::CustomType1,
                            entity::items::Column::CustomType2,
                            entity::items::Column::ManaLeft,
                            entity::items::Column::Time,
                        ])
                        .to_owned(),
                )
                .exec(db)
                .await,
        );
    }
}

pub(super) async fn store_item_auction(
    db: &DatabaseConnection,
    auction_id: i32,
    instance_id: i32,
    auction_item_id: i32,
    starting_time: i64,
    ending_time: i64,
    state_id: i8,
) {
    warn_err(
        entity::item_auction::Entity::insert(entity::item_auction::ActiveModel {
            auction_id: Set(auction_id),
            instance_id: Set(instance_id),
            auction_item_id: Set(auction_item_id),
            starting_time: Set(starting_time),
            ending_time: Set(ending_time),
            auction_state_id: Set(state_id.into()),
        })
        .on_conflict(
            OnConflict::column(entity::item_auction::Column::AuctionId)
                .update_columns([
                    entity::item_auction::Column::InstanceId,
                    entity::item_auction::Column::AuctionItemId,
                    entity::item_auction::Column::StartingTime,
                    entity::item_auction::Column::EndingTime,
                    entity::item_auction::Column::AuctionStateId,
                ])
                .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn store_item_auction_bid(
    db: &DatabaseConnection,
    auction_id: i32,
    player_obj_id: i32,
    bid: i64,
) {
    warn_err(
        entity::item_auction_bid::Entity::insert(entity::item_auction_bid::ActiveModel {
            auction_id: Set(auction_id),
            player_obj_id: Set(player_obj_id),
            player_bid: Set(bid),
        })
        .on_conflict(
            OnConflict::columns([
                entity::item_auction_bid::Column::AuctionId,
                entity::item_auction_bid::Column::PlayerObjId,
            ])
            .update_column(entity::item_auction_bid::Column::PlayerBid)
            .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn delete_item_auction_bid(
    db: &DatabaseConnection,
    auction_id: i32,
    player_obj_id: i32,
) {
    warn_err(
        entity::item_auction_bid::Entity::delete_by_id((auction_id, player_obj_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn delete_item_auction(db: &DatabaseConnection, auction_id: i32) {
    warn_err(
        entity::item_auction::Entity::delete_by_id(auction_id)
            .exec(db)
            .await,
    );
    warn_err(
        entity::item_auction_bid::Entity::delete_many()
            .filter(entity::item_auction_bid::Column::AuctionId.eq(auction_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn store_offline_warehouse_item(
    db: &DatabaseConnection,
    owner_id: i32,
    object_id: i32,
    item_id: i32,
    count: i64,
    enchant: i32,
) {
    warn_err(
        entity::items::Entity::insert(entity::items::ActiveModel {
            owner_id: Set(Some(owner_id)),
            object_id: Set(object_id),
            item_id: Set(Some(item_id)),
            count: Set(count),
            enchant_level: Set(Some(enchant)),
            loc: Set(Some("WAREHOUSE".to_string())),
            loc_data: Set(Some(0)),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::column(entity::items::Column::ObjectId)
                .update_columns([
                    entity::items::Column::OwnerId,
                    entity::items::Column::ItemId,
                    entity::items::Column::Count,
                    entity::items::Column::EnchantLevel,
                    entity::items::Column::Loc,
                    entity::items::Column::LocData,
                ])
                .to_owned(),
        )
        .exec(db)
        .await,
    );
}
