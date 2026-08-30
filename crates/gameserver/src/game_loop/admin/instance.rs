//! `AdminInstance` — the GM instance panel (G27). Port of the datapack
//! `admincommandhandlers/AdminInstance`: an overview page (`//instance`), a
//! template list + per-template detail (`//listinstances` / `//instancelist
//! id=N`), and create / teleport / destroy, each of which redraws the detail
//! page. The per-player instance-reuse view (`AdminInstanceZone`) is deferred
//! with reuse-time tracking.

use super::menu::show_admin_html_replace;
use super::send_message;
use crate::data::instance_data::InstanceTemplate;
use crate::game_loop::helpers::nth_arg;
use crate::game_loop::instances;
use crate::world::World;

/// Templates the retail panel hides from the list — the Olympiad arenas and the
/// Chambers of Delusion, all driven by their own managers (Java
/// `IGNORED_TEMPLATES`).
const IGNORED_TEMPLATES: &[i32] = &[127, 128, 129, 130, 131, 132, 147, 148, 149, 150];

/// `//instance` / `//instances`: the overview page (live-instance + template
/// counts).
pub(super) fn admin_instance_panel(world: &World, client_id: u32) {
    show_admin_html_replace(
        world,
        client_id,
        "instances.htm",
        &[
            ("instCount", world.instances.len().to_string()),
            ("tempCount", world.data.instance_templates.len().to_string()),
        ],
    );
}

/// `//listinstances` / `//instancelist [id=N] [page=N]`: `id>0` opens the
/// template detail, otherwise the template list (Java `processBypass`).
pub(super) fn admin_instance_list(world: &World, client_id: u32, args: &[&str]) {
    let template_id = kv_int(args, "id").unwrap_or(0);
    let page = kv_int(args, "page").unwrap_or(0);
    if template_id > 0 {
        send_template_details(world, client_id, template_id);
    } else {
        send_template_list(world, client_id, page);
    }
}

/// `//instancecreate <templateId> [Alone|Party|CommandChannel]`: build an
/// instance from the template and move the chosen group into it, then redraw
/// its detail page.
pub(super) fn admin_instance_create(world: &mut World, client_id: u32, gm_oid: i32, args: &[&str]) {
    let Some(template_id) = nth_arg::<i32>(args, 0) else {
        send_message(
            world,
            client_id,
            "Usage: //instancecreate <templateId> [Alone|Party]",
        );
        return;
    };
    if world.data.instance_templates.get(template_id).is_none() {
        send_message(world, client_id, "Wrong parameters! Please try again.");
        return;
    }
    // Java's enter groups. Interlude has no command channels, so CommandChannel
    // collapses to Party (which itself falls back to the GM alone).
    let members: Vec<i32> = match args.get(1).copied().unwrap_or("Alone") {
        "Alone" => vec![gm_oid],
        "Party" | "CommandChannel" => crate::game_loop::party::group_or_self(world, gm_oid),
        _ => {
            send_message(
                world,
                client_id,
                "Wrong enter group usage! Please use those values: Alone, Party or CommandChannel.",
            );
            return;
        }
    };

    let Some(instance_id) = instances::create_from_template(world, template_id) else {
        send_message(world, client_id, "Wrong parameters! Please try again.");
        return;
    };
    for member in members {
        instances::enter(world, member, instance_id);
    }
    send_template_details(world, client_id, template_id);
}
fn check_instance_template_for(world: &World, id: i32, client_id: u32) -> Option<i32> {
    let t = world.instances.get(id).map(|i| i.template_id);
    if t.is_none() {
        send_message(world, client_id, &format!("No instance {id}."));
    }
    t
}
/// `//instanceteleport <instanceId>`: enter an existing instance, then redraw
/// its detail page.
pub(super) fn admin_instance_teleport(
    world: &mut World,
    client_id: u32,
    gm_oid: i32,
    args: &[&str],
) {
    let Some(id) = nth_arg::<i32>(args, 0) else {
        send_message(world, client_id, "Usage: //instanceteleport <instanceId>");
        return;
    };
    let Some(template_id) = check_instance_template_for(world, id, client_id) else {
        return;
    };
    instances::enter(world, gm_oid, id);
    send_template_details(world, client_id, template_id);
}

/// `//instancedestroy <instanceId>`: tear an instance down (ousting anyone
/// inside), then redraw its template detail page.
pub(super) fn admin_instance_destroy(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(id) = nth_arg::<i32>(args, 0) else {
        send_message(world, client_id, "Usage: //instancedestroy <instanceId>");
        return;
    };
    let Some(template_id) = check_instance_template_for(world, id, client_id) else {
        return;
    };
    let count = world.instances.member_count(id);
    // Java warns everyone inside before the teleport-out.
    let members: Vec<i32> = world
        .instances
        .get(id)
        .map(|i| i.members.keys().copied().collect())
        .unwrap_or_default();
    for member in members {
        crate::game_loop::helpers::send_to_player(
            world,
            member,
            crate::network::server_packets::ex_show_screen_message(
                "Your instance has been destroyed by Game Master!",
                2,
                10_000,
            ),
        );
    }
    instances::destroy(world, id);
    send_message(
        world,
        client_id,
        &format!("You destroyed Instance {id} with {count} players inside."),
    );
    send_template_details(world, client_id, template_id);
}

/// Java `sendTemplateList`: the non-ignored templates, most-populated first.
/// (No next/prev pager yet — the Interlude template set fits one page.)
fn send_template_list(world: &World, client_id: u32, _page: i32) {
    let mut templates: Vec<&InstanceTemplate> = world
        .data
        .instance_templates
        .iter()
        .filter(|t| !IGNORED_TEMPLATES.contains(&t.id))
        .collect();
    templates.sort_by_key(|t| std::cmp::Reverse(world.instances.world_count(t.id)));

    let mut data = String::new();
    for t in templates {
        data.push_str(&format!(
            "<table border=0 cellpadding=0 cellspacing=0 bgcolor=\"363636\">\
             <tr><td align=center fixwidth=\"250\"><font color=\"LEVEL\">{name} ({id})</font></td></tr></table>\
             <table border=0 cellpadding=0 cellspacing=0 bgcolor=\"363636\">\
             <tr><td align=center fixwidth=\"83\">Active worlds:</td><td align=center fixwidth=\"83\"></td>\
             <td align=center fixwidth=\"83\">{worlds}</td></tr>\
             <tr><td align=center fixwidth=\"83\">Detailed info:</td><td align=center fixwidth=\"83\"></td>\
             <td align=center fixwidth=\"83\"><button value=\"Show me!\" action=\"bypass -h admin_instancelist id={id}\" width=\"85\" height=\"20\" back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table><br>",
            name = template_name(t),
            id = t.id,
            worlds = fmt_worlds(world, t),
        ));
    }
    show_admin_html_replace(
        world,
        client_id,
        "instances_list.htm",
        &[("pages", String::new()), ("data", data)],
    );
}

/// Java `sendTemplateDetails`: the template's stats plus the live instances made
/// from it, each with Teleport / Destroy buttons.
fn send_template_details(world: &World, client_id: u32, template_id: i32) {
    let Some(t) = world.data.instance_templates.get(template_id) else {
        send_message(
            world,
            client_id,
            &format!("Instance template with id {template_id} does not exist!"),
        );
        admin_instance_panel(world, client_id);
        return;
    };

    let mut instance_list = String::from(
        "<table border=0 cellpadding=2 cellspacing=0 bgcolor=\"363636\"><tr>\
         <td fixwidth=\"83\"><font color=\"LEVEL\">Instance ID</font></td>\
         <td fixwidth=\"83\"><font color=\"LEVEL\">Teleport</font></td>\
         <td fixwidth=\"83\"><font color=\"LEVEL\">Destroy</font></td></tr></table>",
    );
    let mut live: Vec<(i32, usize)> = world
        .instances
        .iter()
        .filter(|(_, inst)| inst.template_id == template_id)
        .map(|(id, inst)| (id, inst.members.len()))
        .collect();
    live.sort_by_key(|(_, count)| *count); // Java sorts by player count
    for (id, _) in live {
        instance_list.push_str(&format!(
            "<table border=0 cellpadding=2 cellspacing=0 bgcolor=\"363636\"><tr>\
             <td fixwidth=\"83\">{id}</td>\
             <td fixwidth=\"83\"><button value=\"Teleport!\" action=\"bypass -h admin_instanceteleport {id}\" width=75 height=18 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td>\
             <td fixwidth=\"83\"><button value=\"Destroy!\" action=\"bypass -h admin_instancedestroy {id}\" width=75 height=18 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td></tr></table>",
        ));
    }

    show_admin_html_replace(
        world,
        client_id,
        "instances_detail.htm",
        &[
            ("templateId", template_id.to_string()),
            ("templateName", template_name(t).to_string()),
            ("activeWorlds", fmt_worlds(world, t)),
            ("duration", format!("{} minutes", t.duration_min)),
            ("emptyDuration", format!("{} minutes", t.empty_destroy_min)),
            // Eject time and remove-buff aren't modeled yet (unused by the
            // lifecycle); shown as their retail defaults.
            ("ejectDuration", "0 minutes".to_string()),
            ("removeBuff", "false".to_string()),
            ("instanceList", instance_list),
        ],
    );
}

/// `"<worldCount> / <maxWorlds>"`, with -1 shown as "Unlimited".
fn fmt_worlds(world: &World, t: &InstanceTemplate) -> String {
    let cap = if t.max_worlds == -1 {
        "Unlimited".to_string()
    } else {
        t.max_worlds.to_string()
    };
    format!("{} / {}", world.instances.world_count(t.id), cap)
}

/// The display name (Java's field defaults to "UnknownInstance" when the XML
/// has no `name` attribute).
fn template_name(t: &InstanceTemplate) -> &str {
    t.name.as_deref().unwrap_or("UnknownInstance")
}

/// The GM's party members, or just the GM when they aren't in one.
/// `id=N` / `page=N`-style bypass argument (Java `BypassParser`).
fn kv_int(args: &[&str], key: &str) -> Option<i32> {
    args.iter().find_map(|a| {
        let (k, v) = a.split_once('=')?;
        if k == key {
            v.trim().parse().ok()
        } else {
            None
        }
    })
}

// ---------------------------------------------------------------------------
// `AdminInstanceZone` — the per-character instance-reuse view
// ---------------------------------------------------------------------------

/// `//instancezone [playername]` — the reuse ("re-enter") times a character is
/// holding, as a page with a Clear button per row. Falls back to the current
/// **target** when no name is given, and to the GM themself when there is no
/// target either.
///
/// **The list is always empty on this dist, in Java as much as here.** The one
/// template with a `<reenter>` block (LastImperialTomb, 136) declares no
/// `apply` attribute, so its reenter type is `NONE` and `Instance.setReenterTime`
/// — which only fires for `ON_ENTER`/`ON_FINISH` — never runs. Nothing ever
/// writes `character_instance_time`, which is why the port has no reuse store
/// to read: the page renders the header and no rows, exactly as Java's does.
pub(super) fn admin_instancezone(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let subject = match args.first() {
        Some(name) if !name.trim().is_empty() => {
            match super::find_online_player(world, name.trim()) {
                Some(oid) => oid,
                None => {
                    send_message(
                        world,
                        client_id,
                        &format!("The player {name} is not online"),
                    );
                    send_message(world, client_id, "Usage: //instancezone [playername]");
                    return;
                }
            }
        }
        // No name: Java uses the target if it is a player, and otherwise falls
        // through to `display(activeChar, activeChar)`.
        _ => crate::game_loop::target::current_player(world, object_id).unwrap_or(object_id),
    };
    display_instance_times(world, client_id, subject);
}

/// `//instancezone_clear <playername> <instanceId>` — drop one reuse entry.
///
/// With nothing ever stored (see [`admin_instancezone`]), the clear has no row
/// to remove; what it still does is Java's messaging — the GM is told, the
/// player is told, and the panel is redrawn — so the button on the (empty)
/// page behaves the same way it would with a row under it.
pub(super) fn admin_instancezone_clear(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let usage = "Usage: //instancezone_clear <playername> [instanceId]";
    let (Some(name), Some(template_id)) = (args.first(), nth_arg::<i32>(args, 1)) else {
        send_message(world, client_id, "Failed clearing instance time: ");
        send_message(world, client_id, usage);
        return;
    };
    // Java resolves the player first and throws (into the same message pair)
    // when they are offline.
    let Some(target) = super::find_online_player(world, name.trim()) else {
        send_message(world, client_id, "Failed clearing instance time: ");
        send_message(world, client_id, usage);
        return;
    };
    let instance_name: String = world
        .data
        .instance_templates
        .get(template_id)
        .and_then(|t| t.name.clone())
        .unwrap_or_default();
    send_message(
        world,
        client_id,
        &format!("Instance zone {instance_name} cleared for player {name}"),
    );
    if let Some(cid) = crate::game_loop::helpers::client_for_player(world, target) {
        send_message(
            world,
            cid,
            &format!("Admin cleared instance zone {instance_name} for you"),
        );
    }
    // "for refreshing instance window" — Java redraws the *GM's* own page.
    display_instance_times(world, client_id, object_id);
}

/// `AdminInstanceZone.display` — the page itself.
fn display_instance_times(world: &World, client_id: u32, subject: i32) {
    let name = crate::game_loop::helpers::player_name_or_empty(world, subject);
    let html = format!(
        "<html><center><table width=260>\
         <tr><td width=40><button value=\"Main\" action=\"bypass admin_admin\" width=40 height=21 \
         back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td>\
         <td width=180><center>Character Instances</center></td>\
         <td width=40><button value=\"Back\" action=\"bypass -h admin_current_player\" width=40 \
         height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table><br>\
         <font color=\"LEVEL\">Instances for {name}</font><center><br>\
         <table><tr><td width=150>Name</td><td width=50>Time</td><td width=70>Action</td></tr>\
         </table></html>"
    );
    super::menu::send_admin_html_content(world, client_id, &html);
}
