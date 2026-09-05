//! Starting a swing: the action click, ctrl and shift attack, chasing a
//! target out of reach, and the day/night hit difference.

use super::*;

/// `Action` on a monster tints `MyTargetSelected` with the level gap; a second
/// click on the already-targeted (out-of-range) monster starts the attack and
/// walks toward it (`MoveToPawn`) — never a chat window.
#[test]
fn action_on_monster_colors_target_and_never_talks() {
    let (mut world, ..) = test_world();
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 3, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 8;

    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    assert_eq!(
        rx.try_recv().unwrap()[0],
        server_packets::opcodes::VALIDATE_LOCATION
    );
    let mts = rx.try_recv().unwrap();
    assert_eq!(mts[0], server_packets::opcodes::MY_TARGET_SELECTED);
    assert_eq!(
        i16::from_le_bytes(mts[9..11].try_into().unwrap()),
        5,
        "player 8 vs monster 3"
    );
    assert_eq!(
        rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    );
    assert_eq!(
        rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );

    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    let after: Vec<Vec<u8>> =
        std::iter::from_fn(|| rx.try_recv().ok().map(|p| p.to_vec())).collect();
    assert!(
        after
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "second click starts the attack and walks the out-of-range monster down"
    );
    assert!(
        !after
            .iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE),
        "no chat window from a monster"
    );
}

/// Java's `getConditionBonus` subtracts `darkBonus` — **−10** on this dist —
/// for the whole in-game night, and `calcHitMiss` narrows its chance through
/// that multiplier. The port parsed the value and never applied it, on a doc
/// comment that predated the G33 game clock.
///
/// The proof has to be end-to-end: the formula is swept in `formula_parity`,
/// but what was broken was the *call site* not asking what time it is. So this
/// swings the same attack with the same rolls at in-game noon and at in-game
/// midnight, at a miss roll that sits between the two chances.
#[test]
fn a_swing_that_lands_by_day_misses_at_night() {
    /// One in-game day is 4 real hours; the phase is measured from the epoch,
    /// so an exact multiple is in-game midnight (night) and half past it noon.
    const IG_DAY_MS: i64 = 14_400_000;
    let midnight = 1_800_000_000_000 - (1_800_000_000_000 % IG_DAY_MS);

    let swing_at = |now: i64| -> (f64, f64) {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        world.forced_now_millis = Some(now);
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = NPC_OID + 21;
        // 100 000 HP: the swing must not kill, so the HP delta reads as
        // "landed" or "missed" and nothing else.
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
        drain(&mut a_rx);

        // The attacker stands at (0,0) and the target faces heading 0 at
        // (30,0), so the swing lands from BEHIND — the same position both
        // times, which is what leaves the night term as the only difference.
        let accuracy = pcs(&world, 3001).accuracy;
        let evasion = pcs(&world, npc_oid).evasion;
        let chance = |night: bool| {
            let condition = world.data.hit_condition_bonus.condition_bonus(
                0,
                0,
                model::movement::Position::Back,
                night,
            );
            (f64::from((80 + (2 * (accuracy - evasion))) * 10) * condition).clamp(200.0, 980.0)
        };
        // `calcHitMiss` misses when `chance < roll`, so a roll one past the
        // night chance misses at night and lands by day.
        let roll = chance(true) as i32 + 1;
        assert!(
            chance(false) >= chance(true) + 2.0,
            "the two chances have to be far enough apart to sit a roll between"
        );
        // Swing rolls: the miss roll, then no crit (99) and a ±0 random-damage
        // delta (10) that only a landed hit consumes.
        world.force_rolls([roll, 99, 10]);
        handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
        let before = nvit(&world, npc_oid).cur_hp;
        advance_world(&mut world, 12);
        (before, nvit(&world, npc_oid).cur_hp)
    };

    let (before_day, after_day) = swing_at(midnight + (IG_DAY_MS / 2));
    assert!(
        after_day < before_day,
        "by day the swing lands ({before_day} → {after_day})"
    );

    let (before_night, after_night) = swing_at(midnight);
    assert_eq!(
        after_night, before_night,
        "the same swing at the same roll misses at night"
    );
}

/// Regression: the Ctrl-click force-attack. Java's `ClientPackets` binds *both*
/// `ATTACK` (0x01) and `ATTACK_REQUEST` (0x32) to `AttackRequest`; the Interlude
/// client sends 0x01 on a Ctrl-click. It must route through `on_packet` to the
/// attack handler, and — since a Ctrl-click is a *force attack* — one click both
/// selects the target (`MyTargetSelected`) and engages it (`Attack` intent +
/// broadcast), without waiting for a second click. Before the 0x01 arm existed
/// the packet fell through to the unhandled branch and nothing happened.
#[test]
fn ctrl_click_opcode_0x01_switches_target_and_attacks() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 30;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 20, 0, 0, 100_000, 30);
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

    // A single Ctrl-click with no current target: routes to the handler,
    // switches the target AND engages the attack in one click (force attack).
    world.force_rolls([0, 99, 10]);
    let ctrl_click = [vec![cop::ATTACK], attack_request_body(npc_oid)].concat();
    on_packet(&mut world, 1, ctrl_click);
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(npc_oid),
        "0x01 selects the clicked target"
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MY_TARGET_SELECTED),
        "target switch sends MyTargetSelected"
    );
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(model::PlayerIntent::Attack { .. }))
        ),
        "one Ctrl-click engages the attack intent"
    );
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ATTACK),
        "Attack broadcast on the same click"
    );
}

/// **There is no `dontMove` for melee.** `AttackRequest` reads its trailing
/// byte ("0 for simple click 1 for shift-click") into `_attackId`, a field Java
/// marks `@SuppressWarnings("unused")` and never reads again — so a shift-click
/// walks the target down exactly like a plain click does. The port used to
/// refuse with SM 22 instead, which is a behaviour retail does not have.
#[test]
fn shift_attack_request_chases_because_java_discards_the_flag() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 33;
    // 200 units away — beyond reach 20 + 0 + 10 = 30.
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 200, 0, 0, 5000, 30);
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

    // Shift-attack the far mob: selects it AND engages, same as a plain click.
    on_packet(
        &mut world,
        1,
        [vec![cop::ATTACK], attack_request_body_shift(npc_oid, true)].concat(),
    );
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(npc_oid),
        "shift-attack selects the target"
    );
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(model::PlayerIntent::Attack { .. }))
        ),
        "and engages it — the shift byte changes nothing"
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "the chase starts"
    );
    assert!(
        !packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::YOUR_TARGET_IS_OUT_OF_RANGE),
        "and no out-of-range refusal — Java has no melee dontMove to refuse with"
    );

    // A plain (non-shift) attack behaves identically.
    on_packet(
        &mut world,
        1,
        [vec![cop::ATTACK], attack_request_body(npc_oid)].concat(),
    );
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(model::PlayerIntent::Attack { .. }))
        ),
        "a non-shift attack engages too"
    );
}

/// Chasing: an `AttackRequest` from out of melee reach walks the player
/// toward the monster (`MoveToPawn`) and only swings once in reach; the hurt
/// monster retaliates through its AI think (run mode + `Attack` back), and
/// its damage bites the player's HP directly (no CP soak from NPCs).
#[test]
fn attack_out_of_reach_chases_and_monster_retaliates() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 8;
    // 200 units away — beyond reach 20 + 0 + 10 = 30; big HP pool so the
    // monster survives and hits back.
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 200, 0, 0, 5000, 30);
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
    drain(&mut a_rx);

    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "out of reach: chase starts, no swing yet"
    );
    assert!(
        !packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ATTACK)
    );

    // Player run speed 115 u/s over ~170 units ⇒ in reach in ~1.5 s. Force
    // every swing in the window to a plain hit: each non-miss swing rolls
    // miss(1000), shield-rate(100), shield-perfect(100), crit(100), random(2r+1)
    // — [0, 0, 0, 99, 10] = hit / no shield / no crit / random mul 1.0. Repeated
    // generously so all swings (player + monster) in the window stay deterministic.
    for _ in 0..8 {
        world.force_rolls([0, 0, 0, 99, 10]);
    }
    let hp_before = pvit(&world, 3001).cur_hp;
    let cp_before = pcp(&world, 3001).cur_cp;
    advance_world(&mut world, 45);

    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| is_for(p, server_packets::opcodes::ATTACK, 3001)),
        "player swung after closing in"
    );
    assert!(nvit(&world, npc_oid).cur_hp < 5000.0, "monster took damage");
    assert_eq!(
        world
            .objects
            .get_component::<NpcAi>(&npc_oid)
            .unwrap()
            .intention,
        NpcIntention::Attack
    );
    assert!(
        world
            .objects
            .get_component::<Speeds>(&npc_oid)
            .unwrap()
            .running,
        "aggroed monsters run"
    );
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::CHANGE_MOVE_TYPE),
        "run-mode broadcast"
    );
    assert!(
        packets
            .iter()
            .any(|p| is_for(p, server_packets::opcodes::ATTACK, npc_oid)),
        "monster swung back"
    );
    assert!(pvit(&world, 3001).cur_hp < hp_before, "player HP bitten");
    assert_eq!(
        pcp(&world, 3001).cur_cp,
        cp_before,
        "no CP soak from NPC hits"
    );
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2),
        "victim damage message"
    );
}
