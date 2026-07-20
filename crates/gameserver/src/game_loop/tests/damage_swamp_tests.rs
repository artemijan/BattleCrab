//! `DamageZone` and `SwampZone` (G21 slice 9) — the last two zone types with
//! live content on this dist.

use super::*;

use crate::data::zone_data::{DamageZoneParams, SwampZoneParams, Zone, ZoneKind};
use crate::model::components::{Speeds, Vitals};

const PLAYER: i32 = 2001;
const CID: u32 = 1;
const CASTLE: i32 = 5;

fn cuboid() -> crate::data::spawn_data::Territory {
    crate::data::spawn_data::Territory {
        form: crate::data::spawn_data::ZoneForm::Cuboid { x1: -500, x2: 500, y1: -500, y2: 500 },
        min_z: -1000,
        max_z: 1000,
    }
}

fn damage_params(castle_id: i32) -> DamageZoneParams {
    DamageZoneParams {
        // Java's default, and the value every zone on this dist uses — none
        // override `damageHPPerSec`.
        hp_per_tick: 200,
        mp_per_tick: 0,
        initial_delay: 1000,
        reuse: 5000,
        enabled: true,
        castle_id,
    }
}

fn insert_damage_zone(world: &mut World, p: DamageZoneParams) {
    world.data.zone_data.insert(Zone {
        name: "test_damage".into(),
        kind: ZoneKind::Damage,
        territory: cuboid(),
        castle_id: p.castle_id,
        effect: None,
        damage: Some(p),
        swamp: None,
    });
}

fn insert_swamp_zone(world: &mut World, p: SwampZoneParams) {
    world.data.zone_data.insert(Zone {
        name: "test_swamp".into(),
        kind: ZoneKind::Swamp,
        territory: cuboid(),
        castle_id: p.castle_id,
        effect: None,
        damage: None,
        swamp: Some(p),
    });
}

fn sweep(world: &mut World, n: u64) {
    for _ in 0..n {
        world.tick += crate::game_loop::effect_zones::SWEEP_PERIOD;
        crate::game_loop::effect_zones::damage_zone_tick(world);
    }
}

fn hp(world: &World) -> f64 {
    world.objects.get_component::<Vitals>(&PLAYER).unwrap().cur_hp
}

fn run_speed(world: &World) -> f64 {
    world.objects.get_component::<Speeds>(&PLAYER).unwrap().run_spd
}

// ---------------------------------------------------------------------------
// DamageZone.

#[test]
fn standing_in_a_damage_zone_hurts() {
    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    insert_damage_zone(&mut world, damage_params(0));
    let before = hp(&world);

    sweep(&mut world, 20);

    assert!(hp(&world) < before, "lava should burn ({before} → {})", hp(&world));
}

#[test]
fn outside_the_zone_is_safe() {
    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 5000, 5000);
    insert_damage_zone(&mut world, damage_params(0));
    let before = hp(&world);

    sweep(&mut world, 20);

    assert_eq!(hp(&world), before);
}

#[test]
fn a_disabled_damage_zone_does_nothing() {
    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let mut p = damage_params(0);
    p.enabled = false; // 22 of the dist's 35 ship like this
    insert_damage_zone(&mut world, p);
    let before = hp(&world);

    sweep(&mut world, 20);

    assert_eq!(hp(&world), before);
}

#[test]
fn a_castle_trap_is_inert_outside_a_siege() {
    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    insert_damage_zone(&mut world, damage_params(CASTLE));
    let before = hp(&world);

    sweep(&mut world, 20);

    assert_eq!(hp(&world), before, "no siege, no trap");
}

#[test]
fn a_castle_trap_bites_during_a_siege() {
    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    insert_damage_zone(&mut world, damage_params(CASTLE));
    world.sieges.insert(CASTLE, crate::model::siege::Siege::new(CASTLE));
    let before = hp(&world);

    sweep(&mut world, 20);

    assert!(hp(&world) < before, "the trap is armed while the siege runs");
}

#[test]
fn a_defender_is_spared_by_its_own_castles_trap() {
    // Otherwise the garrison would cook itself on its own defences.
    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    insert_damage_zone(&mut world, damage_params(CASTLE));
    world.sieges.insert(CASTLE, crate::model::siege::Siege::new(CASTLE));
    // Put the player in a clan that owns the castle.
    let clan_id = 700;
    world.objects.get_component_mut::<crate::model::Player>(&PLAYER).unwrap().clan_id = clan_id;
    world.clans.insert(
        clan_id,
        crate::model::clan::Clan {
            id: clan_id,
            name: "Defenders".into(),
            leader_id: PLAYER,
            level: 5,
            reputation_score: 0,
            castle_id: CASTLE,
            members: Vec::new(),
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
        },
    );
    let before = hp(&world);

    sweep(&mut world, 20);

    assert_eq!(hp(&world), before, "the castle's own defenders are immune");
}

// ---------------------------------------------------------------------------
// SwampZone.

#[test]
fn a_swamp_slows_the_player_down() {
    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 5000, 5000);
    let normal = run_speed(&world);
    insert_swamp_zone(
        &mut world,
        SwampZoneParams { move_bonus: 0.2, enabled: true, castle_id: 0 },
    );

    // Walk into it and revalidate.
    {
        let pos = world.objects.get_component_mut::<crate::model::components::Position>(&PLAYER).unwrap();
        pos.x = 0;
        pos.y = 0;
    }
    crate::game_loop::zones::revalidate_zone(&mut world, PLAYER, true);

    let slowed = run_speed(&world);
    assert!(slowed < normal, "0.2 move_bonus should slow the player ({normal} → {slowed})");
    assert!((slowed - normal * 0.2).abs() < 1.0, "and by the declared factor");
}

#[test]
fn leaving_the_swamp_restores_full_speed() {
    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 5000, 5000);
    let normal = run_speed(&world);
    insert_swamp_zone(
        &mut world,
        SwampZoneParams { move_bonus: 0.2, enabled: true, castle_id: 0 },
    );
    {
        let pos = world.objects.get_component_mut::<crate::model::components::Position>(&PLAYER).unwrap();
        pos.x = 0;
        pos.y = 0;
    }
    crate::game_loop::zones::revalidate_zone(&mut world, PLAYER, true);
    assert!(run_speed(&world) < normal);

    {
        let pos = world.objects.get_component_mut::<crate::model::components::Position>(&PLAYER).unwrap();
        pos.x = 5000;
        pos.y = 5000;
    }
    crate::game_loop::zones::revalidate_zone(&mut world, PLAYER, true);

    assert_eq!(run_speed(&world), normal, "speed comes back on exit");
}

#[test]
fn a_castle_swamp_is_inert_outside_a_siege() {
    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 5000, 5000);
    let normal = run_speed(&world);
    insert_swamp_zone(
        &mut world,
        SwampZoneParams { move_bonus: 0.2, enabled: true, castle_id: CASTLE },
    );
    {
        let pos = world.objects.get_component_mut::<crate::model::components::Position>(&PLAYER).unwrap();
        pos.x = 0;
        pos.y = 0;
    }
    crate::game_loop::zones::revalidate_zone(&mut world, PLAYER, true);

    assert_eq!(run_speed(&world), normal, "18 of the 20 swamps are siege-gated castle traps");
}
