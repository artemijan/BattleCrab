# G23 slice 15 — Valakas's entry cinematic

The first thing `SpecialCamera` unblocked.

## Ten beats, scheduled up front

Java arms every step from the **start** of the sequence
(`startQuestTimer("spawn_N", 1700 / 3200 / 6500 / …)`), not as a chain where
each beat schedules the next. The port does the same, and that is deliberate:

**the beats are not evenly spaced.** 330 ms separates steps 5 and 6; 6.7 s
separates 8 and 9. A relative chain would be easy to get subtly wrong and the
error would be invisible except as a cinematic that felt off. A test pins the
26-second end-to-end span and that the beats occupy distinct ticks.

The tenth beat carries **no camera** — it flips the status to `FIGHTING`, which
is what actually starts the fight and locks entry behind it.

## The camera table is transcribed, not re-derived

Each beat stores the eleven camera arguments in Java's own order, `range`
included even though the wire drops it. Keeping the tables shaped like the
source is precisely why slice 14 kept that parameter: the two can be diffed by
eye.

## It plays for the lair, not the neighbourhood

`BOSS_ZONE.broadcastPacket` — a player inside the lair sees the cinematic, one
standing outside sees nothing, which is why Java broadcasts on the *zone* rather
than the boss's region. Tested with one player in and one out; using the
ordinary region broadcast would have passed a weaker test and shown the
cinematic to bystanders.

Valakas is also teleported into the lair before the first shot, since the camera
is framed on him.

## Tests

`valakas_tests` 5 → 10, plus a `pending_ticks_for_test` hook on the scheduler so
a scheduled *sequence's shape* can be asserted rather than its individual
entries.

## Still open in G23

Antharas (1056 lines, 7 camera uses) — the last boss script.
