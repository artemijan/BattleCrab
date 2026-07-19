# G22 slice 4 — FirstClassTransferTalk

Fourth G22 slice, completing the **first-occupation group**: all five races'
`*Change1` scripts plus the headmasters who explain them.

As the Java header says outright: *"None of them provide actual class
transfers, they only talk about it."* Seven newbie-village headmasters, pure
dialog.

## Two conventions that differ from every other village-master script

- Pages are named with an **underscore** (`30026_fighter.html`), not the
  `-NN.htm` numbering the `*Change1` scripts use.
- The extension is **`.html`**.

## The page availability is asymmetric, and that's the logic

I checked which pages each NPC actually ships before writing the branching,
and the file layout *is* the specification:

| NPC | ships |
|---|---|
| 30026 Blitz (Human fighter guild) | `fighter`, `no`, `transfer_1/2` — **no `mystic`** |
| 30031 Biotin (Human temple) | `mystic`, `no`, `transfer_1/2` — **no `fighter`** |
| 30154 / 30358 / 30565 (Elf / Dark Elf / Orc) | both `fighter` and `mystic` |
| 30520 / 30525 (Dwarf) | `fighter` only — Dwarves have no mage line |

So a mage talking to the Human *fighter* guild master gets `no.html`, not a
mystic page — there is no mystic page for that NPC to serve. The script must
answer `no` rather than construct a filename that doesn't exist. A test asserts
the three absences directly, so the branching can't drift back to a "sensible"
symmetric version that would 404.

## Testing the right page, not merely a page

My first version of the seven-case test asserted the reply was non-empty. That
would have passed while serving `fighter` where `mystic` was wanted — useless.
Rewritten to compare against the **actual dist file**, run through the same
`strip_htm` the cache applies plus the `%objectId%` substitution.

It failed immediately on the substitution, which is the point: the assertion
now has teeth.

## Tests

3 added: seven race/mage/progress cases each compared byte-for-byte against the
dist page; the wrong-race refusal; and the page-existence sweep including the
three deliberate absences.

**808 lib tests; all 9 targets green.**

## Where G22 stands

Port has **8 of 16** village-master scripts — the entire first-occupation group
is done:

| | |
|---|---|
| `OrcChange1`, `DwarfBlacksmithChange1`, `DwarfWarehouseChange1` | ✅ |
| `ElfHumanFighterChange1`, `ElfHumanWizardChange1` | ✅ |
| `DarkElfChange1` | ✅ |
| `FirstClassTransferTalk` | ✅ |
| `ClanMaster` | ✅ (pre-existing) |

## Next in G22

- The `*Change2` second-occupation set (7 scripts) — same shape, but gated on
  level 40 and the second-class proof items.
- `AllianceMaster`.
- Then the bulk: ~188 remaining quests, ~81 `ai/` scripts, daily quests
  (`restartTime`), the tutorial (Q00255), and `//reload` hot-reload.
