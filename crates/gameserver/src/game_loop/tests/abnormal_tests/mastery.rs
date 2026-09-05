//! Skill mastery: the cooldown it collapses, the duration it doubles, and
//! the continuous chance it rolls.

use super::*;

/// G34 S4 sub-slice 6 — `SkillMastery` (330 STR / 331 INT) + `SkillMasteryRate`
/// (Focus Skill Mastery 334): a chance for a cast's cooldown to collapse to
/// 100 ms, announced with "A skill is ready to be used again".
///
/// The stat stores the **`BaseStat` ordinal**, not a magnitude, and Java's enum
/// order (`STR, INT, DEX, …`) differs from this port's (`Str, Dex, Con, Int, …`)
/// — copying Java's number across would make Skill Mastery 331 read DEX instead
/// of INT. Asserted by driving both stats.
#[test]
fn skill_mastery_collapses_the_cooldown_and_reads_the_right_base_stat() {
    use crate::model::components::stats::{BaseStats, StatModifiers};
    use crate::model::stats::{BaseStat, Stat};
    let (mut world, _db, _l) = cc2_world();
    // The **real** `statBonus` table: `GameData::for_test`'s stub returns 1.0
    // for every stat, which makes "which BaseStat was selected" unobservable —
    // exactly the property under test. One dist load, reused for all four
    // measurements below.
    world.data.stat_bonus = crate::data::StatBonus::load_from(crate::data::DIST_GAME);
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    // A lopsided stat spread, so "which BaseStat" is observable: huge INT,
    // minimal DEX.
    if let Some(b) = world.objects.get_component_mut::<BaseStats>(&CASTER) {
        b.int_ = 99;
        b.dex = 1;
    }

    // Derive the discriminating roll from the real bonus table rather than
    // guessing a threshold: pick one strictly between the two chances, so the
    // assertion can only pass if the *stat selection* is right.
    const RATE: f64 = 10.0;
    let int_chance = world.data.stat_bonus.bonus(BaseStat::Int, 99) * RATE;
    let dex_chance = world.data.stat_bonus.bonus(BaseStat::Dex, 1) * RATE;
    assert!(
        int_chance > dex_chance + 2.0,
        "the fixture has to separate the two stats: INT {int_chance}, DEX {dex_chance}"
    );
    // `calcSkillMastery` draws `Rnd.nextDouble() * 100`, which the port spells
    // `roll_f64() * 100` — and `roll_f64` quantizes a forced value as
    // `v / 1_000_000`, so a forced `v` reads as the percentage `v / 10_000`.
    // Forcing the *midpoint* of the two chances therefore needs that scale, and
    // gets to keep the fraction the old `as i32` was throwing away.
    let roll = (((int_chance + dex_chance) / 2.0) * 10_000.0) as i32;

    let mastery_fires = |world: &mut World, stat: BaseStat| {
        let mut mods = world
            .objects
            .get_component::<StatModifiers>(&CASTER)
            .cloned()
            .unwrap_or_default();
        mods.add.insert(Stat::SkillMastery, stat as i32 as f64);
        mods.mul.insert(Stat::SkillMasteryRate, RATE);
        world.objects.add_components(&CASTER, mods);
        world.clear_forced_rolls();
        world.force_roll(roll);
        effects::calc_skill_mastery(world, CASTER)
    };

    assert!(
        mastery_fires(&mut world, BaseStat::Int),
        "INT 99 clears a roll of {roll}"
    );
    assert!(
        !mastery_fires(&mut world, BaseStat::Dex),
        "DEX 1 does not — so the ordinal really selects the stat"
    );

    // With no `SKILL_MASTERY` at all there is no proc, whatever the rate.
    let mut mods = world
        .objects
        .get_component::<StatModifiers>(&CASTER)
        .cloned()
        .unwrap();
    mods.add.remove(&Stat::SkillMastery);
    world.objects.add_components(&CASTER, mods);
    world.clear_forced_rolls();
    world.force_roll(0);
    assert!(
        !effects::calc_skill_mastery(&mut world, CASTER),
        "Java's `getAdd(SKILL_MASTERY, -1) == -1` bail"
    );
}

/// `calcEffectSuccess` is gated on **`activateRate != -1` alone**, not on
/// `isBad()`:
///
/// ```java
/// // Skill.applyEffects
/// addContinuousEffects = !passive && (isToggle() || (isContinuous() && Formulas.calcEffectSuccess(effector, effected, this)));
/// // Formulas.calcEffectSuccess
/// if (activateRate == -1) return true;
/// ```
///
/// Three learnable skills on this dist sit in the gap an `isBad()` gate opens,
/// and the first assertion pins them **off the real dist** so the fixture below
/// can't drift away from what it is modelling.
#[test]
fn a_continuous_skill_rolls_to_land_even_when_its_effect_point_is_not_negative() {
    // Veil is a mesmerize (`isDebuff`, trait DERANGEMENT) that declares no
    // `<effectPoint>` at all; the two heals declare a positive one. All three
    // carry an `activateRate`, so all three roll in Java.
    let skills = dist::skills();
    for (id, rate) in [(106, 70), (1217, 0), (1219, 0)] {
        let skill = skills
            .get(id, 1)
            .unwrap_or_else(|| panic!("skill {id} on the dist"));
        assert_eq!(
            skill.activate_rate, rate,
            "skill {id} carries an activateRate"
        );
        assert!(
            !skill.is_bad(),
            "skill {id}'s effectPoint is not negative — an `isBad()` gate would skip its roll"
        );
    }

    const TARGET: i32 = 4713;
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _t = ingame_caster(&mut world, 2, TARGET, 0, 0);

    // Veil's shape: activateRate 70, no lvlBonusRate, effectPoint absent (0).
    let mut veil = cc_skill(106, SkillEffect::Passive, "TURN_PASSIVE");
    veil.effect_point = 0;
    veil.is_debuff = true;
    veil.activate_rate = 70;
    veil.lvl_bonus_rate = 0;
    veil.magic_level = 40;
    veil.abnormal_time = 120;
    world.data.skill_data.insert_for_test(veil.clone());

    // `baseMod = (magicLevel - targetLevel + 3) * 0 + 70 + 30 = 100`, clamped to
    // the config ceiling of 90. Java resists on `finalRate <= Rnd.get(100)`.
    world.clear_forced_rolls();
    world.force_roll(89);
    assert!(
        effects::apply_continuous_effects(&mut world, CASTER, TARGET, &veil, None),
        "89 < 90 — it lands"
    );
    world.clear_forced_rolls();
    world.force_roll(90);
    assert!(
        !effects::apply_continuous_effects(&mut world, CASTER, TARGET, &veil, None),
        "90 is not below 90 — resisted, which an `isBad()` gate would never allow"
    );

    // The `-1` sentinel still short-circuits, and consumes no roll.
    let mut always = veil.clone();
    always.activate_rate = -1;
    world.clear_forced_rolls();
    world.force_roll(0);
    assert!(
        effects::apply_continuous_effects(&mut world, CASTER, TARGET, &always, None),
        "`activateRate == -1` returns true before any roll"
    );
}

/// `Heal.instant` asks **`isPlayer() && isMageClass()`**, not "is it a player":
///
/// ```java
/// if (((sps || bss) && (effector.isPlayer() && effector.getActingPlayer().isMageClass())) || effector.isSummon())
/// {
///     staticShotBonus = skill.getMpConsume();   // ← the mage arm's whole point
///     mAtkMul = bss ? 4 * shotsBonus : 2 * shotsBonus;
/// }
/// ```
///
/// A **fighter** with a spiritshot charged falls through to the grade arm and
/// gets no static bonus at all. The port had stood `isPlayer()` in for the
/// class test, which handed every fighter the mage's `mpConsume` bonus.
///
/// `MAGE_GROUP` is `ClassId.isMage()` exactly for every id this chronicle can
/// reach — the two sets differ only at ids ≥ 143 (Ertheia and the awakened
/// classes), which no character here holds.
#[test]
fn only_a_mage_class_gets_the_spiritshot_heal_bonus() {
    const MP_CONSUME: i32 = 200;
    // 15 = cleric, 1 = warrior — one on each side of `MAGE_GROUP`.
    const CLERIC: i32 = 15;
    const WARRIOR: i32 = 1;

    let (mut world, _db, _l) = cc2_world();
    // The **real** `CategoryData.xml`: the claim under test is that its
    // `MAGE_GROUP` is Java's per-`ClassId` `isMage` flag, so a stub category
    // would assert nothing.
    world.data.categories = crate::data::CategoryData::load_from(crate::data::DIST_GAME);
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    assert!(
        world.data.categories.contains("MAGE_GROUP", CLERIC)
            && !world.data.categories.contains("MAGE_GROUP", WARRIOR),
        "the fixture's two class ids have to straddle the category"
    );

    let mut heal = cc_skill(9393, SkillEffect::Heal { power: 10.0 }, "NONE");
    heal.effect_point = 100;
    heal.is_debuff = false;
    heal.magic_type = 1;
    heal.mp_consume = MP_CONSUME;
    world.data.skill_data.insert_for_test(heal);

    let healed_as = |world: &mut World, class_id: i32| -> f64 {
        if let Some(p) = world.objects.get_component_mut::<model::Player>(&CASTER) {
            p.class_id = class_id;
            p.charge_shot(crate::model::ShotType::Spiritshots);
        }
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&CASTER) {
            v.max_hp = 100_000;
            v.cur_hp = 1.0;
        }
        land(world, 9393, CASTER);
        world
            .objects
            .get_component::<Vitals>(&CASTER)
            .map(|v| v.cur_hp - 1.0)
            .unwrap_or(0.0)
    };

    let mage = healed_as(&mut world, CLERIC);
    let fighter = healed_as(&mut world, WARRIOR);
    // Both arms reach `mAtkMul = 2` (the grade arm's `1 + 1`, the mage arm's
    // `2 · shotsBonus` with an unenchanted weapon), so the sqrt terms cancel and
    // the whole difference is the static bonus.
    assert!(
        (mage - fighter - MP_CONSUME as f64).abs() < 1e-6,
        "the mage's spiritshot is worth exactly the skill's mpConsume more \
         ({mage} vs {fighter})"
    );
}

/// `calcSkillMastery` draws a **continuous** value, not a 0-99 integer:
///
/// ```java
/// final double chance = BaseStat.values()[val].calcBonus(actor) * actor.getStat().getMul(Stat.SKILL_MASTERY_RATE, 1);
/// return ((Rnd.nextDouble() * 100.) < (chance * Config.SKILL_MASTERY_CHANCE_MULTIPLIERS[…]));
/// ```
///
/// `roll(100) < chance` — the shape the port used — rounds every fractional
/// chance **up**, because there is no integer strictly between 30 and 31 to
/// lose on a 30.5. And fractions are the normal case here: the chance is a
/// base-stat *bonus* off a per-point curve, times a rate multiplier.
///
/// The fixture picks 30.5 % and rolls 30.4, the one draw the two shapes
/// disagree about.
#[test]
fn skill_mastery_draws_a_continuous_chance_not_a_whole_percent() {
    use crate::model::components::stats::StatModifiers;
    use crate::model::stats::{BaseStat, Stat};

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    // `cc2_world`'s stat-bonus table answers 1.0 for everything, so the rate
    // *is* the chance — 30.5 %.
    let mut mods = world
        .objects
        .get_component::<StatModifiers>(&CASTER)
        .cloned()
        .unwrap_or_default();
    mods.add
        .insert(Stat::SkillMastery, BaseStat::Int as i32 as f64);
    mods.mul.insert(Stat::SkillMasteryRate, 30.5);
    world.objects.add_components(&CASTER, mods);

    // A forced roll reads as `v / 10_000` percent (`roll_f64` quantizes by
    // 1e-6, and the formula scales by 100).
    for (forced, expected, why) in [
        (304_000, true, "30.4 % is below the 30.5 % chance"),
        (306_000, false, "30.6 % is above it"),
    ] {
        world.clear_forced_rolls();
        world.force_roll(forced);
        assert_eq!(
            effects::calc_skill_mastery(&mut world, CASTER),
            expected,
            "{why} — an integer roll could not tell these apart"
        );
    }
}

/// `Formulas.calcEffectAbnormalTime` — a **Skill Mastery proc doubles a buff's
/// duration**, and does so on a roll entirely separate from the one that
/// collapses the cooldown.
///
/// ```java
/// // BuffInfo(…) constructor
/// _abnormalTime = Formulas.calcEffectAbnormalTime(effector, effected, skill);
/// // Formulas
/// int time = … skill.getAbnormalTime();
/// if (!skill.isStatic() && calcSkillMastery(caster, skill)) time *= 2;
/// ```
///
/// The cooldown proc (`apply_reuse`) is gated to `operateType A1`, which
/// excludes every buff; this one is gated only on `isStatic()`. That difference
/// is the mechanic: an Eva's Saint who learns Skill Mastery 331 at 77 rolls it
/// on each buff they land and sometimes gets twice the duration.
#[test]
fn skill_mastery_doubles_a_buffs_duration() {
    use crate::model::components::stats::StatModifiers;
    use crate::model::skill::effects::StatModifierEffect;
    use crate::model::stats::{BaseStat, Stat, StatModifierType};

    const TARGET: i32 = 4711;
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _t = ingame_caster(&mut world, 2, TARGET, 0, 0);

    let skill = Skill {
        id: 1085,
        level: 1,
        abnormal_type: "MAGIC_ATTACK_UP".into(),
        abnormal_time: 1200,
        // Not `isStatic()` — `magicType == 2` is what would exempt it.
        magic_type: 1,
        effects: vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::MagicalAttack,
            mode: StatModifierType::Diff,
            amount: 25.0,
            ..Default::default()
        })],
        ..Default::default()
    };

    world.data.skill_data.insert_for_test(skill.clone());

    let duration = |world: &mut World| {
        // Re-applying the same abnormal type replaces the live buff in place,
        // so the freshest entry is always the one just landed.
        assert!(
            effects::apply_continuous_effects(world, CASTER, TARGET, &skill, None),
            "the buff has to land for the duration to mean anything"
        );
        let start = world.tick;
        world
            .objects
            .get_component::<Buffs>(&TARGET)
            .and_then(|b| b.0.last().map(|x| x.expires_at_tick - start))
            .expect("the buff landed")
    };

    // No Skill Mastery stat at all → `getAdd(SKILL_MASTERY, -1) == -1` bails
    // before any roll, so this is the plain 1200 s.
    assert_eq!(duration(&mut world), 12_000, "1200 s at 10 ticks/s");

    // Give the caster mastery off INT, with a rate that makes the proc certain
    // for a roll of 0 and impossible for a roll of 99.
    let mut mods = world
        .objects
        .get_component::<StatModifiers>(&CASTER)
        .cloned()
        .unwrap_or_default();
    mods.add
        .insert(Stat::SkillMastery, BaseStat::Int as i32 as f64);
    mods.mul.insert(Stat::SkillMasteryRate, 50.0);
    world.objects.add_components(&CASTER, mods);

    // Forced rolls read as `v / 10_000` percent (see `calcSkillMastery`), so
    // 90 % loses against the fixture's 50 % chance and 0 % wins.
    world.clear_forced_rolls();
    world.force_roll(900_000);
    assert_eq!(
        duration(&mut world),
        12_000,
        "a losing mastery roll leaves the duration alone"
    );

    world.clear_forced_rolls();
    world.force_roll(0);
    assert_eq!(
        duration(&mut world),
        24_000,
        "the proc doubles it — 1200 s becomes 2400 s"
    );

    // A **static** skill is exempt in Java even on a proc.
    let static_skill = Skill {
        magic_type: 2,
        ..skill.clone()
    };
    world.data.skill_data.insert_for_test(static_skill.clone());
    world.clear_forced_rolls();
    world.force_roll(0);
    effects::apply_continuous_effects(&mut world, CASTER, TARGET, &static_skill, None);
    let start = world.tick;
    assert_eq!(
        world
            .objects
            .get_component::<Buffs>(&TARGET)
            .and_then(|b| b.0.last().map(|x| x.expires_at_tick - start))
            .expect("the buff landed"),
        12_000,
        "`isStatic()` skips the doubling"
    );
}
