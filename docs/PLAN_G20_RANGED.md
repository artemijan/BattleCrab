# G20 — Ranged attacks: bows, crossbows and ammunition

The first **G20** slice. G20's gate is *"a bow attack consumes an arrow, a
polearm hits a line, PvP flagging drives auto-attack, a physical skill lands"* —
this takes the first clause.

Java sources: `Creature.doAttack`'s `WeaponType.BOW`/`CROSSBOW` branches,
`Player.checkAndEquipAmmunition`, `Inventory.findArrowForBow` /
`findBoltForCrossBow` / `reduceArrowCount`, `Formulas.calculateReuseTime`,
`SetupGauge`.

---

## 1. Where G20 actually stood

Several of G20's listed items turned out to be done already, so the survey
mattered more than the list:

- **`PhysicalAttack`-type skills** landed with the instant-damage effect work.
- **The rest of `isMovementDisabled` (root/immobilize)** landed in G19's CC
  slice.
- **PvP flagging** has its Phase 1 (flag state, durations, skill/melee
  flagging); the melee-PvP-attack half is still open.

What was genuinely untouched: **ranged weapons**. Bows equipped and swung, but
as plain melee — no ammunition, no MP cost, no reload delay. Their *range*
already worked, since bows declare `pAtkRange` 500 and the item-stat path has
fed `CombatStats.atk_range` since G14.

## 2. What a ranged swing adds

`game_loop/ranged.rs` implements Java's pre-shot gate, called from
`combat::do_auto_attack` before the ordinary swing:

1. **Reload delay** — `_disableRangedAttackEndTime`, ported as a `RangedReload`
   component holding the tick the next shot is allowed. A shot inside it is
   refused with a bare `ActionFailed`, as in Java.
2. **Ammunition** — arrows for bows, bolts for crossbows, matched to the
   weapon's **crystal grade** and auto-equipped into the left hand. No matching
   stack → the attack intention is dropped and "You have run out of arrows"
   goes out.
3. **MP** — `weapon.mp_consume` per shot (Short Bow 13 spends 1), with Java's
   `reducedMpConsume` roll for weapons that declare one. Too little MP refuses
   the shot without spending an arrow.
4. **Firing** — one arrow consumed, `SetupGauge(RED, reuse)` shown, and the
   reload armed at `900000 / pAtkSpd` (`Formulas.calculateReuseTime`).
   Crossbows additionally send "Your crossbow is preparing to fire".

Only *players* run the ammunition/MP half, matching Java, where that whole block
is `isPlayer()`-gated — an NPC archer shoots freely.

## 3. Ammunition can't use the ordinary equip path

`Inventory::equip_ammunition` is new, and deliberately bypasses `equip_item`,
whose two rules are both wrong here:

- it **refuses `Etc` items** outright — and arrows are `Etc`, so they would
  never equip at all; and
- its `SLOT_L_HAND` branch **displaces a two-handed weapon** — which would
  unequip the very bow the arrows are for.

Java sidesteps the same problem the same way: `checkAndEquipAmmunition` calls
`setPaperdollItem(PAPERDOLL_LHAND, arrows)` directly rather than going through
`equipItem`. A test asserts the arrows end up in the left hand *and* the bow
stays in the right.

## 4. Grade matching

Java's `findArrowForBow` compares `getCrystalTypePlus()`, which collapses the
S-grades. Nothing above S exists on an Interlude dist, so plain grade equality
is the same predicate — noted at the site. A B-grade arrow is not picked up for
a no-grade bow, which is tested (it refuses as if there were no ammunition at
all, exactly as Java does).

## 5. Tests

Parse assertions against the real datapack: Short Bow 13 (`mp_consume` 1,
no-grade), Wooden Arrow 17 (`ARROW`, no-grade so it matches), and a melee weapon
spending no MP.

Behaviour (`game_loop/tests/ranged_tests.rs`, 6 cases): the gate line — a shot
auto-equips an arrow, spends one and costs MP; the reload delay blocking the
next shot and releasing after it; running out of arrows cancelling the attack
with its system message; wrong-grade ammunition being ignored; too little MP
refusing without spending an arrow; and a melee swing arming no reload at all.

> Worth recording: the branch was first inserted against an anchor that also
> appears in `do_door_swing`, so it silently landed in the *door*-attack path
> and every ranged test failed while the suite stayed green. When adding to a
> large function, anchor on something unique to it.

## 6. What G20 still owes

From the gate: **polearm sweep** (a line/arc of targets), **PvP auto-attack**
(the melee half of the flag consumers), and **dual-weapon split hits**. Then
**overhit** XP, the `SHOTS_BONUS` dynamic value, and — from the 2026-07 audit —
**duels** (`DuelManager`, whose shape G25's olympiad reuses).

Ranged-specific leftovers: the bow's peace-zone check (Java runs one only for
player-bow attacks), `CHEAPSHOT` zeroing the MP cost (no ported source), and
NPC-archer reuse timing (Java arms the timer for NPCs too, outside the
`isPlayer()` block).
