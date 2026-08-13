//! `CtrlEvent.EVT_ARRIVED` → `CreatureAI.onEvtArrived` → `onEvtThink()`.
//!
//! Java runs `MovementTaskManager` every 100 ms; the tick that lands a creature
//! on its destination fires `EVT_ARRIVED`, and `CreatureAI.onEvtArrived` ends
//! with `onEvtThink()`. For a chasing monster that means `thinkAttack()` →
//! `doAutoAttack(target)` on the very same 100 ms tick it closes the gap —
//! because `Creature.moveToLocation`'s offset branch already shortened the
//! chase destination to `range - 5`, so *arriving* is *being in attack range*.
//!
//! Without that hook the mob sits in range until the next 1 s `AttackableAI`
//! think, which is a visible 0–1000 ms hitch before a monster's first swing.

use super::*;

use crate::model::components::{AttackState, Casting, Movement};
use crate::model::npc::{NpcAi, NpcIntention};

const PLAYER: i32 = 3001;
const MOB: i32 = NPC_OID;
const MOB_ID: i32 = 45000;

/// A monster at its spawn, already locked on to the player, far enough away
/// that it has to walk in.
fn mob_chasing_player(world: &mut World, mob_at: (i32, i32)) {
    add_test_npc(world, MOB, MOB_ID, "Monster", 20, mob_at.0, mob_at.1, 0);
    add_hate(world, MOB, PLAYER, 100.0, 100.0);
    let tick = world.tick;
    let ai = world.objects.get_component_mut::<NpcAi>(&MOB).unwrap();
    ai.intention = NpcIntention::Attack;
    ai.global_aggro = 0;
    // Far out, so the attack-timeout branch never pre-empts the chase.
    ai.attack_timeout_tick = tick + 10_000;
}

/// A swing is in flight: `AttackState` rides along on every NPC from spawn, so
/// presence proves nothing — `attack_end_tick` past *now* is the real signal
/// that `do_auto_attack` just ran.
fn is_swinging(world: &World) -> bool {
    world
        .objects
        .get_component::<AttackState>(&MOB)
        .is_some_and(|st| st.attack_end_tick > world.tick)
}

/// Advance movement one 100 ms tick — and *only* movement. No `npc_ai_tick`,
/// so anything the mob does here came from the arrival hook, not the 1 s think.
fn movement_only_tick(world: &mut World) {
    world.tick += 1;
    super::super::visibility::movement_tick(world);
}

/// Walk the chase out on movement ticks alone. Returns the tick the mob
/// arrived on, and asserts it never swung early.
fn walk_until_arrival(world: &mut World, expect_no_early_swing: bool) -> Option<u64> {
    for _ in 0..600 {
        movement_only_tick(world);
        if !world.objects.has_component::<Movement>(&MOB) {
            return Some(world.tick);
        }
        if expect_no_early_swing {
            assert!(
                !is_swinging(world),
                "no swing before the mob is actually in range"
            );
        }
    }
    None
}

/// The regression: a mob that walks into attack range swings on the arrival
/// tick itself, not up to a second later.
#[test]
fn a_mob_attacks_on_the_tick_it_arrives_in_range() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 400, 0);
    mob_chasing_player(&mut world, (0, 0));
    drain(&mut rx);

    // One AI think starts the chase — the mob is out of attack range.
    ai::npc_ai_tick(&mut world);
    assert!(
        world.objects.has_component::<Movement>(&MOB),
        "the mob is out of range and started closing"
    );
    assert!(
        !is_swinging(&world),
        "it has not swung yet — it is still walking in"
    );
    drain(&mut rx);

    // The chase is several seconds long at monster speed, so a mob that waited
    // for the next 1 s AI think would be caught standing idle here.
    let arrival_tick = walk_until_arrival(&mut world, true).expect("the mob finished its chase");

    assert!(
        is_swinging(&world),
        "the mob swung the moment it arrived, without waiting for the 1 s AI think"
    );
    assert!(
        world
            .objects
            .get_component::<AttackState>(&MOB)
            .unwrap()
            .attack_end_tick
            > arrival_tick,
        "the swing was started on the arrival tick, not left over from earlier"
    );
    let packets = drain(&mut rx);
    assert!(
        packets
            .iter()
            .any(|p| is_for(p, server_packets::opcodes::ATTACK, MOB)),
        "the arrival swing is broadcast as Attack"
    );
}

/// `AbstractAI.notifyEvent`: "we don't process it if we're casting" — a mob
/// that arrives mid-cast lets the cast finish instead of thinking again.
#[test]
fn arrival_does_not_think_while_the_mob_is_casting() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 400, 0);
    mob_chasing_player(&mut world, (0, 0));
    drain(&mut rx);

    ai::npc_ai_tick(&mut world);
    assert!(world.objects.has_component::<Movement>(&MOB));

    world.objects.add_components(
        &MOB,
        Casting(crate::model::CastState {
            skill_id: 1,
            skill_level: 1,
            skill_sub_level: 0,
            target_object_id: PLAYER,
            seq: 1,
            launched: false,
            cancel_ms: 0,
            cool_ms: 0,
            trigger_item_object_id: 0,
        }),
    );

    assert!(
        walk_until_arrival(&mut world, false).is_some(),
        "the mob still finished its walk"
    );
    assert!(
        !is_swinging(&world),
        "no arrival think — the cast owns the mob until it resolves"
    );
}

/// `AttackableAI.onEvtThink` bails unless the region and its neighbors are
/// active. The arrival hook runs off the movement sweep, which is not
/// region-filtered, so it has to make that test itself.
#[test]
fn arrival_does_not_think_in_an_inactive_region() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 400, 0);
    mob_chasing_player(&mut world, (0, 0));
    drain(&mut rx);

    ai::npc_ai_tick(&mut world);
    assert!(world.objects.has_component::<Movement>(&MOB));

    // Everyone logs out mid-chase: nothing keeps the mob's region active.
    world.clients.clear();

    assert!(
        walk_until_arrival(&mut world, false).is_some(),
        "the walk still completes — movement is not region-gated"
    );
    assert!(!is_swinging(&world), "an unwatched region does not think");
}
