# G29 slice 22 — a summon's kill credits its owner

The `getActingPlayer()` audit slice 21 opened, applied to the reward path. This
was the biggest gap of the three found so far.

## The gap

Java `Attackable.calculateRewards`:

```java
final Player attacker = info.getAttacker().getActingPlayer();
```

Every damage dealer is resolved to the player behind it, so a **summon's damage
counts for its owner**. The port keyed the aggro list by the dealer's own object
id and never resolved it — so a summon's damage belonged to nobody.

The result: **a summoner whose pet did the fighting earned nothing.** No exp, no
drops, no quest kill credit. That is the core summoner loop, and it was
completely broken.

## Fixed at the resolution point, not the call sites

Three places needed it, all resolved through the same `pvp::acting_player`:

- the damage-share loop (exp/sp),
- the looter fallback when nobody out-damaged the killer (drops),
- `quests::notify_kill` (kill credit).

Range is measured from the **earner**, as Java does — a pet fighting outside its
owner's reward range earns them nothing.

## The double-count that resolution creates

Once both resolve to the same player, an owner who fought *alongside* their
summon appears **twice** in the aggro list. Naively pushing both would inflate
their slice of a contested kill. Shares now merge per resolved player, and a
test pins it: owner 100 + summon 100 against a rival's 200 must yield **equal**
exp.

This is a bug the fix introduces, not one it inherits — worth stating, because
it would only show up in contested kills, which are exactly the case nobody
tests by hand.

## Three vacuous-failure modes in one test

The probe test needed three corrections before it measured anything:

1. Calling `npc_do_die` with no damage history — rewards are shares of recorded
   damage, so nobody earns (true for players too).
2. Swinging once to create that history — a real swing lands on a **scheduled**
   tick, so nothing was recorded within the test. Damage is now seeded directly:
   this test is about *who damage is credited to*, not attack timing.
3. `default_template` awards **0 exp**, so the assertion read 0 either way.

Then the fix was disabled and the test confirmed to fail before being kept.
Four separate ways for this test to pass while proving nothing — cf.
`l2r-fixture-hides-testcase` and `l2r-verify-test-detects-bug`.

## Tests

`servitor_tests` 103 → 105; death-drop/quests/party/social/combat groups re-run
clean (7/113/19/23/39).

## Still open

- The `getActingPlayer()` audit is **not** exhausted: karma/PK counting on
  player kills and duel damage attribution are unchecked.
- Sweep remainder: `Reuses`, `TargetRef`.
- `PET_EQUIP` paperdoll, pet spiritshots, evolution, reconnect resummon,
  servitor master-buff inheritance, `ServitorSkillUse`.
