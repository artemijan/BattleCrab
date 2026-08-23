//! Port of `model/stats/Formulas.java`, scoped to what the single-target cast
//! pipeline needs: magic damage, magic crit, cast timing, heal, and the
//! cast-break-on-hit roll. Every function documents the Java method it ports
//! and which terms are dropped. The dropped terms are identity values for an
//! unarmed, shotless player: `SHOTS_BONUS`/spiritshots (1.0/absent),
//! `SKILL_POWER_ADD` (0), `RANDOM_DAMAGE` (weapon-supplied, unarmed = 0 →
//! randomMod 1.0), pvp/pve config multipliers (1.0 by default),
//! `MAGICAL_SKILL_POWER` (1.0).
//!
//! The **attribute** mod is real since the G19 attributes slice
//! ([`calc_attribute_bonus`]) and the **trait** mods since the G20 trait slice
//! (`skills::effects::skill_trait_mod`) — callers multiply both in at Java's
//! spots rather than this module folding them, which is why the signatures
//! stop short of them.

use crate::data::GameData;
use crate::model::Player;
use crate::model::skill::Skill;
use crate::model::stats::{BaseStat, Stat};

/// `Formulas.SKILL_LAUNCH_TIME` — the floor on the launch→finish phase.
const SKILL_LAUNCH_TIME_MS: f64 = 500.0;

/// The outcome of `calcMagicDam`'s `ALT_GAME_MAGICFAILURES` block — how a
/// failed [`calc_magic_success`] roll reshapes the damage. Rolled by the
/// caller (it needs the RNG and sends the system messages) and handed to
/// [`calc_magic_dam`] so the adjustment lands at Java's point in the formula:
/// *before* the crit multiplier, which is why a resisted magic crit deals 2
/// damage rather than 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MagicFailure {
    /// The spell landed (or `MagicFailures = False`).
    #[default]
    None,
    /// First roll failed, second succeeded — `damage /= 2`.
    Half,
    /// Both rolls failed — `damage = 1`.
    Resisted,
}

/// `Formulas.calcMagicDam` (the `77 * power * sqrt(mAtk) / mDef` MDAM
/// formula). `mcrit` doubles the damage via `calcCritDamage`'s magic branch
/// (`2 * MAGIC_CRITICAL_DAMAGE(1) * DEFENCE_MAGIC_CRITICAL_DAMAGE(1)`).
/// `shots_bonus` is Java's `bss ? 4 : sps ? 2 : 1` (times the `SHOTS_BONUS`
/// stat, 1.0 here) applied to the base magic damage.
///
/// `failure` is the `ALT_GAME_MAGICFAILURES` verdict. Java mutates `damage`
/// inside the failure block and only then multiplies by `critMod` and the
/// trait/attribute/random/pvpPve mods, so the halving and the `damage = 1`
/// floor are applied here *ahead* of `mcrit`. (The trait and attribute mods are
/// real and applied by the caller; only random/pvpPve are still identities.)
#[allow(clippy::too_many_arguments)]
pub fn calc_magic_dam(
    m_atk: f64,
    m_def: f64,
    power: f64,
    mcrit: bool,
    crit_mul: f64,
    shots_bonus: f64,
    failure: MagicFailure,
    random_mul: f64,
) -> f64 {
    let mut damage = (77.0 * power * m_atk.sqrt() / m_def.max(1.0)) * shots_bonus;
    match failure {
        MagicFailure::None => {}
        MagicFailure::Half => damage /= 2.0,
        MagicFailure::Resisted => damage = 1.0,
    }
    // `damage * critMod * … * randomMod * …` — the spread is Java's, and it
    // applies to a nuke exactly as it does to a swing: a mage's own
    // `randomDamage` (10 on every class template) makes the same cast land
    // anywhere in ±10 %.
    damage * if mcrit { crit_mul } else { 1.0 } * random_mul
}

/// `Formulas.calcProbability` — the chance gate on `Confuse`/`RandomizeHate`.
///
/// ```java
/// return Rnd.get(100) < (((skill.getMagicLevel() + baseChance) - target.getLevel()
///        - getAbnormalResist(...)) * calcAttributeBonus(...) * calcGeneralTraitBonus(...));
/// ```
///
/// The level term is the heart of it: a target far above the skill's level
/// shrugs it off, and the threshold is **unclamped**, so it can go negative
/// (never lands) or above 100 (always lands) exactly as Java's comparison does.
///
/// `attribute_mod` and `trait_mod` are `calcAttributeBonus` and
/// `calcGeneralTraitBonus(…, ignoreResistance = false)`. Both used to be 1.0
/// for every actor this port modelled, and this function said so — that stopped
/// being true when the attribute and trait tables landed, so they are passed in
/// now rather than assumed.
///
/// Java's `Double.isNaN(baseChance)` branch is unreachable here — the parser
/// defaults a missing `<chance>` to 100.
pub fn calc_probability(
    magic_level: i32,
    base_chance: i32,
    target_level: i32,
    // `getAbnormalResist(skill.getBasicProperty(), target)` — the target's
    // `ABNORMAL_RESIST_PHYSICAL`/`_MAGICAL`, **subtracted inside the
    // parenthesis** before either multiplier. The same stat
    // `calc_effect_land_rate` already reads; this formula had been ignoring
    // it, so a target whose abnormal resistance holds off a stun took a
    // Confuse at full rate.
    abnormal_resist: f64,
    attribute_mod: f64,
    trait_mod: f64,
    roll: i32,
) -> bool {
    let threshold = ((magic_level + base_chance - target_level) as f64 - abnormal_resist)
        * attribute_mod
        * trait_mod;
    (roll as f64) < threshold
}

/// `Formulas.calcManaDam` — the MP-drain damage formula, which is *not* the
/// HP one: `(sqrt(mAtk) * power * (targetMaxMp / 97)) / mDef`.
///
/// Note the target's **max MP** is a direct multiplier, so the same nuke drains
/// far more from a high-MP mage than from a fighter. `shots_bonus` is Java's
/// `bss ? 4 : sps ? 2 : 1` applied to `mAtk` *before* the square root (unlike
/// [`calc_magic_dam`], where it scales the finished damage).
///
/// A magic crit triples the result and then **clamps to `crit_limit`** — a
/// per-skill cap (1600 on Aura Sink / Seal of Gloom, 7000 on Mana Burn / Mana
/// Storm) with no equivalent anywhere in the HP formulas.
///
/// Dropped terms, all identity here: `calcGeneralTraitBonus`,
/// `calculatePvpPveBonus`, and the sapphire-jewel bonus (a later chronicle's
/// item, absent from this dist).
#[allow(clippy::too_many_arguments)]
pub fn calc_mana_dam(
    m_atk: f64,
    m_def: f64,
    target_max_mp: f64,
    power: f64,
    shots_bonus: f64,
    failure: MagicFailure,
    mcrit: bool,
    crit_limit: f64,
    trait_bonus: f64,
    pvp_pve_bonus: f64,
) -> f64 {
    let m_atk = m_atk * shots_bonus;
    let mut damage = (m_atk.sqrt() * power * (target_max_mp / 97.0)) / m_def.max(1.0);
    // Java applies both of these **here**, ahead of the failure halving and
    // the crit clamp — and the clamp is the reason the order is not free:
    // `min(damage · 3, criticalLimit)` after the multipliers is not the same
    // as multiplying a clamped value afterwards. The dist ships real limits
    // (1450, 1600, 7000), so a crit that binds one would have been let past
    // it. The trait bonus is read with `ignoreResistance = false` here, unlike
    // every other damage formula.
    damage *= trait_bonus;
    damage *= pvp_pve_bonus;
    // The `ALT_GAME_MAGICFAILURES` block, applied at Java's point in the
    // formula — before the crit, like `calc_magic_dam`. Java has no
    // `damage = 1` floor on the full-resist branch here, only the halving,
    // so `Resisted` and `Half` do the same thing: ported as written.
    match failure {
        MagicFailure::None => {}
        MagicFailure::Half | MagicFailure::Resisted => damage /= 2.0,
    }
    if mcrit {
        damage = (damage * 3.0).min(crit_limit);
    }
    damage
}

/// `Formulas.calcMagicAffected` — the drain's own landing roll, separate from
/// the `calcMagicSuccess` resist check.
///
/// ```java
/// double defence = (skill.isActive() && skill.isBad()) ? target.getMDef() : 0;
/// double attack  = 2 * actor.getMAtk() * traitBonus;      // traitBonus 1 here
/// double d = (attack - defence) / (attack + defence) + 0.5 * Rnd.nextGaussian();
/// return d > 0;
/// ```
///
/// So it is a *noisy* mAtk-vs-mDef comparison: with equal attack and defence
/// the deterministic term is 0 and the coin flip is even; a large mAtk edge
/// pushes it toward certainty without ever quite reaching it.
///
/// `gaussian` is supplied by the caller (`World::roll_gaussian`) to keep this
/// function pure. `defence` is passed as 0 for a skill that is not both active
/// and bad, matching the Java branch.
pub fn calc_magic_affected(m_atk: f64, defence: f64, gaussian: f64) -> bool {
    let attack = 2.0 * m_atk;
    let sum = attack + defence;
    // Java would divide by zero here for a 0-mAtk actor and get NaN, which
    // compares false; guard explicitly rather than relying on that.
    if sum <= 0.0 {
        return false;
    }
    let d = ((attack - defence) / sum) + (0.5 * gaussian);
    d > 0.0
}

/// `Formulas.calcCrit`'s magic branch:
///
/// ```java
/// rate = creature.getStat().getValue(Stat.MAGIC_CRITICAL_RATE);
/// if ((target == null) || !skill.isBad()) return Math.min(rate, 320) > Rnd.get(1000);
/// double finalRate = target.getStat().getValue(Stat.DEFENCE_MAGIC_CRITICAL_RATE, rate) + target.getStat().getValue(Stat.DEFENCE_MAGIC_CRITICAL_RATE_ADD, 0);
/// if ((creature.getLevel() >= 78) && (target.getLevel() >= 78))
/// {
///     finalRate += Math.sqrt(creature.getLevel()) + ((creature.getLevel() - target.getLevel()) / 25);
///     return Math.min(finalRate, 320 * balanceMod) > Rnd.get(1000);
/// }
/// return (Math.min(finalRate, 200) * balanceMod) > Rnd.get(1000);
/// ```
///
/// `m_crit_rate` is the per-mille `Player.m_crit_hit`; `roll` is
/// `Rnd.get(1000)`. A good skill caps at 320‰ and never reaches the level
/// branch; a bad one caps at 200‰ until **both** sides are 78 or over, where
/// the cap lifts to 320‰ and a `sqrt(level)` bonus comes with it.
///
/// Two narrowings, carriers named: the balance multipliers are 1.0 (the dist
/// leaves `PVP_/PVE_MAGICAL_SKILL_CRITICAL_CHANCE_MULTIPLIERS` unpopulated),
/// and `DEFENCE_MAGIC_CRITICAL_RATE`/`_ADD` are declared only by skills in the
/// 10500+ id ranges, none of them learnable or on an NPC list here — so the
/// defender's term stays the identity `getValue(stat, rate) = rate`.
///
/// Java's own `(creature.getLevel() - target.getLevel()) / 25` is **integer**
/// division, so it contributes 0 for every level gap this chronicle can
/// produce; it is written out here rather than dropped, for the same reason
/// the sweep keeps identity terms.
pub fn calc_magic_crit(
    m_crit_rate: f64,
    is_bad: bool,
    caster_level: i32,
    target_level: i32,
    roll: i32,
) -> bool {
    if !is_bad {
        return m_crit_rate.min(320.0) > roll as f64;
    }
    if caster_level >= 78 && target_level >= 78 {
        let rate = m_crit_rate
            + f64::from(caster_level).sqrt()
            + f64::from((caster_level - target_level) / 25);
        return rate.min(320.0) > roll as f64;
    }
    m_crit_rate.min(200.0) > roll as f64
}

/// Inputs to [`calc_magic_success_rate`] — one field per term of Java's
/// `Formulas.calcMagicSuccess`, resolved from the world at the call site.
#[derive(Debug, Clone)]
pub struct MagicSuccess<'a> {
    /// Java `attacker.isAttackable() || target.isAttackable()` — true when
    /// either side is a monster/guard, i.e. any PvE cast. Selects the
    /// level-difference branch; false takes the magic-accuracy branch.
    pub pve: bool,
    pub target_level: i32,
    /// `skill.magicLevel` when `CalculateMagicSuccessBySkillMagicLevel` is on
    /// (dist default) and the skill has a positive magic level, else the
    /// caster's level.
    pub effective_level: i32,
    /// `attacker.getActingPlayer()`'s level — `None` when the caster is an NPC,
    /// which skips the NPC skill-chance penalty entirely (Java's null check).
    pub caster_player_level: Option<i32>,
    /// `target.isRaid() || target.isRaidMinion()` — raids are exempt from the
    /// level-78 skill-chance penalty.
    pub target_is_raid: bool,
    /// `MinNPCLevelForMagicPenalty` (78 on this dist).
    pub min_npc_level_for_magic_penalty: i32,
    /// `SkillChancePenaltyForLvLDifferences` (`2.5, 3.0, 3.25, 3.5`).
    pub skill_chance_penalty: &'a [f64],
    /// `attacker.getMagicAccuracy()` / `target.getMagicEvasionRate()` — read
    /// only on the non-PvE branch.
    pub magic_accuracy: i32,
    pub magic_evasion: i32,
    /// The target's `Stat.MAGIC_SUCCESS_RES` multiplier (Java's `resModifier`,
    /// `getMul(..., 1)`), applied to the whole failure term — so a value above
    /// 1 makes the attacker *less* likely to land the spell.
    pub res_modifier: f64,
}

/// `Formulas.calcMagicSuccess` — the percent chance (may fall below 0, meaning
/// "always fails") that a magic attack is not resisted.
///
/// PvE branch: `lvlModifier = 1.3^(targetLevel - effectiveLevel)`, so the
/// penalty compounds fast — a 9-level gap already costs ~10 points and an
/// 18-level gap drives the rate to 0. On top of that, targets at
/// `MinNPCLevelForMagicPenalty` or above that outlevel the *caster* by 3+
/// multiply the failure term by `SkillChancePenaltyForLvLDifferences`.
///
/// PvP branch: a step table on `magicAccuracy - magicEvasion`.
///
/// Java's `resModifier` is `getMul(MAGIC_SUCCESS_RES, 1)`, applied to the whole
/// failure term.
///
/// **Correction:** this was previously documented as fixed at 1.0 on this dist,
/// on the grounds that the only two items touching `magicSuccRes` (10207/10208,
/// the enhanced shirts) declare it in a `<stats>` block that Java parses into an
/// *additive* func `getMul` never sees. That is true of the items — but it
/// overlooked **skills**: Anti Magic 146 and M. Def. 147 grant the stat through
/// `ResistDDMagic`, an `AbstractStatPercentEffect`, which merges
/// *multiplicatively* and so is exactly what `getMul` reads.
pub fn calc_magic_success_rate(i: &MagicSuccess) -> i32 {
    let mut lvl_modifier = 1.0f64;
    let mut target_modifier = 1.0f64;
    let mut m_acc_modifier = 1i32;

    if i.pve {
        lvl_modifier = 1.3f64.powi(i.target_level - i.effective_level);

        if let Some(caster_level) = i.caster_player_level
            && !i.target_is_raid
            && i.target_level >= i.min_npc_level_for_magic_penalty
            && (i.target_level - caster_level) >= 3
            && !i.skill_chance_penalty.is_empty()
        {
            let level_diff = (i.target_level - caster_level - 2) as usize;
            target_modifier =
                i.skill_chance_penalty[level_diff.min(i.skill_chance_penalty.len() - 1)];
        }
    } else {
        let m_acc_diff = i.magic_accuracy - i.magic_evasion;
        m_acc_modifier = if m_acc_diff > -20 {
            2
        } else if m_acc_diff > -25 {
            30
        } else if m_acc_diff > -30 {
            60
        } else if m_acc_diff > -35 {
            90
        } else {
            100
        };
    }

    100 - java_round_float(m_acc_modifier as f64 * lvl_modifier * target_modifier * i.res_modifier)
}

/// `Rnd.get(100) < rate` — `roll` is 0-99.
pub fn calc_magic_success(i: &MagicSuccess, roll: i32) -> bool {
    roll < calc_magic_success_rate(i)
}

/// Java `Math.round(float)`, which is `(int) floor(a + 0.5f)` — *not* Rust's
/// `f64::round` (half away from zero). The distinction only shows on exact
/// `.5` values, but the narrowing to `f32` first matters too: `1.3^n` grows
/// past `f32::MAX` around n = 330, where both languages saturate the cast to
/// `Integer.MAX_VALUE` / `i32::MAX`.
fn java_round_float(v: f64) -> i32 {
    ((v as f32) + 0.5f32).floor() as i32
}

/// `Formulas.calculateSkillResurrectRestorePercent` — the reviver's WIT scales
/// how much of the lost XP their resurrection gives back.
///
/// ```java
/// if (base == 0 || base == 100) return base;
/// restore = base * WIT.calcBonus(caster);
/// if ((restore - base) > 20.0) restore += 20.0;
/// return min(max(restore, base), 90.0);
/// ```
///
/// Note the quirk on the third line: a bonus that already exceeds +20 gets a
/// *further* flat +20, so high-WIT revivers jump rather than scale smoothly.
/// Ported as written.
pub fn calc_resurrect_restore_percent(base: f64, wit_bonus: f64) -> f64 {
    if base == 0.0 || base == 100.0 {
        return base;
    }
    let mut restore = base * wit_bonus;
    if (restore - base) > 20.0 {
        restore += 20.0;
    }
    restore.max(base).min(90.0)
}

/// `Formulas.calcEffectSuccess` — a continuous skill's landing chance in percent.
///
/// ```java
/// if (activateRate == -1) return true;
/// int magicLevel = skill.getMagicLevel();
/// if (magicLevel <= -1) magicLevel = target.getLevel() + 3;
/// final double targetBasicProperty = getAbnormalResist(skill.getBasicProperty(), target);
/// final double baseMod = ((((((magicLevel - target.getLevel()) + 3) * skill.getLvlBonusRate()) + activateRate) + 30.0) - targetBasicProperty);
/// final double rate = baseMod * elementMod * traitMod * buffDebuffMod;
/// final double finalRate = traitMod > 0 ? CommonUtil.constrain(rate, skill.getMinChance(), skill.getMaxChance()) * basicPropertyResist : 0;
/// ```
///
/// Everything up to `+ 30.0` is **integer** arithmetic in Java and is kept so
/// here; the four mods multiply in Java's left-to-right order, which is not
/// interchangeable in floating point.
///
/// `skill.getMinChance()`/`getMaxChance()` default to `Config.MIN_/MAX_ABNORMAL_
/// STATE_SUCCESS_RATE` and are overridden by no skill this dist can reach — the
/// 15 that declare their own bounds are all id ≥ 11537 and none is learnable —
/// so [`LandRateBounds`] carries the config pair alone.
/// `Character.ini`'s `MinAbnormalStateSuccessRate` / `MaxAbnormalStateSuccessRate`
/// — the `constrain(rate, minChance, maxChance)` bounds in
/// `Formulas.calcEffectLandRate`. Carried as a struct for the same reason
/// [`crate::model::NpcStatMods`] is: the formula stays a pure function of its
/// inputs, and `World` (which owns the config) is not reachable from here.
///
/// `Default` is this dist's 10/90, so a test that is not about the clamp does
/// not have to name it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LandRateBounds {
    pub min: f64,
    pub max: f64,
}

impl Default for LandRateBounds {
    fn default() -> Self {
        Self {
            min: 10.0,
            max: 90.0,
        }
    }
}

impl LandRateBounds {
    pub fn of(cfg: &crate::config::CharacterConfig) -> Self {
        Self {
            min: cfg.min_abnormal_state_success_rate,
            max: cfg.max_abnormal_state_success_rate,
        }
    }
}

pub fn calc_effect_land_rate(
    magic_level: i32,
    activate_rate: i32,
    lvl_bonus_rate: i32,
    target_level: i32,
    // The target's `Stat.RESIST_ABNORMAL_DEBUFF` (Java's `buffDebuffMod`):
    // 1.0 with no resistance, < 1 when resistant (Guts), > 1 when made
    // vulnerable (Touch of Death). Callers pass 1.0 for a non-debuff skill,
    // which is what Java's `skill.isDebuff() ? … : 1` does.
    debuff_resist_mod: f64,
    // `calcAttributeBonus` for the skill's element (Java's `elementMod`) —
    // an elemental debuff lands more easily on a target weak to its element.
    // 1.0 when neither side carries the element.
    element_mod: f64,
    // `calcGeneralTraitBonus` — 1.0 with no matching resistance, `< 1` when the
    // target resists this debuff's trait, `> 1` when the trait is a
    // *vulnerability*, and **0** when they are invulnerable to it.
    trait_mod: f64,
    // `getAbnormalResist(skill.getBasicProperty(), target)` — the target's
    // `ABNORMAL_RESIST_PHYSICAL`/`_MAGICAL` **stat**, *subtracted* inside
    // `baseMod`. 0 with no such stat.
    target_basic_property: f64,
    // `getBasicPropertyResistBonus(skill.getBasicProperty(), target)` — the
    // mesmerizing-debuff accrual chain (1.0 / 0.6 / 0.3 / 0), multiplied
    // **after** the clamp, which is why level 3 is a hard immunity rather than
    // a rate the 10 floor rescues. See `game_loop::basic_property`.
    basic_property_resist: f64,
    // `Config.MIN_/MAX_ABNORMAL_STATE_SUCCESS_RATE`, the `constrain` bounds.
    bounds: LandRateBounds,
) -> f64 {
    if activate_rate == -1 {
        return 100.0;
    }
    let magic_level = if magic_level <= -1 {
        target_level + 3
    } else {
        magic_level
    };
    let base_mod = ((magic_level - target_level + 3) * lvl_bonus_rate + activate_rate + 30) as f64
        - target_basic_property;
    // Invulnerability is *not* clamped: Java's
    // `finalRate = traitMod > 0 ? constrain(rate, min, max) : 0` short-circuits
    // past the 10 floor, so an immune target refuses the debuff outright rather
    // than taking it one roll in ten.
    if trait_mod <= 0.0 {
        return 0.0;
    }
    // Otherwise Java multiplies the raw rate by the mods and clamps *after*
    // (`constrain(baseMod * elementMod * … * buffDebuffMod, minChance,
    // maxChance)`), so heavy resistance can pull an otherwise-capped debuff
    // below the 90 ceiling but never under the 10 floor — **except** through
    // `basicPropertyResist`, which Java multiplies in after the clamp and which
    // therefore *can* reach 0.
    // Java's order: `baseMod * elementMod * traitMod * buffDebuffMod`.
    (base_mod * element_mod * trait_mod * debuff_resist_mod).clamp(bounds.min, bounds.max)
        * basic_property_resist
}

/// `Formulas.calcAttributeBonus`'s arithmetic tail (PLAN_G19_ATTRIBUTES.md):
/// `diff = attack − defence`; a positive gap scales up as
/// `min(1.025 + √(diff³/2)·0.0001, 1.25)`, a negative one down as
/// `max(0.975 − √(−diff³/2)·0.0001, 0.75)`, zero is exactly 1. Note the
/// discontinuity at ±1 (1.0 jumps past 1.025) — Java's, kept.
pub fn calc_attribute_bonus(attack: f64, defence: f64) -> f64 {
    let diff = attack - defence;
    if diff > 0.0 {
        (1.025 + ((diff.powi(3) / 2.0).sqrt() * 0.0001)).min(1.25)
    } else if diff < 0.0 {
        (0.975 - (((-diff).powi(3) / 2.0).sqrt() * 0.0001)).max(0.75)
    } else {
        1.0
    }
}

/// `Formulas.calcAtkSpdMultiplier` (armorBonus = 1). The "weapon base" attack
/// speed for an unarmed player is the class template's `basePAtkSpd`.
pub fn calc_atk_spd_multiplier(
    p: &Player,
    base: &crate::model::components::BaseStats,
    mods: &crate::model::components::StatModifiers,
    data: &GameData,
    // `Stat.weaponBaseValue(creature, PHYSICAL_ATTACK_SPEED)` — the **equipped
    // weapon's** declared attack speed, which replaces the class base for a
    // player holding one (`IStatFunction.calcWeaponBaseValue`, the same rule
    // `PAttackSpeedFinalizer` runs on). `None` bare-handed.
    weapon_p_atk_spd: Option<f64>,
) -> f64 {
    let t = data
        .player_templates
        .get_or_base(p.class_id, p.base_class_id)
        .cloned()
        .unwrap_or_default();
    let dex_bonus = data.stat_bonus.bonus(BaseStat::Dex, base.dex);
    let mul = mods
        .mul
        .get(&Stat::PhysicalAttackSpeed)
        .copied()
        .unwrap_or(1.0);
    let add = mods
        .add
        .get(&Stat::PhysicalAttackSpeed)
        .copied()
        .unwrap_or(0.0);
    let weapon_attack_speed = weapon_p_atk_spd.unwrap_or(t.base_p_atk_spd as f64);
    dex_bonus * (weapon_attack_speed / 333.0) * mul + add / 333.0
}

/// `Formulas.calcMAtkSpdMultiplier` (armorBonus = 1).
pub fn calc_m_atk_spd_multiplier(
    base: &crate::model::components::BaseStats,
    mods: &crate::model::components::StatModifiers,
    data: &GameData,
) -> f64 {
    let wit_bonus = data.stat_bonus.bonus(BaseStat::Wit, base.wit);
    let mul = mods
        .mul
        .get(&Stat::MagicAttackSpeed)
        .copied()
        .unwrap_or(1.0);
    let add = mods
        .add
        .get(&Stat::MagicAttackSpeed)
        .copied()
        .unwrap_or(0.0);
    wit_bonus * mul + add / 333.0
}

/// `Formulas.calcSkillTimeFactor` — the divisor for hit/cancel time (the
/// channeling branch in `calc_cast_times` bypasses it for the hit phase);
/// the spiritshot hit-time term is 0.
pub fn calc_skill_time_factor(
    p: &Player,
    base: &crate::model::components::BaseStats,
    mods: &crate::model::components::StatModifiers,
    data: &GameData,
    skill: &Skill,
    // `creature.isChargedShot(SPIRITSHOTS) || isChargedShot(BLESSED_SPIRITSHOTS)`
    // — Java's `spiritshotHitTime` of **0.4**, i.e. a charged mage casts at
    // `matkSpdMul · 1.4`. Ignored for anything but a magic skill, as in Java.
    spiritshot_charged: bool,
    // The equipped weapon's attack speed, for the physical arm — see
    // [`calc_atk_spd_multiplier`].
    weapon_p_atk_spd: Option<f64>,
) -> f64 {
    // `skill.getOperateType().isChanneling()` heads the same early return as
    // the three static magic types: a channeled skill's timing is fixed, so
    // its factor is 1 and its cancel time is **not** divided by cast speed.
    if skill.operate_type == crate::model::skill::OperateType::Channeling
        || skill.magic_type == 2
        || skill.magic_type == 4
        || skill.magic_type == 21
    {
        return 1.0;
    }
    let factor = if skill.magic_type == 1 {
        let m = calc_m_atk_spd_multiplier(base, mods, data);
        // `factor = matkspdmul + (matkspdmul * spiritshotHitTime)`.
        m + (m * if spiritshot_charged { 0.4 } else { 0.0 })
    } else {
        calc_atk_spd_multiplier(p, base, mods, data, weapon_p_atk_spd)
    };
    factor.max(0.01)
}

/// `Formulas.calcSkillCancelTime` — the launch→finish phase length in ms.
pub fn calc_skill_cancel_time(
    p: &Player,
    base: &crate::model::components::BaseStats,
    mods: &crate::model::components::StatModifiers,
    data: &GameData,
    skill: &Skill,
    spiritshot_charged: bool,
    weapon_p_atk_spd: Option<f64>,
) -> f64 {
    ((skill.hit_cancel_time * 1000.0)
        / calc_skill_time_factor(
            p,
            base,
            mods,
            data,
            skill,
            spiritshot_charged,
            weapon_p_atk_spd,
        ))
    .max(SKILL_LAUNCH_TIME_MS)
}

/// `Formulas.calcAtkSpd` — the post-finish cool phase in ms (magic scales by
/// casting speed against the 333 base, physical by attack speed against 300).
pub fn calc_atk_spd(
    combat: &crate::model::components::CombatStats,
    skill: &Skill,
    skill_time: f64,
) -> i32 {
    if skill.magic_type == 1 {
        (skill_time / combat.m_atk_spd.max(1) as f64 * 333.0) as i32
    } else {
        (skill_time / combat.p_atk_spd.max(1) as f64 * 300.0) as i32
    }
}

/// `SkillCaster.calcSkillTiming` + the `startCasting` cool-time override →
/// `(hit_ms, cancel_ms, cool_ms)`. `calcSkillTiming` computes `_coolTime =
/// coolTime / timeFactor`, but `startCasting` immediately overwrites it with
/// `Formulas.calcAtkSpd(caster, skill, coolTime)` before it's ever used, so
/// only the override is ported. Client-displayed cast time (`MagicSkillUse` /
/// `SetupGauge`) is `hit + cancel`.
pub fn calc_cast_times(
    p: &Player,
    base: &crate::model::components::BaseStats,
    mods: &crate::model::components::StatModifiers,
    combat: &crate::model::components::CombatStats,
    data: &GameData,
    skill: &Skill,
    spiritshot_charged: bool,
    weapon_p_atk_spd: Option<f64>,
) -> (i32, i32, i32) {
    let factor = calc_skill_time_factor(
        p,
        base,
        mods,
        data,
        skill,
        spiritshot_charged,
        weapon_p_atk_spd,
    );
    let cancel = calc_skill_cancel_time(
        p,
        base,
        mods,
        data,
        skill,
        spiritshot_charged,
        weapon_p_atk_spd,
    );
    // Channeling (`CA1`) cast time is **static**: `_hitTime = max(hitTime −
    // cancelTime, 0)`, `_cancelTime = 2866` — no time-factor scaling, so
    // Volcano channels its full duration regardless of casting speed. The
    // cancel term does not scale either: `calcSkillTimeFactor` returns 1 for a
    // channeling skill, which this comment used to claim the opposite of.
    if skill.operate_type == crate::model::skill::OperateType::Channeling {
        let hit = (skill.hit_time as f64 - cancel).max(0.0) as i32;
        let cool = calc_atk_spd(combat, skill, skill.cool_time as f64);
        return (hit, 2866, cool);
    }
    let hit = (skill.hit_time as f64 / factor - cancel).max(0.0) as i32;
    let cool = calc_atk_spd(combat, skill, skill.cool_time as f64);
    (hit, cancel as i32, cool)
}

/// Which of `Heal.instant`'s three caster tests the effector answers to. Java
/// asks `isPlayer() && isMageClass()`, `isSummon()` and `isNpc()` in that order
/// and the arms differ in *both* their `mAtkMul` and their static bonus, so a
/// single "is it a player" flag cannot stand in for the choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealCaster {
    /// `isPlayer() && getClassId().isMage()`.
    PlayerMage,
    /// A player whose class is not a mage class.
    PlayerFighter,
    /// `isSummon()` — and note this one takes the shot branch **with no shot
    /// charged**, which is the only arm that does.
    Summon,
    /// A plain NPC: `isNpc()` and not a summon.
    Npc,
}

/// `handlers/effecthandlers/Heal.java` `instant()`, the amount half.
/// `HEAL_EFFECT`/`HEAL_EFFECT_ADD` and the healing-skill config multiplier are
/// the caller's, as is the overheal clamp; a magic crit triples the result.
///
/// ```java
/// double staticShotBonus = 0;
/// double mAtkMul = 1;
/// final double shotsBonus = effector.getStat().getValue(Stat.SHOTS_BONUS);
/// if (((sps || bss) && (effector.isPlayer() && effector.getActingPlayer().isMageClass())) || effector.isSummon())
/// {
///     staticShotBonus = skill.getMpConsume();
///     mAtkMul = bss ? 4 * shotsBonus : 2 * shotsBonus;
///     staticShotBonus *= bss ? 2.4 : 1.0;
/// }
/// else if ((sps || bss) && effector.isNpc())
/// {
///     staticShotBonus = 2.4 * skill.getMpConsume();
///     mAtkMul = 4 * shotsBonus;
/// }
/// else
/// {
///     if (weaponInst != null) mAtkMul = S84 ? 4 : S80 ? 2 : 1;
///     mAtkMul = bss ? mAtkMul * 4 : mAtkMul + 1;
/// }
/// amount += staticShotBonus + Math.sqrt(mAtkMul * effector.getMAtk());
/// ```
///
/// Three things separate the arms, and all three were collapsed here before:
///
/// * the **NPC** arm reaches `4 · shotsBonus` on *plain* spiritshots, where the
///   others need blessed ones, and pays `2.4 × mpConsume` unconditionally —
///   Java's comment calls it "always blessed spiritshots";
/// * the **summon** arm fires with **no shot charged at all**, so a servitor's
///   heal always takes the static bonus;
/// * the `else` arm reaches the same 4/2 by the *grade* road (no Interlude
///   weapon is S80/S84, so its `mAtkMul` starts at 1) and takes **no**
///   `shotsBonus` — which is what made it look interchangeable with the mage
///   arm for as long as `SHOTS_BONUS` was hard-coded to 1.
pub fn calc_heal(
    power: f64,
    m_atk: f64,
    mcrit: bool,
    sps: bool,
    bss: bool,
    mp_consume: i32,
    caster: HealCaster,
    shots_bonus: f64,
) -> f64 {
    let shot = sps || bss;
    let (static_bonus, m_atk_mul) =
        if (shot && caster == HealCaster::PlayerMage) || caster == HealCaster::Summon {
            (
                mp_consume as f64 * if bss { 2.4 } else { 1.0 },
                if bss { 4.0 } else { 2.0 } * shots_bonus,
            )
        } else if shot && caster == HealCaster::Npc {
            (2.4 * mp_consume as f64, 4.0 * shots_bonus)
        } else {
            // `mAtkMul` starts at the weapon's crystal grade, which is 1 for
            // everything this chronicle ships (S80/S84 are post-Interlude), so
            // `bss ? 1 * 4 : 1 + 1`.
            (0.0, if bss { 4.0 } else { 2.0 })
        };
    (power + static_bonus + (m_atk_mul * m_atk).sqrt()) * if mcrit { 3.0 } else { 1.0 }
}

// ---------------------------------------------------------------------------
// Physical auto-attack formulas (G9). Dropped terms, all identity for the
// actors that exist (unarmed/starting-gear players without parsed item
// `<stats>`, plain monsters): soulshots (`ss = false`, `SHOTS_BONUS`),
// shield defence (`calcShldUse` needs the un-parsed shield `sDef` stat —
// always 0/no-block), ranged/dual weapon branches, `ATTACK_COUNT_MAX`
// polearm sweeps, trait/attribute/pvp-pve multipliers, `CRITICAL_DAMAGE`/
// `CRITICAL_DAMAGE_ADD` stats (base 1/0), `HitConditionBonus`'s night/rain
// terms (no game clock/weather).
// ---------------------------------------------------------------------------

use crate::model::movement::Position;

/// `Formulas.calculateTimeBetweenAttacks`: full swing period in ms.
pub fn calculate_time_between_attacks(p_atk_spd: i32) -> i32 {
    (500_000 / p_atk_spd.max(1)).max(50)
}

/// `Formulas.calculateTimeToHit` for the melee branches (bows/duals are out
/// of scope): when the damage lands within the swing.
pub fn calculate_time_to_hit(total_attack_time: i32, two_handed: bool) -> i32 {
    (total_attack_time as f64 * if two_handed { 0.735 } else { 0.644 }) as i32
}

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

/// `Attackable.calculateExpAndSp`'s level-gap multiplier (the
/// "4gameforum" table): full reward through +2 levels above the mob,
/// tapering to 5% at +10 and beyond.
pub fn exp_sp_level_gap_multiplier(attacker_level: i32, npc_level: i32) -> f64 {
    match attacker_level - npc_level {
        i32::MIN..=2 => 1.0,
        3 => 0.97,
        4 => 0.80,
        5 => 0.61,
        6 => 0.37,
        7 => 0.22,
        8 => 0.13,
        9 => 0.08,
        _ => 0.05,
    }
}

/// `Util.map(value, min, max, targetMin, targetMax)` with the clamping Java's
/// `constrain` performs first — used by the drop level-gap chances.
pub fn map_range(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    let value = value.clamp(from_min.min(from_max), from_min.max(from_max));
    (value - from_min) * (to_max - to_min) / (from_max - from_min) + to_min
}

/// `Formulas.calcAtkBreak`. `men_bonus` is the target's `BaseStat.MEN` bonus;
/// `roll` is `Rnd.get(100)`. `cancel_add`/`cancel_mul` are the target's
/// `Stat.ATTACK_CANCEL` modifiers (Java `getStat().getValue(ATTACK_CANCEL,
/// init)`), which buffs like Concentration lower. The raid/HP-blocked/
/// channeling guards still don't apply to players yet.
///
/// `applies` is Java's `init > 0` test — it starts at **0** and only reaches
/// 15 when a branch of `AltGameCancelByHit` matches what the target is doing
/// (`ALT_GAME_CANCEL_CAST` while casting abortably, `ALT_GAME_CANCEL_BOW`
/// while mid-shot with a bow). `init <= 0` returns `false` outright, so with
/// the key set to neither, damage interrupts nothing at all. The port used to
/// hardcode the 15, which meant a cast was always interruptible however the
/// key was set.
pub fn calc_atk_break(
    dmg: f64,
    men_bonus: f64,
    roll: i32,
    cancel_add: f64,
    cancel_mul: f64,
    applies: bool,
) -> bool {
    if !applies {
        return false;
    }
    let init = 15.0 + (13.0 * dmg).sqrt() - (men_bonus * 100.0 - 100.0);
    let rate = (init * cancel_mul + cancel_add).clamp(1.0, 99.0);
    (roll as f64) < rate
}

/// `Formulas.calculatePvpPveBonus` — a multiplier every damage formula ends
/// with, and one this port previously hard-coded to 1.0 in three places
/// (`calc_auto_attack_damage`, `calc_physical_skill_damage` and
/// `calc_magic_dam` all carry a comment saying "pvp-pve mods 1.0"). That was
/// true only while nothing granted the stats; **~1300 dist effects grant
/// them**, so it stopped being true the moment any of them landed.
///
/// The shape is a *difference of multipliers* rather than a product: each side
/// merges as a `mul` (`amount 5` → 1.05), and the result is
/// `max(0.05, 1 + (attackMul − defenceMul))`. Two +5 % buffs on opposite sides
/// therefore cancel exactly.
///
/// Which pair is read depends on the pairing and the delivery:
/// - **playable vs playable** → the `PVP_*` triple;
/// - **either side `Attackable`** → the `PVE_*` triple, times the
///   level-difference penalty below;
/// - anything else (two NPCs that are not attackable, a door) → 1.0.
///
/// and within each, `skill = None` (an auto-attack) reads the
/// `*_PHYSICAL_ATTACK_*` pair, a magic skill the `*_MAGICAL_SKILL_*` pair, and
/// a physical skill the `*_PHYSICAL_SKILL_*` one.
///
/// Two upstream slips are ported as written because changing them would be a
/// silent divergence, and both are inert on this dist:
/// - Java binds `targetPlayer = attacker.getActingPlayer()` — the *attacker*
///   again — and then uses it only to index class-balance config arrays, which
///   are empty here (`Custom/ClassBalance.ini` ships every multiplier blank),
///   so every class multiplier is 1.0 and the slip cannot be observed.
/// - the raid `*_DEFENCE` terms are read off the **attacker**, not the target.
///
/// Dragon weapons (`DRAGON_WEAPON_DEFENCE`) do not exist in Interlude.
#[allow(clippy::too_many_arguments)]
pub fn calculate_pvp_pve_bonus(
    attack_mul: f64,
    defence_mul: f64,
    raid_attack_mul: f64,
    raid_defence_mul: f64,
    pve_penalty: f64,
) -> f64 {
    (1.0 + ((attack_mul * raid_attack_mul) - (defence_mul * raid_defence_mul))) * pve_penalty
}

/// The PvE half's level-difference penalty (`Config.NPC_SKILL_DMG_PENALTY`,
/// `0.8, 0.6, 0.5, 0.42, 0.36, 0.32, 0.28, 0.25` on this dist — steeper than
/// Mobius' four-entry default, and previously unparsed here).
///
/// It bites only when the **target** is a non-raid NPC of at least
/// `MinNPCLevelForDmgPenalty` (78) that is 2+ levels above the attacking
/// player: `levelDiff − 1` indexes the table, and past its end the last entry
/// (a flat ×0.25) applies. Note the two sibling tables, `DmgPenaltyForLvL-`
/// and `CritDmgPenaltyForLvLDifferences`, are parsed by Java's `Config` and
/// then read by **nothing** — dead config on both sides.
pub fn npc_level_damage_penalty(
    table: &[f64],
    target_level: i32,
    attacker_level: i32,
    target_is_raid: bool,
    min_npc_level: i32,
) -> f64 {
    if target_is_raid || target_level < min_npc_level || (target_level - attacker_level) < 2 {
        return 1.0;
    }
    if table.is_empty() {
        return 1.0;
    }
    let idx = (target_level - attacker_level - 1) as usize;
    table[idx.min(table.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// power 12 (Wind Strike 1), mAtk 100, mDef 60: 77·12·√100/60 = 154.0;
    /// magic crit doubles it.
    #[test]
    fn magic_dam_matches_java_formula() {
        let none = MagicFailure::None;
        let dmg = calc_magic_dam(100.0, 60.0, 12.0, false, 2.0, 1.0, none, 1.0);
        assert!((dmg - 154.0).abs() < 1e-9);
        let crit = calc_magic_dam(100.0, 60.0, 12.0, true, 2.0, 1.0, none, 1.0);
        assert!((crit - 308.0).abs() < 1e-9);
        // Spiritshot doubles, blessed spiritshot quadruples the base.
        assert!(
            (calc_magic_dam(100.0, 60.0, 12.0, false, 2.0, 2.0, none, 1.0) - 308.0).abs() < 1e-9
        );
        assert!(
            (calc_magic_dam(100.0, 60.0, 12.0, false, 2.0, 4.0, none, 1.0) - 616.0).abs() < 1e-9
        );
    }

    /// mDef is floored at 1 so a zero-defence target can't divide by zero.
    #[test]
    fn magic_dam_survives_zero_mdef() {
        assert!(
            calc_magic_dam(100.0, 0.0, 12.0, false, 2.0, 1.0, MagicFailure::None, 1.0).is_finite()
        );
    }

    /// Java applies the `MagicFailures` adjustment to the *base* damage and only
    /// then multiplies by `critMod` — so a resisted crit lands on 2, not 1, and
    /// a halved crit keeps the full ×2.
    #[test]
    fn magic_failure_applies_before_the_crit_multiplier() {
        assert!(
            (calc_magic_dam(100.0, 60.0, 12.0, false, 2.0, 1.0, MagicFailure::Half, 1.0) - 77.0)
                .abs()
                < 1e-9
        );
        assert!(
            (calc_magic_dam(100.0, 60.0, 12.0, true, 2.0, 1.0, MagicFailure::Half, 1.0) - 154.0)
                .abs()
                < 1e-9
        );
        assert!(
            (calc_magic_dam(
                100.0,
                60.0,
                12.0,
                false,
                2.0,
                1.0,
                MagicFailure::Resisted,
                1.0
            ) - 1.0)
                .abs()
                < 1e-9
        );
        assert!(
            (calc_magic_dam(
                100.0,
                60.0,
                12.0,
                true,
                2.0,
                1.0,
                MagicFailure::Resisted,
                1.0
            ) - 2.0)
                .abs()
                < 1e-9
        );
    }

    /// A PvE cast with no level gap and no level-78 penalty: `1.3^0 = 1`, so
    /// `rate = 100 - 1 = 99`.
    fn pve_input(target_level: i32, effective_level: i32) -> MagicSuccess<'static> {
        MagicSuccess {
            pve: true,
            target_level,
            effective_level,
            caster_player_level: Some(effective_level),
            target_is_raid: false,
            min_npc_level_for_magic_penalty: 78,
            skill_chance_penalty: &[2.5, 3.0, 3.25, 3.5],
            magic_accuracy: 0,
            magic_evasion: 0,
            res_modifier: 1.0,
        }
    }

    /// `rate = 100 - round(1.3^levelDiff)`. The curve is the whole reason a nuke
    /// on a far-higher-level mob has to fail: by +18 levels the rate is negative,
    /// so `Rnd.get(100) < rate` can never be true.
    #[test]
    fn magic_success_rate_follows_the_1_3_power_curve() {
        assert_eq!(calc_magic_success_rate(&pve_input(40, 40)), 99);
        assert_eq!(calc_magic_success_rate(&pve_input(46, 40)), 95); // 1.3^6  = 4.83  → 5
        assert_eq!(calc_magic_success_rate(&pve_input(49, 40)), 89); // 1.3^9  = 10.6  → 11
        assert_eq!(calc_magic_success_rate(&pve_input(52, 40)), 77); // 1.3^12 = 23.3  → 23
        assert_eq!(calc_magic_success_rate(&pve_input(55, 40)), 49); // 1.3^15 = 51.2  → 51
        assert!(calc_magic_success_rate(&pve_input(58, 40)) < 0); // 1.3^18 = 112.5
    }

    /// Casting *up* the level curve is free: a high-level caster on a low mob
    /// saturates at 100 (`1.3^-n` rounds to 0).
    #[test]
    fn magic_success_rate_saturates_against_lower_targets() {
        assert_eq!(calc_magic_success_rate(&pve_input(20, 40)), 100);
    }

    /// The roll is `Rnd.get(100) < rate`, so a rate of 49 lands on rolls 0-48.
    #[test]
    fn magic_success_roll_is_exclusive() {
        let input = pve_input(55, 40);
        assert!(calc_magic_success(&input, 48));
        assert!(!calc_magic_success(&input, 49));
        // A negative rate can never be rolled under, even at roll 0.
        assert!(!calc_magic_success(&pve_input(58, 40), 0));
    }

    /// `MinNPCLevelForMagicPenalty` (78) gates the extra `targetModifier`. At 78+
    /// with the caster 3+ levels below, the failure term is multiplied by the
    /// penalty table entry at `targetLevel - casterLevel - 2`, clamped to the last.
    #[test]
    fn magic_success_applies_the_level_78_npc_penalty() {
        // Target 78, caster 75 → levelDiff index 1 → 3.0. 1.3^3 = 2.197.
        // 100 - round(2.197 * 3.0) = 100 - round(6.59) = 93.
        let mut input = pve_input(78, 75);
        assert_eq!(calc_magic_success_rate(&input), 93);
        // Same gap against a 77 mob: below the threshold, so no targetModifier.
        // 100 - round(2.197) = 98.
        let below = pve_input(77, 74);
        assert_eq!(calc_magic_success_rate(&below), 98);
        // Raids are exempt from the penalty even at 78+.
        input.target_is_raid = true;
        assert_eq!(calc_magic_success_rate(&input), 98);
    }

    /// An NPC caster (`getActingPlayer() == null`) never picks up the penalty —
    /// Java's null check comes before the level test.
    #[test]
    fn magic_success_npc_caster_skips_the_penalty() {
        let mut input = pve_input(78, 75);
        input.caster_player_level = None;
        assert_eq!(calc_magic_success_rate(&input), 98);
    }

    /// The index clamps to the last table entry rather than panicking on a gap
    /// wider than the table (Java's `levelDiff >= length` branch).
    #[test]
    fn magic_success_penalty_index_clamps() {
        // Target 99, caster 78: index 19, table has 4 entries → 3.5.
        // Both the clamped and the out-of-range case drive the rate negative.
        assert!(calc_magic_success_rate(&pve_input(99, 78)) < 0);
    }

    /// PvP (neither side an `Attackable`) takes the magic-accuracy step table
    /// instead; the level gap is irrelevant there.
    #[test]
    fn magic_success_pvp_uses_the_accuracy_table() {
        let mut input = pve_input(80, 40);
        input.pve = false;
        // mAccDiff 0 > -20 → mAccModifier 2 → rate 98, despite the 40-level gap.
        assert_eq!(calc_magic_success_rate(&input), 98);
        input.magic_evasion = 22; // diff -22 → 30
        assert_eq!(calc_magic_success_rate(&input), 70);
        input.magic_evasion = 27; // diff -27 → 60
        assert_eq!(calc_magic_success_rate(&input), 40);
        input.magic_evasion = 32; // diff -32 → 90
        assert_eq!(calc_magic_success_rate(&input), 10);
        input.magic_evasion = 40; // diff -40 → 100
        assert_eq!(calc_magic_success_rate(&input), 0);
    }

    /// Decrease Speed 1 (magicLevel 35, activateRate 80, lvlBonusRate 30): the
    /// steep lvlBonusRate caps the chance at 90 vs a low-level mob and floors it
    /// at 10 vs a much higher-level one. `activateRate == -1` always lands (100),
    /// and a skill with no magic level uses `targetLevel + 3`.
    #[test]
    fn effect_land_rate_clamps_and_special_cases() {
        // (35 - 5 + 3)·30 + 80 + 30 = 1100 → clamp to 90.
        assert!(
            (calc_effect_land_rate(35, 80, 30, 5, 1.0, 1.0, 1.0, 0.0, 1.0, Default::default())
                - 90.0)
                .abs()
                < 1e-9
        );
        // (35 - 80 + 3)·30 + 80 + 30 = -1150 → clamp to 10.
        assert!(
            (calc_effect_land_rate(35, 80, 30, 80, 1.0, 1.0, 1.0, 0.0, 1.0, Default::default())
                - 10.0)
                .abs()
                < 1e-9
        );
        // activateRate -1 → guaranteed.
        assert!(
            (calc_effect_land_rate(35, -1, 30, 5, 1.0, 1.0, 1.0, 0.0, 1.0, Default::default())
                - 100.0)
                .abs()
                < 1e-9
        );
        // magicLevel <= -1 falls back to targetLevel + 3, so the level term is
        // (23 - 20 + 3) = 6: 6·5 + 10 + 30 = 70.
        assert!(
            (calc_effect_land_rate(-1, 10, 5, 20, 1.0, 1.0, 1.0, 0.0, 1.0, Default::default())
                - 70.0)
                .abs()
                < 1e-9
        );
    }

    /// The trait multiplier is folded in **before** the clamp, alongside the
    /// element and debuff-resist mods, so Stun Resistance 3 (30 %) takes a
    /// mid-range stun from 50 to 35. **Invulnerability is the exception**: it
    /// skips the clamp entirely (`traitMod > 0 ? constrain(…) : 0`), so an
    /// immune target refuses the debuff outright instead of taking it at the
    /// 10 % floor. And a *negative* defence trait is a vulnerability, which
    /// pushes the rate up into the ceiling.
    #[test]
    fn effect_land_rate_folds_the_trait_bonus_in_before_clamping() {
        // (20 - 20 + 3)·5 + 5 + 30 = 50 unresisted.
        assert!(
            (calc_effect_land_rate(20, 5, 5, 20, 1.0, 1.0, 1.0, 0.0, 1.0, Default::default())
                - 50.0)
                .abs()
                < 1e-9
        );
        // 30 % trait resistance → 0.70 → 35.
        assert!(
            (calc_effect_land_rate(20, 5, 5, 20, 1.0, 1.0, 0.70, 0.0, 1.0, Default::default())
                - 35.0)
                .abs()
                < 1e-9
        );
        // Invulnerable → 0, not the 10 floor.
        assert_eq!(
            calc_effect_land_rate(20, 5, 5, 20, 1.0, 1.0, 0.0, 0.0, 1.0, Default::default()),
            0.0
        );
        // A vulnerability (defence -15 → 1.15) raises it: 50 · 1.15 = 57.5.
        assert!(
            (calc_effect_land_rate(20, 5, 5, 20, 1.0, 1.0, 1.15, 0.0, 1.0, Default::default())
                - 57.5)
                .abs()
                < 1e-9
        );
        // It composes with the other two mods rather than replacing them.
        assert!(
            (calc_effect_land_rate(20, 5, 5, 20, 0.8, 1.25, 0.70, 0.0, 1.0, Default::default())
                - 35.0)
                .abs()
                < 1e-9
        );
        // The always-lands escape hatch is checked first, so even immunity
        // cannot stop an `activateRate == -1` debuff (Java returns true before
        // computing any mod).
        assert_eq!(
            calc_effect_land_rate(20, -1, 5, 20, 1.0, 1.0, 0.0, 0.0, 1.0, Default::default()),
            100.0
        );
    }

    /// Good skills cap the per-mille rate at 320, bad skills at 200; the
    /// comparison is strict (`rate > roll`).
    #[test]
    fn magic_crit_caps_and_thresholds() {
        assert!(calc_magic_crit(1000.0, false, 40, 40, 319));
        assert!(!calc_magic_crit(1000.0, false, 40, 40, 320));
        assert!(calc_magic_crit(1000.0, true, 40, 40, 199));
        assert!(!calc_magic_crit(1000.0, true, 40, 40, 200));
        assert!(!calc_magic_crit(0.0, false, 40, 40, 0));
    }

    /// The bad-skill cap lifts to 320 once **both** sides are 78+, and the
    /// `sqrt(level)` bonus rides in with it — a good skill never reaches that
    /// branch, since Java returns before it.
    #[test]
    fn magic_crit_lifts_its_cap_for_high_level_pairs() {
        assert!(calc_magic_crit(1000.0, true, 78, 78, 319));
        assert!(!calc_magic_crit(1000.0, true, 78, 78, 320));
        // One side below 78 keeps the 200 cap.
        assert!(!calc_magic_crit(1000.0, true, 78, 77, 200));
        // 100%o at level 81 → 100 + 9 = 109 (the level-gap term is Java's own
        // integer division, 0 for any gap under 25).
        assert!(calc_magic_crit(100.0, true, 81, 78, 108));
        assert!(!calc_magic_crit(100.0, true, 81, 78, 109));
    }

    /// Heal: power 83, mAtk 50 → 83 + √100 = 93; crit triples.
    #[test]
    fn heal_matches_java_formula() {
        use HealCaster::{Npc, PlayerFighter, PlayerMage, Summon};
        assert!(
            (calc_heal(83.0, 50.0, false, false, false, 0, PlayerMage, 1.0) - 93.0).abs() < 1e-9
        );
        assert!(
            (calc_heal(83.0, 50.0, true, false, false, 0, PlayerMage, 1.0) - 279.0).abs() < 1e-9
        );
        // Spiritshot on a mage caster adds the MP-consume static bonus (sqrt
        // term unchanged at ×2): 83 + 40 + √100 = 133.
        assert!(
            (calc_heal(83.0, 50.0, false, true, false, 40, PlayerMage, 1.0) - 133.0).abs() < 1e-9
        );
        // Blessed spiritshot: sqrt term ×4 (√200) and static ×2.4: 83 + 96 + √200.
        assert!(
            (calc_heal(83.0, 50.0, false, false, true, 40, PlayerMage, 1.0)
                - (83.0 + 96.0 + 200.0_f64.sqrt()))
            .abs()
                < 1e-9
        );

        // The three arms Java keeps apart, on the same inputs (plain
        // spiritshot, mpConsume 40, `SHOTS_BONUS` 1.03):
        // - a **fighter** falls through to the grade arm: no static bonus, ×2,
        //   and no shots bonus at all;
        let grade_arm = 83.0 + (2.0 * 50.0f64).sqrt();
        assert!(
            (calc_heal(83.0, 50.0, false, true, false, 40, PlayerFighter, 1.03) - grade_arm).abs()
                < 1e-9
        );
        // - an **NPC** gets `2.4 × mpConsume` and ×4 even on a *plain* shot;
        assert!(
            (calc_heal(83.0, 50.0, false, true, false, 40, Npc, 1.03)
                - (83.0 + 96.0 + (4.0 * 1.03 * 50.0f64).sqrt()))
            .abs()
                < 1e-9
        );
        // - a **summon** takes the mage arm **with no shot charged**.
        assert!(
            (calc_heal(83.0, 50.0, false, false, false, 40, Summon, 1.03)
                - (83.0 + 40.0 + (2.0 * 1.03 * 50.0f64).sqrt()))
            .abs()
                < 1e-9
        );
    }

    use crate::model::movement::Position;

    /// `500000 / atkSpd`, floored at 50 ms.
    #[test]
    fn time_between_attacks_matches_java() {
        assert_eq!(calculate_time_between_attacks(300), 1666);
        assert_eq!(calculate_time_between_attacks(1_000_000), 50);
    }

    /// Melee time-to-hit fractions (0.644 / 0.735 two-handed).
    #[test]
    fn time_to_hit_fractions() {
        assert_eq!(calculate_time_to_hit(1666, false), 1072);
        assert_eq!(calculate_time_to_hit(1666, true), 1224);
    }

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

    /// The level-gap XP table: full through +2, tapering to 5%.
    #[test]
    fn exp_gap_table() {
        assert_eq!(exp_sp_level_gap_multiplier(10, 20), 1.0);
        assert_eq!(exp_sp_level_gap_multiplier(12, 10), 1.0);
        assert_eq!(exp_sp_level_gap_multiplier(13, 10), 0.97);
        assert_eq!(exp_sp_level_gap_multiplier(19, 10), 0.08);
        assert_eq!(exp_sp_level_gap_multiplier(40, 10), 0.05);
    }

    /// `Util.map` with constrain: the drop level-gap scaling from 100% at
    /// −min to the floor at −max, clamped outside.
    #[test]
    fn map_range_matches_util_map() {
        assert!((map_range(-8.0, -15.0, -8.0, 10.0, 100.0) - 100.0).abs() < 1e-9);
        assert!((map_range(-15.0, -15.0, -8.0, 10.0, 100.0) - 10.0).abs() < 1e-9);
        assert!(
            (map_range(-20.0, -15.0, -8.0, 10.0, 100.0) - 10.0).abs() < 1e-9,
            "clamped below"
        );
        assert!(
            (map_range(3.0, -15.0, -8.0, 10.0, 100.0) - 100.0).abs() < 1e-9,
            "clamped above"
        );
    }

    /// Neutral MEN (bonus 1.0): rate = 15 + √(13·dmg), clamped to [1, 99].
    #[test]
    fn atk_break_rate_and_clamps() {
        // dmg 100 → 15 + √1300 ≈ 51.06: roll 51 breaks, 52 doesn't.
        assert!(calc_atk_break(100.0, 1.0, 51, 0.0, 1.0, true));
        assert!(!calc_atk_break(100.0, 1.0, 52, 0.0, 1.0, true));
        // Huge MEN bonus can't push the rate below 1%.
        assert!(calc_atk_break(0.0, 2.0, 0, 0.0, 1.0, true));
        assert!(!calc_atk_break(0.0, 2.0, 1, 0.0, 1.0, true));
        // Massive damage caps at 99% — roll 99 still survives.
        assert!(!calc_atk_break(1e9, 1.0, 99, 0.0, 1.0, true));
    }

    /// `Stat.ATTACK_CANCEL`: Concentration's -18 DIFF lowers the interrupt rate
    /// (≈51.06 → ≈33.06), so a roll of 40 now survives where it broke before.
    #[test]
    fn atk_break_honors_cancel_stat() {
        assert!(
            calc_atk_break(100.0, 1.0, 40, 0.0, 1.0, true),
            "no buff: 40 < 51.06 breaks"
        );
        assert!(
            !calc_atk_break(100.0, 1.0, 40, -18.0, 1.0, true),
            "Concentration: 40 > 33.06 survives"
        );
    }
}
