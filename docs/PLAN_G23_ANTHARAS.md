# G23 slice 16 — Antharas's minion waves

The last boss script, opened. At 1056 lines Antharas is several slices; this is
the mechanic that defines the fight.

## Escalating pressure, hard-capped

Adds arrive every five minutes in **growing waves**: the multiplier starts at 1
and climbs on roughly 89% of waves (`getRandom(100) > 10`) to a ceiling of 4, so
waves run one pair → four pairs. The lair is capped at ~100 minions.

The spawn ladder is cap-aware and its steps are **not** interchangeable:

1. room for a full wave → `multiplier` pairs
2. else room for a pair (`count < 98`) → one pair
3. else room for one (`count < 99`) → a **single, randomly chosen** dragon
4. else nothing

**Step 3 is the one worth having.** At 98 minions Antharas adds one more of a
random type rather than skipping the wave, so the lair fills to exactly 99.
Collapsing the ladder to "spawn a pair if there is room for two" reads
equivalent, caps the fight two adds early, and would never be noticed. Its own
test.

## State is per-boss, not global

Java keeps `_minionCount` and `minionMultipler` as script statics. The port puts
them on the boss as a component: two Antharas instances sharing one counter is a
bug waiting to happen, and nothing about the Java relies on the sharing.

A deliberate, documented divergence rather than a transcription — worth flagging
because most of this milestone went the other way.

## Tests

New `antharas_tests` (6): the opening pair, growth to the cap of 4, a low roll
not growing the wave, a full wave giving way to a pair near the cap, the single
random dragon in the last slot, and a full lair spawning nothing while still
rearming — so the fight recovers as adds are killed.

## Still open for Antharas

The entry cinematic (7 camera shots), the Heart of Warding / portal-stone entry
gate, the 200-player cap, and `manageSkills`. Its combat pressure works.
