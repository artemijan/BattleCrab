//! `bypasshandlers/Observation` — the Broadcasting Tower's spectator seats, and
//! `Player.enterObserverMode` / `leaveObserverMode` behind them.
//!
//! A tower sells a view: pay the fee, get teleported to a fixed camera point
//! and put into free-look mode until the client asks to come back. Java uses it
//! for three things — the Coliseum stands, the nine castle siege overlooks, and
//! the Oracle Dusk/Dawn galleries — all off one table of 31 positions.
//!
//! **Only the Coliseum's three seats are reachable on this dist.** Exactly one
//! `BroadcastingTower` npc ships (31031) and the twelve htmls under
//! `data/html/observation/` bind `observe 18`, `19` and `20` — the Coliseum
//! rows. `observesiege` and `observeoracle` appear in no html here, and the
//! Oracle positions belong to Seven Signs, which this dist drops entirely. Both
//! verbs are ported anyway: they are two arms over the same table, and leaving
//! them out would mean explaining the absence at every future audit.
//!
//! This is the **plain** observer mode, not the Olympiad's
//! (`olympiad::observer`). Java keeps one `_observerMode` flag for both but two
//! enter/leave pairs and two client packets, so the port keeps two components
//! for the same reason — the Olympiad viewer is scoped into a match instance
//! and answers `ExOlympiadMode`, this one is not and answers `ObservationMode`.

use crate::game_loop::helpers::{
    send_action_failed, send_message, send_sm_bare_to_client, send_to_client,
};
use crate::game_loop::space::position::maybe_position;
use crate::model::components::Observing;
use crate::network::server_packets::{self, sm_ids};
use crate::world::World;

/// Java's `LOCATIONS` table: `(x, y, z, cost)`. The index is the bypass's
/// second token, so the order is load-bearing — a shifted row sells the wrong
/// view.
const LOCATIONS: &[(i32, i32, i32, i64)] = &[
    // Gludio
    (-18347, 114000, -2360, 500),
    (-18347, 113255, -2447, 500),
    // Dion
    (22321, 155785, -2604, 500),
    (22321, 156492, -2627, 500),
    // Giran
    (112000, 144864, -2445, 500),
    (112657, 144864, -2525, 500),
    // Innadril
    (116260, 244600, -775, 500),
    (116260, 245264, -721, 500),
    // Oren
    (78100, 36950, -2242, 500),
    (78744, 36950, -2244, 500),
    // Aden
    (147457, 9601, -233, 500),
    (147457, 8720, -252, 500),
    // Goddard
    (147542, -43543, -1328, 500),
    (147465, -45259, -1328, 500),
    // Rune
    (20598, -49113, -300, 500),
    (18702, -49150, -600, 500),
    // Schuttgart
    (77541, -147447, 353, 500),
    (77541, -149245, 353, 500),
    // Coliseum — indexes 18, 19, 20, the only three this dist's htmls bind.
    (148416, 46724, -3000, 80),
    (149500, 46724, -3000, 80),
    (150511, 46724, -3000, 80),
    // Dusk (Seven Signs)
    (-77200, 88500, -4800, 500),
    (-75320, 87135, -4800, 500),
    (-76840, 85770, -4800, 500),
    (-76840, 85770, -4800, 500),
    (-79950, 85165, -4800, 500),
    // Dawn (Seven Signs)
    (-79185, 112725, -4300, 500),
    (-76175, 113330, -4300, 500),
    (-74305, 111965, -4300, 500),
    (-75915, 110600, -4300, 500),
    (-78930, 110005, -4300, 500),
];

/// `Observation.useBypass`. `verb` is already lower-cased by the bypass router.
pub(crate) fn handle_bypass(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    npc_object_id: i32,
    verb: &str,
    args: &str,
) {
    // `if (!(target instanceof BroadcastingTower)) return false` — the seats
    // are the tower's, and a bypass forged at another NPC buys nothing.
    let is_tower = crate::game_loop::helpers::npc_id_of(world, npc_object_id)
        .and_then(|id| world.data.npc_data.get(id))
        .is_some_and(|t| t.type_name == "BroadcastingTower");
    if !is_tower {
        return;
    }
    // A summon would be left behind by the teleport.
    if crate::game_loop::servitor::servitor_of(world, player_oid).is_some()
        || crate::game_loop::servitor::pet_of(world, player_oid).is_some()
    {
        send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::YOU_MAY_NOT_OBSERVE_A_SIEGE_WITH_A_SERVITOR_SUMMONED,
        );
        return;
    }
    if world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
        .is_some_and(|p| p.on_event)
    {
        send_message(world, client_id, "Cannot use while current Event");
        return;
    }

    // `Integer.parseInt(command.split(" ")[1])` — Java logs and bails on a
    // non-numeric tail, and range-checks the index against the table.
    let Some(param) = args
        .split_whitespace()
        .next()
        .and_then(|a| a.parse::<usize>().ok())
    else {
        return;
    };
    let Some(&(x, y, z, cost)) = LOCATIONS.get(param) else {
        return;
    };

    if verb == "observesiege"
        && crate::game_loop::siege::active_siege_castle_at(world, x, y, z).is_none()
    {
        send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::OBSERVATION_IS_ONLY_POSSIBLE_DURING_A_SIEGE,
        );
        return;
    }

    do_observe(world, client_id, player_oid, x, y, z, cost);
}

/// Java's `doObserve`: charge the fee, and only on success enter the mode.
/// `ActionFailed` goes out either way — the click is answered whether or not
/// the player could afford it.
fn do_observe(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    x: i32,
    y: i32,
    z: i32,
    cost: i64,
) {
    if reduce_adena(world, player_oid, cost) {
        enter_observer_mode(world, client_id, player_oid, x, y, z);
        // `player.sendItemList(false)` — the fee left the bag.
        crate::game_loop::items::handle_request_item_list(world, client_id);
    }
    send_action_failed(world, client_id);
}

/// `Player.enterObserverMode(loc)`.
///
/// SKIP(census): Java's `stopEffects(AbnormalType.HIDE)` — the port has no HIDE
/// abnormal (no skill on this dist grants it), so there is nothing to stop.
fn enter_observer_mode(world: &mut World, client_id: u32, player_oid: i32, x: i32, y: i32, z: i32) {
    // `setLastLocation()` — where `leaveObserverMode` puts them back.
    let Some(here) = maybe_position(world, player_oid) else {
        return;
    };
    world.objects.add_components(
        &player_oid,
        Observing {
            return_pos: (here.x, here.y, here.z),
        },
    );
    send_to_client(world, client_id, server_packets::observation_mode(x, y, z));
    crate::game_loop::death::teleport_player(world, player_oid, x, y, z);
    crate::game_loop::character::player_info::broadcast_user_info(world, player_oid);
}

/// `ObserverReturn` (0xC1) → `Player.leaveObserverMode`: back to where they
/// started, out of free-look, and visible again.
///
/// A player who is not observing is ignored, exactly as Java's
/// `if (player.inObserverMode())` guard does — which matters because the
/// Olympiad viewer answers a *different* packet and must not be dropped by
/// this one.
pub(crate) fn handle_observer_return(world: &mut World, client_id: u32, player_oid: i32) {
    let Some(observing) = world
        .objects
        .get_component::<Observing>(&player_oid)
        .copied()
    else {
        return;
    };
    world.objects.remove_component::<Observing>(&player_oid);
    let (x, y, z) = observing.return_pos;
    crate::game_loop::combat::target::drop_target_notify(world, player_oid);
    crate::game_loop::death::teleport_player(world, player_oid, x, y, z);
    send_to_client(
        world,
        client_id,
        server_packets::observation_return(x, y, z),
    );
    crate::game_loop::character::player_info::broadcast_user_info(world, player_oid);
}

/// Java `Player.inObserverMode()` for the plain flavour.
pub(crate) fn is_observing(world: &World, player_oid: i32) -> bool {
    world.objects.has_component::<Observing>(&player_oid)
}

fn reduce_adena(world: &mut World, player_oid: i32, count: i64) -> bool {
    use crate::data::item_data::ADENA_ID;
    use crate::model::inventory::Inventory;
    if count <= 0 {
        return true;
    }
    let enough = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .is_some_and(|inv| inv.adena() >= count);
    if !enough {
        return false;
    }
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player_oid) {
        inv.remove_item(ADENA_ID, count);
    }
    true
}
