//! `AdminGeodata`'s editor half — the heading-rotated cell panels
//! (`//geoedit`, `//ge`), the NSWE edits their buttons fire (long forms and
//! Java's `//en`/`//dn`/… aliases), the region export (`//geosave`) and
//! `AdminPathNode`'s `//path_find`. Driven end-to-end through the `//command`
//! bar against the real dist htmls.

use super::*;

use crate::geo::{NSWE_ALL, NSWE_NORTH, synthetic_region};
use crate::model::components::{Position, TargetRef};

const DIST: &str = crate::data::DIST_GAME;

/// Region 20_18 covers world x,y ∈ [0, 32768): flat-ish ground at z = 0, with
/// local cell column x == 10 walled off (no exits at all), so the panels have
/// both open and blocked cells to render.
fn install_region(world: &mut World) {
    std::sync::Arc::get_mut(&mut world.geo)
        .expect("geo Arc not shared yet")
        .set_region(
            20,
            18,
            synthetic_region(|x, _y| if x == 10 { (0, 0) } else { (0, NSWE_ALL) }),
        );
}

/// A GM with geodata under them and the dist htmls reachable.
fn geo_world(
    heading: i32,
) -> (
    World,
    tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
    (i32, i32),
) {
    let (mut world, ..) = admin_world();
    world.data.root = DIST.to_string();
    install_region(&mut world);
    let rx = ingame_player_access(&mut world, 1, 7970, 100);
    {
        let pos = world.objects.get_component_mut::<Position>(&7970).unwrap();
        pos.x = 100;
        pos.y = 100;
        pos.z = 0;
        pos.heading = heading;
    }
    let cell = (world.geo.get_geo_x(100), world.geo.get_geo_y(100));
    (world, rx, cell)
}

/// The reassembled html of a `sendCBHtml` batch: each `ShowBoard` carries
/// `"<chunk id>\u{8}<part>"` after the eight nav strings, and the client
/// concatenates the parts in 101/102/103 order (`"null"` = unused chunk).
fn board_html(pkts: &[Vec<u8>]) -> String {
    pkts.iter()
        .filter(|p| p[0] == server_packets::opcodes::SHOW_BOARD)
        .filter_map(|p| {
            let mut r = commons::network::PacketReader::new(&p[2..]);
            for _ in 0..8 {
                r.read_string().unwrap();
            }
            let content = r.read_string().unwrap_or_default();
            let part = content
                .split_once('\u{8}')
                .map(|(_, part)| part.to_string());
            part.filter(|part| part != "null")
        })
        .collect()
}

/// **`//geoedit` resolves every button to a real geo cell.** Java replaces the
/// `xy_dx_dy` / `bg_dx_dy` tokens of `geoedit.htm` in place, so a leftover
/// token means that button would edit nothing.
#[test]
fn geoedit_panel_resolves_every_cell_button() {
    let (mut world, mut rx, (gx, gy)) = geo_world(0);
    drain(&mut rx);

    on_packet(&mut world, 1, build_admin("geoedit"));
    let html = board_html(&drain(&mut rx));

    assert!(
        !html.is_empty(),
        "the panel is sent as a community board html"
    );
    assert!(
        html.contains(&format!("admin_ge {gx} {gy}")),
        "the centre button edits the GM's own cell"
    );
    assert!(
        !html.contains("xy_") && !html.contains("bg_"),
        "no unresolved grid tokens left"
    );
    assert!(
        !html.contains("%N%") && !html.contains("%W%"),
        "compass tokens replaced"
    );
    // The walled column (local x == 10 → geo x 20*2048 + 10) renders blocked,
    // everything else open.
    assert!(
        html.contains("L2UI_CH3.minibar_food"),
        "open cells use the passable background"
    );
}

/// The `(screen dx, dy) → (geoX, geoY)` map of a rendered `//geoedit` panel.
/// The file's buttons carry their `xy_dx_dy` tokens in a fixed document order,
/// so zipping that order with the rendered values says which cell each *slot*
/// addresses — the thing the heading rotation actually decides. (Asserting
/// that a cell appears *somewhere* proves nothing: all 209 buttons resolve to
/// the same set of cells whatever the rotation.)
fn slot_cells(rendered: &str) -> std::collections::HashMap<(i32, i32), (i32, i32)> {
    let raw = crate::data::htm_cache::read_htm(format!("{DIST}data/html/admin/geoedit.htm"))
        .expect("geoedit.htm");
    let pair = |s: &str, sep: char| -> (i32, i32) {
        let (a, b) = s.split_once(sep).expect("two coordinates");
        (a.parse().expect("dx"), b.parse().expect("dy"))
    };
    let slots: Vec<(i32, i32)> = raw
        .split("admin_ge xy_")
        .skip(1)
        .map(|s| pair(&s[..s.find('"').expect("quoted")], '_'))
        .collect();
    let cells: Vec<(i32, i32)> = rendered
        .split("admin_ge ")
        .skip(1)
        .map(|s| pair(&s[..s.find('"').expect("quoted")], ' '))
        .collect();
    assert_eq!(slots.len(), cells.len(), "every button resolved to a cell");
    slots.into_iter().zip(cells).collect()
}

/// **The grid follows the GM's heading.** Java rotates world offsets into
/// screen offsets so "up" is always the way the GM faces; facing east, the
/// button one row up addresses the cell to the *east*, not the north.
#[test]
fn geoedit_panel_rotates_with_heading() {
    let (mut world, mut rx, (gx, gy)) = geo_world(0);
    drain(&mut rx);
    on_packet(&mut world, 1, build_admin("geoedit"));
    let facing_north = board_html(&drain(&mut rx));

    // Heading 16384 = due east (Java's `getPlayerDirection` == 1).
    world
        .objects
        .get_component_mut::<Position>(&7970)
        .unwrap()
        .heading = 16384;
    on_packet(&mut world, 1, build_admin("geoedit"));
    let facing_east = board_html(&drain(&mut rx));

    let north = slot_cells(&facing_north);
    let east = slot_cells(&facing_east);
    assert_eq!(north[&(0, 0)], (gx, gy), "the centre slot is the GM's cell");
    assert_eq!(east[&(0, 0)], (gx, gy), "…whichever way they face");
    assert_eq!(
        north[&(0, -1)],
        (gx, gy - 1),
        "facing north, the slot above centre is the cell to the north"
    );
    assert_eq!(
        east[&(0, -1)],
        (gx + 1, gy),
        "facing east, that same slot is the cell to the east"
    );
    assert_eq!(
        east[&(1, 0)],
        (gx, gy + 1),
        "and the slot right of centre is the cell to the south"
    );
    assert!(
        facing_north.contains("> N </font>") && facing_east.contains("> E </font>"),
        "the compass letter at the top of the grid is the way the GM faces"
    );
}

/// **The cell panel offers the inverse of the current state, and its buttons
/// work.** An open side gets the *disable* alias; after firing it the edit is
/// live in the engine and the panel comes back showing the *enable* alias —
/// Java's `admin_ge` re-open at the end of every short-alias edit.
#[test]
fn cell_panel_buttons_edit_and_reopen() {
    let (mut world, mut rx, (gx, gy)) = geo_world(0);
    drain(&mut rx);

    on_packet(&mut world, 1, build_admin(&format!("ge {gx} {gy}")));
    let html = board_html(&drain(&mut rx));
    assert!(
        html.contains(&format!("admin_dn {gx} {gy}")),
        "an open north side offers to disable it"
    );
    assert!(
        !html.contains("%bg_n%") && !html.contains("cmd_n"),
        "tokens resolved"
    );

    // Fire the button the panel just drew.
    on_packet(&mut world, 1, build_admin(&format!("dn {gx} {gy}")));
    let after = drain(&mut rx);
    assert!(
        !world.geo.check_nearest_nswe(gx, gy, 0, NSWE_NORTH),
        "the cell lost its north exit"
    );
    let reopened = board_html(&after);
    assert!(
        reopened.contains(&format!("admin_en {gx} {gy}")),
        "the panel re-opens offering to enable it again"
    );
}

/// **The long forms edit the GM's own cell and report in text.** Java only
/// re-opens the panel for the short aliases (`!actualCommand.contains("geo")`),
/// so `//geodisablenorth` typed in the command bar stays a chat-level tool.
#[test]
fn long_form_edits_own_cell_without_the_panel() {
    let (mut world, mut rx, (gx, gy)) = geo_world(0);
    drain(&mut rx);

    on_packet(&mut world, 1, build_admin("geodisablenorth"));
    let pkts = drain(&mut rx);
    assert!(
        !world.geo.check_nearest_nswe(gx, gy, 0, NSWE_NORTH),
        "the GM's own cell is the default target"
    );
    assert!(
        pkts.iter()
            .all(|p| p[0] != server_packets::opcodes::SHOW_BOARD),
        "no panel for the long form"
    );
    assert!(
        pkts.iter().any(|p| contains_utf16(p, "north disabled")),
        "the edit is reported in chat"
    );

    // And it takes an explicit cell like Java's optional geoX/geoY pair.
    on_packet(
        &mut world,
        1,
        build_admin(&format!("geodisableeast {} {gy}", gx + 5)),
    );
    assert!(
        !world
            .geo
            .check_nearest_nswe(gx + 5, gy, 0, crate::geo::NSWE_EAST),
        "explicit coordinates are honoured"
    );
}

/// **`//geosave` writes a real region file with the runtime edits in it.**
/// Java mutates its blocks and dumps them; here the edits live in the engine's
/// override map, so the proof is reloading the written file and finding the
/// edit — and finding the untouched cells unchanged.
#[test]
fn geosave_writes_the_edited_region() {
    let (mut world, mut rx, (gx, gy)) = geo_world(0);
    let dir = std::env::temp_dir().join(format!("l2r-geosave-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    world.geoedit_path = dir.to_string_lossy().to_string();
    drain(&mut rx);

    on_packet(&mut world, 1, build_admin("geodisablenorth"));
    drain(&mut rx);
    on_packet(&mut world, 1, build_admin("geosave"));
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| contains_utf16(p, "Saved region 20_18 at 20_18.l2j")),
        "Java's success line names the tile and file"
    );

    let written = crate::geo::region::Region::load(&dir.join("20_18.l2j"))
        .expect("the exported region reloads");
    let (local_x, local_y) = (gx % 2048, gy % 2048);
    assert!(
        !written.check_nearest_nswe(local_x, local_y, 0, NSWE_NORTH),
        "the edit survived the round trip through the file"
    );
    assert!(
        written.check_nearest_nswe(local_x, local_y + 1, 0, NSWE_ALL),
        "the neighbouring cell is untouched"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// **`//path_find` dumps the route the cell pathfinder returns**, and answers
/// Java's two refusals: no target, and pathfinding switched off in
/// `GeoEngine.ini`.
#[test]
fn path_find_reports_route_target_and_config() {
    let (mut world, mut rx, _) = geo_world(0);
    drain(&mut rx);

    on_packet(&mut world, 1, build_admin("path_find"));
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| contains_utf16(p, "No Target!")),
        "Java refuses without a target"
    );

    // Target a second player a few cells east; the route is a straight walk.
    let _victim = ingame_player(&mut world, 2, 7971, 500, 100, 0);
    world.objects.add_components(&7970, TargetRef(Some(7971)));
    on_packet(&mut world, 1, build_admin("path_find"));
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter().any(|p| contains_utf16(p, "x:")),
        "each node of the route is printed"
    );
    assert!(
        !pkts.iter().any(|p| contains_utf16(p, "No Route!")),
        "a clear walk finds a route"
    );

    world.path_finding = 0;
    on_packet(&mut world, 1, build_admin("path_find"));
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| contains_utf16(p, "PathFinding is disabled.")),
        "Config.PATHFINDING < 1 short-circuits"
    );
}
