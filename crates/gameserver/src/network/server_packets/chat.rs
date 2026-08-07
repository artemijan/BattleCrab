//! Chat packet (`CreatureSay`).

use commons::network::PacketWriter;

use super::opcodes;

fn ex(sub: i16) -> PacketWriter {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(sub);
    w
}

/// Port of `serverpackets/ExWorldChatCnt` — how many world-chat lines the
/// player has left today.
///
/// Java computes the number in the *constructor*, from the player it is handed:
/// `level < WORLD_CHAT_MIN_LEVEL ? 0 : max(points - used, 0)`. Taking the
/// already-resolved count as an argument keeps that arithmetic in one place
/// (`game_loop::chat::world_chat_points_left`) instead of duplicating the
/// level check at each of the three call sites.
pub fn ex_world_chat_cnt(points_left: i32) -> Vec<u8> {
    let mut w = ex(opcodes::EX_WORLD_CHAT_CNT);
    w.write_i32(points_left);
    w.into_bytes()
}

/// Port of `serverpackets/CreatureSay` (the plain-text player branch):
/// sender object id, chat channel, sender name, the NpcString id slot (-1 =
/// literal text), the text — and, for player WHISPERs only, the trailing
/// receiver-relation mask byte + sender level (`whisper_tail`; mask bit 0x01 =
/// sender is on the receiver's friend list, other bits need clans/mentors).
pub fn creature_say(
    sender_object_id: i32,
    chat_type: crate::enums::ChatType,
    sender_name: &str,
    text: &str,
    whisper_tail: Option<(u8, i32)>,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SAY2);
    w.write_i32(sender_object_id);
    w.write_i32(chat_type.client_id());
    w.write_string(sender_name);
    w.write_i32(-1); // NpcString id — plain text
    w.write_string(text);
    if let Some((mask, level)) = whisper_tail {
        w.write_u8(mask);
        if mask & 0x10 == 0 {
            w.write_u8(level as u8);
        }
    }
    w.into_bytes()
}

/// Port of `PetitionVotePacket` (G31) — the empty opcode-only packet that
/// prompts the petitioner's feedback dialog once a consultation ends.
pub fn petition_vote() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PETITION_VOTE);
    w.into_bytes()
}

/// Port of `Snoop` (G31) — one eavesdropped chat line delivered to a snooping
/// GM: the snooped player's id + name, the channel, and the actual speaker +
/// text (for whispers the speaker differs from the snooped player).
pub fn snoop(
    snooped_object_id: i32,
    snooped_name: &str,
    chat_type: crate::enums::ChatType,
    speaker_name: &str,
    text: &str,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SNOOP);
    w.write_i32(snooped_object_id);
    w.write_string(snooped_name);
    w.write_i32(0); // Java writes an unknown 0 here
    w.write_i32(chat_type.client_id());
    w.write_string(speaker_name);
    w.write_string(text);
    w.into_bytes()
}

/// Port of `CreatureSay(ChatType, int charId, SystemMessageId)` — the
/// system-message branch (no `Creature` sender, no literal text). Java writes
/// the sender-name slot as the raw `charId` int (the `_senderName == null`
/// branch): for a small id the high two bytes are zero, so the client reads it
/// as an (empty) UTF-16 string and then the message id. Used for the ferry
/// boarding/departure announcements (`charId` 801, `ChatType::Boat`).
pub fn creature_say_system(
    chat_type: crate::enums::ChatType,
    char_id: i32,
    message_id: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SAY2);
    w.write_i32(0); // sender object id — no Creature sender
    w.write_i32(chat_type.client_id());
    w.write_i32(char_id); // name slot written as an int (senderName == null)
    w.write_i32(message_id); // NpcString / system-message id, no trailing text
    w.into_bytes()
}
