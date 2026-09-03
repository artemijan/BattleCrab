//! `bypasshandlers/Observation` — the Broadcasting Tower's spectator seats.
//!
//! Twelve htmls under `data/html/observation/` bind `observe 18/19/20`, and the
//! verb was in no handler table: every button on the Coliseum tower did
//! nothing.

use super::*;
use crate::game_loop::character::inventory;

use crate::game_loop::space::observation;
use crate::model::components::{Observing, Position};

const TOWER_ID: i32 = 31031;
const TOWER_OID: i32 = 5301;
const PLAYER: i32 = 9801;
const CID: u32 = 1;
/// `observe 18` — the first Coliseum row, 80 adena.
const COLISEUM_A: (i32, i32, i32) = (148416, 46724, -3000);
const SEAT_COST: i64 = 80;

fn tower_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = test_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4900_0000..0x4900_0200;
    let mut t = crate::data::npc_data::default_template(TOWER_ID);
    t.type_name = "BroadcastingTower".into();
    t.name = "Broadcasting Tower".into();
    world.data.npc_data.insert_for_test(t);
    add_test_npc(
        &mut world,
        TOWER_OID,
        TOWER_ID,
        "BroadcastingTower",
        70,
        0,
        0,
        0,
    );
    let rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    (world, rx)
}

fn observe(world: &mut World, param: &str) {
    handle_request_bypass_to_server(
        world,
        CID,
        &bypass_body(&format!("npc_{TOWER_OID}_observe {param}")),
    );
}

fn pos_of(world: &World) -> (i32, i32, i32) {
    let p = world.objects.get_component::<Position>(&PLAYER).unwrap();
    (p.x, p.y, p.z)
}

fn adena(world: &World) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&PLAYER)
        .map(|i| i.adena())
        .unwrap_or(0)
}

/// Pay the fee, get moved to the seat and put into free-look — then come back
/// to exactly where you started.
#[test]
fn a_tower_seat_is_bought_entered_and_left() {
    let (mut world, mut rx) = tower_world();
    inventory::add_inventory_item(&mut world, PLAYER, 57, 1000).unwrap();
    let home = pos_of(&world);
    drain(&mut rx);

    observe(&mut world, "18");

    assert_eq!(adena(&world), 1000 - SEAT_COST, "the seat is paid for");
    assert!(
        world.objects.has_component::<Observing>(&PLAYER),
        "and the viewer is observing"
    );
    // X/Y exactly; Z is compared loosely because `teleport_player` snaps the
    // arrival to the geodata surface, as every teleport in the port does.
    let seat = pos_of(&world);
    assert_eq!((seat.0, seat.1), (COLISEUM_A.0, COLISEUM_A.1), "the seat");
    assert!(
        (seat.2 - COLISEUM_A.2).abs() <= 32,
        "and at its height, geo-snapped: {seat:?}"
    );
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::OBSERVER_START),
        "the client is told to enter free-look"
    );

    observation::handle_observer_return(&mut world, CID, PLAYER);

    assert!(
        !world.objects.has_component::<Observing>(&PLAYER),
        "no longer observing"
    );
    let back = pos_of(&world);
    assert_eq!(
        (back.0, back.1),
        (home.0, home.1),
        "back where they started"
    );
    assert!(
        (back.2 - home.2).abs() <= 32,
        "at the same height: {back:?}"
    );
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::OBSERVER_END),
        "with the matching packet"
    );
}

/// `doObserve` enters the mode **only** if the fee could be paid.
#[test]
fn a_seat_that_cannot_be_paid_for_is_not_entered() {
    let (mut world, mut rx) = tower_world();
    inventory::add_inventory_item(&mut world, PLAYER, 57, SEAT_COST - 1).unwrap();
    let home = pos_of(&world);
    drain(&mut rx);

    observe(&mut world, "18");

    assert!(!world.objects.has_component::<Observing>(&PLAYER));
    assert_eq!(pos_of(&world), home, "nobody moved");
    assert_eq!(adena(&world), SEAT_COST - 1, "and nothing was taken");
}

/// A summon would be stranded by the teleport, so Java refuses first.
#[test]
fn a_player_with_a_summon_is_refused() {
    let (mut world, mut rx) = tower_world();
    inventory::add_inventory_item(&mut world, PLAYER, 57, 1000).unwrap();
    // Park a servitor on the player through the same link the orders use.
    let mut t = crate::data::npc_data::default_template(14799);
    t.type_name = "Servitor".into();
    world.data.npc_data.insert_for_test(t);
    crate::game_loop::servitor::summon_servitor(&mut world, PLAYER, 14799, 283, 0, 0, 0).unwrap();
    drain(&mut rx);

    observe(&mut world, "18");

    assert!(
        !world.objects.has_component::<Observing>(&PLAYER),
        "refused while a summon is out"
    );
    assert_eq!(adena(&world), 1000, "and nothing was charged");
}

/// The index is range-checked against the table, and the bypass only answers
/// from an actual tower — a forged one at another NPC buys nothing.
#[test]
fn a_bad_index_or_a_non_tower_buys_nothing() {
    let (mut world, _rx) = tower_world();
    inventory::add_inventory_item(&mut world, PLAYER, 57, 1000).unwrap();

    observe(&mut world, "99");
    assert!(
        !world.objects.has_component::<Observing>(&PLAYER),
        "no row 99"
    );
    observe(&mut world, "not-a-number");
    assert!(
        !world.objects.has_component::<Observing>(&PLAYER),
        "no index"
    );
    assert_eq!(adena(&world), 1000);

    // The same command at an ordinary NPC.
    let other = TOWER_OID + 1;
    add_test_npc(&mut world, other, 30001, "Merchant", 70, 0, 0, 0);
    handle_request_bypass_to_server(
        &mut world,
        CID,
        &bypass_body(&format!("npc_{other}_observe 18")),
    );
    assert!(
        !world.objects.has_component::<Observing>(&PLAYER),
        "only a BroadcastingTower sells seats"
    );
    assert_eq!(adena(&world), 1000);
}

/// `Action.runImpl`'s observer gate: a spectator clicks nothing. Without it the
/// free-look camera could target and act on whatever it is pointed at.
///
/// The target has to be **beside the seat**, not beside the tower: entering the
/// mode teleports the viewer to the Coliseum, so a click on the tower they just
/// left would be refused for range and prove nothing.
#[test]
fn a_spectator_cannot_click_anything() {
    let (mut world, mut rx) = tower_world();
    inventory::add_inventory_item(&mut world, PLAYER, 57, 1000).unwrap();
    observe(&mut world, "18");
    assert!(world.objects.has_component::<Observing>(&PLAYER));
    let seat = pos_of(&world);
    let neighbour = TOWER_OID + 5;
    add_test_npc(
        &mut world,
        neighbour,
        30001,
        "Merchant",
        70,
        seat.0 + 20,
        seat.1,
        seat.2,
    );
    drain(&mut rx);

    handle_action(&mut world, CID, &action_body(neighbour, 0));
    assert!(
        world
            .objects
            .get_component::<TargetRef>(&PLAYER)
            .and_then(|t| t.0)
            .is_none(),
        "a spectator's click selects nothing"
    );

    // …and the very same click lands once they are out of the mode, which is
    // what makes the assertion above about observing rather than about range.
    observation::handle_observer_return(&mut world, CID, PLAYER);
    crate::game_loop::death::teleport_player(&mut world, PLAYER, seat.0, seat.1, seat.2);
    handle_action(&mut world, CID, &action_body(neighbour, 0));
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&PLAYER)
            .and_then(|t| t.0),
        Some(neighbour),
        "the same click works when not observing"
    );
}
