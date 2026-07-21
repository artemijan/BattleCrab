# G23 slice 18 — Antharas's entry gate

The Heart of Warding's `"enter"` ladder: who gets into the lair, and why not.

## The ladder's order is the user experience

```
DEAD                          → "Antharas is dead"
IN_FIGHT                      → "the fight has started"
lair full (>= 200)            → "no room"
in a group and not the leader → "only your leader may enter"
no Portal Stone               → "you need a stone"
group larger than the room    → "no room"
otherwise                     → admitted
```

The boss's state is checked **before** the ticket, so a player without a stone
arriving at a dead Antharas is told *"Antharas is dead"* rather than *"you need
a stone"* — the reason that would actually help them. Tested with no stone in
inventory, so a reordering shows up as the wrong message.

## Two rungs easy to lose

- **Only the leader may bring a group in**, and for a command channel it is the
  *channel* leader — a party leader inside a CC is not a leader for this. A
  member who talks to the Heart is refused rather than quietly entering alone.
- **The whole group must fit.** `members > MAX_PEOPLE - inside` refuses
  outright rather than admitting as many as will fit, so a raid is never split
  in half by the doorway.

Only members **gathered at the Heart** (within 1000 units) come along, so a
straggler is left behind rather than teleported from across the map.

## A branch no test could reach

The first draft's overfill test admitted in its own comment that it asserted
"the leader alone still fits" — filling a 200-player lair in a unit test is
impractical, so the rung it was named for was unreachable.

Rather than ship a test that documents its own gap, the ladder was split so
occupancy is a parameter (`try_enter_with_occupancy`). The rung is now tested
from both sides: 199 inside with a party of two refuses; 198 admits.

**A branch no test can reach is a branch nothing checks** — and a test named
after it is worse than none, because it reads as coverage.

## Tests

`antharas_tests` 12 → 19.

## Still open for Antharas

`manageSkills`. Its waves, cinematic and entry gate are done.
