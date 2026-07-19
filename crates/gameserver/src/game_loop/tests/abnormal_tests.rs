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
        without_action: false,
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

// ---------------------------------------------------------------------------
// The rest of the CC family: mute, debuff-block, control-block, target-cancel
// ---------------------------------------------------------------------------

const MUTE_ID: i32 = 9310;
const PMUTE_ID: i32 = 9311;
const DBLOCK_ID: i32 = 9312;
const CBLOCK_ID: i32 = 9313;
const TCANCEL_ID: i32 = 9314;

fn cc2_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    // Builds on `cc_world` so the stun/root fixtures are available too — the
    // debuff-block case needs a real debuff to refuse.
    let (mut world, db, l) = cc_world();
    world.data.skill_data.insert_for_test(cc_skill(MUTE_ID, SkillEffect::Mute, "MUTE"));
    world.data.skill_data.insert_for_test(cc_skill(PMUTE_ID, SkillEffect::PhysicalMute, "PHYSICAL_MUTE"));
    world.data.skill_data.insert_for_test(cc_skill(DBLOCK_ID, SkillEffect::DebuffBlock, "DEBUFF_BLOCK"));
    world.data.skill_data.insert_for_test(cc_skill(CBLOCK_ID, SkillEffect::BlockControl, "BLOCK_CONTROL"));
    world.data.skill_data.insert_for_test(cc_skill(TCANCEL_ID, SkillEffect::TargetCancel { chance: 100 }, "NONE"));
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
    assert!(!world.objects.has_component::<Casting>(&CASTER), "a silenced caster can't cast magic");

    // Clear it and confirm the same cast now works.
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, CASTER, MUTE_ID);
    crate::game_loop::skills::cast::use_magic(&mut world, CID, CASTER, 91, false, false);
    assert!(world.objects.has_component::<Casting>(&CASTER), "and can once the silence ends");
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
    assert!(!world.objects.has_component::<Casting>(&CASTER), "the in-flight cast is aborted");
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
    assert!(abnormal::flags_of(&world, NPC_OID) & effect_flag::MUTED != 0, "the flag still applies");
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
        world.objects.get_component::<Buffs>(&VICTIM).is_some_and(|b| b.0.iter().any(|x| x.skill_id == 1068)),
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
        pkts.iter().any(|p| p[0] == server_packets::opcodes::ACTION_FAIL),
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
    world.objects.get_component_mut::<crate::model::components::TargetRef>(&VICTIM).unwrap().0 = Some(NPC_OID);
    crate::game_loop::skills::cast::use_magic(&mut world, VICTIM_CID, VICTIM, 91, false, false);
    assert!(world.objects.has_component::<Casting>(&VICTIM));
    drain(&mut vout);

    land(&mut world, TCANCEL_ID, VICTIM);
    assert_eq!(
        world.objects.get_component::<crate::model::components::TargetRef>(&VICTIM).unwrap().0,
        None,
        "the target is dropped"
    );
    assert!(!world.objects.has_component::<Casting>(&VICTIM), "and the cast is aborted");
}

/// A 0% `TargetCancel` does nothing — proof the chance roll is consulted.
#[test]
fn zero_chance_target_cancel_does_nothing() {
    let (mut world, _db, _l) = cc2_world();
    world.data.skill_data.insert_for_test(cc_skill(9315, SkillEffect::TargetCancel { chance: 0 }, "NONE"));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);

    world.objects.get_component_mut::<crate::model::components::TargetRef>(&VICTIM).unwrap().0 = Some(NPC_OID);
    land(&mut world, 9315, VICTIM);
    assert_eq!(
        world.objects.get_component::<crate::model::components::TargetRef>(&VICTIM).unwrap().0,
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
    let mut stun_vis = cc_skill(9320, SkillEffect::BlockActions { conditional: false }, "STUN_VIS");
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

    assert!(visual_effects(&world, VICTIM).is_empty(), "nothing showing to begin with");

    land(&mut world, 9320, VICTIM);
    assert_eq!(visual_effects(&world, VICTIM), vec![7], "the stun swirl shows");

    land(&mut world, 9321, VICTIM);
    land(&mut world, 9322, VICTIM);
    let vis = visual_effects(&world, VICTIM);
    assert!(vis.contains(&7) && vis.contains(&2), "both visuals show: {vis:?}");
    assert_eq!(vis.iter().filter(|&&v| v == 2).count(), 1, "de-duplicated: {vis:?}");

    // Clearing the stun leaves the poison tint behind.
    crate::game_loop::skills::effects::handle_buff_expire(&mut world, VICTIM, 9320);
    let vis = visual_effects(&world, VICTIM);
    assert!(!vis.contains(&7) && vis.contains(&2), "only the stun's visual went: {vis:?}");
}

/// The visual reaches the wire: `CharInfo` carries the count and ids so nearby
/// players actually see the effect on the victim.
#[test]
fn char_info_carries_the_visual_list() {
    let (mut world, _db, _l) = cc_world();
    let mut stun_vis = cc_skill(9323, SkillEffect::BlockActions { conditional: false }, "STUN_VIS");
    stun_vis.abnormal_visuals = vec![7];
    world.data.skill_data.insert_for_test(stun_vis);
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    let visuals_of = |world: &World| {
        let v = crate::model::PlayerView::of(&world.objects, VICTIM).expect("view");
        server_packets::char_info(&v, &crate::game_loop::abnormal::visual_effects(world, VICTIM))
    };

    let before = visuals_of(&world);
    land(&mut world, 9323, VICTIM);
    let after = visuals_of(&world);
    assert!(after.len() > before.len(), "the stunned CharInfo is longer by the visual entry");
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
        .filter(|p| p[0] == 0xFE && p.len() >= 3 && i16::from_le_bytes([p[1], p[2]]) == server_packets::opcodes::EX_USER_INFO_ABNORMAL_VISUAL_EFFECT)
        .count();
    assert_eq!(ave_pkts, 0, "a visual-less buff pushes no ExUserInfoAbnormalVisualEffect");
}
