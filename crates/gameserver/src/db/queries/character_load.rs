//! Reading a character back out — the selection-screen list and the full
//! per-character row set enter-world needs.

use super::super::CharData;
use super::super::DbEvent;
use super::super::EventTx;
use super::super::FriendInfo;
use super::super::ItemRow;
use super::super::PetRow;
use super::super::SkillBuffRow;
use super::super::SkillReuseRow;
use super::super::SummonRow;
use super::account::load_account_var;
use super::character_store::delete_char;
use commons::util::now_millis;
use models::entity;

use models::sea_orm::DatabaseConnection;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use tracing::warn;

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
/// Java `CharInfoTable` — the offline character name -> id table. Mail is
/// addressed by name to characters who need not be online; nothing else in the
/// port needs this, so it is loaded once and maintained on creation/deletion.
pub(crate) async fn load_char_ids_by_name(db: &DatabaseConnection) -> Vec<(String, i32)> {
    entity::characters::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.char_name.to_lowercase(), row.char_id))
        .collect()
}
/// `RESTORE_CHAR_SUBCLASSES` — a character's subclass slots.
async fn load_subclasses(db: &DatabaseConnection, char_id: i32) -> Vec<crate::model::SubClass> {
    entity::character_subclasses::Entity::find()
        .filter(entity::character_subclasses::Column::CharId.eq(char_id))
        .order_by_asc(entity::character_subclasses::Column::ClassIndex)
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
/// `loadCharacterSelectInfo`: rows for an account, expired deletions purged.
async fn load_characters(db: &DatabaseConnection, account: &str) -> Vec<CharData> {
    let rows = match entity::characters::Entity::find()
        .filter(entity::characters::Column::AccountName.eq(account))
        .order_by_asc(entity::characters::Column::CreateDate)
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
    row: &entity::characters::Model,
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
            // `characters.expBeforeDeath` → the live delta. Java's `restoreExp`
            // computes `(_expBeforeDeath - getExp())` at restore time, so the
            // subtraction belongs here rather than at the write.
            lost_exp_on_death: (row.exp_before_death.unwrap_or(0) - row.exp.unwrap_or(0)).max(0),
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
/// One character by id, with every child collection — the offline-shop restore
/// needs a full `CharData` for a character it reaches through
/// `character_offline_trade`, not through an account's list.
pub(super) async fn load_character(db: &DatabaseConnection, char_id: i32) -> Option<CharData> {
    let row = entity::characters::Entity::find_by_id(char_id)
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
    let rows = entity::character_skills::Entity::find()
        .filter(entity::character_skills::Column::CharId.eq(owner_id))
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
    let rows = entity::character_hennas::Entity::find()
        .filter(entity::character_hennas::Column::CharId.eq(owner_id))
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
    entity::character_recipebook::Entity::find()
        .filter(entity::character_recipebook::Column::CharId.eq(owner_id))
        .filter(entity::character_recipebook::Column::ClassIndex.eq(0))
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
    entity::character_variables::Entity::find()
        .filter(entity::character_variables::Column::CharId.eq(owner_id))
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
    let rows = entity::character_summons::Entity::find()
        .filter(entity::character_summons::Column::OwnerId.eq(owner_id))
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
    entity::character_summon_skills_save::Entity::find()
        .filter(entity::character_summon_skills_save::Column::OwnerId.eq(owner_id))
        .filter(entity::character_summon_skills_save::Column::OwnerClassIndex.eq(0))
        .filter(entity::character_summon_skills_save::Column::SummonSkillId.eq(summon_skill_id))
        .order_by_asc(entity::character_summon_skills_save::Column::BuffIndex)
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
    entity::pets::Entity::find()
        .filter(entity::pets::Column::OwnerId.eq(owner_id))
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
    entity::character_skills_save::Entity::find()
        .filter(entity::character_skills_save::Column::CharId.eq(owner_id))
        .filter(entity::character_skills_save::Column::ClassIndex.eq(class_index))
        .filter(entity::character_skills_save::Column::RestoreType.eq(1))
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
    entity::character_skills_save::Entity::find()
        .filter(entity::character_skills_save::Column::CharId.eq(owner_id))
        .filter(entity::character_skills_save::Column::ClassIndex.eq(class_index))
        .filter(entity::character_skills_save::Column::RestoreType.eq(0))
        .order_by_asc(entity::character_skills_save::Column::BuffIndex)
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
    match entity::character_reco_bonus::Entity::find_by_id(owner_id)
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
    let rows = entity::character_shortcuts::Entity::find()
        .filter(entity::character_shortcuts::Column::CharId.eq(owner_id))
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
/// `character_friends.relation` — the one table stores two lists. Java writes
/// the friend rows with the column defaulted (`INSERT ... (charId, friendId)`)
/// and the block rows with an explicit `1`.
pub(crate) const FRIEND_RELATION: i32 = 0;
/// A character's `character_friends` rows joined with each friend's
/// character row — the name/level/class snapshot Java reads through
/// `CharInfoTable` on demand (`memo` unused).
///
/// **`relation = 0` is load-bearing**, as in Java's
/// `SELECT friendId FROM character_friends WHERE charId=? AND relation=0`. The
/// same table stores the *block* list at `relation = 1`
/// ([`load_block_list`]); without the filter every blocked character would
/// come back as a friend.
async fn load_friends(db: &DatabaseConnection, owner_id: i32) -> Vec<FriendInfo> {
    // The join is two reads instead of one: `character_friends` declares no
    // foreign key, so there is no relation to traverse — and a friend list is a
    // handful of rows.
    let ids: Vec<i32> = entity::character_friends::Entity::find()
        .filter(entity::character_friends::Column::CharId.eq(owner_id))
        .filter(entity::character_friends::Column::Relation.eq(FRIEND_RELATION))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| row.friend_id)
        .collect();
    if ids.is_empty() {
        return Vec::new();
    }
    characters_by_id(db, ids)
        .await
        .into_iter()
        .map(|row| FriendInfo {
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
    let rows = entity::character_quests::Entity::find()
        .filter(entity::character_quests::Column::CharId.eq(owner_id))
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
/// The `characters` rows for a set of char ids — the manual half of every
/// `LEFT JOIN characters` Java writes.
///
/// There is no FK to follow here (Java reaches these fields through
/// `CharInfoTable` for the same reason), so a loader fetches its own rows and
/// matches in memory. A deleted character simply has no row, and every caller
/// turns that into empty display values rather than dropping the record —
/// which is exactly what the LEFT part of Java's join does.
///
/// Callers guard on an empty id set before calling: `is_in([])` is a valid
/// query that returns nothing, but it is still a round trip.
pub(super) async fn characters_by_id(
    db: &DatabaseConnection,
    ids: impl IntoIterator<Item = i32>,
) -> Vec<entity::characters::Model> {
    entity::characters::Entity::find()
        .filter(entity::characters::Column::CharId.is_in(ids))
        .all(db)
        .await
        .unwrap_or_default()
}
/// A character's `character_macroses` rows (Java `MacroList.restoreMe`),
/// commands decoded from the `type,d1,d2[,cmd];…` column encoding.
async fn load_macros(db: &DatabaseConnection, owner_id: i32) -> Vec<crate::model::shortcut::Macro> {
    entity::character_macroses::Entity::find()
        .filter(entity::character_macroses::Column::CharId.eq(owner_id))
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
/// A character's `items` rows (Java: `PlayerInventory.restore`, called for
/// every row shown in `CharSelectionInfo`, not just the entered character).
pub(super) async fn load_items(db: &DatabaseConnection, owner_id: i32) -> Vec<ItemRow> {
    // Java `PlayerInventory.restore` orders by `loc_data` so a client's saved
    // inventory arrangement (`RequestSaveInventoryOrder`) survives relog.
    let rows = entity::items::Entity::find()
        .filter(entity::items::Column::OwnerId.eq(owner_id))
        .order_by_asc(entity::items::Column::LocData)
        .all(db)
        .await
        .unwrap_or_default();
    // Augmentations (Java `Item.restoreAttributes`): object_id → (mineral, o1, o2).
    let variations: std::collections::HashMap<i32, (i32, i32, i32)> =
        entity::item_variations::Entity::find()
            .filter(entity::item_variations::Column::ItemId.is_in(rows.iter().map(|r| r.object_id)))
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
