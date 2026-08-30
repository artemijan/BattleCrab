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

use crate::data::npc_data::NpcTemplate;
use crate::game_loop::helpers::maybe_position;
use crate::game_loop::helpers::{send_message, send_sm_bare_to_client};
use crate::game_loop::target;
use crate::game_loop::{helpers, items};
use crate::model::components::Position;
use crate::model::npc::Npc;
use crate::world::World;

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
    let template_name = helpers::npc_template_name(world, npc_id);
    let count = helpers::nth_arg::<i32>(args, 1).unwrap_or(1);
    // Anchor object = current target (any object) or the GM (Java
    // `target == null ? activeChar : target`); the message reports its id.
    let anchor = target::current(world, object_id).unwrap_or(object_id);
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
            crate::game_loop::npc::spawn_npc_at(world, npc_id, pos.x, pos.y, pos.z, heading)
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
        helpers::nth_arg::<i32>(args, 0),
        helpers::nth_arg::<i32>(args, 1),
        helpers::nth_arg::<i32>(args, 2),
        helpers::nth_arg::<i32>(args, 3),
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
    let heading = helpers::nth_arg::<i32>(args, 4).unwrap_or_else(|| {
        world
            .objects
            .get_component::<Position>(&object_id)
            .map_or(0, |p| p.heading)
    });
    if let Some(spawned) = crate::game_loop::npc::spawn_npc_at(world, npc_id, x, y, z, heading) {
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

/// Shared page body of the two NPC listings (Java `AdminSpawn.showMonsters` /
/// `showNpcs`): the `header` intro line, then one `admin_spawn_monster` link per
/// template starting at `from` and capped at [`LISTING_PAGE_SIZE`], closed by a
/// `Next` button carrying the running offset (`bypass -h <next_command> <i>`,
/// omitted on the last page) and a `Back` to `back_command`.
fn render_npc_listing(
    header: &str,
    mobs: &[&NpcTemplate],
    from: usize,
    next_command: &str,
    back_command: &str,
) -> String {
    let mut html = format!("<html><title>Spawn Monster:</title><body><p> {header}<br>");
    let mut i = from;
    for t in mobs.iter().skip(from).take(LISTING_PAGE_SIZE) {
        html.push_str(&format!(
            "<a action=\"bypass -h admin_spawn_monster {}\">{}</a><br1>",
            t.id, t.name
        ));
        i += 1;
    }
    html.push_str("<br><center>");
    if i < mobs.len() {
        html.push_str(&format!(
            "<button value=\"Next\" action=\"bypass -h {next_command} {i}\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">"
        ));
    }
    html.push_str(&format!(
        "<button value=\"Back\" action=\"bypass -h {back_command}\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>"
    ));
    html
}

/// `AdminSpawn.showMonsters` — the spawn menu's "Spawn by Level" **List** buttons
/// (`admin_spawn_index <level> [from]`). Lists every `Monster`-type NPC of that
/// exact level as `admin_spawn_monster <id>` links, 50 per page, with a `Next`
/// button carrying the running offset and a `Back` to the spawn menu. A missing
/// or non-numeric level falls back to `spawns.htm` (Java's `catch`).
pub(super) fn admin_spawn_index(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(level) = helpers::nth_arg::<i32>(args, 0) else {
        super::menu::show_admin_html(world, client_id, "spawns.htm");
        return;
    };
    let from = helpers::nth_arg::<usize>(args, 1).unwrap_or(0);
    let mobs = world.data.npc_data.monsters_of_level(level);
    let html = render_npc_listing(
        &format!("Level : {level}<br>Total NPCs : {}", mobs.len()),
        &mobs,
        from,
        &format!("admin_spawn_index {level}"),
        "admin_show_spawns",
    );
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
    let from = helpers::nth_arg::<usize>(args, 1).unwrap_or(0);
    let mobs = world.data.npc_data.folk_starting_with(starting);
    let html = render_npc_listing(
        &format!(
            "There are {} Npcs whose name starts with {starting}:",
            mobs.len()
        ),
        &mobs,
        from,
        &format!("admin_npc_index {starting}"),
        "admin_show_npcs",
    );
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
/// Java keys this off `SpawnTable.getSpawns(npcId)` — the **live spawn
/// objects**, one per NPC actually placed, each knowing both where it was
/// configured (`spawn.getX/Y/Z()`) and what currently stands there
/// (`getLastSpawn()`). This port walks the live NPCs of that id (`npcs_by_id`)
/// and reads the same pair off them: `Npc::spawn_loc` is the point the spawn
/// picked, the `Position` component is where the NPC is now.
///
/// It used to enumerate the *loaded spawn definitions* instead, and only those
/// with a fixed `<npc>` location — which is why it answered "No current spawns
/// found" for most of the world: the bulk of this dist spawns inside
/// `<territory>` polygons, where the definition carries no single point and the
/// location is chosen per NPC at spawn time.
///
/// One narrowing remains: a spawn whose NPC is dead and already decayed has no
/// entity here, so it is not listed, where Java's `Spawn` object outlives the
/// NPC and appears with a null `getLastSpawn()`. NPC-name search (Java
/// `getTemplateByName`) is not ported; like the other admin spawn commands this
/// takes a numeric id only.
pub(super) fn admin_list_spawns(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
    show_position: bool,
) {
    // Java takes an id **or a name**: it concatenates the middle words of the
    // command and, if that isn't a number, resolves it through
    // `NpcData.getTemplateByName`. A GM typing "Goblin" into the menu's box got
    // the usage line here instead of a list (GitHub #4).
    let tele_index = helpers::nth_arg::<i32>(args, 1);
    let name_words = if tele_index.is_some() {
        &args[..args.len() - 1]
    } else {
        args
    };
    let npc_id = match helpers::nth_arg::<i32>(args, 0) {
        Some(id) if name_words.len() <= 1 => Some(id),
        _ => {
            let name = name_words.join(" ");
            world.data.npc_data.get_by_name(name.trim()).map(|t| t.id)
        }
    };
    let Some(npc_id) = npc_id else {
        send_message(
            world,
            client_id,
            "Command format is //list_spawns <npcId|npc_name> [tele_index]",
        );
        return;
    };

    // One entry per live spawn of this id, which is Java's `SpawnTable` row:
    // the point the spawn was placed at, and where its NPC is standing now.
    let entries: Vec<(i32, i32, i32)> = world
        .npcs_by_id
        .get(&npc_id)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|oid| {
            let spawn_loc = world
                .objects
                .get_component::<Npc>(&oid)
                .map(|npc| npc.spawn_loc)?;
            // `showposition && (npc != null)` — the live position; otherwise the
            // spawn's own point.
            if show_position && let Some(now) = helpers::pos_of(world, oid) {
                return Some(now);
            }
            Some(spawn_loc)
        })
        .collect();

    if let Some(idx) = tele_index {
        let entry = if idx >= 1 {
            entries.get((idx - 1) as usize).copied()
        } else {
            None
        };
        match entry {
            Some((x, y, z)) => {
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
    let name = helpers::npc_template_name(world, npc_id);
    for (i, &(x, y, z)) in entries.iter().enumerate() {
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
    let top = helpers::nth_arg::<usize>(args, 0).unwrap_or(5).max(1);
    let mut counts: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();
    for oid in all_npc_ids(world) {
        if let Some(npc) = world.objects.get_component::<Npc>(&oid) {
            *counts.entry(npc.npc_id).or_default() += 1;
        }
    }
    let mut sorted: Vec<(i32, i32)> = counts.into_iter().collect();
    sorted.sort_by_key(|b| std::cmp::Reverse(b.1));
    send_message(world, client_id, &format!("=== Top {top} spawns ==="));
    for (npc_id, count) in sorted.into_iter().take(top) {
        let name = helpers::npc_template_name(world, npc_id);
        send_message(world, client_id, &format!("  {count} x {name} ({npc_id})"));
    }
}

/// `AdminSpawn`'s `//spawn_debug_print <type>` — dump the targeted NPC's id and
/// position (Java prints spawn/AI internals; text summary here).
pub(super) fn admin_spawn_debug_print(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) =
        target::current(world, object_id).filter(|oid| world.objects.has_component::<Npc>(oid))
    else {
        send_sm_bare_to_client(
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
    let name = helpers::npc_template_name(world, npc_id);
    let pos = maybe_position(world, target).unwrap_or(Position {
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

/// `AdminScan.DEFAULT_RADIUS` — `//scan` only lists NPCs this close (3D).
const SCAN_DEFAULT_RADIUS: i32 = 1000;
/// `PageBuilder.newBuilder(…, 15, …)` — scan rows per page.
const SCAN_PAGE_SIZE: usize = 15;

/// `AdminScan`'s `//scan` (`processBypass` + `sendNpcList`) — list the NPCs
/// within `radius` of the GM, 15 to a page.
///
/// The range is **3D** (`World.getVisibleObjectsInRange` measures
/// `calculateDistance3D`) and defaults to 1000: on a stacked map (Tower of
/// Insolence stairs, Cruma floors) the NPCs of the floors above/below are
/// horizontally on top of the GM but hundreds of z away, and it is exactly the
/// 3D metric that keeps them (and their hundreds of rows) out of the list.
/// The earlier port dumped every NPC of the 3×3 region block into one
/// unpaginated html — past `setHtml`'s 17 200-char clip, that dialog crashed
/// the client.
///
/// Bypass params (Java `BypassParser`): `id=` exact npc id, `name=` name
/// prefix (case-insensitive), `radius=`/`range=`, `page=`.
pub(super) fn admin_scan(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let id = bypass_param(args, "id")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);
    let name = bypass_param(args, "name").map(str::to_owned);
    let radius = bypass_param(args, "radius")
        .or_else(|| bypass_param(args, "range"))
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(SCAN_DEFAULT_RADIUS);
    // `PageBuilder.currentPage` clamps negatives to 0.
    let page = bypass_param(args, "page")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0)
        .max(0);

    let (Some(region), Some(gm_pos)) = (
        helpers::region_cell_of(world, object_id),
        maybe_position(world, object_id),
    ) else {
        return;
    };
    let gm_instance = crate::game_loop::helpers::instance_of(world, object_id);

    struct Row {
        oid: i32,
        npc_id: i32,
        name: String,
        x: i32,
        y: i32,
        z: i32,
        dist_2d: f64,
    }
    let mut list: Vec<Row> = Vec::new();
    for oid in world.npcs_visible_from(region) {
        let Some(npc) = world.objects.get_component::<Npc>(&oid) else {
            continue;
        };
        let Some(pos) = maybe_position(world, oid) else {
            continue;
        };
        let npc_id = npc.npc_id;
        if crate::game_loop::helpers::instance_of(world, oid) != gm_instance {
            continue;
        }
        if crate::geo::distance::dist3d_xyz(pos.x, pos.y, pos.z, gm_pos.x, gm_pos.y, gm_pos.z)
            > radius as f64
        {
            continue;
        }
        let tname = helpers::npc_template_name(world, npc_id);
        // `processBypass`'s condition: id beats name beats everything.
        if id > 0 {
            if npc_id != id {
                continue;
            }
        } else if let Some(n) = &name
            && !tname.to_lowercase().starts_with(&n.to_lowercase())
        {
            continue;
        }
        list.push(Row {
            oid,
            npc_id,
            name: tname,
            x: pos.x,
            y: pos.y,
            z: pos.z,
            dist_2d: crate::geo::distance::distance_2d_xy(pos.x, pos.y, gm_pos.x, gm_pos.y),
        });
    }

    // `createBypassBuilder`: the filter params survive paging and deletes.
    let mut filter_params = String::new();
    if id > 0 {
        filter_params.push_str(&format!(" id={id}"));
    } else if let Some(n) = &name {
        filter_params.push_str(&format!(" name={n}"));
    }
    if radius > SCAN_DEFAULT_RADIUS {
        filter_params.push_str(&format!(" radius={radius}"));
    }

    // `PageBuilder.build()`: pages = ceil(n / 15); the pager renders only past
    // one page; a `page` beyond the end is clamped to the last page.
    let pages = (list.len() / SCAN_PAGE_SIZE
        + usize::from(!list.len().is_multiple_of(SCAN_PAGE_SIZE))) as i32;
    let pager = if pages > 1 {
        next_prev_pager(
            &format!("bypass -h admin_scan{filter_params}"),
            page,
            pages,
            " page=",
        )
    } else {
        String::new()
    };
    let current = if page > pages { pages - 1 } else { page };
    let start = (SCAN_PAGE_SIZE as i32 * current).max(0) as usize;

    let mut rows = String::new();
    for row in list.iter().skip(start).take(SCAN_PAGE_SIZE) {
        let name = if row.name.is_empty() {
            "No name NPC"
        } else {
            &row.name
        };
        rows.push_str(&format!(
            "<tr><td width=\"45\">{}</td>\
             <td><a action=\"bypass -h admin_move_to {} {} {}\">{name}</a></td>\
             <td width=\"60\">{}</td>\
             <td width=\"54\"><a action=\"bypass -h admin_deleteNpcByObjectId{filter_params} page={page} objectId={}\"><font color=\"LEVEL\">Delete</font></a></td></tr>",
            row.npc_id,
            row.x,
            row.y,
            row.z,
            helpers::format_amount(row.dist_2d.round() as i64),
            row.oid,
        ));
    }

    // Java wraps the pager whenever the list is non-empty (`getPages() > 0`),
    // even when the pager itself stayed empty at a single page.
    let pages_html = if pages > 0 {
        format!("<center><table width=\"100%\" cellspacing=0><tr>{pager}</tr></table></center>")
    } else {
        String::new()
    };
    super::menu::show_admin_html_replace(
        world,
        client_id,
        "scan.htm",
        &[("data", rows), ("pages", pages_html)],
    );
}

/// `DefaultPageHandler` (offset 2) + `ButtonsStyle` — `PageBuilder`'s *default*
/// pager, the numbered strip `1 | 2 3 4 | 9 10`: the two pages either side of
/// the current one, plus the first two and last two when they fall outside that
/// window, and the current page as plain text rather than a button.
///
/// Java's `IBypassFormatter` here is `DefaultFormatter`, which appends
/// `" " + page` to the bypass.
pub(super) fn default_pager(bypass: &str, current: i32, pages: i32) -> String {
    const SEP: &str = "<td align=center> | </td>";
    const OFFSET: i32 = 2;
    let entry = |i: i32| -> String {
        if i == current {
            format!("<td>{}</td>", i + 1)
        } else {
            format!(
                "<td><button action=\"{bypass} {i}\" value=\"{}\" width=\"40\" height=\"15\" \
                 back=\"L2UI_CT1.Button_DF\" fore=\"L2UI_CT1.Button_DF\"></td>",
                i + 1
            )
        }
    };
    let pager_start = (current - OFFSET).max(0);
    let pager_finish = (current + OFFSET + 1).min(pages);
    let mut s = String::new();
    // The leading pages, once the window has moved past them.
    if pager_start > OFFSET {
        for i in 0..OFFSET {
            s.push_str(&entry(i));
        }
        s.push_str(SEP);
    }
    for i in pager_start..pager_finish {
        s.push_str(&entry(i));
    }
    // ...and the trailing ones, while the window has not reached them.
    if pages > pager_finish {
        s.push_str(SEP);
        for i in (pages - OFFSET).max(0)..pages {
            s.push_str(&entry(i));
        }
    }
    s
}

/// `NextPrevPageHandler` + `ButtonsStyle` — the
/// `First | Prev | Page: x/y | Next | Last` strip.
///
/// `page_prefix` is Java's `IBypassFormatter`: `DefaultFormatter` appends
/// `" " + page`, while the scan list reads a `page=<n>` bypass param.
///
/// **Two deliberate deviations from Java, because its pager renders broken on
/// this client:**
///   * Java labels the arrows `<<`/`<`/`>`/`>>`, and a *disabled* arrow is
///     emitted as bare text (`<td><<</td>`). The client parses that `<` as the
///     start of a tag and the strip falls apart, which is exactly how the
///     first page renders (both left arrows disabled). Word labels avoid the
///     escaping question entirely — no dist html has an angle bracket in a
///     button label, so there is no known-good escaping to copy.
///   * Java prints `pages + 1` as the total and points `>>` at index `pages`,
///     one past the last real page — so a 3-page list reads "Page: 1/4" and
///     the last click lands on an empty page. The count and the target are
///     the real ones here.
pub(super) fn next_prev_pager(bypass: &str, current: i32, pages: i32, page_prefix: &str) -> String {
    let last = (pages - 1).max(0);
    let button = |target: i32, label: &str, disabled: bool| -> String {
        if disabled {
            format!("<td align=center>{label}</td>")
        } else {
            format!(
                "<td><button action=\"{bypass}{page_prefix}{target}\" value=\"{label}\" \
                 width=\"40\" height=\"15\" back=\"L2UI_CT1.Button_DF\" fore=\"L2UI_CT1.Button_DF\"></td>"
            )
        }
    };
    const SEP: &str = "<td align=center> | </td>";
    let mut s = String::new();
    s.push_str(&button(0, "First", current <= 0));
    s.push_str(SEP);
    s.push_str(&button(current - 1, "Prev", current <= 0));
    s.push_str(SEP);
    s.push_str(&format!(
        "<td align=\"center\">Page: {}/{}</td>",
        current + 1,
        pages.max(1)
    ));
    s.push_str(SEP);
    s.push_str(&button(current + 1, "Next", current >= last));
    s.push_str(SEP);
    s.push_str(&button(last, "Last", current >= last));
    s
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
    let Some(region) = helpers::region_cell_of(world, target_oid)
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
    let name = helpers::npc_template_name(world, npc_id);
    super::death::despawn_npc(world, target_oid, region);
    send_message(world, client_id, &format!("{name} have been deleted."));
    // Java `processBypass` re-renders the scan list with the same parser —
    // the page and filter params ride along.
    admin_scan(world, client_id, object_id, args);
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
    let Some(id) = helpers::nth_arg::<i32>(args, 0) else {
        send_message(world, client_id, "Incorrect format for command 'summon'");
        return;
    };
    let count = helpers::nth_arg::<i64>(args, 1).unwrap_or(1);
    if id <= 0 || count <= 0 {
        return;
    }
    if id < 1_000_000 {
        items::give_item_with_earned_message(world, client_id, object_id, id, count);
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
    let Some(target) = target::current(world, object_id) else {
        send_message(world, client_id, "Select an NPC first.");
        return;
    };
    if !world.objects.has_component::<Npc>(&target) {
        send_message(world, client_id, "Target is not an NPC.");
        return;
    }
    super::death::despawn_npc_by_oid(world, target);
}

// ---------------------------------------------------------------------------
// `AdminSpawn` world-scale controls (category-4 sweep)
// ---------------------------------------------------------------------------

/// Despawn every live NPC (Java `//unspawnall`'s `deleteVisibleNpcSpawns`
/// sweep). Grand-boss lifecycle timers keep running; a following
/// `//respawnall` puts the world back.
pub(super) fn admin_unspawnall(world: &mut World, client_id: u32) {
    let all: Vec<(i32, (i32, i32))> = {
        let mut v = Vec::new();
        world
            .objects
            .for_each_mut::<(&Npc, &crate::model::components::RegionCell)>(|(n, r)| {
                v.push((n.object_id, r.0))
            });
        v
    };
    let count = all.len();
    for (oid, region) in all {
        super::death::despawn_npc(world, oid, region);
    }
    send_message(world, client_id, &format!("{count} NPCs deleted."));
}

/// `//respawnall` — clear the world and re-run the boot spawn pass.
///
/// The boot pass places NPCs without announcing them, because at boot there is
/// nobody to announce them to (Java's `Spawn.doSpawn` does both, and its own
/// startup runs before any player logs in). A GM running this on a live world
/// is the case where that matters: the NPCs were in the world but no
/// `NpcInfo` was ever sent, so they stayed invisible until the player walked
/// out of the region and back, which is what re-runs the visibility pass.
pub(super) fn admin_respawnall(world: &mut World, client_id: u32) {
    admin_unspawnall(world, client_id);
    let spawned = crate::game_loop::npc::spawn_all(world);
    // Java's `//respawnall` ends with `SpawnData.init()` **and**
    // `DBSpawnManager.load()`: the db-driven spawns come back too. The static
    // pass above deliberately defers those — raid bosses to
    // `pending_boss_spawns`, grand bosses to their own table — so without this
    // the command emptied the world of every boss until the next restart, which
    // is what made Orfen and Queen Ant vanish (GitHub #19).
    crate::game_loop::boss_respawn::resolve_boot(world, Vec::new());
    crate::game_loop::grand_boss::resolve_at_boot(world);
    for oid in npc_object_ids(world) {
        super::death::introduce_npc(world, oid);
    }
    send_message(world, client_id, &format!("{spawned} NPCs respawned."));
}

/// Every live NPC's object id. Collected first because `introduce_npc` needs
/// `&mut World` while the store would otherwise still be borrowed.
fn npc_object_ids(world: &mut World) -> Vec<i32> {
    let mut out = Vec::new();
    world
        .objects
        .for_each_mut::<&Npc>(|npc| out.push(npc.object_id));
    out
}

/// `//spawnnight` / `//spawnday` — the two buttons the shipped
/// `data/html/admin/spawn.htm` carries, which force the `DayNightSpawns` phase
/// instead of waiting for the in-game clock to reach it.
///
/// **Java ships the buttons and no handler.** `AdminSpawn`'s command list has
/// neither `admin_spawnnight` nor `admin_spawnday`, so clicking either one
/// there logs "couldn't find handler" and nothing happens. The buttons are in
/// this dist's own HTML, and the port has the phase swap the minute beat
/// already drives (`spawn_scripts::on_day_night_change`), so they are wired to
/// it rather than left dead — a deliberate step past Java, not a port of it.
///
/// The clock keeps running underneath: the next real phase flip
/// (`area_npcs::handle_day_night_check`) overrides whatever a GM forced here,
/// which is what makes this a "show me the other set" button rather than a
/// mode.
pub(super) fn admin_spawn_phase(world: &mut World, client_id: u32, night: bool) {
    crate::game_loop::spawn_scripts::on_day_night_change(world, night);
    send_message(
        world,
        client_id,
        if night {
            "Night spawns are up."
        } else {
            "Day spawns are up."
        },
    );
}

/// `//spawn_reload` — re-read `data/spawns/**` from disk, then respawn.
pub(super) fn admin_spawn_reload(world: &mut World, client_id: u32) {
    let root = world.data.root.clone();
    world.data.spawn_data = crate::data::SpawnData::load_from(&root);
    send_message(world, client_id, "Spawn data reloaded from disk.");
    admin_respawnall(world, client_id);
}

/// `AdminSpawn`'s `//instance_spawns <instance_id>` — the live NPCs of one
/// instance, first 50 with a "Go" link and the rest counted as skipped ("Only
/// 50 because of client html limitation", as Java's comment puts it).
///
/// Java's guard is `instance >= 300000`, which is where its `InstanceManager`
/// starts allocating ids. The port allocates from 1 and keeps **0** for the
/// shared overworld, so the same guard reads `> 0` here — it exists to reject
/// "not an instance", not to name a number.
pub(super) fn admin_instance_spawns(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(instance_id) = helpers::nth_arg::<i32>(args, 0) else {
        send_message(
            world,
            client_id,
            "Usage //instance_spawns <instance_number>",
        );
        return;
    };
    if instance_id <= 0 {
        send_message(world, client_id, "Invalid instance number.");
        return;
    }
    let Some(instance) = world.instances.get(instance_id) else {
        send_message(
            world,
            client_id,
            &format!("Cannot find instance {instance_id}"),
        );
        return;
    };
    // Java walks `inst.getNpcs()` and skips the dead — a corpse still in the
    // instance is not a spawn you can go to.
    let npcs: Vec<i32> = instance.npcs.clone();
    let mut rows = String::new();
    let (mut shown, mut skipped) = (0, 0);
    for npc_oid in npcs {
        if crate::game_loop::helpers::is_dead(world, npc_oid) {
            continue;
        }
        if shown >= 50 {
            skipped += 1;
            continue;
        }
        let Some(pos) = helpers::maybe_position(world, npc_oid) else {
            continue;
        };
        rows.push_str(&format!(
            "<tr><td>{}</td><td><a action=\"bypass -h admin_move_to {} {} {}\">Go</a></td></tr>",
            helpers::npc_name_or_empty(world, npc_oid),
            pos.x,
            pos.y,
            pos.z
        ));
        shown += 1;
    }
    let html = format!(
        "<html><table width=\"100%\"><tr>\
         <td width=45><button value=\"Main\" action=\"bypass admin_admin\" width=45 height=21 \
         back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td>\
         <td width=180><center><font color=\"LEVEL\">Spawns for {instance_id}</font></td>\
         <td width=45><button value=\"Back\" action=\"bypass -h admin_current_player\" width=45 \
         height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table><br>\
         <table width=\"100%\"><tr><td width=200>NpcName</td><td width=70>Action</td></tr>\
         {rows}<tr><td>Skipped:</td><td>{skipped}</td></tr></table></body></html>"
    );
    super::menu::send_admin_html_content(world, client_id, &html);
}
