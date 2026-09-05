//! The PvP bonus and PvE penalty stat pairs, and where each is read.

use super::*;

/// **`calculatePvpPveBonus`** — a term in every damage formula that this port
/// hard-coded to 1.0, behind comments in three different files saying the
/// pvp/pve mods were 1.0. That was true only while nothing granted the stats;
/// the dist has ~1300 effects that do.
///
/// The shape to get right is that it is a **difference of multipliers**, not a
/// product: `1 + (attackMul − defenceMul)`, so a +50 % attacker facing a +50 %
/// defender comes out at exactly 1.0. A port that multiplied them would give
/// 2.25.
#[test]
fn pvp_damage_bonus_is_a_difference_of_multipliers_not_a_product() {
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = CASTER + 1;
    let _v = ingame_player(&mut world, CID + 1, victim, 40, 0, 0);

    let bonus = |world: &World| effects::pvp_pve_bonus_for_test(world, CASTER, victim, None);
    assert_eq!(bonus(&world), 1.0, "no stats granted: no change");

    // Attacker +50 % PvP auto-attack damage.
    if let Some(m) = world
        .objects
        .get_component_mut::<model::components::stats::StatModifiers>(&CASTER)
    {
        *m.mul.entry(Stat::PvpPhysicalAttackDamage).or_insert(1.0) *= 1.5;
    }
    assert_eq!(bonus(&world), 1.5, "attacker alone: 1 + (1.5 - 1)");

    // Victim +50 % PvP auto-attack *defence* — the two cancel exactly.
    if let Some(m) = world
        .objects
        .get_component_mut::<model::components::stats::StatModifiers>(&victim)
    {
        *m.mul.entry(Stat::PvpPhysicalAttackDefence).or_insert(1.0) *= 1.5;
    }
    assert_eq!(
        bonus(&world),
        1.0,
        "+50 % against +50 % cancels — a *product* would read 2.25 here"
    );
}

/// The branch is picked by *how* the damage is delivered, not just by who is
/// fighting: an auto-attack (Java's `skill == null`) reads the
/// `PHYSICAL_ATTACK` pair, a physical skill the `PHYSICAL_SKILL` pair and a
/// magic skill the `MAGICAL_SKILL` pair. Granting one must not move the others.
#[test]
fn the_pvp_bonus_reads_a_different_stat_pair_per_delivery() {
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = CASTER + 1;
    let _v = ingame_player(&mut world, CID + 1, victim, 40, 0, 0);
    let mut physical = cc_skill(9410, SkillEffect::Root, "NONE");
    physical.magic_type = 0;
    let mut magical = cc_skill(9411, SkillEffect::Root, "NONE");
    magical.magic_type = 1;

    // Only the *magical skill* stat is granted.
    if let Some(m) = world
        .objects
        .get_component_mut::<model::components::stats::StatModifiers>(&CASTER)
    {
        *m.mul.entry(Stat::PvpMagicalSkillDamage).or_insert(1.0) *= 1.5;
    }

    let bonus = |world: &World, skill: Option<&Skill>| {
        effects::pvp_pve_bonus_for_test(world, CASTER, victim, skill)
    };
    assert_eq!(
        bonus(&world, Some(&magical)),
        1.5,
        "the magic skill reads it"
    );
    assert_eq!(
        bonus(&world, Some(&physical)),
        1.0,
        "a physical skill does not"
    );
    assert_eq!(bonus(&world, None), 1.0, "and neither does an auto-attack");
}

/// The **PvE** branch carries a level-difference penalty the port never had:
/// `SkillDmgPenaltyForLvLDifferences`, which this dist tunes down to ×0.25.
/// It only bites on a non-raid NPC at or above `MinNPCLevelForDmgPenalty` (78)
/// that is 2+ levels above the attacker — and a raid boss is exempt, which is
/// the clause a "just multiply by the table" port would drop.
#[test]
fn the_pve_penalty_bites_on_high_level_mobs_and_spares_raids() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    if let Some(p) = world.objects.get_component_mut::<Player>(&CASTER) {
        p.level = 78;
    }
    let mob = NPC_OID;
    let boss = NPC_OID + 1;
    // Synthetic ids so the *level* is ours to set — `add_test_npc` honours its
    // level argument only for templates it has to invent, and every real dist
    // id already carries one. Same level on both, so the raid exemption is the
    // only difference between them.
    add_test_npc(&mut world, mob, 90001, "Monster", 85, 100, 0, 0);
    add_test_npc(&mut world, boss, 90002, "RaidBoss", 85, 150, 0, 0);

    let bonus =
        |world: &World, target: i32| effects::pvp_pve_bonus_for_test(world, CASTER, target, None);
    assert!(
        bonus(&world, mob) < 1.0,
        "a level-85 mob against a level-78 player is penalised, got {}",
        bonus(&world, mob)
    );
    assert_eq!(
        bonus(&world, boss),
        1.0,
        "a raid boss is exempt from the penalty entirely"
    );
}

/// End-to-end: the bonus has to *reach* the damage. A helper that computes the
/// right number and is never multiplied in is the exact failure mode this epic
/// keeps finding — the three formula comments claiming "pvp-pve mods 1.0" were
/// each a call site that had to be edited, not just a stat to register.
#[test]
fn the_pvp_bonus_actually_reaches_a_nukes_damage() {
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world.cfg.character.magic_failures = false;
    let victim = CASTER + 1;
    let _v = ingame_player(&mut world, CID + 1, victim, 40, 0, 0);
    let mut nuke = cc_skill(9412, SkillEffect::MagicalAttack { power: 100.0 }, "NONE");
    nuke.magic_type = 1;
    world.data.skill_data.insert_for_test(nuke);

    let cast_once = |world: &mut World| -> f64 {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&victim) {
            v.max_hp = 1_000_000;
            v.cur_hp = 1_000_000.0;
            v.dead = false;
        }
        world.clear_forced_rolls();
        world.force_rolls([50; 12]);
        land(world, 9412, victim);
        1_000_000.0
            - world
                .objects
                .get_component::<Vitals>(&victim)
                .map(|v| v.cur_hp)
                .unwrap_or(0.0)
    };

    let plain = cast_once(&mut world);
    assert!(plain > 0.0, "the nuke lands for something: {plain}");

    // The *victim* takes a magical-skill PvP defence buff.
    if let Some(m) = world
        .objects
        .get_component_mut::<model::components::stats::StatModifiers>(&victim)
    {
        *m.mul.entry(Stat::PvpMagicalSkillDefence).or_insert(1.0) *= 1.5;
    }
    let defended = cast_once(&mut world);

    assert!(
        defended < plain,
        "a +50 % PvP magical-skill defence must reduce the nuke: {plain} -> {defended}"
    );
    // 1 + (1.0 - 1.5) = 0.5 exactly.
    assert!(
        (defended / plain - 0.5).abs() < 0.02,
        "and by exactly the 1 + (atk - def) factor: {plain} -> {defended}"
    );
}
