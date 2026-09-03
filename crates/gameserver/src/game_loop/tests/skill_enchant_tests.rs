//! Skill enchanting slice 2 (PLAN_G19_SKILL_ENCHANT.md): the
//! `RequestExEnchantSkill` transaction — success, the NORMAL/BLESSED failure
//! split, the step validation, the Java payment quirks (items before SP,
//! adena-flavored +2 consume), and the enchanted variant reaching both the
//! cast pipeline and the load path.

use super::*;

use crate::data::enchant_skill_groups::EnchantSkillCost;
use crate::model::components::{SkillEnchants, Vitals};
use crate::model::skill::Skill;
use crate::model::skill::effects::SkillEffect;
use crate::model::skill::target::{OperateType, TargetType};

const CASTER: i32 = 2001;
const CID: u32 = 1;
const SKILL: i32 = 9500;
const CODEX: i32 = 30297;
const ADENA: i32 = 57;

/// The test skill at level 40 with a +1/+2 route on `power`.
fn install_enchant_data(world: &mut World) {
    let base = Skill {
        self_continuous: false,
        id: SKILL,
        level: 40,
        name: "Test Nuke".into(),
        operate_type: OperateType::Active,
        target_type: TargetType::Enemy,
        effect_point: -100,
        magic_type: 1,
        effects: vec![SkillEffect::MagicalAttack { power: 1.0 }],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(base.clone());
    for sub in 1001..=1005 {
        world.data.skill_data.insert_enchanted_for_test(Skill {
            self_continuous: false,
            sub_level: sub,
            // A big, visible power step so the cast test can tell +N landed.
            effects: vec![SkillEffect::MagicalAttack {
                power: 1.0 + (sub - 1000) as f64 * 10.0,
            }],
            ..base.clone()
        });
    }
    world
        .data
        .skill_data
        .insert_route_for_test(SKILL, 40, (1001, 1005));
    for step in 1..=5 {
        let mut cost = EnchantSkillCost {
            level: step,
            enchant_fail_level: 3,
            ..Default::default()
        };
        cost.sp.insert("NORMAL".into(), 1000);
        cost.sp.insert("BLESSED".into(), 1000);
        cost.chance.insert("NORMAL".into(), 90);
        cost.chance.insert("BLESSED".into(), 90);
        cost.items
            .insert("NORMAL".into(), vec![(CODEX, 1), (ADENA, 500)]);
        cost.items
            .insert("BLESSED".into(), vec![(CODEX, 1), (ADENA, 500)]);
        world.data.enchant_skill_groups.insert_for_test(cost);
    }
}

/// A 3rd-class caster who knows the skill, with SP and reagents.
fn enchanter(world: &mut World) -> UnboundedReceiver<bytes::Bytes> {
    let out = ingame_caster(world, CID, CASTER, 0, 0);
    install_enchant_data(world);
    // The gate reads `FOURTH_CLASS_GROUP` off CategoryData; the fixture
    // player's class id must be a member.
    let class_id = world
        .objects
        .get_component::<Player>(&CASTER)
        .unwrap()
        .class_id;
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[class_id]);
    world
        .objects
        .get_component_mut::<SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(SKILL, 40);
    if let Some(p) = world.objects.get_component_mut::<Player>(&CASTER) {
        p.sp = 100_000;
    }
    {
        let World { objects, data, .. } = world;
        let inv = objects.get_component_mut::<Inventory>(&CASTER).unwrap();
        inv.add_item(&data.item_data, 990_101, CODEX, 5);
        inv.add_item(&data.item_data, 990_102, ADENA, 100_000);
    }
    out
}

fn enchant_body(ty: i32, skill_id: i32, level: i16, sub: i16) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(ty);
    w.write_i32(skill_id);
    w.write_i16(level);
    w.write_i16(sub);
    w.into_bytes()
}

fn sub_of(world: &World, oid: i32, skill: i32) -> i32 {
    world
        .objects
        .get_component::<SkillEnchants>(&oid)
        .and_then(|e| e.0.get(&skill).copied())
        .unwrap_or(0)
}

fn count_of(world: &World, oid: i32, item: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&oid)
        .map(|i| i.count_of(item))
        .unwrap_or(0)
}

fn enchant(world: &mut World, ty: i32, sub: i16, roll: i32) {
    world.clear_forced_rolls();
    world.force_roll(roll);
    enchant::handle_request_enchant_skill(world, CID, &enchant_body(ty, SKILL, 40, sub));
    world.clear_forced_rolls();
}

/// The +1 happy path: codex + adena + SP paid, the sub-level lands, and the
/// next cast fires the enchanted variant (its beefed-up power shows in the
/// damage).
#[test]
fn a_successful_enchant_applies_and_casts_stronger() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = enchanter(&mut world);
    let mob = NPC_OID;
    add_test_npc(&mut world, mob, 20001, "Monster", 5, 100, 0, 0);
    {
        let v = world.objects.get_component_mut::<Vitals>(&mob).unwrap();
        v.max_hp = 100_000;
        v.cur_hp = 100_000.0;
    }
    let sp_before = world.objects.get_component::<Player>(&CASTER).unwrap().sp;
    let adena_before = count_of(&world, CASTER, ADENA);

    // Baseline damage at +0 (pinned RNG tape, same as the attribute tests).
    let hp0 = world.objects.get_component::<Vitals>(&mob).unwrap().cur_hp;
    world.force_rolls([0, 0, 0, 0]);
    let plain = world.data.skill_data.get(SKILL, 40).unwrap().clone();
    effects::apply_skill_effects(&mut world, CASTER, mob, &plain);
    world.clear_forced_rolls();
    let base_damage = hp0 - world.objects.get_component::<Vitals>(&mob).unwrap().cur_hp;

    enchant(&mut world, 0, 1001, 0); // NORMAL, roll 0 ≤ 90 → success
    assert_eq!(sub_of(&world, CASTER, SKILL), 1001, "+1 landed");
    assert_eq!(
        count_of(&world, CASTER, CODEX),
        4,
        "one codex consumed on the +1 step"
    );
    assert_eq!(
        count_of(&world, CASTER, ADENA),
        adena_before - 500,
        "the adena holder too"
    );
    assert_eq!(
        world.objects.get_component::<Player>(&CASTER).unwrap().sp,
        sp_before - 1000,
        "SP paid"
    );

    // The cast pipeline resolves the +1 variant: same tape, bigger hit.
    let hp1 = world.objects.get_component::<Vitals>(&mob).unwrap().cur_hp;
    world.force_rolls([0, 0, 0, 0]);
    use_magic_on(&mut world, CID, CASTER, SKILL, false, false, Some(mob));
    // The nuke has hit_time 0 → launch/finish next ticks.
    advance_ticks(&mut world, 12);
    world.clear_forced_rolls();
    let enchanted_damage = hp1 - world.objects.get_component::<Vitals>(&mob).unwrap().cur_hp;
    assert!(
        enchanted_damage > base_damage * 5.0,
        "the +1 variant's power 11 beats the base power 1: {enchanted_damage} vs {base_damage}"
    );
}

/// From +2 onward Java charges **adena** with every holder's count — the
/// codex stays, the wallet pays its count as adena on top.
#[test]
fn later_steps_charge_adena_instead_of_the_codex() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = enchanter(&mut world);
    enchant(&mut world, 0, 1001, 0);
    assert_eq!(count_of(&world, CASTER, CODEX), 4);
    let adena_after_first = count_of(&world, CASTER, ADENA);

    enchant(&mut world, 0, 1002, 0);
    assert_eq!(sub_of(&world, CASTER, SKILL), 1002);
    assert_eq!(
        count_of(&world, CASTER, CODEX),
        4,
        "the codex is never consumed past +1"
    );
    assert_eq!(
        count_of(&world, CASTER, ADENA),
        adena_after_first - 501,
        "both holders charged as adena: the codex's count (1) + the adena row (500)"
    );
}

/// A NORMAL failure drops the route to the row's `enchantFailLevel`; a
/// BLESSED failure keeps the current step.
#[test]
fn failure_modes_normal_resets_blessed_keeps() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = enchanter(&mut world);
    for step in 0..3 {
        enchant(&mut world, 0, 1001 + step, 0);
    }
    assert_eq!(sub_of(&world, CASTER, SKILL), 1003, "worked up to +3");

    // Roll 91 > 90 → failure. NORMAL: back to route base + failLevel (3).
    enchant(&mut world, 0, 1004, 99);
    assert_eq!(
        sub_of(&world, CASTER, SKILL),
        1003,
        "NORMAL fail resets to the fail step"
    );

    // Push to +4, then fail a BLESSED: the step must survive.
    enchant(&mut world, 0, 1004, 0);
    assert_eq!(sub_of(&world, CASTER, SKILL), 1004);
    enchant(&mut world, 1, 1005, 99);
    assert_eq!(
        sub_of(&world, CASTER, SKILL),
        1004,
        "BLESSED fail keeps the step"
    );
}

/// Step validation: an enchanted skill only advances by exactly one.
#[test]
fn skipping_steps_is_refused() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = enchanter(&mut world);
    enchant(&mut world, 0, 1001, 0);
    let codex_before = count_of(&world, CASTER, CODEX);

    enchant(&mut world, 0, 1004, 0); // +1 → +4: refused before any payment
    assert_eq!(sub_of(&world, CASTER, SKILL), 1001, "still +1");
    assert_eq!(
        count_of(&world, CASTER, CODEX),
        codex_before,
        "nothing was charged"
    );
}

/// The Java payment-order quirk, pinned: the items are consumed **before**
/// the SP check, so a broke enchanter loses the reagents and gains nothing.
#[test]
fn items_are_consumed_before_the_sp_check() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = enchanter(&mut world);
    if let Some(p) = world.objects.get_component_mut::<Player>(&CASTER) {
        p.sp = 10; // below the 1000 cost
    }
    enchant(&mut world, 0, 1001, 0);
    assert_eq!(sub_of(&world, CASTER, SKILL), 0, "no enchant");
    assert_eq!(
        count_of(&world, CASTER, CODEX),
        4,
        "…but the codex is gone (Java's order)"
    );
    assert_eq!(
        world.objects.get_component::<Player>(&CASTER).unwrap().sp,
        10,
        "SP untouched"
    );
}

/// A non-3rd-class player is refused outright.
#[test]
fn the_class_gate_refuses_lower_classes() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    install_enchant_data(&mut world);
    // No FOURTH_CLASS_GROUP membership registered for this class.
    world
        .objects
        .get_component_mut::<SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(SKILL, 40);
    if let Some(p) = world.objects.get_component_mut::<Player>(&CASTER) {
        p.sp = 100_000;
    }
    enchant(&mut world, 0, 1001, 0);
    assert_eq!(sub_of(&world, CASTER, SKILL), 0);
}

/// The load path: `character_skills` rows with a sub-level rebuild both the
/// book and the enchant map, and the `SkillList` packet carries the +N.
#[test]
fn enchants_survive_the_load_path() {
    let (world, _db, _l) = cast_test_world();
    let mut chr = dummy_char(CASTER, "Enchanted");
    chr.skills = vec![(SKILL, 40, 1003), (1177, 1, 0)];
    let player = Player::from_char(&world.data, &chr);
    assert_eq!(player.skills.0.get(&SKILL), Some(&40));
    assert_eq!(player.skill_enchants.0.get(&SKILL), Some(&1003));

    let pkt = crate::network::enter_world::skill_list(
        &player.skills,
        &player.skill_enchants,
        &model::components::ClanSkills::default(),
        &model::components::OptionSkills::default(),
        &world.data,
    );
    // Entry layout: d passive, h level, h sub, d id, … — find our skill's id
    // and read the sub two bytes back.
    let bytes = pkt.as_slice();
    let mut found = false;
    for i in 0..bytes.len().saturating_sub(4) {
        if bytes[i..i + 4] == SKILL.to_le_bytes() {
            let sub = i16::from_le_bytes([bytes[i - 2], bytes[i - 1]]);
            assert_eq!(sub, 1003, "SkillList carries the +3");
            found = true;
        }
    }
    assert!(found, "the skill is in the list");
}

/// The transaction-only busy refusals (`RequestExEnchantSkill.runImpl`):
/// a sell-buff store and a transformation both bounce the enchant with
/// nothing consumed, and the same request goes through once the player is
/// idle again.
#[test]
fn enchant_refused_while_busy() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = enchanter(&mut world);

    world
        .objects
        .get_component_mut::<Player>(&CASTER)
        .unwrap()
        .selling_buffs = true;
    enchant(&mut world, 0, 1001, 0); // the roll would succeed if it got that far
    assert_eq!(sub_of(&world, CASTER, SKILL), 0, "sell-buff store refusal");
    assert_eq!(count_of(&world, CASTER, CODEX), 5, "nothing consumed");

    {
        let p = world.objects.get_component_mut::<Player>(&CASTER).unwrap();
        p.selling_buffs = false;
        p.transform_id = 101;
    }
    enchant(&mut world, 0, 1001, 0);
    assert_eq!(sub_of(&world, CASTER, SKILL), 0, "transformed refusal");

    world
        .objects
        .get_component_mut::<Player>(&CASTER)
        .unwrap()
        .transform_id = 0;
    enchant(&mut world, 0, 1001, 0);
    assert_eq!(
        sub_of(&world, CASTER, SKILL),
        1001,
        "goes through once idle"
    );
}

/// Java's reuse hash spans the sub-level (`SkillData.getSkillHashCode`), so
/// any sub-level move orphans a running cooldown — the enchanted skill is
/// castable at once. A BLESSED failure keeps the step, so it also keeps the
/// cooldown.
#[test]
fn enchant_rekeys_the_running_cooldown() {
    use crate::model::components::Reuses;

    let (mut world, _db, _l) = cast_test_world();
    let _out = enchanter(&mut world);

    let arm_reuse = |world: &mut World| {
        let until = world.tick + 1_000;
        let mut map = std::collections::HashMap::new();
        map.insert(
            SKILL,
            model::SkillReuse {
                skill_level: 40,
                until_tick: until,
                total_ms: 60_000,
            },
        );
        world.objects.add_components(&CASTER, Reuses(map));
    };
    let has_reuse = |world: &World| {
        world
            .objects
            .get_component::<Reuses>(&CASTER)
            .is_some_and(|r| r.0.contains_key(&SKILL))
    };

    arm_reuse(&mut world);
    enchant(&mut world, 0, 1001, 0); // NORMAL success: 0 → 1001
    assert_eq!(sub_of(&world, CASTER, SKILL), 1001);
    assert!(
        !has_reuse(&world),
        "the sub-level moved — the old cooldown is orphaned"
    );

    arm_reuse(&mut world);
    enchant(&mut world, 1, 1002, 95); // BLESSED failure: stays at 1001
    assert_eq!(sub_of(&world, CASTER, SKILL), 1001);
    assert!(
        has_reuse(&world),
        "a BLESSED failure keeps the step, so it keeps the cooldown"
    );

    enchant(&mut world, 0, 1002, 95); // NORMAL failure: 1001 → fail level 1003
    assert_eq!(sub_of(&world, CASTER, SKILL), 1003);
    assert!(
        !has_reuse(&world),
        "a NORMAL failure also moves the sub-level, orphaning the cooldown"
    );
}
