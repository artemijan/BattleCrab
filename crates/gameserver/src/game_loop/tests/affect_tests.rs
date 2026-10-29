//! Affect scopes (G19): the sweep that turns one primary target into the set a
//! skill actually lands on, the friend/foe filter, and toggle on/off.

use super::*;
use crate::game_loop::abnormal::has_buff;

use crate::game_loop::skills::affect::targets_affected;
use crate::model::skill::Skill;
use crate::model::skill::effects::{SkillEffect, StatModifierEffect};
use crate::model::skill::target::{AffectObject, AffectScope, OperateType, TargetType};
use crate::model::stats::{Stat, StatModifierType};

const CASTER: i32 = 2001;
const CID: u32 = 1;

/// A skill template the tests reshape per case.
fn aoe_skill(id: i32, scope: AffectScope, object: AffectObject, range: i32) -> Skill {
    Skill {
        without_action: false,
        trait_type: model::skill::traits::TraitType::None,
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
        soul_max_consume_count: 0,
        is_triggered_skill: false,
        charge_consume_count: 0,
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
            hp_percent: 0,
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
    use crate::model::components::social::PartyRef;
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
    nuke.effects = vec![SkillEffect::MagicalDamage {
        power: 50.0,
        debuff_modifier: 1.0,
        races: Vec::new(),
        race_modifier: 1.0,
    }];
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
        use crate::model::components::social::PartyRef;
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

// ---------------------------------------------------------------------------
// The `TG` / `AU` toggle variants
// ---------------------------------------------------------------------------

fn magic_skill_uses(out: &mut UnboundedReceiver<bytes::Bytes>) -> usize {
    drain_count(out, server_packets::opcodes::MAGIC_SKILL_USE)
}

/// `SkillOperateType.isNotBroadcastable()`: a `TG`/`AU` toggle switches on
/// without a `MagicSkillUse`, where a plain `T` toggle sends one.
#[test]
fn a_not_broadcastable_toggle_switches_on_silently() {
    let (mut world, _db, _l) = cast_test_world();
    let mut aura = toggle_skill(9110, 0);
    aura.not_broadcastable = true;
    world.data.skill_data.insert_for_test(aura);
    world.data.skill_data.insert_for_test(toggle_skill(9111, 0));
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    {
        let book = world
            .objects
            .get_component_mut::<SkillBook>(&CASTER)
            .unwrap();
        book.0.insert(9110, 1);
        book.0.insert(9111, 1);
    }
    drain(&mut out);

    use_magic(&mut world, CID, CASTER, 9110, false, false);
    assert!(has_buff(&world, CASTER, 9110));
    assert_eq!(magic_skill_uses(&mut out), 0);

    use_magic(&mut world, CID, CASTER, 9111, false, false);
    assert!(has_buff(&world, CASTER, 9111));
    assert_eq!(
        magic_skill_uses(&mut out),
        1,
        "a plain toggle still animates"
    );

    // It is still a toggle: the recast switches it off.
    use_magic(&mut world, CID, CASTER, 9110, false, false);
    assert!(!has_buff(&world, CASTER, 9110));
}

/// `Player.useMagic`: recasting a necessary toggle casts nothing and leaves it
/// on — only a sibling in its toggle group takes it off.
#[test]
fn a_necessary_toggle_is_not_switched_off_by_a_recast() {
    let (mut world, _db, _l) = cast_test_world();
    for id in [9120, 9121] {
        let mut stance = toggle_skill(id, 3);
        stance.necessary_toggle = true;
        world.data.skill_data.insert_for_test(stance);
    }
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    {
        let book = world
            .objects
            .get_component_mut::<SkillBook>(&CASTER)
            .unwrap();
        book.0.insert(9120, 1);
        book.0.insert(9121, 1);
    }
    drain(&mut out);

    use_magic(&mut world, CID, CASTER, 9120, false, false);
    use_magic(&mut world, CID, CASTER, 9120, false, false);
    assert!(has_buff(&world, CASTER, 9120), "the recast leaves it on");

    use_magic(&mut world, CID, CASTER, 9121, false, false);
    assert!(has_buff(&world, CASTER, 9121));
    assert!(
        !has_buff(&world, CASTER, 9120),
        "a group sibling still replaces it"
    );
}

// ---------------------------------------------------------------------------
// `CallSkillOnActionTime` — the aura pulse
// ---------------------------------------------------------------------------

const AURA: i32 = 9130;
const PULSE: i32 = 9131;
/// `ticks 5 × EffectTickRatio 666 ms` in 100 ms game ticks.
const PULSE_INTERVAL: u64 = 33;

/// An `AU` toggle that pulses [`PULSE`] and does nothing else.
fn aura_skill() -> Skill {
    let mut s = toggle_skill(AURA, 0);
    s.not_broadcastable = true;
    s.effects = vec![SkillEffect::CallSkillOnActionTime {
        skill_id: PULSE,
        skill_level: 1,
        ticks: 5,
    }];
    s
}

/// The pulsed skill, shaped like the knight auras' party half (`A5`,
/// `MY_PARTY` / `PARTY` / `FRIEND`, a short-lived `AURA` buff).
fn party_pulse() -> Skill {
    let mut s = aoe_skill(PULSE, AffectScope::Party, AffectObject::Friend, 1000);
    s.target_type = TargetType::MyParty;
    s.is_continuous = true;
    s.is_debuff = false;
    s.effect_point = 0;
    s.abnormal_type = "AURA".into();
    s.abnormal_time = 6;
    s
}

fn switch_aura_on(world: &mut World, pulse: Skill) -> UnboundedReceiver<bytes::Bytes> {
    world.data.skill_data.insert_for_test(aura_skill());
    world.data.skill_data.insert_for_test(pulse);
    let mut out = ingame_caster(world, CID, CASTER, 0, 0);
    world
        .objects
        .get_component_mut::<SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(AURA, 1);
    drain(&mut out);
    out
}

fn buff_expiry(world: &World, oid: i32, skill_id: i32) -> Option<u64> {
    world
        .objects
        .get_component::<crate::model::components::skills::Buffs>(&oid)?
        .0
        .iter()
        .find(|b| b.skill_id == skill_id)
        .map(|b| b.expires_at_tick)
}

/// A party aura reaches the holder's party and nobody else, keeps the buff
/// refreshed while it is on, and lets it lapse once switched off. The holder
/// never receives their own pulse — the aura blocks that abnormal type.
#[test]
fn a_party_aura_pulses_its_buff_onto_the_holders_party() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = switch_aura_on(&mut world, party_pulse());
    let (mate, stranger) = (2002, 2003);
    let _mate_rx = ingame_caster(&mut world, 2, mate, 100, 0);
    let _stranger_rx = ingame_caster(&mut world, 3, stranger, 150, 0);
    put_in_party(&mut world, CASTER, mate);

    use_magic(&mut world, CID, CASTER, AURA, false, false);
    assert!(has_buff(&world, CASTER, AURA));
    assert!(
        !has_buff(&world, mate, PULSE),
        "nothing lands before a tick"
    );

    world.tick += PULSE_INTERVAL;
    apply_due_tasks(&mut world);
    assert!(has_buff(&world, mate, PULSE), "the party mate is buffed");
    assert!(!has_buff(&world, stranger, PULSE), "an outsider is not");
    assert!(
        !has_buff(&world, CASTER, PULSE),
        "the holder is immune to their own pulse"
    );

    // The next pulse replaces the buff, pushing its expiry out.
    let first = buff_expiry(&world, mate, PULSE).unwrap();
    world.tick += PULSE_INTERVAL;
    apply_due_tasks(&mut world);
    assert!(buff_expiry(&world, mate, PULSE).unwrap() > first);

    // Off: no more pulses, and the last one runs out on its own (6 s).
    use_magic(&mut world, CID, CASTER, AURA, false, false);
    assert!(!has_buff(&world, CASTER, AURA));
    world.tick += 100;
    apply_due_tasks(&mut world);
    assert!(!has_buff(&world, mate, PULSE));
}

/// `onStart` strips the pulsed abnormal type from the holder before blocking
/// it — switching an aura on drops another knight's aura buff.
#[test]
fn switching_an_aura_on_strips_the_pulsed_abnormal_from_the_holder() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = switch_aura_on(&mut world, party_pulse());
    let pulse = party_pulse();
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, CASTER, CASTER, &pulse);
    assert!(has_buff(&world, CASTER, PULSE));

    use_magic(&mut world, CID, CASTER, AURA, false, false);
    assert!(!has_buff(&world, CASTER, PULSE));
}

/// A hostile aura (`ENEMY` / `RANGE` / `NOT_FRIEND`) debuffs the monsters
/// around the holder and leaves one far outside the range alone.
#[test]
fn an_enemy_aura_debuffs_the_monsters_in_range() {
    let (mut world, _db, _l) = cast_test_world();
    let mut pulse = aoe_skill(PULSE, AffectScope::Range, AffectObject::NotFriend, 300);
    pulse.is_continuous = true;
    pulse.abnormal_type = "AURA".into();
    pulse.abnormal_time = 11;
    let _out = switch_aura_on(&mut world, pulse);
    let (a, b, far) = spawn_cluster(&mut world);

    use_magic(&mut world, CID, CASTER, AURA, false, false);
    world.tick += PULSE_INTERVAL;
    apply_due_tasks(&mut world);
    assert!(has_buff(&world, a, PULSE) && has_buff(&world, b, PULSE));
    assert!(!has_buff(&world, far, PULSE));
}

/// A synergy (`A6`) pulse is exempt from the aura's block (`onStart` skips
/// it) and lands on the holder each pulse (`applyEffects(effector,
/// effector)`), with nobody else around.
#[test]
fn a_synergy_pulse_lands_on_the_holder() {
    let (mut world, _db, _l) = cast_test_world();
    let mut pulse = party_pulse();
    pulse.synergy = true;
    let _out = switch_aura_on(&mut world, pulse);

    use_magic(&mut world, CID, CASTER, AURA, false, false);
    world.tick += PULSE_INTERVAL;
    apply_due_tasks(&mut world);
    assert!(has_buff(&world, CASTER, PULSE));
}

// ---------------------------------------------------------------------------
// The four narrow affect objects
// ---------------------------------------------------------------------------

fn mark_dead(world: &mut World, oid: i32) {
    world
        .objects
        .get_component_mut::<Vitals>(&oid)
        .unwrap()
        .dead = true;
}

/// Sweeper Festival (444): `SELF` + `POINT_BLANK` + `OBJECT_DEAD_NPC_BODY`.
/// `PointBlank.java` drops the dead for every affect object but this one, so
/// the sweep comes back with the corpses and nothing else.
#[test]
fn dead_npc_body_point_blank_sweeps_only_corpses() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (corpse, _alive, far) = spawn_cluster(&mut world);
    let dead_player = 2002;
    let _o2 = ingame_caster(&mut world, 2, dead_player, 50, 0);
    for oid in [corpse, far, dead_player] {
        mark_dead(&mut world, oid);
    }

    let mut skill = aoe_skill(444, AffectScope::PointBlank, AffectObject::DeadNpcBody, 200);
    skill.target_type = TargetType::Self_;

    assert_eq!(
        targets_affected(&mut world, CASTER, CASTER, &skill),
        vec![corpse],
        "the live mob, the dead player and the corpse out of range all stay out"
    );

    // `Range.java` has no such exemption: only the corpse target types keep
    // the dead there.
    skill.affect_scope = AffectScope::Range;
    assert_eq!(
        targets_affected(&mut world, CASTER, CASTER, &skill),
        vec![CASTER],
        "RANGE still drops the dead, and the live mob fails the filter"
    );
}

/// `INVISIBLE` is `target.isInvisible()` — the `//hide` flag here.
#[test]
fn invisible_passes_only_the_hidden() {
    use crate::game_loop::skills::affect::passes_affect_object;
    use crate::model::components::player::AdminFlags;

    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (hidden, seen) = (2002, 2003);
    let _o2 = ingame_caster(&mut world, 2, hidden, 50, 0);
    let _o3 = ingame_caster(&mut world, 3, seen, 80, 0);
    world.objects.add_components(
        &hidden,
        AdminFlags {
            hidden: true,
            ..Default::default()
        },
    );

    let passes =
        |world: &World, oid: i32| passes_affect_object(world, CASTER, oid, AffectObject::Invisible);
    assert!(passes(&world, hidden));
    assert!(!passes(&world, seen));

    let mut skill = aoe_skill(933, AffectScope::Range, AffectObject::Invisible, 300);
    skill.target_type = TargetType::Self_;
    assert_eq!(
        targets_affected(&mut world, CASTER, CASTER, &skill),
        vec![CASTER, hidden],
        "the sweep reaches a hidden player and leaves the visible one"
    );
}

/// `HIDDEN_PLACE` is a bare `return false` in Java; `WYVERN_OBJECT` is
/// membership of the `WYVERN_GROUP` category by npc id.
#[test]
fn hidden_place_passes_nothing_and_wyvern_object_reads_the_category() {
    use crate::game_loop::skills::affect::passes_affect_object;

    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (wyvern, mob, _far) = spawn_cluster(&mut world);
    add_test_npc(&mut world, wyvern + 10, 12621, "Monster", 5, 50, 0, 0);
    let wyvern = wyvern + 10;
    world
        .data
        .categories
        .insert_for_test("WYVERN_GROUP", &[12621]);

    for oid in [CASTER, wyvern, mob] {
        assert!(!passes_affect_object(
            &world,
            CASTER,
            oid,
            AffectObject::HiddenPlace
        ));
    }
    let passes = |oid: i32| passes_affect_object(&world, CASTER, oid, AffectObject::WyvernObject);
    assert!(passes(wyvern));
    assert!(!passes(mob));
    assert!(!passes(CASTER), "a player has no npc id to look up");
}

// ---------------------------------------------------------------------------
// The seven narrow scopes
// ---------------------------------------------------------------------------

/// Caster partied with `mate`, clanned with `clannie`, and a `stranger` who is
/// neither — the fixture both PARTY_PLEDGE scopes read.
fn party_and_clan(world: &mut World) -> (i32, i32, i32) {
    let (mate, clannie, stranger) = (2002, 2003, 2004);
    for (cid, oid, x) in [(2, mate, 50), (3, clannie, 100), (4, stranger, 150)] {
        // The receiver is dropped: nothing here reads the packets.
        let _ = ingame_caster(world, cid, oid, x, 0);
    }
    put_in_party(world, CASTER, mate);
    for oid in [CASTER, clannie] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .unwrap()
            .clan_id = 88;
    }
    (mate, clannie, stranger)
}

/// `PARTY_PLEDGE` rejects a candidate only when it shares neither the clan
/// nor the party.
#[test]
fn party_pledge_scope_covers_the_party_and_the_clan() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (mate, clannie, stranger) = party_and_clan(&mut world);

    let mut skill = aoe_skill(1534, AffectScope::PartyPledge, AffectObject::All, 1000);
    skill.target_type = TargetType::Self_;

    let mut hit = targets_affected(&mut world, CASTER, CASTER, &skill);
    assert_eq!(hit.first(), Some(&CASTER));
    hit.sort_unstable();
    assert_eq!(hit, vec![CASTER, mate, clannie]);
    assert!(!hit.contains(&stranger));
}

/// `DEAD_PARTY_PLEDGE` is the same membership over corpses.
#[test]
fn dead_party_pledge_scope_gathers_party_and_clan_corpses() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (mate, clannie, stranger) = party_and_clan(&mut world);
    for oid in [mate, clannie, stranger] {
        mark_dead(&mut world, oid);
    }

    let mut skill = aoe_skill(11785, AffectScope::DeadPartyPledge, AffectObject::All, 1000);
    skill.target_type = TargetType::Self_;

    let mut hit = targets_affected(&mut world, CASTER, CASTER, &skill);
    hit.sort_unstable();
    assert_eq!(
        hit,
        vec![mate, clannie],
        "the living caster and the stranger's corpse stay out"
    );
}

/// `RANGE_SORT_BY_HP` hands the sweep back most-wounded first, the target
/// sorted in with the rest rather than leading.
#[test]
fn range_sort_by_hp_orders_from_the_lowest_hp() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (a, b, far) = spawn_cluster(&mut world);
    let c = NPC_OID + 3;
    add_test_npc(&mut world, c, 20001, "Monster", 5, 0, 100, 0);
    for (oid, percent) in [(a, 0.6), (b, 0.2), (c, 0.9)] {
        let v = world.objects.get_component_mut::<Vitals>(&oid).unwrap();
        v.cur_hp = v.max_hp as f64 * percent;
    }

    let skill = aoe_skill(1553, AffectScope::RangeSortByHp, AffectObject::All, 200);

    let hit = targets_affected(&mut world, CASTER, a, &skill);
    assert_eq!(
        hit,
        vec![b, a, c],
        "lowest HP first; the caster standing in range is not the target, so stays out"
    );
    assert!(!hit.contains(&far));
}

/// `STATIC_OBJECT_SCOPE` sweeps doors and nothing else — a `SELF` cast drops
/// its own caster — and `HIDDEN_PLACE` then filters every one of them out,
/// which is all Nornil's Power (2353) amounts to in Java.
#[test]
fn static_object_scope_sweeps_doors_only() {
    use crate::data::door_data::DoorOpenMethod;

    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (_a, _b, _far) = spawn_cluster(&mut world);
    let near =
        crate::model::door::spawn_door_for_test(&mut world, test_door(9001, DoorOpenMethod::None));
    let mut away = test_door(9002, DoorOpenMethod::None);
    away.x = 900;
    let _away = crate::model::door::spawn_door_for_test(&mut world, away);
    let mut breached = test_door(9003, DoorOpenMethod::None);
    breached.y = 50;
    let breached = crate::model::door::spawn_door_for_test(&mut world, breached);
    world
        .objects
        .get_component_mut::<crate::model::door::Door>(&breached)
        .unwrap()
        .current_hp = 0;

    let mut skill = aoe_skill(2353, AffectScope::StaticObject, AffectObject::All, 600);
    skill.target_type = TargetType::Self_;
    assert_eq!(
        targets_affected(&mut world, CASTER, CASTER, &skill),
        vec![near],
        "no caster, no monsters, no breached door, no door out of range"
    );

    skill.affect_object = AffectObject::HiddenPlace;
    assert!(targets_affected(&mut world, CASTER, CASTER, &skill).is_empty());
}

/// `BALAKAS_SCOPE` and `WYVERN_SCOPE` are empty bodies in Java, so the cast
/// affects nobody — not even its own `SELF` target. Valakas casts Meteor Storm
/// (4690) from his skill AI; reading the scope as single-target landed it on
/// Valakas himself.
#[test]
fn balakas_and_wyvern_scopes_affect_nobody() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 50, 0);
    let (valakas, _b, _far) = spawn_cluster(&mut world);

    let meteor_storm = dist::skills()
        .get(4690, 1)
        .cloned()
        .expect("Valakas Meteor Storm");
    assert_eq!(meteor_storm.affect_scope, AffectScope::Balakas);
    assert!(targets_affected(&mut world, valakas, valakas, &meteor_storm).is_empty());

    let wyvern_breath = aoe_skill(4289, AffectScope::Wyvern, AffectObject::WyvernObject, 200);
    assert!(targets_affected(&mut world, CASTER, valakas, &wyvern_breath).is_empty());
}
