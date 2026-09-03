use super::CmdRx;
use super::CreateResult;
use super::DbCommand;
use super::DbEvent;
use super::EventTx;
use super::ID_BLOCK_SIZE;
use super::boot;
use super::clean_up_database;
use super::clear_ground_items;
use super::create_character;
use super::delete_char;
use super::load_next_id;
use super::reload;
use super::send_boot_events;
use super::store_ground_items;
use super::store_player;
use super::verify_schema;
use super::warn_err;
use boot::GroundItemBootConfig;
use models::entity;

use models::sea_orm::DatabaseConnection;
use models::sea_orm::sea_query::SimpleExpr;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use tracing::error;
use tracing::info;

mod characters;
mod clans;
mod commerce;
mod minigames;
mod olympiad;
mod residences;
mod social;
mod world;

pub(crate) async fn run(
    url: String,
    max_connections: u32,
    max_characters: i32,
    clean_up: bool,
    ground_items: GroundItemBootConfig,
    mut cmd_rx: CmdRx,
    event_tx: EventTx,
) {
    let db = match commons::db::connect(&url, max_connections).await {
        Ok(db) => db,
        Err(e) => {
            error!("DB thread: failed to open database: {e}");
            return;
        }
    };

    // `create_if_missing(true)` means a wrong path does not fail — SQLite
    // happily makes an empty database, and the server then runs against it,
    // losing every character and account silently. Catch that here rather than
    // letting it surface hours later as "no such table".
    //
    // The usual cause is a relative `URL` resolving somewhere unexpected. It is
    // resolved against the executable's directory, so the database belongs
    // beside the binary — not in the datapack, and not in whatever directory
    // the unit file happened to start the process in.
    if let Err(e) = verify_schema(&db).await {
        error!(
            "DB thread: {e}\n  URL = {url}\n  relative paths resolve next to the executable, in {}\n\
             This is not the game database. Put it beside the binary (the same file the login \
             server opens), or make the URL absolute.",
            commons::db::executable_dir().display(),
        );
        return;
    }

    // Java `IdManager`'s constructor order: clean the orphans first, *then*
    // walk the tables for used object ids. Doing it the other way round would
    // reserve ids belonging to rows this is about to delete.
    if clean_up {
        clean_up_database(&db).await;
    }

    let mut next_id = load_next_id(&db).await;

    // Hand the game thread its initial runtime-id block unprompted (it can't
    // ask before it knows the DB thread is up; see `DbCommand::ReserveIds`).
    let _ = event_tx.send(DbEvent::IdBlock {
        start: next_id,
        end: next_id + ID_BLOCK_SIZE,
    });
    next_id += ID_BLOCK_SIZE;

    send_boot_events(&db, &ground_items, &event_tx).await;

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            DbCommand::LoadCharacters { client_id, account } => {
                reload(&db, &event_tx, client_id, account, true).await
            }
            DbCommand::CreateCharacter { client_id, data } => {
                let result = create_character(&db, &mut next_id, max_characters, &data).await;
                let _ = event_tx.send(DbEvent::CharacterCreated { client_id, result });
                if result == CreateResult::Ok {
                    // Java caches the list after creation but does not re-send it.
                    reload(&db, &event_tx, client_id, data.account, false).await;
                }
            }
            DbCommand::MarkDelete {
                client_id,
                account,
                char_id,
                delete_time,
            } => {
                characters::mark_delete(&db, &event_tx, client_id, account, char_id, delete_time)
                    .await
            }
            DbCommand::RestoreCharacter {
                client_id,
                account,
                char_id,
            } => characters::restore_character(&db, &event_tx, client_id, account, char_id).await,
            DbCommand::DeleteCharacter { char_id } => delete_char(&db, char_id).await,
            DbCommand::StoreGrandBoss { boss } => world::store_grand_boss(&db, boss).await,
            DbCommand::DeletePetRow { collar_object_id } => {
                world::delete_pet_row(&db, collar_object_id).await
            }
            DbCommand::CountCharacters { account } => {
                characters::send_char_count(&db, &event_tx, account).await
            }
            DbCommand::CheckNameCreatable { client_id, name } => {
                characters::check_name_creatable(&db, &event_tx, client_id, name).await
            }
            DbCommand::StoreGroundItems { items } => store_ground_items(&db, &items).await,
            DbCommand::DeleteQuestRows {
                char_id,
                quest_names,
            } => characters::delete_quest_rows(&db, char_id, quest_names).await,
            DbCommand::ClearGroundItems => clear_ground_items(&db).await,
            DbCommand::StorePlayer { save } => store_player(&db, &save).await,
            DbCommand::ReserveIds { count } => {
                let _ = event_tx.send(DbEvent::IdBlock {
                    start: next_id,
                    end: next_id + count,
                });
                next_id += count;
            }
            DbCommand::InsertFriendPair { a, b } => social::insert_friend_pair(&db, a, b).await,
            DbCommand::InsertBlock { owner, target } => {
                social::insert_block(&db, owner, target).await
            }
            DbCommand::DeleteBlock { owner, target } => {
                social::delete_block(&db, owner, target).await
            }
            DbCommand::DeleteFriendPair { a, b } => social::delete_friend_pair(&db, a, b).await,
            DbCommand::InsertClan {
                clan_id,
                name,
                leader_id,
            } => clans::insert_clan(&db, clan_id, name, leader_id).await,
            DbCommand::UpdateCharClan {
                char_id,
                clan_id,
                clan_privs,
            } => clans::update_char_clan(&db, char_id, clan_id, clan_privs).await,
            DbCommand::SaveClanSkill {
                clan_id,
                skill_id,
                skill_level,
                skill_name,
            } => clans::save_clan_skill(&db, clan_id, skill_id, skill_level, skill_name).await,
            DbCommand::DeleteClanSkill { clan_id, skill_id } => {
                clans::delete_clan_skill(&db, clan_id, skill_id).await
            }
            DbCommand::StoreCursedWeapon {
                item_id,
                char_id,
                reputation,
                pk_kills,
                nb_kills,
                end_time,
            } => {
                residences::store_cursed_weapon(
                    &db, item_id, char_id, reputation, pk_kills, nb_kills, end_time,
                )
                .await
            }
            DbCommand::StoreNpcRespawn {
                npc_id,
                x,
                y,
                z,
                heading,
                respawn_time,
                cur_hp,
                cur_mp,
            } => {
                world::store_npc_respawn(
                    &db,
                    npc_id,
                    x,
                    y,
                    z,
                    heading,
                    respawn_time,
                    cur_hp,
                    cur_mp,
                )
                .await
            }
            DbCommand::SaveClanNotice {
                clan_id,
                enabled,
                notice,
            } => clans::save_clan_notice(&db, clan_id, enabled, notice).await,
            DbCommand::WipeSubclassSlot {
                char_id,
                class_index,
                old_class_id,
            } => characters::wipe_subclass_slot(&db, char_id, class_index, old_class_id).await,
            DbCommand::StoreSubClass {
                char_id,
                class_id,
                class_index,
                level,
                exp,
                sp,
            } => {
                characters::store_sub_class(&db, char_id, class_id, class_index, level, exp, sp)
                    .await
            }
            DbCommand::DeleteNpcRespawn { npc_id } => world::delete_npc_respawn(&db, npc_id).await,
            DbCommand::RemoveCursedWeapon { item_id } => {
                residences::remove_cursed_weapon(&db, item_id).await
            }
            DbCommand::RestoreOfflineCursedOwner {
                char_id,
                item_id,
                reputation,
                pk_kills,
                skill_ids,
            } => {
                residences::restore_offline_cursed_owner(
                    &db, char_id, item_id, reputation, pk_kills, skill_ids,
                )
                .await
            }
            DbCommand::UpdateCastleSide { castle_id, side } => {
                residences::update_castle_side(&db, castle_id, side).await
            }
            DbCommand::UpdateCastleShowNpcCrest { castle_id, show } => {
                residences::update_castle_show_npc_crest(&db, castle_id, show).await
            }
            DbCommand::UpdateClanLeader { clan_id, leader_id } => {
                clans::update_clan_leader(&db, clan_id, leader_id).await
            }
            DbCommand::UpdateClanCastle { clan_id, castle_id } => {
                clans::update_clan_castle(&db, clan_id, castle_id).await
            }
            DbCommand::UpdateClanBloodAlliance { clan_id, count } => {
                clans::update_clan_blood_alliance(&db, clan_id, count).await
            }
            DbCommand::UpdateCastleTicketCount { castle_id, count } => {
                residences::update_castle_ticket_count(&db, castle_id, count).await
            }
            DbCommand::SaveBuyListStock {
                list_id,
                item_id,
                count,
                next_restock_time,
            } => {
                commerce::save_buy_list_stock(&db, list_id, item_id, count, next_restock_time).await
            }
            DbCommand::AddHiredSiegeGuard {
                castle_id,
                npc_id,
                x,
                y,
                z,
                heading,
            } => residences::add_hired_siege_guard(&db, castle_id, npc_id, x, y, z, heading).await,
            DbCommand::RemoveHiredSiegeGuard { npc_id, x, y, z } => {
                residences::remove_hired_siege_guard(&db, npc_id, x, y, z).await
            }
            DbCommand::ClearHiredSiegeGuards { castle_id } => {
                residences::clear_hired_siege_guards(&db, castle_id).await
            }
            DbCommand::AddFreightItems { owner_id, items } => {
                commerce::add_freight_items(&db, owner_id, items).await
            }
            DbCommand::StoreOfflineTrader {
                char_id,
                time,
                store_type,
                title,
                items,
            } => commerce::store_offline_trader(&db, char_id, time, store_type, title, items).await,
            DbCommand::ClearOfflineTrader { char_id } => {
                commerce::clear_offline_trader(&db, char_id).await
            }
            DbCommand::StoreManor {
                castle_id,
                production,
                procure,
            } => commerce::store_manor(&db, castle_id, production, procure).await,
            DbCommand::UpdateCastleTreasury {
                castle_id,
                treasury,
            } => residences::update_castle_treasury(&db, castle_id, treasury).await,
            DbCommand::UpdateCastleSiegeTime {
                castle_id,
                siege_date,
                time_registration_over,
                siege_time_registration_end,
            } => {
                residences::update_castle_siege_time(
                    &db,
                    castle_id,
                    siege_date,
                    time_registration_over,
                    siege_time_registration_end,
                )
                .await
            }
            DbCommand::SaveGlobalVariable { var, value } => {
                world::save_global_variable(&db, var, value).await
            }
            DbCommand::SaveSiegeClan {
                castle_id,
                clan_id,
                kind,
            } => residences::save_siege_clan(&db, castle_id, clan_id, kind).await,
            DbCommand::RemoveSiegeClan { castle_id, clan_id } => {
                residences::remove_siege_clan(&db, castle_id, clan_id).await
            }
            DbCommand::SaveClanHallBid {
                hall_id,
                clan_id,
                bid,
                bid_time,
            } => residences::save_clan_hall_bid(&db, hall_id, clan_id, bid, bid_time).await,
            DbCommand::RemoveClanHallBid { hall_id, clan_id } => {
                residences::remove_clan_hall_bid(&db, hall_id, clan_id).await
            }
            DbCommand::ClearClanHallBids { hall_id } => {
                residences::clear_clan_hall_bids(&db, hall_id).await
            }
            DbCommand::SaveClanHall {
                id,
                owner_id,
                paid_until,
            } => residences::save_clan_hall(&db, id, owner_id, paid_until).await,
            DbCommand::SaveResidenceFunction {
                residence_id,
                func_id,
                level,
                expiration,
            } => {
                residences::save_residence_function(&db, residence_id, func_id, level, expiration)
                    .await
            }
            DbCommand::RemoveResidenceFunction {
                residence_id,
                func_id,
            } => residences::remove_residence_function(&db, residence_id, func_id).await,
            DbCommand::SaveOlympiad {
                current_cycle,
                period,
                olympiad_end,
                validation_end,
                next_weekly_change,
                nobles,
            } => {
                olympiad::save_olympiad(
                    &db,
                    current_cycle,
                    period,
                    olympiad_end,
                    validation_end,
                    next_weekly_change,
                    nobles,
                )
                .await
            }
            DbCommand::SaveHeroes { heroes } => olympiad::save_heroes(&db, heroes).await,
            DbCommand::SnapshotOlympiadEom => olympiad::snapshot_olympiad_eom(&db).await,
            DbCommand::ClaimHero { char_id } => olympiad::claim_hero(&db, char_id).await,
            DbCommand::SaveHeroDiary {
                char_id,
                time,
                action,
                param,
            } => olympiad::save_hero_diary(&db, char_id, time, action, param).await,
            DbCommand::UpdateClanLevel { clan_id, level } => {
                clans::update_clan_level(&db, clan_id, level).await
            }
            DbCommand::UpdateClanReputation {
                clan_id,
                reputation,
            } => clans::update_clan_reputation(&db, clan_id, reputation).await,
            DbCommand::UpdateClanPenalties {
                clan_id,
                char_penalty_expiry_time,
                dissolving_expiry_time,
            } => {
                clans::update_clan_penalties(
                    &db,
                    clan_id,
                    char_penalty_expiry_time,
                    dissolving_expiry_time,
                )
                .await
            }
            DbCommand::RemoveClanMember {
                char_id,
                clan_join_expiry,
                clan_create_expiry,
            } => {
                clans::remove_clan_member(&db, char_id, clan_join_expiry, clan_create_expiry).await
            }
            DbCommand::SaveClanRankPrivs {
                clan_id,
                rank,
                privs,
            } => clans::save_clan_rank_privs(&db, clan_id, rank, privs).await,
            DbCommand::UpdateCharPowerGrade {
                char_id,
                power_grade,
            } => clans::update_char_power_grade(&db, char_id, power_grade).await,
            DbCommand::UpdateClanAlly {
                clan_id,
                ally_id,
                ally_name,
                penalty_expiry,
                penalty_type,
            } => {
                clans::update_clan_ally(
                    &db,
                    clan_id,
                    ally_id,
                    ally_name,
                    penalty_expiry,
                    penalty_type,
                )
                .await
            }
            DbCommand::InsertSubPledge {
                clan_id,
                pledge_type,
                name,
                leader_id,
            } => clans::insert_sub_pledge(&db, clan_id, pledge_type, name, leader_id).await,
            DbCommand::UpdateSubPledge {
                clan_id,
                pledge_type,
                name,
                leader_id,
            } => clans::update_sub_pledge(&db, clan_id, pledge_type, name, leader_id).await,
            DbCommand::UpdateCharAcademyLevel {
                char_id,
                lvl_joined_academy,
            } => clans::update_char_academy_level(&db, char_id, lvl_joined_academy).await,
            DbCommand::UpdateCharApprenticeSponsor {
                char_id,
                apprentice,
                sponsor,
            } => clans::update_char_apprentice_sponsor(&db, char_id, apprentice, sponsor).await,
            DbCommand::UpdateCharPledgeType {
                char_id,
                pledge_type,
            } => clans::update_char_pledge_type(&db, char_id, pledge_type).await,
            DbCommand::InsertCrest { id, data, kind } => {
                clans::insert_crest(&db, id, data, kind).await
            }
            DbCommand::DeleteCrest { id } => clans::delete_crest(&db, id).await,
            DbCommand::UpdateClanCrest { clan_id, crest_id } => {
                clans::update_clan_crest(&db, clan_id, crest_id).await
            }
            DbCommand::UpdateClanCrestLarge {
                clan_id,
                crest_large_id,
            } => clans::update_clan_crest_large(&db, clan_id, crest_large_id).await,
            DbCommand::UpdateClanAllyCrestSelf {
                clan_id,
                ally_crest_id,
            } => clans::update_clan_ally_crest_self(&db, clan_id, ally_crest_id).await,
            DbCommand::UpdateAllyCrestForAlliance {
                ally_id,
                ally_crest_id,
            } => clans::update_ally_crest_for_alliance(&db, ally_id, ally_crest_id).await,
            DbCommand::UpsertPledgeApplicant {
                player_id,
                clan_id,
                karma,
                message,
            } => clans::upsert_pledge_applicant(&db, player_id, clan_id, karma, message).await,
            DbCommand::DeletePledgeApplicant { player_id, clan_id } => {
                clans::delete_pledge_applicant(&db, player_id, clan_id).await
            }
            DbCommand::InsertPledgeWaiting { player_id, karma } => {
                clans::insert_pledge_waiting(&db, player_id, karma).await
            }
            DbCommand::DeletePledgeWaiting { player_id } => {
                clans::delete_pledge_waiting(&db, player_id).await
            }
            DbCommand::InsertPledgeRecruit {
                clan_id,
                karma,
                information,
                detailed_information,
                application_type,
                recruit_type,
            } => {
                clans::insert_pledge_recruit(
                    &db,
                    clan_id,
                    karma,
                    information,
                    detailed_information,
                    application_type,
                    recruit_type,
                )
                .await
            }
            DbCommand::UpdatePledgeRecruit {
                clan_id,
                karma,
                information,
                detailed_information,
                application_type,
                recruit_type,
            } => {
                clans::update_pledge_recruit(
                    &db,
                    clan_id,
                    karma,
                    information,
                    detailed_information,
                    application_type,
                    recruit_type,
                )
                .await
            }
            DbCommand::DeletePledgeRecruit { clan_id } => {
                clans::delete_pledge_recruit(&db, clan_id).await
            }
            DbCommand::SaveClanWar {
                attacker,
                attacked,
                attacker_kills,
                attacked_kills,
                winner,
                start_time,
                end_time,
                state,
            } => {
                clans::save_clan_war(
                    &db,
                    attacker,
                    attacked,
                    attacker_kills,
                    attacked_kills,
                    winner,
                    start_time,
                    end_time,
                    state,
                )
                .await
            }
            DbCommand::DeleteClanWar { clan1, clan2 } => {
                clans::delete_clan_war(&db, clan1, clan2).await
            }
            DbCommand::UpdateClanNewLeader {
                clan_id,
                new_leader_id,
            } => clans::update_clan_new_leader(&db, clan_id, new_leader_id).await,
            DbCommand::UpdateCharClanJoinExpiry { char_id, expiry } => {
                clans::update_char_clan_join_expiry(&db, char_id, expiry).await
            }
            DbCommand::DestroyClan {
                clan_id,
                leader_id,
                leader_expiry,
            } => clans::destroy_clan(&db, clan_id, leader_id, leader_expiry).await,
            DbCommand::StoreClanWarehouse { clan_id, items } => {
                clans::store_clan_warehouse(&db, clan_id, items).await
            }
            DbCommand::SetAccessLevel { char_id, level } => {
                characters::set_access_level(&db, char_id, level).await
            }
            DbCommand::StoreAccountVar {
                account_name,
                var,
                value,
            } => characters::store_account_var(&db, account_name, var, value).await,
            DbCommand::StoreCharVar {
                char_id,
                var,
                value,
            } => characters::store_char_var(&db, char_id, var, value).await,
            DbCommand::StorePremium {
                account_name,
                enddate,
            } => characters::store_premium(&db, account_name, enddate).await,
            DbCommand::DeletePremium { account_name } => {
                characters::delete_premium(&db, account_name).await
            }
            DbCommand::StoreMail { message } => social::store_mail(&db, message).await,
            DbCommand::UpdateMailFlags {
                message_id,
                unread,
                has_attachments,
                deleted_by_sender,
                deleted_by_receiver,
            } => {
                social::update_mail_flags(
                    &db,
                    message_id,
                    unread,
                    has_attachments,
                    deleted_by_sender,
                    deleted_by_receiver,
                )
                .await
            }
            DbCommand::DeleteMail { message_id } => social::delete_mail(&db, message_id).await,
            DbCommand::StoreOfflineWarehouseItems { owner_id, items } => {
                commerce::store_offline_warehouse_items(&db, owner_id, items).await
            }
            DbCommand::StoreMailItems {
                message_id,
                owner_id,
                items,
            } => social::store_mail_items(&db, message_id, owner_id, items).await,
            DbCommand::StoreLottery {
                idnr,
                enddate,
                prize,
            } => minigames::store_lottery(&db, idnr, enddate, prize).await,
            DbCommand::FinishLottery {
                idnr,
                prize,
                newprize,
                number1,
                number2,
                prize1,
                prize2,
                prize3,
            } => {
                minigames::finish_lottery(
                    &db, idnr, prize, newprize, number1, number2, prize1, prize2, prize3,
                )
                .await
            }
            DbCommand::IncreaseLotteryPrize { idnr, prize } => {
                minigames::increase_lottery_prize(&db, idnr, prize).await
            }
            DbCommand::LoadCustomMail => social::load_custom_mail(&db, &event_tx).await,
            DbCommand::LoadBirthdays { days } => social::load_birthdays(&db, &event_tx, days).await,
            DbCommand::DeleteCustomMail { date, receiver } => {
                social::delete_custom_mail(&db, date, receiver).await
            }
            DbCommand::LoadLotteryTickets { round } => {
                minigames::load_lottery_tickets(&db, &event_tx, round).await
            }
            DbCommand::SaveMdtHistory {
                race_id,
                first,
                second,
                odd_rate,
            } => minigames::save_mdt_history(&db, race_id, first, second, odd_rate).await,
            DbCommand::SaveMdtBet { lane, bet } => minigames::save_mdt_bet(&db, lane, bet).await,
            DbCommand::ClearMdtBets => minigames::clear_mdt_bets(&db).await,
            DbCommand::StoreItemAuction {
                auction_id,
                instance_id,
                auction_item_id,
                starting_time,
                ending_time,
                state_id,
            } => {
                commerce::store_item_auction(
                    &db,
                    auction_id,
                    instance_id,
                    auction_item_id,
                    starting_time,
                    ending_time,
                    state_id,
                )
                .await
            }
            DbCommand::StoreItemAuctionBid {
                auction_id,
                player_obj_id,
                bid,
            } => commerce::store_item_auction_bid(&db, auction_id, player_obj_id, bid).await,
            DbCommand::DeleteItemAuctionBid {
                auction_id,
                player_obj_id,
            } => commerce::delete_item_auction_bid(&db, auction_id, player_obj_id).await,
            DbCommand::DeleteItemAuction { auction_id } => {
                commerce::delete_item_auction(&db, auction_id).await
            }
            DbCommand::StoreBotReports { rows } => social::store_bot_reports(&db, rows).await,
            DbCommand::StorePunishment {
                id,
                key,
                affect,
                ptype,
                expiration,
                reason,
                punished_by,
            } => {
                social::store_punishment(
                    &db,
                    id,
                    key,
                    affect,
                    ptype,
                    expiration,
                    reason,
                    punished_by,
                )
                .await
            }
            DbCommand::DeletePunishment { id } => social::delete_punishment(&db, id).await,
            DbCommand::StorePetitionFeedback {
                char_name,
                gm_name,
                rate,
                message,
                date,
            } => {
                social::store_petition_feedback(&db, char_name, gm_name, rate, message, date).await
            }
            DbCommand::StoreOfflineWarehouseItem {
                owner_id,
                object_id,
                item_id,
                count,
                enchant,
            } => {
                commerce::store_offline_warehouse_item(
                    &db, owner_id, object_id, item_id, count, enchant,
                )
                .await
            }
            DbCommand::StoreBufferScheme {
                object_id,
                scheme_name,
                skills,
            } => characters::store_buffer_scheme(&db, object_id, scheme_name, skills).await,
            DbCommand::DeleteBufferScheme {
                object_id,
                scheme_name,
            } => characters::delete_buffer_scheme(&db, object_id, scheme_name).await,
            DbCommand::StoreFavorite {
                fav_id,
                player_id,
                title,
                bypass,
                add_date,
            } => characters::store_favorite(&db, fav_id, player_id, title, bypass, add_date).await,
            DbCommand::DeleteFavorite { player_id, fav_id } => {
                characters::delete_favorite(&db, player_id, fav_id).await
            }
            DbCommand::ResetRecommends => characters::reset_recommends(&db).await,
            DbCommand::ResetWorldChatPoints => characters::reset_world_chat_points(&db).await,
            DbCommand::ResetVitality { weekly } => characters::reset_vitality(&db, weekly).await,
            DbCommand::RepairCharacter { char_name } => {
                characters::repair_character(&db, char_name).await
            }
            DbCommand::Shutdown => break,
        }
    }

    let _ = db.close().await;
    info!("DB thread: stopped.");
}

/// Drop every item attached to a mail message.
///
/// `loc = 'MAIL'` **and** `loc_data = <message id>` together: `loc_data` is a
/// generic slot number reused by every storage kind, so filtering on it alone
/// would take warehouse and paperdoll rows with it.
///
/// Both the delete of a message and the store of one begin here — the store
/// because it rewrites the attachment set wholesale, and re-inserting over a
/// stale row would leave items the sender has already taken back.
async fn clear_mail_items(db: &DatabaseConnection, message_id: i32) {
    warn_err(
        entity::items::Entity::delete_many()
            .filter(entity::items::Column::Loc.eq("MAIL"))
            .filter(entity::items::Column::LocData.eq(message_id))
            .exec(db)
            .await,
    );
}

/// `UPDATE <E> SET … WHERE key = id` — the shape most of the `DbCommand` arms
/// above reduce to. **One statement**, however many columns are passed: the
/// arms that set several fields set them together, and issuing a statement per
/// column would make a half-applied write visible.
///
/// `update_many` rather than a loaded `ActiveModel`: the game thread already
/// holds the authoritative value, so re-reading the row first would be a round
/// trip to learn something we are about to overwrite. Failures are logged and
/// swallowed by [`warn_err`], like every other write on this thread — the DB
/// mirrors live state rather than owning it, so a lost write must not take the
/// server down with it.
async fn set_cols<E: EntityTrait>(
    db: &DatabaseConnection,
    key: E::Column,
    id: i32,
    cols: Vec<(E::Column, SimpleExpr)>,
) {
    let mut q = E::update_many();
    for (col, val) in cols {
        q = q.col_expr(col, val);
    }
    warn_err(q.filter(key.eq(id)).exec(db).await);
}

/// [`set_cols`] for the one-column case, which is most of them.
async fn set_col<E: EntityTrait>(
    db: &DatabaseConnection,
    key: E::Column,
    id: i32,
    col: E::Column,
    val: SimpleExpr,
) {
    set_cols::<E>(db, key, id, vec![(col, val)]).await;
}

/// [`set_col`] on `characters`, keyed by `char_id`.
async fn set_char_col(
    db: &DatabaseConnection,
    char_id: i32,
    col: entity::characters::Column,
    val: SimpleExpr,
) {
    set_char_cols(db, char_id, vec![(col, val)]).await;
}

/// [`set_cols`] on `characters`, keyed by `char_id`.
async fn set_char_cols(
    db: &DatabaseConnection,
    char_id: i32,
    cols: Vec<(entity::characters::Column, SimpleExpr)>,
) {
    set_cols::<entity::characters::Entity>(db, entity::characters::Column::CharId, char_id, cols)
        .await;
}

/// [`set_cols`] on `clan_data`, keyed by `clan_id`.
async fn set_clan_cols(
    db: &DatabaseConnection,
    clan_id: i32,
    cols: Vec<(entity::clan_data::Column, SimpleExpr)>,
) {
    set_cols::<entity::clan_data::Entity>(db, entity::clan_data::Column::ClanId, clan_id, cols)
        .await;
}

/// [`set_col`] on `clan_data`, keyed by `clan_id`.
async fn set_clan_col(
    db: &DatabaseConnection,
    clan_id: i32,
    col: entity::clan_data::Column,
    val: SimpleExpr,
) {
    set_clan_cols(db, clan_id, vec![(col, val)]).await;
}

/// [`set_col`] on `castle`, keyed by `castle_id`.
async fn set_castle_col(
    db: &DatabaseConnection,
    castle_id: i32,
    col: entity::castle::Column,
    val: SimpleExpr,
) {
    set_col::<entity::castle::Entity>(db, entity::castle::Column::Id, castle_id, col, val).await;
}
