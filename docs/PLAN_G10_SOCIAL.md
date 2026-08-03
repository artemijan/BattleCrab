# G10 — Social systems (vertical slice: chat + party + friends)

Scope decision: the plan's full G10 gate ("form clans and parties, add
friends, send mail, use the community board") is cut down to the slice that
two live clients can actually exercise today: **chat, party, friends**.

- **Clans deferred**: clan *creation* only happens through village-master NPC
  bypass dialogs (`RequestBypassToServer`), and bypass handling doesn't exist
  yet (it's the G11 quest-engine gate anyway). Porting `Clan.java` (3054
  lines) with no way to create a clan buys nothing. Clan chat answers with
  SM 1699 "you are not in a clan", like Java does for clanless players.
- **Mail / community board / party matching rooms / command channels
  deferred**: separate subsystems with their own packet families; nothing in
  this slice depends on them.
- Chat is *in* even though the plan text didn't name it — `Say2`/
  `CreatureSay` are unhandled today, party chat and whisper are the social
  glue, and the port is small.

Gate: two clients can talk (general/shout/trade/whisper), form a party
(invite → accept → small window), share XP/SP and loot from a kill with
Java's split math, watch each other's HP/MP/CP bars move, befriend each
other (persisted), and message friends. All against the real client.

## 1. How it works in Java (reference map)

### Chat
- `clientpackets/Say2.java` (client 0x49): reads `text`, `type:int`,
  `target:String` (WHISPER only). Guards: unknown type / empty text →
  `ActionFailed` + disconnect; length > 105 (no item link) or > 500 (with
  item link, char 0x08 in text) → SM 1352 spam warning; then dispatches to a
  per-`ChatType` handler (`handlers/chathandlers/*`).
- `ChatType` client ids: GENERAL 0, SHOUT 1, WHISPER 2, PARTY 3, CLAN 4,
  TRADE 8, ALLIANCE 9, HERO_VOICE 17 … (enum in `enums/ChatType.java`).
- `serverpackets/CreatureSay.java` (server 0x4A): objectId, chatType id,
  sender name (or charId), NpcString id (-1 for plain text), text; WHISPER
  from a player appends a relation-mask byte + sender level (mask bit 0x10 =
  GM hides level; our mask is always 0 — no friends-of/clan bits needed for
  the *receiver-relation* mask until clans, and friend bit is cheap to add).
- `ChatGeneral`: broadcast to visible players within **1250** units +
  echo to self. (`.command` voiced-command branch: no voiced commands exist —
  skip.)
- `ChatShout`/`ChatTrade`: dist runs `GlobalChat = ON` / `TradeChat = ON` →
  send to every player with the same `MapRegionManager.getMapRegionLocId`
  (the map-region *tile* id, loader already ported in G9 for respawns).
- `ChatWhisper`: by name, case-insensitive; offline → SM 3 "not online";
  receiver gets `CreatureSay(sender, WHISPER, name, text)`, sender gets the
  `"->Name"` echo.
- `ChatParty`: `party.broadcastCreatureSay` (all members incl. self).
- `ChatClan` (and ALLIANCE): SM 1699 you-are-not-in-a-clan and stop.

### Party
- `model/Party.java`. Members: ordered list, leader = index 0. Party level =
  max member level (`recalculatePartyLevel`). `PartyDistributionType`:
  FINDERS_KEEPERS 0, RANDOM 1, RANDOM_INCLUDING_SPOIL 2, BY_TURN 3,
  BY_TURN_INCLUDING_SPOIL 4 (sysString ids 487/488/798/799/800).
- **Invite** (`RequestJoinParty` 0x42: name + distribution type id): guards —
  target online (SM 1 "must first select"), not self (SM 1391), not already
  in a party (SM 160), requestor is leader (SM 154), party not full (SM 155,
  `AltPartyMaxMembers = 9` on this dist), no pending invitation
  (SM 164 WAITING_FOR_ANOTHER_REPLY), target has no pending request (SM 153
  C1_IS_ON_ANOTHER_TASK). Success: SM 105 "C1 has been invited"; target gets
  `AskJoinParty` (0x39: requestor name + distribution type). First invite
  creates the `Party` eagerly with the requestor alone in it; a decline of
  that first invite removes it again.
  `PartyRequest` timeout: 30 s (both sides' request slots cleared).
- **Answer** (`RequestAnswerJoinParty` 0x43: response int): requestor gets
  `JoinParty` (0x3A: response, then a TODO int 0). Accept (1): full-party
  re-guard (SM 155 to both), `player.joinParty` → `Party.addPartyMember`:
  new member gets `PartySmallWindowAll` (0x4E: leader oid, loot type byte,
  count, then per *other* member oid/name/CP/maxCP/HP/maxHP/MP/maxMP/
  vitality(int)/level(byte)/classId(short)/1(byte)/race(short)/summon
  count(int)=0), SM 106 "you joined C1's party"; everyone else gets SM 107
  "C1 joined" + `PartySmallWindowAdd` (0x4F: leader oid, loot type *int*,
  member oid/name/CP/maxCP/HP/maxHP/MP/maxMP/vitality/level/classId/0(byte)/
  race). All members `broadcastUserInfo` (our relation/party bit lives in
  CharInfo; re-broadcast UserInfo+CharInfo), party level recalc, HP status
  update exchange. Decline (0 or -1): requestor gets `JoinParty(0)` (+ for
  -1, SM 3612 refuse-requests); a 1-member party is dissolved
  (`removePartyMember(requestor, NONE)` — no messages for NONE type).
- **Leave/oust/disband** (`RequestWithDrawalParty` 0x44 /
  `RequestOustPartyMember` 0x45 by name, leader only):
  `removePartyMember(type)` — if 2 members left, or leader leaves with
  `AltLeavePartyLeader = False`, disband instead (SM 1372 THE_PARTY_HAS_
  DISPERSED to all, then each member removed with type NONE). **This dist
  has `AltLeavePartyLeader = True`** → leader leaving transfers lead.
  Left/disconnected: SM 200 "you have withdrawn" + SM 108 "C1 has left" to
  the rest; expelled: SM 201 to victim + SM 109 to the rest. Leaver gets
  `PartySmallWindowDeleteAll` (0x50), the rest get `PartySmallWindowDelete`
  (0x51: oid + name). If the leader changed as a result: SM 168 "C1 has
  become the party leader" + `broadcastToPartyMembersNewLeader` (everyone:
  DeleteAll + fresh `PartySmallWindowAll` + UserInfo re-broadcast). Last
  remaining member: window cleanup + party dropped.
- **Change leader** (`RequestChangePartyLeader` D0:0x0C, name): leader only;
  target must be a member (SM 1402/1403 guards; SM 1400 if already leader);
  swap slots 0↔i, SM 168 + `broadcastToPartyMembersNewLeader`.
- **Loot rule change** (`RequestPartyLootModification` D0:0x75 byte,
  `AnswerPartyLootModification` D0:0x76 int): leader requests a type; other
  members get `ExAskModifyPartyLooting` (FE:C0: leader name, type byte…) and
  15 s to all answer yes; any no / timeout cancels. Result:
  `ExSetPartyLooting` (FE:C1: success int, type byte…) + SM 1852/1853
  (changed-to-S1 / cancelled) to all. (Sys-string param = type's sysStringId.)
- **Member status**: Java `Player.broadcastStatusUpdate` sends
  `PartySmallWindowUpdate` (0x52: member oid, flags:short, then
  masked CP/maxCP/HP/maxHP/MP/maxMP/level/classId… fields —
  `PartySmallWindowUpdateType` masks: CURRENT_CP 1, MAX_CP 2, CURRENT_HP 4,
  MAX_HP 8, CURRENT_MP 16, MAX_MP 32, LEVEL 64, CLASS_ID 128; **natural**
  bit order via `writeShort(flags)`, not the reversed byte-array scheme) to
  the other members whenever a vitals `StatusUpdate` goes out; level-ups
  broadcast the all-flags variant.
  `PartyMemberPosition` (0xBA: count, then oid/x/y/z) every 12 s to all.
- **XP/SP split** (`Attackable.calculateRewards` party branch +
  `Party.distributeXpAndSp`): per killed mob, over the *damage-share map*:
  members alive + within `AltPartyRange` (1500) of the corpse join
  `rewardedMembers`; party damage = their summed shares; `partyMul =
  partyDmg/totalDamage`; level-gap XP table applied at *party level* (max
  member level among rewarded); base exp×sp × partyMul → then
  `distributeXpAndSp`: × `BONUS_EXP_SP[n-1]` (1.0, 1.3, 1.35, 1.4, 1.55,
  1.6, 1.7, 1.8, 2.0) × `RatePartyXp/Sp` (**70 on this dist**) for n ≥ 2;
  each member weighted by `level²/Σlevel²`; dead members get nothing
  (they're not in rewardedMembers; Java's extra isDead re-check is for the
  command-channel path). Per-member cutoff: dist runs `PartyXpCutoffMethod =
  highfive` → gap to party top level 0–9 → 100 %, 10–14 → 30 %, 15+ → 0 %
  (`PartyXpCutoffGaps`/`GapPercent`).
- **Loot split** (`Party.distributeItem`/`distributeAdena`): adena → split
  evenly among members in `AltPartyRange` of the corpse. Items →
  FINDERS_KEEPERS: killer keeps; RANDOM: random in-range member;
  BY_TURN: round-robin (`_itemLastLoot` cursor, skipping out-of-range);
  `*_INCLUDING_SPOIL` behave identically until spoil exists. Non-looting
  members get SM 34/33 (C1_OBTAINED_S3_S2 / C1_OBTAINED_S2).
- **Disconnect**: `removePartyMember(DISCONNECTED)` — like LEFT, but the
  leader-leaving case transfers leadership instead of disbanding even with
  `AltLeavePartyLeader = False`.

### Friends
- Table `character_friends (charId, friendId, relation, memo)` — rows in
  *both* directions per friendship; `relation`/`memo` unused here (0/NULL).
- **Enter world**: `L2FriendList` (0x75: count, then per friend charId/
  name/online int/oid-if-online/level/classId/short 0) + SM 1755
  "your friend S1 just logged in" to each online friend + `FriendStatus`
  (0x59: MODE_ONLINE 1, name; MODE_OFFLINE 0 writes name + oid) to them
  (Java `notifyFriends` on online-status flip; offline notify on deleteMe).
- **Invite** (`RequestFriendInvite` 0x77: name): guards — target online &
  found (SM 1385), not self (SM 1354), not already a friend (SM 1756 …
  ALREADY_REGISTERED… wait, SM id: THIS_PLAYER_IS_ALREADY_REGISTERED… =
  479? — use the id table at implementation time), target not busy
  (SM 153). Sends target `FriendAddRequest` (0x83: byte 0 + requestor name),
  requestor gets SM 1756 "You've requested C1 to be on your friends list"
  (id 621? verify in `sm_ids`). Same single-request slot as party
  (`onTransactionRequest`).
- **Answer** (`RequestAnswerFriendInvite` 0x78: pad byte + response int):
  accept → both DB rows inserted, both in-memory lists updated, SMs to both
  (132 THAT_PERSON_HAS_BEEN…ADDED / 479 S1_HAS_BEEN_ADDED_TO_YOUR_FRIENDS…/
  480 counterpart), and each side gets `FriendAddRequestResult` (0x55:
  result, charId, name, online, oid, level, classId, short 0) for the other.
  Decline → requestor SM 133 YOU_HAVE_FAILED_TO_ADD_A_FRIEND.
- **Delete** (`RequestFriendDel` 0x7A: name): name → char id (DB lookup —
  works offline); not-in-list → SM 171 C1_IS_NOT_ON_YOUR_FRIEND_LIST; else
  delete both rows, SM 173 S1_HAS_BEEN_REMOVED…, `FriendRemove` (0x57:
  response 1, name) to self, and to the (online) ex-friend too.
- **List** (`RequestFriendList` 0x79): SM 487 header + per-friend SM 488
  (online)/489 (offline) + SM 490 footer — pure SystemMessage output.
- **Friend message** (`RequestSendFriendMsg` 0x6B: message, receiver name):
  ≤ 300 chars, receiver online & has *sender* in their friend list →
  `L2FriendSay` (0x78: int 0, receiver, sender, message).

## 2. Rust design

### Chat (`game_loop/chat.rs`, new)
- `handle_say2`: parse (text, type, target), port the Say2 guards we can
  represent (unknown type / empty text → just log + ActionFailed, **no
  disconnect** — one botched packet shouldn't kill a session we can't
  re-auth transparently; documented deviation), 105-char cap — with Java's
  item-link branch (500 chars when the text carries a `\x08` link, GMs
  exempt) since item links landed on 2026-08-03, then match:
  - GENERAL → `CreatureSay` to self + in-game players within 1250 units
    (adjacent-region prefilter, then euclidean check — Java's
    `forEachVisibleObjectInRange`).
  - SHOUT / TRADE → all in-game players in the same map-region tile
    (`map_region` loader; `GlobalChat/TradeChat = ON` semantics; the config
    keys are read but only the ON path is implemented — OFF/GM fall back to
    ON with a boot warning).
  - WHISPER → name lookup over in-game players (case-insensitive); SM 3
    THAT_PLAYER_IS_NOT_ONLINE when absent; echo `"->Name"`; whisper
    relation-mask byte 0 + sender level appended per the packet format.
  - PARTY → party broadcast incl. sender; SM 1696 YOU_ARE_NOT_IN_A_PARTY
    (id per `sm_ids`) when partyless.
  - CLAN / ALLIANCE → SM you-are-not-in-a-clan / not-in-alliance.
  - anything else → log-and-drop.
- `server_packets::creature_say` — objectId, type, name, npcstring -1, text,
  whisper tail when applicable.

### Party (`model/party.rs` + `game_loop/party.rs`, new)
- `World.parties: HashMap<u32, Party>` + `World.next_party_id` (`u32`
  counter; Java identity references → an id key. NOT an ECS entity — a party
  is not a world object, and Java keeps it off the object registry too).
  `Party { members: Vec<i32> /* leader first */, distribution: LootRule,
  level: i32, pending_invitation: bool, pending_invite_expiry: u64,
  item_last_loot: usize, loot_change: Option<LootChangeRequest> }`.
- Component `PartyRef(pub u32)` on player entities (presence = in a party).
- Component `PendingRequest { kind: RequestKind, other: i32, expires_tick:
  u64 }` — one slot per player, covering Java's `_requests` map *and*
  `_activeRequester` (party and friend flows can't race each other in our
  port; Java mostly enforces the same via `isProcessingRequest`). Kind:
  `PartyInvite { party_id }` / `FriendInvite`. Cleared by answer, timeout
  (30 s scheduled task carrying a generation seq like casts), or either
  side leaving the world.
- Handlers port §1 flows 1:1 minus: command channels, matching rooms,
  duels, tactical signs, pets/servitors (summon count writes 0),
  cursed weapons/olympiad/jail/events/block-list guards (systems absent).
- `PartySmallWindowUpdate` hook: a `broadcast_vitals_to_party(world,
  object_id, flags)` helper called from the places that already send player
  `StatusUpdate` (regen tick, damage/heal application, MP consume, level
  up). Level-up additionally sends the LEVEL flag; Java's needCp/Hp/MpUpdate
  hysteresis is dropped (we send when we'd send StatusUpdate anyway).
- Position broadcast: one scheduled task per party (12 s, re-scheduling
  itself while the party lives) sending `PartyMemberPosition` built from
  live `Position` components.
- **XP/SP split** (`game_loop/combat.rs` rewards path): the G9 solo-only
  `calculate_rewards` grows the Java party branch — group damage shares by
  `PartyRef`, in-range/alive filter, party level, `partyMul`, then a
  `distribute_xp_and_sp` port (BONUS_EXP_SP × RatePartyXp/Sp, level² weights,
  highfive cutoff from config keys `PartyXpCutoffMethod/Gaps/GapPercent` —
  loaded into `CombatConfig`; "level"/"percentage"/"auto"/"none" methods also
  ported since `getValidMembers` is small).
- **Loot split** (`combat.rs` drop path): killer in party → adena split /
  looter selection per distribution type (RANDOM via `world.roll`, BY_TURN
  via `item_last_loot`), items auto-looted into the looter's inventory
  (AutoLoot=True dist; the SM 34/33 "C1 obtained" messages go to the other
  members), capacity check skipped (no maxLoad enforcement yet — same G5
  simplification).
- Disconnect/logout/restart: `store_and_remove_player` calls
  `party_on_leave(world, oid, Disconnected)` before despawn.

### Friends (`game_loop/friends.rs`, new)
- DB (`db.rs`): per-character load gains `SELECT f.friendId, c.char_name,
  c.level, c.classid, c.online FROM character_friends f JOIN characters c…`
  → `Vec<FriendInfo>` on `PlayerData`; commands `InsertFriendPair(a, b)`,
  `DeleteFriendPair(a, b)` (fire-and-forget, both directions in one
  statement like Java); `RequestFriendDel` by name resolves through the
  loaded snapshot (covers Java's `CharInfoTable` lookup — you can only
  delete someone who's on your list, and the list snapshot has their name).
- Component `Friends(pub Vec<FriendInfo>)` (`FriendInfo { char_id, name,
  level, class_id }` — online status always live from `World`, never the
  stale DB column).
- Enter world: `L2FriendList` from the component (+ live online flags), SM
  "friend logged in" + `FriendStatus(MODE_ONLINE)` to each online friend
  (scan online players' `Friends` — no global friend index needed at our
  scale). Leave world: `FriendStatus(MODE_OFFLINE)` likewise.
- Invite/answer/delete/list/msg handlers per §1, sharing `PendingRequest`.
- On accept, both sides' `Friends` components get the other's live snapshot
  (level/class from components).

### Packet plumbing
- `client_packets.rs`: opcodes SAY2 0x49, REQUEST_JOIN_PARTY 0x42,
  REQUEST_ANSWER_JOIN_PARTY 0x43, REQUEST_WITH_DRAWAL_PARTY 0x44,
  REQUEST_OUST_PARTY_MEMBER 0x45, REQUEST_SEND_FRIEND_MSG 0x6B,
  REQUEST_FRIEND_INVITE 0x77, REQUEST_ANSWER_FRIEND_INVITE 0x78,
  REQUEST_FRIEND_LIST 0x79, REQUEST_FRIEND_DEL 0x7A; ex-opcodes
  REQUEST_CHANGE_PARTY_LEADER 0x0C, REQUEST_PARTY_LOOT_MODIFICATION 0x75,
  ANSWER_PARTY_LOOT_MODIFICATION 0x76.
- `server_packets.rs`: CreatureSay 0x4A, AskJoinParty 0x39, JoinParty 0x3A,
  PartySmallWindowAll 0x4E, Add 0x4F, DeleteAll 0x50, Delete 0x51,
  Update 0x52, PartyMemberPosition 0xBA, ExAskModifyPartyLooting FE:C0,
  ExSetPartyLooting FE:C1, FriendAddRequestResult 0x55, FriendRemove 0x57,
  FriendStatus 0x59, L2FriendList 0x75, L2FriendSay 0x78,
  FriendAddRequest 0x83.
- `CharInfo`/`UserInfo` relation fields: CharInfo re-broadcast on party
  join/leave/leader change (Java `broadcastUserInfo`); actual
  relation-bit computation (`RelationChanged`) is deferred with PvP/clans.

### Config additions
- `Character.ini`: `AltPartyMaxMembers` (9), `AltLeavePartyLeader` (True),
  `PartyXpCutoffMethod/Gaps/GapPercent/Level/Percent`.
- `Rates.ini`: `RatePartyXp` (70!), `RatePartySp` (70).
- `General.ini`: `GlobalChat` (ON), `TradeChat` (ON).
  (`AltPartyRange` already loaded since G9.)

## 3. Deviations from Java (deliberate)

- Malformed `Say2` (bad type / empty text) logs and drops instead of
  force-disconnecting.
- One `PendingRequest` slot serves both party and friend invites (Java has
  a typed request map + a legacy `_activeRequester` slot; behavior under
  concurrent invites is the same "C1 is on another task" answer).
- No hysteresis on party vitals updates — `PartySmallWindowUpdate` piggybacks
  on every party member `StatusUpdate` send.
- Friend name→id resolution for delete comes from the loaded friend
  snapshot instead of a global name cache.
- Spoil-including loot rules behave like their plain variants (no spoil).
- Whisper relation mask: bit 0x01 (friend) computed, other bits 0 (no
  clan/ally/mentor systems).

## 4. Out of scope (deferred)

Clans (needs bypass/village masters — G11), alliances, mail, community
board, party matching rooms & waiting list, command channels (MPCC),
tactical signs, block list (`BlockList` — all is-blocked checks skipped),
friend memos (`RequestUpdateFriendMemo`/`RequestBlockMemo` — consumed,
no-op), `RequestExFriendListExtended`, party substitute, pets/summons in
party windows, hero/petition/announce chat types, voiced commands, say
filter, chat bans, `RelationChanged` packets. (Item links in chat were out
of scope here and landed later — 2026-08-03, see PROGRESS.md.)

## 5. Tests

- **Formulas**: `distribute_xp_and_sp` against hand-computed Java values
  (2- and 9-member parties, mixed levels, highfive cutoff at gaps 9/10/15,
  BONUS_EXP_SP ladder, level² weighting, dead/out-of-range exclusion,
  partyMul when an outsider did damage).
- **Synthetic-world** (`game_loop::tests`): general chat range scoping
  (1250 in/out), whisper online/offline + echo, party chat member-only;
  invite→accept full packet sequence both sides (window packets byte-shape),
  decline dissolving a 1-member party, invite guards (busy target, full
  party, non-leader), leave/oust/leader-change/disband rules incl. the
  2-member auto-disband and `AltLeavePartyLeader=True` transfer,
  disconnect leadership transfer, loot-rule change round trip (accept +
  timeout), party kill → split XP with exact values + BY_TURN loot
  rotation + adena split, vitals piggyback (damage → other member sees
  `PartySmallWindowUpdate`), friend invite→accept→both lists + DB commands,
  delete both ways, friend-message delivery, enter/leave world friend
  notifications.
- **Persistence** (`char_persistence.rs`): friendship rows round-trip
  (insert pair, reload character, delete pair).
- **E2E**: `e2e_create.rs` skip-helper tolerates the new enter-world
  packets (`L2FriendList`).

## 6. Risks / open points

- Packet layouts above are transcribed from the Interlude-Classic branch
  sources; no client capture yet for the party window packets — byte tests
  are hand-computed like `NpcInfo`'s (mask math for
  `PartySmallWindowUpdate` is a plain short, not the reversed-array
  scheme — do NOT reuse `masks.rs`).
- `JoinParty`'s trailing int is a Java TODO ("Find me!") written as 0 —
  keep 0.
- SystemMessage ids cited by name in §1 must be pulled from
  `SystemMessageId.java` at implementation time and added to `sm_ids`
  (the numbers in this doc are from memory where noted and must be
  verified).
- Party position task and 12 s cadence: use the existing scheduler; make
  sure the task dies with the party (generation counter like `cast_seq`).
