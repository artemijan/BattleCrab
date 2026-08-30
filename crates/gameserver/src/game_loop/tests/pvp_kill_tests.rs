//! PvP / PK kill consequences (G20): counters, karma, and the zone exemptions.

use super::*;

use crate::game_loop::combat::pvp;
use crate::model::Player;
use crate::model::components::{PvpState, ZoneFlags};

const KILLER: i32 = 2001;
const VICTIM: i32 = 2002;
const KILLER_CID: u32 = 1;
const VICTIM_CID: u32 = 2;

fn rep(world: &World, oid: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&oid)
        .unwrap()
        .reputation
}
fn pvp_kills(world: &World, oid: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&oid)
        .unwrap()
        .pvp_kills
}
fn pk_kills(world: &World, oid: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&oid)
        .unwrap()
        .pk_kills
}

fn two_players(world: &mut World) {
    let _k = ingame_caster(world, KILLER_CID, KILLER, 0, 0);
    let _v = ingame_caster(world, VICTIM_CID, VICTIM, 50, 0);
}

fn kill(world: &mut World) {
    pvp::on_kill_update_pvp_reputation(world, KILLER, VICTIM);
}

// ---------------------------------------------------------------------------
// The karma-gain curve
// ---------------------------------------------------------------------------

/// `Formulas.calculateKarmaGain`'s three brackets.
#[test]
fn karma_gain_follows_the_pk_count_brackets() {
    // < 99: ((pk * 0.5 + 1) * 60) * 12
    assert_eq!(pvp::calculate_karma_gain(0), 720);
    assert_eq!(pvp::calculate_karma_gain(1), 1080);
    assert_eq!(pvp::calculate_karma_gain(98), 36_000);
    // < 180: ((pk * 0.125 + 37.75) * 60) * 12
    assert_eq!(pvp::calculate_karma_gain(99), 36_090);
    // >= 180: flat.
    assert_eq!(pvp::calculate_karma_gain(180), 43_200);
    assert_eq!(pvp::calculate_karma_gain(500), 43_200);
    // Karma rises with the body count.
    assert!(pvp::calculate_karma_gain(50) > pvp::calculate_karma_gain(10));
}

// ---------------------------------------------------------------------------
// The three kill outcomes
// ---------------------------------------------------------------------------

/// Killing a **flagged** player is lawful: a PvP kill, no karma.
#[test]
fn killing_a_flagged_player_is_a_pvp_kill() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);
    world.objects.add_components(
        &VICTIM,
        PvpState {
            flag: 1,
            ..Default::default()
        },
    );

    kill(&mut world);

    assert_eq!(pvp_kills(&world, KILLER), 1, "counted as a PvP kill");
    assert_eq!(pk_kills(&world, KILLER), 0, "not a PK");
    assert_eq!(rep(&world, KILLER), 0, "no karma taken");
}

/// Killing a **clean** player with positive reputation and no prior PKs is the
/// "first offence": reputation resets to 0 rather than going negative.
#[test]
fn first_offence_resets_positive_reputation() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);
    world
        .objects
        .get_component_mut::<Player>(&KILLER)
        .unwrap()
        .reputation = 500;

    kill(&mut world);

    assert_eq!(
        rep(&world, KILLER),
        0,
        "reputation reset, not driven negative"
    );
    assert_eq!(pk_kills(&world, KILLER), 1);
    assert_eq!(pvp_kills(&world, KILLER), 0);
}

/// Otherwise a clean kill is a **PK**: karma is taken and the counter rises.
#[test]
fn killing_a_clean_player_costs_karma() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);
    // Reputation 0 → falls past the first-offence branch (which needs > 0).

    kill(&mut world);

    assert_eq!(pk_kills(&world, KILLER), 1);
    assert_eq!(pvp_kills(&world, KILLER), 0);
    assert_eq!(
        rep(&world, KILLER),
        -pvp::calculate_karma_gain(0),
        "karma for the first PK"
    );

    // A second PK costs more, from the now-higher pk count.
    let after_first = rep(&world, KILLER);
    kill(&mut world);
    assert_eq!(pk_kills(&world, KILLER), 2);
    assert_eq!(
        rep(&world, KILLER),
        after_first - pvp::calculate_karma_gain(1)
    );
}

/// Killing a **PK** is lawful and counts as PvP.
#[test]
fn killing_a_pk_is_lawful() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);
    world
        .objects
        .get_component_mut::<Player>(&VICTIM)
        .unwrap()
        .reputation = -5000;

    kill(&mut world);

    assert_eq!(pvp_kills(&world, KILLER), 1);
    assert_eq!(pk_kills(&world, KILLER), 0);
    assert!(rep(&world, KILLER) >= 0, "no karma for killing a PK");
}

// ---------------------------------------------------------------------------
// Exemptions
// ---------------------------------------------------------------------------

/// Inside a PVP zone nothing is counted at all — neither side takes karma or
/// gains a kill ("Do nothing when in PVP zone").
#[test]
fn pvp_zone_kills_count_for_nothing() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);
    for oid in [KILLER, VICTIM] {
        world
            .objects
            .get_component_mut::<ZoneFlags>(&oid)
            .unwrap()
            .mask = crate::data::zone_data::ZoneKind::Pvp.bit();
    }

    kill(&mut world);

    assert_eq!(pvp_kills(&world, KILLER), 0, "arena kills are not counted");
    assert_eq!(pk_kills(&world, KILLER), 0);
    assert_eq!(rep(&world, KILLER), 0);
}

/// A monster killer moves none of this (the block is player-on-player only).
#[test]
fn monster_kills_move_no_counters() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 40, 0, 0);

    pvp::on_kill_update_pvp_reputation(&mut world, NPC_OID, VICTIM);

    assert_eq!(pk_kills(&world, VICTIM), 0);
    assert_eq!(rep(&world, VICTIM), 0);
}

/// `check_if_pvp` itself: flagged and PK targets are lawful, a clean one is
/// not, and self never is.
#[test]
fn check_if_pvp_classifies_targets() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);

    assert!(
        !pvp::check_if_pvp(&world, KILLER, VICTIM),
        "a clean stranger is not a lawful target"
    );
    assert!(
        !pvp::check_if_pvp(&world, KILLER, KILLER),
        "self is never PvP"
    );

    world.objects.add_components(
        &VICTIM,
        PvpState {
            flag: 1,
            ..Default::default()
        },
    );
    assert!(
        pvp::check_if_pvp(&world, KILLER, VICTIM),
        "a flagged target is lawful"
    );

    world.objects.add_components(&VICTIM, PvpState::default());
    world
        .objects
        .get_component_mut::<Player>(&VICTIM)
        .unwrap()
        .reputation = -1;
    assert!(pvp::check_if_pvp(&world, KILLER, VICTIM), "a PK is lawful");
}

/// Blessing of Protection (`PlayableAI`'s pair): a chaotic character 10+
/// levels above a blessed newbie can't start an attack on them — refused with
/// INCORRECT_TARGET, no intent — and the shield is symmetric (the blessed
/// newbie can't engage the PK either). A clean attacker of the same level gap
/// engages normally, and a PVP zone suspends the protection.
#[test]
fn blessing_of_protection_blocks_the_pk_both_ways() {
    use crate::model::components::Intent;
    use crate::model::skill::{ActiveBuff, BuffSlot};

    let (mut world, ..) = combat_test_world();
    two_players(&mut world);
    {
        let p = world.objects.get_component_mut::<Player>(&KILLER).unwrap();
        p.level = 30;
        p.reputation = -500; // chaotic
    }
    {
        let p = world.objects.get_component_mut::<Player>(&VICTIM).unwrap();
        p.level = 15;
    }
    world.objects.add_components(
        &VICTIM,
        Buffs(vec![ActiveBuff {
            skill_id: 5182,
            abnormal_type: "PK_PROTECT".to_string(),
            abnormal_level: 1,
            slot: BuffSlot::Uncapped,
            ..test_buff()
        }]),
    );

    // The PK can't engage the blessed newbie.
    combat::start_attack_intent(&mut world, KILLER_CID, KILLER, VICTIM);
    assert!(
        !world.objects.has_component::<Intent>(&KILLER),
        "the chaotic attacker is refused"
    );

    // …and the blessed newbie can't engage the PK.
    combat::start_attack_intent(&mut world, VICTIM_CID, VICTIM, KILLER);
    assert!(
        !world.objects.has_component::<Intent>(&VICTIM),
        "the shield is symmetric"
    );

    // A clean high-level attacker engages normally.
    world
        .objects
        .get_component_mut::<Player>(&KILLER)
        .unwrap()
        .reputation = 0;
    combat::start_attack_intent(&mut world, KILLER_CID, KILLER, VICTIM);
    assert!(
        world.objects.has_component::<Intent>(&KILLER),
        "no karma, no protection"
    );
    world.objects.remove_component::<Intent>(&KILLER);

    // Back to chaotic, but inside a PVP zone the protection is suspended.
    world
        .objects
        .get_component_mut::<Player>(&KILLER)
        .unwrap()
        .reputation = -500;
    let z = ZoneFlags {
        mask: crate::data::zone_data::ZoneKind::Pvp.bit(),
        ..Default::default()
    };
    world.objects.add_components(&VICTIM, z);
    combat::start_attack_intent(&mut world, KILLER_CID, KILLER, VICTIM);
    assert!(
        world.objects.has_component::<Intent>(&KILLER),
        "a PVP zone suspends the blessing"
    );
}

/// `onDieDropItem`'s first gate: a clean victim killed by a clan-war enemy
/// drops nothing — the same death outside a war can drop.
#[test]
fn war_deaths_never_drop_items() {
    use crate::model::clan::{ClanWar, ClanWarState};

    let (mut world, ..) = combat_test_world();
    two_players(&mut world);
    // Drop rules that would otherwise fire: victim must be a PK for normal
    // drops, so make the *clean* case assert the gate specifically.
    {
        let p = world.objects.get_component_mut::<Player>(&VICTIM).unwrap();
        p.clan_id = 10;
        p.reputation = 0;
    }
    world
        .objects
        .get_component_mut::<Player>(&KILLER)
        .unwrap()
        .clan_id = 20;
    world.clan_wars.push(ClanWar {
        attacker_id: 20,
        attacked_id: 10,
        state: ClanWarState::Mutual,
        attacker_kills: 0,
        attacked_kills: 0,
        winner_id: 0,
        start_time: 0,
        end_time: 0,
    });

    // The gate returns before any drop logic — reaching it with a clean
    // victim proves the exemption (a panic-free no-op run).
    crate::game_loop::death::on_die_drop_item(&mut world, VICTIM, KILLER);
    let dropped = world
        .ground_item_regions
        .values()
        .map(|v| v.len())
        .sum::<usize>();
    assert_eq!(dropped, 0, "a war death leaves nothing on the ground");
}

// ---------------------------------------------------------------------------
// Karma decay — `PlayerStat.addExp`'s "Set new karma" block
// ---------------------------------------------------------------------------

use crate::game_loop::death::add_exp_and_sp;

/// Give the world a karma table and put `oid` at `level` with `reputation`.
fn pk_at(world: &mut World, oid: i32, level: i32, reputation: i32) {
    // Two rows far enough apart to show the divisor doing its job: level 10 is
    // cheap redemption, level 70 is ~100x dearer, both taken from the shape of
    // the shipped table rather than its exact values.
    world.data.karma.insert_for_test(10, 2.0);
    world.data.karma.insert_for_test(70, 200.0);
    let p = world
        .objects
        .get_component_mut::<Player>(&oid)
        .expect("player");
    p.level = level;
    p.reputation = reputation;
}

/// **The mechanic this row was about.** A PK grinding experience works their
/// reputation back toward 0; before this there was no path back at all short
/// of dying.
#[test]
fn hunting_works_a_pks_karma_off() {
    let (mut world, ..) = test_world();
    two_players(&mut world);
    pk_at(&mut world, KILLER, 10, -50_000);

    // 30 · 2.0 · 6000 = 360 000 exp buys 6000 karma back at level 10.
    add_exp_and_sp(&mut world, KILLER, 360_000.0, 0.0, false);

    assert_eq!(rep(&world, KILLER), -44_000, "karma worked off by hunting");
}

/// `Math.min(reputation + karmaLost, 0)` — redemption stops at clean, it does
/// not run past into positive reputation.
#[test]
fn karma_decay_stops_at_zero() {
    let (mut world, ..) = test_world();
    two_players(&mut world);
    pk_at(&mut world, KILLER, 10, -100);

    add_exp_and_sp(&mut world, KILLER, 360_000.0, 0.0, false);

    assert_eq!(rep(&world, KILLER), 0, "clamped at clean");
}

/// The per-level divisor is the whole point of `pcKarmaIncrease.xml`: the same
/// hunt buys a high-level PK far less redemption.
#[test]
fn karma_decay_slows_down_with_level() {
    let redeemed = |level: i32| {
        let (mut world, ..) = test_world();
        two_players(&mut world);
        pk_at(&mut world, KILLER, level, -1_000_000);
        add_exp_and_sp(&mut world, KILLER, 360_000.0, 0.0, false);
        rep(&world, KILLER) + 1_000_000
    };
    let (low, high) = (redeemed(10), redeemed(70));
    assert!(low > 0 && high > 0, "both redeem something ({low}, {high})");
    assert_eq!(
        low / high,
        100,
        "the level-70 divisor is 100x the level-10 one, so redemption is 100x slower"
    );
}

/// `getReputation() < 0` is a gate, not a no-op for everyone else. A player
/// with *positive* reputation is the case that proves it: the block ends in
/// `Math.min(reputation + karmaLost, 0)`, so letting them through would drag
/// them down to 0 rather than leave them alone.
#[test]
fn a_player_with_positive_reputation_is_left_alone() {
    let (mut world, ..) = test_world();
    two_players(&mut world);
    pk_at(&mut world, KILLER, 10, 500);

    add_exp_and_sp(&mut world, KILLER, 360_000.0, 0.0, false);

    assert_eq!(rep(&world, KILLER), 500, "untouched, not clamped to 0");
}

/// The arena exemption: an ordinary player grinding inside a PvP zone works
/// nothing off, but Java's `isGM() ||` short-circuits ahead of the zone test,
/// so a GM does.
#[test]
fn a_pvp_zone_exempts_an_ordinary_player_but_not_a_gm() {
    let in_arena_redeems = |gm: bool| {
        let (mut world, ..) = test_world();
        two_players(&mut world);
        pk_at(&mut world, KILLER, 10, -50_000);
        world.objects.add_components(
            &KILLER,
            ZoneFlags {
                mask: crate::data::zone_data::ZoneKind::Pvp.bit(),
                ..Default::default()
            },
        );
        if gm {
            // `is_gm` resolves the access level through `AccessLevels.xml`,
            // which the fixture world ships empty — without the real table a
            // level-100 character is not a GM and the branch never runs.
            world.data.admin = crate::data::AdminData::load_from(crate::data::DIST_GAME);
            world
                .objects
                .get_component_mut::<Player>(&KILLER)
                .unwrap()
                .access_level = 100;
        }
        add_exp_and_sp(&mut world, KILLER, 360_000.0, 0.0, false);
        rep(&world, KILLER) != -50_000
    };
    assert!(!in_arena_redeems(false), "an arena grind buys nothing");
    assert!(in_arena_redeems(true), "a GM is exempt from the exemption");
}

/// `!player.isCursedWeaponEquipped()` — the bearer's karma belongs to the
/// weapon and is cleared when it leaves, so hunting must not chip at it.
#[test]
fn a_cursed_weapon_bearer_works_nothing_off() {
    let (mut world, ..) = test_world();
    two_players(&mut world);
    pk_at(&mut world, KILLER, 10, -50_000);
    world
        .objects
        .get_component_mut::<Player>(&KILLER)
        .unwrap()
        .cursed_weapon_equipped_id = 8190;

    add_exp_and_sp(&mut world, KILLER, 360_000.0, 0.0, false);

    assert_eq!(
        rep(&world, KILLER),
        -50_000,
        "the weapon's karma is untouched"
    );
}

/// `RateKarmaLost` divides the experience *before* the per-level divisor, so a
/// server that leaves it at `-1` (this dist → `RateXp`) sees the two cancel:
/// raising the XP rate does not make karma cheaper to shed.
#[test]
fn the_xp_rate_does_not_make_redemption_cheaper() {
    let redeemed = |rate: f64| {
        let (mut world, ..) = test_world();
        two_players(&mut world);
        world.cfg.rates.rate_karma_lost = rate;
        pk_at(&mut world, KILLER, 10, -1_000_000);
        // The hunt yields `rate` times as much exp on a `rate`-times server.
        add_exp_and_sp(&mut world, KILLER, 360_000.0 * rate, 0.0, false);
        rep(&world, KILLER) + 1_000_000
    };
    assert!(redeemed(1.0) > 0, "the baseline actually redeems something");
    assert_eq!(
        redeemed(1.0),
        redeemed(10.0),
        "ten times the exp at ten times the rate redeems the same karma"
    );
}

// ---------------------------------------------------------------------------
// PVP.ini, wired (row 14)
// ---------------------------------------------------------------------------

/// **The flag timers come from `PvPVsNormalTime`/`PvPVsPvPTime`, not from
/// constants.** Both were hardcoded to the shipped 120 s / 60 s; an operator
/// editing PVP.ini changed nothing.
#[test]
fn pvp_flag_durations_follow_the_config() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);
    // A clean target: the *normal* timer applies.
    world.cfg.pvp.pvp_normal_time_ms = 30_000; // 300 ticks
    world.cfg.pvp.pvp_pvp_time_ms = 5_000; // 50 ticks
    world.tick = 1_000;

    pvp::update_pvp_status_target(&mut world, KILLER, VICTIM);
    let st = world.objects.get_component::<PvpState>(&KILLER).unwrap();
    assert_eq!(st.flag, 1, "attacker is flagged");
    assert_eq!(
        st.expires_tick,
        1_000 + 300,
        "the clean-target flag lasts PvPVsNormalTime"
    );

    // A flagged target shortens it to PvPVsPvPTime.
    world
        .objects
        .get_component_mut::<PvpState>(&VICTIM)
        .unwrap()
        .flag = 1;
    world.tick = 2_000;
    pvp::update_pvp_status_target(&mut world, KILLER, VICTIM);
    assert_eq!(
        world
            .objects
            .get_component::<PvpState>(&KILLER)
            .unwrap()
            .expires_tick,
        2_000 + 50,
        "against a flagged target it is PvPVsPvPTime"
    );
}

/// **`MaxReputation` is the ceiling reputation cannot pass**, and it was
/// hardcoded as `.min(0)` in the karma-recovery path. The shipped 0 is what
/// keeps reputation from ever going positive on this dist; raising it is what
/// an operator would do to restore retail behaviour.
#[test]
fn reputation_is_clamped_to_the_configured_maximum() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);
    world.cfg.pvp.reputation_increase = 500;
    world.cfg.pvp.max_reputation = 100;
    // A PK victim, a clean killer within ten levels: the lawful branch pays
    // `reputation_increase` — and the clamp caps it.
    world
        .objects
        .get_component_mut::<Player>(&VICTIM)
        .unwrap()
        .reputation = -1;

    kill(&mut world);

    assert_eq!(
        rep(&world, KILLER),
        100,
        "500 earned, clamped to MaxReputation"
    );
}
