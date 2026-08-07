//! The MP-restore family — `ManaHeal`, `ManaHealByLevel`, `ManaHealPercent`,
//! `Mp` (G19).
//!
//! Recharge 1013, Servitor Recharge 1126 and Mass Recharge 1428 each carry
//! **only** `ManaHealByLevel`, so all three parsed to an empty effect list and
//! were dropped whole — the core mage-support skill restored nothing.
//!
//! This slice also closes the G19 deferral the `MagicalAttackMp` slice left:
//! all four of these handlers read `isMpBlocked()`, the flag that had been
//! mis-documented as dead.

use super::*;

use crate::model::components::{Buffs, StatModifiers};
use crate::model::skill::{SkillEffect, effect_flag};
use crate::model::stats::Stat;

use crate::game_loop::skills::effects::recharge_level_penalty;

const CASTER: i32 = 6001;
const TARGET: i32 = 6002;
const CID: u32 = 1;
const TCID: u32 = 2;
const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

fn dist_skills() -> crate::data::skill_data::SkillData {
    crate::data::skill_data::SkillData::load_from(DIST)
}

fn mp(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_mp
}

/// Register a one-effect skill and cast it from `CASTER` at `target`.
fn cast(
    world: &mut World,
    skill_id: i32,
    effects: Vec<SkillEffect>,
    magic_level: i32,
    target: i32,
) {
    use crate::model::skill::{AffectObject, AffectScope, OperateType, Skill, TargetType};
    let skill = Skill {
        self_continuous: false,
        without_action: false,
        trait_type: crate::model::skill::TraitType::None,
        item_consume_id: 0,
        item_consume_count: 0,
        id: skill_id,
        level: 1,
        name: format!("R{skill_id}"),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Target,
        magic_type: 1,
        magic_level,
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
        abnormal_time: 0,
        abnormal_level: 0,
        abnormal_type: "NONE".to_string(),
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
    };
    world.data.skill_data.insert_for_test(skill.clone());
    crate::game_loop::skills::effects::apply_skill_effects(world, CASTER, target, &skill);
}

/// Drain the target so there is headroom to restore into.
fn empty_mp(world: &mut World, oid: i32) {
    world
        .objects
        .get_component_mut::<Vitals>(&oid)
        .unwrap()
        .cur_mp = 0.0;
}

/// Give the target a pool big enough that the overheal clamp can't mask what a
/// test is actually measuring — the level-5 fixture's ~50 MP silently capped
/// both halves of the penalty comparison at max and made them look equal.
fn roomy_mp(world: &mut World, oid: i32) {
    let v = world.objects.get_component_mut::<Vitals>(&oid).unwrap();
    v.max_mp = 1000;
    v.cur_mp = 0.0;
}

// ---------------------------------------------------------------------------
// The level-gap penalty
// ---------------------------------------------------------------------------

/// `ManaHealByLevel`'s ladder: unpenalised to a 5-level gap, then 10% less per
/// level, and **nothing at all** from 15 up. Java writes it as nine `else if`
/// branches; this pins every one of them against the arithmetic that replaced
/// them.
#[test]
fn recharge_level_penalty_matches_javas_ladder() {
    // No penalty at or below a 5-level gap (including the target being lower).
    for diff in [-10, 0, 5] {
        assert_eq!(
            recharge_level_penalty(30 + diff, 30),
            1.0,
            "gap {diff} is unpenalised"
        );
    }
    // Java's explicit branches, 6..=14.
    for (diff, expected) in [
        (6, 0.9),
        (7, 0.8),
        (8, 0.7),
        (9, 0.6),
        (10, 0.5),
        (11, 0.4),
        (12, 0.3),
        (13, 0.2),
        (14, 0.1),
    ] {
        let got = recharge_level_penalty(30 + diff, 30);
        assert!(
            (got - expected).abs() < 1e-9,
            "gap {diff} → {expected}, got {got}"
        );
    }
    // 15 and beyond: zero, not a small number.
    for diff in [15, 16, 40] {
        assert_eq!(
            recharge_level_penalty(30 + diff, 30),
            0.0,
            "gap {diff} restores nothing"
        );
    }
}

/// End to end: the same skill on the same target restores less once the target
/// outlevels it far enough, and nothing at all past the cliff.
#[test]
fn a_high_level_target_is_recharged_less() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _t = ingame_caster(&mut world, TCID, TARGET, 0, 0);

    // Target level 5 (the `ingame_caster` default); skill magicLevel 5 → no gap.
    roomy_mp(&mut world, TARGET);
    cast(
        &mut world,
        9700,
        vec![SkillEffect::ManaHealByLevel { power: 100.0 }],
        5,
        TARGET,
    );
    let unpenalised = mp(&world, TARGET);
    assert!(unpenalised > 0.0, "sanity: the recharge lands");

    // Same skill, magicLevel 5 lower than the target by 10 → ×0.5.
    roomy_mp(&mut world, TARGET);
    cast(
        &mut world,
        9701,
        vec![SkillEffect::ManaHealByLevel { power: 100.0 }],
        5 - 10,
        TARGET,
    );
    let penalised = mp(&world, TARGET);
    assert!(
        (penalised - unpenalised * 0.5).abs() < 1e-6,
        "a 10-level gap halves it: {unpenalised} -> {penalised}"
    );

    // And past the cliff, nothing.
    roomy_mp(&mut world, TARGET);
    cast(
        &mut world,
        9702,
        vec![SkillEffect::ManaHealByLevel { power: 100.0 }],
        5 - 20,
        TARGET,
    );
    assert_eq!(mp(&world, TARGET), 0.0, "a 20-level gap restores nothing");
}

// ---------------------------------------------------------------------------
// The shared apply path
// ---------------------------------------------------------------------------

/// "Prevents overheal": the restore is clamped to the headroom, never past max.
#[test]
fn restore_never_overheals() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let max_mp = world
        .objects
        .get_component::<Vitals>(&CASTER)
        .unwrap()
        .max_mp as f64;

    // Leave 5 MP of headroom, then try to restore 10_000.
    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .cur_mp = max_mp - 5.0;
    cast(
        &mut world,
        9710,
        vec![SkillEffect::ManaHeal { power: 10_000.0 }],
        1,
        CASTER,
    );
    assert_eq!(mp(&world, CASTER), max_mp, "clamped exactly to full");
}

/// `isMpBlocked()` refuses the restore outright — the gate this slice closes
/// what the previous one left open. Without it, MP-block would stop drains but not
/// heals, which is exactly backwards from Java.
#[test]
fn mp_block_refuses_a_restore() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    empty_mp(&mut world, CASTER);

    world
        .objects
        .get_component_mut::<Buffs>(&CASTER)
        .unwrap()
        .0
        .push(crate::model::skill::ActiveBuff {
            displayed: true,
            skill_id: 1418,
            skill_level: 1,
            abnormal_type_client_id: 0,
            abnormal_type: "NONE".to_string(),
            abnormal_level: 0,
            slot: crate::model::skill::BuffSlot::Buff,
            expires_at_tick: u64::MAX,
            passive: false,
            effect_flags: effect_flag::MP_BLOCK,
            blocked_abnormals: Vec::new(),
            abnormal_visuals: Vec::new(),
            effects: Vec::new(),
        });

    cast(
        &mut world,
        9711,
        vec![SkillEffect::ManaHeal { power: 500.0 }],
        1,
        CASTER,
    );
    assert_eq!(
        mp(&world, CASTER),
        0.0,
        "MP_BLOCK blocks restoration, not just drain"
    );
}

/// A dead target is not recharged (Java's `effected.isDead()` bail).
#[test]
fn a_dead_target_is_not_recharged() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _t = ingame_caster(&mut world, TCID, TARGET, 0, 0);
    empty_mp(&mut world, TARGET);
    world
        .objects
        .get_component_mut::<Vitals>(&TARGET)
        .unwrap()
        .dead = true;

    cast(
        &mut world,
        9712,
        vec![SkillEffect::ManaHeal { power: 500.0 }],
        1,
        TARGET,
    );
    assert_eq!(mp(&world, TARGET), 0.0, "the dead are not recharged");
}

/// `Stat::MANA_CHARGE` — Higher Mana Gain 285's flat bonus, read off the
/// **recipient**, not the caster.
#[test]
fn mana_charge_adds_to_the_recharged_amount() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _t = ingame_caster(&mut world, TCID, TARGET, 0, 0);

    empty_mp(&mut world, TARGET);
    cast(
        &mut world,
        9713,
        vec![SkillEffect::ManaHeal { power: 20.0 }],
        1,
        TARGET,
    );
    let bare = mp(&world, TARGET);

    // The bonus belongs to the *recipient*: put it on the target, not the caster.
    world
        .objects
        .get_component_mut::<StatModifiers>(&TARGET)
        .unwrap()
        .add
        .insert(Stat::ManaCharge, 22.0);
    empty_mp(&mut world, TARGET);
    cast(
        &mut world,
        9714,
        vec![SkillEffect::ManaHeal { power: 20.0 }],
        1,
        TARGET,
    );
    assert!(
        (mp(&world, TARGET) - bare - 22.0).abs() < 1e-9,
        "+22 flat: {bare} -> {}",
        mp(&world, TARGET)
    );

    // And it does nothing when it sits on the caster instead.
    world
        .objects
        .get_component_mut::<StatModifiers>(&TARGET)
        .unwrap()
        .add
        .clear();
    world
        .objects
        .get_component_mut::<StatModifiers>(&CASTER)
        .unwrap()
        .add
        .insert(Stat::ManaCharge, 22.0);
    empty_mp(&mut world, TARGET);
    cast(
        &mut world,
        9715,
        vec![SkillEffect::ManaHeal { power: 20.0 }],
        1,
        TARGET,
    );
    assert!(
        (mp(&world, TARGET) - bare).abs() < 1e-9,
        "the caster's own MANA_CHARGE is irrelevant"
    );
}

/// `ManaHealPercent` and `Mp`'s `PER` mode both scale off **max** MP.
#[test]
fn percent_restores_scale_off_max_mp() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let max_mp = world
        .objects
        .get_component::<Vitals>(&CASTER)
        .unwrap()
        .max_mp as f64;

    empty_mp(&mut world, CASTER);
    cast(
        &mut world,
        9720,
        vec![SkillEffect::ManaHealPercent { power: 25.0 }],
        1,
        CASTER,
    );
    assert!(
        (mp(&world, CASTER) - max_mp * 0.25).abs() < 1e-9,
        "25% of max"
    );

    empty_mp(&mut world, CASTER);
    cast(
        &mut world,
        9721,
        vec![SkillEffect::MpRestore {
            amount: 50.0,
            percent: true,
        }],
        1,
        CASTER,
    );
    assert!(
        (mp(&world, CASTER) - max_mp * 0.5).abs() < 1e-9,
        "Mp PER is also a share of max"
    );

    // DIFF mode is the flat reading of the same field.
    empty_mp(&mut world, CASTER);
    cast(
        &mut world,
        9722,
        vec![SkillEffect::MpRestore {
            amount: 7.0,
            percent: false,
        }],
        1,
        CASTER,
    );
    assert!((mp(&world, CASTER) - 7.0).abs() < 1e-9, "Mp DIFF is flat");
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// The three `ManaHealByLevel` skills carry **only** that effect — which is
/// exactly why all three were dropped whole. Pinning it keeps the regression
/// visible.
#[test]
fn the_recharge_skills_carry_only_the_restore_effect() {
    let skills = dist_skills();
    for id in [1013, 1126, 1428] {
        let skill = skills
            .get(id, 1)
            .unwrap_or_else(|| panic!("skill {id} loads"));
        assert_eq!(
            skill.effects.len(),
            1,
            "skill {id} has one effect: {:?}",
            skill.effects
        );
        assert!(matches!(skill.effects[0], SkillEffect::ManaHealByLevel { power } if power > 0.0));
    }
}

/// The rest of the family parses: `Mp` off `<amount>` (not `<power>`), and
/// Mortal Strike's `ManaHeal` alongside the `FatalBlowRate` it already had.
#[test]
fn the_rest_of_the_family_parses() {
    let skills = dist_skills();

    for id in [417, 1157] {
        let skill = skills.get(id, 1).unwrap();
        assert!(
            skill
                .effects
                .iter()
                .any(|e| matches!(e, SkillEffect::MpRestore { amount, .. } if *amount > 0.0)),
            "skill {id} reads its amount: {:?}",
            skill.effects
        );
    }

    // Mortal Strike 410 was described in this slice's plan as "the one
    // learnable `ManaHeal`". That was wrong, and the per-effect level gating
    // added afterwards proves it: its `ManaHeal` is
    // `fromSubLevel="2001" toSubLevel="2020"` — an **enchant-route** effect,
    // and this port has no enchanted skills. So `ManaHeal` has *zero*
    // reachable learnable skills here, and the slice's real reach was 6, not 7.
    let mortal = skills.get(410, 1).unwrap();
    assert!(
        !mortal
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::ManaHeal { .. })),
        "Mortal Strike's ManaHeal is enchant-only and must not apply: {:?}",
        mortal.effects
    );
    assert!(
        mortal
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::StatModifier(m) if m.stat == Stat::BlowRate)),
        "but its ungated FatalBlowRate is untouched"
    );
}

/// Higher Mana Gain 285 is a learnable passive granting `MANA_CHARGE`, so it
/// folds through the ordinary passive path into the `add` map the restore
/// reads. Without it the stat would have no source at all.
#[test]
fn higher_mana_gain_grants_the_mana_charge_stat() {
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let bare = Player::from_char(&world.data, &dummy_char(6101, "Bare"));
    assert_eq!(
        bare.stat_modifiers.add.get(&Stat::ManaCharge),
        None,
        "no skill, no stat"
    );

    let mut chr = dummy_char(6102, "Recharger");
    chr.skills = vec![(285, 1, 0)];
    let bundle = Player::from_char(&world.data, &chr);
    let add = bundle
        .stat_modifiers
        .add
        .get(&Stat::ManaCharge)
        .copied()
        .unwrap_or(0.0);
    assert!(
        (add - 22.0).abs() < 1e-9,
        "Higher Mana Gain lvl 1 is +22 flat, got {add}"
    );
}
