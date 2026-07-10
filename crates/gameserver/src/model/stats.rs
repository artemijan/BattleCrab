//! Port of `model/stats/Stat.java` (`BaseStat`/`Stat` enums) — scoped to the
//! subset G6 actually computes/modifies. Java's `Stat` has ~230 entries and
//! `BaseStat` wraps 8 of them (`STR`/`INT`/`DEX`/`WIT`/`CON`/`MEN`/`CHA`/`LUC`);
//! grow both as later milestones need more names, exactly like the existing
//! `UserInfoType`/`InventorySlot` enums in `enums.rs`.

/// The six primary stats used by G6 (Java's `BaseStat` also has `CHA`/`LUC`,
/// unused by anything ported so far).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaseStat {
    Str,
    Dex,
    Con,
    Int,
    Wit,
    Men,
}

impl BaseStat {
    /// The `<TAG>` block name in `data/stats/statBonus.xml`.
    pub fn xml_tag(self) -> &'static str {
        match self {
            BaseStat::Str => "STR",
            BaseStat::Dex => "DEX",
            BaseStat::Con => "CON",
            BaseStat::Int => "INT",
            BaseStat::Wit => "WIT",
            BaseStat::Men => "MEN",
        }
    }
}

/// A modifiable creature stat (Java `Stat`). Each variant that has a Java
/// "finalizer" gets a `Player::recalculate_stats` case; the rest are only
/// targets of buff effects (`StatModifierEffect`) for now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stat {
    PhysicalAttack,
    PhysicalDefence,
    MagicalAttack,
    MagicalDefence,
    PhysicalAttackSpeed,
    MagicAttackSpeed,
    CriticalRate,
    MagicCriticalRate,
    EvasionRate,
    MagicEvasionRate,
    AccuracyCombat,
    AccuracyMagic,
    RunSpeed,
    WalkSpeed,
    SwimRunSpeed,
    SwimWalkSpeed,
    RegenerateHpRate,
    RegenerateMpRate,
    RegenerateCpRate,
    MaxHp,
    MaxMp,
    MaxCp,
}

/// Java `StatModifierType` (`AbstractStatAddEffect`/`AbstractStatPercentEffect`):
/// a buff either adds a flat amount or multiplies by `1 + amount/100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatModifierType {
    Diff,
    Per,
}
