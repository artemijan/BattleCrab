# G23 slice 4 — the grand-boss respawn lifecycle

## Ported once, not ten times

Every `ai/bosses` script's `onKill` does the same four things: mark the boss
**dead**, roll a respawn window, **persist** it, and arm a timer. Java repeats
that block in all ten files. The port keeps it in `game_loop/grand_boss.rs`,
driven by `GrandBoss.ini`.

What stays per-boss is the interesting part — Queen Ant's nurses, Antharas's
phases, Baium's sleep — not the bookkeeping. Porting QueenAnt's copy first and
generalising later would have meant rewriting it nine times.

## The boot branch is the one that matters

Java's init is a three-way branch, and it is the reason boss state is in a table
at all:

1. **alive** → spawn at the stored location with the stored HP/MP;
2. **dead, timer running** → arm the remaining time;
3. **dead, timer already expired while the server was down** → spawn *now*.

Case 3 is easy to miss and the most visible when missed: a boss whose window
elapsed during downtime would stay dead **forever**, because the only thing that
schedules a respawn is a kill, and it cannot be killed. Its own test.

## Details

- The window is `(interval + getRandom(-random, random)) hours`, so a boss is
  never up at a predictable time. Tested as a **range**, plus a separate test
  that the value actually varies across kills — asserting a single number would
  pass on a broken fixed window.
- **Baium ships an interval with no `RandomOf…Spawn` key**, so the spread
  defaults to 0 rather than being assumed symmetric with the others. Pinned,
  since a copied default would give Baium a spread retail doesn't have.
- Respawning a boss that is already alive is a **no-op** — a duplicate timer or
  an admin spawn would otherwise stand up a second copy.
- A stored HP of 0 means "never wounded", so only a positive value overrides the
  template's full vitals; a boss wounded before a restart comes back wounded.
- `StoreGrandBoss` writes `grandboss_data` on both death and respawn. Sent as a
  fire-and-forget command, not folded into the character flush — it has nothing
  to do with a character and would inherit that transaction's failure mode.

## Tests

New `grand_boss_tests` (8), covering all three boot branches, the window's range
and its variability, the no-op respawn, wounded-HP restore, and the **real**
`GrandBoss.ini` (Queen Ant 36 ± 17, Baium's absent spread, and an ordinary NPC
having no window at all).

## Next for QueenAnt specifically

The lifecycle now drives her spawn and respawn. What remains is her own script:
the nurse/guard/royal minions, the nurse heal behaviour, and `movePlayersTo` on
spawn (the zone for which resolves as of slice 3).
