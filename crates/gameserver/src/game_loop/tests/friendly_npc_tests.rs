//! `FriendlyNpcAI` and the NPC normal follow, on Labyrinth of Belis' real
//! templates: the Infiltration Officer (19155, `FriendlyNpc`) and an
//! Operative (22998, `Monster`).

use super::*;
use crate::game_loop::npc::ai::follow;
use crate::model::components::space::NpcFollow;
use crate::model::npc::{AggroList, NpcAi, NpcIntention};

const OFFICER: i32 = 19155;
const OPERATIVE: i32 = 22998;
const PLAYER: i32 = 7201;
const AT: (i32, i32, i32) = (-111782, 231892, -3178);

fn belis_npcs_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();
    let rx = ingame_player(&mut world, 1, PLAYER, AT.0, AT.1, AT.2);
    (world, rx)
}

fn spawn(world: &mut World, npc_id: i32, dx: i32) -> i32 {
    game_loop::npc::spawn_npc_at(world, npc_id, AT.0 + dx, AT.1, AT.2, 0).expect("spawned")
}

fn distance_2d(world: &World, a: i32, b: i32) -> f64 {
    let pa = world.objects.get_component::<Position>(&a).unwrap();
    let pb = world.objects.get_component::<Position>(&b).unwrap();
    f64::from(pa.x - pb.x).hypot(f64::from(pa.y - pb.y))
}

fn intention(world: &World, oid: i32) -> NpcIntention {
    world
        .objects
        .get_component::<NpcAi>(&oid)
        .unwrap()
        .intention
}

/// `startFollow`: the NPC walks back to within 50–100 of its target every
/// second, and gives up once the target is more than 3000 away.
#[test]
fn a_following_npc_keeps_up_and_gives_up_past_3000() {
    let (mut world, _rx) = belis_npcs_world();
    let officer = spawn(&mut world, OFFICER, 600);
    follow::start_follow(&mut world, officer, PLAYER);
    game_loop::npc::ai::set_running(&mut world, officer);
    advance_world(&mut world, 100);
    assert!(distance_2d(&world, officer, PLAYER) <= 110.0);

    if let Some(p) = world.objects.get_component_mut::<Position>(&PLAYER) {
        p.x += 4000;
    }
    advance_world(&mut world, 10);
    assert!(!world.objects.has_component::<NpcFollow>(&officer));
}

/// `setIntention(MOVE_TO)` stops the follow.
#[test]
fn a_move_to_order_stops_the_follow() {
    let (mut world, _rx) = belis_npcs_world();
    let officer = spawn(&mut world, OFFICER, 600);
    follow::start_follow(&mut world, officer, PLAYER);
    game_loop::npc::ai::set_move_to_intention(&mut world, officer, AT.0, AT.1 + 500, AT.2);
    assert!(!world.objects.has_component::<NpcFollow>(&officer));
    assert_eq!(intention(&world, officer), NpcIntention::MoveTo);
}

/// `FriendlyNpc.addDamage`: a player's blow leaves no damage or hate, and
/// `onActionAttacked` is empty, so the officer does not turn on them.
#[test]
fn a_friendly_npc_ignores_a_players_blow() {
    let (mut world, _rx) = belis_npcs_world();
    let officer = spawn(&mut world, OFFICER, 100);
    game_loop::combat::npc_receive_damage(
        &mut world,
        officer,
        PLAYER,
        10.0,
        game_loop::combat::DamageHit::default(),
    );
    let aggro = world.objects.get_component::<AggroList>(&officer).unwrap();
    assert!(aggro.0.is_empty());
    assert_eq!(intention(&world, officer), NpcIntention::Active);
}

/// A monster's blow is hated but not answered at once; once the spawn calm
/// (`_globalAggro`) runs out, `thinkActive` takes the most hated as the
/// attack target, and `thinkAttack` swings at it.
#[test]
fn a_friendly_npc_attacks_the_monster_that_hit_it_once_calm_ends() {
    let (mut world, _rx) = belis_npcs_world();
    let officer = spawn(&mut world, OFFICER, 1500);
    let operative = spawn(&mut world, OPERATIVE, 1560);
    game_loop::combat::npc_receive_damage(
        &mut world,
        officer,
        operative,
        1.0,
        game_loop::combat::DamageHit::default(),
    );
    assert_eq!(
        intention(&world, officer),
        NpcIntention::Active,
        "no fight-back reflex"
    );
    let full = hp_of(&world, operative);
    advance_world(&mut world, 90);
    assert_eq!(
        intention(&world, officer),
        NpcIntention::Active,
        "still calm"
    );
    advance_world(&mut world, 10);
    let ai = world.objects.get_component::<NpcAi>(&officer).unwrap();
    assert_eq!((ai.intention, ai.target), (NpcIntention::Attack, operative));
    advance_world(&mut world, 100);
    assert!(hp_of(&world, operative) < full, "the officer swung");
}

/// `thinkAttack` with a dead target: stop hating it and go back to ACTIVE.
#[test]
fn a_friendly_npc_drops_a_dead_target() {
    let (mut world, _rx) = belis_npcs_world();
    let officer = spawn(&mut world, OFFICER, 1500);
    let operative = spawn(&mut world, OPERATIVE, 1560);
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&officer) {
        ai.global_aggro = 0;
    }
    game_loop::combat::npc_receive_damage(
        &mut world,
        officer,
        operative,
        1.0,
        game_loop::combat::DamageHit::default(),
    );
    advance_world(&mut world, 10);
    assert_eq!(intention(&world, officer), NpcIntention::Attack);
    game_loop::npc::npc_do_die(&mut world, operative, PLAYER);
    advance_world(&mut world, 10);
    assert_eq!(intention(&world, officer), NpcIntention::Active);
}

/// A monster that hates a `FriendlyNpc` attacks it like any other target.
#[test]
fn a_monster_attacks_a_hated_friendly_npc() {
    let (mut world, _rx) = belis_npcs_world();
    let officer = spawn(&mut world, OFFICER, 1500);
    let operative = spawn(&mut world, OPERATIVE, 1560);
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&operative) {
        aggro.0.entry(officer).or_default().hate = 9999.0;
    }
    game_loop::npc::ai::set_attack_intention(&mut world, operative);
    let full = hp_of(&world, officer);
    advance_world(&mut world, 100);
    assert!(hp_of(&world, officer) < full);
}
