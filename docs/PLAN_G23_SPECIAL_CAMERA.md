# G23 slice 14 — `SpecialCamera`

The concrete blocker named at the end of slice 11: Valakas's entry flow uses it
19 times and Antharas 7, and the packet did not exist.

## `range` is accepted and never written

Java's canonical constructor takes twelve parameters, assigns eleven, and
**never assigns `range`**. The wire carries eleven ints.

The port keeps `range` in the signature (as `_range`) so a call site can
transcribe the Java argument list literally. Dropping the parameter would be
tidier and would make every following argument shift by one at every call site —
26 of them, each a long list of unlabelled integers, which is the worst possible
place for a silent off-by-one.

The test asserts the packet is **eleven** ints and that the field after `time`
is `duration` rather than `range` — the specific corruption a "helpful"
serialisation would cause, and one that would desync the whole cinematic by four
bytes.

## The overload that swaps two arguments

Java also ships an 11-arg overload which forwards `(time, duration, range)` into
the canonical `(time, range, duration)` slots — so a caller's **range is written
as the duration**. It looks like a straightforward bug.

It is not reproduced, because **no boss script uses it**: all 26 call sites
across Valakas and Antharas take the 12-arg form. Checked rather than assumed,
and recorded so the absence is deliberate — the same call as Core's
19-spawns-that-are-3, but landing the other way.

## Tests

New `special_camera_tests` (2): the wire shape with `range` absent, and
Valakas's opening shot transcribed from the script and checked field by field.

## What this unblocks

Valakas's entry flow and Antharas are now portable. Both are long cinematic
sequences (a chain of timed camera shots with spawn/roar/teleport steps between
them) rather than combat logic, so each is its own slice.
