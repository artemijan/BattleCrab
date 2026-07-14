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

use crate::model::components::{PlayerVitals, TargetRef, Vitals};
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
fn dispatch(world: &mut World, client_id: u32, object_id: i32, command: &str, _full: &str) -> bool {
    match command {
        "admin_serverinfo" => admin_serverinfo(world, client_id),
        "admin_heal" => admin_heal(world, object_id),
        "admin_kill" => admin_kill(world, client_id, object_id),
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
    // Mutate under a scoped borrow so the vitals guards drop before the send.
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
