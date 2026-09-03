//! Clan, alliance, crest and recruitment reads.

use super::character_load::{characters_by_id, load_items};
use models::entity;

use models::sea_orm::DatabaseConnection;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

/// The whole `clan_notices` table (Java `Clan.restoreNotice`).
pub(crate) async fn load_clan_notices(db: &DatabaseConnection) -> Vec<(i32, bool, String)> {
    entity::clan_notices::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|n| (n.clan_id, n.enabled.eq_ignore_ascii_case("true"), n.notice))
        .collect()
}
pub(crate) async fn load_clans(db: &DatabaseConnection) -> Vec<crate::model::clan::Clan> {
    let clan_rows = entity::clan_data::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    let mut out = Vec::with_capacity(clan_rows.len());
    for row in &clan_rows {
        let clan_id = row.clan_id;

        let member_rows = entity::characters::Entity::find()
            .filter(entity::characters::Column::Clanid.eq(clan_id))
            .all(db)
            .await
            .unwrap_or_default();
        // Clan warehouse contents (`owner_id = clan_id`, `loc = "CLANWH"`).
        let wh_rows = load_items(db, clan_id).await;
        // Clan skills (Java `Clan.restoreSkills`) — the main-pledge set
        // (`sub_pledge_id = -2`); sub-unit skills aren't modelled, so other
        // sub_pledge ids are ignored.
        let skills = entity::clan_skills::Entity::find()
            .filter(entity::clan_skills::Column::ClanId.eq(clan_id))
            .filter(entity::clan_skills::Column::SubPledgeId.is_in([-2, 0]))
            .all(db)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|s| (s.skill_id, s.skill_level))
            .collect();
        // Rank → privilege-mask rows (Java `restoreRankPrivs`; rank -1 skipped).
        let rank_privs = entity::clan_privs::Entity::find()
            .filter(entity::clan_privs::Column::ClanId.eq(clan_id))
            .all(db)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.rank, r.privs))
            .filter(|&(rank, _)| rank != -1)
            .collect();
        // Sub-pledges (Java `Clan.restoreSubPledges`).
        let sub_pledges: std::collections::HashMap<i32, crate::model::clan::SubPledge> =
            entity::clan_subpledges::Entity::find()
                .filter(entity::clan_subpledges::Column::ClanId.eq(clan_id))
                .all(db)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|r| {
                    (
                        r.sub_pledge_id,
                        crate::model::clan::SubPledge {
                            id: r.sub_pledge_id,
                            name: r.name.unwrap_or_default(),
                            leader_id: r.leader_id,
                        },
                    )
                })
                .collect();
        out.push(crate::model::clan::Clan {
            id: clan_id,
            name: row.clan_name.clone().unwrap_or_default(),
            leader_id: row.leader_id.unwrap_or(0),
            level: row.clan_level.unwrap_or(0),
            reputation_score: row.reputation_score,
            castle_id: row.has_castle.unwrap_or(0),
            blood_alliance_count: row.blood_alliance_count,
            char_penalty_expiry_time: row.char_penalty_expiry_time,
            dissolving_expiry_time: row.dissolving_expiry_time,
            rank_privs,
            new_leader_id: row.new_leader_id,
            sub_pledges,
            ally_id: row.ally_id.unwrap_or(0),
            ally_name: row.ally_name.clone().unwrap_or_default(),
            ally_penalty_expiry_time: row.ally_penalty_expiry_time,
            ally_penalty_type: row.ally_penalty_type,
            crest_id: row.crest_id.unwrap_or(0),
            crest_large_id: row.crest_large_id.unwrap_or(0),
            ally_crest_id: row.ally_crest_id.unwrap_or(0),
            skills,
            warehouse: crate::model::inventory::Warehouse::from_rows(&wh_rows),
            members: member_rows
                .into_iter()
                .map(|m| crate::model::clan::ClanMember {
                    char_id: m.char_id,
                    name: m.char_name,
                    level: m.level.unwrap_or(0),
                    class_id: m.classid.unwrap_or(0),
                    sex: m.sex.unwrap_or(0),
                    race: m.race.unwrap_or(0),
                    power_grade: m.power_grade.unwrap_or(0),
                    title: m.title.unwrap_or_default(),
                    pledge_type: m.subpledge,
                    apprentice: m.apprentice,
                    sponsor: m.sponsor,
                })
                .collect(),
        });
    }
    out
}
/// `ClanTable.restoreClanWars` — the `clan_wars` table (ids in the varchar
/// columns, as Java writes them).
pub(crate) async fn load_clan_wars(db: &DatabaseConnection) -> Vec<crate::model::clan::ClanWar> {
    // `clan1`/`clan2`/`winnerClan` are `varchar(35)` holding clan ids, so the
    // stored values are text. Java reads them with `rset.getInt`, which coerces;
    // the parse here is that coercion. (The pre-ORM code asked sqlx for an i64
    // and silently got 0 for every row, which made every restored war a war
    // between clan 0 and clan 0.)
    fn id(raw: &str) -> i32 {
        raw.trim().parse().unwrap_or(0)
    }
    entity::clan_wars::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| crate::model::clan::ClanWar {
            attacker_id: id(&r.clan1),
            attacked_id: id(&r.clan2),
            attacker_kills: r.clan1_kill,
            attacked_kills: r.clan2_kill,
            winner_id: id(&r.winner_clan),
            start_time: r.start_time,
            end_time: r.end_time,
            state: crate::model::clan::ClanWarState::from_i32(r.state),
        })
        .collect()
}
/// `CrestTable.load` — every stored crest bitmap (`crests` table).
pub(crate) async fn load_crests(db: &DatabaseConnection) -> Vec<crate::model::clan::Crest> {
    entity::crests::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| crate::model::clan::Crest {
            id: r.crest_id,
            data: r.data,
            kind: r.r#type,
        })
        .collect()
}
/// `ClanEntryManager.load`'s `pledge_recruit` half (the boot-time removal of
/// entries for clans that no longer exist is done by the caller, which
/// already has the loaded clan set).
pub(crate) async fn load_recruit_clans(
    db: &DatabaseConnection,
) -> Vec<crate::model::clan_entry::PledgeRecruitInfo> {
    entity::pledge_recruit::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| crate::model::clan_entry::PledgeRecruitInfo {
            clan_id: r.clan_id,
            karma: r.karma,
            information: r.information,
            detailed_information: r.detailed_information,
            application_type: r.application_type,
            recruit_type: r.recruit_type,
        })
        .collect()
}
/// `ClanEntryManager.load`'s `pledge_waiting_list` half (joined with
/// `characters` for the display fields, as Java's own query does).
pub(crate) async fn load_recruit_waiting(
    db: &DatabaseConnection,
) -> Vec<crate::model::clan_entry::PledgeWaitingInfo> {
    let waiting = entity::pledge_waiting_list::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    if waiting.is_empty() {
        return Vec::new();
    }
    // Java's query LEFT JOINs `characters` for the display fields; an applicant
    // whose character is gone keeps the row with empty values.
    let chars = characters_by_id(db, waiting.iter().map(|w| w.char_id)).await;
    waiting
        .into_iter()
        .map(|w| {
            let c = chars.iter().find(|c| c.char_id == w.char_id);
            crate::model::clan_entry::PledgeWaitingInfo {
                player_id: w.char_id,
                level: c.and_then(|c| c.level).unwrap_or(0),
                karma: w.karma,
                class_id: c.map(|c| c.base_class).unwrap_or(0),
                name: c.map(|c| c.char_name.clone()).unwrap_or_default(),
            }
        })
        .collect()
}
/// `ClanEntryManager.load`'s `pledge_applicant` half.
pub(crate) async fn load_recruit_applicants(
    db: &DatabaseConnection,
) -> Vec<crate::model::clan_entry::PledgeApplicantInfo> {
    let applicants = entity::pledge_applicant::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    if applicants.is_empty() {
        return Vec::new();
    }
    let chars = characters_by_id(db, applicants.iter().map(|a| a.char_id)).await;
    applicants
        .into_iter()
        .map(|a| {
            let c = chars.iter().find(|c| c.char_id == a.char_id);
            crate::model::clan_entry::PledgeApplicantInfo {
                player_id: a.char_id,
                name: c.map(|c| c.char_name.clone()).unwrap_or_default(),
                level: c.and_then(|c| c.level).unwrap_or(0),
                karma: a.karma,
                clan_id: a.clan_id,
                message: a.message,
            }
        })
        .collect()
}
