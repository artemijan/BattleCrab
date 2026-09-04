//! Skill tests, split to mirror the `game_loop::skills` modules; the helpers
//! more than one area needs live here.

mod buffs;
mod cast;
mod charges;
mod damage;
mod debuffs;
mod dispel;
mod hate;
mod learning;
mod npc;
mod passives;
mod reuse;
mod targeting;

use super::*;
use crate::game_loop;
use crate::game_loop::abnormal::has_buff;
use crate::game_loop::combat::death;
use crate::game_loop::skills::expertise;
use crate::game_loop::skills::skill_by_id;
use crate::game_loop::stats::passive_skills;
use commons::system_messages::generated::{
    C1_HAS_RESISTED_S2_CHANCE_WAS_S3, S1_LANDED_ON_C2_CHANCE_WAS_S3,
};

/// Spawn the level-5 test mob (40001) targeted for a debuff cast and drain the
/// spawn/target chatter, returning its object id.
fn spawn_debuff_target(world: &mut World, a_rx: &mut UnboundedReceiver<bytes::Bytes>) -> i32 {
    let npc_oid = NPC_OID + 14;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    drain(a_rx);
    npc_oid
}

/// A synthetic self-buff with a `PhysicalDefence +8%` modifier so it lands (a
/// non-empty effect list), tagged with the given abnormal type/level and
/// magic type (3 = dance/song).
fn synthetic_buff(
    id: i32,
    level: i32,
    abnormal_type: &str,
    abnormal_level: i32,
    magic_type: i32,
) -> Skill {
    use model::skill::Skill;
    use model::skill::effects::{SkillEffect, StatModifierEffect};
    use model::stats::{Stat, StatModifierType};
    Skill {
        self_continuous: false,
        basic_property: model::skill::BasicProperty::None,
        conditions: Vec::new(),
        target_conditions: Vec::new(),
        passive_conditions: Vec::new(),
        without_action: false,
        is_suicide_attack: false,
        icon: String::from("icon.skill0000"),
        trait_type: model::skill::traits::TraitType::None,
        static_reuse: false,
        item_consume_id: 0,
        item_consume_count: 0,
        id,
        level,
        name: format!("Buff{id}"),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Self_,
        magic_type,
        magic_level: 0,
        activate_rate: -1,
        lvl_bonus_rate: 0,
        effect_point: 100, // >= 0 → not a debuff
        cast_range: 0,
        effect_range: 0,
        hit_time: 0,
        next_action: Default::default(),
        abnormal_resists: Vec::new(),
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 100,
        abnormal_level,
        abnormal_type: abnormal_type.into(),
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        fan_range: [0; 4],
        attribute_type: None,
        sub_level: 0,
        attribute_value: 0,
        end_effects: Vec::new(),
        channeling_effects: Vec::new(),
        mp_per_channeling: 0,
        channeling_skill_id: 0,
        channeling_tick_ms: 0,
        channeling_start_ms: 0,
        can_be_dispelled: true,
        is_debuff: false,
        excluded_from_check: false,
        shared_with_summon: true,
        stay_after_death: false,
        removed_on_damage: false,
        self_effects: Vec::new(),
        pve_effects: Vec::new(),
        pvp_effects: Vec::new(),
        effects: vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::PhysicalDefence,
            mode: StatModifierType::Per,
            amount: 8.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
            hp_percent: 0,
        })],
    }
}

/// Arm a test player with `item_id` in the right hand.
///
/// **G34 S1 made this necessary.** Every warrior/dagger skill in the dist
/// carries an `<condition name="EquipWeapon">`, which Java enforces and this
/// port ignored until the condition engine landed — so these fixtures used to
/// cast Sonic Blaster and Lethal Blow bare-handed. Java refuses that, and now
/// so do we; the fixture has to hold the weapon the skill demands.
fn arm(world: &mut World, object_id: i32, item_id: i32) {
    // `world.data` is borrowed immutably while the inventory is borrowed
    // mutably, so the catalog has to come out of the ECS first.
    let mut inv = world
        .objects
        .get_component::<Inventory>(&object_id)
        .expect("test player has an inventory")
        .clone();
    let oid = inv.add_item(&world.data.item_data, 0x5000_0001, item_id, 1);
    inv.equip_item(&world.data.item_data, oid);
    world.objects.add_components(&object_id, inv);
}
