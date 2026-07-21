# G29 slice 27 — servitor reconnect

The sibling of slice 26, and a genuinely different shape.

## A servitor has no persistent identity

A pet is restored from its **collar** — a real item with a stable object id.
A servitor has no such anchor. Java restores it by **re-casting the summoning
skill** (`skill.applyEffects(player, player)`) and then stamping the saved
vitals and remaining lifetime onto whatever that produced.

Ported as written, which buys two things:

- A restored servitor is built by the ordinary summon path, so it cannot drift
  from a freshly summoned one.
- It comes back at the player's **current** level of the skill, so levelling
  between sessions is rewarded rather than ignored.

`character_summons` therefore stores the *skill id*, not an npc id.

## Details that matter

- **Remaining lifetime is preserved**, so relogging is not a free duration
  reset. A servitor summoned with no lifetime (`u64::MAX`) stores 0 and lets
  the re-cast decide again.
- **The row is consumed on restore**, before the re-cast (Java's
  `removeServitor` ordering). A skill the player no longer knows — a subclass
  change between sessions — must not be retried on every login.
- **An empty row is written when nothing is out**, or a servitor dismissed
  before logout would come back anyway.

## A write that could have cost a character

`store_player` gained `DELETE FROM character_summons`. With `?`, that aborts
the **entire** transaction on any schema lacking the table — losing items,
skills and position over an absent servitor row. Six unrelated persistence
tests failed on exactly that, which is how it was caught.

The statements are now best-effort (`let _ =`), the same rationale as
`load_account_var`, applied to a *write* because a failing write inside a
transaction takes everything else down with it.

**Worth generalising: adding a table to a shared flush is not additive.** A new
`?` in a transaction is a new way for every prior write to be lost.

## Tests

`servitor_tests` 118 → 121, `char_persistence` extended.

- A servitor out at logout comes back with the HP it had and *roughly its
  remaining* lifetime — asserted as a range, not a fresh 1200 s.
- One dismissed first stays away.
- An unlearned skill restores nothing **and the row is consumed**, so it is not
  retried forever.
- The real-schema test round-trips a `character_summons` row and checks the
  lifetime is not reset by a relog.

## Still open in G29

Pet spiritshots (needs pets to cast), servitor master-buff inheritance,
`ServitorSkillUse`.
