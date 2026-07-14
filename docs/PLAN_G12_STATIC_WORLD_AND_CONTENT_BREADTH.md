# G12 — Static world (zones & doors) + script/content breadth (framework extension)

Status: **executed** — see the G12 section of [PROGRESS.md](PROGRESS.md) for
what shipped (including where the build deviated: all 1180 doors loaded
rather than a filtered subset, BY_TIME cycles instead of BY_CLICK as the
live trigger, and the open questions' resolutions). This doc is kept as the
plan of record.

## 0. Why these two areas are bundled

`docs/PROGRESS.md`'s own snapshot names both as "next natural gates" without
picking one, because neither is a clean single milestone at true scope:

- **Static world** was carved out of G8 and left `⏳` (zones/doors/
  `StaticObjectData`) because G8's gate ("NPCs spawn, are visible, targetable,
  talkable") didn't need them. Nothing since has forced the issue, but three
  places in the current code are already waiting on it: `geo/region.rs:10`
  and `geo/mod.rs:7-8,209,303,347` ("doors/fences punching holes… not ported
  yet"), and `net.rs:85` ("NO_RESTART zones" TODO).
- **Script/content breadth** is literally "port the remaining ~1,131 scripts"
  per the original plan (`PLAN_GAME_SERVER.md` G11, since renumbered) — not a
  milestone, a multi-month backlog (199 quests, 83 `ai/` scripts, 24 bypass
  handler classes, 16 more village-master scripts, 101 multisell lists, 481
  buylists — see the scoping survey below). It cannot be "finished" in one
  gate any more than G11 could finish clans.

Following the precedent set by every breadth milestone so far (G9 combat, G10
social, G11 quests+clans): **G12 is cut to a vertical slice of each area** —
enough new framework + a handful of concrete instances to prove the pattern
end-to-end against the live client — with the long tail explicitly deferred
to G14 ("long tail & parity sweep", already the catch-all for
`instancemanager/*` sieges/castles/olympiad, remaining packets, admin
commands, and the rest of the 57 data loaders). **G14 absorbs everything this
doc doesn't ship**, including the bulk of the 199 quests / 83 `ai/` scripts /
24 bypass handlers — G12 does not attempt to clear that backlog, only to stop
blocking on the framework gaps.

### Gate (live client)

1. Standing in a town shows peace-zone behavior: attacking another player (or
   an NPC) is refused with the Java system message, matching retail.
2. Wading into water (a `WaterZone` area) changes swim state — at minimum the
   speed/regen shift already flagged as a TODO in the G6 notes.
3. A real door (pick one non-siege, non-fortress door from `DoorData.xml`,
   e.g. a dungeon gate) is visible, blocks movement and LOS while closed, and
   opens via its configured trigger (click or an `OP_DOOR`-type skill) —
   broadcasting `DoorStatusUpdate` to nearby clients.
4. A handful of `StaticObjectData` entries (town map panels, a castle throne)
   render for nearby clients via `StaticObjectInfo`.
5. A generic `Link` bypass (`Link <file>`) works from any NPC dialog, not just
   the two hand-wired quests.
6. A merchant NPC's **Buy** shop list opens and a purchase actually debits
   adena and adds the item (`RequestBuyItem` round trip).
7. At least 10 additional quests beyond Q00258/Q00320 are completable
   start-to-finish, chosen to exercise shapes G11 didn't: multi-kill-target,
   multi-step `cond` (3+), an `onAttack`-driven quest, and a non-repeatable
   one-time quest with the completed-mask persisted.
8. One `ai/others` script (a simple standalone behavior, not a boss/instance)
   runs against a live spawn.

## 1. How it works in Java (reference map)

### Zones
- `instancemanager/ZoneManager.java` (812 lines): boot-time parse of every
  `dist/game/data/zones/*.xml` file into `model/zone/type/*` instances (35
  concrete types), indexed into a `ZoneRegion` spatial grid (world split into
  fixed-size cells, each holding the zones overlapping it) for O(1)-ish
  point-in-zone queries.
- `Creature.revalidateZone()` runs on move/teleport: diffs the zone set at
  the new position against the cached one, firing `onEnter`/`onRevalidate`/
  `onExit` on each `ZoneType`. Zones apply effects on enter/exit (add/remove
  an `AbnormalSkill`, flip a flag) rather than being polled every tick.
- Shapes are the same three primitives already ported for spawn territories
  (`ZoneNPoly`/`ZoneCuboid`/`ZoneCylinder`, `min_z`/`max_z`) — `ZoneManager`
  and `SpawnData` share `ZoneForm`/`ZoneNPoly` etc. in Java too.
- Data distribution (from the scoping survey): no single file enumerates
  "all" zones; each XML file owns a type family (`peace.xml`→`PeaceZone`,
  `water.xml`→`WaterZone`, `no_restart.xml`→`NoRestartZone`,
  `castle_siege.xml`/`fortress_siege.xml`→`SiegeZone`, `tax.xml`→`TaxZone`,
  `fishing.xml`→`FishingZone`, etc., ~40 files, 35 zone type classes total).

### Doors
- `model/actor/instance/Door.java` (702 lines): a `Creature` subtype. State:
  `_open`, `_isAttackableDoor`, HP via the normal `Creature` stat model
  (`getDamage()` maps HP% to a 0-6 visual damage stage for siege). Open/close
  triggers from `DoorOpenType` (`BY_SKILL`/`BY_ITEM`/`BY_CLICK`/`BY_TIME`/
  `BY_CYCLE`); `openMe()`/`closeMe()` broadcast `DoorStatusUpdate` +
  `StaticObjectInfo`, can auto-close on a scheduled task, auto-cycle, and
  cascade to sibling/group/child doors. `doDie()` auto-opens (unless mid-siege).
- Data: `data/xml/DoorData.java` (286 lines) parses `DoorData.xml` — **1180
  entries**: id, name, collision polygon (`<nodes>`), location, `<stats>`
  (basePDef/baseMDef/baseHpMax), `<status>` (targetable/showHp),
  `openStatus` default open/close.
- Collision: doors punch holes in geodata LOS/pathing (the gap the current
  `geo/` module explicitly flags as unported) and block `ValidatePosition`
  movement when closed.

### StaticObjectData
- `data/xml/StaticObjectData.java` (121 lines) parses `StaticObjects.xml`
  (**159 entries**): town-map interactables (`type="0"`) and castle thrones
  (`type="1"`). Runtime type `model/actor/instance/StaticObject.java`
  (own `StaticObjectStat`/`StaticObjectStatus`), rendered via
  `StaticObjectInfo` — visual only, no HP/combat, no click behavior needed
  for the town-map/throne subset (both gated on systems — community board,
  castles — that don't exist yet, so they're pure decoration for now).

### Bypass handler families (24 classes, `handlers/bypasshandlers/`)
Full table gathered in the scoping survey; the two chosen for this slice:
- **`Link.java`** (`Link <file>`): the generic "load `<file>.htm` relative to
  the NPC's html dir and show it" primitive nearly every dialog uses
  underneath whatever specific verb triggered it. Currently every ported NPC
  (ClanMaster) fakes this ad hoc inside its own `on_event`; a real `Link`
  handler removes that duplication for every future village-master/quest
  script.
- **`Buy.java`** (`Buy`) / the buylist system: opens `ExBuySellList` from
  `BuyListData` (**338 top-level + 143 custom = 481 lists**), backed by
  `RequestBuyItem` debiting adena and adding items. This is the smallest
  viable shop vertical slice — full `Multisell.java` (item-for-item exchange,
  **101 lists**), `PrivateWarehouse`/`ClanWarehouse`/`Freight`, and
  private-store/trade stay deferred (already flagged since G6).

### Quest hooks not yet in the trait
- `QuestScript` (`game_loop/quests.rs:34-75`) currently has `on_talk`
  (required), `on_event`, `on_kill`, `on_timer`, `start_condition_html` — no
  `on_attack`/`on_spawn`/`on_first_talk`. Java's `Quest`/`AbstractScript`
  attaches these via `addAttackId`/`addSpawnId`/`addFirstTalkId` the same way
  `addKillId` works today. Adding the two mechanically-simplest (`on_spawn`
  for one-time NPC setup, `on_attack` for the "quest starts when a mob first
  hits you" pattern used by several simple quests) lets the +10 quest batch
  include at least one of each shape instead of only kill/talk quests again.

## 2. Proposed Rust design

### Zones
- New `model/zone.rs`: `ZoneShape` (rename/reuse `ZoneForm` — move it out of
  `spawn_data.rs` into a shared location both `spawn_data` and `zone` import,
  per the CLAUDE.md porting note to prefer composition over duplication)
  + `ZoneKind` enum scoped to what this slice needs: `Peace`, `Water`,
  `NoRestart` — **not** all 35 Java types. `ZoneManager`-equivalent: boot-time
  load of `data/zones/peace.xml`, `water.xml`, `no_restart.xml` only (the
  three files that back the gate); everything else (siege/castle/clanhall/
  tax/fishing/olympiad/jail/…) deferred to G14, since none of their owning
  systems (siege, castle, olympiad) exist yet either.
- Spatial index: reuse the existing region-grid pattern from G7.9
  (`World`'s 3×3-region visibility grid) rather than porting `ZoneRegion`
  fresh — zones get bucketed into the same region cells NPCs/players already
  use for broadcast scoping, and a point-in-zone query walks the (typically
  1-3) zones registered in a creature's current region.
- Per-creature zone membership: a `CurrentZones` component (bitset over
  `ZoneKind`), recomputed on move-tick alongside the existing region-index
  update (`game_loop/position.rs`) — diff old vs new, fire enter/exit.
  `Peace` enter/exit flips an `in_peace_zone` flag the attack/cast path
  already needs to check (new gate, mirrors Java's `Attackable`/`Player`
  peace check before `RequestActionUse`/`RequestMagicSkillUse` land).
  `Water` enter/exit sets a swim flag feeding into the speed/regen
  finalizers the G6 notes already TODO'd for "sit/run states".

### Doors
- New object kind in the ECS store per the CLAUDE.md guidance ("new object
  kinds… should become components in that store, not new bare `HashMap`
  fields"): `Door` components (open/closed, hp, template ref) on entities in
  `World.objects`, keyed by object id like NPCs. Data: `data/door_data.rs`
  parsing `DoorData.xml` — start with a **filtered subset**: skip any door
  flagged for a castle/fortress siege (no siege system to open/close them
  competitively) and any with `<nodes>` shapes the current geo collision
  can't yet consume; the gate only needs one working non-siege door.
- Open/close: `BY_CLICK` (action-packet triggered, mirrors the NPC
  interaction path already wired) first; `BY_SKILL`/`BY_TIME`/`BY_CYCLE`
  deferred unless the chosen gate door needs one of them.
- Geo integration: the two flagged TODOs (`geo/region.rs:10`,
  `geo/mod.rs:7-8,209,303,347`) get door state wired into LOS/collision —
  this is the one genuinely new low-level piece, since geodata is currently
  read-only `Arc<GeoData>` with no per-door override; needs a small
  door-state side-table the geo queries consult alongside the static grid.
- Broadcast: `DoorStatusUpdate` on state change, `StaticObjectInfo` on
  enter-visibility (both new packets, small — reuse `NpcInfo`'s known-list
  broadcast plumbing from G8).

### StaticObjectData
- Simplest of the three: `data/static_object_data.rs` parses
  `StaticObjects.xml`, registers a lightweight component with no HP/combat
  fields, broadcasts `StaticObjectInfo` through the same known-list path as
  doors/NPCs. No click behavior implemented (both consumers — community
  board, castle system — are out of scope), so this is pure data-load +
  broadcast, no new interaction logic.

### Bypass: `Link` and `Buy`
- `Link`: extend `npc_bypass` (`game_loop/bypass.rs:77`) with a `"Link"` arm
  that resolves `<file>.htm` under the NPC's html dir (reuse the quest
  engine's existing html-path resolution: script dir → `quests/<Name>/` →
  `noquest.htm` fallback pattern from `show_result`) and sends plain
  `NpcHtmlMessage`. Retrofit `ClanMaster`'s ad hoc page-loading to call
  through it instead of duplicating (see deviation note if this churns too
  much for the slice).
- `Buy`: new `data/buy_list_data.rs` (subset of `BuyListData.xml` — the
  handful of merchant lists needed for the gate NPC, not all 481), new
  `RequestBuyItem`/`ExBuySellList`/`ExBuySellListEx`-or-equivalent packets,
  adena debit + `items::add_inventory_item` (already exists from G11) for
  the added item, `InventoryUpdate` + `ExAdenaInvenCount` refresh (both
  already exist). Sell (`RequestSellItem`) and multisell stay out — Buy only.

### Quest/script breadth
- `QuestScript` trait: add `on_spawn(ctx)` and `on_attack(ctx)` (default
  no-op, same pattern as `on_kill`); wire NPC spawn and first-hit-taken
  points to call them (spawn already has a hook point in `data/spawn_data.rs`
  registration; attack needs one new call in the combat damage-apply path,
  `game_loop/combat.rs`).
- Port **10-15 more quests** from `dist/game/data/scripts/quests/`, picked
  for shape variety rather than raw count — candidates (to confirm against
  live drop/reward tables before committing):
  - `Q00303_CollectArrowheads` (118 lines) — simplest possible, single
    start/talk/kill NPC, good smoke test that the +breadth pipeline is low
    friction.
  - 2-3 multi-kill-target quests (several `MONSTERS` ids feeding one drop).
  - 1-2 multi-step (`cond` 1→2→3+) quests to exercise `__compltdStateFlags`
    beyond G11's single-step cases.
  - 1 `onAttack`-driven quest (simple one, **not** a Saga — the 9 Saga class-
    change quests are 400-1265 lines each with heavy branching and belong in
    G14's long tail, not this slice).
  - 1 non-repeatable one-time quest, to exercise the completed-mask bit in
    `QuestList` (G11 tested via cond math, not an actual completed quest end
    to end).
- Port **1-2 more `village_master/` class-change scripts** (from the 16
  siblings of `ClanMaster` — e.g. one `OrcChange1`/`ElfHumanFighterChange1`
  — structurally identical dialog-navigation shape) as breadth proof; the
  remaining ~14 go to G14.
- Port **1 `ai/others` script** — pick the simplest standalone behavior
  (`RandomWalkingGuards.java` or `FleeMonsters.java`, not a boss/instance/
  manager) to prove an `ai/`-shaped script (no quest state, pure periodic
  behavior driven off spawn) fits the existing `QuestRegistry`-adjacent
  registration path, or needs its own lightweight registry — this is a
  design open question to resolve during implementation, not pre-decided
  here.

## 3. Explicitly deferred to G14 (or later)

- 33 of 35 zone types (siege/castle/clanhall/tax/fishing/olympiad/jail/…) —
  all gated on systems (siege, castle, olympiad) that don't exist.
- The ~1170 non-gate doors, all `BY_SKILL`/`BY_ITEM`/`BY_CYCLE` open types,
  sibling/group/child door cascading, siege damage-stage visuals.
- Multisell (101 lists), Sell, private/clan warehouse, freight, private
  stores, player trade — all of `itemcontainer/` breadth beyond Buy.
- The other 22 bypass handler classes (`Augment`, `ChangePlayerName`,
  `ItemAuctionLink`, `Loto`, `Observation`, `SkillList`, `SupportMagic`,
  `TerritoryStatus`, `VoiceCommand`, etc.) and the 12-class voiced-command
  family.
- 184-189 of the remaining 199 quests (whatever's left after this slice's
  10-15), all 9 Saga class-change quests, tutorial (Q00255).
- 81-82 of 83 `ai/` scripts, all 10 raid bosses, all 16 `ai/areas/` dungeon
  scripts, all `ai/others/` infrastructure managers (castle/clan-hall/siege/
  olympiad-adjacent — same "no owning system yet" gate as the zone types).
- 14-15 of 16 remaining village-master class-change scripts.
- Admin commands (81 classes) — explicitly out of scope, GM tooling not
  player content.
- `StaticObjectData` click behavior (town map, castle throne) — needs
  community board / castle systems neither of which exist.

## 4. Open questions to resolve before/during implementation

1. Does `ai/others` content register through the same `QuestRegistry` (with
   empty quest-shaped fields) or does it want its own `AiScriptRegistry`
   alongside `World.quests`? Bears directly on how much of the ~82 remaining
   `ai/` scripts cost per-script in G14.
2. Is `ZoneForm` worth extracting to a shared module now (touching
   `spawn_data.rs`), or should zones duplicate the three shape variants
   short-term to avoid churn in G8-era code during this slice?
3. Which single door and single merchant NPC anchor the gate? Needs picking
   against the live dist (a starter-town non-siege door; a common vendor
   with a small buylist) before quest/door porting starts, same way G11
   picked Q00258/Q00320 by checking dist-specific rates first.
4. Retrofit `ClanMaster` onto the new generic `Link` handler, or leave it
   as-is and only use `Link` going forward? Retrofitting proves the
   abstraction but risks regressing a working G11 gate for no gate-visible
   benefit.

## 5. Tests (planned shape, mirrors G11's split)

- Unit: zone shape point-in-zone queries (reuse/extend G7.x geometry tests);
  door open/close state machine; the two new `QuestScript` hook call sites.
- DB: none new expected (doors/zones/static objects are boot-loaded, not
  per-character; quests reuse G11's `character_quests` persistence).
- Synthetic-world: peace-zone attack refusal, water-zone flag flip, door
  open blocks-then-allows movement/LOS, `StaticObjectInfo` on
  enter-visibility, `Link` bypass round trip, Buy round trip (adena debit +
  item add + packet shapes), each of the 10-15 new quests' full loops (same
  style as G11's Q00258/Q00320 tests), the one `onAttack` quest's trigger
  path, the one `ai/others` script's behavior loop.
- `e2e_create`: unaffected (no new enter-world burst fields expected from
  this slice — `StaticObjectInfo` only sends near static objects, none of
  which need be near the test's spawn point).
