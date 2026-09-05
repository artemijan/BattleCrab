//! Crowd control (G19): the abnormal-state flags that make stun/sleep/paralyze
//! and root actually do something.

mod block_actions;
mod conditional;
mod gates;
mod lifecycle;
mod mastery;
mod pvp;
mod sleep;
mod targeting;
mod travel;
mod triggers;
mod unlock;
mod visuals;
mod vitals;

use super::*;
use crate::game_loop::abnormal;
use crate::game_loop::abnormal::has_buff;
use crate::game_loop::helpers::stat_add;
use crate::game_loop::skills::skill_by_id;
use crate::game_loop::space::position::pos_of;
use crate::model::components::combat::Casting;
use crate::model::components::skills::Buffs;
use crate::model::components::space::Movement;
use crate::model::skill::effects::SkillEffect;
use crate::model::skill::target::{AffectObject, AffectScope, OperateType, TargetType};
use crate::model::skill::{Skill, effect_flag};

const CASTER: i32 = 2001;

const VICTIM: i32 = 2002;

const CID: u32 = 1;

const VICTIM_CID: u32 = 2;

const STUN_ID: i32 = 9300;

const ROOT_ID: i32 = 9301;

/// A CC skill shaped like the real ones: no stat modifier, the mechanic is
/// entirely the state flag.
fn cc_skill(id: i32, effect: SkillEffect, abnormal: &str) -> Skill {
    Skill {
        self_continuous: false,
        without_action: false,
        trait_type: model::skill::traits::TraitType::None,
        item_consume_id: 0,
        item_consume_count: 0,
        id,
        level: 1,
        name: format!("CC {id}"),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Enemy,
        magic_type: 1,
        magic_level: 0,
        effect_point: -100,
        cast_range: 900,
        effect_range: 1000,
        hit_time: 100,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 9,
        abnormal_level: 1,
        abnormal_type: abnormal.into(),
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
        is_debuff: true,
        stay_after_death: false,
        effects: vec![effect],
        ..Default::default()
    }
}

/// Land a CC skill straight onto `target`, bypassing the cast pipeline (which
/// the affect/cast tests already cover) so these cases isolate the state.
fn land(world: &mut World, skill_id: i32, target: i32) {
    let skill = skill_by_id(world, skill_id, 1).expect("registered");
    effects::apply_skill_effects(world, CASTER, target, &skill);
}

fn cc_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = cast_test_world();
    world.data.skill_data.insert_for_test(cc_skill(
        STUN_ID,
        SkillEffect::BlockActions { conditional: false },
        "STUN",
    ));
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(ROOT_ID, SkillEffect::Root, "ROOT_PHYSICALLY"));
    (world, db, l)
}

// ---------------------------------------------------------------------------
// The flags themselves
// ---------------------------------------------------------------------------

const MUTE_ID: i32 = 9310;

const PMUTE_ID: i32 = 9311;

const DBLOCK_ID: i32 = 9312;

const CBLOCK_ID: i32 = 9313;

const TCANCEL_ID: i32 = 9314;

fn cc2_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    // Builds on `cc_world` so the stun/root fixtures are available too — the
    // debuff-block case needs a real debuff to refuse.
    let (mut world, db, l) = cc_world();
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(MUTE_ID, SkillEffect::Mute, "MUTE"));
    world.data.skill_data.insert_for_test(cc_skill(
        PMUTE_ID,
        SkillEffect::PhysicalMute,
        "PHYSICAL_MUTE",
    ));
    world.data.skill_data.insert_for_test(cc_skill(
        DBLOCK_ID,
        SkillEffect::DebuffBlock,
        "DEBUFF_BLOCK",
    ));
    world.data.skill_data.insert_for_test(cc_skill(
        CBLOCK_ID,
        SkillEffect::BlockControl,
        "BLOCK_CONTROL",
    ));
    world.data.skill_data.insert_for_test(cc_skill(
        TCANCEL_ID,
        SkillEffect::TargetCancel { chance: 100 },
        "NONE",
    ));
    (world, db, l)
}

const SLEEP_ID: i32 = 9310;
