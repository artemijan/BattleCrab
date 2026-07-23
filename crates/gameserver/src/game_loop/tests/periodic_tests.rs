//! Periodic HP/MP effects, healing modifiers and CP restore (G19).

use super::*;

use crate::model::components::{Buffs, PlayerVitals, StatModifiers};
use crate::model::skill::{
    AffectObject, AffectScope, OperateType, Skill, SkillEffect, StatModifierEffect, TargetType,
};
use crate::model::stats::{Stat, StatModifierType};

const CASTER: i32 = 2001;
const CID: u32 = 1;

fn periodic_skill(id: i32, effects: Vec<SkillEffect>, toggle: bool) -> Skill {
    Skill {
        without_action: false,
        item_consume_id: 0,
        item_consume_count: 0,
        id,
        level: 1,
        name: format!("P{id}"),
        operate_type: if toggle {
            OperateType::Toggle
        } else {
            OperateType::Active
        },
        is_continuous: false,
        target_type: if toggle {
            TargetType::None_
        } else {
            TargetType::Self_
        },
        magic_type: 1,
        magic_level: 0,
        effect_point: 0,
        cast_range: 0,
        effect_range: 0,
        hit_time: 0,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        // Long enough that natural expiry never races the tick assertions.
        abnormal_time: 600,
        abnormal_level: 1,
        abnormal_type: format!("PERIODIC{id}"),
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

fn land(world: &mut World, skill_id: i32, target: i32) {
    let skill = world
        .data
        .skill_data
        .get(skill_id, 1)
        .cloned()
        .expect("registered");
    crate::game_loop::skills::effects::apply_skill_effects(world, CASTER, target, &skill);
}

fn hp(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_hp
}
fn mp(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_mp
}
fn has_buff(world: &World, oid: i32, skill_id: i32) -> bool {
    world
        .objects
        .get_component::<Buffs>(&oid)
        .is_some_and(|b| b.0.iter().any(|x| x.skill_id == skill_id))
}

/// Ticks are `ticks * 666 ms`; 5 ticks ≈ 3330 ms ≈ 34 game ticks. Advance
/// generously so exactly one periodic tick has certainly fired.
const ONE_TICK: u64 = 40;

// ---------------------------------------------------------------------------
// HealOverTime
// ---------------------------------------------------------------------------

/// A positive-power HoT restores HP over time and stops at full.
#[test]
fn heal_over_time_restores_hp_and_caps_at_full() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(periodic_skill(
        9500,
        vec![SkillEffect::HealOverTime {
            power: 10.0,
            ticks: 5,
        }],
        false,
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    // Wound the caster so there is headroom to heal into.
    let max_hp = world
        .objects
        .get_component::<Vitals>(&CASTER)
        .unwrap()
        .max_hp as f64;
    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .cur_hp = 10.0;

    land(&mut world, 9500, CASTER);
    advance_ticks(&mut world, ONE_TICK);
    let after = hp(&world, CASTER);
    assert!(after > 10.0, "the HoT healed: {after}");

    // Run it long enough to reach full, and confirm it never overshoots.
    advance_ticks(&mut world, ONE_TICK * 40);
    assert!(hp(&world, CASTER) <= max_hp, "never exceeds max HP");
}

/// **`HealOverTime` with a negative power drains** — this is how the upkeep
/// toggles (Fury Fists 222, Arcane Wisdom 336) pay for themselves. It floors at
/// 1 HP rather than killing its owner.
#[test]
fn negative_heal_over_time_drains_but_never_kills() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(periodic_skill(
        9501,
        vec![SkillEffect::HealOverTime {
            power: -20.0,
            ticks: 5,
        }],
        true,
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let before = hp(&world, CASTER);
    land(&mut world, 9501, CASTER);
    advance_ticks(&mut world, ONE_TICK);
    let after = hp(&world, CASTER);
    assert!(after < before, "the upkeep drained HP: {before} -> {after}");

    // Long enough to have drained far past zero if unclamped.
    advance_ticks(&mut world, ONE_TICK * 60);
    let floored = hp(&world, CASTER);
    assert!(floored >= 1.0, "an HP upkeep floors at 1, got {floored}");
    assert!(
        !world.objects.get_component::<Vitals>(&CASTER).unwrap().dead,
        "and never kills its owner"
    );
}

// ---------------------------------------------------------------------------
// ManaDamOverTime
// ---------------------------------------------------------------------------

/// A toggle's MP upkeep drains MP each tick.
#[test]
fn mana_dam_over_time_drains_mp() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(periodic_skill(
        9510,
        vec![SkillEffect::ManaDamOverTime {
            power: 3.0,
            ticks: 5,
        }],
        true,
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let before = mp(&world, CASTER);
    land(&mut world, 9510, CASTER);
    advance_ticks(&mut world, ONE_TICK);
    assert!(mp(&world, CASTER) < before, "MP upkeep drained");
}

/// **Running out of MP switches the toggle off** and tells the player — Java's
/// `false` return from `onActionTime`, which cancels a toggle. This is the tie
/// between this slice and the toggle support from the first G19 slice.
#[test]
fn toggle_deactivates_when_mp_runs_out() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(periodic_skill(
        9511,
        vec![SkillEffect::ManaDamOverTime {
            power: 50.0,
            ticks: 5,
        }],
        true,
    ));
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    // Not enough MP to pay even one tick.
    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .cur_mp = 1.0;
    land(&mut world, 9511, CASTER);
    assert!(
        has_buff(&world, CASTER, 9511),
        "the toggle is on to begin with"
    );
    drain(&mut out);

    advance_ticks(&mut world, ONE_TICK);
    assert!(
        !has_buff(&world, CASTER, 9511),
        "the toggle switched itself off"
    );
    let pkts = drain(&mut out);
    assert!(
        pkts.iter().any(|p| {
            p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP
        }),
        "and the player is told why"
    );
}

/// A *non*-toggle MP drain just floors at 0 — only toggles self-cancel.
#[test]
fn non_toggle_mp_drain_does_not_self_cancel() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(periodic_skill(
        9512,
        vec![SkillEffect::ManaDamOverTime {
            power: 50.0,
            ticks: 5,
        }],
        false,
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .cur_mp = 1.0;
    land(&mut world, 9512, CASTER);
    advance_ticks(&mut world, ONE_TICK);
    assert!(has_buff(&world, CASTER, 9512), "a non-toggle keeps ticking");
    assert_eq!(mp(&world, CASTER), 0.0, "MP floors at 0");
}

// ---------------------------------------------------------------------------
// HealEffect / Cp
// ---------------------------------------------------------------------------

/// `HealEffect` scales the healing its bearer *receives* — the stat is read off
/// the target, not the healer.
#[test]
fn heal_effect_scales_received_healing() {
    let (mut world, _db, _l) = cast_test_world();
    // A -50% HealEffect (like Touch of Death's -30 PER, exaggerated).
    world.data.skill_data.insert_for_test(periodic_skill(
        9520,
        vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::HealEffect,
            mode: StatModifierType::Per,
            amount: -50.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
        })],
        false,
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let heal_once = |world: &mut World| {
        world
            .objects
            .get_component_mut::<Vitals>(&CASTER)
            .unwrap()
            .cur_hp = 1.0;
        // 1015 is `cast_test_world`'s Battle-Heal-like skill.
        let skill = world
            .data
            .skill_data
            .get(1015, 1)
            .cloned()
            .expect("heal skill");
        crate::game_loop::skills::effects::apply_skill_effects(world, CASTER, CASTER, &skill);
        hp(world, CASTER) - 1.0
    };

    let unmodified = heal_once(&mut world);
    assert!(unmodified > 0.0, "baseline heal landed: {unmodified}");

    land(&mut world, 9520, CASTER);
    let mul = world
        .objects
        .get_component::<StatModifiers>(&CASTER)
        .and_then(|m| m.mul.get(&Stat::HealEffect).copied())
        .unwrap_or(1.0);
    assert!((mul - 0.5).abs() < 1e-9, "-50 PER → x0.5, got {mul}");

    let modified = heal_once(&mut world);
    assert!(
        modified < unmodified,
        "healing is reduced: {modified} < {unmodified}"
    );
}

/// `Cp` restores CP instantly, capped at the pool, and can also take it away.
#[test]
fn cp_effect_restores_and_drains() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(periodic_skill(
        9530,
        vec![SkillEffect::Cp {
            amount: 50.0,
            percent: false,
        }],
        false,
    ));
    world.data.skill_data.insert_for_test(periodic_skill(
        9531,
        vec![SkillEffect::Cp {
            amount: -20.0,
            percent: false,
        }],
        false,
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let max_cp = world
        .objects
        .get_component::<PlayerVitals>(&CASTER)
        .unwrap()
        .max_cp as f64;
    world
        .objects
        .get_component_mut::<PlayerVitals>(&CASTER)
        .unwrap()
        .cur_cp = 0.0;

    land(&mut world, 9530, CASTER);
    let restored = world
        .objects
        .get_component::<PlayerVitals>(&CASTER)
        .unwrap()
        .cur_cp;
    assert!(restored > 0.0, "CP restored: {restored}");

    land(&mut world, 9531, CASTER);
    let drained = world
        .objects
        .get_component::<PlayerVitals>(&CASTER)
        .unwrap()
        .cur_cp;
    assert!(
        drained < restored,
        "a negative amount takes CP away: {drained} < {restored}"
    );

    // Repeated restores never exceed the pool.
    for _ in 0..20 {
        land(&mut world, 9530, CASTER);
    }
    let capped = world
        .objects
        .get_component::<PlayerVitals>(&CASTER)
        .unwrap()
        .cur_cp;
    assert!(
        capped <= max_cp,
        "CP never exceeds max ({capped} <= {max_cp})"
    );
}
