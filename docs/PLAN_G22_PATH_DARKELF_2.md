# G22 slice 16 — Path of the Dark Wizard / Path of the Shillien Oracle

`Q00412_PathOfTheDarkWizard` (384 Java lines) and
`Q00413_PathOfTheShillienOracle` (328), awarding the Jewel of Darkness (1261)
and the Orb of Abyss (1270).

**The Dark Elf first-occupation tier is complete.** `DarkElfChange1` has all
four proofs; a Dark Fighter or Dark Mage can now reach any of its targets in
normal play. Two races down, two to go (Orc 414–416, Dwarf 417–418).

## Q00412 repeats quest 408's third-errand asymmetry — twice makes it a convention

Charkeren and Annika hand their tool over through a **dialog event**; Arkenia
hands the Hub Scent over **inline in `onTalk`**, with no event. That is exactly
the shape quest 408 has (Greenis and Thalia use events, Northwind doesn't).

One occurrence looked like an oversight worth documenting. Two independent
quests, different races, different authors doing the same thing makes it a
datapack convention — so it's modelled (`tool_event: Option<&str>`) with no
further hedging. Both branches are exercised in one test loop.

Arkenia's branch also omits the `hasQuestItems(SEEDS_OF_DESPAIR)` guard its two
siblings carry. Kept as-is: her errand is reachable slightly earlier, and
adding the guard for symmetry would change who can start it.

## Q00412's chance is an equality, not a threshold

All three drops roll `getRandom(2) == 0` — **`==`, where every other Path quest
uses `<`**. The probability is the same 50% here, but the form is not
interchangeable: reading it as `getRandom(2) < 2` makes every kill pay. This
one *is* deterministically testable (unlike the `/10` vs `/100` cases), because
a forced roll of 1 distinguishes the readings — so there's a test for it.

That's now four distinct chance conventions in this family: `/100`, `/10`,
`== 0`, and no roll at all.

## Q00413's succubus kill is a swap, not a drop

Every other collection in the family *adds* an item. This one **consumes** one:
each Dark Succubus takes a Blank Sheet and gives a Bloody Rune, so the counts
move in opposite directions and the stage ends when the sheets run out. The
cond tests *both* conditions — sheets exhausted **and** five runes:

```java
giveItems(killer, BLOODY_RUNE, 1);
takeItems(killer, BLANK_SHEET, 1);
if (!hasQuestItems(killer, BLANK_SHEET) && (getQuestItemsCount(killer, BLOODY_RUNE) == 5))
```

Modelling it as a capped drop would strand five sheets in the bag and never
fire the cond. Tested per-kill in both directions, plus a sixth succubus
proving no sheet means no rune.

Talbot also hands over **five** sheets in one `giveItems(..., 5)` — the same
stack-not-singleton shape as Simplon in quest 405. Neither of 413's drops rolls
a chance, while its sibling 412 rolls all three: the conventions differ quest by
quest even inside one race tier.

## Tests

4 added, all green on first run: Q00412's three errands through to the jewel
(covering both tool-handover styles); the `== 0` coin flip pinned by a forced
roll of 1; Q00413's sheet→rune swap counted per kill; and Q00413 end to end.

## Status

25 quests ported. Dark Elf tier complete. Remaining Path quests: Orc (414 Raider
326, 415 Monk 652, 416 Shaman 525) and Dwarf (417 Scavenger 690, 418 Artisan
562) — the Orc/Dwarf pair are the widest left in the family.
