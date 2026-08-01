# G11 — Scripting engine + quests (+ clans via bypass)

Scope decision: the plan's full script-breadth gate ("port the remaining
~1,131 scripts") is cut to the **engine** slice: `RequestBypassToServer`
routing, a native quest framework with compiled-in trait-object scripts,
two hand-ported quests covering both drop flavors, and clan **creation**
through the ClanMaster village-master dialog. Script breadth stays G12.

Gate (live client): (1) dialog buttons work — `RequestBypassToServer` is
routed; (2) accept **Q00258_BringWolfPelts** at Lector (Talking Island),
kill wolves, watch pelts fill the quest inventory and the quest mark
advance, turn in for the reward, with the state surviving relog; (3) same
for **Q00320_BonesTellTheFuture** (chance drop + rated adena variant, Dark
Elf village); (4) create a clan at any ClanMaster, see the clan name over
the head and the pledge window populate, persisted across relog.

Quest picks (repeatable, no timers/memoState, near starter towns; both
`.htm` *and* `.html` results, so both html-window packets get exercised):
- **Q00258** — Lector 30001, wolves 20120/20442, deterministic 1-per-kill
  drop of item 702 ×40, cond 1→2, reward from a `getRandom(16)` table,
  level gate ≤9 (intro split at 3).
- **Q00320** — Kaitar 30359, skeletons 20517/20518, 0.18-chance drop
  (×`RateQuestDrop` = 10 on this dist → effectively guaranteed) of item
  809 ×10, `giveAdena(500, true)` (×`RateQuestRewardAdena` = 10), Dark Elf
  race gate, level gate ≤18 (intro split at 10).

## 1. How it works in Java (reference map)

### Bypass
- `clientpackets/RequestBypassToServer.java`, opcode **0x23**. The command
  arrives **without** the `bypass -h ` prefix (client strips it). Empty
  command → force disconnect.
- `validateHtmlAction`: every sent html records its `action=` strings; a
  bypass not on the record is dropped, and the recorded origin object id
  resolves the NPC for bare (non-`npc_`) commands, with an
  `INTERACTION_DISTANCE` re-check.
- Routing by prefix: `npc_<objectId>_<rest>` → NPC + distance check →
  `Npc.onBypassFeedback(player, rest)` (+ `ActionFailed` always); else
  `BypassHandler` keyed by first token. `VillageMaster` overrides
  `onBypassFeedback` for `create_clan` etc.
- `handlers/bypasshandlers/QuestLink.java` ("Quest"): no arg → chooser
  window over the NPC's talk quests (buttons colored by state, labels
  `<fstring>{questId}01/02/03</fstring>`, a single available quest
  short-circuits); `Quest <Name>` → `notifyTalk` (weight/40-quest guards);
  `Quest <Name> <event>` → `player.processQuestEvent` → `onEvent` (the
  event string is usually the target html filename).
- Both quest-window routes first probe every candidate with a **simulated**
  `onTalk` (`Quest.onTalk(npc, player, true)`) and drop the quests whose
  only answer would be `noquest.htm` — a quest with nothing to say at this
  NPC gets no button. Ported 2026-08 (`talk_shows_no_quest` +
  `QuestCtx::new_simulated`): without it a *completed* one-time quest was
  listed as `<fstring>{questId}03</fstring>`, and the class-change quests
  ship no `03` string client-side (`NpcStringId` has 40401/40402 but no
  40403), so a finished Q404 rendered a blank grey button at Parina that
  answered `noquest.htm` when clicked. Deliberate deviation inside the
  probe: Java guards only the `QuestState` setters and leaves
  `giveItems`/`takeItems`/`addExpAndSp` live, so probing Q404 while the
  player holds all four trinkets *eats them* and swallows the `exitQuest`;
  our simulated `QuestCtx` suppresses items, XP, packets, spawns, timers
  and teleports as well, so a probe can never cost a player anything.
- The ClanMaster htmls use *bare* `Quest ClanMaster 9000-XX.htm` bypasses
  (origin NPC recovered from the validateHtmlAction record) and
  `npc_%objectId%_create_clan $name` (the `$name` edit-box variable is
  substituted client-side).

### Quest framework
- Scripts are runtime-compiled Java under `dist/game/data/scripts/`; the
  `Quest` constructor self-registers with `QuestManager` (positive id =
  quest, id ≤ 0 = utility script). Listeners attach to *NPC templates* via
  `addStartNpc`/`addTalkId`/`addKillId`; `registerQuestItems` = destroyed
  on exit.
- `notifyTalk`: if the NPC starts this quest and `getStartConditionHtml`
  (the `addCondMinLevel`-family gate) is non-null → show that instead of
  `onTalk`. Results via `showResult`: `.htm`/`.html` → html file, inline
  `<html>` → direct, other → `sendMessage`. `.htm` results of quests with
  `0 < id < 20000` (≠999) go out as **`ExNpcQuestHtmlMessage` FE:0x8E**
  (the client quest window); `.html` → plain `NpcHtmlMessage` 0x19. Html
  path: `data/scripts/<script dir>/<file>`, fallback
  `data/scripts/quests/<Name>/`, no-quest default `data/html/noquest.htm`.
- `QuestState`: state byte (CREATED 0 / STARTED 1 / COMPLETED 2, persisted
  as `"Start"/"Started"/"Completed"`) + string var map. `startQuest()` =
  cond 1 + STARTED + `ItemSound.quest_accept`. `set("cond", …)` maintains
  the `__compltdStateFlags` skipped-step bitset (`QuestState.java:312-405`)
  and pushes **`QuestList` 0x86** (short count + per-started `(id,
  condBitSet)` + 128-byte one-time-completed mask) and **`ExShowQuestMark`
  FE:0x21**. `exitQuest(repeatable)`: destroy registered quest items,
  delete rows (all vs all-but-`<state>`), forget vs COMPLETED.
- Persistence: `character_quests(charId, name, var, value)` PK
  (charId,name,var), state under var `"<state>"`, row-per-var
  fire-and-forget writes (`insert_or_update_quest_var` upsert).
- Item primitives (`AbstractScript`): `giveItems` → SM 52/53/54 ("You have
  earned …") + `InventoryUpdate`; `giveItemRandomly` — chance *and* amount
  ×`RATE_QUEST_DROP`, capped at `limit`, `quest_itemget` sound
  (`quest_middle` exactly at the limit), returns limit-reached;
  `takeItems(-1)` = all; `giveAdena(n, true)` → `rewardItems` ×
  `RATE_QUEST_REWARD_ADENA`. `PlaySound` 0x9E.
- `onKill` fires from the `Attackable` kill path; Q00320 shares via
  `getRandomPartyMemberState(killer, cond, chance, npc)`.
- `RequestQuestAbort` client **0x63** → `exitQuest(true)` + `QuestList`.

### Clans
- `village_master/ClanMaster/ClanMaster.java` (id −1): ~60 NPC ids,
  `onTalk` → `9000-01.htm`, `onEvent` returns the event as the page with
  the `LEADER_REQUIRED` → `-no.htm` remap for non-leaders. Htmls live in
  the script folder.
- `ClanTable.createClan`, guards in order: level < 10 → SM 229; in a clan
  → SM 190; `clan_create_expiry_time` in future → SM 230; not alphanumeric
  or len < 2 → SM 261; len > 16 → SM 262; name taken (in-memory scan) →
  SM 5. Success: `Clan` + leader member, `clan_data` INSERT (13 columns),
  leader gets the all-bits `ClanPrivilege` mask ((1<<24)−1, ordinal = bit),
  packets `PledgeShowInfoUpdate` 0x8E + `PledgeShowMemberListAll` 0x5A +
  `PledgeShowMemberListUpdate` 0x5B + SM 189 + `broadcastUserInfo`.
  Member persistence is `characters.clanid`/`clan_privs`; rosters restore
  from `characters WHERE clanid=?`.
- EnterWorld sends the pledge window to the member and the online-status
  update to the rest; `deleteMe` pings offline.

## 2. What was built (Rust)

- **Bypass** (`game_loop/bypass.rs`): 0x23 dispatch; `npc_<oid>_<cmd>`
  parse + existence + `INTERACTION_DISTANCE` check → verb router (`Quest` →
  quest engine, `create_clan` on `VillageMaster*` templates → clan flow,
  rest log-drop) + unconditional `ActionFailed`; bare `Quest …` resolved
  through the new **`LastFolkNpc`** component (set on every NPC click in
  `handle_action`, Java `NpcAction.action` parity) + distance re-check.
- **Quest state** (`model/quest.rs` + `Quests` component): `QuestState`
  {state byte, var map}, `cond_bit_set()` (incl. the legacy bit-31 unpack),
  and the `__compltdStateFlags` math as the pure `updated_cond_flags`
  (unit-tested against hand-traced Java values). DB:
  `load_quests` (state rows define existence, orphan vars dropped) +
  fire-and-forget `UpsertQuestVar`/`DeleteQuestVar`/`DeleteQuest
  {keep_state}`/`DeleteItem` commands — Java-schema-compatible rows.
- **Engine** (`game_loop/quests.rs`): `QuestScript` trait (stateless
  compiled-in scripts: id/name/html_dir/start_npcs/talk_npcs/kill_npcs/
  quest_items + start_condition_html/on_talk/on_event/on_kill/on_timer) +
  `QuestRegistry` (name + per-npc start/talk/kill indexes) behind
  `World.quests: Arc<…>` (the `World.geo` borrow pattern; tests install
  synthetic registries). `QuestCtx` ports the `QuestState`/`AbstractScript`
  primitives: `start_quest`, `set_cond` (flags math → upsert + `QuestList`
  + `ExShowQuestMark` + optional middle sound), `exit_quest`, `give_items`
  / `reward_items` / `give_adena` / `give_item_randomly` (×`RateQuest*`
  configs, `world.roll_f64()`), `take_items`, `add_exp_and_sp`
  (×`RateQuestRewardXP/SP`), timers. Entry points: `quest_link` (chooser /
  talk / event split per QuestLink), `show_result` (`.htm` quest window vs
  `.html`/inline plain window, `%objectId%`/`%playername%`), `notify_kill`
  (called from `death::npc_do_die` after combat rewards — a top-level
  `&mut World` pass, no borrow gymnastics), `RequestQuestAbort` 0x63, and
  `ScheduledTask::QuestTimer` with the cast_seq-style stale-seq no-op
  (`QuestTimerSeqs`).
- **Packets**: real `QuestList` (replaces the G4 stub; one-time mask with
  Java's id-range exclusions), real `ExQuestItemList` (the `is_quest_item`
  complement of `ItemList`), `ExShowQuestMark`, `ExNpcQuestHtmlMessage`,
  `PlaySound`, and `InventoryUpdate` grown a removed-entry variant
  (`inventory_update_changes`).
- **Items**: first removal path — `Inventory::remove_item` (negative =
  all, multi-instance loop, returns `ItemChange::Modified/Removed`
  snapshots) + the `take_items` wrapper (DB `UpdateItemCount`/`DeleteItem`
  + removed-type `InventoryUpdate` + quest-tab refresh). The
  stack-or-create half of `Player.addItem` extracted from the G9 loot path
  into `items::add_inventory_item`, shared by loot and quest gives.
- **Scripts** (`src/scripts/`): `build_registry()` = Java's boot-time
  script pass. `Q00258BringWolfPelts`, `Q00320BonesTellTheFuture` —
  mechanical ports (Q00258's reward table read as ascending-bound
  thresholds; Java's HashMap iteration made its odds order-dependent) —
  and `ClanMaster` (the 60 NPC ids, `LEADER_REQUIRED` remap verbatim; the
  Clan Advent login/logout skill listeners unported — no clan skills).
- **Clans** (`model/clan.rs` + `game_loop/clans.rs`): `Clan`/`ClanMember`
  + `World.clans`, loaded at boot via an unprompted `DbEvent::ClansLoaded`
  (same pattern as the first `IdBlock`). `create_clan` with Java's guard
  order and SM ids; clan id from the shared `IdManager` pool
  (`alloc_object_id`); `InsertClan` + `UpdateCharClan` persistence
  (`StorePlayer`'s UPDATE doesn't touch clan columns); the pledge packet
  trio + SM 189 + full UserInfo/CharInfo re-broadcast. `Player` grew
  `clan_id`/`clan_privs`/`clan_leader`/`clan_create_expiry_time`
  (leader flag fixed up at enter-world from the live table). Clan id now
  real in `UserInfo` CLAN block, `CharInfo`, `CharSelectionInfo`,
  `CharSelected`; clan chat broadcasts to online members (SM 4202 only for
  the clanless); enter/leave world send `PledgeShowMemberListAll`/`…Update`.

## 3. Deliberate deviations from Java

1. `validateHtmlAction` unported — `LastFolkNpc` + the distance check +
   in-handler guards stand in; unknown bypasses log-and-drop.
2. Empty bypass logs instead of force-disconnecting (G10 `Say2` precedent).
3. Quest kill credit is **killer-only** (`getRandomPartyMemberState`
   deferred with party quest sharing).
4. Quest-window guards skipped: weight penalty / inventory-90% / 40-quest
   cap (no weight model), the chooser's simulated-`onTalk` pre-filter (the
   `_simulated` machinery), `sendNpcLogList`.
5. Q00258's reward roll uses ascending-bound thresholds (deterministic
   reading of Java's order-dependent HashMap scan).
6. `ClanNameTemplate` regex not ported (`.*` on this dist); the
   alphanumeric/length checks are Java's own.
7. Clan crests, RELATION bits, and the Clan Advent buff stay absent; the
   full UserInfo/CharInfo re-send stands in for `RelationChanged`.
8. Quest timers supersede a live same-key timer instead of refusing the
   duplicate (safer under the seq scheme; nothing shipped relies on it).

## 4. Deferred (owning milestone)

Clan invite/leave/dissolve/level-up/wars/ally/academy/sub-pledges, clan
skills + `PledgeSkillList`, crests, notices, warehouse, `PledgeInfo`/
`PledgeStatusChanged` beyond the creation trio; quest party sharing; daily
quests (`restartTime`); `onFirstTalk`/`onAttack`/`onSpawn` registrations
(trait has no slots yet — add with first consumer); tutorial (Q00255);
`ExQuestNpcLogList`; the remaining ~198 quests and all `ai/` scripts;
other bypass families (`Link`, `multisell`, `learn_clan_skills`, `item_`,
`admin_`, `_bbs`, menu/manor selects); `HtmCache` (per-interaction disk
reads stand).

## 5. Tests

- `model::quest` units: state-name round trip, cond gating, bit-31 unpack,
  the four `updated_cond_flags` branches with hand-traced values.
- `char_persistence::quest_states_persist`: real DB thread — upserts, the
  repeatable/non-repeatable delete split, orphan-var filter.
- Synthetic-world (`game_loop::tests`): bypass parse/range/malformed/
  LastFolkNpc; the full Q00258 loop (quest window packet split, accept
  persistence, kill drops + quest-tab refresh + earned-SMs, cond-2 mark +
  sounds, turn-in with forced reward roll, removed-type InventoryUpdate +
  DB deletes, repeatable re-offer); Q00320's forced-roll chance path,
  limit semantics and rated adena; abort; a synthetic-script quest timer
  (fire once, stale-seq cancel); the clan guard matrix + success packet
  trio + persistence commands; ClanMaster leader gating against the real
  dist htmls; roster notifications + clan chat scoping.
- `e2e_create` unchanged (QuestList shape is compatible when empty).
