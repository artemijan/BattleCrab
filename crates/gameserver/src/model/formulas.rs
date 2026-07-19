//! Port of `model/stats/Formulas.java`, scoped to what the single-target cast
//! pipeline needs: magic damage, magic crit, cast timing, heal, and the
//! cast-break-on-hit roll. Every function documents the Java method it ports
//! and which terms are dropped. The dropped terms are all identity values for
//! an unarmed, shotless player with no trait/attribute stats — the only kind
//! of actor that exists so far: `SHOTS_BONUS`/spiritshots (1.0/absent),
//! trait/weakness/attribute mods (1.0), `SKILL_POWER_ADD` (0),
//! `RANDOM_DAMAGE` (weapon-supplied, unarmed = 0 → randomMod 1.0),
//! pvp/pve config multipliers (1.0 by default), `MAGICAL_SKILL_POWER` (1.0).

use crate::data::GameData;
use crate::model::skill::Skill;
use crate::model::stats::{BaseStat, Stat};
use crate::model::Player;

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
/// trait/attribute/random/pvpPve mods (all 1.0 here), so the halving and the
/// `damage = 1` floor are applied here *ahead* of `mcrit`.
pub fn calc_magic_dam(
    m_atk: f64,
    m_def: f64,
    power: f64,
    mcrit: bool,
    shots_bonus: f64,
    failure: MagicFailure,
) -> f64 {
    let mut damage = (77.0 * power * m_atk.sqrt() / m_def.max(1.0)) * shots_bonus;
    match failure {
        MagicFailure::None => {}
        MagicFailure::Half => damage /= 2.0,
        MagicFailure::Resisted => damage = 1.0,
    }
    damage * if mcrit { 2.0 } else { 1.0 }
}

/// `Formulas.calcCrit`'s magic branch for both-below-level-78 actors (base
/// classes cap at 40/76 here). `m_crit_rate` is the per-mille
/// `Player.m_crit_hit`; `roll` is `Rnd.get(1000)`. Good skills cap at 320‰,
/// bad skills at 200‰ (`DEFENCE_MAGIC_CRITICAL_RATE` defaults to the
/// attacker's rate and the balance multipliers to 1.0, so both drop out).
pub fn calc_magic_crit(m_crit_rate: f64, is_bad: bool, roll: i32) -> bool {
    let cap = if is_bad { 200.0 } else { 320.0 };
    m_crit_rate.min(cap) > roll as f64
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
/// Java's `resModifier` (`getMul(MAGIC_SUCCESS_RES, 1)`) is fixed at 1.0 here.
/// The only two dist items touching `magicSuccRes` (10207/10208, the enhanced
/// shirts) declare it in a `<stats>` block, which Java parses into an *additive*
/// func — `getMul` never sees it, so the term is 1.0 on this dist for Java too.
pub fn calc_magic_success_rate(i: &MagicSuccess) -> i32 {
    let mut lvl_modifier = 1.0f64;
    let mut target_modifier = 1.0f64;
    let mut m_acc_modifier = 1i32;

    if i.pve {
        lvl_modifier = 1.3f64.powi(i.target_level - i.effective_level);

        if let Some(caster_level) = i.caster_player_level {
            if !i.target_is_raid
                && i.target_level >= i.min_npc_level_for_magic_penalty
                && (i.target_level - caster_level) >= 3
                && !i.skill_chance_penalty.is_empty()
            {
                let level_diff = (i.target_level - caster_level - 2) as usize;
                target_modifier = i.skill_chance_penalty[level_diff.min(i.skill_chance_penalty.len() - 1)];
            }
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

    100 - java_round_float(m_acc_modifier as f64 * lvl_modifier * target_modifier)
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

/// `Formulas.calcEffectSuccess` — a debuff's landing chance in percent (0-100),
/// reduced to the factors the port currently models. Java scales `baseMod` by an
/// attribute (element) bonus, a trait resist/vulnerability bonus and a
/// `RESIST_ABNORMAL_DEBUFF` mul, `constrain`s to `[minChance, maxChance]`, then
/// scales by a `BasicPropertyResist` bonus — and subtracts an `ABNORMAL_RESIST_*`
/// term up front. None of those stats are modeled server-side yet, so each is 1.0
/// (or 0 for the resist subtrahend), leaving:
///   baseMod   = (magicLevel - targetLevel + 3) * lvlBonusRate + activateRate + 30
///   finalRate = constrain(baseMod, 10, 90)
/// `magicLevel <= -1` falls back to `targetLevel + 3` (Java). `activateRate == -1`
/// means the debuff always lands → 100. The 10/90 clamp is dist `Character.ini`'s
/// Min/MaxAbnormalStateSuccessRate (no Interlude skill overrides minChance/maxChance).
/// TODO(G16): fold in the element/trait/`ABNORMAL_RESIST_*`/`RESIST_ABNORMAL_DEBUFF`/
/// `BasicPropertyResist` bonuses once those stats land.
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
) -> f64 {
    if activate_rate == -1 {
        return 100.0;
    }
    let magic_level = if magic_level <= -1 { target_level + 3 } else { magic_level };
    let base_mod = (magic_level - target_level + 3) * lvl_bonus_rate + activate_rate + 30;
    // Java multiplies the raw rate by the resist mod and clamps *after*
    // (`constrain(baseMod * … * buffDebuffMod, minChance, maxChance)`), so a
    // heavy resistance can pull an otherwise-capped debuff below the 90 ceiling
    // but never under the 10 floor.
    (base_mod as f64 * debuff_resist_mod).clamp(10.0, 90.0)
}

/// `Formulas.calcAtkSpdMultiplier` (armorBonus = 1). The "weapon base" attack
/// speed for an unarmed player is the class template's `basePAtkSpd`.
pub fn calc_atk_spd_multiplier(
    p: &Player,
    base: &crate::model::components::BaseStats,
    mods: &crate::model::components::StatModifiers,
    data: &GameData,
) -> f64 {
    let t = data
        .player_templates
        .get(p.class_id)
        .or_else(|| data.player_templates.get(p.base_class_id))
        .cloned()
        .unwrap_or_default();
    let dex_bonus = data.stat_bonus.bonus(BaseStat::Dex, base.dex);
    let mul = mods.mul.get(&Stat::PhysicalAttackSpeed).copied().unwrap_or(1.0);
    let add = mods.add.get(&Stat::PhysicalAttackSpeed).copied().unwrap_or(0.0);
    dex_bonus * (t.base_p_atk_spd as f64 / 333.0) * mul + add / 333.0
}

/// `Formulas.calcMAtkSpdMultiplier` (armorBonus = 1).
pub fn calc_m_atk_spd_multiplier(
    base: &crate::model::components::BaseStats,
    mods: &crate::model::components::StatModifiers,
    data: &GameData,
) -> f64 {
    let wit_bonus = data.stat_bonus.bonus(BaseStat::Wit, base.wit);
    let mul = mods.mul.get(&Stat::MagicAttackSpeed).copied().unwrap_or(1.0);
    let add = mods.add.get(&Stat::MagicAttackSpeed).copied().unwrap_or(0.0);
    wit_bonus * mul + add / 333.0
}

/// `Formulas.calcSkillTimeFactor` — the divisor for hit/cancel time. No
/// channeling skills or NPCs exist; the spiritshot hit-time term is 0.
pub fn calc_skill_time_factor(
    p: &Player,
    base: &crate::model::components::BaseStats,
    mods: &crate::model::components::StatModifiers,
    data: &GameData,
    skill: &Skill,
) -> f64 {
    if skill.magic_type == 2 || skill.magic_type == 4 || skill.magic_type == 21 {
        return 1.0;
    }
    let factor = if skill.magic_type == 1 {
        calc_m_atk_spd_multiplier(base, mods, data)
    } else {
        calc_atk_spd_multiplier(p, base, mods, data)
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
) -> f64 {
    ((skill.hit_cancel_time * 1000.0) / calc_skill_time_factor(p, base, mods, data, skill)).max(SKILL_LAUNCH_TIME_MS)
}

/// `Formulas.calcAtkSpd` — the post-finish cool phase in ms (magic scales by
/// casting speed against the 333 base, physical by attack speed against 300).
pub fn calc_atk_spd(combat: &crate::model::components::CombatStats, skill: &Skill, skill_time: f64) -> i32 {
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
) -> (i32, i32, i32) {
    let factor = calc_skill_time_factor(p, base, mods, data, skill);
    let cancel = calc_skill_cancel_time(p, base, mods, data, skill);
    let hit = (skill.hit_time as f64 / factor - cancel).max(0.0) as i32;
    let cool = calc_atk_spd(combat, skill, skill.cool_time as f64);
    (hit, cancel as i32, cool)
}

/// `handlers/effecthandlers/Heal.java` `instant()`, narrowed to the player
/// caster path: `HEAL_EFFECT`/`HEAL_EFFECT_ADD` stats absent (×1/+0),
/// healing-skill config multiplier 1.0, `SHOTS_BONUS` stat 1.0. Magic crit
/// triples the heal; the overheal clamp is the caller's job.
///
/// Spiritshots (`sps`/`bss`): the `sqrt` multiplier is `bss ? 4 : 2` (Java's
/// `mAtkMul` collapses to this for both the mage and no-grade-weapon branches),
/// and the mage branch adds a static bonus from the skill's MP consume
/// (`bss ? mpConsume*2.4 : mpConsume`). `is_mage_caster` gates that static
/// bonus — Java's `isMageClass()`; approximated as "the caster is a player"
/// (every Interlude heal-casting class is a mage class, and NPC heals don't
/// reach this fn). Shotless (`sps == bss == false`) reproduces the old
/// `sqrt(2·mAtk)`.
pub fn calc_heal(power: f64, m_atk: f64, mcrit: bool, sps: bool, bss: bool, mp_consume: i32, is_mage_caster: bool) -> f64 {
    let m_atk_mul = if bss { 4.0 } else { 2.0 };
    let static_bonus = if (sps || bss) && is_mage_caster {
        mp_consume as f64 * if bss { 2.4 } else { 1.0 }
    } else {
        0.0
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

/// `Formulas.calcCriticalPositionBonus` with the positional `CRITICAL_RATE`
/// stat values at their default 1.0: 10% from the side, 30% from the back.
pub fn calc_critical_position_bonus(position: Position) -> f64 {
    match position {
        Position::Side => 1.1,
        Position::Back => 1.3,
        Position::Front => 1.0,
    }
}

/// `Formulas.calcCriticalHeightBonus`: ±10% band from the z difference.
pub fn calc_critical_height_bonus(from_z: i32, to_z: i32) -> f64 {
    ((((from_z - to_z).clamp(-25, 25) * 4 / 5) + 10) as f64 / 100.0) + 1.0
}

/// `Formulas.calcCrit`'s auto-attack branch for sub-78 actors
/// (`DEFENCE_CRITICAL_RATE` defaults to the attacker's rate, balance
/// multipliers 1.0): `rate = posBonus · (critStat / 10) · heightBonus`,
/// clamped to [3, 97] percent; crit when `rate > roll` (`Rnd.get(100)`).
pub fn calc_auto_attack_crit(crit_stat: f64, position: Position, from_z: i32, to_z: i32, roll: i32) -> bool {
    let rate = calc_critical_position_bonus(position) * (crit_stat / 10.0) * calc_critical_height_bonus(from_z, to_z);
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
pub fn calc_shield_use(shield_rate: f64, con_bonus: f64, ranged: bool, from_behind: bool, rate_roll: i32, perfect_roll: i32) -> u8 {
    if shield_rate <= 0.0 || from_behind {
        return SHIELD_NONE;
    }
    // A bow attacker raises the block rate by 30% (Java).
    let rate = if ranged { shield_rate * 1.3 } else { shield_rate };
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

/// `Formulas.calcAutoAttackDamage`, melee/shotless narrowing (see the module
/// note): `attack = pAtk·randomMul + proxBonus`, ×77, doubled by a crit
/// (`calcCritDamage` = 2 with default crit-damage stats), over the target's
/// `pDef`. `position` is the attacker's position relative to the target
/// (front 0, side +5%, back +20% of pAtk).
pub fn calc_auto_attack_damage(p_atk: f64, random_mul: f64, position: Position, p_def: f64, crit: bool, ss: bool) -> f64 {
    let prox_bonus = match position {
        Position::Front => 0.0,
        Position::Side => 0.05,
        Position::Back => 0.2,
    } * p_atk;
    // `ssBonus` = `ss ? 2 : 1` (blessed soulshots — 2.15 — don't exist in
    // Interlude; times `SHOTS_BONUS`, 1.0 here).
    let ss_bonus = if ss { 2.0 } else { 1.0 };
    let attack = (p_atk * random_mul + prox_bonus) * ss_bonus * if crit { 2.0 } else { 1.0 } * 77.0;
    (attack / p_def.max(1.0)).max(0.0)
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
/// (soulshots handled via `ss`; trait/weakness/attribute/pvp-pve mods 1.0,
/// `SKILL_POWER_ADD` 0, `PHYSICAL_SKILL_POWER` 1, abnormal/race mods 1.0). The
/// ranged branch (`weaponMod` 70 with the `+pAtk+power` bonus) is deferred with
/// bows (G20); this uses the melee `weaponMod` 77. Shield defence is folded into
/// `p_def` by the caller (perfect block → caller passes damage-1 path).
///
/// `damage = 77·((pAtk·pAtkMod)·levelMod + power) / (pDef·pDefMod) · ssMod ·
/// critMod · randomMod`, where `ssMod = ss ? 2 : 1`, `critMod` = `calcCritDamage`'s
/// physical-skill value (2 with default crit-damage stats).
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
    ss: bool,
) -> f64 {
    let attack = p_atk * p_atk_mod;
    let defence = (p_def * p_def_mod).max(1.0);
    let weapon_mod = 77.0;
    let ss_mod = if ss { 2.0 } else { 1.0 };
    let crit_mod = if crit { 2.0 } else { 1.0 };
    let base_mod = (weapon_mod * ((attack * level_mod) + power)) / defence;
    (base_mod * ss_mod * crit_mod * random_mul).max(0.0)
}

/// `Creature.getLevelMod`: `(level + 89) / 100` (transform stances aside).
pub fn level_mod(level: i32) -> f64 {
    (level as f64 + 89.0) / 100.0
}

/// `Formulas.calcBlowDamage` (dagger blows: FatalBlow/Backstab/SoulBlow),
/// melee/identity-simplified. The crit-damage/trait/attribute/pvp-pve
/// multipliers are all identity for the actors that exist (default crit-damage
/// stats, no traits/attributes) → `cdMult = 1`, `cdPatk = 0`, so only the base
/// blow term survives. `position` adds 20% (back) / 5% (side) of `(power+pAtk)`
/// before the ×77. Shield is folded into `p_def` by the caller (perfect block →
/// the caller shortcuts to 1). `SKILL_POWER_ADD` is 0.
///
/// `77·((power+pAtk)·0.666 + isPos·(power+pAtk)·randomMul) / pDef · ssMod · randomMul`
pub fn calc_blow_damage(p_atk: f64, power: f64, p_def: f64, position: Position, random_mul: f64, ss: bool) -> f64 {
    let is_pos = match position {
        Position::Back => 0.2,
        Position::Side => 0.05,
        Position::Front => 0.0,
    };
    let sum = power + p_atk;
    let ss_mod = if ss { 2.0 } else { 1.0 };
    let base_mod = (77.0 * ((sum * 0.666) + (is_pos * sum * random_mul))) / p_def.max(1.0);
    (base_mod * ss_mod * random_mul).max(0.0)
}

/// `Formulas.calcBlowSuccess` — the "does the blow land" roll (part of the blow
/// effect's `calcSuccess`). `crit_rate` is the caster's finalized crit rate ÷10,
/// standing in for Java's `weaponCritical` (the weapon's raw `CRITICAL_RATE`
/// excluding the DEX bonus); the `limit` cap (`BlowRateChanceLimit`, 100 on
/// dist) dominates for a real dagger user, so the proxy's small overshoot is
/// absorbed. `BLOW_RATE`/`BLOW_RATE_DEFENCE` stats are 1.0. Lands when `roll`
/// (`Rnd.get(100)`) < min(rate, limit).
/// TODO(G20): use the weapon's raw crit rate once weapon stats are exposed.
pub fn calc_blow_success(
    crit_rate: f64,
    position: Position,
    from_z: i32,
    to_z: i32,
    chance_boost: f64,
    limit: f64,
    roll: i32,
) -> bool {
    let rate = calc_critical_position_bonus(position)
        * calc_critical_height_bonus(from_z, to_z)
        * crit_rate
        * ((100.0 + chance_boost) / 100.0);
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

/// `Formulas.calcAtkBreak`, `ALT_GAME_CANCEL_CAST` branch (default config
/// `AltGameCancelByHit = cast`): the caller must already have checked that
/// the target is casting and still abortable (pre-launch) — that check is
/// what sets `init = 15`. `men_bonus` is the target's `BaseStat.MEN` bonus;
/// `roll` is `Rnd.get(100)`. `cancel_add`/`cancel_mul` are the target's
/// `Stat.ATTACK_CANCEL` modifiers (Java `getStat().getValue(ATTACK_CANCEL,
/// init)`), which buffs like Concentration lower. The raid/HP-blocked/
/// channeling guards still don't apply to players yet.
pub fn calc_atk_break(dmg: f64, men_bonus: f64, roll: i32, cancel_add: f64, cancel_mul: f64) -> bool {
    let init = 15.0 + (13.0 * dmg).sqrt() - (men_bonus * 100.0 - 100.0);
    let rate = (init * cancel_mul + cancel_add).clamp(1.0, 99.0);
    (roll as f64) < rate
}

#[cfg(test)]
mod tests {
    use super::*;

    /// power 12 (Wind Strike 1), mAtk 100, mDef 60: 77·12·√100/60 = 154.0;
    /// magic crit doubles it.
    #[test]
    fn magic_dam_matches_java_formula() {
        let none = MagicFailure::None;
        let dmg = calc_magic_dam(100.0, 60.0, 12.0, false, 1.0, none);
        assert!((dmg - 154.0).abs() < 1e-9);
        let crit = calc_magic_dam(100.0, 60.0, 12.0, true, 1.0, none);
        assert!((crit - 308.0).abs() < 1e-9);
        // Spiritshot doubles, blessed spiritshot quadruples the base.
        assert!((calc_magic_dam(100.0, 60.0, 12.0, false, 2.0, none) - 308.0).abs() < 1e-9);
        assert!((calc_magic_dam(100.0, 60.0, 12.0, false, 4.0, none) - 616.0).abs() < 1e-9);
    }

    /// mDef is floored at 1 so a zero-defence target can't divide by zero.
    #[test]
    fn magic_dam_survives_zero_mdef() {
        assert!(calc_magic_dam(100.0, 0.0, 12.0, false, 1.0, MagicFailure::None).is_finite());
    }

    /// Java applies the `MagicFailures` adjustment to the *base* damage and only
    /// then multiplies by `critMod` — so a resisted crit lands on 2, not 1, and
    /// a halved crit keeps the full ×2.
    #[test]
    fn magic_failure_applies_before_the_crit_multiplier() {
        assert!((calc_magic_dam(100.0, 60.0, 12.0, false, 1.0, MagicFailure::Half) - 77.0).abs() < 1e-9);
        assert!((calc_magic_dam(100.0, 60.0, 12.0, true, 1.0, MagicFailure::Half) - 154.0).abs() < 1e-9);
        assert!((calc_magic_dam(100.0, 60.0, 12.0, false, 1.0, MagicFailure::Resisted) - 1.0).abs() < 1e-9);
        assert!((calc_magic_dam(100.0, 60.0, 12.0, true, 1.0, MagicFailure::Resisted) - 2.0).abs() < 1e-9);
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
        assert!((calc_effect_land_rate(35, 80, 30, 5, 1.0) - 90.0).abs() < 1e-9);
        // (35 - 80 + 3)·30 + 80 + 30 = -1150 → clamp to 10.
        assert!((calc_effect_land_rate(35, 80, 30, 80, 1.0) - 10.0).abs() < 1e-9);
        // activateRate -1 → guaranteed.
        assert!((calc_effect_land_rate(35, -1, 30, 5, 1.0) - 100.0).abs() < 1e-9);
        // magicLevel <= -1 falls back to targetLevel + 3, so the level term is
        // (23 - 20 + 3) = 6: 6·5 + 10 + 30 = 70.
        assert!((calc_effect_land_rate(-1, 10, 5, 20, 1.0) - 70.0).abs() < 1e-9);
    }

    /// Good skills cap the per-mille rate at 320, bad skills at 200; the
    /// comparison is strict (`rate > roll`).
    #[test]
    fn magic_crit_caps_and_thresholds() {
        assert!(calc_magic_crit(1000.0, false, 319));
        assert!(!calc_magic_crit(1000.0, false, 320));
        assert!(calc_magic_crit(1000.0, true, 199));
        assert!(!calc_magic_crit(1000.0, true, 200));
        assert!(!calc_magic_crit(0.0, false, 0));
    }

    /// Heal: power 83, mAtk 50 → 83 + √100 = 93; crit triples.
    #[test]
    fn heal_matches_java_formula() {
        assert!((calc_heal(83.0, 50.0, false, false, false, 0, false) - 93.0).abs() < 1e-9);
        assert!((calc_heal(83.0, 50.0, true, false, false, 0, false) - 279.0).abs() < 1e-9);
        // Spiritshot on a mage caster adds the MP-consume static bonus (sqrt
        // term unchanged at ×2): 83 + 40 + √100 = 133.
        assert!((calc_heal(83.0, 50.0, false, true, false, 40, true) - 133.0).abs() < 1e-9);
        // Blessed spiritshot: sqrt term ×4 (√200) and static ×2.4: 83 + 96 + √200.
        assert!((calc_heal(83.0, 50.0, false, false, true, 40, true) - (83.0 + 96.0 + 200.0_f64.sqrt())).abs() < 1e-9);
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
    #[test]
    fn auto_attack_crit_rate() {
        // stat 44 (displayed), front, level ground: 4.4 × 1.1 (height base
        // +10%) = 4.84 → roll 4 crits, roll 5 doesn't.
        assert!(calc_auto_attack_crit(44.0, Position::Front, 0, 0, 4));
        assert!(!calc_auto_attack_crit(44.0, Position::Front, 0, 0, 5));
        // Floor: even 0 stat crits below 3%.
        assert!(calc_auto_attack_crit(0.0, Position::Front, 0, 0, 2));
        // Cap: 97% — a 97 roll never crits.
        assert!(!calc_auto_attack_crit(10_000.0, Position::Back, 25, 0, 97));
    }

    /// `calcAutoAttackDamage`, melee/shotless: pAtk 100 vs pDef 50 → 154;
    /// crit doubles; back position adds 20% of pAtk before the ×77.
    #[test]
    fn auto_attack_damage_matches_java() {
        assert!((calc_auto_attack_damage(100.0, 1.0, Position::Front, 50.0, false, false) - 154.0).abs() < 1e-9);
        assert!((calc_auto_attack_damage(100.0, 1.0, Position::Front, 50.0, true, false) - 308.0).abs() < 1e-9);
        assert!((calc_auto_attack_damage(100.0, 1.0, Position::Back, 50.0, false, false) - 184.8).abs() < 1e-9);
        // A soulshot doubles the swing (×2 ssBonus): 154 → 308.
        assert!((calc_auto_attack_damage(100.0, 1.0, Position::Front, 50.0, false, true) - 308.0).abs() < 1e-9);
        // pDef floors at 1.
        assert!(calc_auto_attack_damage(100.0, 1.0, Position::Front, 0.0, false, false).is_finite());
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
        assert_eq!(calc_shield_use(30.0, 1.0, false, false, 20, 50), SHIELD_SUCCEED);
        assert_eq!(calc_shield_use(30.0, 1.0, false, false, 40, 50), SHIELD_NONE);
        // A back attack is never blocked even at a high rate.
        assert_eq!(calc_shield_use(90.0, 1.0, false, true, 0, 0), SHIELD_NONE);
        // Perfect block when `100 - 2·conBonus < perfectRoll` (conBonus 1.0 →
        // threshold 98): a perfect-roll of 99 upgrades, 98 does not.
        assert_eq!(calc_shield_use(90.0, 1.0, false, false, 0, 99), SHIELD_PERFECT);
        assert_eq!(calc_shield_use(90.0, 1.0, false, false, 0, 98), SHIELD_SUCCEED);
        // A bow attacker raises the rate 30% (rate 30 → 39, blocks a roll of 35).
        assert_eq!(calc_shield_use(30.0, 1.0, true, false, 35, 50), SHIELD_SUCCEED);
    }

    /// Physical skill damage, melee/shotless: pAtk 100, level 40 (levelMod
    /// 1.29), power 50, pDef 60 → 77·((100·1.29)+50)/60 = 229.833…; a crit
    /// doubles, a soulshot doubles, randomMod scales linearly.
    #[test]
    fn physical_skill_damage_matches_java() {
        let lm = level_mod(40);
        let base = calc_physical_skill_damage(100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.0, false, false);
        assert!((base - (77.0 * ((100.0 * 1.29) + 50.0) / 60.0)).abs() < 1e-9);
        assert!((calc_physical_skill_damage(100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.0, true, false) - base * 2.0).abs() < 1e-9);
        assert!((calc_physical_skill_damage(100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.0, false, true) - base * 2.0).abs() < 1e-9);
        assert!((calc_physical_skill_damage(100.0, 1.0, 60.0, 1.0, 50.0, lm, 1.1, false, false) - base * 1.1).abs() < 1e-9);
        // pAtkMod/pDefMod scale attack and defence; defence floors at 1.
        assert!(calc_physical_skill_damage(100.0, 1.0, 0.0, 0.0, 50.0, lm, 1.0, false, false).is_finite());
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
        let front = calc_blow_damage(100.0, 50.0, 60.0, Position::Front, 1.0, false);
        assert!((front - (77.0 * (150.0 * 0.666) / 60.0)).abs() < 1e-9);
        // Back: +0.2·150 inside the bracket.
        let back = calc_blow_damage(100.0, 50.0, 60.0, Position::Back, 1.0, false);
        assert!((back - (77.0 * ((150.0 * 0.666) + (0.2 * 150.0)) / 60.0)).abs() < 1e-9);
        // Soulshot doubles the front hit.
        assert!((calc_blow_damage(100.0, 50.0, 60.0, Position::Front, 1.0, true) - front * 2.0).abs() < 1e-9);
        // pDef floors at 1.
        assert!(calc_blow_damage(100.0, 50.0, 0.0, Position::Front, 1.0, false).is_finite());
    }

    /// Blow success: rate = posBonus · heightBonus · critRate · (100+boost)/100,
    /// capped at `limit`, vs Rnd(100). Equal-z height bonus is 1.1.
    #[test]
    fn blow_success_rate_cap_and_threshold() {
        // 1.0 · 1.1 · 10 · 1.0 = 11: roll 10 lands, 11 doesn't.
        assert!(calc_blow_success(10.0, Position::Front, 0, 0, 0.0, 100.0, 10));
        assert!(!calc_blow_success(10.0, Position::Front, 0, 0, 0.0, 100.0, 11));
        // chanceBoost 100 doubles the rate → 22.
        assert!(calc_blow_success(10.0, Position::Front, 0, 0, 100.0, 100.0, 21));
        // A huge crit rate is capped at `limit` (80): roll 79 lands, 80 doesn't.
        assert!(calc_blow_success(10_000.0, Position::Front, 0, 0, 0.0, 80.0, 79));
        assert!(!calc_blow_success(10_000.0, Position::Front, 0, 0, 0.0, 80.0, 80));
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
        assert!((map_range(-20.0, -15.0, -8.0, 10.0, 100.0) - 10.0).abs() < 1e-9, "clamped below");
        assert!((map_range(3.0, -15.0, -8.0, 10.0, 100.0) - 100.0).abs() < 1e-9, "clamped above");
    }

    /// Neutral MEN (bonus 1.0): rate = 15 + √(13·dmg), clamped to [1, 99].
    #[test]
    fn atk_break_rate_and_clamps() {
        // dmg 100 → 15 + √1300 ≈ 51.06: roll 51 breaks, 52 doesn't.
        assert!(calc_atk_break(100.0, 1.0, 51, 0.0, 1.0));
        assert!(!calc_atk_break(100.0, 1.0, 52, 0.0, 1.0));
        // Huge MEN bonus can't push the rate below 1%.
        assert!(calc_atk_break(0.0, 2.0, 0, 0.0, 1.0));
        assert!(!calc_atk_break(0.0, 2.0, 1, 0.0, 1.0));
        // Massive damage caps at 99% — roll 99 still survives.
        assert!(!calc_atk_break(1e9, 1.0, 99, 0.0, 1.0));
    }

    /// `Stat.ATTACK_CANCEL`: Concentration's -18 DIFF lowers the interrupt rate
    /// (≈51.06 → ≈33.06), so a roll of 40 now survives where it broke before.
    #[test]
    fn atk_break_honors_cancel_stat() {
        assert!(calc_atk_break(100.0, 1.0, 40, 0.0, 1.0), "no buff: 40 < 51.06 breaks");
        assert!(!calc_atk_break(100.0, 1.0, 40, -18.0, 1.0), "Concentration: 40 > 33.06 survives");
    }
}
