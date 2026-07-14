//! GM utility, comms and session commands — `AdminAdmin` (GM list, diet,
//! world-chat), `AdminOnline`, `AdminTargetSay`, `AdminMessages`,
//! `AdminAnnouncements`, `AdminHtml`, `AdminDebug`, `AdminTest`, the
//! `AdminMenu` action buttons, and `AdminKick`'s `//kick_non_gm`.
//!
//! Subsystem-blocked siblings (`//snoop`, `//reload`, `//setconfig`/`//set_mod`/
//! `//config_server`, the hero/olympiad commands, `//ban_menu`/`//unban_menu`,
//! `//fight_calculator`, the missing-html scanners) stay on the not-implemented
//! path — they need chat-snoop, live-config, olympiad or punishment systems the
//! server has not ported.

use crate::model::components::{AdminFlags, Position, PartyRef};
use crate::model::npc::Npc;
use crate::model::Player;
use crate::network::server_packets::{self, sm_ids};
use crate::session::ClientSession;
use crate::world::World;

use super::{current_target, send_message, send_sm};

/// `AdminAdmin`'s `//gmliston` / `//gmlistoff` — register/unregister from the GM
/// list. There is no `//gmlist` consumer yet, so this messages + re-shows the GM
/// menu (the hidden flag is a no-op; see `flags::register_gm`).
pub(super) fn admin_gmlist(world: &mut World, client_id: u32, on: bool) {
    send_message(world, client_id, if on { "Registered into GM list." } else { "Removed from GM list." });
    super::menu::show_admin_html(world, client_id, "gm_menu.htm");
}

/// `AdminAdmin`'s `//diet on|off` — toggle weight-overload immunity
/// (`AdminFlags.diet`; honored once the overload calc lands).
pub(super) fn admin_diet(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let mut flags = world.objects.get_component::<AdminFlags>(&object_id).copied().unwrap_or_default();
    match args.first().copied() {
        Some("on") => flags.diet = true,
        Some("off") => flags.diet = false,
        _ => {
            send_message(world, client_id, "Usage: //diet on|off");
            return;
        }
    }
    world.objects.add_components(&object_id, flags);
    send_message(world, client_id, if flags.diet { "Diet mode on." } else { "Diet mode off." });
}

/// `AdminAdmin`'s `//worldchat shout <message>` — broadcast to every online
/// player. Java uses `ChatType.WORLD`; the port's `ChatType` has no WORLD, so
/// this sends the message as a system-message text line (the same simplification
/// as `//announce`).
pub(super) fn admin_worldchat(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    match args.first().copied() {
        Some("shout") => {
            let text = args[1..].join(" ");
            if text.is_empty() {
                send_message(world, client_id, "Usage: //worldchat shout <message>");
                return;
            }
            let name = world.objects.get_component::<Player>(&object_id).map(|p| p.name.clone()).unwrap_or_default();
            broadcast_text(world, &format!("{name}: {text}"));
        }
        _ => send_message(world, client_id, "Usage: //worldchat shout <message>"),
    }
}

/// `AdminOnline`'s `//online` — online-player counts. IP/offline-mode stats are
/// dropped (no per-client IP is tracked here).
pub(super) fn admin_online(world: &mut World, client_id: u32) {
    let mut total = 0;
    let mut gms = 0;
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            total += 1;
            if world.objects.get_component::<Player>(&s.player_object_id()).is_some_and(|p| p.is_gm(&world.data)) {
                gms += 1;
            }
        }
    }
    send_message(world, client_id, "=== Online ===");
    send_message(world, client_id, &format!("Players online: {total} (GMs: {gms})"));
}

/// `AdminTargetSay`'s `//targetsay <text>` — make the current target say `text`.
pub(super) fn admin_targetsay(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(target) = current_target(world, object_id) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    if args.is_empty() {
        send_message(world, client_id, "Usage: //targetsay <text>");
        return;
    }
    let text = args.join(" ");
    // Java uses GENERAL for players and NPC_GENERAL for NPCs; the port's
    // `ChatType` has no NPC variant, so both use General (documented).
    let name = if let Some(p) = world.objects.get_component::<Player>(&target) {
        p.name.clone()
    } else if let Some(npc) = world.objects.get_component::<Npc>(&target) {
        world.data.npc_data.get(npc.npc_id).map(|t| t.name.clone()).unwrap_or_default()
    } else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let say = server_packets::creature_say(target, crate::enums::ChatType::General, &name, &text, None);
    super::helpers::broadcast_including_self(world, target, &say);
}

/// `AdminMessages`'s `//msg <id>` — send the raw `SystemMessage(id)` to the GM.
pub(super) fn admin_msg(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(id) = args.first().and_then(|s| s.parse::<i16>().ok()) else {
        send_message(world, client_id, "Command format: //msg <SYSTEM_MSG_ID>");
        return;
    };
    send_sm(world, client_id, id);
}

/// `AdminAnnouncements`'s `//announce_crit` / `//announce_screen <message>` —
/// broadcast to all players. The critical/screen variants fall back to the same
/// system-message text line as `//announce` (no ExShowScreenMessage packet yet).
pub(super) fn admin_announce_variant(world: &mut World, client_id: u32, args: &[&str]) {
    if args.is_empty() {
        send_message(world, client_id, "Usage: //announce_screen <message>");
        return;
    }
    broadcast_text(world, &args.join(" "));
}

/// `AdminHtml`'s `//html <path>` / `//loadhtml <path>` — serve an admin HTML
/// file by relative path.
pub(super) fn admin_html(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(path) = args.first() else {
        send_message(world, client_id, "Usage: //html path");
        return;
    };
    super::menu::show_admin_html(world, client_id, path);
}

/// `AdminDebug`'s `//showdoors` — list the doors visible from the GM's region.
pub(super) fn admin_showdoors(world: &mut World, client_id: u32, object_id: i32) {
    let Some(region) = world.objects.get_component::<crate::model::components::RegionCell>(&object_id).map(|r| r.0)
    else {
        return;
    };
    let ids = world.doors_visible_from(region);
    send_message(world, client_id, &format!("=== Doors in view ({}) ===", ids.len()));
    for oid in ids {
        if let Some(d) = world.objects.get_component::<crate::model::door::Door>(&oid) {
            send_message(world, client_id, &format!("  door {} (obj {oid})", d.door_id));
        }
    }
}

/// `AdminDebug`'s `//debug` / `AdminTest`'s `//stats` — dump the current
/// target's core state (or server stats when nothing is targeted).
pub(super) fn admin_debug(world: &mut World, client_id: u32, object_id: i32) {
    match current_target(world, object_id) {
        Some(target) if world.objects.has_component::<Player>(&target) => {
            super::admin_character_info(world, client_id, object_id, &[], false);
        }
        Some(target) if world.objects.has_component::<Npc>(&target) => {
            super::admin_spawn_debug_print(world, client_id, object_id);
        }
        _ => admin_stats(world, client_id),
    }
}

/// `AdminTest`'s `//stats` — server-wide counts.
pub(super) fn admin_stats(world: &mut World, client_id: u32) {
    let online = world.clients.values().filter(|c| matches!(c, ClientSession::InGame(_))).count();
    let npcs: usize = world.npc_regions.values().map(|v| v.len()).sum();
    send_message(world, client_id, "=== Stats ===");
    send_message(world, client_id, &format!("Online: {online}  NPCs: {npcs}  Parties: {}  Tick: {}", world.parties.len(), world.tick));
}

/// `AdminKick`'s `//kick_non_gm` — disconnect every online non-GM player.
pub(super) fn admin_kick_non_gm(world: &mut World, client_id: u32) {
    let targets: Vec<i32> = world
        .clients
        .values()
        .filter_map(|cs| match cs {
            ClientSession::InGame(s) => {
                let oid = s.player_object_id();
                let is_gm = world.objects.get_component::<Player>(&oid).is_some_and(|p| p.is_gm(&world.data));
                (!is_gm).then_some(oid)
            }
            _ => None,
        })
        .collect();
    let n = targets.len();
    for oid in targets {
        disconnect_player(world, oid);
    }
    send_message(world, client_id, &format!("Kicked {n} non-GM player(s)."));
}

/// `AdminMenu`'s `//recall_party_menu` — recall the target's whole party to the
/// GM.
pub(super) fn admin_recall_party(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid)) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let Some(PartyRef(pid)) = world.objects.get_component::<PartyRef>(&target).copied() else {
        send_message(world, client_id, "Target is not in a party.");
        return;
    };
    let members = world.parties.get(&pid).map(|p| p.members.clone()).unwrap_or_default();
    recall_all(world, object_id, &members);
    send_message(world, client_id, &format!("Recalled {} party member(s).", members.len()));
}

/// `AdminMenu`'s `//recall_clan_menu` — recall the target's online clan members
/// to the GM.
pub(super) fn admin_recall_clan(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid)) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let Some(clan_id) = world.objects.get_component::<Player>(&target).map(|p| p.clan_id).filter(|&c| c != 0) else {
        send_message(world, client_id, "Target is not in a clan.");
        return;
    };
    let members: Vec<i32> = world
        .clients
        .values()
        .filter_map(|cs| match cs {
            ClientSession::InGame(s) => {
                let oid = s.player_object_id();
                (world.objects.get_component::<Player>(&oid).map(|p| p.clan_id) == Some(clan_id)).then_some(oid)
            }
            _ => None,
        })
        .collect();
    recall_all(world, object_id, &members);
    send_message(world, client_id, &format!("Recalled {} clan member(s).", members.len()));
}

/// Teleport each of `members` to the GM's position.
fn recall_all(world: &mut World, gm_oid: i32, members: &[i32]) {
    let Some(pos) = world.objects.get_component::<Position>(&gm_oid).copied() else { return };
    for &oid in members {
        super::death::teleport_player(world, oid, pos.x, pos.y, pos.z);
    }
}

/// Broadcast a plain text line to every online player as a `$s1` system message.
fn broadcast_text(world: &World, text: &str) {
    let packet = server_packets::system_message_with(
        sm_ids::S1_TEXT,
        &[server_packets::SmParam::Text(text.to_string())],
    );
    for cs in world.clients.values() {
        if matches!(cs, ClientSession::InGame(_)) {
            cs.send(packet.clone());
        }
    }
}

/// The clean logout teardown for a player (Java `Disconnection.of`): persist,
/// despawn, drop the session.
fn disconnect_player(world: &mut World, target: i32) {
    let Some(tcid) = super::helpers::client_for_player(world, target) else { return };
    if let Some(ClientSession::InGame(session)) = world.clients.remove(&tcid) {
        super::net::store_and_remove_player(world, target);
        session.send(server_packets::leave_world());
    }
}
