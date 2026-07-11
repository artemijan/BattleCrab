//! Target selection handlers (`Action`, `RequestTargetCanceld`), the
//! `Player.setTarget` port, and (G8) the `NpcAction` interact path — talking
//! to a targeted NPC opens its chat window.

use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::broadcast_to_others;
use super::skills::cast::abort_cast;

/// `Npc.INTERACTION_DISTANCE`.
const INTERACTION_DISTANCE: f64 = 250.0;

/// Port of `clientpackets/Action.runImpl` for the single-click case
/// (`action_id == 0`), now resolving both players and NPCs. Java's dispatch:
/// a click on something that isn't your target selects it
/// (`Player.setTarget`); a second click on an NPC target interacts
/// (`NpcAction` — attack for monsters (G9), chat window for the rest).
/// Shift-click (`action_id == 1`) and the flood/bot/trade guards stay out of
/// scope. Always terminates with `ActionFailed`, matching
/// `WorldObject.onAction`'s convention.
pub(crate) fn handle_action(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::Action::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();

    if world.players.contains_key(&pkt.object_id) {
        set_target(world, client_id, object_id, Some(pkt.object_id));
    } else if let Some(npc) = world.npcs.get(&pkt.object_id) {
        // Java `Npc.canTarget` → `WorldObject.isTargetable` (template flag).
        let targetable = npc.template(world).is_none_or(|t| t.targetable);
        if targetable {
            let already_targeted = world.players.get(&object_id).and_then(|p| p.target) == Some(pkt.object_id);
            if already_targeted {
                interact_with_npc(world, client_id, object_id, pkt.object_id);
            } else {
                set_target(world, client_id, object_id, Some(pkt.object_id));
            }
        }
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
    // Esc also ends an attack loop (Java: ATTACK intention → ACTIVE).
    if let Some(p) = world.players.get_mut(&object_id) {
        p.intent = None;
    }
    if !pkt.target_lost {
        return;
    }
    set_target(world, client_id, object_id, None);
}

/// What `set_target` needs to know about a prospective target, whichever
/// world registry it lives in.
struct TargetInfo {
    z: i32,
    max_hp: i32,
    cur_hp: i32,
    /// `MyTargetSelected` color: level diff for auto-attackable targets.
    color: i16,
    is_npc: bool,
    heading: i32,
    x: i32,
    y: i32,
}

fn target_info(world: &World, viewer_level: i32, target_id: i32) -> Option<TargetInfo> {
    if let Some(p) = world.players.get(&target_id) {
        return Some(TargetInfo {
            z: p.z,
            max_hp: p.max_hp,
            cur_hp: p.cur_hp as i32,
            color: 0,
            is_npc: false,
            heading: p.heading,
            x: p.x,
            y: p.y,
        });
    }
    let npc = world.npcs.get(&target_id)?;
    let t = npc.template(world)?;
    Some(TargetInfo {
        z: npc.z,
        max_hp: npc.max_hp,
        cur_hp: npc.cur_hp as i32,
        color: if t.is_auto_attackable() { (viewer_level - t.level) as i16 } else { 0 },
        is_npc: true,
        heading: npc.heading,
        x: npc.x,
        y: npc.y,
    })
}

/// Port of `Player.setTarget`'s core over players and NPCs (no
/// vehicles/party checks yet). Same-target re-click is handled by the caller
/// (`handle_action` routes it to the interact path for NPCs; for players
/// Java only re-sends `ValidateLocation`, which we skip).
pub(crate) fn set_target(world: &mut World, client_id: u32, object_id: i32, new_target: Option<i32>) {
    let Some(player) = world.players.get(&object_id) else { return };
    if player.target == new_target {
        return;
    }
    let viewer_level = player.level;

    // Prevents /target exploiting: reject targets too far away in Z.
    let new_target = new_target.filter(|&t| {
        target_info(world, viewer_level, t).map(|i| (i.z - player.z).abs() <= 1000).unwrap_or(false)
    });
    if player.target == new_target {
        return;
    }

    let (px, py, pz) = (player.x, player.y, player.z);
    if let Some(t) = new_target {
        let Some(info) = target_info(world, viewer_level, t) else { return };
        if let Some(cs) = world.clients.get(&client_id) {
            // Java sends ValidateLocation for any creature target; the
            // player→player path predates it and skips the (cosmetic)
            // correction, so it stays NPC-only here.
            if info.is_npc {
                cs.send(server_packets::validate_location(t, info.x, info.y, info.z, info.heading));
            }
            cs.send(server_packets::my_target_selected(t, info.color));
            cs.send(server_packets::status_update(
                t,
                &[
                    (server_packets::status_update_type::MAX_HP, info.max_hp),
                    (server_packets::status_update_type::CUR_HP, info.cur_hp),
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

/// The `NpcAction` interact branch (second click on the current NPC target):
/// monsters start the auto-attack loop (G9); everything else in interaction
/// range opens its chat window (`Npc.showChatWindow`). Out of range does
/// nothing for dialogs — Java's walk-into-range AI intent is only ported for
/// the attack path.
fn interact_with_npc(world: &mut World, client_id: u32, object_id: i32, npc_object_id: i32) {
    let Some(player) = world.players.get(&object_id) else { return };
    let Some(npc) = world.npcs.get(&npc_object_id) else { return };
    let Some(t) = npc.template(world) else { return };
    if t.is_auto_attackable() {
        if !player.dead {
            super::combat::start_attack_intent(world, client_id, object_id, npc_object_id);
        }
        return;
    }
    // `Npc.canInteract`: plain 3D distance vs INTERACTION_DISTANCE.
    let (dx, dy, dz) = ((npc.x - player.x) as f64, (npc.y - player.y) as f64, (npc.z - player.z) as f64);
    if dx * dx + dy * dy + dz * dz > INTERACTION_DISTANCE * INTERACTION_DISTANCE {
        return;
    }
    // `Npc.showChatWindow(player, 0)`.
    if !t.talkable {
        return;
    }
    let html = load_chat_window_html(&world.data.root, &t.type_name, t.id)
        .unwrap_or_else(|| "<html><body>My Text is missing:<br></body></html>".to_string())
        .replace("%objectId%", &npc_object_id.to_string())
        .replace("%npcname%", &t.name);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(npc_object_id, &html));
    }
}

/// `getHtmlPath` across the instance classes this slice can meet: each
/// subclass roots its dialogs in its own `data/html/<dir>/` (no fallback —
/// Java shows the "text is missing" stub); plain `Folk`/`Npc` use
/// `data/html/default/` falling back to `npcdefault.htm`. Java streams these
/// through `HtmCache`; a per-interaction disk read is fine at this scale
/// (TODO: cache if profiling ever cares).
fn load_chat_window_html(root: &str, type_name: &str, npc_id: i32) -> Option<String> {
    let dir = match type_name {
        "Merchant" => Some("merchant"),
        "Fisherman" => Some("fisherman"),
        "Teleporter" => Some("teleporter"),
        "Warehouse" => Some("warehouse"),
        "Guard" => Some("guard"),
        "PetManager" => Some("petmanager"),
        t if t.starts_with("VillageMaster") => Some("villagemaster"),
        _ => None,
    };
    match dir {
        Some(dir) => std::fs::read_to_string(format!("{root}data/html/{dir}/{npc_id}.htm")).ok(),
        None => std::fs::read_to_string(format!("{root}data/html/default/{npc_id}.htm"))
            .or_else(|_| std::fs::read_to_string(format!("{root}data/html/npcdefault.htm")))
            .ok(),
    }
}
