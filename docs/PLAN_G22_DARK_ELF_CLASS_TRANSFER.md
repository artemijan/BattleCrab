# G22 slice 3 — Dark Elf first-class transfer + a second class-corruption fix

Third G22 slice. `DarkElfChange1` completes the racial first-occupation set
(Human, Elf, Dark Elf, Orc, Dwarf all covered).

## The important part isn't the script

While wiring it I read `QuestCtx::set_class_id` and found it still did:

```rust
p.class_id = class_id;
p.base_class_id = class_id;   // unconditional
```

That is **the same bug I fixed in `//setclass` during G17 slice 6** — in a
second writer I hadn't checked. A quest-driven class transfer taken while a
subclass was active would have rewritten the character's *base* class.

I recorded the lesson last time as *"when a new axis appears, every existing
writer of the affected field becomes suspect"*. I then fixed exactly one writer
and moved on. Finding the second one by accident, a milestone later, is the
cost of not having actually enumerated them. `QuestCtx::set_class_id` now
routes through `subclass::set_class_id`, so all three paths — GM command,
village-master script, quest — share one implementation.

That change surfaced 4 failing quest tests, because the shared mechanic
(correctly) refuses a class id with no template and the synthetic quest world
registered only class 0. Fixed in the fixture, centrally.

## Three ways DarkElfChange1 differs from its siblings

Easy to get wrong by pattern-matching on the scripts already ported:

1. Java already writes it as a **table**, and the bypass event is the **row
   index** (`0..3`), *not* a class id.
2. The page order inside a row is `lowNoProof, low, noProof, done` — the
   opposite pairing to `ElfHumanFighterChange1`'s `low, lowNoProof, done,
   noProof`.
3. The pages are **`.html`**, not `.htm`.

Each is silent if mis-ported: wrong page, or a blank window.

It also honours `if (player.isSubClassActive()) return getNoQuestMsg(player)`,
which the G17 work makes expressible — added `QuestCtx::is_subclass_active`.

## Tests

4 added: the row-index transfer; a Dark Mage refused the Dark Fighter row
(source-class check, same NPC serves both); refusal while a subclass is active;
and the `.html` page sweep across all 3 NPCs and all 16 matrix pages.

**805 lib tests; all 9 targets green.**

## Next in G22

- `FirstClassTransferTalk` finishes the first-occupation group (port has 7 of
  16 village-master scripts).
- Then the `*Change2` second-occupation set, ~188 remaining quests, ~81 `ai/`
  scripts, daily quests (`restartTime`), the tutorial (Q00255), `//reload`.
