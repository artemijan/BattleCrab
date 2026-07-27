# PLAN — G30.5 Item Auction

A DB-backed, scheduled auction house (Forsaiken's `ItemAuction*`). Config-enabled
on this dist (`AltItemAuctionEnabled = True`) **but the data ships empty** —
`data/ItemAuctions.xml` has every `<instance>` commented out, so no auctions
actually run. Per [[l2r-config-disabled-still-port]] the engine is still ported
(an operator adds instances); the gate is met via a synthetic auction in tests.

**Deps:** G15 economy (adena escrow + warehouse delivery, both landed). G30 mail
is *not* required — Java delivers the won item to the winner's **warehouse**
(`ItemAuctionInstance.onAuctionFinished` → `player.getWarehouse().addItem`), not
by mail.

## Java sources (~1958 lines)

- `instancemanager/ItemAuctionManager.java` (168) — loads the XML into per-NPC
  `ItemAuctionInstance`s; owns the `auctionId` allocator; `deleteAuction`.
- `model/itemauction/ItemAuctionInstance.java` (615) — one auctioneer NPC's
  schedule: the current + next auction, `AuctionDateGenerator` timing, the
  state-transition task, `onAuctionFinished` (winner → warehouse).
- `model/itemauction/ItemAuction.java` (546) — one auction's lifecycle: bid
  register (adena escrow, outbid, the 5-min/3-min/config ending-extend states),
  DB persist, `cancelBid` (loser refund).
- `AuctionItem.java` (87) — the item template being auctioned (id, count, init
  bid, length, enchant extras). `AuctionDateGenerator.java` (124) — day-of-week
  / interval scheduling. `ItemAuctionBid.java` (64), `ItemAuctionState.java`
  (50), `ItemAuctionExtendState.java` (28).
- Packets: `ExItemAuctionInfoPacket` (0xFE 0x69, server), `RequestBidItemAuction`
  + `RequestInfoItemAuction` (client). Bypass: `ItemAuctionLink.java` (133, the
  auctioneer NPC dialog: `show` / `cancel`).
- DB: `item_auction` (auctionId, instanceId, auctionItemId, start/end, stateId)
  + `item_auction_bid` (auctionId, playerObjId, playerBid) — both in l2r's dist.

## Mechanics

- Each auctioneer NPC (an `<instance id=…>`) auctions a rotating list of
  `AuctionItem`s on a schedule (day-of-week @ hour:minute, or an interval in
  days). At any time it has a **current** (running/finished) and a **next**
  (created) auction.
- **Bid** (`registerBid`): must be ≥ the item's init bid and > the current
  highest, ≤ 999.9 bn. Adena is escrowed (`reduceAdena` for a new bid, or the
  delta when raising your own). The prior highest bidder is notified they were
  outbid. Within the last 10 min a bid extends the end time (INITIAL → +5 min →
  +3 min → config phases).
- **Finish** (`onAuctionFinished`): the highest bidder's item is placed in their
  **warehouse**; with no bids the auction just closes. Losers reclaim their
  escrowed adena via the NPC's `cancel` (`cancelBid` → `addAdena`).
- **Persistence**: `storeMe` on every state change; bids upserted per change;
  `deleteAuction` clears finished auctions past `AltItemAuctionExpiredAfter`
  (14 days). The `auctionId` allocator resumes from `MAX(auctionId)+1` at boot.

## Slice breakdown

### Slice 1 — Data + model + DB foundation  ⬅ start here
- `data/item_auction_data.rs`: parse `ItemAuctions.xml` → `AuctionInstanceCfg`
  (NPC id, schedule) + `AuctionItem` (auctionItemId, itemId, count, initBid,
  lengthMin, enchant). Empty on this dist — parser + a `insert_for_test`.
- `model/item_auction.rs`: `AuctionState`, `ExtendState`, `ItemAuctionBid`,
  `ItemAuction` (id, instance, start/end, item, bids, highest), and the
  `ItemAuctionManager` runtime on `World`.
- DB: `item_auction`/`item_auction_bid` load at boot (`DbEvent::ItemAuctionsLoaded`)
  + `DbCommand::{StoreItemAuction, StoreItemAuctionBid, DeleteItemAuctionBid,
  DeleteItemAuction}`; the `auctionId` allocator from `MAX+1`.
- The `AuctionDateGenerator` next-occurrence math (pure, testable).
- **Gate for the slice:** the manager boot-loads (empty on dist), a synthetic
  instance schedules its first auction, and the date math + persistence round-trip.

### Slice 2 — Auction lifecycle + scheduling
- `ItemAuctionInstance` current/next auction, the CREATED→STARTED→FINISHED
  state machine on `ScheduledTask::ItemAuction*`, `checkAndSetCurrentAndNext`.

### Slice 3 — Bidding + packets + the NPC dialog
- `RequestBid`/`RequestInfoItemAuction` + `ExItemAuctionInfoPacket`; adena
  escrow, outbid, the ending-extend states; `ItemAuctionLink` (`show`/`cancel`).

### Slice 4 — Finish + delivery + expiry
- `onAuctionFinished` (winner → warehouse), loser refund on cancel, the
  expired-auction cleanup, boot resume of an in-flight auction.

## Watch-list

- Won item goes to the **warehouse**, not inventory or mail — don't reach for
  the G30 mail path.
- Bidding is **adena escrow**: a new bid reduces the full amount; raising your
  own reduces only the delta; a canceled-then-rebid reduces the full amount
  again. Port `registerBid`'s branches exactly.
- The ending-extend state machine (5-min/3-min/two config phases, gated on
  "a *different* player bid") is fiddly retail behaviour — port verbatim.
- `AuctionDateGenerator` supports both `day_of_week`+`hour`+`minute` and
  `interval` (days) — handle both.
