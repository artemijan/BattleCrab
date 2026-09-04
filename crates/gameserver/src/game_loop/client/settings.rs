//! Client UI settings — the `settings/` client packets: the key layout the
//! player customised in-game (`RequestSaveKeyMapping` ex 0x22 →
//! `RequestKeyMapping` ex 0x21 → `ExUISetting`).
//!
//! Java stores the raw blob in a player variable under `StoreUISettings`
//! (**on** for this dist) and replays it verbatim; the port does the same, so
//! the layout survives a relogin with the character's other variables.

use crate::model::components::player::UI_KEY_MAPPING;
use crate::session::ClientSession;
use crate::world::World;

/// Java `RequestSaveKeyMapping`: `dataSize` then that many raw bytes, stored
/// as-is. A zero-length payload is ignored (Java's `_uiKeyMapping == null`).
pub(crate) fn handle_save_key_mapping(world: &mut World, client_id: u32, body: &[u8]) {
    let mut r = commons::network::PacketReader::new(body);
    let Some(size) = r.read_i32().filter(|&n| n > 0) else {
        return;
    };
    let Some(bytes) = r.read_bytes(size as usize) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    // Java joins the bytes with a tab (`SPLIT_VAR`) into one variable string.
    let encoded = bytes
        .iter()
        .map(|b| (*b as i8).to_string())
        .collect::<Vec<_>>()
        .join("\t");
    crate::game_loop::helpers::set_player_var(world, object_id, UI_KEY_MAPPING, encoded);
}

/// The stored layout for this client's character, decoded back to raw bytes
/// (empty when they never saved one) — Java `ExUISetting`'s payload.
pub(crate) fn stored_key_mapping(world: &World, client_id: u32) -> Vec<u8> {
    let object_id = match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => s.player_object_id(),
        _ => return Vec::new(),
    };
    decode_key_mapping(crate::game_loop::helpers::player_var(
        world,
        object_id,
        UI_KEY_MAPPING,
    ))
}

/// Decode the tab-joined variable back to the raw blob the client saved. Shared
/// with the enter-world burst, which reads the same variable off the freshly
/// loaded character bundle.
pub(crate) fn decode_key_mapping(stored: Option<&str>) -> Vec<u8> {
    stored
        .map(|stored| {
            stored
                .split('\t')
                .filter(|p| !p.is_empty())
                .filter_map(|p| p.parse::<i8>().ok())
                .map(|b| b as u8)
                .collect()
        })
        .unwrap_or_default()
}
