//! Party, party-matching-room and command-channel (MPCC) packets.

use commons::network::PacketReader;

/// Port of `clientpackets/RequestPartyMatchConfig` (`ddd`, G30): the room-list
/// page, the location (community-board region) filter, and the level-band mode
/// (`0` = my level range, anything else = all).
pub struct RequestPartyMatchConfig {
    pub page: i32,
    pub location: i32,
    pub level_filter: i32,
}

impl RequestPartyMatchConfig {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            page: r.read_i32()?,
            location: r.read_i32()?,
            level_filter: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/RequestPartyMatchList` (`dddddS`, G30): create a room
/// (`room_id <= 0`) or edit the one you lead.
pub struct RequestPartyMatchList {
    pub room_id: i32,
    pub max_members: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub loot_type: i32,
    pub title: String,
}

impl RequestPartyMatchList {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            room_id: r.read_i32()?,
            max_members: r.read_i32()?,
            min_level: r.read_i32()?,
            max_level: r.read_i32()?,
            loot_type: r.read_i32()?,
            title: r.read_string()?,
        })
    }
}

/// Port of `clientpackets/RequestPartyMatchDetail` (`ddd`, G30): join a room by
/// id, or — when `room_id <= 0` — the first room matching a location + level.
pub struct RequestPartyMatchDetail {
    pub room_id: i32,
    pub location: i32,
    pub level: i32,
}

impl RequestPartyMatchDetail {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            room_id: r.read_i32()?,
            location: r.read_i32()?,
            level: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/RequestListPartyMatchingWaitingRoom` (`dddd(d)*(S)?`,
/// G30): the looking-for-party browse filter.
pub struct RequestListPartyMatchingWaitingRoom {
    pub page: i32,
    pub min_level: i32,
    pub max_level: i32,
    /// Empty means "any class" (Java leaves the list null).
    pub class_ids: Vec<i32>,
    /// Optional name substring; Java only reads it when bytes remain.
    pub query: Option<String>,
}

impl RequestListPartyMatchingWaitingRoom {
    /// Java's own bound: it only consumes the class ids when
    /// `0 < size < 128`, which desyncs the rest of the read for a larger
    /// count. The port consumes exactly what the count claims (capped) so the
    /// trailing query string still lines up.
    const MAX_CLASSES: i32 = 127;

    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let page = r.read_i32()?;
        let min_level = r.read_i32()?;
        let max_level = r.read_i32()?;
        let size = r.read_i32()?;
        let mut class_ids = Vec::new();
        if size > 0 {
            if size > Self::MAX_CLASSES {
                return None;
            }
            for _ in 0..size {
                class_ids.push(r.read_i32()?);
            }
        }
        let query = if r.remaining() > 0 {
            r.read_string()
        } else {
            None
        };
        Some(Self {
            page,
            min_level,
            max_level,
            class_ids,
            query,
        })
    }
}

/// Port of `clientpackets/RequestExAskJoinMPCC` (`S`): invite a player's party
/// into a command channel by the clicked player's name.
pub struct RequestExAskJoinMpcc {
    pub name: String,
}

impl RequestExAskJoinMpcc {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        read_named(body_after_opcode, |name| Self { name })
    }
}

pub(crate) fn read_named<T>(body: &[u8], ctor: impl FnOnce(String) -> T) -> Option<T> {
    PacketReader::new(body).read_string().map(ctor)
}

/// Port of `clientpackets/RequestExAcceptJoinMPCC` (`d`): 1 = accept.
pub struct RequestExAcceptJoinMpcc {
    pub response: i32,
}

impl RequestExAcceptJoinMpcc {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            response: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/RequestExOustFromMPCC` (`S`): dismiss the named
/// player's whole party from the channel.
pub struct RequestExOustFromMpcc {
    pub name: String,
}

impl RequestExOustFromMpcc {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        read_named(body_after_opcode, |name| Self { name })
    }
}

/// Port of `clientpackets/RequestExMPCCShowPartyMembersInfo` (`d`): the CC
/// window queries a party's roster by its leader's object id.
pub struct RequestExMpccShowPartyMembersInfo {
    pub party_leader_object_id: i32,
}

impl RequestExMpccShowPartyMembersInfo {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            party_leader_object_id: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/RequestExListMpccWaiting` (`ddd`): browse CC rooms.
pub struct RequestExListMpccWaiting {
    pub page: i32,
    pub location: i32,
    pub level: i32,
}

impl RequestExListMpccWaiting {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            page: r.read_i32()?,
            location: r.read_i32()?,
            level: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/RequestExManageMpccRoom` (`dddddS`): edit the CC
/// room you lead. The fifth int (party distribution type) is read and
/// discarded, as in Java.
pub struct RequestExManageMpccRoom {
    pub room_id: i32,
    pub max_members: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub title: String,
}

impl RequestExManageMpccRoom {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let room_id = r.read_i32()?;
        let max_members = r.read_i32()?;
        let min_level = r.read_i32()?;
        let max_level = r.read_i32()?;
        let _loot_type = r.read_i32()?;
        Some(Self {
            room_id,
            max_members,
            min_level,
            max_level,
            title: r.read_string()?,
        })
    }
}

/// Port of `clientpackets/RequestExJoinMpccRoom` (`d`).
pub struct RequestExJoinMpccRoom {
    pub room_id: i32,
}

impl RequestExJoinMpccRoom {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            room_id: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/RequestExOustFromMpccRoom` (`d`): kick by object id.
pub struct RequestExOustFromMpccRoom {
    pub object_id: i32,
}

impl RequestExOustFromMpccRoom {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            object_id: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/RequestJoinParty`: invitee name + the loot rule a
/// brand-new party would use.
pub struct RequestJoinParty {
    pub name: String,
    pub loot_rule_id: i32,
}

impl RequestJoinParty {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let name = r.read_string()?;
        let loot_rule_id = r.read_i32()?;
        Some(Self { name, loot_rule_id })
    }
}

/// `RequestAnswerJoinParty` / `AnswerPartyLootModification` — one int
/// (1 = yes; party-answer -1 = auto-refuse mode).
pub fn read_answer(body_after_opcode: &[u8]) -> Option<i32> {
    PacketReader::new(body_after_opcode).read_i32()
}
