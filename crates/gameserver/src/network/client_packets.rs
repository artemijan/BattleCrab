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
    pub const REQUEST_PRIVATE_STORE_MANAGE_SELL: u8 = 0x30;
    pub const SET_PRIVATE_STORE_LIST_SELL: u8 = 0x31;
    pub const REQUEST_PRIVATE_STORE_QUIT_SELL: u8 = 0x96;
    pub const REQUEST_PRIVATE_STORE_BUY: u8 = 0x83;
    pub const TRADE_REQUEST: u8 = 0x1A;
    pub const ADD_TRADE_ITEM: u8 = 0x1B;
    pub const TRADE_DONE: u8 = 0x1C;
    pub const ANSWER_TRADE_REQUEST: u8 = 0x55;
    pub const SEND_WARE_HOUSE_DEPOSIT_LIST: u8 = 0x3B;
    pub const SEND_WARE_HOUSE_WITH_DRAW_LIST: u8 = 0x3C;
    pub const ACTION: u8 = 0x1F;
    pub const REQUEST_MAGIC_SKILL_USE: u8 = 0x39;
    pub const REQUEST_TARGET_CANCELD: u8 = 0x48;
    pub const REQUEST_RESTART: u8 = 0x57;
    pub const VALIDATE_POSITION: u8 = 0x59;
    pub const REQUEST_ACQUIRE_SKILL: u8 = 0x7C;
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
    pub const REQUEST_QUEST_ABORT: u8 = 0x63;
    /// `RequestPledgeInfo` — asks for a clan's name/ally name by clan id.
    pub const REQUEST_PLEDGE_INFO: u8 = 0x65;
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
    pub const REQUEST_FRIEND_INVITE: u8 = 0x77;
    pub const REQUEST_ANSWER_FRIEND_INVITE: u8 = 0x78;
    pub const REQUEST_FRIEND_LIST: u8 = 0x79;
    pub const REQUEST_FRIEND_DEL: u8 = 0x7A;
    /// `MultiSellChoose` — a purchase/exchange click in the multisell window.
    pub const MULTI_SELL_CHOOSE: u8 = 0xB0;
    /// Extended packets: opcode 0xD0 + a 2-byte little-endian sub-opcode.
    pub const EX_PACKET: u8 = 0xD0;
}

/// Extended (`0xD0`) client sub-opcodes.
pub mod ex_opcodes {
    pub const REQUEST_MANOR_LIST: u16 = 0x01;
    pub const REQUEST_KEY_MAPPING: u16 = 0x21;
    pub const REQUEST_CHARACTER_NAME_CREATABLE: u16 = 0xA9;
    pub const REQUEST_USER_BAN_INFO: u16 = 0x138;
    /// `ExSendClientIni` — the client reports its `client.ini` after auth.
    /// Mobius registers a `null` handler (no packet class), so it is consumed
    /// and ignored.
    pub const EX_SEND_CLIENT_INI: u16 = 0x104;
    pub const REQUEST_GOTO_LOBBY: u16 = 0x33;
    pub const REQUEST_CHANGE_PARTY_LEADER: u16 = 0x0C;
    pub const REQUEST_PARTY_LOOT_MODIFICATION: u16 = 0x75;
    pub const ANSWER_PARTY_LOOT_MODIFICATION: u16 = 0x76;
    pub const REQUEST_SAVE_INVENTORY_ORDER: u16 = 0x24;
    pub const REQUEST_STOP_MOVE: u16 = 0xED;
    pub const EX_SEND_SELECTED_QUEST_ZONE_ID: u16 = 0xFF;
    pub const REQUEST_AUTO_SOULSHOT: u16 = 0x0D;
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
pub struct PrivateStoreItemList {
    /// `RequestPrivateStoreBuy` only: the seller's object id (`0` for a set-list).
    pub target_object_id: i32,
    pub items: Vec<(i32, i64, i64)>,
}

impl PrivateStoreItemList {
    /// `SetPrivateStoreListSell`: `packageSale(int)` then the item lines.
    pub fn read_set_list(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let _package = r.read_i32()?;
        Self::read_lines(&mut r, 0)
    }

    /// `RequestPrivateStoreBuy`: `storePlayerId(int)` then the item lines.
    pub fn read_buy(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let seller = r.read_i32()?;
        Self::read_lines(&mut r, seller)
    }

    fn read_lines(r: &mut PacketReader, target: i32) -> Option<Self> {
        let count = r.read_i32()?;
        if count < 1 || count > 500 {
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
        Some(Self { target_object_id: target, items })
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

/// Port of `clientpackets/MultiSellChoose`: the item exchange click. Reads the
/// full retail body (enchant/augment/elemental stats follow the amount), but
/// only `list_id`/`entry_id`/`amount` drive the community-board exchange path
/// (the enchant-maintenance validation is a `maintainEnchantment`-list concern,
/// TODO(G30)).
pub struct MultiSellChoose {
    pub list_id: i32,
    pub entry_id: i32,
    pub amount: i64,
}

impl MultiSellChoose {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let list_id = r.read_i32()?;
        let entry_id = r.read_i32()?;
        let amount = r.read_i64()?;
        // enchantLevel(short), augment1(int), augment2(int), attackAttribute
        // (short), attributePower(short), and six elemental defence shorts —
        // consumed to keep the reader honest even though this path ignores them.
        let _enchant_level = r.read_i16()?;
        let _augment1 = r.read_i32()?;
        let _augment2 = r.read_i32()?;
        for _ in 0..8 {
            let _ = r.read_i16()?;
        }
        Some(Self { list_id, entry_id, amount })
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
        Some(Self { object_id, count, x, y, z })
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
        Some(Self { object_id, skill_id, skill_level, skill_sub_level })
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
    PacketReader::new(body_after_opcode).read_string().map(|s| s.trim().to_string())
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
        Some(Self { message_id, answer, requester_id })
    }
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

/// The single-int henna packets (`RequestHennaEquip`/`Remove`/`ItemInfo`/
/// `ItemRemoveInfo`): one int `symbolId`.
pub fn read_symbol_id(body: &[u8]) -> Option<i32> {
    PacketReader::new(body).read_i32()
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
