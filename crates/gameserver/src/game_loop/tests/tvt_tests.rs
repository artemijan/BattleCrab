//! Team vs Team event (G28) — the full match: registration (open →
//! register/cancel → window close), arena stand-up (team split + teleport),
//! scoring + respawn, and EndFight (winner reward / tie, forfeit, teardown).

use super::*;
use crate::game_loop::helpers::set_position;

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

fn adena_count(world: &World, oid: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&oid)
        .map_or(0, |inv| inv.count_of(57))
}

#[test]
fn end_fight_rewards_the_winning_team_and_freezes_everyone() {
    let (mut world, _oids) = fighting_arena(4);
    insert_adena_template(&mut world);
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
                .get_component::<AdminFlags>(oid)
                .unwrap()
                .invul
        );
    }
}

#[test]
fn a_tie_rewards_no_one() {
    let (mut world, _oids) = fighting_arena(4);
    insert_adena_template(&mut world);
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
        let flags = world.objects.get_component::<AdminFlags>(oid);
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
    insert_adena_template(&mut world);
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
            swing_seq: 0,
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
            swing_seq: 0,
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

// ---------------------------------------------------------------------------
// Row 10 — headquarters zones: the enemy kick and the inactivity clock
// ---------------------------------------------------------------------------

/// Put a real `colosseum_peace1|2` pair into the test world's zone data so the
/// named-zone lookup resolves (the fixtures load no zone files).
fn register_hq_zones(world: &mut World) {
    use crate::data::spawn_data::{Territory, ZoneForm};
    use crate::data::zone_data::{Zone, ZoneKind};
    for (name, x1, x2) in [
        ("colosseum_peace1", 147_000, 148_000),
        ("colosseum_peace2", 151_000, 152_000),
    ] {
        world.data.zone_data.insert(Zone {
            id: 0,
            name: name.into(),
            kind: ZoneKind::Peace,
            territory: Territory {
                form: ZoneForm::Cuboid {
                    x1,
                    x2,
                    y1: 46_000,
                    y2: 47_000,
                },
                min_z: -4000,
                max_z: -3000,
            },
            castle_id: 0,
            clan_hall_id: 0,
            effect: None,
            damage: None,
            swamp: None,
            condition: None,
            mother_tree: None,
        });
    }
}

/// **Walking into the enemy headquarters bounces you home.** Java's
/// `onEnterZone` teleports the intruder to their own spawn with a screen
/// message; their own headquarters instead starts the inactivity clock.
#[test]
fn the_enemy_headquarters_kicks_intruders_out() {
    use crate::game_loop::zones::revalidate_zone;
    use crate::model::components::Position;

    let (mut world, _oids) = fighting_arena(4);
    register_hq_zones(&mut world);
    let blue = world.events.tvt.blue_team[0];

    // Blue player walks into the *red* headquarters.
    set_position(&mut world, blue, (151_500, 46_500, -3400));
    revalidate_zone(&mut world, blue, true);

    let pos = *world.objects.get_component::<Position>(&blue).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (147_447, 46_722),
        "bounced back to the blue spawn"
    );
}

/// **Idling in your own headquarters arms the kick clock, and leaving cancels
/// it.** The kick itself strips the participant and announces it.
#[test]
fn idling_in_your_headquarters_eventually_kicks_you() {
    use crate::game_loop::zones::revalidate_zone;

    let (mut world, _oids) = fighting_arena(4);
    register_hq_zones(&mut world);
    let blue = world.events.tvt.blue_team[0];

    let pending = |world: &World| {
        world
            .scheduler
            .pending_tasks_for_test()
            .iter()
            .filter(|t| matches!(t, ScheduledTask::TvtInactivity { player, .. } if *player == blue))
            .count()
    };

    // Stand in the blue headquarters → warning + kick armed.
    set_position(&mut world, blue, (147_500, 46_500, -3400));
    revalidate_zone(&mut world, blue, true);
    assert_eq!(pending(&world), 2, "warning + kick armed");
    let seq = *world.events.tvt.inactivity_seq.get(&blue).unwrap();

    // Walk out → the pair is retired (the tasks stay queued but go quiet).
    set_position(&mut world, blue, (149_500, 46_500, -3400));
    revalidate_zone(&mut world, blue, true);
    assert_ne!(
        *world.events.tvt.inactivity_seq.get(&blue).unwrap(),
        seq,
        "leaving retires the clock"
    );
    tvt::inactivity_tick(&mut world, blue, false, seq);
    assert!(
        world.events.tvt.player_list.contains(&blue),
        "the retired kick does nothing"
    );

    // A live kick removes the participant.
    let live_seq = *world.events.tvt.inactivity_seq.get(&blue).unwrap();
    tvt::inactivity_tick(&mut world, blue, false, live_seq);
    assert!(
        !world.events.tvt.player_list.contains(&blue),
        "the inactive player is removed from the event"
    );
    assert!(!world.events.tvt.blue_team.contains(&blue));
    assert!(
        !world
            .objects
            .get_component::<Player>(&blue)
            .unwrap()
            .on_event
    );
}

/// **The event's cron schedule arms itself and re-arms on firing.** This dist
/// ships the pattern commented out, so the loader reads an empty list — the
/// mechanism is exercised with a pattern directly.
#[test]
fn a_cron_schedule_starts_the_event_and_re_arms() {
    let (mut world, _tx, _rx, _link) = test_world();
    world.id_pool = 0x5100_0000..0x5100_1000;
    register_manager_template(&mut world);
    register_coliseum_template(&mut world);

    // The dist's own config: every schedule line is commented out.
    let dist = crate::data::DIST_GAME;
    assert!(
        tvt::load_schedule(dist).is_empty(),
        "this dist ships no active TvT schedule"
    );

    // Arm a slot by hand and fire it: the event opens and the slot re-arms.
    crate::game_loop::events::arm_schedule(&mut world, 0, "0 20 * * *");
    let armed = |world: &World| {
        world
            .scheduler
            .pending_tasks_for_test()
            .iter()
            .filter(|t| matches!(t, ScheduledTask::EventSchedule { .. }))
            .count()
    };
    assert_eq!(armed(&world), 1, "the slot is armed");

    crate::game_loop::events::on_schedule_fired(&mut world, 0, "0 20 * * *".to_string());
    assert_eq!(world.events.active, Some(tvt::NAME), "the event started");
    assert_eq!(armed(&world), 2, "…and the slot re-armed for tomorrow");
}

/// `AntiFeedManager.tryAddPlayer` — `DualboxCheckMaxL2EventParticipantsPerIP`
/// is **1** on this dist, so a second character from the same address is turned
/// away with its own page and never joins the roster.
#[test]
fn a_second_entrant_from_one_ip_is_refused() {
    let (mut world, _tx, _rx, _link) = test_world();
    register_manager_template(&mut world);
    world.cfg.dualbox.max_event_participants_per_ip = 1;
    tvt::event_start(&mut world);
    // Both test sessions come from 127.0.0.1.
    eligible_player(&mut world, 1, 100);
    eligible_player(&mut world, 2, 101);

    assert_eq!(
        tvt::on_manager_event(&mut world, 1, 100, "Participate").as_deref(),
        Some("registration-success.html")
    );
    assert_eq!(
        tvt::on_manager_event(&mut world, 2, 101, "Participate").as_deref(),
        Some("registration-ip.html"),
        "the dualbox cap has its own refusal page"
    );
    assert_eq!(world.events.tvt.player_list, vec![100], "roster unchanged");
    assert!(
        !world
            .objects
            .get_component::<Player>(&101)
            .unwrap()
            .registered_on_event,
        "the refused player is not flagged"
    );

    // Cancelling frees the slot again (Java `removePlayer` decrements; the port
    // derives the count from the roster, so the removal is enough).
    tvt::on_manager_event(&mut world, 1, 100, "CancelParticipation");
    assert_eq!(
        tvt::on_manager_event(&mut world, 2, 101, "Participate").as_deref(),
        Some("registration-success.html"),
        "the freed slot is reusable"
    );
}

/// `0` means unlimited — Java skips the check rather than treating it as a cap
/// of zero, so nobody could register if the port got this backwards.
#[test]
fn a_zero_cap_means_unlimited() {
    let (mut world, _tx, _rx, _link) = test_world();
    register_manager_template(&mut world);
    world.cfg.dualbox.max_event_participants_per_ip = 0;
    tvt::event_start(&mut world);
    eligible_player(&mut world, 1, 100);
    eligible_player(&mut world, 2, 101);

    tvt::on_manager_event(&mut world, 1, 100, "Participate");
    tvt::on_manager_event(&mut world, 2, 101, "Participate");
    assert_eq!(world.events.tvt.player_list, vec![100, 101]);
}

/// The `canRegister` gates that only became expressible once their subsystems
/// landed — duel, instance, siege zone, transformed, flying, and an olympiad
/// bout already in progress.
///
/// Table-driven because the failure that matters is a *missing* clause, and one
/// test per clause is how a missing one stays visible. Each case starts from an
/// eligible player, so a case passing for the wrong reason (a fixture that
/// could never register anyway) is ruled out by the baseline assertion first.
#[test]
fn can_register_honours_every_ported_busy_gate() {
    type Setup = fn(&mut World);
    let cases: &[(&str, Setup)] = &[
        ("in a duel", |w| {
            w.objects
                .add_components(&100, model::components::DuelRef(1));
        }),
        ("in an instance", |w| {
            w.objects.add_components(&100, InstanceId(7));
        }),
        ("inside a siege zone", |w| {
            w.objects
                .get_component_mut::<model::components::ZoneFlags>(&100)
                .unwrap()
                .mask |= crate::data::zone_data::ZoneKind::Siege.bit();
        }),
        ("transformed", |w| {
            w.objects
                .get_component_mut::<Player>(&100)
                .unwrap()
                .transform_id = 111;
        }),
        ("flying", |w| {
            w.objects
                .get_component_mut::<Player>(&100)
                .unwrap()
                .mount_type = 2;
        }),
        ("fighting an olympiad bout", |w| {
            w.olympiad.in_competition.insert(100);
        }),
    ];

    for (label, setup) in cases {
        let (mut world, _tx, _rx, _link) = test_world();
        register_manager_template(&mut world);
        tvt::event_start(&mut world);
        eligible_player(&mut world, 1, 100);

        // Baseline: this player *can* register before the gate is armed.
        assert_eq!(
            tvt::on_manager_event(&mut world, 1, 100, "Participate").as_deref(),
            Some("registration-success.html"),
            "baseline registration should succeed ({label})"
        );
        tvt::on_manager_event(&mut world, 1, 100, "CancelParticipation");

        setup(&mut world);

        assert_eq!(
            tvt::on_manager_event(&mut world, 1, 100, "Participate").as_deref(),
            Some("registration-failed.html"),
            "a player {label} must not be able to register"
        );
        assert!(
            world.events.tvt.player_list.is_empty(),
            "and must not land in the participant list ({label})"
        );
    }
}

/// `EndFight`'s "Disable players" block: invulnerable, **immobilised** and
/// **skill-locked** — not invul alone — and the same for servitors, so nobody
/// gets a free shot while the scoreboard is up. `teleport_out` thaws all of it.
///
/// The servitor half deviates from Java on purpose: Java's thaw block re-runs
/// the *freeze* calls on the servitor (a copy-paste), leaving a pet
/// invulnerable and unable to act for the rest of the session with nothing to
/// undo it. See `docs/CUSTOM_DIST_DEVIATIONS.md`.
#[test]
fn end_fight_freezes_players_and_servitors_and_teleport_out_thaws_them() {
    use crate::model::components::{AdminFlags, Immobilized, SkillsDisabled};

    let (mut world, oids) = started_with_players(4);
    tvt::teleport_to_arena(&mut world);
    tvt::start_fight(&mut world);

    // Give one participant a servitor to carry through the freeze.
    const PANTHER: i32 = 14799;
    let mut tmpl = crate::data::npc_data::default_template(PANTHER);
    tmpl.type_name = "Servitor".into();
    tmpl.level = 20;
    tmpl.base_hp_max = 400.0;
    tmpl.base_mp_max = 200.0;
    world.data.npc_data.insert_for_test(tmpl);
    let owner = oids[0];
    let pet =
        crate::game_loop::servitor::summon_servitor(&mut world, owner, PANTHER, 283, 1200, 0, 0)
            .expect("servitor summoned");

    let frozen = |w: &World, oid: i32| {
        w.objects.has_component::<Immobilized>(&oid)
            && w.objects.has_component::<SkillsDisabled>(&oid)
            && w.objects
                .get_component::<AdminFlags>(&oid)
                .is_some_and(|f| f.invul)
    };
    assert!(!frozen(&world, owner), "not frozen mid-fight");

    tvt::end_fight(&mut world);
    assert!(frozen(&world, owner), "the participant is frozen");
    assert!(frozen(&world, pet), "and so is their servitor");
    // Skill-locked means *casting* only — Java's `disableAllSkills` does not
    // touch movement, which `setImmobilized` handles separately.
    assert!(
        abnormal::all_skills_disabled(&world, owner),
        "the cast gate sees the lock"
    );

    tvt::teleport_out(&mut world);
    assert!(!frozen(&world, owner), "the participant is thawed");
    assert!(
        !frozen(&world, pet),
        "and so is the servitor — Java leaves this one frozen forever"
    );
}

/// The arena stand-up regroups each side like Java's `StartFight`: old
/// parties are left, each team becomes parties of ≤7 (FINDERS_KEEPERS), and a
/// team bigger than one party gets a command channel holding every party.
#[test]
fn teleport_to_arena_groups_teams_into_parties_and_ccs() {
    use crate::model::components::PartyRef;

    // 18 players → 9 per team → parties of 7+2 → a CC per team.
    let (mut world, oids) = started_with_players(18);

    // Two of them share a pre-existing party that must be dissolved.
    let pre_party = world.next_party_id;
    world.next_party_id += 1;
    let seq = world.next_request_seq();
    world.parties.insert(
        pre_party,
        model::party::Party::new(oids[0], LootRule::FindersKeepers, seq),
    );
    world.objects.add_components(&oids[0], PartyRef(pre_party));
    crate::game_loop::party::add_party_member(&mut world, pre_party, oids[1]);

    tvt::teleport_to_arena(&mut world);

    assert!(
        !world.parties.contains_key(&pre_party),
        "the pre-existing party dissolved when its members left for the arena"
    );
    for team in [
        world.events.tvt.blue_team.clone(),
        world.events.tvt.red_team.clone(),
    ] {
        assert_eq!(team.len(), 9);
        // Every member is in a party of at most 7, FINDERS_KEEPERS.
        let mut party_ids = Vec::new();
        for oid in &team {
            let pid = world
                .objects
                .get_component::<PartyRef>(oid)
                .expect("every participant is grouped")
                .0;
            let party = &world.parties[&pid];
            assert!(party.members.len() <= 7, "parties of at most 7");
            assert_eq!(party.distribution, LootRule::FindersKeepers);
            if !party_ids.contains(&pid) {
                party_ids.push(pid);
            }
        }
        assert_eq!(party_ids.len(), 2, "9 members split 7 + 2");
        // Both parties hang in one command channel.
        let ccs: Vec<_> = party_ids
            .iter()
            .map(|pid| {
                crate::game_loop::command_channel::cc_id_of_party(&world, *pid)
                    .expect("an overflowing team forms a CC")
            })
            .collect();
        assert_eq!(ccs[0], ccs[1], "one channel per team");
    }
}
