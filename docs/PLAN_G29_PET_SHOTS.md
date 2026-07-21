# G29 slice 18 — summon shots (Beast Soulshot)

The port's `RequestAutoSoulShot` carried an explicit narrowing — "summon shots
aren't in scope" — and `soulshot_count`/`spiritshot_count` were unparsed. Pets
could not use shots at all.

## Shots belong to the owner, the swing belongs to the pet

Java `Summon.rechargeShots` reads the **owner's** auto-shot list, consumes from
the **owner's** inventory, and charges the **summon**. So a Beast Soulshot is
toggled and paid for by the player but spent by the pet's attack — three
different actors in one flow, which is why it needs its own path rather than
reusing the player one.

`getSoulShotsPerHit()` is the pet's **per-level** `soulshot_count`, so a
levelled pet costs more per swing. That is the mechanic (keeping a high-level
pet shotted is a real drain), not an incidental value, and a test asserts the
cost tracks the level rather than being fixed.

## Java's summon branch skips the weapon-grade check

`RequestAutoSoulShot`'s `isSummonShot` branch checks `player.hasSummon()` and
**never looks at the player's weapon** — the shots are for the pet's swing.
Reusing the player's grade check would have rejected every Beast Soulshot,
since the player's weapon grade has nothing to do with it. The toggle also
charges the summon immediately on activation, as Java does.

## `_chargedShots` lives on Creature, not Player

The port grew the player half first (`Player.is_charged_shot`), so an NPC
attacker skipped the charge/spend entirely. Java keeps `_chargedShots` on
`Creature`, shared by players and summons. Added as a `ChargedShots` component
for summons rather than moving the player's bits, because unifying them touches
every player-shot call site for no behavioural gain today —
`TODO(G29+)` at the site.

## Partial payment buys nothing

If fewer shots remain than one swing costs, nothing is spent and the pet stays
uncharged — no partial charge for a partial payment. Java also drops the toggle
when the item runs out entirely, which is ported.

## Tests

`servitor_tests` 86 → 92, `pet_data` +2 assertions.

Charging from the owner, the **cost following the pet's level** (2 at level 1,
3 at level 2 in the fixture — deliberately different so the assertion is
answerable), no double-spend while charged, a partial stack buying nothing, the
charge spending exactly once, and no charge at all without the auto-use toggle.

Datapack-backed: the shipped Wolf's `soulshot_count` is 1 at level 1 and rises
with level, so the growth is real rather than a fixture artefact.

## Still open for pets

- **Spiritshots**: `SummonSpiritshot`/`BeastSpiritShot` parse and the count is
  stored, but only the physical half is wired — the magic bonus needs the pet
  to cast, which it does not yet do.
- `PET_EQUIP` paperdoll, evolution, reconnect resummon (`CharSummonTable`).
