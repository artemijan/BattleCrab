//! `AttackableAI._globalAggro` — the calm windows a mob sits in before it will
//! act on hate at all.
//!
//! Two of them, both counted down one step per 1 s think in `thinkActive`:
//! - **−10**, seeded in the `AttackableAI` constructor, so a freshly spawned
//!   mob ignores everyone for ~10 s.
//! - **−25**, from `setGlobalAggro(-25)` in `Attackable.setTarget(null)`, so a
//!   mob whose last hated target vanished stands down instead of re-aggroing
//!   the next passer-by on the very next scan tick.
//!
//! The gate wraps *both* halves of `thinkActive`'s aggro block in Java — the
//! range scan **and** the most-hated/attack decision. Gating only the scan
//! leaks: hate can arrive without ever clearing the counter (faction calls,
//! minion relays, script seeding), and >10 of it would then punch straight
//! through the calm window.

use super::*;

use crate::model::components::space::Position;
use crate::model::npc::{AggroList, NpcAi, NpcIntention};

const PLAYER: i32 = 3001;
const MOB: i32 = NPC_OID;
const MOB_ID: i32 = 45100;

/// A monster beside the player carrying `hate` for them already, with the
/// AI left in whatever calm state the caller wants to test.
fn mob_hating_player(world: &mut World, hate: f64) {
    add_test_npc(world, MOB, MOB_ID, "Monster", 20, 100, 0, 0);
    add_hate(world, MOB, PLAYER, hate, hate);
}

fn intention(world: &World) -> NpcIntention {
    world
        .objects
        .get_component::<NpcAi>(&MOB)
        .unwrap()
        .intention
}

fn global_aggro(world: &World) -> i32 {
    world
        .objects
        .get_component::<NpcAi>(&MOB)
        .unwrap()
        .global_aggro
}

fn hate_count(world: &World) -> usize {
    world
        .objects
        .get_component::<AggroList>(&MOB)
        .map(|a| a.0.len())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// The −10 post-spawn window.
// ---------------------------------------------------------------------------

/// **The bug this guards.** A mob carrying more hate than the calm counter is
/// deep must still refuse to engage: Java skips the whole block, so hate never
/// gets weighed against `_globalAggro` at all while it is negative. With only
/// the scan gated, `100 + (-9) > 0` promoted the mob to `Attack` on its first
/// think — a mob that faction-called or was script-seeded charged out of its
/// spawn window instantly.
#[test]
fn a_calm_mob_ignores_hate_it_already_carries() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 0, 0);
    mob_hating_player(&mut world, 100.0);
    drain(&mut rx);

    ai::npc_ai_tick(&mut world);

    assert_eq!(global_aggro(&world), -9, "the counter ticked one step");
    assert_eq!(
        intention(&world),
        NpcIntention::Active,
        "still calm: 100 hate does not out-vote a negative _globalAggro"
    );
}

/// The zero case that keeps the one above honest — once the window closes the
/// very same hate does engage the mob.
#[test]
fn hate_takes_effect_once_the_calm_window_closes() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 0, 0);
    mob_hating_player(&mut world, 100.0);
    drain(&mut rx);

    // 10 thinks to walk -10 up to 0, and one more to act on it.
    for _ in 0..11 {
        ai::npc_ai_tick(&mut world);
    }

    assert_eq!(global_aggro(&world), 0, "window closed");
    assert_eq!(
        intention(&world),
        NpcIntention::Attack,
        "engages once the spawn window is over"
    );
}

/// A calm mob is not a frozen one: because Java's `return` sits *inside* the
/// `_globalAggro >= 0` block, a mob holding hate it may not act on falls
/// through to the idle branches and still walks home. It must not be parked
/// over its aggro list.
#[test]
fn a_calm_mob_still_drifts_back_towards_its_spawn() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 0, 0);
    mob_hating_player(&mut world, 100.0);
    // Drag it well past `MAX_DRIFT_RANGE` from its spawn anchor.
    {
        let pos = world.objects.get_component_mut::<Position>(&MOB).unwrap();
        pos.x = 900;
    }
    drain(&mut rx);

    ai::npc_ai_tick(&mut world);

    assert_eq!(intention(&world), NpcIntention::Active, "still calm");
    assert!(
        world.objects.has_component::<Movement>(&MOB),
        "the idle branches still ran: it is walking home, not parked on its hate list"
    );
}

// ---------------------------------------------------------------------------
// The −25 window: `Attackable.setTarget(null)` via `EVT_FORGET_OBJECT`.
// ---------------------------------------------------------------------------

/// Java fires `EVT_FORGET_OBJECT` when the mob's target leaves its 3×3 block,
/// and `Attackable.setTarget(null)` turns that into: drop the entry, and — the
/// list now being empty — stand down for ~25 s. Without it the mob keeps the
/// grudge and re-engages the instant the player steps back into range.
#[test]
fn a_mob_whose_only_target_leaves_the_block_stands_down_for_25s() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 0, 0);
    mob_hating_player(&mut world, 100.0);
    drain(&mut rx);

    // Region 0 → region 3: out of the mob's 3×3 block.
    world
        .objects
        .get_component_mut::<Position>(&PLAYER)
        .unwrap()
        .x = 3 * 2048 + 100;
    visibility::update_region(&mut world, PLAYER);

    assert_eq!(hate_count(&world), 0, "the target's entry is removed");
    assert_eq!(
        global_aggro(&world),
        -25,
        "dropped into the long calm window"
    );
    assert_eq!(intention(&world), NpcIntention::Active, "back to the scan");
}

/// Logging out mid-fight is the other departure Java forgets on
/// (`World.removeVisibleObject`).
#[test]
fn a_logout_mid_fight_also_stands_the_mob_down() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 0, 0);
    mob_hating_player(&mut world, 100.0);
    drain(&mut rx);

    visibility::on_leave_world(&mut world, PLAYER);

    assert_eq!(hate_count(&world), 0, "the target's entry is removed");
    assert_eq!(global_aggro(&world), -25, "stood down");
}

/// The `if (_aggroList.isEmpty())` guard, read literally as Java does: a second
/// attacker still on the list — even one whose hate has decayed to nothing —
/// keeps the mob out of the calm window, because there is still someone to
/// re-pick next think.
#[test]
fn a_second_attacker_on_the_list_keeps_the_mob_engaged() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 0, 0);
    mob_hating_player(&mut world, 100.0);
    add_hate(&mut world, MOB, PLAYER + 1, 0.0, 5.0);
    drain(&mut rx);

    world
        .objects
        .get_component_mut::<Position>(&PLAYER)
        .unwrap()
        .x = 3 * 2048 + 100;
    visibility::update_region(&mut world, PLAYER);

    assert_eq!(hate_count(&world), 1, "only the departed target is dropped");
    assert_ne!(
        global_aggro(&world),
        -25,
        "no stand-down: the list is not empty"
    );
}

/// The edge, not the level. A grudge seeded on someone who was never in the
/// mob's block (quest choreography spawning two duellists far apart) must not
/// count as a forget — Java only raises `EVT_FORGET_OBJECT` for an object that
/// *was* known and stopped being known.
#[test]
fn a_grudge_seeded_across_the_map_is_not_a_forget() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 5 * 2048, 0);
    mob_hating_player(&mut world, 100.0);
    drain(&mut rx);

    // The player moves, but between two regions both far from the mob — its
    // adjacency to the mob never changes, so no forget fires.
    world
        .objects
        .get_component_mut::<Position>(&PLAYER)
        .unwrap()
        .x = 6 * 2048;
    visibility::update_region(&mut world, PLAYER);

    assert_eq!(hate_count(&world), 1, "the seeded grudge survives");
    assert_ne!(global_aggro(&world), -25, "no spurious stand-down");
}
