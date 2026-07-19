# G17 slice 1 — nobless

First slice of **G17 (Sub-classes, class change & nobless)**, which is the next
milestone after G21 and the one **G22 depends on**.

## Why nobless first

G17's gate is *"a character changes class and gets the new skill tree; a
subclass can be added and switched."* The class-change half already part-exists
(`//setclass` grants advanced-class trees, from the G13 skill-tree slice), and
subclasses are the larger piece. Nobless is the self-contained third: a
character flag that several already-written systems were waiting on.

`characters.nobless` was **read at login and then dropped on the floor** — it
never reached the `Player`, nothing consumed it, and it was not in the save
UPDATE, so it could not be set either.

## What landed

- `Player.is_noble`, sourced from the existing `CharData.noble`.
- `nobleSkillTree.xml` loaded (**8** skills: Noblesse Blessing, the three
  Noblesse songs, Build Advanced Headquarters, Wyvern Aegis, …). The tree
  loader explicitly skipped every non-`classSkillTree` block, so this file had
  never been parsed. It has the same flat shape as the hero tree, so it reuses
  that parser.
- `//setnoble` — mirrors `//sethero`: toggle the flag, grant or remove the
  tree, resend the skill list and `UserInfo`.
- **Persistence**: `nobless` added to the `characters` UPDATE. Without it
  `//setnoble` looked like it worked and silently reverted on restart.
- **Consumers unblocked**: the noblesse teleport lists now check the player's
  nobless instead of refusing everyone.

## One rule that differs from hero, deliberately

`setHero` only grants the hero tree while the player is **on their base class**.
`setNoble` has no such gate in Java — nobless belongs to the character, not to
the active class, so a subclass keeps it. That distinction matters as soon as
the subclass slice lands, so it has its own test now.

## Tests

6 in `game_loop/tests/noble_tests.rs`: a new character isn't noble; `//setnoble`
grants the tree; removing it takes the skills back; nobless is granted
regardless of the active class (the hero contrast above); the flag reaches the
save command; and a real-datapack check that `nobleSkillTree.xml` has 8 skills
including 1323 and 326.

**755 tests green across all 8 targets.**

## Deliberate narrowings (`TODO(G17)`/`TODO(G25)` at the site)

- Nobless is only obtainable via `//setnoble`; the Olympiad path that awards it
  is G25.
- The noblesse tiara item and the `Noblesse Blessing` death-penalty exemption
  aren't wired — the skill is granted, its effect handler is a separate item.

## Next in G17

- **Subclasses** — the gate's headline. `character_subclasses` exists in the
  schema and `class_index = 0` is hard-coded in six places in `db.rs`; that's
  the surface to open up.
- Occupation change through the village-master flow (the *mechanic* can precede
  G22's quests).
- Certification skills.
