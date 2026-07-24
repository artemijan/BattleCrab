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

/// Resolve a spawn-menu "Id/Name" token to an npc id. All-digit tokens are npc
/// ids (Java `monsterId.matches("[0-9]*")`); anything else is a name — `_` maps
/// to a space and lookup is case-insensitive (Java `getTemplateByName`). Returns
/// `None` when the id/name is unknown (Java's null-template → `spawns.htm`).
fn resolve_npc_id(world: &World, token: &str) -> Option<i32> {
    if !token.is_empty() && token.bytes().all(|b| b.is_ascii_digit()) {
        let id = token.parse::<i32>().ok()?;
        return world.data.npc_data.get(id).map(|_| id);
    }
    world
        .data
        .npc_data
        .get_by_name(&token.replace('_', " "))
        .map(|t| t.id)
}

/// `AdminSpawn`'s `//spawn` / `//spawn_monster` / `//spawn_once
/// <npcId> [count] [respawn]` (the main-menu "Spawn" button is
/// `admin_spawn_monster $qbox`) — port of `AdminSpawn.spawnMonster`. A missing or
/// unknown npc id opens `spawns.htm` (Java's `catch`/NPE-on-null-template path);
/// otherwise `count` (default 1) NPCs spawn at the current target's location (or
/// the GM's), facing the GM's heading, and the GM gets "Created <name> on
/// <targetObjectId>". Respawn is not persisted for runtime spawns (module note).
///
/// The first token is an npc id when all-numeric (Java `monsterId.matches("[0-9]*")`);
/// otherwise it is an npc name — `_` becomes a space and the template is looked
/// up case-insensitively (Java `getTemplateByName`), which is what the spawn
/// menu's "Id/Name" input relies on. Multi-word name search (Java's token-walk)
/// is not ported: the menu passes a single underscore-joined token.
pub(super) fn admin_spawn(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let npc_id = args.first().and_then(|token| resolve_npc_id(world, token));
    let Some(npc_id) = npc_id else {
        super::menu::show_admin_html(world, client_id, "spawns.htm");
        let token = args.first().copied().unwrap_or("");
        send_message(world, client_id, &format!("NPC {token} doesnt exist"));
        return;
    };
    let template_name = world
        .data
        .npc_data
        .get(npc_id)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let count = args.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(1);
    // Anchor object = current target (any object) or the GM (Java
    // `target == null ? activeChar : target`); the message reports its id.
    let anchor = current_target(world, object_id).unwrap_or(object_id);
    let Some(pos) = world
        .objects
        .get_component::<Position>(&anchor)
        .or_else(|| world.objects.get_component::<Position>(&object_id))
        .copied()
    else {
        return;
    };
    // Heading = the GM's heading (Java `spawn.setHeading(activeChar.getHeading())`).
    let heading = world
        .objects
        .get_component::<Position>(&object_id)
        .map_or(0, |p| p.heading);
    for _ in 0..count.max(0) {
        if let Some(spawned) =
            crate::model::npc::spawn_npc_at(world, npc_id, pos.x, pos.y, pos.z, heading)
        {
            super::death::introduce_npc(world, spawned);
        }
    }
    send_message(
        world,
        client_id,
        &format!("Created {template_name} on {anchor}"),
    );
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
        send_message(
            world,
            client_id,
            "Usage: //spawnat <npcId> <x> <y> <z> [heading]",
        );
        return;
    };
    if world.data.npc_data.get(npc_id).is_none() {
        send_message(
            world,
            client_id,
            &format!("NPC id {npc_id} does not exist."),
        );
        return;
    }
    let heading = args
        .get(4)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or_else(|| {
            world
                .objects
                .get_component::<Position>(&object_id)
                .map_or(0, |p| p.heading)
        });
    if let Some(spawned) = crate::model::npc::spawn_npc_at(world, npc_id, x, y, z, heading) {
        super::death::introduce_npc(world, spawned);
        send_message(
            world,
            client_id,
            &format!("Spawned NPC {npc_id} at {x},{y},{z}."),
        );
    }
}

/// `AdminSpawn`'s static HTML menu commands (`//show_spawns`, `//show_npcs`,
/// `//spawn_debug_menu`) — open the matching admin HTML page. The dynamic
/// listings (`//spawn_index`, `//npc_index`) have their own handlers below.
pub(super) fn admin_spawn_menu(world: &mut World, client_id: u32, command: &str) {
    let page = match command {
        "admin_show_npcs" => "npcs.htm",
        "admin_spawn_debug_menu" => "spawns_debug.htm",
        _ => "spawns.htm",
    };
    super::menu::show_admin_html(world, client_id, page);
}

/// Number of rows one listing page shows before the `Next` button (Java's
/// `j < 50` loop bound in `showMonsters`/`showNpcs`).
const LISTING_PAGE_SIZE: usize = 50;

/// `AdminSpawn.showMonsters` — the spawn menu's "Spawn by Level" **List** buttons
/// (`admin_spawn_index <level> [from]`). Lists every `Monster`-type NPC of that
/// exact level as `admin_spawn_monster <id>` links, 50 per page, with a `Next`
/// button carrying the running offset and a `Back` to the spawn menu. A missing
/// or non-numeric level falls back to `spawns.htm` (Java's `catch`).
pub(super) fn admin_spawn_index(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(level) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        super::menu::show_admin_html(world, client_id, "spawns.htm");
        return;
    };
    let from = args
        .get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let mobs = world.data.npc_data.monsters_of_level(level);
    let total = mobs.len();

    let mut html = format!(
        "<html><title>Spawn Monster:</title><body><p> Level : {level}<br>Total NPCs : {total}<br>"
    );
    let mut i = from;
    for t in mobs.iter().skip(from).take(LISTING_PAGE_SIZE) {
        html.push_str(&format!(
            "<a action=\"bypass -h admin_spawn_monster {}\">{}</a><br1>",
            t.id, t.name
        ));
        i += 1;
    }
    if i >= total {
        html.push_str(
            "<br><center><button value=\"Back\" action=\"bypass -h admin_show_spawns\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>",
        );
    } else {
        html.push_str(&format!(
            "<br><center><button value=\"Next\" action=\"bypass -h admin_spawn_index {level} {i}\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"><button value=\"Back\" action=\"bypass -h admin_show_spawns\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>"
        ));
    }
    super::menu::send_admin_html_content(world, client_id, &html);
}

/// `AdminSpawn.showNpcs` — the NPC menu's A–Z **letter** buttons
/// (`admin_npc_index <letter> [from]`). Lists `Folk`-type NPCs whose name starts
/// with `letter` (case-sensitive prefix, as Java) as `admin_spawn_monster <id>`
/// links, paged like [`admin_spawn_index`]. A missing letter falls back to
/// `npcs.htm` (Java's `catch`).
pub(super) fn admin_npc_index(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(&starting) = args.first() else {
        super::menu::show_admin_html(world, client_id, "npcs.htm");
        return;
    };
    let from = args
        .get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let mobs = world.data.npc_data.folk_starting_with(starting);
    let total = mobs.len();

    let mut html = format!(
        "<html><title>Spawn Monster:</title><body><p> There are {total} Npcs whose name starts with {starting}:<br>"
    );
    let mut i = from;
    for t in mobs.iter().skip(from).take(LISTING_PAGE_SIZE) {
        html.push_str(&format!(
            "<a action=\"bypass -h admin_spawn_monster {}\">{}</a><br1>",
            t.id, t.name
        ));
        i += 1;
    }
    if i >= total {
        html.push_str(
            "<br><center><button value=\"Back\" action=\"bypass -h admin_show_npcs\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>",
        );
    } else {
        html.push_str(&format!(
            "<br><center><button value=\"Next\" action=\"bypass -h admin_npc_index {starting} {i}\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"><button value=\"Back\" action=\"bypass -h admin_show_npcs\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>"
        ));
    }
    super::menu::send_admin_html_content(world, client_id, &html);
}

/// All NPC object ids currently in the world (across every region index).
fn all_npc_ids(world: &World) -> Vec<i32> {
    world.npc_regions.values().flatten().copied().collect()
}

/// `AdminSpawn.findNpcs` — the `//list_spawns` / `//list_positions
/// <npcId> [tele_index]` commands. Lists every configured spawn line of `npcId`
/// 1-indexed; with a numeric `tele_index` it teleports the GM to that line
/// instead of listing. `show_position` (`//list_positions`) reports the current
/// location of the nearest live NPC of that id when one exists, else the
/// configured point; `//list_spawns` always reports the configured point.
///
/// Java keys this off `SpawnTable.getSpawns(npcId)` (one entry per registered
/// spawn line, each with `getLastSpawn()`). This port enumerates the loaded
/// fixed-location spawn definitions in file order; territory-only lines (no
/// single configured point) and the per-line `count` multiplier are collapsed
/// to one entry — documented deviations, immaterial to the teleport use this
/// command exists for. NPC-name search (Java `getTemplateByName`) is not ported;
/// like the other admin spawn commands this takes a numeric id only.
pub(super) fn admin_list_spawns(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
    show_position: bool,
) {
    let Some(npc_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(
            world,
            client_id,
            "Command format is //list_spawns <npcId> [tele_index]",
        );
        return;
    };
    let tele_index = args.get(1).and_then(|s| s.parse::<i32>().ok());

    // Configured spawn points for this id, in file order — the 1-based index
    // space shared by both listing and the teleport form.
    let entries: Vec<(i32, i32, i32)> = world
        .data
        .spawn_data
        .spawns
        .iter()
        .flat_map(|t| t.groups.iter())
        .flat_map(|g| g.npcs.iter())
        .filter(|def| def.npc_id == npc_id)
        .filter_map(|def| def.loc.map(|l| (l.x, l.y, l.z)))
        .collect();

    // Live NPCs of this id (for `//list_positions` current-location reporting).
    let live: Vec<(i32, i32, i32)> = all_npc_ids(world)
        .into_iter()
        .filter(|oid| world.objects.get_component::<Npc>(oid).map(|n| n.npc_id) == Some(npc_id))
        .filter_map(|oid| {
            world
                .objects
                .get_component::<Position>(&oid)
                .map(|p| (p.x, p.y, p.z))
        })
        .collect();

    // For `//list_positions`, resolve an entry to the nearest live NPC's current
    // position (Java's `spawn.getLastSpawn()`); fall back to the configured
    // point when none is alive or for `//list_spawns`.
    let resolve = |(ex, ey, ez): (i32, i32, i32)| -> (i32, i32, i32) {
        if show_position {
            if let Some(&(x, y, z)) = live.iter().min_by_key(|(x, y, z)| {
                let (dx, dy, dz) = ((x - ex) as i64, (y - ey) as i64, (z - ez) as i64);
                dx * dx + dy * dy + dz * dz
            }) {
                return (x, y, z);
            }
        }
        (ex, ey, ez)
    };

    if let Some(idx) = tele_index {
        let entry = if idx >= 1 {
            entries.get((idx - 1) as usize).copied()
        } else {
            None
        };
        match entry {
            Some(e) => {
                let (x, y, z) = resolve(e);
                super::death::teleport_player(world, object_id, x, y, z);
            }
            None => send_message(world, client_id, "No spawn found at that index."),
        }
        return;
    }

    if entries.is_empty() {
        // Java `findNpcs`: `getClass().getSimpleName() + ": No current spawns found."`.
        send_message(world, client_id, "AdminSpawn: No current spawns found.");
        return;
    }
    let name = world
        .data
        .npc_data
        .get(npc_id)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    for (i, &entry) in entries.iter().enumerate() {
        let (x, y, z) = resolve(entry);
        // Java line: `index + " - " + name + " (" + spawn + "): " + x + " " + y + " " + z`.
        // The `spawn` token is the Java `Spawn.toString()` (an internal handle),
        // omitted here as it has no faithful port.
        send_message(
            world,
            client_id,
            &format!("{} - {name}: {x} {y} {z}", i + 1),
        );
    }
}

/// `AdminSpawn`'s `//top_spawn_count [n]` — the `n` most-spawned NPC ids in the
/// live world.
pub(super) fn admin_top_spawn_count(world: &mut World, client_id: u32, args: &[&str]) {
    let top = args
        .first()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(5)
        .max(1);
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
        let name = world
            .data
            .npc_data
            .get(npc_id)
            .map(|t| t.name.clone())
            .unwrap_or_default();
        send_message(world, client_id, &format!("  {count} x {name} ({npc_id})"));
    }
}

/// `AdminSpawn`'s `//spawn_debug_print <type>` — dump the targeted NPC's id and
/// position (Java prints spawn/AI internals; text summary here).
pub(super) fn admin_spawn_debug_print(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) =
        current_target(world, object_id).filter(|oid| world.objects.has_component::<Npc>(oid))
    else {
        super::send_sm(
            world,
            client_id,
            crate::network::server_packets::sm_ids::INVALID_TARGET,
        );
        return;
    };
    let npc_id = world
        .objects
        .get_component::<Npc>(&target)
        .map_or(0, |n| n.npc_id);
    let name = world
        .data
        .npc_data
        .get(npc_id)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let pos = world
        .objects
        .get_component::<Position>(&target)
        .copied()
        .unwrap_or(Position {
            x: 0,
            y: 0,
            z: 0,
            heading: 0,
        });
    send_message(
        world,
        client_id,
        &format!("NPC {name} ({npc_id}) obj {target}"),
    );
    send_message(
        world,
        client_id,
        &format!("Loc: {},{},{} heading {}", pos.x, pos.y, pos.z, pos.heading),
    );
}

/// `AdminScan`'s `//scan` — list the NPCs visible from the GM's region.
pub(super) fn admin_scan(world: &mut World, client_id: u32, object_id: i32) {
    let Some(region) = world
        .objects
        .get_component::<RegionCell>(&object_id)
        .map(|r| r.0)
    else {
        return;
    };
    let ids = world.npcs_visible_from(region);
    // `scan.htm` (Java `AdminScan.getScanResult`): each NPC is a `move_to` link
    // plus a Delete link (`admin_deletenpcbyobjectid`).
    let mut rows = String::new();
    for oid in ids {
        if let Some(npc) = world.objects.get_component::<Npc>(&oid) {
            let name = world
                .data
                .npc_data
                .get(npc.npc_id)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            let name = if name.is_empty() {
                "No name NPC".to_string()
            } else {
                name
            };
            let pos = world
                .objects
                .get_component::<Position>(&oid)
                .copied()
                .unwrap_or(Position {
                    x: 0,
                    y: 0,
                    z: 0,
                    heading: 0,
                });
            rows.push_str(&format!(
                "<tr><td width=\"45\">{}</td>\
                 <td><a action=\"bypass -h admin_move_to {} {} {}\">{name}</a></td>\
                 <td width=\"54\"><a action=\"bypass -h admin_deletenpcbyobjectid objectId={oid}\"><font color=\"LEVEL\">Delete</font></a></td></tr>",
                npc.npc_id, pos.x, pos.y, pos.z
            ));
        }
    }
    super::menu::show_admin_html_replace(
        world,
        client_id,
        "scan.htm",
        &[("data", rows), ("pages", String::new())],
    );
}

/// `AdminScan`'s `//deleteNpcByObjectId objectId=<id>` — the scan list's
/// **Delete** links. Port of `AdminScan.useAdminCommand`'s
/// `admin_deletenpcbyobjectid` case: resolve the object id (a `key=value` bypass
/// param), despawn the NPC if it is one, message the GM, then re-render the scan
/// list. Runtime spawns carry no persisted respawn here, so `deleteMe` maps to
/// [`despawn_npc`] with no spawn-table bookkeeping (documented module note).
pub(super) fn admin_delete_npc_by_object_id(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    // Java: no token after the command → usage message.
    if args.is_empty() {
        send_message(
            world,
            client_id,
            "Usage: //deletenpcbyobjectid objectId=<object_id>",
        );
        return;
    }
    // `BypassParser.getInt("objectId", 0)` over the `key=value` tokens.
    let target_oid = bypass_param(args, "objectId")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);
    if target_oid == 0 {
        // Java sends this but does not return; it falls through to the
        // not-an-NPC branch below (findObject(0) == null).
        send_message(world, client_id, "objectId is not set!");
    }
    let Some(region) = world
        .objects
        .get_component::<RegionCell>(&target_oid)
        .map(|r| r.0)
        .filter(|_| world.objects.has_component::<Npc>(&target_oid))
    else {
        send_message(
            world,
            client_id,
            "NPC does not exist or object_id does not belong to an NPC",
        );
        return;
    };
    let npc_id = world
        .objects
        .get_component::<Npc>(&target_oid)
        .map_or(0, |n| n.npc_id);
    let name = world
        .data
        .npc_data
        .get(npc_id)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    super::death::despawn_npc(world, target_oid, region);
    send_message(world, client_id, &format!("{name} have been deleted."));
    // Java `processBypass` re-renders the scan list.
    admin_scan(world, client_id, object_id);
}

/// Extract a `key=value` bypass parameter (Java `BypassParser`). The key is
/// matched case-insensitively; the first match wins.
fn bypass_param<'a>(args: &[&'a str], key: &str) -> Option<&'a str> {
    args.iter().find_map(|tok| {
        let (k, v) = tok.split_once('=')?;
        k.eq_ignore_ascii_case(key).then_some(v)
    })
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
        send_message(
            world,
            client_id,
            "This is only a temporary spawn. The mob(s) will NOT respawn.",
        );
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
    if !world
        .objects
        .has_component::<crate::model::npc::Npc>(&target)
    {
        send_message(world, client_id, "Target is not an NPC.");
        return;
    }
    let Some(region) = world
        .objects
        .get_component::<RegionCell>(&target)
        .map(|r| r.0)
    else {
        return;
    };
    super::death::despawn_npc(world, target, region);
}
