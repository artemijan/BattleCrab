//! `AdminDebug`'s doors/geodata/movement visualizers — the three drawing
//! toggles of the GM Debug panel, drawn client-side with `ExServerPrimitive`
//! (FE:11). Each toggle runs a per-GM redraw loop (Java's fixed-rate tasks:
//! doors 3000 ms, geodata 1500 ms, movement 100 ms — 30/15/1 ticks here) that
//! re-renders when the GM has moved more than 15 units from the last frame's
//! anchor. Clearing re-sends each drawing's name with a single zero-length
//! black line at z −16000, Java's erase idiom.

use crate::game_loop::helpers::{send_message, send_to_client as send};
use crate::game_loop::space::position::{pos_of, region_cell_of};
use crate::model::components::space::DebugDraw;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;
const GREEN: u32 = 0x00FF00;
const RED: u32 = 0xFF0000;
const BLUE: u32 = 0x0000FF;
const BLACK: u32 = 0x000000;

const DOOR_TICKS: u64 = 30; // 3000 ms
const GEO_TICKS: u64 = 15; // 1500 ms
const MOVE_TICKS: u64 = 1; // 100 ms

/// A drawing's erase frame: one zero-length black line parked at z −16000.
fn erase(name: &str, x: i32, y: i32) -> Vec<u8> {
    server_packets::ex_server_primitive(
        name,
        x,
        y,
        -16000,
        &[(BLACK, (x, y, -16000), (x, y, -16000))],
    )
}

fn state_mut(world: &mut World, object_id: i32) -> &mut DebugDraw {
    if !world.objects.has_component::<DebugDraw>(&object_id) {
        world
            .objects
            .add_components(&object_id, DebugDraw::default());
    }
    world
        .objects
        .get_component_mut::<DebugDraw>(&object_id)
        .expect("just inserted")
}

pub(crate) fn flags(world: &World, object_id: i32) -> (bool, bool, bool) {
    world
        .objects
        .get_component::<DebugDraw>(&object_id)
        .map_or((false, false, false), |d| (d.doors, d.geo, d.movement))
}

fn moved_over(a: (i32, i32, i32), b: (i32, i32, i32), dist: i64) -> bool {
    let dx = (a.0 - b.0) as i64;
    let dy = (a.1 - b.1) as i64;
    dx * dx + dy * dy > dist * dist
}

// ---------------------------------------------------------------------------
// Doors (Java `AdminDoorControl.admin_showdoors` + `setDoorDebugging`)
// ---------------------------------------------------------------------------

/// Draw every door visible from the GM's region that isn't drawn yet: the
/// 12-line double-box + diagonals over the door's collision polygon, green
/// when open, red when closed.
fn draw_doors(world: &mut World, client_id: u32, object_id: i32) {
    let Some((px, py, pz)) = pos_of(world, object_id) else {
        return;
    };
    let Some(region) = region_cell_of(world, object_id) else {
        return;
    };
    let mut new_doors: Vec<(i32, Vec<u8>)> = Vec::new();
    {
        let shown = &state_mut(world, object_id).shown_doors.clone();
        for oid in world.doors_visible_from(region) {
            let Some(door) = world
                .objects
                .get_component::<crate::model::door::Door>(&oid)
            else {
                continue;
            };
            if shown.contains(&door.door_id) {
                continue;
            }
            let Some(t) = world.data.door_data.get(door.door_id) else {
                continue;
            };
            let open = world.geo.doors.is_open(door.door_id);
            let color = if open { GREEN } else { RED };
            let (zmin, zmax) = (t.node_z, t.node_z + t.height);
            let c = |i: usize| (t.node_x[i], t.node_y[i]);
            // Java's 12 lines: two boxes + diagonals over the polygon.
            let lines = vec![
                (color, (c(0).0, c(0).1, zmin), (c(1).0, c(1).1, zmin)),
                (color, (c(1).0, c(1).1, zmin), (c(2).0, c(2).1, zmax)),
                (color, (c(2).0, c(2).1, zmax), (c(3).0, c(3).1, zmax)),
                (color, (c(3).0, c(3).1, zmax), (c(0).0, c(0).1, zmin)),
                (color, (c(0).0, c(0).1, zmax), (c(1).0, c(1).1, zmax)),
                (color, (c(1).0, c(1).1, zmax), (c(2).0, c(2).1, zmin)),
                (color, (c(2).0, c(2).1, zmin), (c(3).0, c(3).1, zmin)),
                (color, (c(3).0, c(3).1, zmin), (c(0).0, c(0).1, zmax)),
                (color, (c(0).0, c(0).1, zmin), (c(1).0, c(1).1, zmax)),
                (color, (c(2).0, c(2).1, zmin), (c(3).0, c(3).1, zmax)),
                (color, (c(0).0, c(0).1, zmax), (c(1).0, c(1).1, zmin)),
                (color, (c(2).0, c(2).1, zmax), (c(3).0, c(3).1, zmin)),
            ];
            new_doors.push((
                door.door_id,
                server_packets::ex_server_primitive(
                    &format!("Door{}", door.door_id),
                    px,
                    py,
                    -16000,
                    &lines,
                ),
            ));
        }
    }
    for (door_id, pkt) in new_doors {
        send(world, client_id, pkt);
        send_message(world, client_id, &format!("Found door {door_id}"));
        state_mut(world, object_id).shown_doors.push(door_id);
    }
    state_mut(world, object_id).door_anchor = (px, py, pz);
}

fn clear_doors(world: &mut World, client_id: u32, object_id: i32) {
    let Some((px, py, _)) = pos_of(world, object_id) else {
        return;
    };
    let shown = std::mem::take(&mut state_mut(world, object_id).shown_doors);
    for door_id in shown {
        send(world, client_id, erase(&format!("Door{door_id}"), px, py));
    }
}

// ---------------------------------------------------------------------------
// Geodata grid (Java `GeoUtils.debugGrid` — radius 20 cells, 40 per packet)
// ---------------------------------------------------------------------------

const GEO_RADIUS: i32 = 20;
const CELLS_PER_PACKET: i32 = 40;

/// One NSWE arrow per direction and cell, green when passable, red when
/// blocked — the exact line offsets of Java's `debugGrid`.
fn draw_geo_grid(world: &mut World, client_id: u32, object_id: i32) {
    let Some((px, py, pz)) = pos_of(world, object_id) else {
        return;
    };
    let geo = world.geo.clone();
    let pgx = geo.get_geo_x(px);
    let pgy = geo.get_geo_y(py);
    let mut lines: Vec<(u32, (i32, i32, i32), (i32, i32, i32))> = Vec::new();
    let mut cell_count = 0;
    let mut packet_idx = 0;
    for dx in -GEO_RADIUS..=GEO_RADIUS {
        for dy in -GEO_RADIUS..=GEO_RADIUS {
            if cell_count >= CELLS_PER_PACKET {
                cell_count = 0;
                send(
                    world,
                    client_id,
                    server_packets::ex_server_primitive(
                        &format!("DebugGrid_{packet_idx}"),
                        px,
                        py,
                        -16000,
                        &lines,
                    ),
                );
                packet_idx += 1;
                lines.clear();
            }
            let gx = pgx + dx;
            let gy = pgy + dy;
            let x = geo.get_world_x(gx);
            let y = geo.get_world_y(gy);
            let z = geo.get_height(x, y, pz);
            let nswe = geo.nearest_nswe(gx, gy, pz);
            let dir = |mask: u8| if nswe & mask != 0 { GREEN } else { RED };
            // north arrow
            let col = dir(crate::geo::NSWE_NORTH);
            lines.push((col, (x - 1, y - 7, z), (x + 1, y - 7, z)));
            lines.push((col, (x - 2, y - 6, z), (x + 2, y - 6, z)));
            lines.push((col, (x - 3, y - 5, z), (x + 3, y - 5, z)));
            lines.push((col, (x - 4, y - 4, z), (x + 4, y - 4, z)));
            // east arrow
            let col = dir(crate::geo::NSWE_EAST);
            lines.push((col, (x + 7, y - 1, z), (x + 7, y + 1, z)));
            lines.push((col, (x + 6, y - 2, z), (x + 6, y + 2, z)));
            lines.push((col, (x + 5, y - 3, z), (x + 5, y + 3, z)));
            lines.push((col, (x + 4, y - 4, z), (x + 4, y + 4, z)));
            // south arrow
            let col = dir(crate::geo::NSWE_SOUTH);
            lines.push((col, (x - 1, y + 7, z), (x + 1, y + 7, z)));
            lines.push((col, (x - 2, y + 6, z), (x + 2, y + 6, z)));
            lines.push((col, (x - 3, y + 5, z), (x + 3, y + 5, z)));
            lines.push((col, (x - 4, y + 4, z), (x + 4, y + 4, z)));
            // west arrow
            let col = dir(crate::geo::NSWE_WEST);
            lines.push((col, (x - 7, y - 1, z), (x - 7, y + 1, z)));
            lines.push((col, (x - 6, y - 2, z), (x - 6, y + 2, z)));
            lines.push((col, (x - 5, y - 3, z), (x - 5, y + 3, z)));
            lines.push((col, (x - 4, y - 4, z), (x - 4, y + 4, z)));
            cell_count += 1;
        }
    }
    send(
        world,
        client_id,
        server_packets::ex_server_primitive(
            &format!("DebugGrid_{packet_idx}"),
            px,
            py,
            -16000,
            &lines,
        ),
    );
    state_mut(world, object_id).geo_anchor = (px, py, pz);
}

fn clear_geo_grid(world: &mut World, client_id: u32, object_id: i32) {
    let Some((px, py, _)) = pos_of(world, object_id) else {
        return;
    };
    // (radius diameter)² cells / 40 per packet, same numbering as the draw.
    let total = (2 * GEO_RADIUS + 1) * (2 * GEO_RADIUS + 1);
    let packets = total / CELLS_PER_PACKET + 1;
    for i in 0..packets {
        send(world, client_id, erase(&format!("DebugGrid_{i}"), px, py));
    }
}

/// `AdminGeodata`'s `//geogrid [off]` — Java's one-shot `GeoUtils.debugGrid` /
/// `hideDebugGrid`, the same 41×41 NSWE arrow grid the Debug panel draws, but
/// drawn once: no redraw loop is armed and the panel's `geodata` flag is left
/// alone. That is Java's own layering — `AdminDebug.setGeodataDebugging` runs
/// its refresh by re-issuing `admin_geogrid` / `admin_geogrid off` on a timer,
/// so the two share one renderer and one set of `DebugGrid_<n>` drawing names.
pub(super) fn admin_geogrid(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    // Java: any argument other than "off" (and no argument at all) draws.
    if args.first().is_some_and(|a| a.eq_ignore_ascii_case("off")) {
        clear_geo_grid(world, client_id, object_id);
    } else {
        draw_geo_grid(world, client_id, object_id);
    }
}

// ---------------------------------------------------------------------------
// Movement line (Java `drawMoveLine`/`clearMoveLine`)
// ---------------------------------------------------------------------------

fn draw_move_line(world: &mut World, client_id: u32, object_id: i32) {
    let Some((px, py, pz)) = pos_of(world, object_id) else {
        return;
    };
    let mv = world
        .objects
        .get_component::<crate::model::components::space::Movement>(&object_id)
        .map(|m| {
            (
                (m.0.dest_x, m.0.dest_y, m.0.dest_z),
                m.0.geo_path.as_ref().map(|g| g.points.clone()),
            )
        });
    let Some((dest, path)) = mv else {
        // Not moving — just move the anchor (Java's early return).
        state_mut(world, object_id).move_anchor = (px, py, pz);
        return;
    };
    let mut lines = vec![(GREEN, (px, py, pz), dest)];
    if let Some(points) = &path {
        for pair in points.windows(2) {
            lines.push((BLUE, pair[0], pair[1]));
        }
        let st = state_mut(world, object_id);
        if st.last_path.as_ref() == Some(points) {
            return;
        }
        st.last_path = Some(points.clone());
        st.move_anchor = (px, py, pz);
    } else {
        let st = state_mut(world, object_id);
        if st.last_dest.is_some_and(|prev| !moved_over(prev, dest, 15)) {
            return;
        }
        st.last_dest = Some(dest);
        st.last_path = None;
        st.move_anchor = (px, py, pz);
    }
    send(
        world,
        client_id,
        server_packets::ex_server_primitive("DebugMove", px, py, -16000, &lines),
    );
}

fn clear_move_line(world: &mut World, client_id: u32, object_id: i32) {
    let Some((px, py, _)) = pos_of(world, object_id) else {
        return;
    };
    send(world, client_id, erase("DebugMove", px, py));
    let st = state_mut(world, object_id);
    st.last_dest = None;
    st.last_path = None;
}

// ---------------------------------------------------------------------------
// Toggles + ticks
// ---------------------------------------------------------------------------

/// `//debug doors|geodata|movement [on|off]` — flip one visualizer.
pub(super) fn set_debug(world: &mut World, client_id: u32, object_id: i32, kind: &str, on: bool) {
    match kind {
        "doors" => {
            let was = state_mut(world, object_id).doors;
            state_mut(world, object_id).doors = on;
            if on && !was {
                draw_doors(world, client_id, object_id);
                world.scheduler.schedule(
                    world.tick + DOOR_TICKS,
                    ScheduledTask::DebugDoorTick { object_id },
                );
            } else if !on && was {
                clear_doors(world, client_id, object_id);
            }
            send_message(
                world,
                client_id,
                &format!(
                    "Door debugging is {}",
                    if on { "enabled." } else { "disabled." }
                ),
            );
        }
        "geodata" => {
            let was = state_mut(world, object_id).geo;
            state_mut(world, object_id).geo = on;
            if on && !was {
                draw_geo_grid(world, client_id, object_id);
                world.scheduler.schedule(
                    world.tick + GEO_TICKS,
                    ScheduledTask::DebugGeoTick { object_id },
                );
            } else if !on && was {
                clear_geo_grid(world, client_id, object_id);
            }
            send_message(
                world,
                client_id,
                &format!(
                    "Geodata debugging is {}",
                    if on { "enabled." } else { "disabled." }
                ),
            );
        }
        "movement" => {
            let was = state_mut(world, object_id).movement;
            state_mut(world, object_id).movement = on;
            if on && !was {
                draw_move_line(world, client_id, object_id);
                world.scheduler.schedule(
                    world.tick + MOVE_TICKS,
                    ScheduledTask::DebugMoveTick { object_id },
                );
            } else if !on && was {
                clear_move_line(world, client_id, object_id);
            }
            send_message(
                world,
                client_id,
                &format!(
                    "Movement debugging is {}",
                    if on { "enabled." } else { "disabled." }
                ),
            );
        }
        _ => {}
    }
}

/// The client to draw into, or `None` once the GM has left the world — the
/// gate every redraw beat below opens with (Java's `isOnline` branch).
fn tick_client(world: &World, object_id: i32) -> Option<u32> {
    super::helpers::client_for_player(world, object_id)
}

// The three per-kind redraw beats. Each arms the next one only while its own
// flag is on and the player is still in world, so flipping the flag off or
// logging out ends the chain by simply not rescheduling. The redraw itself is
// skipped until the GM has moved far enough from where the last one was drawn.

pub(crate) fn door_tick(world: &mut World, object_id: i32) {
    let (doors, ..) = flags(world, object_id);
    let Some(client_id) = tick_client(world, object_id) else {
        return;
    };
    if !doors {
        return;
    }
    let anchor = state_mut(world, object_id).door_anchor;
    if pos_of(world, object_id).is_some_and(|p| moved_over(p, anchor, 15)) {
        draw_doors(world, client_id, object_id);
    }
    world.scheduler.schedule(
        world.tick + DOOR_TICKS,
        ScheduledTask::DebugDoorTick { object_id },
    );
}

pub(crate) fn geo_tick(world: &mut World, object_id: i32) {
    let (_, geo, movement) = flags(world, object_id);
    let Some(client_id) = tick_client(world, object_id) else {
        return;
    };
    if !geo {
        return;
    }
    // Java skips the geo redraw while the movement debugger holds a path.
    let anchor = state_mut(world, object_id).geo_anchor;
    if !movement && pos_of(world, object_id).is_some_and(|p| moved_over(p, anchor, 15)) {
        draw_geo_grid(world, client_id, object_id);
    }
    world.scheduler.schedule(
        world.tick + GEO_TICKS,
        ScheduledTask::DebugGeoTick { object_id },
    );
}

pub(crate) fn move_tick(world: &mut World, object_id: i32) {
    let (.., movement) = flags(world, object_id);
    let Some(client_id) = tick_client(world, object_id) else {
        return;
    };
    if !movement {
        return;
    }
    let anchor = state_mut(world, object_id).move_anchor;
    if pos_of(world, object_id).is_some_and(|p| moved_over(p, anchor, 15)) {
        draw_move_line(world, client_id, object_id);
    } else if state_mut(world, object_id).last_dest.is_some()
        || state_mut(world, object_id).last_path.is_some()
    {
        clear_move_line(world, client_id, object_id);
    }
    world.scheduler.schedule(
        world.tick + MOVE_TICKS,
        ScheduledTask::DebugMoveTick { object_id },
    );
}
