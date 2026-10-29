//! The instance framework the class quests run through: `enterInstance` with
//! the template's conditions, the per-door `openStatus`, a spawn line's
//! `respawnTime`, and the duration clock (`setDuration` / `finishInstance`).
//! Driven against the real Labyrinth of Belis template (178).

use super::*;
use crate::game_loop::space::instances;
use crate::model::components::space::{InstanceDoorOpen, InstanceId};
use crate::model::door::Door;
use crate::network::server_packets::sm_ids;

const BELIS: i32 = 178;
const EVIL_INCUBATOR: i32 = 185;
const PLAYER: i32 = 7101;
const HANDYMAN: i32 = 22997;

fn belis_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();
    let rx = ingame_player(&mut world, 1, PLAYER, -111782, 231892, -3178);
    (world, rx)
}

fn door_open(world: &World, instance_id: i32, door_id: i32) -> bool {
    let inst = world.instances.get(instance_id).expect("instance");
    let oid = *inst
        .doors
        .iter()
        .find(|&&oid| world.objects.get_component::<Door>(&oid).unwrap().door_id == door_id)
        .expect("door copy");
    world
        .objects
        .get_component::<InstanceDoorOpen>(&oid)
        .unwrap()
        .0
}

/// `ConditionLevel`: below `min` the whole group gets SM 2097 and nothing is
/// created; at it, the player lands on the enter location inside a fresh
/// world whose first door honours the template's `openStatus default="open"`.
#[test]
fn the_level_condition_gates_entry_and_the_world_opens_as_templated() {
    let (mut world, mut rx) = belis_world();
    set_level(&mut world, PLAYER, 17);
    drain(&mut rx);
    assert_eq!(instances::enter_instance(&mut world, PLAYER, BELIS), None);
    assert!(has_sm(
        &drain(&mut rx),
        sm_ids::C1_S_LEVEL_DOES_NOT_CORRESPOND_TO_THE_REQUIREMENTS_FOR_ENTRY
    ));
    assert_eq!(world.instances.world_count(BELIS), 0);

    set_level(&mut world, PLAYER, 18);
    let id = instances::enter_instance(&mut world, PLAYER, BELIS).expect("entered");
    assert_eq!(
        world
            .objects
            .get_component::<InstanceId>(&PLAYER)
            .map(|i| i.0),
        Some(id)
    );
    let pos = world.objects.get_component::<Position>(&PLAYER).unwrap();
    assert_eq!((pos.x, pos.y), (-119941, 211148));
    assert!(door_open(&world, id, 16240001), "openStatus default=open");
    assert!(
        !door_open(&world, id, 16240002),
        "the door template's default"
    );
}

/// `enterInstance` sends a player back into the world that already lets them
/// in, and refuses a different template with SM 2105.
#[test]
fn an_allowed_player_reenters_their_world_and_no_other() {
    let (mut world, mut rx) = belis_world();
    set_level(&mut world, PLAYER, 20);
    let id = instances::enter_instance(&mut world, PLAYER, BELIS).expect("entered");
    assert_eq!(
        instances::enter_instance(&mut world, PLAYER, BELIS),
        Some(id)
    );
    assert_eq!(world.instances.world_count(BELIS), 1);

    drain(&mut rx);
    assert_eq!(
        instances::enter_instance(&mut world, PLAYER, EVIL_INCUBATOR),
        None
    );
    assert!(has_sm(
        &drain(&mut rx),
        sm_ids::YOU_HAVE_ENTERED_ANOTHER_INSTANT_ZONE_THEREFORE_YOU_CANNOT_ENTER_CORRESPONDING_DUNGEON
    ));
}

/// Belis has `empty="0"`: the last player out takes the world down.
#[test]
fn an_empty_zero_world_goes_when_its_last_player_leaves() {
    let (mut world, _rx) = belis_world();
    set_level(&mut world, PLAYER, 20);
    let id = instances::enter_instance(&mut world, PLAYER, BELIS).expect("entered");
    instances::exit(&mut world, PLAYER);
    advance_ticks(&mut world, 1);
    assert!(world.instances.get(id).is_none());
}

/// A Handyman line carries `respawnTime="15sec"`: after its corpse decays the
/// NPC comes back into the same world.
#[test]
fn an_instance_spawn_line_respawns_into_its_world() {
    let (mut world, _rx) = belis_world();
    set_level(&mut world, PLAYER, 20);
    let id = instances::enter_instance(&mut world, PLAYER, BELIS).expect("entered");
    let handymen = |world: &World| -> Vec<i32> {
        world
            .instances
            .get(id)
            .unwrap()
            .npcs
            .iter()
            .copied()
            .filter(|&oid| {
                world.objects.contains_id(&oid)
                    && crate::game_loop::npc::npc_id_of(world, oid) == Some(HANDYMAN)
            })
            .collect()
    };
    let before = handymen(&world);
    assert_eq!(before.len(), 4);
    let victim = before[0];
    world
        .objects
        .get_component_mut::<Vitals>(&victim)
        .unwrap()
        .dead = true;
    crate::game_loop::npc::handle_npc_decay(&mut world, victim);
    assert_eq!(handymen(&world).len(), 3);
    advance_ticks(&mut world, 150);
    let after = handymen(&world);
    assert_eq!(after.len(), 4, "the line respawned");
    let fresh = *after.iter().find(|oid| !before.contains(oid)).unwrap();
    assert_eq!(
        world
            .objects
            .get_component::<InstanceId>(&fresh)
            .map(|i| i.0),
        Some(id)
    );
}

/// `setDuration`: Belis runs 60 minutes, warned at 5 (SM 2107) and torn down
/// at the end; `finishInstance(1)` warns at 1 and ends a minute later.
#[test]
fn the_duration_clock_warns_then_destroys() {
    let (mut world, mut rx) = belis_world();
    set_level(&mut world, PLAYER, 20);
    let id = instances::enter_instance(&mut world, PLAYER, BELIS).expect("entered");
    drain(&mut rx);
    advance_ticks(&mut world, 55 * 600);
    assert!(has_sm(
        &drain(&mut rx),
        sm_ids::THIS_INSTANT_ZONE_WILL_BE_TERMINATED_IN_S1_MIN_YOU_WILL_BE_FORCED_OUT_OF_THE_DUNGEON_WHEN_THE_TIME_EXPIRES
    ));
    instances::finish_instance(&mut world, id, 1);
    assert!(has_sm(
        &drain(&mut rx),
        sm_ids::THIS_INSTANT_ZONE_WILL_BE_TERMINATED_IN_S1_MIN_YOU_WILL_BE_FORCED_OUT_OF_THE_DUNGEON_WHEN_THE_TIME_EXPIRES
    ));
    advance_ticks(&mut world, 599);
    assert!(world.instances.get(id).is_some(), "not before the minute");
    advance_ticks(&mut world, 2);
    assert!(
        world.instances.get(id).is_none(),
        "the old 60-minute clock is gone too"
    );
    assert_eq!(
        world
            .objects
            .get_component::<InstanceId>(&PLAYER)
            .map(|i| i.0),
        None,
        "the player was put out"
    );
}
