//! Vitality (G16): the pool's clamp/notify behaviour, the kill-cost formula,
//! the exp/sp bonus multiplier it feeds, and the premium reward rates.

use super::*;

use crate::game_loop::vitality;
use crate::model::{MAX_VITALITY_POINTS, MIN_VITALITY_POINTS};
use crate::network::server_packets::sm_ids;

const CID: u32 = 1;
const OID: i32 = 268_500_001;

/// A world with vitality switched on, matching this dist's `Character.ini`.
fn vitality_world() -> (
    World,
    db::CmdTx,
    db::CmdRx,
    UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, tx, rx, link) = test_world();
    // `for_test` ships an empty experience table, which caps exp at -1; give
    // the world a real ladder so the reward assertions mean something.
    world.data.experience = crate::data::ExperienceData::from_table(
        vec![
            0, 0, 1000, 5000, 20_000, 100_000, 500_000, 2_000_000, 10_000_000,
        ],
        8,
    );
    world.cfg.character.enable_vitality = true;
    world.cfg.rates.rate_vitality_exp_multiplier = 2.0;
    world.cfg.rates.rate_vitality_gain = 1.0;
    world.cfg.rates.rate_vitality_lost = 1.0;
    (world, tx, rx, link)
}

fn set_points(world: &mut World, oid: i32, points: i32) {
    world
        .objects
        .get_component_mut::<Player>(&oid)
        .unwrap()
        .vitality_points = points;
}

fn points(world: &World, oid: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&oid)
        .unwrap()
        .vitality_points
}

// ---------------------------------------------------------------------------
// The pool itself (`PlayerStat.setVitalityPoints` / `updateVitalityPoints`)
// ---------------------------------------------------------------------------

/// Setting the pool clamps to `0..=140_000` and announces the direction of the
/// change plus the "at maximum" edge line.
#[test]
fn set_clamps_and_announces() {
    let (mut world, _tx, _rx, _l) = vitality_world();
    let mut out = ingame_player(&mut world, CID, OID, 0, 0, 0);
    set_points(&mut world, OID, 1000);
    drain(&mut out);

    // Way over the cap: clamped, and the "at maximum" line fires.
    assert!(vitality::set_vitality_points(
        &mut world, OID, 999_999, false
    ));
    assert_eq!(points(&world, OID), MAX_VITALITY_POINTS);
    let packets = drain(&mut out);
    assert!(has_sm(&packets, sm_ids::YOUR_VITALITY_HAS_INCREASED));
    assert!(has_sm(&packets, sm_ids::YOUR_VITALITY_IS_AT_MAXIMUM));

    // Down to empty: only the exhausted line. Java would also send
    // `YOUR_VITALITY_HAS_DECREASED` here; it is suppressed on purpose (it would
    // fire on nearly every monster kill), so this pins its absence.
    assert!(vitality::set_vitality_points(&mut world, OID, -50, false));
    assert_eq!(points(&world, OID), MIN_VITALITY_POINTS);
    let packets = drain(&mut out);
    assert!(!has_sm(&packets, sm_ids::YOUR_VITALITY_HAS_DECREASED));
    assert!(has_sm(&packets, sm_ids::YOUR_VITALITY_IS_FULLY_EXHAUSTED));
}

/// A plain drain that doesn't reach zero is now completely silent — no
/// decrease line, and no edge line either. This is the case that mattered:
/// every monster kill calls `updateVitalityPoints` with a negative delta.
#[test]
fn ordinary_drain_sends_no_system_message() {
    let (mut world, _tx, _rx, _l) = vitality_world();
    let mut out = ingame_player(&mut world, CID, OID, 0, 0, 0);
    set_points(&mut world, OID, 50_000);
    drain(&mut out);

    assert!(vitality::set_vitality_points(
        &mut world, OID, 49_000, false
    ));
    let packets = drain(&mut out);
    assert!(!has_sm(&packets, sm_ids::YOUR_VITALITY_HAS_DECREASED));
    assert!(!has_sm(&packets, sm_ids::YOUR_VITALITY_IS_FULLY_EXHAUSTED));
    assert!(!has_sm(&packets, sm_ids::YOUR_VITALITY_IS_AT_MAXIMUM));
    // The gauge still updates, so the client shows the drain.
    assert!(
        packets
            .iter()
            .any(|p| p[0] == 0xFE && u16::from_le_bytes([p[1], p[2]]) == 0xA1),
        "expected ExVitalityPointInfo"
    );
}

/// `quiet = true` (what `//set_vitality` uses) suppresses the system messages
/// but still pushes the gauge packet — Java sends `ExVitalityPointInfo`
/// outside the quiet guard.
#[test]
fn quiet_set_skips_messages_but_updates_gauge() {
    let (mut world, _tx, _rx, _l) = vitality_world();
    let mut out = ingame_player(&mut world, CID, OID, 0, 0, 0);
    drain(&mut out);

    vitality::set_vitality_points(&mut world, OID, 5000, true);
    let packets = drain(&mut out);
    assert_eq!(points(&world, OID), 5000);
    assert!(!has_sm(&packets, sm_ids::YOUR_VITALITY_HAS_INCREASED));
    // 0xFE extended packet 0xA1 = ExVitalityPointInfo.
    assert!(
        packets
            .iter()
            .any(|p| p[0] == 0xFE && u16::from_le_bytes([p[1], p[2]]) == 0xA1),
        "expected ExVitalityPointInfo"
    );
}

/// A no-op set (same value) returns false and sends nothing.
#[test]
fn set_to_same_value_is_a_no_op() {
    let (mut world, _tx, _rx, _l) = vitality_world();
    let mut out = ingame_player(&mut world, CID, OID, 0, 0, 0);
    set_points(&mut world, OID, 4242);
    drain(&mut out);

    assert!(!vitality::set_vitality_points(&mut world, OID, 4242, false));
    assert!(drain(&mut out).is_empty());
}

/// `updateVitalityPoints` applies a signed delta and floors at 0 rather than
/// going negative.
#[test]
fn update_applies_delta_and_floors_at_zero() {
    let (mut world, _tx, _rx, _l) = vitality_world();
    let mut out = ingame_player(&mut world, CID, OID, 0, 0, 0);
    set_points(&mut world, OID, 500);
    drain(&mut out);

    vitality::update_vitality_points(&mut world, OID, -200, true, true);
    assert_eq!(points(&world, OID), 300);

    // Overdraw: floored, not negative.
    vitality::update_vitality_points(&mut world, OID, -9999, true, true);
    assert_eq!(points(&world, OID), MIN_VITALITY_POINTS);
}

/// With `EnableVitality = False` the whole update path is inert (Java's first
/// guard in `updateVitalityPoints`).
#[test]
fn update_is_inert_when_the_system_is_off() {
    let (mut world, _tx, _rx, _l) = vitality_world();
    world.cfg.character.enable_vitality = false;
    let _out = ingame_player(&mut world, CID, OID, 0, 0, 0);
    set_points(&mut world, OID, 500);

    assert!(!vitality::update_vitality_points(
        &mut world, OID, -200, true, true
    ));
    assert_eq!(points(&world, OID), 500);
}

/// `RateVitalityLost` scales a consumption; `RateVitalityGain` a restore.
#[test]
fn gain_and_lost_rates_scale_the_delta() {
    let (mut world, _tx, _rx, _l) = vitality_world();
    world.cfg.rates.rate_vitality_lost = 3.0;
    world.cfg.rates.rate_vitality_gain = 10.0;
    let _out = ingame_player(&mut world, CID, OID, 0, 0, 0);
    set_points(&mut world, OID, 1000);

    vitality::update_vitality_points(&mut world, OID, -100, true, true);
    assert_eq!(
        points(&world, OID),
        700,
        "-100 at RateVitalityLost=3 spends 300"
    );

    vitality::update_vitality_points(&mut world, OID, 30, true, true);
    assert_eq!(
        points(&world, OID),
        1000,
        "+30 at RateVitalityGain=10 restores 300"
    );

    // `useRates = false` bypasses both.
    vitality::update_vitality_points(&mut world, OID, -100, false, true);
    assert_eq!(points(&world, OID), 900);
}

// ---------------------------------------------------------------------------
// The exp/sp bonus (`getVitalityExpBonus` / `getExpBonusMultiplier`)
// ---------------------------------------------------------------------------

/// Any remaining point buys the full `RateVitalityExpMultiplier`; an empty
/// pool buys nothing. Java's test is `getVitalityPoints() > 0`, not a ratio.
#[test]
fn exp_bonus_is_all_or_nothing() {
    let (mut world, _tx, _rx, _l) = vitality_world();
    let _out = ingame_player(&mut world, CID, OID, 0, 0, 0);

    set_points(&mut world, OID, 1);
    assert_eq!(vitality::vitality_exp_bonus(&world, OID), 2.0);
    assert_eq!(vitality::exp_bonus_multiplier(&world, OID), 2.0);

    set_points(&mut world, OID, 0);
    assert_eq!(vitality::vitality_exp_bonus(&world, OID), 1.0);
    assert_eq!(vitality::exp_bonus_multiplier(&world, OID), 1.0);
}

/// The multiplier reaches the reward: the same kill pays double while vitality
/// lasts, and the acquisition message reports the surplus in its bonus slot.
#[test]
fn vitality_doubles_awarded_exp_and_reports_the_bonus() {
    let (mut world, _tx, _rx, _l) = vitality_world();
    let mut out = ingame_player(&mut world, CID, OID, 0, 0, 0);
    set_points(&mut world, OID, 10_000);
    drain(&mut out);

    let before = world.objects.get_component::<Player>(&OID).unwrap().exp;
    crate::game_loop::death::add_exp_and_sp(&mut world, OID, 1000.0, 100.0, true);
    let gained = world.objects.get_component::<Player>(&OID).unwrap().exp - before;
    assert_eq!(gained, 2000, "×2 vitality multiplier");

    // SM params: [finalExp, expBonus, finalSp, spBonus] — the bonus half is
    // the surplus over base, i.e. 1000 exp / 100 sp.
    let packets = drain(&mut out);
    assert!(has_sm(
        &packets,
        sm_ids::YOU_HAVE_ACQUIRED_S1_XP_BONUS_S2_AND_S3_SP_BONUS_S4
    ));

    // Drained pool: the same call now pays base only.
    set_points(&mut world, OID, 0);
    let before = world.objects.get_component::<Player>(&OID).unwrap().exp;
    crate::game_loop::death::add_exp_and_sp(&mut world, OID, 1000.0, 100.0, true);
    let gained = world.objects.get_component::<Player>(&OID).unwrap().exp - before;
    assert_eq!(gained, 1000);
}

/// `use_bonuses = false` — the quest / `//add_exp_sp` overload — never applies
/// the multiplier, even on a full pool.
#[test]
fn quest_rewards_skip_the_vitality_bonus() {
    let (mut world, _tx, _rx, _l) = vitality_world();
    let mut out = ingame_player(&mut world, CID, OID, 0, 0, 0);
    set_points(&mut world, OID, MAX_VITALITY_POINTS);
    drain(&mut out);

    let before = world.objects.get_component::<Player>(&OID).unwrap().exp;
    crate::game_loop::death::add_exp_and_sp(&mut world, OID, 1000.0, 0.0, false);
    assert_eq!(
        world.objects.get_component::<Player>(&OID).unwrap().exp - before,
        1000
    );
}

// ---------------------------------------------------------------------------
// The kill cost (`Attackable.getVitalityPoints`)
// ---------------------------------------------------------------------------

/// Below level 85 the divisor is a hard-coded 1000 and the level gap is
/// floored at 1: `-(exp / 1000 * max(playerLvl - npcLvl, 1))`.
#[test]
fn kill_cost_uses_the_sub_85_formula() {
    let (world, _tx, _rx, _l) = vitality_world();

    // exp 5000, same level → gap floors to 1 → -(5000/1000 * 1) = -5.
    assert_eq!(
        vitality::kill_vitality_delta(&world, 20, 100.0, 20, 5000.0, false),
        -5
    );
    // Killer 5 levels above → gap 5 → -25.
    assert_eq!(
        vitality::kill_vitality_delta(&world, 20, 100.0, 25, 5000.0, false),
        -25
    );
    // Killer *below* the mob still floors the gap at 1.
    assert_eq!(
        vitality::kill_vitality_delta(&world, 40, 100.0, 20, 5000.0, false),
        -5
    );
}

/// Any positive-exp kill costs at least one point (Java's `Math.max(…, 1)`),
/// and a mob with no level or no exp reward costs nothing.
#[test]
fn kill_cost_floors_at_one_and_skips_worthless_mobs() {
    let (world, _tx, _rx, _l) = vitality_world();

    // Tiny exp truncates to 0 → floored to 1 point.
    assert_eq!(
        vitality::kill_vitality_delta(&world, 10, 100.0, 10, 1.0, false),
        -1
    );
    // No exp reward on the template, or a level-less NPC: no change at all.
    assert_eq!(
        vitality::kill_vitality_delta(&world, 10, 0.0, 10, 500.0, false),
        0
    );
    assert_eq!(
        vitality::kill_vitality_delta(&world, 0, 100.0, 10, 500.0, false),
        0
    );
}

/// A real kill drains the killer's pool through the full `calculate_rewards`
/// path, and the drained pool then stops doubling the reward.
#[test]
fn killing_a_monster_drains_vitality() {
    let (mut world, _tx, _rx, _l) = vitality_world();
    let mut out = ingame_player(&mut world, CID, OID, 0, 0, 0);
    set_points(&mut world, OID, 10_000);
    drain(&mut out);

    // Register the template first, with a real exp reward — the kill is only
    // worth vitality when `getExpReward() > 0`.
    let mut t = crate::data::npc_data::default_template(20001);
    t.type_name = "Monster".into();
    t.level = 10;
    t.base_hp_max = 100.0;
    t.base_mp_max = 50.0;
    t.exp = 5000.0;
    t.sp = 100.0;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 10, 0, 0, 0);

    // Rewards are shared by *damage dealt*, so the killer needs an aggro-list
    // entry — the real path fills this in from `Attackable.addDamage`.
    add_hate(&mut world, NPC_OID, OID, 100.0, 100.0);
    world
        .objects
        .get_component_mut::<Vitals>(&NPC_OID)
        .unwrap()
        .cur_hp = 1.0;
    crate::game_loop::death::npc_do_die(&mut world, NPC_OID, OID);

    assert!(
        points(&world, OID) < 10_000,
        "the kill should have spent vitality"
    );
}
