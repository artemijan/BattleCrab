# G21 slice 6 — NPC HP/MP regeneration

Sixth G21 slice. `CreatureStatus.doRegeneration` existed in the port but only
ever ran for **players** — every NPC in the game was frozen at whatever HP it
was left on.

## Why this over the remaining zones

`DamageZone` (35) and `SwampZone` (20) looked like the cheap next step now that
the zone parser and sweep exist. Checking `default_enabled` first says
otherwise:

| Type | Total | Enabled by default | Needs a siege script |
|---|---|---|---|
| `DamageZone` | 35 | **13** | 22 (castle traps) |
| `SwampZone` | 20 | **2** | 18 (castle traps) |

15 live zones between them. Against that:

| Fact | Number |
|---|---|
| NPC templates declaring `hpRegen` | **14855** |
| …of which zero | 58 |
| Most common values | 8.5 (5467), 7.5 (3380), 10.5 (1291) |

`base_hp_reg`/`base_mp_reg` were *parsed* and then read by nothing. So a
wounded mob stayed wounded until it despawned, and a raid boss whittled down
over several attempts never recovered a point — you could grind any boss down
across sessions. 14855 templates' worth of data doing nothing beats 15 zones.

## The NPC formula is much shorter than the player one

That's Java, not a narrowing. In `RegenHPFinalizer` the level-mod, the CON/MEN
bonus and the sitting/standing/running multipliers all sit **inside**
`if (creature.isPlayer())`. An NPC's rate is just:

```
base_hp_reg × (isRaid ? RaidHpRegenMultiplier : HpRegenMultiplier)
```

Both multipliers are 100 % on this dist, so in practice a mob regenerates its
raw template value every 3 s. The raid branch is ported anyway and tested by
overriding the config, since the shipped value makes the two paths
indistinguishable.

## Regen runs *during* combat

Java's task checks only "not dead" and "not already full" — never an in-combat
flag. So a high-regen boss turns a long fight into a DPS race. That reads like
a bug and isn't; there's a test named for it so nobody "fixes" it later.

## Broadcast discipline

A heal broadcasts `StatusUpdate` to the region so targeting players see the HP
bar move — but only when HP actually changed. Without that guard every full-HP
NPC in the world would emit a packet every 3 s (tens of thousands, for no
visible change). The tick also filters to wounded NPCs up front, so the common
case is one component read per NPC and nothing else.

## Tests

10 in `game_loop/tests/npc_regen_tests.rs`: heals at the template rate, clamps
at max, a corpse doesn't heal, MP regenerates, the raid multiplier applies to a
raid and *not* to a Monster, regen continues during combat, the HP bar is
broadcast, a full-HP mob broadcasts nothing, and a dist check that >10k
templates carry a positive `hpRegen`.

**694 lib tests green**, `char_persistence` 7/7, `e2e_create` 1/1 (×2).

## A fixture trap worth noting

`add_test_npc` hard-codes a 100/50 HP/MP pool regardless of the template. Seven
tests failed at first because `cur_hp = 500` landed *above* `max_hp = 100`, so
the tick correctly treated the mob as already full. Raise the pool before
setting the current value — the same ordering trap recorded in earlier slices.

## Deliberate narrowings (`TODO(G21)` at the site)

- `REGENERATE_HP_RATE`/`REGENERATE_MP_RATE` stat modifiers from buffs aren't
  folded in — Java's `Stat.defaultValue` would apply them. No NPC-facing regen
  buff exists in the ported effect set yet.
- Doors regenerate at 1/100th the rate (`getRegeneratePeriod` × 100); doors
  don't regenerate here at all.
- `Config.CHAMPION_HP_REGEN` — champion mobs aren't modelled.

## Next in G21

- `DamageZone` (13 live) + `SwampZone` (2 live) — small, and both now cheap.
- NPC pathfinding — the G7.85 worker for NPCs (they still move straight-line).
- Wire `skillTargetReconsider` (faction data landed in slice 2).
- Fences (`FenceData`), `HtmCache`, walker routes, `CreatureSeeTaskManager`.
