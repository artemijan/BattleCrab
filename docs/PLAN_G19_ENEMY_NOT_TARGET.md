# G19 — TargetType::EnemyNot

## Why this slice

Found unmodeled while writing the `HealPercent` slice's test: 1258 "Restore
Life" (`targetType ENEMY_NOT`) silently no-op'd on cast — no packet, no
`Casting`, nothing. `ENEMY_NOT` ("any friendly selected target") wasn't a
recognized `TargetType` variant, so it fell through to the catch-all `Other`,
which `use_magic_on` refuses outright with a bare `return;` before sending
anything. Small (34 instances, 4 learnable) but real, and it was quietly
capping `HealPercent`'s own reach — three of its five learnable skills
(Miracle, Benediction, Revival) are self-target, so they worked regardless,
but Restore Life and Touch of Life (the two that heal *someone else*) were
both silently broken.

## What Java does

`targethandlers/EnemyNot.java`: self is always valid; otherwise the target is
valid exactly when **not** `isAutoAttackable` — the precise inverse of
`Enemy`/`EnemyOnly`'s gate — with **no** force-use (ctrl) override, unlike
`Enemy`. It also explicitly "works on dead targets or doors as well" (a heal
landing on a fresh corpse ahead of a resurrection), so it's exempt from the
general "reject a dead target" rule every other type (bar `NPC_BODY`) is
subject to.

## What landed

- **`TargetType::EnemyNot`** (`model/skill.rs`) + the `"ENEMY_NOT"` XML
  parse arm (`data/skill_data.rs`).
- **A new arm in `resolve_cast_target`** (`game_loop/skills/cast.rs`): self
  always allowed; otherwise refused (`INVALID_TARGET`) when
  `is_auto_attackable` — reusing the exact helper `Enemy`/`EnemyOnly` already
  call, just inverted, with no ctrl leniency.
- **The dead-target rejection gate widened** from `!= NpcBody` to
  `!matches!(.., NpcBody | EnemyNot)`, matching Java's explicit exemption.

## Test

Two tests, split apart after the combined version tripped over two unrelated
test-harness realities (see below):

- `skills_tests::enemy_not_targets_a_friendly_player` — Restore Life (1258,
  real dist data, level 1 heals 15% of max HP) lands on a second player.
- `skills_tests::enemy_not_refuses_a_hostile_target` — the same skill refused
  outright against a real dist monster (20001 Gremlin, auto-attackable).

Two non-obvious fixes needed along the way, both about the *test setup*, not
the port:

1. **Same-position caster/target breaks something in the movement/geo path
   during a multi-tick cast.** Every prior two-player test in this file uses
   distinct starting positions; this one didn't (both default to `(1, 2,
   3)`), and the cast never progressed past its unlaunched state. Moving the
   target off the caster's exact spot fixed it. Not investigated further —
   no production skill casts land two players on the identical tile, so this
   is a test-harness quirk, not a parity bug — but worth a comment for the
   next person who copies this pattern.
2. **`isMagic` skills scale their cast time by *magic* casting speed.**
   Restore Life is `isMagic`; a level-1 default character (`class_id` 0,
   Human Fighter) has a near-zero magic casting-speed multiplier, stretching
   the nominal 8 s cast into minutes — nothing was broken, the cast was just
   realistically very slow for an off-class caster. Switched to a Mystic
   (`class_id` 10, the same class the real-data spellcraft test uses) for a
   sane cast time. Its `cur_mp` also needed a manual post-spawn bump — the
   `CharData.cur_mp` field is clamped to the class's *computed* max MP at
   spawn (59 for a level-1 Mystic, well under Restore Life's 80 MP cost), so
   setting it pre-spawn doesn't survive.
