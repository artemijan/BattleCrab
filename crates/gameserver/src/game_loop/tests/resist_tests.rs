//! Abnormal resistance, blocking and probabilistic dispel (G19).

use super::*;
use crate::game_loop::abnormal::has_buff;
use crate::game_loop::helpers::stat_mul;
use crate::game_loop::skills::effects::handle_buff_expire;
use crate::model::skill::{
    AffectObject, AffectScope, OperateType, Skill, SkillEffect, StatModifierEffect, TargetType,
};
use crate::model::stats::{Stat, StatModifierType};

/// A stand-in attacker with no `AttackTraits` of its own — these tests are
/// about the *defence* side, and Java's `getAttackTrait` identity is 1.0.
const ATTACKER_NO_TRAITS: i32 = 2999;
const CASTER: i32 = 2001;
const VICTIM: i32 = 2002;
const CID: u32 = 1;
const VICTIM_CID: u32 = 2;

fn base_skill(id: i32, effects: Vec<SkillEffect>) -> Skill {
    Skill {
        self_continuous: false,
        without_action: false,
        trait_type: crate::model::skill::TraitType::None,
        item_consume_id: 0,
        item_consume_count: 0,
        id,
        level: 1,
        name: format!("T{id}"),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Enemy,
        magic_type: 1,
        magic_level: 40,
        effect_point: -100,
        cast_range: 900,
        effect_range: 1000,
        hit_time: 100,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 20,
        abnormal_level: 1,
        abnormal_type: "NONE".into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects,
        ..Default::default()
    }
}
// ---------------------------------------------------------------------------
// Debuff resistance
// ---------------------------------------------------------------------------

/// `ResistAbnormalByCategory` pumps a **multiplier** on incoming debuff chance:
/// a negative amount makes you resistant, a positive one vulnerable. This is
/// the parse-level guarantee that the PER mode is forced (a DIFF read would
/// make Guts' `-50` mean "-50 percentage points" instead of "×0.5").
#[test]
fn resist_buff_pumps_a_multiplier() {
    let (mut world, _db, _l) = cast_test_world();
    let resist = base_skill(
        9400,
        vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::ResistAbnormalDebuff,
            mode: StatModifierType::Per,
            amount: -50.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
        })],
    );
    world.data.skill_data.insert_for_test(resist);
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    land_skill_on_target(&mut world, 9400, CASTER);
    let mul = stat_mul(&world, CASTER, Stat::ResistAbnormalDebuff);
    assert!((mul - 0.5).abs() < 1e-9, "-50 PER → x0.5, got {mul}");
}

/// The multiplier reaches the landing formula: the same debuff is half as
/// likely to land on a resistant target, and the clamp is applied *after* the
/// multiply (Java's `constrain(baseMod * buffDebuffMod, min, max)`).
#[test]
fn resist_multiplier_lowers_the_landing_rate() {
    use crate::model::formulas::calc_effect_land_rate;

    // magic_level 40 vs target level 40, activate_rate 50, lvl_bonus 0
    // → base_mod = 3*0 + 50 + 30 = 80.
    let unresisted = calc_effect_land_rate(40, 50, 0, 40, 1.0, 1.0, 1.0, 0.0, 1.0);
    assert!((unresisted - 80.0).abs() < 1e-9, "got {unresisted}");

    // Guts (x0.5): 80 * 0.5 = 40.
    let resisted = calc_effect_land_rate(40, 50, 0, 40, 0.5, 1.0, 1.0, 0.0, 1.0);
    assert!((resisted - 40.0).abs() < 1e-9, "got {resisted}");

    // Touch of Death (x1.3): 80 * 1.3 = 104, clamped down to the 90 ceiling.
    let vulnerable = calc_effect_land_rate(40, 50, 0, 40, 1.3, 1.0, 1.0, 0.0, 1.0);
    assert!(
        (vulnerable - 90.0).abs() < 1e-9,
        "clamped after the multiply, got {vulnerable}"
    );

    // The 10 floor still holds under a crushing resistance.
    let crushed = calc_effect_land_rate(40, 50, 0, 40, 0.01, 1.0, 1.0, 0.0, 1.0);
    assert!((crushed - 10.0).abs() < 1e-9, "got {crushed}");

    // An always-lands debuff (`activate_rate == -1`) ignores resistance
    // entirely, as in Java (the early return precedes the whole formula).
    assert_eq!(
        calc_effect_land_rate(40, -1, 0, 40, 0.01, 1.0, 1.0, 0.0, 1.0),
        100.0
    );
}

// ---------------------------------------------------------------------------
// BlockAbnormalSlot
// ---------------------------------------------------------------------------

/// A live `BlockAbnormalSlot` refuses any buff of a blocked abnormal type —
/// the mechanic behind the Prophecies being mutually exclusive — while leaving
/// everything else alone.
#[test]
fn blocked_abnormal_types_cannot_land() {
    let (mut world, _db, _l) = cast_test_world();

    let mut blocker = base_skill(
        9410,
        vec![SkillEffect::BlockAbnormalSlot {
            slots: vec!["BUFF_SPECIAL_ATTACK".into()],
        }],
    );
    blocker.abnormal_type = "PROPHECY".into();
    world.data.skill_data.insert_for_test(blocker);

    // A buff of the blocked type, and one of a different type.
    let stat = |id: i32, abnormal: &str| {
        let mut s = base_skill(
            id,
            vec![SkillEffect::StatModifier(StatModifierEffect {
                stat: Stat::PhysicalAttack,
                mode: StatModifierType::Per,
                amount: 10.0,
                armor_condition: 0,
                weapon_condition: 0,
                qualifier: None,
                two_handed: false,
            })],
        );
        s.abnormal_type = abnormal.into();
        s
    };
    world
        .data
        .skill_data
        .insert_for_test(stat(9411, "BUFF_SPECIAL_ATTACK"));
    world
        .data
        .skill_data
        .insert_for_test(stat(9412, "SOMETHING_ELSE"));

    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    // Baseline: with no blocker up, the buff lands.
    land_skill_on_target(&mut world, 9411, CASTER);
    assert!(
        has_buff(&world, CASTER, 9411),
        "lands freely when nothing blocks it"
    );
    handle_buff_expire(&mut world, CASTER, 9411);

    // With the blocker up it is refused, while the unrelated buff still lands.
    land_skill_on_target(&mut world, 9410, CASTER);
    assert!(has_buff(&world, CASTER, 9410), "the blocker itself lands");
    land_skill_on_target(&mut world, 9411, CASTER);
    assert!(
        !has_buff(&world, CASTER, 9411),
        "a blocked abnormal type is refused"
    );
    land_skill_on_target(&mut world, 9412, CASTER);
    assert!(
        has_buff(&world, CASTER, 9412),
        "an unblocked type is unaffected"
    );

    // Once the blocker goes, the previously blocked buff lands again.
    handle_buff_expire(&mut world, CASTER, 9410);
    land_skill_on_target(&mut world, 9411, CASTER);
    assert!(
        has_buff(&world, CASTER, 9411),
        "blocking ends with the buff"
    );
}

// ---------------------------------------------------------------------------
// DispelBySlotProbability
// ---------------------------------------------------------------------------

fn seed_dispel_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = cast_test_world();
    // Two dispellable buffs plus one that is not on the list. The two must
    // carry *distinct* abnormal types — same-type buffs replace each other via
    // the abnormal stacking rules, which would mask what the dispel did.
    // Both types below are on the real Warrior Bane list.
    for (id, abnormal) in [
        (9421, "SPEED_UP"),
        (9422, "IMPROVE_SPEED_AVOID_UP"),
        (9423, "UNRELATED"),
    ] {
        let mut s = base_skill(
            id,
            vec![SkillEffect::StatModifier(StatModifierEffect {
                stat: Stat::PhysicalAttack,
                mode: StatModifierType::Per,
                amount: 5.0,
                armor_condition: 0,
                weapon_condition: 0,
                qualifier: None,
                two_handed: false,
            })],
        );
        s.abnormal_type = abnormal.into();
        world.data.skill_data.insert_for_test(s);
    }
    (world, db, l)
}

/// At `rate = 100` the Bane strips every matching buff and nothing else.
#[test]
fn certain_dispel_strips_every_matching_buff() {
    let (mut world, _db, _l) = seed_dispel_world();
    world.data.skill_data.insert_for_test(base_skill(
        9420,
        vec![SkillEffect::DispelBySlotProbability {
            dispel: vec!["SPEED_UP".into(), "IMPROVE_SPEED_AVOID_UP".into()],
            rate: 100,
        }],
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    for id in [9421, 9422, 9423] {
        land_skill_on_target(&mut world, id, VICTIM);
        assert!(has_buff(&world, VICTIM, id));
    }

    land_skill_on_target(&mut world, 9420, VICTIM);
    assert!(!has_buff(&world, VICTIM, 9421), "matching buff stripped");
    assert!(
        !has_buff(&world, VICTIM, 9422),
        "both matching buffs stripped"
    );
    assert!(
        has_buff(&world, VICTIM, 9423),
        "an unlisted abnormal type survives"
    );
}

/// At `rate = 0` nothing is stripped — proving the roll is actually consulted
/// rather than the dispel being unconditional.
#[test]
fn zero_rate_dispel_strips_nothing() {
    let (mut world, _db, _l) = seed_dispel_world();
    world.data.skill_data.insert_for_test(base_skill(
        9420,
        vec![SkillEffect::DispelBySlotProbability {
            dispel: vec!["SPEED_UP".into(), "IMPROVE_SPEED_AVOID_UP".into()],
            rate: 0,
        }],
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    for id in [9421, 9422] {
        land_skill_on_target(&mut world, id, VICTIM);
    }
    land_skill_on_target(&mut world, 9420, VICTIM);
    assert!(has_buff(&world, VICTIM, 9421), "a 0% Bane strips nothing");
    assert!(has_buff(&world, VICTIM, 9422));
}

// ---------------------------------------------------------------------------
// Trait resistances (G16 sweep) — `DefenceTrait` vs a debuff's `<trait>`
// ---------------------------------------------------------------------------

/// **Stun Resistance actually resists stuns now.** The dist pairs a debuff's
/// `<trait>` (304 skills declare `SHOCK`) with `DefenceTrait`'s per-trait
/// percentages; before this the buff landed icon-only and changed nothing.
///
/// Levels 1..4 of Stun Resistance (1259) grant 15/20/30/40 %, and the bonus is
/// Java's `max(1.0 - defence, 0.05)` multiplier on the landing chance.
#[test]
fn stun_resistance_lowers_a_shock_debuffs_land_rate() {
    use crate::game_loop::skills::effects::{
        calc_general_trait_bonus, merge_defence_traits, remove_defence_traits,
    };
    use crate::model::skill::TraitType;

    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    // No buff → no resistance.
    assert_eq!(
        calc_general_trait_bonus(&world, ATTACKER_NO_TRAITS, 3001, TraitType::Shock, false),
        1.0,
        "an unprotected target resists nothing"
    );

    // Stun Resistance level 3 = 30 %.
    let traits = [(TraitType::Shock, 0.30)];
    merge_defence_traits(&mut world, 3001, &traits);
    assert!(
        (calc_general_trait_bonus(&world, ATTACKER_NO_TRAITS, 3001, TraitType::Shock, false)
            - 0.70)
            .abs()
            < 1e-9,
        "a SHOCK debuff lands at 70% of its chance"
    );
    // …and only against that trait.
    assert_eq!(
        calc_general_trait_bonus(&world, ATTACKER_NO_TRAITS, 3001, TraitType::Sleep, false),
        1.0,
        "resisting stuns does nothing against sleep"
    );

    // The resistance goes when the buff does.
    remove_defence_traits(&mut world, 3001, &traits);
    assert_eq!(
        calc_general_trait_bonus(&world, ATTACKER_NO_TRAITS, 3001, TraitType::Shock, false),
        1.0
    );
}

/// Two resistance buffs **stack additively** (Java `mergeDefenceTrait` sums),
/// and a value of 100+ in the XML is not "100 % resist" but outright
/// invulnerability — which is a different code path, and returns 0.
#[test]
fn defence_traits_stack_and_100_means_invulnerable() {
    use crate::game_loop::skills::effects::{
        calc_general_trait_bonus, merge_defence_traits, remove_defence_traits,
    };
    use crate::model::skill::TraitType;

    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    merge_defence_traits(&mut world, 3001, &[(TraitType::Shock, 0.30)]);
    merge_defence_traits(&mut world, 3001, &[(TraitType::Shock, 0.20)]);
    assert!(
        (calc_general_trait_bonus(&world, ATTACKER_NO_TRAITS, 3001, TraitType::Shock, false)
            - 0.50)
            .abs()
            < 1e-9,
        "30% + 20% = 50%"
    );
    // Removing one leaves the other.
    remove_defence_traits(&mut world, 3001, &[(TraitType::Shock, 0.20)]);
    assert!(
        (calc_general_trait_bonus(&world, ATTACKER_NO_TRAITS, 3001, TraitType::Shock, false)
            - 0.70)
            .abs()
            < 1e-9
    );

    // Invulnerability is its own branch (`>= 1.0` → `mergeInvulnerableTrait`).
    merge_defence_traits(&mut world, 3001, &[(TraitType::Hold, 1.0)]);
    assert_eq!(
        calc_general_trait_bonus(&world, ATTACKER_NO_TRAITS, 3001, TraitType::Hold, false),
        0.0,
        "invulnerable, not merely resistant"
    );
}

/// Only Java's **group 3** traits are resisted *through the landing roll*. A
/// weapon-type trait (group 1) passes through at 1.0 here, and so does the
/// `*_WEAKNESS` family (group 2) unless the attacker carries the matching
/// `AttackTrait`. Both groups do bite elsewhere — the damage formulas read them
/// through `calcWeaponTraitBonus`/`calcWeaknessBonus` — this is only about the
/// landing roll.
#[test]
fn only_the_resistable_trait_group_is_scaled() {
    use crate::game_loop::skills::effects::{calc_general_trait_bonus, merge_defence_traits};
    use crate::model::skill::{TraitType, WeaknessTrait};

    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    merge_defence_traits(
        &mut world,
        3001,
        &[
            (TraitType::Weakness(WeaknessTrait::Beast), 0.50),
            (TraitType::Other, 0.50),
        ],
    );
    assert_eq!(
        calc_general_trait_bonus(
            &world,
            ATTACKER_NO_TRAITS,
            3001,
            TraitType::Weakness(WeaknessTrait::Beast),
            false
        ),
        1.0,
        "group 2 needs the attacker's AttackTrait, which nothing here grants"
    );
    assert_eq!(
        calc_general_trait_bonus(&world, ATTACKER_NO_TRAITS, 3001, TraitType::Other, false),
        1.0,
        "group 1 (weapon types) is never resisted through this path"
    );
    assert_eq!(
        calc_general_trait_bonus(&world, ATTACKER_NO_TRAITS, 3001, TraitType::None, false),
        1.0
    );

    // …but **invulnerability** is tested before the group switch, so it does
    // reach a group-2 trait. Java's clause order, not an accident.
    merge_defence_traits(
        &mut world,
        3001,
        &[(TraitType::Weakness(WeaknessTrait::Beast), 1.0)],
    );
    assert_eq!(
        calc_general_trait_bonus(
            &world,
            ATTACKER_NO_TRAITS,
            3001,
            TraitType::Weakness(WeaknessTrait::Beast),
            false
        ),
        0.0,
        "invulnerability is checked ahead of the group gate"
    );
}

/// The dist parse: Stun Resistance's real XML yields a `SHOCK` `DefenceTrait`
/// whose value is the percentage over 100, per level.
#[test]
fn stun_resistance_parses_its_per_level_percentages() {
    use crate::model::skill::{SkillEffect, TraitType};
    let sd = dist::skills();
    for (level, pct) in [(1, 0.15), (2, 0.20), (3, 0.30), (4, 0.40)] {
        let s = sd.get(1259, level).expect("Stun Resistance");
        let traits = s
            .effects
            .iter()
            .find_map(|e| match e {
                SkillEffect::DefenceTrait { traits } => Some(traits.clone()),
                _ => None,
            })
            .expect("a DefenceTrait effect");
        assert_eq!(traits.len(), 1);
        assert_eq!(traits[0].0, TraitType::Shock);
        assert!(
            (traits[0].1 - pct).abs() < 1e-9,
            "level {level} is {pct}, got {}",
            traits[0].1
        );
    }
    // And a stun declares the matching trait, which is what pairs them.
    let stun = sd.get(100, 1).expect("Stun Attack");
    assert_eq!(stun.trait_type, TraitType::Shock);
}

/// End to end: casting a `DefenceTrait` buff **installs** the resistance and
/// letting it expire **takes it back**. Java does this in
/// `DefenceTrait.onStart`/`onExit` (`mergeDefenceTrait` / `removeDefenceTrait`),
/// and the buff carries no stat modifier of its own — so it also has to survive
/// the empty-effects guard that drops icon-only buffs.
#[test]
fn a_defence_trait_buff_installs_and_removes_its_resistance() {
    use crate::game_loop::skills::effects::calc_general_trait_bonus;
    use crate::model::skill::TraitType;

    let (mut world, _db, _l) = cast_test_world();
    let mut buff = base_skill(
        9410,
        vec![SkillEffect::DefenceTrait {
            traits: vec![(TraitType::Shock, 0.30), (TraitType::Sleep, 1.0)],
        }],
    );
    buff.name = "Stun Resistance".into();
    buff.target_type = TargetType::Self_;
    buff.effect_point = 100;
    buff.is_continuous = true;
    world.data.skill_data.insert_for_test(buff);
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    land_skill_on_target(&mut world, 9410, CASTER);
    assert!(
        has_buff(&world, CASTER, 9410),
        "an effect-less DefenceTrait buff still lands as a timed buff"
    );
    assert!(
        (calc_general_trait_bonus(&world, ATTACKER_NO_TRAITS, CASTER, TraitType::Shock, false)
            - 0.70)
            .abs()
            < 1e-9
    );
    assert_eq!(
        calc_general_trait_bonus(&world, ATTACKER_NO_TRAITS, CASTER, TraitType::Sleep, false),
        0.0
    );

    handle_buff_expire(&mut world, CASTER, 9410);
    assert!(!has_buff(&world, CASTER, 9410));
    assert_eq!(
        calc_general_trait_bonus(&world, ATTACKER_NO_TRAITS, CASTER, TraitType::Shock, false),
        1.0,
        "the resistance leaves with the buff"
    );
    assert_eq!(
        calc_general_trait_bonus(&world, ATTACKER_NO_TRAITS, CASTER, TraitType::Sleep, false),
        1.0
    );
}
