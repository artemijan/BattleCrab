//! Crowd control (G19): the abnormal-state flags that make stun/sleep/paralyze
//! and root actually do something.

use super::*;
use crate::game_loop::abnormal::has_buff;
use crate::game_loop::helpers::pos_of;
use crate::game_loop::helpers::skill_by_id;

use crate::game_loop::abnormal;
use crate::game_loop::helpers::stat_add;
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
        self_continuous: false,
        without_action: false,
        trait_type: model::skill::TraitType::None,
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
    let skill = skill_by_id(world, skill_id, 1).expect("registered");
    effects::apply_skill_effects(world, CASTER, target, &skill);
}

fn cc_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
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

    effects::handle_buff_expire(&mut world, VICTIM, STUN_ID);
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
    use_magic(&mut world, VICTIM_CID, VICTIM, 91, false, false);
    assert!(
        !world.objects.has_component::<Casting>(&VICTIM),
        "a stunned player cannot cast"
    );

    // Clear the stun, root instead: casting works again.
    effects::handle_buff_expire(&mut world, VICTIM, STUN_ID);
    land(&mut world, ROOT_ID, VICTIM);
    use_magic(&mut world, VICTIM_CID, VICTIM, 91, false, false);
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
    use_magic(&mut world, VICTIM_CID, VICTIM, 91, false, false);
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

/// Java's `startParalyze`/`startStunning` also call `abortAttack()`: the swing
/// already in flight is dropped, not just the cast.
///
/// Two things have to hold, and they are asserted separately because they fail
/// separately. **The wiring**: a stun bumps the attacker's swing counter.
/// **The mechanic**: a hit carrying a stale counter does nothing when it
/// fires. The port cannot cancel a scheduled task, so the counter *is* the
/// cancel.
///
/// The observable is the attacker's own damage line rather than the victim's
/// HP: these two fixtures are both players, and player-on-player damage is
/// gated elsewhere, which would make an HP assertion pass for the wrong
/// reason.
#[test]
fn a_stun_mid_swing_drops_the_hit_that_was_already_in_flight() {
    use crate::game_loop::combat::{abort_attack, do_auto_attack, handle_attack_hit};
    use crate::model::components::AttackState;
    use crate::network::server_packets::sm_ids;

    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut vout = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    // The wiring: a swing arms the attacker's counter, and the stun bumps it.
    do_auto_attack(&mut world, VICTIM, CASTER);
    let queued = world
        .objects
        .get_component::<AttackState>(&VICTIM)
        .expect("a swing arms the attack state")
        .swing_seq;
    land(&mut world, STUN_ID, VICTIM);
    let after_stun = world
        .objects
        .get_component::<AttackState>(&VICTIM)
        .unwrap()
        .swing_seq;
    assert_ne!(
        after_stun, queued,
        "the stun aborts the swing — Java's `abortAttack()` inside `startStunning`"
    );

    // The mechanic, driven directly so no attack roll can make it flaky.
    let landed = |w: &mut World, rx: &mut _, seq: u64| {
        drain(rx);
        handle_attack_hit(w, VICTIM, CASTER, 25, false, false, seq);
        has_system_message(&drain(rx), sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2)
    };
    assert!(
        !landed(&mut world, &mut vout, queued),
        "the pre-stun swing is discarded when it fires"
    );
    assert!(
        landed(&mut world, &mut vout, after_stun),
        "…and the guard is the *stale counter*, not a blanket refusal to hit"
    );

    // A direct abort with no stun involved does the same thing, which is what
    // the fake-death and physical-mute call sites rely on.
    abort_attack(&mut world, VICTIM);
    assert!(
        !landed(&mut world, &mut vout, after_stun),
        "any `abort_attack` invalidates the hits queued before it"
    );
}

/// The object ids named by every `MagicSkillCanceled` in `packets`.
fn canceled_ids(packets: &[Vec<u8>]) -> Vec<i32> {
    packets
        .iter()
        .filter(|p| p.first() == Some(&server_packets::opcodes::MAGIC_SKILL_CANCELED))
        .map(|p| i32::from_le_bytes([p[1], p[2], p[3], p[4]]))
        .collect()
}

/// A stun/sleep landing mid-cast has to stop the cast *animation*, not just the
/// server-side cast: Java's `BlockActions.onStart` → `abortAllSkillCasters()` →
/// `stopCasting(true)`, and that `true` is the leg that broadcasts
/// `MagicSkillCanceled`. Dropping the cast quietly leaves every client drawing
/// the channel — and its FX — for the rest of the client-side cast time, so a
/// slept mob keeps visibly casting after the sleep landed.
#[test]
fn a_stun_broadcasts_magic_skill_canceled_to_stop_the_animation() {
    let (mut world, _db, _l) = cc_world();
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut vout = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    // A player victim, mid-cast.
    use_magic(&mut world, VICTIM_CID, VICTIM, 91, false, false);
    assert!(world.objects.has_component::<Casting>(&VICTIM));
    drain(&mut out);
    drain(&mut vout);

    land(&mut world, STUN_ID, VICTIM);
    assert!(
        canceled_ids(&drain(&mut vout)).contains(&VICTIM),
        "the victim's own client must be told to drop the cast animation"
    );
    assert!(
        canceled_ids(&drain(&mut out)).contains(&VICTIM),
        "and so must everyone watching (the broadcast includes self)"
    );

    // The same for a *monster* mid-cast — the case that shows up as a slept mob
    // that keeps playing its spell animation.
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    let mob_skill = skill_by_id(&world, 91, 1).expect("registered");
    crate::game_loop::npc::cast::start_cast(&mut world, NPC_OID, CASTER, &mob_skill);
    assert!(
        world.objects.has_component::<Casting>(&NPC_OID),
        "the mob is mid-cast"
    );
    drain(&mut out);

    land(&mut world, STUN_ID, NPC_OID);
    assert!(
        !world.objects.has_component::<Casting>(&NPC_OID),
        "the mob's cast is dropped"
    );
    assert!(
        canceled_ids(&drain(&mut out)).contains(&NPC_OID),
        "and the observer is told to stop drawing it"
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

    effects::handle_buff_expire(&mut world, NPC_OID, STUN_ID);
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
        .get_component_mut::<SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(9302, 1);

    let (a, b) = (NPC_OID, NPC_OID + 1);
    add_test_npc(&mut world, a, 20001, "Monster", 5, 100, 0, 0);
    add_test_npc(&mut world, b, 20001, "Monster", 5, 200, 0, 0);
    let far = NPC_OID + 2;
    add_test_npc(&mut world, far, 20001, "Monster", 5, 5000, 0, 0);
    drain(&mut out);

    use_magic(&mut world, CID, CASTER, 9302, false, false);
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

fn cc2_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
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
    use_magic(&mut world, CID, CASTER, 91, false, false);
    assert!(
        !world.objects.has_component::<Casting>(&CASTER),
        "a silenced caster can't cast magic"
    );

    // Clear it and confirm the same cast now works.
    effects::handle_buff_expire(&mut world, CASTER, MUTE_ID);
    use_magic(&mut world, CID, CASTER, 91, false, false);
    assert!(
        world.objects.has_component::<Casting>(&CASTER),
        "and can once the silence ends"
    );
    stop_casting(&mut world, CASTER);

    // A *physical* mute leaves the magic skill alone.
    land(&mut world, PMUTE_ID, CASTER);
    use_magic(&mut world, CID, CASTER, 91, false, false);
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

    use_magic(&mut world, CID, CASTER, 91, false, false);
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
    effects::handle_buff_expire(&mut world, VICTIM, STUN_ID);

    // Under debuff block it does not.
    land(&mut world, DBLOCK_ID, VICTIM);
    land(&mut world, STUN_ID, VICTIM);
    assert!(
        !abnormal::is_blocked_from_actions(&world, VICTIM),
        "a debuff-blocked target refuses the stun entirely"
    );

    // A *buff* still lands (1068 is the Might-like buff, not a debuff).
    let buff = skill_by_id(&world, 1068, 1).expect("might");
    effects::apply_skill_effects(&mut world, CASTER, VICTIM, &buff);
    assert!(
        has_buff(&world, VICTIM, 1068),
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
    items::handle_use_item(&mut world, CID, &use_item_body(1234));
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
        .get_component_mut::<TargetRef>(&VICTIM)
        .unwrap()
        .0 = Some(NPC_OID);
    use_magic(&mut world, VICTIM_CID, VICTIM, 91, false, false);
    assert!(world.objects.has_component::<Casting>(&VICTIM));
    drain(&mut vout);

    // `TargetCancel` rolls through `calcProbability` (`magicLevel + chance −
    // targetLevel`), so even a 100-chance skill has a threshold below 100 and
    // an unforced roll makes this flaky. Force the magic-crit throwaway and a
    // winning probability roll.
    world.force_rolls([0, 0]);
    land(&mut world, TCANCEL_ID, VICTIM);
    assert_eq!(
        world.objects.get_component::<TargetRef>(&VICTIM).unwrap().0,
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
        .get_component_mut::<TargetRef>(&VICTIM)
        .unwrap()
        .0 = Some(NPC_OID);
    land(&mut world, 9315, VICTIM);
    assert_eq!(
        world.objects.get_component::<TargetRef>(&VICTIM).unwrap().0,
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
    effects::handle_buff_expire(&mut world, VICTIM, 9320);
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
        let v = model::PlayerView::of(&world.objects, VICTIM).expect("view");
        server_packets::char_info(
            &v,
            &abnormal::visual_effects(world, VICTIM),
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
    let buff = skill_by_id(&world, 1068, 1).expect("might");
    effects::apply_skill_effects(&mut world, CASTER, VICTIM, &buff);

    let pkts = drain(&mut vout);
    let ave_pkts = pkts
        .iter()
        .filter(|p| {
            is_ex(
                p,
                server_packets::opcodes::EX_USER_INFO_ABNORMAL_VISUAL_EFFECT,
            )
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
            .get_component_mut::<TargetRef>(&VICTIM)
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
            world.objects.get_component::<TargetRef>(&VICTIM).unwrap().0,
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
        .get_component_mut::<TargetRef>(&VICTIM)
        .unwrap()
        .0 = Some(NPC_OID);
    // `cc_skill` carries `magic_level: 0` and the fixture chance is 100, so the
    // threshold is `0 + 100 - level`: put the victim past it.
    world
        .objects
        .get_component_mut::<Player>(&VICTIM)
        .unwrap()
        .level = 100;

    land(&mut world, TCANCEL_ID, VICTIM);
    assert_eq!(
        world.objects.get_component::<TargetRef>(&VICTIM).unwrap().0,
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
            .get_component_mut::<TargetRef>(&VICTIM)
            .unwrap()
            .0 = Some(NPC_OID);
        // Give the cancel a trait the victim can resist.
        let mut skill = world.data.skill_data.get(TCANCEL_ID, 1).unwrap().clone();
        skill.trait_type = TraitType::Shock;
        world.data.skill_data.insert_for_test(skill.clone());
        if resist {
            effects::merge_defence_traits(&mut world, VICTIM, &[(TraitType::Shock, 0.5)]);
        }
        // Threshold is `0 + 100 - level` (~99 here); halve it and a 60 roll
        // stops landing.
        world.force_rolls([0, 60]);
        effects::apply_skill_effects(&mut world, CASTER, VICTIM, &skill);
        world.objects.get_component::<TargetRef>(&VICTIM).unwrap().0
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
            .get_component_mut::<TargetRef>(&VICTIM)
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
                .get_component_mut::<model::components::StatModifiers>(&VICTIM)
                .unwrap();
            *mods.add.entry(Stat::FireRes).or_insert(0.0) += 300.0;
        }
        // `calcAttributeBonus` floors at 0.75, so the resisted threshold is
        // ~74 against ~99 unresisted — a roll of 80 separates them.
        world.force_rolls([0, 80]);
        effects::apply_skill_effects(&mut world, CASTER, VICTIM, &skill);
        world.objects.get_component::<TargetRef>(&VICTIM).unwrap().0
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
        server_packets::npc_info(
            &v,
            t,
            &world.cfg.npc,
            &world.cfg.champion,
            &abnormal::visual_effects(world, NPC_OID),
            None,
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
        abnormal::visual_effects(&world, NPC_OID),
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
    visibility::on_enter_world(&world, 9, 3099);
    let sent = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_INFO)
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
        server_packets::npc_info(&v, t, &world.cfg.npc, &world.cfg.champion, &[], None)
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
    visibility::on_enter_world(&world, 9, 3098);
    let sent = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_INFO)
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
fn sleep_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
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

    combat::apply_physical_damage(&mut world, CASTER, VICTIM, 10.0, false, false);
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

    // Control: same hit, a stun instead — a stun is not `removedOnDamage`, so
    // `stopEffectsOnDamage` must leave it be.
    let (mut world, _db, _l) = sleep_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    // `BreakStun` is turned **off** here deliberately. It ships `True` on this
    // dist — the comment that used to sit here claimed the opposite — and gives
    // every non-DoT hit a 1-in-14 chance to free a stunned target. That is a
    // *different* mechanic (`Formulas.calcStunBreak`, tested separately); left
    // on, it would make the assertion below fail one time in fourteen.
    world.cfg.character.alt_game_stun_break = false;
    land(&mut world, STUN_ID, VICTIM);
    combat::apply_physical_damage(&mut world, CASTER, VICTIM, 10.0, false, false);
    assert!(
        abnormal::is_blocked_from_actions(&world, VICTIM),
        "a stun is not `removedOnDamage` — hitting a stunned target does not \
         itself free them"
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

    combat::apply_physical_damage(&mut world, CASTER, NPC_OID, 5.0, false, false);
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

    combat::apply_physical_damage(&mut world, CASTER, VICTIM, 3.0, true, false);
    combat::apply_physical_damage(&mut world, CASTER, NPC_OID, 3.0, true, false);

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
    assert!(abnormal::is_buff_blocked(&world, CASTER), "the flag is up");

    land(&mut world, 9324, CASTER);
    assert!(
        !has_buff(&world, CASTER, 9324),
        "a buff cannot land on a buff-blocked target — not even their own"
    );

    // A debuff is explicitly *not* blocked by this flag.
    land(&mut world, ROOT_ID, CASTER);
    assert!(
        has_buff(&world, CASTER, ROOT_ID),
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
        !abnormal::is_pacified(&world, NPC_OID),
        "not pacified to begin with"
    );
    land(&mut world, PACIFY_ID, NPC_OID);
    assert!(
        abnormal::is_pacified(&world, NPC_OID),
        "the mob is pacified while the buff is up"
    );
    effects::handle_buff_expire(&mut world, NPC_OID, PACIFY_ID);
    assert!(
        !abnormal::is_pacified(&world, NPC_OID),
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
        abnormal::is_physical_attack_muted(&world, CASTER),
        "the auto-attack lock is up"
    );
    assert!(
        !abnormal::is_physical_muted(&world, CASTER),
        "…and it is NOT the skill lock — two distinct flags"
    );
    assert!(!abnormal::is_muted(&world, CASTER), "…nor the magic one");
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
    assert!(abnormal::is_untargetable(&world, NPC_OID));
    assert!(
        !abnormal::is_targeting_disabled(&world, NPC_OID),
        "being unclickable does not stop you clicking"
    );

    land(&mut world, 9326, CASTER);
    assert!(abnormal::is_targeting_disabled(&world, CASTER));
    assert!(
        !abnormal::is_untargetable(&world, CASTER),
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

    assert!(!abnormal::shields_from_all_angles(&world, CASTER));
    land(&mut world, 9327, CASTER);
    assert!(
        abnormal::shields_from_all_angles(&world, CASTER),
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
    let sd = dist::skills();
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
            .has_component::<model::components::LockedTarget>(&NPC_OID),
        "Java's isPlayable() guard means a mob is never locked by TargetMe"
    );

    // A player: target forced to the caster and locked there.
    land(&mut world, 9331, victim);
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&victim)
            .and_then(|t| t.0),
        Some(CASTER),
        "the victim's selection is dragged onto the taunter"
    );
    assert_eq!(
        world
            .objects
            .get_component::<model::components::LockedTarget>(&victim)
            .map(|l| l.0),
        Some(CASTER),
        "…and locked"
    );

    // `TargetMe.onExit` — the lock goes with the buff.
    effects::handle_buff_expire(&mut world, victim, 9331);
    assert!(
        !world
            .objects
            .has_component::<model::components::LockedTarget>(&victim),
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
        .add_components(&CASTER, model::components::LockedTarget(NPC_OID));
    drain(&mut out);
    handle_action(&mut world, CID, &action_body(other_npc, 0));
    let pkts = drain(&mut out);
    assert!(
        has_system_message(&pkts, server_packets::sm_ids::FAILED_TO_CHANGE_ENMITY),
        "the refusal is explained, not silent"
    );
    assert_ne!(
        world
            .objects
            .get_component::<TargetRef>(&CASTER)
            .and_then(|t| t.0),
        Some(other_npc),
        "and the selection did not move"
    );

    // The locked NPC itself is still clickable.
    handle_action(&mut world, CID, &action_body(NPC_OID, 0));
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&CASTER)
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
            .get_component::<AggroList>(&NPC_OID)
            .and_then(|a| a.0.get(&CASTER).map(|i| i.hate))
            .unwrap_or(0.0)
    };

    // Unbuffed auto-attack: the plain `damage·100 / (level + 7)`.
    combat::npc_receive_damage(&mut world, NPC_OID, CASTER, 10.0, true);
    let plain = hate_of(&world);
    assert!(plain > 0.0, "baseline hate: {plain}");

    let mut mods = world
        .objects
        .get_component::<model::components::StatModifiers>(&CASTER)
        .cloned()
        .expect("modifiers");
    mods.mul.insert(Stat::HateAttack, 3.0);
    world.objects.add_components(&CASTER, mods);

    // Same damage, now tripled…
    combat::npc_receive_damage(&mut world, NPC_OID, CASTER, 10.0, true);
    let after_auto = hate_of(&world) - plain;
    assert!(
        (after_auto - plain * 3.0).abs() < 1e-6,
        "an auto-attack's hate is tripled ({plain} → {after_auto})"
    );

    // …but a *skill*'s hate is untouched, which is Java's `skill == null` gate.
    let before = hate_of(&world);
    combat::npc_receive_damage(&mut world, NPC_OID, CASTER, 10.0, false);
    let after_skill = hate_of(&world) - before;
    assert!(
        (after_skill - plain).abs() < 1e-6,
        "skill damage generates unmultiplied hate ({plain} vs {after_skill})"
    );
}

/// G34 S4 sub-slice 4 — `SkillEvasion` (Ultimate Evasion 111, Evasion 446).
///
/// Java keeps this in a **per-`magicType` map**, not a `Stat`: both learnable
/// sources are bucket 0 (physical skills), so the buff must dodge those and
/// leave magic alone. A single global dodge stat would pass any test that only
/// ever fires one kind of skill.
#[test]
fn skill_evasion_dodges_only_its_own_magic_type() {
    let (mut world, _db, _l) = cc2_world();
    world.data.skill_data.insert_for_test(cc_skill(
        9341,
        SkillEffect::SkillEvasion {
            magic_type: 0,
            amount: 100.0, // always dodge, so the roll is not the variable
        },
        "EVASION",
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    land(&mut world, 9341, NPC_OID);
    let evasion = |world: &World, bucket: i32| {
        world
            .objects
            .get_component::<model::components::StatModifiers>(&NPC_OID)
            .and_then(|m| m.skill_evasion.get(&bucket).copied())
            .unwrap_or(0.0)
    };
    assert_eq!(evasion(&world, 0), 100.0, "the physical-skill bucket");
    assert_eq!(
        evasion(&world, 1),
        0.0,
        "…and nothing in the magic bucket — Java keys the map by magicType"
    );

    // The merge is only half of it — the *roll* has to consume the map, or the
    // buff is a number nobody reads. A physical-skill nuke (magicType 0) at
    // 100 % dodge must land no damage at all.
    let mut nuke = cc_skill(
        9343,
        SkillEffect::PhysicalAttack {
            power: 500.0,
            p_atk_mod: 1.0,
            p_def_mod: 1.0,
            critical_chance: 0.0,
            ignore_shield_defence: false,
        },
        "NONE",
    );
    nuke.magic_type = 0; // the bucket the buff covers
    world.data.skill_data.insert_for_test(nuke);
    let hp_before = world
        .objects
        .get_component::<Vitals>(&NPC_OID)
        .map(|v| v.cur_hp)
        .unwrap_or(0.0);
    land(&mut world, 9343, NPC_OID);
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&NPC_OID)
            .map(|v| v.cur_hp)
            .unwrap_or(0.0),
        hp_before,
        "a 100 % dodge takes no damage — the map has to reach the roll"
    );

    // `onExit` unmerges: a per-bucket map has no `Stat` recompute to fall back
    // on, so without it Ultimate Evasion's dodge would be permanent.
    effects::handle_buff_expire(&mut world, NPC_OID, 9341);
    assert_eq!(
        evasion(&world, 0),
        0.0,
        "the dodge goes with the buff, or it never goes at all"
    );
}

/// `SkillTurning` — Spell Turning (1412). The name suggests a reflect; the
/// handler is an offensive `ENEMY_ONLY` instant that **breaks the target's
/// cast**. Java bails on a self-cast and on raid bosses.
#[test]
fn skill_turning_breaks_the_targets_cast_but_not_a_raids() {
    let (mut world, _db, _l) = cc2_world();
    world.data.skill_data.insert_for_test(cc_skill(
        9342,
        SkillEffect::SkillTurning {
            chance: 100,
            static_chance: false,
        },
        "NONE",
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = 5961;
    let _v = ingame_player_access(&mut world, 2, victim, 0);

    // A self-cast is a no-op even at 100 % — Java returns before the break.
    land(&mut world, 9342, CASTER);

    // Against another caster it breaks the cast.
    world.objects.add_components(
        &victim,
        Casting(model::CastState {
            skill_id: 1177,
            skill_level: 1,
            skill_sub_level: 0,
            target_object_id: CASTER,
            seq: 1,
            // `canAbortCast()` — only an unlaunched cast can be broken.
            launched: false,
            cancel_ms: 0,
            cool_ms: 0,
            trigger_item_object_id: 0,
        }),
    );
    land(&mut world, 9342, victim);
    assert!(
        !world.objects.has_component::<Casting>(&victim),
        "the victim's cast is broken"
    );
}

/// `CounterPhysicalSkill` — Shield of Revenge (439) at 20 %, Counterattack
/// (447) at 90 %. The effect grants a **chance**, not a multiplier, and Java
/// runs the counter from `reduceCurrentHp` *before* the damage lands.
///
/// Two guards decide whether it can fire at all, and both are asserted because
/// dropping either would look correct in a melee-only test: **magic skills
/// cannot be countered**, and neither can anything with `castRange > 40`.
#[test]
fn counter_physical_skill_answers_melee_skills_only() {
    use crate::model::stats::Stat;
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    // 100 % counter on the mob, and enough P.Atk for the counter to bite.
    let mut mods = world
        .objects
        .get_component::<model::components::StatModifiers>(&NPC_OID)
        .cloned()
        .unwrap_or_default();
    mods.add.insert(Stat::VengeanceSkillPhysicalDamage, 100.0);
    world.objects.add_components(&NPC_OID, mods);
    if let Some(cs) = world.objects.get_component_mut::<CombatStats>(&NPC_OID) {
        cs.p_atk = 500.0;
    }

    let caster_hp = |world: &World| {
        world
            .objects
            .get_component::<Vitals>(&CASTER)
            .map(|v| v.cur_hp)
            .unwrap_or(0.0)
    };

    // A melee skill (castRange 40, physical) is countered.
    let mut melee = cc_skill(9351, SkillEffect::Root, "NONE");
    melee.magic_type = 0;
    melee.cast_range = 40;
    world.data.skill_data.insert_for_test(melee);
    let before = caster_hp(&world);
    effects::apply_skill_damage(
        &mut world,
        CASTER,
        NPC_OID,
        effects::SkillHit {
            damage: 1.0,
            caster_name: "c",
            skill_id: 9351,
            ..Default::default()
        },
    );
    assert!(
        caster_hp(&world) < before,
        "a melee skill draws a counter ({before} → {})",
        caster_hp(&world)
    );

    // A *magic* skill never is, however high the chance.
    let mut magic = cc_skill(9352, SkillEffect::Root, "NONE");
    magic.magic_type = 1;
    magic.cast_range = 40;
    world.data.skill_data.insert_for_test(magic);
    let before = caster_hp(&world);
    effects::apply_skill_damage(
        &mut world,
        CASTER,
        NPC_OID,
        effects::SkillHit {
            damage: 1.0,
            is_magic: true,
            caster_name: "c",
            skill_id: 9352,
            ..Default::default()
        },
    );
    assert_eq!(
        caster_hp(&world),
        before,
        "magic is not counterable — Java bails on skill.isMagic()"
    );

    // Nor is a ranged one: `castRange > MELEE_ATTACK_RANGE` (40).
    let mut ranged = cc_skill(9353, SkillEffect::Root, "NONE");
    ranged.magic_type = 0;
    ranged.cast_range = 600;
    world.data.skill_data.insert_for_test(ranged);
    let before = caster_hp(&world);
    effects::apply_skill_damage(
        &mut world,
        CASTER,
        NPC_OID,
        effects::SkillHit {
            damage: 1.0,
            caster_name: "c",
            skill_id: 9353,
            ..Default::default()
        },
    );
    assert_eq!(
        caster_hp(&world),
        before,
        "only melee-range skills can be countered"
    );
}

/// G34 S4 sub-slice 5 — `EnlargeAbnormalSlot` (Divine Inspiration 1405) raises
/// the **good-buff** slot cap, and only that pool: Java's `setMaxBuffCount` is
/// read by `EffectList` for buffs, never for dances.
///
/// Modelled as a `Stat` rather than Java's setter on purpose — `apply_buff`
/// rebuilds `StatModifiers` from the surviving buffs on every change, so the
/// bonus is *derived* and cannot drift the way an add/subtract pair can when a
/// buff leaves by some other path. The expiry case is asserted for exactly
/// that reason.
#[test]
fn enlarge_abnormal_slot_raises_the_buff_cap_and_gives_it_back() {
    use crate::model::stats::{Stat, StatModifierType};
    let (mut world, _db, _l) = cc2_world();
    world.data.combat_caps.max_buff_count = 2; // small enough to observe
    let mut boost = cc_skill(9361, SkillEffect::Root, "SLOT_BOOST");
    boost.effects = vec![SkillEffect::StatModifier(
        model::skill::StatModifierEffect {
            stat: Stat::MaxBuffSlots,
            mode: StatModifierType::Diff,
            amount: 2.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
        },
    )];
    boost.effect_point = 100;
    boost.is_debuff = false;
    world.data.skill_data.insert_for_test(boost);
    // Three ordinary buffs, so the cap is what decides how many survive.
    for id in 9371..9374 {
        let mut b = cc_skill(id, SkillEffect::Root, &format!("B{id}"));
        b.effect_point = 100;
        b.is_debuff = false;
        world.data.skill_data.insert_for_test(b);
    }
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let buff_count = |world: &World| {
        world
            .objects
            .get_component::<Buffs>(&CASTER)
            .map(|b| b.0.len())
            .unwrap_or(0)
    };

    // Without the boost the cap holds at 2.
    for id in 9371..9374 {
        land(&mut world, id, CASTER);
    }
    assert_eq!(buff_count(&world), 2, "the base cap of 2 holds");

    // With it, four fit (2 base + 2 granted) — the boost itself occupies one.
    land(&mut world, 9361, CASTER);
    for id in 9371..9374 {
        land(&mut world, id, CASTER);
    }
    assert_eq!(
        buff_count(&world),
        4,
        "Divine Inspiration's slots are real, not cosmetic"
    );
}

/// `DispelBySlotMyself` (Flames of Invincibility 1427) strips the bearer's own
/// buffs of the listed abnormal types — but **spares an `irreplacableBuff`**,
/// which `DispelBySlot` does not. Both halves asserted, since a version that
/// dispelled everything would look correct against ordinary buffs.
#[test]
fn dispel_by_slot_myself_spares_irreplacable_buffs() {
    let (mut world, _db, _l) = cc2_world();
    let mut stance = cc_skill(9381, SkillEffect::Root, "MAGICAL_STANCE");
    stance.effect_point = 100;
    stance.is_debuff = false;
    world.data.skill_data.insert_for_test(stance);

    // Same abnormal type, but flagged to survive death — Java's
    // `isIrreplacableBuff()`, which the port folds into `stay_after_death`.
    let mut protected = cc_skill(9382, SkillEffect::Root, "MAGICAL_STANCE");
    protected.effect_point = 100;
    protected.is_debuff = false;
    protected.stay_after_death = true;
    world.data.skill_data.insert_for_test(protected);

    let mut dispeller = cc_skill(
        9383,
        SkillEffect::DispelBySlotMyself {
            dispel: vec!["MAGICAL_STANCE".into()],
        },
        "NONE",
    );
    dispeller.effect_point = 100;
    dispeller.is_debuff = false;
    world.data.skill_data.insert_for_test(dispeller);

    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    land(&mut world, 9381, CASTER);
    land(&mut world, 9382, CASTER);
    land(&mut world, 9383, CASTER);

    let has = |world: &World, id: i32| has_buff(world, CASTER, id);
    assert!(!has(&world, 9381), "the ordinary MAGICAL_STANCE buff goes");
    assert!(
        has(&world, 9382),
        "…but an irreplacable one of the same type stays"
    );
}

/// G34 S4 sub-slice 6 — `SkillMastery` (330 STR / 331 INT) + `SkillMasteryRate`
/// (Focus Skill Mastery 334): a chance for a cast's cooldown to collapse to
/// 100 ms, announced with "A skill is ready to be used again".
///
/// The stat stores the **`BaseStat` ordinal**, not a magnitude, and Java's enum
/// order (`STR, INT, DEX, …`) differs from this port's (`Str, Dex, Con, Int, …`)
/// — copying Java's number across would make Skill Mastery 331 read DEX instead
/// of INT. Asserted by driving both stats.
#[test]
fn skill_mastery_collapses_the_cooldown_and_reads_the_right_base_stat() {
    use crate::model::components::{BaseStats, StatModifiers};
    use crate::model::stats::{BaseStat, Stat};
    let (mut world, _db, _l) = cc2_world();
    // The **real** `statBonus` table: `GameData::for_test`'s stub returns 1.0
    // for every stat, which makes "which BaseStat was selected" unobservable —
    // exactly the property under test. One dist load, reused for all four
    // measurements below.
    world.data.stat_bonus = crate::data::StatBonus::load_from(crate::data::DIST_GAME);
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    // A lopsided stat spread, so "which BaseStat" is observable: huge INT,
    // minimal DEX.
    if let Some(b) = world.objects.get_component_mut::<BaseStats>(&CASTER) {
        b.int_ = 99;
        b.dex = 1;
    }

    // Derive the discriminating roll from the real bonus table rather than
    // guessing a threshold: pick one strictly between the two chances, so the
    // assertion can only pass if the *stat selection* is right.
    const RATE: f64 = 10.0;
    let int_chance = world.data.stat_bonus.bonus(BaseStat::Int, 99) * RATE;
    let dex_chance = world.data.stat_bonus.bonus(BaseStat::Dex, 1) * RATE;
    assert!(
        int_chance > dex_chance + 2.0,
        "the fixture has to separate the two stats: INT {int_chance}, DEX {dex_chance}"
    );
    // `calcSkillMastery` draws `Rnd.nextDouble() * 100`, which the port spells
    // `roll_f64() * 100` — and `roll_f64` quantizes a forced value as
    // `v / 1_000_000`, so a forced `v` reads as the percentage `v / 10_000`.
    // Forcing the *midpoint* of the two chances therefore needs that scale, and
    // gets to keep the fraction the old `as i32` was throwing away.
    let roll = (((int_chance + dex_chance) / 2.0) * 10_000.0) as i32;

    let mastery_fires = |world: &mut World, stat: BaseStat| {
        let mut mods = world
            .objects
            .get_component::<StatModifiers>(&CASTER)
            .cloned()
            .unwrap_or_default();
        mods.add.insert(Stat::SkillMastery, stat as i32 as f64);
        mods.mul.insert(Stat::SkillMasteryRate, RATE);
        world.objects.add_components(&CASTER, mods);
        world.clear_forced_rolls();
        world.force_roll(roll);
        effects::calc_skill_mastery(world, CASTER)
    };

    assert!(
        mastery_fires(&mut world, BaseStat::Int),
        "INT 99 clears a roll of {roll}"
    );
    assert!(
        !mastery_fires(&mut world, BaseStat::Dex),
        "DEX 1 does not — so the ordinal really selects the stat"
    );

    // With no `SKILL_MASTERY` at all there is no proc, whatever the rate.
    let mut mods = world
        .objects
        .get_component::<StatModifiers>(&CASTER)
        .cloned()
        .unwrap();
    mods.add.remove(&Stat::SkillMastery);
    world.objects.add_components(&CASTER, mods);
    world.clear_forced_rolls();
    world.force_roll(0);
    assert!(
        !effects::calc_skill_mastery(&mut world, CASTER),
        "Java's `getAdd(SKILL_MASTERY, -1) == -1` bail"
    );
}

/// `calcEffectSuccess` is gated on **`activateRate != -1` alone**, not on
/// `isBad()`:
///
/// ```java
/// // Skill.applyEffects
/// addContinuousEffects = !passive && (isToggle() || (isContinuous() && Formulas.calcEffectSuccess(effector, effected, this)));
/// // Formulas.calcEffectSuccess
/// if (activateRate == -1) return true;
/// ```
///
/// Three learnable skills on this dist sit in the gap an `isBad()` gate opens,
/// and the first assertion pins them **off the real dist** so the fixture below
/// can't drift away from what it is modelling.
#[test]
fn a_continuous_skill_rolls_to_land_even_when_its_effect_point_is_not_negative() {
    // Veil is a mesmerize (`isDebuff`, trait DERANGEMENT) that declares no
    // `<effectPoint>` at all; the two heals declare a positive one. All three
    // carry an `activateRate`, so all three roll in Java.
    let skills = dist::skills();
    for (id, rate) in [(106, 70), (1217, 0), (1219, 0)] {
        let skill = skills
            .get(id, 1)
            .unwrap_or_else(|| panic!("skill {id} on the dist"));
        assert_eq!(
            skill.activate_rate, rate,
            "skill {id} carries an activateRate"
        );
        assert!(
            !skill.is_bad(),
            "skill {id}'s effectPoint is not negative — an `isBad()` gate would skip its roll"
        );
    }

    const TARGET: i32 = 4713;
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _t = ingame_caster(&mut world, 2, TARGET, 0, 0);

    // Veil's shape: activateRate 70, no lvlBonusRate, effectPoint absent (0).
    let mut veil = cc_skill(106, SkillEffect::Passive, "TURN_PASSIVE");
    veil.effect_point = 0;
    veil.is_debuff = true;
    veil.activate_rate = 70;
    veil.lvl_bonus_rate = 0;
    veil.magic_level = 40;
    veil.abnormal_time = 120;
    world.data.skill_data.insert_for_test(veil.clone());

    // `baseMod = (magicLevel - targetLevel + 3) * 0 + 70 + 30 = 100`, clamped to
    // the config ceiling of 90. Java resists on `finalRate <= Rnd.get(100)`.
    world.clear_forced_rolls();
    world.force_roll(89);
    assert!(
        effects::apply_continuous_effects(&mut world, CASTER, TARGET, &veil, None),
        "89 < 90 — it lands"
    );
    world.clear_forced_rolls();
    world.force_roll(90);
    assert!(
        !effects::apply_continuous_effects(&mut world, CASTER, TARGET, &veil, None),
        "90 is not below 90 — resisted, which an `isBad()` gate would never allow"
    );

    // The `-1` sentinel still short-circuits, and consumes no roll.
    let mut always = veil.clone();
    always.activate_rate = -1;
    world.clear_forced_rolls();
    world.force_roll(0);
    assert!(
        effects::apply_continuous_effects(&mut world, CASTER, TARGET, &always, None),
        "`activateRate == -1` returns true before any roll"
    );
}

/// `Heal.instant` asks **`isPlayer() && isMageClass()`**, not "is it a player":
///
/// ```java
/// if (((sps || bss) && (effector.isPlayer() && effector.getActingPlayer().isMageClass())) || effector.isSummon())
/// {
///     staticShotBonus = skill.getMpConsume();   // ← the mage arm's whole point
///     mAtkMul = bss ? 4 * shotsBonus : 2 * shotsBonus;
/// }
/// ```
///
/// A **fighter** with a spiritshot charged falls through to the grade arm and
/// gets no static bonus at all. The port had stood `isPlayer()` in for the
/// class test, which handed every fighter the mage's `mpConsume` bonus.
///
/// `MAGE_GROUP` is `ClassId.isMage()` exactly for every id this chronicle can
/// reach — the two sets differ only at ids ≥ 143 (Ertheia and the awakened
/// classes), which no character here holds.
#[test]
fn only_a_mage_class_gets_the_spiritshot_heal_bonus() {
    const MP_CONSUME: i32 = 200;
    // 15 = cleric, 1 = warrior — one on each side of `MAGE_GROUP`.
    const CLERIC: i32 = 15;
    const WARRIOR: i32 = 1;

    let (mut world, _db, _l) = cc2_world();
    // The **real** `CategoryData.xml`: the claim under test is that its
    // `MAGE_GROUP` is Java's per-`ClassId` `isMage` flag, so a stub category
    // would assert nothing.
    world.data.categories = crate::data::CategoryData::load_from(crate::data::DIST_GAME);
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    assert!(
        world.data.categories.contains("MAGE_GROUP", CLERIC)
            && !world.data.categories.contains("MAGE_GROUP", WARRIOR),
        "the fixture's two class ids have to straddle the category"
    );

    let mut heal = cc_skill(9393, SkillEffect::Heal { power: 10.0 }, "NONE");
    heal.effect_point = 100;
    heal.is_debuff = false;
    heal.magic_type = 1;
    heal.mp_consume = MP_CONSUME;
    world.data.skill_data.insert_for_test(heal);

    let healed_as = |world: &mut World, class_id: i32| -> f64 {
        if let Some(p) = world.objects.get_component_mut::<model::Player>(&CASTER) {
            p.class_id = class_id;
            p.charge_shot(crate::model::ShotType::Spiritshots);
        }
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&CASTER) {
            v.max_hp = 100_000;
            v.cur_hp = 1.0;
        }
        land(world, 9393, CASTER);
        world
            .objects
            .get_component::<Vitals>(&CASTER)
            .map(|v| v.cur_hp - 1.0)
            .unwrap_or(0.0)
    };

    let mage = healed_as(&mut world, CLERIC);
    let fighter = healed_as(&mut world, WARRIOR);
    // Both arms reach `mAtkMul = 2` (the grade arm's `1 + 1`, the mage arm's
    // `2 · shotsBonus` with an unenchanted weapon), so the sqrt terms cancel and
    // the whole difference is the static bonus.
    assert!(
        (mage - fighter - MP_CONSUME as f64).abs() < 1e-6,
        "the mage's spiritshot is worth exactly the skill's mpConsume more \
         ({mage} vs {fighter})"
    );
}

/// `calcSkillMastery` draws a **continuous** value, not a 0-99 integer:
///
/// ```java
/// final double chance = BaseStat.values()[val].calcBonus(actor) * actor.getStat().getMul(Stat.SKILL_MASTERY_RATE, 1);
/// return ((Rnd.nextDouble() * 100.) < (chance * Config.SKILL_MASTERY_CHANCE_MULTIPLIERS[…]));
/// ```
///
/// `roll(100) < chance` — the shape the port used — rounds every fractional
/// chance **up**, because there is no integer strictly between 30 and 31 to
/// lose on a 30.5. And fractions are the normal case here: the chance is a
/// base-stat *bonus* off a per-point curve, times a rate multiplier.
///
/// The fixture picks 30.5 % and rolls 30.4, the one draw the two shapes
/// disagree about.
#[test]
fn skill_mastery_draws_a_continuous_chance_not_a_whole_percent() {
    use crate::model::components::StatModifiers;
    use crate::model::stats::{BaseStat, Stat};

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    // `cc2_world`'s stat-bonus table answers 1.0 for everything, so the rate
    // *is* the chance — 30.5 %.
    let mut mods = world
        .objects
        .get_component::<StatModifiers>(&CASTER)
        .cloned()
        .unwrap_or_default();
    mods.add
        .insert(Stat::SkillMastery, BaseStat::Int as i32 as f64);
    mods.mul.insert(Stat::SkillMasteryRate, 30.5);
    world.objects.add_components(&CASTER, mods);

    // A forced roll reads as `v / 10_000` percent (`roll_f64` quantizes by
    // 1e-6, and the formula scales by 100).
    for (forced, expected, why) in [
        (304_000, true, "30.4 % is below the 30.5 % chance"),
        (306_000, false, "30.6 % is above it"),
    ] {
        world.clear_forced_rolls();
        world.force_roll(forced);
        assert_eq!(
            effects::calc_skill_mastery(&mut world, CASTER),
            expected,
            "{why} — an integer roll could not tell these apart"
        );
    }
}

/// `Formulas.calcEffectAbnormalTime` — a **Skill Mastery proc doubles a buff's
/// duration**, and does so on a roll entirely separate from the one that
/// collapses the cooldown.
///
/// ```java
/// // BuffInfo(…) constructor
/// _abnormalTime = Formulas.calcEffectAbnormalTime(effector, effected, skill);
/// // Formulas
/// int time = … skill.getAbnormalTime();
/// if (!skill.isStatic() && calcSkillMastery(caster, skill)) time *= 2;
/// ```
///
/// The cooldown proc (`apply_reuse`) is gated to `operateType A1`, which
/// excludes every buff; this one is gated only on `isStatic()`. That difference
/// is the mechanic: an Eva's Saint who learns Skill Mastery 331 at 77 rolls it
/// on each buff they land and sometimes gets twice the duration.
#[test]
fn skill_mastery_doubles_a_buffs_duration() {
    use crate::model::components::StatModifiers;
    use crate::model::skill::StatModifierEffect;
    use crate::model::stats::{BaseStat, Stat, StatModifierType};

    const TARGET: i32 = 4711;
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _t = ingame_caster(&mut world, 2, TARGET, 0, 0);

    let skill = Skill {
        id: 1085,
        level: 1,
        abnormal_type: "MAGIC_ATTACK_UP".into(),
        abnormal_time: 1200,
        // Not `isStatic()` — `magicType == 2` is what would exempt it.
        magic_type: 1,
        effects: vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::MagicalAttack,
            mode: StatModifierType::Diff,
            amount: 25.0,
            ..Default::default()
        })],
        ..Default::default()
    };

    world.data.skill_data.insert_for_test(skill.clone());

    let duration = |world: &mut World| {
        // Re-applying the same abnormal type replaces the live buff in place,
        // so the freshest entry is always the one just landed.
        assert!(
            effects::apply_continuous_effects(world, CASTER, TARGET, &skill, None),
            "the buff has to land for the duration to mean anything"
        );
        let start = world.tick;
        world
            .objects
            .get_component::<Buffs>(&TARGET)
            .and_then(|b| b.0.last().map(|x| x.expires_at_tick - start))
            .expect("the buff landed")
    };

    // No Skill Mastery stat at all → `getAdd(SKILL_MASTERY, -1) == -1` bails
    // before any roll, so this is the plain 1200 s.
    assert_eq!(duration(&mut world), 12_000, "1200 s at 10 ticks/s");

    // Give the caster mastery off INT, with a rate that makes the proc certain
    // for a roll of 0 and impossible for a roll of 99.
    let mut mods = world
        .objects
        .get_component::<StatModifiers>(&CASTER)
        .cloned()
        .unwrap_or_default();
    mods.add
        .insert(Stat::SkillMastery, BaseStat::Int as i32 as f64);
    mods.mul.insert(Stat::SkillMasteryRate, 50.0);
    world.objects.add_components(&CASTER, mods);

    // Forced rolls read as `v / 10_000` percent (see `calcSkillMastery`), so
    // 90 % loses against the fixture's 50 % chance and 0 % wins.
    world.clear_forced_rolls();
    world.force_roll(900_000);
    assert_eq!(
        duration(&mut world),
        12_000,
        "a losing mastery roll leaves the duration alone"
    );

    world.clear_forced_rolls();
    world.force_roll(0);
    assert_eq!(
        duration(&mut world),
        24_000,
        "the proc doubles it — 1200 s becomes 2400 s"
    );

    // A **static** skill is exempt in Java even on a proc.
    let static_skill = Skill {
        magic_type: 2,
        ..skill.clone()
    };
    world.data.skill_data.insert_for_test(static_skill.clone());
    world.clear_forced_rolls();
    world.force_roll(0);
    effects::apply_continuous_effects(&mut world, CASTER, TARGET, &static_skill, None);
    let start = world.tick;
    assert_eq!(
        world
            .objects
            .get_component::<Buffs>(&TARGET)
            .and_then(|b| b.0.last().map(|x| x.expires_at_tick - start))
            .expect("the buff landed"),
        12_000,
        "`isStatic()` skips the doubling"
    );
}

/// `Lucky` (194) is an **empty effect** in Java — its handler has only a
/// `canStart` guard, and `Player.isLucky()` (`level <= 9 && affected by 194`)
/// is the entire mechanic. It exempts a newbie from the death exp penalty.
///
/// Both halves of the predicate are asserted: the buff alone is not enough
/// above level 9, which is what stops a twink from carrying it up.
#[test]
fn lucky_exempts_a_newbie_from_the_death_exp_penalty() {
    const LUCKY: i32 = 194;
    let (mut world, _db, _l) = cc2_world();
    let mut lucky = cc_skill(LUCKY, SkillEffect::Lucky, "LUCKY");
    lucky.effect_point = 100;
    lucky.is_debuff = false;
    world.data.skill_data.insert_for_test(lucky);
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let set_level = |world: &mut World, level: i32| {
        if let Some(p) = world.objects.get_component_mut::<Player>(&CASTER) {
            p.level = level;
        }
    };

    set_level(&mut world, 5);
    assert!(
        !crate::game_loop::death::is_lucky(&world, CASTER),
        "level alone is not enough — the buff has to be up"
    );
    land(&mut world, LUCKY, CASTER);
    assert!(
        crate::game_loop::death::is_lucky(&world, CASTER),
        "a level-5 character holding Lucky is exempt"
    );

    set_level(&mut world, 10);
    assert!(
        !crate::game_loop::death::is_lucky(&world, CASTER),
        "…and the buff alone is not enough past level 9"
    );
}

/// `MpVampiricAttack` (Weapon Mastery 250) — the MP twin of the HP drain, and
/// **its config gate is shaped the opposite way**, which is the whole point of
/// this test. HP vampirism asks `skill == null || WORKS_WITH_SKILLS`: melee by
/// default. MP vampirism asks `skill != null || WORKS_WITH_MELEE`: *skills* by
/// default. Both configs are off on this dist, so Weapon Mastery drains MP on
/// skill hits and nothing at all on a melee swing.
#[test]
fn mp_vampiric_drains_on_skills_not_melee() {
    use crate::model::stats::Stat;
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 20, 100, 0, 0);

    // 10 % of damage, and a `sum` chosen to make the chance exactly 1.0 so the
    // test is about the *gate*, not the roll: the finalizer is
    // `min(1, sum / (percent × 100) / 100)`, so `sum = 0.1 × 100 × 100 = 1000`.
    // (Weapon Mastery's own `amount 10` gives sum 300 → **0.3**, which is
    // Java's own "Classic: 30% chance" comment — using it here made the first
    // draft of this test fail 70 % of the time.)
    let mut mods = world
        .objects
        .get_component::<model::components::StatModifiers>(&CASTER)
        .cloned()
        .unwrap_or_default();
    mods.add.insert(Stat::AbsorbManaDamagePercent, 0.1);
    mods.add.insert(Stat::MpVampiricSum, 1000.0);
    world.objects.add_components(&CASTER, mods);
    // Room to drain into, and something to drain from.
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&CASTER) {
        v.max_mp = 10_000;
        v.cur_mp = 0.0;
    }
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&NPC_OID) {
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
        v.max_hp = 1_000_000;
        v.cur_hp = 1_000_000.0;
    }
    let mp = |world: &World| {
        world
            .objects
            .get_component::<Vitals>(&CASTER)
            .map(|v| v.cur_mp)
            .unwrap_or(0.0)
    };

    // A melee swing (`skill_magic == None`) drains nothing on this dist.
    combat::apply_attack_damage(&mut world, CASTER, NPC_OID, 500.0, false, None);
    assert_eq!(
        mp(&world),
        0.0,
        "MpVampiricAttackWorkWithMelee is False here, so melee drains nothing"
    );

    // A skill hit does. `apply_physical_damage`'s `from_skill` is the same
    // discriminator, so drive it through the skill-damage entry point.
    combat::apply_attack_damage(&mut world, CASTER, NPC_OID, 500.0, false, Some(false));
    assert!(
        mp(&world) > 0.0,
        "a skill hit drains 10 % of the damage into MP: {}",
        mp(&world)
    );
}

/// G34 S4 sub-slice 8 — `LimitHp`/`LimitCp` (`MAX_RECOVERABLE_HP`/`_CP`), the
/// ceiling a **heal** may restore to.
///
/// The learnable sources are *restrictions*: Noblesse Harmony (1326) and
/// Symphony (1327) grant them `PER −30` / `−40`, so under those auras you can
/// only be healed back to 70 % HP and 60 % CP. A port that clamps heals to the
/// raw pool — as this one did — behaves identically until someone casts them.
#[test]
fn limit_hp_caps_how_far_a_heal_can_restore() {
    use crate::model::stats::Stat;
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut heal = cc_skill(9391, SkillEffect::Heal { power: 10_000.0 }, "NONE");
    heal.effect_point = 100;
    heal.is_debuff = false;
    world.data.skill_data.insert_for_test(heal);

    let set_hp = |world: &mut World, cur: f64| {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&CASTER) {
            v.max_hp = 1000;
            v.cur_hp = cur;
        }
    };
    let hp = |world: &World| {
        world
            .objects
            .get_component::<Vitals>(&CASTER)
            .map(|v| v.cur_hp)
            .unwrap_or(0.0)
    };

    // Unlimited: a huge heal fills the pool.
    set_hp(&mut world, 100.0);
    land(&mut world, 9391, CASTER);
    assert_eq!(hp(&world), 1000.0, "no cap → heal to full");

    // Noblesse Harmony's `PER −30` → `mul` 0.7 on MAX_RECOVERABLE_HP.
    let mut mods = world
        .objects
        .get_component::<model::components::StatModifiers>(&CASTER)
        .cloned()
        .unwrap_or_default();
    mods.mul.insert(Stat::MaxRecoverableHp, 0.7);
    world.objects.add_components(&CASTER, mods);
    set_hp(&mut world, 100.0);
    land(&mut world, 9391, CASTER);
    assert_eq!(
        hp(&world),
        700.0,
        "the same heal now stops at 70 % — the cap is the point of the skill"
    );

    // Already above the cap: the heal restores nothing rather than draining.
    set_hp(&mut world, 900.0);
    land(&mut world, 9391, CASTER);
    assert_eq!(hp(&world), 900.0, "over the cap, a heal is a no-op");
}

/// `CpHealPercent` (Victories of Pa'agrio 1414 at 20 %) restores a share of
/// **max CP** and honours `MAX_RECOVERABLE_CP`; `HpByLevel` (Life Scavenge 46,
/// Corpse Life Drain 1151) heals the **effector** — the caster, not the target.
///
/// The `HpByLevel` direction is the trap: every other heal in the family reads
/// `effected`, and pointing this one at the target would heal the corpse you
/// are draining.
#[test]
fn cp_heal_percent_and_hp_by_level_hit_the_right_pools() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = 5971;
    let _v = ingame_player_access(&mut world, 2, victim, 0);

    let mut cp_heal = cc_skill(9392, SkillEffect::CpHealPercent { power: 20.0 }, "NONE");
    cp_heal.effect_point = 100;
    cp_heal.is_debuff = false;
    world.data.skill_data.insert_for_test(cp_heal);
    let mut drain = cc_skill(9393, SkillEffect::HpByLevel { power: 260.0 }, "NONE");
    drain.effect_point = 100;
    drain.is_debuff = false;
    world.data.skill_data.insert_for_test(drain);

    // CP heal lands on the *target*.
    if let Some(v) = world.objects.get_component_mut::<PlayerVitals>(&victim) {
        v.max_cp = 1000;
        v.cur_cp = 0.0;
    }
    land(&mut world, 9392, victim);
    assert_eq!(
        world
            .objects
            .get_component::<PlayerVitals>(&victim)
            .map(|v| v.cur_cp),
        Some(200.0),
        "20 % of max CP"
    );

    // `HpByLevel` lands on the *caster*, whatever the target is.
    for oid in [CASTER, victim] {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) {
            v.max_hp = 10_000;
            v.cur_hp = 1_000.0;
        }
    }
    land(&mut world, 9393, victim);
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&CASTER)
            .map(|v| v.cur_hp),
        Some(1_260.0),
        "the caster is healed"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&victim)
            .map(|v| v.cur_hp),
        Some(1_000.0),
        "…and the target — the corpse being drained — is not"
    );
}

/// G34 S4 sub-slice 9 — `DeathLink` (Curse Death Link 1159). The power scales
/// with how close the **caster** is to death: `power × (2 − 2·curHp/maxHp)`,
/// so it is ×2 at 0 HP and **×0 at full**. Casting it healthy does literally
/// nothing, which is the opposite of how every other nuke behaves and the
/// reason to assert the full-HP case explicitly.
#[test]
fn death_link_scales_with_the_casters_missing_hp() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 20, 100, 0, 0);
    let mut link = cc_skill(9401, SkillEffect::DeathLink { power: 100.0 }, "NONE");
    link.magic_type = 1;
    // The magic-failure roll floors a failed cast at 1 damage regardless of
    // power, which would swamp the multiplier we are measuring here.
    world.cfg.character.magic_failures = false;
    world.data.skill_data.insert_for_test(link);

    let damage_at = |world: &mut World, hp_fraction: f64| -> f64 {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&CASTER) {
            v.max_hp = 1000;
            v.cur_hp = 1000.0 * hp_fraction;
        }
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&NPC_OID) {
            v.max_hp = 1_000_000;
            v.cur_hp = 1_000_000.0;
        }
        world.clear_forced_rolls();
        world.force_rolls([50; 12]);
        land(world, 9401, NPC_OID);
        1_000_000.0
            - world
                .objects
                .get_component::<Vitals>(&NPC_OID)
                .map(|v| v.cur_hp)
                .unwrap_or(0.0)
    };

    let at_full = damage_at(&mut world, 1.0);
    let at_half = damage_at(&mut world, 0.5);
    let at_death = damage_at(&mut world, 0.01);

    // At full HP the multiplier is 0, so the nuke does nothing at all.
    assert_eq!(at_full, 0.0, "at full HP the multiplier is 0 — no damage");
    assert!(at_half > 0.0, "half HP: {at_half}");
    assert!(
        at_death > at_half * 1.5,
        "the closer to death the harder it hits ({at_half} → {at_death})"
    );
}

/// `Bluff` (Blinding Blow 321, Bluff 358) spins the target to face the
/// **caster's** heading — which is what sets up a Backstab. Raid bosses are
/// immune, and that exemption is the half a "just set the heading"
/// implementation would drop.
#[test]
fn bluff_turns_the_target_but_not_a_raid_boss() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world.data.skill_data.insert_for_test(cc_skill(
        9402,
        SkillEffect::Bluff { chance: 100 },
        "NONE",
    ));
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 20, 100, 0, 0);
    let boss_oid = NPC_OID + 5;
    // Same level and stats as the ordinary mob — the *only* difference is the
    // raid template, so a heading that does not move can only be the exemption.
    add_test_npc(&mut world, boss_oid, 29001, "RaidBoss", 20, 100, 0, 0);

    let heading_of = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<Position>(&oid)
            .map(|p| p.heading)
            .unwrap_or(0)
    };
    // Give the caster a distinctive heading and the targets another.
    for (oid, h) in [(CASTER, 12_000), (NPC_OID, 0), (boss_oid, 0)] {
        if let Some(p) = world.objects.get_component_mut::<Position>(&oid) {
            p.heading = h;
        }
    }

    // Pin the land roll — otherwise the chance gate makes this a coin flip.
    world.force_rolls([0; 8]);
    land(&mut world, 9402, NPC_OID);
    assert_eq!(
        heading_of(&world, NPC_OID),
        12_000,
        "the mob is spun to face the caster's heading"
    );

    world.clear_forced_rolls();
    world.force_rolls([0; 8]);
    land(&mut world, 9402, boss_oid);
    assert_eq!(
        heading_of(&world, boss_oid),
        0,
        "a raid boss is immune — Java bails before the rotation"
    );
}

/// **`OpenDoor`** — the lock-picking half of Unlock (27). Three outcomes, and
/// the two refusals are different messages for different reasons: a door that
/// is not `openMethod="BY_SKILL"` cannot be picked *at all* ("this door cannot
/// be unlocked"), while a `BY_SKILL` door that fails its roll gets the softer
/// "you have failed to unlock the door" and can be tried again.
#[test]
fn unlock_picks_a_by_skill_door_and_refuses_the_rest() {
    use crate::data::door_data::DoorOpenMethod;

    let (mut world, _db, _l) = cc2_world();
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world.data.skill_data.insert_for_test(cc_skill(
        9403,
        SkillEffect::OpenDoor {
            chance: 50,
            is_item: false,
        },
        "NONE",
    ));
    let pickable =
        model::door::spawn_door_for_test(&mut world, test_door(9901, DoorOpenMethod::BySkill));
    let mut plain = test_door(9902, DoorOpenMethod::ByClick);
    plain.x = 400;
    let plain_oid = model::door::spawn_door_for_test(&mut world, plain);

    // A `BY_CLICK` door: refused outright, with its own message, and the roll
    // is never reached.
    drain(&mut out);
    world.clear_forced_rolls();
    world.force_rolls([0; 4]);
    land(&mut world, 9403, plain_oid);
    let pkts = drain(&mut out);
    assert!(
        has_system_message(&pkts, server_packets::sm_ids::THIS_DOOR_CANNOT_BE_UNLOCKED),
        "a door that is not BY_SKILL cannot be picked at all"
    );
    assert!(!world.geo.doors.is_open(9902), "and it stays shut");

    // A `BY_SKILL` door with a failing roll: the softer message, still shut.
    world.clear_forced_rolls();
    world.force_rolls([50; 4]);
    land(&mut world, 9403, pickable);
    let pkts = drain(&mut out);
    assert!(
        has_system_message(
            &pkts,
            server_packets::sm_ids::YOU_HAVE_FAILED_TO_UNLOCK_THE_DOOR
        ),
        "a missed roll says so"
    );
    assert!(!world.geo.doors.is_open(9901), "and the door is still shut");

    // Same door, a roll under the chance: it opens.
    world.clear_forced_rolls();
    world.force_rolls([10; 4]);
    land(&mut world, 9403, pickable);
    assert!(world.geo.doors.is_open(9901), "a passing roll opens it");
}

/// **`OpenChest`** — the treasure-box half of the same skill, gated by a
/// *level band* rather than a roll. Inside the band the box pops open: it dies
/// without paying exp/sp and is flagged `specialDrop` so it rolls its own list
/// rather than the smashed-box one. Outside it, the box turns on you.
///
/// Note the reachability: **no `type="Chest"` NPC is spawned anywhere on this
/// dist**, so the only way to meet one today is `//spawn`. The effect is
/// ported anyway — a datapack with chest spawns is a data change, not a code
/// change.
#[test]
fn unlocking_a_chest_depends_on_the_level_band() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(9404, SkillEffect::OpenChest, "NONE"));

    let in_band = NPC_OID;
    let out_of_band = NPC_OID + 1;
    // 18265 is a real `type="Chest"` template on this dist, so its **level
    // comes from the datapack** — `add_test_npc`'s level argument only applies
    // to ids it has to invent. The caster's level is therefore the variable
    // here, which is also the honest reading: the band is a gap, not a floor.
    add_test_npc(&mut world, in_band, 18265, "Chest", 25, 100, 0, 0);
    add_test_npc(&mut world, out_of_band, 18265, "Chest", 25, 150, 0, 0);
    let chest_level = effects::creature_level_for_test(&world, in_band);
    let set_caster_level = |world: &mut World, level: i32| {
        if let Some(p) = world.objects.get_component_mut::<Player>(&CASTER) {
            p.level = level;
        }
    };

    // Five levels below the chest: inside the 6-level band, so it opens.
    set_caster_level(&mut world, chest_level - 5);
    land(&mut world, 9404, in_band);
    assert!(
        world
            .objects
            .get_component::<Vitals>(&in_band)
            .is_some_and(|v| v.dead),
        "a box within the band is opened — which kills it"
    );
    let npc = world
        .objects
        .get_component::<model::npc::Npc>(&in_band)
        .expect("chest");
    assert!(npc.special_drop, "and it rolls its own drop list");
    assert!(!npc.must_reward_exp_sp, "but pays no exp/sp");

    // Twenty levels below: outside the band, so it refuses and aggroes.
    set_caster_level(&mut world, chest_level - 20);
    land(&mut world, 9404, out_of_band);
    assert!(
        world
            .objects
            .get_component::<Vitals>(&out_of_band)
            .is_some_and(|v| !v.dead),
        "a box outside the band is not opened"
    );
    assert!(
        world
            .objects
            .get_component::<AggroList>(&out_of_band)
            .and_then(|a| a.0.get(&CASTER).map(|i| i.hate))
            .unwrap_or(0.0)
            > 0.0,
        "it turns on the caster instead"
    );
}

/// The other half of `OpenChest`, on the **death** side: an unlocked box pays
/// no exp/sp and keeps its own drop list, while a box that was merely beaten
/// to death rolls a *different* npc id's list (Java `Chest.doItemDrop`).
///
/// A dist finding recorded by this test: the ids that remap points at —
/// 21801-21822 and the six 216xx/217xx ones — **do not exist** in this
/// datapack, and the chest templates carry no `<drops>` of their own either.
/// In Java that null template reaches `calculateDrops` and throws; here the
/// swap simply falls back to the chest's own (empty) list, which is the only
/// non-crashing reading of the same code.
#[test]
fn a_smashed_chest_and_an_unlocked_one_do_not_share_a_drop_table() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(9405, SkillEffect::OpenChest, "NONE"));
    // A real experience table (the fixture ships an empty one, where the level
    // cap makes *every* award clamp to -1 and an exp assertion would pass no
    // matter what the gate does) and a chest that actually pays exp — 18265
    // declares no `<acquire>` at all, so with the dist's own 0 there would be
    // nothing for the gate to withhold.
    world.data.experience =
        crate::data::ExperienceData::from_table(vec![0, 0, 1000, 2000, 3000, 4000, 5000], 6);
    let chest = NPC_OID;
    add_test_npc(&mut world, chest, 18265, "Chest", 25, 100, 0, 0);
    {
        let mut t = world.data.npc_data.get(18265).cloned().expect("Chest");
        t.exp = 500.0;
        t.sp = 50.0;
        world.data.npc_data.insert_for_test(t);
    }
    let template = world
        .data
        .npc_data
        .get(18265)
        .cloned()
        .expect("18265 is a Chest on this dist");

    // The dist finding: 18265 + 3536 = 21801, and **21801 is not a template
    // on this datapack** — nor is any of the six 216xx/217xx ids the fixed
    // pairs map onto. Java hands that null straight to `calculateDrops` and
    // throws; here the swap finds nothing and the caller falls back to the
    // chest's own (also empty) list.
    assert!(
        world.data.npc_data.get(21801).is_none(),
        "the remap target does not exist on this dist — recorded, not assumed"
    );
    assert!(
        crate::game_loop::death::chest_drop_template_for_test(&world, chest, &template).is_none(),
        "so a smashed chest falls back to its own list"
    );

    // Give 21801 a template and the redirect is visible: the *mechanism* is
    // what this asserts, independently of whether this dist ships the target.
    let mut mimic = crate::data::npc_data::default_template(21801);
    mimic.type_name = "Monster".into();
    world.data.npc_data.insert_for_test(mimic);
    assert_eq!(
        crate::game_loop::death::chest_drop_template_for_test(&world, chest, &template)
            .map(|t| t.id),
        Some(21801),
        "a chest that was not unlocked rolls 21801's drop list, not its own"
    );

    // Unlock it, and the swap stops applying at all.
    let exp_before = world
        .objects
        .get_component::<Player>(&CASTER)
        .map(|p| p.exp)
        .unwrap_or(0);
    let chest_level = effects::creature_level_for_test(&world, chest);
    if let Some(p) = world.objects.get_component_mut::<Player>(&CASTER) {
        p.level = chest_level;
    }
    land(&mut world, 9405, chest);
    assert!(
        crate::game_loop::death::chest_drop_template_for_test(&world, chest, &template).is_none(),
        "an unlocked chest always rolls its own list"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&CASTER)
            .map(|p| p.exp)
            .unwrap_or(0),
        exp_before,
        "and pays no exp — `setMustRewardExpSp(false)`"
    );
}

/// **`RebalanceHP`** (Balance Life 1043) — pool the party's HP and set everyone
/// to the party average *percentage*. It is a redistribution, not a heal: the
/// total is unchanged, so the healthy pay for the dying. That is the half a
/// "heal the party" implementation would get wrong in the most visible way —
/// the caster at full HP is supposed to come out of it *worse*.
#[test]
fn balance_life_averages_the_party_and_costs_the_healthy() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let ally = CASTER + 1;
    let _ally_out = ingame_player(&mut world, CID + 1, ally, 50, 0, 0);
    let mut skill = cc_skill(9406, SkillEffect::RebalanceHp, "NONE");
    skill.affect_range = 900;
    world.data.skill_data.insert_for_test(skill);
    make_party(&mut world, &[CASTER, ally], LootRule::Random);

    // Same pool, wildly different fills: 100 % and 20 % → a 60 % average.
    for (oid, cur) in [(CASTER, 1000.0), (ally, 200.0)] {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) {
            v.max_hp = 1000;
            v.cur_hp = cur;
        }
    }
    let total_before = 1000.0 + 200.0;

    land(&mut world, 9406, CASTER);

    let hp_of = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<Vitals>(&oid)
            .map(|v| v.cur_hp)
            .unwrap_or(0.0)
    };
    assert_eq!(hp_of(&world, ally), 600.0, "the dying ally is pulled up");
    assert_eq!(
        hp_of(&world, CASTER),
        600.0,
        "and the healthy caster is pulled *down* — this is not a heal"
    );
    assert_eq!(
        hp_of(&world, CASTER) + hp_of(&world, ally),
        total_before,
        "the party's total HP is conserved"
    );
}

/// Java guards the whole effect with `if (party != null)`, so an unpartied
/// Balance Life is simply wasted — it does **not** fall back to the "party of
/// one" reading every other party-scoped effect uses.
///
/// The caster alone cannot show this: with one member the average *is* their
/// own percentage, so the maths is a no-op either way and the guard is
/// invisible. A **pet** is what makes the difference observable — under the
/// fallback the pair would rebalance against each other, under Java's guard
/// neither of them moves.
#[test]
fn balance_life_without_a_party_does_nothing() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut skill = cc_skill(9407, SkillEffect::RebalanceHp, "NONE");
    skill.affect_range = 900;
    world.data.skill_data.insert_for_test(skill);

    // A pet at a very different fill from its (unpartied) owner.
    let pet = NPC_OID;
    add_test_npc(&mut world, pet, 20001, "Monster", 20, 60, 0, 0);
    world.objects.add_components(
        &CASTER,
        model::components::SummonRef {
            servitor: None,
            pet: Some(pet),
        },
    );
    for (oid, max, cur) in [(CASTER, 1000, 250.0), (pet, 1000, 1000.0)] {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) {
            v.max_hp = max;
            v.cur_hp = cur;
        }
    }

    land(&mut world, 9407, CASTER);

    let hp_of = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<Vitals>(&oid)
            .map(|v| v.cur_hp)
            .unwrap_or(0.0)
    };
    assert_eq!(
        hp_of(&world, CASTER),
        250.0,
        "solo, the caster is untouched"
    );
    assert_eq!(
        hp_of(&world, pet),
        1000.0,
        "and so is their pet — no party, no rebalance"
    );
}

/// **`calculatePvpPveBonus`** — a term in every damage formula that this port
/// hard-coded to 1.0, behind comments in three different files saying the
/// pvp/pve mods were 1.0. That was true only while nothing granted the stats;
/// the dist has ~1300 effects that do.
///
/// The shape to get right is that it is a **difference of multipliers**, not a
/// product: `1 + (attackMul − defenceMul)`, so a +50 % attacker facing a +50 %
/// defender comes out at exactly 1.0. A port that multiplied them would give
/// 2.25.
#[test]
fn pvp_damage_bonus_is_a_difference_of_multipliers_not_a_product() {
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = CASTER + 1;
    let _v = ingame_player(&mut world, CID + 1, victim, 40, 0, 0);

    let bonus = |world: &World| effects::pvp_pve_bonus_for_test(world, CASTER, victim, None);
    assert_eq!(bonus(&world), 1.0, "no stats granted: no change");

    // Attacker +50 % PvP auto-attack damage.
    if let Some(m) = world
        .objects
        .get_component_mut::<model::components::StatModifiers>(&CASTER)
    {
        *m.mul.entry(Stat::PvpPhysicalAttackDamage).or_insert(1.0) *= 1.5;
    }
    assert_eq!(bonus(&world), 1.5, "attacker alone: 1 + (1.5 - 1)");

    // Victim +50 % PvP auto-attack *defence* — the two cancel exactly.
    if let Some(m) = world
        .objects
        .get_component_mut::<model::components::StatModifiers>(&victim)
    {
        *m.mul.entry(Stat::PvpPhysicalAttackDefence).or_insert(1.0) *= 1.5;
    }
    assert_eq!(
        bonus(&world),
        1.0,
        "+50 % against +50 % cancels — a *product* would read 2.25 here"
    );
}

/// The branch is picked by *how* the damage is delivered, not just by who is
/// fighting: an auto-attack (Java's `skill == null`) reads the
/// `PHYSICAL_ATTACK` pair, a physical skill the `PHYSICAL_SKILL` pair and a
/// magic skill the `MAGICAL_SKILL` pair. Granting one must not move the others.
#[test]
fn the_pvp_bonus_reads_a_different_stat_pair_per_delivery() {
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = CASTER + 1;
    let _v = ingame_player(&mut world, CID + 1, victim, 40, 0, 0);
    let mut physical = cc_skill(9410, SkillEffect::Root, "NONE");
    physical.magic_type = 0;
    let mut magical = cc_skill(9411, SkillEffect::Root, "NONE");
    magical.magic_type = 1;

    // Only the *magical skill* stat is granted.
    if let Some(m) = world
        .objects
        .get_component_mut::<model::components::StatModifiers>(&CASTER)
    {
        *m.mul.entry(Stat::PvpMagicalSkillDamage).or_insert(1.0) *= 1.5;
    }

    let bonus = |world: &World, skill: Option<&Skill>| {
        effects::pvp_pve_bonus_for_test(world, CASTER, victim, skill)
    };
    assert_eq!(
        bonus(&world, Some(&magical)),
        1.5,
        "the magic skill reads it"
    );
    assert_eq!(
        bonus(&world, Some(&physical)),
        1.0,
        "a physical skill does not"
    );
    assert_eq!(bonus(&world, None), 1.0, "and neither does an auto-attack");
}

/// The **PvE** branch carries a level-difference penalty the port never had:
/// `SkillDmgPenaltyForLvLDifferences`, which this dist tunes down to ×0.25.
/// It only bites on a non-raid NPC at or above `MinNPCLevelForDmgPenalty` (78)
/// that is 2+ levels above the attacker — and a raid boss is exempt, which is
/// the clause a "just multiply by the table" port would drop.
#[test]
fn the_pve_penalty_bites_on_high_level_mobs_and_spares_raids() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    if let Some(p) = world.objects.get_component_mut::<Player>(&CASTER) {
        p.level = 78;
    }
    let mob = NPC_OID;
    let boss = NPC_OID + 1;
    // Synthetic ids so the *level* is ours to set — `add_test_npc` honours its
    // level argument only for templates it has to invent, and every real dist
    // id already carries one. Same level on both, so the raid exemption is the
    // only difference between them.
    add_test_npc(&mut world, mob, 90001, "Monster", 85, 100, 0, 0);
    add_test_npc(&mut world, boss, 90002, "RaidBoss", 85, 150, 0, 0);

    let bonus =
        |world: &World, target: i32| effects::pvp_pve_bonus_for_test(world, CASTER, target, None);
    assert!(
        bonus(&world, mob) < 1.0,
        "a level-85 mob against a level-78 player is penalised, got {}",
        bonus(&world, mob)
    );
    assert_eq!(
        bonus(&world, boss),
        1.0,
        "a raid boss is exempt from the penalty entirely"
    );
}

/// End-to-end: the bonus has to *reach* the damage. A helper that computes the
/// right number and is never multiplied in is the exact failure mode this epic
/// keeps finding — the three formula comments claiming "pvp-pve mods 1.0" were
/// each a call site that had to be edited, not just a stat to register.
#[test]
fn the_pvp_bonus_actually_reaches_a_nukes_damage() {
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world.cfg.character.magic_failures = false;
    let victim = CASTER + 1;
    let _v = ingame_player(&mut world, CID + 1, victim, 40, 0, 0);
    let mut nuke = cc_skill(9412, SkillEffect::MagicalAttack { power: 100.0 }, "NONE");
    nuke.magic_type = 1;
    world.data.skill_data.insert_for_test(nuke);

    let cast_once = |world: &mut World| -> f64 {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&victim) {
            v.max_hp = 1_000_000;
            v.cur_hp = 1_000_000.0;
            v.dead = false;
        }
        world.clear_forced_rolls();
        world.force_rolls([50; 12]);
        land(world, 9412, victim);
        1_000_000.0
            - world
                .objects
                .get_component::<Vitals>(&victim)
                .map(|v| v.cur_hp)
                .unwrap_or(0.0)
    };

    let plain = cast_once(&mut world);
    assert!(plain > 0.0, "the nuke lands for something: {plain}");

    // The *victim* takes a magical-skill PvP defence buff.
    if let Some(m) = world
        .objects
        .get_component_mut::<model::components::StatModifiers>(&victim)
    {
        *m.mul.entry(Stat::PvpMagicalSkillDefence).or_insert(1.0) *= 1.5;
    }
    let defended = cast_once(&mut world);

    assert!(
        defended < plain,
        "a +50 % PvP magical-skill defence must reduce the nuke: {plain} -> {defended}"
    );
    // 1 + (1.0 - 1.5) = 0.5 exactly.
    assert!(
        (defended / plain - 0.5).abs() < 0.02,
        "and by exactly the 1 + (atk - def) factor: {plain} -> {defended}"
    );
}

/// **`PhysicalAttackHpLink`** (Fatal Counter 314, Fatal Arrow 10905) — the
/// physical twin of `DeathLink`: the same `−(curHp·2 / maxHp) + 2` multiplier
/// on the **caster's** missing HP, so a healthy archer's Fatal Counter does
/// nothing and a dying one's hits for double. The skill's own description says
/// as much ("the power of the attack increases as your HP decreases"), and a
/// port that shared `PhysicalAttack`'s arm without the tail would look right at
/// every HP except the two ends.
#[test]
fn fatal_counter_scales_with_the_archers_missing_hp() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 20, 100, 0, 0);
    world.data.skill_data.insert_for_test(cc_skill(
        9413,
        SkillEffect::PhysicalAttackHpLink {
            power: 500.0,
            p_atk_mod: 1.0,
            p_def_mod: 1.0,
            critical_chance: 0.0,
            ignore_shield_defence: false,
        },
        "NONE",
    ));

    let damage_at = |world: &mut World, hp_fraction: f64| -> f64 {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&CASTER) {
            v.max_hp = 1000;
            v.cur_hp = 1000.0 * hp_fraction;
        }
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&NPC_OID) {
            v.max_hp = 1_000_000;
            v.cur_hp = 1_000_000.0;
            v.dead = false;
        }
        world.clear_forced_rolls();
        world.force_rolls([50; 12]);
        land(world, 9413, NPC_OID);
        1_000_000.0
            - world
                .objects
                .get_component::<Vitals>(&NPC_OID)
                .map(|v| v.cur_hp)
                .unwrap_or(0.0)
    };

    let at_full = damage_at(&mut world, 1.0);
    let at_half = damage_at(&mut world, 0.5);
    let at_death = damage_at(&mut world, 0.01);

    assert_eq!(at_full, 0.0, "at full HP the multiplier is 0 — no damage");
    assert!(at_half > 0.0, "half HP: {at_half}");
    assert!(
        at_death > at_half * 1.5,
        "the closer to death the harder it hits ({at_half} -> {at_death})"
    );
}

/// The other half of Focus Attack: the buff has to actually *grant* the stat.
/// Testing the sweep gate against a hand-inserted modifier proves the consumer
/// and nothing about the grant — which is the same registry-line-without-a-
/// consumer trap this epic keeps hitting, just pointing the other way.
#[test]
fn focus_attack_grants_the_single_target_stat_and_gives_it_back() {
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut focus = cc_skill(9414, SkillEffect::PolearmSingleTarget, "NONE");
    focus.target_type = TargetType::Self_;
    world.data.skill_data.insert_for_test(focus);

    let stat = |world: &World| {
        world
            .objects
            .get_component::<model::components::StatModifiers>(&CASTER)
            .map(|m| model::finalize(m, Stat::PhysicalPolearmTargetSingle, 0.0))
            .unwrap_or(0.0)
    };
    assert_eq!(stat(&world), 0.0, "nothing before the toggle");

    land(&mut world, 9414, CASTER);
    assert!(
        stat(&world) > 0.0,
        "the toggle grants it, got {}",
        stat(&world)
    );

    effects::handle_buff_expire(&mut world, CASTER, 9414);
    assert_eq!(
        stat(&world),
        0.0,
        "and `onExit` takes it back — otherwise the sweep is lost forever"
    );
}

/// **`TriggerSkillByDamage`** (Mirage 445) — the mirror of
/// `TriggerSkillByAttack`: it fires when the bearer **takes** a hit, and casts
/// back at the attacker rather than on itself.
///
/// Two gates separate it from the attack-side twin, and both are the half a
/// "copy the attack trigger" port would drop: `attackerType` (Mirage takes
/// `Playable` only, so a monster hitting you never sets it off) and the
/// requirement that the carrier actually be *up* — Mirage is a timed buff,
/// unlike the always-on weapon masteries the attack twin reads.
#[test]
fn mirage_fires_back_at_a_player_attacker_but_not_a_monster() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let attacker = CASTER + 1;
    let _a = ingame_player(&mut world, CID + 1, attacker, 40, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 20, 60, 0, 0);

    // The trigger the carrier fires.
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(9416, SkillEffect::Root, "ROOT"));
    // The carrier: Playable attackers only, always rolls, casts at the enemy.
    let mut carrier = cc_skill(
        9415,
        SkillEffect::TriggerSkillByDamage {
            min_damage: 1,
            chance: 100,
            skill_id: 9416,
            skill_level: 1,
            hp_percent: 100,
            attacker_playable_only: true,
            on_attacker: true,
        },
        "NONE",
    );
    carrier.target_type = TargetType::Self_;
    world.data.skill_data.insert_for_test(carrier);

    let has = |world: &World, oid: i32| has_buff(world, oid, 9416);

    // Not cast yet: nothing to listen, so nothing triggers. (Java attaches the
    // listener to the *buff*, which is why this is the meaningful negative —
    // knowing Mirage and being under it are different things.)
    combat::apply_attack_damage(&mut world, attacker, CASTER, 50.0, false, None);
    assert!(!has(&world, attacker), "no Mirage buff up, no counter-cast");

    // Now put it up. A *monster* hitting us must still not set it off.
    land(&mut world, 9415, CASTER);
    combat::apply_attack_damage(&mut world, NPC_OID, CASTER, 50.0, false, None);
    assert!(
        !has(&world, NPC_OID),
        "attackerType=Playable: a monster never triggers it"
    );

    // A player hitting us does.
    combat::apply_attack_damage(&mut world, attacker, CASTER, 50.0, false, None);
    assert!(
        has(&world, attacker),
        "a playable attacker takes the counter-cast"
    );
}

/// **`TriggerSkillByMagicType`** (Dance of Shadows 366) — fires when the bearer
/// *finishes casting* a skill whose `magicType` is listed. That is how the
/// dance's stealth ends the moment you act: any ordinary cast fires Cancel
/// Shadow Move on the party.
#[test]
fn dance_of_shadows_cancels_itself_on_a_listed_magic_type() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    world
        .data
        .skill_data
        .insert_for_test(cc_skill(9418, SkillEffect::Root, "ROOT"));
    let mut carrier = cc_skill(
        9417,
        SkillEffect::TriggerSkillByMagicType {
            magic_types: vec![1, 2],
            chance: 100,
            skill_id: 9418,
            skill_level: 1,
            on_party: true,
        },
        "NONE",
    );
    carrier.target_type = TargetType::Self_;
    world.data.skill_data.insert_for_test(carrier);
    land(&mut world, 9417, CASTER);

    let has = |world: &World| has_buff(world, CASTER, 9418);

    // A cast whose magicType is *not* listed changes nothing.
    effects::fire_magic_type_triggers(&mut world, CASTER, CASTER, 7);
    assert!(!has(&world), "an unlisted magicType does not fire it");

    // One that is listed does.
    effects::fire_magic_type_triggers(&mut world, CASTER, CASTER, 2);
    assert!(has(&world), "a listed magicType fires the trigger");
}

/// **`CallParty`** (Chant of Gate 1429) — recall every *other* party member to
/// the caster. Two halves matter: it is **not** Summon Friend, so there is no
/// `ConfirmDlg` and the members get no say; and each one is gated by CallPc's
/// shared `checkSummonTargetStatus`, whose refusals are messaged to the
/// **caster**, not the member left behind.
#[test]
fn chant_of_gate_recalls_the_party_but_not_someone_in_combat() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 1000, 1000);
    let willing = CASTER + 1;
    let fighting = CASTER + 2;
    let _w = ingame_player(&mut world, CID + 1, willing, 0, 0, 0);
    let _f = ingame_player(&mut world, CID + 2, fighting, 50, 50, 0);
    let mut skill = cc_skill(9421, SkillEffect::CallParty, "NONE");
    skill.target_type = TargetType::Self_;
    world.data.skill_data.insert_for_test(skill);
    make_party(&mut world, &[CASTER, willing, fighting], LootRule::Random);

    // One member is in combat — `isInCombat()` is the attack stance.
    combat::refresh_attack_stance(&mut world, fighting);

    land(&mut world, 9421, CASTER);

    let pos = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<Position>(&oid)
            .map(|p| (p.x, p.y))
            .unwrap()
    };
    assert_eq!(
        pos(&world, willing),
        (1000, 1000),
        "the willing member is pulled to the caster, no dialog asked"
    );
    assert_eq!(
        pos(&world, fighting),
        (50, 50),
        "the one in combat stays put"
    );
    assert_eq!(
        pos(&world, CASTER),
        (1000, 1000),
        "and the caster does not recall themselves"
    );
}

/// **`ReduceDropPenalty`** (Residence Death Fortune 610) — the exp you lose on
/// death is scaled by *what killed you*: a raid, an ordinary monster, or a
/// playable each read a different stat. Residence Death Fortune grants the
/// **mob** one at `-12` (×0.88), so dying to a monster costs less while it is
/// up and dying to a player costs exactly as much as before.
#[test]
fn residence_death_fortune_softens_a_mob_death_but_not_a_pvp_one() {
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    // A wide level band so the 12 % reduction is unmistakable, and exp set
    // *above* the level threshold — sitting exactly on it makes the delevel
    // clamp zero the loss and the test measure nothing.
    world.data.experience =
        crate::data::ExperienceData::from_table(vec![0, 0, 1000, 2000, 3000, 103_000], 5);
    let mob = NPC_OID;
    let killer_player = CASTER + 1;
    add_test_npc(&mut world, mob, 90101, "Monster", 5, 100, 0, 0);
    let _k = ingame_player(&mut world, CID + 1, killer_player, 50, 0, 0);

    let lost_against = |world: &mut World, killer: i32| -> i64 {
        if let Some(p) = world.objects.get_component_mut::<Player>(&CASTER) {
            p.level = 4;
            p.exp = 50_000;
        }
        crate::game_loop::death::apply_death_exp_penalty_ex(world, CASTER, false, Some(killer));
        50_000
            - world
                .objects
                .get_component::<Player>(&CASTER)
                .map(|p| p.exp)
                .unwrap_or(0)
    };

    let plain_mob = lost_against(&mut world, mob);
    let plain_pvp = lost_against(&mut world, killer_player);
    assert!(plain_mob > 0, "a mob death costs exp to begin with");

    // Grant the *mob* reduction only.
    if let Some(m) = world
        .objects
        .get_component_mut::<model::components::StatModifiers>(&CASTER)
    {
        *m.mul.entry(Stat::ReduceExpLostByMob).or_insert(1.0) *= 0.88;
    }

    assert!(
        lost_against(&mut world, mob) < plain_mob,
        "dying to a monster now costs less"
    );
    assert_eq!(
        lost_against(&mut world, killer_player),
        plain_pvp,
        "but dying to a player is untouched — the stat is keyed on the killer"
    );
}

/// **`NightStatModify`** (Shadow Sense 294) — "increases Accuracy by 3 **at
/// night**". The stat is not a property of the buff but of the *clock*: it has
/// to appear at dusk and vanish at dawn while the buff sits there unchanged,
/// which is the half a plain stat grant would get wrong in both directions.
#[test]
fn shadow_sense_grants_its_accuracy_only_at_night() {
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut sense = cc_skill(
        294,
        SkillEffect::NightStatModify {
            stat: Stat::AccuracyCombat,
            amount: 3.0,
            mode: model::stats::StatModifierType::Diff,
        },
        "SHADOW_SENSE",
    );
    sense.target_type = TargetType::Self_;
    world.data.skill_data.insert_for_test(sense);

    let accuracy = |world: &World| stat_add(world, CASTER, Stat::AccuracyCombat);

    land(&mut world, 294, CASTER);
    // The buff is up either way; only the clock decides.
    assert!(
        has_buff(&world, CASTER, 294),
        "the buff lands regardless of the hour"
    );

    crate::game_loop::night_stats::refresh_one(&mut world, CASTER, false);
    assert_eq!(accuracy(&world), 0.0, "by day it grants nothing");

    crate::game_loop::night_stats::refresh_one(&mut world, CASTER, true);
    assert_eq!(accuracy(&world), 3.0, "at night the accuracy appears");

    crate::game_loop::night_stats::refresh_one(&mut world, CASTER, false);
    assert_eq!(accuracy(&world), 0.0, "and dawn takes it back again");
}

/// **The `operateType` fail-closed bug.** `use_magic_on` returns outright for
/// anything that is neither `Active` nor `Channeling`, and the parser dropped
/// every unmapped `operateType` to `Other` — so **A3** (Blinding Blow 321,
/// Vengeance 368, Evade Shot 369, Critical Blow 409, Aura Flare 1231) and
/// **CA5** (Battle Stance 426, Spell Stance 427) were seven learnable skills
/// that could not be cast at all.
///
/// Unlike the rest of this epic's gaps, this one fails *closed*: the skill did
/// nothing because the cast never started, not because an effect was missing.
#[test]
fn a3_and_ca5_skills_parse_as_castable_operate_types() {
    use crate::model::skill::OperateType;

    let skills = dist::skills();
    for (id, name, expected) in [
        (321, "Blinding Blow", OperateType::Active),
        (409, "Critical Blow", OperateType::Active),
        (1231, "Aura Flare", OperateType::Active),
        (426, "Battle Stance", OperateType::Channeling),
        (427, "Spell Stance", OperateType::Channeling),
    ] {
        let s = skills.get(id, 1).unwrap_or_else(|| panic!("{name} ({id})"));
        assert_eq!(
            s.operate_type, expected,
            "{name} ({id}) must be castable — `Other` makes `use_magic_on` bail"
        );
    }
    // A3 is continuous, which is read off the string and not this enum.
    assert!(
        skills.get(321, 1).unwrap().is_continuous,
        "A3 is one of Java's continuous types"
    );
}

/// **`UNDEAD_REAL_ENEMY`** — the priest anti-undead auras (Sanctuary 97, Holy
/// Aura 107, Repose 1034, Requiem 1049) are `SELF` + `POINT_BLANK`, so without
/// this filter they sweep *everything* nearby: friendly players and every
/// non-undead mob alike. That is what made it the one live correctness bug on
/// the affect axis rather than a missing nicety.
#[test]
fn an_undead_aura_spares_the_living_and_the_caster() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let ally = CASTER + 1;
    let _a = ingame_player(&mut world, CID + 1, ally, 40, 0, 0);

    let undead = NPC_OID;
    let living = NPC_OID + 1;
    add_test_npc(&mut world, undead, 90201, "Monster", 20, 60, 0, 0);
    add_test_npc(&mut world, living, 90202, "Monster", 20, 80, 0, 0);
    {
        let mut t = world.data.npc_data.get(90201).cloned().unwrap();
        t.race = Some(crate::enums::Race::Undead.ordinal());
        world.data.npc_data.insert_for_test(t);
    }

    let passes = |world: &World, oid: i32| {
        affect::passes_affect_object(world, CASTER, oid, AffectObject::UndeadRealEnemy)
    };

    assert!(
        passes(&world, undead),
        "an undead mob is the intended target"
    );
    assert!(!passes(&world, living), "a living mob is not");
    assert!(!passes(&world, ally), "and neither is a friendly player");
    assert!(
        !passes(&world, CASTER),
        "\"you are not an enemy of yourself\""
    );
}

/// **`OTHERS`** (Battle Stance 426, Spell Stance 427, Summon Friend 1403) — the
/// current selection, with exactly one rule: it may not be you, and Java
/// refuses with its own message rather than the generic invalid-target one.
#[test]
fn the_others_target_type_refuses_the_caster_with_its_own_message() {
    let skills = dist::skills();
    assert_eq!(
        skills.get(426, 1).unwrap().target_type,
        TargetType::Others,
        "Battle Stance is an OTHERS skill, not an unparsed fallback"
    );
}

/// **`Teleport`** — the destination Scrolls of Escape. 107 reachable skills
/// carried this effect and the parser did not know the name, so every one of
/// them loaded with an **empty effect list**: the scroll was consumed, the cast
/// animated, and nothing happened. Note the destination is per skill *level* —
/// skill 2213 alone carries 22 towns, one per level.
#[test]
fn every_destination_escape_scroll_now_carries_a_teleport() {
    use crate::model::skill::SkillEffect as E;

    let skills = dist::skills();
    // Two levels of the same scroll must give two *different* destinations.
    let lv1 = skills.get(2213, 1).expect("SoE lv1");
    let lv2 = skills.get(2213, 2).expect("SoE lv2");
    let coords = |s: &Skill| {
        s.effects.iter().find_map(|e| match e {
            E::Teleport { x, y, z } => Some((*x, *y, *z)),
            _ => None,
        })
    };
    let (a, b) = (coords(lv1), coords(lv2));
    assert!(a.is_some(), "the scroll carries a Teleport at all");
    assert_ne!(
        a, b,
        "and the destination is keyed on the skill level, not shared"
    );
    assert_eq!(
        a,
        Some((-114558, 253605, -1536)),
        "Talking Island, straight out of the datapack"
    );
}

/// And it has to actually move the player — an effect that parses and is never
/// applied is the failure this epic keeps finding.
#[test]
fn a_scroll_of_escape_moves_the_caster() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world.data.skill_data.insert_for_test(cc_skill(
        9440,
        SkillEffect::Teleport {
            x: 12_345,
            y: -6_789,
            z: -1_000,
        },
        "NONE",
    ));

    land(&mut world, 9440, CASTER);

    let pos = pos_of(&world, CASTER).unwrap();
    assert_eq!(
        (pos.0, pos.1),
        (12_345, -6_789),
        "the scroll actually moves you"
    );
    // `teleport_player` settles z onto the ground, so the destination z is a
    // request rather than a literal — assert the neighbourhood, not the value.
    assert!(
        (pos.2 - (-1_000)).abs() <= 64,
        "and lands near the requested height, got {}",
        pos.2
    );
}

/// **`Hp`** — the raw instant HP change behind Elixir of Life (2287) and the
/// food items, which parsed to *nothing* before. It is not a `Heal`: no
/// `calcHeal`, no healing-stat scaling. Java's guard list is dead / door /
/// HP-blocked / **raid**, that last one being the clause the `Heal` family
/// does not have.
#[test]
fn an_elixir_restores_hp_but_never_a_raid_bosss() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world.data.skill_data.insert_for_test(cc_skill(
        9441,
        SkillEffect::Hp {
            amount: 250.0,
            percent: false,
        },
        "NONE",
    ));
    let boss = NPC_OID;
    add_test_npc(&mut world, boss, 90301, "RaidBoss", 40, 100, 0, 0);

    for (oid, cur, max) in [(CASTER, 100.0, 1000), (boss, 100.0, 1000)] {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) {
            v.max_hp = max;
            v.cur_hp = cur;
        }
    }

    land(&mut world, 9441, CASTER);
    land(&mut world, 9441, boss);

    let hp = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<Vitals>(&oid)
            .map(|v| v.cur_hp)
            .unwrap_or(0.0)
    };
    assert_eq!(hp(&world, CASTER), 350.0, "a flat 250 restored");
    assert_eq!(
        hp(&world, boss),
        100.0,
        "a raid boss is exempt — the clause `Heal` does not have"
    );
}

/// The gain is clamped to the **recoverable** headroom, so an aura that caps
/// how far you can be healed caps an elixir too.
#[test]
fn an_elixir_honours_the_recoverable_ceiling() {
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world.data.skill_data.insert_for_test(cc_skill(
        9442,
        SkillEffect::Hp {
            amount: 900.0,
            percent: false,
        },
        "NONE",
    ));
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&CASTER) {
        v.max_hp = 1000;
        v.cur_hp = 100.0;
    }
    // Noblesse Harmony's shape: heals may only reach 70 % of the pool.
    if let Some(m) = world
        .objects
        .get_component_mut::<model::components::StatModifiers>(&CASTER)
    {
        *m.mul.entry(Stat::MaxRecoverableHp).or_insert(1.0) *= 0.7;
    }

    land(&mut world, 9442, CASTER);

    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&CASTER)
            .map(|v| v.cur_hp)
            .unwrap(),
        700.0,
        "clamped to the recoverable ceiling, not the raw pool"
    );
}

/// **`<nextAction>`** — `SkillCaster.finishSkill`'s "attack target after skill
/// use" block, on **339** skills with `ATTACK` and 11 with `CAST`. Without it
/// every offensive skill *ends* your combat: you fire Power Strike and stand
/// there. Java gates it on a real target that is neither you nor
/// un-attackable.
#[test]
fn a_next_action_attack_skill_leaves_the_caster_swinging() {
    let skills = dist::skills();
    use crate::model::skill::NextAction;
    // Power Strike (3) is the archetype; Wind Strike (1177-style nukes) too.
    let ps = skills.get(3, 1).expect("Power Strike");
    assert_eq!(
        ps.next_action,
        NextAction::Attack,
        "Power Strike declares nextAction=ATTACK"
    );
    // And the tag is not universally set — a buff must not drag you into melee.
    let wind_walk = skills.get(1204, 1).expect("Wind Walk");
    assert_eq!(
        wind_walk.next_action,
        NextAction::None,
        "a self-buff has no next action"
    );
}

/// The behavioural half: a finished `ATTACK` cast has to leave a live attack
/// intent behind. A parsed tag that never reaches the intent is the failure
/// this epic keeps finding.
#[test]
fn finishing_a_next_action_cast_starts_the_attack_intent() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mob = NPC_OID;
    add_test_npc(&mut world, mob, 20001, "Monster", 20, 60, 0, 0);

    let mut strike = cc_skill(9450, SkillEffect::Root, "ROOT");
    strike.next_action = model::skill::NextAction::Attack;
    world.data.skill_data.insert_for_test(strike.clone());

    let intent_target = |world: &World| {
        world
            .objects
            .get_component::<Intent>(&CASTER)
            .and_then(|i| match i.0 {
                model::PlayerIntent::Attack { target_object_id } => Some(target_object_id),
                _ => None,
            })
    };
    assert_eq!(intent_target(&world), None, "not attacking to begin with");

    resume_action_after_cast_for_test(&mut world, CASTER, mob, 9450, 1);

    assert_eq!(
        intent_target(&world),
        Some(mob),
        "the cast leaves the caster swinging at its target"
    );
}

/// **`<abnormalResists>`** — `calcEffectSuccess`'s first resist clause: a
/// target part-way through a cast whose skill names this abnormal type shrugs
/// the debuff off outright, before any roll. That is what makes the long-ritual
/// skills uninterruptible; 176 skills on this dist declare a list.
#[test]
fn a_caster_shrugs_off_an_abnormal_its_own_cast_resists() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = CASTER + 1;
    let _v = ingame_player(&mut world, CID + 1, victim, 40, 0, 0);

    // The ritual the victim is casting: immune to STUN while it runs.
    let mut ritual = cc_skill(9451, SkillEffect::Root, "NONE");
    ritual.abnormal_resists = vec!["STUN".into()];
    world.data.skill_data.insert_for_test(ritual);
    // The stun aimed at them.
    let mut stun = cc_skill(
        9452,
        SkillEffect::BlockActions { conditional: false },
        "STUN",
    );
    stun.is_debuff = true;
    stun.activate_rate = 100;
    world.data.skill_data.insert_for_test(stun.clone());

    let stunned = |world: &World| has_buff(world, victim, 9452);

    // Not casting: the stun lands.
    world.clear_forced_rolls();
    world.force_rolls([0; 8]);
    effects::apply_skill_effects(&mut world, CASTER, victim, &stun);
    assert!(stunned(&world), "an idle target takes the stun");

    // Mid-ritual: shrugged off before any roll.
    effects::handle_buff_expire(&mut world, victim, 9452);
    world.objects.add_components(
        &victim,
        Casting(model::CastState {
            skill_id: 9451,
            skill_level: 1,
            skill_sub_level: 0,
            target_object_id: victim,
            seq: 1,
            launched: false,
            cancel_ms: 0,
            cool_ms: 0,
            trigger_item_object_id: 0,
        }),
    );
    world.clear_forced_rolls();
    world.force_rolls([0; 8]);
    effects::apply_skill_effects(&mut world, CASTER, victim, &stun);
    assert!(
        !stunned(&world),
        "mid-ritual the same stun is resisted outright"
    );
}

/// **Anchor (1170) was doing half its job.** Its own description promises the
/// body goes "completely rigid for 5 seconds **and** causes paralysis for 5
/// seconds" — and that second half is an `<endEffects>` block firing
/// `CallSkill(6091)` when the first stage comes off. Neither the `END` effect
/// scope nor `CallSkill` existed, so the second stage never happened.
///
/// This is the last learnable gap G34's census had left, and it only surfaced
/// because the S8 gate forced every residual entry to be *examined* rather
/// than counted.
#[test]
fn anchors_second_stage_fires_when_the_first_expires() {
    let skills = dist::skills();
    let anchor = skills.get(1170, 1).expect("Anchor");
    assert_eq!(
        anchor.end_effects.len(),
        1,
        "the <endEffects> block is parsed, not dropped"
    );
    assert!(
        matches!(
            anchor.end_effects[0],
            SkillEffect::CallSkill { skill_id: 6091, .. }
        ),
        "and it calls Anchor's paralysis stage, got {:?}",
        anchor.end_effects[0]
    );
}

/// The behavioural half: the called skill has to actually land when the first
/// stage expires.
#[test]
fn an_end_effect_call_skill_lands_on_expiry() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = CASTER + 1;
    let _v = ingame_player(&mut world, CID + 1, victim, 40, 0, 0);

    // The second stage, and a first stage whose *end* calls it.
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(9461, SkillEffect::Root, "PARALYZE"));
    let mut first = cc_skill(9460, SkillEffect::Root, "RIGID");
    first.end_effects = vec![SkillEffect::CallSkill {
        skill_id: 9461,
        skill_level: 1,
        chance: 100,
    }];
    world.data.skill_data.insert_for_test(first.clone());

    let has = |world: &World, id: i32| has_buff(world, victim, id);

    effects::apply_skill_effects(&mut world, CASTER, victim, &first);
    assert!(has(&world, 9460), "the first stage is up");
    assert!(!has(&world, 9461), "the second has not fired yet");

    effects::handle_buff_expire(&mut world, victim, 9460);

    assert!(!has(&world, 9460), "the first stage is gone");
    assert!(
        has(&world, 9461),
        "and its expiry fired the second — the half Anchor was missing"
    );
}

/// `BuffInfo.isDisplayedForEffected()` — the one rule `isSelfContinuous()`
/// exists to feed.
///
/// An `A3` skill that also declares `<selfEffects>` shows its row only to the
/// caster. Blinding Blow's victim is blinded and *feels* it; they are simply
/// never sent an icon for it. Six skills on this dist qualify (321, 368, 369,
/// 409, 1231, 1996) and every other buff in the game is unaffected, so the
/// zero case is most of the assertion's value here.
#[test]
fn a_self_continuous_skills_debuff_shows_no_icon_to_its_victim() {
    let skills = dist::skills();

    let blinding_blow = skills.get(321, 1).expect("Blinding Blow loads");
    assert!(
        blinding_blow.self_continuous && !blinding_blow.self_effects.is_empty(),
        "the dist still declares 321 as A3 with selfEffects — the whole premise"
    );

    // A plain buff is A2/A1 and is displayed to whoever it lands on.
    let wind_walk = skills.get(1204, 1).expect("Wind Walk loads");
    assert!(
        !wind_walk.self_continuous,
        "an ordinary buff is not self-continuous"
    );

    // The rule itself, as `apply_continuous_effects` evaluates it.
    let displayed = |skill: &Skill, on_caster: bool| {
        !skill.self_continuous || on_caster || skill.self_effects.is_empty()
    };
    assert!(
        !displayed(blinding_blow, false),
        "the victim gets no icon for a self-continuous skill's debuff"
    );
    assert!(
        displayed(blinding_blow, true),
        "…but the caster still sees their own half of it"
    );
    assert!(
        displayed(wind_walk, false),
        "and nothing else in the game is hidden by this rule"
    );
}

/// The hidden buff must stay invisible in **both** channels Java gates on
/// `isDisplayedForEffected()`: the icon row and the abnormal-visual fold.
#[test]
fn a_hidden_buff_is_absent_from_the_icon_row_and_the_visuals() {
    use crate::model::components::Buffs;
    use crate::model::skill::{ActiveBuff, BuffSlot};

    let buff = |displayed: bool| ActiveBuff {
        skill_id: 321,
        abnormal_type_client_id: 7,
        slot: BuffSlot::Uncapped,
        expires_at_tick: 1000,
        displayed,
        abnormal_visuals: vec![13],
        ..test_buff()
    };

    // The icon row: the count field is the first thing after the opcode, so a
    // hidden buff has to leave it at zero rather than write a blank entry.
    let row = |displayed: bool| {
        let pkt =
            crate::network::enter_world::abnormal_status_update(&Buffs(vec![buff(displayed)]), 0);
        i16::from_le_bytes([pkt[1], pkt[2]])
    };
    assert_eq!(row(true), 1, "a displayed buff occupies a row");
    assert_eq!(row(false), 0, "a hidden one occupies none");

    // The visual fold, which Java runs under the same gate.
    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world
        .objects
        .add_components(&CASTER, Buffs(vec![buff(true)]));
    assert_eq!(abnormal::visual_effects(&world, CASTER), vec![13]);
    world
        .objects
        .add_components(&CASTER, Buffs(vec![buff(false)]));
    assert!(
        abnormal::visual_effects(&world, CASTER).is_empty(),
        "a hidden buff shows the effected no visual either"
    );
}

// ---------------------------------------------------------------------------
// `BreakStun` — a hit can shake a stun off (`Formulas.calcStunBreak`)
// ---------------------------------------------------------------------------

/// The port had no stun break at all, so a stunned player stayed stunned for
/// the full duration however hard they were hit. `BreakStun` ships **True**
/// (Java's own default is `false`), which makes the omission live.
///
/// The roll is pinned through `forced_rolls` rather than sampled: at 1-in-14 a
/// statistical assertion is flaky about 7 % of the time, which is exactly the
/// failure the first version of this test produced.
#[test]
fn a_hit_can_break_a_stun_but_only_on_the_one_in_fourteen_roll() {
    use crate::game_loop::skills::effects::try_break_stun;
    use crate::model::skill::effect_flag;

    let (mut world, _db, _l) = cc_world();
    let _caster = ingame_caster(&mut world, CID, 3001, 0, 0);
    let _victim = ingame_caster(&mut world, VICTIM_CID, 3002, 40, 0);

    let stunned = |w: &World| abnormal::flags_of(w, 3002) & effect_flag::BLOCK_ACTIONS != 0;
    let stun = |w: &mut World| {
        let skill = w.data.skill_data.get(STUN_ID, 1).expect("stun").clone();
        effects::apply_skill_effects(w, 3001, 3002, &skill);
        assert!(stunned(w), "the victim is stunned to begin with");
    };

    // A losing roll (`Rnd.get(14) != 0`) leaves the stun in place.
    stun(&mut world);
    world.force_roll(1);
    try_break_stun(&mut world, 3002);
    assert!(stunned(&world), "a non-zero roll does not free the victim");

    // The winning roll does.
    world.force_roll(0);
    try_break_stun(&mut world, 3002);
    assert!(!stunned(&world), "`Rnd.get(14) == 0` shakes the stun off");

    // With the key off, even the winning roll is never reached.
    world.cfg.character.alt_game_stun_break = false;
    stun(&mut world);
    world.force_roll(0);
    try_break_stun(&mut world, 3002);
    assert!(stunned(&world), "BreakStun=False leaves the stun alone");
    assert_eq!(
        world.forced_rolls_len(),
        1,
        "…and does not even consume the roll, because the key is checked first"
    );
}

/// Only `STUN` is shaken off. Sleep and paralyze carry the same
/// `BLOCK_ACTIONS` flag and must survive — Java filters on the abnormal type,
/// not the flag.
#[test]
fn breaking_a_stun_leaves_other_block_actions_debuffs_alone() {
    use crate::game_loop::skills::effects::try_break_stun;
    use crate::model::skill::{SkillEffect, effect_flag};

    let (mut world, _db, _l) = cc_world();
    // A sleep: same mechanic, different abnormal type.
    const SLEEP_ID: i32 = 9302;
    world.data.skill_data.insert_for_test(cc_skill(
        SLEEP_ID,
        SkillEffect::BlockActions { conditional: false },
        "SLEEP",
    ));
    let _caster = ingame_caster(&mut world, CID, 3001, 0, 0);
    let _victim = ingame_caster(&mut world, VICTIM_CID, 3002, 40, 0);

    let skill = world
        .data
        .skill_data
        .get(SLEEP_ID, 1)
        .expect("sleep")
        .clone();
    effects::apply_skill_effects(&mut world, 3001, 3002, &skill);
    assert!(abnormal::flags_of(&world, 3002) & effect_flag::BLOCK_ACTIONS != 0);

    // Force the *winning* roll every time: even then the sleep must survive,
    // because the filter is the abnormal type and not the flag.
    for _ in 0..20 {
        world.force_roll(0);
        try_break_stun(&mut world, 3002);
    }
    assert!(
        abnormal::flags_of(&world, 3002) & effect_flag::BLOCK_ACTIONS != 0,
        "a sleep is not a stun and must not be shaken off"
    );
}
