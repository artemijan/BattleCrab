//! **Formula parity** — the port's damage maths against a literal
//! transcription of Java's, swept over an input grid instead of spot-checked.
//!
//! Every other Java-comparison test in this tree pins one or two hand-computed
//! cases per formula. That finds a wrong constant; it does not find a *missing
//! term*, because a term that is absent from both the port and the expected
//! value agrees with itself. This file compares two independent expressions:
//!
//! * [`java`] holds transcriptions written **from the Java source**, quoted
//!   above each one, with every term Java multiplies in — including the ones
//!   the port had been dropping;
//! * the port side calls `model::formulas` exactly as the game does.
//!
//! Then it sweeps: levels, attack and defence, crit/shield/shot flags, ranged
//! and melee, front/side/back. A divergence anywhere in that grid fails with
//! the inputs that produced it.
//!
//! # What a failure here means
//!
//! Not "pick a new expected number". It means the two expressions disagree,
//! and the Java side is the specification — so read the transcription, find
//! which term differs, and fix the port. If the port is *deliberately* narrower
//! (a stat with no carrier on this dist, say), the narrowing belongs in the
//! sweep as a fixed input, documented, not as a tolerance.
//!
//! # The transcription has to come from the source
//!
//! The first draft of `java::attribute_bonus` here was written from memory as
//! a linear band, and the sweep failed against a port that was **right** — the
//! real curve is `1.025 + sqrt(diff³/2)·0.0001`. A transcription that is
//! recalled rather than copied turns this file into a second opinion of equal
//! confidence, which is worth nothing. Copy the expression, quote it, then
//! sweep.
//!
//! # Why the numbers are not asserted directly
//!
//! There is no golden file: goldens rot silently and encode whatever the port
//! did on the day they were written. The transcription is checked in instead,
//! and it can be re-read against Java's source by anyone.

use gameserver::model::formulas::{self, CritDamage, HealCaster};
use gameserver::model::movement::Position;

// `common` is shared with the census tests, which use more of it than the
// sweeps do; the sweeps want `DIST` alone.
#[allow(dead_code)]
mod common;

/// Transcriptions of Java's expressions. Each function quotes the source it
/// came from; nothing here calls the port.
mod java {
    use gameserver::data::item_data::CrystalType as Grade;
    use gameserver::model::formulas::HealCaster as Caster;
    use gameserver::model::movement::Position;

    /// `Formulas.calcAutoAttackDamage`:
    ///
    /// ```java
    /// double defence = target.getPDef();
    /// switch (shld) { case SHIELD_DEFENSE_SUCCEED: defence += target.getShldDef(); break;
    ///                 case SHIELD_DEFENSE_PERFECT_BLOCK: return 1; }
    /// final boolean isRanged = (weapon != null) && weapon.getItemType().isRanged();
    /// final double shotsBonus = attacker.getStat().getValue(Stat.SHOTS_BONUS);
    /// final double cAtk = crit ? calcCritDamage(attacker, target, null) : 1;
    /// final double cAtkAdd = crit ? calcCritDamageAdd(attacker, target, null) : 0;
    /// final double critMod = crit ? (isRanged ? 0.5 : 1) : 0;
    /// final double ssBonus = ss ? (ssBlessed ? 2.15 : 2) * shotsBonus : 1;
    /// final double randomDamage = attacker.getRandomDamageMultiplier();
    /// final double proxBonus = (attacker.isInFrontOf(target) ? 0
    ///     : (attacker.isBehind(target) ? 0.2 : 0.05)) * attacker.getPAtk();
    /// double attack = (attacker.getPAtk() * randomDamage) + proxBonus;
    /// attack = ((((attack * cAtk * ssBonus) + cAtkAdd) * critMod) * (isRanged ? 154 : 77))
    ///        + (attack * (1 - critMod) * ssBonus * (isRanged ? 154 : 77));
    /// double damage = attack / defence;
    /// damage *= calcAttackTraitBonus(attacker, target);
    /// damage *= calcAttributeBonus(attacker, target, null);
    /// damage *= calculatePvpPveBonus(attacker, target, null, crit);
    /// damage *= attacker.getStat().getMul(Stat.AUTO_ATTACK_DAMAGE_BONUS);
    /// return Math.max(0, damage);
    /// ```
    ///
    /// `AUTO_ATTACK_DAMAGE_BONUS` is left out of the transcription on purpose:
    /// the only skill declaring `AutoAttackDamageBonus` on this dist is in the
    /// 30500 range, so no character here can carry it and the term is a fixed
    /// 1.0 on both sides.
    ///
    /// `SHOTS_BONUS` is **not** in that category, though it was recorded there
    /// on the first pass. `ShotsBonusFinalizer` reads it off the equipped
    /// weapon's **enchant level** (`1 + enchant·0.003`), which every geared
    /// character carries — so it is swept as a real input.
    #[allow(clippy::too_many_arguments)]
    pub fn auto_attack_damage(
        p_atk: f64,
        random_damage: f64,
        prox: f64,
        defence: f64,
        crit: bool,
        c_atk: f64,
        c_atk_add: f64,
        ss: bool,
        shots_bonus: f64,
        is_ranged: bool,
        trait_bonus: f64,
        attribute_bonus: f64,
        pvp_pve_bonus: f64,
    ) -> f64 {
        let c_atk = if crit { c_atk } else { 1.0 };
        let c_atk_add = if crit { c_atk_add } else { 0.0 };
        let crit_mod = if crit {
            if is_ranged { 0.5 } else { 1.0 }
        } else {
            0.0
        };
        // Blessed soulshots do not exist on Interlude, so `ssBlessed` is false.
        let ss_bonus = if ss { 2.0 * shots_bonus } else { 1.0 };
        let prox_bonus = prox * p_atk;
        let weapon_mod = if is_ranged { 154.0 } else { 77.0 };
        let attack = (p_atk * random_damage) + prox_bonus;
        let attack = ((((attack * c_atk * ss_bonus) + c_atk_add) * crit_mod) * weapon_mod)
            + (attack * (1.0 - crit_mod) * ss_bonus * weapon_mod);
        let mut damage = attack / defence;
        damage *= trait_bonus;
        damage *= attribute_bonus;
        damage *= pvp_pve_bonus;
        damage.max(0.0)
    }

    /// `handlers/effecthandlers/PhysicalAttack.instant`, the damage half:
    ///
    /// ```java
    /// final double attack = effector.getPAtk() * _pAtkMod;
    /// double defence = effected.getPDef() * _pDefMod;   // + shield, or -1 on a perfect block
    /// final double power = ((_power * (hasAbnormalType ? _abnormalPowerMod : 1))
    ///                       + effector.getStat().getValue(Stat.SKILL_POWER_ADD, 0));
    /// final double weaponMod = effector.getAttackType().isRanged() ? 70 : 77;
    /// final double rangedBonus = effector.getAttackType().isRanged() ? attack + power : 0;
    /// final double critMod = critical ? Formulas.calcCritDamage(effector, effected, skill) : 1;
    /// double ssmod = 1;
    /// if (skill.useSoulShot()) { if (charged) ssmod = 2 * SHOTS_BONUS; else if (blessed) ssmod = 4 * SHOTS_BONUS; }
    /// final double baseMod = (weaponMod * ((attack * effector.getLevelMod()) + power + rangedBonus)) / defence;
    /// damage = baseMod * (hasAbnormalType ? _abnormalDamageMod : 1) * ssmod * critMod
    ///        * weaponTraitMod * (generalTraitMod == 0 ? 1 : generalTraitMod) * weaknessMod
    ///        * attributeMod * pvpPveMod * randomMod;
    /// damage *= effector.getStat().getValue(Stat.PHYSICAL_SKILL_POWER, 1);
    /// ```
    ///
    /// `mods` stands for the block of multipliers the port's **caller**
    /// applies (traits, weakness, attribute, pvp/pve, `PHYSICAL_SKILL_POWER`,
    /// race and the abnormal pair): they are a product either way, so the
    /// sweep varies them as one factor and the leaf's own arithmetic is what
    /// is under test. `SKILL_POWER_ADD` has no carrier on this dist.
    #[allow(clippy::too_many_arguments)]
    pub fn physical_skill_damage(
        p_atk: f64,
        p_atk_mod: f64,
        p_def: f64,
        p_def_mod: f64,
        power: f64,
        level_mod: f64,
        random_mod: f64,
        crit: bool,
        crit_mul: f64,
        ss: bool,
        shots_bonus: f64,
        is_ranged: bool,
        mods: f64,
    ) -> f64 {
        let attack = p_atk * p_atk_mod;
        let defence = p_def * p_def_mod;
        let weapon_mod = if is_ranged { 70.0 } else { 77.0 };
        let ranged_bonus = if is_ranged { attack + power } else { 0.0 };
        let crit_mod = if crit { crit_mul } else { 1.0 };
        let ss_mod = if ss { 2.0 * shots_bonus } else { 1.0 };
        let base_mod = (weapon_mod * ((attack * level_mod) + power + ranged_bonus)) / defence;
        base_mod * ss_mod * crit_mod * random_mod * mods
    }

    /// `Formulas.calcMagicDam`, the arithmetic without the packets:
    ///
    /// ```java
    /// final double shotsBonus = bss ? (4 * SHOTS_BONUS) : sps ? (2 * SHOTS_BONUS) : 1;
    /// final double critMod = mcrit ? calcCritDamage(attacker, target, skill) : 1;
    /// double damage = ((77 * (power + SKILL_POWER_ADD) * Math.sqrt(mAtk)) / mDef) * shotsBonus;
    /// // …failure: damage /= 2 (half) or damage = 1 (resisted)…
    /// damage = damage * critMod * (generalTraitMod == 0 ? 1 : generalTraitMod) * weaknessMod
    ///        * attributeMod * randomMod * pvpPveMod;
    /// damage *= attacker.getStat().getValue(Stat.MAGICAL_SKILL_POWER, 1);
    /// ```
    ///
    /// `mods` is again the caller's product. `randomMod` is **not** in it: it
    /// belongs to the leaf, and leaving it out is what made every nuke land on
    /// the same number before this sweep was written.
    #[allow(clippy::too_many_arguments)]
    pub fn magic_damage(
        m_atk: f64,
        m_def: f64,
        power: f64,
        mcrit: bool,
        crit_mul: f64,
        shots_bonus: f64,
        failure: u8,
        random_mod: f64,
        mods: f64,
    ) -> f64 {
        let mut damage = ((77.0 * power * m_atk.sqrt()) / m_def) * shots_bonus;
        match failure {
            1 => damage /= 2.0,
            2 => damage = 1.0,
            _ => {}
        }
        let crit_mod = if mcrit { crit_mul } else { 1.0 };
        damage * crit_mod * random_mod * mods
    }

    /// `Formulas.calcBlowDamage`:
    ///
    /// ```java
    /// final double cdMult = criticalMod * (((criticalPositionMod - 1) / 2) + 1) * (((criticalVulnMod - 1) / 2) + 1);
    /// final double cdPatk = (criticalAddMod + criticalAddVuln) * criticalSkillMod;   // criticalSkillMod = calcCritDamage(...)/2
    /// final double isPosition = position == BACK ? 0.2 : position == SIDE ? 0.05 : 0;
    /// final double ssmod = ss ? (2 * SHOTS_BONUS) : 1;
    /// final double baseMod = (77 * (((skillPower + pAtk) * 0.666)
    ///                        + (isPosition * (skillPower + pAtk) * randomMod)
    ///                        + (6 * cdPatk))) / defence;
    /// final double damage = baseMod * ssmod * cdMult * weaponTraitMod * generalTraitMod
    ///                     * weaknessMod * attributeMod * randomMod * pvpPveMod * balanceMod;
    /// ```
    ///
    /// Note this formula uses `generalTraitMod` **raw** — the
    /// `== 0 ? 1 : …` guard the physical and magic ones carry is absent here.
    #[allow(clippy::too_many_arguments)]
    pub fn blow_damage(
        p_atk: f64,
        power: f64,
        p_def: f64,
        is_position: f64,
        random_mod: f64,
        ss: bool,
        shots_bonus: f64,
        cd_mult: f64,
        cd_patk: f64,
        mods: f64,
    ) -> f64 {
        let ssmod = if ss { 2.0 * shots_bonus } else { 1.0 };
        let sum = power + p_atk;
        let base_mod =
            (77.0 * ((sum * 0.666) + (is_position * sum * random_mod) + (6.0 * cd_patk))) / p_def;
        base_mod * ssmod * cd_mult * random_mod * mods
    }

    /// `Formulas.calcManaDam`:
    ///
    /// ```java
    /// mAtk *= bss ? 4 * (shotsBonus + sapphire) : sps ? 2 * (shotsBonus + sapphire) : 1;
    /// double damage = (Math.sqrt(mAtk) * power * (mp / 97)) / mDef;
    /// damage *= calcGeneralTraitBonus(attacker, target, skill.getTraitType(), false);
    /// damage *= calculatePvpPveBonus(attacker, target, skill, mcrit);
    /// // …failure: damage /= 2…
    /// if (mcrit) { damage *= 3; damage = Math.min(damage, critLimit); }
    /// ```
    ///
    /// The **order** is the point: both multipliers land before the crit, and
    /// the crit ends in a `min` against the skill's `criticalLimit`. Applying
    /// them afterwards lets a capped crit exceed its cap, which is what the
    /// port was doing. Sapphire jewels are Kamael-era and absent here.
    #[allow(clippy::too_many_arguments)]
    pub fn mana_damage(
        m_atk: f64,
        m_def: f64,
        target_max_mp: f64,
        power: f64,
        shots_bonus: f64,
        failure: u8,
        mcrit: bool,
        crit_limit: f64,
        trait_bonus: f64,
        pvp_pve_bonus: f64,
    ) -> f64 {
        let m_atk = m_atk * shots_bonus;
        let mut damage = (m_atk.sqrt() * power * (target_max_mp / 97.0)) / m_def;
        damage *= trait_bonus;
        damage *= pvp_pve_bonus;
        if failure != 0 {
            damage /= 2.0;
        }
        if mcrit {
            damage *= 3.0;
            damage = damage.min(crit_limit);
        }
        damage
    }

    /// `Formulas.calcAtkSpd` and `calculateTimeBetweenAttacks`:
    ///
    /// ```java
    /// public static int calcAtkSpd(Creature attacker, Skill skill, double skillTime) {
    ///     if (skill.isMagic()) return (int) ((skillTime / attacker.getMAtkSpd()) * 333);
    ///     return (int) ((skillTime / attacker.getPAtkSpd()) * 300);
    /// }
    /// public static int calculateTimeBetweenAttacks(int attackSpeed) {
    ///     return Math.max(50, (500000 / attackSpeed));
    /// }
    /// ```
    pub fn atk_spd(skill_time: f64, atk_spd: i32, magic: bool) -> i32 {
        if magic {
            ((skill_time / atk_spd as f64) * 333.0) as i32
        } else {
            ((skill_time / atk_spd as f64) * 300.0) as i32
        }
    }

    /// See [`atk_spd`].
    pub fn time_between_attacks(attack_speed: i32) -> i32 {
        (500_000 / attack_speed).max(50)
    }

    /// `Formulas.calcSkillTimeFactor`'s magic branch, and the early return it
    /// shares with channeling:
    ///
    /// ```java
    /// if (skill.getOperateType().isChanneling() || (magicType == 2) || (magicType == 4) || (magicType == 21)) return 1.0d;
    /// double factor = 0.0;
    /// if (skill.getMagicType() == 1) {
    ///     final double spiritshotHitTime = (isChargedShot(SPIRITSHOTS) || isChargedShot(BLESSED_SPIRITSHOTS)) ? 0.4 : 0;
    ///     factor = getMAttackSpeedMultiplier() + (getMAttackSpeedMultiplier() * spiritshotHitTime);
    /// } else { factor = getAttackSpeedMultiplier(); }
    /// // …npc hitTimeFactorSkill divisor…
    /// return Math.max(0.01, factor);
    /// ```
    pub fn skill_time_factor(
        multiplier: f64,
        magic: bool,
        channeling: bool,
        static_magic_type: bool,
        spiritshot_charged: bool,
    ) -> f64 {
        if channeling || static_magic_type {
            return 1.0;
        }
        let factor = if magic && spiritshot_charged {
            multiplier + (multiplier * 0.4)
        } else {
            multiplier
        };
        factor.max(0.01)
    }

    /// `handlers/effecthandlers/Heal.instant`, the amount half:
    ///
    /// ```java
    /// double amount = _power;
    /// double staticShotBonus = 0; double mAtkMul = 1;
    /// if (((sps || bss) && (effector.isPlayer() && isMageClass())) || effector.isSummon()) {
    ///     staticShotBonus = skill.getMpConsume();
    ///     mAtkMul = bss ? 4 * shotsBonus : 2 * shotsBonus;
    ///     staticShotBonus *= bss ? 2.4 : 1.0;
    /// } else if ((sps || bss) && effector.isNpc()) {
    ///     staticShotBonus = 2.4 * skill.getMpConsume(); mAtkMul = 4 * shotsBonus;
    /// } else {
    ///     if (weaponInst != null) mAtkMul = S84 ? 4 : S80 ? 2 : 1;   // both post-Interlude
    ///     mAtkMul = bss ? mAtkMul * 4 : mAtkMul + 1;
    /// }
    /// if (!skill.isStatic()) {
    ///     amount += staticShotBonus + Math.sqrt(mAtkMul * effector.getMAtk());
    ///     amount *= HEAL_EFFECT; amount += HEAL_EFFECT_ADD;
    ///     if (magic && crit) amount *= 3;
    /// }
    /// ```
    ///
    /// The `else` branch's `mAtkMul + 1` is why an unshot heal multiplies mAtk
    /// by **2**, not 1 — the same number the shot branch reaches by a different
    /// road. That coincidence held only while `SHOTS_BONUS` was a hard 1: the
    /// shot branch scales by it and the grade branch does not, so the three arms
    /// are transcribed separately here. `HEAL_EFFECT`/`_ADD` are the caller's,
    /// and no Interlude weapon reaches the S80/S84 grades.
    #[allow(clippy::too_many_arguments)]
    pub fn heal_amount(
        power: f64,
        m_atk: f64,
        mcrit: bool,
        sps: bool,
        bss: bool,
        mp_consume: i32,
        caster: Caster,
        shots_bonus: f64,
    ) -> f64 {
        let (static_shot_bonus, m_atk_mul) =
            if ((sps || bss) && caster == Caster::PlayerMage) || caster == Caster::Summon {
                (
                    mp_consume as f64 * if bss { 2.4 } else { 1.0 },
                    if bss { 4.0 } else { 2.0 } * shots_bonus,
                )
            } else if (sps || bss) && caster == Caster::Npc {
                (2.4 * mp_consume as f64, 4.0 * shots_bonus)
            } else {
                // Weapon grade is 1 on this chronicle, so `mAtkMul + 1` = 2 and
                // `mAtkMul * 4` = 4 — and this branch takes **no** `shotsBonus`.
                (0.0, if bss { 4.0 } else { 2.0 })
            };
        let amount = power + static_shot_bonus + (m_atk_mul * m_atk).sqrt();
        amount * if mcrit { 3.0 } else { 1.0 }
    }

    /// `Formulas.calcAttributeBonus`, after the element election:
    ///
    /// ```java
    /// final int diff = attackAttribute - defenceAttribute;
    /// if (diff > 0)  return Math.min(1.025 + (Math.sqrt(Math.pow(diff, 3) / 2) * 0.0001), 1.25);
    /// if (diff < 0)  return Math.max(0.975 - (Math.sqrt(Math.pow(-diff, 3) / 2) * 0.0001), 0.75);
    /// return 1;
    /// ```
    ///
    /// The election itself (which element, and whether a skill names one) is
    /// the port's `attribute_mod`; what is swept here is the curve, which is
    /// where an off-by-a-constant would hide.
    pub fn attribute_bonus(attack: f64, defence: f64) -> f64 {
        let diff = attack - defence;
        if diff > 0.0 {
            (1.025 + ((diff.powi(3) / 2.0).sqrt() * 0.0001)).min(1.25)
        } else if diff < 0.0 {
            (0.975 - (((-diff).powi(3) / 2.0).sqrt() * 0.0001)).max(0.75)
        } else {
            1.0
        }
    }

    /// `Formulas.calcShldUse` — the arithmetic half, with the paperdoll and
    /// angle checks (which are the *caller's* inputs here) left out.
    ///
    /// ```java
    /// double shldRate = target.getStat().getValue(Stat.SHIELD_DEFENCE_RATE) * BaseStat.CON.calcBonus(target);
    /// if (attacker.getAttackType().isRanged()) shldRate *= 1.3;
    /// byte shldSuccess = SHIELD_DEFENSE_FAILED;
    /// if (shldRate > Rnd.get(100))
    /// {
    ///     if (((100 - (2 * BaseStat.CON.calcBonus(target))) < Rnd.get(100)))
    ///         shldSuccess = SHIELD_DEFENSE_PERFECT_BLOCK;
    ///     else
    ///         shldSuccess = SHIELD_DEFENSE_SUCCEED;
    /// }
    /// ```
    ///
    /// `shldRate` arrives already multiplied by the CON bonus (the port folds
    /// that into `shield_stats`, matching `getValue(SHIELD_DEFENCE_RATE) * CON`),
    /// so `con_bonus` is here only for the perfect-block line — which is exactly
    /// how Java reads it a second time.
    pub fn shield_use(
        shield_rate: f64,
        con_bonus: f64,
        ranged: bool,
        rate_roll: i32,
        perfect_roll: i32,
    ) -> u8 {
        let mut rate = shield_rate;
        if ranged {
            rate *= 1.3;
        }
        if rate > rate_roll as f64 {
            if (100.0 - (2.0 * con_bonus)) < perfect_roll as f64 {
                2 // SHIELD_DEFENSE_PERFECT_BLOCK
            } else {
                1 // SHIELD_DEFENSE_SUCCEED
            }
        } else {
            0 // SHIELD_DEFENSE_FAILED
        }
    }

    /// `Formulas.calcEffectSuccess` — everything from the `activateRate` check
    /// down, which is the whole of the arithmetic.
    ///
    /// ```java
    /// final int activateRate = skill.getActivateRate();
    /// if ((activateRate == -1)) return true;
    /// int magicLevel = skill.getMagicLevel();
    /// if (magicLevel <= -1) magicLevel = target.getLevel() + 3;
    /// final double targetBasicProperty = getAbnormalResist(skill.getBasicProperty(), target);
    /// final double baseMod = ((((((magicLevel - target.getLevel()) + 3) * skill.getLvlBonusRate()) + activateRate) + 30.0) - targetBasicProperty);
    /// final double elementMod = calcAttributeBonus(attacker, target, skill);
    /// final double traitMod = calcGeneralTraitBonus(attacker, target, skill.getTraitType(), false);
    /// final double basicPropertyResist = getBasicPropertyResistBonus(skill.getBasicProperty(), target);
    /// final double buffDebuffMod = skill.isDebuff() ? target.getStat().getValue(Stat.RESIST_ABNORMAL_DEBUFF, 1) : 1;
    /// final double rate = baseMod * elementMod * traitMod * buffDebuffMod;
    /// final double finalRate = traitMod > 0 ? CommonUtil.constrain(rate, skill.getMinChance(), skill.getMaxChance()) * basicPropertyResist : 0;
    /// ```
    ///
    /// `activateRate == -1` is written as 100 rather than a bare `true` so the
    /// two sides return the same type; the port does the same.
    #[allow(clippy::too_many_arguments)]
    pub fn effect_land_rate(
        magic_level: i32,
        activate_rate: i32,
        lvl_bonus_rate: i32,
        target_level: i32,
        buff_debuff_mod: f64,
        element_mod: f64,
        trait_mod: f64,
        target_basic_property: f64,
        basic_property_resist: f64,
        min_chance: f64,
        max_chance: f64,
    ) -> f64 {
        if activate_rate == -1 {
            return 100.0;
        }
        let mut magic_level = magic_level;
        if magic_level <= -1 {
            magic_level = target_level + 3;
        }
        let base_mod =
            ((((magic_level - target_level) + 3) * lvl_bonus_rate) + activate_rate) as f64 + 30.0
                - target_basic_property;
        let rate = base_mod * element_mod * trait_mod * buff_debuff_mod;
        if trait_mod > 0.0 {
            constrain(rate, min_chance, max_chance) * basic_property_resist
        } else {
            0.0
        }
    }

    /// `CommonUtil.constrain(double, double, double)`.
    fn constrain(value: f64, min: f64, max: f64) -> f64 {
        if value < min {
            min
        } else if value > max {
            max
        } else {
            value
        }
    }

    /// `Formulas.calcMagicSuccess` — the rate, without the final `Rnd.get(100)`.
    ///
    /// ```java
    /// double lvlModifier = 1;
    /// float targetModifier = 1;
    /// int mAccModifier = 1;
    /// if (attacker.isAttackable() || target.isAttackable())
    /// {
    ///     lvlModifier = Math.pow(1.3, target.getLevel() - (…skill.getMagicLevel() : attacker.getLevel()));
    ///     if ((attacker.getActingPlayer() != null) && !target.isRaid() && !target.isRaidMinion() && (target.getLevel() >= Config.MIN_NPC_LEVEL_MAGIC_PENALTY) && ((target.getLevel() - attacker.getActingPlayer().getLevel()) >= 3))
    ///     {
    ///         final int levelDiff = target.getLevel() - attacker.getActingPlayer().getLevel() - 2;
    ///         if (levelDiff >= Config.NPC_SKILL_CHANCE_PENALTY.length)
    ///             targetModifier = Config.NPC_SKILL_CHANCE_PENALTY[Config.NPC_SKILL_CHANCE_PENALTY.length - 1];
    ///         else
    ///             targetModifier = Config.NPC_SKILL_CHANCE_PENALTY[levelDiff];
    ///     }
    /// }
    /// else
    /// {
    ///     final int mAccDiff = attacker.getMagicAccuracy() - target.getMagicEvasionRate();
    ///     mAccModifier = 100;
    ///     if (mAccDiff > -20) mAccModifier = 2;
    ///     else if (mAccDiff > -25) mAccModifier = 30;
    ///     else if (mAccDiff > -30) mAccModifier = 60;
    ///     else if (mAccDiff > -35) mAccModifier = 90;
    /// }
    /// final double resModifier = target.getStat().getMul(Stat.MAGIC_SUCCESS_RES, 1);
    /// final int rate = 100 - Math.round((float) (mAccModifier * lvlModifier * targetModifier * resModifier));
    /// ```
    ///
    /// Two details the shape depends on: `targetModifier` is a **float**, and
    /// the whole product is narrowed to `float` before `Math.round` — which is
    /// what `java_round_float` reproduces on the port's side.
    #[allow(clippy::too_many_arguments)]
    pub fn magic_success_rate(
        pve: bool,
        target_level: i32,
        effective_level: i32,
        caster_player_level: Option<i32>,
        target_is_raid: bool,
        min_npc_level_for_magic_penalty: i32,
        skill_chance_penalty: &[f64],
        magic_accuracy: i32,
        magic_evasion: i32,
        res_modifier: f64,
    ) -> i32 {
        let mut lvl_modifier = 1.0f64;
        let mut target_modifier = 1.0f32;
        let mut m_acc_modifier = 1i32;
        if pve {
            lvl_modifier = 1.3f64.powi(target_level - effective_level);
            if let Some(player_level) = caster_player_level
                && !target_is_raid
                && target_level >= min_npc_level_for_magic_penalty
                && (target_level - player_level) >= 3
            {
                let level_diff = (target_level - player_level - 2) as usize;
                target_modifier = if level_diff >= skill_chance_penalty.len() {
                    skill_chance_penalty[skill_chance_penalty.len() - 1] as f32
                } else {
                    skill_chance_penalty[level_diff] as f32
                };
            }
        } else {
            let m_acc_diff = magic_accuracy - magic_evasion;
            m_acc_modifier = 100;
            if m_acc_diff > -20 {
                m_acc_modifier = 2;
            } else if m_acc_diff > -25 {
                m_acc_modifier = 30;
            } else if m_acc_diff > -30 {
                m_acc_modifier = 60;
            } else if m_acc_diff > -35 {
                m_acc_modifier = 90;
            }
        }
        let product = m_acc_modifier as f64 * lvl_modifier * target_modifier as f64 * res_modifier;
        100 - java_round_float(product)
    }

    /// `Math.round(float)` — `floor(x + 0.5)` on the narrowed value.
    fn java_round_float(v: f64) -> i32 {
        ((v as f32) + 0.5f32).floor() as i32
    }

    /// `Formulas.calculateSkillResurrectRestorePercent`:
    ///
    /// ```java
    /// if ((baseRestorePercent == 0) || (baseRestorePercent == 100)) return baseRestorePercent;
    /// double restorePercent = baseRestorePercent * BaseStat.WIT.calcBonus(caster);
    /// if ((restorePercent - baseRestorePercent) > 20.0) restorePercent += 20.0;
    /// restorePercent = Math.max(restorePercent, baseRestorePercent);
    /// restorePercent = Math.min(restorePercent, 90.0);
    /// ```
    pub fn resurrect_restore_percent(base: f64, wit_bonus: f64) -> f64 {
        if base == 0.0 || base == 100.0 {
            return base;
        }
        let mut restore = base * wit_bonus;
        if (restore - base) > 20.0 {
            restore += 20.0;
        }
        restore = restore.max(base);
        restore.min(90.0)
    }

    /// `IStatFunction.calcEnchantDefBonus`:
    ///
    /// ```java
    /// switch (item.getTemplate().getCrystalTypePlus())
    /// {
    ///     case R: return ((2 * blessedBonus * enchant) + (6 * blessedBonus * Math.max(0, enchant - 3)));
    ///     default: return enchant + (3 * Math.max(0, enchant - 3));
    /// }
    /// ```
    pub fn enchant_def_bonus(crystal_plus: Grade, enchant: i32) -> f64 {
        match crystal_plus {
            Grade::R => 2.0 * enchant as f64 + 6.0 * i32::max(0, enchant - 3) as f64,
            _ => enchant as f64 + 3.0 * i32::max(0, enchant - 3) as f64,
        }
    }

    /// `IStatFunction.calcEnchantMatkBonus`:
    ///
    /// ```java
    /// case R: return ((5 * blessedBonus * enchant) + (10 * blessedBonus * Math.max(0, enchant - 3)));
    /// case S: return (4 * enchant) + (8 * Math.max(0, enchant - 3));
    /// case A: case B: case C: return (3 * enchant) + (6 * Math.max(0, enchant - 3));
    /// default: return (2 * enchant) + (4 * Math.max(0, enchant - 3));
    /// ```
    pub fn enchant_m_atk_bonus(crystal_plus: Grade, enchant: i32) -> f64 {
        let e = enchant as f64;
        let o = i32::max(0, enchant - 3) as f64;
        match crystal_plus {
            Grade::R => 5.0 * e + 10.0 * o,
            Grade::S => 4.0 * e + 8.0 * o,
            Grade::A | Grade::B | Grade::C => 3.0 * e + 6.0 * o,
            _ => 2.0 * e + 4.0 * o,
        }
    }

    /// `IStatFunction.calcEnchantedPAtkBonus`, all four reachable grade arms and
    /// the gradeless default. `twoHand` is Java's `(bodyPart == SLOT_LR_HAND)
    /// && (itemType != WeaponType.POLE)` — a polearm occupies the two-handed
    /// slot but is paid as a one-hander.
    pub fn enchant_p_atk_bonus(
        crystal_plus: Grade,
        two_hand: bool,
        ranged: bool,
        enchant: i32,
    ) -> f64 {
        let e = enchant as f64;
        let o = i32::max(0, enchant - 3) as f64;
        match crystal_plus {
            Grade::R => {
                if two_hand {
                    if ranged {
                        12.0 * e + 24.0 * o
                    } else {
                        7.0 * e + 14.0 * o
                    }
                } else {
                    6.0 * e + 12.0 * o
                }
            }
            Grade::S => {
                if two_hand {
                    if ranged {
                        10.0 * e + 20.0 * o
                    } else {
                        6.0 * e + 12.0 * o
                    }
                } else {
                    5.0 * e + 10.0 * o
                }
            }
            Grade::A => {
                if two_hand {
                    if ranged {
                        8.0 * e + 16.0 * o
                    } else {
                        5.0 * e + 10.0 * o
                    }
                } else {
                    4.0 * e + 8.0 * o
                }
            }
            Grade::B | Grade::C => {
                if two_hand {
                    if ranged {
                        6.0 * e + 12.0 * o
                    } else {
                        4.0 * e + 8.0 * o
                    }
                } else {
                    3.0 * e + 6.0 * o
                }
            }
            _ => {
                if ranged {
                    4.0 * e + 8.0 * o
                } else {
                    2.0 * e + 4.0 * o
                }
            }
        }
    }

    /// `ShotsBonusFinalizer`:
    ///
    /// ```java
    /// double baseValue = 1;
    /// if ((weapon != null) && weapon.isEnchanted()) baseValue += (weapon.getEnchantLevel() * 0.3) / 100;
    /// ```
    ///
    /// `isEnchanted()` is `getEnchantLevel() > 0`, so the guard and the term
    /// agree at 0 — but only because `0 * 0.3 / 100` is 0. It is transcribed
    /// with the guard in place anyway, since that is what Java runs.
    pub fn shots_bonus(enchant: i32) -> f64 {
        let mut base = 1.0;
        if enchant > 0 {
            base += (enchant as f64 * 0.3) / 100.0;
        }
        base
    }

    /// `Formulas.calcEffectAbnormalTime`:
    ///
    /// ```java
    /// int time = (skill == null) || skill.isPassive() || skill.isToggle() ? -1 : skill.getAbnormalTime();
    /// if ((skill != null) && !skill.isStatic() && calcSkillMastery(caster, skill)) time *= 2;
    /// ```
    pub fn effect_abnormal_time(abnormal_time: i32, is_static: bool, mastery_procs: bool) -> i32 {
        let mut time = abnormal_time;
        if !is_static && mastery_procs {
            time *= 2;
        }
        time
    }

    // ---------------------------------------------------------------------
    // The roll family: the formulas that decide *whether* something happens.
    // Everything above answers "how much"; a wrong term there moves a number,
    // a wrong term here moves a rate — and a rate is what nobody can see.
    // ---------------------------------------------------------------------

    /// `Formulas.calcHitMiss` — returns **true for a miss**:
    ///
    /// ```java
    /// int chance = (80 + (2 * (attacker.getAccuracy() - target.getEvasionRate()))) * 10;
    /// chance *= HitConditionBonusData.getInstance().getConditionBonus(attacker, target);
    /// chance = Math.max(chance, 200);
    /// chance = Math.min(chance, 980);
    /// return chance < Rnd.get(1000);
    /// ```
    ///
    /// `chance` is an `int` and `chance *= double` is a **narrowing** compound
    /// assignment, so Java truncates toward zero before the clamp. That is
    /// transcribed literally here rather than smoothed into `f64`; the port
    /// stays in `f64`, and the sweep is what says the two agree (they do — for
    /// an integer `roll`, `trunc(x) < r` and `x < r` cannot disagree).
    pub fn hit_miss(accuracy: i32, evasion: i32, condition_bonus: f64, roll: i32) -> bool {
        let mut chance = (80 + (2 * (accuracy - evasion))) * 10;
        chance = (f64::from(chance) * condition_bonus) as i32;
        chance = chance.max(200);
        chance = chance.min(980);
        chance < roll
    }

    /// `HitConditionBonusData.getConditionBonus` — the multiplier the line
    /// above narrows through:
    ///
    /// ```java
    /// double mod = 100;
    /// if ((attacker.getZ() - target.getZ()) > 50) mod += highBonus;
    /// else if ((attacker.getZ() - target.getZ()) < -50) mod += lowBonus;
    /// if (GameTimeTaskManager.getInstance().isNight()) mod += darkBonus;
    /// switch (Position.getPosition(attacker, target))
    /// {
    ///     case SIDE: mod += sideBonus; break;
    ///     case BACK: mod += backBonus; break;
    ///     default: mod += frontBonus; break;
    /// }
    /// return Math.max(mod / 100, 0);
    /// ```
    ///
    /// The rain arm is commented out in Java too. `darkBonus` is **not**: it is
    /// −10 on this dist and applies for the whole in-game night, which the port
    /// dropped until this axis was opened.
    pub fn hit_condition_bonus(
        bonuses: &gameserver::data::HitConditionBonusData,
        attacker_z: i32,
        target_z: i32,
        night: bool,
        position: Position,
    ) -> f64 {
        let mut modifier = 100.0;
        if attacker_z - target_z > 50 {
            modifier += bonuses.high_bonus;
        } else if attacker_z - target_z < -50 {
            modifier += bonuses.low_bonus;
        }
        if night {
            modifier += bonuses.dark_bonus;
        }
        modifier += match position {
            Position::Side => bonuses.side_bonus,
            Position::Back => bonuses.back_bonus,
            Position::Front => bonuses.front_bonus,
        };
        (modifier / 100.0).max(0.0)
    }

    /// `Formulas.calcCriticalHeightBonus`:
    ///
    /// ```java
    /// return ((((CommonUtil.constrain(from.getZ() - target.getZ(), -25, 25) * 4) / 5) + 10) / 100) + 1;
    /// ```
    ///
    /// `getZ()` is an `int`, `constrain(int, int, int)` returns an `int`, and
    /// the literals are `int` — so the whole expression is integer arithmetic,
    /// the `/ 100` truncates a numerator that never leaves −10..30, and the
    /// method is a flat `1` for every z difference in the game. Written out
    /// term by term instead of as `1.0`, because the point of the
    /// transcription is that it can be re-read against Java.
    pub fn critical_height_bonus(from_z: i32, to_z: i32) -> f64 {
        f64::from((((((from_z - to_z).clamp(-25, 25) * 4) / 5) + 10) / 100) + 1)
    }

    /// `Formulas.calcCriticalPositionBonus`:
    ///
    /// ```java
    /// case SIDE: return 1.1 * creature.getStat().getPositionTypeValue(Stat.CRITICAL_RATE, Position.SIDE);
    /// case BACK: return 1.3 * creature.getStat().getPositionTypeValue(Stat.CRITICAL_RATE, Position.BACK);
    /// default: return creature.getStat().getPositionTypeValue(Stat.CRITICAL_RATE, Position.FRONT);
    /// ```
    pub fn critical_position_bonus(position: Position, position_mul: f64) -> f64 {
        match position {
            Position::Side => 1.1 * position_mul,
            Position::Back => 1.3 * position_mul,
            Position::Front => position_mul,
        }
    }

    /// `Formulas.calcCrit`'s auto-attack arm (the `skill == null` tail):
    ///
    /// ```java
    /// final double criticalRateMod = (target.getStat().getValue(Stat.DEFENCE_CRITICAL_RATE, rate) + target.getStat().getValue(Stat.DEFENCE_CRITICAL_RATE_ADD, 0)) / 10;
    /// final double criticalLocBonus = calcCriticalPositionBonus(creature, target);
    /// final double criticalHeightBonus = calcCriticalHeightBonus(creature, target);
    /// rate = criticalLocBonus * criticalRateMod * criticalHeightBonus;
    /// if ((creature.getLevel() >= 78) || (target.getLevel() >= 78))
    /// {
    ///     rate += (Math.sqrt(creature.getLevel()) * (creature.getLevel() - target.getLevel()) * 0.125);
    /// }
    /// rate = CommonUtil.constrain(rate, 3, 97);
    /// return (rate * balanceMod) > Rnd.get(100);
    /// ```
    ///
    /// `balanceMod` is 1.0: it indexes
    /// `Config.PVP_/PVE_PHYSICAL_ATTACK_CRITICAL_CHANCE_MULTIPLIERS`, and this
    /// dist populates neither table, so every class gets the `1f` default.
    #[allow(clippy::too_many_arguments)]
    pub fn auto_attack_crit(
        crit_stat: f64,
        defence_mul: f64,
        defence_add: f64,
        position: Position,
        position_mul: f64,
        from_z: i32,
        to_z: i32,
        attacker_level: i32,
        target_level: i32,
        roll: i32,
    ) -> bool {
        let rate_mod = ((defence_mul * crit_stat) + defence_add) / 10.0;
        let mut rate = critical_position_bonus(position, position_mul)
            * rate_mod
            * critical_height_bonus(from_z, to_z);
        if attacker_level >= 78 || target_level >= 78 {
            rate +=
                f64::from(attacker_level).sqrt() * f64::from(attacker_level - target_level) * 0.125;
        }
        rate = rate.clamp(3.0, 97.0);
        rate > f64::from(roll)
    }

    /// `Formulas.calcCrit`'s magic arm:
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
    /// Two identity terms, carriers named: `balanceMod` reads the unpopulated
    /// `*_MAGICAL_SKILL_CRITICAL_CHANCE_MULTIPLIERS` tables, and
    /// `DEFENCE_MAGIC_CRITICAL_RATE`/`_ADD` are declared only by skills in the
    /// 10500+ ranges — none learnable, none on an NPC skill list here.
    /// `(level - targetLevel) / 25` is integer division, kept as such.
    pub fn magic_crit(
        m_crit_rate: f64,
        is_bad: bool,
        caster_level: i32,
        target_level: i32,
        roll: i32,
    ) -> bool {
        if !is_bad {
            return m_crit_rate.min(320.0) > f64::from(roll);
        }
        let mut final_rate = m_crit_rate;
        if caster_level >= 78 && target_level >= 78 {
            final_rate +=
                f64::from(caster_level).sqrt() + f64::from((caster_level - target_level) / 25);
            return final_rate.min(320.0) > f64::from(roll);
        }
        final_rate.min(200.0) > f64::from(roll)
    }

    /// `Formulas.calcCrit`'s physical-skill arm:
    ///
    /// ```java
    /// return CommonUtil.constrain(rate * statBonus * rateBonus * balanceMod, 5, 90) > Rnd.get(100);
    /// ```
    ///
    /// `statBonus` is `BaseStat.STR.calcBonus(creature)` unless
    /// `STAT_BONUS_SKILL_CRITICAL` names another stat (no carrier on this
    /// dist); `rateBonus` is `getMul(CRITICAL_RATE_SKILL, 1)`, whose stat name
    /// appears nowhere in the datapack; `balanceMod` is the unpopulated config
    /// table again. Both identity terms are swept as inputs anyway.
    pub fn physical_skill_crit(
        critical_chance: f64,
        stat_bonus: f64,
        rate_bonus: f64,
        roll: i32,
    ) -> bool {
        (critical_chance * stat_bonus * rate_bonus).clamp(5.0, 90.0) > f64::from(roll)
    }

    /// `Formulas.calcBlowSuccess`:
    ///
    /// ```java
    /// final double critHeightBonus = calcCriticalHeightBonus(creature, target);
    /// final double criticalPosition = calcCriticalPositionBonus(creature, target);
    /// final double chanceBoostMod = (100 + chanceBoost) / 100;
    /// final double blowRateMod = creature.getStat().getValue(Stat.BLOW_RATE, 1);
    /// final double blowRateDefenseMod = target.getStat().getValue(Stat.BLOW_RATE_DEFENCE, 1);
    /// final double rate = criticalPosition * critHeightBonus * weaponCritical * chanceBoostMod * blowRateMod * blowRateDefenseMod;
    /// return Rnd.get(100) < Math.min(rate, Config.BLOW_RATE_CHANCE_LIMIT);
    /// ```
    ///
    /// `blowRateDefenseMod` has no carrier — `FatalBlowRateDefence` appears
    /// nowhere in the datapack — so the port folds only the attacker's
    /// `BLOW_RATE`. It is swept here as a separate input to keep the two
    /// multiplications distinguishable.
    #[allow(clippy::too_many_arguments)]
    pub fn blow_success(
        weapon_critical: f64,
        position: Position,
        position_mul: f64,
        from_z: i32,
        to_z: i32,
        chance_boost: f64,
        blow_rate_mod: f64,
        blow_rate_defence_mod: f64,
        limit: f64,
        roll: i32,
    ) -> bool {
        let rate = critical_position_bonus(position, position_mul)
            * critical_height_bonus(from_z, to_z)
            * weapon_critical
            * ((100.0 + chance_boost) / 100.0)
            * blow_rate_mod
            * blow_rate_defence_mod;
        f64::from(roll) < rate.min(limit)
    }

    /// `Formulas.calcAtkBreak`'s arithmetic — everything after the gates
    /// (channelling, `DC_MOD`, raid, HP-blocked, and the two
    /// `ALT_GAME_CANCEL_*` config switches that are the only sources of the
    /// opening 15):
    ///
    /// ```java
    /// init += Math.sqrt(13 * dmg);
    /// init -= ((BaseStat.MEN.calcBonus(target) * 100) - 100);
    /// double rate = target.getStat().getValue(Stat.ATTACK_CANCEL, init);
    /// rate = Math.max(Math.min(rate, 99), 1);
    /// return Rnd.get(100) < rate;
    /// ```
    ///
    /// `getValue(stat, init)` is `mul · init + add`, which is why the port
    /// takes the pair rather than a single modifier.
    // Java writes `Math.max(Math.min(rate, 99), 1)`; the transcription keeps
    // that nesting rather than collapsing it to `clamp`, so it reads as the
    // source does.
    #[allow(clippy::manual_clamp)]
    pub fn atk_break(
        dmg: f64,
        men_bonus: f64,
        cancel_mul: f64,
        cancel_add: f64,
        roll: i32,
    ) -> bool {
        let mut init = 15.0;
        init += (13.0 * dmg).sqrt();
        init -= (men_bonus * 100.0) - 100.0;
        let rate = ((init * cancel_mul) + cancel_add).min(99.0).max(1.0);
        f64::from(roll) < rate
    }

    /// `Formulas.calculatePvpPveBonus`, PvE arm:
    ///
    /// ```java
    /// return Math.max(0.05, (1 + ((pveAttack * pveRaidAttack) - (pveDefense * pveRaidDefense))) * pvePenalty);
    /// ```
    ///
    /// The PvP arm is the same shape with `dragonDefense * (1 + (pvpAttack -
    /// pvpDefense))` and no penalty; dragon weapons post-date Interlude, so the
    /// two collapse onto one expression with the raid pair and the penalty at
    /// identity. The `Math.max(0.05, …)` floor lives at the port's call sites
    /// (`skills::effects::traits`), not inside the formula, so the sweep
    /// applies it on the port side too.
    pub fn pvp_pve_bonus(
        attack_mul: f64,
        defence_mul: f64,
        raid_attack_mul: f64,
        raid_defence_mul: f64,
        pve_penalty: f64,
    ) -> f64 {
        (0.05f64).max(
            (1.0 + ((attack_mul * raid_attack_mul) - (defence_mul * raid_defence_mul)))
                * pve_penalty,
        )
    }

    /// `Formulas.calcMagicAffected` — the mana-drain landing roll:
    ///
    /// ```java
    /// double defence = 0;
    /// if (skill.isActive() && skill.isBad()) defence = target.getMDef();
    /// final double attack = 2 * actor.getMAtk() * calcGeneralTraitBonus(actor, target, skill.getTraitType(), false);
    /// double d = (attack - defence) / (attack + defence);
    /// d += 0.5 * Rnd.nextGaussian();
    /// return d > 0;
    /// ```
    ///
    /// `calcGeneralTraitBonus` is 1.0 for every carrier: all 23 skills with a
    /// `MagicalAttackMp` effect on this dist declare no `<trait>`, so the
    /// trait type is `NONE` and Java's own first branch returns 1. It is swept
    /// as an input regardless.
    pub fn magic_affected(m_atk: f64, defence: f64, trait_bonus: f64, gaussian: f64) -> bool {
        let attack = 2.0 * m_atk * trait_bonus;
        let d = ((attack - defence) / (attack + defence)) + (0.5 * gaussian);
        d > 0.0
    }

    /// `Creature.getRandomDamageMultiplier`:
    ///
    /// ```java
    /// final int random = (int) _stat.getValue(Stat.RANDOM_DAMAGE);
    /// return (1 + ((double) Rnd.get(-random, random) / 100));
    /// ```
    pub fn random_damage_multiplier(roll_neg_r_to_r: i32) -> f64 {
        1.0 + (f64::from(roll_neg_r_to_r) / 100.0)
    }
}

/// The grid. Small on purpose: every combination of these is swept, so the
/// product matters more than any single row's spread.
const P_ATKS: &[f64] = &[1.0, 37.0, 250.0, 1_337.0, 9_999.0];
const P_DEFS: &[f64] = &[1.0, 43.0, 300.0, 2_048.0];
const RANDOM_MULS: &[f64] = &[0.9, 1.0, 1.1];
const CRIT_MULS: &[f64] = &[2.0, 3.5];
const CRIT_ADDS: &[f64] = &[0.0, 137.0];
const MODS: &[f64] = &[1.0, 0.75, 1.4];
/// `(ss/sps charged, SHOTS_BONUS)` pairs. The bonus is `1 + enchant·0.003`, so
/// the three values are a bare weapon, a +4 and a +16 — and the shotless row
/// carries a non-1 bonus on purpose, to catch a formula that multiplies it in
/// where Java's `: 1` arm does not.
/// `Heal.instant`'s three caster tests, which its arms disagree about in both
/// the mAtk multiplier and the static bonus.
const CASTERS: &[HealCaster] = &[
    HealCaster::PlayerMage,
    HealCaster::PlayerFighter,
    HealCaster::Summon,
    HealCaster::Npc,
];
const SHOTS: &[(bool, f64)] = &[
    (false, 1.0),
    (false, 1.03),
    (true, 1.0),
    (true, 1.012),
    (true, 1.048),
];

fn positions() -> [(Position, f64); 3] {
    // The `proxBonus` fraction Java picks per position.
    [
        (Position::Front, 0.0),
        (Position::Side, 0.05),
        (Position::Back, 0.2),
    ]
}

/// **The sweep.** ~48 000 cases: every position × crit × shot × ranged over the
/// attack/defence/random/crit-stat/modifier grid.
#[test]
fn auto_attack_damage_matches_java_across_the_grid() {
    let mut cases = 0usize;
    for &p_atk in P_ATKS {
        for &p_def in P_DEFS {
            for &random_mul in RANDOM_MULS {
                for &(position, prox) in &positions() {
                    for &crit in &[false, true] {
                        for &(ss, shots_bonus) in SHOTS {
                            for &is_ranged in &[false, true] {
                                for &c_mul in CRIT_MULS {
                                    for &c_add in CRIT_ADDS {
                                        for &m in MODS {
                                            let cd = CritDamage {
                                                mul: c_mul,
                                                add: c_add,
                                            };
                                            let ours = formulas::calc_auto_attack_damage(
                                                p_atk,
                                                random_mul,
                                                position,
                                                p_def,
                                                crit,
                                                cd,
                                                ss,
                                                shots_bonus,
                                                is_ranged,
                                                m,
                                                m,
                                                m,
                                            );
                                            let theirs = java::auto_attack_damage(
                                                p_atk,
                                                random_mul,
                                                prox,
                                                p_def,
                                                crit,
                                                c_mul,
                                                c_add,
                                                ss,
                                                shots_bonus,
                                                is_ranged,
                                                m,
                                                m,
                                                m,
                                            );
                                            assert!(
                                                (ours - theirs).abs() <= theirs.abs() * 1e-12,
                                                "auto-attack damage diverged: ours {ours}, Java \
                                                 {theirs} — pAtk {p_atk}, pDef {p_def}, random \
                                                 {random_mul}, {position:?}, crit {crit}, ss \
                                                 {ss}/{shots_bonus}, ranged {is_ranged}, cAtk \
                                                 {c_mul}, cAtkAdd {c_add}, mods {m}"
                                            );
                                            cases += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 10_000, "the grid collapsed to {cases} cases");
}

/// The two shapes that are easy to get right in one case and wrong in the
/// other, called out separately so a failure names the mechanism rather than a
/// grid coordinate.
#[test]
fn the_ranged_weapon_mod_doubles_and_its_crit_splits() {
    let plain = |is_ranged| {
        formulas::calc_auto_attack_damage(
            100.0,
            1.0,
            Position::Front,
            50.0,
            false,
            CritDamage::default(),
            false,
            1.0,
            is_ranged,
            1.0,
            1.0,
            1.0,
        )
    };
    assert!(
        (plain(true) - plain(false) * 2.0).abs() < 1e-9,
        "a bow swings on 154 where a sword swings on 77"
    );

    // A ranged **crit** takes half the crit branch and half the flat one; a
    // melee crit takes the crit branch alone. With cAtk 2 that makes the
    // ranged crit 1.5× its own flat hit, not 2×.
    let crit = |is_ranged| {
        formulas::calc_auto_attack_damage(
            100.0,
            1.0,
            Position::Front,
            50.0,
            true,
            CritDamage::default(),
            false,
            1.0,
            is_ranged,
            1.0,
            1.0,
            1.0,
        )
    };
    assert!(
        (crit(false) - plain(false) * 2.0).abs() < 1e-9,
        "melee crit: the whole crit branch"
    );
    assert!(
        (crit(true) - plain(true) * 1.5).abs() < 1e-9,
        "ranged crit: half of each branch"
    );
}

/// The elemental ladder, swept across the band and both caps.
#[test]
fn attribute_bonus_matches_java_across_the_band() {
    for attack in (0..=400).step_by(7) {
        for defence in (0..=400).step_by(11) {
            let (a, d) = (attack as f64, defence as f64);
            let ours = formulas::calc_attribute_bonus(a, d);
            let theirs = java::attribute_bonus(a, d);
            assert!(
                (ours - theirs).abs() < 1e-9,
                "attribute bonus diverged at attack {a} / defence {d}: ours {ours}, Java {theirs}"
            );
        }
    }
}

/// **The physical-skill sweep** — `PhysicalAttack`'s damage half over the same
/// grid, both weapon classes, crit and shot on and off.
#[test]
fn physical_skill_damage_matches_java_across_the_grid() {
    let mut cases = 0usize;
    for &p_atk in P_ATKS {
        for &p_def in P_DEFS {
            for &power in &[0.0, 55.0, 1_200.0] {
                for &level_mod in &[0.5, 1.0, 1.89] {
                    for &random_mod in RANDOM_MULS {
                        for &crit in &[false, true] {
                            for &(ss, shots_bonus) in SHOTS {
                                for &is_ranged in &[false, true] {
                                    for &m in MODS {
                                        let ours = formulas::calc_physical_skill_damage(
                                            p_atk,
                                            1.0,
                                            p_def,
                                            1.0,
                                            power,
                                            level_mod,
                                            random_mod,
                                            crit,
                                            2.0,
                                            ss,
                                            shots_bonus,
                                            is_ranged,
                                        ) * m;
                                        let theirs = java::physical_skill_damage(
                                            p_atk,
                                            1.0,
                                            p_def,
                                            1.0,
                                            power,
                                            level_mod,
                                            random_mod,
                                            crit,
                                            2.0,
                                            ss,
                                            shots_bonus,
                                            is_ranged,
                                            m,
                                        );
                                        assert!(
                                            (ours - theirs).abs() <= theirs.abs() * 1e-12,
                                            "physical skill damage diverged: ours {ours}, Java \
                                             {theirs} — pAtk {p_atk}, pDef {p_def}, power \
                                             {power}, levelMod {level_mod}, random {random_mod}, \
                                             crit {crit}, ss {ss}/{shots_bonus}, ranged \
                                             {is_ranged}, mods {m}"
                                        );
                                        cases += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 10_000, "the grid collapsed to {cases} cases");
}

/// **The magic sweep** — `calcMagicDam` including its failure branches and the
/// `randomMod` the port had been dropping.
#[test]
fn magic_damage_matches_java_across_the_grid() {
    use gameserver::model::formulas::MagicFailure;

    let mut cases = 0usize;
    for &m_atk in &[1.0, 40.0, 900.0, 4_000.0] {
        for &m_def in &[1.0, 38.0, 400.0, 3_000.0] {
            for &power in &[1.0, 12.0, 340.0] {
                for &(failure, code) in &[
                    (MagicFailure::None, 0u8),
                    (MagicFailure::Half, 1),
                    (MagicFailure::Resisted, 2),
                ] {
                    for &shots in &[1.0, 2.0, 4.0] {
                        for &mcrit in &[false, true] {
                            for &random_mod in RANDOM_MULS {
                                for &m in MODS {
                                    let ours = formulas::calc_magic_dam(
                                        m_atk, m_def, power, mcrit, 3.0, shots, failure, random_mod,
                                    ) * m;
                                    let theirs = java::magic_damage(
                                        m_atk, m_def, power, mcrit, 3.0, shots, code, random_mod, m,
                                    );
                                    assert!(
                                        (ours - theirs).abs() <= theirs.abs() * 1e-12,
                                        "magic damage diverged: ours {ours}, Java {theirs} — \
                                         mAtk {m_atk}, mDef {m_def}, power {power}, failure \
                                         {code}, shots {shots}, mcrit {mcrit}, random \
                                         {random_mod}, mods {m}"
                                    );
                                    cases += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 5_000, "the grid collapsed to {cases} cases");
}

/// **The blow sweep** — the dagger formula, including the crit-damage block
/// that only bites when the attacker carries the stats.
#[test]
fn blow_damage_matches_java_across_the_grid() {
    use gameserver::model::formulas::BlowCritDamage;

    let mut cases = 0usize;
    for &p_atk in P_ATKS {
        for &p_def in P_DEFS {
            for &power in &[0.0, 90.0, 2_000.0] {
                for &(position, is_position) in &positions_blow() {
                    for &random_mod in RANDOM_MULS {
                        for &(ss, shots_bonus) in SHOTS {
                            for &cd_mult in &[1.0, 1.35, 2.2] {
                                for &cd_patk in &[0.0, 18.0] {
                                    for &m in MODS {
                                        let ours = formulas::calc_blow_damage(
                                            p_atk,
                                            power,
                                            p_def,
                                            position,
                                            random_mod,
                                            ss,
                                            shots_bonus,
                                            BlowCritDamage {
                                                mult: cd_mult,
                                                p_atk_add: cd_patk,
                                            },
                                        ) * m;
                                        let theirs = java::blow_damage(
                                            p_atk,
                                            power,
                                            p_def,
                                            is_position,
                                            random_mod,
                                            ss,
                                            shots_bonus,
                                            cd_mult,
                                            cd_patk,
                                            m,
                                        );
                                        assert!(
                                            (ours - theirs).abs() <= theirs.abs() * 1e-12,
                                            "blow damage diverged: ours {ours}, Java {theirs} — \
                                             pAtk {p_atk}, pDef {p_def}, power {power}, \
                                             {position:?}, random {random_mod}, ss {ss}, cdMult \
                                             {cd_mult}, cdPatk {cd_patk}, mods {m}"
                                        );
                                        cases += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 10_000, "the grid collapsed to {cases} cases");
}

/// **The mana sweep** — and the ordering it exists to pin: the trait and
/// pvp/pve multipliers go in **before** the crit's `min(damage·3, critLimit)`.
#[test]
fn mana_damage_matches_java_across_the_grid() {
    use gameserver::model::formulas::MagicFailure;

    let mut cases = 0usize;
    for &m_atk in &[1.0, 40.0, 900.0, 4_000.0] {
        for &m_def in &[1.0, 38.0, 400.0] {
            for &max_mp in &[97.0, 970.0, 4_800.0] {
                for &power in &[1.0, 20.0, 260.0] {
                    for &(failure, code) in &[
                        (MagicFailure::None, 0u8),
                        (MagicFailure::Half, 1),
                        (MagicFailure::Resisted, 2),
                    ] {
                        for &mcrit in &[false, true] {
                            // The dist's real limits, plus one low enough to
                            // bind on every input.
                            for &limit in &[100.0, 1_450.0, 7_000.0] {
                                for &m in MODS {
                                    let ours = formulas::calc_mana_dam(
                                        m_atk, m_def, max_mp, power, 1.0, failure, mcrit, limit, m,
                                        m,
                                    );
                                    let theirs = java::mana_damage(
                                        m_atk, m_def, max_mp, power, 1.0, code, mcrit, limit, m, m,
                                    );
                                    assert!(
                                        (ours - theirs).abs() <= theirs.abs() * 1e-12,
                                        "mana damage diverged: ours {ours}, Java {theirs} — mAtk \
                                         {m_atk}, mDef {m_def}, maxMp {max_mp}, power {power}, \
                                         failure {code}, mcrit {mcrit}, limit {limit}, mods {m}"
                                    );
                                    cases += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 3_000, "the grid collapsed to {cases} cases");
}

/// The blow formula's positions, with the fraction Java picks for each.
fn positions_blow() -> [(Position, f64); 3] {
    [
        (Position::Front, 0.0),
        (Position::Side, 0.05),
        (Position::Back, 0.2),
    ]
}

/// **The timing sweep** — attack speed, the swing interval and the cast-time
/// factor, including the spiritshot bonus and the channeling early return.
#[test]
fn timing_formulas_match_java_across_the_grid() {
    use gameserver::model::skill::{OperateType, Skill};

    // `calcAtkSpd` / `calculateTimeBetweenAttacks` — pure integer maths, so
    // the comparison is exact.
    for &skill_time in &[0.0, 500.0, 1_333.0, 15_000.0] {
        for &spd in &[1, 33, 300, 1_500, 9_999] {
            for &magic in &[false, true] {
                let combat = gameserver::model::components::CombatStats {
                    p_atk_spd: spd,
                    m_atk_spd: spd,
                    ..Default::default()
                };
                let skill = Skill {
                    magic_type: if magic { 1 } else { 0 },
                    ..Default::default()
                };
                assert_eq!(
                    formulas::calc_atk_spd(&combat, &skill, skill_time),
                    java::atk_spd(skill_time, spd, magic),
                    "calcAtkSpd diverged at skillTime {skill_time}, spd {spd}, magic {magic}"
                );
            }
            assert_eq!(
                formulas::calculate_time_between_attacks(spd),
                java::time_between_attacks(spd),
                "calculateTimeBetweenAttacks diverged at {spd}"
            );
        }
    }

    // `calcSkillTimeFactor` — the port reads the multiplier off the world, so
    // the shape is what is swept: which branch fires, and what the spiritshot
    // bonus does to it.
    for &multiplier in &[0.001, 0.5, 1.0, 2.7] {
        for &magic in &[false, true] {
            for &channeling in &[false, true] {
                for &static_magic in &[false, true] {
                    for &charged in &[false, true] {
                        let theirs = java::skill_time_factor(
                            multiplier,
                            magic,
                            channeling,
                            static_magic,
                            charged,
                        );
                        // The port's own composition of the same branches.
                        let ours = if channeling || static_magic {
                            1.0
                        } else if magic && charged {
                            (multiplier + multiplier * 0.4).max(0.01)
                        } else {
                            multiplier.max(0.01)
                        };
                        assert!(
                            (ours - theirs).abs() < 1e-12,
                            "skill time factor diverged: ours {ours}, Java {theirs} — mul \
                             {multiplier}, magic {magic}, channeling {channeling}, static \
                             {static_magic}, charged {charged}"
                        );
                    }
                }
            }
        }
    }
    let _ = OperateType::Channeling;
}

/// **The heal sweep.** The arithmetic came back clean on the second pass and is
/// kept because an agreement nobody can re-derive is indistinguishable from an
/// untested formula — but the grid now also drives `SHOTS_BONUS`, which the
/// shot branch scales by and the grade branch does **not**. That asymmetry is
/// the only thing separating two branches that otherwise both land on 2.
#[test]
fn heal_amount_matches_java_across_the_grid() {
    let mut cases = 0usize;
    for &power in &[0.0, 25.0, 340.0, 5_000.0] {
        for &m_atk in &[1.0, 60.0, 900.0, 6_000.0] {
            for &mp_consume in &[0, 12, 90] {
                for &(sps, bss) in &[(false, false), (true, false), (false, true)] {
                    for &caster in CASTERS {
                        for &shots_bonus in &[1.0, 1.012, 1.048] {
                            for &mcrit in &[false, true] {
                                let ours = formulas::calc_heal(
                                    power,
                                    m_atk,
                                    mcrit,
                                    sps,
                                    bss,
                                    mp_consume,
                                    caster,
                                    shots_bonus,
                                );
                                let theirs = java::heal_amount(
                                    power,
                                    m_atk,
                                    mcrit,
                                    sps,
                                    bss,
                                    mp_consume,
                                    caster,
                                    shots_bonus,
                                );
                                assert!(
                                    (ours - theirs).abs() <= theirs.abs() * 1e-12,
                                    "heal diverged: ours {ours}, Java {theirs} — power {power}, \
                                     mAtk {m_atk}, mpConsume {mp_consume}, sps {sps}, bss {bss}, \
                                     caster {caster:?}, shots {shots_bonus}, crit {mcrit}"
                                );
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 500, "the grid collapsed to {cases} cases");
}

/// **The shield sweep.** The bug it was written for: the port hard-coded the
/// attacker as melee at every one of its four call sites, so Java's
/// `if (attacker.getAttackType().isRanged()) shldRate *= 1.3` never applied and
/// a bow lost its 30 % block bonus against every shield in the game.
#[test]
fn shield_use_matches_java_across_the_grid() {
    let mut cases = 0usize;
    for &shield_rate in &[0.0, 5.0, 20.0, 37.5, 60.0, 100.0] {
        for &con_bonus in &[0.8, 1.0, 1.24, 1.5] {
            for &ranged in &[false, true] {
                for rate_roll in [0, 19, 25, 26, 50, 99] {
                    for perfect_roll in [0, 50, 71, 72, 99] {
                        let ours = formulas::calc_shield_use(
                            shield_rate,
                            con_bonus,
                            ranged,
                            // `from_behind` is the port's name for Java's
                            // `degreeside` gate, which is a *caller* input here.
                            false,
                            rate_roll,
                            perfect_roll,
                        );
                        let theirs = java::shield_use(
                            shield_rate,
                            con_bonus,
                            ranged,
                            rate_roll,
                            perfect_roll,
                        );
                        assert_eq!(
                            ours, theirs,
                            "shield use diverged — rate {shield_rate}, con {con_bonus}, ranged \
                             {ranged}, rolls {rate_roll}/{perfect_roll}"
                        );
                        cases += 1;
                    }
                }
            }
        }
    }
    assert!(cases > 1_000, "the grid collapsed to {cases} cases");
}

/// **The land-rate sweep** (`calcEffectSuccess`). The clamp is the interesting
/// part: `traitMod > 0` short-circuits *past* it, so invulnerability is a 0 and
/// not the 10 floor, while `basicPropertyResist` multiplies in *after* it and so
/// can reach 0 from the other side.
///
/// The port's live divergence here was not arithmetic but the **gate**: it only
/// ran this formula for `isBad()` skills, where Java runs it for every
/// continuous skill whose `activateRate` is not the `-1` sentinel.
#[test]
fn effect_land_rate_matches_java_across_the_grid() {
    let bounds = formulas::LandRateBounds {
        min: 10.0,
        max: 90.0,
    };
    let mut cases = 0usize;
    for &magic_level in &[-1, 0, 20, 46, 78] {
        for &activate_rate in &[-1, 0, 35, 70, 100] {
            for &lvl_bonus_rate in &[0, 5, 20, 30] {
                for &target_level in &[1, 40, 80] {
                    for &buff_debuff_mod in MODS {
                        for &element_mod in MODS {
                            for &trait_mod in &[0.0, 0.7, 1.0, 1.15] {
                                for &(basic_property, basic_resist) in
                                    &[(0.0, 1.0), (13.0, 0.6), (40.0, 0.0)]
                                {
                                    let ours = formulas::calc_effect_land_rate(
                                        magic_level,
                                        activate_rate,
                                        lvl_bonus_rate,
                                        target_level,
                                        buff_debuff_mod,
                                        element_mod,
                                        trait_mod,
                                        basic_property,
                                        basic_resist,
                                        bounds,
                                    );
                                    let theirs = java::effect_land_rate(
                                        magic_level,
                                        activate_rate,
                                        lvl_bonus_rate,
                                        target_level,
                                        buff_debuff_mod,
                                        element_mod,
                                        trait_mod,
                                        basic_property,
                                        basic_resist,
                                        bounds.min,
                                        bounds.max,
                                    );
                                    assert_eq!(
                                        ours.to_bits(),
                                        theirs.to_bits(),
                                        "land rate diverged: ours {ours}, Java {theirs} — mLvl \
                                         {magic_level}, activate {activate_rate}, lvlBonus \
                                         {lvl_bonus_rate}, tLvl {target_level}, buffDebuff \
                                         {buff_debuff_mod}, element {element_mod}, trait \
                                         {trait_mod}, basicProperty {basic_property}, resist \
                                         {basic_resist}"
                                    );
                                    cases += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 5_000, "the grid collapsed to {cases} cases");
}

/// **The magic-success sweep** (`calcMagicSuccess`). Bit-exact rather than
/// approximate because the result is an `int`: the whole formula funnels through
/// `Math.round((float) …)`, so the only thing that can diverge is the narrowing,
/// and an epsilon comparison would hide exactly that.
#[test]
fn magic_success_rate_matches_java_across_the_grid() {
    // `Config.NPC_SKILL_CHANCE_PENALTY` as this dist ships it.
    const PENALTY: &[f64] = &[2.5, 3.0, 3.25, 3.5];
    let mut cases = 0usize;
    for &pve in &[false, true] {
        for &target_level in &[1, 40, 77, 78, 85] {
            for &effective_level in &[1, 40, 78, 85] {
                for &caster_player_level in &[None, Some(40), Some(75), Some(82)] {
                    for &target_is_raid in &[false, true] {
                        for &(magic_accuracy, magic_evasion) in
                            &[(100, 100), (100, 118), (100, 122), (100, 128), (100, 140)]
                        {
                            for &res_modifier in &[0.5, 1.0, 1.3] {
                                let input = formulas::MagicSuccess {
                                    pve,
                                    target_level,
                                    effective_level,
                                    caster_player_level,
                                    target_is_raid,
                                    min_npc_level_for_magic_penalty: 78,
                                    skill_chance_penalty: PENALTY,
                                    magic_accuracy,
                                    magic_evasion,
                                    res_modifier,
                                };
                                let ours = formulas::calc_magic_success_rate(&input);
                                let theirs = java::magic_success_rate(
                                    pve,
                                    target_level,
                                    effective_level,
                                    caster_player_level,
                                    target_is_raid,
                                    78,
                                    PENALTY,
                                    magic_accuracy,
                                    magic_evasion,
                                    res_modifier,
                                );
                                assert_eq!(
                                    ours, theirs,
                                    "magic success diverged: ours {ours}, Java {theirs} — pve \
                                     {pve}, tLvl {target_level}, effLvl {effective_level}, caster \
                                     {caster_player_level:?}, raid {target_is_raid}, mAcc \
                                     {magic_accuracy}/{magic_evasion}, res {res_modifier}"
                                );
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 1_000, "the grid collapsed to {cases} cases");
}

/// **The resurrect sweep.** Small, but it guards a genuinely odd line — a bonus
/// that already exceeds +20 gets a *further* flat +20, so the curve steps rather
/// than scales. A "tidied" port that dropped the step would still look right at
/// low WIT.
#[test]
fn resurrect_restore_matches_java_across_the_band() {
    for base in [0.0, 1.0, 20.0, 35.0, 50.0, 70.0, 85.0, 100.0] {
        for wit_bonus in [0.5, 1.0, 1.16, 1.4, 2.0, 3.0] {
            let ours = formulas::calc_resurrect_restore_percent(base, wit_bonus);
            let theirs = java::resurrect_restore_percent(base, wit_bonus);
            assert_eq!(
                ours.to_bits(),
                theirs.to_bits(),
                "resurrect restore diverged: ours {ours}, Java {theirs} — base {base}, wit \
                 {wit_bonus}"
            );
        }
    }
}

/// **The abnormal-time sweep** (`calcEffectAbnormalTime`). Trivial arithmetic;
/// the bug was that the port did not run it at all, so a Skill Mastery proc
/// never doubled a buff's duration. Swept against the port's own composition,
/// which is the shape `apply_continuous_effects` computes inline.
#[test]
fn effect_abnormal_time_matches_java_across_the_grid() {
    for abnormal_time in [-1, 0, 15, 120, 1_200, 3_600] {
        for is_static in [false, true] {
            for mastery in [false, true] {
                let ours = if !is_static && mastery {
                    abnormal_time * 2
                } else {
                    abnormal_time
                };
                let theirs = java::effect_abnormal_time(abnormal_time, is_static, mastery);
                assert_eq!(
                    ours, theirs,
                    "abnormal time diverged — time {abnormal_time}, static {is_static}, mastery \
                     {mastery}"
                );
            }
        }
    }
}

/// **The enchant-table sweep** (`IStatFunction.calcEnchantedItemBonus`'s three
/// tables). The port had none of them: an enchanted weapon added no P.Atk and
/// enchanted armour no P.Def, which is most of what enchanting is *for*.
///
/// Swept over every grade including the unreachable R arm, both weapon-slot
/// classes and both weapon reaches — the arms Interlude cannot reach are cheap
/// to carry and the transcription reads like Java with them in.
#[test]
fn enchant_bonus_tables_match_java_across_the_grid() {
    use gameserver::data::item_data::{CrystalType, SLOT_LR_HAND, SLOT_R_HAND, WeaponType};
    use gameserver::model::enchant_bonus::{
        enchant_def_bonus, enchant_m_atk_bonus, enchant_p_atk_bonus,
    };

    const GRADES: &[CrystalType] = &[
        CrystalType::None,
        CrystalType::D,
        CrystalType::C,
        CrystalType::B,
        CrystalType::A,
        CrystalType::S,
        // `getCrystalTypePlus()` folds these onto S and R — swept so a missing
        // `.plus()` shows up as a divergence rather than as a silent default.
        CrystalType::S80,
        CrystalType::S84,
        CrystalType::R,
        CrystalType::R99,
    ];
    // `(bodyPart, itemType)` triples covering Java's `SLOT_LR_HAND && !POLE`
    // test from both sides, plus the ranged split.
    const SHAPES: &[(i32, WeaponType)] = &[
        (SLOT_R_HAND, WeaponType::Sword),
        (SLOT_LR_HAND, WeaponType::Sword),
        (SLOT_LR_HAND, WeaponType::Pole),
        (SLOT_LR_HAND, WeaponType::Bow),
        (SLOT_R_HAND, WeaponType::Bow),
        (SLOT_LR_HAND, WeaponType::TwoHandCrossbow),
    ];

    let mut cases = 0usize;
    for &grade in GRADES {
        for enchant in 0..=20 {
            assert_eq!(
                enchant_def_bonus(grade, enchant).to_bits(),
                java::enchant_def_bonus(grade.plus(), enchant).to_bits(),
                "enchant def bonus diverged — {grade:?} +{enchant}"
            );
            assert_eq!(
                enchant_m_atk_bonus(grade, enchant).to_bits(),
                java::enchant_m_atk_bonus(grade.plus(), enchant).to_bits(),
                "enchant mAtk bonus diverged — {grade:?} +{enchant}"
            );
            for &(body_part, weapon_type) in SHAPES {
                let two_hand = body_part == SLOT_LR_HAND && weapon_type != WeaponType::Pole;
                let ranged = matches!(
                    weapon_type,
                    WeaponType::Bow | WeaponType::Crossbow | WeaponType::TwoHandCrossbow
                );
                assert_eq!(
                    enchant_p_atk_bonus(grade, body_part, weapon_type, enchant).to_bits(),
                    java::enchant_p_atk_bonus(grade.plus(), two_hand, ranged, enchant).to_bits(),
                    "enchant pAtk bonus diverged — {grade:?} +{enchant}, slot {body_part:#x}, \
                     {weapon_type:?}"
                );
                cases += 1;
            }
        }
    }
    assert!(cases > 1_000, "the grid collapsed to {cases} cases");

    // The shape the whole family shares, asserted once so a table that lost its
    // `max(0, enchant - 3)` term fails on the mechanism rather than on a
    // coordinate: past +3 each level is worth three times a level below it.
    let s_two_hand = |e| enchant_p_atk_bonus(CrystalType::S, SLOT_LR_HAND, WeaponType::Sword, e);
    assert_eq!(s_two_hand(3), 18.0, "6 per level up to +3");
    assert_eq!(s_two_hand(4) - s_two_hand(3), 18.0, "and 18 for the fourth");
}

/// **The shots-bonus sweep** (`ShotsBonusFinalizer`). Tiny, but it was recorded
/// on the first parity pass as a term with *no carrier on this dist* — which was
/// wrong, and wrong in a way a sweep of `calcAutoAttackDamage` alone could never
/// show, because both sides had hard-coded the same 1.0.
#[test]
fn shots_bonus_matches_java_across_the_band() {
    use gameserver::model::enchant_bonus::shots_bonus;
    for enchant in 0..=25 {
        assert_eq!(
            shots_bonus(enchant).to_bits(),
            java::shots_bonus(enchant).to_bits(),
            "shots bonus diverged at +{enchant}"
        );
    }
    assert_eq!(shots_bonus(0), 1.0, "an unenchanted weapon buys nothing");
    assert!(
        (shots_bonus(10) - 1.03).abs() < 1e-12,
        "a +10 weapon lifts every shot by 3 %"
    );
}

// ---------------------------------------------------------------------------
// The roll family
//
// Opened after the sixteen magnitude sweeps above. Those take attack, defence
// and the modifiers as *inputs* and check the number that comes out; none of
// them touches the formulas that decide whether a swing lands, crits, breaks a
// cast or steals mana. A wrong term in a damage formula shows up as a number
// somebody can compare against a Java server; a wrong term in a rate shows up
// as nothing at all, which is why two of the three findings below had been sat
// on since the systems were ported.
// ---------------------------------------------------------------------------

/// Rolls for the 1000-sided formulas (hit, magic crit), including both sides of
/// each clamp.
const ROLLS_1000: &[i32] = &[0, 1, 199, 200, 201, 319, 320, 499, 500, 979, 980, 981, 999];
/// Rolls for the 100-sided formulas (crit, blow, cast break).
const ROLLS_100: &[i32] = &[
    0, 1, 2, 3, 4, 5, 9, 10, 11, 33, 50, 79, 80, 89, 90, 96, 97, 99,
];
/// Levels either side of Java's 78 gate, up to this dist's `MaximumPlayerLevel`.
const LEVELS: &[i32] = &[1, 40, 77, 78, 79, 80];
/// z differences either side of the ±50 hit-condition band and the ±25 crit
/// clamp.
const Z_DIFFS: &[i32] = &[-200, -51, -50, -26, -25, 0, 25, 26, 50, 51, 200];

/// **The hit sweep** (`calcHitMiss` + `getConditionBonus`), on the real
/// `hitConditionBonus.xml`.
///
/// The condition bonus is the half that mattered: the port parsed `dark` and
/// then dropped it, on a doc comment saying there was no game clock — which
/// stopped being true at G33, when `game_time::is_night_at` landed and the
/// night spawns started using it. Java subtracts 10 points of hit chance for
/// the whole in-game night, so every auto-attack in the dark was landing at
/// the daytime rate.
#[test]
fn hit_rolls_match_java_across_the_grid() {
    let bonuses = gameserver::data::HitConditionBonusData::load_from(common::DIST);
    let mut cases = 0usize;
    let mut nights = 0usize;
    for &accuracy in &[0, 37, 100, 251, 500] {
        for &evasion in &[0, 41, 100, 251, 500] {
            for &(position, _) in &positions() {
                for &z in Z_DIFFS {
                    for &night in &[false, true] {
                        let ours = bonuses.condition_bonus(z, 0, position, night);
                        let theirs = java::hit_condition_bonus(&bonuses, z, 0, night, position);
                        assert!(
                            (ours - theirs).abs() < 1e-9,
                            "condition bonus diverged — z {z}, night {night}, {position:?}: \
                             {ours} vs {theirs}"
                        );
                        for &roll in ROLLS_1000 {
                            let ours = formulas::calc_hit_miss(accuracy, evasion, theirs, roll);
                            let theirs = java::hit_miss(accuracy, evasion, theirs, roll);
                            assert_eq!(
                                ours, theirs,
                                "hit/miss diverged — acc {accuracy}, eva {evasion}, z {z}, \
                                 night {night}, {position:?}, roll {roll}"
                            );
                            cases += 1;
                            nights += usize::from(night);
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 10_000, "the grid collapsed to {cases} cases");
    assert!(nights > 0, "the night half of the grid never ran");
}

/// **The auto-attack crit sweep** (`calcCrit`'s `skill == null` tail).
///
/// Two findings, both live:
///
/// * the **height bonus** was evaluated in floating point. Java's expression is
///   `int` throughout and its `/ 100` truncates a numerator that never leaves
///   −10..30, so Java's answer is a flat 1 and the port was handing out 1.1 on
///   level ground and up to 1.3 uphill — a 10 % crit-rate gift to everyone,
///   before position;
/// * the **level term** was missing outright. Java adds
///   `sqrt(level) · (level − targetLevel) · 0.125` as soon as *either* side is
///   78 or over, which an 80-level cap puts inside the endgame's reach.
#[test]
fn auto_attack_crit_rolls_match_java_across_the_grid() {
    let mut cases = 0usize;
    for &crit_stat in &[0.0, 44.0, 120.0, 440.0, 1_500.0] {
        for &defence_mul in &[1.0, 0.85, 0.7] {
            for &defence_add in &[0.0, 100.0] {
                for &(position, _) in &positions() {
                    for &position_mul in &[1.0, 0.7, 1.6] {
                        for &z in Z_DIFFS {
                            for &attacker_level in LEVELS {
                                for &target_level in LEVELS {
                                    for &roll in ROLLS_100 {
                                        let ours = formulas::calc_auto_attack_crit(
                                            crit_stat,
                                            defence_mul,
                                            defence_add,
                                            position,
                                            position_mul,
                                            z,
                                            0,
                                            attacker_level,
                                            target_level,
                                            roll,
                                        );
                                        let theirs = java::auto_attack_crit(
                                            crit_stat,
                                            defence_mul,
                                            defence_add,
                                            position,
                                            position_mul,
                                            z,
                                            0,
                                            attacker_level,
                                            target_level,
                                            roll,
                                        );
                                        assert_eq!(
                                            ours, theirs,
                                            "auto-attack crit diverged — stat {crit_stat}, \
                                             def {defence_mul}/{defence_add}, {position:?} \
                                             x{position_mul}, z {z}, levels \
                                             {attacker_level}v{target_level}, roll {roll}"
                                        );
                                        cases += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 100_000, "the grid collapsed to {cases} cases");
}

/// The height bonus, pinned on its own. It is the one term in this family that
/// is a **constant** in Java, and a port that "fixes" the integer division
/// silently doubles every dagger's blow rate off a ledge.
#[test]
fn the_critical_height_bonus_is_flat_one_at_every_z() {
    for &z in Z_DIFFS {
        for &to_z in &[-1_000, 0, 1_000] {
            assert!(
                (formulas::calc_critical_height_bonus(z + to_z, to_z) - 1.0).abs() < 1e-12,
                "height bonus moved at z {z}"
            );
            assert!(
                (java::critical_height_bonus(z + to_z, to_z) - 1.0).abs() < 1e-12,
                "the transcription moved at z {z}"
            );
        }
    }
}

/// **The magic and physical skill-crit sweep** (`calcCrit`'s other two arms).
///
/// The magic arm's bad-skill cap lifts from 200‰ to 320‰ once both sides are
/// 78 or over, and a `sqrt(level)` bonus rides in with it; the port capped at a
/// flat 200‰ for every debuff, so an endgame nuker's landed crits were short.
#[test]
fn skill_crit_rolls_match_java_across_the_grid() {
    let mut cases = 0usize;
    for &rate in &[0.0, 50.0, 100.0, 199.0, 200.0, 320.0, 1_000.0] {
        for &is_bad in &[false, true] {
            for &caster_level in LEVELS {
                for &target_level in LEVELS {
                    for &roll in ROLLS_1000 {
                        let ours = formulas::calc_magic_crit(
                            rate,
                            is_bad,
                            caster_level,
                            target_level,
                            roll,
                        );
                        let theirs =
                            java::magic_crit(rate, is_bad, caster_level, target_level, roll);
                        assert_eq!(
                            ours, theirs,
                            "magic crit diverged — rate {rate}, bad {is_bad}, levels \
                             {caster_level}v{target_level}, roll {roll}"
                        );
                        cases += 1;
                    }
                }
            }
        }
    }
    // The physical arm shares the entry point but none of the terms.
    for &chance in &[0.0, 5.0, 10.0, 15.0, 40.0, 100.0] {
        for &stat_bonus in &[0.5, 1.0, 1.2, 3.0] {
            for &roll in ROLLS_100 {
                let ours = formulas::calc_physical_skill_crit(chance, stat_bonus, roll);
                // `CRITICAL_RATE_SKILL` has no carrier on this dist, so the
                // rate bonus is the identity the port folds away.
                let theirs = java::physical_skill_crit(chance, stat_bonus, 1.0, roll);
                assert_eq!(
                    ours, theirs,
                    "physical skill crit diverged — chance {chance}, stat {stat_bonus}, \
                     roll {roll}"
                );
                cases += 1;
            }
        }
    }
    assert!(cases > 5_000, "the grid collapsed to {cases} cases");
}

/// **The blow sweep** (`calcBlowSuccess`) — the dagger's whole reason to stand
/// behind you. It multiplies the same height bonus the crit roll does, so the
/// floating-point version was inflating every backstab's landing rate too.
#[test]
fn blow_success_matches_java_across_the_grid() {
    let mut cases = 0usize;
    for &weapon_critical in &[0.0, 4.0, 10.0, 80.0, 500.0] {
        for &(position, _) in &positions() {
            for &position_mul in &[1.0, 0.7, 1.6] {
                for &z in Z_DIFFS {
                    for &chance_boost in &[0.0, 50.0, 100.0] {
                        for &blow_rate_mod in &[1.0, 1.03, 1.3] {
                            for &limit in &[80.0, 100.0] {
                                for &roll in ROLLS_100 {
                                    let ours = formulas::calc_blow_success(
                                        weapon_critical,
                                        position,
                                        position_mul,
                                        z,
                                        0,
                                        chance_boost,
                                        blow_rate_mod,
                                        limit,
                                        roll,
                                    );
                                    let theirs = java::blow_success(
                                        weapon_critical,
                                        position,
                                        position_mul,
                                        z,
                                        0,
                                        chance_boost,
                                        blow_rate_mod,
                                        // `BLOW_RATE_DEFENCE` has no carrier.
                                        1.0,
                                        limit,
                                        roll,
                                    );
                                    assert_eq!(
                                        ours, theirs,
                                        "blow success diverged — crit {weapon_critical}, \
                                         {position:?} x{position_mul}, z {z}, boost \
                                         {chance_boost}, mod {blow_rate_mod}, limit {limit}, \
                                         roll {roll}"
                                    );
                                    cases += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 10_000, "the grid collapsed to {cases} cases");
}

/// **The cast-break sweep** (`calcAtkBreak`). The gates — channelling, the
/// `DC_MOD` abnormal, raids, HP-blocked targets and the two `ALT_GAME_CANCEL_*`
/// switches — are the port's `applies` flag; everything after them is swept.
#[test]
fn atk_break_matches_java_across_the_grid() {
    let mut cases = 0usize;
    for &dmg in &[0.0, 1.0, 50.0, 500.0, 5_000.0, 50_000.0] {
        for &men_bonus in &[0.7, 1.0, 1.18, 1.5] {
            for &cancel_mul in &[1.0, 0.5, 1.4] {
                for &cancel_add in &[0.0, -20.0, 30.0] {
                    for &roll in ROLLS_100 {
                        let ours = formulas::calc_atk_break(
                            dmg, men_bonus, roll, cancel_add, cancel_mul, true,
                        );
                        let theirs = java::atk_break(dmg, men_bonus, cancel_mul, cancel_add, roll);
                        assert_eq!(
                            ours, theirs,
                            "cast break diverged — dmg {dmg}, men {men_bonus}, cancel \
                             x{cancel_mul}+{cancel_add}, roll {roll}"
                        );
                        cases += 1;
                    }
                }
            }
        }
        // The gates are the caller's, and a closed gate is an unconditional no.
        assert!(!formulas::calc_atk_break(dmg, 1.0, 0, 0.0, 1.0, false));
    }
    assert!(cases > 1_000, "the grid collapsed to {cases} cases");
}

/// **The pvp/pve bonus sweep**. Java floors the whole product at 0.05 —
/// "Bonus should not be negative" — and the port applies that floor at its two
/// call sites rather than inside the formula, so the sweep applies it there
/// too. A defence stat one point past the attack stat is what reaches it.
#[test]
fn pvp_pve_bonus_matches_java_across_the_grid() {
    let mut cases = 0usize;
    let mut floored = 0usize;
    for &attack_mul in &[1.0, 1.15, 0.8] {
        for &defence_mul in &[1.0, 1.15, 2.5] {
            for &raid_attack_mul in &[1.0, 1.2] {
                for &raid_defence_mul in &[1.0, 1.3] {
                    for &penalty in &[1.0, 0.7, 0.35, 0.05] {
                        let ours = formulas::calculate_pvp_pve_bonus(
                            attack_mul,
                            defence_mul,
                            raid_attack_mul,
                            raid_defence_mul,
                            penalty,
                        )
                        .max(0.05);
                        let theirs = java::pvp_pve_bonus(
                            attack_mul,
                            defence_mul,
                            raid_attack_mul,
                            raid_defence_mul,
                            penalty,
                        );
                        assert!(
                            (ours - theirs).abs() < 1e-9,
                            "pvp/pve bonus diverged — atk {attack_mul}x{raid_attack_mul}, def \
                             {defence_mul}x{raid_defence_mul}, penalty {penalty}: {ours} vs \
                             {theirs}"
                        );
                        floored += usize::from((theirs - 0.05).abs() < 1e-9);
                        cases += 1;
                    }
                }
            }
        }
    }
    assert!(cases > 100, "the grid collapsed to {cases} cases");
    assert!(floored > 0, "no row in the grid reached the 0.05 floor");
}

/// **The mana-drain sweep** (`calcMagicAffected`). The gaussian is the caller's
/// roll, so the sweep walks it either side of the tipping point instead.
#[test]
fn magic_affected_matches_java_across_the_grid() {
    let mut cases = 0usize;
    for &m_atk in &[1.0, 50.0, 300.0, 2_000.0] {
        for &defence in &[0.0, 50.0, 300.0, 2_000.0] {
            for &gaussian in &[-4.0, -1.0, -0.25, 0.0, 0.25, 1.0, 4.0] {
                let ours = formulas::calc_magic_affected(m_atk, defence, gaussian);
                // No `MagicalAttackMp` carrier declares a `<trait>`, so Java's
                // `calcGeneralTraitBonus` returns 1 for every drain here.
                let theirs = java::magic_affected(m_atk, defence, 1.0, gaussian);
                assert_eq!(
                    ours, theirs,
                    "mana drain diverged — mAtk {m_atk}, mDef {defence}, gaussian {gaussian}"
                );
                cases += 1;
            }
        }
    }
    // Java divides by zero when both sides are 0 and gets NaN, which compares
    // false; the port guards explicitly and returns the same answer.
    assert!(!formulas::calc_magic_affected(0.0, 0.0, 5.0));
    assert!(cases > 50, "the grid collapsed to {cases} cases");
}

/// `getRandomDamageMultiplier` — one line, swept for the sake of the census
/// column rather than out of suspicion.
#[test]
fn random_damage_multiplier_matches_java_across_the_band() {
    for roll in -30..=30 {
        let ours = formulas::random_damage_multiplier(roll);
        let theirs = java::random_damage_multiplier(roll);
        assert!(
            (ours - theirs).abs() < 1e-12,
            "random damage diverged at {roll}: {ours} vs {theirs}"
        );
    }
}
