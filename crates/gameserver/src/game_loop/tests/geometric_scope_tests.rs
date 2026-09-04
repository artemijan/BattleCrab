//! Geometric affect scopes (G19, PLAN_G19_GEOMETRIC_SCOPES.md): FAN's
//! heading-relative arc, SQUARE's rotated rectangle, RING_RANGE's annulus,
//! and the Range.java minion-buff fix folded into the same slice.

use super::*;

use crate::game_loop::skills::affect::targets_affected;
use crate::model::components::space::Position;
use crate::model::skill::Skill;
use crate::model::skill::target::{AffectObject, AffectScope, OperateType, TargetType};

const CASTER: i32 = 2001;
const CID: u32 = 1;

/// A bare offensive AoE reshaped per case; geometry comes from `fan_range`
/// exactly as the parser would deliver it (`unk;startDegree;radius;angle`).
fn geo_skill(scope: AffectScope, fan_range: [i32; 4], affect_range: i32) -> Skill {
    Skill {
        self_continuous: false,
        id: 9100,
        name: "Geo test".into(),
        operate_type: OperateType::Active,
        target_type: TargetType::Enemy,
        effect_point: -100, // bad skill
        cast_range: 900,
        effect_range: 1000,
        affect_scope: scope,
        affect_object: AffectObject::NotFriend,
        affect_range,
        fan_range,
        ..Default::default()
    }
}

fn face(world: &mut World, oid: i32, heading: i32) {
    world
        .objects
        .get_component_mut::<Position>(&oid)
        .unwrap()
        .heading = heading;
}

/// Heading units for a given degree bearing (Java divides by 182.044444444).
fn heading_units(deg: f64) -> i32 {
    (deg * 182.044444444).round() as i32
}

// ---------------------------------------------------------------------------
// FAN
// ---------------------------------------------------------------------------

/// Sonic Buster's shape: a 180° fan of radius 200. Facing east (heading 0),
/// both mobs ahead are swept — the primary *and* the bystander, which is what
/// a single-target fallback can't produce — and the mob behind is not, even
/// though all three sit well inside the radius.
#[test]
fn fan_hits_ahead_and_misses_behind() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (ahead, bystander, behind) = (NPC_OID, NPC_OID + 1, NPC_OID + 2);
    add_test_npc(&mut world, ahead, 20001, "Monster", 5, 100, 0, 0);
    add_test_npc(&mut world, bystander, 20001, "Monster", 5, 120, 40, 0);
    add_test_npc(&mut world, behind, 20001, "Monster", 5, -100, 0, 0);
    face(&mut world, CASTER, 0);
    let skill = geo_skill(AffectScope::Fan, [0, 0, 200, 180], 200);

    let hit = targets_affected(&mut world, CASTER, ahead, &skill);
    assert!(hit.contains(&ahead), "the primary in the arc is hit");
    assert!(
        hit.contains(&bystander),
        "the bystander in the arc is swept"
    );
    assert!(!hit.contains(&behind), "the mob behind the caster is not");
}

/// The radius is `fan_range[2]`: a mob inside the arc but past the radius is
/// dropped, while a second in-arc mob inside the radius is swept (so this
/// fails loudly if the fan degenerates to single-target).
#[test]
fn fan_respects_its_radius() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (near, mid, far) = (NPC_OID, NPC_OID + 1, NPC_OID + 2);
    add_test_npc(&mut world, near, 20001, "Monster", 5, 100, 0, 0);
    add_test_npc(&mut world, mid, 20001, "Monster", 5, 150, 0, 0);
    add_test_npc(&mut world, far, 20001, "Monster", 5, 600, 0, 0);
    face(&mut world, CASTER, 0);
    let skill = geo_skill(AffectScope::Fan, [0, 0, 200, 180], 200);

    let hit = targets_affected(&mut world, CASTER, near, &skill);
    assert!(hit.contains(&near));
    assert!(hit.contains(&mid), "150 is inside the 200 radius");
    assert!(!hit.contains(&far), "600 units is past the 200 radius");
}

/// **The geometry applies to the primary target too.** A fan cast at a target
/// standing behind the caster misses it — the affected set comes back without
/// the target the cast named. This is the behavioural break from the
/// radius/group scopes, where the primary is always included.
#[test]
fn fan_can_drop_its_own_primary_target() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (ahead, behind) = (NPC_OID, NPC_OID + 1);
    add_test_npc(&mut world, ahead, 20001, "Monster", 5, 100, 0, 0);
    add_test_npc(&mut world, behind, 20001, "Monster", 5, -100, 0, 0);
    face(&mut world, CASTER, 0);
    let skill = geo_skill(AffectScope::Fan, [0, 0, 200, 180], 200);

    let hit = targets_affected(&mut world, CASTER, behind, &skill);
    assert!(
        !hit.contains(&behind),
        "the named target is behind the caster"
    );
    assert!(
        hit.contains(&ahead),
        "the bystander in the arc is still swept"
    );
}

/// Java's angle test has **no wrap-around normalization** — pinned in both
/// directions. A caster whose heading maps to 350° misses a target at bearing
/// 10° (|10 − 350| = 340 > 90) even though the target is only 20° off-axis;
/// the same 20° separation away from the seam (heading 30°) hits. The live
/// server misses across the 0°/360° seam, so the port must too.
#[test]
fn fan_angle_seam_quirk_is_java_faithful() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mob = NPC_OID;
    // Bearing 10°: (cos 10°, sin 10°) × 100.
    add_test_npc(&mut world, mob, 20001, "Monster", 5, 98, 17, 0);
    let skill = geo_skill(AffectScope::Fan, [0, 0, 200, 180], 200);

    // Heading 350°: the seam sits inside the arc, and the target on the far
    // side of it is missed.
    face(&mut world, CASTER, heading_units(350.0));
    let hit = targets_affected(&mut world, CASTER, mob, &skill);
    assert!(
        !hit.contains(&mob),
        "|10 − 350| = 340 > 90: missed across the 0° seam"
    );

    // The same 20° separation with no seam in between: heading 30°.
    face(&mut world, CASTER, heading_units(30.0));
    let hit = targets_affected(&mut world, CASTER, mob, &skill);
    assert!(
        hit.contains(&mob),
        "|10 − 30| = 20 ≤ 90: hit away from the seam"
    );
}

/// `fanHalfAngle = fanAngle / 2` is integer division: a 35° fan tests against
/// 17.0, so a target 17.4° off-axis is **hit** (|17.4| ≤ 17 fails… pinned the
/// other way: 17.4 > 17.0 misses, 16.5 hits).
#[test]
fn fan_half_angle_is_integer_division() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (inside, outside) = (NPC_OID, NPC_OID + 1);
    // Bearing ≈16.7° and ≈17.35°: (cos, sin) × 1000. The primary is a third
    // mob straight ahead, so both assertions are about *swept* bystanders.
    let primary = NPC_OID + 2;
    add_test_npc(&mut world, inside, 20001, "Monster", 5, 958, 287, 0);
    add_test_npc(&mut world, outside, 20001, "Monster", 5, 954, 298, 0);
    add_test_npc(&mut world, primary, 20001, "Monster", 5, 500, 0, 0);
    face(&mut world, CASTER, 0);
    let skill = geo_skill(AffectScope::Fan, [0, 0, 1100, 35], 1100);

    let hit = targets_affected(&mut world, CASTER, primary, &skill);
    assert!(hit.contains(&inside), "16.7° ≤ 17.0 (35/2 truncated)");
    assert!(
        !hit.contains(&outside),
        "17.35° > 17.0 — 17.5 would have kept it"
    );
}

/// The rolled affect limit caps a fan sweep like any other scope.
#[test]
fn fan_respects_the_affect_limit() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    for i in 0..4 {
        add_test_npc(
            &mut world,
            NPC_OID + i,
            20001,
            "Monster",
            5,
            80 + 20 * i,
            0,
            0,
        );
    }
    face(&mut world, CASTER, 0);
    let mut skill = geo_skill(AffectScope::Fan, [0, 0, 400, 180], 400);
    skill.affect_limit = (2, 0); // min-only: no RNG draw

    let hit = targets_affected(&mut world, CASTER, NPC_OID, &skill);
    assert_eq!(hit.len(), 2, "four mobs in the arc, capped at 2: {hit:?}");
}

// ---------------------------------------------------------------------------
// SQUARE
// ---------------------------------------------------------------------------

/// A 200×100 rectangle ahead of an east-facing caster: the mob straight ahead
/// is inside, the mob 80 units to the side is not (width/2 = 50), the mob
/// behind is not.
#[test]
fn square_is_a_forward_rectangle() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (ahead, in_rect, side, behind) = (NPC_OID, NPC_OID + 1, NPC_OID + 2, NPC_OID + 3);
    add_test_npc(&mut world, ahead, 20001, "Monster", 5, 100, 0, 0);
    add_test_npc(&mut world, in_rect, 20001, "Monster", 5, 150, 30, 0);
    add_test_npc(&mut world, side, 20001, "Monster", 5, 100, 80, 0);
    add_test_npc(&mut world, behind, 20001, "Monster", 5, -100, 0, 0);
    face(&mut world, CASTER, 0);
    let skill = geo_skill(AffectScope::Square, [0, 0, 200, 100], 200);

    let hit = targets_affected(&mut world, CASTER, ahead, &skill);
    assert!(hit.contains(&ahead), "inside the 200×100 rect");
    assert!(
        hit.contains(&in_rect),
        "the bystander at (150, 30) is swept too"
    );
    assert!(!hit.contains(&side), "80 > width/2 = 50");
    assert!(!hit.contains(&behind), "the rect extends forward only");
}

/// The rectangle rotates with the caster's heading: facing north (90°), the
/// mob to the north is inside and the mob to the east — inside when facing
/// east — is out.
#[test]
fn square_rotates_with_heading() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (north, north2, east) = (NPC_OID, NPC_OID + 1, NPC_OID + 2);
    add_test_npc(&mut world, north, 20001, "Monster", 5, 0, 100, 0);
    add_test_npc(&mut world, north2, 20001, "Monster", 5, 20, 150, 0);
    add_test_npc(&mut world, east, 20001, "Monster", 5, 100, 0, 0);
    face(&mut world, CASTER, heading_units(90.0));
    let skill = geo_skill(AffectScope::Square, [0, 0, 200, 100], 200);

    let hit = targets_affected(&mut world, CASTER, north, &skill);
    assert!(hit.contains(&north), "the rect now extends along +Y");
    assert!(
        hit.contains(&north2),
        "the second northern mob is swept too"
    );
    assert!(!hit.contains(&east), "the old forward direction is out");
}

// ---------------------------------------------------------------------------
// RING_RANGE
// ---------------------------------------------------------------------------

/// Divine Judgment's shape: a 100..270 annulus around the target. The mob in
/// the ring is hit; the mob inside the inner radius and the **epicenter
/// target itself** are not — that is the donut hole.
#[test]
fn ring_range_is_a_donut_around_the_target() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let (epicenter, ring, hole, outside) = (NPC_OID, NPC_OID + 1, NPC_OID + 2, NPC_OID + 3);
    add_test_npc(&mut world, epicenter, 20001, "Monster", 5, 500, 0, 0);
    add_test_npc(&mut world, ring, 20001, "Monster", 5, 650, 0, 0); // 150 out
    add_test_npc(&mut world, hole, 20001, "Monster", 5, 550, 0, 0); // 50 out
    add_test_npc(&mut world, outside, 20001, "Monster", 5, 900, 0, 0); // 400 out
    let skill = geo_skill(AffectScope::RingRange, [0, 0, 100, 0], 270);

    let hit = targets_affected(&mut world, CASTER, epicenter, &skill);
    assert!(hit.contains(&ring), "150 sits in the 100..270 annulus");
    assert!(!hit.contains(&hole), "50 is inside the inner radius");
    assert!(
        !hit.contains(&epicenter),
        "the epicenter target is never affected"
    );
    assert!(!hit.contains(&outside), "400 is past the outer radius");
}

// ---------------------------------------------------------------------------
// Range.java's minion-buff fix (folded-in parity)
// ---------------------------------------------------------------------------

/// The dist's `Range.java` carries a local fix ("Fix minion buffs are given
/// to players"): a monster's *good* RANGE skill never sweeps players in. The
/// same sweep with a *bad* skill still hits the player — the gate is
/// good-skill-from-monster specifically.
#[test]
fn monster_group_buffs_do_not_sweep_players_in() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 500, 50);
    let (minion_a, minion_b) = (NPC_OID, NPC_OID + 1);
    add_test_npc(&mut world, minion_a, 20001, "Monster", 5, 500, 0, 0);
    add_test_npc(&mut world, minion_b, 20001, "Monster", 5, 500, 100, 0);
    let mut buff = geo_skill(AffectScope::Range, [0; 4], 200);
    buff.effect_point = 100; // a good skill
    buff.affect_object = AffectObject::All;

    // `minion_a` mass-buffs around `minion_b`: the player 50 units away is
    // inside the radius but must not be swept in.
    let hit = targets_affected(&mut world, minion_a, minion_b, &buff);
    assert!(hit.contains(&minion_b), "the fellow monster is buffed");
    assert!(!hit.contains(&CASTER), "the bystander player is not");

    // The same sweep as a *bad* skill hits the player: the gate is only for
    // good skills from monsters.
    let mut nuke = buff.clone();
    nuke.effect_point = -100;
    let hit = targets_affected(&mut world, minion_a, minion_b, &nuke);
    assert!(
        hit.contains(&CASTER),
        "a monster's offensive AoE still hits players"
    );
}
