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
        trait_type: crate::model::skill::TraitType::None,
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
fn mob_world(
    skills: &[Skill],
) -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
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
        .insert(
            PLAYER,
            crate::model::npc::AggroInfo {
                hate: 100.0,
                damage: 0.0,
            },
        );
    if let Some(ai) = world
        .objects
        .get_component_mut::<crate::model::npc::NpcAi>(&NPC_OID)
    {
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
    let mut skill = npc_skill(
        NUKE,
        "Nuke",
        vec![SkillEffect::MagicalAttack { power: 50.0 }],
    );
    skill.cast_range = 600;
    let (mut world, _db, _l) = mob_world(&[skill]);
    rebuild_ai_index(&mut world);

    let ai = world.data.npc_ai_skills.get(MAGE_NPC).expect("indexed");
    assert_eq!(
        ai.get(AiSkillScope::Attack),
        &[(NUKE, 1)],
        "damage → ATTACK"
    );
    assert_eq!(
        ai.get(AiSkillScope::LongRange),
        &[(NUKE, 1)],
        "castRange 600 > 150 → LONG_RANGE"
    );
    assert!(ai.get(AiSkillScope::ShortRange).is_empty());
    assert_eq!(
        ai.get(AiSkillScope::General),
        &[(NUKE, 1)],
        "everything non-suicide is also GENERAL"
    );
}

#[test]
fn short_range_nuke_buckets_as_short_range() {
    let mut skill = npc_skill(
        NUKE,
        "Jab",
        vec![SkillEffect::PhysicalAttack {
            power: 20.0,
            p_atk_mod: 1.0,
            p_def_mod: 1.0,
            critical_chance: 0.0,
            ignore_shield_defence: false,
        }],
    );
    skill.cast_range = 40;
    let (mut world, _db, _l) = mob_world(&[skill]);
    rebuild_ai_index(&mut world);

    let ai = world.data.npc_ai_skills.get(MAGE_NPC).expect("indexed");
    assert_eq!(
        ai.get(AiSkillScope::ShortRange),
        &[(NUKE, 1)],
        "castRange 40 <= 150"
    );
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
        vec![
            SkillEffect::MagicalAttack { power: 10.0 },
            SkillEffect::DamOverTime {
                power: 5.0,
                ticks: 5,
                can_kill: false,
            },
        ],
    );
    skill.is_continuous = true;
    skill.is_debuff = true;
    skill.abnormal_type = "POISON".into();
    let (mut world, _db, _l) = mob_world(&[skill]);
    rebuild_ai_index(&mut world);

    let ai = world.data.npc_ai_skills.get(MAGE_NPC).expect("indexed");
    assert_eq!(ai.get(AiSkillScope::Debuff), &[(NUKE, 1)]);
    assert_eq!(
        ai.get(AiSkillScope::Cot),
        &[(NUKE, 1)],
        "a debuff is worth interrupting a caster with"
    );
    assert!(
        ai.get(AiSkillScope::Attack).is_empty(),
        "continuous arm wins over the attack arm"
    );
}

#[test]
fn passive_template_skills_are_not_ai_skills() {
    // The 4408/4410/4412 stat holders every mob carries must never be cast.
    let mut passive = npc_skill(NUKE, "HP Increase", vec![]);
    passive.operate_type = OperateType::Passive;
    let (mut world, _db, _l) = mob_world(&[passive]);
    rebuild_ai_index(&mut world);

    assert!(
        world.data.npc_ai_skills.get(MAGE_NPC).is_none(),
        "passive-only template gets no bucket entry"
    );
}

// ---------------------------------------------------------------------------
// The cast ladder.

#[test]
fn mage_mob_casts_at_its_target_and_deals_damage() {
    let (mut world, _db, _l) = mob_world(&[npc_skill(
        NUKE,
        "Nuke",
        vec![SkillEffect::MagicalAttack { power: 200.0 }],
    )]);
    let mut out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engage(&mut world);
    let hp_before = world
        .objects
        .get_component::<Vitals>(&PLAYER)
        .unwrap()
        .cur_hp;

    advance_world(&mut world, 30);

    let hp_after = world
        .objects
        .get_component::<Vitals>(&PLAYER)
        .unwrap()
        .cur_hp;
    assert!(
        hp_after < hp_before,
        "mob's nuke should have damaged the player ({hp_before} → {hp_after})"
    );
    let packets = drain(&mut out);
    assert!(
        packets
            .iter()
            .any(|p| p.first() == Some(&crate::network::server_packets::opcodes::MAGIC_SKILL_USE)),
        "the client must see MagicSkillUse so the mob plays its cast animation"
    );
}

#[test]
fn fighter_mob_without_the_roll_does_not_cast_while_moving() {
    // A non-MAGE only casts when standing still. Give it a nuke and set it
    // moving: `hasSkillChance` is never even rolled.
    let (mut world, _db, _l) = mob_world(&[npc_skill(
        NUKE,
        "Nuke",
        vec![SkillEffect::MagicalAttack { power: 200.0 }],
    )]);
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

/// A running MAGE must not cast either. The ladder guard in `try_cast` is
/// `(!npc.isMoving() && npc.hasSkillChance()) || (aiType == MAGE)` — the mage
/// arm deliberately skips the `isMoving()` test (its job is to skip the
/// skill-chance roll), so the thing that actually stops a running mage is
/// `Creature.doCast`'s first statement, "Attackables cannot cast while
/// moving". Without it every one of this dist's 402 MAGE templates nuked
/// mid-sprint.
#[test]
fn mage_mob_does_not_cast_while_running() {
    let (mut world, _db, _l) = mob_world(&[npc_skill(
        NUKE,
        "Nuke",
        vec![SkillEffect::MagicalAttack { power: 200.0 }],
    )]);
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

    crate::game_loop::npc_cast::try_cast(&mut world, NPC_OID, PLAYER);

    assert!(
        !world.objects.has_component::<Casting>(&NPC_OID),
        "a mage with move data in flight must not have started a cast"
    );
    assert!(
        world
            .objects
            .has_component::<crate::model::components::Movement>(&NPC_OID),
        "and the refusal must leave the chase alone — Java's doCast just returns"
    );

    // The zero case, so the assertion above is not passing for some unrelated
    // reason (no MP, target out of range, bucket empty): the same mob standing
    // still casts on the very next call.
    world
        .objects
        .remove_component::<crate::model::components::Movement>(&NPC_OID);

    crate::game_loop::npc_cast::try_cast(&mut world, NPC_OID, PLAYER);

    assert!(
        world.objects.has_component::<Casting>(&NPC_OID),
        "standing still, the same mage casts the same nuke at the same target"
    );
}

/// `thinkAttack`'s very first line is `if (npc.isCastingNow()) return;`. Before
/// that guard the think fell straight past `try_cast` — which refuses a
/// *second*, concurrent cast and so reports `false` — into the range tail and
/// re-issued `chase()` every second. That is the reported bug: mobs sprinting
/// at the player with the cast bar still up.
#[test]
fn a_mob_with_a_cast_in_flight_does_not_chase() {
    let (mut world, _db, _l) = mob_world(&[npc_skill(
        NUKE,
        "Nuke",
        vec![SkillEffect::MagicalAttack { power: 200.0 }],
    )]);
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engage(&mut world);
    // Well outside the 40-unit attack range, so the range tail would order a
    // chase — but still inside the leash from its (100, 0) spawn.
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::Position>(&NPC_OID)
    {
        p.x = 1000;
    }

    // A cast in flight, exactly as `start_cast` leaves it.
    world.objects.add_components(
        &NPC_OID,
        Casting(crate::model::CastState {
            skill_id: NUKE,
            skill_level: 1,
            skill_sub_level: 0,
            target_object_id: PLAYER,
            seq: 1,
            launched: false,
            cancel_ms: 0,
            cool_ms: 0,
            trigger_item_object_id: 0,
        }),
    );

    crate::game_loop::npc_ai::npc_ai_tick(&mut world);

    assert!(
        !world
            .objects
            .has_component::<crate::model::components::Movement>(&NPC_OID),
        "a mob mid-cast must not have been given a chase to run"
    );

    // The zero case: drop the cast and the identical think does order the
    // chase, so the assertion above is testing the guard and not a mob that
    // was never going to move.
    world.objects.remove_component::<Casting>(&NPC_OID);

    crate::game_loop::npc_ai::npc_ai_tick(&mut world);

    assert!(
        world
            .objects
            .has_component::<crate::model::components::Movement>(&NPC_OID),
        "with no cast in flight the same setup chases"
    );
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

    assert!(
        !cast_started,
        "the mob already carries MIGHT at this level — recasting it would be wasted"
    );
}

#[test]
fn mob_without_the_mp_does_not_cast() {
    let mut nuke = npc_skill(
        NUKE,
        "Expensive Nuke",
        vec![SkillEffect::MagicalAttack { power: 200.0 }],
    );
    nuke.mp_consume = 400;
    let (mut world, _db, _l) = mob_world(&[nuke]);
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engage(&mut world);
    world
        .objects
        .get_component_mut::<Vitals>(&NPC_OID)
        .unwrap()
        .cur_mp = 10.0;

    let cast_started = crate::game_loop::npc_cast::try_cast(&mut world, NPC_OID, PLAYER);

    assert!(!cast_started, "not enough MP");
}

#[test]
fn heal_is_skipped_at_full_hp_and_taken_when_wounded() {
    let mut heal = npc_skill(
        MOB_HEAL,
        "Mob Heal",
        vec![SkillEffect::Heal { power: 100.0 }],
    );
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
    assert!(
        world.objects.has_component::<Casting>(&NPC_OID),
        "the heal cast should be in flight"
    );
}

#[test]
fn a_mob_mid_cast_does_not_start_a_second_one() {
    let (mut world, _db, _l) = mob_world(&[npc_skill(
        NUKE,
        "Nuke",
        vec![SkillEffect::MagicalAttack { power: 50.0 }],
    )]);
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engage(&mut world);

    assert!(
        crate::game_loop::npc_cast::try_cast(&mut world, NPC_OID, PLAYER),
        "first cast starts"
    );
    assert!(
        !crate::game_loop::npc_cast::try_cast(&mut world, NPC_OID, PLAYER),
        "a mob already casting must not start another spell"
    );
}

// ---------------------------------------------------------------------------
// `Skill.getTarget` for an NPC caster (`checkSkillTarget`'s first line, and
// the re-resolve inside `SkillCaster.castSkill`).

/// The Catherok bug. Skill 4072 "Stun" is `targetType=SELF` /
/// `affectScope=POINT_BLANK` / `affectRange=150`: a shockwave centred on the
/// mob. The AI thinks about the hated player, but `doCast` resolves the target
/// through `Self.java`, so the cast lands on the **mob** and the ring decides
/// who is caught.
///
/// Casting it *at the player* instead made the player the primary affected
/// target, and the stun connected from however far away the mob happened to be
/// — which is how a monster standing on its spawn point stunned someone across
/// the field.
#[test]
fn a_self_target_skill_is_cast_at_the_caster_not_the_hated_player() {
    let mut stun = npc_skill(NUKE, "Stun", vec![]);
    stun.target_type = TargetType::Self_;
    stun.affect_scope = AffectScope::PointBlank;
    stun.affect_range = 150;
    stun.affect_object = AffectObject::NotFriend;
    stun.is_continuous = true;
    stun.is_debuff = true;
    stun.abnormal_type = "STUN".into();
    stun.abnormal_level = 1;
    stun.cast_range = -1;
    let (mut world, _db, _l) = mob_world(&[stun]);
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engage(&mut world); // mob at x=100, player at the origin

    assert!(crate::game_loop::npc_cast::try_cast(
        &mut world, NPC_OID, PLAYER
    ));

    let cast = world
        .objects
        .get_component::<Casting>(&NPC_OID)
        .expect("cast started");
    assert_eq!(
        cast.0.target_object_id, NPC_OID,
        "SELF resolves to the caster; the hated player is only who the AI was \
         thinking about"
    );
}

/// The control: an `ENEMY` skill still goes where the AI aimed it. Without
/// this, "resolve every NPC cast through the handlers" could silently turn
/// every mob nuke into a self-cast and the test above would still pass.
#[test]
fn an_enemy_skill_is_still_cast_at_the_hated_player() {
    let (mut world, _db, _l) = mob_world(&[npc_skill(
        NUKE,
        "Nuke",
        vec![SkillEffect::MagicalAttack { power: 50.0 }],
    )]);
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engage(&mut world);

    assert!(crate::game_loop::npc_cast::try_cast(
        &mut world, NPC_OID, PLAYER
    ));
    assert_eq!(
        world
            .objects
            .get_component::<Casting>(&NPC_OID)
            .expect("cast started")
            .0
            .target_object_id,
        PLAYER
    );
}

/// `Enemy.java` refuses a target that isn't `isAutoAttackable` by the caster,
/// and another monster is not. Before the handlers were consulted an NPC could
/// aim an offensive skill at anything the AI handed it.
#[test]
fn an_enemy_skill_is_refused_against_a_fellow_monster() {
    let (mut world, _db, _l) = mob_world(&[npc_skill(
        NUKE,
        "Nuke",
        vec![SkillEffect::MagicalAttack { power: 50.0 }],
    )]);
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engage(&mut world);
    let bystander = NPC_OID + 7;
    add_test_npc(&mut world, bystander, MAGE_NPC, "Monster", 5, 140, 0, 0);

    assert!(
        !crate::game_loop::npc_cast::try_cast(&mut world, NPC_OID, bystander),
        "ENEMY is not auto-attackable between two monsters"
    );
}

/// Porta's (20213) signature move, skill 4161 "Summon": `CallPc` on an `ENEMY`
/// target from a **monster** caster drags the victim onto the caster —
/// `stopMove`, `FlyToLocation(DUMMY)`, `setLocation(effector)`.
///
/// The effect had no handler, so the skill cast, animated for two seconds, and
/// did nothing; Porta read as a plain melee mob.
#[test]
fn a_monsters_call_pc_drags_the_player_onto_it() {
    let (mut world, _db, _l) = mob_world(&[npc_skill(NUKE, "Summon", vec![SkillEffect::CallPc])]);
    let mut out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engage(&mut world); // mob at x=100

    advance_world(&mut world, 30);

    let pos = world
        .objects
        .get_component::<crate::model::components::Position>(&PLAYER)
        .unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (100, 0),
        "the player is yanked onto the caster"
    );
    let packets = drain(&mut out);
    assert!(
        packets
            .iter()
            .any(|p| p.first() == Some(&crate::network::server_packets::opcodes::FLY_TO_LOCATION)),
        "FlyToLocation is what animates the drag; without it the client keeps \
         drawing the player where they were"
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
        let Some(ai) = index.get(template.id) else {
            continue;
        };
        for &(id, lvl) in ai.get(AiSkillScope::General) {
            let skill = skill_data.get(id, lvl).expect("bucketed skill resolves");
            assert_ne!(
                skill.operate_type,
                OperateType::Passive,
                "skill {id} is passive but was bucketed"
            );
            checked += 1;
        }
        if checked > 500 {
            break;
        }
    }
    assert!(checked > 0, "expected some bucketed active skills");
}
