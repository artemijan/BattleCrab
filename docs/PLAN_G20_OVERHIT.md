# G20 — Over-hit: bonus XP for overshooting a killing blow

The fourth **G20** slice, and the first past the milestone gate (met by
[PLAN_G20_PVP_KILLS.md](PLAN_G20_PVP_KILLS.md)). Pure breadth.

Java sources: `Attackable.setOverhitValues` / `calculateOverhitExp`,
`AttackableStatus.reduceHp`, the damage effect handlers' `overHit` param.

---

## 1. The mechanic

A skill flagged `overHit` that lands the **killing blow** banks the *excess*
damage — the amount by which it overshot the victim's remaining HP. On the kill
reward, that excess pays bonus XP:

```
bonus% = min(excess / maxHp * 100, 25)
bonus  = bonus% / 100 * exp
```

so a blow that overshoots by a quarter of the mob's health pays +25 %, and
anything beyond that is capped. The killer also gets the "Over-hit!" notice.

59 **learnable** skills carry it — Triple Slash, Power Strike, Sonic Storm and
the rest of the early physical-skill set — so it is squarely live content.

## 2. Where the flag actually lives — a mis-port the tests caught

The first implementation read `<overHit>` as a **skill-level** field. It isn't:
it is an **effect parameter**, sitting inside `<effect>` alongside `power` and
`criticalChance`, and each damage handler reads it as
`params.getBoolean("overHit", false)`.

The behavioural test suite passed with the wrong reading — because the test
fixtures set the flag on synthetic skills directly — and only the parse
assertion against the real datapack (Triple Slash) failed. That is exactly the
value of asserting against real data rather than fixtures alone: fixtures agree
with whatever the code believes.

The flag is now hoisted from the effect params to the skill (`any effect
declares it`). A skill carries at most one damage effect in practice, so this
is behaviourally identical to Java's per-effect reading and avoids threading a
bool through every `SkillEffect` variant.

## 3. Recording and paying

`record_overhit` runs at the top of `apply_skill_damage`, where Java's
`AttackableStatus.reduceHp` consults the flag. `excess = damage - currentHp`; a
negative excess means the blow didn't kill, which **disarms** the record — as
does any damage from a non-over-hit skill. So the record only ever survives on a
corpse, and only from the blow that made it one.

`overhit_bonus` then pays it in `calculate_rewards`, to the attacker who landed
it and no one else, clearing the record so a kill pays it once.

> **The record is transient by design.** A lethal blow runs
> `apply_skill_damage` → … → `npc_do_die` → the reward path, which spends and
> clears the record — all inside the same call. Tests therefore assert the
> observable outcome (exp paid, notice sent) rather than the intermediate
> component, which is already gone by the time they could look.

Java's companion message 362 ("acquired N bonus XP from a successful over-hit")
is defined but never sent in this build, so only `OVER_HIT` (361) is ported.

## 4. Tests

Parse assertions against real skills: Triple Slash 1 and Sonic Storm 7 over-hit,
Might 1068 does not.

Behaviour (`game_loop/tests/overhit_tests.rs`, 4 cases): an over-hit kill paying
more exp than the same kill without it *and* announcing "Over-hit!"; the 25 %
cap holding when the overshoot is enormous; a non-killing blow banking nothing;
and a plain skill banking nothing on a lethal blow.

## 5. What G20 still owes

**Duels** (`DuelManager` + the three `RequestDuel*` packets — G25's olympiad
reuses their shape) are the last substantial G20 feature. Then the
`SHOTS_BONUS` dynamic value. Smaller leftovers: karma decay while hunting
(`calculateKarmaLost` needs a per-level `KarmaData` table absent from this
dist), PK item drops, and the ranged trio from slice 1 (bow peace-zone check,
`CHEAPSHOT`, NPC-archer reuse timing).
