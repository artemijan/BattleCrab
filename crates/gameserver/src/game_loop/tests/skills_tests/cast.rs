//! The cast pipeline itself — charging a shot, aborting, the MP shortfall,
//! a precast broken by damage, and how far a cast broadcasts.

use super::*;

/// Esc aborts a pre-launch cast: `MagicSkillCanceled` broadcast (self
/// included) + `ActionFailed`, the stale phase tasks no-op, the reuse
/// registered at cast start still stands (Java semantics), and once it
/// runs out the caster can cast again.
#[test]
fn esc_aborts_cast_and_stale_tasks_noop() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    drain(&mut a_rx);
    drain(&mut b_rx);
    let mp_after_start = pvit(&world, 3001).cur_mp;

    // Esc (targetLost=false: abort only, keep the target).
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(false));
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_CANCELED
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_CANCELED
    );

    // The scheduled launch is stale: nothing fires, nothing lands.
    world.tick += 40;
    apply_due_tasks(&mut world);
    assert!(a_rx.try_recv().is_err());
    assert!(b_rx.try_recv().is_err());
    assert_eq!(
        pvit(&world, 3001).cur_mp,
        mp_after_start,
        "no finish consume after abort"
    );
    assert_eq!(pvit(&world, 3002).cur_hp, 100.0);

    // Reuse (registered at cast start) still blocks, then expires.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::S2_SECONDS_REMAINING_FOR_REUSE
    );
    drain(&mut a_rx);
    world.tick += 60;
    apply_due_tasks(&mut world);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "castable again after reuse expiry"
    );
}

/// Finish-phase MP shortfall stops the cast quietly: SM 24 +
/// ActionFailed to the caster, but no `MagicSkillCanceled` (Java
/// `stopCasting(false)`), and no effects land.
#[test]
fn finish_phase_mp_shortfall_aborts_quietly() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    drain(&mut a_rx);
    drain(&mut b_rx);

    world
        .objects
        .get_component_mut::<Vitals>(&3001)
        .unwrap()
        .cur_mp = 0.0;

    advance_ticks(&mut world, 45);
    // Launch fires normally (range fine), then the finish fails on MP.
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_LAUNCHED
    );
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::NOT_ENOUGH_MP
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(a_rx.try_recv().is_err());
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_LAUNCHED
    );
    assert!(b_rx.try_recv().is_err(), "no cancel packet on a quiet stop");
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert_eq!(pvit(&world, 3002).cur_hp, 100.0, "no damage landed");
}

/// Incoming magic damage can break a victim's pre-launch cast
/// (`Formulas.calcAtkBreak`): `MagicSkillCanceled` broadcast + SM 27 to
/// the victim, and their stale launch task no-ops.
#[test]
fn incoming_magic_damage_can_break_precast() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);

    // B starts a slow self-cast (hit = 9500 ms = 95 ticks).
    handle_request_magic_skill_use(&mut world, 2, &magic_skill_use_body(91, false));
    assert!(world.objects.has_component::<Casting>(&3002));

    // A nukes B; the nuke lands at 40 ticks, well before B's launch.
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Force the rolls: crit d1000 (rate 0 → miss regardless), the magic-success
    // d100 (PvP accuracy branch, rate 98 → 0 lands, so damage is unreduced),
    // the random-damage d21 (10 is the middle, i.e. a ×1.0 spread — the term
    // `calcMagicDam` multiplies by, added when the formula-parity sweep found
    // it missing), then the atk-break d100 → 0 always breaks (rate ≥ 1).
    world.force_rolls([999, 0, 10, 0]);

    advance_ticks(&mut world, 45);

    assert!(
        !world.objects.has_component::<Casting>(&3002),
        "victim's cast broken"
    );
    let b_packets = drain(&mut b_rx);
    assert!(
        b_packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_CANCELED)
    );
    assert!(
        b_packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::YOUR_CASTING_HAS_BEEN_INTERRUPTED)
    );
    let a_packets = drain(&mut a_rx);
    assert!(
        a_packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_CANCELED)
    );

    // B's stale launch task fires and no-ops: no buff ever lands.
    advance_ticks(&mut world, 60);
    assert_eq!(pbuffs(&world, 3002), 0);
}

/// Casting any skill while running stops the move for good — Java's
/// `PlayerAI.changeIntention` saves the MOVE_TO as `_nextIntention` for a
/// good-skill cast, but `SkillCaster.startCasting` immediately replaces the
/// intention with IDLE, wiping the saved move; a bad skill clears it in
/// `changeIntention` itself. Either way the player stands where the cast
/// began and does not resume walking.
#[test]
fn cast_discards_inflight_move() {
    use model::components::combat::QueuedAction;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    world
        .objects
        .get_component_mut::<Speeds>(&3001)
        .unwrap()
        .run_spd = 100.0;
    world
        .objects
        .get_component_mut::<Speeds>(&3001)
        .unwrap()
        .running = true;

    handle_move_backward_to_location(&mut world, 1, &move_body((600, 0, 0), (0, 0, 0), 1));
    assert!(world.objects.has_component::<Movement>(&3001));
    drain(&mut a_rx);

    // Slow Aura (good, self): the move stops and its destination is dropped.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));
    assert!(world.objects.has_component::<Casting>(&3001));
    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "cast stops the move"
    );
    assert!(
        !world.objects.has_component::<QueuedAction>(&3001),
        "good skill forgets the move (startCasting sets IDLE)"
    );

    // hit 9500 ms (95 ticks) + finish 5 ticks later: still standing.
    advance_ticks(&mut world, 101);
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "move must not resume after the cast"
    );

    // An offensive cast forgets the interrupted move the same way.
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    handle_move_backward_to_location(&mut world, 1, &move_body((600, 0, 0), (0, 0, 0), 1));
    assert!(world.objects.has_component::<Movement>(&3001));
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "cast stops the move"
    );
    assert!(
        !world.objects.has_component::<QueuedAction>(&3001),
        "bad skill forgets the move"
    );
    advance_ticks(&mut world, 45);
    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "nothing resumes after a nuke"
    );
}

/// Broadcasts only reach players whose region cell is adjacent to the
/// broadcaster's (Java `broadcastPacket` over `forEachVisibleObject`).
#[test]
fn broadcast_is_scoped_to_surrounding_regions() {
    let (mut world, ..) = test_world();
    let _mover_rx = ingame_player(&mut world, 1, 6101, 0, 0, 0);
    let mut near_rx = ingame_player(&mut world, 2, 6102, 500, 500, 0);
    let mut far_rx = ingame_player(&mut world, 3, 6103, 10_000, 10_000, 0);
    world
        .objects
        .get_component_mut::<Speeds>(&6101)
        .unwrap()
        .run_spd = 100.0;

    handle_move_backward_to_location(&mut world, 1, &move_body((1000, 0, 0), (0, 0, 0), 1));

    assert_eq!(
        near_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MOVE_TO_LOCATION
    );
    assert!(
        far_rx.try_recv().is_err(),
        "far player must not see the move"
    );
}

/// **A non-combat transform cannot walk into range to cast** — Java's "while
/// flying there is no move to cast" (`checkTransformed(t -> !t.isCombat())` →
/// SM 748 + `ActionFailed`, `maybeMoveToPawn` returning true).
///
/// The discrimination is the point: a COMBAT form walks as normal. Asserting
/// only the refusal would pass just as well if the gate ignored the flag and
/// refused everyone.
#[test]
fn a_non_combat_transform_is_refused_a_walk_to_cast() {
    use model::Player;

    let refused_for = |transform_id: i32| -> bool {
        let (mut world, ..) = cast_test_world();
        world.data.transforms = crate::data::TransformData::load_from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/"
        ));
        let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        // A target far enough that the cast needs a walk.
        let _target_rx = ingame_caster(&mut world, 2, 3002, 5000, 0);
        world
            .objects
            .get_component_mut::<Player>(&3001)
            .unwrap()
            .transform_id = transform_id;
        set_target(&mut world, 1, 3001, Some(3002));
        drain(&mut rx);
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::THE_DISTANCE_IS_TOO_FAR_AND_SO_THE_CASTING_HAS_BEEN_CANCELLED,
        )
    };

    // Transform 101 is NON_COMBAT on this dist; 1 is COMBAT.
    assert!(
        refused_for(101),
        "a non-combat form is refused the walk-to-cast"
    );
    assert!(
        !refused_for(1),
        "a COMBAT form walks as normal — the gate reads the flag, not merely 'is transformed'"
    );
    assert!(!refused_for(0), "and an untransformed player is unaffected");
}

/// **Spiritshots speed the cast up**: `calcSkillTimeFactor`'s
/// `spiritshotHitTime` is 0.4, so a charged mage casts at `matkSpdMul × 1.4`.
/// The port had no shot term in the factor at all, so a spiritshot bought
/// damage and nothing else.
#[test]
fn a_charged_spiritshot_shortens_the_cast() {
    use crate::model::components::stats::{BaseStats, StatModifiers};
    use crate::model::formulas;

    let (mut world, _db, _l) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let skill = world
        .data
        .skill_data
        .get(1177, 1)
        .expect("Wind Strike loads");
    assert_eq!(skill.magic_type, 1, "the fixture skill must be magic");

    let factor = |charged: bool, world: &World| {
        let p = world
            .objects
            .get_component::<Player>(&3001)
            .expect("player");
        let base = world
            .objects
            .get_component::<BaseStats>(&3001)
            .expect("base");
        let mods = world
            .objects
            .get_component::<StatModifiers>(&3001)
            .expect("mods");
        formulas::timing::calc_skill_time_factor(p, base, mods, &world.data, skill, charged, None)
    };

    let plain = factor(false, &world);
    let charged = factor(true, &world);
    assert!(
        (charged - plain * 1.4).abs() < 1e-9,
        "a charged spiritshot is worth ×1.4 on the cast-time factor ({charged} vs {plain})"
    );

    // …and a **channeled** skill ignores both: Java's factor is a flat 1, so
    // its cancel time is not divided by cast speed either.
    let mut channeled = skill.clone();
    channeled.operate_type = crate::model::skill::target::OperateType::Channeling;
    let p = world
        .objects
        .get_component::<Player>(&3001)
        .expect("player");
    let base = world
        .objects
        .get_component::<BaseStats>(&3001)
        .expect("base");
    let mods = world
        .objects
        .get_component::<StatModifiers>(&3001)
        .expect("mods");
    assert_eq!(
        formulas::timing::calc_skill_time_factor(
            p,
            base,
            mods,
            &world.data,
            &channeled,
            true,
            None
        ),
        1.0,
        "a channeling skill's timing is static"
    );
}
