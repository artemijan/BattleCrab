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
}

/// Extended (`0xD0`) client sub-opcodes.
pub mod ex_opcodes {
    pub const REQUEST_MANOR_LIST: u16 = 0x01;
    /// `RequestProcureCropList` — a player sells crops to a Manor Manager.
    pub const REQUEST_PROCURE_CROP_LIST: u16 = 0x02;
    /// `RequestSetSeed` — the manor owner submits the next-period seed setup.
    pub const REQUEST_SET_SEED: u16 = 0x03;
    /// `RequestSetCrop` — the manor owner submits the next-period crop setup.
    pub const REQUEST_SET_CROP: u16 = 0x04;
    pub const REQUEST_KEY_MAPPING: u16 = 0x21;
    pub const REQUEST_CHARACTER_NAME_CREATABLE: u16 = 0xA9;
    pub const REQUEST_USER_BAN_INFO: u16 = 0x138;
    /// `ExSendClientIni` — the client reports its `client.ini` after auth.
    /// Mobius registers a `null` handler (no packet class), so it is consumed
    /// and ignored.
    pub const EX_SEND_CLIENT_INI: u16 = 0x104;
    /// `RequestHardWareInfo` (G31) — the client's hardware fingerprint (MAC,
    /// CPU, VGA, Windows build). Sendable at any connection state.
    pub const REQUEST_HARDWARE_INFO: u16 = 0xAE;
    /// `RequestExMagicSkillUseGround` — a GROUND-target cast aimed at a world
    /// position (G19, PLAN_G19_GROUND_CHANNELING.md).
    pub const REQUEST_EX_MAGIC_SKILL_USE_GROUND: u16 = 0x41;
    /// `RequestConfirmTargetItem` — the augment window's first slot.
    pub const REQUEST_CONFIRM_TARGET_ITEM: u16 = 0x26;
    /// `RequestConfirmGemStone` — the augment window's fee slot.
    pub const REQUEST_CONFIRM_GEMSTONE: u16 = 0x28;
    /// `RequestConfirmCancelItem` — the augment *cancel* window's slot.
    pub const REQUEST_CONFIRM_CANCEL_ITEM: u16 = 0x3F;
    /// `RequestSaveKeyMapping` — store the client's key layout.
    pub const REQUEST_SAVE_KEY_MAPPING: u16 = 0x22;
    /// `RequestCursedWeaponList` — the client's cursed-weapon window opening.
    pub const REQUEST_CURSED_WEAPON_LIST: u16 = 0x2A;
    /// `RequestCursedWeaponLocation` — "where are they?" for that window.
    pub const REQUEST_CURSED_WEAPON_LOCATION: u16 = 0x2B;
    /// `SetPrivateStoreWholeMsg` — the package-sell store's title.
    pub const SET_PRIVATE_STORE_WHOLE_MSG: u16 = 0x47;
    /// Skill enchanting (G19, PLAN_G19_SKILL_ENCHANT.md).
    pub const REQUEST_EX_ENCHANT_SKILL_INFO: u16 = 0x0E;
    pub const REQUEST_EX_ENCHANT_SKILL: u16 = 0x0F;
    pub const REQUEST_EX_ENCHANT_SKILL_INFO_DETAIL: u16 = 0x43;
    /// Duels (G20) — `RequestDuelStart` / `AnswerStart` / `Surrender`.
    pub const REQUEST_DUEL_START: u16 = 0x1B;
    pub const REQUEST_DUEL_ANSWER_START: u16 = 0x1C;
    pub const REQUEST_DUEL_SURRENDER: u16 = 0x42;
    /// Olympiad observer mode: end / open list / refresh list.
    pub const REQUEST_OLYMPIAD_OBSERVER_END: u16 = 0x29;
    pub const REQUEST_OLYMPIAD_MATCH_LIST: u16 = 0x2E;
    pub const REQUEST_EX_OLYMPIAD_MATCH_LIST_REFRESH: u16 = 0x85;
    pub const REQUEST_GOTO_LOBBY: u16 = 0x33;
    pub const REQUEST_CHANGE_PARTY_LEADER: u16 = 0x0C;
    /// Mail / post (G30) — `RequestPostItemList` through
    /// `RequestCancelPostAttachment`, Java's ex 0x62..0x6C block.
    pub const REQUEST_POST_ITEM_LIST: u16 = 0x62;
    pub const REQUEST_SEND_POST: u16 = 0x63;
    pub const REQUEST_RECEIVED_POST_LIST: u16 = 0x64;
    pub const REQUEST_DELETE_RECEIVED_POST: u16 = 0x65;
    pub const REQUEST_RECEIVED_POST: u16 = 0x66;
    pub const REQUEST_POST_ATTACHMENT: u16 = 0x67;
    pub const REQUEST_REJECT_POST_ATTACHMENT: u16 = 0x68;
    pub const REQUEST_SENT_POST_LIST: u16 = 0x69;
    pub const REQUEST_DELETE_SENT_POST: u16 = 0x6A;
    pub const REQUEST_SENT_POST: u16 = 0x6B;
    pub const REQUEST_CANCEL_POST_ATTACHMENT: u16 = 0x6C;
    /// `RequestRefundItem` — buy back items from the sell window's refund tab.
    pub const REQUEST_REFUND_ITEM: u16 = 0x72;
    /// `RequestBuySellUIClose` — the client closed a buy/sell window; the
    /// server answers with a full inventory refresh (same as `RequestItemList`).
    pub const REQUEST_BUY_SELL_UI_CLOSE: u16 = 0x73;
    /// Command channels (MPCC): invite / answer / oust a party, and the CC
    /// window's party-roster query.
    pub const REQUEST_EX_ASK_JOIN_MPCC: u16 = 0x06;
    pub const REQUEST_EX_ACCEPT_JOIN_MPCC: u16 = 0x07;
    pub const REQUEST_EX_OUST_FROM_MPCC: u16 = 0x08;
    pub const REQUEST_EX_MPCC_SHOW_PARTY_MEMBERS_INFO: u16 = 0x2D;
    /// MPCC matching rooms (the CC counterpart of the party rooms).
    pub const REQUEST_EX_LIST_MPCC_WAITING: u16 = 0x5A;
    pub const REQUEST_EX_MANAGE_MPCC_ROOM: u16 = 0x5B;
    pub const REQUEST_EX_JOIN_MPCC_ROOM: u16 = 0x5C;
    pub const REQUEST_EX_OUST_FROM_MPCC_ROOM: u16 = 0x5D;
    pub const REQUEST_EX_DISMISS_MPCC_ROOM: u16 = 0x5E;
    pub const REQUEST_EX_WITHDRAW_MPCC_ROOM: u16 = 0x5F;
    pub const REQUEST_EX_MPCC_PARTYMASTER_LIST: u16 = 0x61;
    /// `RequestOustFromPartyRoom` (G30) — the room leader kicks a member.
    pub const REQUEST_OUST_FROM_PARTY_ROOM: u16 = 0x09;
    /// `RequestDismissPartyRoom` (G30) — the room leader disbands the room.
    pub const REQUEST_DISMISS_PARTY_ROOM: u16 = 0x0A;
    /// `RequestWithdrawPartyRoom` (G30) — leave the room you are in.
    pub const REQUEST_WITHDRAW_PARTY_ROOM: u16 = 0x0B;
    /// `RequestExitPartyMatchingWaitingRoom` (G30) — stop advertising yourself
    /// as looking-for-party. No body.
    pub const REQUEST_EXIT_PARTY_MATCHING_WAITING_ROOM: u16 = 0x25;
    /// `RequestAskJoinPartyRoom` (G30) — invite a player to your room by name.
    pub const REQUEST_ASK_JOIN_PARTY_ROOM: u16 = 0x2F;
    /// `AnswerJoinPartyRoom` (G30) — accept/decline a room invitation.
    pub const ANSWER_JOIN_PARTY_ROOM: u16 = 0x30;
    /// `RequestListPartyMatchingWaitingRoom` (G30) — browse the players who are
    /// advertising themselves as looking-for-party.
    pub const REQUEST_LIST_PARTY_MATCHING_WAITING_ROOM: u16 = 0x31;
    pub const REQUEST_PARTY_LOOT_MODIFICATION: u16 = 0x75;
    pub const ANSWER_PARTY_LOOT_MODIFICATION: u16 = 0x76;
    pub const REQUEST_SAVE_INVENTORY_ORDER: u16 = 0x24;
    pub const REQUEST_STOP_MOVE: u16 = 0xED;
    pub const EX_SEND_SELECTED_QUEST_ZONE_ID: u16 = 0xFF;
    pub const REQUEST_AUTO_SOULSHOT: u16 = 0x0D;
    /// `ExRequestAutoFish` — toggle the auto-fishing loop (G32).
    pub const REQUEST_EX_AUTO_FISH: u16 = 0x105;
    /// Item auction (G30.5): bid on / request info about an auctioneer's auction.
    pub const REQUEST_BID_ITEM_AUCTION: u16 = 0x36;
    pub const REQUEST_INFO_ITEM_AUCTION: u16 = 0x37;
    /// `RequestAllCastleInfo` / `RequestAllFortressInfo` — sent by the world
    /// map window when it opens (empty bodies).
    pub const REQUEST_ALL_CASTLE_INFO: u16 = 0x39;
    pub const REQUEST_ALL_FORTRESS_INFO: u16 = 0x3A;
    /// `RequestExTryToPutEnchantTargetItem` — pick the item to enchant (`objectId`).
    pub const REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM: u16 = 0x49;
    /// `RequestExCancelEnchantItem` — close the enchant window (empty body).
    pub const REQUEST_EX_CANCEL_ENCHANT_ITEM: u16 = 0x4B;
    /// `RequestExTryToPutEnchantSupportItem` — add a support item
    /// (`supportObjId`, `enchantObjId`).
    pub const REQUEST_EX_TRY_TO_PUT_ENCHANT_SUPPORT_ITEM: u16 = 0x4A;
    /// `RequestExRemoveEnchantSupportItem` — clear the support (empty body).
    pub const REQUEST_EX_REMOVE_ENCHANT_SUPPORT_ITEM: u16 = 0xE4;
    /// `RequestExAddEnchantScrollItem` — scroll + target selection
    /// (`scrollObjectId`, `enchantObjectId`).
    pub const REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM: u16 = 0xE3;
    /// `RequestConfirmRefinerItem` — augment: pick the life stone (`targetObjId`,
    /// `refinerObjId`).
    pub const REQUEST_CONFIRM_REFINER_ITEM: u16 = 0x27;
    /// `RequestRefine` — augment: apply (`targetObjId`, `mineralObjId`,
    /// `feeObjId`, `feeCount:long`).
    pub const REQUEST_REFINE: u16 = 0x3E;
    /// `RequestRefineCancel` — remove an augment (`targetObjId`).
    pub const REQUEST_REFINE_CANCEL: u16 = 0x40;
    /// `RequestVoteNew` — recommend the currently-targeted player (`targetId`).
    pub const REQUEST_VOTE_NEW: u16 = 0x7B;
    /// `RequestDispel` — alt+click a buff icon to cancel it (`objectId`,
    /// `skillId`, `skillLevel:short`, `skillSubLevel:short`).
    pub const REQUEST_DISPEL: u16 = 0x48;
    /// Clan entry (recruitment) queries the clan window fires on open.
    /// `RequestPledgeRecruitInfo` (`clanId`) asks for a clan's recruitment
    /// summary; the waiting/apply ones are empty-bodied status polls, and
    /// `RequestPledgeRecruitBoardSearch` (`clanLevel`, `karma`, `type`,
    /// `query:string`, `sort`, `descending`, `page`, `applicationType`) is the
    /// recruit-board tab's filter search. The rest of the
    /// `RequestPledgeRecruit*` family (board access/detail, waiting
    /// list management, draft list) is the G18 `ClanEntryManager` port.
    pub const REQUEST_PLEDGE_POWER_GRADE_LIST: u16 = 0x13;
    pub const REQUEST_PLEDGE_MEMBER_POWER_INFO: u16 = 0x14;
    pub const REQUEST_PLEDGE_SET_MEMBER_POWER_GRADE: u16 = 0x15;
    pub const REQUEST_PLEDGE_MEMBER_INFO: u16 = 0x16;
    pub const REQUEST_PLEDGE_REORGANIZE_MEMBER: u16 = 0x2C;
    pub const REQUEST_PLEDGE_WAR_LIST: u16 = 0x17;
    pub const REQUEST_EX_PLEDGE_CREST_LARGE: u16 = 0x10;
    pub const REQUEST_EX_SET_PLEDGE_CREST_LARGE: u16 = 0x11;
    pub const REQUEST_PLEDGE_RECRUIT_INFO: u16 = 0xD3;
    pub const REQUEST_PLEDGE_RECRUIT_BOARD_SEARCH: u16 = 0xD4;
    pub const REQUEST_PLEDGE_WAITING_APPLIED: u16 = 0xD8;
    pub const REQUEST_PLEDGE_RECRUIT_APPLY_INFO: u16 = 0xDE;
    pub const REQUEST_PLEDGE_RECRUIT_BOARD_ACCESS: u16 = 0xD5;
    pub const REQUEST_PLEDGE_RECRUIT_BOARD_DETAIL: u16 = 0xD6;
    pub const REQUEST_PLEDGE_WAITING_APPLY: u16 = 0xD7;
    pub const REQUEST_PLEDGE_WAITING_LIST: u16 = 0xD9;
    pub const REQUEST_PLEDGE_WAITING_USER: u16 = 0xDA;
    pub const REQUEST_PLEDGE_WAITING_USER_ACCEPT: u16 = 0xDB;
    pub const REQUEST_PLEDGE_DRAFT_LIST_SEARCH: u16 = 0xDC;
    pub const REQUEST_PLEDGE_DRAFT_LIST_APPLY: u16 = 0xDD;
    pub const REQUEST_PLEDGE_SIGN_IN_FOR_OPEN_JOINING_METHOD: u16 = 0x111;
}

/// A client's hardware fingerprint (Java `ClientHardwareInfoHolder`, G31),
/// reported by `RequestHardWareInfo`. Keyed off the MAC address for HWID
/// punishments; the rest is shown by `//hwid`. Only the display-relevant fields
/// are kept.
#[derive(Debug, Clone, Default)]
pub struct HardwareInfo {
    pub mac_address: String,
    pub windows_platform_id: i32,
    pub windows_major_version: i32,
    pub windows_minor_version: i32,
    pub windows_build_number: i32,
    pub cpu_name: String,
    pub cpu_speed: i32,
    pub cpu_core_count: i32,
    pub vga_name: String,
    pub vga_driver_version: String,
}

impl HardwareInfo {
    /// Parse a `RequestHardWareInfo` body (the 19-field `cdddddddd…` layout).
    pub fn read(ex_body: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(ex_body);
        let mac_address = r.read_string()?;
        let windows_platform_id = r.read_i32()?;
        let windows_major_version = r.read_i32()?;
        let windows_minor_version = r.read_i32()?;
        let windows_build_number = r.read_i32()?;
        r.read_i32()?; // directxVersion
        r.read_i32()?; // directxRevision
        let cpu_name = r.read_string()?;
        let cpu_speed = r.read_i32()?;
        let cpu_core_count = r.read_i32()?;
        r.read_i32()?; // vgaCount
        r.read_i32()?; // vgaPcxSpeed
        r.read_i32()?; // physMemorySlot1
        r.read_i32()?; // physMemorySlot2
        r.read_i32()?; // physMemorySlot3
        r.read_i32()?; // videoMemory
        r.read_i32()?; // vgaVersion
        let vga_name = r.read_string()?;
        let vga_driver_version = r.read_string()?;
        Some(Self {
            mac_address,
            windows_platform_id,
            windows_major_version,
            windows_minor_version,
            windows_build_number,
            cpu_name,
            cpu_speed,
            cpu_core_count,
            vga_name,
            vga_driver_version,
        })
    }
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

/// The quest-zone id of `ExSendSelectedQuestZoneID` (`readInt`, after the
/// sub-opcode).
pub fn read_selected_quest_zone_id(ex_body: &[u8]) -> Option<i32> {
    PacketReader::new(ex_body).read_i32()
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
        Self {
            version: r.read_i32().unwrap_or(0),
        }
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
        Some(Self {
            login_name,
            play_key1,
            play_key2,
            login_key1,
            login_key2,
        })
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
        Some(Self {
            name,
            is_female,
            class_id,
            hair_style,
            hair_color,
            face,
        })
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
        Some(Self {
            object_id,
            ctrl_pressed,
        })
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

/// Port of `clientpackets/RequestDestroyItem` (`dq`): the inventory item object
/// id and the count to destroy.
pub struct RequestDestroyItem {
    pub object_id: i32,
    pub count: i64,
}

impl RequestDestroyItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let object_id = r.read_i32()?;
        let count = r.read_i64()?;
        Some(Self { object_id, count })
    }
}

/// Port of `SendWareHouseDepositList` / `SendWareHouseWithDrawList` (`d[dq]`):
/// a count-prefixed list of `(object_id, count)` pairs — the items to move into
/// or out of the warehouse.
pub struct WarehouseItemList {
    pub items: Vec<(i32, i64)>,
}

impl WarehouseItemList {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let count = r.read_i32()?;
        if count <= 0 || count > 500 {
            return None;
        }
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let object_id = r.read_i32()?;
            let cnt = r.read_i64()?;
            if object_id < 1 || cnt < 0 {
                return None;
            }
            items.push((object_id, cnt));
        }
        Some(Self { items })
    }
}

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
            for _ in 0..count {
                let object_id = r.read_i32()?;
                let cnt = r.read_i64()?;
                if object_id < 1 || cnt < 0 {
                    return None;
                }
                items.push((object_id, cnt));
            }
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

/// Port of `clientpackets/RequestPartyMatchConfig` (`ddd`, G30): the room-list
/// page, the location (community-board region) filter, and the level-band mode
/// (`0` = my level range, anything else = all).
pub struct RequestPartyMatchConfig {
    pub page: i32,
    pub location: i32,
    pub level_filter: i32,
}

impl RequestPartyMatchConfig {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            page: r.read_i32()?,
            location: r.read_i32()?,
            level_filter: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/RequestPartyMatchList` (`dddddS`, G30): create a room
/// (`room_id <= 0`) or edit the one you lead.
pub struct RequestPartyMatchList {
    pub room_id: i32,
    pub max_members: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub loot_type: i32,
    pub title: String,
}

impl RequestPartyMatchList {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            room_id: r.read_i32()?,
            max_members: r.read_i32()?,
            min_level: r.read_i32()?,
            max_level: r.read_i32()?,
            loot_type: r.read_i32()?,
            title: r.read_string()?,
        })
    }
}

/// Port of `clientpackets/RequestPartyMatchDetail` (`ddd`, G30): join a room by
/// id, or — when `room_id <= 0` — the first room matching a location + level.
pub struct RequestPartyMatchDetail {
    pub room_id: i32,
    pub location: i32,
    pub level: i32,
}

impl RequestPartyMatchDetail {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            room_id: r.read_i32()?,
            location: r.read_i32()?,
            level: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/RequestListPartyMatchingWaitingRoom` (`dddd(d)*(S)?`,
/// G30): the looking-for-party browse filter.
pub struct RequestListPartyMatchingWaitingRoom {
    pub page: i32,
    pub min_level: i32,
    pub max_level: i32,
    /// Empty means "any class" (Java leaves the list null).
    pub class_ids: Vec<i32>,
    /// Optional name substring; Java only reads it when bytes remain.
    pub query: Option<String>,
}

impl RequestListPartyMatchingWaitingRoom {
    /// Java's own bound: it only consumes the class ids when
    /// `0 < size < 128`, which desyncs the rest of the read for a larger
    /// count. The port consumes exactly what the count claims (capped) so the
    /// trailing query string still lines up.
    const MAX_CLASSES: i32 = 127;

    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let page = r.read_i32()?;
        let min_level = r.read_i32()?;
        let max_level = r.read_i32()?;
        let size = r.read_i32()?;
        let mut class_ids = Vec::new();
        if size > 0 {
            if size > Self::MAX_CLASSES {
                return None;
            }
            for _ in 0..size {
                class_ids.push(r.read_i32()?);
            }
        }
        let query = if r.remaining() > 0 {
            r.read_string()
        } else {
            None
        };
        Some(Self {
            page,
            min_level,
            max_level,
            class_ids,
            query,
        })
    }
}

/// Port of `clientpackets/RequestExAskJoinMPCC` (`S`): invite a player's party
/// into a command channel by the clicked player's name.
pub struct RequestExAskJoinMpcc {
    pub name: String,
}

impl RequestExAskJoinMpcc {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            name: r.read_string()?,
        })
    }
}

/// Port of `clientpackets/RequestExAcceptJoinMPCC` (`d`): 1 = accept.
pub struct RequestExAcceptJoinMpcc {
    pub response: i32,
}

impl RequestExAcceptJoinMpcc {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            response: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/RequestExOustFromMPCC` (`S`): dismiss the named
/// player's whole party from the channel.
pub struct RequestExOustFromMpcc {
    pub name: String,
}

impl RequestExOustFromMpcc {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            name: r.read_string()?,
        })
    }
}

/// Port of `clientpackets/RequestExMPCCShowPartyMembersInfo` (`d`): the CC
/// window queries a party's roster by its leader's object id.
pub struct RequestExMpccShowPartyMembersInfo {
    pub party_leader_object_id: i32,
}

impl RequestExMpccShowPartyMembersInfo {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            party_leader_object_id: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/RequestExListMpccWaiting` (`ddd`): browse CC rooms.
pub struct RequestExListMpccWaiting {
    pub page: i32,
    pub location: i32,
    pub level: i32,
}

impl RequestExListMpccWaiting {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            page: r.read_i32()?,
            location: r.read_i32()?,
            level: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/RequestExManageMpccRoom` (`dddddS`): edit the CC
/// room you lead. The fifth int (party distribution type) is read and
/// discarded, as in Java.
pub struct RequestExManageMpccRoom {
    pub room_id: i32,
    pub max_members: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub title: String,
}

impl RequestExManageMpccRoom {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let room_id = r.read_i32()?;
        let max_members = r.read_i32()?;
        let min_level = r.read_i32()?;
        let max_level = r.read_i32()?;
        let _loot_type = r.read_i32()?;
        Some(Self {
            room_id,
            max_members,
            min_level,
            max_level,
            title: r.read_string()?,
        })
    }
}

/// Port of `clientpackets/RequestExJoinMpccRoom` (`d`).
pub struct RequestExJoinMpccRoom {
    pub room_id: i32,
}

impl RequestExJoinMpccRoom {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            room_id: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/RequestExOustFromMpccRoom` (`d`): kick by object id.
pub struct RequestExOustFromMpccRoom {
    pub object_id: i32,
}

impl RequestExOustFromMpccRoom {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            object_id: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/AddTradeItem` (`ddq`): the trade id, the inventory
/// item object id, and how many to add.
pub struct AddTradeItem {
    pub object_id: i32,
    pub count: i64,
}

impl AddTradeItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        r.read_i32()?; // trade id — unused (one active trade per player)
        let object_id = r.read_i32()?;
        let count = r.read_i64()?;
        Some(Self { object_id, count })
    }
}

/// Port of `SetPrivateStoreListSell` (`dd [dqq]`): the items to offer —
/// `(object_id, count, price)`. `RequestPrivateStoreBuy` (`dd [dqq]`) shares the
/// same trailing layout but leads with the seller's object id.
/// One offered line of `RequestPrivateStoreSell`.
pub struct StoreSellLine {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub price: i64,
}

/// `RequestPrivateStoreSell` — a customer filling someone's buy store.
pub struct StoreSellRequest {
    pub store_player: i32,
    pub items: Vec<StoreSellLine>,
}

/// One line of a buy store's wanted list (`SetPrivateStoreListBuy`).
pub struct WantedLine {
    pub item_id: i32,
    pub enchant: i32,
    pub count: i64,
    pub price: i64,
}

pub struct PrivateStoreItemList {
    /// `RequestPrivateStoreBuy` only: the seller's object id (`0` for a set-list).
    pub target_object_id: i32,
    pub items: Vec<(i32, i64, i64)>,
}

impl PrivateStoreItemList {
    /// `SetPrivateStoreListSell`: `packageSale(int)` then the item lines.
    /// Returns the leading **package-sale** flag alongside the lines: `1` opens
    /// a `PACKAGE_SELL` store (Java `SetPrivateStoreListSell._packageSale`).
    pub fn read_set_list(body_after_opcode: &[u8]) -> Option<(bool, Self)> {
        let mut r = PacketReader::new(body_after_opcode);
        let packaged = r.read_i32()? == 1;
        Self::read_lines(&mut r, 0).map(|lines| (packaged, lines))
    }

    /// `RequestPrivateStoreBuy`: `storePlayerId(int)` then the item lines.
    pub fn read_buy(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let seller = r.read_i32()?;
        Self::read_lines(&mut r, seller)
    }

    /// `SetPrivateStoreListBuy`: the wanted lines, keyed by **item id** (the
    /// owner doesn't own them yet) with the client's enchant/augment/element
    /// tail that this port ignores.
    pub fn read_set_list_buy(body_after_opcode: &[u8]) -> Option<Vec<WantedLine>> {
        let mut r = PacketReader::new(body_after_opcode);
        let count = r.read_i32()?;
        if !(1..=500).contains(&count) {
            return None;
        }
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let item_id = r.read_i32()?;
            let enchant = r.read_i16()? as i32;
            let _unknown = r.read_i16()?;
            let cnt = r.read_i64()?;
            let price = r.read_i64()?;
            let _option1 = r.read_i32()?;
            let _option2 = r.read_i32()?;
            // attack element (id + power) then the six defence elements.
            for _ in 0..8 {
                r.read_i16()?;
            }
            let _visual_id = r.read_i32()?;
            if item_id < 1 || cnt < 1 || price < 0 {
                return None;
            }
            items.push(WantedLine {
                item_id,
                enchant,
                count: cnt,
                price,
            });
        }
        Some(items)
    }

    /// `RequestPrivateStoreSell`: the store owner's object id, then the lines
    /// the customer offers — inventory object id, item id, count and the price
    /// the client believes the store pays, plus a soul-crystal/SA tail this
    /// port skips.
    pub fn read_store_sell(body_after_opcode: &[u8]) -> Option<StoreSellRequest> {
        let mut r = PacketReader::new(body_after_opcode);
        let store_player = r.read_i32()?;
        let count = r.read_i32()?;
        if !(1..=500).contains(&count) {
            return None;
        }
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let object_id = r.read_i32()?;
            let item_id = r.read_i32()?;
            let _enchant = r.read_i16()?;
            let _unknown = r.read_i16()?;
            let cnt = r.read_i64()?;
            let price = r.read_i64()?;
            let _visual = r.read_i32()?;
            let _option1 = r.read_i32()?;
            let _option2 = r.read_i32()?;
            // Two length-prefixed tails (soul-crystal options, SA effects).
            for _ in 0..2 {
                let extra = r.read_u8()? as i32;
                for _ in 0..extra {
                    r.read_i32()?;
                }
            }
            if item_id < 1 || cnt < 1 || price < 0 {
                return None;
            }
            items.push(StoreSellLine {
                object_id,
                item_id,
                count: cnt,
                price,
            });
        }
        Some(StoreSellRequest {
            store_player,
            items,
        })
    }

    fn read_lines(r: &mut PacketReader, target: i32) -> Option<Self> {
        let count = r.read_i32()?;
        if !(1..=500).contains(&count) {
            return None;
        }
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let object_id = r.read_i32()?;
            let cnt = r.read_i64()?;
            let price = r.read_i64()?;
            if object_id < 1 || cnt < 1 || price < 0 {
                return None;
            }
            items.push((object_id, cnt, price));
        }
        Some(Self {
            target_object_id: target,
            items,
        })
    }
}

/// Port of `clientpackets/RequestSellItem` (`dd [dq]`... actually `ddd q` per
/// entry): the buy-list id and the items to sell — `(object_id, item_id, count)`.
pub struct RequestSellItem {
    pub list_id: i32,
    pub items: Vec<(i32, i32, i64)>,
}

impl RequestSellItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let list_id = r.read_i32()?;
        let size = r.read_i32()?;
        if size <= 0 || size > 500 {
            return None;
        }
        let mut items = Vec::with_capacity(size as usize);
        for _ in 0..size {
            let object_id = r.read_i32()?;
            let item_id = r.read_i32()?;
            let count = r.read_i64()?;
            if object_id < 1 || item_id < 1 || count < 1 {
                return None;
            }
            items.push((object_id, item_id, count));
        }
        Some(Self { list_id, items })
    }
}

/// Port of `clientpackets/RequestRefundItem` (ex 0x72): buy back items from
/// the refund tab — the buy-list id and the refund-list positions to reclaim.
pub struct RequestRefundItem {
    #[allow(dead_code)] // Java validates it against BuyListData; we don't (yet).
    pub list_id: i32,
    pub indexes: Vec<i32>,
}

impl RequestRefundItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let list_id = r.read_i32()?;
        let count = r.read_i32()?;
        if count <= 0 || count > 500 {
            return None;
        }
        let mut indexes = Vec::with_capacity(count as usize);
        for _ in 0..count {
            indexes.push(r.read_i32()?);
        }
        Some(Self { list_id, indexes })
    }
}

/// Port of `clientpackets/RequestTutorialLinkHtml` (`dS`): a `link` click in
/// the tutorial window — a discarded int, then the bypass string.
pub struct RequestTutorialLinkHtml {
    pub bypass: String,
}

impl RequestTutorialLinkHtml {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let _unused = r.read_i32()?;
        Some(Self {
            bypass: r.read_string()?,
        })
    }
}

/// Port of `clientpackets/RequestTutorialPassCmdToServer` (`S`): a `bypass`
/// press in the tutorial window (no leading int, unlike the link packet).
pub struct RequestTutorialPassCmd {
    pub bypass: String,
}

impl RequestTutorialPassCmd {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            bypass: r.read_string()?,
        })
    }
}

/// Port of `clientpackets/RequestTutorialQuestionMark` (`cd`): the leading
/// byte mirrors the mark-type byte 0xA7 writes; only the mark id matters.
pub struct RequestTutorialQuestionMark {
    pub number: i32,
}

impl RequestTutorialQuestionMark {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let _mark_type = r.read_u8()?;
        Some(Self {
            number: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/MultiSellChoose`: the item exchange click. Reads the
/// full retail body (enchant/augment/elemental stats follow the amount), but
/// only `list_id`/`entry_id`/`amount` drive the community-board exchange path
/// (the enchant-maintenance validation is a `maintainEnchantment`-list concern,
/// TODO(G30)).
pub struct MultiSellChoose {
    pub list_id: i32,
    pub entry_id: i32,
    pub amount: i64,
    /// The enchant level the client echoes back for the row it clicked. Java
    /// refuses the exchange when it disagrees with the item the inventory-only
    /// window paired with that row.
    pub enchant_level: i32,
}

impl MultiSellChoose {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let list_id = r.read_i32()?;
        let entry_id = r.read_i32()?;
        let amount = r.read_i64()?;
        let enchant_level = i32::from(r.read_i16()?);
        // augment1(int), augment2(int), attackAttribute(short), attributePower
        // (short) and six elemental defence shorts — read to keep the reader
        // honest; augments/attributes aren't compared on this path (no dist
        // multisell carries them as ingredients).
        let _augment1 = r.read_i32()?;
        let _augment2 = r.read_i32()?;
        for _ in 0..8 {
            let _ = r.read_i16()?;
        }
        Some(Self {
            list_id,
            entry_id,
            amount,
            enchant_level,
        })
    }
}

/// Port of `clientpackets/RequestDropItem` (`dqddd`): item object id, count,
/// and the requested drop location.
pub struct RequestDropItem {
    pub object_id: i32,
    pub count: i64,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl RequestDropItem {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let object_id = r.read_i32()?;
        let count = r.read_i64()?;
        let x = r.read_i32()?;
        let y = r.read_i32()?;
        let z = r.read_i32()?;
        Some(Self {
            object_id,
            count,
            x,
            y,
            z,
        })
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
        Some(Self {
            magic_id,
            ctrl_pressed,
            shift_pressed,
        })
    }
}

/// Port of `clientpackets/RequestExMagicSkillUseGround` (ex 0x41) — a
/// `targetType GROUND` cast aimed at a world position (format `dddddc`).
pub struct RequestExMagicSkillUseGround {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub skill_id: i32,
    pub ctrl_pressed: bool,
    pub shift_pressed: bool,
}

impl RequestExMagicSkillUseGround {
    pub fn read(ex_body: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(ex_body);
        let x = r.read_i32()?;
        let y = r.read_i32()?;
        let z = r.read_i32()?;
        let skill_id = r.read_i32()?;
        let ctrl_pressed = r.read_i32()? != 0;
        let shift_pressed = r.read_u8().is_some_and(|b| b != 0);
        Some(Self {
            x,
            y,
            z,
            skill_id,
            ctrl_pressed,
            shift_pressed,
        })
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
    pub const PLEDGE: i32 = 2;
    pub const SUBPLEDGE: i32 = 3;

    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let skill_id = r.read_i32()?;
        let skill_level = r.read_i32()?;
        let acquire_type = r.read_i32()?;
        if acquire_type == Self::SUBPLEDGE {
            r.read_i32()?; // sub_type — unused (see doc comment)
        }
        Some(Self {
            skill_id,
            skill_level,
            acquire_type,
        })
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
        Some(Self {
            object_id,
            action_id,
        })
    }
}

/// Port of `clientpackets/AttackRequest` (`cddddc`) — the client clicking an
/// attackable creature. The origin coordinates are read and discarded like
/// Java's unused fields; the trailing `attackId` byte (`0` = simple click, `1`
/// = shift-click) is Java's `dontMove` flag — Java ignores it, but we honour it
/// so a shift-attack refuses to chase (see `start_attack_intent`).
pub struct AttackRequest {
    pub object_id: i32,
    pub shift: bool,
}

impl AttackRequest {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let object_id = r.read_i32()?;
        r.read_i32()?; // origin_x — unused
        r.read_i32()?; // origin_y — unused
        r.read_i32()?; // origin_z — unused
        let shift = r.read_u8()? == 1;
        Some(Self { object_id, shift })
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

/// Port of `clientpackets/RequestDispel` (ex `dddhh`): the alt+click buff-cancel
/// on a buff icon. `object_id` is whose buff (self, pet, or servitor);
/// `skill_id`/`skill_level`/`skill_sub_level` identify the buff to strip.
pub struct RequestDispel {
    pub object_id: i32,
    pub skill_id: i32,
    pub skill_level: i32,
    pub skill_sub_level: i32,
}

impl RequestDispel {
    /// `readImpl`: readInt objectId, readInt skillId, readShort skillLevel,
    /// readShort skillSubLevel. Called with the body after the 2-byte sub-opcode.
    pub fn read(ex_body: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(ex_body);
        let object_id = r.read_i32()?;
        let skill_id = r.read_i32()?;
        let skill_level = r.read_i16()? as i32;
        let skill_sub_level = r.read_i16()? as i32;
        Some(Self {
            object_id,
            skill_id,
            skill_level,
            skill_sub_level,
        })
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
        Some(Self {
            target_x,
            target_y,
            target_z,
            origin_x,
            origin_y,
            origin_z,
            movement_mode,
        })
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
        Some(Self {
            kind,
            slot: slot_raw % 12,
            page: slot_raw / 12,
            id,
            level,
            sub_level,
            character_type,
        })
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
        Some(Self {
            slot: slot_raw % 12,
            page: slot_raw / 12,
        })
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
            commands.push(MacroCmd {
                entry,
                kind,
                d1,
                d2,
                cmd,
            });
        }
        Some(Self {
            macro_: Macro {
                id,
                icon,
                name,
                descr,
                acronym,
                commands,
            },
            commands_length,
        })
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

/// Port of `clientpackets/RequestQuestAbort` — the quest UI's Abandon button.
pub struct RequestQuestAbort {
    pub quest_id: i32,
}

impl RequestQuestAbort {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            quest_id: r.read_i32()?,
        })
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

/// The single-int henna packets (`RequestHennaEquip`/`Remove`/`ItemInfo`/
/// `ItemRemoveInfo`): one int `symbolId`.
pub fn read_symbol_id(body: &[u8]) -> Option<i32> {
    PacketReader::new(body).read_i32()
}

/// One line of a `RequestRecipeShopListSet` manufacture list: recipe-list id +
/// adena cost.
#[derive(Debug, Clone, Copy)]
pub struct ManufactureLine {
    pub recipe_id: i32,
    pub cost: i64,
}

/// `RequestRecipeShopListSet` (0xBB): the manufacture recipes + prices the
/// seller set. Java: `count(int)` then `count × (id:int, cost:long)`; a
/// negative cost aborts the whole read (Java nulls `_items`).
pub fn read_recipe_shop_list_set(body: &[u8]) -> Option<Vec<ManufactureLine>> {
    let mut r = PacketReader::new(body);
    let count = r.read_i32()?;
    if !(0..=500).contains(&count) {
        return None;
    }
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let recipe_id = r.read_i32()?;
        let cost = r.read_i64()?;
        if cost < 0 {
            return None;
        }
        items.push(ManufactureLine { recipe_id, cost });
    }
    Some(items)
}

/// `RequestRecipeShopMakeItem` (0xBF): `manufacturerId(int)`, `recipeId(int)`,
/// then an unused long.
pub fn read_recipe_shop_make_item(body: &[u8]) -> Option<(i32, i32)> {
    let mut r = PacketReader::new(body);
    let manufacturer = r.read_i32()?;
    let recipe_id = r.read_i32()?;
    let _unknown = r.read_i64()?;
    Some((manufacturer, recipe_id))
}

/// `RequestRecipeShopMakeInfo` (0xBE): `playerObjectId(int)`, `recipeId(int)`.
pub fn read_recipe_shop_make_info(body: &[u8]) -> Option<(i32, i32)> {
    let mut r = PacketReader::new(body);
    Some((r.read_i32()?, r.read_i32()?))
}

/// The single-int recipe packets (`RequestRecipeBookDestroy` 0xB6,
/// `RequestRecipeItemMakeInfo` 0xB7, `RequestRecipeItemMakeSelf` 0xB8): one int.
pub fn read_recipe_single_int(body: &[u8]) -> Option<i32> {
    PacketReader::new(body).read_i32()
}

/// `RequestRecipeShopMessageSet` (0xBA): the store title string.
pub fn read_recipe_shop_message_set(body: &[u8]) -> Option<String> {
    PacketReader::new(body).read_string()
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
        let pairs: Vec<(i32, i32)> = (0..RequestSaveInventoryOrder::LIMIT as i32 + 10)
            .map(|i| (2000 + i, i))
            .collect();
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

/// `RequestDuelStart` — the challenged player's name and the party-duel flag.
pub fn read_duel_start(body: &[u8]) -> Option<(String, i32)> {
    let mut r = PacketReader::new(body);
    let name = r.read_string()?;
    let party_duel = r.read_i32().unwrap_or(0);
    Some((name, party_duel))
}

/// `RequestDuelAnswerStart` — reads `partyDuel`, an unused field, then the
/// response (1 accepts, anything else declines).
pub fn read_duel_answer(body: &[u8]) -> Option<i32> {
    let mut r = PacketReader::new(body);
    let _party_duel = r.read_i32()?;
    let _unused = r.read_i32().unwrap_or(0);
    Some(r.read_i32().unwrap_or(0))
}

/// Port of `clientpackets/RequestActionUse` — the action bar's non-skill
/// buttons (sit/stand, socials, and the servitor commands).
#[derive(Debug, Clone, Copy)]
pub struct RequestActionUse {
    pub action_id: i32,
    pub ctrl_pressed: bool,
    pub shift_pressed: bool,
}

impl RequestActionUse {
    pub fn read(body: &[u8]) -> Option<Self> {
        let mut r = commons::network::PacketReader::new(body);
        let action_id = r.read_i32()?;
        let ctrl_pressed = r.read_i32()? == 1;
        let shift_pressed = r.read_u8()? == 1;
        Some(Self {
            action_id,
            ctrl_pressed,
            shift_pressed,
        })
    }
}
