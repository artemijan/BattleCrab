# PLAN — G26.5 Lottery & Monster Race

Two self-contained "games" subsystems from the 2026-07 roadmap audit. Both ship
**config-disabled** on this dist (`AllowLottery = False`, `AllowRace = False` in
`General.ini`) but are ported anyway — an operator may enable them, and
config-off is not a reason to skip (see [[l2r-config-disabled-still-port]]).

**Deps:** G15 economy (adena charge + item grant) — landed.

## Java sources

- **Lottery** — `instancemanager/games/Lottery.java` (521),
  `handlers/bypasshandlers/Loto.java` (341, the Lottery Manager NPC dialog).
  DB-backed: the `lottery` table (`dist/db_installer/sql/*/game/lottery.sql`)
  and ticket **item 4442**, which encodes the round id in `custom_type1` and the
  five picked numbers as a bitmask across `enchant_level` + `custom_type2`.
- **Monster Race** — `instancemanager/games/MonsterRace.java` (623, the race
  state machine), `model/actor/instance/RaceManager.java` (370, the Race Track
  NPC), `network/serverpackets/MonRaceInfo.java` (75). Mostly in-memory (an
  in-process history list); ticket items 4443–4470.

## Lottery mechanics (Lottery.java + Loto.java)

- A weekly round (`idnr`), persisted in `lottery(id=1, idnr, enddate, prize,
  newprize, finished, number1, number2, prize1, prize2, prize3)`. `prize` is the
  current pot; `newprize` accrues as tickets sell.
- `startLottery` arms `stopSellingTickets` (enddate − 10 min) and `finishLottery`
  (enddate). `isSellableTickets()` / `isStarted()` gate the NPC.
- Buying (`Loto`): the player picks 5 distinct numbers 1–20 through the dialog,
  pays `AltLotteryTicketPrice` (2000 adena); a ticket item 4442 is created with
  the round id + number bitmask; the pot grows by the configured share.
- `finishLottery`: roll 5 winning numbers → bitmask; walk every sold ticket,
  count matches → 3 prize tiers (5/4/3+bonus); write the winning numbers + tier
  payouts back to the row. Prize claim (`Loto`): a matching ticket is consumed
  for its adena payout.

## Monster Race mechanics (MonsterRace.java + RaceManager.java)

- `RaceState`: `ACCEPTING_BETS → WAITING → STARTING_RACE → RACE_END`, driven by a
  1 s `Announcement` tick that also sends the countdown system messages.
- 8 racers, each with a random 20-step speed table; `MonRaceInfo` broadcasts the
  setup, the lane info, and the run. `RaceManager` (the Track NPC) shows the
  board + `PlaySound` cues.
- Betting: a ticket on a lane (items 4443–4470) at the NPC; odds from the pool;
  `RACE_END` pays the winners and records a `HistoryInfo`.

## Slice breakdown

### Slice 1 — Lottery round lifecycle + persistence  ✅ LANDED

**Landed.** `LotteryState` on `World` (`model/lottery.rs`) + `game_loop/lottery.rs`
(the round engine): `on_loaded` (boot restore — fresh round #1 / carry a
finished row's pot into the next / resume a live round with its draw armed),
`open_round` (next-Sunday-19:00 draw via `siege::next_siege_millis`, insert row,
announce), `stop_selling`, and `finish_lottery` (slice-1 rollover: **no draw**,
whole pot carries — the number-roll + ticket-match + tiers are the `TODO(G26.5)`
slice-2 body). Persistence: `lottery` table load (`db.rs::load_lottery` +
`DbEvent::LotteryLoaded`, boot-pushed before `ClansLoaded`) + writes
(`DbCommand::{StoreLottery, FinishLottery}`). Scheduler tasks
`LotteryStart`/`StopSelling`/`Finish` (wall-clock → tick conversion like siege).
Config: the `AltLottery*` keys in `GeneralConfig` (`AllowLottery` gate). 5 tests
(fresh boot, disabled-inert, finished carry-over, live resume, finish rollover),
sabotage-verified.

**Deferred to slice 2:** the `Loto` NPC dialog moves here (it's mainly the
ticket-buy UI, so it lands with purchase), along with the draw + prize claim.

### Slice 2 — Ticket purchase + Loto NPC dialog + draw + prize claim

**Design finalized (ready to build).** The whole lottery economics — the biggest,
most parity-sensitive slice; port the arithmetic verbatim.

- **NPC dialog** (`bypass.rs` `"Loto"` verb → `lottery::loto_bypass`; port of
  `Loto.java`, NPCs 30990–30994, htmls `data/html/default/3099X-{1..6}.htm`):
  value 0 reset picks + page 1; 1–21 toggle a number pick (gated on
  started + sellable) and re-render the button HTML, swapping the Return link to
  "22" once 5 are picked; 22 buy; 23 jackpot; 24 prize-claim list; 25
  instructions; >25 claim by item object id. Per-player pick buffer = a
  `LotoPicks([i32;5])` component.
- **Encoding (verbatim):** number `n` (1–20) → `n < 17 ? enchant |= 1<<(n-1) :
  type2 |= 1<<(n-17)`. A ticket is item 4442 (non-stackable EtcItem) with
  `custom_type1 = round`, `enchant_level = enchant`, `custom_type2 = type2`.
  `decodeNumbers` reverses it; the match count is a 16-bit popcount of
  `ticket.enchant & drawn.enchant` + `ticket.type2 & drawn.type2`.
- **Purchase (value 22):** charge `AltLotteryTicketPrice` adena
  (`Inventory::adena` check + `remove_item(57, price)`), `increase_prize(price)`
  (new `DbCommand::IncreaseLotteryPrize`), create the ticket (needs a new
  `Inventory` setter for enchant/custom fields by oid).
- **Draw — two-phase async (faithful online + DB scan):** `LotteryFinish` fires
  `finish_begin`, which rolls the 5 winning numbers, stores them, and sends
  `DbCommand::LoadLotteryTickets { round }`. The DB thread replies
  `DbEvent::LotteryTicketsLoaded { round, rows }`; `finish_complete` merges those
  offline rows with a scan of every **online** inventory for item 4442 of this
  round — **deduped by object id** (an online player's ticket may already be
  flushed to the DB) — then counts tiers (5/4/3/1–2 matches → count1/2/3/4),
  computes `prize4 = count4 * 2and1`, `prizeN = ((prize - prize4) * rateN) /
  countN`, `newprize = prize - Σ`, and persists via `FinishLottery` before the
  1-minute `LotteryStart` rollover.
- **Claim (24 list / >25):** `checkTicket` reads the drawn row's numbers +
  prize1/2/3; a winning earlier-round ticket pays its adena and is destroyed.

### Slice 3 — Monster Race state machine + board
- `MonsterRaceState` (the 4-phase machine on a 1 s tick), 8 racer spawns +
  speed tables, `MonRaceInfo` packet, the `RaceManager` NPC board + sounds.
  `AllowRace` gate. **Gate:** a race runs its full cycle and animates.

### Slice 4 — Betting + payout
- Lane bets (items 4443–4470) at the NPC, odds from the pool, `RACE_END` payout
  to winners, `HistoryInfo`.

## Watch-list

- Ticket-number encoding is bitmask-across-fields (`enchant_level` +
  `custom_type2`), not a plain integer — port the exact `Loto`/`Lottery` bit
  math, don't reinvent it.
- `finishLottery` prize tiers and the pot carry-over (`newprize` when no 1st-tier
  winner) are load-bearing retail economics — port the arithmetic verbatim.
- The race `Announcement` tick both advances state *and* sends countdown SMs;
  keep the two coupled as Java has them.
