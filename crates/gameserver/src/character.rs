//! A stored character row, as loaded for the selection screen
//! (`CharSelectInfoPackage`) and cached by the `InLobby` session for slot →
//! object-id mapping. Paperdoll/augmentation are added with the inventory system
//! (G6); for now equipment shows empty on the selection screen.

/// One row of the `characters` table, restored for character selection.
#[derive(Debug, Clone)]
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
    pub clan_id: i32,
    pub race: i32,
    pub class_id: i32,
    pub base_class_id: i32,
    pub delete_time: i64,
    pub last_access: i64,
    pub vitality_points: i32,
    pub access_level: i32,
    pub noble: bool,
    pub char_slot: i32,
}
