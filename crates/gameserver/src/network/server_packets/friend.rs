//! Friend packets (G10).

use commons::network::PacketWriter;

use super::opcodes;

/// One friend entry + live online flag, assembled by `game_loop/friends.rs`.
pub struct FriendEntry {
    pub char_id: i32,
    pub name: String,
    pub level: i32,
    pub class_id: i32,
    pub online: bool,
}

/// `serverpackets/friend/L2FriendList` — the enter-world friend roster.
pub fn l2_friend_list(entries: &[FriendEntry]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::L2_FRIEND_LIST);
    w.write_i32(entries.len() as i32);
    for e in entries {
        w.write_i32(e.char_id);
        w.write_string(&e.name);
        w.write_i32(e.online as i32);
        w.write_i32(if e.online { e.char_id } else { 0 });
        w.write_i32(e.level);
        w.write_i32(e.class_id);
        w.write_i16(0);
    }
    w.into_bytes()
}

/// `serverpackets/friend/FriendAddRequest` — the invite popup.
pub fn friend_add_request(requestor_name: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::FRIEND_ADD_REQUEST);
    w.write_u8(0);
    w.write_string(requestor_name);
    w.into_bytes()
}

/// `serverpackets/friend/FriendAddRequestResult` — pushes the new friend into
/// the client-side list (result 1 = accepted).
pub fn friend_add_request_result(result: i32, e: &FriendEntry) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::FRIEND_ADD_REQUEST_RESULT);
    w.write_i32(result);
    w.write_i32(e.char_id);
    w.write_string(&e.name);
    w.write_i32(e.online as i32);
    w.write_i32(if e.online { e.char_id } else { 0 });
    w.write_i32(e.level);
    w.write_i32(e.class_id);
    w.write_i16(0); // "Always 0 on retail"
    w.into_bytes()
}

/// `serverpackets/friend/FriendRemove`.
pub fn friend_remove(name: &str, response: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::FRIEND_REMOVE);
    w.write_i32(response);
    w.write_string(name);
    w.into_bytes()
}

/// `FriendStatus` modes.
pub mod friend_status_mode {
    pub const OFFLINE: i32 = 0;
    pub const ONLINE: i32 = 1;
    pub const LEVEL: i32 = 2;
    pub const CLASS: i32 = 3;
}

/// `serverpackets/friend/FriendStatus` — login/logout/level/class pings to
/// everyone who has this player friended.
pub fn friend_status(mode: i32, name: &str, extra: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::FRIEND_STATUS);
    w.write_i32(mode);
    w.write_string(name);
    match mode {
        friend_status_mode::OFFLINE | friend_status_mode::LEVEL | friend_status_mode::CLASS => w.write_i32(extra),
        _ => {} // ONLINE writes nothing extra
    }
    w.into_bytes()
}

/// `serverpackets/friend/L2FriendSay` — a `RequestSendFriendMsg` delivery.
pub fn l2_friend_say(sender: &str, receiver: &str, message: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::L2_FRIEND_SAY);
    w.write_i32(0);
    w.write_string(receiver);
    w.write_string(sender);
    w.write_string(message);
    w.into_bytes()
}
