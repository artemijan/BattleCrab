# G23 slice 2 — raid points

## Two candidates measured, one struck

The remaining G23 items were checked for reachability before picking:

- **Chaos target swaps** — `isChaos` has **zero occurrences** in this dist's NPC
  data. The mechanic exists in Java and no NPC enables it here. **Struck**, like
  agathions and pet evolution in G29.
- **Raid points** — 409 `<acquire raidPoints>` attributes, of which **374 are
  non-zero** (5, 7, 21, 100). Real content, and unimplemented: zero references
  in the port.
- **Minion waves** — already done, in G21.

## The award rule

Raid points are a separate currency from exp, and the distribution differs from
the exp split in ways worth stating:

- They go to the **top damage dealer** (falling back to the last attacker), not
  proportionally to everyone who helped.
- If that player is in a **party**, the points are split among party members
  **within `ALT_PARTY_RANGE` of the corpse** — including members who dealt no
  damage at all, and excluding ones who hung back.
- `Math.max(points / size, 1)` — a split never rounds a member down to zero.
- `!_isRaidMinion` — a boss's adds award nothing, only the boss itself.

`CONGRATULATIONS_YOUR_RAID_WAS_SUCCESSFUL` is **broadcast**, not sent to the
earner: everyone present sees the raid succeeded.

## Persistence

`raidbossPoints` is an existing `characters` column, so this is one field on
`Player`/`PlayerSnapshot` and one more bind in the update — no new table, and
none of the shared-flush hazard from G29 slice 27.

## Tests

`raid_curse_tests` 7 → 13, `char_persistence` extended.

Solo award, ordinary monsters awarding nothing, the party split, a distant
member earning nothing while the in-range one takes the whole share, the rate
config, and a **datapack-backed** check that many NPCs really carry non-zero
raid points — a fixture cannot catch a parse regression on
`<acquire raidPoints>`.

## Still open in G23

Boss zones + entry conditions. Minion waves and persistence are done (G21),
raid curse landed in slice 1, chaos is struck.
