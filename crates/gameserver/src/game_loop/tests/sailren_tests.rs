//! Sailren — the dinosaur wave ladder.

use super::*;
use crate::game_loop::sailren;

use crate::model::components::{AdminFlags, Immobilized, Position, SailrenWaveMob, Vitals};

const SAILREN: i32 = 29065;
const VELOCIRAPTOR: i32 = 22218;
const PTEROSAUR: i32 = 22199;
const TREX: i32 = 22217;
const CUBIC: i32 = 32107;
/// A stand-in killer id — only used as an aggro key.
const KILLER: i32 = 500;

fn sailren_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    for id in [SAILREN, VELOCIRAPTOR, PTEROSAUR, TREX, CUBIC] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 80;
        t.base_hp_max = 10_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    (world, db, l)
}

/// The object ids of tagged wave mobs of `npc_id`.
fn tagged(world: &mut World, npc_id: i32) -> Vec<i32> {
    let mut v = Vec::new();
    world
        .objects
        .for_each_mut::<(&model::npc::Npc, &SailrenWaveMob)>(|(n, _)| {
            if n.npc_id == npc_id {
                v.push(n.object_id);
            }
        });
    v
}

fn kill(world: &mut World, oid: i32) {
    world
        .objects
        .get_component_mut::<Vitals>(&oid)
        .unwrap()
        .dead = true;
}

#[test]
fn the_wave_climbs_from_raptors_up_to_sailren() {
    let (mut world, _db, _l) = sailren_world();
    sailren::begin_fight(&mut world);
    let raptors = tagged(&mut world, VELOCIRAPTOR);
    assert_eq!(raptors.len(), 3, "three velociraptors enter");

    // The first two deaths leave raptors standing → no Pterosaur yet.
    for &r in &raptors[..2] {
        kill(&mut world, r);
        sailren::on_wave_kill(&mut world, KILLER, VELOCIRAPTOR);
    }
    assert_eq!(npc_count(&mut world, PTEROSAUR), 0, "raptors remain");

    // The third clears them → the Pterosaur enters.
    kill(&mut world, raptors[2]);
    sailren::on_wave_kill(&mut world, KILLER, VELOCIRAPTOR);
    assert_eq!(npc_count(&mut world, PTEROSAUR), 1, "pterosaur summoned");

    // Pterosaur → Tyrannosaurus.
    let ptero = tagged(&mut world, PTEROSAUR)[0];
    kill(&mut world, ptero);
    sailren::on_wave_kill(&mut world, KILLER, PTEROSAUR);
    assert_eq!(npc_count(&mut world, TREX), 1, "trex summoned");

    // Trex falling arms Sailren's entrance (on a timer, not immediate).
    let trex = tagged(&mut world, TREX)[0];
    kill(&mut world, trex);
    sailren::on_wave_kill(&mut world, KILLER, TREX);
    assert_eq!(
        npc_count(&mut world, SAILREN),
        0,
        "not until the timer fires"
    );
    sailren::handle_spawn_sailren(&mut world);
    assert_eq!(npc_count(&mut world, SAILREN), 1, "Sailren enters");
}

#[test]
fn sailren_enters_invulnerable_then_the_fight_begins() {
    let (mut world, _db, _l) = sailren_world();
    sailren::handle_spawn_sailren(&mut world);
    let sailren = tagged(&mut world, SAILREN)[0];

    assert!(
        world
            .objects
            .get_component::<AdminFlags>(&sailren)
            .unwrap()
            .invul,
        "invulnerable during the intro"
    );
    assert!(
        world.objects.has_component::<Immobilized>(&sailren),
        "rooted during the intro"
    );

    sailren::handle_attack_enable(&mut world, sailren);
    assert!(
        !world
            .objects
            .get_component::<AdminFlags>(&sailren)
            .map(|f| f.invul)
            .unwrap_or(true),
        "vulnerable once the fight begins"
    );
    assert!(
        !world.objects.has_component::<Immobilized>(&sailren),
        "free to move once the fight begins"
    );
}

#[test]
fn felling_sailren_drops_the_exit_cube() {
    let (mut world, _db, _l) = sailren_world();
    sailren::on_wave_kill(&mut world, KILLER, SAILREN);
    assert_eq!(npc_count(&mut world, CUBIC), 1, "the teleport cube appears");
}

/// A solo player can't start the fight — Java shows `32109-01.html`.
#[test]
fn a_solo_player_cannot_start_the_fight() {
    let (mut world, _db, _l) = sailren_world();
    let _rx = ingame_player(&mut world, 1, 100, 100, 100, 0);
    assert_eq!(
        sailren::entry_refusal(&mut world, 100),
        Some("32109-01.html"),
        "no party, no fight"
    );
}

/// Admitting a party teleports the leader's nearby members to the nest and arms
/// the first wave.
#[test]
fn admitting_a_party_teleports_members_and_arms_the_wave() {
    let (mut world, _db, _l) = sailren_world();
    let _a = ingame_player(&mut world, 1, 100, 26_000, -6_000, -2_000);
    let _b = ingame_player(&mut world, 2, 200, 26_050, -6_000, -2_000);
    make_party(&mut world, &[100, 200], LootRule::FindersKeepers);
    let before = world.scheduler.len();

    sailren::enter_party(&mut world, 100);

    let pos = world.objects.get_component::<Position>(&200).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (27549, -6638),
        "the member is teleported in"
    );
    assert!(world.scheduler.len() > before, "the first wave is armed");
}
