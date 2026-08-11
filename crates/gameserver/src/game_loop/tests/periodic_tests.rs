//! Periodic HP/MP effects, healing modifiers and CP restore (G19).

use super::*;
use crate::game_loop::abnormal::has_buff;
use crate::game_loop::helpers::skill_by_id;

use crate::game_loop::helpers::stat_mul;
use crate::game_loop::sit_stand;
use crate::model::components::PlayerVitals;
use crate::model::skill::{
    AffectObject, AffectScope, OperateType, Skill, SkillEffect, StatModifierEffect, TargetType,
};
use crate::model::stats::{Stat, StatModifierType};

const CASTER: i32 = 2001;
const CID: u32 = 1;

fn periodic_skill(id: i32, effects: Vec<SkillEffect>, toggle: bool) -> Skill {
    Skill {
        self_continuous: false,
        without_action: false,
        trait_type: crate::model::skill::TraitType::None,
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

fn hp(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_hp
}
fn mp(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_mp
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

    land_skill_on_target(&mut world, 9500, CASTER);
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
    land_skill_on_target(&mut world, 9501, CASTER);
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
    land_skill_on_target(&mut world, 9510, CASTER);
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
    land_skill_on_target(&mut world, 9511, CASTER);
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
    land_skill_on_target(&mut world, 9512, CASTER);
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
        let skill = skill_by_id(world, 1015, 1).expect("heal skill");
        effects::apply_skill_effects(world, CASTER, CASTER, &skill);
        hp(world, CASTER) - 1.0
    };

    let unmodified = heal_once(&mut world);
    assert!(unmodified > 0.0, "baseline heal landed: {unmodified}");

    land_skill_on_target(&mut world, 9520, CASTER);
    let mul = stat_mul(&world, CASTER, Stat::HealEffect);
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

    land_skill_on_target(&mut world, 9530, CASTER);
    let restored = world
        .objects
        .get_component::<PlayerVitals>(&CASTER)
        .unwrap()
        .cur_cp;
    assert!(restored > 0.0, "CP restored: {restored}");

    land_skill_on_target(&mut world, 9531, CASTER);
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
        land_skill_on_target(&mut world, 9530, CASTER);
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

// ---------------------------------------------------------------------------
// Relax (skill 226) — the seated MP-upkeep toggle
// ---------------------------------------------------------------------------

const RELAX: i32 = 9560;

fn relax_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = cast_test_world();
    world.data.skill_data.insert_for_test(periodic_skill(
        RELAX,
        vec![SkillEffect::Relax {
            power: 1.0,
            ticks: 3,
        }],
        true,
    ));
    (world, db, l)
}

/// Wound the caster so the "HP is full" stop condition does not fire first —
/// without this every Relax test would end on the wrong branch.
fn wound(world: &mut World, oid: i32) {
    let v = world.objects.get_component_mut::<Vitals>(&oid).unwrap();
    v.cur_hp = 10.0;
}

/// `Relax.onStart` seats its caster: Java calls `sitDown(false)`.
#[test]
fn relax_seats_its_caster() {
    let (mut world, _db, _l) = relax_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    wound(&mut world, CASTER);
    assert!(!sit_stand::is_sitting(&world, CASTER));

    land_skill_on_target(&mut world, RELAX, CASTER);

    assert!(
        sit_stand::is_sitting(&world, CASTER),
        "casting Relax sits the player down"
    );
}

/// While seated it drains MP each tick — the upkeep that pays for the HP regen.
#[test]
fn relax_drains_mp_while_seated() {
    let (mut world, _db, _l) = relax_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    wound(&mut world, CASTER);

    land_skill_on_target(&mut world, RELAX, CASTER);
    let before = mp(&world, CASTER);
    advance_ticks(&mut world, ONE_TICK);

    assert!(
        mp(&world, CASTER) < before,
        "MP upkeep is paid: {before} -> {}",
        mp(&world, CASTER)
    );
    assert!(
        has_buff(&world, CASTER, RELAX),
        "and the toggle is still on"
    );
}

/// Standing up ends it — Java `stopEffects(EffectFlag.RELAXING)` on `standUp`.
///
/// The tick's own "not sitting" gate would also catch this eventually, so the
/// assertion is deliberately made **before** any tick could fire: the point is
/// that standing stops the upkeep at once rather than up to an interval later.
#[test]
fn standing_up_ends_relax_immediately() {
    let (mut world, _db, _l) = relax_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    wound(&mut world, CASTER);
    land_skill_on_target(&mut world, RELAX, CASTER);
    assert!(has_buff(&world, CASTER, RELAX));

    sit_stand::stand_up(&mut world, CASTER);

    assert!(
        !has_buff(&world, CASTER, RELAX),
        "standing up ends the toggle without waiting for a tick"
    );
}

/// Out of MP, the toggle switches itself off (SM 140), like the other upkeep
/// effects.
#[test]
fn relax_switches_off_when_mp_runs_out() {
    let (mut world, _db, _l) = relax_world();
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    wound(&mut world, CASTER);
    land_skill_on_target(&mut world, RELAX, CASTER);
    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .cur_mp = 0.0;
    drain(&mut out);

    advance_ticks(&mut world, ONE_TICK);

    assert!(!has_buff(&world, CASTER, RELAX), "the toggle switched off");
    assert!(
        sm_ids_of(&drain(&mut out))
            .contains(&server_packets::sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP),
        "and said why"
    );
}

/// At full HP it retires itself with its **own** message — the branch that
/// distinguishes "job done" from "ran dry", and the one a naive port collapses
/// into the MP check.
#[test]
fn relax_switches_off_at_full_hp_with_its_own_message() {
    let (mut world, _db, _l) = relax_world();
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    wound(&mut world, CASTER);
    land_skill_on_target(&mut world, RELAX, CASTER);
    // Heal to full *after* it started, so `onStart` still ran normally.
    let max_hp = world
        .objects
        .get_component::<Vitals>(&CASTER)
        .unwrap()
        .max_hp as f64;
    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .cur_hp = max_hp;
    drain(&mut out);

    advance_ticks(&mut world, ONE_TICK);

    assert!(!has_buff(&world, CASTER, RELAX), "the toggle switched off");
    let sms = sm_ids_of(&drain(&mut out));
    assert!(
        sms.contains(
            &server_packets::sm_ids::THAT_SKILL_HAS_BEEN_DE_ACTIVATED_AS_HP_WAS_FULLY_RECOVERED
        ),
        "with the full-HP message, not the out-of-MP one"
    );
    assert!(
        !sms.contains(&server_packets::sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP),
        "and not both"
    );
}

/// The real skill 226 parses its `Relax` effect. Pinned against the datapack
/// because this whole feature was dormant behind a missing parser arm: the
/// effect name was simply not matched, so a level-5 toggle every Human and Orc
/// Fighter learns did nothing at all.
#[test]
fn the_real_relax_skill_parses_its_effect() {
    let skills = crate::data::skill_data::SkillData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    let relax = skills.get(226, 1).expect("skill 226 Relax");
    assert!(
        relax
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::Relax { .. })),
        "the <effect name=\"Relax\"> is parsed, not dropped"
    );
    assert_eq!(
        relax.operate_type,
        OperateType::Toggle,
        "it is a toggle, which is what drives the self-deactivation"
    );
}

// ---------------------------------------------------------------------------
// G34 S4 sub-slice 11 — ChameleonRest and ManaHealOverTime
// ---------------------------------------------------------------------------

const CHAMELEON: i32 = 9561;
const MANA_HOT: i32 = 9562;

fn chameleon_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = cast_test_world();
    world.data.skill_data.insert_for_test(periodic_skill(
        CHAMELEON,
        vec![SkillEffect::ChameleonRest {
            power: 2.0,
            ticks: 5,
        }],
        true,
    ));
    (world, db, l)
}

/// **The difference from Relax.** Both sit you down and both drain MP, but
/// Relax retires itself once HP is full — it exists to regenerate. Chameleon
/// Rest exists to *hide*, so a full HP bar means nothing to it and it keeps
/// running. A port that reused Relax's arm wholesale would switch this skill
/// off exactly when a healthy player wanted to hide.
#[test]
fn chameleon_rest_keeps_running_at_full_hp_where_relax_would_stop() {
    let (mut world, _db, _l) = chameleon_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    // Deliberately *not* wounded — this is the state that stops Relax.
    assert_eq!(
        hp(&world, CASTER),
        world
            .objects
            .get_component::<Vitals>(&CASTER)
            .map(|v| v.max_hp as f64)
            .unwrap(),
        "the caster starts at full HP"
    );

    land_skill_on_target(&mut world, CHAMELEON, CASTER);
    assert!(
        sit_stand::is_sitting(&world, CASTER),
        "it seats its caster, like Relax"
    );

    let before = mp(&world, CASTER);
    advance_ticks(&mut world, ONE_TICK);
    assert!(
        has_buff(&world, CASTER, CHAMELEON),
        "full HP does not retire it — that stop belongs to Relax alone"
    );
    assert!(
        mp(&world, CASTER) < before,
        "and the upkeep is still being paid: {before} -> {}",
        mp(&world, CASTER)
    );
}

/// Standing up ends it, the same `EffectFlag.RELAXING` cancellation Relax gets
/// — and it must not wait for the next tick to notice.
#[test]
fn standing_up_ends_chameleon_rest_immediately() {
    let (mut world, _db, _l) = chameleon_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    land_skill_on_target(&mut world, CHAMELEON, CASTER);
    assert!(has_buff(&world, CASTER, CHAMELEON));

    sit_stand::stand_up(&mut world, CASTER);

    assert!(
        !has_buff(&world, CASTER, CHAMELEON),
        "standing up ends the toggle without waiting for a tick"
    );
}

/// **`ManaHealOverTime`** (Force Meditation 441, Invocation 1430, Soul Harmony
/// 1480) — the mirror of `ManaDamOverTime`: it *restores* MP per tick, clamped
/// at the pool. Without it these three skills landed a buff icon and gave back
/// nothing.
#[test]
fn mana_heal_over_time_restores_mp_and_stops_at_full() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(periodic_skill(
        MANA_HOT,
        vec![SkillEffect::ManaHealOverTime {
            power: 10.0,
            ticks: 5,
        }],
        false,
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let max_mp = world
        .objects
        .get_component::<Vitals>(&CASTER)
        .map(|v| v.max_mp as f64)
        .unwrap();
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&CASTER) {
        v.cur_mp = 1.0;
    }

    land_skill_on_target(&mut world, MANA_HOT, CASTER);
    advance_ticks(&mut world, ONE_TICK);
    let after_one = mp(&world, CASTER);
    assert!(
        after_one > 1.0,
        "a tick restores MP, it does not drain it: 1 -> {after_one}"
    );

    // Run it dry against the ceiling: MP never exceeds the pool.
    for _ in 0..40 {
        advance_ticks(&mut world, ONE_TICK);
    }
    assert!(
        mp(&world, CASTER) <= max_mp,
        "and it clamps at the pool: {} > {max_mp}",
        mp(&world, CASTER)
    );
}
