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
    w.write_i32(0); // reputation score
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
    w.write_i32(0); // reputation score
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
