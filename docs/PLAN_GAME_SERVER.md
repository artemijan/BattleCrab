# Implementation Plan — Game Server

**Status: PROPOSED (awaiting approval).** Second implementation phase of the
Rust rewrite. Builds the game server on top of the `commons` crate produced in
the [login phase](PLAN_LOGIN_SERVER.md). Architecture per
[CONCURRENCY_MODEL.md](CONCURRENCY_MODEL.md); language-mapping decisions per
[JAVA_TO_RUST_CHALLENGES.md](JAVA_TO_RUST_CHALLENGES.md).

> This is a large port — the Java `gameserver` is ~343k LOC across 3,407 files
> (`Player.java` alone is 14k lines, 891 packet classes, 57 XML data loaders,
> 1,131 compiled scripts, plus geodata). The plan is therefore organised as a
> **vertical slice first** (G0–G4: get a real client walking around and
> chatting, proving the whole architecture end to end) **then breadth**
> (G5+: fill in systems one at a time, each verifiable against the running Java
> server). Full parity is many milestones; the ordering guarantees a working,
> testable server at every step rather than a big-bang integration at the end.

## 1. Goal and definition of done

A headless Rust game server that is a **drop-in replacement** for the Java one:

1. Uses the **existing `dist/` folder unchanged** — same `dist/game/config/*.ini`
   files (read verbatim by the ported `PropertiesParser`), same `dist/game/data`
   XML/geodata, same SQLite schema. No config or data edits required to run.
2. Interoperates with our Rust **login server** over the existing GS-link
   protocol (and, as a cross-check, with the Java login server) — a real
   Interlude client (rev `0x0106` / `0xc621`) logs in, selects/creates a
   character, enters the world, and plays.
3. Same wire protocol byte-for-byte, same observable behaviour, same DB tables.
   Excluded by prior decisions: Swing GUI (`gameserver/ui`, decision #10),
   MariaDB/Postgres (decision #9), `tools/` GUIs.

**Definition of "done" is per-milestone** (each has its own ✔ gate). "Feature
complete" = the parity checklist (§8) is green, mirroring
[LOGIN_SERVER_PARITY.md](LOGIN_SERVER_PARITY.md).

## 2. What the Java game server consists of (port inventory)

Measured from source:

| Area | Java scope | Notes |
|---|---|---|
| Boot / lifecycle | `GameServer.java` (495 lines, ~90 subsystem inits in order), `Shutdown`, `LoginServerThread` | boot order is the milestone spine (§4) |
| Network | 891 classes: 365 client packets, 494 server packets, 19 login-link, `GameClient`, `GamePacketHandler`, `Encryption` (XOR cipher), `ConnectionState` | framing reused from `commons`; cipher is new |
| Model | 821 classes under `model/` — `actor/` (Creature/Player/Npc/… + 53 instances), `item/`, `skill/`, `stats/`, `zone/`, `clan/`, `conditions/` (104), `holders/` (60), `events/` (144) | the core domain; `Player.java` = 14k lines |
| Static data | 57 `data/xml/*` loaders + 8 `data/sql/*` tables, feeding 2,704 XML files under `dist/game/data` | loaded incrementally per milestone |
| Managers | 47 `instancemanager/*` singletons | ported as owned `World` sub-structs / services |
| Task managers | 29 `taskmanager/*` (fixed-rate tick loops) | become tick systems (CONCURRENCY_MODEL §2.2) |
| Handlers | 35 `handler/*` dispatchers (action, item, skill, admin, chat, bypass…) | trait-object registries |
| Scripts | 1,131 `.java` under `dist/game/data/scripts` (quests, AI, instances, village masters, custom, master handler) | **compiled into the binary** (decision #3) |
| Geoengine | 21 classes + `dist/game/data/geodata/*.l2j` | LOS + pathfinding as a service (path workers) |
| AI | 18 `ai/*` (Creature/Attackable/Summon controllers) | tick systems + per-actor AI state |
| Enums / strings | 113 `enums/*`, `SystemMessageId` (16k lines), `NpcStringId` (35k lines) | mostly mechanical / codegen candidates |
| Commons | crypt(2), database(6), network(30), threads(5), util(15) — ~10k LOC | already ported (login) or reused |

Client connection state machine (`ConnectionState`):
`CONNECTED → AUTHENTICATED → ENTERING → IN_GAME` (+ `CLOSING`/`DISCONNECTED`).

## 3. Architecture recap (already decided — see the linked docs)

Nothing new is decided here; the plan just *applies* the existing decisions:

- **One game thread owns `World`** (all objects, regions, managers); network,
  DB, pathfinding, and login-link run on other threads and talk to it through
  bounded channels. 100 ms base tick loop: drain packets → drain service
  results → fire timers → run tick systems → flush outbound/DB.
  ([CONCURRENCY_MODEL §2](CONCURRENCY_MODEL.md))
- **ID-based object graph:** cross-object access is a lookup by `objectId` in
  `World` registries; no `Arc`/`Mutex`/`Weak` in game logic (challenge #2).
- **Inheritance → composition + enums:** the `WorldObject → Creature →
  Player/Npc/…` hierarchy becomes shared component structs + an actor enum with
  ID-dispatched behaviour (challenge #1).
- **Scripts compiled into the binary:** each of the 1,131 script classes becomes
  Rust, registered at startup (challenge #3) — no runtime script engine.
- **Singletons → fields of `World`** (or a `GameData` bundle it owns), not global
  statics (challenge #4).
- **Timers capture IDs, not references;** dead ID ⇒ no-op (CONCURRENCY_MODEL §2.2).

Reused from `commons`: framing, packet read/write buffers (LE ints, f64,
UTF-16LE strings), config `PropertiesParser`, SQLite layer, `Rnd`/hex/util. The
**only new network pieces** are the game XOR cipher, the game packet enums, and
the game-thread executor wiring.

### 3.1 Player & session lifecycle — the type-state pattern

A connected client moves through a fixed lifecycle, and the set of *valid*
actions is different in each stage. Java models this with the `ConnectionState`
enum (`CONNECTED → AUTHENTICATED → ENTERING → IN_GAME`) and gates every
`ClientPacket` by the states it's allowed in, checked **at runtime**. In Rust we
encode the same lifecycle with the **type-state pattern** so that "which methods
exist" follows from "which state you're in", checked **at compile time**: you
*cannot* call `broadcast_move` on a client that is still choosing a character,
because that method doesn't exist on that type.

**The states** (game-thread view of a client; carry only the data valid then):

| Type-state | Java `ConnectionState` | Meaning / data it holds |
|---|---|---|
| `Session<Connecting>` | CONNECTED | TCP up, protocol OK; no account yet |
| `Session<Authenticated>` | AUTHENTICATED | `AuthLogin` session key validated; holds account + `SessionKey` |
| `Session<InLobby>` | AUTHENTICATED | character list loaded; choosing/creating/deleting |
| `Session<Entering>` | ENTERING | a character is selected, being loaded from DB |
| `Session<InGame>` | IN_GAME | in the world; links to the live `Player` entity |

`CLOSING`/`DISCONNECTED` aren't states you *hold* — they're the session being
dropped from the registry.

**Shape** — a generic wrapper parameterised by a state struct; transitions
consume `self` and return the next type, so a stale earlier-state value can't be
used after a transition:

```rust
struct Session<S> {
    client_id: u32,
    out: OutboundTx,          // queue packets back to the connection task
    state: S,
}

struct Connecting;
struct Authenticated { account: String, session_key: SessionKey }
struct InLobby       { account: String, session_key: SessionKey, chars: Vec<CharSelectInfo> }
struct Entering      { account: String, char_object_id: i32 }
struct InGame        { account: String, player_object_id: i32 }   // links to the entity

impl Session<Authenticated> {
    fn into_lobby(self, chars: Vec<CharSelectInfo>) -> Session<InLobby> { /* … */ }
}
impl Session<InLobby> {
    fn send_char_selection(&self) { /* only exists in the lobby */ }
    fn select_character(self, idx: usize) -> Session<Entering> { /* … */ }
}
impl Session<InGame> {
    fn player_id(&self) -> i32 { self.state.player_object_id }
    fn broadcast_move(&mut self, world: &mut World, /* … */) { /* only exists in game */ }
}
```

**Reconciling type-state with the single-owner ID registry.** The registry
(`World.clients: HashMap<u32, _>`) needs one concrete type, so at the *storage
and dispatch boundary* the typed sessions are wrapped in a plain enum that acts
as the runtime tag — the standard way to combine type-state with a container:

```rust
enum ClientSession {
    Connecting(Session<Connecting>),
    Authenticated(Session<Authenticated>),
    InLobby(Session<InLobby>),
    Entering(Session<Entering>),
    InGame(Session<InGame>),
}
```

Packet dispatch matches `(state, packet)` — which *is* Java's per-state gating,
now exhaustive: an unmatched combination is an out-of-state packet and is
logged/ignored exactly as Java rejects it. **Inside** a matched arm you hold a
statically-typed `Session<InGame>`, so only in-game methods are callable — that's
where the compile-time guarantee pays off. A state transition takes the session
out of the map (`remove`/`mem::replace`), consumes it through the typed
transition, and reinserts the new variant.

**How the `Player` entity fits (composition, not type-state).** `Player` itself
is a plain **composed** struct (identity, position, stats, inventory… — challenge
#1), and it lives in the `World` object registry keyed by `objectId`, because
visibility/broadcast need every spatial object in one place (challenge #2).
`Session<InGame>` therefore stores the **`player_object_id`**, not the `Player`
by value — this keeps a single owner for the entity and sidesteps the
double-borrow problem when an in-game action must touch both the actor and the
rest of the world (handled by the established "take the actor out, act on the
world, put it back" / id-lookup pattern). So: **type-state governs the session
lifecycle and which actions are legal; composition + the id registry govern the
entity.**

**Where it lives.** This machine is on the **game thread**. The connection task
keeps only the transport-level `ConnectionState` it needs for the handshake
(G1); the richer lifecycle above is the game-thread's model of the client and is
built out in G2 (`Authenticated`), G3 (`InLobby`/`Entering`), and G4 (`InGame`).

## 4. The critical path (why the milestone order is what it is)

`GameServer.java` initialises ~90 subsystems in a fixed order, but a client only
needs a small subset to *enter the world*. The vertical slice targets exactly
that path:

```
client TCP connect
  → ProtocolVersion  (KeyPacket: hand client the XOR key, enable cipher)
  → AuthLogin        (validate SessionKey via LoginServerThread ↔ login server)
  → CharSelectInfo   (load characters for the account from DB)
  → [CharacterCreate / CharacterDelete / Restore]
  → RequestCharacterSelect
  → EnterWorld       (build Player, register in World + region, send UserInfo)
  → MoveToLocation / Say2 / Logout   (move, chat, leave)
```

Everything else (NPCs, items, skills, combat, quests, clans…) is **breadth added
on top of a working slice**, each piece independently verifiable against the live
Java server on the same DB and client.

## 5. Workspace layout (additions)

```
l2r_interlude/
├── crates/
│   ├── commons/            # unchanged — reused as-is
│   ├── loginserver/        # unchanged
│   └── gameserver/         # NEW binary crate
│       ├── main.rs             # boot (mirrors GameServer.java order)
│       ├── config/             # Config.java port, split by ini file
│       ├── game_loop.rs        # the 100 ms tick loop + scheduler
│       ├── world/              # World, region grid, id manager, registries
│       ├── model/
│       │   ├── actor/          # Creature/Player/Npc components + actor enum
│       │   ├── item/, skill/, stats/, zone/, clan/ …  (mirror Java model/)
│       │   └── holders/, conditions/ …
│       ├── data/               # 57 XML loaders + sql tables (added per milestone)
│       ├── session.rs          # client lifecycle: Session<S> type-state (§3.1)
│       ├── network/
│       │   ├── cipher.rs        # Encryption (XOR) port  ← golden-vector tested
│       │   ├── client.rs        # GameClient + ConnectionState (transport)
│       │   ├── client_packets/  # inbound, same names as Java
│       │   └── server_packets/  # outbound, same names as Java
│       ├── loginlink.rs        # LoginServerThread (game side of GS link)
│       ├── managers/           # instancemanager/* (added per milestone)
│       ├── taskmanager/        # tick systems
│       ├── handler/            # action/item/skill/admin/chat/bypass registries
│       ├── ai/                 # AI controllers
│       ├── geoengine/          # geodata + LOS; pathfinding = path-worker service
│       └── scripts/            # the 1,131 compiled-in scripts (added per milestone)
└── dist/                       # used AS-IS (already copied), never edited
```

Java file → Rust file stays 1:1 wherever the language allows (project goal). The
big files (`Player.java` 14k, `Creature.java` 6k) are split by concern (Rust
modules) but keep method names for traceability.

## 6. Milestones

Ordered so every step is verifiable against the real client and the live Java
server. G0–G4 are the **vertical slice** (architecture proof); G5+ are breadth.

> **Note.** This section's coarse G5–G12 grouping predates the finer numbering
> the work actually followed (G0–G13 + sub-milestones — see
> [PROGRESS.md](PROGRESS.md)). The remaining subsystem breadth (what §6's
> "G12 Long tail" gestured at) is now planned milestone-by-milestone in
> **[ROADMAP.md](ROADMAP.md) (G14→G33)**, which is the authoritative post-G13
> plan.

### Vertical slice

- **G0 — Scaffold & boot.** `gameserver` crate; `Config` port reading the
  *existing* `dist/game/config/*.ini` verbatim (start with `Server.ini` +
  general keys, grow per milestone); `DatabaseFactory` → SQLite pool on the
  existing schema; ThreadPool → game-thread + tokio-runtime skeleton; empty
  100 ms tick loop with the timer scheduler and tick-overrun metric; `log.cfg`
  logging; ctrl-c → graceful drain/exit. **✔** = boots, loads all targeted
  config keys, opens the DB, ticks idle, shuts down clean.

- **G1 — Client link & cipher parity.** Game network runtime (tokio, reuse
  `commons` framing); port `Encryption` (the rolling XOR cipher) + `ConnectionState`
  machine + `GameClient`; `ProtocolVersion` → `KeyPacket` (Blowfish keygen table
  `BlowFishKeygen`, hand client the key, first-packet flag). **✔** =
  **golden-vector tests** for the cipher (Java harness dumps in/out vectors like
  login M1) match byte-for-byte; a real client completes the protocol handshake
  with no crypto error.

- **G2 — Login-link + auth.** `LoginServerThread` (game side of the GS-link we
  already implemented on the login server): register the GS, receive the session
  keys, `PlayerAuthRequest`/`PlayerInGame`/`PlayerLogout`, kick. `AuthLogin`
  client packet validates the client's `SessionKey` against the login server.
  Introduces the session type-state (§3.1): `Session<Connecting>` →
  `Session<Authenticated>`. **✔** = the Rust GS registers with the Rust login
  server (cross-checked against the Java login server); a client that
  authenticated at login reaches the character-list state (list may be empty).

- **G3 — Character selection & persistence.** Minimal `World`/`Player` skeleton;
  DB thread (`DbCommand`/`DbRequest`); `CharInfoTable`; loaders needed to build a
  character: `PlayerTemplateData`, `ClassListData`, `ExperienceData`,
  `InitialEquipmentData`, `InitialShortcutData`; packets `CharSelectInfo`,
  `NewCharacter`/`CharacterCreate`, `CharacterDelete`/`Restore`,
  `RequestCharacterSelect` → `CharSelected`. Adds the `Session<InLobby>` and
  `Session<Entering>` states (§3.1). **✔** = create a character, see it on the
  selection screen, it persists across a server restart, delete works.

- **G4 — Enter world (VERTICAL-SLICE GATE).** `EnterWorld`; `World` registration +
  region grid + known-list (visibility); `UserInfo`; `CharInfo` broadcast to
  nearby players; movement (`MoveToLocation` → validate → `MovementTaskManager`
  tick system → `MoveToLocation`/`StopMove` broadcast); chat (`Say2`);
  `Logout`/`RequestRestart`/`RestartResponse`. Reaches `Session<InGame>` linking
  the client to the `Player` entity in the object registry (§3.1). **✔** = **two
  real clients enter the world on the Rust server, see each other, walk around,
  and chat.** This is the phase gate — it exercises network, cipher, login-link,
  DB, World, regions, a tick system, and broadcast together.

### Breadth (each verifiable against the live Java server on the same DB/client)

- **G5 — Static world content.** `NpcData` + `NpcTemplate`; `SpawnData` + spawn
  registration + respawn tick; NPC `CharInfo`/known-list; `MapRegionManager`;
  `DoorData`; `ZoneManager` + zone shapes/effects; `StaticObjectData`;
  `GeoEngine` geodata load (LOS first; pathfinding wired to path workers).
  **✔** = NPCs spawn at retail coords, are visible and respawn; doors and zones
  active; LOS works.

- **G6 — Items & inventory.** Full `ItemData`; `itemcontainer/` (Inventory,
  Warehouse, PcInventory); `Item` instances + DB persistence; equip/unequip with
  `InventoryUpdate`/`UserInfo`; pickup/drop + `ItemsOnGroundManager`;
  `BuyListData`/`MultisellData` shops; private stores & trade. **✔** = loot,
  equip (paper-doll updates), buy/sell, warehouse, player-to-player trade.

- **G7 — Stats, skills & effects.** Stats engine (`stats/`, `Formulas`, `Stat`
  calc, `conditions/`); `SkillData` + `Skill`; **effect handlers as compiled-in
  scripts** (the `EffectHandler`/`SkillConditionHandler` pattern); buff/debuff
  lifecycle (apply, tick, expire via scheduler); cast pipeline
  (`RequestMagicSkillUse` → cast → land → apply → `MagicSkillUse`/`MagicSkillLaunched`);
  `SkillTreeData` learn. **✔** = learn and cast skills; buffs apply, show on the
  status bar, and expire on schedule.

- **G8 — Combat & AI.** Attack pipeline (`CreatureAttackTaskManager`);
  `Attackable`; damage via `Formulas`; death/decay; `AttackableAI` + AI-think
  tick; aggro lists; drops (`EventDropManager`); XP/SP gain and leveling;
  `Attack`/`Die`/`StatusUpdate` broadcast. **✔** = kill a monster (melee + skill),
  take damage, die and revive, receive loot/XP, level up.

- **G9 — Social & persistence systems.** `ClanTable`/`Clan` + pledge packets;
  party & `MatchingRoomManager`; friends; `MailManager`; `communitybbs`;
  private-store persistence / offline trade; `GlobalVariablesManager` &
  character variables. **✔** = form clans and parties, add friends, send mail,
  use the community board.

- **G10 — Scripting engine + quest framework.** The compiled-in script registry;
  `Quest`/`AbstractScript` framework (event listeners: talk, attack, kill, spawn,
  enter-zone…); master handler wiring; `QuestManager`; a **representative slice**
  of scripts (a few quests, village masters, one instance, custom handlers) to
  validate the framework. **✔** = a full quest is completable start-to-finish; an
  instance can be entered and cleared.

- **G11 — Script & content breadth.** Port the remaining ~1,131 scripts by
  category (quests, `ai/`, instances, events, custom, handlers). Naturally
  sub-divided; each script is independently testable. **✔** = quest/AI parity
  checklist green.

- **G12 — Long tail & parity sweep.** Remaining `instancemanager/*` (sieges,
  castles, clan halls, olympiad, item auction, manor, grand bosses…), remaining
  `taskmanager/*`, remaining packets, admin commands, all 57 data loaders,
  multilang, scheduled/precautionary restart, deadlock detector, offline-trader
  restore, full geoengine pathfinding tuning, Dockerfile (mirror `game.Dockerfile`).
  **✔** = file-by-file parity checklist (§8) complete.

**Suggested review gates:** after G1 (cipher vectors), after **G4** (the
vertical-slice interop demo — the big one), after G8 (combat), after G10
(scripting framework).

## 7. Data, scripts & geodata strategy

- **Config (`Config.java`, 3,621 lines):** ported as a struct-per-ini-file under
  `config/`, using the reused `PropertiesParser`. Read the real `dist/game/config`
  files unchanged (per instruction). Ported **incrementally** — each milestone
  adds only the keys its subsystem needs — rather than all 3,621 lines up front.
- **XML data loaders (57):** each is a straightforward `quick-xml` port of a Java
  loader reading the same `dist/game/data/*.xml`. Added per milestone (G3 needs
  ~5; G5–G8 pull in most of the rest). Snapshot-test loaded counts against the
  Java server's startup logs.
- **Compiled-in scripts (1,131):** the bulk of the remaining work (G10–G11). Each
  Java script class → a Rust module implementing the quest/AI trait, registered
  in a generated registry at boot (replacing `ScriptEngineManager`). Framework in
  G10, breadth in G11; consider a codegen helper for the mechanical shells.
- **Geodata:** `.l2j` region files are large (the Java server needed a multi-GB
  heap). Load lazily/memory-mapped where possible; LOS on the game thread against
  read-only `Arc<GeoData>`; A* pathfinding on the path-worker pool
  (request/reply), never on the game thread.
- **Generated strings:** `SystemMessageId` (16k lines) and `NpcStringId` (35k
  lines) are pure ID tables — generate from the Java source or the client dat
  files rather than hand-porting.

## 8. Testing strategy

1. **Golden vectors (G1)** — the game XOR cipher, same method as login M1
   (Java harness dumps in/out; Rust matches byte-for-byte).
2. **Packet snapshot tests** — serialize each server packet with fixed inputs,
   compare hex against Java captures (log-based or a small Java dump harness);
   the client is unforgiving about layout.
3. **Interop integration (G2+)** — real Interlude client + our login server (and
   Java login as cross-check) against the Rust GS, on the **same SQLite DB** the
   Java server uses, so behaviour is directly comparable.
4. **Data-load assertions** — each XML loader's entry counts match the Java
   startup logs.
5. **Tick-system unit tests** — movement/attack/AI/scheduler driven by synthetic
   `World` state, no sockets (the single-owner `&mut World` makes these trivial).
6. **Parity checklist** — a `PARITY_GAME_SERVER.md` mirroring the login parity
   doc: every Java file marked ported / folded / dropped (GUI, MariaDB) / dead.

## 9. Dependencies (additions to the workspace set)

Mostly already present from login. Likely new: `quick-xml` (already used),
`memmap2` (geodata), `rayon` or a fixed pool (path workers), `glam`/simple vec
math (positions), possibly `phf`/codegen for the string-id and script registries.
No game framework — keep the stack thin and auditable, as in login.

## 10. Risks

| Risk | Mitigation |
|---|---|
| Sheer scale (343k LOC, 1,131 scripts) makes "one big port" impossible | Vertical slice G0–G4 first; every later milestone is independently shippable and testable against live Java |
| Client-observable packet layout differences (any wrong byte = client desync/crash) | Snapshot tests vs. Java captures; port packet write order 1:1; enable early against the real client (G4) |
| Game XOR cipher / key-shift subtleties | Golden vectors in G1 before any gameplay, exactly like login |
| Inheritance-heavy model (`Player` 14k lines) resists literal translation | Composition + actor enum decided (challenge #1); split by concern, keep method names; land the skeleton in G3–G4 and grow it |
| Mid-handler synchronous DB reads (Java pattern) don't fit the no-block game thread | Inventory the request/continue split points up front (CONCURRENCY_MODEL open-Q #4): login/enter-world, char create/delete, name/clan checks |
| Geodata memory blow-up (Java needed multi-GB heap) | mmap + read-only shared geodata; pathfinding off the game thread |
| Single-core game logic insufficient under load | Out of scope for v1; region/instance sharding path already documented (CONCURRENCY_MODEL §2.9) — evolution, not rewrite |
| Behavioural drift from Java's concurrent/unordered handling | Intentional differences already catalogued (CONCURRENCY_MODEL §2.7); freeze tick-internal system order once (open-Q #1) |

## 11. Explicitly out of scope (this phase)

- Swing GUI / `gameserver/ui` (decision #10); MariaDB/Postgres (decision #9);
  `tools/` ports.
- Horizontal sharding of `World` (CONCURRENCY_MODEL §2.9) — only if a profiler
  demands it.
- Any change to `dist/` — config and data are consumed as-is.
