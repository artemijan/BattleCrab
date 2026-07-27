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
        None => current_target(world, object_id)
            .filter(|oid| world.objects.has_component::<Player>(oid)),
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
    let Some(target) =
        current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid))
    else {
        send_message(world, client_id, "Select a player first.");
        return;
    };
    disconnect_player(world, target);
}

/// The clean logout teardown for a player (Java `Disconnection.of`): persist,
/// despawn, and drop the session.
fn disconnect_player(world: &mut World, target: i32) {
    let Some(tcid) = super::helpers::client_for_player(world, target) else {
        return;
    };
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
    let Some(name) = world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.name.clone())
    else {
        return;
    };
    // Java passes a null sender (object id 0); the name carries the display.
    let say = server_packets::creature_say(0, crate::enums::ChatType::Alliance, &name, &text, None);
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            if world
                .objects
                .get_component::<Player>(&s.player_object_id())
                .is_some_and(|p| p.is_gm(&world.data))
            {
                cs.send(say.clone());
            }
        }
    }
}

/// `AdminChangeAccessLevel`'s `//changelvl <level>` (target/self) or
/// `//changelvl <name> <level>` — set a character's GM access level. The
/// change is applied in memory (colors + is_gm) and persisted immediately (Java
/// `setAccessLevel(updateInDb=true)`). This sets the *character* access level;
/// the login-server account-level relay is `//login_ban` (G31 slice 4).
pub(super) fn admin_changelvl(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (target, level) = match args {
        [level_str] => {
            let Some(level) = level_str.parse::<i32>().ok() else {
                send_message(
                    world,
                    client_id,
                    "Usage: //changelvl <level> | //changelvl <name> <level>",
                );
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
            send_message(
                world,
                client_id,
                "Usage: //changelvl <level> | //changelvl <name> <level>",
            );
            return;
        }
    };
    // Java `AdminData.getAccessLevel(level) != null` — the tier must exist.
    if world.data.admin.access_level(level).level != level {
        send_message(
            world,
            client_id,
            &format!("Access level {level} does not exist."),
        );
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
            (
                crate::model::DEFAULT_NAME_COLOR,
                crate::model::DEFAULT_TITLE_COLOR,
            )
        }
    };
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.access_level = level;
        p.name_color = name_color;
        p.title_color = title_color;
    }
    if persist {
        let _ = world.db.send(crate::db::DbCommand::SetAccessLevel {
            char_id: target,
            level,
        });
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

/// `AdminPunishment`'s `//jail <name> [minutes]` — jail an online player
/// (Java's `admin_jail` fixes CHARACTER/JAIL; the optional minutes come from the
/// underlying `admin_punishment_add`, `0`/omitted = forever). The character's
/// object id is the punishment key, so it survives relog.
pub(super) fn admin_jail(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(name) = args.first() else {
        send_message(world, client_id, "Usage: //jail <player name> [minutes]");
        return;
    };
    let Some(target) = find_online_player(world, name) else {
        send_message(world, client_id, &format!("Player '{name}' is not online."));
        return;
    };
    let minutes = args.get(1).and_then(|m| m.parse::<i64>().ok()).unwrap_or(0);
    let by = world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "System".to_string());
    let applied = super::super::punishment::jail_character(
        world,
        target,
        minutes,
        "Jailed by admin".to_string(),
        by,
    );
    if applied {
        send_message(
            world,
            client_id,
            &format!("Player '{name}' has been jailed."),
        );
    } else {
        send_message(
            world,
            client_id,
            "Target is already affected by that punishment.",
        );
    }
}

/// `AdminPunishment`'s `//unjail <name>` — release a jailed player.
pub(super) fn admin_unjail(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(name) = args.first() else {
        send_message(world, client_id, "Usage: //unjail <player name>");
        return;
    };
    let Some(target) = find_online_player(world, name) else {
        send_message(world, client_id, &format!("Player '{name}' is not online."));
        return;
    };
    if super::super::punishment::unjail_character(world, target) {
        send_message(
            world,
            client_id,
            &format!("Player '{name}' has been released."),
        );
    } else {
        send_message(world, client_id, &format!("Player '{name}' is not jailed."));
    }
}

// --- Ban / chat-ban / party-ban (Java `AdminPunishment`, G31 slice 2) --------

use crate::model::punishment::{PunishmentAffect, PunishmentType};

/// The GM's display name for the `punishedBy` field.
fn gm_name(world: &World, object_id: i32) -> String {
    world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "System".to_string())
}

/// Resolve a character target for the un-ban commands: an online player by name,
/// else a raw numeric char id — so a kicked/offline ban can still be lifted
/// (this port has no offline name→id table like Java's `CharInfoTable`).
fn resolve_char(world: &World, arg: &str) -> Option<i32> {
    find_online_player(world, arg).or_else(|| arg.parse::<i32>().ok())
}

/// Shared body for `//ban` / `//chatban` / `//partyban` on an online character.
fn char_punish(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
    ptype: PunishmentType,
    verb: &str,
) {
    let Some(name) = args.first() else {
        send_message(
            world,
            client_id,
            &format!("Usage: //{verb} <player name> [minutes]"),
        );
        return;
    };
    let Some(target) = find_online_player(world, name) else {
        send_message(world, client_id, &format!("Player '{name}' is not online."));
        return;
    };
    let minutes = args.get(1).and_then(|m| m.parse::<i64>().ok()).unwrap_or(0);
    let by = gm_name(world, object_id);
    let applied = super::super::punishment::start_punishment(
        world,
        target.to_string(),
        PunishmentAffect::Character,
        ptype,
        super::super::punishment::expiration_from_minutes(minutes),
        format!("{verb} by admin"),
        by,
    );
    if applied {
        send_message(
            world,
            client_id,
            &format!("Player '{name}' has been {verb}ned."),
        );
    } else {
        send_message(
            world,
            client_id,
            "Target is already affected by that punishment.",
        );
    }
}

/// Shared body for `//unban` / `//chatunban` / `//partyunban`.
fn char_unpunish(
    world: &mut World,
    client_id: u32,
    args: &[&str],
    ptype: PunishmentType,
    verb: &str,
) {
    let Some(arg) = args.first() else {
        send_message(
            world,
            client_id,
            &format!("Usage: //{verb} <player name | char id>"),
        );
        return;
    };
    let Some(target) = resolve_char(world, arg) else {
        send_message(
            world,
            client_id,
            &format!(
                "'{arg}' is not online — pass the character id to lift an offline punishment."
            ),
        );
        return;
    };
    if super::super::punishment::stop_character_punishment(world, target, ptype) {
        send_message(world, client_id, &format!("Punishment lifted for '{arg}'."));
    } else {
        send_message(
            world,
            client_id,
            &format!("'{arg}' has no such active punishment."),
        );
    }
}

/// `//ban_char <name> [minutes]` (Java `admin_ban_char`, CHARACTER/BAN) — kicks
/// the player and blocks re-login.
pub(super) fn admin_ban_char(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    char_punish(
        world,
        client_id,
        object_id,
        args,
        PunishmentType::Ban,
        "ban",
    );
}

/// `//unban_char <name | id>`.
pub(super) fn admin_unban_char(world: &mut World, client_id: u32, args: &[&str]) {
    char_unpunish(world, client_id, args, PunishmentType::Ban, "unban");
}

/// `//ban_chat <name> [minutes]` (Java `admin_ban_chat`, CHARACTER/CHAT_BAN).
pub(super) fn admin_ban_chat(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    char_punish(
        world,
        client_id,
        object_id,
        args,
        PunishmentType::ChatBan,
        "chatban",
    );
}

/// `//unban_chat <name | id>`.
pub(super) fn admin_unban_chat(world: &mut World, client_id: u32, args: &[&str]) {
    char_unpunish(world, client_id, args, PunishmentType::ChatBan, "chatunban");
}

/// `//ban_party <name> [minutes]` (CHARACTER/PARTY_BAN) — a port convenience;
/// Java only sets PARTY_BAN through the generic `admin_punishment_add` flow.
pub(super) fn admin_ban_party(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    char_punish(
        world,
        client_id,
        object_id,
        args,
        PunishmentType::PartyBan,
        "partyban",
    );
}

/// `//unban_party <name | id>`.
pub(super) fn admin_unban_party(world: &mut World, client_id: u32, args: &[&str]) {
    char_unpunish(
        world,
        client_id,
        args,
        PunishmentType::PartyBan,
        "partyunban",
    );
}

/// `//ban_acc <account> [minutes]` (Java `admin_ban_acc`, ACCOUNT/BAN).
pub(super) fn admin_ban_acc(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(account) = args.first() else {
        send_message(world, client_id, "Usage: //ban_acc <account> [minutes]");
        return;
    };
    let minutes = args.get(1).and_then(|m| m.parse::<i64>().ok()).unwrap_or(0);
    let by = gm_name(world, object_id);
    let applied = super::super::punishment::start_punishment(
        world,
        account.to_string(),
        PunishmentAffect::Account,
        PunishmentType::Ban,
        super::super::punishment::expiration_from_minutes(minutes),
        "ban by admin".to_string(),
        by,
    );
    if applied {
        send_message(
            world,
            client_id,
            &format!("Account '{account}' has been banned."),
        );
    } else {
        send_message(world, client_id, "That account is already banned.");
    }
}

/// `//unban_acc <account>`.
pub(super) fn admin_unban_acc(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(account) = args.first() else {
        send_message(world, client_id, "Usage: //unban_acc <account>");
        return;
    };
    if super::super::punishment::stop_punishment(
        world,
        account,
        PunishmentAffect::Account,
        PunishmentType::Ban,
    ) {
        send_message(
            world,
            client_id,
            &format!("Account '{account}' has been unbanned."),
        );
    } else {
        send_message(world, client_id, "That account is not banned.");
    }
}

// --- Login-ban relay + IP tools (Java `Player.setAccountAccesslevel` +
// `AdminEditChar` find_ip/find_dualbox/tracert, G31 slice 4) -----------------

/// A player's live client IP (`Player.getIPAddress`), or `None` when offline.
fn player_ip(world: &World, object_id: i32) -> Option<String> {
    super::helpers::client_for_player(world, object_id)
        .and_then(|cid| world.clients.get(&cid))
        .map(|cs| cs.addr().ip().to_string())
}

/// Every online player's `(object_id, ip)` (the IP tools' shared scan).
fn online_ips(world: &World) -> Vec<(i32, String)> {
    world
        .clients
        .values()
        .filter_map(|cs| match cs {
            ClientSession::InGame(s) => Some((s.player_object_id(), cs.addr().ip().to_string())),
            _ => None,
        })
        .collect()
}

/// The online characters connected from `ip` (Java `findCharactersPerIp`).
pub(crate) fn characters_from_ip(world: &World, ip: &str) -> Vec<i32> {
    online_ips(world)
        .into_iter()
        .filter(|(_, pip)| pip == ip)
        .map(|(oid, _)| oid)
        .collect()
}

/// IPs with `threshold` or more online characters, most-populous first (Java
/// `findDualbox`).
pub(crate) fn dualbox_ips(world: &World, threshold: usize) -> Vec<(String, usize)> {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (_, ip) in online_ips(world) {
        *counts.entry(ip).or_default() += 1;
    }
    let mut hits: Vec<(String, usize)> = counts
        .into_iter()
        .filter(|(_, c)| *c >= threshold)
        .collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1));
    hits
}

/// Relay an account's new access level to the login server (Java
/// `Player.setAccountAccesslevel` → `LoginServerThread.sendAccessLevel`).
fn relay_account_access(world: &World, account: &str, level: i32) {
    let _ = world
        .login
        .link
        .send(crate::loginlink::LoginLinkCommand::SetAccountAccessLevel {
            account: account.to_string(),
            level,
        });
}

/// `//login_ban <account>` — ban an account at the login server (relay access
/// level −1), and kick anyone on it who is currently online. This is the
/// login-link ban, distinct from the game-side `//ban_acc` punishment.
pub(super) fn admin_login_ban(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(account) = args.first() else {
        send_message(world, client_id, "Usage: //login_ban <account>");
        return;
    };
    relay_account_access(world, account, -1);
    // Kick any online characters on that account (Java sets the LS level; we
    // also drop live sessions so the ban bites immediately, not just next login).
    let online: Vec<i32> = world
        .clients
        .values()
        .filter_map(|cs| match cs {
            ClientSession::InGame(s) => {
                let oid = s.player_object_id();
                world
                    .objects
                    .get_component::<Player>(&oid)
                    .filter(|p| p.account == *account)
                    .map(|_| oid)
            }
            _ => None,
        })
        .collect();
    for oid in online {
        disconnect_player(world, oid);
    }
    send_message(
        world,
        client_id,
        &format!("Login ban relayed for account '{account}'."),
    );
}

/// `//login_unban <account>` — restore an account at the login server (relay
/// access level 0).
pub(super) fn admin_login_unban(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(account) = args.first() else {
        send_message(world, client_id, "Usage: //login_unban <account>");
        return;
    };
    relay_account_access(world, account, 0);
    send_message(
        world,
        client_id,
        &format!("Login ban lifted for account '{account}'."),
    );
}

/// `AdminEditChar`'s `//find_ip <ip>` — list the online characters connected
/// from a given IP.
pub(super) fn admin_find_ip(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(ip) = args.first() else {
        send_message(world, client_id, "Usage: //find_ip <a.b.c.d>");
        return;
    };
    let matches = characters_from_ip(world, ip);
    send_message(world, client_id, &format!("=== Characters from {ip} ==="));
    if matches.is_empty() {
        send_message(world, client_id, "None online.");
        return;
    }
    for oid in matches {
        if let Some(p) = world.objects.get_component::<Player>(&oid) {
            send_message(world, client_id, &format!("{} (Lv {})", p.name, p.level));
        }
    }
}

/// `AdminEditChar`'s `//find_dualbox [n]` — IPs with `n` or more online
/// characters (default 2). Java's default `multibox` is 2.
pub(super) fn admin_find_dualbox(world: &mut World, client_id: u32, args: &[&str]) {
    let threshold = args
        .first()
        .and_then(|a| a.parse::<usize>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(2);
    let hits = dualbox_ips(world, threshold);
    send_message(
        world,
        client_id,
        &format!("=== Dualbox (>= {threshold}) ==="),
    );
    if hits.is_empty() {
        send_message(world, client_id, "None found.");
        return;
    }
    for (ip, count) in hits {
        send_message(world, client_id, &format!("{ip} ({count})"));
    }
}

/// `AdminEditChar`'s `//tracert <name>` — show a player's connecting IP (Java
/// dumps the client's route trace; the port has only the peer address, so it
/// reports that — a documented simplification).
pub(super) fn admin_tracert(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let target = match args.first() {
        Some(name) => find_online_player(world, name),
        None => current_target(world, object_id)
            .filter(|oid| world.objects.has_component::<Player>(oid)),
    };
    let Some(target) = target else {
        send_message(world, client_id, "Usage: //tracert <player name>");
        return;
    };
    let name = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    match player_ip(world, target) {
        Some(ip) => send_message(world, client_id, &format!("{name} — IP {ip}")),
        None => send_message(world, client_id, "Client is null."),
    }
}

/// `AdminServerInfo` — Java opens an HTML window; we send the key figures as
/// text lines (a documented G13.A simplification; the HTML build waits for the
/// admin-menu work in G13.B).
pub(super) fn admin_serverinfo(world: &World, client_id: u32) {
    let online = world
        .clients
        .values()
        .filter(|c| matches!(c, ClientSession::InGame(_)))
        .count();
    send_message(world, client_id, "=== Server Info ===");
    send_message(world, client_id, &format!("Online players: {online}"));
    send_message(world, client_id, &format!("Server tick: {}", world.tick));
}
