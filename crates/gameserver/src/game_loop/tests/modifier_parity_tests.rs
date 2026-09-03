//! **Modifier parity** — the bonuses every damage and land-rate formula takes
//! as an *input*.
//!
//! `crates/tools/tests/formula_parity.rs` sweeps the expressions that consume
//! these: attack, defence, crit, land rate, timing. It feeds them trait and
//! attribute multipliers off a grid of made-up constants, because the real ones
//! are not arithmetic over numbers — they are lookups over two per-trait tables
//! with their own defaults, membership tests and gates. So a wrong bonus is
//! invisible to all twenty-five of those sweeps, which is where this file comes
//! in: the same method, but the port side is driven through a real `World` with
//! the components set, against a transcription of Java that takes the tables as
//! plain values.
//!
//! The tables are the whole subtlety. `CreatureStat` fills
//! `_attackTraitValues` with **1** and `_defenceTraitValues` with **0** in its
//! constructor, and keeps a separate `Set` per side for `hasAttackTrait` /
//! `hasDefenceTrait` — so "absent" means 1.0 on one side, 0.0 on the other, and
//! *membership* is a third question that some gates ask instead of the value.
//! The port models the pair as two optional components; an absent component is
//! an empty table, not a reason to stop.

use super::*;

use crate::game_loop::skills::effects as port;
use crate::model::components::{AttackTraits, DefenceTraits, StatModifiers, Vitals};
use crate::model::skill::{TraitType, WeaknessTrait};
use crate::model::stats::Stat;

/// Transcriptions of Java's expressions. Nothing here touches the port.
mod java {
    use crate::model::skill::TraitType;

    /// `Formulas.calcGeneralTraitBonus`:
    ///
    /// ```java
    /// if (traitType == TraitType.NONE) return 1.0;
    /// if (target.getStat().isInvulnerableTrait(traitType)) return 0;
    /// switch (traitType.getType())
    /// {
    ///     case 2: if (!attacker.getStat().hasAttackTrait(traitType) || !target.getStat().hasDefenceTrait(traitType)) return 1.0; break;
    ///     case 3: if (ignoreResistance) return 1.0; break;
    ///     default: return 1.0;
    /// }
    /// return Math.max(attacker.getStat().getAttackTrait(traitType) - target.getStat().getDefenceTrait(traitType), 0.05);
    /// ```
    ///
    /// `attack`/`defence` are the *table* values — `None` meaning the trait is
    /// not in that side's set, which is Java's `hasAttackTrait` /
    /// `hasDefenceTrait`. The values it reads in that case are the constructor
    /// defaults: **1.0** attacking, **0.0** defending.
    pub fn general_trait_bonus(
        trait_type: TraitType,
        attack: Option<f64>,
        defence: Option<f64>,
        invulnerable: bool,
        ignore_resistance: bool,
    ) -> f64 {
        if trait_type == TraitType::None {
            return 1.0;
        }
        if invulnerable {
            return 0.0;
        }
        match trait_type.group() {
            2 => {
                if attack.is_none() || defence.is_none() {
                    return 1.0;
                }
            }
            3 => {
                if ignore_resistance {
                    return 1.0;
                }
            }
            _ => return 1.0,
        }
        (attack.unwrap_or(1.0) - defence.unwrap_or(0.0)).max(0.05)
    }

    /// `Formulas.calcWeaknessBonus`:
    ///
    /// ```java
    /// double result = 1;
    /// for (TraitType trait : TraitType.getAllWeakness())
    /// {
    ///     if ((traitType != trait) && target.getStat().hasDefenceTrait(trait) && attacker.getStat().hasAttackTrait(trait) && !target.getStat().isInvulnerableTrait(traitType))
    ///     {
    ///         result *= Math.max(attacker.getStat().getAttackTrait(trait) - target.getStat().getDefenceTrait(trait), 0.05);
    ///     }
    /// }
    /// return result;
    /// ```
    ///
    /// The invulnerability test reads the **skill's** trait, not the loop
    /// variable — that is Java's, and it makes an immunity to the skill's own
    /// trait suppress the whole product.
    pub fn weakness_bonus(
        skill_trait: TraitType,
        invulnerable_to_skill_trait: bool,
        attack: &dyn Fn(TraitType) -> Option<f64>,
        defence: &dyn Fn(TraitType) -> Option<f64>,
    ) -> f64 {
        let mut result = 1.0;
        for trait_type in TraitType::ALL_WEAKNESS {
            if trait_type != skill_trait
                && defence(trait_type).is_some()
                && attack(trait_type).is_some()
                && !invulnerable_to_skill_trait
            {
                result *= (attack(trait_type).unwrap_or(1.0) - defence(trait_type).unwrap_or(0.0))
                    .max(0.05);
            }
        }
        result
    }

    /// `Formulas.calcWeaponTraitBonus`:
    ///
    /// ```java
    /// return Math.max(0.22, 1.0 - target.getStat().getDefenceTrait(attacker.getAttackType().getTraitType()));
    /// ```
    ///
    /// No membership gate on this one — the raw table value is read, so an
    /// absent entry is a clean 1.0.
    pub fn weapon_trait_bonus(defence: Option<f64>) -> f64 {
        (1.0 - defence.unwrap_or(0.0)).max(0.22)
    }

    /// `Formulas.calcAttackTraitBonus`:
    ///
    /// ```java
    /// final double weaponTraitBonus = calcWeaponTraitBonus(attacker, target);
    /// if (weaponTraitBonus == 0) return 0;
    /// double weaknessBonus = 1.0;
    /// for (TraitType traitType : TraitType.values())
    /// {
    ///     if (traitType.getType() == 2)
    ///     {
    ///         weaknessBonus *= calcGeneralTraitBonus(attacker, target, traitType, true);
    ///         if (weaknessBonus == 0) return 0;
    ///     }
    /// }
    /// return Math.max(weaponTraitBonus * weaknessBonus, 0.05);
    /// ```
    pub fn attack_trait_bonus(
        weapon_defence: Option<f64>,
        invulnerable: &dyn Fn(TraitType) -> bool,
        attack: &dyn Fn(TraitType) -> Option<f64>,
        defence: &dyn Fn(TraitType) -> Option<f64>,
    ) -> f64 {
        let weapon = weapon_trait_bonus(weapon_defence);
        if weapon == 0.0 {
            return 0.0;
        }
        let mut weakness = 1.0;
        for trait_type in TraitType::ALL_WEAKNESS {
            weakness *= general_trait_bonus(
                trait_type,
                attack(trait_type),
                defence(trait_type),
                invulnerable(trait_type),
                true,
            );
            if weakness == 0.0 {
                return 0.0;
            }
        }
        (weapon * weakness).max(0.05)
    }
}

/// The grid: one group-3 trait (what the dist's `<trait>` tags almost all
/// declare), one group-2 weakness, one weapon trait and `NONE`.
fn traits_under_test() -> [TraitType; 4] {
    [
        TraitType::Shock,
        TraitType::Weakness(WeaknessTrait::Dragon),
        TraitType::Weapon(crate::model::skill::WeaponTrait::Bow),
        TraitType::None,
    ]
}

/// Table values swept on both sides. `None` is "not in the set", which is a
/// different input from `Some(1.0)`/`Some(0.0)` for every gate that asks
/// `hasAttackTrait`/`hasDefenceTrait`.
const ATTACK_VALUES: &[Option<f64>] = &[None, Some(1.0), Some(1.15), Some(1.71), Some(0.8)];
const DEFENCE_VALUES: &[Option<f64>] = &[None, Some(0.0), Some(0.3), Some(1.0), Some(-0.15)];

fn world_with_two_actors() -> (World, i32, i32) {
    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let attacker = 3001;
    let target = 3002;
    for (oid, name) in [(attacker, "Attacker"), (target, "Target")] {
        let chr = dummy_char(oid, name);
        Player::from_char(&world.data, &chr).spawn_into(&mut world);
    }
    (world, attacker, target)
}

/// Rebuilds both tables from scratch. The entries are the *table* values, so a
/// `None` is "not in that side's set" rather than a zero, and `invulnerable`
/// names the trait the target is immune to — which Java keys independently of
/// the resist entry, and which the weakness sweep needs to point at a trait the
/// tables don't otherwise carry. An empty table is left as an absent component,
/// which is the state most targets are in and which no formula here treats as a
/// short circuit.
fn set_tables(
    world: &mut World,
    attacker: i32,
    target: i32,
    attack: &[(TraitType, Option<f64>)],
    resist: &[(TraitType, Option<f64>)],
    invulnerable: Option<TraitType>,
) {
    world.objects.remove_component::<AttackTraits>(&attacker);
    world.objects.remove_component::<DefenceTraits>(&target);
    let mut at = AttackTraits::default();
    at.values
        .extend(attack.iter().filter_map(|&(t, v)| Some((t, v?))));
    if !at.values.is_empty() {
        world.objects.add_components(&attacker, at);
    }
    let mut dt = DefenceTraits::default();
    dt.resist
        .extend(resist.iter().filter_map(|&(t, v)| Some((t, v?))));
    dt.invulnerable.extend(invulnerable);
    if !dt.resist.is_empty() || !dt.invulnerable.is_empty() {
        world.objects.add_components(&target, dt);
    }
}

/// **The general-trait sweep.**
///
/// The finding: the port bailed out to 1.0 whenever the target carried no
/// `DefenceTraits` component at all. Java has no such exit — its arrays are
/// always present — so for a group-3 trait it computes
/// `max(attackTrait − 0, 0.05)`, which is the *attacker's* bonus. Most targets
/// carry no defence traits, so that early return was throwing away every
/// group-3 `AttackTrait` in the game: the four augment options (3952–3955), the
/// boss-jewel line, and Dual - Physical/Mental Trait Increase.
#[test]
fn general_trait_bonus_matches_java_across_the_grid() {
    let (mut world, attacker, target) = world_with_two_actors();
    let mut cases = 0usize;
    for trait_type in traits_under_test() {
        for &attack in ATTACK_VALUES {
            for &defence in DEFENCE_VALUES {
                for invulnerable in [false, true] {
                    for ignore_resistance in [false, true] {
                        set_tables(
                            &mut world,
                            attacker,
                            target,
                            &[(trait_type, attack)],
                            &[(trait_type, defence)],
                            invulnerable.then_some(trait_type),
                        );
                        let ours = port::calc_general_trait_bonus(
                            &world,
                            attacker,
                            target,
                            trait_type,
                            ignore_resistance,
                        );
                        let theirs = java::general_trait_bonus(
                            trait_type,
                            attack,
                            defence,
                            invulnerable,
                            ignore_resistance,
                        );
                        assert!(
                            (ours - theirs).abs() < 1e-9,
                            "general trait bonus diverged — {trait_type:?}, attack {attack:?}, \
                             defence {defence:?}, invulnerable {invulnerable}, ignore \
                             {ignore_resistance}: {ours} vs {theirs}"
                        );
                        cases += 1;
                    }
                }
            }
        }
    }
    assert!(cases > 300, "the grid collapsed to {cases} cases");
}

/// The case the sweep was opened for, spelled out: a stun-resistance augment
/// (`AttackTrait SHOCK 1.71` → a table value of 1.0171) casting a SHOCK skill
/// at a target with no defence traits at all.
#[test]
fn an_attack_trait_still_counts_against_an_untraited_target() {
    let (mut world, attacker, target) = world_with_two_actors();
    let mut at = AttackTraits::default();
    at.values.insert(TraitType::Shock, 1.0171);
    world.objects.add_components(&attacker, at);
    assert!(
        world
            .objects
            .get_component::<DefenceTraits>(&target)
            .is_none()
    );

    let bonus = port::calc_general_trait_bonus(&world, attacker, target, TraitType::Shock, false);
    assert!(
        (bonus - 1.0171).abs() < 1e-9,
        "Java reads max(attackTrait − 0, 0.05) here, not 1.0 ({bonus})"
    );
    // …and the damage formulas, which pass `ignoreResistance = true`, still
    // short-circuit group 3 to 1.0 — a stun resistance does not soften damage.
    assert_eq!(
        port::calc_general_trait_bonus(&world, attacker, target, TraitType::Shock, true),
        1.0
    );
}

/// **The weakness sweep.** Java's loop needs *both* sides in their sets, skips
/// the skill's own trait, and tests invulnerability against that skill trait
/// rather than the loop variable.
#[test]
fn weakness_bonus_matches_java_across_the_grid() {
    let (mut world, attacker, target) = world_with_two_actors();
    let dragon = TraitType::Weakness(WeaknessTrait::Dragon);
    let giant = TraitType::Weakness(WeaknessTrait::Giant);
    let mut cases = 0usize;
    for &attack in ATTACK_VALUES {
        for &defence in DEFENCE_VALUES {
            for invulnerable in [false, true] {
                for skill_trait in [dragon, giant, TraitType::Shock] {
                    // The tables carry DRAGON only, so a skill whose own trait
                    // is DRAGON must skip it and one that isn't must count it.
                    set_tables(
                        &mut world,
                        attacker,
                        target,
                        &[(dragon, attack)],
                        &[(dragon, defence)],
                        invulnerable.then_some(skill_trait),
                    );
                    let ours = port::calc_weakness_bonus(&world, attacker, target, skill_trait);
                    let theirs = java::weakness_bonus(
                        skill_trait,
                        invulnerable,
                        &|t| if t == dragon { attack } else { None },
                        &|t| if t == dragon { defence } else { None },
                    );
                    assert!(
                        (ours - theirs).abs() < 1e-9,
                        "weakness bonus diverged — skill trait {skill_trait:?}, attack \
                         {attack:?}, defence {defence:?}, invulnerable {invulnerable}: {ours} \
                         vs {theirs}"
                    );
                    cases += 1;
                }
            }
        }
    }
    assert!(cases > 100, "the grid collapsed to {cases} cases");
}

/// **The weapon and auto-attack trait sweep.** `calcWeaponTraitBonus` reads the
/// raw table with no membership gate and floors at 0.22;
/// `calcAttackTraitBonus` multiplies it by every group-2 trait at
/// `ignoreResistance = true` and floors the product at 0.05.
#[test]
fn weapon_and_attack_trait_bonuses_match_java_across_the_grid() {
    let (mut world, attacker, target) = world_with_two_actors();
    let dragon = TraitType::Weakness(WeaknessTrait::Dragon);
    let mut cases = 0usize;
    for &defence in DEFENCE_VALUES {
        for &weakness_attack in ATTACK_VALUES {
            for &weakness_defence in DEFENCE_VALUES {
                for invulnerable in [false, true] {
                    // Bare-handed here, so the weapon trait the port looks up
                    // is `TraitType::None` — the table entry that answers it is
                    // keyed the same way on both sides.
                    set_tables(
                        &mut world,
                        attacker,
                        target,
                        &[(dragon, weakness_attack)],
                        &[(TraitType::None, defence), (dragon, weakness_defence)],
                        invulnerable.then_some(dragon),
                    );

                    let ours = port::calc_weapon_trait_bonus(&world, attacker, target);
                    let theirs = java::weapon_trait_bonus(defence);
                    assert!(
                        (ours - theirs).abs() < 1e-9,
                        "weapon trait bonus diverged — defence {defence:?}: {ours} vs {theirs}"
                    );

                    let ours = port::calc_attack_trait_bonus(&world, attacker, target);
                    let theirs = java::attack_trait_bonus(
                        defence,
                        &|t| invulnerable && t == dragon,
                        &|t| if t == dragon { weakness_attack } else { None },
                        &|t| {
                            if t == dragon { weakness_defence } else { None }
                        },
                    );
                    assert!(
                        (ours - theirs).abs() < 1e-9,
                        "attack trait bonus diverged — weapon defence {defence:?}, weakness \
                         {weakness_attack:?}/{weakness_defence:?}, invulnerable \
                         {invulnerable}: {ours} vs {theirs}"
                    );
                    cases += 2;
                }
            }
        }
    }
    assert!(cases > 100, "the grid collapsed to {cases} cases");
}

/// **`Stat.weaponBaseValue`, the input the timing formulas were missing.**
///
/// `Formulas.calcAtkSpdMultiplier` opens on
/// `Stat.weaponBaseValue(creature, PHYSICAL_ATTACK_SPEED)`, and
/// `IStatFunction.calcWeaponBaseValue` resolves that to the **equipped
/// weapon's** declared attack speed for a player holding one — the class
/// template base is only the fallback. The port's finalizer honoured that for
/// the displayed `pAtkSpd` but the skill-timing path did not: it read
/// `template.base_p_atk_spd` and so gave a physical skill the same cast time
/// bare-handed as with a weapon.
#[test]
fn a_physical_skill_casts_at_the_weapons_attack_speed() {
    /// Short Sword — `<stat name="pAtkSpd">379</stat>` in the real item data.
    const SHORT_SWORD: i32 = 1;
    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.data.item_data = dist::items_owned();
    world.data.player_templates = dist::player_templates_owned();

    let oid = 3001;
    let chr = dummy_char(oid, "Swinger");
    Player::from_char(&world.data, &chr).spawn_into(&mut world);

    let template_base = {
        let p = world.objects.get_component::<Player>(&oid).expect("player");
        world
            .data
            .player_templates
            .get_or_base(p.class_id, p.base_class_id)
            .expect("class template")
            .base_p_atk_spd as f64
    };

    // Bare-handed: no weapon value, so the formula keeps the class base.
    {
        let inv = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&oid)
            .expect("inventory");
        assert_eq!(
            crate::model::weapon_base_stat(
                inv,
                &world.data,
                crate::model::stats::Stat::PhysicalAttackSpeed
            ),
            None
        );
    }

    // …and with the sword in hand, the weapon's own 379.
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects
            .get_component_mut::<crate::model::inventory::Inventory>(&oid)
            .expect("inventory");
        inv.add_item(&data.item_data, 0x5100_0001, SHORT_SWORD, 1);
        inv.equip_item(&data.item_data, 0x5100_0001);
    }
    let weapon_speed = {
        let inv = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&oid)
            .expect("inventory");
        crate::model::weapon_base_stat(
            inv,
            &world.data,
            crate::model::stats::Stat::PhysicalAttackSpeed,
        )
    };
    assert_eq!(
        weapon_speed,
        Some(379.0),
        "the Short Sword's declared speed"
    );
    assert_ne!(
        weapon_speed,
        Some(template_base),
        "the fixture is only meaningful while the two differ"
    );

    let p = world.objects.get_component::<Player>(&oid).expect("player");
    let base = world
        .objects
        .get_component::<crate::model::components::BaseStats>(&oid)
        .expect("base stats");
    let mods = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&oid)
        .expect("stat modifiers");
    let bare = crate::model::formulas::calc_atk_spd_multiplier(p, base, mods, &world.data, None);
    let armed =
        crate::model::formulas::calc_atk_spd_multiplier(p, base, mods, &world.data, weapon_speed);
    assert!(
        (armed / bare - 379.0 / template_base).abs() < 1e-9,
        "the multiplier scales by weapon/template ({armed} vs {bare}, base {template_base})"
    );
    assert!(
        armed > bare,
        "and a 379-speed sword is faster than the {template_base} class base"
    );
}

/// **`calcCounterAttack` reads its bonuses in Java's orientation.**
///
/// ```java
/// double counterdmg = ((target.getPAtk() * 873) / attacker.getPDef());
/// counterdmg *= calcWeaponTraitBonus(attacker, target);
/// counterdmg *= calcGeneralTraitBonus(attacker, target, skill.getTraitType(), true);
/// counterdmg *= calcAttributeBonus(attacker, target, skill);
/// ```
///
/// All three take `(attacker, target)` — the attacker being the one *taking*
/// the counter — so the weapon term reads the target's (the counter-attacker's)
/// resistance table even though the damage flows the other way. The port used
/// the shared `skill_trait_mod` helper with the actors swapped, which also
/// folded in a `calcWeaknessBonus` and the `generalTraitMod == 0 ? 1` guard
/// that only the `PhysicalAttack` handler family has.
///
/// The fixture puts a resistance on the **counter-attacker**: under Java's
/// orientation it halves the counter, under the swapped one it does nothing.
#[test]
fn the_counter_attack_reads_the_defenders_table_java_side_up() {
    /// Power Strike — `castRange` 40, `magicType` 0, no `<trait>`.
    const MELEE_SKILL: i32 = 3;
    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.data.skill_data = dist::skills_owned();

    let (attacker, counterer) = (3001, 3002);
    for (oid, name) in [(attacker, "Attacker"), (counterer, "Counterer")] {
        let chr = dummy_char(oid, name);
        Player::from_char(&world.data, &chr).spawn_into(&mut world);
    }
    for oid in [attacker, counterer] {
        let cs = world
            .objects
            .get_component_mut::<CombatStats>(&oid)
            .expect("combat stats");
        cs.p_atk = 100.0;
        cs.p_def = 100.0;
    }
    // `dummy_char` spawns with an empty HP bar, and a dead actor takes no
    // counter — give the one about to be countered something to lose.
    {
        let v = world
            .objects
            .get_component_mut::<Vitals>(&attacker)
            .expect("vitals");
        v.max_hp = 10_000;
        v.cur_hp = 10_000.0;
        v.dead = false;
    }
    // `CounterPhysicalSkill` grants a *chance*; 100 makes every roll counter.
    world
        .objects
        .get_component_mut::<StatModifiers>(&counterer)
        .expect("stat modifiers")
        .add
        .insert(Stat::VengeanceSkillPhysicalDamage, 100.0);

    let counter_damage = |world: &mut World, resist_on: Option<i32>| -> f64 {
        for oid in [attacker, counterer] {
            world.objects.remove_component::<DefenceTraits>(&oid);
        }
        if let Some(oid) = resist_on {
            let mut dt = DefenceTraits::default();
            // Bare-handed, so the weapon trait looked up is `TraitType::None`.
            dt.resist.insert(TraitType::None, 0.5);
            world.objects.add_components(&oid, dt);
        }
        // CP soaks damage before HP, so the fixture measures a bare HP drop.
        world
            .objects
            .get_component_mut::<crate::model::components::PlayerVitals>(&attacker)
            .expect("cp")
            .cur_cp = 0.0;
        let before = world
            .objects
            .get_component::<Vitals>(&attacker)
            .expect("vitals")
            .cur_hp;
        world.force_roll(0); // the counter chance roll
        crate::game_loop::skills::effects::damage::calc_counter_attack(
            world,
            attacker,
            counterer,
            MELEE_SKILL,
            false,
        );
        let after = world
            .objects
            .get_component::<Vitals>(&attacker)
            .expect("vitals")
            .cur_hp;
        // Refill so each run measures its own hit.
        let max_hp = world
            .objects
            .get_component::<Vitals>(&attacker)
            .expect("vitals")
            .max_hp as f64;
        world
            .objects
            .get_component_mut::<Vitals>(&attacker)
            .expect("vitals")
            .cur_hp = max_hp;
        before - after
    };

    let plain = counter_damage(&mut world, None);
    assert!(plain > 0.0, "the counter lands at all ({plain})");
    let resisted_on_counterer = counter_damage(&mut world, Some(counterer));
    let resisted_on_attacker = counter_damage(&mut world, Some(attacker));

    assert!(
        (resisted_on_counterer - plain * 0.5).abs() < 1e-6,
        "Java reads the table of the actor it calls `target` — the counter-attacker \
         ({resisted_on_counterer} vs {plain})"
    );
    assert!(
        (resisted_on_attacker - plain).abs() < 1e-6,
        "and not the one taking the damage ({resisted_on_attacker} vs {plain})"
    );
}
