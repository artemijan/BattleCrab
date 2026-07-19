//! `ServerPackets` opcodes (the single-byte `_id1`).

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
pub const CONFIRM_DLG: u8 = 0xF3;
pub const RESTART_RESPONSE: u8 = 0x71;
pub const LOG_OUT_OK: u8 = 0x84;
pub const SETUP_GAUGE: u8 = 0x6B;
pub const SKILL_COOL_TIME: u8 = 0xC7;
pub const HENNA_ITEM_INFO: u8 = 0xE4;
pub const HENNA_INFO: u8 = 0xE5;
pub const HENNA_UNEQUIP_LIST: u8 = 0xE6;
pub const HENNA_UNEQUIP_INFO: u8 = 0xE7;
pub const HENNA_EQUIP_LIST: u8 = 0xEE;
pub const RECIPE_BOOK_ITEM_LIST: u8 = 0xDC;
pub const RECIPE_ITEM_MAKE_INFO: u8 = 0xDD;
pub const RECIPE_SHOP_MANAGE_LIST: u8 = 0xDE;
pub const RECIPE_SHOP_SELL_LIST: u8 = 0xDF;
pub const RECIPE_SHOP_ITEM_INFO: u8 = 0xE0;
pub const RECIPE_SHOP_MSG: u8 = 0xE1;
pub const ACQUIRE_SKILL_DONE: u8 = 0x94;
pub const MY_TARGET_SELECTED: u8 = 0xB9;
pub const DIE: u8 = 0x00;
pub const REVIVE: u8 = 0x01;
pub const TELEPORT_TO_LOCATION: u8 = 0x22;
pub const RIDE: u8 = 0x8C;
/// Ground items: an item already lying in view (`SpawnItem`), a fresh drop with
/// the toss animation (`DropItem`), and the pickup animation (`GetItem`).
pub const SPAWN_ITEM: u8 = 0x05;
pub const DROP_ITEM: u8 = 0x16;
pub const GET_ITEM: u8 = 0x17;
/// Personal/clan warehouse deposit + withdraw list windows.
pub const WAREHOUSE_DEPOSIT_LIST: u8 = 0x41;
pub const WAREHOUSE_WITHDRAW_LIST: u8 = 0x42;
/// Private store: the owner's manage window, a buyer's view, and the title
/// message shown above a store owner.
pub const PRIVATE_STORE_MANAGE_LIST: u8 = 0xA0;
pub const PRIVATE_STORE_LIST: u8 = 0xA1;
pub const PRIVATE_STORE_MSG: u8 = 0xA2;
/// Player-to-player trade window.
pub const TRADE_START: u8 = 0x14;
pub const TRADE_OWN_ADD: u8 = 0x1A;
pub const TRADE_OTHER_ADD: u8 = 0x1B;
pub const TRADE_DONE: u8 = 0x1C;
pub const TRADE_PRESS_OWN_OK: u8 = 0x53;
pub const SEND_TRADE_REQUEST: u8 = 0x70;
pub const TRADE_PRESS_OTHER_OK: u8 = 0x82;
pub const AUTO_ATTACK_START: u8 = 0x25;
pub const AUTO_ATTACK_STOP: u8 = 0x26;
pub const RELATION_CHANGED: u8 = 0xCE;
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
pub const PLEDGE_INFO: u8 = 0x89;
pub const PLEDGE_SHOW_MEMBER_LIST_DELETE_ALL: u8 = 0x88;
pub const GM_VIEW_PLEDGE_INFO: u8 = 0x96;
pub const DOOR_STATUS_UPDATE: u8 = 0x4D;
pub const STATIC_OBJECT: u8 = 0x9F;
pub const NPC_SAY: u8 = 0x30;
pub const CHOOSE_INVENTORY_ITEM: u8 = 0x7C;
pub const ENCHANT_RESULT: u8 = 0x87;
pub const SHOW_MINI_MAP: u8 = 0xA3;
pub const SUN_RISE: u8 = 0x12;
pub const SUN_SET: u8 = 0x13;
pub const EARTHQUAKE: u8 = 0xD3;
pub const SHOW_BOARD: u8 = 0x7B;
pub const RADAR_CONTROL: u8 = 0xF1;
/// `MultiSellList` — the multisell exchange window (one packet per 40-entry page).
pub const MULTI_SELL_LIST: u8 = 0xD0;

/// Extended packets: opcode 0xFE + a 2-byte little-endian sub-opcode.
pub const EX: u8 = 0xFE;
pub const EX_IS_CHAR_NAME_CREATABLE: i16 = 0x10B;
pub const EX_SEND_MANOR_LIST: i16 = 0x22;
pub const EX_SHOW_CASTLE_INFO: i16 = 0x14;
pub const EX_SHOW_FORTRESS_INFO: i16 = 0x15;
pub const EX_SHOW_CROP_INFO: i16 = 0x24;
pub const EX_UI_SETTING: i16 = 0x71;
pub const EX_ASK_MODIFY_PARTY_LOOTING: i16 = 0xC0;
pub const EX_SET_PARTY_LOOTING: i16 = 0xC1;
pub const EX_SHOW_QUEST_MARK: i16 = 0x21;
pub const EX_NPC_QUEST_HTML_MESSAGE: i16 = 0x8E;
pub const EX_QUEST_ITEM_LIST: i16 = 0xC7;
pub const EX_SET_COMPASS_ZONE_CODE: i16 = 0x33;
pub const EX_PCCAFE_POINT_INFO: i16 = 0x32;
pub const EX_VOTE_SYSTEM_INFO: i16 = 0xCA;
/// `PledgeSkillList` — the clan window's skill tab (`(id, level)` list).
pub const EX_PLEDGE_SKILL_LIST: i16 = 0x3A;
/// `PledgeSkillListAdd` — one newly-learned clan skill `(id, level)`.
pub const EX_PLEDGE_SKILL_LIST_ADD: i16 = 0x3B;
/// `ExTeleportToLocationActivate` — the "teleport finished" packet; the
/// client stays on the loading screen until it arrives.
pub const EX_TELEPORT_TO_LOCATION_ACTIVATE: i16 = 0x14A;
/// `ExUserInfoAbnormalVisualEffect` — the abnormal-visual list (incl. GM
/// invisibility's STEALTH glow).
pub const EX_USER_INFO_ABNORMAL_VISUAL_EFFECT: i16 = 0x158;
pub const EX_AUTO_SOUL_SHOT: i16 = 0x0C;
pub const EX_RED_SKY: i16 = 0x42;
pub const EX_PUT_ENCHANT_TARGET_ITEM_RESULT: i16 = 0x82;
/// `ExMultiSellResult` — the post-exchange "you got N of X" acknowledgement.
pub const EX_MULTISELL_RESULT: i16 = 0x182;
pub const EX_PUT_ENCHANT_SCROLL_ITEM_RESULT: i16 = 0x152;
pub const EX_PUT_ENCHANT_SUPPORT_ITEM_RESULT: i16 = 0x83;
pub const EX_REMOVE_ENCHANT_SUPPORT_ITEM_RESULT: i16 = 0x153;
pub const EX_SHOW_VARIATION_MAKE_WINDOW: i16 = 0x52;
pub const EX_SHOW_VARIATION_CANCEL_WINDOW: i16 = 0x53;
pub const EX_PUT_INTENSIVE_RESULT_FOR_VARIATION_MAKE: i16 = 0x55;
pub const EX_VARIATION_RESULT: i16 = 0x57;
pub const EX_VARIATION_CANCEL_RESULT: i16 = 0x59;
