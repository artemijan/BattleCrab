//! Port of `data/xml/ItemData` + `util/DocumentItem`, scoped to what G5 needs:
//! identity, equip slot, weight, stackability, and the `type1`/`type2` pair the
//! client needs for `ItemList` sorting/icons, plus (for `UseItem`'s `EtcItem`
//! branch) the `handler`/`<capsuled_items>` pair `ExtractableItems` reads, plus
//! the combat-stat bonuses under `<stats>` (Java `ItemTemplate._funcTemplates`)
//! the stats engine folds in when the item is equipped ([`ItemStats`], applied
//! by `Player::recalculate_stats`), plus the `<cond>` blocks
//! ([`crate::data::item_cond`]) that gate whether the item may be equipped or
//! used at all — Java `ItemTemplate._preConditions`, evaluated by
//! [`crate::game_loop::items::conditions`].

pub mod kinds;
pub mod parse;
pub mod template;

use crate::data::xml;
#[cfg(test)]
use crate::model::stats::Stat;
use kinds::{ArmorType, WeaponType};
use parse::parse_file;
use std::collections::HashMap;
use template::{ItemStats, ItemTemplate};
use tracing::info;

pub const ITEMS_DIR: &str = "data/stats/items";

/// `Inventory.ADENA_ID` / `ANCIENT_ADENA_ID`.
pub const ADENA_ID: i32 = 57;
pub(super) const ANCIENT_ADENA_ID: i32 = 5575;

// `ItemTemplate.SLOT_*` (bodypart bitmask constants).
pub const SLOT_NONE: i32 = 0x0000;
pub const SLOT_UNDERWEAR: i32 = 0x0001;
pub const SLOT_R_EAR: i32 = 0x0002;
pub const SLOT_L_EAR: i32 = 0x0004;
/// Combined `rear;lear` bodypart value (both bits set) — the dual-slot ear
/// items actually carry this, not the single `SLOT_*_EAR` bits.
pub const SLOT_LR_EAR: i32 = 0x0006;
pub const SLOT_NECK: i32 = 0x0008;
pub const SLOT_R_FINGER: i32 = 0x0010;
pub const SLOT_L_FINGER: i32 = 0x0020;
/// Combined `rfinger;lfinger` bodypart value, see `SLOT_LR_EAR`.
pub const SLOT_LR_FINGER: i32 = 0x0030;
pub const SLOT_HEAD: i32 = 0x0040;
pub const SLOT_R_HAND: i32 = 0x0080;
pub const SLOT_L_HAND: i32 = 0x0100;
pub const SLOT_GLOVES: i32 = 0x0200;
pub const SLOT_CHEST: i32 = 0x0400;
pub const SLOT_LEGS: i32 = 0x0800;
pub const SLOT_FEET: i32 = 0x1000;
pub const SLOT_BACK: i32 = 0x2000;
pub const SLOT_LR_HAND: i32 = 0x4000;
pub const SLOT_FULL_ARMOR: i32 = 0x8000;
pub const SLOT_HAIR: i32 = 0x010000;
pub const SLOT_ALLDRESS: i32 = 0x020000;
pub const SLOT_HAIR2: i32 = 0x040000;
pub const SLOT_HAIRALL: i32 = 0x080000;
pub const SLOT_R_BRACELET: i32 = 0x100000;
pub const SLOT_L_BRACELET: i32 = 0x200000;
pub const SLOT_DECO: i32 = 0x400000;
pub const SLOT_BELT: i32 = 0x1000_0000_u32 as i32;
pub const SLOT_BROOCH: i32 = 0x2000_0000_u32 as i32;
pub const SLOT_BROOCH_JEWEL: i32 = 0x4000_0000_u32 as i32;
pub const SLOT_WOLF: i32 = -100;
pub const SLOT_HATCHLING: i32 = -101;
pub const SLOT_STRIDER: i32 = -102;
pub const SLOT_BABYPET: i32 = -103;
pub const SLOT_GREATWOLF: i32 = -104;

// `ItemTemplate.TYPE1_*` / `TYPE2_*`.
pub const TYPE1_WEAPON_RING_EARRING_NECKLACE: i32 = 0;
pub const TYPE1_SHIELD_ARMOR: i32 = 1;
pub const TYPE1_ITEM_QUESTITEM_ADENA: i32 = 4;
pub const TYPE2_WEAPON: i32 = 0;
pub const TYPE2_SHIELD_ARMOR: i32 = 1;
pub const TYPE2_ACCESSORY: i32 = 2;
pub const TYPE2_QUEST: i32 = 3;
pub const TYPE2_MONEY: i32 = 4;
pub const TYPE2_OTHER: i32 = 5;

/// `ItemData.SLOTS` — the `bodypart` XML attribute string → slot bitmask table.
/// Also serves callers outside this module (the enchant loader resolves
/// `<item slot=…>` strings the same way).
pub(crate) fn slot_mask(name: &str) -> i32 {
    match name {
        "shirt" | "underwear" => SLOT_UNDERWEAR,
        "lbracelet" => SLOT_L_BRACELET,
        "rbracelet" => SLOT_R_BRACELET,
        "talisman" | "deco1" => SLOT_DECO,
        "chest" => SLOT_CHEST,
        "fullarmor" | "onepiece" => SLOT_FULL_ARMOR,
        "head" => SLOT_HEAD,
        "hair" => SLOT_HAIR,
        "hairall" | "dhair" => SLOT_HAIRALL,
        "back" => SLOT_BACK,
        "neck" => SLOT_NECK,
        "legs" => SLOT_LEGS,
        "feet" => SLOT_FEET,
        "gloves" => SLOT_GLOVES,
        "chest,legs" => SLOT_CHEST | SLOT_LEGS,
        "belt" | "waist" => SLOT_BELT,
        "rhand" => SLOT_R_HAND,
        "lhand" => SLOT_L_HAND,
        "lrhand" => SLOT_LR_HAND,
        "rear;lear" => SLOT_R_EAR | SLOT_L_EAR,
        "rfinger;lfinger" => SLOT_R_FINGER | SLOT_L_FINGER,
        "wolf" => SLOT_WOLF,
        "greatwolf" => SLOT_GREATWOLF,
        "hatchling" => SLOT_HATCHLING,
        "strider" => SLOT_STRIDER,
        "babypet" => SLOT_BABYPET,
        "brooch" => SLOT_BROOCH,
        "brooch_jewel" => SLOT_BROOCH_JEWEL,
        "hair2" => SLOT_HAIR2,
        "alldress" => SLOT_ALLDRESS,
        _ => SLOT_NONE,
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ItemData {
    by_id: HashMap<i32, ItemTemplate>,
    /// Parsed `<stats>` blocks, keyed by item id. Sparse: only equipable
    /// items with a non-empty block have an entry.
    stat_bonuses: HashMap<i32, ItemStats>,
    /// `<set name="armor_type"/>` by item id, side-mapped (like `stat_bonuses`)
    /// so the `ItemTemplate` literals stay untouched. Sparse: only items that
    /// declared a non-`None` armor type have an entry — the armor-conditioned
    /// passive check (`ConditionUsingItemType`) reads it for the worn chest/legs.
    armor_types: HashMap<i32, ArmorType>,
    /// `<set name="weapon_type"/>` by item id, side-mapped like `armor_types`.
    /// Sparse: only weapons with a non-`None` type. Read for the equipped weapon
    /// by the weapon-conditioned passive check (skill effect `<weaponType>`).
    weapon_types: HashMap<i32, WeaponType>,
    /// `Weapon._soulShotCount` / `_spiritShotCount` (`<set name="soulshots"/>`
    /// / `<set name="spiritshots"/>`) by weapon item id — how many shots one
    /// charge consumes, and (non-zero ⇒) whether the weapon can use that shot
    /// kind at all. Sparse: only weapons that declared a non-zero count.
    weapon_shots: HashMap<i32, (i32, i32)>,
    /// `<set name="icon"/>` by item id, side-mapped like the others. Read by the
    /// community-board drop search's item-icon buttons (Java `ItemTemplate
    /// .getIcon()`); missing → the question-mark fallback (see [`ItemData::icon`]).
    icons: HashMap<i32, String>,
}

impl ItemData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        Self::load_from_with(file_path, true)
    }

    /// `ItemData.load()` with Java's `Config.CUSTOM_ITEMS_LOAD` branch.
    ///
    /// **This dist ships no `stats/items/custom/` directory**, so the flag is
    /// inert here — wired anyway, because an operator adding custom items is
    /// exactly who would set the key, and a silently-ignored directory is the
    /// hardest kind of missing feature to diagnose.
    pub fn load_from_with(file_path: &str, custom: bool) -> Self {
        let mut by_id = HashMap::new();
        let mut stat_bonuses = HashMap::new();
        let mut armor_types = HashMap::new();
        let mut weapon_types = HashMap::new();
        let mut weapon_shots = HashMap::new();
        let mut icons = HashMap::new();
        let dir = format!("{file_path}{ITEMS_DIR}");
        {
            let mut paths = xml::xml_files_in(&dir);
            if custom {
                paths.extend(xml::xml_files_in(format!("{dir}/custom")));
            }
            for path in paths {
                parse_file(
                    &path,
                    &mut by_id,
                    &mut stat_bonuses,
                    &mut armor_types,
                    &mut weapon_types,
                    &mut weapon_shots,
                    &mut icons,
                );
            }
        }
        info!("ItemData: Loaded {} item templates.", by_id.len());
        Self {
            by_id,
            stat_bonuses,
            armor_types,
            weapon_types,
            weapon_shots,
            icons,
        }
    }

    /// `<set name="icon"/>` of an item, or the client question-mark fallback
    /// (Java `DropSearchBoard`'s `icon == null` default). Never fails.
    pub fn icon(&self, item_id: i32) -> &str {
        self.icons
            .get(&item_id)
            .map(String::as_str)
            .unwrap_or("icon.etc_question_mark_i00")
    }

    /// Every loaded template (Java `ItemData.getAllItems`), unordered.
    pub fn all(&self) -> impl Iterator<Item = &ItemTemplate> {
        self.by_id.values()
    }

    pub fn get(&self, item_id: i32) -> Option<&ItemTemplate> {
        self.by_id.get(&item_id)
    }

    /// The `<stats>` combat bonuses of an equipable item, if it declared any.
    pub fn item_stats(&self, item_id: i32) -> Option<&ItemStats> {
        self.stat_bonuses.get(&item_id)
    }

    /// The item's `<set name="armor_type"/>`, or `ArmorType::None` when
    /// undeclared/unknown (Java's `ArmorType.NONE` default). Weapons/etc items
    /// report `None` — the armor condition only inspects chest/legs armor.
    pub fn armor_type(&self, item_id: i32) -> ArmorType {
        self.armor_types
            .get(&item_id)
            .copied()
            .unwrap_or(ArmorType::None)
    }

    /// The item's `<set name="weapon_type"/>`, or `WeaponType::None` when
    /// undeclared/unknown (non-weapons report `None`). Read for the equipped
    /// weapon by the weapon-conditioned passive check.
    pub fn weapon_type(&self, item_id: i32) -> WeaponType {
        self.weapon_types
            .get(&item_id)
            .copied()
            .unwrap_or(WeaponType::None)
    }

    /// `Weapon.getSoulShotCount()` — shots consumed per soulshot charge; 0 when
    /// the weapon can't take soulshots (Java's default `_soulShotCount = 0`).
    pub fn soulshot_count(&self, weapon_item_id: i32) -> i32 {
        self.weapon_shots
            .get(&weapon_item_id)
            .map(|s| s.0)
            .unwrap_or(0)
    }

    /// `Weapon.getSpiritShotCount()` — shots consumed per spiritshot charge.
    pub fn spiritshot_count(&self, weapon_item_id: i32) -> i32 {
        self.weapon_shots
            .get(&weapon_item_id)
            .map(|s| s.1)
            .unwrap_or(0)
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            by_id: HashMap::new(),
            stat_bonuses: HashMap::new(),
            armor_types: HashMap::new(),
            weapon_types: HashMap::new(),
            weapon_shots: HashMap::new(),
            icons: HashMap::new(),
        }
    }

    /// Attach a `<stats>` block to an already-registered template (tests that
    /// exercise gear stat contributions without reading `dist/game` XML).
    #[doc(hidden)]
    pub fn set_item_stats_for_test(&mut self, item_id: i32, stats: ItemStats) {
        self.stat_bonuses.insert(item_id, stats);
    }

    /// Attach an armor type to an already-registered template (tests exercising
    /// the armor-conditioned passive check without reading `dist/game` XML).
    #[doc(hidden)]
    pub fn set_armor_type_for_test(&mut self, item_id: i32, armor_type: ArmorType) {
        self.armor_types.insert(item_id, armor_type);
    }

    /// Attach a weapon type to an already-registered template (tests exercising
    /// the weapon-conditioned passive check without reading `dist/game` XML).
    #[doc(hidden)]
    pub fn set_weapon_type_for_test(&mut self, item_id: i32, weapon_type: WeaponType) {
        self.weapon_types.insert(item_id, weapon_type);
    }

    /// Attach weapon soulshot/spiritshot counts (tests exercising shot charging
    /// without reading `dist/game` XML).
    #[doc(hidden)]
    pub fn set_weapon_shots_for_test(
        &mut self,
        weapon_item_id: i32,
        soulshots: i32,
        spiritshots: i32,
    ) {
        self.weapon_shots
            .insert(weapon_item_id, (soulshots, spiritshots));
    }

    /// Synthetic catalog for unit tests that need specific templates without
    /// reading `dist/game` XML.
    #[doc(hidden)]
    pub fn from_templates(templates: Vec<ItemTemplate>) -> Self {
        Self {
            by_id: templates.into_iter().map(|t| (t.item_id, t)).collect(),
            stat_bonuses: HashMap::new(),
            armor_types: HashMap::new(),
            weapon_types: HashMap::new(),
            weapon_shots: HashMap::new(),
            icons: HashMap::new(),
        }
    }

    /// Register one synthetic template (same hook as `NpcData`'s).
    #[doc(hidden)]
    /// A neutral template to build test fixtures from — a plain, weightless,
    /// non-stackable `EtcItem`. Only exists so tests can name the two or three
    /// fields they care about instead of spelling out all thirty.
    /// Attach `<stats>` bonuses to an item in a test fixture.
    #[cfg(test)]
    pub fn insert_stats_for_test(&mut self, item_id: i32, bonuses: Vec<(Stat, f64)>) {
        self.stat_bonuses.insert(
            item_id,
            ItemStats {
                bonuses,
                ..Default::default()
            },
        );
    }

    pub fn insert_for_test(&mut self, t: ItemTemplate) {
        self.by_id.insert(t.item_id, t);
    }
}

#[cfg(test)]
mod tests;
