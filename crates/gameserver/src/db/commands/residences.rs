//! Castles, sieges, clan halls and cursed weapons — the residence-owned rows.

use super::super::warn_err;
use super::{set_castle_col, set_char_cols};
use models::entity;
use models::sea_orm::ActiveValue::Set;
use models::sea_orm::DatabaseConnection;
use models::sea_orm::sea_query::OnConflict;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

pub(super) async fn store_cursed_weapon(
    db: &DatabaseConnection,
    item_id: i32,
    char_id: i32,
    reputation: i32,
    pk_kills: i32,
    nb_kills: i32,
    end_time: i64,
) {
    warn_err(
        entity::cursed_weapons::Entity::insert(entity::cursed_weapons::ActiveModel {
            item_id: Set(item_id),
            char_id: Set(char_id),
            player_reputation: Set(Some(reputation)),
            player_pk_kills: Set(Some(pk_kills)),
            nb_kills: Set(Some(nb_kills)),
            end_time: Set(end_time),
        })
        .on_conflict(
            OnConflict::column(entity::cursed_weapons::Column::ItemId)
                .update_columns([
                    entity::cursed_weapons::Column::CharId,
                    entity::cursed_weapons::Column::PlayerReputation,
                    entity::cursed_weapons::Column::PlayerPkKills,
                    entity::cursed_weapons::Column::NbKills,
                    entity::cursed_weapons::Column::EndTime,
                ])
                .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn remove_cursed_weapon(db: &DatabaseConnection, item_id: i32) {
    warn_err(
        entity::cursed_weapons::Entity::delete_by_id(item_id)
            .exec(db)
            .await,
    );
}

pub(super) async fn restore_offline_cursed_owner(
    db: &DatabaseConnection,
    char_id: i32,
    item_id: i32,
    reputation: i32,
    pk_kills: i32,
    skill_ids: Vec<i32>,
) {
    warn_err(
        entity::items::Entity::delete_many()
            .filter(entity::items::Column::OwnerId.eq(char_id))
            .filter(entity::items::Column::ItemId.eq(item_id))
            .exec(db)
            .await,
    );
    set_char_cols(
        db,
        char_id,
        vec![
            (entity::characters::Column::Reputation, reputation.into()),
            (entity::characters::Column::Pkkills, pk_kills.into()),
        ],
    )
    .await;
    if !skill_ids.is_empty() {
        warn_err(
            entity::character_skills::Entity::delete_many()
                .filter(entity::character_skills::Column::CharId.eq(char_id))
                .filter(entity::character_skills::Column::SkillId.is_in(skill_ids))
                .exec(db)
                .await,
        );
    }
}

pub(super) async fn update_castle_side(db: &DatabaseConnection, castle_id: i32, side: String) {
    set_castle_col(db, castle_id, entity::castle::Column::Side, side.into()).await;
}

pub(super) async fn update_castle_show_npc_crest(
    db: &DatabaseConnection,
    castle_id: i32,
    show: bool,
) {
    set_castle_col(
        db,
        castle_id,
        entity::castle::Column::ShowNpcCrest,
        if show { "true" } else { "false" }.into(),
    )
    .await;
}

pub(super) async fn update_castle_ticket_count(
    db: &DatabaseConnection,
    castle_id: i32,
    count: i32,
) {
    set_castle_col(
        db,
        castle_id,
        entity::castle::Column::TicketBuyCount,
        count.into(),
    )
    .await;
}

pub(super) async fn add_hired_siege_guard(
    db: &DatabaseConnection,
    castle_id: i32,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
) {
    warn_err(
        entity::castle_siege_guards::Entity::insert(entity::castle_siege_guards::ActiveModel {
            castle_id: Set(castle_id),
            npc_id: Set(npc_id),
            x: Set(x),
            y: Set(y),
            z: Set(z),
            heading: Set(heading),
            respawn_delay: Set(0),
            is_hired: Set(1),
            ..Default::default()
        })
        .exec(db)
        .await,
    );
}

pub(super) async fn remove_hired_siege_guard(
    db: &DatabaseConnection,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
) {
    warn_err(
        entity::castle_siege_guards::Entity::delete_many()
            .filter(entity::castle_siege_guards::Column::NpcId.eq(npc_id))
            .filter(entity::castle_siege_guards::Column::X.eq(x))
            .filter(entity::castle_siege_guards::Column::Y.eq(y))
            .filter(entity::castle_siege_guards::Column::Z.eq(z))
            .filter(entity::castle_siege_guards::Column::IsHired.eq(1))
            .exec(db)
            .await,
    );
}

pub(super) async fn clear_hired_siege_guards(db: &DatabaseConnection, castle_id: i32) {
    warn_err(
        entity::castle_siege_guards::Entity::delete_many()
            .filter(entity::castle_siege_guards::Column::CastleId.eq(castle_id))
            .filter(entity::castle_siege_guards::Column::IsHired.eq(1))
            .exec(db)
            .await,
    );
}

pub(super) async fn update_castle_treasury(db: &DatabaseConnection, castle_id: i32, treasury: i64) {
    set_castle_col(
        db,
        castle_id,
        entity::castle::Column::Treasury,
        treasury.into(),
    )
    .await;
}

pub(super) async fn update_castle_siege_time(
    db: &DatabaseConnection,
    castle_id: i32,
    siege_date: i64,
    time_registration_over: bool,
    siege_time_registration_end: Option<i64>,
) {
    // `regTimeOver` is an enum('true','false') stored as text.
    let flag = if time_registration_over {
        "true"
    } else {
        "false"
    };
    let mut update = entity::castle::Entity::update_many()
        .col_expr(entity::castle::Column::SiegeDate, siege_date.into())
        .col_expr(entity::castle::Column::RegTimeOver, flag.into());
    if let Some(end) = siege_time_registration_end {
        update = update.col_expr(entity::castle::Column::RegTimeEnd, end.into());
    }
    warn_err(
        update
            .filter(entity::castle::Column::Id.eq(castle_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn save_siege_clan(
    db: &DatabaseConnection,
    castle_id: i32,
    clan_id: i32,
    kind: i32,
) {
    warn_err(
        entity::siege_clans::Entity::insert(entity::siege_clans::ActiveModel {
            clan_id: Set(clan_id),
            castle_id: Set(castle_id),
            r#type: Set(Some(kind)),
            castle_owner: Set(Some(0)),
        })
        .on_conflict(
            OnConflict::columns([
                entity::siege_clans::Column::ClanId,
                entity::siege_clans::Column::CastleId,
            ])
            .update_columns([
                entity::siege_clans::Column::Type,
                entity::siege_clans::Column::CastleOwner,
            ])
            .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn remove_siege_clan(db: &DatabaseConnection, castle_id: i32, clan_id: i32) {
    warn_err(
        entity::siege_clans::Entity::delete_many()
            .filter(entity::siege_clans::Column::CastleId.eq(castle_id))
            .filter(entity::siege_clans::Column::ClanId.eq(clan_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn save_clan_hall_bid(
    db: &DatabaseConnection,
    hall_id: i32,
    clan_id: i32,
    bid: i64,
    bid_time: i64,
) {
    warn_err(
        entity::clanhall_auctions_bidders::Entity::insert(
            entity::clanhall_auctions_bidders::ActiveModel {
                clan_hall_id: Set(hall_id),
                clan_id: Set(clan_id),
                bid: Set(bid),
                bid_time: Set(bid_time),
            },
        )
        .on_conflict(
            OnConflict::columns([
                entity::clanhall_auctions_bidders::Column::ClanHallId,
                entity::clanhall_auctions_bidders::Column::ClanId,
            ])
            .update_columns([
                entity::clanhall_auctions_bidders::Column::Bid,
                entity::clanhall_auctions_bidders::Column::BidTime,
            ])
            .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn remove_clan_hall_bid(db: &DatabaseConnection, hall_id: i32, clan_id: i32) {
    warn_err(
        entity::clanhall_auctions_bidders::Entity::delete_by_id((hall_id, clan_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn clear_clan_hall_bids(db: &DatabaseConnection, hall_id: i32) {
    warn_err(
        entity::clanhall_auctions_bidders::Entity::delete_many()
            .filter(entity::clanhall_auctions_bidders::Column::ClanHallId.eq(hall_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn save_clan_hall(
    db: &DatabaseConnection,
    id: i32,
    owner_id: i32,
    paid_until: i64,
) {
    warn_err(
        entity::clanhall::Entity::insert(entity::clanhall::ActiveModel {
            id: Set(id),
            owner_id: Set(owner_id),
            paid_until: Set(paid_until),
        })
        .on_conflict(
            OnConflict::column(entity::clanhall::Column::Id)
                .update_columns([
                    entity::clanhall::Column::OwnerId,
                    entity::clanhall::Column::PaidUntil,
                ])
                .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn save_residence_function(
    db: &DatabaseConnection,
    residence_id: i32,
    func_id: i32,
    level: i32,
    expiration: i64,
) {
    warn_err(
        entity::residence_functions::Entity::insert(entity::residence_functions::ActiveModel {
            id: Set(func_id),
            level: Set(level),
            expiration: Set(expiration),
            residence_id: Set(residence_id),
        })
        .on_conflict(
            OnConflict::columns([
                entity::residence_functions::Column::Id,
                entity::residence_functions::Column::Level,
                entity::residence_functions::Column::ResidenceId,
            ])
            .update_column(entity::residence_functions::Column::Expiration)
            .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn remove_residence_function(
    db: &DatabaseConnection,
    residence_id: i32,
    func_id: i32,
) {
    warn_err(
        entity::residence_functions::Entity::delete_many()
            .filter(entity::residence_functions::Column::ResidenceId.eq(residence_id))
            .filter(entity::residence_functions::Column::Id.eq(func_id))
            .exec(db)
            .await,
    );
}
