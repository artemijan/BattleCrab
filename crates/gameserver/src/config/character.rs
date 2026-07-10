//! `Character.ini` — port of the `CHARACTER_CONFIG_FILE` block of `Config.java`.
//! Only the keys needed so far are loaded (grown per milestone).

use commons::config::PropertiesParser;

pub const CHARACTER_CONFIG_FILE: &str = "config/Character.ini";

pub struct CharacterConfig {
    /// `DeleteCharAfterDays`: 0 = delete immediately, else mark with a timer.
    pub delete_days: i32,
}

impl CharacterConfig {
    pub fn load() -> Self {
        let p = PropertiesParser::load(CHARACTER_CONFIG_FILE);
        Self { delete_days: p.get_int("DeleteCharAfterDays", 1) }
    }
}
