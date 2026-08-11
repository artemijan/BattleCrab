//! Port of `data/xml/ItemData` + `util/DocumentItem`, scoped to what G5 needs:
//! identity, equip slot, weight, stackability, and the `type1`/`type2` pair the
//! client needs for `ItemList` sorting/icons, plus (for `UseItem`'s `EtcItem`
//! branch) the `handler`/`<capsuled_items>` pair `ExtractableItems` reads, plus
//! the combat-stat bonuses under `<stats>` (Java `ItemTemplate._funcTemplates`)
//! the stats engine folds in when the item is equipped ([`ItemStats`], applied
//! by `Player::recalculate_stats`). `<cond>` is still not parsed.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

use crate::data::xml::{attr_f64, attr_i32, attr_i64, attr_str};
use crate::model::stats::Stat;

pub const ITEMS_DIR: &str = "data/stats/items";

/// `Inventory.ADENA_ID` / `ANCIENT_ADENA_ID`.
pub const ADENA_ID: i32 = 57;
const ANCIENT_ADENA_ID: i32 = 5575;

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
/// Public wrapper for callers outside this module (e.g. the enchant loader,
/// which resolves `<item slot=…>` strings the same way).
pub(crate) fn slot_mask(name: &str) -> i32 {
    body_part(name)
}

fn body_part(name: &str) -> i32 {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemKind {
    Weapon,
    Armor,
    /// The default: most items are `EtcItem`, and it is the harmless choice for
    /// a template built field-by-field in a test.
    #[default]
    Etc,
}

/// Port of `model/item/type/ArmorType`, scoped to the armor kinds the
/// armor-conditioned passive skills (`ConditionUsingItemType`) test against.
/// `<set name="armor_type" val="..."/>`; absent → `None`. `mask_bit` gives each
/// type its own bit so a `ConditionUsingItemType` mask (the OR of an effect's
/// `<armorType>` list) can be intersected against the worn chest/legs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArmorType {
    #[default]
    None,
    Light,
    Heavy,
    Magic,
    Sigil,
    Shield,
}

impl ArmorType {
    /// The single-type mask bit (Java `ArmorType.mask()`, reduced to a `u8`
    /// since only these six kinds are ever masked here).
    pub const fn mask_bit(self) -> u8 {
        match self {
            ArmorType::None => 1,
            ArmorType::Light => 2,
            ArmorType::Heavy => 4,
            ArmorType::Magic => 8,
            ArmorType::Sigil => 16,
            ArmorType::Shield => 32,
        }
    }

    /// `<set name="armor_type"/>` / `<armorType><item>..</item>` value → variant
    /// (Java `set.getEnum("armor_type", ArmorType.class, NONE)`). Unknown → `None`.
    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_uppercase().as_str() {
            "LIGHT" => ArmorType::Light,
            "HEAVY" => ArmorType::Heavy,
            "MAGIC" => ArmorType::Magic,
            "SIGIL" => ArmorType::Sigil,
            "SHIELD" => ArmorType::Shield,
            _ => ArmorType::None,
        }
    }
}

/// `<set name="weapon_type" val="..."/>` — the weapon's kind (Java
/// `WeaponType`). `mask_bit` gives each kind its own bit so a skill effect's
/// `<weaponType>` list (an OR of these bits) can be intersected against the
/// currently equipped weapon — the weapon-gated counterpart of [`ArmorType`],
/// e.g. Weapon Mastery 249's `-30% MagicalAttackSpeed` for BOW/POLE only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WeaponType {
    /// No weapon / not a real combat type (fists count as unarmed, `Etc`, …) —
    /// never matches a `<weaponType>` condition (bit 0).
    #[default]
    None,
    Sword,
    Blunt,
    Dagger,
    Bow,
    Crossbow,
    Pole,
    Fist,
    Dual,
    DualBlunt,
    DualDagger,
    DualFist,
    Rapier,
    AncientSword,
    TwoHandCrossbow,
    FishingRod,
}

impl WeaponType {
    /// The single-type mask bit; `None` is 0 so it never intersects any
    /// `<weaponType>` condition mask (a bare hand can't satisfy "BOW or POLE").
    pub const fn mask_bit(self) -> u32 {
        match self {
            WeaponType::None => 0,
            WeaponType::Sword => 1 << 0,
            WeaponType::Blunt => 1 << 1,
            WeaponType::Dagger => 1 << 2,
            WeaponType::Bow => 1 << 3,
            WeaponType::Crossbow => 1 << 4,
            WeaponType::Pole => 1 << 5,
            WeaponType::Fist => 1 << 6,
            WeaponType::Dual => 1 << 7,
            WeaponType::DualBlunt => 1 << 8,
            WeaponType::DualDagger => 1 << 9,
            WeaponType::DualFist => 1 << 10,
            WeaponType::Rapier => 1 << 11,
            WeaponType::AncientSword => 1 << 12,
            WeaponType::TwoHandCrossbow => 1 << 13,
            WeaponType::FishingRod => 1 << 14,
        }
    }

    /// `<set name="weapon_type"/>` / `<weaponType><item>..</item>` value →
    /// variant (Java `WeaponType.valueOf`). Unknown/`ETC` → `None`.
    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_uppercase().as_str() {
            "SWORD" => WeaponType::Sword,
            "BLUNT" => WeaponType::Blunt,
            "DAGGER" => WeaponType::Dagger,
            "BOW" => WeaponType::Bow,
            "CROSSBOW" => WeaponType::Crossbow,
            "POLE" => WeaponType::Pole,
            "FIST" => WeaponType::Fist,
            "DUAL" => WeaponType::Dual,
            "DUALBLUNT" => WeaponType::DualBlunt,
            "DUALDAGGER" => WeaponType::DualDagger,
            "DUALFIST" => WeaponType::DualFist,
            "RAPIER" => WeaponType::Rapier,
            "ANCIENTSWORD" => WeaponType::AncientSword,
            "TWOHANDCROSSBOW" => WeaponType::TwoHandCrossbow,
            "FISHINGROD" => WeaponType::FishingRod,
            _ => WeaponType::None,
        }
    }
}

/// Port of `model/item/type/CrystalType` — an item's grade. `level()` returns
/// the same ordinal Java's `CrystalType(int level, ...)` uses, which is what
/// the expertise/grade-penalty check compares against `Player.getExpertiseLevel`
/// (`Inventory`/`Player.refreshExpertisePenalty`). Parsed from
/// `<set name="crystal_type" val="D"/>`; absent → `None` (level 0, no penalty).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrystalType {
    #[default]
    None,
    D,
    C,
    B,
    A,
    S,
    S80,
    S84,
    R,
    R95,
    R99,
    Event,
}

impl CrystalType {
    /// `ItemTemplate.getCrystalTypePlus`: collapses the split top grades
    /// (S80/S84→S, R95/R99→R) so a shot matches any weapon of the same base
    /// grade. Identity for every grade Interlude actually uses (≤ S).
    pub fn plus(self) -> CrystalType {
        match self {
            CrystalType::S80 | CrystalType::S84 => CrystalType::S,
            CrystalType::R95 | CrystalType::R99 => CrystalType::R,
            other => other,
        }
    }

    /// Java `CrystalType.getLevel()` (the first enum-constructor arg).
    pub fn level(self) -> i32 {
        match self {
            CrystalType::None => 0,
            CrystalType::D => 1,
            CrystalType::C => 2,
            CrystalType::B => 3,
            CrystalType::A => 4,
            CrystalType::S => 5,
            CrystalType::S80 => 6,
            CrystalType::S84 => 7,
            CrystalType::R => 8,
            CrystalType::R95 => 9,
            CrystalType::R99 => 10,
            CrystalType::Event => 11,
        }
    }

    /// `CrystalType.getCrystalItemId()` — the crystal item the grade yields on
    /// crystallization (`Crystal (D-grade)` 1458 … `S-grade` 1462). `None` for
    /// un-crystallizable grades (NONE and R+, which the ported set doesn't use).
    pub fn crystal_item_id(self) -> Option<i32> {
        Some(match self {
            CrystalType::D => 1458,
            CrystalType::C => 1459,
            CrystalType::B => 1460,
            CrystalType::A => 1461,
            CrystalType::S | CrystalType::S80 | CrystalType::S84 => 1462,
            _ => return None,
        })
    }

    /// The minimum `CRYSTALLIZE` (skill 248) level needed to crystallize this
    /// grade (Java's `RequestCrystallizeItem` per-grade gate): D→1 … S→5.
    pub fn required_crystallize_level(self) -> i32 {
        self.plus().level().min(5)
    }

    /// `<set name="crystal_type" val="..."/>` → variant (Java
    /// `CrystalType.valueOf(name.toUpperCase())`). Unknown/absent → `None`.
    pub(crate) fn from_name(name: Option<&str>) -> Self {
        match name.map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("D") => CrystalType::D,
            Some("C") => CrystalType::C,
            Some("B") => CrystalType::B,
            Some("A") => CrystalType::A,
            Some("S") => CrystalType::S,
            Some("S80") => CrystalType::S80,
            Some("S84") => CrystalType::S84,
            Some("R") => CrystalType::R,
            Some("R95") => CrystalType::R95,
            Some("R99") => CrystalType::R99,
            Some("EVENT") => CrystalType::Event,
            _ => CrystalType::None,
        }
    }
}

/// `<set name="handler">` (Java `EtcItem._handlerName`, dispatched at use time
/// through `ItemHandler.getInstance().getHandler(name)`). Rust resolves the
/// name to a typed variant once at load time instead of a runtime string
/// registry — mirrors `SkillEffect`'s `name="..."` → enum pattern. Add new
/// variants here (and a match arm in `game_loop::items::use_etc_item`) as
/// more handlers get ported; unrecognized/absent names fall back to `None`
/// and the item is consumed as a no-op, same as Java's "Unmanaged Item
/// handler" branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemHandler {
    #[default]
    None,
    ExtractableItems,
    /// `ItemSkills`/`ItemSkillsTemplate` — casts the item's `<skills>` list
    /// (potions, buff scrolls, …) via `ItemTemplate::item_skills`. Both Java
    /// classes collapse to one variant: the only difference between them is
    /// an Olympiad-mode guard, and there's no Olympiad here.
    ItemSkills,
    /// `handlers/itemhandlers/Seed` — sow a manor seed on a monster: flags the
    /// mob with the seed, then casts the item's `<skills>` (the Sow skill).
    Seed,
    /// `handlers/itemhandlers/SoulShots` — charges the physical-attack shot on
    /// the equipped weapon (`ShotType::SOULSHOTS`).
    SoulShots,
    /// `handlers/itemhandlers/SpiritShot` — charges the magic-attack shot
    /// (`ShotType::SPIRITSHOTS`).
    SpiritShot,
    /// `handlers/itemhandlers/BlessedSpiritShot` — the blessed magic shot
    /// (`ShotType::BLESSED_SPIRITSHOTS`, ×4 magic bonus vs. ×2).
    BlessedSpiritShot,
    /// `handlers/itemhandlers/EnchantScrolls` — opens the enchant window for an
    /// enchant scroll (adds an `EnchantRequest` + `ChooseInventoryItem`).
    EnchantScrolls,
    /// `handlers/itemhandlers/Recipes` — registers the recipe the item teaches
    /// into the player's dwarven/common recipe book (G15.7).
    Recipes,
    /// `handlers/itemhandlers/BeastSoulShot` / `BeastSpiritShot` — a **pet's**
    /// shots. Toggled on the owner like any auto-shot, but spent by
    /// `Summon.rechargeShots` when the summon swings, not by the owner.
    BeastSoulShot,
    BeastSpiritShot,
    /// `handlers/itemhandlers/FishShots` — a fishing shot (Corroded Fishing
    /// Shot). Charged during a cast (`rechargeShots(fish=true)`), it doubles the
    /// fishing win chance (`ShotType::FISH_SOULSHOTS`).
    FishShots,
}

impl ItemHandler {
    /// Whether this handler charges a physical (soulshot) shot — the
    /// `rechargeShots(physical=…)` category (Java `ActionType.SOULSHOT`).
    pub fn is_soulshot(self) -> bool {
        matches!(self, ItemHandler::SoulShots | ItemHandler::BeastSoulShot)
    }

    /// Whether this handler charges a magic (spirit/blessed) shot — the
    /// `rechargeShots(magic=…)` category (Java `ActionType.SPIRITSHOT`).
    pub fn is_spiritshot(self) -> bool {
        matches!(
            self,
            ItemHandler::SpiritShot | ItemHandler::BlessedSpiritShot | ItemHandler::BeastSpiritShot
        )
    }

    /// Whether this handler charges a fishing shot — the
    /// `rechargeShots(fish=…)` category (Java `ActionType.FISHINGSHOT`).
    pub fn is_fishshot(self) -> bool {
        matches!(self, ItemHandler::FishShots)
    }
}

/// `<set name="etcitem_type">`, narrowed to the enchant-scroll kinds and the
/// ammunition kinds the
/// enchant flow branches on (Java `EtcItemType`, used through
/// `AbstractEnchantItem.ENCHANT_TYPES` + `EnchantScroll`'s type flags). Every
/// other value collapses to [`EtcItemType::Other`]. The `is_*` helpers mirror
/// `EnchantScroll`'s `_isWeapon`/`_isBlessed`/`_isBlessedDown`/`_isSafe`/
/// `_isGiant` classification.
/// `<set name="default_action">` (Java `ActionType`), narrowed to the three
/// values `ItemSkillsTemplate.checkConsume` branches on. Everything else —
/// `EQUIP`, `PEEL`, `RECIPE`, … — collapses to [`ActionType::Other`], which
/// takes `checkConsume`'s fallthrough (`return hasConsumeSkill`) exactly like
/// Java's unlisted cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActionType {
    #[default]
    Other,
    Capsule,
    SkillReduce,
    /// The item is destroyed by `SkillCaster.finishSkill` when the cast
    /// actually lands, never by the item handler.
    SkillReduceOnSkillSuccess,
    /// Beast Soulshot / Beast Spiritshot — a **summon's** shots, charged from
    /// the owner's inventory before the summon swings (Java
    /// `Summon.rechargeShots`). Distinct from the player's own shots, which
    /// carry no `default_action` of this kind.
    SummonSoulshot,
    SummonSpiritshot,
}

impl ActionType {
    fn from_name(name: Option<&str>) -> Self {
        match name {
            Some("CAPSULE") => ActionType::Capsule,
            Some("SKILL_REDUCE") => ActionType::SkillReduce,
            Some("SKILL_REDUCE_ON_SKILL_SUCCESS") => ActionType::SkillReduceOnSkillSuccess,
            Some("SUMMON_SOULSHOT") => ActionType::SummonSoulshot,
            Some("SUMMON_SPIRITSHOT") => ActionType::SummonSpiritshot,
            _ => ActionType::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EtcItemType {
    #[default]
    Other,
    /// `ARROW` / `BOLT` — bow and crossbow ammunition. Matched to the weapon by
    /// crystal grade (`findArrowForBow`) and auto-equipped into the left hand.
    Arrow,
    Bolt,
    EnchtWp,
    EnchtAm,
    BlessEnchtWp,
    BlessEnchtAm,
    BlessEnchtAmDown,
    GiantEnchtWp,
    GiantEnchtAm,
    EnchtAttrCrystalEnchantWp,
    EnchtAttrCrystalEnchantAm,
    EnchtAttrAncientCrystalEnchantWp,
    EnchtAttrAncientCrystalEnchantAm,
    // Support items (`EnchantSupportItem`) — raise the chance and can widen the
    // success step. `ENCHT_ATTR_INC_PROP_*` plus their blessed/giant variants.
    EnchtAttrIncPropEnchtWp,
    EnchtAttrIncPropEnchtAm,
    BlessedEnchtAttrIncPropEnchtWp,
    BlessedEnchtAttrIncPropEnchtAm,
    GiantEnchtAttrIncPropEnchtWp,
    GiantEnchtAttrIncPropEnchtAm,
    BlessedGiantEnchtAttrIncPropEnchtWp,
    BlessedGiantEnchtAttrIncPropEnchtAm,
}

impl EtcItemType {
    fn from_name(name: Option<&str>) -> Self {
        match name {
            Some("ARROW") => EtcItemType::Arrow,
            Some("BOLT") => EtcItemType::Bolt,
            Some("ENCHT_WP") => EtcItemType::EnchtWp,
            Some("ENCHT_AM") => EtcItemType::EnchtAm,
            Some("BLESS_ENCHT_WP") => EtcItemType::BlessEnchtWp,
            Some("BLESS_ENCHT_AM") => EtcItemType::BlessEnchtAm,
            Some("BLESS_ENCHT_AM_DOWN") => EtcItemType::BlessEnchtAmDown,
            Some("GIANT_ENCHT_WP") => EtcItemType::GiantEnchtWp,
            Some("GIANT_ENCHT_AM") => EtcItemType::GiantEnchtAm,
            Some("ENCHT_ATTR_CRYSTAL_ENCHANT_WP") => EtcItemType::EnchtAttrCrystalEnchantWp,
            Some("ENCHT_ATTR_CRYSTAL_ENCHANT_AM") => EtcItemType::EnchtAttrCrystalEnchantAm,
            Some("ENCHT_ATTR_ANCIENT_CRYSTAL_ENCHANT_WP") => {
                EtcItemType::EnchtAttrAncientCrystalEnchantWp
            }
            Some("ENCHT_ATTR_ANCIENT_CRYSTAL_ENCHANT_AM") => {
                EtcItemType::EnchtAttrAncientCrystalEnchantAm
            }
            Some("ENCHT_ATTR_INC_PROP_ENCHT_WP") => EtcItemType::EnchtAttrIncPropEnchtWp,
            Some("ENCHT_ATTR_INC_PROP_ENCHT_AM") => EtcItemType::EnchtAttrIncPropEnchtAm,
            Some("BLESSED_ENCHT_ATTR_INC_PROP_ENCHT_WP") => {
                EtcItemType::BlessedEnchtAttrIncPropEnchtWp
            }
            Some("BLESSED_ENCHT_ATTR_INC_PROP_ENCHT_AM") => {
                EtcItemType::BlessedEnchtAttrIncPropEnchtAm
            }
            Some("GIANT_ENCHT_ATTR_INC_PROP_ENCHT_WP") => EtcItemType::GiantEnchtAttrIncPropEnchtWp,
            Some("GIANT_ENCHT_ATTR_INC_PROP_ENCHT_AM") => EtcItemType::GiantEnchtAttrIncPropEnchtAm,
            Some("BLESSED_GIANT_ENCHT_ATTR_INC_PROP_ENCHT_WP") => {
                EtcItemType::BlessedGiantEnchtAttrIncPropEnchtWp
            }
            Some("BLESSED_GIANT_ENCHT_ATTR_INC_PROP_ENCHT_AM") => {
                EtcItemType::BlessedGiantEnchtAttrIncPropEnchtAm
            }
            _ => EtcItemType::Other,
        }
    }

    /// Any of the enchant-scroll kinds (Java `AbstractEnchantItem.isEnchantScroll`,
    /// narrowed to the scroll — not the support — types).
    pub fn is_enchant_scroll(self) -> bool {
        self != EtcItemType::Other && !self.is_enchant_support()
    }

    /// An `EnchantSupportItem` type (the `INC_PROP` family).
    pub fn is_enchant_support(self) -> bool {
        matches!(
            self,
            EtcItemType::EnchtAttrIncPropEnchtWp
                | EtcItemType::EnchtAttrIncPropEnchtAm
                | EtcItemType::BlessedEnchtAttrIncPropEnchtWp
                | EtcItemType::BlessedEnchtAttrIncPropEnchtAm
                | EtcItemType::GiantEnchtAttrIncPropEnchtWp
                | EtcItemType::GiantEnchtAttrIncPropEnchtAm
                | EtcItemType::BlessedGiantEnchtAttrIncPropEnchtWp
                | EtcItemType::BlessedGiantEnchtAttrIncPropEnchtAm
        )
    }

    /// `EnchantSupportItem._isWeapon`.
    pub fn support_is_weapon(self) -> bool {
        matches!(
            self,
            EtcItemType::EnchtAttrIncPropEnchtWp
                | EtcItemType::BlessedEnchtAttrIncPropEnchtWp
                | EtcItemType::GiantEnchtAttrIncPropEnchtWp
                | EtcItemType::BlessedGiantEnchtAttrIncPropEnchtWp
        )
    }

    /// `EnchantSupportItem._isBlessed`.
    pub fn support_is_blessed(self) -> bool {
        matches!(
            self,
            EtcItemType::BlessedEnchtAttrIncPropEnchtWp
                | EtcItemType::BlessedEnchtAttrIncPropEnchtAm
                | EtcItemType::BlessedGiantEnchtAttrIncPropEnchtWp
                | EtcItemType::BlessedGiantEnchtAttrIncPropEnchtAm
        )
    }

    /// `EnchantSupportItem._isGiant`.
    pub fn support_is_giant(self) -> bool {
        matches!(
            self,
            EtcItemType::GiantEnchtAttrIncPropEnchtWp
                | EtcItemType::GiantEnchtAttrIncPropEnchtAm
                | EtcItemType::BlessedGiantEnchtAttrIncPropEnchtWp
                | EtcItemType::BlessedGiantEnchtAttrIncPropEnchtAm
        )
    }

    /// `EnchantScroll._isWeapon`.
    pub fn is_enchant_weapon(self) -> bool {
        matches!(
            self,
            EtcItemType::EnchtWp
                | EtcItemType::BlessEnchtWp
                | EtcItemType::GiantEnchtWp
                | EtcItemType::EnchtAttrAncientCrystalEnchantWp
        )
    }

    /// `EnchantScroll._isBlessed` — item survives a failure, enchant resets to 0.
    pub fn is_blessed(self) -> bool {
        matches!(self, EtcItemType::BlessEnchtWp | EtcItemType::BlessEnchtAm)
    }

    /// `EnchantScroll._isBlessedDown` — item survives, enchant drops by 1.
    pub fn is_blessed_down(self) -> bool {
        self == EtcItemType::BlessEnchtAmDown
    }

    /// `EnchantScroll._isGiant`.
    pub fn is_giant(self) -> bool {
        matches!(self, EtcItemType::GiantEnchtWp | EtcItemType::GiantEnchtAm)
    }

    /// `EnchantScroll._isSafe` — enchant level is retained on failure.
    pub fn is_safe(self) -> bool {
        matches!(
            self,
            EtcItemType::EnchtAttrCrystalEnchantWp
                | EtcItemType::EnchtAttrCrystalEnchantAm
                | EtcItemType::EnchtAttrAncientCrystalEnchantWp
                | EtcItemType::EnchtAttrAncientCrystalEnchantAm
        )
    }
}

/// One `<capsuled_items><item .../></capsuled_items>` entry (Java
/// `ExtractableProduct`). `chance` is pre-scaled the same way Java's
/// constructor does (`(int) (chance * 1000)`), so it compares directly
/// against a `World::roll(100_000)` draw. `minEnchant`/`maxEnchant` are not
/// parsed — none of the currently-loaded extractable items set them, and
/// applying an enchant level to a freshly granted item needs an `Inventory`
/// setter that doesn't exist yet.
#[derive(Debug, Clone, Copy)]
pub struct CapsuledItem {
    pub item_id: i32,
    pub min: i64,
    pub max: i64,
    pub chance: i32,
}

/// Parsed `<stats>` block of an equipable item (Java `ItemTemplate`'s
/// `_funcTemplates`, all `FuncAdd`). Kept in a side-map on [`ItemData`] rather
/// than on [`ItemTemplate`] so the (many) template literals stay untouched.
/// The stats engine distinguishes two application rules when the item is worn
/// (see `Player::recalculate_stats`), matching the Java stat finalizers:
///   * **weapon-replace** (`calcWeaponBaseValue`): the equipped weapon's
///     `pAtk`/`mAtk`/`pAtkSpd`/`rCrit`/`mCritRate` value *replaces* the wearer's
///     naked class base before the STR/level multipliers apply;
///   * **sum-add** (`calcWeaponPlusBaseValue` / paperdoll loop): `pDef`/`mDef`/
///     `accCombat`/`accMagic`/`rEvas`/`mEvas`/`maxHp`/`maxMp` are summed across
///     every equipped piece and added on top of the computed base.
#[derive(Debug, Clone, Default)]
pub struct ItemStats {
    /// `<stat type="..">` entries mapped to an engine [`Stat`], in document
    /// order. Types the engine doesn't compute yet (elemental power/res,
    /// `sDef`, `rShld`, …) are dropped during parse.
    pub bonuses: Vec<(Stat, f64)>,
    /// `pAtkRange` — a weapon-only template constant (not a `Stat`); replaces
    /// `CombatStats.atk_range` while the weapon is equipped.
    pub atk_range: Option<i32>,
    /// `randomDamage` — weapon damage spread; replaces `CombatStats.random_dmg`
    /// (class templates all declare 10) while the weapon is equipped.
    pub random_damage: Option<i32>,
    /// `sDef` — a shield's block defence (added to the wearer's pDef on a
    /// successful shield block, Java `getShldDef`). Shield-only.
    pub shield_def: Option<i32>,
    /// `rShld` — a shield's base block *rate* (percent, before the CON bonus),
    /// Java `Stat.SHIELD_DEFENCE_RATE`. Shield-only.
    pub shield_rate: Option<i32>,
}

/// The datapack's per-item transfer restrictions (Java `ItemTemplate`'s
/// `_dropable` / `_tradeable` / `_destroyable` / `_depositable`). Every flag
/// defaults to **true** in Java — `set.getBoolean("is_dropable", true)` — so an
/// item is fully transferable unless its XML says otherwise; `Default` here
/// mirrors that, which also keeps `..Default::default()` test fixtures
/// permissive.
///
/// Time-limited reward boxes such as *Mage Class Equipment Set (10-day)*
/// (15195) declare all three of `is_tradable`/`is_dropable`/`is_sellable` as
/// `false`: they may only be used, warehoused or destroyed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeFlags {
    /// `<set name="is_dropable">` — may be dropped on the ground
    /// (`RequestDropItem`) and scattered by `Player.onDieDropItem`.
    pub dropable: bool,
    /// `<set name="is_tradable">` — may change owner: player trade, private
    /// store (buy and sell), mail attachment.
    pub tradable: bool,
    /// `<set name="is_destroyable">` — may be destroyed (`RequestDestroyItem`).
    pub destroyable: bool,
    /// `<set name="is_depositable">` — may be put in a warehouse. Java forces
    /// this to `!is_questitem` for quest items; for everything else it reads
    /// the tag (default true). Note `Item.isDepositable(isPrivateWareHouse)`
    /// only additionally demands tradability for the *clan* warehouse and
    /// freight — a private warehouse takes untradable items, which is why a
    /// bound reward box can still be stored.
    pub depositable: bool,
}

impl Default for TradeFlags {
    fn default() -> Self {
        Self {
            dropable: true,
            tradable: true,
            destroyable: true,
            depositable: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ItemTemplate {
    pub item_id: i32,
    pub name: String,
    pub kind: ItemKind,
    /// `<set name="crystal_type"/>` — the item's grade, `None` when undeclared.
    /// Drives the expertise/grade penalty (`refresh_expertise_penalty`).
    pub crystal_type: CrystalType,
    /// `<set name="crystal_count"/>` — crystals yielded on crystallization (0 =
    /// not crystallizable).
    pub crystal_count: i32,
    /// `<set name="damage_range" val="0;0;radius;angle"/>` — the weapon's
    /// melee sweep geometry (Java `Weapon.getBaseAttackRadius/Angle`). A
    /// polearm reaches 66 with a 120° arc; a sword 40/120. Java's fallback when
    /// the tag is absent or malformed is radius 40, angle **0** — an angle of 0
    /// makes the multi-target sweep hit nothing, which is the intended
    /// "single-target weapon" behaviour.
    pub attack_radius: i32,
    pub attack_angle: i32,
    /// `<set name="mp_consume"/>` — MP a **ranged** weapon spends per shot
    /// (Short Bow 13 → 1). 0 for everything else.
    pub mp_consume: i32,
    /// `<set name="reduced_mp_consume"/>` + `reduced_mp_consume_chance` — some
    /// bows roll a cheaper shot. Absent on this dist's bows; ported for
    /// faithfulness (Java: `Rnd.get(100) < chance` swaps in the cheaper cost).
    pub reduced_mp_consume: i32,
    pub reduced_mp_consume_chance: i32,
    pub body_part: i32,
    pub weight: i32,
    pub is_stackable: bool,
    /// `<set name="is_infinite">` (Java `EtcItem._infinite`) — ammunition that
    /// is never spent. `PlayerInventory.reduceArrowCount` returns before the
    /// decrement for these, so an infinite quiver lasts forever. This dist
    /// ships 14 of them: arrows 32249-32255 and bolts 32256-32262.
    pub is_infinite: bool,
    pub type1: i32,
    pub type2: i32,
    pub is_quest_item: bool,
    /// `<set name="is_sellable">` (Java `ItemTemplate._sellable`, default
    /// **true**) — whether a merchant buys the item; gates both the sell-tab
    /// listing and `RequestSellItem`. Note the derived `Default` yields
    /// `false` — test fixtures built with `..Default::default()` are
    /// non-sellable unless they say otherwise.
    pub is_sellable: bool,
    /// `<set name="is_freightable">` (Java `ItemTemplate._freightable`, default
    /// **false**) — whether the item may be sent to another character on the
    /// account through the freight (`RequestPackageSend`). 3457 items declare
    /// it on this dist; everything else is refused.
    pub is_freightable: bool,
    /// `is_dropable` / `is_tradable` / `is_destroyable` / `is_depositable` —
    /// see [`TradeFlags`]. Grouped into a sub-struct so the derived `Default`
    /// on `ItemTemplate` yields Java's permissive defaults (everything allowed)
    /// instead of `false` (everything forbidden).
    pub trade_flags: TradeFlags,
    /// `<set name="time">` — lifetime in minutes for a time-limited item
    /// (Java `ItemTemplate.getTime()`, `-1`/absent = permanent). Only the
    /// "is this time-limited at all" question is modelled: expiry itself is not
    /// ported yet, but Java's `Player.onDieDropItem` refuses to scatter such
    /// items and that guard is honoured.
    /// SKIP(off-chronicle): actual expiry (`Item.scheduleLifeTimeTask`) is not
    /// ported, and this is a decision rather than a deferral. All **3230**
    /// items on this dist declaring a positive `time` sit in the 10015-47923
    /// band — every one post-Interlude content, the lowest being Prison Gate
    /// Key (10015). Nothing reachable on this chronicle expires, so the timer
    /// would be machinery with no consumer.
    ///
    /// Re-verified 2026-08-05 against the datapack, not against this prose:
    /// 3230 items, lowest id exactly 10015, none below it. Re-run that check
    /// before porting the timer — the claim is about the *data*, so new data
    /// is what would overturn it.
    pub time: i32,
    /// `<set name="duration">` — a **shadow item**'s starting mana, in
    /// minutes of wear (Java `ItemTemplate.getDuration()`, `-1`/absent =
    /// not a shadow item). Every freshly created instance starts at this
    /// value (Java's `Item` constructors: `_mana = _itemTemplate
    /// .getDuration()`), and `Item.isShadowItem()` is simply `mana >= 0`.
    /// 1353 items declare it on this dist, but inside the Interlude id range
    /// they are the 238 `Shadow Item: …` weapons (8821+, 90 or 300 minutes)
    /// plus the talismans; the rest are later-chronicle ids nothing here can
    /// hand out. See [`crate::game_loop::item_mana`].
    pub duration: i32,
    /// `<set name="price">` — the reference price (sell value = half of it;
    /// the `CorrectPrices` buylist floor uses it too). 0 when undeclared.
    pub price: i64,
    /// `<set name="handler">`, resolved to a typed dispatch target.
    pub handler: ItemHandler,
    /// `<capsuled_items>` children, in document order (`ExtractableItems`
    /// rolls each entry independently against its `chance`).
    pub capsuled_items: Vec<CapsuledItem>,
    /// `<set name="extractableCountMin">` — 0 (the common case) means "no
    /// minimum, one pass over the list is enough"; > 0 means keep re-rolling
    /// the whole list until at least this many distinct entries have hit
    /// (Java `ExtractableItems.useItem`'s `while` loop — used by "pick one of
    /// N" reward boxes).
    pub extractable_count_min: i32,
    /// `<set name="extractableCountMax">` — 0 (the common case) means "no
    /// cap, grant every entry that hits"; `ExtractableItems` stops rolling
    /// once this many entries have been granted.
    pub extractable_count_max: i32,
    /// `<skills><skill id=".." level=".." /></skills>` (Java
    /// `EtcItem._skills`, read by `ItemSkillsTemplate.useItem` via
    /// `getSkills(ItemSkillType.NORMAL)`) — `(skill_id, skill_level)` pairs,
    /// in document order.
    pub item_skills: Vec<(i32, i32)>,
    /// `<set name="etcitem_type">`, narrowed to enchant classification.
    pub etc_item_type: EtcItemType,
    /// `<set name="enchant_enabled">` (Java `_enchantable`) — whether this item
    /// can be enchanted at all.
    pub enchant_enabled: bool,
    /// `<set name="enchant_limit">` (Java `_enchantLimit`, 0 = no limit) — the
    /// cap an enchant scroll may not push past.
    pub enchant_limit: i32,
    /// `<set name="is_magic_weapon">` (Java `Weapon._isMagicWeapon`; false for
    /// non-weapons) — splits the fighter/mage enchant rate groups.
    pub is_magic_weapon: bool,
    /// `<set name="immediate_effect">` / `<set name="ex_immediate_effect">`
    /// (Java `ItemTemplate.hasImmediateEffect`/`hasExImmediateEffect`, both
    /// default false). Either one makes `ItemSkillsTemplate` fire the item's
    /// skills instantly instead of casting them; `immediate_effect` alone
    /// also feeds `checkConsume`.
    pub immediate_effect: bool,
    pub ex_immediate_effect: bool,
    /// `<set name="default_action">` — only the values `checkConsume`
    /// distinguishes are kept (see [`ActionType`]).
    pub default_action: ActionType,
}

#[cfg(test)]
impl ItemTemplate {
    /// A blank template carrying **Java's** field defaults, for test fixtures.
    ///
    /// The derived `Default` is a zero-fill, which disagrees with Java in four
    /// places. Every hand-written fixture in the tree used to correct them by
    /// spelling out all ~35 fields; build on this instead and list only what
    /// the test is actually about.
    ///
    /// | field | derived `Default` | Java |
    /// |---|---|---|
    /// | `time` / `duration` | `0` | `-1` (permanent) |
    /// | `attack_radius` | `0` | `40` (single-target melee reach) |
    /// | `is_sellable` | `false` | `true` |
    pub(crate) fn for_test() -> Self {
        Self {
            time: -1,
            duration: -1,
            attack_radius: 40,
            is_sellable: true,
            ..Default::default()
        }
    }
}

impl ItemTemplate {
    /// `ItemTemplate.isEquipable`: has a body part and isn't an `EtcItem`.
    pub fn is_equipable(&self) -> bool {
        self.kind != ItemKind::Etc && self.body_part != SLOT_NONE
    }

    /// `ItemTemplate.isEnchantable` — narrowed to the `enchant_enabled` flag
    /// (the `Config.ENCHANT_BLACKLIST` check isn't modelled).
    pub fn is_enchantable(&self) -> bool {
        self.enchant_enabled
    }

    /// `Item.isDropable` — droppable on the ground and by a death drop.
    /// (Java also refuses augmented / visual-transformed items; neither is a
    /// state this port carries on the ground-drop path.)
    pub fn is_dropable(&self) -> bool {
        self.trade_flags.dropable
    }

    /// `Item.isTradeable` — may change owner (trade, private store, mail).
    pub fn is_tradable(&self) -> bool {
        self.trade_flags.tradable
    }

    /// `Item.isDestroyable` — may be destroyed by the player.
    pub fn is_destroyable(&self) -> bool {
        self.trade_flags.destroyable
    }

    /// `Item.isDepositable(isPrivateWareHouse)` minus the equipped check (the
    /// caller owns that, since it needs the inventory). A private warehouse
    /// accepts any depositable item; the clan warehouse and freight also
    /// require tradability.
    pub fn is_depositable(&self, private_warehouse: bool) -> bool {
        self.trade_flags.depositable && (private_warehouse || self.is_tradable())
    }

    /// `Item.isTimeLimitedItem` — the template declares a `time` lifetime, so
    /// the item expires. Java refuses to scatter such items on death.
    pub fn is_time_limited(&self) -> bool {
        self.time > 0
    }
}

#[derive(Clone)]
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
        let mut by_id = HashMap::new();
        let mut stat_bonuses = HashMap::new();
        let mut armor_types = HashMap::new();
        let mut weapon_types = HashMap::new();
        let mut weapon_shots = HashMap::new();
        let mut icons = HashMap::new();
        let dir = format!("{file_path}{ITEMS_DIR}");
        {
            for path in super::xml::xml_files_in(&dir) {
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

/// `damage_range` is `a;b;radius;angle`; Java only reads it when all four parts
/// parse, otherwise falling back to 40/0.
fn damage_range_part(raw: Option<&String>, index: usize, fallback: i32) -> i32 {
    let Some(raw) = raw else { return fallback };
    let parts: Vec<&str> = raw.split(';').collect();
    if parts.len() < 4 {
        return fallback;
    }
    parts
        .get(index)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(fallback)
}

fn parse_file(
    path: &std::path::Path,
    out: &mut HashMap<i32, ItemTemplate>,
    stats_out: &mut HashMap<i32, ItemStats>,
    armor_out: &mut HashMap<i32, ArmorType>,
    weapon_out: &mut HashMap<i32, WeaponType>,
    weapon_shots_out: &mut HashMap<i32, (i32, i32)>,
    icons_out: &mut HashMap<i32, String>,
) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let mut reader = Reader::from_str(&content);

    let mut cur_id: Option<i32> = None;
    let mut cur_name = String::new();
    let mut cur_kind = ItemKind::Etc;
    let mut attrs: HashMap<String, String> = HashMap::new();
    let mut in_capsules = false;
    let mut cur_capsules: Vec<CapsuledItem> = Vec::new();
    let mut in_skills = false;
    let mut cur_item_skills: Vec<(i32, i32)> = Vec::new();
    let mut in_stats = false;
    let mut cur_stat_type: Option<String> = None;
    let mut cur_stats = ItemStats::default();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"item" => {
                cur_id = attr_i32(&e, b"id");
                cur_name = attr_str(&e, b"name").unwrap_or_default();
                cur_kind = match attr_str(&e, b"type").as_deref() {
                    Some("Weapon") => ItemKind::Weapon,
                    Some("Armor") => ItemKind::Armor,
                    _ => ItemKind::Etc,
                };
                attrs.clear();
                cur_capsules.clear();
                cur_item_skills.clear();
                cur_stats = ItemStats::default();
            }
            Ok(Event::Start(e)) if e.name().as_ref() == b"stats" => {
                in_stats = true;
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"stats" => {
                in_stats = false;
            }
            Ok(Event::Start(e)) if in_stats && e.name().as_ref() == b"stat" => {
                cur_stat_type = attr_str(&e, b"type");
            }
            Ok(Event::End(e)) if in_stats && e.name().as_ref() == b"stat" => {
                cur_stat_type = None;
            }
            Ok(Event::Text(t)) if in_stats && cur_stat_type.is_some() => {
                let ty = cur_stat_type.as_deref().unwrap();
                if let Ok(text) = t.unescape()
                    && let Ok(val) = text.trim().parse::<f64>()
                {
                    match ty {
                        "pAtkRange" => cur_stats.atk_range = Some(val as i32),
                        "randomDamage" => cur_stats.random_damage = Some(val as i32),
                        "sDef" => cur_stats.shield_def = Some(val as i32),
                        "rShld" => cur_stats.shield_rate = Some(val as i32),
                        _ => {
                            if let Some(stat) = stat_from_xml(ty) {
                                cur_stats.bonuses.push((stat, val));
                            }
                        }
                    }
                }
            }
            Ok(Event::Empty(e)) if e.name().as_ref() == b"set" => {
                if cur_id.is_none() {
                    continue;
                }
                if let (Some(name), Some(val)) = (attr_str(&e, b"name"), attr_str(&e, b"val")) {
                    attrs.insert(name, val);
                }
            }
            Ok(Event::Start(e)) if e.name().as_ref() == b"capsuled_items" => {
                in_capsules = true;
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"capsuled_items" => {
                in_capsules = false;
            }
            Ok(Event::Empty(e)) if in_capsules && e.name().as_ref() == b"item" => {
                if let (Some(item_id), Some(min), Some(max), Some(chance)) = (
                    attr_i32(&e, b"id"),
                    attr_i64(&e, b"min"),
                    attr_i64(&e, b"max"),
                    attr_f64(&e, b"chance"),
                ) {
                    cur_capsules.push(CapsuledItem {
                        item_id,
                        min,
                        max,
                        chance: (chance * 1000.0) as i32,
                    });
                }
            }
            Ok(Event::Start(e)) if e.name().as_ref() == b"skills" => {
                in_skills = true;
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"skills" => {
                in_skills = false;
            }
            Ok(Event::Empty(e)) if in_skills && e.name().as_ref() == b"skill" => {
                if let (Some(id), Some(level)) = (attr_i32(&e, b"id"), attr_i32(&e, b"level")) {
                    cur_item_skills.push((id, level));
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"item" => {
                if let Some(item_id) = cur_id.take() {
                    out.insert(
                        item_id,
                        make_template(
                            item_id,
                            std::mem::take(&mut cur_name),
                            cur_kind,
                            &attrs,
                            std::mem::take(&mut cur_capsules),
                            std::mem::take(&mut cur_item_skills),
                        ),
                    );
                    let stats = std::mem::take(&mut cur_stats);
                    if !stats.bonuses.is_empty()
                        || stats.atk_range.is_some()
                        || stats.random_damage.is_some()
                    {
                        stats_out.insert(item_id, stats);
                    }
                    if let Some(at) = attrs.get("armor_type").map(|s| ArmorType::from_name(s))
                        && at != ArmorType::None
                    {
                        armor_out.insert(item_id, at);
                    }
                    if let Some(wt) = attrs.get("weapon_type").map(|s| WeaponType::from_name(s))
                        && wt != WeaponType::None
                    {
                        weapon_out.insert(item_id, wt);
                    }
                    // `Weapon._soulShotCount`/`_spiritShotCount` — only weapons
                    // declaring a non-zero count can charge that shot kind.
                    let ss = attrs
                        .get("soulshots")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    let sps = attrs
                        .get("spiritshots")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    if ss != 0 || sps != 0 {
                        weapon_shots_out.insert(item_id, (ss, sps));
                    }
                    if let Some(icon) = attrs.get("icon") {
                        icons_out.insert(item_id, icon.clone());
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
}

fn make_template(
    item_id: i32,
    name: String,
    kind: ItemKind,
    attrs: &HashMap<String, String>,
    capsuled_items: Vec<CapsuledItem>,
    item_skills: Vec<(i32, i32)>,
) -> ItemTemplate {
    let weight = attrs
        .get("weight")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let is_stackable = attrs
        .get("is_stackable")
        .map(|v| v == "true")
        .unwrap_or(false);
    let is_quest_item = attrs
        .get("is_questitem")
        .map(|v| v == "true")
        .unwrap_or(false);
    let is_infinite = attrs
        .get("is_infinite")
        .map(|v| v == "true")
        .unwrap_or(false);
    let part = body_part(attrs.get("bodypart").map(|s| s.as_str()).unwrap_or("none"));

    let (type1, type2) = match kind {
        ItemKind::Weapon => (TYPE1_WEAPON_RING_EARRING_NECKLACE, TYPE2_WEAPON),
        ItemKind::Armor => {
            if part == SLOT_NECK
                || (part & SLOT_L_EAR) != 0
                || (part & SLOT_L_FINGER) != 0
                || (part & SLOT_R_BRACELET) != 0
                || (part & SLOT_L_BRACELET) != 0
            {
                (TYPE1_WEAPON_RING_EARRING_NECKLACE, TYPE2_ACCESSORY)
            } else {
                (TYPE1_SHIELD_ARMOR, TYPE2_SHIELD_ARMOR)
            }
        }
        ItemKind::Etc => {
            let type2 = if is_quest_item {
                TYPE2_QUEST
            } else if item_id == ADENA_ID || item_id == ANCIENT_ADENA_ID {
                TYPE2_MONEY
            } else {
                TYPE2_OTHER
            };
            (TYPE1_ITEM_QUESTITEM_ADENA, type2)
        }
    };

    let handler = match attrs.get("handler").map(|s| s.as_str()) {
        Some("ExtractableItems") => ItemHandler::ExtractableItems,
        Some("ItemSkills") | Some("ItemSkillsTemplate") => ItemHandler::ItemSkills,
        Some("Seed") => ItemHandler::Seed,
        Some("SoulShots") => ItemHandler::SoulShots,
        Some("SpiritShot") => ItemHandler::SpiritShot,
        Some("BlessedSpiritShot") => ItemHandler::BlessedSpiritShot,
        Some("EnchantScrolls") => ItemHandler::EnchantScrolls,
        Some("Recipes") => ItemHandler::Recipes,
        Some("BeastSoulShot") => ItemHandler::BeastSoulShot,
        Some("BeastSpiritShot") => ItemHandler::BeastSpiritShot,
        Some("FishShots") => ItemHandler::FishShots,
        _ => ItemHandler::None,
    };

    ItemTemplate {
        item_id,
        name,
        kind,
        crystal_type: CrystalType::from_name(attrs.get("crystal_type").map(|s| s.as_str())),
        attack_radius: damage_range_part(attrs.get("damage_range"), 2, 40),
        attack_angle: damage_range_part(attrs.get("damage_range"), 3, 0),
        mp_consume: attrs
            .get("mp_consume")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        reduced_mp_consume: attrs
            .get("reduced_mp_consume")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        reduced_mp_consume_chance: attrs
            .get("reduced_mp_consume_chance")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        crystal_count: attrs
            .get("crystal_count")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        body_part: part,
        weight,
        is_stackable,
        is_infinite,
        type1,
        type2,
        is_quest_item,
        is_sellable: attrs
            .get("is_sellable")
            .map(|v| v == "true")
            .unwrap_or(true),
        is_freightable: attrs.get("is_freightable").map(|v| v == "true") == Some(true),
        trade_flags: TradeFlags {
            dropable: attrs
                .get("is_dropable")
                .map(|v| v == "true")
                .unwrap_or(true),
            tradable: attrs
                .get("is_tradable")
                .map(|v| v == "true")
                .unwrap_or(true),
            destroyable: attrs
                .get("is_destroyable")
                .map(|v| v == "true")
                .unwrap_or(true),
            // Java: quest items are never depositable (barring the
            // `CustomDepositableQuestItems` config, which this dist leaves off).
            depositable: !is_quest_item
                && attrs
                    .get("is_depositable")
                    .map(|v| v == "true")
                    .unwrap_or(true),
        },
        time: attrs.get("time").and_then(|v| v.parse().ok()).unwrap_or(-1),
        duration: attrs
            .get("duration")
            .and_then(|v| v.parse().ok())
            .unwrap_or(-1),
        price: attrs.get("price").and_then(|v| v.parse().ok()).unwrap_or(0),
        handler,
        capsuled_items,
        extractable_count_min: attrs
            .get("extractableCountMin")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        extractable_count_max: attrs
            .get("extractableCountMax")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        item_skills,
        etc_item_type: EtcItemType::from_name(attrs.get("etcitem_type").map(|s| s.as_str())),
        enchant_enabled: attrs
            .get("enchant_enabled")
            .map(|v| v == "true")
            .unwrap_or(false),
        enchant_limit: attrs
            .get("enchant_limit")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        is_magic_weapon: kind == ItemKind::Weapon
            && attrs
                .get("is_magic_weapon")
                .map(|v| v == "true")
                .unwrap_or(false),
        immediate_effect: attrs
            .get("immediate_effect")
            .map(|v| v == "true")
            .unwrap_or(false),
        ex_immediate_effect: attrs
            .get("ex_immediate_effect")
            .map(|v| v == "true")
            .unwrap_or(false),
        default_action: ActionType::from_name(attrs.get("default_action").map(|s| s.as_str())),
    }
}

/// Map an item `<stat type="..">` name to the engine [`Stat`] it feeds.
/// Returns `None` for stat kinds the finalizers don't compute yet (elemental
/// power/resistance, shield defence, `sDef`, `moveSpeed`, …); those are dropped
/// rather than silently miscredited to a related stat. `pAtkRange`/
/// `randomDamage` are handled by the caller (they aren't `Stat`s).
fn stat_from_xml(name: &str) -> Option<Stat> {
    Some(match name {
        "pAtk" => Stat::PhysicalAttack,
        "mAtk" => Stat::MagicalAttack,
        "pDef" => Stat::PhysicalDefence,
        "mDef" => Stat::MagicalDefence,
        "pAtkSpd" => Stat::PhysicalAttackSpeed,
        "mAtkSpd" => Stat::MagicAttackSpeed,
        "rCrit" => Stat::CriticalRate,
        "mCritRate" => Stat::MagicCriticalRate,
        "accCombat" => Stat::AccuracyCombat,
        "accMagic" => Stat::AccuracyMagic,
        "rEvas" => Stat::EvasionRate,
        "mEvas" => Stat::MagicEvasionRate,
        "maxHp" => Stat::MaxHp,
        "maxMp" => Stat::MaxMp,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_short_sword_and_adena() {
        let data = ItemData::load_from(crate::data::DIST_GAME);
        let sword = data.get(1).expect("item 1 (Short Sword)");
        assert_eq!(sword.name, "Short Sword");
        assert_eq!(sword.kind, ItemKind::Weapon);
        assert_eq!(sword.body_part, SLOT_R_HAND);
        assert!(sword.is_equipable());
        // No-grade weapon → CrystalType::None (level 0), so it never penalizes.
        assert_eq!(sword.crystal_type, CrystalType::None);
        assert_eq!(sword.crystal_type.level(), 0);

        // Ranged weapons (G20): Short Bow 13 costs MP per shot and reaches 500;
        // Wooden Arrow 17 is ARROW ammunition. Both are no-grade, so they match.
        let bow = data.get(13).expect("Short Bow 13");
        assert_eq!(bow.mp_consume, 1, "a bow spends MP per shot");
        assert_eq!(bow.crystal_type, CrystalType::None);
        let arrow = data.get(17).expect("Wooden Arrow 17");
        assert_eq!(arrow.etc_item_type, EtcItemType::Arrow);
        assert_eq!(
            arrow.crystal_type,
            CrystalType::None,
            "matches the no-grade bow"
        );
        // A melee weapon spends nothing.
        assert_eq!(data.get(2).map(|t| t.mp_consume), Some(0));

        // Melee sweep geometry (G20): a polearm reaches further than a sword,
        // both with a 120-degree arc (`damage_range` = a;b;radius;angle).
        let polearm = data.get(15).expect("polearm 15");
        assert_eq!((polearm.attack_radius, polearm.attack_angle), (66, 120));
        assert_eq!((sword.attack_radius, sword.attack_angle), (40, 120));

        // G15 item-cast slice: the flags `ItemSkillsTemplate` branches on.
        // A Scroll of Escape is *not* immediate — it casts its 20 s skill —
        // while a Healing Potion is, so it fires instantly. Both are
        // SKILL_REDUCE, which is what makes `checkConsume` spend them.
        let soe = data.get(736).expect("Scroll of Escape 736");
        assert!(!soe.immediate_effect, "SoE must take the cast branch");
        assert!(!soe.ex_immediate_effect);
        assert_eq!(soe.default_action, ActionType::SkillReduce);
        assert_eq!(soe.item_skills, vec![(2013, 1)]);
        let potion = data.get(1060).expect("Healing Potion 1060");
        assert!(potion.immediate_effect, "potions stay instant");
        assert_eq!(potion.default_action, ActionType::SkillReduce);
        // Packs are CAPSULE + immediate.
        let pack = data.get(22599).expect("spiritshot pack 22599");
        assert!(pack.immediate_effect);
        assert_eq!(pack.default_action, ActionType::Capsule);

        // A graded item parses its <set name="crystal_type"/>.
        let boots = data.get(40).expect("item 40 (Leather Boots)");
        assert_eq!(boots.crystal_type, CrystalType::D);
        assert_eq!(boots.crystal_type.level(), 1);

        let adena = data.get(ADENA_ID).expect("adena");
        assert!(adena.is_stackable);
        assert_eq!(adena.type2, TYPE2_MONEY);
        assert!(!adena.is_equipable());

        // Shots: the D-grade soulshot (1463) resolves to the SoulShots handler
        // and carries its NORMAL visual skill; a graded weapon declares a shot
        // count so it can charge (Java `Weapon._soulShotCount`).
        let soulshot = data.get(1463).expect("item 1463 (Soulshot D)");
        assert_eq!(soulshot.handler, ItemHandler::SoulShots);
        assert!(soulshot.item_skills.iter().any(|&(id, _)| id == 2150));
        assert_eq!(soulshot.crystal_type, CrystalType::D);
        // Some real weapon must declare soulshots/spiritshots.
        assert!(
            data.weapon_shots.values().any(|&(ss, _)| ss > 0),
            "a weapon declares a soulshot count"
        );
        assert!(
            data.weapon_shots.values().any(|&(_, sps)| sps > 0),
            "a weapon declares a spiritshot count"
        );

        assert!(data.by_id.len() > 5000);
    }

    #[test]
    fn parses_extractable_pack_handler_and_capsules() {
        let data = ItemData::load_from(crate::data::DIST_GAME);
        let pack = data
            .get(15195)
            .expect("item 15195 (Mage Class Equipment Set, 10-day)");
        assert_eq!(pack.handler, ItemHandler::ExtractableItems);
        assert_eq!(pack.extractable_count_min, 0);
        assert_eq!(pack.extractable_count_max, 0);
        assert_eq!(pack.capsuled_items.len(), 9);
        let robe = pack
            .capsuled_items
            .iter()
            .find(|c| c.item_id == 15230)
            .expect("Dark Crystal Robe pack entry");
        assert_eq!(robe.min, 1);
        assert_eq!(robe.max, 1);
        assert_eq!(robe.chance, 100_000); // chance="100" -> (100.0 * 1000) as i32

        let box_item = data
            .get(23762)
            .expect("item 23762 (High-grade Elixir Pack)");
        assert_eq!(box_item.extractable_count_min, 1);
        assert_eq!(box_item.extractable_count_max, 1);
    }

    /// The datapack's transfer restrictions: *Mage Class Equipment Set
    /// (10-day)* (15195) is bound — untradable, undroppable, unsellable and
    /// time-limited — while an ordinary item declares none of the tags and so
    /// inherits Java's permissive defaults.
    #[test]
    fn parses_bound_item_trade_flags() {
        let data = ItemData::load_from(crate::data::DIST_GAME);
        let bound = data
            .get(15195)
            .expect("item 15195 (Mage Class Equipment Set, 10-day)");
        assert!(!bound.is_dropable(), "is_dropable=false in the XML");
        assert!(!bound.is_tradable(), "is_tradable=false in the XML");
        assert!(!bound.is_sellable, "is_sellable=false in the XML");
        assert!(bound.is_time_limited(), "time=14400 makes it expire");
        // Storing and destroying stay available: a private warehouse takes
        // untradable items, and nothing marks the box undestroyable.
        assert!(bound.is_depositable(true), "a private warehouse takes it");
        assert!(!bound.is_depositable(false), "the clan warehouse does not");
        assert!(bound.is_destroyable(), "it can still be deleted");

        // An ordinary item declares none of the tags → all defaults true.
        let sword = data.get(1).expect("item 1 (Short Sword)");
        assert!(sword.is_dropable());
        assert!(sword.is_tradable());
        assert!(sword.is_destroyable());
        assert!(sword.is_depositable(false));
        assert!(!sword.is_time_limited());

        // Quest items are never depositable (Java forces the flag off).
        let quest = data
            .all()
            .find(|t| t.is_quest_item)
            .expect("at least one quest item");
        assert!(
            !quest.is_depositable(true),
            "quest items stay out of the WH"
        );
    }

    #[test]
    fn parses_item_icons_with_fallback() {
        let data = ItemData::load_from(crate::data::DIST_GAME);
        // Adena carries an explicit `<set name="icon">`.
        assert_eq!(data.icon(57), "icon.etc_adena_i00");
        // An unknown item falls back to the client question-mark (Java default).
        assert_eq!(data.icon(-1), "icon.etc_question_mark_i00");
        // `all()` yields the loaded catalog (Java `getAllItems`).
        assert!(
            data.all().any(|i| i.item_id == 57),
            "adena is in the catalog"
        );
    }

    #[test]
    fn parses_weapon_and_armor_stats() {
        let data = ItemData::load_from(crate::data::DIST_GAME);

        // Short Sword (item 1): pAtk/mAtk/rCrit/pAtkSpd + range/random-damage.
        let sword = data.item_stats(1).expect("item 1 <stats>");
        let get = |s: Stat| {
            sword
                .bonuses
                .iter()
                .find(|(st, _)| *st == s)
                .map(|(_, v)| *v)
        };
        assert_eq!(get(Stat::PhysicalAttack), Some(8.0));
        assert_eq!(get(Stat::MagicalAttack), Some(6.0));
        assert_eq!(get(Stat::CriticalRate), Some(8.0));
        assert_eq!(get(Stat::PhysicalAttackSpeed), Some(379.0));
        assert_eq!(sword.atk_range, Some(40)); // pAtkRange (not a Stat)
        assert_eq!(sword.random_damage, Some(10)); // randomDamage (not a Stat)

        // Leather Boots (item 40): a single pDef contribution.
        let boots = data.item_stats(40).expect("item 40 <stats>");
        assert_eq!(boots.bonuses, vec![(Stat::PhysicalDefence, 19.0)]);

        // Hoplon (item 628): a shield — sDef/rShld parsed into the shield fields
        // (not the Stat bonus list), rEvas into the sum-add bonuses.
        let hoplon = data.item_stats(628).expect("item 628 <stats>");
        assert_eq!(hoplon.shield_def, Some(128));
        assert_eq!(hoplon.shield_rate, Some(20));
        assert_eq!(
            hoplon
                .bonuses
                .iter()
                .find(|(s, _)| *s == Stat::EvasionRate)
                .map(|(_, v)| *v),
            Some(-8.0)
        );

        // Stackable/etc items with no <stats> have no side-map entry.
        assert!(data.item_stats(ADENA_ID).is_none());
    }
}

#[cfg(test)]
mod for_test_is_faithful {
    use super::*;

    /// Every field the fixture reduction dropped, asserted against what
    /// `for_test()` actually produces. If this drifts, the reduced fixtures
    /// silently change meaning.
    #[test]
    fn dropped_fields_match_the_base() {
        let t = ItemTemplate::for_test();
        assert_eq!(t.trade_flags, TradeFlags::default());
        assert_eq!(t.time, -1);
        assert_eq!(t.duration, -1);
        assert!(!t.immediate_effect);
        assert!(!t.ex_immediate_effect);
        assert_eq!(t.default_action, ActionType::Other);
        assert_eq!(t.weight, 0);
        assert!(!t.is_infinite);
        assert_eq!(t.type1, 0);
        assert_eq!(t.type2, 0);
        assert!(t.is_sellable);
        assert!(!t.is_freightable);
        assert_eq!(t.price, 0);
        assert_eq!(t.handler, ItemHandler::None);
        assert_eq!(t.crystal_type, CrystalType::None);
        assert_eq!(t.crystal_count, 0);
        assert_eq!(t.attack_radius, 40);
        assert_eq!(t.attack_angle, 0);
        assert_eq!(t.mp_consume, 0);
        assert_eq!(t.reduced_mp_consume, 0);
        assert_eq!(t.reduced_mp_consume_chance, 0);
        assert!(t.capsuled_items.is_empty());
        assert_eq!(t.extractable_count_min, 0);
        assert_eq!(t.extractable_count_max, 0);
        assert!(t.item_skills.is_empty());
        assert_eq!(t.etc_item_type, EtcItemType::Other);
        assert!(!t.enchant_enabled);
        assert_eq!(t.enchant_limit, 0);
        assert!(!t.is_magic_weapon);
    }
}
