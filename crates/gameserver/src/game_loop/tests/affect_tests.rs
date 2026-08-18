//! Affect scopes (G19): the sweep that turns one primary target into the set a
//! skill actually lands on, the friend/foe filter, and toggle on/off.

use super::*;
use crate::game_loop::abnormal::has_buff;

use crate::game_loop::skills::affect::targets_affected;
use crate::model::skill::{
    AffectObject, AffectScope, OperateType, Skill, SkillEffect, StatModifierEffect, TargetType,
};
use crate::model::stats::{Stat, StatModifierType};

const CASTER: i32 = 2001;
const CID: u32 = 1;

/// A skill template the tests reshape per case.
fn aoe_skill(id: i32, scope: AffectScope, object: AffectObject, range: i32) -> Skill {
    Skill {
        self_continuous: false,
        without_action: false,
        trait_type: model::skill::TraitType::None,
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

/// POINT_BLANK rings the **target** — `PointBlank.java` is
/// `forEachVisibleObjectInRange(target, …)`, the same reference object
/// `Range.java` uses — and, unlike RANGE, the target itself is left out of its
/// own blast. `Range.java` carries an explicit "Add object of origin since its
/// skipped in the forEachVisibleObjectInRange method"; `PointBlank.java` has no
/// such line.
///
/// This test used to assert a caster-centred ring, which is what the port
/// believed. That reading is invisible for the 757 `SELF` point-blank skills
/// (target *is* the caster) and wrong for the 19 that aren't.
#[test]
fn point_blank_rings_the_target_and_spares_it() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    // The target sits 400 off; `beside` hugs *it*, `near_caster` hugs the
    // caster instead. Only the first is inside a target-centred 200 ring.
    let (near_caster, target, beside) = (NPC_OID, NPC_OID + 1, NPC_OID + 2);
    add_test_npc(&mut world, near_caster, 20001, "Monster", 5, 50, 0, 0);
    add_test_npc(&mut world, target, 20001, "Monster", 5, 400, 0, 0);
    add_test_npc(&mut world, beside, 20001, "Monster", 5, 450, 0, 0);
    let skill = aoe_skill(9004, AffectScope::PointBlank, AffectObject::NotFriend, 200);

    let hit = targets_affected(&mut world, CASTER, target, &skill);
    assert!(hit.contains(&beside), "50 units from the target is inside");
    assert!(
        !hit.contains(&near_caster),
        "350 units from the target is outside — the ring is not caster-centred"
    );
    assert!(
        !hit.contains(&target),
        "the object at the centre is spared: PointBlank.java never re-adds it"
    );
}

/// The `SELF` + `POINT_BLANK` shape — 757 of this dist's 786 point-blank
/// skills, and the one Catherok's Stun (4072, `affectRange` 150) uses. Once
/// `npc_cast` resolves `SELF` to the caster, the ring sits on the mob: whoever
/// is standing on it is stunned and anyone beyond `affectRange` is not, no
/// matter who the AI was aiming at.
#[test]
fn self_point_blank_rings_the_caster_only_within_range() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (adjacent, distant) = (NPC_OID, NPC_OID + 1);
    add_test_npc(&mut world, adjacent, 20001, "Monster", 5, 120, 0, 0);
    add_test_npc(&mut world, distant, 20001, "Monster", 5, 400, 0, 0);
    let skill = aoe_skill(9014, AffectScope::PointBlank, AffectObject::NotFriend, 200);

    // `SELF` resolves target == caster.
    let hit = targets_affected(&mut world, CASTER, CASTER, &skill);
    assert!(hit.contains(&adjacent), "120 units is inside the 200 ring");
    assert!(!hit.contains(&distant), "400 units is not");
    assert!(!hit.contains(&CASTER), "the caster does not blast itself");
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
    assert!(
        hit.len() >= 3,
        "uncapped sweep picks up the cluster, got {}",
        hit.len()
    );
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
    assert!(
        !hit.contains(&b),
        "a corpse is not swept into a hostile AoE"
    );
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
    assert!(
        !hit.contains(&CASTER),
        "the caster is not their own AoE victim"
    );
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
    assert!(
        !hit.contains(&mate),
        "a party mate is not swept into a hostile AoE"
    );

    let helpful = aoe_skill(9009, AffectScope::Range, AffectObject::Friend, 500);
    let hit = targets_affected(&mut world, CASTER, mate, &helpful);
    assert!(
        hit.contains(&mate),
        "a FRIEND scope does reach the party mate"
    );
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
    assert_eq!(
        targets_affected(&mut world, CASTER, CASTER, &skill),
        vec![CASTER]
    );

    put_in_party(&mut world, CASTER, mate);
    let hit = targets_affected(&mut world, CASTER, CASTER, &skill);
    assert!(
        hit.contains(&CASTER) && hit.contains(&mate),
        "the whole party is covered"
    );
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
        .get_component_mut::<SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(9100, 1);
    drain(&mut out);

    // On: toggles are instant, so the buff is live without advancing any ticks.
    use_magic(&mut world, CID, CASTER, 9100, false, false);
    assert!(
        has_buff(&world, CASTER, 9100),
        "first use switches the toggle on"
    );
    assert!(
        !world.objects.has_component::<Casting>(&CASTER),
        "a toggle is an instant cast — it never occupies the cast bar"
    );

    // Off: the recast strips it.
    use_magic(&mut world, CID, CASTER, 9100, false, false);
    assert!(
        !has_buff(&world, CASTER, 9100),
        "second use switches it back off"
    );
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
            .get_component_mut::<SkillBook>(&CASTER)
            .unwrap();
        book.0.insert(9101, 1);
        book.0.insert(9102, 1);
    }
    drain(&mut out);

    use_magic(&mut world, CID, CASTER, 9101, false, false);
    assert!(has_buff(&world, CASTER, 9101));

    // The sibling replaces it rather than stacking.
    use_magic(&mut world, CID, CASTER, 9102, false, false);
    assert!(
        has_buff(&world, CASTER, 9102),
        "the newly toggled skill is on"
    );
    assert!(
        !has_buff(&world, CASTER, 9101),
        "its group sibling was switched off"
    );
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
        .get_component_mut::<SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(9200, 1);

    // Two mobs inside the 300 sweep of the primary target, one far outside.
    let (a, b, far) = spawn_cluster(&mut world);
    world
        .objects
        .get_component_mut::<TargetRef>(&CASTER)
        .unwrap()
        .0 = Some(a);
    let hp_before = |w: &World, oid: i32| w.objects.get_component::<Vitals>(&oid).unwrap().cur_hp;
    let (a0, b0, far0) = (
        hp_before(&world, a),
        hp_before(&world, b),
        hp_before(&world, far),
    );
    drain(&mut out);

    use_magic(&mut world, CID, CASTER, 9200, true, false);
    // The cast is phased (not a toggle), so let it run to the finish.
    advance_ticks(&mut world, 60);

    assert!(hp_before(&world, a) < a0, "the primary target took damage");
    assert!(
        hp_before(&world, b) < b0,
        "the mob inside the sweep took damage too"
    );
    assert_eq!(
        hp_before(&world, far),
        far0,
        "the mob outside the sweep was untouched"
    );
}

// ---------------------------------------------------------------------------
// The DEAD_* family — mass resurrection (G19 sweep)
// ---------------------------------------------------------------------------

/// **`DEAD_PLEDGE` is the Bishop's Mass Resurrection**, and it is the only
/// learnable skill in the whole previously-unported scope set. It picks up the
/// caster's *dead* clan mates in range — and nobody else.
///
/// The three things that make it different from `PLEDGE` are all asserted
/// here: the living are skipped, the origin (a `SELF` cast by a living caster)
/// is filtered out rather than assumed in, and outsiders' corpses don't count.
#[test]
fn dead_pledge_scope_gathers_only_the_clans_corpses() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let dead_mate = 2002;
    let live_mate = 2003;
    let dead_outsider = 2004;
    let _o2 = ingame_caster(&mut world, 2, dead_mate, 100, 0);
    let _o3 = ingame_caster(&mut world, 3, live_mate, 150, 0);
    let _o4 = ingame_caster(&mut world, 4, dead_outsider, 200, 0);
    for oid in [CASTER, dead_mate, live_mate] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .unwrap()
            .clan_id = 77;
    }
    world
        .objects
        .get_component_mut::<Player>(&dead_outsider)
        .unwrap()
        .clan_id = 88;
    for oid in [dead_mate, dead_outsider] {
        world
            .objects
            .get_component_mut::<Vitals>(&oid)
            .unwrap()
            .dead = true;
    }

    let mut skill = aoe_skill(1254, AffectScope::DeadPledge, AffectObject::All, 1000);
    skill.target_type = TargetType::Self_;
    skill.effect_point = 290; // a good skill

    let hit = targets_affected(&mut world, CASTER, CASTER, &skill);
    assert_eq!(
        hit,
        vec![dead_mate],
        "only the dead clan mate: the caster is alive, the live mate is alive, \
         and the outsider's corpse is another clan's problem"
    );

    // Out of range → nothing at all, and the caster is still not in the list.
    world
        .objects
        .get_component_mut::<Position>(&dead_mate)
        .unwrap()
        .x = 5000;
    assert!(
        targets_affected(&mut world, CASTER, CASTER, &skill).is_empty(),
        "a corpse beyond affect_range is out of reach"
    );
}

/// `DEAD_PARTY` is the same shape over the party, and the affect limit counts
/// corpses from zero (there is no "primary target" occupying the first slot).
#[test]
fn dead_party_scope_respects_the_affect_limit() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let a = 2002;
    let b = 2003;
    let _o2 = ingame_caster(&mut world, 2, a, 100, 0);
    let _o3 = ingame_caster(&mut world, 3, b, 150, 0);
    // One party of three — `put_in_party` mints a *new* party per call, so
    // calling it twice would split them.
    put_in_party(&mut world, CASTER, a);
    {
        use crate::model::components::PartyRef;
        let pid = world
            .objects
            .get_component::<PartyRef>(&CASTER)
            .map(|r| r.0)
            .unwrap();
        world.parties.get_mut(&pid).unwrap().members.push(b);
        world.objects.add_components(&b, PartyRef(pid));
    }
    for oid in [a, b] {
        world
            .objects
            .get_component_mut::<Vitals>(&oid)
            .unwrap()
            .dead = true;
    }

    let mut skill = aoe_skill(9020, AffectScope::DeadParty, AffectObject::All, 1000);
    skill.target_type = TargetType::Self_;
    skill.effect_point = 100;
    assert_eq!(
        targets_affected(&mut world, CASTER, CASTER, &skill).len(),
        2,
        "both fallen party mates"
    );

    skill.affect_limit = (1, 1);
    assert_eq!(
        targets_affected(&mut world, CASTER, CASTER, &skill).len(),
        1,
        "the limit counts corpses from zero, so 1 means 1"
    );
}
