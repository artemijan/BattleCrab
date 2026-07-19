# G22 slice 5 — Dwarf second-class transfers

Fifth G22 slice, opening the `*Change2` (second-occupation) group.

| Script | From → to | Proofs |
|---|---|---|
| Blacksmith (8 masters) | Artisan (56) → Warsmith (57) | Guildsman, Prosperity, Maestro |
| Warehouse (8 masters) | Scavenger (54) → Bounty Hunter (55) | Guildsman, Prosperity, Searcher |

## Three differences from the `*Change1` pair

1. The level gate is **40**, not 20.
2. **Three** proof items are required, and all three are consumed. Java's
   `hasQuestItems(player, a, b, c)` is an **AND**, not an OR — reading it as
   "any" would let a player transfer holding a single mark. There's a test that
   supplies two of three and asserts nothing moves and nothing is taken.
3. The reward is a **C**-grade shadow coupon (8870), not D-grade.

## One structural quirk

**Every** page is hard-coded to the *first* NPC's id (`30512-…` / `30511-…`)
whichever of the eight masters you talk to. The `*Change1` scripts did this only
for the fourth-class refusal; here it's the whole dialog. The dist confirms it:
each script ships exactly one 12-page set, and the page test asserts the other
masters ship nothing of their own — so this can't be "tidied" into per-NPC
pages that would 404.

## A fixture gap the mechanic keeps catching

The transfer test failed first time because the shared `set_class_id` refuses a
class id with no template, and the quest fixture's class list didn't include 55
or 57. That's the third time this validation has caught a fixture gap rather
than a logic error — which is the right way round, and worth noting as evidence
the G17 mechanic is pulling its weight rather than just being plumbing.

## Tests

4 added: the Warsmith transfer end to end; **two of three marks is refused**;
level 39 refused while holding all three; and the page sweep including the
"only the first NPC ships pages" assertion.

**812 lib tests; all 9 targets green.**

## Next in G22

- The remaining `*Change2` scripts: `OrcChange2`, `DarkElfChange2`,
  `ElfHumanFighterChange2` (477 lines — the widest), `ElfHumanWizardChange2`,
  `ElfHumanClericChange2`.
- `AllianceMaster`.
- Then ~188 quests, ~81 `ai/` scripts, daily quests (`restartTime`), the
  tutorial (Q00255), `//reload`.
