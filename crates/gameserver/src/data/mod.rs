//! Static data loaders — ports of `data/xml/*`, reading the existing
//! `dist/game/data` XML. Added per milestone; G3 covers the character-creation
//! set (experience table + player templates).

pub mod action_data;
pub mod experience;
pub mod player_template;
pub mod skill_tree;
pub mod stat_bonus;

pub use action_data::ActionData;
pub use experience::ExperienceData;
pub use player_template::PlayerTemplateData;
pub use skill_tree::SkillTreeData;
pub use stat_bonus::StatBonus;

/// The static game data bundle owned by the game thread (Java: the swarm of
/// `*Data.getInstance()` singletons, here a plain struct — decision #4).
pub struct GameData {
    pub experience: ExperienceData,
    pub player_templates: PlayerTemplateData,
    pub skill_trees: SkillTreeData,
    pub stat_bonus: StatBonus,
    pub action_data: ActionData,
}

impl GameData {
    pub fn load_from(file_path: &str) -> Self {
        Self {
            experience: ExperienceData::load_from(file_path),
            player_templates: PlayerTemplateData::load_from(file_path),
            skill_trees: SkillTreeData::load_from(file_path),
            stat_bonus: StatBonus::load_from(file_path),
            action_data: ActionData::load_from(file_path),
        }
    }
    pub fn load() -> Self {
        Self {
            experience: ExperienceData::load(),
            player_templates: PlayerTemplateData::load(),
            skill_trees: SkillTreeData::load(),
            stat_bonus: StatBonus::load(),
            action_data: ActionData::load(),
        }
    }

    /// Empty data bundle for tests that don't exercise the loaders.
    #[doc(hidden)]
    #[cfg(test)]
    pub fn for_test() -> Self {
        Self {
            experience: ExperienceData::empty(),
            player_templates: PlayerTemplateData::empty(),
            skill_trees: SkillTreeData::empty(),
            stat_bonus: StatBonus::empty(),
            action_data: ActionData::empty(),
        }
    }
}
