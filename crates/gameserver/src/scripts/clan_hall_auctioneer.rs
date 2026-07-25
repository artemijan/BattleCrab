//! The Clan Hall Auctioneer (NPC 30767) — the player side of clan-hall auctions
//! (`ai/others/ClanHallAuctioneer`). A clan leader bids on / cancels bids for a
//! free hall; the auction logic lives in [`crate::game_loop::clan_hall_auction`].
//!
//! The dist htmls (map / hall list / bidder list) are served as-is for
//! navigation; the interactive bid form is built inline so the hall id is baked
//! into the submit bypass (the framework only substitutes `%objectId%`). Full
//! templating of the dynamic pages (current bid, clan adena) is later polish.

use crate::game_loop::clan_hall_auction::{self, BidOutcome};
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::Player;

const AUCTIONEER: i32 = 30767;

pub struct ClanHallAuctioneer;

impl QuestScript for ClanHallAuctioneer {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "ClanHallAuctioneer"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/ClanHallAuctioneer"
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
        let mut parts = event.split_whitespace();
        let verb = parts.next().unwrap_or("");
        let (mut hall_id, mut bid) = (0i32, 0i64);
        for p in parts {
            if let Some(v) = p.strip_prefix("id=") {
                hall_id = v.parse().unwrap_or(0);
            } else if let Some(v) = p.strip_prefix("bid=") {
                bid = v.parse().unwrap_or(0);
            }
        }

        match verb {
            "bid" => self.on_bid(ctx, hall_id, bid),
            "cancel" => self.on_cancel(ctx),
            // The static navigation pages (map / list / bidder list / …).
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
        // No amount yet → offer the bid form with the hall id baked in.
        if bid == 0 {
            let min = clan_hall_auction::highest_bid(ctx.world, hall_id);
            return Some(bid_form(hall_id, min));
        }
        let now = commons::util::now_millis();
        let outcome = clan_hall_auction::place_bid(ctx.world, hall_id, clan_id, bid, now);
        Some(message(bid_message(outcome)))
    }

    fn on_cancel(&self, ctx: &mut QuestCtx) -> Option<String> {
        let Some(clan_id) = clan_of(ctx) else {
            return Some(message("You must be in a clan."));
        };
        match clan_hall_auction::clan_bid_hall(ctx.world, clan_id) {
            Some(hall_id) => {
                clan_hall_auction::cancel_bid(ctx.world, hall_id, clan_id);
                Some(message("Your bid has been canceled."))
            }
            None => Some(message("You have no active bid to cancel.")),
        }
    }
}

fn clan_of(ctx: &QuestCtx) -> Option<i32> {
    ctx.world
        .objects
        .get_component::<Player>(&ctx.player)
        .map(|p| p.clan_id)
        .filter(|&c| c != 0)
}

/// A minimal one-line message window.
fn message(text: &str) -> String {
    format!("<html><body>Clan Hall Auction:<br>{text}</body></html>")
}

/// The inline bid form — the hall id is fixed into the submit bypass.
fn bid_form(hall_id: i32, min_bid: i64) -> String {
    format!(
        "<html><body>Clan Hall Auction:<br>\
         Minimum bid: {min_bid} adena.<br>\
         Enter your bid:<br>\
         <edit var=\"bidamount\" width=100><br>\
         <a action=\"bypass -h Quest ClanHallAuctioneer bid id={hall_id} bid=$bidamount\">Place bid</a>\
         </body></html>"
    )
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
