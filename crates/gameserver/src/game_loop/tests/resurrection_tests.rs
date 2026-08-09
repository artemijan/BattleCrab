//! `Resurrection` — Resurrection 1016, Mass Resurrection 1254 (G19).
//!
//! A resurrection does not revive anyone directly: it *proposes* a revive, the
//! corpse answers a `ConfirmDlg`, and only then do they come back — with the
//! skill's own HP/MP/CP percentages and a share of the XP the death cost.

use super::*;
use crate::game_loop::abnormal::has_buff;

use crate::model::skill::{SkillEffect, TargetType};

use crate::game_loop::death::{
    do_revive_with, handle_revive_answer, resurrect_restore_percent, revive_request,
};

const REVIVER: i32 = 9601;
const CORPSE: i32 = 9602;
const CID: u32 = 1;
const TCID: u32 = 2;
const DIST: &str = crate::data::DIST_GAME;

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

    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0, 1016, 0);

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

    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0, 1016, 0);
    let first = world
        .objects
        .get_component::<crate::model::Player>(&CORPSE)
        .unwrap()
        .revive_request;
    revive_request(&mut world, REVIVER, CORPSE, 90, 10, 10, 0, 1016, 0);
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

    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0, 1016, 0);
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
    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0, 1016, 0);

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
    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0, 1016, 0);

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

// ---------------------------------------------------------------------------
// The siege battlefield (G24)
// ---------------------------------------------------------------------------

/// A point inside castle 1's `SiegeZone`, and the castle it belongs to.
const SIEGE_POS: (i32, i32, i32) = (-17964, 110730, -1000);
const SIEGE_CASTLE: i32 = 1;

/// A world with a siege in progress over castle 1 and both actors standing in
/// its zone.
fn battlefield() -> (World, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
    use crate::model::siege::{Siege, SiegeClanType};
    let (mut world, _db, _l) = cast_test_world();
    world.data.zone_data = crate::data::zone_data::ZoneData::load_from(DIST);
    let (x, y, z) = SIEGE_POS;
    let r = ingame_caster(&mut world, CID, REVIVER, x, y);
    let _c = ingame_caster(&mut world, TCID, CORPSE, x, y);
    for oid in [REVIVER, CORPSE] {
        let p = world
            .objects
            .get_component_mut::<crate::model::components::Position>(&oid)
            .unwrap();
        p.z = z;
    }
    assert_eq!(
        world.data.zone_data.siege_castle_at(x, y, z),
        Some(SIEGE_CASTLE),
        "the fixture really stands on a battlefield"
    );
    let mut siege = Siege::new(SIEGE_CASTLE);
    siege.in_progress = true;
    siege.add_clan(500, SiegeClanType::Owner);
    siege.add_clan(700, SiegeClanType::Attacker);
    siege.control_tower_count = 2;
    world.sieges.insert(SIEGE_CASTLE, siege);
    kill(&mut world, CORPSE, 10_000);
    (world, r)
}

fn set_clan(world: &mut World, oid: i32, clan_id: i32) {
    world
        .objects
        .get_component_mut::<crate::model::Player>(&oid)
        .unwrap()
        .clan_id = clan_id;
}

fn proposed(world: &World) -> bool {
    world
        .objects
        .get_component::<crate::model::Player>(&CORPSE)
        .unwrap()
        .revive_request
        .is_some()
}

/// **A normal resurrection never works on a battlefield.** Java's
/// `ConditionPlayerCanResurrect` refuses in *every* branch once a siege is in
/// progress — the tower/flag counts only pick the message. Before this, a
/// Bishop could freely raise defenders mid-siege.
#[test]
fn a_normal_resurrection_is_refused_during_a_siege() {
    let (mut world, _rx) = battlefield();
    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0, 1016, 0);
    assert!(
        !proposed(&world),
        "no dialog was put in front of the corpse"
    );
}

/// Which message depends on the corpse's side: no clan and "some other case"
/// both read the generic battleground line, a defender with **no control
/// towers** left reads the guardian-tower one, and an attacker with **no base
/// camp** reads that one.
#[test]
fn the_refusal_message_depends_on_the_side_and_the_towers() {
    use crate::network::server_packets::sm_ids;

    let refusal = |setup: &dyn Fn(&mut World)| {
        let (mut world, mut rx) = battlefield();
        setup(&mut world);
        drain(&mut rx);
        revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0, 1016, 0);
        sm_ids_of(&drain(&mut rx))
    };

    // Clanless corpse → the generic line.
    assert!(refusal(&|_w| {}).contains(&sm_ids::IT_IS_NOT_POSSIBLE_TO_RESURRECT_IN_BATTLEGROUNDS));

    // A defender whose towers are all down.
    assert!(
        refusal(&|w| {
            set_clan(w, CORPSE, 500);
            w.sieges.get_mut(&SIEGE_CASTLE).unwrap().control_tower_count = 0;
        })
        .contains(&sm_ids::THE_GUARDIAN_TOWER_HAS_BEEN_DESTROYED_AND_RESURRECTION_IS_NOT_POSSIBLE)
    );

    // …but a defender who still holds a tower gets the generic line, and is
    // refused all the same.
    assert!(
        refusal(&|w| set_clan(w, CORPSE, 500))
            .contains(&sm_ids::IT_IS_NOT_POSSIBLE_TO_RESURRECT_IN_BATTLEGROUNDS)
    );

    // An attacker with no planted flag.
    assert!(
        refusal(&|w| set_clan(w, CORPSE, 700))
            .contains(&sm_ids::IF_A_BASE_CAMP_DOES_NOT_EXIST_RESURRECTION_IS_NOT_POSSIBLE)
    );

    // With a base camp planted, the generic refusal again.
    assert!(
        refusal(&|w| {
            set_clan(w, CORPSE, 700);
            w.sieges
                .get_mut(&SIEGE_CASTLE)
                .unwrap()
                .flags
                .push((700, 1));
        })
        .contains(&sm_ids::IT_IS_NOT_POSSIBLE_TO_RESURRECT_IN_BATTLEGROUNDS)
    );
}

/// **Two things get through**: the Blessed Scroll of Resurrection
/// (Battleground) skill 2393, and — because Java's condition opens with
/// `if (skill.getAffectRange() > 0) return true;` — *any* AoE resurrection,
/// which on this dist means Mass Resurrection 1254.
#[test]
fn the_battleground_scroll_and_mass_resurrection_still_work() {
    let (mut world, _rx) = battlefield();
    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0, 2393, 0);
    assert!(proposed(&world), "the battleground scroll is the exception");

    let (mut world, _rx) = battlefield();
    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0, 1254, 400);
    assert!(
        proposed(&world),
        "an affectRange > 0 skips the whole condition — Java's own shortcut"
    );
}

/// Off the battlefield, or before the siege starts, nothing is refused.
#[test]
fn a_resurrection_outside_a_running_siege_is_untouched() {
    // Siege registered but not started.
    let (mut world, _rx) = battlefield();
    world.sieges.get_mut(&SIEGE_CASTLE).unwrap().in_progress = false;
    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0, 1016, 0);
    assert!(proposed(&world), "a pending siege blocks nothing");

    // In progress, but the corpse is nowhere near the castle.
    let (mut world, _rx) = battlefield();
    {
        let p = world
            .objects
            .get_component_mut::<crate::model::components::Position>(&CORPSE)
            .unwrap();
        p.x = 0;
        p.y = 0;
        p.z = 0;
    }
    revive_request(&mut world, REVIVER, CORPSE, 40, 70, 70, 0, 1016, 0);
    assert!(proposed(&world), "the block is the *zone*, not the siege");
}

// ---------------------------------------------------------------------------
// G34 S4 sub-slice 16 — ResurrectionSpecial (the auto-resurrect)
// ---------------------------------------------------------------------------

/// **`ResurrectionSpecial`** (Salvation 1410, Soul of the Phoenix 438) is an
/// auto-resurrect, and the whole mechanic is in the *wrong* lifecycle hook for
/// anyone porting it by eye: the buff does nothing at all while it is up, and
/// fires its revive proposal from **`onExit`** — which is what death does to
/// it. A port that wired it to `onStart` would propose a revive to a living
/// player and then do nothing when they actually died.
#[test]
fn salvation_proposes_its_revive_when_death_strips_the_buff() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CORPSE, 0, 0);

    let salvation = crate::model::skill::Skill {
        self_continuous: false,
        id: 1410,
        level: 1,
        target_type: TargetType::Self_,
        abnormal_time: 1200,
        abnormal_type: "SALVATION".into(),
        effects: vec![SkillEffect::ResurrectionSpecial {
            power: 100,
            hp_percent: 0,
            mp_percent: 0,
            cp_percent: 0,
        }],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(salvation.clone());
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, CORPSE, CORPSE, &salvation);

    let pending = |world: &World| {
        world
            .objects
            .get_component::<crate::model::Player>(&CORPSE)
            .unwrap()
            .revive_request
    };
    assert!(
        pending(&world).is_none(),
        "while the buff is up it does nothing at all"
    );

    // Now die. Death strips the buff, which is what fires `onExit`.
    kill(&mut world, CORPSE, 10_000);
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, CORPSE, 1410);

    assert!(
        pending(&world).is_some(),
        "losing the buff is what proposes the revive"
    );
}

/// **…except in an olympiad match**, where Java returns before proposing
/// (`effected.getActingPlayer().isInOlympiadMode()`). An auto-resurrect inside
/// a duel to the death would decide the match, which is why the gate exists.
#[test]
fn salvation_does_not_fire_inside_an_olympiad_match() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CORPSE, 0, 0);

    let salvation = crate::model::skill::Skill {
        self_continuous: false,
        id: 1410,
        level: 1,
        target_type: TargetType::Self_,
        abnormal_time: 1200,
        abnormal_type: "SALVATION".into(),
        effects: vec![SkillEffect::ResurrectionSpecial {
            power: 100,
            hp_percent: 0,
            mp_percent: 0,
            cp_percent: 0,
        }],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(salvation.clone());
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, CORPSE, CORPSE, &salvation);

    // Put the holder in a running match.
    world
        .olympiad
        .matches
        .push(crate::model::olympiad::OlympiadMatch {
            arena: 0,
            player_a: CORPSE,
            player_b: CORPSE + 1,
            instance_id: 0,
            deadline_tick: u64::MAX,
            return_a: (0, 0, 0),
            return_b: (0, 0, 0),
        });

    kill(&mut world, CORPSE, 10_000);
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, CORPSE, 1410);

    assert!(
        world
            .objects
            .get_component::<crate::model::Player>(&CORPSE)
            .unwrap()
            .revive_request
            .is_none(),
        "no auto-resurrect inside an olympiad match"
    );
}

/// The other half of the flag: like Noblesse Blessing, a `RESURRECTION_SPECIAL`
/// holder loses **only that effect** on death and keeps the rest of its buffs.
/// Without this the auto-resurrect would revive you stripped, which is the
/// opposite of what the buff is for.
#[test]
fn salvation_spares_the_rest_of_the_buffs_through_death() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CORPSE, 0, 0);

    let salvation = crate::model::skill::Skill {
        self_continuous: false,
        id: 1410,
        level: 1,
        target_type: TargetType::Self_,
        abnormal_time: 1200,
        abnormal_type: "SALVATION".into(),
        effects: vec![SkillEffect::ResurrectionSpecial {
            power: 100,
            hp_percent: 0,
            mp_percent: 0,
            cp_percent: 0,
        }],
        ..Default::default()
    };
    // An ordinary buff that does *not* survive death on its own.
    let haste = crate::model::skill::Skill {
        self_continuous: false,
        id: 9430,
        level: 1,
        target_type: TargetType::Self_,
        abnormal_time: 1200,
        abnormal_type: "HASTE".into(),
        effects: vec![SkillEffect::StatModifier(
            crate::model::skill::StatModifierEffect {
                stat: crate::model::stats::Stat::RunSpeed,
                mode: crate::model::stats::StatModifierType::Diff,
                amount: 30.0,
                ..Default::default()
            },
        )],
        ..Default::default()
    };
    for s in [&salvation, &haste] {
        world.data.skill_data.insert_for_test((*s).clone());
        crate::game_loop::skills::effects::apply_skill_effects(&mut world, CORPSE, CORPSE, s);
    }

    let has = |world: &World, id: i32| has_buff(world, CORPSE, id);
    assert!(has(&world, 1410) && has(&world, 9430), "both are up");

    crate::game_loop::death::stop_effects_on_death_for_test(&mut world, CORPSE);

    assert!(!has(&world, 1410), "Salvation itself is consumed");
    assert!(
        has(&world, 9430),
        "but everything else survives — that is what the flag buys"
    );
}
