//! Salvation's damage-handler modifiers: `PhysicalDamage`'s abnormal and race
//! terms, `MagicalDamage`'s debuff term, the two soul-damage handlers that
//! spend charges, and the handlers that multiply a raw `generalTraitMod`.

use super::*;
use crate::model::components::skills::Buffs;
use crate::model::skill::active_buff::ActiveBuff;
use crate::model::skill::effects::SkillEffect;

const CASTER: i32 = 6501;
const POWER_STRIKE: i32 = 3;
const WIND_STRIKE: i32 = 1177;

fn npc_oid() -> i32 {
    crate::game_loop::npc::FIRST_NPC_OBJECT_ID + 7901
}

fn world_with_target() -> World {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();
    let _rx = ingame_player_access(&mut world, 1, CASTER, 0);
    add_test_npc(&mut world, npc_oid(), 20001, "Monster", 20, 0, 0, 0);
    world
}

/// The HP `effect` takes off the target, cast as a copy of `base_id` carrying
/// only that effect, with the roll queue pinned so two casts differ only in
/// what the test changed.
fn hit(world: &mut World, base_id: i32, effect: SkillEffect, tweak: impl Fn(&mut Skill)) -> f64 {
    let npc = npc_oid();
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&npc) {
        v.max_hp = 1_000_000;
        v.cur_hp = 1_000_000.0;
    }
    let mut skill = skill_by_id(world, base_id, 1).expect("skill");
    skill.effects = vec![effect];
    tweak(&mut skill);
    world.clear_forced_rolls();
    world.force_rolls([50; 12]);
    effects::apply_skill_effects(world, CASTER, npc, &skill);
    1_000_000.0 - hp_of(world, npc)
}

fn physical(
    abnormals: &[&str],
    damage_mod: f64,
    power_mod: f64,
    races: Vec<crate::enums::Race>,
    race_mod: f64,
) -> SkillEffect {
    SkillEffect::PhysicalDamage {
        power: 300.0,
        p_atk_mod: 1.0,
        p_def_mod: 1.0,
        critical_chance: 0.0,
        ignore_shield_defence: true,
        abnormals: abnormals.iter().map(|a| a.to_string()).collect(),
        abnormal_damage_mod: damage_mod,
        abnormal_power_mod: power_mod,
        races,
        race_modifier: race_mod,
    }
}

/// Give the target one active (non-passive) buff of `skill_id` with
/// `abnormal_type`.
fn give_buff(world: &mut World, skill_id: i32, abnormal_type: &str) {
    let mut buff = ActiveBuff::passive_pump(skill_id, 1, Vec::new());
    buff.passive = false;
    buff.abnormal_type = abnormal_type.to_string();
    world.objects.add_components(&npc_oid(), Buffs(vec![buff]));
}

/// `PhysicalDamage`'s `damageModifier` multiplies the hit only while the
/// target carries one of the listed abnormal types.
#[test]
fn physical_damage_abnormal_modifier_needs_the_listed_abnormal() {
    let mut world = world_with_target();
    let effect = physical(&["PD_DOWN"], 2.0, 1.0, Vec::new(), 1.0);

    let plain = hit(&mut world, POWER_STRIKE, effect.clone(), |_| {});
    give_buff(&mut world, 9601, "ATTACK_TIME_UP");
    let other = hit(&mut world, POWER_STRIKE, effect.clone(), |_| {});
    give_buff(&mut world, 9601, "PD_DOWN");
    let listed = hit(&mut world, POWER_STRIKE, effect, |_| {});

    assert!(plain > 1.0, "the skill deals damage: {plain}");
    assert_eq!(other, plain, "an unlisted abnormal changes nothing");
    assert!(
        (listed - plain * 2.0).abs() < 1.0,
        "a listed abnormal doubles it: {plain} → {listed}"
    );
}

/// `raceModifier` applies against the listed races only.
#[test]
fn physical_damage_race_modifier_needs_a_listed_race() {
    let mut world = world_with_target();
    let race = crate::game_loop::skills::conditions::race_of(&world, npc_oid()).expect("npc race");
    let other_race = if race == crate::enums::Race::Human {
        crate::enums::Race::Elf
    } else {
        crate::enums::Race::Human
    };

    let plain = hit(
        &mut world,
        POWER_STRIKE,
        physical(&[], 1.0, 1.0, Vec::new(), 3.0),
        |_| {},
    );
    let unlisted = hit(
        &mut world,
        POWER_STRIKE,
        physical(&[], 1.0, 1.0, vec![other_race], 3.0),
        |_| {},
    );
    let listed = hit(
        &mut world,
        POWER_STRIKE,
        physical(&[], 1.0, 1.0, vec![race], 3.0),
        |_| {},
    );

    assert_eq!(unlisted, plain);
    assert!((listed - plain * 3.0).abs() < 1.0, "{plain} → {listed}");
}

/// `MagicalDamage`'s `debuffModifier` applies while the target carries any
/// debuff — `getDebuffCount() > 0`, which a plain buff does not raise.
#[test]
fn magical_damage_debuff_modifier_needs_a_debuff() {
    let mut world = world_with_target();
    let mut debuff = skill_by_id(&world, WIND_STRIKE, 1).expect("skill");
    debuff.id = 9602;
    debuff.is_debuff = true;
    debuff.effects = Vec::new();
    world.data.skill_data.insert_for_test(debuff.clone());
    let mut good = debuff;
    good.id = 9603;
    good.is_debuff = false;
    world.data.skill_data.insert_for_test(good);

    let effect = SkillEffect::MagicalDamage {
        power: 1000.0,
        debuff_modifier: 1.5,
        races: Vec::new(),
        race_modifier: 1.0,
    };
    let plain = hit(&mut world, WIND_STRIKE, effect.clone(), |_| {});
    give_buff(&mut world, 9603, "NONE");
    let buffed = hit(&mut world, WIND_STRIKE, effect.clone(), |_| {});
    give_buff(&mut world, 9602, "NONE");
    let debuffed = hit(&mut world, WIND_STRIKE, effect, |_| {});

    assert!(plain > 1.0, "the nuke deals damage: {plain}");
    assert_eq!(buffed, plain, "a buff is not a debuff");
    assert!((debuffed - plain * 1.5).abs() < 1.0, "{plain} → {debuffed}");
}

fn set_charges(world: &mut World, n: i32) {
    world
        .objects
        .get_component_mut::<Player>(&CASTER)
        .unwrap()
        .charges = n;
}

fn charges(world: &World) -> i32 {
    world
        .objects
        .get_component::<Player>(&CASTER)
        .unwrap()
        .charges
}

/// `PhysicalSoulDamage` spends `min(soulMaxConsumeCount, charges)` from the
/// **charges** pool and scales by `1 + souls·0.048`.
#[test]
fn physical_soul_damage_spends_charges_as_souls() {
    let mut world = world_with_target();
    let effect = SkillEffect::PhysicalSoulDamage {
        power: 300.0,
        critical_chance: 0.0,
        ignore_shield_defence: true,
    };
    let souls_3 = |s: &mut Skill| s.soul_max_consume_count = 3;

    set_charges(&mut world, 0);
    let none = hit(&mut world, POWER_STRIKE, effect.clone(), souls_3);
    set_charges(&mut world, 5);
    let three = hit(&mut world, POWER_STRIKE, effect, souls_3);

    assert!(none > 1.0, "no souls still hits: {none}");
    assert_eq!(charges(&world), 2, "5 - min(3, 5)");
    assert!(
        (three - none * 1.144).abs() < 1.0,
        "three souls: ×1.144 ({none} → {three})"
    );
}

/// `MagicalSoulDamage` scales the caster's mAtk by `1 + souls·0.022` before
/// `calcMagicDam`, which takes its square root.
#[test]
fn magical_soul_damage_scales_matk_by_the_souls() {
    let mut world = world_with_target();
    let effect = SkillEffect::MagicalSoulDamage { power: 1000.0 };
    let souls_3 = |s: &mut Skill| s.soul_max_consume_count = 3;

    set_charges(&mut world, 0);
    let none = hit(&mut world, WIND_STRIKE, effect.clone(), souls_3);
    set_charges(&mut world, 4);
    let three = hit(&mut world, WIND_STRIKE, effect, souls_3);

    assert_eq!(charges(&world), 1, "4 - min(3, 4)");
    let want = none * (1.0f64 + 3.0 * 0.022).sqrt();
    assert!((three - want).abs() < 1.0, "{none} → {three}, want {want}");
}

/// `PhysicalDamage` treats a trait-invulnerable target's `generalTraitMod`
/// of 0 as 1; `PhysicalSoulDamage` multiplies it raw, so it deals nothing.
#[test]
fn the_soul_handler_multiplies_a_raw_general_trait() {
    use crate::model::skill::traits::TraitType;
    let mut world = world_with_target();
    let mut traits = model::components::stats::DefenceTraits::default();
    traits.invulnerable.insert(TraitType::Shock);
    world.objects.add_components(&npc_oid(), traits);
    let shock = |s: &mut Skill| s.trait_type = TraitType::Shock;

    let guarded = hit(
        &mut world,
        POWER_STRIKE,
        physical(&[], 1.0, 1.0, Vec::new(), 1.0),
        shock,
    );
    let raw = hit(
        &mut world,
        POWER_STRIKE,
        SkillEffect::PhysicalSoulDamage {
            power: 300.0,
            critical_chance: 0.0,
            ignore_shield_defence: true,
        },
        shock,
    );

    assert!(guarded > 1.0, "the guarded handler still hits: {guarded}");
    assert_eq!(raw, 0.0, "the raw one is zeroed");
}

/// The soul handlers are player-only: an NPC caster lands nothing.
#[test]
fn an_npc_caster_lands_no_soul_damage() {
    let mut world = world_with_target();
    let victim = npc_oid() + 1;
    add_test_npc(&mut world, victim, 20001, "Monster", 20, 0, 0, 0);
    let skill = {
        let mut s = skill_by_id(&world, POWER_STRIKE, 1).expect("skill");
        s.effects = vec![SkillEffect::PhysicalSoulDamage {
            power: 300.0,
            critical_chance: 0.0,
            ignore_shield_defence: true,
        }];
        s
    };
    let before = hp_of(&world, victim);
    effects::apply_skill_effects(&mut world, npc_oid(), victim, &skill);
    assert_eq!(hp_of(&world, victim), before);
}

/// The dist side: Salvation's `PhysicalAttack`/`MagicalAttack` are stat
/// effects, the damage handlers are `*Damage`, and the soul and energy
/// handlers read their own params.
#[test]
fn salvation_damage_names_parse_from_the_dist() {
    let sd = dist::skills();
    let ws = sd.get(WIND_STRIKE, 1).expect("Wind Strike");
    assert!(matches!(
        ws.effects.as_slice(),
        [SkillEffect::MagicalDamage { power, debuff_modifier, race_modifier, .. }]
            if *power > 0.0 && *debuff_modifier == 1.0 && *race_modifier == 1.0
    ));
    // Might 1068: a `PhysicalAttack` stat buff, not a damage effect.
    let might = sd.get(1068, 1).expect("Might");
    assert!(
        might
            .stat_modifier_effects()
            .iter()
            .any(|e| e.stat == model::stats::Stat::PhysicalAttack),
        "{:?}",
        might.effects
    );
    assert!(
        !might
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::PhysicalDamage { .. })),
    );
    // Skill 30500 carries `PhysicalDamage` with `abnormalType`/`damageModifier`.
    let abn = sd.get(30500, 1).expect("skill 30500");
    assert!(
        abn.effects.iter().any(|e| matches!(
            e,
            SkillEffect::PhysicalDamage { abnormals, abnormal_damage_mod, .. }
                if !abnormals.is_empty() && *abnormal_damage_mod != 1.0
        )),
        "{:?}",
        abn.effects
    );
    // Fallen Arrow 1431 is a `MagicalSoulDamage` nuke.
    let fallen = sd.get(1431, 1).expect("skill 1431");
    assert!(
        fallen
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::MagicalSoulDamage { .. }))
    );
    // Sonic Blaster 6 reads `chargeConsume` off its own effect.
    let sonic = sd.get(6, 1).expect("Sonic Blaster");
    assert!(
        sonic.effects.iter().any(|e| matches!(
            e,
            SkillEffect::EnergyDamage { charge_consume, .. } if *charge_consume > 0
        )),
        "{:?}",
        sonic.effects
    );
}
