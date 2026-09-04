//! The write payloads a `DbCommand` carries: a new character and its
//! starting rows, and the full player snapshot the autosave flushes.

use super::super::ItemRow;
use super::rows::{PetRow, SkillBuffRow, SkillReuseRow, SummonRow};

/// A starting item, already slot-resolved by the game thread (see
/// `game_loop::handle_character_create`) so the DB thread just persists rows.
#[derive(Debug, Clone)]
pub struct NewItem {
    pub item_id: i32,
    pub count: i64,
    /// `Some(paperdoll_index)` if equipped, `None` for a plain inventory item.
    pub paperdoll_index: Option<usize>,
}

/// An initial shortcut to persist at creation (`InitialShortcutData.
/// registerAllShortcuts`), already filtered by the game thread (unknown
/// skills / missing macro presets dropped). For `ShortcutType::Item` the `id`
/// is still the *item id* — the DB thread resolves it to the freshly created
/// item's object id (the game thread never learns those).
#[derive(Debug, Clone, Copy)]
pub struct NewShortcut {
    pub slot: i32,
    pub page: i32,
    pub kind: crate::model::shortcut::ShortcutType,
    pub id: i32,
    pub level: i32,
}

/// A validated character to insert (built by the game thread from the template).
#[derive(Debug, Clone)]
pub struct NewCharacter {
    pub account: String,
    pub name: String,
    pub race: i32,
    pub class_id: i32,
    pub sex: i32,
    pub face: i32,
    pub hair_style: i32,
    pub hair_color: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub max_hp: i32,
    pub max_mp: i32,
    /// Initial `(skill_id, skill_level)` from the class skill tree.
    pub skills: Vec<(i32, i32)>,
    /// Initial equipment + starting adena, pre-resolved by the game thread.
    pub items: Vec<NewItem>,
    /// Initial panel shortcuts (`initialShortcuts.xml`, global + class pages).
    pub shortcuts: Vec<NewShortcut>,
    /// Macro presets referenced by MACRO shortcuts above.
    pub macros: Vec<crate::model::shortcut::Macro>,
    /// `CharacterCreate`: `min(StartingVitalityPoints, MAX_VITALITY_POINTS)`
    /// when `EnableVitality`, else the column default (0). Resolved on the game
    /// thread, which owns the config.
    pub vitality_points: i32,
}

/// The persistable slice of a `Player`, snapshotted on the game thread when the
/// character leaves the world (restart / logout / disconnect) — Java
/// `Disconnection.storeMe().deleteMe()`. Covers the `storeCharBase` columns the
/// Rust `Player` actually tracks; the rest (clan, title, online time, faction,
/// …) keep their stored values. Java's companion stores — `storeCharSub`,
/// `storeEffect` (`character_skills_save`), item reuse — write through their
/// own paths rather than this one: subclasses landed with G17
/// (`character_subclasses`) and buff restore on login with G19's relative
/// `remaining_time` rows, both flushed where they are mutated.
/// Items and learned skills are already persisted at mutation time.
#[derive(Debug, Clone)]
pub struct PlayerSnapshot {
    pub object_id: i32,
    pub level: i32,
    pub max_hp: i32,
    pub cur_hp: f64,
    pub max_cp: i32,
    pub cur_cp: f64,
    pub max_mp: i32,
    pub cur_mp: f64,
    pub face: i32,
    pub hair_style: i32,
    pub hair_color: i32,
    pub sex: i32,
    pub heading: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub exp: i64,
    pub sp: i64,
    pub reputation: i32,
    pub pvp_kills: i32,
    pub pk_kills: i32,
    pub raidboss_points: i32,
    pub rec_have: i32,
    pub rec_left: i32,
    pub race: i32,
    pub class_id: i32,
    pub base_class_id: i32,
    pub vitality_points: i32,
    pub pccafe_points: i32,
    /// `characters.expBeforeDeath` — Java `Player._expBeforeDeath`, the exp
    /// total the character had **before** its last death, so a resurrection
    /// can hand back a percentage of what the death penalty took. Java stores
    /// it in `UPDATE_CHARACTER` and restores it, which is what lets a player
    /// die, log out, come back and *then* be resurrected with their exp.
    ///
    /// The live side holds the **delta** (`Player::lost_exp_on_death`), so this
    /// is `exp + lost` on the way out and `stored − exp` on the way back —
    /// which is exactly the arithmetic Java's `restoreExp` does at the far end.
    /// Zero means nothing to restore.
    pub exp_before_death: i64,
    /// `characters.nobless` — Olympiad nobless, toggled by `//setnoble`.
    pub noble: bool,
}

impl PlayerSnapshot {
    pub fn of(
        p: &crate::model::Player,
        pos: &crate::model::components::space::Position,
        vitals: &crate::model::components::stats::Vitals,
        pvitals: &crate::model::components::stats::PlayerVitals,
    ) -> Self {
        Self {
            object_id: p.object_id,
            level: p.level,
            max_hp: vitals.max_hp,
            cur_hp: vitals.cur_hp,
            max_cp: pvitals.max_cp,
            cur_cp: pvitals.cur_cp,
            max_mp: vitals.max_mp,
            cur_mp: vitals.cur_mp,
            face: p.face,
            hair_style: p.hair_style,
            hair_color: p.hair_color,
            sex: p.is_female as i32,
            heading: pos.heading,
            x: pos.x,
            y: pos.y,
            z: pos.z,
            exp: p.exp,
            sp: p.sp,
            reputation: p.reputation,
            pvp_kills: p.pvp_kills,
            pk_kills: p.pk_kills,
            raidboss_points: p.raidboss_points,
            rec_have: p.rec_have,
            rec_left: p.rec_left,
            race: p.race,
            class_id: p.class_id,
            base_class_id: p.base_class_id,
            vitality_points: p.vitality_points,
            pccafe_points: p.pccafe_points,
            // The column is Java's absolute "exp before the death"; the live
            // side keeps the delta, so re-add the current total. Zero when
            // there is nothing to restore, which is also what Java writes once
            // `restoreExp` has cleared it.
            exp_before_death: if p.lost_exp_on_death > 0 {
                p.exp + p.lost_exp_on_death
            } else {
                0
            },
            noble: p.is_noble,
        }
    }
}

/// The full persistable state of an online player, gathered on the game thread
/// and flushed by the DB thread in one transaction (`store_player`). Built by
/// `game_loop::net::build_save_data` at the four flush points — the staggered
/// periodic autosave, logout, class-transfer, and shutdown save-all. Between
/// flushes, gameplay mutations (equip, loot, skill learn, shortcuts, quests)
/// touch only in-memory ECS components; nothing is written on the packet path,
/// so no client packet can drive database load (the memory-first model — Java
/// `Player.store()` gathers the same data, but Java also writes eagerly on many
/// actions, which is exactly what this port deliberately does not do).
#[derive(Debug, Clone)]
pub struct PlayerSaveData {
    /// The `characters` row (level/exp/vitals/position/appearance).
    pub base: PlayerSnapshot,
    /// Every item the character owns — inventory + equipped — serialized from
    /// the `Inventory` component (`Inventory::to_rows`). The DB thread deletes
    /// any `items` row for this owner not present here, so this is the whole
    /// authoritative set, covering pickups, drops, stack changes and equips.
    pub items: Vec<ItemRow>,
    /// `Config.UPDATE_ITEMS_ON_CHAR_STORE` (**True** here, Java default
    /// `false`) — whether this save writes the item half at all.
    ///
    /// It has to be a flag rather than an empty [`Self::items`], because the
    /// write is delete-then-reinsert over the whole owned set: an empty vector
    /// would not mean "leave the items alone", it would mean "delete them".
    pub store_items: bool,
    /// Learned skills as `(skill_id, skill_level, skill_sub_level)` for the
    /// **active** class index (see [`Self::class_index`]).
    pub skills: Vec<(i32, i32, i32)>,
    /// The *inactive* class indices' books (G17 subclasses), so a slot keeps
    /// what it learned while it was active.
    pub skills_by_index: std::collections::HashMap<i32, Vec<(i32, i32, i32)>>,
    /// Inactive indices' worn hennas.
    pub hennas_by_index: std::collections::HashMap<i32, Vec<(i32, i32)>>,
    /// Inactive indices' shortcut bars.
    pub shortcuts_by_index: std::collections::HashMap<i32, Vec<crate::model::shortcut::Shortcut>>,
    /// Which class index [`Self::skills`] belongs to.
    pub class_index: i32,
    /// Worn henna dyes as `(slot 1-3, symbol_id)` (class_index 0).
    pub hennas: Vec<(i32, i32)>,
    /// Registered recipes as `(recipe_list_id, is_dwarven)` — the `type` column
    /// (1 dwarven / 0 common) is derived on the game thread from `RecipeData`.
    pub recipe_book: Vec<(i32, bool)>,
    /// Panel/hotbar shortcuts (`Shortcuts` component).
    pub shortcuts: Vec<crate::model::shortcut::Shortcut>,
    /// Macro definitions (`Macros` component).
    pub macros: Vec<crate::model::shortcut::Macro>,
    /// Quest states + vars (`Quests` component), keyed by quest name.
    pub quests: std::collections::HashMap<String, crate::model::quest::QuestState>,
    /// Live skill reuse cooldowns (`Reuses` component) as `character_skills_save`
    /// rows — empty when `StoreSkillCooltime` is off. See [`SkillReuseRow`].
    pub skill_reuses: Vec<SkillReuseRow>,
    /// Active buffs (`Buffs` component) as `character_skills_save` rows —
    /// empty when `StoreSkillCooltime` is off. See [`SkillBuffRow`].
    pub skill_buffs: Vec<SkillBuffRow>,
    /// `character_variables` rows (`PlayerVariables` component) as `(var, val)`.
    pub variables: Vec<(String, String)>,
    /// Every `pets` row this character owns (`PlayerPets` component), including
    /// the currently-summoned pet, whose live state is folded in before the
    /// save. Upserted row by row — **never** deleted as a set, because a row is
    /// keyed by a collar this character may trade away rather than by the
    /// character (Java writes one pet at a time for the same reason).
    pub pets: Vec<PetRow>,
    /// `character_summons` rows — at most one on this dist. Replaced as a set
    /// (unlike `pets`), because a servitor row is keyed by its **owner** and
    /// so cannot be traded away.
    pub summons: Vec<SummonRow>,
}
