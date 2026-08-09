//! Buffs and death (`Playable.doDie`): dying strips the buff list, unless
//! Noblesse Blessing is up — then the blessing is the only thing lost.

use super::*;
use crate::game_loop::helpers::skill_by_id;

use crate::game_loop::abnormal;
use crate::model::components::{Buffs, Vitals};
use crate::model::skill::{
    AffectObject, AffectScope, OperateType, Skill, SkillEffect, TargetType, effect_flag,
};
use crate::model::stats::{Stat, StatModifierType};

const VICTIM: i32 = 2001;
const KILLER: i32 = 2002;
const VICTIM_CID: u32 = 1;
const KILLER_CID: u32 = 2;

/// Might-like buff (a real stat pump, so its removal is observable in the
/// modifier maps too).
const MIGHT_ID: i32 = 1068;
/// Noblesse-Blessing-like: no stat modifier, the mechanic is the flag.
const BLESS_ID: i32 = 9401;
/// A `<stayAfterDeath>` buff — survives an ordinary death.
const LASTING_ID: i32 = 9402;

fn buff_skill(id: i32, effects: Vec<SkillEffect>, stay_after_death: bool) -> Skill {
    Skill {
        self_continuous: false,
        without_action: false,
        trait_type: crate::model::skill::TraitType::None,
        item_consume_id: 0,
        item_consume_count: 0,
        id,
        level: 1,
        name: format!("Buff {id}"),
        operate_type: OperateType::Active,
        is_continuous: true,
        target_type: TargetType::Target,
        magic_type: 1,
        magic_level: 0,
        effect_point: 1,
        cast_range: 400,
        effect_range: 900,
        hit_time: 100,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 3600,
        abnormal_level: 1,
        abnormal_type: format!("TYPE_{id}"),
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
        stay_after_death,
        effects,
        ..Default::default()
    }
}

fn death_buff_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = cast_test_world();
    let pump = SkillEffect::StatModifier(crate::model::skill::StatModifierEffect {
        stat: Stat::PhysicalAttack,
        mode: StatModifierType::Per,
        amount: 8.0,
        armor_condition: 0,
        weapon_condition: 0,
        qualifier: None,
        two_handed: false,
    });
    world
        .data
        .skill_data
        .insert_for_test(buff_skill(MIGHT_ID, vec![pump.clone()], false));
    world.data.skill_data.insert_for_test(buff_skill(
        BLESS_ID,
        vec![SkillEffect::NoblesseBless],
        false,
    ));
    world
        .data
        .skill_data
        .insert_for_test(buff_skill(LASTING_ID, vec![pump], true));
    (world, db, l)
}

/// Land a buff straight onto the target, bypassing the cast pipeline.
fn land(world: &mut World, skill_id: i32, target: i32) {
    let skill = skill_by_id(world, skill_id, 1).expect("registered");
    crate::game_loop::skills::effects::apply_skill_effects(world, target, target, &skill);
}

fn live_buffs(world: &World, oid: i32) -> Vec<i32> {
    world
        .objects
        .get_component::<Buffs>(&oid)
        .map(|b| {
            b.0.iter()
                .filter(|x| !x.passive)
                .map(|x| x.skill_id)
                .collect()
        })
        .unwrap_or_default()
}

fn setup(world: &mut World) {
    let _v = ingame_caster(world, VICTIM_CID, VICTIM, 0, 0);
    let _k = ingame_caster(world, KILLER_CID, KILLER, 50, 0);
}

// ---------------------------------------------------------------------------

/// The plain case: death takes every buff with it.
#[test]
fn death_without_noblesse_strips_all_buffs() {
    let (mut world, _db, _l) = death_buff_world();
    setup(&mut world);

    land(&mut world, MIGHT_ID, VICTIM);
    assert_eq!(
        live_buffs(&world, VICTIM),
        vec![MIGHT_ID],
        "the buff landed"
    );

    crate::game_loop::death::player_do_die(&mut world, VICTIM, KILLER);

    assert!(
        world
            .objects
            .get_component::<Vitals>(&VICTIM)
            .is_some_and(|v| v.dead),
        "the player died"
    );
    assert!(
        live_buffs(&world, VICTIM).is_empty(),
        "death stripped the buff list"
    );
}

/// Noblesse Blessing up: every other buff rides through death, and the
/// blessing itself is the one casualty (`Playable.doDie`).
#[test]
fn noblesse_blessing_keeps_buffs_and_consumes_itself() {
    let (mut world, _db, _l) = death_buff_world();
    setup(&mut world);

    land(&mut world, MIGHT_ID, VICTIM);
    land(&mut world, BLESS_ID, VICTIM);
    assert!(
        abnormal::flags_of(&world, VICTIM) & effect_flag::NOBLESS_BLESSING != 0,
        "the blessing landed as a real buff and carries its flag"
    );

    crate::game_loop::death::player_do_die(&mut world, VICTIM, KILLER);

    assert_eq!(
        live_buffs(&world, VICTIM),
        vec![MIGHT_ID],
        "only the blessing was removed"
    );
    assert_eq!(
        abnormal::flags_of(&world, VICTIM),
        0,
        "and its flag went with it"
    );
}

/// A second death with no blessing left now clears the survivors — the
/// blessing is single-use, not a standing exemption.
#[test]
fn blessing_does_not_survive_to_a_second_death() {
    let (mut world, _db, _l) = death_buff_world();
    setup(&mut world);

    land(&mut world, MIGHT_ID, VICTIM);
    land(&mut world, BLESS_ID, VICTIM);
    crate::game_loop::death::player_do_die(&mut world, VICTIM, KILLER);
    // Revive so the second `doDie` isn't the already-dead no-op.
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&VICTIM) {
        v.dead = false;
        v.cur_hp = 1.0;
    }

    crate::game_loop::death::player_do_die(&mut world, VICTIM, KILLER);

    assert!(
        live_buffs(&world, VICTIM).is_empty(),
        "the unblessed death strips what the blessed one spared"
    );
}

/// `<stayAfterDeath>` buffs are the other exemption, and they don't need the
/// blessing (`stopAllEffectsExceptThoseThatLastThroughDeath`).
#[test]
fn stay_after_death_buffs_survive_an_unblessed_death() {
    let (mut world, _db, _l) = death_buff_world();
    setup(&mut world);

    land(&mut world, MIGHT_ID, VICTIM);
    land(&mut world, LASTING_ID, VICTIM);

    crate::game_loop::death::player_do_die(&mut world, VICTIM, KILLER);

    assert_eq!(
        live_buffs(&world, VICTIM),
        vec![LASTING_ID],
        "only the ordinary buff was stripped"
    );
}
