# l2r_interlude — Milestone Progress & State

Living status tracker for the Java→Rust rewrite. Plans:
[PLAN_LOGIN_SERVER.md](PLAN_LOGIN_SERVER.md) (login, M0–M5) and
[PLAN_GAME_SERVER.md](PLAN_GAME_SERVER.md) (game, G0–G12). Architecture:
[CONCURRENCY_MODEL.md](CONCURRENCY_MODEL.md),
[JAVA_TO_RUST_CHALLENGES.md](JAVA_TO_RUST_CHALLENGES.md).

**Legend:** ✅ done · 🚧 in progress · ⏳ not started.

---

## Snapshot

| Phase | Milestone                                                   | Status |
|-------|-------------------------------------------------------------|---|
| Login | M0–M5                                                       | ✅ feature-complete, interop-verified with Java GS |
| Game  | G0 Scaffold & boot                                          | ✅ |
| Game  | G1 Client link & cipher parity                              | ✅ |
| Game  | G2 Login-link + auth                                        | ✅ |
| Game  | G3 Character selection & persistence                        | ✅ |
| Game  | G4 Enter world (Player, HP/MP, UserInfo, enter-world burst) | ✅ (incl. paperdoll/mask enums) |
| Game  | G5 Items & inventory                                        | ✅ vertical slice (items, equip/unequip, initial gear) |
| Game  | G6 Stats, skills & effects                                  | ✅ vertical slice (stat engine, skill learn/cast, buffs) |
| Game  | G7 Movement & targeting (no geodata)                        | ✅ |
| Game  | G7.5 Full single-target skill casting                       | ✅ (real cast timing/formulas, reuse, abort, nukes/heals/buffs on others) |
| Game  | G7.8 Geodata & position validation                          | ✅ (`.l2j` loading, LOS, move clamping, ValidatePosition — zones still ⏳) |
| Game  | G7.85 Pathfinding (path-worker service)                     | ✅ (`CellPathFinding` port, dedicated worker thread + channels, multi-segment route following for player moves — NPC moves still straight-line) |
| Game  | G7.9 Region-grid visibility & scoped broadcasting           | ✅ (CharInfo/DeleteObject, 3×3 region knownlist, region-scoped broadcasts) |
| Game  | G8 Static world content (NPCs/spawns)                       | ✅ vertical slice (34.9k NPCs spawned, visible, targetable, talkable — zones/doors still ⏳) |
| Game  | G9 Combat & AI                                              | ✅ vertical slice (auto-attack, monster AI, death/decay/respawn, XP/SP/level-ups, auto-loot drops, die→revive) |
| Game  | G9.5 ECS stage 2 — split components, one world              | ✅ (plan: [PLAN_ECS_STAGE2.md](PLAN_ECS_STAGE2.md)) |
| Game  | G9.6 Macros & panel shortcuts                               | ✅ (plan: [PLAN_MACROS_SHORTCUTS.md](PLAN_MACROS_SHORTCUTS.md)) |
| Game  | G10 Social systems                                          | ✅ vertical slice (chat, party, friends — clans/mail/BBS deferred) |
| Game  | G11 Scripting engine + quests (+ clans via bypass)          | ✅ vertical slice (bypass routing, quest engine, Q00258/Q00320, clan creation — plan: [PLAN_G11_QUESTS_CLANS.md](PLAN_G11_QUESTS_CLANS.md)) |
| Game  | G12 Static world + script/content breadth                   | ✅ vertical slice (zones peace/water/no-restart, all 1180 doors + geo collision, static objects, Link/Buy bypasses, +10 quests with on_attack/on_spawn hooks, OrcChange1, TeleportWithCharm — plan: [PLAN_G12_STATIC_WORLD_AND_CONTENT_BREADTH.md](PLAN_G12_STATIC_WORLD_AND_CONTENT_BREADTH.md)) |
| Game  | G13 Long tail & parity sweep                                | ⏳ |

**Verified end-to-end:** a scripted client does the real login crypto → server
select → game `AuthLogin` → char list → **create** (with initial skills) →
reconnect → **CharacterSelect → CharSelected → EnterWorld → UserInfo + full
enter-world burst** with correct computed HP/MP, then manor / key-mapping /
skill-cooltime requests. See `crates/gameserver/tests/e2e_create.rs`.

---

## Login server (M0–M5) — ✅

Drop-in replacement for the Java login server; the unmodified Java game server
registers and interoperates. Crates: `commons` (framing, L2 crypto, config,
SQLite), `loginserver`. All crypto golden-vector tested. Parity checklist:
[LOGIN_SERVER_PARITY.md](LOGIN_SERVER_PARITY.md).

Post-M5 fixes:
- **Account case-insensitivity** (`4f29af4`): the login server now lowercases
  accounts everywhere (Java `AccountInfo._login = login.toLowerCase()`), so the
  game's lowercase `PlayerAuthRequest` matches `authed_clients`. Without it,
  mixed-case logins reached the server list but never the lobby.

---

## Game server

### G0 — Scaffold & boot ✅ (`5a8f681`)
`gameserver` crate; `Config` reads `dist/game/config/*.ini` verbatim; runs with
`dist/game` as cwd (auto-chdir); SQLite pool on the real DB; 100 ms game-thread
tick loop with id-capturing scheduler + tick-overrun metric; ctrl-c graceful
shutdown.

### G1 — Client link & cipher parity ✅ (`80d4c4d`)
Game XOR `Encryption` cipher (golden-vector verified byte-for-byte);
tokio per-connection tasks (`commons` framing); `GameClient` + `ConnectionState`;
`ProtocolVersion → KeyPacket` handshake with cipher enablement; decrypted packets
forwarded to the game thread over `NetEvent`.

### G2 — Login-link + auth ✅ (`3896fc1`)
`LoginServerThread` port (`loginlink/`): GS-link handshake (InitLS → BlowFishKey
RSA → AuthRequest → AuthResponse), relays commands/packets. Shared GS-link crypto
lifted into `commons`. Session type-state (`session.rs`, plan §3.1):
`Connecting → Authenticated`. `AuthLogin` handled on the game thread. Loads
`hexid.txt`. Real network config via **`IPConfigData` port** (`7366365`) —
`ipconfig.xml` + subnet auto-detection, so the login ServerList hands each client
the right game address.

### G3 — Character selection & persistence ✅ (`d596924`, `5fb30b1`, `98a988b`, `44fb451`)
- **DB thread** (`db.rs`): dedicated OS thread owns the SQLite pool; game thread
  sends `DbCommand`s, drains `DbEvent`s. Minimal `IdManager`.
- **Data loaders**: `ExperienceData`, `PlayerTemplateData`.
- `CharSelectionInfo` (real rows), `NewCharacter`/`CharacterCreate` (validate +
  insert with base stats/spawn), `CharacterDelete`/`Restore` (deletion timer).
  Session `InLobby`.
- **Create fixes**: match Java (no re-send of `CharSelectionInfo` after
  `CharCreateOk` — `send_list` flag); Unicode name validation;
  `RequestCharacterNameCreatable` → `ExIsCharNameCreatable`.
- **Initial skills**: `SkillTreeData` reads `StartingClass/*.xml` for level-1
  auto-get skills → `character_skills` (Mystic 5, Orc Fighter 1, …).

### G4 — Enter world ✅ core (`82c86a0`, `0121575`, `ee682cc`, `0761efe`, `a6aea48`)
- **Player model** (`model/`): composed struct built from a stored character +
  template. **Proper max HP/MP/CP = base level-table value × CON/MEN stat bonus**
  (`MaxHp/Mp/CpFinalizer`), via new `StatBonus` (`statBonus.xml`) and per-level
  HP/MP/CP tables. Verified vs. L2 (Human Fighter L1 = 126, Mystic = 98/59).
- **Packets**: `CharSelected`; full masked **`UserInfo`** (23 blocks, mask
  `[0xFF,0xFF,0xFE]`) — byte-verified against a real client capture in a unit
  test (`a6aea48`).
- **Flow**: `CharacterSelect` → `Entering` (sends `CharSelected`); `EnterWorld`
  → moves Player into `World.players`, sends the **full enter-world packet
  burst** (`enter_world.rs`) → `InGame`. `ActionData` loader (242 ids) for
  `ExBasicActionList`.
- **In-game requests handled**: `RequestManorList`→`ExSendManorList`,
  `RequestKeyMapping`→`ExUISetting`, `RequestSkillCoolTime`→`SkillCoolTime`,
  `RequestUserBanInfo` (consumed, no reply — matches Mobius null handler).

### ✅ Paperdoll & inventory bitmasks (part of G4, items landed in G5)
Replaced hardcoded paperdoll/mask values with Java-faithful enums/bitmasks:
- **`model/inventory.rs`**: `PaperdollSlot` (32 `Inventory.PAPERDOLL_*` ids) +
  `Inventory` with paperdoll getters (`object_id`/`item_id`/`visual_id`/
  `augmentation`, zero-for-empty like Java); `Player.inventory` field. Items
  themselves landed in G5.
- **`network/masks.rs`**: `AbstractMaskPacket` port — reversed
  `DEFAULT_FLAG_ARRAY = [0x80,0x40,…,0x01]` (mask 0 → 0x80), `add_mask` /
  `contains_mask` / `build_mask`, unit-tested against the known-good UserInfo
  mask bytes.
- **`enums.rs`**: `InventorySlot` (33 wire-order components incl. `LRHand`,
  mask = ordinal, `slot()` → `PaperdollSlot`) and `UserInfoType` (23 blocks,
  mask = ordinal + `block_length()`).
- **Packets driven through the enums**: `UserInfo` (mask bytes, block count,
  `init_size`, per-block lengths all derived from `UserInfoType`; byte test
  unchanged), `ExUserInfoEquipSlot` (mask built from `InventorySlot::VALUES`,
  paperdoll values read via `Player.inventory`), `CharSelectionInfo`
  (`ServerPacket.PAPERDOLL_ORDER` + its own visual/enchant slot orders).
- **Bug fixed**: `ex_user_info_equip_slot` mask byte 5 was `0x01`; slot 32 in
  reversed flag order is `0x80` — now produced by `build_mask`.

### G5 — Items & inventory ✅ vertical slice
Full itemcontainer parity (warehouse/trade/pickup/enchant/crystallization/
augmentation) is deferred; this milestone gets items flowing end-to-end the
same way G0–G4 got a vertical slice through "enter world":
- **`data/item_data.rs`**: generic StatSet-style parse of all 441
  `dist/game/data/stats/items/*.xml` files → `ItemTemplate` (id, name,
  kind, body part, weight, stackable, `type1`/`type2` computed the same way as
  the Java `Weapon`/`Armor`/`EtcItem` constructors). Combat-stat bonuses under
  `<stats>` stay unparsed (later milestone).
- **`data/initial_equipment.rs`**: `initialEquipment.xml` → starting gear per
  class.
- **`model/inventory.rs`** rewritten: real `ItemInstance`s + a paperdoll that
  stores `object_id`s into that list (mirrors Java's `PlayerInventory`
  referencing the same `Item` objects). `equip_item`/`unequip_slot` port
  `PlayerInventory.equipItem`'s slot-conflict resolution for the cases
  ordinary gear hits (two-handed weapons, full-armor vs chest+legs, dual ear/
  finger/bracelet slots) — formalwear, pet items, and arrow/bolt auto-swap are
  explicitly out of scope.
- **DB**: `items` rows load alongside every character (not just the one
  entered — `CharSelectionInfo` needs paperdoll icons for the whole select
  list too); `CreateCharacter` persists resolved starting gear; new
  fire-and-forget `DbCommand::UpdateItemLocation` for runtime equip/unequip.
- **Character creation**: replays `initialEquipment.xml` through a scratch
  `Inventory` (`add_item`/`equip_item` in XML order, exactly like Java's
  `initNewChar` loop) so slot-conflict resolution matches Java by
  construction; starting adena from `Character.ini` `StartingAdena`.
- **Packets**: `ItemList`, `InventoryUpdate`, `ExAdenaInvenCount`,
  `ExUserInfoInvenWeight` now carry real data; `ExUserInfoEquipSlot` and
  `CharSelectionInfo`'s paperdoll block needed no format changes, just real
  data behind them.
- **Runtime**: `UseItem` (0x19, gear only — potions/shots stay a no-op) and
  `RequestUnEquipItem` (0x16) toggle equip state, send `InventoryUpdate` +
  `UserInfo`, persist via `UpdateItemLocation`.
- **Bug fixed**: `IdManager`'s next-id counter only checked
  `MAX(characters.charId)`, not `MAX(items.object_id)` — on the real dev DB
  (which has items with higher object ids than any character), freshly
  allocated item ids collided with existing rows and silently failed to
  insert (only some starting items would show up). Fixed to take the max of
  both tables, matching Java's single shared `IdManager` pool.

### G6 — Stats, skills & effects ✅ vertical slice
Real combat-stat calc, persisted/learnable skills, and a working buff cast
pipeline — scoped to self-targeted skills (see below); damage-dealing effects
and combat proper wait for G9, which is where there's finally something to
hit. Full writeup + scope rationale in the design research behind this
milestone; summary:

- **`model/stats.rs`** (new): `Stat` enum (scoped subset: p/m atk+def,
  atk/cast speed, crit, evasion, accuracy, regen rates, speed — grows as later
  milestones need more, same pattern as `UserInfoType`/`InventorySlot`) and
  `BaseStat` (STR/DEX/CON/INT/WIT/MEN). `data/stat_bonus.rs` extended from
  CON/MEN-only to all six, still one `statBonus.xml` table.
- **`Player::recalculate_stats`**: real `p_atk`/`p_def`/`m_atk`/`m_def`/
  `p_atk_spd`/`m_atk_spd`/`crit_hit`/`m_crit_hit`/`evasion`/`accuracy`/
  `magic_evasion`/`magic_accuracy`/speed, ported from the Java `Stat`
  finalizers (`PAttackFinalizer`, `PDefenseFinalizer`, …): template base ×
  `BaseStat` bonus × level mod (`(level+89)/100`), then `Player.stats_add`/
  `stats_mul` (Java `CreatureStat`'s two modifier maps) folded in — this is
  what buffs push into. Replaces the G4-era placeholder (template value or 0).
  TODO(G8+): weapon/armor `<stats>` contributions — item stat bonuses aren't
  parsed yet, so this is the unarmed/naked value (same simplification G5 made
  for item stats generally).
- **Passive regen**: a 3 s fixed-rate tick (`REGEN_TICK_PERIOD`, Java
  `Formulas.getRegeneratePeriod`) over in-game players, porting
  `RegenHPFinalizer`/`MPFinalizer`/`CPFinalizer` (× a flat "standing still"
  1.1 multiplier — TODO: sit/run states, out of G7's move-only scope). New
  `StatusUpdate` server packet.
- **Skills**: `character_skills` now loads on select/enter-world and persists
  via a new fire-and-forget `DbCommand::UpsertSkill`; `Player.skills` (skill_id
  → level); real `SkillList`. `data/skill_tree.rs` extended from "level-1
  autoGet only" to the full base-class progression (`SkillLearn`:
  `get_level`/`level_up_sp`), driving a real `AcquireSkillList` and
  `RequestAcquireSkill` (`AcquireSkillType::CLASS` only — confirmed Java skips
  the trainer-NPC check for `CLASS`, so learning needs no village-master NPC).
- **Effects**: `model/skill.rs`'s `StatModifierEffect{stat, mode, amount}` is
  the Rust counterpart of Java's `AbstractStatAddEffect`/
  `AbstractStatPercentEffect` — one generic type instead of the 63 one-line
  subclasses Java has. `data/skill_data.rs`: a generic per-level-value XML
  loader for `data/stats/skills/*.xml`, with a curated `<effect name>` → `Stat`
  registry (18 names — `PAtk`, `PhysicalDefence`, `HpRegen`, …; unregistered
  names, e.g. the damage effects, are dropped and the skill still loads).
  Buffs live in `Player.buffs`, expire via a new `ScheduledTask::BuffExpire`.
  Real `AbnormalStatusUpdate` (self-only — no known-list yet for
  `ExAbnormalStatusUpdateFromTarget`).
- **Cast pipeline** *(superseded by G7.5 below — real 3-phase timing,
  targeting, reuse, abort)*: `RequestMagicSkillUse` → a 2-phase scheduled flow
  (`ScheduledTask::SkillLaunch` at `hit_time`, then `finishSkill` inline — no
  separate cancel-time wait, since G6 only handles instant `SELF`-targeting)
  porting `SkillCaster`: MP/HP checks at both start and landing,
  `MagicSkillUse`/`SetupGauge` → `MagicSkillLaunched` → `StatusUpdate` +
  `AbnormalStatusUpdate`. Scoped to `TargetType::SELF`, `OperateType::Active`
  known skills — other targeting, passive/toggle skills, and damage effects
  are out of scope (no NPCs/combat/visibility to aim at yet; see G9).
- **Tests**: `data::skill_tree::tests` (learn-list gating by level/known-skill);
  a synthetic-`World` test (`game_loop::tests::
  learn_and_cast_buff_skill_applies_and_expires`, no sockets, per the tick-
  system testing strategy) drives the real handlers end-to-end — learn
  "Defense Aura" (SP spend + level gate) → cast it → land (P.Def +8%, right
  packet sequence) → fast-forward `world.tick` past `abnormalTime` → expire
  (P.Def back to naked) — since real-time-waiting out a 20+ minute retail buff
  isn't a reasonable thing for a unit test to do.
- **`e2e_create.rs` fix**: the new regen tick can push an unsolicited
  `StatusUpdate` mid-test once a character is in-game (e.g. CP regenerating
  from its post-creation 0); added `GameClient::recv_skip_status_update` so
  reply-then-assert exchanges after enter-world aren't thrown off by it.

### G7 — Movement & targeting (no geodata) ✅
Scoped-down slice of the vertical-slice gate's original "movement +
known-list" gap: player-to-player targeting and click-to-move, both trusting
the client outright (no geodata/pathfinding validation yet — see the
deferred-TODO note below).

- **`Player` fields**: `target: Option<i32>` (targeted object id — Player-only,
  no NPCs/items exist as `WorldObject`s yet) and `move_data: Option<MoveData>`
  (`model/movement.rs`, a geodata-free port of Java's nullable `Creature._move`
  — start/dest x/y/z, `start_tick`, `total_ticks`).
- **Targeting**: `Action` (0x1F) resolves a click to another in-world player
  and calls `set_target`, a narrowed port of `Player.setTarget` (skips the
  party/vehicle/GM checks — neither exist yet): same-target re-click is a
  no-op; a real change sends `MyTargetSelected` + a `StatusUpdate`(HP) to the
  selector and broadcasts `TargetSelected` to everyone else; clearing
  broadcasts `TargetUnselected`. `RequestTargetCanceld` (0x48) reads the
  `targetLost` flag and clears the target the same way. Every `Action` ends
  with the `ActionFailed` terminator, matching `WorldObject.onAction`'s
  convention (**`ActionFailed`/opcode `0x1F` server packet added** — didn't
  exist before this milestone).
- **Movement**: `MoveBackwardToLocation` (0x0F) ports the
  `Creature.moveToLocation` math minus the entire geodata/pathfinding block
  (`Creature.java` ~3651-3816) — same-origin/target → `StopMove`; max
  click-distance (9900²) and `player.casting` are the only guards kept (the
  rest of Java's `isMovementDisabled()` — rooted/overloaded/immobilized/dead/
  teleporting — has no state to check yet); otherwise computes heading
  (`Util.calculateHeadingFrom` port) and `total_ticks` from distance/speed,
  sets `move_data`, and broadcasts one `MoveToLocation` to other players (the
  mover self-predicts, per Java — no packet sent back to itself). A new
  per-tick system (`movement::tick`, called unconditionally every 100 ms
  iteration, unlike the gated `REGEN_TICK_PERIOD` systems) interpolates
  position each tick and snaps to the destination on arrival — no `StopMove`
  broadcast needed then, since the client already predicted it.
- **Broadcast stopgap**: `broadcast_to_others` (`game_loop.rs`) sends to every
  connected in-game player except the actor — a flat pass, not a real
  known-list/region-grid (superseded by G7.9's region-scoped visibility).
- **Tests**: synthetic-`World` unit tests (`game_loop::tests`) —
  `action_selects_switches_and_cancels_target` (select/re-click no-op/cancel,
  checking both the selector's and the target's packet streams) and
  `move_backward_to_location_interpolates_and_arrives` (mid-flight
  interpolation + exact arrival snap, verifying the bystander gets
  `MoveToLocation` but the mover doesn't) plus the same-origin `StopMove` case.

### G7.5 — Full single-target skill casting ✅
Supersedes G6's self-only 2-phase cast slice with a faithful port of the
`RequestMagicSkillUse` → `Player.useMagic` → `SkillCaster` pipeline: casting
on the current target (players only — still no NPCs), Java's real timing and
damage math, server-side reuse enforcement, and cast interruption.

- **`model/formulas.rs`** (new): ports of `Formulas.calcMagicDam`
  (`77·power·√mAtk/mDef`, ×2 on magic crit), `calcCrit`'s magic branch
  (per-mille rate, 320/200 caps), `calcSkillTimeFactor`/`calcSkillCancelTime`/
  `calcAtkSpd` (casting-speed-scaled `hitTime`, 500 ms launch floor, cool
  phase), `Heal.java`'s `power + √(2·mAtk)` (×3 crit), and `calcAtkBreak`
  (cast break on hit). Each fn doc-comments the dropped terms — all identity
  for unarmed/shotless players (shots, traits, attribute, pvp/pve config
  multipliers). The `ALT_GAME_MAGICFAILURES` resist branch is deferred
  (equivalent to `MagicFailures = False`).
- **3-phase cast state machine**: `Player.cast: Option<CastState>` (replaces
  `casting: bool`; snapshots skill/target/timings) + `cast_seq` generation
  counter. `startCasting` (reuse registration, stop-move, `ExRotation` target
  facing, initial MP, broadcast `MagicSkillUse`, SM 46 + `SetupGauge`) →
  `SkillLaunch` at `hit` (effect-range re-check → SM 748 quiet stop;
  broadcast `MagicSkillLaunched`; marks the cast unabortable) → `SkillFinish`
  at `+cancel` (MP/HP consume with SM 23/24 on shortfall, effect application)
  → `CastEnd` at `+cool`. Scheduled tasks carry `cast_seq` and no-op on
  mismatch — aborting is just clearing `Player.cast`, no heap surgery.
- **Abort/interrupt**: `abort_cast` (port of `Creature.abortCast` →
  `stopCasting(true)`, pre-launch only): broadcast `MagicSkillCanceled`
  (new packet, 0x49) + `ActionFailed`. Wired to Esc
  (`RequestTargetCanceld`, which Java aborts on regardless of the
  `targetLost` flag) and to incoming magic damage via `calcAtkBreak`
  (SM 27). Movement during a cast stays blocked with `ActionFailed`
  (`PlayerAI.onIntentionMoveTo` semantics — it does *not* abort).
- **Reuse**: `Player.reuses` (`Skill::reuse_key()` — the shared
  `reuseDelayGroup` when set, else skill id — → `SkillReuse`, one map for
  Java's `_reuseTimeStampsSkills`/`_disabledSkills` split), registered at
  cast start, checked lazily in the `useMagic` gate — SM 48 for short
  reuses, SM 2303/2304/2305 with the h/m/s breakdown for >3 s ones. Real
  `SkillCoolTime` packet (enter-world + `RequestSkillCoolTime`).
  Persistence across relog still deferred.
- **Targeting**: `resolve_cast_target` — static match port of the
  `Self`/`Target`/`Enemy`/`EnemyOnly` target-handler scripts (players only,
  no geodata LOS/peace zones; with no PvP flags an `ENEMY` cast always needs
  ctrl/force-use). Cast-range gate ports `Util.checkIfInRange` with collision
  radii (out-of-range = `ActionFailed`; Java's walk-into-range AI was not
  ported at the time — done post-G9.5 via `PlayerIntent::Cast`).
- **Effects**: `SkillEffect` enum (`StatModifier` | `MagicalAttack` |
  `Heal`) replaces the stat-modifier-only effect list; buffs now land on the
  *resolved target* (buff-a-friend works). Magic damage drains **CP first**
  then HP (`PlayerStatus.reduceHp`), clamped at 1.0 HP — no death system
  yet (TODO G9 `doDie`) — with SM 2261/2262 damage messages + `M_CRITICAL`.
  Heals overheal-clamp and send SM 1066/1067.
- **Packets**: parameterized `SystemMessage` builder
  (`system_message_with` + `SmParam` Text/Int/SkillName/PlayerName, `sm_ids`
  constants), `MagicSkillUse` with real target fields, multi-target
  `MagicSkillLaunched`, `MagicSkillCanceled`, real `SkillCoolTime`;
  `RequestMagicSkillUse` now reads `shiftPressed`. `World.rng` + `roll()`
  (test hook: `forced_rolls`) for the crit/break dice.
- **Skill-XML loader fix**: the `<list>` document root was being pushed onto
  the parser's tag stack, shifting every depth check by one — **the loader
  parsed 0 skills from the real dist XMLs** (G6's tests bypassed it with
  `insert_for_test`, hiding it). Now guarded + regression-tested against the
  real files (`loads_real_dist_files`, >10 000 skill levels). Parser also
  reads per-level `targetType`, `isMagic`, `effectPoint`, `hitCancelTime`,
  and `<power>` effect params.
- **Tests**: `formulas` unit tests with exact Java values; parser tests
  (Wind-Strike/Heal-shaped XML); synthetic-`World` integration tests for the
  full nuke-on-player flow (exact damage, CP-first, both packet streams,
  reuse gate), no-ctrl/out-of-range rejections, HP clamp, Esc abort +
  stale-task no-op + reuse surviving the abort, effect-range re-check,
  heal-with-formula + overheal clamp, buff-on-other + expiry, quiet
  finish-phase MP failure, `SkillCoolTime` contents, and damage breaking a
  victim's pre-launch cast.

### G7.8 — Geodata & position validation ✅
Closes G7's "trust the client outright" gap: the stock `.l2j` geodata files
now load and back server-side LOS + walkability checks.

- **`geo/` module** (`mod.rs`, `region.rs`, `line.rs`): port of
  `geoengine/GeoEngine` + `geodata/GeoData`/`regions/Region`/`blocks/*` and
  the `LinePointIterator`/`3D` cell walkers. Unlike Java's eager
  multi-GB block-object parse, each region file is **mmap'd read-only**
  (`memmap2`) and queried in place; the only parsed state is a 64K-entry
  block-offset index built in one validation pass (plan §risks: "mmap +
  read-only shared geodata"). Flat/complex/multilayer blocks, NSWE checks
  (incl. `checkNearestNsweAntiCornerCut`, Java's NW quirk kept for parity),
  `getNearestZ`/`getNextLowerZ`/`getNextHigherZ`, `getSpawnHeight`,
  `canSeeTarget` (48-unit see-over, elevated-origin allowance),
  `canMoveToTarget`, `getValidLocation`. Not ported: door/fence LOS
  carve-outs (no doors/fences yet), runtime NSWE editing.
  (`CellPathFinding` landed later as G7.85 — see below.)
- **Boot**: new `config/geoengine.rs` reads `GeoEngine.ini` (`GeoDataPath`,
  `PathFinding`); `main.rs` prints the Geodata section and loads all 227
  dist regions (~2.5 s, debug) into `World.geo` (`GeoEngine::empty()` =
  Java `NullRegion` everywhere for tests).
- **Movement** (`handle_move_backward_to_location`): ports
  `Creature.moveToLocation`'s geodata block — destination clamped via
  `getValidLocation` (players keep client z, far-click > 3000 and
  fall-intent guards honored), fully-clamped moves canceled with
  `ActionFailed`. (The pathfinding fallback — Java walks around an
  obstacle when the clamp shortened the path > 30 — landed as G7.85.)
- **`ValidatePosition` (0x59)** — previously unhandled: full
  `runImpl` reconciliation (trust-the-climb z adoption, moderate-drift
  `ValidateLocation` correction (new packet, 0x79), out-of-sync snap with
  geodata z pull-down), storing `Player.client_x/y/z/heading`. Vehicle/
  falling/flying/water/observer/Blink branches skipped (states don't exist).
- **Casting LOS**: `resolve_cast_target` now returns `Result` and ends with
  the target handlers' "Geodata check when character is within range" —
  `canSeeTarget` failure → SM 181 (`CANNOT_SEE_TARGET`) + `ActionFailed`
  (self-target bypasses, per `Target.java`).
- **Tests**: region cell-encoding/block-type/corruption units; line-walker
  units; synthetic-region wall & low-fence LOS/movement/`getValidLocation`
  behavior; real-dist load smoke test (Giran ground z, open-square LOS,
  spawn snap); game-loop tests for move clamping, blocked-move cancel,
  SM 181 on cast through a wall, and the three `ValidatePosition` branches.
  Also fixed a test-suite race: dist-loading tests now use absolute
  `CARGO_MANIFEST_DIR` paths (the ipconfig test chdirs the process
  mid-run and could starve relative-path loaders).

### G7.85 — Pathfinding (path-worker service) ✅
Closes G7.8's "walks up to the obstacle and stops" gap: blocked player
moves now route around obstacles via the `CellPathFinding` port, running
on a dedicated worker thread per CONCURRENCY_MODEL §2.4 (the game thread
never blocks on a path search).

- **`geo/path.rs`**: pure-function port of `CellNodeBuffer` (best-first
  search with the cost-sorted-chain open list, arena-allocated nodes
  instead of Java's object graph, all weights/`MAX_ITERATIONS`/z-keying
  quirks kept) + `CellPathFinding.findPath` (buffer sizing from
  `PathFindBuffers`, `constructPath` direction-change compression, the
  `canMoveToTarget` postfilter with its playable/AI pass asymmetry).
  Java's cross-thread buffer pool is collapsed to "smallest configured
  size that fits, allocated fresh" — single worker, so pooling buys
  nothing; the size ceiling (too-far request ⇒ no path) is preserved.
- **`geo/worker.rs`**: the path-worker thread. `PathRequest` in via
  `std::sync::mpsc`, `PathEvent` back to the game loop, drained per tick
  (`drain_path`, same shape as `drain_db`). `World.geo` became
  `Arc<GeoEngine>` so the worker shares the mmap'd geodata read-only.
- **Async move flow** (`position.rs`): when the `getValidLocation` clamp
  shortens a click by > 30 units, the handler stores a `PathWait { seq }`
  component and sends the *original* destination to the worker instead of
  starting the move; the reply (`handle_path_result`) either starts a
  route move or answers `ActionFailed` (no path — Java's player branch).
  Stale replies (player re-clicked → newer seq, or left) are dropped;
  re-clicks onto the geo cell already being pathed to are ignored and
  clicks elsewhere abandon route following, both per Java
  `isOnGeodataPath()`. The one-tick (~100 ms) confirmation delay replaces
  Java's synchronous in-handler search.
- **Route following** (`model/movement.rs`): `MoveData.geo_path`
  (`points`/`index`/`accurateTx/Ty`/`gtx/gty` as one `Option<GeoPath>`);
  segment completion in the movement tick runs `moveToNextRoutePoint`
  (next dest — accurate destination on the final segment — ticks
  recomputed at current speed, heading updated) and the caller broadcasts
  `MoveToLocation` per segment.
- **Config/boot**: `config/geoengine.rs` now reads the full tuning block
  (`PathFindBuffers`, `Low/Medium/High/DiagonalWeight`,
  `AdvancedDiagonalStrategy`, `MaxPostfilterPasses`) into a `PathConfig`;
  `main.rs` spawns the worker with a clone of the geodata `Arc` and joins
  it at shutdown (channel close stops it).
- **Not ported yet**: NPC moves stay straight-line (Java also paths
  chase/return-home moves and has the Attackable closest-reachable-point
  grid scan); `GeoPathFinding` (`PathFinding = 1` node files — Java's own
  default is 2, cell pathfinding); debug-item drops and `getStat()`
  counters.
- **Tests**: algorithm units on synthetic regions (walk-around through a
  wall gap with every postfiltered leg verified walkable, sealed wall ⇒
  `None`, no-geodata ⇒ `None`, over-buffer distance ⇒ `None`) + a
  real-dist Giran route; game-loop tests for the deferral (PathWait, no
  packet until reply) and a full round-trip against a live worker thread
  (click across a wall → route move with several segments →
  `MoveToLocation` per advance → arrival at the exact requested
  destination).

### Post-G7.8 — Restart/Logout + player persistence ✅
Fixed "relogin ignored": the client's `RequestRestart` (0x57) and `Logout`
(0x00) opcodes were unhandled, so leaving the world was impossible without
killing the client.

- **`RequestRestart`**: Java `storeMe().deleteMe()` + `RestartResponse.TRUE`,
  session `InGame → Authenticated` (new type-state transition; `InGame` now
  carries the `SessionKey` for it), then the character list reloads through
  the normal `Authenticated → InLobby` path. `canLogout` guards (attack
  stance, NO_RESTART zones) are TODO with combat (G9).
- **`Logout`**: store + remove player, send `LeaveWorld` (0x84), drop the
  session (socket closes after the flush; `on_disconnect` does the login-
  server notify). From the lobby it just disconnects, like Java.
- **Persistence** (`DbCommand::StorePlayer` + `PlayerSnapshot`): port of
  `Player.storeCharBase` narrowed to tracked columns (level, HP/MP/CP,
  position/heading, exp/sp, reputation, PvP/PK, class ids, vitality) +
  `updateOnlineStatus` (`online=0`, `lastAccess=now`) in one UPDATE. Runs on
  restart, logout, **and unexpected disconnect** (incl. the `Entering`
  state, where the `Player` still lives on the session). `storeCharSub`/
  `storeEffect`/item-reuse persistence deferred (need subclasses and
  buff restore on login).
- **Tests**: restart store+lobby round trip, restart → re-enter world (the
  original bug), logout store+`LeaveWorld`, disconnect store.

### Post-G7.8 — Skill reuse groups ✅
Fixed "every skill icon refreshes on any cast": `MagicSkillUse` (and
`SkillList`) hardcoded the reuse-group field to 0, which the client treats as
a shared everything-group; Java sends `Skill.reuseDelayGroup` (default **-1**
= ungrouped).

- **`Skill.reuse_delay_group`**: parsed from `<reuseDelayGroup>` (default -1)
  and written raw into `MagicSkillUse` and `SkillList`.
- **Shared cooldowns**: `Player.reuses` is now keyed by `Skill::reuse_key()`
  (group id when positive, else skill id — Java's `_reuseHashCode` minus the
  per-level dimension), value is a `SkillReuse` carrying the cast level so
  `SkillCoolTime` can report `group-or-id + level` like Java.
- **Tests**: ungrouped casts assert the -1 group byte in `MagicSkillUse`;
  grouped siblings share one cooldown (gate + `SkillCoolTime` group id);
  `loads_real_dist_files` probes a real grouped skill (10248 → group 10008).

### G7.9 — Region-grid visibility & scoped broadcasting ✅

Port of Java's world-region knownlist for player↔player visibility — the
first time two clients actually see each other's characters.

- **Region math** (`world.rs`): `REGION_SHIFT` (Java `World.SHIFT_BY` = 11 ⇒
  2048-unit cells), `region_of(x, y)`, and `regions_adjacent` (the 3×3
  surrounding-region rule, Java `WorldRegion.isSurroundingRegion`). Java's
  per-region object lists are *not* materialized: with players as the only
  world objects, each `Player` carries its current region cell
  (`Player.region`, kept in sync by `game_loop/visibility.rs`) and every
  query is an adjacency compare — identical semantics, no grid to keep
  consistent. The real grid collections can arrive with G8 NPC counts.
- **`CharInfo` (0x31) + `DeleteObject` (0x08)** (`server_packets.rs`): the
  full Interlude-Classic `CharInfo` layout (paperdoll/augment/visual orders
  included; clan/mount/store/cubic/fishing fields as empty Java defaults).
- **Scoped broadcasting** (`game_loop/helpers.rs`): `broadcast_to_others` /
  `broadcast_including_self` now send only to players whose region is
  adjacent to the broadcaster's (Java `broadcastPacket` via
  `World.forEachVisibleObject`), replacing the flat all-clients pass.
- **Visibility lifecycle** (`game_loop/visibility.rs`): `on_enter_world`
  (Java `spawnMe` → `addVisibleObject`: mutual `CharInfo`), `update_region`
  (Java `updateWorldRegion` → `switchRegion`: `DeleteObject`/`CharInfo`
  deltas both ways, dangling-target clearing, and
  `describeStateToPlayer`-style `MoveToLocation` for movers entering view),
  `on_leave_world` (Java `removeVisibleObject`: `DeleteObject` to watchers on
  logout/restart/disconnect). Hooked into the movement tick
  (`visibility::movement_tick` wraps `movement::tick`), the
  `ValidatePosition` out-of-sync snap, `handle_enter_world`, and
  `store_and_remove_player`.
- **Tests** (`game_loop::tests`): enter-world CharInfo exchange scoped by
  region, broadcast scoping (near vs far bystander), region-crossing
  `DeleteObject`/`CharInfo` + mid-move introduction, and leave-world
  `DeleteObject` + target drop.

### G8 — Static world content (NPCs/spawns) ✅ vertical slice
The world is no longer empty: every static spawn line places a live NPC that
players can see, target, and talk to. Scoped to what makes NPCs *exist* —
zones, doors, static objects, respawn, and any NPC behaviour (AI, random walk,
combat) are deferred (respawn is unreachable anyway until G9's `doDie` gives
NPCs a way to die).

- **`data/npc_data.rs`**: port of `NpcData` — all 191 `data/stats/npcs/*.xml`
  files → 14 407 `NpcTemplate`s (identity/display fields, base stats/vitals/
  speeds, collision, equipment rhand/lhand, status flags, aggro ranges;
  skill/drop/attribute lists wait for G9). Type classification
  (`is_monster`/`is_attackable_class`) mirrors Java's `instanceof
  Monster`/`Attackable` subtree checks — there's no class hierarchy to lean
  on, so the `type` attribute is matched against the instance-class sets.
- **`data/spawn_data.rs`**: port of `SpawnData`/`model/spawns/*` — all 154
  `data/spawns/**` files → 27 154 spawn lines (fixed locations, `count`,
  `respawnTime`/`respawnRandom` durations, spawn- and group-level
  `<territories>` with the NPoly/Cuboid/Cylinder `ZoneForm`s). Features with
  zero usages in this dist are not ported (`zone=`, `banned_territory`,
  `<locations>`, `<minions>`, `respawnPattern`); `dbSave` raid persistence
  (`DBSpawnManager`, 225 lines) spawns statically for now.
- **`model/npc.rs`**: the composed `Npc` world object (position/region/
  HP/MP; everything else reads through the template) + `spawn_all`, the
  `Spawn.doSpawn`/`initializeNpc` port: territory spawns get a random point
  (bounding-box rejection sampling, Java's 1000-try cap) at
  `GeoEngine.getHeight`, monsters snap to the geodata surface (<300 units),
  `heading == -1` randomizes with Java's odd `Rnd.get(61794)` bound.
  Boot places **34 869 NPCs** in ~1 s (891 lines skipped: Servitor/Pet/
  Defender/Decoy/Trap plus types with no instance class — those fail
  reflection on the Java server too). NPC object ids come from a dedicated
  transient base (`0x4000_0000`) instead of Java's shared `IdManager` pool
  (the pool lives on the DB thread; NPCs never persist).
- **`World`**: `npcs` registry + `npc_regions` — the first materialized
  region-grid collection (players still use the per-player adjacency compare;
  NPCs are static and 34.9k strong, so the index is built once at spawn).
- **`NpcInfo` (0x0C)** (`server_packets.rs`): the masked packet (5 mask
  bytes, "mask_bits_37", pre-set gap components) via the shared `masks.rs`
  helpers + a new `NpcInfoType` enum (explicit non-contiguous discriminants).
  Component selection ports the Java constructor with absent systems at their
  defaults. Unit-tested against hand-computed bytes (no NPC client capture
  yet — the mask math is shared with the byte-verified `UserInfo` path).
  `write_f32` added to `commons::PacketWriter` for the speed multipliers.
- **Visibility** (`visibility.rs`): enter-world sends `NpcInfo` for the 3×3
  region block; region crossings send `NpcInfo`/`DeleteObject` deltas both
  ways and drop dangling NPC targets (players get nothing new from NPCs —
  aggro/AI eyes are G9).
- **Targeting/interaction** (`target.rs`): `Action` resolves NPCs —
  `Player.setTarget` generalized over players and NPCs (`ValidateLocation` +
  `MyTargetSelected` with the level-diff color for auto-attackable targets +
  HP `StatusUpdate` + `TargetSelected` broadcast; z-diff and `targetable`
  guards). Second click = the `NpcAction` interact branch: monsters no-op
  (attack intent is G9), others within `INTERACTION_DISTANCE` (250) get
  `Npc.showChatWindow` — `NpcHtmlMessage` (0x19) from
  `data/html/<type-dir>/{id}.htm` with the Folk `npcdefault.htm` fallback and
  `%objectId%`/`%npcname%` replacement (read per interaction; no `HtmCache`).
  Out-of-range clicks walk in first (`PlayerIntent::Interact`, `combat.rs`'s
  `start_interact_intent`/`player_interact_think` — same chase-then-act shape
  as the cast/attack intents) and re-run the interact click on arrival, same
  as Java's `doInteract` re-dispatching `onAction`.
- **Tests**: loader tests against the real dist (counts + hand-checked
  templates/spawn lines, elemental `<attribute>` vs base `<defence>`
  disambiguation, duration parsing, NPoly containment); `spawn_all` smoke
  test over the real datapack (placement count, retail coordinates, region-
  index consistency); `NpcInfo` byte test; synthetic-world tests for
  enter-world NPC burst scoping, region-cross deltas + NPC-target drop, and
  the two-click select→chat-window / monster-no-chat flows. `e2e_create`'s
  skip-unsolicited helper now also skips `NpcInfo` (the starting village's
  NPCs arrive in the enter-world burst).

### G9 — Combat & AI ✅ vertical slice
The G9 gate end-to-end: kill a monster (melee and skill), take damage back,
receive XP/SP/loot, level up, die, and revive in town. Scoped to melee
single-hit combat and plain monsters — see the deferred list for what
consciously stayed out.

- **Config** (`config/rates.rs`, `config/npc.rs`, `character.rs` grown):
  `Rates.ini` (XP/SP ×50 on this dist!, drop chance/amount multipliers incl.
  the per-item `57,50;…` lists, `DropMaxOccurrences*`, the drop level-gap
  window keys), `NPC.ini` (`DefaultCorpseTime`, `MaxDriftRange`),
  `Character.ini` (`AutoLoot` — **True** on this dist, `RespawnRestoreHP` 65,
  `AltPartyRange`, `Delevel`/`DelevelMinimum`, `RandomRespawnInTownEnabled`).
  Bundled as `CombatConfig` on `World.cfg` (tests get Java defaults, ×1
  rates).
- **Data loaders**: `hit_condition_bonus.rs` (front/side/back/high/low —
  night/rain need a game clock/weather), `xp_lost.rs`
  (`playerXpPercentLost.xml`), `map_region.rs` (`data/mapregion/*` tiles +
  town respawn points, `talking_island_town` fallback); `npc_data.rs` grown:
  `<attack random critical>`, `<corpseTime>`, `<dropLists>` (`<drop>` lines
  + the `<group chance>` shape the Primeval Isle file uses; spoil dropped).
- **Physical formulas** (`model/formulas.rs`): `calculateTimeBetweenAttacks`
  (`500000/atkSpd`, 50 ms floor), melee `calculateTimeToHit` (0.644/0.735),
  `calcHitMiss` (`(80+2(acc−evasion))·10` × HitConditionBonus, clamp
  [200,980]), auto-attack `calcCrit` (position 1.1/1.3 + height bonus, clamp
  [3,97]), `calcAutoAttackDamage` (`(pAtk·rnd + proxBonus)·77/pDef`, crit ×2
  — soulshot/shield/ranged/trait terms identity and documented), the
  level-gap XP table, `Util.map` for the drop level gates. `Position`
  (front/side/back from headings) in `movement.rs`.
- **Auto-attack pipeline** (`game_loop/combat.rs`): `AttackRequest` (0x32) /
  second `Action` click on a monster → `PlayerIntent::Attack` — a per-tick
  think (`PlayerAI.thinkAttack` + the 500 ms follow cadence) that chases via
  `MoveToPawn` and swings with `Creature.doAutoAttack`'s shape: hit rolled at
  swing start (`generateHit`), `Attack` (0x33) broadcast, damage landing on a
  scheduled `AttackHit` at `timeToHit` (in-flight swings die with either
  side). Shared `Combatant` view derives NPC stats from templates through
  the same finalizer math (STR/DEX bonuses × level mod). Combat stance
  tracker (`AutoAttackStart/Stop` 0x25/0x26, 15 s), damage messages
  (SM 2261/2262/2264/2265/2266 + miss/crit), CP soak only from playable
  attackers, cast-break on hit. Magic damage now routes through the same
  receivers — the G7.5 "clamp at 1.0 HP" is gone.
- **Monster AI** (`game_loop/npc_ai.rs`): 1 s think over monsters in active
  regions (player-adjacent cells only, Java's region-activation gate).
  `thinkActive`: `_globalAggro` −10→0 spawn calm, aggro-range scan (alive +
  region-adjacent + LOS) seeding 1 hate, most-hated pick → run mode
  (`ChangeMoveType` 0x28) + Attack intention; drift-home walk past
  `MaxDriftRange`. `thinkAttack`: 120 s attack timeout (walks home — Java
  teleports), hate pruning on dead targets, chase (`MoveToPawn` re-pathed per
  think) and swing through the shared pipeline. NPC movement rides the
  interpolation tick with `npc_regions` re-indexing + `NpcInfo`/
  `DeleteObject` visibility deltas on cell crossings.
- **Death/decay/respawn** (`game_loop/death.rs`): `doDie` both kinds (`Die`
  0x00 broadcast; players get the to-village flag + XP penalty via
  `playerXpPercentLost` with the `Delevel` clamp; dead players are barred
  from move/cast/attack and regen). NPC corpse decays after
  `<corpseTime>`/`DefaultCorpseTime` (`DeleteObject`, dangling targets
  dropped), `Spawn.decreaseCount` schedules the respawn (min/max random
  spread) and the spawn line re-runs — fresh transient object id, a
  documented deviation from Java's id-reusing `respawnNpc`.
- **Rewards**: `calculateRewards` from the aggro damage shares (solo-only —
  parties don't exist), `ALT_PARTY_RANGE`/surrounding-region gates,
  level-gap multiplier, ×`RateXp/RateSp`; `addExpAndSp` (SM 3259) with the
  `PlayableStat.addExp` level scan → `addLevel`: vitals re-derived, CP
  refill, autoGet skill grants (`rewardSkills`), `SocialAction` 2122 + SM 96
  + StatusUpdate/UserInfo/SkillList. Drops: `calculateDrops` port (level-gap
  gates, per-item chance/amount multipliers, occurrence cap — the cap's
  mid-list reshuffle simplified to a hard stop) **auto-looted** into the
  killer's inventory (SM 28/29/30 + InventoryUpdate) — the dist runs
  `AutoLoot = True`; ground drops wait for item-on-ground world objects.
  Runtime item ids come from DB-thread-reserved blocks
  (`DbEvent::IdBlock`/`DbCommand::ReserveIds` — Java `IdManager` semantics
  without a per-item round trip); new `InsertItem`/`UpdateItemCount`
  persistence.
- **Die → revive loop**: `RequestRestartPoint` (0x7D, TO_VILLAGE) → map
  region town respawn → `teleport_player` (`TeleportToLocation` 0x22 +
  `decayMe`-style DeleteObject) → client `Appearing` (0x3A) → `doRevive`
  (65% HP restore, `Revive` 0x01) + `spawnMe` visibility exchange + fresh
  UserInfo. Dead-on-login characters get their death dialog back
  (`EnterWorld` → `Die`).
- **Casting on NPCs**: `resolve_cast_target` resolves both registries
  (monsters are valid `Enemy` targets without ctrl), `MagicSkillUse` carries
  NPC target coords, NPC `mDef` through the `MDefenseFinalizer` shape; buffs
  on NPC targets are dropped (no NPC effect list — nothing casts on them
  yet).
- **Tests**: formula units with exact Java values; loader tests against the
  real dist (Gremlin `random`/`critical`, Goblin's 9 drop lines + 450 aggro
  range, Santa's `<corpseTime>3`, grouped drops, xp-lost + hit-condition
  tables, Giran map-region respawn); synthetic-world integration tests
  driving the real tick systems — the full melee kill
  (Attack/stance/Die/XP/level-up/adena auto-loot + DB insert/decay),
  out-of-reach chase + monster retaliation (run mode, `MoveToPawn`, HP bite
  with no CP soak), unprovoked aggro on an idle player, kill-by-nuke through
  the same death path, player death (penalty + to-village `Die`) →
  restart-point teleport → `Appearing` revive at 65%, and decay → respawn
  with a fresh id announced by `NpcInfo`.

### Post-G9 — ECS object storage (`bevy_ecs`) ✅
The world's object registries were refactored onto an **ECS
(Entity–Component–System)** backbone using the standalone `bevy_ecs` crate —
see [CONCURRENCY_MODEL.md §2.8](CONCURRENCY_MODEL.md) for the pattern
rationale (dense archetype-table iteration for the per-tick sweeps instead of
HashMap bucket walks).

- **`store.rs`** (new): `EntityStore<T>` — a `bevy_ecs::World` whose entities
  carry the game object as a component, an `object_id → Entity` index for
  O(1) id lookups, and a cached `QueryState` so `values_mut()` (the
  regen/movement/AI tick sweeps) is dense table iteration. Exposes the
  HashMap-shaped API the handlers were written against (`get`/`get_mut`/
  `insert`/`remove`/`values`/`values_mut`/`Index`/…), so call sites and the
  single-owner model are unchanged.
- **`World.players` / `World.npcs`**: `HashMap<i32, T>` → `EntityStore<T>`;
  `Player` and `Npc` derive `Component` (one fat component per entity —
  stage 1; component splitting + one merged world + `Schedule`-driven systems
  are the documented stage 2).
- **Tests**: `store::tests` (roundtrip + iteration); the whole existing suite
  runs against the ECS-backed stores unchanged.

### G9.5 — ECS stage 2: split components, one world ✅
Plan: [PLAN_ECS_STAGE2.md](PLAN_ECS_STAGE2.md); executed in the planned
split-first/merge-second phases, each gated on the full (behavior-level)
test suite — no gameplay change.

- **Components** (`model/components.rs`), split along system access seams:
  shared `Position`, `RegionCell`, `Vitals` (HP/MP + `dead`), `Speeds`,
  `Collision`, `CombatStats`, `AttackState`; presence-based `Movement`/
  `Casting`/`Intent` (insert = state starts, remove = it ends — the
  movement tick sweeps only entities carrying `Movement` instead of
  scanning 34.9k static NPCs' `None`s, and the player combat tick sweeps
  only intent-holders); player-only `PlayerVitals` (CP), `BaseStats`,
  `StatModifiers`, `Buffs`, `Inventory`, `SkillBook`, `Reuses`, `TargetRef`,
  `ClientPos`; NPC-only `NpcAi`, `AggroList`.
- **One world** (`store.rs`): `World.players`/`World.npcs` →
  `World.objects: EntityStore` (non-generic) — one `bevy_ecs::World`, one
  id → `Entity` index (`npc_regions` unchanged). API:
  `spawn`/`despawn`/`get_component(_mut)`/`get_many_mut`/`has_component`/
  `add_components`/`remove_component`/`for_each_mut`/`count`. Object ids
  stay the only foreign key; `Entity` never leaves `store.rs`.
- **Residual cores as markers:** `Player`/`Npc` shrank to identity +
  bookkeeping nothing sweeps and double as the kind markers (the plan's
  separate `PlayerTag`/`NpcTag` were redundant). `combat::combatant()` is
  one component fetch for both kinds — NPC stats are memoized into
  `CombatStats` at spawn (`npc_combat_stats`, same finalizer math as the
  deleted per-call template derivation, m_def included for the magic path).
- **Movement unification:** one sweep advances every mover (player or NPC),
  returning moved-NPC ids for region re-indexing — the duplicated
  `tick`/`tick_npcs` pair is gone.
- **Boundary DTO:** `PlayerData` (né `PlayerBundle`) carries the full
  component set outside the ECS (from_char → `Entering` session →
  `spawn_into` at EnterWorld); `PlayerView` is its borrowed read-side for
  packet builders (UserInfo/CharInfo/CharSelected take one view arg, not
  eight components). Persistence (`PlayerSnapshot`) and NPC decay gather
  state from components *before* `despawn` — the old `remove() → whole
  struct` shape is gone.
- **Plan deviations:** kind markers folded into the residual cores (no
  zero-sized tags); `pair_mut` never materialized (no call site holds two
  entities' components mutably at once — the sequential re-fetch shape the
  handlers already had survived the merge); `SparseSet` storage fallback
  not needed. Known bevy quirk documented on `get_many_mut`: `Option<&C>`
  errors for never-registered `C` (probe with `has_component` instead).
- **Verified:** full suite green (147 tests incl. the real-socket
  `e2e_create` login→create→enter-world flow and the 34.9k-NPC dist spawn
  smoke test) after every phase; stage-3 (`Schedule` + ECS resources)
  logged in CONCURRENCY_MODEL §2.8 as an open question, default **no**.

### G9.6 — Macros & panel shortcuts ✅
Plan: [PLAN_MACROS_SHORTCUTS.md](PLAN_MACROS_SHORTCUTS.md). The shortcut bar
and server-stored macros, persisted per character. Macro *execution* is
client-side in the Java reference too — the server only stores and echoes.

- **Model** (`model/shortcut.rs` + `Shortcuts`/`Macros` components):
  `Shortcut`/`Macro`/`MacroCmd` + the `ShortcutType`/`MacroType`/
  `MacroUpdateType` enums (wire value = Java ordinal); registry logic as
  component methods (slot key `slot + page*12`, macro ids allocated from
  1000 skipping taken ones, insertion-ordered entries like Java's
  `LinkedHashMap`); the `type,d1,d2[,cmd];` DB `commands` codec with Java's
  tokenizer semantics (4th comma-token only, 255-char truncation) kept for
  round-trip parity.
- **DB** (`db.rs`): `character_shortcuts`/`character_macroses` load with the
  per-character select (like items/skills; `class_index` always 0); new
  fire-and-forget `UpsertShortcut`/`DeleteShortcut`/`UpsertMacro`/
  `DeleteMacro`; creation inserts the initial panel + macro presets,
  resolving ITEM entries item id → created object id on the DB thread.
- **Packets**: `ShortCutInit` (0x45, real per-type layouts — replaces the
  empty G4 stub), `ShortCutRegister` (0x44), `SendMacroList` (0xE8, one
  packet per macro with total count on enter world; ADD=1/MODIFY=2/DELETE=0
  echoes) — hand-computed byte tests (no client capture yet).
- **Handlers**: `RequestShortCutReg` 0x3D (page 0-19 gate, ITEM verified
  against the inventory + template shared-reuse-group; the
  `ShortCutRegister` echo and `SkillList` re-send are unconditional, a Java
  quirk kept), `RequestShortCutDel` 0x3F (deletion re-sends the whole
  `ShortCutInit` — there's no per-slot delete packet), `RequestMakeMacro`
  0xCD (Java's validation order: >255 command chars → SM 810, >48 macros →
  SM 797, empty name → SM 838, >32-char descr → SM 837),
  `RequestDeleteMacro` 0xCE (panel-slot cascade + DELETE echo).
- **Deliberate deviation — no recurring macros:** `RequestMakeMacro`
  rejects any macro containing a `SHORTCUT`-type command (SM 810 "Invalid
  macro"). That command ("press panel slot X") is the only way a macro can
  invoke another macro — the classic looping AFK macro, which Java happily
  registers. Blocking the command type outright is the airtight rule: slot
  contents can be rebound after registration, so checking what the slot
  holds is bypassable.
- **Hooks**: enter world sends the macro LIST burst before `ItemList` and
  the real `ShortCutInit` after it (Java's order); relog restore prunes
  ITEM shortcuts whose object id left the inventory (component + DB row);
  skill learn and level-up auto-grants rewrite matching SKILL slots
  (`updateShortCuts`: level bump + `ShortCutRegister` + row upsert).
- **New characters** (`data/initial_shortcut.rs`): `initialShortcuts.xml`
  port — global + per-class pages + macro presets (`enabled="false"`
  presets skipped, and MACRO slots referencing them dropped, so the stock
  example macro never lands). Mystic-class quirk: the class page's Self
  Heal shares slot 10 with the global Sit/Stand and overwrites it (Java
  map-put order) — a fresh Human Mystic panel is 5 slots, asserted in
  `e2e_create`.
- **Deferred**: pet/summon panels (`character_type` 2 is stored, nothing
  consumes it), RECIPE/BOOKMARK behavior (packet arms exist, nothing
  produces them), auto-soulshot deactivation on shortcut delete, the
  item-removal prune hook (no drop/trade/destroy exists yet — the
  restore-time prune covers stale rows meanwhile).
- **Tests**: codec/registry units; `initialShortcuts.xml` loader vs the
  real dist; packet byte tests; synthetic-world tests (register/delete
  round trip incl. DB commands, ITEM-verify reject, every
  `RequestMakeMacro` rejection incl. the SHORTCUT-command rule, delete
  cascade, skill-upgrade slot rewrite, `from_char` restore + stale-ITEM
  prune, enter-world packet order); `char_persistence::
  shortcuts_and_macros_persist` (real DB thread: creation panel + ITEM
  resolution, upserts/deletes, commands round-trip); `e2e_create` asserts
  the macro LIST packet + the 5-slot Mystic panel in the burst.

### G10 — Social systems ✅ vertical slice (chat + party + friends)
Plan: [PLAN_G10_SOCIAL.md](PLAN_G10_SOCIAL.md). Scoped to what two live
clients can exercise: chat, party, friends. **Clans deferred** (creation
only exists through village-master bypass dialogs — the G11 gate), with
mail/community board/matching rooms/command channels.

- **Chat** (`game_loop/chat.rs`): `Say2` (0x49) → `CreatureSay` (0x4A) with
  the `ChatType` enum. GENERAL = 1250-unit radius (region prefilter),
  SHOUT/TRADE = same map-region tile bucket (`GlobalChat/TradeChat = ON`
  semantics), WHISPER by name with the relation-mask tail (friend bit 0x01
  live, other bits await clans), PARTY via the party broadcast, CLAN/
  ALLIANCE answer SM 4202/4203. Guards: 105-char cap (SM 1078); malformed
  type/empty text **log-and-drop instead of Java's force disconnect**
  (deliberate deviation). Chat bans/jail/olympiad/block-list/say-filter/
  voiced commands/item links skipped with their systems.
- **Party** (`model/party.rs` + `game_loop/party.rs`): `World.parties`
  id-keyed map + `PartyRef` component back-pointer; one `PendingRequest`
  component slot covers Java's request map + `_activeRequester` for party
  *and* friend invites (30 s / 15 s seq-guarded `RequestTimeout` tasks).
  Full invite flow (`RequestJoinParty` 0x42 with the embryo-party shape —
  the Party exists from first invite, the leader binds on accept —
  `AskJoinParty`/`JoinParty`, busy/full/leader/pending guards),
  `PartySmallWindowAll/Add/Delete/DeleteAll` (0x4E–0x51), leave/oust with
  Java's disband rules (2 members left; leader-quit honors
  `AltLeavePartyLeader = True` on this dist; disconnect always transfers
  lead — SM 1384 + full window rebuild), `RequestChangePartyLeader`
  (D0:0x0C) slot swap, loot-rule voting (D0:0x75/0x76 →
  `ExAskModifyPartyLooting`/`ExSetPartyLooting` FE:C0/C1, unanimous-yes,
  15 s timeout), 12 s `PartyMemberPosition` (0xBA) self-rescheduling task
  (dies with the party via a seq bump), and `PartySmallWindowUpdate` (0x52
  — plain-short mask, **not** the reversed `masks.rs` scheme) piggybacked
  on every member vitals `StatusUpdate` (regen/damage/heal/MP consume;
  level-ups send the all-flags variant). Java's needCp/Hp/MpUpdate
  hysteresis dropped.
- **Party rewards** (`death.rs::calculate_rewards` party branch +
  `party::distribute_xp_and_sp`/`distribute_item`): members pool damage
  shares (alive + `AltPartyRange` of the corpse), level-gap multiplier at
  the top rewarded level, Java's fraction-squared `partyMul` quirk kept,
  `BONUS_EXP_SP` ladder × `RatePartyXp/Sp` (**70** on this dist) for 2+,
  level²-weighted split, all four `PartyXpCutoffMethod`s ported (dist runs
  `highfive`: gaps 0–9 → 100 %, 10–14 → 30 %, 15+ → 0). Auto-loot routes
  through `Party.distributeItem`: adena splits evenly in range; items go
  FINDERS_KEEPERS/RANDOM/BY_TURN (spoil variants inert — no spoil), with
  SM 299/300 "C1 has obtained" to the rest.
- **Friends** (`game_loop/friends.rs`): `character_friends` loads with the
  character (joined name/level/class snapshot → `Friends` component; new
  `InsertFriendPair`/`DeleteFriendPair` both-direction DB commands).
  Invite/answer (`FriendAddRequest` 0x83 → `FriendAddRequestResult` 0x55 +
  both lists/rows), delete by name from the snapshot (no global name cache
  needed — you can only delete someone on your list), SM-based
  `RequestFriendList`, `RequestSendFriendMsg` → `L2FriendSay` (0x78,
  receiver must have the *sender* friended). Enter world sends the real
  `L2FriendList` (0x75, replacing the G4-era empty 0x58 stub) + SM 503 and
  `FriendStatus(ONLINE)` (0x59) to online friends; leave world pings
  `FriendStatus(OFFLINE)`.
- **Config**: `AltPartyMaxMembers`/`AltLeavePartyLeader`/`PartyXpCutoff*`
  (Character.ini), `RatePartyXp/Sp` (Rates.ini). `GlobalChat`/`TradeChat`
  read as always-ON (dist value; OFF/GM variants unported).
- **Deferred**: clans/alliances (all clan chat answers "not in a clan"),
  mail, community board, party matching rooms & waiting list, command
  channels, tactical signs, block list, friend memos, `RelationChanged`
  packets (UserInfo/CharInfo re-broadcast stands in), pets in party
  windows, hero/petition chats.
- **Tests**: `model/party` units (bonus ladder, highfive gaps, cutoff
  methods); synthetic-world tests for chat scoping (1250 range, region
  bucket, whisper echo + offline SM 145, party-only chat), the invite/
  accept/decline/guards/timeout flows (packet shapes both sides), disband
  rules + leadership transfer on disconnect + oust + leader change, loot
  votes (accept + timeout), the 12 s position task lifecycle, vitals
  piggyback, party kill XP split with exact Java values, adena split +
  BY_TURN rotation skipping out-of-range members, friend invite/accept/
  delete/message round trips + login/logout notifications;
  `char_persistence::friendships_persist` (real DB thread); `e2e_create`
  now asserts the real `L2FriendList` in the burst.

### G11 — Scripting engine + quests + clans via bypass ✅ vertical slice
Plan: [PLAN_G11_QUESTS_CLANS.md](PLAN_G11_QUESTS_CLANS.md). The engine
slice of the script-breadth gate: bypass routing, a native quest framework
(compiled-in trait-object scripts), two completable quests, and clan
creation through the ClanMaster dialog. Script breadth is G12.

- **Bypass** (`game_loop/bypass.rs`): `RequestBypassToServer` 0x23 —
  `npc_<oid>_<cmd>` (existence + `INTERACTION_DISTANCE` + `ActionFailed`
  terminator) routed by first token (`Quest`, `create_clan` on
  `VillageMaster*` templates; rest log-drop); bare `Quest …` resolves its
  NPC via the new `LastFolkNpc` component (set on every NPC click —
  `validateHtmlAction` is deliberately unported, distance re-checks stand
  in). Empty bypass logs instead of Java's disconnect.
- **Quest framework**: `model/quest.rs` (`QuestState`, the
  `__compltdStateFlags` skipped-step math as a pure function, legacy
  bit-31 `condBitSet` unpack) + `Quests`/`QuestTimerSeqs` components;
  `game_loop/quests.rs` — `QuestScript` trait + `QuestRegistry` (per-npc
  start/talk/kill indexes) behind `World.quests: Arc<…>` (the `geo`
  borrow pattern), `QuestCtx` porting the `QuestState`/`AbstractScript`
  primitives (start/cond/exit, give/reward/take items, `giveItemRandomly`
  with ×`RateQuestDrop`, rated adena/XP/SP), QuestLink's chooser/talk/
  event split, `showResult`'s `.htm`-quest-window vs `.html`-plain split
  (`ExNpcQuestHtmlMessage` FE:0x8E vs `NpcHtmlMessage`), `onKill` fired
  from `npc_do_die` after combat rewards (killer-only — party sharing
  deferred), `RequestQuestAbort` 0x63, and seq-guarded
  `ScheduledTask::QuestTimer`.
- **Persistence**: `character_quests` row-per-var, Java-schema-compatible
  (`<state>` as `Start/Started/Completed`); `load_quests` (orphan vars
  dropped) + fire-and-forget `UpsertQuestVar`/`DeleteQuestVar`/
  `DeleteQuest{keep_state}`.
- **Packets/items**: real `QuestList` (one-time mask incl. Java's
  id-range exclusions) and `ExQuestItemList` replace the G4 stubs;
  `ExShowQuestMark`, `PlaySound`; **first item-removal path** —
  `Inventory::remove_item` → `ItemChange`s → removed-type
  `InventoryUpdate` + `DbCommand::DeleteItem`; `Player.addItem`'s
  stack-or-create core extracted to `items::add_inventory_item` (shared
  with G9 loot). SM 52/53/54 "earned" trio for quest gives.
- **Scripts** (`src/scripts/`, `build_registry()` = the boot-time script
  pass): `Q00258_BringWolfPelts` (deterministic drop, reward table),
  `Q00320_BonesTellTheFuture` (0.18-chance drop ×`RateQuestDrop`, rated
  adena), `ClanMaster` (60 NPC ids, `LEADER_REQUIRED` → `-no.htm` remap;
  Clan Advent buff unported). Quest htmls read from the dist tree with
  the `quests/<Name>/` fallback and `noquest.htm` default.
- **Clans** (`model/clan.rs` + `game_loop/clans.rs`): `World.clans`
  loaded at boot (unprompted `DbEvent::ClansLoaded`, `IdBlock` pattern);
  `create_clan` with Java's guard order (SM 229/190/230/261/262/5), clan
  id from the shared `IdManager` pool, `InsertClan` + `UpdateCharClan`
  persistence, `PledgeShowInfoUpdate`/`PledgeShowMemberListAll`/
  `PledgeShowMemberListUpdate` + SM 189 + UserInfo/CharInfo re-broadcast.
  `Player` grew `clan_id`/`clan_privs`/`clan_leader` (fixed up at
  enter-world)/`clan_create_expiry_time`; clan id real in UserInfo CLAN
  block, CharInfo, CharSelectionInfo, CharSelected; clan chat now
  broadcasts to online members; enter/leave world send the roster window
  and online/offline pings.
- **Tests**: cond-flags/bit-unpack units; `char_persistence::
  quest_states_persist`; synthetic-world tests for bypass routing, the
  full Q00258 loop (accept → drops → cond mark → turn-in → repeatable
  re-offer, packet+DB assertions), Q00320's forced-roll chance path and
  rated adena, abort, a synthetic-script quest timer (fire/cancel), the
  clan guard matrix + creation packet trio + persistence, ClanMaster
  leader gating against the real dist htmls, and roster/chat scoping.

### G12 — Static world + script/content breadth ✅ vertical slice
Plan: [PLAN_G12_STATIC_WORLD_AND_CONTENT_BREADTH.md](PLAN_G12_STATIC_WORLD_AND_CONTENT_BREADTH.md).
Both plan areas landed as vertical slices; the long tail (33 more zone
types, multisell/sell/warehouse, ~188 more quests, ~81 `ai/` scripts, admin
commands) stays G13.

**Zones** (`data/zone_data.rs`, `game_loop/zones.rs`):
- `ZoneManager` port narrowed to the three files with live consumers —
  `peace.xml`/`water.xml`/`no_restart.xml` (590 zones), reusing the spawn
  territories' `ZoneForm` geometry, indexed into Java's `SHIFT_BY = 15`
  zone-grid cells (bounding-box overlap registration, point query walks
  the cell's zones).
- `ZoneFlags` component (mask + `_lastZoneValidateLocation` 100-unit filter
  + `_lastCompassZone`), revalidated from the movement tick, enter world,
  teleports (`Appearing`) and the `ValidatePosition` snap — Java's
  `revalidateZone` call graph. `ExSetCompassZoneCode` (FE:0x33) pushes the
  peace icon on change (deviation: the initial no-op GENERAL push is
  suppressed — a fresh client already displays general).
- **Peace gate** where Java actually has it (playable-vs-playable only):
  `resolve_cast_target`'s `Enemy`/`EnemyOnly` arm → SM 2167 after the LOS
  check, and `Self.java`'s bad-self-skill branch. Auto-attack needs no gate
  (player targets aren't attackable until PvP exists).
- **Water**: `Speeds.swimming` flips on enter/exit (`getMoveSpeed`'s swim
  branch) + `broadcastUserInfo`; breath/drowning deferred. NO_RESTART only
  tracks membership — nothing reads the flag in this Mobius version.

**Doors** (`data/door_data.rs`, `geo/doors.rs`, `model/door.rs`,
`game_loop/doors.rs`):
- All **1180** `DoorData.xml` doors parse (Java's flattened child-attribute
  StatSet) and spawn as ECS entities; `masterClose`/`isWall` and the unused
  group/child/emitter machinery are not carried.
- Collision is Java's real shape — **doors don't carve geodata**: a
  `DoorGrid` inside `GeoEngine` (registered before the `Arc` is shared, so
  the path worker sees it; open flags are atomics) runs the
  `checkIfDoorsBetween` segment-vs-polygon test at the head of
  `can_see_target` (double-face), `get_valid_location` and
  `can_move_to_target` — closed doors block LOS, movement and pathfinding.
- `StaticObjectInfo` (0x9F) + `DoorStatusUpdate` (0x4D) render doors on
  enter world/region cross; `open_door`/`close_door` broadcast state flips,
  with the auto-close task (seq-guarded) and the BY_TIME cycle
  (`startTimerOpen`/`TimerOpen` verbatim, 111 doors self-toggling). BY_CLICK
  is intentionally inert — `isOpenableByClick` has no consumer in this
  Mobius version either (clan-hall dialogs are its only route).
- **Static objects**: 86 of the 159 `StaticObjects.xml` entries (73 are
  commented out) spawn and render via `StaticObjectInfo`; click behavior
  (town map, thrones) is gated on community board/castles.

**Bypasses/shop** (`game_loop/bypass.rs`, `game_loop/shop.rs`,
`data/buy_list_data.rs`, `network/trade.rs`):
- `Link <file>`: `Link.java`'s whitelist (23 pages) served from
  `data/html/` as plain `NpcHtmlMessage`; `..`-escapes dropped.
- `Buy <listId>` on `Merchant`/`Fisherman` templates →
  `Merchant.showBuyWindow`: all **338** buylists load (file name = list id,
  `CorrectPrices = True` floors prices to sell value at load; limited stock
  treated as unlimited — 3 lists), `BuyList` + `ExBuySellList` (FE:0xB8 both)
  with the shared `AbstractItemPacket` item block, and `RequestBuyItem`
  (0x40) with Java's validation ladder (off-list/unstackable-quantity/
  MAX_ADENA/adena shortfall) → charge, deliver, `ExUserInfoInvenWeight` +
  sell-refresh + SM 4358. Weight/slot capacity gates wait for encumbrance;
  Sell/multisell deferred. `ItemTemplate` grew the reference `price`.

**Quest/script breadth** (`game_loop/quests.rs`, `src/scripts/`):
- `QuestScript` grew `on_attack`/`attack_npcs` (fired from
  `npc_receive_damage`, killing blow included) and `on_spawn`/`spawn_npcs`
  (fired from `spawn_one` — boot pass and respawns; no player in the ctx),
  plus `Npc.script_value` (Java's per-instance scratch, reset by respawn),
  `NpcSay` (0x30), and ctx primitives: category checks
  (`data/category_data.rs` — full `CategoryData.xml`), `set_class_id`
  (immediate `StorePlayer` + `broadcastUserInfo`), `teleport_to`,
  `already_completed_html`.
- **+10 quests** picked for shape variety: Q00303/Q00313 (single-kill
  collect), Q00260/Q00263/Q00265/Q00273 (multi-kill-target with per-monster
  drop tables), Q00317 (uncapped drops, pay-out-and-continue turn-in),
  Q00324 (10th-item cond bump), **Q00316** (the `on_attack` consumer —
  Varool Foulclaw's one-shot NpcSay via script value + his one-only fang),
  **Q00109** (multi-step cond 1→2→3 across three NPCs, **one-time** —
  first COMPLETED-state quest, already-completed page included).
- **OrcChange1** (village master #2): the full first-transfer matrix
  (category gates, proof marks, level 20, 15 shadow coupons, class change
  persisted immediately) through the dist htmls' `Quest OrcChange1 <event>`
  bypasses.
- **TeleportWithCharm** (first `ai/others` script): token-consuming
  teleport, registered through the same `QuestRegistry` — resolved plan
  question #1: utility scripts fit the existing registry; a new opt-in
  `bare_talk()` routes their `on_talk` from the bare `Quest` bypass
  (deviation: this Mobius build's chooser short-circuit leaves such
  scripts unreachable even though the dist htmls point at that button).
- Resolved plan question #4: ClanMaster keeps its ad hoc page loading —
  retrofitting onto `Link` risked the working G11 gate for no visible gain.
- **Tests**: zone loader/grid units + peace/water/filter world tests; door
  grid + engine-level geo units, enter-world door burst, LOS-until-opened,
  auto-close staleness, BY_TIME cycling; static-object loader/burst; Link
  whitelist round trip; buylist loader vs dist (CorrectPrices floor
  verified globally), Buy window + purchase/guards; per-shape quest loops
  (Q00303, Q00316 incl. the shout + fang cap, Q00109 incl. the completed
  mask), OrcChange1 transfer + category refusal, TeleportWithCharm, and a
  synthetic `on_spawn` script. `e2e_create` runs against the full boot
  (zones + doors + statics + 15 scripts); its skip-unsolicited helper now
  also skips the compass code (the mage-start spawn lies in a peace zone).

**Post-G12 fixes:**
- **`AutoLearnSkills` config now honored** (`config/character.rs`,
  `data/skill_tree.rs`, `game_loop/death.rs`, `game_loop/lobby.rs`): the port
  ignored `Character.ini`'s `AutoLearnSkills = True`, so players only ever got
  autoGet skills. `Player.rewardSkills` now branches on the flag — with it on,
  `SkillTreeData.all_available_skills` (a base-tree narrowing of Java's
  `getAllAvailableSkills`: highest reachable level per class skill, no
  FS/removeSkills/required-items in `StartingClass/*.xml`) grants every
  reachable class skill on both enter-world and level-up, with the
  `ShortCutInit` + "learned N skills" (`SystemMessageId.S1_2`) notice. FS /
  divine-inspiration / removeSkills paths stay out of scope (absent from base
  trees). Unit + level-up/enter-world grant tests.

---

## Deferred TODOs (by system)

Empty/placeholder now, to be filled in the owning milestone:

- **Inventory/items (post-G5):** warehouse/clan warehouse/freight/mail,
  trade, pickup/drop, item actions (`RequestActionUse` beyond equip),
  crystallization, enchanting, augmentation, elemental attributes,
  `ExQuestItemList` (no quest items exist yet), real `maxLoad` calc +
  encumbrance enforcement, `ItemList`/`ExUserInfoEquipSlot` visual-id block.
  Also blocks full P.Def/P.Atk/M.Def/M.Atk accuracy (see G6: naked-value only
  until item `<stats>` are parsed). `UseItem`'s `EtcItem` branch dispatches
  through a typed `ItemHandler` (`data/item_data.rs`); `ExtractableItems`
  (pack/box unpacking, e.g. "Mage Class Equipment Set") and `ItemSkills`/
  `ItemSkillsTemplate` (potions/buff scrolls — casts the item's `<skills>`
  list immediately via the existing skill-effect pipeline, `Heal`/
  `MagicalAttack`/`StatModifier` only since that's all `EFFECT_REGISTRY`
  covers so far; reuse shared with `game_loop::skills::cast::{check,set}
  _skill_reuse`, also extracted for `use_magic_on`) are ported — soulshots/
  dyes/enchant scrolls and the rest of Java's `handlers/itemhandlers/*` are
  still no-ops (`game_loop/items.rs::use_etc_item`'s `ItemHandler::None`
  arm), as is `<cond>`-gating and the `itemConsumeId`/
  `SKILL_REDUCE_ON_SKILL_SUCCESS` non-consume case (every `ItemSkills` use is
  treated as consume-on-success).
- **Skills/combat (post-G9):** `PhysicalAttack`-type *skills* (auto-attack
  damage is done; skill-based physical hits reuse `apply_physical_damage`);
  bows/crossbows (reuse gauge, arrows), dual-weapon split hits, polearm
  sweeps, soulshots (`SHOTS_BONUS`), shield defence (`calcShldUse` — needs
  item `<stats>` parsing), PvP auto-attack (needs PvP flags/karma); AoE
  affect scopes (only `SINGLE` resolves); `ALT_GAME_MAGICFAILURES`
  magic-resist rolls (`calcMagicSuccess`); ~~queued skills +
  walk-into-cast-range AI~~ (both done: `QueuedAction` slot + `PlayerIntent::Cast`
  chase — an out-of-range cast walks into cast range then casts at the
  snapshotted target, shift-click = `dontMove` → SM 748; ground-target
  `maybeMoveToPosition` still waits on GROUND targeting);
  the other 8 `AcquireSkillType`s (PLEDGE,
  TRANSFORM, TRANSFER, SUBCLASS, …); toggle-type skills; skill mastery +
  `MAGIC_REUSE_RATE`; skill reuse-delay persistence across relog;
  `ExAbnormalStatusUpdateFromTarget` (broadcast to other players); most of
  the 230-entry `Stat` enum and 369 effect classes (grow `EFFECT_REGISTRY`/
  `SkillEffect` as needed); overhit XP bonus; buffs/effects on NPC targets
  (no NPC effect list).
- **Movement/targeting (post-G7.8):** NPC pathfinding (player moves path
  via the G7.85 worker; NPC chase/return-home moves are still straight-line,
  and the Attackable closest-reachable-point grid scan is unported);
  ~~zones~~/~~door LOS+movement checks~~ (✅ G12 — peace/water/no-restart
  zones and all 1180 doors; the other 33 zone types, fence checks, and
  `ValidatePosition`'s door-exploit tail remain); the rest of
  `isMovementDisabled()`
  (rooted/overloaded/immobilized/dead/teleporting); cursor-key movement
  (`_cursorKeyMovement` path incl. `canMoveToTarget` front-cell check and
  `getLastServerPosition` stop); falling damage/state (`isFalling`).
- **NPCs/world content (post-G9):** random walk / `randomAnimation`; guard
  aggro (needs karma), clan/faction help calls (`<clanList>` unparsed),
  minions, raid/grand-boss behaviours (chaos target swaps, raid curse,
  raid points); NPC skill casting (`AISkillScope` lists unparsed) + NPC
  buffs/effect list; NPC regen; ground drops + pickup (`AutoLoot = False`
  path — needs item world objects; herbs likewise), spoil/sweep; party XP
  split + overhit; Java's teleport-home on attack timeout (we walk);
  elemental attributes (template parse skips them); `dbSave` raid
  persistence (`DBSpawnManager` — spawned statically at full HP);
  `HtmCache` (dialog `.htm`s are read per interaction);
  ~~zones/doors/`StaticObjectData`~~ (✅ G12 vertical slice);
  `NpcNameLocalisationData`/multilang; the death
  dialog's non-village restart points (clan hall/castle/fixed-feather).
- **Quests/scripts (post-G11/G12):** party quest sharing
  (`getRandomPartyMemberState` — kill credit is killer-only); daily quests
  (`restartTime`/reset hour); `onFirstTalk` hook (~~onAttack/onSpawn~~ ✅
  G12); tutorial (Q00255);
  `ExQuestNpcLogList`; the quest-window weight/inventory-90%/40-quest
  guards; the chooser's simulated-`onTalk` pre-filter; `validateHtmlAction`
  (bare bypasses resolve via `LastFolkNpc` + distance); the remaining ~188
  quests, ~14 village-master scripts and ~81 `ai/` scripts; other bypass
  families (~~`Link`~~/~~`Buy`~~ ✅ G12; `multisell`, sell,
  `learn_clan_skills`, `item_`, `admin_`, `_bbs`, menu/manor selects).
- **Social (post-G10/G11):** clans past creation (invite/leave/dissolve/
  level-up/wars/ally/academy/sub-pledges, clan skills +
  `PledgeSkillList`, crests, notices, warehouse, `PledgeInfo`/
  `PledgeStatusChanged` beyond the creation trio, the Clan Advent buff,
  RELATION bits / `RelationChanged` — the full UserInfo/CharInfo re-send
  stands in); ally chat; mail; community board; party matching rooms;
  command channels (MPCC); tactical signs; block list (`BlockList` checks
  skipped everywhere); friend memos + `RequestExFriendListExtended`;
  pet/servitor party-window packets; chat bans/say filter/voiced
  commands/item links in chat; `GlobalChat`/`TradeChat` OFF/GM modes;
  skill/reuse persistence for party-relevant buffs unchanged (see skills
  section).
- **Misc:** ~~macros~~ (✅ G9.6), `HennaInfo` empty, `ExUserBanInfo`, `ExVitalityEffectInfo`
  bonuses, real castle list for manor, game-time clock (CharSelected/UserInfo
  use 0), periodic auto-save while in game (`AutoSaveManager`; persistence on
  restart/logout/disconnect is done).

---

## Tests / verification

- **Crypto:** golden vectors (`commons/tests`, `gameserver` cipher).
- **Protocol parity:** GS↔LS packet cross-checks (loginserver as gameserver
  dev-dep), `AuthRequest`/`BlowFishKey`/`PlayerAuthRequest` layouts.
- **DB:** `char_persistence.rs` — create/load/delete/restore against the stock
  schema.
- **Full E2E:** `e2e_create.rs` — real two-server login→create→enter-world with a
  scripted client; drains the enter-world burst; checks computed HP/MP and
  (G5) that the Human Mystic's starting wand shows up equipped in `ItemList`/
  `ExUserInfoEquipSlot`.
- **UserInfo bytes:** unit test against a real client capture.
- **Inventory:** `model::inventory::tests` — item/equipment loaders load real
  `dist/game` data; `equip_item` slot-conflict cases (full armor vs
  chest+legs, two-handed vs dual single-hand, ear/finger fill order).
- **NPCs (G8):** loader counts + hand-checked templates against the real
  dist; `spawn_all` placement/coordinate/region-index smoke test; `NpcInfo`
  hand-computed byte test; synthetic-world visibility & two-click
  interaction tests.
- **Social (G10):** chat/party/friend synthetic-world tests (see the G10
  section), party-math units with exact Java values, friendship DB
  round-trip.
- **Quests/clans (G11):** cond-flags math units vs hand-traced Java
  values; `character_quests` DB round-trip; synthetic-world tests for the
  full quest loops (Q00258/Q00320 with forced rolls), bypass routing,
  abort, quest timers, the clan guard matrix/creation flow, ClanMaster
  dialog gating vs the real dist htmls, and clan roster/chat scoping.
- **Combat (G9):** physical-formula units with exact Java values; drop/
  corpse/aggro template assertions against the real dist; synthetic-world
  integration tests over the real tick systems — melee kill (rewards,
  level-up, auto-loot, decay), chase + retaliation, unprovoked aggro,
  kill-by-nuke, player death → to-village revive, decay → respawn.

Run: `cargo test` (all green). Boot a pair on alt ports:
`cargo run -p loginserver` + `CONFIG_SERVER_GAMESERVERPORT=… cargo run -p gameserver`.

---

## Cross-cutting notes

- Game server runs from `dist/game`; all ini/data paths resolve unedited.
  `GameData::load_from(path)` lets tests point at the datapack from any cwd.
- Session lifecycle is a **type-state** machine (plan §3.1):
  `Connecting → Authenticated → InLobby → Entering → InGame`; the `Player` lives
  in `World.players` keyed by object id, `InGame` links by id.
- The object registry (`World.objects`) is **one `bevy_ecs` world** holding
  players and NPCs as entities decomposed into per-concern components
  (CONCURRENCY_MODEL §2.8; G9.5 / [PLAN_ECS_STAGE2.md](PLAN_ECS_STAGE2.md)).
  The game thread remains the sole owner; no parallel scheduling; object
  ids are the only foreign key (`Entity` never leaves `store.rs`).
- Masked packets use the reversed `DEFAULT_FLAG_ARRAY` bit order — get this right
  or the client desyncs (root cause of the earlier UserInfo mask fix).
