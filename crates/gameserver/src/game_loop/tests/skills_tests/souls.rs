//! Souls (Soul Mastery's absorb, Life to Soul, the cast-finish spend and the
//! ten-minute decay) and the `MAX_MOMENTUM`-driven Force effects.

use super::*;
use crate::game_loop::skills::effects;

fn souls(world: &World, oid: i32) -> i32 {
    world.objects.get_component::<Player>(&oid).unwrap().souls
}

fn charges(world: &World, oid: i32) -> i32 {
    world.objects.get_component::<Player>(&oid).unwrap().charges
}

fn set_souls(world: &mut World, oid: i32, n: i32) {
    world
        .objects
        .get_component_mut::<Player>(&oid)
        .unwrap()
        .souls = n;
}

/// Learn `(id, level)` pairs and fold the passives in.
fn learn(world: &mut World, oid: i32, skills: &[(i32, i32)]) {
    let book = &mut world
        .objects
        .get_component_mut::<SkillBook>(&oid)
        .unwrap()
        .0;
    for &(id, level) in skills {
        book.insert(id, level);
    }
    passive_skills::refresh_conditioned_passives(world, oid);
}

/// Enough HP and MP for any cast's consume, set after `learn` (whose passive
/// refresh recomputes the pools).
fn fill_vitals(world: &mut World, oid: i32) {
    let v = &mut world.objects.get_component_mut::<Vitals>(&oid).unwrap();
    v.max_hp = 100_000;
    v.cur_hp = 100_000.0;
    v.max_mp = 10_000;
    v.cur_mp = 10_000.0;
}

/// The souls byte of the last `EtcStatusUpdate` (0xF9) in `packets`: opcode,
/// charges, weight int, weapon, armor, death, then souls.
fn last_etc_souls(packets: &[Vec<u8>]) -> Option<u8> {
    packets.iter().rev().find(|p| p[0] == 0xF9).map(|p| p[9])
}

/// Soul Mastery (467) level 1: `maxSouls` 5, `expNeeded` 73. An exp gain of
/// at least 73 absorbs one soul (SM 2162, `EtcStatusUpdate` carrying it); a
/// smaller one does nothing; at the cap SM 2186 and no gain.
#[test]
fn soul_mastery_absorbs_a_soul_from_a_big_enough_exp_gain() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();
    let mut rx = ingame_player_access(&mut world, 1, 5601, 0);
    learn(&mut world, 5601, &[(467, 1)]);
    assert_eq!(
        effects::souls::max_souls(&world, 5601),
        5,
        "the pump sets MAX_SOULS"
    );
    drain(&mut rx);

    death::add_exp_and_sp(&mut world, 5601, 72.0, 0.0, false);
    assert_eq!(souls(&world, 5601), 0, "72 exp is under expNeeded");

    death::add_exp_and_sp(&mut world, 5601, 73.0, 0.0, false);
    assert_eq!(souls(&world, 5601), 1, "73 exp absorbs one");
    let packets = drain(&mut rx);
    assert!(has_system_message(
        &packets,
        server_packets::sm_ids::YOUR_SOUL_COUNT_HAS_INCREASED_BY_S1_IT_IS_NOW_AT_S2
    ));
    assert_eq!(last_etc_souls(&packets), Some(1), "EtcStatusUpdate souls");

    set_souls(&mut world, 5601, 5);
    death::add_exp_and_sp(&mut world, 5601, 1000.0, 0.0, false);
    assert_eq!(souls(&world, 5601), 5, "no gain past the cap");
    assert!(has_system_message(
        &drain(&mut rx),
        server_packets::sm_ids::SOUL_CANNOT_BE_ABSORBED_ANYMORE
    ));
}

/// Without Soul Mastery nothing is absorbed: the listener only exists while
/// the passive does.
#[test]
fn no_soul_mastery_no_souls() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();
    let _rx = ingame_player_access(&mut world, 1, 5602, 0);
    death::add_exp_and_sp(&mut world, 5602, 10_000.0, 0.0, false);
    assert_eq!(souls(&world, 5602), 0);
}

/// Life to Soul (502): `OpSoulMax` gates it on room below `MAX_SOULS`, and its
/// `FocusSouls` adds one. With no Soul Mastery `MAX_SOULS` is 0, so it is
/// refused outright; at the cap it is refused too.
#[test]
fn life_to_soul_fills_souls_until_op_soul_max_refuses() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();
    let _rx = ingame_player_access(&mut world, 1, 5603, 0);
    learn(&mut world, 5603, &[(502, 1)]);
    fill_vitals(&mut world, 5603);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(502, false));
    advance_world(&mut world, 20);
    assert_eq!(souls(&world, 5603), 0, "MAX_SOULS 0: OpSoulMax refuses");

    learn(&mut world, 5603, &[(467, 1)]);
    fill_vitals(&mut world, 5603);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(502, false));
    advance_world(&mut world, 20); // hitTime 900 ms
    assert_eq!(souls(&world, 5603), 1, "FocusSouls charge 1");

    set_souls(&mut world, 5603, 5);
    advance_world(&mut world, 120); // reuse 11 s
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(502, false));
    advance_world(&mut world, 20);
    assert_eq!(souls(&world, 5603), 5, "at the cap: refused");
}

/// `FocusSouls` itself: tops up to the cap rather than past it, and at the cap
/// sends SM 2163.
#[test]
fn focus_souls_caps_at_max_souls() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();
    let mut rx = ingame_player_access(&mut world, 1, 5604, 0);
    learn(&mut world, 5604, &[(467, 1)]);
    set_souls(&mut world, 5604, 3);
    drain(&mut rx);

    effects::souls::focus_souls(&mut world, 5604, 50);
    assert_eq!(souls(&world, 5604), 5, "3 + 50 clamps to 5");
    effects::souls::focus_souls(&mut world, 5604, 1);
    assert_eq!(souls(&world, 5604), 5);
    assert!(has_system_message(
        &drain(&mut rx),
        server_packets::sm_ids::SOUL_CANNOT_BE_INCREASED_ANYMORE
    ));
}

/// Shining Edge (505, `soulMaxConsumeCount` 5): `finishSkill` spends up to 5
/// souls whether or not the blow lands.
#[test]
fn a_soul_skill_spends_its_souls_at_cast_finish() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();
    let _rx = ingame_player_access(&mut world, 1, 5605, 0);
    learn(&mut world, 5605, &[(505, 1)]);
    arm(&mut world, 5605, 9209); // Rapier — `EquipWeapon` RAPIER
    set_souls(&mut world, 5605, 7);
    fill_vitals(&mut world, 5605);

    let npc_oid = NPC_OID + 3;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 20001, 20, 0, 0, 100_000, 30);
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
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(505, false));
    advance_world(&mut world, 40); // hitTime 1500 ms
    assert_eq!(souls(&world, 5605), 2, "7 - 5");
}

/// `ResetSoulsTask`: ten minutes after the last gain the souls clear; a later
/// gain restarts the clock, so the first task fires harmlessly.
#[test]
fn souls_decay_after_ten_minutes_and_a_gain_restarts_the_clock() {
    const DECAY: u64 = 6_000;
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();
    let _rx = ingame_player_access(&mut world, 1, 5606, 0);

    effects::souls::increase_souls(&mut world, 5606, 2);
    advance_world(&mut world, DECAY / 2);
    effects::souls::increase_souls(&mut world, 5606, 1);
    advance_world(&mut world, DECAY / 2 + 100);
    assert_eq!(souls(&world, 5606), 3, "the first deadline was superseded");
    advance_world(&mut world, DECAY / 2);
    assert_eq!(souls(&world, 5606), 0, "the second one clears them");

    // A spend that leaves some restarts it; one that empties the pool stops it.
    effects::souls::increase_souls(&mut world, 5606, 3);
    advance_world(&mut world, DECAY / 2);
    effects::souls::decrease_souls(&mut world, 5606, 1);
    advance_world(&mut world, DECAY / 2 + 100);
    assert_eq!(souls(&world, 5606), 2, "restarted by the partial spend");
    advance_world(&mut world, DECAY / 2);
    assert_eq!(souls(&world, 5606), 0);
}

/// `FocusMaxMomentum`: charges straight to `MAX_MOMENTUM` (Sonic Mastery 992
/// level 3 → 3 while a sword is equipped), with SM 323. Bare-handed the
/// passive's `EquipWeapon` condition is off, so `MAX_MOMENTUM` is 0.
#[test]
fn focus_max_momentum_fills_to_max_momentum() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();
    let mut rx = ingame_player_access(&mut world, 1, 5607, 0);
    learn(&mut world, 5607, &[(992, 3)]);
    effects::focus_max_momentum(&mut world, 5607);
    assert_eq!(charges(&world, 5607), 0, "bare-handed: no MAX_MOMENTUM");

    arm(&mut world, 5607, 2);
    passive_skills::refresh_conditioned_passives(&mut world, 5607);
    drain(&mut rx);

    effects::focus_max_momentum(&mut world, 5607);
    assert_eq!(charges(&world, 5607), 3);
    assert!(has_system_message(
        &drain(&mut rx),
        server_packets::sm_ids::YOUR_FORCE_HAS_INCREASED_TO_LEVEL_S1
    ));
}

/// Momentum Charge (10280): a toggle whose `GetMomentum` adds a charge every
/// 15 ticks on its own period — not the 5-tick `DamOverTime` one beside it.
#[test]
fn momentum_charge_gains_force_on_its_own_period() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();
    let _rx = ingame_player_access(&mut world, 1, 5608, 0);
    arm(&mut world, 5608, 2);
    learn(&mut world, 5608, &[(992, 8), (10280, 1)]);
    fill_vitals(&mut world, 5608);
    let period = effects::dot_interval_ticks(15, world.cfg.character.effect_tick_ratio_ms);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(10280, false));
    advance_world(&mut world, period - 1);
    assert_eq!(charges(&world, 5608), 0, "not before the first period");
    advance_world(&mut world, 1);
    assert_eq!(charges(&world, 5608), 1, "one per period");
    advance_world(&mut world, period);
    assert_eq!(charges(&world, 5608), 2);
}

/// SoulBlow's boost reads the souls left **after** the cast-finish spend:
/// 10 souls, Shining Edge spends 5, and the blow is ×(1 + 5·0.048); with 5 it
/// spends them all and the blow is unboosted. The two worlds are otherwise
/// identical and every roll is pinned, so the ratio is exact.
#[test]
fn soul_blow_is_boosted_by_the_souls_left_after_the_spend() {
    let blow = |start_souls: i32| {
        let (mut world, ..) = test_world();
        world.data = dist::game_data_owned();
        let _rx = ingame_player_access(&mut world, 1, 5609, 0);
        learn(&mut world, 5609, &[(505, 1)]);
        arm(&mut world, 5609, 9209);
        set_souls(&mut world, 5609, start_souls);
        fill_vitals(&mut world, 5609);
        let npc_oid = NPC_OID + 4;
        let (npc, extra) = model::npc::Npc::for_test(npc_oid, 20001, 20, 0, 0, 100_000, 30);
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
        let before = pvit(&world, npc_oid).cur_hp;
        // Every roll pinned to 0 (the blow lands, lowest random multiplier),
        // so both worlds draw the same outcomes whatever consumes them.
        world.force_rolls(std::iter::repeat_n(0, 1_000));
        handle_action(&mut world, 1, &action_body(npc_oid, 0));
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(505, false));
        // Read the HP on the tick the cast finishes (the soul spend marks it),
        // before `nextAction ATTACK`'s swings add their own damage.
        for _ in 0..40 {
            advance_world(&mut world, 1);
            if souls(&world, 5609) != start_souls {
                break;
            }
        }
        assert_eq!(souls(&world, 5609), (start_souls - 5).max(0));
        before - pvit(&world, npc_oid).cur_hp
    };
    let (plain, boosted) = (blow(5), blow(10));
    assert!(plain > 0.0, "the blow landed: {plain}");
    assert!(
        (boosted / plain - 1.24).abs() < 1e-9,
        "×1.24 with 5 souls left: {plain} -> {boosted}"
    );
}
