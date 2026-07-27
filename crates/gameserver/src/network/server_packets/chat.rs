//! Chat packet (`CreatureSay`).

use commons::network::PacketWriter;

use super::opcodes;

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
