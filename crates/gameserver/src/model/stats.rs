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
    /// Parse the `<stat>` name a `SkillMastery` effect declares.
    ///
    /// **Deliberately by name, never by ordinal.** Java's `BaseStat` is
    /// `STR, INT, DEX, WIT, CON, MEN, CHA, LUC`; this enum is
    /// `Str, Dex, Con, Int, Wit, Men`. `SkillMastery` stores the *ordinal* and
    /// `calcSkillMastery` reads it back with `BaseStat.values()[val]`, so
    /// copying Java's number across would silently select the wrong stat —
    /// Skill Mastery 331 (INT) would come out as DEX.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "STR" => Self::Str,
            "DEX" => Self::Dex,
            "CON" => Self::Con,
            "INT" => Self::Int,
            "WIT" => Self::Wit,
            "MEN" => Self::Men,
            // CHA and LUC exist in Java's enum but have no `statBonus` table
            // here and no skill on this dist names them.
            _ => return None,
        })
    }

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
    /// Blessed Shield (`ShieldDefenceRate`) and Shield Mastery (153) raise it;
    /// folded into `game_loop::combat::shield_stats` before the CON-bonus
    /// multiply, matching `ShieldDefenceRateFinalizer`'s `defaultValue(base *
    /// mul + add)` over the shield's own `rShld`.
    ShieldDefenceRate,
    /// Java `Stat.SHIELD_DEFENCE` ("sDef") — the flat defence added to pDef on
    /// a successful shield block (`Formulas.calcShldUse`'s `SHIELD_SUCCEED`
    /// branch). `ShieldDefence` (153 Shield Mastery, 322 Shield Fortress, 429
    /// Knighthood, …) raises it; folded the same way as `ShieldDefenceRate`,
    /// over the shield's own `sDef` (`ShieldDefenceFinalizer`'s
    /// `calcWeaponPlusBaseValue`).
    ShieldDefence,
    /// Java `Stat.ATTACK_COUNT_MAX` ("atkCountMax") — how many creatures one
    /// swing may hit. 1 by default; Polearm Mastery 216 (`HitNumber`) raises it
    /// to 5, which is what turns a polearm into a sweep weapon.
    AttackCountMax,
    /// Java `Stat.HEAL_EFFECT` ("healEffect", multiplicative) and
    /// `HEAL_EFFECT_ADD` ("healEffectAdd", additive) — how much healing the
    /// *recipient* actually receives. `Heal` applies them as
    /// `amount = amount * HEAL_EFFECT + HEAL_EFFECT_ADD`. Touch of Life 341
    /// (`+30 PER`) raises it; Touch of Death 342 (`-30 PER`) cuts it.
    HealEffect,
    HealEffectAdd,
    /// Java `Stat.RESIST_ABNORMAL_DEBUFF` ("debuffVuln") — a **multiplier on
    /// incoming debuff landing chance** (`Formulas.calcEffectSuccess`'s
    /// `buffDebuffMod`). Below 1 = resistant (Guts `amount=-50` → ×0.5), above
    /// 1 = vulnerable (Touch of Death `amount=+30` → ×1.3).
    ResistAbnormalDebuff,
    /// Java `Stat.ABNORMAL_RESIST_PHYSICAL` / `_MAGICAL` — read by
    /// `Formulas.getAbnormalResist` and subtracted from a mesmerizing debuff's
    /// base landing chance. Fed by the `PhysicalAbnormalResist` /
    /// `MagicalAbnormalResist` effects, which have no reachable source on this
    /// dist (G34 S2).
    /// Java `Stat.BREATH` — how long a swimmer can hold their breath, in ms,
    /// against a base of 60 000 (`Player.startWaterTask`'s
    /// `getValue(Stat.BREATH, 60000)`). Boost Breath (195) and Eva's Kiss
    /// (1073) are the learnable sources; the Doom armour set adds 19 more.
    /// Java `Stat.PHYSICAL_SKILL_POWER` / `MAGICAL_SKILL_POWER` — a flat
    /// multiplier on a *skill's* damage, applied last. Java reads the physical
    /// one from each `PhysicalAttack`-family effect handler
    /// (`damage *= getValue(PHYSICAL_SKILL_POWER, 1)`) and the magical one
    /// inside `calcMagicDam`. Focus Skill Mastery (334) is the learnable
    /// source of the first; the second is item-only here.
    /// Java `Stat.HATE_ATTACK` — multiplies the hate an **auto-attack**
    /// generates on an `Attackable` (`reduceCurrentHp`'s `if (skill == null)`
    /// branch). Sword/Blunt Weapon Mastery (217) is the learnable source, and
    /// the skill-exclusion is the point: it helps a tank hold aggro through
    /// ordinary swings, not through taunts.
    /// Java `Stat.DAMAGE_ZONE_VULN` — a **vulnerability** percentage on
    /// damage-zone ticks: `multiplier = 1 + (value / 100)`. Iron Body (295)
    /// and Dance of Protection (311) grant it *negative* (−40 / −30), so the
    /// stat's name notwithstanding, the learnable sources are mitigation.
    /// Java has no `Stat` for this — `EnlargeAbnormalSlot` calls
    /// `setMaxBuffCount(getMaxBuffCount() + slots)` directly. Modelled as a
    /// stat here **on purpose**: `apply_buff` rebuilds `StatModifiers` from the
    /// surviving buffs on every change, so the bonus is *derived* rather than
    /// accumulated and cannot drift the way an add/subtract pair can when a
    /// buff is dropped by some other path. Divine Inspiration (1405) is the
    /// learnable source (+1..+6 slots).
    /// Java `Stat.SKILL_MASTERY` — **not a magnitude**: `SkillMastery` stores
    /// the *ordinal of the `BaseStat`* that drives the proc chance (STR for
    /// Skill Mastery 330, INT for 331), and `calcSkillMastery` reads it back
    /// with `BaseStat.values()[val]`. `-1` (absent) means no mastery at all.
    SkillMastery,
    /// Java `Stat.SKILL_MASTERY_RATE` — the multiplier on that chance
    /// (Focus Skill Mastery 334).
    SkillMasteryRate,
    /// Java `Stat.MAX_CUBIC` — **dead in Java**. `CubicMastery` (143) is the
    /// only writer and *nothing in the entire tree reads it* (grepped `java/`
    /// and `dist/game/data/scripts/`); the cubic limit comes from
    /// `Config.ALLOWED_CUBIC_COUNT` instead. Registered so the effect parses
    /// and the skill's buff is not dropped whole, with no consumer — the same
    /// shape as `effect_flag::ABNORMAL_SHIELD`.
    /// Java `Stat.MAX_RECOVERABLE_HP` / `_CP` — the ceiling a **heal** may
    /// restore to, `getValue(stat, getMaxHp())` so identity is the full pool.
    /// Noblesse Harmony (1326) and Symphony (1327) grant them **negative**
    /// (`PER −30` / `−40`), so under those auras you can only be healed back to
    /// 70 % HP / 60 % CP — the name is literal, and the learnable sources are
    /// restrictions rather than bonuses.
    MaxRecoverableHp,
    MaxRecoverableCp,
    MaxCubic,
    MaxBuffSlots,
    DamageZoneVuln,
    /// Java `Stat.TRANSFER_DAMAGE_SUMMON_PERCENT` — the share of incoming
    /// player damage redirected to a nearby servitor (Transfer Pain 1262).
    TransferDamageSummonPercent,
    /// Java `Stat.VENGEANCE_SKILL_PHYSICAL_DAMAGE` — the *chance* (not a
    /// multiplier) that a **melee physical skill** hitting its bearer is
    /// countered (Shield of Revenge 439 at 20 %, Counterattack 447 at 90 %).
    VengeanceSkillPhysicalDamage,
    HateAttack,
    PhysicalSkillPower,
    MagicalSkillPower,
    /// Java `Stat.PHYSICAL_SKILL_CRITICAL_DAMAGE` and its defence twin — the
    /// crit multiplier for a *physical skill*, which `Formulas.calcCritDamage`
    /// reads instead of `CRITICAL_DAMAGE` when a skill is involved. Heroic
    /// Berserker (396) is the learnable source.
    PhysicalSkillCriticalDamage,
    DefencePhysicalSkillCriticalDamage,
    /// Java's PvP/PvE damage balance stats, read by one function each —
    /// `Formulas.calculatePvpPveBonus`, which is a term in *every* damage
    /// formula. All of them are `AbstractStatPercentEffect`s, so they merge as
    /// **muls** (`amount 5` → ×1.05) and the bonus is
    /// `max(0.05, 1 + (attackerMul − targetMul))` — a difference of
    /// multipliers added to 1, not a product.
    ///
    /// The branch is picked by *who* is fighting and *how*: playable-vs-playable
    /// takes the PVP triple, anything involving an `Attackable` takes the PVE
    /// one, and inside each, a magic skill / a physical skill / an auto-attack
    /// (Java's `skill == null`) read a different pair.
    PvpPhysicalAttackDamage,
    PvpPhysicalAttackDefence,
    PvpPhysicalSkillDamage,
    PvpPhysicalSkillDefence,
    PvpMagicalSkillDamage,
    PvpMagicalSkillDefence,
    PvePhysicalAttackDamage,
    PvePhysicalAttackDefence,
    PvePhysicalSkillDamage,
    PvePhysicalSkillDefence,
    PveMagicalSkillDamage,
    PveMagicalSkillDefence,
    /// The raid trio. Java reads all three off the **attacker** — including the
    /// `*_DEFENCE` half, which is almost certainly an upstream slip (every
    /// other defence term reads the target) but is ported as written, and is
    /// inert here regardless: the only carriers are three item skills, and
    /// they are only consulted when the attacker `isRaid()`.
    PveRaidPhysicalAttackDefence,
    PveRaidPhysicalSkillDefence,
    PveRaidMagicalSkillDefence,
    Breath,
    /// Java `Stat.WEIGHT_LIMIT` / `WEIGHT_PENALTY` — `Creature.getMaxLoad()`
    /// (`getValue(WEIGHT_LIMIT, CON bonus × 69000 × config)`) and
    /// `getBonusWeightPenalty()` (`getValue(WEIGHT_PENALTY, 1)`), the extra
    /// penalty *bands* a skill grants before the overload steps bite.
    WeightLimit,
    WeightPenalty,
    AbnormalResistPhysical,
    AbnormalResistMagical,
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
    /// Java `Stat.MAGIC_SUCCESS_RES` ("magicSuccRes") — the target's general
    /// magic resistance, read by `Formulas.calcMagicSuccess` as
    /// `getMul(MAGIC_SUCCESS_RES, 1)`. It multiplies the *failure* term, so a
    /// value above 1 lowers the attacker's success rate.
    ///
    /// Granted by Anti Magic 146 and M. Def. 147 (`ResistDDMagic`, an
    /// `AbstractStatPercentEffect`, so it merges multiplicatively and
    /// `getMul` does see it).
    MagicSuccessRes,
    /// Java `Stat.DEFENCE_CRITICAL_RATE` / `DEFENCE_CRITICAL_RATE_ADD` — the
    /// **defender's** modifier on an incoming autoattack's crit *chance*
    /// (Light Armor Mastery 233 at `-15% PER`, Pa'agrio's Eye 1364 at `-30%`).
    ///
    /// Read as `getValue(DEFENCE_CRITICAL_RATE, attackerRate)` — the two-arg
    /// form, so the defender's multiplier scales the *attacker's* rate rather
    /// than standing alone.
    DefenceCriticalRate,
    DefenceCriticalRateAdd,
    /// Java `Stat.DEFENCE_CRITICAL_DAMAGE` — the **target-side** crit-damage
    /// multiplier (`DefenceCriticalDamage`, an `AbstractStatEffect` over the
    /// same mul/add pair as `CriticalDamage`). Read off the victim in
    /// `calcCritDamage`'s autoattack branch.
    DefenceCriticalDamage,
    /// Java `Stat.DEFENCE_CRITICAL_DAMAGE_ADD` — its flat twin.
    DefenceCriticalDamageAdd,
    /// Java `Stat.MAGIC_CRITICAL_DAMAGE` — the magic-crit multiplier
    /// (Prophecy of Wind 1357, Victories of Pa'agrio 1414), read in
    /// `calcCritDamage`'s `skill.isMagic()` branch.
    MagicCriticalDamage,
    /// Java `Stat.DEFENCE_MAGIC_CRITICAL_DAMAGE` — the target-side twin. No
    /// learnable skill grants it on this dist (7 non-learnable ones do), so it
    /// is folded for completeness and reads as the 1.0 default in practice.
    DefenceMagicCriticalDamage,
    /// Java `Stat.REFLECT_SKILL_PHYSIC` / `REFLECT_SKILL_MAGIC` — the percent
    /// chance that an incoming **debuff** is bounced back onto its caster
    /// (Riposte Stance 340, Physical Mirror 350, Magical Mirror 351). Additive,
    /// read off the *target* in `Formulas.calcBuffDebuffReflection`. Which of
    /// the two applies is decided by the incoming skill's `isMagic`, not by the
    /// defender.
    ReflectSkillPhysic,
    ReflectSkillMagic,
    /// Java `Stat.REFLECT_DAMAGE_PERCENT` — the percentage of *damage taken*
    /// bounced back at the attacker, granted by `DamageShield` (Reflect Damage
    /// 86, Riposte Stance 340, Blazing/Freezing Skin 1232/1238, Chant of
    /// Revenge 1284, Song of Vengeance 305). Additive, read off the **target**
    /// in `Creature.doAttack` — not to be confused with
    /// [`Stat::ReflectSkillPhysic`], which bounces a whole *debuff* rather than
    /// damage.
    ReflectDamagePercent,
    /// Java `Stat.ABSORB_DAMAGE_PERCENT` — the share of melee damage dealt that
    /// comes back as HP (`VampiricAttack`: Vampiric Rage 1268, Dance of the
    /// Vampire 310, Chant of Vampire 1310, Prophecy of Wind 1357). Java stores
    /// the effect's `amount / 100`, so a `<amount>8</amount>` reads as 0.08.
    AbsorbDamagePercent,
    /// Java's `CreatureStat._vampiricSum` — **not** a `Stat` upstream but a
    /// hand-managed accumulator fed by the same effect (`amount · chance`).
    /// Kept here so both halves ride the ordinary modifier machinery; the pair
    /// is what `VampiricChanceFinalizer` turns into an actual chance:
    /// `min(1, vampiricSum / (absorbPercent · 100) / 100)`, i.e. the
    /// absorb-weighted mean of the contributing buffs' own chances.
    VampiricSum,
    /// Java `Stat.ABSORB_MANA_DAMAGE_PERCENT` + the `mpVampiricSum` accumulator
    /// — the MP twin of [`Self::AbsorbDamagePercent`]/[`Self::VampiricSum`],
    /// granted by `MpVampiricAttack` (Weapon Mastery 250). The chance is
    /// `min(1, mpVampiricSum / (percent × 100) / 100)`, exactly as the HP one.
    AbsorbManaDamagePercent,
    MpVampiricSum,
    /// Java `Stat.MANA_CHARGE` ("manaCharge") — a flat bonus on the amount a
    /// *recharge* skill restores, granted by Higher Mana Gain 285 (`ManaCharge`,
    /// `mode=DIFF`, +22..81 by level). Read off the **recipient** by
    /// `ManaHeal`/`ManaHealByLevel` as `getValue(MANA_CHARGE, amount)` —
    /// i.e. `mul * amount + add`, so a DIFF grant is a flat addition.
    ManaCharge,
    /// Java `Stat.INVENTORY_NORMAL` ("inventoryLimit") — a flat bonus on top
    /// of the race-based inventory-slot base. Expand Inventory (1372,
    /// `EnlargeSlot` with no `<type>`, which defaults to this) raises it.
    InventoryNormal,
    /// Java `Stat.STORAGE_PRIVATE` ("whLimit") — private-warehouse slot bonus.
    /// Expand Warehouse (1371, `EnlargeSlot` type=STORAGE_PRIVATE) raises it.
    StoragePrivate,
    /// Java `Stat.TRADE_SELL`/`TRADE_BUY` ("tradeSellLimit"/"tradeBuyLimit") —
    /// private-store listing slot bonuses. Expand Trade (1370) carries two
    /// `EnlargeSlot` effects, one per stat.
    TradeSell,
    TradeBuy,
    /// Java `Stat.RECIPE_DWARVEN`/`RECIPE_COMMON` ("dwarfRecipeLimit"/
    /// "commonRecipeLimit") — recipe-book slot bonuses. Expand Dwarven Craft
    /// (1368) and Expand Common Craft (1369) raise them.
    RecipeDwarven,
    RecipeCommon,
    /// Java `Stat.PHYSICAL_ATTACK_RANGE` ("pAtkRange") — the melee/bow reach
    /// added on top of the equipped weapon's own range (`PRangeFinalizer`).
    /// Archery 431/Snipe 972 (`DIFF`, bow-conditioned) raise it; Long Shot 113
    /// too (level-scaled DIFF); Rapid Fire 413 cuts it (`PER -50`, a stance
    /// trading range for reload speed) — all four gated on `<weaponType>BOW`.
    PhysicalAttackRange,
    /// Java `Stat.BLOW_RATE` ("blowRate", multiplicative) — a factor on the
    /// `Blow`/`FatalBlow` landing roll (`Formulas.calcBlowSuccess`). Focus
    /// Death 355, Critical Blow 409, Mortal Strike 410 and Assassination 432
    /// (`FatalBlowRate`, all `PER`) raise it. `Stat.BLOW_RATE_DEFENCE`
    /// (`FatalBlowRateDefence`) is *not* ported — grepped the whole datapack,
    /// nothing grants it, matching `INSTANT_KILL_RESIST`/`MAX_MOMENTUM`'s
    /// established "dead in Java too" pattern.
    BlowRate,
    /// The elemental attack values — Java `Stat.FIRE_POWER`… (`AttributeFinalizer`,
    /// PLAN_G19_ATTRIBUTES.md). Fed by `AttackAttribute` effects (Holy Weapon
    /// 1043's `HOLY +20`); read by `Formulas.calcAttributeBonus`'s attack side.
    FirePower,
    WaterPower,
    WindPower,
    EarthPower,
    HolyPower,
    DarkPower,
    /// The elemental defence values — Java `Stat.FIRE_RES`…. Base comes from
    /// the NPC template's `<attribute><defence …/>`; `DefenceAttribute`
    /// effects (the Resist Fire family, Day of Doom's −50s) merge on top.
    FireRes,
    WaterRes,
    WindRes,
    EarthRes,
    HolyRes,
    DarkRes,
}

/// Java `AttributeType` minus `NONE` (represented as `Option<Element>`): the
/// six combat elements. Order matches the client ids (`findByClientId`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Element {
    Fire,
    Water,
    Wind,
    Earth,
    Holy,
    Dark,
}

impl Element {
    pub const ALL: [Element; 6] = [
        Element::Fire,
        Element::Water,
        Element::Wind,
        Element::Earth,
        Element::Holy,
        Element::Dark,
    ];

    pub fn from_xml(name: &str) -> Option<Self> {
        Some(match name {
            "FIRE" => Element::Fire,
            "WATER" => Element::Water,
            "WIND" => Element::Wind,
            "EARTH" => Element::Earth,
            "HOLY" => Element::Holy,
            "DARK" => Element::Dark,
            _ => return None,
        })
    }

    /// Index into per-element arrays (client-id order).
    pub fn index(self) -> usize {
        match self {
            Element::Fire => 0,
            Element::Water => 1,
            Element::Wind => 2,
            Element::Earth => 3,
            Element::Holy => 4,
            Element::Dark => 5,
        }
    }

    /// `Stat.valueOf(attribute + "_POWER")`.
    pub fn power_stat(self) -> Stat {
        match self {
            Element::Fire => Stat::FirePower,
            Element::Water => Stat::WaterPower,
            Element::Wind => Stat::WindPower,
            Element::Earth => Stat::EarthPower,
            Element::Holy => Stat::HolyPower,
            Element::Dark => Stat::DarkPower,
        }
    }

    /// `Stat.valueOf(attribute + "_RES")`.
    pub fn res_stat(self) -> Stat {
        match self {
            Element::Fire => Stat::FireRes,
            Element::Water => Stat::WaterRes,
            Element::Wind => Stat::WindRes,
            Element::Earth => Stat::EarthRes,
            Element::Holy => Stat::HolyRes,
            Element::Dark => Stat::DarkRes,
        }
    }
}

impl Stat {
    /// Java `Stat.valueOf(name)` — the datapack's `<stat>` element name.
    ///
    /// Java's enum covers all ~200 stats; only the names actually used by a
    /// `<stat>` element in this dist are mapped, since that is the sole caller
    /// (`StatByMoveType`). An unmapped name yields `None` and the effect is
    /// dropped, matching Java's own behaviour on an unknown enum constant
    /// (`getEnum` throws and the handler is skipped with a log).
    pub fn from_xml(name: &str) -> Option<Self> {
        Some(match name {
            "REGENERATE_HP_RATE" => Stat::RegenerateHpRate,
            "REGENERATE_MP_RATE" => Stat::RegenerateMpRate,
            "REGENERATE_CP_RATE" => Stat::RegenerateCpRate,
            "EVASION_RATE" => Stat::EvasionRate,
            _ => return None,
        })
    }
}

/// Java `model/stats/MoveType` — the creature's current locomotion state,
/// which `StatByMoveType` effects are conditioned on.
///
/// Derived, not stored: Java computes it fresh in `Creature.getMoveType`
/// (`isMoving() && isRunning()` → `Running`, `isMoving()` → `Walking`, else
/// `Standing`), with `Player` overriding it to return `Sitting` while seated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoveType {
    Walking,
    Running,
    /// Derived from `Player.sitting` (the seated branch wins over movement, as
    /// in Java). The only skill in this dist that *conditions* on it is a
    /// non-learnable belt-item skill (13200), but the move-type also drives the
    /// seated regen bonus, which every character gets.
    Sitting,
    Standing,
}

impl MoveType {
    pub fn from_xml(name: &str) -> Option<Self> {
        Some(match name {
            "WALKING" => MoveType::Walking,
            "RUNNING" => MoveType::Running,
            "SITTING" => MoveType::Sitting,
            "STANDING" => MoveType::Standing,
            _ => return None,
        })
    }
}

/// A condition narrowing *when* a [`StatModifierEffect`](crate::model::skill::StatModifierEffect)
/// contributes. Java keeps one map per kind on `CreatureStat`, each with its
/// own merge function and identity — which is why these stay two maps on
/// [`StatModifiers`](crate::model::components::StatModifiers) rather than one:
///
/// | variant | Java map | merge | identity |
/// |---|---|---|---|
/// | [`Self::MoveType`] | `_moveTypeStats` | `MathUtil::add` | `0.0` |
/// | [`Self::Position`] | `_positionTypeStats` | `MathUtil::mul` | `1.0` |
///
/// Both are read at finalize time against the creature's *live* state, so the
/// value swings as they move or as the attacker circles the target, with no
/// stat recompute anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatQualifier {
    /// `StatByMoveType` — counts only in this locomotion state.
    MoveType(MoveType),
    /// `CriticalDamagePosition` — counts only when the *attacker* stands in
    /// this position relative to the target (Focus Death 355, Focus Power 357).
    Position(crate::model::movement::Position),
}

/// Java `StatModifierType` (`AbstractStatAddEffect`/`AbstractStatPercentEffect`):
/// a buff either adds a flat amount or multiplies by `1 + amount/100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatModifierType {
    Diff,
    Per,
}
