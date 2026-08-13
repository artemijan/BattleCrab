//! The Clan Hall Auctioneer (NPC 30767) — the player side of clan-hall auctions
//! (`ai/others/ClanHallAuctioneer`). A clan leader browses the free halls, reads
//! a hall's auction info, bids on / cancels bids for it, and lists the current
//! bidders; the auction logic lives in [`crate::game_loop::clans::hall_auction`].
//!
//! The dynamic dist htmls are templated here: the hall list (`%agitList%`), the
//! per-hall info page, the bid form (`%clanAdena%`/`%minBid%`), the bidder list
//! (`%bidderList%`), and the cancel-confirmation. `%auctionEnd%`/`%hours%`/
//! `%minutes%` come from `World.auction_end_tick` (the weekly-close countdown).

use crate::game_loop::clans::clan_name_or_empty;
use crate::game_loop::clans::hall_auction::{self, BidOutcome, bid_count, highest_bid};
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::Player;
use crate::model::clan_hall::{ClanHallGrade, ClanHallType};
use crate::world::World;

const AUCTIONEER: i32 = 30767;
const HTML_DIR: &str = "ai/others/ClanHallAuctioneer";
const ADENA_ID: i32 = crate::data::item_data::ADENA_ID;
/// 10 ticks per second.
const MS_PER_TICK: u64 = 100;

pub struct ClanHallAuctioneer;

impl QuestScript for ClanHallAuctioneer {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "ClanHallAuctioneer"
    }
    fn html_dir(&self) -> &'static str {
        HTML_DIR
    }
    fn start_npcs(&self) -> &[i32] {
        &[AUCTIONEER]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[AUCTIONEER]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[AUCTIONEER]
    }

    fn on_first_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        Some("ClanHallAuctioneer.html".to_string())
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        // The dist bid form posts `bid id=%id% bid=' $bidprice '` (quotes and
        // spaces around the value), so parse tolerantly rather than by token.
        let cleaned = event.replace(['\'', '"'], " ");
        let toks: Vec<&str> = cleaned.split_whitespace().collect();
        let verb = *toks.first().unwrap_or(&"");
        let hall_id = field(&toks, "id=") as i32;
        let bid = field(&toks, "bid=");

        match verb {
            "auctionList" if hall_id > 0 => render_hall_info(ctx.world, hall_id),
            "auctionList" => Some(render_hall_list(ctx.world)),
            "listBidder" => render_bidder_list(ctx.world, hall_id),
            "bid" => self.on_bid(ctx, hall_id, bid),
            "cancelBid" => self.on_cancel_page(ctx),
            "cancel" => self.on_cancel(ctx),
            // The static navigation pages (main / map).
            e if e.ends_with(".html") => Some(e.to_string()),
            _ => Some("ClanHallAuctioneer.html".to_string()),
        }
    }
}

impl ClanHallAuctioneer {
    fn on_bid(&self, ctx: &mut QuestCtx, hall_id: i32, bid: i64) -> Option<String> {
        // Only a clan leader may bid (the clan-level check is in `place_bid`).
        if !ctx.is_clan_leader() {
            return Some(message(
                "Only a clan leader whose clan is level 2 or above may bid.",
            ));
        }
        let Some(clan_id) = clan_of(ctx) else {
            return Some(message("You must be in a clan to bid."));
        };
        // No amount yet → the templated bid form (clan adena + current minimum).
        if bid == 0 {
            return render_bid_form(ctx.world, clan_id, hall_id);
        }
        let now = commons::util::now_millis();
        let outcome = hall_auction::place_bid(ctx.world, hall_id, clan_id, bid, now);
        Some(message(bid_message(outcome)))
    }

    /// The cancel-confirmation page (`cancelBid` verb): shows the clan's own bid
    /// and the non-refundable tax note.
    fn on_cancel_page(&self, ctx: &mut QuestCtx) -> Option<String> {
        let Some(clan_id) = clan_of(ctx) else {
            return Some(message("You must be in a clan."));
        };
        let Some(hall_id) = hall_auction::clan_bid_hall(ctx.world, clan_id) else {
            return Some(message("You have no active bid to cancel."));
        };
        let my_bid = hall_auction::highest_bidder(ctx.world, hall_id)
            .filter(|&(c, _)| c == clan_id)
            .map(|(_, a)| a)
            .unwrap_or_else(|| {
                ctx.world
                    .clan_hall_bids
                    .get(&hall_id)
                    .and_then(|b| b.get(&clan_id))
                    .map(|b| b.amount)
                    .unwrap_or(0)
            });
        // Java `%myBidRemain%` = the bid minus the 10% tax (`* 9 / 10`), shown as
        // `getClanBid(clan) * 9` in the dist (a per-mille display quirk kept).
        Some(
            tpl("ClanHallAuctioneer-cancelBid.html", ctx.world)
                .replace("%myBid%", &my_bid.to_string())
                .replace("%myBidRemain%", &(my_bid * 9).to_string()),
        )
    }

    fn on_cancel(&self, ctx: &mut QuestCtx) -> Option<String> {
        let Some(clan_id) = clan_of(ctx) else {
            return Some(message("You must be in a clan."));
        };
        match hall_auction::clan_bid_hall(ctx.world, clan_id) {
            Some(hall_id) => {
                hall_auction::cancel_bid(ctx.world, hall_id, clan_id);
                Some(message("Your bid has been canceled."))
            }
            None => Some(message("You have no active bid to cancel.")),
        }
    }
}

/// The list of free auctionable halls (`ClanHallAuctioneer-list.html`), each row
/// linking to its info page and showing the current highest bid.
pub(crate) fn render_hall_list(world: &World) -> String {
    let mut halls: Vec<&crate::model::clan_hall::ClanHall> = world
        .clan_halls
        .values()
        .filter(|h| h.owner_id == 0 && h.hall_type == ClanHallType::Auctionable)
        .collect();
    halls.sort_by_key(|h| h.id);

    let end_date = fmt_date(auction_end_millis(world));
    let mut rows = String::new();
    for h in halls {
        // `&^<id>;` / `&%<id>;` are client-side clan-hall name/desc string refs.
        rows.push_str(&format!(
            "<tr><td width=50><font color=\"aaaaff\">&^{id};</font></td>\
             <td width=100><a action=\"bypass -h Quest ClanHallAuctioneer auctionList id={id}\">\
             <font color=\"ffffaa\">&%{id};[0]</font></a></td>\
             <td width=50>{end_date}</td>\
             <td width=70 align=right><font color=\"aaffff\">{bid}</font></td></tr>",
            id = h.id,
            bid = highest_bid(world, h.id),
        ));
    }
    tpl("ClanHallAuctioneer-list.html", world)
        .replace("%agitList%", &rows)
        .replace("%pages%", "")
}

/// One hall's auction info (`ClanHallAuctioneer-info.html`).
pub(crate) fn render_hall_info(world: &World, hall_id: i32) -> Option<String> {
    let hall = world.clan_halls.get(&hall_id)?;
    let (owner, leader) = owner_names(world, hall.owner_id);
    let (hours, minutes) = auction_remaining(world);
    Some(
        tpl("ClanHallAuctioneer-info.html", world)
            .replace("%owner%", &owner)
            .replace("%clanLeader%", &leader)
            .replace("%rent%", &hall.lease.to_string())
            .replace("%grade%", &grade_value(hall.grade).to_string())
            .replace("%minBid%", &highest_bid(world, hall_id).to_string())
            .replace("%bidNumber%", &bid_count(world, hall_id).to_string())
            .replace("%auctionEnd%", &fmt_date(auction_end_millis(world)))
            .replace("%hours%", &hours.to_string())
            .replace("%minutes%", &minutes.to_string())
            // Last: `%id%` also appears in the button bypasses and string refs.
            .replace("%id%", &hall_id.to_string()),
    )
}

/// The bid form (`ClanHallAuctioneer-bid1.html`) — the clan's warehouse adena
/// and the current minimum, with the hall id fixed into the confirm bypass.
pub(crate) fn render_bid_form(world: &World, clan_id: i32, hall_id: i32) -> Option<String> {
    world.clan_halls.get(&hall_id)?;
    let adena = world
        .clans
        .get(&clan_id)
        .map(|c| c.warehouse.0.count_of(ADENA_ID))
        .unwrap_or(0);
    Some(
        tpl("ClanHallAuctioneer-bid1.html", world)
            .replace("%clanAdena%", &adena.to_string())
            .replace("%minBid%", &highest_bid(world, hall_id).to_string())
            .replace("%id%", &hall_id.to_string()),
    )
}

/// The bidder list (`ClanHallAuctioneer-bidderList.html`), newest bid first.
pub(crate) fn render_bidder_list(world: &World, hall_id: i32) -> Option<String> {
    let bids = world.clan_hall_bids.get(&hall_id)?;
    let mut list: Vec<(i32, &crate::model::clan_hall::ClanHallBid)> =
        bids.iter().map(|(k, v)| (*k, v)).collect();
    // Java sorts by bid time, most recent first.
    list.sort_by_key(|(_, b)| std::cmp::Reverse(b.bid_time));

    let mut rows = String::new();
    for (clan_id, b) in list {
        let name = clan_name_or_empty(world, clan_id);
        rows.push_str(&format!(
            "<tr><td width=100>{name}</td><td width=100>{amount}</td>\
             <td width=70>{time}</td></tr>",
            amount = b.amount,
            time = fmt_date(b.bid_time),
        ));
    }
    Some(
        tpl("ClanHallAuctioneer-bidderList.html", world)
            .replace("%bidderList%", &rows)
            .replace("%pages%", "")
            .replace("%id%", &hall_id.to_string()),
    )
}

/// Read one of the auctioneer's html templates.
fn tpl(file: &str, world: &World) -> String {
    crate::data::htm_cache::read_htm(format!("{}data/scripts/{HTML_DIR}/{file}", world.data.root))
        .unwrap_or_default()
}

/// The clan's name and its leader's name for an owner id (`("", "")` if free).
fn owner_names(world: &World, owner_id: i32) -> (String, String) {
    if owner_id == 0 {
        return (String::new(), String::new());
    }
    let name = clan_name_or_empty(world, owner_id);
    let leader = world
        .clans
        .get(&owner_id)
        .and_then(|c| world.objects.get_component::<Player>(&c.leader_id))
        .map(|p| p.name.clone())
        .unwrap_or_default();
    (name, leader)
}

fn clan_of(ctx: &QuestCtx) -> Option<i32> {
    crate::game_loop::guard::clan_of(ctx.world, ctx.player)
}

/// The value after `key` (e.g. `id=`), tolerating the dist templates' quotes and
/// spaces around the number (`bid=' 5000000 '` after quote-stripping becomes
/// `bid=` then a separate `5000000` token).
fn field(toks: &[&str], key: &str) -> i64 {
    for (i, t) in toks.iter().enumerate() {
        if let Some(v) = t.strip_prefix(key) {
            let v = v.trim();
            if !v.is_empty() {
                return v.parse().unwrap_or(0);
            }
            return toks
                .get(i + 1)
                .and_then(|n| n.trim().parse().ok())
                .unwrap_or(0);
        }
    }
    0
}

/// Epoch-millis the current auction cycle closes.
fn auction_end_millis(world: &World) -> i64 {
    let remaining = world.auction_end_tick.saturating_sub(world.tick) * MS_PER_TICK;
    commons::util::now_millis() + remaining as i64
}

/// Whole hours and minutes until the auction closes.
fn auction_remaining(world: &World) -> (u64, u64) {
    let ms = world.auction_end_tick.saturating_sub(world.tick) * MS_PER_TICK;
    (ms / 3_600_000, (ms % 3_600_000) / 60_000)
}

/// `dd/MM/yyyy HH:mm` from epoch-millis.
fn fmt_date(millis: i64) -> String {
    let (year, month, day, hour, minute, _) = commons::util::civil_from_millis(millis);
    format!("{day:02}/{month:02}/{year:04} {hour:02}:{minute:02}")
}

fn grade_value(grade: ClanHallGrade) -> i32 {
    match grade {
        ClanHallGrade::None => 0,
        ClanHallGrade::D => 10,
        ClanHallGrade::C => 20,
        ClanHallGrade::B => 30,
        ClanHallGrade::A => 40,
        ClanHallGrade::S => 50,
    }
}

/// A minimal one-line message window.
fn message(text: &str) -> String {
    format!("<html><body>Clan Hall Auction:<br>{text}</body></html>")
}

fn bid_message(outcome: BidOutcome) -> &'static str {
    match outcome {
        BidOutcome::Accepted => "Your bid has been successfully placed.",
        BidOutcome::HallUnavailable => "That clan hall is not up for auction.",
        BidOutcome::ClanTooLow => "Only a clan of level 2 or above may bid.",
        BidOutcome::AlreadyOwnsHall => "Your clan already owns a clan hall.",
        BidOutcome::BiddingElsewhere => "You have already bid on another clan hall.",
        BidOutcome::BidTooHigh => "The bid is over the maximum allowed.",
        BidOutcome::BidTooLow => "Your bid must be higher than the current highest bid.",
        BidOutcome::NotEnoughAdena => "There is not enough adena in the clan warehouse.",
    }
}
