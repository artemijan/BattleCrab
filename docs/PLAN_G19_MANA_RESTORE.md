# G19 — MP restoration family

## Why this slice

The unconsumed-stat sweep came back clean again, and the name ranking left a
**five-way tie at 3 learnable** (`TriggerSkillByAttack`, `ReflectSkill`,
`BlockMove`, `ManaHealByLevel`, `TwoHandedBluntBonus`, `Confuse`). Rather than
take one arbitrarily, `ManaHealByLevel` was picked because it anchors a
*cluster*: four Java handlers that share one gate, one clamp and one message
pair, differing only in how they compute the amount.

| effect | learnable | skills |
|---|---|---|
| `ManaHealByLevel` | 3 | Recharge 1013, Servitor Recharge 1126, Mass Recharge 1428 |
| `Mp` | 2 | Pain of Sagittarius 417, Body To Mind 1157 |
| `ManaHeal` | 1 | Mortal Strike 410 |
| `ManaCharge` | 1 | Higher Mana Gain 285 (the stat the above read) |
| `ManaHealPercent` | 0 | 46 item/potion skills |

**7 learnable skills** — more than any single tied entry — and it closes the
`TODO(G19)` the `MagicalAttackMp` slice left behind on `isMpBlocked`.

**Recharge 1013, Servitor Recharge 1126 and Mass Recharge 1428 each carry only
`ManaHealByLevel`**, so all three parsed to an empty effect list and were
dropped whole. The core mage-support skill in the game restored nothing.

`ManaCharge` was found by applying the both-tree grep lesson from the last
slice: `Stat.MANA_CHARGE` looks unused if you only grep `java/`, but
`dist/game/data/scripts/handlers/effecthandlers/ManaCharge.java` grants it, and
a **learnable** skill (Higher Mana Gain 285) uses it. Without porting it the
recharge skills would have read a stat with no source.

## What Java does

All four handlers are instant and share a tail:

```java
if (effected.isDead() || effected.isDoor() || effected.isMpBlocked()) return;
...
amount = Math.max(Math.min(amount, effected.getMaxRecoverableMp() - effected.getCurrentMp()), 0);
if (amount != 0) { effected.setCurrentMp(...); effected.broadcastStatusUpdate(effector); }
sm = (effector != effected) ? S2_MP_HAS_BEEN_RESTORED_BY_C1 : S1_MP_HAS_BEEN_RESTORED;
```

The amounts differ:

- `ManaHeal` — flat `power`, then `getValue(MANA_CHARGE, amount)`
  (`mul * amount + add`, so Higher Mana Gain's `DIFF` grant is a flat bonus).
- `ManaHealByLevel` — the same, **then** a level-gap penalty: unpenalised to a
  5-level gap, then an `else if` ladder from `levelDiff == 6` (×0.9) down to
  `== 14` (×0.1), and `>= 15` → **0**. That collapses to
  `1 - (diff - 5)/10` over the ladder's range, so the port does the arithmetic
  rather than nine branches.
- `ManaHealPercent` — `maxMp * power / 100`.
- `Mp` — `amount` flat, or `maxMp * amount / 100` in `PER` mode. Note it reads
  `<amount>`, not `<power>`, unlike its three siblings.

Two things checked rather than assumed:

- **`MANA_CHARGE` is live** (see above) — ported as a plain `EFFECT_REGISTRY`
  entry.
- **`MAX_RECOVERABLE_MP` is not.** The `LimitMp` handler exists but **no skill
  in this dist grants it**, so `getMaxRecoverableMp()` is plain `maxMp` here.
  Documented at the clamp rather than plumbed.

## What landed

- Four `SkillEffect` variants (`ManaHeal`, `ManaHealByLevel`,
  `ManaHealPercent`, `MpRestore`) + parse arms, and `Stat::ManaCharge` via the
  generic registry.
- **`restore_mp`** — the shared tail: dead/`isMpBlocked` gate, overheal clamp,
  write, `broadcast_vitals`, and the self-vs-other message
  (`S1_MP_HAS_BEEN_RESTORED` 1067 / `S2_MP_HAS_BEEN_RESTORED_BY_C1` 1068, both
  new).
- **`recharge_level_penalty`** and `mana_charge_of` as separate testable
  functions.
- The `MP_BLOCK` doc comment updated: the flag now has real consumers on both
  the drain and restore sides, and the `TODO(G19)` is closed.

## Tests

`game_loop::tests::mana_restore_tests` (10):

- `recharge_level_penalty_matches_javas_ladder` — every one of Java's nine
  branches plus both boundaries, against the arithmetic that replaced them.
- `a_high_level_target_is_recharged_less` — the same end to end. **This one
  failed first**: the level-5 fixture's ~50 max MP meant the overheal clamp
  capped both halves of the comparison at max and they read equal. A `roomy_mp`
  helper (and a comment explaining why) fixes it — a reminder that a clamp
  downstream of what you're measuring will happily hide it.
- `mp_block_refuses_a_restore` — the gate this slice closes the TODO on.
  Without it MP-block would stop drains but not heals, exactly backwards.
- `mana_charge_adds_to_the_recharged_amount` — including a negative case
  proving the bonus is read off the **recipient**, not the caster.
- `the_recharge_skills_carry_only_the_restore_effect` — pins the single-effect
  shape that made all three drop whole.
- `higher_mana_gain_grants_the_mana_charge_stat` — the passive path, real data.

## Deferred (not this slice)

- **`FACEOFF`** in all four gates — an unmodeled flag.
- **`ADDITIONAL_POTION_MP`** (the potion/elixir bonus) — needs the item context
  threaded into effect application, which no effect on this port has yet.
- **`MAX_RECOVERABLE_MP`** — no skill grants it on this dist.
- The rest of the tied-at-3 cluster: `TriggerSkillByAttack`, `ReflectSkill`,
  `BlockMove`, `TwoHandedBluntBonus`, `Confuse`.
