# G30 — Mail & Party Matching (the milestone's missing half)

G30's community-board half landed earlier (home/buffer/gatekeeper/premium/scheme
buffer). This plan covers the two fully-absent systems the
[2026-07-27 remaining-ports audit](PROGRESS.md) ranked #1 and #2: **mail/post**
(ex `0x62`–`0x6C`) and **party matching rooms** (top `0x7F`/`0x80`/`0x81` +
ex `0x09`/`0x0A`/`0x0B`/`0x25`/`0x2F`/`0x30`/`0x31`).

Java sources: `instancemanager/MailManager`, `model/Message`,
`model/itemcontainer/Mail`, `taskmanager/MessageDeletionTaskManager`;
`model/matching/{MatchingRoom,PartyMatchingRoom}`,
`instancemanager/MatchingRoomManager`.

## Gates

- **Party matching:** a player creates a room, a second player finds it in the
  room list, joins it, and the leader can kick/disband — with both sides'
  windows correct.
- **Mail:** player A mails player B with an attached item and a COD price; B
  reads it, pays, receives the item, and A receives the adena.

## Scope gate (Interlude Classic)

Ported: the party half of the matching system, `MatchingMemberType` 0–2, and the
Interlude packet set listed above.

**Excluded as post-Interlude** (confirmed by SM ids 2994–3003 and the Java
`// Helios` comments):
- The whole `CommandChannelMatchingRoom` + ex `0x5A`–`0x61` MPCC-room family and
  the CC branch of `RequestPartyMatchConfig`. (Command channels themselves are a
  separate unscheduled feature; see PROGRESS's audit row #3.)
- `ExRequestPartyMatchingHistory` (ex `0x11C`) and the `party_matching_history`
  table — Helios-era, and its count field is hardcoded 100 in Java.
- The instance-time sub-blocks inside `ExPartyRoomMember` /
  `ExListPartyMatchingWaitingRoom`, and the trailing
  `World.getPartyCount()`/`getPartyMemberCount()` ints of `ListPartyWaiting`
  (marked `// Helios` in Java) — sending these desyncs an Interlude client.
- Mail: the `COMMISSION_ITEM_SOLD` / `COMMISSION_ITEM_RETURNED` mail types and
  their conditional packet blocks (commission house is Kamael-era and out of
  scope per ROADMAP), `PRIME_SHOP_GIFT`, `MENTOR_NPC`, and `CustomMailManager`
  (a Mobius `Custom/*` feature). `MailType::{Regular,NewsInformer,Npc,Birthday}`
  are kept so the wire ordinal matches.

## Java bugs deliberately NOT reproduced

Each is a real defect found while reading the source; the port fixes it and
pins the fix with a test.

1. `MatchingRoom.deleteMember` — the constructor puts the leader in `_members`,
   so `getMembers().isEmpty()` is never true when the leader leaves: a solo room
   leaks forever in `MatchingRoomManager` and keeps showing in `ListPartyWaiting`.
   The port removes the member first, deletes the room when empty, and keeps the
   promoted leader in the member set (Java drops him).
2. `MatchingRoomManager.getPartyMathchingRooms(location, MY_LEVEL_RANGE, lvl)`
   — the filter is inverted (`min >= lvl && max <= lvl`, i.e. only `min==max==lvl`).
   The port uses `min <= lvl && max >= lvl`, matching the sibling CC/by-id
   lookups that are already correct.
3. `RequestOustFromPartyRoom` — `memberParty = player.getParty()` should be
   `member.getParty()`, so the "can't force-kick your own party member" rule
   never fires correctly.
4. `PartyMatchingRoom.notifyRemovedMember` — builds `ExPartyRoomMember` from the
   *removed* player, so every recipient gets the leaver's member type; and sends
   SM 1397 "leader has changed" unconditionally, ignoring `leaderChanged`.
5. `RequestPartyMatchList` NPEs when `roomId > 0` and the player has no room;
   `RequestAskJoinPartyRoom` NPEs when the inviter has no room;
   `AnswerJoinPartyRoom` leaves `activeRequester` set on one early-return path.
6. `RequestListPartyMatchingWaitingRoom` desyncs its read when the class-filter
   count is `>= 128` (the ints are consumed only inside the guarded branch).

## Slices

### Slice 1 — Matching-room core: create, list, waiting list ✅
`model/matching_room.rs` (`MatchingRoom` + `MatchingRoomManager` on `World`),
`game_loop/party_room.rs`, the `bbs` map-region field (the room "location"),
and packets: `RequestPartyMatchConfig` 0x7F (implicit LFP registration),
`RequestPartyMatchList` 0x80 (create + edit), `ExitPartyMatchingWaitingRoom`
ex 0x25, `RequestListPartyMatchingWaitingRoom` ex 0x31 →
`ListPartyWaiting` 0x9C, `PartyRoomInfo` 0x9D, `ExPartyRoomMember` 0xFE 0x08,
`ExListPartyMatchingWaitingRoom` 0xFE 0x36.

### Slice 2 — Room membership: join, leave, kick, disband, invite ✅
`RequestPartyMatchDetail` 0x81 (join), ex 0x09 oust, ex 0x0A dismiss,
ex 0x0B withdraw, ex 0x2F ask-join + ex 0x30 answer (via a new
`RequestKind::PartyRoomInvite`), `ExClosePartyRoom` 0xFE 0x09,
`ExAskJoinPartyRoom` 0xFE 0x35; the `UserInfo` CLAN-block `isInMatchingRoom`
byte; the logout hook in `store_and_remove_player`; the party-withdraw and
party-invite-accept cross-hooks; `ChatType::PartyMatchRoom` (14).

### Slice 3 — Mail foundation + inbox/outbox listing
`model/mail.rs` (`Message`, `MailType`), the `messages` table boot load
(`DbEvent::MailLoaded`) + `DbCommand`s, a `CharInfoTable` equivalent
(boot-loaded name→id map on `World`, needed because mail addresses *offline*
characters), packets `RequestPostItemList` ex 0x62, `RequestReceivedPostList`
ex 0x64, `RequestSentPostList` ex 0x69 → `ExShowReceivedPostList` 0xFE 0xAB,
`ExShowSentPostList` 0xFE 0xAD, `ExReplyPostItemList` 0xFE 0xB3,
`ExUnReadMailCount` 0xFE 0x13C, and `ExNoticePostArrived` 0xFE 0xAA at enter-world.

### Slice 4 — Send, read, delete
`RequestSendPost` ex 0x63 (the full 25-step guard chain + the
`100 + 1000/slot` fee), `RequestReceivedPost` ex 0x66,
`RequestSentPost` ex 0x6B, `RequestDeleteReceivedPost` ex 0x65,
`RequestDeleteSentPost` ex 0x6A → `ExReplyReceivedPost` 0xFE 0xAC,
`ExReplySentPost` 0xFE 0xAE, `ExChangePostState` 0xFE 0xB4,
`ExNoticePostSent` 0xFE 0xB5 (note: writes the `EX_REPLY_WRITE_POST` id).

### Slice 5 — Attachments, COD, expiry
The `MAIL` item location + a world-level attachment container,
`RequestPostAttachment` ex 0x67 (receive + COD payment, incl. the offline-sender
adena row), `RequestCancelPostAttachment` ex 0x6C,
`RequestRejectPostAttachment` ex 0x68 (the return-to-sender message),
and `ScheduledTask::MailExpire` (Java `MessageDeletionTaskManager`: 15-day
regular / 12-hour COD expiry, returning attachments to the sender's warehouse).

## Notes carried from the Java read

- `messageId` comes from the **global object-id pool** in Java (`IdManager`) and
  is released on delete — use `World::alloc_object_id`.
- Booleans persist as the strings `'true'`/`'false'` in `messages`; the table
  already ships in `dist/db_installer/sql/*/game/messages.sql`. `isLocked` is
  vestigial (derive from `reqAdena > 0`).
- Batch delete aborts the whole batch on the first bad id (only a missing
  message is skipped) — preserve that.
- The send fee is charged before the transfer and is never refunded on
  cancel/reject/expire.
- Java's `hasUnreadPost()` ignores `isDeletedByReceiver` while `getUnreadCount()`
  respects it; the port uses the consistent (inbox-based) definition.
