# G9.6 — Macros & panel shortcuts

> **Status: ✅ executed** — see PROGRESS.md §G9.6 for the as-built summary
> (one notable find: the Mystic class page's Self Heal overwrites the global
> Sit/Stand on slot 10, so a fresh Mystic panel is 5 slots, not 6).

Port of the Java shortcut-panel and macro systems: the 10×12 shortcut bar
(items/skills/actions/macros) persisted per character, and player-defined
macros stored server-side and pushed to the client. This is the milestone that
makes a relogged character's panel come back the way they left it, and makes
new characters start with the class-default panel.

**Gate:** create a character → it enters world with the `initialShortcuts.xml`
panel (Attack/Pick Up/Sit + class skills); drag a skill/item onto a slot,
create a macro and put it on the bar, relog → everything is still there;
delete an item shortcut and a macro (macro deletion clears its panel slots);
learning a skill upgrade updates the skill's shortcut level in place.

---

## 1. How it works in Java (reference map)

All under `interlude_classic/java/org/l2jmobius/gameserver/`:

| Piece | File |
|---|---|
| Shortcut DTO (slot 0-11, page 0-9, type, id, level, characterType, sharedReuseGroup) | `model/Shortcut.java` |
| Per-player shortcut registry + DB I/O | `model/ShortCuts.java` |
| Macro DTO (id, icon, name, descr, acronym, commands) + command (entry, type, d1, d2, cmd) | `model/Macro.java`, `model/MacroCmd.java` |
| Per-player macro registry + DB I/O (`;`/`,`-encoded commands column) | `model/MacroList.java` |
| Enums | `enums/ShortcutType.java` (NONE/ITEM/SKILL/ACTION/MACRO/RECIPE/BOOKMARK), `enums/MacroType.java` (NONE/SKILL/ACTION/TEXT/SHORTCUT/ITEM/DELAY), `enums/MacroUpdateType.java` (ADD=1/LIST=1/MODIFY=2/DELETE=0) |
| Client packets | `RequestShortCutReg` (0x3D), `RequestShortCutDel` (0x3F), `RequestMakeMacro` (0xCD), `RequestDeleteMacro` (0xCE) |
| Server packets | `ShortCutInit` (0x45), `ShortCutRegister` (0x44), `SendMacroList` (0xE8) — `ShortCutDelete` (0x46) exists but is never sent; deletion re-sends a full `ShortCutInit` |
| New-character defaults | `data/xml/InitialShortcutData.java` ← `dist/game/data/stats/initialShortcuts.xml` (global pages + per-classId pages + macro presets), applied in `CharacterCreate.runImpl` |
| Enter-world hooks | `EnterWorld.java:327` (`sendAllMacros()`, before ItemList) and `:339` (`ShortCutInit`, after ItemList) |
| Skill-upgrade hook | `ShortCuts.updateShortCuts` ← `RequestAcquireSkill` and `Player.addSkill`-on-level-up path |

Key facts that shape the port:

- **Macro execution is 100 % client-side** in this codebase. The server only
  stores macros and echoes them back via `SendMacroList`; when the player runs
  one, the client replays each command as ordinary packets
  (`RequestMagicSkillUse`, `Say2`, …). There is no server-side macro
  interpreter to port — but this also means the *only* server-side control
  point over what macros can do is registration time (`RequestMakeMacro`).
  A macro the server refuses to register does not exist client-side.
- **DB tables already exist** in our dev DB (`character_shortcuts`:
  charId/slot/page/type/shortcut_id/level/sub_level/class_index PK(charId,
  slot,page,class_index); `character_macroses`: charId/id/icon/name/descr/
  acronym/commands PK(charId,id)). No schema work.
- Storage key is `slot + page*12`; pages are validated to `0..=19` in the
  handlers even though the UI has 10.
- `class_index` is the subclass index — always **0** for us (no subclasses).
  `sub_level` likewise stays 0 (no skill sub-levels in Interlude data).
- `sharedReuseGroup`: skill shortcuts always write −1; item shortcuts copy
  the item template's `shared_reuse_group`, which **defaults to 0 and is
  never set in this dist's item XMLs** — so ITEM writes 0, SKILL writes −1,
  no new item-XML parsing needed.
- Macro ids are per-player, allocated from a counter starting at **1000**
  (client sends id 0 for "new", real id for "modify").

## 2. Deviation from Java: no recurring macros

Java happily registers macros containing `SHORTCUT`-type commands (type 4:
"press panel slot d2 of page d1"). Pointing such a command at a slot holding a
macro — including the macro's own slot — is the classic infinite-loop AFK
macro, and the client will chain them forever.

**We reject any macro containing a `SHORTCUT` command** in
`RequestMakeMacro`, answering with SM 810
(`INVALID_MACRO_REFER_TO_THE_HELP_FILE_FOR_INSTRUCTIONS`) like the other
validation failures. Blocking the command type outright (rather than checking
what the referenced slot currently holds) is the only airtight rule: slot
contents can be rebound after the macro is created, so a content check is
trivially bypassed. `SHORTCUT` is the only `MacroType` that can invoke another
macro, so this closes the loop vector completely; SKILL/ACTION/TEXT/ITEM/DELAY
commands all stay allowed. The `initialShortcuts.xml` macro presets must pass
this check (the stock "Macro Test" preset is TEXT-only — verify in a test).

## 3. Work items

### 3.1 Model (`model/shortcut.rs`, new)
- `ShortcutType` / `MacroType` enums (wire value = Java ordinal, with the
  handlers' out-of-range → NONE clamp on read).
- `Shortcut { slot, page, kind, id, level, character_type, shared_reuse_group }`.
- `Macro { id, icon, name, descr, acronym, commands: Vec<MacroCmd> }`,
  `MacroCmd { entry, kind, d1, d2, cmd }`.
- Player-only components (`model/components.rs`):
  `Shortcuts(BTreeMap<i32, Shortcut>)` keyed by `slot + page*12` (BTreeMap so
  `ShortCutInit` order is stable), and
  `Macros { next_id: i32 /* starts 1000 */, entries: Vec<Macro> /* insertion
  order, like Java's LinkedHashMap */ }`.
- Registration logic as component methods, mirroring `ShortCuts.java` /
  `MacroList.java`: ITEM shortcuts verified against the inventory
  (object id must exist, copy shared reuse group; drop silently otherwise),
  macro register (id 0 → allocate ≥1000 skipping taken ids → ADD; else
  replace → MODIFY), macro delete cascades into deleting every
  `ShortcutType::MACRO` panel slot with that id.

### 3.2 Data loader (`data/initial_shortcut.rs`, new)
Port of `InitialShortcutData`: parse `data/stats/initialShortcuts.xml` into
global pages, per-classId pages, and macro presets. `register_all` port runs at
character creation (`game_loop/lobby.rs`, after the initial-equipment replay so
ITEM entries can resolve item id → created object id): skip SKILL entries the
new character doesn't know, skip ITEM entries not in inventory, register preset
macros referenced by MACRO entries. Rows persist through the same DB commands
as runtime registration (Java persists them identically). Note Java also
sends `ShortCutRegister`/`SendMacroList` during creation — pointless (the
creating client is in the lobby) and our creation path has no in-world
session; persist only.

### 3.3 DB (`db.rs`)
- Load: extend the existing per-character select/enter-world load (where
  `character_skills` and `items` already load) with `character_shortcuts`
  (`WHERE charId=? AND class_index=0`) and `character_macroses` rows;
  carry them on `PlayerData` into `spawn_into`. Macro `commands` column is
  the Java `type,d1,d2[,cmd];…` string — parse with the same
  skip-malformed-token behavior as `MacroList.restoreMe`.
- New fire-and-forget commands, following the `UpsertSkill` pattern:
  - `UpsertShortcut { char_id, slot, page, kind, shortcut_id, level }`
    (INSERT OR REPLACE — Java does delete+insert; the PK makes upsert
    equivalent),
  - `DeleteShortcut { char_id, slot, page }`,
  - `UpsertMacro { char_id, id, icon, name, descr, acronym, commands }`
    (commands re-encoded to the Java string format, with Java's 255-char
    truncation quirk kept for round-trip parity),
  - `DeleteMacro { char_id, id }`.
- Restore-time verification (Java `ShortCuts.restoreMe` tail): after loading,
  drop ITEM shortcuts whose object id is no longer in the inventory
  (+ `DeleteShortcut`). The Java soulshot/`ExAutoSoulShot` branch is skipped —
  no auto-soulshot system.

### 3.4 Client packets (`game_loop/` handler + `client_packets.rs`)
- **`RequestShortCutReg` (0x3D)**: read type (clamped), `slot%12`/`slot/12`,
  id, level (i16), sub-level (i16), character type; reject page outside
  `0..=19`; register + persist + reply `ShortCutRegister`; then re-send
  `SkillList` (Java quirk — port it, the client expects it).
- **`RequestShortCutDel` (0x3F)**: slot/page from one int; delete + persist;
  reply is a full fresh `ShortCutInit` (that's what Java's `deleteShortCut`
  sends), no per-slot delete packet.
- **`RequestMakeMacro` (0xCD)**: read macro (≤12 commands hard-capped at
  read); validation order per Java: total command-string length > 255 → SM
  810, > 48 macros → SM 797, empty name → SM 838, descr > 32 chars → SM
  837; **plus the §2 no-SHORTCUT-command rule → SM 810**; then register
  (ADD or MODIFY) + persist + `SendMacroList` echo.
- **`RequestDeleteMacro` (0xCE)**: delete + cascade shortcut deletion +
  persist both + `SendMacroList` DELETE echo.

### 3.5 Server packets (`server_packets.rs`)
- **`ShortCutInit` (0x45)**: replace the empty stub in `enter_world.rs` with
  the real per-type layout (ITEM: id/enabled/reuse-group/0/0/augment 0/0/
  visual 0; SKILL: id/level i16/sub-level i16/reuse-group/0 u8/1; ACTION/
  MACRO/RECIPE/BOOKMARK: id/1).
- **`ShortCutRegister` (0x44)**: same per-type layout with the two extra
  trailing ints on SKILL (0, 0) and `character_type` where Init writes
  constants.
- **`SendMacroList` (0xE8)**: update-type u8, macro id (0 for LIST), count
  u8, has-macro u8, then the macro body (id, name, descr, acronym, icon,
  command count, per-command entry/type/d1/d2 u8/cmd) — body omitted for
  DELETE. Enter-world `sendAllMacros`: one packet per macro with
  `count = total` and LIST type, or a single empty LIST packet when the
  player has none.

### 3.6 Hooks
- **Enter world** (`enter_world.rs`): send the macro list at Java's position
  (~before `ItemList`) and the real `ShortCutInit` where the stub sits today.
- **Skill upgrade** (`game_loop/skills/mod.rs` learn path + the level-up
  `rewardSkills` auto-grant): port `updateShortCuts` — every SKILL shortcut
  for that skill id gets its level bumped, a `ShortCutRegister` re-sent, and
  the DB row upserted.
- **Item removal**: Java prunes item shortcuts via `deleteShortCutByObjectId`
  when an item leaves the inventory. No current system removes items
  (no drop/trade/destroy yet) — leave a TODO where those land; the
  restore-time prune (§3.3) covers stale rows in the meantime.

## 4. Out of scope
- Server-side macro execution (doesn't exist in the Java reference either).
- `ShortcutType::RECIPE`/`BOOKMARK` behavior (no crafting/bookmarks; the
  packet arms are trivial and can be written, but nothing produces them).
- Auto-soulshot interactions on shortcut delete (`ExAutoSoulShot`).
- Subclass `class_index` dimension and skill `sub_level`s (always 0).
- Summon/pet panel (`character_type` 2) — read and stored, nothing consumes it.

## 5. Tests
- **Packet units**: `ShortCutInit`/`ShortCutRegister`/`SendMacroList` byte
  layouts per type arm (hand-computed, same approach as `NpcInfo` — no
  capture available).
- **Loader**: `initialShortcuts.xml` against the real dist (global page 0
  actions, Human Mystic Wind Strike/Self Heal slots, the macro preset — and
  that the preset passes the no-SHORTCUT validation).
- **Synthetic-world integration** (`game_loop/tests.rs` pattern):
  - register skill/item/action shortcuts → `ShortCutRegister` + `SkillList`
    replies, DB rows present; delete → fresh `ShortCutInit`, row gone;
  - unknown-item ITEM registration silently dropped;
  - relog restore (store → reload `PlayerData`) round-trips the panel, and
    prunes an ITEM shortcut whose object id vanished;
  - macro create (id 0 → 1000, ADD echo) / modify (MODIFY echo) / delete
    (DELETE echo + its panel slot cascade-deleted);
  - validation rejections: >48 macros, empty name, long descr, long
    commands, **SHORTCUT-command macro → SM 810, nothing stored**;
  - skill learn upgrades a matching shortcut's level (packet + DB).
- **E2E** (`e2e_create.rs`): a freshly created character's enter-world burst
  now carries the class-default `ShortCutInit` (non-zero count) and the
  macro LIST packet; keep the skip-unsolicited helper in sync.

## 6. Risks / open points
- `SendMacroList`/`ShortCutInit` layouts come from the Mobius source, not a
  capture — if the client desyncs on the macro window or panel, capture a
  session against the Java server and byte-compare (same play as the
  `UserInfo` mask fix).
- Enter-world packet *order* matters to the client more often than expected;
  keep Java's macro-list-before-ItemList, ShortCutInit-after-ItemList order.
- The DB `commands` string format must round-trip exactly (`;`/`,` separators,
  no escaping — a `,` or `;` inside a TEXT command's body is ambiguous in
  Java too; port the same tokenizer behavior rather than inventing escaping).
