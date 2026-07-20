//! `RequestBypassToServer` (0x23) routing — the server side of every HTML
//! `action="bypass -h …"` button. Port of
//! `clientpackets/RequestBypassToServer.runImpl` narrowed to the command
//! families this slice serves: `npc_<objectId>_<command>` (dialog verbs on a
//! specific NPC) and the bare `Quest …` links the quest htmls use.
//!
//! Deliberate deviations from Java (documented in the G11 plan):
//! - `validateHtmlAction` (the sent-action anti-cheat registry) is not
//!   ported. Bare commands resolve their NPC through the [`LastFolkNpc`]
//!   component instead of the recorded html origin id, and every route
//!   re-checks `INTERACTION_DISTANCE` — the same guard Java applies on top
//!   of validation.
//! - An empty bypass logs and drops instead of force-disconnecting (the
//!   G10 `Say2` precedent for malformed-but-harmless client input).
//! - Unhandled commands log-and-drop (Java logs too; `admin_`, `_bbs`,
//!   `item_`, menu/manor selects and the rest of the prefix zoo wait for
//!   their systems).

use tracing::warn;

use crate::model::components::LastFolkNpc;
use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

use super::target::can_interact;

pub(crate) fn handle_request_bypass_to_server(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(command) = cp::read_bypass_command(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();

    if command.is_empty() {
        warn!("Bypass: client {client_id} sent empty bypass, dropped.");
        return;
    }

    if let Some(rest) = command.strip_prefix("npc_") {
        // `npc_<objectId>_<command>`: Java parses the id between the two
        // underscores and requires a command tail to act at all (the
        // ActionFailed terminator is sent regardless).
        if let Some((id_str, npc_command)) = rest.split_once('_') {
            if let Ok(npc_object_id) = id_str.parse::<i32>() {
                if world.objects.has_component::<crate::model::npc::Npc>(&npc_object_id)
                    && can_interact(world, object_id, npc_object_id)
                {
                    npc_bypass(world, client_id, object_id, npc_object_id, npc_command);
                }
            }
        }
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
    } else if command == "Quest" || command.starts_with("Quest ") {
        // Bare quest link (`bypass -h Quest <Name> [<event>]`) — the form
        // the quest/script htmls use. Java recovers the NPC from the
        // validateHtmlAction origin; we use the last folk NPC (set on every
        // NPC click) and re-check range like Java does after validation.
        let Some(&LastFolkNpc(npc_object_id)) = world.objects.get_component::<LastFolkNpc>(&object_id)
        else {
            return;
        };
        if world.objects.has_component::<crate::model::npc::Npc>(&npc_object_id)
            && can_interact(world, object_id, npc_object_id)
        {
            npc_bypass(world, client_id, object_id, npc_object_id, &command);
        }
    } else if let Some(html_path) = command.strip_prefix("Link ") {
        handle_link(world, client_id, object_id, html_path.trim());
    } else if command == "NpcViewMod" || command.starts_with("NpcViewMod ") {
        // `bypasshandlers/NpcViewMod`: the shift-click NPC info window's own
        // buttons (Show Drop / pages). Java resolves the target by object id
        // with no range check, so no `can_interact` gate here.
        super::npc_view::handle_npc_view_bypass(world, client_id, object_id, &command);
    } else if command.starts_with("admin_") {
        // Admin HTML-menu buttons (Java `RequestBypassToServer`'s `admin_`
        // branch) → the same entry as the `//command` bar, confirm enabled.
        super::admin::use_admin_command(world, client_id, &command, true);
    } else if command.starts_with("_bbs") {
        // Community-board buttons (`RequestBypassToServer`'s
        // `isCommunityBoardCommand` branch) — home, buffs, heal, teleport, …
        super::community_board::handle_parse_command(world, client_id, &command);
    } else {
        warn!("Bypass: client {client_id} sent unhandled bypass [{command}].");
    }
}

/// The `Link.java` whitelist: only these files may be served through the
/// generic `Link <file>` bypass (everything else answers the empty-html
/// window, like Java's null content).
const VALID_LINKS: &[&str] = &[
    "common/craft_01.htm",
    "common/craft_02.htm",
    "common/skill_enchant_help_01.htm",
    "common/skill_enchant_help_02.htm",
    "common/skill_enchant_help_03.htm",
    "common/weapon_sa_01.htm",
    "default/BlessingOfProtection.htm",
    "default/SupportMagic.htm",
    "fisherman/exchange_old_items.htm",
    "fisherman/fish_appearance_exchange.htm",
    "fisherman/fishing_manual001.htm",
    "fisherman/fishing_manual002.htm",
    "fisherman/fishing_manual003.htm",
    "fisherman/fishing_manual004.htm",
    "fisherman/fishing_manual008.htm",
    "fisherman/fishing_manual009.htm",
    "fisherman/fishing_manual010.htm",
    "fortress/foreman.htm",
    "petmanager/evolve.htm",
    "petmanager/exchange.htm",
    "petmanager/instructions.htm",
    "warehouse/clanwh.htm",
    "warehouse/privatewh.htm",
];

/// Port of `bypasshandlers/Link.java`: serve a whitelisted `data/html/`
/// page through a plain `NpcHtmlMessage`. The dialog anchor (`%objectId%`
/// and the html window's owner) is the last clicked NPC — Java passes the
/// `validateHtmlAction` origin, 0 when there is none. The teleporter
/// precaution is skipped (no teleporter pages are in the whitelist).
fn handle_link(world: &mut World, client_id: u32, object_id: i32, html_path: &str) {
    if html_path.is_empty() || html_path.contains("..") {
        warn!("Bypass: client {client_id} sent invalid link html [{html_path}].");
        return;
    }
    let npc_object_id = world
        .objects
        .get_component::<LastFolkNpc>(&object_id)
        .map(|&LastFolkNpc(id)| id)
        .filter(|id| world.objects.has_component::<crate::model::npc::Npc>(id))
        .unwrap_or(0);
    let content = if VALID_LINKS.contains(&html_path) {
        crate::data::htm_cache::read_htm(format!("{}data/html/{html_path}", world.data.root))
    } else {
        None
    };
    let html = content
        .map(|c| c.replace("%objectId%", &npc_object_id.to_string()))
        .unwrap_or_default();
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(npc_object_id, &html));
    }
}

/// Port of `Npc.onBypassFeedback` + the `VillageMaster` override: route an
/// NPC-scoped command by its first token. The caller has already verified
/// the NPC exists and is within `INTERACTION_DISTANCE`.
fn npc_bypass(world: &mut World, client_id: u32, object_id: i32, npc_object_id: i32, command: &str) {
    let verb = command.split(' ').next().unwrap_or("");
    match verb {
        "Quest" => super::quests::quest_link(world, client_id, object_id, npc_object_id, command),
        // `bypasshandlers/ChatLink.java`: the follow-up dialog pages every
        // folk html walks through (`Chat 1` → `<npcId>-1.htm`). Java parses
        // the tail with `Integer.parseInt`, falling back to page 0.
        // TODO(G23): `Chat 0` on an NPC with an `ON_NPC_FIRST_TALK` listener
        // fires that quest event instead of the static page.
        "Chat" => {
            let value =
                command.strip_prefix("Chat").and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(0);
            super::target::show_chat_window(world, client_id, npc_object_id, value);
        }
        // `VillageMaster.onBypassFeedback` verbs — gated on the instance
        // class like Java's subclass override (`type_name` check stands in
        // for `instanceof VillageMaster`).
        // `VillageMaster.onBypassFeedback`'s `Subclass` verb — the in-game
        // add/change flow over the G17 subclass mechanic.
        "Subclass" if is_village_master(world, npc_object_id) => {
            let args = command.strip_prefix("Subclass").unwrap_or("").trim();
            super::subclass::handle_village_master_bypass(
                world,
                client_id,
                object_id,
                npc_object_id,
                args,
            );
        }
        "create_clan" if is_village_master(world, npc_object_id) => {
            let args = command.strip_prefix("create_clan").unwrap_or("").trim();
            super::clans::handle_create_clan(world, client_id, object_id, args);
        }
        // `VillageMaster.onBypassFeedback`: leader-requested clan dissolution
        // (delayed 7 days) and its cancellation.
        "dissolve_clan" if is_village_master(world, npc_object_id) => {
            super::clans::handle_dissolve_clan(world, client_id, object_id);
        }
        "recover_clan" if is_village_master(world, npc_object_id) => {
            super::clans::handle_recover_clan(world, client_id, object_id);
        }
        // `VillageMaster`: clan level-up (SP + adena/Blood Mark ladder) and the
        // leader's learnable pledge-skill window.
        "increase_clan_level" if is_village_master(world, npc_object_id) => {
            super::clans::handle_increase_clan_level(world, client_id, object_id);
        }
        "learn_clan_skills" if is_village_master(world, npc_object_id) => {
            super::clans::show_pledge_skill_list(world, client_id, object_id);
        }
        // `bypasshandlers/PrivateWarehouse.java`: the keeper's deposit/withdraw
        // windows (the bypass only appears on warehouse-keeper htmls).
        "WithdrawP" => {
            super::warehouse::set_active(world, object_id, crate::model::components::ActiveWarehouse::Private);
            super::warehouse::open_withdraw_window(world, client_id);
        }
        "DepositP" => {
            super::warehouse::set_active(world, object_id, crate::model::components::ActiveWarehouse::Private);
            super::warehouse::open_deposit_window(world, client_id);
        }
        // `bypasshandlers/ClanWarehouse.java`: the shared clan warehouse.
        "WithdrawC" => super::warehouse::open_clan(world, client_id, object_id, true),
        "DepositC" => super::warehouse::open_clan(world, client_id, object_id, false),
        // `bypasshandlers/Freight.java`: the account-package warehouse. Only
        // the withdraw half is wired; `package_deposit` (the cross-character
        // send) needs the account char list and offline freight writes.
        "package_withdraw" => super::warehouse::open_freight_withdraw(world, client_id),
        // `bypasshandlers/Augment.java`: `Augment 1` = make window, `Augment 2`
        // = cancel window.
        "Augment" => {
            let make = command.split(' ').nth(1).map(|a| a.trim() != "2").unwrap_or(true);
            super::augment::open_window(world, client_id, make);
        }
        // `Teleporter.onBypassFeedback` (G15.5): the gatekeeper verbs —
        // list windows + the actual teleport, gated on the instance class
        // like Java's subclass override.
        "showTeleports" | "showTeleportsHunting" | "teleport" | "showNoblesSelect"
            if super::teleporter::is_teleporter(world, npc_object_id) =>
        {
            super::teleporter::handle_bypass(world, client_id, object_id, npc_object_id, command);
        }
        // `bypasshandlers/Buy.java`: merchants only.
        "Buy" if super::shop::is_merchant(world, npc_object_id) => {
            if let Some(list_id) =
                command.strip_prefix("Buy").and_then(|s| s.trim().parse::<i32>().ok())
            {
                super::shop::show_buy_window(world, client_id, object_id, npc_object_id, list_id);
            }
        }
        // `bypasshandlers/SupportBlessing.java`: the Newbie Helper / Gatekeeper
        // Blessing of Protection (the `default/BlessingOfProtection.htm` button).
        "GiveBlessing" => super::support_magic::give_blessing(world, client_id, object_id, npc_object_id),
        // `bypasshandlers/SupportMagic.java`: the newbie support buffs (the
        // `default/SupportMagic.htm` / `SupportMagicServitor.htm` buttons).
        "SupportMagic" => super::support_magic::support_magic(world, client_id, object_id, npc_object_id, false),
        "SupportMagicServitor" => {
            super::support_magic::support_magic(world, client_id, object_id, npc_object_id, true)
        }
        // `ai/others/SymbolMaker`: the dye-symbol NPC's "Draw"/"Remove" buttons
        // (only that script's htms emit these verbs).
        "Draw" => super::henna::handle_item_list(world, client_id),
        "Remove" => super::henna::handle_remove_list(world, client_id),
        _ => {
            warn!("Bypass: unhandled npc bypass verb [{verb}] in [{command}].");
        }
    }
}

fn is_village_master(world: &World, npc_object_id: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_object_id)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.type_name.starts_with("VillageMaster"))
}
