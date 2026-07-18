//! Community board (BBS) — the runtime side of `CommunityBoardHandler` plus
//! the `HomeBoard` handler from `dist/game/data/scripts/handlers/communityboard`.
//!
//! This dist runs `CustomCommunityBoard = True`, so the live board is the
//! custom navigation board. Ported so far:
//!   - the board window plumbing (`RequestShowBoard` → `_bbshome`, the
//!     `_bbs*` bypass routing, and the chunked `ShowBoard` sender);
//!   - `HomeBoard` home rendering (`_bbshome`/`_bbstop`) with the navigation
//!     panel injected;
//!   - the cheap, already-portable actions: `_bbsheal`, `_bbsteleport`,
//!     `_bbsbuff` (direct buff list);
//!   - `_bbspremium` — buy account premium (reuses the `PremiumManager` store
//!     also driving `//premium_*`);
//!   - the scheme buffer (`_bbs_buff_scheme_create`/`_delete`/`_execute`),
//!     backed by the `buffer_schemes` table + the `SchemeBufferSkills.xml`
//!     available-buff levels.
//!
//! Deferred with `TODO(G30)` at each site (needs subsystems not ported yet):
//! the merchant multisell/sell (`_bbsmultisell`/`_bbsexcmultisell`/`_bbssell`
//! — no multisell/buy-list system); drop-search (`_bbs_search_*` — needs the
//! item-icon data + a `RadarControl` packet); `_bbsdelevel` (config-off in the
//! dist); and the retail forum boards (`_bbsloc`/`_bbsclan`/`_bbsmail`/…),
//! which the custom navigation never links to anyway.

use tracing::warn;

use crate::model::components::Casting;
use crate::model::inventory::Inventory;
use crate::model::Player;
use crate::network::server_packets as sp;
use crate::session::ClientSession;
use crate::world::World;

/// `Util.sendCBHtml`'s single-packet chunk size (client reassembles 101/102/103).
const CHUNK: usize = 16250;

/// `Config.BUFFER_MAX_SCHEMES` from `config/Custom/SchemeBuffer.ini`
/// (`BufferMaxSchemesPerChar = 5`). Inlined like the premium flag — a dedicated
/// SchemeBuffer.ini loader isn't ported and the dist value is authoritative.
const MAX_SCHEMES: usize = 5;

/// Entry point for `RequestShowBoard` and every `_bbs*` bypass — port of
/// `CommunityBoardHandler.handleParseCommand` + `HomeBoard.parseCommunityBoardCommand`.
pub(crate) fn handle_parse_command(world: &mut World, client_id: u32, command: &str) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();

    if !world.cfg.community_board.enabled {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(sp::system_message_with(
                sp::sm_ids::THE_COMMUNITY_SERVER_IS_CURRENTLY_OFFLINE,
                &[],
            ));
        }
        return;
    }

    // `HomeBoard.COMBAT_CHECK`: the custom action commands are refused while the
    // player is busy. The gate that exists here is a subset — casting / pvp flag
    // / dead. TODO(G30): duel, olympiad, SIEGE/PVP zones and event state once
    // those exist (Java also checks `isInDuel`/`isInOlympiadMode`/`isOnEvent`).
    if is_custom_action(command) && is_busy(world, object_id) {
        send_message(world, client_id, "You can't use the Community Board right now.");
        return;
    }

    // `HomeBoard.KARMA_CHECK`.
    if world.cfg.community_board.karma_disabled && reputation(world, object_id) < 0 {
        send_message(world, client_id, "Players with Karma cannot use the Community Board.");
        return;
    }

    match first_token(command) {
        "_bbshome" | "_bbstop" => show_home(world, client_id, object_id, command),
        "_bbsheal" => do_heal(world, client_id, object_id, command),
        "_bbsteleport" => do_teleport(world, client_id, object_id, command),
        "_bbsbuff" => do_buff(world, client_id, object_id, command),
        "_bbspremium" => do_premium(world, client_id, object_id, command),
        "_bbs_buff_scheme_create" | "_bbs_buff_scheme_delete" | "_bbs_buff_scheme_execute" => {
            do_scheme(world, client_id, object_id, command)
        }
        // TODO(G30): the merchant `_bbsmultisell`/`_bbsexcmultisell`/`_bbssell`
        // need the multisell + buy-list systems; the `_bbs_search_*` drop-search
        // needs item-icon data + a `RadarControl` packet; `_bbsdelevel` is
        // config-disabled in the dist. Each is a `HomeBoard`/`DropSearchBoard`
        // branch in the Java source.
        other => {
            warn!("CommunityBoard: unhandled/unported command [{other}] (full: [{command}]).");
        }
    }
}

/// Port of `CommunityBoardHandler.handleWriteCommand` — the `RequestBBSwrite`
/// submit. Java maps `url` → a `_bbs*` write command (Topic/Region/Notice),
/// all of which target the retail forum boards that are deferred here, so we
/// answer Java's "not implemented yet" page for every `url`.
pub(crate) fn handle_write_command(world: &mut World, client_id: u32, url: &str) {
    if !world.cfg.community_board.enabled {
        return;
    }
    // TODO(G30): Topic/Post/Region/Notice map to the forum boards (`_bbstop`/
    // `_bbsloc`/`_bbsclan`) — port when those boards land.
    let html = format!(
        "<html><body><br><br><center>The command: {url} is not implemented yet.</center><br><br></body></html>"
    );
    send_cb_html(world, client_id, &html);
}

/// `HomeBoard`'s `_bbshome`/`_bbstop` branch: load the home page (custom or
/// retail) and inject the navigation panel.
fn show_home(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    let custom = world.cfg.community_board.custom_enabled;
    let root = &world.data.root;

    // `_bbstop;<page>.html` serves a Custom sub-page (the nav buttons post
    // back through this); bare `_bbshome`/`_bbstop` is the landing page.
    let page = command.strip_prefix("_bbstop;").filter(|p| p.ends_with(".html"));
    let rel = match page {
        Some(p) if custom => format!("data/html/CommunityBoard/Custom/{p}"),
        Some(p) => format!("data/html/CommunityBoard/{p}"),
        None if custom => "data/html/CommunityBoard/Custom/home.html".to_string(),
        None => "data/html/CommunityBoard/home.html".to_string(),
    };

    let Some(mut html) = read_html(root, &rel) else {
        warn!("CommunityBoard: missing html [{rel}].");
        return;
    };

    if custom {
        html = finalize_custom(world, object_id, html, "");
    } else {
        // Retail home counters. TODO(G30): real favorite/region counts (need the
        // `bbs_favorites` table + region registration) and clan count.
        html = html.replace("%fav_count%", "0");
        html = html.replace("%region_count%", "0");
        html = html.replace("%clan_count%", "0");
    }

    send_cb_html(world, client_id, &html);
}

/// `HomeBoard`'s `_bbsheal;<page>` branch: full HP/MP/CP restore, then re-render
/// the page. Reuses the `//heal` primitive.
fn do_heal(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    if !world.cfg.community_board.enable_heal {
        return;
    }
    let price = world.cfg.community_board.heal_price;
    if !charge(world, client_id, object_id, price) {
        return;
    }
    super::admin::vitals::heal_creature(world, object_id);
    // TODO(G30): Java also restores the player's pet/servitors (not summonable
    // until G29).
    send_message(world, client_id, "You used heal!");
    serve_page(world, client_id, object_id, command.strip_prefix("_bbsheal;"), "");
}

/// `HomeBoard`'s `_bbsteleport;<x> <y> <z>` branch: charge, hide the board and
/// teleport to the whitelisted destination. Reuses the gatekeeper teleport
/// primitive.
fn do_teleport(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    if !world.cfg.community_board.enable_teleports {
        return;
    }
    let Some(key) = command.strip_prefix("_bbsteleport;") else { return };
    let Some(&(x, y, z)) = world.cfg.community_board.available_teleports.get(key.trim()) else {
        warn!("CommunityBoard: teleport [{key}] not in the gatekeeper whitelist.");
        return;
    };
    let price = world.cfg.community_board.teleport_price;
    if !charge(world, client_id, object_id, price) {
        return;
    }
    // Java hides the board (`new ShowBoard()`) and `disableAllSkills()` for 3 s
    // around the teleport. TODO(G30): the temporary skill lock (needs a timed
    // re-enable task) — the teleport itself is the observable effect.
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(sp::show_board_hide());
    }
    let dead = world
        .objects
        .get_component::<crate::model::components::Vitals>(&object_id)
        .is_none_or(|v| v.dead);
    if !dead {
        super::death::teleport_player(world, object_id, x, y, z);
    }
}

/// `HomeBoard`'s `_bbsbuff;<id,lvl>;…;<page>` branch: apply each whitelisted
/// buff to the player, then re-render the page. Reuses the effect engine.
fn do_buff(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    if !world.cfg.community_board.enable_buffs {
        return;
    }
    let body = command.strip_prefix("_bbsbuff;").unwrap_or("");
    let parts: Vec<&str> = body.split(';').collect();
    // Last token is the return page; the rest are `id,level` pairs.
    let (page, buffs) = parts.split_last().map(|(p, b)| (Some(*p), b)).unwrap_or((None, &[]));

    let price = world.cfg.community_board.buff_price * buffs.len() as i64;
    if !charge(world, client_id, object_id, price) {
        return;
    }

    for spec in buffs {
        let mut it = spec.split(',');
        let (Some(id), Some(lvl)) = (
            it.next().and_then(|s| s.trim().parse::<i32>().ok()),
            it.next().and_then(|s| s.trim().parse::<i32>().ok()),
        ) else {
            continue;
        };
        if !world.cfg.community_board.available_buffs.contains(&id) {
            continue; // anti-exploit whitelist (Java `COMMUNITY_AVAILABLE_BUFFS`)
        }
        let Some(skill) = world.data.skill_data.get(id, lvl).cloned() else {
            warn!("CommunityBoard: buff skill {id}/{lvl} missing from skill data.");
            continue;
        };
        // Self-cast, no MP/cast bar (Java `skill.applyEffects(player, player)`).
        // TODO(G30): pet/servitor targets + the `CommunityCastAnimations`
        // `MagicSkillUse` broadcast (summons land in G29).
        crate::game_loop::skills::effects::apply_skill_effects(world, object_id, object_id, &skill);
    }
    serve_page(world, client_id, object_id, page, "");
}

/// `HomeBoard`'s `_bbspremium;<days>` branch: buy `<days>` (1–30) days of
/// account premium at `premium_price_per_day` each, then serve the thank-you
/// page. Reuses the `PremiumManager` store already ported for `//premium_*`.
fn do_premium(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    use super::admin::premium;
    // `HomeBoard.CUSTOM_COMMANDS` only registers `_bbspremium` when both the
    // global premium system and the community premium option are on.
    if !premium::PREMIUM_SYSTEM_ENABLED || !world.cfg.community_board.community_premium_system {
        return;
    }
    // `_bbspremium;<days>` → Java splits the tail on `,` and takes the first field.
    let days: i64 = command
        .strip_prefix("_bbspremium;")
        .and_then(|t| t.split(',').next())
        .and_then(|d| d.trim().parse().ok())
        .unwrap_or(0);
    let price = world.cfg.community_board.premium_price_per_day.saturating_mul(days);
    // Java folds the range check into the "Not enough currency!" guard.
    if !(1..=30).contains(&days) {
        send_message(world, client_id, "Not enough currency!");
        return;
    }
    let coin = world.cfg.community_board.premium_coin_id;
    if !charge_item(world, client_id, object_id, coin, price) {
        return;
    }

    let Some(account) = account_of(world, client_id) else { return };
    let enddate = premium::add_premium_time(world, &account, days * premium::DAY_MILLIS);
    send_message(
        world,
        client_id,
        &format!(
            "Your account will now have premium status until {}.",
            premium::format_datetime(enddate)
        ),
    );
    // TODO(G16): Java also runs `PcCafePointsManager.run(player)` here when
    // `Config.PC_CAFE_RETAIL_LIKE` (that manager is unported).
    serve_page(world, client_id, object_id, Some("premium/thankyou.html"), "");
}

/// `HomeBoard`'s `_bbs_buff_scheme_*` branch: create a scheme from the player's
/// active buffs, delete one, or execute (re-cast) one, then re-render the
/// return page with any validation error banner. The bypass carries
/// space-separated args: `<cmd> <name> <returnPath> [self|pet]`.
fn do_scheme(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    // Return page: `parts[2]` when present, else `parts[1]` (Java `parts.length
    // < 3`); only if it names an html.
    let return_path = if parts.len() < 3 { parts.get(1) } else { parts.get(2) }
        .copied()
        .filter(|p| p.ends_with(".html"));

    // Java loads the return html first, runs the command (which may set an error
    // message), then re-renders — so we always serve the return page.
    let error = run_scheme_command(world, client_id, object_id, &parts).err().unwrap_or_default();
    serve_page(world, client_id, object_id, return_path, &error);
}

/// Port of `HomeBoard.parseSchemeNameOrError` + the create/delete/execute
/// dispatch. `Err(msg)` becomes the `%errorMessage%` banner.
fn run_scheme_command(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    parts: &[&str],
) -> Result<(), String> {
    if parts.len() < 3 {
        return Err("Please enter scheme name.".to_string());
    }
    let command_name = parts[0];
    let scheme_name = parts[1];
    if scheme_name.chars().count() > 14 {
        return Err("Scheme's name must contain up to 14 chars.".to_string());
    }
    if !is_alphanumeric(scheme_name) {
        return Err("Please use plain alphanumeric characters.".to_string());
    }
    if command_name == "_bbs_buff_scheme_create" {
        if let Some(schemes) = world.buffer_schemes.get(&object_id) {
            if schemes.len() >= MAX_SCHEMES {
                return Err("Maximum schemes amount is already reached.".to_string());
            }
            if schemes.iter().any(|(n, _)| n.eq_ignore_ascii_case(scheme_name)) {
                return Err("The scheme name already exists.".to_string());
            }
        }
    }

    match command_name {
        "_bbs_buff_scheme_create" => scheme_create(world, object_id, scheme_name),
        "_bbs_buff_scheme_delete" => {
            scheme_delete(world, object_id, scheme_name);
            Ok(())
        }
        "_bbs_buff_scheme_execute" => {
            let is_pet = parts.get(3) == Some(&"pet");
            apply_scheme(world, client_id, object_id, scheme_name, is_pet)
        }
        _ => Ok(()),
    }
}

/// Java create branch: snapshot the player's currently-active whitelisted buffs
/// into a new scheme, write it through to `buffer_schemes`.
fn scheme_create(world: &mut World, object_id: i32, scheme_name: &str) -> Result<(), String> {
    let buffs: Vec<i32> = world
        .objects
        .get_component::<crate::model::components::Buffs>(&object_id)
        .map(|b| {
            b.0.iter()
                .map(|a| a.skill_id)
                .filter(|id| world.cfg.community_board.available_buffs.contains(id))
                .collect()
        })
        .unwrap_or_default();
    if buffs.is_empty() {
        return Err("You don't have any buffs applied.".to_string());
    }
    let skills = buffs.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
    world.buffer_schemes.entry(object_id).or_default().push((scheme_name.to_string(), buffs));
    let _ = world.db.send(crate::db::DbCommand::StoreBufferScheme {
        object_id,
        scheme_name: scheme_name.to_string(),
        skills,
    });
    Ok(())
}

/// Java `removeScheme` + the shutdown save collapse into an immediate delete.
fn scheme_delete(world: &mut World, object_id: i32, scheme_name: &str) {
    if let Some(schemes) = world.buffer_schemes.get_mut(&object_id) {
        schemes.retain(|(n, _)| !n.eq_ignore_ascii_case(scheme_name));
    }
    let _ = world.db.send(crate::db::DbCommand::DeleteBufferScheme {
        object_id,
        scheme_name: scheme_name.to_string(),
    });
}

/// Port of `HomeBoard.applyBuffs`: re-cast every skill in a scheme onto the
/// player, at the level from the buffer's available-buff table.
fn apply_scheme(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    scheme_name: &str,
    is_pet: bool,
) -> Result<(), String> {
    let scheme: Vec<i32> = world
        .buffer_schemes
        .get(&object_id)
        .and_then(|s| s.iter().find(|(n, _)| n.eq_ignore_ascii_case(scheme_name)))
        .map(|(_, skills)| skills.clone())
        .unwrap_or_default();

    // TODO(G29): pets/servitors aren't summonable yet, so the "Pet" button
    // always lands on Java's `player.getPet() == null && !player.hasServitors()`
    // branch. Once summons exist, apply to pet + servitors here.
    if is_pet {
        return Err("You don't have a pet.".to_string());
    }

    let buff_price = world.cfg.community_board.buff_price;
    let cost = if buff_price > 0 { buff_price * scheme.len() as i64 } else { 0 };
    // NOTE: Java's guard is `(cost == 0) || inventoryCount < cost` — an inverted
    // check that applies the scheme for free (dist `BuffPrice = 0`) and, were the
    // price ever positive, would refuse only when the player CAN pay. Ported
    // faithfully ("dist data is the spec"); with the dist price of 0, `cost` is
    // always 0 so the buffs always apply.
    let currency = world.cfg.community_board.currency_id;
    let have = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map(|inv| inv.count_of(currency))
        .unwrap_or(0);
    if !(cost == 0 || have < cost) {
        return Err("You don't have enough items for this action.".to_string());
    }
    if cost > 0 {
        // Java `destroyItemByItemId("CB_Buff", CURRENCY, cost, …)` — a no-op in
        // dist; best-effort, never blocks the (already faithful) apply.
        charge(world, client_id, object_id, cost);
    }

    for skill_id in &scheme {
        if !world.cfg.community_board.available_buffs.contains(skill_id) {
            continue;
        }
        let Some(level) = world.data.scheme_buffer.level_of(*skill_id) else { continue };
        let Some(skill) = world.data.skill_data.get(*skill_id, level).cloned() else {
            warn!("CommunityBoard: scheme buff {skill_id}/{level} missing from skill data.");
            continue;
        };
        crate::game_loop::skills::effects::apply_skill_effects(world, object_id, object_id, &skill);
    }
    Ok(())
}

/// Java `Util.isAlphaNumeric` — non-empty and every char a letter or digit.
fn is_alphanumeric(s: &str) -> bool {
    !s.is_empty() && s.chars().all(char::is_alphanumeric)
}

/// Re-render a Custom sub-page after an action (the `page` tail the action
/// bypasses carry, e.g. `buffer/main` or `buffer/schemes.html`), with an
/// optional error banner. No-op if the tail is missing.
fn serve_page(world: &mut World, client_id: u32, object_id: i32, page: Option<&str>, error_message: &str) {
    let Some(page) = page.filter(|p| !p.is_empty()) else { return };
    // Java's `_bbsbuff`/`_bbsheal` pass a bare tail (`buffer/main`) and append
    // ".html" unconditionally; the scheme/premium paths already carry it.
    let file = if page.ends_with(".html") { page.to_string() } else { format!("{page}.html") };
    let rel = format!("data/html/CommunityBoard/Custom/{file}");
    let Some(html) = read_html(&world.data.root, &rel) else {
        warn!("CommunityBoard: missing sub-page [{rel}].");
        return;
    };
    let html = finalize_custom(world, object_id, html, error_message);
    send_cb_html(world, client_id, &html);
}

/// Port of `HomeBoard`'s post-build custom substitution: inject the navigation
/// panel, the `%errorMessage%` banner, and the `%schemenames%` scheme buttons.
fn finalize_custom(world: &World, object_id: i32, html: String, error_message: &str) -> String {
    let navigation =
        read_html(&world.data.root, "data/html/CommunityBoard/Custom/navigation.html").unwrap_or_default();
    let mut html = html.replace("%navigation%", &navigation).replace("%errorMessage%", error_message);
    if html.contains("%schemenames%") {
        html = html.replace("%schemenames%", &render_scheme_names(world, object_id));
    }
    html
}

/// The `%schemenames%` block: the player's scheme rows, or Java's empty-state
/// line when the player has no schemes registered (`getPlayerSchemes == null`).
fn render_scheme_names(world: &World, object_id: i32) -> String {
    match world.buffer_schemes.get(&object_id) {
        Some(schemes) => build_scheme_html(schemes),
        None => {
            "No buffer schemes yet, please make sure you have buffs and then click Create Scheme.".to_string()
        }
    }
}

/// Java `HomeBoard.buildBufferSchemesHtml`: one execute/pet/delete button row
/// per scheme, names sorted case-insensitively (Java iterates a
/// `TreeMap(CASE_INSENSITIVE_ORDER)`).
fn build_scheme_html(schemes: &[(String, Vec<i32>)]) -> String {
    const ROW: &str = concat!(
        "<td><button value=\"%schemename%\" action=\"bypass _bbs_buff_scheme_execute %schemename% buffer/schemes.html self\" height=\"26\" width=\"130\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\" /></td>",
        "<td><button value=\"%schemename% (Pet)\" action=\"bypass _bbs_buff_scheme_execute %schemename% buffer/schemes.html pet\" height=\"26\" width=\"130\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\" /></td>",
        "<td><button value=\"X\" action=\"bypass _bbs_buff_scheme_delete %schemename% buffer/schemes.html\" height=\"26\" width=\"26\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\" /></td>",
    );
    let mut names: Vec<&str> = schemes.iter().map(|(n, _)| n.as_str()).collect();
    names.sort_by_key(|n| n.to_lowercase());
    let mut out = String::from("<table align=\"center\">");
    for name in names {
        out.push_str("<tr>");
        out.push_str(&ROW.replace("%schemename%", name));
        out.push_str("</tr>");
    }
    out.push_str("</table>");
    out
}

// --- helpers ---------------------------------------------------------------

/// The `_bbs*` commands `HomeBoard.CUSTOM_COMMANDS` guards behind the combat
/// check (everything but `_bbshome`/`_bbstop`).
fn is_custom_action(command: &str) -> bool {
    let c = first_token(command);
    matches!(
        c,
        "_bbspremium"
            | "_bbsexcmultisell"
            | "_bbsmultisell"
            | "_bbssell"
            | "_bbsteleport"
            | "_bbsbuff"
            | "_bbsheal"
            | "_bbsdelevel"
    ) || c.starts_with("_bbs_buff_scheme")
}

/// The command word up to the first `;` or space (`_bbsteleport;1 2 3` →
/// `_bbsteleport`).
fn first_token(command: &str) -> &str {
    command.split([';', ' ']).next().unwrap_or(command)
}

/// The subset of `isInCombat()`-style busy states currently modeled.
fn is_busy(world: &World, object_id: i32) -> bool {
    let casting = world.objects.has_component::<Casting>(&object_id);
    let pvp = world
        .objects
        .get_component::<crate::model::components::PvpState>(&object_id)
        .is_some_and(|s| s.flag > 0);
    let dead = world
        .objects
        .get_component::<crate::model::components::Vitals>(&object_id)
        .is_some_and(|v| v.dead);
    casting || pvp || dead
}

fn reputation(world: &World, object_id: i32) -> i32 {
    world.objects.get_component::<Player>(&object_id).map(|p| p.reputation).unwrap_or(0)
}

/// The account name behind a client (Java `player.getAccountName()`), for the
/// account-scoped premium store.
fn account_of(world: &World, client_id: u32) -> Option<String> {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => Some(s.account().to_string()),
        _ => None,
    }
}

/// `player.destroyItemByItemId(currency, price)` on the board currency with the
/// "not enough" guard.
fn charge(world: &mut World, client_id: u32, object_id: i32, price: i64) -> bool {
    let currency = world.cfg.community_board.currency_id;
    charge_item(world, client_id, object_id, currency, price)
}

/// `player.destroyItemByItemId(item_id, price)` with the "not enough" guard.
/// A zero/negative price is free (no inventory touch). Returns whether the
/// action may proceed.
fn charge_item(world: &mut World, client_id: u32, object_id: i32, item_id: i32, price: i64) -> bool {
    if price <= 0 {
        return true;
    }
    let have = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map(|inv| inv.count_of(item_id))
        .unwrap_or(0);
    if have < price || !super::quests::take_items(world, client_id, object_id, item_id, price) {
        send_message(world, client_id, "Not enough currency!");
        return false;
    }
    true
}

fn read_html(root: &str, rel: &str) -> Option<String> {
    std::fs::read_to_string(format!("{root}{rel}")).ok()
}

fn send_message(world: &World, client_id: u32, text: &str) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(sp::system_message_with(
            sp::sm_ids::S1_TEXT,
            &[sp::SmParam::Text(text.to_string())],
        ));
    }
}

/// Port of `Util.sendCBHtml`: split the html into ≤3 chunks tagged 101/102/103
/// and send each as a `ShowBoard`. Split by char boundaries (htmls are ASCII,
/// so this matches Java's UTF-16-length branches for the content we serve).
fn send_cb_html(world: &World, client_id: u32, html: &str) {
    let Some(cs) = world.clients.get(&client_id) else { return };
    for packet in build_cb_packets(html) {
        cs.send(packet);
    }
}

/// The chunk packets for one board html (pure, so it's unit-testable).
fn build_cb_packets(html: &str) -> Vec<Vec<u8>> {
    let chars: Vec<char> = html.chars().collect();
    let n = chars.len();
    let take = |from: usize, to: usize| -> String { chars[from..to.min(n)].iter().collect() };
    if n < CHUNK {
        vec![
            sp::show_board("101", Some(html)),
            sp::show_board("102", None),
            sp::show_board("103", None),
        ]
    } else if n < CHUNK * 2 {
        vec![
            sp::show_board("101", Some(&take(0, CHUNK))),
            sp::show_board("102", Some(&take(CHUNK, n))),
            sp::show_board("103", None),
        ]
    } else if n < CHUNK * 3 {
        vec![
            sp::show_board("101", Some(&take(0, CHUNK))),
            sp::show_board("102", Some(&take(CHUNK, CHUNK * 2))),
            sp::show_board("103", Some(&take(CHUNK * 2, n))),
        ]
    } else {
        vec![
            sp::show_board(
                "101",
                Some("<html><body><br><center>Error: HTML was too long!</center></body></html>"),
            ),
            sp::show_board("102", None),
            sp::show_board("103", None),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decode a `ShowBoard` packet's content string (skip the opcode, the
    /// show/hide byte and the 8 fixed nav strings).
    fn cb_content(pkt: &[u8]) -> String {
        assert_eq!(pkt[0], sp::opcodes::SHOW_BOARD);
        let mut r = commons::network::PacketReader::new(&pkt[2..]);
        for _ in 0..8 {
            r.read_string().unwrap();
        }
        r.read_string().unwrap()
    }

    #[test]
    fn short_html_is_one_content_packet_plus_two_empty() {
        let pkts = build_cb_packets("<html><body>hi</body></html>");
        assert_eq!(pkts.len(), 3);
        assert!(cb_content(&pkts[0]).starts_with("101\u{0008}"), "first chunk tagged 101");
        assert!(cb_content(&pkts[0]).contains("hi"), "html lands in the first chunk");
        // Empty continuation chunks reproduce Java's literal `null`.
        assert_eq!(cb_content(&pkts[1]), "102\u{0008}null");
        assert_eq!(cb_content(&pkts[2]), "103\u{0008}null");
    }

    #[test]
    fn long_html_splits_across_two_chunks() {
        let html = "x".repeat(CHUNK + 500); // between 1 and 2 chunks
        let pkts = build_cb_packets(&html);
        assert_eq!(pkts.len(), 3);
        let c0 = cb_content(&pkts[0]);
        let c1 = cb_content(&pkts[1]);
        assert_eq!(c0.chars().count(), "101\u{0008}".chars().count() + CHUNK, "first chunk is full");
        assert_eq!(c1.chars().count(), "102\u{0008}".chars().count() + 500, "remainder in the second");
        assert_eq!(cb_content(&pkts[2]), "103\u{0008}null", "third chunk empty");
    }
}
