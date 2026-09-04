//! Force charges: building them with Focus Momentum, the ten-minute decay,
//! and spending them on an energy attack.

use super::*;

/// G19 `FocusMomentum` effect: Sonic Focus (8, real dist data, level 1 grants
/// max 1 charge) builds "Force" — previously silently dropped, so the
/// warrior Force-builder skills did nothing. First cast lands (0 → 1,
/// already at the level-1 cap) with SM 324 ("reached maximum capacity");
/// recasting at the cap is refused outright (no further gain, same SM).
#[test]
fn focus_momentum_builds_force_and_refuses_past_the_cap() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();

    let mut rx = ingame_player_access(&mut world, 1, 5501, 0);
    drain(&mut rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5501)
        .unwrap()
        .0
        .insert(8, 1);
    // `EquipWeapon` DUAL/DUALBLUNT/SWORD/BLUNT — Long Sword satisfies it.
    arm(&mut world, 5501, 2);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(8, false));
    advance_world(&mut world, 20); // hitTime 900 ms
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5501)
            .unwrap()
            .charges,
        1,
        "0 -> 1, the level-1 cap"
    );
    let packets = drain(&mut rx);
    assert!(
        has_system_message(
            &packets,
            server_packets::sm_ids::YOUR_FORCE_HAS_REACHED_MAXIMUM_CAPACITY
        ),
        "reached-cap SystemMessage on the capping cast"
    );

    // Off cooldown or not, the skill is castable again; already at the cap,
    // it refuses outright (no charge change, same SM, no further gain).
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(8, false));
    advance_world(&mut world, 20);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5501)
            .unwrap()
            .charges,
        1,
        "still capped at 1"
    );
}

/// Java's ten-minute Force decay (`ResetChargesTask`): charges clear on their
/// own, the clock restarts on every gain, and it stops when the pool empties.
///
/// The port cannot cancel a scheduled task, so "restart" is a generation
/// counter — a stale task fires and does nothing. Each leg below fails
/// differently, so they are asserted separately.
#[test]
fn force_charges_decay_after_ten_minutes_and_the_clock_restarts_on_a_gain() {
    /// 600 000 ms at 100 ms a tick.
    const DECAY: u64 = 6_000;

    let charges = |w: &World| w.objects.get_component::<Player>(&5541).unwrap().charges;
    let gain = |w: &mut World| {
        handle_request_magic_skill_use(w, 1, &magic_skill_use_body(8, false));
        advance_world(w, 20);
    };
    let build = || {
        let (mut world, ..) = test_world();
        world.data = dist::game_data_owned();
        let rx = ingame_player_access(&mut world, 1, 5541, 0);
        world
            .objects
            .get_component_mut::<SkillBook>(&5541)
            .unwrap()
            .0
            .insert(8, 1);
        arm(&mut world, 5541, 2);
        (world, rx)
    };

    // A charge sitting untouched for ten minutes clears itself.
    let (mut world, _rx) = build();
    gain(&mut world);
    assert_eq!(charges(&world), 1, "sanity: Sonic Focus charged");
    advance_world(&mut world, DECAY - 100);
    assert_eq!(charges(&world), 1, "still there just before the deadline");
    advance_world(&mut world, 200);
    assert_eq!(charges(&world), 0, "and gone just after it");

    // A second gain restarts the clock: the *first* task still fires at its
    // original deadline and must do nothing, or the pool empties early.
    let (mut world, _rx) = build();
    gain(&mut world);
    advance_world(&mut world, DECAY / 2);
    world
        .objects
        .get_component_mut::<Player>(&5541)
        .unwrap()
        .charges = 0; // clear the cap so the next cast really charges
    gain(&mut world);
    assert_eq!(charges(&world), 1, "recharged");
    advance_world(&mut world, DECAY / 2 + 100);
    assert_eq!(
        charges(&world),
        1,
        "the first task's deadline passed, but it was superseded"
    );
    advance_world(&mut world, DECAY / 2);
    assert_eq!(charges(&world), 0, "the second one still expires on time");
}

/// G19 `EnergyAttack` effect: Sonic Blaster (6, real dist data, level 1:
/// power 369, criticalChance 15, `chargeConsume` 2 — a *skill-level* tag)
/// spends Force for bonus physical damage — previously silently dropped, so
/// every "Sonic"/"Force" attack skill (Double Sonic Slash, Sonic Blaster,
/// Sonic Buster, Force Burst/Storm/Blaster, …) did nothing on cast.
#[test]
fn energy_attack_spends_charges_for_bonus_damage() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();

    let mut a_rx = ingame_player_access(&mut world, 1, 5511, 0);
    drain(&mut a_rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5511)
        .unwrap()
        .0
        .insert(6, 1);
    // Pre-set Force rather than grinding Sonic Focus casts (level 1 only
    // grants 1, below Sonic Blaster's own chargeConsume of 2) — this effect
    // only cares about the charges already on hand, not how they got there.
    world
        .objects
        .get_component_mut::<Player>(&5511)
        .unwrap()
        .charges = 5;
    // `EquipWeapon` DUAL/SWORD/BLUNT/DUALBLUNT (the `EnergySaved` 2 is already
    // satisfied by the 5 charges above).
    arm(&mut world, 5511, 2);

    let npc_oid = NPC_OID + 2;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 20001, 50, 0, 0, 100_000, 30);
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
    let npc_hp_before = pvit(&world, npc_oid).cur_hp;
    let p_atk = pcs(&world, 5511).p_atk;
    let p_def = combat::combatant(&world, npc_oid).unwrap().p_def;

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(6, false));
    advance_world(&mut world, 40); // hitTime 1900 ms

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5511)
            .unwrap()
            .charges,
        3,
        "5 - chargeConsume(2)"
    );
    // `77 * ((pAtk * levelMod) + power) / pDef * energyChargesBoost(1 + 2×0.1)`,
    // times 2 on a crit. The crit roll isn't pinned here: `advance_world`
    // also runs the spawned NPC's periodic AI think tick, which draws from
    // the same `forced_rolls` queue as the cast's own crit check, so which
    // one actually consumes a pushed value depends on tick timing rather
    // than cast order — tolerate either outcome instead of fighting that.
    let level = world.objects.get_component::<Player>(&5511).unwrap().level;
    let level_mod = formulas::physical::level_mod(level);
    let base = (77.0 * ((p_atk * level_mod) + 369.0) / p_def.max(1.0)) * 1.2;
    let actual_damage = npc_hp_before - pvit(&world, npc_oid).cur_hp;
    assert!(
        (actual_damage - base).abs() < 1e-6 || (actual_damage - base * 2.0).abs() < 1e-6,
        "Sonic Blaster damage with the Force bonus: {actual_damage} (expected {base} or {} on a crit)",
        base * 2.0
    );
}
