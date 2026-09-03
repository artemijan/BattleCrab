//! Castle, siege, clan-hall, manor and cursed-weapon reads.

use super::super::ClanHallBidRow;
use super::super::ClanHallRow;
use super::super::CursedWeaponRow;
use super::super::ManorProcureRow;
use super::super::ManorProductionRow;
use super::super::ResidenceFunctionRow;
use super::super::SiegeClanRow;
use models::entity;

use models::sea_orm::DatabaseConnection;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `CursedWeaponsManager.restore`: every `cursed_weapons` state row.
pub(crate) async fn load_cursed_weapons(db: &DatabaseConnection) -> Vec<CursedWeaponRow> {
    entity::cursed_weapons::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| CursedWeaponRow {
            item_id: r.item_id,
            char_id: r.char_id,
            player_reputation: r.player_reputation.unwrap_or(0),
            player_pk_kills: r.player_pk_kills.unwrap_or(0),
            nb_kills: r.nb_kills.unwrap_or(0),
            end_time: r.end_time,
        })
        .collect()
}
/// The **hired** mercenaries (`castle_siege_guards WHERE isHired=1`) — the
/// postings the owning clan paid for between sieges.
pub(crate) async fn load_hired_siege_guards(
    db: &DatabaseConnection,
) -> Vec<(i32, crate::model::siege::SiegeSpawn)> {
    load_guards_where(db, 1).await
}
/// The stationed siege guards (`castle_siege_guards WHERE isHired=0`) — the
/// non-mercenary garrison spawned at siege start.
pub(crate) async fn load_siege_guards(
    db: &DatabaseConnection,
) -> Vec<(i32, crate::model::siege::SiegeSpawn)> {
    load_guards_where(db, 0).await
}
async fn load_guards_where(
    db: &DatabaseConnection,
    is_hired: i32,
) -> Vec<(i32, crate::model::siege::SiegeSpawn)> {
    entity::castle_siege_guards::Entity::find()
        .filter(entity::castle_siege_guards::Column::IsHired.eq(is_hired))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            (
                r.castle_id,
                crate::model::siege::SiegeSpawn {
                    npc_id: r.npc_id,
                    x: r.x,
                    y: r.y,
                    z: r.z,
                    heading: r.heading,
                },
            )
        })
        .collect()
}
/// `Siege.loadSiegeClan`: every `siege_clans` row.
pub(crate) async fn load_siege_clans(db: &DatabaseConnection) -> Vec<SiegeClanRow> {
    entity::siege_clans::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| SiegeClanRow {
            castle_id: r.castle_id,
            clan_id: r.clan_id,
            kind: r.r#type.unwrap_or(0),
        })
        .collect()
}
/// `CastleManorManager.loadDb`: the `castle_manor_production` rows (seeds the
/// manor sells). Missing table → empty (the manor is simply unset).
pub(crate) async fn load_manor_production(db: &DatabaseConnection) -> Vec<ManorProductionRow> {
    entity::castle_manor_production::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| ManorProductionRow {
            castle_id: r.castle_id,
            seed_id: r.seed_id,
            amount: r.amount.into(),
            start_amount: r.start_amount.into(),
            price: r.price.into(),
            next_period: r.next_period != 0,
        })
        .collect()
}
/// `CastleManorManager.loadDb`: the `castle_manor_procure` rows (crops the manor
/// buys). Missing table → empty.
pub(crate) async fn load_manor_procure(db: &DatabaseConnection) -> Vec<ManorProcureRow> {
    entity::castle_manor_procure::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| ManorProcureRow {
            castle_id: r.castle_id,
            crop_id: r.crop_id,
            amount: r.amount.into(),
            start_amount: r.start_amount.into(),
            price: r.price.into(),
            reward_type: r.reward_type,
            next_period: r.next_period != 0,
        })
        .collect()
}
/// The `clanhall` table — persisted hall ownership (id → owner/paidUntil).
pub(crate) async fn load_clan_hall_owners(db: &DatabaseConnection) -> Vec<ClanHallRow> {
    entity::clanhall::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| ClanHallRow {
            id: r.id,
            owner_id: r.owner_id,
            paid_until: r.paid_until,
        })
        .collect()
}
/// The `clanhall_auctions_bidders` table — the live auction bids.
pub(crate) async fn load_clan_hall_bidders(db: &DatabaseConnection) -> Vec<ClanHallBidRow> {
    entity::clanhall_auctions_bidders::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| ClanHallBidRow {
            hall_id: r.clan_hall_id,
            clan_id: r.clan_id,
            bid: r.bid,
            bid_time: r.bid_time,
        })
        .collect()
}
/// The `residence_functions` table — active hall function upgrades.
pub(crate) async fn load_residence_functions(db: &DatabaseConnection) -> Vec<ResidenceFunctionRow> {
    entity::residence_functions::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| ResidenceFunctionRow {
            residence_id: r.residence_id,
            func_id: r.id,
            level: r.level,
            expiration: r.expiration,
        })
        .collect()
}
pub(crate) async fn load_castles(db: &DatabaseConnection) -> Vec<crate::model::castle::Castle> {
    entity::castle::Entity::find()
        .order_by_asc(entity::castle::Column::Id)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| crate::model::castle::Castle {
            id: r.id,
            name: r.name,
            side: crate::model::castle::CastleSide::from_string(&r.side).unwrap_or_default(),
            ticket_buy_count: r.ticket_buy_count,
            show_npc_crest: r.show_npc_crest == "true",
            // Runtime-only in Java too — a restart clears it.
            first_mid_victory: false,
            // `regTimeOver` is an enum('true','false'); default (missing) is true.
            time_registration_over: r.reg_time_over != "false",
            siege_time_registration_end: r.reg_time_end,
            siege_date: r.siege_date,
            treasury: r.treasury,
        })
        .collect()
}
