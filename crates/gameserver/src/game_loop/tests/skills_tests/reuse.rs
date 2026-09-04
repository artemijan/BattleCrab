//! Reuse: cooldowns, shared reuse groups, and the cool-time list.

use super::*;

/// The full happy path of an offensive cast on another player, phase by
/// phase, plus the reuse gate on an immediate re-cast: exact
/// Formulas.calcMagicDam damage, CP absorbed before HP, the SM
/// 2261/2262 damage messages, and every broadcast reaching the target.
#[test]
fn cast_enemy_nuke_deals_damage_and_enforces_reuse() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    // A nuke now carries Java's ±10 % `randomMod`; this test compares two
    // casts, so the spread is switched off rather than averaged out.
    zero_random_damage(&mut world, 3001);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);

    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Without ctrl an unflagged player is not a valid enemy target.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::INVALID_TARGET
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(!world.objects.has_component::<Casting>(&3001));

    // With ctrl: ExRotation (face target) + initial-MP StatusUpdate +
    // MagicSkillUse to everyone, YOU_USE_S1 + SetupGauge to the caster.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::EX);
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    );
    let msu = a_rx.try_recv().unwrap();
    assert_eq!(msu[0], server_packets::opcodes::MAGIC_SKILL_USE);
    assert_eq!(
        i32::from_le_bytes(msu[25..29].try_into().unwrap()),
        -1,
        "ungrouped skill must send reuse group -1 (0 greys every icon client-side)"
    );
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::YOU_USE_S1
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::SETUP_GAUGE
    );
    assert!(a_rx.try_recv().is_err());
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::EX);
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_USE
    );
    assert!(b_rx.try_recv().is_err());
    assert_eq!(pvit(&world, 3001).cur_mp, 48.0, "50 - mpInitialConsume(2)");

    // Launch at hit = 4000/1.0 − 500 = 3500 ms = 35 ticks.
    world.tick += 35;
    apply_due_tasks(&mut world);
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_LAUNCHED
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_LAUNCHED
    );

    // Finish 500 ms later: MP consume, damage, messages, status updates.
    // Pin the two rolls the finish consumes: the magic crit (d1000) and the
    // `MagicFailures` success roll. PvP takes `calcMagicSuccess`' magic-accuracy
    // branch (both sides players, neither `isAttackable()`), which is only a
    // 98 % rate — left unforced, the nuke would be resisted ~2 % of runs and the
    // exact-damage assertions below would flake.
    world.force_rolls([999, 0]);
    world.tick += 5;
    apply_due_tasks(&mut world);

    let m_atk = pcs(&world, 3001).m_atk;
    let m_def = pcs(&world, 3002).m_def;
    let damage = formulas::magic::calc_magic_dam(
        m_atk,
        m_def,
        12.0,
        false,
        2.0,
        1.0,
        formulas::magic::MagicFailure::None,
        1.0,
    );
    assert!(
        damage > 100.0,
        "sanity: the nuke must overflow B's CP ({damage})"
    );
    {
        let b = pvit(&world, 3002);
        let bcp = pcp(&world, 3002);
        assert_eq!(bcp.cur_cp, 0.0, "CP absorbs first");
        assert!(
            (b.cur_hp - (100.0 - (damage - 100.0))).abs() < 1e-9,
            "HP takes the rest"
        );
    }
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    ); // MP consume
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2
    );
    // Being hit puts B in combat stance (CreatureAI.onEvtAttacked ->
    // clientStartAutoAttack broadcast), then B's CP/HP status.
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::AUTO_ATTACK_START
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    ); // B's CP/HP
    // Nuking a player flags the caster (SkillCaster: bad skill on a playable →
    // updatePvPStatus(target)): a PVP_FLAG StatusUpdate for object 3001, then
    // the caster's own stance — both broadcast, object 3001.
    let a_flag = a_rx.try_recv().unwrap();
    assert_eq!(a_flag[0], server_packets::opcodes::STATUS_UPDATE);
    assert_eq!(
        i32::from_le_bytes(a_flag[1..5].try_into().unwrap()),
        3001,
        "caster's own pvp-flag update"
    );
    let a_stance = a_rx.try_recv().unwrap();
    assert_eq!(a_stance[0], server_packets::opcodes::AUTO_ATTACK_START);
    assert_eq!(
        i32::from_le_bytes(a_stance[1..5].try_into().unwrap()),
        3001,
        "caster's own stance"
    );
    assert!(a_rx.try_recv().is_err());
    assert_eq!(
        sm_id(&b_rx.try_recv().unwrap()),
        server_packets::sm_ids::C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::AUTO_ATTACK_START
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    );
    // B also sees A's flag: the PVP_FLAG StatusUpdate + a RelationChanged.
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE,
        "B sees A's pvp-flag update"
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::RELATION_CHANGED,
        "B sees A's relation change"
    );
    let b_sees_a = b_rx.try_recv().unwrap();
    assert_eq!(b_sees_a[0], server_packets::opcodes::AUTO_ATTACK_START);
    assert_eq!(
        i32::from_le_bytes(b_sees_a[1..5].try_into().unwrap()),
        3001,
        "B sees the caster's stance"
    );
    assert!(b_rx.try_recv().is_err());
    assert!(
        world
            .objects
            .get_component::<model::components::combat::AttackState>(&3001)
            .is_some_and(|st| st.stance_until_tick > world.tick),
        "caster is in combat stance → canLogout refuses relogin"
    );
    assert_eq!(
        world
            .objects
            .get_component::<model::components::combat::PvpState>(&3001)
            .unwrap()
            .flag,
        1,
        "caster is now flagged for attacking a player"
    );
    assert!(
        !world.objects.has_component::<Casting>(&3001),
        "coolTime 0 frees the slot"
    );

    // Immediate re-cast: 10 s reuse still has 6 s left → SM 2303 + fail.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::S2_SECONDS_REMAINING_FOR_REUSE
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert!(b_rx.try_recv().is_err(), "rejected cast must not broadcast");
}

/// `RequestSkillCoolTime` reports the remaining reuse of a just-cast
/// skill.
#[test]
fn skill_cool_time_lists_remaining_reuse() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));
    drain(&mut a_rx);

    on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_COOL_TIME]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::SKILL_COOL_TIME);
    assert_eq!(
        i32::from_le_bytes(pkt[1..5].try_into().unwrap()),
        0,
        "Slow Aura has no reuse delay"
    );

    // A reuse with 6 s left is reported with its total and remainder.
    let until_tick = world.tick + 60;
    arm_reuse(
        &mut world,
        3001,
        1177,
        model::SkillReuse {
            skill_level: 1,
            until_tick,
            total_ms: 10_000,
        },
    );
    on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_COOL_TIME]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::SKILL_COOL_TIME);
    assert_eq!(i32::from_le_bytes(pkt[1..5].try_into().unwrap()), 1);
    assert_eq!(i32::from_le_bytes(pkt[5..9].try_into().unwrap()), 1177);
    assert_eq!(
        i32::from_le_bytes(pkt[9..13].try_into().unwrap()),
        1,
        "known level"
    );
    assert_eq!(
        i32::from_le_bytes(pkt[13..17].try_into().unwrap()),
        10,
        "total seconds"
    );
    assert_eq!(
        i32::from_le_bytes(pkt[17..21].try_into().unwrap()),
        6,
        "remaining seconds"
    );
}

/// Skills sharing a positive `reuseDelayGroup` share one cooldown entry
/// keyed by the group id: the `MagicSkillUse` broadcast carries the group,
/// casting one blocks the sibling (SM 48 — short reuse), and
/// `SkillCoolTime` reports the group id with the cast level.
#[test]
fn shared_reuse_group_blocks_sibling_skill() {
    let (mut world, ..) = cast_test_world();

    // Two quick self-skills in shared group 9000 (potion-style), cloned
    // off Slow Aura (91) so only the reuse fields differ.
    let base = world.data.skill_data.get(91, 1).unwrap().clone();
    for id in [7001, 7002] {
        world.data.skill_data.insert_for_test(Skill {
            self_continuous: false,
            id,
            hit_time: 400,
            reuse_delay: 2000,
            reuse_delay_group: 9000,
            ..base.clone()
        });
    }

    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let skills = &mut world
        .objects
        .get_component_mut::<SkillBook>(&3001)
        .unwrap()
        .0;
    skills.insert(7001, 1);
    skills.insert(7002, 1);

    // Cast the first: MagicSkillUse carries group 9000 + the 2000 ms
    // delay, and the reuse lands under the group key, not the skill id.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(7001, false));
    let msu = drain(&mut a_rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE)
        .expect("MagicSkillUse broadcast");
    assert_eq!(
        i32::from_le_bytes(msu[25..29].try_into().unwrap()),
        9000,
        "reuse group"
    );
    assert_eq!(
        i32::from_le_bytes(msu[29..33].try_into().unwrap()),
        2000,
        "reuse delay"
    );
    let reuses = &world.objects.get_component::<Reuses>(&3001).unwrap().0;
    assert!(reuses.contains_key(&9000) && !reuses.contains_key(&7001));

    // The sibling is blocked by the shared cooldown (reuse gate fires
    // before the busy-casting-slot check, same as Java's useMagic order).
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(7002, false));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::S1_IS_NOT_AVAILABLE_REUSE
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );

    // SkillCoolTime reports the group id, cast level, 2 s total/remaining.
    on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_COOL_TIME]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::SKILL_COOL_TIME);
    assert_eq!(i32::from_le_bytes(pkt[1..5].try_into().unwrap()), 1);
    assert_eq!(
        i32::from_le_bytes(pkt[5..9].try_into().unwrap()),
        9000,
        "group id, not skill id"
    );
    assert_eq!(
        i32::from_le_bytes(pkt[9..13].try_into().unwrap()),
        1,
        "cast level"
    );
    assert_eq!(
        i32::from_le_bytes(pkt[13..17].try_into().unwrap()),
        2,
        "total seconds"
    );
    assert_eq!(
        i32::from_le_bytes(pkt[17..21].try_into().unwrap()),
        2,
        "remaining seconds"
    );
}

/// `StoreSkillCooltime` round-trip: a live cooldown is captured into the save
/// (as an absolute wall-clock end time) and, on relog, `restore_reuses` re-arms
/// it against the current game tick — the cooldown survives the trip.
#[test]
fn skill_reuse_cooldown_survives_relog() {
    use model::SkillReuse;

    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    // A cooldown on reuse-key 1177, ending 500 ticks (50 s) out.
    let until_tick = world.tick + 500;
    arm_reuse(
        &mut world,
        3001,
        1177,
        SkillReuse {
            skill_level: 3,
            until_tick,
            total_ms: 300_000,
        },
    );

    // The save captures it (config default = on) as an absolute systime.
    let save = build_save_data(&world, 3001).expect("save data");
    assert_eq!(save.skill_reuses.len(), 1);
    let row = save.skill_reuses[0];
    assert_eq!(
        (row.reuse_key, row.skill_level, row.reuse_delay),
        (1177, 3, 300_000)
    );

    // Relog: a fresh bundle from a CharData carrying that row, restored against
    // the current tick + wall clock.
    let mut chr = dummy_char(3002, "Relog");
    chr.skill_reuses = vec![row];
    let mut bundle = Player::from_char(&world.data, &chr);
    bundle.restore_reuses(&chr, world.tick, commons::util::now_millis());

    let restored = bundle.reuses.0.get(&1177).expect("cooldown restored");
    assert_eq!((restored.skill_level, restored.total_ms), (3, 300_000));
    let remaining = restored.until_tick - world.tick;
    assert!(
        (498..=500).contains(&remaining),
        "≈500 ticks left, got {remaining}"
    );

    // With the config off, nothing is persisted (and the DB rows get cleared).
    world.cfg.character.store_skill_cooltime = false;
    assert!(
        build_save_data(&world, 3001)
            .unwrap()
            .skill_reuses
            .is_empty()
    );
}
