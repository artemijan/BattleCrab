//! Sell buffs — port of `instancemanager/SellBuffsManager` +
//! `custom/SellBuff/SellBuff.java`, gated on `Custom/SellBuffs.ini`
//! (`SellBuffEnable = True` on this dist).
//!
//! A character sits down, lists some of their own buffs at a price each, and
//! other players walk up and buy a cast. The shop rides the `PACKAGE_SELL`
//! private-store type — so other clients draw the usual shop label and the
//! seller sits — but everything else is its own: its own list, its own
//! community-board pages, its own bypasses.
//!
//! **The menus are community-board html** (`CommunityBoardHandler.
//! separateAndSend`), not `NpcHtmlMessage`, which is why every page arrives
//! through [`crate::game_loop::community_board`]'s chunked sender.
//!
//! Buying is deliberately asymmetric: the **buyer** pays the price in
//! `PaymentID`, the **seller** pays the MP, and the skill lands on the buyer.
//! A seller too low on MP is refused with a message rather than the cast
//! silently failing.

use crate::game_loop::combat::duel;
use crate::game_loop::helpers::{is_dead, nth_arg, player_name_or_empty, send_message};
use crate::game_loop::items;
use crate::game_loop::skills::skill_by_id;
use crate::model::Player;
use crate::network::server_packets as sp;
use crate::world::World;

/// Java `Npc.INTERACTION_DISTANCE` — how close a buyer must stand.
const INTERACTION_DISTANCE: f64 = 150.0;
/// `SellBuffsManager`'s page size (`ceiling = index + 10`).
const PAGE_SIZE: usize = 10;
/// Java `PrivateStoreType.PACKAGE_SELL`, the type a buff shop wears.
const STORE_TYPE_PACKAGE_SELL: u8 = 8;
/// The html these pages live in.
const HTML_FOLDER: &str = "data/html/mods/SellBuffs/";

/// `SellBuffsManager.sendSellMenu` — also the `.sellbuff` / `.sellbuffs` voiced
/// command. One of two pages, depending on whether the shop is already running.
pub(crate) fn send_sell_menu(world: &World, client_id: u32, player_oid: i32) {
    let selling = is_selling(world, player_oid);
    let page = if selling {
        "BuffMenu_already.html"
    } else {
        "BuffMenu.html"
    };
    let html = read_page(world, player_oid, page);
    crate::game_loop::community_board::send_cb_html(world, client_id, &html);
}

/// The `sellbuff*` bypass family (Java `SellBuff.useBypass`). `rest` is
/// everything after the command word. Returns whether the command was ours.
pub(crate) fn handle_bypass(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    command: &str,
    rest: &str,
) -> bool {
    // Java registers the handlers only when the feature is on, so an unknown
    // bypass simply does nothing.
    if !world.cfg.sell_buffs.enabled {
        return false;
    }
    let args: Vec<&str> = rest.split_whitespace().collect();
    match command {
        "sellbuffstart" => start(world, client_id, player_oid, &args),
        "sellbuffstop" => stop(world, client_id, player_oid),
        "sellbuffadd" => {
            if !is_selling(world, player_oid) {
                let index = args.first().and_then(|a| a.parse().ok()).unwrap_or(0);
                send_buff_choice_menu(world, client_id, player_oid, index);
            }
        }
        "sellbuffedit" => {
            if !is_selling(world, player_oid) {
                send_buff_edit_menu(world, client_id, player_oid);
            }
        }
        "sellbuffaddskill" => add_skill(world, client_id, player_oid, &args),
        "sellbuffchangeprice" => change_price(world, client_id, player_oid, &args),
        "sellbuffremove" => remove_skill(world, client_id, player_oid, &args),
        "sellbuffbuymenu" => buy_menu(world, client_id, player_oid, &args),
        "sellbuffbuyskill" => buy_skill(world, client_id, player_oid, &args),
        _ => return false,
    }
    true
}

/// `SellBuffsManager.startSellBuffs`: sit, flag, wear the package-sell store
/// type with the given title, and re-show the menu. Java checks
/// `canStartSellBuffs` from the html button; the empty-list and title-length
/// gates live in the bypass itself.
fn start(world: &mut World, client_id: u32, player_oid: i32, args: &[&str]) {
    if is_selling(world, player_oid) || args.is_empty() {
        return;
    }
    if sell_list(world, player_oid).is_empty() {
        send_message(
            world,
            client_id,
            "Your list of buffs is empty, please add some buffs first!",
        );
        return;
    }
    // Java builds `"BUFF SELL: " + params` and rejects the whole thing over 40
    // characters — the message says 29 because the prefix is 11 long.
    let title = format!("BUFF SELL: {}", args.join(" "));
    if title.chars().count() > 40 {
        send_message(
            world,
            client_id,
            "Your title cannot exceed 29 characters in length. Please try again.",
        );
        return;
    }
    if !can_start(world, client_id, player_oid) {
        return;
    }
    crate::game_loop::character::sit_stand::sit_down(world, player_oid);
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.selling_buffs = true;
        p.store_type = STORE_TYPE_PACKAGE_SELL;
    }
    crate::game_loop::character::player_info::broadcast_user_info(world, player_oid);
    let packet = sp::ex_private_store_whole_msg(player_oid, &title);
    crate::game_loop::helpers::broadcast_including_self(world, player_oid, &packet);
    send_sell_menu(world, client_id, player_oid);
}

/// `SellBuffsManager.stopSellBuffs` — clear the flag and the store type, stand
/// up, re-show the menu. The list itself survives, so a seller can reopen.
fn stop(world: &mut World, client_id: u32, player_oid: i32) {
    if !is_selling(world, player_oid) {
        return;
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.selling_buffs = false;
        p.store_type = 0;
    }
    crate::game_loop::character::sit_stand::stand_up(world, player_oid);
    crate::game_loop::character::player_info::broadcast_user_info(world, player_oid);
    send_sell_menu(world, client_id, player_oid);
}

/// `SellBuffsManager.canStartSellBuffs` — the state gate, message and all.
/// Two of Java's legs have no port equivalent and are noted at the site.
fn can_start(world: &World, client_id: u32, player_oid: i32) -> bool {
    let refuse = |text: &str| {
        send_message(world, client_id, text);
        false
    };
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return false;
    };
    let dead = is_dead(world, player_oid);
    if dead {
        return refuse("You can't sell buffs in fake death!");
    }
    if world.olympiad.in_competition.contains(&player_oid)
        || world.olympiad.is_registered(player_oid)
    {
        return refuse("You can't sell buffs with Olympiad status!");
    }
    if p.registered_on_event {
        return refuse("You can't sell buffs while registered in an event!");
    }
    if p.cursed_weapon_equipped_id != 0 || p.reputation < 0 {
        return refuse("You can't sell buffs in Chaotic state!");
    }
    if duel::is_in_duel(world, player_oid) {
        return refuse("You can't sell buffs in Duel state!");
    }
    if world
        .objects
        .get_component::<crate::model::components::FishingSession>(&player_oid)
        .is_some_and(|f| f.is_fishing)
    {
        return refuse("You can't sell buffs while fishing.");
    }
    if p.is_mounted() {
        return refuse("You can't sell buffs in Mount state!");
    }
    if p.transform_id != 0 {
        return refuse("You can't sell buffs in Transform state!");
    }
    // `isInsideZone(NO_STORE) || !isInsideZone(PEACE) || isJailed()` — all
    // three answer with the same line.
    let no_store = crate::game_loop::commerce::private_store::in_no_store_zone(world, player_oid);
    let in_peace = world
        .objects
        .get_component::<crate::model::components::ZoneFlags>(&player_oid)
        .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Peace));
    let jailed = world
        .objects
        .get_component::<Player>(&player_oid)
        .is_some_and(|p| p.jailed);
    if no_store || !in_peace || jailed {
        return refuse("You can't sell buffs here!");
    }
    true
}

// ---------------------------------------------------------------------------
// The seller's own menus
// ---------------------------------------------------------------------------

/// `sendBuffChoiceMenu` — the paged list of *addable* skills: known, on the
/// `SellBuffData.xml` whitelist, and not already listed.
fn send_buff_choice_menu(world: &World, client_id: u32, player_oid: i32, index: usize) {
    let candidates = addable_skills(world, player_oid);
    let mut rows = String::new();
    for (skill_id, level) in candidates.iter().skip(index).take(PAGE_SIZE) {
        let name = skill_name(world, *skill_id, *level);
        rows.push_str(&format!(
            "<tr><td><a action=\"bypass sellbuffaddskill {skill_id} $price\">{name}</a></td>\
             <td><edit var=\"price\" width=100></td></tr>"
        ));
    }
    rows.push_str(&pager("sellbuffadd", index, candidates.len()));
    let html = read_page(world, player_oid, "BuffChoice.html").replace("%list%", &rows);
    crate::game_loop::community_board::send_cb_html(world, client_id, &html);
}

/// `sendBuffEditMenu` — the listed buffs, each with a re-price and a remove
/// link. Java reuses `BuffChoice.html` for this page too.
fn send_buff_edit_menu(world: &World, client_id: u32, player_oid: i32) {
    let list = sell_list(world, player_oid);
    let mut rows = String::new();
    for (skill_id, price) in &list {
        let level = known_level(world, player_oid, *skill_id).unwrap_or(1);
        let name = skill_name(world, *skill_id, level);
        rows.push_str(&format!(
            "<tr><td>{name}</td><td>{price}</td>\
             <td><a action=\"bypass sellbuffchangeprice {skill_id} $price\">Change</a></td>\
             <td><a action=\"bypass sellbuffremove {skill_id}\">Remove</a></td></tr>"
        ));
    }
    let html = read_page(world, player_oid, "BuffChoice.html").replace("%list%", &rows);
    crate::game_loop::community_board::send_cb_html(world, client_id, &html);
}

/// `sellbuffaddskill <skillId> <price>` — the price bounds and the list cap are
/// all checked here, each with Java's own message.
fn add_skill(world: &mut World, client_id: u32, player_oid: i32, args: &[&str]) {
    if is_selling(world, player_oid) {
        return;
    }
    let (Some(skill_id), Some(price)) = (nth_arg::<i32>(args, 0), nth_arg::<i64>(args, 1)) else {
        return;
    };
    if known_level(world, player_oid, skill_id).is_none() {
        return; // `getKnownSkill(skillId) == null`
    }
    let cfg = world.cfg.sell_buffs.clone();
    if price < cfg.min_price {
        send_message(
            world,
            client_id,
            &format!("Too small price! Minimum price is {}", cfg.min_price),
        );
        return;
    }
    if price > cfg.max_price {
        send_message(
            world,
            client_id,
            &format!("Too big price! Maximum price is {}", cfg.max_price),
        );
        return;
    }
    if sell_list(world, player_oid).len() >= cfg.max_buffs {
        send_message(
            world,
            client_id,
            &format!(
                "You already reached max count of buffs! Max buffs is: {}",
                cfg.max_buffs
            ),
        );
        return;
    }
    if sell_list(world, player_oid)
        .iter()
        .any(|(id, _)| *id == skill_id)
    {
        return; // already listed
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.sell_buff_list.push((skill_id, price));
    }
    let level = known_level(world, player_oid, skill_id).unwrap_or(1);
    let name = skill_name(world, skill_id, level);
    send_message(world, client_id, &format!("{name} has been added!"));
    send_buff_choice_menu(world, client_id, player_oid, 0);
}

/// `sellbuffchangeprice <skillId> <price>`. Java does **not** re-check the
/// min/max bounds here — only `sellbuffaddskill` does — so a seller can
/// re-price outside them. Kept verbatim.
fn change_price(world: &mut World, client_id: u32, player_oid: i32, args: &[&str]) {
    if is_selling(world, player_oid) {
        return;
    }
    let (Some(skill_id), Some(price)) = (nth_arg::<i32>(args, 0), nth_arg::<i64>(args, 1)) else {
        return;
    };
    let Some(level) = known_level(world, player_oid, skill_id) else {
        return;
    };
    let found = world
        .objects
        .get_component_mut::<Player>(&player_oid)
        .and_then(|p| {
            p.sell_buff_list
                .iter_mut()
                .find(|(id, _)| *id == skill_id)
                .map(|entry| {
                    entry.1 = price;
                })
        })
        .is_some();
    if found {
        let name = skill_name(world, skill_id, level);
        send_message(
            world,
            client_id,
            &format!("Price of {name} has been changed to {price}!"),
        );
        send_buff_edit_menu(world, client_id, player_oid);
    }
}

/// `sellbuffremove <skillId>`.
fn remove_skill(world: &mut World, client_id: u32, player_oid: i32, args: &[&str]) {
    if is_selling(world, player_oid) {
        return;
    }
    let Some(skill_id) = nth_arg::<i32>(args, 0) else {
        return;
    };
    let Some(level) = known_level(world, player_oid, skill_id) else {
        return;
    };
    let removed = world
        .objects
        .get_component_mut::<Player>(&player_oid)
        .map(|p| {
            let before = p.sell_buff_list.len();
            p.sell_buff_list.retain(|(id, _)| *id != skill_id);
            before != p.sell_buff_list.len()
        })
        .unwrap_or(false);
    if removed {
        let name = skill_name(world, skill_id, level);
        send_message(world, client_id, &format!("Skill {name} has been removed!"));
        send_buff_edit_menu(world, client_id, player_oid);
    }
}

// ---------------------------------------------------------------------------
// The buyer's side
// ---------------------------------------------------------------------------

/// `sellbuffbuymenu <sellerObjId> [index]` — the shop's page, refused unless
/// the seller is really selling and the buyer is in interaction range.
pub(crate) fn buy_menu(world: &mut World, client_id: u32, player_oid: i32, args: &[&str]) {
    let Some(seller) = nth_arg::<i32>(args, 0) else {
        return;
    };
    let index = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(0);
    send_buff_menu(world, client_id, player_oid, seller, index);
}

/// `SellBuffsManager.sendBuffMenu`.
pub(crate) fn send_buff_menu(
    world: &World,
    client_id: u32,
    buyer_oid: i32,
    seller_oid: i32,
    index: usize,
) {
    if !is_selling(world, seller_oid)
        || !crate::geo::distance::within_3d(world, buyer_oid, seller_oid, INTERACTION_DISTANCE)
    {
        return;
    }
    let list = sell_list(world, seller_oid);
    if list.is_empty() {
        return;
    }
    let mut rows = String::new();
    for (skill_id, price) in list.iter().skip(index).take(PAGE_SIZE) {
        let level = known_level(world, seller_oid, *skill_id).unwrap_or(1);
        let name = skill_name(world, *skill_id, level);
        rows.push_str(&format!(
            "<tr><td><a action=\"bypass sellbuffbuyskill {seller_oid} {skill_id} {index}\">\
             {name}</a></td><td>{price}</td></tr>"
        ));
    }
    rows.push_str(&pager(
        &format!("sellbuffbuymenu {seller_oid}"),
        index,
        list.len(),
    ));
    let html = read_page(world, buyer_oid, "BuffBuyMenu.html").replace("%list%", &rows);
    crate::game_loop::community_board::send_cb_html(world, client_id, &html);
}

/// `sellbuffbuyskill <sellerObjId> <skillId> [index]` — the transaction:
/// buyer pays, **seller** pays the MP, and the skill lands on the buyer.
fn buy_skill(world: &mut World, client_id: u32, buyer_oid: i32, args: &[&str]) {
    let (Some(seller_oid), Some(skill_id)) = (nth_arg::<i32>(args, 0), nth_arg::<i32>(args, 1))
    else {
        return;
    };
    let index = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(0);
    let Some(level) = known_level(world, seller_oid, skill_id) else {
        return;
    };
    if !is_selling(world, seller_oid)
        || !crate::geo::distance::within_3d(world, buyer_oid, seller_oid, INTERACTION_DISTANCE)
    {
        return;
    }
    let Some(price) = sell_list(world, seller_oid)
        .iter()
        .find(|(id, _)| *id == skill_id)
        .map(|(_, price)| *price)
    else {
        // Java still re-shows the menu when the holder is missing.
        send_buff_menu(world, client_id, buyer_oid, seller_oid, index);
        return;
    };
    let Some(skill) = skill_by_id(world, skill_id, level) else {
        return;
    };
    let mp_cost = (skill.mp_consume * world.cfg.sell_buffs.mp_multiplier) as f64;
    let seller_mp = world
        .objects
        .get_component::<crate::model::components::Vitals>(&seller_oid)
        .map_or(0.0, |v| v.cur_mp);
    if seller_mp < mp_cost {
        let seller_name = player_name_or_empty(world, seller_oid);
        send_message(
            world,
            client_id,
            &format!("{seller_name} has no enough mana for {}!", skill.name),
        );
        send_buff_menu(world, client_id, buyer_oid, seller_oid, index);
        return;
    }
    let payment_id = world.cfg.sell_buffs.payment_id;
    let paid = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&buyer_oid)
        .map_or(0, |inv| inv.count_of(payment_id));
    if paid < price {
        let item_name = world
            .data
            .item_data
            .get(payment_id)
            .map(|t| t.name.clone())
            .unwrap_or_default();
        let text = if item_name.is_empty() {
            "Not enough items!".to_string()
        } else {
            format!("Not enough {item_name}!")
        };
        send_message(world, client_id, &text);
        send_buff_menu(world, client_id, buyer_oid, seller_oid, index);
        return;
    }
    items::take_items(world, client_id, buyer_oid, payment_id, price);
    items::add_inventory_item(world, seller_oid, payment_id, price);
    crate::game_loop::helpers::spend_mp(world, seller_oid, mp_cost);
    // `skill.activateSkill(seller, player)` — the *seller* casts it on the
    // buyer, so the buff is attributed to the seller like any other cast.
    crate::game_loop::skills::effects::apply_skill_effects(world, seller_oid, buyer_oid, &skill);
    send_buff_menu(world, client_id, buyer_oid, seller_oid, index);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn is_selling(world: &World, player_oid: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&player_oid)
        .is_some_and(|p| p.selling_buffs)
}

fn sell_list(world: &World, player_oid: i32) -> Vec<(i32, i64)> {
    world
        .objects
        .get_component::<Player>(&player_oid)
        .map(|p| p.sell_buff_list.clone())
        .unwrap_or_default()
}

/// The skills this player could still add: known, whitelisted, not listed yet.
fn addable_skills(world: &World, player_oid: i32) -> Vec<(i32, i32)> {
    let listed = sell_list(world, player_oid);
    let Some(book) = world
        .objects
        .get_component::<crate::model::components::SkillBook>(&player_oid)
    else {
        return Vec::new();
    };
    let mut out: Vec<(i32, i32)> = book
        .0
        .iter()
        .filter(|(id, _)| world.data.sell_buff_data.allows(**id))
        .filter(|(id, _)| !listed.iter().any(|(listed_id, _)| listed_id == *id))
        .map(|(id, level)| (*id, *level))
        .collect();
    // A `HashMap` iteration order would shuffle the page between views.
    out.sort_unstable();
    out
}

fn known_level(world: &World, player_oid: i32, skill_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<crate::model::components::SkillBook>(&player_oid)
        .and_then(|b| b.0.get(&skill_id).copied())
}

fn skill_name(world: &World, skill_id: i32, level: i32) -> String {
    world
        .data
        .skill_data
        .get(skill_id, level)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("Skill {skill_id}"))
}

/// `Util.checkIfInRange(Npc.INTERACTION_DISTANCE, …, true)` — 3D.
/// The previous/next links Java appends under each page.
fn pager(bypass: &str, index: usize, total: usize) -> String {
    let mut out = String::new();
    if index >= PAGE_SIZE {
        out.push_str(&format!(
            "<tr><td><a action=\"bypass {bypass} {}\">Previous</a></td></tr>",
            index - PAGE_SIZE
        ));
    }
    if total > index + PAGE_SIZE {
        out.push_str(&format!(
            "<tr><td><a action=\"bypass {bypass} {}\">Next</a></td></tr>",
            index + PAGE_SIZE
        ));
    }
    out
}

fn read_page(world: &World, viewer_oid: i32, file: &str) -> String {
    crate::data::htm_cache::read_htm_for(
        world,
        viewer_oid,
        format!("{}{HTML_FOLDER}{file}", world.data.root),
    )
    .unwrap_or_else(|| "<html><body>%list%</body></html>".to_string())
}

/// Java `Player.onActionRequest`'s sell-buff branch: clicking a seller opens
/// their buff shop instead of the ordinary private store.
pub(crate) fn on_action(world: &World, client_id: u32, buyer_oid: i32, seller_oid: i32) -> bool {
    if !world.cfg.sell_buffs.enabled || !is_selling(world, seller_oid) {
        return false;
    }
    send_buff_menu(world, client_id, buyer_oid, seller_oid, 0);
    true
}

/// The seller must lose the shop when they stop being able to hold one — Java
/// clears `_isSellingBuffs` from `stopSellBuffs` only, but the flag is also
/// read by `canOpenPrivateStore`, so a stale one would lock a player out of
/// ordinary stores. Called from the logout/teleport teardown.
pub(crate) fn clear(world: &mut World, player_oid: i32) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.selling_buffs = false;
        p.sell_buff_list.clear();
    }
}
