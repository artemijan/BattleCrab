# G22 slice 2 — Elf/Human first-class transfers

Second G22 slice. `ElfHumanFighterChange1` and `ElfHumanWizardChange1`, which
between them cover the Human and Elf first occupations.

## Two races through one script

Unlike the Dwarf pair, these serve **two races from the same NPCs**: a Human
Fighter (0) and an Elven Fighter (18) talk to Pabris and see different class
lists; likewise Mage (10) / Elven Mage (25) on the wizard side.

| Script | From → to | Proof |
|---|---|---|
| Fighter | Fighter → Warrior (1) | Medallion of Warrior |
| | Fighter → Knight (4) | Sword of Ritual |
| | Fighter → Rogue (7) | Beziques' Recommendation |
| | Elven Fighter → Elven Knight (19) | Elven Knight Brooch |
| | Elven Fighter → Elven Scout (22) | Reisa's Recommendation |
| Wizard | Mage → Wizard (11) | Bead of Season |
| | Mage → Cleric (15) | Mark of Faith |
| | Elven Mage → Elven Wizard (26) | Eternity Diamond |
| | Elven Mage → Oracle (29) | Leaf of Oracle |

**The `from_class` half of each pair is load-bearing.** Java matches on
`(classId == TARGET) && (player.getClassId() == SOURCE)`; drop the source check
and a Human Fighter could take Elven Knight from the same NPC. There's a test
for exactly that — it asks for class 19 as a Human and asserts nothing moves
and nothing is consumed.

## Table, not branches

Java writes nine near-identical `else if` blocks. Each target owns **four
consecutive html pages** in a fixed order — `lowLevel`, `lowLevelNoProof`,
`afterClassChange`, `noProof` — so the port expresses the matrix as a table of
`(to_class, from_class, proof_item, first_page)` and derives the page from the
offset. That's ~40 lines instead of ~200, and the ordering is stated once
rather than nine times.

The risk of a table is an off-by-one silently serving the wrong page, so the
page-existence test walks **every** target's four-page block across **every**
NPC (9 NPCs × 9 targets + the fixed talk/refusal pages). It passes, which is
what makes the compression safe.

## Tests

3 added: the Human Warrior transfer plus the cross-race refusal; the Elven Mage
→ Oracle transfer; and the full page-existence sweep.

**801 lib tests; all 9 targets green.**

## Deliberate narrowings

- The fourth-class refusal page is hard-coded to the *first* NPC's id, as in
  Java and as in the Dwarf scripts — only that NPC ships the page.

## Next in G22

- `DarkElfChange1` and `FirstClassTransferTalk` complete the first-occupation
  set (port has 6 of 16 village-master scripts after this slice).
- Then the `*Change2` second-occupation set, ~188 remaining quests, ~81 `ai/`
  scripts, daily quests (`restartTime`), the tutorial (Q00255), and `//reload`.
