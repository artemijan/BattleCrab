//! `SystemMessage` packet, its parameter types, and the `SystemMessageId`
//! constants the handlers emit.

use commons::network::PacketWriter;

use super::opcodes;

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
    // Clan admin (`//pledge`)
    pub const S1_IS_NOT_A_CLAN_LEADER: i16 = 9;
    pub const CLAN_HAS_DISPERSED: i16 = 193;
    pub const YOUR_CLAN_S_LEVEL_HAS_INCREASED: i16 = 274;
    pub const NOW_THAT_YOUR_CLAN_LEVEL_IS_ABOVE_LEVEL_5_IT_CAN_ACCUMULATE_CLAN_REPUTATION: i16 = 1771;
    // Quests (G11): "earned" for quest gives, vs the loot "obtained" trio.
    pub const YOU_HAVE_EARNED_S1_ADENA: i16 = 52;
    pub const YOU_HAVE_EARNED_S2_S1_S: i16 = 53;
    pub const YOU_HAVE_EARNED_S1: i16 = 54;
    pub const YOU_HAVE_OBTAINED_S1_ADENA: i16 = 28;
    pub const YOU_HAVE_OBTAINED_S2_S1: i16 = 29;
    pub const YOU_HAVE_OBTAINED_S1: i16 = 30;
    /// "You have obtained a +$s1 $s2." — the enchant-carrying grant message
    /// (`Restoration`/`RestorationRandom` when the created item is enchanted).
    pub const YOU_HAVE_OBTAINED_A_S1_S2: i16 = 369;
    // Item use (G14): `ExtractableItems` (pack/box unpacking).
    pub const THERE_WAS_NOTHING_FOUND_INSIDE: i16 = 1669;
    pub const YOUR_INVENTORY_IS_FULL: i16 = 129;
    pub const YOU_HAVE_AVOIDED_C1_S_ATTACK: i16 = 42;
    pub const YOU_HAVE_MISSED: i16 = 43;
    pub const CRITICAL_HIT: i16 = 44;
    pub const YOUR_LEVEL_HAS_INCREASED: i16 = 96;
    /// Java `SystemMessage(String)` / `Player.sendMessage(String)` — a bare
    /// `$s1` text line (`SystemMessageId.S1_2`).
    pub const S1_TEXT: i16 = 1983;
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
    /// "Your shield defense has succeeded." (Interlude has no separate perfect-
    /// block message; the perfect block reuses this.)
    pub const SHIELD_DEFENSE_SUCCEEDED: i16 = 111;
    pub const NOTHING_HAPPENED: i16 = 61;
    pub const CANNOT_SEE_TARGET: i16 = 181;
    // GM silence / message refusal (G13.B)
    pub const THAT_PERSON_IS_IN_MESSAGE_REFUSAL_MODE: i16 = 176;
    pub const MESSAGE_REFUSAL_MODE: i16 = 177;
    pub const MESSAGE_ACCEPTANCE_MODE: i16 = 178;
    // Zones (G12)
    pub const YOU_MAY_NOT_ATTACK_THIS_TARGET_IN_A_PEACEFUL_ZONE: i16 = 85;
    pub const YOU_CANNOT_USE_SKILLS_THAT_MAY_HARM_OTHER_PLAYERS_IN_HERE: i16 = 2167;
    // Skill acquisition (G13.9)
    pub const YOU_DO_NOT_HAVE_ENOUGH_SP_TO_LEARN_THIS_SKILL: i16 = 278;
    pub const YOU_DO_NOT_MEET_THE_SKILL_LEVEL_REQUIREMENTS: i16 = 2208;
    // Shop (G12)
    pub const YOU_DO_NOT_HAVE_ENOUGH_ADENA: i16 = 279;
    // User commands (G15.5)
    /// "Current Location: $s1" — the `/loc` fallback when the map region has
    /// no `locId` message.
    pub const CURRENT_LOCATION_S1: i16 = 2361;
    pub const YOU_HAVE_EXCEEDED_THE_QUANTITY_THAT_CAN_BE_INPUTTED: i16 = 1036;
    pub const EXCHANGE_IS_SUCCESSFUL: i16 = 4358;
    pub const DISTANCE_TOO_FAR_CASTING_CANCELLED: i16 = 748;
    pub const YOUR_TARGET_IS_OUT_OF_RANGE: i16 = 22;
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
    // Soulshots / spiritshots
    pub const THE_SOULSHOT_YOU_ARE_ATTEMPTING_TO_USE_DOES_NOT_MATCH_THE_GRADE_OF_YOUR_EQUIPPED_WEAPON: i16 = 337;
    pub const YOU_DO_NOT_HAVE_ENOUGH_SOULSHOTS_FOR_THAT: i16 = 338;
    pub const CANNOT_USE_SOULSHOTS: i16 = 339;
    pub const YOUR_SOULSHOTS_ARE_ENABLED: i16 = 342;
    pub const YOUR_SPIRITSHOT_DOES_NOT_MATCH_THE_WEAPON_S_GRADE: i16 = 530;
    pub const YOU_DO_NOT_HAVE_ENOUGH_SPIRITSHOT_FOR_THAT: i16 = 531;
    pub const YOU_MAY_NOT_USE_SPIRITSHOTS: i16 = 532;
    pub const YOUR_SPIRITSHOT_HAS_BEEN_ENABLED: i16 = 533;
    pub const THE_AUTOMATIC_USE_OF_S1_HAS_BEEN_ACTIVATED: i16 = 1433;
    pub const THE_AUTOMATIC_USE_OF_S1_HAS_BEEN_DEACTIVATED: i16 = 1434;
    pub const DUE_TO_INSUFFICIENT_S1_THE_AUTOMATIC_USE_FUNCTION_HAS_BEEN_DEACTIVATED: i16 = 1435;
    pub const DUE_TO_INSUFFICIENT_S1_THE_AUTOMATIC_USE_FUNCTION_CANNOT_BE_ACTIVATED: i16 = 1436;
    // Recommendations (RequestVoteNew / RecoGiveTask / GiveRecommendation)
    pub const SELECT_TARGET: i16 = 242;
    pub const THAT_IS_AN_INCORRECT_TARGET: i16 = 144;
    pub const YOU_CANNOT_RECOMMEND_YOURSELF: i16 = 829;
    pub const YOU_HAVE_RECOMMENDED_C1_YOU_HAVE_S2_RECOMMENDATIONS_LEFT: i16 = 830;
    pub const YOU_HAVE_BEEN_RECOMMENDED_BY_C1: i16 = 831;
    pub const YOUR_SELECTED_TARGET_CAN_NO_LONGER_RECEIVE_A_RECOMMENDATION: i16 = 1188;
    pub const YOU_ARE_OUT_OF_RECOMMENDATIONS_TRY_AGAIN_LATER: i16 = 3206;
    pub const YOU_OBTAINED_S1_RECOMMENDATION_S: i16 = 3207;
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

/// `SystemMessageId.S1_3` (id 1987, `"$s1"`) — the id the admin `ConfirmDlg`
/// uses, echoed back by the client in `DlgAnswer` so the reply can be matched
/// to its request.
pub const S1_3_MESSAGE_ID: i32 = 1987;

/// Port of `serverpackets/ConfirmDlg` for the admin-confirm case: an
/// `S1_3` message with a single text param (the "Are you sure…?" prompt).
///
/// Wire format differs from `SystemMessage`: the message id and parameter
/// count are 32-bit, each param's type tag is 32-bit (here `TYPE_TEXT` = 0),
/// and the packet ends with `time` then `requesterId` (both 0 for admin
/// confirms — no auto-decline timer, no requester object).
pub fn confirm_dlg_text(text: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CONFIRM_DLG);
    w.write_i32(S1_3_MESSAGE_ID);
    w.write_i32(1); // parameter count
    w.write_i32(0); // TYPE_TEXT
    w.write_string(text);
    w.write_i32(0); // time
    w.write_i32(0); // requesterId
    w.into_bytes()
}
