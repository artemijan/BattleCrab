# G21 slice 9 — `DamageZone` + `SwampZone`

Ninth G21 slice, and the last zone types with live content on this dist. Both
reuse the parsing and sweep infrastructure from slice 5, so this was cheap.

## Content

| Type | Total | Enabled by default | Siege-gated castle traps |
|---|---|---|---|
| `DamageZone` | 35 | 13 | 22 |
| `SwampZone` | 20 | 2 | 18 |

Zone census after this slice: **898** (was 843) — Damage 35, Swamp 20, Effect
218, Water 423, Peace 134, NoRestart 47, Pvp 12, Siege 9.

**No `DamageZone` in this dist declares `damageHPPerSec`**, so every one of them
uses Java's field default of **200** per tick. Worth stating because the number
appears nowhere in the datapack — reading only the XML would suggest these
zones do nothing.

## What landed

**`DamageZone`** — `damage_zone_tick`, the same shape as the effect sweep: one
pass per second, players grouped by occupied zone, each zone firing on its own
`reuse` (Java's `DamageZone` default is **5000 ms**, not the `EffectZone`'s
30000 — the parser corrects for that). Flat HP and MP loss.

**`SwampZone`** — a move-speed multiplier (0.2 on this dist; Java's field
default is 0.5). Java re-reads the zone inside `SpeedFinalizer` on every stat
computation; the port caches the multiplier on `Speeds` and refreshes it in
`revalidate_zone` on the enter/exit edges, so the hot stat path stays free of
world lookups. On a change it recomputes speeds and rebroadcasts `UserInfo`,
matching Java's `broadcastUserInfo()` on both edges. The multiplier is applied
inside `recalculate_stats` at `SpeedFinalizer`'s position — after the run-speed
boost, before the `MaxRunSpeed` clamp.

## Castle traps: siege-gated, and they spare the garrison

Both types accept a `castleId`, and most of the dist's instances have one (22 of
35 damage zones, 18 of 20 swamps live in `castle_trap.xml`). Java gates them
twice:

1. Nothing happens unless that castle's siege is **in progress**.
2. Even then, players **defending that castle** are skipped.

The second rule is the one that would be easy to miss and very visible: without
it a castle's own garrison would cook itself on its own defences during the
siege it is fighting. Both rules have a test.

## Tests

9 in `game_loop/tests/damage_swamp_tests.rs`: damage inside / safe outside /
disabled zone does nothing; a castle trap is inert with no siege, bites during
one, and spares a defender; a swamp slows by the declared factor, restores full
speed on exit, and a castle swamp is inert outside a siege. Plus the dist
census assertions in `zone_data.rs` (35 damage, 20 swamp, 898 total).

**721 lib tests green**, `char_persistence` 7/7, `e2e_create` 1/1. The one build
warning (`is_in_duel`) predates this work.

## Deliberate narrowings (`TODO(G21)` at the site)

- `DAMAGE_ZONE_VULN` — Java scales the damage by a stat multiplier; no effect
  in the ported set grants it, so it is always 1.0 here.
- `OnEventTrigger` on swamp enter/exit (the client-side visual for zones with
  an `eventId`) is not sent.
- Nothing enables a `default_enabled="false"` zone at runtime yet — the siege
  scripts that would flip the non-castle-gated traps don't exist.

## G21 status

The remaining tail is all low or zero value on this dist:

- **Walker routes** — 14 routes, **12** NPCs (`TownNpcWalkers.xml`). Small but
  real; the only remaining item with visible content.
- **`HtmCache`** — 2629 `.htm` files, already read at runtime. Caching only, no
  behaviour change.
- **`CreatureSeeTaskManager`** — a trigger for AI *scripts*; there is no script
  engine yet, so it would fire into nothing.
- **`FenceData`** — **one** fence, named `"demo"`. Not worth porting.
