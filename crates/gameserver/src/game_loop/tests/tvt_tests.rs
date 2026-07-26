//! Team vs Team event (G28) — slice 1: the lifecycle skeleton and the
//! registration phase (open → register/cancel → window close). Arena, fight
//! and rewards are slices 2–4.

use super::*;

use crate::game_loop::events::tvt;
use crate::model::event::TvtPhase;
use crate::scheduler::ScheduledTask;

/// Register the manager NPC template (70010) so `event_start`'s spawn resolves.
fn register_manager_template(world: &mut World) {
    let mut t = crate::data::npc_data::default_template(tvt::MANAGER);
    t.type_name = "Npc".into();
    t.level = 70;
    t.base_hp_max = 100.0;
    t.base_mp_max = 50.0;
    world.data.npc_data.insert_for_test(t);
}

/// An in-game player at `oid` with a participation-eligible level.
fn eligible_player(world: &mut World, client_id: u32, oid: i32) {
    ingame_player(world, client_id, oid, 83425, 148585, -3406);
    if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
        p.level = 80;
    }
}

#[test]
fn event_start_opens_registration_and_arms_close_timer() {
    let (mut world, _tx, _rx, _link) = test_world();
    register_manager_template(&mut world);

    assert!(tvt::event_start(&mut world));
    assert_eq!(world.events.active, Some("TvT"));
    assert_eq!(world.events.tvt.phase, TvtPhase::Registration);
    // Manager NPC spawned.
    assert!(world.events.tvt.manager_oid.is_some());
    // The registration-close timer is armed.
    assert!(world
        .scheduler
        .pending_tasks_for_test()
        .contains(&ScheduledTask::TvtTeleportToArena));

    // Re-entry guard: a second start refuses while one is running.
    assert!(!tvt::event_start(&mut world));
}

#[test]
fn participate_registers_an_eligible_player() {
    let (mut world, _tx, _rx, _link) = test_world();
    register_manager_template(&mut world);
    tvt::event_start(&mut world);
    eligible_player(&mut world, 1, 100);

    let html = tvt::on_manager_event(&mut world, 1, 100, "Participate");
    assert_eq!(html.as_deref(), Some("registration-success.html"));
    assert!(world.events.tvt.player_list.contains(&100));
    assert!(
        world
            .objects
            .get_component::<Player>(&100)
            .unwrap()
            .registered_on_event
    );
}

#[test]
fn can_register_rejects_a_too_low_level_player() {
    let (mut world, _tx, _rx, _link) = test_world();
    register_manager_template(&mut world);
    tvt::event_start(&mut world);
    // Default dummy level is 1 — below the level 76 floor.
    ingame_player(&mut world, 1, 100, 0, 0, 0);

    let html = tvt::on_manager_event(&mut world, 1, 100, "Participate");
    assert_eq!(html.as_deref(), Some("registration-failed.html"));
    assert!(world.events.tvt.player_list.is_empty());
    assert!(
        !world
            .objects
            .get_component::<Player>(&100)
            .unwrap()
            .registered_on_event
    );
}

#[test]
fn cancel_participation_removes_the_registrant() {
    let (mut world, _tx, _rx, _link) = test_world();
    register_manager_template(&mut world);
    tvt::event_start(&mut world);
    eligible_player(&mut world, 1, 100);
    tvt::on_manager_event(&mut world, 1, 100, "Participate");

    let html = tvt::on_manager_event(&mut world, 1, 100, "CancelParticipation");
    assert_eq!(html.as_deref(), Some("registration-canceled.html"));
    assert!(world.events.tvt.player_list.is_empty());
    assert!(
        !world
            .objects
            .get_component::<Player>(&100)
            .unwrap()
            .registered_on_event
    );
}

#[test]
fn window_close_cancels_for_too_few_participants() {
    let (mut world, _tx, _rx, _link) = test_world();
    register_manager_template(&mut world);
    tvt::event_start(&mut world);
    eligible_player(&mut world, 1, 100);
    tvt::on_manager_event(&mut world, 1, 100, "Participate");

    // Only one registrant (< MINIMUM_PARTICIPANT_COUNT of 4): the window-close
    // handler cancels the event and clears the flag.
    tvt::teleport_to_arena(&mut world);
    assert_eq!(world.events.active, None);
    assert_eq!(world.events.tvt.phase, TvtPhase::Inactive);
    assert!(world.events.tvt.player_list.is_empty());
    assert!(
        !world
            .objects
            .get_component::<Player>(&100)
            .unwrap()
            .registered_on_event
    );
    // Manager despawned.
    assert!(world.events.tvt.manager_oid.is_none());
}

#[test]
fn event_stop_cancels_a_running_event() {
    let (mut world, _tx, _rx, _link) = test_world();
    register_manager_template(&mut world);
    tvt::event_start(&mut world);
    eligible_player(&mut world, 1, 100);
    tvt::on_manager_event(&mut world, 1, 100, "Participate");

    assert!(tvt::event_stop(&mut world));
    assert_eq!(world.events.active, None);
    assert_eq!(world.events.tvt.phase, TvtPhase::Inactive);
    assert!(
        !world
            .objects
            .get_component::<Player>(&100)
            .unwrap()
            .registered_on_event
    );
    // Stopping again with nothing running is a no-op.
    assert!(!tvt::event_stop(&mut world));
}
