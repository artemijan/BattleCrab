//! World-feature commands — `AdminDoorControl` (open/close doors),
//! `AdminZone`/`AdminZones` (zone inspection), `AdminShop` (`//buy`/`//gmshop`),
//! `AdminClan`'s `//clan_info`, `AdminGeodata`'s read-only queries and
//! `AdminPathNode`'s `//path_find`. The geodata *editor* half lives in
//! [`super::geo_editor`].
//!
//! `AdminFence` needs a spawnable-fence runtime the server has not ported;
//! quest-script reload / clan leadership ops / pledge editing likewise need
//! systems (script engine, clan mutation) that are only partially present, so
//! those siblings stay on the not-implemented path.

use crate::data::zone_data::ZoneKind;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::send_to_client;
use crate::game_loop::helpers::{nth_arg, send_message, send_sm_bare_to_client};
use crate::game_loop::npc::doors;
use crate::game_loop::space::position::maybe_position;
use crate::model::components::space::ZoneFlags;

use crate::model::door::Door;
use crate::network::server_packets;
use crate::network::trade;
use crate::world::World;

/// `Inventory.ADENA_ID` — Java's zone visualiser drops adena as its marker.
use crate::data::item_data::ADENA_ID;
use crate::game_loop::clans;
use crate::game_loop::combat::target;
use crate::game_loop::space::position;

/// `AdminDoorControl`'s `//open`/`//close [doorId]` and `//openall`/`//closeall`
/// — toggle one door (by template id, or the targeted door) or every door.
pub(super) fn admin_door(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    open: bool,
    all: bool,
    args: &[&str],
) {
    if all {
        let door_oids: Vec<i32> = world.door_regions.values().flatten().copied().collect();
        for oid in door_oids {
            toggle(world, oid, open);
        }
        send_message(
            world,
            client_id,
            if open {
                "All doors opened."
            } else {
                "All doors closed."
            },
        );
        return;
    }
    if let Some(door_id) = nth_arg::<i32>(args, 0) {
        doors::open_close_by_door_id(world, door_id, open);
        return;
    }
    // No id → the targeted door.
    let Some(target) =
        target::current(world, object_id).filter(|oid| world.objects.has_component::<Door>(oid))
    else {
        send_message(world, client_id, "Incorrect target.");
        return;
    };
    toggle(world, target, open);
}

fn toggle(world: &mut World, door_oid: i32, open: bool) {
    if open {
        doors::open_door(world, door_oid);
    } else {
        doors::close_door(world, door_oid);
    }
}

/// `AdminZone`/`AdminZones`'s `//zones` / `//zone_check` — report which zones the
/// GM currently stands in (Java opens a map-region HTML; text here).
pub(super) fn admin_zones(world: &mut World, client_id: u32, object_id: i32) {
    let mask = world
        .objects
        .get_component::<ZoneFlags>(&object_id)
        .map_or(0, |z| z.mask);
    let mut names = Vec::new();
    for (kind, label) in [
        (ZoneKind::Peace, "Peace"),
        (ZoneKind::Water, "Water"),
        (ZoneKind::NoRestart, "NoRestart"),
        (ZoneKind::Pvp, "PvP"),
    ] {
        if mask & kind.bit() != 0 {
            names.push(label);
        }
    }
    send_message(world, client_id, "=== Zones here ===");
    send_message(
        world,
        client_id,
        &if names.is_empty() {
            "None".to_string()
        } else {
            names.join(", ")
        },
    );
}

/// `AdminZone`'s `//zone_visual <id|all>` — drop a line of adena along each
/// zone boundary so a GM can *see* where a zone actually is.
///
/// Java's `ZoneForm.visualizeZone(z)`, one implementation per shape, all
/// stepping 10 units and dropping `ADENA_ID` ×1 at the GM's own Z. `all`
/// visualises every zone covering the GM; a numeric argument visualises that
/// zone by id.
///
/// SKIP(census): Java also walks `getSpawnTerritories(activeChar)` on `all` —
/// the spawn-territory polygons out of `spawns.xml`. The port keeps those in
/// `SpawnData` rather than in the zone list, and they are not what the command
/// is for; the zones themselves are.
pub(super) fn admin_zone_visual(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(pos) = maybe_position(world, object_id) else {
        return;
    };
    let Some(arg) = args.first() else {
        // Java's `st.nextToken()` throws here; the handler's caller catches
        // nothing, so the command simply does not run.
        return;
    };

    let mut points: Vec<(i32, i32)> = Vec::new();
    if arg.eq_ignore_ascii_case("all") {
        let forms: Vec<_> = world
            .data
            .zone_data
            .zones_at(pos.x, pos.y, pos.z)
            .map(|z| z.territory.form.clone())
            .collect();
        for form in &forms {
            border_points(form, &mut points);
        }
    } else if let Ok(zone_id) = arg.parse::<i32>() {
        let form = world
            .data
            .zone_data
            .zones
            .iter()
            .find(|z| z.id == zone_id)
            .map(|z| z.territory.form.clone());
        let Some(form) = form else {
            send_message(world, client_id, &format!("No zone with id {zone_id}."));
            return;
        };
        border_points(&form, &mut points);
    } else {
        return;
    }

    for (x, y) in points {
        let oid = crate::game_loop::items::ground_items::spawn_ground_item(
            world,
            ADENA_ID,
            1,
            0,
            x,
            y,
            pos.z,
            0,
            crate::game_loop::items::ground_items::DropSource::Npc,
        );
        world.zone_debug_items.push(oid);
    }
    send_message(
        world,
        client_id,
        &format!("{} zone markers dropped.", world.zone_debug_items.len()),
    );
}

/// `//zone_visual_clear` → `ZoneManager.clearDebugItems()`: every marker this
/// GM session dropped decays.
pub(super) fn admin_zone_visual_clear(world: &mut World, client_id: u32) {
    let markers = std::mem::take(&mut world.zone_debug_items);
    let count = markers.len();
    for oid in markers {
        if let Some(region) = position::region_cell_of(world, oid) {
            crate::game_loop::items::ground_items::despawn_ground_item(world, oid, region);
        }
    }
    send_message(world, client_id, &format!("{count} zone markers cleared."));
}

/// `ZoneForm.visualizeZone`, one arm per shape. `STEP` is Java's 10.
fn border_points(form: &crate::data::spawn_data::ZoneForm, out: &mut Vec<(i32, i32)>) {
    use crate::data::spawn_data::ZoneForm;
    const STEP: i32 = 10;
    match form {
        ZoneForm::NPoly { xs, ys } => {
            for i in 0..xs.len() {
                let next = (i + 1) % xs.len();
                let (vx, vy) = (xs[next] - xs[i], ys[next] - ys[i]);
                let length = ((vx * vx + vy * vy) as f64).sqrt() / f64::from(STEP);
                if length <= 0.0 {
                    continue;
                }
                let mut o = 1.0;
                while o <= length {
                    out.push((
                        xs[i] + (o / length * f64::from(vx)) as i32,
                        ys[i] + (o / length * f64::from(vy)) as i32,
                    ));
                    o += 1.0;
                }
            }
        }
        ZoneForm::Cuboid { x1, x2, y1, y2 } => {
            let mut x = *x1;
            while x < *x2 {
                out.push((x, *y1));
                out.push((x, *y2));
                x += STEP;
            }
            let mut y = *y1;
            while y < *y2 {
                out.push((*x1, y));
                out.push((*x2, y));
                y += STEP;
            }
        }
        ZoneForm::Cylinder { x, y, rad } => {
            let count = ((2.0 * std::f64::consts::PI * f64::from(*rad)) / f64::from(STEP)) as i32;
            if count <= 0 {
                return;
            }
            let angle = (2.0 * std::f64::consts::PI) / f64::from(count);
            for i in 0..count {
                out.push((
                    x + ((angle * f64::from(i)).cos() * f64::from(*rad)) as i32,
                    y + ((angle * f64::from(i)).sin() * f64::from(*rad)) as i32,
                ));
            }
        }
    }
}

/// `AdminShop`'s `//buy <buyListId>` — open a buy window for a merchant buy-list
/// (admin path skips the npc-allowed check Java also bypasses).
pub(super) fn admin_buy(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(list_id) = nth_arg::<i32>(args, 0) else {
        send_message(world, client_id, "Please specify buylist.");
        return;
    };
    let Some(list) = world.data.buy_lists.get(list_id) else {
        send_message(world, client_id, &format!("Buylist {list_id} not found."));
        return;
    };
    let refund_items = crate::game_loop::commerce::shop::refund_items_of(world, object_id);
    let Some(inventory) = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&object_id)
    else {
        return;
    };
    // Java `AdminShop`: `new BuyList(buyList, activeChar, 0)` — no castle tax.
    send_to_client(
        world,
        client_id,
        trade::buy_list(
            list,
            inventory,
            &world.data,
            0.0,
            world.cfg.rates.rate_siege_guards_price,
            |p| crate::game_loop::commerce::shop::stock_left(world, list_id, p),
        ),
    );
    send_to_client(
        world,
        client_id,
        trade::ex_buy_sell_list_sell(
            inventory,
            &refund_items,
            &world.data,
            false,
            crate::game_loop::servitor::active_pet_collar(world, object_id),
        ),
    );
}

/// `AdminClan`'s `//clan_info` — dump the targeted player's clan (name, leader,
/// level, member count).
pub(super) fn admin_clan_info(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = target::current_player(world, object_id) else {
        send_sm_bare_to_client(world, client_id, server_packets::sm_ids::INVALID_TARGET);
        return;
    };
    let name = player_name_or_empty(world, target);
    let Some(clan_id) = clans::clan_of(world, target) else {
        // Java sends THE_TARGET_MUST_BE_A_CLAN_MEMBER; that sysstring id isn't
        // in the ported table yet, so fall back to INVALID_TARGET.
        send_sm_bare_to_client(world, client_id, server_packets::sm_ids::INVALID_TARGET);
        return;
    };
    // The clan hall this clan owns, if any (Java `ClanHall.getOwner()` reverse).
    let clan_hall = world
        .clan_halls
        .values()
        .find(|h| h.owner_id == clan_id)
        .map(|h| h.name.clone())
        .unwrap_or_else(|| "No".into());
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    // `claninfo.htm` (Java `AdminClan.admin_clan_info`). Castle/fort/reputation/
    // ally aren't modelled yet → the Java "None"/0 defaults.
    let r: Vec<(&str, String)> = vec![
        ("clan_name", clan.name.clone()),
        ("clan_leader", clan.leader_name().to_string()),
        ("clan_level", clan.level.to_string()),
        ("clan_has_castle", "No".into()),
        ("clan_has_clanhall", clan_hall),
        ("clan_has_fortress", "No".into()),
        ("clan_points", "0".into()),
        ("clan_players_count", clan.members.len().to_string()),
        ("clan_ally", "Not in ally".into()),
        ("current_player_objectId", target.to_string()),
        ("current_player_name", name),
    ];
    super::menu::show_admin_html_replace(world, client_id, "claninfo.htm", &r);
}

/// `AdminGeodata`'s read-only queries: `//geo_pos` / `//geo_spawn_pos` (report
/// the GM's geo coordinates + height) and `//geo_can_move` / `//geo_can_see`
/// (line-of-sight from the GM to the current target).
pub(super) fn admin_geo_pos(world: &mut World, client_id: u32, object_id: i32, spawn: bool) {
    let Some(pos) = maybe_position(world, object_id) else {
        return;
    };
    let geo = &world.geo;
    let (gx, gy) = (geo.get_geo_x(pos.x), geo.get_geo_y(pos.y));
    if !geo.has_geo_pos(gx, gy) {
        send_message(world, client_id, "There is no geodata at this position.");
        return;
    }
    let gz = if spawn {
        geo.get_spawn_height(pos.x, pos.y, pos.z)
    } else {
        geo.get_height(pos.x, pos.y, pos.z)
    };
    send_message(
        world,
        client_id,
        &format!(
            "WorldX: {}, WorldY: {}, WorldZ: {}, GeoX: {gx}, GeoY: {gy}, GeoZ: {gz}",
            pos.x, pos.y, pos.z
        ),
    );
}

pub(super) fn admin_geo_can_see(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = target::current(world, object_id) else {
        send_sm_bare_to_client(world, client_id, server_packets::sm_ids::INVALID_TARGET);
        return;
    };
    let (Some(a), Some(b)) = (
        maybe_position(world, object_id),
        maybe_position(world, target),
    ) else {
        return;
    };
    let visible = world.geo.can_see_target(a.x, a.y, a.z, b.x, b.y, b.z);
    send_message(
        world,
        client_id,
        if visible {
            "Can see target."
        } else {
            "Cannot see target."
        },
    );
}

/// `AdminGeodata`'s `//geomap` — the geodata tile (region file) the GM stands in
/// plus that tile's world bounds.
pub(super) fn admin_geomap(world: &mut World, client_id: u32, object_id: i32) {
    let Some(pos) = maybe_position(world, object_id) else {
        return;
    };
    let ((tx, ty), (min_x, min_y), (max_x, max_y)) = world.geo.geomap_tile(pos.x, pos.y);
    send_message(
        world,
        client_id,
        &format!("GeoMap: {tx}_{ty} ({min_x},{min_y} to {max_x},{max_y})"),
    );
}

/// `AdminGeodata`'s `//geocell` — the geo cell (geoX/geoY), its nearest Z and
/// the cell-center world coords at the GM's position.
pub(super) fn admin_geocell(world: &mut World, client_id: u32, object_id: i32) {
    let Some(pos) = maybe_position(world, object_id) else {
        return;
    };
    let geo = &world.geo;
    let (gx, gy) = (geo.get_geo_x(pos.x), geo.get_geo_y(pos.y));
    let gz = geo.get_nearest_z(gx, gy, pos.z);
    let (wx, wy) = (geo.get_world_x(gx), geo.get_world_y(gy));
    send_message(
        world,
        client_id,
        &format!("GeoCell: {gx}, {gy}. XYZ ({wx}, {wy}, {gz})"),
    );
}

/// `AdminPathNode`'s `//path_find` — run the cell pathfinder from the GM to
/// their target and dump every node of the route, Java's `PathFinding.findPath`
/// with `playable = true`.
pub(super) fn admin_path_find(world: &mut World, client_id: u32, object_id: i32) {
    if world.path_finding < 1 {
        send_message(world, client_id, "PathFinding is disabled.");
        return;
    }
    let Some(target) = target::current(world, object_id) else {
        send_message(world, client_id, "No Target!");
        return;
    };
    let (Some(from), Some(to)) = (
        maybe_position(world, object_id),
        maybe_position(world, target),
    ) else {
        return;
    };
    let path = crate::geo::path::find_path(
        &world.geo,
        &world.path_cfg,
        (from.x, from.y, from.z),
        (to.x, to.y, to.z),
        true,
    );
    let Some(path) = path else {
        send_message(world, client_id, "No Route!");
        return;
    };
    for (x, y, z) in path {
        send_message(world, client_id, &format!("x:{x} y:{y} z:{z}"));
    }
}

// ---------------------------------------------------------------------------
// `AdminShutdown` + `AdminLogin` (category-4 sweep)
// ---------------------------------------------------------------------------

/// Java's `Shutdown` countdown announce marks (seconds).
const SHUTDOWN_MARKS: &[u64] = &[
    540, 480, 420, 360, 300, 240, 180, 120, 60, 30, 10, 5, 4, 3, 2, 1,
];

fn announce_all(world: &World, text: &str) {
    let packet = server_packets::system_message_with(
        server_packets::sm_ids::S1_TEXT,
        &[server_packets::SmParam::Text(text.to_string())],
    );
    world.broadcast_to_all_online(&packet);
}

/// `//server_shutdown [sec]` / `//server_restart [sec]` — start the countdown
/// (Java `Shutdown.startShutdown`). The final tick requests the game thread's
/// graceful stop, which runs the save-all path `main` already wires; under
/// systemd a "restart" is the same stop with the service manager bringing the
/// process back (Java's dedicated restart exit code isn't needed).
pub(super) fn admin_server_shutdown(
    world: &mut World,
    client_id: u32,
    args: &[&str],
    restart: bool,
) {
    let Some(secs) = nth_arg::<u64>(args, 0) else {
        send_message(world, client_id, "Usage: //server_shutdown <seconds>");
        return;
    };
    begin_shutdown(world, secs as i32, restart);
}

/// `Shutdown.startShutdown(null, seconds, restart)` — the countdown itself,
/// shared by `//server_shutdown`, the scheduled restart and the watchdog, so
/// all three announce alike and all three are cancelled by `//server_abort`.
pub(crate) fn begin_shutdown(world: &mut World, seconds: i32, restart: bool) {
    let secs = seconds.max(0) as u64;
    let deadline = world.tick + secs * 10;
    world.pending_shutdown = Some((deadline, restart));
    announce_all(
        world,
        &format!(
            "The server will be coming down in {secs} seconds! Please find a safe place to log out."
        ),
    );
    schedule_shutdown_tick(world);
}

/// `//server_abort` — cancel a running countdown (Java `Shutdown.abort`).
pub(super) fn admin_server_abort(world: &mut World, client_id: u32) {
    if world.pending_shutdown.take().is_some() {
        announce_all(world, "Server aborts and continues normal operation.");
    } else {
        send_message(world, client_id, "No shutdown is in progress.");
    }
}

/// Schedule the next countdown beat: the next Java announce mark, or the
/// deadline itself.
fn schedule_shutdown_tick(world: &mut World) {
    let Some((deadline, _)) = world.pending_shutdown else {
        return;
    };
    let remaining = deadline.saturating_sub(world.tick) / 10;
    let next_mark = SHUTDOWN_MARKS
        .iter()
        .copied()
        .find(|&m| m < remaining)
        .unwrap_or(0);
    let fire_at = deadline.saturating_sub(next_mark * 10).max(world.tick + 1);
    world
        .scheduler
        .schedule(fire_at, crate::scheduler::ScheduledTask::ServerShutdownTick);
}

/// The countdown beat — announce the mark, or stop the server at 0.
pub(crate) fn server_shutdown_tick(world: &mut World) {
    let Some((deadline, restart)) = world.pending_shutdown else {
        return; // aborted — stale beat
    };
    if world.tick >= deadline {
        announce_all(world, "The server is shutting down now.");
        if let Some(signal) = &world.shutdown_signal {
            signal.request();
        }
        tracing::info!(
            "GM {} requested — stopping the game thread (save-all runs on exit).",
            if restart { "restart" } else { "shutdown" }
        );
        return;
    }
    let remaining = deadline.saturating_sub(world.tick) / 10;
    announce_all(
        world,
        &format!(
            "The server will be coming down in {remaining} second(s)! Please find a safe place to log out."
        ),
    );
    schedule_shutdown_tick(world);
}

/// `AdminLogin`'s `ServerStatus` toggles — pushed straight over the login
/// link. `//server_gm_only`/`//server_all` flip the listing status,
/// `//server_max_player <n>`, `//server_list_age <0|15|18>`, and
/// `//server_list_type <n>` set their attributes (Java's named-type parsing
/// accepts the numeric bitmask here).
pub(super) fn admin_server_status(world: &mut World, client_id: u32, cmd: &str, args: &[&str]) {
    use crate::loginlink::{LoginLinkCommand, status};
    let attrs: Vec<(i32, i32)> = match cmd {
        "admin_server_gm_only" => vec![(status::SERVER_LIST_STATUS, status::STATUS_GM_ONLY)],
        "admin_server_all" => vec![(status::SERVER_LIST_STATUS, status::STATUS_AUTO)],
        "admin_server_max_player" => {
            let Some(n) = nth_arg::<i32>(args, 0) else {
                send_message(world, client_id, "Format: //server_max_player <number>");
                return;
            };
            vec![(status::MAX_PLAYERS, n)]
        }
        "admin_server_list_age" => {
            let age = match nth_arg::<i32>(args, 0) {
                Some(15) => status::SERVER_AGE_15,
                Some(18) => status::SERVER_AGE_18,
                Some(0) => status::SERVER_AGE_ALL,
                _ => {
                    send_message(world, client_id, "Format: //server_list_age <0|15|18>");
                    return;
                }
            };
            vec![(status::SERVER_AGE, age)]
        }
        "admin_server_list_type" => {
            let Some(n) = nth_arg::<i32>(args, 0) else {
                send_message(world, client_id, "Format: //server_list_type <type mask>");
                return;
            };
            vec![(status::SERVER_TYPE, n)]
        }
        _ => return,
    };
    // Java's `setServerStatus`/`setMaxPlayer` keep the pushed value on the
    // `LoginServerThread`, which is where `//server_login`'s page reads it
    // back from. Remember it here for the same reason.
    for &(attr, value) in &attrs {
        match attr {
            status::SERVER_LIST_STATUS => world.login.server_status = value,
            status::MAX_PLAYERS => world.login.max_players = value,
            _ => {}
        }
    }
    let _ = world
        .login
        .link
        .send(LoginLinkCommand::ServerStatus { attrs });
    send_message(
        world,
        client_id,
        "Server status updated on the login server.",
    );
    // Every branch of `AdminLogin` ends with `showMainPage(activeChar)` — the
    // page is the panel these buttons live on, so it redraws with the value
    // that just changed.
    admin_server_login(world, client_id);
}

/// `AdminLogin.showMainPage` — `//server_login`, the Server Management Menu.
/// `data/html/admin/login.htm` is the page the five `//server_*` commands
/// above are buttons on; without this it was a file nothing served.
pub(super) fn admin_server_login(world: &World, client_id: u32) {
    use crate::loginlink::status;
    // `ServerStatus.STATUS_STRING[_status]`.
    let status_name = match world.login.server_status {
        status::STATUS_AUTO => "Auto",
        1 => "Good",
        2 => "Normal",
        3 => "Full",
        4 => "Down",
        status::STATUS_GM_ONLY => "Gm Only",
        _ => "",
    };
    super::menu::show_admin_html_replace(
        world,
        client_id,
        "login.htm",
        &[
            (
                "server_name",
                world.login.server_name.clone().unwrap_or_default(),
            ),
            ("status", status_name.to_string()),
            ("clock", server_type_name(world.cfg.server.server_list_type)),
            ("brackets", world.cfg.server.server_list_bracket.to_string()),
            ("max_players", world.login.max_players.to_string()),
        ],
    );
}

/// `AdminLogin.getServerTypeName` — the `ServerListType` bitmask spelled out,
/// `+`-joined in bit order.
fn server_type_name(server_type: i32) -> String {
    const NAMES: [(i32, &str); 7] = [
        (0x01, "Normal"),
        (0x02, "Relax"),
        (0x04, "Test"),
        (0x08, "NoLabel"),
        (0x10, "Restricted"),
        (0x20, "Event"),
        (0x40, "Free"),
    ];
    NAMES
        .iter()
        .filter(|(bit, _)| server_type & bit != 0)
        .map(|(_, name)| *name)
        .collect::<Vec<_>>()
        .join("+")
}
