//! Static data loaders — ports of `data/xml/*`, reading the existing
//! `dist/game/data` XML. Added per milestone; G3 covers the character-creation
//! set (experience table + player templates).

pub mod action_data;
pub mod admin_data;
pub mod armor_set_data;
pub mod buy_list_data;
pub mod castle_siege_guards;
pub mod castle_zone_data;
pub mod category_data;
pub mod clan_hall_data;
pub mod cubic_data;
pub mod cursed_weapon_data;
/// The real datapack, loaded once per test process — the fixture every test
/// that wants dist content should go through.
#[cfg(test)]
pub(crate) mod dist;
pub mod door_data;
pub mod enchant_data;
pub mod enchant_skill_groups;
pub mod experience;
pub mod fishing_data;
pub mod four_sepulchers_data;
pub mod henna_data;
pub mod hit_condition_bonus;
pub mod htm_cache;
pub mod initial_equipment;
pub mod initial_shortcut;
pub mod instance_data;
pub mod item_auction_data;
pub mod item_data;
pub mod manor_data;
pub mod map_region;
pub mod multisell_data;
pub mod npc_ai_skills;
pub mod npc_data;
pub mod option_data;
pub mod pet_data;
pub mod player_template;
pub mod pledge_skill_tree;
pub mod recipe_data;
pub mod residence_function_data;
pub mod route_data;
pub mod scheme_buffer;
pub mod sell_buff_data;
pub mod siege_data;
pub mod skill_data;
pub mod skill_expr;
pub mod skill_tree;
/// The test fixtures' binary cache of the parsed catalogues. Test-only: the
/// server parses the datapack it was pointed at, every boot.
#[cfg(test)]
pub(crate) mod snapshot;
pub mod soul_crystal_data;
pub mod spawn_data;
pub mod stat_bonus;
pub mod static_object_data;
pub mod teleporter_data;
pub mod transform_data;
pub mod variation_data;
pub mod xml;
pub mod xp_lost;
pub mod zone_data;

pub use action_data::ActionData;
pub use admin_data::AdminData;
pub use buy_list_data::BuyListData;
pub use category_data::CategoryData;
pub use cubic_data::CubicData;
pub use cursed_weapon_data::CursedWeaponData;
pub use door_data::DoorData;
pub use enchant_data::EnchantData;
pub use enchant_skill_groups::EnchantSkillGroups;
pub use experience::ExperienceData;
pub use fishing_data::FishingData;
pub use four_sepulchers_data::FourSepulchersData;
pub use henna_data::HennaData;
pub use hit_condition_bonus::HitConditionBonusData;
pub use initial_equipment::InitialEquipmentData;
pub use initial_shortcut::InitialShortcutData;
pub use item_data::ItemData;
pub use map_region::MapRegionData;
pub use multisell_data::MultisellData;
pub use npc_ai_skills::{AiSkillScope, NpcAiSkillIndex, NpcAiSkills};
pub use npc_data::NpcData;
pub use option_data::OptionData;
pub use player_template::PlayerTemplateData;
pub use pledge_skill_tree::PledgeSkillTreeData;
pub use recipe_data::RecipeData;
pub use route_data::RouteData;
pub use scheme_buffer::SchemeBufferData;
pub use skill_data::SkillData;
pub use skill_tree::SkillTreeData;
pub use soul_crystal_data::SoulCrystalData;
pub use spawn_data::SpawnData;
pub use stat_bonus::StatBonus;
pub use static_object_data::StaticObjectData;
pub use teleporter_data::TeleporterData;
pub use transform_data::TransformData;
pub use variation_data::VariationData;
pub use xp_lost::PlayerXpPercentLostData;
pub use zone_data::ZoneData;

/// The in-repo datapack root, for the tests that load the real XML rather than
/// a synthetic fixture.
///
/// `CARGO_MANIFEST_DIR` resolves at compile time, so this holds regardless of
/// which directory the test binary is run from — and unlike the `DATAPACK_ROOT`
/// override, it cannot be pointed somewhere else, which is the property the
/// loader tests need. Integration tests and the `tools` crate spell their own
/// out: `CARGO_MANIFEST_DIR` is per-crate, so this exact relative path only
/// means "the repo's datapack" from inside `crates/gameserver`.
#[cfg(test)]
pub(crate) const DIST_GAME: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

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
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
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
    /// `MaxBuffAmount` (24) — good-buff slot cap, see [`crate::config::character::CharacterConfig::max_buff_count`].
    pub max_buff_count: i32,
    /// `MaxDanceAmount` (12) — dance/song slot cap.
    pub max_dance_count: i32,
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
            max_buff_count: 24,
            max_dance_count: 12,
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
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct GmSettings {
    pub hero_aura: bool,
    pub startup_builder_hide: bool,
    pub startup_invulnerable: bool,
    pub startup_invisible: bool,
    pub startup_silence: bool,
    pub startup_auto_list: bool,
    pub startup_diet_mode: bool,
    /// `GMGiveSpecialSkills` / `GMGiveSpecialAuraSkills` — hand a GM the
    /// convenience kits at enter-world. Session-only: Java grants them with
    /// `addSkill(skill, false)`.
    pub give_special_skills: bool,
    pub give_special_aura_skills: bool,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct GameData {
    pub experience: ExperienceData,
    pub player_templates: PlayerTemplateData,
    pub skill_trees: SkillTreeData,
    /// `SellBuffData.xml` — the skills a player buff shop may list.
    pub sell_buff_data: sell_buff_data::SellBuffData,
    /// Clan (pledge/sub-pledge) skill trees — see [`PledgeSkillTreeData`].
    pub pledge_skill_trees: PledgeSkillTreeData,
    pub stat_bonus: StatBonus,
    pub action_data: ActionData,
    pub item_data: ItemData,
    pub initial_equipment: InitialEquipmentData,
    pub initial_shortcuts: InitialShortcutData,
    pub skill_data: SkillData,
    pub npc_data: NpcData,
    pub cubic_data: CubicData,
    pub fishing_data: FishingData,
    pub soul_crystal_data: SoulCrystalData,
    pub four_sepulchers: FourSepulchersData,
    /// Pet templates (collar → npc, food, hunger) — see [`pet_data::PetData`].
    pub pet_data: pet_data::PetData,
    /// Per-template AI skill buckets — see [`NpcAiSkillIndex`].
    pub npc_ai_skills: NpcAiSkillIndex,
    pub spawn_data: SpawnData,
    pub hit_condition_bonus: HitConditionBonusData,
    pub xp_lost: PlayerXpPercentLostData,
    pub map_region: MapRegionData,
    pub zone_data: ZoneData,
    pub door_data: DoorData,
    pub static_object_data: StaticObjectData,
    pub buy_lists: BuyListData,
    /// Multisell exchange lists (`data/multisell/*`, incl. the custom CB shop
    /// lists), see [`MultisellData`].
    pub multisells: MultisellData,

    /// Instance templates (`data/instances/**/*.xml`), see [`instance_data`].
    pub instance_templates: instance_data::InstanceData,
    /// Item-auction auctioneer instances (`ItemAuctions.xml`, empty on this
    /// dist), see [`item_auction_data::ItemAuctionData`].
    pub item_auctions: item_auction_data::ItemAuctionData,
    /// Community-board scheme buffer available-buff table (`_availableBuffs`),
    /// see [`SchemeBufferData`].
    pub scheme_buffer: SchemeBufferData,
    pub hennas: HennaData,
    /// The castle-manor seed catalogue (`Seeds.xml`), see [`manor_data::ManorData`].
    pub manor: manor_data::ManorData,
    pub recipes: RecipeData,
    /// NPC walking routes (`Routes.xml`), see [`RouteData`].
    pub routes: RouteData,
    pub categories: CategoryData,
    pub cursed_weapons: CursedWeaponData,
    /// Control/flame tower spawns per castle, from `Siege.ini`.
    pub siege_towers: std::collections::HashMap<i32, Vec<crate::model::siege::SiegeSpawn>>,
    /// The weekly per-castle siege calendar (`config/SiegeSchedule.xml`).
    pub siege_schedule: std::collections::HashMap<i32, siege_data::SiegeScheduleEntry>,
    /// Castle residence respawn points per castle, from `castle_hall.xml` — where
    /// defenders respawn while the control towers stand.
    pub castle_restart_points:
        std::collections::HashMap<i32, castle_zone_data::CastleRespawnPoints>,
    /// The 48 clan-hall definitions (`data/residences/clanHalls/**`), keyed by id.
    /// Static defs only; ownership is overlaid onto `World::clan_halls` at boot.
    pub clan_halls: std::collections::HashMap<i32, crate::model::clan_hall::ClanHall>,
    /// Mercenary posting tickets per castle (`data/residences/castles/*.xml`
    /// `<siegeGuards>`) — read by `ItemAction`'s pickup refusal.
    pub castle_siege_guards: castle_siege_guards::CastleSiegeGuards,
    /// The clan-hall function upgrade catalogue (`data/ResidenceFunctions.xml`).
    pub residence_functions: residence_function_data::ResidenceFunctionData,
    /// Gatekeeper teleport lists (G15.5), see [`TeleporterData`].
    pub teleporters: TeleporterData,
    pub transforms: TransformData,
    pub armor_sets: armor_set_data::ArmorSetData,
    /// Enchant chance engine (rate groups + branded scrolls), see [`EnchantData`].
    pub enchant: EnchantData,
    /// Skill-enchant cost table — see [`EnchantSkillGroups`].
    pub enchant_skill_groups: EnchantSkillGroups,
    /// Augmentation roll engine (life stone → option pair + fees), see
    /// [`VariationData`].
    pub variations: VariationData,
    /// Augment option bonuses (`data/stats/augmentation/options`).
    pub options: OptionData,
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
        let multisells = MultisellData::load_from(file_path, &item_data);
        let instance_templates = instance_data::InstanceData::load_from(file_path);
        let item_auctions = item_auction_data::ItemAuctionData::load_from(file_path);
        // The NPC AI skill index buckets each template's *active* skills by
        // what the AI would use them for, so it needs both loaders done first
        // (Java does the same bucketing inline at the end of `NpcData.parse`).
        let skill_data = SkillData::load_from(file_path);
        let npc_data = NpcData::load_from(file_path);
        let cubic_data = CubicData::load_from(file_path);
        let fishing_data = FishingData::load_from(file_path);
        let soul_crystal_data = SoulCrystalData::load_from(file_path);
        let four_sepulchers = FourSepulchersData::load_from(file_path);
        let npc_ai_skills = NpcAiSkillIndex::build(&npc_data, &skill_data);
        let data = Self {
            root: file_path.to_string(),
            experience: ExperienceData::load_from(file_path),
            player_templates: PlayerTemplateData::load_from(file_path),
            skill_trees: SkillTreeData::load_from(file_path),
            sell_buff_data: sell_buff_data::SellBuffData::load_from(file_path),
            pledge_skill_trees: PledgeSkillTreeData::load_from(file_path),
            stat_bonus: StatBonus::load_from(file_path),
            action_data: ActionData::load_from(file_path),
            item_data,
            initial_equipment: InitialEquipmentData::load_from(file_path),
            initial_shortcuts: InitialShortcutData::load_from(file_path),
            skill_data,
            npc_data,
            cubic_data,
            fishing_data,
            soul_crystal_data,
            four_sepulchers,
            pet_data: pet_data::PetData::load_from(file_path),
            npc_ai_skills,
            spawn_data: SpawnData::load_from(file_path),
            hit_condition_bonus: HitConditionBonusData::load_from(file_path),
            xp_lost: PlayerXpPercentLostData::load_from(file_path),
            map_region: MapRegionData::load_from(file_path),
            zone_data: ZoneData::load_from(file_path),
            door_data: DoorData::load_from(file_path),
            static_object_data: StaticObjectData::load_from(file_path),
            buy_lists,
            multisells,
            instance_templates,
            item_auctions,
            scheme_buffer: SchemeBufferData::load_from(file_path),
            hennas: HennaData::load_from(file_path),
            manor: manor_data::ManorData::load_from(file_path),
            recipes: RecipeData::load_from(file_path),
            routes: RouteData::load_from(file_path),
            categories: CategoryData::load_from(file_path),
            cursed_weapons: CursedWeaponData::load_from(file_path),
            siege_towers: siege_data::load_siege_towers(file_path),
            siege_schedule: siege_data::load_siege_schedule(file_path),
            castle_restart_points: castle_zone_data::load_castle_restart_points(file_path),
            clan_halls: clan_hall_data::load_clan_halls(file_path),
            castle_siege_guards: castle_siege_guards::CastleSiegeGuards::load_from(file_path),
            residence_functions: residence_function_data::ResidenceFunctionData::load_from(
                file_path,
            ),
            teleporters: TeleporterData::load_from(file_path),
            transforms: TransformData::load_from(file_path),
            armor_sets: armor_set_data::ArmorSetData::load_from(file_path),
            enchant: EnchantData::load_from(file_path),
            enchant_skill_groups: EnchantSkillGroups::load_from(file_path),
            variations: VariationData::load_from(file_path),
            options: OptionData::load_from(file_path),
            admin: AdminData::load_from(file_path),
            // Overwritten from the parsed `CharacterConfig` at boot (`main.rs`);
            // the default is this dist's Character.ini values.
            combat_caps: CombatCaps::default(),
            // Overwritten from the parsed `GeneralConfig` at boot (`main.rs`).
            gm: GmSettings::default(),
        };
        // Deferred to here, not `SkillData::load_from`: separating a parser gap
        // that touches a skill a player can learn from one that only touches
        // later-chronicle datapack content needs the skill trees, and this is
        // the first point at which they exist.
        skill_data::parse::log_gaps(
            data.skill_data.gaps(),
            &data.skill_trees.all_learnable_skill_ids(),
        );
        data
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
            sell_buff_data: sell_buff_data::SellBuffData::default(),
            pledge_skill_trees: PledgeSkillTreeData::empty(),
            stat_bonus: StatBonus::empty(),
            action_data: ActionData::empty(),
            item_data: ItemData::empty(),
            initial_equipment: InitialEquipmentData::empty(),
            initial_shortcuts: InitialShortcutData::empty(),
            skill_data: SkillData::empty(),
            npc_data: NpcData::empty(),
            cubic_data: CubicData::empty(),
            fishing_data: FishingData::empty(),
            soul_crystal_data: SoulCrystalData::empty(),
            four_sepulchers: FourSepulchersData::empty(),
            pet_data: Default::default(),
            npc_ai_skills: NpcAiSkillIndex::default(),
            spawn_data: SpawnData::empty(),
            hit_condition_bonus: HitConditionBonusData::default(),
            xp_lost: PlayerXpPercentLostData::empty(),
            map_region: MapRegionData::empty(),
            zone_data: ZoneData::empty(),
            door_data: DoorData::empty(),
            static_object_data: StaticObjectData::empty(),
            buy_lists: BuyListData::empty(),
            multisells: MultisellData::empty(),
            instance_templates: instance_data::InstanceData::empty(),
            item_auctions: item_auction_data::ItemAuctionData::empty(),
            scheme_buffer: SchemeBufferData::default(),
            hennas: HennaData::empty(),
            manor: manor_data::ManorData::empty(),
            recipes: RecipeData::empty(),
            routes: RouteData::default(),
            categories: CategoryData::empty(),
            cursed_weapons: CursedWeaponData::empty(),
            siege_towers: std::collections::HashMap::new(),
            siege_schedule: std::collections::HashMap::new(),
            castle_restart_points: std::collections::HashMap::new(),
            clan_halls: std::collections::HashMap::new(),
            castle_siege_guards: castle_siege_guards::CastleSiegeGuards::empty(),
            residence_functions: residence_function_data::ResidenceFunctionData::default(),
            teleporters: TeleporterData::empty(),
            transforms: TransformData::empty(),
            armor_sets: armor_set_data::ArmorSetData::empty(),
            enchant: EnchantData::empty(),
            enchant_skill_groups: EnchantSkillGroups::empty(),
            variations: VariationData::empty(),
            options: OptionData::empty(),
            admin: AdminData::empty(),
            combat_caps: CombatCaps::default(),
            gm: GmSettings::default(),
        }
    }
}
