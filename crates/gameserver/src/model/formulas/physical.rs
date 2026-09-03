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
/// final double criticalRateMod = (target.getStat().getValue(Stat.DEFENCE_CRITICAL_RATE, rate) + target.getStat().getValue(Stat.DEFENCE_CRITICAL_RATE_ADD, 0)) / 10;
/// final double criticalLocBonus = calcCriticalPositionBonus(creature, target);
/// final double criticalHeightBonus = calcCriticalHeightBonus(creature, target);
/// rate = criticalLocBonus * criticalRateMod * criticalHeightBonus;
/// // Autoattack critical depends on level difference at high levels as well.
/// if ((creature.getLevel() >= 78) || (target.getLevel() >= 78))
/// {
///     rate += (Math.sqrt(creature.getLevel()) * (creature.getLevel() - target.getLevel()) * 0.125);
/// }
/// rate = CommonUtil.constrain(rate, 3, 97);
/// return (rate * balanceMod) > Rnd.get(100);
/// ```
///
/// The level term fires when **either** side is 78 or over, which the 80-level
/// cap on this dist puts well inside reach: an 80 hitting a 70 adds ~11 points
/// of crit rate before the clamp, and the same 80 hit by that 70 loses them.
pub fn calc_auto_attack_crit(
    crit_stat: f64,
    defence_mul: f64,
    defence_add: f64,
    position: Position,
    // `getPositionTypeValue(CRITICAL_RATE, position)` — Focus Chance 356's
    // per-position multiplier, identity 1.0 without it.
    crit_position_mul: f64,
    from_z: i32,
    to_z: i32,
    attacker_level: i32,
    target_level: i32,
    roll: i32,
) -> bool {
    // `criticalRateMod = (target.getValue(DEFENCE_CRITICAL_RATE, rate)
    //                     + target.getValue(DEFENCE_CRITICAL_RATE_ADD, 0)) / 10`
    // — the two-arg `getValue` is `mul * baseValue + add`, so the defender's
    // multiplier scales the *attacker's* rate. Both default to identity
    // (1.0 / 0.0), which reproduces the plain `crit_stat / 10` this had before.
    let rate_mod = ((defence_mul * crit_stat) + defence_add) / 10.0;
    let mut rate = calc_critical_position_bonus(position, crit_position_mul)
        * rate_mod
        * calc_critical_height_bonus(from_z, to_z);
    if attacker_level >= 78 || target_level >= 78 {
        rate += f64::from(attacker_level).sqrt() * f64::from(attacker_level - target_level) * 0.125;
    }
    rate.clamp(3.0, 97.0) > roll as f64
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
/// [`Self::default`] is Java's stat-free result (`2 * 1 * 1 * 1` and `0`),
/// which is what every actor without a crit-damage buff gets, and what the
/// whole port hard-coded before this slice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CritDamage {
    /// `cAtk` — `2 · criticalDamage · defenceCriticalDamage · balanceMod`.
    /// The 2 is Java's, baked into `calcCritDamage` itself.
    pub mul: f64,
    /// `cAtkAdd` — `criticalDamageAdd + defenceCriticalDamageAdd`.
    pub add: f64,
}
impl Default for CritDamage {
    fn default() -> Self {
        Self { mul: 2.0, add: 0.0 }
    }
}

/// `Formulas.calcAutoAttackDamage` — **the whole expression**, with every
/// world-dependent term passed in so the arithmetic can be swept against a
/// transcription of Java's (`tools/tests/formula_parity.rs`).
///
/// ```text
/// attack = (pAtk · randomDamage) + proxBonus
/// attack = (((attack · cAtk · ss) + cAtkAdd) · critMod + attack · (1 − critMod) · ss)
///          · (isRanged ? 154 : 77)
/// damage = attack / defence · traitBonus · attributeBonus · pvpPveBonus
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
    // `Stat.SHOTS_BONUS` — `1 + weaponEnchant·0.003` (`ShotsBonusFinalizer`).
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
    // `ssBonus` = `ss ? (blessed ? 2.15 : 2) · SHOTS_BONUS : 1`. Blessed
    // soulshots do not exist on Interlude; `SHOTS_BONUS` does have a carrier —
    // the weapon's enchant level, through `ShotsBonusFinalizer`.
    let ss_bonus = if ss { 2.0 * shots_bonus } else { 1.0 };
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
    (damage * trait_bonus * attribute_bonus * pvp_pve_bonus).max(0.0)
}

/// `Formulas.calcCrit`'s physical-skill branch for sub-78 actors
/// (`handlers/effecthandlers/PhysicalAttack.java` passes `_criticalChance`,
/// default 10). `statBonus` is the caster's STR bonus (`BaseStat.STR.calcBonus`,
/// the default skill-crit stat); `CRITICAL_RATE_SKILL` mul (1.0) and the
/// pvp/pve balance multipliers (1.0) drop out. Chance-in-100 is clamped to
/// [5, 90]; crit when `rate > roll` (`Rnd.get(100)`).
pub fn calc_physical_skill_crit(critical_chance: f64, str_bonus: f64, roll: i32) -> bool {
    (critical_chance * str_bonus).clamp(5.0, 90.0) > roll as f64
}

/// `handlers/effecthandlers/PhysicalAttack.java` `instant()`, melee/shotless
/// narrowing — the same dropped-terms rationale as `calc_auto_attack_damage`
/// (soulshots handled via `ss`; pvp-pve mods 1.0, `SKILL_POWER_ADD` 0,
/// `PHYSICAL_SKILL_POWER` 1, abnormal/race mods 1.0). The trait/weakness and
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
/// …then `· ssMod · critMod · randomMod`, where `ssMod = ss ? 2 : 1` and
/// `critMod` is `calcCritDamage`'s physical-skill value (2 with default
/// crit-damage stats).
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
    ss: bool,
    // `Stat.SHOTS_BONUS` (`ShotsBonusFinalizer`) — `ssmod = 2 · SHOTS_BONUS`.
    shots_bonus: f64,
    ranged: bool,
) -> f64 {
    let attack = p_atk * p_atk_mod;
    let defence = (p_def * p_def_mod).max(1.0);
    let weapon_mod = if ranged { 70.0 } else { 77.0 };
    let ranged_bonus = if ranged { attack + power } else { 0.0 };
    let ss_mod = if ss { 2.0 * shots_bonus } else { 1.0 };
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
/// the caller shortcuts to 1). `SKILL_POWER_ADD` is 0.
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
    let ss_mod = if ss { 2.0 * shots_bonus } else { 1.0 };
    let base_mod = (77.0 * ((sum * 0.666) + (is_pos * sum * random_mul) + (6.0 * cd.p_atk_add)))
        / p_def.max(1.0);
    (base_mod * ss_mod * cd.mult * random_mul).max(0.0)
}

/// `Formulas.calcBlowSuccess` — the "does the blow land" roll (part of the blow
/// effect's `calcSuccess`). `crit_rate` is Java's `weaponCritical`: the
/// equipped weapon's raw `rCrit` stat (no DEX bonus, no finalize), or the
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

    /// Auto-attack crit: rate = position · stat/10 · height, clamped [3, 97].
    /// The height bonus is Java's flat 1 (its `/100` is integer division), so
    /// level ground and a 25-unit rise weigh the same.
    #[test]
    fn auto_attack_crit_rate() {
        // stat 44 (displayed), front, level ground: 4.4 → roll 4 crits, roll 5
        // doesn't.
        assert!(calc_auto_attack_crit(
            44.0,
            1.0,
            0.0,
            Position::Front,
            1.0,
            0,
            0,
            40,
            40,
            4
        ));
        assert!(!calc_auto_attack_crit(
            44.0,
            1.0,
            0.0,
            Position::Front,
            1.0,
            0,
            0,
            40,
            40,
            5
        ));
        // Floor: even 0 stat crits below 3%.
        assert!(calc_auto_attack_crit(
            0.0,
            1.0,
            0.0,
            Position::Front,
            1.0,
            0,
            0,
            40,
            40,
            2
        ));
        // Cap: 97% — a 97 roll never crits.
        assert!(!calc_auto_attack_crit(
            10_000.0,
            1.0,
            0.0,
            Position::Back,
            1.0,
            25,
            0,
            40,
            40,
            97
        ));
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
        // A soulshot doubles the swing (×2 ssBonus): 154 → 308.
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
            ) - 308.0)
                .abs()
                < 1e-9
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
    /// doubles, a soulshot doubles, randomMod scales linearly.
    #[test]
    fn physical_skill_damage_matches_java() {
        let lm = level_mod(40);
        let base = calc_physical_skill_damage(
            100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.0, false, 2.0, false, 1.0, false,
        );
        assert!((base - (77.0 * ((100.0 * 1.29) + 50.0) / 60.0)).abs() < 1e-9);
        assert!(
            (calc_physical_skill_damage(
                100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.0, true, 2.0, false, 1.0, false,
            ) - base * 2.0)
                .abs()
                < 1e-9
        );
        assert!(
            (calc_physical_skill_damage(
                100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.0, false, 2.0, true, 1.0, false,
            ) - base * 2.0)
                .abs()
                < 1e-9
        );
        assert!(
            (calc_physical_skill_damage(
                100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.1, false, 2.0, false, 1.0, false,
            ) - base * 1.1)
                .abs()
                < 1e-9
        );
        // pAtkMod/pDefMod scale attack and defence; defence floors at 1.
        assert!(
            calc_physical_skill_damage(
                100.0, 1.0, 0.0, 0.0, 50.0, lm, 1.0, false, 2.0, false, 1.0, false,
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
            100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.0, false, 2.0, false, 1.0, false,
        );
        let ranged = calc_physical_skill_damage(
            100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.0, false, 2.0, false, 1.0, true,
        );
        assert!((ranged - (70.0 * ((100.0 * 1.29) + 50.0 + 100.0 + 50.0) / 60.0)).abs() < 1e-9);
        assert!(
            ranged > melee,
            "the bonus term outweighs the smaller weaponMod: {ranged} vs {melee}"
        );
        // Not simply the weaponMod ratio.
        assert!((ranged - melee * 70.0 / 77.0).abs() > 1.0);
    }

    /// Physical skill crit: `critChance · strBonus` clamped [5, 90] vs Rnd(100).
    #[test]
    fn physical_skill_crit_rate_and_clamps() {
        // chance 10, STR bonus 1.2 → 12: roll 11 crits, 12 doesn't (strict `>`).
        assert!(calc_physical_skill_crit(10.0, 1.2, 11));
        assert!(!calc_physical_skill_crit(10.0, 1.2, 12));
        // Floor 5% even at zero chance; cap 90% at huge chance.
        assert!(calc_physical_skill_crit(0.0, 1.0, 4));
        assert!(!calc_physical_skill_crit(1000.0, 1.0, 90));
    }

    /// Blow damage: pAtk 100, power 50, pDef 60, front, no shot →
    /// 77·((150·0.666))/60 = 128.205; back adds 20% of (power+pAtk); a soulshot
    /// doubles; randomMul scales (and also feeds the positional term).
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
        // Soulshot doubles the front hit.
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
            ) - front * 2.0)
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
