# G29 slice 17 — pet regeneration

## The carried-forward claim was wrong (again)

Slices 13–16 each closed with "pet regen: `org_hp_regen`/`org_mp_regen` parsed,
but **`NpcTemplate` has no regen fields at all**, so wiring them means touching
the NPC regen path." That claim rode through three plan docs and three PROGRESS
rows.

It was false. `NpcTemplate` has `base_hp_reg`/`base_mp_reg`, and
`run_npc_regen_tick` already reads them. I had grepped for `hp_regen` and the
fields are named `hp_reg` — a two-character miss that turned a ten-line
substitution into an imagined subsystem.

That is the second carried-forward TODO in three slices to be wrong on
inspection (after the pet-corpse "24 hours"). **Re-verify a note before acting
on it, and make the grep that produced it wide enough to fail loudly.**

## The actual change

Java's `RegenHPFinalizer`/`RegenMPFinalizer` pet branch:

```java
baseValue = ((Pet) creature).getPetLevelData().getPetRegenHP()
            * Config.PET_HP_REGEN_MULTIPLIER;
```

Same shape as every other pet stat (slice 13): **substitute the base, keep the
pipeline**. The one difference is that regen re-reads the template on every tick
rather than caching onto a component, so the substitution lives in
`run_npc_regen_tick` rather than in `pet_template_at_level`.

`PetHpRegenMultiplier`/`PetMpRegenMultiplier` are now real config keys. Both are
100 (×1.0) on this dist, so inlining 1.0 would have been invisible today and
wrong for any server that retunes them — and a monster-regen retune must not
silently retune pets.

## Tests

`servitor_tests` 82 → 86, `pet_data` +2 assertions.

- A pet regenerates from its pet row (the Wolf NPC template has no regen, so a
  template-driven pet would not heal at all — the assertion distinguishes the
  two sources rather than just checking "healed").
- Regen clamps at maximum.
- The **pet** multiplier applies and the monster one does not: the test sets the
  monster multiplier to an absurd 100× that must not appear in the result.
- A dead pet does not regenerate back to life while its corpse waits to decay.

Datapack-backed: the shipped Wolf's `org_hp_regen` is 2.0 at level 1 and rises
with level, so the growth is real rather than a fixture artefact.

## Still open for pets

- `PET_EQUIP` paperdoll — battle-pet armour and weapons.
- `soulshot_count`/`spiritshot_count` — parsed nowhere yet; pets can't use shots.
- Evolution, reconnect resummon (`CharSummonTable`).
