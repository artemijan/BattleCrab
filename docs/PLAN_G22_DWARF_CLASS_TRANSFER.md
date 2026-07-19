# G22 slice 1 — Dwarf first-class transfers

First G22 slice. G22 depended on G17, which completed last; the class-transfer
quests are the part G17's `setClassId` mechanic unblocked.

## Where G22 stands

16 village-master scripts ship in `dist/game/data/scripts/village_master/`.
Before this slice the port had 2 (`OrcChange1`, `ClanMaster`); another session
had separately landed `onFirstTalk` + the Newbie Guide, which is also G22
territory — worth re-reading `main` before planning more of this milestone.

## What landed

`DwarfBlacksmithChange1` and `DwarfWarehouseChange1`, both turning a **Dwarven
Fighter** (53) into a first occupation at level 20+, taking the proof item and
paying 15 D-grade shadow coupons:

| Script | Target | Proof |
|---|---|---|
| Blacksmith (Tapoy, Mendio, Opix, Bolin) | Artisan (56) | Final Pass Certificate |
| Warehouse (Moke, Rikadio, Ranspo, Alder) | Scavenger (54) | Ring of Raven |

The two Java scripts are identical bar the NPC list, target class, proof item
and the category `onTalk` gates on, so they share one implementation
parameterised by a `Branch`. They call `ctx.set_class_id`, i.e. the G17
mechanic — so a class transfer through a village master and a GM `//setclass`
now go through the same code.

## A quirk kept rather than tidied

The fourth-class refusal page is hard-coded to the **first** NPC's id
(`30499-12.htm` / `30498-12.htm`) regardless of which of the four you are
talking to. That looks like a Java bug, and it is — but only the first NPC of
each set actually *ships* a `-12` page, so "fixing" it to use the current NPC's
id would produce a missing-file blank window. Kept, with the reason recorded at
the site.

## Tests

4 in `quests_tests.rs`: the Scavenger transfer end to end (class and base class
move, proof consumed, coupons paid); **level 19 is refused with the proof
kept and nothing paid**; no proof means no transfer; and every html page the
scripts can return exists in the dist.

That last one earned its place immediately — it failed on the first run because
I had assumed `data/html/village_master/…` when these pages actually live under
`data/scripts/village_master/…`. A player would have hit a blank window at the
exact moment of their class change.

**790 lib tests; all 9 targets green.**

## Next in G22

The remaining first-class scripts follow the same shape and are mostly
mechanical, though wider (multiple target classes each):

- `ElfHumanFighterChange1` (Warrior/Knight/Rogue, Elven Knight/Scout)
- `ElfHumanWizardChange1` (Wizard/Cleric, Elven Wizard/Oracle)
- `DarkElfChange1`
- `FirstClassTransferTalk`

Then the `*Change2` second-occupation set, the ~188 remaining quests, the
~81 `ai/` scripts, daily quests (`restartTime`), the tutorial (Q00255), and
`//reload` hot-reload.
