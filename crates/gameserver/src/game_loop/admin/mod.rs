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

use commons::audit;
use serde_json::json;
use tracing::warn;

use crate::enums::AdminTeleportType;
use crate::game_loop::guard;
use crate::model::Player;
use crate::model::inventory::PaperdollSlot;
use crate::network::server_packets::{self, sm_ids};
use crate::session::ClientSession;
use crate::world::World;

mod castle;
mod character;
pub(crate) mod cursed_weapons;
pub(crate) mod debug_draw;
mod editchar;
pub(crate) mod effects;
mod events;
mod flags;
mod geo_editor;
mod gm_util;
mod grand_boss;
pub(crate) mod hero;
mod instance;
mod items;
mod menu;
pub(crate) mod missing_htmls;
mod mobgroup;
pub(crate) mod moderation;
pub(crate) mod mounts;
pub(crate) mod npc_info;
mod pforge;
pub(crate) use world_cmds::{begin_shutdown, server_shutdown_tick};
/// `PlayerCondOverride.SEE_ALL_PLAYERS.ordinal()` — the visibility consumer.
pub(crate) const SEE_ALL_PLAYERS_ORDINAL: u8 = 13;
mod pledge;
mod points;
pub(crate) mod premium;
mod skills;
mod spawn;
mod teleport;
pub(crate) mod transforms;
pub(crate) mod vitals;
mod world_cmds;

// The command bodies and the small enums the dispatch table names live in the
// modules above; glob them in so `dispatch` reads as one flat routing table.
use castle::*;
use character::*;
use cursed_weapons::*;
use editchar::*;
use effects::*;
use flags::*;
use gm_util::*;
use grand_boss::*;
use hero::*;
use menu::*;

// The enter-world GM startup block (`EnterWorld.runImpl`) is driven from
// `lobby::handle_enter_world`, so re-export it out of the admin module.
pub(crate) use flags::apply_gm_startup;
use instance::*;
use items::*;
use mobgroup::*;
use moderation::*;
use mounts::*;
use pledge::*;
use points::*;
use premium::*;
// `SkillList` resends aren't admin-only either: the cursed-weapon login restore
// (`game_loop::cursed_weapon`) grants a skill outside any GM command.
pub(crate) use skills::refresh_skill_list;
use skills::*;
use spawn::*;
use teleport::*;
use transforms::*;
use vitals::*;
use world_cmds::*;

// Sibling `game_loop` modules the command bodies reach through `super::`. Before
// `admin` became a folder these were plain `super::` siblings; re-importing them
// here keeps every `super::helpers::…` / `super::death::…` call in the bodies
// resolving (a child's `super` now points at this module).
pub(super) use crate::game_loop::helpers::send_sm_bare_to_client as send_sm;
use crate::game_loop::{death, helpers, net, party, quests, target, visibility};

/// Java `AdminCommandHandler.useAdminCommand`. `full` is the whole command
/// string *including* the `admin_` prefix, e.g. `"admin_heal 100"`.
///
/// Java runs the body on a threadpool task (server-freeze protection); the game
/// loop is single-threaded here, so it runs inline.
pub(crate) fn use_admin_command(world: &mut World, client_id: u32, full: &str, use_confirm: bool) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();
    let Some(player) = world.objects.get_component::<Player>(&object_id) else {
        return;
    };

    // Java `if (!player.isGM()) return;` — silent, no message.
    if !player.is_gm(&world.data) {
        return;
    }
    let access_level = player.access_level;

    // Command word = the first whitespace-delimited token (Java
    // `fullCommand.split(" ")[0]`); the rest are arguments. Lowercased so both
    // the access table and the dispatch below are case-insensitive, matching
    // L2J's `switch (actualCommand.toLowerCase())` (camelCase commands like
    // `admin_deleteNpcByObjectId` otherwise miss the lowercase dispatch arms).
    let command = full
        .split_whitespace()
        .next()
        .unwrap_or(full)
        .to_ascii_lowercase();
    let display = command
        .strip_prefix("admin_")
        .unwrap_or(&command)
        .to_string();

    // Handler existence (Java `getHandler(command) == null`). The "known" set
    // is the AdminCommands.xml command table — but Java's `AdminData.hasAccess`
    // **auto-grants unlisted commands to the highest access level** (the dist
    // xml genuinely lacks e.g. `admin_settargetable`, which AdminEffects
    // registers), so a top-level GM's unlisted command falls through to the
    // dispatch instead of "does not exist".
    if !world.data.admin.has_command(&command)
        && !world.data.admin.has_access(&command, access_level)
    {
        send_message(
            world,
            client_id,
            &format!("The command '{display}' does not exist!"),
        );
        warn!("No handler registered for admin command '{command}'.");
        return;
    }

    // Access rights (Java `AdminData.hasAccess`).
    if !world.data.admin.has_access(&command, access_level) {
        send_message(
            world,
            client_id,
            "You don't have the access rights to use this command!",
        );
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

    // GMAudit (Java `AdminCommandHandler` → `GMAudit.auditGMAction`, gated by
    // `Config.GMAUDIT`). Java resolves the GM's current target here and records
    // "no-target" when nothing is selected; the record goes to the never-dropped
    // audit sink, not the diagnostic log.
    if world.cfg.general.gm_audit {
        let target = target_display_name(world, object_id);
        if let Some(p) = world.objects.get_component::<Player>(&object_id) {
            audit::record(
                audit::Category::GmAudit,
                json!({
                    "gm": p.name,
                    "gm_oid": object_id,
                    "command": full,
                    "target": target,
                }),
            );
        }
    }

    // A gated-but-unimplemented command (G13.C) lands on the `false` arm.
    if !dispatch(world, client_id, object_id, &command, full) {
        send_message(
            world,
            client_id,
            &format!("Admin command '{display}' is not implemented yet."),
        );
    }
}

/// Port of `clientpackets/DlgAnswer` narrowed to the admin-confirm case (the
/// only `ConfirmDlg` this server sends; door/summon/offline-play/olympiad
/// dialogs are unported). On the echoed `S1_3` id, the pending command is
/// consumed and — on "yes" — re-run with confirmation disabled (Java
/// `useAdminCommand(player, cmd, false)`).
pub(crate) fn handle_dlg_answer(
    world: &mut World,
    client_id: u32,
    answer: crate::network::client_packets::DlgAnswer,
) {
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
        | "admin_admin5" | "admin_admin6" | "admin_admin7" => {
            admin_admin(world, client_id, command)
        }
        "admin_serverinfo" => admin_serverinfo(world, client_id),
        // `AdminManor` — the manor status page (`game_menu.htm`'s "Manor info").
        "admin_manor" => castle::admin_manor(world, client_id),
        // Instances (G27) — the `AdminInstance` panel: overview, template list /
        // detail, and create / teleport / destroy.
        "admin_instance" | "admin_instances" => admin_instance_panel(world, client_id),
        // Note: the overview page's "Show me all templates" button fires
        // `admin_listinstances`, which retail's AdminCommands.xml never
        // registers — so that button is inert in retail too; only
        // `admin_instancelist` (or typing `//instancelist`) reaches the list.
        "admin_instancelist" => admin_instance_list(world, client_id, &args),
        "admin_instancecreate" => admin_instance_create(world, client_id, object_id, &args),
        "admin_instanceteleport" => admin_instance_teleport(world, client_id, object_id, &args),
        "admin_instancedestroy" => admin_instance_destroy(world, client_id, &args),
        "admin_event_start" => events::admin_event_start(world, client_id, &args),
        "admin_event_stop" => events::admin_event_stop(world, client_id, &args),
        "admin_heal" => admin_heal(world, client_id, object_id, &args),
        "admin_kill" => admin_kill(world, client_id, object_id, &args, false),
        "admin_kill_monster" => admin_kill(world, client_id, object_id, &args, true),
        "admin_res" => admin_res(world, client_id, object_id, &args),
        "admin_res_monster" => admin_res_monster(world, client_id, object_id, &args),
        "admin_gmspeed" => admin_gmspeed(world, client_id, object_id, &args),
        // Super-haste movement buff (self).
        "admin_superhaste" | "admin_speed" => admin_superhaste(world, client_id, object_id, &args),
        "admin_superhaste_menu" | "admin_speed_menu" => {
            menu::show_admin_html(world, client_id, "gm_menu.htm")
        }
        // Directional self-nudge.
        "admin_gonorth" => admin_go(world, client_id, object_id, "north", &args),
        "admin_gosouth" => admin_go(world, client_id, object_id, "south", &args),
        "admin_goeast" => admin_go(world, client_id, object_id, "east", &args),
        "admin_gowest" => admin_go(world, client_id, object_id, "west", &args),
        "admin_goup" => admin_go(world, client_id, object_id, "up", &args),
        "admin_godown" => admin_go(world, client_id, object_id, "down", &args),
        // Move GM to coords / send a player home / teleport a player to coords.
        "admin_walk" => admin_walk(world, client_id, object_id, &args),
        "admin_sendhome" => admin_sendhome(world, client_id, object_id, &args),
        "admin_teleport_character" => admin_teleport_character(world, client_id, object_id, &args),
        "admin_recall_npc" => admin_recall_npc(world, client_id, object_id),
        // Teleport HTML menus — `admin_tele` is the "Additional Movement
        // Options" window (`move.htm`), NOT a coordinate teleport.
        "admin_show_moves" | "admin_show_moves_other" | "admin_show_teleport" | "admin_tele" => {
            admin_teleport_menu(world, client_id, command)
        }
        // AdminRide — strider/wolf/wyvern are mounts; horse/bike are transforms.
        "admin_ride_strider" => admin_ride(world, client_id, object_id, mounts::Mount::Strider),
        "admin_ride_wolf" => admin_ride(world, client_id, object_id, mounts::Mount::Wolf),
        "admin_ride_wyvern" => admin_ride(world, client_id, object_id, mounts::Mount::Wyvern),
        "admin_ride_horse" => admin_ride_transform(world, client_id, object_id, 106),
        "admin_ride_bike" => admin_ride_transform(world, client_id, object_id, 20001),
        "admin_unride" | "admin_unride_strider" | "admin_unride_wolf" | "admin_unride_wyvern" => {
            admin_dismount_or_untransform(world, object_id)
        }
        // AdminTransform.
        "admin_transform" => admin_transform(world, client_id, object_id, &args),
        "admin_untransform" => admin_untransform(world, object_id),
        // `AdminTransform`: the Effects panel's "Transform" button opens the
        // transform sub-page, not the main GM menu (Java
        // `AdminHtml.showAdminHtml(activeChar, "transform.htm")`).
        "admin_transform_menu" => menu::show_admin_html(world, client_id, "transform.htm"),
        // "Teleport" main-menu button: coords → teleport, empty → teleports.htm.
        "admin_move_to" => admin_move_to(world, client_id, object_id, &args),
        // Self-teleport to explicit coordinates.
        "admin_teleport" => admin_teleport_coords(world, client_id, object_id, &args),
        // Bring a player to the GM.
        "admin_recall" => admin_recall(world, client_id, object_id, &args),
        // Send the GM to a *named* player (`//teleto` uses the current target).
        "admin_teleportto" => admin_teleportto(world, client_id, object_id, &args),
        // "Additional Movement Options" click-to-move latches (`move.htm`'s
        // "Move:" row). `//instant_move` is a command of its own; the other
        // three ride on `//teleto <word>`, which otherwise teleports the GM to
        // its target.
        "admin_instant_move" => {
            admin_teleto_mode(world, client_id, object_id, AdminTeleportType::Demonic)
        }
        // Only the `//teleto` spelling carries the mode words in Java; the two
        // aliases are the plain teleport-to-character forms.
        "admin_teleto" => match args.first().copied() {
            Some("sayune") => {
                admin_teleto_mode(world, client_id, object_id, AdminTeleportType::Sayune)
            }
            Some("charge") => {
                admin_teleto_mode(world, client_id, object_id, AdminTeleportType::Charge)
            }
            Some("end") => {
                admin_teleto_mode(world, client_id, object_id, AdminTeleportType::Normal)
            }
            // Send the GM to the current target.
            _ => admin_teleto(world, client_id, object_id),
        },
        // `AdminMenu`'s target-based jump. Its `AdminTeleport` namesake
        // `//teleportto <name>` is a *different* command — by name, dispatched
        // above — and this arm used to answer for both.
        "admin_teleport_to_character" => admin_teleto(world, client_id, object_id),
        // Create an item on the GM.
        "admin_create_item" => admin_create_item(world, client_id, object_id, &args),
        // Create a named coin (adena/alt currencies) on the GM.
        "admin_create_coin" => admin_create_coin(world, client_id, object_id, &args),
        // Item / enchant HTML menus.
        "admin_itemcreate" => admin_item_menu(world, client_id, "itemcreation.htm"),
        "admin_enchant" => admin_item_menu(world, client_id, "enchant.htm"),
        // Wipe the GM's own inventory (equipped included only for the `all` forms).
        "admin_destroy_all_items" | "admin_destroyallitems" => {
            admin_destroy_items(world, client_id, object_id, true)
        }
        "admin_destroy_items" | "admin_destroyitems" => {
            admin_destroy_items(world, client_id, object_id, false)
        }
        // Destroy part or all of one stack, by the item's object id.
        "admin_delete_item" => admin_delete_item(world, client_id, &args),
        // Custom: destroy by item (template) id, on the target or a named player.
        "admin_delete_quest_item" => admin_delete_quest_item(world, client_id, object_id, &args),
        // Give an item to the targeted player.
        "admin_give_item_target" => admin_give_item_target(world, client_id, object_id, &args),
        // Give an item to every online player.
        "admin_give_item_to_all" => admin_give_item_to_all(world, client_id, &args),
        // Disconnect a player (named or targeted).
        "admin_kick" => admin_kick(world, client_id, object_id, &args),
        // Add/remove exp/sp to the targeted player (Java requires a target).
        "admin_add_exp_sp" => admin_add_exp_sp(world, client_id, object_id, &args),
        "admin_remove_exp_sp" => admin_remove_exp_sp(world, client_id, object_id, &args),
        "admin_add_exp_sp_to_character" => admin_add_exp_sp_menu(world, client_id, object_id),
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
        // Jail / release a player (Java `AdminPunishment`, G31).
        "admin_jail" => admin_jail(world, client_id, object_id, &args),
        "admin_unjail" => admin_unjail(world, client_id, &args),
        // Ban / chat-ban / party-ban (Java `AdminPunishment`, G31 slice 2).
        "admin_ban_char" | "admin_ban" => admin_ban_char(world, client_id, object_id, &args),
        "admin_unban_char" | "admin_unban" => admin_unban_char(world, client_id, &args),
        "admin_ban_acc" => admin_ban_acc(world, client_id, object_id, &args),
        "admin_unban_acc" => admin_unban_acc(world, client_id, &args),
        "admin_ban_chat" | "admin_chatban" => admin_ban_chat(world, client_id, object_id, &args),
        "admin_unban_chat" | "admin_chatunban" => admin_unban_chat(world, client_id, &args),
        "admin_ban_party" | "admin_partyban" => admin_ban_party(world, client_id, object_id, &args),
        "admin_unban_party" | "admin_partyunban" => admin_unban_party(world, client_id, &args),
        // Petitions (Java `AdminPetition`, G31 slice 3).
        "admin_view_petitions" => super::petition::send_pending_list(world, object_id),
        "admin_view_petition" => {
            if let Some(id) = args.first().and_then(|a| a.parse().ok()) {
                super::petition::view_petition(world, object_id, id);
            }
        }
        "admin_accept_petition" => {
            if let Some(id) = args.first().and_then(|a| a.parse().ok()) {
                super::petition::accept_petition(world, object_id, id);
            }
        }
        "admin_reject_petition" => {
            if let Some(id) = args.first().and_then(|a| a.parse().ok()) {
                super::petition::reject_petition(world, object_id, id);
            }
        }
        "admin_reset_petitions" => super::petition::reset_petitions(world, object_id),
        // Login-ban relay + IP tools (Java `AdminEditChar`/login link, G31 slice 4).
        "admin_login_ban" => admin_login_ban(world, client_id, &args),
        "admin_login_unban" => admin_login_unban(world, client_id, &args),
        "admin_find_ip" => admin_find_ip(world, client_id, &args),
        "admin_find_dualbox" => admin_find_dualbox(world, client_id, &args),
        "admin_tracert" => admin_tracert(world, client_id, object_id, &args),
        "admin_snoop" => admin_snoop(world, client_id, object_id, &args),
        "admin_hwid" | "admin_hwinfo" => admin_hwid(world, client_id, object_id, &args),
        // Repair a broken offline character (Java `AdminRepairChar`, G33).
        "admin_repair" | "admin_restore" => admin_repair_char(world, client_id, &args),
        // Broadcast a message to every online player.
        "admin_announce" => admin_announce(world, client_id, &args),
        // Spawn NPC(s) at the anchor (target or GM).
        "admin_spawn" | "admin_spawn_monster" | "admin_spawn_once" => {
            admin_spawn(world, client_id, object_id, &args)
        }
        // Spawn one NPC at explicit coordinates.
        "admin_spawnat" => admin_spawnat(world, client_id, object_id, &args),
        // Spawn/NPC HTML menus.
        "admin_show_spawns" | "admin_show_npcs" | "admin_spawn_debug_menu" => {
            admin_spawn_menu(world, client_id, command)
        }
        // "Spawn by Level" List buttons → Monster-type NPCs of that level.
        "admin_spawn_index" => admin_spawn_index(world, client_id, &args),
        // A–Z letter buttons → Folk-type NPCs whose name starts with the letter.
        "admin_npc_index" => admin_npc_index(world, client_id, &args),
        // PC-cafe loyalty points (target player or self) + the pccafe.htm menu.
        "admin_pccafepoints" => admin_pccafepoints(world, client_id, object_id, &args),
        // Prime (NCoin) points — account-scoped — + the primepoints.htm menu.
        "admin_primepoints" => admin_primepoints(world, client_id, object_id, &args),
        // Premium account management (menu + add/info/remove by account name).
        "admin_premium_menu"
        | "admin_premium_add1"
        | "admin_premium_add2"
        | "admin_premium_add3"
        | "admin_premium_info"
        | "admin_premium_remove" => admin_premium(world, client_id, command, &args),
        // Spawn-line inspection + teleport-to-index (`goSpawn`/`goPosition`).
        "admin_list_spawns" => admin_list_spawns(world, client_id, object_id, &args, false),
        "admin_list_positions" => admin_list_spawns(world, client_id, object_id, &args, true),
        "admin_top_spawn_count" | "admin_topspawncount" => {
            admin_top_spawn_count(world, client_id, &args)
        }
        "admin_spawn_debug_print" | "admin_spawn_debug_print_menu" => {
            admin_spawn_debug_print(world, client_id, object_id)
        }
        // List NPCs visible from the GM.
        "admin_scan" => admin_scan(world, client_id, object_id, &args),
        // Delete an NPC by object id (the scan list's Delete links).
        "admin_deletenpcbyobjectid" => {
            admin_delete_npc_by_object_id(world, client_id, object_id, &args)
        }
        // Create item / one-off spawn by combined id.
        "admin_summon" => admin_summon(world, client_id, object_id, &args),
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
        // Toggle an abnormal visual effect on the target (or a radius).
        "admin_ave_abnormal" => flags::admin_ave_abnormal(world, client_id, object_id, &args),
        // Toggle message-refusal (chat block) mode.
        "admin_silence" => admin_silence(world, client_id, object_id),
        // Grant / remove a skill on the targeted player (or self).
        "admin_add_skill" => admin_add_skill(world, client_id, object_id, &args),
        "admin_remove_skill" => admin_remove_skill(world, client_id, object_id, &args),
        // Add a skill to the GM themselves.
        "admin_setskill" => admin_setskill(world, client_id, object_id, &args),
        // Grant / strip / reset / copy a player's whole skill set.
        "admin_give_all_skills" | "admin_give_all_skills_fs" => {
            admin_give_all_skills(world, client_id, object_id)
        }
        "admin_give_clan_skills" => admin_give_clan_skills(world, client_id, object_id, false),
        "admin_give_all_clan_skills" => admin_give_clan_skills(world, client_id, object_id, true),
        "admin_remove_all_skills" => admin_remove_all_skills(world, client_id, object_id),
        "admin_reset_skills" => admin_reset_skills(world, client_id, object_id),
        "admin_get_skills" => admin_get_skills(world, client_id, object_id),
        // Cast a skill's effects onto the current target (or self).
        "admin_cast" | "admin_castnow" => admin_cast(world, client_id, object_id, &args),
        // `//show_skills` opens the target's char-skills window (Java
        // `showMainPage` → `charskills.htm`), not the skill catalog.
        "admin_show_skills" => {
            let target = guard::target(world, object_id)
                .filter(|oid| world.objects.has_component::<crate::model::Player>(oid))
                .unwrap_or(object_id);
            skills::show_char_skills(world, client_id, target)
        }
        // Skill catalog HTML menus (browse skills to add).
        "admin_skill_list" | "admin_skill_index" | "admin_remove_skills" => {
            admin_skill_menu(world, client_id, command, &args)
        }
        // Per-slot enchant (`AdminEnchant`): //set<slot> <0..127>.
        "admin_seteh" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::Head, &args),
        "admin_setec" => {
            admin_set_enchant(world, client_id, object_id, PaperdollSlot::Chest, &args)
        }
        "admin_seteg" => {
            admin_set_enchant(world, client_id, object_id, PaperdollSlot::Gloves, &args)
        }
        "admin_setel" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::Legs, &args),
        "admin_seteb" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::Feet, &args),
        "admin_setew" => {
            admin_set_enchant(world, client_id, object_id, PaperdollSlot::RHand, &args)
        }
        "admin_setes" => {
            admin_set_enchant(world, client_id, object_id, PaperdollSlot::LHand, &args)
        }
        "admin_setle" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::LEar, &args),
        "admin_setre" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::REar, &args),
        "admin_setlf" => {
            admin_set_enchant(world, client_id, object_id, PaperdollSlot::LFinger, &args)
        }
        "admin_setrf" => {
            admin_set_enchant(world, client_id, object_id, PaperdollSlot::RFinger, &args)
        }
        "admin_seten" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::Neck, &args),
        "admin_setun" => {
            admin_set_enchant(world, client_id, object_id, PaperdollSlot::Under, &args)
        }
        "admin_setba" => {
            admin_set_enchant(world, client_id, object_id, PaperdollSlot::Cloak, &args)
        }
        "admin_setbe" => admin_set_enchant(world, client_id, object_id, PaperdollSlot::Belt, &args),
        // Apply a skill's effects (buff) to the target.
        "admin_buff" => admin_buff(world, client_id, object_id, &args),
        // List the target's active buffs.
        "admin_getbuffs" => admin_getbuffs(world, client_id, object_id, &args),
        "admin_getbuffs_ps" => admin_getbuffs_ps(world, client_id, object_id, &args),
        // Clear buffs in a radius / clear a player's skill reuse timers.
        "admin_areacancel" => admin_areacancel(world, client_id, object_id, &args),
        "admin_removereuse" => admin_removereuse(world, client_id, object_id, &args),
        // Remove one / all buffs from the target.
        "admin_stopbuff" => admin_stopbuff(world, client_id, object_id, &args),
        "admin_stopallbuffs" => admin_stopallbuffs(world, client_id, object_id),
        // EditChar field setters (target player or self).
        "admin_setreputation" => {
            set_int_field(world, client_id, object_id, IntField::Reputation, &args)
        }
        "admin_nokarma" => set_field_value(world, client_id, object_id, IntField::Reputation, 0),
        "admin_setfame" => set_int_field(world, client_id, object_id, IntField::Fame, &args),
        "admin_setpk" => set_int_field(world, client_id, object_id, IntField::Pk, &args),
        "admin_setpvp" => set_int_field(world, client_id, object_id, IntField::Pvp, &args),
        // Fill the targeted pet's food bar.
        "admin_fullfood" => admin_fullfood(world, client_id, object_id),
        "admin_settitle" => admin_set_title(world, client_id, object_id, &args),
        "admin_setcolor" => admin_set_color(world, client_id, object_id, &args, false),
        "admin_settcolor" => admin_set_color(world, client_id, object_id, &args, true),
        "admin_setsex" => admin_set_sex(world, client_id, object_id),
        "admin_set_hp" => set_vital(world, client_id, object_id, Vital::Hp, &args),
        "admin_set_mp" => set_vital(world, client_id, object_id, Vital::Mp, &args),
        "admin_set_cp" => set_vital(world, client_id, object_id, Vital::Cp, &args),
        "admin_setclass" => admin_setclass(world, client_id, object_id, &args),
        "admin_setsubclass" => character::admin_setsubclass(world, client_id, object_id, &args),
        "admin_changesubclass" => {
            character::admin_changesubclass(world, client_id, object_id, &args)
        }

        // AdminEditChar breadth — info/search, rename, party, pvp-flag, clan penalty.
        "admin_current_player" => admin_character_info(world, client_id, object_id, &args, true),
        "admin_character_info" => admin_character_info(world, client_id, object_id, &args, false),
        "admin_character_list" | "admin_show_characters" => {
            admin_character_list(world, client_id, &args)
        }
        "admin_find_character" => admin_find_character(world, client_id, &args),
        "admin_find_account" => admin_find_account(world, client_id, &args),
        "admin_edit_character" => admin_edit_character(world, client_id, object_id),
        "admin_changename" => admin_changename(world, client_id, object_id, &args),
        "admin_set_pvp_flag" => admin_set_pvp_flag(world, client_id, object_id),
        "admin_partyinfo" => admin_partyinfo(world, client_id, object_id, &args),
        "admin_remove_clan_penalty" => admin_remove_clan_penalty(world, client_id, &args),
        "admin_setparam" => admin_setparam(world, client_id, object_id, &args, true),
        "admin_unsetparam" => admin_setparam(world, client_id, object_id, &args, false),

        // --- AdminAdmin / GM utility & comms (B5) ---
        "admin_gmliston" => admin_gmlist(world, client_id, true),
        "admin_gmlistoff" => admin_gmlist(world, client_id, false),
        // Java `//gmon` is intentionally empty.
        "admin_gmon" => {}
        "admin_diet" => admin_diet(world, client_id, object_id, &args),
        "admin_worldchat" => admin_worldchat(world, client_id, object_id, &args),
        "admin_online" => admin_online(world, client_id),
        "admin_targetsay" => admin_targetsay(world, client_id, object_id, &args),
        "admin_msg" => admin_msg(world, client_id, &args),
        "admin_announce_screen" => admin_announce_variant(world, client_id, &args, true),
        "admin_announce_crit" | "admin_announces" => {
            admin_announce_variant(world, client_id, &args, false)
        }
        "admin_html" | "admin_loadhtml" => admin_html(world, client_id, &args),
        "admin_showdoors" => admin_showdoors(world, client_id, object_id),
        "admin_debug" => admin_debug(world, client_id, object_id, &args),
        "admin_stats" => admin_stats(world, client_id),
        "admin_kick_non_gm" => admin_kick_non_gm(world, client_id),
        "admin_gmchat_menu" => menu::show_admin_html(world, client_id, "gm_menu.htm"),
        // --- AdminMenu action buttons — delegate to the underlying handlers ---
        "admin_char_manage" => admin_character_info(world, client_id, object_id, &args, false),
        "admin_goto_char_menu" => admin_goto_char(world, client_id, object_id, &args),
        "admin_recall_char_menu" => admin_recall(world, client_id, object_id, &args),
        "admin_recall_party_menu" => admin_recall_party(world, client_id, object_id, &args),
        "admin_recall_clan_menu" => admin_recall_clan(world, client_id, object_id, &args),
        "admin_kick_menu" => admin_kick(world, client_id, object_id, &args),
        "admin_kill_menu" => admin_kill(world, client_id, object_id, &args, false),
        "admin_teleport_character_to_menu" => {
            menu::show_admin_html(world, client_id, "teleports.htm")
        }

        // --- AdminDoorControl / AdminZone / AdminShop / AdminClan (B6) ---
        "admin_open" => admin_door(world, client_id, object_id, true, false, &args),
        "admin_close" => admin_door(world, client_id, object_id, false, false, &args),
        "admin_openall" => admin_door(world, client_id, object_id, true, true, &args),
        "admin_closeall" => admin_door(world, client_id, object_id, false, true, &args),
        "admin_zones" | "admin_zone_check" => admin_zones(world, client_id, object_id),
        "admin_buy" => admin_buy(world, client_id, object_id, &args),
        "admin_gmshop" => menu::show_admin_html(world, client_id, "gmshops.htm"),
        "admin_clan_info" => admin_clan_info(world, client_id, object_id),
        // AdminPledge — the Game panel's "Clan Related" buttons (create / info /
        // setlevel / dismiss / rep), plus AdminClan's pending-leader listing.
        "admin_pledge" => admin_pledge(world, client_id, object_id, &args),
        "admin_clan_show_pending" => admin_clan_show_pending(world, client_id),
        "admin_clan_force_pending" => pledge::admin_clan_force_pending(world, client_id, &args),
        // AdminGrandBoss — the Game panel's "Grand Boss Info" button. Status
        // display is live; the skip/respawn/minions/abort actions await the
        // grand-boss AI (G21).
        "admin_grandboss" => admin_grandboss(world, client_id, &args),
        "admin_grandboss_skip"
        | "admin_grandboss_respawn"
        | "admin_grandboss_minions"
        | "admin_grandboss_abort" => admin_grandboss_action(world, client_id, command, &args),
        // AdminCursedWeapons — the Game panel's "Cursed Weapons" buttons.
        "admin_cw_info" => admin_cw_info(world, client_id),
        "admin_cw_info_menu" => admin_cw_info_menu(world, client_id),
        "admin_cw_reload" => admin_cw_reload(world),
        "admin_cw_add" => admin_cw_add(world, client_id, object_id, &args),
        "admin_cw_remove" => admin_cw_remove(world, client_id, &args),
        "admin_cw_goto" => admin_cw_goto(world, client_id, object_id, &args),
        // AdminAdmin hero commands — the Game panel's "Set Hero" / "Give
        // unclaimed Hero" buttons.
        "admin_sethero" => admin_sethero(world, client_id, object_id),
        "admin_setnoble" => hero::admin_setnoble(world, client_id, object_id),
        "admin_givehero" => admin_givehero(world, client_id, object_id),
        // AdminCastle — the Game panel's "Castle" button (siege actions deferred).
        "admin_castlemanage" => admin_castlemanage(world, client_id, object_id, &args),
        // AdminVitality (player vitality points).
        "admin_set_vitality" => admin_vitality(world, client_id, object_id, "set", &args),
        "admin_full_vitality" => admin_vitality(world, client_id, object_id, "full", &args),
        "admin_empty_vitality" => admin_vitality(world, client_id, object_id, "empty", &args),
        "admin_get_vitality" => admin_vitality(world, client_id, object_id, "get", &args),
        // AdminGeodata read-only queries.
        "admin_geo_pos" => admin_geo_pos(world, client_id, object_id, false),
        "admin_geo_spawn_pos" => admin_geo_pos(world, client_id, object_id, true),
        "admin_geo_can_move" | "admin_geo_can_see" => {
            admin_geo_can_see(world, client_id, object_id)
        }
        // AdminGeodata editor: tile/cell info, NSWE editing, the cell panels
        // and the region export. The `//en`/`//dn`/… aliases are the panel's
        // own buttons — they carry `<geoX> <geoY>` and re-open it on the edited
        // cell, which is Java's `!actualCommand.contains("geo")` branch.
        "admin_geomap" => admin_geomap(world, client_id, object_id),
        "admin_geocell" => admin_geocell(world, client_id, object_id),
        "admin_geoenablenorth" | "admin_en" => geo_editor::admin_geo_nswe(
            world,
            client_id,
            object_id,
            crate::geo::NSWE_NORTH,
            true,
            &args,
            command == "admin_en",
        ),
        "admin_geoenablesouth" | "admin_es" => geo_editor::admin_geo_nswe(
            world,
            client_id,
            object_id,
            crate::geo::NSWE_SOUTH,
            true,
            &args,
            command == "admin_es",
        ),
        "admin_geoenableeast" | "admin_ee" => geo_editor::admin_geo_nswe(
            world,
            client_id,
            object_id,
            crate::geo::NSWE_EAST,
            true,
            &args,
            command == "admin_ee",
        ),
        "admin_geoenablewest" | "admin_ew" => geo_editor::admin_geo_nswe(
            world,
            client_id,
            object_id,
            crate::geo::NSWE_WEST,
            true,
            &args,
            command == "admin_ew",
        ),
        "admin_geodisablenorth" | "admin_dn" => geo_editor::admin_geo_nswe(
            world,
            client_id,
            object_id,
            crate::geo::NSWE_NORTH,
            false,
            &args,
            command == "admin_dn",
        ),
        "admin_geodisablesouth" | "admin_ds" => geo_editor::admin_geo_nswe(
            world,
            client_id,
            object_id,
            crate::geo::NSWE_SOUTH,
            false,
            &args,
            command == "admin_ds",
        ),
        "admin_geodisableeast" | "admin_de" => geo_editor::admin_geo_nswe(
            world,
            client_id,
            object_id,
            crate::geo::NSWE_EAST,
            false,
            &args,
            command == "admin_de",
        ),
        "admin_geodisablewest" | "admin_dw" => geo_editor::admin_geo_nswe(
            world,
            client_id,
            object_id,
            crate::geo::NSWE_WEST,
            false,
            &args,
            command == "admin_dw",
        ),
        "admin_forge" => pforge::admin_forge(world, client_id),
        "admin_forge_values" => pforge::admin_forge_values(world, client_id, &args),
        "admin_forge_send" => pforge::admin_forge_send(world, client_id, object_id, &args),
        "admin_geomap_missing_htmls" => {
            missing_htmls::admin_geomap_missing_htmls(world, client_id, object_id)
        }
        "admin_world_missing_htmls" => missing_htmls::admin_world_missing_htmls(world, client_id),
        "admin_next_missing_html" => {
            missing_htmls::admin_next_missing_html(world, client_id, object_id)
        }
        "admin_geosave" => geo_editor::admin_geosave(world, client_id, object_id),
        "admin_geosaveall" => geo_editor::admin_geosaveall(world, client_id),
        "admin_geogrid" => debug_draw::admin_geogrid(world, client_id, object_id, &args),
        "admin_geoedit" => geo_editor::admin_geoedit(world, client_id, object_id),
        "admin_ge" => geo_editor::admin_ge(world, client_id, object_id, &args),
        // AdminPathNode.
        "admin_path_find" => admin_path_find(world, client_id, object_id),

        // --- AdminMobGroup (B8) ---
        "admin_mobmenu" => admin_mobmenu(world, client_id),
        "admin_mobgroup_list" => admin_mobgroup_list(world, client_id),
        "admin_mobgroup_create" => admin_mobgroup_create(world, client_id, &args),
        "admin_mobgroup_remove" | "admin_mobgroup_delete" => {
            admin_mobgroup_remove(world, client_id, object_id, &args)
        }
        "admin_mobgroup_spawn" => admin_mobgroup_spawn(world, client_id, object_id, &args),
        "admin_mobgroup_unspawn" => admin_mobgroup_unspawn(world, client_id, &args),
        "admin_mobgroup_kill" => admin_mobgroup_kill(world, client_id, object_id, &args),
        "admin_mobgroup_teleport" => admin_mobgroup_teleport(world, client_id, object_id, &args),
        "admin_mobgroup_invul" => admin_mobgroup_invul(world, client_id, &args),
        "admin_mobgroup_idle" => admin_mobgroup_state(world, client_id, object_id, "idle", &args),
        "admin_mobgroup_rnd" => admin_mobgroup_state(world, client_id, object_id, "rnd", &args),
        "admin_mobgroup_nomove" => {
            admin_mobgroup_state(world, client_id, object_id, "nomove", &args)
        }
        "admin_mobgroup_attack" => {
            admin_mobgroup_state(world, client_id, object_id, "attack", &args)
        }
        "admin_mobgroup_attackgrp" => {
            admin_mobgroup_state(world, client_id, object_id, "attackgrp", &args)
        }
        "admin_mobgroup_follow" => {
            admin_mobgroup_state(world, client_id, object_id, "follow", &args)
        }
        "admin_mobgroup_return" => {
            admin_mobgroup_state(world, client_id, object_id, "return", &args)
        }
        "admin_mobgroup_casting" => {
            admin_mobgroup_state(world, client_id, object_id, "casting", &args)
        }

        "admin_social" | "admin_social_menu" => admin_social(world, client_id, object_id, &args),
        "admin_effect" | "admin_npc_use_skill" => admin_effect(world, client_id, object_id, &args),
        "admin_earthquake" | "admin_earthquake_menu" => {
            admin_earthquake(world, client_id, object_id, &args)
        }
        "admin_atmosphere" | "admin_atmosphere_menu" => admin_atmosphere(world, client_id, &args),
        "admin_play_sound" => admin_play_sound(world, client_id, object_id, &args),
        // AdminEffects' G19 tail (see effects.rs).
        "admin_setteam" => effects::admin_setteam(world, client_id, object_id, &args, false),
        "admin_setteam_close" => effects::admin_setteam(world, client_id, object_id, &args, true),
        "admin_clearteams" => effects::admin_clearteams(world, client_id, object_id),
        "admin_settargetable" => effects::admin_settargetable(world, client_id, object_id),
        "admin_para" | "admin_para_menu" => {
            effects::admin_para(world, client_id, object_id, &args, true, false)
        }
        "admin_unpara" | "admin_unpara_menu" => {
            effects::admin_para(world, client_id, object_id, &args, false, false)
        }
        "admin_para_all" | "admin_para_all_menu" => {
            effects::admin_para(world, client_id, object_id, &args, true, true)
        }
        "admin_unpara_all" | "admin_unpara_all_menu" => {
            effects::admin_para(world, client_id, object_id, &args, false, true)
        }
        "admin_bighead" => effects::admin_bighead(world, client_id, object_id, true),
        "admin_shrinkhead" => effects::admin_bighead(world, client_id, object_id, false),
        "admin_playmovie" => effects::admin_playmovie(world, client_id, &args),
        "admin_event_trigger" => effects::admin_event_trigger(world, client_id, object_id, &args),
        "admin_set_displayeffect" | "admin_set_displayeffect_menu" => {
            effects::admin_set_displayeffect(world, client_id, object_id, &args)
        }
        // `AdminEffects`' invisibility family: `//invis` *sets* hidden,
        // `//vis` *sets* visible (never toggles), `//setinvis` toggles the
        // targeted player, and the gm_menu "Invis" button toggles + re-serves
        // the panel.
        "admin_invis" | "admin_invisible" => {
            flags::admin_set_hidden(world, client_id, object_id, true)
        }
        "admin_vis" | "admin_visible" => {
            flags::admin_set_hidden(world, client_id, object_id, false)
        }
        "admin_setinvis" => flags::admin_setinvis(world, client_id, object_id),
        // `AdminEditChar` pet/summon subcommands + `//rec` (category-4 sweep).
        "admin_rec" => editchar::admin_rec(world, client_id, object_id, &args),
        "admin_unsummon" => editchar::admin_unsummon(world, client_id, object_id),
        "admin_summon_info" => editchar::admin_summon_info(world, client_id, object_id),
        "admin_summon_setlvl" => editchar::admin_summon_setlvl(world, client_id, object_id, &args),
        "admin_show_pet_inv" => editchar::admin_show_pet_inv(world, client_id, object_id, &args),
        // Strict = plain dualbox grouping: Java narrows by IP+tracert pack,
        // and no per-client tracert is recorded in this port.
        "admin_strict_find_dualbox" => moderation::admin_find_dualbox(world, client_id, &args),
        // `AdminPunishment` console + `AdminMenu` ban wrappers + `//force_peti`.
        "admin_punishment" => moderation::admin_punishment(world, client_id, object_id, &args),
        "admin_punishment_add" => {
            moderation::admin_punishment_add(world, client_id, object_id, &args)
        }
        "admin_punishment_remove" => moderation::admin_punishment_remove(world, client_id, &args),
        "admin_ban_menu" => moderation::admin_ban_menu(world, client_id, object_id, &args),
        // Category-4 sweep: spawns / clans / panels.
        // Server control (`AdminShutdown` / `AdminLogin`).
        "admin_server_shutdown" => {
            world_cmds::admin_server_shutdown(world, client_id, &args, false)
        }
        "admin_server_restart" => world_cmds::admin_server_shutdown(world, client_id, &args, true),
        "admin_server_abort" => world_cmds::admin_server_abort(world, client_id),
        "admin_server_gm_only"
        | "admin_server_all"
        | "admin_server_max_player"
        | "admin_server_list_age"
        | "admin_server_list_type" => {
            world_cmds::admin_server_status(world, client_id, command, &args)
        }
        // Olympiad manual controls (`AdminAdmin`).
        "admin_endolympiad" => {
            super::olympiad::handle_olympiad_end(world);
            send_message(world, client_id, "Heroes formed.");
        }
        "admin_saveolymp" => {
            super::olympiad::save_all(world);
            send_message(world, client_id, "Olympiad system saved.");
        }
        // Java `settruehero` is NOT a synonym for `sethero`: it toggles
        // `Player._trueHero`, a separate flag with its own `100 : 0` byte in
        // both CharInfo and UserInfo (`isTrueHero()`), and touches neither the
        // hero skill tree nor the `isHero()` glow byte.
        "admin_settruehero" => hero::admin_settruehero(world, client_id, object_id),
        // Quest admin. Two *different* Java handlers, aliased here until the
        // shift-click NPC window's `Quests` button exposed the difference:
        // `AdminShowQuests`' `//charquestmenu` edits a **player's** quest
        // states, `AdminQuest`'s `//show_quests` lists the scripts registered
        // on the current **target**.
        "admin_charquestmenu" => editchar::admin_charquestmenu(world, client_id, object_id, &args),
        "admin_show_quests" => gm_util::admin_show_quests(world, client_id, object_id),
        "admin_setcharquest" => editchar::admin_setcharquest(world, client_id, &args),
        // Tail polish: tradeoff / cond overrides / quest info / clan halls /
        // reload / gm-buff switch.
        "admin_tradeoff" => gm_util::admin_tradeoff(world, client_id, object_id, &args),
        "admin_exceptions" => gm_util::admin_exceptions(world, client_id, object_id),
        "admin_set_exception" => gm_util::admin_set_exception(world, client_id, object_id, &args),
        "admin_quest_info" => gm_util::admin_quest_info(world, client_id, &args),
        "admin_clanhall" => gm_util::admin_clanhall(world, client_id, object_id, &args),
        "admin_reload" => gm_util::admin_reload(world, client_id, &args),
        "admin_switch_gm_buffs" => gm_util::admin_switch_gm_buffs(world, client_id),

        "admin_unspawnall" => spawn::admin_unspawnall(world, client_id),
        "admin_respawnall" => spawn::admin_respawnall(world, client_id),
        "admin_spawn_reload" => spawn::admin_spawn_reload(world, client_id),
        "admin_clan_changeleader" => {
            pledge::admin_clan_changeleader(world, client_id, object_id, &args)
        }
        "admin_add_clan_skill" => pledge::admin_add_clan_skill(world, client_id, object_id, &args),
        "admin_play_sounds" => gm_util::admin_play_sounds(world, client_id, &args),
        "admin_effect_menu" => gm_util::admin_effect_menu(world, client_id),
        "admin_event_menu" | "admin_event_start_menu" | "admin_event_stop_menu" => {
            gm_util::admin_event_menu(world, client_id)
        }
        "admin_bbs" => gm_util::admin_bbs(world, client_id, object_id),
        "admin_viewblockedeffects" => {
            gm_util::admin_viewblockedeffects(world, client_id, object_id)
        }

        "admin_unban_menu" => moderation::admin_unban_menu(world, client_id, &args),
        "admin_force_peti" => {
            let text = args.join(" ");
            match guard::player_target(world, object_id) {
                _ if text.is_empty() => send_message(world, client_id, "Usage: //force_peti text"),
                Some(target) => {
                    if !super::petition::force_petition(world, object_id, target, &text) {
                        send_message(world, client_id, "That player already has a petition.");
                    }
                }
                None => send_sm(world, client_id, sm_ids::THAT_IS_AN_INCORRECT_TARGET),
            }
        }

        "admin_invis_menu" => flags::admin_invis_menu(world, client_id, object_id),
        _ => return false,
    }
    show_effects_main_page(world, client_id, command);
    true
}

/// The `AdminEffects` commands carrying the `_menu` suffix — the ones the
/// Effects panel's buttons fire. Java ends its handler with
/// `if (command.contains("menu")) showMainPage(activeChar, command);`, but that
/// tail belongs to `AdminEffects` alone; the port has a single dispatcher for
/// every handler, so the set is spelled out instead of matched on the
/// substring (`//invis_menu` re-serves `gm_menu.htm` from its own branch and
/// must not be overwritten, and other handlers have `_menu` commands of their
/// own).
const EFFECTS_PANEL_MENU_COMMANDS: &[&str] = &[
    "admin_earthquake_menu",
    "admin_para_menu",
    "admin_unpara_menu",
    "admin_para_all_menu",
    "admin_unpara_all_menu",
    "admin_effect_menu",
    "admin_social_menu",
    "admin_atmosphere_menu",
    "admin_set_displayeffect_menu",
];

/// Java `AdminEffects.showMainPage` — after an Effects-panel `*_menu` command
/// the panel is served again, so the GM keeps clicking instead of being left
/// staring at the world. Social gets its own sub-page (`social.htm`);
/// everything else returns to `effects_menu.htm`.
///
/// Without this the panel vanished on every button press, and "Social" never
/// opened its page at all.
fn show_effects_main_page(world: &mut World, client_id: u32, command: &str) {
    if !EFFECTS_PANEL_MENU_COMMANDS.contains(&command) {
        return;
    }
    let file = if command.contains("social") {
        "social.htm"
    } else {
        "effects_menu.htm"
    };
    menu::show_admin_html(world, client_id, file);
}

/// EditChar target = the current target if it's a player, else the GM.
pub(super) fn target_player(world: &World, object_id: i32) -> i32 {
    guard::player_target(world, object_id).unwrap_or(object_id)
}

/// Java `target.getName()` for the GM-audit record, with Java's `"no-target"`
/// sentinel when nothing is selected. Players answer with their character name;
/// NPCs resolve through the template, since the object itself only carries the
/// template id.
fn target_display_name(world: &World, gm_object_id: i32) -> String {
    let Some(target_id) = guard::target(world, gm_object_id) else {
        return "no-target".to_string();
    };
    if let Some(p) = world.objects.get_component::<Player>(&target_id) {
        return p.name.clone();
    }
    if let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&target_id)
        && let Some(template) = world.data.npc_data.get(npc.npc_id)
    {
        return template.name.clone();
    }
    format!("object:{target_id}")
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

/// Java `World.forEachVisibleObjectInRange(center, …, radius, …)`, narrowed to
/// the creature kinds the admin radius commands touch. Returns the object ids of
/// visible players and/or NPCs within `radius` (2D) of `center_oid`, excluding
/// `center_oid` itself. Visibility follows the knownlist (adjacent regions),
/// matching Java's iteration over the object's visible set.
pub(super) fn creatures_in_range(
    world: &World,
    center_oid: i32,
    radius: i32,
    include_players: bool,
    include_npcs: bool,
) -> Vec<i32> {
    use crate::model::components::{Position, RegionCell};
    let Some(&center) = world.objects.get_component::<Position>(&center_oid) else {
        return Vec::new();
    };
    let r = radius as f64;
    let mut out = Vec::new();
    if include_players {
        for cs in world.clients.values() {
            if let ClientSession::InGame(s) = cs {
                let oid = s.player_object_id();
                if oid == center_oid {
                    continue;
                }
                if world
                    .objects
                    .get_component::<Position>(&oid)
                    .is_some_and(|p| p.distance_2d(&center) <= r)
                {
                    out.push(oid);
                }
            }
        }
    }
    if include_npcs
        && let Some(region) = world
            .objects
            .get_component::<RegionCell>(&center_oid)
            .map(|c| c.0)
    {
        for oid in world.npcs_visible_from(region) {
            if world
                .objects
                .get_component::<Position>(&oid)
                .is_some_and(|p| p.distance_2d(&center) <= r)
            {
                out.push(oid);
            }
        }
    }
    out
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
