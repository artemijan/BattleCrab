//! `AdminGrandBoss` — the Game panel's "Grand Boss Info" button. `//grandboss`
//! opens a boss menu; `//grandboss <id>` shows one boss's status / respawn time
//! / players-in-zone from [`crate::model::grand_boss`] (the `grandboss_data`
//! slice loaded at boot). The mutation buttons on the per-boss panels
//! (`//grandboss_skip|respawn|minions|abort`) drive the grand-boss AI, which is
//! unported (G21) — in this dist Java's `antharasAi()`/`baiumAi()` return null,
//! so those buttons throw an NPE the dispatcher catches; that outcome is
//! reproduced here until the AI lands.

use super::send_message;
use crate::world::World;

// Boss NPC ids (Java `AdminGrandBoss` constants).
const ANTHARAS: i32 = 29068;
/// `AdminGrandBoss.ANTHARAS_ZONE` — the Antharas Nest `NoRestartZone`.
const ANTHARAS_ZONE: i32 = 70050;
const VALAKAS: i32 = 29028;
const BAIUM: i32 = 29020;
/// `AdminGrandBoss.BAIUM_ZONE`.
const BAIUM_ZONE: i32 = 70051;
const QUEENANT: i32 = 29001;
const ORFEN: i32 = 29014;
const CORE: i32 = 29006;

/// The six bosses the panel knows (Java `Arrays.asList(...)` in `manageHtml`).
const PANEL_BOSSES: [i32; 6] = [ANTHARAS, VALAKAS, BAIUM, QUEENANT, ORFEN, CORE];

/// `//grandboss` — no arg opens the boss list; `//grandboss <id>` shows one
/// boss's status page (Java `AdminGrandBoss.admin_grandboss`).
pub(super) fn admin_grandboss(world: &mut World, client_id: u32, args: &[&str]) {
    match args.first() {
        None => super::menu::show_admin_html(world, client_id, "grandboss/grandboss.htm"),
        Some(id_str) => match id_str.parse::<i32>() {
            Ok(id) => manage_html(world, client_id, id),
            // Java `Integer.parseInt` throws → dispatcher catch (no page shown).
            Err(_) => exec_exception_nfe(world, client_id, "admin_grandboss", args, id_str),
        },
    }
}

/// Java `AdminGrandBoss.manageHtml`: render one boss's status page.
fn manage_html(world: &mut World, client_id: u32, boss_id: i32) {
    if !PANEL_BOSSES.contains(&boss_id) {
        send_message(world, client_id, "Wrong ID!");
        return;
    }

    // Java `GrandBossManager.getStatus` → -1 when the boss isn't stored.
    let status = world
        .grand_bosses
        .get(&boss_id)
        .map(|b| b.status)
        .unwrap_or(-1);
    let respawn = world
        .grand_bosses
        .get(&boss_id)
        .map(|b| b.respawn_time)
        .unwrap_or(0);

    let html_patch = match boss_id {
        ANTHARAS => "grandboss/grandboss_antharas.htm",
        VALAKAS => "grandboss/grandboss_valakas.htm",
        BAIUM => "grandboss/grandboss_baium.htm",
        QUEENANT => "grandboss/grandboss_queenant.htm",
        ORFEN => "grandboss/grandboss_orfen.htm",
        CORE => "grandboss/grandboss_core.htm",
        _ => unreachable!(),
    };

    // Antharas/Valakas/Baium have a 4-state lifecycle (dead == 3); the others
    // are alive/dead (dead == 1). Java's default text is "Unk <status>".
    let (text, color, dead_status) = if [ANTHARAS, VALAKAS, BAIUM].contains(&boss_id) {
        let (t, c) = match status {
            0 => ("Alive", "00FF00"),
            1 => ("Waiting", "FFFF00"),
            2 => ("In Fight", "FF9900"),
            3 => ("Dead", "FF0000"),
            _ => ("", "FFFFFF"),
        };
        (
            if t.is_empty() {
                format!("Unk {status}")
            } else {
                t.to_string()
            },
            c,
            3,
        )
    } else {
        let (t, c) = match status {
            0 => ("Alive", "00FF00"),
            1 => ("Dead", "FF0000"),
            _ => ("", "FFFFFF"),
        };
        (
            if t.is_empty() {
                format!("Unk {status}")
            } else {
                t.to_string()
            },
            c,
            1,
        )
    };

    let respawn_str = if status == dead_status {
        format_epoch_millis(respawn)
    } else {
        "Already respawned!".to_string()
    };

    // `bossZone != null ? bossZone.getPlayersInside().size() : "Zone not found!"`
    // — only Antharas and Baium have a nest zone in Java's table, so every
    // other boss shows the fallback string, which is Java's behaviour and not a
    // gap.
    let players_inside = match boss_zone_id(boss_id) {
        Some(zone_id) => players_in_zone(world, zone_id).to_string(),
        None => "Zone not found!".to_string(),
    };

    let replacements = [
        ("bossStatus", text),
        ("bossColor", color.to_string()),
        ("respawnTime", respawn_str),
        ("playersInside", players_inside),
    ];
    super::menu::show_admin_html_replace(world, client_id, html_patch, &replacements);
}

/// The `NoRestartZone` Java pairs with a boss on this panel (`ANTHARAS_ZONE` /
/// `BAIUM_ZONE`). The other four panel bosses have none.
fn boss_zone_id(boss_id: i32) -> Option<i32> {
    match boss_id {
        ANTHARAS => Some(ANTHARAS_ZONE),
        BAIUM => Some(BAIUM_ZONE),
        _ => None,
    }
}

/// `ZoneManager.getZoneById(id).getPlayersInside().size()` — the port has no
/// per-zone occupancy list, so the players are tested against the zone's
/// territory instead. Same answer, since Java's list is maintained by exactly
/// these enter/exit tests.
fn players_in_zone(world: &World, zone_id: i32) -> usize {
    let Some(zone) = world.data.zone_data.zones.iter().find(|z| z.id == zone_id) else {
        return 0;
    };
    world
        .in_game_player_oids()
        .filter(|oid| {
            world
                .objects
                .get_component::<crate::model::components::Position>(oid)
                .is_some_and(|p| {
                    p.z >= zone.territory.min_z
                        && p.z <= zone.territory.max_z
                        && zone.territory.contains_2d(p.x, p.y)
                })
        })
        .count()
}

/// `//grandboss_skip|respawn|minions|abort <id>` — the per-boss panel's action
/// buttons. Each targets the grand-boss AI (Antharas for skip; Antharas + Baium
/// for the rest). Both AI accessors `return null` in this build, so Java NPEs
/// on the `notifyEvent` call; the same failure is reproduced here.
pub(super) fn admin_grandboss_action(
    world: &mut World,
    client_id: u32,
    command: &str,
    args: &[&str],
) {
    // "admin_grandboss_skip" → "grandboss_skip" for the usage line.
    let usage_name = command.strip_prefix("admin_").unwrap_or(command);
    let Some(id_str) = args.first() else {
        send_message(world, client_id, &format!("Usage: //{usage_name} Id"));
        return;
    };
    let Ok(id) = id_str.parse::<i32>() else {
        exec_exception_nfe(world, client_id, command, args, id_str);
        return;
    };
    // `skip` is Antharas-only; respawn/minions/abort also handle Baium.
    let supported: &[i32] = if command == "admin_grandboss_skip" {
        &[ANTHARAS]
    } else {
        &[ANTHARAS, BAIUM]
    };
    if supported.contains(&id) {
        // **Not a gap.** `AdminGrandBoss.antharasAi()` / `baiumAi()` are
        // literally `return null;` in this build — the `QuestManager` lookup
        // beneath them is commented out — so every one of these buttons NPEs in
        // Java too, whatever the boss AI does. The port reproduces the NPE
        // rather than wiring the (ported) Antharas/Baium AI behind a button
        // that is dead upstream.
        exec_exception_npe(world, client_id, command, args);
    } else {
        send_message(world, client_id, "Wrong ID!");
    }
}

/// Reproduce `AdminCommandHandler`'s `catch (RuntimeException)` message for the
/// null-AI `NullPointerException` (Java prints `"Exception during execution of
/// '<full>': " + e` — note the two spaces after "of").
fn exec_exception_npe(world: &World, client_id: u32, command: &str, args: &[&str]) {
    let full = if args.is_empty() {
        command.to_string()
    } else {
        format!("{command} {}", args.join(" "))
    };
    send_message(
        world,
        client_id,
        &format!("Exception during execution of  '{full}': java.lang.NullPointerException"),
    );
}

/// As above for the `Integer.parseInt` `NumberFormatException` (unreachable via
/// the panel buttons, which hardcode numeric ids, but faithful to a typed
/// non-numeric arg).
fn exec_exception_nfe(world: &World, client_id: u32, command: &str, args: &[&str], bad: &str) {
    let full = format!("{command} {}", args.join(" "));
    send_message(
        world,
        client_id,
        &format!(
            "Exception during execution of  '{full}': java.lang.NumberFormatException: For input string: \"{bad}\""
        ),
    );
}

/// Format epoch millis as `yyyy-MM-dd HH:mm:ss` (Java `SimpleDateFormat`). UTC,
/// matching the port's convention (Java uses the server-local zone; the port is
/// UTC throughout — see `reco`'s daily-reset note). Civil date via Hinnant's
/// days-from-epoch algorithm.
fn format_epoch_millis(ms: i64) -> String {
    let (y, m, d, hh, mm, ss) = commons::util::civil_from_millis(ms);
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02}")
}
