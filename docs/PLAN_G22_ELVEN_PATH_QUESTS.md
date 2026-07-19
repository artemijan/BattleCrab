# G22 slice 9 — the Elven first-occupation quests

First slice of G22's quest body, and it was chosen by a gap the previous eight
slices created: the 16 village-master scripts are done, and **every one of them
consumes a proof item that no quest in the port produced.** The transfers were
only reachable via `//setclass` or a GM-spawned mark.

`Q00406_PathOfTheElvenKnight` (278 Java lines) and `Q00407_PathOfTheElvenScout`
(341) award the Elven Knight Brooch (1204) and Reisa's Recommendation (1217) —
the two elven proofs `ElfHumanFighterChange1` takes. The elven half of that
transfer is now reachable in normal play.

## The finding: this quest deliberately ignores the drop rate

Q00406 does **not** call `giveItemRandomly`. It hand-rolls
`getRandom(100) < chance` and calls a plain `giveItems`. That matters because
the port's `give_item_randomly` faithfully reproduces Java's helper, which
multiplies **both chance and amount** by `RateQuestDrop` — so reaching for the
convenient helper here would have silently scaled a drop Java leaves alone.

I only caught it by diffing against `Q00303_CollectArrowheads`, which *does*
call `giveItemRandomly` and whose port therefore correctly uses the helper. The
two quests look identical in shape and differ in exactly this. There's a test
that sets `RateQuestDrop = 3.0` and asserts a single kill still yields one
topaz.

Generalised: **check whether the Java quest calls the helper or rolls its own
before picking the Rust primitive.** They are not interchangeable.

## Q00407's tag mechanic needs both hooks

`onAttack` stamps the mob's script value with the attacker's object id;
`onKill` pays out only if the killer matches. Porting one without the other
fails silently in opposite directions — `onKill` alone never matches (every
kill drops nothing), `onAttack` alone leaks the tag. Tested both ways: a mob
killed without being attacked pays nothing, the same mob attacked first pays.

## Page conventions

Extensions are **mixed inside a single quest**: `.htm` for the pre-accept
dialog, `.html` for everything after the quest starts. Copied exactly rather
than normalised.

Prias ships `30426-01`, `-02` and `-04` but **no `-03`**, and Java never names
one — the same "the gap is real" shape as `FirstClassTransferTalk`. The page
test asserts the absence so it isn't helpfully filled in.

One deliberate deviation, commented at the site: Reoria's talk branches are
reordered (honorary-guard hoisted above the `variable` check). Safe because
Java's `variable` branch is itself guarded by
`!hasAtLeastOneQuestItem(REISAS_LETTER, HONORARY_GUARD)`, making the two
mutually exclusive in either order.

Also collapsed a Java three-way level branch (`>= 20` / `== 19` / else) that
awards **the same 80314 exp / 5087 sp in all three arms** — noted in a comment
so it doesn't read as a dropped case.

## Tests

4 added: the rate-independence test above; the full Q00406 chain (20 topaz →
Sorius' letter → Kluto's memo → 20 emerald → the box → brooch + `SocialAction`
+ COMPLETED state); Q00407's tag mechanic; and a page sweep over both quests
covering the mixed extensions and Prias' gap.

The chain test failed once on first run, on `exitQuest(false, …)` — I asserted
the quest record was gone, but a one-time exit keeps it as **COMPLETED**
(deleting it would let the quest be repeated). Assertion corrected, not the
code.

Added `QuestCtx::social_action` (Java sends it with `sendPacket`, to the player
only — not a broadcast).

## Status

2 of ~188 quests in G22's quest body; 14 quests ported overall. The obvious
continuation is the remaining seven Elf/Human `Path of the *` quests (401–405,
408, 409), which would make both `ElfHumanFighterChange1` and
`ElfHumanWizardChange1` fully reachable.
