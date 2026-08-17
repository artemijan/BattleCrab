//! What premium status actually *buys* (G16): per-item drop overrides and the
//! quest-reward rates.
//!
//! Both blocks were parsed into `PremiumConfig` and read by nobody — a premium
//! killer's drops and quest rewards were identical to everyone else's.

use super::*;

use crate::game_loop::death::{PremiumDropRate, premium_drop_mult};
use crate::model::Player;

const PLAYER: i32 = 8001;
const CID: u32 = 1;
const ADENA: i32 = 57;
const DIST: &str = crate::data::DIST_GAME;

fn grant_premium(world: &mut World, oid: i32) {
    world.cfg.premium.enabled = true;
    let account = world
        .objects
        .get_component::<Player>(&oid)
        .unwrap()
        .account
        .clone();
    crate::game_loop::admin::premium::add_premium_time(
        world,
        &account,
        30 * crate::game_loop::admin::premium::DAY_MILLIS,
    );
}

// ---------------------------------------------------------------------------
// Per-item drop overrides
// ---------------------------------------------------------------------------

/// The dist declares `PremiumRateDropChanceByItemId = 57,2;6656,1;…`. Adena is
/// doubled; the listed jewels are pinned to **×1**, which is the whole point of
/// the map — it *replaces* the flat rate rather than stacking with it, so
/// premium buys nothing on them even though the flat amount rate is ×2.
#[test]
fn the_per_item_override_replaces_the_flat_rate() {
    let (mut world, _db, _l) = cast_test_world();
    world.cfg.premium = crate::config::premium::PremiumConfig::load_from(DIST);

    assert_eq!(
        premium_drop_mult(&world, ADENA, false, PremiumDropRate::Amount),
        2.0,
        "adena is in the map at 2"
    );
    assert_eq!(
        premium_drop_mult(&world, 6656, false, PremiumDropRate::Amount),
        1.0,
        "a mapped jewel is pinned to 1, NOT the flat 2"
    );
    // Something absent from the map falls through to the flat rate.
    assert_eq!(
        premium_drop_mult(&world, 1060, false, PremiumDropRate::Amount),
        world.cfg.premium.rate_drop_amount,
    );
    assert_eq!(
        premium_drop_mult(&world, 1060, false, PremiumDropRate::Chance),
        world.cfg.premium.rate_drop_chance,
    );
}

/// **Java's herb and raid arms are empty** — a premium killer gets no bonus at
/// all on a herb drop or a raid drop unless the item is in the map. (The two
/// `Premium herb chance? :)` musings are Java's own.)
#[test]
fn herbs_and_raid_drops_get_no_premium_bonus() {
    let (mut world, _db, _l) = cast_test_world();
    world.cfg.premium = crate::config::premium::PremiumConfig::load_from(DIST);
    world.cfg.premium.rate_drop_chance = 5.0;
    world.cfg.premium.rate_drop_amount = 5.0;

    // A herb — `ex_immediate_effect` on the item template is what marks one.
    let herb_id = {
        let dist_items = dist::items();
        let herb = dist_items.get(8600).expect("Herb of Life");
        assert!(herb.ex_immediate_effect, "8600 really is a herb");
        world.data.item_data.insert_for_test(herb.clone());
        8600
    };
    assert_eq!(
        premium_drop_mult(&world, herb_id, false, PremiumDropRate::Chance),
        1.0,
        "no premium herb bonus"
    );
    assert_eq!(
        premium_drop_mult(&world, herb_id, false, PremiumDropRate::Amount),
        1.0,
    );
    assert_eq!(
        premium_drop_mult(&world, 1060, true, PremiumDropRate::Amount),
        1.0,
        "no premium raid bonus"
    );
    // …but the map still wins over both.
    assert_eq!(
        premium_drop_mult(&world, ADENA, true, PremiumDropRate::Amount),
        2.0,
        "a mapped item is checked before the raid arm"
    );
}

/// End to end: the same mob, killed by the same character, drops twice the
/// adena once that character's account is premium.
#[test]
fn a_premium_killer_gets_the_doubled_adena_count() {
    let loot = |world: &mut World| {
        let t = world.data.npc_data.get(40001).unwrap().clone();
        // chance roll pass, amount is fixed (min == max), gap gates pass.
        world.force_rolls([0, 0]);
        crate::game_loop::death::roll_drops_for_test(world, &t, PLAYER)
    };

    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    world.cfg.premium = crate::config::premium::PremiumConfig::load_from(DIST);

    let plain = loot(&mut world);
    assert_eq!(plain, vec![(ADENA, 5)], "the fixture drop is 5 adena");

    grant_premium(&mut world, PLAYER);
    let premium = loot(&mut world);
    assert_eq!(
        premium,
        vec![(ADENA, 10)],
        "the per-item x2 amount override applied"
    );
}

// ---------------------------------------------------------------------------
// Quest reward rates
// ---------------------------------------------------------------------------

/// `PremiumRateQuestXp`/`Sp` multiply a quest turn-in **before** the server's
/// `RateQuestReward*`. Both are 1 on this dist, so the test sets them.
#[test]
fn premium_multiplies_quest_rewards() {
    // `combat_test_world` for its exp table — `add_exp_and_sp` needs one to
    // place the award against a level.
    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    world.cfg.rates.rate_quest_reward_xp = 2.0;
    world.cfg.rates.rate_quest_reward_sp = 2.0;
    world.cfg.premium.rate_quest_xp = 3.0;
    world.cfg.premium.rate_quest_sp = 3.0;

    let exp_of = |w: &World| {
        w.objects
            .get_component::<Player>(&PLAYER)
            .map(|p| (p.exp, p.sp))
            .unwrap()
    };
    let (e0, s0) = exp_of(&world);
    crate::game_loop::quests::add_quest_exp_and_sp(&mut world, PLAYER, 1_000, 100);
    let (e1, s1) = exp_of(&world);
    assert_eq!(e1 - e0, 2_000, "server rate only");
    assert_eq!(s1 - s0, 200);

    grant_premium(&mut world, PLAYER);
    crate::game_loop::quests::add_quest_exp_and_sp(&mut world, PLAYER, 1_000, 100);
    let (e2, s2) = exp_of(&world);
    assert_eq!(e2 - e1, 6_000, "premium x3 then server x2");
    assert_eq!(s2 - s1, 600);
}

/// The dist's own values, so a change to the ini is visible here.
#[test]
fn the_premium_rate_block_is_read_from_the_dist_ini() {
    let cfg = crate::config::premium::PremiumConfig::load_from(DIST);
    assert!(
        (cfg.rate_quest_xp - 1.0).abs() < 1e-9,
        "PremiumRateQuestXp = 1"
    );
    assert!((cfg.rate_quest_sp - 1.0).abs() < 1e-9);
    assert_eq!(cfg.rate_drop_chance_by_id.get(&ADENA).copied(), Some(2.0));
    assert_eq!(cfg.rate_drop_amount_by_id.get(&ADENA).copied(), Some(2.0));
    assert_eq!(cfg.rate_drop_amount_by_id.get(&6656).copied(), Some(1.0));
    assert_eq!(
        cfg.rate_drop_amount_by_id.len(),
        11,
        "all eleven entries parse"
    );
}

/// The **chance** side, which the amount test above can't see: a drop the base
/// rate would have missed lands once the premium multiplier doubles its chance.
#[test]
fn the_premium_chance_multiplier_can_turn_a_miss_into_a_drop() {
    // The fixture mob drops adena at 70 %. Roll 0.80: a miss at 70, a hit once
    // adena's ×2 override takes it to 140.
    let loot = |world: &mut World| {
        let t = world.data.npc_data.get(40001).unwrap().clone();
        world.force_rolls([0, 800_000]); // level-gap pass, then the chance roll
        crate::game_loop::death::roll_drops_for_test(world, &t, PLAYER)
    };

    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    world.cfg.premium = crate::config::premium::PremiumConfig::load_from(DIST);

    assert!(loot(&mut world).is_empty(), "80 % roll misses a 70 % drop");

    grant_premium(&mut world, PLAYER);
    assert_eq!(
        loot(&mut world),
        vec![(ADENA, 10)],
        "the doubled chance turns it into a drop"
    );
}

/// Spoil has its **own** premium pair (`PremiumRateSpoilChance`/`Amount`) and
/// — unlike the death branch — reads no per-item overrides at all.
#[test]
fn spoil_uses_the_flat_premium_spoil_rates() {
    let spoil = |world: &mut World| {
        let t = world.data.npc_data.get(40001).unwrap().clone();
        world.force_rolls([0, 800_000]); // level-gap pass, then the chance roll
        crate::game_loop::death::roll_spoil_drops_for_test(world, &t, PLAYER)
    };

    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    world.cfg.premium = crate::config::premium::PremiumConfig::load_from(DIST);
    // The fixture mob has no spoil list; give it a 70 % one worth 5.
    {
        let mut t = world.data.npc_data.get(40001).unwrap().clone();
        t.drop_list_spoil.push(crate::data::npc_data::DropHolder {
            item_id: ADENA,
            min: 5,
            max: 5,
            chance: 70.0,
        });
        world.data.npc_data.insert_for_test(t);
    }
    // `PremiumRateSpoilChance` is 1 on this dist and the amount 2 — set the
    // chance so the roll can see it.
    world.cfg.premium.rate_spoil_chance = 2.0;

    assert!(
        spoil(&mut world).is_empty(),
        "80 % roll misses a 70 % spoil"
    );

    grant_premium(&mut world, PLAYER);
    assert_eq!(
        spoil(&mut world),
        vec![(ADENA, 10)],
        "x2 chance lands it, x2 amount doubles it"
    );
}

// ---------------------------------------------------------------------------
// `.premium` — the account panel (G33)
// ---------------------------------------------------------------------------

/// Decode the `NpcHtmlMessage` a voiced command produced, if any.
fn voiced_html(pkts: &[Vec<u8>]) -> Option<String> {
    pkts.iter()
        .find(|p| p[0] == crate::network::server_packets::opcodes::NPC_HTML_MESSAGE)
        .and_then(|p| decode_npc_html(p))
}

fn say(world: &mut World, text: &str) {
    let mut w = commons::network::PacketWriter::new();
    w.write_string(text);
    w.write_i32(0); // ChatType::General
    crate::game_loop::chat::handle_say2(world, CID, &w.into_bytes());
}

/// **Without premium the panel advertises what premium would give**, showing
/// the base rates as "Normal" alongside the premium ones.
#[test]
fn the_premium_panel_shows_normal_status_and_the_upgrade_rates() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    world.cfg.premium.enabled = true;
    world.cfg.rates.rate_xp = 2.0;
    world.cfg.premium.rate_xp = 3.0;
    drain(&mut rx);

    say(&mut world, ".premium");
    let html = voiced_html(&drain(&mut rx)).expect("the panel is an NpcHtmlMessage");
    assert!(
        html.contains("Normal"),
        "no premium → Normal status: {html}"
    );
    // Java multiplies the premium rate *onto* the base rate, so the page shows
    // the effective x6, not the x3 multiplier.
    assert!(html.contains("x2"), "base rate shown: {html}");
    assert!(html.contains("x6"), "effective premium rate shown: {html}");
}

/// **With premium it reports Premium status and an expiry date.**
#[test]
fn the_premium_panel_reports_status_and_expiry_when_active() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    grant_premium(&mut world, PLAYER);
    drain(&mut rx);

    say(&mut world, ".premium");
    let html = voiced_html(&drain(&mut rx)).expect("the panel is an NpcHtmlMessage");
    assert!(html.contains("Premium"), "active status: {html}");
    assert!(html.contains("Expires"), "expiry row present: {html}");
}

/// **With the system off the line is said aloud, not handled** — Java only
/// registers the handler when `EnablePremiumSystem` is on, so an unregistered
/// voiced command falls through to chat. Sending a panel anyway would leak a
/// feature the operator switched off.
#[test]
fn premium_is_not_a_command_when_the_system_is_disabled() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    world.cfg.premium.enabled = false;
    drain(&mut rx);

    say(&mut world, ".premium");
    let pkts = drain(&mut rx);
    assert!(
        voiced_html(&pkts).is_none(),
        "no panel when the system is off"
    );
}
