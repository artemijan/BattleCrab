# G29 slice 19 — the `SUMMON` target type

## Found by sweeping a bug class, not by looking for this

Slice 18 ended with a note: *"a field Java puts on `Creature` that the port put
on `Player` will quietly no-op for summons — there may be more."* Sweeping that
class led somewhere else entirely: **`TargetType::SUMMON` was never implemented
at all.**

Ranking target types by *learnable* skills:

| targetType | skills | **learnable** |
|---|---:|---:|
| SELF | 8053 | 289 |
| ENEMY | 2295 | 97 |
| TARGET | 1339 | 92 |
| ENEMY_ONLY | 766 | 79 |
| NONE | 80 | 26 |
| **SUMMON** | **52** | **18** |
| NPC_BODY | 17 | 5 |
| ENEMY_NOT | 34 | 4 |
| PC_BODY | 41 | 2 |

`SUMMON` outranks `NpcBody`, `EnemyNot` and `PcBody` **combined** — all three of
which the port already handled — and it fell through to
`TargetType::Other => INVALID_TARGET`.

## What was dead

The entire Summoner support kit:

```
1126 Servitor Recharge      1139 Servitor Magic Shield    1146 Mighty Servitor
1127 Servitor Heal          1140 Servitor Physical Shield 1299 Servitor Empowerment
1141 Servitor Haste         1144 Servitor Wind Walk       1300 Servitor Cure
1145 Servitor Magic Boost   1301 Servitor Blessing
1346 Warrior Servitor  1347 Wizard Servitor  1348 Assassin Servitor  1349 Final Servitor
1383/1384/1385 Mass Surrender to Fire/Water/Wind
```

Every one returned "Invalid target". A Summoner could summon a servitor and
then do nothing for it.

## A Java quirk kept as written

```java
if (creature.isPlayer() && creature.hasSummon()) return getAnyServitor();
return creature.getPet();
```

`getAnyServitor()` returns null when the player has only a **pet** — and
`hasSummon()` is true for a pet — so the `getPet()` fallback is unreachable for
players. A pet owner casting "Servitor Heal" targets nothing.

That reads like a bug, and it is thematically right: these are the Summoner's
servitor skills, and a Wolf is not a servitor. Ported as written, with a test
pinning it so a later "fix" has to be a deliberate divergence rather than an
accident.

## Tests

`servitor_tests` 92 → 97: a `SUMMON` skill heals the servitor, resolution finds
the caster's own servitor without it being selected, no summon means no target
(rather than silently falling back to the caster), and a pet is **not** a valid
summon target.

Since the fixture builds its own skill and so cannot catch a parse-arm mistake,
one test reads the **real** kit from the datapack and asserts skills 1126/1127/
1146/1349 parse as `Summon`-targeted.

## Still open

- The `Creature`-vs-`Player` sweep that started this is **not finished** — it
  found this by accident and should be run properly.
- `PET_EQUIP` paperdoll, pet spiritshots, evolution, reconnect resummon,
  servitor master-buff inheritance, `ServitorSkillUse`.
