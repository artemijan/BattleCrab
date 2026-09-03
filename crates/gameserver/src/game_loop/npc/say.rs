//! NPC chat-say broadcasts (moved from helpers).

use crate::game_loop::net::broadcast;
use crate::model::npc::Npc;
use crate::world::World;

/// `npc.broadcastPacket(new NpcSay(npc, NPC_GENERAL, npcStringId))` — an NPC
/// says a line to everyone nearby.
///
/// Lifted out of `QuestCtx` so a **boss script** can use it: the body only ever
/// needed the world and the speaker, and the quest coupling was incidental.
/// `QuestCtx::npc_say` now delegates here.
pub(crate) fn npc_say(world: &World, npc_oid: i32, npc_string_id: i32) {
    npc_say_param(world, npc_oid, npc_string_id, None);
}

/// [`npc_say`] with the line's single `$s1` substitution — Java
/// `broadcastSay(NPC_GENERAL, id, param)`.
///
/// `None` is not the same as `Some("")`: the parameterless packet is a
/// different opcode payload, and a client fed an empty parameter draws the
/// placeholder rather than the line.
pub(crate) fn npc_say_param(world: &World, npc_oid: i32, npc_string_id: i32, param: Option<&str>) {
    let Some(npc) = world.objects.get_component::<Npc>(&npc_oid) else {
        return;
    };
    let pkt = match param {
        Some(p) => {
            crate::network::server_packets::npc_say_param(npc_oid, npc.npc_id, npc_string_id, p)
        }
        None => crate::network::server_packets::npc_say(npc_oid, npc.npc_id, npc_string_id),
    };
    broadcast::broadcast_from(world, npc_oid, &pkt);
}

/// `npc.broadcastSay(NPC_GENERAL, text)` — a literal-text chat bubble.
pub(crate) fn npc_say_text(world: &World, npc_oid: i32, text: &str) {
    let Some(npc) = world.objects.get_component::<Npc>(&npc_oid) else {
        return;
    };
    let pkt = crate::network::server_packets::npc_say_text(npc_oid, npc.npc_id, text);
    broadcast::broadcast_from(world, npc_oid, &pkt);
}
