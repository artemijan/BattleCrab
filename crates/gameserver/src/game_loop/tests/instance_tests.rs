//! Instances (G27) slice 1: id allocation + cross-instance invisibility.

use super::*;

use crate::model::components::InstanceId;
use crate::network::server_packets::opcodes;

#[test]
fn instance_manager_allocates_unique_ids() {
    let (mut world, _tx, _db, _l) = test_world();
    let a = world.instances.create(0);
    let b = world.instances.create(0);
    assert_ne!(a, b, "distinct ids");
    assert!(a >= 1 && b >= 1, "0 is reserved for the overworld");
    assert!(world.instances.contains(a) && world.instances.contains(b));

    world.instances.destroy(a);
    assert!(!world.instances.contains(a), "destroyed");
    assert!(world.instances.contains(b), "the other survives");
}

fn saw_char_info(packets: &[Vec<u8>]) -> bool {
    packets
        .iter()
        .any(|p| p.first() == Some(&opcodes::CHAR_INFO))
}

#[test]
fn players_share_visibility_only_within_an_instance() {
    let (mut world, _tx, _db, _l) = test_world();
    // Two players standing on the exact same spot.
    let mut rx_a = ingame_player(&mut world, 1, 100, 1000, 1000, 0);
    let _rx_b = ingame_player(&mut world, 2, 200, 1000, 1000, 0);

    // Same instance (both overworld) → they see each other on enter.
    crate::game_loop::visibility::on_enter_world(&world, 1, 100);
    assert!(
        saw_char_info(&drain(&mut rx_a)),
        "same instance: CharInfo exchanged"
    );

    // Move A into a private instance → no cross-instance CharInfo.
    world.objects.add_components(&100, InstanceId(7));
    crate::game_loop::visibility::on_enter_world(&world, 1, 100);
    assert!(
        !saw_char_info(&drain(&mut rx_a)),
        "different instances: invisible to each other"
    );
}

fn saw_npc_info(packets: &[Vec<u8>]) -> bool {
    packets
        .iter()
        .any(|p| p.first() == Some(&opcodes::NPC_INFO))
}

#[test]
fn npcs_are_visible_only_within_their_instance() {
    let (mut world, _tx, _db, _l) = test_world();
    add_test_npc(&mut world, 800, 30001, "Folk", 5, 1000, 1000, 0);
    let mut rx = ingame_player(&mut world, 1, 100, 1000, 1000, 0);

    // Player in a private instance, NPC in the overworld → NPC hidden.
    world.objects.add_components(&100, InstanceId(3));
    crate::game_loop::visibility::on_enter_world(&world, 1, 100);
    assert!(
        !saw_npc_info(&drain(&mut rx)),
        "an instanced player doesn't see overworld NPCs"
    );

    // Move the NPC into the same instance → now visible.
    world.objects.add_components(&800, InstanceId(3));
    crate::game_loop::visibility::on_enter_world(&world, 1, 100);
    assert!(
        saw_npc_info(&drain(&mut rx)),
        "a same-instance NPC is visible"
    );
}

// ---- slice 4: instance lifecycle (create → enter → exit → destroy) ----

use crate::data::instance_data::{ExitType, InstanceTemplate, SpawnGroup, TemplateSpawn};
use crate::game_loop::helpers::instance_of;
use crate::game_loop::instances;

/// Register an NPC template so `spawn_npc_at` resolves it, then seed an
/// instance template with a default group (one NPC) and a non-default group
/// (one NPC) — plus an enter location and an ORIGIN exit.
fn seed_instance_template(world: &mut World, template_id: i32, npc_id: i32) {
    if world.data.npc_data.get(npc_id).is_none() {
        world
            .data
            .npc_data
            .insert_for_test(crate::data::npc_data::default_template(npc_id));
    }
    world
        .data
        .instance_templates
        .insert_for_test(InstanceTemplate {
            id: template_id,
            name: Some("Test Instance".into()),
            max_worlds: -1,
            duration_min: 0,
            empty_destroy_min: 0,
            enter: Some((5000, 5000, 100)),
            exit: ExitType::Origin,
            doors: vec![],
            groups: vec![
                SpawnGroup {
                    name: "default".into(),
                    spawn_by_default: true,
                    npcs: vec![TemplateSpawn {
                        npc_id,
                        x: 5000,
                        y: 5000,
                        z: 100,
                        heading: 0,
                    }],
                },
                SpawnGroup {
                    name: "onDemand".into(),
                    spawn_by_default: false,
                    npcs: vec![TemplateSpawn {
                        npc_id,
                        x: 5100,
                        y: 5100,
                        z: 100,
                        heading: 0,
                    }],
                },
            ],
        });
}

#[test]
fn create_from_template_spawns_only_the_default_group() {
    let (mut world, _tx, _db, _l) = test_world();
    seed_instance_template(&mut world, 900, 30001);

    let iid = instances::create_from_template(&mut world, 900).expect("template exists");
    let inst = world.instances.get(iid).expect("live");
    // Only the spawn_by_default group populated (1 NPC, not 2).
    assert_eq!(inst.npcs.len(), 1, "the on-demand group stays dormant");

    // The spawned NPC is tagged into this instance.
    let npc_oid = inst.npcs[0];
    assert_eq!(
        instance_of(&world, npc_oid),
        iid,
        "NPC lives in the instance"
    );

    // An unknown template id yields nothing.
    assert!(instances::create_from_template(&mut world, 99999).is_none());
}

#[test]
fn enter_then_exit_round_trips_position_and_membership() {
    let (mut world, _tx, _db, _l) = test_world();
    seed_instance_template(&mut world, 900, 30001);
    let _rx = ingame_player(&mut world, 1, 100, 1000, 1000, 0);

    let iid = instances::create_from_template(&mut world, 900).expect("template");

    instances::enter(&mut world, 100, iid);
    assert_eq!(instance_of(&world, 100), iid, "player is inside");
    assert_eq!(world.instances.member_count(iid), 1);
    let pos = world
        .objects
        .get_component::<Position>(&100)
        .expect("position");
    assert_eq!((pos.x, pos.y), (5000, 5000), "teleported to enter location");

    instances::exit(&mut world, 100);
    assert_eq!(instance_of(&world, 100), 0, "back in the overworld");
    assert_eq!(world.instances.member_count(iid), 0);
    let pos = world
        .objects
        .get_component::<Position>(&100)
        .expect("position");
    assert_eq!(
        (pos.x, pos.y),
        (1000, 1000),
        "ORIGIN exit returns to the entry spot"
    );
}

#[test]
fn destroy_ousts_members_and_despawns_npcs() {
    let (mut world, _tx, _db, _l) = test_world();
    seed_instance_template(&mut world, 900, 30001);
    let _rx = ingame_player(&mut world, 1, 100, 1000, 1000, 0);

    let iid = instances::create_from_template(&mut world, 900).expect("template");
    let npc_oid = world.instances.get(iid).unwrap().npcs[0];
    instances::enter(&mut world, 100, iid);

    instances::destroy(&mut world, iid);

    assert!(!world.instances.contains(iid), "instance is gone");
    assert_eq!(instance_of(&world, 100), 0, "member ousted to overworld");
    let pos = world
        .objects
        .get_component::<Position>(&100)
        .expect("position");
    assert_eq!((pos.x, pos.y), (1000, 1000), "ousted back to entry spot");
    assert!(
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
            .is_none(),
        "the instance's NPC was despawned"
    );
}

fn saw_delete_object(packets: &[Vec<u8>]) -> bool {
    packets
        .iter()
        .any(|p| p.first() == Some(&opcodes::DELETE_OBJECT))
}

/// An instanced NPC's despawn `DeleteObject` reaches same-instance players only
/// — the overworld player standing on the same spot never learns of it (G27
/// slice 6: NPC-lifecycle broadcasts are instance-scoped).
#[test]
fn instanced_npc_despawn_reaches_only_the_instance() {
    let (mut world, _tx, _db, _l) = test_world();
    add_test_npc(&mut world, 800, 30001, "Folk", 5, 1000, 1000, 0);
    world.objects.add_components(&800, InstanceId(7));

    let mut rx_in = ingame_player(&mut world, 1, 100, 1000, 1000, 0);
    world.objects.add_components(&100, InstanceId(7));
    let mut rx_out = ingame_player(&mut world, 2, 200, 1000, 1000, 0); // overworld
    drain(&mut rx_in);
    drain(&mut rx_out);

    let region = world
        .objects
        .get_component::<crate::model::components::RegionCell>(&800)
        .expect("npc region")
        .0;
    crate::game_loop::death::despawn_npc(&mut world, 800, region);

    assert!(
        saw_delete_object(&drain(&mut rx_in)),
        "the same-instance player sees the despawn"
    );
    assert!(
        !saw_delete_object(&drain(&mut rx_out)),
        "the overworld player on the same spot does not"
    );
}

#[test]
fn empty_check_destroys_only_when_still_empty() {
    let (mut world, _tx, _db, _l) = test_world();
    seed_instance_template(&mut world, 900, 30001);
    let _rx = ingame_player(&mut world, 1, 100, 1000, 1000, 0);

    let iid = instances::create_from_template(&mut world, 900).expect("template");
    instances::enter(&mut world, 100, iid);
    instances::exit(&mut world, 100); // arms the empty check

    // A member re-entered during the grace period → the check spares it.
    instances::enter(&mut world, 100, iid);
    instances::handle_empty_check(&mut world, iid);
    assert!(world.instances.contains(iid), "occupied: not destroyed");

    // Empty again → the check tears it down.
    instances::exit(&mut world, 100);
    instances::handle_empty_check(&mut world, iid);
    assert!(!world.instances.contains(iid), "empty: destroyed");
}

#[test]
fn broadcast_is_scoped_to_the_instance() {
    let (mut world, _tx, _db, _l) = test_world();
    let _rx_a = ingame_player(&mut world, 1, 100, 1000, 1000, 0);
    let mut rx_b = ingame_player(&mut world, 2, 200, 1000, 1000, 0);
    world.objects.add_components(&100, InstanceId(7)); // A alone in instance 7

    crate::game_loop::helpers::broadcast_to_others(&world, 100, &[0xAB, 0xCD]);

    assert!(
        drain(&mut rx_b).is_empty(),
        "an overworld player receives nothing from an instanced broadcaster"
    );
}
