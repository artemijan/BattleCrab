# PLAN — G22 `ai/others` NPC scripts (remaining-ports audit row 5)

The last big breadth chunk of the Java datapack: `dist/game/data/scripts/ai/others`
— 39 Java scripts of town-service NPCs, small mob behaviours and global spawn
managers. Row 4 (`ai/areas`) closed 2026-07-30; this is its sibling.
**Gate:** every `ai/others` script is either ported or recorded here as a
verified dead-content / out-of-scope skip, with the reason.

Delivered slice-by-slice, highest leverage first — same shape as the areas
sweep (see [PLAN_G22_ELVEN_PATH_QUESTS.md] siblings and
`game_loop/area_npcs.rs` for the global-beat pattern).

## Coverage audit (2026-07-30)

Verified three ways, not from the docs: NPC ids grepped against
`crates/gameserver/src`, against `dist/game/data/spawns/**` (does the NPC
exist on this dist at all?), and against `dist/game/data/stats/npcs`.

### Already covered (no work)

| Java script | Covered by |
|---|---|
| `CastleChamberlain` | `scripts/castle_chamberlain.rs` |
| `ClanHallManager` / `ClanHallDoorManager` / `ClanHallAuctioneer` | `scripts/clan_hall_*.rs` (G24 clan halls) |
| `MonumentOfHeroes` | `scripts/monument_of_heroes.rs` (G17 nobless) |
| `NewbieGuide` | `scripts/newbie_guide.rs` |
| `OlyManager` | `scripts/oly_manager.rs` (G25) |
| `WyvernManager` | wyvern flight (`game_loop/servitor.rs` + config) |
| `TeleportToRaceTrack`, `TeleportWithCharm`, `ValakasTeleporters` | ported with their owning features |
| `SiegeGuards` | `game_loop/siege.rs` — guards are spawned by the siege engine, not per-id script |
| `SeeThroughSilentMove` | the stealth aggro gate (`data/skill_data.rs` + stealth tests) |

### Live content, unported (the work)

| Java script | NPCs / ids | Note |
|---|---|---|
| `Mammons/{Merchant,Blacksmith,Priest}OfMammon` | 31113, 31126, 33511 | script-owned wandering spawn (30 min) + first-talk html + spawn announce; htmls are pure `multisell` / `exc_multisell` buttons |
| `CastleBlacksmith` | 35098 … 35553 (9) | rights gate (clan leader / `CS_MANOR_ADMIN` / cond-override) → html |
| `CastleWarehouse` | 35099 … 35554 (9) | first-talk html; the Blood Alliance branch is off-chronicle (items 9910/9911) |
| `CastleMercenaryManager` | 35102 … 35557 (9) | `CS_MERCENARIES` gate, siege-state html, `buy <n>` → castle buylist |
| `CastleDoorManager` | 35096/35097 … (28) | owner gate + `manageDoors 0/1` over the `DoorId1`/`DoorId2` spawn parameters |
| `CastleSiegeManager` | 35104 … 35559 (9) | first-talk → owner html / siege html / `listRegisterClan` |
| `CastleSideEffect` | zones 11020 … 11035 | on town-zone enter, push `ExCastleState` for every castle |
| `CastleTeleporter` | 35092 … 35546 (26) | **audit row 5 wrongly listed this as covered** — no Rust hit for any id; mass-teleport timer + `teleportMe <n>` over spawn parameters |
| `SymbolMaker` | 31046 … 31953 (11) | **partially covered**: the `Draw`/`Remove` bypass verbs exist (`game_loop/henna.rs`), but the script's `symbol_maker.htm` first-talk that renders the buttons is unported, so the dye NPCs are mute |
| `PolymorphingOnAttack` | 21258/21261-4/21265-7/21271-3/2152x-2154x (15 chains) | HP-threshold + chance morph into the next stage, with the taunt bark |
| `PolymorphingAngel` | 20830/20831/21062/21067/21070 | kill → spawn the paired angel |
| `TimakOrcTroopLeader` | 20767 | on-attack minion call (`SummonPrivateRate`, cap 3) — rides the named-minion work from the areas sweep |
| `FleeMonsters` | 20432 Elpy (20002 Rabbit has no spawns) | flee 500 units away from the attacker on hit |
| `FairyTrees` | 27185-27188 | immobile; on kill within 1500 spawn 20 Soul Guardians, half of them casting Venomous Poison |
| `NonLethalableNpcs` | 35062 (siege HQ) | `setLethalable(false)` — HQ is a runtime spawn, so absent from `spawns/` but live |
| `ArenaManager` | 31225/31226 | adena-priced CP/HP recovery (2 s delayed cast, PVP-zone gate) + 6-buff package |
| `ToIVortex` | 30949-30954 | Tower of Insolence floor teleports for dimension stones + 100k-adena stone trade |
| `RandomWalkingGuards` | 31032-31036 | 15-45 s random drift around the spawn point when out of combat |
| `Spawns/DayNightSpawns` | 50 spawn templates carry `ai="DayNightSpawns"` | day/night spawn groups (`dayTime`/`nightTime`) — **unblocked by the G33 game clock** |
| `Spawns/NoRandomActivity` | 1 template | `disableRandomWalk` / `disableRandomAnimation` spawn parameters |
| `Servitors/SinEater` | 12564 | summon chatter on talk/attack/death (cosmetic) |

### Verified skips (dead content / out of scope)

| Java script | Why |
|---|---|
| `Proclaimer` (36609-36617) | 0 spawns on this dist, and the gate is `isOnDarkSide()` — Seven Signs, removed from the port (see G26 notes). Buff skill 19036 is later-chronicle. |
| `OlyBuffer` (36402) | 0 spawns and nothing in the Java tree spawns it either (only `SkillCaster` references the id); the Interlude olympiad has no buffer NPC. |
| `Scarecrow` (19546, 27457) | templates exist, 0 spawns, no spawner — Classic event content. |
| `DivineBeast` (14870) | Gracia transformation-summon (transform 258) — no spawn, no summon path on this dist. |
| `Incarnation` (13302-13579) | later-chronicle summons. |
| `Servitors/TreeOfLife` (14933-15154) | later-chronicle servitors; skill holder `s_tree_heal` comes from `<parameters>` the port does not carry. |
| `ClassMaster` | custom (config-driven) class changer, explicitly out of scope — the port does class transfers through the village masters. |

## Slice breakdown

### Slice 1 — the `multisell` NPC bypass + the three Mammons  ✅ LANDED 2026-07-30

Two things because the first blocks the second.

1. **`multisell` / `exc_multisell` NPC bypass verbs.** The multisell *engine*
   exists (`game_loop/multisell.rs`, built for the community board), but
   `game_loop/bypass.rs` has no `multisell` verb — only the `_bbsmultisell`
   BBS entry points. **97 dist htmls** emit `npc_%objectId%_multisell <id>` /
   `_exc_multisell <id>` (44 under `html/merchant`, 12 `html/petmanager`,
   23 in `scripts/ai`, …), so every exchange shop button in the game is
   currently dead. Java: `Merchant`/`AbstractNpcAI` bypass → `MultisellData.
   separateAndSend(listId, player, npc, inventoryOnly)`.
2. **Mammons** (`ai/others/Mammons/*`): Merchant 31113 (8 haunts), Blacksmith
   31126 (6), Priest 33511 (3). Each: a script-owned spawn placed at boot and
   relocated every 30 min (Java `RESPAWN_*` repeating quest timer, `deleteMe`
   the old one), a first-talk html, and — behind `AnnounceMammonSpawn` (`True`
   in `dist/game/config/NPC.ini`) — a server-wide announce naming the nearest
   castle. The lifecycle is the Toma pattern: it lives in
   `game_loop/area_npcs.rs`, not in the quest-timer machinery (no player to
   anchor to); the chat window is a `QuestScript` like `scripts/toma.rs`.

**Gate for the slice:** talk to a spawned Merchant of Mammon and the exchange
multisell window opens with the real `31113001` list; the 30-minute beat moves
him to a different haunt and announces it.

### Slice 2 — castle service NPCs  ✅ LANDED 2026-07-30
`CastleBlacksmith`, `CastleWarehouse`, `CastleMercenaryManager`,
`CastleDoorManager`, `CastleSiegeManager` and `CastleTeleporter` in one
`scripts/castle_services.rs`, over a shared rights layer (`isMyLord` /
owning clan / `ClanPrivilege` / the GM cond-override) that resolves the castle
the way Java's `npc.getCastle()` does — `nearest_castle_at`, no id table.

What the slice needed underneath:
- The door ids and teleport posts live on the **NPC template**
  `<parameters>` (`DoorId1`, `pos_x01`…), not the spawn entries; the template
  parser already keeps them (`ai_param_i32`).
- `ClanPrivilege.CS_OPEN_DOOR` (16) / `CS_MERCENARIES` (22) were unnamed. Naming
  them exposed a bug in `RANK9_PRIVS_MASK`, which used bit 15
  (`CH_SET_FUNCTIONS`) for `CS_OPEN_DOOR` — academy members kept hall-function
  rights and lost the castle-door right the mask exists to grant. Fixed here.
- `ResidenceTeleportZone` (`castle_teleport.xml`, 9 zones) now loads, giving
  `Castle.oustAllPlayers` (`siege::oust_all_players`) its territory and oust
  points, which the mass gatekeeper's `MASS_TELEPORT` needs.
- `Siege.listRegisterClan` is exposed for the Siege Manager's non-owner branch —
  it re-uses the existing `SiegeInfo` window, so **audit row 11's "siege info
  window" is reachable from an NPC now**, not just from `RequestJoinSiege`.

**`CastleSideEffect` is deliberately skipped**: it pushes `ExCastleState` (the
Grand Crusade castle-side banner) on town-zone entry — no Interlude opcode.

**Deviation:** the `MASS_TELEPORT` shout goes to the gatekeeper's broadcast
region rather than Java's `MapRegionManager` region (no map-region table in
this port) — same audience in practice, marked `TODO(G22)` at the site.

### Slice 3 — combat mob behaviours
`PolymorphingOnAttack`, `PolymorphingAngel`, `TimakOrcTroopLeader`,
`FleeMonsters`, `FairyTrees`, `NonLethalableNpcs`. All small, all on hooks the
areas sweep already landed (`on_attack`, `on_kill`, `on_spawn`,
`minions::spawn_minion_group`).

### Slice 4 — day/night spawn groups
`Spawns/DayNightSpawns` + `Spawns/NoRandomActivity`: teach the spawn loader the
`ai=` attribute and the `dayTime`/`nightTime` group names, and drive
spawn/despawn off the G33 clock (`game_time::is_night_at`, already polled every
real minute by `ScheduledTask::DayNightCheck`). 50 templates on this dist —
this is the one item in row 5 with real world-population impact.

### Slice 5 — the talk/utility tail
`ArenaManager`, `ToIVortex`, `RandomWalkingGuards`, `SymbolMaker`'s missing
first-talk html, `Servitors/SinEater` chatter. Then update
`docs/PROGRESS.md` row 5 + `PARITY_CHECKLIST_G33.md`, and close the audit row.

## Watch-list

- **Nearest castle**: Java's announce uses `npc.getCastle().getName()`
  (`CastleManager.getCastle(obj)` = residence zone, else nearest by distance).
  The port has castle *names* (`model/castle.rs`) and castle-hall restart
  points (`data/castle_zone_data.rs`) but no position → decide between porting
  a nearest-castle helper and keying the fixed Mammon haunts to castle ids;
  prefer the helper, since slice 2's castle NPCs want `getCastle()` too.
- `PriestOfMammon`'s Java `onEvent` switches on `31113*.html` names (copy-paste
  from the Merchant) while its only html is `33511.html` — port the behaviour,
  not the intent: the priest's html buttons are `multisell` bypasses, so the
  dead branch never fires. See the `l2r-port-behaviour-not-intent` lesson.
- The Priest (33511) is **also statically spawned** in seven `spawns/Others/*.xml`
  tiles; Java runs both, so the script's roaming copy is an extra NPC, not a
  replacement. Don't "fix" the data.
- `Mammons` htmls use `exc_multisell` (inventory-only exchange). `multisell.rs`
  carries a `TODO(G30)` that `inventoryOnly` is not implemented — slice 1 has to
  land it for the Blacksmith's SA/crystal exchanges to behave.
- Row 5 in the audit lists `CastleTeleporter`/`SymbolMaker` as already covered.
  Both are wrong (see the table above); fix the audit text when the slices land.
- `CastleWarehouse`'s Receive/Exchange branch is Blood Alliance / Blood Oath
  (9910/9911) — later-chronicle siege reward currency, and `Clan` has no
  `bloodAllianceCount` here. Port the html shell, leave a `TODO(G22)` on the
  branch rather than inventing a counter.
