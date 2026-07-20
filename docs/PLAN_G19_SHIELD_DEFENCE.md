# G19 — ShieldDefence / ShieldDefenceRate skill effects

## Why this slice

Next on the learnable-skill ranking after `MpConsumePerLevel`, once
`EnergyAttack` (9 learnable) was set aside: it depends on the Dwarf "Force/
Charges" resource, which isn't modeled anywhere on this port yet (no
`Player.charges` field, no gain/consume mechanic) — a bigger, separate lift
than a single effect, deferred rather than scope-crept into this slice.
`ShieldDefence` (8 learnable skills, 56 instances) is next, and unlike
`EnergyAttack` it's cheap: a single-stat `AbstractStatEffect`
(`Stat.SHIELD_DEFENCE`), the same shape as a dozen already-ported effects.

The headline skill is **Shield Mastery (153)** — a passive every shield-using
class can learn, `autoLearn` for several of them — so this wasn't an obscure
gap. It carries three effects: `PhysicalDefence` (armor-conditioned, already
worked) and `ShieldDefenceRate` + `ShieldDefence`. `ShieldDefenceRate` turned
out to have been *parsed* since an earlier slice (it's in `EFFECT_REGISTRY`,
landing the buff/icon correctly) but never actually **read** anywhere —
`game_loop::combat::shield_stats` computed the shield block rate straight off
the equipped item's raw `rShld`, bypassing `StatModifiers` entirely.
`ShieldDefence` wasn't even parsed. Together: every shield-using character's
actual block chance and block-defence bonus were flatly wrong the moment they
learned the class's own core shield passive.

## What Java does

Both are plain `Stat.defaultValue` finalizers (`base * mul + add`) over
`calcWeaponPlusBaseValue` — the equipped shield's own `sDef`/`rShld` (the only
item contribution to either stat) plus every buff/passive's `PER`/`DIFF`
modifier, exactly the same shape `PhysicalDefence` already uses. The CON-bonus
multiply on the rate happens *after*, inside `Formulas.calcShldUse` itself —
never baked into the stat.

Critically, `calcShldUse` bails on `!(secondaryWeaponItem instanceof Armor)`
**before** ever reading either stat — so a flat `add` from a buff like
Residence Shield Defense (603, +225 DIFF) contributes nothing without an
actual shield equipped. The port's existing `shield_stats` already had this
early-return shape (no shield → `(0.0, 0.0, con_bonus)`); the fold had to
preserve it rather than applying `finalize` unconditionally.

## What landed

- **`Stat::ShieldDefence`** (`model/stats.rs`) alongside the existing
  `ShieldDefenceRate` + the `"ShieldDefence"` `EFFECT_REGISTRY` entry
  (`data/skill_data.rs`) — the same single-stat pattern as `PhysicalDefence`.
- **`model::finalize` bumped to `pub(crate)`** so `game_loop::combat::
  shield_stats` can reuse the exact `base * mul + add` Java uses, rather than
  duplicating it. Shield stats live outside the `recalculate_stats`/
  `CombatStats` pass (they're computed fresh at combat-lookup time, not
  cached), so this is the one place both stats needed folding.
- **`shield_stats` now finalizes both stats** over the shield's own
  `sDef`/`rShld`, gated behind the existing "no shield equipped" early return
  (preserving the `calcShldUse` short-circuit) — then applies the CON-bonus
  multiply to the rate exactly as before.

## Test

`skills_tests::shield_mastery_passive_raises_shield_block_stats` — real dist
data (skill 153 "Shield Mastery" level 4, item 628 "Hoplon" — `sDef` 128,
`rShld` 20): a bare character reads the shield's raw stats unchanged; one with
Shield Mastery reads `sDef × 1.6` and `rShld × 2.0` (the level-4 `PER`
amounts), checked via `game_loop::combat::combatant`'s finalized fields
(`pub(crate)`, so directly reachable from tests) rather than the cast
pipeline, since passives fold into `StatModifiers` at `Player::from_char`.

## Deferred (not this slice)

- `EnergyAttack` (Dwarf Force/Charges resource) — needs the charges mechanic
  modeled first.
- The `AttackTrait` effect (7 learnable, 260 instances) is next on the
  ranking; a heavier lift (per-trait damage-bonus table against monster
  traits), not started here.
