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
        effects: vec![SkillEffect::MagicalDamage {
            power: 1.0,
            debuff_modifier: 1.0,
            races: Vec::new(),
            race_modifier: 1.0,
        }],
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
        assert_eq!(ints[7], crate::enums::FlyType::Charge as i32);
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

/// `DA3`: `castRange` straight back along the caster's own heading, sent as
/// `FlyType.WARP_BACK` rather than a charge.
#[test]
fn back_step_warps_cast_range_behind_the_caster() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 100, 50);
    let mut step = rush(TargetType::Self_);
    step.fly = Some(SkillFly::Back);
    step.cast_range = 200;
    step.effect_point = 0;
    step.effects = Vec::new();
    learn_skill(&mut world, 3001, &step);
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .heading = 0; // facing due east, so "back" is west
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(RUSH, false));
    world.tick += 100;
    apply_due_tasks(&mut world);
    assert_eq!(xy(&world, 3001), (-100, 50));

    let pkts = drain(&mut a_rx);
    let fly = opcode_index(&pkts, server_packets::opcodes::FLY_TO_LOCATION).unwrap();
    assert_eq!(
        i32::from_le_bytes(pkts[fly][29..33].try_into().unwrap()),
        crate::enums::FlyType::WarpBack as i32
    );
}

/// `DA4`/`DA5`: beside the target, the two collision radii off its centre —
/// 270° and 90° round from the *target's* heading, on opposite flanks.
#[test]
fn sidesteps_land_on_the_targets_flanks() {
    for (fly, sign) in [(SkillFly::TargetLeft, 1), (SkillFly::TargetRight, -1)] {
        let (mut world, ..) = cast_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let _b_rx = ingame_caster(&mut world, 2, 3002, 300, 0);
        let mut step = rush(TargetType::EnemyOnly);
        step.fly = Some(fly);
        learn_skill(&mut world, 3001, &step);
        world
            .objects
            .get_component_mut::<Position>(&3002)
            .unwrap()
            .heading = 0;
        // The fixture's players are points; give them the radii the landing
        // distance is made of.
        for (oid, radius) in [(3001, 9.0), (3002, 12.0)] {
            world.objects.add_components(
                &oid,
                crate::model::components::space::Collision {
                    radius,
                    height: 23.0,
                },
            );
        }
        handle_action(&mut world, 1, &action_body(3002, 0));
        drain(&mut a_rx);

        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(RUSH, true));
        world.tick += 100;
        apply_due_tasks(&mut world);

        assert_eq!(xy(&world, 3001), (300, sign * 21), "{fly:?}");
    }
}

// ---------------------------------------------------------------------------
// `KnockBack` and `Blink` — effects that move the *effected* creature
// ---------------------------------------------------------------------------

use crate::enums::FlyType;
use crate::game_loop::skills::effects::apply_skill_effects;

const SHOVE: i32 = 99_010;

/// An instant push. `magicLevel 10` puts `calcProbability(100, …)` past any
/// roll against the fixture's level-5 targets.
fn shove(distance: i32) -> Skill {
    Skill {
        id: SHOVE,
        magic_level: 10,
        effect_point: -100,
        target_type: TargetType::EnemyOnly,
        effects: vec![SkillEffect::KnockBack {
            distance,
            knock_down: false,
            fly_type: FlyType::PushHorizontal,
        }],
        ..Default::default()
    }
}

/// A knock-down: the same push riding a continuous debuff.
fn knock_down(id: i32, abnormal_type: &str) -> Skill {
    Skill {
        id,
        effect_point: -100,
        target_type: TargetType::EnemyOnly,
        is_continuous: true,
        is_debuff: true,
        abnormal_time: 3,
        abnormal_level: 1,
        abnormal_type: abnormal_type.into(),
        effects: vec![SkillEffect::KnockBack {
            distance: 50,
            knock_down: true,
            fly_type: FlyType::PushDownHorizontal,
        }],
        ..Default::default()
    }
}

/// The instant push throws the victim `distance` directly away from the
/// effector and confirms the landing with a `ValidateLocation`.
#[test]
fn knock_back_pushes_the_victim_away_from_the_effector() {
    let (mut world, ..) = cast_test_world();
    let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    drain(&mut b_rx);

    apply_skill_effects(&mut world, 3001, 3002, &shove(50));
    assert_eq!(xy(&world, 3002), (150, 0));

    let pkts = drain(&mut b_rx);
    let fly = opcode_index(&pkts, server_packets::opcodes::FLY_TO_LOCATION).unwrap();
    assert_eq!(
        i32::from_le_bytes(pkts[fly][29..33].try_into().unwrap()),
        FlyType::PushHorizontal as i32
    );
    let validate = opcode_index(&pkts, server_packets::opcodes::VALIDATE_LOCATION).unwrap();
    assert!(fly < validate);
}

/// `effected.isNpc() && !effected.isAttackable()` — a town NPC stays put,
/// while a monster beside it is thrown.
#[test]
fn knock_back_does_not_move_town_npcs() {
    let (mut world, ..) = cast_test_world();
    let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    add_test_npc(&mut world, NPC_OID + 1, 30001, "Merchant", 5, 0, 100, 0);

    apply_skill_effects(&mut world, 3001, NPC_OID, &shove(50));
    apply_skill_effects(&mut world, 3001, NPC_OID + 1, &shove(50));
    assert_eq!(xy(&world, NPC_OID), (150, 0));
    assert_eq!(xy(&world, NPC_OID + 1), (0, 100));
}

/// The knock-down form moves the victim when its debuff lands, holds them in
/// `BLOCK_ACTIONS`, and turns them to face the effector. A second knock-down
/// landing while they are still down does not throw them again.
#[test]
fn knock_down_floors_the_victim_once() {
    let (mut world, ..) = cast_test_world();
    let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);

    // The "already down" check reads the live buff's skill row, so both
    // skills have to be in the table, as a real one always is.
    for (id, abnormal) in [(99_011, "KNOCKDOWN"), (99_012, "KNOCKDOWN2")] {
        world
            .data
            .skill_data
            .insert_for_test(knock_down(id, abnormal));
    }

    apply_skill_effects(&mut world, 3001, 3002, &knock_down(99_011, "KNOCKDOWN"));
    assert!(has_buff(&world, 3002, 99_011));
    assert_eq!(xy(&world, 3002), (150, 0));
    assert!(crate::game_loop::abnormal::is_blocked_from_actions(
        &world, 3002
    ));
    assert_eq!(
        world
            .objects
            .get_component::<Position>(&3002)
            .unwrap()
            .heading,
        32767, // 180° × 182.044444444, truncated
        "facing west, back toward the effector"
    );

    apply_skill_effects(&mut world, 3001, 3002, &knock_down(99_012, "KNOCKDOWN2"));
    assert!(has_buff(&world, 3002, 99_012));
    assert_eq!(
        xy(&world, 3002),
        (150, 0),
        "already down, so not thrown again"
    );
}

fn blink_skill(angle: i32) -> Skill {
    Skill {
        id: 99_020,
        target_type: TargetType::Self_,
        effects: vec![SkillEffect::Blink {
            angle,
            range: 200,
            fly_type: FlyType::Dummy,
        }],
        ..Default::default()
    }
}

/// `Blink`: `range` along `angle` from the creature's own heading — 180 is
/// straight ahead and 0 straight back.
#[test]
fn blink_warps_along_its_angle_from_the_heading() {
    for (angle, x) in [(180, 200), (0, -200)] {
        let (mut world, ..) = cast_test_world();
        let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        world
            .objects
            .get_component_mut::<Position>(&3001)
            .unwrap()
            .heading = 0;

        apply_skill_effects(&mut world, 3001, 3001, &blink_skill(angle));
        assert_eq!(xy(&world, 3001), (x, 0), "angle {angle}");
    }
}

/// `Blink.canStart`: `!effected.cannotEscape()`.
#[test]
fn blink_is_refused_under_an_escape_block() {
    let (mut world, ..) = cast_test_world();
    let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let anchor = Skill {
        id: 99_021,
        is_continuous: true,
        abnormal_time: 30,
        abnormal_type: "ANCHOR".into(),
        effects: vec![SkillEffect::BlockEscape],
        ..Default::default()
    };
    apply_skill_effects(&mut world, 3001, 3001, &anchor);
    assert!(crate::game_loop::abnormal::cannot_escape(&world, 3001));

    apply_skill_effects(&mut world, 3001, 3001, &blink_skill(180));
    assert_eq!(xy(&world, 3001), (0, 0));
}
