//! Item-auction house (G30.5) — port of Java `ItemAuctionManager` +
//! `ItemAuctionInstance`. This slice is the boot load (config gate + resume the
//! persisted auctions + the auction-id allocator); the lifecycle state machine,
//! bidding, and delivery come in later slices.

use crate::model::item_auction::ItemAuction;
use crate::world::World;

/// Boot restore (Java `ItemAuctionManager` constructor), driven by
/// `DbEvent::ItemAuctionsLoaded`: gate on `AltItemAuctionEnabled`, seed the
/// auction-id allocator, and load the persisted in-flight auctions.
pub(crate) fn on_loaded(world: &mut World, next_auction_id: i32, auctions: Vec<ItemAuction>) {
    if !world.cfg.general.alt_item_auction_enabled {
        return;
    }
    world.item_auctions.enabled = true;
    world.item_auctions.next_auction_id = next_auction_id.max(1);
    world.item_auctions.auctions = auctions.into_iter().map(|a| (a.auction_id, a)).collect();
    // TODO(G30.5) slice 2: `checkAndSetCurrentAndNextAuction` per instance —
    //   pick the current/next auction and arm the state task. The dist ships
    //   `ItemAuctions.xml` empty, so there are no instances to schedule yet.
}
