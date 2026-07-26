# PLAN — G28 Events engine (TvT), the second half of G28

The cursed-weapon half of G28 landed already
([PLAN_G28_CURSED_WEAPONS.md](PLAN_G28_CURSED_WEAPONS.md), the drop→pickup→expiry
loop). This plan covers the **events-engine** half of the G28 gate:

> **Gate:** a TvT event runs start-to-finish.
> **Unblocks:** `AdminEvents`, `//event*` (`//tvt_*`).

## What the Java side actually is

The representative event is `dist/game/data/scripts/custom/events/TeamVsTeam/TvT.java`
(843 lines) — a `Quest`-derived `Event` script (`Event extends Quest`, adding
`eventStart`/`eventStop`/`eventBypass`). There is **no separate "EventManager"
engine class** in this dist beyond that thin `Event` base; the event *is* a
script that registers talk/first-talk/zone/death/logout listeners and drives
itself with quest timers. So on the Rust side "the events engine" = a small
lifecycle wrapper (start/stop a named event) plus the framework extensions the
event needs (zone / player-death / logout listeners), not a large new subsystem.

**Config note:** this dist ships `config.xml` with the `<schedule>` pattern
**commented out** — TvT never auto-starts here; it is meant to be GM-triggered
via AdminEvents. Per [[l2r-config-disabled-still-port]] we still port it (an
operator may enable the schedule), but the *gate* is met via the admin trigger,
and the cron `SchedulingPattern` auto-start is the last (optional) slice.

## Primitives that already exist (verified in the tree)

- Instances (G27): `game_loop/instances.rs::{create_from_template, open_close_door,
  spawn_npc, broadcast_to_instance, enter, exit, destroy}`, `model/instance.rs`
  (`add_member`/allowed, doors, vars). Coliseum template `3049` is at
  `dist/game/data/instances/custom/coliseum.xml`.
- `ExShowScreenMessage` (`network/server_packets/effect.rs`).
- `Creature.team: u8` (`model/mod.rs:191`; 0 none / 1 blue / 2 red) + `//setteam`.
- Parties (`model/party.rs`), quest/script framework with talk/first_talk/event/
  timer/kill/attack/spawn/skill_see (`game_loop/quests.rs`), NPC spawn from code.
- Duel / olympiad-registration / instance / siege state (for `canRegister` gates).

## Primitives that DON'T exist yet (this milestone builds them)

1. **Event lifecycle wrapper** — a `GameEvent` trait + registry + a `//event_start
   <name>` / `//event_stop <name>` admin command (AdminEvents subset). Holds the
   active TvT runtime as a `World` resource.
2. **`ExPVPMatchCCRecord`** server packet (scoreboard: INITIALIZE/UPDATE/FINISH).
3. **Script-framework listener extensions:** `on_enter_zone` / `on_exit_zone`
   (per zone id), `on_player_death` (player killed, not NPC `on_kill`), and
   `on_player_logout`. TvT is the first script needing all three.
4. **Command channel** (CC of parties) — TvT groups each team into parties of 7
   and CCs them. A thin port (or a per-team party list) suffices; no CC combat
   mechanics are exercised by TvT beyond membership.
5. **`SchedulingPattern`** (cron 5-field) — only for the optional auto-schedule
   slice; the admin trigger doesn't need it.

## Slice breakdown

### Slice 1 — Event lifecycle + TvT registration phase  ⬅ start here
- `game_loop/events/mod.rs`: `GameEvent` trait (`name`, `event_start`,
  `event_stop`) + `EventManager` resource (active event + TvT runtime state:
  phase, `player_list`, `scores`, `blue`/`red`, world id).
- `game_loop/events/tvt.rs`: `event_start` → spawn manager NPC 70010 at
  `MANAGER_SPAWN_LOC`, open registration for `REGISTRATION_TIME`, broadcast the
  two announcements, arm the `TeleportToArena` timer.
- Manager talk/first-talk: register / cancel dialogs + `Participate` /
  `CancelParticipation` (PLAYER_LIST + `registered_on_event` flag + `canRegister`
  eligibility). Port the 8 HTML files under an events html dir.
- Admin: `//event_start TvT`, `//event_stop TvT`, `//event_menu`.
- **Gate for the slice:** GM starts TvT; a player talks to the manager and
  registers; cancel works; registration window closing with < min players
  cancels cleanly.

### Slice 2 — Teleport to arena, team split, instance stand-up
- `TeleportToArena`: prune offline, min-count check, `create_from_template(3049)`,
  close doors, shuffle + split BLUE/RED, teleport to spawn locs in the instance,
  set `team`, add allowed; parties-of-7 (+ CC), spawn the two buffers.
- `ExPVPMatchCCRecord::INITIALIZE` broadcast; `StartFight` countdown timers.

### Slice 3 — Fight lifecycle + scoring + respawn
- `StartFight` (open doors, screen countdown), **player-death listener** → team
  score + `ExPVPMatchCCRecord::UPDATE` + score screen message; `ResurrectPlayer`
  (Ghost Walking invuln + respawn at team spawn); **zone enter/exit listeners**
  (enemy-HQ kick + inactivity kick timers); manager `BuffHeal`.

### Slice 4 — End, rewards, cleanup, forfeit, logout
- `EndFight` (freeze players, revive dead, winner firework + adena reward, tie
  social action), `ScoreBoard` FINISH, `TeleportOut` + `destroy`, `manageForfeit`,
  **logout listener** + `event_stop` full cleanup.

### Slice 5 (optional) — Auto-schedule + AdminEvents polish
- Port `SchedulingPattern` (cron), wire `config.xml` schedule (off by default),
  fuller AdminEvents menu (`//event`, list/next).

## Java-faithfulness watch-list (fill in as slices land)

- `canRegister` has ~14 gates (level, weight/inventory-80%, cursed weapon /
  reputation, duel, olympiad, instance, siege, fishing, transform, flying) —
  port the ones whose state exists; `TODO(G28)` the rest at the site.
- Team assignment: extra odd player goes to a *random* team (`getRandomBoolean`).
- Reward is Adena 57 ×100000; winner also fires `CommonSkill.FIREWORK` +
  social action 3; tie is social action 13.
- Respawn invuln is skill `100000` "Ghost Walking" (custom) — verify it exists
  in skill data / add if missing.
- Manager despawn is timed (`REGISTRATION_TIME`), buffers despawn at
  `WAIT_TIME + FIGHT_TIME`.
