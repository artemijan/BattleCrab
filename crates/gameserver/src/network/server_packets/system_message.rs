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
    // Community board (G30)
    pub const THE_COMMUNITY_SERVER_IS_CURRENTLY_OFFLINE: i16 = 938;
    // Clans (G11)
    pub const S1_ALREADY_EXISTS: i16 = 5;
    pub const YOUR_CLAN_HAS_BEEN_CREATED: i16 = 189;
    pub const YOU_HAVE_FAILED_TO_CREATE_A_CLAN: i16 = 190;
    pub const YOU_DO_NOT_MEET_THE_CRITERIA_IN_ORDER_TO_CREATE_A_CLAN: i16 = 229;
    pub const YOU_MUST_WAIT_10_DAYS_BEFORE_CREATING_A_NEW_CLAN: i16 = 230;
    pub const CLAN_NAME_IS_INVALID: i16 = 261;
    pub const CLAN_NAME_S_LENGTH_IS_INCORRECT: i16 = 262;
    // Cursed weapons (`//cw_*`)
    pub const YOU_HAVE_EQUIPPED_YOUR_S1: i16 = 49;
    pub const EMPTY_3: i16 = 490;
    pub const THE_OWNER_OF_S2_HAS_APPEARED_IN_THE_S1_REGION: i16 = 1816;
    pub const S1_HAS_DISAPPEARED: i16 = 1818;
    // Castle sieges (`//castlemanage`)
    pub const YOU_HAVE_ALREADY_REQUESTED_A_CASTLE_SIEGE: i16 = 638;
    pub const THE_S1_SIEGE_HAS_STARTED: i16 = 711;
    pub const THE_S1_SIEGE_HAS_FINISHED: i16 = 712;
    pub const CLAN_S1_IS_VICTORIOUS_OVER_S2_S_CASTLE_SIEGE: i16 = 291;
    pub const THE_SIEGE_OF_S1_HAS_ENDED_IN_A_DRAW: i16 = 856;
    // Clan admin (`//pledge`, `//give_clan_skills`)
    pub const S1_IS_NOT_A_CLAN_LEADER: i16 = 9;
    pub const THE_TARGET_MUST_BE_A_CLAN_MEMBER: i16 = 234;
    pub const THE_CLAN_SKILL_S1_HAS_BEEN_ADDED: i16 = 1788;
    pub const CLAN_HAS_DISPERSED: i16 = 193;
    pub const YOUR_CLAN_S_LEVEL_HAS_INCREASED: i16 = 274;
    pub const NOW_THAT_YOUR_CLAN_LEVEL_IS_ABOVE_LEVEL_5_IT_CAN_ACCUMULATE_CLAN_REPUTATION: i16 = 1771;
    // Quests (G11): "earned" for quest gives, vs the loot "obtained" trio.
    pub const YOU_HAVE_EARNED_S1_ADENA: i16 = 52;
    pub const YOU_HAVE_EARNED_S2_S1_S: i16 = 53;
    pub const YOU_HAVE_EARNED_S1: i16 = 54;
    // Multisell (G30): the enchanted-product ack and the ingredient-shortfall
    // message.
    pub const ACQUIRED_S1_S2: i16 = 371;
    pub const YOU_NEED_S2_S1_S: i16 = 2961;
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
    /// "$c1 has resisted your $s2." — a debuff failed its landing roll
    /// (`Formulas.calcEffectSuccess`). Params: `[Text(targetName), SkillName]`.
    pub const C1_HAS_RESISTED_YOUR_S2: i16 = 139;
    /// "Drain was only 50% successful." — `calcMagicDam`'s half-damage branch
    /// on an HP-drain skill.
    pub const DRAIN_WAS_ONLY_50_SUCCESSFUL: i16 = 156;
    /// "You resisted $c1's drain." — shown to a *player target* that resisted
    /// an incoming drain. Params: `[Text(casterName)]`.
    pub const YOU_RESISTED_C1_S_DRAIN: i16 = 157;
    /// "Your attack has failed." — `calcMagicDam`'s half-damage branch on a
    /// non-drain magic skill.
    pub const YOUR_ATTACK_HAS_FAILED: i16 = 158;
    /// "You resisted $c1's magic." — shown to a *player target* that resisted
    /// an incoming spell. Params: `[Text(casterName)]`.
    pub const YOU_RESISTED_C1_S_MAGIC: i16 = 159;
    pub const YOU_HAVE_ACQUIRED_S1_SP: i16 = 331;
    pub const YOUR_SP_HAS_DECREASED_BY_S1: i16 = 538;
    pub const YOUR_XP_HAS_DECREASED_BY_S1: i16 = 539;
    pub const C1_HAS_EVADED_C2_S_ATTACK: i16 = 2264;
    pub const C1_S_ATTACK_WENT_ASTRAY: i16 = 2265;
    pub const C1_LANDED_A_CRITICAL_HIT: i16 = 2266;
    /// `Player.sendDamageMessage`: shown to the attacker when the target
    /// silently blocks the hit (invul / HP-block) instead of the damage line.
    pub const THE_ATTACK_HAS_BEEN_BLOCKED: i16 = 1996;
    /// `Player.sendDamageMessage`: the attacker's line against a door / control
    /// tower — a plain damage number with no target name.
    pub const YOU_HIT_FOR_S1_DAMAGE: i16 = 35;
    pub const YOU_HAVE_ACQUIRED_S1_XP_BONUS_S2_AND_S3_SP_BONUS_S4: i16 = 3259;
    pub const NOT_ENOUGH_HP: i16 = 23;
    pub const NOT_ENOUGH_MP: i16 = 24;
    // Crafting (G15.7).
    pub const S1_HAS_BEEN_ADDED: i16 = 851;
    pub const THAT_RECIPE_IS_ALREADY_REGISTERED: i16 = 840;
    pub const THE_RECIPE_CANNOT_BE_REGISTERED_YOU_DO_NOT_HAVE_THE_ABILITY_TO_CREATE_ITEMS: i16 = 1061;
    pub const YOUR_CREATE_ITEM_LEVEL_IS_TOO_LOW_TO_REGISTER_THIS_RECIPE: i16 = 404;
    pub const UP_TO_S1_RECIPES_CAN_BE_REGISTERED: i16 = 894;
    pub const YOU_MAY_NOT_ALTER_YOUR_RECIPE_BOOK_WHILE_ENGAGED_IN_MANUFACTURING: i16 = 853;
    pub const YOU_NEED_S2_MORE_S1_S: i16 = 854;
    pub const YOU_FAILED_AT_MIXING_THE_ITEM: i16 = 719;
    pub const S1_DISAPPEARED: i16 = 302;
    pub const S2_S1_S_DISAPPEARED: i16 = 301;
    pub const WHILE_YOU_ARE_ENGAGED_IN_COMBAT_YOU_CANNOT_OPERATE_A_PRIVATE_STORE_OR_PRIVATE_WORKSHOP: i16 = 1135;
    pub const S2_HAS_BEEN_CREATED_FOR_C1_AFTER_THE_PAYMENT_OF_S3_ADENA_WAS_RECEIVED: i16 = 1145;
    pub const C1_CREATED_S2_AFTER_RECEIVING_S3_ADENA: i16 = 1146;
    pub const S3_S2_S_HAVE_BEEN_CREATED_FOR_C1_AT_THE_PRICE_OF_S4_ADENA: i16 = 1147;
    pub const C1_CREATED_S3_S2_S_AT_THE_PRICE_OF_S4_ADENA: i16 = 1148;
    pub const YOU_FAILED_TO_CREATE_S2_FOR_C1_AT_THE_PRICE_OF_S3_ADENA: i16 = 1149;
    pub const C1_HAS_FAILED_TO_CREATE_S2_AT_THE_PRICE_OF_S3_ADENA: i16 = 1150;
    /// `OpenCommonRecipeBook`/`OpenDwarfRecipeBook` — a craft skill used while a
    /// private store is up.
    pub const ITEM_CREATION_IS_NOT_POSSIBLE_WHILE_ENGAGED_IN_A_TRADE: i16 = 1125;
    pub const YOUR_CASTING_HAS_BEEN_INTERRUPTED: i16 = 27;
    pub const YOU_USE_S1: i16 = 46;
    pub const S1_IS_NOT_AVAILABLE_REUSE: i16 = 48;
    pub const INVALID_TARGET: i16 = 109;
    /// Spoil / Sweeper (`Spoil`/`Sweeper` effects, `ConditionPlayerCanSweep`).
    pub const SWEEPER_FAILED_TARGET_NOT_SPOILED: i16 = 343;
    pub const IT_HAS_ALREADY_BEEN_SPOILED: i16 = 357;
    pub const THE_SPOIL_CONDITION_HAS_BEEN_ACTIVATED: i16 = 612;
    pub const THERE_ARE_NO_PRIORITY_RIGHTS_ON_A_SWEEPER: i16 = 683;
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
    // Siege zone (combat zone) enter/exit
    pub const YOU_HAVE_ENTERED_A_COMBAT_ZONE: i16 = 283;
    pub const YOU_HAVE_LEFT_A_COMBAT_ZONE: i16 = 284;
    // Skill acquisition (G13.9)
    pub const YOU_DO_NOT_HAVE_ENOUGH_SP_TO_LEARN_THIS_SKILL: i16 = 278;
    pub const YOU_DO_NOT_MEET_THE_SKILL_LEVEL_REQUIREMENTS: i16 = 2208;
    // Shop (G12)
    pub const YOU_DO_NOT_HAVE_ENOUGH_ADENA: i16 = 279;
    // Henna / dye symbols (G16).
    pub const THE_SYMBOL_HAS_BEEN_ADDED: i16 = 877;
    pub const THE_SYMBOL_HAS_BEEN_DELETED: i16 = 878;
    pub const THE_SYMBOL_CANNOT_BE_DRAWN: i16 = 899;
    pub const NO_SLOT_EXISTS_TO_DRAW_THE_SYMBOL: i16 = 900;
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
    // Clan membership lifecycle (G18 slice 1)
    pub const YOU_CANNOT_ASK_YOURSELF_TO_APPLY_TO_A_CLAN: i16 = 4;
    pub const S1_IS_ALREADY_A_MEMBER_OF_ANOTHER_CLAN: i16 = 10;
    pub const CLAN_MEMBER_S1_HAS_BEEN_EXPELLED: i16 = 191;
    pub const ENTERED_THE_CLAN: i16 = 195;
    pub const YOU_HAVE_WITHDRAWN_FROM_THE_CLAN: i16 = 197;
    pub const YOU_HAVE_RECENTLY_BEEN_DISMISSED_FROM_A_CLAN: i16 = 199;
    pub const YOU_ARE_NOT_A_CLAN_MEMBER_AND_CANNOT_PERFORM_THIS_ACTION: i16 = 212;
    pub const S1_HAS_JOINED_THE_CLAN: i16 = 222;
    pub const S1_HAS_WITHDRAWN_FROM_THE_CLAN: i16 = 223;
    pub const S1_DID_NOT_RESPOND_INVITATION_TO_THE_CLAN_HAS_BEEN_CANCELLED: i16 = 224;
    pub const YOU_DIDN_T_RESPOND_TO_S1_S_INVITATION_JOINING_HAS_BEEN_CANCELLED: i16 = 225;
    pub const AFTER_A_CLAN_MEMBER_IS_DISMISSED_THE_CLAN_MUST_WAIT_A_DAY: i16 = 231;
    pub const AFTER_LEAVING_A_CLAN_YOU_MUST_WAIT_A_DAY_BEFORE_JOINING_ANOTHER: i16 = 232;
    pub const THE_CLAN_IS_FULL: i16 = 233;
    pub const A_CLAN_LEADER_CANNOT_WITHDRAW_FROM_THEIR_OWN_CLAN: i16 = 239;
    pub const YOU_HAVE_ALREADY_REQUESTED_THE_DISSOLUTION_OF_YOUR_CLAN: i16 = 263;
    pub const YOU_CANNOT_DISSOLVE_A_CLAN_WHILE_ENGAGED_IN_A_WAR: i16 = 264;
    pub const YOU_CANNOT_DISSOLVE_A_CLAN_DURING_A_SIEGE: i16 = 265;
    pub const YOU_CANNOT_DISSOLVE_A_CLAN_WHILE_OWNING_A_CLAN_HALL_OR_CASTLE: i16 = 266;
    pub const YOU_CANNOT_DISMISS_YOURSELF: i16 = 269;
    pub const YOU_HAVE_SUCCEEDED_IN_EXPELLING_THE_CLAN_MEMBER: i16 = 309;
    pub const YOU_CANNOT_DISPERSE_THE_CLANS_IN_YOUR_ALLIANCE: i16 = 554;
    pub const C1_CANNOT_JOIN_THE_CLAN_ONE_DAY_HAS_NOT_PASSED_SINCE_LEAVING: i16 = 760;
    pub const YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT: i16 = 794;
    pub const YOU_CANNOT_LEAVE_A_CLAN_WHILE_ENGAGED_IN_COMBAT: i16 = 1116;
    pub const A_CLAN_MEMBER_MAY_NOT_BE_DISMISSED_DURING_COMBAT: i16 = 1117;
    pub const IN_ORDER_TO_JOIN_THE_CLAN_ACADEMY_YOU_MUST_BE_UNAFFILIATED: i16 = 1734;
    pub const S1_DOES_NOT_MEET_THE_REQUIREMENTS_TO_JOIN_A_CLAN_ACADEMY: i16 = 1735;
    pub const S1_IS_FULL_AND_CANNOT_ACCEPT_ADDITIONAL_CLAN_MEMBERS: i16 = 1835;
    // Recruitment registry (G18 slice 8)
    pub const YOU_MAY_APPLY_FOR_ENTRY_AFTER_S1_MINUTES_DUE_TO_CANCELLING: i16 = 4038;
    pub const ENTRY_APPLICATION_COMPLETE_AUTO_CANCELLED_AFTER_30_DAYS: i16 = 4039;
    pub const ENTRY_APPLICATION_CANCELLED_YOU_MAY_APPLY_AFTER_5_MINUTES: i16 = 4040;
    pub const ENTERED_INTO_WAITING_LIST_AUTO_DELETED_AFTER_30_DAYS: i16 = 4043;
    pub const ONLY_THE_CLAN_LEADER_OR_RANK_MANAGER_MAY_REGISTER_THE_CLAN: i16 = 4031;
    // Crests (G18 slice 7)
    pub const THE_SIZE_OF_THE_IMAGE_FILE_IS_INAPPROPRIATE_16X12: i16 = 209;
    pub const PLEASE_ADJUST_THE_IMAGE_SIZE_TO_8X12: i16 = 476;
    pub const A_CLAN_CREST_CAN_ONLY_BE_REGISTERED_WHEN_THE_CLAN_S_SKILL_LEVEL_IS_3_OR_ABOVE: i16 = 272;
    pub const AS_YOU_ARE_SCHEDULED_FOR_CLAN_DISSOLUTION_CANNOT_REGISTER_OR_DELETE_CREST: i16 = 552;
    pub const THE_CLAN_MARK_WAS_SUCCESSFULLY_REGISTERED_ON_ITEMS: i16 = 1663;
    pub const THE_CLAN_MARK_HAS_BEEN_DELETED: i16 = 1861;
    pub const THE_SIZE_OF_THE_UPLOADED_SYMBOL_DOES_NOT_MEET_STANDARDS: i16 = 3122;
    pub const THE_CREST_WAS_SUCCESSFULLY_REGISTERED: i16 = 3140;
    // Sub-pledges & academy (G18 slice 6)
    pub const YOUR_TITLE_CANNOT_EXCEED_16_CHARACTERS: i16 = 80;
    pub const TO_ESTABLISH_A_CLAN_ACADEMY_YOUR_CLAN_MUST_BE_LEVEL_5_OR_HIGHER: i16 = 1730;
    pub const CONGRATULATIONS_THE_S1_S_CLAN_ACADEMY_HAS_BEEN_CREATED: i16 = 1741;
    pub const THAT_PRIVILEGE_CANNOT_BE_GRANTED_TO_A_CLAN_ACADEMY_MEMBER: i16 = 1754;
    pub const THE_CONDITIONS_NECESSARY_TO_CREATE_A_MILITARY_UNIT_HAVE_NOT_BEEN_MET: i16 = 1791;
    pub const THE_KNIGHTS_OF_S1_HAVE_BEEN_CREATED: i16 = 1794;
    pub const THE_ROYAL_GUARD_OF_S1_HAVE_BEEN_CREATED: i16 = 1795;
    pub const C1_HAS_BEEN_SELECTED_AS_THE_CAPTAIN_OF_S2: i16 = 1793;
    pub const THE_ROYAL_GUARD_CAPTAIN_CANNOT_BE_APPOINTED: i16 = 1851;
    pub const THE_CAPTAIN_OF_THE_ORDER_OF_KNIGHTS_CANNOT_BE_APPOINTED: i16 = 1850;
    pub const ANOTHER_MILITARY_UNIT_ALREADY_USES_THAT_NAME: i16 = 1855;
    pub const YOUR_CLAN_HAS_ALREADY_ESTABLISHED_A_CLAN_ACADEMY: i16 = 1738;
    // Alliances (G18 slice 5)
    pub const THIS_FEATURE_IS_ONLY_AVAILABLE_TO_ALLIANCE_LEADERS: i16 = 464;
    pub const YOU_ARE_NOT_CURRENTLY_ALLIED_WITH_ANY_CLANS: i16 = 465;
    pub const YOU_HAVE_EXCEEDED_THE_LIMIT: i16 = 466;
    pub const MAY_NOT_ACCEPT_ANY_CLAN_WITHIN_A_DAY_AFTER_EXPELLING: i16 = 467;
    pub const WITHDRAWN_OR_EXPELLED_CLAN_CANNOT_ENTER_ALLIANCE_FOR_A_DAY: i16 = 468;
    pub const YOU_MAY_NOT_ALLY_WITH_A_CLAN_YOU_ARE_AT_WAR_WITH: i16 = 469;
    pub const ONLY_THE_CLAN_LEADER_MAY_APPLY_FOR_WITHDRAWAL_FROM_THE_ALLIANCE: i16 = 470;
    pub const ALLIANCE_LEADERS_CANNOT_WITHDRAW: i16 = 471;
    pub const DIFFERENT_ALLIANCE: i16 = 473;
    pub const THAT_CLAN_DOES_NOT_EXIST: i16 = 474;
    pub const NO_RESPONSE_INVITATION_TO_JOIN_AN_ALLIANCE_HAS_BEEN_CANCELLED: i16 = 477;
    pub const NO_RESPONSE_YOUR_ENTRANCE_TO_THE_ALLIANCE_HAS_BEEN_CANCELLED: i16 = 478;
    pub const ALLIANCE_INFORMATION: i16 = 491;
    pub const ALLIANCE_NAME_S1: i16 = 492;
    pub const CONNECTION_S1_TOTAL_S2: i16 = 493;
    pub const ALLIANCE_LEADER_S2_OF_S1: i16 = 494;
    pub const AFFILIATED_CLANS_TOTAL_S1_CLAN_S: i16 = 495;
    pub const CLAN_INFORMATION: i16 = 496;
    pub const CLAN_NAME_S1: i16 = 497;
    pub const CLAN_LEADER_S1: i16 = 498;
    pub const CLAN_LEVEL_S1: i16 = 499;
    pub const EMPTY_4: i16 = 500;
    pub const YOU_ALREADY_BELONG_TO_ANOTHER_ALLIANCE: i16 = 502;
    pub const ONLY_CLAN_LEADERS_MAY_CREATE_ALLIANCES: i16 = 504;
    pub const CANNOT_CREATE_A_NEW_ALLIANCE_WITHIN_1_DAY_OF_DISSOLUTION: i16 = 505;
    pub const INCORRECT_ALLIANCE_NAME: i16 = 506;
    pub const INCORRECT_LENGTH_FOR_AN_ALLIANCE_NAME: i16 = 507;
    pub const THAT_ALLIANCE_NAME_ALREADY_EXISTS: i16 = 508;
    pub const YOU_HAVE_ACCEPTED_THE_ALLIANCE: i16 = 517;
    pub const YOU_HAVE_WITHDRAWN_FROM_THE_ALLIANCE: i16 = 519;
    pub const YOU_HAVE_SUCCEEDED_IN_EXPELLING_THE_CLAN: i16 = 521;
    pub const THE_ALLIANCE_HAS_BEEN_DISSOLVED: i16 = 523;
    pub const SUCCESSFULLY_ADDED_TO_YOUR_FRIEND_LIST: i16 = 525;
    pub const S1_LEADER_S2_HAS_REQUESTED_AN_ALLIANCE: i16 = 527;
    pub const TO_CREATE_AN_ALLIANCE_YOUR_CLAN_MUST_BE_LEVEL_5_OR_HIGHER: i16 = 549;
    pub const SCHEDULED_FOR_CLAN_DISSOLUTION_NO_ALLIANCE_CAN_BE_CREATED: i16 = 550;
    pub const S1_CLAN_IS_ALREADY_A_MEMBER_OF_S2_ALLIANCE: i16 = 691;
    pub const CANNOT_DISSOLVE_ALLIANCE_WHILE_AFFILIATED_CLAN_IN_SIEGE: i16 = 722;
    pub const THE_OPPOSING_CLAN_IS_PARTICIPATING_IN_A_SIEGE_BATTLE: i16 = 723;
    pub const S1_CLAN_CANNOT_JOIN_ALLIANCE_ONE_DAY_NOT_PASSED: i16 = 761;
    pub const YOU_ARE_NOT_IN_AN_ALLIANCE: i16 = 4203;
    // Clan wars (G18 slice 4)
    pub const CLAN_WAR_STARTED_WITH_CLAN_S1: i16 = 215; // "the clan that cancels the war first will lose 500 reputation…"
    pub const YOUR_CLAN_LOST_500_REPUTATION_FOR_WITHDRAWING_FROM_THE_WAR: i16 = 260;
    pub const S1_HAS_DECLARED_A_CLAN_WAR_KILL_5_TO_START: i16 = 1561;
    pub const YOU_HAVE_DECLARED_A_CLAN_WAR_WITH_S1: i16 = 1562;
    pub const CLAN_WAR_NEEDS_LEVEL_3_AND_15_MEMBERS: i16 = 1564;
    pub const CLAN_WAR_TARGET_DOES_NOT_EXIST: i16 = 1565;
    pub const CANNOT_DECLARE_WAR_ON_ALLIED_CLAN: i16 = 1569;
    pub const CANNOT_DECLARE_WAR_ON_MORE_THAN_30_CLANS: i16 = 1570;
    pub const FOOL_YOU_CANNOT_DECLARE_WAR_AGAINST_YOUR_OWN_CLAN: i16 = 1610;
    pub const CEASE_FIRE_CANNOT_BE_CALLED_WHILE_MEMBERS_IN_BATTLE: i16 = 1677;
    pub const YOU_HAVE_NOT_DECLARED_A_CLAN_WAR_AGAINST_THE_CLAN_S1: i16 = 1678;
    pub const CANNOT_DECLARE_WAR_ON_DISSOLVING_CLAN: i16 = 1684;
    pub const THE_CLAN_REPUTATION_IS_TOO_LOW: i16 = 1860;
    pub const CANNOT_DECLARE_DEFEAT_BEFORE_7_DAYS_WITH_CLAN_S1: i16 = 3283;
    pub const THE_WAR_ENDED_BY_YOUR_DEFEAT_DECLARATION_WITH_THE_S1_CLAN: i16 = 3284;
    pub const THE_WAR_ENDED_BY_THE_S1_CLAN_S_DEFEAT_DECLARATION: i16 = 3285;
    pub const CANNOT_DECLARE_WAR_21_DAYS_AFTER_DEFEAT_WITH_S1: i16 = 3286;
    pub const BECAUSE_C1_KILLED_BY_S2_CLAN_REPUTATION_DECREASED_BY_1: i16 = 3811;
    pub const BECAUSE_S1_MEMBER_KILLED_BY_C2_CLAN_REPUTATION_INCREASED_BY_1: i16 = 3812;
    pub const BECAUSE_CLAN_S1_DID_NOT_FIGHT_BACK_THE_WAR_WAS_CANCELLED: i16 = 3813;
    pub const A_CLAN_WAR_DECLARED_BY_CLAN_S1_WAS_CANCELLED: i16 = 3814;
    pub const S1_MEMBER_KILLED_S2_MORE_KILLS_TO_START_WAR: i16 = 3815;
    // Clan ranks + leader transfer (G18 slice 3)
    pub const S1_DOES_NOT_EXIST: i16 = 2;
    pub const THAT_PLAYER_IS_NOT_CURRENTLY_ONLINE: i16 = 161;
    pub const CLAN_MEMBER_C1_S_PRIVILEGE_LEVEL_HAS_BEEN_CHANGED_TO_S2: i16 = 1761;
    // Clan level-up + pledge skill learning (G18 slice 2)
    pub const AS_YOU_ARE_SCHEDULED_FOR_CLAN_DISSOLUTION_LEVEL_CANNOT_INCREASE: i16 = 551;
    pub const YOU_DO_NOT_HAVE_ANY_FURTHER_SKILLS_TO_LEARN_COME_BACK_AT_LEVEL_S1: i16 = 607;
    pub const S1_ADENA_DISAPPEARED: i16 = 672;
    pub const S1_POINTS_HAVE_BEEN_DEDUCTED_FROM_THE_CLAN_S_REPUTATION: i16 = 1787;
    pub const THE_CONDITIONS_TO_INCREASE_THE_CLAN_S_LEVEL_HAVE_NOT_BEEN_MET: i16 = 1790;
    pub const SKILL_ACQUIRE_FAILED_INSUFFICIENT_CLAN_REPUTATION: i16 = 1852;
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
    /// "Your skill was deactivated due to lack of MP." — a toggle's MP upkeep
    /// tick failing (`ManaDamOverTime`).
    pub const YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP: i16 = 140;
    /// "Over-hit!" — the notice for a killing blow that overshot. Java's
    /// companion message 362 (the bonus *amount*) is defined but never sent in
    /// this build, so it is not ported.
    pub const OVER_HIT: i16 = 361;
    // Duels (G20).
    pub const THERE_IS_NO_OPPONENT_TO_RECEIVE_YOUR_CHALLENGE_FOR_A_DUEL: i16 = 1926;
    pub const C1_HAS_BEEN_CHALLENGED_TO_A_DUEL: i16 = 1927;
    pub const C1_IS_TOO_FAR_AWAY_TO_RECEIVE_A_DUEL_CHALLENGE: i16 = 2028;
    pub const C1_HAS_DECLINED_YOUR_CHALLENGE_TO_A_DUEL: i16 = 1931;
    pub const YOU_ARE_UNABLE_TO_REQUEST_A_DUEL_AT_THIS_TIME: i16 = 1940;
    pub const THE_DUEL_WILL_BEGIN_IN_S1_SECOND_S: i16 = 1945;
    pub const LET_THE_DUEL_BEGIN: i16 = 1949;
    pub const C1_HAS_WON_THE_DUEL: i16 = 1950;
    pub const THE_DUEL_HAS_ENDED_IN_A_TIE: i16 = 1952;
    pub const C1_CANNOT_DUEL_BECAUSE_C1_S_HP_OR_MP_IS_BELOW_50: i16 = 2019;
    pub const C1_CANNOT_DUEL_BECAUSE_C1_IS_CURRENTLY_ENGAGED_IN_BATTLE: i16 = 2021;
    pub const C1_CANNOT_DUEL_BECAUSE_C1_IS_ALREADY_ENGAGED_IN_A_DUEL: i16 = 2022;
    // Ranged attacks (G20).
    pub const YOU_HAVE_RUN_OUT_OF_ARROWS: i16 = 112;
    pub const YOUR_CROSSBOW_IS_PREPARING_TO_FIRE: i16 = 2224;
    // Vitality (G16): the four `PlayerStat.setVitalityPoints` notices.
    pub const YOUR_VITALITY_IS_AT_MAXIMUM: i16 = 2314;
    pub const YOUR_VITALITY_HAS_INCREASED: i16 = 2315;
    pub const YOUR_VITALITY_HAS_DECREASED: i16 = 2316;
    pub const YOUR_VITALITY_IS_FULLY_EXHAUSTED: i16 = 2317;
    // Transformation (G19): `ConditionPlayerCanTransform`'s cast refusals.
    pub const YOU_ALREADY_POLYMORPHED_AND_CANNOT_POLYMORPH_AGAIN: i16 = 2058;
    pub const YOU_CANNOT_POLYMORPH_INTO_THE_DESIRED_FORM_IN_WATER: i16 = 2060;
    pub const YOU_CANNOT_TRANSFORM_WHILE_RIDING_A_PET: i16 = 2063;
    // Force/charges (G19): `FocusMomentum`/`GetMomentum`.
    pub const YOUR_FORCE_HAS_INCREASED_TO_LEVEL_S1: i16 = 323;
    pub const YOUR_FORCE_HAS_REACHED_MAXIMUM_CAPACITY: i16 = 324;
    // Lethal (G19): `Lethal.instant`'s outcome messages.
    pub const LETHAL_STRIKE: i16 = 1667;
    pub const HIT_WITH_LETHAL_STRIKE: i16 = 1668;
    pub const HALF_KILL: i16 = 2336;
    pub const YOUR_CP_WAS_DRAINED_BECAUSE_YOU_WERE_HIT_WITH_A_HALF_KILL_SKILL: i16 = 2337;
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
    /// `TYPE_CASTLE_NAME` (5) — `addCastleId` (the client resolves the name).
    CastleName(i32),
    /// `TYPE_LONG_NUMBER` (6) — `addLong`.
    Long(i64),
    /// `TYPE_PLAYER_NAME` (12) — `addPcName`.
    PlayerName(String),
    /// `TYPE_SYSTEM_STRING` (13) — `addSystemString` (sysstring-e.dat id).
    SysString(i32),
    /// `TYPE_POPUP_ID` (16) — `addPopup(target, attacker, damage)`. Mobius's
    /// on-screen floating damage number: the client draws it over `target`'s
    /// head when the "show damage" client option is enabled. `damage` is passed
    /// negative (`-damage`) exactly as `Player.sendDamageMessage` does.
    Popup { target: i32, attacker: i32, damage: i32 },
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
            SmParam::CastleName(castle_id) => {
                w.write_u8(5);
                w.write_i32(*castle_id);
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
            SmParam::Popup { target, attacker, damage } => {
                w.write_u8(16);
                w.write_i32(*target);
                w.write_i32(*attacker);
                w.write_i32(*damage);
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
