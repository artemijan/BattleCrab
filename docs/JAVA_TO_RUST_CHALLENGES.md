# Java → Rust Migration Challenges: interlude_classic

Analysis of `interlude_classic` (L2J Mobius, Java 22, ~3,400 core source files +
1,131 runtime-compiled script files) for a 1:1 rewrite in Rust.

Goal of this document: enumerate every place where the Java codebase relies on a
concept that has **no direct Rust equivalent**, quantify how widespread it is, and
list candidate Rust approaches. Phase 2 = pick one approach per challenge.

Challenges are ordered by how much they will shape the Rust architecture.
The first three are the ones that make a literal 1:1 translation impossible;
everything else is mechanical or solvable locally.

---

## 1. Deep implementation inheritance (THE core problem)

### What the Java code does

The entire game model is one inheritance tree with behavior overridden at every level:

```
WorldObject (extends ListenersContainer)
├── Item
└── Creature                    (abstract, 5,984 lines)
    ├── Door
    ├── Npc
    │   ├── Folk
    │   └── Attackable
    │       ├── Guard
    │       └── Monster (→ RaidBoss, GrandBoss, ...)
    └── Playable                (abstract)
        ├── Player              (14,085 lines!)
        └── Summon
            ├── Pet
            └── Servitor
```

Files: `gameserver/model/WorldObject.java`, `gameserver/model/actor/*.java`,
`gameserver/model/actor/instance/*.java`.

- **1,350** `class X extends Y` declarations project-wide, **66** abstract classes.
- Subclasses override protected methods (`onSpawn`, `doDie`, `calcStat`, `updateAbnormalEffect`, …)
  and call `super.method()` in the middle of their own logic ("call super" pattern).
- **203** `instanceof` checks plus the Mobius idiom `obj.isPlayer()` / `obj.asPlayer()`
  (downcasting) used throughout AI, skills, and packet code.
- Parallel hierarchies exist elsewhere: `CreatureAI → PlayableAI/AttackableAI/PlayerAI`,
  `ItemTemplate → Weapon/Armor/EtcItem`, `ClientPacket` (891 network packet classes),
  `Quest`/`AbstractScript`, effect handlers, `Inventory → PlayerInventory/PetInventory`.

### Why it's hard in Rust

Rust has **no implementation inheritance at all**. Traits give you interface
inheritance and default methods, but:

- No fields in traits — `Creature`'s ~200 fields can't be inherited by `Player`.
- No `super.doDie()` — there is no built-in way for an override to wrap the parent's implementation.
- No protected methods, no abstract classes.
- Downcasting (`instanceof` → `Any::downcast_ref`) is possible but unidiomatic and clumsy.

### Candidate Rust approaches

| Approach | Sketch | Pros | Cons |
|---|---|---|---|
| **A. Composition + delegation** | `struct Player { creature: Creature, ... }`, explicit method forwarding | Closest to 1:1; each Java class becomes one struct | `super.x()` calls become explicit; tons of boilerplate delegation (can be macro-generated) |
| **B. Enum of kinds** | `enum Actor { Player(PlayerData), Monster(MonsterData), ... }` + shared `Creature` struct | Idiomatic; `instanceof` becomes `match`; no dyn dispatch | Adding a kind touches many matches; "class per file" mapping is lost |
| **C. Trait objects** | `trait Creature: WorldObject { ... }`, store `Box<dyn Creature>` | Feels like Java interfaces | Can't share fields; downcasting painful; doesn't solve overriding |
| **D. ECS (specs/bevy_ecs/hecs)** | Entities + components (Position, Stats, PlayerData…) | Best long-term fit for a game server; kills challenge #2 too | Furthest from 1:1; big conceptual rewrite |

For a "as close to Java as possible" rewrite: **A + B hybrid** — shared base
structs embedded by composition, an enum for the kind/dispatch, and the
`isPlayer()/asPlayer()` helpers reimplemented on that enum. This preserves the
Java file/class layout almost exactly.

---

## 2. Garbage-collected shared mutable object graph

### What the Java code does

Java's GC lets every object hold plain references to every other object, mutably,
from any thread, with cycles everywhere:

- `World` (singleton) holds all `WorldObject`s; each `WorldObject` knows its `WorldRegion`;
  regions hold objects back → cycle.
- `Player ↔ Party ↔ Clan ↔ Siege ↔ Castle` — all mutually referencing.
- `Player._target` points to another `Creature`; that creature's `_attackByList`
  points back.
- AI objects hold their actor; the actor holds its AI.
- `ThreadPool.schedule(...)` closures capture live objects for minutes and mutate
  them later.
- Dead objects are simply dropped and the GC collects them (19 uses of
  `WeakReference`/`SoftReference` where they bothered).

### Why it's hard in Rust

This is the single biggest *daily* pain point of the port. Rust ownership forbids
shared mutable references; cycles leak under `Rc/Arc`; every cross-reference must
be answered with a deliberate choice.

### Candidate Rust approaches

| Approach | Sketch | Pros | Cons |
|---|---|---|---|
| **A. `Arc<RwLock<T>>` everywhere + `Weak` for back-edges** | Direct translation of Java references | Most 1:1; least redesign | Deadlock risk (Java `synchronized` is reentrant, Rust locks are NOT — see #5); runtime borrow cost; cycle leaks if `Weak` is missed |
| **B. ID-based indirection** | Objects own no references; store `object_id: u32` and look up via `World` registry | No cycles ever; matches how the DB and network already identify things (objectId) | Every `player.getTarget().getName()` becomes two lookups; lookups can fail (dead id) |
| **C. Arena/generational (slotmap, generational-arena)** | Central arenas keyed by generational indices | Fast, safe, no leaks; dead-id reuse detected | Same as B plus new dependency concepts |
| **D. ECS** | Same as 1.D | Solves it wholesale | Not 1:1 |

> **DECIDED: approach B/C — ID-based references, single-owner world.** All world
> state lives in a `World` struct owned by one game thread; cross-object
> references are `objectId` lookups (the game already has globally unique IDs —
> `IUniqueId`). No `Arc<RwLock>` in game logic. Full design:
> [CONCURRENCY_MODEL.md](CONCURRENCY_MODEL.md).
>
> **Amendment (post-G9): the registries' storage engine is an ECS** — a
> deliberately narrow slice of option D. `World.players`/`npcs` are
> `bevy_ecs`-backed (`store::EntityStore`: entity per object, the object as
> one fat component, `object_id → Entity` index), which buys archetype-table
> (dense, cache-friendly) iteration for the per-tick systems *without*
> abandoning 1:1 — references are still ids, handlers still see the same
> HashMap-shaped API, and the single-owner rule is untouched. See
> [CONCURRENCY_MODEL.md](CONCURRENCY_MODEL.md) §2.8.

---

## 3. Runtime-compiled Java scripts (1,131 files)

### What the Java code does

`gameserver/scripting/ScriptEngineManager.java` compiles
`dist/game/data/scripts/**/*.java` **at server startup using the JDK compiler**
(`JavaExecutionContext`), then reflectively instantiates them:

- `quests/` — hundreds of quest classes extending `Quest`
- `ai/` — NPC AI overrides
- `handlers/MasterHandler.java` — registers all item/skill/admin/voice handlers
- `instances/`, `events/`, `village_master/`, `custom/`, `vehicles/`

This gives L2J its "drop a .java file in, restart, it works" modding model.

### Why it's hard in Rust

Rust cannot compile and load code at runtime (no JIT, no classloader). This is a
**hard architectural break** — the scripts folder concept cannot survive as-is.

### Candidate Rust approaches

> **DECIDED: approach A — compile scripts into the binary.** The 1,131 script
> files become regular Rust source in the project tree (same layout:
> `scripts/quests/Q00001_...`, `scripts/ai/...`, `scripts/handlers/...`),
> compiled together with the server during the normal build. Runtime compilation
> is eliminated; registration happens at startup (explicit list or
> `inventory`/`linkme` link-time registration — exact mechanism to pick during
> implementation). Trade-off accepted: any quest/AI change requires rebuilding.
> To keep compile times sane, scripts should live in their own workspace crate
> (see #13).

| Approach | Sketch | Pros | Cons |
|---|---|---|---|
| **A. Compile scripts into the binary** ✅ | Each script becomes a module; a build-script or `inventory`/`linkme` crate auto-registers them | Simplest; full type safety; scripts are already "part of the repo" anyway | Any quest change = recompile server (Rust compile times on ~1,100 modules are real) |
| **B. Embedded scripting language (Lua/mlua, Rhai)** | Port quests to Lua/Rhai, expose a game API | Keeps hot-reload modding; quests are mostly simple event logic | Must translate 1,131 files to another language; FFI boundary design; loses 1:1 |
| **C. WASM plugins** | Scripts compiled to .wasm, loaded via wasmtime | Sandboxed, any language | Heavy machinery; API boundary very verbose for chatty game logic |
| **D. Dynamic libraries (dlopen)** | Scripts as cdylib crates | Native speed | ABI instability, unsafe, practically miserable |

---

## 4. Singletons and global mutable state

### What the Java code does

**176 classes** expose `getInstance()` singletons holding mutable state:
`World`, `Config` (thousands of mutable static fields), `SkillData`, `NpcData`,
all `instancemanager/*` (SiegeManager, CastleManager, …), `ThreadPool`, `GameTimeTaskManager` etc.
They freely call each other, including during initialization, and some are
lazily initialized via holder classes.

### Why it's hard in Rust

Global mutable state in Rust requires `static` + interior mutability + thread
safety proofs. `static mut` is effectively forbidden. Lazy cyclic init that JVM
classloading tolerates can deadlock or panic with `OnceLock`.

### Candidate approaches

- **A.** `OnceLock<T>` / `LazyLock<T>` statics with `RwLock` interior state — direct
  `getInstance()` translation, keeps call sites identical (`World::get_instance()`).
- **B.** One `GameContext` struct owning all managers, passed by `&` — idiomatic,
  testable, but changes every call site (not 1:1).
- **C.** Config as a `LazyLock<Config>` loaded once; live-reload (`//reload config`
  admin command exists) needs `RwLock` or `ArcSwap`.

For 1:1: **A**, with awareness of init-order (Java's classloader resolved order
implicitly; Rust needs an explicit startup sequence like `GameServer.java`'s
constructor already has).

---

## 5. Concurrency model: synchronized, volatile, ThreadPool

### What the Java code does

- `commons/threads/ThreadPool.java`: two global pools (scheduled + instant),
  everything from AI ticks to skill casts to autosaves is a scheduled `Runnable`
  capturing live objects.
- **275** `synchronized` blocks/methods, **48** `volatile` fields, plus
  `ConcurrentHashMap`, `CopyOnWriteArrayList`, `AtomicInteger` scattered through
  the model.
- Java monitors are **reentrant** and per-object (`synchronized(this)` on any object);
  code implicitly relies on being able to re-lock while holding the lock and on
  benign data races that the JVM memory model + GC make "safe enough".

### Why it's hard in Rust

- `std::sync::Mutex`/`RwLock` are **not reentrant** — a direct translation of
  nested `synchronized` calls (e.g. `doDie` → `onKill` → back into the same
  creature) deadlocks instantly.
- Rust will surface every data race Java silently tolerated: all shared state must
  be `Send + Sync`, which interacts with challenge #2's choice.
- Closures scheduled on a pool must own (`'static`) their captures → pushes toward
  ID-based access (challenge 2.B) rather than captured references.

### Candidate approaches

- **A. Threaded, like Java**: `rayon`/custom pool + `parking_lot` locks (cheaper,
  still non-reentrant), scheduled tasks via a timer wheel crate. Closest to 1:1.
- **B. Tokio async**: timers (`tokio::time`), tasks, async I/O in one runtime.
  Natural for the network layer, but async recursion/locking across the game model
  is its own can of worms.
- **C. Sharded/actor world**: world regions or a single game-loop thread own the
  mutable state (like most game servers); network threads only send messages.
  Eliminates most locking — but is a redesign, not a translation.

> **DECIDED: single game thread + service threads (variant of C).** Game logic
> runs on one dedicated thread in a 100 ms tick loop that owns all world state;
> tokio handles network I/O (read/decrypt/parse and serialize/encrypt/write on
> worker threads); dedicated threads/tasks for SQLite, pathfinding, and the
> login link. All 275 `synchronized` sites and the three Java thread pools are
> replaced by the single-owner invariant — no locks in game logic at all.
> The ~300 `ThreadPool.schedule` sites become a game-thread timer queue holding
> object IDs; the periodic task managers become tick systems at the same rates.
> Full design incl. analysis of the Java stack:
> [CONCURRENCY_MODEL.md](CONCURRENCY_MODEL.md).

---

## 6. Reflection and annotations

### What the Java code does (~24 reflection sites + annotation scanning)

- Event bus: `model/events/` — listeners registered by scanning methods annotated
  `@RegisterEvent`, `@RegisterType`, `@Id`, `@NpcLevelRange`… (`AbstractScript`,
  `EventDispatcher`, `ListenersContainer` — note `WorldObject` *extends*
  `ListenersContainer`, so every object is an event bus).
- `Class.forName` / `newInstance` for: script instantiation (#3), packet-to-class
  mapping, DB driver loading (`Config`), handler registration.
- Enum `valueOf` from XML/DB strings everywhere.

### Rust approaches

- Annotations → **attribute/derive macros** or explicit registration calls; the
  `inventory`/`linkme` crates give "register from anywhere" semantics at link time.
- Event bus itself maps fine: `EventType` enum + `HashMap<EventType, Vec<Box<dyn Fn>>>`.
- `Class.forName(packetClass)` → a match/lookup table (the packet opcode table
  already exists, so this is easy).
- Enum-from-string → `strum` derive.

Hard part is only the *ergonomics*: Java's "annotate and forget" becomes either a
macro to maintain or explicit registration lists.

---

## 7. Nullability → Option<T>

Java uses `null` pervasively: `getTarget()`, `getClan()`, `getParty()`,
`getActiveWeaponInstance()` all return null routinely, and calling code
null-checks (or doesn't, and NPEs are caught by broad try/catch).

In Rust everything becomes `Option<T>`, which is strictly better but means:
- Every field access chain (`player.getClan().getLeader().getName()`) becomes
  `?`-chains or nested `if let` — mechanical but touches nearly every line.
- The port will *expose* latent NPE bugs; decide upfront whether to replicate
  buggy behavior (silently skip) or fix (log + skip). For 1:1 behavior: match the
  surrounding Java `try/catch` semantics.

## 8. Exceptions → Result

- Java code uses unchecked exceptions and broad `catch (Exception e) { log }` as
  control flow, especially in packet handling ("bad packet? log and drop") and DB
  code.
- Rust: `Result` + `?` with an error enum (`anyhow`/`thiserror`), and the packet
  handler loop becomes `if let Err(e) = handle(...) { log }` — same semantics, but
  every fallible signature changes.
- **No panics for game logic** — a panic in Rust kills the thread/task; Java's
  equivalent (`RuntimeException`) was routinely swallowed. Panic = bug policy, with
  `catch_unwind` only at the packet/task boundary if we want Java-like resilience.

## 9. Database layer (JDBC → Rust)

> **DECIDED: SQLite only (for now).** MariaDB/PostgreSQL support is dropped from
> the initial port; the schema/SQL is written against SQLite. If multi-driver
> support returns later, `sqlx`'s `Any` driver covers all three original backends.

Java: JDBC with three drivers (MariaDB, PostgreSQL, SQLite — see `pom.xml`,
recent "DB drivers support" commit), a `commons/database` connection factory,
raw SQL strings with `PreparedStatement`, and DAO classes.

Rust: **sqlx** with the SQLite driver (or `rusqlite` if we prefer a simple
blocking API — a closer match to JDBC's style and a good fit since SQLite is
in-process anyway). Raw SQL strings port 1:1. Watch out for: Java's `ResultSet`
implicit type coercions, and connection-per-call patterns that need a pool.
SQLite-only also simplifies concurrency: one writer at a time (WAL mode), which
argues for a dedicated DB thread/queue rather than concurrent pool writes.

## 10. Swing GUI

> **DECIDED: no GUI.** The Rust servers run headless with console + log files
> (they run in Docker anyway). Not ported: `loginserver/ui/`, `gameserver/ui/`
> (SystemPanel), `commons/ui/` (DarkTheme, SplashScreen, LimitLinesDocumentListener),
> and the `tools/dbinstaller` GUI (replaced by a CLI/SQL-script flow if needed).

## 11. Third-party library mapping

| Java (pom.xml) | Used for | Rust equivalent |
|---|---|---|
| `exp4j` | Runtime math expressions in `Config.java`, `SkillData.java` | `evalexpr` / `meval` |
| `jsoup` | HTML parsing (community board / html cache) | `scraper` / manual — check actual usage depth |
| `slf4j-simple` + `java.util.logging` | Logging | `tracing` or `log` + `env_logger` |
| JDBC drivers | DB | `sqlx` (SQLite) or `rusqlite` — SQLite only, per decision in #9 |
| Java NIO (`commons/network`) | Login+Game TCP servers, Blowfish/XTEA crypt | `tokio` + `cipher`/`blowfish` crates |
| `javax.tools` compiler | Script compilation | eliminated (see #3) |
| Swing | GUI | dropped — decided, see #10 |
| `java.util.Timer`/pools | Scheduling | `tokio::time` / timer wheel |

XML data loading (`IXmlReader`, dozens of `data/xml/*.xml` parsers): `quick-xml`
(+ optional `serde`). HTML files sent to client port as plain strings.

## 12. Small-but-everywhere semantic differences

These won't block the architecture but will cause subtle behavior differences in
a 1:1 port if ignored:

- **Integer overflow**: Java wraps silently; Rust panics in debug, wraps in release.
  Damage/exp formulas that rely on wrap need `wrapping_*` or `i64` upgrades.
  Decide a project-wide rule.
- **Strings**: Java is UTF-16; the L2 protocol sends UTF-16LE strings. Rust `String`
  is UTF-8 — packet read/write must convert explicitly (`encode_utf16`).
- **`equals`/`hashCode` identity semantics**: Java code sometimes relies on
  reference identity in maps/sets (e.g. `WorldObject` keyed by identity). With
  ID-based design (#2.B) this becomes keying by `objectId` — usually what was meant.
- **HashMap iteration order** differs; any code depending on it (it shouldn't, but
  10-year-old game code…) may behave differently.
- **`double` math**: identical IEEE-754 semantics — formulas port safely. But
  `Math.random()` vs `rand` sequences differ (only matters for tests).
- **Static init order**: JVM lazy class-init resolved dependency order implicitly;
  Rust needs the explicit boot sequence (already mostly explicit in `GameServer.java`).
- **`System.exit()` / shutdown hooks** → explicit shutdown path + `ctrlc` crate.
- **Object monitors** (`wait/notify`) → `Condvar` or channels.

## 13. Scale and build reality

- ~575k+ lines of Java (3,400 files) + 1,131 script files. A faithful Rust port
  is plausibly 400–700k lines of Rust.
- One giant crate will have painful compile times → plan a **workspace** mirroring
  the Java packages: `commons`, `loginserver`, `gameserver`, `scripts` (and
  `tools/dbinstaller` probably dropped/CLI-fied).
- No tests exist in the Java project (no `src/test`) — behavioral parity must be
  verified by protocol-level testing (connect a client / packet replay). Worth
  deciding early how "1:1 correctness" will be checked.

---

## Summary: what actually blocks a literal 1:1

| # | Challenge | Severity | Must decide in phase 2 |
|---|---|---|---|
| 1 | Implementation inheritance (WorldObject tree, 1,350 extends) | 🔴 Architectural | composition+enum vs traits vs ECS |
| 2 | GC'd cyclic shared mutable graph | ✅ Decided | ID-based registry, single-owner `World`, ECS-backed storage (`bevy_ecs`) — [CONCURRENCY_MODEL.md](CONCURRENCY_MODEL.md) |
| 3 | Runtime-compiled scripts (1,131 files) | ✅ Decided | compile into the binary as normal Rust source (own workspace crate) |
| 5 | Reentrant locks, scheduled closures | ✅ Decided | single game thread + tokio network + service threads — [CONCURRENCY_MODEL.md](CONCURRENCY_MODEL.md) |
| 4 | 176 mutable singletons | 🟠 High | OnceLock statics vs context struct |
| 6 | Reflection/annotation registration | 🟡 Medium | macros vs explicit registration |
| 9 | JDBC 3-driver DB layer | ✅ Decided | SQLite only; sqlx or rusqlite (crate choice pending) |
| 10 | Swing GUIs | ✅ Decided | dropped — headless server |
| 7,8 | null → Option, exceptions → Result | 🟢 Mechanical | conventions only |
| 11,12 | Libraries & semantics | 🟢 Mechanical | crate choices, overflow rule |

Interdependency note: choices are coupled. Picking **2.B (ID-based registry)**
makes **5** far easier (scheduled tasks capture IDs, not objects) and softens
**1** (less need for shared base references). Picking **1.D/2.D (ECS)** solves
1+2+5 together but abandons the 1:1 goal. This coupling is the main thing to
settle in phase 2.
