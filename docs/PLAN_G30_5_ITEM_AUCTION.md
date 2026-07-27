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

### Slice 1 — Data + model + DB foundation  ✅ LANDED
- `data/item_auction_data.rs`: `ItemAuctions.xml` parser → `AuctionInstanceCfg`
  (NPC id + `AuctionSchedule`) + `AuctionItem`; empty on this dist (verified by a
  test), plus `insert_for_test`. Handles both `interval` and `day_of_week`
  (normalized `1=Mon..7=Sun` → `Mon=0..Sun=6`), drops an item-less instance.
- `model/item_auction.rs`: `AuctionState` (byte ids), `ExtendState`,
  `ItemAuctionBid` (canceled = `last_bid ≤ 0`), `ItemAuction` (+ `highest_bid`
  ignoring canceled), and the `ItemAuctionManager` runtime on `World`
  (`enabled`, `next_auction_id` allocator, live `auctions`). Plus the pure
  `next_date` schedule math (Java `AuctionDateGenerator`, UTC like siege).
- DB: `item_auction`/`item_auction_bid` boot load (`DbEvent::ItemAuctionsLoaded`,
  `auctionId` allocator from `MAX+1`, bids attached) → `item_auction::on_loaded`
  (config-gated on `AltItemAuctionEnabled`); writes `DbCommand::{StoreItemAuction,
  StoreItemAuctionBid, DeleteItemAuctionBid, DeleteItemAuction}`.
- `AltItemAuctionEnabled` config in `GeneralConfig` (dist `True`).
- 9 tests (parser: empty dist / interval / weekday-normalize / item-less drop;
  date math interval + weekday; config gate; boot-load allocator + auctions;
  highest-bid-ignores-canceled), the date math sabotage-verified.

### Slice 2 — Auction lifecycle + scheduling  ✅ LANDED
- `check_and_set_current_and_next` (per instance): the 0/1/many-auction switch
  picking current + next, creating a fresh auction when needed
  (`START_TIME_SPACE`=1min / `FINISH_TIME_SPACE`=10min), and arming the state
  task; `InstanceRuntime { current, next }` per auctioneer on the manager.
- `create_auction` (random catalogue item, `next_date` start, id + `storeMe`),
  `run_state_task` (Java `ScheduleAuctionTask.runImpl`: CREATED→STARTED then
  re-check; STARTED→FINISHED with the bid-driven ending-extend re-arm — inert
  until slice 3 — then `on_auction_finished` + re-check). `on_loaded` now calls
  `check_and_set` for each configured instance.
- New `ScheduledTask::ItemAuctionState { auction_id }`; `ItemAuction`
  `scheduled_extend_state` field. `on_auction_finished` is a slice-4 stub
  (winner→warehouse deferred). 5 lifecycle tests (boot-creates-next, full
  CREATED→STARTED→FINISHED, started-at-boot-becomes-current), sabotage-verified.

### Slice 3 — Bidding + packets + the NPC dialog  ✅ LANDED
- `register_bid` — adena escrow (full new / delta on raise / full after cancel),
  the ≥init-bid / >highest / ≤999.9 bn gates, outbid notify, and the
  ending-extend state machine (last-10-min → +5min → +3min → config phases,
  each past the first gated on a *different* bidder). This activates the
  (slice-2, inert) `reschedule_for_extend` in `run_state_task`.
- `cancel_bid` — the loser refund (+ the "you hold the highest bid, reserve not
  met" branch, and the winner/expired refusals), persisted (delete when
  finished, else store the canceled row).
- `ItemAuctionLink` bypass (`ItemAuction show`/`cancel`), the two client packets
  (`on_request_bid` 0x36 / `on_request_info` 0x37), and `ExItemAuctionInfoPacket`
  (0xFE 0x69) reusing `write_item_entry` for a synthetic reward item.
- 14 SM ids + config `AltItemAuctionExpiredAfter`/`AltItemAuctionTimeExtendsOnBid`.
- 6 bidding tests (escrow, below-init reject, raise-delta, loser refund on
  cancel, highest-can't-cancel, last-minute extend), sabotage-verified.

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
