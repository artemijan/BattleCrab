# G29 slice 13 — per-level pet stats

Slice 12 made pets level up, but a levelled pet's *number* moved while it stayed
exactly as strong as at level 1: combat still read the NPC template, and the
`org_pattack`/`org_hp`/… columns sat parsed-but-unread. This closes that.

## Checking who consumes them first

After the cubic `power` episode — where a field looked unconsumed and the port
was in fact consuming the *wrong* source — the first step was finding Java's
readers rather than assuming these were merely unread. They are consumed at the
**finalizer** level:

| finalizer | pet override |
|---|---|
| `MaxHpFinalizer` | `getPetMaxHP()` |
| `MaxMpFinalizer` | `getPetMaxMP()` |
| `PDefenseFinalizer` | `getPetPDef()` |
| `MDefenseFinalizer` | `getPetMDef()` |
| `IStatFunction.calcWeaponBaseValue` | `getPetPAtk()` / `getPetMAtk()` |
| `RegenHPFinalizer` / `RegenMPFinalizer` | `getPetRegenHP/MP() × Config.PET_*_REGEN_MULTIPLIER` |

The shape is uniform: **wherever a normal NPC uses its template base, a pet
substitutes its per-level row and then runs the same bonus math.**

## So substitute the bases, not the pipeline

`pet_template_at_level` clones the NPC template with the pet row's values (and
the **pet's own level**, which drives `levelMod`) patched in, then hands it to
the existing `npc_finalized_stats`. That reproduces Java exactly while reusing
the entire pipeline already built and tested for NPCs — STR/INT/CON/MEN bonuses,
`levelMod`, the m.atk `^2.2072` power, template passive skills, and player-cast
buffs — instead of growing a parallel pet-only stat path that would drift.

## Two details worth stating

- **Levelling preserves the HP/MP *fraction*.** A recompute that refilled the
  bar would be a free heal on demand; one that kept the absolute value would
  effectively wound the pet as its maximum rose. Tested.
- **A row missing a stat falls back to the NPC template** rather than
  substituting zero. Java reads the pet row unconditionally and every shipped
  species populates all of these, so this never fires on real data — but a
  single missing `org_hp` would otherwise give the pet **0 max HP**. This guard
  was not speculative: the shared test fixture carries no combat stats, and
  without it two restore tests started returning a pet at 0 HP.

## Regen is parsed but still unread

`org_hp_regen` / `org_mp_regen` are parsed and stored. `NpcTemplate` has no
regen fields at all on this port, so wiring them means touching the NPC regen
path rather than the pet one — deliberately left for its own slice, and flagged
here so it stays greppable rather than becoming another silent gap.

## Tests

`servitor_tests` 62 → 66, `pet_data` 2 → 3.

The fixture gives level 2 strictly better stats than level 1, so "did levelling
do anything?" is answerable rather than vacuous. Alongside: stats come from the
pet table and *not* the NPC template (asserted as a difference, not just
non-zero), levelling raises p.atk/m.atk/p.def/max-HP, the HP fraction is
preserved, and the template fallback works.

Because fixtures can't catch a parse-arm slip, the `pet_data` test reads the
**shipped Wolf**: exact `org_pattack`/`org_mattack`/`org_pdefend`/`org_hp`/
`org_mp` values, `get_exp_type == 73`, and that the top level is strictly
stronger than level 1.

## Still open for pets

- Pet **regen** (above).
- Pet death, `restoreExp` on resurrection, and the exp lost on death.
- `PET_EQUIP` paperdoll — battle-pet armour and weapons.
- Soulshot/spiritshot counts (`soulshot_count`/`spiritshot_count` parsed
  nowhere yet), evolution, auto-resummon on reconnect.
