//! The four zone kinds row 10 of the measured-gaps audit restored.
//!
//! `kind_from_type` returned `None` for each, and the loader's comment said
//! plainly what that meant — "`None` for kinds not ported yet, so mixed files
//! can be read without pulling in unported behaviour". The zones were dropped
//! at load, so every rule keyed on them was unenforced with nothing to show
//! for it: no log line, no marker, no failing test.

use super::*;

use crate::data::zone_data::{MotherTreeParams, Zone, ZoneKind};
use crate::model::Player;
use crate::model::components::Position;

const PLAYER: i32 = 9701;
const CID: u32 = 1;

/// A cuboid of `kind` covering the origin, ±1000 on each axis.
fn zone_at_origin(kind: ZoneKind, mother_tree: Option<MotherTreeParams>) -> Zone {
    Zone {
        id: 0,
        name: format!("test_{kind:?}"),
        kind,
        territory: crate::data::spawn_data::Territory {
            form: crate::data::spawn_data::ZoneForm::Cuboid {
                x1: -1000,
                x2: 1000,
                y1: -1000,
                y2: 1000,
            },
            min_z: -1000,
            max_z: 1000,
        },
        castle_id: 0,
        clan_hall_id: 0,
        effect: None,
        damage: None,
        swamp: None,
        condition: None,
        mother_tree,
    }
}

// ---------------------------------------------------------------------------
// MotherTreeZone — the regen bonus
// ---------------------------------------------------------------------------

/// The Elven nursery and the Devil's Isle pool add a **flat** bonus to the
/// regen base, which Java folds in "at last" — after every residence
/// multiplier and before the sitting term.
#[test]
fn a_mother_tree_adds_its_flat_regen_bonus() {
    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);

    let outside = crate::game_loop::regen::mother_tree_regen_bonus(&world, PLAYER);
    assert_eq!(outside, (0.0, 0.0), "no bonus off the zone");

    world.data.zone_data.insert(zone_at_origin(
        ZoneKind::MotherTree,
        Some(MotherTreeParams {
            hp_regen_bonus: 2,
            mp_regen_bonus: 1,
            enter_msg_id: 114,
            leave_msg_id: 115,
        }),
    ));

    assert_eq!(
        crate::game_loop::regen::mother_tree_regen_bonus(&world, PLAYER),
        (2.0, 1.0),
        "the shipped Elven/Devil's Isle pair is 2 HP / 1 MP"
    );
}

/// The real files, so a parse slip in the `<stat>` names is caught: six mother
/// trees ship (five Elven Village + the Devil's Isle pool), each 2 HP / 1 MP.
#[test]
fn the_real_mother_trees_carry_their_bonuses() {
    let data = crate::data::zone_data::ZoneData::load_from(crate::data::DIST_GAME);
    let trees: Vec<_> = data
        .zones
        .iter()
        .filter(|z| z.kind == ZoneKind::MotherTree)
        .collect();
    assert_eq!(trees.len(), 6, "5 Elven Village + 1 Devil's Isle");
    for z in &trees {
        let p = z.mother_tree.expect("params parsed");
        assert_eq!((p.hp_regen_bonus, p.mp_regen_bonus), (2, 1), "{}", z.name);
        assert!(p.enter_msg_id != 0, "{} has an enter message", z.name);
    }
}

// ---------------------------------------------------------------------------
// NoStoreZone
// ---------------------------------------------------------------------------

/// 18 zones forbid a shop. Before this the player simply opened one.
#[test]
fn a_no_store_zone_refuses_a_private_store() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    drain(&mut rx);
    assert!(
        crate::game_loop::private_store::can_open_private_store(&world, CID, PLAYER),
        "a shop opens fine on open ground"
    );

    world
        .data
        .zone_data
        .insert(zone_at_origin(ZoneKind::NoStore, None));

    assert!(
        !crate::game_loop::private_store::can_open_private_store(&world, CID, PLAYER),
        "and is refused inside a NoStoreZone"
    );
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE),
        "with Java's own message, not silence"
    );
}

// ---------------------------------------------------------------------------
// NoSummonFriendZone
// ---------------------------------------------------------------------------

/// `OpCallPcSkillCondition` — Summon Friend refuses to reach out of one of the
/// 27 no-summon zones. The port had the jail *state* leg only, with a comment
/// saying the zone kinds did not exist.
#[test]
fn a_no_summon_friend_zone_blocks_summon_friend() {
    use crate::model::skill::SkillCondition;
    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    let ok = |w: &World| {
        crate::game_loop::skills::conditions::check_for_test(
            w,
            PLAYER,
            PLAYER,
            &[SkillCondition::CallPc],
        )
    };
    assert!(ok(&world), "castable on open ground");

    world
        .data
        .zone_data
        .insert(zone_at_origin(ZoneKind::NoSummonFriend, None));

    assert!(!ok(&world), "refused inside the zone");
}

// ---------------------------------------------------------------------------
// LandingZone
// ---------------------------------------------------------------------------

/// `CanUntransformSkillCondition`'s altitude leg — the only reason
/// `LandingZone` exists. A wyvern rider may drop their transform over one of
/// the 69 landing zones and nowhere else; anyone on foot is unaffected.
#[test]
fn a_wyvern_rider_may_only_untransform_over_a_landing_zone() {
    use crate::model::skill::SkillCondition;
    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    let ok = |w: &World| {
        crate::game_loop::skills::conditions::check_for_test(
            w,
            PLAYER,
            PLAYER,
            &[SkillCondition::CanUntransform],
        )
    };
    assert!(ok(&world), "on foot, anywhere");

    const MOUNT_WYVERN: u8 = 2;
    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .mount_type = MOUNT_WYVERN;
    assert!(!ok(&world), "in the air, outside a landing zone");

    world
        .data
        .zone_data
        .insert(zone_at_origin(ZoneKind::Landing, None));
    assert!(ok(&world), "and permitted once over one");
}

/// The zone is a *place*, not a flag on the player: stepping out of its cuboid
/// is what changes the answer.
#[test]
fn the_landing_zone_is_geometric() {
    use crate::model::skill::SkillCondition;
    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    const MOUNT_WYVERN: u8 = 2;
    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .mount_type = MOUNT_WYVERN;
    world
        .data
        .zone_data
        .insert(zone_at_origin(ZoneKind::Landing, None));
    let ok = |w: &World| {
        crate::game_loop::skills::conditions::check_for_test(
            w,
            PLAYER,
            PLAYER,
            &[SkillCondition::CanUntransform],
        )
    };
    assert!(ok(&world), "inside");

    world
        .objects
        .get_component_mut::<Position>(&PLAYER)
        .unwrap()
        .x = 5000;
    assert!(!ok(&world), "outside");
}
