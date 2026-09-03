//! Elemental attributes (G19, PLAN_G19_ATTRIBUTES.md): `calcAttributeBonus`'s
//! arithmetic, the dist parse of skill elements / the AttackAttribute+
//! DefenceAttribute effects / NPC template defences, and the damage behavior —
//! resistance reduces, Surrender-style debuffs restore, Holy Weapon elects.

use super::*;

use crate::model::components::{Buffs, StatModifiers, Vitals};
use crate::model::formulas::calc_attribute_bonus;
use crate::model::skill::active_buff::ActiveBuff;
use crate::model::skill::effects::{SkillEffect, StatModifierEffect};
use crate::model::skill::target::{OperateType, TargetType};
use crate::model::skill::{BuffSlot, Skill};
use crate::model::stats::{Element, Stat, StatModifierType};

const CASTER: i32 = 2001;
const CID: u32 = 1;

/// A template with a chosen fire resistance, sturdy enough to survive the
/// comparisons (deep HP, real m.def so a test nuke doesn't one-shot).
fn register_mob(world: &mut World, npc_id: i32, fire_res: i32) {
    let mut t = crate::data::npc_data::default_template(npc_id);
    t.type_name = "Monster".into();
    t.level = 5;
    t.base_hp_max = 100_000.0;
    t.base_mp_max = 100.0;
    t.base_m_def = 300.0;
    t.base_p_def = 300.0;
    t.base_element_res[Element::Fire.index()] = fire_res;
    world.data.npc_data.insert_for_test(t);
}

fn fire_nuke(id: i32) -> Skill {
    Skill {
        self_continuous: false,
        id,
        name: format!("Test Fire Nuke {id}"),
        operate_type: OperateType::Active,
        target_type: TargetType::Enemy,
        effect_point: -100,
        magic_type: 1,
        attribute_type: Some(Element::Fire),
        attribute_value: 20,
        effects: vec![SkillEffect::MagicalAttack { power: 50.0 }],
        ..Default::default()
    }
}

fn hp_of(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_hp
}

/// One cast with a pinned RNG tape, returning the HP it removed. Both sides
/// of every comparison use the same tape, so crit/failure rolls cancel out.
fn cast_damage(world: &mut World, caster: i32, target: i32, skill: &Skill) -> f64 {
    let before = hp_of(world, target);
    world.clear_forced_rolls();
    world.force_rolls([0, 0, 0, 0, 0, 0]);
    effects::apply_skill_effects(world, caster, target, skill);
    world.clear_forced_rolls();
    before - hp_of(world, target)
}

// ---------------------------------------------------------------------------
// The formula
// ---------------------------------------------------------------------------

/// `calcAttributeBonus`'s curve: 1.0 at parity, ≈1.031 at +20, ≈0.969 at −20,
/// hard caps 1.25 / 0.75 far out. The land-rate factor multiplies before the
/// clamp like every other mod.
#[test]
fn attribute_bonus_curve_and_caps() {
    assert_eq!(calc_attribute_bonus(0.0, 0.0), 1.0);
    assert_eq!(calc_attribute_bonus(50.0, 50.0), 1.0);
    assert!((calc_attribute_bonus(20.0, 0.0) - 1.031324).abs() < 1e-4);
    assert!((calc_attribute_bonus(0.0, 20.0) - 0.968675).abs() < 1e-4);
    assert_eq!(calc_attribute_bonus(300.0, 0.0), 1.25, "capped above");
    assert_eq!(calc_attribute_bonus(0.0, 300.0), 0.75, "capped below");

    // The land-rate element factor: a fire debuff vs a fire-weak target
    // (element_mod > 1) lands more often than vs a resistant one.
    let weak =
        formulas::calc_effect_land_rate(40, 50, 0, 40, 1.0, 1.2, 1.0, 0.0, 1.0, Default::default());
    let strong =
        formulas::calc_effect_land_rate(40, 50, 0, 40, 1.0, 0.8, 1.0, 0.0, 1.0, Default::default());
    assert!(weak > strong);
}

// ---------------------------------------------------------------------------
// Dist parse
// ---------------------------------------------------------------------------

/// Volcano carries FIRE 20; Holy Weapon 1043 is an `AttackAttribute HOLY +20`
/// (a `HolyPower` modifier); Day of Doom's aura 5145 debuffs four resistances
/// by 50; totem 13028's template declares 20 across the board.
#[test]
fn dist_attributes_parse() {
    let sd = dist::skills();
    let volcano = sd.get(1419, 1).expect("Volcano");
    assert_eq!(volcano.attribute_type, Some(Element::Fire));
    assert_eq!(volcano.attribute_value, 20);

    let holy_weapon = sd.get(1043, 1).expect("Holy Weapon");
    assert!(
        holy_weapon
            .stat_modifier_effects()
            .iter()
            .any(|m| m.stat == Stat::HolyPower && m.amount == 20.0),
        "Holy Weapon grants HolyPower +20: {:?}",
        holy_weapon.effects
    );

    let dod_aura = sd.get(5145, 1).expect("Day of Doom aura");
    let res_debuffs: Vec<_> = dod_aura
        .stat_modifier_effects()
        .into_iter()
        .filter(|m| {
            matches!(
                m.stat,
                Stat::FireRes | Stat::WaterRes | Stat::WindRes | Stat::EarthRes
            ) && m.amount == -50.0
        })
        .collect();
    assert_eq!(
        res_debuffs.len(),
        4,
        "four elemental −50s: {:?}",
        dod_aura.effects
    );

    let npcs = dist::npcs();
    let totem = npcs.get(13028).expect("totem 13028");
    assert_eq!(totem.base_element_res, [20; 6]);
}

// ---------------------------------------------------------------------------
// Behavior
// ---------------------------------------------------------------------------

/// A FIRE nuke does less to a fire-resistant mob than to a neutral one, and a
/// Surrender-style `DefenceAttribute` debuff on the resistant mob brings the
/// damage back up.
#[test]
fn fire_resistance_reduces_and_surrender_restores() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    register_mob(&mut world, 91400, 0);
    register_mob(&mut world, 91401, 60);
    let (neutral, resistant) = (NPC_OID, NPC_OID + 1);
    add_test_npc(&mut world, neutral, 91400, "Monster", 5, 100, 0, 0);
    add_test_npc(&mut world, resistant, 91401, "Monster", 5, 200, 0, 0);
    // add_test_npc spawns with 100/50 vitals regardless of template; deepen.
    for oid in [neutral, resistant] {
        let v = world.objects.get_component_mut::<Vitals>(&oid).unwrap();
        v.max_hp = 100_000;
        v.cur_hp = 100_000.0;
    }
    let nuke = fire_nuke(9400);

    let vs_neutral = cast_damage(&mut world, CASTER, neutral, &nuke);
    let vs_resistant = cast_damage(&mut world, CASTER, resistant, &nuke);
    assert!(vs_neutral > 0.0, "sanity: the nuke hurts");
    assert!(
        vs_resistant < vs_neutral,
        "60 fire res beats the nuke's FIRE 20: {vs_resistant} vs {vs_neutral}"
    );

    // Surrender to Fire's shape: FireRes −80 as a live debuff on the mob —
    // folded on read (NPCs keep no StatModifiers), so damage climbs past the
    // neutral case (net res −20 vs the skill's attack 20).
    if let Some(b) = world.objects.get_component_mut::<Buffs>(&resistant) {
        b.0.clear()
    }
    world.objects.add_components(
        &resistant,
        Buffs(vec![ActiveBuff {
            skill_id: 9401,
            abnormal_type: "SURRENDER".into(),
            abnormal_level: 1,
            slot: BuffSlot::Uncapped,
            effects: vec![StatModifierEffect {
                stat: Stat::FireRes,
                mode: StatModifierType::Diff,
                amount: -80.0,
                armor_condition: 0,
                weapon_condition: 0,
                qualifier: None,
                two_handed: false,
                hp_percent: 0,
            }],
            ..test_buff()
        }]),
    );
    let vs_surrendered = cast_damage(&mut world, CASTER, resistant, &nuke);
    assert!(
        vs_surrendered > vs_neutral,
        "net −20 res takes more than neutral: {vs_surrendered} vs {vs_neutral}"
    );
}

/// Holy Weapon's election: an attribute-**less** skill still gets the bonus
/// once the caster carries a POWER stat — the strongest element is elected
/// (Java `getAttackElement`'s scan).
#[test]
fn holy_weapon_colors_an_attributeless_skill() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    register_mob(&mut world, 91402, 0);
    let mob = NPC_OID;
    add_test_npc(&mut world, mob, 91402, "Monster", 5, 100, 0, 0);
    {
        let v = world.objects.get_component_mut::<Vitals>(&mob).unwrap();
        v.max_hp = 100_000;
        v.cur_hp = 100_000.0;
    }
    let mut nuke = fire_nuke(9402);
    nuke.attribute_type = None;
    nuke.attribute_value = 0;

    let plain = cast_damage(&mut world, CASTER, mob, &nuke);
    world
        .objects
        .get_component_mut::<StatModifiers>(&CASTER)
        .unwrap()
        .add
        .insert(Stat::HolyPower, 20.0);
    let blessed = cast_damage(&mut world, CASTER, mob, &nuke);
    assert!(
        blessed > plain,
        "HolyPower 20 elected onto the attribute-less nuke: {blessed} vs {plain}"
    );
}

/// **`calcAutoAttackDamage` multiplies by `calcAttributeBonus(attacker,
/// target, null)`** — with no skill to name an element, the attacker's
/// strongest POWER stat elects one. So a Holy Weapon-style buff colours plain
/// swings, not just skills; the port had been applying the elemental term on
/// the skill paths only.
#[test]
fn an_elemental_buff_reaches_plain_auto_attacks() {
    use crate::game_loop::combat;
    use crate::model::components::{StatModifiers, Vitals};
    use crate::model::stats::Stat;

    const ATTACKER: i32 = 8801;
    const TARGET: i32 = NPC_OID;

    fn damage_of(elemental: bool) -> f64 {
        let (mut world, _db, _l) = combat_test_world();
        let _rx = ingame_caster(&mut world, 1, ATTACKER, 0, 0);
        // The elemental term must be the only thing that moves between the two
        // runs; the sampling below covers what this cannot pin (see the helper).
        pin_swing_damage(&mut world, ATTACKER);
        if elemental {
            world
                .objects
                .get_component_mut::<StatModifiers>(&ATTACKER)
                .expect("mods")
                .add
                .insert(Stat::FirePower, 60.0);
        }
        add_test_npc(&mut world, TARGET, 20001, "Monster", 5, 0, 0, 0);
        {
            let v = world
                .objects
                .get_component_mut::<Vitals>(&TARGET)
                .expect("target");
            v.max_hp = 1_000_000;
            v.cur_hp = 1_000_000.0;
        }
        // The **smallest landed** swing over 40, which is the ordinary
        // non-crit hit: with the spread off, every plain swing deals exactly
        // the same amount, a crit deals strictly more, and a miss deals
        // nothing and is skipped. Taking the first landed swing instead made
        // this test fail about one run in fifty — a single crit slipping into
        // one of the two samples moved the ratio by the crit multiplier, which
        // dwarfs the 5 % the elemental term is worth.
        let mut smallest = f64::MAX;
        for _ in 0..40 {
            let before = world
                .objects
                .get_component::<Vitals>(&TARGET)
                .expect("target")
                .cur_hp;
            combat::do_auto_attack(&mut world, ATTACKER, TARGET);
            advance_ticks(&mut world, 60);
            let dealt = before
                - world
                    .objects
                    .get_component::<Vitals>(&TARGET)
                    .expect("target")
                    .cur_hp;
            if dealt > 0.0 {
                smallest = smallest.min(dealt);
            }
        }
        if smallest == f64::MAX { 0.0 } else { smallest }
    }

    let plain = damage_of(false);
    let fiery = damage_of(true);
    assert!(plain > 0.0, "the swing landed");

    // 60 attack vs 0 defence — the same ladder `calc_attribute_bonus` walks.
    let expected = calc_attribute_bonus(60.0, 0.0);
    assert!(expected > 1.0, "the fixture's element must actually pay");
    let ratio = fiery / plain;
    assert!(
        (ratio - expected).abs() < 0.02,
        "an elemental attacker should swing for ×{expected}, got ×{ratio} \
         ({fiery} vs {plain})"
    );
}
