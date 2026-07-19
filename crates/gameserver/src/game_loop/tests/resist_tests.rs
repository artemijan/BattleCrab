//! Abnormal resistance, blocking and probabilistic dispel (G19).

use super::*;

use crate::model::components::{Buffs, StatModifiers};
use crate::model::skill::{AffectObject, AffectScope, OperateType, Skill, SkillEffect, StatModifierEffect, TargetType};
use crate::model::stats::{Stat, StatModifierType};

const CASTER: i32 = 2001;
const VICTIM: i32 = 2002;
const CID: u32 = 1;
const VICTIM_CID: u32 = 2;

fn base_skill(id: i32, effects: Vec<SkillEffect>) -> Skill {
    Skill {
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
        effects,
    }
}

fn land(world: &mut World, skill_id: i32, target: i32) {
    let skill = world.data.skill_data.get(skill_id, 1).cloned().expect("registered");
    crate::game_loop::skills::effects::apply_skill_effects(world, CASTER, target, &skill);
}

fn has_buff(world: &World, oid: i32, skill_id: i32) -> bool {
    world.objects.get_component::<Buffs>(&oid).is_some_and(|b| b.0.iter().any(|x| x.skill_id == skill_id))
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
        })],
    );
    world.data.skill_data.insert_for_test(resist);
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    land(&mut world, 9400, CASTER);
    let mul = world
        .objects
        .get_component::<StatModifiers>(&CASTER)
        .and_then(|m| m.mul.get(&Stat::ResistAbnormalDebuff).copied())
        .unwrap_or(1.0);
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
    let unresisted = calc_effect_land_rate(40, 50, 0, 40, 1.0);
    assert!((unresisted - 80.0).abs() < 1e-9, "got {unresisted}");

    // Guts (x0.5): 80 * 0.5 = 40.
    let resisted = calc_effect_land_rate(40, 50, 0, 40, 0.5);
    assert!((resisted - 40.0).abs() < 1e-9, "got {resisted}");

    // Touch of Death (x1.3): 80 * 1.3 = 104, clamped down to the 90 ceiling.
    let vulnerable = calc_effect_land_rate(40, 50, 0, 40, 1.3);
    assert!((vulnerable - 90.0).abs() < 1e-9, "clamped after the multiply, got {vulnerable}");

    // The 10 floor still holds under a crushing resistance.
    let crushed = calc_effect_land_rate(40, 50, 0, 40, 0.01);
    assert!((crushed - 10.0).abs() < 1e-9, "got {crushed}");

    // An always-lands debuff (`activate_rate == -1`) ignores resistance
    // entirely, as in Java (the early return precedes the whole formula).
    assert_eq!(calc_effect_land_rate(40, -1, 0, 40, 0.01), 100.0);
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

    let mut blocker = base_skill(9410, vec![SkillEffect::BlockAbnormalSlot { slots: vec!["BUFF_SPECIAL_ATTACK".into()] }]);
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
            })],
        );
        s.abnormal_type = abnormal.into();
        s
    };
    world.data.skill_data.insert_for_test(stat(9411, "BUFF_SPECIAL_ATTACK"));
    world.data.skill_data.insert_for_test(stat(9412, "SOMETHING_ELSE"));

    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    // Baseline: with no blocker up, the buff lands.
    land(&mut world, 9411, CASTER);
    assert!(has_buff(&world, CASTER, 9411), "lands freely when nothing blocks it");
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, CASTER, 9411);

    // With the blocker up it is refused, while the unrelated buff still lands.
    land(&mut world, 9410, CASTER);
    assert!(has_buff(&world, CASTER, 9410), "the blocker itself lands");
    land(&mut world, 9411, CASTER);
    assert!(!has_buff(&world, CASTER, 9411), "a blocked abnormal type is refused");
    land(&mut world, 9412, CASTER);
    assert!(has_buff(&world, CASTER, 9412), "an unblocked type is unaffected");

    // Once the blocker goes, the previously blocked buff lands again.
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, CASTER, 9410);
    land(&mut world, 9411, CASTER);
    assert!(has_buff(&world, CASTER, 9411), "blocking ends with the buff");
}

// ---------------------------------------------------------------------------
// DispelBySlotProbability
// ---------------------------------------------------------------------------

fn seed_dispel_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = cast_test_world();
    // Two dispellable buffs plus one that is not on the list. The two must
    // carry *distinct* abnormal types — same-type buffs replace each other via
    // the abnormal stacking rules, which would mask what the dispel did.
    // Both types below are on the real Warrior Bane list.
    for (id, abnormal) in [(9421, "SPEED_UP"), (9422, "IMPROVE_SPEED_AVOID_UP"), (9423, "UNRELATED")] {
        let mut s = base_skill(
            id,
            vec![SkillEffect::StatModifier(StatModifierEffect {
                stat: Stat::PhysicalAttack,
                mode: StatModifierType::Per,
                amount: 5.0,
                armor_condition: 0,
                weapon_condition: 0,
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
        land(&mut world, id, VICTIM);
        assert!(has_buff(&world, VICTIM, id));
    }

    land(&mut world, 9420, VICTIM);
    assert!(!has_buff(&world, VICTIM, 9421), "matching buff stripped");
    assert!(!has_buff(&world, VICTIM, 9422), "both matching buffs stripped");
    assert!(has_buff(&world, VICTIM, 9423), "an unlisted abnormal type survives");
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
        land(&mut world, id, VICTIM);
    }
    land(&mut world, 9420, VICTIM);
    assert!(has_buff(&world, VICTIM, 9421), "a 0% Bane strips nothing");
    assert!(has_buff(&world, VICTIM, 9422));
}
