//! `EffectZone` (G21 slice 5): zones that periodically cast on the players
//! standing in them.

use super::*;

use crate::data::zone_data::{EffectZoneParams, Zone, ZoneKind};
use crate::model::components::Vitals;
use crate::model::skill::{AffectObject, AffectScope, OperateType, Skill, SkillEffect, TargetType};

const PLAYER: i32 = 2001;
const CID: u32 = 1;
const BURN_ID: i32 = 8500;
const BUFF_ID: i32 = 8501;

fn zone_skill(id: i32, effects: Vec<SkillEffect>, abnormal: &str) -> Skill {
    Skill {
        without_action: false,
        trait_type: crate::model::skill::TraitType::None,
        item_consume_id: 0,
        item_consume_count: 0,
        id,
        level: 1,
        name: format!("Zone {id}"),
        operate_type: OperateType::Active,
        is_continuous: !abnormal.is_empty(),
        target_type: TargetType::Self_,
        magic_type: 1,
        magic_level: 1,
        effect_point: -100,
        cast_range: -1,
        effect_range: -1,
        hit_time: 0,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: if abnormal.is_empty() { 0 } else { 100 },
        abnormal_level: 1,
        abnormal_type: if abnormal.is_empty() {
            "NONE".into()
        } else {
            abnormal.into()
        },
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
        is_debuff: !abnormal.is_empty(),
        stay_after_death: false,
        effects,
        ..Default::default()
    }
}

fn params(skills: Vec<(i32, i32)>) -> EffectZoneParams {
    EffectZoneParams {
        skills,
        chance: 100,
        initial_delay: 0,
        // 6000 ms is the shortest reuse in the datapack.
        reuse: 6000,
        enabled: true,
        casts_on_players: true,
        remove_effects_on_exit: false,
    }
}

fn insert_effect_zone(world: &mut World, p: EffectZoneParams) {
    world.data.zone_data.insert(Zone {
        id: 0,
        name: "test_effect_zone".into(),
        kind: ZoneKind::Effect,
        territory: crate::data::spawn_data::Territory {
            form: crate::data::spawn_data::ZoneForm::Cuboid {
                x1: -500,
                x2: 500,
                y1: -500,
                y2: 500,
            },
            min_z: -1000,
            max_z: 1000,
        },
        castle_id: 0,
        clan_hall_id: 0,
        effect: Some(p),
        damage: None,
        swamp: None,
        condition: None,
    });
}

fn zone_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    // A flat 50-damage burn, and a stat buff with an abnormal type.
    world.data.skill_data.insert_for_test(zone_skill(
        BURN_ID,
        vec![SkillEffect::MagicalAttack { power: 50.0 }],
        "",
    ));
    world.data.skill_data.insert_for_test(zone_skill(
        BUFF_ID,
        vec![SkillEffect::StatModifier(
            crate::model::skill::StatModifierEffect {
                stat: crate::model::stats::Stat::PhysicalAttack,
                mode: crate::model::stats::StatModifierType::Per,
                amount: 1.2,
                armor_condition: 0,
                weapon_condition: 0,
                qualifier: None,
                two_handed: false,
            },
        )],
        "MIGHT",
    ));
    (world, db, l)
}

/// Run the effect-zone sweep `n` times at its real period.
fn sweep(world: &mut World, n: u64) {
    for _ in 0..n {
        world.tick += crate::game_loop::effect_zones::SWEEP_PERIOD;
        crate::game_loop::effect_zones::effect_zone_tick(world);
    }
}

fn hp(world: &World) -> f64 {
    world
        .objects
        .get_component::<Vitals>(&PLAYER)
        .unwrap()
        .cur_hp
}

// ---------------------------------------------------------------------------

#[test]
fn a_player_standing_in_a_damage_zone_takes_damage() {
    let (mut world, _db, _l) = zone_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    insert_effect_zone(&mut world, params(vec![(BURN_ID, 1)]));
    let before = hp(&world);

    sweep(&mut world, 20); // 20 s — several 6 s reuses

    assert!(
        hp(&world) < before,
        "the zone should have burned the player ({before} → {})",
        hp(&world)
    );
}

#[test]
fn a_player_outside_the_zone_is_untouched() {
    let (mut world, _db, _l) = zone_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 5000, 5000);
    insert_effect_zone(&mut world, params(vec![(BURN_ID, 1)]));
    let before = hp(&world);

    sweep(&mut world, 20);

    assert_eq!(
        hp(&world),
        before,
        "standing outside the cuboid must be safe"
    );
}

#[test]
fn the_zone_fires_on_its_own_reuse_not_every_sweep() {
    // The sweep runs every second; a 6 s reuse must not mean six casts.
    let (mut world, _db, _l) = zone_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    insert_effect_zone(&mut world, params(vec![(BURN_ID, 1)]));
    let full = hp(&world);

    sweep(&mut world, 7); // ~7 s → one reuse elapsed
    let after_one = full - hp(&world);
    assert!(after_one > 0.0, "at least one tick landed");

    sweep(&mut world, 7); // ~14 s total
    let after_two = full - hp(&world);

    assert!(
        after_two < after_one * 3.0,
        "damage should scale with the 6 s reuse, not the 1 s sweep ({after_one} then {after_two} total)"
    );
}

#[test]
fn a_disabled_zone_does_nothing() {
    let (mut world, _db, _l) = zone_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let mut p = params(vec![(BURN_ID, 1)]);
    p.enabled = false; // `default_enabled="false"`, e.g. the siege traps
    insert_effect_zone(&mut world, p);
    let before = hp(&world);

    sweep(&mut world, 20);

    assert_eq!(
        hp(&world),
        before,
        "a disabled zone waits for a script to enable it"
    );
}

#[test]
fn an_npc_targeted_zone_casts_on_nobody() {
    // 27 zones on this dist declare `targetClass="Npc"`. Java tracks only NPCs
    // as inside, then the tick requires `isPlayer()` — so they reach no one.
    let (mut world, _db, _l) = zone_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let mut p = params(vec![(BURN_ID, 1)]);
    p.casts_on_players = false;
    insert_effect_zone(&mut world, p);
    let before = hp(&world);

    sweep(&mut world, 20);

    assert_eq!(
        hp(&world),
        before,
        "targetClass=Npc zones are inert for players"
    );
}

#[test]
fn zero_chance_never_fires() {
    let (mut world, _db, _l) = zone_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let mut p = params(vec![(BURN_ID, 1)]);
    p.chance = 0;
    insert_effect_zone(&mut world, p);
    let before = hp(&world);

    sweep(&mut world, 30);

    assert_eq!(hp(&world), before, "chance 0 means never");
}

#[test]
fn a_buff_zone_grants_its_buff_once_not_repeatedly() {
    // The Hot Springs trio: without Java's `getAffectedSkillLevel < level`
    // guard the zone would re-cast every reuse forever.
    let (mut world, _db, _l) = zone_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    insert_effect_zone(&mut world, params(vec![(BUFF_ID, 1)]));

    sweep(&mut world, 30);

    let buffs = world
        .objects
        .get_component::<crate::model::components::Buffs>(&PLAYER)
        .unwrap();
    let count = buffs.0.iter().filter(|b| b.skill_id == BUFF_ID).count();
    assert_eq!(count, 1, "the zone buff should be held once, not stacked");
}

#[test]
fn a_dead_player_is_skipped() {
    let (mut world, _db, _l) = zone_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    insert_effect_zone(&mut world, params(vec![(BURN_ID, 1)]));
    {
        let v = world.objects.get_component_mut::<Vitals>(&PLAYER).unwrap();
        v.cur_hp = 1.0;
        v.dead = true;
    }

    sweep(&mut world, 20);

    assert_eq!(hp(&world), 1.0, "a corpse doesn't keep burning");
}

#[test]
fn multiple_skills_all_land_in_one_tick() {
    let (mut world, _db, _l) = zone_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    insert_effect_zone(&mut world, params(vec![(BURN_ID, 1), (BUFF_ID, 1)]));
    let before = hp(&world);

    sweep(&mut world, 20);

    assert!(hp(&world) < before, "the damage skill landed");
    let buffs = world
        .objects
        .get_component::<crate::model::components::Buffs>(&PLAYER)
        .unwrap();
    assert!(
        buffs.0.iter().any(|b| b.skill_id == BUFF_ID),
        "and the buff skill landed too"
    );
}
