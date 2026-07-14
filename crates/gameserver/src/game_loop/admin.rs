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

use crate::model::components::{AdminFlags, PlayerVitals, Position, Speeds, TargetRef, Vitals};
use crate::model::inventory::{Inventory, PaperdollSlot};
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
fn target_player(world: &World, object_id: i32) -> i32 {
    current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id)
}

/// The GM flags togglable via `AdminFlags`.
#[derive(Clone, Copy)]
enum GmFlag {
    Invul,
    Undying,
    Hidden,
}

impl GmFlag {
    fn label(self) -> &'static str {
        match self {
            GmFlag::Invul => "Invulnerability",
            GmFlag::Undying => "Undying",
            GmFlag::Hidden => "Hide",
        }
    }
}

/// Flip a GM flag on `target`, returning its new state.
fn set_flag(world: &mut World, target: i32, flag: GmFlag) -> bool {
    let mut flags = world.objects.get_component::<AdminFlags>(&target).copied().unwrap_or_default();
    let now = match flag {
        GmFlag::Invul => {
            flags.invul = !flags.invul;
            flags.invul
        }
        GmFlag::Undying => {
            flags.undying = !flags.undying;
            flags.undying
        }
        GmFlag::Hidden => {
            flags.hidden = !flags.hidden;
            flags.hidden
        }
    };
    world.objects.add_components(&target, flags);
    now
}

/// `AdminHide`'s `//hide` — toggle the GM's visibility to other players (Java
/// `setInvisible` + `decayMe`/`spawnMe`). Hiding sends `DeleteObject` to nearby
/// players; unhiding re-runs the visibility exchange. While hidden, the
/// visibility system won't describe the GM to anyone (see `send_char_info`).
fn admin_hide(world: &mut World, client_id: u32, object_id: i32) {
    let hidden = set_flag(world, object_id, GmFlag::Hidden);
    if hidden {
        super::helpers::broadcast_to_others(world, object_id, &server_packets::delete_object(object_id));
        send_message(world, client_id, "You are now hidden.");
    } else {
        super::visibility::on_enter_world(world, client_id, object_id);
        send_message(world, client_id, "You are now visible.");
    }
}

/// `//invul` / `//undying` — toggle the flag on the GM.
fn toggle_flag(world: &mut World, client_id: u32, object_id: i32, flag: GmFlag) {
    let on = set_flag(world, object_id, flag);
    send_message(world, client_id, &format!("{} {}.", flag.label(), if on { "enabled" } else { "disabled" }));
}

/// `//setinvul` / `//setundying` — toggle the flag on the targeted player.
fn toggle_flag_on_target(world: &mut World, client_id: u32, object_id: i32, flag: GmFlag) {
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid))
    else {
        send_message(world, client_id, "Select a player first.");
        return;
    };
    let on = set_flag(world, target, flag);
    send_message(world, client_id, &format!("Target {} {}.", flag.label().to_lowercase(), if on { "enabled" } else { "disabled" }));
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

/// `AdminSpawn`'s `//spawn <npcId>` — spawn one NPC at the GM's location with
/// no respawn (the permanent/respawn forms are TODO).
fn admin_spawn(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(npc_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //spawn <npcId>");
        return;
    };
    if world.data.npc_data.get(npc_id).is_none() {
        send_message(world, client_id, &format!("NPC id {npc_id} does not exist."));
        return;
    }
    let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() else { return };
    if let Some(spawned) = crate::model::npc::spawn_npc_at(world, npc_id, pos.x, pos.y, pos.z, pos.heading) {
        super::death::introduce_npc(world, spawned);
    }
}

/// `AdminDelete`'s `//delete` — despawn the targeted NPC.
fn admin_delete(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id) else {
        send_message(world, client_id, "Select an NPC first.");
        return;
    };
    if !world.objects.has_component::<crate::model::npc::Npc>(&target) {
        send_message(world, client_id, "Target is not an NPC.");
        return;
    }
    let Some(region) = world.objects.get_component::<crate::model::components::RegionCell>(&target).map(|r| r.0)
    else {
        return;
    };
    super::death::despawn_npc(world, target, region);
}

/// `AdminSkill`'s `//add_skill <id> [level]` — grant a skill to the targeted
/// player (or self) and refresh their skill list. Passive stat effects apply on
/// the next recompute/relog (the full immediate-passive path is TODO).
fn admin_add_skill(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //add_skill <id> [level]");
        return;
    };
    let level = args.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(1).max(1);
    if world.data.skill_data.get(skill_id, level).is_none() {
        send_message(world, client_id, &format!("Skill {skill_id} level {level} does not exist."));
        return;
    }
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    if let Some(book) = world.objects.get_component_mut::<crate::model::components::SkillBook>(&target) {
        book.0.insert(skill_id, level);
    }
    refresh_skill_list(world, target);
    send_message(world, client_id, &format!("Added skill {skill_id} (level {level})."));
}

/// `AdminSkill`'s `//remove_skill <id>` — remove a skill from the targeted
/// player (or self).
fn admin_remove_skill(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //remove_skill <id>");
        return;
    };
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    if let Some(book) = world.objects.get_component_mut::<crate::model::components::SkillBook>(&target) {
        book.0.remove(&skill_id);
    }
    refresh_skill_list(world, target);
    send_message(world, client_id, &format!("Removed skill {skill_id}."));
}

/// The integer `Player` fields `//editchar` can set directly.
#[derive(Clone, Copy)]
enum IntField {
    Reputation,
    Fame,
    Pk,
    Pvp,
}

impl IntField {
    fn label(self) -> &'static str {
        match self {
            IntField::Reputation => "Reputation",
            IntField::Fame => "Fame",
            IntField::Pk => "PK count",
            IntField::Pvp => "PvP count",
        }
    }
}

/// `//setreputation`/`//setfame`/`//setpk`/`//setpvp <n>` — set an integer field
/// on the target player (or self).
fn set_int_field(world: &mut World, client_id: u32, object_id: i32, field: IntField, args: &[&str]) {
    let Some(value) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, &format!("Usage: //set{} <value>", field.label().to_lowercase()));
        return;
    };
    set_field_value(world, client_id, object_id, field, value);
}

fn set_field_value(world: &mut World, client_id: u32, object_id: i32, field: IntField, value: i32) {
    let target = target_player(world, object_id);
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        match field {
            IntField::Reputation => p.reputation = value,
            IntField::Fame => p.fame = value,
            IntField::Pk => p.pk_kills = value,
            IntField::Pvp => p.pvp_kills = value,
        }
    }
    super::party::broadcast_user_info(world, target);
    send_message(world, client_id, &format!("{} set to {value}.", field.label()));
}

/// `AdminEditChar`'s `//setclass <id>` — change the target player's (or self's)
/// class. Reuses `set_level` at the current level to recompute vitals/stats and
/// grant the new class's skills, then rebroadcasts. The old class's skills are
/// not pruned yet (Java's full class-transfer cleanup is TODO).
fn admin_setclass(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(class_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //setclass <classId>");
        return;
    };
    if world.data.player_templates.get(class_id).is_none() {
        send_message(world, client_id, &format!("Class id {class_id} does not exist."));
        return;
    }
    let target = target_player(world, object_id);
    let Some(level) = world.objects.get_component::<Player>(&target).map(|p| p.level) else { return };
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.class_id = class_id;
        p.base_class_id = class_id;
    }
    // Recompute HP/MP/stats + grant the new class's reachable skills, with the
    // status/UserInfo/SkillList refresh; then CharInfo to nearby (new class).
    super::death::set_level(world, target, level);
    super::party::broadcast_user_info(world, target);
    send_message(world, client_id, &format!("Class set to {class_id}."));
}

/// `//settitle <text>` — set the target player's title.
fn admin_set_title(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let title = args.join(" ");
    let target = target_player(world, object_id);
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.title = title;
    }
    super::party::broadcast_user_info(world, target);
    send_message(world, client_id, "Title changed.");
}

/// `//setcolor`/`//settcolor <hex>` — set the target player's name/title color.
fn admin_set_color(world: &mut World, client_id: u32, object_id: i32, args: &[&str], title: bool) {
    let Some(color) = args.first().and_then(|s| i32::from_str_radix(s.trim_start_matches("0x"), 16).ok()) else {
        send_message(world, client_id, "Usage: //setcolor <hex, e.g. FF0000>");
        return;
    };
    let target = target_player(world, object_id);
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        if title {
            p.title_color = color;
        } else {
            p.name_color = color;
        }
    }
    super::party::broadcast_user_info(world, target);
    send_message(world, client_id, "Color changed.");
}

/// `//setsex` — flip the target player's gender.
fn admin_set_sex(world: &mut World, client_id: u32, object_id: i32) {
    let target = target_player(world, object_id);
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.is_female = !p.is_female;
    }
    super::party::broadcast_user_info(world, target);
    send_message(world, client_id, "Gender flipped.");
}

/// The vitals `//editchar` can set directly.
#[derive(Clone, Copy)]
enum Vital {
    Hp,
    Mp,
    Cp,
}

/// `//set_hp`/`//set_mp`/`//set_cp <n>` — set a vital on the target player (or
/// self), clamped to its max, with a `StatusUpdate`.
fn set_vital(world: &mut World, client_id: u32, object_id: i32, vital: Vital, args: &[&str]) {
    let Some(value) = args.first().and_then(|s| s.parse::<f64>().ok()) else {
        send_message(world, client_id, "Usage: //set_hp <value>");
        return;
    };
    let target = target_player(world, object_id);
    let update = {
        match vital {
            Vital::Hp | Vital::Mp => {
                let Some(v) = world.objects.get_component_mut::<Vitals>(&target) else { return };
                match vital {
                    Vital::Hp => {
                        v.cur_hp = value.clamp(0.0, v.max_hp as f64);
                        v.dead = v.cur_hp < 0.5;
                        (sut::CUR_HP, v.cur_hp as i32)
                    }
                    _ => {
                        v.cur_mp = value.clamp(0.0, v.max_mp as f64);
                        (sut::CUR_MP, v.cur_mp as i32)
                    }
                }
            }
            Vital::Cp => {
                let Some(pv) = world.objects.get_component_mut::<PlayerVitals>(&target) else { return };
                pv.cur_cp = value.clamp(0.0, pv.max_cp as f64);
                (sut::CUR_CP, pv.cur_cp as i32)
            }
        }
    };
    let packet = server_packets::status_update(target, &[update]);
    if let Some(cid) = super::helpers::client_for_player(world, target) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(packet);
        }
    }
    super::party::notify_party_vitals(world, target);
}

/// `AdminBuffs`'s `//stopbuff <skillId>` — remove a single buff from the target
/// (reuses the buff-expiry path: stat revert + rebroadcast).
fn admin_stopbuff(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //stopbuff <skillId>");
        return;
    };
    let target = current_target(world, object_id).unwrap_or(object_id);
    crate::game_loop::skills::effects::handle_buff_expire(world, target, skill_id);
    send_message(world, client_id, &format!("Removed buff {skill_id}."));
}

/// `AdminBuffs`'s `//stopallbuffs` (confirmDlg) — remove every timed buff from
/// the target (passive grade-penalty pumps are kept). Each removal reverts its
/// stat contribution through the buff-expiry path.
fn admin_stopallbuffs(world: &mut World, client_id: u32, object_id: i32) {
    let target = current_target(world, object_id).unwrap_or(object_id);
    let skill_ids: Vec<i32> = world
        .objects
        .get_component::<crate::model::components::Buffs>(&target)
        .map(|b| b.0.iter().filter(|x| !x.passive).map(|x| x.skill_id).collect())
        .unwrap_or_default();
    let count = skill_ids.len();
    for skill_id in skill_ids {
        crate::game_loop::skills::effects::handle_buff_expire(world, target, skill_id);
    }
    send_message(world, client_id, &format!("Removed {count} buff(s)."));
}

/// `AdminBuffs`'s `//getbuffs` — list the target player's active (non-passive)
/// buffs as text (Java shows an HTML window; documented simplification).
fn admin_getbuffs(world: &mut World, client_id: u32, object_id: i32) {
    let target = target_player(world, object_id);
    let name = world.objects.get_component::<Player>(&target).map(|p| p.name.clone()).unwrap_or_default();
    let now = world.tick;
    let lines: Vec<String> = world
        .objects
        .get_component::<crate::model::components::Buffs>(&target)
        .map(|b| {
            b.0.iter()
                .filter(|x| !x.passive)
                .map(|x| {
                    let secs = x.expires_at_tick.saturating_sub(now) / 10;
                    format!("  skill {} lvl {} — {secs}s left", x.skill_id, x.skill_level)
                })
                .collect()
        })
        .unwrap_or_default();
    send_message(world, client_id, &format!("=== Buffs on {name} ({} active) ===", lines.len()));
    for line in lines {
        send_message(world, client_id, &line);
    }
}

/// `AdminBuffs`'s `//buff <skillId> [level]` — apply a skill's effects to the
/// target (any creature) or self, exactly as a cast would (reuses the cast
/// effect pipeline, so buffs/heals broadcast correctly).
fn admin_buff(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //buff <skillId> [level]");
        return;
    };
    let level = args.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(1).max(1);
    let Some(skill) = world.data.skill_data.get(skill_id, level).cloned() else {
        send_message(world, client_id, &format!("Skill {skill_id} level {level} does not exist."));
        return;
    };
    let target = current_target(world, object_id).unwrap_or(object_id);
    crate::game_loop::skills::effects::apply_skill_effects(world, object_id, target, &skill);
}

/// `AdminEnchant`'s per-slot `//set<slot> <value>` — set the enchant level of
/// the item equipped in `slot` on the targeted player (or self). Enchant *stat*
/// bonuses aren't applied yet (item stats are a later milestone); this sets the
/// stored level, refreshes the inventory, and rebroadcasts UserInfo (glow).
fn admin_set_enchant(world: &mut World, client_id: u32, object_id: i32, slot: PaperdollSlot, args: &[&str]) {
    let Some(value) = args.first().and_then(|s| s.parse::<i32>().ok()).filter(|v| (0..=127).contains(v))
    else {
        send_message(world, client_id, "Usage: //set<slot> <0..127>");
        return;
    };
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    let changed = world
        .objects
        .get_component_mut::<Inventory>(&target)
        .and_then(|inv| inv.set_paperdoll_enchant(slot, value));
    let Some(item_oid) = changed else {
        send_message(world, client_id, "No item equipped in that slot.");
        return;
    };
    if let Some(cid) = super::helpers::client_for_player(world, target) {
        if let Some(inv) = world.objects.get_component::<Inventory>(&target) {
            let packet = crate::network::enter_world::inventory_update(inv, &world.data, &[item_oid]);
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(packet);
            }
        }
    }
    super::party::broadcast_user_info(world, target);
    send_message(world, client_id, &format!("Enchant set to +{value}."));
}

/// Resend a player's `SkillList` after a skill-book change.
fn refresh_skill_list(world: &World, target: i32) {
    let Some(cid) = super::helpers::client_for_player(world, target) else { return };
    let Some(book) = world.objects.get_component::<crate::model::components::SkillBook>(&target) else {
        return;
    };
    let packet = crate::network::enter_world::skill_list(book, &world.data);
    if let Some(cs) = world.clients.get(&cid) {
        cs.send(packet);
    }
}

/// `AdminTarget`'s `//target <name>` — select a player by name (reuses the
/// normal targeting flow, including its Z-distance guard).
fn admin_target(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
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

// ---------------------------------------------------------------------------
// AdminEffects — the broadcast-driven visual/environment commands. The
// abnormal-visual-effect subset (`//invis`/`//para`/`//bighead`/…, teams,
// `//settargetable`, `//playmovie`, `//event_trigger`, `//set_displayeffect`)
// needs a per-creature AbnormalVisualEffect list / Team / targetable runtime
// state this server does not model yet, so those stay deferred (still gated by
// `AdminCommands.xml`, reaching the "not implemented" path).
// ---------------------------------------------------------------------------

/// Whether `oid` is a `Creature` in Java terms — a player or an NPC (the only
/// creature kinds this server models; doors/static objects are not creatures).
fn is_creature(world: &World, oid: i32) -> bool {
    world.objects.has_component::<Player>(&oid)
        || world.objects.has_component::<crate::model::npc::Npc>(&oid)
}

/// Java `WorldObject.getName()` for GM feedback — player name, else the NPC
/// template name, else the object id.
fn object_name(world: &World, oid: i32) -> String {
    if let Some(p) = world.objects.get_component::<Player>(&oid) {
        return p.name.clone();
    }
    if let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&oid) {
        if let Some(t) = world.data.npc_data.get(npc.npc_id) {
            return t.name.clone();
        }
    }
    oid.to_string()
}

/// Port of `AdminEffects.performSocial` — broadcast a `SocialAction` on
/// `target`, gated by the same action-id ranges (NPCs 1..=20, players 2..=18 or
/// the level-up gesture). Returns whether the gesture was performed;
/// `NOTHING_HAPPENED` is sent to the GM on the out-of-range rejections exactly
/// as Java does inside this method.
fn perform_social(world: &World, action: i32, target: i32, gm_client_id: u32) -> bool {
    if !is_creature(world, target) {
        return false;
    }
    let is_npc = world.objects.has_component::<crate::model::npc::Npc>(&target);
    // (Java also rejects `Chest` NPCs outright; no Chest type exists here.)
    if is_npc && !(1..=20).contains(&action) {
        send_sm(world, gm_client_id, sm_ids::NOTHING_HAPPENED);
        return false;
    }
    if !is_npc
        && (action < 2 || (action > 18 && action != server_packets::SOCIAL_ACTION_LEVEL_UP))
    {
        send_sm(world, gm_client_id, sm_ids::NOTHING_HAPPENED);
        return false;
    }
    let packet = server_packets::social_action(target, action);
    super::helpers::broadcast_including_self(world, target, &packet);
    true
}

/// `AdminEffects`' `//social <id> [player_name|radius]` — play a social gesture
/// on the target/self, a named player, or every creature within a radius.
fn admin_social(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    match args.len() {
        2 => {
            let Some(social) = args[0].parse::<i32>().ok() else { return };
            let who = args[1];
            if let Some(pid) = find_online_player(world, who) {
                if perform_social(world, social, pid, client_id) {
                    let name = object_name(world, pid);
                    send_message(world, client_id, &format!("{name} was affected by your request."));
                }
            } else if let Ok(radius) = who.parse::<i32>() {
                let Some(center) = world.objects.get_component::<Position>(&object_id).copied() else { return };
                for oid in creatures_in_range(world, &center, radius, object_id) {
                    perform_social(world, social, oid, client_id);
                }
                send_message(world, client_id, &format!("{radius} units radius affected by your request."));
            } else {
                send_message(world, client_id, "Incorrect parameter");
            }
        }
        1 => {
            let Some(social) = args[0].parse::<i32>().ok() else { return };
            let target = current_target(world, object_id).unwrap_or(object_id);
            if perform_social(world, social, target, client_id) {
                let name = object_name(world, target);
                send_message(world, client_id, &format!("{name} was affected by your request."));
            } else {
                send_sm(world, client_id, sm_ids::NOTHING_HAPPENED);
            }
        }
        _ => send_message(world, client_id, "Usage: //social <social_id> [player_name|radius]"),
    }
}

/// Every creature (player or NPC) within `radius` of `center`, excluding
/// `exclude` — Java `World.forEachVisibleObjectInRange(activeChar, …)`, which
/// omits the reference object itself.
fn creatures_in_range(world: &World, center: &Position, radius: i32, exclude: i32) -> Vec<i32> {
    let r = radius as f64;
    let mut out = Vec::new();
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let oid = s.player_object_id();
            if oid == exclude {
                continue;
            }
            if world.objects.get_component::<Position>(&oid).is_some_and(|p| center.distance_2d(p) <= r) {
                out.push(oid);
            }
        }
    }
    let region = crate::world::region_of(center.x, center.y);
    for oid in world.npcs_visible_from(region) {
        if world.objects.get_component::<Position>(&oid).is_some_and(|p| center.distance_2d(p) <= r) {
            out.push(oid);
        }
    }
    out
}

/// `AdminEffects`' `//effect` / `//npc_use_skill <skill> [level [hittime]]` —
/// broadcast a `MagicSkillUse` so the targeted creature (or the GM if none)
/// plays the skill's animation toward the GM. Purely cosmetic (no effects run).
fn admin_effect(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //effect skill [level | level hittime]");
        return;
    };
    let level = args.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(1);
    let hit_time = args.get(2).and_then(|s| s.parse::<i32>().ok()).unwrap_or(1);
    // Java: obj = target, or self if none; must be a creature.
    let source = current_target(world, object_id).unwrap_or(object_id);
    if !is_creature(world, source) {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    }
    let (Some(src_pos), Some(gm_pos)) = (
        world.objects.get_component::<Position>(&source).copied(),
        world.objects.get_component::<Position>(&object_id).copied(),
    ) else {
        return;
    };
    let packet = server_packets::magic_skill_use_raw(
        (source, src_pos.x, src_pos.y, src_pos.z),
        (object_id, gm_pos.x, gm_pos.y, gm_pos.z),
        skill_id,
        level,
        hit_time,
    );
    super::helpers::broadcast_including_self(world, source, &packet);
    let name = object_name(world, source);
    send_message(world, client_id, &format!("{name} performs MSU {skill_id}/{level} by your request."));
}

/// `AdminEffects`' `//earthquake <intensity> <duration>` — a localised
/// screen-shake centred on the GM, broadcast to the surrounding regions.
fn admin_earthquake(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(intensity), Some(duration)) = (
        args.first().and_then(|s| s.parse::<i32>().ok()),
        args.get(1).and_then(|s| s.parse::<i32>().ok()),
    ) else {
        send_message(world, client_id, "Usage: //earthquake <intensity> <duration>");
        return;
    };
    let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() else { return };
    let packet = server_packets::earthquake(pos.x, pos.y, pos.z, intensity, duration);
    super::helpers::broadcast_including_self(world, object_id, &packet);
}

/// `AdminEffects`' `//atmosphere <type> <state> <duration>` — port of
/// `adminAtmosphere`: only `sky day|night|red` is a real packet; the
/// `signsky` form is a no-op in Java too. Broadcast to *all* online players
/// (`Broadcast.toAllOnlinePlayers`), not just the surrounding regions.
fn admin_atmosphere(world: &mut World, client_id: u32, args: &[&str]) {
    let usage = "Usage: //atmosphere <signsky dawn|dusk>|<sky day|night|red> <duration>";
    let (Some(&kind), Some(&state)) = (args.first(), args.get(1)) else {
        send_message(world, client_id, usage);
        return;
    };
    let duration = args.get(2).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
    let packet = if kind == "sky" {
        match state {
            "night" => Some(server_packets::sun_set()),
            "day" => Some(server_packets::sun_rise()),
            "red" => Some(server_packets::ex_red_sky(if duration != 0 { duration } else { 10 })),
            _ => None,
        }
    } else {
        None
    };
    let Some(packet) = packet else {
        send_message(world, client_id, usage);
        return;
    };
    for cs in world.clients.values() {
        if matches!(cs, ClientSession::InGame(_)) {
            cs.send(packet.clone());
        }
    }
}

/// `AdminEffects`' `//play_sound <name>` — play a client sound for the GM and
/// everyone who can see them.
fn admin_play_sound(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(&sound) = args.first() else {
        send_message(world, client_id, "Usage: //play_sound <soundname>");
        return;
    };
    let packet = server_packets::play_sound(sound);
    super::helpers::broadcast_including_self(world, object_id, &packet);
    send_message(world, client_id, &format!("Playing {sound}."));
}

/// Send a bare `SystemMessage(id)` to one client.
fn send_sm(world: &World, client_id: u32, message_id: i16) {
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
