# G23 slice 19 — Antharas's skill ladder, and the caller neither boss had

## A decision procedure with no caller

`baium::manage_skills` was written, documented and tested in slice 12. Nothing
in the crate ever called it. Baium chose skills into the void and only ever
swung — for seven slices, while the plan doc said *"Baium is complete."*

This is the [[l2r-regen-stat-pipeline]] shape one level up. That slice found
stats that were *pumped but never read*; this is a whole procedure that is
correct, covered, and unreachable. **The chooser being well-tested is what hid
it**: nothing about it looked unfinished, and a unit test calls the function
directly, so the tests passed exactly as they would have if it were wired.

The check that finds this is not a test — it is `cargo build`'s dead-code
warning, and the habit of reading the entry point rather than the unit.

Both bosses are now driven from `onAttack`, which is where Java calls
`manageSkills` (plus `onSpellFinished` and a 60s idle timer, both still open).

## The threat table was duplicated

Antharas's `refreshAiParams` and weighting ladder are **identical to Baium's,
line for line** — same ×1000 melee weight, same four HP bands, same
`rnd(3000)` jitter, same 9000-unit prune, same 70%-decay-to-500 rotation. They
were ported separately, six slices apart, and the duplication only became
visible when the second one arrived.

Extracted to `boss_threat.rs`. Each boss now keeps only its own skill ladder.

## The tail sweep's angle is absolute, not relative

Java gates the tail sweep and the curse on `npc.calculateDirectionTo(c2)` —
plain `atan2(dy, dx)`, the direction in **world coordinates**. Antharas's
`heading` never enters the comparison. So "within 8° of 180°" does not mean
*behind him*; it means the target is due **west**, whichever way he is turned.

The windows are plainly shaped like a rear arc, and every other cone check in
the codebase (`Creature.isBehind`, `Formulas.calcCastBreak`) subtracts
`convertHeadingToDegree(getHeading())` first. This one lost its heading term.

**Ported exactly as written.** The datapack is the specification, and
"correcting" it would change how often the tail lands — a real behaviour change
dressed up as a fix. A test puts the target due west while Antharas faces east
and asserts the sweep still lands, so the next reader can see it is deliberate.

## The ladder is a chain, not a table

Each rung is `else if getRandom(100) < N`, reached only when every rung above
it has already failed — so the printed percentages are conditional, not
marginal. A `getRandomBoolean()` two-thirds of the way down means everything
below it is reached less than half the time it is eligible.

Four bands, and the repertoire opens as he is worn down:

| band | gains |
|---|---|
| above 75% | tail, meteor, breath, two ordinary attacks |
| below 75% | + the stomp |
| below 50% | + the curse |
| below 25% | + the Breath Attack, **rolled first, at 30%** |

The Breath Attack is the only skill that *opens* a band rather than being
appended to it — below a quarter health it is considered before distance or
angle is looked at.

`castOnTarget == false` means the tail, curse and stomp are cast **with
Antharas as their own target**: they are areas centred on him, not on the
player who drew them. Dropping that would silently make each a single-target
hit, so it is a field on the returned `Choice` and asserted for all three.

## A test that changed because the order is real

Two Baium weighting tests began failing once the hook was wired: choosing a
target knocks the top threat down to 500 seven times out of ten, *immediately*,
because Java runs `refreshAiParams` then `manageSkills` in one breath. The
tests were right about the weighting and wrong about where to observe it — they
now read the table where it is written rather than after the boss has acted.

## Tests

`antharas_tests` 19 → 27, `baium_tests` 14 → 15. The two hook tests were
checked against the previous commit and fail there, which is the only evidence
that the missing caller is actually restored.

## Still open for Antharas

The entry gate from slice 18 has the same problem this slice fixed: `try_enter`
is complete and tested, and **nothing calls it** — it needs the Heart of
Warding's html and bypass verbs, which is its own slice. `cargo build` names
its four constants as unused; that warning is the open item, not noise.
