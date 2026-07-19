# G22 slice 12 — Path of the Human Wizard / Path of the Cleric

`Q00404_PathOfTheHumanWizard` (397 Java lines) and `Q00405_PathOfTheCleric`
(329), awarding the Bead of Season (1292) and the Mark of Faith (1201) — the
two human-mage proofs for `ElfHumanWizardChange1`. That script now has two of
its four; 408 and 409 (the elven half) finish it.

## Q00404: four identical branches, one exception

A strictly linear elemental chain — Fire → Wind → Water → Earth — where each
spirit runs the same three-step bargain: token out, collect something, trade
token + collection for a trinket. The repetition is exact right down to the
page numbering (`{npc}-01..04.html` for all four spirits), so it ports as an
`ELEMENTS` table rather than four hand-written blocks.

**The exception is Wind: its collectable is not a drop.** The feather comes
from a dialog bypass on the Wasteland Lizardman (`30410-03.html`), who sits
outside the four-page scheme with his own pages. Every other branch collects by
killing. A table-driven port that assumed "collect ⇒ kill" would leave the wind
branch permanently stuck, so the feather is handled in `on_event` and the test
asserts specifically that it arrives from dialog.

**Chance denominator: `/100` here** (`getRandom(100) < 20|80`), where quests
401 and 403 use `getRandom(10)`. Same family, three quests apart, checked at
each call site instead of carried over — this is the third distinct denominator
convention in the Path family now.

### A test I deliberately did not write

I could not write an honest deterministic test for the `/100` denominator.
`forced_rolls` returns its value regardless of the bound, so `forced < chance`
is literally the same predicate under either reading — no forced test can
distinguish them. The statistical trick that worked for Q00403 doesn't apply
either: there the misreading direction made drops **rarer** (8% vs 80%), which
40 kills detect easily; here it would make them *more common*, and with a cap
of 1 you observe a single Bernoulli per quest instance, so detecting it needs
many independent worlds for little value. The denominator is instead pinned by
a comment at the call site, and the lesson is already covered by Q00403's test.
Better no test than one that looks like it proves something it can't.

## Q00405: two things that break if normalised

**Simplon hands over a stack of three.** `giveItems(BOOK_OF_SIMPLON, 3)` where
Vivyan and Praga give one each — and the completion correspondingly does
`takeItems(BOOK_OF_SIMPLON, -1)` (all of them) but `takeItems(..., 1)` for the
other two. Treating the three books uniformly would either strand two of
Simplon's in the bag or make the count check unsatisfiable. Tested explicitly.

**The cond-2 checks contain a no-op term.** Each book-giver re-checks all three
counts after giving its own, but writes its *own* slot as `>= 0`:

```java
giveItems(player, BOOK_OF_VIVYAN, 1);
if ((count(SIMPLON) >= 3) && (count(VIVYAN) >= 0) && (count(PRAGA) >= 1))
```

`>= 0` is trivially true — a placeholder for "the one I just handed over". All
three sites therefore reduce to the same predicate (hold all three books,
Simplon's counting three), which the port checks once in `has_all_books`. Read
literally it looks like a bug; it's only redundant, and collapsing it is safe
precisely because the giver's own count is guaranteed non-zero at that point.

Praga's pendant drops from Ruin Zombies with **no chance roll** at all — the
first kill after taking the necklace pays.

## Tests

5 added, all green on first run: the full Q00404 elemental chain end to end
(including the dialog-sourced feather and the two-pebble water step); branch
gating (no mirror without the Flame Earring); Simplon's stack of three plus the
whole-stack take-back; Q00405's courier loop to the Mark of Faith; and page
existence for both.

## Status

19 quests ported. `ElfHumanWizardChange1` has 2 of 4 proofs — 408 (Elven
Wizard, 446 lines) and 409 (Elven Oracle, 408) are the pair that finish it, and
with them the whole Elf/Human first-occupation tier is self-sufficient.
