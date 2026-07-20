//! Clan / pledge packets (G11).

use commons::network::PacketWriter;

use super::opcodes;

/// Port of `serverpackets/PledgeShowInfoUpdate` — the clan-info refresh
/// sent to the new leader on creation. Castle/hideout/fort/rank/reputation/
/// ally/war all zero (their systems are later milestones).
pub fn pledge_show_info_update(clan: &crate::model::clan::Clan) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PLEDGE_SHOW_INFO_UPDATE);
    w.write_i32(clan.id);
    w.write_i32(1); // Config.SERVER_ID
    w.write_i32(0); // crest id
    w.write_i32(clan.level);
    w.write_i32(0); // castle id
    w.write_i32(0); // castle state
    w.write_i32(0); // hideout id
    w.write_i32(0); // fort id
    w.write_i32(0); // rank
    w.write_i32(clan.reputation_score); // reputation score
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0); // ally id
    w.write_string(""); // ally name
    w.write_i32(0); // ally crest id
    w.write_i32(0); // at war
    w.write_i32(0);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/PledgeInfo` — the compact clan name/ally answer
/// to `RequestPledgeInfo`, sent when the client needs a clan's display names
/// (e.g. rendering another player's pledge). Ally name is empty until the
/// alliance system lands (a later milestone).
pub fn pledge_info(clan: &crate::model::clan::Clan) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PLEDGE_INFO);
    w.write_i32(1); // Config.SERVER_ID
    w.write_i32(clan.id);
    w.write_string(&clan.name);
    w.write_string(""); // ally name
    w.into_bytes()
}

/// Port of `serverpackets/PledgeShowMemberListAll` for the main pledge
/// (`_pledgeId` 0): the full roster with per-member online status resolved
/// live against the world registry.
pub fn pledge_show_member_list_all(
    clan: &crate::model::clan::Clan,
    objects: &crate::store::EntityStore,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PLEDGE_SHOW_MEMBER_LIST_ALL);
    w.write_i32(1); // !isSubPledge
    w.write_i32(clan.id);
    w.write_i32(1); // Config.SERVER_ID
    w.write_i32(0); // pledge id (main)
    w.write_string(&clan.name);
    w.write_string(clan.leader_name());
    w.write_i32(0); // crest id
    w.write_i32(clan.level);
    w.write_i32(0); // castle id
    w.write_i32(0);
    w.write_i32(0); // hideout id
    w.write_i32(0); // fort id
    w.write_i32(0); // rank
    w.write_i32(clan.reputation_score); // reputation score
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0); // ally id
    w.write_string(""); // ally name
    w.write_i32(0); // ally crest id
    w.write_i32(0); // at war
    w.write_i32(0); // territory castle id
    w.write_i32(clan.members.len() as i32);
    for m in &clan.members {
        let online = objects.has_component::<crate::model::Player>(&m.char_id);
        w.write_string(&m.name);
        w.write_i32(m.level);
        w.write_i32(m.class_id);
        w.write_i32(m.sex);
        w.write_i32(m.race);
        w.write_i32(if online { m.char_id } else { 0 });
        w.write_i32(0); // has sponsor
        w.write_u8(online as u8);
    }
    w.into_bytes()
}

/// Port of `serverpackets/PledgeShowMemberListUpdate` — one member's
/// online-status/level refresh, pushed to the rest of the clan.
pub fn pledge_show_member_list_update(m: &crate::model::clan::ClanMember, online: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PLEDGE_SHOW_MEMBER_LIST_UPDATE);
    w.write_string(&m.name);
    w.write_i32(m.level);
    w.write_i32(m.class_id);
    w.write_i32(m.sex);
    w.write_i32(m.race);
    if online {
        w.write_i32(m.char_id);
        w.write_i32(0); // pledge type (main)
    } else {
        w.write_i32(0);
        w.write_i32(0);
    }
    w.write_i32(0); // has sponsor
    w.write_u8(online as u8);
    w.into_bytes()
}

/// Port of `serverpackets/AskJoinPledge` — the clan-invite confirm dialog on
/// the invited player's screen. Java appends the pledge-type int only when
/// non-zero (the academy/sub-unit variant of the dialog).
pub fn ask_join_pledge(requestor_oid: i32, requestor_name: &str, pledge_type: i32, pledge_name: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::ASK_JOIN_PLEDGE);
    w.write_i32(requestor_oid);
    w.write_string(requestor_name);
    w.write_string(pledge_name);
    if pledge_type != 0 {
        w.write_i32(pledge_type);
    }
    w.into_bytes()
}

/// Port of `serverpackets/JoinPledge` — tells the accepting client which clan
/// it just entered (sent before the roster/info burst).
pub fn join_pledge(clan_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::JOIN_PLEDGE);
    w.write_i32(clan_id);
    w.into_bytes()
}

/// Port of `serverpackets/PledgeShowMemberListAdd` — one new member appended
/// to the other members' open clan windows.
pub fn pledge_show_member_list_add(m: &crate::model::clan::ClanMember) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PLEDGE_SHOW_MEMBER_LIST_ADD);
    w.write_string(&m.name);
    w.write_i32(m.level);
    w.write_i32(m.class_id);
    w.write_i32(0);
    w.write_i32(1);
    w.write_i32(1); // 1 = online (the member just accepted, so they are)
    w.write_i32(0); // pledge type (main)
    w.into_bytes()
}

/// Port of `serverpackets/PledgeShowMemberListDelete` — one member removed
/// from the online members' clan windows (leave/oust).
pub fn pledge_show_member_list_delete(name: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PLEDGE_SHOW_MEMBER_LIST_DELETE);
    w.write_string(name);
    w.into_bytes()
}

/// Port of `serverpackets/ExPledgeCount` — the member-count refresh the
/// Classic client shows in the clan window header.
pub fn ex_pledge_count(count: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_PLEDGE_COUNT);
    w.write_i32(count);
    w.into_bytes()
}

/// Port of `serverpackets/PledgeShowMemberListDeleteAll` — the opcode-only
/// packet that tells the client to close/clear its clan window. Sent to each
/// member when their clan is dissolved (`//pledge dismiss`).
pub fn pledge_show_member_list_delete_all() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PLEDGE_SHOW_MEMBER_LIST_DELETE_ALL);
    w.into_bytes()
}

/// Port of `serverpackets/PledgeSkillList` — the clan window's skill tab. The
/// port folds sub-unit (squad) skills into the main clan skill set (no
/// sub-pledges modelled), so the squad-skill section is always empty.
pub fn pledge_skill_list(skills: &[(i32, i32)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_PLEDGE_SKILL_LIST);
    w.write_i32(skills.len() as i32);
    w.write_i32(0); // squad-skill count (sub-pledges unmodelled)
    for &(id, level) in skills {
        w.write_i32(id);
        w.write_i16(level as i16);
        w.write_i16(0); // sub level
    }
    w.into_bytes()
}

/// Port of `serverpackets/PledgeSkillListAdd` — one clan skill just learned.
pub fn pledge_skill_list_add(skill_id: i32, level: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_PLEDGE_SKILL_LIST_ADD);
    w.write_i32(skill_id);
    w.write_i32(level);
    w.into_bytes()
}

/// Port of `serverpackets/GMViewPledgeInfo` — the GM `//pledge info` clan dump.
/// `viewer_name` is Java's `_player.getName()` (the inspected clan member the GM
/// targeted). Castle/hideout/fort/rank/ally/war stay zero (their systems are
/// later milestones); reputation and level are live.
pub fn gm_view_pledge_info(
    clan: &crate::model::clan::Clan,
    viewer_name: &str,
    objects: &crate::store::EntityStore,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::GM_VIEW_PLEDGE_INFO);
    w.write_i32(0);
    w.write_string(viewer_name);
    w.write_i32(clan.id);
    w.write_i32(0);
    w.write_string(&clan.name);
    w.write_string(clan.leader_name());
    w.write_i32(0); // crest id
    w.write_i32(clan.level);
    w.write_i32(0); // castle id
    w.write_i32(0); // hideout id
    w.write_i32(0); // fort id
    w.write_i32(0); // rank (reputation-derived; RankManager unported)
    w.write_i32(clan.reputation_score);
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0); // ally id
    w.write_string(""); // ally name
    w.write_i32(0); // ally crest id
    w.write_i32(0); // at war
    w.write_i32(0); // T3 unknown
    w.write_i32(clan.members.len() as i32);
    for m in &clan.members {
        let online = objects.has_component::<crate::model::Player>(&m.char_id);
        w.write_string(&m.name);
        w.write_i32(m.level);
        w.write_i32(m.class_id);
        w.write_i32(m.sex);
        w.write_i32(m.race);
        w.write_i32(if online { m.char_id } else { 0 });
        w.write_i32(0); // has sponsor
    }
    w.into_bytes()
}

// ---------------------------------------------------------------------------
// Clan entry (recruitment) — the query slice; the full ClanEntryManager
// board/waiting-list system is G18.
// ---------------------------------------------------------------------------

fn ex(sub: i16) -> PacketWriter {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(sub);
    w
}

/// Port of `serverpackets/ExPledgeRecruitApplyInfo` (0xFE:0x140) — the
/// player's clan-entry state, driving which recruitment buttons the clan
/// window shows. The int is Java's `ClanEntryStatus` ordinal: 0 DEFAULT,
/// 1 WAITING (clanless, application pending), 2 ORDERED (leader of a
/// recruiting clan).
pub fn ex_pledge_recruit_apply_info(status: i32) -> Vec<u8> {
    let mut w = ex(0x140);
    w.write_i32(status);
    w.into_bytes()
}

/// Port of `serverpackets/ExPledgeRecruitBoardSearch` (0xFE:0x141) — one page
/// of the clan recruitment board. Java pages a `PledgeRecruitInfo` list from
/// `ClanEntryManager` 12 clans at a time; the registry is unported
/// (TODO(G18): `ClanEntryManager` + the board entry lists), so the board is
/// always empty — `currentPage` echoed, 0 total pages, 0 clans on the page,
/// no entries: exactly Java's answer on an empty registry.
pub fn ex_pledge_recruit_board_search_empty(current_page: i32) -> Vec<u8> {
    let mut w = ex(0x141);
    w.write_i32(current_page);
    w.write_i32(0); // total pages: ceil(0 / 12)
    w.write_i32(0); // clans on this page
    w.into_bytes()
}

/// Port of `serverpackets/ExPledgeRecruitInfo` (0xFE:0x13F) — a clan's
/// summary for the recruitment UI. Java appends the sub-pledge list
/// (`getAllSubPledges`); the port has no sub-units yet (G18), so the count
/// is 0 and no entries follow.
pub fn ex_pledge_recruit_info(clan: &crate::model::clan::Clan) -> Vec<u8> {
    let mut w = ex(0x13F);
    w.write_string(&clan.name);
    w.write_string(clan.leader_name());
    w.write_i32(clan.level);
    w.write_i32(clan.members.len() as i32);
    w.write_i32(0); // sub-pledge count
    w.into_bytes()
}

/// Port of `serverpackets/ManagePledgePower` — the rank-privilege editor's
/// answer: the (possibly just-updated) privilege mask of one rank.
pub fn manage_pledge_power(rank: i32, action: i32, privs: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MANAGE_PLEDGE_POWER);
    w.write_i32(rank);
    w.write_i32(action);
    w.write_i32(privs);
    w.into_bytes()
}

/// Port of `serverpackets/PledgePowerGradeList` — the rank list (rank id +
/// `party`, the latter always 0 as in Java's `RankPrivs`).
pub fn pledge_power_grade_list(ranks: &[i32]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_PLEDGE_POWER_GRADE_LIST);
    w.write_i32(ranks.len() as i32);
    for &rank in ranks {
        w.write_i32(rank);
        w.write_i32(0); // party
    }
    w.into_bytes()
}

/// Port of `serverpackets/PledgeReceivePowerInfo` — one member's rank + the
/// privilege mask that rank currently holds.
pub fn pledge_receive_power_info(power_grade: i32, name: &str, privs: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_PLEDGE_RECEIVE_POWER_INFO);
    w.write_i32(power_grade);
    w.write_string(name);
    w.write_i32(privs);
    w.into_bytes()
}

/// Port of `serverpackets/PledgeReceiveMemberInfo` — the member-detail pane of
/// the clan window. Apprentice/sponsor stays empty (TODO(G18.6): academy).
pub fn pledge_receive_member_info(m: &crate::model::clan::ClanMember, clan_name: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_PLEDGE_RECEIVE_MEMBER_INFO);
    w.write_i32(0); // pledge type (main)
    w.write_string(&m.name);
    w.write_string(&m.title);
    w.write_i32(m.power_grade);
    w.write_string(clan_name); // main pledge → the clan's own name
    w.write_string(""); // apprentice/sponsor name
    w.into_bytes()
}
