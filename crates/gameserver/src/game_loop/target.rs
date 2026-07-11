//! Target selection handlers (`Action`, `RequestTargetCanceld`) and the
//! `Player.setTarget` port.

use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::broadcast_to_others;
use super::skills::cast::abort_cast;

/// Port of `clientpackets/Action.runImpl`, narrowed to the single-click
/// (`action_id == 0`) select-a-player case — the only targetable `WorldObject`
/// kind that exists yet (no NPCs/items until G8+). Clicking yourself goes
/// through the same path (Java routes self-clicks through `PlayerAction`
/// like any other player target). Shift-click (`action_id == 1`, examine
/// window) and the flood-protector/bot-penalty/trade/instance guards Java
/// has are all skipped as out of scope (no trade/instances/bot-detection in
/// the Rust port yet). Always terminates with `ActionFailed`, matching
/// `WorldObject.onAction`'s convention.
pub(crate) fn handle_action(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::Action::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();

    if world.players.contains_key(&pkt.object_id) {
        set_target(world, client_id, object_id, Some(pkt.object_id));
    }
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::action_failed());
    }
}

/// Port of `clientpackets/RequestTargetCanceld.runImpl`: Esc aborts an
/// in-flight cast (Java `abortAllSkillCasters`, regardless of the
/// `targetLost` flag), then clears the target if `targetLost`. The
/// locked-target/queued-skill/air-ship guards are features that don't exist
/// yet.
pub(crate) fn handle_request_target_canceld(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestTargetCanceld::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    abort_cast(world, object_id);
    if !pkt.target_lost {
        return;
    }
    set_target(world, client_id, object_id, None);
}

/// Port of `Player.setTarget`'s core, narrowed to Player targets (no
/// NPCs/vehicles/party checks yet — see the handlers above). Same-target
/// re-click is a no-op (Java only re-sends `ValidateLocation`, a cosmetic
/// target-position correction we skip).
pub(crate) fn set_target(world: &mut World, client_id: u32, object_id: i32, new_target: Option<i32>) {
    let Some(player) = world.players.get(&object_id) else { return };
    if player.target == new_target {
        return;
    }

    // Prevents /target exploiting: reject targets too far away in Z.
    let new_target = new_target.filter(|&t| {
        let Some(target_player) = world.players.get(&t) else { return false };
        (target_player.z - player.z).abs() <= 1000
    });
    if player.target == new_target {
        return;
    }

    let (px, py, pz) = (player.x, player.y, player.z);
    if let Some(t) = new_target {
        let Some(target_player) = world.players.get(&t) else { return };
        let (max_hp, cur_hp) = (target_player.max_hp, target_player.cur_hp as i32);
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::my_target_selected(t));
            cs.send(server_packets::status_update(
                t,
                &[
                    (server_packets::status_update_type::MAX_HP, max_hp),
                    (server_packets::status_update_type::CUR_HP, cur_hp),
                ],
            ));
        }
        broadcast_to_others(world, object_id, &server_packets::target_selected(object_id, t, px, py, pz));
    } else {
        // Java's clear path uses broadcastPacket(includeSelf=true): the
        // deselecting client must get TargetUnselected too, or its UI keeps
        // the target locked.
        let pkt = server_packets::target_unselected(object_id, px, py, pz);
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(pkt.clone());
        }
        broadcast_to_others(world, object_id, &pkt);
    }

    if let Some(player) = world.players.get_mut(&object_id) {
        player.target = new_target;
    }
}

