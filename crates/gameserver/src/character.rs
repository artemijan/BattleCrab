//! A stored character row, as loaded for the selection screen
//! (`CharSelectInfoPackage`) and cached by the `InLobby` session for slot →
//! object-id mapping.

/// One row of the `items` table (`Item`/`ItemInfo` as stored). Owner-scoped —
/// `owner_id` isn't kept here since rows always arrive already grouped by
/// character.
#[derive(Debug, Clone)]
pub struct ItemRow {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub enchant_level: i32,
    /// `ItemLocation` name (`"INVENTORY"`, `"PAPERDOLL"`, …) as stored.
    pub loc: String,
    /// Paperdoll slot index when `loc == "PAPERDOLL"`; unused otherwise.
    pub loc_data: i32,
    pub custom_type1: i32,
    pub custom_type2: i32,
    pub mana_left: i32,
    pub time: i32,
    /// Augmentation from `item_variations` (life stone id + two option ids);
    /// all `0` when unaugmented.
    pub augment_mineral: i32,
    pub augment_option1: i32,
    pub augment_option2: i32,
}

/// One `character_friends` row joined with the friend's character row
/// (Java reads the extra columns through `CharInfoTable` on demand).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendInfo {
    pub char_id: i32,
    pub name: String,
    pub level: i32,
    pub class_id: i32,
}

/// One row of the `characters` table, restored for character selection.
#[derive(Debug, Clone, Default)]
pub struct CharData {
    pub object_id: i32,
    pub name: String,
    pub account_name: String,
    pub level: i32,
    pub max_hp: i32,
    pub cur_hp: f64,
    pub max_mp: i32,
    pub cur_mp: f64,
    pub face: i32,
    pub hair_style: i32,
    pub hair_color: i32,
    pub sex: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub exp: i64,
    pub sp: i64,
    pub reputation: i32,
    pub pk_kills: i32,
    pub pvp_kills: i32,
    /// `character_reco_bonus.rec_have` / `.rec_left` (a separate table, not
    /// `characters`). Loaded by `db::load_reco_bonus`; 0/0 when the row is
    /// absent (Java `Player.loadRecommendations` leaves the field defaults).
    pub rec_have: i32,
    pub rec_left: i32,
    pub clan_id: i32,
    /// `characters.clan_privs` (the leader's is the all-bits mask).
    pub clan_privs: i32,
    /// `characters.clan_create_expiry_time` (10-day recreate cooldown).
    pub clan_create_expiry_time: i64,
    /// `characters.clan_join_expiry_time` (1-day rejoin penalty after
    /// leaving/being ousted from a clan).
    pub clan_join_expiry_time: i64,
    /// `characters.power_grade` (clan rank; 0 = never set → 5 at login).
    pub power_grade: i32,
    /// `characters.subpledge` (sub-unit membership; 0 = main pledge).
    pub pledge_type: i32,
    pub race: i32,
    pub class_id: i32,
    pub base_class_id: i32,
    pub delete_time: i64,
    pub last_access: i64,
    pub vitality_points: i32,
    /// `characters.pccafe_points` (PC bang loyalty points; capped by
    /// `PC_CAFE_MAX_POINTS`). Character-scoped like vitality.
    pub pccafe_points: i32,
    /// Account-scoped `account_gsdata` "PRIME_POINTS" (NCoin balance). Loaded at
    /// selection alongside the character row; not a `characters` column.
    pub prime_points: i32,
    pub access_level: i32,
    pub noble: bool,
    /// `character_subclasses` rows for this character (class index 1..N).
    pub subclasses: Vec<crate::model::SubClass>,
    pub char_slot: i32,
    pub items: Vec<ItemRow>,
    /// `character_skills` rows: (skill_id, skill_level).
    /// Learned skills for the **active** class index (what the login path puts
    /// in the skill book).
    pub skills: Vec<(i32, i32)>,
    /// Every class index's learned skills, so a subclass switch can restore the
    /// target slot's book instead of re-deriving it.
    pub skills_by_index: std::collections::HashMap<i32, Vec<(i32, i32)>>,
    /// Worn hennas per class index (G17) — dyes are per-subclass in Java too.
    pub hennas_by_index: std::collections::HashMap<i32, Vec<(i32, i32)>>,
    /// Shortcut bars per class index (G17).
    pub shortcuts_by_index: std::collections::HashMap<i32, Vec<crate::model::shortcut::Shortcut>>,
    /// `character_hennas` rows: (slot 1-3, dye/symbol id).
    pub hennas: Vec<(i32, i32)>,
    /// `character_recipebook` rows: recipe-list ids (dwarven/common split
    /// derived from `RecipeData` at `PlayerData::from_char`).
    pub recipe_book: Vec<i32>,
    /// `character_variables` rows: (var, val) — the `PlayerVariables` store.
    pub variables: Vec<(String, String)>,
    /// `pets` rows owned by this character — the `PlayerPets` store, keyed by
    /// collar object id.
    pub pets: Vec<crate::db::PetRow>,
    /// `character_summons` rows — the servitor that was out at logout.
    pub summons: Vec<crate::db::SummonRow>,
    /// `character_shortcuts` rows (class_index 0).
    pub shortcuts: Vec<crate::model::shortcut::Shortcut>,
    /// `character_macroses` rows, commands already decoded.
    pub macros: Vec<crate::model::shortcut::Macro>,
    /// `character_friends` joined with each friend's character row.
    pub friends: Vec<FriendInfo>,
    /// `character_quests` rows grouped by quest name (Java
    /// `Quest.playerEnter`); only quests with a `<state>` row count.
    pub quests: std::collections::HashMap<String, crate::model::quest::QuestState>,
    /// `character_skills_save` reuse rows (Java `restoreEffects`, skill-reuse
    /// half). Non-expired only; converted to live `Reuses` at enter-world.
    pub skill_reuses: Vec<crate::db::SkillReuseRow>,
    /// `character_skills_save` buff rows (Java `restoreEffects`, buff half).
    /// Re-applied as live `Buffs` once the character has spawned at
    /// enter-world, each with its stored remaining time.
    pub skill_buffs: Vec<crate::db::SkillBuffRow>,
}
