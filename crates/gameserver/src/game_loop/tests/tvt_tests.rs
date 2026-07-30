//! Team vs Team event (G28) — the full match: registration (open →
//! register/cancel → window close), arena stand-up (team split + teleport),
//! scoring + respawn, and EndFight (winner reward / tie, forfeit, teardown).

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
    // A live object-id pool so the winner reward's `add_inventory_item` can
    // allocate item stacks (see the cursed-weapons id-pool gotcha).
    world.id_pool = 0x5000_0000..0x5000_1000;
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
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .contains(&ScheduledTask::TvtTeleportToArena)
    );

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
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .contains(&ScheduledTask::TvtStartFight)
    );
}

#[test]
fn start_fight_opens_the_window_and_arms_end() {
    let (mut world, _oids) = started_with_players(4);
    tvt::teleport_to_arena(&mut world);

    tvt::start_fight(&mut world);
    assert_eq!(world.events.tvt.phase, TvtPhase::Fighting);
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .contains(&ScheduledTask::TvtEndFight)
    );
}

#[test]
fn end_fight_tears_the_arena_down_and_frees_players() {
    let (mut world, oids) = started_with_players(4);
    tvt::teleport_to_arena(&mut world);
    let instance_id = world.events.tvt.world_id.unwrap();
    tvt::start_fight(&mut world);

    tvt::end_fight(&mut world);
    tvt::teleport_out(&mut world); // the 7s teleport-out timer

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
    // open → register → teleport in → fight window → end (rewards) →
    // teleport-out teardown, with no state left behind.
    let (mut world, _oids) = started_with_players(4);
    tvt::teleport_to_arena(&mut world);
    tvt::start_fight(&mut world);
    tvt::end_fight(&mut world);
    // EndFight resolves + freezes but leaves the arena up for the scoreboard;
    // the teleport-out timer (fired here) does the teardown.
    assert_eq!(world.events.tvt.phase, TvtPhase::Ending);
    tvt::teleport_out(&mut world);

    assert_eq!(world.events.active, None);
    assert_eq!(world.events.tvt.phase, TvtPhase::Inactive);
    assert!(world.events.tvt.player_list.is_empty());
    assert!(world.events.tvt.blue_team.is_empty());
    assert!(world.events.tvt.red_team.is_empty());
    assert_eq!(world.instances.len(), 0);
}

// ---------------------------------------------------------------------------
// Slice 3 — scoring & respawn
// ---------------------------------------------------------------------------

/// Get to the fighting phase with `n` participants split into teams.
fn fighting_arena(n: i32) -> (World, Vec<i32>) {
    let (mut world, oids) = started_with_players(n);
    tvt::teleport_to_arena(&mut world);
    tvt::start_fight(&mut world);
    (world, oids)
}

#[test]
fn a_cross_team_kill_scores_and_queues_respawn() {
    let (mut world, _oids) = fighting_arena(4);
    let killer = world.events.tvt.blue_team[0];
    let victim = world.events.tvt.red_team[0];

    tvt::on_player_death(&mut world, victim, killer);

    assert_eq!(world.events.tvt.blue_score, 1);
    assert_eq!(world.events.tvt.red_score, 0);
    assert_eq!(world.events.tvt.scores.get(&killer), Some(&1));
    // The victim is queued for a timed respawn.
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .contains(&ScheduledTask::TvtResurrect { player: victim })
    );
}

#[test]
fn a_same_team_kill_does_not_score() {
    let (mut world, _oids) = fighting_arena(4);
    let killer = world.events.tvt.blue_team[0];
    let victim = world.events.tvt.blue_team[1];

    tvt::on_player_death(&mut world, victim, killer);

    assert_eq!(world.events.tvt.blue_score, 0);
    assert_eq!(world.events.tvt.red_score, 0);
    assert_eq!(
        world.events.tvt.scores.get(&killer).copied().unwrap_or(0),
        0
    );
}

#[test]
fn resurrect_revives_a_dead_participant_at_their_team_spawn() {
    let (mut world, _oids) = fighting_arena(4);
    let victim = world.events.tvt.red_team[0];
    world
        .objects
        .get_component_mut::<Vitals>(&victim)
        .unwrap()
        .dead = true;

    tvt::resurrect_player(&mut world, victim);

    assert!(!world.objects.get_component::<Vitals>(&victim).unwrap().dead);
    // Teleported to the red spawn (x is not geo-adjusted, unlike z).
    assert_eq!(
        world.objects.get_component::<Position>(&victim).unwrap().x,
        151536
    );
}

#[test]
fn a_stale_respawn_after_teardown_is_a_no_op() {
    let (mut world, _oids) = fighting_arena(4);
    let victim = world.events.tvt.red_team[0];
    world
        .objects
        .get_component_mut::<Vitals>(&victim)
        .unwrap()
        .dead = true;
    tvt::teleport_out(&mut world); // ends the event + clears on_event

    // The queued resurrect now fires late: no revive (off-event), no panic.
    tvt::resurrect_player(&mut world, victim);
    assert!(world.objects.get_component::<Vitals>(&victim).unwrap().dead);
}

#[test]
fn player_do_die_drives_tvt_scoring() {
    // The real wire: a player death routed through `death::player_do_die` must
    // reach TvT's scoring (the uncalled-code catcher).
    let (mut world, _oids) = fighting_arena(4);
    let killer = world.events.tvt.blue_team[0];
    let victim = world.events.tvt.red_team[0];

    crate::game_loop::death::player_do_die(&mut world, victim, killer);

    assert_eq!(world.events.tvt.blue_score, 1);
    assert!(world.objects.get_component::<Vitals>(&victim).unwrap().dead);
}

// ---------------------------------------------------------------------------
// Slice 4 — EndFight rewards, teardown, forfeit, logout
// ---------------------------------------------------------------------------

/// Register a stackable Adena template so the winner reward lands.
fn register_adena(world: &mut World) {
    let mut t = crate::data::item_data::ItemTemplate::default();
    t.item_id = 57;
    t.name = "Adena".into();
    t.is_stackable = true;
    world.data.item_data.insert_for_test(t);
}

fn adena_count(world: &World, oid: i32) -> i64 {
    world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&oid)
        .map_or(0, |inv| inv.count_of(57))
}

#[test]
fn end_fight_rewards_the_winning_team_and_freezes_everyone() {
    let (mut world, _oids) = fighting_arena(4);
    register_adena(&mut world);
    // Blue takes the lead.
    world.events.tvt.blue_score = 3;
    world.events.tvt.red_score = 1;
    let blue = world.events.tvt.blue_team.clone();
    let red = world.events.tvt.red_team.clone();

    tvt::end_fight(&mut world);

    assert_eq!(world.events.tvt.phase, TvtPhase::Ending);
    // The scoreboard + teleport-out timers are armed.
    let pending = world.scheduler.pending_tasks_for_test();
    assert!(pending.contains(&ScheduledTask::TvtScoreBoard));
    assert!(pending.contains(&ScheduledTask::TvtTeleportOut));
    // Winners rewarded, losers not.
    for oid in &blue {
        assert_eq!(adena_count(&world, *oid), 100_000);
    }
    for oid in &red {
        assert_eq!(adena_count(&world, *oid), 0);
    }
    // Everyone is frozen (invulnerable).
    for oid in blue.iter().chain(red.iter()) {
        assert!(
            world
                .objects
                .get_component::<crate::model::components::AdminFlags>(oid)
                .unwrap()
                .invul
        );
    }
}

#[test]
fn a_tie_rewards_no_one() {
    let (mut world, _oids) = fighting_arena(4);
    register_adena(&mut world);
    // Scores level (both 0).
    let everyone: Vec<i32> = world
        .events
        .tvt
        .blue_team
        .iter()
        .chain(world.events.tvt.red_team.iter())
        .copied()
        .collect();

    tvt::end_fight(&mut world);

    assert_eq!(world.events.tvt.phase, TvtPhase::Ending);
    for oid in &everyone {
        assert_eq!(adena_count(&world, *oid), 0);
    }
}

#[test]
fn teleport_out_unfreezes_and_tears_down() {
    let (mut world, oids) = fighting_arena(4);
    let instance_id = world.events.tvt.world_id.unwrap();
    tvt::end_fight(&mut world);

    tvt::teleport_out(&mut world);

    assert_eq!(world.events.active, None);
    assert!(!world.instances.contains(instance_id));
    for oid in &oids {
        let flags = world
            .objects
            .get_component::<crate::model::components::AdminFlags>(oid);
        // Invul cleared (either the flag is gone or false).
        assert!(flags.is_none_or(|f| !f.invul));
        assert!(world.objects.get_component::<InstanceId>(oid).is_none());
    }
}

#[test]
fn a_logout_that_empties_a_team_forfeits_the_match() {
    let (mut world, _oids) = fighting_arena(4);
    let reds = world.events.tvt.red_team.clone();

    // First red leaves: red still has a member, no forfeit yet.
    tvt::on_player_logout(&mut world, reds[0]);
    assert!(
        !world
            .scheduler
            .pending_tasks_for_test()
            .contains(&ScheduledTask::TvtEndFight)
            || world.events.tvt.red_team.len() == 1
    );
    // Second red leaves: red empty, blue not → forfeit arms an early EndFight.
    tvt::on_player_logout(&mut world, reds[1]);

    assert!(world.events.tvt.red_team.is_empty());
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .contains(&ScheduledTask::TvtEndFight)
    );
}

#[test]
fn a_full_event_with_a_winner_end_to_end() {
    // The G28 gate in full: open → register → arena → a scored kill → EndFight
    // (winner rewarded) → teleport-out, clean.
    let (mut world, _oids) = fighting_arena(4);
    register_adena(&mut world);
    let killer = world.events.tvt.blue_team[0];
    let victim = world.events.tvt.red_team[0];

    tvt::on_player_death(&mut world, victim, killer);
    assert_eq!(world.events.tvt.blue_score, 1);

    tvt::end_fight(&mut world);
    assert_eq!(adena_count(&world, killer), 100_000);

    tvt::teleport_out(&mut world);
    assert_eq!(world.events.active, None);
    assert_eq!(world.instances.len(), 0);
}

// ---------------------------------------------------------------------------
// Row 10 — countdown screens and the in-arena manager's buff/heal
// ---------------------------------------------------------------------------

/// **Each phase arms Java's second-by-second countdown.** The warm-up gets
/// "5".."1" and the fight "10".."1", each a one-shot task; ending the fight
/// bumps the generation so the pending ticks from the old chain go quiet.
#[test]
fn each_phase_arms_a_countdown_that_the_end_cancels() {
    let (mut world, _oids) = started_with_players(4);
    tvt::teleport_to_arena(&mut world);

    let countdowns = |world: &World| {
        world
            .scheduler
            .pending_tasks_for_test()
            .iter()
            .filter(|t| matches!(t, ScheduledTask::TvtCountdown { .. }))
            .count()
    };
    assert_eq!(countdowns(&world), 5, "the warm-up arms 5..1");

    tvt::start_fight(&mut world);
    assert_eq!(countdowns(&world), 15, "the fight adds 10..1");

    // A tick from the live chain shows its number; the seq bump at EndFight
    // silences whatever is still queued.
    let seq = world.events.tvt.countdown_seq;
    tvt::end_fight(&mut world);
    assert_ne!(
        world.events.tvt.countdown_seq, seq,
        "ending the fight retires the chain"
    );
}

/// **The in-arena manager buffs and tops the participant up.** Java's
/// `BuffHeal` casts the class-appropriate set and refills HP/MP/CP; a player
/// in combat is refused (Java shows `manager-combat.html`).
#[test]
fn the_arena_manager_buffs_and_heals() {
    use crate::model::components::{AttackState, LastFolkNpc, PlayerVitals, Vitals};

    let (mut world, _oids) = fighting_arena(4);
    let player = world.events.tvt.blue_team[0];
    // The manager the player clicked (Java passes the npc straight through;
    // only the *page* differs between the town and arena copies).
    let manager = *world
        .events
        .tvt
        .arena_managers
        .first()
        .expect("an arena manager stands in the instance");
    world.objects.add_components(&player, LastFolkNpc(manager));
    // Hurt them.
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&player) {
        v.cur_hp = 1.0;
        v.cur_mp = 1.0;
    }
    if let Some(pv) = world.objects.get_component_mut::<PlayerVitals>(&player) {
        pv.cur_cp = 0.0;
    }

    // In combat: refused, nothing changes.
    world.objects.add_components(
        &player,
        AttackState {
            attack_end_tick: 0,
            stance_until_tick: world.tick + 100,
        },
    );
    tvt::on_manager_event(&mut world, 1, player, "BuffHeal");
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&player)
            .unwrap()
            .cur_hp,
        1.0,
        "a fighting participant is refused"
    );

    // Out of combat: full top-up.
    world.objects.add_components(
        &player,
        AttackState {
            attack_end_tick: 0,
            stance_until_tick: 0,
        },
    );
    tvt::on_manager_event(&mut world, 1, player, "BuffHeal");
    let v = *world.objects.get_component::<Vitals>(&player).unwrap();
    assert_eq!(v.cur_hp, f64::from(v.max_hp), "HP topped up");
    assert_eq!(v.cur_mp, f64::from(v.max_mp), "MP topped up");
    let pv = *world
        .objects
        .get_component::<PlayerVitals>(&player)
        .unwrap();
    assert_eq!(pv.cur_cp, f64::from(pv.max_cp), "CP topped up");
}
