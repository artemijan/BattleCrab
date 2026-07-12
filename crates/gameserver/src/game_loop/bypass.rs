//! `RequestBypassToServer` (0x23) routing — the server side of every HTML
//! `action="bypass -h …"` button. Port of
//! `clientpackets/RequestBypassToServer.runImpl` narrowed to the command
//! families this slice serves: `npc_<objectId>_<command>` (dialog verbs on a
//! specific NPC) and the bare `Quest …` links the quest htmls use.
//!
//! Deliberate deviations from Java (documented in the G11 plan):
//! - `validateHtmlAction` (the sent-action anti-cheat registry) is not
//!   ported. Bare commands resolve their NPC through the [`LastFolkNpc`]
//!   component instead of the recorded html origin id, and every route
//!   re-checks `INTERACTION_DISTANCE` — the same guard Java applies on top
//!   of validation.
//! - An empty bypass logs and drops instead of force-disconnecting (the
//!   G10 `Say2` precedent for malformed-but-harmless client input).
//! - Unhandled commands log-and-drop (Java logs too; `admin_`, `_bbs`,
//!   `item_`, menu/manor selects and the rest of the prefix zoo wait for
//!   their systems).

use tracing::warn;

use crate::model::components::LastFolkNpc;
use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

use super::target::can_interact;

pub(crate) fn handle_request_bypass_to_server(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(command) = cp::read_bypass_command(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();

    if command.is_empty() {
        warn!("Bypass: client {client_id} sent empty bypass, dropped.");
        return;
    }

    if let Some(rest) = command.strip_prefix("npc_") {
        // `npc_<objectId>_<command>`: Java parses the id between the two
        // underscores and requires a command tail to act at all (the
        // ActionFailed terminator is sent regardless).
        if let Some((id_str, npc_command)) = rest.split_once('_') {
            if let Ok(npc_object_id) = id_str.parse::<i32>() {
                if world.objects.has_component::<crate::model::npc::Npc>(&npc_object_id)
                    && can_interact(world, object_id, npc_object_id)
                {
                    npc_bypass(world, client_id, object_id, npc_object_id, npc_command);
                }
            }
        }
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
    } else if command == "Quest" || command.starts_with("Quest ") {
        // Bare quest link (`bypass -h Quest <Name> [<event>]`) — the form
        // the quest/script htmls use. Java recovers the NPC from the
        // validateHtmlAction origin; we use the last folk NPC (set on every
        // NPC click) and re-check range like Java does after validation.
        let Some(&LastFolkNpc(npc_object_id)) = world.objects.get_component::<LastFolkNpc>(&object_id)
        else {
            return;
        };
        if world.objects.has_component::<crate::model::npc::Npc>(&npc_object_id)
            && can_interact(world, object_id, npc_object_id)
        {
            npc_bypass(world, client_id, object_id, npc_object_id, &command);
        }
    } else {
        warn!("Bypass: client {client_id} sent unhandled bypass [{command}].");
    }
}

/// Port of `Npc.onBypassFeedback` + the `VillageMaster` override: route an
/// NPC-scoped command by its first token. The caller has already verified
/// the NPC exists and is within `INTERACTION_DISTANCE`.
fn npc_bypass(world: &mut World, client_id: u32, object_id: i32, npc_object_id: i32, command: &str) {
    let verb = command.split(' ').next().unwrap_or("");
    match verb {
        "Quest" => super::quests::quest_link(world, client_id, object_id, npc_object_id, command),
        // `VillageMaster.onBypassFeedback` verbs — gated on the instance
        // class like Java's subclass override (`type_name` check stands in
        // for `instanceof VillageMaster`).
        "create_clan" if is_village_master(world, npc_object_id) => {
            let args = command.strip_prefix("create_clan").unwrap_or("").trim();
            super::clans::handle_create_clan(world, client_id, object_id, args);
        }
        _ => {
            warn!("Bypass: unhandled npc bypass verb [{verb}] in [{command}].");
        }
    }
}

fn is_village_master(world: &World, npc_object_id: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_object_id)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.type_name.starts_with("VillageMaster"))
}
