//! `VampiricAttack` and `DamageShield` — the two on-hit reactions in Java's
//! `Creature.doAttack` (G20).
//!
//! Both were icon-only markers: Vampiric Rage healed nothing and Reflect Damage
//! bounced nothing. Java `pump`s them as ordinary additive stats, so the port
//! routes them through `stat_modifier_effects` and reads them back where the
//! damage lands.

use super::*;

use crate::model::components::{StatModifiers, Vitals};
use crate::model::skill::SkillEffect;
use crate::model::stats::Stat;

const ATTACKER: i32 = 4001;
const CID: u32 = 1;
const DIST: &str = crate::data::DIST_GAME;

fn hp(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_hp
}

fn set_hp(world: &mut World, oid: i32, value: f64) {
    world
        .objects
        .get_component_mut::<Vitals>(&oid)
        .unwrap()
        .cur_hp = value;
}

fn add_stat(world: &mut World, oid: i32, stat: Stat, amount: f64) {
    let mods = world
        .objects
        .get_component_mut::<StatModifiers>(&oid)
        .expect("a statted actor");
    *mods.add.entry(stat).or_insert(0.0) += amount;
}

/// A caster plus a mob to hit.
fn onhit_world() -> (World, i32) {
    let (mut world, _db, _l) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    let npc_oid = 0x4000_0222;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    // A bare fixture mob carries no `StatModifiers`; a real one gets it the
    // moment a buff lands on it, and `reflect_damage` reads the reflect stat
    // from there.
    world
        .objects
        .add_components(&npc_oid, StatModifiers::default());
    drain(&mut a_rx);
    // `Npc::for_test`'s 1 000 000 pool is clamped by the damage path's recalc;
    // start from the template's real max.
    let max = world
        .objects
        .get_component::<Vitals>(&npc_oid)
        .unwrap()
        .max_hp as f64;
    set_hp(&mut world, npc_oid, max);
    (world, npc_oid)
}

// ---------------------------------------------------------------------------
// VampiricAttack
// ---------------------------------------------------------------------------

/// The two halves Java `pump`s: `ABSORB_DAMAGE_PERCENT += amount/100` and
/// `vampiricSum += amount · chance`. Vampiric Rage 1 is `amount 6, chance 80`.
#[test]
fn vampiric_attack_grants_both_of_its_stats() {
    let sd = crate::data::skill_data::SkillData::load_from(DIST);
    let rage = sd.get(1268, 1).expect("Vampiric Rage");
    let mods = rage.stat_modifier_effects();
    let of = |stat| {
        mods.iter()
            .find(|m| m.stat == stat)
            .map(|m| m.amount)
            .unwrap_or(0.0)
    };
    assert!(
        (of(Stat::AbsorbDamagePercent) - 0.06).abs() < 1e-9,
        "Java stores amount/100"
    );
    assert!(
        (of(Stat::VampiricSum) - 480.0).abs() < 1e-9,
        "amount x chance = 6 x 80"
    );
    // …and the pair reproduces the buff's own chance:
    // min(1, 480 / (0.06 x 100) / 100) = 0.8.
    let chance = (of(Stat::VampiricSum) / (of(Stat::AbsorbDamagePercent) * 100.0) / 100.0).min(1.0);
    assert!((chance - 0.8).abs() < 1e-9, "got {chance}");
}

/// A melee swing under Vampiric Rage heals the attacker for `percent · damage`.
#[test]
fn a_melee_hit_absorbs_hp_for_the_attacker() {
    let (mut world, mob) = onhit_world();
    add_stat(&mut world, ATTACKER, Stat::AbsorbDamagePercent, 0.5);
    add_stat(&mut world, ATTACKER, Stat::VampiricSum, 5_000.0); // chance 1.0
    set_hp(&mut world, ATTACKER, 10.0);

    // The absorb rolls `roll_f64` (< chance); 0 always wins.
    world.forced_rolls.extend([0]);
    crate::game_loop::combat::apply_attack_damage(&mut world, ATTACKER, mob, 40.0, false, None);
    assert_eq!(hp(&world, ATTACKER), 30.0, "50% of 40 damage came back");
}

/// **A bow drains nothing** — Java's "Do not absorb if weapon is ranged" is the
/// first gate, ahead of the chance roll.
#[test]
fn a_ranged_weapon_absorbs_nothing() {
    let (mut world, mob) = onhit_world();
    world.data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    add_stat(&mut world, ATTACKER, Stat::AbsorbDamagePercent, 0.5);
    add_stat(&mut world, ATTACKER, Stat::VampiricSum, 5_000.0);
    set_hp(&mut world, ATTACKER, 10.0);
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects
            .get_component_mut::<crate::model::inventory::Inventory>(&ATTACKER)
            .unwrap();
        let oid = inv.add_item(&data.item_data, 0x5200_0001, 13, 1); // Short Bow
        inv.equip_item(&data.item_data, oid);
    }

    world.forced_rolls.extend([0]);
    crate::game_loop::combat::apply_attack_damage(&mut world, ATTACKER, mob, 40.0, false, None);
    assert_eq!(hp(&world, ATTACKER), 10.0, "a bow feeds no vampire");
}

/// `VampiricAttackWorkWithSkills` is **False** on this dist, so a *skill* hit
/// absorbs nothing however big the buff.
#[test]
fn a_skill_hit_absorbs_nothing_on_this_dists_config() {
    let (mut world, mob) = onhit_world();
    world.cfg.character.vampiric_attack_works_with_skills = false;
    add_stat(&mut world, ATTACKER, Stat::AbsorbDamagePercent, 0.5);
    add_stat(&mut world, ATTACKER, Stat::VampiricSum, 5_000.0);
    set_hp(&mut world, ATTACKER, 10.0);

    world.forced_rolls.extend([0]);
    crate::game_loop::combat::apply_attack_damage(
        &mut world,
        ATTACKER,
        mob,
        40.0,
        false,
        Some(false),
    );
    assert_eq!(hp(&world, ATTACKER), 10.0);

    // Flip the config and the same skill hit does feed.
    world.cfg.character.vampiric_attack_works_with_skills = true;
    world.forced_rolls.extend([0]);
    crate::game_loop::combat::apply_attack_damage(
        &mut world,
        ATTACKER,
        mob,
        40.0,
        false,
        Some(false),
    );
    assert_eq!(hp(&world, ATTACKER), 30.0);
}

/// The absorb never overheals, and **never takes more than the victim has
/// left** — you cannot drain more blood than is in the body.
///
/// (Java clamps by the healer's missing HP as well; here that clamp is
/// belt-and-braces, because the HP write clamps to `max_hp` too. It is kept
/// because it is Java's, not because it is separately observable.)
#[test]
fn the_absorb_never_overheals_and_is_capped_by_the_victim() {
    let (mut world, mob) = onhit_world();
    add_stat(&mut world, ATTACKER, Stat::AbsorbDamagePercent, 1.0);
    add_stat(&mut world, ATTACKER, Stat::VampiricSum, 10_000.0);

    // Nearly full: only 3 HP missing, so only 3 come back from a 100 drain.
    let max = world
        .objects
        .get_component::<Vitals>(&ATTACKER)
        .unwrap()
        .max_hp as f64;
    set_hp(&mut world, ATTACKER, max - 3.0);
    world.forced_rolls.extend([0]);
    crate::game_loop::combat::apply_attack_damage(&mut world, ATTACKER, mob, 100.0, false, None);
    assert_eq!(hp(&world, ATTACKER), max, "no overheal");

    // Victim down to 2 HP: a 100-damage swing can only take those 2.
    set_hp(&mut world, ATTACKER, 10.0);
    set_hp(&mut world, mob, 2.0);
    world.forced_rolls.extend([0]);
    crate::game_loop::combat::apply_attack_damage(&mut world, ATTACKER, mob, 100.0, false, None);
    assert_eq!(hp(&world, ATTACKER), 12.0, "capped at the victim's HP");
}

// ---------------------------------------------------------------------------
// DamageShield
// ---------------------------------------------------------------------------

/// Song of Vengeance (305) and friends grant `REFLECT_DAMAGE_PERCENT`, which
/// the target reads when it takes a hit.
#[test]
fn damage_shield_grants_the_reflect_stat() {
    let sd = crate::data::skill_data::SkillData::load_from(DIST);
    for (id, name, pct) in [
        (305, "Song of Vengeance", 20.0),
        (340, "Riposte Stance", 30.0),
        (86, "Reflect Damage", 10.0),
    ] {
        let s = sd.get(id, 1).unwrap_or_else(|| panic!("{name}"));
        assert!(
            s.effects
                .iter()
                .any(|e| matches!(e, SkillEffect::DamageShield { amount } if *amount == pct)),
            "{name}: {:?}",
            s.effects
        );
        assert!(
            s.stat_modifier_effects()
                .iter()
                .any(|m| m.stat == Stat::ReflectDamagePercent && m.amount == pct),
            "{name} reaches the stat pipeline"
        );
    }
}

/// The target bounces its percentage back at the attacker.
#[test]
fn a_shielded_target_reflects_damage_at_its_attacker() {
    let (mut world, mob) = onhit_world();
    add_stat(&mut world, mob, Stat::ReflectDamagePercent, 25.0);
    let before = hp(&world, ATTACKER);

    crate::game_loop::combat::apply_attack_damage(&mut world, ATTACKER, mob, 40.0, false, None);
    assert_eq!(
        before - hp(&world, ATTACKER),
        10.0,
        "25% of the 40 damage came back"
    );
}

/// **Reflected damage does not reflect again** — the bounce goes through the
/// raw HP-reduction path, so two shielded fighters can't ping-pong forever.
#[test]
fn a_reflected_hit_does_not_bounce_back() {
    let (mut world, mob) = onhit_world();
    add_stat(&mut world, mob, Stat::ReflectDamagePercent, 50.0);
    add_stat(&mut world, ATTACKER, Stat::ReflectDamagePercent, 50.0);
    let attacker_before = hp(&world, ATTACKER);
    let mob_before = hp(&world, mob);

    crate::game_loop::combat::apply_attack_damage(&mut world, ATTACKER, mob, 40.0, false, None);
    assert_eq!(attacker_before - hp(&world, ATTACKER), 20.0, "one bounce");
    assert_eq!(
        mob_before - hp(&world, mob),
        40.0,
        "and the mob took only the original hit, not a re-reflection"
    );
}

/// **A killing blow is not reflected** ("when killing blow is made, the target
/// doesn't reflect"), and neither is a DoT tick.
#[test]
fn a_dead_target_and_a_dot_reflect_nothing() {
    let (mut world, mob) = onhit_world();
    add_stat(&mut world, mob, Stat::ReflectDamagePercent, 50.0);

    // DoT: no bounce.
    let before = hp(&world, ATTACKER);
    crate::game_loop::combat::apply_attack_damage(&mut world, ATTACKER, mob, 20.0, true, None);
    assert_eq!(hp(&world, ATTACKER), before, "a DoT tick never reflects");

    // Killing blow: no bounce either.
    crate::game_loop::combat::apply_attack_damage(
        &mut world,
        ATTACKER,
        mob,
        1_000_000.0,
        false,
        None,
    );
    assert!(
        world.objects.get_component::<Vitals>(&mob).unwrap().dead,
        "the mob died"
    );
    assert_eq!(
        hp(&world, ATTACKER),
        before,
        "a corpse reflects nothing, however big the blow"
    );
}

/// The bounce is capped by the reflector's own defence: `pDef` for a melee or
/// physical-skill hit, `mDef · 1.5` for a magic one.
#[test]
fn the_reflected_amount_is_capped_by_the_reflectors_defence() {
    use crate::model::components::CombatStats;

    let (mut world, mob) = onhit_world();
    add_stat(&mut world, mob, Stat::ReflectDamagePercent, 100.0);
    let (p_def, m_def) = {
        let cs = world.objects.get_component::<CombatStats>(&mob).unwrap();
        (cs.p_def, cs.m_def)
    };
    assert!(
        p_def > 0.0 && m_def > 0.0,
        "the mob has defences to cap with"
    );

    // A huge physical hit bounces back at most `pDef`.
    let before = hp(&world, ATTACKER);
    crate::game_loop::combat::apply_attack_damage(
        &mut world, ATTACKER, mob, 100_000.0, false, None,
    );
    assert_eq!(before - hp(&world, ATTACKER), p_def.trunc());

    // The same through a *magic* skill caps at `mDef * 1.5` instead.
    let (mut world, mob) = onhit_world();
    add_stat(&mut world, mob, Stat::ReflectDamagePercent, 100.0);
    let before = hp(&world, ATTACKER);
    crate::game_loop::combat::apply_attack_damage(
        &mut world,
        ATTACKER,
        mob,
        100_000.0,
        false,
        Some(true),
    );
    assert_eq!(before - hp(&world, ATTACKER), (m_def * 1.5).trunc());
}

/// `PlayerReflectPercentLimit` / `NonPlayerReflectPercentLimit` clamp the
/// percentage before it is applied — 100 each on this dist, and the *mob*
/// branch is the non-player one.
#[test]
fn the_reflect_percentage_is_clamped_by_its_config_limit() {
    let (mut world, mob) = onhit_world();
    add_stat(&mut world, mob, Stat::ReflectDamagePercent, 400.0);
    world.cfg.character.non_player_reflect_percent_limit = 50.0;
    let before = hp(&world, ATTACKER);

    crate::game_loop::combat::apply_attack_damage(&mut world, ATTACKER, mob, 40.0, false, None);
    assert_eq!(
        before - hp(&world, ATTACKER),
        20.0,
        "400% was clamped to the 50% limit"
    );
}

/// The dist's own config values, so a change to either ini shows up here.
#[test]
fn the_vampiric_and_reflect_config_is_read_from_the_dist() {
    let cfg = crate::config::character::CharacterConfig::load_from(DIST);
    assert!(
        !cfg.vampiric_attack_works_with_skills,
        "VampiricAttackWorkWithSkills = False"
    );
    assert!(
        cfg.vampiric_attack_affects_pvp,
        "VampiricAttackAffectsPvP = True — and it lives in PVP.ini"
    );
    assert_eq!(cfg.player_reflect_percent_limit, 100.0);
    assert_eq!(cfg.non_player_reflect_percent_limit, 100.0);
}

/// The absorb is a **roll**, not a certainty: `VampiricChanceFinalizer` turns
/// the buff pair into a chance and `Rnd.nextDouble() < chance` decides. Vampiric
/// Rage's own 80 % is what a single buff produces.
#[test]
fn the_absorb_rolls_its_chance() {
    let (mut world, mob) = onhit_world();
    // absorb 0.5, sum 2500 → chance = 2500 / 50 / 100 = 0.5.
    add_stat(&mut world, ATTACKER, Stat::AbsorbDamagePercent, 0.5);
    add_stat(&mut world, ATTACKER, Stat::VampiricSum, 2_500.0);
    set_hp(&mut world, ATTACKER, 10.0);

    // `roll_f64` reads a forced value as `v / 1_000_000`: 0.6 loses the 0.5 roll.
    world.forced_rolls.extend([600_000]);
    crate::game_loop::combat::apply_attack_damage(&mut world, ATTACKER, mob, 40.0, false, None);
    assert_eq!(hp(&world, ATTACKER), 10.0, "the roll was lost");

    // 0.4 wins it.
    world.forced_rolls.extend([400_000]);
    crate::game_loop::combat::apply_attack_damage(&mut world, ATTACKER, mob, 40.0, false, None);
    assert_eq!(hp(&world, ATTACKER), 30.0, "and won on the next swing");
}
