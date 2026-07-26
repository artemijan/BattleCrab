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

### Slice 1 — Event lifecycle + TvT registration phase  ✅ LANDED

**Landed.** `model/event.rs` (`EventManager` + `TvtState`/`TvtPhase`, a `World`
field); `game_loop/events/{mod,tvt}.rs` (name-dispatched `start`/`stop` +
the TvT `event_start`/`event_stop`/`teleport_to_arena` runtime); `scripts/tvt.rs`
(the manager NPC 70010 talk/first-talk routing `Participate`/`CancelParticipation`);
`game_loop/admin/events.rs` (`//event_start [name]` / `//event_stop [name]` —
the config schedule ships commented out, so this is the operator trigger);
`ScheduledTask::TvtTeleportToArena` (the registration-close timer);
`ChatType::Announcement` (`Broadcast.toAllOnlinePlayers`); `Player.on_event` /
`registered_on_event` flags. The 8 dist HTML files load as-is from
`data/scripts/custom/events/TeamVsTeam/`. `canRegister` ports the gates whose
state exists (level 76–200, already-registered, max-count, cursed weapon /
reputation, olympiad-registration, fishing) with `TODO(G28)` at the site for the
rest (flying/transform/inventory-80%/weight/duel/instance/siege). The
window-close handler fully implements the **too-few-participants cancel**; the
enough-players path is a `TODO(G28)` stub that ends the event cleanly until
slice 2 stands up the arena. 6 tvt tests (sabotage-verified).

The original slice-1 plan below.
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

### Slice 2 — Teleport to arena, team split, instance stand-up  ✅ LANDED

**Landed.** `teleport_to_arena`'s enough-players branch: `create_from_template(3049)`
(coliseum doors default closed), shuffle + strict-alternate split into BLUE/RED
from a random start side, `instances::enter` + `teleport_player` to the team
spawn, `team`/`on_event` flags, the two arena buffers (manager NPC reused),
`ExPVPMatchCCRecord::INITIALIZE` (new packet + `EX_PVP_MATCH_CCRECORD` 0x8A opcode
+ `PVP_MATCH_*` state consts), then `TvtPhase::Warmup` + the `StartFight` timer.
`start_fight` opens the BLUE/RED doors, screen-messages "The fight has began!",
arms `EndFight`. `end_fight` (slice-2 minimal) screen-messages the end and tears
the arena down via `instances::destroy` (ousts everyone to their ORIGIN return
loc, despawns arena NPCs/doors) + clears `team`/`on_event`. `event_stop` now
destroys a live arena too. New scheduler tasks `TvtStartFight`/`TvtEndFight`;
`TvtPhase::{Warmup,Fighting}`. 4 new tests (arena stand-up + 2/2 split, door/timer
window, teardown+free, full start→finish run), sabotage-verified.

**Deferred to slice 3/4 (TODO(G28) at the sites):** parties-of-7 + command
channels + `leaveParty` + `addDeathListener` (need a CC primitive); the 5..1 /
10..1 countdown screen messages (cosmetic); and the real `EndFight` — freeze
players, revive the dead, resolve BLUE vs RED (firework + adena / tie social
action), `ExPVPMatchCCRecord::FINISH`, the 7s scoreboard delay.

### Slice 3 — Scoring + respawn  ✅ LANDED

**Landed.** `on_player_death` — hooked into `death::player_do_die` (after the
death broadcast, mirroring the cursed-weapon hook), no-op off-event: a cross-team
kill scores for the killer's side + the killer's personal tally, broadcasts the
"Blue: X - Red: Y" bottom-right tally + `ExPVPMatchCCRecord::UPDATE`, and queues
the victim's respawn (`ScheduledTask::TvtResurrect`, 10s). `resurrect_player` —
still-dead + still-on-event guard, teleport to team spawn, `do_revive`, then the
Ghost Walking skill (100000 — `DamageBlock` HP/MP invuln + Speed, already ported
in G19) for 30s. 5 new tests incl. one driving the real `player_do_die` wire
(sabotage-verified the hook).

**Deferred to slice 4 (TODO(G28) at the site):** the **zone enter/exit
listeners** (enemy-HQ kick + inactivity `KickPlayer` timers) — they need the
`on_enter_zone`/`on_exit_zone` framework hooks — and the manager `BuffHeal`
(needs `SkillCaster.triggerCast` on the in-arena manager). The 5..1/10..1
countdown screen messages stay deferred (cosmetic).

### Slice 4 — End, rewards, forfeit, logout  ✅ LANDED

**Landed.** Real `end_fight` (`TvtPhase::Ending`, idempotent): close doors, freeze
participants (`set_invul` via `AdminFlags` — Java also immobilizes + skill-locks,
`TODO(G28)` no flag on this port) + revive the dead, resolve BLUE vs RED — winner
side gets the firework flourish (`MagicSkillUse` skill 5965) + cheer social
action + Adena 57×100000 (`give_item_with_earned_message`), a tie shrugs (social
13) — then arm `ScoreBoard` (3.5s → `ExPVPMatchCCRecord::FINISH`) and
`TeleportOut` (7s → unfreeze + `instances::destroy` + reset). `on_player_logout`
(hooked into `net::handle_logout` **and** `on_disconnect`, no-op off-event): drop
the participant from every list; if that empties one team mid-arena,
`manage_forfeit` arms an early `EndFight` (the original `FIGHT_TIME` timer's later
firing no-ops via the `Ending` guard — no timer cancellation needed). New
scheduler tasks `TvtScoreBoard`/`TvtTeleportOut`; `TvtPhase::Ending`. 6 new tests
(winner reward + freeze, tie, teardown/unfreeze, forfeit-on-logout, full
end-to-end with a winner), sabotage-verified.

**Deferred (still `TODO(G28)`):** enemy-HQ zone kicks + inactivity `KickPlayer`
timers (need the `on_enter_zone`/`on_exit_zone` framework hooks + the
`colosseum_peace1/2` zones); the manager `BuffHeal`; parties-of-7 + command
channels; the countdown screen messages. These are polish on top of a
now-complete match; a follow-up or slice 5.

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
