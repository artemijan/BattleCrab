//! Affect scopes (G19): the sweep that turns one primary target into the set a
//! skill actually lands on, the friend/foe filter, and toggle on/off.

use super::*;

use crate::game_loop::skills::affect::targets_affected;
use crate::model::components::Buffs;
use crate::model::skill::{AffectObject, AffectScope, OperateType, Skill, SkillEffect, StatModifierEffect, TargetType};
use crate::model::stats::{Stat, StatModifierType};

const CASTER: i32 = 2001;
const CID: u32 = 1;

/// A skill template the tests reshape per case.
fn aoe_skill(id: i32, scope: AffectScope, object: AffectObject, range: i32) -> Skill {
    Skill {
        without_action: false,
        item_consume_id: 0,
        item_consume_count: 0,
        id,
        level: 1,
        name: format!("Test {id}"),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Enemy,
        magic_type: 1,
        magic_level: 0,
        effect_point: -100, // bad skill
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
        affect_scope: scope,
        affect_object: object,
        affect_range: range,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: true,
        stay_after_death: false,
        effects: vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::PhysicalDefence,
            mode: StatModifierType::Per,
            amount: -10.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
        })],
        ..Default::default()
    }
}

/// Put three monsters around the origin: two inside a 200-unit sweep, one far
/// outside it.
fn spawn_cluster(world: &mut World) -> (i32, i32, i32) {
    let (a, b, far) = (NPC_OID, NPC_OID + 1, NPC_OID + 2);
    add_test_npc(world, a, 20001, "Monster", 5, 0, 0, 0);
    add_test_npc(world, b, 20001, "Monster", 5, 100, 0, 0);
    add_test_npc(world, far, 20001, "Monster", 5, 5000, 0, 0);
    (a, b, far)
}

// ---------------------------------------------------------------------------
// Scope sweeps
// ---------------------------------------------------------------------------

/// A SINGLE skill resolves to exactly its primary target, untouched by the new
/// machinery.
#[test]
fn single_scope_affects_only_the_target() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (a, _b, _far) = spawn_cluster(&mut world);
    let skill = aoe_skill(9001, AffectScope::Single, AffectObject::NotFriend, 200);

    assert_eq!(targets_affected(&mut world, CASTER, a, &skill), vec![a]);
}

/// RANGE sweeps everything within `affect_range` of the **target**, and drops
/// what lies outside it.
#[test]
fn range_scope_sweeps_around_the_target() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (a, b, far) = spawn_cluster(&mut world);
    let skill = aoe_skill(9002, AffectScope::Range, AffectObject::NotFriend, 200);

    let hit = targets_affected(&mut world, CASTER, a, &skill);
    assert_eq!(hit.first(), Some(&a), "primary target comes first");
    assert!(hit.contains(&b), "the mob 100 units away is swept up");
    assert!(!hit.contains(&far), "the mob 5000 units away is not");
}

/// `affect_range` 0 (a scope with no radius in the XML) degenerates to the
/// primary target rather than sweeping the world.
#[test]
fn zero_affect_range_hits_only_the_target() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (a, b, _far) = spawn_cluster(&mut world);
    let skill = aoe_skill(9003, AffectScope::Range, AffectObject::NotFriend, 0);

    let hit = targets_affected(&mut world, CASTER, a, &skill);
    assert_eq!(hit, vec![a]);
    assert!(!hit.contains(&b));
}

/// POINT_BLANK measures from the **caster**, not the target — so a mob near the
/// caster is caught even when the primary target is far away.
#[test]
fn point_blank_sweeps_around_the_caster() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    // `near` hugs the caster; the primary target sits 400 units off, beyond the
    // 200 sweep, so a target-centred scope would miss `near` entirely.
    let (near, target) = (NPC_OID, NPC_OID + 1);
    add_test_npc(&mut world, near, 20001, "Monster", 5, 50, 0, 0);
    add_test_npc(&mut world, target, 20001, "Monster", 5, 400, 0, 0);
    let skill = aoe_skill(9004, AffectScope::PointBlank, AffectObject::NotFriend, 200);

    let hit = targets_affected(&mut world, CASTER, target, &skill);
    assert!(hit.contains(&near), "caster-centred sweep catches the near mob");
    assert!(hit.contains(&target), "the primary target is always included");
}

/// `affectLimit` caps how many extra targets a sweep may pick up. The primary
/// target counts toward the cap, so a limit of 1 means "the target only".
#[test]
fn affect_limit_caps_the_sweep() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (a, b, _far) = spawn_cluster(&mut world);
    add_test_npc(&mut world, NPC_OID + 3, 20001, "Monster", 5, 120, 0, 0);

    let mut skill = aoe_skill(9005, AffectScope::Range, AffectObject::NotFriend, 300);
    skill.affect_limit = (1, 0); // min 1, no random spread
    assert_eq!(targets_affected(&mut world, CASTER, a, &skill).len(), 1);

    skill.affect_limit = (2, 0);
    assert_eq!(targets_affected(&mut world, CASTER, a, &skill).len(), 2);

    // Uncapped: the whole cluster inside 300 units.
    skill.affect_limit = (0, 0);
    let hit = targets_affected(&mut world, CASTER, a, &skill);
    assert!(hit.len() >= 3, "uncapped sweep picks up the cluster, got {}", hit.len());
    assert!(hit.contains(&b));
}

/// A dead creature is never swept into an ordinary AoE.
#[test]
fn dead_creatures_are_skipped() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (a, b, _far) = spawn_cluster(&mut world);
    world.objects.get_component_mut::<Vitals>(&b).unwrap().dead = true;

    let skill = aoe_skill(9006, AffectScope::Range, AffectObject::NotFriend, 300);
    let hit = targets_affected(&mut world, CASTER, a, &skill);
    assert!(!hit.contains(&b), "a corpse is not swept into a hostile AoE");
}

/// Drop two players into one party directly — the invite/accept packet dance
/// is exercised in `social_tests`; here only the resulting state matters.
fn put_in_party(world: &mut World, leader: i32, member: i32) {
    use crate::model::components::PartyRef;
    use crate::model::party::{LootRule, Party};

    let party_id = world.next_party_id;
    world.next_party_id += 1;
    let mut party = Party::new(leader, LootRule::FindersKeepers, 0);
    party.members.push(member);
    world.parties.insert(party_id, party);
    world.objects.add_components(&leader, PartyRef(party_id));
    world.objects.add_components(&member, PartyRef(party_id));
}

// ---------------------------------------------------------------------------
// Affect-object filtering
// ---------------------------------------------------------------------------

/// NOT_FRIEND keeps the caster out of their own offensive AoE even when they
/// stand inside its radius.
#[test]
fn not_friend_excludes_the_caster() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (a, _b, _far) = spawn_cluster(&mut world);
    let skill = aoe_skill(9007, AffectScope::Range, AffectObject::NotFriend, 500);

    let hit = targets_affected(&mut world, CASTER, a, &skill);
    assert!(!hit.contains(&CASTER), "the caster is not their own AoE victim");
}

/// A party mate is a "friend": a NOT_FRIEND AoE skips them, while a FRIEND
/// scope picks them up. This is the filter that stops an AoE nuke from
/// shredding your own party.
#[test]
fn party_mates_are_friends() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mate = 2002;
    let _out2 = ingame_caster(&mut world, 2, mate, 100, 0);
    let (a, _b, _far) = spawn_cluster(&mut world);

    put_in_party(&mut world, CASTER, mate);

    let hostile = aoe_skill(9008, AffectScope::Range, AffectObject::NotFriend, 500);
    let hit = targets_affected(&mut world, CASTER, a, &hostile);
    assert!(!hit.contains(&mate), "a party mate is not swept into a hostile AoE");

    let helpful = aoe_skill(9009, AffectScope::Range, AffectObject::Friend, 500);
    let hit = targets_affected(&mut world, CASTER, mate, &helpful);
    assert!(hit.contains(&mate), "a FRIEND scope does reach the party mate");
}

/// PARTY scope reaches the target's party regardless of who is nearby, and an
/// unpartied target is simply "a party of one".
#[test]
fn party_scope_covers_the_party() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mate = 2002;
    let _out2 = ingame_caster(&mut world, 2, mate, 100, 0);

    let mut skill = aoe_skill(9010, AffectScope::Party, AffectObject::Friend, 1000);
    skill.effect_point = 100; // a party buff is a good skill

    // Unpartied: only the target.
    assert_eq!(targets_affected(&mut world, CASTER, CASTER, &skill), vec![CASTER]);

    put_in_party(&mut world, CASTER, mate);
    let hit = targets_affected(&mut world, CASTER, CASTER, &skill);
    assert!(hit.contains(&CASTER) && hit.contains(&mate), "the whole party is covered");
}

// ---------------------------------------------------------------------------
// Toggles
// ---------------------------------------------------------------------------

fn toggle_skill(id: i32, group: i32) -> Skill {
    let mut s = aoe_skill(id, AffectScope::Single, AffectObject::All, 0);
    s.operate_type = OperateType::Toggle;
    s.target_type = TargetType::None_;
    s.effect_point = 0;
    s.abnormal_time = 0; // toggles carry no duration — on until switched off
    s.abnormal_type = format!("TOGGLE{id}");
    s.is_debuff = false;
    s.toggle_group_id = group;
    s
}

fn has_buff(world: &World, oid: i32, skill_id: i32) -> bool {
    world
        .objects
        .get_component::<Buffs>(&oid)
        .is_some_and(|b| b.0.iter().any(|x| x.skill_id == skill_id))
}

/// A toggle switches on when first used and **off** when used again — the
/// recast casts nothing, it just strips the effect.
#[test]
fn toggle_switches_on_and_off() {
    let (mut world, _db, _l) = cast_test_world();
    let skill = toggle_skill(9100, 0);
    world.data.skill_data.insert_for_test(skill);
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::components::SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(9100, 1);
    drain(&mut out);

    // On: toggles are instant, so the buff is live without advancing any ticks.
    crate::game_loop::skills::cast::use_magic(&mut world, CID, CASTER, 9100, false, false);
    assert!(has_buff(&world, CASTER, 9100), "first use switches the toggle on");
    assert!(
        !world.objects.has_component::<crate::model::components::Casting>(&CASTER),
        "a toggle is an instant cast — it never occupies the cast bar"
    );

    // Off: the recast strips it.
    crate::game_loop::skills::cast::use_magic(&mut world, CID, CASTER, 9100, false, false);
    assert!(!has_buff(&world, CASTER, 9100), "second use switches it back off");
}

/// Toggles sharing a `toggleGroupId` are mutually exclusive — switching one on
/// drops its siblings (`EffectList.stopAllTogglesOfGroup`).
#[test]
fn toggles_in_a_group_are_mutually_exclusive() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(toggle_skill(9101, 7));
    world.data.skill_data.insert_for_test(toggle_skill(9102, 7));
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    {
        let book = world
            .objects
            .get_component_mut::<crate::model::components::SkillBook>(&CASTER)
            .unwrap();
        book.0.insert(9101, 1);
        book.0.insert(9102, 1);
    }
    drain(&mut out);

    crate::game_loop::skills::cast::use_magic(&mut world, CID, CASTER, 9101, false, false);
    assert!(has_buff(&world, CASTER, 9101));

    // The sibling replaces it rather than stacking.
    crate::game_loop::skills::cast::use_magic(&mut world, CID, CASTER, 9102, false, false);
    assert!(has_buff(&world, CASTER, 9102), "the newly toggled skill is on");
    assert!(!has_buff(&world, CASTER, 9101), "its group sibling was switched off");
}

// ---------------------------------------------------------------------------
// End to end: the milestone gate
// ---------------------------------------------------------------------------

/// **G19 gate — "an AoE nuke hits a cluster".** Cast a real RANGE-scope nuke
/// through the whole pipeline (`use_magic` → cast phases → `handle_skill_finish`
/// → per-target effects) and confirm every mob in the sweep took damage while
/// the one outside it did not.
#[test]
fn aoe_nuke_damages_the_whole_cluster() {
    let (mut world, _db, _l) = cast_test_world();

    let mut nuke = aoe_skill(9200, AffectScope::Range, AffectObject::NotFriend, 300);
    nuke.effects = vec![SkillEffect::MagicalAttack { power: 50.0 }];
    nuke.hit_time = 100;
    world.data.skill_data.insert_for_test(nuke);

    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::components::SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(9200, 1);

    // Two mobs inside the 300 sweep of the primary target, one far outside.
    let (a, b, far) = spawn_cluster(&mut world);
    world.objects.get_component_mut::<TargetRef>(&CASTER).unwrap().0 = Some(a);
    let hp_before = |w: &World, oid: i32| w.objects.get_component::<Vitals>(&oid).unwrap().cur_hp;
    let (a0, b0, far0) = (hp_before(&world, a), hp_before(&world, b), hp_before(&world, far));
    drain(&mut out);

    crate::game_loop::skills::cast::use_magic(&mut world, CID, CASTER, 9200, true, false);
    // The cast is phased (not a toggle), so let it run to the finish.
    advance_ticks(&mut world, 60);

    assert!(hp_before(&world, a) < a0, "the primary target took damage");
    assert!(hp_before(&world, b) < b0, "the mob inside the sweep took damage too");
    assert_eq!(hp_before(&world, far), far0, "the mob outside the sweep was untouched");
}
