//! Mail / post packets (G30).
//!
//! Interlude scoping (PLAN_G30_MAIL_PARTY_MATCHING.md): the conditional
//! commission-house blocks Java writes for `COMMISSION_ITEM_SOLD` /
//! `COMMISSION_ITEM_RETURNED` are omitted — those mail types are Kamael-era and
//! can't exist on this dist, so the branches would be dead.

use commons::network::PacketWriter;

use super::ex;
use super::opcodes;
use crate::data::item_data::ItemTemplate;
use crate::model::inventory::ItemInstance;
use crate::network::enter_world::write_item_entry;

/// Java `RequestSendPost.MESSAGE_FEE` — the flat adena cost of sending mail.
pub const MESSAGE_FEE: i64 = 100;
/// Java `RequestSendPost.MESSAGE_FEE_PER_SLOT` — added per attached item.
pub const MESSAGE_FEE_PER_SLOT: i64 = 1000;

/// One row of an inbox/outbox listing.
pub struct MailListView {
    pub message_id: i32,
    pub subject: String,
    /// Sender name for the inbox, receiver name for the outbox.
    pub counterparty: String,
    pub locked: bool,
    pub expiration_seconds: i32,
    pub unread: bool,
    pub has_attachments: bool,
    pub returned: bool,
    /// `MailType` ordinal — inbox only.
    pub mail_type: i32,
}

/// `ExShowReceivedPostList` (0xFE 0xAB) — the inbox.
pub fn ex_show_received_post_list(now_seconds: i32, mails: &[MailListView]) -> Vec<u8> {
    let mut w = ex(opcodes::EX_SHOW_RECEIVED_POST_LIST);
    w.write_i32(now_seconds);
    w.write_i32(mails.len() as i32);
    for m in mails {
        w.write_i32(m.mail_type);
        w.write_i32(m.message_id);
        w.write_string(&m.subject);
        w.write_string(&m.counterparty);
        w.write_i32(m.locked as i32);
        w.write_i32(m.expiration_seconds);
        w.write_i32(m.unread as i32);
        // "deletable" — always true here; Java only clears it for the
        // commission mail types this dist cannot produce.
        w.write_i32(1);
        w.write_i32(m.has_attachments as i32);
        w.write_i32(m.returned as i32);
        w.write_i32(0); // SysString slot, unused
    }
    w.write_i32(MESSAGE_FEE as i32);
    w.write_i32(MESSAGE_FEE_PER_SLOT as i32);
    w.into_bytes()
}

/// `ExShowSentPostList` (0xFE 0xAD) — the outbox.
pub fn ex_show_sent_post_list(now_seconds: i32, mails: &[MailListView]) -> Vec<u8> {
    let mut w = ex(opcodes::EX_SHOW_SENT_POST_LIST);
    w.write_i32(now_seconds);
    w.write_i32(mails.len() as i32);
    for m in mails {
        w.write_i32(m.message_id);
        w.write_string(&m.subject);
        w.write_string(&m.counterparty);
        w.write_i32(m.locked as i32);
        w.write_i32(m.expiration_seconds);
        w.write_i32(m.unread as i32);
        w.write_i32(1);
        w.write_i32(m.has_attachments as i32);
        w.write_i32(0);
    }
    w.into_bytes()
}

/// The item block both "read a mail" packets share: the standard item entry
/// followed by a repeat of the object id.
fn write_attachments(w: &mut PacketWriter, items: &[(&ItemInstance, &ItemTemplate)]) {
    w.write_i32(items.len() as i32);
    for (item, template) in items {
        write_item_entry(w, item, template, false);
        w.write_i32(item.object_id);
    }
}

/// `ExReplyReceivedPost` (0xFE 0xAC) — one opened inbox message.
pub fn ex_reply_received_post(
    message_id: i32,
    locked: bool,
    sender_name: &str,
    subject: &str,
    content: &str,
    items: &[(&ItemInstance, &ItemTemplate)],
    req_adena: i64,
    has_attachments: bool,
    returned: bool,
) -> Vec<u8> {
    let mut w = ex(opcodes::EX_SHOW_RECEIVED_POST);
    w.write_i32(crate::model::mail::MailType::Regular.id());
    w.write_i32(message_id);
    w.write_i32(locked as i32);
    w.write_i32(0); // unknown, Java writes a literal 0
    w.write_string(sender_name);
    w.write_string(subject);
    w.write_string(content);
    write_attachments(&mut w, items);
    w.write_i64(req_adena);
    w.write_i32(has_attachments as i32);
    w.write_i32(returned as i32);
    w.into_bytes()
}

/// `ExReplySentPost` (0xFE 0xAE) — one opened outbox message. Note the layout
/// differs from the inbox twin: no "unknown" int after `locked`.
pub fn ex_reply_sent_post(
    message_id: i32,
    locked: bool,
    receiver_name: &str,
    subject: &str,
    content: &str,
    items: &[(&ItemInstance, &ItemTemplate)],
    req_adena: i64,
    has_attachments: bool,
    returned: bool,
) -> Vec<u8> {
    let mut w = ex(opcodes::EX_SHOW_SENT_POST);
    w.write_i32(0); // Java's "GOD" placeholder, always 0 here
    w.write_i32(message_id);
    w.write_i32(locked as i32);
    w.write_string(receiver_name);
    w.write_string(subject);
    w.write_string(content);
    write_attachments(&mut w, items);
    w.write_i64(req_adena);
    w.write_i32(has_attachments as i32);
    w.write_i32(returned as i32);
    w.into_bytes()
}

/// `ExReplyPostItemList` (0xFE 0xB3) — what the compose window may attach.
pub fn ex_reply_post_item_list(items: &[(&ItemInstance, &ItemTemplate)]) -> Vec<u8> {
    let mut w = ex(opcodes::EX_REPLY_POST_ITEM_LIST);
    w.write_i32(items.len() as i32);
    for (item, template) in items {
        write_item_entry(&mut w, item, template, false);
    }
    w.into_bytes()
}

/// `ExChangePostState` (0xFE 0xB4) — tell the client a message changed state.
/// `received_board` picks the inbox (true) or outbox (false) list.
pub fn ex_change_post_state(received_board: bool, message_ids: &[i32], change_id: i32) -> Vec<u8> {
    let mut w = ex(opcodes::EX_CHANGE_POST_STATE);
    w.write_i32(received_board as i32);
    w.write_i32(message_ids.len() as i32);
    for id in message_ids {
        w.write_i32(*id);
        w.write_i32(change_id);
    }
    w.into_bytes()
}

/// `ExNoticePostArrived` (0xFE 0xAA) — the "you have mail" chime.
pub fn ex_notice_post_arrived(play_animation: bool) -> Vec<u8> {
    let mut w = ex(opcodes::EX_NOTICE_POST_ARRIVED);
    w.write_i32(play_animation as i32);
    w.into_bytes()
}

/// `ExNoticePostSent` (0xFE 0xB5) — sent confirmation. Java writes the
/// `EX_REPLY_WRITE_POST` opcode for this packet; kept verbatim.
pub fn ex_notice_post_sent(play_animation: bool) -> Vec<u8> {
    let mut w = ex(opcodes::EX_REPLY_WRITE_POST);
    w.write_i32(play_animation as i32);
    w.into_bytes()
}

/// `ExUnReadMailCount` (0xFE 0x13C) — the unread badge.
pub fn ex_unread_mail_count(count: i32) -> Vec<u8> {
    let mut w = ex(opcodes::EX_UN_READ_MAIL_COUNT);
    w.write_i32(count);
    w.into_bytes()
}
