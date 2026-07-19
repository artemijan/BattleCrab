# G20 — Death item drops (the karma penalty)

The sixth **G20** slice, and the one that finishes the karma system started in
[PLAN_G20_PVP_KILLS.md](PLAN_G20_PVP_KILLS.md): a PK who dies scatters part of
their inventory.

Java source: `Player.onDieDropItem`.

---

## 1. Picking it — and what got skipped

G20's named features were done, so this slice came from checking what remained
against **this dist** rather than the roadmap's list:

| item | verdict |
|---|---|
| **PK item drops** | **live** — all five `Karma*Drop*` rates enabled in `Rates.ini`, plus the `Player*Drop*` set |
| `SHOTS_BONUS` | **dead** — *zero* items declare `reducedSoulshot`, so the stat has no source and always reads 1. Porting it would be a verified no-op |
| karma decay while hunting | **blocked** — `calculateKarmaLost` needs a per-level `KarmaData` table that isn't in this dist's `data/` |
| party duels | **blocked** — needs arena instances (G27) |

So one of the four was worth doing, one is provably pointless here, and two are
blocked. That is what "G20 is finished" actually looks like.

## 2. Two rate sets, two very different meanings

Java runs the same drop loop with two configurations, and the distinction
matters:

- **A playable killer** only triggers drops when the victim is a PK *past*
  `MinimumPKRequiredToDrop` (4). This is the **karma penalty** — not a general
  looting mechanic. Killing a clean player takes nothing from them.
- **A monster killer** uses the `Player*` rates (5 % gate on this dist), so an
  ordinary death to a mob can still cost an item regardless of karma.

Both are tested, because the natural assumption — "dying to a player drops
loot" — is wrong.

## 3. The loop

Gate roll on the rate, then per item: adena and quest items never drop;
equipped items are unequipped first and use the equip (or *weapon*) percentage
rather than the inventory one; each success drops to the ground via the G15
ground-item machinery and counts toward the limit.

Exemptions ported: PVP-zone deaths when a player did the killing (arena is
free), and GMs.

Not modelled, each for want of a ported source: shadow and time-limited items,
pet control items (G29), the `KarmaListNonDroppableItems` whitelists, and the
clan-war exemption — warring clans don't make each other drop (`TODO(G18)`).

## 4. Tests

`game_loop/tests/death_drop_tests.rs`, 7 cases: a repeat PK killed by a player
scattering their loot; a **clean** player killed by a player keeping everything;
a PK below the kill threshold being spared; a monster kill costing an item
through the player rates; the drop count capped by the limit; adena and quest
items never falling; and arena deaths being free.

Rates are pinned to 100 in the fixtures so a wiring bug can't hide behind a
roll.

## 5. G20 is complete

Everything the milestone named is done, and the remainder is either dead on this
dist (`SHOTS_BONUS`) or blocked on another milestone (karma decay's data, party
duels' arena instances). The three ranged leftovers from slice 1 — the bow
peace-zone check, `CHEAPSHOT`, NPC-archer reuse timing — are the only genuinely
open scraps, and none is reachable in normal play.

Natural next milestones by the roadmap's ordering: **G17** (sub-classes, class
change, nobless) or **G18** (clans, full) on the progression track, or **G21**
(NPC AI & world-content breadth) which now has G19 and G20 beneath it.
