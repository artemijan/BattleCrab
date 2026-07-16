# G16 (partial) — Main-menu admin commands: points, premium & spawn lists

Status: **in progress**. Scope carved from the `//admin` main menu
(`data/html/admin/main_menu.htm`) — the 14 buttons on the top panel. Ten were
already implemented in G13.B (`Item`/`create_item`, `Teleport`/`move_to`,
`Spawn`/`spawn_monster`, `Open`, `Close`, `Heal`, and the text form of
`ListPos`/`ListSpwn`). This plan finishes the remaining behaviour and pulls the
**account-scoped storage** foundation of G16 forward, because three of the
buttons (NCoins, Premium) require it.

This doc is the plan of record; the shipped result is recorded in the G16 row of
[PROGRESS.md](PROGRESS.md).

## The 14 buttons → Java handlers

| Button      | Bypass                       | Java handler            | Pre-plan state |
|-------------|------------------------------|-------------------------|----------------|
| Item        | `admin_create_item`          | `AdminCreateItem`       | done (G13.B) |
| Teleport    | `admin_move_to`              | `AdminSpawn`/teleport   | done |
| Spawn       | `admin_spawn_monster`        | `AdminSpawn`            | done |
| ListPos     | `admin_list_positions`       | `AdminSpawn.findNpcs`   | **partial** — text only |
| ListSpwn    | `admin_list_spawns`          | `AdminSpawn.findNpcs`   | **partial** |
| goPosition  | `admin_list_positions … 1`   | `AdminSpawn.findNpcs`   | **broken** — no `tele_index` |
| goSpawn     | `admin_list_spawns … 1`      | `AdminSpawn.findNpcs`   | **broken** |
| PC Points   | `admin_pccafepoints`         | `AdminPcCafePoints`     | **missing** |
| NCoins      | `admin_primepoints`          | `AdminPrimePoints`      | **missing** |
| Premium     | `admin_premium_menu`         | `AdminPremium`          | **missing** |
| Open        | `admin_open`                 | `AdminDoorControl`      | done |
| Close       | `admin_close`                | `AdminDoorControl`      | done |
| Heal        | `admin_heal`                 | `AdminHeal`             | done |
| Full Food   | `admin_fullfood`             | `AdminEditChar`         | **blocked** (pets/G29) |

## Stage 1 — `list_positions` / `list_spawns` + `tele_index`

**Java:** `AdminSpawn.findNpcs` (iterates `SpawnTable.getSpawns(npcId)`,
1-indexed). **Rust:** `game_loop/admin/spawn.rs::admin_list_spawns` (currently
walks live `npc_regions` with no stable index, ignores the `tele_index` arg — so
goPosition/goSpawn do nothing).

- Parse `<npcId|name> [tele_index]`.
- Iterate the loaded spawn definitions for `npc_id` with a stable 1-based index.
- `list_positions` reports the last *live* spawn's location; `list_spawns`
  reports the spawn *definition* location.
- When `tele_index == index`, teleport the GM there — via the **validated
  teleport helper** used by `admin_move_to`, never a raw `Position` write
  (see memory `l2r-teleport-validate-position`).

## Stage 2 — PC Points (`admin_pccafepoints`) — character-scoped

**Java:** `AdminPcCafePoints`, `Player.setPcCafePoints` (cap `PC_CAFE_MAX_POINTS`),
column `characters.pccafe_points`, packet `ExPCCafePointInfo`.

1. Persistence: add a `pccafe_points` field to the `Player` component + DB
   struct, mirroring the vitality-points field (DDL column, UPDATE bind, load).
2. Config: `PC_CAFE_MAX_POINTS`, `PC_CAFE_RETAIL_LIKE`.
3. Packet `ExPCCafePointInfo`: `int points, int addPoint, byte periodType=1,
   int remainTime=0, byte pointType(add<0?2:1), int time*3`. Verify the opcode
   against the real Ex-outgoing table — do not guess.
4. Handler: `set`/`increase`/`decrease`/`rewardOnline [range]`; target =
   selected player else self; `pccafe.htm` menu with `%points%`/`%targetName%`.
5. Register in `admin_data.rs` gated list + `dispatch()`.

## Stage 3 — Account-variables store + NCoins (`admin_primepoints`)

**Java:** `AdminPrimePoints`, `Player.setPrimePoints` →
`AccountVariables.set("PRIME_POINTS", …)` + immediate `storeMe()`.

1. New account-scoped key/value store (table keyed by account name) with
   write-through — the minimal G16 foundation. Load a player's vars on
   enter-world. Must persist immediately, not via the memory-first character
   autosave (memory `l2r-player-persistence`).
2. Handler mirrors PC Points minus the packet; caps at `i32::MAX`/0;
   `primepoints.htm` menu.
3. Register in `admin_data.rs` + `dispatch()`.

## Stage 4 — Premium (`admin_premium_menu` + subcommands)

**Java:** `AdminPremium`, `PremiumManager`, table `account_premium(account_name,
enddate)`.

1. Minimal `PremiumManager` + `account_premium` table:
   `add_premium_time`, `get_premium_expiration`, `remove_premium_status`
   (account-scoped; lives beside the Stage 3 store).
2. Handler: `admin_premium_menu` (shows `premium_menu.htm`);
   `admin_premium_add1/2/3 <account>` add 1/2/3×30 days; `admin_premium_info`;
   `admin_premium_remove`. Gate on new `PREMIUM_SYSTEM_ENABLED` config.
   Skip the `PcCafePointsManager.run` retail-like hook →
   `TODO(G16): PC_CAFE_RETAIL_LIKE premium hook`.
3. Register the 6 commands.

## Stage 5 — Full Food (`admin_fullfood`) — pet-blocked stub

**Java:** `AdminEditChar` — sets the targeted **pet's** `CurrentFed` to
`MaxFed`, broadcasts status. Pets are G29. Register the command, resolve the
target, and (no pet target can exist yet) send `INVALID_TARGET` with a
`TODO(G29): set pet CurrentFed=MaxFed + broadcastStatusUpdate` at the site.

## Cross-cutting notes

- Menus construct `NpcHtmlMessage(0, 1)`; use the `admin/menu.rs` helper that
  sends `itemId=1` so the window stays open on click (memory
  `l2r-admin-html-itemId`).
- Factor one shared `get_target` helper ("selected player's acting-player else
  self") for PC Points / NCoins.
- Verification: `cargo build` per stage; targeted tests only (memory
  `l2r-cargo-test-hangs`); a `pccafe_points` round-trip test mirroring
  `char_persistence.rs`; in-client button click-through for the menus.
