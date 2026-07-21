# G23 slice 17 — Antharas's entry cinematic

## Antharas chains; Valakas batches

The obvious move after slice 15 was to reuse the Valakas cinematic table. That
would have been wrong.

**Valakas** arms all ten beats up front from the start of the sequence.
**Antharas** has each beat schedule the next with a *relative* delay, and one
beat forks a second, independent timer. The two scripts genuinely differ, and
reshaping Antharas into the Valakas table would have silently changed its timing
model.

Ported as a chain. A test asserts exactly **one** cinematic timer is pending
after the sequence starts — which is what distinguishes a chain from a batch,
and would fail immediately if someone "unified" the two.

## The beat that forks

`CAMERA_3` roars, schedules `CAMERA_4` at +200 ms, **and** schedules a second
social action at +5200 ms. It is the only beat that arms two timers, so a
uniform "each beat arms the next" port drops the second roar entirely. Its own
test asserts two timers, and another that the forked social fires on its own.

## The tail starts the fight

`START_MOVE` hands Antharas his AI back, walks him into the lair, and — in the
port — **starts the minion waves**. Slice 16 had them starting at spawn; they
now start when the fight actually begins, so a boss standing un-engaged in its
lair is not already producing adds. Both directions tested.

## A vacuous assertion caught before merge

The forked-social test first read
`assert!(drain(&mut rx, 0x2D) > 0 || true, …)` — the `|| true` makes it pass
unconditionally, and the opcode was wrong besides. Replaced with an exact count
against the real opcode (`SocialAction` = 0x27), which also gained the
independent-fire test.

Caught on review rather than by the suite, which is the point: a passing suite
says nothing about assertions that cannot fail.

## Tests

`antharas_tests` 6 → 12.

## Still open for Antharas

The Heart of Warding / portal-stone entry gate, the 200-player cap, and
`manageSkills`.
