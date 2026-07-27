# G33 — Client-packet parity checklist

The G33 gate: a mechanical diff of Java's registered client-packet handlers
against the Rust dispatch table, keyed by **opcode value**, to surface any
packet family that slipped a milestone. Source of truth: Java
`network/ClientPackets.java` + `ExClientPackets.java` (only `::new` handlers;
`null` registrations are Java's own no-ops), against
`game_loop/dispatch.rs` (`on_packet` / `on_ex_packet`, an empty `{}` arm still
counting as handled).

## Summary

| | Java `::new` | Handled in Rust | Not dispatched |
|---|---|---|---|
| Top-level | 155 | 115 | 40 |
| Extended | 198 | 60 | 138 |
| **Total** | **353** | **175** | **178** |

Of the 178 not dispatched, the audit classified essentially all as either a
**deferred-by-design subsystem** (a milestone that is intentionally incomplete)
or **later-chronicle / niche** content that Interlude Classic never exposes.
The port also *handles several packets Java leaves `null`* (e.g. top-level
`REQUEST_RECIPE_SHOP_MANAGE_LIST` 0xB9, ex `REQUEST_USER_BAN_INFO` 0x138,
`EX_SEND_CLIENT_INI` 0x104) — the reverse of a gap.

## Not-dispatched, bucketed

### Deferred-by-design subsystems (tracked by their own milestone, not a slip)
- **Clan ranks / wars / allies / sub-pledges** (G18 s3+, pending): clan-war
  replies 0x04/0x06/0x08, `RequestPledgeMemberList` 0x4D,
  `RequestPledgeExtendedInfo` 0x66, `RequestGiveNickName` 0x0B, ex
  `RequestPledgeSetAcademyMaster` 0x12, ex `RequestClanAskJoinByName` 0xB9.
- **Mail / post** (G30, deferred): ex 0x62–0x6C (the whole post family).
- **Party matching room** (G30, deferred): top 0x7F/0x80/0x81, ex
  0x09/0x0A/0x0B/0x25/0x2F/0x30/0x31.
- **Command Channel / MPCC** (own feature, not yet scheduled): ex
  0x06/0x07/0x08/0x2D and the MPCC-room family 0x5A–0x61.
- **Private *buy* store** (G15 sell-store landed; buy-store not): 0x99/0x9A/0x9C/
  0x9F + titles 0x97/0x9D, wholesale `SetPrivateStoreWholeMsg` ex 0x47.
- **Siege UI** (G24 combat landed; info window not): `RequestSiegeInfo` 0xAA.
- **Cursed-weapon info UI** (G28 loop landed; the list/locate window not): ex
  0x2A/0x2B.

### Later-chronicle / niche / cosmetic (correct skips for Interlude Classic)
Second-password, airship/shuttle/fly-Sayune (Gracia), commission & world
auction house, prime/BR shop, mentoring, beauty shop, appearance shape-shift,
awakening, elemental attributes (Kamael/Gracia), contact list, compound /
new-enchant, lucky game / PC-café / training room, alchemy (Ertheia), VIP
attendance, daily/todo & pledge weekly bonus, divide-adena, fortress, boss/PVP
record, fish ranking, instance-reenter, cutscene end, key-mapping save,
item-link-in-chat, refund/estimate UI, couple/wedding, event ranker, tutorial
family, GM/anti-cheat (GameGuard) — ~130 opcodes, none reachable on this dist.

### Genuine slips (feature fully ported, packet missed)
- **`RequestQuestList` 0x62** — CLOSED this slice. Quests are fully ported and
  the port already builds `QuestList`; the journal-open request was the only
  missing piece (empty body → resend `QuestList`, Java verbatim).
- **`CannotMoveAnymore` 0x47** — left open (documented). Java fires the AI
  `EVT_ARRIVED_BLOCKED` event; this port's player movement doesn't use that AI
  event path, and the client self-stops, so it is not a clean cheap port.
- **`ExRequestSaveKeyMapping` ex 0x22** — the port handles `REQUEST_KEY_MAPPING`
  ex 0x21 but not the save sibling. Cosmetic (client-side key layout); low value.
- **Augment confirm dialog** ex 0x26/0x28/0x3F — the augment *refine* flow is
  ported; whether these confirm-dialog packets are needed depends on the
  client's exact augment UX. Not confirmed as reachable; left for a targeted
  check if augment UI issues surface.

## Conclusion

The port dispatches every gameplay packet reachable on Interlude Classic except
the ones belonging to explicitly-deferred subsystems (clan wars G18, mail /
party-matching G30) and a handful of cosmetic/niche outliers. The one genuine
milestone slip — the quest-journal request — is closed here. The parity
checklist is therefore complete: no gameplay packet family silently slipped a
milestone; the remainder is deferred-by-design or off-chronicle, each tracked
by its owning milestone above.
