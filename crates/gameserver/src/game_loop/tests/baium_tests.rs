//! Baium — archangels and the strider debuff.

use super::*;

use crate::game_loop::baium::{ARCHANGEL, BAIUM};

const BAIUM_OID: i32 = NPC_OID + 110;
const PLAYER: i32 = 9995;
const CID: u32 = 1;
const ANTI_STRIDER: i32 = 4258;
const MOUNT_STRIDER: u8 = 1;

fn baium_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    for (id, kind) in [(BAIUM, "GrandBoss"), (ARCHANGEL, "Monster")] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = kind.into();
        t.level = 75;
        t.base_hp_max = 100_000.0;
        t.base_mp_max = 10_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    world.data.skill_data.insert_for_test(crate::model::skill::Skill {
        id: ANTI_STRIDER,
        level: 1,
        abnormal_time: 60,
        effects: vec![crate::model::skill::SkillEffect::StatModifier(crate::model::skill::StatModifierEffect {
            stat: crate::model::stats::Stat::RunSpeed,
            mode: crate::model::stats::StatModifierType::Diff,
            amount: -50.0,
            ..Default::default()
        })],
        ..Default::default()
    });
    (world, db, l)
}

fn count(world: &mut World, npc_id: i32) -> usize {
    let mut n = 0;
    world.objects.for_each_mut::<&crate::model::npc::Npc>(|x| {
        if x.npc_id == npc_id {
            n += 1;
        }
    });
    n
}

fn has_debuff(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::Buffs>(&oid)
        .is_some_and(|b| b.0.iter().any(|x| x.skill_id == ANTI_STRIDER))
}

/// Baium brings out five archangels. They are **not** in a minion table, so
/// nothing but the script would place them.
#[test]
fn baium_spawns_five_archangels() {
    let (mut world, _db, _l) = baium_world();
    crate::game_loop::baium::on_baium_spawned(&mut world);
    assert_eq!(count(&mut world, ARCHANGEL), 5);
}

/// A strider-mounted attacker is hindered.
#[test]
fn a_strider_rider_is_hindered() {
    let (mut world, _db, _l) = baium_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 20, 0, 0);
    {
        let v = world.objects.get_component_mut::<Vitals>(&BAIUM_OID).unwrap();
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
    }
    world.objects.get_component_mut::<crate::model::Player>(&PLAYER).unwrap().mount_type = MOUNT_STRIDER;

    crate::game_loop::baium::on_baium_attacked(&mut world, BAIUM_OID, PLAYER);
    for _ in 0..60 {
        advance_ticks(&mut world, 1);
    }
    assert!(has_debuff(&world, PLAYER), "the rider was hindered");
}

/// An unmounted attacker is left alone — the debuff is aimed at striders
/// specifically, not at everyone.
#[test]
fn an_unmounted_attacker_is_not_hindered() {
    let (mut world, _db, _l) = baium_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 20, 0, 0);

    crate::game_loop::baium::on_baium_attacked(&mut world, BAIUM_OID, PLAYER);
    for _ in 0..60 {
        advance_ticks(&mut world, 1);
    }
    assert!(!has_debuff(&world, PLAYER), "on foot, no debuff");
}

/// The debuff is cast **once**, not on every swing — Java guards on
/// `!isAffectedBySkill(4258)`.
#[test]
fn the_strider_debuff_is_not_recast_while_it_holds() {
    let (mut world, _db, _l) = baium_world();
    let mut rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 20, 0, 0);
    {
        let v = world.objects.get_component_mut::<Vitals>(&BAIUM_OID).unwrap();
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
    }
    world.objects.get_component_mut::<crate::model::Player>(&PLAYER).unwrap().mount_type = MOUNT_STRIDER;

    crate::game_loop::baium::on_baium_attacked(&mut world, BAIUM_OID, PLAYER);
    for _ in 0..60 {
        advance_ticks(&mut world, 1);
    }
    assert!(has_debuff(&world, PLAYER));
    while rx.try_recv().is_ok() {}

    // A second hit while it still holds must start no new cast.
    crate::game_loop::baium::on_baium_attacked(&mut world, BAIUM_OID, PLAYER);
    let mut casts = 0;
    while let Ok(p) = rx.try_recv() {
        if p.first() == Some(&0x48) {
            // MagicSkillUse
            casts += 1;
        }
    }
    assert_eq!(casts, 0, "already hindered, nothing recast");
}
