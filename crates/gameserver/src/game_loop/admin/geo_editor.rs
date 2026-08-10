//! `AdminGeodata`'s geo *editor* — the half of the handler that changes
//! geodata rather than reporting it: the community-board cell panels
//! (`//geoedit`, `//ge`), the NSWE passability edits their buttons fire
//! (`//geoenable*`/`//geodisable*` and Java's `//en`/`//dn`/`//es`/… aliases)
//! and the region export (`//geosave`, `//geosaveall`).
//!
//! The panels are drawn from the GM's heading, so "north" on screen is always
//! the direction they face: `//geoedit` rotates a 19×19 button grid of geo
//! cells, `//ge <geoX> <geoY>` rotates the four-arrow editor for one cell.
//! Every button routes back through the short alias with explicit cell coords,
//! which is why the aliases exist at all.
//!
//! Edits themselves live in the engine's override map (the region files stay
//! mmap'd read-only, unlike Java's mutable block objects); `//geosave` folds
//! them back into the on-disk format — see [`crate::geo::region::Region::write_to`].

use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::nth_arg;
use std::path::Path;

use crate::game_loop::community_board::send_cb_html;
use crate::geo;
use crate::network::server_packets;
use crate::world::World;

use super::send_message;

/// Java `AdminGeodata.getPlayerDirection`: 0 = north, 1 = east, 2 = south,
/// 3 = west, from the GM's heading.
fn player_direction(heading: i32) -> u8 {
    match heading {
        h if !(8192..=57344).contains(&h) => 0,
        h if h < 24576 => 1,
        h if h < 40960 => 2,
        _ => 3,
    }
}

/// Java's `%N%/%E%/%S%/%W%` rotation: the compass letter to print in each
/// screen slot for a GM facing `direction`.
fn compass(direction: u8) -> [&'static str; 4] {
    match direction {
        0 => ["N", "E", "S", "W"],
        1 => ["E", "S", "W", "N"],
        2 => ["S", "W", "N", "E"],
        _ => ["W", "N", "E", "S"],
    }
}

/// Java's per-direction `(translatedDx, translatedDy)` mapping: which screen
/// offset a world-cell offset appears at.
fn rotate(direction: u8, dx: i32, dy: i32) -> (i32, i32) {
    match direction {
        0 => (dx, dy),
        1 => (dy, -dx),
        2 => (-dx, -dy),
        _ => (-dy, dx),
    }
}

const PASSABLE_BG: &str = "L2UI_CH3.minibar_food";
const BLOCKED_BG: &str = "L2UI_CH3.minibar_arrow";

fn read_admin_htm(world: &World, name: &str) -> Option<String> {
    crate::data::htm_cache::read_htm(format!("{}data/html/admin/{name}", world.data.root))
}

/// `//geoedit` — the 19×19 cell grid (Java's `geoRadius = 9`), each button a
/// `//ge <geoX> <geoY>` into the single-cell editor, each background telling
/// whether that cell is open in all four directions.
pub(super) fn admin_geoedit(world: &mut World, client_id: u32, object_id: i32) {
    let Some(pos) = maybe_position(world, object_id) else {
        return;
    };
    let Some(mut content) = read_admin_htm(world, "geoedit.htm") else {
        send_message(world, client_id, "Missing data/html/admin/geoedit.htm.");
        return;
    };
    let direction = player_direction(pos.heading);
    for (token, letter) in ["%N%", "%E%", "%S%", "%W%"].iter().zip(compass(direction)) {
        content = content.replace(token, letter);
    }

    const GEO_RADIUS: i32 = 9;
    let geo = &world.geo;
    let (gx0, gy0) = (geo.get_geo_x(pos.x), geo.get_geo_y(pos.y));
    for dx in -GEO_RADIUS..=GEO_RADIUS {
        for dy in -GEO_RADIUS..=GEO_RADIUS {
            let (tdx, tdy) = rotate(direction, dx, dy);
            let (gx, gy) = (gx0 + dx, gy0 + dy);
            content = content.replace(&format!("xy_{tdx}_{tdy}"), &format!("{gx} {gy}"));
            let z = geo.get_nearest_z(gx, gy, pos.z);
            let open = geo.check_nearest_nswe(gx, gy, z, geo::NSWE_ALL);
            content = content.replace(
                &format!("bg_{tdx}_{tdy}"),
                if open { PASSABLE_BG } else { BLOCKED_BG },
            );
        }
    }
    send_cb_html(world, client_id, &content);
}

/// `//ge [geoX geoY]` — the single-cell editor: four buttons, one per
/// direction, green when the cell may be exited that way (the button then
/// *disables* it) and red when it may not (the button enables it). Bare `//ge`
/// falls back to the grid, as Java does.
pub(super) fn admin_ge(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    if args.is_empty() {
        admin_geoedit(world, client_id, object_id);
        return;
    }
    let (Some(gx), Some(gy)) = (nth_arg::<i32>(args, 0), nth_arg::<i32>(args, 1)) else {
        send_message(world, client_id, "Usage: //ge <geoX> <geoY>");
        return;
    };
    let Some(pos) = maybe_position(world, object_id) else {
        return;
    };
    let Some(mut content) = read_admin_htm(world, "geoedit_cell.htm") else {
        send_message(
            world,
            client_id,
            "Missing data/html/admin/geoedit_cell.htm.",
        );
        return;
    };

    // Java rotates the four `%bg_*%`/`%cmd_*%` slots with the heading, so the
    // button the GM sees "north" of the cell edits the cell's north side as
    // they face it. North needs no mapping — it just drops every `%`.
    let direction = player_direction(pos.heading);
    if direction == 0 {
        content = content.replace('%', "");
    } else {
        let letters = compass(direction);
        for (token, letter) in ["%N%", "%E%", "%S%", "%W%"].iter().zip(letters) {
            content = content.replace(token, letter);
        }
        // Same rotation over the slot names: N takes E's slot for a GM facing
        // east, and so on. Each replacement writes a `%`-less name, so later
        // passes cannot re-match what an earlier one produced.
        let rotated = match direction {
            1 => ["e", "s", "w", "n"],
            2 => ["s", "w", "n", "e"],
            _ => ["w", "n", "e", "s"],
        };
        for (slot, target) in ["n", "e", "s", "w"].iter().zip(rotated) {
            content = content.replace(&format!("%bg_{slot}%"), &format!("bg_{target}"));
            content = content.replace(&format!("%cmd_{slot}%"), &format!("cmd_{target}"));
        }
    }

    let z = world.geo.get_nearest_z(gx, gy, pos.z);
    for (slot, bit, enable, disable) in [
        ("n", geo::NSWE_NORTH, "en", "dn"),
        ("e", geo::NSWE_EAST, "ee", "de"),
        ("s", geo::NSWE_SOUTH, "es", "ds"),
        ("w", geo::NSWE_WEST, "ew", "dw"),
    ] {
        let open = world.geo.check_nearest_nswe(gx, gy, z, bit);
        let (bg, cmd) = if open {
            (PASSABLE_BG, disable)
        } else {
            (BLOCKED_BG, enable)
        };
        content = content.replace(&format!("bg_{slot}"), bg);
        content = content.replace(&format!("cmd_{slot}"), &format!("{cmd} {gx} {gy}"));
    }
    send_cb_html(world, client_id, &content);
}

/// `//geoenable<dir>` / `//geodisable<dir>` and their `//en`/`//dn`/… aliases —
/// set or clear one NSWE passability bit (Java `setNearestNswe` /
/// `unsetNearestNswe`). The cell is the GM's own unless the command carries
/// `<geoX> <geoY>`, which is how the `//ge` panel's buttons address it; those
/// short forms then re-open the panel on the edited cell, so the GM sees the
/// arrow flip. The edit layers over the immutable base geodata and takes
/// effect immediately for movement/pathfinding.
pub(super) fn admin_geo_nswe(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    nswe: u8,
    enable: bool,
    args: &[&str],
    reopen_panel: bool,
) {
    let Some(pos) = maybe_position(world, object_id) else {
        return;
    };
    // Java reads the pair as `geoX geoY`; a partial pair falls back to the
    // GM's own cell, as `hasMoreTokens()` does there.
    let (gx, gy) = match (nth_arg::<i32>(args, 0), nth_arg::<i32>(args, 1)) {
        (Some(x), Some(y)) => (x, y),
        // NOTE: Java's `admin_geodisablenorth` branch reads its default geoX
        // through `getGeoY(getX())` — a copy-paste slip that would edit a cell
        // 4096 columns away. Every other branch uses `getGeoX`, and the panel
        // always passes explicit coords, so the slip is only reachable by
        // typing `//geodisablenorth` bare; ported as the correct `getGeoX`.
        _ => (world.geo.get_geo_x(pos.x), world.geo.get_geo_y(pos.y)),
    };
    if !world.geo.has_geo_pos(gx, gy) {
        send_message(world, client_id, "There is no geodata at this position.");
        return;
    }
    if enable {
        world.geo.set_nearest_nswe(gx, gy, pos.z, nswe);
    } else {
        world.geo.unset_nearest_nswe(gx, gy, pos.z, nswe);
    }
    if reopen_panel {
        admin_ge(
            world,
            client_id,
            object_id,
            &[&gx.to_string(), &gy.to_string()],
        );
    } else {
        send_message(
            world,
            client_id,
            &format!(
                "Cell {gx},{gy}: {} {}.",
                dir_name(nswe),
                if enable { "enabled" } else { "disabled" }
            ),
        );
    }
}

fn dir_name(nswe: u8) -> &'static str {
    match nswe {
        geo::NSWE_NORTH => "north",
        geo::NSWE_SOUTH => "south",
        geo::NSWE_EAST => "east",
        _ => "west",
    }
}

// ---------------------------------------------------------------------------
// Region export (`Region.saveToFile`)
// ---------------------------------------------------------------------------

/// Java creates `Config.GEOEDIT_PATH` up front and bails out of the command if
/// it cannot.
fn geoedit_dir(world: &World, client_id: u32) -> Option<std::path::PathBuf> {
    let dir = std::path::PathBuf::from(&world.geoedit_path);
    if std::fs::create_dir_all(&dir).is_err() {
        send_message(world, client_id, "Could not create output directory.");
        return None;
    }
    Some(dir)
}

/// `//geosave` — write the region the GM is standing in, runtime NSWE edits
/// folded into the cells they changed.
pub(super) fn admin_geosave(world: &mut World, client_id: u32, object_id: i32) {
    let Some(pos) = maybe_position(world, object_id) else {
        return;
    };
    let Some(dir) = geoedit_dir(world, client_id) else {
        return;
    };
    let ((tx, ty), ..) = world.geo.geomap_tile(pos.x, pos.y);
    let message = save_one(&world.geo, tx, ty, &dir);
    send_message(world, client_id, &message);
}

/// One tile's save plus the line Java reports for it.
fn save_one(geo: &geo::GeoEngine, tile_x: i32, tile_y: i32, dir: &Path) -> String {
    let name = format!("{tile_x}_{tile_y}.l2j");
    match geo.save_region(tile_x, tile_y, dir) {
        None => format!("Could not find region: {tile_x}_{tile_y}"),
        Some(true) => format!("Saved region {tile_x}_{tile_y} at {name}"),
        Some(false) => format!("Could not save region {tile_x}_{tile_y}"),
    }
}

/// `//geosaveall` — every region that has geodata. Java writes them on the
/// caller's thread; here that is the game loop, and the dist's geodata is
/// ~1.4 GB, so the sweep runs on a worker thread (the region bytes are an
/// immutable shared mmap and the edit map is behind an `RwLock`, so nothing
/// else has to stop) and streams Java's per-region lines back as they land.
pub(super) fn admin_geosaveall(world: &mut World, client_id: u32) {
    let Some(dir) = geoedit_dir(world, client_id) else {
        return;
    };
    let Some(out) = world.clients.get(&client_id).map(|cs| cs.outbound()) else {
        return;
    };
    let geo = world.geo.clone();
    let tiles = geo.loaded_tiles();
    send_message(
        world,
        client_id,
        &format!("Saving {} regions in the background...", tiles.len()),
    );
    std::thread::spawn(move || {
        let say = |text: String| {
            out.send(bytes::Bytes::from(server_packets::system_message_with(
                server_packets::sm_ids::S1_TEXT,
                &[server_packets::SmParam::Text(text)],
            )));
        };
        let mut count = 0;
        for (tile_x, tile_y) in tiles {
            let message = save_one(&geo, tile_x, tile_y, &dir);
            if message.starts_with("Saved") {
                count += 1;
            }
            say(message);
        }
        say(format!("Saved {count} regions."));
    });
}
