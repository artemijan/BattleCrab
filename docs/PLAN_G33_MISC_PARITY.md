# PLAN — G33 Misc parity & finishing sweep

The residuals milestone: the wall-clock task manager and the resets riding on
it, the game-time clock, periodic autosave, offline-trader restore, `//geosave`,
the niche admin tools, and the file-by-file parity checklist. **Gate:** parity
checklist complete. Delivered slice-by-slice, highest leverage first.

## What already exists (verified)

- **The daily reset skeleton**: `reco.rs`'s `schedule_initial_daily_reset`
  (next 06:30 **UTC**) + `handle_daily_reco_reset` (`ScheduledTask::
  DailyRecoReset`) — recommends only. This is the pattern the rest of
  `DailyTaskManager` folds into.
- **Vitality** (`vitality.rs`): the pool (`Player.vitality_points`, 0..=140000),
  `set_vitality_points`, and the mob-kill/premium drain — but **nothing ever
  refills it** (the daily/weekly resets are `TODO(G33)`), so vitality only ever
  goes down. `MAX_VITALITY_POINTS = 140000`.
- No `GlobalVariablesManager` in the port → the boot **catch-up** (run a reset
  missed during downtime) has no persistence hook yet.

## Java sources

- `instancemanager/DailyTaskManager` — `onReset` at 06:30 daily (Java Calendar,
  server-local; the port uses UTC): on **Wednesday** `resetVitalityWeekly`
  (set `MAX_VITALITY_POINTS`), else `resetVitalityDaily` (add
  `MAX_VITALITY_POINTS / 4` = 35000), plus `resetRecommends` (ported). The rest
  (`resetDailySkills/Items/PrimeShop/Missions`) are off-chronicle for Interlude
  Classic. Offline population updated by two SQL `CASE WHEN` statements.
- `GameTimeTaskManager` — the in-game clock (1 game-min ≈ 10 real-s), sent in
  `CharSelected`/`UserInfo` (both write `0` today).

## Slice breakdown

### Slice 1 — DailyTaskManager + vitality refills  ⬅ start here
- Generalise `DailyRecoReset` → a single `DailyReset` task (Java `onReset`) in a
  new `daily_tasks` module: run `reset_recommends` (moved from `reco.rs`) +
  `reset_vitality` (daily add 35000 / weekly-on-Wednesday full), online (via
  `set_vitality_points`) **and** offline (a new `DbCommand::ResetVitality`),
  then reschedule 24 h out. Gated on `enable_vitality`.
- **Gate for the slice:** at the 06:30 tick, an online player's vitality pool
  goes **up** (daily) — the drain-only bug is fixed — and the weekly branch
  refills to max. Boot catch-up stays a documented `TODO(G33)` (no
  GlobalVariables yet).

### Slice 2 — Game-time clock
- Port `GameTimeTaskManager`: a game-time counter off `world.tick`, fed into
  `CharSelected` and `UserInfo` (both hardcode `0`), plus the day/night state.

### Slice 3 — Periodic autosave cadence ✅ **already done — verified 2026-08-03**

Checked rather than ported, and it was already complete: `game_loop::autosave_tick`
is `PlayerAutoSaveTaskManager.run`, on the same 1 s fixed-rate sweep, flushing
**at most one** due player per sweep — Java's `break; // Prevent SQL flood` —
and rescheduling it one `CharacterDataStoreInterval` (15 min on this dist) out.

The snapshot covers everything `Player.autoSave()` does: `storeMe`,
`storeRecommendations` (`rec_have`/`rec_left` are on the save struct) and, per
`UpdateItemsOnCharStore`, all three item containers — inventory, warehouse and
freight. `lobby_tests::autosave_flushes_one_due_player_and_reschedules` already
pins the one-per-sweep guard and the reschedule.

This plan entry was simply stale. Recorded rather than re-ported.

### Slice 4 — Parity audit
- Mechanical diff of Java `network/clientpackets` (298 handlers) against the
  Rust opcode table; the one-time `Custom/*.ini` enable-flag audit; surface any
  packet family that slipped a milestone. Close the file-by-file checklist.

### Later slices (as scope allows)
- ~~Offline-trader restore~~ **DONE 2026-07-31** (`game_loop/offline_trade.rs` +
  `config/offline_trade.rs`): logout/`.offline` leaves the shop standing, it
  keeps trading through `world.offline_traders`, and `DbEvent::
  OfflineTradersLoaded` brings it back at boot.
- ~~`//geosave` binary-region serializer~~ **DONE** — `geo::save_region` +
  `admin::geo_editor` (`//geosave`, `//geosaveall`), with the runtime NSWE
  overrides folded into the written cells. Round-trip tested
  (`geosave_writes_the_edited_region`: write, reload, find the edit, and
  confirm untouched cells did not move).
- ~~AdminFightCalculator / AdminRepairChar / AdminPcCondOverride~~ **DONE**
  (`admin/gm_util.rs`, `admin/mod.rs`, `db.rs`).
- Still open, re-verified against the code 2026-08-03:
  - **AdminPForge** and **AdminMissingHtmls** — no trace in the port.
  - **Precautionary/scheduled restart + deadlock detector** — the config keys
    parse (`config/server.rs`) but **nothing reads them**: a parsed flag with
    no consumer, which is this project's most-repeated failure shape.
  - **`NpcNameLocalisationData` / multilang** — absent.
  - **Dockerfile parity** — no `Dockerfile` in the tree.

  Everything above this line was listed as outstanding until it was checked;
  five of the eight items had already landed. Check the code before working
  from this list.

## Watch-list

- The port computes 06:30 in **UTC** (like the reco reset), not server-local —
  keep that consistent; "Wednesday" is therefore UTC-Wednesday.
- Java's offline vitality SQL adds `MAX/4` **uncapped**; the read-side clamp
  (`vitality_points`) hides any overflow, so it is observably equivalent.
- Subclass vitality: Java resets per-subclass too — port only if the schema
  carries `character_subclasses.vitality_points`, else `TODO(G33)`.
