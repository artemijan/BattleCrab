# PLAN — G25 Grand Olympiad & Hero

The marquee Interlude PvP endgame. Java `model/olympiad/*` (~5.5k LoC) +
`ai/others/OlyManager` (the manager NPC) + `handlers/admincommandhandlers/
AdminOlympiad`. Ported in `l2r_interlude` as a `World.olympiad` state manager
(`model/olympiad.rs`) plus `game_loop/olympiad.rs`.

Dist facts pinned down (this Interlude-Classic build):
- **Registration is NON-CLASSED 1v1 only** — the `OlyManager` NPC's only
  register verb is `register1v1 → CompetitionType.NON_CLASSED`. The class-based
  queue exists in `OlympiadManager` but nothing registers into it here.
- **Eligibility is the "Classic noble equivalent"**: 3rd/4th class group **and**
  level ≥ 55 (not the `_noble` flag), plus not-on-a-subclass and Olympiad points
  > 0 (the NPC gates the subclass/points; the manager gates class+level+timing).
- Config `Olympiad.ini`: start points 10, weekly points 10, max 30 matches/week,
  20 classed / 20 non-classed participants, comp window 18:00 for 6 h, weekly
  refresh 1 week, validation 24 h.
- `getClassGroup` → base class id in Interlude (the SIXTH_*/ERTHEIA branches are
  later-chronicle, unreachable).
- Manager NPC = 31688; SQL `olympiad_data` / `olympiad_nobles` /
  `olympiad_nobles_eom`.

## Slices

1. **Noble registration (DONE).** `OlympiadState` on `World` (period, cycle,
   `in_comp_period`, `comp_end_tick`, the noble registry, the two queues).
   `register`/`unregister` with the gates (period open, 20-min reg cutoff,
   weekly cap, already-registered, class+level eligibility), creating the noble
   record with the starting points on first join. Wired to the manager NPC
   (31688) bypass verbs `register1v1` / `unregister`.
2. **The Grand Olympiad Manager NPC dialog.** Port `OlyManager` first-talk +
   HTML pages (`OlyManager-*.html`): join/leave buttons, info/rules/points/rank
   pages, the period/participant substitutions. The subclass/points/weight gates
   the NPC applies before calling register.
3. **DB persistence + the period state machine.** Load/save `olympiad_data` +
   `olympiad_nobles`; the daily competition-window open/close scheduling
   (18:00 + 6 h), the weekly point refresh, and the period 0↔1 transitions.
4. **Match-making + the stadium.** Pair waiting nobles (`hasEnoughRegistered`,
   wait time), teleport to a stadium, buff-strip + restore, the countdown.
5. **Match run + scoring.** Fight to the death/timeout, win/loss/draw, points
   transfer, competitions_done/won/lost, the spectator packets.
6. **Monthly hero calculation + hero status.** End-of-period ranking, crown the
   class leaders as heroes (`Hero`), grant hero skills + the aura/`//sethero`
   integration, hero diary/message board.

Later-chronicle bits (class-based queue population, `SIXTH_*` groups) stay out
of scope.
