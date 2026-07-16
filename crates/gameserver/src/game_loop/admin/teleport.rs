//! Movement commands — `AdminGmSpeed`/`AdminSuperHaste` and the `AdminTeleport`
//! family (`//teleport`, `//recall`, `//teleto`, the directional `//go*`,
//! `//sendhome`, `//walk`, `//teleport_character`, `//recall_npc`, and the
//! teleport HTML menus).

use crate::model::components::{Position, RegionCell, Speeds};
use crate::model::npc::Npc;
use crate::model::Player;
use crate::world::World;

use super::{current_target, find_online_player, send_message, send_sm};

/// `AdminGmSpeed` — scale the target player's (or self's) movement speed. Java
/// adds `baseSpeed * boost` as a fixed value to each speed stat, i.e. total =
/// `baseSpeed * (1 + boost)`; the Rust move model already carries a
/// `move_multiplier`, so `1 + boost` is the exact equivalent (boost 0 resets).
/// Range 0..=10, matching Java's custom clamp. NPC targets are TODO.
pub(super) fn admin_gmspeed(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
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
pub(super) fn admin_teleport_coords(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
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
pub(super) fn admin_recall(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
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
pub(super) fn admin_teleto(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id) else {
        send_message(world, client_id, "Select a target first.");
        return;
    };
    let Some(&pos) = world.objects.get_component::<Position>(&target) else { return };
    super::death::teleport_player(world, object_id, pos.x, pos.y, pos.z);
}

/// `AdminTeleport`'s directional `//gonorth|gosouth|goeast|gowest|goup|godown
/// [offset]` — nudge the GM by `offset` (default 150) units along one axis
/// (Java: north = -y, south = +y, east = +x, west = -x, up = +z, down = -z).
pub(super) fn admin_go(world: &mut World, client_id: u32, object_id: i32, dir: &str, args: &[&str]) {
    let offset = args.first().and_then(|s| s.parse::<i32>().ok()).unwrap_or(150);
    let Some(mut pos) = world.objects.get_component::<Position>(&object_id).copied() else { return };
    match dir {
        "east" => pos.x += offset,
        "west" => pos.x -= offset,
        "north" => pos.y -= offset,
        "south" => pos.y += offset,
        "up" => pos.z += offset,
        "down" => pos.z -= offset,
        _ => {
            send_message(world, client_id, "Usage: //go<north|south|east|west|up|down> [offset]");
            return;
        }
    }
    super::death::teleport_player(world, object_id, pos.x, pos.y, pos.z);
}

/// `AdminTeleport`'s `//walk <x> <y> <z>` — Java sets a move-to AI intention;
/// this server teleports the GM there instead (the admin move-intent path is a
/// documented simplification).
pub(super) fn admin_walk(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    admin_teleport_coords(world, client_id, object_id, args);
}

/// `AdminTeleport`'s `//sendhome [name]` — teleport the targeted or named player
/// to their town respawn point (Java `teleportHome`).
pub(super) fn admin_sendhome(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let target = match args.first() {
        Some(name) => match find_online_player(world, name) {
            Some(t) => t,
            None => {
                send_sm(world, client_id, crate::network::server_packets::sm_ids::THAT_PLAYER_IS_NOT_ONLINE);
                return;
            }
        },
        None => match current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid)) {
            Some(t) => t,
            None => {
                send_sm(world, client_id, crate::network::server_packets::sm_ids::INVALID_TARGET);
                return;
            }
        },
    };
    let Some(pos) = world.objects.get_component::<Position>(&target).copied() else { return };
    let race = world
        .objects
        .get_component::<Player>(&target)
        .and_then(|p| crate::enums::Race::from_ordinal(p.race))
        .unwrap_or(crate::enums::Race::Human);
    if let Some((x, y, z)) = world.data.map_region.town_respawn(pos.x, pos.y, pos.z, race, 0) {
        super::death::teleport_player(world, target, x, y, z);
    }
}

/// `AdminTeleport`'s `//teleport_character <x> <y> <z>` — teleport the currently
/// targeted player to explicit coordinates.
pub(super) fn admin_teleport_character(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(x), Some(y), Some(z)) = (
        args.first().and_then(|s| s.parse::<i32>().ok()),
        args.get(1).and_then(|s| s.parse::<i32>().ok()),
        args.get(2).and_then(|s| s.parse::<i32>().ok()),
    ) else {
        send_message(world, client_id, "Wrong or no Coordinates given.");
        return;
    };
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid)) else {
        send_sm(world, client_id, crate::network::server_packets::sm_ids::INVALID_TARGET);
        return;
    };
    super::death::teleport_player(world, target, x, y, z);
}

/// `AdminTeleport`'s `//recall_npc` — move the targeted NPC to the GM (Java
/// re-creates the spawn at the GM; here it despawns the corpse-less NPC and
/// spawns a fresh one of the same id at the GM's position).
pub(super) fn admin_recall_npc(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Npc>(oid)) else {
        send_sm(world, client_id, crate::network::server_packets::sm_ids::INVALID_TARGET);
        return;
    };
    let npc_id = world.objects.get_component::<Npc>(&target).map_or(0, |n| n.npc_id);
    let Some(region) = world.objects.get_component::<RegionCell>(&target).map(|r| r.0) else { return };
    let Some(gm_pos) = world.objects.get_component::<Position>(&object_id).copied() else { return };
    super::death::despawn_npc(world, target, region);
    if let Some(spawned) = crate::model::npc::spawn_npc_at(world, npc_id, gm_pos.x, gm_pos.y, gm_pos.z, gm_pos.heading) {
        super::death::introduce_npc(world, spawned);
        let name = world.data.npc_data.get(npc_id).map(|t| t.name.clone()).unwrap_or_default();
        send_message(world, client_id, &format!("Recalled {name}."));
    }
}

/// `AdminTeleport`'s teleport HTML menus (`//show_moves`, `//show_moves_other`,
/// `//show_teleport`).
pub(super) fn admin_teleport_menu(world: &mut World, client_id: u32, command: &str) {
    let page = match command {
        "admin_show_moves_other" => "tele/other.html",
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
    let Some(level) = args.first().and_then(|s| s.parse::<i32>().ok()).filter(|v| (0..=4).contains(v)) else {
        send_message(world, client_id, "Usage: //superhaste <Effect level (0-4)>");
        return;
    };
    // Always clear any existing super-haste first (Java stopSkillEffects).
    crate::game_loop::skills::effects::handle_buff_expire(world, object_id, SUPER_HASTE_ID);
    if level == 0 {
        return;
    }
    let Some(skill) = world.data.skill_data.get(SUPER_HASTE_ID, level).cloned() else {
        send_message(world, client_id, "Super-haste skill not found.");
        return;
    };
    crate::game_loop::skills::effects::apply_skill_effects(world, object_id, object_id, &skill);
}
