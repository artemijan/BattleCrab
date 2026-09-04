//! The item classification enums — Java's `ItemType` hierarchy
//! (`ArmorType`/`WeaponType`/`EtcItemType`), the crystal grade, and the
//! `handler`/`<action>` tags that decide what using the item does.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]

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
    /// `Enum.valueOf(CrystalType.class, …)` for `Character.ini`'s
    /// `MaxEquipableItemGrade`. Separate from [`Self::from_name`] because that
    /// one reads a datapack attribute and answers `None` for anything it does
    /// not know; an unreadable *config* value must not silently become "no
    /// grade", which would filter the entire shop catalogue away. Java throws
    /// here, so the port keeps the permissive end of the enum and says so.
    pub fn from_config_name(name: &str) -> Self {
        match name.trim().to_ascii_uppercase().as_str() {
            "NONE" => CrystalType::None,
            "D" => CrystalType::D,
            "C" => CrystalType::C,
            "B" => CrystalType::B,
            "A" => CrystalType::A,
            "S" => CrystalType::S,
            "S80" => CrystalType::S80,
            "S84" => CrystalType::S84,
            "R" => CrystalType::R,
            "R95" => CrystalType::R95,
            "R99" => CrystalType::R99,
            "EVENT" => CrystalType::Event,
            other => {
                tracing::warn!(
                    "MaxEquipableItemGrade: unknown crystal type {other:?}; using EVENT (no filter)"
                );
                CrystalType::Event
            }
        }
    }

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
    /// `handlers/itemhandlers/SummonItems` — **every pet collar on this dist**,
    /// the Wolf Collar included. `extends ItemSkillsTemplate`: it adds the
    /// summon guards, parks the item as Java's `PetItemHolder`, then casts the
    /// item's own skills like any other. Until this variant existed the name
    /// fell through to [`ItemHandler::None`] and a collar was eaten in silence,
    /// which left the whole pet system with no way in from the client.
    SummonItems,
    /// `handlers/itemhandlers/Book` — a readable book: opens
    /// `data/html/help/<itemId>.htm` in a dialog. Not consumed.
    Book,
    /// `handlers/itemhandlers/RollingDice` — the party dice (4625–4628).
    RollingDice,
    /// `handlers/itemhandlers/MercTicket` — a mercenary posting ticket. 499
    /// ship across the nine castles; using one inside your own castle's grounds
    /// asks for confirmation and then posts a defender at that exact spot.
    MercTicket,
    /// `handlers/itemhandlers/PetFood` — food used from the **owner's** bag.
    /// The pet eating out of its *own* inventory is a different packet
    /// (`RequestPetUseItem`, already ported); this arm is Java's other branch,
    /// where a mounted rider feeds the mount they are sitting on.
    PetFood,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
    pub fn from_name(name: Option<&str>) -> Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum EtcItemType {
    #[default]
    Other,
    /// `ARROW` / `BOLT` — bow and crossbow ammunition. Matched to the weapon by
    /// crystal grade (`findArrowForBow`) and auto-equipped into the left hand.
    Arrow,
    Bolt,
    /// `CASTLE_GUARD` — the mercenary posting tickets. The only thing that
    /// reads it is `Product.getPrice`'s `RateSiegeGuardsPrice` multiply.
    CastleGuard,
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
    pub(super) fn from_name(name: Option<&str>) -> Self {
        match name {
            Some("ARROW") => EtcItemType::Arrow,
            Some("BOLT") => EtcItemType::Bolt,
            Some("CASTLE_GUARD") => EtcItemType::CastleGuard,
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
