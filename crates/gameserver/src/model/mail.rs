//! Mail / post (G30) — port of Java `model/Message` +
//! `instancemanager/MailManager`.
//!
//! A message is a row in `messages` plus, when it carries attachments, a set of
//! `items` rows parked at `loc = 'MAIL'`, `loc_data = messageId`. Both sides of
//! a message may be offline, so — unlike the player-owned containers — the whole
//! store lives on `World` and every mutation writes through to the DB
//! immediately (the clan-warehouse pattern, not the memory-first player one).
//!
//! Out of scope (see PLAN_G30_MAIL_PARTY_MATCHING.md): the commission-house and
//! prime-shop mail types and their conditional packet blocks, and Mobius's
//! `CustomMailManager`.

use std::collections::HashMap;

/// Java `enums/MailType`. The **ordinal is the wire value**, so the
/// out-of-scope Kamael-era variants keep their slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailType {
    /// Player-to-player mail — the only kind a client can send.
    Regular = 0,
    NewsInformer = 1,
    Npc = 2,
    Birthday = 3,
}

impl MailType {
    pub fn id(self) -> i32 {
        self as i32
    }

    pub fn from_id(v: i32) -> Self {
        match v {
            1 => Self::NewsInformer,
            2 => Self::Npc,
            3 => Self::Birthday,
            _ => Self::Regular,
        }
    }

    /// Java `Message.getSenderName()` shows a literal "System" for every
    /// non-`REGULAR` type regardless of the stored sender.
    pub fn is_system(self) -> bool {
        !matches!(self, Self::Regular)
    }
}

/// Java `Message.EXPIRATION` — 360 hours ("15 days") for ordinary mail.
pub const EXPIRATION_HOURS: i64 = 360;
/// Java `Message.COD_EXPIRATION` — 12 hours for a payment-request mail.
pub const COD_EXPIRATION_HOURS: i64 = 12;

/// The `changeId` of `ExChangePostState` (Java `Message.DELETED/READED/REJECTED`).
pub const STATE_DELETED: i32 = 0;
pub const STATE_READ: i32 = 1;
pub const STATE_REJECTED: i32 = 2;

/// One mail message. Mirrors the `messages` table one-to-one, minus the
/// vestigial `isLocked` column (Java never reads or writes it and derives the
/// flag from `req_adena > 0`) and the commission-only display columns.
#[derive(Debug, Clone)]
pub struct Message {
    pub id: i32,
    /// `-1` for system mail.
    pub sender_id: i32,
    pub receiver_id: i32,
    pub subject: String,
    pub content: String,
    /// Absolute epoch millis.
    pub expiration: i64,
    /// Cash-on-delivery price; `> 0` makes the message "locked".
    pub req_adena: i64,
    pub has_attachments: bool,
    pub unread: bool,
    pub deleted_by_sender: bool,
    pub deleted_by_receiver: bool,
    pub mail_type: MailType,
    /// Set on the copy generated when a receiver rejects the attachments.
    pub returned: bool,
}

impl Message {
    /// Java's player-to-player constructor.
    pub fn new_player_mail(
        id: i32,
        sender_id: i32,
        receiver_id: i32,
        is_cod: bool,
        subject: String,
        content: String,
        req_adena: i64,
        now_millis: i64,
    ) -> Self {
        let hours = if is_cod {
            COD_EXPIRATION_HOURS
        } else {
            EXPIRATION_HOURS
        };
        Self {
            id,
            sender_id,
            receiver_id,
            subject,
            content,
            expiration: now_millis + hours * 3_600_000,
            req_adena,
            has_attachments: false,
            unread: true,
            deleted_by_sender: false,
            deleted_by_receiver: false,
            mail_type: MailType::Regular,
            returned: false,
        }
    }

    /// Java's system-mail constructor: no live sender, and already "deleted"
    /// on the sender side so it never shows in anyone's outbox.
    pub fn new_system_mail(
        id: i32,
        receiver_id: i32,
        subject: String,
        content: String,
        mail_type: MailType,
        now_millis: i64,
    ) -> Self {
        Self {
            id,
            sender_id: -1,
            receiver_id,
            subject,
            content,
            expiration: now_millis + EXPIRATION_HOURS * 3_600_000,
            req_adena: 0,
            has_attachments: false,
            unread: true,
            deleted_by_sender: true,
            deleted_by_receiver: false,
            mail_type,
            returned: false,
        }
    }

    /// Java `isLocked()` — a COD mail cannot be opened without paying.
    pub fn is_locked(&self) -> bool {
        self.req_adena > 0
    }

    /// Java `getExpirationSeconds()`.
    pub fn expiration_seconds(&self) -> i32 {
        (self.expiration / 1000) as i32
    }

    pub fn is_expired(&self, now_millis: i64) -> bool {
        now_millis >= self.expiration
    }
}

/// Java `MailManager`: every message in the world, keyed by id.
#[derive(Debug, Default)]
pub struct MailManager {
    pub messages: HashMap<i32, Message>,
    /// Attachments per message id — the `items` rows at `loc = 'MAIL'`.
    /// Held here rather than on a player because neither party need be online.
    pub attachments: HashMap<i32, crate::model::inventory::Inventory>,
}

/// Java's `INBOX_SIZE`/`OUTBOX_SIZE`.
pub const MAILBOX_LIMIT: usize = 240;

impl MailManager {
    pub fn get(&self, id: i32) -> Option<&Message> {
        self.messages.get(&id)
    }

    pub fn get_mut(&mut self, id: i32) -> Option<&mut Message> {
        self.messages.get_mut(&id)
    }

    /// Java `getInbox` — everything addressed to a player that they have not
    /// deleted, newest first (Java loads `ORDER BY expiration` and the client
    /// shows them in list order).
    pub fn inbox(&self, object_id: i32) -> Vec<&Message> {
        let mut v: Vec<&Message> = self
            .messages
            .values()
            .filter(|m| m.receiver_id == object_id && !m.deleted_by_receiver)
            .collect();
        v.sort_by_key(|m| std::cmp::Reverse(m.id));
        v
    }

    /// Java `getOutbox`.
    pub fn outbox(&self, object_id: i32) -> Vec<&Message> {
        let mut v: Vec<&Message> = self
            .messages
            .values()
            .filter(|m| m.sender_id == object_id && !m.deleted_by_sender)
            .collect();
        v.sort_by_key(|m| std::cmp::Reverse(m.id));
        v
    }

    pub fn inbox_size(&self, object_id: i32) -> usize {
        self.messages
            .values()
            .filter(|m| m.receiver_id == object_id && !m.deleted_by_receiver)
            .count()
    }

    pub fn outbox_size(&self, object_id: i32) -> usize {
        self.messages
            .values()
            .filter(|m| m.sender_id == object_id && !m.deleted_by_sender)
            .count()
    }

    /// Java `getUnreadCount`. Note Java's sibling `hasUnreadPost()` ignores
    /// `deletedByReceiver` and so can disagree with this; the port uses the
    /// inbox-based definition everywhere.
    pub fn unread_count(&self, object_id: i32) -> i32 {
        self.inbox(object_id).iter().filter(|m| m.unread).count() as i32
    }

    pub fn insert(&mut self, message: Message) {
        self.messages.insert(message.id, message);
    }

    pub fn remove(&mut self, id: i32) -> Option<Message> {
        self.attachments.remove(&id);
        self.messages.remove(&id)
    }
}

/// A listing row, flattened out of a [`Message`] so the game-loop layer can
/// resolve names without borrowing the manager while it writes.
pub struct MailListRow {
    pub message_id: i32,
    pub subject: String,
    /// The *other* party: sender for an inbox row, receiver for an outbox row.
    pub counterparty_id: i32,
    /// Java shows a literal "System" instead of the sender for system mail.
    pub system_sender: bool,
    pub locked: bool,
    pub expiration_seconds: i32,
    pub unread: bool,
    pub has_attachments: bool,
    pub returned: bool,
    pub mail_type: i32,
}

impl MailListRow {
    fn of(m: &Message, counterparty_id: i32, system_sender: bool) -> Self {
        Self {
            message_id: m.id,
            subject: m.subject.clone(),
            counterparty_id,
            system_sender,
            locked: m.is_locked(),
            expiration_seconds: m.expiration_seconds(),
            unread: m.unread,
            has_attachments: m.has_attachments,
            returned: m.returned,
            mail_type: m.mail_type.id(),
        }
    }

    pub fn inbox(mgr: &MailManager, object_id: i32) -> Vec<Self> {
        mgr.inbox(object_id)
            .into_iter()
            .map(|m| Self::of(m, m.sender_id, m.mail_type.is_system()))
            .collect()
    }

    pub fn outbox(mgr: &MailManager, object_id: i32) -> Vec<Self> {
        mgr.outbox(object_id)
            .into_iter()
            .map(|m| Self::of(m, m.receiver_id, false))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_700_000_000_000;

    fn mail(id: i32, sender: i32, receiver: i32) -> Message {
        Message::new_player_mail(id, sender, receiver, false, "s".into(), "c".into(), 0, NOW)
    }

    #[test]
    fn regular_mail_expires_in_fifteen_days_and_cod_in_twelve_hours() {
        let regular = mail(1, 10, 20);
        assert_eq!(regular.expiration, NOW + 360 * 3_600_000);
        let cod = Message::new_player_mail(2, 10, 20, true, "s".into(), "c".into(), 500, NOW);
        assert_eq!(cod.expiration, NOW + 12 * 3_600_000);
        assert!(cod.is_locked() && !regular.is_locked());
        assert!(!cod.is_expired(NOW) && cod.is_expired(cod.expiration));
    }

    #[test]
    fn system_mail_never_appears_in_an_outbox() {
        let mut mgr = MailManager::default();
        mgr.insert(Message::new_system_mail(
            1,
            20,
            "gift".into(),
            "".into(),
            MailType::Birthday,
            NOW,
        ));
        assert_eq!(mgr.inbox(20).len(), 1);
        assert_eq!(mgr.outbox(-1).len(), 0, "deleted_by_sender from the start");
        assert!(MailType::Birthday.is_system());
        assert!(!MailType::Regular.is_system());
    }

    #[test]
    fn deleting_from_one_side_hides_it_only_from_that_side() {
        let mut mgr = MailManager::default();
        mgr.insert(mail(1, 10, 20));
        assert_eq!(mgr.inbox_size(20), 1);
        assert_eq!(mgr.outbox_size(10), 1);

        mgr.get_mut(1).unwrap().deleted_by_receiver = true;
        assert_eq!(mgr.inbox_size(20), 0);
        assert_eq!(mgr.outbox_size(10), 1, "the sender still sees it");
    }

    #[test]
    fn unread_count_ignores_read_and_receiver_deleted_mail() {
        let mut mgr = MailManager::default();
        for id in 1..=3 {
            mgr.insert(mail(id, 10, 20));
        }
        assert_eq!(mgr.unread_count(20), 3);
        mgr.get_mut(1).unwrap().unread = false;
        mgr.get_mut(2).unwrap().deleted_by_receiver = true;
        assert_eq!(mgr.unread_count(20), 1);
    }

    #[test]
    fn removing_a_message_drops_its_attachments_too() {
        let mut mgr = MailManager::default();
        mgr.insert(mail(1, 10, 20));
        mgr.attachments
            .insert(1, crate::model::inventory::Inventory::default());
        assert!(mgr.remove(1).is_some());
        assert!(mgr.messages.is_empty() && mgr.attachments.is_empty());
    }

    #[test]
    fn mail_type_ordinals_match_the_wire() {
        assert_eq!(MailType::Regular.id(), 0);
        assert_eq!(MailType::Birthday.id(), 3);
        assert_eq!(MailType::from_id(3), MailType::Birthday);
        assert_eq!(
            MailType::from_id(99),
            MailType::Regular,
            "unknown ordinals degrade to REGULAR"
        );
    }
}
