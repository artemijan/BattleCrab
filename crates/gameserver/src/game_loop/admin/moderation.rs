//! Moderation & session commands — disconnects (`//kick`,
//! `//character_disconnect`), broadcasts (`//announce`, `//gmchat`), access-level
//! management (`//changelvl`, `//gm`), targeting (`//target`), and
//! `//serverinfo`.

use crate::model::Player;
use crate::network::server_packets::{self, sm_ids};
use crate::session::ClientSession;
use crate::world::World;

use super::{current_target, find_online_player, send_message};

/// `AdminKick`'s `//kick <name>` (or the targeted player) — the clean logout
/// teardown: persist, despawn, and drop the session (Java `Disconnection.of`).
pub(super) fn admin_kick(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let target = match args.first() {
        Some(name) => find_online_player(world, name),
        None => current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid)),
    };
    let Some(target) = target else {
        send_message(world, client_id, "Usage: //kick <player name>");
        return;
    };
    disconnect_player(world, target);
}

/// `AdminDisconnect`'s `//character_disconnect` — disconnect the targeted
/// player.
pub(super) fn admin_character_disconnect(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid))
    else {
        send_message(world, client_id, "Select a player first.");
        return;
    };
    disconnect_player(world, target);
}

/// The clean logout teardown for a player (Java `Disconnection.of`): persist,
/// despawn, and drop the session.
fn disconnect_player(world: &mut World, target: i32) {
    let Some(tcid) = super::helpers::client_for_player(world, target) else { return };
    if let Some(ClientSession::InGame(session)) = world.clients.remove(&tcid) {
        super::net::store_and_remove_player(world, target);
        session.send(server_packets::leave_world());
    }
}

/// `AdminAnnouncements`'s `//announce <message>` — broadcast to every online
/// player. Java sends a `ChatType.ANNOUNCEMENT` `CreatureSay`; we send it as a
/// system-message text line (documented simplification — the announce chat
/// type isn't wired yet).
pub(super) fn admin_announce(world: &mut World, client_id: u32, args: &[&str]) {
    if args.is_empty() {
        send_message(world, client_id, "Usage: //announce <message>");
        return;
    }
    let packet = server_packets::system_message_with(
        sm_ids::S1_TEXT,
        &[server_packets::SmParam::Text(args.join(" "))],
    );
    for cs in world.clients.values() {
        if matches!(cs, ClientSession::InGame(_)) {
            cs.send(packet.clone());
        }
    }
}

/// `AdminGmChat`'s `//gmchat <message>` — broadcast to every online GM
/// (`AdminData.broadcastToGMs`, `ChatType.ALLIANCE`).
pub(super) fn admin_gmchat(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    if args.is_empty() {
        send_message(world, client_id, "Usage: //gmchat <message>");
        return;
    }
    let text = args.join(" ");
    let Some(name) = world.objects.get_component::<Player>(&object_id).map(|p| p.name.clone()) else {
        return;
    };
    // Java passes a null sender (object id 0); the name carries the display.
    let say = server_packets::creature_say(0, crate::enums::ChatType::Alliance, &name, &text, None);
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            if world.objects.get_component::<Player>(&s.player_object_id()).is_some_and(|p| p.is_gm(&world.data)) {
                cs.send(say.clone());
            }
        }
    }
}

/// `AdminChangeAccessLevel`'s `//changelvl <level>` (target/self) or
/// `//changelvl <name> <level>` — set a character's GM access level. The
/// change is applied in memory (colors + is_gm) and persisted immediately (Java
/// `setAccessLevel(updateInDb=true)`). The login-server `ChangeAccessLevel`
/// relay (account-level access) is not ported.
pub(super) fn admin_changelvl(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (target, level) = match args {
        [level_str] => {
            let Some(level) = level_str.parse::<i32>().ok() else {
                send_message(world, client_id, "Usage: //changelvl <level> | //changelvl <name> <level>");
                return;
            };
            let target = current_target(world, object_id)
                .filter(|oid| world.objects.has_component::<Player>(oid))
                .unwrap_or(object_id);
            (target, level)
        }
        [name, level_str] => {
            let Some(target) = find_online_player(world, name) else {
                send_message(world, client_id, &format!("Player '{name}' is not online."));
                return;
            };
            let Some(level) = level_str.parse::<i32>().ok() else {
                send_message(world, client_id, "Usage: //changelvl <name> <level>");
                return;
            };
            (target, level)
        }
        _ => {
            send_message(world, client_id, "Usage: //changelvl <level> | //changelvl <name> <level>");
            return;
        }
    };
    // Java `AdminData.getAccessLevel(level) != null` — the tier must exist.
    if world.data.admin.access_level(level).level != level {
        send_message(world, client_id, &format!("Access level {level} does not exist."));
        return;
    }
    set_access(world, target, level, true);
    send_message(world, client_id, &format!("Access level set to {level}."));
}

/// `AdminGm`'s `//gm` — deactivate the caller's own GM access for this session
/// (Java `setAccessLevel(0, broadcast=true, updateInDb=false)`), not persisted.
pub(super) fn admin_gm(world: &mut World, client_id: u32, object_id: i32) {
    set_access(world, object_id, 0, false);
    send_message(world, client_id, "GM access deactivated for this session.");
}

/// Apply an access-level change: level + name/title colors in memory, an
/// optional immediate DB persist, and a UserInfo rebroadcast (colors changed).
fn set_access(world: &mut World, target: i32, level: i32, persist: bool) {
    let (name_color, title_color) = {
        let al = world.data.admin.access_level(level);
        if level != 0 {
            (al.name_color, al.title_color)
        } else {
            (crate::model::DEFAULT_NAME_COLOR, crate::model::DEFAULT_TITLE_COLOR)
        }
    };
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.access_level = level;
        p.name_color = name_color;
        p.title_color = title_color;
    }
    if persist {
        let _ = world.db.send(crate::db::DbCommand::SetAccessLevel { char_id: target, level });
    }
    super::party::broadcast_user_info(world, target);
}

/// `AdminTarget`'s `//target <name>` — select a player by name (reuses the
/// normal targeting flow, including its Z-distance guard).
pub(super) fn admin_target(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(name) = args.first() else {
        send_message(world, client_id, "Usage: //target <player name>");
        return;
    };
    let Some(target) = find_online_player(world, name) else {
        send_message(world, client_id, &format!("Player '{name}' is not online."));
        return;
    };
    super::target::set_target(world, client_id, object_id, Some(target));
}

/// `AdminServerInfo` — Java opens an HTML window; we send the key figures as
/// text lines (a documented G13.A simplification; the HTML build waits for the
/// admin-menu work in G13.B).
pub(super) fn admin_serverinfo(world: &World, client_id: u32) {
    let online = world.clients.values().filter(|c| matches!(c, ClientSession::InGame(_))).count();
    send_message(world, client_id, "=== Server Info ===");
    send_message(world, client_id, &format!("Online players: {online}"));
    send_message(world, client_id, &format!("Server tick: {}", world.tick));
}
