use super::*;

pub(crate) async fn reload(
    db: &DatabaseConnection,
    event_tx: &EventTx,
    client_id: u32,
    account: String,
    send_list: bool,
) {
    let chars = load_characters(db, &account).await;
    let _ = event_tx.send(DbEvent::CharactersLoaded {
        client_id,
        account,
        chars,
        send_list,
    });
}

/// Best-effort read of one `account_gsdata` variable (Java
/// `AccountVariables.restoreMe`). Returns `None` on a missing row or any error
/// (e.g. the table absent in a minimal test schema), mirroring Java's
/// catch-and-default-empty behaviour.
async fn load_account_var(db: &DatabaseConnection, account: &str, var: &str) -> Option<String> {
    account_gsdata::Entity::find_by_id((account.to_string(), var.to_string()))
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|row| row.value)
}

/// Best-effort boot load of the whole `account_premium` table (Java
/// `PremiumManager` has no table-wide load; this port caches all rows so the
/// admin `//premium_*` commands work for offline accounts). Missing table → empty.
pub(crate) async fn load_premium(db: &DatabaseConnection) -> Vec<(String, i64)> {
    account_premium::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.account_name.to_lowercase(), row.enddate))
        .collect()
}

/// The most recent lottery round (Java `Lottery.SELECT_LAST_LOTTERY`). `None`
/// when the table is empty or unavailable.
pub(crate) async fn load_lottery(
    db: &DatabaseConnection,
) -> Option<crate::model::lottery::LotteryRow> {
    let row = lottery::Entity::find()
        .filter(lottery::Column::Id.eq(1))
        .order_by_desc(lottery::Column::Idnr)
        .one(db)
        .await
        .ok()
        .flatten()?;
    Some(crate::model::lottery::LotteryRow {
        idnr: row.idnr,
        prize: row.prize,
        newprize: row.newprize,
        enddate: row.enddate,
        finished: row.finished == 1,
    })
}

/// Every finished lottery round's draw result (Java re-queries per
/// `checkTicket`; loaded once at boot into the game-thread cache).
pub(crate) async fn load_lottery_draws(
    db: &DatabaseConnection,
) -> Vec<(i32, crate::model::lottery::DrawnRound)> {
    lottery::Entity::find()
        .filter(lottery::Column::Id.eq(1))
        .filter(lottery::Column::Finished.eq(1))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            (
                row.idnr,
                crate::model::lottery::DrawnRound {
                    number1: row.number1,
                    number2: row.number2,
                    prize1: row.prize1,
                    prize2: row.prize2,
                    prize3: row.prize3,
                },
            )
        })
        .collect()
}

/// Every Monster Race history record, oldest first (Java `MonsterRace
/// .loadHistory` — also fixes the current race number by the row count).
pub(crate) async fn load_mdt_history(
    db: &DatabaseConnection,
) -> Vec<crate::model::monster_race::HistoryInfo> {
    mdt_history::Entity::find()
        .order_by_asc(mdt_history::Column::RaceId)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| crate::model::monster_race::HistoryInfo {
            race_id: row.race_id,
            first: row.first.unwrap_or(0),
            second: row.second.unwrap_or(0),
            odd_rate: row.odd_rate.unwrap_or(0.0),
        })
        .collect()
}

/// The current lane bets (Java `MonsterRace.loadBets`): `(lane_id, bet)`.
pub(crate) async fn load_mdt_bets(db: &DatabaseConnection) -> Vec<(i32, i64)> {
    mdt_bets::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.lane_id, row.bet.unwrap_or(0)))
        .collect()
}

/// Every persisted item auction + its bids, plus the next auction id (Java
/// `ItemAuctionManager` boot load: `MAX(auctionId)+1` and each instance's
/// `loadAuction`). Empty on this dist.
/// Java `MailManager.load` + the `loc = 'MAIL'` item rows, in one pass.
/// Tolerates the tables being absent (a minimal test schema has neither).
pub(crate) async fn load_mail(
    db: &DatabaseConnection,
) -> (
    Vec<crate::model::mail::Message>,
    Vec<(i32, Vec<crate::character::ItemRow>)>,
) {
    use crate::model::mail::{MailType, Message};

    // The flag columns are enum('true','false') text; older rows may carry '1'.
    let truthy = |v: &str| v.eq_ignore_ascii_case("true") || v == "1";
    let messages = messages::Entity::find()
        .order_by_asc(messages::Column::Expiration)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| Message {
            id: r.message_id,
            sender_id: r.sender_id,
            receiver_id: r.receiver_id,
            subject: r.subject.unwrap_or_default(),
            content: r.content.unwrap_or_default(),
            expiration: r.expiration,
            req_adena: r.req_adena,
            has_attachments: truthy(&r.has_attachments),
            unread: truthy(&r.is_unread),
            deleted_by_sender: truthy(&r.is_deleted_by_sender),
            deleted_by_receiver: truthy(&r.is_deleted_by_receiver),
            mail_type: MailType::from_id(r.send_by_system),
            returned: truthy(&r.is_returned),
        })
        .collect();

    let mut by_message: std::collections::HashMap<i32, Vec<crate::character::ItemRow>> =
        std::collections::HashMap::new();
    for r in items::Entity::find()
        .filter(items::Column::Loc.eq("MAIL"))
        .all(db)
        .await
        .unwrap_or_default()
    {
        // Attachments hang off the message through `loc_data`.
        let message_id = r.loc_data.unwrap_or(0);
        by_message
            .entry(message_id)
            .or_default()
            .push(crate::character::ItemRow {
                object_id: r.object_id,
                item_id: r.item_id.unwrap_or(0),
                count: r.count,
                enchant_level: r.enchant_level.unwrap_or(0),
                loc: "MAIL".to_string(),
                loc_data: message_id,
                custom_type1: r.custom_type1.unwrap_or(0),
                custom_type2: r.custom_type2.unwrap_or(0),
                mana_left: r.mana_left,
                time: r.time as i32,
                augment_mineral: 0,
                augment_option1: 0,
                augment_option2: 0,
            });
    }
    (messages, by_message.into_iter().collect())
}

/// Java `CharInfoTable` — the offline character name -> id table. Mail is
/// addressed by name to characters who need not be online; nothing else in the
/// port needs this, so it is loaded once and maintained on creation/deletion.
pub(crate) async fn load_char_ids_by_name(db: &DatabaseConnection) -> Vec<(String, i32)> {
    characters::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.char_name.to_lowercase(), row.char_id))
        .collect()
}

pub(crate) async fn load_item_auctions(
    db: &DatabaseConnection,
) -> (i32, Vec<crate::model::item_auction::ItemAuction>) {
    use crate::model::item_auction::{AuctionState, ItemAuction, ItemAuctionBid};

    let mut auctions: Vec<ItemAuction> = item_auction::Entity::find()
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
    for bid in item_auction_bid::Entity::find()
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

/// `PunishmentManager.load` (G31): every active punishment, minus the rows that
/// have already expired (Java skips them, counting them as "expired"). Returns
/// `(next_id, rows)` — `next_id` seeds the game-thread id allocator. Fail-open
/// (empty) if the table is absent, like a minimal test schema.
/// Java `BotReportTable.loadReportedCharData` — every stored report row.
/// Fail-open (empty) if the table is absent, like the other boot loaders.
pub(crate) async fn load_bot_reports(db: &DatabaseConnection) -> Vec<(i32, i32, i64)> {
    bot_reported_char_data::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.bot_id, r.reporter_id, r.report_date))
        .collect()
}

pub(crate) async fn load_punishments(
    db: &DatabaseConnection,
) -> (i32, Vec<crate::model::punishment::Punishment>) {
    use crate::model::punishment::{Punishment, PunishmentAffect, PunishmentType};

    let now = commons::util::now_millis();
    let all = punishments::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    let rows: Vec<Punishment> = all
        .iter()
        .filter_map(|row| {
            let affect = PunishmentAffect::from_name(&row.affect)?;
            let ptype = PunishmentType::from_name(&row.r#type)?;
            // Java's `load` skips already-expired rows.
            if row.expiration > 0 && now > row.expiration {
                return None;
            }
            Some(Punishment {
                id: row.id,
                key: row.key.clone(),
                affect,
                ptype,
                expiration: row.expiration,
                reason: row.reason.clone(),
                punished_by: row.punished_by.clone(),
            })
        })
        .collect();

    // The id allocator must clear *every* persisted id, not just the still-active
    // ones — an expired row we filtered out above may still own the max id until
    // the operator purges it, and reusing that id would collide on INSERT.
    // `all` (not `rows`) on purpose: an expired row we filtered out still owns
    // its id.
    let loaded_max = all.iter().map(|row| row.id).max().unwrap_or(0);
    let next_id = (loaded_max + 1).max(1);
    (next_id, rows)
}

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

/// One `npc_respawns` row — a raid boss's persisted state.
#[derive(Debug, Clone, Copy)]
pub struct NpcRespawnRow {
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// Absolute unix millis the boss is due back, or 0 when it's alive (Java
    /// stores 0 for a living boss and the due time for a dead one).
    pub respawn_time: i64,
    pub cur_hp: f64,
    pub cur_mp: f64,
}

/// `RESTORE_CHAR_SUBCLASSES` — a character's subclass slots.
async fn load_subclasses(db: &DatabaseConnection, char_id: i32) -> Vec<crate::model::SubClass> {
    character_subclasses::Entity::find()
        .filter(character_subclasses::Column::CharId.eq(char_id))
        .order_by_asc(character_subclasses::Column::ClassIndex)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| crate::model::SubClass {
            class_id: row.class_id,
            class_index: row.class_index,
            level: row.level,
            exp: row.exp,
            sp: row.sp,
        })
        .collect()
}

/// Boot load of the whole `npc_respawns` table (Java `DBSpawnManager.load`).
/// Missing table → empty, like the other boot loads.
pub(crate) async fn load_npc_respawns(db: &DatabaseConnection) -> Vec<NpcRespawnRow> {
    npc_respawns::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| NpcRespawnRow {
            npc_id: row.id,
            x: row.x,
            y: row.y,
            z: row.z,
            heading: row.heading,
            respawn_time: row.respawn_time,
            cur_hp: row.current_hp,
            cur_mp: row.current_mp,
        })
        .collect()
}

/// Boot load of the whole `buffer_schemes` table (Java `SchemeBufferTable.load`).
/// `skills` is stored comma-joined; parse it here, drop empties. Availability
/// filtering (skills still in the buffer table) happens on the game thread,
/// where the datapack lives. Missing table → empty.
pub(crate) async fn load_buffer_schemes(db: &DatabaseConnection) -> Vec<(i32, String, Vec<i32>)> {
    buffer_schemes::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            let skills = row
                .skills
                .split(',')
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect();
            (row.object_id, row.scheme_name, skills)
        })
        .collect()
}

/// Boot load of the whole `bbs_favorites` table (Java `FavoriteBoard` loads it
/// per-player on `_bbsgetfav`; this port caches all rows at boot like the
/// buffer schemes). `ORDER BY favAddDate DESC` matches Java's list order.
/// Missing table → empty.
pub(crate) async fn load_favorites(
    db: &DatabaseConnection,
) -> Vec<(i32, i32, String, String, String)> {
    bbs_favorites::Entity::find()
        .order_by_desc(bbs_favorites::Column::FavAddDate)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            (
                row.player_id,
                row.fav_id,
                row.fav_title,
                row.fav_bypass,
                row.fav_add_date,
            )
        })
        .collect()
}

/// Java's `IdManager` hands out ids from a single pool shared by every
/// world-object type, so the next free id must clear the high-water mark of
/// every table that stores one — not just `characters` (a fresh id here that
/// collides with an existing `items.object_id` fails its INSERT silently).
pub(crate) async fn load_next_id(db: &DatabaseConnection) -> i64 {
    let max_char = characters::Entity::find()
        .select_only()
        .column_as(characters::Column::CharId.max(), "m")
        .into_tuple::<Option<i64>>()
        .one(db)
        .await
        .ok()
        .flatten()
        .flatten()
        .unwrap_or(0);
    let max_item = items::Entity::find()
        .select_only()
        .column_as(items::Column::ObjectId.max(), "m")
        .into_tuple::<Option<i64>>()
        .one(db)
        .await
        .ok()
        .flatten()
        .flatten()
        .unwrap_or(0);
    (max_char.max(max_item) + 1).max(FIRST_OID)
}

/// `loadCharacterSelectInfo`: rows for an account, expired deletions purged.
async fn load_characters(db: &DatabaseConnection, account: &str) -> Vec<CharData> {
    let rows = match characters::Entity::find()
        .filter(characters::Column::AccountName.eq(account))
        .order_by_asc(characters::Column::CreateDate)
        .all(db)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("DB thread: load_characters failed: {e}");
            return Vec::new();
        }
    };

    // Account-scoped prime (NCoin) balance — same for every char on the
    // account. Best-effort: absent table/row → 0 (Java `restoreMe` catch).
    let prime_points = load_account_var(db, account, "PRIME_POINTS")
        .await
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);

    let now = now_millis();
    let mut out = Vec::new();
    for (slot, row) in rows.iter().enumerate() {
        let delete_time = row.deletetime;
        let object_id = row.char_id;
        if delete_time > 0 && now > delete_time {
            delete_char(db, object_id).await; // restoreChar: purge expired
            continue;
        }
        out.push(char_data_of(db, row, slot as i32, prime_points).await);
    }
    // Characters marked for deletion are listed last in the lobby; the stable
    // sort keeps createDate order within each group. Slots are the list
    // positions the client will send back, so renumber after sorting.
    out.sort_by_key(|c| c.delete_time > 0);
    for (slot, c) in out.iter_mut().enumerate() {
        c.char_slot = slot as i32;
    }
    out
}

/// Everything hanging off one `characters` row, as the `CharData` the lobby and
/// the offline-shop restore both consume. Split out of `load_characters` so a
/// single character can be loaded by id without going through an account.
async fn char_data_of(
    db: &DatabaseConnection,
    row: &characters::Model,
    slot: i32,
    prime_points: i32,
) -> CharData {
    {
        let object_id = row.char_id;
        let delete_time = row.deletetime;
        let items = load_items(db, object_id).await;
        let skills_by_index = load_skills(db, object_id).await;
        let subclasses = load_subclasses(db, object_id).await;
        let class_id_now = row.classid.unwrap_or(0);
        // Java keeps the *active* class in `characters.classid`; the index is
        // whichever subclass slot carries it (0 when it's the base class).
        let active_index = subclasses
            .iter()
            .find(|s| s.class_id == class_id_now)
            .map(|s| s.class_index)
            .unwrap_or(0);
        let hennas_by_index = load_hennas(db, object_id).await;
        let recipe_book = load_recipe_book(db, object_id).await;
        let variables = load_variables(db, object_id).await;
        let pets = load_pets(db, object_id).await;
        let summons = load_summons(db, object_id).await;
        let shortcuts_by_index = load_shortcuts(db, object_id).await;
        let macros = load_macros(db, object_id).await;
        let friends = load_friends(db, object_id).await;
        let quests = load_quests(db, object_id).await;
        let skill_reuses = load_skill_reuses(db, object_id, active_index).await;
        let skill_buffs = load_skill_buffs(db, object_id, active_index).await;
        let (rec_have, rec_left) = load_reco_bonus(db, object_id).await;
        CharData {
            object_id,
            name: row.char_name.clone(),
            account_name: row.account_name.clone().unwrap_or_default(),
            level: row.level.unwrap_or(0),
            max_hp: row.max_hp.unwrap_or(0),
            cur_hp: row.cur_hp.map(f64::from).unwrap_or(0.0),
            max_mp: row.max_mp.unwrap_or(0),
            cur_mp: row.cur_mp.map(f64::from).unwrap_or(0.0),
            cur_cp: row.cur_cp.map(f64::from).unwrap_or(0.0),
            face: row.face.unwrap_or(0),
            hair_style: row.hair_style.unwrap_or(0),
            hair_color: row.hair_color.unwrap_or(0),
            sex: row.sex.unwrap_or(0),
            x: row.x.unwrap_or(0),
            y: row.y.unwrap_or(0),
            z: row.z.unwrap_or(0),
            exp: row.exp.unwrap_or(0),
            sp: row.sp,
            reputation: row.reputation.unwrap_or(0),
            pk_kills: row.pkkills.unwrap_or(0),
            raidboss_points: row.raidboss_points,
            pvp_kills: row.pvpkills.unwrap_or(0),
            rec_have,
            rec_left,
            clan_id: row.clanid.unwrap_or(0),
            clan_privs: row.clan_privs.unwrap_or(0),
            clan_create_expiry_time: row.clan_create_expiry_time,
            clan_join_expiry_time: row.clan_join_expiry_time,
            create_date: row.create_date.clone(),
            power_grade: row.power_grade.unwrap_or(0),
            pledge_type: row.subpledge,
            lvl_joined_academy: row.lvl_joined_academy,
            apprentice: row.apprentice,
            sponsor: row.sponsor,
            race: row.race.unwrap_or(0),
            class_id: class_id_now,
            base_class_id: row.base_class,
            delete_time,
            last_access: row.last_access,
            vitality_points: row.vitality_points,
            pccafe_points: row.pccafe_points,
            prime_points,
            access_level: row.accesslevel.unwrap_or(0),
            noble: row.nobless == 1,
            subclasses,
            char_slot: slot,
            items,
            // The active class index is whichever subclass row matches the
            // `characters.classid` we just loaded; base class → 0.
            skills: skills_by_index
                .get(&active_index)
                .cloned()
                .unwrap_or_default(),
            skills_by_index,
            hennas: hennas_by_index
                .get(&active_index)
                .cloned()
                .unwrap_or_default(),
            hennas_by_index,
            recipe_book,
            variables,
            pets,
            summons,
            shortcuts: shortcuts_by_index
                .get(&active_index)
                .cloned()
                .unwrap_or_default(),
            shortcuts_by_index,
            macros,
            friends,
            quests,
            skill_reuses,
            skill_buffs,
        }
    }
}

/// `LOAD_OFFLINE_STATUS` + `LOAD_OFFLINE_ITEMS`, joined per trader. A row whose
/// character no longer exists is dropped (Java's `Player.load` returning null
/// lands in its catch block).
pub(crate) async fn load_offline_traders(db: &DatabaseConnection) -> Vec<OfflineTraderRow> {
    let rows = character_offline_trade::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    let mut out = Vec::new();
    for row in rows {
        let items = character_offline_trade_items::Entity::find()
            .filter(character_offline_trade_items::Column::CharId.eq(row.char_id))
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

/// One character by id, with every child collection — the offline-shop restore
/// needs a full `CharData` for a character it reaches through
/// `character_offline_trade`, not through an account's list.
async fn load_character(db: &DatabaseConnection, char_id: i32) -> Option<CharData> {
    let row = characters::Entity::find_by_id(char_id)
        .one(db)
        .await
        .ok()??;
    let prime_points = match row.account_name.as_deref() {
        Some(account) => load_account_var(db, account, "PRIME_POINTS")
            .await
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0),
        None => 0,
    };
    Some(char_data_of(db, &row, 0, prime_points).await)
}

/// A character's `character_skills` rows (Java: `Player.restoreSkills`,
/// called for every row shown in `CharSelectionInfo` — same treatment as
/// `load_items`).
async fn load_skills(
    db: &DatabaseConnection,
    owner_id: i32,
) -> std::collections::HashMap<i32, Vec<(i32, i32, i32)>> {
    let rows = character_skills::Entity::find()
        .filter(character_skills::Column::CharId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default();
    let mut out: std::collections::HashMap<i32, Vec<(i32, i32, i32)>> =
        std::collections::HashMap::new();
    for row in rows {
        out.entry(row.class_index).or_default().push((
            row.skill_id,
            row.skill_level,
            row.skill_sub_level,
        ));
    }
    out
}

/// A character's `character_hennas` rows (Java `Player.restoreHenna`) as
/// `(slot, symbol_id)`. `class_index = 0` — no subclasses on this dist.
async fn load_hennas(
    db: &DatabaseConnection,
    owner_id: i32,
) -> std::collections::HashMap<i32, Vec<(i32, i32)>> {
    let rows = character_hennas::Entity::find()
        .filter(character_hennas::Column::CharId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default();
    let mut out: std::collections::HashMap<i32, Vec<(i32, i32)>> = std::collections::HashMap::new();
    for row in rows {
        let (slot, sym) = (row.slot, row.symbol_id.unwrap_or(0));
        if (1..=3).contains(&slot) && sym != 0 {
            out.entry(row.class_index).or_default().push((slot, sym));
        }
    }
    out
}

/// A character's `character_recipebook` rows (Java `Player.restoreRecipeBook`)
/// as recipe-*list* ids. The dwarven/common split (the `type` column) is
/// re-derived from `RecipeData` on the game thread, so the DB layer just
/// returns the ids. `classIndex = 0` — no subclasses on this dist.
async fn load_recipe_book(db: &DatabaseConnection, owner_id: i32) -> Vec<i32> {
    character_recipebook::Entity::find()
        .filter(character_recipebook::Column::CharId.eq(owner_id))
        .filter(character_recipebook::Column::ClassIndex.eq(0))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| row.id as i32)
        .collect()
}

/// A character's `character_variables` rows (Java `PlayerVariables.restoreMe`)
/// as `(var, val)` pairs. Values stay strings — the component parses on read,
/// like Java's `StatSet` getters.
async fn load_variables(db: &DatabaseConnection, owner_id: i32) -> Vec<(String, String)> {
    character_variables::Entity::find()
        .filter(character_variables::Column::CharId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.var, row.val))
        .collect()
}

/// Every pet this character owns (Java `Pet.restore`, hoisted from per-summon
/// to per-login — see `PlayerPets`). Java reads one row by collar object id at
/// summon time; loading the whole set here keeps the summon path off the DB
/// thread and costs one extra query per login.
/// The servitor this character had out at logout, if any (Java
/// `CharSummonTable.LOAD_SUMMON`).
async fn load_summons(db: &DatabaseConnection, owner_id: i32) -> Vec<SummonRow> {
    let rows = character_summons::Entity::find()
        .filter(character_summons::Column::OwnerId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default();
    let mut out = Vec::new();
    for row in rows {
        out.push(SummonRow {
            summon_skill_id: row.summon_skill_id,
            cur_hp: row.cur_hp.unwrap_or(0),
            cur_mp: row.cur_mp.unwrap_or(0),
            remaining_secs: row.time,
            buffs: load_summon_buffs(db, owner_id, row.summon_skill_id).await,
        });
    }
    out
}

/// A servitor's saved buffs (Java `Servitor.RESTORE_SKILL_SAVE`), ordered by
/// `buff_index` so they come back in the order they were applied — which
/// matters for the buff-slot cap.
async fn load_summon_buffs(
    db: &DatabaseConnection,
    owner_id: i32,
    summon_skill_id: i32,
) -> Vec<SkillBuffRow> {
    character_summon_skills_save::Entity::find()
        .filter(character_summon_skills_save::Column::OwnerId.eq(owner_id))
        .filter(character_summon_skills_save::Column::OwnerClassIndex.eq(0))
        .filter(character_summon_skills_save::Column::SummonSkillId.eq(summon_skill_id))
        .order_by_asc(character_summon_skills_save::Column::BuffIndex)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| SkillBuffRow {
            skill_id: row.skill_id,
            skill_level: row.skill_level,
            remaining_time_secs: row.remaining_time,
        })
        .collect()
}

async fn load_pets(db: &DatabaseConnection, owner_id: i32) -> Vec<PetRow> {
    pets::Entity::find()
        .filter(pets::Column::OwnerId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| PetRow {
            collar_object_id: row.item_obj_id,
            name: row.name.unwrap_or_default(),
            level: row.level,
            cur_hp: row.cur_hp.map(f64::from).unwrap_or(0.0),
            cur_mp: row.cur_mp.map(f64::from).unwrap_or(0.0),
            exp: row.exp.unwrap_or(0),
            sp: row.sp.unwrap_or(0),
            fed: row.fed.unwrap_or(0),
            restore: row.restore == "true",
        })
        .collect()
}

/// A character's `character_skills_save` reuse rows for the **active** class
/// index (Java `restoreEffects`, `restore_type = 1` half). Already-expired rows (`systime <= now`) are
/// dropped here; the survivors carry the absolute `systime` and the game side
/// converts it to a game tick when the character enters the world. Buff rows
/// (restore_type 0) are loaded separately by [`load_skill_buffs`].
async fn load_skill_reuses(
    db: &DatabaseConnection,
    owner_id: i32,
    class_index: i32,
) -> Vec<SkillReuseRow> {
    let now = now_millis();
    character_skills_save::Entity::find()
        .filter(character_skills_save::Column::CharId.eq(owner_id))
        .filter(character_skills_save::Column::ClassIndex.eq(class_index))
        .filter(character_skills_save::Column::RestoreType.eq(1))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|row| {
            (row.systime > now).then_some(SkillReuseRow {
                reuse_key: row.skill_id,
                skill_level: row.skill_level,
                reuse_delay: row.reuse_delay,
                systime_ms: row.systime,
            })
        })
        .collect()
}

/// A character's `character_skills_save` **buff** rows for the **active** class
/// index (Java `restoreEffects`, `restore_type = 0` half), in `buff_index`
/// order so the buff bar comes back in the order it was stored.
///
/// No expiry filtering happens here, unlike [`load_skill_reuses`]: a buff's
/// `remaining_time` is relative and its countdown is frozen while the character
/// is offline, so there is no elapsed time to compare against. Rows with a
/// non-positive remaining time are dropped since they'd restore an
/// already-dead buff.
async fn load_skill_buffs(
    db: &DatabaseConnection,
    owner_id: i32,
    class_index: i32,
) -> Vec<SkillBuffRow> {
    character_skills_save::Entity::find()
        .filter(character_skills_save::Column::CharId.eq(owner_id))
        .filter(character_skills_save::Column::ClassIndex.eq(class_index))
        .filter(character_skills_save::Column::RestoreType.eq(0))
        .order_by_asc(character_skills_save::Column::BuffIndex)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|row| {
            (row.remaining_time > 0).then_some(SkillBuffRow {
                skill_id: row.skill_id,
                skill_level: row.skill_level,
                remaining_time_secs: row.remaining_time,
            })
        })
        .collect()
}

/// A character's recommendation counters (Java `Player.loadRecommendations`).
/// Returns `(rec_have, rec_left)`; `(0, 0)` when the row is absent, matching
/// Java's field defaults for a character whose `character_reco_bonus` row
/// hasn't been written yet.
async fn load_reco_bonus(db: &DatabaseConnection, owner_id: i32) -> (i32, i32) {
    match character_reco_bonus::Entity::find_by_id(owner_id)
        .one(db)
        .await
    {
        Ok(Some(row)) => (row.rec_have, row.rec_left),
        _ => (0, 0),
    }
}

/// A character's `character_shortcuts` rows (Java `ShortCuts.restoreMe` —
/// the inventory verification half runs on the game thread, in
/// `Player::from_char`). `characterType` isn't stored; restore hardcodes 1
/// like Java. `shared_reuse_group` starts at the -1 default; `from_char`
/// fills it for EtcItem shortcuts.
async fn load_shortcuts(
    db: &DatabaseConnection,
    owner_id: i32,
) -> std::collections::HashMap<i32, Vec<crate::model::shortcut::Shortcut>> {
    let rows = character_shortcuts::Entity::find()
        .filter(character_shortcuts::Column::CharId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default();
    let mut out: std::collections::HashMap<i32, Vec<crate::model::shortcut::Shortcut>> =
        std::collections::HashMap::new();
    for row in rows {
        out.entry(row.class_index)
            .or_default()
            .push(crate::model::shortcut::Shortcut {
                slot: row.slot,
                page: row.page,
                kind: crate::model::shortcut::ShortcutType::from_ordinal(row.r#type.unwrap_or(0)),
                id: row.shortcut_id.unwrap_or(0) as i32,
                level: row.level.unwrap_or(0),
                character_type: 1,
                shared_reuse_group: -1,
            });
    }
    out
}

/// A character's `character_friends` rows joined with each friend's
/// character row — the name/level/class snapshot Java reads through
/// `CharInfoTable` on demand (`relation`/`memo` unused).
async fn load_friends(db: &DatabaseConnection, owner_id: i32) -> Vec<crate::character::FriendInfo> {
    // The join is two reads instead of one: `character_friends` declares no
    // foreign key, so there is no relation to traverse — and a friend list is a
    // handful of rows.
    let ids: Vec<i32> = character_friends::Entity::find()
        .filter(character_friends::Column::CharId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| row.friend_id)
        .collect();
    if ids.is_empty() {
        return Vec::new();
    }
    characters::Entity::find()
        .filter(characters::Column::CharId.is_in(ids))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| crate::character::FriendInfo {
            char_id: row.char_id,
            name: row.char_name,
            level: row.level.unwrap_or(0),
            class_id: row.classid.unwrap_or(0),
        })
        .collect()
}

/// A character's `character_quests` rows grouped by quest name (Java
/// `Quest.playerEnter`): the `<state>` rows define which quests exist, the
/// remaining rows fill each one's variable map. Vars for a quest without a
/// state row are orphans — Java warns (or deletes with
/// `AUTODELETE_INVALID_QUEST_DATA`); we drop them from the load.
async fn load_quests(
    db: &DatabaseConnection,
    owner_id: i32,
) -> std::collections::HashMap<String, crate::model::quest::QuestState> {
    use crate::model::quest::{QuestState, STATE_VAR, state};
    let rows = character_quests::Entity::find()
        .filter(character_quests::Column::CharId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default();
    let mut out: std::collections::HashMap<String, QuestState> = std::collections::HashMap::new();
    for row in rows.iter().filter(|r| r.var == STATE_VAR) {
        out.insert(
            row.name.clone(),
            QuestState {
                state: state::from_name(row.value.as_deref().unwrap_or_default()),
                ..Default::default()
            },
        );
    }
    for row in rows.iter().filter(|r| r.var != STATE_VAR) {
        if let Some(qs) = out.get_mut(&row.name) {
            qs.vars
                .insert(row.var.clone(), row.value.clone().unwrap_or_default());
        }
    }
    out
}

/// `GrandBossManager.init`: every `grandboss_data` row. The NPC-template
/// filter (`NpcData.getTemplate != null`) runs on the game thread, which owns
/// the datapack; here we just read the table.
/// `Olympiad.load` — the single `olympiad_data` row (defaults if absent: cycle
/// 1, period 0) plus every `olympiad_nobles` record.
pub(crate) async fn load_olympiad(db: &DatabaseConnection) -> DbEvent {
    let data = olympiad_data::Entity::find()
        .filter(olympiad_data::Column::Id.eq(0))
        .one(db)
        .await
        .ok()
        .flatten();
    let (current_cycle, period, olympiad_end, validation_end, next_weekly_change) = match &data {
        Some(r) => (
            r.current_cycle,
            r.period,
            r.olympiad_end,
            r.validation_end,
            r.next_weekly_change,
        ),
        // Java's defaults for a database with no olympiad row yet.
        None => (1, 0, 0, 0, 0),
    };
    let nobles = olympiad_nobles::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| OlympiadNobleRow {
            char_id: r.char_id,
            class_id: r.class_id,
            points: r.olympiad_points,
            comp_done: r.competitions_done,
            comp_won: r.competitions_won,
            comp_lost: r.competitions_lost,
            comp_drawn: r.competitions_drawn,
            comp_done_week: r.competitions_done_week,
        })
        .collect();
    DbEvent::OlympiadLoaded {
        current_cycle,
        period,
        olympiad_end,
        validation_end,
        next_weekly_change,
        nobles,
        eom: load_olympiad_eom(db).await,
    }
}

/// `Olympiad.getClassLeaderBoard`'s source table — the previous cycle's
/// snapshot, joined to `characters` for the display names exactly as Java's
/// `GET_EACH_CLASS_LEADER` does. Ranking happens in memory at read time.
async fn load_olympiad_eom(db: &DatabaseConnection) -> Vec<OlympiadEomRow> {
    let rows = olympiad_nobles_eom::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    if rows.is_empty() {
        return Vec::new();
    }
    let chars = characters::Entity::find()
        .filter(characters::Column::CharId.is_in(rows.iter().map(|r| r.char_id)))
        .all(db)
        .await
        .unwrap_or_default();
    rows.into_iter()
        .map(|r| OlympiadEomRow {
            class_id: r.class_id,
            // Java's join drops a row whose character is gone; an empty name is
            // the same thing one step later, and keeps the count honest.
            name: chars
                .iter()
                .find(|c| c.char_id == r.char_id)
                .map(|c| c.char_name.clone())
                .unwrap_or_default(),
            points: r.olympiad_points,
            comp_done: r.competitions_done,
            comp_won: r.competitions_won,
        })
        .collect()
}

/// `Hero.init` — the currently-crowned heroes (`heroes` rows with `played = 1`).
pub(crate) async fn load_heroes(db: &DatabaseConnection) -> Vec<HeroRow> {
    // The name/clan half of the row lives on `characters`; Java reads it
    // through `CharInfoTable` for the same reason there is no FK to follow.
    let heroes = heroes::Entity::find()
        .filter(heroes::Column::Played.eq(1))
        .all(db)
        .await
        .unwrap_or_default();
    if heroes.is_empty() {
        return Vec::new();
    }
    let chars = characters::Entity::find()
        .filter(characters::Column::CharId.is_in(heroes.iter().map(|h| h.char_id)))
        .all(db)
        .await
        .unwrap_or_default();
    heroes
        .into_iter()
        .map(|h| {
            let c = chars.iter().find(|c| c.char_id == h.char_id);
            HeroRow {
                char_id: h.char_id,
                class_id: h.class_id,
                count: h.count,
                name: c.map(|c| c.char_name.clone()).unwrap_or_default(),
                clan_id: c.and_then(|c| c.clanid).unwrap_or(0),
                message: h.message,
                // Java `Boolean.parseBoolean(rset.getString(CLAIMED))` — anything
                // but "true" reads false.
                claimed: h.claimed == "true",
            }
        })
        .collect()
}

/// Every hero-diary entry (Java `Hero.loadDiary` per hero, batched here into one
/// query), oldest first: `(charId, time, action, param)`.
pub(crate) async fn load_hero_diary(db: &DatabaseConnection) -> Vec<(i32, i64, i8, i32)> {
    heroes_diary::Entity::find()
        .order_by_asc(heroes_diary::Column::Time)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.char_id, r.time, r.action as i8, r.param))
        .collect()
}

pub(crate) async fn load_grandboss_data(
    db: &DatabaseConnection,
) -> Vec<crate::model::grand_boss::GrandBoss> {
    grandboss_data::Entity::find()
        .order_by_asc(grandboss_data::Column::BossId)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| crate::model::grand_boss::GrandBoss {
            boss_id: r.boss_id,
            loc_x: r.loc_x,
            loc_y: r.loc_y,
            loc_z: r.loc_z,
            heading: r.heading,
            respawn_time: r.respawn_time,
            current_hp: r.current_hp,
            current_mp: r.current_mp,
            status: r.status,
        })
        .collect()
}

/// `CursedWeaponsManager.restore`: every `cursed_weapons` state row.
pub(crate) async fn load_cursed_weapons(db: &DatabaseConnection) -> Vec<CursedWeaponRow> {
    cursed_weapons::Entity::find()
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

/// `ClanTable`'s boot restore: every `clan_data` row + its member roster
/// from `characters WHERE clanid=?` (Java `Clan.restore`).
/// The stationed siege guards (`castle_siege_guards WHERE isHired=0`) — the
/// non-mercenary garrison spawned at siege start.
pub(crate) async fn load_siege_guards(
    db: &DatabaseConnection,
) -> Vec<(i32, crate::model::siege::SiegeSpawn)> {
    castle_siege_guards::Entity::find()
        .filter(castle_siege_guards::Column::IsHired.eq(0))
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
    siege_clans::Entity::find()
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
    castle_manor_production::Entity::find()
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
    castle_manor_procure::Entity::find()
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
    clanhall::Entity::find()
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
    clanhall_auctions_bidders::Entity::find()
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
    residence_functions::Entity::find()
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

/// `CastleManager.load`: every `castle` row (id/name/side).
/// `GlobalVariablesManager.restoreMe` — the whole `global_variables` table.
pub(crate) async fn load_global_variables(db: &DatabaseConnection) -> Vec<(String, String)> {
    global_variables::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.var, r.value.unwrap_or_default()))
        .collect()
}

pub(crate) async fn load_castles(db: &DatabaseConnection) -> Vec<crate::model::castle::Castle> {
    castle::Entity::find()
        .order_by_asc(castle::Column::Id)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| crate::model::castle::Castle {
            id: r.id,
            name: r.name,
            side: crate::model::castle::CastleSide::from_str(&r.side).unwrap_or_default(),
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

pub(crate) async fn load_clans(db: &DatabaseConnection) -> Vec<crate::model::clan::Clan> {
    let clan_rows = clan_data::Entity::find().all(db).await.unwrap_or_default();
    let mut out = Vec::with_capacity(clan_rows.len());
    for row in &clan_rows {
        let clan_id = row.clan_id;
        let member_rows = characters::Entity::find()
            .filter(characters::Column::Clanid.eq(clan_id))
            .all(db)
            .await
            .unwrap_or_default();
        // Clan warehouse contents (`owner_id = clan_id`, `loc = "CLANWH"`).
        let wh_rows = load_items(db, clan_id).await;
        // Clan skills (Java `Clan.restoreSkills`) — the main-pledge set
        // (`sub_pledge_id = -2`); sub-unit skills aren't modelled, so other
        // sub_pledge ids are ignored.
        let skills = clan_skills::Entity::find()
            .filter(clan_skills::Column::ClanId.eq(clan_id))
            .filter(clan_skills::Column::SubPledgeId.is_in([-2, 0]))
            .all(db)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|s| (s.skill_id, s.skill_level))
            .collect();
        // Rank → privilege-mask rows (Java `restoreRankPrivs`; rank -1 skipped).
        let rank_privs = clan_privs::Entity::find()
            .filter(clan_privs::Column::ClanId.eq(clan_id))
            .all(db)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.rank, r.privs))
            .filter(|&(rank, _)| rank != -1)
            .collect();
        // Sub-pledges (Java `Clan.restoreSubPledges`).
        let sub_pledges: std::collections::HashMap<i32, crate::model::clan::SubPledge> =
            clan_subpledges::Entity::find()
                .filter(clan_subpledges::Column::ClanId.eq(clan_id))
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
                })
                .collect(),
        });
    }
    out
}

/// A character's `character_macroses` rows (Java `MacroList.restoreMe`),
/// commands decoded from the `type,d1,d2[,cmd];…` column encoding.
async fn load_macros(db: &DatabaseConnection, owner_id: i32) -> Vec<crate::model::shortcut::Macro> {
    character_macroses::Entity::find()
        .filter(character_macroses::Column::CharId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| crate::model::shortcut::Macro {
            id: r.id,
            icon: r.icon.unwrap_or(0),
            name: r.name.unwrap_or_default(),
            descr: r.descr.unwrap_or_default(),
            acronym: r.acronym.unwrap_or_default(),
            commands: crate::model::shortcut::decode_commands(
                r.commands.as_deref().unwrap_or_default(),
            ),
        })
        .collect()
}

async fn upsert_shortcut(
    db: &DatabaseConnection,
    char_id: i32,
    slot: i32,
    page: i32,
    kind: i32,
    shortcut_id: i32,
    level: i32,
) {
    let row = character_shortcuts::ActiveModel {
        char_id: Set(char_id),
        slot: Set(slot),
        page: Set(page),
        r#type: Set(Some(kind)),
        shortcut_id: Set(Some(shortcut_id.into())),
        level: Set(Some(level)),
        sub_level: Set(0),
        class_index: Set(0),
    };
    let res = character_shortcuts::Entity::insert(row)
        .on_conflict(
            OnConflict::columns([
                character_shortcuts::Column::CharId,
                character_shortcuts::Column::Slot,
                character_shortcuts::Column::Page,
                character_shortcuts::Column::ClassIndex,
            ])
            .update_columns([
                character_shortcuts::Column::Type,
                character_shortcuts::Column::ShortcutId,
                character_shortcuts::Column::Level,
            ])
            .to_owned(),
        )
        .exec(db)
        .await;
    if let Err(e) = res {
        warn!("DB thread: upsert_shortcut failed: {e}");
    }
}

async fn upsert_macro(db: &DatabaseConnection, char_id: i32, m: &crate::model::shortcut::Macro) {
    let row = character_macroses::ActiveModel {
        char_id: Set(char_id),
        id: Set(m.id),
        icon: Set(Some(m.icon)),
        name: Set(Some(m.name.clone())),
        descr: Set(Some(m.descr.clone())),
        acronym: Set(Some(m.acronym.clone())),
        commands: Set(Some(crate::model::shortcut::encode_commands(&m.commands))),
    };
    let res = character_macroses::Entity::insert(row)
        .on_conflict(
            OnConflict::columns([
                character_macroses::Column::CharId,
                character_macroses::Column::Id,
            ])
            .update_columns([
                character_macroses::Column::Icon,
                character_macroses::Column::Name,
                character_macroses::Column::Descr,
                character_macroses::Column::Acronym,
                character_macroses::Column::Commands,
            ])
            .to_owned(),
        )
        .exec(db)
        .await;
    if let Err(e) = res {
        warn!("DB thread: upsert_macro failed: {e}");
    }
}

/// A character's `items` rows (Java: `PlayerInventory.restore`, called for
/// every row shown in `CharSelectionInfo`, not just the entered character).
async fn load_items(db: &DatabaseConnection, owner_id: i32) -> Vec<ItemRow> {
    // Java `PlayerInventory.restore` orders by `loc_data` so a client's saved
    // inventory arrangement (`RequestSaveInventoryOrder`) survives relog.
    let rows = items::Entity::find()
        .filter(items::Column::OwnerId.eq(owner_id))
        .order_by_asc(items::Column::LocData)
        .all(db)
        .await
        .unwrap_or_default();
    // Augmentations (Java `Item.restoreAttributes`): object_id → (mineral, o1, o2).
    let variations: std::collections::HashMap<i32, (i32, i32, i32)> =
        item_variations::Entity::find()
            .filter(item_variations::Column::ItemId.is_in(rows.iter().map(|r| r.object_id)))
            .all(db)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.item_id, (r.mineral_id, r.option1, r.option2)))
            .collect();
    rows.into_iter()
        .map(|r| {
            let (augment_mineral, augment_option1, augment_option2) =
                variations.get(&r.object_id).copied().unwrap_or((0, 0, 0));
            ItemRow {
                object_id: r.object_id,
                item_id: r.item_id.unwrap_or(0),
                count: r.count,
                enchant_level: r.enchant_level.unwrap_or(0),
                loc: r.loc.unwrap_or_default(),
                loc_data: r.loc_data.unwrap_or(0),
                custom_type1: r.custom_type1.unwrap_or(0),
                custom_type2: r.custom_type2.unwrap_or(0),
                mana_left: r.mana_left,
                time: r.time as i32,
                augment_mineral,
                augment_option1,
                augment_option2,
            }
        })
        .collect()
}

/// Case-insensitive character-name existence check (`getIdByName`).
pub(crate) async fn name_exists(db: &DatabaseConnection, name: &str) -> bool {
    // `COLLATE NOCASE` is the point of this query — two characters may not
    // differ only by case — and sea-query cannot attach a collation, so the
    // comparison stays a bound custom expression.
    characters::Entity::find()
        .filter(models::sea_orm::sea_query::Expr::cust_with_values(
            "char_name = ? COLLATE NOCASE",
            [name],
        ))
        .count(db)
        .await
        .unwrap_or(0)
        > 0
}

/// `characters.createDate` is a `date` column SQLite fills with `date('now')`;
/// the entity carries it as text, so the value is formatted here.
fn today() -> String {
    commons::util::format_date(commons::util::now_millis())
}

/// Runs an insert that the caller treats as best-effort, logging a failure the
/// way the old `exec` helper did.
async fn insert_or_warn<A: models::sea_orm::ActiveModelTrait>(
    db: &DatabaseConnection,
    insert: models::sea_orm::Insert<A>,
) {
    if let Err(e) = insert.exec(db).await {
        warn!("DB thread: insert failed: {e}");
    }
}

/// Logs a failed fire-and-forget write, the way the old `exec` helper did.
///
/// The DB thread must not stop for one bad statement: the game thread has
/// already applied the change in memory and is not waiting for a reply.
pub(crate) fn warn_err<T>(res: Result<T, DbErr>) {
    if let Err(e) = res {
        warn!("DB thread: query failed: {e}");
    }
}

pub(crate) async fn create_character(
    db: &DatabaseConnection,
    next_id: &mut i64,
    max_characters: i32,
    data: &NewCharacter,
) -> CreateResult {
    if name_exists(db, &data.name).await {
        return CreateResult::NameExists;
    }
    let count = characters::Entity::find()
        .filter(characters::Column::AccountName.eq(&data.account))
        .count(db)
        .await
        .unwrap_or(0) as i64;
    if max_characters > 0 && count >= max_characters as i64 {
        return CreateResult::TooMany;
    }

    let char_id = *next_id;
    *next_id += 1;
    // Columns the template does not set keep their DDL defaults, exactly as the
    // old INSERT's column list did. `createDate` is SQLite's `date('now')`.
    let row = characters::ActiveModel {
        account_name: Set(Some(data.account.clone())),
        char_id: Set(char_id as i32),
        char_name: Set(data.name.clone()),
        level: Set(Some(1)),
        max_hp: Set(Some(data.max_hp)),
        cur_hp: Set(Some(f64::from(data.max_hp).into())),
        max_cp: Set(Some(0)),
        cur_cp: Set(Some(0.0.into())),
        max_mp: Set(Some(data.max_mp)),
        cur_mp: Set(Some(f64::from(data.max_mp).into())),
        face: Set(Some(data.face)),
        hair_style: Set(Some(data.hair_style)),
        hair_color: Set(Some(data.hair_color)),
        sex: Set(Some(data.sex)),
        heading: Set(Some(0)),
        x: Set(Some(data.x)),
        y: Set(Some(data.y)),
        z: Set(Some(data.z)),
        exp: Set(Some(0)),
        sp: Set(0),
        reputation: Set(Some(0)),
        race: Set(Some(data.race)),
        classid: Set(Some(data.class_id)),
        base_class: Set(data.class_id),
        deletetime: Set(0),
        title: Set(Some(String::new())),
        accesslevel: Set(Some(0)),
        online: Set(Some(0)),
        char_slot: Set(Some(count as i32)),
        last_access: Set(now_millis()),
        create_date: Set(today()),
        vitality_points: Set(data.vitality_points),
        ..Default::default()
    };
    if let Err(e) = characters::Entity::insert(row).exec(db).await {
        error!("DB thread: character insert failed: {e}");
        return CreateResult::Fail;
    }

    // Seed the recommendation row: Java `Player.create` grants rec_left=20,
    // persisted to `character_reco_bonus` when the freshly-created character
    // disconnects back to the lobby.
    insert_or_warn(
        db,
        character_reco_bonus::Entity::insert(character_reco_bonus::ActiveModel {
            char_id: Set(char_id as i32),
            rec_have: Set(0),
            rec_left: Set(20),
            time_left: Set(0),
        }),
    )
    .await;

    // Initial skills (character_skills).
    for (skill_id, skill_level) in &data.skills {
        insert_or_warn(
            db,
            character_skills::Entity::insert(character_skills::ActiveModel {
                char_id: Set(char_id as i32),
                skill_id: Set(*skill_id),
                skill_level: Set(*skill_level),
                skill_sub_level: Set(0),
                class_index: Set(0),
            }),
        )
        .await;
    }

    // Initial equipment + starting adena. The item_id → object_id map feeds
    // ITEM shortcut resolution below (first occurrence wins, like Java
    // `getItemByItemId`).
    let mut item_object_ids: std::collections::HashMap<i32, i64> = std::collections::HashMap::new();
    for item in &data.items {
        let item_object_id = *next_id;
        *next_id += 1;
        item_object_ids
            .entry(item.item_id)
            .or_insert(item_object_id);
        let (loc, loc_data) = match item.paperdoll_index {
            Some(slot) => ("PAPERDOLL", slot as i32),
            None => ("INVENTORY", 0),
        };
        insert_or_warn(
            db,
            items::Entity::insert(items::ActiveModel {
                owner_id: Set(Some(char_id as i32)),
                object_id: Set(item_object_id as i32),
                item_id: Set(Some(item.item_id)),
                count: Set(item.count),
                enchant_level: Set(Some(0)),
                loc: Set(Some(loc.to_string())),
                loc_data: Set(Some(loc_data)),
                custom_type1: Set(Some(0)),
                custom_type2: Set(Some(0)),
                mana_left: Set(-1),
                time: Set(0),
                ..Default::default()
            }),
        )
        .await;
    }

    // Initial shortcuts + macro presets (`InitialShortcutData.
    // registerAllShortcuts` — persistence only; there's no in-world session to
    // echo packets to at creation).
    for sc in &data.shortcuts {
        let shortcut_id = if sc.kind == crate::model::shortcut::ShortcutType::Item {
            // ITEM entries reference an item id; skip ones the new character
            // didn't actually receive (Java `continue`s).
            match item_object_ids.get(&sc.id) {
                Some(&object_id) => object_id as i32,
                None => continue,
            }
        } else {
            sc.id
        };
        upsert_shortcut(
            db,
            char_id as i32,
            sc.slot,
            sc.page,
            sc.kind.ordinal(),
            shortcut_id,
            sc.level,
        )
        .await;
    }
    for m in &data.macros {
        upsert_macro(db, char_id as i32, m).await;
    }
    info!(
        "Created character '{}' ({}) for account {} with {} initial skill(s), {} item(s).",
        data.name,
        char_id,
        data.account,
        data.skills.len(),
        data.items.len()
    );
    CreateResult::Ok
}

/// Java `Player.storeCharBase` (narrowed to the tracked columns, see
/// [`PlayerSnapshot`]) + `updateOnlineStatus` — the character leaves the world,
/// so `online=0` and `lastAccess=now` in the same write.
/// Flush a whole player to the database in one transaction — the only path that
/// writes character-owned gameplay state (memory-first model). Reconciles the
/// `characters` row plus every child table (items, skills, shortcuts, macros,
/// quests) so a single flush captures pickups, drops, equips, skill changes,
/// shortcut/macro edits and quest progress made since the last flush. Child
/// tables are rewritten wholesale (delete-this-owner + re-insert), which is
/// atomic inside the transaction and doubles as the delete path — anything no
/// longer in memory is gone from the DB after the flush. On any error the
/// transaction is dropped (rolled back) and logged, leaving the last good save
/// intact.
pub(crate) async fn store_player(db: &DatabaseConnection, s: &PlayerSaveData) {
    if let Err(e) = store_player_tx(db, s).await {
        error!(
            "store_player: flush for char {} failed (rolled back): {e}",
            s.base.object_id
        );
    }
}

async fn store_player_tx(db: &DatabaseConnection, s: &PlayerSaveData) -> Result<(), DbErr> {
    let b = &s.base;
    let char_id = b.object_id;
    let tx = db.begin().await?;

    // characters row (Java storeCharBase). `online` stays 0: the port never
    // sets it to 1, and char-select doesn't read it — a periodic save of an
    // online player must not diverge from that. Columns left `NotSet` keep
    // their stored values, which is what the old UPDATE's column list did.
    characters::ActiveModel {
        char_id: Unchanged(char_id),
        level: Set(Some(b.level)),
        max_hp: Set(Some(b.max_hp)),
        cur_hp: Set(Some(b.cur_hp.into())),
        max_cp: Set(Some(b.max_cp)),
        cur_cp: Set(Some(b.cur_cp.into())),
        max_mp: Set(Some(b.max_mp)),
        cur_mp: Set(Some(b.cur_mp.into())),
        face: Set(Some(b.face)),
        hair_style: Set(Some(b.hair_style)),
        hair_color: Set(Some(b.hair_color)),
        sex: Set(Some(b.sex)),
        heading: Set(Some(b.heading)),
        x: Set(Some(b.x)),
        y: Set(Some(b.y)),
        z: Set(Some(b.z)),
        exp: Set(Some(b.exp)),
        sp: Set(b.sp),
        reputation: Set(Some(b.reputation)),
        pvpkills: Set(Some(b.pvp_kills)),
        pkkills: Set(Some(b.pk_kills)),
        raidboss_points: Set(b.raidboss_points),
        race: Set(Some(b.race)),
        classid: Set(Some(b.class_id)),
        base_class: Set(b.base_class_id),
        vitality_points: Set(b.vitality_points),
        pccafe_points: Set(b.pccafe_points),
        nobless: Set(if b.noble { 1 } else { 0 }),
        online: Set(Some(0)),
        last_access: Set(now_millis()),
        ..Default::default()
    }
    .update(&tx)
    .await?;

    // character_reco_bonus (Java `Player.storeRecommendations`). `time_left` is
    // always 0 — the reco bonus timer isn't used in Interlude Classic.
    character_reco_bonus::Entity::insert(character_reco_bonus::ActiveModel {
        char_id: Set(char_id),
        rec_have: Set(b.rec_have),
        rec_left: Set(b.rec_left),
        time_left: Set(0),
    })
    .on_conflict(
        OnConflict::column(character_reco_bonus::Column::CharId)
            .update_columns([
                character_reco_bonus::Column::RecHave,
                character_reco_bonus::Column::RecLeft,
                character_reco_bonus::Column::TimeLeft,
            ])
            .to_owned(),
    )
    .exec(&tx)
    .await?;

    // items (inventory + equipped): `Inventory::to_rows` is the whole owned set.
    items::Entity::delete_many()
        .filter(items::Column::OwnerId.eq(char_id))
        .exec(&tx)
        .await?;
    for it in &s.items {
        items::Entity::insert(items::ActiveModel {
            owner_id: Set(Some(char_id)),
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
        .exec(&tx)
        .await?;
    }

    // Augmentations, keyed to the item rows just written (the old statement
    // sub-selected the same set).
    item_variations::Entity::delete_many()
        .filter(item_variations::Column::ItemId.is_in(s.items.iter().map(|it| it.object_id)))
        .exec(&tx)
        .await?;
    for it in s
        .items
        .iter()
        .filter(|it| it.augment_option1 != 0 || it.augment_option2 != 0)
    {
        item_variations::Entity::insert(item_variations::ActiveModel {
            item_id: Set(it.object_id),
            mineral_id: Set(it.augment_mineral),
            option1: Set(it.augment_option1),
            option2: Set(it.augment_option2),
        })
        .exec(&tx)
        .await?;
    }

    character_skills::Entity::delete_many()
        .filter(character_skills::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;

    character_hennas::Entity::delete_many()
        .filter(character_hennas::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    let mut henna_idx: Vec<(i32, &Vec<(i32, i32)>)> =
        s.hennas_by_index.iter().map(|(i, v)| (*i, v)).collect();
    henna_idx.push((s.class_index, &s.hennas));
    for (class_index, hennas) in henna_idx {
        for (slot, symbol_id) in hennas {
            character_hennas::Entity::insert(character_hennas::ActiveModel {
                char_id: Set(char_id),
                symbol_id: Set(Some(*symbol_id)),
                slot: Set(*slot),
                class_index: Set(class_index),
            })
            .on_conflict(
                OnConflict::columns([
                    character_hennas::Column::CharId,
                    character_hennas::Column::Slot,
                    character_hennas::Column::ClassIndex,
                ])
                .update_column(character_hennas::Column::SymbolId)
                .to_owned(),
            )
            .exec(&tx)
            .await?;
        }
    }
    let mut per_index: Vec<(i32, &Vec<(i32, i32, i32)>)> =
        s.skills_by_index.iter().map(|(i, v)| (*i, v)).collect();
    per_index.push((s.class_index, &s.skills));
    for (class_index, skills) in per_index {
        for (skill_id, level, sub_level) in skills {
            character_skills::Entity::insert(character_skills::ActiveModel {
                char_id: Set(char_id),
                skill_id: Set(*skill_id),
                skill_level: Set(*level),
                skill_sub_level: Set(*sub_level),
                class_index: Set(class_index),
            })
            .on_conflict(
                OnConflict::columns([
                    character_skills::Column::CharId,
                    character_skills::Column::SkillId,
                    character_skills::Column::ClassIndex,
                ])
                .update_columns([
                    character_skills::Column::SkillLevel,
                    character_skills::Column::SkillSubLevel,
                ])
                .to_owned(),
            )
            .exec(&tx)
            .await?;
        }
    }

    character_recipebook::Entity::delete_many()
        .filter(character_recipebook::Column::CharId.eq(char_id))
        .filter(character_recipebook::Column::ClassIndex.eq(0))
        .exec(&tx)
        .await?;
    for (list_id, is_dwarven) in &s.recipe_book {
        character_recipebook::Entity::insert(character_recipebook::ActiveModel {
            char_id: Set(char_id),
            id: Set((*list_id).into()),
            class_index: Set(0),
            r#type: Set(if *is_dwarven { 1 } else { 0 }),
        })
        .exec(&tx)
        .await?;
    }

    character_variables::Entity::delete_many()
        .filter(character_variables::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    for (var, val) in &s.variables {
        character_variables::Entity::insert(character_variables::ActiveModel {
            char_id: Set(char_id),
            var: Set(var.clone()),
            val: Set(val.clone()),
        })
        .exec(&tx)
        .await?;
    }

    for pet in &s.pets {
        pets::Entity::insert(pets::ActiveModel {
            item_obj_id: Set(pet.collar_object_id),
            name: Set(Some(pet.name.clone())),
            level: Set(pet.level),
            cur_hp: Set(Some(pet.cur_hp.into())),
            cur_mp: Set(Some(pet.cur_mp.into())),
            exp: Set(Some(pet.exp)),
            sp: Set(Some(pet.sp)),
            fed: Set(Some(pet.fed)),
            owner_id: Set(char_id),
            restore: Set(if pet.restore { "true" } else { "false" }.to_string()),
        })
        .on_conflict(
            OnConflict::column(pets::Column::ItemObjId)
                .update_columns([
                    pets::Column::Name,
                    pets::Column::Level,
                    pets::Column::CurHp,
                    pets::Column::CurMp,
                    pets::Column::Exp,
                    pets::Column::Sp,
                    pets::Column::Fed,
                    pets::Column::OwnerId,
                    pets::Column::Restore,
                ])
                .to_owned(),
        )
        .exec(&tx)
        .await?;
    }

    // Summons are best-effort, as they were before: a servitor that fails to
    // persist costs a resummon, and must not roll back the character save.
    let _ = character_summons::Entity::delete_many()
        .filter(character_summons::Column::OwnerId.eq(char_id))
        .exec(&tx)
        .await;
    for summon in &s.summons {
        let _ = character_summons::Entity::insert(character_summons::ActiveModel {
            owner_id: Set(char_id),
            summon_id: Set(0),
            summon_skill_id: Set(summon.summon_skill_id),
            cur_hp: Set(Some(summon.cur_hp)),
            cur_mp: Set(Some(summon.cur_mp)),
            time: Set(summon.remaining_secs),
        })
        .exec(&tx)
        .await;
        let _ = character_summon_skills_save::Entity::delete_many()
            .filter(character_summon_skills_save::Column::OwnerId.eq(char_id))
            .filter(character_summon_skills_save::Column::OwnerClassIndex.eq(0))
            .filter(character_summon_skills_save::Column::SummonSkillId.eq(summon.summon_skill_id))
            .exec(&tx)
            .await;
        for (i, buff) in summon.buffs.iter().enumerate() {
            let _ = character_summon_skills_save::Entity::insert(
                character_summon_skills_save::ActiveModel {
                    owner_id: Set(char_id),
                    owner_class_index: Set(0),
                    summon_skill_id: Set(summon.summon_skill_id),
                    skill_id: Set(buff.skill_id),
                    skill_level: Set(buff.skill_level),
                    skill_sub_level: Set(0),
                    remaining_time: Set(buff.remaining_time_secs),
                    buff_index: Set(i as i32),
                },
            )
            .exec(&tx)
            .await;
        }
    }

    character_shortcuts::Entity::delete_many()
        .filter(character_shortcuts::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    let mut sc_idx: Vec<(i32, &Vec<crate::model::shortcut::Shortcut>)> =
        s.shortcuts_by_index.iter().map(|(i, v)| (*i, v)).collect();
    sc_idx.push((s.class_index, &s.shortcuts));
    for (class_index, shortcuts) in sc_idx {
        for sc in shortcuts {
            character_shortcuts::Entity::insert(character_shortcuts::ActiveModel {
                char_id: Set(char_id),
                slot: Set(sc.slot),
                page: Set(sc.page),
                r#type: Set(Some(sc.kind.ordinal())),
                shortcut_id: Set(Some(sc.id.into())),
                level: Set(Some(sc.level)),
                sub_level: Set(0),
                class_index: Set(class_index),
            })
            .on_conflict(
                OnConflict::columns([
                    character_shortcuts::Column::CharId,
                    character_shortcuts::Column::Slot,
                    character_shortcuts::Column::Page,
                    character_shortcuts::Column::ClassIndex,
                ])
                .update_columns([
                    character_shortcuts::Column::Type,
                    character_shortcuts::Column::ShortcutId,
                    character_shortcuts::Column::Level,
                ])
                .to_owned(),
            )
            .exec(&tx)
            .await?;
        }
    }

    character_macroses::Entity::delete_many()
        .filter(character_macroses::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    for m in &s.macros {
        character_macroses::Entity::insert(character_macroses::ActiveModel {
            char_id: Set(char_id),
            id: Set(m.id),
            icon: Set(Some(m.icon)),
            name: Set(Some(m.name.clone())),
            descr: Set(Some(m.descr.clone())),
            acronym: Set(Some(m.acronym.clone())),
            commands: Set(Some(crate::model::shortcut::encode_commands(&m.commands))),
        })
        .exec(&tx)
        .await?;
    }

    character_quests::Entity::delete_many()
        .filter(character_quests::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    for (name, qs) in &s.quests {
        use crate::model::quest::{STATE_VAR, state};
        if qs.state == state::CREATED && qs.vars.is_empty() {
            continue;
        }
        character_quests::Entity::insert(character_quests::ActiveModel {
            char_id: Set(char_id),
            name: Set(name.clone()),
            var: Set(STATE_VAR.to_string()),
            value: Set(Some(state::name(qs.state).to_string())),
        })
        .exec(&tx)
        .await?;
        for (var, value) in &qs.vars {
            character_quests::Entity::insert(character_quests::ActiveModel {
                char_id: Set(char_id),
                name: Set(name.clone()),
                var: Set(var.clone()),
                value: Set(Some(value.clone())),
            })
            .exec(&tx)
            .await?;
        }
    }

    character_skills_save::Entity::delete_many()
        .filter(character_skills_save::Column::CharId.eq(char_id))
        .filter(character_skills_save::Column::ClassIndex.eq(s.class_index))
        .exec(&tx)
        .await?;
    for (i, b) in s.skill_buffs.iter().enumerate() {
        character_skills_save::Entity::insert(character_skills_save::ActiveModel {
            char_id: Set(char_id),
            skill_id: Set(b.skill_id),
            skill_level: Set(b.skill_level),
            skill_sub_level: Set(0),
            remaining_time: Set(b.remaining_time_secs),
            reuse_delay: Set(0),
            systime: Set(0),
            restore_type: Set(0),
            class_index: Set(s.class_index),
            buff_index: Set(i as i32 + 1),
        })
        .exec(&tx)
        .await?;
    }
    let buff_rows = s.skill_buffs.len() as i32;
    for (i, r) in s.skill_reuses.iter().enumerate() {
        character_skills_save::Entity::insert(character_skills_save::ActiveModel {
            char_id: Set(char_id),
            skill_id: Set(r.reuse_key),
            skill_level: Set(r.skill_level),
            skill_sub_level: Set(0),
            remaining_time: Set(-1),
            reuse_delay: Set(r.reuse_delay),
            systime: Set(r.systime_ms),
            restore_type: Set(1),
            class_index: Set(s.class_index),
            buff_index: Set(buff_rows + i as i32 + 1),
        })
        .exec(&tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Character count + pending-deletion timestamps for the login server's
/// `ReplyCharacters` (Java `LoginServerThread.getCharsOnServer`). Mirrors
/// [`load_characters`]: a character whose deletion timer has **expired** is
/// purged and excluded, so the login server-select count never exceeds the
/// char-select list the client sees on entry (the port has no separate global
/// expired-char sweep, so counting raw rows would over-report).
pub(crate) async fn count_characters(db: &DatabaseConnection, account: &str) -> (u8, Vec<i64>) {
    let rows = characters::Entity::find()
        .filter(characters::Column::AccountName.eq(account))
        .all(db)
        .await
        .unwrap_or_default();
    let now = now_millis();
    let mut count: u8 = 0;
    let mut del_times = Vec::new();
    for row in &rows {
        if row.deletetime > 0 && now > row.deletetime {
            delete_char(db, row.char_id).await; // restoreChar: purge expired
            continue;
        }
        count += 1;
        if row.deletetime != 0 {
            del_times.push(row.deletetime); // still counting down toward deletion
        }
    }
    (count, del_times)
}

pub(crate) async fn delete_char(db: &DatabaseConnection, char_id: i32) {
    if let Err(e) = characters::Entity::delete_by_id(char_id).exec(db).await {
        warn!("DB thread: delete_char failed: {e}");
    }
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
    clan_wars::Entity::find()
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
    crests::Entity::find()
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
    pledge_recruit::Entity::find()
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
    let waiting = pledge_waiting_list::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    if waiting.is_empty() {
        return Vec::new();
    }
    // Java's query LEFT JOINs `characters` for the display fields; an applicant
    // whose character is gone keeps the row with empty values.
    let chars = characters::Entity::find()
        .filter(characters::Column::CharId.is_in(waiting.iter().map(|w| w.char_id)))
        .all(db)
        .await
        .unwrap_or_default();
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
    let applicants = pledge_applicant::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    if applicants.is_empty() {
        return Vec::new();
    }
    let chars = characters::Entity::find()
        .filter(characters::Column::CharId.is_in(applicants.iter().map(|a| a.char_id)))
        .all(db)
        .await
        .unwrap_or_default();
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
