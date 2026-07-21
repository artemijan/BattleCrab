# G23 slice 7 — Orfen (and Zaken for free)

## Zaken needed no script

Zaken's 109 Java lines are **entirely** the spawn/respawn boilerplate slice 4
ported once: no `onAttack`, no `onSpawn`, no minions, nothing. Verified by
grep before writing anything (`addAttackId|addSpawnId|onAttack|onSpawn|minion`
→ 0 hits), so Zaken is already driven by the shared lifecycle and gets no module.

That is the shared-lifecycle slice paying for itself: one of the ten scripts
turned out to be zero work.

## Orfen's two mechanics

**The drag.** An attacker between **300 and 1000** units away has a 1-in-10
chance per hit of being teleported *onto* Orfen and paralysed. The band is the
mechanic: melee (inside 300) is never dragged, and past 1000 you are out of
reach. It punishes ranged damage specifically. Both edges tested, and the roll
is forced so the mechanic rather than the RNG is under test.

**The half-HP relocation.** The first time Orfen drops below half it clears
aggro and teleports home — **once per life**, not once per hit below the
threshold. The second half of that is what a naive port drops; tested by
shoving Orfen elsewhere and hitting it again.

Java's `if / else if` means the relocation wins when both could fire, and the
ordering matters: a boss that just relocated should not also drag someone to
where it no longer is.

## Riba Iren heals on **its own** wounds

The minion heals Orfen when **the minion itself** falls below half — not when
Orfen does. That is the opposite of every other healer in the game (Queen Ant's
nurses watch their target's health), so it is exactly the kind of thing a port
gets backwards by pattern-matching. Both directions tested: a wounded minion
heals a wounded Orfen, and a **healthy** minion ignores a nearly-dead Orfen.

## A vacuous test, caught and replaced

The first version of the Riba Iren test asserted
`get_component::<Vitals>(&ORFEN_OID).is_some()` — always true. Replacing it with
a real measurement (Orfen's HP rose) made it **fail**, which exposed a second
mistake: the fixture had given `ORFEN_HEAL` the *paralysis* effect list, so the
heal never healed.

One vacuous assertion was hiding a broken fixture. Worth stating plainly,
because both were mine and the test suite was green throughout.

## Tests

New `orfen_tests` (8): the drag inside the band, melee excluded, distant
excluded, the 1-in-10 roll, relocation at half health, no relocation above it,
no *second* relocation, and both directions of the minion heal.

## Still open in G23

Six scripts: Antharas (1056 lines), Baium (787), Valakas (581), Sailren (326),
DrChaos (321), plus boss barks.
