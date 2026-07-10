# l2r_interlude — Milestone Progress & State

Living status tracker for the Java→Rust rewrite. Plans:
[PLAN_LOGIN_SERVER.md](PLAN_LOGIN_SERVER.md) (login, M0–M5) and
[PLAN_GAME_SERVER.md](PLAN_GAME_SERVER.md) (game, G0–G12). Architecture:
[CONCURRENCY_MODEL.md](CONCURRENCY_MODEL.md),
[JAVA_TO_RUST_CHALLENGES.md](JAVA_TO_RUST_CHALLENGES.md).

**Legend:** ✅ done · 🚧 in progress · ⏳ not started.

---

## Snapshot

| Phase | Milestone | Status |
|---|---|---|
| Login | M0–M5 | ✅ feature-complete, interop-verified with Java GS |
| Game | G0 Scaffold & boot | ✅ |
| Game | G1 Client link & cipher parity | ✅ |
| Game | G2 Login-link + auth | ✅ |
| Game | G3 Character selection & persistence | ✅ |
| Game | G4 Enter world (Player, HP/MP, UserInfo, enter-world burst) | ✅ core; 🚧 paperdoll/inventory |
| Game | G5 Static world content (NPCs/spawns/zones/geodata) | ⏳ |
| Game | G6 Items & inventory | ⏳ (started early: paperdoll bitmasks) |
| Game | G7 Stats, skills & effects | ⏳ |
| Game | G8 Combat & AI | ⏳ |
| Game | G9 Social systems | ⏳ |
| Game | G10 Scripting engine + quests | ⏳ |
| Game | G11 Script/content breadth | ⏳ |
| Game | G12 Long tail & parity sweep | ⏳ |

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

### 🚧 In progress — paperdoll & inventory bitmasks (part of G4/G6)
Goal: replace hardcoded paperdoll/mask values with Java-faithful enums/bitmasks.
- Java: 32 `Inventory.PAPERDOLL_*` slots; `InventorySlot` enum (`getMask()` =
  ordinal, 33 entries incl. `LRHAND`); masks use `AbstractMaskPacket` with the
  **reversed** `DEFAULT_FLAG_ARRAY = {0x80,0x40,…,0x01}` (mask 0 → 0x80).
- Known bug to fix in this pass: `enter_world::ex_user_info_equip_slot` mask is
  hardcoded `[0xFF,0xFF,0xFF,0xFF,0x01]` — should be `[…,0x80]` (slot 32 → 0x80).
- Plan: `model/inventory.rs` (`PaperdollSlot` enum + `Inventory` struct, empty
  for now), a mask helper (`DEFAULT_FLAG_ARRAY` + `build_mask`), an
  `InventorySlot` enum, and drive `ExUserInfoEquipSlot` +
  `CharSelectionInfo` paperdoll + `UserInfo` slots through them.

### G5–G12 — ⏳ not started
See [PLAN_GAME_SERVER.md §6](PLAN_GAME_SERVER.md). Next natural gate: finish the
G4 vertical-slice gate proper — **movement + visibility/known-list + `Say2`
chat** (two clients see each other, walk, chat) — then G5 static content.

---

## Deferred TODOs (by system)

Empty/placeholder now, to be filled in the owning milestone:

- **Inventory/items (G6):** `ItemList`, `ExQuestItemList`, `ExUserInfoEquipSlot`
  paperdoll, adena/weight, enchant/elemental blocks, initial equipment on create.
- **Skills (G7):** `SkillList`/`AcquireSkillList` (sent empty), full combat-stat
  calc (evasion/accuracy/etc. are base template values or 0), cast pipeline.
- **Quests (G10):** `QuestList` empty, `ExQuestItemList` empty.
- **Social (G9):** clan/ally blocks in `UserInfo`, `FriendList` empty, mail.
- **Misc:** macros, `HennaInfo` empty, real inventory limit, `maxLoad` calc,
  `ExUserBanInfo`, `ExVitalityEffectInfo` bonuses, real castle list for manor,
  game-time clock (CharSelected/UserInfo use 0), player persistence on logout.

---

## Tests / verification

- **Crypto:** golden vectors (`commons/tests`, `gameserver` cipher).
- **Protocol parity:** GS↔LS packet cross-checks (loginserver as gameserver
  dev-dep), `AuthRequest`/`BlowFishKey`/`PlayerAuthRequest` layouts.
- **DB:** `char_persistence.rs` — create/load/delete/restore against the stock
  schema.
- **Full E2E:** `e2e_create.rs` — real two-server login→create→enter-world with a
  scripted client; drains the enter-world burst; checks computed HP/MP.
- **UserInfo bytes:** unit test against a real client capture.

Run: `cargo test` (all green). Boot a pair on alt ports:
`cargo run -p loginserver` + `CONFIG_SERVER_GAMESERVERPORT=… cargo run -p gameserver`.

---

## Cross-cutting notes

- Game server runs from `dist/game`; all ini/data paths resolve unedited.
  `GameData::load_from(path)` lets tests point at the datapack from any cwd.
- Session lifecycle is a **type-state** machine (plan §3.1):
  `Connecting → Authenticated → InLobby → Entering → InGame`; the `Player` lives
  in `World.players` keyed by object id, `InGame` links by id.
- Masked packets use the reversed `DEFAULT_FLAG_ARRAY` bit order — get this right
  or the client desyncs (root cause of the earlier UserInfo mask fix).
