//! NPC skill casting (G21): the `AttackableAI` cast ladder and the AI skill
//! scope buckets it walks.

use super::*;

use crate::data::npc_ai_skills::{AiSkillScope, NpcAiSkillIndex};
use crate::data::npc_data::AiType;
use crate::model::components::{Buffs, Casting, Vitals};
use crate::model::skill::{AffectObject, AffectScope, OperateType, Skill, SkillEffect, TargetType};

const PLAYER: i32 = 2001;
const CID: u32 = 1;
const MAGE_NPC: i32 = 41001;
const NUKE: i32 = 8400;
const SELF_BUFF: i32 = 8401;
const MOB_HEAL: i32 = 8402;

fn npc_skill(id: i32, name: &str, effects: Vec<SkillEffect>) -> Skill {
    Skill {
        without_action: false,
        item_consume_id: 0,
        item_consume_count: 0,
        id,
        level: 1,
        name: name.into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Enemy,
        magic_type: 1,
        magic_level: 1,
        effect_point: -100,
        cast_range: 600,
        effect_range: 900,
        // Instant cast keeps the tests to a single scheduler step.
        hit_time: 0,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 0,
        abnormal_level: 0,
        abnormal_type: "NONE".into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects,
        ..Default::default()
    }
}

/// A world with one MAGE-type mob that knows `skills`, engaged on the player.
/// MAGE is used for the behaviour tests because it casts on every think
/// without the `hasSkillChance()` roll — otherwise the assertions would be
/// probabilistic.
fn mob_world(skills: &[Skill]) -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    let mut t = crate::data::npc_data::default_template(MAGE_NPC);
    t.type_name = "Monster".into();
    t.name = "Test Caster".into();
    t.level = 5;
    t.base_hp_max = 500.0;
    t.base_mp_max = 500.0;
    t.base_m_atk = 100.0;
    t.base_atk_range = 40;
    t.collision_radius = 10.0;
    t.ai_type = AiType::Mage;
    t.skill_list = skills.iter().map(|s| (s.id, s.level)).collect();
    world.data.npc_data.insert_for_test(t);
    for s in skills {
        world.data.skill_data.insert_for_test(s.clone());
    }
    rebuild_ai_index(&mut world);
    (world, db, l)
}

/// The index is normally built once at `GameData::load_from`; tests register
/// templates afterwards, so rebuild it explicitly.
fn rebuild_ai_index(world: &mut World) {
    world.data.npc_ai_skills = NpcAiSkillIndex::build(&world.data.npc_data, &world.data.skill_data);
}

/// Put the mob in range and make it hate the player, so `think_attack` runs
/// with a live target.
fn engage(world: &mut World) -> i32 {
    add_test_npc(world, NPC_OID, MAGE_NPC, "Monster", 5, 100, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&NPC_OID)
        .unwrap()
        .0
        .insert(PLAYER, crate::model::npc::AggroInfo { hate: 100.0, damage: 0.0 });
    if let Some(ai) = world.objects.get_component_mut::<crate::model::npc::NpcAi>(&NPC_OID) {
        ai.intention = crate::model::npc::NpcIntention::Attack;
        ai.global_aggro = 0;
        ai.attack_timeout_tick = u64::MAX;
    }
    NPC_OID
}

// ---------------------------------------------------------------------------
// Bucketing (`NpcData.parse`'s AISkillScope ladder).

#[test]
fn nuke_buckets_as_attack_and_by_range() {
    let mut skill = npc_skill(NUKE, "Nuke", vec![SkillEffect::MagicalAttack { power: 50.0 }]);
    skill.cast_range = 600;
    let (mut world, _db, _l) = mob_world(&[skill]);
    rebuild_ai_index(&mut world);

    let ai = world.data.npc_ai_skills.get(MAGE_NPC).expect("indexed");
    assert_eq!(ai.get(AiSkillScope::Attack), &[(NUKE, 1)], "damage → ATTACK");
    assert_eq!(ai.get(AiSkillScope::LongRange), &[(NUKE, 1)], "castRange 600 > 150 → LONG_RANGE");
    assert!(ai.get(AiSkillScope::ShortRange).is_empty());
    assert_eq!(ai.get(AiSkillScope::General), &[(NUKE, 1)], "everything non-suicide is also GENERAL");
}

#[test]
fn short_range_nuke_buckets_as_short_range() {
    let mut skill = npc_skill(NUKE, "Jab", vec![SkillEffect::PhysicalAttack { power: 20.0, p_atk_mod: 1.0, p_def_mod: 1.0, critical_chance: 0.0 }]);
    skill.cast_range = 40;
    let (mut world, _db, _l) = mob_world(&[skill]);
    rebuild_ai_index(&mut world);

    let ai = world.data.npc_ai_skills.get(MAGE_NPC).expect("indexed");
    assert_eq!(ai.get(AiSkillScope::ShortRange), &[(NUKE, 1)], "castRange 40 <= 150");
    assert!(ai.get(AiSkillScope::LongRange).is_empty());
}

#[test]
fn continuous_debuff_buckets_as_debuff_and_cot_not_attack() {
    // A continuous skill takes the *first* ladder arm, so it never reaches the
    // ATTACK arm even though it also carries a damage effect. This is the
    // ordering Java relies on and the reason the ladder must not be reordered.
    let mut skill = npc_skill(
        NUKE,
        "Curse",
        vec![SkillEffect::MagicalAttack { power: 10.0 }, SkillEffect::DamOverTime { power: 5.0, ticks: 5, can_kill: false }],
    );
    skill.is_continuous = true;
    skill.is_debuff = true;
    skill.abnormal_type = "POISON".into();
    let (mut world, _db, _l) = mob_world(&[skill]);
    rebuild_ai_index(&mut world);

    let ai = world.data.npc_ai_skills.get(MAGE_NPC).expect("indexed");
    assert_eq!(ai.get(AiSkillScope::Debuff), &[(NUKE, 1)]);
    assert_eq!(ai.get(AiSkillScope::Cot), &[(NUKE, 1)], "a debuff is worth interrupting a caster with");
    assert!(ai.get(AiSkillScope::Attack).is_empty(), "continuous arm wins over the attack arm");
}

#[test]
fn passive_template_skills_are_not_ai_skills() {
    // The 4408/4410/4412 stat holders every mob carries must never be cast.
    let mut passive = npc_skill(NUKE, "HP Increase", vec![]);
    passive.operate_type = OperateType::Passive;
    let (mut world, _db, _l) = mob_world(&[passive]);
    rebuild_ai_index(&mut world);

    assert!(world.data.npc_ai_skills.get(MAGE_NPC).is_none(), "passive-only template gets no bucket entry");
}

// ---------------------------------------------------------------------------
// The cast ladder.

#[test]
fn mage_mob_casts_at_its_target_and_deals_damage() {
    let (mut world, _db, _l) = mob_world(&[npc_skill(NUKE, "Nuke", vec![SkillEffect::MagicalAttack { power: 200.0 }])]);
    let mut out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engage(&mut world);
    let hp_before = world.objects.get_component::<Vitals>(&PLAYER).unwrap().cur_hp;

    advance_world(&mut world, 30);

    let hp_after = world.objects.get_component::<Vitals>(&PLAYER).unwrap().cur_hp;
    assert!(hp_after < hp_before, "mob's nuke should have damaged the player ({hp_before} → {hp_after})");
    let packets = drain(&mut out);
    assert!(
        packets.iter().any(|p| p.first() == Some(&crate::network::server_packets::opcodes::MAGIC_SKILL_USE)),
        "the client must see MagicSkillUse so the mob plays its cast animation"
    );
}

#[test]
fn fighter_mob_without_the_roll_does_not_cast_while_moving() {
    // A non-MAGE only casts when standing still. Give it a nuke and set it
    // moving: `hasSkillChance` is never even rolled.
    let (mut world, _db, _l) = mob_world(&[npc_skill(NUKE, "Nuke", vec![SkillEffect::MagicalAttack { power: 200.0 }])]);
    {
        let mut t = world.data.npc_data.get(MAGE_NPC).unwrap().clone();
        t.ai_type = AiType::Fighter;
        world.data.npc_data.insert_for_test(t);
    }
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engage(&mut world);
    world.objects.add_components(
        &NPC_OID,
        crate::model::components::Movement(crate::model::movement::MoveData {
            start_x: 100,
            start_y: 0,
            start_z: 0,
            dest_x: 900,
            dest_y: 0,
            dest_z: 0,
            start_tick: world.tick,
            total_ticks: 100,
            geo_path: None,
        }),
    );

    let cast_started = crate::game_loop::npc_cast::try_cast(&mut world, NPC_OID, PLAYER);

    assert!(!cast_started, "a moving non-mage must not cast");
}

#[test]
fn mob_does_not_recast_a_buff_it_already_has() {
    let mut buff = npc_skill(SELF_BUFF, "Might", vec![]);
    buff.is_continuous = true;
    buff.is_debuff = false;
    buff.abnormal_type = "MIGHT".into();
    buff.abnormal_level = 1;
    buff.effect_point = 100;
    buff.target_type = TargetType::Self_;
    let (mut world, _db, _l) = mob_world(&[buff]);
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engage(&mut world);

    // Pre-load the abnormal at the same level.
    world.objects.add_components(
        &NPC_OID,
        Buffs(vec![crate::model::skill::ActiveBuff {
            skill_id: SELF_BUFF,
            skill_level: 1,
            abnormal_type_client_id: 0,
            abnormal_type: "MIGHT".into(),
            abnormal_level: 1,
            slot: crate::model::skill::BuffSlot::Buff,
            expires_at_tick: u64::MAX,
            passive: false,
            effect_flags: 0,
            abnormal_visuals: Vec::new(),
            blocked_abnormals: Vec::new(),
            effects: Vec::new(),
        }]),
    );

    let cast_started = crate::game_loop::npc_cast::try_cast(&mut world, NPC_OID, PLAYER);

    assert!(!cast_started, "the mob already carries MIGHT at this level — recasting it would be wasted");
}

#[test]
fn mob_without_the_mp_does_not_cast() {
    let mut nuke = npc_skill(NUKE, "Expensive Nuke", vec![SkillEffect::MagicalAttack { power: 200.0 }]);
    nuke.mp_consume = 400;
    let (mut world, _db, _l) = mob_world(&[nuke]);
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engage(&mut world);
    world.objects.get_component_mut::<Vitals>(&NPC_OID).unwrap().cur_mp = 10.0;

    let cast_started = crate::game_loop::npc_cast::try_cast(&mut world, NPC_OID, PLAYER);

    assert!(!cast_started, "not enough MP");
}

#[test]
fn heal_is_skipped_at_full_hp_and_taken_when_wounded() {
    let mut heal = npc_skill(MOB_HEAL, "Mob Heal", vec![SkillEffect::Heal { power: 100.0 }]);
    heal.effect_point = 100;
    heal.is_debuff = false;
    heal.target_type = TargetType::Self_;
    heal.cast_range = -1;
    let (mut world, _db, _l) = mob_world(&[heal]);
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engage(&mut world);

    // Full HP: `checkSkillTarget` rejects a heal on an undamaged target.
    assert!(
        !crate::game_loop::npc_cast::try_cast(&mut world, NPC_OID, PLAYER),
        "a healthy mob must not waste its heal"
    );

    // Below 33 % the heal chance `(100 - hp%) * 1.5` exceeds 100, so it is
    // certain — no flake.
    {
        let v = world.objects.get_component_mut::<Vitals>(&NPC_OID).unwrap();
        v.cur_hp = v.max_hp as f64 * 0.1;
    }
    assert!(
        crate::game_loop::npc_cast::try_cast(&mut world, NPC_OID, PLAYER),
        "at 10 % HP the heal chance is 135 % — the mob must heal itself"
    );
    assert!(world.objects.has_component::<Casting>(&NPC_OID), "the heal cast should be in flight");
}

#[test]
fn a_mob_mid_cast_does_not_start_a_second_one() {
    let (mut world, _db, _l) = mob_world(&[npc_skill(NUKE, "Nuke", vec![SkillEffect::MagicalAttack { power: 50.0 }])]);
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engage(&mut world);

    assert!(crate::game_loop::npc_cast::try_cast(&mut world, NPC_OID, PLAYER), "first cast starts");
    assert!(
        !crate::game_loop::npc_cast::try_cast(&mut world, NPC_OID, PLAYER),
        "a mob already casting must not start another spell"
    );
}

// ---------------------------------------------------------------------------
// Real datapack. Fixtures agree with whatever the code believes; the dist does
// not — so the index is also asserted against the actual NPC/skill XML.

#[test]
fn real_dist_index_buckets_a_known_caster() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let npc_data = crate::data::NpcData::load_from(DIST);
    let skill_data = crate::data::SkillData::load_from(DIST);
    let index = NpcAiSkillIndex::build(&npc_data, &skill_data);

    // ~5013 templates on this dist. A floor rather than the exact count so
    // datapack tweaks don't churn the test — but a high enough floor that a
    // parser regression dropping skill lists (as the nested-`<minions>` bug
    // once did for stats) fails here instead of quietly making mobs passive.
    assert!(
        index.len() > 4000,
        "expected ~5000 NPC templates with castable skills, got {}",
        index.len()
    );

    // Every bucketed entry must be a genuinely non-passive skill — the whole
    // point of the filter, and the thing a regression would silently break by
    // making mobs "cast" their stat holders.
    let mut checked = 0;
    for template in npc_data.all() {
        let Some(ai) = index.get(template.id) else { continue };
        for &(id, lvl) in ai.get(AiSkillScope::General) {
            let skill = skill_data.get(id, lvl).expect("bucketed skill resolves");
            assert_ne!(skill.operate_type, OperateType::Passive, "skill {id} is passive but was bucketed");
            checked += 1;
        }
        if checked > 500 {
            break;
        }
    }
    assert!(checked > 0, "expected some bucketed active skills");
}
