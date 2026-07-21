# G23 slice 12 — Baium's threat table

The piece slice 11 deliberately left out, ported rather than approximated.

## Baium does not use the aggro list

He keeps a **top-3 threat table** (`c_quest0..2` / `i_quest0..2` on NPC
variables), fed by a weighting that shifts as he is worn down:

| condition | weight | a 300 hit scores |
|---|---|---:|
| melee (`skill == null`) | `damage × 1000` | 300 000 |
| below 25% HP | `(damage / 3) × 100` | 10 000 |
| below 50% HP | `damage × 20` | 6 000 |
| below 75% HP | `damage × 10` | 3 000 |
| otherwise | `(damage / 3) × 20` | 2 000 |

Two things fall out of that table, and both are the fight rather than trivia:

- **Melee threat is worth 150× an equal caster hit at full health.** Baium
  fixates on whoever is in melee range.
- **The caster weighting climbs fivefold as he weakens** — a caster beneath
  notice early becomes a real target below 25%.

Both are asserted as *relationships* (a ratio, an ordered progression across the
four bands), not as four independent magic numbers, so a mis-ported band shows
up as the wrong shape rather than one wrong constant.

## Two behaviours easy to flatten

Java's `refreshAiParams` is not "set the value":

- An attacker **already on the table** is raised only when its stored value is
  below `aggro + 1000`, and is then set to `damage + rnd(3000)` — so repeated
  small hits do not ratchet a threat upward indefinitely. Tested with a big hit
  followed by a small one.
- An attacker **not** on the table replaces the **weakest** slot, value and
  identity together — not the oldest, and not nobody. Tested with a fourth
  attacker arriving.

## Tests

`baium_tests` 4 → 8. The jitter (`getRandom(3000)`) is forced to 0 throughout,
so the ladder alone decides and the assertions can be exact.

## Still open in G23

`manageSkills` (Baium's skill selection off the table), Valakas's entry flow and
Antharas — the last two gated on `SpecialCamera`, which is now the concrete
blocker rather than a vague remainder.
