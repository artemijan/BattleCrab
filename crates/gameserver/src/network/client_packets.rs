//! Inbound (client → server) packets. Ported 1:1 from
//! `gameserver/network/clientpackets`. G1 covers only the transport handshake
//! packet `ProtocolVersion`; gameplay packets are parsed/dispatched on the game
//! thread from G2 on.

use commons::network::PacketReader;

/// `ClientPackets` opcodes (single-byte `_id`).
pub mod opcodes {
    pub const LOGOUT: u8 = 0x00;
    pub const MOVE_BACKWARD_TO_LOCATION: u8 = 0x0F;
    pub const PROTOCOL_VERSION: u8 = 0x0E;
    pub const AUTH_LOGIN: u8 = 0x2B;
    pub const CHARACTER_CREATE: u8 = 0x0C;
    pub const CHARACTER_DELETE: u8 = 0x0D;
    pub const ENTER_WORLD: u8 = 0x11;
    pub const CHARACTER_SELECT: u8 = 0x12;
    pub const NEW_CHARACTER: u8 = 0x13;
    pub const REQUEST_ITEM_LIST: u8 = 0x14;
    pub const REQUEST_SKILL_COOL_TIME: u8 = 0xA6;
    pub const CHARACTER_RESTORE: u8 = 0x7B;
    pub const REQUEST_UN_EQUIP_ITEM: u8 = 0x16;
    pub const USE_ITEM: u8 = 0x19;
    pub const ACTION: u8 = 0x1F;
    pub const REQUEST_MAGIC_SKILL_USE: u8 = 0x39;
    pub const REQUEST_TARGET_CANCELD: u8 = 0x48;
    pub const REQUEST_RESTART: u8 = 0x57;
    pub const VALIDATE_POSITION: u8 = 0x59;
    pub const REQUEST_ACQUIRE_SKILL: u8 = 0x7C;
    pub const REQUEST_SKILL_LIST: u8 = 0x50;
    pub const ATTACK_REQUEST: u8 = 0x32;
    pub const APPEARING: u8 = 0x3A;
    pub const REQUEST_RESTART_POINT: u8 = 0x7D;
    pub const REQUEST_SHORT_CUT_REG: u8 = 0x3D;
    pub const REQUEST_SHORT_CUT_DEL: u8 = 0x3F;
    pub const REQUEST_MAKE_MACRO: u8 = 0xCD;
    pub const REQUEST_DELETE_MACRO: u8 = 0xCE;
    pub const SAY2: u8 = 0x49;
    pub const REQUEST_BYPASS_TO_SERVER: u8 = 0x23;
    pub const REQUEST_QUEST_ABORT: u8 = 0x63;
    pub const REQUEST_BUY_ITEM: u8 = 0x40;
    pub const REQUEST_JOIN_PARTY: u8 = 0x42;
    pub const REQUEST_ANSWER_JOIN_PARTY: u8 = 0x43;
    pub const REQUEST_WITH_DRAWAL_PARTY: u8 = 0x44;
    pub const REQUEST_OUST_PARTY_MEMBER: u8 = 0x45;
    pub const REQUEST_SEND_FRIEND_MSG: u8 = 0x6B;
    pub const REQUEST_FRIEND_INVITE: u8 = 0x77;
    pub const REQUEST_ANSWER_FRIEND_INVITE: u8 = 0x78;
    pub const REQUEST_FRIEND_LIST: u8 = 0x79;
    pub const REQUEST_FRIEND_DEL: u8 = 0x7A;
    /// Extended packets: opcode 0xD0 + a 2-byte little-endian sub-opcode.
    pub const EX_PACKET: u8 = 0xD0;
}

/// Extended (`0xD0`) client sub-opcodes.
pub mod ex_opcodes {
    pub const REQUEST_MANOR_LIST: u16 = 0x01;
    pub const REQUEST_KEY_MAPPING: u16 = 0x21;
    pub const REQUEST_CHARACTER_NAME_CREATABLE: u16 = 0xA9;
    pub const REQUEST_USER_BAN_INFO: u16 = 0x138;
    pub const REQUEST_GOTO_LOBBY: u16 = 0x33;
    pub const REQUEST_CHANGE_PARTY_LEADER: u16 = 0x0C;
    pub const REQUEST_PARTY_LOOT_MODIFICATION: u16 = 0x75;
    pub const ANSWER_PARTY_LOOT_MODIFICATION: u16 = 0x76;
    pub const REQUEST_SAVE_INVENTORY_ORDER: u16 = 0x24;
}

/// Split an extended-packet body (after the `0xD0` opcode) into its 2-byte LE
/// sub-opcode and the remaining payload.
pub fn read_ex_opcode(body_after_opcode: &[u8]) -> Option<(u16, &[u8])> {
    if body_after_opcode.len() < 2 {
        return None;
    }
    let sub = u16::from_le_bytes([body_after_opcode[0], body_after_opcode[1]]);
    Some((sub, &body_after_opcode[2..]))
}

/// The name field of `RequestCharacterNameCreatable` (after the sub-opcode).
pub fn read_name_creatable(ex_body: &[u8]) -> Option<String> {
    PacketReader::new(ex_body).read_string()
}

/// Port of `clientpackets/ProtocolVersion`. Never encrypted (first packet).
/// A missing/short version reads as 0 (Java swallows the exception → `_version = 0`).
pub struct ProtocolVersion {
    pub version: i32,
}

impl ProtocolVersion {
    /// `readImpl`: the opcode byte has already been consumed by the dispatcher.
    pub fn read(body_after_opcode: &[u8]) -> Self {
        let mut r = PacketReader::new(body_after_opcode);
        Self { version: r.read_i32().unwrap_or(0) }
    }
}

/// Port of `clientpackets/AuthLogin`. The account name and the two session-key
/// halves the client echoes from the login handoff. Field order matches
/// `readImpl`: name, playKey2, playKey1, loginKey1, loginKey2.
pub struct AuthLogin {
    pub login_name: String,
    pub play_key1: i32,
    pub play_key2: i32,
    pub login_key1: i32,
    pub login_key2: i32,
}

impl AuthLogin {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let login_name = r.read_string()?.to_lowercase();
        let play_key2 = r.read_i32()?;
        let play_key1 = r.read_i32()?;
        let login_key1 = r.read_i32()?;
        let login_key2 = r.read_i32()?;
        Some(Self { login_name, play_key1, play_key2, login_key1, login_key2 })
    }
}

/// Port of `clientpackets/CharacterCreate` (`cSdddddddddddd`).
pub struct CharacterCreate {
    pub name: String,
    pub is_female: bool,
    pub class_id: i32,
    pub hair_style: i32,
    pub hair_color: i32,
    pub face: i32,
}

impl CharacterCreate {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let name = r.read_string()?;
        r.read_i32()?; // race (ignored; derived from class)
        let is_female = r.read_i32()? != 0;
        let class_id = r.read_i32()?;
        for _ in 0..6 {
            r.read_i32()?; // int/str/con/men/dex/wit (ignored)
        }
        let hair_style = r.read_i32()? & 0xff;
        let hair_color = r.read_i32()? & 0xff;
        let face = r.read_i32()? & 0xff;
        Some(Self { name, is_female, class_id, hair_style, hair_color, face })
    }
}

/// `clientpackets/CharacterDelete` / `CharacterRestore` — both carry a char
/// slot. `RequestUnEquipItem`'s single `int` field (a body-part bitmask, not a
/// slot index — see `Inventory::unequip_slot`) has the same shape, so it
/// reuses this reader too.
pub fn read_char_slot(body_after_opcode: &[u8]) -> Option<i32> {
    PacketReader::new(body_after_opcode).read_i32()
}

/// One purchase line of `RequestBuyItem`.
pub struct BuyLine {
    pub item_id: i32,
    pub count: i64,
}

/// Port of `clientpackets/RequestBuyItem.readImpl`: list id + item lines;
/// any non-positive id/count invalidates the whole request (Java nulls
/// `_items` and the handler answers ActionFailed — here the packet just
/// fails to parse, same net effect as the guards re-run in the handler).
pub struct RequestBuyItem {
    pub list_id: i32,
    pub items: Vec<BuyLine>,
}

impl RequestBuyItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let list_id = r.read_i32()?;
        let size = r.read_i32()?;
        // Java: `(size > 500) || ((size * 12) != remaining)` drops the packet.
        if size <= 0 || size > 500 {
            return None;
        }
        let mut items = Vec::with_capacity(size as usize);
        for _ in 0..size {
            let item_id = r.read_i32()?;
            let count = r.read_i64()?;
            if item_id < 1 || count < 1 {
                return None;
            }
            items.push(BuyLine { item_id, count });
        }
        Some(Self { list_id, items })
    }
}

/// Port of `clientpackets/UseItem` (`cdc`): the target item's object id, plus
/// a ctrl-pressed flag (used for split-stack prompts — not needed while gear
/// is the only thing `UseItem` acts on).
pub struct UseItem {
    pub object_id: i32,
    pub ctrl_pressed: bool,
}

impl UseItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let object_id = r.read_i32()?;
        let ctrl_pressed = r.read_i32()? != 0;
        Some(Self { object_id, ctrl_pressed })
    }
}

/// Port of `clientpackets/RequestSaveInventoryOrder` (`d[dd]`): the client's
/// custom inventory arrangement — one `(object_id, order)` pair per grid slot.
/// `order` is the slot index the client wants that item stored at (`items.
/// loc_data` for `INVENTORY`-located items). Java caps the count at `LIMIT`
/// (125) and silently drops the overflow.
pub struct RequestSaveInventoryOrder {
    pub order: Vec<(i32, i32)>,
}

impl RequestSaveInventoryOrder {
    const LIMIT: usize = 125;

    pub fn read(ex_body: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(ex_body);
        let count = (r.read_i32()? as usize).min(Self::LIMIT);
        let mut order = Vec::with_capacity(count);
        for _ in 0..count {
            let object_id = r.read_i32()?;
            let slot = r.read_i32()?;
            order.push((object_id, slot));
        }
        Some(Self { order })
    }
}

/// Port of `clientpackets/RequestMagicSkillUse` (`cdc`). `shift_pressed` is
/// Java's `dontMove`: an out-of-range shift-cast is cancelled (SM 748)
/// instead of walking into range. Ground targeting still waits on a later
/// milestone.
pub struct RequestMagicSkillUse {
    pub magic_id: i32,
    pub ctrl_pressed: bool,
    pub shift_pressed: bool,
}

impl RequestMagicSkillUse {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let magic_id = r.read_i32()?;
        let ctrl_pressed = r.read_i32()? != 0;
        let shift_pressed = r.read_u8().is_some_and(|b| b != 0);
        Some(Self { magic_id, ctrl_pressed, shift_pressed })
    }
}

/// Port of `clientpackets/RequestAcquireSkill`. `sub_type` is only meaningful
/// for `AcquireSkillType::Subpledge` (id `3`) — out of scope here (see the G6
/// plan's "only `CLASS`" note), read anyway to keep the reader positioned
/// correctly if the client ever sends it.
pub struct RequestAcquireSkill {
    pub skill_id: i32,
    pub skill_level: i32,
    pub acquire_type: i32,
}

impl RequestAcquireSkill {
    pub const CLASS: i32 = 0;
    pub const SUBPLEDGE: i32 = 3;

    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let skill_id = r.read_i32()?;
        let skill_level = r.read_i32()?;
        let acquire_type = r.read_i32()?;
        if acquire_type == Self::SUBPLEDGE {
            r.read_i32()?; // sub_type — unused (see doc comment)
        }
        Some(Self { skill_id, skill_level, acquire_type })
    }
}

/// Port of `clientpackets/Action` (`cdddc`). Origin x/y/z are the client's own
/// echoed position — Java reads them but never uses them (`@SuppressWarnings
/// ("unused")` on all three), so they're dropped here too.
pub struct Action {
    pub object_id: i32,
    pub action_id: u8,
}

impl Action {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let object_id = r.read_i32()?;
        r.read_i32()?; // origin_x — unused
        r.read_i32()?; // origin_y — unused
        r.read_i32()?; // origin_z — unused
        let action_id = r.read_u8()?;
        Some(Self { object_id, action_id })
    }
}

/// Port of `clientpackets/AttackRequest` (`cddddc`) — the client clicking an
/// already-targeted attackable creature. The origin coordinates and the
/// shift-click byte are read and discarded, like Java's unused fields.
pub struct AttackRequest {
    pub object_id: i32,
}

impl AttackRequest {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let object_id = r.read_i32()?;
        Some(Self { object_id })
    }
}

/// Port of `clientpackets/RequestRestartPoint` (`cd`) — the death dialog's
/// revive choice (0 = to village; the clan-hall/castle/fixed variants need
/// systems that don't exist yet).
pub struct RequestRestartPoint {
    pub point_type: i32,
}

impl RequestRestartPoint {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let point_type = r.read_i32()?;
        Some(Self { point_type })
    }
}

/// Port of `clientpackets/RequestTargetCanceld` (`ch`): a single flag, nonzero
/// meaning "the client wants its target cleared".
pub struct RequestTargetCanceld {
    pub target_lost: bool,
}

impl RequestTargetCanceld {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let target_lost = r.read_i16()? != 0;
        Some(Self { target_lost })
    }
}

/// Port of `clientpackets/MoveBackwardToLocation` (`cddddddd`). `origin_x/y/z`
/// is only used for the same-origin/target "stop" check — not stored as
/// server-trusted state, per the no-geodata scope (client position is trusted
/// only insofar as it drives where we start interpolating from; the server's
/// own `player.x/y/z` is the authoritative start point).
pub struct MoveBackwardToLocation {
    pub target_x: i32,
    pub target_y: i32,
    pub target_z: i32,
    pub origin_x: i32,
    pub origin_y: i32,
    pub origin_z: i32,
    pub movement_mode: i32,
}

impl MoveBackwardToLocation {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let target_x = r.read_i32()?;
        let target_y = r.read_i32()?;
        let target_z = r.read_i32()?;
        let origin_x = r.read_i32()?;
        let origin_y = r.read_i32()?;
        let origin_z = r.read_i32()?;
        let movement_mode = r.read_i32()?;
        Some(Self { target_x, target_y, target_z, origin_x, origin_y, origin_z, movement_mode })
    }
}

/// Port of `clientpackets/RequestShortCutReg` — drag something onto a panel
/// slot. The combined slot int decodes to `slot % 12` / `page = slot / 12`;
/// the type id is clamped to NONE outside 1-6, both like Java.
pub struct RequestShortCutReg {
    pub kind: crate::model::shortcut::ShortcutType,
    pub slot: i32,
    pub page: i32,
    pub id: i32,
    pub level: i32,
    pub sub_level: i32,
    pub character_type: i32,
}

impl RequestShortCutReg {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let kind = crate::model::shortcut::ShortcutType::from_ordinal(r.read_i32()?);
        let slot_raw = r.read_i32()?;
        let id = r.read_i32()?;
        let level = r.read_i16()? as i32;
        let sub_level = r.read_i16()? as i32;
        let character_type = r.read_i32()?;
        Some(Self { kind, slot: slot_raw % 12, page: slot_raw / 12, id, level, sub_level, character_type })
    }
}

/// Port of `clientpackets/RequestShortCutDel` — clear a panel slot.
pub struct RequestShortCutDel {
    pub slot: i32,
    pub page: i32,
}

impl RequestShortCutDel {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let slot_raw = r.read_i32()?;
        Some(Self { slot: slot_raw % 12, page: slot_raw / 12 })
    }
}

/// Port of `clientpackets/RequestMakeMacro` — create or edit a macro (macro
/// id 0 = create). The command count is hard-capped at 12
/// (`MAX_MACRO_LENGTH`); `commands_length` accumulates the command strings'
/// lengths for the 255-char validity gate.
pub struct RequestMakeMacro {
    pub macro_: crate::model::shortcut::Macro,
    pub commands_length: usize,
}

impl RequestMakeMacro {
    pub const MAX_MACRO_LENGTH: u8 = 12;

    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        use crate::model::shortcut::{Macro, MacroCmd, MacroType};
        let mut r = PacketReader::new(body_after_opcode);
        let id = r.read_i32()?;
        let name = r.read_string()?;
        let descr = r.read_string()?;
        let acronym = r.read_string()?;
        let icon = r.read_i32()?;
        let count = r.read_u8()?.min(Self::MAX_MACRO_LENGTH);
        let mut commands = Vec::with_capacity(count as usize);
        let mut commands_length = 0;
        for _ in 0..count {
            let entry = r.read_u8()? as i32;
            let kind = MacroType::from_ordinal(r.read_u8()? as i32);
            let d1 = r.read_i32()?;
            let d2 = r.read_u8()? as i32;
            let cmd = r.read_string()?;
            commands_length += cmd.chars().count();
            commands.push(MacroCmd { entry, kind, d1, d2, cmd });
        }
        Some(Self { macro_: Macro { id, icon, name, descr, acronym, commands }, commands_length })
    }
}

/// Port of `clientpackets/RequestDeleteMacro`.
pub struct RequestDeleteMacro {
    pub id: i32,
}

impl RequestDeleteMacro {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self { id: r.read_i32()? })
    }
}

/// Port of `clientpackets/ValidatePosition` — the client's periodic position
/// report. The trailing vehicle id is read and discarded (no boats).
pub struct ValidatePosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

impl ValidatePosition {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let x = r.read_i32()?;
        let y = r.read_i32()?;
        let z = r.read_i32()?;
        let heading = r.read_i32()?;
        let _vehicle_id = r.read_i32()?;
        Some(Self { x, y, z, heading })
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
        Some(Self { text, chat_type, target })
    }
}

/// Port of `clientpackets/RequestBypassToServer` — one command string, sent
/// by the client when an HTML `action="bypass -h …"` link is clicked (the
/// `bypass -h ` prefix is stripped client-side; the command arrives bare).
pub fn read_bypass_command(body_after_opcode: &[u8]) -> Option<String> {
    PacketReader::new(body_after_opcode).read_string()
}

/// Port of `clientpackets/RequestQuestAbort` — the quest UI's Abandon button.
pub struct RequestQuestAbort {
    pub quest_id: i32,
}

impl RequestQuestAbort {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self { quest_id: r.read_i32()? })
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

#[cfg(test)]
mod tests {
    use super::*;
    use commons::network::PacketWriter;

    fn save_order_body(count: i32, pairs: &[(i32, i32)]) -> Vec<u8> {
        let mut w = PacketWriter::new();
        w.write_i32(count);
        for &(object_id, order) in pairs {
            w.write_i32(object_id);
            w.write_i32(order);
        }
        w.into_bytes()
    }

    #[test]
    fn save_inventory_order_reads_pairs() {
        let pairs = [(1000, 0), (1001, 2), (1002, 1)];
        let body = save_order_body(pairs.len() as i32, &pairs);
        let pkt = RequestSaveInventoryOrder::read(&body).expect("parses");
        assert_eq!(pkt.order, pairs);
    }

    #[test]
    fn save_inventory_order_caps_at_limit() {
        // A count above LIMIT reads exactly LIMIT pairs; trailing pairs the
        // client sent past the cap are ignored (matches Java's `Math.min`).
        let pairs: Vec<(i32, i32)> =
            (0..RequestSaveInventoryOrder::LIMIT as i32 + 10).map(|i| (2000 + i, i)).collect();
        let body = save_order_body(pairs.len() as i32, &pairs);
        let pkt = RequestSaveInventoryOrder::read(&body).expect("parses");
        assert_eq!(pkt.order.len(), RequestSaveInventoryOrder::LIMIT);
        assert_eq!(pkt.order, pairs[..RequestSaveInventoryOrder::LIMIT]);
    }

    #[test]
    fn save_inventory_order_rejects_truncated() {
        // Claims two pairs but only supplies one.
        let body = save_order_body(2, &[(1000, 0)]);
        assert!(RequestSaveInventoryOrder::read(&body).is_none());
    }
}
