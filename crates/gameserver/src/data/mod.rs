//! Static data loaders — ports of `data/xml/*`, reading the existing
//! `dist/game/data` XML. Added per milestone; G3 covers the character-creation
//! set (experience table + player templates).

pub mod experience;
pub mod player_template;

pub use experience::ExperienceData;
pub use player_template::PlayerTemplateData;

/// The static game data bundle owned by the game thread (Java: the swarm of
/// `*Data.getInstance()` singletons, here a plain struct — decision #4).
pub struct GameData {
    pub experience: ExperienceData,
    pub player_templates: PlayerTemplateData,
}

impl GameData {
    pub fn load() -> Self {
        Self { experience: ExperienceData::load(), player_templates: PlayerTemplateData::load() }
    }

    /// Empty data bundle for tests that don't exercise the loaders.
    #[doc(hidden)]
    pub fn for_test() -> Self {
        Self { experience: ExperienceData::empty(), player_templates: PlayerTemplateData::empty() }
    }
}
