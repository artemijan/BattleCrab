# PLAN — G18: Clans (full)

Everything past G11's creation/display slice and G15's clan warehouse
(ROADMAP.md §G18). Java sources: `model/clan/Clan.java` (3054 lines),
`data/sql/ClanTable.java`, the `RequestPledge*`/`Request*Pledge*` client
packets, `VillageMaster`'s clan verbs, `instancemanager/ClanEntryManager`.

**Gate (ROADMAP):** form a clan, invite members, level it, learn a clan skill,
declare war, form an ally. **Unblocks:** `//clan_*`, `//pledge`,
`//add_clan_skill` (the `//give_clan_skills` grant already exists; real
learning is rep-gated).

## What already exists (pre-G18)

- `game_loop/clans.rs`: `create_clan` (village-master bypass, full guard
  chain), `destroy_clan` (admin), `set_clan_level`/`add_clan_reputation`
  (admin), clan skills (grant + login re-apply + social-class gate), Clan
  Advent aura, pledge windows (`PledgeInfo`, `PledgeShowMemberListAll/Update`,
  `PledgeSkillList`), enter/leave-world roster sync, clan chat.
- `model/clan.rs`: `Clan { id, name, leader_id, level, reputation_score,
  castle_id, members, skills, warehouse }`, `pledge_class_of`, privilege
  bits (`ALL_CLAN_PRIVILEGES`, `CL_VIEW_WAREHOUSE`, `has_privilege`).
- Clan warehouse (G15), clan-window recruit-query empty answers, siege
  registration rows (G24 prep), `PendingRequest`/`RequestKind` transaction
  slot (party/friend invites) in `game_loop/party.rs`.

## Slices

### Slice 1 — Membership lifecycle (this branch)

The four client packets 0x26–0x29 + village-master dissolve/recover:

- **Invite**: `RequestJoinPledge` (0x26) → `Clan.checkClanJoinCondition`
  guard chain (CL_JOIN_CLAN bit 1, wrong target SM 152, self SM 4, clan
  oust-penalty SM 231, target already clanned SM 10, target join-penalty
  SM 760, academy-eligibility SMs 1735/1734, per-pledge-type member cap
  SM 1835/233) → `AskJoinPledge` (0x2C) via the `PendingRequest` slot.
- **Answer**: `RequestAnswerJoinPledge` (0x27): decline → SM 225 to target,
  SM 224 to inviter. Accept → re-check join condition, `JoinPledge` (0x2D),
  `Clan.addClanMember`: roster insert, privs from rank (new member power
  grade 5 → rank privs 0 until slice 3's rank table), SM 195 to enterer,
  SM 222 broadcast, `PledgeShowMemberListAdd` (0x5C) to others,
  `PledgeShowInfoUpdate` + `ExPledgeCount` (0xFE:0x13D) broadcast,
  `PledgeShowMemberListAll` to the enterer, join-expiry reset, UserInfo.
- **Leave**: `RequestWithdrawalPledge` (0x28): not-a-member SM 212, leader
  reject SM 239, combat reject SM 1116 (`has_attack_stance`), then
  `removeClanMember` with join-penalty `now + DaysBeforeJoinAClan(1) days`;
  SM 223 broadcast, `PledgeShowMemberListDelete` (0x5D) + `ExPledgeCount`
  broadcast, SM 197 + SM 232 to the leaver.
- **Oust**: `RequestOustPledgeMember` (0x29): CL_DISMISS (bit 6), self
  reject SM 269, target-in-combat SM 1117; `removeClanMember` with the same
  join penalty **plus** the clan-side `char_penalty_expiry_time` (SM 231
  gate on the next invite); SM 191 broadcast, SM 309 + SM 231 to the
  ouster, SM 199 to the (online) target.
- **`removeClanMember`** (shared): strip title (non-noble), Clan Advent +
  clan skills, clan-leave for the online player (clan fields zeroed, join
  penalty unless academy, pledge class recalc, UserInfo,
  `PledgeShowMemberListDeleteAll`); offline removal writes the penalty
  through the DB reset. Apprentice/sponsor/sub-pledge-leader cleanup is
  slice 6 (TODO(G18.6) at the site); castle circlets are G24.
- **Dissolve/recover** (village-master `dissolve_clan`/`recover_clan`):
  leader-only SM 794, ally gate SM 554 (TODO until slice 5 — no allies yet,
  gate is a no-op), war gate SM 264 (slice 4 no-op), castle/CH gate SM 266,
  siege-registered/siege-zone gate SM 265, double-request SM 263; stamps
  `dissolving_expiry_time = now + DaysToPassToDissolveAClan(7) days`,
  applies the leader's full death-XP penalty, schedules the removal
  (`ScheduledTask::ClanDissolve`, re-armed at boot from the persisted
  stamp — past-due dissolves fire immediately, Java `ClanTable` ctor).
  Recover zeroes the stamp.
- **Persistence**: `characters.clan_join_expiry_time` (load + write),
  `clan_data.char_penalty_expiry_time`/`dissolving_expiry_time` (load +
  write). `UpdateCharClan` covers the clan-leave column reset.

### Slice 2 — Clan level-up + rep-gated clan-skill learning
Village-master `increase_clan_level` (SP/item/rep costs per level from
`Clan.levelUpClan`), `PledgeStatusChanged`; `RequestAcquireSkill`'s
`SUBPLEDGE`/pledge branch spending reputation (the tree data is loaded).

### Slice 3 — Ranks & power grades
`RequestPledgePower`/`RequestPledgeSetMemberPowerGrade`/
`RequestPledgePowerGradeList`/`RequestPledgeMemberInfo`/
`RequestPledgeMemberPowerInfo`/`RequestPledgeReorganizeMember`,
`clan_privs` rank table (`rank_privs`), leader transfer (village master).

### Slice 4 — Clan wars
`RequestStartPledgeWar`/`RequestStopPledgeWar`/`RequestSurrenderPledgeWar`
(+ reply variants), `RequestPledgeWarList`, `clan_wars` table, war kill
rep/PK rules, `PledgeReceiveWarList`, dissolve/at-war interlocks.

### Slice 5 — Alliances
Ally create/dissolve/join/leave/oust + crest, `checkAllyJoinCondition`
(ported above as the guard spec), ally penalty types 1–4, `AllianceInfo`,
the `AllianceMaster` script verbs (G22 dialog shell already talks).

### Slice 6 — Sub-pledges & academy
Royal guards / orders of knights / academy (`subpledges` table), academy
graduation at 2nd class change, apprentice/sponsor links, `pledge_class`
widening, sub-unit member caps (the `getMaxNrOfMembers` table is already
ported in slice 1 for types −1/100/200/1001/1002/2001/2002).

### Slice 7 — Crests & notices
`RequestSetPledgeCrest`/`RequestPledgeCrest`/large/ally variants,
`crests` storage + id plumbing in CharInfo/UserInfo/PledgeInfo, clan
notice (`clan_notices`, login popup).

### Slice 8 — Recruitment registry & Classic pledge bonus
`ClanEntryManager` behind the already-answered ex 0xD3/0xD4/0xD8/0xDC/0xDE
queries, waiting/draft lists, `ClanRewardData` + `pledgebonus` packets.

## Notes

- Member caps by pledge type/clan level (`getMaxNrOfMembers`): main 0 →
  10/15/20/30/40 (lv 0/1/2/3/4+); academy −1 → 20; royal 100/200 → 20
  (30 at lv 11); knights 1001/1002/2001/2002 → 10 (25 at lv 9+).
- Academy invites (pledge type −1) are read and their eligibility SMs
  ported in slice 1, but the accept path refuses non-zero pledge types
  until slice 6 (no sub-units to put the member in) — TODO(G18.6).
- `ClanPrivilege` ordinal = bit index: CL_JOIN_CLAN=1, CL_GIVE_TITLE=2,
  CL_VIEW_WAREHOUSE=3, CL_MANAGE_RANKS=4, CL_PLEDGE_WAR=5, CL_DISMISS=6,
  CL_REGISTER_CREST=7, CL_APPRENTICE=8, …
- Config (Character.ini, all defaults): `DaysBeforeJoinAClan=1`,
  `DaysBeforeCreateAClan=10`, `DaysToPassToDissolveAClan=7`.
