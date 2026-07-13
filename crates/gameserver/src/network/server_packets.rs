//! Outbound (server → client) packets. Ported 1:1 from
//! `gameserver/network/serverpackets`. Each builder returns the serialized body
//! (opcode + payload, unencrypted); the connection task encrypts and frames it.
//!
//! G1 covers only `KeyPacket`; the rest arrive with their milestones.

use commons::network::PacketWriter;

use crate::data::npc_data::NpcTemplate;
use crate::enums::NpcInfoType;
use crate::model::inventory::PaperdollSlot;
use crate::model::Player;
use crate::network::masks;

/// `ServerPackets` opcodes (the single-byte `_id1`).
pub mod opcodes {
    pub const DELETE_OBJECT: u8 = 0x08;
    pub const NPC_INFO: u8 = 0x0C;
    pub const NPC_HTML_MESSAGE: u8 = 0x19;
    pub const CHARACTER_SELECTION_INFO: u8 = 0x09;
    pub const LOGIN_FAIL: u8 = 0x0A;
    pub const CHAR_SELECTED: u8 = 0x0B;
    pub const NEW_CHARACTER_SUCCESS: u8 = 0x0D;
    pub const CHAR_CREATE_SUCCESS: u8 = 0x0F;
    pub const CHAR_CREATE_FAIL: u8 = 0x10;
    pub const CHAR_DELETE_SUCCESS: u8 = 0x1D;
    pub const CHAR_DELETE_FAIL: u8 = 0x1E;
    pub const VERSION_CHECK: u8 = 0x2E;
    pub const ACTION_FAIL: u8 = 0x1F;
    pub const TARGET_SELECTED: u8 = 0x23;
    pub const TARGET_UNSELECTED: u8 = 0x24;
    pub const MOVE_TO_LOCATION: u8 = 0x2F;
    pub const CHAR_INFO: u8 = 0x31;
    pub const STOP_MOVE: u8 = 0x47;
    pub const VALIDATE_LOCATION: u8 = 0x79;
    pub const STATUS_UPDATE: u8 = 0x18;
    pub const MAGIC_SKILL_USE: u8 = 0x48;
    pub const MAGIC_SKILL_CANCELED: u8 = 0x49;
    pub const MAGIC_SKILL_LAUNCHED: u8 = 0x54;
    pub const SYSTEM_MESSAGE: u8 = 0x62;
    pub const RESTART_RESPONSE: u8 = 0x71;
    pub const LOG_OUT_OK: u8 = 0x84;
    pub const SETUP_GAUGE: u8 = 0x6B;
    pub const SKILL_COOL_TIME: u8 = 0xC7;
    pub const ACQUIRE_SKILL_DONE: u8 = 0x94;
    pub const MY_TARGET_SELECTED: u8 = 0xB9;
    pub const DIE: u8 = 0x00;
    pub const REVIVE: u8 = 0x01;
    pub const TELEPORT_TO_LOCATION: u8 = 0x22;
    pub const AUTO_ATTACK_START: u8 = 0x25;
    pub const AUTO_ATTACK_STOP: u8 = 0x26;
    pub const SOCIAL_ACTION: u8 = 0x27;
    pub const CHANGE_MOVE_TYPE: u8 = 0x28;
    pub const ATTACK: u8 = 0x33;
    pub const MOVE_TO_PAWN: u8 = 0x72;
    pub const SHORT_CUT_REGISTER: u8 = 0x44;
    pub const SHORT_CUT_INIT: u8 = 0x45;
    pub const MACRO_LIST: u8 = 0xE8;
    pub const SAY2: u8 = 0x4A;
    pub const ASK_JOIN_PARTY: u8 = 0x39;
    pub const JOIN_PARTY: u8 = 0x3A;
    pub const PARTY_SMALL_WINDOW_ALL: u8 = 0x4E;
    pub const PARTY_SMALL_WINDOW_ADD: u8 = 0x4F;
    pub const PARTY_SMALL_WINDOW_DELETE_ALL: u8 = 0x50;
    pub const PARTY_SMALL_WINDOW_DELETE: u8 = 0x51;
    pub const PARTY_SMALL_WINDOW_UPDATE: u8 = 0x52;
    pub const PARTY_MEMBER_POSITION: u8 = 0xBA;
    pub const FRIEND_ADD_REQUEST_RESULT: u8 = 0x55;
    pub const FRIEND_REMOVE: u8 = 0x57;
    pub const FRIEND_STATUS: u8 = 0x59;
    pub const L2_FRIEND_LIST: u8 = 0x75;
    pub const L2_FRIEND_SAY: u8 = 0x78;
    pub const FRIEND_ADD_REQUEST: u8 = 0x83;
    pub const PLAY_SOUND: u8 = 0x9E;
    pub const QUEST_LIST: u8 = 0x86;
    pub const PLEDGE_SHOW_MEMBER_LIST_ALL: u8 = 0x5A;
    pub const PLEDGE_SHOW_MEMBER_LIST_UPDATE: u8 = 0x5B;
    pub const PLEDGE_SHOW_INFO_UPDATE: u8 = 0x8E;
    pub const DOOR_STATUS_UPDATE: u8 = 0x4D;
    pub const STATIC_OBJECT: u8 = 0x9F;
    pub const NPC_SAY: u8 = 0x30;

    /// Extended packets: opcode 0xFE + a 2-byte little-endian sub-opcode.
    pub const EX: u8 = 0xFE;
    pub const EX_IS_CHAR_NAME_CREATABLE: i16 = 0x10B;
    pub const EX_SEND_MANOR_LIST: i16 = 0x22;
    pub const EX_SHOW_CROP_INFO: i16 = 0x24;
    pub const EX_UI_SETTING: i16 = 0x71;
    pub const EX_ASK_MODIFY_PARTY_LOOTING: i16 = 0xC0;
    pub const EX_SET_PARTY_LOOTING: i16 = 0xC1;
    pub const EX_SHOW_QUEST_MARK: i16 = 0x21;
    pub const EX_NPC_QUEST_HTML_MESSAGE: i16 = 0x8E;
    pub const EX_QUEST_ITEM_LIST: i16 = 0xC7;
    pub const EX_SET_COMPASS_ZONE_CODE: i16 = 0x33;
}

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

/// Port of `serverpackets/PlaySound`'s quest-sound shape (`new
/// PlaySound(soundFile)`): every non-string field 0.
pub fn play_sound(sound_file: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PLAY_SOUND);
    w.write_i32(0); // 0 for quest sounds
    w.write_string(sound_file);
    w.write_i32(0); // 1 for ship sounds
    w.write_i32(0); // ship object id
    w.write_i32(0); // x
    w.write_i32(0); // y
    w.write_i32(0); // z
    w.write_i32(0);
    w.into_bytes()
}

/// The `QuestSound` file names this slice plays.
pub mod quest_sounds {
    pub const ACCEPT: &str = "ItemSound.quest_accept";
    pub const MIDDLE: &str = "ItemSound.quest_middle";
    pub const FINISH: &str = "ItemSound.quest_finish";
    pub const ITEMGET: &str = "ItemSound.quest_itemget";
}

/// Port of `serverpackets/ExShowQuestMark` — the on-screen quest marker,
/// sent after every cond change (Java `QuestState.setCond`).
pub fn ex_show_quest_mark(quest_id: i32, quest_state: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SHOW_QUEST_MARK);
    w.write_i32(quest_id);
    w.write_i32(quest_state);
    w.into_bytes()
}

/// Port of `serverpackets/NpcQuestHtmlMessage` — the quest-window variant of
/// `NpcHtmlMessage`, used for `.htm` results of quests with `0 < id < 20000`
/// (`Quest.showHtmlFile`'s `questwindow` branch).
pub fn ex_npc_quest_html_message(npc_object_id: i32, html: &str, quest_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_NPC_QUEST_HTML_MESSAGE);
    w.write_i32(npc_object_id);
    w.write_string(html);
    w.write_i32(quest_id);
    w.into_bytes()
}

/// Port of `serverpackets/DoorStatusUpdate`. `enemy` (siege-active doors)
/// and the HP-damage grade are always their idle values — no sieges, and
/// nothing damages doors yet.
pub fn door_status_update(door: &crate::model::door::Door, t: &crate::data::door_data::DoorTemplate, open: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::DOOR_STATUS_UPDATE);
    w.write_i32(door.object_id);
    w.write_i32(!open as i32); // "isClosed"
    w.write_i32(0); // damage grade (getDamage)
    w.write_i32(0); // isEnemy
    w.write_i32(door.door_id);
    w.write_i32(t.hp_max); // current HP (always full)
    w.write_i32(t.hp_max);
    w.into_bytes()
}

/// Port of `serverpackets/StaticObjectInfo`'s door constructor
/// (`type = 1`, mesh index 1 — `Door._meshindex` default; the GM
/// forced-targetable variant is not ported).
pub fn static_object_info_door(door: &crate::model::door::Door, t: &crate::data::door_data::DoorTemplate, open: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::STATIC_OBJECT);
    w.write_i32(door.door_id); // staticObjectId
    w.write_i32(door.object_id);
    w.write_i32(1); // type: door
    w.write_i32(t.targetable as i32);
    w.write_i32(1); // mesh index
    w.write_i32(!open as i32); // isClosed
    w.write_i32(0); // isEnemy
    w.write_i32(t.hp_max); // current HP
    w.write_i32(t.hp_max); // max HP
    w.write_i32(t.show_hp as i32);
    w.write_i32(0); // damage grade
    w.into_bytes()
}

/// Port of `serverpackets/NpcSay`'s npc-string shape (`new NpcSay(npc,
/// NPC_GENERAL, npcStringId)`): chat bubble over an NPC with a client-side
/// localized string (no parameters).
pub fn npc_say(npc_object_id: i32, npc_id: i32, npc_string_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::NPC_SAY);
    w.write_i32(npc_object_id);
    w.write_i32(22); // ChatType.NPC_GENERAL client id
    w.write_i32(1_000_000 + npc_id);
    w.write_i32(npc_string_id);
    w.into_bytes()
}

/// Port of `serverpackets/StaticObjectInfo`'s StaticObject constructor —
/// Java hardcodes type 0/targetable/mesh 0/no HP for the decoration kind
/// regardless of the template's `type` attribute.
pub fn static_object_info(static_id: i32, object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::STATIC_OBJECT);
    w.write_i32(static_id);
    w.write_i32(object_id);
    w.write_i32(0); // type
    w.write_i32(1); // targetable
    w.write_i32(0); // mesh index
    w.write_i32(0); // isClosed
    w.write_i32(0); // isEnemy
    w.write_i32(0); // current HP
    w.write_i32(0); // max HP
    w.write_i32(0); // showHp
    w.write_i32(0); // damage grade
    w.into_bytes()
}

/// `ExSetCompassZoneCode` values this slice can produce (Java declares
/// seven; the siege/PvP/altered codes wait for their zone types).
pub mod compass_zone {
    pub const PEACE: i32 = 0x0C;
    pub const GENERAL: i32 = 0x0F;
}

/// Port of `serverpackets/ExSetCompassZoneCode` — the client's compass zone
/// indicator (peace icon vs general), sent by `Player.revalidateZone` when
/// the code changes.
pub fn ex_set_compass_zone_code(code: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SET_COMPASS_ZONE_CODE);
    w.write_i32(code);
    w.into_bytes()
}

/// Port of `serverpackets/ExSendManorList` — the castles that have a manor.
/// TODO(G12): the real castle list from `CastleManager` (empty for now).
pub fn ex_send_manor_list() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SEND_MANOR_LIST);
    w.write_i32(0); // castle count
    w.into_bytes()
}

/// One crop-procurement line for [`ex_show_crop_info`], flattening the fields
/// Java reads from a `CropProcure` plus its resolved `Seed`. When the seed
/// can't be resolved Java writes `seed_level = 0` and both reward item ids as
/// `0`, so a caller with no seed data supplies those defaults directly.
pub struct CropInfoEntry {
    pub crop_id: i32,
    /// `CropProcure.getAmount` — remaining quantity the manor will still buy.
    pub amount: i64,
    /// `CropProcure.getStartAmount` — the quantity originally offered.
    pub start_amount: i64,
    pub price: i64,
    /// `CropProcure.getReward` — the reward-type id (0/1/2).
    pub reward: u8,
    /// `Seed.getLevel`, or 0 when the seed is unknown.
    pub seed_level: i32,
    /// `Seed.getReward(1)` / `getReward(2)` item ids, or 0 when unknown.
    pub reward1_item_id: i32,
    pub reward2_item_id: i32,
}

/// Port of `serverpackets/ExShowCropInfo` — the "Crop Sales" manor dialog a
/// castle owner opens through the chamberlain (`OnNpcManorBypass` request 4).
/// `crops = None` mirrors Java's `_crops == null` (next-period view while the
/// manor isn't yet approved): the header is written without a crop count.
///
/// TODO(manor): nothing sends this yet — the trigger (CastleChamberlain
/// `onNpcManorBypass` + castle ownership) and the crop data source
/// (`CastleManorManager.getCropProcure` / `Seed`) are unported, so callers
/// currently have no `CropInfoEntry` list to pass. Wire it once the manor
/// system lands; the serializer itself matches `writeImpl` byte-for-byte.
pub fn ex_show_crop_info(manor_id: i32, hide_buttons: bool, crops: Option<&[CropInfoEntry]>) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SHOW_CROP_INFO);
    w.write_u8(u8::from(hide_buttons)); // hide "Crop Sales" button
    w.write_i32(manor_id);
    w.write_i32(0);
    if let Some(crops) = crops {
        w.write_i32(crops.len() as i32);
        for crop in crops {
            w.write_i32(crop.crop_id);
            w.write_i64(crop.amount); // buy residual
            w.write_i64(crop.start_amount); // buy
            w.write_i64(crop.price); // buy price
            w.write_u8(crop.reward);
            w.write_i32(crop.seed_level);
            w.write_u8(1); // reward 1 present
            w.write_i32(crop.reward1_item_id);
            w.write_u8(1); // reward 2 present
            w.write_i32(crop.reward2_item_id);
        }
    }
    w.into_bytes()
}

/// Port of `serverpackets/settings/ExUISetting` — the player's stored UI key
/// mapping. TODO(G-later): load the stored mapping; null → length 0 for now.
pub fn ex_ui_setting() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_UI_SETTING);
    w.write_i32(0); // no stored key-mapping
    w.into_bytes()
}

/// Port of `serverpackets/ExIsCharNameCreatable`. `allowed` = -1 when the name
/// may be used; 1..5 is a `RequestCharacterNameCreatable` failure reason.
pub fn ex_is_char_name_creatable(allowed: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_IS_CHAR_NAME_CREATABLE);
    w.write_i32(allowed);
    w.into_bytes()
}

/// Port of `serverpackets/KeyPacket` — the reply to `ProtocolVersion`. Hands the
/// client the first 8 bytes of the cipher key and the crypt/server flags.
///
/// * `key8` — first 8 bytes of the 16-byte cipher key (the static tail is
///   hard-coded in the client).
/// * `result` — 1 = protocol ok, 0 = wrong protocol.
pub fn key_packet(key8: &[u8; 8], result: u8, packet_encryption: bool, server_id: i32, is_classic: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::VERSION_CHECK);
    w.write_u8(result); // 0 - wrong protocol, 1 - protocol ok
    for b in key8 {
        w.write_u8(*b);
    }
    w.write_i32(packet_encryption as i32); // use blowfish encryption
    w.write_i32(server_id);
    w.write_u8(1);
    w.write_i32(0); // obfuscation key
    w.write_u8(is_classic as u8);
    w.into_bytes()
}

/// Port of `serverpackets/LoginFail`. `LoginFail.LOGIN_SUCCESS` = `(-1, 0)`.
pub fn login_fail(success: i32, reason: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::LOGIN_FAIL);
    w.write_i32(success);
    w.write_i32(reason);
    w.into_bytes()
}

/// `LoginFail.LOGIN_SUCCESS`.
pub fn login_success() -> Vec<u8> {
    login_fail(-1, 0)
}

/// Java `ServerPacket.PAPERDOLL_ORDER` — the 33-int equipment write order the
/// client expects (the `InventorySlot` wire order mapped to paperdoll slots;
/// `RHand` repeats where the LRHAND display component sits).
pub const PAPERDOLL_ORDER: [PaperdollSlot; 33] = [
    PaperdollSlot::Under,
    PaperdollSlot::REar,
    PaperdollSlot::LEar,
    PaperdollSlot::Neck,
    PaperdollSlot::RFinger,
    PaperdollSlot::LFinger,
    PaperdollSlot::Head,
    PaperdollSlot::RHand,
    PaperdollSlot::LHand,
    PaperdollSlot::Gloves,
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Feet,
    PaperdollSlot::Cloak,
    PaperdollSlot::RHand,
    PaperdollSlot::Hair,
    PaperdollSlot::Hair2,
    PaperdollSlot::RBracelet,
    PaperdollSlot::LBracelet,
    PaperdollSlot::Deco1,
    PaperdollSlot::Deco2,
    PaperdollSlot::Deco3,
    PaperdollSlot::Deco4,
    PaperdollSlot::Deco5,
    PaperdollSlot::Deco6,
    PaperdollSlot::Belt,
    PaperdollSlot::Brooch,
    PaperdollSlot::BroochJewel1,
    PaperdollSlot::BroochJewel2,
    PaperdollSlot::BroochJewel3,
    PaperdollSlot::BroochJewel4,
    PaperdollSlot::BroochJewel5,
    PaperdollSlot::BroochJewel6,
];

/// `CharSelectionInfo.PAPERDOLL_ORDER_VISUAL_ID` — this packet overrides the
/// `ServerPacket` default with its own 9-slot visual-id order.
const CHAR_SELECT_PAPERDOLL_VISUAL_ORDER: [PaperdollSlot; 9] = [
    PaperdollSlot::RHand,
    PaperdollSlot::LHand,
    PaperdollSlot::Gloves,
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Feet,
    PaperdollSlot::RHand,
    PaperdollSlot::Hair,
    PaperdollSlot::Hair2,
];

/// The five armor slots whose enchant effect the lobby shows, in write order.
const CHAR_SELECT_ENCHANT_ORDER: [PaperdollSlot; 5] = [
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Head,
    PaperdollSlot::Gloves,
    PaperdollSlot::Feet,
];

/// The lobby slot to highlight: the most-recently-accessed character.
/// Characters marked for deletion never become active; if every character is
/// marked (or the list is empty), none is highlighted (-1).
fn lobby_active_id(chars: &[crate::character::CharData]) -> i32 {
    chars
        .iter()
        .enumerate()
        .filter(|(_, c)| c.delete_time == 0)
        .max_by_key(|(_, c)| c.last_access)
        .map(|(i, _)| i as i32)
        .unwrap_or(-1)
}

/// Port of `serverpackets/CharSelectionInfo`. Writes the real character rows
/// and paperdoll; augmentation/visual id are zero (later milestones).
pub fn char_selection_info(
    login_name: &str,
    session_id: i32,
    chars: &[crate::character::CharData],
    active_id: i32,
    max_characters: i32,
    exp: &crate::data::ExperienceData,
) -> Vec<u8> {
    let now = commons::util::now_millis();
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHARACTER_SELECTION_INFO);
    let size = chars.len() as i32;
    w.write_i32(size); // created character count
    w.write_i32(max_characters);
    w.write_u8((size == max_characters) as u8); // 1 = can't create new char
    w.write_u8(1); // 1 = can play free until level 85
    w.write_i32(2); // client region flag
    w.write_u8(0); // Balthus Knights / premium suggestion

    // If no active id was given, the most-recently-accessed character is active.
    let active_id = if active_id == -1 { lobby_active_id(chars) } else { active_id };

    for (i, c) in chars.iter().enumerate() {
        w.write_string(&c.name);
        w.write_i32(c.object_id);
        w.write_string(login_name);
        w.write_i32(session_id);
        w.write_i32(c.clan_id);
        w.write_i32(0); // builder level
        w.write_i32(c.sex);
        w.write_i32(c.race);
        w.write_i32(c.base_class_id);
        w.write_i32(1); // game server name
        w.write_i32(c.x);
        w.write_i32(c.y);
        w.write_i32(c.z);
        w.write_f64(c.cur_hp);
        w.write_f64(c.cur_mp);
        w.write_i64(c.sp);
        w.write_i64(c.exp);
        w.write_f64(exp_percent(exp, c.exp, c.level));
        w.write_i32(c.level);
        w.write_i32(c.reputation);
        w.write_i32(c.pk_kills);
        w.write_i32(c.pvp_kills);
        for _ in 0..9 {
            w.write_i32(0); // 7 reserved + 2 Ertheia
        }
        // Per-character paperdoll (Java `CharSelectInfoPackage`), read from the
        // `items` rows loaded alongside the character.
        let inv = crate::model::inventory::Inventory::from_rows(&c.items);
        for slot in PAPERDOLL_ORDER {
            let item_id = inv.paperdoll_item_id(slot);
            w.write_i32(item_id);
        }
        for slot in CHAR_SELECT_PAPERDOLL_VISUAL_ORDER {
            let visual_id = inv.paperdoll_visual_id(slot); // always 0 (appearance: later milestone)
            w.write_i32(visual_id);
        }
        for slot in CHAR_SELECT_ENCHANT_ORDER {
            let enchant = inv.paperdoll_enchant_level(slot);
            w.write_i16(enchant as i16);
        }
        w.write_i32(c.hair_style);
        w.write_i32(c.hair_color);
        w.write_i32(c.face);
        w.write_f64(c.max_hp as f64);
        w.write_f64(c.max_mp as f64);
        w.write_i32(if c.delete_time > 0 { ((c.delete_time - now) / 1000) as i32 } else { 0 });
        w.write_i32(c.class_id);
        w.write_i32((i as i32 == active_id) as i32);
        w.write_u8(inv.paperdoll_enchant_level(PaperdollSlot::RHand).min(127) as u8); // rhand weapon enchant (capped 127)
        w.write_i32(0); // augmentation option 1
        w.write_i32(0); // augmentation option 2
        w.write_i32(0); // transform
        w.write_i32(0); // pet npc id
        w.write_i32(0); // pet level
        w.write_i32(0); // pet food
        w.write_i32(0); // pet food level
        w.write_f64(0.0); // pet hp
        w.write_f64(0.0); // pet mp
        w.write_i32(c.vitality_points);
        w.write_i32(100); // vitality percent (RATE_VITALITY_EXP_MULTIPLIER * 100)
        w.write_i32(0); // remaining vitality item uses
        w.write_i32((c.access_level != -100) as i32); // char active
        w.write_u8(c.noble as u8);
        w.write_u8(0); // hero glow
        w.write_u8(1); // hair accessory enabled
    }
    w.into_bytes()
}

fn exp_percent(exp: &crate::data::ExperienceData, current_exp: i64, level: i32) -> f64 {
    let base = exp.exp_for_level(level);
    let next = exp.exp_for_level(level + 1);
    let denom = next - base;
    if denom <= 0 {
        0.0
    } else {
        (current_exp - base) as f64 / denom as f64
    }
}

/// `serverpackets/NewCharacterSuccess` — the base-stat table for the creation
/// screen (one entry per offered template).
pub fn new_character_success(templates: &[(i32, crate::enums::Race, &crate::data::player_template::PlayerTemplate)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::NEW_CHARACTER_SUCCESS);
    w.write_i32(templates.len() as i32);
    for (class_id, race, t) in templates {
        w.write_i32(race.ordinal());
        w.write_i32(*class_id);
        w.write_i32(99);
        w.write_i32(t.base_str);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_dex);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_con);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_int);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_wit);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_men);
        w.write_i32(1);
    }
    w.into_bytes()
}

/// `serverpackets/CharCreateOk`.
pub fn char_create_ok() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_CREATE_SUCCESS);
    w.write_i32(1);
    w.into_bytes()
}

/// `serverpackets/CharCreateFail` (`CharCreateFail.REASON_*`).
pub fn char_create_fail(reason: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_CREATE_FAIL);
    w.write_i32(reason);
    w.into_bytes()
}

/// `serverpackets/CharDeleteSuccess`.
pub fn char_delete_success() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_DELETE_SUCCESS);
    w.into_bytes()
}

/// `serverpackets/CharDeleteFail` (`CharacterDeleteFailType`).
pub fn char_delete_fail(reason: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_DELETE_FAIL);
    w.write_i32(reason);
    w.into_bytes()
}

/// Port of `serverpackets/CharSelected` — the reply to `CharacterSelect` that
/// starts the enter-world loading screen. `game_time` is minutes of the in-game
/// day (TODO(G4): real GameTimeTaskManager clock; 0 = midnight for now).
pub fn char_selected(v: &crate::model::PlayerView, session_id: i32, game_time: i32) -> Vec<u8> {
    let crate::model::PlayerView { p, pos, vitals, .. } = v;
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_SELECTED);
    w.write_string(&p.name);
    w.write_i32(p.object_id);
    w.write_string(&p.title);
    w.write_i32(session_id);
    w.write_i32(p.clan_id);
    w.write_i32(0);
    w.write_i32(p.is_female as i32);
    w.write_i32(p.race);
    w.write_i32(p.class_id);
    w.write_i32(1); // active
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    w.write_f64(vitals.cur_hp);
    w.write_f64(vitals.cur_mp);
    w.write_i64(p.sp);
    w.write_i64(p.exp);
    w.write_i32(p.level);
    w.write_i32(p.reputation);
    w.write_i32(p.pk_kills);
    w.write_i32(game_time % (24 * 60));
    w.write_i32(0);
    w.write_i32(p.class_id);
    w.write_bytes(&[0u8; 16]);
    for _ in 0..9 {
        w.write_i32(0);
    }
    w.write_bytes(&[0u8; 28]);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/RestartResponse` (`TRUE`/`FALSE` statics): whether
/// the `RequestRestart` was accepted — `true` sends the client back to the
/// character-selection screen (a `CharSelectionInfo` must follow).
pub fn restart_response(result: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::RESTART_RESPONSE);
    w.write_i32(result as i32);
    w.into_bytes()
}

/// Port of `serverpackets/LeaveWorld` (`LOG_OUT_OK`): the "safe to quit"
/// acknowledgement for the client's `Logout`; the server closes the connection
/// right after it.
pub fn leave_world() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::LOG_OUT_OK);
    w.into_bytes()
}

/// Java `StatusUpdateType` client ids used so far (grow as more stats need to
/// be pushed — regen, level-up, gear/buff changes, …).
pub mod status_update_type {
    pub const CUR_HP: u8 = 0x09;
    pub const MAX_HP: u8 = 0x0A;
    pub const CUR_MP: u8 = 0x0B;
    pub const MAX_MP: u8 = 0x0C;
    pub const CUR_CP: u8 = 0x21;
    pub const MAX_CP: u8 = 0x22;
}

/// Port of `serverpackets/StatusUpdate`. `updates` is a list of
/// `(StatusUpdateType client id, value)` pairs, in the order Java's
/// `LinkedHashMap` would iterate (insertion order). `isVisible`/caster id
/// (used to tell nearby players who's responsible for the change) are scoped
/// to "not visible" — self-only updates, no known-list broadcast yet (G7).
pub fn status_update(object_id: i32, updates: &[(u8, i32)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::STATUS_UPDATE);
    w.write_i32(object_id);
    w.write_i32(0); // caster id
    w.write_u8(0); // isVisible
    w.write_u8(updates.len() as u8);
    for &(kind, value) in updates {
        w.write_u8(kind);
        w.write_i32(value);
    }
    w.into_bytes()
}

/// Port of `serverpackets/ActionFailed.STATIC_PACKET` — the "request consumed"
/// terminator Java sends after (almost) every `Action`/movement request,
/// success or not. `castingType` is always 0 (no `SkillCastingType` bar
/// tracking yet).
pub fn action_failed() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::ACTION_FAIL);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/MyTargetSelected`, sent only to the selecting
/// player. `color` is `player.level - target.level` for auto-attackable
/// targets (tints the target bar by level gap), 0 otherwise.
pub fn my_target_selected(target_object_id: i32, color: i16) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MY_TARGET_SELECTED);
    w.write_i32(1); // Grand Crusade
    w.write_i32(target_object_id);
    w.write_i16(color);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/TargetSelected` — broadcast to other known players,
/// never to the selecting player themselves (they get `MyTargetSelected`).
pub fn target_selected(object_id: i32, target_object_id: i32, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TARGET_SELECTED);
    w.write_i32(object_id);
    w.write_i32(target_object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/TargetUnselected` — unlike `target_selected`,
/// Java broadcasts this with includeSelf=true, so the deselecting player
/// receives it too (the client needs it to drop its target UI).
pub fn target_unselected(object_id: i32, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TARGET_UNSELECTED);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/ValidateLocation` — the server's "you are actually
/// here" correction to a drifted client (`ValidatePosition` reply).
pub fn validate_location(object_id: i32, x: i32, y: i32, z: i32, heading: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::VALIDATE_LOCATION);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(heading);
    w.write_u8(0xff); // Java: trailing byte, "TODO: Find me!"
    w.into_bytes()
}

/// Port of `serverpackets/StopMove`.
pub fn stop_move(object_id: i32, x: i32, y: i32, z: i32, heading: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::STOP_MOVE);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(heading);
    w.into_bytes()
}

/// One rolled hit inside an `Attack` packet (Java `model/Hit`): flag bits
/// from `enums/AttackType` (miss 0x01 within flags... see `hit_flags`).
#[derive(Debug, Clone, Copy)]
pub struct AttackHit {
    pub target_object_id: i32,
    pub damage: i32,
    pub miss: bool,
    pub crit: bool,
}

/// Java `enums/AttackType` masks folded by `Hit`'s constructor: `MISSED` =
/// 0x01, `BLOCKED` = 0x02 (never set — no shield defence), `CRITICAL` = 0x04,
/// `SHOT_USED` = 0x08 (never — no soulshots).
fn hit_flags(hit: &AttackHit) -> i32 {
    if hit.miss {
        return 0x01;
    }
    if hit.crit {
        0x04
    } else {
        0
    }
}

/// Port of `serverpackets/Attack` (single-hit melee shape — the trailing
/// extra-hit list is empty, matching non-dual weapons).
pub fn attack(attacker_object_id: i32, hit: &AttackHit, ax: i32, ay: i32, az: i32, tx: i32, ty: i32, tz: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::ATTACK);
    w.write_i32(attacker_object_id);
    w.write_i32(hit.target_object_id);
    w.write_i32(0); // soulshot visual substitute (brooch jewels)
    w.write_i32(hit.damage);
    w.write_i32(hit_flags(hit));
    w.write_i32(0); // ss grade
    w.write_i32(ax);
    w.write_i32(ay);
    w.write_i32(az);
    w.write_i16(0); // no additional hits
    w.write_i32(tx);
    w.write_i32(ty);
    w.write_i32(tz);
    w.into_bytes()
}

/// Port of `serverpackets/Die` — broadcast on any creature's death. Every
/// revive-destination flag is written explicitly; for NPCs they are all
/// false. `to_village` = `canRevive() && !isPendingRevive()` for players.
pub fn die(object_id: i32, to_village: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::DIE);
    w.write_i32(object_id);
    w.write_i32(to_village as i32); // to village
    w.write_i32(0); // to clan hall
    w.write_i32(0); // to castle
    w.write_i32(0); // to outpost / siege HQ
    w.write_i32(0); // sweepable
    w.write_i32(0); // use feather
    w.write_i32(0); // to fortress
    w.write_i32(0); // disables feather button timer
    w.write_i32(0); // adventure's song
    w.write_u8(0); // hide die animation
    w.write_i32(0); // items enabled
    w.write_i32(0); // item count
    w.into_bytes()
}

/// Port of `serverpackets/Revive`.
pub fn revive(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::REVIVE);
    w.write_i32(object_id);
    w.into_bytes()
}

/// Port of `serverpackets/AutoAttackStart` — combat stance begins.
pub fn auto_attack_start(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::AUTO_ATTACK_START);
    w.write_i32(object_id);
    w.into_bytes()
}

/// Port of `serverpackets/AutoAttackStop` — combat stance ends (15 s after
/// the last swing, `AttackStanceTaskManager.COMBAT_TIME`).
pub fn auto_attack_stop(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::AUTO_ATTACK_STOP);
    w.write_i32(object_id);
    w.into_bytes()
}

/// Port of `serverpackets/SocialAction` (also carries the level-up effect,
/// `SocialAction.LEVEL_UP` = 2122).
pub const SOCIAL_ACTION_LEVEL_UP: i32 = 2122;
pub fn social_action(object_id: i32, action_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SOCIAL_ACTION);
    w.write_i32(object_id);
    w.write_i32(action_id);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/ChangeMoveType` — walk/run toggle broadcast (Java
/// `Creature.setRunning`/`setWalking`).
pub fn change_move_type(object_id: i32, running: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHANGE_MOVE_TYPE);
    w.write_i32(object_id);
    w.write_i32(if running { 1 } else { 0 });
    w.write_i32(0); // c2
    w.into_bytes()
}

/// Port of `serverpackets/MoveToPawn` — "walk toward that creature, stopping
/// at `distance`" (chasing/follow movement; plain destination moves use
/// `MoveToLocation`).
pub fn move_to_pawn(object_id: i32, target_object_id: i32, distance: i32, x: i32, y: i32, z: i32, tx: i32, ty: i32, tz: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MOVE_TO_PAWN);
    w.write_i32(object_id);
    w.write_i32(target_object_id);
    w.write_i32(distance);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(tx);
    w.write_i32(ty);
    w.write_i32(tz);
    w.into_bytes()
}

/// Port of `serverpackets/TeleportToLocation` (fade-style, like Java's
/// constant 0).
pub fn teleport_to_location(object_id: i32, x: i32, y: i32, z: i32, heading: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TELEPORT_TO_LOCATION);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(0); // fade 0, instant 1
    w.write_i32(heading);
    w.write_i32(0); // unknown
    w.into_bytes()
}

/// Port of `serverpackets/MoveToLocation` with an explicit destination —
/// unlike `enter_world::move_to_location` (which always sends dest==current
/// for the enter-world burst), this is the real move-start packet, broadcast
/// once to the mover *and* other known players (the client only starts
/// walking on the server's confirmation; Java's `Player.broadcastPacket`
/// includes self).
pub fn move_to_location(object_id: i32, dest_x: i32, dest_y: i32, dest_z: i32, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MOVE_TO_LOCATION);
    w.write_i32(object_id);
    w.write_i32(dest_x);
    w.write_i32(dest_y);
    w.write_i32(dest_z);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.into_bytes()
}

/// Port of `serverpackets/MagicSkillUse` (no ground-targeted skills, no
/// `RequestActionUse` action id yet). `casting_bar_id` is
/// `SkillCastingType.NORMAL`'s client bar id (0). `hit_time` is the
/// client-displayed cast time (`_hitTime + _cancelTime`). Self-casts pass the
/// caster as `target`. `reuse_group` is the skill's `reuseDelayGroup` — -1
/// when ungrouped (the client greys *every* icon on 0, Java's constructor
/// default is -1).
pub fn magic_skill_use(
    caster: &Player,
    caster_pos: &crate::model::components::Position,
    target: (i32, i32, i32, i32), // (object_id, x, y, z) — player or NPC
    skill_id: i32,
    skill_level: i32,
    hit_time: i32,
    reuse_group: i32,
    reuse_delay: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MAGIC_SKILL_USE);
    w.write_i32(0); // casting bar: NORMAL
    w.write_i32(caster.object_id);
    w.write_i32(target.0);
    w.write_i32(skill_id);
    w.write_i32(skill_level);
    w.write_i32(hit_time);
    w.write_i32(reuse_group);
    w.write_i32(reuse_delay);
    w.write_i32(caster_pos.x);
    w.write_i32(caster_pos.y);
    w.write_i32(caster_pos.z);
    w.write_i16(0); // isGroundTargetSkill
    w.write_i16(0); // no ground location
    w.write_i32(target.1);
    w.write_i32(target.2);
    w.write_i32(target.3);
    w.write_i32(0); // actionId used
    w.write_i32(0); // actionId
    w.into_bytes()
}

/// Port of `serverpackets/MagicSkillLaunched`: the launch flourish, broadcast
/// with the resolved target list (`SkillCaster._targets`).
pub fn magic_skill_launched(caster_object_id: i32, skill_id: i32, skill_level: i32, targets: &[i32]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MAGIC_SKILL_LAUNCHED);
    w.write_i32(0); // casting bar: NORMAL
    w.write_i32(caster_object_id);
    w.write_i32(skill_id);
    w.write_i32(skill_level);
    w.write_i32(targets.len() as i32);
    for &t in targets {
        w.write_i32(t);
    }
    w.into_bytes()
}

/// Port of `serverpackets/MagicSkillCanceled` — stops the cast animation
/// client-side. Broadcast (self included) by `stopCasting(aborted == true)`;
/// never sent on a quiet stop (finish-phase failures, natural end).
pub fn magic_skill_canceld(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MAGIC_SKILL_CANCELED);
    w.write_i32(object_id);
    w.into_bytes()
}

/// Port of `serverpackets/SetupGauge` (the cast-bar packet). `color`: 0 = blue.
pub fn setup_gauge(object_id: i32, color: i32, time_ms: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SETUP_GAUGE);
    w.write_i32(object_id);
    w.write_i32(color);
    w.write_i32(time_ms);
    w.write_i32(time_ms);
    w.into_bytes()
}

/// The `SystemMessageId` constants the cast pipeline sends (Java's enum has
/// ~6800 — added as handlers need them; the zero-parameter welcome message
/// keeps using `enter_world::system_message`).
pub mod sm_ids {
    pub const YOU_MAY_CREATE_UP_TO_48_MACROS: i16 = 797;
    pub const INVALID_MACRO_REFER_TO_THE_HELP_FILE_FOR_INSTRUCTIONS: i16 = 810;
    pub const MACRO_DESCRIPTIONS_MAY_CONTAIN_UP_TO_32_CHARACTERS: i16 = 837;
    pub const ENTER_THE_NAME_OF_THE_MACRO: i16 = 838;
    // Clans (G11)
    pub const S1_ALREADY_EXISTS: i16 = 5;
    pub const YOUR_CLAN_HAS_BEEN_CREATED: i16 = 189;
    pub const YOU_HAVE_FAILED_TO_CREATE_A_CLAN: i16 = 190;
    pub const YOU_DO_NOT_MEET_THE_CRITERIA_IN_ORDER_TO_CREATE_A_CLAN: i16 = 229;
    pub const YOU_MUST_WAIT_10_DAYS_BEFORE_CREATING_A_NEW_CLAN: i16 = 230;
    pub const CLAN_NAME_IS_INVALID: i16 = 261;
    pub const CLAN_NAME_S_LENGTH_IS_INCORRECT: i16 = 262;
    // Quests (G11): "earned" for quest gives, vs the loot "obtained" trio.
    pub const YOU_HAVE_EARNED_S1_ADENA: i16 = 52;
    pub const YOU_HAVE_EARNED_S2_S1_S: i16 = 53;
    pub const YOU_HAVE_EARNED_S1: i16 = 54;
    pub const YOU_HAVE_OBTAINED_S1_ADENA: i16 = 28;
    pub const YOU_HAVE_OBTAINED_S2_S1: i16 = 29;
    pub const YOU_HAVE_OBTAINED_S1: i16 = 30;
    // Item use (G13): `ExtractableItems` (pack/box unpacking).
    pub const THERE_WAS_NOTHING_FOUND_INSIDE: i16 = 1669;
    pub const YOUR_INVENTORY_IS_FULL: i16 = 129;
    pub const YOU_HAVE_AVOIDED_C1_S_ATTACK: i16 = 42;
    pub const YOU_HAVE_MISSED: i16 = 43;
    pub const CRITICAL_HIT: i16 = 44;
    pub const YOUR_LEVEL_HAS_INCREASED: i16 = 96;
    pub const YOU_HAVE_ACQUIRED_S1_SP: i16 = 331;
    pub const YOUR_SP_HAS_DECREASED_BY_S1: i16 = 538;
    pub const YOUR_XP_HAS_DECREASED_BY_S1: i16 = 539;
    pub const C1_HAS_EVADED_C2_S_ATTACK: i16 = 2264;
    pub const C1_S_ATTACK_WENT_ASTRAY: i16 = 2265;
    pub const C1_LANDED_A_CRITICAL_HIT: i16 = 2266;
    pub const YOU_HAVE_ACQUIRED_S1_XP_BONUS_S2_AND_S3_SP_BONUS_S4: i16 = 3259;
    pub const NOT_ENOUGH_HP: i16 = 23;
    pub const NOT_ENOUGH_MP: i16 = 24;
    pub const YOUR_CASTING_HAS_BEEN_INTERRUPTED: i16 = 27;
    pub const YOU_USE_S1: i16 = 46;
    pub const S1_IS_NOT_AVAILABLE_REUSE: i16 = 48;
    pub const INVALID_TARGET: i16 = 109;
    pub const CANNOT_SEE_TARGET: i16 = 181;
    // Zones (G12)
    pub const YOU_MAY_NOT_ATTACK_THIS_TARGET_IN_A_PEACEFUL_ZONE: i16 = 85;
    pub const YOU_CANNOT_USE_SKILLS_THAT_MAY_HARM_OTHER_PLAYERS_IN_HERE: i16 = 2167;
    // Shop (G12)
    pub const YOU_DO_NOT_HAVE_ENOUGH_ADENA: i16 = 279;
    pub const YOU_HAVE_EXCEEDED_THE_QUANTITY_THAT_CAN_BE_INPUTTED: i16 = 1036;
    pub const EXCHANGE_IS_SUCCESSFUL: i16 = 4358;
    pub const DISTANCE_TOO_FAR_CASTING_CANCELLED: i16 = 748;
    pub const S1_HP_HAS_BEEN_RESTORED: i16 = 1066;
    pub const S2_HP_HAS_BEEN_RESTORED_BY_C1: i16 = 1067;
    pub const M_CRITICAL: i16 = 1280;
    pub const C1_HAS_INFLICTED_S3_DAMAGE_ON_C2: i16 = 2261;
    pub const C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2: i16 = 2262;
    pub const S2_SECONDS_REMAINING_FOR_REUSE: i16 = 2303;
    pub const S2_MINUTES_S3_SECONDS_REMAINING_FOR_REUSE: i16 = 2304;
    pub const S2_HOURS_S3_MINUTES_S4_SECONDS_REMAINING_FOR_REUSE: i16 = 2305;
    // Party (G10)
    pub const C1_HAS_BEEN_INVITED_TO_THE_PARTY: i16 = 105;
    pub const YOU_HAVE_JOINED_S1_S_PARTY: i16 = 106;
    pub const C1_HAS_JOINED_THE_PARTY: i16 = 107;
    pub const C1_HAS_LEFT_THE_PARTY: i16 = 108;
    pub const YOU_HAVE_INVITED_THE_WRONG_TARGET: i16 = 152;
    pub const C1_IS_ON_ANOTHER_TASK_PLEASE_TRY_AGAIN_LATER: i16 = 153;
    pub const ONLY_THE_LEADER_CAN_GIVE_OUT_INVITATIONS: i16 = 154;
    pub const THE_PARTY_IS_FULL: i16 = 155;
    pub const C1_IS_A_MEMBER_OF_ANOTHER_PARTY_AND_CANNOT_BE_INVITED: i16 = 160;
    pub const WAITING_FOR_ANOTHER_REPLY: i16 = 164;
    pub const YOU_MUST_FIRST_SELECT_A_USER_TO_INVITE_TO_YOUR_PARTY: i16 = 185;
    pub const YOU_HAVE_WITHDRAWN_FROM_THE_PARTY: i16 = 200;
    pub const C1_WAS_EXPELLED_FROM_THE_PARTY: i16 = 201;
    pub const YOU_HAVE_BEEN_EXPELLED_FROM_THE_PARTY: i16 = 202;
    pub const THE_PARTY_HAS_DISPERSED: i16 = 203;
    pub const C1_HAS_OBTAINED_S3_S2: i16 = 299;
    pub const C1_HAS_OBTAINED_S2: i16 = 300;
    pub const THE_PLAYER_DECLINED_TO_JOIN_YOUR_PARTY: i16 = 305;
    pub const C1_HAS_BECOME_THE_PARTY_LEADER: i16 = 1384;
    pub const SLOW_DOWN_YOU_ARE_ALREADY_THE_PARTY_LEADER: i16 = 1401;
    pub const YOU_MAY_ONLY_TRANSFER_PARTY_LEADERSHIP: i16 = 1402;
    pub const REQUESTING_APPROVAL_FOR_CHANGING_PARTY_LOOT_TO_S1: i16 = 3135;
    pub const PARTY_LOOT_CHANGE_WAS_CANCELLED: i16 = 3137;
    pub const PARTY_LOOT_WAS_CHANGED_TO_S1: i16 = 3138;
    pub const C1_IS_SET_TO_REFUSE_PARTY_REQUESTS: i16 = 3168;
    // Friends (G10)
    pub const S1_HAS_BEEN_ADDED_TO_YOUR_FRIENDS_LIST: i16 = 132;
    pub const YOU_CANNOT_ADD_YOURSELF_TO_YOUR_OWN_FRIEND_LIST: i16 = 165;
    pub const C1_IS_ALREADY_ON_YOUR_FRIEND_LIST: i16 = 167;
    pub const FRIEND_INVITE_TARGET_NOT_FOUND: i16 = 170;
    pub const C1_IS_NOT_ON_YOUR_FRIEND_LIST: i16 = 171;
    pub const S1_HAS_BEEN_ADDED_TO_YOUR_FRIENDS_LIST_2: i16 = 479;
    pub const S1_HAS_BEEN_REMOVED_FROM_YOUR_FRIENDS_LIST_2: i16 = 481;
    pub const THIS_PLAYER_IS_ALREADY_REGISTERED_ON_YOUR_FRIENDS_LIST: i16 = 484;
    pub const FRIENDS_LIST_HEADER: i16 = 487;
    pub const S1_CURRENTLY_ONLINE: i16 = 488;
    pub const S1_CURRENTLY_OFFLINE: i16 = 489;
    pub const FRIENDS_LIST_FOOTER: i16 = 490;
    pub const YOUR_FRIEND_S1_JUST_LOGGED_IN: i16 = 503;
    pub const FRIEND_ADDED_SUCCESSFULLY: i16 = 525;
    pub const YOU_HAVE_FAILED_TO_ADD_A_FRIEND: i16 = 526;
    pub const YOU_VE_REQUESTED_C1_TO_BE_ON_YOUR_FRIENDS_LIST: i16 = 2911;
    // Chat (G10)
    pub const THAT_PLAYER_IS_NOT_ONLINE: i16 = 145;
    pub const KEYBOARD_INPUT_SPAM_WARNING: i16 = 1078;
    pub const YOU_ARE_NOT_IN_A_PARTY: i16 = 4201;
    pub const YOU_ARE_NOT_IN_A_CLAN: i16 = 4202;
    pub const YOU_ARE_NOT_IN_AN_ALLIANCE: i16 = 4203;
}

/// One `SystemMessage` parameter (Java `SystemMessage.SMParam`), scoped to the
/// types the cast pipeline emits.
pub enum SmParam {
    /// `TYPE_TEXT` (0) — `addString`.
    Text(String),
    /// `TYPE_INT_NUMBER` (1) — `addInt`.
    Int(i32),
    /// `TYPE_SKILL_NAME` (4) — `addSkillName` (id, level, sub-level 0).
    SkillName { id: i32, level: i32 },
    /// `TYPE_NPC_NAME` (2) — `addNpcName` (template id + 1000000).
    NpcName(i32),
    /// `TYPE_ITEM_NAME` (3) — `addItemName`.
    ItemName(i32),
    /// `TYPE_LONG_NUMBER` (6) — `addLong`.
    Long(i64),
    /// `TYPE_PLAYER_NAME` (12) — `addPcName`.
    PlayerName(String),
    /// `TYPE_SYSTEM_STRING` (13) — `addSystemString` (sysstring-e.dat id).
    SysString(i32),
}

/// Port of `serverpackets/SystemMessage.writeImpl` (localisation branch
/// skipped — `MULTILANG_ENABLE` is off by default): message id, parameter
/// count, then each parameter as a type byte + payload.
pub fn system_message_with(message_id: i16, params: &[SmParam]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SYSTEM_MESSAGE);
    w.write_i16(message_id);
    w.write_u8(params.len() as u8);
    for param in params {
        match param {
            SmParam::Text(s) => {
                w.write_u8(0);
                w.write_string(s);
            }
            SmParam::Int(v) => {
                w.write_u8(1);
                w.write_i32(*v);
            }
            SmParam::SkillName { id, level } => {
                w.write_u8(4);
                w.write_i32(*id);
                w.write_i16(*level as i16);
                w.write_i16(0); // sub-level
            }
            SmParam::NpcName(template_id) => {
                w.write_u8(2);
                w.write_i32(1_000_000 + *template_id);
            }
            SmParam::ItemName(item_id) => {
                w.write_u8(3);
                w.write_i32(*item_id);
            }
            SmParam::Long(v) => {
                w.write_u8(6);
                w.write_i64(*v);
            }
            SmParam::PlayerName(s) => {
                w.write_u8(12);
                w.write_string(s);
            }
            SmParam::SysString(id) => {
                w.write_u8(13);
                w.write_i32(*id);
            }
        }
    }
    w.into_bytes()
}

/// Port of `serverpackets/SkillCoolTime`: every skill still on reuse
/// (`Player.reuses` entries with time remaining), total and remaining in
/// whole seconds. The id written is the map key — the shared reuse group when
/// the skill has one, else the skill id (Java writes
/// `sharedReuseGroup > 0 ? group : skillId`). Sent on enter-world and on
/// `RequestSkillCoolTime`.
pub fn skill_cool_time(reuses: &crate::model::components::Reuses, now_tick: u64) -> Vec<u8> {
    let entries: Vec<(i32, i32, i32, i32)> = reuses
        .0
        .iter()
        .filter_map(|(&reuse_key, r)| {
            let remaining_ticks = r.until_tick.checked_sub(now_tick)?;
            if remaining_ticks == 0 {
                return None;
            }
            Some((reuse_key, r.skill_level, r.total_ms / 1000, (remaining_ticks / 10) as i32))
        })
        .collect();
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SKILL_COOL_TIME);
    w.write_i32(entries.len() as i32);
    for (skill_id, level, total_secs, remaining_secs) in entries {
        w.write_i32(skill_id);
        w.write_i32(level);
        w.write_i32(total_secs);
        w.write_i32(remaining_secs);
    }
    w.into_bytes()
}

/// Port of `serverpackets/AcquireSkillDone` — no body.
pub fn acquire_skill_done() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::ACQUIRE_SKILL_DONE);
    w.into_bytes()
}

/// Port of `serverpackets/DeleteObject` — removes an object from the client's
/// screen when it leaves the observer's known area.
pub fn delete_object(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::DELETE_OBJECT);
    w.write_i32(object_id);
    w.write_u8(0); // c2
    w.into_bytes()
}

/// `CharInfo.PAPERDOLL_ORDER` — the 12-slot equipment view other clients get
/// (a subset of the 33-slot `UserInfo` order; `RHand` repeats for LRHAND).
const CHAR_INFO_PAPERDOLL_ORDER: [PaperdollSlot; 12] = [
    PaperdollSlot::Under,
    PaperdollSlot::Head,
    PaperdollSlot::RHand,
    PaperdollSlot::LHand,
    PaperdollSlot::Gloves,
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Feet,
    PaperdollSlot::Cloak,
    PaperdollSlot::RHand,
    PaperdollSlot::Hair,
    PaperdollSlot::Hair2,
];

/// `ServerPacket.PAPERDOLL_ORDER_AUGMENT`.
const CHAR_INFO_PAPERDOLL_ORDER_AUGMENT: [PaperdollSlot; 3] =
    [PaperdollSlot::RHand, PaperdollSlot::LHand, PaperdollSlot::RHand];

/// `ServerPacket.PAPERDOLL_ORDER_VISUAL_ID`.
const CHAR_INFO_PAPERDOLL_ORDER_VISUAL_ID: [PaperdollSlot; 9] = [
    PaperdollSlot::RHand,
    PaperdollSlot::LHand,
    PaperdollSlot::RHand,
    PaperdollSlot::Gloves,
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Feet,
    PaperdollSlot::Hair,
    PaperdollSlot::Hair2,
];

/// Port of `serverpackets/CharInfo` — how this player appears on *other*
/// players' clients (the counterpart of `UserInfo` for the owner). Values for
/// systems not yet modeled (clan, mounts, stores, cubics, fishing, abnormal
/// visual effects…) are their empty Java defaults; the vehicle branch and the
/// GM-sees-invisible variant are skipped (no boats/GM model).
pub fn char_info(v: &crate::model::PlayerView) -> Vec<u8> {
    let crate::model::PlayerView { p, pos, vitals, pvitals, speeds, collision, combat, inventory, .. } = v;
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_INFO);
    w.write_u8(0); // Grand Crusade
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    w.write_i32(0); // vehicle object id
    w.write_i32(p.object_id);
    w.write_string(&p.name);
    w.write_i16(p.race as i16);
    w.write_u8(p.is_female as u8);
    w.write_i32(p.base_class_id); // root class id

    for slot in CHAR_INFO_PAPERDOLL_ORDER {
        let item_id = inventory.paperdoll_item_id(slot);
        w.write_i32(item_id);
    }
    for slot in CHAR_INFO_PAPERDOLL_ORDER_AUGMENT {
        let augment = inventory.paperdoll_augmentation(slot);
        w.write_i32(augment.map_or(0, |a| a.0));
        w.write_i32(augment.map_or(0, |a| a.1));
    }
    w.write_u8(0); // armor min enchant
    for slot in CHAR_INFO_PAPERDOLL_ORDER_VISUAL_ID {
        let visual_id = inventory.paperdoll_visual_id(slot);
        w.write_i32(visual_id);
    }

    w.write_u8(0); // pvp flag
    w.write_i32(p.reputation);
    w.write_i32(combat.m_atk_spd);
    w.write_i32(combat.p_atk_spd);
    w.write_i16(speeds.run_spd as i16);
    w.write_i16(speeds.walk_spd as i16);
    w.write_i16(speeds.swim_run_spd as i16);
    w.write_i16(speeds.swim_walk_spd as i16);
    w.write_i16(0); // fly run
    w.write_i16(0); // fly walk
    w.write_i16(0); // fly run (repeat)
    w.write_i16(0); // fly walk (repeat)
    w.write_f64(speeds.move_multiplier);
    w.write_f64(1.0); // attack speed multiplier
    w.write_f64(collision.radius);
    w.write_f64(collision.height);
    w.write_i32(p.hair_style); // visual hair
    w.write_i32(p.hair_color);
    w.write_i32(p.face);
    w.write_string(&p.title);
    w.write_i32(p.clan_id);
    w.write_i32(0); // clan crest id
    w.write_i32(0); // ally id
    w.write_i32(0); // ally crest id
    w.write_u8(1); // !isSitting — standing
    w.write_u8(speeds.running as u8);
    w.write_u8(0); // in combat
    w.write_u8(0); // alike dead
    w.write_u8(0); // invisible
    w.write_u8(0); // mount type
    w.write_u8(0); // private store type
    w.write_i16(0); // cubic count (+ cubic ids)
    w.write_u8(0); // in matching room
    w.write_u8(0); // 1 water, 2 flying mount
    w.write_i16(0); // recom have
    w.write_i32(0); // mount npc id
    w.write_i32(p.class_id);
    w.write_i32(0); // TODO: Find me! (Java unknown)
    w.write_u8(inventory.paperdoll_enchant_level(PaperdollSlot::RHand) as u8); // weapon enchant
    w.write_u8(0); // team id
    w.write_i32(0); // clan crest large id
    w.write_u8(0); // noble
    w.write_u8(0); // hero
    w.write_u8(0); // fishing
    w.write_i32(0); // bait x
    w.write_i32(0); // bait y
    w.write_i32(0); // bait z
    w.write_i32(0xFFFFFF); // name color
    w.write_i32(pos.heading);
    w.write_u8(0); // pledge class
    w.write_i16(0); // pledge type
    w.write_i32(0xFFFF77); // title color
    w.write_u8(0); // cursed weapon level
    w.write_i32(0); // clan reputation score
    w.write_i32(0); // transformation display id
    w.write_i32(0); // agathion id
    w.write_u8(0); // nPvPRestrainStatus
    w.write_i32(pvitals.cur_cp.round() as i32);
    w.write_i32(vitals.max_hp);
    w.write_i32(vitals.cur_hp.round() as i32);
    w.write_i32(vitals.max_mp);
    w.write_i32(vitals.cur_mp.round() as i32);
    w.write_u8(0); // cBRLectureMark
    w.write_i32(0); // abnormal visual effect count (+ short ids)
    w.write_u8(0); // true hero (100 when true)
    w.write_u8(1); // hair accessory enabled
    w.write_u8(0); // used ability points
    w.into_bytes()
}

/// Port of `serverpackets/NpcInfo` (masked, 5 mask bytes / "mask_bits_37").
/// Component selection follows the Java constructor with the not-yet-modeled
/// state at its defaults: no summon animation, no water/fly/team/enchant/
/// clone/transform/abnormals, no clan, reputation 0, pvp flag 0. The
/// localisation pass (`MULTILANG_ENABLE`) is skipped.
pub fn npc_info(v: &crate::model::npc::NpcView, t: &NpcTemplate) -> Vec<u8> {
    let crate::model::npc::NpcView { npc, pos, vitals, speeds } = v;
    use NpcInfoType as T;

    // Java `NpcInfo._masks` starts with the two unnamed always-on component
    // pairs (0x0C/0x0D and 0x14/0x15) pre-set.
    let mut mask_bytes: [u8; 5] = [0x00, 0x0C, 0x0C, 0x00, 0x00];
    let mut init_size: i32 = 0;
    let mut block_size: i32 = 0;
    let mut add = |mask_bytes: &mut [u8; 5], ty: T| {
        masks::add_mask(mask_bytes, ty.mask());
        // `calcBlockSize`: ATTACKABLE/RELATIONS/TITLE go in block 1, the rest
        // in block 2; the string components add their chars on top.
        match ty {
            T::Attackable | T::Relations => init_size += ty.block_length(),
            T::Title => init_size += ty.block_length() + t.title.len() as i32 * 2,
            T::Name => block_size += ty.block_length() + t.name.len() as i32 * 2,
            _ => block_size += ty.block_length(),
        }
    };

    add(&mut mask_bytes, T::Attackable);
    add(&mut mask_bytes, T::Relations);
    add(&mut mask_bytes, T::Id);
    add(&mut mask_bytes, T::Position);
    add(&mut mask_bytes, T::StopMode);
    add(&mut mask_bytes, T::MoveMode);
    if pos.heading > 0 {
        add(&mut mask_bytes, T::Heading);
    }
    if t.base_p_atk_spd > 0 || t.base_m_atk_spd > 0 {
        add(&mut mask_bytes, T::AtkCastSpeed);
    }
    if t.base_run_spd > 0.0 {
        add(&mut mask_bytes, T::SpeedMultiplier);
    }
    if t.rhand > 0 || t.lhand > 0 {
        add(&mut mask_bytes, T::Equipped);
    }
    if vitals.max_hp > 0 {
        add(&mut mask_bytes, T::MaxHp);
    }
    if vitals.max_mp > 0 {
        add(&mut mask_bytes, T::MaxMp);
    }
    if vitals.cur_hp <= vitals.max_hp as f64 {
        add(&mut mask_bytes, T::CurrentHp);
    }
    if vitals.cur_mp <= vitals.max_mp as f64 {
        add(&mut mask_bytes, T::CurrentMp);
    }
    if t.server_side_name {
        add(&mut mask_bytes, T::Name);
    }
    if t.server_side_title {
        add(&mut mask_bytes, T::Title);
    }
    add(&mut mask_bytes, T::PetEvolutionId);
    // Status mask: 0x01 in combat, 0x02 dead, 0x04 targetable, 0x08 show name.
    let mut status_mask = 0u8;
    if t.targetable {
        status_mask |= 0x04;
    }
    if t.show_name {
        status_mask |= 0x08;
    }
    if status_mask != 0 {
        add(&mut mask_bytes, T::VisualState);
    }

    let contains = |ty: T| masks::contains_mask(&mask_bytes, ty.mask());

    let mut w = PacketWriter::new();
    w.write_u8(opcodes::NPC_INFO);
    w.write_i32(npc.object_id);
    w.write_u8(0); // 0=teleported 1=default 2=summoned
    w.write_i16(37); // mask_bits_37
    w.write_bytes(&mask_bytes);

    // Block 1.
    w.write_u8(init_size as u8);
    w.write_u8(u8::from(t.is_attackable_class() && t.type_name != "Guard"));
    w.write_i32(0); // relations
    if contains(T::Title) {
        w.write_string(&t.title);
    }

    // Block 2.
    w.write_i16(block_size as i16);
    w.write_i32(t.display_id + 1_000_000);
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    if contains(T::Heading) {
        w.write_i32(pos.heading);
    }
    if contains(T::AtkCastSpeed) {
        w.write_i32(t.base_p_atk_spd);
        w.write_i32(t.base_m_atk_spd);
    }
    if contains(T::SpeedMultiplier) {
        // Current speed / template base speed — 1.0 until buffs/AI exist.
        w.write_f32(1.0); // movement speed multiplier
        w.write_f32(1.0); // attack speed multiplier
    }
    if contains(T::Equipped) {
        w.write_i32(t.rhand);
        w.write_i32(0); // armor id (Java writes 0)
        w.write_i32(t.lhand);
    }
    w.write_u8(1); // STOP_MODE: !isDead
    w.write_u8(speeds.running as u8); // MOVE_MODE
    w.write_i32(0); // PET_EVOLUTION_ID
    if contains(T::CurrentHp) {
        w.write_i32(vitals.cur_hp as i32);
    }
    if contains(T::CurrentMp) {
        w.write_i32(vitals.cur_mp as i32);
    }
    if contains(T::MaxHp) {
        w.write_i32(vitals.max_hp);
    }
    if contains(T::MaxMp) {
        w.write_i32(vitals.max_mp);
    }
    if contains(T::Name) {
        w.write_string(&t.name);
    }
    if contains(T::VisualState) {
        w.write_u8(status_mask);
    }
    w.into_bytes()
}

/// Port of `serverpackets/NpcHtmlMessage` — the NPC dialog window. `item_id`
/// stays 0 (item-triggered dialogs aren't a thing yet).
pub fn npc_html_message(npc_object_id: i32, html: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::NPC_HTML_MESSAGE);
    w.write_i32(npc_object_id);
    w.write_string(html);
    w.write_i32(0); // item id
    w.write_i32(0); // show common board
    w.into_bytes()
}

/// The ITEM-arm tail shared by `ShortCutInit` and `ShortCutRegister`:
/// augmentation option 1/2 + visual id — all 0, our `ItemInstance` carries
/// neither (augmentation/appearance are later milestones; Java reads them
/// off the item and writes the same zeros for a plain one).
fn write_shortcut_item_tail(w: &mut PacketWriter) {
    w.write_i32(0); // augmentation option 1
    w.write_i32(0); // augmentation option 2
    w.write_i32(0); // visual id
}

/// Port of `serverpackets/ShortCutInit` — the full panel, sent on enter
/// world and after every deletion (there's no per-slot delete packet; Java
/// re-sends the whole panel).
pub fn shortcut_init(shortcuts: &crate::model::components::Shortcuts) -> Vec<u8> {
    use crate::model::shortcut::ShortcutType;
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SHORT_CUT_INIT);
    w.write_i32(shortcuts.0.len() as i32);
    for sc in shortcuts.0.values() {
        w.write_i32(sc.kind.ordinal());
        w.write_i32(sc.client_slot());
        match sc.kind {
            ShortcutType::Item => {
                w.write_i32(sc.id);
                w.write_i32(1); // enabled
                w.write_i32(sc.shared_reuse_group);
                w.write_i32(0);
                w.write_i32(0);
                write_shortcut_item_tail(&mut w);
            }
            ShortcutType::Skill => {
                w.write_i32(sc.id);
                w.write_i16(sc.level as i16);
                w.write_i16(0); // sub-level
                w.write_i32(sc.shared_reuse_group);
                w.write_u8(0); // C5
                w.write_i32(1); // C6
            }
            ShortcutType::Action | ShortcutType::Macro | ShortcutType::Recipe | ShortcutType::Bookmark => {
                w.write_i32(sc.id);
                w.write_i32(1); // C6
            }
            // Java's switch has no NONE arm — nothing more is written.
            ShortcutType::None => {}
        }
    }
    w.into_bytes()
}

/// Port of `serverpackets/ShortCutRegister` — the echo for one registered
/// slot.
pub fn shortcut_register(sc: &crate::model::shortcut::Shortcut) -> Vec<u8> {
    use crate::model::shortcut::ShortcutType;
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SHORT_CUT_REGISTER);
    w.write_i32(sc.kind.ordinal());
    w.write_i32(sc.client_slot());
    match sc.kind {
        ShortcutType::Item => {
            w.write_i32(sc.id);
            w.write_i32(sc.character_type);
            w.write_i32(sc.shared_reuse_group);
            w.write_i32(0); // unknown
            w.write_i32(0); // unknown
            write_shortcut_item_tail(&mut w);
        }
        ShortcutType::Skill => {
            w.write_i32(sc.id);
            w.write_i16(sc.level as i16);
            w.write_i16(0); // sub-level
            w.write_i32(sc.shared_reuse_group);
            w.write_u8(0); // C5
            w.write_i32(sc.character_type);
            w.write_i32(0); // if 1 - can't use
            w.write_i32(0); // reuse delay ?
        }
        ShortcutType::Action | ShortcutType::Macro | ShortcutType::Recipe | ShortcutType::Bookmark => {
            w.write_i32(sc.id);
            w.write_i32(sc.character_type);
        }
        ShortcutType::None => {}
    }
    w.into_bytes()
}

/// Port of `serverpackets/SendMacroList`. `count` is the header's macro
/// count: the running total for `List` bursts, 1 for `Add`/`Modify`, 0 for
/// `Delete` (matching the Java call sites). The body is omitted for `Delete`
/// and for the has-no-macros `List` (macro `None`).
pub fn send_macro_list(
    count: i32,
    macro_: Option<&crate::model::shortcut::Macro>,
    update: crate::model::shortcut::MacroUpdateType,
) -> Vec<u8> {
    use crate::model::shortcut::MacroUpdateType;
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MACRO_LIST);
    w.write_u8(update.id());
    // Modified, created or deleted macro's id; 0 for LIST entries.
    w.write_i32(if update != MacroUpdateType::List { macro_.map_or(0, |m| m.id) } else { 0 });
    w.write_u8(count as u8);
    w.write_u8(macro_.is_some() as u8);
    if let Some(m) = macro_ {
        if update != MacroUpdateType::Delete {
            w.write_i32(m.id);
            w.write_string(&m.name);
            w.write_string(&m.descr);
            w.write_string(&m.acronym);
            w.write_i32(m.icon);
            w.write_u8(m.commands.len() as u8);
            for (i, cmd) in m.commands.iter().enumerate() {
                w.write_u8((i + 1) as u8);
                w.write_u8(cmd.kind.ordinal() as u8);
                w.write_i32(cmd.d1);
                w.write_u8(cmd.d2 as u8);
                w.write_string(&cmd.cmd);
            }
        }
    }
    w.into_bytes()
}

/// `MacroList.sendAllMacros` — the enter-world macro burst: one packet per
/// macro carrying the total count, or a single empty LIST packet when the
/// player has none.
pub fn send_all_macros(macros: &crate::model::components::Macros) -> Vec<Vec<u8>> {
    use crate::model::shortcut::MacroUpdateType;
    if macros.entries.is_empty() {
        vec![send_macro_list(0, None, MacroUpdateType::List)]
    } else {
        let count = macros.entries.len() as i32;
        macros.entries.iter().map(|m| send_macro_list(count, Some(m), MacroUpdateType::List)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::lobby_active_id;
    use crate::character::CharData;
    use commons::network::PacketWriter;

    /// `NpcInfo` byte layout, hand-computed against the Java constructor +
    /// `writeImpl` (no client capture available for NPCs yet, unlike the
    /// UserInfo test — the mask math is shared with that byte-verified path).
    #[test]
    fn npc_info_layout_matches_java() {
        let mut t = crate::data::npc_data::default_template(30001);
        t.name = "Gina".into();
        t.server_side_name = true;
        t.level = 5;
        t.base_hp_max = 100.0;
        t.base_mp_max = 50.0;
        // Defaults keep: p_atk_spd 300, m_atk_spd 333, run 120, rhand/lhand 0,
        // targetable + show_name true (→ status mask 0x0C), type Folk.
        let (npc, (mut pos, _region, vitals, speeds, _collision, _attack, _ai, _aggro)) =
            crate::model::npc::Npc::for_test(0x4000_0001, 30001, 100, 200, -300, 100, 50);
        pos.heading = 4000;
        let v = crate::model::npc::NpcView { npc: &npc, pos: &pos, vitals: &vitals, speeds: &speeds };

        let mut w = PacketWriter::new();
        w.write_u8(0x0C); // NPC_INFO
        w.write_i32(0x4000_0001);
        w.write_u8(0); // no summon animation
        w.write_i16(37);
        // Components: Id, Attackable, Relations, Name, Position, Heading,
        // AtkCastSpeed | SpeedMultiplier, StopMode, MoveMode (+ pre-set
        // 0x0C/0x0D) | PetEvolutionId (+ pre-set 0x14/0x15) | CurrentHp,
        // CurrentMp, MaxHp, MaxMp | VisualState(37).
        w.write_bytes(&[0xFD, 0xBC, 0x1C, 0xF0, 0x04]);
        w.write_u8(5); // init size: attackable(1) + relations(4)
        w.write_u8(0); // Folk is not in the Attackable subtree
        w.write_i32(0); // relations
        w.write_i16(69); // block 2 size
        w.write_i32(1_030_001); // display id + 1000000
        w.write_i32(100);
        w.write_i32(200);
        w.write_i32(-300);
        w.write_i32(4000); // heading
        w.write_i32(300); // p atk spd
        w.write_i32(333); // m atk spd
        w.write_f32(1.0); // movement multiplier
        w.write_f32(1.0); // attack speed multiplier
        w.write_u8(1); // stop mode: alive
        w.write_u8(0); // move mode: walking
        w.write_i32(0); // pet evolution id
        w.write_i32(100); // cur hp
        w.write_i32(50); // cur mp
        w.write_i32(100); // max hp
        w.write_i32(50); // max mp
        w.write_string("Gina");
        w.write_u8(0x0C); // visual state: targetable | show name
        let expected = w.into_bytes();

        assert_eq!(super::npc_info(&v, &t), expected);
    }

    fn chr(last_access: i64, delete_time: i64) -> CharData {
        CharData { last_access, delete_time, ..Default::default() }
    }

    #[test]
    fn active_id_is_most_recently_accessed_non_deleted() {
        let chars = [chr(100, 0), chr(300, commons::util::now_millis()), chr(200, 0)];
        assert_eq!(lobby_active_id(&chars), 2);
    }

    #[test]
    fn active_id_is_none_when_all_marked_for_deletion() {
        let now = commons::util::now_millis();
        let chars = [chr(100, now), chr(300, now)];
        assert_eq!(lobby_active_id(&chars), -1);
    }

    #[test]
    fn active_id_is_none_for_empty_list() {
        assert_eq!(lobby_active_id(&[]), -1);
    }

    // ---- G9.6 shortcut/macro packet layouts (hand-computed against the
    // Java writeImpls — no client capture available, same approach as the
    // NpcInfo test above). ----

    use crate::model::components::{Macros, Shortcuts};
    use crate::model::shortcut::{Macro, MacroCmd, MacroType, MacroUpdateType, Shortcut, ShortcutType};

    fn skill_shortcut() -> Shortcut {
        Shortcut { slot: 1, page: 0, kind: ShortcutType::Skill, id: 1177, level: 2, character_type: 1, shared_reuse_group: -1 }
    }

    #[test]
    fn shortcut_init_layout_per_type() {
        let shortcuts = Shortcuts(std::collections::BTreeMap::from_iter(
            [
                Shortcut { slot: 0, page: 0, kind: ShortcutType::Action, id: 2, level: 0, character_type: 1, shared_reuse_group: -1 },
                skill_shortcut(),
                Shortcut { slot: 0, page: 1, kind: ShortcutType::Item, id: 0x1000_0005, level: 0, character_type: 1, shared_reuse_group: 0 },
            ]
            .into_iter()
            .map(|sc| (sc.client_slot(), sc)),
        ));
        let mut w = PacketWriter::new();
        w.write_u8(0x45);
        w.write_i32(3);
        // Slot 0: ACTION (Attack).
        w.write_i32(3); // ShortcutType.ACTION ordinal
        w.write_i32(0);
        w.write_i32(2);
        w.write_i32(1); // C6
        // Slot 1: SKILL.
        w.write_i32(2);
        w.write_i32(1);
        w.write_i32(1177);
        w.write_i16(2); // level
        w.write_i16(0); // sub-level
        w.write_i32(-1); // shared reuse group
        w.write_u8(0); // C5
        w.write_i32(1); // C6
        // Slot 12 (page 1 slot 0): ITEM.
        w.write_i32(1);
        w.write_i32(12);
        w.write_i32(0x1000_0005);
        w.write_i32(1); // enabled
        w.write_i32(0); // shared reuse group (template default)
        w.write_i32(0);
        w.write_i32(0);
        w.write_i32(0); // augment 1
        w.write_i32(0); // augment 2
        w.write_i32(0); // visual id
        assert_eq!(super::shortcut_init(&shortcuts), w.into_bytes());
    }

    #[test]
    fn shortcut_register_skill_layout() {
        let mut w = PacketWriter::new();
        w.write_u8(0x44);
        w.write_i32(2); // SKILL
        w.write_i32(1); // slot + page*12
        w.write_i32(1177);
        w.write_i16(2);
        w.write_i16(0);
        w.write_i32(-1);
        w.write_u8(0); // C5
        w.write_i32(1); // character type
        w.write_i32(0); // if 1 - can't use
        w.write_i32(0); // reuse delay ?
        assert_eq!(super::shortcut_register(&skill_shortcut()), w.into_bytes());
    }

    fn test_macro() -> Macro {
        Macro {
            id: 1000,
            icon: 3,
            name: "aa".into(),
            descr: "bb".into(),
            acronym: "cc".into(),
            commands: vec![MacroCmd { entry: 0, kind: MacroType::Skill, d1: 1177, d2: 1, cmd: String::new() }],
        }
    }

    #[test]
    fn send_macro_list_add_layout() {
        let m = test_macro();
        let mut w = PacketWriter::new();
        w.write_u8(0xE8);
        w.write_u8(1); // ADD
        w.write_i32(1000); // the created macro's id
        w.write_u8(1); // count
        w.write_u8(1); // has macro
        w.write_i32(1000);
        w.write_string("aa");
        w.write_string("bb");
        w.write_string("cc");
        w.write_i32(3); // icon
        w.write_u8(1); // command count
        w.write_u8(1); // running index (1-based)
        w.write_u8(1); // MacroType.SKILL ordinal
        w.write_i32(1177);
        w.write_u8(1); // d2
        w.write_string("");
        assert_eq!(super::send_macro_list(1, Some(&m), MacroUpdateType::Add), w.into_bytes());
    }

    #[test]
    fn send_macro_list_delete_has_no_body() {
        let m = test_macro();
        let mut w = PacketWriter::new();
        w.write_u8(0xE8);
        w.write_u8(0); // DELETE
        w.write_i32(1000); // the deleted macro's id
        w.write_u8(0); // count
        w.write_u8(1); // macro non-null, body still omitted
        assert_eq!(super::send_macro_list(0, Some(&m), MacroUpdateType::Delete), w.into_bytes());
    }

    #[test]
    fn send_all_macros_empty_and_counted() {
        // No macros: a single empty LIST packet.
        let empty = super::send_all_macros(&Macros::default());
        let mut w = PacketWriter::new();
        w.write_u8(0xE8);
        w.write_u8(1); // LIST
        w.write_i32(0); // no macro id in LIST entries
        w.write_u8(0); // count
        w.write_u8(0); // no macro
        assert_eq!(empty, vec![w.into_bytes()]);

        // Two macros: one packet each, both carrying the total count.
        let m2 = Macro { id: 1001, ..test_macro() };
        let macros = Macros { next_id: 1002, entries: vec![test_macro(), m2] };
        let pkts = super::send_all_macros(&macros);
        assert_eq!(pkts.len(), 2);
        for pkt in &pkts {
            assert_eq!(pkt[0], 0xE8);
            assert_eq!(pkt[1], 1); // LIST
            assert_eq!(i32::from_le_bytes([pkt[2], pkt[3], pkt[4], pkt[5]]), 0);
            assert_eq!(pkt[6], 2); // total count
            assert_eq!(pkt[7], 1); // has macro
        }
    }

    #[test]
    fn ex_show_crop_info_layout_matches_java() {
        use super::{ex_show_crop_info, opcodes, CropInfoEntry};

        // One crop line, buttons hidden. Byte layout hand-computed against
        // `ExShowCropInfo.writeImpl`.
        let crops = [CropInfoEntry {
            crop_id: 5000,
            amount: 10,
            start_amount: 40,
            price: 999,
            reward: 2,
            seed_level: 3,
            reward1_item_id: 6000,
            reward2_item_id: 6001,
        }];
        let bytes = ex_show_crop_info(1, true, Some(&crops));

        let mut exp = PacketWriter::new();
        exp.write_u8(opcodes::EX);
        exp.write_i16(opcodes::EX_SHOW_CROP_INFO);
        exp.write_u8(1); // hide buttons
        exp.write_i32(1); // manor id
        exp.write_i32(0);
        exp.write_i32(1); // crop count
        exp.write_i32(5000);
        exp.write_i64(10);
        exp.write_i64(40);
        exp.write_i64(999);
        exp.write_u8(2); // reward type
        exp.write_i32(3); // seed level
        exp.write_u8(1);
        exp.write_i32(6000);
        exp.write_u8(1);
        exp.write_i32(6001);
        assert_eq!(bytes, exp.into_bytes());

        // `crops = None` (Java `_crops == null`): header only, no crop count.
        let none = ex_show_crop_info(7, false, None);
        let mut exp_none = PacketWriter::new();
        exp_none.write_u8(opcodes::EX);
        exp_none.write_i16(opcodes::EX_SHOW_CROP_INFO);
        exp_none.write_u8(0);
        exp_none.write_i32(7);
        exp_none.write_i32(0);
        assert_eq!(none, exp_none.into_bytes());
    }
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

// ---------------------------------------------------------------------------
// Party packets (G10)
// ---------------------------------------------------------------------------

/// The per-member fields the party-window packets write, gathered from the
/// member's components by `game_loop/party.rs`.
#[derive(Debug, Clone)]
pub struct PartyMemberView {
    pub object_id: i32,
    pub name: String,
    pub cp: i32,
    pub max_cp: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub vitality: i32,
    pub level: i32,
    pub class_id: i32,
    pub race: i32,
}

/// `serverpackets/AskJoinParty`.
pub fn ask_join_party(requestor_name: &str, loot_rule_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::ASK_JOIN_PARTY);
    w.write_string(requestor_name);
    w.write_i32(loot_rule_id);
    w.into_bytes()
}

/// `serverpackets/JoinParty` — the answer echoed to the requestor. The
/// trailing int is Java's own `// TODO: Find me!`, written 0 there too.
pub fn join_party(response: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::JOIN_PARTY);
    w.write_i32(response);
    w.write_i32(0);
    w.into_bytes()
}

fn write_party_member(w: &mut PacketWriter, m: &PartyMemberView) {
    w.write_i32(m.object_id);
    w.write_string(&m.name);
    w.write_i32(m.cp);
    w.write_i32(m.max_cp);
    w.write_i32(m.hp);
    w.write_i32(m.max_hp);
    w.write_i32(m.mp);
    w.write_i32(m.max_mp);
    w.write_i32(m.vitality);
    w.write_u8(m.level as u8);
    w.write_i16(m.class_id as i16);
}

/// `serverpackets/PartySmallWindowAll` — the receiver's full party window
/// (every member **except the receiver**, leader first).
pub fn party_small_window_all(leader_object_id: i32, loot_rule_id: i32, others: &[PartyMemberView]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PARTY_SMALL_WINDOW_ALL);
    w.write_i32(leader_object_id);
    w.write_u8(loot_rule_id as u8);
    w.write_u8(others.len() as u8);
    for m in others {
        write_party_member(&mut w, m);
        w.write_u8(1); // unk
        w.write_i16(m.race as i16);
        w.write_i32(0); // summon count — no pets/servitors
    }
    w.into_bytes()
}

/// `serverpackets/PartySmallWindowAdd` — one new member for existing members'
/// windows (note: loot rule is an **int** here, a byte in `…All`).
pub fn party_small_window_add(leader_object_id: i32, loot_rule_id: i32, member: &PartyMemberView) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PARTY_SMALL_WINDOW_ADD);
    w.write_i32(leader_object_id);
    w.write_i32(loot_rule_id);
    write_party_member(&mut w, member);
    w.write_u8(0);
    w.write_i16(member.race as i16);
    w.into_bytes()
}

/// `serverpackets/PartySmallWindowDelete`.
pub fn party_small_window_delete(object_id: i32, name: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PARTY_SMALL_WINDOW_DELETE);
    w.write_i32(object_id);
    w.write_string(name);
    w.into_bytes()
}

/// `serverpackets/PartySmallWindowDeleteAll`.
pub fn party_small_window_delete_all() -> Vec<u8> {
    vec![opcodes::PARTY_SMALL_WINDOW_DELETE_ALL]
}

/// `PartySmallWindowUpdateType` masks — **natural** bit values written as one
/// short (`writeShort(_flags)`), NOT the reversed-array masked-packet scheme.
pub mod party_window_flags {
    pub const CURRENT_CP: u16 = 1;
    pub const MAX_CP: u16 = 2;
    pub const CURRENT_HP: u16 = 4;
    pub const MAX_HP: u16 = 8;
    pub const CURRENT_MP: u16 = 16;
    pub const MAX_MP: u16 = 32;
    pub const LEVEL: u16 = 64;
    pub const CLASS_ID: u16 = 128;
    pub const PARTY_SUBSTITUTE: u16 = 256;
    pub const VITALITY_POINTS: u16 = 512;
    /// The CP/HP/MP block `broadcastStatusUpdate` sends.
    pub const VITALS: u16 = CURRENT_CP | MAX_CP | CURRENT_HP | MAX_HP | CURRENT_MP | MAX_MP;
    /// `PartySmallWindowUpdate(member, true)` — every component.
    pub const ALL: u16 = VITALS | LEVEL | CLASS_ID | PARTY_SUBSTITUTE | VITALITY_POINTS;
}

/// `serverpackets/PartySmallWindowUpdate` — masked member-field refresh for
/// the other members' windows.
pub fn party_small_window_update(m: &PartyMemberView, flags: u16) -> Vec<u8> {
    use party_window_flags as f;
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PARTY_SMALL_WINDOW_UPDATE);
    w.write_i32(m.object_id);
    w.write_i16(flags as i16);
    for (flag, value) in [
        (f::CURRENT_CP, m.cp),
        (f::MAX_CP, m.max_cp),
        (f::CURRENT_HP, m.hp),
        (f::MAX_HP, m.max_hp),
        (f::CURRENT_MP, m.mp),
        (f::MAX_MP, m.max_mp),
    ] {
        if flags & flag != 0 {
            w.write_i32(value);
        }
    }
    if flags & f::LEVEL != 0 {
        w.write_u8(m.level as u8);
    }
    if flags & f::CLASS_ID != 0 {
        w.write_i16(m.class_id as i16);
    }
    if flags & f::PARTY_SUBSTITUTE != 0 {
        w.write_u8(0);
    }
    if flags & f::VITALITY_POINTS != 0 {
        w.write_i32(m.vitality);
    }
    w.into_bytes()
}

/// `serverpackets/PartyMemberPosition` — the 12 s member-location refresh.
pub fn party_member_position(locations: &[(i32, i32, i32, i32)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PARTY_MEMBER_POSITION);
    w.write_i32(locations.len() as i32);
    for &(object_id, x, y, z) in locations {
        w.write_i32(object_id);
        w.write_i32(x);
        w.write_i32(y);
        w.write_i32(z);
    }
    w.into_bytes()
}

/// `serverpackets/ExAskModifyPartyLooting` — the leader wants a new loot rule.
pub fn ex_ask_modify_party_looting(leader_name: &str, loot_rule_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_ASK_MODIFY_PARTY_LOOTING);
    w.write_string(leader_name);
    w.write_i32(loot_rule_id);
    w.into_bytes()
}

/// `serverpackets/ExSetPartyLooting` — the loot-rule change verdict.
pub fn ex_set_party_looting(result: i32, loot_rule_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SET_PARTY_LOOTING);
    w.write_i32(result);
    w.write_i32(loot_rule_id);
    w.into_bytes()
}

// ---------------------------------------------------------------------------
// Friend packets (G10)
// ---------------------------------------------------------------------------

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
