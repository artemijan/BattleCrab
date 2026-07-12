# G9.5 — ECS stage 2: split components, one world

Plan for the second (final) stage of the `bevy_ecs` adoption started post-G9
(see [CONCURRENCY_MODEL.md §2.8](CONCURRENCY_MODEL.md)). Stage 1 moved the
object registries into ECS storage but kept each game object as **one fat
component** (`Player`, `Npc`) in **two separate ECS worlds**. Stage 2 makes the
storage actually component-shaped: shared data (position, vitals, movement,
combat stats, …) becomes real components, players and NPCs live in **one**
`bevy_ecs::World`, and the per-tick systems become queries over exactly the
components they touch.

**Legend for phases:** every phase must end with `cargo build` clean and the
full `cargo test` suite green — the suite is behavior-level (synthetic-world
integration tests + the real-socket `e2e_create.rs`), so it is the parity
harness for this refactor. No gameplay behavior may change.

---

## 1. Goals & non-goals

Goals:

1. **Split the fat components along system seams.** Movement interpolates
   `Position` + `Movement`; regen and damage touch `Vitals`; the combat
   formulas read `CombatStats` — each without dragging the rest of a 50-field
   struct through the cache or the borrow checker.
2. **One ECS world.** Players and NPCs become entities in the same
   `bevy_ecs::World` distinguished by components, not by which map they sit
   in. Cross-type code (movement tick, `Combatant`, targeting, visibility)
   becomes a single query instead of two hand-written passes / an if-else
   over two registries.
3. **Presence-as-filter for hot sweeps.** `Option<MoveData>` on 34 869 mostly
   static NPCs means the movement tick scans 34 869 `None`s every 100 ms.
   With `Movement` as a component that exists only while moving, the sweep
   iterates exactly the movers (`Query<(&mut Position, &mut Movement)>`).
   Same for the attack-intent and casting sweeps.

Non-goals (explicitly out of scope for this milestone):

- **No `Schedule`, no parallel systems, no `Resource`s.** Tick systems stay
  plain `fn(&mut World)` called in the fixed §2.2 order; the single-owner
  game thread remains the concurrency model. Migrating services
  (clients/scheduler/geo/DB channel) into ECS resources and systems into a
  single-threaded `Schedule` is a possible stage 3 — decide *after* this
  lands, if ever. It buys ergonomics, not correctness or speed, and it can be
  done incrementally later precisely because stage 2 keeps systems as plain
  functions.
- **No new gameplay.** Deferred TODOs (NPC regen, ground drops, zones …) stay
  deferred, even where the split makes them cheap — they land in their owning
  milestones on top of the new storage.
- **No `Entity` in the game logic's vocabulary.** Object ids (`i32`) remain
  the only key handlers, packets, and the scheduler speak (§5).

---

## 2. Target component taxonomy

Rule of thumb: **split along system access seams, not per field.** A
component is worth existing when some system reads/writes it *without* the
rest, or when its presence/absence is itself the filter a sweep needs.
Fields that are only ever read together stay together — over-atomizing
(`X(i32)`, `Y(i32)`, …) buys nothing and multiplies query tuples.

### Shared components (players and NPCs)

| Component | Fields (from today's fat structs) | Used by |
|---|---|---|
| `ObjId` | `object_id: i32` | everything (reverse lookup in queries) |
| `Position` | `x, y, z, heading` | movement, visibility, combat range, packets |
| `RegionCell` | `(i32, i32)` (today `region`) | visibility deltas, broadcast scoping, npc_regions |
| `Vitals` | `max_hp, cur_hp, max_mp, cur_mp, dead` | regen, damage/heal, death, StatusUpdate |
| `CombatStats` | `p_atk, p_def, m_atk, m_def, p_atk_spd, m_atk_spd, crit_hit, m_crit_hit, evasion, accuracy, magic_evasion, magic_accuracy, atk_range, random_dmg` | formulas, UserInfo/NpcInfo |
| `Speeds` | `run_spd, walk_spd, swim_*, move_multiplier, running` | movement, MoveToLocation/CharInfo/NpcInfo |
| `Collision` | `collision_radius, collision_height` | reach/range checks, packets |
| `AttackState` | `attack_end_tick, stance_until_tick` | combat tick, stance sweep |
| `Movement` | today's `MoveData` — **present only while moving** | movement tick (`Query` filter), move cancellation |

Notes:

- `dead` lives in `Vitals`, not a marker: every writer of `dead` is also
  touching HP in the same statement (`doDie`, revive, damage clamp), and
  death is checked as a branch inside many systems rather than being a sweep
  filter. Keeping it a field avoids an archetype move on every death/revive
  and keeps `reduceHp`-style helpers one-component functions.
- `CombatStats` becomes **stored for NPCs too**, computed once at spawn from
  the template (the same finalizer math `combat::combatant` runs on demand
  today — values are identical, so behavior is unchanged; it's memoization).
  This is what lets `Combatant` collapse into a query (§4).
- CP is player-only in L2; it goes in `PlayerVitals` (below), not `Vitals`,
  so NPC damage code never sees a CP field it must ignore.

### Player-only components

| Component | Fields | Notes |
|---|---|---|
| `PlayerCore` | `name, account, title, class_id, base_class_id, race, is_female, face, hair_*, level, exp, sp, reputation, pk_kills, pvp_kills, vitality_points, fame, cast_seq, pending_revive, teleporting` | residual identity/progression bag — nothing sweeps it |
| `PlayerVitals` | `max_cp, cur_cp` | CP-first damage soak, regen, StatusUpdate |
| `BaseStats` | `str_, dex, con, int_, wit, men` | recalculate_stats, regen bonuses |
| `StatModifiers` | `stats_add, stats_mul` | buff pump/reset |
| `Buffs` | `Vec<ActiveBuff>` | apply/remove/expire, AbnormalStatusUpdate |
| `Inventory` | today's `Inventory` | items, equip, packets |
| `SkillBook` | `skills: HashMap<i32, i32>` | SkillList, learn, cast gate |
| `Reuses` | `HashMap<i32, SkillReuse>` | reuse gate, SkillCoolTime |
| `Casting` | today's `CastState` — **present only mid-cast** | cast phase tasks, abort, move-block |
| `Intent` | today's `PlayerIntent` — **present only while set** | player combat tick filter |
| `TargetRef` | `target: Option<i32>` | targeting, cast resolution |
| `ClientPos` | `client_x/y/z/heading` | ValidatePosition |

### NPC-only components

| Component | Fields | Notes |
|---|---|---|
| `NpcCore` | `npc_id, spawn_loc, spawn_ref, respawn_secs, respawn_random_secs` | template lookup, decay/respawn |
| `NpcAi` | `intention, global_aggro, attack_timeout_tick` | 1 s think |
| `AggroList` | `HashMap<i32, AggroInfo>` | hate/reward shares |

### Markers

`PlayerTag` and `NpcTag` (zero-sized), attached at spawn. Queries that are
type-specific filter `With<PlayerTag>` / `With<NpcTag>`; genuinely shared
systems (movement) filter neither. A `MonsterTag` (from
`NpcTemplate.is_monster` at spawn) replaces the repeated
template-lookup-then-`is_monster` checks in targeting/AI where convenient.

### What deliberately stays fat

`PlayerCore` and `NpcCore` are still multi-purpose bags. That's intentional:
no per-tick system sweeps them, so there is no seam to cut. If a future
milestone adds one (e.g. a vitality decay sweep), split then — components
are cheap to introduce once the pattern is established, expensive to guess
in advance.

---

## 3. The fat structs survive as boundary DTOs

The `Player` struct does **not** disappear — it becomes `PlayerData`, a plain
(non-`Component`) struct used everywhere a player exists *outside* the ECS
world:

- **Session `Entering` state**: today `Session<Entering>` owns the `Player`
  before `EnterWorld` moves it into `World.players`. It will own a
  `PlayerData`; `EnterWorld` explodes it into a component bundle
  (`spawn(bundle)`), and restart/logout/disconnect reassemble what
  persistence needs.
- **Construction**: `Player::from_char` becomes `PlayerData::from_char`
  unchanged, then `impl PlayerData { fn into_bundle(self) -> PlayerBundle }`.
- **Persistence**: `PlayerSnapshot` (already a separate reassembly for
  `DbCommand::StorePlayer`) gathers from components instead of one struct —
  a `fn snapshot(world, oid) -> Option<PlayerSnapshot>` query.

`Npc` similarly: `spawn_all`/`handle_npc_respawn` build an `NpcBundle`;
there is no out-of-world NPC state, so no `NpcData` DTO is needed beyond the
bundle builder.

`bevy_ecs` `#[derive(Bundle)]` on `PlayerBundle`/`NpcBundle` gives the
spawn-side grouping for free and is the single place the component list is
enumerated — adding a component later touches the bundle + the systems that
want it, nothing else.

---

## 4. Storage & access model after the merge

`store.rs` is rewritten around one world (working name stays `EntityStore`,
now non-generic):

```rust
pub struct EntityStore {
    ecs: bevy_ecs::world::World,
    /// object id → Entity — players and NPCs share it (id ranges are
    /// disjoint: NPCs allocate from 0x4000_0000 up, persistent ids from
    /// db::FIRST_OID up — is_npc_oid() already relies on this).
    index: HashMap<i32, Entity>,
}
```

`World.players` / `World.npcs` are replaced by one `World.objects:
EntityStore`. `World.npc_regions` stays exactly as is (it stores object ids,
not entities).

Access API — designed around how the ~70 non-test call sites actually use
the stores today:

1. **Single-object field access** (the bulk of handler code):
   `objects.get::<C>(oid)` / `objects.get_mut::<C>(oid)` — id → `Entity` →
   typed component borrow. Multi-component variants via a small set of
   helpers: `objects.get_many_mut2::<A, B>(oid)` (one entity, two components
   — safe, disjoint by type) built on `EntityWorldMut::get_mut`.
2. **Two entities, same component** (attacker/target `Vitals`,
   caster/target): `objects.pair_mut::<C>(oid_a, oid_b)` wrapping
   `World::get_many_entities_mut` / `Query::get_many_mut` — this replaces
   today's "read fields out of A, then `get_mut(B)`, then `get_mut(A)`
   again" dance in the damage path with one straight-line borrow.
3. **Sweeps** (tick systems): cached `QueryState`s owned by the store (same
   trick as stage 1), exposed as e.g.
   `objects.query::<(&mut Position, &mut Movement)>()`. Systems that need
   the surrounding `World` services mid-sweep keep the stage-1 pattern:
   collect the ids/outputs first, then act — that discipline already exists
   everywhere (see `run_regen_tick`).
4. **Borrows against services**: `World` keeps `objects` as one field among
   siblings (`clients`, `scheduler`, `geo`, `data`, `db`, `cfg`, `rng`), so
   `&mut world.objects` + `&world.data` + `&world.clients` stay disjoint
   struct-field borrows exactly like today. This is the reason services do
   *not* become ECS resources in this milestone — resources would route
   every service access through the same `&mut ecs` borrow and force the
   `SystemState` machinery onto every handler.

What this kills:

- **`Combatant` assembly by hand** (`combat.rs:41-105`): becomes
  `objects.get_many::<(&Position, &Collision, &Vitals, &CombatStats)>(oid)`
  — one code path for both kinds, NPC branch deleted (stats were memoized
  into `CombatStats` at spawn, §2).
- **Duplicated player/NPC movement interpolation**: one
  `(&mut Position, &mut Movement, &Speeds)` sweep; the per-kind epilogues
  (player region visibility vs. `npc_regions` re-index + `NpcInfo` deltas)
  hang off `With<PlayerTag>` / `With<NpcTag>` follow-ups over the
  arrived/crossed ids collected by the sweep.
- **The `is_npc_oid` if-else** in targeting/casting/visibility resolution —
  replaced by querying the components the code actually needs and letting
  absence (e.g. no `Inventory` on an NPC) fall out as `None`.

---

## 5. Object ids stay the only foreign key

Unchanged decisions, restated because the merge makes them tempting to
"improve":

- **The scheduler keeps capturing `i32` ids, never `Entity`.** The dead-id ⇒
  no-op contract (§2.6 rule 3) already handles despawn races; `Entity`'s
  generational index would add a second, redundant liveness mechanism and
  leak ECS types into `ScheduledTask`.
- **`TargetRef`, `AggroList`, packets, sessions: ids.** The wire protocol is
  ids; keeping one key type end-to-end means no translation layer and no
  stale-`Entity` class of bugs.
- The `index` HashMap is the single id→Entity translation point, private to
  the store.

---

## 6. Phasing

Two big moves — **(A) split components, (B) merge worlds** — and the order
matters. Split **first**, merge **second**:

- Splitting first keeps today's two-store borrow model (`&mut` players while
  reading `world.npcs`) while call sites migrate field-by-field; churn per
  phase is bounded to the systems that touch the extracted component.
- Merging first was considered and rejected: with fat components in one
  world, every existing cross-type site (combat, targeting, visibility —
  `&mut Player` + `&Npc`) immediately needs `get_many_mut`/`SystemState`
  scaffolding that the component split then rewrites *again*. Splitting
  first means by merge time the cross-type sites already speak shared
  components, and the merge is mostly mechanical.

Stage-1's generic `EntityStore<T>` grows two abilities for the split phases:
spawning a `Bundle` instead of a bare `T` (`insert_bundle`), and tuple
queries (`get_components::<Q>` / cached multi-component `QueryState`s). Both
worlds keep their `object_id → Entity` index untouched.

### Phase 0 — scaffolding (no behavior change)
- `EntityStore<T>`: `Bundle` spawn + multi-component get/query API + tests.
- New `model/components.rs`: the shared component types from §2, `Bundle`
  derives. Empty of logic — data + small impl blocks only.
- Decision checkpoint: freeze the taxonomy table (§2) against the actual
  field list at implementation time (drift since this plan ⇒ update the
  plan, not silently).

### Phase 1 — `Position` + `RegionCell` (the pattern-proving slice)
- Extract from both `Player` and `Npc`; delete the fields from the fat
  structs (compiler drives the call-site migration — **no transitional
  duplicate fields**, ever; a field lives in exactly one place).
- Touches the widest call-site set (movement, visibility, combat range,
  every Info packet builder) — deliberately: once this lands, the migration
  pattern (fetch tuple, pass pieces) is established on the hardest case and
  every later phase is smaller.
- `server_packets` builders switch from `&Player`/`&Npc` to taking the
  component refs they read (e.g. `char_info(&PlayerCore, &Position, &Speeds,
  &Collision, &Vitals, &Inventory, …)`). Verbose but honest — and it makes
  the packet ↔ state dependencies visible for the first time.

### Phase 2 — `Vitals` (+ `PlayerVitals`), `Speeds`, `Collision`, `BaseStats`
- Regen, damage/heal path (`reduce_hp` becomes a `Vitals`(+`PlayerVitals`)
  function), death checks, StatusUpdate.

### Phase 3 — `Movement` as presence
- `move_data: Option<MoveData>` → insert/remove `Movement`. The movement
  tick becomes a filtered sweep (movers only — the 34.9k-static-NPC scan
  disappears). Move-stop paths (`StopMove`, teleport, death) remove the
  component; arrival removal happens inside the sweep via
  `Commands`-less deferred collection (collect arrived ids, remove after
  the iteration — same two-pass shape the visibility tick already uses).
- Same treatment for `Casting` and `Intent` (player combat tick then sweeps
  `With<Intent>` instead of all players).

### Phase 4 — `CombatStats` + `AttackState`, NPC memoization
- Extract player combat stats (recalculate_stats writes into the component);
  compute NPC `CombatStats` at spawn/respawn; delete the `Combatant` NPC
  derivation branch; `combatant()` becomes a thin query wrapper (kept as a
  named fn — the formulas still want one plain-struct view for readability).

### Phase 5 — remaining player/NPC splits
- `StatModifiers`, `Buffs`, `Inventory`, `SkillBook`, `Reuses`, `TargetRef`,
  `ClientPos`, `NpcAi`, `AggroList`, `NpcCore`; fat structs shrink to
  `PlayerCore` residual + bundle builders; `Player`/`Npc` renamed
  `PlayerData`/(bundle fn) per §3, session `Entering` switched to
  `PlayerData`.

### Phase 6 — merge the worlds
- One `EntityStore` (non-generic, §4), one id index; `World.players` /
  `World.npcs` → `World.objects`; `PlayerTag`/`NpcTag` markers;
  `pair_mut` for the two-entity mutation sites; unify the movement sweep;
  delete `is_npc_oid` dispatch where components now answer the question
  (the fn itself stays — the id-range fact is still used for id allocation).
- `PlayerSnapshot` reassembly + `remove`-on-despawn: death/decay/logout code
  that today gets the whole struct back from `remove()` switches to
  gather-what-you-need-then-`despawn` (§7 risk 3).

### Phase 7 — docs & closeout
- Rewrite CONCURRENCY_MODEL §2.8 (stage 2 = done, describe final model);
  update PROGRESS.md; note the stage-3 (`Schedule`/resources) decision as an
  explicit open question, default **no** until something needs it.

Phases 1–5 are each a normal-sized PR; 6 is the big one but lands on
pre-migrated call sites. Every phase: full `cargo test` + a manual
two-client smoke (login → fight a monster → die → revive) before merge.

---

## 7. Risks & mitigations

1. **Borrow-checker churn at ~70 call sites.** The two-store model hid a lot
   of aliasing questions that one world surfaces. Mitigation: split-first
   phasing (§6) so cross-type sites already take narrow components before
   the merge; `pair_mut`/`get_many_mut2` helpers centralize the unsafe-ish
   plumbing in `store.rs` behind safe signatures.
2. **Archetype moves from presence-toggled components** (`Movement`,
   `Casting`, `Intent`). A move copies the entity's other components between
   tables — fine at human action rates (a click), pathological only if
   toggled per-tick in a sweep. Policy: presence-based only for states that
   gate *which entities a sweep visits* (worth it: the sweep shrinks by
   orders of magnitude); plain fields for states checked as branches
   (`dead`, `running`, `teleporting`). If profiling ever flags the moves,
   the fallback is `SparseSet` storage for those components — a one-line
   `#[component(storage = "SparseSet")]` change, not a redesign.
3. **`remove()` semantics change.** Today `players.remove(&id)` returns the
   whole `Player` (logout persistence uses it). After the split there is no
   single struct to return. All removal sites switch to: build
   `PlayerSnapshot` from components → `despawn`. Audit every `.remove(&`
   call site in phase 5/6 — silently dropping a component that persistence
   needed is the data-loss failure mode here, and `e2e` + the
   restart/logout/disconnect tests are the guard.
4. **Iteration-order sensitivity.** Bevy table order differs from HashMap
   order and changes as archetypes split. Stage 1 already crossed this
   bridge (suite passed unchanged), but the split creates *new* archetype
   boundaries mid-suite. Tests using `forced_rolls` assume a fixed
   actor-processing order — any new flakiness means a test encoded storage
   order, fix the test (assert per-actor outcomes, not global sequences).
5. **Packet builders' signature explosion** (a `CharInfo` needs ~7
   components). Acceptable: it documents real dependencies. Where it gets
   silly, group with bevy's `QueryData` derive (one `#[derive(QueryData)]
   struct CharInfoView { … }` per heavy packet) so the tuple lives in one
   place.
6. **bevy_ecs 0.19 API drift** vs. this plan's method names
   (`get_many_entities_mut` etc. moved around across 0.14→0.19). Verify
   exact API at phase-0 time; the capabilities (multi-entity disjoint
   mutable access, cached `QueryState`, bundles, sparse-set storage) all
   exist — only spelling varies.

---

## 8. Definition of done — **all met** (see PROGRESS.md G9.5 for the
completion notes and the small deviations: kind markers folded into the
`Player`/`Npc` residual cores instead of zero-sized tags; `pair_mut` never
needed)

- [x] No `Component` type has both player-only and npc-only fields; shared
      components carry no `is_player`-style discriminants.
- [x] One `bevy_ecs::World`; `World.players`/`World.npcs` gone; single id
      index; `npc_regions` unchanged.
- [x] Movement/regen/AI/combat/stance sweeps are component queries; the
      movement sweep visits only entities with `Movement` (players and NPCs
      in one sweep).
- [x] `Combatant` NPC stat derivation deleted (memoized `CombatStats`).
- [x] Fat structs survive only as boundary DTOs (`PlayerData` +
      bundles); session/persistence round-trip proven by the existing
      restart/logout/disconnect tests.
- [x] Scheduler tasks, targets, aggro, packets still speak `i32` object ids
      exclusively; `Entity` appears nowhere outside `store.rs`.
- [x] Full suite green after every phase (147 tests); `e2e_create.rs`
      (real-socket login→create→enter-world) green; the 34.9k-NPC dist
      spawn smoke test runs through the merged store at pre-migration
      speed. (An interactive two-client combat smoke against the real
      client remains worthwhile before shipping further milestones.)
- [x] CONCURRENCY_MODEL §2.8 rewritten to describe the final model; stage-3
      question logged (default **no**).
