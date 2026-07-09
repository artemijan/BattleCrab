# Concurrency Model — lineage2_rust

**Status: DECIDED (pending review).** This document defines the threading and
ownership model for the Rust rewrite and records the analysis of the Java
implementation it replaces. Linked from
[JAVA_TO_RUST_CHALLENGES.md](JAVA_TO_RUST_CHALLENGES.md) challenges **#2**
(object graph) and **#5** (concurrency).

**One-line summary:** many OS threads by use case, but exactly one thread — the
*game thread* — owns and mutates world state. Network, DB, and heavy computation
run on their own threads and talk to the game thread through queues.

---

## Part 1 — How the Java server actually works today

Verified against source; this is what we are replacing, so the Rust design must
account for every row here.

### 1.1 The network stack (Async-mmocore, `commons/network`)

The network core is JoeAlisson's *Async-mmocore* vendored into
`commons/network`, built on Java AIO (`AsynchronousChannelGroup`):

- **I/O threads:** one AIO channel group with a fixed pool of
  `max(2, cores − 2)` threads (`ConnectionConfig`, overridable in
  `config/Network.ini`). These threads run read/write *completions*.
- **Framing:** 2-byte little-endian length header, then payload
  (`HEADER_SIZE = 2`). `ReadHandler` runs a state machine: read 2-byte header →
  read payload → parse. One packet is read at a time per connection
  (`Client.isReading` / `_readNext` flags; `AutoReading` config).
- **Inbound path** (`ReadHandler.completed`, **on the AIO thread**):
  decrypt in place → `PacketHandler.handlePacket` maps opcode → packet object →
  `packet.read()` parses fields → hand off to the `PacketExecutor`.
- **The crucial wiring** (`GameServer.java:450`):
  ```java
  new ConnectionBuilder<>(addr, GameClient::new, new GamePacketHandler(), ThreadPool::execute)
  ```
  The packet executor is **`ThreadPool::execute`** — i.e. `packet.run()`
  (the actual game logic: attack, trade, enchant…) executes on the shared
  **instant pool**, concurrently across clients and even concurrently for one
  client. Correctness rests entirely on `synchronized` + concurrent collections.
- **Outbound path** (`Client.writePacket`): per-client
  `ConcurrentLinkedQueue<WritablePacket>` + CAS `_writing` flag. Serialization
  and encryption happen on *whatever thread sends* (game logic thread or AIO
  write-completion thread). A `FairnessController` round-robins ready clients so
  one busy client can't monopolize write capacity. Optional drop policy:
  if `DropPackets=true` and a client's queue exceeds `DropPacketThreshold`
  (250), packets flagged `canBeDropped()` are discarded.
- **Buffer pooling:** `ResourcePool` with size-classed `BufferPool`s (GC
  pressure optimization — mostly irrelevant in Rust).

### 1.2 Thread inventory of the running Java game server

| # | Threads | Source | What runs there |
|---|---|---|---|
| 1 | AIO group, `max(2, cores−2)` | `ConnectionHandler` | socket I/O, decrypt, packet parse, encrypt, serialize |
| 2 | Instant pool, `2 × cores` | `ThreadPool.INSTANT_POOL` | **client packet handlers (game logic!)**, `ThreadPool.execute` tasks |
| 3 | Scheduled pool, `4 × cores` | `ThreadPool.SCHEDULED_POOL` | ~300 `schedule*` call sites: skill cast completion, respawns, buff expiry, autosave… + most periodic task managers |
| 4 | High-priority scheduled pool, `scheduled/4` (min 2, thread priority 8) | `ThreadPool.HIGH_PRIORITY_SCHEDULED_POOL` | movement, AI think, attack ticks (see 1.3) |
| 5 | `GameTimeTaskManager` | dedicated `Thread`, 100 ms sleep loop | game-time ticks (10 ticks/s), day/night events |
| 6 | `LoginServerThread` | dedicated `Thread`, **blocking** `java.net.Socket` | game↔login link (auth handoff, kick, account status) |
| 7 | (login server) `GameServerThread` per registered GS | dedicated blocking thread | login side of the same link |
| 8 | Swing EDT + SystemPanel timer | `gameserver/ui` | GUI — **dropped by decision #10** |
| 9 | Shutdown hook | JVM | save-all on exit |

So Java "game logic" runs concurrently on pools #1–#4 simultaneously, guarded by
275 `synchronized` sites, `ConcurrentHashMap`s, and `volatile`s.

### 1.3 The task managers — Java is already (multi-threaded) tick-based

`gameserver/taskmanager/*` is the important discovery for our design. The
per-creature work is *not* event-driven; it's fixed-rate ticks over sharded
sets, scheduled on the pools:

| Task manager | Rate | Pool | Work |
|---|---|---|---|
| `MovementTaskManager` | 100 ms | high-prio | `updatePosition()` for pools of ≤1000 moving creatures |
| `AttackableThinkTaskManager` | 100 ms | high-prio | AI `onEvtThink()` for active monsters |
| `CreatureAttackTaskManager` | 100 ms | high-prio | attack hit/abort timing |
| `CreatureFollowTaskManager` | 500/1000 ms | high-prio | follow logic |
| `AutoPlay/AutoUse/AutoPotion` | 100–1000 ms | high-prio | auto farm/consumables |
| `AttackStanceTaskManager` | 1 s | high-prio | combat stance timeout |
| `PvpFlagTaskManager` | 1 s | high-prio | PvP flag decay |
| `DecayTaskManager`, `RespawnTaskManager`, `RandomAnimationTaskManager`, `CreatureSeeTaskManager`, `ItemMana/LifeTime/Appearance`, `PlayerAutoSaveTaskManager` | 1–10 s | scheduled | corpse decay, respawns, social anims, aggro-range "see", item timers, autosave |
| `BuyListTaskManager` | 50 ms / 60 s | scheduled | shop restock + persistence |
| `TaskManager` | cron-like | scheduled | daily/global tasks |

Each of these runnables iterates a `ConcurrentHashMap`-backed set while other
threads mutate the same creatures. **In Rust these all collapse into systems
called from the single game loop** — same rates, same order every tick, no
sharding needed.

### 1.4 One-shot scheduled tasks

~300 `ThreadPool.schedule(...)` call sites (296 in core + 22 script files):
skill cast completion, teleport finish, buff/debuff expiry, door
open/close, event phases, siege timers. Closures capture live objects
(`Player`, `Npc`) and mutate them when they fire, on a pool thread.

### 1.5 What the Java model does NOT guarantee (important!)

These are latent defects of the current design, not features to preserve:

- **No per-client packet ordering under load.** Packets are executed on the
  instant pool; two packets from the same client can run concurrently or out of
  order. (The read side is sequential, but execution is fire-and-forget.)
- **No inbound backpressure.** The instant pool queue is unbounded
  (`LinkedBlockingQueue`); a packet-flooding client grows the queue without limit
  (mitigated only by the read-one-at-a-time flow).
- **Races by design.** Creature sets are iterated while mutated; stats read
  without locks; "benign" races tolerated because the JVM makes them memory-safe.
- **Reentrant-lock dependency.** Nested `synchronized` on the same object is
  everywhere (`doDie` → listeners → back into the creature).

---

## Part 2 — The Rust design

### 2.1 Thread/task inventory

```
                    ┌───────────────────────────────────────────────┐
   client sockets   │ NETWORK RUNTIME (tokio, N worker threads)     │
  ◄────────────────►│ • per-connection tasks: read, decrypt, frame, │
                    │   parse → typed ClientPacket structs          │
                    │ • serialize + encrypt outbound, write         │
                    │ • connection accept/close, IP bans            │
                    └───────┬───────────────────────────▲───────────┘
                   inbound  │ mpsc<(ClientId, Packet)>  │ per-client out queues
                            ▼                           │
                    ┌───────────────────────────────────┴───────────┐
                    │ GAME THREAD (1 dedicated OS thread)           │
                    │ owns World: all objects, regions, managers    │
                    │ loop at 100 ms base tick:                     │
                    │   1. drain inbound packets → handlers         │
                    │   2. drain service results (DB/path/login)    │
                    │   3. fire due timers (one-shot scheduler)     │
                    │   4. run tick systems (movement, AI, …)       │
                    │   5. flush outbound / DB commands             │
                    └──┬──────────────┬──────────────┬──────────────┘
                       │              │              │
                       ▼              ▼              ▼
                ┌────────────┐ ┌─────────────┐ ┌──────────────────┐
                │ DB THREAD  │ │ PATH WORKERS│ │ LOGIN-LINK TASK  │
                │ SQLite,    │ │ geodata     │ │ (tokio) game↔login│
                │ WAL, single│ │ pathfinding │ │ TCP, blowfish     │
                │ writer,    │ │ (rayon or   │ └──────────────────┘
                │ cmd queue  │ │ fixed pool) │
                └────────────┘ └─────────────┘
```

Roughly: on an 8-core machine ⇒ 1 game thread + ~4 tokio workers + 1 DB thread
+ 2 path workers + misc. All cores usable; exactly one may touch `World`.

### 2.2 The game loop

Base tick = **100 ms**, matching Java's `GameTimeTaskManager` tick and the
high-priority task-manager rate. Slower Java rates (1 s, 5 s, …) become systems
that run every Nth tick.

```rust
loop {
    let tick_start = Instant::now();

    // 1. Client packets: bounded drain, per-client FIFO preserved
    while let Ok((client_id, packet)) = inbound.try_recv() {
        handle_packet(&mut world, client_id, packet); // full read/write World access
    }

    // 2. Results from services
    drain_db_results(&mut world);      // e.g. char loaded → enter world
    drain_path_results(&mut world);    // path found → creature follows it
    drain_login_link(&mut world);      // auth ok / kick requests

    // 3. One-shot timers (ThreadPool.schedule equivalents)
    world.scheduler.run_due(now);      // skill land, teleport finish, buff expiry…

    // 4. Fixed-rate systems (the Java task managers, same rates)
    movement::tick(&mut world);                    // every tick   (100 ms)
    ai_think::tick(&mut world);                    // every tick
    attack::tick(&mut world);                      // every tick
    if world.tick % 10 == 0 { attack_stance::tick(&mut world); }   // 1 s
    if world.tick % 10 == 0 { decay::tick(&mut world); }           // 1 s
    if world.tick % 10 == 0 { respawn::tick(&mut world); }         // 1 s
    // … remaining managers from table 1.3, each at its Java rate

    // 5. Flush: outbound packets already queued during handlers; DB commands
    flush_db_commands(&mut world);

    sleep_until(tick_start + TICK);    // + tick-overrun warning metric
}
```

**Scheduler:** a binary-heap timer keyed by tick, entries =
`(fire_at, ScheduledTask)` where `ScheduledTask` is an enum or boxed closure
capturing **object IDs, never references** — the 1:1 translation of every
`ThreadPool.schedule(() -> ...)` site. If the target ID is dead when the timer
fires, the task is a no-op (Java gets the same effect from `isDead()`/null
checks inside the runnable).

**World access rule:** handlers and systems receive `&mut World` (or a context
struct wrapping it). Cross-object access = lookup by `objectId` in `World`'s
registries (decision for challenge #2: **ID-based, option B/C**). No `Arc`, no
locks, no `Weak` anywhere in game logic.

### 2.3 Network runtime (replaces Async-mmocore)

tokio multi-threaded runtime, N = `max(2, cores/2)` workers (tunable, mirrors
`Network.ini ThreadPoolSize`).

Per connection, an async task:

- **Read:** 2-byte LE length header → payload (same framing), decrypt
  (login: Blowfish; game: L2 XOR cipher — port of `commons/crypt`), parse into a
  **typed packet struct** (the 891 packet classes become enum variants /
  structs), then `inbound.send((client_id, packet))`.
- Parsing/validation failures are handled like Java (`ReadHandler` swallows) —
  log + drop packet, disconnect on framing corruption.
- **Write:** each client gets a bounded outbound queue
  (`mpsc<Bytes>` or `mpsc<ServerPacket>`); the connection task serializes,
  encrypts, and writes. Java's `FairnessController` comes for free — tokio
  schedules connection tasks fairly.
- **Drop policy:** replicate `DropPackets/DropPacketThreshold`: when a client's
  outbound queue is over the threshold, packets marked droppable (broadcast
  movement/social) are discarded; essential packets (inventory, HP) never.
- **Disconnect** = message to game thread (`ClientEvent::Disconnected`), which
  runs the `onDisconnection` logic (store player, remove from world) *on the
  game thread* — in Java this runs on an AIO thread and is a known race spot.

Where serialization happens is a deliberate change: Java serializes on the
sending thread and encrypts inside the client lock; we serialize on the network
side from packet values. Game thread produces cheap packet values; CPU-heavy
byte work is parallel per connection.

### 2.4 Services

- **DB thread (SQLite):** one dedicated thread owning the connection (WAL
  mode). Input: `DbCommand` queue (fire-and-forget writes: item updates, char
  saves) and `DbRequest`s with reply channels (loads during login). Game thread
  **never blocks on DB** — Java's synchronous JDBC-in-handler calls
  become request → next-tick result. This is the biggest *behavioral* port
  hazard: Java code that reads DB mid-handler must be split into
  request/continue (fortunately most gameplay-path DB work is already
  fire-and-forget saves or done at login).
- **Pathfinding/geodata workers:** small fixed pool (or rayon). Same
  request/reply pattern. Simple LOS checks against immutable geodata can stay
  synchronous on the game thread (read-only shared `Arc<GeoData>`).
- **Login↔Game link:** one tokio task replacing the blocking
  `LoginServerThread`; protocol (Blowfish, `SendablePacket` framing) ports as-is.
  On the login server, `GameServerThread`-per-GS likewise becomes tasks.
- **Game time:** no dedicated thread — `world.tick` counter (10 ticks/s) *is*
  `GameTimeTaskManager`; day/night transitions are a tick system.
- **Shutdown:** `ctrlc` handler sends `Shutdown` message; game loop finishes the
  tick, flushes saves through the DB thread, joins it, exits (replaces the JVM
  shutdown hook + `Shutdown` class).

### 2.5 Java construct → Rust construct

| Java | Rust |
|---|---|
| AIO group threads + `ReadHandler`/`WriteHandler` | tokio connection tasks |
| `PacketExecutor = ThreadPool::execute` (logic on instant pool) | inbound mpsc → handled on game thread |
| `Client._packetsToWrite` + `FairnessController` + drop threshold | bounded per-client outbound mpsc + droppable flag |
| `ThreadPool.schedule(runnable, delay)` (~300 sites) | `world.scheduler.schedule(tick + n, Task::X { object_id })` |
| `scheduleAtFixedRate` task managers (table 1.3) | tick systems at the same rates |
| High-priority pool / thread priority 8 | unnecessary — systems run first in the loop by construction |
| `GameTimeTaskManager` thread | `world.tick` counter + day/night system |
| `synchronized` / `volatile` / `ConcurrentHashMap` in game model | deleted — single-owner `&mut World` |
| `LoginServerThread` (blocking socket) | tokio task + channel to game thread |
| JDBC calls inside handlers | `DbCommand`/`DbRequest` to DB thread |
| Buffer `ResourcePool` | `bytes::BytesMut` reuse; only if profiling demands |
| Shutdown hook | ctrl-c handler → drain-and-save in loop |
| Swing UI threads | dropped (decision #10) |

### 2.6 Rules (enforced by structure, checked in review)

1. **Nothing on the game thread may block.** No file/DB/socket syscalls, no
   `Mutex::lock` on anything shared with other threads, no pathfinding. The type
   system helps: handlers get `&mut World` and channel senders — blocking APIs
   simply aren't passed in.
2. **All cross-thread channels are bounded.** Inbound per-client budget
   (disconnect flooders — *improvement over Java's unbounded queue*); outbound
   per-client (drop policy); DB queue (backpressure alarm, never drop writes).
3. **Timers and tasks capture IDs, not objects.** Dead ID ⇒ no-op.
4. **Tick budget is a metric.** Warn when a tick exceeds e.g. 50 ms; that is the
   failure mode of this architecture and must be visible from day one.

### 2.7 Behavioral differences vs. Java (intentional)

- **Per-client packet ordering is now guaranteed** (Java: unordered under load).
  Strictly more correct; no client depends on unordered handling.
- **Tick alignment:** Java's many independent 100 ms schedules fire with random
  phase; ours fire in a fixed order within one tick. Eliminates a class of
  races; ordering within the tick (packets → timers → movement → AI → attack)
  is now *defined* and must be chosen once (proposed order above mirrors
  Java's priority pool semantics).
- **Mid-handler DB reads become async** (split into request/continue) — the
  only place the port requires restructuring handler logic rather than
  translating it.
- **Latency profile:** a packet arriving right after a tick's drain waits up to
  one tick (≤100 ms) before handling. Java handled it "immediately" (pool
  permitting) — but 100 ms is already the game's own action granularity, and the
  client interpolates; this matches how retail-era servers behaved. If it ever
  matters, the drain step can be woken early by the channel instead of sleeping.

### 2.8 Scaling path (explicitly out of scope for v1)

If one core for logic proves insufficient: shard `World` by region/instance
into N tick loops with cross-shard messages (towns, hunting zones, and
instances partition naturally). The single-owner invariant is unchanged, so
this is an evolution, not a rewrite. Do not build it until a profiler asks.

---

## Open questions for review

1. **Tick-internal system order** — proposal in 2.2/2.7; needs a decision, then
   it's frozen (changing it later changes game behavior subtly).
2. **Early wake on inbound packets** vs. strict 100 ms cadence (see 2.7
   latency note). Proposal: strict cadence first, measure.
3. **Login server:** it has no game world — proposal: pure tokio app, no game
   thread at all (its "world" is just account sessions behind one task).
4. **Handler async splits:** inventory the handlers that do synchronous DB reads
   mid-logic (login/enter-world, character create/delete, clan/name checks) so
   the request/continue points are known before porting them.
