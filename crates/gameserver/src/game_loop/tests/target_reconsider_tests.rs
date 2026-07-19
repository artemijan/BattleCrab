//! `skillTargetReconsider` (G21 slice 8): a support mob heals/buffs its pack
//! instead of only itself.

use super::*;

use crate::data::npc_data::{AiType, MinionHolder};
use crate::model::components::{Casting, Vitals};
use crate::model::npc::{AggroInfo, AggroList, NpcAi, NpcIntention};
use crate::model::skill::{AffectObject, AffectScope, OperateType, Skill, SkillEffect, TargetType};

const PLAYER: i32 = 2001;
const CID: u32 = 1;
const HEALER_ID: i32 = 45000;
const ALLY_ID: i32 = 45001;
const STRANGER_ID: i32 = 45002;
const HEAL_SKILL: i32 = 8600;
const BUFF_SKILL: i32 = 8601;

const HEALER: i32 = NPC_OID;
const ALLY: i32 = NPC_OID + 1;
const STRANGER: i32 = NPC_OID + 2;

fn support_skill(id: i32, effects: Vec<SkillEffect>, continuous: bool) -> Skill {
    Skill {
        id,
        level: 1,
        name: format!("Support {id}"),
        operate_type: OperateType::Active,
        is_continuous: continuous,
        target_type: TargetType::Target,
        magic_type: 1,
        magic_level: 1,
        // Positive effect points: a *good* skill, which is what routes it down
        // the faction branch rather than the aggro-list branch.
        effect_point: 100,
        cast_range: 600,
        effect_range: 900,
        hit_time: 0,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: if continuous { 100 } else { 0 },
        abnormal_level: 1,
        abnormal_type: if continuous { "MIGHT".into() } else { "NONE".into() },
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
    }
}

fn template(id: i32, clans: &[&str], skills: &[(i32, i32)]) -> crate::data::npc_data::NpcTemplate {
    let mut t = crate::data::npc_data::default_template(id);
    t.type_name = "Monster".into();
    t.name = format!("Mob {id}");
    t.level = 20;
    t.base_hp_max = 1000.0;
    t.base_mp_max = 500.0;
    t.collision_radius = 10.0;
    // MAGE casts every think, so the tests aren't probabilistic.
    t.ai_type = AiType::Mage;
    t.clan_help_range = 500;
    t.clans = clans.iter().map(|s| s.to_string()).collect();
    t.skill_list = skills.to_vec();
    t.minions = Vec::<MinionHolder>::new();
    t
}

fn support_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    world.data.skill_data.insert_for_test(support_skill(HEAL_SKILL, vec![SkillEffect::Heal { power: 300.0 }], false));
    world.data.skill_data.insert_for_test(support_skill(
        BUFF_SKILL,
        vec![SkillEffect::StatModifier(crate::model::skill::StatModifierEffect {
            stat: crate::model::stats::Stat::PhysicalAttack,
            mode: crate::model::stats::StatModifierType::Per,
            amount: 1.2,
            armor_condition: 0,
            weapon_condition: 0,
        })],
        true,
    ));
    world.data.npc_data.insert_for_test(template(HEALER_ID, &["ORC"], &[(HEAL_SKILL, 1), (BUFF_SKILL, 1)]));
    world.data.npc_data.insert_for_test(template(ALLY_ID, &["ORC"], &[]));
    world.data.npc_data.insert_for_test(template(STRANGER_ID, &["LIZARDMAN"], &[]));
    world.data.npc_ai_skills =
        crate::data::npc_ai_skills::NpcAiSkillIndex::build(&world.data.npc_data, &world.data.skill_data);
    world.next_npc_object_id = STRANGER + 1;
    (world, db, l)
}

/// Place the healer plus an optional companion, and engage the healer so
/// `try_cast` runs with a live target.
fn scene(world: &mut World, companion_id: Option<i32>, companion_oid: i32, companion_x: i32) {
    add_test_npc(world, HEALER, HEALER_ID, "Monster", 20, 0, 0, 0);
    if let Some(cid) = companion_id {
        add_test_npc(world, companion_oid, cid, "Monster", 20, companion_x, 0, 0);
        let v = world.objects.get_component_mut::<Vitals>(&companion_oid).unwrap();
        v.max_hp = 1000;
        v.cur_hp = 1000.0;
    }
    {
        let v = world.objects.get_component_mut::<Vitals>(&HEALER).unwrap();
        v.max_hp = 1000;
        v.max_mp = 500;
        v.cur_hp = 1000.0;
        v.cur_mp = 500.0;
    }
    world
        .objects
        .get_component_mut::<AggroList>(&HEALER)
        .unwrap()
        .0
        .insert(PLAYER, AggroInfo { hate: 100.0, damage: 50.0 });
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&HEALER) {
        ai.intention = NpcIntention::Attack;
        ai.global_aggro = 0;
        ai.attack_timeout_tick = u64::MAX;
    }
}

fn wound(world: &mut World, oid: i32, cur_hp: f64) {
    let v = world.objects.get_component_mut::<Vitals>(&oid).unwrap();
    v.max_hp = 1000;
    v.cur_hp = cur_hp;
}

/// Who is the healer currently casting at?
fn cast_target(world: &World) -> Option<i32> {
    world.objects.get_component::<Casting>(&HEALER).map(|c| c.0.target_object_id)
}

// ---------------------------------------------------------------------------

#[test]
fn a_healer_heals_its_wounded_faction_mate_not_itself() {
    // The whole point of the slice: before this, heal always resolved to self.
    let (mut world, _db, _l) = support_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    scene(&mut world, Some(ALLY_ID), ALLY, 200);
    wound(&mut world, ALLY, 50.0); // 5 % — heal chance 142 %, certain

    assert!(crate::game_loop::npc_cast::try_cast(&mut world, HEALER, PLAYER), "a cast starts");

    assert_eq!(cast_target(&world), Some(ALLY), "the wounded ally is the target");
}

#[test]
fn a_healer_picks_the_worst_off_of_several_allies() {
    let (mut world, _db, _l) = support_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    scene(&mut world, Some(ALLY_ID), ALLY, 200);
    add_test_npc(&mut world, STRANGER, ALLY_ID, "Monster", 20, 250, 0, 0);
    wound(&mut world, ALLY, 400.0); // 40 %
    wound(&mut world, STRANGER, 60.0); // 6 % — worse

    assert!(crate::game_loop::npc_cast::try_cast(&mut world, HEALER, PLAYER));

    assert_eq!(cast_target(&world), Some(STRANGER), "heal goes to the lowest HP percentage");
}

#[test]
fn a_healer_still_heals_itself_when_it_is_the_worst_off() {
    let (mut world, _db, _l) = support_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    scene(&mut world, Some(ALLY_ID), ALLY, 200);
    wound(&mut world, ALLY, 900.0); // 90 %
    wound(&mut world, HEALER, 40.0); // 4 %

    assert!(crate::game_loop::npc_cast::try_cast(&mut world, HEALER, PLAYER));

    assert_eq!(cast_target(&world), Some(HEALER), "Java adds self to the candidate list");
}

#[test]
fn a_different_faction_is_not_healed() {
    let (mut world, _db, _l) = support_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    scene(&mut world, Some(STRANGER_ID), ALLY, 200);
    wound(&mut world, ALLY, 50.0); // badly hurt, but LIZARDMAN
    wound(&mut world, HEALER, 500.0); // 50 %

    assert!(crate::game_loop::npc_cast::try_cast(&mut world, HEALER, PLAYER));

    assert_eq!(cast_target(&world), Some(HEALER), "an ORC healer ignores a wounded LIZARDMAN");
}

#[test]
fn a_wounded_player_is_never_healed() {
    // Java's candidate set is *every* visible creature, and its
    // auto-attackable filter sits inside the isContinuous() branch — a heal
    // isn't continuous, so as written a mob would heal the player fighting it.
    // The port scopes good-skill targets to the caster's faction instead.
    let (mut world, _db, _l) = support_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    scene(&mut world, None, ALLY, 0);
    wound(&mut world, HEALER, 500.0);
    {
        let v = world.objects.get_component_mut::<Vitals>(&PLAYER).unwrap();
        v.cur_hp = 1.0; // far worse off than the healer
    }

    assert!(crate::game_loop::npc_cast::try_cast(&mut world, HEALER, PLAYER));

    assert_eq!(cast_target(&world), Some(HEALER), "the mob must never heal the player attacking it");
}

#[test]
fn an_ally_out_of_range_is_not_considered() {
    let (mut world, _db, _l) = support_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    // 2000 is Java's reconsider range for a non-cast-range lookup; put the
    // ally well beyond it.
    scene(&mut world, Some(ALLY_ID), ALLY, 5000);
    wound(&mut world, ALLY, 50.0);
    wound(&mut world, HEALER, 500.0);

    assert!(crate::game_loop::npc_cast::try_cast(&mut world, HEALER, PLAYER));

    assert_eq!(cast_target(&world), Some(HEALER), "a distant ally is out of the reconsider range");
}

#[test]
fn a_healthy_pack_casts_no_heal() {
    let (mut world, _db, _l) = support_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    scene(&mut world, Some(ALLY_ID), ALLY, 200);
    // Everyone at full HP: `checkSkillTarget` rejects a heal on an undamaged
    // target, so the ladder should fall through to the buff step instead.
    let cast = crate::game_loop::npc_cast::try_cast(&mut world, HEALER, PLAYER);

    if cast {
        let target = cast_target(&world).expect("a target");
        let skill_id = world.objects.get_component::<Casting>(&HEALER).unwrap().0.skill_id;
        assert_eq!(skill_id, BUFF_SKILL, "a full-HP pack gets the buff, never the heal");
        assert!(target == HEALER || target == ALLY, "buff lands on the pack");
    }
}

#[test]
fn a_buff_goes_to_a_faction_mate_that_lacks_it() {
    let (mut world, _db, _l) = support_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    scene(&mut world, Some(ALLY_ID), ALLY, 200);
    // Give the healer the buff already, so the only valid target is the ally
    // (`checkSkillTarget` refuses a re-cast of a held abnormal).
    world.objects.add_components(
        &HEALER,
        crate::model::components::Buffs(vec![crate::model::skill::ActiveBuff {
            skill_id: BUFF_SKILL,
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

    assert!(crate::game_loop::npc_cast::try_cast(&mut world, HEALER, PLAYER));

    assert_eq!(cast_target(&world), Some(ALLY), "the buff goes to the mate who lacks it");
}

#[test]
fn a_clanless_mob_only_ever_targets_itself() {
    // Most mobs declare no faction at all; they must keep the old behaviour.
    let (mut world, _db, _l) = support_world();
    {
        let mut t = world.data.npc_data.get(HEALER_ID).unwrap().clone();
        t.clans.clear();
        world.data.npc_data.insert_for_test(t);
    }
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    scene(&mut world, Some(ALLY_ID), ALLY, 200);
    wound(&mut world, ALLY, 50.0);
    wound(&mut world, HEALER, 500.0);

    assert!(crate::game_loop::npc_cast::try_cast(&mut world, HEALER, PLAYER));

    assert_eq!(cast_target(&world), Some(HEALER), "no faction → self only");
}
