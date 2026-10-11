//! Rush skills (`SkillOperateType.isFlyType()`): the cast gates that only they
//! carry and the slide `SkillCaster.handleSkillFly` performs at launch.

use super::*;
use crate::model::components::combat::Immobilized;
use crate::model::skill::effects::SkillEffect;
use crate::model::skill::target::SkillFly;

const RUSH: i32 = 99_001;

/// A `DA1`-shaped rush: instant damage, 600 cast range, no cast-time scaling
/// worth caring about (the tests run the scheduler well past the launch).
fn rush(target_type: TargetType) -> Skill {
    Skill {
        id: RUSH,
        name: "Rush".into(),
        target_type,
        fly: Some(SkillFly::ToTarget),
        cast_range: 600,
        effect_range: 1100,
        hit_time: 500,
        effect_point: -100,
        effects: vec![SkillEffect::MagicalAttack { power: 1.0 }],
        ..Default::default()
    }
}

fn install_fence_region(world: &mut World) {
    Arc::get_mut(&mut world.geo)
        .expect("geo Arc not shared yet")
        .set_region(
            20,
            18,
            crate::geo::synthetic_region(crate::geo::fence_column),
        );
}

fn xy(world: &World, oid: i32) -> (i32, i32) {
    let p = world.objects.get_component::<Position>(&oid).unwrap();
    (p.x, p.y)
}

fn opcode_index(pkts: &[Vec<u8>], opcode: u8) -> Option<usize> {
    pkts.iter().position(|p| p[0] == opcode)
}

/// The launch slides the caster onto the target and tells everyone with a
/// `FlyToLocation` that goes out *before* `MagicSkillLaunched`.
#[test]
fn rush_slides_the_caster_onto_its_target_at_launch() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 300, 0);
    learn_skill(&mut world, 3001, &rush(TargetType::EnemyOnly));
    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);
    drain(&mut b_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(RUSH, true));
    assert!(world.objects.has_component::<Casting>(&3001));
    assert_eq!(xy(&world, 3001), (0, 0), "nothing moves before the launch");

    world.tick += 100;
    apply_due_tasks(&mut world);
    assert_eq!(xy(&world, 3001), (300, 0));
    assert!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .blink_active,
        "the next ValidatePosition must not snap the caster back"
    );

    for rx in [&mut a_rx, &mut b_rx] {
        let pkts = drain(rx);
        let fly = opcode_index(&pkts, server_packets::opcodes::FLY_TO_LOCATION)
            .expect("FlyToLocation reaches the caster and the onlooker");
        let launched = opcode_index(&pkts, server_packets::opcodes::MAGIC_SKILL_LAUNCHED)
            .expect("MagicSkillLaunched");
        assert!(fly < launched, "the fly is sent before the launch flourish");
        // object id, destination, origin, then `FlyType.CHARGE`.
        let ints: Vec<i32> = pkts[fly][1..]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| i32::from_le_bytes(*c))
            .collect();
        assert_eq!(ints[0], 3001);
        assert_eq!((ints[1], ints[2]), (300, 0));
        assert_eq!((ints[4], ints[5]), (0, 0));
        assert_eq!(ints[7], server_packets::FlyType::Charge as i32);
    }
}

/// `handleSkillFly` lands on `GeoEngine.getValidLocation`, not on the raw
/// target point: an `ENEMY_ONLY` rush has no walkability gate of its own, so a
/// target seen over a fence is a legal cast that stops short of the fence.
#[test]
fn rush_stops_at_the_last_walkable_point() {
    let (mut world, ..) = cast_test_world();
    install_fence_region(&mut world);
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 8, 8);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 328, 8); // across the fence
    learn_skill(&mut world, 3001, &rush(TargetType::EnemyOnly));
    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(RUSH, true));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "seen over the fence, so the cast starts"
    );
    world.tick += 100;
    apply_due_tasks(&mut world);

    let (x, y) = xy(&world, 3001);
    assert!(
        x > 8 && x < 160,
        "moved toward the target but stayed west of the fence column, got x = {x}"
    );
    assert_eq!(y, 8);
}

/// `Target.java`'s rush gate: a `TARGET` rush refuses outright when the
/// straight line is not walkable — SM 2187, and no cast.
#[test]
fn target_rush_is_refused_where_it_cannot_charge() {
    let (mut world, ..) = cast_test_world();
    install_fence_region(&mut world);
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 8, 8);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 328, 8);
    learn_skill(&mut world, 3001, &rush(TargetType::Target));
    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(RUSH, true));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::THE_TARGET_IS_LOCATED_WHERE_YOU_CANNOT_CHARGE
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert_eq!(xy(&world, 3001), (8, 8));
}

/// `checkUseConditions`: `skill.isFlyType() && caster.isMovementDisabled()` —
/// a pinned caster gets a bare ActionFailed, while an ordinary skill is still
/// castable from the same spot.
#[test]
fn rooted_caster_cannot_rush() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 300, 0);
    learn_skill(&mut world, 3001, &rush(TargetType::EnemyOnly));
    handle_action(&mut world, 1, &action_body(3002, 0));
    world.objects.add_components(&3001, Immobilized);
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(RUSH, true));
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(a_rx.try_recv().is_err(), "no system message");
    assert!(!world.objects.has_component::<Casting>(&3001));

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "being rooted only blocks the fly type"
    );
}

/// A self-targeted rush (`creature == target`) goes `castRange` straight
/// ahead along the caster's heading.
#[test]
fn self_targeted_rush_dashes_cast_range_ahead() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 100, 50);
    let mut dash = rush(TargetType::Self_);
    dash.cast_range = 200;
    dash.effect_point = 0;
    dash.effects = Vec::new();
    learn_skill(&mut world, 3001, &dash);
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .heading = 0; // due east
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(RUSH, false));
    world.tick += 100;
    apply_due_tasks(&mut world);
    assert_eq!(xy(&world, 3001), (300, 50));
}
