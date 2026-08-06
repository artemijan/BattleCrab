use super::*;

pub(crate) async fn run(
    url: String,
    max_connections: u32,
    max_characters: i32,
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

    let mut next_id = load_next_id(&db).await;

    // Hand the game thread its initial runtime-id block unprompted (it can't
    // ask before it knows the DB thread is up; see `DbCommand::ReserveIds`).
    let _ = event_tx.send(DbEvent::IdBlock {
        start: next_id,
        end: next_id + ID_BLOCK_SIZE,
    });
    next_id += ID_BLOCK_SIZE;

    send_boot_events(&db, &event_tx).await;

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            DbCommand::LoadCharacters { client_id, account } => {
                reload(&db, &event_tx, client_id, account, true).await;
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
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Deletetime, delete_time.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
                reload(&db, &event_tx, client_id, account, true).await;
            }
            DbCommand::RestoreCharacter {
                client_id,
                account,
                char_id,
            } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Deletetime, 0.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
                reload(&db, &event_tx, client_id, account, true).await;
            }
            DbCommand::DeleteCharacter { char_id } => {
                delete_char(&db, char_id).await;
            }
            DbCommand::StoreGrandBoss { boss } => {
                warn_err(
                    grandboss_data::Entity::update_many()
                        .col_expr(grandboss_data::Column::LocX, boss.loc_x.into())
                        .col_expr(grandboss_data::Column::LocY, boss.loc_y.into())
                        .col_expr(grandboss_data::Column::LocZ, boss.loc_z.into())
                        .col_expr(grandboss_data::Column::Heading, boss.heading.into())
                        .col_expr(
                            grandboss_data::Column::RespawnTime,
                            boss.respawn_time.into(),
                        )
                        .col_expr(grandboss_data::Column::CurrentHp, boss.current_hp.into())
                        .col_expr(grandboss_data::Column::CurrentMp, boss.current_mp.into())
                        .col_expr(grandboss_data::Column::Status, boss.status.into())
                        .filter(grandboss_data::Column::BossId.eq(boss.boss_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::DeletePetRow { collar_object_id } => {
                warn_err(pets::Entity::delete_by_id(collar_object_id).exec(&db).await);
            }
            DbCommand::CountCharacters { account } => {
                let (count, del_times) = count_characters(&db, &account).await;
                let _ = event_tx.send(DbEvent::CharCount {
                    account,
                    count,
                    del_times,
                });
            }
            DbCommand::CheckNameCreatable { client_id, name } => {
                // RequestCharacterNameCreatable: NAME_ALREADY_EXISTS=2,
                // INVALID_LENGTH=3, creatable=-1 (validity was checked already).
                let result = if name_exists(&db, &name).await {
                    2
                } else if name.chars().count() > 16 {
                    3
                } else {
                    -1
                };
                let _ = event_tx.send(DbEvent::NameCreatable { client_id, result });
            }
            DbCommand::StorePlayer { save } => {
                store_player(&db, &save).await;
            }
            DbCommand::ReserveIds { count } => {
                let _ = event_tx.send(DbEvent::IdBlock {
                    start: next_id,
                    end: next_id + count,
                });
                next_id += count;
            }
            DbCommand::InsertFriendPair { a, b } => {
                // Both directions in one statement, as Java's two-row INSERT does.
                warn_err(
                    character_friends::Entity::insert_many([
                        character_friends::ActiveModel {
                            char_id: Set(a),
                            friend_id: Set(b),
                            relation: Set(0),
                            memo: NotSet,
                        },
                        character_friends::ActiveModel {
                            char_id: Set(b),
                            friend_id: Set(a),
                            relation: Set(0),
                            memo: NotSet,
                        },
                    ])
                    .on_conflict(
                        OnConflict::columns([
                            character_friends::Column::CharId,
                            character_friends::Column::FriendId,
                        ])
                        .do_nothing()
                        .to_owned(),
                    )
                    .exec_without_returning(&db)
                    .await,
                );
            }
            DbCommand::DeleteFriendPair { a, b } => {
                warn_err(
                    character_friends::Entity::delete_many()
                        .filter(
                            Condition::any()
                                .add(
                                    Condition::all()
                                        .add(character_friends::Column::CharId.eq(a))
                                        .add(character_friends::Column::FriendId.eq(b)),
                                )
                                .add(
                                    Condition::all()
                                        .add(character_friends::Column::CharId.eq(b))
                                        .add(character_friends::Column::FriendId.eq(a)),
                                ),
                        )
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::InsertClan {
                clan_id,
                name,
                leader_id,
            } => {
                warn_err(
                    clan_data::Entity::insert(clan_data::ActiveModel {
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
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::UpdateCharClan {
                char_id,
                clan_id,
                clan_privs,
            } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Clanid, clan_id.into())
                        .col_expr(characters::Column::ClanPrivs, clan_privs.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::SaveClanSkill {
                clan_id,
                skill_id,
                skill_level,
                skill_name,
            } => {
                warn_err(
                    clan_skills::Entity::insert(clan_skills::ActiveModel {
                        clan_id: Set(clan_id),
                        skill_id: Set(skill_id),
                        skill_level: Set(skill_level),
                        skill_name: Set(Some(skill_name)),
                        sub_pledge_id: Set(-2),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            clan_skills::Column::ClanId,
                            clan_skills::Column::SkillId,
                            clan_skills::Column::SubPledgeId,
                        ])
                        .update_columns([
                            clan_skills::Column::SkillLevel,
                            clan_skills::Column::SkillName,
                        ])
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeleteClanSkill { clan_id, skill_id } => {
                warn_err(
                    clan_skills::Entity::delete_many()
                        .filter(clan_skills::Column::ClanId.eq(clan_id))
                        .filter(clan_skills::Column::SkillId.eq(skill_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreCursedWeapon {
                item_id,
                char_id,
                reputation,
                pk_kills,
                nb_kills,
                end_time,
            } => {
                warn_err(
                    cursed_weapons::Entity::insert(cursed_weapons::ActiveModel {
                        item_id: Set(item_id),
                        char_id: Set(char_id),
                        player_reputation: Set(Some(reputation)),
                        player_pk_kills: Set(Some(pk_kills)),
                        nb_kills: Set(Some(nb_kills)),
                        end_time: Set(end_time),
                    })
                    .on_conflict(
                        OnConflict::column(cursed_weapons::Column::ItemId)
                            .update_columns([
                                cursed_weapons::Column::CharId,
                                cursed_weapons::Column::PlayerReputation,
                                cursed_weapons::Column::PlayerPkKills,
                                cursed_weapons::Column::NbKills,
                                cursed_weapons::Column::EndTime,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
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
                warn_err(
                    npc_respawns::Entity::insert(npc_respawns::ActiveModel {
                        id: Set(npc_id),
                        x: Set(x),
                        y: Set(y),
                        z: Set(z),
                        heading: Set(heading),
                        respawn_time: Set(respawn_time),
                        current_hp: Set(cur_hp),
                        current_mp: Set(cur_mp),
                    })
                    .on_conflict(
                        OnConflict::column(npc_respawns::Column::Id)
                            .update_columns([
                                npc_respawns::Column::X,
                                npc_respawns::Column::Y,
                                npc_respawns::Column::Z,
                                npc_respawns::Column::Heading,
                                npc_respawns::Column::RespawnTime,
                                npc_respawns::Column::CurrentHp,
                                npc_respawns::Column::CurrentMp,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::WipeSubclassSlot {
                char_id,
                class_index,
                old_class_id,
            } => {
                warn_err(
                    character_subclasses::Entity::delete_many()
                        .filter(character_subclasses::Column::CharId.eq(char_id))
                        .filter(character_subclasses::Column::ClassId.eq(old_class_id))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    character_skills::Entity::delete_many()
                        .filter(character_skills::Column::CharId.eq(char_id))
                        .filter(character_skills::Column::ClassIndex.eq(class_index))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    character_hennas::Entity::delete_many()
                        .filter(character_hennas::Column::CharId.eq(char_id))
                        .filter(character_hennas::Column::ClassIndex.eq(class_index))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    character_shortcuts::Entity::delete_many()
                        .filter(character_shortcuts::Column::CharId.eq(char_id))
                        .filter(character_shortcuts::Column::ClassIndex.eq(class_index))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreSubClass {
                char_id,
                class_id,
                class_index,
                level,
                exp,
                sp,
            } => {
                warn_err(
                    character_subclasses::Entity::insert(character_subclasses::ActiveModel {
                        char_id: Set(char_id),
                        class_id: Set(class_id),
                        exp: Set(exp),
                        sp: Set(sp),
                        level: Set(level),
                        vitality_points: Set(0),
                        class_index: Set(class_index),
                        dual_class: Set(0),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            character_subclasses::Column::CharId,
                            character_subclasses::Column::ClassId,
                        ])
                        .update_columns([
                            character_subclasses::Column::Exp,
                            character_subclasses::Column::Sp,
                            character_subclasses::Column::Level,
                            character_subclasses::Column::ClassIndex,
                        ])
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeleteNpcRespawn { npc_id } => {
                warn_err(npc_respawns::Entity::delete_by_id(npc_id).exec(&db).await);
            }
            DbCommand::RemoveCursedWeapon { item_id } => {
                warn_err(
                    cursed_weapons::Entity::delete_by_id(item_id)
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::RestoreOfflineCursedOwner {
                char_id,
                item_id,
                reputation,
                pk_kills,
                skill_ids,
            } => {
                warn_err(
                    items::Entity::delete_many()
                        .filter(items::Column::OwnerId.eq(char_id))
                        .filter(items::Column::ItemId.eq(item_id))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Reputation, reputation.into())
                        .col_expr(characters::Column::Pkkills, pk_kills.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
                if !skill_ids.is_empty() {
                    warn_err(
                        character_skills::Entity::delete_many()
                            .filter(character_skills::Column::CharId.eq(char_id))
                            .filter(character_skills::Column::SkillId.is_in(skill_ids))
                            .exec(&db)
                            .await,
                    );
                }
            }
            DbCommand::UpdateCastleSide { castle_id, side } => {
                warn_err(
                    castle::Entity::update_many()
                        .col_expr(castle::Column::Side, side.into())
                        .filter(castle::Column::Id.eq(castle_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateCastleShowNpcCrest { castle_id, show } => {
                warn_err(
                    castle::Entity::update_many()
                        .col_expr(
                            castle::Column::ShowNpcCrest,
                            if show { "true" } else { "false" }.into(),
                        )
                        .filter(castle::Column::Id.eq(castle_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanLeader { clan_id, leader_id } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::LeaderId, leader_id.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanCastle { clan_id, castle_id } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::HasCastle, castle_id.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanBloodAlliance { clan_id, count } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::BloodAllianceCount, count.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateCastleTicketCount { castle_id, count } => {
                warn_err(
                    castle::Entity::update_many()
                        .col_expr(castle::Column::TicketBuyCount, count.into())
                        .filter(castle::Column::Id.eq(castle_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::AddFreightItems { owner_id, items } => {
                for it in &items {
                    warn_err(
                        items::Entity::insert(items::ActiveModel {
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
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::StoreOfflineTrader {
                char_id,
                time,
                store_type,
                title,
                items,
            } => {
                // Java rewrites both tables for this trader (`onTransaction`
                // clears the item rows first, then re-inserts).
                warn_err(
                    character_offline_trade_items::Entity::delete_many()
                        .filter(character_offline_trade_items::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    character_offline_trade::Entity::delete_many()
                        .filter(character_offline_trade::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    character_offline_trade::Entity::insert(character_offline_trade::ActiveModel {
                        char_id: Set(char_id),
                        time: Set(time),
                        r#type: Set(store_type),
                        title: Set(Some(title)),
                    })
                    .exec(&db)
                    .await,
                );
                for (item, count, price) in &items {
                    warn_err(
                        character_offline_trade_items::Entity::insert(
                            character_offline_trade_items::ActiveModel {
                                char_id: Set(char_id),
                                item: Set(*item),
                                count: Set(*count),
                                price: Set(*price),
                            },
                        )
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::ClearOfflineTrader { char_id } => {
                warn_err(
                    character_offline_trade_items::Entity::delete_many()
                        .filter(character_offline_trade_items::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    character_offline_trade::Entity::delete_many()
                        .filter(character_offline_trade::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreManor {
                castle_id,
                production,
                procure,
            } => {
                warn_err(
                    castle_manor_production::Entity::delete_many()
                        .filter(castle_manor_production::Column::CastleId.eq(castle_id))
                        .exec(&db)
                        .await,
                );
                for r in &production {
                    warn_err(
                        castle_manor_production::Entity::insert(
                            castle_manor_production::ActiveModel {
                                castle_id: Set(r.castle_id),
                                seed_id: Set(r.seed_id),
                                amount: Set(r.amount as i32),
                                start_amount: Set(r.start_amount as i32),
                                price: Set(r.price as i32),
                                next_period: Set(i32::from(r.next_period)),
                            },
                        )
                        .exec(&db)
                        .await,
                    );
                }
                warn_err(
                    castle_manor_procure::Entity::delete_many()
                        .filter(castle_manor_procure::Column::CastleId.eq(castle_id))
                        .exec(&db)
                        .await,
                );
                for r in &procure {
                    warn_err(
                        castle_manor_procure::Entity::insert(castle_manor_procure::ActiveModel {
                            castle_id: Set(r.castle_id),
                            crop_id: Set(r.crop_id),
                            amount: Set(r.amount as i32),
                            start_amount: Set(r.start_amount as i32),
                            price: Set(r.price as i32),
                            reward_type: Set(r.reward_type),
                            next_period: Set(i32::from(r.next_period)),
                        })
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::UpdateCastleTreasury {
                castle_id,
                treasury,
            } => {
                warn_err(
                    castle::Entity::update_many()
                        .col_expr(castle::Column::Treasury, treasury.into())
                        .filter(castle::Column::Id.eq(castle_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateCastleSiegeTime {
                castle_id,
                siege_date,
                time_registration_over,
                siege_time_registration_end,
            } => {
                // `regTimeOver` is an enum('true','false') stored as text.
                let flag = if time_registration_over {
                    "true"
                } else {
                    "false"
                };
                let mut update = castle::Entity::update_many()
                    .col_expr(castle::Column::SiegeDate, siege_date.into())
                    .col_expr(castle::Column::RegTimeOver, flag.into());
                if let Some(end) = siege_time_registration_end {
                    update = update.col_expr(castle::Column::RegTimeEnd, end.into());
                }
                warn_err(
                    update
                        .filter(castle::Column::Id.eq(castle_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::SaveGlobalVariable { var, value } => {
                warn_err(
                    global_variables::Entity::insert(global_variables::ActiveModel {
                        var: Set(var),
                        value: Set(Some(value)),
                    })
                    .on_conflict(
                        OnConflict::column(global_variables::Column::Var)
                            .update_column(global_variables::Column::Value)
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::SaveSiegeClan {
                castle_id,
                clan_id,
                kind,
            } => {
                warn_err(
                    siege_clans::Entity::insert(siege_clans::ActiveModel {
                        clan_id: Set(clan_id),
                        castle_id: Set(castle_id),
                        r#type: Set(Some(kind)),
                        castle_owner: Set(Some(0)),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            siege_clans::Column::ClanId,
                            siege_clans::Column::CastleId,
                        ])
                        .update_columns([
                            siege_clans::Column::Type,
                            siege_clans::Column::CastleOwner,
                        ])
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::RemoveSiegeClan { castle_id, clan_id } => {
                warn_err(
                    siege_clans::Entity::delete_many()
                        .filter(siege_clans::Column::CastleId.eq(castle_id))
                        .filter(siege_clans::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::SaveClanHallBid {
                hall_id,
                clan_id,
                bid,
                bid_time,
            } => {
                warn_err(
                    clanhall_auctions_bidders::Entity::insert(
                        clanhall_auctions_bidders::ActiveModel {
                            clan_hall_id: Set(hall_id),
                            clan_id: Set(clan_id),
                            bid: Set(bid),
                            bid_time: Set(bid_time),
                        },
                    )
                    .on_conflict(
                        OnConflict::columns([
                            clanhall_auctions_bidders::Column::ClanHallId,
                            clanhall_auctions_bidders::Column::ClanId,
                        ])
                        .update_columns([
                            clanhall_auctions_bidders::Column::Bid,
                            clanhall_auctions_bidders::Column::BidTime,
                        ])
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::RemoveClanHallBid { hall_id, clan_id } => {
                warn_err(
                    clanhall_auctions_bidders::Entity::delete_by_id((hall_id, clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::ClearClanHallBids { hall_id } => {
                warn_err(
                    clanhall_auctions_bidders::Entity::delete_many()
                        .filter(clanhall_auctions_bidders::Column::ClanHallId.eq(hall_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::SaveClanHall {
                id,
                owner_id,
                paid_until,
            } => {
                warn_err(
                    clanhall::Entity::insert(clanhall::ActiveModel {
                        id: Set(id),
                        owner_id: Set(owner_id),
                        paid_until: Set(paid_until),
                    })
                    .on_conflict(
                        OnConflict::column(clanhall::Column::Id)
                            .update_columns([
                                clanhall::Column::OwnerId,
                                clanhall::Column::PaidUntil,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::SaveResidenceFunction {
                residence_id,
                func_id,
                level,
                expiration,
            } => {
                warn_err(
                    residence_functions::Entity::insert(residence_functions::ActiveModel {
                        id: Set(func_id),
                        level: Set(level),
                        expiration: Set(expiration),
                        residence_id: Set(residence_id),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            residence_functions::Column::Id,
                            residence_functions::Column::Level,
                            residence_functions::Column::ResidenceId,
                        ])
                        .update_column(residence_functions::Column::Expiration)
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::RemoveResidenceFunction {
                residence_id,
                func_id,
            } => {
                warn_err(
                    residence_functions::Entity::delete_many()
                        .filter(residence_functions::Column::ResidenceId.eq(residence_id))
                        .filter(residence_functions::Column::Id.eq(func_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::SaveOlympiad {
                current_cycle,
                period,
                olympiad_end,
                validation_end,
                next_weekly_change,
                nobles,
            } => {
                warn_err(
                    olympiad_data::Entity::insert(olympiad_data::ActiveModel {
                        id: Set(0),
                        current_cycle: Set(current_cycle),
                        period: Set(period),
                        olympiad_end: Set(olympiad_end),
                        validation_end: Set(validation_end),
                        next_weekly_change: Set(next_weekly_change),
                    })
                    .on_conflict(
                        OnConflict::column(olympiad_data::Column::Id)
                            .update_columns([
                                olympiad_data::Column::CurrentCycle,
                                olympiad_data::Column::Period,
                                olympiad_data::Column::OlympiadEnd,
                                olympiad_data::Column::ValidationEnd,
                                olympiad_data::Column::NextWeeklyChange,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
                for n in nobles {
                    warn_err(
                        olympiad_nobles::Entity::insert(olympiad_nobles::ActiveModel {
                            char_id: Set(n.char_id),
                            class_id: Set(n.class_id),
                            olympiad_points: Set(n.points),
                            competitions_done: Set(n.comp_done),
                            competitions_won: Set(n.comp_won),
                            competitions_lost: Set(n.comp_lost),
                            competitions_drawn: Set(n.comp_drawn),
                            competitions_done_week: Set(n.comp_done_week),
                        })
                        .on_conflict(
                            OnConflict::column(olympiad_nobles::Column::CharId)
                                .update_columns([
                                    olympiad_nobles::Column::ClassId,
                                    olympiad_nobles::Column::OlympiadPoints,
                                    olympiad_nobles::Column::CompetitionsDone,
                                    olympiad_nobles::Column::CompetitionsWon,
                                    olympiad_nobles::Column::CompetitionsLost,
                                    olympiad_nobles::Column::CompetitionsDrawn,
                                    olympiad_nobles::Column::CompetitionsDoneWeek,
                                ])
                                .to_owned(),
                        )
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::SaveHeroes { heroes } => {
                // `Hero.computeNewHeroes` replaces the active crown.
                warn_err(heroes::Entity::delete_many().exec(&db).await);
                for h in heroes {
                    warn_err(
                        heroes::Entity::insert(heroes::ActiveModel {
                            char_id: Set(h.char_id),
                            class_id: Set(h.class_id),
                            count: Set(h.count),
                            played: Set(1),
                            claimed: Set(if h.claimed { "true" } else { "false" }.to_string()),
                            ..Default::default()
                        })
                        .on_conflict(
                            OnConflict::column(heroes::Column::CharId)
                                .update_columns([
                                    heroes::Column::ClassId,
                                    heroes::Column::Count,
                                    heroes::Column::Played,
                                    heroes::Column::Claimed,
                                ])
                                .to_owned(),
                        )
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::SnapshotOlympiadEom => {
                // Java runs `TRUNCATE olympiad_nobles_eom` then
                // `INSERT INTO olympiad_nobles_eom SELECT … FROM olympiad_nobles`.
                warn_err(olympiad_nobles_eom::Entity::delete_many().exec(&db).await);
                let live = olympiad_nobles::Entity::find()
                    .all(&db)
                    .await
                    .unwrap_or_default();
                for n in live {
                    warn_err(
                        olympiad_nobles_eom::Entity::insert(olympiad_nobles_eom::ActiveModel {
                            char_id: Set(n.char_id),
                            class_id: Set(n.class_id),
                            olympiad_points: Set(n.olympiad_points),
                            competitions_done: Set(n.competitions_done),
                            competitions_won: Set(n.competitions_won),
                            competitions_lost: Set(n.competitions_lost),
                            competitions_drawn: Set(n.competitions_drawn),
                        })
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::ClaimHero { char_id } => {
                warn_err(
                    heroes::Entity::update_many()
                        .col_expr(heroes::Column::Claimed, "true".into())
                        .filter(heroes::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::SaveHeroDiary {
                char_id,
                time,
                action,
                param,
            } => {
                warn_err(
                    heroes_diary::Entity::insert(heroes_diary::ActiveModel {
                        char_id: Set(char_id),
                        time: Set(time),
                        action: Set(action),
                        param: Set(param),
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::UpdateClanLevel { clan_id, level } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::ClanLevel, level.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanReputation {
                clan_id,
                reputation,
            } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::ReputationScore, reputation.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanPenalties {
                clan_id,
                char_penalty_expiry_time,
                dissolving_expiry_time,
            } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(
                            clan_data::Column::CharPenaltyExpiryTime,
                            char_penalty_expiry_time.into(),
                        )
                        .col_expr(
                            clan_data::Column::DissolvingExpiryTime,
                            dissolving_expiry_time.into(),
                        )
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::RemoveClanMember {
                char_id,
                clan_join_expiry,
                clan_create_expiry,
            } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Clanid, 0.into())
                        .col_expr(characters::Column::Title, "".into())
                        .col_expr(characters::Column::ClanPrivs, 0.into())
                        .col_expr(
                            characters::Column::ClanJoinExpiryTime,
                            clan_join_expiry.into(),
                        )
                        .col_expr(
                            characters::Column::ClanCreateExpiryTime,
                            clan_create_expiry.into(),
                        )
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::SaveClanRankPrivs {
                clan_id,
                rank,
                privs,
            } => {
                warn_err(
                    clan_privs::Entity::insert(clan_privs::ActiveModel {
                        clan_id: Set(clan_id),
                        rank: Set(rank),
                        party: Set(0),
                        privs: Set(privs),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            clan_privs::Column::ClanId,
                            clan_privs::Column::Rank,
                            clan_privs::Column::Party,
                        ])
                        .update_column(clan_privs::Column::Privs)
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::UpdateCharPowerGrade {
                char_id,
                power_grade,
            } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::PowerGrade, power_grade.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanAlly {
                clan_id,
                ally_id,
                ally_name,
                penalty_expiry,
                penalty_type,
            } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::AllyId, ally_id.into())
                        .col_expr(clan_data::Column::AllyName, ally_name.into())
                        .col_expr(
                            clan_data::Column::AllyPenaltyExpiryTime,
                            penalty_expiry.into(),
                        )
                        .col_expr(clan_data::Column::AllyPenaltyType, penalty_type.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::InsertSubPledge {
                clan_id,
                pledge_type,
                name,
                leader_id,
            } => {
                warn_err(
                    clan_subpledges::Entity::insert(clan_subpledges::ActiveModel {
                        clan_id: Set(clan_id),
                        sub_pledge_id: Set(pledge_type),
                        name: Set(Some(name)),
                        leader_id: Set(leader_id),
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::UpdateSubPledge {
                clan_id,
                pledge_type,
                name,
                leader_id,
            } => {
                warn_err(
                    clan_subpledges::Entity::update_many()
                        .col_expr(clan_subpledges::Column::LeaderId, leader_id.into())
                        .col_expr(clan_subpledges::Column::Name, name.into())
                        .filter(clan_subpledges::Column::ClanId.eq(clan_id))
                        .filter(clan_subpledges::Column::SubPledgeId.eq(pledge_type))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateCharAcademyLevel {
                char_id,
                lvl_joined_academy,
            } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(
                            characters::Column::LvlJoinedAcademy,
                            lvl_joined_academy.into(),
                        )
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateCharApprenticeSponsor {
                char_id,
                apprentice,
                sponsor,
            } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Apprentice, apprentice.into())
                        .col_expr(characters::Column::Sponsor, sponsor.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateCharPledgeType {
                char_id,
                pledge_type,
            } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Subpledge, pledge_type.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::InsertCrest { id, data, kind } => {
                warn_err(
                    crests::Entity::insert(crests::ActiveModel {
                        crest_id: Set(id),
                        data: Set(data),
                        r#type: Set(kind),
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeleteCrest { id } => {
                warn_err(
                    crests::Entity::delete_many()
                        .filter(crests::Column::CrestId.eq(id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanCrest { clan_id, crest_id } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::CrestId, crest_id.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanCrestLarge {
                clan_id,
                crest_large_id,
            } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::CrestLargeId, crest_large_id.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanAllyCrestSelf {
                clan_id,
                ally_crest_id,
            } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::AllyCrestId, ally_crest_id.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateAllyCrestForAlliance {
                ally_id,
                ally_crest_id,
            } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::AllyCrestId, ally_crest_id.into())
                        .filter(clan_data::Column::AllyId.eq(ally_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpsertPledgeApplicant {
                player_id,
                clan_id,
                karma,
                message,
            } => {
                warn_err(
                    pledge_applicant::Entity::insert(pledge_applicant::ActiveModel {
                        char_id: Set(player_id),
                        clan_id: Set(clan_id),
                        karma: Set(karma),
                        message: Set(message),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            pledge_applicant::Column::CharId,
                            pledge_applicant::Column::ClanId,
                        ])
                        .update_columns([
                            pledge_applicant::Column::Karma,
                            pledge_applicant::Column::Message,
                        ])
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeletePledgeApplicant { player_id, clan_id } => {
                warn_err(
                    pledge_applicant::Entity::delete_by_id((player_id, clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::InsertPledgeWaiting { player_id, karma } => {
                warn_err(
                    pledge_waiting_list::Entity::insert(pledge_waiting_list::ActiveModel {
                        char_id: Set(player_id),
                        karma: Set(karma),
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeletePledgeWaiting { player_id } => {
                warn_err(
                    pledge_waiting_list::Entity::delete_many()
                        .filter(pledge_waiting_list::Column::CharId.eq(player_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::InsertPledgeRecruit {
                clan_id,
                karma,
                information,
                detailed_information,
                application_type,
                recruit_type,
            } => {
                warn_err(
                    pledge_recruit::Entity::insert(pledge_recruit::ActiveModel {
                        clan_id: Set(clan_id),
                        karma: Set(karma),
                        information: Set(information),
                        detailed_information: Set(detailed_information),
                        application_type: Set(application_type),
                        recruit_type: Set(recruit_type),
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::UpdatePledgeRecruit {
                clan_id,
                karma,
                information,
                detailed_information,
                application_type,
                recruit_type,
            } => {
                warn_err(
                    pledge_recruit::Entity::update_many()
                        .col_expr(pledge_recruit::Column::Karma, karma.into())
                        .col_expr(pledge_recruit::Column::Information, information.into())
                        .col_expr(
                            pledge_recruit::Column::DetailedInformation,
                            detailed_information.into(),
                        )
                        .col_expr(
                            pledge_recruit::Column::ApplicationType,
                            application_type.into(),
                        )
                        .col_expr(pledge_recruit::Column::RecruitType, recruit_type.into())
                        .filter(pledge_recruit::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::DeletePledgeRecruit { clan_id } => {
                warn_err(
                    pledge_recruit::Entity::delete_many()
                        .filter(pledge_recruit::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
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
                // The clan-id columns are `varchar(35)`; SQLite stored the bound
                // integers as text anyway, and `load_clan_wars` parses them back.
                warn_err(
                    clan_wars::Entity::insert(clan_wars::ActiveModel {
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
                        OnConflict::columns([clan_wars::Column::Clan1, clan_wars::Column::Clan2])
                            .update_columns([
                                clan_wars::Column::Clan1Kill,
                                clan_wars::Column::Clan2Kill,
                                clan_wars::Column::WinnerClan,
                                clan_wars::Column::StartTime,
                                clan_wars::Column::EndTime,
                                clan_wars::Column::State,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeleteClanWar { clan1, clan2 } => {
                let (a, b) = (clan1.to_string(), clan2.to_string());
                warn_err(
                    clan_wars::Entity::delete_many()
                        .filter(
                            Condition::any()
                                .add(
                                    Condition::all()
                                        .add(clan_wars::Column::Clan1.eq(a.clone()))
                                        .add(clan_wars::Column::Clan2.eq(b.clone())),
                                )
                                .add(
                                    Condition::all()
                                        .add(clan_wars::Column::Clan1.eq(b))
                                        .add(clan_wars::Column::Clan2.eq(a)),
                                ),
                        )
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanNewLeader {
                clan_id,
                new_leader_id,
            } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::NewLeaderId, new_leader_id.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateCharClanJoinExpiry { char_id, expiry } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::ClanJoinExpiryTime, expiry.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::DestroyClan {
                clan_id,
                leader_id,
                leader_expiry,
            } => {
                warn_err(
                    clan_data::Entity::delete_many()
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    clan_skills::Entity::delete_many()
                        .filter(clan_skills::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Clanid, 0.into())
                        .col_expr(characters::Column::ClanPrivs, 0.into())
                        .filter(characters::Column::Clanid.eq(clan_id))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(
                            characters::Column::ClanCreateExpiryTime,
                            leader_expiry.into(),
                        )
                        .filter(characters::Column::CharId.eq(leader_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreClanWarehouse { clan_id, items } => {
                warn_err(
                    items::Entity::delete_many()
                        .filter(items::Column::OwnerId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
                for it in &items {
                    warn_err(
                        items::Entity::insert(items::ActiveModel {
                            owner_id: Set(Some(clan_id)),
                            object_id: Set(it.object_id),
                            item_id: Set(Some(it.item_id)),
                            count: Set(it.count),
                            enchant_level: Set(Some(it.enchant_level)),
                            loc: Set(Some(it.loc.clone())),
                            loc_data: Set(Some(it.loc_data)),
                            custom_type1: Set(Some(it.custom_type1)),
                            custom_type2: Set(Some(it.custom_type2)),
                            mana_left: Set(it.mana_left),
                            time: Set(it.time.into()),
                            ..Default::default()
                        })
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::SetAccessLevel { char_id, level } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Accesslevel, level.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreAccountVar {
                account_name,
                var,
                value,
            } => {
                warn_err(
                    account_gsdata::Entity::insert(account_gsdata::ActiveModel {
                        account_name: Set(account_name),
                        var: Set(var),
                        value: Set(value),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            account_gsdata::Column::AccountName,
                            account_gsdata::Column::Var,
                        ])
                        .update_column(account_gsdata::Column::Value)
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::StoreCharVar {
                char_id,
                var,
                value,
            } => {
                // The table has no unique key, so replace by delete + insert
                // (Java `REMOVE_UNCLAIMED_POINTS` then `INSERT_UNCLAIMED_POINTS`).
                warn_err(
                    character_variables::Entity::delete_many()
                        .filter(character_variables::Column::CharId.eq(char_id))
                        .filter(character_variables::Column::Var.eq(var.clone()))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    character_variables::Entity::insert(character_variables::ActiveModel {
                        char_id: Set(char_id),
                        var: Set(var),
                        val: Set(value),
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::StorePremium {
                account_name,
                enddate,
            } => {
                warn_err(
                    account_premium::Entity::insert(account_premium::ActiveModel {
                        account_name: Set(account_name),
                        enddate: Set(enddate),
                    })
                    .on_conflict(
                        OnConflict::column(account_premium::Column::AccountName)
                            .update_column(account_premium::Column::Enddate)
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeletePremium { account_name } => {
                warn_err(
                    account_premium::Entity::delete_by_id(account_name)
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreMail { message } => {
                // The boolean-ish columns are enum('true','false') text.
                let b = |v: bool| if v { "true" } else { "false" }.to_string();
                warn_err(
                    messages::Entity::insert(messages::ActiveModel {
                        message_id: Set(message.message_id),
                        sender_id: Set(message.sender_id),
                        receiver_id: Set(message.receiver_id),
                        subject: Set(Some(message.subject.clone())),
                        content: Set(Some(message.content.clone())),
                        expiration: Set(message.expiration),
                        req_adena: Set(message.req_adena),
                        has_attachments: Set(b(message.has_attachments)),
                        is_unread: Set(b(message.unread)),
                        is_deleted_by_sender: Set(b(message.deleted_by_sender)),
                        is_deleted_by_receiver: Set(b(message.deleted_by_receiver)),
                        send_by_system: Set(message.send_by_system),
                        is_returned: Set(b(message.returned)),
                        ..Default::default()
                    })
                    .on_conflict(
                        OnConflict::column(messages::Column::MessageId)
                            .update_columns([
                                messages::Column::SenderId,
                                messages::Column::ReceiverId,
                                messages::Column::Subject,
                                messages::Column::Content,
                                messages::Column::Expiration,
                                messages::Column::ReqAdena,
                                messages::Column::HasAttachments,
                                messages::Column::IsUnread,
                                messages::Column::IsDeletedBySender,
                                messages::Column::IsDeletedByReceiver,
                                messages::Column::SendBySystem,
                                messages::Column::IsReturned,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::UpdateMailFlags {
                message_id,
                unread,
                has_attachments,
                deleted_by_sender,
                deleted_by_receiver,
            } => {
                let b = |v: bool| if v { "true" } else { "false" };
                warn_err(
                    messages::Entity::update_many()
                        .col_expr(messages::Column::IsUnread, b(unread).into())
                        .col_expr(messages::Column::HasAttachments, b(has_attachments).into())
                        .col_expr(
                            messages::Column::IsDeletedBySender,
                            b(deleted_by_sender).into(),
                        )
                        .col_expr(
                            messages::Column::IsDeletedByReceiver,
                            b(deleted_by_receiver).into(),
                        )
                        .filter(messages::Column::MessageId.eq(message_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::DeleteMail { message_id } => {
                warn_err(messages::Entity::delete_by_id(message_id).exec(&db).await);
                warn_err(
                    items::Entity::delete_many()
                        .filter(items::Column::Loc.eq("MAIL"))
                        .filter(items::Column::LocData.eq(message_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreOfflineWarehouseItems { owner_id, items } => {
                for it in &items {
                    warn_err(
                        items::Entity::insert(items::ActiveModel {
                            owner_id: Set(Some(owner_id)),
                            object_id: Set(it.object_id),
                            item_id: Set(Some(it.item_id)),
                            count: Set(it.count),
                            enchant_level: Set(Some(it.enchant_level)),
                            loc: Set(Some("WAREHOUSE".to_string())),
                            loc_data: Set(Some(0)),
                            custom_type1: Set(Some(it.custom_type1)),
                            custom_type2: Set(Some(it.custom_type2)),
                            mana_left: Set(it.mana_left),
                            time: Set(it.time.into()),
                            ..Default::default()
                        })
                        .on_conflict(
                            OnConflict::column(items::Column::ObjectId)
                                .update_columns([
                                    items::Column::OwnerId,
                                    items::Column::ItemId,
                                    items::Column::Count,
                                    items::Column::EnchantLevel,
                                    items::Column::Loc,
                                    items::Column::LocData,
                                    items::Column::CustomType1,
                                    items::Column::CustomType2,
                                    items::Column::ManaLeft,
                                    items::Column::Time,
                                ])
                                .to_owned(),
                        )
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::StoreMailItems {
                message_id,
                owner_id,
                items,
            } => {
                warn_err(
                    items::Entity::delete_many()
                        .filter(items::Column::Loc.eq("MAIL"))
                        .filter(items::Column::LocData.eq(message_id))
                        .exec(&db)
                        .await,
                );
                for it in &items {
                    warn_err(
                        items::Entity::insert(items::ActiveModel {
                            owner_id: Set(Some(owner_id)),
                            object_id: Set(it.object_id),
                            item_id: Set(Some(it.item_id)),
                            count: Set(it.count),
                            enchant_level: Set(Some(it.enchant_level)),
                            loc: Set(Some("MAIL".to_string())),
                            loc_data: Set(Some(message_id)),
                            custom_type1: Set(Some(it.custom_type1)),
                            custom_type2: Set(Some(it.custom_type2)),
                            mana_left: Set(it.mana_left),
                            time: Set(it.time.into()),
                            ..Default::default()
                        })
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::StoreLottery {
                idnr,
                enddate,
                prize,
            } => {
                warn_err(
                    lottery::Entity::insert(lottery::ActiveModel {
                        id: Set(1),
                        idnr: Set(idnr),
                        enddate: Set(enddate),
                        prize: Set(prize),
                        newprize: Set(prize),
                        ..Default::default()
                    })
                    .on_conflict(
                        OnConflict::columns([lottery::Column::Id, lottery::Column::Idnr])
                            .update_columns([
                                lottery::Column::Enddate,
                                lottery::Column::Prize,
                                lottery::Column::Newprize,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
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
                warn_err(
                    lottery::Entity::update_many()
                        .col_expr(lottery::Column::Finished, 1.into())
                        .col_expr(lottery::Column::Prize, prize.into())
                        .col_expr(lottery::Column::Newprize, newprize.into())
                        .col_expr(lottery::Column::Number1, number1.into())
                        .col_expr(lottery::Column::Number2, number2.into())
                        .col_expr(lottery::Column::Prize1, prize1.into())
                        .col_expr(lottery::Column::Prize2, prize2.into())
                        .col_expr(lottery::Column::Prize3, prize3.into())
                        .filter(lottery::Column::Id.eq(1))
                        .filter(lottery::Column::Idnr.eq(idnr))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::IncreaseLotteryPrize { idnr, prize } => {
                warn_err(
                    lottery::Entity::update_many()
                        .col_expr(lottery::Column::Prize, prize.into())
                        .col_expr(lottery::Column::Newprize, prize.into())
                        .filter(lottery::Column::Id.eq(1))
                        .filter(lottery::Column::Idnr.eq(idnr))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::LoadCustomMail => {
                let rows = custom_mail::Entity::find()
                    .all(&db)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|r| CustomMailRow {
                        date: r.date,
                        receiver: r.receiver,
                        subject: r.subject,
                        message: r.message,
                        items: r.items,
                    })
                    .collect();
                let _ = event_tx.send(DbEvent::CustomMailLoaded { rows });
            }
            DbCommand::DeleteCustomMail { date, receiver } => {
                warn_err(
                    custom_mail::Entity::delete_many()
                        .filter(custom_mail::Column::Date.eq(date))
                        .filter(custom_mail::Column::Receiver.eq(receiver))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::LoadLotteryTickets { round } => {
                // Lottery tickets are ordinary items (id 4442) whose
                // `custom_type1` is the round they were bought in.
                let rows = items::Entity::find()
                    .filter(items::Column::ItemId.eq(4442))
                    .filter(items::Column::CustomType1.eq(round))
                    .all(&db)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|r| {
                        (
                            r.object_id,
                            r.enchant_level.unwrap_or(0),
                            r.custom_type2.unwrap_or(0),
                        )
                    })
                    .collect();
                let _ = event_tx.send(DbEvent::LotteryTicketsLoaded { round, rows });
            }
            DbCommand::SaveMdtHistory {
                race_id,
                first,
                second,
                odd_rate,
            } => {
                warn_err(
                    mdt_history::Entity::insert(mdt_history::ActiveModel {
                        race_id: Set(race_id),
                        first: Set(Some(first)),
                        second: Set(Some(second)),
                        odd_rate: Set(Some(odd_rate)),
                    })
                    .on_conflict(
                        OnConflict::column(mdt_history::Column::RaceId)
                            .update_columns([
                                mdt_history::Column::First,
                                mdt_history::Column::Second,
                                mdt_history::Column::OddRate,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::SaveMdtBet { lane, bet } => {
                warn_err(
                    mdt_bets::Entity::insert(mdt_bets::ActiveModel {
                        lane_id: Set(lane),
                        bet: Set(Some(bet)),
                    })
                    .on_conflict(
                        OnConflict::column(mdt_bets::Column::LaneId)
                            .update_column(mdt_bets::Column::Bet)
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::ClearMdtBets => {
                warn_err(
                    mdt_bets::Entity::update_many()
                        .col_expr(mdt_bets::Column::Bet, 0.into())
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreItemAuction {
                auction_id,
                instance_id,
                auction_item_id,
                starting_time,
                ending_time,
                state_id,
            } => {
                warn_err(
                    item_auction::Entity::insert(item_auction::ActiveModel {
                        auction_id: Set(auction_id),
                        instance_id: Set(instance_id),
                        auction_item_id: Set(auction_item_id),
                        starting_time: Set(starting_time),
                        ending_time: Set(ending_time),
                        auction_state_id: Set(state_id.into()),
                    })
                    .on_conflict(
                        OnConflict::column(item_auction::Column::AuctionId)
                            .update_columns([
                                item_auction::Column::InstanceId,
                                item_auction::Column::AuctionItemId,
                                item_auction::Column::StartingTime,
                                item_auction::Column::EndingTime,
                                item_auction::Column::AuctionStateId,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::StoreItemAuctionBid {
                auction_id,
                player_obj_id,
                bid,
            } => {
                warn_err(
                    item_auction_bid::Entity::insert(item_auction_bid::ActiveModel {
                        auction_id: Set(auction_id),
                        player_obj_id: Set(player_obj_id),
                        player_bid: Set(bid),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            item_auction_bid::Column::AuctionId,
                            item_auction_bid::Column::PlayerObjId,
                        ])
                        .update_column(item_auction_bid::Column::PlayerBid)
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeleteItemAuctionBid {
                auction_id,
                player_obj_id,
            } => {
                warn_err(
                    item_auction_bid::Entity::delete_by_id((auction_id, player_obj_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::DeleteItemAuction { auction_id } => {
                warn_err(
                    item_auction::Entity::delete_by_id(auction_id)
                        .exec(&db)
                        .await,
                );
                warn_err(
                    item_auction_bid::Entity::delete_many()
                        .filter(item_auction_bid::Column::AuctionId.eq(auction_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreBotReports { rows } => {
                // Java clears first and re-inserts the whole table.
                warn_err(
                    bot_reported_char_data::Entity::delete_many()
                        .exec(&db)
                        .await,
                );
                for (bot_id, reporter_id, report_date) in rows {
                    warn_err(
                        bot_reported_char_data::Entity::insert(
                            bot_reported_char_data::ActiveModel {
                                bot_id: Set(bot_id),
                                reporter_id: Set(reporter_id),
                                report_date: Set(report_date),
                            },
                        )
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::StorePunishment {
                id,
                key,
                affect,
                ptype,
                expiration,
                reason,
                punished_by,
            } => {
                warn_err(
                    punishments::Entity::insert(punishments::ActiveModel {
                        id: Set(id),
                        key: Set(key),
                        affect: Set(affect),
                        r#type: Set(ptype),
                        expiration: Set(expiration),
                        reason: Set(reason),
                        punished_by: Set(punished_by),
                    })
                    .on_conflict(
                        OnConflict::column(punishments::Column::Id)
                            .update_columns([
                                punishments::Column::Key,
                                punishments::Column::Affect,
                                punishments::Column::Type,
                                punishments::Column::Expiration,
                                punishments::Column::Reason,
                                punishments::Column::PunishedBy,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeletePunishment { id } => {
                warn_err(punishments::Entity::delete_by_id(id).exec(&db).await);
            }
            DbCommand::StorePetitionFeedback {
                char_name,
                gm_name,
                rate,
                message,
                date,
            } => {
                warn_err(
                    petition_feedback::Entity::insert(petition_feedback::ActiveModel {
                        char_name: Set(char_name),
                        gm_name: Set(gm_name),
                        rate: Set(rate),
                        message: Set(message),
                        date: Set(date),
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::StoreOfflineWarehouseItem {
                owner_id,
                object_id,
                item_id,
                count,
                enchant,
            } => {
                warn_err(
                    items::Entity::insert(items::ActiveModel {
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
                        OnConflict::column(items::Column::ObjectId)
                            .update_columns([
                                items::Column::OwnerId,
                                items::Column::ItemId,
                                items::Column::Count,
                                items::Column::EnchantLevel,
                                items::Column::Loc,
                                items::Column::LocData,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::StoreBufferScheme {
                object_id,
                scheme_name,
                skills,
            } => {
                warn_err(
                    buffer_schemes::Entity::insert(buffer_schemes::ActiveModel {
                        object_id: Set(object_id),
                        scheme_name: Set(scheme_name),
                        skills: Set(skills),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            buffer_schemes::Column::ObjectId,
                            buffer_schemes::Column::SchemeName,
                        ])
                        .update_column(buffer_schemes::Column::Skills)
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeleteBufferScheme {
                object_id,
                scheme_name,
            } => {
                warn_err(
                    buffer_schemes::Entity::delete_by_id((object_id, scheme_name))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreFavorite {
                fav_id,
                player_id,
                title,
                bypass,
                add_date,
            } => {
                warn_err(
                    bbs_favorites::Entity::insert(bbs_favorites::ActiveModel {
                        fav_id: Set(fav_id),
                        player_id: Set(player_id),
                        fav_title: Set(title),
                        fav_bypass: Set(bypass),
                        fav_add_date: Set(add_date),
                    })
                    .on_conflict(
                        OnConflict::column(bbs_favorites::Column::FavId)
                            .update_columns([
                                bbs_favorites::Column::PlayerId,
                                bbs_favorites::Column::FavTitle,
                                bbs_favorites::Column::FavBypass,
                                bbs_favorites::Column::FavAddDate,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeleteFavorite { player_id, fav_id } => {
                warn_err(
                    bbs_favorites::Entity::delete_many()
                        .filter(bbs_favorites::Column::PlayerId.eq(player_id))
                        .filter(bbs_favorites::Column::FavId.eq(fav_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::ResetRecommends => {
                // Java `DailyTaskManager.resetRecommends`: rec_left → 0 for
                // everyone; rec_have → 0 for those at/under 20, else -20.
                warn_err(
                    character_reco_bonus::Entity::update_many()
                        .col_expr(character_reco_bonus::Column::RecLeft, 0.into())
                        .col_expr(character_reco_bonus::Column::RecHave, 0.into())
                        .filter(character_reco_bonus::Column::RecHave.lte(20))
                        .exec(&db)
                        .await,
                );
                // `ExprTrait` is imported here rather than at module scope: it
                // adds `min`/`max`/`add` to *every* type, which shadows the
                // `Ord` ones everywhere else in this file.
                use models::sea_orm::sea_query::ExprTrait as _;
                warn_err(
                    character_reco_bonus::Entity::update_many()
                        .col_expr(character_reco_bonus::Column::RecLeft, 0.into())
                        .col_expr(
                            character_reco_bonus::Column::RecHave,
                            Expr::col(character_reco_bonus::Column::RecHave).sub(20),
                        )
                        .filter(character_reco_bonus::Column::RecHave.gt(20))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::ResetVitality { weekly } => {
                // Java `resetVitalityDaily`/`resetVitalityWeekly` — both the
                // `characters` and `character_subclasses` rows. `MAX/4` is added
                // uncapped (as Java does); the read-side clamp hides any overflow.
                const MAX: i32 = 140_000;
                // `ExprTrait` is imported here rather than at module scope: it
                // adds `min`/`max`/`add` to *every* type, which shadows the
                // `Ord` ones everywhere else in this file.
                use models::sea_orm::sea_query::ExprTrait as _;
                // Daily adds a quarter of the cap unless the pool is already
                // full; weekly refills it outright.
                fn refill<C: models::sea_orm::ColumnTrait>(col: C, weekly: bool) -> Expr {
                    if weekly {
                        Expr::value(MAX)
                    } else {
                        CaseStatement::new()
                            .case(Expr::col(col).eq(MAX), Expr::col(col))
                            .finally(Expr::col(col).add(MAX / 4))
                            .into()
                    }
                }
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(
                            characters::Column::VitalityPoints,
                            refill(characters::Column::VitalityPoints, weekly),
                        )
                        .exec(&db)
                        .await,
                );
                warn_err(
                    character_subclasses::Entity::update_many()
                        .col_expr(
                            character_subclasses::Column::VitalityPoints,
                            refill(character_subclasses::Column::VitalityPoints, weekly),
                        )
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::RepairCharacter { char_name } => {
                // Java `AdminRepairChar`, verbatim. Best-effort: each statement
                // is independent, keyed by name / resolved id.
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::X, (-84318).into())
                        .col_expr(characters::Column::Y, 244579.into())
                        .col_expr(characters::Column::Z, (-3730).into())
                        .filter(characters::Column::CharName.eq(&char_name))
                        .exec(&db)
                        .await,
                );
                let obj_id = characters::Entity::find()
                    .filter(characters::Column::CharName.eq(&char_name))
                    .one(&db)
                    .await
                    .ok()
                    .flatten()
                    .map(|c| c.char_id);
                if let Some(obj_id) = obj_id {
                    warn_err(
                        character_shortcuts::Entity::delete_many()
                            .filter(character_shortcuts::Column::CharId.eq(obj_id))
                            .exec(&db)
                            .await,
                    );
                    warn_err(
                        items::Entity::update_many()
                            .col_expr(items::Column::Loc, "INVENTORY".into())
                            .filter(items::Column::OwnerId.eq(obj_id))
                            .exec(&db)
                            .await,
                    );
                }
            }
            DbCommand::Shutdown => break,
        }
    }

    let _ = db.close().await;
    info!("DB thread: stopped.");
}
