//! `ServerPackets` opcodes (the single-byte `_id1`).

pub const DELETE_OBJECT: u8 = 0x08;
pub const NPC_INFO: u8 = 0x0C;
/// `SummonInfo` — a summon as seen by players **other than** its owner (the
/// owner gets `PET_INFO`).
pub const SUMMON_INFO: u8 = 0x8B;
/// `SetSummonRemainTime` — drives the summon's remaining-lifetime bar.
pub const SET_SUMMON_REMAIN_TIME: u8 = 0xD1;
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
pub const ACQUIRE_SKILL_INFO: u8 = 0x91;
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
/// `PackageToList` — the account's other characters, the freight "send to" list.
pub const PACKAGE_TO_LIST: u8 = 0xC8;
/// `PackageSendableList` — the items that may be freighted to that character.
pub const PACKAGE_SENDABLE_LIST: u8 = 0xD2;
/// Private store: the owner's manage window, a buyer's view, and the title
/// message shown above a store owner.
pub const PRIVATE_STORE_MANAGE_LIST: u8 = 0xA0;
pub const PRIVATE_STORE_LIST: u8 = 0xA1;
pub const PRIVATE_STORE_MSG: u8 = 0xA2;
pub const PRIVATE_STORE_BUY_MANAGE_LIST: u8 = 0xBD;
pub const PRIVATE_STORE_BUY_LIST: u8 = 0xBE;
pub const PRIVATE_STORE_BUY_MSG: u8 = 0xBF;
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
pub const CHANGE_WAIT_TYPE: u8 = 0x29;
/// `PetSummonInfo` — what a summon's **owner** sees (others get `SummonInfo`).
pub const PET_INFO: u8 = 0xB2;
pub const ATTACK: u8 = 0x33;
pub const MOVE_TO_PAWN: u8 = 0x72;
pub const SHORT_CUT_REGISTER: u8 = 0x44;
pub const SHORT_CUT_INIT: u8 = 0x45;
pub const MACRO_LIST: u8 = 0xE8;
pub const SAY2: u8 = 0x4A;
/// `PetitionVotePacket` (G31) — the feedback prompt after a consultation ends.
pub const PETITION_VOTE: u8 = 0xFC;
/// `Snoop` (G31) — a mirrored chat line to a GM eavesdropping via `//snoop`.
pub const SNOOP: u8 = 0xDB;
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
// Tutorial windows (Q255 newbie tutorial).
pub const TUTORIAL_SHOW_HTML: u8 = 0xA6;
pub const TUTORIAL_SHOW_QUESTION_MARK: u8 = 0xA7;
pub const TUTORIAL_CLOSE_HTML: u8 = 0xA9;
pub const ASK_JOIN_PLEDGE: u8 = 0x2C;
pub const MANAGE_PLEDGE_POWER: u8 = 0x2A;
pub const SET_PLEDGE_CREST: u8 = 0x69;
pub const PLEDGE_CREST: u8 = 0x6A;
pub const SET_ALLIANCE_CREST: u8 = 0xAE;
pub const ALLIANCE_CREST: u8 = 0xAF;
pub const SURRENDER_PLEDGE_WAR: u8 = 0x67;
pub const ALLIANCE_INFO: u8 = 0xB5;
pub const ASK_JOIN_ALLIANCE: u8 = 0xBB;
pub const JOIN_PLEDGE: u8 = 0x2D;
pub const PLEDGE_SHOW_MEMBER_LIST_ADD: u8 = 0x5C;
pub const PLEDGE_SHOW_MEMBER_LIST_DELETE: u8 = 0x5D;
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
/// Vehicles / boats (G24.5).
pub const VEHICLE_INFO: u8 = 0x60;
pub const VEHICLE_DEPARTURE: u8 = 0x6C;
pub const GET_ON_VEHICLE: u8 = 0x6E;
pub const GET_OFF_VEHICLE: u8 = 0x6F;
pub const MOVE_TO_LOCATION_IN_VEHICLE: u8 = 0x7E;
pub const STOP_MOVE_IN_VEHICLE: u8 = 0x7F;
/// `MonRaceInfo` — the Monster Race Track board/animation (G26.5).
pub const MON_RACE_INFO: u8 = 0xE3;

/// `ListPartyWaiting` — the party-matching room browser (G30).
pub const LIST_PARTY_WAITING: u8 = 0x9C;
/// `PartyRoomInfo` — a matching room's settings panel (G30).
pub const PARTY_ROOM_INFO: u8 = 0x9D;

/// Extended packets: opcode 0xFE + a 2-byte little-endian sub-opcode.
pub const EX: u8 = 0xFE;
/// Mail / post (G30). `EX_REPLY_WRITE_POST` is what Java's `ExNoticePostSent`
/// writes — the id name is Java's, the packet is the sent-confirmation.
pub const EX_NOTICE_POST_ARRIVED: i16 = 0xAA;
pub const EX_SHOW_RECEIVED_POST_LIST: i16 = 0xAB;
pub const EX_SHOW_RECEIVED_POST: i16 = 0xAC;
pub const EX_SHOW_SENT_POST_LIST: i16 = 0xAD;
pub const EX_SHOW_SENT_POST: i16 = 0xAE;
pub const EX_REPLY_POST_ITEM_LIST: i16 = 0xB3;
pub const EX_CHANGE_POST_STATE: i16 = 0xB4;
pub const EX_REPLY_WRITE_POST: i16 = 0xB5;
pub const EX_UN_READ_MAIL_COUNT: i16 = 0x13C;
/// `ExPartyRoomMember` — the matching room's member list (G30).
pub const EX_PARTY_ROOM_MEMBER: i16 = 0x08;
/// `ExClosePartyRoom` — close the matching-room window (G30).
pub const EX_CLOSE_PARTY_ROOM: i16 = 0x09;
/// `ExAskJoinPartyRoom` — matching-room invitation dialog (G30).
pub const EX_ASK_JOIN_PARTY_ROOM: i16 = 0x35;
/// `ExListPartyMatchingWaitingRoom` — the looking-for-party list (G30).
pub const EX_LIST_PARTY_MATCHING_WAITING_ROOM: i16 = 0x36;
// Command channels / MPCC.
/// `ExManageMpccRoomMember` (shares Java's `EX_MANAGE_PARTY_ROOM_MEMBER` id).
pub const EX_MANAGE_PARTY_ROOM_MEMBER: i16 = 0x0A;
pub const EX_OPEN_MPCC: i16 = 0x12;
pub const EX_CLOSE_MPCC: i16 = 0x13;
pub const EX_ASK_JOIN_MPCC: i16 = 0x1A;
pub const EX_MPCC_SHOW_PARTY_MEMBER_INFO: i16 = 0x4C;
pub const EX_MPCC_PARTY_INFO_UPDATE: i16 = 0x5C;
pub const EX_MULTI_PARTY_COMMAND_CHANNEL_INFO: i16 = 0x31;
/// `ExInzoneWaiting` — the instance re-enter window (`/instancezone`).
pub const EX_INZONE_WAITING_INFO: i16 = 0x11E;
/// `ExPrivateStoreSetWholeMsg` — the package-sell store title.
pub const EX_PRIVATE_STORE_WHOLE_MSG: i16 = 0x81;
/// `ExCursedWeaponList` — the ids of every cursed weapon this server knows.
pub const EX_CURSED_WEAPON_LIST: i16 = 0x47;
/// `ExCursedWeaponLocation` — where the live ones currently are.
pub const EX_CURSED_WEAPON_LOCATION: i16 = 0x48;
pub const EX_MPCC_ROOM_INFO: i16 = 0x9C;
pub const EX_LIST_MPCC_WAITING: i16 = 0x9D;
pub const EX_DISSMISS_MPCC_ROOM: i16 = 0x9E;
pub const EX_MPCC_ROOM_MEMBER: i16 = 0xA0;
pub const EX_MPCC_PARTYMASTER_LIST: i16 = 0xA3;
pub const EX_IS_CHAR_NAME_CREATABLE: i16 = 0x10B;
pub const EX_SEND_MANOR_LIST: i16 = 0x22;
pub const EX_SHOW_CASTLE_INFO: i16 = 0x14;
pub const EX_SHOW_FORTRESS_INFO: i16 = 0x15;
pub const EX_SHOW_SEED_INFO: i16 = 0x23;
pub const EX_SHOW_CROP_INFO: i16 = 0x24;
pub const EX_SHOW_MANOR_DEFAULT_INFO: i16 = 0x25;
pub const EX_SHOW_SEED_SETTING: i16 = 0x26;
pub const EX_SHOW_CROP_SETTING: i16 = 0x2B;
pub const EX_UI_SETTING: i16 = 0x71;
pub const EX_ASK_MODIFY_PARTY_LOOTING: i16 = 0xC0;
pub const EX_SET_PARTY_LOOTING: i16 = 0xC1;
pub const EX_SHOW_QUEST_MARK: i16 = 0x21;
pub const EX_SHOW_SCREEN_MESSAGE: i16 = 0x39;
/// `ExPVPMatchCCRecord` — the TvT/CC scoreboard (G28).
pub const EX_PVP_MATCH_CCRECORD: i16 = 0x8A;
/// `ExItemAuctionInfoPacket` — the auctioneer's current/next auction window (G30.5).
pub const EX_ITEM_AUCTION_INFO: i16 = 0x69;
pub const EX_NPC_QUEST_HTML_MESSAGE: i16 = 0x8E;
pub const EX_QUEST_ITEM_LIST: i16 = 0xC7;
pub const EX_SERVER_PRIMITIVE: i16 = 0x11;
pub const EX_SET_COMPASS_ZONE_CODE: i16 = 0x33;
pub const EX_PCCAFE_POINT_INFO: i16 = 0x32;
pub const EX_VOTE_SYSTEM_INFO: i16 = 0xCA;
pub const EX_FISHING_START: i16 = 0x1E;
pub const EX_FISHING_END: i16 = 0x1F;
pub const EX_USER_INFO_FISHING: i16 = 0x159;
pub const EX_AUTO_FISH_AVAILABLE: i16 = 0x17B;
/// `PledgeSkillList` — the clan window's skill tab (`(id, level)` list).
pub const EX_PLEDGE_COUNT: i16 = 0x13D;
pub const EX_PLEDGE_POWER_GRADE_LIST: i16 = 0x3D;
pub const EX_PLEDGE_RECEIVE_POWER_INFO: i16 = 0x3E;
pub const EX_PLEDGE_RECEIVE_MEMBER_INFO: i16 = 0x3F;
pub const EX_PLEDGE_RECEIVE_WAR_LIST: i16 = 0x40;
pub const EX_PLEDGE_RECEIVE_SUB_PLEDGE_CREATED: i16 = 0x41;
pub const EX_PLEDGE_EMBLEM: i16 = 0x1B;
pub const EX_PLEDGE_RECRUIT_BOARD_DETAIL: i16 = 0x142;
pub const EX_PLEDGE_WAITING_LIST_APPLIED: i16 = 0x143;
pub const EX_PLEDGE_WAITING_LIST: i16 = 0x144;
pub const EX_PLEDGE_WAITING_USER: i16 = 0x145;
pub const EX_PLEDGE_DRAFT_LIST_SEARCH: i16 = 0x146;
pub const EX_PLEDGE_WAITING_LIST_ALARM: i16 = 0x147;
pub const EX_ACQUIRABLE_SKILL_LIST_BY_CLASS: i16 = 0xFA;
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
/// `ExPutItemResultForVariationMake` — the augment window's target-item echo.
pub const EX_PUT_ITEM_RESULT_FOR_VARIATION_MAKE: i16 = 0x54;
pub const EX_PUT_INTENSIVE_RESULT_FOR_VARIATION_MAKE: i16 = 0x55;
/// `ExPutCommissionResultForVariationMake` — the gemstone-fee echo.
pub const EX_PUT_COMMISSION_RESULT_FOR_VARIATION_MAKE: i16 = 0x56;
/// `ExPutItemResultForVariationCancel` — the cancel window's item echo.
pub const EX_PUT_ITEM_RESULT_FOR_VARIATION_CANCEL: i16 = 0x58;
pub const EX_VARIATION_RESULT: i16 = 0x57;
pub const EX_VARIATION_CANCEL_RESULT: i16 = 0x59;

/// Duel packets (G20) — `ExDuelAskStart`/`Ready`/`Start`/`End`/`UpdateUserInfo`.
pub const EX_DUEL_ASK_START: i16 = 0x4D;
pub const EX_DUEL_READY: i16 = 0x4E;
pub const EX_DUEL_START: i16 = 0x4F;
pub const EX_DUEL_END: i16 = 0x50;
pub const EX_DUEL_UPDATE_USER_INFO: i16 = 0x51;

/// Siege registration (`CASTLE_SIEGE_INFO`) — the register/roster window.
pub const CASTLE_SIEGE_INFO: u8 = 0xC9;

/// Siege attacker roster (`CASTLE_SIEGE_ATTACKER_LIST`).
pub const CASTLE_SIEGE_ATTACKER_LIST: u8 = 0xCA;

/// Siege defender roster (`CASTLE_SIEGE_DEFENDER_LIST`).
pub const CASTLE_SIEGE_DEFENDER_LIST: u8 = 0xCB;
