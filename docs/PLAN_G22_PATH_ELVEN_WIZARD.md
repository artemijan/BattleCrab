# G22 slice 14 — Path of the Elven Wizard

`Q00408_PathOfTheElvenWizard` (446 Java lines), awarding the **Eternity
Diamond** (1230) — the last of `ElfHumanWizardChange1`'s four proofs.

**With this, the whole Elf/Human first-occupation tier is self-sufficient.**
Both `ElfHumanFighterChange1` (5 proofs) and `ElfHumanWizardChange1` (4) can now
be satisfied entirely in normal play. Nine quests were needed for that; this is
the ninth.

## Three parallel errands, one table

Rossela runs three errands, all required, in any order, and each is the same
four beats: she hands out an introduction, the specialist swaps it for a charm,
the charm gates a monster drop, and the specialist trades the full set for a
gem. Three gems buy the diamond.

| # | Introduction | Specialist | Charm | Mob | Material | Need | Chance | Gem |
|---|---|---|---|---|---|---|---|---|
| 1 | Rossela's Letter | Greenis | Greenis's Charm | Pincer Spider | Red Down | 5 | 70% | Ruby |
| 2 | Appetizing Apple | Thalia | Sap of the Mother Tree | Dryad Elder | Gold Leaves | 5 | 40% | Aquamarine |
| 3 | Immortal Love | Northwind | Lucky Potpourri | Sukar Wererat Leader | Amethyst | 2 | 40% | Nobility Amethyst |

## The third errand is missing a step — and the dist proves it isn't a bug

Errands 1 and 2 perform the introduction → charm swap in a **dialog event**
(`30157-02.html`, `30371-02.html`). Errand 3 has no such event: Northwind does
the swap inline in `onTalk`.

That is exactly the kind of asymmetry worth "regularising" — until you count
pages. Greenis and Thalia each ship four; **Northwind ships three**
(`30423-01..03`). There is no fourth page for an event to return, so adding one
would 404 at the moment a player takes the third errand. The port keeps the
irregularity (`swap_event: Option<&str>`, `None` for Northwind) and the page
test asserts `30423-04.html` does **not** exist, so the reasoning survives the
next reader.

This is the same shape as `FirstClassTransferTalk`'s asymmetric pages and
Q00407's missing `30426-03`: **when a script's structure looks inconsistent,
check whether the dist's page set explains it before normalising.**

## Never advances `cond`

Like quest 402, `setCond` appears **zero** times in 446 lines — verified by
grep, not inferred from the parts I read. `startQuest` sets cond 1 and it stays
there; progress lives entirely in which items you hold, which is why the
`onTalk` chains read as long item interrogations. Reaching a collection cap
plays the middle sound and nothing else.

Chance denominator is `/100` here, as in 404 and 406 — not the `/10` of
401/403.

## Tests

3 added, all green on first run: all three errands end to end through to the
diamond (asserting the two event-driven swaps *and* Northwind's talk-driven one
in the same loop); the charm as drop gate (the same mob pays nothing before the
swap); and the page sweep, including Northwind's absent fourth page.

## Status

21 quests ported. The Elf/Human first-occupation tier is complete. Natural
continuations: the Dark Elf / Orc / Dwarf `Path of the *` quests (410–418, nine
of them) would do the same for the other three races' `*Change1` scripts, which
are already ported and currently proof-starved in exactly the way the Elf/Human
ones were.
