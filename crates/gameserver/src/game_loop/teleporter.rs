//! Gatekeeper teleports (G15.5) — the runtime half of
//! `model/teleporter/TeleportHolder` (`showTeleportList`/`doTeleport`) plus
//! the `Teleporter.onBypassFeedback` verb routing. Data side:
//! [`crate::data::teleporter_data`].
//!
//! Deliberate narrowings (documented per the plan):
//! - Castle-siege gates (`TELEPORT_WHILE_SIEGE_IN_PROGRESS`, the
//!   `castleteleporter-busy.htm` branch, `castleId` checks) — no sieges (G24).
//! - The Mon/Tue 20:00–24:00 half-price window (`calculateFee`'s Calendar
//!   branch) — needs server-local wall-clock plumbing (G33's game clock).
//! - Noblesse lists check the player's nobless (G17); non-nobles are refused —
//!   behavior Java shows a non-noble.
//! - `isSubClassActive()` in the free-teleport check — no subclasses (G17).
//! - The combat-flag gate (SM 2348) — no combat flags (G24).
//! - Fee consumption sends `InventoryUpdate` but not Java's
//!   `destroyItemByItemId` "disappeared" system message — none of the ported
//!   consume paths (quest takes, buy) send those yet.

use tracing::warn;

use crate::data::item_data::ADENA_ID;
use crate::data::teleporter_data::{TeleportHolder, TeleportLocation};
use crate::model::components::Vitals;
use crate::network::server_packets::{self, sm_ids};
use crate::world::World;

/// `instanceof Teleporter` stand-in (same pattern as `is_village_master`).
pub(crate) fn is_teleporter(world: &World, npc_object_id: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_object_id)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.type_name == "Teleporter")
}

/// The `Teleporter.onBypassFeedback` verbs this slice serves. Returns `false`
/// for verbs that aren't teleporter commands (the caller falls through to its
/// unhandled-verb log).
pub(crate) fn handle_bypass(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    npc_object_id: i32,
    command: &str,
) -> bool {
    let mut tokens = command.split_whitespace();
    match tokens.next().unwrap_or("") {
        "showNoblesSelect" => {
            // No nobless status yet (G17): everyone gets the refusal page.
            send_teleporter_html(world, client_id, npc_object_id, "not_nobles.htm");
            true
        }
        "showTeleports" => {
            let list_name = tokens.next().unwrap_or("NORMAL");
            let bypass = format!("npc_{npc_object_id}_teleport");
            show_teleport_list(
                world,
                client_id,
                object_id,
                npc_object_id,
                list_name,
                &bypass,
            );
            true
        }
        "showTeleportsHunting" => {
            let list_name = tokens.next().unwrap_or("HUNTING");
            let bypass = format!("npc_{npc_object_id}_teleport");
            show_teleport_list(
                world,
                client_id,
                object_id,
                npc_object_id,
                list_name,
                &bypass,
            );
            true
        }
        "teleport" => {
            // Java requires exactly two more tokens (`countTokens() != 2`).
            let (Some(list_name), Some(loc_id), None) =
                (tokens.next(), tokens.next(), tokens.next())
            else {
                warn!("Teleporter: unhandled teleport command [{command}].");
                return true;
            };
            let loc_id = loc_id.parse::<usize>().ok();
            do_teleport(
                world,
                client_id,
                object_id,
                npc_object_id,
                list_name,
                loc_id,
            );
            true
        }
        _ => false,
    }
}

fn npc_template_id(world: &World, npc_object_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_object_id)
        .and_then(|n| n.template(world))
        .map(|t| t.id)
}

fn holder<'a>(world: &'a World, npc_object_id: i32, list_name: &str) -> Option<&'a TeleportHolder> {
    let template_id = npc_template_id(world, npc_object_id)?;
    world.data.teleporters.holder(template_id, list_name)
}

/// `TeleportHolder.shouldPayFee`: non-NORMAL lists always charge; NORMAL/
/// HUNTING charge above the free-teleport level (subclass check skipped).
fn should_pay_fee(
    world: &World,
    level: i32,
    holder: &TeleportHolder,
    loc: &TeleportLocation,
) -> bool {
    !holder.is_normal_teleport()
        || (level > world.cfg.character.max_free_teleport_level
            && loc.fee_id != 0
            && loc.fee_count > 0)
}

/// `TeleportHolder.calculateFee` minus the Mon/Tue evening discount (see
/// module docs).
fn calculate_fee(
    world: &World,
    level: i32,
    holder: &TeleportHolder,
    loc: &TeleportLocation,
) -> i64 {
    if holder.is_normal_teleport() && level <= world.cfg.character.max_free_teleport_level {
        return 0;
    }
    loc.fee_count
}

/// `TeleportHolder.getItemName(feeId, fstring)` — the fee suffix on the list
/// buttons. Adena/ancient adena use client fstrings like Java; anything else
/// falls back to the template name.
fn fee_item_name(world: &World, fee_id: i32) -> String {
    const ANCIENT_ADENA_ID: i32 = 5575;
    match fee_id {
        ADENA_ID => "<fstring>1000308</fstring>".to_string(),
        ANCIENT_ADENA_ID => "<fstring>1000309</fstring>".to_string(),
        _ => world
            .data
            .item_data
            .get(fee_id)
            .map(|t| t.name.clone())
            .unwrap_or_else(|| format!("Unknown item: {fee_id}")),
    }
}

/// `TeleportHolder.showTeleportList`: build the button list into
/// `data/html/teleporter/teleports.htm` and send it (quest-zone priority buttons
/// skipped — no quest zones ported). `bypass` is the button-action prefix —
/// normally `npc_<oid>_teleport` (the gatekeeper handler), but the Clan Hall
/// Manager passes its own quest bypass so the list routes back through it.
pub(crate) fn show_teleport_list(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    npc_object_id: i32,
    list_name: &str,
    bypass: &str,
) {
    let Some(level) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .map(|p| p.level)
    else {
        return;
    };
    let Some(holder) = holder(world, npc_object_id, list_name) else {
        warn!("Teleporter: unknown teleport list [{list_name}] for npc {npc_object_id}.");
        return;
    };
    // Java `TeleportType.NOBLESS`: only a noble may open the list.
    if holder.is_noblesse() && !is_noble(world, object_id) {
        warn!("Teleporter: noblesse teleport list [{list_name}] requested by a non-noble.");
        return;
    }

    let mut buttons = String::new();
    for (i, loc) in holder.locations.iter().enumerate() {
        let (mut final_name, confirm_desc) = if loc.npc_string_id >= 0 {
            (
                format!("<fstring>{}</fstring>", loc.npc_string_id),
                format!("F;{}", loc.npc_string_id),
            )
        } else {
            let name = loc.name.clone().unwrap_or_default();
            (name.clone(), name)
        };
        if should_pay_fee(world, level, holder, loc) {
            let fee = calculate_fee(world, level, holder, loc);
            if fee != 0 {
                final_name.push_str(&format!(" - {fee} {}", fee_item_name(world, loc.fee_id)));
            }
        }
        buttons.push_str(&format!(
            "<button align=left icon=\"teleport\" action=\"bypass -h {bypass} {} {i}\" \
             msg=\"811;{confirm_desc}\">{final_name}</button>",
            holder.name
        ));
    }

    let html = crate::data::htm_cache::read_htm(format!(
        "{}data/html/teleporter/teleports.htm",
        world.data.root
    ))
    .unwrap_or_else(|| "<html><body>%locations%</body></html>".to_string())
    .replace("%locations%", &buttons);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(npc_object_id, &html));
    }
}

/// `TeleportHolder.doTeleport`: validate, charge the fee, and teleport.
pub(crate) fn do_teleport(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    npc_object_id: i32,
    list_name: &str,
    loc_id: Option<usize>,
) {
    let (Some(level), Some(reputation)) = ({
        let p = world
            .objects
            .get_component::<crate::model::Player>(&object_id);
        (p.map(|p| p.level), p.map(|p| p.reputation))
    }) else {
        return;
    };
    let Some(holder) = holder(world, npc_object_id, list_name) else {
        warn!("Teleporter: unknown teleport list [{list_name}] for npc {npc_object_id}.");
        return;
    };
    if holder.is_noblesse() && !is_noble(world, object_id) {
        warn!("Teleporter: noblesse teleport requested by a non-noble.");
        return;
    }
    let Some(loc) = loc_id.and_then(|id| holder.locations.get(id)).cloned() else {
        warn!("Teleporter: unknown teleport location {loc_id:?} within list [{list_name}].");
        return;
    };
    let is_normal = holder.is_normal_teleport();
    let pay = should_pay_fee(world, level, holder, &loc);
    let fee = calculate_fee(world, level, holder, &loc);

    // NORMAL-list conditions (`doTeleport`'s isNormalTeleport block): the
    // karma gate; siege/combat-flag gates skipped (see module docs).
    if is_normal && !world.cfg.character.alt_karma_player_can_use_gk && reputation < 0 {
        super::admin::send_message(world, client_id, "Go away, you're not welcome here.");
        return;
    }

    // Fee charge — Java `destroyItemByItemId` checks the full amount before
    // touching anything; `take_items` strips partial stacks, so pre-check.
    if pay && fee > 0 {
        let have = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&object_id)
            .map(|inv| inv.count_of(loc.fee_id))
            .unwrap_or(0);
        if have < fee || !super::quests::take_items(world, client_id, object_id, loc.fee_id, fee) {
            if loc.fee_id == ADENA_ID {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(server_packets::system_message_with(
                        sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA,
                        &[],
                    ));
                }
            } else {
                let item = world
                    .data
                    .item_data
                    .get(loc.fee_id)
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| format!("Unknown item: {}", loc.fee_id));
                super::admin::send_message(
                    world,
                    client_id,
                    &format!("You do not have enough {item}"),
                );
            }
            return;
        }
    }

    // `!player.isAlikeDead()` → teleport.
    let dead = world
        .objects
        .get_component::<Vitals>(&object_id)
        .is_none_or(|v| v.dead);
    if !dead {
        super::death::teleport_player(world, object_id, loc.x, loc.y, loc.z);
    }
}

/// `Teleporter.sendHtmlMessage`: a fixed page from `data/html/teleporter/`
/// with the `%objectId%`/`%npcname%` replacements.
fn send_teleporter_html(world: &World, client_id: u32, npc_object_id: i32, file: &str) {
    let name = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_object_id)
        .and_then(|n| n.template(world))
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let html =
        crate::data::htm_cache::read_htm(format!("{}data/html/teleporter/{file}", world.data.root))
            .unwrap_or_else(|| "<html><body>My Text is missing:<br></body></html>".to_string())
            .replace("%objectId%", &npc_object_id.to_string())
            .replace("%npcname%", &name);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(npc_object_id, &html));
    }
}

/// `Player.isNoble()` — nobless gates the `NOBLESS` teleport lists.
fn is_noble(world: &World, player_object_id: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::Player>(&player_object_id)
        .is_some_and(|p| p.is_noble)
}
