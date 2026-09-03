//! The character write paths that do not go through a `DbCommand` arm:
//! creation, the full `store_player` flush, and deletion.

use super::super::CreateResult;
use super::super::NewCharacter;
use super::super::PlayerSaveData;
use super::{insert_or_warn, item_row_model, today};
use commons::util::now_millis;
use models::entity;

use models::sea_orm::ActiveValue::Set;
use models::sea_orm::ActiveValue::Unchanged;
use models::sea_orm::DatabaseConnection;
use models::sea_orm::DbErr;
use models::sea_orm::sea_query::Expr;
use models::sea_orm::sea_query::OnConflict;
use models::sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, TransactionTrait,
};
use tracing::error;
use tracing::info;
use tracing::warn;

/// One `character_shortcuts` row upsert. A slot is keyed by (char, page, slot,
/// class index), so re-saving the same slot overwrites what it holds rather
/// than colliding. Left un-`exec`ed so callers can run it on the pool or
/// inside a save transaction.
fn shortcut_upsert(
    char_id: i32,
    class_index: i32,
    slot: i32,
    page: i32,
    kind: i32,
    shortcut_id: i32,
    level: i32,
) -> models::sea_orm::Insert<entity::character_shortcuts::ActiveModel> {
    entity::character_shortcuts::Entity::insert(entity::character_shortcuts::ActiveModel {
        char_id: Set(char_id),
        slot: Set(slot),
        page: Set(page),
        r#type: Set(Some(kind)),
        shortcut_id: Set(Some(shortcut_id.into())),
        level: Set(Some(level)),
        sub_level: Set(0),
        class_index: Set(class_index),
    })
    .on_conflict(
        OnConflict::columns([
            entity::character_shortcuts::Column::CharId,
            entity::character_shortcuts::Column::Slot,
            entity::character_shortcuts::Column::Page,
            entity::character_shortcuts::Column::ClassIndex,
        ])
        .update_columns([
            entity::character_shortcuts::Column::Type,
            entity::character_shortcuts::Column::ShortcutId,
            entity::character_shortcuts::Column::Level,
        ])
        .to_owned(),
    )
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
    let res = shortcut_upsert(char_id, 0, slot, page, kind, shortcut_id, level)
        .exec(db)
        .await;
    if let Err(e) = res {
        warn!("DB thread: upsert_shortcut failed: {e}");
    }
}
/// One `character_macroses` row upsert, keyed by (char, macro id) so re-saving
/// a macro overwrites it. Left un-`exec`ed so callers can run it on the pool or
/// inside a save transaction, like [`shortcut_upsert`].
fn macro_upsert(
    char_id: i32,
    m: &crate::model::shortcut::Macro,
) -> models::sea_orm::Insert<entity::character_macroses::ActiveModel> {
    entity::character_macroses::Entity::insert(entity::character_macroses::ActiveModel {
        char_id: Set(char_id),
        id: Set(m.id),
        icon: Set(Some(m.icon)),
        name: Set(Some(m.name.clone())),
        descr: Set(Some(m.descr.clone())),
        acronym: Set(Some(m.acronym.clone())),
        commands: Set(Some(crate::model::shortcut::encode_commands(&m.commands))),
    })
    .on_conflict(
        OnConflict::columns([
            entity::character_macroses::Column::CharId,
            entity::character_macroses::Column::Id,
        ])
        .update_columns([
            entity::character_macroses::Column::Icon,
            entity::character_macroses::Column::Name,
            entity::character_macroses::Column::Descr,
            entity::character_macroses::Column::Acronym,
            entity::character_macroses::Column::Commands,
        ])
        .to_owned(),
    )
}
async fn upsert_macro(db: &DatabaseConnection, char_id: i32, m: &crate::model::shortcut::Macro) {
    let res = macro_upsert(char_id, m).exec(db).await;
    if let Err(e) = res {
        warn!("DB thread: upsert_macro failed: {e}");
    }
}
/// Case-insensitive character-name existence check (`getIdByName`).
pub(crate) async fn name_exists(db: &DatabaseConnection, name: &str) -> bool {
    // `COLLATE NOCASE` is the point of this query — two characters may not
    // differ only by case — and sea-query cannot attach a collation, so the
    // comparison stays a bound custom expression.
    entity::characters::Entity::find()
        .filter(Expr::cust_with_values(
            "char_name = ? COLLATE NOCASE",
            [name],
        ))
        .count(db)
        .await
        .unwrap_or(0)
        > 0
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
    let count = entity::characters::Entity::find()
        .filter(entity::characters::Column::AccountName.eq(&data.account))
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
    let row = entity::characters::ActiveModel {
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
    if let Err(e) = entity::characters::Entity::insert(row).exec(db).await {
        error!("DB thread: character insert failed: {e}");
        return CreateResult::Fail;
    }

    // Seed the recommendation row: Java `Player.create` grants rec_left=20,
    // persisted to `character_reco_bonus` when the freshly-created character
    // disconnects back to the lobby.
    insert_or_warn(
        db,
        entity::character_reco_bonus::Entity::insert(entity::character_reco_bonus::ActiveModel {
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
            entity::character_skills::Entity::insert(entity::character_skills::ActiveModel {
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
            entity::items::Entity::insert(entity::items::ActiveModel {
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
    entity::characters::ActiveModel {
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
        exp_before_death: Set(Some(b.exp_before_death)),
        nobless: Set(if b.noble { 1 } else { 0 }),
        online: Set(Some(0)),
        last_access: Set(now_millis()),
        ..Default::default()
    }
    .update(&tx)
    .await?;

    // character_reco_bonus (Java `Player.storeRecommendations`). `time_left` is
    // always 0 — the reco bonus timer isn't used in Interlude Classic.
    entity::character_reco_bonus::Entity::insert(entity::character_reco_bonus::ActiveModel {
        char_id: Set(char_id),
        rec_have: Set(b.rec_have),
        rec_left: Set(b.rec_left),
        time_left: Set(0),
    })
    .on_conflict(
        OnConflict::column(entity::character_reco_bonus::Column::CharId)
            .update_columns([
                entity::character_reco_bonus::Column::RecHave,
                entity::character_reco_bonus::Column::RecLeft,
                entity::character_reco_bonus::Column::TimeLeft,
            ])
            .to_owned(),
    )
    .exec(&tx)
    .await?;

    // items (inventory + equipped): `Inventory::to_rows` is the whole owned set.
    // Skipped wholesale under `UpdateItemsOnCharStore = False`, which is Java's
    // `autoSave` gate — the rows then survive untouched until a path that
    // writes them directly (logout, a trade, an enchant) does so.
    if s.store_items {
        entity::items::Entity::delete_many()
            .filter(entity::items::Column::OwnerId.eq(char_id))
            .exec(&tx)
            .await?;
        for it in &s.items {
            entity::items::Entity::insert(item_row_model(char_id, it, None))
                .exec(&tx)
                .await?;
        }
    }

    // Augmentations, keyed to the item rows just written (the old statement
    // sub-selected the same set).
    entity::item_variations::Entity::delete_many()
        .filter(
            entity::item_variations::Column::ItemId.is_in(s.items.iter().map(|it| it.object_id)),
        )
        .exec(&tx)
        .await?;
    for it in s
        .items
        .iter()
        .filter(|it| it.augment_option1 != 0 || it.augment_option2 != 0)
    {
        entity::item_variations::Entity::insert(entity::item_variations::ActiveModel {
            item_id: Set(it.object_id),
            mineral_id: Set(it.augment_mineral),
            option1: Set(it.augment_option1),
            option2: Set(it.augment_option2),
        })
        .exec(&tx)
        .await?;
    }

    entity::character_skills::Entity::delete_many()
        .filter(entity::character_skills::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;

    entity::character_hennas::Entity::delete_many()
        .filter(entity::character_hennas::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    let mut henna_idx: Vec<(i32, &Vec<(i32, i32)>)> =
        s.hennas_by_index.iter().map(|(i, v)| (*i, v)).collect();
    henna_idx.push((s.class_index, &s.hennas));
    for (class_index, hennas) in henna_idx {
        for (slot, symbol_id) in hennas {
            entity::character_hennas::Entity::insert(entity::character_hennas::ActiveModel {
                char_id: Set(char_id),
                symbol_id: Set(Some(*symbol_id)),
                slot: Set(*slot),
                class_index: Set(class_index),
            })
            .on_conflict(
                OnConflict::columns([
                    entity::character_hennas::Column::CharId,
                    entity::character_hennas::Column::Slot,
                    entity::character_hennas::Column::ClassIndex,
                ])
                .update_column(entity::character_hennas::Column::SymbolId)
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
            entity::character_skills::Entity::insert(entity::character_skills::ActiveModel {
                char_id: Set(char_id),
                skill_id: Set(*skill_id),
                skill_level: Set(*level),
                skill_sub_level: Set(*sub_level),
                class_index: Set(class_index),
            })
            .on_conflict(
                OnConflict::columns([
                    entity::character_skills::Column::CharId,
                    entity::character_skills::Column::SkillId,
                    entity::character_skills::Column::ClassIndex,
                ])
                .update_columns([
                    entity::character_skills::Column::SkillLevel,
                    entity::character_skills::Column::SkillSubLevel,
                ])
                .to_owned(),
            )
            .exec(&tx)
            .await?;
        }
    }

    entity::character_recipebook::Entity::delete_many()
        .filter(entity::character_recipebook::Column::CharId.eq(char_id))
        .filter(entity::character_recipebook::Column::ClassIndex.eq(0))
        .exec(&tx)
        .await?;
    for (list_id, is_dwarven) in &s.recipe_book {
        entity::character_recipebook::Entity::insert(entity::character_recipebook::ActiveModel {
            char_id: Set(char_id),
            id: Set((*list_id).into()),
            class_index: Set(0),
            r#type: Set(if *is_dwarven { 1 } else { 0 }),
        })
        .exec(&tx)
        .await?;
    }

    entity::character_variables::Entity::delete_many()
        .filter(entity::character_variables::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    for (var, val) in &s.variables {
        entity::character_variables::Entity::insert(entity::character_variables::ActiveModel {
            char_id: Set(char_id),
            var: Set(var.clone()),
            val: Set(val.clone()),
        })
        .exec(&tx)
        .await?;
    }

    for pet in &s.pets {
        entity::pets::Entity::insert(entity::pets::ActiveModel {
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
            OnConflict::column(entity::pets::Column::ItemObjId)
                .update_columns([
                    entity::pets::Column::Name,
                    entity::pets::Column::Level,
                    entity::pets::Column::CurHp,
                    entity::pets::Column::CurMp,
                    entity::pets::Column::Exp,
                    entity::pets::Column::Sp,
                    entity::pets::Column::Fed,
                    entity::pets::Column::OwnerId,
                    entity::pets::Column::Restore,
                ])
                .to_owned(),
        )
        .exec(&tx)
        .await?;
    }

    // Summons are best-effort, as they were before: a servitor that fails to
    // persist costs a resummon, and must not roll back the character save.
    let _ = entity::character_summons::Entity::delete_many()
        .filter(entity::character_summons::Column::OwnerId.eq(char_id))
        .exec(&tx)
        .await;
    for summon in &s.summons {
        let _ = entity::character_summons::Entity::insert(entity::character_summons::ActiveModel {
            owner_id: Set(char_id),
            summon_id: Set(0),
            summon_skill_id: Set(summon.summon_skill_id),
            cur_hp: Set(Some(summon.cur_hp)),
            cur_mp: Set(Some(summon.cur_mp)),
            time: Set(summon.remaining_secs),
        })
        .exec(&tx)
        .await;
        let _ = entity::character_summon_skills_save::Entity::delete_many()
            .filter(entity::character_summon_skills_save::Column::OwnerId.eq(char_id))
            .filter(entity::character_summon_skills_save::Column::OwnerClassIndex.eq(0))
            .filter(
                entity::character_summon_skills_save::Column::SummonSkillId
                    .eq(summon.summon_skill_id),
            )
            .exec(&tx)
            .await;
        for (i, buff) in summon.buffs.iter().enumerate() {
            let _ = entity::character_summon_skills_save::Entity::insert(
                entity::character_summon_skills_save::ActiveModel {
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

    entity::character_shortcuts::Entity::delete_many()
        .filter(entity::character_shortcuts::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    let mut sc_idx: Vec<(i32, &Vec<crate::model::shortcut::Shortcut>)> =
        s.shortcuts_by_index.iter().map(|(i, v)| (*i, v)).collect();
    sc_idx.push((s.class_index, &s.shortcuts));
    for (class_index, shortcuts) in sc_idx {
        for sc in shortcuts {
            shortcut_upsert(
                char_id,
                class_index,
                sc.slot,
                sc.page,
                sc.kind.ordinal(),
                sc.id,
                sc.level,
            )
            .exec(&tx)
            .await?;
        }
    }

    entity::character_macroses::Entity::delete_many()
        .filter(entity::character_macroses::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    for m in &s.macros {
        macro_upsert(char_id, m).exec(&tx).await?;
    }

    entity::character_quests::Entity::delete_many()
        .filter(entity::character_quests::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    for (name, qs) in &s.quests {
        use crate::model::quest::{STATE_VAR, state};
        if qs.state == state::CREATED && qs.vars.is_empty() {
            continue;
        }
        entity::character_quests::Entity::insert(entity::character_quests::ActiveModel {
            char_id: Set(char_id),
            name: Set(name.clone()),
            var: Set(STATE_VAR.to_string()),
            value: Set(Some(state::name(qs.state).to_string())),
        })
        .exec(&tx)
        .await?;
        for (var, value) in &qs.vars {
            entity::character_quests::Entity::insert(entity::character_quests::ActiveModel {
                char_id: Set(char_id),
                name: Set(name.clone()),
                var: Set(var.clone()),
                value: Set(Some(value.clone())),
            })
            .exec(&tx)
            .await?;
        }
    }

    entity::character_skills_save::Entity::delete_many()
        .filter(entity::character_skills_save::Column::CharId.eq(char_id))
        .filter(entity::character_skills_save::Column::ClassIndex.eq(s.class_index))
        .exec(&tx)
        .await?;
    for (i, b) in s.skill_buffs.iter().enumerate() {
        entity::character_skills_save::Entity::insert(entity::character_skills_save::ActiveModel {
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
        entity::character_skills_save::Entity::insert(entity::character_skills_save::ActiveModel {
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
    let rows = entity::characters::Entity::find()
        .filter(entity::characters::Column::AccountName.eq(account))
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
    if let Err(e) = entity::characters::Entity::delete_by_id(char_id)
        .exec(db)
        .await
    {
        warn!("DB thread: delete_char failed: {e}");
    }
}
