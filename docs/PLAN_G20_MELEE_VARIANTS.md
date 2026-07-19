# G20 — Multi-hit melee: dual weapons and the polearm sweep

The second **G20** slice, completing the `doAttack` variant family started by
[PLAN_G20_RANGED.md](PLAN_G20_RANGED.md). Java dispatches a swing to one of
four shapes — simple, bow, pole, dual — and this takes the last two.

Java sources: `Creature.generateAttackTargetData` / `generateHit`,
`Weapon.getBaseAttackRadius`/`getBaseAttackAngle`,
`handlers/effecthandlers/HitNumber.java`, `serverpackets/Attack`.

---

## 1. A wrong first read, corrected

The first pass at this concluded the polearm sweep was **dead on this dist**,
because `CreatureStat.getPhysicalAttackAngle()` returns `0` (an angle of 0
selects nothing) and a scan for `HitNumber` found 23 skills, none learnable.

Both halves of that were wrong:

- `PlayableStat` **overrides** radius and angle to read from the *weapon*, whose
  values come from `<set name="damage_range" val="0;0;radius;angle"/>`. A
  polearm is `0;0;66;120`, a sword `0;0;40;120`. The `40/0` in `CreatureStat` is
  only the no-weapon fallback.
- The `HitNumber` scan missed **Polearm Mastery 216** — a perfectly ordinary
  learnable skill — because the regex demanded a self-closing effect tag.

So the sweep is live, and the lesson is the same one the `StatUp` false lead
taught: verify a "this is dead" conclusion as carefully as a "this is live" one,
because a bad regex reads exactly like an absent feature.

## 2. Dual weapons

`DUAL`, `DUALBLUNT`, `DUALDAGGER` and `DUALFIST` (1 571 items on this dist)
generate **two hits on the main target**, each at half damage — Java's
`halfDamage` flag, applied as a plain `damage /= 2` after the full roll.

> **Deviation:** Java rolls the second hit independently (its own miss, crit and
> shield rolls). This port reuses the first roll's outcome for both halves, so a
> dual swing is two halves of one roll rather than two rolls. Marked
> `TODO(G20)` at the site; factoring the roll out of `do_auto_attack` is the
> prerequisite.

## 3. The polearm sweep

Gated on **`ATTACK_COUNT_MAX > 1`** — a *stat*, not the weapon type. Nothing
about holding a polearm sweeps by itself; **Polearm Mastery 216** sets
`HitNumber` to 5, which is what turns the weapon into a multi-target one. That
distinction is worth keeping in mind: the natural assumption ("polearm ⇒ sweep")
is wrong, and both cases are tested.

Extra targets must be alive, auto-attackable, inside the **weapon's** attack
radius (66 for a polearm, 40 for most others) and within its attack angle of the
attacker's heading (120° both ways). Each gets a *simple* hit — no halving, even
for a dual — capped by the remaining count.

Java additionally skips the sweep when `PHYSICAL_POLEARM_TARGET_SINGLE > 0`; no
ported effect sets that stat, so the check is omitted (`TODO(G20)`).

## 4. The Attack packet carries several hits

`serverpackets/Attack` was already shaped for this — first hit inline, then
`writeShort(size - 1)` and a block each — but the port hard-coded the count to
`0` ("no additional hits"). It now takes `&[AttackHit]`, and every hit is
scheduled as its own `AttackHit` task so each target takes its damage through
the normal victim-side path (CP soak, hate, AI wake, death).

## 5. Tests

Parse assertions against real data: polearm 15 is `66/120`, sword 1 is `40/120`,
and Polearm Mastery 216 carries `ATTACK_COUNT_MAX +5`.

Behaviour (`game_loop/tests/melee_variants_tests.rs`, 6 cases), asserted by
decoding the Attack packet's hit list: a dual weapon landing two equal half
hits on one target; a single weapon landing one; a polearm **without** mastery
landing one; a polearm **with** mastery sweeping a neighbour in the arc while
sparing a distant mob; the sweep capped by `ATTACK_COUNT_MAX`; and a mob 180°
behind the attacker being outside the 120° arc.

## 6. What G20 still owes

The gate's remaining clause is **PvP auto-attack** (the melee half of the flag
consumers — the flag state itself landed in the PvP slice). Then **overhit** XP,
the `SHOTS_BONUS` dynamic value, and **duels** (`DuelManager`, whose shape G25's
olympiad reuses).

Ranged leftovers from the previous slice are unchanged: the bow's peace-zone
check, `CHEAPSHOT`, and NPC-archer reuse timing.
