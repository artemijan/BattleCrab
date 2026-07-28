//! `Resurrection` — Resurrection 1016, Mass Resurrection 1254 (G19).
//!
//! A resurrection does not revive anyone directly: it *proposes* a revive, the
//! corpse answers a `ConfirmDlg`, and only then do they come back — with the
//! skill's own HP/MP/CP percentages and a share of the XP the death cost.

use super::*;

use crate::model::skill::{SkillEffect, TargetType};

use crate::game_loop::death::{
    do_revive_with, handle_revive_answer, resurrect_restore_percent, revive_request,
};

const REVIVER: i32 = 9601;
const CORPSE: i32 = 9602;
const CID: u32 = 1;
const TCID: u32 = 2;
const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

fn kill(world: &mut World, oid: i32, lost_exp: i64) {
    world
        .objects
        .get_component_mut::<Vitals>(&oid)
        .unwrap()
        .dead = true;
    let p = world
        .objects
        .get_component_mut::<crate::model::Player>(&oid)
        .unwrap();
    p.lost_exp_on_death = lost_exp;
    p.exp = 100_000;
}

fn is_dead(world: &World, oid: i32) -> bool {
    world.objects.get_component::<Vitals>(&oid).unwrap().dead
}

// ---------------------------------------------------------------------------
// The restore-percent formula
// ---------------------------------------------------------------------------

/// `calculateSkillResurrectRestorePercent`. The quirk worth pinning is the
/// **extra flat +20** once the WIT bonus has already added more than 20 — high
/// WIT jumps rather than scaling smoothly.
#[test]
fn restore_percent_matches_javas_formula_including_the_plus_twenty_jump() {
    // 0 and 100 short-circuit before any scaling.
    assert_eq!(resurrect_restore_percent(0.0, 5.0), 0.0);
    assert_eq!(resurrect_restore_percent(100.0, 5.0), 100.0);
    // A modest bonus scales: 40 * 1.2 = 48, and 48 - 40 = 8 ≤ 20, no jump.
    assert!((resurrect_restore_percent(40.0, 1.2) - 48.0).abs() < 1e-9);
    // A big one jumps: 40 * 1.6 = 64; 64 - 40 = 24 > 20, so +20 → 84.
    assert!((resurrect_restore_percent(40.0, 1.6) - 84.0).abs() < 1e-9);
    // Clamped at 90 …
    assert_eq!(resurrect_restore_percent(80.0, 2.0), 90.0);
    // … and never below the declared base.
    assert_eq!(resurrect_restore_percent(50.0, 0.5), 50.0);
}

// ---------------------------------------------------------------------------
// The proposal
// ---------------------------------------------------------------------------

/// A proposal does **not** revive — it only puts the request on the corpse.
#[test]
fn a_proposal_does_not_revive_by_itself() {
    let (mut world, _db, _l) = cast_test_world();
    let _r = ingame_caster(&mut world, CID, REVIVER, 0, 0);
    let _c = ingame_caster(&mut world, TCID, CORPSE, 40, 0);
    kill(&mut world, CORPSE, 10_000);

    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0);

    assert!(is_dead(&world, CORPSE), "still dead until they accept");
    assert!(
        world
            .objects
            .get_component::<crate::model::Player>(&CORPSE)
            .unwrap()
            .revive_request
            .is_some(),
        "but the proposal is recorded"
    );
}

/// A second proposal while one is outstanding is refused — what stops two
/// clerics from racing.
#[test]
fn a_second_proposal_is_refused_while_one_is_pending() {
    let (mut world, _db, _l) = cast_test_world();
    let _r = ingame_caster(&mut world, CID, REVIVER, 0, 0);
    let _c = ingame_caster(&mut world, TCID, CORPSE, 40, 0);
    kill(&mut world, CORPSE, 10_000);

    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0);
    let first = world
        .objects
        .get_component::<crate::model::Player>(&CORPSE)
        .unwrap()
        .revive_request;
    revive_request(&mut world, REVIVER, CORPSE, 90, 10, 10, 0);
    let second = world
        .objects
        .get_component::<crate::model::Player>(&CORPSE)
        .unwrap()
        .revive_request;

    assert_eq!(
        first, second,
        "the pending proposal is untouched by the second attempt"
    );
}

/// A living player is never proposed to.
#[test]
fn a_living_player_gets_no_proposal() {
    let (mut world, _db, _l) = cast_test_world();
    let _r = ingame_caster(&mut world, CID, REVIVER, 0, 0);
    let _c = ingame_caster(&mut world, TCID, CORPSE, 40, 0);

    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0);
    assert!(
        world
            .objects
            .get_component::<crate::model::Player>(&CORPSE)
            .unwrap()
            .revive_request
            .is_none()
    );
}

// ---------------------------------------------------------------------------
// The answer
// ---------------------------------------------------------------------------

/// Accepting revives with the skill's own percentages and restores its share of
/// the lost XP.
#[test]
fn accepting_revives_with_the_skills_percentages_and_restores_xp() {
    let (mut world, _db, _l) = cast_test_world();
    let _r = ingame_caster(&mut world, CID, REVIVER, 0, 0);
    let _c = ingame_caster(&mut world, TCID, CORPSE, 40, 0);
    kill(&mut world, CORPSE, 10_000);
    let max_hp = world
        .objects
        .get_component::<Vitals>(&CORPSE)
        .unwrap()
        .max_hp as f64;

    // Force a known restore percent by driving `do_revive_with` directly; the
    // WIT-scaled path is covered by the formula test above.
    do_revive_with(&mut world, CORPSE, 50, 30, 0, 40.0);

    assert!(!is_dead(&world, CORPSE), "back up");
    let v = world.objects.get_component::<Vitals>(&CORPSE).unwrap();
    assert!(
        (v.cur_hp - max_hp * 0.5).abs() < 1e-6,
        "50% HP, not the config default"
    );
    let p = world
        .objects
        .get_component::<crate::model::Player>(&CORPSE)
        .unwrap();
    assert_eq!(
        p.exp,
        100_000 + 4_000,
        "40% of the 10 000 lost XP is given back"
    );
    assert_eq!(p.lost_exp_on_death, 0, "and the debt is cleared");
}

/// Declining consumes the proposal and leaves the corpse dead.
#[test]
fn declining_leaves_the_corpse_dead() {
    let (mut world, _db, _l) = cast_test_world();
    let _r = ingame_caster(&mut world, CID, REVIVER, 0, 0);
    let _c = ingame_caster(&mut world, TCID, CORPSE, 40, 0);
    kill(&mut world, CORPSE, 10_000);
    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0);

    assert!(
        handle_revive_answer(&mut world, CORPSE, false),
        "the reply was claimed by the revive flow"
    );
    assert!(is_dead(&world, CORPSE), "still dead");
    assert!(
        world
            .objects
            .get_component::<crate::model::Player>(&CORPSE)
            .unwrap()
            .revive_request
            .is_none(),
        "and the proposal is consumed, so a new one can be made"
    );
}

/// An answer with no proposal pending is **not** claimed — which is what lets
/// the admin-confirm flow keep using the same packet.
#[test]
fn an_unrelated_answer_is_not_claimed() {
    let (mut world, _db, _l) = cast_test_world();
    let _r = ingame_caster(&mut world, CID, REVIVER, 0, 0);
    assert!(
        !handle_revive_answer(&mut world, REVIVER, true),
        "no proposal, not ours"
    );
}

/// Java re-checks the corpse is still dead: they may have used "to village"
/// while the dialog sat on screen, and must not be revived on top of that.
#[test]
fn accepting_after_already_respawning_does_nothing() {
    let (mut world, _db, _l) = cast_test_world();
    let _r = ingame_caster(&mut world, CID, REVIVER, 0, 0);
    let _c = ingame_caster(&mut world, TCID, CORPSE, 40, 0);
    kill(&mut world, CORPSE, 10_000);
    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0);

    // They respawned by themselves first.
    world
        .objects
        .get_component_mut::<Vitals>(&CORPSE)
        .unwrap()
        .dead = false;
    let exp_before = world
        .objects
        .get_component::<crate::model::Player>(&CORPSE)
        .unwrap()
        .exp;

    assert!(handle_revive_answer(&mut world, CORPSE, true), "claimed");
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::Player>(&CORPSE)
            .unwrap()
            .exp,
        exp_before,
        "no second helping of XP"
    );
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// Both skills parse, and Resurrection 1016 targets `PC_BODY` — a dead player
/// corpse, a target type this port had no equivalent for before.
#[test]
fn real_dist_resurrection_skills_parse() {
    let skills = crate::data::skill_data::SkillData::load_from(DIST);

    let res = skills.get(1016, 2).expect("Resurrection loads");
    assert_eq!(
        res.target_type,
        TargetType::PcBody,
        "it is cast on a corpse"
    );
    assert!(
        res.effects
            .iter()
            .any(|e| matches!(e, SkillEffect::Resurrection { power, .. } if *power > 0)),
        "with a real restore power at level 2: {:?}",
        res.effects
    );

    let mass = skills.get(1254, 2).expect("Mass Resurrection loads");
    assert!(
        mass.effects
            .iter()
            .any(|e| matches!(e, SkillEffect::Resurrection { .. }))
    );
}

/// Level 1 of both declares `power = 0` — the skill revives but restores no XP.
/// Same shape as Rage's level-1 zero, and worth pinning so the formula's
/// `base == 0` short-circuit is exercised by real data.
#[test]
fn level_one_resurrection_restores_no_xp() {
    let skills = crate::data::skill_data::SkillData::load_from(DIST);
    let res = skills.get(1016, 1).expect("Resurrection lvl 1 loads");
    let power = res.effects.iter().find_map(|e| match e {
        SkillEffect::Resurrection { power, .. } => Some(*power),
        _ => None,
    });
    assert_eq!(power, Some(0), "level 1 restores no XP");
    assert_eq!(
        resurrect_restore_percent(0.0, 2.0),
        0.0,
        "and the formula short-circuits on it"
    );
}
