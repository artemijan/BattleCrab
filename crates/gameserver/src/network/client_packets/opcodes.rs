//! `ClientPackets` opcodes (single-byte `_id`).

pub const LOGOUT: u8 = 0x00;
pub const MOVE_BACKWARD_TO_LOCATION: u8 = 0x0F;
pub const PROTOCOL_VERSION: u8 = 0x0E;
/// `ObserverReturn` — leave the Broadcasting Tower's spectator mode.
pub const OBSERVER_RETURN: u8 = 0xC1;
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
pub const REQUEST_DROP_ITEM: u8 = 0x17;
pub const USE_ITEM: u8 = 0x19;
pub const REQUEST_DESTROY_ITEM: u8 = 0x60;
pub const REQUEST_CRYSTALLIZE_ITEM: u8 = 0x2F;
pub const REQUEST_SELL_ITEM: u8 = 0x37;
/// `RequestBuySeed` — a player buys seeds from a Manor Manager's current
/// production.
pub const REQUEST_BUY_SEED: u8 = 0xC5;
pub const REQUEST_PRIVATE_STORE_MANAGE_SELL: u8 = 0x30;
pub const SET_PRIVATE_STORE_LIST_SELL: u8 = 0x31;
pub const REQUEST_PRIVATE_STORE_QUIT_SELL: u8 = 0x96;
pub const REQUEST_PRIVATE_STORE_BUY: u8 = 0x83;
pub const SET_PRIVATE_STORE_MSG_SELL: u8 = 0x97;
pub const REQUEST_PRIVATE_STORE_MANAGE_BUY: u8 = 0x99;
pub const SET_PRIVATE_STORE_LIST_BUY: u8 = 0x9A;
pub const REQUEST_PRIVATE_STORE_QUIT_BUY: u8 = 0x9C;
pub const SET_PRIVATE_STORE_MSG_BUY: u8 = 0x9D;
pub const REQUEST_PRIVATE_STORE_SELL: u8 = 0x9F;
pub const TRADE_REQUEST: u8 = 0x1A;
pub const ADD_TRADE_ITEM: u8 = 0x1B;
pub const TRADE_DONE: u8 = 0x1C;
pub const ANSWER_TRADE_REQUEST: u8 = 0x55;
/// `RequestPackageSendableItemList` — open the freight send window for the
/// chosen account character.
/// `RequestSiegeInfo` — **empty in this Java build** (both `readImpl` and
/// `runImpl` are no-ops); the `SiegeInfo` window is pushed by the castle
/// Siege Manager's bypass instead.
pub const REQUEST_BLOCK: u8 = 0xA9;
pub const REQUEST_SIEGE_INFO: u8 = 0xAA;
/// `CannotMoveAnymore` — the client reports a blocked move.
pub const CANNOT_MOVE_ANYMORE: u8 = 0x47;
pub const REQUEST_PACKAGE_SENDABLE_ITEM_LIST: u8 = 0xA7;
/// `RequestPackageSend` — freight the listed items to that character.
pub const REQUEST_PACKAGE_SEND: u8 = 0xA8;
pub const SEND_WARE_HOUSE_DEPOSIT_LIST: u8 = 0x3B;
pub const SEND_WARE_HOUSE_WITH_DRAW_LIST: u8 = 0x3C;
pub const ACTION: u8 = 0x1F;
pub const REQUEST_MAGIC_SKILL_USE: u8 = 0x39;
pub const REQUEST_TARGET_CANCELD: u8 = 0x48;
pub const REQUEST_RESTART: u8 = 0x57;
/// Boats (G24.5) — board / step off a ferry.
pub const REQUEST_GET_ON_VEHICLE: u8 = 0x53;
pub const REQUEST_GET_OFF_VEHICLE: u8 = 0x54;
/// Boats (G24.5) — walk around on a ferry's deck.
pub const REQUEST_MOVE_TO_LOCATION_IN_VEHICLE: u8 = 0x75;
pub const REQUEST_ACTION_USE: u8 = 0x56;
/// `RequestSiegeAttackerList` (G24) — view a castle's registered attackers.
pub const REQUEST_SIEGE_ATTACKER_LIST: u8 = 0xAB;
/// `RequestSiegeDefenderList` (G24) — view a castle's owner + defenders.
pub const REQUEST_SIEGE_DEFENDER_LIST: u8 = 0xAC;
/// `RequestJoinSiege` (G24) — a clan leader registers/cancels for a siege.
pub const REQUEST_JOIN_SIEGE: u8 = 0xAD;
/// `RequestConfirmSiegeWaitingList` (G24) — the castle owner approves or
/// rejects a pending defender clan.
pub const REQUEST_CONFIRM_SIEGE_WAITING_LIST: u8 = 0xAE;
/// `RequestSetCastleSiegeTime` (G24) — the owner picks the siege hour.
pub const REQUEST_SET_CASTLE_SIEGE_TIME: u8 = 0xAF;
/// `RequestGetItemFromPet` — move an item from the pet's inventory back
/// to the owner's.
pub const REQUEST_GET_ITEM_FROM_PET: u8 = 0x2C;
/// `RequestPetUseItem` — the owner clicks an item in the pet's window.
pub const REQUEST_PET_USE_ITEM: u8 = 0x94;
/// `RequestGiveItemToPet` — move an item from the owner's inventory into
/// the pet's.
pub const REQUEST_GIVE_ITEM_TO_PET: u8 = 0x95;
pub const VALIDATE_POSITION: u8 = 0x59;
pub const REQUEST_ACQUIRE_SKILL: u8 = 0x7C;
pub const REQUEST_ACQUIRE_SKILL_INFO: u8 = 0x73;
/// `RequestEnchantItem` — perform the enchant (`objectId`, `supportId`).
pub const REQUEST_ENCHANT_ITEM: u8 = 0x5F;
pub const REQUEST_SKILL_LIST: u8 = 0x50;
/// Force attack / target switch (Ctrl-click). Java's `ClientPackets`
/// binds both this and `ATTACK_REQUEST` (0x32) to `AttackRequest`; the
/// Interlude client sends this one on a Ctrl-click, so both must route to
/// the same handler.
pub const ATTACK: u8 = 0x01;
pub const ATTACK_REQUEST: u8 = 0x32;
pub const APPEARING: u8 = 0x3A;
pub const REQUEST_RESTART_POINT: u8 = 0x7D;
pub const REQUEST_SHORT_CUT_REG: u8 = 0x3D;
pub const REQUEST_SHORT_CUT_DEL: u8 = 0x3F;
pub const REQUEST_MAKE_MACRO: u8 = 0xCD;
pub const REQUEST_DELETE_MACRO: u8 = 0xCE;
pub const SAY2: u8 = 0x49;
/// `RequestPartyMatchConfig` (G30) — open the party-matching board, which
/// also registers the requester in the looking-for-party waiting list.
pub const REQUEST_PARTY_MATCH_CONFIG: u8 = 0x7F;
/// `RequestPartyMatchList` (G30) — create a matching room, or edit the one
/// the requester leads.
pub const REQUEST_PARTY_MATCH_LIST: u8 = 0x80;
/// `RequestPartyMatchDetail` (G30) — join a matching room.
pub const REQUEST_PARTY_MATCH_DETAIL: u8 = 0x81;
/// `RequestPetition` (G31) — content string + petition-type int (1-9).
pub const REQUEST_PETITION: u8 = 0x89;
/// `RequestPetitionCancel` (G31) — no body.
pub const REQUEST_PETITION_CANCEL: u8 = 0x8A;
/// `RequestPetitionFeedback` (G31) — unused int, rate int (0-4), message.
pub const REQUEST_PETITION_FEEDBACK: u8 = 0xC9;
pub const REQUEST_BYPASS_TO_SERVER: u8 = 0x23;
/// `RequestShowBoard` — the client's community-board button. Body is one
/// unused int; opens the board at `Config.BBSDefault`.
pub const REQUEST_SHOW_BOARD: u8 = 0x5E;
/// `RequestBBSwrite` — a board write/submit (post, memo, …). Body is one
/// int + six strings (url + five args).
pub const REQUEST_BBS_WRITE: u8 = 0x24;
/// `SendBypassBuildCmd` — the `//command` GM bar (admin commands).
pub const SEND_BYPASS_BUILD_CMD: u8 = 0x74;
/// `BypassUserCmd` — the client `/command` bar (`/unstuck`, `/loc`, …);
/// body is one int command id.
pub const BYPASS_USER_CMD: u8 = 0xB3;
/// `DlgAnswer` — reply to a `ConfirmDlg` (used by the admin-confirm flow).
pub const DLG_ANSWER: u8 = 0xC6;
/// `RequestQuestList` (G33) — the client opening its quest journal; empty
/// body, the server just re-sends `QuestList`.
pub const REQUEST_QUEST_LIST: u8 = 0x62;
pub const REQUEST_QUEST_ABORT: u8 = 0x63;
/// `RequestPledgeInfo` — asks for a clan's name/ally name by clan id.
pub const REQUEST_JOIN_PLEDGE: u8 = 0x26;
pub const REQUEST_ANSWER_JOIN_PLEDGE: u8 = 0x27;
pub const REQUEST_WITHDRAWAL_PLEDGE: u8 = 0x28;
pub const REQUEST_OUST_PLEDGE_MEMBER: u8 = 0x29;
pub const REQUEST_PLEDGE_INFO: u8 = 0x65;
pub const REQUEST_PLEDGE_POWER: u8 = 0xCC;
pub const REQUEST_START_PLEDGE_WAR: u8 = 0x03;
pub const REQUEST_ALLY_INFO: u8 = 0x2E;
pub const REQUEST_SET_PLEDGE_CREST: u8 = 0x09;
pub const REQUEST_PLEDGE_CREST: u8 = 0x67;
pub const REQUEST_JOIN_ALLY: u8 = 0x8C;
pub const REQUEST_ANSWER_JOIN_ALLY: u8 = 0x8D;
pub const ALLY_LEAVE: u8 = 0x8E;
pub const ALLY_DISMISS: u8 = 0x8F;
pub const REQUEST_DISMISS_ALLY: u8 = 0x90;
pub const REQUEST_SET_ALLY_CREST: u8 = 0x91;
pub const REQUEST_ALLY_CREST: u8 = 0x92;
pub const REQUEST_STOP_PLEDGE_WAR: u8 = 0x05;
pub const REQUEST_SURRENDER_PLEDGE_WAR: u8 = 0x07;
/// `RequestPledgeMemberList` — the clan window's roster tab, re-requested.
/// Empty body.
pub const REQUEST_PLEDGE_MEMBER_LIST: u8 = 0x4D;
/// `RequestMagicSkillList` — "resend my skill list", one int (the
/// requester's own object id, which Java verifies).
pub const REQUEST_MAGIC_SKILL_LIST: u8 = 0x38;
/// `RequestGmList` — the `/gmlist` chat command. Empty body.
pub const REQUEST_GM_LIST: u8 = 0x8B;
/// `SnoopQuit` — the snooped player's id; closes one `//snoop` window.
pub const SNOOP_QUIT: u8 = 0xB4;
/// `StartRotating` — keyboard turn begins: `degree`, `side`.
pub const START_ROTATING: u8 = 0x5B;
/// `FinishRotating` — keyboard turn settles: `degree`, then an int Java
/// itself labels "Unknown".
pub const FINISH_ROTATING: u8 = 0x5C;
/// `CannotMoveAnymoreInVehicle` — stopped while walking a boat's deck:
/// `boatId`, `x`, `y`, `z`, `heading`.
pub const CANNOT_MOVE_ANYMORE_IN_VEHICLE: u8 = 0x76;
/// `RequestRecipeShopManagePrev` — the browse window's back button.
/// Empty body.
pub const REQUEST_RECIPE_SHOP_MANAGE_PREV: u8 = 0xC0;
/// `RequestGiveNickName` — grant a title: target name, then the title.
pub const REQUEST_GIVE_NICK_NAME: u8 = 0x0B;
/// `RequestLinkHtml` — an `action="link <path>"` html anchor.
pub const REQUEST_LINK_HTML: u8 = 0x22;
/// `RequestPetGetItem` — order the pet to fetch one ground item.
pub const REQUEST_PET_GET_ITEM: u8 = 0x98;
/// `RequestPreviewItem` — the shop's "try on" button: unknown int, list
/// id, count, then that many item ids.
pub const REQUEST_PREVIEW_ITEM: u8 = 0xC7;
/// `RequestGMCommand` — a GM inspecting a player: target name, then a
/// command number (1 status, 2 clan, 3 skills, 4 quests, 5 inventory,
/// 6 warehouse).
pub const REQUEST_GM_COMMAND: u8 = 0x7E;
pub const REQUEST_BUY_ITEM: u8 = 0x40;
pub const REQUEST_JOIN_PARTY: u8 = 0x42;
pub const REQUEST_ANSWER_JOIN_PARTY: u8 = 0x43;
pub const REQUEST_WITH_DRAWAL_PARTY: u8 = 0x44;
pub const REQUEST_OUST_PARTY_MEMBER: u8 = 0x45;
pub const REQUEST_SEND_FRIEND_MSG: u8 = 0x6B;
/// `RequestShowMiniMap` — the client's map button (empty body).
pub const REQUEST_SHOW_MINI_MAP: u8 = 0x6C;
/// `RequestRecipeBookOpen` — the "Common Craft" / "Dwarven Craft" action;
/// body is one int (`0` = dwarven craft, else common).
pub const REQUEST_RECIPE_BOOK_OPEN: u8 = 0xB5;
pub const REQUEST_HENNA_EQUIP: u8 = 0x6F;
pub const REQUEST_HENNA_REMOVE_LIST: u8 = 0x70;
pub const REQUEST_HENNA_ITEM_REMOVE_INFO: u8 = 0x71;
pub const REQUEST_HENNA_REMOVE: u8 = 0x72;
pub const REQUEST_HENNA_ITEM_LIST: u8 = 0xC3;
pub const REQUEST_HENNA_ITEM_INFO: u8 = 0xC4;
pub const REQUEST_RECIPE_BOOK_DESTROY: u8 = 0xB6;
pub const REQUEST_RECIPE_ITEM_MAKE_INFO: u8 = 0xB7;
pub const REQUEST_RECIPE_ITEM_MAKE_SELF: u8 = 0xB8;
pub const REQUEST_RECIPE_SHOP_MANAGE_LIST: u8 = 0xB9;
pub const REQUEST_RECIPE_SHOP_MESSAGE_SET: u8 = 0xBA;
pub const REQUEST_RECIPE_SHOP_LIST_SET: u8 = 0xBB;
pub const REQUEST_RECIPE_SHOP_MANAGE_QUIT: u8 = 0xBC;
pub const REQUEST_RECIPE_SHOP_MAKE_INFO: u8 = 0xBE;
pub const REQUEST_RECIPE_SHOP_MAKE_ITEM: u8 = 0xBF;
pub const REQUEST_FRIEND_INVITE: u8 = 0x77;
pub const REQUEST_ANSWER_FRIEND_INVITE: u8 = 0x78;
pub const REQUEST_FRIEND_LIST: u8 = 0x79;
pub const REQUEST_FRIEND_DEL: u8 = 0x7A;
/// `MultiSellChoose` — a purchase/exchange click in the multisell window.
pub const MULTI_SELL_CHOOSE: u8 = 0xB0;
/// Tutorial windows (Q255): a `link` click, a `bypass` press, a shown
/// question-mark click, and the dead client-event echo.
pub const REQUEST_TUTORIAL_LINK_HTML: u8 = 0x85;
pub const REQUEST_TUTORIAL_PASS_CMD_TO_SERVER: u8 = 0x86;
pub const REQUEST_TUTORIAL_QUESTION_MARK: u8 = 0x87;
pub const REQUEST_TUTORIAL_CLIENT_EVENT: u8 = 0x88;
/// Extended packets: opcode 0xD0 + a 2-byte little-endian sub-opcode.
pub const EX_PACKET: u8 = 0xD0;
