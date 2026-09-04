//! Direct damage: nukes, blows, lethal, vampiric and the heal/skill-power
//! terms that scale them, plus the force charges an energy attack spends.

use super::*;

/// A lethal nuke kills (G9): HP hits 0, the victim is dead, and `Die` with
/// the to-village flag reaches both sides.
#[test]
fn nuke_kills_at_zero_hp() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    world
        .objects
        .get_component_mut::<PlayerVitals>(&3002)
        .unwrap()
        .cur_cp = 0.0;
    world
        .objects
        .get_component_mut::<Vitals>(&3002)
        .unwrap()
        .cur_hp = 5.0;
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    advance_ticks(&mut world, 45);
    let b = pvit(&world, 3002);
    assert_eq!(b.cur_hp, 0.0);
    assert!(b.dead);
    let a_packets = drain(&mut a_rx);
    let b_packets = drain(&mut b_rx);
    for packets in [&a_packets, &b_packets] {
        let die = packets
            .iter()
            .find(|p| is_for(p, server_packets::opcodes::DIE, 3002))
            .expect("Die packet for B");
        assert_eq!(
            i32::from_le_bytes(die[5..9].try_into().unwrap()),
            1,
            "to-village flag"
        );
    }
}

/// An offensive skill lands on a siege gate: `resolve_cast_target` accepts the
/// door (siege-attackable), the LOS check is skipped for a door target (Java
/// `canSeeTarget` short-circuit), the pipeline resolves the door's position,
/// and the magic damage routes to the gate's HP instead of the creature path.
#[test]
fn cast_nuke_damages_siege_door() {
    use crate::data::door_data::DoorOpenMethod;
    use model::door::Door;
    use model::siege::Siege;
    let (mut world, ..) = cast_test_world();
    insert_siege_zone(&mut world, 3, 0, 1000, -1000, 1000); // covers the gate at (100, 0)
    let mut siege = Siege::new(3);
    siege.in_progress = true;
    world.sieges.insert(3, siege);
    let door =
        model::door::spawn_door_for_test(&mut world, test_door(24190001, DoorOpenMethod::None));
    world
        .objects
        .get_component_mut::<Door>(&door)
        .unwrap()
        .current_hp = 100_000;
    let mut rx = ingame_caster(&mut world, 1, 3001, 150, 0); // within Wind Strike's 600 cast range

    // Ctrl-cast Wind Strike (1177, EnemyOnly) at the gate.
    handle_action(&mut world, 1, &action_body(door, 0));
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "the door is a valid enemy target"
    );
    advance_ticks(&mut world, 45); // launch (35) + finish (5) with margin

    assert!(
        world
            .objects
            .get_component::<Door>(&door)
            .unwrap()
            .current_hp
            < 100_000,
        "the nuke damaged the gate"
    );
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| is_for(p, server_packets::opcodes::STATUS_UPDATE, door)),
        "the gate's HP bar is refreshed for onlookers",
    );
}

/// A mob that dies **mid-chase** must broadcast `StopMove` (Java `doDie` →
/// `stopMove(null)`) so the client freezes the corpse at the death spot instead
/// of sliding it toward its last move destination — the lingering selection/
/// target decal "where the mob died". The `StopMove` carries the mob's current
/// position and comes before the `Die` broadcast.
#[test]
fn moving_mob_death_broadcasts_stop_move() {
    use crate::game_loop::npc;
    use model::components::space::Movement;
    use model::movement::MoveData;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    add_test_npc(&mut world, npc_oid, 40001, "Monster", 5, 40, 0, 0);
    // Give it an in-flight chase move (client is interpolating it toward 400,0).
    world.objects.add_components(
        &npc_oid,
        Movement(MoveData {
            start_x: 40,
            start_y: 0,
            start_z: 0,
            dest_x: 400,
            dest_y: 0,
            dest_z: 0,
            start_tick: world.tick,
            total_ticks: 100,
            geo_path: None,
        }),
    );
    drain(&mut a_rx);

    npc::npc_do_die(&mut world, npc_oid, 3001);

    let packets = drain(&mut a_rx);
    let stop_idx = packets
        .iter()
        .position(|p| is_for(p, server_packets::opcodes::STOP_MOVE, npc_oid))
        .expect("StopMove broadcast for the dying mob");
    // Frozen at the death spot (40,0), not the move destination (400,0).
    let stop = &packets[stop_idx];
    assert_eq!(
        i32::from_le_bytes(stop[5..9].try_into().unwrap()),
        40,
        "StopMove at death x"
    );
    assert_eq!(
        i32::from_le_bytes(stop[9..13].try_into().unwrap()),
        0,
        "StopMove at death y"
    );
    // Ordering: StopMove precedes Die (Java doDie order).
    let die_idx = packets
        .iter()
        .position(|p| is_for(p, server_packets::opcodes::DIE, npc_oid))
        .expect("Die broadcast");
    assert!(stop_idx < die_idx, "StopMove is sent before Die");
}

/// Nuking a monster with a skill wakes its AI exactly like a melee hit and
/// kills through the same death path (the "kill a monster with a skill"
/// half of the G9 gate).
#[test]
fn nuke_kills_monster_and_rewards() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 11;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 100, 0, 0, 100, 30);
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

    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .exp = 4000; // level 5 on the test table
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);

    // Monsters are valid Enemy targets without ctrl.
    let exp_before = world
        .objects
        .get_component::<Player>(&3001)
        .expect("player")
        .exp;
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "cast accepted without force-use"
    );
    // Roll order at cast finish: magic crit (d1000, 999_999 → no crit), the
    // `MagicFailures` success roll (d100, 0 → lands at full damage against a
    // level-5 mob), then the drop roll at death (999_999 → fails, so no loot
    // noise in this test).
    world.force_rolls([999_999, 0, 999_999]);
    advance_world(&mut world, 45);

    assert!(nvit(&world, npc_oid).dead, "the nuke killed it");
    assert!(
        world
            .objects
            .get_component::<Player>(&3001)
            .expect("player")
            .exp
            > exp_before,
        "XP rewarded through the same death path"
    );
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::DIE));
}

/// A nuke against a far-higher-level monster is resisted down to 1 damage —
/// `Formulas.calcMagicDam`'s `ALT_GAME_MAGICFAILURES` branch. `calcMagicSuccess`
/// scales the failure term by `1.3^(targetLevel - skillMagicLevel)`, so at a
/// ~55-level gap the rate is far below 0 and *both* rolls fail whatever they
/// land on, floating the damage to 1. Until this was wired up, a level-5
/// character's Wind Strike killed a level-60 mob at full damage.
#[test]
fn nuke_on_a_far_higher_level_monster_is_resisted_to_one_damage() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    // A nuke now carries Java's ±10 % `randomMod`; this test compares two
    // casts, so the spread is switched off rather than averaged out.
    zero_random_damage(&mut world, 3001);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .exp = 4000; // level 5

    let npc_oid = NPC_OID + 31;
    add_test_npc(&mut world, npc_oid, 40099, "Monster", 60, 100, 0, 0);
    let hp_before = nvit(&world, npc_oid).cur_hp;

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    // Only the crit roll needs pinning — a magic crit would double the floored
    // damage to 2. The two success rolls fail on any value at this gap.
    world.force_rolls([999]);
    advance_world(&mut world, 45);

    // The next regen tick is at 60, past the cast, so nothing heals the 1 back.
    let dealt = hp_before - nvit(&world, npc_oid).cur_hp;
    assert!(
        (dealt - 1.0).abs() < 1e-9,
        "a resisted nuke deals exactly 1 damage, dealt {dealt}"
    );
    assert!(
        !nvit(&world, npc_oid).dead,
        "1 damage can't kill a 100 HP mob"
    );

    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::C1_HAS_RESISTED_YOUR_S2),
        "the caster is told the target resisted"
    );
}

/// The same nuke against a same-level monster is unaffected: the failure roll
/// is a 97 % proposition at a 4-level gap, so the damage is the full MDAM
/// figure. Guards the penalty against over-firing on normal-level content.
#[test]
fn nuke_on_a_same_level_monster_deals_full_damage() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .exp = 4000; // level 5

    let npc_oid = NPC_OID + 32;
    add_test_npc(&mut world, npc_oid, 40098, "Monster", 5, 100, 0, 0);
    let m_atk = pcs(&world, 3001).m_atk;
    let m_def = pcs(&world, npc_oid).m_def; // `pcs` reads any object's CombatStats
    let unresisted = formulas::magic::calc_magic_dam(
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
        unresisted > 100.0,
        "sanity: an unresisted nuke overkills a 100 HP mob ({unresisted})"
    );

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    // Crit misses; the success roll of 0 lands against the 97 % rate.
    world.force_rolls([999, 0]);
    advance_world(&mut world, 45);

    // Full damage overkills, so the mob dies — the exact figure is pinned by
    // `magic_dam_matches_java_formula`; what matters here is that no level
    // penalty bit at a 4-level gap (contrast the level-60 case above, which
    // survives on 1 damage).
    assert!(
        nvit(&world, npc_oid).dead,
        "an unpenalized nuke kills a same-level mob"
    );
}

/// Dagger blows deal damage (Mortal Blow, a FatalBlow), and Backstab only
/// lands from a flank: behind the mob it hits, from the front it silently fails.
#[test]
fn dagger_blows_deal_damage_and_backstab_requires_flank() {
    use crate::game_loop;
    use model::components::space::Position;
    use model::components::stats::{CombatStats, Vitals};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0); // caster at (0,0)
    let npc_oid = NPC_OID + 16;
    // NPC at (40,0). Heading 0 (faces +x, east) → caster to its west is BEHIND.
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
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
    // Deterministic land roll: crit rate > 0 so the blow can land, no random spread.
    {
        let c = world
            .objects
            .get_component_mut::<CombatStats>(&3001)
            .unwrap();
        c.crit_hit = 1000.0;
        c.random_dmg = 0;
    }
    drain(&mut a_rx);

    // Mortal Blow (FatalBlow) — lands from behind, deals damage.
    let mortal = world
        .data
        .skill_data
        .get(16, 1)
        .expect("Mortal Blow")
        .clone();
    let hp0 = nvit(&world, npc_oid).cur_hp;
    world.force_rolls([999_999, 0, 999_999]); // top magic roll; success lands; crit-double fails
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &mortal);
    assert!(
        nvit(&world, npc_oid).cur_hp < hp0,
        "FatalBlow dealt damage (was a no-op before)"
    );
    world
        .objects
        .get_component_mut::<Vitals>(&npc_oid)
        .unwrap()
        .cur_hp = hp0;

    // Backstab from behind — lands.
    let backstab = world.data.skill_data.get(30, 1).expect("Backstab").clone();
    world.force_rolls([999_999, 0, 999_999]);
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &backstab);
    assert!(
        nvit(&world, npc_oid).cur_hp < hp0,
        "Backstab from the flank landed"
    );
    world
        .objects
        .get_component_mut::<Vitals>(&npc_oid)
        .unwrap()
        .cur_hp = hp0;

    // Turn the mob to face the caster (heading 0x8000 = west) → caster is now in
    // front → Backstab silently fails, dealing no damage.
    world
        .objects
        .get_component_mut::<Position>(&npc_oid)
        .unwrap()
        .heading = 0x8000;
    world.force_rolls([999_999, 0, 999_999]);
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &backstab);
    assert_eq!(
        nvit(&world, npc_oid).cur_hp,
        hp0,
        "front Backstab dealt no damage"
    );
}

/// Vampiric Touch (1147, HpDrain) deals magic damage to the mob and heals the
/// caster by 40% of the HP drained — the regression guard for the whole
/// `HpDrain` family, which used to cast but deal (and drain) nothing.
#[test]
fn vampiric_touch_deals_damage_and_heals_caster() {
    use crate::game_loop;
    use model::components::stats::Vitals;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 15;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
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
    // Wound the caster (with ample headroom) so the lifesteal isn't
    // overheal-clamped away.
    {
        let v = world.objects.get_component_mut::<Vitals>(&3001).unwrap();
        v.max_hp = 100_000;
        v.cur_hp = 1.0;
    }
    let npc_hp_before = nvit(&world, npc_oid).cur_hp;
    let caster_hp_before = world.objects.get_component::<Vitals>(&3001).unwrap().cur_hp;
    drain(&mut a_rx);

    let skill = world
        .data
        .skill_data
        .get(1147, 1)
        .expect("Vampiric Touch")
        .clone();
    // magic-crit roll fails, then the `MagicFailures` success roll lands (0).
    world.force_rolls([999_999, 0]);
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

    let dmg = npc_hp_before - nvit(&world, npc_oid).cur_hp;
    assert!(
        dmg > 0.0,
        "Vampiric Touch dealt damage (was a silent no-op before)"
    );
    let healed = world.objects.get_component::<Vitals>(&3001).unwrap().cur_hp - caster_hp_before;
    assert!(
        (healed - 0.40 * dmg).abs() < 1.0,
        "caster healed {healed}, expected 40% of {dmg}"
    );
}

/// G19 `HealPercent` effect: "Revival" (181, real dist data — a self-target,
/// 100%-power heal) restores HP on cast. Before this slice every
/// `HealPercent` skill — including the priest staples Miracle, Benediction,
/// Restore Life, Touch of Life — parsed to an empty effect list, so the cast
/// landed but healed nothing. Self-cast rather than on another player only
/// because that's what this particular skill is — see
/// `enemy_not_targets_a_friendly_player_but_refuses_a_hostile_one` for
/// Restore Life healing someone else (its `targetType ENEMY_NOT`).
#[test]
fn heal_percent_restores_a_share_of_max_hp() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();

    let mut rx = ingame_player_access(&mut world, 1, 5301, 0);
    drain(&mut rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5301)
        .unwrap()
        .0
        .insert(181, 1);

    let max_hp = pvit(&world, 5301).max_hp as f64;
    // Revival's own `<condition name="RemainHpPer">` is `LESS 10` on the
    // caster — it is the emergency self-heal, and Java refuses it above 10 %.
    // The fixture used to sit at 20 % and cast anyway, because no condition
    // was enforced (G34 S1).
    let low = max_hp * 0.05;
    world
        .objects
        .get_component_mut::<Vitals>(&5301)
        .unwrap()
        .cur_hp = low;

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(181, false));
    advance_world(&mut world, 40); // hitTime 1500 ms, well inside 40 × 100 ms ticks

    assert!(
        (pvit(&world, 5301).cur_hp - max_hp).abs() < 1e-6,
        "Revival (power 100) fully restores HP: {} (max {})",
        pvit(&world, 5301).cur_hp,
        max_hp
    );
    let packets = drain(&mut rx);
    assert!(
        has_system_message(&packets, server_packets::sm_ids::S1_HP_HAS_BEEN_RESTORED),
        "self-cast heal SystemMessage sent"
    );
}

/// G19 `Lethal` effect: Lethal Blow (344, real dist data — pairs `FatalBlow`
/// with `fullLethal` 0 / `halfLethal` 15) sets the target's CP to 1 on a
/// landed half-kill — previously dropped, so Backstab/Lethal Blow/Deadly
/// Blow/Critical Blow/Lethal Shot dealt their (already-ported) damage but
/// never rolled the bonus kill chance. Force-targets a second player (`ctrl`)
/// so the assertion (CP → 1) is decoupled from FatalBlow's own HP damage,
/// which lands first in the same effect list. Every `world.roll` is flooded
/// with `0` — not just the half-kill roll, since `FatalBlow`'s own land/crit
/// rolls (and the spawned NPC's periodic AI think tick, present in other
/// tests in this file) also draw from the same queue ahead of it.
#[test]
fn lethal_half_kill_sets_player_cp_to_1() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();

    let mut a_rx = ingame_player_access(&mut world, 1, 5601, 0);
    let mut b_rx = ingame_player_access(&mut world, 2, 5602, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    world
        .objects
        .get_component_mut::<Position>(&5602)
        .unwrap()
        .x = 30;
    world
        .objects
        .get_component_mut::<SkillBook>(&5601)
        .unwrap()
        .0
        .insert(344, 1);
    // `EquipWeapon` DAGGER/DUALDAGGER — Bone Dagger satisfies it.
    arm(&mut world, 5601, 11);
    world
        .objects
        .get_component_mut::<Vitals>(&5601)
        .unwrap()
        .cur_mp = 200.0;
    // A level-1 default has a tiny naked CP pool (possibly already ≤ 1) —
    // give the target real headroom so "drained to 1" is an observable drop.
    {
        let pv = world
            .objects
            .get_component_mut::<PlayerVitals>(&5602)
            .unwrap();
        pv.max_cp = 50;
        pv.cur_cp = 50.0;
    }

    handle_action(&mut world, 1, &action_body(5602, 0));
    drain(&mut a_rx);
    world.force_rolls(std::iter::repeat_n(0, 30));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(344, true)); // ctrl: force a clean player target
    advance_world(&mut world, 30); // hitTime 1080 ms

    assert_eq!(
        world
            .objects
            .get_component::<PlayerVitals>(&5602)
            .unwrap()
            .cur_cp,
        1.0,
        "half-kill drains CP to 1"
    );
    let b_packets = drain(&mut b_rx);
    assert!(
        has_system_message(&b_packets, server_packets::sm_ids::HALF_KILL)
            && has_system_message(&b_packets, server_packets::sm_ids::YOUR_CP_WAS_DRAINED_BECAUSE_YOU_WERE_HIT_WITH_A_HALF_KILL_SKILL),
        "target sees both Half-Kill SystemMessages"
    );
}

/// `Lethal.instant`'s closing `calcCounterAttack` — "No matter if lethal
/// succeeded or not, its reflected", in Java's own words.
///
/// Java has exactly two `calcCounterAttack` call sites: `reduceCurrentHp`
/// (once per damaging skill) and `Lethal.instant`. Every lethal carrier on
/// this dist pairs Lethal with a damage effect, so both fire and the victim
/// counters **twice** for one cast. That double is the observable, and it is
/// Java's behaviour, not a duplicate to suppress — before this, only one
/// counter fired.
#[test]
fn a_lethal_cast_counters_twice_because_java_rolls_it_from_both_sites() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();

    let mut a_rx = ingame_player_access(&mut world, 1, 5621, 0);
    let mut b_rx = ingame_player_access(&mut world, 2, 5622, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    world
        .objects
        .get_component_mut::<Position>(&5622)
        .unwrap()
        .x = 30;
    world
        .objects
        .get_component_mut::<SkillBook>(&5621)
        .unwrap()
        .0
        .insert(344, 1);
    arm(&mut world, 5621, 11);
    world
        .objects
        .get_component_mut::<Vitals>(&5621)
        .unwrap()
        .cur_mp = 200.0;
    // Survive the blow: the counter bails on a dead target, so a one-shot
    // victim would silently lose the second roll and look like a missing
    // call site.
    {
        let v = world.objects.get_component_mut::<Vitals>(&5622).unwrap();
        v.max_hp = 100_000;
        v.cur_hp = 100_000.0;
    }
    // The counter needs a real p_atk behind it or the damage rounds to 0 and
    // the whole thing bails before sending anything.
    if let Some(cs) = world.objects.get_component_mut::<CombatStats>(&5622) {
        cs.p_atk = 500.0;
    }
    // `VENGEANCE_SKILL_PHYSICAL_DAMAGE` at 100 — the counter always rolls.
    {
        let mut mods = world
            .objects
            .get_component::<model::components::stats::StatModifiers>(&5622)
            .cloned()
            .unwrap_or_default();
        mods.add.insert(Stat::VengeanceSkillPhysicalDamage, 100.0);
        world.objects.add_components(&5622, mods);
    }

    handle_action(&mut world, 1, &action_body(5622, 0));
    drain(&mut a_rx);
    drain(&mut b_rx);
    world.force_rolls(std::iter::repeat_n(0, 40));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(344, true));
    advance_world(&mut world, 30);

    let counters = drain(&mut b_rx)
        .iter()
        .filter(|p| {
            has_system_message(
                std::slice::from_ref(*p),
                server_packets::sm_ids::YOU_COUNTERED_C1_S_ATTACK,
            )
        })
        .count();
    assert_eq!(
        counters, 2,
        "one counter from the damage effect and one from Lethal — Java's two \
         call sites; a single counter means the Lethal one is missing"
    );
}

/// The other half of `Lethal`: raid bosses are immune (`isLethalable()`),
/// mirroring the same raid-immunity check `Mute`'s cast-interrupt already
/// has. A real dist raid boss (3404 "Tracker Captain Sharuk", level 23 — well
/// under Lethal Blow's magicLevel 76, so the separate level gate doesn't
/// interfere) takes `FatalBlow`'s damage but keeps its Force/CP-equivalent
/// untouched: HP drops from the blow, but never gets forced to 1 or halved
/// again on top of that by a landed Lethal.
#[test]
fn lethal_spares_a_raid_boss() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();

    let mut a_rx = ingame_player_access(&mut world, 1, 5611, 0);
    drain(&mut a_rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5611)
        .unwrap()
        .0
        .insert(344, 1);
    // `EquipWeapon` DAGGER/DUALDAGGER — Bone Dagger satisfies it.
    arm(&mut world, 5611, 11);
    world
        .objects
        .get_component_mut::<Vitals>(&5611)
        .unwrap()
        .cur_mp = 200.0;

    let npc_oid = NPC_OID + 10;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 3404, 30, 0, 0, 1_000_000, 100);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(3404).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    assert!(
        world.data.npc_data.get(3404).unwrap().is_raid(),
        "sanity: 3404 really is a RaidBoss template"
    );
    let hp_before = pvit(&world, npc_oid).cur_hp;

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);
    world.force_rolls(std::iter::repeat_n(0, 30));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(344, false));
    advance_world(&mut world, 30); // hitTime 1080 ms

    let hp_after = pvit(&world, npc_oid).cur_hp;
    assert!(
        hp_after < hp_before,
        "sanity: FatalBlow's own damage still landed"
    );
    assert!(
        hp_after > hp_before * 0.4,
        "a landed Lethal half-kill would have halved *whatever HP FatalBlow left* on top \
         of the blow's own damage — well below 50% of the pre-cast HP; immunity keeps it \
         from ever compounding like that: {hp_before} -> {hp_after}"
    );
}

/// G34 S4 sub-slice 2 — `PHYSICAL_SKILL_POWER` / `MAGICAL_SKILL_POWER`, the
/// last multiplier a skill's damage passes through. Focus Skill Mastery (334)
/// is the learnable physical source; the magical one is item-only here.
///
/// Java applies the physical stat from each `PhysicalAttack`-family *effect
/// handler* but the magical one from **inside `calcMagicDam`** — so every
/// caller of that function gets it, HpDrain included, even though HpDrain's own
/// handler never mentions the stat. Both damage paths are asserted for that
/// reason.
///
/// **One world, four measurements.** An earlier version built a fresh dist-
/// loaded world per case; four `GameData::load_from` calls made it the slowest
/// test in the suite and it started timing out under parallel load. The mob's
/// HP is reset between measurements instead, and its pool is far deeper than
/// any hit under test — otherwise the clamp, not the multiplier, is what the
/// assertion measures.
#[test]
fn the_skill_power_stats_scale_finished_skill_damage() {
    use crate::game_loop;
    use model::components::stats::StatModifiers;
    use model::stats::Stat;

    const POWER_STRIKE: i32 = 3;
    const WIND_STRIKE: i32 = 1177;
    const CASTER: i32 = 6401;
    let npc = game_loop::npc::FIRST_NPC_OBJECT_ID + 7801;

    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();
    let _rx = ingame_player_access(&mut world, 1, CASTER, 0);
    add_test_npc(&mut world, npc, 20001, "Monster", 20, 0, 0, 0);

    let measure = |world: &mut World, skill_id: i32, stat: Option<(Stat, f64)>| -> f64 {
        let mut mods = world
            .objects
            .get_component::<StatModifiers>(&CASTER)
            .cloned()
            .expect("modifiers");
        mods.mul.remove(&Stat::PhysicalSkillPower);
        mods.mul.remove(&Stat::MagicalSkillPower);
        if let Some((s, v)) = stat {
            mods.mul.insert(s, v);
        }
        world.objects.add_components(&CASTER, mods);
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&npc) {
            v.max_hp = 1_000_000;
            v.cur_hp = 1_000_000.0;
        }
        let skill = skill_by_id(world, skill_id, 1).expect("skill");
        world.clear_forced_rolls();
        world.force_rolls([50; 12]);
        effects::apply_skill_effects(world, CASTER, npc, &skill);
        1_000_000.0 - pvit_npc_hp(world, npc)
    };

    let plain = measure(&mut world, POWER_STRIKE, None);
    let boosted = measure(
        &mut world,
        POWER_STRIKE,
        Some((Stat::PhysicalSkillPower, 2.0)),
    );
    assert!(plain > 0.0, "the skill deals damage at all: {plain}");
    assert!(
        (boosted - plain * 2.0).abs() < 1.0,
        "PHYSICAL_SKILL_POWER ×2 doubles it ({plain} → {boosted})"
    );

    let plain_m = measure(&mut world, WIND_STRIKE, None);
    let boosted_m = measure(
        &mut world,
        WIND_STRIKE,
        Some((Stat::MagicalSkillPower, 2.0)),
    );
    assert!(plain_m > 0.0, "the nuke deals damage at all: {plain_m}");
    assert!(
        (boosted_m - plain_m * 2.0).abs() < 1.0,
        "MAGICAL_SKILL_POWER ×2 doubles it ({plain_m} → {boosted_m})"
    );
}

fn pvit_npc_hp(world: &World, oid: i32) -> f64 {
    world
        .objects
        .get_component::<Vitals>(&oid)
        .map(|v| v.cur_hp)
        .unwrap_or(0.0)
}

/// **A nuke carries the caster's random-damage spread**, exactly as a swing
/// does: `calcMagicDam`'s tail multiplies by
/// `attacker.getRandomDamageMultiplier()`. Every class template declares
/// `baseRndDam = 10`, so the same cast on the same target must land on more
/// than one number — before this the port's magic damage was identical every
/// time.
#[test]
fn magic_damage_varies_with_the_casters_random_spread() {
    use crate::model::formulas;
    use crate::model::formulas::magic::MagicFailure;

    // The formula's own term first: ±10 % around the deterministic value.
    let at = |random_mul: f64| {
        formulas::magic::calc_magic_dam(
            100.0,
            60.0,
            12.0,
            false,
            2.0,
            1.0,
            MagicFailure::None,
            random_mul,
        )
    };
    let base = at(1.0);
    assert!((at(1.1) - base * 1.1).abs() < 1e-9, "the high end");
    assert!((at(0.9) - base * 0.9).abs() < 1e-9, "the low end");

    // …and it reaches the cast path: 40 nukes from a caster with the template
    // spread must not all be the same number.
    let (mut world, _db, _l) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..40 {
        seen.insert(
            crate::game_loop::skills::effects::random_damage_multiplier_of(&mut world, 3001)
                .to_bits(),
        );
    }
    assert!(
        seen.len() > 1,
        "the caster's spread produced one value in 40 rolls"
    );
}
