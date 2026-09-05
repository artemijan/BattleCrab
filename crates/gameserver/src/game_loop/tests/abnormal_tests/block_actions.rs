//! Stun, root, mute and physical mute: what each blocks, the in-flight cast
//! and mid-swing hit they drop, the raid-boss exemptions, breaking a stun,
//! and bluff.

use super::*;

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
    use crate::model::components::combat::AttackState;
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

/// The same `isRaid()` bail, on the **stun** side — `BlockActions.onStart`:
///
/// ```java
/// public void onStart(Creature effector, Creature effected, Skill skill, Item item)
/// {
///     if ((effected == null) || effected.isRaid()) return;
///     …
///     effected.startParalyze();
///     effected.abortAllSkillCasters();
/// }
/// ```
///
/// The buff still lands and its `BLOCK_ACTIONS` flag still counts — `onStart`
/// is the only thing Java skips — so a stun on a raid gates its *next* action
/// while leaving the cast and the swing already in flight alone. Without the
/// bail, a chain of stuns cancels a boss's every cast and the fight is decided
/// by stun uptime.
#[test]
fn raid_bosses_ignore_the_stun_interrupt() {
    use crate::model::components::space::Movement;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let mut t = crate::data::npc_data::default_template(20051);
    t.type_name = "RaidBoss".into();
    t.level = 40;
    t.base_hp_max = 5000.0;
    world.data.npc_data.insert_for_test(t);
    let mut plain = crate::data::npc_data::default_template(20052);
    plain.level = 40;
    plain.base_hp_max = 5000.0;
    world.data.npc_data.insert_for_test(plain);

    // Object ids must be in the **NPC** range: `is_npc_oid` is a range test,
    // and a player-range id would route the buff down the player branch.
    let raid = NPC_OID;
    let mob = NPC_OID + 1;
    add_test_npc(&mut world, raid, 20051, "RaidBoss", 40, 100, 0, 0);
    add_test_npc(&mut world, mob, 20052, "Monster", 40, 200, 0, 0);
    assert!(
        world.data.npc_data.get(20051).is_some_and(|t| t.is_raid())
            && !world.data.npc_data.get(20052).is_some_and(|t| t.is_raid()),
        "the fixture has to straddle `isRaid()` for this to mean anything"
    );

    // `startParalyze()`'s visible half here is that the victim is frozen where
    // it stands — the `Movement` component is dropped.
    let stun_and_check_frozen = |world: &mut World, oid: i32| -> bool {
        world.objects.add_components(
            &oid,
            Movement(crate::model::movement::MoveData {
                start_x: 0,
                start_y: 0,
                start_z: 0,
                dest_x: 500,
                dest_y: 0,
                dest_z: 0,
                start_tick: world.tick,
                total_ticks: 50,
                geo_path: None,
            }),
        );
        land(world, STUN_ID, oid);
        assert!(
            abnormal::flags_of(world, oid) & effect_flag::BLOCK_ACTIONS != 0,
            "the flag lands either way — only `onStart` is skipped"
        );
        !world.objects.has_component::<Movement>(&oid)
    };

    assert!(
        stun_and_check_frozen(&mut world, mob),
        "an ordinary monster is frozen by the stun"
    );
    assert!(
        !stun_and_check_frozen(&mut world, raid),
        "a raid boss keeps moving — Java returns before `startParalyze()`"
    );
}

const ATKMUTE_ID: i32 = 9323;

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
    use crate::model::skill::effect_flag;
    use crate::model::skill::effects::SkillEffect;

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
