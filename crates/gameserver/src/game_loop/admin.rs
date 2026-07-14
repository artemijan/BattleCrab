//! Admin / GM command framework — port of `handler/AdminCommandHandler` plus
//! the two dispatch paths that reach it: `SendBypassBuildCmd` (the `//command`
//! bar, client opcode 0x74) and an `admin_`-prefixed `RequestBypassToServer`
//! (the admin HTML-menu buttons). Per-command required access level and the
//! confirm flag come from [`AdminData`](crate::data::AdminData) (G13.A).
//!
//! Command bodies are the `admin_*` functions below, routed by [`dispatch`].
//! Handlers whose backing subsystem is not ported yet (sieges, olympiad,
//! instances… — G13.C) are simply absent here: they are still gated correctly
//! by the access table and reach the "not implemented" path rather than
//! crashing.

use tracing::{info, warn};

use crate::model::components::{PlayerVitals, Position, Speeds, TargetRef, Vitals};
use crate::model::Player;
use crate::network::server_packets::{self, sm_ids, status_update_type as sut};
use crate::session::ClientSession;
use crate::world::World;

/// Java `AdminCommandHandler.useAdminCommand`. `full` is the whole command
/// string *including* the `admin_` prefix, e.g. `"admin_heal 100"`.
///
/// Java runs the body on a threadpool task (server-freeze protection); the game
/// loop is single-threaded here, so it runs inline.
pub(crate) fn use_admin_command(world: &mut World, client_id: u32, full: &str, use_confirm: bool) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    let Some(player) = world.objects.get_component::<Player>(&object_id) else { return };

    // Java `if (!player.isGM()) return;` — silent, no message.
    if !player.is_gm(&world.data) {
        return;
    }
    let access_level = player.access_level;

    // Command word = the first whitespace-delimited token (Java
    // `fullCommand.split(" ")[0]`); the rest are arguments.
    let command = full.split_whitespace().next().unwrap_or(full).to_string();
    let display = command.strip_prefix("admin_").unwrap_or(&command).to_string();

    // Handler existence (Java `getHandler(command) == null`). The "known" set
    // is the AdminCommands.xml command table.
    if !world.data.admin.has_command(&command) {
        send_message(world, client_id, &format!("The command '{display}' does not exist!"));
        warn!("No handler registered for admin command '{command}'.");
        return;
    }

    // Access rights (Java `AdminData.hasAccess`).
    if !world.data.admin.has_access(&command, access_level) {
        send_message(world, client_id, "You don't have the access rights to use this command!");
        warn!("Object {object_id} tried admin command '{command}' without proper access level.");
        return;
    }

    // Confirmation dialog (Java `AdminData.requireConfirm`): prompt and defer
    // the real execution to the `DlgAnswer` reply (handled in `handle_dlg_answer`).
    if use_confirm && world.data.admin.require_confirm(&command) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::confirm_dlg_text(&format!(
                "Are you sure you want to execute command '{display}' ?"
            )));
        }
        if let Some(ClientSession::InGame(session)) = world.clients.get_mut(&client_id) {
            session.set_admin_confirm(full.to_string());
        }
        return;
    }

    // GMAudit (Java `GMAudit.auditGMAction`) — a log line here, not the
    // per-GM audit file.
    if let Some(p) = world.objects.get_component::<Player>(&object_id) {
        info!("GMAudit: {} [{object_id}] used '{full}'.", p.name);
    }

    // A gated-but-unimplemented command (G13.C) lands on the `false` arm.
    if !dispatch(world, client_id, object_id, &command, full) {
        send_message(world, client_id, &format!("Admin command '{display}' is not implemented yet."));
    }
}

/// Port of `clientpackets/DlgAnswer` narrowed to the admin-confirm case (the
/// only `ConfirmDlg` this server sends; door/summon/offline-play/olympiad
/// dialogs are unported). On the echoed `S1_3` id, the pending command is
/// consumed and — on "yes" — re-run with confirmation disabled (Java
/// `useAdminCommand(player, cmd, false)`).
pub(crate) fn handle_dlg_answer(world: &mut World, client_id: u32, answer: crate::network::client_packets::DlgAnswer) {
    if answer.message_id != server_packets::S1_3_MESSAGE_ID {
        return;
    }
    let pending = match world.clients.get_mut(&client_id) {
        Some(ClientSession::InGame(session)) => session.take_admin_confirm(),
        _ => return,
    };
    let Some(command) = pending else { return };
    if answer.answer == 1 {
        use_admin_command(world, client_id, &command, false);
    }
}

/// Route a resolved + authorized command to its body. Returns `false` when the
/// command has no body yet (gated but unported — G13.C).
fn dispatch(world: &mut World, client_id: u32, object_id: i32, command: &str, full: &str) -> bool {
    // Arguments = the whitespace-delimited tokens after the command word.
    let args: Vec<&str> = full.split_whitespace().skip(1).collect();
    match command {
        "admin_serverinfo" => admin_serverinfo(world, client_id),
        "admin_heal" => admin_heal(world, object_id),
        "admin_kill" => admin_kill(world, client_id, object_id),
        "admin_res" => admin_res(world, object_id),
        "admin_gmspeed" => admin_gmspeed(world, client_id, object_id, &args),
        // Self-teleport to explicit coordinates.
        "admin_teleport" | "admin_move_to" | "admin_tele" | "admin_instant_move" => {
            admin_teleport_coords(world, client_id, object_id, &args)
        }
        // Bring a player to the GM.
        "admin_recall" => admin_recall(world, client_id, object_id, &args),
        // Send the GM to the current target.
        "admin_teleto" | "admin_teleportto" | "admin_teleport_to_character" => {
            admin_teleto(world, client_id, object_id)
        }
        // Create an item on the GM.
        "admin_create_item" => admin_create_item(world, client_id, object_id, &args),
        // Give an item to the targeted player.
        "admin_give_item_target" => admin_give_item_target(world, client_id, object_id, &args),
        // Give an item to every online player.
        "admin_give_item_to_all" => admin_give_item_to_all(world, client_id, &args),
        // Disconnect a player (named or targeted).
        "admin_kick" => admin_kick(world, client_id, object_id, &args),
        // Add exp/sp to the targeted player (or self).
        "admin_add_exp_sp" => admin_add_exp_sp(world, client_id, object_id, &args),
        // Add N levels to / set the level of the targeted player (or self).
        "admin_add_level" => admin_change_level(world, client_id, object_id, &args, false),
        "admin_set_level" => admin_change_level(world, client_id, object_id, &args, true),
        // Broadcast a message to all online GMs.
        "admin_gmchat" => admin_gmchat(world, client_id, object_id, &args),
        // Set a player's access level (persisted).
        "admin_changelvl" => admin_changelvl(world, client_id, object_id, &args),
        // Deactivate the caller's own GM access for this session.
        "admin_gm" => admin_gm(world, client_id, object_id),
        // Disconnect the targeted player.
        "admin_character_disconnect" => admin_character_disconnect(world, client_id, object_id),
        // Broadcast a message to every online player.
        "admin_announce" => admin_announce(world, client_id, &args),
        _ => return false,
    }
    true
}

/// The GM's current target object id, or `None` if nothing is selected.
fn current_target(world: &World, object_id: i32) -> Option<i32> {
    world.objects.get_component::<TargetRef>(&object_id).and_then(|t| t.0)
}

/// `AdminHeal` (first slice): fully restore the targeted player's HP/MP/CP, or
/// the GM's own if no *player* is targeted. NPC targets and the `<name>` form
/// are TODO (G13.B breadth).
fn admin_heal(world: &mut World, object_id: i32) {
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    full_restore(world, target);
}

/// `AdminRes` (first slice): revive the targeted player (or self) and fully
/// restore them. `admin_res_monster` (NPC) is TODO.
fn admin_res(world: &mut World, object_id: i32) {
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    if world.objects.get_component::<Vitals>(&target).is_some_and(|v| v.dead) {
        super::death::do_revive(world, target);
    }
    full_restore(world, target);
}

/// Set a player's HP/MP/CP to full (clearing death) and push the resulting
/// `StatusUpdate` to that player + their party. Shared by `//heal` and `//res`.
fn full_restore(world: &mut World, target: i32) {
    let updates = {
        let Some((mut vitals, mut pvitals)) =
            world.objects.get_many_mut::<(&mut Vitals, &mut PlayerVitals)>(&target)
        else {
            return;
        };
        vitals.cur_hp = vitals.max_hp as f64;
        vitals.cur_mp = vitals.max_mp as f64;
        vitals.dead = false;
        pvitals.cur_cp = pvitals.max_cp as f64;
        [(sut::CUR_HP, vitals.max_hp), (sut::CUR_MP, vitals.max_mp), (sut::CUR_CP, pvitals.max_cp)]
    };
    let packet = server_packets::status_update(target, &updates);
    if let Some(cid) = super::helpers::client_for_player(world, target) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(packet);
        }
    }
    super::party::notify_party_vitals(world, target);
}

/// `AdminGmSpeed` — scale the target player's (or self's) movement speed. Java
/// adds `baseSpeed * boost` as a fixed value to each speed stat, i.e. total =
/// `baseSpeed * (1 + boost)`; the Rust move model already carries a
/// `move_multiplier`, so `1 + boost` is the exact equivalent (boost 0 resets).
/// Range 0..=10, matching Java's custom clamp. NPC targets are TODO.
fn admin_gmspeed(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(boost) = args.first().and_then(|s| s.parse::<f64>().ok()).filter(|b| (0.0..=10.0).contains(b))
    else {
        send_message(world, client_id, "//gmspeed [0...10]");
        return;
    };
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    if let Some(speeds) = world.objects.get_component_mut::<Speeds>(&target) {
        speeds.move_multiplier = 1.0 + boost;
    }
    super::party::broadcast_user_info(world, target);
}

/// `AdminTeleport`'s coordinate form (`//teleport x y z`) — send the GM to an
/// explicit location. The menu/target-teleport variants are TODO.
fn admin_teleport_coords(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let coords = (
        args.first().and_then(|s| s.parse::<i32>().ok()),
        args.get(1).and_then(|s| s.parse::<i32>().ok()),
        args.get(2).and_then(|s| s.parse::<i32>().ok()),
    );
    let (Some(x), Some(y), Some(z)) = coords else {
        send_message(world, client_id, "Usage: //teleport <x> <y> <z>");
        return;
    };
    super::death::teleport_player(world, object_id, x, y, z);
}

/// `AdminTeleport`'s `//recall <name>` — bring an online player to the GM's
/// location (or, with no name, the currently targeted player).
fn admin_recall(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let target = match args.first() {
        Some(name) => find_online_player(world, name),
        None => current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid)),
    };
    let Some(target) = target else {
        send_message(world, client_id, "Usage: //recall <player name>");
        return;
    };
    let Some(&pos) = world.objects.get_component::<Position>(&object_id) else { return };
    super::death::teleport_player(world, target, pos.x, pos.y, pos.z);
}

/// `AdminTeleport`'s `//teleto` — send the GM to the current target's position.
fn admin_teleto(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id) else {
        send_message(world, client_id, "Select a target first.");
        return;
    };
    let Some(&pos) = world.objects.get_component::<Position>(&target) else { return };
    super::death::teleport_player(world, object_id, pos.x, pos.y, pos.z);
}

/// `AdminCreateItem`'s `//create_item <id> [count]` — create an item on the GM.
fn admin_create_item(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(item_id), count) = parse_item_args(args) else {
        send_message(world, client_id, "Usage: //create_item <id> [count]");
        return;
    };
    if world.data.item_data.get(item_id).is_none() {
        send_message(world, client_id, &format!("Item id {item_id} does not exist."));
        return;
    }
    super::quests::give_item_with_earned_message(world, client_id, object_id, item_id, count);
}

/// `AdminCreateItem`'s `//give_item_target <id> [count]` — give to the targeted
/// player (or the GM if none is selected).
fn admin_give_item_target(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(item_id), count) = parse_item_args(args) else {
        send_message(world, client_id, "Usage: //give_item_target <id> [count]");
        return;
    };
    if world.data.item_data.get(item_id).is_none() {
        send_message(world, client_id, &format!("Item id {item_id} does not exist."));
        return;
    }
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    let Some(tcid) = super::helpers::client_for_player(world, target) else { return };
    super::quests::give_item_with_earned_message(world, tcid, target, item_id, count);
}

/// `AdminCreateItem`'s `//give_item_to_all <id> [count]` — give to every online
/// player.
fn admin_give_item_to_all(world: &mut World, client_id: u32, args: &[&str]) {
    let (Some(item_id), count) = parse_item_args(args) else {
        send_message(world, client_id, "Usage: //give_item_to_all <id> [count]");
        return;
    };
    if world.data.item_data.get(item_id).is_none() {
        send_message(world, client_id, &format!("Item id {item_id} does not exist."));
        return;
    }
    let recipients: Vec<(u32, i32)> = world
        .clients
        .iter()
        .filter_map(|(&cid, cs)| match cs {
            ClientSession::InGame(s) => Some((cid, s.player_object_id())),
            _ => None,
        })
        .collect();
    let count_given = recipients.len();
    for (cid, oid) in recipients {
        super::quests::give_item_with_earned_message(world, cid, oid, item_id, count);
    }
    send_message(world, client_id, &format!("Gave item {item_id} to {count_given} player(s)."));
}

/// Parse `<id> [count]` — item id (required) and count (default 1, min 1).
fn parse_item_args(args: &[&str]) -> (Option<i32>, i64) {
    let item_id = args.first().and_then(|s| s.parse::<i32>().ok());
    let count = args.get(1).and_then(|s| s.parse::<i64>().ok()).unwrap_or(1).max(1);
    (item_id, count)
}

/// `AdminKick`'s `//kick <name>` (or the targeted player) — the clean logout
/// teardown: persist, despawn, and drop the session (Java `Disconnection.of`).
fn admin_kick(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
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
fn admin_character_disconnect(world: &mut World, client_id: u32, object_id: i32) {
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
fn admin_announce(world: &mut World, client_id: u32, args: &[&str]) {
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

/// `AdminExpSp`'s `//add_exp_sp <exp> <sp>` — grant exp+sp to the targeted
/// player (or self), driving the level-up path.
fn admin_add_exp_sp(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(exp), Some(sp)) = (
        args.first().and_then(|s| s.parse::<i64>().ok()),
        args.get(1).and_then(|s| s.parse::<i64>().ok()),
    ) else {
        send_message(world, client_id, "Usage: //add_exp_sp <exp> <sp>");
        return;
    };
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    super::death::add_exp_and_sp(world, target, exp, sp);
}

/// `AdminLevel`'s `//add_level <n>` / `//set_level <n>` — add levels to, or set
/// the level of, the targeted player (or self). `set` chooses between the two.
fn admin_change_level(world: &mut World, client_id: u32, object_id: i32, args: &[&str], set: bool) {
    let Some(value) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, if set { "Usage: //set_level <level>" } else { "Usage: //add_level <levels>" });
        return;
    };
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    let Some(current) = world.objects.get_component::<Player>(&target).map(|p| p.level) else { return };
    let max_level = world.data.experience.max_level as i32;
    let new_level = if set { value } else { current + value }.clamp(1, max_level);
    // Set exp to the level's threshold so the exp bar and future exp math stay
    // consistent (Java `PlayerStat.setLevel` → `setExp(getExpForLevel(level))`).
    let exp = world.data.experience.exp_for_level(new_level);
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.exp = exp;
    }
    super::death::set_level(world, target, new_level);
}

/// `AdminGmChat`'s `//gmchat <message>` — broadcast to every online GM
/// (`AdminData.broadcastToGMs`, `ChatType.ALLIANCE`).
fn admin_gmchat(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
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
fn admin_changelvl(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
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
fn admin_gm(world: &mut World, client_id: u32, object_id: i32) {
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

/// `World.getPlayer(name)` — case-insensitive scan over in-game players.
fn find_online_player(world: &World, name: &str) -> Option<i32> {
    world.clients.values().find_map(|cs| match cs {
        ClientSession::InGame(s) => {
            let oid = s.player_object_id();
            world
                .objects
                .get_component::<Player>(&oid)
                .filter(|p| p.name.eq_ignore_ascii_case(name))
                .map(|_| oid)
        }
        _ => None,
    })
}

/// `AdminKill` (first slice): kill the current target (player or NPC) with the
/// GM as the killer. The `<name>` / radius forms are TODO (G13.B breadth).
fn admin_kill(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id) else {
        send_message(world, client_id, "Select a target first.");
        return;
    };
    if world.objects.has_component::<Player>(&target) {
        super::death::player_do_die(world, target, object_id);
    } else if world.objects.has_component::<crate::model::npc::Npc>(&target) {
        super::death::npc_do_die(world, target, object_id);
    }
}

/// `AdminServerInfo` — Java opens an HTML window; we send the key figures as
/// text lines (a documented G13.A simplification; the HTML build waits for the
/// admin-menu work in G13.B).
fn admin_serverinfo(world: &World, client_id: u32) {
    let online = world.clients.values().filter(|c| matches!(c, ClientSession::InGame(_))).count();
    send_message(world, client_id, "=== Server Info ===");
    send_message(world, client_id, &format!("Online players: {online}"));
    send_message(world, client_id, &format!("Server tick: {}", world.tick));
}

/// Java `Player.sendMessage(String)` — a bare `$s1` system message.
pub(crate) fn send_message(world: &World, client_id: u32, text: &str) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::system_message_with(
            sm_ids::S1_TEXT,
            &[server_packets::SmParam::Text(text.to_string())],
        ));
    }
}
