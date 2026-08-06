//! The automatic weekly siege schedule (G24 slice 1).

use super::*;

use crate::data::siege_data::{SiegeScheduleEntry, load_siege_schedule};
use crate::game_loop::siege::next_siege_millis;
use crate::model::castle::{Castle, CastleSide};
use crate::model::siege::Siege;
use crate::scheduler::ScheduledTask;

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
const DAY_MS: i64 = 86_400_000;
const HOUR_MS: i64 = 3_600_000;

fn weekday_of(millis: i64) -> u32 {
    (millis.div_euclid(DAY_MS) + 3).rem_euclid(7) as u32
}

/// `next_siege_millis` lands strictly in the future, on the target weekday, at
/// `hour`:00 UTC, within a week.
#[test]
fn next_siege_is_the_next_matching_weekday_and_hour() {
    // A fixed reference: 1970-01-01 00:00 UTC was a Thursday (weekday 3).
    let thursday_midnight = 0i64;
    assert_eq!(weekday_of(thursday_midnight), 3, "epoch is Thursday");

    for now in [
        0i64,
        5 * DAY_MS + 3 * HOUR_MS,
        123_456_789_000,
        1_700_000_000_000,
    ] {
        for weekday in 0..7u32 {
            for hour in [0u32, 16, 20, 23] {
                let at = next_siege_millis(now, weekday, hour);
                assert!(at > now, "strictly future: now={now} -> {at}");
                assert_eq!(weekday_of(at), weekday, "on the target weekday");
                assert_eq!(at.rem_euclid(DAY_MS), hour as i64 * HOUR_MS, "at hour:00");
                assert!(at - now <= 7 * DAY_MS, "within a week");
            }
        }
    }
}

/// A slot earlier *today* rolls to next week, not today.
#[test]
fn a_passed_slot_today_rolls_a_week_forward() {
    // Epoch is Thursday(3) 00:00. Asking for Thursday@0 must skip to next week.
    let at = next_siege_millis(0, 3, 0);
    assert_eq!(at, 7 * DAY_MS, "next Thursday, not today");
    // Thursday@16 the same day is still ahead → today.
    let at = next_siege_millis(0, 3, 16);
    assert_eq!(at, 16 * HOUR_MS, "later today");
}

/// The dist schedule loads: all nine castles, Sunday, enabled, hours 16/20.
#[test]
fn the_dist_schedule_loads_all_nine_castles() {
    let sched = load_siege_schedule(DIST);
    assert_eq!(sched.len(), 9, "nine castles");
    for id in 1..=9 {
        let e = sched.get(&id).unwrap_or_else(|| panic!("castle {id}"));
        assert_eq!(e.weekday, 6, "Sunday");
        assert!(e.enabled);
        assert!(e.hour == 16 || e.hour == 20, "hour {}", e.hour);
    }
    // Gludio 16:00, Dion 20:00 (transcribed from the file).
    assert_eq!(sched.get(&1).unwrap().hour, 16);
    assert_eq!(sched.get(&2).unwrap().hour, 20);
}

fn schedule_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    world.castles = vec![
        Castle {
            show_npc_crest: false,
            id: 1,
            name: "Gludio".into(),
            side: CastleSide::Neutral,
            ticket_buy_count: 0,
            first_mid_victory: false,
            time_registration_over: true,
            siege_time_registration_end: 0,
            siege_date: 0,
            treasury: 0,
        },
        Castle {
            show_npc_crest: false,
            id: 2,
            name: "Dion".into(),
            side: CastleSide::Neutral,
            ticket_buy_count: 0,
            first_mid_victory: false,
            time_registration_over: true,
            siege_time_registration_end: 0,
            siege_date: 0,
            treasury: 0,
        },
    ];
    world.sieges.insert(1, Siege::new(1));
    world.sieges.insert(2, Siege::new(2));
    world.data.siege_schedule.insert(
        1,
        SiegeScheduleEntry {
            weekday: 6,
            hour: 16,
            enabled: true,
        },
    );
    // Castle 2 is disabled — it must not be armed.
    world.data.siege_schedule.insert(
        2,
        SiegeScheduleEntry {
            weekday: 6,
            hour: 20,
            enabled: false,
        },
    );
    (world, db, l)
}

fn pending_siege_starts(world: &World) -> usize {
    world
        .scheduler
        .pending_tasks_for_test()
        .iter()
        .filter(|t| matches!(t, ScheduledTask::SiegeStart { .. }))
        .count()
}

/// Boot arms exactly one auto-task per *enabled* castle (the disabled one is
/// skipped), and a hop days out **re-arms rather than starting**.
///
/// This replaced an assertion that the first firing started the siege outright.
/// That was the old one-shot design, and it is exactly what made the owner's
/// chosen hour unreachable: a task armed against the fixed schedule could never
/// notice a date set after it was armed.
#[test]
fn boot_arms_enabled_castles_and_a_distant_hop_only_re_arms() {
    let (mut world, _db, _l) = schedule_world();

    crate::game_loop::siege::schedule_all_at_boot(&mut world);
    assert_eq!(
        pending_siege_starts(&world),
        1,
        "only the enabled castle is armed"
    );

    // (Calling the handler directly leaves the boot-armed task in the heap —
    // production drains it first — so measure the *delta* each firing adds.)
    assert!(!world.sieges[&1].in_progress);
    let before = pending_siege_starts(&world);
    let now = commons::util::now_millis();
    crate::game_loop::siege::run_auto_task(&mut world, 1, now);
    assert!(
        !world.sieges[&1].in_progress,
        "a hop days out re-arms, it does not start the siege"
    );
    assert_eq!(
        pending_siege_starts(&world) - before,
        1,
        "and it arms exactly one next hop"
    );
}

/// The ladder walks down Java's rungs and only then starts. Driven with an
/// explicit clock: each step sets "now" to the rung distance and checks the
/// chain has not fired early.
#[test]
fn the_auto_task_ladder_starts_only_once_the_date_passes() {
    let (mut world, _db, _l) = schedule_world();
    let siege_at = crate::game_loop::siege::next_siege_millis(commons::util::now_millis(), 6, 16);

    // Every rung above zero must re-arm without starting. 13_600_000 is Java's
    // registration-close rung — its comment claims "1 hr" but the literal is
    // 3 h 46 m 40 s, and the value is what retail runs on.
    for remaining in [
        2 * DAY_MS,
        DAY_MS - 1,
        13_600_000 - 1,
        600_000 - 1,
        300_000 - 1,
        10_000 - 1,
    ] {
        let (mut w, _d, _l) = schedule_world();
        w.sieges.insert(1, crate::model::siege::Siege::new(1));
        // The stored date is what the chain reads; boot stamps it in
        // production, and this test drives `run_auto_task` directly.
        w.castles.iter_mut().find(|c| c.id == 1).unwrap().siege_date = siege_at;
        crate::game_loop::siege::run_auto_task(&mut w, 1, siege_at - remaining);
        assert!(
            !w.sieges[&1].in_progress,
            "{remaining} ms out: re-arm, never start"
        );
        assert_eq!(
            pending_siege_starts(&w),
            1,
            "{remaining} ms out: exactly one next hop armed"
        );
    }

    // Once the moment has passed, it starts.
    world
        .castles
        .iter_mut()
        .find(|c| c.id == 1)
        .unwrap()
        .siege_date = siege_at;
    crate::game_loop::siege::run_auto_task(&mut world, 1, siege_at + 1);
    assert!(
        world.sieges[&1].in_progress,
        "the siege begins once its date is behind us"
    );
}

/// **The feature.** An hour chosen by the castle owner *after* the chain was
/// armed is honored, because every hop re-reads the date.
///
/// Under the old one-shot timer the task was pinned to the `SiegeSchedule.xml`
/// hour at arming time, so a later choice changed the SiegeInfo window and the
/// registration cut-off but never the moment the siege actually began.
#[test]
fn an_hour_chosen_after_arming_is_honoured() {
    let (mut world, _db, _l) = schedule_world();
    let now = commons::util::now_millis();
    let fixed = crate::game_loop::siege::next_siege_millis(now, 6, 16);

    crate::game_loop::siege::schedule_all_at_boot(&mut world);

    // The owner picks the *other* dist hour, 20:00 — four hours after the
    // fixed slot the chain was armed against. Derived from `fixed` rather than
    // from a second `next_siege_millis(now, 6, 20)`: between 16:00 and 20:00 on
    // a Sunday that call returns *today* 20:00 while `fixed` has already rolled
    // to next week, so the "later hour" the test needs would be earlier.
    let chosen = fixed + 4 * 60 * 60 * 1000;
    world
        .castles
        .iter_mut()
        .find(|c| c.id == 1)
        .unwrap()
        .siege_date = chosen;

    // At the old fixed moment the siege must NOT begin — the chain re-reads and
    // sees the chosen date still ahead.
    crate::game_loop::siege::run_auto_task(&mut world, 1, fixed + 1);
    assert!(
        !world.sieges[&1].in_progress,
        "the fixed hour no longer starts the siege once an hour was chosen"
    );

    // At the chosen moment it does.
    crate::game_loop::siege::run_auto_task(&mut world, 1, chosen + 1);
    assert!(
        world.sieges[&1].in_progress,
        "the siege begins at the hour the owner chose"
    );
}

/// A siege ending reopens the owner's hour-picking window for 24 h (Java
/// `saveCastleSiege`). Without this half the window never opens on its own —
/// `regTimeOver` defaults to `true` — and hour-picking stays dormant.
#[test]
fn ending_a_siege_reopens_the_hour_picking_window() {
    let (mut world, _db, _l) = schedule_world();
    world
        .castles
        .iter_mut()
        .find(|c| c.id == 1)
        .unwrap()
        .time_registration_over = true;
    world.sieges.get_mut(&1).unwrap().in_progress = true;

    let before = commons::util::now_millis();
    crate::game_loop::siege::end_siege(&mut world, 1);

    let c = world.castles.iter().find(|c| c.id == 1).unwrap();
    assert!(
        !c.time_registration_over,
        "the owner may pick the next siege's hour again"
    );
    let window = c.siege_time_registration_end - before;
    assert!(
        (DAY_MS - 5_000..=DAY_MS + 5_000).contains(&window),
        "the window is 24 h (got {window} ms)"
    );
}

/// `ExShowCastleInfo` reports live ownership rather than the static
/// all-unowned list it sent until 2026-08-05.
///
/// The overlay is what the world map draws, so every field is a visible claim:
/// who holds the castle, what it taxes, when it is next besieged, and whether
/// a siege is running right now.
#[test]
fn castle_info_overlay_carries_owner_tax_and_siege() {
    use crate::network::server_packets;

    let (mut world, ..) = test_world();
    world.castles = vec![
        Castle {
            show_npc_crest: false,
            id: 1,
            name: "Gludio".into(),
            side: CastleSide::Dark,
            ticket_buy_count: 0,
            first_mid_victory: false,
            time_registration_over: true,
            siege_time_registration_end: 0,
            siege_date: 1_700_000_000_000,
            treasury: 0,
        },
        Castle {
            show_npc_crest: false,
            id: 2,
            name: "Dion".into(),
            side: CastleSide::Neutral,
            ticket_buy_count: 0,
            first_mid_victory: false,
            time_registration_over: true,
            siege_time_registration_end: 0,
            siege_date: 0,
            treasury: 0,
        },
    ];
    // Gludio is held; Dion is not. A castle with no owning clan must still
    // occupy its slot with an empty name, or every field after it shifts.
    world.clans.insert(10, owning_clan(10, 1));
    world.cfg.feature.castle_buy_tax_dark = 8;
    world.cfg.feature.castle_buy_tax_neutral = 15;
    let mut siege = Siege::new(1);
    siege.in_progress = true;
    world.sieges.insert(1, siege);

    let pkt = server_packets::ex_show_castle_info(&world);
    let mut r = commons::network::PacketReader::new(&pkt[3..]);
    assert_eq!(r.read_i32().unwrap(), 2, "one entry per castle in the list");

    assert_eq!(r.read_i32().unwrap(), 1, "Gludio id");
    assert_eq!(r.read_string().unwrap(), "Clan10", "owner clan name");
    assert_eq!(r.read_i32().unwrap(), 8, "buy tax follows the DARK side");
    assert_eq!(
        r.read_i32().unwrap(),
        1_700_000_000,
        "siege date in SECONDS, not the millis it is stored as"
    );
    assert_eq!(r.read_u8().unwrap(), 1, "siege in progress");
    assert_eq!(r.read_u8().unwrap(), 2, "CastleSide::Dark ordinal");

    assert_eq!(r.read_i32().unwrap(), 2, "Dion id");
    assert_eq!(r.read_string().unwrap(), "", "unowned writes an empty name");
    assert_eq!(r.read_i32().unwrap(), 15, "NEUTRAL tax");
    assert_eq!(r.read_i32().unwrap(), 0, "no siege scheduled");
    assert_eq!(r.read_u8().unwrap(), 0, "no siege running");
    assert_eq!(r.read_u8().unwrap(), 0, "CastleSide::Neutral ordinal");
}

/// A minimal clan holding `castle_id` — enough for the overlay's owner lookup.
fn owning_clan(id: i32, castle_id: i32) -> crate::model::clan::Clan {
    crate::model::clan::Clan {
        id,
        name: format!("Clan{id}"),
        leader_id: id * 10,
        level: 5,
        reputation_score: 0,
        castle_id,
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
        blood_alliance_count: 0,
    }
}

// ---------------------------------------------------------------------------
// The siege-zone fame task
// ---------------------------------------------------------------------------

/// `SiegeZone.onEnter` → `startFameTask`, `FameTask.run`, `stopFameTask`.
///
/// Everything about this is inert on the shipped dist, where
/// `CastleZoneFameAquirePoints = 0` — so the test raises the amount, which is
/// exactly the operator change the port has to keep working.
#[test]
fn a_participant_standing_in_the_siege_zone_earns_fame_until_they_leave() {
    use crate::model::siege::{Siege, SiegeClanType};
    use crate::network::server_packets::sm_ids;

    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    const POS: (i32, i32, i32) = (-17964, 110730, -1000);
    const PLAYER: i32 = 6101;
    const CASTLE: i32 = 1;
    const CID: u32 = 1;

    let build = || {
        let (mut world, _db, _l) = combat_test_world();
        world.data.zone_data = crate::data::zone_data::ZoneData::load_from(DIST);
        world.cfg.character.castle_zone_fame_task_frequency = 300;
        world.cfg.character.castle_zone_fame_acquire_points = 125;
        world.cfg.character.fame_for_dead_players = false;
        let rx = ingame_caster(&mut world, CID, PLAYER, POS.0, POS.1);
        world
            .objects
            .get_component_mut::<crate::model::components::Position>(&PLAYER)
            .unwrap()
            .z = POS.2;
        world
            .objects
            .get_component_mut::<crate::model::Player>(&PLAYER)
            .unwrap()
            .clan_id = 700;
        let mut siege = Siege::new(CASTLE);
        siege.in_progress = true;
        siege.add_clan(700, SiegeClanType::Attacker);
        world.sieges.insert(CASTLE, siege);
        crate::game_loop::zones::revalidate_zone(&mut world, PLAYER, true);
        (world, rx)
    };

    let fame = |w: &World| {
        w.objects
            .get_component::<crate::model::Player>(&PLAYER)
            .unwrap()
            .fame
    };

    // Entering the zone arms the task; it pays on the configured cadence.
    let (mut world, mut rx) = build();
    assert!(
        world.siege_fame_armed.contains(&PLAYER),
        "standing in the zone as a participant arms the task"
    );
    assert_eq!(fame(&world), 0, "and pays nothing before the first tick");
    drain(&mut rx);
    advance_world(&mut world, 3001);
    assert_eq!(fame(&world), 125, "one payment after the frequency elapses");
    assert!(has_system_message(
        &drain(&mut rx),
        sm_ids::YOU_HAVE_ACQUIRED_S1_FAME
    ));
    // It re-arms, so a second period pays again.
    advance_world(&mut world, 3001);
    assert_eq!(fame(&world), 250, "and it keeps ticking");

    // Walking out ends it — the task notices at its next firing and stops
    // re-arming, which is this port's stand-in for `stopFameTask()`.
    let (mut world, _rx) = build();
    world
        .objects
        .get_component_mut::<crate::model::components::Position>(&PLAYER)
        .unwrap()
        .x = 0;
    advance_world(&mut world, 3001);
    assert_eq!(fame(&world), 0, "no pay outside the zone");
    assert!(
        !world.siege_fame_armed.contains(&PLAYER),
        "and the task is not re-armed"
    );

    // A corpse in the zone is skipped while `FameForDeadPlayers` is off, but
    // the task keeps running — Java only skips the payment, not the task.
    let (mut world, _rx) = build();
    world
        .objects
        .get_component_mut::<crate::model::components::Vitals>(&PLAYER)
        .unwrap()
        .dead = true;
    advance_world(&mut world, 3001);
    assert_eq!(fame(&world), 0, "a corpse earns nothing…");
    world
        .objects
        .get_component_mut::<crate::model::components::Vitals>(&PLAYER)
        .unwrap()
        .dead = false;
    advance_world(&mut world, 3001);
    assert_eq!(fame(&world), 125, "…but earns again once it stands up");
}
