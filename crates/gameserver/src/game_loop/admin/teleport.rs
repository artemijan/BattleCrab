//! Movement commands — `AdminGmSpeed`/`AdminSuperHaste` and the `AdminTeleport`
//! family (`//teleport`, `//recall`, `//teleto`, the directional `//go*`,
//! `//sendhome`, `//walk`, `//teleport_character`, `//recall_npc`, and the
//! teleport HTML menus).

use crate::enums::AdminTeleportType;
use crate::game_loop::guard::{self, Guard, OrReject, Reject};
use crate::game_loop::helpers;
use crate::game_loop::helpers::nth_arg;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::skill_by_id;
use crate::model::Player;
use crate::model::components::Speeds;
use crate::model::npc::Npc;
use crate::network::server_packets::sm_ids;
use crate::world::World;

use super::{find_online_player, send_message, send_sm};
use crate::game_loop::helpers::region_cell_of;

/// `AdminGmSpeed` — scale the target player's (or self's) movement speed. Java
/// **The argument is an outright multiplier, not a boost**, despite Java naming
/// it `runSpeedBoost`. It goes in through `addFixedValue`, and a fixed value is
/// an *override*: `CreatureStat.getValue` returns it directly and never reaches
/// the finalizer. So `//gmspeed 3` is `base * 3`, and `//gmspeed 1` is a no-op
/// rather than double speed. This read `1 + boost` — off by one whole multiple
/// of base speed at every setting except 0.
///
/// `0` removes the fixed value, i.e. back to the normally-finalized speed.
/// Range 0..=10 is Java's own custom clamp ("real retail limit is unknown").
///
/// The target is any **Creature** — Java takes `target.isCreature()`, so an NPC
/// can be sped up too; only a non-creature target falls back to the GM.
pub(super) fn admin_gmspeed(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(mult) = nth_arg::<f64>(args, 0).filter(|b| (0.0..=10.0).contains(b)) else {
        send_message(world, client_id, "//gmspeed [0...10]");
        return;
    };
    // Java's `getTarget()` filtered by `isCreature()`; the port's creature
    // targets are players and NPCs.
    let target = guard::target(world, object_id)
        .filter(|oid| {
            world.objects.has_component::<Player>(oid) || world.objects.has_component::<Npc>(oid)
        })
        .unwrap_or(object_id);
    if let Some(speeds) = world.objects.get_component_mut::<Speeds>(&target) {
        // `removeFixedValue` on 0, else the override.
        speeds.move_multiplier = if mult > 0.0 { mult } else { 1.0 };
    }
    let name = if let Some(p) = world.objects.get_component::<Player>(&target) {
        p.name.clone()
    } else {
        world
            .objects
            .get_component::<Npc>(&target)
            .and_then(|n| n.template(world).map(|t| t.name.clone()))
            .unwrap_or_default()
    };
    if world.objects.has_component::<Player>(&target) {
        super::party::broadcast_user_info(world, target);
    } else if let Some(pkt) = crate::game_loop::visibility::npc_info_bytes(world, target) {
        // Java `broadcastInfo()` for a non-player creature.
        crate::game_loop::helpers::broadcast_including_self(world, target, &pkt);
    }
    send_message(
        world,
        client_id,
        &format!("[{name}] speed is [{}0]% fast.", mult * 100.0),
    );
}

/// `AdminTeleport`'s `//move_to <x> <y> <z>` (the main-menu "Teleport" button,
/// `admin_move_to $qbox`) — a faithful port of `AdminTeleport`'s `admin_move_to`
/// + `teleportTo`. Empty coordinates (blank QuickBox) open `teleports.htm`
/// (Java's `StringIndexOutOfBoundsException` branch); a non-numeric token sends
/// the usage line and opens `teleports.htm` (`NumberFormatException`); too few
/// tokens sends "Wrong or no Coordinates given." (`teleportTo`'s
/// `NoSuchElementException`); valid coordinates teleport the GM and confirm.
pub(super) fn admin_move_to(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    if args.is_empty() {
        super::menu::show_admin_html(world, client_id, "teleports.htm");
        return;
    }
    // Emulate `teleportTo`'s sequential tokenizer parse of exactly three ints.
    let mut coords = [0i32; 3];
    for (i, slot) in coords.iter_mut().enumerate() {
        match args.get(i) {
            // Fewer than three tokens → `NoSuchElementException`.
            None => {
                send_message(world, client_id, "Wrong or no Coordinates given.");
                return;
            }
            // Non-numeric token → `NumberFormatException` (outer catch).
            Some(tok) => match tok.parse::<i32>() {
                Ok(v) => *slot = v,
                Err(_) => {
                    send_message(world, client_id, "Usage: //move_to <x> <y> <z>");
                    super::menu::show_admin_html(world, client_id, "teleports.htm");
                    return;
                }
            },
        }
    }
    super::death::teleport_player(world, object_id, coords[0], coords[1], coords[2]);
    send_message(
        world,
        client_id,
        &format!("You have been teleported to {}", args.join(" ")),
    );
}

/// `AdminTeleport`'s coordinate form (`//teleport x y z`) — send the GM to an
/// explicit location.
///
/// The sibling variants live next door: [`admin_teleportto`] takes a player
/// name, [`admin_teleto`] uses the current target, and [`admin_move_to`] is the
/// html picker's button.
pub(super) fn admin_teleport_coords(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let coords = (
        nth_arg::<i32>(args, 0),
        nth_arg::<i32>(args, 1),
        nth_arg::<i32>(args, 2),
    );
    let (Some(x), Some(y), Some(z)) = coords else {
        send_message(world, client_id, "Usage: //teleport <x> <y> <z>");
        return;
    };
    super::death::teleport_player(world, object_id, x, y, z);
}

/// `AdminTeleport`'s `//recall <name>` — bring an online player to the GM's
/// location (or, with no name, the currently targeted player).
pub(super) fn admin_recall(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let result = recall(world, object_id, args);
    guard::finish(world, client_id, result);
}

fn recall(world: &mut World, object_id: i32, args: &[&str]) -> Guard<()> {
    let target = match args.first() {
        Some(name) => find_online_player(world, name),
        None => guard::player_target(world, object_id),
    }
    .or_msg("Usage: //recall <player name>")?;
    super::death::teleport_to_object(world, target, object_id);
    Ok(())
}

/// `AdminTeleport`'s `//teleto` — send the GM to the current target's position.
///
/// Java reaches this only through the *aliases* (`//teleportto <name>`,
/// `//teleport_to_character`); its own `admin_teleto` arm is the mode latch
/// below, and a bare `//teleto` falls off the end of the if-chain doing
/// nothing. The port keeps the bare form as the target-teleport (it is the
/// documented behaviour of `AdminMenu.teleportToCharacter`) and routes the
/// three mode words to [`admin_teleto_mode`] before ever getting here.
pub(super) fn admin_teleto(world: &mut World, client_id: u32, object_id: i32) {
    let result = teleto(world, object_id);
    guard::finish(world, client_id, result);
}

fn teleto(world: &mut World, object_id: i32) -> Guard<()> {
    let target = guard::target(world, object_id).or_msg("Select a target first.")?;
    super::death::teleport_to_object(world, object_id, target);
    Ok(())
}

/// `AdminTeleport`'s `//teleportto <name>` — send the GM to a **named** player,
/// where [`admin_teleto`] goes to the current target.
///
/// Java's `teleportToCharacter` guards in order: a missing or non-player target
/// answers `INVALID_TARGET`, and targeting *yourself* answers
/// `YOU_CANNOT_USE_THIS_ON_YOURSELF` — note Java sends that second one **to the
/// target**, which for the self case is the same person, so it reads correctly
/// either way. A successful jump clears the GM's AI intention first and then
/// confirms by name.
pub(super) fn admin_teleportto(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let name = args.join(" ");
    // Java's guard is `startsWith("admin_teleportto ")` — *with* the trailing
    // space — so a bare `//teleportto` misses it and falls through the if-chain
    // into the next arm, `startsWith("admin_teleport")`, whose tokenizer then
    // fails on the missing coordinates. Following the fallthrough rather than
    // returning quietly keeps that "Wrong coordinates!" reply.
    if name.trim().is_empty() {
        admin_teleport_coords(world, client_id, object_id, args);
        return;
    }
    let Some(target) = find_online_player(world, name.trim()) else {
        send_sm(
            world,
            client_id,
            crate::network::server_packets::sm_ids::INVALID_TARGET,
        );
        return;
    };
    if target == object_id {
        send_sm(
            world,
            client_id,
            crate::network::server_packets::sm_ids::YOU_CANNOT_USE_THIS_ON_YOURSELF,
        );
        return;
    }
    let target_name = player_name_or_empty(world, target);
    super::death::teleport_to_object(world, object_id, target);
    send_message(
        world,
        client_id,
        &format!("You have teleported to character {target_name}."),
    );
}

/// `AdminTeleport`'s click-to-move latches — the "Move:" row of
/// `html/admin/move.htm` ("Additional Movement Options"). Java arms
/// `Player.setTeleMode(...)` and the next `MoveBackwardToLocation` consumes it
/// (see [`crate::game_loop::position::handle_move_backward_to_location`]):
///
/// * `//instant_move` → `DEMONIC` ("Demonic mode")
/// * `//teleto sayune` → `SAYUNE`
/// * `//teleto charge` → `CHARGE`
/// * `//teleto end` → `NORMAL` ("Normal mode"), the only one with no chat line
///
/// The confirmation strings are Java's verbatim (`BuilderUtil.sendSysMessage`).
pub(super) fn admin_teleto_mode(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    mode: AdminTeleportType,
) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.tele_mode = mode;
    } else {
        return;
    }
    let msg = match mode {
        AdminTeleportType::Demonic => "Instant move ready. Click where you want to go.",
        AdminTeleportType::Sayune => "Sayune move ready. Click where you want to go.",
        AdminTeleportType::Charge => "Charge move ready. Click where you want to go.",
        // Java's `admin_teleto end` arm only calls `setTeleMode(NORMAL)`.
        AdminTeleportType::Normal => return,
    };
    send_message(world, client_id, msg);
}

/// `AdminMenu`'s `admin_goto_char_menu <name>` — the "Go To" button on
/// `charinfo.htm` / `char_menu.htm` / `charmanage.htm`, which passes the already
/// chosen character's name (`%name%` / `$qbox`). Java resolves that name via
/// `World.getPlayer(name)` and never consults the GM's target, so this must NOT
/// delegate to `//teleto`; only the empty-name case (blank QuickBox) falls back
/// to the current target. Mirrors Java `AdminMenu.teleportToCharacter`: a
/// non-player resolves to `INVALID_TARGET`, self to
/// `YOU_CANNOT_USE_THIS_ON_YOURSELF`, otherwise teleport + confirmation, and in
/// the latter two cases the char-manage page is reopened (`showMainPage`).
pub(super) fn admin_goto_char(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let result = goto_char(world, client_id, object_id, args);
    // Java `showMainPage` runs on every path *except* the unresolved-target
    // one, which returns straight out — and that is the only rejection this
    // handler has, so `is_ok()` is the tail condition. The self-target message
    // below is deliberately NOT a rejection: Java sends it and still re-opens
    // the page, so it stays an ordinary send on the success path.
    let reached_tail = result.is_ok();
    guard::finish(world, client_id, result);
    if reached_tail {
        super::menu::show_admin_html(world, client_id, "charmanage.htm");
    }
}

fn goto_char(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) -> Guard<()> {
    let target = match args.first() {
        Some(name) => find_online_player(world, name),
        None => guard::player_target(world, object_id),
    }
    // Java: `!target.isPlayer()` → INVALID_TARGET, and no main page.
    .or_sm(sm_ids::INVALID_TARGET)?;
    if target == object_id {
        send_sm(world, client_id, sm_ids::YOU_CANNOT_USE_THIS_ON_YOURSELF);
    } else if super::death::teleport_to_object(world, object_id, target) {
        // The confirmation stays gated on the teleport actually happening: a
        // target with no position is silently skipped, exactly as before.
        let name = helpers::player_name_or_empty(world, target);
        send_message(
            world,
            client_id,
            &format!("You're teleporting yourself to character {name}"),
        );
    }
    Ok(())
}

/// `AdminTeleport`'s directional `//gonorth|gosouth|goeast|gowest|goup|godown
/// [offset]` — nudge the GM by `offset` (default 150) units along one axis
/// (Java: north = -y, south = +y, east = +x, west = -x, up = +z, down = -z),
/// then re-open the `move.htm` nudge pad (`showTeleportWindow`) so the arrows
/// stay under the cursor for the next click.
pub(super) fn admin_go(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    dir: &str,
    args: &[&str],
) {
    let offset = nth_arg::<i32>(args, 0).unwrap_or(150);
    let Some(mut pos) = guard::maybe_position(world, object_id) else {
        return;
    };
    match dir {
        "east" => pos.x += offset,
        "west" => pos.x -= offset,
        "north" => pos.y -= offset,
        "south" => pos.y += offset,
        "up" => pos.z += offset,
        "down" => pos.z -= offset,
        _ => {
            send_message(
                world,
                client_id,
                "Usage: //go<north|south|east|west|up|down> [offset]",
            );
            return;
        }
    }
    super::death::teleport_player(world, object_id, pos.x, pos.y, pos.z);
    super::menu::show_admin_html(world, client_id, "move.htm");
}

/// `AdminTeleport`'s `//walk <x> <y> <z>` — the "Walk" button next to "Tele" on
/// `move.htm`. Java sets `AI_INTENTION_MOVE_TO`, i.e. the GM *walks* there
/// under the ordinary movement pipeline (geodata clamp, pathfinder, arrival
/// events) rather than teleporting; the pair only makes sense as a pair, so
/// this routes to the same [`intention_move_to`](crate::game_loop::position::intention_move_to)
/// the move packet uses. A malformed coordinate is swallowed silently, as in
/// Java (`catch (Exception e) {}` with an empty body).
pub(super) fn admin_walk(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(x), Some(y), Some(z)) = (
        nth_arg::<i32>(args, 0),
        nth_arg::<i32>(args, 1),
        nth_arg::<i32>(args, 2),
    ) else {
        return;
    };
    let Some(cur) = guard::maybe_position(world, object_id) else {
        return;
    };
    crate::game_loop::position::intention_move_to(world, client_id, object_id, cur, (x, y, z));
}

/// `AdminTeleport`'s `//sendhome [name]` — teleport the targeted or named player
/// to their town respawn point (Java `teleportHome`).
pub(super) fn admin_sendhome(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let result = sendhome(world, object_id, args);
    guard::finish(world, client_id, result);
}

fn sendhome(world: &mut World, object_id: i32, args: &[&str]) -> Guard<()> {
    // The two lookups refuse with different messages, which is why the message
    // belongs at the call site rather than inside the resolver.
    let target = match args.first() {
        Some(name) => find_online_player(world, name).or_sm(sm_ids::THAT_PLAYER_IS_NOT_ONLINE)?,
        None => guard::player_target(world, object_id).or_sm(sm_ids::INVALID_TARGET)?,
    };
    super::death::teleport_to_town(world, target, 0);
    Ok(())
}

/// `AdminTeleport`'s `//teleport_character <x> <y> <z>` — teleport the currently
/// targeted player to explicit coordinates.
pub(super) fn admin_teleport_character(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let result = teleport_character(world, object_id, args);
    guard::finish(world, client_id, result);
}

fn teleport_character(world: &mut World, object_id: i32, args: &[&str]) -> Guard<()> {
    let (Some(x), Some(y), Some(z)) = (
        nth_arg::<i32>(args, 0),
        nth_arg::<i32>(args, 1),
        nth_arg::<i32>(args, 2),
    ) else {
        return Err(Reject::Msg("Wrong or no Coordinates given.".to_string()));
    };
    let target = guard::player_target(world, object_id).or_sm(sm_ids::INVALID_TARGET)?;
    super::death::teleport_player(world, target, x, y, z);
    Ok(())
}

/// `AdminTeleport`'s `//recall_npc` — move the targeted NPC to the GM (Java
/// re-creates the spawn at the GM; here it despawns the corpse-less NPC and
/// spawns a fresh one of the same id at the GM's position).
pub(super) fn admin_recall_npc(world: &mut World, client_id: u32, object_id: i32) {
    let result = recall_npc(world, client_id, object_id);
    guard::finish(world, client_id, result);
}

fn recall_npc(world: &mut World, client_id: u32, object_id: i32) -> Guard<()> {
    let target = guard::npc_target(world, object_id).or_sm(sm_ids::INVALID_TARGET)?;
    let npc_id = world
        .objects
        .get_component::<Npc>(&target)
        .map_or(0, |n| n.npc_id);
    let region = region_cell_of(world, target).or_silent()?;
    let gm_pos = guard::maybe_position(world, object_id).or_silent()?;
    super::death::despawn_npc(world, target, region);
    if let Some(spawned) =
        crate::model::npc::spawn_npc_at(world, npc_id, gm_pos.x, gm_pos.y, gm_pos.z, gm_pos.heading)
    {
        super::death::introduce_npc(world, spawned);
        let name = world
            .data
            .npc_data
            .get(npc_id)
            .map(|t| t.name.clone())
            .unwrap_or_default();
        send_message(world, client_id, &format!("Recalled {name}."));
    }
    Ok(())
}

/// `AdminTeleport`'s teleport HTML menus (`//show_moves`, `//show_moves_other`,
/// `//show_teleport`, `//tele`).
///
/// `//tele` is Java's `showTeleportWindow` → `move.htm`, the "Additional
/// Movement Options" window reached from the button of that name on
/// `teleports.htm`: the directional nudge pad, the click-to-move mode row, the
/// GM-speed row and the tele/walk coordinate box. It is a *different* page from
/// `teleports.htm` (`//show_moves`), and the directional `//go*` handlers
/// re-open it after each nudge, exactly as Java does.
pub(super) fn admin_teleport_menu(world: &mut World, client_id: u32, command: &str) {
    let page = match command {
        "admin_show_moves_other" => "tele/other.html",
        "admin_tele" => "move.htm",
        _ => "teleports.htm",
    };
    super::menu::show_admin_html(world, client_id, page);
}

/// The super-haste skill (`AdminSuperHaste.SUPER_HASTE_ID`), a movement-speed
/// buff applied to the GM.
const SUPER_HASTE_ID: i32 = 7029;

/// `AdminSuperHaste`'s `//superhaste` / `//speed <0-4>` — apply the super-haste
/// buff at the given level to the GM (level 0 removes it).
pub(super) fn admin_superhaste(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(level) = nth_arg::<i32>(args, 0).filter(|v| (0..=4).contains(v)) else {
        send_message(world, client_id, "Usage: //superhaste <Effect level (0-4)>");
        return;
    };
    // Always clear any existing super-haste first (Java stopSkillEffects).
    crate::game_loop::skills::effects::handle_buff_expire(world, object_id, SUPER_HASTE_ID);
    if level == 0 {
        return;
    }
    let Some(skill) = skill_by_id(world, SUPER_HASTE_ID, level) else {
        send_message(world, client_id, "Super-haste skill not found.");
        return;
    };
    crate::game_loop::skills::effects::apply_skill_effects(world, object_id, object_id, &skill);
}
