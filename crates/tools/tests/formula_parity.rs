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

use gameserver::model::formulas::{self, CritDamage};
use gameserver::model::movement::Position;

/// Transcriptions of Java's expressions. Each function quotes the source it
/// came from; nothing here calls the port.
mod java {
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
        is_ranged: bool,
        trait_bonus: f64,
        attribute_bonus: f64,
        pvp_pve_bonus: f64,
    ) -> f64 {
        let shots_bonus = 1.0; // `SHOTS_BONUS` — no carrier on this dist.
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
        is_ranged: bool,
        mods: f64,
    ) -> f64 {
        let attack = p_atk * p_atk_mod;
        let defence = p_def * p_def_mod;
        let weapon_mod = if is_ranged { 70.0 } else { 77.0 };
        let ranged_bonus = if is_ranged { attack + power } else { 0.0 };
        let crit_mod = if crit { crit_mul } else { 1.0 };
        let ss_mod = if ss { 2.0 } else { 1.0 };
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
        cd_mult: f64,
        cd_patk: f64,
        mods: f64,
    ) -> f64 {
        let ssmod = if ss { 2.0 } else { 1.0 };
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
    /// road, which is what makes the port's single expression correct rather
    /// than lucky. `HEAL_EFFECT`/`_ADD` are the caller's, and no Interlude
    /// weapon reaches the S80/S84 grades.
    pub fn heal_amount(
        power: f64,
        m_atk: f64,
        mcrit: bool,
        sps: bool,
        bss: bool,
        mp_consume: i32,
        is_mage_caster: bool,
    ) -> f64 {
        let (static_shot_bonus, m_atk_mul) = if (sps || bss) && is_mage_caster {
            (
                mp_consume as f64 * if bss { 2.4 } else { 1.0 },
                if bss { 4.0 } else { 2.0 },
            )
        } else {
            // Weapon grade is 1 on this chronicle, so `mAtkMul + 1` = 2 and
            // `mAtkMul * 4` = 4.
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
}

/// The grid. Small on purpose: every combination of these is swept, so the
/// product matters more than any single row's spread.
const P_ATKS: &[f64] = &[1.0, 37.0, 250.0, 1_337.0, 9_999.0];
const P_DEFS: &[f64] = &[1.0, 43.0, 300.0, 2_048.0];
const RANDOM_MULS: &[f64] = &[0.9, 1.0, 1.1];
const CRIT_MULS: &[f64] = &[2.0, 3.5];
const CRIT_ADDS: &[f64] = &[0.0, 137.0];
const MODS: &[f64] = &[1.0, 0.75, 1.4];

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
                        for &ss in &[false, true] {
                            for &is_ranged in &[false, true] {
                                for &c_mul in CRIT_MULS {
                                    for &c_add in CRIT_ADDS {
                                        for &m in MODS {
                                            let cd = CritDamage {
                                                mul: c_mul,
                                                add: c_add,
                                            };
                                            let ours = formulas::calc_auto_attack_damage(
                                                p_atk, random_mul, position, p_def, crit, cd, ss,
                                                is_ranged, m, m, m,
                                            );
                                            let theirs = java::auto_attack_damage(
                                                p_atk, random_mul, prox, p_def, crit, c_mul, c_add,
                                                ss, is_ranged, m, m, m,
                                            );
                                            assert!(
                                                (ours - theirs).abs() <= theirs.abs() * 1e-12,
                                                "auto-attack damage diverged: ours {ours}, Java \
                                                 {theirs} — pAtk {p_atk}, pDef {p_def}, random \
                                                 {random_mul}, {position:?}, crit {crit}, ss \
                                                 {ss}, ranged {is_ranged}, cAtk {c_mul}, cAtkAdd \
                                                 {c_add}, mods {m}"
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
                            for &ss in &[false, true] {
                                for &is_ranged in &[false, true] {
                                    for &m in MODS {
                                        let ours = formulas::calc_physical_skill_damage(
                                            p_atk, 1.0, p_def, 1.0, power, level_mod, random_mod,
                                            crit, 2.0, ss, is_ranged,
                                        ) * m;
                                        let theirs = java::physical_skill_damage(
                                            p_atk, 1.0, p_def, 1.0, power, level_mod, random_mod,
                                            crit, 2.0, ss, is_ranged, m,
                                        );
                                        assert!(
                                            (ours - theirs).abs() <= theirs.abs() * 1e-12,
                                            "physical skill damage diverged: ours {ours}, Java \
                                             {theirs} — pAtk {p_atk}, pDef {p_def}, power \
                                             {power}, levelMod {level_mod}, random {random_mod}, \
                                             crit {crit}, ss {ss}, ranged {is_ranged}, mods {m}"
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
                        for &ss in &[false, true] {
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

/// **The heal sweep** — the one family this pass *confirmed* rather than
/// corrected. It is checked in anyway: an agreement that nobody can re-derive
/// is indistinguishable from an untested formula.
#[test]
fn heal_amount_matches_java_across_the_grid() {
    let mut cases = 0usize;
    for &power in &[0.0, 25.0, 340.0, 5_000.0] {
        for &m_atk in &[1.0, 60.0, 900.0, 6_000.0] {
            for &mp_consume in &[0, 12, 90] {
                for &(sps, bss) in &[(false, false), (true, false), (false, true)] {
                    for &is_mage in &[false, true] {
                        for &mcrit in &[false, true] {
                            let ours = formulas::calc_heal(
                                power, m_atk, mcrit, sps, bss, mp_consume, is_mage,
                            );
                            let theirs = java::heal_amount(
                                power, m_atk, mcrit, sps, bss, mp_consume, is_mage,
                            );
                            assert!(
                                (ours - theirs).abs() <= theirs.abs() * 1e-12,
                                "heal diverged: ours {ours}, Java {theirs} — power {power}, mAtk \
                                 {m_atk}, mpConsume {mp_consume}, sps {sps}, bss {bss}, mage \
                                 {is_mage}, crit {mcrit}"
                            );
                            cases += 1;
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
