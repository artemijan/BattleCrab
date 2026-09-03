//! Extended (`0xD0`) client sub-opcodes.

pub const REQUEST_MANOR_LIST: u16 = 0x01;
/// `RequestProcureCropList` — a player sells crops to a Manor Manager.
pub const REQUEST_PROCURE_CROP_LIST: u16 = 0x02;
/// `RequestSetSeed` — the manor owner submits the next-period seed setup.
pub const REQUEST_SET_SEED: u16 = 0x03;
/// `RequestSetCrop` — the manor owner submits the next-period crop setup.
pub const REQUEST_SET_CROP: u16 = 0x04;
pub const REQUEST_KEY_MAPPING: u16 = 0x21;
/// `RequestExRqItemLink` — a reader clicked a shift-clicked item link in a
/// chat line; the body is that item's object id. Answered with
/// `ExRpItemLink`, without which the link stays a bare "?".
pub const REQUEST_EX_RQ_ITEM_LINK: u16 = 0x1E;
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
/// `EndScenePlayer` — the client's notice that a cinematic finished; the
/// body echoes the movie's client id.
pub const END_SCENE_PLAYER: u16 = 0x58;
/// `RequestExEscapeScene` — the player pressed Esc during an escapable
/// cinematic (empty body).
pub const REQUEST_EX_ESCAPE_SCENE: u16 = 0x90;
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
/// `RequestPledgeSetAcademyMaster` — pair/unpair an academy member with a
/// sponsor (G18.6).
pub const REQUEST_PLEDGE_SET_ACADEMY_MASTER: u16 = 0x12;
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
