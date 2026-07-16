//! Static data loaders — ports of `data/xml/*`, reading the existing
//! `dist/game/data` XML. Added per milestone; G3 covers the character-creation
//! set (experience table + player templates).

pub mod action_data;
pub mod admin_data;
pub mod buy_list_data;
pub mod category_data;
pub mod cursed_weapon_data;
pub mod door_data;
pub mod siege_data;
pub mod enchant_data;
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
pub mod static_object_data;
pub mod teleporter_data;
pub mod transform_data;
pub mod variation_data;
pub mod xp_lost;
pub mod zone_data;

pub use action_data::ActionData;
pub use admin_data::AdminData;
pub use buy_list_data::BuyListData;
pub use category_data::CategoryData;
pub use cursed_weapon_data::CursedWeaponData;
pub use door_data::DoorData;
pub use enchant_data::EnchantData;
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
pub use static_object_data::StaticObjectData;
pub use teleporter_data::TeleporterData;
pub use transform_data::TransformData;
pub use variation_data::VariationData;
pub use xp_lost::PlayerXpPercentLostData;
pub use zone_data::ZoneData;

/// The static game data bundle owned by the game thread (Java: the swarm of
/// `*Data.getInstance()` singletons, here a plain struct — decision #4).
/// `Character.ini` stat ceilings + the flat run-speed boost — the tuning values
/// the stat finalizers clamp/offset with (`MaxPAtk`/`MaxEvasion`/…,
/// `RunSpeedBoost`). Carried on `GameData` so the stat engine
/// (`Player::recalculate_stats`, `recompute_npc_stats_from_buffs`) reads them
/// without threading `CharacterConfig` through the whole pipeline. Defaults are
/// this dist's Character.ini values (which the finalizers used to hardcode);
/// production overwrites them from the parsed config at boot (`main.rs`), tests
/// keep the defaults.
#[derive(Debug, Clone, Copy)]
pub struct CombatCaps {
    pub run_spd_boost: f64,
    pub max_p_atk: f64,
    pub max_m_atk: f64,
    pub max_p_crit_rate: f64,
    pub max_m_crit_rate: f64,
    pub max_p_atk_speed: f64,
    pub max_m_atk_speed: f64,
    pub max_evasion: f64,
    pub max_run_speed: f64,
}

impl Default for CombatCaps {
    fn default() -> Self {
        Self {
            run_spd_boost: 35.0,
            max_p_atk: 999_999.0,
            max_m_atk: 999_999.0,
            max_p_crit_rate: 500.0,
            max_m_crit_rate: 200.0,
            max_p_atk_speed: 1500.0,
            max_m_atk_speed: 1999.0,
            max_evasion: 250.0,
            max_run_speed: 300.0,
        }
    }
}

/// GM login-state / hero-aura settings from `General.ini` (`Config.GM_*`).
/// Carried on `GameData` the same way as [`CombatCaps`] so the enter-world
/// flow and packet builders (`Player::from_char` hero aura, `UserInfo`) read
/// them off `world.data` without threading `GeneralConfig` through the
/// pipeline. Defaults are the Java `Config` fallbacks (all `false`);
/// production overwrites them from the parsed config at boot (`main.rs`),
/// tests keep the defaults.
#[derive(Debug, Clone, Copy, Default)]
pub struct GmSettings {
    pub hero_aura: bool,
    pub startup_builder_hide: bool,
    pub startup_invulnerable: bool,
    pub startup_invisible: bool,
    pub startup_silence: bool,
    pub startup_auto_list: bool,
    pub startup_diet_mode: bool,
}

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
    pub zone_data: ZoneData,
    pub door_data: DoorData,
    pub static_object_data: StaticObjectData,
    pub buy_lists: BuyListData,
    pub categories: CategoryData,
    pub cursed_weapons: CursedWeaponData,
    /// Control/flame tower spawns per castle, from `Siege.ini`.
    pub siege_towers: std::collections::HashMap<i32, Vec<crate::model::siege::SiegeSpawn>>,
    /// Gatekeeper teleport lists (G15.5), see [`TeleporterData`].
    pub teleporters: TeleporterData,
    pub transforms: TransformData,
    /// Enchant chance engine (rate groups + branded scrolls), see [`EnchantData`].
    pub enchant: EnchantData,
    /// Augmentation roll engine (life stone → option pair + fees), see
    /// [`VariationData`].
    pub variations: VariationData,
    /// GM access-level table + per-command access rights (G13).
    pub admin: AdminData,
    /// Stat ceilings + run-speed boost, from `Character.ini` (see [`CombatCaps`]).
    pub combat_caps: CombatCaps,
    /// GM login-state / hero-aura settings, from `General.ini` (see [`GmSettings`]).
    pub gm: GmSettings,
    /// Datapack root prefix (`""` when running from `dist/game`) — for the
    /// odd loose file read at runtime (NPC dialog `.htm`s, which Java streams
    /// through `HtmCache` rather than a boot-time loader).
    pub root: String,
}

impl GameData {
    pub fn load_from(file_path: &str) -> Self {
        // Buy lists read item reference prices (`CorrectPrices`), so items
        // load first.
        let item_data = ItemData::load_from(file_path);
        let buy_lists = BuyListData::load_from(file_path, &item_data);
        Self {
            root: file_path.to_string(),
            experience: ExperienceData::load_from(file_path),
            player_templates: PlayerTemplateData::load_from(file_path),
            skill_trees: SkillTreeData::load_from(file_path),
            stat_bonus: StatBonus::load_from(file_path),
            action_data: ActionData::load_from(file_path),
            item_data,
            initial_equipment: InitialEquipmentData::load_from(file_path),
            initial_shortcuts: InitialShortcutData::load_from(file_path),
            skill_data: SkillData::load_from(file_path),
            npc_data: NpcData::load_from(file_path),
            spawn_data: SpawnData::load_from(file_path),
            hit_condition_bonus: HitConditionBonusData::load_from(file_path),
            xp_lost: PlayerXpPercentLostData::load_from(file_path),
            map_region: MapRegionData::load_from(file_path),
            zone_data: ZoneData::load_from(file_path),
            door_data: DoorData::load_from(file_path),
            static_object_data: StaticObjectData::load_from(file_path),
            buy_lists,
            categories: CategoryData::load_from(file_path),
            cursed_weapons: CursedWeaponData::load_from(file_path),
            siege_towers: siege_data::load_siege_towers(file_path),
            teleporters: TeleporterData::load_from(file_path),
            transforms: TransformData::load_from(file_path),
            enchant: EnchantData::load_from(file_path),
            variations: VariationData::load_from(file_path),
            admin: AdminData::load_from(file_path),
            // Overwritten from the parsed `CharacterConfig` at boot (`main.rs`);
            // the default is this dist's Character.ini values.
            combat_caps: CombatCaps::default(),
            // Overwritten from the parsed `GeneralConfig` at boot (`main.rs`).
            gm: GmSettings::default(),
        }
    }
    pub fn load() -> Self {
        Self::load_from("")
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
            zone_data: ZoneData::empty(),
            door_data: DoorData::empty(),
            static_object_data: StaticObjectData::empty(),
            buy_lists: BuyListData::empty(),
            categories: CategoryData::empty(),
            cursed_weapons: CursedWeaponData::empty(),
            siege_towers: std::collections::HashMap::new(),
            teleporters: TeleporterData::empty(),
            transforms: TransformData::empty(),
            enchant: EnchantData::empty(),
            variations: VariationData::empty(),
            admin: AdminData::empty(),
            combat_caps: CombatCaps::default(),
            gm: GmSettings::default(),
        }
    }
}
