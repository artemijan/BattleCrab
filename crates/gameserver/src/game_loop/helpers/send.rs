//! Unicast send and system-message helpers.

use super::*;

/// Send one packet to a connected client — Java `GameClient.sendPacket`.
///
/// A direct `clients` lookup. Prefer this over [`send_to_player`] whenever the
/// handler already holds the client id, which packet handlers always do.
pub(crate) fn send_to_client(world: &World, client_id: u32, packet: Vec<u8>) {
    if let Some(&opcode) = packet.first() {
        crate::game_loop::dispatch::log_server_packet(world, opcode);
    }
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// Send one packet to the client driving `player_object_id` — Java
/// `Player.sendPacket`. No-op when that player is offline.
///
/// Keyed by **object id**, resolved through [`client_for_player`]'s reverse
/// index. Both this and [`send_to_client`] are O(1) now; prefer the latter
/// when the client id is already in hand, and reach for this when all you have
/// is the object id (scheduled tasks, effects resolved against a target).
pub(crate) fn send_to_player(world: &World, player_object_id: i32, packet: Vec<u8>) {
    if let Some(cid) = client_for_player(world, player_object_id) {
        send_to_client(world, cid, packet);
    }
}
/// `SystemMessage` to a connected client. Pass `&[]` for a message with no
/// substitution parameters.
pub(crate) fn send_sm_to_client(
    world: &World,
    client_id: u32,
    message_id: i16,
    params: &[server_packets::SmParam],
) {
    send_to_client(
        world,
        client_id,
        server_packets::system_message_with(message_id, params),
    );
}

/// `SystemMessage` to a player by object id — the scanning counterpart of
/// [`send_sm_to_client`]. Pass `&[]` when the message takes no parameters.
pub(crate) fn send_sm_to_player(
    world: &World,
    player_object_id: i32,
    message_id: i16,
    params: &[server_packets::SmParam],
) {
    send_to_player(
        world,
        player_object_id,
        server_packets::system_message_with(message_id, params),
    );
}

/// A **bare** `SystemMessage` — one that takes no substitution parameters — to
/// a connected client. Most system messages are bare, so this saves the `&[]`
/// at the call site; reach for [`send_sm_to_client`] when there are params.
pub(crate) fn send_sm_bare_to_client(world: &World, client_id: u32, message_id: i16) {
    send_sm_to_client(world, client_id, message_id, &[]);
}

/// A bare `SystemMessage` to a player by object id — the object-id counterpart
/// of [`send_sm_bare_to_client`].
pub(crate) fn send_sm_bare_to_player(world: &World, player_object_id: i32, message_id: i16) {
    send_sm_to_player(world, player_object_id, message_id, &[]);
}

/// Java `Player.sendMessage(String)` — free text delivered as the `$s1` system
/// message, the only way the client renders an arbitrary string as a message
/// line.
pub(crate) fn send_message(world: &World, client_id: u32, text: &str) {
    send_sm_to_client(
        world,
        client_id,
        server_packets::sm_ids::S1_TEXT,
        &[server_packets::SmParam::Text(text.to_string())],
    );
}

/// Java `Broadcast.toAllOnlinePlayers(text, false)` — a yellow announcement
/// line to every player in the world.
pub(crate) fn announce_to_all_online(world: &World, text: &str) {
    let packet =
        server_packets::creature_say(0, crate::enums::ChatType::Announcement, "", text, None);
    world.broadcast_to_all_online(&packet);
}

/// The full `SkillList` packet for an in-world player — their skill book plus
/// any transiently-granted clan skills (Java `sendSkillList`). `None` when the
/// object carries no skill book (not a live player). The single funnel every
/// `SkillList` resend goes through, so clan skills never fall off the list.
pub(crate) fn skill_list_packet(world: &World, object_id: i32) -> Option<Vec<u8>> {
    use crate::model::components::{ClanSkills, OptionSkills, SkillBook, SkillEnchants};
    let book = world.objects.get_component::<SkillBook>(&object_id)?;
    let empty = ClanSkills::default();
    let clan = world
        .objects
        .get_component::<ClanSkills>(&object_id)
        .unwrap_or(&empty);
    let no_options = OptionSkills::default();
    let options = world
        .objects
        .get_component::<OptionSkills>(&object_id)
        .unwrap_or(&no_options);
    let no_enchants = SkillEnchants::default();
    let enchants = world
        .objects
        .get_component::<SkillEnchants>(&object_id)
        .unwrap_or(&no_enchants);
    Some(crate::network::enter_world::skill_list(
        book,
        enchants,
        clan,
        options,
        &world.data,
    ))
}

/// Send a fresh `EtcStatusUpdate` to one player, built from their current state
/// (expertise grade penalties + silence/message-refusal), mirroring Java's
/// `sendPacket(new EtcStatusUpdate(this))` which reads it all off the player.
/// This is what redraws the grade-penalty and chat-block icons.
pub(crate) fn send_etc_status_update(world: &World, client_id: u32, object_id: i32) {
    use crate::model::components::{AdminFlags, ExpertisePenalty};
    let ep = world
        .objects
        .get_component::<ExpertisePenalty>(&object_id)
        .copied()
        .unwrap_or_default();
    // Java `EtcStatusUpdate._mask` bit 0x01 = message-refusal OR chat-ban OR
    // silence; the chat-block icon is the union.
    let silence = world
        .objects
        .get_component::<AdminFlags>(&object_id)
        .is_some_and(|f| f.silence)
        || crate::game_loop::punishment::is_chat_banned(world, object_id);
    let charges = world
        .objects
        .get_component::<Player>(&object_id)
        .map_or(0, |p| p.charges);
    let wp = world
        .objects
        .get_component::<model::components::WeightPenalty>(&object_id)
        .map_or(0, |w| w.level);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::enter_world::etc_status_update(
            charges, wp, ep.weapon, ep.armor, silence,
        ));
    }
}

/// The clean logout teardown for a player (Java `Disconnection.of`): persist,
/// despawn, drop the session.
pub(crate) fn disconnect_player(world: &mut World, target: i32) {
    let Some(tcid) = client_for_player(world, target) else {
        return;
    };
    if let Some(ClientSession::InGame(session)) = world.clients.remove(&tcid) {
        crate::game_loop::net::store_and_remove_player(world, target);
        session.send(server_packets::leave_world());
    }
}
/// Java `client.sendPacket(ActionFailed.STATIC_PACKET)` — the bare "I am not
/// doing that" reply, and the single most-sent packet in the port.
///
/// It is not optional politeness: the client arms a local "request in flight"
/// lock the moment it sends an action, and only a reply releases it. A handler
/// that returns without one leaves the player unable to click anything until
/// the next server packet happens to arrive. Every early return in a request
/// handler owes the client one of these.
///
/// [`send_sm_and_action_failed`] is the variant that explains *why* first.
pub(crate) fn send_action_failed(world: &World, client_id: u32) {
    send_to_client(world, client_id, server_packets::action_failed());
}

/// Send a `SystemMessage` + `ActionFailed` to one client — the standard
/// "request rejected" reply shape all over `Player.useMagic` /
/// `SkillCaster.checkUseConditions`.
pub(crate) fn send_sm_and_action_failed(
    world: &World,
    client_id: u32,
    message_id: i16,
    params: &[server_packets::SmParam],
) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::system_message_with(message_id, params));
        cs.send(server_packets::action_failed());
    }
}
