//! Team vs Team event (G28) — slices 1–2: the lifecycle, the registration phase
//! (open → register/cancel → window close), and the arena stand-up (team split
//! + teleport, fight-window door/timer chain, teardown). Scoring and rewards are
//! slices 3–4.

use super::*;

use crate::data::instance_data::{ExitType, InstanceTemplate};
use crate::game_loop::events::tvt;
use crate::model::components::InstanceId;
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

/// A minimal coliseum template (3049) so `teleport_to_arena` can create the
/// arena. Doors are left empty — the fight-door open/close is a no-op then,
/// which is fine for the lifecycle assertions (G27 covers instance doors).
fn register_coliseum_template(world: &mut World) {
    world
        .data
        .instance_templates
        .insert_for_test(InstanceTemplate {
            id: 3049,
            name: Some("coliseum".into()),
            max_worlds: -1,
            duration_min: 0,
            empty_destroy_min: 0,
            enter: None,
            exit: ExitType::Origin,
            doors: vec![],
            groups: vec![],
        });
}

/// An in-game player at `oid` with a participation-eligible level.
fn eligible_player(world: &mut World, client_id: u32, oid: i32) {
    ingame_player(world, client_id, oid, 83425, 148585, -3406);
    if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
        p.level = 80;
    }
}

/// The templates a full run needs, an open event, and `n` registered players.
fn started_with_players(n: i32) -> (World, Vec<i32>) {
    let (mut world, _tx, _rx, _link) = test_world();
    register_manager_template(&mut world);
    register_coliseum_template(&mut world);
    tvt::event_start(&mut world);
    let mut oids = Vec::new();
    for i in 0..n {
        let cid = (i + 1) as u32;
        let oid = 100 + i;
        eligible_player(&mut world, cid, oid);
        tvt::on_manager_event(&mut world, cid, oid, "Participate");
        oids.push(oid);
    }
    (world, oids)
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

// ---------------------------------------------------------------------------
// Slice 2 — arena stand-up, fight window, teardown
// ---------------------------------------------------------------------------

#[test]
fn teleport_to_arena_stands_up_the_arena_and_splits_teams() {
    let (mut world, oids) = started_with_players(4);

    tvt::teleport_to_arena(&mut world);

    // The arena instance is up and we're in the warm-up phase.
    assert!(world.events.tvt.world_id.is_some());
    assert_eq!(world.events.tvt.phase, TvtPhase::Warmup);
    // 4 players → strict alternation → 2 per side.
    assert_eq!(world.events.tvt.blue_team.len(), 2);
    assert_eq!(world.events.tvt.red_team.len(), 2);
    // Every participant is in the instance with a team colour set.
    let instance_id = world.events.tvt.world_id.unwrap();
    for oid in &oids {
        assert_eq!(
            world.objects.get_component::<InstanceId>(oid).map(|i| i.0),
            Some(instance_id)
        );
        assert_ne!(world.objects.get_component::<Player>(oid).unwrap().team, 0);
        // Registration flag cleared, in-event flag set.
        let p = world.objects.get_component::<Player>(oid).unwrap();
        assert!(!p.registered_on_event);
        assert!(p.on_event);
    }
    // The fight-start timer is armed.
    assert!(world
        .scheduler
        .pending_tasks_for_test()
        .contains(&ScheduledTask::TvtStartFight));
}

#[test]
fn start_fight_opens_the_window_and_arms_end() {
    let (mut world, _oids) = started_with_players(4);
    tvt::teleport_to_arena(&mut world);

    tvt::start_fight(&mut world);
    assert_eq!(world.events.tvt.phase, TvtPhase::Fighting);
    assert!(world
        .scheduler
        .pending_tasks_for_test()
        .contains(&ScheduledTask::TvtEndFight));
}

#[test]
fn end_fight_tears_the_arena_down_and_frees_players() {
    let (mut world, oids) = started_with_players(4);
    tvt::teleport_to_arena(&mut world);
    let instance_id = world.events.tvt.world_id.unwrap();
    tvt::start_fight(&mut world);

    tvt::end_fight(&mut world);

    // Event over, arena destroyed.
    assert_eq!(world.events.active, None);
    assert_eq!(world.events.tvt.phase, TvtPhase::Inactive);
    assert!(world.events.tvt.world_id.is_none());
    assert!(!world.instances.contains(instance_id));
    // Players ousted: no instance tag, team + event flags cleared.
    for oid in &oids {
        assert!(world.objects.get_component::<InstanceId>(oid).is_none());
        let p = world.objects.get_component::<Player>(oid).unwrap();
        assert_eq!(p.team, 0);
        assert!(!p.on_event);
    }
}

#[test]
fn a_full_event_runs_start_to_finish() {
    // The G28 gate, minus scoring/rewards (slices 3–4): open → register →
    // teleport in → fight window → teardown, with no state left behind.
    let (mut world, _oids) = started_with_players(4);
    tvt::teleport_to_arena(&mut world);
    tvt::start_fight(&mut world);
    tvt::end_fight(&mut world);

    assert_eq!(world.events.active, None);
    assert_eq!(world.events.tvt.phase, TvtPhase::Inactive);
    assert!(world.events.tvt.player_list.is_empty());
    assert!(world.events.tvt.blue_team.is_empty());
    assert!(world.events.tvt.red_team.is_empty());
    assert_eq!(world.instances.len(), 0);
}
