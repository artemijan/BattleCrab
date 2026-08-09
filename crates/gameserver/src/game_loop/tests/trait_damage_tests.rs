//! The **damage** side of the trait tables (G20) — `calcWeaponTraitBonus`,
//! `calcWeaknessBonus` and `calcAttackTraitBonus`, plus the attacker-side
//! `AttackTrait` accumulator they read.
//!
//! The G16 slice ported `DefenceTrait` and wired it into the *landing roll*.
//! These are the other consumers: Deflect Arrow really softening arrows,
//! Provoke really making pole hits hurt more, and the Hunter's "Detect …
//! Weakness" line finally doing something.

use super::*;

use crate::game_loop::skills::effects::{
    calc_attack_trait_bonus, calc_general_trait_bonus, calc_weakness_bonus,
    calc_weapon_trait_bonus, merge_attack_traits, merge_defence_traits, remove_attack_traits,
};
use crate::model::skill::{SkillEffect, TraitType, WeaknessTrait, WeaponTrait};

const ATTACKER: i32 = 5001;
const TARGET: i32 = 5002;
const DIST: &str = crate::data::DIST_GAME;

fn two_players() -> World {
    let (mut world, _db, _l) = combat_test_world();
    let _a = ingame_caster(&mut world, 1, ATTACKER, 0, 0);
    let _b = ingame_caster(&mut world, 2, TARGET, 0, 0);
    world
}

// ---------------------------------------------------------------------------
// The attacker-side accumulator
// ---------------------------------------------------------------------------

/// **The attack table's identity is 1.0, not 0** — the opposite of the defence
/// table — because the pair is consumed as `attackTrait − defenceTrait`.
/// Detect Beast Weakness (80) grants 30, i.e. 1.30.
#[test]
fn attack_traits_merge_onto_an_identity_of_one() {
    let mut world = two_players();
    let beast = TraitType::Weakness(WeaknessTrait::Beast);

    // Unbuffed: the *value* is already 1.0, but `hasAttackTrait` is false — and
    // several formulas gate on the latter, not the former.
    assert_eq!(
        calc_weakness_bonus(&world, ATTACKER, TARGET, TraitType::None),
        1.0
    );

    merge_attack_traits(&mut world, ATTACKER, &[(beast, 0.30)]);
    let traits = world
        .objects
        .get_component::<crate::model::components::AttackTraits>(&ATTACKER)
        .expect("the accumulator exists");
    assert!((traits.values[&beast] - 1.30).abs() < 1e-9, "1.0 + 30/100");

    // A second grant stacks; removing one leaves the other.
    merge_attack_traits(&mut world, ATTACKER, &[(beast, 0.20)]);
    assert!(
        (world
            .objects
            .get_component::<crate::model::components::AttackTraits>(&ATTACKER)
            .unwrap()
            .values[&beast]
            - 1.50)
            .abs()
            < 1e-9
    );
    remove_attack_traits(&mut world, ATTACKER, &[(beast, 0.20)]);
    remove_attack_traits(&mut world, ATTACKER, &[(beast, 0.30)]);
    assert!(
        !world
            .objects
            .get_component::<crate::model::components::AttackTraits>(&ATTACKER)
            .unwrap()
            .values
            .contains_key(&beast),
        "back at 1.0 → the entry goes, so `hasAttackTrait` is false again"
    );
}

/// The dist's own "Detect &lt;Category&gt; Weakness" line parses its traits.
#[test]
fn the_detect_weakness_skills_parse_their_traits() {
    let sd = crate::data::skill_data::SkillData::load_from(DIST);
    let beast = sd.get(80, 1).expect("Detect Beast Weakness");
    assert!(
        beast.effects.iter().any(|e| matches!(
            e,
            SkillEffect::AttackTrait { traits }
                if traits.contains(&(TraitType::Weakness(WeaknessTrait::Beast), 0.30))
        )),
        "{:?}",
        beast.effects
    );
    // Eye of Slayer carries four at once.
    let slayer = sd.get(360, 1).expect("Eye of Slayer");
    let count = slayer
        .effects
        .iter()
        .find_map(|e| match e {
            SkillEffect::AttackTrait { traits } => Some(traits.len()),
            _ => None,
        })
        .expect("an AttackTrait effect");
    assert_eq!(count, 4, "BEAST/GIANT/CONSTRUCT/DRAGON");
}

// ---------------------------------------------------------------------------
// calcWeaponTraitBonus
// ---------------------------------------------------------------------------

/// **Deflect Arrow really deflects arrows now.** The attacker's *weapon type* is
/// itself a trait, and the target's matching `DefenceTrait` softens the hit:
/// `max(0.22, 1 − defence)`.
#[test]
fn a_weapon_defence_trait_softens_that_weapons_hits() {
    let mut world = two_players();
    world.data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    // Bare-handed: no weapon trait at all, so nothing defends it.
    assert_eq!(calc_weapon_trait_bonus(&world, ATTACKER, TARGET), 1.0);

    // Deflect Arrow 4 is BOW 40 %.
    merge_defence_traits(
        &mut world,
        TARGET,
        &[(TraitType::Weapon(WeaponTrait::Bow), 0.40)],
    );
    // …but only against a bow.
    assert_eq!(
        calc_weapon_trait_bonus(&world, ATTACKER, TARGET),
        1.0,
        "an unarmed swing is not an arrow"
    );
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects
            .get_component_mut::<crate::model::inventory::Inventory>(&ATTACKER)
            .unwrap();
        let oid = inv.add_item(&data.item_data, 0x5300_0001, 13, 1); // Short Bow
        inv.equip_item(&data.item_data, oid);
    }
    assert!(
        (calc_weapon_trait_bonus(&world, ATTACKER, TARGET) - 0.60).abs() < 1e-9,
        "1 - 0.40"
    );
}

/// A **negative** weapon defence trait is a vulnerability — Provoke (286) takes
/// POLE −10, i.e. the taunt makes pole hits land *harder*. And the whole term
/// floors at Java's 0.22, however much resistance is stacked.
#[test]
fn a_negative_weapon_trait_is_a_vulnerability_and_the_floor_is_0_22() {
    let mut world = two_players();
    world.data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects
            .get_component_mut::<crate::model::inventory::Inventory>(&ATTACKER)
            .unwrap();
        let oid = inv.add_item(&data.item_data, 0x5300_0002, 13, 1);
        inv.equip_item(&data.item_data, oid);
    }
    let bow = TraitType::Weapon(WeaponTrait::Bow);

    merge_defence_traits(&mut world, TARGET, &[(bow, -0.10)]);
    assert!(
        (calc_weapon_trait_bonus(&world, ATTACKER, TARGET) - 1.10).abs() < 1e-9,
        "a negative trait multiplies damage up"
    );

    // Pile on resistance past 100 % and the floor holds. Note it takes *two*
    // sub-1.0 grants to get there: a single value ≥ 1.0 is not 100 % resistance
    // but outright invulnerability, which `merge_defence_traits` routes to a
    // different table entirely.
    merge_defence_traits(&mut world, TARGET, &[(bow, 0.9)]);
    merge_defence_traits(&mut world, TARGET, &[(bow, 0.9)]);
    assert!(
        (calc_weapon_trait_bonus(&world, ATTACKER, TARGET) - 0.22).abs() < 1e-9,
        "1 - 1.7 floors at 0.22, got {}",
        calc_weapon_trait_bonus(&world, ATTACKER, TARGET)
    );
}

// ---------------------------------------------------------------------------
// calcWeaknessBonus / calcAttackTraitBonus
// ---------------------------------------------------------------------------

/// A weakness needs **both sides**: the attacker's `AttackTrait` and the
/// target's `DefenceTrait`. Either alone is a no-op — which is exactly why
/// "Detect Beast Weakness" looked inert before the target side landed.
#[test]
fn a_weakness_needs_both_the_attack_and_the_defence_trait() {
    let beast = TraitType::Weakness(WeaknessTrait::Beast);

    // Attacker side only.
    let mut world = two_players();
    merge_attack_traits(&mut world, ATTACKER, &[(beast, 0.30)]);
    assert_eq!(
        calc_weakness_bonus(&world, ATTACKER, TARGET, TraitType::None),
        1.0
    );

    // Target side only.
    let mut world = two_players();
    merge_defence_traits(&mut world, TARGET, &[(beast, -0.15)]);
    assert_eq!(
        calc_weakness_bonus(&world, ATTACKER, TARGET, TraitType::None),
        1.0
    );

    // Both: `max(attackTrait - defenceTrait, 0.05)` = 1.30 - (-0.15) = 1.45.
    merge_attack_traits(&mut world, ATTACKER, &[(beast, 0.30)]);
    assert!(
        (calc_weakness_bonus(&world, ATTACKER, TARGET, TraitType::None) - 1.45).abs() < 1e-9,
        "the race skill's -15 vulnerability plus the Hunter's +30"
    );
}

/// `calcWeaknessBonus` **excludes the skill's own trait** — that one is already
/// counted by `calcGeneralTraitBonus`, and double-counting it would square the
/// bonus.
#[test]
fn the_skills_own_trait_is_excluded_from_the_weakness_product() {
    let beast = TraitType::Weakness(WeaknessTrait::Beast);
    let mut world = two_players();
    merge_attack_traits(&mut world, ATTACKER, &[(beast, 0.30)]);
    merge_defence_traits(&mut world, TARGET, &[(beast, -0.15)]);

    assert!((calc_weakness_bonus(&world, ATTACKER, TARGET, TraitType::None) - 1.45).abs() < 1e-9);
    assert_eq!(
        calc_weakness_bonus(&world, ATTACKER, TARGET, beast),
        1.0,
        "a BEAST_WEAKNESS skill does not count BEAST_WEAKNESS twice"
    );
}

/// `ignore_resistance` is what separates the damage formulas from the landing
/// roll: a **group-3** resistance short-circuits to 1.0 for damage (Stun
/// Resistance does not soften a stun's damage) but still scales the roll.
#[test]
fn ignore_resistance_short_circuits_the_resistable_group() {
    let mut world = two_players();
    merge_defence_traits(&mut world, TARGET, &[(TraitType::Shock, 0.30)]);

    assert!(
        (calc_general_trait_bonus(&world, ATTACKER, TARGET, TraitType::Shock, false) - 0.70).abs()
            < 1e-9,
        "the landing roll still sees the resistance"
    );
    assert_eq!(
        calc_general_trait_bonus(&world, ATTACKER, TARGET, TraitType::Shock, true),
        1.0,
        "the damage formulas do not"
    );
    // Invulnerability is still checked ahead of the group switch, either way.
    merge_defence_traits(&mut world, TARGET, &[(TraitType::Hold, 1.0)]);
    assert_eq!(
        calc_general_trait_bonus(&world, ATTACKER, TARGET, TraitType::Hold, true),
        0.0
    );
}

/// `calcAttackTraitBonus` is the auto-attack's whole trait term: the weapon
/// bonus times every group-2 weakness, floored at 0.05.
#[test]
fn the_auto_attack_trait_bonus_multiplies_weapon_and_weakness() {
    let mut world = two_players();
    world.data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects
            .get_component_mut::<crate::model::inventory::Inventory>(&ATTACKER)
            .unwrap();
        let oid = inv.add_item(&data.item_data, 0x5300_0003, 13, 1);
        inv.equip_item(&data.item_data, oid);
    }
    let bow = TraitType::Weapon(WeaponTrait::Bow);
    let beast = TraitType::Weakness(WeaknessTrait::Beast);

    assert_eq!(calc_attack_trait_bonus(&world, ATTACKER, TARGET), 1.0);

    merge_defence_traits(&mut world, TARGET, &[(bow, 0.40), (beast, -0.15)]);
    merge_attack_traits(&mut world, ATTACKER, &[(beast, 0.30)]);
    // 0.60 (Deflect Arrow) x 1.45 (the beast weakness) = 0.87.
    assert!(
        (calc_attack_trait_bonus(&world, ATTACKER, TARGET) - 0.87).abs() < 1e-9,
        "got {}",
        calc_attack_trait_bonus(&world, ATTACKER, TARGET)
    );
}

// ---------------------------------------------------------------------------
// End to end
// ---------------------------------------------------------------------------

/// The gate: the same **auto-attack**, against the same target, lands for less
/// once that target is defending the attacker's weapon type.
#[test]
fn an_auto_attack_is_softened_by_the_targets_weapon_defence() {
    let mut world = two_players();
    world.data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    let npc_oid = 0x4000_0333;
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

    let bonus_before = calc_attack_trait_bonus(&world, ATTACKER, npc_oid);
    merge_defence_traits(
        &mut world,
        npc_oid,
        &[(TraitType::Weapon(WeaponTrait::Bow), 0.40)],
    );
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects
            .get_component_mut::<crate::model::inventory::Inventory>(&ATTACKER)
            .unwrap();
        let oid = inv.add_item(&data.item_data, 0x5300_0004, 13, 1);
        inv.equip_item(&data.item_data, oid);
    }
    let bonus_after = calc_attack_trait_bonus(&world, ATTACKER, npc_oid);

    assert_eq!(bonus_before, 1.0);
    assert!(
        (bonus_after - 0.60).abs() < 1e-9,
        "the mob now takes 60 % from a bow: {bonus_after}"
    );
}

/// The race skills that make all of this reachable: `Undead` (4416) sits on
/// **13 547** NPC templates and carries negative `*_WEAKNESS` defence traits —
/// which is precisely what the Hunter's line is for.
#[test]
fn the_race_skills_carry_the_weakness_defence_traits() {
    let sd = crate::data::skill_data::SkillData::load_from(DIST);
    let undead = sd.get(4416, 2).expect("Undead lvl 2");
    let traits = undead
        .effects
        .iter()
        .find_map(|e| match e {
            SkillEffect::DefenceTrait { traits } => Some(traits.clone()),
            _ => None,
        })
        .expect("a DefenceTrait effect");
    assert!(
        traits
            .iter()
            .any(|(t, v)| matches!(t, TraitType::Weakness(_)) && *v < 0.0),
        "a negative weakness = a vulnerability: {traits:?}"
    );
}

/// **The gate, through the real swing.** `do_auto_attack` against the same mob
/// deals visibly less once that mob defends the attacker's weapon type — this
/// is what pins `calcAutoAttackDamage`'s `damage *= calcAttackTraitBonus(...)`
/// to the actual combat path rather than to the helper in isolation.
///
/// A **sword**, not a bow: a ranged swing needs ammunition loaded, and an
/// unarmed one carries no weapon trait to defend against.
#[test]
fn a_real_auto_attack_is_softened_by_the_weapon_trait() {
    use crate::model::components::Vitals;

    let swing = |trait_pct: Option<f64>| {
        let (mut world, _db, _l) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, ATTACKER, 0, 0);
        world.data.item_data = crate::data::item_data::ItemData::load_from(DIST);
        let npc_oid = 0x4000_0444;
        let (npc, extra) =
            crate::model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
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
        drain(&mut a_rx);
        {
            let World { objects, data, .. } = &mut world;
            let inv = objects
                .get_component_mut::<crate::model::inventory::Inventory>(&ATTACKER)
                .unwrap();
            let oid = inv.add_item(&data.item_data, 0x5300_0005, 1, 1); // Short Sword
            inv.equip_item(&data.item_data, oid);
        }
        if let Some(pct) = trait_pct {
            merge_defence_traits(
                &mut world,
                npc_oid,
                &[(TraitType::Weapon(WeaponTrait::Sword), pct)],
            );
        }
        let max = world
            .objects
            .get_component::<Vitals>(&npc_oid)
            .unwrap()
            .max_hp as f64;
        world
            .objects
            .get_component_mut::<Vitals>(&npc_oid)
            .unwrap()
            .cur_hp = max;
        // No random spread, and every roll the swing takes forced, so the two
        // swings differ **only** by the trait. Leaving the miss/crit rolls to
        // the RNG made this compare two independently-rolled swings, which is
        // flaky by construction — it failed roughly one full-suite run in ten.
        // Order: miss(1000), shield rate(100), shield perfect(100), crit(100).
        world
            .objects
            .get_component_mut::<crate::model::components::CombatStats>(&ATTACKER)
            .unwrap()
            .random_dmg = 0;
        world.forced_rolls.extend([0, 99, 99, 99]);
        crate::game_loop::combat::do_auto_attack(&mut world, ATTACKER, npc_oid);
        advance_ticks(&mut world, 40);
        max - world
            .objects
            .get_component::<Vitals>(&npc_oid)
            .unwrap()
            .cur_hp
    };

    let plain = swing(None);
    let deflected = swing(Some(0.40));
    assert!(plain > 0.0, "the swing landed: {plain}");
    assert!(
        deflected < plain,
        "the sword resistance softened it: {deflected} vs {plain}"
    );
}

/// And the **skill** paths multiply the same term in (`weaponTraitMod ·
/// generalTraitMod · weaknessMod`), which is a separate wiring from the
/// auto-attack's `calcAttackTraitBonus`.
#[test]
fn a_physical_skill_is_softened_by_the_weapon_trait_too() {
    use crate::model::components::Vitals;

    let hit = |with_trait: bool| {
        let (mut world, _db, _l) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, ATTACKER, 0, 0);
        world.data.item_data = crate::data::item_data::ItemData::load_from(DIST);
        let npc_oid = 0x4000_0555;
        let (npc, extra) =
            crate::model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
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
        drain(&mut a_rx);
        world
            .objects
            .get_component_mut::<crate::model::components::CombatStats>(&ATTACKER)
            .unwrap()
            .random_dmg = 0;
        {
            let World { objects, data, .. } = &mut world;
            let inv = objects
                .get_component_mut::<crate::model::inventory::Inventory>(&ATTACKER)
                .unwrap();
            let oid = inv.add_item(&data.item_data, 0x5300_0006, 13, 1);
            inv.equip_item(&data.item_data, oid);
        }
        if with_trait {
            merge_defence_traits(
                &mut world,
                npc_oid,
                &[(TraitType::Weapon(WeaponTrait::Bow), 0.40)],
            );
        }

        let mut skill = world.data.skill_data.get(1160, 1).expect("fixture").clone();
        skill.id = 9800;
        skill.magic_type = 0;
        skill.activate_rate = -1;
        skill.effects = vec![SkillEffect::PhysicalAttack {
            power: 50.0,
            p_atk_mod: 1.0,
            p_def_mod: 40.0,
            critical_chance: 0.0,
            ignore_shield_defence: true,
        }];

        let max = world
            .objects
            .get_component::<Vitals>(&npc_oid)
            .unwrap()
            .max_hp as f64;
        world
            .objects
            .get_component_mut::<Vitals>(&npc_oid)
            .unwrap()
            .cur_hp = max;
        world.forced_rolls.extend([0, 50]);
        crate::game_loop::skills::effects::apply_skill_effects(
            &mut world, ATTACKER, npc_oid, &skill,
        );
        max - world
            .objects
            .get_component::<Vitals>(&npc_oid)
            .unwrap()
            .cur_hp
    };

    let plain = hit(false);
    let deflected = hit(true);
    assert!(plain > 0.0, "the skill landed: {plain}");
    assert!(
        (deflected - plain * 0.60).abs() < 1e-6,
        "the weapon trait multiplied it by 0.60: {deflected} vs {plain}"
    );
}
