//! NPC spawn commands — `AdminSpawn` (`//spawn`, `//spawn_monster`,
//! `//spawn_once`, `//spawnat`, the spawn/npc HTML menus, `//list_spawns`,
//! `//top_spawn_count`, `//spawn_debug_print`), `AdminScan`'s `//scan`,
//! `AdminSummon`'s `//summon`, and `AdminDelete`'s `//delete`.
//!
//! Runtime admin spawns are one-off (no respawn): Java registers them in the
//! `SpawnTable` so the respawn scheduler can re-run the spawn line, but this
//! server's respawn path keys off a real spawn-line index that a runtime spawn
//! has no entry in. The `respawn`/`permanent` argument is therefore accepted
//! and ignored (documented deviation).

use crate::model::components::{Position, RegionCell};
use crate::model::npc::Npc;
use crate::world::World;

use super::{current_target, send_message};

/// Position to spawn at — the current target's if one is selected (any object),
/// else the GM's own (Java `target == null ? activeChar : target`).
fn spawn_anchor(world: &World, object_id: i32) -> Option<Position> {
    let anchor = current_target(world, object_id).unwrap_or(object_id);
    world
        .objects
        .get_component::<Position>(&anchor)
        .or_else(|| world.objects.get_component::<Position>(&object_id))
        .copied()
}

/// `AdminSpawn`'s `//spawn` / `//spawn_monster` / `//spawn_once
/// <npcId> [count] [respawn]` — spawn `count` NPCs at the anchor (target or GM).
/// Respawn is not persisted for runtime spawns (see the module note).
pub(super) fn admin_spawn(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(npc_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //spawn <npcId> [count] [respawn]");
        return;
    };
    let count = args.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(1).clamp(1, 100);
    let Some(template_name) = world.data.npc_data.get(npc_id).map(|t| t.name.clone()) else {
        send_message(world, client_id, &format!("NPC id {npc_id} does not exist."));
        return;
    };
    let Some(pos) = spawn_anchor(world, object_id) else { return };
    for _ in 0..count {
        if let Some(spawned) = crate::model::npc::spawn_npc_at(world, npc_id, pos.x, pos.y, pos.z, pos.heading) {
            super::death::introduce_npc(world, spawned);
        }
    }
    send_message(world, client_id, &format!("Created {template_name} x{count}."));
}

/// `AdminSpawn`'s `//spawnat <npcId> <x> <y> <z> [heading]` — spawn one NPC at
/// explicit coordinates.
pub(super) fn admin_spawnat(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(npc_id), Some(x), Some(y), Some(z)) = (
        args.first().and_then(|s| s.parse::<i32>().ok()),
        args.get(1).and_then(|s| s.parse::<i32>().ok()),
        args.get(2).and_then(|s| s.parse::<i32>().ok()),
        args.get(3).and_then(|s| s.parse::<i32>().ok()),
    ) else {
        send_message(world, client_id, "Usage: //spawnat <npcId> <x> <y> <z> [heading]");
        return;
    };
    if world.data.npc_data.get(npc_id).is_none() {
        send_message(world, client_id, &format!("NPC id {npc_id} does not exist."));
        return;
    }
    let heading = args
        .get(4)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or_else(|| world.objects.get_component::<Position>(&object_id).map_or(0, |p| p.heading));
    if let Some(spawned) = crate::model::npc::spawn_npc_at(world, npc_id, x, y, z, heading) {
        super::death::introduce_npc(world, spawned);
        send_message(world, client_id, &format!("Spawned NPC {npc_id} at {x},{y},{z}."));
    }
}

/// `AdminSpawn`'s HTML menu commands (`//show_spawns`, `//show_npcs`,
/// `//spawn_debug_menu`, `//spawn_index`, `//npc_index`) — open the matching
/// admin HTML page.
pub(super) fn admin_spawn_menu(world: &mut World, client_id: u32, command: &str) {
    let page = match command {
        "admin_show_npcs" | "admin_npc_index" => "npcs.htm",
        "admin_spawn_debug_menu" => "spawns_debug.htm",
        _ => "spawns.htm",
    };
    super::menu::show_admin_html(world, client_id, page);
}

/// All NPC object ids currently in the world (across every region index).
fn all_npc_ids(world: &World) -> Vec<i32> {
    world.npc_regions.values().flatten().copied().collect()
}

/// `AdminSpawn`'s `//list_spawns` / `//list_positions <npcId>` — list the live
/// positions of every spawned NPC with that id (Java opens an HTML window; text
/// here).
pub(super) fn admin_list_spawns(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(npc_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Command format is //list_spawns <npcId> [tele_index]");
        return;
    };
    let name = world.data.npc_data.get(npc_id).map(|t| t.name.clone()).unwrap_or_default();
    let mut n = 0;
    send_message(world, client_id, &format!("=== Spawns of {name} ({npc_id}) ==="));
    for oid in all_npc_ids(world) {
        if world.objects.get_component::<Npc>(&oid).map(|npc| npc.npc_id) == Some(npc_id) {
            if let Some(p) = world.objects.get_component::<Position>(&oid) {
                n += 1;
                send_message(world, client_id, &format!("  {},{},{}", p.x, p.y, p.z));
            }
        }
    }
    send_message(world, client_id, &format!("{n} spawn(s) found."));
}

/// `AdminSpawn`'s `//top_spawn_count [n]` — the `n` most-spawned NPC ids in the
/// live world.
pub(super) fn admin_top_spawn_count(world: &mut World, client_id: u32, args: &[&str]) {
    let top = args.first().and_then(|s| s.parse::<usize>().ok()).unwrap_or(5).max(1);
    let mut counts: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();
    for oid in all_npc_ids(world) {
        if let Some(npc) = world.objects.get_component::<Npc>(&oid) {
            *counts.entry(npc.npc_id).or_default() += 1;
        }
    }
    let mut sorted: Vec<(i32, i32)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    send_message(world, client_id, &format!("=== Top {top} spawns ==="));
    for (npc_id, count) in sorted.into_iter().take(top) {
        let name = world.data.npc_data.get(npc_id).map(|t| t.name.clone()).unwrap_or_default();
        send_message(world, client_id, &format!("  {count} x {name} ({npc_id})"));
    }
}

/// `AdminSpawn`'s `//spawn_debug_print <type>` — dump the targeted NPC's id and
/// position (Java prints spawn/AI internals; text summary here).
pub(super) fn admin_spawn_debug_print(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Npc>(oid)) else {
        super::send_sm(world, client_id, crate::network::server_packets::sm_ids::INVALID_TARGET);
        return;
    };
    let npc_id = world.objects.get_component::<Npc>(&target).map_or(0, |n| n.npc_id);
    let name = world.data.npc_data.get(npc_id).map(|t| t.name.clone()).unwrap_or_default();
    let pos = world.objects.get_component::<Position>(&target).copied().unwrap_or(Position { x: 0, y: 0, z: 0, heading: 0 });
    send_message(world, client_id, &format!("NPC {name} ({npc_id}) obj {target}"));
    send_message(world, client_id, &format!("Loc: {},{},{} heading {}", pos.x, pos.y, pos.z, pos.heading));
}

/// `AdminScan`'s `//scan` — list the NPCs visible from the GM's region.
pub(super) fn admin_scan(world: &mut World, client_id: u32, object_id: i32) {
    let Some(region) = world.objects.get_component::<RegionCell>(&object_id).map(|r| r.0) else { return };
    let ids = world.npcs_visible_from(region);
    send_message(world, client_id, &format!("=== NPCs in view ({}) ===", ids.len()));
    for oid in ids {
        if let Some(npc) = world.objects.get_component::<Npc>(&oid) {
            let name = world.data.npc_data.get(npc.npc_id).map(|t| t.name.clone()).unwrap_or_default();
            let pos = world.objects.get_component::<Position>(&oid).copied().unwrap_or(Position { x: 0, y: 0, z: 0, heading: 0 });
            send_message(world, client_id, &format!("  {name} ({}) @ {},{},{}", npc.npc_id, pos.x, pos.y, pos.z));
        }
    }
}

/// `AdminSummon`'s `//summon <id> [count]` — Java delegates: `id < 1000000` is
/// `//create_item id count`; otherwise a one-off spawn of `id - 1000000`.
pub(super) fn admin_summon(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Incorrect format for command 'summon'");
        return;
    };
    let count = args.get(1).and_then(|s| s.parse::<i64>().ok()).unwrap_or(1);
    if id <= 0 || count <= 0 {
        return;
    }
    if id < 1_000_000 {
        super::quests::give_item_with_earned_message(world, client_id, object_id, id, count);
    } else {
        send_message(world, client_id, "This is only a temporary spawn. The mob(s) will NOT respawn.");
        let npc_id = (id - 1_000_000).to_string();
        let cnt = count.to_string();
        admin_spawn(world, client_id, object_id, &[&npc_id, &cnt]);
    }
}

/// `AdminDelete`'s `//delete` — despawn the targeted NPC.
pub(super) fn admin_delete(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id) else {
        send_message(world, client_id, "Select an NPC first.");
        return;
    };
    if !world.objects.has_component::<crate::model::npc::Npc>(&target) {
        send_message(world, client_id, "Target is not an NPC.");
        return;
    }
    let Some(region) = world.objects.get_component::<RegionCell>(&target).map(|r| r.0)
    else {
        return;
    };
    super::death::despawn_npc(world, target, region);
}
