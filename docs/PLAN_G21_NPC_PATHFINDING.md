# G21 slice 7 — NPC pathfinding

Seventh G21 slice. Mobs now consult geodata when they move: a chase around a
wall goes *around* it instead of through it.

## What was missing

`Creature.moveToLocation` in Java is shared between players and NPCs — the
geodata clamp and the pathfinding hand-off apply to both. The port had ported
only the player half (G7.85). `move_npc_to` built a straight-line `MoveData`
with `geo_path: None` and no geodata consultation at all, so every chase,
drift-return and random walk ignored terrain completely.

The path worker was *already* built for this: `PathRequest.playable` is
documented as "full postfilter for players, one pass for AI" and had never been
called with `false`.

## What landed

`move_npc_to` gained the NPC half of `moveToLocation`:

1. **Destination clamp** via `get_valid_location`, subject to Java's two skips
   (distance > 3000, and intentional falls: `curZ - z > 300` with distance
   < 300).
2. **The NPC takes the geodata-corrected z.** Java: `if (!isPlayer()) z =
   destiny.getZ()` — a player keeps the z its client asked for, a mob does not.
   Tested by aiming a mob at a z 5000 units in the air.
3. **A clamp shortfall > 30 units hands off to the path worker** with
   `playable: false`, against the *original* destination, and the move starts
   when the reply lands.

## Making the reply path NPC-safe

`handle_path_result` and `start_move` were player-only: they looked up
`world.clients[client_id]` to send `ActionFailed` and the mover's own
`MoveToLocation` copy. An NPC has no client, so every client-facing send is now
gated on `has_component::<Player>` rather than on the `client_id` value — a
sentinel id could otherwise collide with a real client. `broadcast_to_others`
already keyed off the source object's `RegionCell`, so onlookers were fine.

## Two things that would bite without care

- **Request flooding.** The AI re-issues a chase every 1 s think. Without a
  guard, a mob stuck behind a wall would queue a fresh path request every
  second forever. One outstanding request per mob (`PathWait`), tested.
- **Permanent paralysis.** That guard is only safe because the worker replies
  to *every* request (`path: None` when no route exists) and
  `handle_path_result` clears `PathWait` **before** the no-route branch
  returns. I checked both before relying on it, and there's a test asserting
  the wait clears on a failed route — otherwise a single unroutable target
  would freeze that mob for the rest of its life.

## Tests

9 in `game_loop/tests/npc_path_tests.rs`, against **real dist geodata** rather
than a synthetic grid. I probed outward from Giran town square to find genuinely
blocked and genuinely clear lines first: `+600` on x is fully blocked (the clamp
eats all 600 units), `+600` on y is open ground. Covered: clear line moves
straight with no request; blocked line queues exactly one request carrying the
*original* destination and `playable: false`, and starts no move meanwhile; a
second think doesn't duplicate; the reply starts a route move for an NPC; a
no-route reply leaves the mob still *and clears the wait*; a reply for a mob
that died meanwhile is dropped; `PathFinding=0` falls back to the old straight
move; the corrected-z rule; and a rooted mob still refuses to move (the geodata
work sits after that gate).

Geodata is loaded once per module via a `OnceLock` — it's seconds to parse.

**703 lib tests green**, `char_persistence` 7/7, `e2e_create` 1/1 (×2).

Also cleaned up two warnings I introduced in the previous slice's regen code
(an unnecessary `mut` and a `drop()` on a reference, which does nothing). The
only remaining build warning, `is_in_duel`, predates this work.

## Deliberate narrowings (`TODO(G21)` at the site)

- Java's `isOnGeodataPath()` re-click short-circuit (ignore a re-target onto
  the same geo cell, abandon the route otherwise) is player-only here; an NPC
  re-issuing the same chase just hits the in-flight guard instead.
- No move-to-pawn offset: NPCs still path to the target's exact position rather
  than to contact range.

## Next in G21

- `DamageZone` (13 live) + `SwampZone` (2 live).
- Wire `skillTargetReconsider` (faction data landed in slice 2).
- Fences (`FenceData`), `HtmCache`, walker routes, `CreatureSeeTaskManager`.
