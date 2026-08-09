//! `Fear` — forced flight (G19).
//!
//! Java `handlers/effecthandlers/Fear.java`. The whole mechanic is movement:
//! `EffectFlag.FEAR` has no reader anywhere in the Java tree and `EVT_AFRAID`
//! has no handler, so a feared creature is *not* gated out of acting — it is
//! simply shoved 500 units away on landing and again on every 5-tick beat.

use super::*;
use crate::game_loop::helpers::skill_by_id;

use crate::model::components::{Movement, Position};
use crate::model::npc::{NpcAi, NpcIntention};
use crate::model::skill::{AffectObject, AffectScope, OperateType, Skill, SkillEffect, TargetType};

const CASTER: i32 = 3001;
const VICTIM: i32 = 3002;
const CID: u32 = 1;
const VICTIM_CID: u32 = 2;

/// A `Fear`-only skill, so the assertions can't be satisfied by some other
/// effect on the same skill. The real dist skills pair `Fear` with
/// `BlockControl` (and sometimes `Lethal`); `real_dist_fear_skill_*` below
/// covers that shape.
fn fear_skill(id: i32) -> Skill {
    Skill {
        self_continuous: false,
        without_action: false,
        trait_type: crate::model::skill::TraitType::None,
        item_consume_id: 0,
        item_consume_count: 0,
        id,
        level: 1,
        name: format!("Fear{id}"),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Enemy,
        magic_type: 1,
        magic_level: 0,
        effect_point: 0,
        cast_range: 0,
        effect_range: 0,
        hit_time: 0,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        // Long enough that natural expiry never races the tick assertions.
        abnormal_time: 600,
        abnormal_level: 1,
        abnormal_type: format!("FEAR{id}"),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: true,
        stay_after_death: false,
        effects: vec![SkillEffect::Fear { ticks: 5 }],
        ..Default::default()
    }
}

fn land(world: &mut World, skill_id: i32, target: i32) {
    let skill = skill_by_id(world, skill_id, 1).expect("registered");
    crate::game_loop::skills::effects::apply_skill_effects(world, CASTER, target, &skill);
}

fn dest(world: &World, oid: i32) -> Option<(i32, i32)> {
    world
        .objects
        .get_component::<Movement>(&oid)
        .map(|Movement(m)| (m.dest_x, m.dest_y))
}

fn pos(world: &World, oid: i32) -> (i32, i32) {
    let p = world.objects.get_component::<Position>(&oid).unwrap();
    (p.x, p.y)
}

/// One fear beat is `5 * 666 ms` ≈ 3330 ms ≈ 34 game ticks. A 500-unit leg
/// takes a little longer than that at ordinary run speed, so a beat generally
/// lands mid-leg and re-aims it — which is exactly Java's behaviour.
const ONE_BEAT: u64 = 40;

/// Advance with the scheduler **and** the movement/AI systems running, the way
/// the real loop does. Plain `advance_ticks` only fires due tasks, so a fear
/// beat would arm a move that never actually travels — and the mob AI would
/// never get the chance to chase back that
/// `feared_mob_stops_thinking_until_it_arrives` needs it to have.
fn advance_moving(world: &mut World, n: u64) {
    for _ in 0..n {
        world.tick += 1;
        apply_due_tasks(world);
        if world
            .tick
            .is_multiple_of(crate::game_loop::npc_ai::NPC_THINK_PERIOD)
        {
            crate::game_loop::npc_ai::npc_ai_tick(world);
        }
        crate::game_loop::visibility::movement_tick(world);
    }
}

// ---------------------------------------------------------------------------
// The shove itself
// ---------------------------------------------------------------------------

/// `Fear.onStart` → `fearAction(effector, effected)`: the victim runs *directly
/// away from the caster*, 500 units along the caster→victim bearing. With the
/// caster at the origin and the victim due east of them, that is straight
/// further east — same y, x + 500.
#[test]
fn fear_shoves_the_victim_directly_away_from_the_caster() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(fear_skill(9600));
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_player(&mut world, VICTIM_CID, VICTIM, 100, 0, 0);

    assert!(
        dest(&world, VICTIM).is_none(),
        "not moving before the fear lands"
    );
    land(&mut world, 9600, VICTIM);

    let (dx, dy) = dest(&world, VICTIM).expect("the fear started a move");
    assert!(
        (dx - 600).abs() <= 2,
        "flees 500 further from the caster (x 100 -> ~600), got {dx}"
    );
    assert!(
        dy.abs() <= 2,
        "and straight along the caster->victim line, got y {dy}"
    );
}

/// `onActionTime` passes `null` for the effector, so each repeat steers by the
/// victim's **own heading** rather than re-aiming away from the caster. The
/// heading was set by the first shove, so the victim keeps going the same way:
/// each beat moves them further out, monotonically.
#[test]
fn fear_keeps_pushing_the_victim_further_each_beat() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(fear_skill(9601));
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_player(&mut world, VICTIM_CID, VICTIM, 100, 0, 0);

    land(&mut world, 9601, VICTIM);
    let start_x = pos(&world, VICTIM).0;

    // Let the first leg run out, then take a reading after each of two beats.
    advance_moving(&mut world, ONE_BEAT * 2);
    let after_one = pos(&world, VICTIM).0;
    advance_moving(&mut world, ONE_BEAT * 2);
    let after_two = pos(&world, VICTIM).0;

    assert!(
        after_one > start_x,
        "the first leg ran: {start_x} -> {after_one}"
    );
    assert!(
        after_two > after_one,
        "and the fear kept shoving: {after_one} -> {after_two}"
    );
    assert!(
        pos(&world, VICTIM).1.abs() <= 8,
        "still fleeing along the original bearing"
    );
}

/// The chain is the buff's: once the fear expires nothing reschedules, so the
/// victim stops where they are. (The tick handler bails on a missing buff — the
/// same contract the DoT chain has.)
#[test]
fn fear_stops_pushing_once_the_buff_is_gone() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(fear_skill(9602));
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_player(&mut world, VICTIM_CID, VICTIM, 100, 0, 0);

    land(&mut world, 9602, VICTIM);
    advance_moving(&mut world, ONE_BEAT * 2);
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, VICTIM, 9602);

    // Let the leg that was already in flight run itself out — expiry doesn't
    // teleport anyone to a halt, it just stops arming new legs.
    for _ in 0..1_000 {
        if !world.objects.has_component::<Movement>(&VICTIM) {
            break;
        }
        advance_moving(&mut world, 1);
    }
    assert!(
        !world.objects.has_component::<Movement>(&VICTIM),
        "the in-flight leg finished"
    );

    // Well past several beats' worth: nothing shoves the victim again.
    let settled = pos(&world, VICTIM);
    advance_moving(&mut world, ONE_BEAT * 4);
    assert_eq!(pos(&world, VICTIM), settled, "no buff, no more shoves");
}

// ---------------------------------------------------------------------------
// canStart
// ---------------------------------------------------------------------------

/// `Fear.canStart` bails on `effected.isRaid()` — a raid boss is never feared,
/// the same immunity `Mute` has. Without it a single fear would walk a boss out
/// of its own lair.
#[test]
fn fear_never_moves_a_raid_boss() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(fear_skill(9603));
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 90001, "RaidBoss", 40, 100, 0, 0);

    land(&mut world, 9603, NPC_OID);
    assert!(dest(&world, NPC_OID).is_none(), "a raid boss does not flee");
    assert_eq!(
        world
            .objects
            .get_component::<NpcAi>(&NPC_OID)
            .map(|ai| ai.intention),
        Some(NpcIntention::Active),
        "and its AI is untouched"
    );
}

/// The siege-defence exemption: `Defender`/`FortCommander`/`SiegeFlag` (and the
/// `SIEGE_WEAPON` race) are all `Attackable`, so without Java's explicit
/// carve-out a fear would scatter a castle's stationed defenders off the wall.
#[test]
fn fear_never_moves_siege_defenders() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(fear_skill(9604));
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);

    for (i, type_name) in ["Defender", "FortCommander", "SiegeFlag"]
        .iter()
        .enumerate()
    {
        let oid = NPC_OID + i as i32;
        add_test_npc(&mut world, oid, 90010 + i as i32, type_name, 40, 100, 0, 0);
        land(&mut world, 9604, oid);
        assert!(dest(&world, oid).is_none(), "{type_name} holds its post");
    }
}

/// A plain non-`Attackable` NPC (a merchant) is outside Java's
/// `isPlayer() || isSummon() || isAttackable()` set, so it is not fearable
/// either — the effect simply never starts.
#[test]
fn fear_does_not_move_a_non_attackable_npc() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(fear_skill(9605));
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 90020, "Merchant", 40, 100, 0, 0);

    land(&mut world, 9605, NPC_OID);
    assert!(
        dest(&world, NPC_OID).is_none(),
        "a merchant is not fearable"
    );
}

// ---------------------------------------------------------------------------
// The NPC AI half
// ---------------------------------------------------------------------------

/// The load-bearing bit of the NPC port: `AttackableAI.onEvtThink`'s switch has
/// no `AI_INTENTION_MOVE_TO` case, so a fleeing mob thinks about nothing while
/// it runs. Without that gate the next think tick re-issues a chase and the mob
/// simply walks back to its victim — the flee would be invisible.
///
/// `onEvtArrived` then puts it back on `Active`.
#[test]
fn feared_mob_stops_thinking_until_it_arrives() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(fear_skill(9606));
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 90030, "Monster", 40, 100, 0, 0);
    // Aggroed onto the caster: this is what would drag it back mid-flight.
    world
        .objects
        .get_component_mut::<NpcAi>(&NPC_OID)
        .unwrap()
        .intention = NpcIntention::Attack;
    world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&NPC_OID)
        .unwrap()
        .0
        .entry(CASTER)
        .or_default()
        .hate = 100.0;

    land(&mut world, 9606, NPC_OID);
    assert_eq!(
        world
            .objects
            .get_component::<NpcAi>(&NPC_OID)
            .unwrap()
            .intention,
        NpcIntention::MoveTo,
        "the fear takes the mob off its attack intention"
    );
    let fleeing_to = dest(&world, NPC_OID).expect("and starts it running");
    assert!(
        fleeing_to.0 > 100,
        "away from the caster at the origin, got {fleeing_to:?}"
    );

    // Run out the leg with the AI ticking. The mob must not have turned
    // around: `think_attack` never got to re-target it.
    advance_moving(&mut world, ONE_BEAT);
    assert!(
        pos(&world, NPC_OID).0 > 100,
        "the mob fled instead of chasing back"
    );
}

/// `Fear.onExit`: `if (!effected.isPlayer()) notifyEvent(EVT_THINK)` — a mob
/// whose fear runs out mid-flight is still parked on `MoveTo`, whose think arm
/// does nothing. Without the re-think it would finish its last leg before ever
/// re-engaging.
#[test]
fn fear_expiry_returns_the_mob_to_active() {
    let (mut world, _db, _l) = cast_test_world();
    world.data.skill_data.insert_for_test(fear_skill(9607));
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 90040, "Monster", 40, 100, 0, 0);

    land(&mut world, 9607, NPC_OID);
    assert_eq!(
        world
            .objects
            .get_component::<NpcAi>(&NPC_OID)
            .unwrap()
            .intention,
        NpcIntention::MoveTo
    );

    crate::game_loop::skills::effects::handle_buff_expire(&mut world, NPC_OID, 9607);
    assert_eq!(
        world
            .objects
            .get_component::<NpcAi>(&NPC_OID)
            .unwrap()
            .intention,
        NpcIntention::Active,
        "the mob starts thinking again the moment the fear drops"
    );
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// The real skills parse to a `Fear` effect alongside the `BlockControl` they
/// all carry. The pairing is why this was a *quiet* gap: the buff already
/// landed (icon, duration, `BLOCK_CONTROL`) off `BlockControl` alone, so the
/// debuff looked like it worked — it just never moved anyone.
#[test]
fn real_dist_fear_skills_parse_a_fear_effect() {
    const DIST: &str = crate::data::DIST_GAME;
    let skills = crate::data::skill_data::SkillData::load_from(DIST);

    // Horror 65, Banish Undead 405, Banish Seraph 450, Fear 1092, Curse Fear
    // 1169, Word of Fear 1272, Mass Curse Fear 1381, Turn Undead 1400 — every
    // learnable `Fear` instance in this dist.
    for id in [65, 405, 450, 1092, 1169, 1272, 1381, 1400] {
        let skill = skills
            .get(id, 1)
            .unwrap_or_else(|| panic!("skill {id} loads"));
        assert!(
            skill
                .effects
                .iter()
                .any(|e| matches!(e, SkillEffect::Fear { ticks: 5 })),
            "skill {id} carries Fear with Java's hard-coded 5 ticks: {:?}",
            skill.effects
        );
        assert!(
            skill
                .effects
                .iter()
                .any(|e| matches!(e, SkillEffect::BlockControl)),
            "skill {id} still carries its BlockControl too"
        );
        assert!(
            skill.effect_flags() & crate::model::skill::effect_flag::FEAR != 0,
            "skill {id} contributes the FEAR flag"
        );
    }
}
