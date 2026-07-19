# G21 slice 10 — NPC walking routes

Tenth G21 slice, and the last item in the milestone with live content. Town
NPCs now walk their circuits instead of standing still.

## Content

13 routes in `Routes.xml`, attached to 14 NPC ids, spawned from
`TownNpcWalkers.xml`: Giran's porters (Remy, Tate), scribes and tetrarch
agents, the running boy, and Gordon — a raid boss on a **67-node** patrol.

Only two `repeatStyle`s occur here: **`cycle`** (walk to the last node, head
straight back to the first) and **`back`** (walk to the last node, then retrace).
`conveyor` (teleport home) and `random` are parsed for shape but never selected
by this datapack.

One of the 14 attachments, `FPC_Giran_Evi` → npc 80000, has no NPC template —
it's a fake-player route. It parses and simply never attaches to anything.

## What landed

- `data/route_data.rs` — the `Routes.xml` loader (`WalkingManager`'s), with
  nodes carrying `delay`, `run` and an optional `string` to shout on arrival.
- `game_loop/walkers.rs` — `WalkState` on the NPC plus a one-second sweep.

**Shape difference from Java, deliberate.** Java hangs a `ScheduledFuture` off
each arrival and keeps per-NPC `WalkInfo` in a manager map. The port keeps the
state on the NPC as a component and drives it from a sweep at the same cadence
the rest of the NPC AI already uses. The state machine is the two phases Java's
arrive-task implies:

- **Travelling** — a `Movement` is in flight; when it disappears the NPC has
  arrived, so bank that node's `delay` and switch to waiting.
- **Waiting** — once the delay elapses, advance a node and walk.

Splitting it this way matters: setting the delay *before* the leg starts would
let travel time eat the pause, so a node with a 10 s wait would be skipped
whenever the walk took longer than 10 s.

## The `back` arithmetic looks wrong and isn't

Java's `calculateNextNode` steps back **two** on overrunning the last node
(`_currentNode -= 2`), because the index was already incremented past the end.
The result lands on the second-to-last node — the first step of the return leg.
The test pins the exact sequence for a 3-node route: `0 → 1 → 2 → 1 → 0 → 1 → 2`.
Getting this off by one would make a walker bounce on the spot at each end.

## Tests

9 in `game_loop/tests/walker_tests.rs`: route attached on spawn; an NPC with no
route gets no state; the first leg heads for node 1; the exact node sequence for
**both** repeat styles; a non-repeating route drops its state at the last node;
a node `delay` holds the walker in place and releases it; a dead walker stops
permanently (`WalkingManager.onDeath`); plus dist-backed parse assertions
(13 routes, Porter Remy's 18-node cycle, Leandro's `back` style, and that no
other style occurs).

## A verification gap I hit — and closed

`cargo build --tests` surfaced that **`tests/user_info_packet.rs` no longer
compiled**: the previous slice added a field to `Speeds` and I'd only ever run
`--lib`, `char_persistence` and `e2e_create`. That target was broken on `main`.

Fixed here, and the habit changed: this slice was verified with a plain
`cargo test -p gameserver`, which runs **all 8 targets** — 749 tests — rather
than the three I'd been running.

**749 tests green across all targets.** The one remaining build warning
(`is_in_duel`) predates this work.

## Deliberate narrowings (`TODO(G21)` at the site)

- `run` per node is parsed but not applied — NPC walk/run speed selection is a
  separate switch the AI owns.
- Node `string`/`npcString` chat on arrival is parsed but not broadcast.
- `conveyor` behaves as `cycle` (no NPC teleport plumbing); no route on this
  dist uses it.

## G21 status

**Complete for practical purposes.** The gate was met at slice 3, and every
remaining item is blocked or empty on this dist:

- **`HtmCache`** — 2629 `.htm` files, already read at runtime. Caching only.
- **`CreatureSeeTaskManager`** — a trigger for AI *scripts*; no script engine.
- **`FenceData`** — one fence, named `"demo"`.
