# PLAN G22 — Q00210 Obtain a Wolf Pet

**Status:** landed (branch `feat/g22-wolf-pet`). A standalone dialog quest that
closes a dangling-value connection: it is *how a player acquires the starter
wolf pet*, and the G29 pet system that consumes the reward is already built.

## Java source (`quests/Q00210_ObtainAWolfPet`, 186 lines)

A pure four-NPC talk chain, no kills, no item collection:

- **Lundy (30827)**, Gludin, `addCondMinLevel(15, "no_level.htm")` — starts the
  quest (cond 1), and at the end hands over the **Wolf Collar (2375)**.
- **Bella (30256)** → cond 2, **Bynn (30335)** → cond 3, **Sydnia (30321)** →
  cond 4, each gated on the prior cond.
- Back to **Lundy** at cond 4 → `rewardItems(WOLF_COLLAR, 1)` +
  `exitQuest(false)` (one-time).

## Port — `scripts/q00210_obtain_a_wolf_pet.rs`

Straight `QuestScript` transcription following the established pattern:
`on_event` advances cond behind the `is_cond(n)` guards and pays out at
`30827-05.html`; `on_talk` serves the per-state/per-NPC pages. The min-level
gate lives in `on_talk`'s CREATED branch (`player_level() < 15 → no_level.htm`),
the same shape the other ported quests use for `addCondMinLevel`.

**Java quirk kept:** `onEvent` lists `30827-04.htm` as a no-op page, but no html
button links it (verified by grepping the html set) — dead in Java, transcribed
anyway rather than "cleaned up".

Registered in `scripts::build_registry`.

## Tests — `game_loop/tests/quests_tests.rs` (113 → 115)

- The full chain through the **real bypass router**
  (`handle_request_bypass_to_server`): cond 1→4 and the Wolf Collar rewarded,
  one-time COMPLETED. Includes an **out-of-order guard** assertion (clicking
  Bynn while still at cond 1 does not skip ahead).
- The **level gate**: a level-14 starter gets `no_level.htm` (an `.htm` file, so
  it ships as `ExNpcQuestHtmlMessage`, not a plain `NpcHtmlMessage`) and the
  quest stays un-started.
- Registration sabotage-verified (the router e2e fails when the script is
  unregistered); the cond guard sabotage-verified.
