//! `AdminAdmin` — the `//admin` GM menu. Port of `AdminAdmin.showMainPage`:
//! `admin_admin` / `admin_admin1..7` open the admin HTML panels. The panels'
//! buttons fire `bypass admin_…`, which the `admin_` bypass path routes back
//! through [`use_admin_command`](super::use_admin_command) into the ordinary
//! handlers — so the menu drives the same commands the `//` bar does.

use crate::network::server_packets;
use crate::world::World;

/// `AdminAdmin.showMainPage`: `mode = parseInt(command.substring(11))` (the
/// digit after `admin_admin`), switch 1..7 → menu page, default `main`.
pub(super) fn admin_admin(world: &mut World, client_id: u32, command: &str) {
    // `command.substring(11)` — "admin_admin" is exactly 11 chars, so this is
    // the trailing digit ("" for the bare `admin_admin`, which falls through to
    // `main`, matching Java's parseInt-throws → mode 0 default).
    let filename = match command.get(11..).and_then(|s| s.trim().parse::<i32>().ok()) {
        Some(1) => "main",
        Some(2) => "game",
        Some(3) => "effects",
        Some(4) => "server",
        Some(5) => "mods",
        Some(6) => "char",
        Some(7) => "gm",
        _ => "main",
    };
    show_admin_html(world, client_id, &format!("{filename}_menu.htm"));
}

/// Java `AdminHtml.showAdminHtml`: serve `data/html/admin/<path>` through a
/// `NpcHtmlMessage(0, 1)` (not NPC-scoped; item id 1 keeps the window open
/// when its bypass buttons are clicked, so the GM menu stays up across
/// commands that send no html back). A missing file shows the retail "text is
/// missing" placeholder rather than nothing.
pub(super) fn show_admin_html(world: &World, client_id: u32, path: &str) {
    show_admin_html_replace(world, client_id, path, &[]);
}

/// As [`show_admin_html`] but substitutes `%token%` placeholders first (Java
/// `NpcHtmlMessage.replace`). Each `(token, value)` replaces every `%token%`.
pub(super) fn show_admin_html_replace(world: &World, client_id: u32, path: &str, replacements: &[(&str, String)]) {
    let full = format!("{}data/html/admin/{path}", world.data.root);
    let mut content = std::fs::read_to_string(&full).unwrap_or_else(|_| {
        format!("<html><body>My text is missing:<br>data/html/admin/{path}</body></html>")
    });
    for (token, value) in replacements {
        content = content.replace(&format!("%{token}%"), value);
    }
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message_item(0, 1, &content));
    }
}

/// Send fully-built admin HTML through the same `NpcHtmlMessage(0, 1)` channel
/// as [`show_admin_html`], for the dynamically-generated listings (spawn-by-level,
/// npc-by-letter) that Java assembles in a `StringBuilder` rather than a file.
pub(super) fn send_admin_html_content(world: &World, client_id: u32, html: &str) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message_item(0, 1, html));
    }
}
