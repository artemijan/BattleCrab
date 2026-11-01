# Network protocol: Classic Interlude (p110) → Salvation (p140)

Java trees compared:
- J1 = `/Users/artem/dev/l2/interlude_classic/java/org/l2jmobius` (protocol 110, `AllowedProtocolRevisions = 110`)
- J2 = `/Users/artem/dev/l2/gitlab/L2J_Mobius/L2J_Mobius_05.0_Salvation/java/org/l2jmobius` (protocol 140, `AllowedProtocolRevisions = 140`)


**Headline:** Salvation sits much closer to Classic Interlude on the wire than the version jump suggests. Mobius' Classic trees were forked from the mainline tree, so the opcode tables are nearly identical. Crypto, the handshake and the login protocol are unchanged. The real work is concentrated in a few changes: (a) the item-info encoding (+2 bytes per item, plus an optional ensoul block); (b) the protocol-140 **two-phase item-list** scheme (`sendType` 1 = header, 2 = items) and its short→int count widening across about 15 list packets; (c) a handful of character/self packets (UserInfo, CharSelectionInfo, ExUserInfoEquipSlot, AcquireSkillList, henna); (d) four client packets that gained fields.

Normalized file-diff noise is large: 121 of 485 shared server packets and 169 of 356 shared client packets differ textually. Almost all of that is the Mobius refactor (`ClassId`→`PlayerClass`, `Config`→`XxxConfig`, `CharSelectInfoPackage`→`CharacterInfoHolder`, `getActingPlayer()`→`asPlayer()`, BazookaRpm "readability" rewrites). Only **43 server packets** and **~10 client packets** change on the wire.

---

## 1. Opcode tables

### 1a. `ClientPackets.java` (single-byte client opcodes): 170 → 170
| change | detail |
|---|---|
| renamed | `0x0F MOVE_BACKWARD_TO_LOCATION/MoveBackwardToLocation` → `MOVE_TO_LOCATION/MoveToLocation` (same opcode, identical `readImpl`) |
| handler class renamed | `REQUEST_SHORT_CUT_REG/DEL` → `RequestShortcutReg/Del` (identical reads) |
| handler added | `NET_PING` null → `RequestNetPing` (server also gains `NetPing` packet: `writeInt(time)`) |
| renumbered/removed | **none** |

### 1b. `ExClientPackets.java` (0xD0 sub-opcodes): 303 → 342
- **+39 added**:
  - `REQUEST_TARGET_ACTION_MENU 0xFE` (has a handler)
  - `EX_CARD_UPDOWN_* 0x137–0x13A` (null handlers)
  - `EX_ARENA_RANK_ALL/MYRANK 0x13B–0x13C` (null)
  - `EX_SWAP_AGATHION_SLOT_ITEMS 0x13D` (null)
  - pledge V2 `0x13E–0x14F` (14 with handlers: contribution rank/info/reward, level-up, mission info/reward, mastery info/set/reset, skill info/activate, item list/buy, announce/announce-set)
  - `EX_ELEMENTAL_SPIRIT_* 0x150–0x158` (null)
  - `EX_REQUEST_LOCKED/UNLOCKED_ITEM, EX_LOCKED/UNLOCKED_ITEM_CANCEL 0x159–0x15C` (null)
- **3 renumbered**:
  - `REQUEST_BLOCK_LIST_FOR_AD 0x136→0x15D`
  - `REQUEST_UPGRADE_SYSTEM_RESULT 0x137→0x136`
  - `REQUEST_USER_BAN_INFO 0x138→0x15E`
- **0 removed**.
- **29 handler-presence changes** on unchanged opcodes:
  - These were `null` in Interlude and are now implemented: CubeGame change-team/ready-answer, CallToChangeClass, Ceremony of Chaos ×4, Ability ×6, Alchemy ×2, ItemEnsoul, TryEnSoulExtraction, UserFactionInfo, RaidBossSpawnInfo, RaidServerInfo, ItemAuctionStatus, MonsterBook ×3.
  - These were implemented in Interlude and are now `null`: `REQUEST_TODO_LIST`, `REQUEST_ONE_DAY_REWARD_RECEIVE`, `REQUEST_PLEDGE_BONUS_OPEN/REWARD_LIST/REWARD`.
  - These are feature changes, not protocol changes.

### 1c. `ServerPackets.java` (incl. 0xFE ex): 721 → 763
- Main single-byte opcodes: **identical**.
- **+42 added ex ops at FE 0x1CF–0x1F8**: card up/down ×5, arena rank/myrank/observe, `EX_PLEDGE_CLASSIC_RAID_INFO`, `EX_HTML_WITH_NPC_VIEWPORT`, pledge V2 (contribution, raid info/rank, level-up, `EX_PLEDGE_SHOW_INFO_UPDATE`, mission, mastery, skill, item, announce, `EX_SET_PLEDGE_EMBLEM`, `EX_SHOW_CREATE_PLEDGE`), `EX_TUTORIAL_SHOW_ID 0x1E5`, elemental spirit ×7, `EX_CHOOSE_LOCKED_ITEM`, `EX_LOCKED_RESULT`.
- **5 renumbered**:
  - `EX_DRESS_ROOM_UI_OPEN 0x1CD→0x1F9`
  - `EX_DRESS_HANGER_LIST 0x1CE→0x1FA`
  - `EX_SHOW_UPGRADE_SYSTEM 0x1CF→0x1CD`
  - `EX_UPGRADE_SYSTEM_RESULT 0x1D0→0x1CE`
  - `EX_USER_BAN_INFO 0x1D1→0x1FB`
- **0 removed**.

Server packet classes: 9 exist only in Interlude:
- `ShortCutInit/Register`, renamed to `ShortcutInit/Register` with an identical wire format
- dailymission ×2
- pledgebonus ×4
- `CSShowComBoard`

44 exist only in Salvation: ability, alchemy, ceremonyofchaos, ensoul, faction, monsterbook, pledgeV2 ×11, raidbossinfo, `ExItemAuctionStatus`, `ExPVPMatch*`, `ExTutorialShowId`, `NetPing`. Client packet classes: 9 only in Interlude, 44 only in Salvation (same feature families).

**Rust impact of opcode changes: tiny.** None of the renumbered server ex ops (dress room, upgrade system, user ban info) is emitted by Rust (grep confirms). On the client side only `REQUEST_USER_BAN_INFO` matters: it is defined in `crates/gameserver/src/network/client_packets/ex_opcodes.rs:16` as `0x138` and swallowed in `game_loop/client/dispatch.rs:694-699`. It must become `0x15E`, otherwise the new `0x138` (`EX_CARD_UPDOWN_GAME_REWARD_REQUEST`, null) would be the one swallowed, which is also harmless. Optionally rename `MOVE_BACKWARD_TO_LOCATION` in `client_packets/opcodes.rs:4`.

---

## 2. Server packet `write` changes (shared packets)

43 packets differ in wire signature. Ranked by importance:

### Tier 1: ripples everywhere / breaks login→world
1. **`AbstractItemPacket.writeItem` (item-info encoding)**. Used by ItemList, InventoryUpdate, every trade/store/warehouse/mail/multisell/buy list, ExRpItemLink, etc.
   - **+2 bytes after `isAvailable`**: `writeByte(0); writeByte(0); // 140 protocol`.
   - New mask bit `ItemListType.SOUL_CRYSTAL = 16`. When an item has ensoul options it appends `byte n, int[n] options, byte m, int[m] specialOptions`. With no ensoul the bit is clear and nothing is appended.
   - `getBodyPart()` → `getBodyPart().getMask()`: same long value, but the agathion bodypart mask is `0x3000000000`.
   - New overload `writeItem(TradeItem, long count, ...)` (used by TradeUpdate).
2. **Two-phase item lists (`_sendType` 1 = header-only, 2 = items).** The sender now sends each packet twice (`new X(1, ...)`, `new X(2, ...)`), and counts widen from `short` to `int`.
   - `ItemList`:
     - Interlude: `short showWindow, short n, items, inventoryBlock`.
     - Salvation type 1: `byte 1, int 0, int n, inventoryBlock`.
     - Salvation type 2: `byte 2, int n, int n, items, inventoryBlock`.
     - EnterWorld and `Player.sendItemList` send both.
   - `ExQuestItemList`, `GMViewItemList`, `WareHouseDepositList`, `WareHouseWithdrawalList`, `GMViewWarehouseWithdrawList`, `PackageSendableList`, `TradeStart`, `TradeOwnAdd`, `TradeOtherAdd`, `TradeUpdate`, `PrivateStoreManageListSell`, `PrivateStoreManageListBuy`, `ExReplyPostItemList`, `commission/ExResponseCommissionItemList`: all get the same byte `sendType` prefix with a branch. Warehouse lists also drop the stackable-id block and change `short`→`int` counts. Trade packets change `short 1`→`int 1`.
   - `AbstractInventoryUpdate` (InventoryUpdate / PetInventoryUpdate): header `short n` → `byte 0, int 0, int n`. It stays a single packet.
3. **`CharSelectionInfo`**
   - Header gains 1 byte: a new "gift message" byte before the Balthus byte (Interlude has 1 byte, Salvation 2).
   - Per char: **+5 `writeInt(0) // Salvation`** between the paperdoll item ids and the visual-id block. The paperdoll order itself is unchanged.
   - Delete-timer field becomes `-1` for banned chars (accessLevel < 0).
   - Hero glow becomes a `bool` (Interlude wrote `2`).
4. **`UserInfo`** (masked)
   - Mask bit count `writeShort(23)` → `writeShort(24)`. `UserInfoType` still has 23 entries and the mask is still 3 bytes, so Rust must hardcode 24, not `VALUES.len()`.
   - **SLOTS block** grows from 9 to 11 bytes. Interlude: `short 9, talisman, brooch, team, 0, 0, 0, 0`. Salvation: `short 11, talisman, brooch, team, int 0, byte charmSlotsFlag, byte agathionSlots-1`.
   - Value-only changes:
     - name = visible name
     - LUC/CHA real values
     - crystallize-grade byte
     - ability-points byte
     - 6 defense element shorts
     - attack element byte + short
     - noble *level* (byte)
     - `getArmorSetEnchant`
5. **`ExUserInfoEquipSlot`**: `InventorySlot` grows from 33 to 38 entries (AGATHION1–5 inserted after the hair/bracelet slots). The count short becomes 38, the mask stays 5 bytes, and there are 38 blocks of 22 bytes.

### Tier 2: common gameplay
- **`AcquireSkillList`**:
  - skill level `short`→`int`
  - adds `byte dualClassLevel` (was 0)
  - adds a new `byte isNew` (`getKnownSkill == null`)
  - removed-skill level `short`→`int`
- **`HennaEquipList`**: `+int duration` per entry.
- **`HennaItemDrawInfo`**: tail grows: `int LUC, short LUC+henna, int CHA, short CHA+henna, int duration` (Interlude had 1 int).
- **`HennaInfo`**: identical layout (LUC/CHA real values, premium slot filled).
- **`PartySpelled`**: `+short subLevel` per effect, in both loops. Rust doesn't implement PartySpelled yet.
- **`GMViewPledgeInfo`**: member online field `int`→`byte` plus a trailing `int 0`.
- **`ExChangeAttributeItemList`**: `+byte type`.
- **`MultiSellList`**: the two zero bytes become `writeItemEnsoulOptions(...)`, which is wire-identical when empty. `bodyPart.getMask()`.
- **`SpawnItem`/`DropItem`**: the "Grand Crusade" byte becomes the special-ability count (value only).
- **`ValidateLocation`**: trailing byte `0xFF` → `1` (value).
- **`SkillCoolTime`**: `max(1, remaining/1000)` (value).
- **`GMViewCharacterInfo`**: uses `ServerPacket.PAPERDOLL_ORDER`, which **gains AGATHION1–5** (5 more ints in the paperdoll block). LUC/CHA values.
- **`PledgeShowMemberListAll` / `PledgeShowMemberListUpdate` / `PledgeReceiveMemberInfo`**: sub-pledge branches removed. Same layout per branch, with zeros for sex/race/sponsor/online (semantic, not layout).
- **Wire-identical despite big text diffs:** `ExFriendDetailInfo`, `ExBlockDetailInfo` (moved to constructor), `SiegeDefender/AttackerList`, `ExShowAgitInfo`, `ExRequestPartyMatchingHistory`, `RecipeShopManageList`, `AbstractHtmlPacket`, `ExShowScreenMessage`, `ExAlterSkillRequest` (config gate), `StatusUpdate`, `MagicSkillLaunched`, `NpcInfo`, `CharInfo`, `ExPetInfo`, `SkillList`, `AbnormalStatusUpdate`.

### Masked packets
- `UserInfoType`, `NpcInfoType` and `StatusUpdateType` enums: **identical**.
- `ItemListType`: +`SOUL_CRYSTAL(16)`.
- `InventorySlot`: +5 agathion slots.
- NpcInfo/SummonInfo/ExPetInfo: no wire change.

### Cross-cutting (model, not just protocol): paperdoll indices shift
Salvation inserts `PAPERDOLL_AGATHION1..5 = 17..21`, so `PAPERDOLL_TOTALSLOTS` goes from 32 to 37 and every later slot (DECO1–6, BELT, BROOCH, BROOCH_JEWEL1–6) shifts by +5. This affects:
- `RequestUnEquipItem` (now reads a body-part mask via `BodyPart.fromPaperdollSlot`)
- `InventorySlot`
- `PAPERDOLL_ORDER` arrays
- the persisted `items.loc_data` for equipped items (needs a DB migration)

Coordinate this with the model/DB axis.

---

## 3. Client packet `read` changes (shared packets)
| packet | change | severity |
|---|---|---|
| `SetPrivateStoreListBuy` (0x9A) | per item, after `visualId`: `byte n, int[n] ensoul, byte m, int[m] specialEnsoul` | **High**: the format is sequential, so item 2+ desyncs if not parsed |
| `MultiSellChoose` | trailing `byte n, int[n], byte m, int[m]` ensoul options | low (trailing) |
| `RequestRecipeItemMakeSelf` | trailing `int offeringsCount, {int objId, long count}[]` | low (trailing) |
| `RequestRecipeShopMakeItem` | trailing offerings block, same shape | low (trailing) |
| `RequestEnchantItem` | `supportId` no longer read (commented out) | none: Rust already does `read_i32().unwrap_or(0)` (`game_loop/items/enchant.rs:320`) |
| `RequestUnEquipItem` | same int, now interpreted as a paperdoll slot → BodyPart | semantic (see paperdoll shift) |
| `CharacterCreate` | race now used (already read) | none |
| `RequestSendPost` / `SendWareHouseDepositList` | validation `count < 1` | none |
| everything else (RecipeShopListSet, PrivateStoreSell/Buy, PledgeReorganizeMember, LuckyGamePlay, PledgeDraftListSearch, ListPartyMatchingWaitingRoom) | refactor only | none |

---

## 4. Handshake, crypto, login
- **Game cipher** (`gameserver/network/Encryption.java`): rewritten for readability but algorithmically identical (same XOR chain, same key[8..11] rolling offset, first `encrypt` only enables).
- **`BlowFishKeygen`**: same (8 random bytes + fixed tail `C8 27 93 01 A1 6C 31 97`).
- **`KeyPacket`**: identical layout: `byte result, 8 key bytes, int encryption, int serverId, byte 1, int 0 (obfuscation key), byte isClassic`. **No client-opcode obfuscation/shuffling** in either tree (obfuscation key always 0).
- **`ProtocolVersion`**: identical apart from a debug `println`. Only the allowed list changes (config, 110 → 140).
- **Config defaults differ:**
  - Salvation ships `PacketEncryption = False` (Interlude: True). Rust already honours `packet_encryption` (`network/connection.rs`, `network/game_client.rs`, `server_packets/lobby.rs`).
  - `ServerListType = Normal`, so the KeyPacket `isClassic` byte is 0 and the login ServerList type changes (Interlude: Classic = 0x400).
- **Login server**: `LoginEncryption`, `NewCrypt` (both copies), `BlowfishEngine` and `ScrambledKeyPair` are refactored but algorithmically identical. Login client/server packets (`Init`, `RequestAuthLogin`, `ServerList`, `RequestServerLogin`, `AuthGameGuard`, `PlayOk`...) have **zero wire changes**; the normalized diff of `loginserver/network/serverpackets` is empty apart from debug printlns.
- **GS↔LS internal link** (`GameServerAuth`, `BlowFishKey`, `PlayerInGame`, `ServerStatus`): hardening/refactor only, same wire. Rust's `commons/src/crypt/*` and `loginserver/src/network/*` need **no changes**.
- **SystemMessageId** (4908 → 5220 entries; 342 ids new, 30 gone, 11 names renumbered such as `BEGIN_MATCHMAKING 5509→5521`) and **NpcStringId** (8502 → 11419) change. Rust's `crates/commons/src/system_messages/generated.rs` is generated (`l2r-tools gen-messages`), so it needs regenerating from the Salvation client's `SystemMsg*.dat`, with a check of the few ids Rust uses by name.

---

## 5. Mapping onto the Rust code

Rust network layer: about 13.7k lines in `crates/gameserver/src/network`. Client-packet *parsing* is mostly inline in `game_loop/*` handlers, routed from `game_loop/client/dispatch.rs`.

| Area | Rust location | Change | Size |
|---|---|---|---|
| Item encoding | `network/enter_world.rs:33 write_item_entry` (26 refs: `trade.rs`, `server_packets/{private_store,npc,mail,warehouse,player_trade,gm_view}.rs`, `game_loop/items/item_auction.rs`) | +2 zero bytes after `available`; optional SOUL_CRYSTAL mask bit + ensoul block once ensoul exists | S (1 fn) + test fixture updates |
| ItemList / ExQuestItemList / GMViewItemList | `network/enter_world.rs:75,99,550`; senders in `game_loop/*/inventory::send_inventory_item_list` and the enter-world burst | two-phase: builder takes `send_type`, callers emit 1 then 2; int counts | M |
| InventoryUpdate (+ pet) | `network/enter_world.rs:137 inventory_update_changes`, pet variant in `server_packets/npc.rs` | header `byte 0, int 0, int n` | S |
| Warehouse / package | `server_packets/warehouse.rs:21,51,96`; GM: `server_packets/gm_view.rs:237` | sendType split, drop stackable block, int counts | M |
| Trade | `server_packets/player_trade.rs:23,42` (+ trade update) | sendType split, `int 1` counts, `writeItem(item, count)` | S–M |
| Private store manage lists | `server_packets/private_store.rs:45,107` | sendType split | S–M |
| Mail item list | `server_packets/mail.rs:145` | sendType split | S |
| CharSelectionInfo | `server_packets/lobby.rs:89` | +1 header byte, +5 ints/char, banned −1, hero bool | S |
| UserInfo | `network/user_info.rs:59` (`write_i16(VALUES.len())` at :101) | hardcode 24; SLOTS block 11 bytes (+int, charm flag, agathion count); fill LUC/CHA/elements/noble-level if available | S–M |
| ExUserInfoEquipSlot | `network/enter_world.rs:707`, `crate::enums::InventorySlot` | +5 agathion slots (33→38); update `masks.rs` test (`build_mask::<5>(0..33)`) | S, but depends on the paperdoll renumbering |
| AcquireSkillList | `network/enter_world.rs:316` | level `i16`→`i32`, +byte dual-class, +byte isNew | S |
| Henna | `server_packets/henna.rs:63,90` | +duration int; draw-info tail | S |
| GMViewPledgeInfo | `server_packets/clan.rs:267` | online `int`→`byte` + trailing int | XS |
| GMViewCharacterInfo | `server_packets/gm_view.rs:27` | paperdoll order +5 agathion ints | XS |
| ValidateLocation | `server_packets/movement.rs:20` | trailing byte 0xFF→1 | XS |
| Opcodes | `client_packets/ex_opcodes.rs:16` (`0x138`→`0x15E`), `client_packets/opcodes.rs:4` (rename) | | XS |
| Client reads | `game_loop/commerce/private_store.rs` (`handle_set_list_buy`), `game_loop/commerce/multisell.rs`, `game_loop/commerce/crafting.rs` (make-self / shop-make) | parse ensoul arrays per item (required) and trailing offerings (optional) | S |
| Masks | `network/masks.rs` | no logic change; test constants only | XS |
| Handshake/crypto | `network/cipher.rs`, `network/connection.rs`, `server_packets/lobby.rs:16`, `crates/commons/src/crypt/*`, `crates/loginserver/src/network/*` | none (config: protocol list 140, `packet_encryption`, server list type Normal) | none |
| System messages | `crates/commons/src/system_messages/generated.rs` | regenerate from the Salvation client dat | S (tooling exists) |
| Paperdoll renumbering | `crate::enums::InventorySlot`, inventory model, DB `loc_data` | cross-axis (model/DB) | M, owned by the model axis |

Byte-level tests also need updating: e.g. `network/enter_world.rs` equip-slot tests (33 blocks), `masks.rs` tests, `tests/handshake.rs`, `tests/e2e_create.rs`, and any `game_loop/tests/*` that decode ItemList/InventoryUpdate/CharSelectionInfo.

### Effort (protocol axis only, excluding new Salvation features)
- Opcodes + config + handshake: **< 0.5 day**
- Item encoding + InventoryUpdate: **0.5 day**
- Two-phase list packets (about 15 packets plus call sites that must send twice): **1.5–2 days**
- UserInfo / CharSelectionInfo / EquipSlot / AcquireSkillList / henna / small ones: **1–1.5 days**
- Client read changes: **0.5 day**
- Test fixture churn + live-client verification against a p140 client: **1–2 days**
- **Total ≈ 5–7 dev-days.** On top of that come the paperdoll/agathion renumbering (model/DB axis) and *optional* feature packets (pledge V2, ability, alchemy, ensoul, CoC, monster book, elemental spirits: 44 new server and 44 new client classes) if those features are wanted.

### Recommended ordering
1. Config + handshake (protocol 140, encryption flag, server list type) and the `REQUEST_USER_BAN_INFO` opcode, to get to char select.
2. `CharSelectionInfo` (+header byte, +5 ints): unblocks the lobby.
3. Item encoding (+2 bytes) + `ItemList` two-phase + `InventoryUpdate` header + `UserInfo` (24, SLOTS) + `ExUserInfoEquipSlot`, coordinated with the paperdoll/agathion renumbering. Together these unblock EnterWorld without a client crash.
4. `AcquireSkillList`, henna, `GMView*`, `ValidateLocation`.
5. The remaining two-phase list packets (warehouse, trade, private store, mail, package) and the client reads (`SetPrivateStoreListBuy` first, then multisell/recipe).
6. Regenerate system messages/NpcStrings, update byte-level tests, then do a live-client smoke run of each flow.
