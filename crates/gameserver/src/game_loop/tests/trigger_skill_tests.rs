//! `TriggerSkillByAttack` — chance-on-hit skill triggers (G19).
//!
//! Sword/Blunt Weapon Mastery 205, Dagger Mastery 209 and Dance of Shadows 366
//! all carry one. Each fires a short self- or party-buff when the carrier lands
//! a qualifying hit; before this slice the effect was unparsed, so the masteries
//! were passives whose on-hit half did nothing.

use super::*;

use crate::model::components::{Buffs, SkillBook};
use crate::model::skill::SkillEffect;

const PLAYER: i32 = 8001;
const CID: u32 = 1;
const MOB_ID: i32 = 48000;
const MOB_OID: i32 = NPC_OID;
const CARRIER: i32 = 9900;
const TRIGGERED: i32 = 9901;
const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

fn trigger_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    let mut t = crate::data::npc_data::default_template(MOB_ID);
    t.type_name = "Monster".into();
    t.name = "Dummy".into();
    t.level = 5;
    t.base_hp_max = 5000.0;
    t.collision_radius = 10.0;
    world.data.npc_data.insert_for_test(t);
    (world, db, l)
}

/// Build a carrier skill holding one `TriggerSkillByAttack`, plus the skill it
/// triggers (a plain 60s stat buff, so "did it fire" is just "is the buff up").
fn install(world: &mut World, effect: SkillEffect) {
    use crate::model::skill::{
        AffectObject, AffectScope, OperateType, Skill, StatModifierEffect, TargetType,
    };
    use crate::model::stats::{Stat, StatModifierType};
    let base = |id: i32, effects: Vec<SkillEffect>, abnormal_time: i32, op: OperateType| Skill {
        self_continuous: false,
        without_action: false,
        trait_type: crate::model::skill::TraitType::None,
        item_consume_id: 0,
        item_consume_count: 0,
        id,
        level: 1,
        name: format!("T{id}"),
        operate_type: op,
        is_continuous: false,
        target_type: TargetType::Self_,
        magic_type: 0,
        magic_level: 1,
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
        abnormal_time,
        abnormal_level: 1,
        abnormal_type: format!("TRIG{id}"),
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
    world
        .data
        .skill_data
        .insert_for_test(base(CARRIER, vec![effect], 0, OperateType::Passive));
    world.data.skill_data.insert_for_test(base(
        TRIGGERED,
        vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::PhysicalAttack,
            mode: StatModifierType::Diff,
            amount: 10.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
        })],
        60,
        OperateType::Active,
    ));
}

fn a_trigger(chance: i32, is_critical: bool) -> SkillEffect {
    SkillEffect::TriggerSkillByAttack {
        min_damage: 1,
        chance,
        skill_id: TRIGGERED,
        skill_level: 1,
        on_party: false,
        is_critical,
        allow_weapons: 0,
    }
}

fn know(world: &mut World, oid: i32) {
    world
        .objects
        .get_component_mut::<SkillBook>(&oid)
        .unwrap()
        .0
        .insert(CARRIER, 1);
}

fn triggered(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<Buffs>(&oid)
        .is_some_and(|b| b.0.iter().any(|x| x.skill_id == TRIGGERED))
}

/// Land one normal hit of `damage`, critical or not.
fn hit(world: &mut World, damage: i32, crit: bool) {
    crate::game_loop::combat::handle_attack_hit(world, PLAYER, MOB_OID, damage, false, crit, 0);
}

// ---------------------------------------------------------------------------
// Firing
// ---------------------------------------------------------------------------

/// The baseline: a carrier with a 100% non-crit trigger fires on an ordinary
/// hit, and the triggered skill's buff really lands.
#[test]
fn a_qualifying_hit_fires_the_trigger() {
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(&mut world, a_trigger(100, false));
    know(&mut world, PLAYER);

    assert!(!triggered(&world, PLAYER), "nothing up before the hit");
    hit(&mut world, 50, false);
    assert!(
        triggered(&world, PLAYER),
        "the trigger fired and its buff landed"
    );
}

/// Without the carrier skill nothing fires — proving the trigger comes from the
/// attacker's skill book and not from the attack itself.
#[test]
fn an_attacker_without_the_carrier_triggers_nothing() {
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(&mut world, a_trigger(100, false));
    // deliberately not `know(...)`

    hit(&mut world, 50, false);
    assert!(!triggered(&world, PLAYER), "no carrier, no trigger");
}

/// **`isCritical` is an equality test, not a minimum.** An `isCritical=false`
/// trigger fires only on *non*-crits; `isCritical=true` only on crits. Dance of
/// Shadows 366 ships one of each, so getting this backwards would silently
/// halve or double it.
#[test]
fn is_critical_matches_the_hit_exactly() {
    // isCritical=false: fires on a normal hit, not on a crit.
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(&mut world, a_trigger(100, false));
    know(&mut world, PLAYER);
    hit(&mut world, 50, true);
    assert!(
        !triggered(&world, PLAYER),
        "a non-crit trigger must not fire on a crit"
    );
    hit(&mut world, 50, false);
    assert!(triggered(&world, PLAYER), "but does on a normal hit");

    // isCritical=true: the mirror image.
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(&mut world, a_trigger(100, true));
    know(&mut world, PLAYER);
    hit(&mut world, 50, false);
    assert!(
        !triggered(&world, PLAYER),
        "a crit trigger must not fire on a normal hit"
    );
    hit(&mut world, 50, true);
    assert!(triggered(&world, PLAYER), "but does on a crit");
}

/// `damage < minDamage` bails — a hit that scratches for less than the floor
/// never triggers.
#[test]
fn a_hit_below_min_damage_does_not_fire() {
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(
        &mut world,
        SkillEffect::TriggerSkillByAttack {
            min_damage: 100,
            chance: 100,
            skill_id: TRIGGERED,
            skill_level: 1,
            on_party: false,
            is_critical: false,
            allow_weapons: 0,
        },
    );
    know(&mut world, PLAYER);

    hit(&mut world, 99, false);
    assert!(!triggered(&world, PLAYER), "99 < minDamage 100");
    hit(&mut world, 100, false);
    assert!(
        triggered(&world, PLAYER),
        "100 meets it (the check is `<`, so the floor itself qualifies)"
    );
}

/// `chance == 0` is Java's explicit early bail, distinct from "rolled and lost".
#[test]
fn a_zero_chance_trigger_never_fires() {
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(&mut world, a_trigger(0, false));
    know(&mut world, PLAYER);

    for _ in 0..20 {
        hit(&mut world, 50, false);
    }
    assert!(!triggered(&world, PLAYER), "chance 0 never fires");
}

/// The refresh guard: Java re-casts only when the buff is absent or at a lower
/// level, so a trigger that is already up is not spammed on every swing.
#[test]
fn an_already_active_trigger_is_not_recast() {
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(&mut world, a_trigger(100, false));
    know(&mut world, PLAYER);

    hit(&mut world, 50, false);
    let after_first = world
        .objects
        .get_component::<Buffs>(&PLAYER)
        .unwrap()
        .0
        .len();
    for _ in 0..5 {
        hit(&mut world, 50, false);
    }
    let after_many = world
        .objects
        .get_component::<Buffs>(&PLAYER)
        .unwrap()
        .0
        .len();
    assert_eq!(
        after_first, after_many,
        "the buff is not re-applied while it is already up"
    );
}

/// A creature never triggers off hitting itself (`attacker == target` bails).
#[test]
fn a_self_hit_never_triggers() {
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    install(&mut world, a_trigger(100, false));
    know(&mut world, PLAYER);

    crate::game_loop::combat::handle_attack_hit(&mut world, PLAYER, PLAYER, 50, false, false, 0);
    assert!(!triggered(&world, PLAYER), "self-hits are excluded");
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// The three learnable carriers parse with their real parameters, including the
/// `fromLevel="9"` gating the previous slice added — 205 and 209 only gain the
/// trigger from level 9.
#[test]
fn real_dist_carriers_parse() {
    let skills = crate::data::skill_data::SkillData::load_from(DIST);

    let trigger_of = |id: i32, level: i32| {
        skills.get(id, level).and_then(|s| {
            s.effects.iter().find_map(|e| match e {
                SkillEffect::TriggerSkillByAttack {
                    chance,
                    skill_id,
                    is_critical,
                    allow_weapons,
                    ..
                } => Some((*chance, *skill_id, *is_critical, *allow_weapons != 0)),
                _ => None,
            })
        })
    };

    // Sword/Blunt Weapon Mastery 205: 50% on a crit, sword/blunt only, casts 5604.
    assert_eq!(trigger_of(205, 9), Some((50, 5604, true, true)));
    // Dagger Mastery 209: 33% on a crit, dagger only, casts 5603.
    assert_eq!(trigger_of(209, 9), Some((33, 5603, true, true)));
    // Both are `fromLevel="9"`, so level 8 has no trigger at all.
    assert_eq!(trigger_of(205, 8), None, "the level gate still applies");
    assert_eq!(trigger_of(209, 8), None);
}

/// The skills these actually cast carry ported effects, so the trigger has a
/// visible result rather than landing an empty buff: Dagger Mastery's 5603
/// grants `FatalBlowRate` for 5 seconds.
#[test]
fn the_triggered_skills_carry_real_effects() {
    let skills = crate::data::skill_data::SkillData::load_from(DIST);
    let dagger = skills.get(5603, 1).expect("Dagger Mastery buff loads");
    assert!(
        dagger
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::StatModifier(m) if m.stat == crate::model::stats::Stat::BlowRate)),
        "5603 grants FatalBlowRate: {:?}",
        dagger.effects
    );
    assert_eq!(dagger.abnormal_time, 5, "for 5 seconds");
}
