//! Daily task manager (G33 slice 1): the vitality daily/weekly refill and the
//! self-rescheduling daily reset.

use super::*;

use crate::db::DbCommand;
use crate::game_loop::{daily_tasks, vitality};
use crate::model::{MAX_VITALITY_POINTS, Player};
use crate::scheduler::ScheduledTask;

fn set_vitality(world: &mut World, oid: i32, points: i32) {
    world
        .objects
        .get_component_mut::<Player>(&oid)
        .unwrap()
        .vitality_points = points;
}

fn vitality_of(world: &World, oid: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&oid)
        .unwrap()
        .vitality_points
}

#[test]
fn daily_refill_adds_a_quarter_and_updates_offline_rows() {
    let (mut world, _tx, mut db_rx, _link) = test_world();
    world.cfg.character.enable_vitality = true;
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    set_vitality(&mut world, 3001, 50_000);
    drain_db(&mut db_rx);

    vitality::reset_vitality(&mut world, false);

    assert_eq!(
        vitality_of(&world, 3001),
        50_000 + MAX_VITALITY_POINTS / 4,
        "online pool gains MAX/4"
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, DbCommand::ResetVitality { weekly: false })),
        "offline daily reset queued"
    );
}

#[test]
fn daily_refill_clamps_at_the_maximum() {
    let (mut world, ..) = test_world();
    world.cfg.character.enable_vitality = true;
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    set_vitality(&mut world, 3001, MAX_VITALITY_POINTS - 1000);

    vitality::reset_vitality(&mut world, false);
    assert_eq!(vitality_of(&world, 3001), MAX_VITALITY_POINTS, "capped");
}

#[test]
fn weekly_refill_sets_the_pool_to_full() {
    let (mut world, _tx, mut db_rx, _link) = test_world();
    world.cfg.character.enable_vitality = true;
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    set_vitality(&mut world, 3001, 1_000);
    drain_db(&mut db_rx);

    vitality::reset_vitality(&mut world, true);

    assert_eq!(
        vitality_of(&world, 3001),
        MAX_VITALITY_POINTS,
        "full refill"
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, DbCommand::ResetVitality { weekly: true }))
    );
}

#[test]
fn a_disabled_vitality_system_does_not_refill() {
    let (mut world, _tx, mut db_rx, _link) = test_world();
    world.cfg.character.enable_vitality = false;
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    set_vitality(&mut world, 3001, 10_000);
    drain_db(&mut db_rx);

    vitality::reset_vitality(&mut world, false);
    assert_eq!(vitality_of(&world, 3001), 10_000, "unchanged");
    assert!(
        drain_db(&mut db_rx).is_empty(),
        "no DB reset when vitality is off"
    );
}

#[test]
fn the_daily_reset_runs_both_sub_resets_and_reschedules_itself() {
    let (mut world, _tx, mut db_rx, _link) = test_world();
    world.cfg.character.enable_vitality = true;
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    set_vitality(&mut world, 3001, 20_000);
    drain_db(&mut db_rx);

    daily_tasks::handle_daily_reset(&mut world);

    // Vitality moved (its sub-reset ran) and both offline UPDATEs were queued.
    assert!(vitality_of(&world, 3001) > 20_000, "vitality refilled");
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter()
            .any(|c| matches!(c, DbCommand::ResetVitality { .. }))
    );
    assert!(cmds.iter().any(|c| matches!(c, DbCommand::ResetRecommends)));
    // The task re-armed itself for the next day.
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .iter()
            .any(|t| matches!(t, ScheduledTask::DailyReset))
    );
}

/// A clan with a stamped `new_leader_id` (the village master's delegated
/// transfer) hands leadership over at the **Wednesday** reset — Java
/// `DailyTaskManager.clanLeaderApply`, which sits in the weekly branch beside
/// the full vitality refill, not in the daily one.
#[test]
fn a_pending_clan_leader_transfer_applies_on_the_weekly_reset() {
    let (mut world, _tx, _db_rx, _link) = test_world();
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    let clan_id = pending_transfer_clan(&mut world, 3001, 3002);

    // A non-Wednesday reset leaves the stamp alone.
    daily_tasks::run_reset(&mut world, false);
    assert_eq!(world.clans[&clan_id].leader_id, 3001, "daily reset: no-op");
    assert_eq!(world.clans[&clan_id].new_leader_id, 3002, "stamp survives");

    // Wednesday applies it and clears the stamp.
    daily_tasks::run_reset(&mut world, true);
    assert_eq!(world.clans[&clan_id].leader_id, 3002, "leadership moved");
    assert_eq!(world.clans[&clan_id].new_leader_id, 0, "stamp cleared");
    assert!(
        world
            .objects
            .get_component::<Player>(&3002)
            .unwrap()
            .clan_leader,
        "the new leader's flag is set"
    );
}

/// Java skips a clan whose nominee is no longer a member (`getClanMember` →
/// `continue`) — the stamp stays, so the transfer simply never fires.
#[test]
fn a_transfer_to_a_departed_member_is_skipped_not_cleared() {
    let (mut world, _tx, _db_rx, _link) = test_world();
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let clan_id = pending_transfer_clan(&mut world, 3001, 3002);
    // The nominee leaves the clan before Wednesday.
    world
        .clans
        .get_mut(&clan_id)
        .unwrap()
        .members
        .retain(|m| m.char_id != 3002);

    daily_tasks::run_reset(&mut world, true);

    assert_eq!(
        world.clans[&clan_id].leader_id, 3001,
        "leadership unchanged"
    );
    assert_eq!(
        world.clans[&clan_id].new_leader_id, 3002,
        "the stamp is left in place, exactly as Java's `continue` leaves it"
    );
}

/// A clan carrying a pending transfer, leader `leader` and nominee `nominee`.
fn pending_transfer_clan(world: &mut World, leader: i32, nominee: i32) -> i32 {
    let clan_id = 0x4100_0001;
    let member = |char_id: i32| model::clan::ClanMember {
        char_id,
        name: format!("P{char_id}"),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
        power_grade: 5,
        title: String::new(),
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
    };
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "Pending".into(),
            leader_id: leader,
            level: 5,
            reputation_score: 0,
            castle_id: 0,
            members: vec![member(leader), member(nominee)],
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: nominee,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    for oid in [leader, nominee] {
        if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
            p.clan_id = clan_id;
            p.clan_leader = oid == leader;
        }
    }
    clan_id
}
