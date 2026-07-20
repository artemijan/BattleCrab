# G19 — HealPercent skill effect

## Why this slice

Next on the learnable-skill ranking after `ShieldDefence`, once `AttackTrait`
(7 learnable) was set aside — it needs a whole `TraitType` damage-bonus
system (attacker trait map + weakness/general-trait math in the combat
formulas) that doesn't exist on this port at all, a much bigger lift than a
single effect. `HealPercent` (5 learnable, 138 instances) is next, and it's
cheap: `AbstractEffect.instant()`, the same shape as the already-ported
`Heal`.

The five learnable instances are not obscure — they're core priest kit:
**Miracle (1426)**, **Benediction (1271)**, **Restore Life (1258)**,
**Revival (181)**, and **Touch of Life (341)**. Every one of them parsed to
an empty effect list before this slice (the XML name wasn't recognized), so
casting any of them consumed MP, played the animation, and healed exactly
nothing.

## What Java does

`HealPercent.instant`: `amount = power == 100.0 ? maxHp : maxHp * power /
100`, clamped against overheal, then either heals (`setCurrentHp` + SM
`S1_HP_HAS_BEEN_RESTORED`/`S2_HP_HAS_BEEN_RESTORED_BY_C1`) or — for a
negative `power` — damages instead (`reduceCurrentHp` +
`sendDamageMessage`). Two details it deliberately does **not** do that
`Heal.java` does: it never reads the recipient's `HEAL_EFFECT`/
`HEAL_EFFECT_ADD` stats, and it has its own `isHpBlocked()`/potion-bonus
branches `Heal` doesn't.

## What landed

- **`SkillEffect::HealPercent { power }`** (`model/skill.rs`) + the
  `"HealPercent"` parse arm (`data/skill_data.rs`).
- **A new match arm in `apply_skill_effects`**, structured like the existing
  `Heal` arm (same NPC-silent / player-with-SM split, same overheal clamp,
  same `S1_HP_HAS_BEEN_RESTORED`/`S2_HP_HAS_BEEN_RESTORED_BY_C1` messages) but
  computing the amount as a max-HP percentage instead of the magic-formula
  power, and *not* applying the recipient's `HealEffect`/`HealEffectAdd`
  modifiers — matching Java's real asymmetry rather than reusing `Heal`'s
  logic wholesale.
- **Negative-power branch** ported for parity (routes through the shared
  `apply_skill_damage`) even though none of the 5 learnable instances use it
  — a few unlearnable ones elsewhere in the datapack do (Larva Sting, various
  raid mechanics).
- **Not gated on `isHpBlocked()`**: Java skips healing while `DamageBlock`'s
  `BLOCK_HP` flag is up; that effect isn't ported on this build yet (it was
  set aside during the `ShieldDefence` slice too), so there's nothing to gate
  on. TODO(G19) left at the site.

## Test

`skills_tests::heal_percent_restores_a_share_of_max_hp` — real dist data
(skill 181 "Revival", self-target, power 100): a character at 20% HP casts it
and ends at full HP, with the self-cast `S1_HP_HAS_BEEN_RESTORED` message
sent.

Originally written against **Restore Life (1258)** healing a second player,
which surfaced an unrelated pre-existing gap: 1258's `targetType ENEMY_NOT`
isn't a modeled `TargetType` variant at all — it falls through to `Other`,
and `use_magic_on` silently no-ops on that (no packet, no cast, nothing).
`ENEMY_NOT` ("friendly/non-hostile target") appears on 34 skill instances (4
learnable, including 1258 itself) — a small but real, separate gap, noted
here rather than folded into this slice.

## Deferred (not this slice)

- `TargetType::EnemyNot` — blocks `HealPercent`'s own Restore Life plus an
  unknown number of other already-ported skills that share the target type.
- `AttackTrait` (7 learnable) — needs the `TraitType` system built first.
- `DamageBlock`'s `BLOCK_HP`/`BLOCK_MP` flags — `HealPercent` (and presumably
  `Heal`) should gate on `BLOCK_HP` once it lands.
