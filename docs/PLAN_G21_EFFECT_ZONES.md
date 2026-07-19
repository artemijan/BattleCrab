# G21 slice 5 — `EffectZone` + per-zone `type=` parsing

Fifth G21 slice, and the first into the zone backlog. Zones that periodically
cast on the players inside them now work: the Blazing Swamp burns, the Sea of
Spores poisons, the Hot Springs hand out Haste/Focus/Might.

## Survey — and why *not* ConditionZone

2779 `<zone>` elements exist; only 5 kinds were ported. By raw count the
obvious target was `ConditionZone` at **1080**. It's the wrong target:

| Zone type | Count | Verdict |
|---|---|---|
| `ConditionZone` | 1080 | **1073 are `NoBookmark=true`** — bookmarks are a later-chronicle feature that doesn't exist in Interlude. Only 7 (`NoItemDrop`) do anything here. Effectively dormant. |
| `EffectZone` | **218** | Live: 204 declare a skill list. **Picked.** |
| `ScriptZone` | 133 | Inert without the script engine. |
| `TaxZone` | 122 | Needs castle tax income (G24-ish). |
| `DamageZone` | 35 | Mostly castle traps, `default_enabled=false`. |
| `SwampZone` | 20 | `move_bonus=0.2`; siege-gated. |

So the biggest number in the census is ~99 % inert on this chronicle, and the
real gameplay sits in a group a fifth its size. Counting elements would have
picked the wrong slice.

The EffectZone skills are already-ported effects: `4150` Flame and `4148`
Poison are `DamOverTime`, `4644/4645/4647` (Haste/Focus/Might) are stat
modifiers.

## `targetClass="Npc"` zones cast on nobody

27 EffectZones declare `targetClass="Npc"`. In Java that makes `ZoneType`
track only NPCs as being "inside" — but `EffectZone.ApplySkill` *also* requires
`character.isPlayer()`. The two filters are mutually exclusive, so those 27
zones apply skills to **no one**. Modelled explicitly as
`EffectZoneParams.casts_on_players` (with a test) so they stay inert rather
than being "fixed" into life by a later reader.

The default is the opposite way round — absent `targetClass` means `Creature`,
i.e. everyone — and I had it backwards on the first pass; the dist parse test
caught it (207 vs the expected 27).

## Per-zone `type=` parsing

The loader mapped *filename → kind*, with a standing note that
`underground_coliseum.xml` "mixes zone types and needs per-zone `type=` parsing
before it can be loaded — deferred". EffectZones are spread over six files,
several mixed, so that had to land first. Each zone's own `type=` now wins,
and a zone whose type isn't ported is skipped outright rather than mis-filed
under the filename fallback (a test asserts no unported kind can sneak in).

**Bonus fix:** the mixed files also contain zones of kinds already ported —
loading them recovered **20 zones that were silently missing from the world
entirely**: +7 Peace, +7 NoRestart, +6 Pvp. Total zones 605 → 843.

## Runtime shape (deliberate difference from Java)

Java starts a per-zone `scheduleAtFixedRate` when someone enters and cancels it
when the zone empties — which needs a live "characters inside" set per zone.
The port has no such set, so instead one global sweep runs every second, groups
players by the effect zones they occupy, and fires each zone whose own `reuse`
has elapsed. Same observable behaviour (per-zone cadence, per-creature chance
roll) without the enter/exit bookkeeping; an empty zone costs a hash lookup and
never advances its timer.

Ported faithfully within that: the chance is rolled **once per creature, not
per skill**; `initialDelay` defers the first fire; and Java's
`getAffectedSkillLevel(id) < level` guard means a buff zone grants its buff
**once** instead of re-casting every 6 s forever.

## Tests

9 behaviour tests in `game_loop/tests/effect_zone_tests.rs` (damage inside /
safe outside; fires on its own reuse rather than every sweep; disabled zone;
`targetClass=Npc` zone; chance 0; buff granted once not stacked; dead player
skipped; multi-skill tick) plus 2 dist-backed parse tests asserting 218
EffectZones, 27 npc-targeted, the Blazing Swamp's exact skill/chance, and the
full per-kind census.

**684 lib tests green**, `char_persistence` 7/7, `e2e_create` 1/1 (×2).

## Deliberate narrowings (`TODO(G21)` at the site)

- `removeEffectsOnExit` is parsed but not acted on — it needs zone-exit
  tracking, which is the same "characters inside" set the sweep avoids.
- The `ZoneKind::Effect` bit maps to Java's `ZoneId.ALTERED`; nothing reads it
  yet. `showDangerIcon` → `ZoneId.DANGER_AREA` + `EtcStatusUpdate` is not sent.
- Enabling/disabling a zone at runtime (siege scripts flipping
  `default_enabled=false` traps) has no caller yet.

## Next in G21

- `DamageZone` (35) and `SwampZone` (20) — both now cheap, since the parser and
  the sweep exist; both are mostly siege-gated.
- NPC pathfinding (the G7.85 worker for NPCs) and NPC regen.
- Wire `skillTargetReconsider` (faction data landed in slice 2).
- Fences (`FenceData`), `HtmCache`, walker routes, `CreatureSeeTaskManager`.
