//! Admin / GM command framework — port of `handler/AdminCommandHandler` plus
//! the two dispatch paths that reach it: `SendBypassBuildCmd` (the `//command`
//! bar, client opcode 0x74) and an `admin_`-prefixed `RequestBypassToServer`
//! (the admin HTML-menu buttons). Per-command required access level and the
//! confirm flag come from [`AdminData`](crate::data::AdminData) (G13.A).
//!
//! Command bodies live in the subsystem modules below, grouped by the feature
//! they drive, and are routed by [`dispatch`]. Handlers whose backing subsystem
//! is not ported yet (sieges, olympiad, instances… — G13.C) are simply absent:
//! they are still gated correctly by the access table and reach the "not
//! implemented" path rather than crashing.

use tracing::{info, warn};

use crate::model::components::TargetRef;
use crate::model::inventory::PaperdollSlot;
use crate::model::Player;
use crate::network::server_packets::{self, sm_ids};
use crate::session::ClientSession;
use crate::world::World;

mod character;
mod effects;
mod flags;
mod items;
mod menu;
mod moderation;
mod skills;
mod spawn;
mod teleport;
mod vitals;

// The command bodies and the small enums the dispatch table names live in the
// modules above; glob them in so `dispatch` reads as one flat routing table.
use character::*;
use effects::*;
use flags::*;
use menu::*;

// The enter-world GM startup block (`EnterWorld.runImpl`) is driven from
// `lobby::handle_enter_world`, so re-export it out of the admin module.
pub(crate) use flags::apply_gm_startup;
use items::*;
use moderation::*;
use skills::*;
use spawn::*;
use teleport::*;
use vitals::*;

// Sibling `game_loop` modules the command bodies reach through `super::`. Before
// `admin` became a folder these were plain `super::` siblings; re-importing them
// here keeps every `super::helpers::…` / `super::death::…` call in the bodies
// resolving (a child's `super` now points at this module).
use crate::game_loop::{death, helpers, net, party, quests, target, visibility};

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
        // The `//admin` GM menu (`AdminAdmin.showMainPage`): main + the six
        // sub-panels. Their buttons route back through the `admin_` bypass.
        "admin_admin" | "admin_admin1" | "admin_admin2" | "admin_admin3" | "admin_admin4"
        | "admin_admin5" | "admin_admin6" | "admin_admin7" => admin_admin(world, client_id, command),
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
        // Spawn an NPC at the GM's location.
        "admin_spawn" => admin_spawn(world, client_id, object_id, &args),
        // Despawn the targeted NPC.
        "admin_delete" => admin_delete(world, client_id, object_id),
        // Target a player by name.
        "admin_target" => admin_target(world, client_id, object_id, &args),
        // Invulnerability / undying toggles (self or targeted player).
        "admin_invul" => toggle_flag(world, client_id, object_id, GmFlag::Invul),
        "admin_setinvul" => toggle_flag_on_target(world, client_id, object_id, GmFlag::Invul),
        "admin_undying" => toggle_flag(world, client_id, object_id, GmFlag::Undying),
        "admin_setundying" => toggle_flag_on_target(world, client_id, object_id, GmFlag::Undying),
        // Toggle the GM's visibility to other players.
        "admin_hide" => admin_hide(world, client_id, object_id),
        // Grant / remove a skill on the targeted player (or self).
        "admin_add_skill" => admin_add_skill(world, client_id, object_id, &args),
        "admin_remove_skill" => admin_remove_skill(world, client_id, object_id, &args),
        // Per-slot enchant (`AdminEnchant`): //set<slot> <0..127>.
        "admin_seteh" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::Head, &args),
        "admin_setec" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::Chest, &args),
        "admin_seteg" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::Gloves, &args),
        "admin_setel" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::Legs, &args),
        "admin_seteb" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::Feet, &args),
        "admin_setew" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::RHand, &args),
        "admin_setes" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::LHand, &args),
        "admin_setle" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::LEar, &args),
        "admin_setre" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::REar, &args),
        "admin_setlf" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::LFinger, &args),
        "admin_setrf" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::RFinger, &args),
        "admin_seten" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::Neck, &args),
        "admin_setun" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::Under, &args),
        "admin_setba" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::Cloak, &args),
        "admin_setbe" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::Belt, &args),
        // Apply a skill's effects (buff) to the target.
        "admin_buff" => admin_buff(world, client_id, object_id, &args),
        // List the target's active buffs.
        "admin_getbuffs" => admin_getbuffs(world, client_id, object_id),
        // Remove one / all buffs from the target.
        "admin_stopbuff" => admin_stopbuff(world, client_id, object_id, &args),
        "admin_stopallbuffs" => admin_stopallbuffs(world, client_id, object_id),
        // EditChar field setters (target player or self).
        "admin_setreputation" => set_int_field(world, client_id, object_id, IntField::Reputation, &args),
        "admin_nokarma" => set_field_value(world, client_id, object_id, IntField::Reputation, 0),
        "admin_setfame" => set_int_field(world, client_id, object_id, IntField::Fame, &args),
        "admin_setpk" => set_int_field(world, client_id, object_id, IntField::Pk, &args),
        "admin_setpvp" => set_int_field(world, client_id, object_id, IntField::Pvp, &args),
        "admin_settitle" => admin_set_title(world, client_id, object_id, &args),
        "admin_setcolor" => admin_set_color(world, client_id, object_id, &args, false),
        "admin_settcolor" => admin_set_color(world, client_id, object_id, &args, true),
        "admin_setsex" => admin_set_sex(world, client_id, object_id),
        "admin_set_hp" => set_vital(world, client_id, object_id, Vital::Hp, &args),
        "admin_set_mp" => set_vital(world, client_id, object_id, Vital::Mp, &args),
        "admin_set_cp" => set_vital(world, client_id, object_id, Vital::Cp, &args),
        "admin_setclass" => admin_setclass(world, client_id, object_id, &args),

        "admin_social" | "admin_social_menu" => admin_social(world, client_id, object_id, &args),
        "admin_effect" | "admin_npc_use_skill" => admin_effect(world, client_id, object_id, &args),
        "admin_earthquake" | "admin_earthquake_menu" => admin_earthquake(world, client_id, object_id, &args),
        "admin_atmosphere" | "admin_atmosphere_menu" => admin_atmosphere(world, client_id, &args),
        "admin_play_sound" => admin_play_sound(world, client_id, object_id, &args),
        _ => return false,
    }
    true
}

/// EditChar target = the current target if it's a player, else the GM.
pub(super) fn target_player(world: &World, object_id: i32) -> i32 {
    current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id)
}

/// The GM's current target object id, or `None` if nothing is selected.
pub(super) fn current_target(world: &World, object_id: i32) -> Option<i32> {
    world.objects.get_component::<TargetRef>(&object_id).and_then(|t| t.0)
}

/// `World.getPlayer(name)` — case-insensitive scan over in-game players.
pub(super) fn find_online_player(world: &World, name: &str) -> Option<i32> {
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

/// Send a bare `SystemMessage(id)` to one client.
pub(super) fn send_sm(world: &World, client_id: u32, message_id: i16) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::system_message_with(message_id, &[]));
    }
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
