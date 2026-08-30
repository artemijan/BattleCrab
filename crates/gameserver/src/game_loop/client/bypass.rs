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

use crate::game_loop::combat::target::can_interact;
use crate::game_loop::helpers::npc_template;
use crate::game_loop::helpers::send_action_failed;
use crate::game_loop::helpers::send_to_client;
use crate::game_loop::items::{augment, item_auction};
use crate::game_loop::npc::{teleporter, view};
use crate::model::components::LastFolkNpc;
use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::world::World;
use tracing::warn;

pub(crate) fn handle_request_bypass_to_server(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(command) = cp::read_bypass_command(body) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };

    if command.is_empty() {
        warn!("Bypass: client {client_id} sent empty bypass, dropped.");
        return;
    }

    if let Some(rest) = command.strip_prefix("npc_") {
        // `npc_<objectId>_<command>`: Java parses the id between the two
        // underscores and requires a command tail to act at all (the
        // ActionFailed terminator is sent regardless).
        if let Some((id_str, npc_command)) = rest.split_once('_')
            && let Ok(npc_object_id) = id_str.parse::<i32>()
            && world
                .objects
                .has_component::<crate::model::npc::Npc>(&npc_object_id)
            && can_interact(world, object_id, npc_object_id)
        {
            npc_bypass(world, client_id, object_id, npc_object_id, npc_command);
        }
        send_action_failed(world, client_id);
    } else if command == "Quest" || command.starts_with("Quest ") {
        // Bare quest link (`bypass -h Quest <Name> [<event>]`) — the form
        // the quest/script htmls use. Java recovers the NPC from the
        // validateHtmlAction origin; we use the last folk NPC (set on every
        // NPC click) and re-check range like Java does after validation.
        let Some(&LastFolkNpc(npc_object_id)) =
            world.objects.get_component::<LastFolkNpc>(&object_id)
        else {
            return;
        };
        if world
            .objects
            .has_component::<crate::model::npc::Npc>(&npc_object_id)
            && can_interact(world, object_id, npc_object_id)
        {
            npc_bypass(world, client_id, object_id, npc_object_id, &command);
        }
    } else if let Some(html_path) = command.strip_prefix("Link ") {
        let npc_object_id = world
            .objects
            .get_component::<LastFolkNpc>(&object_id)
            .map(|&LastFolkNpc(id)| id)
            .filter(|id| world.objects.has_component::<crate::model::npc::Npc>(id))
            .unwrap_or(0);
        handle_link(world, client_id, npc_object_id, html_path.trim());
    } else if command.starts_with("player_help ") {
        // `bypasshandlers/PlayerHelp` — the in-game help book. 92 pages under
        // `data/html/help/` link to each other through it, and the `Book` item
        // handler opens the first one.
        handle_player_help(world, client_id, &command);
    } else if command == "NpcViewMod" || command.starts_with("NpcViewMod ") {
        // `bypasshandlers/NpcViewMod`: the shift-click NPC info window's own
        // buttons (Show Drop / pages). Java resolves the target by object id
        // with no range check, so no `can_interact` gate here.
        view::handle_npc_view_bypass(world, client_id, object_id, &command);
    } else if command.starts_with("admin_") {
        // Admin HTML-menu buttons (Java `RequestBypassToServer`'s `admin_`
        // branch) → the same entry as the `//command` bar, confirm enabled.
        crate::game_loop::admin::use_admin_command(world, client_id, &command, true);
    } else if command.starts_with("sellbuff") {
        // `custom/SellBuff`'s bypass family — the buff shop's own menus. Java
        // registers these only when `SellBuffEnable` is on, which
        // `sell_buffs::handle_bypass` re-checks.
        let (cmd, rest) = command.split_once(' ').unwrap_or((command.as_str(), ""));
        crate::game_loop::commerce::sell_buffs::handle_bypass(
            world, client_id, object_id, cmd, rest,
        );
    } else if command.starts_with("_bbs") {
        // Community-board buttons (`RequestBypassToServer`'s
        // `isCommunityBoardCommand` branch) — home, buffs, heal, teleport, …
        crate::game_loop::community_board::handle_parse_command(world, client_id, &command);
    } else if command == "watchmatch" {
        // Olympiad spectate: the OlyManager's "watch a match" → the arena list.
        crate::game_loop::olympiad::send_match_list(world, client_id);
    } else if let Some(arg) = command.strip_prefix("arenachange ") {
        // Jump to (or between) an arena's spectator stand.
        if let Ok(arena) = arg.trim().parse::<i32>() {
            crate::game_loop::olympiad::enter_observer(world, client_id, object_id, arena);
        }
    } else if let Some(field) = command.strip_prefix("_olympiad?command=move_op_field&field=") {
        // The match-list window's arena buttons (Java translates `field N` to
        // `arenachange N-1`).
        if let Ok(n) = field.trim().parse::<i32>() {
            crate::game_loop::olympiad::enter_observer(world, client_id, object_id, n - 1);
        }
    } else if let Some(args) = command.strip_prefix("_diary") {
        // The hero-diary window (Java `Hero.showHeroDiary`): a hero-list link
        // `_diary?class=<classId>&page=<n>`.
        crate::game_loop::olympiad::show_hero_diary(world, client_id, object_id, args);
    } else if command.starts_with("manor_menu_select") {
        // The chamberlain's manor.html buttons (Java `RequestBypassToServer`'s
        // `manor_menu_select` branch → `OnNpcManorBypass`). The folk NPC and
        // interaction range are re-derived/re-checked inside.
        crate::game_loop::manor::handle_manor_menu_select(world, client_id, object_id, &command);
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
    "petmanager/evolve_no.htm",
    "petmanager/exchange_no.htm",
    "petmanager/instructions.htm",
    "petmanager/restore_no.htm",
    "warehouse/clanwh.htm",
    "warehouse/privatewh.htm",
];

/// Port of `bypasshandlers/Link.java`: serve a whitelisted `data/html/`
/// page through a plain `NpcHtmlMessage`. The dialog anchor (`%objectId%`
/// and the html window's owner) is `useBypass`'s `target` — the NPC the
/// bypass was invoked on for the `npc_<id>_Link` form, and the last clicked
/// NPC (Java: the `validateHtmlAction` origin, 0 when there is none) for the
/// bare form. The teleporter precaution is skipped (no teleporter pages are
/// in the whitelist).
fn handle_link(world: &mut World, client_id: u32, npc_object_id: i32, html_path: &str) {
    if html_path.is_empty() || html_path.contains("..") {
        warn!("Bypass: client {client_id} sent invalid link html [{html_path}].");
        return;
    }
    let content = if VALID_LINKS.contains(&html_path) {
        crate::data::htm_cache::read_htm_for_client(
            world,
            client_id,
            format!("{}data/html/{html_path}", world.data.root),
        )
    } else {
        None
    };
    let html = content
        .map(|c| c.replace("%objectId%", &npc_object_id.to_string()))
        .unwrap_or_default();
    send_to_client(
        world,
        client_id,
        server_packets::npc_html_message(npc_object_id, &html),
    );
}

/// Port of `bypasshandlers/PlayerHelp` — `bypass -h player_help <page>`.
///
/// The page name may carry an `#<itemId>` suffix, which Java turns into
/// `NpcHtmlMessage(0, itemId)`: an item-bound dialog the client does **not**
/// close when a button inside it is pressed, which is what lets the help book
/// page through its own "Next Page" links.
fn handle_player_help(world: &mut World, client_id: u32, command: &str) {
    // `command.substring(12)` — everything past `player_help `.
    let path = &command["player_help ".len()..];
    // Java's own traversal guard, verbatim.
    if path.is_empty() || path.contains("..") {
        return;
    }
    // `new StringTokenizer(path).nextToken()` — the first whitespace-delimited
    // token, then split on `#`.
    let token = path.split_whitespace().next().unwrap_or("");
    let (page, item_id) = match token.split_once('#') {
        Some((page, id)) => (page, id.parse::<i32>().unwrap_or(0)),
        None => (token, 0),
    };
    let html = crate::data::htm_cache::read_htm_for_client(
        world,
        client_id,
        format!("{}data/html/help/{page}", world.data.root),
    )
    .unwrap_or_default();
    send_to_client(
        world,
        client_id,
        server_packets::npc_html_message_item(0, item_id, &html),
    );
}

/// Port of `bypasshandlers/TerritoryStatus` — the "local lord and tax rate"
/// button. 254 htm files carry it: fishermen, pet managers, warehouse keepers
/// and the `default/` folk pages.
///
/// `npc.getCastle()` is `findNearestCastle`, **not** the siege zone the NPC
/// stands in — which is why a fisherman in the middle of a town can answer at
/// all. `nearest_castle_at` is that lookup.
fn handle_territory_status(world: &mut World, client_id: u32, npc_object_id: i32) {
    let Some(pos) = crate::game_loop::helpers::maybe_position(world, npc_object_id) else {
        return;
    };
    let Some(castle_id) = world.data.zone_data.nearest_castle_at(pos.x, pos.y, pos.z) else {
        return;
    };
    let owner = crate::game_loop::siege::owner_clan_id_opt(world, castle_id);
    let file = if owner.is_some() {
        "territorystatus.htm"
    } else {
        "territorynoclan.htm"
    };
    let Some(mut html) = crate::data::htm_cache::read_htm_for_client(
        world,
        client_id,
        format!("{}data/html/{file}", world.data.root),
    ) else {
        return;
    };
    if let Some(clan_id) = owner
        && let Some(clan) = world.clans.get(&clan_id)
    {
        let leader = clan
            .members
            .iter()
            .find(|m| m.char_id == clan.leader_id)
            .map(|m| m.name.clone())
            .unwrap_or_default();
        html = html
            .replace("%clanname%", &clan.name)
            .replace("%clanleadername%", &leader);
    }
    let castle_name = world
        .castle(castle_id)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    let tax = crate::game_loop::siege::treasury::tax_percent(
        world,
        castle_id,
        crate::game_loop::siege::treasury::TaxType::Buy,
    );
    // Castles 1..=6 are Aden, 7..=9 (Goddard, Rune, Schuttgart) are Elmore.
    let territory = if castle_id > 6 {
        "The Kingdom of Elmore"
    } else {
        "The Kingdom of Aden"
    };
    let html = html
        .replace("%castlename%", &castle_name)
        .replace("%taxpercent%", &tax.to_string())
        .replace("%objectId%", &npc_object_id.to_string())
        .replace("%territory%", territory);
    send_to_client(
        world,
        client_id,
        server_packets::npc_html_message(npc_object_id, &html),
    );
}

/// Port of `Npc.onBypassFeedback` + the `VillageMaster` override: route an
/// NPC-scoped command by its first token. The caller has already verified
/// the NPC exists and is within `INTERACTION_DISTANCE`.
fn npc_bypass(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    npc_object_id: i32,
    command: &str,
) {
    let raw_verb = command.split(' ').next().unwrap_or("");
    // `BypassHandler` registers every handler verb lower-cased *and* lower-cases
    // the incoming one before the map lookup, so the whole `bypasshandlers/` set
    // is case-insensitive in Java — and the dist leans on it. Fold any casing
    // back onto the spelling the arms below use; verbs answered by an NPC
    // subclass override (`VillageMaster`, `Teleporter`, `PetManager`,
    // `RaceManager`, `SymbolMaker`) are deliberately absent from the table
    // because Java matches those with a case-sensitive `startsWith`.
    let verb = canonical_handler_verb(raw_verb).unwrap_or(raw_verb);
    // Re-case the whole command too: the arms below (and the handlers they call)
    // cut their argument tail with `strip_prefix(verb)`, which would miss on a
    // differently-spelled original.
    let recased;
    let command = if verb == raw_verb {
        command
    } else {
        recased = format!("{verb}{}", &command[raw_verb.len()..]);
        recased.as_str()
    };
    match verb {
        "Quest" => crate::game_loop::quests::quest_link(
            world,
            client_id,
            object_id,
            npc_object_id,
            command,
        ),
        // `bypasshandlers/Link.java` in its NPC-scoped form: the fisherman
        // manuals, warehouse/pet-manager info pages, craft and skill-enchant
        // help. Java routes both `Link <page>` and `npc_<id>_Link <page>` to
        // the same handler; here the NPC is the one that was clicked.
        "Link" => {
            let html_path = command.strip_prefix("Link").unwrap_or("").trim();
            handle_link(world, client_id, npc_object_id, html_path);
        }
        // `bypasshandlers/TerritoryStatus.java` — "See the local lord and tax
        // rate", on 254 of the dist's folk pages.
        "TerritoryStatus" => handle_territory_status(world, client_id, npc_object_id),
        // `bypasshandlers/Observation.java` — the Broadcasting Tower's seats.
        "observe" | "observesiege" | "observeoracle" => {
            let args = command.strip_prefix(verb).unwrap_or("").trim();
            crate::game_loop::space::observation::handle_bypass(
                world,
                client_id,
                object_id,
                npc_object_id,
                verb,
                args,
            );
        }
        // `bypasshandlers/Loto.java` — the Lucky Lottery ticket seller dialog.
        "Loto" => crate::game_loop::activities::lottery::loto_bypass(
            world,
            client_id,
            object_id,
            npc_object_id,
            command,
        ),
        // `bypasshandlers/ItemAuctionLink.java` — the auctioneer NPC (G30.5).
        "ItemAuction" => {
            item_auction::link_bypass(world, client_id, object_id, npc_object_id, command)
        }
        // `RaceManager` NPC — the Monster Race Track betting dialog (G26.5).
        "BuyTicket" | "ShowOdds" | "ShowInfo" | "ShowTickets" | "ShowTicket" | "CalculateWin"
        | "ViewHistory" => crate::game_loop::activities::monster_race::race_bypass(
            world,
            client_id,
            object_id,
            npc_object_id,
            command,
        ),
        // `bypasshandlers/ChatLink.java`: the follow-up dialog pages every
        // folk html walks through (`Chat 1` → `<npcId>-1.htm`). Java parses
        // the tail with `Integer.parseInt`, falling back to page 0 — and a
        // `Chat 0` on an NPC with an `ON_NPC_FIRST_TALK` listener fires that
        // quest event instead of the static page.
        "Chat" => {
            let value = command
                .strip_prefix("Chat")
                .and_then(|s| s.trim().parse::<i32>().ok())
                .unwrap_or(0);
            let npc_id = world
                .objects
                .get_component::<crate::model::npc::Npc>(&npc_object_id)
                .map_or(0, |n| n.npc_id);
            if value == 0
                && crate::game_loop::quests::notify_first_talk(
                    world,
                    client_id,
                    object_id,
                    npc_object_id,
                    npc_id,
                )
            {
                return;
            }
            crate::game_loop::combat::target::show_chat_window(
                world,
                client_id,
                npc_object_id,
                value,
            );
        }
        // `VillageMaster.onBypassFeedback` verbs — gated on the instance
        // class like Java's subclass override (`type_name` check stands in
        // for `instanceof VillageMaster`).
        // `VillageMaster.onBypassFeedback`'s `Subclass` verb — the in-game
        // add/change flow over the G17 subclass mechanic.
        "Subclass" if is_village_master(world, npc_object_id) => {
            let args = command.strip_prefix("Subclass").unwrap_or("").trim();
            crate::game_loop::character::subclass::handle_village_master_bypass(
                world,
                client_id,
                object_id,
                npc_object_id,
                args,
            );
        }
        "create_clan" if is_village_master(world, npc_object_id) => {
            let args = command.strip_prefix("create_clan").unwrap_or("").trim();
            crate::game_loop::clans::handle_create_clan(world, client_id, object_id, args);
        }
        // `VillageMaster.onBypassFeedback`: leader-requested clan dissolution
        // (delayed 7 days) and its cancellation.
        "dissolve_clan" if is_village_master(world, npc_object_id) => {
            crate::game_loop::clans::handle_dissolve_clan(world, client_id, object_id);
        }
        "recover_clan" if is_village_master(world, npc_object_id) => {
            crate::game_loop::clans::handle_recover_clan(world, client_id, object_id);
        }
        // `VillageMaster`: clan level-up (SP + adena/Blood Mark ladder) and the
        // leader's learnable pledge-skill window.
        "increase_clan_level" if is_village_master(world, npc_object_id) => {
            crate::game_loop::clans::handle_increase_clan_level(world, client_id, object_id);
        }
        "learn_clan_skills" if is_village_master(world, npc_object_id) => {
            crate::game_loop::clans::show_pledge_skill_list(world, client_id, object_id);
        }
        // `VillageMaster`: the delegated leader transfer and its cancellation.
        // Delivery is `daily_tasks::clan_leader_apply`, gated on the weekly
        // (Wednesday) reset like Java's `DailyTaskManager.onReset`.
        "change_clan_leader" if is_village_master(world, npc_object_id) => {
            let args = command
                .strip_prefix("change_clan_leader")
                .unwrap_or("")
                .trim();
            crate::game_loop::clans::handle_change_clan_leader(
                world,
                client_id,
                object_id,
                npc_object_id,
                args,
            );
        }
        "cancel_clan_leader_change" if is_village_master(world, npc_object_id) => {
            crate::game_loop::clans::handle_cancel_clan_leader_change(
                world,
                client_id,
                object_id,
                npc_object_id,
            );
        }
        // `VillageMaster`: alliance creation/dissolution (`Clan.createAlly`/
        // `dissolveAlly`).
        "create_ally" if is_village_master(world, npc_object_id) => {
            let args = command.strip_prefix("create_ally").unwrap_or("").trim();
            crate::game_loop::clans::handle_create_ally(world, client_id, object_id, args);
        }
        "dissolve_ally" if is_village_master(world, npc_object_id) => {
            crate::game_loop::clans::handle_dissolve_ally(world, client_id, object_id);
        }
        // `VillageMaster`: sub-pledge (academy/royal-guard/knight-order)
        // creation, renaming, and captain assignment.
        "create_academy" if is_village_master(world, npc_object_id) => {
            let args = command.strip_prefix("create_academy").unwrap_or("").trim();
            crate::game_loop::clans::handle_create_academy(world, client_id, object_id, args);
        }
        "create_royal" if is_village_master(world, npc_object_id) => {
            let args = command.strip_prefix("create_royal").unwrap_or("").trim();
            crate::game_loop::clans::handle_create_royal(world, client_id, object_id, args);
        }
        "create_knight" if is_village_master(world, npc_object_id) => {
            let args = command.strip_prefix("create_knight").unwrap_or("").trim();
            crate::game_loop::clans::handle_create_knight(world, client_id, object_id, args);
        }
        "rename_pledge" if is_village_master(world, npc_object_id) => {
            let args = command.strip_prefix("rename_pledge").unwrap_or("").trim();
            crate::game_loop::clans::handle_rename_pledge(world, client_id, object_id, args);
        }
        "assign_subpl_leader" if is_village_master(world, npc_object_id) => {
            let args = command
                .strip_prefix("assign_subpl_leader")
                .unwrap_or("")
                .trim();
            crate::game_loop::clans::handle_assign_subpledge_leader(
                world, client_id, object_id, args,
            );
        }
        // `bypasshandlers/PrivateWarehouse.java` / `ClanWarehouse.java`: all
        // four open with `if (!Config.ALLOW_WAREHOUSE) return false;`, which
        // refuses the *bypass* — the keeper still shows the button and the
        // click does nothing, rather than the link disappearing.
        "WithdrawP" | "DepositP" | "WithdrawC" | "DepositC"
            if !world.cfg.general.allow_warehouse => {}
        "WithdrawP" => {
            crate::game_loop::commerce::warehouse::set_active(
                world,
                object_id,
                crate::model::components::ActiveWarehouse::Private,
            );
            crate::game_loop::commerce::warehouse::open_withdraw_window(world, client_id);
        }
        "DepositP" => {
            crate::game_loop::commerce::warehouse::set_active(
                world,
                object_id,
                crate::model::components::ActiveWarehouse::Private,
            );
            crate::game_loop::commerce::warehouse::open_deposit_window(world, client_id);
        }
        // `bypasshandlers/ClanWarehouse.java`: the shared clan warehouse.
        "WithdrawC" => {
            crate::game_loop::commerce::warehouse::open_clan(world, client_id, object_id, true)
        }
        "DepositC" => {
            crate::game_loop::commerce::warehouse::open_clan(world, client_id, object_id, false)
        }
        // `bypasshandlers/Freight.java`: the account-package warehouse — the
        // withdraw half, and the cross-character send (`package_deposit` →
        // `PackageToList` → `RequestPackageSend`).
        "package_withdraw" => {
            crate::game_loop::commerce::warehouse::open_freight_withdraw(world, client_id)
        }
        "package_deposit" => {
            crate::game_loop::commerce::warehouse::open_freight_send(world, client_id)
        }
        // `bypasshandlers/Augment.java`: `Augment 1` = make window, `Augment 2`
        // = cancel window.
        "Augment" => {
            let make = command
                .split(' ')
                .nth(1)
                .map(|a| a.trim() != "2")
                .unwrap_or(true);
            augment::open_window(world, client_id, make);
        }
        // `Teleporter.onBypassFeedback` (G15.5): the gatekeeper verbs —
        // list windows + the actual teleport, gated on the instance class
        // like Java's subclass override.
        "showTeleports" | "showTeleportsHunting" | "teleport" | "showNoblesSelect"
            if teleporter::is_teleporter(world, npc_object_id) =>
        {
            teleporter::handle_bypass(world, client_id, object_id, npc_object_id, command);
        }
        // `bypasshandlers/Buy.java`: merchants only.
        "Buy" if crate::game_loop::commerce::shop::is_merchant(world, npc_object_id) => {
            if let Some(list_id) = command
                .strip_prefix("Buy")
                .and_then(|s| s.trim().parse::<i32>().ok())
            {
                crate::game_loop::commerce::shop::show_buy_window(
                    world,
                    client_id,
                    object_id,
                    npc_object_id,
                    list_id,
                );
            }
        }
        // `bypasshandlers/SupportBlessing.java`: the Newbie Helper / Gatekeeper
        // Blessing of Protection (the `default/BlessingOfProtection.htm` button).
        "GiveBlessing" => crate::game_loop::npc::support_magic::give_blessing(
            world,
            client_id,
            object_id,
            npc_object_id,
        ),
        // `bypasshandlers/SupportMagic.java`: the newbie support buffs (the
        // `default/SupportMagic.htm` / `SupportMagicServitor.htm` buttons).
        "SupportMagic" => crate::game_loop::npc::support_magic::support_magic(
            world,
            client_id,
            object_id,
            npc_object_id,
            false,
        ),
        "SupportMagicServitor" => crate::game_loop::npc::support_magic::support_magic(
            world,
            client_id,
            object_id,
            npc_object_id,
            true,
        ),
        // `bypasshandlers/Multisell.java`: the exchange windows every merchant,
        // pet manager, fisherman and Mammon html opens (`multisell <id>` full /
        // `exc_multisell <id>` inventory-only). Java parses the id off a fixed
        // offset (`substring(9)`/`substring(13)`) — the token split is the same
        // cut, and a non-numeric tail is Java's swallowed `NumberFormatException`.
        "multisell" | "exc_multisell" => {
            if let Some(list_id) = command
                .split_once(' ')
                .and_then(|(_, rest)| rest.trim().parse::<i32>().ok())
            {
                crate::game_loop::commerce::multisell::separate_and_send(
                    world,
                    client_id,
                    object_id,
                    Some(npc_object_id),
                    list_id,
                    verb == "exc_multisell",
                );
            } else {
                warn!("Bypass: bad multisell command [{command}].");
            }
        }
        // `PetManager.onBypassFeedback` — the pet manager's three verbs. Its
        // `evolve.htm`/`exchange.htm` pages are already in the Link whitelist
        // above, so without these the buttons render and do nothing.
        "exchange" => crate::game_loop::servitor::evolve::handle_exchange(
            world,
            client_id,
            object_id,
            npc_object_id,
            command,
        ),
        "evolve" => crate::game_loop::servitor::evolve::handle_evolve(
            world,
            client_id,
            object_id,
            npc_object_id,
            command,
        ),
        "restore" => crate::game_loop::servitor::evolve::handle_restore(
            world,
            client_id,
            object_id,
            npc_object_id,
            command,
        ),
        // `ai/others/SymbolMaker`: the dye-symbol NPC's "Draw"/"Remove" buttons
        // (only that script's htms emit these verbs).
        "Draw" => crate::game_loop::character::henna::handle_item_list(world, client_id),
        "Remove" => crate::game_loop::character::henna::handle_remove_list(world, client_id),
        _ => {
            warn!("Bypass: unhandled npc bypass verb [{verb}] in [{command}].");
        }
    }
}

/// The `bypasshandlers/` verbs this module answers, in the spelling
/// [`npc_bypass`]'s match arms use. `BypassHandler.registerHandler` lower-cases
/// each `getBypassList()` entry and `getHandler` lower-cases the command, so a
/// html may spell them any way it likes — this dist does: Giran's luxury shop
/// (`30097` Galladucci / `30098` Alexandria) and six other merchant htmls emit
/// `Multisell`, and `ClanHallManager-10.html` emits `withdrawc`.
///
/// Verbs handled by an NPC `onBypassFeedback` override are *not* here: those go
/// through case-sensitive `startsWith` checks in Java, so they stay exact.
fn canonical_handler_verb(verb: &str) -> Option<&'static str> {
    const HANDLER_VERBS: &[&str] = &[
        "Quest",
        "Link",
        "Chat",
        "Loto",
        "ItemAuction",
        "TerritoryStatus",
        "observe",
        "observesiege",
        "observeoracle",
        "Buy",
        "Augment",
        "multisell",
        "exc_multisell",
        "WithdrawP",
        "DepositP",
        "WithdrawC",
        "DepositC",
        "package_withdraw",
        "package_deposit",
        "GiveBlessing",
        "SupportMagic",
        "SupportMagicServitor",
    ];
    HANDLER_VERBS
        .iter()
        .find(|v| v.eq_ignore_ascii_case(verb))
        .copied()
}

fn is_village_master(world: &World, npc_object_id: i32) -> bool {
    npc_template(world, npc_object_id).is_some_and(|t| t.type_name.starts_with("VillageMaster"))
}

/// `RequestLinkHtml` (0x22) — an html `<a action="link ...">`, which serves a
/// page straight out of `data/html/` instead of running a bypass command.
///
/// The two input guards are Java's and both matter: an empty link is dropped,
/// and a link containing `..` is refused before it can escape the html root.
///
/// The `validateHtmlAction` deviation documented at the top of this module
/// applies here too — Java recovers the origin NPC from the recorded action
/// and range-checks it, so this resolves [`LastFolkNpc`] and applies the same
/// `INTERACTION_DISTANCE` check that follows validation there.
pub(crate) fn handle_request_link_html(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = commons::network::PacketReader::new(body);
    let Some(link) = r.read_string() else {
        return;
    };
    if link.is_empty() {
        warn!("Player {player} sent empty html link!");
        return;
    }
    // Path traversal — Java's own check, and the only thing between the
    // client and the rest of the filesystem.
    if link.contains("..") {
        warn!("Player {player} sent invalid html link: link {link}");
        return;
    }
    // Java's origin id: 0 when the page came from no NPC, which skips the
    // range check entirely.
    let npc_object_id = world
        .objects
        .get_component::<LastFolkNpc>(&player)
        .map_or(0, |&LastFolkNpc(oid)| oid);
    if npc_object_id > 0 && !can_interact(world, player, npc_object_id) {
        // Java logs nothing here — "this could be a common case".
        return;
    }
    let path = format!("{}data/html/{link}", world.data.root);
    let Some(html) = crate::data::htm_cache::read_htm_for_client(world, client_id, path) else {
        warn!("Player {player} requested missing html link: {link}");
        return;
    };
    let html = html.replace("%objectId%", &npc_object_id.to_string());
    send_to_client(
        world,
        client_id,
        server_packets::npc_html_message(npc_object_id, &html),
    );
}
