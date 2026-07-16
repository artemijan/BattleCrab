use super::*;

/// `Action` on a monster tints `MyTargetSelected` with the level gap; a second
/// click on the already-targeted (out-of-range) monster starts the attack and
/// walks toward it (`MoveToPawn`) — never a chat window.
#[test]
fn action_on_monster_colors_target_and_never_talks() {
    let (mut world, ..) = test_world();
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 3, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<crate::model::Player>(&3001).unwrap().level = 8;

    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::VALIDATE_LOCATION);
    let mts = rx.try_recv().unwrap();
    assert_eq!(mts[0], server_packets::opcodes::MY_TARGET_SELECTED);
    assert_eq!(i16::from_le_bytes(mts[9..11].try_into().unwrap()), 5, "player 8 vs monster 3");
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE);
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);

    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    let after: Vec<Vec<u8>> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert!(
        after.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "second click starts the attack and walks the out-of-range monster down"
    );
    assert!(
        !after.iter().any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE),
        "no chat window from a monster"
    );
}

/// PvP flag lifecycle (`Player.updatePvPStatus` + `PvpFlagTaskManager`): a
/// hostile action flags the player solid (1), the 1 s sweep blinks it (2) in
/// the final 20 s, then clears it (0) past expiry.
#[test]
fn pvp_flag_starts_blinks_and_expires() {
    use crate::game_loop::pvp;
    use crate::model::components::PvpState;
    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let start = world.tick;

    pvp::update_pvp_status(&mut world, 5001);
    let st = *world.objects.get_component::<PvpState>(&5001).unwrap();
    assert_eq!(st.flag, 1, "flagged solid");
    assert_eq!(st.expires_tick, start + 1200, "PVP_NORMAL_TIME = 120 s @ 100 ms ticks");

    // Mid-life (before the last 20 s) stays solid.
    world.tick = start + 900;
    pvp::pvp_flag_tick(&mut world);
    assert_eq!(world.objects.get_component::<PvpState>(&5001).unwrap().flag, 1);

    // Final 20 s (200 ticks) → blinking (2).
    world.tick = start + 1100;
    pvp::pvp_flag_tick(&mut world);
    assert_eq!(world.objects.get_component::<PvpState>(&5001).unwrap().flag, 2, "blinks in the last 20 s");

    // Past expiry → cleared.
    world.tick = start + 1200;
    pvp::pvp_flag_tick(&mut world);
    assert_eq!(world.objects.get_component::<PvpState>(&5001).unwrap().flag, 0, "cleared past expiry");
}

/// `updatePvPStatus(target)`: attacking a clean player flags for
/// `PVP_NORMAL_TIME`; attacking an already-flagged/PK player flags for the
/// shorter `PVP_PVP_TIME` (`checkIfPvP`). Attacking a PK doesn't flag at all.
#[test]
fn pvp_flag_duration_depends_on_target_state() {
    use crate::game_loop::pvp;
    use crate::model::components::PvpState;
    let (mut world, ..) = test_world();
    let _a = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 5002, 50, 0, 0);
    let start = world.tick;

    // A attacks a clean B → 120 s.
    pvp::update_pvp_status_target(&mut world, 5001, 5002);
    assert_eq!(world.objects.get_component::<PvpState>(&5001).unwrap().expires_tick, start + 1200);

    // B (clean) attacks the now-flagged A → 60 s (checkIfPvP true).
    world.tick = start + 10;
    pvp::update_pvp_status_target(&mut world, 5002, 5001);
    assert_eq!(world.objects.get_component::<PvpState>(&5002).unwrap().expires_tick, start + 10 + 600, "PVP time vs a flagged target");

    // Attacking a PK doesn't flag the attacker (target freely attackable).
    world.objects.get_component_mut::<Player>(&5002).unwrap().reputation = -1;
    world.objects.get_component_mut::<PvpState>(&5001).unwrap().flag = 0;
    world.objects.get_component_mut::<PvpState>(&5001).unwrap().expires_tick = 0;
    pvp::update_pvp_status_target(&mut world, 5001, 5002);
    assert_eq!(world.objects.get_component::<PvpState>(&5001).unwrap().flag, 0, "no flag for attacking a PK");
}

/// `isAutoAttackable` relation for players: a clean player needs Ctrl (not
/// auto-attackable), a flagged or PK one does not.
#[test]
fn flagged_or_pk_player_is_auto_attackable() {
    use crate::game_loop::pvp;
    let (mut world, ..) = test_world();
    let _a = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 5002, 50, 0, 0);

    assert!(!pvp::is_player_auto_attackable(&world, 5001, 5002), "clean player needs force");

    pvp::update_pvp_status(&mut world, 5002);
    assert!(pvp::is_player_auto_attackable(&world, 5001, 5002), "flagged player is attackable");

    world.objects.get_component_mut::<crate::model::components::PvpState>(&5002).unwrap().flag = 0;
    world.objects.get_component_mut::<Player>(&5002).unwrap().reputation = -1;
    assert!(pvp::is_player_auto_attackable(&world, 5001, 5002), "PK is attackable");
}

/// Arena (`ArenaZone`/`ZoneId.PVP`): both players in a PVP zone are freely
/// auto-attackable, and hostile actions there don't raise a flag.
#[test]
fn arena_players_attackable_without_flagging() {
    use crate::game_loop::pvp;
    use crate::model::components::{PvpState, ZoneFlags};
    let (mut world, ..) = test_world();
    let _a = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 5002, 30, 0, 0);
    let pvp_bit = crate::data::zone_data::ZoneKind::Pvp.bit();
    world.objects.get_component_mut::<ZoneFlags>(&5001).unwrap().mask = pvp_bit;
    world.objects.get_component_mut::<ZoneFlags>(&5002).unwrap().mask = pvp_bit;

    // Freely attackable (no Ctrl) while both are in the arena.
    assert!(pvp::is_player_auto_attackable(&world, 5001, 5002));
    // Attacking there does not flag the attacker.
    pvp::update_pvp_status_target(&mut world, 5001, 5002);
    assert_eq!(world.objects.get_component::<PvpState>(&5001).unwrap().flag, 0, "no flag inside an arena");
}

/// The full melee kill: AttackRequest → Attack packet + combat stance, the
/// scheduled hit lands with `Formulas` damage, the monster dies (Die), the
/// killer gets XP/SP (level-up: SocialAction 2122 + SM 96), auto-loot adena
/// (SM 28 + InventoryUpdate; memory-first — the loot persists on the next
/// flush, not on pickup), and the corpse decays (DeleteObject) with no respawn
/// for a respawn-less spawn line.
#[test]
fn melee_kill_rewards_and_decay() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    {
        // Level 5 exactly at its threshold +500 (table: L5 = 4000, L6 = 5000).
        let p = world.objects.get_component_mut::<crate::model::Player>(&3001).unwrap();
        p.exp = 4500;
    }
    let npc_oid = NPC_OID + 7;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 100, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);

    // Swing rolls: hit (miss roll 0), no crit (99), random-damage delta 0
    // (roll(21) == 10 → ±0 on rndDam 10).
    world.forced_rolls.extend([0, 99, 10]);
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));

    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::ATTACK), "Attack broadcast");
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::AUTO_ATTACK_START), "combat stance");

    // Expected damage: pAtk × rand(1.0) [+ position bonus] ×77 / pDef.
    // Attacker at (0,0), target heading 0 at (30,0) → attacker is BEHIND.
    let p_atk = pcs(&world, 3001).p_atk;
    let p_def = 40.0 * (5.0 + 89.0) / 100.0;
    let expected = formulas::calc_auto_attack_damage(
        p_atk,
        1.0,
        crate::model::movement::Position::Back,
        p_def,
        false,
        false,
    );
    assert!(expected > 100.0, "sanity: one swing must kill the 100 HP monster ({expected})");

    // Hit lands at timeToHit = 1666 × 0.644 ≈ 1073 ms ⇒ 11 ticks. Queue the
    // drop rolls it will consume on death: level-gap pass (0), chance pass
    // (0 < 70%).
    world.forced_rolls.extend([0, 0]);
    advance_world(&mut world, 12);

    // Monster died: Die broadcast, rewards granted.
    assert!(nvit(&world, npc_oid).dead);
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::DIE
            && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid),
        "Die broadcast for the monster"
    );
    // XP: 2000 × share 1.0 × gap 1.0 (same level) → 4500 + 2000 = 6500 ⇒ level 6.
    let p = &world.objects.get_component::<crate::model::Player>(&3001).expect("player");
    assert_eq!(p.exp, 6500);
    assert_eq!(p.level, 6);
    let cp = pcp(&world, 3001);
    assert_eq!(cp.cur_cp, cp.max_cp as f64, "level-up refills CP");
    assert_eq!(p.sp, 100);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::YOU_HAVE_ACQUIRED_S1_XP_BONUS_S2_AND_S3_SP_BONUS_S4),
        "XP/SP system message"
    );
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION
            && i32::from_le_bytes(p[5..9].try_into().unwrap()) == server_packets::SOCIAL_ACTION_LEVEL_UP),
        "level-up flourish"
    );
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::YOUR_LEVEL_HAS_INCREASED),
        "level-up message"
    );
    // Auto-loot: 5 adena in the inventory, SM 28, persisted via InsertItem.
    let inv = world.objects.get_component::<crate::model::inventory::Inventory>(&3001).unwrap();
    let adena = inv.items().iter().find(|i| i.item_id == 57).expect("looted adena");
    assert_eq!(adena.count, 5);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::YOU_HAVE_OBTAINED_S1_ADENA),
        "obtained-adena message"
    );
    // Memory-first: loot lands in the Inventory component (adena count asserted
    // above); it persists on the next flush, not on pickup.

    // The attack intent drops on the next combat tick (dead target).
    advance_world(&mut world, 1);
    assert!(!world.objects.has_component::<Intent>(&3001));

    // Decay after the 2 s corpse time: DeleteObject, corpse gone, no respawn
    // scheduled (respawn_secs == 0).
    advance_world(&mut world, 20);
    assert!(!world.objects.has_component::<crate::model::npc::Npc>(&npc_oid));
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT),
        "corpse DeleteObject"
    );
    assert!(world.scheduler.is_empty(), "no respawn for a respawn-less spawn line");
}

/// The dead mob stays *selected* for its whole corpse window (so future
/// sweep/loot logic can act on the selected corpse); the target is released
/// only when it decays. At decay, `TargetUnselected` goes to *every* player who
/// still had it selected — not just the killer — clearing each ground ring (our
/// client keeps a dead/deleted target locked without the packet). Each
/// server-side `TargetRef` is cleared too.
#[test]
fn decaying_mob_sends_target_unselected_to_all_holders() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    // A second player nearby who also has the mob targeted but did not kill it.
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 20, 0);
    let npc_oid = NPC_OID + 11;
    add_test_npc(&mut world, npc_oid, 40001, "Monster", 5, 40, 0, 0);

    // Both players select the mob (each client now shows its target ring).
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    handle_action(&mut world, 2, &action_body(npc_oid, 0));
    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, Some(npc_oid));
    assert_eq!(world.objects.get_component::<TargetRef>(&3002).unwrap().0, Some(npc_oid));
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Player 1 lands the kill — the corpse stays selected (sweep window).
    death::npc_do_die(&mut world, npc_oid, 3001);
    let got_unselect = |packets: &[Vec<u8>], player_oid: i32| {
        packets.iter().any(|p| p[0] == server_packets::opcodes::TARGET_UNSELECTED
            && i32::from_le_bytes(p[1..5].try_into().unwrap()) == player_oid)
    };
    assert!(!got_unselect(&drain(&mut a_rx), 3001), "no TargetUnselected at death");
    assert!(!got_unselect(&drain(&mut b_rx), 3002), "no TargetUnselected at death");
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(npc_oid),
        "corpse stays selected while it lasts (for sweep/loot)"
    );
    assert_eq!(world.objects.get_component::<TargetRef>(&3002).unwrap().0, Some(npc_oid));

    // Corpse decays → both clients get their own TargetUnselected (payload
    // carries the *deselecting* player's id) and both server-side targets clear.
    death::handle_npc_decay(&mut world, npc_oid);
    assert!(got_unselect(&drain(&mut a_rx), 3001), "killer's ring clears at decay");
    assert!(got_unselect(&drain(&mut b_rx), 3002), "onlooker's ring clears at decay");
    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, None);
    assert_eq!(world.objects.get_component::<TargetRef>(&3002).unwrap().0, None);
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
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 20, 0, 0, 100_000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    // A single Ctrl-click with no current target: routes to the handler,
    // switches the target AND engages the attack in one click (force attack).
    world.forced_rolls.extend([0, 99, 10]);
    let ctrl_click = [vec![cop::ATTACK], attack_request_body(npc_oid)].concat();
    on_packet(&mut world, 1, ctrl_click);
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(npc_oid),
        "0x01 selects the clicked target"
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::MY_TARGET_SELECTED),
        "target switch sends MyTargetSelected"
    );
    assert!(
        matches!(world.objects.get_component::<Intent>(&3001), Some(Intent(crate::model::PlayerIntent::Attack { .. }))),
        "one Ctrl-click engages the attack intent"
    );
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::ATTACK), "Attack broadcast on the same click");
}

/// Shift-click is `dontMove`: an out-of-reach shift-attack refuses to chase and
/// fails with "your target is out of range" (SM 22) + `ActionFailed`, leaving no
/// attack intent and no movement. A plain (non-shift) attack on the same mob
/// chases instead — the contrast the shift flag controls. (Java discards the
/// byte; this is a deliberate enhancement.)
#[test]
fn shift_attack_out_of_reach_fails_instead_of_chasing() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 33;
    // 200 units away — beyond reach 20 + 0 + 10 = 30.
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 200, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    // Shift-attack the far mob: selects it, but refuses to move.
    on_packet(&mut world, 1, [vec![cop::ATTACK], attack_request_body_shift(npc_oid, true)].concat());
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(npc_oid),
        "shift-attack still selects the target"
    );
    assert!(!world.objects.has_component::<Intent>(&3001), "no attack intent — dontMove");
    assert!(!world.objects.has_component::<Movement>(&3001), "no chase — dontMove");
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::YOUR_TARGET_IS_OUT_OF_RANGE),
        "out-of-range system message"
    );
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::ACTION_FAIL), "ActionFailed");

    // Contrast: a plain (non-shift) attack on the same mob DOES chase.
    on_packet(&mut world, 1, [vec![cop::ATTACK], attack_request_body(npc_oid)].concat());
    assert!(
        matches!(world.objects.get_component::<Intent>(&3001), Some(Intent(crate::model::PlayerIntent::Attack { .. }))),
        "a non-shift attack engages (and will chase)"
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
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 200, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);

    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "out of reach: chase starts, no swing yet"
    );
    assert!(!packets.iter().any(|p| p[0] == server_packets::opcodes::ATTACK));

    // Player run speed 115 u/s over ~170 units ⇒ in reach in ~1.5 s. Force
    // every swing in the window to a plain hit: each non-miss swing rolls
    // miss(1000), shield-rate(100), shield-perfect(100), crit(100), random(2r+1)
    // — [0, 0, 0, 99, 10] = hit / no shield / no crit / random mul 1.0. Repeated
    // generously so all swings (player + monster) in the window stay deterministic.
    for _ in 0..8 {
        world.forced_rolls.extend([0, 0, 0, 99, 10]);
    }
    let hp_before = pvit(&world, 3001).cur_hp;
    let cp_before = pcp(&world, 3001).cur_cp;
    advance_world(&mut world, 45);

    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::ATTACK
        && i32::from_le_bytes(p[1..5].try_into().unwrap()) == 3001), "player swung after closing in");
    assert!(nvit(&world, npc_oid).cur_hp < 5000.0, "monster took damage");
    assert_eq!(world.objects.get_component::<crate::model::npc::NpcAi>(&npc_oid).unwrap().intention, crate::model::npc::NpcIntention::Attack);
    assert!(world.objects.get_component::<Speeds>(&npc_oid).unwrap().running, "aggroed monsters run");
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::CHANGE_MOVE_TYPE),
        "run-mode broadcast"
    );
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::ATTACK
            && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid),
        "monster swung back"
    );
    assert!(pvit(&world, 3001).cur_hp < hp_before, "player HP bitten");
    assert_eq!(pcp(&world, 3001).cur_cp, cp_before, "no CP soak from NPC hits");
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2),
        "victim damage message"
    );
}

/// An idle monster with random walk enabled wanders: with no target and
/// inside its drift radius, the 1-in-30 roll fires and it moves to a random
/// spot near its spawn, broadcasting `MoveToLocation`
/// (`AttackableAI.thinkActive`'s random-walk branch).
#[test]
fn idle_monster_random_walks_near_spawn() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    {
        // 40001 is passive (won't aggro the nearby player) but wanders.
        let mut t = world.data.npc_data.get(40001).unwrap().clone();
        t.random_walk = true;
        world.data.npc_data.insert_for_test(t);
    }
    // A player keeps the spawn region active so `npc_ai_tick` visits the mob.
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 0, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // Force the walk-rate hit (0) and a delta landing well within drift (300):
    // deltaX = 500, deltaY = 500 + 83 = 583 → √(583²−500²) ≈ 299 → (200, −1).
    world.forced_rolls.extend([0, 500, 83]);
    npc_ai::npc_ai_tick(&mut world);

    let mv = world.objects.get_component::<Movement>(&npc_oid).expect("idle mob started a random walk");
    let from_spawn = ((mv.0.dest_x as f64).powi(2) + (mv.0.dest_y as f64).powi(2)).sqrt();
    assert!(from_spawn <= world.cfg.npc.max_drift_range as f64, "wander destination stays within drift range");
    assert!((mv.0.dest_x, mv.0.dest_y) != (0, 0), "actually moved off the spawn spot");
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_LOCATION
            && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid),
        "the wander is broadcast as MoveToLocation"
    );
}

/// An idle NPC in an active region plays a random social animation once its
/// pending timer elapses, broadcasting `SocialAction` with id 2 or 3
/// (`RandomAnimationTaskManager` → `onRandomAnimation`).
#[test]
fn idle_npc_plays_random_social_animation() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 0, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    // Pretend the animation timer already elapsed (skip the 5–60 s wait).
    world.tick = 100;
    world.objects.get_component_mut::<crate::model::npc::NpcAi>(&npc_oid).unwrap().next_animation_tick = Some(50);
    drain(&mut a_rx);

    npc_ai::npc_ai_tick(&mut world);

    let packets = drain(&mut a_rx);
    let social = packets
        .iter()
        .find(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid)
        .expect("idle NPC broadcast a SocialAction");
    let action_id = i32::from_le_bytes(social[5..9].try_into().unwrap());
    assert!((2..=3).contains(&action_id), "random idle animation is 2 or 3, got {action_id}");
    // The 6 s throttle is now armed and the next attempt was rescheduled out.
    let ai = world.objects.get_component::<crate::model::npc::NpcAi>(&npc_oid).unwrap();
    assert_eq!(ai.last_social_tick, 100);
    assert!(ai.next_animation_tick.unwrap() > 100, "next animation rescheduled into the future");
}

/// A moving NPC does not play idle animations even when its timer is due
/// (Java gates on `!isMoving()`).
#[test]
fn moving_npc_skips_random_animation() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 0, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    world.tick = 100;
    world.objects.get_component_mut::<crate::model::npc::NpcAi>(&npc_oid).unwrap().next_animation_tick = Some(50);
    // Currently walking somewhere (`isMoving()`), so no idle animation.
    world.objects.add_components(
        &npc_oid,
        Movement(crate::model::movement::MoveData {
            start_x: 0,
            start_y: 0,
            start_z: 0,
            dest_x: 500,
            dest_y: 0,
            dest_z: 0,
            start_tick: 100,
            total_ticks: 100,
            geo_path: None,
        }),
    );
    drain(&mut a_rx);

    npc_ai::npc_ai_tick(&mut world);

    let packets = drain(&mut a_rx);
    assert!(
        !packets.iter().any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION),
        "a walking NPC plays no idle animation"
    );
    // Still rescheduled, but the throttle stayed unarmed (nothing broadcast).
    let ai = world.objects.get_component::<crate::model::npc::NpcAi>(&npc_oid).unwrap();
    assert_eq!(ai.last_social_tick, 0);
    assert!(ai.next_animation_tick.unwrap() > 100);
}

/// An aggressive monster acquires a player who just stands inside its aggro
/// range: after the spawn-calm `_globalAggro` ticks up to 0, the scan seeds
/// hate and the AI attacks unprovoked.
#[test]
fn aggressive_monster_aggros_idle_player() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    {
        // Make 40001 aggressive for this test.
        let mut t = world.data.npc_data.get(40001).unwrap().clone();
        t.is_aggressive = true;
        t.aggro_range = 300;
        world.data.npc_data.insert_for_test(t);
    }
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 150, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // 10 think seconds of calm (globalAggro −10 → 0), then the scan seeds
    // hate and the AI locks on, chases in, and swings (both swings within
    // 140 ticks forced to plain hits).
    world.forced_rolls.extend([0, 99, 10, 0, 99, 10]);
    advance_world(&mut world, 140);
    assert_eq!(world.objects.get_component::<crate::model::npc::NpcAi>(&npc_oid).unwrap().intention, crate::model::npc::NpcIntention::Attack);
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::ATTACK
            && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid),
        "unprovoked attack on the idle player"
    );
    assert!(pvit(&world, 3001).cur_hp < 100.0, "the swing landed");
}

/// Death and the to-village loop: a killing blow sends `Die` with the
/// to-village flag and applies the XP penalty; `RequestRestartPoint` ports
/// the corpse to the map-region town respawn (`TeleportToLocation`), and
/// `Appearing` revives at the configured 65% HP (`Revive` broadcast).
#[test]
fn player_death_penalty_and_revive_to_village() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    // One town region covering the fight location, respawn at (1000, 1000).
    world.data.map_region = crate::data::MapRegionData::from_regions(vec![crate::data::map_region::MapRegion {
        name: "test_town".into(),
        loc_id: 0,
        respawn_points: vec![(1000, 1000, 7)],
        tiles: vec![(20, 18)],
    }]);
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    {
        let p = world.objects.get_component_mut::<crate::model::Player>(&3001).unwrap();
        p.exp = 4500; // level 5 (threshold 4000) + 500 into the level
        p.level = 5;
    }
    world.objects.get_component_mut::<Vitals>(&3001).unwrap().cur_hp = 1.0;
    world.objects.get_component_mut::<PlayerVitals>(&3001).unwrap().cur_cp = 0.0;
    let npc_oid = NPC_OID + 10;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    // Wake the monster by damage (as if the player had hit it).
    combat::npc_receive_damage(&mut world, npc_oid, 3001, 10.0);
    drain(&mut a_rx);

    // Its swing kills the 1-HP player: force a clean hit.
    world.forced_rolls.extend([0, 99, 10]);
    advance_world(&mut world, 30);

    let p = pvit(&world, 3001);
    assert!(p.dead);
    assert_eq!(p.cur_hp, 0.0);
    // Death penalty: 1% (empty table default) of the 1000-XP level = 10.
    assert_eq!(world.objects.get_component::<crate::model::Player>(&3001).expect("player").exp, 4490);
    let packets = drain(&mut a_rx);
    let die = packets
        .iter()
        .find(|p| p[0] == server_packets::opcodes::DIE && i32::from_le_bytes(p[1..5].try_into().unwrap()) == 3001)
        .expect("player Die packet");
    assert_eq!(i32::from_le_bytes(die[5..9].try_into().unwrap()), 1, "to-village enabled");
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::YOUR_XP_HAS_DECREASED_BY_S1),
        "XP-loss message"
    );

    // To village: teleport to the region respawn point.
    world.forced_rolls.push_back(0); // random respawn-point pick
    handle_request_restart_point(&mut world, 1, &{
        let mut w = PacketWriter::new();
        w.write_i32(0); // TO_VILLAGE
        w.into_bytes()
    });
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (1000, 1000, 12), "respawn point z lifted by 5 (teleToLocation)");
    let p = &world.objects.get_component::<crate::model::Player>(&3001).expect("player");
    assert!(p.teleporting && p.pending_revive && pvit(&world, 3001).dead);
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION));

    // Client finished loading: Appearing → revive at 65% HP.
    on_packet(&mut world, 1, vec![cp::opcodes::APPEARING]);
    let p = &world.objects.get_component::<crate::model::Player>(&3001).expect("player");
    assert!(!pvit(&world, 3001).dead && !p.teleporting && !p.pending_revive);
    let v = pvit(&world, 3001);
    assert_eq!(v.cur_hp, v.max_hp as f64 * 0.65);
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::REVIVE));
}

/// The decay → respawn loop over a real spawn line: the corpse decays
/// (`DeleteObject`), `Spawn.decreaseCount` schedules the respawn, and the
/// respawned NPC (fresh object id) is announced with `NpcInfo`.
#[test]
fn dead_monster_decays_and_respawns() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    world.data.spawn_data = crate::data::SpawnData {
        spawns: vec![crate::data::spawn_data::SpawnTemplate {
            name: None,
            territories: vec![],
            groups: vec![crate::data::spawn_data::SpawnGroup {
                territories: vec![],
                npcs: vec![crate::data::spawn_data::NpcSpawnDef {
                    npc_id: 40001,
                    count: 1,
                    loc: Some(crate::data::spawn_data::FixedLoc { x: 30, y: 0, z: 0, heading: 0 }),
                    respawn_secs: 3,
                    respawn_random_secs: 0,
                }],
            }],
        }],
    };
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = crate::model::npc::spawn_one(&mut world, 0, 0, 0).expect("spawned");
    world.objects.get_component_mut::<TargetRef>(&3001).unwrap().0 = Some(npc_oid);
    drain(&mut a_rx);

    // Kill it outright (drop level-gap roll forced to fail: no loot noise).
    world.forced_rolls.push_back(999_999);
    combat::npc_receive_damage(&mut world, npc_oid, 3001, 1_000_000.0);
    assert!(nvit(&world, npc_oid).dead);

    // Decay at +2 s: corpse gone, DeleteObject seen, dangling target dropped,
    // respawn pending.
    advance_world(&mut world, 21);
    assert!(!world.objects.has_component::<crate::model::npc::Npc>(&npc_oid));
    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, None);
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT));

    // Respawn at +3 s more: a fresh NPC on the same spawn line, announced.
    advance_world(&mut world, 31);
    let mut respawned_ids: Vec<i32> = Vec::new();
    world.objects.for_each_mut::<&crate::model::npc::Npc>(|n| {
        if n.npc_id == 40001 {
            respawned_ids.push(n.object_id);
        }
    });
    let respawned_oid = *respawned_ids.first().expect("respawned");
    assert_ne!(respawned_oid, npc_oid, "transient ids are not reused");
    let rpos = world.objects.get_component::<Position>(&respawned_oid).unwrap();
    assert_eq!((rpos.x, rpos.y, rpos.z), (30, 0, 0));
    assert!(!nvit(&world, respawned_oid).dead);
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::NPC_INFO),
        "respawn announced with NpcInfo"
    );
}

/// The `on_spawn` hook fires for registered NPCs on every (re)spawn — a
/// synthetic script stamps the NPC's script value at spawn.
#[test]
fn on_spawn_hook_fires_for_registered_npcs() {
    struct SpawnStamp;
    impl crate::game_loop::quests::QuestScript for SpawnStamp {
        fn id(&self) -> i32 {
            -1
        }
        fn name(&self) -> &'static str {
            "SpawnStamp"
        }
        fn html_dir(&self) -> &'static str {
            ""
        }
        fn start_npcs(&self) -> &[i32] {
            &[]
        }
        fn talk_npcs(&self) -> &[i32] {
            &[]
        }
        fn spawn_npcs(&self) -> &[i32] {
            &[40001]
        }
        fn on_talk(&self, _ctx: &mut crate::game_loop::quests::QuestCtx) -> Option<String> {
            None
        }
        fn on_spawn(&self, ctx: &mut crate::game_loop::quests::QuestCtx) {
            ctx.set_npc_script_value(7);
        }
    }
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    world.quests = std::sync::Arc::new(crate::game_loop::quests::QuestRegistry::new(vec![
        std::sync::Arc::new(SpawnStamp),
    ]));
    // Spawn through the real spawn line (template 40001 registered by
    // combat_test_world's spawn_data? — spawn directly via spawn_one needs
    // a spawn line; use notify path through add_test_npc + explicit call).
    add_test_npc(&mut world, NPC_OID, 40001, "Monster", 5, 30, 0, 0);
    crate::game_loop::quests::notify_spawn(&mut world, NPC_OID, 40001);
    assert_eq!(
        world.objects.get_component::<crate::model::npc::Npc>(&NPC_OID).unwrap().script_value,
        7
    );
}

/// A `SiegeZone` makes the players inside it mutually auto-attackable — but only
/// while that castle's siege is in progress (Java `SiegeZone` active state).
#[test]
fn siege_zone_makes_participants_attackable_only_during_siege() {
    let (mut world, ..) = test_world();
    // Siege zone for castle 3 covering (0,0)..(1000,1000).
    insert_siege_zone(&mut world, 3, 0, 1000, 0, 1000);
    world.sieges.insert(3, crate::model::siege::Siege::new(3));
    let _a = ingame_player(&mut world, 1, 4001, 500, 500, 0);
    let _b = ingame_player(&mut world, 2, 4002, 510, 510, 0);
    let attackable = |w: &World| crate::game_loop::pvp::is_player_auto_attackable(w, 4001, 4002);

    // Zone loaded but siege idle → two unflagged players aren't attackable.
    assert!(!attackable(&world), "no siege PvP while the siege is idle");

    // Siege in progress → both stand in the battlefield → freely attackable.
    world.sieges.get_mut(&3).unwrap().in_progress = true;
    assert!(attackable(&world), "siege PvP once the siege starts");

    // A player outside the siege zone is not part of it (position-based check).
    world.objects.get_component_mut::<Position>(&4002).unwrap().x = 5000;
    assert!(!attackable(&world), "outside the siege zone → not attackable");
}

/// Starting a siege evicts everyone in the battlefield except the owning clan
/// to their nearest town (Java teleportPlayer(NotOwner, TOWN)).
#[test]
fn siege_start_evicts_non_owners_to_town() {
    use crate::model::castle::{Castle, CastleSide};
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::siege::Siege;
    const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (mut world, ..) = test_world();
    world.data.map_region = crate::data::MapRegionData::load_from(ROOT);
    insert_siege_zone(&mut world, 3, 0, 1000, 0, 1000);
    world.castles = vec![Castle { id: 3, name: "Giran".into(), side: CastleSide::Neutral }];
    world.sieges.insert(3, Siege::new(3));
    // Owner clan 500 holds castle 3.
    world.clans.insert(
        500,
        Clan {
            id: 500,
            name: "Owners".into(),
            leader_id: 9002,
            level: 5,
            reputation_score: 0,
            castle_id: 3,
            members: vec![ClanMember { char_id: 9002, name: "P9002".into(), level: 40, class_id: 0, sex: 0, race: 0 }],
            warehouse: Default::default(),
        },
    );
    let _o = ingame_player(&mut world, 1, 9002, 500, 500, 0); // owner-clan member in the zone
    let _n = ingame_player(&mut world, 2, 9003, 600, 600, 0); // non-owner in the zone
    world.objects.get_component_mut::<Player>(&9002).unwrap().clan_id = 500;

    crate::game_loop::siege::start_siege(&mut world, 3);

    // Owner-clan member stays in the battlefield.
    let op = *world.objects.get_component::<Position>(&9002).unwrap();
    assert_eq!(world.data.zone_data.siege_castle_at(op.x, op.y, op.z), Some(3), "owner clan holds the castle");
    // Non-owner is teleported out of the siege zone.
    let np = *world.objects.get_component::<Position>(&9003).unwrap();
    assert_ne!(world.data.zone_data.siege_castle_at(np.x, np.y, np.z), Some(3), "non-owner evicted to town");
}

/// Mid-siege capture transfers castle ownership to the attacker and reshuffles
/// siege roles; endSiege then declares the new owner victorious. Port of Java
/// Siege capture (midVictory) + endSiege victory determination.
#[test]
fn siege_capture_transfers_ownership_and_endsiege_declares_victor() {
    use crate::model::castle::{Castle, CastleSide};
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::siege::{Siege, SiegeClanType};
    let (mut world, _db_tx, mut db_rx, _link) = test_world();
    world.castles = vec![Castle { id: 3, name: "Giran".into(), side: CastleSide::Neutral }];
    let mut siege = Siege::new(3);
    siege.add_clan(500, SiegeClanType::Owner); // defender/owner
    siege.add_clan(700, SiegeClanType::Attacker); // attacker
    world.sieges.insert(3, siege);
    let clan = |id: i32, name: &str, leader: i32, castle: i32| Clan {
        id,
        name: name.into(),
        leader_id: leader,
        level: 5,
        reputation_score: 0,
        castle_id: castle,
        members: vec![ClanMember { char_id: leader, name: format!("P{leader}"), level: 40, class_id: 0, sex: 0, race: 0 }],
        warehouse: Default::default(),
    };
    world.clans.insert(500, clan(500, "Defenders", 8002, 3)); // owns castle 3
    world.clans.insert(700, clan(700, "Attackers", 8003, 0));
    let mut rx = ingame_player(&mut world, 1, 8002, 0, 0, 0); // hears the announcements
    drain(&mut rx);

    crate::game_loop::siege::start_siege(&mut world, 3);
    assert_eq!(world.sieges[&3].first_owner_clan_id, 500, "first owner captured at start");
    drain(&mut rx);
    drain_db(&mut db_rx);

    // Capture by attacker clan 700.
    crate::game_loop::siege::capture(&mut world, 3, 700);
    assert_eq!(world.clans[&700].castle_id, 3, "captor now owns the castle");
    assert_eq!(world.clans[&500].castle_id, 0, "old owner lost the castle");
    let role = |cid: i32| world.sieges[&3].clans.iter().find(|c| c.clan_id == cid).map(|c| c.kind);
    assert_eq!(role(700), Some(SiegeClanType::Owner), "captor is the new owner side");
    assert_eq!(role(500), Some(SiegeClanType::Attacker), "old owner becomes an attacker");
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::UpdateClanCastle { clan_id: 700, castle_id: 3 })), "captor persisted");
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::UpdateClanCastle { clan_id: 500, castle_id: 0 })), "old owner cleared");

    // endSiege → the captor (owner changed) is declared victorious.
    crate::game_loop::siege::end_siege(&mut world, 3);
    assert!(!world.sieges[&3].in_progress, "siege ended");
    assert!(
        sm_ids_of(&drain(&mut rx)).contains(&server_packets::sm_ids::CLAN_S1_IS_VICTORIOUS_OVER_S2_S_CASTLE_SIEGE),
        "victor announced"
    );
}

/// Castle doors during a siege: start closes the gates (full HP), a breach
/// (damage to 0) swings a door open, and endSiege revives + closes them. Port
/// of Castle.spawnDoor + the door-breach engine.
#[test]
fn siege_doors_close_on_start_and_breach_on_damage() {
    use crate::data::door_data::DoorOpenMethod;
    use crate::model::door::Door;
    use crate::model::siege::Siege;
    let (mut world, ..) = test_world();
    insert_siege_zone(&mut world, 3, 0, 1000, -1000, 1000); // covers the door at (100, 0)
    world.sieges.insert(3, Siege::new(3));
    let door = crate::model::door::spawn_door_for_test(&mut world, test_door(24190001, DoorOpenMethod::None)); // closed, hp 1000
    crate::game_loop::doors::open_door(&mut world, door);
    assert!(world.geo.doors.is_open(24190001), "door starts open");

    // start_siege → the castle gate is closed at full HP.
    crate::game_loop::siege::start_siege(&mut world, 3);
    assert!(!world.geo.doors.is_open(24190001), "siege closes the gate");
    assert_eq!(world.objects.get_component::<Door>(&door).unwrap().current_hp, 1000, "gate at full HP");

    // Breach: damage to 0 → the gate is destroyed and swings open.
    assert!(crate::game_loop::siege::damage_door(&mut world, door, 1000), "breached this hit");
    assert_eq!(world.objects.get_component::<Door>(&door).unwrap().current_hp, 0, "gate destroyed");
    assert!(world.geo.doors.is_open(24190001), "breached gate swings open");
    // A second hit on the dead gate does nothing.
    assert!(!crate::game_loop::siege::damage_door(&mut world, door, 500), "already breached");

    // endSiege → spawnDoor revives the gate to full HP + closes it.
    crate::game_loop::siege::end_siege(&mut world, 3);
    let d = world.objects.get_component::<Door>(&door).unwrap();
    assert_eq!(d.current_hp, 1000, "revived to full HP");
    assert!(!world.geo.doors.is_open(24190001), "and closed");
}
