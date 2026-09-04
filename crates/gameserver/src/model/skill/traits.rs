//! Java `TraitType` (`enums/TraitType.java`) — the attack/defence trait pairs
//! a skill can carry, plus the weapon/weakness groupings built on them.

/// Java `enums/TraitType` — the tag a debuff carries (`<trait>`) and the tag a
/// resistance buff raises (`DefenceTrait`'s params). The numeric group is
/// Java's `_type`, and it decides how `calcGeneralTraitBonus` treats the pair:
///
/// - **3** (`SHOCK`, `HOLD`, `SLEEP`, `POISON`, `DERANGEMENT`, `PARALYZE`,
///   `BLEED`, `DEATH`, …) — a plain resistance: the target's defence applies
///   with no attacker-side requirement. **This is the group the dist uses**:
///   304 skills carry `<trait>SHOCK</trait>`, 194 `DERANGEMENT`, and the
///   learnable Stun/Mental/Poison resistances defend exactly these.
/// - **2** (the `*_WEAKNESS` family) — needs the *attacker* to carry a matching
///   `AttackTrait` as well, which is why "Detect Beast Weakness" is inert on
///   this dist (nothing gives a monster the paired `DefenceTrait`).
/// - **1** (weapon types) and **0** (`NONE`) — never resisted this way.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum TraitType {
    #[default]
    None,
    /// Group 3 — the resistable debuff traits this dist actually uses.
    Poison,
    Hold,
    Bleed,
    Sleep,
    Shock,
    Derangement,
    Paralyze,
    Death,
    Boss,
    CriticalPoison,
    RootPhysically,
    RootMagically,
    TurnStone,
    Gust,
    PhysicalBlockade,
    Target,
    PhysicalWeakness,
    MagicalWeakness,
    Knockback,
    Knockdown,
    Pull,
    Hate,
    Aggression,
    Airbind,
    Disarm,
    Deport,
    Changebody,
    Zone,
    Psychic,
    /// Group 2 — the creature-category weaknesses (attacker-gated).
    Weakness(WeaknessTrait),
    /// Group 1 — the weapon types (and `ETC`). Distinct values so a bearer's
    /// SWORD and DAGGER resistances can't fold into one bucket, which
    /// `calcWeaponTraitBonus` reads to soften hits from that weapon type
    /// (Deflect Arrow's BOW, Provoke's negative POLE).
    Weapon(WeaponTrait),
    /// Anything unrecognised — treated as group 1, i.e. never resisted.
    Other,
}

/// The `*_WEAKNESS` members of `TraitType`, kept as one variant so the enum
/// stays small; they all share group 2 and the same attacker-gated rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WeaknessTrait {
    Bug,
    Animal,
    Plant,
    Beast,
    Dragon,
    Giant,
    Construct,
    Valakas,
    Anesthesia,
    Demonic,
    Divine,
    Elemental,
    Fairy,
    Human,
    Humanoid,
    Undead,
    Embryo,
    Spirit,
}

/// Java's group-1 `TraitType` members: the weapon types a `DefenceTrait` can
/// name, plus `ETC` (which 61 skills declare as their `<trait>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WeaponTrait {
    Sword,
    Blunt,
    Dagger,
    Pole,
    Fist,
    Bow,
    Etc,
    Dual,
    DualFist,
    Rapier,
    Crossbow,
    AncientSword,
    DualDagger,
    DualBlunt,
    TwoHandCrossbow,
}

impl TraitType {
    /// Java's `TraitType.getAllWeakness()` — the group-2 members, in Java's own
    /// order (the product is commutative, but the list is the authority on
    /// *which* traits count as a weakness).
    pub const ALL_WEAKNESS: [TraitType; 18] = {
        use WeaknessTrait as W;
        [
            TraitType::Weakness(W::Bug),
            TraitType::Weakness(W::Animal),
            TraitType::Weakness(W::Plant),
            TraitType::Weakness(W::Beast),
            TraitType::Weakness(W::Dragon),
            TraitType::Weakness(W::Giant),
            TraitType::Weakness(W::Construct),
            TraitType::Weakness(W::Valakas),
            TraitType::Weakness(W::Anesthesia),
            TraitType::Weakness(W::Demonic),
            TraitType::Weakness(W::Divine),
            TraitType::Weakness(W::Elemental),
            TraitType::Weakness(W::Fairy),
            TraitType::Weakness(W::Human),
            TraitType::Weakness(W::Humanoid),
            TraitType::Weakness(W::Undead),
            TraitType::Weakness(W::Embryo),
            TraitType::Weakness(W::Spirit),
        ]
    };

    /// Java `WeaponType.getTraitType()` — every weapon type *is* a trait, which
    /// is what `calcWeaponTraitBonus` looks the target's defence up by. The
    /// types Java maps to `NONE` (fishing rod, flag) stay `None`.
    pub fn of_weapon(weapon: crate::data::item_data::kinds::WeaponType) -> Self {
        use crate::data::item_data::kinds::WeaponType as W;
        use WeaponTrait as P;
        match weapon {
            W::Sword => Self::Weapon(P::Sword),
            W::Blunt => Self::Weapon(P::Blunt),
            W::Dagger => Self::Weapon(P::Dagger),
            W::Pole => Self::Weapon(P::Pole),
            W::DualFist => Self::Weapon(P::DualFist),
            W::Bow => Self::Weapon(P::Bow),
            W::Dual => Self::Weapon(P::Dual),
            W::DualBlunt => Self::Weapon(P::DualBlunt),
            W::Fist => Self::Weapon(P::Fist),
            W::Rapier => Self::Weapon(P::Rapier),
            W::Crossbow => Self::Weapon(P::Crossbow),
            W::AncientSword => Self::Weapon(P::AncientSword),
            W::DualDagger => Self::Weapon(P::DualDagger),
            W::TwoHandCrossbow => Self::Weapon(P::TwoHandCrossbow),
            // Java maps `WeaponType.NONE`/`FISHINGROD` to `TraitType.NONE`,
            // but a *creature* never reaches this with NONE: `getAttackType()`
            // falls back to `_template.getBaseAttackType()`, which is **FIST**
            // for every player template and — since no NPC row declares
            // `baseAtkType` — for every NPC here as well. The distinction only
            // shows up against a FIST defence trait, and the one in-chronicle
            // skill that grants one (5525, Chain Buff - Melee Resistance) is on
            // no NPC skill list and in no skill tree; 10338 is post-Interlude.
            // Left as `None` with the carrier named rather than mapped to FIST
            // on a guess about `of_weapon`'s other callers.
            W::None | W::FishingRod => Self::None,
        }
    }

    /// Java's `TraitType._type`.
    pub fn group(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Weakness(_) => 2,
            Self::Weapon(_) | Self::Other => 1,
            _ => 3,
        }
    }

    /// Parse a `<trait>` tag or a `DefenceTrait` param name. Unknown names fall
    /// to [`TraitType::Other`] (group 1) rather than being dropped, so a later
    /// chronicle's trait can't silently read as `None` and skip the gate.
    pub fn from_xml(name: &str) -> Self {
        use WeaknessTrait as W;
        use WeaponTrait as P;
        match name {
            "NONE" => Self::None,
            "POISON" => Self::Poison,
            "HOLD" => Self::Hold,
            "BLEED" => Self::Bleed,
            "SLEEP" => Self::Sleep,
            "SHOCK" => Self::Shock,
            "DERANGEMENT" => Self::Derangement,
            "PARALYZE" => Self::Paralyze,
            "DEATH" => Self::Death,
            "BOSS" => Self::Boss,
            "CRITICAL_POISON" => Self::CriticalPoison,
            "ROOT_PHYSICALLY" => Self::RootPhysically,
            "ROOT_MAGICALLY" => Self::RootMagically,
            "TURN_STONE" => Self::TurnStone,
            "GUST" => Self::Gust,
            "PHYSICAL_BLOCKADE" => Self::PhysicalBlockade,
            "TARGET" => Self::Target,
            "PHYSICAL_WEAKNESS" => Self::PhysicalWeakness,
            "MAGICAL_WEAKNESS" => Self::MagicalWeakness,
            "KNOCKBACK" => Self::Knockback,
            "KNOCKDOWN" => Self::Knockdown,
            "PULL" => Self::Pull,
            "HATE" => Self::Hate,
            "AGGRESSION" => Self::Aggression,
            "AIRBIND" => Self::Airbind,
            "DISARM" => Self::Disarm,
            "DEPORT" => Self::Deport,
            "CHANGEBODY" => Self::Changebody,
            "ZONE" => Self::Zone,
            "PSYCHIC" => Self::Psychic,
            "BUG_WEAKNESS" => Self::Weakness(W::Bug),
            "ANIMAL_WEAKNESS" => Self::Weakness(W::Animal),
            "PLANT_WEAKNESS" => Self::Weakness(W::Plant),
            "BEAST_WEAKNESS" => Self::Weakness(W::Beast),
            "DRAGON_WEAKNESS" => Self::Weakness(W::Dragon),
            "GIANT_WEAKNESS" => Self::Weakness(W::Giant),
            "CONSTRUCT_WEAKNESS" => Self::Weakness(W::Construct),
            "VALAKAS" => Self::Weakness(W::Valakas),
            "ANESTHESIA" => Self::Weakness(W::Anesthesia),
            "DEMONIC_WEAKNESS" => Self::Weakness(W::Demonic),
            "DIVINE_WEAKNESS" => Self::Weakness(W::Divine),
            "ELEMENTAL_WEAKNESS" => Self::Weakness(W::Elemental),
            "FAIRY_WEAKNESS" => Self::Weakness(W::Fairy),
            "HUMAN_WEAKNESS" => Self::Weakness(W::Human),
            "HUMANOID_WEAKNESS" => Self::Weakness(W::Humanoid),
            "UNDEAD_WEAKNESS" => Self::Weakness(W::Undead),
            "EMBRYO_WEAKNESS" => Self::Weakness(W::Embryo),
            "SPIRIT_WEAKNESS" => Self::Weakness(W::Spirit),
            "SWORD" => Self::Weapon(P::Sword),
            "BLUNT" => Self::Weapon(P::Blunt),
            "DAGGER" => Self::Weapon(P::Dagger),
            "POLE" => Self::Weapon(P::Pole),
            "FIST" => Self::Weapon(P::Fist),
            "BOW" => Self::Weapon(P::Bow),
            "ETC" => Self::Weapon(P::Etc),
            "DUAL" => Self::Weapon(P::Dual),
            "DUALFIST" => Self::Weapon(P::DualFist),
            "RAPIER" => Self::Weapon(P::Rapier),
            "CROSSBOW" => Self::Weapon(P::Crossbow),
            "ANCIENTSWORD" => Self::Weapon(P::AncientSword),
            "DUALDAGGER" => Self::Weapon(P::DualDagger),
            "DUALBLUNT" => Self::Weapon(P::DualBlunt),
            "TWOHANDCROSSBOW" => Self::Weapon(P::TwoHandCrossbow),
            _ => Self::Other,
        }
    }
}
