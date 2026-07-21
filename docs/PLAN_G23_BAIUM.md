# G23 slice 11 — Baium (archangels + strider debuff)

## Chosen because it has no cinematics

Valakas's entry flow was next on the list, but it is **19 `SpecialCamera`
calls** — the camera packet isn't ported, so most of that slice would be stubs.
Counting them redirected the work:

| script | `SpecialCamera` uses |
|---|---|
| Valakas | 19 |
| Antharas | 7 |
| **Baium** | **0** |

Baium is the only one of the three great bosses with no cinematics at all, so it
is portable *now* rather than after the camera work. One grep changed which
slice was worth doing.

## What landed

- **Five archangels**, at fixed points with headings. Not in a minion table —
  the script places them, so nothing else would.
- **The anti-strider debuff** (4258 "Hinder Strider"), cast **once**: Java
  guards on `!isAffectedBySkill(4258)`, so it is not recast every swing. Tested
  by draining the client channel and asserting a second hit starts no new cast.

## What is deliberately not here

Baium's targeting is a **top-3 threat table** on NPC variables
(`c_quest0..2`/`i_quest0..2`), fed by a weighting that shifts as he is worn
down:

| condition | weight |
|---|---|
| melee (`skill == null`) | `damage × 1000` |
| below 25% HP | `(damage / 3) × 100` |
| below 50% HP | `damage × 20` |
| below 75% HP | `damage × 10` |
| otherwise | `(damage / 3) × 20` |

Melee threat is worth **fifty times** a caster's at full health, and the caster
weighting swings by a factor of ten across the bands. Folding that into the
port's ordinary aggro list would look like it worked and would not be Baium, so
it is left for its own slice rather than approximated — with the table written
down so the next slice starts from the numbers rather than re-deriving them.

## Tests

New `baium_tests` (4): five archangels, a strider rider hindered, an unmounted
attacker left alone, and no recast while the debuff holds.

## Still open in G23

Baium's threat table; Valakas's entry flow and Antharas (both gated on
`SpecialCamera`).
