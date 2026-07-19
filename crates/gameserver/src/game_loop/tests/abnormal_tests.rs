//! Crowd control (G19): the abnormal-state flags that make stun/sleep/paralyze
//! and root actually do something.

use super::*;

use crate::game_loop::abnormal;
use crate::model::components::{Buffs, Casting, Movement};
use crate::model::skill::{effect_flag, AffectObject, AffectScope, OperateType, Skill, SkillEffect, TargetType};

const CASTER: i32 = 2001;
const VICTIM: i32 = 2002;
const CID: u32 = 1;
const VICTIM_CID: u32 = 2;

const STUN_ID: i32 = 9300;
const ROOT_ID: i32 = 9301;

/// A CC skill shaped like the real ones: no stat modifier, the mechanic is
/// entirely the state flag.
fn cc_skill(id: i32, effect: SkillEffect, abnormal: &str) -> Skill {
    Skill {
        id,
        level: 1,
        name: format!("CC {id}"),
        operate_type: OperateType::Active,
        target_type: TargetType::Enemy,
        magic_type: 1,
        magic_level: 0,
        effect_point: -100,
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
        abnormal_time: 9,
        abnormal_level: 1,
        abnormal_type: abnormal.into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: true,
        effects: vec![effect],
    }
}

/// Land a CC skill straight onto `target`, bypassing the cast pipeline (which
/// the affect/cast tests already cover) so these cases isolate the state.
fn land(world: &mut World, skill_id: i32, target: i32) {
    let skill = world.data.skill_data.get(skill_id, 1).cloned().expect("registered");
    crate::game_loop::skills::effects::apply_skill_effects(world, CASTER, target, &skill);
}

fn cc_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = cast_test_world();
    world.data.skill_data.insert_for_test(cc_skill(STUN_ID, SkillEffect::BlockActions { conditional: false }, "STUN"));
    world.data.skill_data.insert_for_test(cc_skill(ROOT_ID, SkillEffect::Root, "ROOT_PHYSICALLY"));
    (world, db, l)
}

// ---------------------------------------------------------------------------
// The flags themselves
// ---------------------------------------------------------------------------

/// A stun sets `BLOCK_ACTIONS`; a root sets only `ROOTED`. Both land as real
/// buffs despite carrying no stat modifier — the guard that drops
/// effect-less buffs must not eat them.
#[test]
fn cc_effects_land_and_set_their_flags() {
    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    land(&mut world, STUN_ID, VICTIM);
    assert!(
        world.objects.get_component::<Buffs>(&VICTIM).is_some_and(|b| !b.0.is_empty()),
        "a stun with no stat modifier still lands as a buff"
    );
    assert!(abnormal::is_blocked_from_actions(&world, VICTIM));
    assert!(abnormal::is_movement_disabled(&world, VICTIM), "a stun also stops movement");

    // A root blocks movement but leaves actions alone.
    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    land(&mut world, ROOT_ID, VICTIM);
    assert!(!abnormal::is_blocked_from_actions(&world, VICTIM), "a root does not block actions");
    assert!(abnormal::is_movement_disabled(&world, VICTIM), "but it does stop movement");
}

/// The mask clears when the buff goes, and the creature acts again.
#[test]
fn flags_clear_when_the_buff_ends() {
    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    land(&mut world, STUN_ID, VICTIM);
    assert!(abnormal::is_blocked_from_actions(&world, VICTIM));

    crate::game_loop::skills::effects::handle_buff_expire(&mut world, VICTIM, STUN_ID);
    assert_eq!(abnormal::flags_of(&world, VICTIM), 0, "the mask is a fold over live buffs");
    assert!(!abnormal::is_blocked_from_actions(&world, VICTIM));
}

// ---------------------------------------------------------------------------
// The gates
// ---------------------------------------------------------------------------

/// A stunned player's move request is refused (and answered with a StopMove so
/// the client snaps back), while an unstunned one moves.
#[test]
fn stun_blocks_movement_requests() {
    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut vout = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    // Baseline: the move is accepted.
    handle_move_backward_to_location(&mut world, VICTIM_CID, &move_body((500, 0, 0), (50, 0, 0), 1));
    assert!(world.objects.has_component::<Movement>(&VICTIM), "moves normally when unimpaired");
    world.objects.remove_component::<Movement>(&VICTIM);
    drain(&mut vout);

    land(&mut world, STUN_ID, VICTIM);
    drain(&mut vout);
    handle_move_backward_to_location(&mut world, VICTIM_CID, &move_body((900, 0, 0), (50, 0, 0), 1));
    assert!(!world.objects.has_component::<Movement>(&VICTIM), "a stunned player cannot move");
    let pkts = drain(&mut vout);
    assert!(
        pkts.iter().any(|p| p[0] == server_packets::opcodes::STOP_MOVE),
        "the refusal is answered with StopMove"
    );
}

/// A rooted player cannot move either — that is the whole of a root.
#[test]
fn root_blocks_movement_requests() {
    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    land(&mut world, ROOT_ID, VICTIM);
    handle_move_backward_to_location(&mut world, VICTIM_CID, &move_body((900, 0, 0), (50, 0, 0), 1));
    assert!(!world.objects.has_component::<Movement>(&VICTIM), "a rooted player cannot move");
}

/// A stunned player cannot cast; a rooted one still can.
#[test]
fn stun_blocks_casting_but_root_does_not() {
    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut vout = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    // Skill 91 is the self-cast in `cast_test_world` (`TargetType::Self_`), so
    // the victim can cast it without holding a target.
    drain(&mut vout);

    land(&mut world, STUN_ID, VICTIM);
    crate::game_loop::skills::cast::use_magic(&mut world, VICTIM_CID, VICTIM, 91, false, false);
    assert!(!world.objects.has_component::<Casting>(&VICTIM), "a stunned player cannot cast");

    // Clear the stun, root instead: casting works again.
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, VICTIM, STUN_ID);
    land(&mut world, ROOT_ID, VICTIM);
    crate::game_loop::skills::cast::use_magic(&mut world, VICTIM_CID, VICTIM, 91, false, false);
    assert!(world.objects.has_component::<Casting>(&VICTIM), "a rooted player can still cast");
}

/// Landing a stun interrupts what the victim was already doing — Java's
/// `stopMove` + `abortCast`. Without this a stun would only prevent the *next*
/// action and an in-flight cast would still land.
#[test]
fn stun_interrupts_an_in_flight_cast_and_movement() {
    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut vout = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    drain(&mut vout);

    // Victim starts casting and moving.
    crate::game_loop::skills::cast::use_magic(&mut world, VICTIM_CID, VICTIM, 91, false, false);
    handle_move_backward_to_location(&mut world, VICTIM_CID, &move_body((900, 0, 0), (50, 0, 0), 1));
    assert!(world.objects.has_component::<Casting>(&VICTIM));

    land(&mut world, STUN_ID, VICTIM);
    assert!(!world.objects.has_component::<Casting>(&VICTIM), "the in-flight cast is aborted");
    assert!(!world.objects.has_component::<Movement>(&VICTIM), "and the victim is frozen in place");
}

/// A stunned *monster* stops attacking: its AI think is short-circuited while
/// the flag is up, and resumes once it clears.
#[test]
fn stunned_monster_stops_acting() {
    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    land(&mut world, STUN_ID, NPC_OID);
    assert!(abnormal::is_blocked_from_actions(&world, NPC_OID), "the mob is stunned");

    // Ticking the AI while stunned must not start a chase.
    advance_ticks(&mut world, 20);
    assert!(
        !world.objects.has_component::<Movement>(&NPC_OID),
        "a stunned mob neither chases nor wanders"
    );

    crate::game_loop::skills::effects::handle_buff_expire(&mut world, NPC_OID, STUN_ID);
    assert!(!abnormal::is_blocked_from_actions(&world, NPC_OID), "and it recovers when the stun ends");
}

/// An AoE stun (the real shape of Thunder Storm 48: a POINT_BLANK sweep whose
/// effect is `BlockActions`) stuns every mob it catches — the two G19 slices
/// composing.
#[test]
fn aoe_stun_blocks_the_whole_cluster() {
    let (mut world, _db, _l) = cc_world();
    let mut aoe = cc_skill(9302, SkillEffect::BlockActions { conditional: false }, "STUN");
    aoe.affect_scope = AffectScope::PointBlank;
    aoe.affect_object = AffectObject::NotFriend;
    aoe.affect_range = 300;
    aoe.target_type = TargetType::Self_;
    world.data.skill_data.insert_for_test(aoe);

    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::components::SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(9302, 1);

    let (a, b) = (NPC_OID, NPC_OID + 1);
    add_test_npc(&mut world, a, 20001, "Monster", 5, 100, 0, 0);
    add_test_npc(&mut world, b, 20001, "Monster", 5, 200, 0, 0);
    let far = NPC_OID + 2;
    add_test_npc(&mut world, far, 20001, "Monster", 5, 5000, 0, 0);
    drain(&mut out);

    crate::game_loop::skills::cast::use_magic(&mut world, CID, CASTER, 9302, false, false);
    advance_ticks(&mut world, 60);

    assert!(abnormal::is_blocked_from_actions(&world, a), "mob in the sweep is stunned");
    assert!(abnormal::is_blocked_from_actions(&world, b), "so is the second one");
    assert!(!abnormal::is_blocked_from_actions(&world, far), "the one outside is not");
}
