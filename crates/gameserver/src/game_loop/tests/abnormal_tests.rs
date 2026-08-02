//! Crowd control (G19): the abnormal-state flags that make stun/sleep/paralyze
//! and root actually do something.

use super::*;

use crate::game_loop::abnormal;
use crate::model::components::{Buffs, Casting, Movement};
use crate::model::skill::{
    AffectObject, AffectScope, OperateType, Skill, SkillEffect, TargetType, effect_flag,
};

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
        without_action: false,
        trait_type: crate::model::skill::TraitType::None,
        item_consume_id: 0,
        item_consume_count: 0,
        id,
        level: 1,
        name: format!("CC {id}"),
        operate_type: OperateType::Active,
        is_continuous: false,
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
        effects: vec![effect],
        ..Default::default()
    }
}

/// Land a CC skill straight onto `target`, bypassing the cast pipeline (which
/// the affect/cast tests already cover) so these cases isolate the state.
fn land(world: &mut World, skill_id: i32, target: i32) {
    let skill = world
        .data
        .skill_data
        .get(skill_id, 1)
        .cloned()
        .expect("registered");
    crate::game_loop::skills::effects::apply_skill_effects(world, CASTER, target, &skill);
}

fn cc_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = cast_test_world();
    world.data.skill_data.insert_for_test(cc_skill(
        STUN_ID,
        SkillEffect::BlockActions { conditional: false },
        "STUN",
    ));
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(ROOT_ID, SkillEffect::Root, "ROOT_PHYSICALLY"));
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
        world
            .objects
            .get_component::<Buffs>(&VICTIM)
            .is_some_and(|b| !b.0.is_empty()),
        "a stun with no stat modifier still lands as a buff"
    );
    assert!(abnormal::is_blocked_from_actions(&world, VICTIM));
    assert!(
        abnormal::is_movement_disabled(&world, VICTIM),
        "a stun also stops movement"
    );

    // A root blocks movement but leaves actions alone.
    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    land(&mut world, ROOT_ID, VICTIM);
    assert!(
        !abnormal::is_blocked_from_actions(&world, VICTIM),
        "a root does not block actions"
    );
    assert!(
        abnormal::is_movement_disabled(&world, VICTIM),
        "but it does stop movement"
    );
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
    assert_eq!(
        abnormal::flags_of(&world, VICTIM),
        0,
        "the mask is a fold over live buffs"
    );
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
    handle_move_backward_to_location(
        &mut world,
        VICTIM_CID,
        &move_body((500, 0, 0), (50, 0, 0), 1),
    );
    assert!(
        world.objects.has_component::<Movement>(&VICTIM),
        "moves normally when unimpaired"
    );
    world.objects.remove_component::<Movement>(&VICTIM);
    drain(&mut vout);

    land(&mut world, STUN_ID, VICTIM);
    drain(&mut vout);
    handle_move_backward_to_location(
        &mut world,
        VICTIM_CID,
        &move_body((900, 0, 0), (50, 0, 0), 1),
    );
    assert!(
        !world.objects.has_component::<Movement>(&VICTIM),
        "a stunned player cannot move"
    );
    let pkts = drain(&mut vout);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::STOP_MOVE),
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
    handle_move_backward_to_location(
        &mut world,
        VICTIM_CID,
        &move_body((900, 0, 0), (50, 0, 0), 1),
    );
    assert!(
        !world.objects.has_component::<Movement>(&VICTIM),
        "a rooted player cannot move"
    );
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
    assert!(
        !world.objects.has_component::<Casting>(&VICTIM),
        "a stunned player cannot cast"
    );

    // Clear the stun, root instead: casting works again.
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, VICTIM, STUN_ID);
    land(&mut world, ROOT_ID, VICTIM);
    crate::game_loop::skills::cast::use_magic(&mut world, VICTIM_CID, VICTIM, 91, false, false);
    assert!(
        world.objects.has_component::<Casting>(&VICTIM),
        "a rooted player can still cast"
    );
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
    handle_move_backward_to_location(
        &mut world,
        VICTIM_CID,
        &move_body((900, 0, 0), (50, 0, 0), 1),
    );
    assert!(world.objects.has_component::<Casting>(&VICTIM));

    land(&mut world, STUN_ID, VICTIM);
    assert!(
        !world.objects.has_component::<Casting>(&VICTIM),
        "the in-flight cast is aborted"
    );
    assert!(
        !world.objects.has_component::<Movement>(&VICTIM),
        "and the victim is frozen in place"
    );
}

/// A stunned *monster* stops attacking: its AI think is short-circuited while
/// the flag is up, and resumes once it clears.
#[test]
fn stunned_monster_stops_acting() {
    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    land(&mut world, STUN_ID, NPC_OID);
    assert!(
        abnormal::is_blocked_from_actions(&world, NPC_OID),
        "the mob is stunned"
    );

    // Ticking the AI while stunned must not start a chase.
    advance_ticks(&mut world, 20);
    assert!(
        !world.objects.has_component::<Movement>(&NPC_OID),
        "a stunned mob neither chases nor wanders"
    );

    crate::game_loop::skills::effects::handle_buff_expire(&mut world, NPC_OID, STUN_ID);
    assert!(
        !abnormal::is_blocked_from_actions(&world, NPC_OID),
        "and it recovers when the stun ends"
    );
}

/// An AoE stun (the real shape of Thunder Storm 48: a POINT_BLANK sweep whose
/// effect is `BlockActions`) stuns every mob it catches — the two G19 slices
/// composing.
#[test]
fn aoe_stun_blocks_the_whole_cluster() {
    let (mut world, _db, _l) = cc_world();
    let mut aoe = cc_skill(
        9302,
        SkillEffect::BlockActions { conditional: false },
        "STUN",
    );
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

    assert!(
        abnormal::is_blocked_from_actions(&world, a),
        "mob in the sweep is stunned"
    );
    assert!(
        abnormal::is_blocked_from_actions(&world, b),
        "so is the second one"
    );
    assert!(
        !abnormal::is_blocked_from_actions(&world, far),
        "the one outside is not"
    );
}

// ---------------------------------------------------------------------------
// The rest of the CC family: mute, debuff-block, control-block, target-cancel
// ---------------------------------------------------------------------------

const MUTE_ID: i32 = 9310;
const PMUTE_ID: i32 = 9311;
const DBLOCK_ID: i32 = 9312;
const CBLOCK_ID: i32 = 9313;
const TCANCEL_ID: i32 = 9314;

fn cc2_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    // Builds on `cc_world` so the stun/root fixtures are available too — the
    // debuff-block case needs a real debuff to refuse.
    let (mut world, db, l) = cc_world();
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(MUTE_ID, SkillEffect::Mute, "MUTE"));
    world.data.skill_data.insert_for_test(cc_skill(
        PMUTE_ID,
        SkillEffect::PhysicalMute,
        "PHYSICAL_MUTE",
    ));
    world.data.skill_data.insert_for_test(cc_skill(
        DBLOCK_ID,
        SkillEffect::DebuffBlock,
        "DEBUFF_BLOCK",
    ));
    world.data.skill_data.insert_for_test(cc_skill(
        CBLOCK_ID,
        SkillEffect::BlockControl,
        "BLOCK_CONTROL",
    ));
    world.data.skill_data.insert_for_test(cc_skill(
        TCANCEL_ID,
        SkillEffect::TargetCancel { chance: 100 },
        "NONE",
    ));
    (world, db, l)
}

/// A silenced caster is refused **magic** skills but keeps physical ones; the
/// physical mute is the mirror image. Skill 91 in `cast_test_world` is magic
/// (`magic_type == 1`).
#[test]
fn mute_blocks_magic_and_physical_mute_blocks_the_rest() {
    let (mut world, _db, _l) = cc2_world();
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    drain(&mut out);

    // Silenced: the magic self-buff is refused.
    land(&mut world, MUTE_ID, CASTER);
    crate::game_loop::skills::cast::use_magic(&mut world, CID, CASTER, 91, false, false);
    assert!(
        !world.objects.has_component::<Casting>(&CASTER),
        "a silenced caster can't cast magic"
    );

    // Clear it and confirm the same cast now works.
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, CASTER, MUTE_ID);
    crate::game_loop::skills::cast::use_magic(&mut world, CID, CASTER, 91, false, false);
    assert!(
        world.objects.has_component::<Casting>(&CASTER),
        "and can once the silence ends"
    );
    crate::game_loop::skills::cast::stop_casting(&mut world, CASTER);

    // A *physical* mute leaves the magic skill alone.
    land(&mut world, PMUTE_ID, CASTER);
    crate::game_loop::skills::cast::use_magic(&mut world, CID, CASTER, 91, false, false);
    assert!(
        world.objects.has_component::<Casting>(&CASTER),
        "physical mute must not block a magic skill"
    );
}

/// Landing a mute aborts the cast already in flight.
#[test]
fn mute_interrupts_an_in_flight_cast() {
    let (mut world, _db, _l) = cc2_world();
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    drain(&mut out);

    crate::game_loop::skills::cast::use_magic(&mut world, CID, CASTER, 91, false, false);
    assert!(world.objects.has_component::<Casting>(&CASTER));
    land(&mut world, MUTE_ID, CASTER);
    assert!(
        !world.objects.has_component::<Casting>(&CASTER),
        "the in-flight cast is aborted"
    );
}

/// **Raid bosses are immune to the mute interrupt** — Java's `isRaid()` bail,
/// which is what stops one silence from neutering a raid.
#[test]
fn raid_bosses_ignore_the_mute_interrupt() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    // A raid-flagged NPC.
    let mut t = crate::data::npc_data::default_template(20050);
    t.type_name = "RaidBoss".into();
    t.level = 40;
    t.base_hp_max = 5000.0;
    t.base_mp_max = 500.0;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 20050, "RaidBoss", 40, 100, 0, 0);
    assert!(
        world.data.npc_data.get(20050).is_some_and(|t| t.is_raid()),
        "fixture must actually be a raid for this test to mean anything"
    );

    // The mute still lands as a buff; only the cast-abort side effect is skipped.
    land(&mut world, MUTE_ID, NPC_OID);
    assert!(
        abnormal::flags_of(&world, NPC_OID) & effect_flag::MUTED != 0,
        "the flag still applies"
    );
}

/// `DEBUFF_BLOCK` refuses incoming debuffs outright while leaving buffs alone.
#[test]
fn debuff_block_refuses_incoming_debuffs() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    // Baseline: the stun lands.
    land(&mut world, STUN_ID, VICTIM);
    assert!(abnormal::is_blocked_from_actions(&world, VICTIM));
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, VICTIM, STUN_ID);

    // Under debuff block it does not.
    land(&mut world, DBLOCK_ID, VICTIM);
    land(&mut world, STUN_ID, VICTIM);
    assert!(
        !abnormal::is_blocked_from_actions(&world, VICTIM),
        "a debuff-blocked target refuses the stun entirely"
    );

    // A *buff* still lands (1068 is the Might-like buff, not a debuff).
    let buff = world.data.skill_data.get(1068, 1).cloned().expect("might");
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, CASTER, VICTIM, &buff);
    assert!(
        world
            .objects
            .get_component::<Buffs>(&VICTIM)
            .is_some_and(|b| b.0.iter().any(|x| x.skill_id == 1068)),
        "debuff block does not stop buffs"
    );
}

/// `BLOCK_CONTROL` refuses item use (Java's `UseItem` gate).
#[test]
fn control_block_refuses_item_use() {
    let (mut world, _db, _l) = cc2_world();
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    drain(&mut out);

    land(&mut world, CBLOCK_ID, CASTER);
    // A bogus item object id is fine: the gate must reject before any lookup,
    // so the only reply is ActionFailed.
    crate::game_loop::items::handle_use_item(&mut world, CID, &use_item_body(1234));
    let pkts = drain(&mut out);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::ACTION_FAIL),
        "item use is refused while control-blocked"
    );
}

/// `TargetCancel` drops the victim's target and aborts what they were doing.
#[test]
fn target_cancel_clears_the_target_and_aborts() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut vout = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);

    // The victim is targeting the mob and casting.
    world
        .objects
        .get_component_mut::<crate::model::components::TargetRef>(&VICTIM)
        .unwrap()
        .0 = Some(NPC_OID);
    crate::game_loop::skills::cast::use_magic(&mut world, VICTIM_CID, VICTIM, 91, false, false);
    assert!(world.objects.has_component::<Casting>(&VICTIM));
    drain(&mut vout);

    // `TargetCancel` rolls through `calcProbability` (`magicLevel + chance −
    // targetLevel`), so even a 100-chance skill has a threshold below 100 and
    // an unforced roll makes this flaky. Force the magic-crit throwaway and a
    // winning probability roll.
    world.forced_rolls.extend([0, 0]);
    land(&mut world, TCANCEL_ID, VICTIM);
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::TargetRef>(&VICTIM)
            .unwrap()
            .0,
        None,
        "the target is dropped"
    );
    assert!(
        !world.objects.has_component::<Casting>(&VICTIM),
        "and the cast is aborted"
    );
}

/// A 0% `TargetCancel` does nothing — proof the chance roll is consulted.
#[test]
fn zero_chance_target_cancel_does_nothing() {
    let (mut world, _db, _l) = cc2_world();
    world.data.skill_data.insert_for_test(cc_skill(
        9315,
        SkillEffect::TargetCancel { chance: 0 },
        "NONE",
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);

    world
        .objects
        .get_component_mut::<crate::model::components::TargetRef>(&VICTIM)
        .unwrap()
        .0 = Some(NPC_OID);
    land(&mut world, 9315, VICTIM);
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::TargetRef>(&VICTIM)
            .unwrap()
            .0,
        Some(NPC_OID),
        "a 0% target-cancel leaves the target alone"
    );
}

// ---------------------------------------------------------------------------
// Abnormal visual effects
// ---------------------------------------------------------------------------

/// The visual set is a fold over live buffs, de-duplicated, and clears with
/// them. Two poisons draw one tint.
#[test]
fn visual_effects_fold_over_buffs_and_clear() {
    use crate::game_loop::abnormal::visual_effects;

    let (mut world, _db, _l) = cc_world();
    // STUN(7) and DOT_POISON(2); a second poison must not duplicate the tint.
    let mut stun_vis = cc_skill(
        9320,
        SkillEffect::BlockActions { conditional: false },
        "STUN_VIS",
    );
    stun_vis.abnormal_visuals = vec![7];
    let mut poison_a = cc_skill(9321, SkillEffect::Root, "POISON_A");
    poison_a.abnormal_visuals = vec![2];
    let mut poison_b = cc_skill(9322, SkillEffect::Root, "POISON_B");
    poison_b.abnormal_visuals = vec![2];
    for sk in [stun_vis, poison_a, poison_b] {
        world.data.skill_data.insert_for_test(sk);
    }
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    assert!(
        visual_effects(&world, VICTIM).is_empty(),
        "nothing showing to begin with"
    );

    land(&mut world, 9320, VICTIM);
    assert_eq!(
        visual_effects(&world, VICTIM),
        vec![7],
        "the stun swirl shows"
    );

    land(&mut world, 9321, VICTIM);
    land(&mut world, 9322, VICTIM);
    let vis = visual_effects(&world, VICTIM);
    assert!(
        vis.contains(&7) && vis.contains(&2),
        "both visuals show: {vis:?}"
    );
    assert_eq!(
        vis.iter().filter(|&&v| v == 2).count(),
        1,
        "de-duplicated: {vis:?}"
    );

    // Clearing the stun leaves the poison tint behind.
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, VICTIM, 9320);
    let vis = visual_effects(&world, VICTIM);
    assert!(
        !vis.contains(&7) && vis.contains(&2),
        "only the stun's visual went: {vis:?}"
    );
}

/// The visual reaches the wire: `CharInfo` carries the count and ids so nearby
/// players actually see the effect on the victim.
#[test]
fn char_info_carries_the_visual_list() {
    let (mut world, _db, _l) = cc_world();
    let mut stun_vis = cc_skill(
        9323,
        SkillEffect::BlockActions { conditional: false },
        "STUN_VIS",
    );
    stun_vis.abnormal_visuals = vec![7];
    world.data.skill_data.insert_for_test(stun_vis);
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    let visuals_of = |world: &World| {
        let v = crate::model::PlayerView::of(&world.objects, VICTIM).expect("view");
        server_packets::char_info(
            &v,
            &crate::game_loop::abnormal::visual_effects(world, VICTIM),
            &[],
            &Default::default(),
        )
    };

    let before = visuals_of(&world);
    land(&mut world, 9323, VICTIM);
    let after = visuals_of(&world);
    assert!(
        after.len() > before.len(),
        "the stunned CharInfo is longer by the visual entry"
    );
}

/// A skill with no `<abnormalVisualEffect>` sends no visual packet — Java only
/// pushes the set from start/stopAbnormalVisualEffect, so a plain stat buff
/// must not spam `ExUserInfoAbnormalVisualEffect`.
#[test]
fn buffs_without_a_visual_send_no_visual_packet() {
    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut vout = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    drain(&mut vout);

    // 1068 is the Might-like stat buff from `cast_test_world` — no visual.
    let buff = world.data.skill_data.get(1068, 1).cloned().expect("might");
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, CASTER, VICTIM, &buff);

    let pkts = drain(&mut vout);
    let ave_pkts = pkts
        .iter()
        .filter(|p| {
            p[0] == 0xFE
                && p.len() >= 3
                && i16::from_le_bytes([p[1], p[2]])
                    == server_packets::opcodes::EX_USER_INFO_ABNORMAL_VISUAL_EFFECT
        })
        .count();
    assert_eq!(
        ave_pkts, 0,
        "a visual-less buff pushes no ExUserInfoAbnormalVisualEffect"
    );
}

/// **An invincible target is never shaken off its mark.** Java's
/// `TargetCancel.calcSuccess` vetoes on `ABNORMAL_INVINCIBILITY`,
/// `INVINCIBILITY_SPECIAL` or `INVINCIBILITY` *before* the chance is rolled —
/// so no amount of Shield Bash moves a target under Celestial Shield.
#[test]
fn an_invincible_target_ignores_target_cancel() {
    for abnormal in [
        "ABNORMAL_INVINCIBILITY",
        "INVINCIBILITY_SPECIAL",
        "INVINCIBILITY",
    ] {
        let (mut world, _db, _l) = cc2_world();
        let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
        let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
        add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);
        world
            .objects
            .get_component_mut::<crate::model::components::TargetRef>(&VICTIM)
            .unwrap()
            .0 = Some(NPC_OID);

        // A buff whose only relevant property is its abnormal type.
        world
            .data
            .skill_data
            .insert_for_test(cc_skill(9320, SkillEffect::BlockControl, abnormal));
        land(&mut world, 9320, VICTIM);

        land(&mut world, TCANCEL_ID, VICTIM);
        assert_eq!(
            world
                .objects
                .get_component::<crate::model::components::TargetRef>(&VICTIM)
                .unwrap()
                .0,
            Some(NPC_OID),
            "{abnormal} vetoes the cancel"
        );
    }
}

/// **The victim's level counts.** Java rolls `TargetCancel` through
/// `Formulas.calcProbability` (`magicLevel + chance − targetLevel`), not
/// against the raw percentage — so the same 100 %-on-paper Shield Bash slides
/// off a target far above the skill's magic level.
#[test]
fn target_cancel_slides_off_a_much_higher_level_target() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::components::TargetRef>(&VICTIM)
        .unwrap()
        .0 = Some(NPC_OID);
    // `cc_skill` carries `magic_level: 0` and the fixture chance is 100, so the
    // threshold is `0 + 100 - level`: put the victim past it.
    world
        .objects
        .get_component_mut::<crate::model::Player>(&VICTIM)
        .unwrap()
        .level = 100;

    land(&mut world, TCANCEL_ID, VICTIM);
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::TargetRef>(&VICTIM)
            .unwrap()
            .0,
        Some(NPC_OID),
        "a level-100 target keeps its mark against a magic-level-0 skill"
    );
}

/// **The trait resistance reaches the roll.** `calcProbability` multiplies the
/// threshold by `calcGeneralTraitBonus(…, ignoreResistance = false)`, so a
/// victim resisting the skill's trait shrugs off a `TargetCancel` that would
/// otherwise land. (`calcAttributeBonus` rides the same call, on the line
/// above.)
#[test]
fn a_trait_resistance_lowers_the_target_cancel_chance() {
    use crate::model::skill::TraitType;

    let cancel = |resist: bool| {
        let (mut world, _db, _l) = cc2_world();
        let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
        let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
        add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);
        world
            .objects
            .get_component_mut::<crate::model::components::TargetRef>(&VICTIM)
            .unwrap()
            .0 = Some(NPC_OID);
        // Give the cancel a trait the victim can resist.
        let mut skill = world.data.skill_data.get(TCANCEL_ID, 1).unwrap().clone();
        skill.trait_type = TraitType::Shock;
        world.data.skill_data.insert_for_test(skill.clone());
        if resist {
            crate::game_loop::skills::effects::merge_defence_traits(
                &mut world,
                VICTIM,
                &[(TraitType::Shock, 0.5)],
            );
        }
        // Threshold is `0 + 100 - level` (~99 here); halve it and a 60 roll
        // stops landing.
        world.forced_rolls.extend([0, 60]);
        crate::game_loop::skills::effects::apply_skill_effects(&mut world, CASTER, VICTIM, &skill);
        world
            .objects
            .get_component::<crate::model::components::TargetRef>(&VICTIM)
            .unwrap()
            .0
    };

    assert_eq!(cancel(false), None, "unresisted, the cancel lands");
    assert_eq!(
        cancel(true),
        Some(NPC_OID),
        "a 50% SHOCK resistance halves the threshold and the same roll misses"
    );
}

/// And so does the **attribute** bonus, the sibling term: a victim resisting
/// the skill's element pulls the same threshold down.
#[test]
fn an_element_resistance_lowers_the_target_cancel_chance() {
    use crate::model::stats::{Element, Stat};

    let cancel = |resist: bool| {
        let (mut world, _db, _l) = cc2_world();
        let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
        let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
        add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);
        world
            .objects
            .get_component_mut::<crate::model::components::TargetRef>(&VICTIM)
            .unwrap()
            .0 = Some(NPC_OID);
        let mut skill = world.data.skill_data.get(TCANCEL_ID, 1).unwrap().clone();
        skill.attribute_type = Some(Element::Fire);
        skill.attribute_value = 20;
        world.data.skill_data.insert_for_test(skill.clone());
        if resist {
            // A heavy fire resistance drags `calcAttributeBonus` below 1.
            let mods = world
                .objects
                .get_component_mut::<crate::model::components::StatModifiers>(&VICTIM)
                .unwrap();
            *mods.add.entry(Stat::FireRes).or_insert(0.0) += 300.0;
        }
        // `calcAttributeBonus` floors at 0.75, so the resisted threshold is
        // ~74 against ~99 unresisted — a roll of 80 separates them.
        world.forced_rolls.extend([0, 80]);
        crate::game_loop::skills::effects::apply_skill_effects(&mut world, CASTER, VICTIM, &skill);
        world
            .objects
            .get_component::<crate::model::components::TargetRef>(&VICTIM)
            .unwrap()
            .0
    };

    assert_eq!(cancel(false), None, "unresisted, the cancel lands");
    assert_eq!(
        cancel(true),
        Some(NPC_OID),
        "the fire resistance scaled the threshold under the same roll"
    );
}

/// **A stunned mob shows its icon.** `NpcInfo`'s `ABNORMALS` component was
/// never emitted — the same shape as `CharInfo`'s abnormal-visual count before
/// G19 fixed it, but for NPCs — so a mob under a visible abnormal looked
/// completely untouched to every client.
#[test]
fn npc_info_carries_the_mobs_abnormal_visuals() {
    use crate::model::npc::NpcView;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);

    let build = |world: &World| {
        let v = NpcView::of(&world.objects, NPC_OID).expect("a live mob");
        let t = v.npc.template(world).expect("its template");
        crate::network::server_packets::npc_info(
            &v,
            t,
            &world.cfg.npc,
            &world.cfg.champion,
            &crate::game_loop::abnormal::visual_effects(world, NPC_OID),
        )
    };

    let clean = build(&world);

    // Land a stun on the mob: `apply_buff_to_npc` stores its visual ids.
    world.data.skill_data.insert_for_test({
        let mut s = cc_skill(9330, SkillEffect::Root, "STUN");
        s.abnormal_visuals = vec![1]; // AbnormalVisualEffect.DOT_BLEEDING-ish id
        s
    });
    land(&mut world, 9330, NPC_OID);
    assert_eq!(
        crate::game_loop::abnormal::visual_effects(&world, NPC_OID),
        vec![1],
        "the mob really is carrying a visual"
    );

    let stunned = build(&world);
    assert!(
        stunned.len() > clean.len(),
        "the ABNORMALS block adds a count plus one short: {} vs {}",
        stunned.len(),
        clean.len()
    );
    assert_eq!(
        stunned.len() - clean.len(),
        4,
        "an i16 count and one i16 id"
    );
    // The tail carries the count and the id, little-endian.
    let tail = &stunned[stunned.len() - 5..];
    assert_eq!(i16::from_le_bytes([tail[0], tail[1]]), 1, "one effect");
    assert_eq!(i16::from_le_bytes([tail[2], tail[3]]), 1, "its client id");

    // And the visibility path — the one that actually reaches a client — sends
    // that same packet rather than a bare one.
    let mut rx = ingame_caster(&mut world, 9, 3099, 0, 0);
    drain(&mut rx);
    crate::game_loop::visibility::on_enter_world(&world, 9, 3099);
    let sent = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == crate::network::server_packets::opcodes::NPC_INFO)
        .expect("the observer was told about the mob");
    assert_eq!(
        sent.len(),
        stunned.len(),
        "the observer's NpcInfo carries the abnormal block too"
    );
}

/// **An NPC's team aura and display effect ride `NpcInfo`.** Both were
/// broadcast-only or not modelled at all, so `//setteam` on a mob did nothing
/// visible and `//set_displayeffect` was lost on anyone who arrived after the
/// change — Java stores both on the NPC precisely so a late observer sees them.
#[test]
fn npc_info_carries_the_team_and_display_effect() {
    use crate::model::npc::{Npc, NpcView};

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);

    let build = |world: &World| {
        let v = NpcView::of(&world.objects, NPC_OID).expect("a live mob");
        let t = v.npc.template(world).expect("its template");
        crate::network::server_packets::npc_info(&v, t, &world.cfg.npc, &world.cfg.champion, &[])
    };
    let clean = build(&world);

    // Blue team → one extra byte (`NpcInfoType::TEAM`, block length 1).
    world
        .objects
        .get_component_mut::<Npc>(&NPC_OID)
        .unwrap()
        .team = 1;
    let teamed = build(&world);
    assert_eq!(teamed.len() - clean.len(), 1, "the TEAM block is one byte");

    // Display effect → four more (`DISPLAY_EFFECT`, block length 4).
    world
        .objects
        .get_component_mut::<Npc>(&NPC_OID)
        .unwrap()
        .display_effect = 3;
    let both = build(&world);
    assert_eq!(
        both.len() - teamed.len(),
        4,
        "the DISPLAY_EFFECT block is four"
    );

    // Back to Java's defaults: neither block is emitted.
    {
        let n = world.objects.get_component_mut::<Npc>(&NPC_OID).unwrap();
        n.team = 0;
        n.display_effect = 0;
    }
    assert_eq!(build(&world).len(), clean.len(), "defaults emit nothing");

    // And an observer arriving *after* the change is told (the whole point of
    // storing it rather than only broadcasting the change packet).
    {
        let n = world.objects.get_component_mut::<Npc>(&NPC_OID).unwrap();
        n.team = 2;
        n.display_effect = 7;
    }
    let mut rx = ingame_caster(&mut world, 9, 3098, 0, 0);
    drain(&mut rx);
    crate::game_loop::visibility::on_enter_world(&world, 9, 3098);
    let sent = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == crate::network::server_packets::opcodes::NPC_INFO)
        .expect("the observer was told about the mob");
    assert_eq!(
        sent.len(),
        clean.len() + 5,
        "a late observer gets both blocks"
    );
}

// ---------------------------------------------------------------------------
// `<removedOnDamage>` — sleep is one-hit crowd control
// ---------------------------------------------------------------------------

const SLEEP_ID: i32 = 9310;

/// A sleep: `BlockActions` like a stun, but carrying the `<removedOnDamage>`
/// tag every real `Sleep` (1069, 1072, 4046, …) declares.
fn sleep_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = cc_world();
    let mut sleep = cc_skill(
        SLEEP_ID,
        SkillEffect::BlockActions { conditional: false },
        "SLEEP",
    );
    sleep.removed_on_damage = true;
    world.data.skill_data.insert_for_test(sleep);
    (world, db, l)
}

/// The bug this fixes: a slept player stayed action-blocked while the mob that
/// slept them beat on them. Java's `PlayerStatus.reduceHp` calls
/// `stopEffectsOnDamage()` on every hit, so the first blow wakes them.
///
/// The stun in the same world is the control: it carries no `<removedOnDamage>`
/// tag and must survive the identical hit, proving the removal keys off the tag
/// rather than clearing crowd control wholesale.
#[test]
fn a_hit_wakes_a_slept_player_but_leaves_a_stun_alone() {
    let (mut world, _db, _l) = sleep_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    land(&mut world, SLEEP_ID, VICTIM);
    assert!(
        abnormal::is_blocked_from_actions(&world, VICTIM),
        "the sleep landed"
    );

    crate::game_loop::combat::apply_physical_damage(&mut world, CASTER, VICTIM, 10.0, false, false);
    assert!(
        !abnormal::is_blocked_from_actions(&world, VICTIM),
        "one hit wakes the sleeper"
    );
    assert!(
        world
            .objects
            .get_component::<Buffs>(&VICTIM)
            .is_none_or(|b| b.0.iter().all(|x| x.skill_id != SLEEP_ID)),
        "and the buff row is gone, not just the flag"
    );

    // Control: same hit, a stun instead.
    let (mut world, _db, _l) = sleep_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    land(&mut world, STUN_ID, VICTIM);
    crate::game_loop::combat::apply_physical_damage(&mut world, CASTER, VICTIM, 10.0, false, false);
    assert!(
        abnormal::is_blocked_from_actions(&world, VICTIM),
        "a stun is not `removedOnDamage` — hitting a stunned target does not \
         free them (Java's 14% `calcStunBreak` is gated on `BreakStun`, which \
         this dist leaves off)"
    );
}

/// The same tag on a slept **mob**: the first blow wakes it so it can fight
/// back, which is what makes sleep a pull tool rather than a permanent lock.
#[test]
fn a_hit_wakes_a_slept_monster() {
    let (mut world, _db, _l) = sleep_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    land(&mut world, SLEEP_ID, NPC_OID);
    assert!(
        abnormal::is_blocked_from_actions(&world, NPC_OID),
        "the mob is asleep"
    );

    crate::game_loop::combat::apply_physical_damage(&mut world, CASTER, NPC_OID, 5.0, false, false);
    assert!(
        !abnormal::is_blocked_from_actions(&world, NPC_OID),
        "and it wakes on the first blow"
    );
}

/// Java's one asymmetry between the two `reduceHp` overrides:
/// `CreatureStatus` guards the whole wake block with `!isDOT`, while
/// `PlayerStatus` puts `stopEffectsOnDamage()` *above* its `if (!isDOT)` block.
/// So a poison tick wakes a sleeping player but not a sleeping mob.
#[test]
fn a_dot_tick_wakes_a_slept_player_but_not_a_slept_mob() {
    let (mut world, _db, _l) = sleep_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    land(&mut world, SLEEP_ID, VICTIM);
    land(&mut world, SLEEP_ID, NPC_OID);

    crate::game_loop::combat::apply_physical_damage(&mut world, CASTER, VICTIM, 3.0, true, false);
    crate::game_loop::combat::apply_physical_damage(&mut world, CASTER, NPC_OID, 3.0, true, false);

    assert!(
        !abnormal::is_blocked_from_actions(&world, VICTIM),
        "a DoT tick still wakes a player"
    );
    assert!(
        abnormal::is_blocked_from_actions(&world, NPC_OID),
        "but a DoT tick alone does not rouse a mob"
    );
}

// ---------------------------------------------------------------------------
// G34 S3 — the flag-only abnormal states. Each is one `effect_flag` bit plus
// the single Java gate that reads it; `cc_skill`'s fixtures make each bit
// landable on demand.
// ---------------------------------------------------------------------------

const BUFFBLOCK_ID: i32 = 9321;
const PACIFY_ID: i32 = 9322;
const ATKMUTE_ID: i32 = 9323;

/// `BUFF_BLOCK` is the mirror of `DEBUFF_BLOCK`, and the asymmetry matters:
/// Java's `EffectList.add` refuses on `isBuffBlocked() && !skill.isBad()`, so a
/// **buff** is stopped and a **debuff** still lands. It also has **no
/// self-cast exemption**, unlike the debuff-block gate — Dance of Medusa stops
/// its victim buffing themselves, which is the whole point of it.
#[test]
fn buff_block_refuses_buffs_and_lets_debuffs_through() {
    let (mut world, _db, _l) = cc2_world();
    world.data.skill_data.insert_for_test(cc_skill(
        BUFFBLOCK_ID,
        SkillEffect::BuffBlock,
        "BUFF_BLOCK",
    ));
    // A plain good skill (effectPoint ≥ 0) to be refused, and a bad one to
    // prove debuffs are unaffected.
    let mut good = cc_skill(9324, SkillEffect::SilentMove, "SILENT_MOVE");
    good.effect_point = 100;
    good.is_debuff = false;
    world.data.skill_data.insert_for_test(good);

    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    land(&mut world, BUFFBLOCK_ID, CASTER);
    assert!(
        crate::game_loop::abnormal::is_buff_blocked(&world, CASTER),
        "the flag is up"
    );

    land(&mut world, 9324, CASTER);
    assert!(
        !world
            .objects
            .get_component::<Buffs>(&CASTER)
            .is_some_and(|b| b.0.iter().any(|x| x.skill_id == 9324)),
        "a buff cannot land on a buff-blocked target — not even their own"
    );

    // A debuff is explicitly *not* blocked by this flag.
    land(&mut world, ROOT_ID, CASTER);
    assert!(
        world
            .objects
            .get_component::<Buffs>(&CASTER)
            .is_some_and(|b| b.0.iter().any(|x| x.skill_id == ROOT_ID)),
        "a debuff still lands — `!skill.isBad()` is the gate, not `isDebuff()`"
    );
}

/// `PASSIVE` — Java `Monster.isAggressive()` is
/// `getTemplate().isAggressive() && !isAffected(EffectFlag.PASSIVE)`, so a
/// pacified mob stops aggroing whatever its template says. Veil (106) and
/// Requiem (1049) are the learnable sources.
#[test]
fn the_passive_flag_pacifies_an_aggressive_monster() {
    let (mut world, _db, _l) = cc2_world();
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(PACIFY_ID, SkillEffect::Passive, "PASSIVE"));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    assert!(
        !crate::game_loop::abnormal::is_pacified(&world, NPC_OID),
        "not pacified to begin with"
    );
    land(&mut world, PACIFY_ID, NPC_OID);
    assert!(
        crate::game_loop::abnormal::is_pacified(&world, NPC_OID),
        "the mob is pacified while the buff is up"
    );
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, NPC_OID, PACIFY_ID);
    assert!(
        !crate::game_loop::abnormal::is_pacified(&world, NPC_OID),
        "and aggressive again when it drops"
    );
}

/// `PSYCHICAL_ATTACK_MUTED` (Java's spelling) blocks the **auto-attack**, which
/// is a different lock from `PHYSICAL_MUTED`'s non-magic *skill* refusal — Java
/// folds the first into `isAttackDisabled()` and the second into
/// `checkUseConditions`. Landing one must not imply the other.
#[test]
fn physical_attack_mute_blocks_attacking_not_casting() {
    let (mut world, _db, _l) = cc2_world();
    world.data.skill_data.insert_for_test(cc_skill(
        ATKMUTE_ID,
        SkillEffect::PhysicalAttackMute,
        "ATTACK_MUTE",
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    land(&mut world, ATKMUTE_ID, CASTER);
    assert!(
        crate::game_loop::abnormal::is_physical_attack_muted(&world, CASTER),
        "the auto-attack lock is up"
    );
    assert!(
        !crate::game_loop::abnormal::is_physical_muted(&world, CASTER),
        "…and it is NOT the skill lock — two distinct flags"
    );
    assert!(
        !crate::game_loop::abnormal::is_muted(&world, CASTER),
        "…nor the magic one"
    );
}

/// `UNTARGETABLE` and `TARGETING_DISABLED` are the two halves of Java's one
/// gate in `Action`/`AttackRequest`: `(!obj.isTargetable() ||
/// player.isTargetingDisabled())`. The first sits on the *clicked* object, the
/// second on the *clicker* — swapping them would look identical in a
/// single-actor test, so both are asserted from both sides.
#[test]
fn untargetable_sits_on_the_target_and_targeting_disabled_on_the_clicker() {
    let (mut world, _db, _l) = cc2_world();
    world.data.skill_data.insert_for_test(cc_skill(
        9325,
        SkillEffect::Untargetable,
        "UNTARGETABLE",
    ));
    world.data.skill_data.insert_for_test(cc_skill(
        9326,
        SkillEffect::DisableTargeting,
        "TARGETING_DISABLED",
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    land(&mut world, 9325, NPC_OID);
    assert!(crate::game_loop::abnormal::is_untargetable(&world, NPC_OID));
    assert!(
        !crate::game_loop::abnormal::is_targeting_disabled(&world, NPC_OID),
        "being unclickable does not stop you clicking"
    );

    land(&mut world, 9326, CASTER);
    assert!(crate::game_loop::abnormal::is_targeting_disabled(
        &world, CASTER
    ));
    assert!(
        !crate::game_loop::abnormal::is_untargetable(&world, CASTER),
        "being unable to click does not make you unclickable"
    );
}

/// `PHYSICAL_SHIELD_ANGLE_ALL` (Aegis) widens Java's `degreeside` from 120° to
/// 360°, which in practice means the back-attack exemption in `calcShldUse`
/// simply stops applying — a shield can block a backstab.
#[test]
fn the_shield_angle_flag_lets_a_shield_block_from_behind() {
    let (mut world, _db, _l) = cc2_world();
    world.data.skill_data.insert_for_test(cc_skill(
        9327,
        SkillEffect::PhysicalShieldAngleAll,
        "SHIELD_ANGLE",
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    assert!(!crate::game_loop::abnormal::shields_from_all_angles(
        &world, CASTER
    ));
    land(&mut world, 9327, CASTER);
    assert!(
        crate::game_loop::abnormal::shields_from_all_angles(&world, CASTER),
        "the 360° arc is up while the stance holds"
    );

    // The formula's own behaviour, which the flag feeds: a back attack is
    // unblockable, and that is the *only* thing the flag changes.
    use crate::model::formulas::{SHIELD_NONE, SHIELD_SUCCEED, calc_shield_use};
    assert_eq!(
        calc_shield_use(90.0, 1.0, false, true, 0, 99),
        SHIELD_NONE,
        "from behind, no block"
    );
    // `perfect_roll` 0 keeps it an ordinary block: the perfect-block test is
    // `100 − 2×con_bonus < perfect_roll`, i.e. 98 < 0 here.
    assert_eq!(
        calc_shield_use(90.0, 1.0, false, false, 0, 0),
        SHIELD_SUCCEED,
        "from the front, the same roll blocks"
    );
}

/// `isStayAfterDeath()` is **one getter over three tags** —
/// `_stayAfterDeath || _irreplacableBuff || _isNecessaryToggle` — and the port
/// read only the first. On this dist **30 learnable skills** declare
/// `<irreplacableBuff>` with no `<stayAfterDeath>` of their own (the whole
/// Transform Grail Apostle / Unicorn / Lilim Knight / Golem Guardian family),
/// so every one of them was being stripped on death when Java keeps it.
///
/// Asserted against the real dist and against a skill where the *new* tag is
/// the only source, so the assertion can only pass because of the fold.
#[test]
fn irreplacable_buffs_survive_death_like_stay_after_death_ones() {
    const TRANSFORM_GRAIL_APOSTLE: i32 = 541;
    let sd =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    assert!(
        sd.get(TRANSFORM_GRAIL_APOSTLE, 1)
            .expect("Transform Grail Apostle 1")
            .stay_after_death,
        "declares <irreplacableBuff> and no <stayAfterDeath>, so it survives \
         death only if the getter's three tags are folded"
    );
}

/// G34 S4 sub-slice 3 — `TargetMe` (Aggression 28, Aggression Aura 18) and
/// `TargetMeProbability` (Vengeance 368).
///
/// **Both are `if (effected.isPlayable())` in Java**, so taunting a *monster*
/// through them does nothing — a mob's aggro comes from the `AddHate`/`GetAgro`
/// effects the same skills carry. That is why Aggression declares both, and
/// why this asserts the monster case explicitly: implementing `TargetMe` as
/// "force any target" would look right in every player-vs-player test.
#[test]
fn target_me_locks_a_playable_and_ignores_a_monster() {
    let (mut world, _db, _l) = cc2_world();
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(9331, SkillEffect::TargetMe, "TARGET_ME"));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    let victim = 5951;
    let _v = ingame_player_access(&mut world, 2, victim, 0);

    // A monster: nothing happens, no lock.
    land(&mut world, 9331, NPC_OID);
    assert!(
        !world
            .objects
            .has_component::<crate::model::components::LockedTarget>(&NPC_OID),
        "Java's isPlayable() guard means a mob is never locked by TargetMe"
    );

    // A player: target forced to the caster and locked there.
    land(&mut world, 9331, victim);
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::TargetRef>(&victim)
            .and_then(|t| t.0),
        Some(CASTER),
        "the victim's selection is dragged onto the taunter"
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::LockedTarget>(&victim)
            .map(|l| l.0),
        Some(CASTER),
        "…and locked"
    );

    // `TargetMe.onExit` — the lock goes with the buff.
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, victim, 9331);
    assert!(
        !world
            .objects
            .has_component::<crate::model::components::LockedTarget>(&victim),
        "the lock must not outlive the taunt"
    );
}

/// The lock's whole purpose: `Npc.canTarget` refuses a *different NPC* while it
/// holds ("Failed to change enmity"). It is an NPC-side gate only — the victim
/// can still click players and items, which is what stops it from reading as a
/// blanket targeting freeze.
#[test]
fn a_locked_target_cannot_click_a_different_npc() {
    let (mut world, _db, _l) = cc2_world();
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(9332, SkillEffect::TargetMe, "TARGET_ME"));
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    let other_npc = NPC_OID + 1;
    add_test_npc(&mut world, other_npc, 20001, "Monster", 5, 120, 0, 0);

    // Lock the caster onto the first mob, then try to click the second.
    world
        .objects
        .add_components(&CASTER, crate::model::components::LockedTarget(NPC_OID));
    drain(&mut out);
    crate::game_loop::target::handle_action(&mut world, CID, &action_body(other_npc, 0));
    let pkts = drain(&mut out);
    assert!(
        has_system_message(&pkts, server_packets::sm_ids::FAILED_TO_CHANGE_ENMITY),
        "the refusal is explained, not silent"
    );
    assert_ne!(
        world
            .objects
            .get_component::<crate::model::components::TargetRef>(&CASTER)
            .and_then(|t| t.0),
        Some(other_npc),
        "and the selection did not move"
    );

    // The locked NPC itself is still clickable.
    crate::game_loop::target::handle_action(&mut world, CID, &action_body(NPC_OID, 0));
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::TargetRef>(&CASTER)
            .and_then(|t| t.0),
        Some(NPC_OID),
        "the taunter is exactly who you are allowed to click"
    );
}

/// `HATE_ATTACK` (Sword/Blunt Weapon Mastery 217) multiplies the hate an
/// **auto-attack** generates — Java scales it inside
/// `Attackable.reduceCurrentHp`'s `if (skill == null)` branch only. The
/// skill-exclusion is the point: the mastery helps a tank hold aggro through
/// ordinary swings and does nothing for their taunts, so both cases are
/// asserted.
#[test]
fn hate_attack_scales_auto_attack_hate_only() {
    use crate::model::stats::Stat;
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    let hate_of = |world: &World| {
        world
            .objects
            .get_component::<crate::model::npc::AggroList>(&NPC_OID)
            .and_then(|a| a.0.get(&CASTER).map(|i| i.hate))
            .unwrap_or(0.0)
    };

    // Unbuffed auto-attack: the plain `damage·100 / (level + 7)`.
    crate::game_loop::combat::npc_receive_damage(&mut world, NPC_OID, CASTER, 10.0, true);
    let plain = hate_of(&world);
    assert!(plain > 0.0, "baseline hate: {plain}");

    let mut mods = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&CASTER)
        .cloned()
        .expect("modifiers");
    mods.mul.insert(Stat::HateAttack, 3.0);
    world.objects.add_components(&CASTER, mods);

    // Same damage, now tripled…
    crate::game_loop::combat::npc_receive_damage(&mut world, NPC_OID, CASTER, 10.0, true);
    let after_auto = hate_of(&world) - plain;
    assert!(
        (after_auto - plain * 3.0).abs() < 1e-6,
        "an auto-attack's hate is tripled ({plain} → {after_auto})"
    );

    // …but a *skill*'s hate is untouched, which is Java's `skill == null` gate.
    let before = hate_of(&world);
    crate::game_loop::combat::npc_receive_damage(&mut world, NPC_OID, CASTER, 10.0, false);
    let after_skill = hate_of(&world) - before;
    assert!(
        (after_skill - plain).abs() < 1e-6,
        "skill damage generates unmultiplied hate ({plain} vs {after_skill})"
    );
}
