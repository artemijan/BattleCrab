//! `PcCafePointsManager` — earning PA points (G16).
//!
//! The store and the `//pccafepoints` GM command already existed; these cover
//! the two earning modes and the guards that decide which (if either) runs.

use super::*;

use crate::game_loop::character::pc_cafe;
use crate::model::Player;
use crate::network::server_packets::sm_ids;

const PLAYER: i32 = 6001;
const CID: u32 = 1;

/// The dist's own configuration, then whatever the test needs on top.
fn enable(world: &mut World, retail_like: bool) {
    world.cfg.premium.pc_cafe_enabled = true;
    world.cfg.premium.pc_cafe_retail_like = retail_like;
    world.cfg.premium.pc_cafe_only_premium = false;
    world.cfg.premium.pc_cafe_enable_double_points = false;
    world.cfg.premium.pc_cafe_random_point = false;
    world.cfg.premium.pc_cafe_reward_low_exp_kills = false;
}

fn points(world: &World) -> i32 {
    pc_cafe::points_of(world, PLAYER)
}

// ---------------------------------------------------------------------------
// The retail-like timer
// ---------------------------------------------------------------------------

/// `run` arms a fixed-rate task that pays `AcquisitionPointsRetailLikePoints`
/// every `PcCafeRewardTime` and re-arms itself, so a player logged in for three
/// periods has been paid three times.
#[test]
fn the_retail_timer_pays_on_every_period_and_keeps_going() {
    let (mut world, _db, _l) = cast_test_world();
    let mut out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    enable(&mut world, true);
    // 5 s instead of the dist's 5 min, so the test can watch three periods.
    world.cfg.premium.pc_cafe_reward_time = 5_000;

    pc_cafe::on_enter_world(&mut world, PLAYER);
    drain(&mut out);
    assert_eq!(points(&world), 0, "nothing before the first period elapses");

    advance_ticks(&mut world, 50);
    assert_eq!(points(&world), 10, "one period, one flat award");
    let msgs = drain(&mut out);
    assert!(
        has_system_message(&msgs, sm_ids::YOU_EARNED_S1_PA_POINT_S),
        "the single-point message, not the double one"
    );
    assert!(
        has_opcode(&msgs, server_packets::opcodes::EX),
        "and an ExPCCafePointInfo to refresh the counter"
    );

    advance_ticks(&mut world, 100);
    assert_eq!(points(&world), 30, "the task re-arms itself");
}

/// A second `run` (Java re-runs it on every premium purchase) re-stamps the
/// generation, so the older schedule goes stale rather than stacking a second
/// payout timer on the same player.
#[test]
fn re_arming_replaces_the_timer_rather_than_doubling_it() {
    let (mut world, _db, _l) = cast_test_world();
    let mut out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    enable(&mut world, true);
    world.cfg.premium.pc_cafe_reward_time = 5_000;

    pc_cafe::run(&mut world, PLAYER);
    advance_ticks(&mut world, 10);
    pc_cafe::run(&mut world, PLAYER);
    drain(&mut out);

    // Far enough for both schedules to have fired had they both survived.
    advance_ticks(&mut world, 100);
    assert_eq!(
        points(&world),
        20,
        "two periods' worth from one timer, not four from two"
    );
}

/// Nothing is armed at all while the system is off, or while it is in
/// exp-proportional mode — `run`'s own two guards.
#[test]
fn the_timer_is_not_armed_when_disabled_or_not_retail_like() {
    for (enabled, retail) in [(false, true), (true, false)] {
        let (mut world, _db, _l) = cast_test_world();
        let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
        enable(&mut world, retail);
        world.cfg.premium.pc_cafe_enabled = enabled;
        world.cfg.premium.pc_cafe_reward_time = 5_000;

        pc_cafe::on_enter_world(&mut world, PLAYER);
        // Nothing is even *scheduled* — `give_retail_point` re-checks the same
        // flags, so a payout assertion alone would pass with `run`'s guard
        // deleted.
        assert!(
            !world
                .scheduler
                .pending_tasks_for_test()
                .iter()
                .any(|t| matches!(t, crate::scheduler::ScheduledTask::PcCafeReward { .. })),
            "enabled={enabled} retail_like={retail} arms no task"
        );
        advance_ticks(&mut world, 200);
        assert_eq!(
            points(&world),
            0,
            "enabled={enabled} retail_like={retail} pays nothing"
        );
    }
}

/// The timer is armed by the **real login path**, not just by calling the
/// manager — `EnterWorld.runImpl`'s `PcCafePointsManager.run(player)`.
#[test]
fn logging_in_arms_the_timer() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = entering_player(&mut world, CID, PLAYER, 0, 0, 0);
    world.cfg.premium.pc_cafe_enabled = true;
    world.cfg.premium.pc_cafe_retail_like = true;

    handle_enter_world(&mut world, CID);
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .iter()
            .any(|t| matches!(
                t,
                crate::scheduler::ScheduledTask::PcCafeReward { player_object_id, .. }
                    if *player_object_id == PLAYER
            )),
        "enter-world armed the PA-point timer"
    );
}

/// `PcCafeOnlyPremium` (True on this dist) gates the award, not the timer: the
/// task still fires, it just pays nothing until the account is premium.
#[test]
fn only_premium_gates_the_award() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    enable(&mut world, true);
    world.cfg.premium.pc_cafe_reward_time = 5_000;
    world.cfg.premium.pc_cafe_only_premium = true;
    world.cfg.premium.enabled = true;

    pc_cafe::on_enter_world(&mut world, PLAYER);
    advance_ticks(&mut world, 60);
    assert_eq!(points(&world), 0, "no premium, no points");

    // Grant premium to this character's account; the *same* timer now pays.
    let account = world
        .objects
        .get_component::<Player>(&PLAYER)
        .unwrap()
        .account
        .clone();
    crate::game_loop::admin::premium::add_premium_time(
        &mut world,
        &account,
        30 * crate::game_loop::admin::premium::DAY_MILLIS,
    );
    advance_ticks(&mut world, 60);
    assert_eq!(points(&world), 10);
}

// ---------------------------------------------------------------------------
// The exp-proportional award
// ---------------------------------------------------------------------------

/// `points = exp * 0.0001 * rate`, truncated. 100 000 exp at the dist's rate
/// of 1.0 is 10 points.
#[test]
fn the_exp_award_is_a_ten_thousandth_of_the_exp() {
    let (mut world, _db, _l) = cast_test_world();
    let mut out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    enable(&mut world, false);

    pc_cafe::give_point(&mut world, PLAYER, 100_000.0);
    assert_eq!(points(&world), 10);

    // `AcquisitionPointsRate` scales it.
    world.cfg.premium.pc_cafe_point_rate = 2.5;
    pc_cafe::give_point(&mut world, PLAYER, 100_000.0);
    assert_eq!(points(&world), 35);

    // **Java's else branch sends the DOUBLE-points message** — a copy-paste slip
    // in `givePcCafePoint` that `giveRetailPcCafePont` does not share. Ported as
    // written.
    let msgs = drain(&mut out);
    assert!(has_system_message(
        &msgs,
        sm_ids::DOUBLE_POINTS_YOU_EARNED_S1_PA_POINT_S
    ));
    assert!(
        !has_system_message(&msgs, sm_ids::YOU_EARNED_S1_PA_POINT_S),
        "the correct single-point string never appears on this path"
    );
}

/// **The two modes are mutually exclusive.** `givePcCafePoint`'s very first
/// guard is `PC_CAFE_RETAIL_LIKE`, so on this dist's configuration (retail-like
/// on) no amount of killing earns anything.
#[test]
fn the_exp_award_is_dead_while_retail_like_is_on() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    enable(&mut world, true);

    pc_cafe::give_point(&mut world, PLAYER, 10_000_000.0);
    assert_eq!(points(&world), 0);
}

/// No points from a peace, PVP or siege zone, and none while jailed.
#[test]
fn the_exp_award_is_refused_in_the_guarded_states() {
    use crate::data::zone_data::ZoneKind;
    use crate::model::components::ZoneFlags;

    for kind in [ZoneKind::Peace, ZoneKind::Pvp, ZoneKind::Siege] {
        let (mut world, _db, _l) = cast_test_world();
        let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
        enable(&mut world, false);
        if let Some(f) = world.objects.get_component_mut::<ZoneFlags>(&PLAYER) {
            f.mask |= kind.bit();
        }
        pc_cafe::give_point(&mut world, PLAYER, 100_000.0);
        assert_eq!(points(&world), 0, "{kind:?} pays nothing");
    }

    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    enable(&mut world, false);
    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .jailed = true;
    pc_cafe::give_point(&mut world, PLAYER, 100_000.0);
    assert_eq!(points(&world), 0, "a jailed player earns nothing");
}

/// A kill too small to be worth a point still pays 1, `RewardLowExpKillsChance`
/// percent of the time — and only when there *was* exp.
#[test]
fn a_low_exp_kill_can_still_pay_one_point() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    enable(&mut world, false);
    world.cfg.premium.pc_cafe_reward_low_exp_kills = true;
    world.cfg.premium.pc_cafe_low_exp_kills_chance = 50;

    // A winning roll (49 < 50), then a losing one (50 is not < 50).
    world.force_rolls([49]);
    pc_cafe::give_point(&mut world, PLAYER, 500.0);
    assert_eq!(points(&world), 1);

    world.force_rolls([50]);
    pc_cafe::give_point(&mut world, PLAYER, 500.0);
    assert_eq!(points(&world), 1, "the losing roll pays nothing");

    // Zero exp is not a low-exp kill, it is no kill — no roll, no point.
    pc_cafe::give_point(&mut world, PLAYER, 0.0);
    assert_eq!(points(&world), 1);
}

// ---------------------------------------------------------------------------
// The shared tail
// ---------------------------------------------------------------------------

/// `DoublingAcquisitionPoints` (True on this dist) doubles the award and
/// switches the message, on a `DoublingAcquisitionPointsChance` percent roll.
#[test]
fn double_points_doubles_the_award_and_the_message() {
    let (mut world, _db, _l) = cast_test_world();
    let mut out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    enable(&mut world, true);
    world.cfg.premium.pc_cafe_enable_double_points = true;
    world.cfg.premium.pc_cafe_double_points_chance = 10;

    world.force_rolls([9]); // 9 < 10 → doubled
    pc_cafe::give_retail_point(&mut world, PLAYER);
    assert_eq!(points(&world), 20);
    let msgs = drain(&mut out);
    assert!(has_system_message(
        &msgs,
        sm_ids::DOUBLE_POINTS_YOU_EARNED_S1_PA_POINT_S
    ));

    world.force_rolls([10]); // not < 10 → plain
    pc_cafe::give_retail_point(&mut world, PLAYER);
    assert_eq!(points(&world), 30);
    let msgs = drain(&mut out);
    assert!(has_system_message(&msgs, sm_ids::YOU_EARNED_S1_PA_POINT_S));
}

/// The balance is clamped to `MaxPcCafePoints`, and the message reports the
/// *clamped* amount (Java calls `addLong` after the clamp).
#[test]
fn the_award_is_clamped_to_the_ceiling() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    enable(&mut world, true);
    world.cfg.premium.pc_cafe_max_points = 105;
    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .pccafe_points = 100;

    pc_cafe::give_retail_point(&mut world, PLAYER);
    assert_eq!(points(&world), 105, "10 offered, 5 taken");
    pc_cafe::give_retail_point(&mut world, PLAYER);
    assert_eq!(points(&world), 105, "already full");
}

/// **The retail-like max check compares the award to the ceiling, not the
/// balance** — `if (points >= Config.PC_CAFE_MAX_POINTS)`. Reproduced: a
/// ceiling below the award refuses outright with the "maximum" message even for
/// a player holding nothing, while the exp path's check (which *does* read the
/// balance) lets the same player earn.
#[test]
fn the_retail_max_check_reads_the_award_not_the_balance() {
    let (mut world, _db, _l) = cast_test_world();
    let mut out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    enable(&mut world, true);
    world.cfg.premium.pc_cafe_max_points = 5; // below the flat award of 10

    pc_cafe::give_retail_point(&mut world, PLAYER);
    assert_eq!(points(&world), 0, "refused although the balance is empty");
    assert!(has_system_message(
        &drain(&mut out),
        sm_ids::YOU_HAVE_EARNED_THE_MAXIMUM_NUMBER_OF_PA_POINTS
    ));

    // The exp path's guard is the correct one, and lets the same player earn.
    enable(&mut world, false);
    world.cfg.premium.pc_cafe_max_points = 5;
    pc_cafe::give_point(&mut world, PLAYER, 30_000.0);
    assert_eq!(points(&world), 3);
}

// ---------------------------------------------------------------------------
// Call sites
// ---------------------------------------------------------------------------

/// **The gate: a real kill pays PA points.** Java awards them beside
/// `updateVitalityPoints` in `Attackable.onKill` — but *outside* the
/// vitality-enabled guard, so the points must survive `EnableVitality = False`
/// too. That is exactly the placement bug this call site invites.
#[test]
fn a_real_kill_pays_pa_points_even_with_vitality_off() {
    const NPC_OID: i32 = 6100;

    for vitality_on in [true, false] {
        let (mut world, _db, _l) = cast_test_world();
        let mut out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
        enable(&mut world, false);
        world.cfg.character.enable_vitality = vitality_on;
        drain(&mut out);

        let mut t = crate::data::npc_data::default_template(20001);
        t.type_name = "Monster".into();
        t.level = 10;
        t.base_hp_max = 100.0;
        t.exp = 1_000_000.0;
        t.sp = 100.0;
        world.data.npc_data.insert_for_test(t);
        add_test_npc(&mut world, NPC_OID, 20001, "Monster", 10, 0, 0, 0);
        add_hate(&mut world, NPC_OID, PLAYER, 100.0, 100.0);
        world
            .objects
            .get_component_mut::<Vitals>(&NPC_OID)
            .unwrap()
            .cur_hp = 1.0;
        crate::game_loop::npc::npc_do_die(&mut world, NPC_OID, PLAYER);

        assert!(
            points(&world) > 0,
            "vitality on={vitality_on}: the kill still pays PA points"
        );
    }
}

/// The block is read from the dist's own `Custom/PremiumSystem.ini` — the file
/// Java's `CUSTOM_PREMIUM_SYSTEM_CONFIG_FILE` points at. (`Custom/PcCafe.ini`
/// also exists on this dist and is **read by nothing**: no Java constant names
/// it, so its `PcCafeEnabled = True` is inert and the authoritative answer is
/// PremiumSystem.ini's `False`.)
#[test]
fn the_pc_cafe_block_is_read_from_the_dist_ini() {
    let cfg = crate::config::premium::PremiumConfig::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    assert!(!cfg.pc_cafe_enabled, "PcCafeEnabled = False");
    assert!(cfg.pc_cafe_retail_like, "PcCafeRetailLike = True");
    assert!(cfg.pc_cafe_only_premium, "PcCafeOnlyPremium = True");
    // Java declares PC_CAFE_REWARD_TIME and never assigns it; the ini's value
    // is the specification, so the port reads it.
    assert_eq!(cfg.pc_cafe_reward_time, 300_000);
    assert_eq!(cfg.pc_cafe_max_points, 200_000);
    assert_eq!(cfg.acquisition_pc_cafe_retail_like_points, 10);
    assert!((cfg.pc_cafe_point_rate - 1.0).abs() < 1e-9);
    assert!(!cfg.pc_cafe_random_point);
    assert!(
        cfg.pc_cafe_enable_double_points,
        "DoublingAcquisitionPoints"
    );
    assert_eq!(cfg.pc_cafe_double_points_chance, 1);
    assert!(cfg.pc_cafe_reward_low_exp_kills);
    assert_eq!(cfg.pc_cafe_low_exp_kills_chance, 50);
}

/// Java's two out-of-range rules differ, and both are Java's rather than
/// tidy-ups: the doubling chance **falls back to 1** outside 0..=100, while the
/// low-exp chance is **clamped** to the bound. A negative max is clamped to 0
/// and a negative rate resets to 1.
#[test]
fn the_out_of_range_rules_differ_between_the_two_chances() {
    let dir = std::env::temp_dir().join("l2r_pc_cafe_cfg_test");
    let ini = dir.join("config/Custom/PremiumSystem.ini");
    std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
    std::fs::write(
        &ini,
        "DoublingAcquisitionPointsChance = 500\n\
         RewardLowExpKillsChance = 500\n\
         MaxPcCafePoints = -7\n\
         AcquisitionPointsRate = -2.0\n",
    )
    .unwrap();

    let cfg = crate::config::premium::PremiumConfig::load_from(&format!("{}/", dir.display()));
    assert_eq!(cfg.pc_cafe_double_points_chance, 1, "reset, not clamped");
    assert_eq!(cfg.pc_cafe_low_exp_kills_chance, 100, "clamped, not reset");
    assert_eq!(cfg.pc_cafe_max_points, 0);
    assert!((cfg.pc_cafe_point_rate - 1.0).abs() < 1e-9);

    std::fs::remove_dir_all(&dir).ok();
}

/// The other two `run` call sites: buying premium on the community board, and
/// a GM granting it with `//premium_add`. Both exist because
/// `PcCafeOnlyPremium` may only now be satisfied — the timer has to be
/// (re-)armed at the moment the account becomes eligible.
#[test]
fn buying_premium_arms_the_timer_on_both_paths() {
    let armed = |world: &World, oid: i32| {
        world.scheduler.pending_tasks_for_test().iter().any(|t| {
            matches!(
                t,
                crate::scheduler::ScheduledTask::PcCafeReward { player_object_id, .. }
                    if *player_object_id == oid
            )
        })
    };

    // --- community board `_bbspremium` ---
    let (mut world, _db, _l) = cast_test_world();
    let mut out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    world.cfg.premium.enabled = true;
    world.cfg.premium.pc_cafe_enabled = true;
    world.cfg.premium.pc_cafe_retail_like = true;
    world.cfg.community_board.enabled = true;
    world.cfg.community_board.premium_price_per_day = 0;
    drain(&mut out);
    assert!(!armed(&world, PLAYER), "nothing armed yet");

    crate::game_loop::community_board::handle_parse_command(&mut world, CID, "_bbspremium;7");
    assert!(armed(&world, PLAYER), "the board purchase armed the timer");

    // --- `//premium_add <account>` (the GM is the account's own character
    // here, which is also the "online player on that account" Java looks up) ---
    // `admin_world` loads the real AccessLevels/AdminCommands tables, without
    // which `use_admin_command`'s `isGM()` gate refuses silently.
    let (mut world, _tx, _db, _l) = admin_world();
    let _out = ingame_player_access(&mut world, CID, PLAYER, 100);
    world.cfg.premium.enabled = true;
    world.cfg.premium.pc_cafe_enabled = true;
    world.cfg.premium.pc_cafe_retail_like = true;
    let account = world
        .objects
        .get_component::<Player>(&PLAYER)
        .unwrap()
        .account
        .clone();
    assert!(!armed(&world, PLAYER));

    crate::game_loop::admin::use_admin_command(
        &mut world,
        CID,
        &format!("admin_premium_add1 {account}"),
        false,
    );
    assert!(armed(&world, PLAYER), "the GM grant armed it too");
}

/// A **party** kill pays each rewarded member from their own post-cutoff share
/// (Java `Party.distributeXpAndSp`), not the party total — so the two members
/// of an even split each get half the points a solo killer would.
#[test]
fn a_party_kill_pays_each_member_from_their_own_share() {
    const OTHER: i32 = 6002;

    let (mut world, _db, _l) = cast_test_world();
    let _a = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let _b = ingame_caster(&mut world, 2, OTHER, 0, 0);
    enable(&mut world, false);

    let mut t = crate::data::npc_data::default_template(20002);
    t.type_name = "Monster".into();
    t.level = 10;
    world.data.npc_data.insert_for_test(t.clone());

    // Two same-level members split 200 000 exp, plus the party bonus ladder.
    crate::game_loop::party::distribute_xp_and_sp(
        &mut world,
        &[(PLAYER, 10), (OTHER, 10)],
        10,
        200_000.0,
        0.0,
        &t,
        true, // not a champion kill → vitality/PA points apply as before
    );

    let a = pc_cafe::points_of(&world, PLAYER);
    let b = pc_cafe::points_of(&world, OTHER);
    assert_eq!(a, b, "an even split pays both members alike");
    assert!(
        (10..20).contains(&a),
        "each member's share, not the party total: {a}"
    );
}
