//! Static data loaders — ports of `data/xml/*`, reading the existing
//! `dist/game/data` XML. Added per milestone; G3 covers the character-creation
//! set (experience table + player templates).

pub mod action_data;
pub mod experience;
pub mod hit_condition_bonus;
pub mod initial_equipment;
pub mod initial_shortcut;
pub mod item_data;
pub mod map_region;
pub mod npc_data;
pub mod player_template;
pub mod skill_data;
pub mod skill_tree;
pub mod spawn_data;
pub mod stat_bonus;
pub mod xp_lost;

pub use action_data::ActionData;
pub use experience::ExperienceData;
pub use hit_condition_bonus::HitConditionBonusData;
pub use initial_equipment::InitialEquipmentData;
pub use initial_shortcut::InitialShortcutData;
pub use item_data::ItemData;
pub use map_region::MapRegionData;
pub use npc_data::NpcData;
pub use player_template::PlayerTemplateData;
pub use skill_data::SkillData;
pub use skill_tree::SkillTreeData;
pub use spawn_data::SpawnData;
pub use stat_bonus::StatBonus;
pub use xp_lost::PlayerXpPercentLostData;

/// The static game data bundle owned by the game thread (Java: the swarm of
/// `*Data.getInstance()` singletons, here a plain struct — decision #4).
pub struct GameData {
    pub experience: ExperienceData,
    pub player_templates: PlayerTemplateData,
    pub skill_trees: SkillTreeData,
    pub stat_bonus: StatBonus,
    pub action_data: ActionData,
    pub item_data: ItemData,
    pub initial_equipment: InitialEquipmentData,
    pub initial_shortcuts: InitialShortcutData,
    pub skill_data: SkillData,
    pub npc_data: NpcData,
    pub spawn_data: SpawnData,
    pub hit_condition_bonus: HitConditionBonusData,
    pub xp_lost: PlayerXpPercentLostData,
    pub map_region: MapRegionData,
    /// Datapack root prefix (`""` when running from `dist/game`) — for the
    /// odd loose file read at runtime (NPC dialog `.htm`s, which Java streams
    /// through `HtmCache` rather than a boot-time loader).
    pub root: String,
}

impl GameData {
    pub fn load_from(file_path: &str) -> Self {
        Self {
            root: file_path.to_string(),
            experience: ExperienceData::load_from(file_path),
            player_templates: PlayerTemplateData::load_from(file_path),
            skill_trees: SkillTreeData::load_from(file_path),
            stat_bonus: StatBonus::load_from(file_path),
            action_data: ActionData::load_from(file_path),
            item_data: ItemData::load_from(file_path),
            initial_equipment: InitialEquipmentData::load_from(file_path),
            initial_shortcuts: InitialShortcutData::load_from(file_path),
            skill_data: SkillData::load_from(file_path),
            npc_data: NpcData::load_from(file_path),
            spawn_data: SpawnData::load_from(file_path),
            hit_condition_bonus: HitConditionBonusData::load_from(file_path),
            xp_lost: PlayerXpPercentLostData::load_from(file_path),
            map_region: MapRegionData::load_from(file_path),
        }
    }
    pub fn load() -> Self {
        Self {
            root: String::new(),
            experience: ExperienceData::load(),
            player_templates: PlayerTemplateData::load(),
            skill_trees: SkillTreeData::load(),
            stat_bonus: StatBonus::load(),
            action_data: ActionData::load(),
            item_data: ItemData::load(),
            initial_equipment: InitialEquipmentData::load(),
            initial_shortcuts: InitialShortcutData::load(),
            skill_data: SkillData::load(),
            npc_data: NpcData::load(),
            spawn_data: SpawnData::load(),
            hit_condition_bonus: HitConditionBonusData::load(),
            xp_lost: PlayerXpPercentLostData::load(),
            map_region: MapRegionData::load(),
        }
    }

    /// Empty data bundle for tests that don't exercise the loaders.
    #[doc(hidden)]
    #[cfg(test)]
    pub fn for_test() -> Self {
        Self {
            root: String::new(),
            experience: ExperienceData::empty(),
            player_templates: PlayerTemplateData::empty(),
            skill_trees: SkillTreeData::empty(),
            stat_bonus: StatBonus::empty(),
            action_data: ActionData::empty(),
            item_data: ItemData::empty(),
            initial_equipment: InitialEquipmentData::empty(),
            initial_shortcuts: InitialShortcutData::empty(),
            skill_data: SkillData::empty(),
            npc_data: NpcData::empty(),
            spawn_data: SpawnData::empty(),
            hit_condition_bonus: HitConditionBonusData::default(),
            xp_lost: PlayerXpPercentLostData::empty(),
            map_region: MapRegionData::empty(),
        }
    }
}
