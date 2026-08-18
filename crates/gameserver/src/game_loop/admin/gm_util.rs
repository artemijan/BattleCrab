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

use crate::game_loop::guard;
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::{disconnect_player, nth_arg};
use crate::model::Player;
use crate::model::components::{AdminFlags, PartyRef};
use crate::model::npc::Npc;
use crate::network::server_packets::{self, sm_ids};
use crate::session::ClientSession;
use crate::world::World;

use super::{find_online_player, send_message, send_sm};
use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::helpers::region_cell_of;

/// `AdminAdmin`'s `//gmliston` / `//gmlistoff` — register/unregister from the
/// GM list. Both are **message-only in Java too**: neither calls `showGm` or
/// `hideGm` (which have no callers at all), so the `hidden` flag stays whatever
/// enter-world set it to. The list itself is served by the `/gmlist` packet —
/// see [`super::handle_request_gm_list`].
pub(super) fn admin_gmlist(world: &mut World, client_id: u32, on: bool) {
    send_message(
        world,
        client_id,
        if on {
            "Registered into GM list."
        } else {
            "Removed from GM list."
        },
    );
    super::menu::show_admin_html(world, client_id, "gm_menu.htm");
}

/// `AdminAdmin`'s `//diet on|off` — toggle weight-overload immunity
/// (`AdminFlags.diet`). Honoured by `weight::penalty_level`, which returns 0 in
/// diet mode however much the GM carries, and by `weight::is_overloaded`.
pub(super) fn admin_diet(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let mut flags = world
        .objects
        .get_component::<AdminFlags>(&object_id)
        .copied()
        .unwrap_or_default();
    match args.first().copied() {
        Some("on") => flags.diet = true,
        Some("off") => flags.diet = false,
        _ => {
            send_message(world, client_id, "Usage: //diet on|off");
            return;
        }
    }
    world.objects.add_components(&object_id, flags);
    send_message(
        world,
        client_id,
        if flags.diet {
            "Diet mode on."
        } else {
            "Diet mode off."
        },
    );
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
            let name = player_name_or_empty(world, object_id);
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
            if world
                .objects
                .get_component::<Player>(&s.player_object_id())
                .is_some_and(|p| p.is_gm(&world.data))
            {
                gms += 1;
            }
        }
    }
    send_message(world, client_id, "=== Online ===");
    send_message(
        world,
        client_id,
        &format!("Players online: {total} (GMs: {gms})"),
    );
}

/// `AdminTargetSay`'s `//targetsay <text>` — make the current target say `text`.
pub(super) fn admin_targetsay(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(target) = guard::target(world, object_id) else {
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
        world
            .data
            .npc_data
            .get(npc.npc_id)
            .map(|t| t.name.clone())
            .unwrap_or_default()
    } else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let say =
        server_packets::creature_say(target, crate::enums::ChatType::General, &name, &text, None);
    super::helpers::broadcast_including_self(world, target, &say);
}

/// `AdminMessages`'s `//msg <id>` — send the raw `SystemMessage(id)` to the GM.
pub(super) fn admin_msg(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(id) = nth_arg::<i16>(args, 0) else {
        send_message(world, client_id, "Command format: //msg <SYSTEM_MSG_ID>");
        return;
    };
    send_sm(world, client_id, id);
}

/// `AdminAnnouncements`'s `//announce_crit` / `//announce_screen <message>` —
/// broadcast to all players. `//announce_screen` puts the text on everyone's
/// screen as an `ExShowScreenMessage` (top-centre, 10 s); `//announce_crit` /
/// `//announces` fall back to the ordinary system-message text line.
pub(super) fn admin_announce_variant(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
    screen: bool,
) {
    if args.is_empty() {
        send_message(world, client_id, "Usage: //announce_screen <message>");
        return;
    }
    let text = args.join(" ");
    if screen {
        // `ExShowScreenMessage(text, TOP_CENTER, 10000)`.
        broadcast_packet(
            world,
            server_packets::ex_show_screen_message(&text, 2, 10_000),
        );
    } else {
        // Java appends the name on the `//announce`/`//announce_crit` branch
        // only — `announce_screen` returns before reaching it.
        let text = with_announcer_name(world, object_id, text);
        broadcast_text(world, &text);
    }
}

/// `Config.GM_ANNOUNCER_NAME` (**False** here): Java's
/// `announce = announce + " [" + activeChar.getName() + "]"`.
///
/// Off on this dist, so today it returns the text unchanged — the point of
/// wiring it is that an operator who turns it on gets the attribution instead
/// of nothing.
pub(super) fn with_announcer_name(world: &World, object_id: i32, text: String) -> String {
    if !world.cfg.general.gm_announcer_name {
        return text;
    }
    match world.objects.get_component::<Player>(&object_id) {
        Some(p) => format!("{text} [{}]", p.name),
        None => text,
    }
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
    let Some(region) = region_cell_of(world, object_id) else {
        return;
    };
    let ids = world.doors_visible_from(region);
    send_message(
        world,
        client_id,
        &format!("=== Doors in view ({}) ===", ids.len()),
    );
    for oid in ids {
        if let Some(d) = world
            .objects
            .get_component::<crate::model::door::Door>(&oid)
        {
            send_message(
                world,
                client_id,
                &format!("  door {} (obj {oid})", d.door_id),
            );
        }
    }
}

/// Port of `AdminDebug` — `//debug [packets|doors|geodata|movement] [on|off]
/// [menu]`. Bare `//debug` opens the four-toggle Debug panel (`debug.htm`,
/// its `%…_status%` tokens filled from live state — Java `showMenu`; the old
/// port routed this to a chat-text stat dump, so the panel was unreachable).
/// The packets toggle drives `World::debug_packets` (Java flips
/// `Config.DEBUG_*_PACKETS` for console packet logging); the
/// doors/geodata/movement visualizers (Java `setDoorDebugging`/
/// `setGeodataDebugging`/`setMovementDebugging` per-player draw tasks) run
/// through `admin::debug_draw` over `ExServerPrimitive`, landed since.
pub(super) fn admin_debug(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(&sub) = args.first() else {
        return show_debug_menu(world, client_id, object_id);
    };
    // The htm buttons send e.g. `admin_debug packets on menu` — the trailing
    // literal re-renders the panel (Java `command.contains("menu")`).
    let menu = args.contains(&"menu");
    match sub {
        "packet" | "packets" => {
            let on = match args.get(1).copied() {
                Some("on") => true,
                Some("off") => false,
                // Bare `//debug packets` toggles (Java's no-token branch).
                _ => !world.debug_packets,
            };
            world.debug_packets = on;
            send_message(
                world,
                client_id,
                &format!(
                    "Packet debugging on console is {}.",
                    if on { "enabled" } else { "disabled" }
                ),
            );
            if menu {
                show_debug_menu(world, client_id, object_id);
            }
        }
        "door" | "doors" | "geo" | "geodata" | "move" | "movement" | "path" | "pathfind" => {
            let kind = match sub {
                "door" | "doors" => "doors",
                "geo" | "geodata" => "geodata",
                _ => "movement",
            };
            let (doors, geo, movement) = super::debug_draw::flags(world, object_id);
            let current = match kind {
                "doors" => doors,
                "geodata" => geo,
                _ => movement,
            };
            let on = match args.get(1).copied() {
                Some("on") => true,
                Some("off") => false,
                _ => !current,
            };
            super::debug_draw::set_debug(world, client_id, object_id, kind, on);
            if menu {
                show_debug_menu(world, client_id, object_id);
            }
        }
        _ => send_message(world, client_id, "Usage: //debug <parameter> <value>"),
    }
}

/// Java `AdminDebug.showMenu` — `debug.htm` with each `%token%` pair swapped
/// for the live state (`Disable`+`… off` when active, `Enable`+`… on` when
/// not).
fn show_debug_menu(world: &World, client_id: u32, object_id: i32) {
    let toggle = |on: bool, noun: &str| {
        if on {
            ("Disable", format!("{noun} off"))
        } else {
            ("Enable", format!("{noun} on"))
        }
    };
    let (doors, geo, movement) = super::debug_draw::flags(world, object_id);
    let (packets_status, packets_cmd) = toggle(world.debug_packets, "packets");
    let (doors_status, doors_cmd) = toggle(doors, "doors");
    let (geo_status, geo_cmd) = toggle(geo, "geodata");
    let (move_status, move_cmd) = toggle(movement, "movement");
    super::menu::show_admin_html_replace(
        world,
        client_id,
        "debug.htm",
        &[
            ("packets_status", packets_status.to_string()),
            ("packets", packets_cmd),
            ("doors_status", doors_status.to_string()),
            ("doors", doors_cmd),
            ("geodata_status", geo_status.to_string()),
            ("geodata", geo_cmd),
            ("movement_status", move_status.to_string()),
            ("movement", move_cmd),
        ],
    );
}

/// `AdminTest`'s `//stats` — server-wide counts.
pub(super) fn admin_stats(world: &mut World, client_id: u32) {
    let online = world
        .clients
        .values()
        .filter(|c| matches!(c, ClientSession::InGame(_)))
        .count();
    let npcs: usize = world.npc_regions.values().map(|v| v.len()).sum();
    send_message(world, client_id, "=== Stats ===");
    send_message(
        world,
        client_id,
        &format!(
            "Online: {online}  NPCs: {npcs}  Parties: {}  Tick: {}",
            world.parties.len(),
            world.tick
        ),
    );
}

/// `AdminKick`'s `//kick_non_gm` — disconnect every online non-GM player.
pub(super) fn admin_kick_non_gm(world: &mut World, client_id: u32) {
    let targets: Vec<i32> = world
        .in_game_player_oids()
        .filter(|oid| {
            !world
                .objects
                .get_component::<Player>(oid)
                .is_some_and(|p| p.is_gm(&world.data))
        })
        .collect();
    let n = targets.len();
    for oid in targets {
        disconnect_player(world, oid);
    }
    send_message(world, client_id, &format!("Kicked {n} non-GM player(s)."));
}

/// The `AdminMenu` character-panel convention: the button carries the name of
/// the character already chosen on the previous page, so the name argument wins
/// and the GM's own selection is only a fallback for a blank QuickBox.
fn resolve_named_or_target(world: &World, object_id: i32, args: &[&str]) -> Option<i32> {
    match args.first() {
        Some(name) => find_online_player(world, name),
        None => guard::player_target(world, object_id),
    }
}

/// `AdminMenu`'s `//recall_party_menu <name>` — recall the named character's
/// whole party to the GM. Like the "Go To" button, the Character panel passes
/// the character chosen on the previous page (`$qbox`), and Java resolves it
/// with `World.getPlayer(command.substring(24))`; only a blank name falls back
/// to the GM's current target. A character with no party is simply recalled
/// alone (Java sends "Player is not in party." and still teleports them).
pub(super) fn admin_recall_party(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(target) = resolve_named_or_target(world, object_id, args) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let Some(PartyRef(pid)) = world.objects.get_component::<PartyRef>(&target).copied() else {
        send_message(world, client_id, "Player is not in party.");
        recall_all(world, object_id, &[target]);
        return;
    };
    let members = world
        .parties
        .get(&pid)
        .map(|p| p.members.clone())
        .unwrap_or_default();
    recall_all(world, object_id, &members);
    send_message(
        world,
        client_id,
        &format!("Recalled {} party member(s).", members.len()),
    );
}

/// `AdminMenu`'s `//recall_clan_menu <name>` — recall the named character's
/// online clan members to the GM. Name resolution matches
/// [`admin_recall_party`] (Java `command.substring(23)`), and a clanless
/// character is recalled alone.
pub(super) fn admin_recall_clan(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(target) = resolve_named_or_target(world, object_id, args) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let Some(clan_id) = guard::clan_of(world, target) else {
        send_message(world, client_id, "Player is not in a clan.");
        recall_all(world, object_id, &[target]);
        return;
    };
    let members: Vec<i32> = world
        .in_game_player_oids()
        .filter(|oid| {
            world
                .objects
                .get_component::<Player>(oid)
                .map(|p| p.clan_id)
                == Some(clan_id)
        })
        .collect();
    recall_all(world, object_id, &members);
    send_message(
        world,
        client_id,
        &format!("Recalled {} clan member(s).", members.len()),
    );
}

/// Teleport each of `members` to the GM's position.
fn recall_all(world: &mut World, gm_oid: i32, members: &[i32]) {
    let Some(pos) = maybe_position(world, gm_oid) else {
        return;
    };
    for &oid in members {
        super::death::teleport_player(world, oid, pos.x, pos.y, pos.z);
    }
}

/// Broadcast a plain text line to every online player as a `$s1` system message.
fn broadcast_text(world: &World, text: &str) {
    broadcast_packet(
        world,
        server_packets::system_message_with(
            sm_ids::S1_TEXT,
            &[server_packets::SmParam::Text(text.to_string())],
        ),
    );
}

/// Send one prebuilt packet to every online player (Java
/// `Broadcast.toAllOnlinePlayers`).
fn broadcast_packet(world: &World, packet: Vec<u8>) {
    world.broadcast_to_all_online(&packet);
}

// ---------------------------------------------------------------------------
// Panel commands (category-4 sweep)
// ---------------------------------------------------------------------------

/// `AdminEffects`' `//play_sounds [page]` — the jukebox pages
/// (`songs/songs.htm`, `songs/songs2.htm`, …); each button fires the already-
/// wired `//play_sound <name>`.
pub(super) fn admin_play_sounds(world: &World, client_id: u32, args: &[&str]) {
    let page = args.first().copied().unwrap_or("");
    let file = if page.is_empty() {
        "songs/songs.htm".to_string()
    } else {
        format!("songs/songs{page}.htm")
    };
    super::menu::show_admin_html(world, client_id, &file);
}

/// `//effect_menu` — the effects panel (same page as `//admin3`).
pub(super) fn admin_effect_menu(world: &World, client_id: u32) {
    super::menu::show_admin_html(world, client_id, "effects_menu.htm");
}

/// `//event_menu` (and the start/stop menu aliases) — `gm_events.htm` with
/// `%LIST%` filled from the registered event engines (G28), each with
/// Start/Stop buttons routing to the wired `//event_start`/`//event_stop`.
pub(super) fn admin_event_menu(world: &World, client_id: u32) {
    let mut list = String::new();
    for name in crate::game_loop::events::EVENT_NAMES {
        list.push_str(&format!(
            "<tr><td>{name}</td>\
             <td><button value=\"Start\" action=\"bypass -h admin_event_start {name}\" \
             width=65 height=21 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td>\
             <td><button value=\"Stop\" action=\"bypass -h admin_event_stop {name}\" \
             width=65 height=21 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td></tr>"
        ));
    }
    super::menu::show_admin_html_replace(world, client_id, "gm_events.htm", &[("LIST", list)]);
}

/// `//bbs` — open the community board home for the GM (Java routes it into
/// the CB parse loop).
pub(super) fn admin_bbs(world: &mut World, client_id: u32, object_id: i32) {
    crate::game_loop::community_board::open_home_for_admin(world, client_id, object_id);
}

/// `AdminBuffs`' `//viewblockedeffects` — list the abnormal slots currently
/// blocked on the target by live `BlockAbnormalSlot` effects.
pub(super) fn admin_viewblockedeffects(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = guard::target(world, object_id).or(Some(object_id)) else {
        return;
    };
    let mut blocked: Vec<String> = Vec::new();
    if let Some(buffs) = world
        .objects
        .get_component::<crate::model::components::Buffs>(&target)
    {
        for b in buffs.0.iter() {
            if let Some(skill) = world.data.skill_data.get(b.skill_id, b.skill_level) {
                for eff in &skill.effects {
                    if let crate::model::skill::SkillEffect::BlockAbnormalSlot { slots } = eff {
                        blocked.extend(slots.iter().cloned());
                    }
                }
            }
        }
    }
    let text = if blocked.is_empty() {
        "No abnormal slots are blocked on the target.".to_string()
    } else {
        format!("Blocked abnormal slots: {}.", blocked.join(", "))
    };
    send_message(world, client_id, &text);
}

// ---------------------------------------------------------------------------
// Tail polish: tradeoff, cond overrides, quest_info, clanhall, reload,
// switch_gm_buffs
// ---------------------------------------------------------------------------

/// Java `PlayerCondOverride.ITEM_CONDITIONS` — ordinal 1. Read by
/// `ItemTemplate.checkCondition` (unported — see `GMItemRestriction`) and by
/// `TradeStart`'s available-item list.
pub(crate) const ITEM_CONDITIONS_ORDINAL: u8 = 1;

/// Java `PlayerCondOverride.SKILL_CONDITIONS` — ordinal 2, read by
/// `Skill.checkCondition` and the `restoreSkills` skill check.
pub(crate) const SKILL_CONDITIONS_ORDINAL: u8 = 2;

/// Java `PlayerCondOverride.DROP_ALL_ITEMS` — ordinal 15, read by
/// `RequestDropItem`. Deliberately *not* `ITEM_CONDITIONS`: Java uses a
/// different override for dropping than for the trade window, on the same
/// config key.
pub(crate) const DROP_ALL_ITEMS_ORDINAL: u8 = 15;

/// Java `PlayerCondOverride.getAllExceptionsMask()` — every ordinal set.
///
/// Load-bearing at login: `Player.restore` gives a **GM** this mask as the
/// *default* value of the `cond_override` character variable, so a GM who has
/// never touched `//set_exception` still overrides everything.
pub(crate) fn all_exceptions_mask() -> u64 {
    COND_OVERRIDES.iter().map(|&(o, _)| 1u64 << o).sum()
}

/// Java `PlayerCondOverride` ordinals + panel descriptions (bit = ordinal).
pub(crate) const COND_OVERRIDES: &[(u8, &str)] = &[
    (0, "Overrides maximum states conditions"),
    (1, "Overrides item usage conditions"),
    (2, "Overrides skill usage conditions"),
    (3, "Overrides zone conditions"),
    (4, "Overrides castle conditions"),
    (5, "Overrides fortress conditions"),
    (6, "Overrides clan hall conditions"),
    (7, "Overrides floods conditions"),
    (8, "Overrides chat conditions"),
    (9, "Overrides instance conditions"),
    (10, "Overrides quest conditions"),
    (11, "Overrides death penalty conditions"),
    (12, "Overrides item destroy conditions"),
    (13, "Overrides the conditions to see hidden players"),
    (14, "Overrides target conditions"),
    (15, "Overrides item drop conditions"),
];

/// `//tradeoff on|off|status` (Java `AdminAdmin`).
pub(super) fn admin_tradeoff(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    match args.first().copied() {
        Some("on") | Some("off") => {
            let on = args[0] == "on";
            if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
                p.trade_refusal = on;
            }
            send_message(
                world,
                client_id,
                if on {
                    "Trade refusal enabled"
                } else {
                    "Trade refusal disabled"
                },
            );
        }
        _ => {
            let on = world
                .objects
                .get_component::<Player>(&object_id)
                .is_some_and(|p| p.trade_refusal);
            send_message(
                world,
                client_id,
                &format!(
                    "Trade refusal now {}",
                    if on { "enabled" } else { "disabled" }
                ),
            );
        }
    }
}

/// `//exceptions` — the `cond_override.htm` panel with per-override toggles
/// (Java `AdminPcCondOverride`).
pub(super) fn admin_exceptions(world: &mut World, client_id: u32, object_id: i32) {
    let mask = world
        .objects
        .get_component::<Player>(&object_id)
        .map_or(0, |p| p.cond_overrides);
    let mut rows = String::new();
    for &(ord, descr) in COND_OVERRIDES {
        let on = mask & (1u64 << ord) != 0;
        rows.push_str(&format!(
            "<tr><td fixwidth=\"180\">{descr}:</td><td><a action=\"bypass -h \
             admin_set_exception {ord}\">{}</a></td></tr>",
            if on { "Disable" } else { "Enable" }
        ));
    }
    super::menu::show_admin_html_replace(
        world,
        client_id,
        "cond_override.htm",
        &[("cond_table", rows)],
    );
}

/// `//set_exception <ordinal|enable_all|disable_all>`.
pub(super) fn admin_set_exception(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let Some(&token) = args.first() else {
        send_message(
            world,
            client_id,
            "Usage: //set_exception <id|enable_all|disable_all>",
        );
        return;
    };
    let all_mask: u64 = all_exceptions_mask();
    match token {
        "enable_all" => {
            if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
                p.cond_overrides = all_mask;
            }
            send_message(
                world,
                client_id,
                "All condition exceptions have been enabled.",
            );
        }
        "disable_all" => {
            if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
                p.cond_overrides = 0;
            }
            send_message(
                world,
                client_id,
                "All condition exceptions have been disabled.",
            );
        }
        t => {
            let Some(&(ord, descr)) = t
                .parse::<u8>()
                .ok()
                .and_then(|n| COND_OVERRIDES.iter().find(|&&(o, _)| o == n))
            else {
                send_message(
                    world,
                    client_id,
                    "Usage: //set_exception <id|enable_all|disable_all>",
                );
                return;
            };
            let now = {
                let Some(p) = world.objects.get_component_mut::<Player>(&object_id) else {
                    return;
                };
                p.cond_overrides ^= 1u64 << ord;
                p.cond_overrides & (1u64 << ord) != 0
            };
            send_message(
                world,
                client_id,
                &format!(
                    "You've {} {descr}",
                    if now { "enabled" } else { "disabled" }
                ),
            );
        }
    }
    admin_exceptions(world, client_id, object_id);
}

/// `AdminQuest`'s `//show_quests` — the quest scripts registered on the
/// **current target**, each a link into `//quest_info`. This is the `Quests`
/// button on the shift-click admin `npcinfo.htm` (whose `%tmplid%` argument
/// Java ignores: it reads `activeChar.getTarget()`).
///
/// Java walks every `EventType`'s listeners on the target creature and
/// dedups by quest name through a `TreeSet` (hence alphabetical); the port
/// asks its compiled-in registry which scripts name this npc id in any of
/// their hook lists — the same question, one indirection earlier.
///
/// Not to be confused with `AdminShowQuests`' `//charquestmenu`, the *player*
/// quest-state editor — the two were aliased here, so this button used to open
/// the player menu and answer `INVALID_TARGET` on an NPC.
pub(super) fn admin_show_quests(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = guard::target(world, object_id) else {
        send_message(world, client_id, "Get a target first.");
        return;
    };
    // Java's gate is `isCreature()` — an NPC or a player, not a door/item.
    let npc_id = npc_id_of(world, target);
    if npc_id.is_none() && !world.objects.has_component::<Player>(&target) {
        send_message(world, client_id, "Invalid Target.");
        return;
    }
    // A player target has no quest *listeners* in this port (quest state lives
    // on the player, hooks live on NPCs), so the list is empty — as in Java,
    // where a player carries listeners only in exotic cases.
    let mut rows = String::new();
    if let Some(npc_id) = npc_id {
        for name in world.quests.clone().scripts_for_npc(npc_id) {
            rows.push_str(&format!(
                "<tr><td colspan=\"4\"><font color=\"LEVEL\">\
                 <a action=\"bypass -h admin_quest_info {name}\">{name}</a></font></td></tr>"
            ));
        }
    }
    super::menu::show_admin_html_replace(
        world,
        client_id,
        "npc-quests.htm",
        &[
            ("quests", rows),
            ("objid", target.to_string()),
            ("questName", String::new()),
        ],
    );
}

/// `//quest_info [name]` — the registered quest scripts (bare: list; with a
/// name: its registration detail from the compiled-in registry).
pub(super) fn admin_quest_info(world: &mut World, client_id: u32, args: &[&str]) {
    let quests = world.quests.clone();
    if let Some(&name) = args.first() {
        let Some(script) = quests.by_name(name) else {
            send_message(
                world,
                client_id,
                &format!("Couldn't find quest or script: {name}"),
            );
            return;
        };
        let html = format!(
            "<html><title>{}</title><body>ID: {}<br>Start NPCs: {:?}<br>Talk NPCs: {:?}<br>\
             First-talk NPCs: {:?}<br></body></html>",
            script.name(),
            script.id(),
            script.start_npcs(),
            script.talk_npcs(),
            script.first_talk_npcs(),
        );
        super::menu::send_admin_html_content(world, client_id, &html);
        return;
    }
    let mut rows = String::new();
    for name in quests.names() {
        rows.push_str(&format!(
            "<tr><td><a action=\"bypass -h admin_quest_info {name}\">{name}</a></td></tr>"
        ));
    }
    let html = format!(
        "<html><title>Quest scripts</title><body><table width=270>{rows}</table></body></html>"
    );
    super::menu::send_admin_html_content(world, client_id, &html);
}

/// `//clanhall [id [give|take]]` — list halls with owners; `give` hands the
/// hall to the targeted player's clan, `take` frees it (the panel half of
/// Java `AdminClanHall`; doors/teleport have their own commands).
pub(super) fn admin_clanhall(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(hall_id) = nth_arg::<i32>(args, 0) else {
        let mut halls: Vec<_> = world.clan_halls.values().collect();
        halls.sort_by_key(|h| h.id);
        let rows: String = halls
            .iter()
            .map(|h| {
                let owner = if h.owner_id == 0 {
                    "free".to_string()
                } else {
                    world
                        .clans
                        .get(&h.owner_id)
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| h.owner_id.to_string())
                };
                format!(
                    "<tr><td>{}</td><td>{}</td><td>{owner}</td></tr>",
                    h.id, h.name
                )
            })
            .collect();
        let html = format!(
            "<html><title>Clan Halls</title><body><table width=270>\
             <tr><td>Id</td><td>Name</td><td>Owner</td></tr>{rows}</table>\
             <br>//clanhall &lt;id&gt; give|take</body></html>"
        );
        super::menu::send_admin_html_content(world, client_id, &html);
        return;
    };
    if !world.clan_halls.contains_key(&hall_id) {
        send_message(world, client_id, "No such clan hall.");
        return;
    }
    match args.get(1).copied() {
        Some("give") => {
            let clan_id =
                guard::target(world, object_id).and_then(|oid| guard::clan_of(world, oid));
            let Some(clan_id) = clan_id else {
                send_message(world, client_id, "Target a member of the receiving clan.");
                return;
            };
            crate::game_loop::clans::hall_auction::set_hall_owner(world, hall_id, clan_id);
            send_message(
                world,
                client_id,
                &format!("Clan hall {hall_id} given to clan {clan_id}."),
            );
        }
        Some("take") => {
            crate::game_loop::clans::hall_auction::revoke_hall(world, hall_id);
            send_message(
                world,
                client_id,
                &format!("Clan hall {hall_id} is now free."),
            );
        }
        _ => send_message(world, client_id, "Usage: //clanhall <id> give|take"),
    }
}

/// `//reload <section>` — re-read a data section from disk (Java
/// `AdminReload`, narrowed to the loaders this port has; scripts are
/// compiled in and `htm` has no cache to flush).
pub(super) fn admin_reload(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(&what) = args.first() else {
        send_message(
            world,
            client_id,
            "Usage: //reload <config|access|npc|skill|item|multisell|buylist|teleport|fishing>",
        );
        return;
    };
    let root = world.data.root.clone();
    let msg = match what {
        "config" => {
            world.cfg = crate::config::Config::load_from(&root).combat();
            "Configs reloaded."
        }
        "access" => {
            world.data.admin = crate::data::AdminData::load_from(&root);
            "Access levels reloaded."
        }
        "npc" => {
            world.data.npc_data = crate::data::NpcData::load_from(&root);
            "NPC templates reloaded (respawn with //respawnall to apply)."
        }
        "skill" => {
            world.data.skill_data = crate::data::SkillData::load_from(&root);
            "Skill data reloaded."
        }
        "item" => {
            world.data.item_data = crate::data::ItemData::load_from(&root);
            "Item templates reloaded."
        }
        "multisell" => {
            world.data.multisells =
                crate::data::MultisellData::load_from(&root, &world.data.item_data);
            "Multisell lists reloaded."
        }
        "buylist" => {
            // `//reload buylist` re-applies the *live* `MaxEquipableItemGrade`,
            // so editing Character.ini and reloading is enough to change the
            // catalogue without a restart.
            let max_grade = world.cfg.character.max_equipable_item_grade;
            world.data.buy_lists =
                crate::data::BuyListData::load_from(&root, &world.data.item_data, max_grade);
            "Buylists reloaded."
        }
        "teleport" => {
            world.data.teleporters = crate::data::TeleporterData::load_from(&root);
            "Teleporter lists reloaded."
        }
        "fishing" => {
            world.data.fishing_data = crate::data::FishingData::load_from(&root);
            "Fishing data reloaded."
        }
        "quest" | "script" => "Scripts are compiled in — rebuild and restart to change them.",
        "htm" | "html" => "Admin/quest html is read from disk per request — no cache to flush.",
        _ => {
            send_message(
                world,
                client_id,
                &format!("Unknown reload section '{what}'."),
            );
            return;
        }
    };
    send_message(world, client_id, msg);
}

/// `//switch_gm_buffs` — Java swaps the GM special-skill tree for the aura
/// variant only when `GmGiveSpecialSkills != GmGiveSpecialAuraSkills`; both
/// default False on this dist, so Java answers exactly this.
pub(super) fn admin_switch_gm_buffs(world: &World, client_id: u32) {
    send_message(world, client_id, "There is nothing to switch.");
}
