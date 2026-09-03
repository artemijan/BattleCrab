//! NPC pathfinding (G21 slice 7): the geodata half of `Creature.moveToLocation`
//! for mobs, which until now moved in straight lines through walls.

use super::*;

use crate::geo::worker::{PathEvent, PathRequest};
use crate::model::components::{Movement, PathWait, Position, Vitals};

const NPC_ID: i32 = 44000;

/// Giran town square — real geodata, and the probe below is anchored on it.
const GIRAN: (i32, i32, i32) = (82698, 148638, -3473);
/// +600 on x from `GIRAN` is fully blocked (the clamp cuts the whole 600).
const BLOCKED_DELTA: (i32, i32) = (600, 0);
/// +600 on y is open ground (clamp shortfall 0).
const CLEAR_DELTA: (i32, i32) = (0, 600);

/// The real geodata, loaded once for the whole test module — it's ~seconds to
/// parse and several tests need it.
fn geo() -> Arc<crate::geo::GeoEngine> {
    static GEO: std::sync::OnceLock<Arc<crate::geo::GeoEngine>> = std::sync::OnceLock::new();
    GEO.get_or_init(|| {
        Arc::new(crate::geo::GeoEngine::load(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/data/geodata"
        ))))
    })
    .clone()
}

/// A world with real geodata and an observable path channel.
fn path_world() -> (World, std::sync::mpsc::Receiver<PathRequest>, i32) {
    let (mut world, _db, _l) = combat_test_world();
    world.geo = geo();
    let (tx, rx) = std::sync::mpsc::channel();
    world.path = tx;

    let mut t = crate::data::npc_data::default_template(NPC_ID);
    t.type_name = "Monster".into();
    t.name = "Pathing Mob".into();
    t.level = 20;
    t.base_hp_max = 500.0;
    t.base_run_spd = 120.0;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(
        &mut world, NPC_OID, NPC_ID, "Monster", 20, GIRAN.0, GIRAN.1, GIRAN.2,
    );
    (world, rx, NPC_OID)
}

/// The destination every routed test walks toward: +600 x from Giran, which the
/// clamp cuts entirely, so the move has to go out to the path worker.
fn blocked_target() -> (i32, i32, i32) {
    (
        GIRAN.0 + BLOCKED_DELTA.0,
        GIRAN.1 + BLOCKED_DELTA.1,
        GIRAN.2,
    )
}

/// A two-leg route ending at `target` — the shape the worker replies with.
fn route_to(target: (i32, i32, i32)) -> Option<Vec<(i32, i32, i32)>> {
    Some(vec![(GIRAN.0, GIRAN.1 + 300, GIRAN.2), target])
}

/// Feed back what the geo worker would have answered `req` with: its request
/// context echoed, plus the route it found (`None` when there is none).
fn reply_with_path(world: &mut World, req: &PathRequest, path: Option<Vec<(i32, i32, i32)>>) {
    handle_path_result(
        world,
        PathEvent {
            seq: req.seq,
            client_id: req.client_id,
            object_id: req.object_id,
            to: req.to,
            path,
        },
    );
}
// ---------------------------------------------------------------------------

#[test]
fn a_clear_line_moves_straight_without_asking_the_path_worker() {
    let (mut world, rx, oid) = path_world();

    ai::move_npc_to(
        &mut world,
        oid,
        GIRAN.0 + CLEAR_DELTA.0,
        GIRAN.1 + CLEAR_DELTA.1,
        GIRAN.2,
    );

    assert!(requests(&rx).is_empty(), "open ground needs no pathfinding");
    let mv = world
        .objects
        .get_component::<Movement>(&oid)
        .expect("straight move started");
    assert!(mv.0.geo_path.is_none(), "and no route attached");
}

#[test]
fn a_blocked_line_asks_the_path_worker_instead_of_walking_through_the_wall() {
    let (mut world, rx, oid) = path_world();
    let target = blocked_target();

    ai::move_npc_to(&mut world, oid, target.0, target.1, target.2);

    let reqs = requests(&rx);
    assert_eq!(
        reqs.len(),
        1,
        "the blocked line should have queued one path request"
    );
    assert_eq!(
        reqs[0].to, target,
        "the worker gets the ORIGINAL destination, not the clamped one"
    );
    assert!(
        !reqs[0].playable,
        "AI movers use Java's cheaper single-pass filter"
    );
    assert!(
        world.objects.get_component::<Movement>(&oid).is_none(),
        "no move starts until the route comes back — the mob must not walk into the wall meanwhile"
    );
    assert!(
        world.objects.has_component::<PathWait>(&oid),
        "and it is marked as waiting"
    );
}

#[test]
fn a_second_think_does_not_queue_a_duplicate_request() {
    // The AI re-issues a chase every 1 s think; without the in-flight guard
    // that floods the worker with duplicates for the same mob.
    let (mut world, rx, oid) = path_world();
    let target = blocked_target();

    ai::move_npc_to(&mut world, oid, target.0, target.1, target.2);
    ai::move_npc_to(&mut world, oid, target.0, target.1, target.2);
    ai::move_npc_to(&mut world, oid, target.0, target.1, target.2);

    assert_eq!(requests(&rx).len(), 1, "one outstanding request per mob");
}

#[test]
fn the_reply_starts_a_route_move_for_an_npc() {
    // The reply path was player-only before this slice; an NPC riding it is
    // the main risk in the change.
    let (mut world, rx, oid) = path_world();
    let target = blocked_target();
    ai::move_npc_to(&mut world, oid, target.0, target.1, target.2);
    let req = requests(&rx).pop().expect("request queued");

    reply_with_path(&mut world, &req, route_to(target));

    let mv = world
        .objects
        .get_component::<Movement>(&oid)
        .expect("route move started");
    assert!(
        mv.0.geo_path.is_some(),
        "the NPC follows the returned route"
    );
    assert!(
        !world.objects.has_component::<PathWait>(&oid),
        "the wait is cleared"
    );
}

#[test]
fn a_route_that_cannot_be_found_leaves_the_npc_still() {
    let (mut world, rx, oid) = path_world();
    let target = blocked_target();
    ai::move_npc_to(&mut world, oid, target.0, target.1, target.2);
    let req = requests(&rx).pop().expect("request queued");

    reply_with_path(&mut world, &req, None);

    assert!(
        world.objects.get_component::<Movement>(&oid).is_none(),
        "no route, no move"
    );
    assert!(
        !world.objects.has_component::<PathWait>(&oid),
        "but the wait must clear, or the mob could never path again"
    );
}

#[test]
fn a_reply_for_a_mob_that_died_meanwhile_is_dropped() {
    let (mut world, rx, oid) = path_world();
    let target = blocked_target();
    ai::move_npc_to(&mut world, oid, target.0, target.1, target.2);
    let req = requests(&rx).pop().expect("request queued");
    world
        .objects
        .get_component_mut::<Vitals>(&oid)
        .unwrap()
        .dead = true;

    reply_with_path(&mut world, &req, route_to(target));

    assert!(
        world.objects.get_component::<Movement>(&oid).is_none(),
        "a corpse doesn't walk the route"
    );
}

#[test]
fn pathfinding_disabled_falls_back_to_the_old_straight_move() {
    let (mut world, rx, oid) = path_world();
    world.path_finding = 0;
    let target = blocked_target();

    ai::move_npc_to(&mut world, oid, target.0, target.1, target.2);

    assert!(
        requests(&rx).is_empty(),
        "PathFinding=0 never consults the worker"
    );
    let mv = world
        .objects
        .get_component::<Movement>(&oid)
        .expect("straight move");
    assert_eq!(
        (mv.0.dest_x, mv.0.dest_y),
        (target.0, target.1),
        "unclamped, exactly as before"
    );
}

#[test]
fn the_npc_takes_the_geodata_corrected_z() {
    // Java: `if (!isPlayer()) z = destiny.getZ()` — a player keeps the z its
    // client asked for, an NPC does not.
    let (mut world, _rx, oid) = path_world();
    let absurd_z = GIRAN.2 + 5000;

    ai::move_npc_to(
        &mut world,
        oid,
        GIRAN.0 + CLEAR_DELTA.0,
        GIRAN.1 + CLEAR_DELTA.1,
        absurd_z,
    );

    let mv = world
        .objects
        .get_component::<Movement>(&oid)
        .expect("move started");
    assert_ne!(
        mv.0.dest_z, absurd_z,
        "the mob must not path to a z 5000 units in the air"
    );
}

#[test]
fn a_rooted_mob_still_refuses_to_move() {
    // The geodata work sits after the movement-disabled gate; make sure the
    // new code didn't move that check.
    let (mut world, rx, oid) = path_world();
    let pos = world
        .objects
        .get_component::<Position>(&oid)
        .copied()
        .unwrap();
    assert_eq!((pos.x, pos.y), (GIRAN.0, GIRAN.1));
    // Root the mob via a buff carrying the movement-disabled flag.
    world.objects.add_components(
        &oid,
        Buffs(vec![model::skill::active_buff::ActiveBuff {
            skill_id: 1,
            abnormal_type: "ROOT".into(),
            abnormal_level: 1,
            slot: model::skill::BuffSlot::Uncapped,
            effect_flags: model::skill::effect_flag::ROOTED,
            ..test_buff()
        }]),
    );

    ai::move_npc_to(
        &mut world,
        oid,
        GIRAN.0 + CLEAR_DELTA.0,
        GIRAN.1 + CLEAR_DELTA.1,
        GIRAN.2,
    );

    assert!(requests(&rx).is_empty());
    assert!(
        world.objects.get_component::<Movement>(&oid).is_none(),
        "a rooted mob stays put"
    );
}
