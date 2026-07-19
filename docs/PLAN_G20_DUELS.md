# G20 — Duels (1v1)

The fifth **G20** slice and the last substantial feature the milestone names.
Duels were a 2026-07 audit addition, placed in G20 because **G25's olympiad
matches reuse their shape** — a consented, refereed, consequence-free fight with
a countdown, an end condition sweep, and a restore.

Java sources: `Duel` (1 070 lines), `DuelManager`, `RequestDuelStart` /
`RequestDuelAnswerStart` / `RequestDuelSurrender`, `ExDuelAskStart` / `Ready` /
`Start` / `End` / `UpdateUserInfo`, `Player.canDuel`.

---

## 1. Scope: 1v1 only

Java supports **party** duels as well, which teleport both parties into a
dedicated arena instance, open its doors, snapshot every member's condition and
restore them afterwards. That needs instances (**G27**) and the arena data, so
this slice implements the player-vs-player duel that happens where the two
players stand. A `partyDuel` request is refused politely rather than
half-handled (`TODO(G27)`).

That is the same vertical-slice discipline the rest of the port uses: a
complete, working 1v1 duel beats a partial everything.

## 2. The lifecycle

1. **Challenge** (`RequestDuelStart`) — validated against `canDuel` for *both*
   sides plus a 250-unit range check, then `ExDuelAskStart` to the target and a
   pending marker on them.
2. **Answer** (`RequestDuelAnswerStart`) — declining tells the challenger;
   accepting re-checks `canDuel` (Java does too — state can change while the
   prompt sits) and starts the countdown.
3. **Countdown** — 5 seconds, announcing each one, then "Let the duel begin!".
   Java's count-4 step teleports the parties; that is party-only, so a 1v1 just
   counts down in place.
4. **Running** — `ExDuelReady` + `ExDuelStart` to both, each side gets the
   opponent's bars (`ExDuelUpdateUserInfo`), and a per-second sweep checks the
   end conditions.
5. **End** — a win (someone dropped or surrendered) or a cancel (120 s timeout,
   drifting >1600 units apart, a disconnect), then `ExDuelEnd` and the restore.

Both sides carry the duel marker from the **countdown** on, not from the start,
so neither can be challenged again while one is pending.

## 3. A duel never kills

The defining property. `duel_lethal_guard` sits in the player damage path: when
a blow between two duel opponents would finish the target, it is capped at 1 HP
and the duel ends with the striker as winner. So the loser is never killed, no
death penalty applies, no karma or PvP counters move, and both are restored to
full afterwards.

> **Restore is simplified.** Java snapshots each duelist's HP/MP/CP *before* the
> duel and restores exactly that, also stripping debuffs picked up during the
> fight. This slice restores both sides to full instead of snapshotting —
> visible only if you entered a duel wounded (`canDuel` already requires ≥50 %
> of both bars, so the gap is small). Marked `TODO(G20)`.

## 4. Tests

`game_loop/tests/duel_tests.rs`, 11 cases: the challenge reaching the target;
declining ending it; accepting starting the countdown and marking both sides;
the `canDuel` gates (below-half HP, already dueling, out of range) each with
their own refusal message; the countdown running down and firing `ExDuelStart`
with the 120 s clock set; surrender handing over the win; **a losing blow not
killing** and leaving the loser restored; drifting apart cancelling with no
winner; and the end clearing both markers so they can duel again.

## 5. What G20 still owes

With duels landed, G20's named features are done. The remainder is small:
`SHOTS_BONUS` (a G14 micro-gap — only `reducedSoulshot` weapons), karma decay
while hunting (`calculateKarmaLost` needs a per-level `KarmaData` table absent
from this dist), PK item drops (`Config.KARMA_DROP*`), the party-duel variant
above, and the ranged trio from slice 1 (bow peace-zone check, `CHEAPSHOT`,
NPC-archer reuse timing).
