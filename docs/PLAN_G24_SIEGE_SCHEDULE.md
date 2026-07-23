# G24 slice 1 — the automatic siege schedule

## Why

G24's siege *combat* is extensively built (towers, guards, flags, doors, the
throne-room artifact capture, siege zones, PvP relations, `start_siege`/
`end_siege`/`capture`). But **sieges only ever start from a GM command**
(`//castlemanage startSiege`) — `SiegeSchedule.xml` (the weekly per-castle
calendar) is never loaded, so on a real server no siege ever fires. This slice
makes the schedule real: each castle's siege starts itself on its scheduled
day and hour, then re-arms for the next week.

Milestone-status note (the G20.5 lesson): the `capture`/`try_capture_artifact`
path is **already reachable** — the Holy Artifact (35063 etc., type
`Artefact`) is a permanent castle spawn and the interaction is siege-gated, so
the `#[allow(dead_code)]`/"nothing reaches capture" comments on `capture` are
stale. This slice removes the `allow` and pins capture with a test, alongside
the schedule.

## Design (no DB/model changes)

The schedule is a fixed weekly calendar, so the next siege time is a pure
function of the wall clock — no persisted `siegeDate` is needed. Boot computes
the next occurrence and arms a timer; when it fires, the siege starts and the
**next week is re-armed immediately**, so the timer perpetuates itself whether
or not a given siege actually runs (a castle with no registered attackers just
holds — Java's behaviour too).

- **`SiegeSchedule.xml` loader** (`data/siege_data.rs`): `castleId → { weekday,
  hour, enabled }`. `day="SUNDAY"` etc. → a `Mon=0..Sun=6` weekday. Into
  `GameData.siege_schedule`.
- **Pure date math** (`siege::next_siege_millis(now, weekday, hour)`): the next
  `weekday`@`hour`:00 **UTC** strictly after `now`. 1970-01-01 was a Thursday,
  so `weekday_mon0 = (now/86_400_000 + 3) % 7`; step forward to the target
  weekday and hour, +7 days if that lands at-or-before now. Documented
  divergence: Java uses server-local time; Rust std has no timezone, so this is
  UTC. (A deployment sets its clock; the weekly cadence is exact either way.)
- **Boot** (`schedule_all_at_boot`, from the `SiegesLoaded` handler where the
  per-castle `Siege`s already exist): arm `SiegeStart { castle_id }` at the
  next occurrence for each **enabled** castle.
- **`SiegeStart` task** → `start_siege(castle_id)` (which no-ops if already in
  progress) **and** re-arm next week's `SiegeStart`.

## Tests

1. `next_siege_millis`: the result is strictly future, on the target weekday,
   at `hour`:00 UTC, and within 7 days; a target earlier today rolls to next
   week; an anchored case from a known Sunday.
2. Schedule load: all 9 castles, Sunday, the right hours (16/20), enabled.
3. Boot arms one `SiegeStart` per enabled castle; firing it starts the siege
   (in_progress) and re-arms exactly one more `SiegeStart` (the weekly
   perpetuation — sabotage the re-arm and the count drops).
4. Capture still works and is no longer dead code: an attacker touching the
   artifact mid-siege takes the castle (removes the stale `#[allow]`).
