//! NPC walking routes (G21 slice 10) — `WalkingManager` / `WalkInfo`.

use super::*;

use crate::data::route_data::{RepeatStyle, RouteData, WalkNode, WalkRoute};
use crate::game_loop::walkers::WalkState;
use crate::model::components::{Movement, Position, Vitals};

const WALKER_ID: i32 = 46000;
const WALKER: i32 = NPC_OID;

fn node(x: i32, delay: i32) -> WalkNode {
    WalkNode {
        x,
        y: 0,
        z: 0,
        delay,
        run: false,
        chat: String::new(),
    }
}

/// A three-node route, so the turn-around arithmetic is observable.
fn route(style: RepeatStyle, repeat: bool) -> WalkRoute {
    WalkRoute {
        name: "test_route".into(),
        repeat,
        repeat_style: style,
        nodes: vec![node(0, 0), node(100, 0), node(200, 0)],
    }
}

fn walker_world(
    style: RepeatStyle,
    repeat: bool,
) -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    let mut t = crate::data::npc_data::default_template(WALKER_ID);
    t.type_name = "Folk".into();
    t.name = "Town Walker".into();
    t.level = 30;
    t.base_hp_max = 500.0;
    t.base_run_spd = 120.0;
    t.base_walk_spd = 80.0;
    world.data.npc_data.insert_for_test(t);

    let mut routes = RouteData::default();
    routes.routes.push(route(style, repeat));
    routes.attach_for_test(WALKER_ID, 0);
    world.data.routes = routes;
    (world, db, l)
}

fn place(world: &mut World) {
    add_test_npc(world, WALKER, WALKER_ID, "Folk", 30, 0, 0, 0);
    crate::game_loop::walkers::on_npc_spawn(world, WALKER, WALKER_ID);
}

fn state(world: &World) -> Option<WalkState> {
    world.objects.get_component::<WalkState>(&WALKER).copied()
}

/// Run one sweep, then teleport the NPC to its destination and drop the
/// `Movement` — standing in for the leg actually being walked, so the tests
/// don't have to simulate travel time.
fn walk_one_leg(world: &mut World) {
    crate::game_loop::walkers::walker_tick(world);
    if let Some(mv) = world.objects.get_component::<Movement>(&WALKER).cloned() {
        let (dx, dy, dz) = (mv.0.dest_x, mv.0.dest_y, mv.0.dest_z);
        if let Some(p) = world.objects.get_component_mut::<Position>(&WALKER) {
            p.x = dx;
            p.y = dy;
            p.z = dz;
        }
        world.objects.remove_component::<Movement>(&WALKER);
    }
    // The arrival sweep: notices the Movement is gone and banks the delay.
    world.tick += crate::game_loop::walkers::WALKER_PERIOD;
    crate::game_loop::walkers::walker_tick(world);
}

/// The node index the walker visits over `legs` legs.
fn visited(world: &mut World, legs: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for _ in 0..legs {
        walk_one_leg(world);
        match state(world) {
            Some(s) => out.push(s.node),
            None => break,
        }
    }
    out
}

// ---------------------------------------------------------------------------

#[test]
fn a_route_is_attached_on_spawn() {
    let (mut world, _db, _l) = walker_world(RepeatStyle::GoFirst, true);
    place(&mut world);

    let s = state(&world).expect("the walker picked up its route");
    assert_eq!(s.node, 0);
    assert!(!s.travelling, "it starts standing at node 0");
}

#[test]
fn an_npc_without_a_route_gets_no_state() {
    let (mut world, _db, _l) = walker_world(RepeatStyle::GoFirst, true);
    add_test_npc(&mut world, WALKER, 40001, "Monster", 5, 0, 0, 0);
    crate::game_loop::walkers::on_npc_spawn(&mut world, WALKER, 40001);

    assert!(world.objects.get_component::<WalkState>(&WALKER).is_none());
}

#[test]
fn the_walker_starts_moving_towards_the_next_node() {
    let (mut world, _db, _l) = walker_world(RepeatStyle::GoFirst, true);
    place(&mut world);

    crate::game_loop::walkers::walker_tick(&mut world);

    let mv = world
        .objects
        .get_component::<Movement>(&WALKER)
        .expect("a leg started");
    assert_eq!(mv.0.dest_x, 100, "heading for node 1");
    assert!(state(&world).unwrap().travelling);
}

#[test]
fn a_cycle_route_returns_to_the_first_node() {
    // `cycle` (REPEAT_GO_FIRST): 0 → 1 → 2 → 0 → 1 …
    let (mut world, _db, _l) = walker_world(RepeatStyle::GoFirst, true);
    place(&mut world);

    assert_eq!(visited(&mut world, 5), vec![1, 2, 0, 1, 2]);
}

#[test]
fn a_back_route_retraces_its_steps() {
    // `back` (REPEAT_GO_BACK): 0 → 1 → 2 → 1 → 0 → 1 → 2 …
    // The `-= 2` in Java's arithmetic is what produces the 1 after the 2.
    let (mut world, _db, _l) = walker_world(RepeatStyle::GoBack, true);
    place(&mut world);

    assert_eq!(visited(&mut world, 6), vec![1, 2, 1, 0, 1, 2]);
}

#[test]
fn a_non_repeating_route_stops_at_the_last_node() {
    let (mut world, _db, _l) = walker_world(RepeatStyle::None, false);
    place(&mut world);

    walk_one_leg(&mut world); // → node 1
    walk_one_leg(&mut world); // → node 2 (last)
    assert_eq!(state(&world).map(|s| s.node), Some(2));
    walk_one_leg(&mut world); // would overrun

    assert!(
        state(&world).is_none(),
        "the route is dropped once it runs out"
    );
}

#[test]
fn a_node_delay_holds_the_walker_in_place() {
    let (mut world, _db, _l) = walker_world(RepeatStyle::GoFirst, true);
    {
        // Give node 1 a 10 s pause.
        let r = &mut world.data.routes.routes[0];
        r.nodes[1].delay = 10;
    }
    place(&mut world);
    walk_one_leg(&mut world); // arrive at node 1, bank its delay

    // Immediately after arriving, no new leg should start.
    crate::game_loop::walkers::walker_tick(&mut world);
    assert!(
        world.objects.get_component::<Movement>(&WALKER).is_none(),
        "still serving the delay"
    );

    // Past the delay it sets off again.
    world.tick += 10 * crate::game_loop::walkers::WALKER_PERIOD + 1;
    crate::game_loop::walkers::walker_tick(&mut world);
    assert!(
        world.objects.get_component::<Movement>(&WALKER).is_some(),
        "delay served, walking again"
    );
}

#[test]
fn a_dead_walker_stops_permanently() {
    // `WalkingManager.onDeath` cancels the route.
    let (mut world, _db, _l) = walker_world(RepeatStyle::GoFirst, true);
    place(&mut world);
    world
        .objects
        .get_component_mut::<Vitals>(&WALKER)
        .unwrap()
        .dead = true;

    crate::game_loop::walkers::walker_tick(&mut world);

    assert!(state(&world).is_none(), "a dead walker drops its route");
    assert!(world.objects.get_component::<Movement>(&WALKER).is_none());
}

/// The real datapack: a walker template that actually ships with a route.
#[test]
fn real_dist_attaches_porter_remy() {
    let routes = RouteData::load_from(crate::data::DIST_GAME);
    let (_, r) = routes.route_for_npc(31356).expect("Porter Remy walks");
    assert_eq!(r.nodes.len(), 18);
    assert_eq!(r.repeat_style, RepeatStyle::GoFirst);
}
