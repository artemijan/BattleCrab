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
const VALAKAS: i32 = 29028;
const BAIUM: i32 = 29020;
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
    let status = world.grand_bosses.get(&boss_id).map(|b| b.status).unwrap_or(-1);
    let respawn = world.grand_bosses.get(&boss_id).map(|b| b.respawn_time).unwrap_or(0);

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
        (if t.is_empty() { format!("Unk {status}") } else { t.to_string() }, c, 3)
    } else {
        let (t, c) = match status {
            0 => ("Alive", "00FF00"),
            1 => ("Dead", "FF0000"),
            _ => ("", "FFFFFF"),
        };
        (if t.is_empty() { format!("Unk {status}") } else { t.to_string() }, c, 1)
    };

    let respawn_str =
        if status == dead_status { format_epoch_millis(respawn) } else { "Already respawned!".to_string() };

    // Java counts players in the boss's NoRestartZone (Antharas/Baium only) and
    // otherwise shows "Zone not found!". The grand-boss zones aren't modelled
    // yet (G21), so every boss falls back to "Zone not found!" — matching Java's
    // own fallback when the zone isn't loaded.
    // TODO(G21): count ZoneManager.getZoneById(70050/70051).getPlayersInside().
    let players_inside = "Zone not found!".to_string();

    let replacements = [
        ("bossStatus", text),
        ("bossColor", color.to_string()),
        ("respawnTime", respawn_str),
        ("playersInside", players_inside),
    ];
    super::menu::show_admin_html_replace(world, client_id, html_patch, &replacements);
}

/// `//grandboss_skip|respawn|minions|abort <id>` — the per-boss panel's action
/// buttons. Each targets the grand-boss AI (Antharas for skip; Antharas + Baium
/// for the rest). That AI is unported (G21) and null in this dist, so Java NPEs
/// on the `notifyEvent` call; the same failure is reproduced here.
pub(super) fn admin_grandboss_action(world: &mut World, client_id: u32, command: &str, args: &[&str]) {
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
    let supported: &[i32] = if command == "admin_grandboss_skip" { &[ANTHARAS] } else { &[ANTHARAS, BAIUM] };
    if supported.contains(&id) {
        // TODO(G21): dispatch to the real Antharas/Baium AI, e.g.
        //   antharas_ai.notify_event("SKIP_WAITING" | "RESPAWN_ANTHARAS" |
        //   "DESPAWN_MINIONS" | "ABORT_FIGHT"). Until then this reproduces the
        //   dist's NPE (antharasAi()/baiumAi() return null).
        exec_exception_npe(world, client_id, command, args);
    } else {
        send_message(world, client_id, "Wrong ID!");
    }
}

/// Reproduce `AdminCommandHandler`'s `catch (RuntimeException)` message for the
/// null-AI `NullPointerException` (Java prints `"Exception during execution of
/// '<full>': " + e` — note the two spaces after "of").
fn exec_exception_npe(world: &World, client_id: u32, command: &str, args: &[&str]) {
    let full = if args.is_empty() { command.to_string() } else { format!("{command} {}", args.join(" ")) };
    send_message(world, client_id, &format!("Exception during execution of  '{full}': java.lang.NullPointerException"));
}

/// As above for the `Integer.parseInt` `NumberFormatException` (unreachable via
/// the panel buttons, which hardcode numeric ids, but faithful to a typed
/// non-numeric arg).
fn exec_exception_nfe(world: &World, client_id: u32, command: &str, args: &[&str], bad: &str) {
    let full = format!("{command} {}", args.join(" "));
    send_message(
        world,
        client_id,
        &format!("Exception during execution of  '{full}': java.lang.NumberFormatException: For input string: \"{bad}\""),
    );
}

/// Format epoch millis as `yyyy-MM-dd HH:mm:ss` (Java `SimpleDateFormat`). UTC,
/// matching the port's convention (Java uses the server-local zone; the port is
/// UTC throughout — see `reco`'s daily-reset note). Civil date via Hinnant's
/// days-from-epoch algorithm.
fn format_epoch_millis(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (tod / 3600, (tod % 3600) / 60, tod % 60);

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02}")
}
