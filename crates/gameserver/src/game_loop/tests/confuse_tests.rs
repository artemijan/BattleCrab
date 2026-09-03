//! `Confuse` + `RandomizeHate` — turning a victim on a bystander (G19).
//!
//! Both were blocked on the same thing: the hate-effects slice deferred
//! `RandomizeHate` because it "needs a general nearby-visible-creatures query
//! `faction_call`'s NPC-only neighbour scan doesn't provide". Building
//! `visibility::visible_creatures` once unblocks both.
//!
//! Madness 1105, Curse Discord 1163 and Confusion 2 each carry **only** the
//! unported effect, so all three were dropped whole.

use super::*;

use crate::model::formulas;
use crate::model::npc::AggroList;
use crate::model::skill::effects::SkillEffect;

const CASTER: i32 = 7001;
const CID: u32 = 1;
const MOB_ID: i32 = 47000;
const ORC_A: i32 = 47001;
const ORC_B: i32 = 47002;
const VICTIM_OID: i32 = NPC_OID;
const BYSTANDER_OID: i32 = NPC_OID + 1;

fn mob_template(id: i32, clans: &[&str]) -> crate::data::npc_data::NpcTemplate {
    let mut t = crate::data::npc_data::default_template(id);
    t.type_name = "Monster".into();
    t.name = format!("Mob {id}");
    t.level = 5;
    t.base_hp_max = 500.0;
    t.collision_radius = 10.0;
    t.clans = clans.iter().map(|s| s.to_string()).collect();
    t
}

fn confuse_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    world
        .data
        .npc_data
        .insert_for_test(mob_template(MOB_ID, &[]));
    world
        .data
        .npc_data
        .insert_for_test(mob_template(ORC_A, &["ORC"]));
    world
        .data
        .npc_data
        .insert_for_test(mob_template(ORC_B, &["ORC"]));
    (world, db, l)
}

fn set_hate(world: &mut World, npc: i32, target: i32, hate: f64) {
    world
        .objects
        .get_component_mut::<AggroList>(&npc)
        .unwrap()
        .0
        .entry(target)
        .or_default()
        .hate = hate;
}

/// Cast a one-effect skill from `CASTER` at `target`. `magic_level` feeds the
/// `calcProbability` gate.
fn cast(
    world: &mut World,
    skill_id: i32,
    effects: Vec<SkillEffect>,
    magic_level: i32,
    target: i32,
) {
    use crate::model::skill::Skill;
    use crate::model::skill::target::{AffectObject, AffectScope, OperateType, TargetType};
    let skill = Skill {
        self_continuous: false,
        without_action: false,
        trait_type: model::skill::traits::TraitType::None,
        item_consume_id: 0,
        item_consume_count: 0,
        id: skill_id,
        level: 1,
        name: format!("C{skill_id}"),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Enemy,
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
        is_debuff: true,
        stay_after_death: false,
        effects,
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(skill.clone());
    effects::apply_skill_effects(world, CASTER, target, &skill);
}

// ---------------------------------------------------------------------------
// The chance gate
// ---------------------------------------------------------------------------

/// `magicLevel + chance - targetLevel`, compared unclamped against a d100 —
/// so a high-level target can push the threshold to zero or below and the
/// effect simply never lands.
#[test]
fn calc_probability_is_magic_level_plus_chance_minus_target_level() {
    // 40 + 80 - 20 = 100 → every roll 0..99 lands.
    assert!(formulas::magic::calc_probability(
        40, 80, 20, 0.0, 1.0, 1.0, 99
    ));
    // 40 + 80 - 120 = 0 → nothing lands, not even roll 0.
    assert!(!formulas::magic::calc_probability(
        40, 80, 120, 0.0, 1.0, 1.0, 0
    ));
    // Boundary: threshold 50, roll 49 lands and 50 does not (strict `<`).
    assert!(formulas::magic::calc_probability(
        30, 40, 20, 0.0, 1.0, 1.0, 49
    ));
    assert!(!formulas::magic::calc_probability(
        30, 40, 20, 0.0, 1.0, 1.0, 50
    ));
}

/// The attribute and trait bonuses scale that threshold. They used to be 1.0
/// for everyone this port modelled — that stopped being true once the attribute
/// and trait tables landed, so they are real inputs now.
#[test]
fn calc_probability_scales_by_the_attribute_and_trait_bonuses() {
    // Threshold 50. A 1.25 attribute bonus lifts it to 62.5, so roll 62 lands…
    assert!(formulas::magic::calc_probability(
        30, 40, 20, 0.0, 1.25, 1.0, 62
    ));
    assert!(!formulas::magic::calc_probability(
        30, 40, 20, 0.0, 1.25, 1.0, 63
    ));
    // …and a 0.5 trait resistance halves it to 25.
    assert!(formulas::magic::calc_probability(
        30, 40, 20, 0.0, 1.0, 0.5, 24
    ));
    assert!(!formulas::magic::calc_probability(
        30, 40, 20, 0.0, 1.0, 0.5, 25
    ));
    // They compose, and an invulnerable trait (0) means nothing ever lands.
    assert!(!formulas::magic::calc_probability(
        30, 40, 20, 0.0, 1.25, 0.0, 0
    ));
}

/// The gate is real end to end: an out-of-reach target is never confused.
#[test]
fn a_far_higher_level_target_is_never_confused() {
    let (mut world, _db, _l) = confuse_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, VICTIM_OID, MOB_ID, "Monster", 5, 100, 0, 0);
    add_test_npc(&mut world, BYSTANDER_OID, MOB_ID, "Monster", 5, 150, 0, 0);
    // Skill magicLevel 1, chance 1, victim level 5 → threshold -3.
    cast(
        &mut world,
        9800,
        vec![SkillEffect::Confuse { chance: 1 }],
        1,
        VICTIM_OID,
    );
    assert_eq!(
        hate_on(&world, VICTIM_OID, BYSTANDER_OID),
        0.0,
        "the gate refused it"
    );
}

// ---------------------------------------------------------------------------
// Confuse
// ---------------------------------------------------------------------------

/// The victim turns on a bystander instead of whoever it was fighting.
///
/// The candidate list is `[CASTER, BYSTANDER_OID]` sorted by object id (NPC
/// ids start far above player ids), so forcing the index roll to 1 picks the
/// bystander deterministically — otherwise the pick is a coin flip and the
/// assertion below would be flaky.
#[test]
fn a_confused_mob_turns_on_a_bystander() {
    let (mut world, _db, _l) = confuse_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, VICTIM_OID, MOB_ID, "Monster", 5, 100, 0, 0);
    add_test_npc(&mut world, BYSTANDER_OID, MOB_ID, "Monster", 5, 150, 0, 0);
    // The victim is currently fixated on the caster.
    set_hate(&mut world, VICTIM_OID, CASTER, 500.0);

    // Three rolls are consumed, in this order:
    //   1. `apply_skill_effects`' unconditional per-cast magic-crit `roll(1000)`
    //   2. `Confuse`'s chance gate, `roll(100)`
    //   3. the candidate index, `roll(len)`
    //
    // The first one is easy to miss — it is charged before any effect runs. An
    // earlier version of this test forced only two values, so the *index* fell
    // through to the real RNG and the assertion below passed or failed on a
    // coin flip. It happened to pass for two slices before a later change
    // shifted the draw and exposed it.
    world.force_rolls([0, 0, 1]);
    cast(
        &mut world,
        9801,
        vec![SkillEffect::Confuse { chance: 100 }],
        80,
        VICTIM_OID,
    );

    let on_bystander = hate_on(&world, VICTIM_OID, BYSTANDER_OID);
    let on_caster = hate_on(&world, VICTIM_OID, CASTER);
    assert!(
        on_bystander > on_caster,
        "the bystander is now the most hated ({on_bystander} vs {on_caster})"
    );
    assert_eq!(
        world
            .objects
            .get_component::<NpcAi>(&VICTIM_OID)
            .unwrap()
            .intention,
        NpcIntention::Attack,
        "and the mob is woken into an attack"
    );
}

/// `Confuse` does **not** clear the original hate — Java only adds a new
/// target, unlike `RandomizeHate` which moves it. Worth pinning because the two
/// effects look interchangeable at a glance.
#[test]
fn confuse_adds_a_target_without_erasing_the_old_hate() {
    let (mut world, _db, _l) = confuse_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, VICTIM_OID, MOB_ID, "Monster", 5, 100, 0, 0);
    add_test_npc(&mut world, BYSTANDER_OID, MOB_ID, "Monster", 5, 150, 0, 0);
    set_hate(&mut world, VICTIM_OID, CASTER, 500.0);

    cast(
        &mut world,
        9802,
        vec![SkillEffect::Confuse { chance: 100 }],
        80,
        VICTIM_OID,
    );

    // Which of the two visible creatures the roll picks is not fixed, so the
    // invariant asserted here is the one that holds either way: `Confuse` only
    // ever *adds* a target, so the caster's entry survives at its original
    // value or higher. (`RandomizeHate`, below, is the one that removes it.)
    assert!(
        hate_on(&world, VICTIM_OID, CASTER) >= 500.0,
        "Confuse never erases existing hate, got {}",
        hate_on(&world, VICTIM_OID, CASTER)
    );
    assert!(
        world
            .objects
            .get_component::<AggroList>(&VICTIM_OID)
            .unwrap()
            .0
            .contains_key(&CASTER),
        "and the caster stays on the aggro list"
    );
}

/// With nobody else around there is nothing to turn on, and the effect is a
/// no-op rather than a panic or a self-target.
#[test]
fn confuse_with_no_bystanders_does_nothing() {
    let (mut world, _db, _l) = confuse_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, VICTIM_OID, MOB_ID, "Monster", 5, 100, 0, 0);
    set_hate(&mut world, VICTIM_OID, CASTER, 500.0);

    cast(
        &mut world,
        9803,
        vec![SkillEffect::Confuse { chance: 100 }],
        80,
        VICTIM_OID,
    );

    // The only visible creature is the caster, so that is who it picks — and
    // the hate simply becomes dominant, which it already was.
    assert!(
        hate_on(&world, VICTIM_OID, CASTER) >= 500.0,
        "no crash, no self-target"
    );
    assert_eq!(
        hate_on(&world, VICTIM_OID, VICTIM_OID),
        0.0,
        "the victim never targets itself"
    );
}

// ---------------------------------------------------------------------------
// RandomizeHate
// ---------------------------------------------------------------------------

/// The caster's hate is **moved**, not copied: they drop off the aggro list
/// entirely and the bystander inherits the whole amount.
#[test]
fn randomize_hate_moves_the_casters_hate_to_a_bystander() {
    let (mut world, _db, _l) = confuse_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, VICTIM_OID, MOB_ID, "Monster", 5, 100, 0, 0);
    add_test_npc(&mut world, BYSTANDER_OID, MOB_ID, "Monster", 5, 150, 0, 0);
    set_hate(&mut world, VICTIM_OID, CASTER, 500.0);

    cast(
        &mut world,
        9810,
        vec![SkillEffect::RandomizeHate { chance: 100 }],
        80,
        VICTIM_OID,
    );

    assert_eq!(
        hate_on(&world, VICTIM_OID, CASTER),
        0.0,
        "the caster is off the list entirely"
    );
    assert_eq!(
        hate_on(&world, VICTIM_OID, BYSTANDER_OID),
        500.0,
        "and the bystander inherited all of it"
    );
}

/// "Aggro cannot be transfered to a mob of the same faction" — with the only
/// bystander being a clan-mate, there is no valid recipient and the hate stays
/// put. This is the exclusion `Confuse` does *not* have.
#[test]
fn randomize_hate_refuses_to_pass_aggro_to_a_clan_mate() {
    let (mut world, _db, _l) = confuse_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, VICTIM_OID, ORC_A, "Monster", 5, 100, 0, 0);
    add_test_npc(&mut world, BYSTANDER_OID, ORC_B, "Monster", 5, 150, 0, 0);
    set_hate(&mut world, VICTIM_OID, CASTER, 500.0);

    cast(
        &mut world,
        9811,
        vec![SkillEffect::RandomizeHate { chance: 100 }],
        80,
        VICTIM_OID,
    );

    assert_eq!(
        hate_on(&world, VICTIM_OID, CASTER),
        500.0,
        "no valid recipient, so nothing moved"
    );
    assert_eq!(
        hate_on(&world, VICTIM_OID, BYSTANDER_OID),
        0.0,
        "the clan-mate is never handed the aggro"
    );
}

/// Java bails when the effected is not an `Attackable` — a player cannot have
/// their "hate" randomized.
#[test]
fn randomize_hate_ignores_a_player_target() {
    let (mut world, _db, _l) = confuse_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, 2, 7002, 100, 0);
    add_test_npc(&mut world, BYSTANDER_OID, MOB_ID, "Monster", 5, 150, 0, 0);

    // Must not panic, and must not invent an aggro list for the player.
    cast(
        &mut world,
        9812,
        vec![SkillEffect::RandomizeHate { chance: 100 }],
        80,
        7002,
    );
    assert!(
        world.objects.get_component::<AggroList>(&7002).is_none(),
        "players have no aggro list to shuffle"
    );
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// The three skills that carry *only* one of these effects — the ones that were
/// dropped whole. Confusion 2's chance is the real 80, and the Confuse skills
/// default to 100 (they declare no `<chance>` at all).
#[test]
fn real_dist_skills_parse_with_their_chances() {
    let skills = dist::skills();

    // Every one of these declares a real `<chance>` — **none** falls back to
    // the parser's default of 100, so the default is exercised by no shipped
    // skill on this dist.
    for (id, chance) in [(1105, 20), (1163, 20), (1213, 60)] {
        let skill = skills
            .get(id, 1)
            .unwrap_or_else(|| panic!("skill {id} loads"));
        let got = skill.effects.iter().find_map(|e| match e {
            SkillEffect::Confuse { chance } => Some(*chance),
            _ => None,
        });
        assert_eq!(got, Some(chance), "skill {id} carries Confuse at {chance}%");
    }
    // Madness and Curse Discord carry *only* Confuse — the reason both were
    // dropped whole before this slice.
    for id in [1105, 1163] {
        assert_eq!(
            skills.get(id, 1).unwrap().effects.len(),
            1,
            "skill {id} has exactly one effect"
        );
    }

    let confusion = skills.get(2, 1).expect("Confusion loads");
    assert_eq!(
        confusion.effects.len(),
        1,
        "Confusion carries only RandomizeHate"
    );
    assert!(
        matches!(
            confusion.effects[0],
            SkillEffect::RandomizeHate { chance: 80 }
        ),
        "with its real 80% chance"
    );

    // Switch 12 pairs it with the already-ported TargetCancel, which must survive.
    let switch = skills.get(12, 1).expect("Switch loads");
    assert!(
        switch
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::RandomizeHate { chance: 80 }))
    );
    assert!(
        switch
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::TargetCancel { .. })),
        "TargetCancel survives"
    );
}

/// The `abnormalTime="20"` **attribute** on Madness/Curse Discord/Seal of
/// Mirage's `<effect>` element is dead data: Java's `parseNamedParamInfo` reads
/// only `name`/`level`/`from|toLevel`/`sub*Level` off an effect, so the
/// attribute is silently ignored — and none of these skills has an
/// `<abnormalTime>` *child*, which is where a real duration would live.
///
/// That is what makes `effect_flag::CONFUSED` unreachable: with no duration
/// there is no buff for an instant effect's flag to persist in.
#[test]
fn confuse_skills_have_no_real_abnormal_time() {
    let skills = dist::skills();
    for id in [1105, 1163, 1213] {
        assert_eq!(
            skills.get(id, 1).unwrap().abnormal_time,
            0,
            "skill {id} has no duration"
        );
    }
}

/// **`calcProbability` subtracts the target's abnormal resistance** —
/// `getAbnormalResist(skill.getBasicProperty(), target)`, inside the
/// parenthesis and ahead of both multipliers. The same stat the debuff
/// land-rate already honoured; Confuse and Randomize Hate had been ignoring it.
#[test]
fn abnormal_resistance_lowers_the_confuse_chance() {
    use crate::model::formulas;

    // magicLevel 30 + chance 40 − targetLevel 20 = 50, so a roll of 49 lands
    // and 50 does not.
    assert!(formulas::magic::calc_probability(
        30, 40, 20, 0.0, 1.0, 1.0, 49
    ));
    assert!(!formulas::magic::calc_probability(
        30, 40, 20, 0.0, 1.0, 1.0, 50
    ));

    // 20 points of abnormal resistance take the threshold to 30.
    assert!(formulas::magic::calc_probability(
        30, 40, 20, 20.0, 1.0, 1.0, 29
    ));
    assert!(
        !formulas::magic::calc_probability(30, 40, 20, 20.0, 1.0, 1.0, 30),
        "the resistance is subtracted before the multipliers, not after"
    );

    // …and it is inside the parenthesis: with an attribute bonus of 2 the
    // resisted threshold is (50 − 20)·2 = 60, not 50·2 − 20 = 80.
    assert!(formulas::magic::calc_probability(
        30, 40, 20, 20.0, 2.0, 1.0, 59
    ));
    assert!(!formulas::magic::calc_probability(
        30, 40, 20, 20.0, 2.0, 1.0, 60
    ));
}
