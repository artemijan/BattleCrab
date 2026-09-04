//! Picking and keeping a target: walking into range, retargeting, the cast
//! queue, mid-swing and mid-cast interruptions, and line of sight.

use super::*;

/// A shift-click cast out of range (Java `dontMove`) is cancelled with
/// SM 748 — no walk-into-range, nothing announced.
#[test]
fn shift_cast_out_of_range_cancelled_without_moving() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 700, 0); // castRange 600
    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);
    drain(&mut b_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body_shift(1177, true));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(a_rx.try_recv().is_err());
    assert!(b_rx.try_recv().is_err());
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert!(
        !world.objects.has_component::<Intent>(&3001),
        "dontMove must not start a walk-to-cast"
    );
    assert!(!world.objects.has_component::<Movement>(&3001));
}

/// The launch-phase `effectRange` re-check: a target who got away between
/// start and launch cancels the cast quietly (SM 748, no cancel packet —
/// Java `stopCasting(false)`).
#[test]
fn effect_range_recheck_cancels_when_target_moves_away() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    drain(&mut a_rx);
    drain(&mut b_rx);

    world
        .objects
        .get_component_mut::<Position>(&3002)
        .unwrap()
        .x = 5000; // > effectRange 1100

    world.tick += 40;
    apply_due_tasks(&mut world);
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED
    );
    assert!(
        a_rx.try_recv().is_err(),
        "no MagicSkillLaunched, no cancel packet"
    );
    assert!(b_rx.try_recv().is_err());
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert_eq!(pvit(&world, 3002).cur_hp, 100.0);
}

/// A skill clicked during a cast is queued (`Player._queuedSkill`) and fires
/// when the cast stops, resolved against the player's *current* target — so
/// re-targeting mid-cast redirects the queued skill (Java `stopCasting` →
/// `useMagic`, which re-resolves the target).
#[test]
fn skill_queued_during_cast_replays_on_current_target() {
    use model::components::combat::QueuedAction;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    let _c_rx = ingame_caster(&mut world, 3, 3003, 150, 0);
    world
        .objects
        .get_component_mut::<Vitals>(&3003)
        .unwrap()
        .cur_hp = 50.0;

    // A nukes B (hit 3500 + finish 500 ms = 40 ticks).
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
    drain(&mut a_rx);

    // Mid-cast: select C, then click Battle Heal → rejected but queued.
    handle_action(&mut world, 1, &action_body(3003, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(a_rx.try_recv().is_err(), "nothing else while the cast runs");
    assert!(
        matches!(
            world.objects.get_component::<QueuedAction>(&3001),
            Some(QueuedAction::Skill { skill_id: 1015, .. })
        ),
        "skill click parked in the queue slot"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Casting>(&3001)
            .unwrap()
            .0
            .skill_id,
        1177,
        "the running cast is untouched"
    );

    // The nuke finishes → the queued heal starts by itself, aimed at C.
    advance_ticks(&mut world, 45);
    let cast = world
        .objects
        .get_component::<Casting>(&3001)
        .expect("queued skill cast started");
    assert_eq!(cast.0.skill_id, 1015);
    assert_eq!(
        cast.0.target_object_id, 3003,
        "replay resolves the mid-cast re-target"
    );
    assert!(
        !world.objects.has_component::<QueuedAction>(&3001),
        "queue consumed"
    );

    // Heal phases (hit 500 + finish 500 ms): C's HP goes up.
    advance_ticks(&mut world, 12);
    assert!(
        pvit(&world, 3003).cur_hp > 50.0,
        "heal landed on the new target"
    );
}

/// A Ctrl-click (force attack) mid-cast on a *new* target must record the
/// attack as the next intention, so the swing starts once the cast ends —
/// Java's `onForcedAttack` → `setIntention(ATTACK)` (deferred to
/// `_nextIntention` while casting). Regression for the "it changes the target
/// but forgets to put the next intention, so when the cast finishes it doesn't
/// start a new action" report: a single ctrl-click used to only select.
#[test]
fn force_attack_mid_cast_engages_new_target_after_cast() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    // Nuke victim + the mob we force-attack next (in melee reach at x=20).
    add_test_npc(&mut world, NPC_OID + 90, 45001, "Monster", 5, 60, 0, 0);
    add_test_npc(&mut world, NPC_OID + 91, 45002, "Monster", 5, 20, 0, 0);
    let cast_target = NPC_OID + 90;
    let next = NPC_OID + 91;

    // Start a nuke on the first monster.
    handle_action(&mut world, 1, &action_body(cast_target, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "nuke is casting"
    );
    drain(&mut a_rx);

    // A SINGLE Ctrl-click on the second monster mid-cast: switches target AND
    // parks the attack as the intention (it can't swing yet — still casting).
    on_packet(
        &mut world,
        1,
        [vec![cop::ATTACK], attack_request_body(next)].concat(),
    );
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(next),
        "target switched to the ctrl-clicked mob"
    );
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(model::PlayerIntent::Attack { target_object_id })) if *target_object_id == next
        ),
        "the force-attack is remembered as the next intention"
    );
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "the running nuke is untouched"
    );

    // When the nuke finishes, the parked attack engages the new mob.
    let hp_before = nvit(&world, next).cur_hp;
    world.force_rolls(std::iter::repeat_n([0i32, 99, 10], 12).flatten());
    advance_world(&mut world, 55);
    assert!(
        nvit(&world, next).cur_hp < hp_before,
        "the new target took melee damage after the cast"
    );
}

/// A skill clicked mid-swing (`isAttackingNow`) queues and fires when the
/// swing period ends (Java `thinkAttack`'s queued-skill check /
/// `EVT_READY_TO_ACT`), leaving the attack intent alive to resume after.
#[test]
fn skill_mid_swing_is_queued_until_swing_end() {
    use crate::game_loop;
    use model::components::combat::QueuedAction;

    let (mut world, ..) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 20;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 100_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    // Swing rolls: hit, no crit, ±0 random damage.
    world.force_rolls([0, 99, 10]);
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    drain(&mut a_rx);
    let swing_end = world
        .objects
        .get_component::<model::components::combat::AttackState>(&3001)
        .unwrap()
        .attack_end_tick;
    assert!(swing_end > world.tick, "swing in flight");

    // Mid-swing skill click: rejected, queued, intent intact.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(
        !world.objects.has_component::<Casting>(&3001),
        "no cast mid-swing"
    );
    assert!(matches!(
        world.objects.get_component::<QueuedAction>(&3001),
        Some(QueuedAction::Skill { skill_id: 91, .. })
    ));
    assert!(
        world.objects.has_component::<Intent>(&3001),
        "skill click keeps the attack intent"
    );

    // Swing period over (`AttackFinish`): the queued cast starts.
    let remaining = swing_end - world.tick;
    advance_ticks(&mut world, remaining);
    let cast = world
        .objects
        .get_component::<Casting>(&3001)
        .expect("queued skill fired at swing end");
    assert_eq!(cast.0.skill_id, 91);
    assert!(
        world.objects.has_component::<Intent>(&3001),
        "attack resumes after the cast"
    );
}

/// The target-handler geodata check: a wall between caster and target
/// fails the cast with SM 181 (`CANNOT_SEE_TARGET`); with the target on
/// the caster's side the same cast starts normally.
#[test]
fn cast_blocked_by_wall_sends_cannot_see_target() {
    let (mut world, ..) = cast_test_world();
    install_wall_region(&mut world);
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 8, 8);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 328, 8); // across the wall

    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::CANNOT_SEE_TARGET
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(!world.objects.has_component::<Casting>(&3001));

    // Same side of the wall: the cast starts.
    world
        .objects
        .get_component_mut::<Position>(&3002)
        .unwrap()
        .x = 72; // cell 4
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
}

/// An out-of-range cast walks the caster into cast range (Java `useMagic` →
/// CAST intention → `thinkCast`/`maybeMoveToPawn`) and only then starts the
/// cast at the snapshotted target.
#[test]
fn cast_out_of_range_walks_into_range_then_casts() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    // 700 away — castRange 600 + collision 9 + 10 leaves ~81 units to walk.
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 700);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "walks toward the cast target"
    );
    assert!(
        !packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "no cast before range"
    );
    assert!(world.objects.has_component::<Intent>(&3001));
    assert!(!world.objects.has_component::<Casting>(&3001));

    // ~81 units at run speed 115 ⇒ in range in ~8 ticks.
    advance_world(&mut world, 15);
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "cast starts on arrival"
    );
    assert!(
        !world.objects.has_component::<Intent>(&3001),
        "the walk-to-cast intent is consumed"
    );
    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "chase leg stopped before casting"
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE)
    );

    // Launch (35 ticks) + finish (5): the nuke lands on the walked-to monster.
    advance_world(&mut world, 45);
    assert!(
        nvit(&world, npc_oid).cur_hp < 5000.0,
        "nuke landed after the walk"
    );
}

/// Repro for "cast on a monster, mid-cast select a far monster and click the
/// same skill again → the click is forgotten": the queued skill must replay at
/// cast end against the new target and, being out of range, start the
/// walk-to-cast leg (Java `stopCasting` → `useMagic` → CAST intention →
/// `thinkCast`/`maybeMoveToPawn`).
#[test]
fn queued_skill_on_far_retarget_walks_into_range_after_cast() {
    use crate::game_loop;
    use model::components::combat::QueuedAction;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let near = NPC_OID + 70;
    let far = NPC_OID + 71;
    spawn_targeted_monster(&mut world, &mut a_rx, near, 100);
    // The far monster: outside castRange 600, spawned untargeted.
    let (npc, extra) = model::npc::Npc::for_test(far, 40001, 900, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1.0).or_default().push(far);
    world.objects.spawn(far, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&far, cs);

    // Nuke the near monster (hit 3500 + finish 500 ms = 40 ticks).
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "first cast running"
    );

    // Mid-cast, past the real Wind Strike's 1200 ms reuse (the test skill's
    // 10 s reuse is dropped to model the dist timing, where the reuse expires
    // while the 4 s cast is still running): select the far monster and click
    // the same skill again.
    advance_world(&mut world, 15);
    if let Some(reuses) = world.objects.get_component_mut::<Reuses>(&3001) {
        reuses.0.clear();
    }
    handle_action(&mut world, 1, &action_body(far, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        matches!(
            world.objects.get_component::<QueuedAction>(&3001),
            Some(QueuedAction::Skill { skill_id: 1177, .. })
        ),
        "second click parked in the queue slot"
    );
    drain(&mut a_rx);

    // Cast end → replay → out of range → walk-to-cast toward the far monster.
    advance_world(&mut world, 30);
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(model::PlayerIntent::Cast { target_object_id, .. })) if *target_object_id == far
        ),
        "replayed click walks to the far monster (got intent {:?}, queued {:?}, casting {:?})",
        world.objects.get_component::<Intent>(&3001),
        world.objects.get_component::<QueuedAction>(&3001),
        world.objects.get_component::<Casting>(&3001)
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "MoveToPawn broadcast for the walk"
    );

    // The nuked monster has 5000 HP, so it survived and has been meleeing the
    // caster since the nuke landed — and now that a mob swings the instant it
    // closes (`EVT_ARRIVED` → `onEvtThink`) that is enough damage to kill the
    // 100 HP test caster during the walk below. This test is about the
    // queued-skill replay, not about retaliation: call the monster off and
    // top the caster back up so the walk-to-cast is what is being measured.
    world
        .objects
        .get_component_mut::<AggroList>(&near)
        .unwrap()
        .0
        .clear();
    world
        .objects
        .get_component_mut::<NpcAi>(&near)
        .unwrap()
        .intention = NpcIntention::Active;
    {
        let v = world.objects.get_component_mut::<Vitals>(&3001).unwrap();
        v.cur_hp = v.max_hp as f64;
    }

    // ~300 units at run speed ⇒ in range, then the cast starts on the far mob.
    // 40 ticks is the bare arrival time, with no slack for a tick lost to the
    // walk's start, so allow a couple more — the cast has 40 ticks of its own
    // to run, which leaves plenty of window to observe it.
    advance_world(&mut world, 45);
    let cast = world
        .objects
        .get_component::<Casting>(&3001)
        .expect("cast started after the walk");
    assert_eq!(
        cast.0.target_object_id, far,
        "cast aimed at the far monster"
    );
}

/// Same scenario through the real client packet sequence: switching targets
/// sends `RequestTargetCanceld` (aborting the running cast) before the
/// `Action` click, so the second skill click must start the walk-to-cast
/// immediately.
#[test]
fn far_retarget_after_target_cancel_walks_into_range() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let near = NPC_OID + 72;
    let far = NPC_OID + 73;
    spawn_targeted_monster(&mut world, &mut a_rx, near, 100);
    let (npc, extra) = model::npc::Npc::for_test(far, 40001, 900, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1.0).or_default().push(far);
    world.objects.spawn(far, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&far, cs);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "first cast running"
    );

    // Client target switch: TargetCanceld (aborts the cast) + Action(far).
    advance_world(&mut world, 15);
    if let Some(reuses) = world.objects.get_component_mut::<Reuses>(&3001) {
        reuses.0.clear();
    }
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(false));
    assert!(
        !world.objects.has_component::<Casting>(&3001),
        "cast aborted by the switch"
    );
    handle_action(&mut world, 1, &action_body(far, 0));
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(model::PlayerIntent::Cast { target_object_id, .. })) if *target_object_id == far
        ),
        "second click walks to the far monster (got intent {:?})",
        world.objects.get_component::<Intent>(&3001)
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "MoveToPawn broadcast for the walk"
    );

    advance_world(&mut world, 40);
    let cast = world
        .objects
        .get_component::<Casting>(&3001)
        .expect("cast started after the walk");
    assert_eq!(
        cast.0.target_object_id, far,
        "cast aimed at the far monster"
    );
}

/// The same "queue on a far retarget" flow against the real datapack: real
/// Wind Strike (4 s cast, 1.2 s reuse — the reuse expires while the cast is
/// still running, so the mid-cast second click must reach the queue slot).
#[test]
fn queued_far_retarget_with_real_datapack_timings() {
    use crate::game_loop;
    use model::components::combat::QueuedAction;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    world.data = dist::game_data_owned();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    // The first cast's damage makes the (passive, level-1) Gremlin retaliate,
    // and a melee hit on a still-abortable cast rolls `Formulas.calcAtkBreak`
    // — `15 + sqrt(13 * dmg)`, ~20 % at this damage — which then decides the
    // *last* assertion below. That made this test fail ~2 % of the time for a
    // reason it does not cover. `isInvul` returns out of `reduce_hp` ahead of
    // the break roll, so the retaliation lands no damage and rolls nothing,
    // leaving the queue/retarget flow under test untouched.
    world.objects.add_components(
        &3001,
        AdminFlags {
            invul: true,
            ..Default::default()
        },
    );
    let near = NPC_OID + 74;
    let far = NPC_OID + 75;
    // Real-datapack monsters (Gremlin, 20001) at 100 and 900 units.
    for (oid, x) in [(near, 100), (far, 900)] {
        let (npc, extra) = model::npc::Npc::for_test(oid, 20001, x, 0, 0, 5000, 30);
        world.npc_regions.entry(extra.1.0).or_default().push(oid);
        world.objects.spawn(oid, (npc, extra));
        let cs = game_loop::npc::npc_combat_stats(
            world.data.npc_data.get(20001).unwrap(),
            &world.data.stat_bonus,
        );
        world.objects.add_components(&oid, cs);
    }
    handle_action(&mut world, 1, &action_body(near, 0));
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "first cast running"
    );
    drain(&mut a_rx);

    // 2 s in: reuse (1.2 s) expired, cast (~4 s) still running. Select the far
    // monster and click the same skill again.
    advance_world(&mut world, 20);
    handle_action(&mut world, 1, &action_body(far, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        matches!(
            world.objects.get_component::<QueuedAction>(&3001),
            Some(QueuedAction::Skill { skill_id: 1177, .. })
        ),
        "second click parked in the queue slot (casting {:?}, reuses {:?})",
        world
            .objects
            .get_component::<Casting>(&3001)
            .map(|c| c.0.skill_id),
        world.objects.get_component::<Reuses>(&3001)
    );
    drain(&mut a_rx);

    // Cast end → replay → walk-to-cast toward the far monster.
    advance_world(&mut world, 40);
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(model::PlayerIntent::Cast { target_object_id, .. })) if *target_object_id == far
        ) || world
            .objects
            .get_component::<Casting>(&3001)
            .is_some_and(|c| c.0.target_object_id == far),
        "replayed click acts on the far monster (intent {:?}, queued {:?}, casting {:?})",
        world.objects.get_component::<Intent>(&3001),
        world.objects.get_component::<QueuedAction>(&3001),
        world.objects.get_component::<Casting>(&3001)
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "MoveToPawn broadcast for the walk"
    );

    advance_world(&mut world, 40);
    let cast = world
        .objects
        .get_component::<Casting>(&3001)
        .expect("cast started after the walk");
    assert_eq!(
        cast.0.target_object_id, far,
        "cast aimed at the far monster"
    );
}

/// G19 `TargetType::EnemyNot` — the "any friendly selected target" gate
/// `targethandlers/EnemyNot.java` backs (34 instances, 4 learnable, including
/// "Restore Life" itself), found unmodeled while testing `HealPercent`: it
/// fell through to `Other`, and `use_magic_on` silently no-ops on that (no
/// packet, no cast). Restore Life (1258, real dist data, level 1 heals 15%
/// of max HP) now lands on a friendly player.
#[test]
fn enemy_not_targets_a_friendly_player() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();

    // Restore Life is `isMagic`, so its cast time scales by the caster's
    // *magic* casting speed — a level-1 default (Human Fighter, class 0) has
    // a near-zero one, stretching an 8 s cast into minutes. Use a Mystic
    // (class 10, as the real-data spellcraft test does) for a sane cast time.
    let mut chr = dummy_char(5401, "Healer");
    chr.class_id = 10;
    chr.base_class_id = 10;
    chr.skills = vec![(1258, 1, 0)];
    let bundle = Player::from_char(&world.data, &chr);
    let (out_tx, mut a_rx) = tokio::sync::mpsc::unbounded_channel();
    let s = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![])
        .into_entering(bundle);
    let (session, bundle) = s.into_ingame();
    bundle.spawn_into(&mut world);
    world.clients.insert(1, ClientSession::InGame(session));
    // `chr.cur_mp` gets clamped to the class's computed max MP at spawn (59
    // for a level-1 Mystic) — below level-1 Restore Life's 80 MP cost, so
    // bump it directly rather than fighting the clamp through `CharData`.
    world
        .objects
        .get_component_mut::<Vitals>(&5401)
        .unwrap()
        .cur_mp = 200.0;

    let mut b_rx = ingame_player_access(&mut world, 2, 5402, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    // Distinct position from the caster's default (1, 2, 3) — same-position
    // casters/targets aren't exercised elsewhere in this suite.
    world
        .objects
        .get_component_mut::<Position>(&5402)
        .unwrap()
        .x = 50;

    let max_hp = pvit(&world, 5402).max_hp as f64;
    let half = max_hp * 0.5;
    world
        .objects
        .get_component_mut::<Vitals>(&5402)
        .unwrap()
        .cur_hp = half;

    handle_action(&mut world, 1, &action_body(5402, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1258, false));
    advance_world(&mut world, 200); // hitTime 8000 ms at a Mystic's casting speed

    let expected = half + max_hp * 0.15; // level 1 power = 15%, no overheal clamp hit
    assert!(
        (pvit(&world, 5402).cur_hp - expected).abs() < 1e-6,
        "healed a friendly player 15% of max HP: {} (expected {})",
        pvit(&world, 5402).cur_hp,
        expected
    );
    let b_packets = drain(&mut b_rx);
    assert!(
        has_system_message(
            &b_packets,
            server_packets::sm_ids::S2_HP_HAS_BEEN_RESTORED_BY_C1
        ),
        "target sees the heal SystemMessage"
    );
}

/// The other half of `TargetType::EnemyNot`: the exact inverse of `Enemy`'s
/// gate, so a hostile target is refused outright (no ctrl/force-use override,
/// unlike `Enemy`).
#[test]
fn enemy_not_refuses_a_hostile_target() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();

    let mut a_rx = ingame_player_access(&mut world, 1, 5411, 0);
    drain(&mut a_rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5411)
        .unwrap()
        .0
        .insert(1258, 1);
    world
        .objects
        .get_component_mut::<Vitals>(&5411)
        .unwrap()
        .cur_mp = 200.0;

    // A real dist monster (20001 Gremlin) is auto-attackable.
    let npc_oid = NPC_OID + 1;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 20001, 50, 0, 0, 1000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(20001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1258, false));
    assert!(
        !world.objects.has_component::<Casting>(&5411),
        "refused: no cast against a hostile target"
    );
}
