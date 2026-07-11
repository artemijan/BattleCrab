//! Static data loaders — ports of `data/xml/*`, reading the existing
//! `dist/game/data` XML. Added per milestone; G3 covers the character-creation
//! set (experience table + player templates).

pub mod action_data;
pub mod experience;
pub mod initial_equipment;
pub mod item_data;
pub mod npc_data;
pub mod player_template;
pub mod skill_data;
pub mod skill_tree;
pub mod spawn_data;
pub mod stat_bonus;

pub use action_data::ActionData;
pub use experience::ExperienceData;
pub use initial_equipment::InitialEquipmentData;
pub use item_data::ItemData;
pub use npc_data::NpcData;
pub use player_template::PlayerTemplateData;
pub use skill_data::SkillData;
pub use skill_tree::SkillTreeData;
pub use spawn_data::SpawnData;
pub use stat_bonus::StatBonus;

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
    pub skill_data: SkillData,
    pub npc_data: NpcData,
    pub spawn_data: SpawnData,
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
            skill_data: SkillData::load_from(file_path),
            npc_data: NpcData::load_from(file_path),
            spawn_data: SpawnData::load_from(file_path),
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
            skill_data: SkillData::load(),
            npc_data: NpcData::load(),
            spawn_data: SpawnData::load(),
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
            skill_data: SkillData::empty(),
            npc_data: NpcData::empty(),
            spawn_data: SpawnData::empty(),
        }
    }
}
