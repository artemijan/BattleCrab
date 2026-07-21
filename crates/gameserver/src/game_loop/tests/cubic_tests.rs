//! Cubics — G29 slice 9.
//!
//! Cubics were chosen over agathions by the learnable-skill ranking:
//! `SummonCubic` has 12 learnable skills on this dist, `SummonAgathion` has
//! **zero** (all 166 are off every skill tree). See `docs/PLAN_G29_CUBICS.md`.

use super::*;

use crate::data::cubic_data::{CubicSkill, CubicTargetType, CubicTemplate};
use crate::game_loop::cubic::{handle_cubic_action, summon_cubic, Cubics};

const OWNER: i32 = 9911;
const CID: u32 = 1;
const FOE: i32 = NPC_OID + 20;

const CUBIC_ID: i32 = 1;
/// The cubic's attack skill, registered as a plain magical hit.
const CUBIC_SKILL: i32 = 4049;

fn cubic_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    combat_test_world()
}

/// A one-skill attack cubic: 900 s life, acts every 10 s, always succeeds.
fn attack_template(level: i32) -> CubicTemplate {
    CubicTemplate {
        id: CUBIC_ID,
        level,
        slot: 1,
        duration: 900,
        delay: 10,
        max_count: 30,
        power: 282.0,
        target_type: CubicTargetType::Target,
        skills: vec![CubicSkill {
            skill_id: CUBIC_SKILL,
            skill_level: 1,
            success_rate: 100,
            trigger_rate: 100,
            can_use_on_static_objects: true,
            target_type: None,
        }],
        hp_condition: None,
        range: Some(1000),
        health_percent: None,
    }
}

fn register(world: &mut World, t: CubicTemplate) {
    world.data.cubic_data.insert_for_test(t);
    let skill = crate::model::skill::Skill {
        id: CUBIC_SKILL,
        level: 1,
        effects: vec![crate::model::skill::SkillEffect::MagicalAttack { power: 50.0 }],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(skill);
}

fn cubics(world: &World) -> Vec<i32> {
    world.objects.get_component::<Cubics>(&OWNER).map(|c| c.ids()).unwrap_or_default()
}

fn hp(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_hp
}

/// The baseline: summoning attaches a cubic to the player.
#[test]
fn summoning_attaches_a_cubic() {
    let (mut world, _db, _l) = cubic_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register(&mut world, attack_template(1));

    summon_cubic(&mut world, OWNER, CUBIC_ID, 1);
    assert_eq!(cubics(&world), vec![CUBIC_ID]);
}

/// A cubic is not a world object — it must not appear in the NPC store, or it
/// would be targetable, attackable and visible as a creature.
#[test]
fn a_cubic_is_not_a_world_object() {
    let (mut world, _db, _l) = cubic_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register(&mut world, attack_template(1));
    let count = |w: &mut World| {
        let mut n = 0;
        w.objects.for_each_mut::<&crate::model::npc::Npc>(|_| n += 1);
        n
    };
    let before = count(&mut world);

    summon_cubic(&mut world, OWNER, CUBIC_ID, 1);
    assert_eq!(count(&mut world), before, "no NPC entity was spawned");
}

/// Re-casting the same cubic replaces it rather than stacking a second copy.
#[test]
fn recasting_replaces_rather_than_stacks() {
    let (mut world, _db, _l) = cubic_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register(&mut world, attack_template(1));
    register(&mut world, attack_template(2));

    summon_cubic(&mut world, OWNER, CUBIC_ID, 1);
    summon_cubic(&mut world, OWNER, CUBIC_ID, 2);
    assert_eq!(cubics(&world), vec![CUBIC_ID], "still exactly one");
    assert_eq!(
        world.objects.get_component::<Cubics>(&OWNER).unwrap().0[0].level,
        2,
        "upgraded to the newly cast level"
    );
}

/// Java returns outright when the existing cubic outranks the new one — a
/// weaker cast must not downgrade it.
#[test]
fn a_weaker_recast_does_not_downgrade() {
    let (mut world, _db, _l) = cubic_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register(&mut world, attack_template(1));
    register(&mut world, attack_template(2));

    summon_cubic(&mut world, OWNER, CUBIC_ID, 2);
    summon_cubic(&mut world, OWNER, CUBIC_ID, 1);
    assert_eq!(
        world.objects.get_component::<Cubics>(&OWNER).unwrap().0[0].level,
        2,
        "the higher-level cubic survives the weaker cast"
    );
}

/// The action tick casts the cubic's skill at the owner's current target.
#[test]
fn the_cubic_attacks_the_owners_target() {
    let (mut world, _db, _l) = cubic_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register(&mut world, attack_template(1));
    add_test_npc(&mut world, FOE, 20001, "Monster", 20, 100, 0, 0);
    world.objects.get_component_mut::<crate::model::components::TargetRef>(&OWNER).unwrap().0 = Some(FOE);

    summon_cubic(&mut world, OWNER, CUBIC_ID, 1);
    let before = hp(&world, FOE);
    handle_cubic_action(&mut world, OWNER, CUBIC_ID);
    assert!(hp(&world, FOE) < before, "the cubic's skill damaged the target");
}

/// With no target there is nothing to hit, and the cubic must not spend a
/// charge on a cast that never happened.
#[test]
fn a_cubic_with_no_target_does_nothing() {
    let (mut world, _db, _l) = cubic_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register(&mut world, attack_template(1));

    summon_cubic(&mut world, OWNER, CUBIC_ID, 1);
    handle_cubic_action(&mut world, OWNER, CUBIC_ID);
    assert_eq!(
        world.objects.get_component::<Cubics>(&OWNER).unwrap().0[0].remaining_count,
        30,
        "no charge spent when no skill fired"
    );
}

/// `maxCount` counts *actions*: the cubic goes away once it has acted that
/// many times.
#[test]
fn a_cubic_expires_after_max_count_actions() {
    let (mut world, _db, _l) = cubic_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let mut t = attack_template(1);
    t.max_count = 2;
    register(&mut world, t);
    add_test_npc(&mut world, FOE, 20001, "Monster", 20, 100, 0, 0);
    world.objects.get_component_mut::<crate::model::components::TargetRef>(&OWNER).unwrap().0 = Some(FOE);

    // The dummy must survive both casts: a dead target yields no cast, so it
    // would spend no charge and the cubic would never reach zero. (That is
    // correct behaviour — it is what made the first draft of this test fail.)
    {
        let v = world.objects.get_component_mut::<Vitals>(&FOE).unwrap();
        v.max_hp = 100_000;
        v.cur_hp = 100_000.0;
    }

    summon_cubic(&mut world, OWNER, CUBIC_ID, 1);
    handle_cubic_action(&mut world, OWNER, CUBIC_ID);
    assert_eq!(cubics(&world), vec![CUBIC_ID], "one charge left");
    handle_cubic_action(&mut world, OWNER, CUBIC_ID);
    assert!(cubics(&world).is_empty(), "spent its last charge and went away");
}

/// The cubic stops when its duration runs out.
#[test]
fn a_cubic_expires_after_its_duration() {
    let (mut world, _db, _l) = cubic_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register(&mut world, attack_template(1));

    summon_cubic(&mut world, OWNER, CUBIC_ID, 1);
    world.tick += 900 * 10 + 1;
    handle_cubic_action(&mut world, OWNER, CUBIC_ID);
    assert!(cubics(&world).is_empty(), "duration elapsed");
}

/// `<hp type="GREATER" percent="33"/>` gates the *owner*: a badly wounded
/// player's attack cubic stops firing.
#[test]
fn the_owner_hp_condition_gates_the_cast() {
    let (mut world, _db, _l) = cubic_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let mut t = attack_template(1);
    t.hp_condition = Some(crate::data::cubic_data::HpCondition { percent: 33, greater: true });
    register(&mut world, t);
    add_test_npc(&mut world, FOE, 20001, "Monster", 20, 100, 0, 0);
    world.objects.get_component_mut::<crate::model::components::TargetRef>(&OWNER).unwrap().0 = Some(FOE);
    summon_cubic(&mut world, OWNER, CUBIC_ID, 1);

    // Drop the owner to 10% — below the 33% floor.
    {
        let v = world.objects.get_component_mut::<Vitals>(&OWNER).unwrap();
        v.cur_hp = v.max_hp as f64 * 0.10;
    }
    let before = hp(&world, FOE);
    handle_cubic_action(&mut world, OWNER, CUBIC_ID);
    assert_eq!(hp(&world, FOE), before, "wounded owner's cubic held fire");

    // Back above the floor and it acts again.
    {
        let v = world.objects.get_component_mut::<Vitals>(&OWNER).unwrap();
        v.cur_hp = v.max_hp as f64;
    }
    handle_cubic_action(&mut world, OWNER, CUBIC_ID);
    assert!(hp(&world, FOE) < before, "healthy owner's cubic fires");
}

/// A target beyond `<range>` is not cast at.
#[test]
fn a_target_out_of_range_is_skipped() {
    let (mut world, _db, _l) = cubic_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register(&mut world, attack_template(1));
    add_test_npc(&mut world, FOE, 20001, "Monster", 20, 100, 0, 0);
    world.objects.get_component_mut::<crate::model::components::TargetRef>(&OWNER).unwrap().0 = Some(FOE);
    // Shove the target well past the 1000-unit range.
    world.objects.get_component_mut::<Position>(&FOE).unwrap().x += 5000;

    summon_cubic(&mut world, OWNER, CUBIC_ID, 1);
    let before = hp(&world, FOE);
    handle_cubic_action(&mut world, OWNER, CUBIC_ID);
    assert_eq!(hp(&world, FOE), before, "out of range, no cast");
}

/// Cubics do not survive their owner leaving the world.
#[test]
fn cubics_do_not_outlive_their_owner() {
    let (mut world, _db, _l) = cubic_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register(&mut world, attack_template(1));
    summon_cubic(&mut world, OWNER, CUBIC_ID, 1);

    crate::game_loop::cubic::on_owner_leave_world(&mut world, OWNER);
    assert!(cubics(&world).is_empty());
}

/// A cubic must reach other players' clients: `CharInfo` carries the id list,
/// and it was hard-coded to zero before this slice — the same shape as the
/// abnormal-visual bug G19 found.
#[test]
fn char_info_carries_the_cubic_ids() {
    let (mut world, _db, _l) = cubic_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register(&mut world, attack_template(1));
    summon_cubic(&mut world, OWNER, CUBIC_ID, 1);

    let v = crate::model::PlayerView::of(&world.objects, OWNER).unwrap();
    let with = crate::network::server_packets::char_info(&v, &[], &[CUBIC_ID]);
    let without = crate::network::server_packets::char_info(&v, &[], &[]);
    assert_eq!(
        with.len(),
        without.len() + 2,
        "one cubic adds exactly one short to the packet"
    );
}

/// The converse of the max-count fix above, pinned so it can't regress: a
/// cubic whose target is already dead does not spend a charge.
#[test]
fn a_dead_target_costs_no_charge() {
    let (mut world, _db, _l) = cubic_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register(&mut world, attack_template(1));
    add_test_npc(&mut world, FOE, 20001, "Monster", 20, 100, 0, 0);
    world.objects.get_component_mut::<crate::model::components::TargetRef>(&OWNER).unwrap().0 = Some(FOE);
    summon_cubic(&mut world, OWNER, CUBIC_ID, 1);

    world.objects.get_component_mut::<Vitals>(&FOE).unwrap().dead = true;
    handle_cubic_action(&mut world, OWNER, CUBIC_ID);
    assert_eq!(
        world.objects.get_component::<Cubics>(&OWNER).unwrap().0[0].remaining_count,
        30,
        "no charge spent on a corpse"
    );
}
