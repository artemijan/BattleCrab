//! The physical path: hit/miss, the critical rolls and their position and
//! height bonuses, shield block, and the auto-attack, physical-skill and
//! blow damage formulas.

use crate::model::movement::Position;

/// `Formulas.calcHitMiss`: chance-in-1000 =
/// `(80 + 2·(accuracy − evasion)) · 10 × conditionBonus`, clamped to
/// [200, 980]; the hit misses when `roll` (`Rnd.get(1000)`) lands above it.
pub fn calc_hit_miss(accuracy: i32, evasion: i32, condition_bonus: f64, roll: i32) -> bool {
    let chance = ((80 + (2 * (accuracy - evasion))) * 10) as f64 * condition_bonus;
    let chance = chance.clamp(200.0, 980.0);
    chance < roll as f64
}

/// `Formulas.calcCriticalPositionBonus`: 10 % from the side, 30 % from the
/// back, **times the attacker's positional `CRITICAL_RATE`** —
/// `getPositionTypeValue(Stat.CRITICAL_RATE, position)`, which
/// `CriticalRatePositionBonus` (Focus Chance 356) pumps. `position_mul` is that
/// value, identity 1.0 for anyone without the passive; it used to be hard-coded
/// there, which made Focus Chance inert (G34 S4).
///
/// Note Focus Chance is the one skill that declares **all three** positions —
/// −30 % front, +30 % side, +60 % back — so a rogue who circles is rewarded and
/// one who stands in front is punished. Dropping the front term would look like
/// a pure buff.
pub fn calc_critical_position_bonus(position: Position, position_mul: f64) -> f64 {
    let base = match position {
        Position::Side => 1.1,
        Position::Back => 1.3,
        Position::Front => 1.0,
    };
    base * position_mul
}

/// `Formulas.calcCriticalHeightBonus` — **identically 1.0**, and that is the
/// port of it:
///
/// ```java
/// return ((((CommonUtil.constrain(from.getZ() - target.getZ(), -25, 25) * 4) / 5) + 10) / 100) + 1;
/// ```
///
/// Every operand is an `int` — `getZ()`, the `constrain(int, int, int)`
/// overload, the literals — so the whole expression is integer arithmetic and
/// the final `/ 100` truncates. The numerator spans −10..30, so it is 0 for
/// every z difference and the method returns a flat 1.
///
/// This is an upstream bug (the band was clearly meant to be ±10 %/+30 %), but
/// Java is the specification here: the port used to divide in `f64` and hand
/// out a 1.1 crit multiplier on level ground and 1.3 uphill, which is a rate no
/// Java server grants. Kept as a function rather than folded into the callers
/// so the sweep in `formula_parity.rs` can pin it against the transcription.
pub fn calc_critical_height_bonus(from_z: i32, to_z: i32) -> f64 {
    f64::from((((((from_z - to_z).clamp(-25, 25) * 4) / 5) + 10) / 100) + 1)
}

/// `Formulas.calcCrit`'s auto-attack branch (balance multipliers 1.0 — the
/// dist populates none of the `*_CRITICAL_CHANCE_MULTIPLIERS` tables):
///
/// ```java
/// final double defenceCriticalRate = target.getStat().getValue(Stat.DEFENCE_CRITICAL_RATE, 1) - 1;
/// final double attackRateMod = (baseRate - target.getStat().getValue(Stat.DEFENCE_CRITICAL_RATE_ADD, 0)) * (1 - Math.min(0.8, defenceCriticalRate));
/// double attackRate = attackRateMod * calcCriticalPositionBonus(creature, target) * calcCriticalHeightBonus(creature, target);
/// if ((creature.getLevel() >= 78) || (target.getLevel() >= 78))
/// {
///     attackRate += (Math.sqrt(creature.getLevel()) * (creature.getLevel() - target.getLevel()) * 0.125);
/// }
/// return MathUtil.clamp(attackRate * balanceMod, 30, PlayerConfig.MAX_PCRIT_RATE) > Rnd.get(1000);
/// ```
///
/// Salvation rolls on the **1000 scale** the `critRate` stat is kept in, so
/// the old `/ 10` is gone and the clamp is `[30, MaxPCritRate]` (3 %–50 % on
/// this dist) rather than Interlude's 3–97 %.
///
/// The defender's `DEFENCE_CRITICAL_RATE` is no longer a multiplier on the
/// attacker's rate but a *reduction* `1 − min(0.8, value − 1)`: a value of 1.3
/// strips 30 %, and nothing strips more than 80 %. A value below 1 (a debuff)
/// turns it into a bonus. `_ADD` is subtracted before that, not added.
///
/// The level term fires when **either** side is 78 or over.
#[allow(clippy::too_many_arguments)]
pub fn calc_auto_attack_crit(
    crit_stat: f64,
    // `target.getValue(DEFENCE_CRITICAL_RATE, 1)` — `1 · mul + add`.
    defence_rate: f64,
    // `target.getValue(DEFENCE_CRITICAL_RATE_ADD, 0)`.
    defence_add: f64,
    position: Position,
    // `getPositionTypeValue(CRITICAL_RATE, position)` — Focus Chance 356's
    // per-position multiplier, identity 1.0 without it.
    crit_position_mul: f64,
    from_z: i32,
    to_z: i32,
    attacker_level: i32,
    target_level: i32,
    // `PlayerConfig.MAX_PCRIT_RATE` (`MaxPCritRate`, 500).
    max_rate: f64,
    roll: i32,
) -> bool {
    let attack_rate_mod = (crit_stat - defence_add) * (1.0 - (defence_rate - 1.0).min(0.8));
    let mut rate = attack_rate_mod
        * calc_critical_position_bonus(position, crit_position_mul)
        * calc_critical_height_bonus(from_z, to_z);
    if attacker_level >= 78 || target_level >= 78 {
        rate += f64::from(attacker_level).sqrt() * f64::from(attacker_level - target_level) * 0.125;
    }
    rate.clamp(30.0, max_rate) > roll as f64
}

/// `Creature.getRandomDamageMultiplier`: `1 + Rnd.get(-r, r)/100` — the
/// caller rolls `Rnd.get(-r, r)` (test-forceable) and passes it in.
pub fn random_damage_multiplier(roll_neg_r_to_r: i32) -> f64 {
    1.0 + roll_neg_r_to_r as f64 / 100.0
}

/// `Formulas.calcShldUse` result: no block / normal block (shield def added to
/// pDef) / perfect block (damage reduced to 1).
pub const SHIELD_NONE: u8 = 0;

pub const SHIELD_SUCCEED: u8 = 1;

pub const SHIELD_PERFECT: u8 = 2;

/// `Formulas.calcShldUse` (melee narrowing). `shield_rate` is the shield's base
/// `rShld` × the target's CON bonus (Java `SHIELD_DEFENCE_RATE × CON.calcBonus`);
/// `con_bonus` is that CON bonus (for the perfect-block roll). `from_behind` is
/// the attacker outside the 120° shield arc (Java's `degreeside` check — a back
/// attack can't be blocked). `rate_roll`/`perfect_roll` are `Rnd.get(100)`.
pub fn calc_shield_use(
    shield_rate: f64,
    con_bonus: f64,
    ranged: bool,
    from_behind: bool,
    rate_roll: i32,
    perfect_roll: i32,
) -> u8 {
    if shield_rate <= 0.0 || from_behind {
        return SHIELD_NONE;
    }
    // A bow attacker raises the block rate by 30% (Java).
    let rate = if ranged {
        shield_rate * 1.3
    } else {
        shield_rate
    };
    if rate > rate_roll as f64 {
        if (100.0 - (2.0 * con_bonus)) < perfect_roll as f64 {
            SHIELD_PERFECT
        } else {
            SHIELD_SUCCEED
        }
    } else {
        SHIELD_NONE
    }
}

/// `Formulas.calcCritDamage` / `calcCritDamageAdd` — the crit-damage
/// multiplier and flat bonus for one attacker/target pair.
///
/// [`Self::default`] is Java's stat-free result (`1 + 1 · 1 · max(0.2, 2 − 1)`
/// = 2 and `0`), which is what every actor without a crit-damage buff gets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CritDamage {
    /// `cAtk` — [`crit_damage_mul`].
    pub mul: f64,
    /// `cAtkAdd` — [`crit_damage_add`].
    pub add: f64,
}
impl Default for CritDamage {
    fn default() -> Self {
        Self { mul: 2.0, add: 0.0 }
    }
}

/// `Formulas.calcCritDamage`'s return, shared by its three branches:
///
/// ```java
/// // The limit for reducing critical damage is 80% (0.2).
/// return 1 + ((1 * criticalDamage * balanceMod) * Math.max(0.2, (2 - defenceCriticalDamage)));
/// ```
///
/// The defender's vulnerability is no longer a plain multiplier: a value of
/// 1.3 takes 30 % off the *bonus* part, and the bonus never drops below a
/// fifth. Balance multipliers are 1.0 on this dist.
pub fn crit_damage_mul(critical_damage: f64, defence_critical_damage: f64) -> f64 {
    1.0 + (critical_damage * (2.0 - defence_critical_damage).max(0.2))
}

/// `Formulas.calcCritDamageAdd`: `Math.max(0, criticalDamageAdd −
/// defenceCriticalDamageAdd)` — the defender's add now *subtracts*, and the
/// result never goes negative.
pub fn crit_damage_add(critical_damage_add: f64, defence_critical_damage_add: f64) -> f64 {
    (critical_damage_add - defence_critical_damage_add).max(0.0)
}

/// `Formulas.calcAutoAttackDamage` — **the whole expression**, with every
/// world-dependent term passed in so the arithmetic can be swept against a
/// transcription of Java's (`tools/tests/formula_parity.rs`).
///
/// ```text
/// attack = (pAtk · randomDamage) + proxBonus
/// attack = (((attack · cAtk · ss) + cAtkAdd) · critMod + attack · (1 − critMod) · ss)
///          · (isRanged ? 154 : 77)
/// damage = max(1, attack / defence · traitBonus · attributeBonus · pvpPveBonus)
/// ```
///
/// `critMod` is `crit ? (isRanged ? 0.5 : 1) : 0`, which is what makes a
/// **ranged crit** take *half* the crit branch and half the flat one rather
/// than all of either — the shape a plain `if crit` cannot express, and the
/// reason this function takes `is_ranged` instead of deferring it.
///
/// `defence` already carries the shield (a perfect block never reaches here;
/// the caller shortcuts to 1 damage). `AUTO_ATTACK_DAMAGE_BONUS` is Java's
/// last multiplier and is **not** a parameter: the only skill on this dist
/// declaring `AutoAttackDamageBonus` is in the 30500 range, so nothing here
/// can grant it and a 1.0 term would be noise.
#[allow(clippy::too_many_arguments)]
pub fn calc_auto_attack_damage(
    p_atk: f64,
    random_mul: f64,
    position: Position,
    p_def: f64,
    crit: bool,
    cd: CritDamage,
    ss: bool,
    // `Stat.SHOTS_BONUS` — `1 + weaponEnchant·0.007` (`ShotsBonusFinalizer`).
    shots_bonus: f64,
    is_ranged: bool,
    trait_bonus: f64,
    attribute_bonus: f64,
    pvp_pve_bonus: f64,
) -> f64 {
    let prox_bonus = match position {
        Position::Front => 0.0,
        Position::Side => 0.05,
        Position::Back => 0.2,
    } * p_atk;
    // `ssBonus = ss ? (ssBlessed ? 2.15 : 2) + SHOTS_BONUS / 100 : 1` —
    // additive, and the stat is divided by 100 first, so a +10 weapon's 1.07
    // is worth 0.0107 here. `ssBlessed` is always false: no item on this dist
    // has a handler that charges `BLESSED_SOULSHOTS`.
    let ss_bonus = if ss { 2.0 + (shots_bonus / 100.0) } else { 1.0 };
    let weapon_mod = if is_ranged { 154.0 } else { 77.0 };
    let crit_mod = if crit {
        if is_ranged { 0.5 } else { 1.0 }
    } else {
        0.0
    };
    let attack = p_atk * random_mul + prox_bonus;
    let attack = ((((attack * cd.mul * ss_bonus) + cd.add) * crit_mod)
        + (attack * (1.0 - crit_mod) * ss_bonus))
        * weapon_mod;
    let damage = attack / p_def.max(1.0);
    (damage * trait_bonus * attribute_bonus * pvp_pve_bonus).max(1.0)
}

/// `Formulas.calcCrit`'s physical-skill branch (balance multipliers 1.0):
///
/// ```java
/// final double physicalSkillRate = creature.getStat().getValue(Stat.CRITICAL_RATE_SKILL);
/// final double physicalSkillRateFinal = physicalSkillRate * statBonus;
/// return MathUtil.clamp((((baseRate / 100) + 1) * physicalSkillRateFinal) * balanceMod, 30, PlayerConfig.MAX_PSKILLCRIT_RATE) > Rnd.get(1000);
/// ```
///
/// `base_rate` is the effect's `criticalChance`; it no longer *is* the rate
/// but a percentage boost to the caster's `CRITICAL_RATE_SKILL` stat
/// (`PSkillCritRateFinalizer`, which already carries one STR bonus — this
/// multiplies a second in through `stat_bonus`). Rolled on the 1000 scale and
/// clamped to `[30, MaxPSkillCritRate]`.
pub fn calc_physical_skill_crit(
    base_rate: f64,
    skill_crit_rate: f64,
    stat_bonus: f64,
    max_rate: f64,
    roll: i32,
) -> bool {
    (((base_rate / 100.0) + 1.0) * skill_crit_rate * stat_bonus).clamp(30.0, max_rate) > roll as f64
}

/// The soulshot multiplier of the `PhysicalDamage` family (`PhysicalDamage`,
/// `EnergyDamage`, `PhysicalDamageHpLink`, `…SaveHp`, `…WeaponBonus`,
/// `PhysicalSoulDamage`):
///
/// ```java
/// if (effector.isChargedShot(ShotType.SOULSHOTS)) ssmod = Math.max(1, (2 + (SHOTS_BONUS / 100)));
/// else if (effector.isChargedShot(ShotType.BLESSED_SOULSHOTS)) ssmod = Math.max(1, (4 + (SHOTS_BONUS / 100)));
/// ```
///
/// Only the first arm is reachable: no item on this dist has a handler that
/// charges `BLESSED_SOULSHOTS`.
pub fn skill_ss_mod(ss: bool, shots_bonus: f64) -> f64 {
    if ss {
        (2.0 + (shots_bonus / 100.0)).max(1.0)
    } else {
        1.0
    }
}

/// `handlers/skill/effects/PhysicalDamage.java` `instant()`, melee/shotless
/// narrowing — the same dropped-terms rationale as `calc_auto_attack_damage`
/// (soulshots handled via `ss`; pvp-pve mods 1.0, `PHYSICAL_SKILL_POWER` 1,
/// abnormal/race mods 1.0). `power` already carries `SKILL_POWER_ADD`. The trait/weakness and
/// attribute mods are **real** and multiplied in by the caller.
/// Shield defence is folded into `p_def` by the caller (a perfect block never
/// reaches here — the caller shortcuts to 1 damage).
///
/// `ranged` picks Java's second formula. A bow/crossbow uses **`weaponMod` 70**
/// *and* adds a second `pAtk + power` term inside the bracket, which is why an
/// archer's skill is not simply `70/77` of a swordsman's:
///
/// - melee:  `77·((pAtk·pAtkMod)·levelMod + power) / pDef`
/// - ranged: `70·((pAtk·pAtkMod)·levelMod + power + pAtk + power) / pDef`
///
/// …then `· ssMod · critMod · randomMod`, where `ssMod` is [`skill_ss_mod`]
/// and `critMod` is `calcCritDamage`'s physical-skill value (2 with default
/// crit-damage stats). The caller multiplies in the trait, attribute and
/// pvp/pve terms, then adds `calcCritDamageAdd` and floors the hit at 1.
///
/// **Java's `rangedBonus` reads the raw `attack`, not `attack · levelMod`** —
/// the level modifier applies only to the first term.
#[allow(clippy::too_many_arguments)]
pub fn calc_physical_skill_damage(
    p_atk: f64,
    p_atk_mod: f64,
    p_def: f64,
    p_def_mod: f64,
    power: f64,
    level_mod: f64,
    random_mul: f64,
    crit: bool,
    crit_mul: f64,
    // [`skill_ss_mod`].
    ss_mod: f64,
    ranged: bool,
) -> f64 {
    let attack = p_atk * p_atk_mod;
    let defence = (p_def * p_def_mod).max(1.0);
    let weapon_mod = if ranged { 70.0 } else { 77.0 };
    let ranged_bonus = if ranged { attack + power } else { 0.0 };
    let crit_mod = if crit { crit_mul } else { 1.0 };
    let base_mod = (weapon_mod * ((attack * level_mod) + power + ranged_bonus)) / defence;
    (base_mod * ss_mod * crit_mod * random_mul).max(0.0)
}

/// `Creature.getLevelMod`: `(level + 89) / 100` (transform stances aside).
pub fn level_mod(level: i32) -> f64 {
    (level as f64 + 89.0) / 100.0
}

/// `Formulas.calcBlowDamage` (dagger blows: FatalBlow/Backstab/SoulBlow),
/// melee/identity-simplified. The crit-damage and pvp-pve multipliers are
/// identity for the actors that exist (default crit-damage stats) →
/// `cdMult = 1`, `cdPatk = 0`, so only the base blow term survives; the trait
/// and attribute mods are **real** and multiplied in by the caller. `position` adds 20% (back) / 5% (side) of `(power+pAtk)`
/// before the ×77. Shield is folded into `p_def` by the caller (perfect block →
/// the caller shortcuts to 1). `power` is Java's `skillPower`, already
/// carrying `SKILL_POWER_ADD`.
///
/// `77·((power+pAtk)·0.666 + isPos·(power+pAtk)·randomMul) / pDef · ssMod · randomMul`
/// The two crit-damage terms Java's blow formula carries, which are **not**
/// the shape the other formulas use:
///
/// * `mult` = `CRITICAL_DAMAGE · ((positionValue−1)/2 + 1) · ((DEFENCE_CRITICAL_DAMAGE−1)/2 + 1)`
///   — the position and vulnerability halves count *half*, which is Java's own
///   arithmetic and not a simplification;
/// * `p_atk_add` = `(CRITICAL_DAMAGE_ADD + DEFENCE_CRITICAL_DAMAGE_ADD) ·
///   (calcCritDamage(skill)/2)`, entering **inside** the bracket at ×6 — so it
///   is divided by defence like everything else rather than added on top.
///
/// Both are their identity (1.0 / 0.0) for an actor carrying no such stats,
/// which is why the port ran without them unnoticed: a bare dagger is
/// unaffected, a Death Whisper'd one is not.
#[derive(Debug, Clone, Copy)]
pub struct BlowCritDamage {
    pub mult: f64,
    pub p_atk_add: f64,
}
impl Default for BlowCritDamage {
    fn default() -> Self {
        Self {
            mult: 1.0,
            p_atk_add: 0.0,
        }
    }
}

pub fn calc_blow_damage(
    p_atk: f64,
    power: f64,
    p_def: f64,
    position: Position,
    random_mul: f64,
    ss: bool,
    // `Stat.SHOTS_BONUS` (`ShotsBonusFinalizer`).
    shots_bonus: f64,
    cd: BlowCritDamage,
) -> f64 {
    let is_pos = match position {
        Position::Back => 0.2,
        Position::Side => 0.05,
        Position::Front => 0.0,
    };
    let sum = power + p_atk;
    // `ssmod = ss ? (2 + (SHOTS_BONUS / 100)) : 1`.
    let ss_mod = if ss { 2.0 + (shots_bonus / 100.0) } else { 1.0 };
    let base_mod = (77.0 * ((sum * 0.666) + (is_pos * sum * random_mul) + (6.0 * cd.p_atk_add)))
        / p_def.max(1.0);
    (base_mod * ss_mod * cd.mult * random_mul).max(0.0)
}

/// `Formulas.calcBlowSuccess` — the "does the blow land" roll (part of the blow
/// effect's `calcSuccess`). `crit_rate` is Java's `weaponCritical`: the
/// equipped weapon's raw `critRate` stat (no DEX bonus, no finalize), or the
/// caster template's `baseCritRate` bare-handed — resolved by the caller.
/// `blow_rate_mod` is the caster's finalized `Stat.BLOW_RATE`
/// (`FatalBlowRate` — Focus Death, Critical Blow, Mortal Strike, Assassination
/// — default 1.0 for anyone without one of those). `Stat.BLOW_RATE_DEFENCE`
/// (`FatalBlowRateDefence`) stays identity — nothing in this datapack grants
/// it. Lands when `roll` (`Rnd.get(100)`) < min(rate, limit).
pub fn calc_blow_success(
    crit_rate: f64,
    position: Position,
    crit_position_mul: f64,
    from_z: i32,
    to_z: i32,
    chance_boost: f64,
    blow_rate_mod: f64,
    limit: f64,
    roll: i32,
) -> bool {
    let rate = calc_critical_position_bonus(position, crit_position_mul)
        * calc_critical_height_bonus(from_z, to_z)
        * crit_rate
        * ((100.0 + chance_boost) / 100.0)
        * blow_rate_mod;
    (roll as f64) < rate.min(limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Equal accuracy/evasion → 800‰ hit chance: roll 800 hits (strict `<`),
    /// 801 misses. Extreme gaps clamp to [200, 980].
    #[test]
    fn hit_miss_thresholds_and_clamps() {
        assert!(!calc_hit_miss(50, 50, 1.0, 800));
        assert!(calc_hit_miss(50, 50, 1.0, 801));
        // Hopeless accuracy still hits on a roll below the 200 floor.
        assert!(!calc_hit_miss(0, 500, 1.0, 199));
        assert!(calc_hit_miss(0, 500, 1.0, 201));
        // Overwhelming accuracy still misses above the 980 cap.
        assert!(calc_hit_miss(500, 0, 1.0, 981));
    }

    /// Auto-attack crit on Salvation's 1000 scale: `(stat − defAdd) · (1 −
    /// min(0.8, defRate − 1)) · position · height`, clamped to `[30,
    /// MaxPCritRate]`, against `Rnd.get(1000)`. The height bonus is Java's flat
    /// 1 (its `/100` is integer division).
    #[test]
    fn auto_attack_crit_rate() {
        let roll = |stat: f64, def_rate: f64, def_add: f64, pos, lvl: i32, tlvl: i32, r: i32| {
            calc_auto_attack_crit(stat, def_rate, def_add, pos, 1.0, 0, 0, lvl, tlvl, 500.0, r)
        };
        // Stat 440, front, no defence: 440 — roll 439 crits, 440 doesn't.
        assert!(roll(440.0, 1.0, 0.0, Position::Front, 40, 40, 439));
        assert!(!roll(440.0, 1.0, 0.0, Position::Front, 40, 40, 440));
        // Floor 30 even at zero stat.
        assert!(roll(0.0, 1.0, 0.0, Position::Front, 40, 40, 29));
        assert!(!roll(0.0, 1.0, 0.0, Position::Front, 40, 40, 30));
        // Cap at MaxPCritRate 500.
        assert!(roll(10_000.0, 1.0, 0.0, Position::Back, 40, 40, 499));
        assert!(!roll(10_000.0, 1.0, 0.0, Position::Back, 40, 40, 500));
        // Back: 300 · 1.3 = 390.
        assert!(roll(300.0, 1.0, 0.0, Position::Back, 40, 40, 389));
        assert!(!roll(300.0, 1.0, 0.0, Position::Back, 40, 40, 390));
        // Defence rate 1.3 strips 30 %: 440 · 0.7 = 308.
        assert!(roll(440.0, 1.3, 0.0, Position::Front, 40, 40, 307));
        assert!(!roll(440.0, 1.3, 0.0, Position::Front, 40, 40, 308));
        // …and never more than 80 %: rate 2.5 still leaves 440 · 0.2 = 88.
        assert!(roll(440.0, 2.5, 0.0, Position::Front, 40, 40, 87));
        assert!(!roll(440.0, 2.5, 0.0, Position::Front, 40, 40, 88));
        // The add is subtracted first: (440 − 40) · 1 = 400.
        assert!(roll(440.0, 1.0, 40.0, Position::Front, 40, 40, 399));
        assert!(!roll(440.0, 1.0, 40.0, Position::Front, 40, 40, 400));
        // Level term, either side 78+: 440 + √80 · 10 · 0.125 = 451.18.
        assert!(roll(440.0, 1.0, 0.0, Position::Front, 80, 70, 451));
        assert!(!roll(440.0, 1.0, 0.0, Position::Front, 80, 70, 452));
    }

    /// `calcAutoAttackDamage`, melee/shotless: pAtk 100 vs pDef 50 → 154;
    /// crit doubles; back position adds 20% of pAtk before the ×77.
    #[test]
    fn auto_attack_damage_matches_java() {
        assert!(
            (calc_auto_attack_damage(
                100.0,
                1.0,
                Position::Front,
                50.0,
                false,
                CritDamage::default(),
                false,
                1.0,
                false,
                1.0,
                1.0,
                1.0,
            ) - 154.0)
                .abs()
                < 1e-9
        );
        assert!(
            (calc_auto_attack_damage(
                100.0,
                1.0,
                Position::Front,
                50.0,
                true,
                CritDamage::default(),
                false,
                1.0,
                false,
                1.0,
                1.0,
                1.0,
            ) - 308.0)
                .abs()
                < 1e-9
        );
        assert!(
            (calc_auto_attack_damage(
                100.0,
                1.0,
                Position::Back,
                50.0,
                false,
                CritDamage::default(),
                false,
                1.0,
                false,
                1.0,
                1.0,
                1.0,
            ) - 184.8)
                .abs()
                < 1e-9
        );
        // A soulshot is `2 + SHOTS_BONUS/100`: with the stat at 1, 154 · 2.01.
        assert!(
            (calc_auto_attack_damage(
                100.0,
                1.0,
                Position::Front,
                50.0,
                false,
                CritDamage::default(),
                true,
                1.0,
                false,
                1.0,
                1.0,
                1.0,
            ) - 154.0 * 2.01)
                .abs()
                < 1e-9
        );
        // A +10 weapon (1.07) adds 0.0107, not 7 %: 154 · 2.0107.
        assert!(
            (calc_auto_attack_damage(
                100.0,
                1.0,
                Position::Front,
                50.0,
                false,
                CritDamage::default(),
                true,
                1.07,
                false,
                1.0,
                1.0,
                1.0,
            ) - 154.0 * 2.0107)
                .abs()
                < 1e-9
        );
        // `Math.max(1, damage)`: 77 · 1 / 10 000 rounds up to a 1-damage hit.
        assert_eq!(
            calc_auto_attack_damage(
                1.0,
                1.0,
                Position::Front,
                10_000.0,
                false,
                CritDamage::default(),
                false,
                1.0,
                false,
                1.0,
                1.0,
                1.0,
            ),
            1.0
        );
        // pDef floors at 1.
        assert!(
            calc_auto_attack_damage(
                100.0,
                1.0,
                Position::Front,
                0.0,
                false,
                CritDamage::default(),
                false,
                1.0,
                false,
                1.0,
                1.0,
                1.0,
            )
            .is_finite()
        );
    }

    /// `calcShldUse`: no shield → never blocks; a back attack can't be blocked;
    /// the block rate is `rate > Rnd(100)`; a low perfect-roll upgrades to a
    /// perfect block.
    #[test]
    fn shield_block_rolls() {
        // No shield equipped (rate 0) → never a block.
        assert_eq!(calc_shield_use(0.0, 1.2, false, false, 0, 0), SHIELD_NONE);
        // Rate 30 blocks a roll of 20, not a roll of 40 (low perfect-roll keeps
        // it a normal block).
        assert_eq!(
            calc_shield_use(30.0, 1.0, false, false, 20, 50),
            SHIELD_SUCCEED
        );
        assert_eq!(
            calc_shield_use(30.0, 1.0, false, false, 40, 50),
            SHIELD_NONE
        );
        // A back attack is never blocked even at a high rate.
        assert_eq!(calc_shield_use(90.0, 1.0, false, true, 0, 0), SHIELD_NONE);
        // Perfect block when `100 - 2·conBonus < perfectRoll` (conBonus 1.0 →
        // threshold 98): a perfect-roll of 99 upgrades, 98 does not.
        assert_eq!(
            calc_shield_use(90.0, 1.0, false, false, 0, 99),
            SHIELD_PERFECT
        );
        assert_eq!(
            calc_shield_use(90.0, 1.0, false, false, 0, 98),
            SHIELD_SUCCEED
        );
        // A bow attacker raises the rate 30% (rate 30 → 39, blocks a roll of 35).
        assert_eq!(
            calc_shield_use(30.0, 1.0, true, false, 35, 50),
            SHIELD_SUCCEED
        );
    }

    /// Physical skill damage, melee/shotless: pAtk 100, level 40 (levelMod
    /// 1.29), power 50, pDef 60 → 77·((100·1.29)+50)/60 = 229.833…; a crit
    /// doubles, a soulshot is ×(2 + SHOTS_BONUS/100), randomMod scales
    /// linearly.
    #[test]
    fn physical_skill_damage_matches_java() {
        let lm = level_mod(40);
        let base = calc_physical_skill_damage(
            100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.0, false, 2.0, 1.0, false,
        );
        assert!((base - (77.0 * ((100.0 * 1.29) + 50.0) / 60.0)).abs() < 1e-9);
        assert!(
            (calc_physical_skill_damage(
                100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.0, true, 2.0, 1.0, false,
            ) - base * 2.0)
                .abs()
                < 1e-9
        );
        assert!(
            (calc_physical_skill_damage(
                100.0,
                1.0,
                60.0,
                1.0,
                50.0,
                lm,
                1.0,
                false,
                2.0,
                skill_ss_mod(true, 1.0),
                false,
            ) - base * 2.01)
                .abs()
                < 1e-9
        );
        assert!(
            (calc_physical_skill_damage(
                100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.1, false, 2.0, 1.0, false,
            ) - base * 1.1)
                .abs()
                < 1e-9
        );
        // pAtkMod/pDefMod scale attack and defence; defence floors at 1.
        assert!(
            calc_physical_skill_damage(
                100.0, 1.0, 0.0, 0.0, 50.0, lm, 1.0, false, 2.0, 1.0, false,
            )
            .is_finite()
        );
    }

    /// The **ranged** branch is not `70/77` of the melee one: it also adds a
    /// second `pAtk + power` inside the bracket, and that bonus reads the raw
    /// `pAtk` — the level modifier applies only to the first term.
    /// `70·((100·1.29) + 50 + 100 + 50) / 60`.
    #[test]
    fn ranged_physical_skill_damage_adds_its_bonus_term() {
        let lm = level_mod(40);
        let melee = calc_physical_skill_damage(
            100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.0, false, 2.0, 1.0, false,
        );
        let ranged =
            calc_physical_skill_damage(100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.0, false, 2.0, 1.0, true);
        assert!((ranged - (70.0 * ((100.0 * 1.29) + 50.0 + 100.0 + 50.0) / 60.0)).abs() < 1e-9);
        assert!(
            ranged > melee,
            "the bonus term outweighs the smaller weaponMod: {ranged} vs {melee}"
        );
        // Not simply the weaponMod ratio.
        assert!((ranged - melee * 70.0 / 77.0).abs() > 1.0);
    }

    /// Physical skill crit: `((criticalChance/100) + 1) · CRITICAL_RATE_SKILL ·
    /// statBonus`, clamped to `[30, MaxPSkillCritRate]`, vs `Rnd.get(1000)`.
    #[test]
    fn physical_skill_crit_rate_and_clamps() {
        // 1.5 · 200 · 1.5 = 450: roll 449 crits, 450 doesn't.
        assert!(calc_physical_skill_crit(50.0, 200.0, 1.5, 2000.0, 449));
        assert!(!calc_physical_skill_crit(50.0, 200.0, 1.5, 2000.0, 450));
        // An unbuffed caster — base 5 × STR bonus 1.3 in the stat, × 1.3 again
        // here, × 1.1 for criticalChance 10 — sits at 9.3, under the 30 floor.
        assert!(calc_physical_skill_crit(10.0, 6.5, 1.3, 2000.0, 29));
        assert!(!calc_physical_skill_crit(10.0, 6.5, 1.3, 2000.0, 30));
        // The cap binds a configured ceiling below the roll's range.
        assert!(!calc_physical_skill_crit(100.0, 1000.0, 1.0, 400.0, 400));
        assert!(calc_physical_skill_crit(100.0, 1000.0, 1.0, 400.0, 399));
    }

    /// `PhysicalDamage`'s `ssmod`: `max(1, 2 + SHOTS_BONUS/100)` with a
    /// soulshot, 1 without.
    #[test]
    fn skill_soulshot_mod_is_additive() {
        assert_eq!(skill_ss_mod(false, 1.07), 1.0);
        assert!((skill_ss_mod(true, 1.0) - 2.01).abs() < 1e-12);
        assert!((skill_ss_mod(true, 1.07) - 2.0107).abs() < 1e-12);
    }

    /// `calcCritDamage`: `1 + critDmg · max(0.2, 2 − defCritDmg)`; and
    /// `calcCritDamageAdd`: `max(0, add − defAdd)`.
    #[test]
    fn crit_damage_terms() {
        // No stats: 1 + 1 · 1 = 2.
        assert_eq!(crit_damage_mul(1.0, 1.0), 2.0);
        // Death Whisper's ×1.3 on the attacker: 2.3.
        assert!((crit_damage_mul(1.3, 1.0) - 2.3).abs() < 1e-12);
        // A defender at 1.3 takes 30 % off the bonus part: 1 + 0.7.
        assert!((crit_damage_mul(1.0, 1.3) - 1.7).abs() < 1e-12);
        // …and the bonus never drops below a fifth.
        assert!((crit_damage_mul(1.0, 5.0) - 1.2).abs() < 1e-12);
        // A defender below 1 raises it: 1 + 1 · 1.5.
        assert!((crit_damage_mul(1.0, 0.5) - 2.5).abs() < 1e-12);
        assert_eq!(crit_damage_add(300.0, 100.0), 200.0);
        assert_eq!(crit_damage_add(100.0, 300.0), 0.0);
    }

    /// Blow damage: pAtk 100, power 50, pDef 60, front, no shot →
    /// 77·((150·0.666))/60 = 128.205; back adds 20% of (power+pAtk); a soulshot
    /// is ×(2 + SHOTS_BONUS/100); randomMul scales (and also feeds the
    /// positional term).
    #[test]
    fn blow_damage_matches_java() {
        let front = calc_blow_damage(
            100.0,
            50.0,
            60.0,
            Position::Front,
            1.0,
            false,
            1.0,
            BlowCritDamage::default(),
        );
        assert!((front - (77.0 * (150.0 * 0.666) / 60.0)).abs() < 1e-9);
        // Back: +0.2·150 inside the bracket.
        let back = calc_blow_damage(
            100.0,
            50.0,
            60.0,
            Position::Back,
            1.0,
            false,
            1.0,
            BlowCritDamage::default(),
        );
        assert!((back - (77.0 * ((150.0 * 0.666) + (0.2 * 150.0)) / 60.0)).abs() < 1e-9);
        // Soulshot: ×2.01 with the stat at 1.
        assert!(
            (calc_blow_damage(
                100.0,
                50.0,
                60.0,
                Position::Front,
                1.0,
                true,
                1.0,
                BlowCritDamage::default(),
            ) - front * 2.01)
                .abs()
                < 1e-9
        );
        // pDef floors at 1.
        assert!(
            calc_blow_damage(
                100.0,
                50.0,
                0.0,
                Position::Front,
                1.0,
                false,
                1.0,
                BlowCritDamage::default(),
            )
            .is_finite()
        );
    }

    /// Blow success: rate = posBonus · heightBonus · critRate · (100+boost)/100
    /// · blowRateMod, capped at `limit`, vs Rnd(100). The height bonus is
    /// Java's flat 1 at every z (its `/100` is integer division).
    #[test]
    fn blow_success_rate_cap_and_threshold() {
        // 1.0 · 1.0 · 10 · 1.0 · 1.0 = 10: roll 9 lands, 10 doesn't.
        assert!(calc_blow_success(
            10.0,
            Position::Front,
            1.0,
            0,
            0,
            0.0,
            1.0,
            100.0,
            9
        ));
        assert!(!calc_blow_success(
            10.0,
            Position::Front,
            1.0,
            0,
            0,
            0.0,
            1.0,
            100.0,
            10
        ));
        // chanceBoost 100 doubles the rate → 20.
        assert!(calc_blow_success(
            10.0,
            Position::Front,
            1.0,
            0,
            0,
            100.0,
            1.0,
            100.0,
            19
        ));
        // A huge crit rate is capped at `limit` (80): roll 79 lands, 80 doesn't.
        assert!(calc_blow_success(
            10_000.0,
            Position::Front,
            1.0,
            0,
            0,
            0.0,
            1.0,
            80.0,
            79
        ));
        assert!(!calc_blow_success(
            10_000.0,
            Position::Front,
            1.0,
            0,
            0,
            0.0,
            1.0,
            80.0,
            80
        ));
        // Assassination lvl1 (`blowRateMod = 1.03`, +3% PER) raises the same
        // rate to 10.3 — roll 10 now lands, 11 still doesn't.
        assert!(calc_blow_success(
            10.0,
            Position::Front,
            1.0,
            0,
            0,
            0.0,
            1.03,
            100.0,
            10
        ));
        assert!(!calc_blow_success(
            10.0,
            Position::Front,
            1.0,
            0,
            0,
            0.0,
            1.03,
            100.0,
            11
        ));
    }
}
