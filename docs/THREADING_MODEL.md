# Threading model

**The rule, in one sentence:** many threads by use case, but exactly one — the
*game thread* — owns and mutates world state; everything else talks to it over
channels.

That single sentence is why there is no `Arc<Mutex<Player>>` anywhere in this
codebase, why game logic reads like straight-line code, and why the port could
translate Java handlers one-for-one without inheriting Java's 275 `synchronized`
blocks.

This document describes the model **as built**. For the concept-level analysis
of the Java implementation it replaces, see
[CONCURRENCY_MODEL.md](CONCURRENCY_MODEL.md); for the object-graph decisions it
rests on, [JAVA_TO_RUST_CHALLENGES.md](JAVA_TO_RUST_CHALLENGES.md).

---

## 1. Topology

```mermaid
flowchart TB
    subgraph net["tokio runtime — N worker threads"]
        conn["per-connection tasks<br/>read · decrypt · frame · parse<br/>serialize · encrypt · write"]
        link["login-link task<br/>game ↔ login TCP, Blowfish"]
    end

    subgraph game["GAME THREAD — 1 dedicated OS thread"]
        world[("owns World<br/>ECS objects · regions · managers<br/>scheduler · client table")]
        tickloop["100 ms tick loop"]
    end

    subgraph svc["service threads"]
        db["DB THREAD<br/>SQLite, WAL<br/>single writer"]
        path["PATH WORKER<br/>CellPathFinding<br/>over shared read-only geodata"]
    end

    clients(("game<br/>clients")) <-->|"TCP"| conn
    conn -->|"GameEvent::Net"| tickloop
    tickloop -->|"per-client outbound queue<br/>(drop policy past threshold)"| conn
    link -->|"GameEvent::Login"| tickloop
    tickloop -->|"unbounded mpsc"| link
    tickloop -->|"mpsc&lt;DbCommand&gt;"| db
    db -->|"GameEvent::Db"| tickloop
    tickloop -->|"mpsc&lt;PathRequest&gt;"| path
    path -->|"GameEvent::Path"| tickloop
    tickloop --- world
```

On an 8-core machine that is 1 game thread + tokio workers + 1 DB thread +
1 path worker. Every core is usable; exactly one may touch `World`.

The four service→game arrows are one physical channel: every service sends a
`GameEvent` variant into a single `std::sync::mpsc` (`crate::events`), through
a typed per-service sender facade so a service cannot send another's events.
One channel because the game thread *sleeps on it* — see §2.

**Nothing in that picture shares game state.** The only objects crossing a
thread boundary by reference are the geodata (`Arc<GeoEngine>`), read-only
after boot, and two per-connection atomics that carry flow-control counts, not
game state: the outbound queue-depth estimate and the inbound in-flight
permits (§4 rule 3).

Boot order and the channel wiring are in `crates/gameserver/src/main.rs`; the
loop itself is `game_loop/mod.rs`.

---

## 2. The tick

Base tick is **100 ms** (`game_loop::TICK`), chosen to match Java's
`GameTimeTaskManager` and its high-priority task-manager rate. Slower Java rates
become `world.tick % N == 0` gates, so a "1 s" Java task manager is a system
that runs every 10th tick — same rate, but now at a *defined* phase instead of
whenever its pool thread happened to fire.

```
   ┌─ tick ──────────────────────────────────────────────────────────────┐
   │                                                                     │
   │  1. events        packets, login-link, DB results, path replies —   │
   │                   handled the moment they arrive. This IS the tick  │
   │                   sleep: the thread blocks in recv_timeout on the   │
   │                   unified channel until the tick deadline           │
   │  2. fire due timers   one-shot scheduler (Java ThreadPool.schedule) │
   │  3. tick systems      in fixed order, see table below               │
   │  4. flush             outbound packets, DB commands                 │
   │                                                                     │
   │  busy time > 50 ms → warn names the 3 slowest steps;                │
   │  tick_busy_micros gauges every tick's busy time                     │
   └─ next deadline = max(deadline + 100 ms, now) ───────────────────────┘
```

Because the sleep is a blocking receive rather than a clock wait, a packet is
handled within microseconds of arrival — input latency is bounded by handler
cost, not by the tick. Timers and systems still run strictly at the 100 ms
boundary, so system ordering and rates are exactly as before. An overrun tick
slides the phase (`max(deadline + TICK, now)`) rather than running
back-to-back catch-up ticks — the same policy as the old skip-the-sleep loop.

| System | Every | Wall clock |
|---|---:|---|
| movement + region-switch visibility | 1 tick | 100 ms |
| player combat (chase + swing) | 1 tick | 100 ms |
| drowning | 1 tick | 100 ms |
| auto-play / auto-use | 3 ticks | 300 ms |
| NPC AI think, combat stance, PvP flag decay | 10 ticks | 1 s |
| effect & damage zones | 10 ticks | 1 s |
| walker routes | 10 ticks | 1 s |
| auto-potions | 10 ticks | 1 s |
| autosave check, teleport watchdog | 10 ticks | 1 s |
| HP/MP/CP regen, weight sweep | 30 ticks | 3 s |

**A rate is not the whole story.** Java's 1 s `AttackableThinkTaskManager` does
not mean a monster waits a second to react — `onEvtThink()` is *also* called
directly off the fast paths (`onEvtArrived` and friends, at 100 ms). Porting the
fixed-rate sweep and dropping those event edges is precisely what makes a ported
mob feel sluggish: it closes on its target, then stands there for up to a second
before swinging. When a system here is the port of a slow Java task manager,
check `CtrlEvent`/`notifyEvent` for the edges that re-enter it early.

**One-shot timers** are a binary heap keyed by tick. Every Java
`ThreadPool.schedule(() -> ...)` site — skill cast completion, teleport finish,
buff expiry, door open/close, siege phases — becomes an entry capturing **object
ids, never references**. If the target is gone when the timer fires, the task is
a no-op, which is the same net effect Java gets from its `isDead()`/null checks
inside the runnable.

---

## 3. Why this design

### What it replaces

Java runs game logic concurrently on four pools at once. The wiring that decides
this is a single argument in `GameServer.java`:

```java
new ConnectionBuilder<>(addr, GameClient::new, new GamePacketHandler(), ThreadPool::execute)
```

The packet executor is `ThreadPool::execute`, so `packet.run()` — attack, trade,
enchant — executes on a shared pool, concurrently across clients *and
concurrently for the same client*. Correctness rests entirely on 275
`synchronized` sites, `ConcurrentHashMap`s and `volatile`s. What that model does
**not** give you, and which are defects rather than features to preserve:

- **No per-client packet ordering under load.** Two packets from one client can
  execute out of order.
- **No inbound backpressure.** The instant pool's queue is unbounded. *(This
  port bounds it — see rule 3.)*
- **Races by design.** Creature sets are iterated while being mutated; stats are
  read without locks. Tolerated because the JVM keeps them memory-safe.

### Why single-owner instead of fine-grained locking

Rust will not let you have Java's benign races. The two honest options were a
lock-per-object graph (`Arc<Mutex<…>>`, or an actor per creature) and a single
owner. Single owner won on three counts:

1. **The port stays a port.** A Java handler that touches attacker, target,
   party, and the region's known list in one method becomes one Rust function
   that does the same. With per-object locks it becomes a lock-ordering problem
   — and Java's nested `synchronized` (`doDie` → listeners → back into the
   creature) would have become deadlocks needing per-site redesign.
2. **Deadlock and data races stop being possible**, rather than being made
   unlikely. There is no lock to take in the wrong order.
3. **The workload fits.** This is a ~100 ms-granularity game with tens of
   thousands of mostly idle NPCs, not a physics engine. One core is enough for
   logic; the expensive work — crypto, serialization, pathfinding, SQLite — is
   already off the game thread.

### What "off the game thread" buys

Per-connection tasks do the CPU-heavy byte work (decrypt, parse, serialize,
encrypt) in parallel. The game thread produces cheap packet *values*. Java
serializes on whichever thread sends and encrypts inside the client lock; moving
that out is a deliberate change, not an accident of the port.

---

## 4. Rules

These are enforced by structure and checked in review.

1. **Nothing on the game thread may block.** No file, DB or socket syscalls, no
   pathfinding, no `Mutex::lock` on anything shared. The type system helps:
   handlers receive `&mut World` and channel senders, so blocking APIs are
   simply not in scope.
2. **Timers and queued tasks capture ids, not objects.** A dead id is a no-op.
3. **Backpressure exists in both directions; keep it that way.** The channels
   are still *unbounded types* (the game thread must never block on a send),
   but both directions are pressure-managed:
   - **Outbound** — `OutboundTx` tracks a per-connection queue-depth estimate;
     past `Network.ini`'s `DropPacketThreshold` (with `DropPackets` on, as the
     dist sets) the `canBeDropped` packet types — `StatusUpdate`,
     `AutoAttackStart/Stop`, `SocialAction`, `MoveToPawn`, `MoveToLocation` —
     are discarded and counted (`packets_dropped` metric). This is Java's
     `Client.packetCanBeDropped`, ported. State-bearing packets always queue.
   - **Inbound** — each connection holds `MAX_PACKETS_IN_FLIGHT` (256)
     semaphore permits; a forwarded packet carries its permit inside the
     event and the game thread releases it by dropping the handled event. At
     the cap the connection task simply stops reading the socket, so TCP flow
     control pushes back to the client. Java has no equivalent; combined with
     `Security.ini`'s per-second rate limit, a flooder costs bounded memory
     even across a game-thread stall.
4. **Tick budget is a metric, not a hope.** A tick over 50 ms warns with its
   number. Tick overrun is *the* failure mode of this architecture, so it has to
   be visible from day one.
5. **A panic must not outlive its packet, and a dead game thread must not
   outlive its process.** Each inbound packet is handled inside `catch_unwind`
   (Java parity: `ExecuteThread` catches `Throwable` per packet); the offending
   client is disconnected, because its handler may have died mid-mutation and
   that session's state is suspect, and the world lives on for everyone else.
   If a panic escapes anyway — a tick system, a timer — `main` notices the game
   thread exiting without a shutdown request and exits nonzero, so systemd's
   `Restart=on-failure` brings the server back rather than leaving a live
   listener in front of a dead game loop.

   > This is why `panic = "abort"` is **not** set in the release profile. It
   > would turn a single bad packet into a process kill.

---

## 5. Trade-offs

Honest ledger. The wins are structural; the remaining costs are real, and the
ones that could be engineered away have been — see the retired list at the end
of this section.

### What this buys

| | |
|---|---|
| **No locks in game logic** | No deadlocks, no lock ordering, no `Arc<Mutex<>>` noise. Handlers read like the Java they came from. |
| **Deterministic ordering** | Systems run in the same order every tick. A whole class of Java races cannot occur, and reproducing a bug does not depend on thread interleaving. |
| **Per-client packet ordering guaranteed** | Strictly more correct than Java, which loses ordering under load. |
| **Tests are simple and fast** | A test builds a `World`, calls the system, asserts. No async runtime, no synchronization, no flakiness — which is how the suite reaches ~2,970 tests. |
| **Parallelism where it pays** | Crypto, serialization, pathfinding and SQLite all run off the game thread, on all cores. |

### What it costs

| | |
|---|---|
| **One core for logic** | The ceiling is a single thread's throughput. It is now *measured* rather than guessed — `tick_busy_micros` gauges every tick's busy time against the 100 ms budget, and an overrun warning names the slowest steps — and the heavy work is already elsewhere; but it is a real ceiling, and the scaling path in §6 is not built. |
| **Mid-handler DB reads had to be restructured** | Java calls JDBC inline in a handler; here that would block the world. Those sites split into request → continue-on-reply. This is **the one place** the port had to restructure logic rather than translate it, and it is a genuine porting hazard: the Java code reads as if the value is available now. (The *latency* of the split is no longer tick-quantized — the DB reply wakes the sleeping loop, so the continuation runs the moment the row arrives.) |
| **A slow system stalls everything** | There is no preemption. One pathological loop delays every player, not just the one who triggered it. The per-step timings say *which* system it was, but cannot make it cheaper. |

### Costs retired

Rows this table used to carry, and what removed them:

| | |
|---|---|
| ~~Up to one tick of added latency~~ | The tick sleep became a blocking receive on the unified event channel (§2): a packet, DB row or path reply is handled the moment it arrives instead of waiting out the remainder of the 100 ms. |
| ~~No backpressure yet~~ | Both directions are pressure-managed (rule 3): Java's outbound `DropPackets` policy is ported, and the inbound in-flight permit cap — which Java itself lacks — bounds a flooder's memory. |
| ~~`bevy_ecs` parallelism paid for but unused~~ | Compiled out: `default-features = false, features = ["std"]` drops the `async_executor` task pool, `bevy_reflect` and backtrace machinery from the build entirely. |

---

## 6. Scaling path (not built)

If one core for logic ever proves insufficient: shard `World` by region or
instance into N tick loops that exchange messages, since towns, hunting zones
and instances partition naturally. The single-owner invariant is unchanged
inside each shard, so this is an evolution rather than a rewrite.

**Do not build it until a profiler asks.** The tick-overrun warning and the
`tick_busy_micros` headroom gauge are the triggers to watch.

---

## 7. Object storage: ECS

`World`'s object registries are an **ECS** (Entity–Component–System) built on the
standalone [`bevy_ecs`](https://crates.io/crates/bevy_ecs) crate — no other part
of Bevy is used, and the crate itself is compiled `std`-only (no parallel
executor, no reflection; §5 "costs retired").

Instead of each game object being one large struct in a map, an object is an
*entity* (an id) whose data lives in *components*. Components of the same shape
are stored together in contiguous archetype tables, so per-tick sweeps become
dense linear scans instead of pointer-chasing a `HashMap`. It also replaces
Java's `WorldObject → Creature → Playable → Player` inheritance with
composition: new behaviour attaches a component rather than growing a hierarchy.

What matters for *this* document: **the ECS changes nothing about the threading
model.** The `bevy_ecs::World` lives inside our `World` alongside its sibling
services, owned and mutated by the game thread alone; systems remain plain
functions called in the §2 order. Presence-based components (`Movement`,
`Casting`, `Intent`) exist only while that state is active, so a sweep visits
exactly the movers and casters rather than 34.9k idle NPCs.

Details of the component split are in
[CONCURRENCY_MODEL.md](CONCURRENCY_MODEL.md) §2.8 and
`crates/gameserver/src/store.rs`.
