//! Chat, mail, friends and the html-driven commands (bypass, community
//! board, `//` build commands, user commands).

use super::items::read_item_lines;
use commons::network::PacketReader;

/// Port of `clientpackets/RequestSendPost` (G30): recipient, COD flag, subject,
/// body, the attachment list, and the payment-request price.
pub struct RequestSendPost {
    pub receiver: String,
    pub is_cod: bool,
    pub subject: String,
    pub text: String,
    /// `(object id, count)` per attached item; empty when nothing is attached.
    pub items: Vec<(i32, i64)>,
    pub req_adena: i64,
}

impl RequestSendPost {
    /// Java's own caps, re-checked in the handler for the ones that carry a
    /// system message.
    pub const MAX_ATTACHMENTS: usize = 8;

    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let receiver = r.read_string()?;
        let is_cod = r.read_i32()? != 0;
        let subject = r.read_string()?;
        let text = r.read_string()?;
        let count = r.read_i32()?;
        let mut items = Vec::new();
        if count > 0 {
            // Java rejects the whole packet when the declared count doesn't
            // match the bytes left (`count * 12 + 8 != remaining`).
            if count as usize > Self::MAX_ATTACHMENTS * 4 {
                return None;
            }
            items = read_item_lines(&mut r, count)?;
        }
        let req_adena = r.read_i64()?;
        Some(Self {
            receiver,
            is_cod,
            subject,
            text,
            items,
            req_adena,
        })
    }
}

/// Port of `RequestDeleteReceivedPost` / `RequestDeleteSentPost` (G30): a
/// count-prefixed list of message ids.
pub struct DeletePostList {
    pub message_ids: Vec<i32>,
}

impl DeletePostList {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let count = r.read_i32()?;
        if count <= 0 || count > 240 {
            return None;
        }
        let mut message_ids = Vec::with_capacity(count as usize);
        for _ in 0..count {
            message_ids.push(r.read_i32()?);
        }
        Some(Self { message_ids })
    }
}

/// Port of `clientpackets/Say2`: chat text, channel (`ChatType` client id),
/// and — for WHISPER (2) only — the target player name.
pub struct Say2 {
    pub text: String,
    pub chat_type: i32,
    pub target: Option<String>,
}

impl Say2 {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let text = r.read_string()?;
        let chat_type = r.read_i32()?;
        let target = if chat_type == crate::enums::ChatType::Whisper.client_id() {
            Some(r.read_string()?)
        } else {
            None
        };
        Some(Self {
            text,
            chat_type,
            target,
        })
    }
}

/// Port of `clientpackets/RequestBypassToServer` — one command string, sent
/// by the client when an HTML `action="bypass -h …"` link is clicked (the
/// `bypass -h ` prefix is stripped client-side; the command arrives bare).
pub fn read_bypass_command(body_after_opcode: &[u8]) -> Option<String> {
    PacketReader::new(body_after_opcode).read_string()
}

/// Port of `clientpackets/RequestBBSwrite.readImpl` — a board write/submit:
/// the target `url` plus five string arguments. `CommunityBoardHandler`
/// maps the `url` (`Topic`/`Post`/`Region`/`Notice`) to a `_bbs*` write
/// command; anything else answers a "not implemented" page.
pub fn read_bbs_write(body_after_opcode: &[u8]) -> Option<[String; 6]> {
    let mut r = PacketReader::new(body_after_opcode);
    Some([
        r.read_string()?, // url
        r.read_string()?, // arg1
        r.read_string()?, // arg2
        r.read_string()?, // arg3
        r.read_string()?, // arg4
        r.read_string()?, // arg5
    ])
}

/// Port of `clientpackets/SendBypassBuildCmd.readImpl` — one command string for
/// the `//command` GM bar, trimmed. The `admin_` prefix is added by the caller
/// (Java's `useAdminCommand(player, "admin_" + cmd, true)`).
pub fn read_build_command(body_after_opcode: &[u8]) -> Option<String> {
    PacketReader::new(body_after_opcode)
        .read_string()
        .map(|s| s.trim().to_string())
}

/// Port of `clientpackets/BypassUserCmd` — the `/command` bar's int command id.
pub fn read_user_command(body_after_opcode: &[u8]) -> Option<i32> {
    PacketReader::new(body_after_opcode).read_i32()
}

/// Port of `clientpackets/DlgAnswer` — the client's reply to a `ConfirmDlg`:
/// the echoed message id, the answer (1 = yes, 0 = no), and the requester id.
pub struct DlgAnswer {
    pub message_id: i32,
    pub answer: i32,
    pub requester_id: i32,
}

impl DlgAnswer {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let message_id = r.read_i32()?;
        let answer = r.read_i32()?;
        let requester_id = r.read_i32()?;
        Some(Self {
            message_id,
            answer,
            requester_id,
        })
    }
}

/// `RequestOustPartyMember` / `RequestChangePartyLeader` — one name.
pub fn read_name(body_after_opcode: &[u8]) -> Option<String> {
    PacketReader::new(body_after_opcode).read_string()
}

/// `RequestAnswerFriendInvite` — a pad byte, then the response int.
pub fn read_friend_answer(body_after_opcode: &[u8]) -> Option<i32> {
    let mut r = PacketReader::new(body_after_opcode);
    r.read_u8()?;
    r.read_i32()
}

/// Port of `clientpackets/friend/RequestSendFriendMsg`.
pub struct RequestSendFriendMsg {
    pub message: String,
    pub receiver: String,
}

impl RequestSendFriendMsg {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let message = r.read_string()?;
        let receiver = r.read_string()?;
        Some(Self { message, receiver })
    }
}
