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

### Slice 1 — Lottery round lifecycle + Manager info  ⬅ start here
- `lottery` table load/persist (via `db.rs`, the SQL-manager pattern) →
  `LotteryState` (round id, prize, enddate, sellable/started) on `World`.
- The weekly scheduler: `start → stopSelling (−10 min) → finish` as
  `ScheduledTask::Lottery*` transitions (the finish *draw* is slice 2).
- The `Loto` NPC bypass dialog skeleton: current round, pot, end date, and the
  "sellable / not started" branches. `AllowLottery` gate.
- **Gate:** with lottery enabled, the Manager shows the live round + pot and the
  round rolls over on schedule.

### Slice 2 — Ticket purchase + draw + prize claim
- Number-pick dialog → create ticket item 4442 (round id + bitmask), charge
  adena, accrue the pot. `finishLottery` draw (5 numbers → tiers → payouts,
  persisted). Prize check/claim: consume a winning ticket for its adena.

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
