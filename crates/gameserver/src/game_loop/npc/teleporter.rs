//! Gatekeeper teleports (G15.5) — the runtime half of
//! `model/teleporter/TeleportHolder` (`showTeleportList`/`doTeleport`) plus
//! the `Teleporter.onBypassFeedback` verb routing. Data side:
//! [`crate::data::teleporter_data`].
//!
//! The G15.5 narrowings are closed by the row-9 tail: the siege gates
//! (`TeleportWhileSiegeInProgress` = **False** here, so a destination whose
//! `castleId` is under siege is refused, and a castle gatekeeper serves
//! `castleteleporter-busy.htm` while its own castle is besieged), the Mon/Tue
//! 20:00+ half-price window, `isSubClassActive()` in the free-teleport check,
//! the combat-flag gate, and the noble page.
//!
//! Still narrowed:
//! - Fee consumption sends `InventoryUpdate` but not Java's
//!   `destroyItemByItemId` "disappeared" system message — none of the ported
//!   consume paths (quest takes, buy) send those yet.
//! - The Mon/Tue window is evaluated in **UTC**, like the port's other
//!   wall-clock work (`daily_tasks`), where Java uses server-local time.

use crate::game_loop::helpers::clan_of_or_zero;
use crate::game_loop::helpers::{
    is_dead, npc_name_or_empty, npc_template, send_message, send_to_client,
};
use tracing::warn;

use crate::data::item_data::ADENA_ID;
use crate::data::teleporter_data::{TeleportHolder, TeleportLocation};
use crate::game_loop::{death, items, siege};
use crate::network::server_packets::{self, sm_ids};
use crate::world::World;

/// `instanceof Teleporter` stand-in (same pattern as `is_village_master`).
pub(crate) fn is_teleporter(world: &World, npc_object_id: i32) -> bool {
    npc_template(world, npc_object_id).is_some_and(|t| t.type_name == "Teleporter")
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
            let file = if is_noble(world, object_id) {
                "nobles_select.htm"
            } else {
                "not_nobles.htm"
            };
            send_teleporter_html(world, client_id, npc_object_id, file);
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
    npc_template(world, npc_object_id).map(|t| t.id)
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
    object_id: i32,
    holder: &TeleportHolder,
    loc: &TeleportLocation,
) -> bool {
    !holder.is_normal_teleport()
        || ((level > world.cfg.character.max_free_teleport_level
            || is_subclass_active(world, object_id))
            && loc.fee_id != 0
            && loc.fee_count > 0)
}

/// `TeleportHolder.calculateFee`: free below the level cap (unless a subclass is
/// active), and **half price from 20:00 on Monday and Tuesday** — Java's
/// `Calendar` branch, evaluated in UTC here.
fn calculate_fee(
    world: &World,
    level: i32,
    object_id: i32,
    holder: &TeleportHolder,
    loc: &TeleportLocation,
) -> i64 {
    let now = world.now_millis();
    calculate_fee_at(world, level, object_id, holder, loc, now)
}

/// [`calculate_fee`] with the clock injected, so the Mon/Tue window is testable.
pub(crate) fn calculate_fee_at(
    world: &World,
    level: i32,
    object_id: i32,
    holder: &TeleportHolder,
    loc: &TeleportLocation,
    now_millis: i64,
) -> i64 {
    if holder.is_normal_teleport() {
        if !is_subclass_active(world, object_id)
            && level <= world.cfg.character.max_free_teleport_level
        {
            return 0;
        }
        if is_half_price_window(now_millis) {
            return loc.fee_count / 2;
        }
    }
    loc.fee_count
}

/// Java's `(hour >= 20) && (dayOfWeek >= MONDAY && dayOfWeek <= TUESDAY)` —
/// the 20:00–24:00 Monday/Tuesday discount. Epoch day 0 (1970-01-01) was a
/// Thursday, so `((days + 4) % 7)` is 0 = Sunday … 1 = Monday, 2 = Tuesday.
pub(crate) fn is_half_price_window(now_millis: i64) -> bool {
    let days = now_millis.div_euclid(86_400_000);
    let weekday = (days + 4).rem_euclid(7);
    let hour = now_millis.div_euclid(3_600_000).rem_euclid(24);
    hour >= 20 && (weekday == 1 || weekday == 2)
}

/// Java `Player.isSubClassActive()` — the character is playing a subclass, not
/// their base class. Such a character pays the teleport fee at any level.
fn is_subclass_active(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .is_some_and(|p| p.class_id != p.base_class_id)
}

/// Java `Player.isCombatFlagEquipped()` — carrying a siege ward (item 9819)
/// blocks gatekeeper teleports.
pub(crate) fn has_combat_flag(world: &World, object_id: i32) -> bool {
    const COMBAT_FLAG: i32 = 9819;
    world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&object_id)
        .is_some_and(|inv| inv.count_of(COMBAT_FLAG) > 0)
}

/// Whether any of `castle_ids` has a siege running (Java's destination check).
fn any_siege_in_progress(world: &World, castle_ids: &[i32]) -> bool {
    castle_ids
        .iter()
        .any(|id| world.sieges.get(id).is_some_and(|s| s.in_progress))
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
        if should_pay_fee(world, level, object_id, holder, loc) {
            let fee = calculate_fee(world, level, object_id, holder, loc);
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

    let html = crate::data::htm_cache::read_htm_for(
        world,
        object_id,
        format!("{}data/html/teleporter/teleports.htm", world.data.root),
    )
    .unwrap_or_else(|| "<html><body>%locations%</body></html>".to_string())
    .replace("%locations%", &buttons);
    send_to_client(
        world,
        client_id,
        server_packets::npc_html_message(npc_object_id, &html),
    );
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
    let pay = should_pay_fee(world, level, object_id, holder, &loc);
    let fee = calculate_fee(world, level, object_id, holder, &loc);

    // A destination whose castle is under siege is refused outright
    // (`TeleportWhileSiegeInProgress` is False on this dist).
    if !world.cfg.character.teleport_while_siege_in_progress
        && any_siege_in_progress(world, &loc.castle_ids)
    {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(
                sm_ids::YOU_CANNOT_TELEPORT_TO_A_VILLAGE_THAT_IS_IN_A_SIEGE,
                &[],
            ),
        );
        return;
    }

    // NORMAL-list conditions (`doTeleport`'s isNormalTeleport block).
    if is_normal {
        // The gatekeeper's own castle under siege: the busy page.
        let npc_castle = npc_castle_id(world, npc_object_id);
        if !world.cfg.character.teleport_while_siege_in_progress
            && npc_castle.is_some_and(|id| any_siege_in_progress(world, &[id]))
        {
            send_teleporter_html(world, client_id, npc_object_id, "castleteleporter-busy.htm");
            return;
        }
        if !world.cfg.character.alt_karma_player_can_use_gk && reputation < 0 {
            send_message(world, client_id, "Go away, you're not welcome here.");
            return;
        }
        if has_combat_flag(world, object_id) {
            send_to_client(
                world,
                client_id,
                server_packets::system_message_with(
                    sm_ids::YOU_CANNOT_TELEPORT_WHILE_IN_POSSESSION_OF_A_WARD,
                    &[],
                ),
            );
            return;
        }
    }

    // Fee charge — Java `destroyItemByItemId` checks the full amount before
    // touching anything; `take_items` strips partial stacks, so pre-check.
    if pay && fee > 0 {
        let have = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&object_id)
            .map(|inv| inv.count_of(loc.fee_id))
            .unwrap_or(0);
        if have < fee || !items::take_items(world, client_id, object_id, loc.fee_id, fee) {
            if loc.fee_id == ADENA_ID {
                send_to_client(
                    world,
                    client_id,
                    server_packets::system_message_with(sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA, &[]),
                );
            } else {
                let item = world
                    .data
                    .item_data
                    .get(loc.fee_id)
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| format!("Unknown item: {}", loc.fee_id));
                send_message(world, client_id, &format!("You do not have enough {item}"));
            }
            return;
        }
    }

    // `!player.isAlikeDead()` → teleport.
    let dead = is_dead(world, object_id);
    if !dead {
        death::teleport_player(world, object_id, loc.x, loc.y, loc.z);
    }
}

/// `Teleporter.sendHtmlMessage`: a fixed page from `data/html/teleporter/`
/// with the `%objectId%`/`%npcname%` replacements.
pub(crate) fn send_teleporter_html(world: &World, client_id: u32, npc_object_id: i32, file: &str) {
    let name = npc_name_or_empty(world, npc_object_id);
    let html = crate::data::htm_cache::read_htm_for_client(
        world,
        client_id,
        format!("{}data/html/teleporter/{file}", world.data.root),
    )
    .unwrap_or_else(|| "<html><body>My Text is missing:<br></body></html>".to_string())
    .replace("%objectId%", &npc_object_id.to_string())
    .replace("%npcname%", &name);
    send_to_client(
        world,
        client_id,
        server_packets::npc_html_message(npc_object_id, &html),
    );
}

/// `Player.isNoble()` — nobless gates the `NOBLESS` teleport lists.
fn is_noble(world: &World, player_object_id: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::Player>(&player_object_id)
        .is_some_and(|p| p.is_noble)
}

/// `CastleManager.getCastle(npc)` for a gatekeeper — the castle whose
/// `SiegeZone` actually **contains** the gatekeeper, if any.
///
/// This is `CastleManager.getCastle(x, y, z)` (a `checkIfInZone` scan over the
/// castle list), **not** `Npc.getCastle()` / `findNearestCastle`: only
/// `Teleporter.showChatWindow` reaches for the strict containment form, and the
/// difference is load-bearing. `findNearestCastle` falls back to the closest
/// castle at *any* distance, so resolving through it makes every town
/// gatekeeper in the world (Roxxy in Talking Island, …) "stand on castle
/// ground" and answer with `castleteleporter-no.htm`.
fn npc_castle_id(world: &World, npc_object_id: i32) -> Option<i32> {
    let pos = world
        .objects
        .get_component::<crate::model::components::Position>(&npc_object_id)?;
    world
        .data
        .zone_data
        .siege_castle_at(pos.x, pos.y, pos.z)
        // Java iterates castles, so a `SiegeZone` carrying no castle id (the
        // stray later-chronicle `GainakSiege`) belongs to no castle.
        .filter(|&id| id > 0)
}

/// `Teleporter.showChatWindow`'s castle branch. `None` when the gatekeeper does
/// not stand on castle ground (Java falls back to `super.showChatWindow`);
/// otherwise the page file, relative to `data/html/teleporter/`, except the
/// owner case which returns `None` so the normal `<id>.htm` lookup runs.
pub(crate) fn castle_landing_page(
    world: &World,
    npc_object_id: i32,
    player_object_id: i32,
) -> Option<String> {
    let castle_id = npc_castle_id(world, npc_object_id)?;
    let clan_id = clan_of_or_zero(world, player_object_id);
    if clan_id != 0 && siege::owner_clan_id_opt(world, castle_id) == Some(clan_id) {
        return None; // the owner sees the gatekeeper's own page
    }
    Some(if any_siege_in_progress(world, &[castle_id]) {
        "castleteleporter-busy.htm".to_string()
    } else {
        "castleteleporter-no.htm".to_string()
    })
}
