# G22 slice 7 — Elf/Human second-class transfers

Seventh G22 slice, and the one that closes the `*Change2` group:
`ElfHumanFighterChange2` (10 targets, 477 Java lines — the widest village-master
script), `ElfHumanWizardChange2` (5) and `ElfHumanClericChange2` (3).

## The surprise: these three are actually uniform

The previous slice was all about how `OrcChange2` and `DarkElfChange2` differ in
four silent ways. I went into this one expecting the same and looking for the
same four axes. There aren't any: all three scripts share the level-40 gate, the
three-proof `AND`, the 15 C-grade coupons, the `.htm` extension, the class-id
bypass event, and the same page order inside a row (`low, lowNoProof, done,
noProof`). So the port has **no per-branch code path at all** — one `Spec`
struct holds the data and one code path reads it.

That's worth stating explicitly rather than just doing, because "the last two
differed in four ways" is exactly the prior that would push you to invent
per-branch handling that isn't there.

## What genuinely differs: the greeting gate

Each script serves a Human line and an Elven line from one NPC set, and Java
gates the greeting on a *different pair* of race categories per branch:

| Script | class group | race categories | pages |
|---|---|---|---|
| Fighter | `FIGHTER_GROUP` | `HUMAN_FALL_CLASS` / `ELF_FALL_CLASS` | `30109-01..79` |
| Wizard | `WIZARD_GROUP` | `HUMAN_MALL_CLASS` / `ELF_MALL_CLASS` | `30115-01..41` |
| Cleric | `CLERIC_GROUP` | `HUMAN_CALL_CLASS` / `ELF_CALL_CLASS` | `30120-01..27` |

`FALL` / `MALL` / `CALL` are the fighter / mystic / cleric "call class" category
families — three near-identical names, and picking the wrong one greets the
right player with the class-mismatch page.

Two behaviours preserved from Java that read as bugs:

- **All** pages are hard-coded to the first NPC's id whichever master you talk
  to. The dist ships exactly one page set per script (the test asserts a second
  master ships nothing), so per-NPC names would 404.
- `THIRD_CLASS_GROUP` is checked *before* the source-class match, so a
  third-class player asking for anything gets the refusal page rather than the
  silence a non-matching row produces.

## The `from_class` half is load-bearing

Same lesson as the Change1 slice, and worse here: all ten Fighter targets hang
off one NPC, so matching only on the target class would let a Human Knight take
**Temple Knight** — an Elven Knight's class — from the same master. Java's
condition is `(classId == TARGET) && (getClassId() == SOURCE)`; there's a test
that hands a Human Knight exactly the Temple Knight marks and asserts nothing
happens and nothing is consumed.

## A numbering coincidence, noted so it isn't "tidied"

In the Cleric script the third-class refusal is page 15 and the first row starts
at page 16 — the same numbers as the Cleric (15) and Bishop (16) class ids. Pure
coincidence; the page and class-id spaces never mix. Commented at the table.

## Tests

5 added, all passing on first run: the Warrior→Gladiator transfer with marks
consumed and coupons paid; the wrong-source-class refusal above; a
two-of-three-marks refusal on the Cleric branch; a 9-case `onTalk` sweep across
all three scripts comparing the reply **byte-for-byte** against the dist page
(non-empty would pass while serving the wrong window); and a page sweep over all
three full matrices (10 + 5 + 3 rows × 4 pages, plus the fixed talk/refusal
pages) that also asserts the non-owning masters ship nothing.

First run passing is itself the result worth noting — the previous four slices
each failed first on the quest fixture's enumerated class ids, which slice 6
replaced with the full `0..=57` range. The fix held.

## Status

Port has **15 of 16** village-master scripts. Remaining: `AllianceMaster`
(67 lines), which closes the group.
