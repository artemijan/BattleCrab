//! Clan, alliance, pledge and crest writes.

use super::super::ItemRow;
use super::super::item_row_model;
use super::super::warn_err;
use super::{set_char_col, set_char_cols, set_clan_col, set_clan_cols};
use models::entity;
use models::sea_orm::ActiveValue::Set;
use models::sea_orm::Condition;
use models::sea_orm::DatabaseConnection;
use models::sea_orm::sea_query::OnConflict;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

pub(super) async fn insert_clan(
    db: &DatabaseConnection,
    clan_id: i32,
    name: String,
    leader_id: i32,
) {
    warn_err(
        entity::clan_data::Entity::insert(entity::clan_data::ActiveModel {
            clan_id: Set(clan_id),
            clan_name: Set(Some(name)),
            clan_level: Set(Some(0)),
            has_castle: Set(Some(0)),
            blood_alliance_count: Set(0),
            blood_oath_count: Set(0),
            ally_id: Set(Some(0)),
            ally_name: Set(None),
            leader_id: Set(Some(leader_id)),
            crest_id: Set(Some(0)),
            crest_large_id: Set(Some(0)),
            ally_crest_id: Set(Some(0)),
            new_leader_id: Set(0),
            ..Default::default()
        })
        .exec(db)
        .await,
    );
}

pub(super) async fn update_char_clan(
    db: &DatabaseConnection,
    char_id: i32,
    clan_id: i32,
    clan_privs: i32,
) {
    warn_err(
        entity::characters::Entity::update_many()
            .col_expr(entity::characters::Column::Clanid, clan_id.into())
            .col_expr(entity::characters::Column::ClanPrivs, clan_privs.into())
            .filter(entity::characters::Column::CharId.eq(char_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn save_clan_skill(
    db: &DatabaseConnection,
    clan_id: i32,
    skill_id: i32,
    skill_level: i32,
    skill_name: String,
) {
    warn_err(
        entity::clan_skills::Entity::insert(entity::clan_skills::ActiveModel {
            clan_id: Set(clan_id),
            skill_id: Set(skill_id),
            skill_level: Set(skill_level),
            skill_name: Set(Some(skill_name)),
            sub_pledge_id: Set(-2),
        })
        .on_conflict(
            OnConflict::columns([
                entity::clan_skills::Column::ClanId,
                entity::clan_skills::Column::SkillId,
                entity::clan_skills::Column::SubPledgeId,
            ])
            .update_columns([
                entity::clan_skills::Column::SkillLevel,
                entity::clan_skills::Column::SkillName,
            ])
            .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn delete_clan_skill(db: &DatabaseConnection, clan_id: i32, skill_id: i32) {
    warn_err(
        entity::clan_skills::Entity::delete_many()
            .filter(entity::clan_skills::Column::ClanId.eq(clan_id))
            .filter(entity::clan_skills::Column::SkillId.eq(skill_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn save_clan_notice(
    db: &DatabaseConnection,
    clan_id: i32,
    enabled: bool,
    notice: String,
) {
    warn_err(
        entity::clan_notices::Entity::insert(entity::clan_notices::ActiveModel {
            clan_id: Set(clan_id),
            enabled: Set(if enabled { "true" } else { "false" }.to_string()),
            notice: Set(notice),
        })
        .on_conflict(
            OnConflict::column(entity::clan_notices::Column::ClanId)
                .update_columns([
                    entity::clan_notices::Column::Enabled,
                    entity::clan_notices::Column::Notice,
                ])
                .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn update_clan_leader(db: &DatabaseConnection, clan_id: i32, leader_id: i32) {
    set_clan_col(
        db,
        clan_id,
        entity::clan_data::Column::LeaderId,
        leader_id.into(),
    )
    .await;
}

pub(super) async fn update_clan_castle(db: &DatabaseConnection, clan_id: i32, castle_id: i32) {
    set_clan_col(
        db,
        clan_id,
        entity::clan_data::Column::HasCastle,
        castle_id.into(),
    )
    .await;
}

pub(super) async fn update_clan_blood_alliance(db: &DatabaseConnection, clan_id: i32, count: i32) {
    set_clan_col(
        db,
        clan_id,
        entity::clan_data::Column::BloodAllianceCount,
        count.into(),
    )
    .await;
}

pub(super) async fn update_clan_level(db: &DatabaseConnection, clan_id: i32, level: i32) {
    set_clan_col(
        db,
        clan_id,
        entity::clan_data::Column::ClanLevel,
        level.into(),
    )
    .await;
}

pub(super) async fn update_clan_reputation(db: &DatabaseConnection, clan_id: i32, reputation: i32) {
    set_clan_col(
        db,
        clan_id,
        entity::clan_data::Column::ReputationScore,
        reputation.into(),
    )
    .await;
}

pub(super) async fn update_clan_penalties(
    db: &DatabaseConnection,
    clan_id: i32,
    char_penalty_expiry_time: i64,
    dissolving_expiry_time: i64,
) {
    set_clan_cols(
        db,
        clan_id,
        vec![
            (
                entity::clan_data::Column::CharPenaltyExpiryTime,
                char_penalty_expiry_time.into(),
            ),
            (
                entity::clan_data::Column::DissolvingExpiryTime,
                dissolving_expiry_time.into(),
            ),
        ],
    )
    .await;
}

pub(super) async fn remove_clan_member(
    db: &DatabaseConnection,
    char_id: i32,
    clan_join_expiry: i64,
    clan_create_expiry: i64,
) {
    set_char_cols(
        db,
        char_id,
        vec![
            (entity::characters::Column::Clanid, 0.into()),
            (entity::characters::Column::Title, "".into()),
            (entity::characters::Column::ClanPrivs, 0.into()),
            (
                entity::characters::Column::ClanJoinExpiryTime,
                clan_join_expiry.into(),
            ),
            (
                entity::characters::Column::ClanCreateExpiryTime,
                clan_create_expiry.into(),
            ),
        ],
    )
    .await;
}

pub(super) async fn save_clan_rank_privs(
    db: &DatabaseConnection,
    clan_id: i32,
    rank: i32,
    privs: i32,
) {
    warn_err(
        entity::clan_privs::Entity::insert(entity::clan_privs::ActiveModel {
            clan_id: Set(clan_id),
            rank: Set(rank),
            party: Set(0),
            privs: Set(privs),
        })
        .on_conflict(
            OnConflict::columns([
                entity::clan_privs::Column::ClanId,
                entity::clan_privs::Column::Rank,
                entity::clan_privs::Column::Party,
            ])
            .update_column(entity::clan_privs::Column::Privs)
            .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn update_char_power_grade(
    db: &DatabaseConnection,
    char_id: i32,
    power_grade: i32,
) {
    set_char_col(
        db,
        char_id,
        entity::characters::Column::PowerGrade,
        power_grade.into(),
    )
    .await;
}

pub(super) async fn update_clan_ally(
    db: &DatabaseConnection,
    clan_id: i32,
    ally_id: i32,
    ally_name: String,
    penalty_expiry: i64,
    penalty_type: i32,
) {
    set_clan_cols(
        db,
        clan_id,
        vec![
            (entity::clan_data::Column::AllyId, ally_id.into()),
            (entity::clan_data::Column::AllyName, ally_name.into()),
            (
                entity::clan_data::Column::AllyPenaltyExpiryTime,
                penalty_expiry.into(),
            ),
            (
                entity::clan_data::Column::AllyPenaltyType,
                penalty_type.into(),
            ),
        ],
    )
    .await;
}

pub(super) async fn insert_sub_pledge(
    db: &DatabaseConnection,
    clan_id: i32,
    pledge_type: i32,
    name: String,
    leader_id: i32,
) {
    warn_err(
        entity::clan_subpledges::Entity::insert(entity::clan_subpledges::ActiveModel {
            clan_id: Set(clan_id),
            sub_pledge_id: Set(pledge_type),
            name: Set(Some(name)),
            leader_id: Set(leader_id),
        })
        .exec(db)
        .await,
    );
}

pub(super) async fn update_sub_pledge(
    db: &DatabaseConnection,
    clan_id: i32,
    pledge_type: i32,
    name: String,
    leader_id: i32,
) {
    warn_err(
        entity::clan_subpledges::Entity::update_many()
            .col_expr(entity::clan_subpledges::Column::LeaderId, leader_id.into())
            .col_expr(entity::clan_subpledges::Column::Name, name.into())
            .filter(entity::clan_subpledges::Column::ClanId.eq(clan_id))
            .filter(entity::clan_subpledges::Column::SubPledgeId.eq(pledge_type))
            .exec(db)
            .await,
    );
}

pub(super) async fn update_char_academy_level(
    db: &DatabaseConnection,
    char_id: i32,
    lvl_joined_academy: i32,
) {
    set_char_col(
        db,
        char_id,
        entity::characters::Column::LvlJoinedAcademy,
        lvl_joined_academy.into(),
    )
    .await;
}

pub(super) async fn update_char_apprentice_sponsor(
    db: &DatabaseConnection,
    char_id: i32,
    apprentice: i32,
    sponsor: i32,
) {
    set_char_cols(
        db,
        char_id,
        vec![
            (entity::characters::Column::Apprentice, apprentice.into()),
            (entity::characters::Column::Sponsor, sponsor.into()),
        ],
    )
    .await;
}

pub(super) async fn update_char_pledge_type(
    db: &DatabaseConnection,
    char_id: i32,
    pledge_type: i32,
) {
    set_char_col(
        db,
        char_id,
        entity::characters::Column::Subpledge,
        pledge_type.into(),
    )
    .await;
}

pub(super) async fn insert_crest(db: &DatabaseConnection, id: i32, data: Vec<u8>, kind: i32) {
    warn_err(
        entity::crests::Entity::insert(entity::crests::ActiveModel {
            crest_id: Set(id),
            data: Set(data),
            r#type: Set(kind),
        })
        .exec(db)
        .await,
    );
}

pub(super) async fn delete_crest(db: &DatabaseConnection, id: i32) {
    warn_err(
        entity::crests::Entity::delete_many()
            .filter(entity::crests::Column::CrestId.eq(id))
            .exec(db)
            .await,
    );
}

pub(super) async fn update_clan_crest(db: &DatabaseConnection, clan_id: i32, crest_id: i32) {
    set_clan_col(
        db,
        clan_id,
        entity::clan_data::Column::CrestId,
        crest_id.into(),
    )
    .await;
}

pub(super) async fn update_clan_crest_large(
    db: &DatabaseConnection,
    clan_id: i32,
    crest_large_id: i32,
) {
    set_clan_col(
        db,
        clan_id,
        entity::clan_data::Column::CrestLargeId,
        crest_large_id.into(),
    )
    .await;
}

pub(super) async fn update_clan_ally_crest_self(
    db: &DatabaseConnection,
    clan_id: i32,
    ally_crest_id: i32,
) {
    set_clan_col(
        db,
        clan_id,
        entity::clan_data::Column::AllyCrestId,
        ally_crest_id.into(),
    )
    .await;
}

pub(super) async fn update_ally_crest_for_alliance(
    db: &DatabaseConnection,
    ally_id: i32,
    ally_crest_id: i32,
) {
    warn_err(
        entity::clan_data::Entity::update_many()
            .col_expr(entity::clan_data::Column::AllyCrestId, ally_crest_id.into())
            .filter(entity::clan_data::Column::AllyId.eq(ally_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn upsert_pledge_applicant(
    db: &DatabaseConnection,
    player_id: i32,
    clan_id: i32,
    karma: i32,
    message: String,
) {
    warn_err(
        entity::pledge_applicant::Entity::insert(entity::pledge_applicant::ActiveModel {
            char_id: Set(player_id),
            clan_id: Set(clan_id),
            karma: Set(karma),
            message: Set(message),
        })
        .on_conflict(
            OnConflict::columns([
                entity::pledge_applicant::Column::CharId,
                entity::pledge_applicant::Column::ClanId,
            ])
            .update_columns([
                entity::pledge_applicant::Column::Karma,
                entity::pledge_applicant::Column::Message,
            ])
            .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn delete_pledge_applicant(db: &DatabaseConnection, player_id: i32, clan_id: i32) {
    warn_err(
        entity::pledge_applicant::Entity::delete_by_id((player_id, clan_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn insert_pledge_waiting(db: &DatabaseConnection, player_id: i32, karma: i32) {
    warn_err(
        entity::pledge_waiting_list::Entity::insert(entity::pledge_waiting_list::ActiveModel {
            char_id: Set(player_id),
            karma: Set(karma),
        })
        .exec(db)
        .await,
    );
}

pub(super) async fn delete_pledge_waiting(db: &DatabaseConnection, player_id: i32) {
    warn_err(
        entity::pledge_waiting_list::Entity::delete_many()
            .filter(entity::pledge_waiting_list::Column::CharId.eq(player_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn insert_pledge_recruit(
    db: &DatabaseConnection,
    clan_id: i32,
    karma: i32,
    information: String,
    detailed_information: String,
    application_type: i32,
    recruit_type: i32,
) {
    warn_err(
        entity::pledge_recruit::Entity::insert(entity::pledge_recruit::ActiveModel {
            clan_id: Set(clan_id),
            karma: Set(karma),
            information: Set(information),
            detailed_information: Set(detailed_information),
            application_type: Set(application_type),
            recruit_type: Set(recruit_type),
        })
        .exec(db)
        .await,
    );
}

pub(super) async fn update_pledge_recruit(
    db: &DatabaseConnection,
    clan_id: i32,
    karma: i32,
    information: String,
    detailed_information: String,
    application_type: i32,
    recruit_type: i32,
) {
    warn_err(
        entity::pledge_recruit::Entity::update_many()
            .col_expr(entity::pledge_recruit::Column::Karma, karma.into())
            .col_expr(
                entity::pledge_recruit::Column::Information,
                information.into(),
            )
            .col_expr(
                entity::pledge_recruit::Column::DetailedInformation,
                detailed_information.into(),
            )
            .col_expr(
                entity::pledge_recruit::Column::ApplicationType,
                application_type.into(),
            )
            .col_expr(
                entity::pledge_recruit::Column::RecruitType,
                recruit_type.into(),
            )
            .filter(entity::pledge_recruit::Column::ClanId.eq(clan_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn delete_pledge_recruit(db: &DatabaseConnection, clan_id: i32) {
    warn_err(
        entity::pledge_recruit::Entity::delete_many()
            .filter(entity::pledge_recruit::Column::ClanId.eq(clan_id))
            .exec(db)
            .await,
    );
}

pub(super) async fn save_clan_war(
    db: &DatabaseConnection,
    attacker: i32,
    attacked: i32,
    attacker_kills: i32,
    attacked_kills: i32,
    winner: i32,
    start_time: i64,
    end_time: i64,
    state: i32,
) {
    // The clan-id columns are `varchar(35)`; SQLite stored the bound
    // integers as text anyway, and `load_clan_wars` parses them back.
    warn_err(
        entity::clan_wars::Entity::insert(entity::clan_wars::ActiveModel {
            clan1: Set(attacker.to_string()),
            clan2: Set(attacked.to_string()),
            clan1_kill: Set(attacker_kills),
            clan2_kill: Set(attacked_kills),
            winner_clan: Set(winner.to_string()),
            start_time: Set(start_time),
            end_time: Set(end_time),
            state: Set(state),
        })
        .on_conflict(
            OnConflict::columns([
                entity::clan_wars::Column::Clan1,
                entity::clan_wars::Column::Clan2,
            ])
            .update_columns([
                entity::clan_wars::Column::Clan1Kill,
                entity::clan_wars::Column::Clan2Kill,
                entity::clan_wars::Column::WinnerClan,
                entity::clan_wars::Column::StartTime,
                entity::clan_wars::Column::EndTime,
                entity::clan_wars::Column::State,
            ])
            .to_owned(),
        )
        .exec(db)
        .await,
    );
}

pub(super) async fn delete_clan_war(db: &DatabaseConnection, clan1: i32, clan2: i32) {
    let (a, b) = (clan1.to_string(), clan2.to_string());
    warn_err(
        entity::clan_wars::Entity::delete_many()
            .filter(
                Condition::any()
                    .add(
                        Condition::all()
                            .add(entity::clan_wars::Column::Clan1.eq(a.clone()))
                            .add(entity::clan_wars::Column::Clan2.eq(b.clone())),
                    )
                    .add(
                        Condition::all()
                            .add(entity::clan_wars::Column::Clan1.eq(b))
                            .add(entity::clan_wars::Column::Clan2.eq(a)),
                    ),
            )
            .exec(db)
            .await,
    );
}

pub(super) async fn update_clan_new_leader(
    db: &DatabaseConnection,
    clan_id: i32,
    new_leader_id: i32,
) {
    set_clan_col(
        db,
        clan_id,
        entity::clan_data::Column::NewLeaderId,
        new_leader_id.into(),
    )
    .await;
}

pub(super) async fn update_char_clan_join_expiry(
    db: &DatabaseConnection,
    char_id: i32,
    expiry: i64,
) {
    set_char_col(
        db,
        char_id,
        entity::characters::Column::ClanJoinExpiryTime,
        expiry.into(),
    )
    .await;
}

pub(super) async fn destroy_clan(
    db: &DatabaseConnection,
    clan_id: i32,
    leader_id: i32,
    leader_expiry: i64,
) {
    warn_err(
        entity::clan_data::Entity::delete_many()
            .filter(entity::clan_data::Column::ClanId.eq(clan_id))
            .exec(db)
            .await,
    );
    warn_err(
        entity::clan_skills::Entity::delete_many()
            .filter(entity::clan_skills::Column::ClanId.eq(clan_id))
            .exec(db)
            .await,
    );
    warn_err(
        entity::characters::Entity::update_many()
            .col_expr(entity::characters::Column::Clanid, 0.into())
            .col_expr(entity::characters::Column::ClanPrivs, 0.into())
            .filter(entity::characters::Column::Clanid.eq(clan_id))
            .exec(db)
            .await,
    );
    set_char_col(
        db,
        leader_id,
        entity::characters::Column::ClanCreateExpiryTime,
        leader_expiry.into(),
    )
    .await;
}

pub(super) async fn store_clan_warehouse(
    db: &DatabaseConnection,
    clan_id: i32,
    items: Vec<ItemRow>,
) {
    warn_err(
        entity::items::Entity::delete_many()
            .filter(entity::items::Column::OwnerId.eq(clan_id))
            .exec(db)
            .await,
    );
    for it in &items {
        warn_err(
            entity::items::Entity::insert(item_row_model(clan_id, it, None))
                .exec(db)
                .await,
        );
    }
}
