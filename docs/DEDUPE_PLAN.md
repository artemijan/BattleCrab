# Deduplication plan

Derived from RustRover's **Duplicated code fragment** inspection run over
`crates/{commons,dashboard_api,gameserver}` (464 non-test, non-generated files),
cross-checked against a type-2 clone scan that pairs the anchors RustRover
reports individually.

> RustRover reports *where* a duplicate is, never *what it duplicates*. Every
> pairing below was established by reading both sides. Line numbers are against
> `b6153ddb`.

**Estimated net removal: 1,500–2,000 lines.** Phases are independent; each is a
separate commit that must leave `cargo test` green.

---

## Ground rules

1. One phase per commit. `cargo check --all-targets && cargo test` between each.
2. **Behaviour-preserving phases (1, 4–8) never change an edge case.** If a
   phase turns out to need a semantic decision, it moves to phase 2/3.
3. Phase 2 is the only phase that changes behaviour. It lands last among the
   early phases so a bisect can separate "moved code" from "changed answer".
4. Never widen visibility further than needed: `pub(crate)` in
   `game_loop::helpers`, not `pub`.

---

## Phase 1 — The accessor layer ✅ **done** (`186791d4`, `e2356a00`, `be9d42fe`)

Landed as three commits. `helpers::region_of` shipped as **`region_cell_of`**:
`crate::world::region_of(x, y)` already existed and derives a region from raw
coordinates, which is a different operation.

The original plan follows.

### Original plan

The root cause of most findings: 2,478 `get_component::<…>` call sites and no
accessor layer, so modules either inline the chain or write a private helper.
`game_loop/helpers.rs` already *is* this layer (`pos_of`, `npc_id_of`, `adena`,
`instance_of`) — it just isn't discoverable, so it gets reinvented.

### 1a. `player_name` — 11 identical private copies

All eleven are byte-identical modulo the parameter name and whether `Player` is
imported or path-qualified (`party_room.rs` spells it `.map_or_else(String::new,
…)`, same semantics).

| File | Line | Local name |
|---|---|---|
| `game_loop/crafting.rs` | 1183 | `name_of` |
| `game_loop/party_room.rs` | 48 | `name_of` |
| `game_loop/duel.rs` | 887 | `player_name` |
| `game_loop/clans/mod.rs` | 638 | `player_name` (already `pub(crate)`) |
| `game_loop/command_channel.rs` | 50 | `name_of` |
| `game_loop/sell_buffs.rs` | 549 | `name_of` |
| `game_loop/party.rs` | 310 | `player_name` |
| `game_loop/olympiad.rs` | 1352 | `player_name` |
| `game_loop/admin/points.rs` | 380 | `name_of` |
| `game_loop/four_sepulchers.rs` | 572 | `player_name` |
| `game_loop/petition.rs` | 27 | `player_name` |
| `game_loop/guard.rs` | 158 | `player_name` → `Option<String>` |

**Action.** Keep `guard.rs`'s shape as the canonical one — it loses nothing and
lets each caller state its own fallback:

```rust
// game_loop/helpers.rs
pub(crate) fn player_name(world: &World, oid: i32) -> Option<String> {
    world.objects.get_component::<Player>(&oid).map(|p| p.name.clone())
}
```

Delete all eleven; callers wanting the old `String` write
`.unwrap_or_default()`. ~39 further sites inline the same chain and can be
migrated opportunistically — do **not** hunt them all in this commit.

### 1b. `region_of` — ~63 inline sites, no helper anywhere

```rust
let Some(region) = world.objects.get_component::<RegionCell>(&oid).map(|r| r.0) else { return; };
```

Confirmed anchors: `antharas.rs:1048`, `core_boss.rs:181`, `dr_chaos.rs:223,289`,
`grand_boss.rs:48`, `siege.rs:533,566,894`, `private_store.rs:1023`,
`skills/cast.rs:557`, `instances.rs:120`, `servitor.rs:651`, `pvp.rs:423,440`,
`ground_items.rs:178`, `effect_point.rs:176`, `npc_ai.rs:238,506,637,928`.

**Action.** Add `helpers::region_of(world, oid) -> Option<(i32, i32)>`. Pure
addition, zero risk — the best place to start.

### 1c. `clan_name` — ~18 inline sites, 7 of them in `clans/wars.rs`

```rust
let clan_name = world.clans.get(&clan_id).map(|c| c.name.clone()).unwrap_or_default();
```

**Action.** `clans::clan_name(world, clan_id) -> Option<String>`.

### 1d. `npc_id_of` — one copy of an existing helper

`item_auction.rs:867` reimplements `helpers::npc_id_of`. Delete and import.

**Phase 1 exit check:** `rg -c 'fn (player_name|name_of|region_of|clan_name)\('`
returns one hit each.

---

## Phase 2 — Semantic reconciliation ✅ **done** (`d17cc0eb`, `cb610c59`)

**Outcome: only one of the three families actually changed behaviour, and the
`clan_of` bug this phase was built around does not exist.**

- **2a `is_dead`** — flipped to fail-closed. All three permissive call sites
  were audited individually and none changes observable behaviour (details in
  `d17cc0eb`). 40 already-fail-closed inline chains folded in.
  **Still open:** 28 inline `is_some_and(|v| v.dead)` chains across 23 files
  were deliberately left alone — each needs the same per-site audit.
- **2b `clan_of`** — audited first, as planned. Every bare-`i32` call site
  already guards the 0 sentinel (`pvp.rs:181` tests `!= 0`; `mutual_war_between`
  guards internally; `clanned_target` pre-filters; `siege_remove` returns on
  `<= 0`). Consolidation was mechanical after all.
- **2c `position_of`** — the two `(0, 0, 0)` coercions both store a *return*
  location, and both are unreachable in practice (a player must be in the world
  to enter an instance or an olympiad match). The fallback now sits at the call
  site with a comment rather than hidden in a helper. Modelling the return
  location as `Option` properly is left as a follow-up, since it changes
  `OlympiadMatch` and the instance exit path.

The original analysis is kept below for reference.

### Original analysis

Three helper families exist in **mutually contradictory versions**. These are
latent bugs, not style issues. Each needs a decision before the merge.

### 2a. `is_dead` — 6 copies, split 3/3 on missing `Vitals`

```
missing Vitals ⇒ ALIVE          missing Vitals ⇒ DEAD
events/tvt.rs:502               skills/conditions.rs:460
npc_cast.rs:516                 valakas.rs:407
olympiad.rs:1002                skills/affect.rs:771  (.map().unwrap_or(true))
```

Plus ~36 inline `get_component::<Vitals>(&oid).is_none_or(|v| v.dead)` sites.

**Recommendation:** missing `Vitals` ⇒ **dead** (`is_none_or`). An object that
reaches these call sites without `Vitals` has been despawned, and every one of
these is a "may I act on this target?" guard where failing closed is correct.

**Before flipping**, read the three `is_some_and` sites — `tvt.rs` decides round
scoring, `npc_cast.rs` gates AI casting, `olympiad.rs` decides match outcome. If
any of them depends on the current permissive answer, that is a real bug and
should be its own commit with a test.

### 2b. `clan_of` — 4 copies, two incompatible return contracts

```
Option<i32>, 0 filtered out      i32, 0 passed through
community_board.rs:454           pvp.rs:368
guard.rs:149                     admin/castle.rs:253
```

Callers of the bare-`i32` version can compare two clanless players and conclude
they are clanmates.

**Recommendation:** keep the `Option` form — it makes "clanless" unrepresentable
as a clan id. Convert the two bare callers with an explicit `.unwrap_or(0)` at
the call site. **Audit `pvp.rs` for the clanless-vs-clanless comparison** before
converting; that is where the bug would bite.

### 2c. `position_of` — 5 copies, 2 coerce a missing position to the origin

```
Option<(i32,i32,i32)>            (i32,i32,i32), else (0,0,0)
helpers.rs:34  (canonical)       instances.rs:336
geo/distance.rs:4                olympiad.rs:994
npc_ai.rs:432
```

A missing position becoming the map origin produces a teleport to nowhere rather
than an error.

**Recommendation:** delete all four, route through `helpers::pos_of`, and make
the two `(0,0,0)` callers handle `None` explicitly.

---

## Phase 3 — Party / command-channel group API ✅ **done** (`2e497509`)

Shipped as three functions in `game_loop::party`, as planned — the three solo
readings are three different game rules and stay distinct:

| | |
|---|---|
| `party_members` → `Option<Vec<i32>>` | the honest lookup |
| `group_or_self` → `Vec<i32>` | flavour A, solo = a party of one |
| `leader_and_members` → `Option<(i32, Vec<i32>)>` | the raid-entry shape |

Flavours B (`.unwrap_or_default()`) and C (`else { return; }`) fall out of the
`Option` at each call site, which is the point.

**The CC-aware promotion in the original plan turned out to be fiction.**
`antharas::group_of`'s doc comment described the command channel outranking the
party in full detail, but the body never touched `command_channels` and was
byte-identical to `sailren::group_of`, which documents itself as party-only. So
Antharas's entry gate has never honoured the CC. Recorded as
`TODO(antharas-cc)` — registered in the census inventory and written up under
*Deferred TODOs* in `PROGRESS.md` — rather than fixed in passing: honouring the
channel means deciding what a 200-player CC does to the lair cap.

Worth noting how it stayed hidden: the claim lived in prose, and the repo's
marker census only sees `TODO(G<N>)` tags. A doc comment that promises
behaviour the body does not implement is invisible to it by construction.

### Original plan

The idiom that started this audit. 26 non-test sites write out
`PartyRef → world.parties → members.clone()`, with **three incompatible solo
fallbacks**:

| Flavour | Meaning | Sites |
|---|---|---|
| A `unwrap_or_else(\|\| vec![oid])` | solo = party of one | `skills/effects/triggers.rs:197,305`; `admin/instance.rs:250` |
| B `unwrap_or_default()` | solo = nobody | `duel.rs:393`; `quests.rs:1719`; `four_sepulchers.rs:443` |
| C `else { return; }` | solo aborts caller | `skills/effects/control.rs:520,678` |

Two more add the command-channel layer: `antharas.rs:434` (`group_of`, CC wins
over party) and `death/resurrect.rs:501`. `sailren.rs:119` and `cubic.rs:365`
reimplement the party half again.

**Action.** Three named helpers in `game_loop/party.rs` — not one:

```rust
pub(crate) fn party_members(world: &World, oid: i32) -> Option<Vec<i32>>;
pub(crate) fn group_or_self(world: &World, oid: i32) -> Vec<i32>;   // flavour A
```

…and promote `antharas::group_of` to `command_channel` for the CC-aware case.
Flavours B and C then fall out of the `Option` at each call site, which keeps the
difference deliberate instead of accidental. **Do not collapse A/B/C into one
helper** — they are three different game rules.

---

## Phase 4 — Parameter bundling ✅ **done** (`ee4250be`, `9a07ea71`, `b7b149bb`)

- **4a** shipped as `game_loop::stat_ctx::with_stat_ctx`, a **closure-scoped**
  borrow rather than a struct the caller holds: half the eight sites make
  several buff calls against one lookup, so a per-call helper would have turned
  one component lookup into N in the stat-recalc path. −244/+44.
- **4b** — `ItemTemplate` already derived `Default`; the fixtures predated it
  and couldn't use it because the derive is a zero-fill that disagrees with
  Java in four places. Added a `#[cfg(test)] for_test()` base carrying those
  corrections. All 14 sites are test fixtures, not the "4 non-test + 6 test"
  this plan originally claimed. −388/+81.
- **4c** — **costs ~70 lines rather than saving them.** The bools genuinely
  vary across the ten call sites, so the win is that a transposed argument is
  now a compile error, not a smaller diff.

The original plan follows.

### Original plan

Not copy-paste so much as signatures long enough that the call becomes a block.

### 4a. `StatCtx` — 8 sites of a 10–22 line `get_many_mut`

```rust
if let Some((target, base, mut mods, inventory, mut buffs, mut speeds, mut combat)) =
    world.objects.get_many_mut::<( … 7 components … )>(&oid)
```

`passive_skills.rs:88`, `clans/skills.rs:181`, `death/progression.rs:537`,
`options.rs:145,176`, `expertise.rs:76`, `skills/effects/continuous.rs:369`,
`skills/effects/ticks.rs:648`.

The same six `&mut` accumulators are then threaded positionally into
`add_buff`/`remove_buff` at 14 further sites (`weight.rs:204,216`,
`options.rs:157,188`, `expertise.rs:93,103,114,126`, `passive_skills.rs:103,115`,
`death/progression.rs:549`, `skills/effects/ticks.rs:659`).

**Action.** Bundle into one `StatCtx<'_>` borrow struct and take `&mut StatCtx`
in `add_buff`/`remove_buff`. Collapses both families at once — the single
highest-leverage change in the plan.

### 4b. `ItemTemplate: Default` — a 17-line literal, 10 times

`network/enter_world.rs:824`, `model/inventory.rs:1177,1214,1264`, plus six in
`game_loop/tests/mod.rs` and `data/skill_data/tests.rs`.

**Action.** Derive/impl `Default`, reduce every site to the 2–3 fields it sets.
Highest line-count win per unit of risk, and it makes all ten immune to the next
field added to the struct.

### 4c. `apply_skill_damage` params — 7 sites in `skills/instant.rs`

Lines 81, 177, 469, 701, 889, 1103, 1281. All eleven arguments identical except
`damage` and `mcrit`.

**Action.** A small params struct with `..Default::default()` so what actually
varies is readable at a glance.

---

## Phase 5 — Large block extraction ✅ **done**

Everything in the table below has landed except the `db` items-insert pair,
which is not a real clone (see "Not extracted"). A clone scan of the working
tree afterwards turns up no logic duplication above 10 lines — what is left at
that size is data tables, config field lists and `pub mod` runs.

### Not extracted, deliberately

- **`db/commands.rs` ↔ `db/queries.rs` items insert.** The three sites share
  the sea-orm *column list*, which is just the table's shape, but every value
  comes from somewhere different: the freight path hardcodes `loc: "FREIGHT"`,
  the initial-equipment path hardcodes the enchant/mana/time columns, and the
  restore path reads all of them off an `ItemRow`. A shared builder would need
  eight parameters and read worse than the three literals. Same false-positive
  shape as `warn_err`.

### Landed

Landed (`1996a836`, `0e796435`, `25422468`, `cc9e72f7`):

| What | Sites | Result |
|---|---|---|
| illegal-action punish preamble | 35 | `punishment::illegal_action` reads the config itself |
| `heal` / `heal_percent` shared branches | 35+30, 28+28 | `heal_npc`, `notify_heal` |
| stop-movement block | 5 | `helpers::stop_movement` |
| door lookup | 5 | `doors::find_shared_door` + `castle::find_upgradable_door` |
| `class_level` | **3**, not 2 | `helpers::class_level` |
| `ride_target` | 2 | `mounts::ride_target`, now `pub(super)` |

Two corrections worth keeping:

- **`class_level` had a third copy** in `user_commands.rs:696` that the
  inspection never paired with the other two. It turned up only when checking
  callers — a reminder that RustRover reports anchors, not clusters.
- **The `warn_err` "duplication" is not real.** `warn_err` is already a shared
  helper in `db/queries.rs`; what the inspection flagged 24 times is the shape
  of the sea-orm statements passed *to* it, which differ by table and column.
  The macro this plan proposed would not pay for itself. Dropped.

Also landed (`ca522237`, `321a0523`, `72893acc`):

| What | Result |
|---|---|
| siege town-respawn loop, 17 × 2 | `siege::teleport_to_town` |
| clans dissolve gate, 16 × 2 | `clans::in_siege_zone` |
| clans pending-request gate, 15 × 2 | `clans::refuse_if_busy` |
| level-change broadcast, 16 × 2 | `progression::apply_level_change` |
| restore-lost-exp, 15 × 2 | `restart::restore_lost_exp` |
| synthetic passive buff, 16 × **4** | `ActiveBuff::passive_pump` |
| valakas cinematic keyframe, 14 × 2 | `valakas::broadcast_camera` |
| untransform collision restore, 14 × 2 | `transforms::restore_class_collision` |
| party in-range filter, 13 × 2 | `party::members_within` |
| dispel candidates snapshot, 11 × 2 | `instant::buffs_on` |

> **Watch the project path when re-running the inspection.** The RustRover MCP
> indexes `/Users/artem/dev/l2/l2r_interlude` — the *main* checkout — not a
> worktree under it. Anchors from a run while work sits unmerged in a worktree
> describe `main`, not what you are editing. Verify completion with the
> standalone scanner pointed at the worktree instead.

### Original table

Ranked by lines removed. Each is a local `fn` extraction inside its own file
unless noted.

| Lines | Sites | What |
|---|---|---|
| 35 + 30 | `skills/instant.rs:988`, `:1123` | NPC-vs-player heal branch; identical but for a comment |
| 28 × 2 | `skills/instant.rs:1048`, `:1162` | `client_for_player` → status-update block |
| 22 × 2 | `clans/skills.rs:181`, `options.rs:145` | (folded into phase 4a) |
| 17 × 2 | `siege.rs:618`, `:1066` | `for oid in targets` broadcast loop |
| 16 × 2 | `db/commands.rs:1537`, `db/queries.rs:1902` | `items::Entity::insert(items::ActiveModel {…})` — extract to `db::queries` |
| 16 × 2 | `clans/alliance.rs:181`, `clans/membership.rs:748` | `if let Some(pos) = world` |
| 16 × 2 | `death/progression.rs:141`, `:176` | level-change broadcast |
| 16 × 2 | `expertise.rs:226`, `clans/skills.rs:156` | `ActiveBuff { … }` construction |
| 15 × 2 | `clans/alliance.rs:408`, `clans/membership.rs:157` | membership guard |
| 15 × 2 | `death/restart.rs:182`, `:231` | `let restored = { … }` |
| 14 × 2 | `admin/mounts.rs:323`, `admin/transforms.rs:288` | ride/transform target resolve |
| 14 × 2 | `valakas.rs:530`, `:670` | `special_camera` cinematic |
| 14 + 9 | `henna.rs:23`, `clans/mod.rs:650` | `ClassId.level()` occupation tier — move to `enums` |
| 13 × 2 | `party.rs:1342`, `:1399` | `in_range` member filter |
| 11 × 5 | `combat/intent.rs:435,886,968,1080`, `skills/cast.rs:1162` | `has_component::<Movement>` stop-move block |
| 10–11 × 5 | `doors.rs:74,95,127`, `castle.rs:440,475` | `door_regions.values().flatten().find(…)` — one `find_door` helper |
| 7–10 × 24 | `db/commands.rs` (68, 269, 504, 523, 544, 553, 562, 571, 714, 1044, 1056, 1136, 1202, 1218, 1231, 1259, 1271, 1283, 1295, 1475, 1484, 1517, 1557, 1706, 1757) | `warn_err(…)` wrapper — **replace with a macro**, ~170 lines → ~30 |
| 8 × 35 | `private_store.rs`, `mail.rs`, `servitor.rs`, `manor.rs`, `trade.rs`, `shop.rs`, `ground_items.rs` | illegal-action punish preamble — fold `world.cfg.general.default_punish` into `punishment::illegal_action()` |

---

## Phase 6 — Boss-script commons ✅ **done** (`46db5240`, `d82ed747`)

| Helper | Copies | Home |
|---|---|---|
| `set_status` | 3 + 1 inline | `grand_boss` — the write half of the `status()` already there |
| `find_alive` | **3**, not the 1 recorded | `grand_boss` |
| `find_spawned` | 2 (`find_antharas`/`find_valakas`) | `grand_boss`; both keep their names as one-line wrappers |
| `has_buff` | 3 | `abnormal`, **not** `grand_boss` |

Three notes:

- **`has_buff` is not a boss helper.** `auto_use` uses it too, and `abnormal`
  is where the other "is this state up?" predicates live. Its doc marks the
  distinction from the `effect_flag` predicates beside it: those ask whether
  *some* buff imposes a state, this one asks about a named skill.
- **`find_alive` and `find_spawned` stay separate.** `find_alive` walks every
  object (so it needs `&mut World`) and filters corpses; `find_spawned` reads
  the region index from `&World` and does not. Each doc points at the other.
- **Baium wrote the status-and-persist pair inline**, with no local
  `set_status`, so neither the inspection nor the first sweep saw it. Found by
  grepping for the *operation* (`grand_bosses.get_mut` followed by
  `.status =`) rather than for a function name — worth repeating for the
  remaining phases.

A clone scan filtered to the boss modules afterwards returns only a shared
function-parameter list, no logic.

### Original plan

The grand-boss modules were ported one-file-per-boss and each grew its own copy
of the same three helpers.

| Helper | Copies |
|---|---|
| `set_status` (6 ln) | `antharas.rs:905`, `dr_chaos.rs:44`, `valakas.rs:773` |
| `has_buff` (6 ln) | `antharas.rs:792`, `valakas.rs:425`, `auto_use.rs:451` |
| `npc_regions…find(…)` (6 ln) | `antharas.rs:914`, `valakas.rs:782` |
| `find_alive` (11 ln) | `baium.rs:474` (pairs with the `find` idiom above) |

**Action.** A `game_loop/grand_boss.rs` commons section (the module already
exists). Keep per-boss constants where they are — only the mechanics move.

---

## Phase 7 — Module twins ✅ **done** (`fc67c1ca`, `67f9a8fb`)

Phases 5 and 6 had already drained `mounts`↔`transforms` and
`clans/alliance`↔`clans/membership`. What was left:

| What | Sites | Result |
|---|---|---|
| `items().iter().find(\|i\| i.object_id == …)` | **39** | `Inventory::by_object_id` |
| `carried` (`auto_potions` ↔ `auto_use`) | 2 | `helpers::carried_item` |
| `two_handed` (`combat/attack` ↔ `combat/intent`) | 2 | `combat::wields_two_handed` |
| inline clan-id read | **30** + 2 wrappers | `guard::clan_of_or_zero` |

Two notes:

- **`by_object_id` was a missing method, not a clone.** Its sibling
  `first_of_item` was already on `Inventory`, and *its* doc says it exists
  because callers open-coded the item-id version. The object-id half — Java's
  `getItemByObjectId` — had simply never been added, so 39 sites open-coded it.
- **The clan-id sweep is a phase-2 leftover** this scan surfaced. Phase 2
  consolidated the four `clan_of` *helpers* but left the inline reads, and
  `pvp.rs` / `admin/castle.rs` each kept a private `i32` wrapper.
  `guard::clan_of_or_zero` is that wrapper once;
  `clan_of(..).unwrap_or(0)` is exactly the inline expression, since `clan_of`
  filters the `0` sentinel to `None` and `unwrap_or(0)` puts it back.

`lottery`↔`monster_race` and `db/commands`↔`db/queries` turned out to have no
shared logic left once `by_object_id` landed — what the inspection paired in
the `db` case was the sea-orm column list, recorded under phase 5.

### Original list

Pairs of files that are near-copies. Lower priority: each needs a judgement call
about whether the shared shape is real or coincidental.

- `admin/mounts.rs` ↔ `admin/transforms.rs` — 7, 11 and 14-line clones
- `clans/alliance.rs` ↔ `clans/membership.rs` — 7, 15 and 16-line clones
- `auto_potions.rs` ↔ `auto_use.rs` — `carried` (13/11 ln), `has_buff`, `item_skills`
- `combat/attack.rs` ↔ `combat/intent.rs` — `two_handed` (12 ln)
- `db/commands.rs` ↔ `db/queries.rs` — insert/on-conflict blocks
- `lottery.rs` ↔ `monster_race.rs` — ticket lookup + client send

---

## Phase 8 — Optional: `Path prefix not necessary`

Not duplication, but RustRover flags it several hundred times: `crate::model::Player`
written out where `Player` is already imported. Densest in `death/restart.rs`
(19), `death/rewards.rs` (15), `admin/mod.rs` (60+), `skills/effects/mod.rs` (13).

Phase 1 removes many of these incidentally. Do the rest as one mechanical sweep
**after** phase 1, or not at all — it is pure noise reduction and will conflict
with every other phase if done first.

---

## Excluded as intentional

- `commons/src/system_messages/generated.rs` — ~4,100 `MessageInfo` repeats and
  190 near-identical `SystemMessage::new` constructors. Machine-generated; the
  repetition belongs to the generator.
- `gameserver/src/scripts/*.rs` — quest scripts share a `QuestCtx` shape by
  construction. Real hot spots exist (`q00260` vs `q00263` are 31 lines
  identical; `q00125`/`q00126` repeat a 12-line block six times) but a faithful
  one-file-per-quest port is a deliberate structure.
- `models/src/entity/**` — SeaORM derive boilerplate, 95 files.
- `network/server_packets/**` — `w.write_i32(…)` runs reflect the wire format.

---

## Reproducing the scan

RustRover, per batch of ~25 files:

```
mcp__rustrover__lint_files(files=[…], min_severity="warning")
```

Filter for `"description": "Duplicated code fragment"`. Note it reports each
anchor independently with no pairing — use the type-2 scanner at
`~/dev/l2/_tooling/rust_clone_detect.py` to recover which sites pair with which:

```
WIN=5 MIN_COUNT=3 SKIP=generated.rs,/tests/,_tests.rs,/scripts/ python3 rust_clone_detect.py
```
