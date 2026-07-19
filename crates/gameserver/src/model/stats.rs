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
    /// Java `Stat.ATTACK_CANCEL` ("cancel") — modifies the chance a hit
    /// interrupts the target's cast (`Formulas.calcAtkBreak`). Concentration
    /// (`ReduceCancel`) lowers it.
    AttackCancel,
    /// Java `Stat.SHIELD_DEFENCE_RATE` ("rShld") — the shield block chance.
    /// Blessed Shield (`ShieldDefenceRate`) raises it. No finalizer yet, so it's
    /// only a buff-modifier target (the buff lands with its icon regardless).
    ShieldDefenceRate,
    /// Java `Stat.RESIST_ABNORMAL_DEBUFF` ("debuffVuln") — a **multiplier on
    /// incoming debuff landing chance** (`Formulas.calcEffectSuccess`'s
    /// `buffDebuffMod`). Below 1 = resistant (Guts `amount=-50` → ×0.5), above
    /// 1 = vulnerable (Touch of Death `amount=+30` → ×1.3).
    ResistAbnormalDebuff,
    /// Java `Stat.RESIST_DISPEL_BUFF` ("cancelVuln") — the same shape for being
    /// *dispelled*: Ultimate Defense (`amount=-80`) makes its buffs ×0.2 as
    /// likely to be cancelled.
    ResistDispelBuff,
    /// Java `Stat.CRITICAL_DAMAGE` ("cAtk", multiplicative) — the crit-damage
    /// multiplier. Death Whisper (`CriticalDamage`, `mode=PER`) raises it.
    CriticalDamage,
    /// Java `Stat.CRITICAL_DAMAGE_ADD` ("cAtkAdd", additive) — the flat
    /// crit-damage bonus that `CriticalDamage` effects with `mode=DIFF` feed
    /// (the same handler picks this stat over `CRITICAL_DAMAGE` for diff mode).
    CriticalDamageAdd,
}

/// Java `StatModifierType` (`AbstractStatAddEffect`/`AbstractStatPercentEffect`):
/// a buff either adds a flat amount or multiplies by `1 + amount/100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatModifierType {
    Diff,
    Per,
}
