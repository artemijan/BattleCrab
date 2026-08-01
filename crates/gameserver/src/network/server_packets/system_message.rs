//! `SystemMessage` packet, its parameter types, and the `SystemMessageId`
//! constants the handlers emit.

use commons::network::PacketWriter;

use super::opcodes;

/// The `SystemMessageId` constants the cast pipeline sends (Java's enum has
/// ~6800 — added as handlers need them; the zero-parameter welcome message
/// keeps using `enter_world::system_message`).
pub mod sm_ids {
    /// `Clan leader privileges have been transferred to $c1.`
    pub const CLAN_LEADER_PRIVILEGES_HAVE_BEEN_TRANSFERRED_TO_C1: i16 = 1798;
    // Mounts / wyvern
    pub const YOU_CANNOT_DISMOUNT_FROM_THIS_ELEVATION: i16 = 1158;
    pub const YOU_ARE_NOT_ALLOWED_TO_DISMOUNT_IN_THIS_LOCATION: i16 = 1385;
    /// `$s1 has been unequipped.` — the mount disarm.
    pub const S1_HAS_BEEN_UNEQUIPPED: i16 = 417;
    /// `The equipment, +$s1 $s2, has been removed.` — enchanted variant.
    pub const THE_EQUIPMENT_S1_S2_HAS_BEEN_REMOVED: i16 = 1064;
    pub const YOU_MAY_CREATE_UP_TO_48_MACROS: i16 = 797;
    pub const INVALID_MACRO_REFER_TO_THE_HELP_FILE_FOR_INSTRUCTIONS: i16 = 810;
    pub const MACRO_DESCRIPTIONS_MAY_CONTAIN_UP_TO_32_CHARACTERS: i16 = 837;
    pub const ENTER_THE_NAME_OF_THE_MACRO: i16 = 838;
    // User commands (`/time`, `/partyinfo`, `/channelinfo`, …) — G15.5 tail.
    /// `The current time is $s1:$s2.` (day) and its night variant.
    pub const THE_CURRENT_TIME_IS_S1_S2: i16 = 927;
    pub const THE_CURRENT_TIME_IS_S1_S2_NIGHT: i16 = 928;
    pub const PARTY_INFORMATION: i16 = 1030;
    pub const LOOTING_METHOD_FINDERS_KEEPERS: i16 = 1031;
    pub const LOOTING_METHOD_RANDOM: i16 = 1032;
    pub const LOOTING_METHOD_RANDOM_INCLUDING_SPOIL: i16 = 1033;
    pub const LOOTING_METHOD_BY_TURN: i16 = 1034;
    pub const LOOTING_METHOD_BY_TURN_INCLUDING_SPOIL: i16 = 1035;
    pub const CLANS_YOU_VE_DECLARED_WAR_ON: i16 = 1571;
    pub const CLANS_THAT_HAVE_DECLARED_WAR_ON_YOU: i16 = 1572;
    pub const NOT_JOINED_IN_ANY_CLAN: i16 = 238;
    /// `$s1 ($s2 alliance)` / `$s1 (no alliance exists)` — one clan-war row.
    pub const S1_S2_ALLIANCE: i16 = 1200;
    pub const S1_NO_ALLIANCE_EXISTS: i16 = 1202;
    pub const ONLY_A_PARTY_LEADER_CAN_LEAVE_A_COMMAND_CHANNEL: i16 = 1683;
    pub const ONLY_A_NOBLE_CLAN_LEADER_CAN_VIEW_THE_SIEGE_STATUS: i16 = 1694;
    pub const COMMAND_AVAILABLE_AFTER_THE_2ND_CLASS_TRANSFER: i16 = 1674;
    pub const FOR_THE_CURRENT_OLYMPIAD_YOU_HAVE_PARTICIPATED: i16 = 1673;
    pub const THE_MATCHES_THIS_WEEK_ARE_ALL_CLASS_BATTLES: i16 = 3261;
    pub const C1_S_BIRTHDAY_IS_S3_S4_S2: i16 = 2450;
    // `/mount` — the strider gate ladder (Java `Player.mountPlayer`).
    pub const A_HUNGRY_STRIDER_CANNOT_BE_MOUNTED_OR_DISMOUNTED: i16 = 1008;
    pub const A_STRIDER_CANNOT_BE_RIDDEN_WHEN_DEAD: i16 = 1009;
    pub const A_DEAD_STRIDER_CANNOT_BE_RIDDEN: i16 = 1010;
    pub const A_STRIDER_IN_BATTLE_CANNOT_BE_RIDDEN: i16 = 1011;
    pub const A_STRIDER_CANNOT_BE_RIDDEN_WHILE_IN_BATTLE: i16 = 1012;
    pub const A_STRIDER_CAN_BE_RIDDEN_ONLY_WHEN_STANDING: i16 = 1013;
    /// "You cannot transform while sitting."
    pub const YOU_CANNOT_TRANSFORM_WHILE_SITTING: i16 = 2283;
    /// "You cannot open a Private Store here." — `canOpenPrivateStore`'s
    /// `Custom/PrivateStoreRange.ini` spacing rule.
    pub const YOU_CANNOT_OPEN_A_PRIVATE_STORE_HERE: i16 = 1296;
    /// "You can't build headquarters here." — `BuildCampSkillCondition`'s
    /// `isInsideZone(ZoneId.HQ)` gate, the only one of its branches with a
    /// message of its own.
    pub const YOU_CAN_T_BUILD_HEADQUARTERS_HERE: i16 = 290;
    /// "You can't fish here." — `Fishing.castLine` with no FishingZone/WaterZone.
    pub const YOU_CAN_T_FISH_HERE: i16 = 1457;
    pub const YOU_ARE_TOO_FAR_AWAY_FROM_YOUR_MOUNT_TO_RIDE: i16 = 1846;
    /// "You are out of feed. Mount status canceled." — the mount feed task's
    /// force-dismount when the gauge empties.
    pub const YOU_ARE_OUT_OF_FEED_MOUNT_STATUS_CANCELED: i16 = 1248;
    /// `That character does not exist.` — the freight send with no other
    /// character on the account.
    pub const THAT_CHARACTER_DOES_NOT_EXIST: i16 = 873;
    // Augment window confirm steps (row 11).
    /// `This is not a suitable item.`
    pub const THIS_IS_NOT_A_SUITABLE_ITEM: i16 = 1960;
    /// `Augmentation removal can only be done on an augmented item.`
    pub const AUGMENTATION_REMOVAL_ONLY_ON_AN_AUGMENTED_ITEM: i16 = 1964;
    // Datapack transfer restrictions (`is_dropable` / `is_tradable` /
    // `is_destroyable`), the refusals Java's item handlers send.
    /// `That item cannot be discarded.` (Java `RequestDropItem`.)
    pub const THAT_ITEM_CANNOT_BE_DISCARDED: i16 = 729;
    /// `This item cannot be traded or sold.` (Java's trade/store/mail refusal.)
    pub const THIS_ITEM_CANNOT_BE_TRADED_OR_SOLD: i16 = 99;
    /// `This item cannot be destroyed.` (Java `RequestDestroyItem`.)
    pub const THIS_ITEM_CANNOT_BE_DESTROYED: i16 = 98;
    // Gatekeeper teleports (G15.5 tail).
    pub const YOU_CANNOT_TELEPORT_TO_A_VILLAGE_THAT_IS_IN_A_SIEGE: i16 = 707;
    pub const YOU_CANNOT_TELEPORT_WHILE_IN_POSSESSION_OF_A_WARD: i16 = 2778;
    // Manor (G26). The ids come from `SystemMessageId.java`'s `@ClientString`
    // annotations — earlier manor slices assumed they weren't available here.
    /// `The manor information has been updated.`
    pub const THE_MANOR_INFORMATION_HAS_BEEN_UPDATED: i16 = 884;
    /// `You do not have enough funds in the clan warehouse for the Manor to operate.`
    pub const NOT_ENOUGH_FUNDS_IN_CLAN_WAREHOUSE_FOR_MANOR: i16 = 935;
    /// `This seed may not be sown here.`
    pub const THIS_SEED_MAY_NOT_BE_SOWN_HERE: i16 = 872;
    /// `A manor cannot be set up between 4:30 am and 8 pm.`
    pub const A_MANOR_CANNOT_BE_SET_UP_BETWEEN_4_30_AM_AND_8_PM: i16 = 1675;
    /// `Failed in trading $s2 of $s1 crops.`
    pub const FAILED_IN_TRADING_S2_OF_S1_CROPS: i16 = 1491;
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
    // Cursed weapons (`//cw_*`). Ids straight off Java's `@ClientString`
    // annotations — 1815 is the *drop* line and 1817 the *login* one; they are
    // easy to swap by counting up from 1816, which is what a first pass here
    // did (the drop announce rendered as "…'s owner has logged into…").
    pub const YOU_HAVE_EQUIPPED_YOUR_S1: i16 = 49;
    // Shadow-item mana countdown (`Item.decreaseMana`) — the warnings a worn
    // shadow weapon prints as its charge runs out, then its farewell.
    /// `$s1's remaining Mana is now 10.`
    pub const S1_S_REMAINING_MANA_IS_NOW_10: i16 = 1979;
    /// `$s1's remaining Mana is now 5.`
    pub const S1_S_REMAINING_MANA_IS_NOW_5: i16 = 1980;
    /// `$s1's remaining Mana is now 1. It will disappear soon.`
    pub const S1_S_REMAINING_MANA_IS_NOW_1_IT_WILL_DISAPPEAR_SOON: i16 = 1981;
    /// `$s1's remaining Mana is now 0, and the item has disappeared.`
    pub const S1_S_REMAINING_MANA_IS_NOW_0_AND_THE_ITEM_HAS_DISAPPEARED: i16 = 1982;
    pub const EMPTY_3: i16 = 490;
    pub const S1_HAS_S2_MINUTE_S_OF_USAGE_TIME_REMAINING: i16 = 1814;
    pub const S2_WAS_DROPPED_IN_THE_S1_REGION: i16 = 1815;
    pub const THE_OWNER_OF_S2_HAS_APPEARED_IN_THE_S1_REGION: i16 = 1816;
    pub const S2_S_OWNER_HAS_LOGGED_INTO_THE_S1_REGION: i16 = 1817;
    pub const S1_HAS_DISAPPEARED: i16 = 1818;
    pub const C1_DOES_NOT_MEET_THE_PARTICIPATION_REQUIREMENTS_THE_OWNER_OF_S2_CANNOT_PARTICIPATE_IN_THE_OLYMPIAD: i16 = 1750;
    pub const SHOUT_AND_TRADE_CHATTING_CANNOT_BE_USED_WHILE_POSSESSING_A_CURSED_WEAPON: i16 = 2085;
    // Castle sieges (`//castlemanage`)
    // The registration-refusal block is contiguous (anchored by the already-
    // ported 638) in the Interlude systemmsg table.
    pub const ONLY_CLANS_OF_LEVEL_3_OR_ABOVE_MAY_REGISTER_FOR_A_CASTLE_SIEGE: i16 = 636;
    pub const CASTLE_OWNING_CLANS_ARE_AUTOMATICALLY_REGISTERED_ON_THE_DEFENDING_SIDE: i16 = 637;
    pub const YOU_HAVE_ALREADY_REQUESTED_A_CASTLE_SIEGE: i16 = 638;
    pub const A_CLAN_THAT_OWNS_A_CASTLE_CANNOT_PARTICIPATE_IN_ANOTHER_SIEGE: i16 = 639;
    pub const NO_MORE_REGISTRATIONS_MAY_BE_ACCEPTED_FOR_THE_ATTACKER_SIDE: i16 = 640;
    pub const NO_MORE_REGISTRATIONS_MAY_BE_ACCEPTED_FOR_THE_DEFENDER_SIDE: i16 = 641;
    /// `S1_HAS_ANNOUNCED_THE_NEXT_CASTLE_SIEGE_TIME` — the Interlude id for the
    /// owner's `RequestSetCastleSiegeTime` announcement.
    pub const S1_HAS_ANNOUNCED_THE_NEXT_CASTLE_SIEGE_TIME: i16 = 1104;
    /// `THE_REGISTRATION_TERM_FOR_S1_HAS_ENDED` (`@ClientString(id = 293)`) —
    /// broadcast when the siege auto-task closes attacker/defender registration.
    pub const THE_REGISTRATION_TERM_FOR_S1_HAS_ENDED: i16 = 293;
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
    pub const NOW_THAT_YOUR_CLAN_LEVEL_IS_ABOVE_LEVEL_5_IT_CAN_ACCUMULATE_CLAN_REPUTATION: i16 =
        1771;
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
    pub const THE_RECIPE_CANNOT_BE_REGISTERED_YOU_DO_NOT_HAVE_THE_ABILITY_TO_CREATE_ITEMS: i16 =
        1061;
    pub const YOUR_CREATE_ITEM_LEVEL_IS_TOO_LOW_TO_REGISTER_THIS_RECIPE: i16 = 404;
    pub const UP_TO_S1_RECIPES_CAN_BE_REGISTERED: i16 = 894;
    pub const YOU_MAY_NOT_ALTER_YOUR_RECIPE_BOOK_WHILE_ENGAGED_IN_MANUFACTURING: i16 = 853;
    pub const YOU_NEED_S2_MORE_S1_S: i16 = 854;
    pub const YOU_FAILED_AT_MIXING_THE_ITEM: i16 = 719;
    pub const S1_DISAPPEARED: i16 = 302;
    /// "You have taken $s1 damage because you were unable to breathe." — the
    /// drowning beat (`WaterTask`).
    pub const YOU_HAVE_TAKEN_S1_DAMAGE_BECAUSE_YOU_WERE_UNABLE_TO_BREATHE: i16 = 297;
    /// "Summoning your pet…" — the pet-manager evolve/restore flows (G29).
    pub const SUMMONING_YOUR_PET: i16 = 547;
    pub const S2_S1_S_DISAPPEARED: i16 = 301;
    pub const WHILE_YOU_ARE_ENGAGED_IN_COMBAT_YOU_CANNOT_OPERATE_A_PRIVATE_STORE_OR_PRIVATE_WORKSHOP: i16 = 1135;
    /// "The purchase price is higher than the amount of money that you have and
    /// so you cannot open a personal store."
    pub const THE_PURCHASE_PRICE_IS_HIGHER_THAN_YOUR_MONEY: i16 = 720;
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
    /// "There are not enough necessary items to use the skill." — the reagent
    /// gate (`SkillCaster.checkUseConditions`, G19).
    pub const THERE_ARE_NOT_ENOUGH_NECESSARY_ITEMS_TO_USE_THE_SKILL: i16 = 2156;
    // Skill enchanting (G19 slice 2)
    pub const YOU_DO_NOT_HAVE_ALL_OF_THE_ITEMS_NEEDED_TO_ENCHANT_THAT_SKILL: i16 = 1439;
    /// "Skill enchant was successful! $s1 has been enchanted."
    pub const SKILL_ENCHANT_WAS_SUCCESSFUL_S1_HAS_BEEN_ENCHANTED: i16 = 1440;
    pub const SKILL_ENCHANT_FAILED_THE_SKILL_WILL_BE_INITIALIZED: i16 = 1441;
    pub const YOU_DO_NOT_HAVE_ENOUGH_SP_TO_ENCHANT_THAT_SKILL: i16 = 1443;
    /// "Enchant skill route change was successful. Lv of enchant skill $s1 will remain."
    pub const ENCHANT_SKILL_ROUTE_CHANGE_WAS_SUCCESSFUL: i16 = 2073;
    /// "Skill enchant failed. Current level of enchant skill $s1 will remain unchanged."
    pub const SKILL_ENCHANT_FAILED_CURRENT_LEVEL_WILL_REMAIN_UNCHANGED: i16 = 2074;
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
    /// "Clan member $c1 was named a hero. $s2 points have been added to your
    /// Clan Reputation." — `Hero.claimHero`, broadcast to the hero's clan.
    pub const CLAN_MEMBER_C1_WAS_NAMED_A_HERO_S2_POINTS_HAVE_BEEN_ADDED_TO_YOUR_CLAN_REPUTATION:
        i16 = 1776;
    // Skill acquisition (G13.9)
    pub const YOU_DO_NOT_HAVE_ENOUGH_ITEMS_TO_LEARN_THIS_SKILL: i16 = 276;
    pub const YOU_DO_NOT_HAVE_ENOUGH_SP_TO_LEARN_THIS_SKILL: i16 = 278;
    pub const YOU_DO_NOT_MEET_THE_SKILL_LEVEL_REQUIREMENTS: i16 = 2208;
    // Shop (G12)
    pub const YOU_DO_NOT_HAVE_ENOUGH_ADENA: i16 = 279;
    // Offline shops (G33)
    pub const DO_YOU_WISH_TO_EXIT_THE_GAME: i16 = 125;
    pub const PRIVATE_STORE_ALREADY_CLOSED: i16 = 349;
    // Lucky Lottery (G26.5)
    pub const TICKETS_FOR_THE_CURRENT_LOTTERY_ARE_NO_LONGER_AVAILABLE: i16 = 784;
    pub const LOTTERY_TICKETS_ARE_NOT_CURRENTLY_BEING_SOLD: i16 = 930;
    // Monster Race (G26.5). Reuses `ACQUIRED_S1_S2` (371) above for the ticket
    // buy, with params `[Int(raceNum), ItemName]`.
    pub const MONSTER_RACE_PAYOUT_INFORMATION_IS_NOT_AVAILABLE_WHILE_TICKETS_ARE_BEING_SOLD: i16 =
        1044;
    pub const MONSTER_RACE_TICKETS_ARE_NO_LONGER_AVAILABLE: i16 = 1046;
    // Item auction (G30.5). `*_S1` variants take `[Long(bid)]` / `[ItemName]`.
    pub const YOUR_BID_PRICE_MUST_BE_HIGHER_THAN_THE_MINIMUM_PRICE: i16 = 677;
    pub const YOU_HAVE_SUBMITTED_A_BID_FOR_THE_AUCTION_OF_S1: i16 = 678;
    pub const YOU_HAVE_CANCELED_YOUR_BID: i16 = 679;
    pub const THERE_ARE_NO_OFFERINGS_I_OWN_OR_I_MADE_A_BID_FOR: i16 = 1883;
    pub const IT_IS_NOT_AN_AUCTION_PERIOD: i16 = 2075;
    pub const THE_HIGHEST_BID_IS_OVER_999_9_BILLION: i16 = 2076;
    pub const YOUR_BID_MUST_BE_HIGHER_THAN_THE_CURRENT_HIGHEST_BID: i16 = 2077;
    pub const YOU_DO_NOT_HAVE_ENOUGH_ADENA_FOR_THIS_BID: i16 = 2078;
    pub const YOU_CURRENTLY_HAVE_THE_HIGHEST_BID_BUT_THE_RESERVE_HAS_NOT_BEEN_MET: i16 = 2079;
    pub const YOU_WERE_OUTBID_THE_NEW_HIGHEST_BID_IS_S1_ADENA: i16 = 2080;
    pub const BIDDER_EXISTS_THE_AUCTION_TIME_HAS_BEEN_EXTENDED_BY_5_MINUTES: i16 = 2159;
    pub const BIDDER_EXISTS_AUCTION_TIME_HAS_BEEN_EXTENDED_BY_3_MINUTES: i16 = 2160;
    // Instances (G27 / Frintezza).
    pub const YOU_DO_NOT_HAVE_ENOUGH_REQUIRED_ITEMS: i16 = 701;
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
    /// "Congratulations. Your raid was successful."
    pub const CONGRATULATIONS_YOUR_RAID_WAS_SUCCESSFUL: i16 = 1209;
    /// "You have earned $s1 raid point(s)."
    pub const YOU_HAVE_EARNED_S1_RAID_POINTS: i16 = 1725;
    /// "You already have a pet."
    pub const YOU_ALREADY_HAVE_A_PET: i16 = 542;
    /// "Your pet gained $s1 XP."
    pub const YOUR_PET_GAINED_S1_XP: i16 = 1014;
    /// "Your pet ate a little, but is still hungry."
    pub const YOUR_PET_ATE_A_LITTLE_BUT_IS_STILL_HUNGRY: i16 = 596;
    /// "This pet cannot use this item."
    pub const THIS_PET_CANNOT_USE_THIS_ITEM: i16 = 972;
    /// "Your pet was hungry so it ate $s1."
    pub const YOUR_PET_WAS_HUNGRY_SO_IT_ATE_S1: i16 = 1527;
    /// "There is not much time remaining until the pet leaves."
    pub const THERE_IS_NOT_MUCH_TIME_REMAINING_UNTIL_THE_PET_LEAVES: i16 = 2372;
    /// "The pet is now leaving."
    pub const THE_PET_IS_NOW_LEAVING: i16 = 2373;
    /// "Your pet is starving and will not obey until it gets it's food. Feed your pet!"
    pub const YOUR_PET_IS_STARVING: i16 = 3213;
    /// "Your servitor passed away." — its lifetime ran out.
    pub const YOUR_SERVITOR_PASSED_AWAY: i16 = 1520;
    /// "The pet has been killed. If you don't resurrect it within 24 hours,
    /// the pet's body will disappear along with all the pet's items."
    ///
    /// Note 1519 vs 1520: this was written as the servitor-expiry id in G29
    /// slice 1, so an expiring servitor showed *this* text instead.
    pub const THE_PET_HAS_BEEN_KILLED: i16 = 1519;
    /// "A summoned monster uses $s1." — the periodic upkeep item.
    pub const A_SUMMONED_MONSTER_USES_S1: i16 = 1027;
    /// "Since you do not have enough items to maintain the servitor's stay, the
    /// servitor has disappeared."
    pub const NOT_ENOUGH_ITEMS_TO_MAINTAIN_SERVITOR: i16 = 1142;
    /// "You do not have a servitor."
    pub const YOU_DO_NOT_HAVE_A_SERVITOR: i16 = 2310;
    /// "Resurrection has already been proposed."
    pub const RESURRECTION_HAS_ALREADY_BEEN_PROPOSED: i16 = 1512;
    /// "$s1 cannot be used due to unsuitable terms."
    pub const S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS: i16 = 113;
    /// "Reject resurrection."
    pub const REJECT_RESURRECTION: i16 = 356;
    /// "If a base camp does not exist, resurrection is not possible."
    pub const IF_A_BASE_CAMP_DOES_NOT_EXIST_RESURRECTION_IS_NOT_POSSIBLE: i16 = 716;
    /// "The guardian tower has been destroyed and resurrection is not possible."
    pub const THE_GUARDIAN_TOWER_HAS_BEEN_DESTROYED_AND_RESURRECTION_IS_NOT_POSSIBLE: i16 = 717;
    /// "It is not possible to resurrect in battlegrounds where a siege war is
    /// taking place."
    pub const IT_IS_NOT_POSSIBLE_TO_RESURRECT_IN_BATTLEGROUNDS: i16 = 1053;
    /// "$s1 MP has been restored." — a self-cast recharge.
    pub const S1_MP_HAS_BEEN_RESTORED: i16 = 1067;
    /// "$s2 MP has been restored by $c1." — someone else recharged you.
    pub const S2_MP_HAS_BEEN_RESTORED_BY_C1: i16 = 1068;
    /// "$s2's MP has been drained by $c1." — the drain victim's notice.
    pub const S2_S_MP_HAS_BEEN_DRAINED_BY_C1: i16 = 970;
    /// "Your opponent's MP was reduced by $s1." — the caster's own.
    pub const YOUR_OPPONENT_S_MP_WAS_REDUCED_BY_S1: i16 = 1867;
    /// "$c1 resisted $c2's drain." — the victim's notice when `calcMagicAffected` fails.
    pub const C1_RESISTED_C2_S_DRAIN: i16 = 2267;
    pub const C1_HAS_INFLICTED_S3_DAMAGE_ON_C2: i16 = 2261;
    pub const C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2: i16 = 2262;
    pub const S2_SECONDS_REMAINING_FOR_REUSE: i16 = 2303;
    pub const S2_MINUTES_S3_SECONDS_REMAINING_FOR_REUSE: i16 = 2304;
    pub const S2_HOURS_S3_MINUTES_S4_SECONDS_REMAINING_FOR_REUSE: i16 = 2305;
    // Clan membership lifecycle (G18 slice 1)
    pub const YOU_CANNOT_ASK_YOURSELF_TO_APPLY_TO_A_CLAN: i16 = 4;
    pub const S1_IS_ALREADY_A_MEMBER_OF_ANOTHER_CLAN: i16 = 10;
    pub const CLAN_MEMBER_S1_HAS_BEEN_EXPELLED: i16 = 191;
    // Clan academy (G18.6) — the join/priv refusals already live below.
    pub const CONGRATULATIONS_YOU_WILL_NOW_GRADUATE_FROM_THE_CLAN_ACADEMY: i16 = 1749;
    pub const S2_HAS_BEEN_DESIGNATED_AS_THE_APPRENTICE_OF_CLAN_MEMBER_S1: i16 = 1755;
    pub const YOUR_APPRENTICE_S1_HAS_LOGGED_IN: i16 = 1756;
    pub const YOUR_SPONSOR_C1_HAS_LOGGED_IN: i16 = 1758;
    pub const YOU_DO_NOT_HAVE_THE_RIGHT_TO_DISMISS_AN_APPRENTICE: i16 = 1762;
    pub const S2_CLAN_MEMBER_C1_S_APPRENTICE_HAS_BEEN_REMOVED: i16 = 1763;
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
    pub const A_CLAN_CREST_CAN_ONLY_BE_REGISTERED_WHEN_THE_CLAN_S_SKILL_LEVEL_IS_3_OR_ABOVE: i16 =
        272;
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
    // Mail / post (G30)
    pub const IT_S_A_PAYMENT_REQUEST_TRANSACTION_PLEASE_ATTACH_THE_ITEM: i16 = 2966;
    pub const THE_MAIL_LIMIT_240_HAS_BEEN_EXCEEDED_AND_THIS_CANNOT_BE_FORWARDED: i16 = 2968;
    pub const YOU_CANNOT_FORWARD_IN_A_NON_PEACE_ZONE_LOCATION: i16 = 2970;
    pub const YOU_CANNOT_FORWARD_DURING_AN_EXCHANGE: i16 = 2971;
    pub const YOU_CANNOT_FORWARD_BECAUSE_THE_PRIVATE_STORE_OR_WORKSHOP_IS_IN_PROGRESS: i16 = 2972;
    pub const THE_ITEM_THAT_YOU_RE_TRYING_TO_SEND_CANNOT_BE_FORWARDED: i16 = 2974;
    pub const YOU_CANNOT_FORWARD_BECAUSE_YOU_DON_T_HAVE_ENOUGH_ADENA: i16 = 2975;
    pub const YOU_CANNOT_RECEIVE_IN_A_NON_PEACE_ZONE_LOCATION: i16 = 2976;
    pub const YOU_CANNOT_RECEIVE_DURING_AN_EXCHANGE: i16 = 2977;
    pub const YOU_CANNOT_RECEIVE_BECAUSE_THE_PRIVATE_STORE_OR_WORKSHOP_IS_IN_PROGRESS: i16 = 2978;
    pub const YOU_CANNOT_RECEIVE_BECAUSE_YOU_DON_T_HAVE_ENOUGH_ADENA: i16 = 2980;
    pub const YOU_COULD_NOT_RECEIVE_BECAUSE_YOUR_INVENTORY_IS_FULL: i16 = 2981;
    pub const YOU_CANNOT_CANCEL_IN_A_NON_PEACE_ZONE_LOCATION: i16 = 2982;
    pub const YOU_CANNOT_CANCEL_DURING_AN_EXCHANGE: i16 = 2983;
    pub const YOU_CANNOT_CANCEL_BECAUSE_THE_PRIVATE_STORE_OR_WORKSHOP_IS_IN_PROGRESS: i16 = 2984;
    pub const YOU_COULD_NOT_CANCEL_RECEIPT_BECAUSE_YOUR_INVENTORY_IS_FULL: i16 = 2988;
    pub const WHEN_THE_RECIPIENT_DOESN_T_EXIST_SENDING_MAIL_IS_NOT_POSSIBLE: i16 = 3002;
    pub const MAIL_SUCCESSFULLY_SENT: i16 = 3009;
    pub const MAIL_SUCCESSFULLY_RETURNED: i16 = 3010;
    pub const MAIL_SUCCESSFULLY_CANCELLED: i16 = 3011;
    pub const MAIL_SUCCESSFULLY_RECEIVED: i16 = 3012;
    pub const ITEM_SELECTION_IS_POSSIBLE_UP_TO_8: i16 = 3016;
    pub const YOU_CANNOT_SEND_A_MAIL_TO_YOURSELF: i16 = 3019;
    pub const WHEN_NOT_ENTERING_THE_AMOUNT_FOR_THE_PAYMENT_REQUEST_YOU_CANNOT_SEND_ANY_MAIL: i16 =
        3020;
    pub const S2_HAS_MADE_A_PAYMENT_OF_S1_ADENA_PER_YOUR_PAYMENT_REQUEST_MAIL: i16 = 3025;
    pub const S1_RETURNED_THE_MAIL: i16 = 3029;
    pub const YOU_CANNOT_CANCEL_SENT_MAIL_SINCE_THE_RECIPIENT_RECEIVED_IT: i16 = 3030;
    pub const YOU_CANNOT_RECEIVE_OR_SEND_MAIL_WITH_ATTACHED_ITEMS_IN_NON_PEACE_ZONE_REGIONS: i16 =
        3066;
    pub const S1_CANCELED_THE_SENT_MAIL: i16 = 3067;
    pub const THE_MAIL_WAS_RETURNED_DUE_TO_THE_EXCEEDED_WAITING_TIME: i16 = 3068;
    pub const S1_ACQUIRED_THE_ATTACHED_ITEM_TO_YOUR_MAIL: i16 = 3072;
    pub const YOU_HAVE_ACQUIRED_S2_S1: i16 = 3073;
    pub const THE_ALLOWED_LENGTH_FOR_RECIPIENT_EXCEEDED: i16 = 3074;
    pub const THE_ALLOWED_LENGTH_FOR_A_TITLE_EXCEEDED: i16 = 3075;
    pub const YOU_CANNOT_SEND_MAIL_TO_THE_GM_STAFF: i16 = 1370;

    // Party matching rooms (G30)
    pub const YOU_HAVE_CREATED_A_PARTY_ROOM: i16 = 1388;
    pub const YOU_HAVE_EXITED_THE_PARTY_ROOM: i16 = 1391;
    pub const C1_HAS_LEFT_THE_PARTY_ROOM: i16 = 1392;
    pub const YOU_HAVE_BEEN_OUSTED_FROM_THE_PARTY_ROOM: i16 = 1393;
    pub const C1_HAS_BEEN_KICKED_FROM_THE_PARTY_ROOM: i16 = 1394;
    pub const THE_PARTY_ROOM_HAS_BEEN_DISBANDED: i16 = 1395;
    pub const THE_LIST_OF_PARTY_ROOMS_CAN_ONLY_BE_VIEWED_BY_A_PERSON_WHO_IS_NOT_PART_OF_A_PARTY:
        i16 = 1396;
    pub const THE_LEADER_OF_THE_PARTY_ROOM_HAS_CHANGED: i16 = 1397;
    pub const YOU_DO_NOT_MEET_THE_REQUIREMENTS_TO_ENTER_THAT_PARTY_ROOM: i16 = 1413;
    pub const YOU_CANNOT_DISMISS_A_PARTY_MEMBER_BY_FORCE: i16 = 1699;
    pub const THE_RECIPIENT_OF_YOUR_INVITATION_DID_NOT_ACCEPT_THE_PARTY_MATCHING_INVITATION: i16 =
        1728;
    pub const C1_HAS_ENTERED_THE_PARTY_ROOM: i16 = 1900;

    // Command channels / MPCC
    pub const YOUR_TARGET_CANNOT_BE_FOUND: i16 = 50;
    pub const YOU_CANNOT_USE_THIS_ON_YOURSELF: i16 = 51;
    // Loot protection (pickup refusals)
    pub const YOU_HAVE_FAILED_TO_PICK_UP_S1_ADENA: i16 = 55;
    pub const YOU_HAVE_FAILED_TO_PICK_UP_S1: i16 = 56;
    pub const YOU_HAVE_FAILED_TO_PICK_UP_S2_S1_S: i16 = 57;
    pub const C1_IS_INVITING_YOU_TO_A_COMMAND_CHANNEL_DO_YOU_ACCEPT: i16 = 1529;
    pub const COMMAND_CHANNELS_CAN_ONLY_BE_FORMED_BY_A_PARTY_LEADER_WHO_IS_ALSO_THE_LEADER_OF_A_LEVEL_5_CLAN: i16 = 1574;
    pub const THE_COMMAND_CHANNEL_HAS_BEEN_FORMED: i16 = 1580;
    pub const THE_COMMAND_CHANNEL_HAS_BEEN_DISBANDED: i16 = 1581;
    pub const YOU_HAVE_JOINED_THE_COMMAND_CHANNEL: i16 = 1582;
    pub const YOU_HAVE_QUIT_THE_COMMAND_CHANNEL: i16 = 1586;
    pub const C1_S_PARTY_HAS_LEFT_THE_COMMAND_CHANNEL: i16 = 1587;
    pub const YOU_WERE_DISMISSED_FROM_THE_COMMAND_CHANNEL: i16 = 1583;
    pub const C1_S_PARTY_HAS_BEEN_DISMISSED_FROM_THE_COMMAND_CHANNEL: i16 = 1584;
    pub const COMMAND_CHANNEL_AUTHORITY_HAS_BEEN_TRANSFERRED_TO_C1: i16 = 1589;
    pub const YOU_DO_NOT_HAVE_AUTHORITY_TO_INVITE_SOMEONE_TO_THE_COMMAND_CHANNEL: i16 = 1593;
    pub const C1_S_PARTY_IS_ALREADY_A_MEMBER_OF_THE_COMMAND_CHANNEL: i16 = 1594;
    // MPCC matching rooms
    pub const THE_COMMAND_CHANNEL_MATCHING_ROOM_WAS_CANCELLED: i16 = 2994;
    pub const YOU_CANNOT_ENTER_THE_COMMAND_CHANNEL_MATCHING_ROOM_BECAUSE_YOU_DO_NOT_MEET_THE_REQUIREMENTS: i16 = 2996;
    pub const YOU_EXITED_FROM_THE_COMMAND_CHANNEL_MATCHING_ROOM: i16 = 2997;
    pub const YOU_WERE_EXPELLED_FROM_THE_COMMAND_CHANNEL_MATCHING_ROOM: i16 = 2998;
    pub const THE_COMMAND_CHANNEL_AFFILIATED_PARTY_S_PARTY_MEMBER_CANNOT_USE_THE_MATCHING_SCREEN:
        i16 = 2999;
    pub const THE_COMMAND_CHANNEL_MATCHING_ROOM_WAS_CREATED: i16 = 3000;
    pub const THE_COMMAND_CHANNEL_MATCHING_ROOM_INFORMATION_WAS_EDITED: i16 = 3001;
    pub const C1_ENTERED_THE_COMMAND_CHANNEL_MATCHING_ROOM: i16 = 3003;

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
    // Punishment / moderation (G31)
    pub const CHATTING_IS_CURRENTLY_PROHIBITED: i16 = 966;
    // Petitions (G31)
    pub const THE_GAME_CLIENT_ENCOUNTERED_AN_ERROR_PETITION_SERVER: i16 = 381;
    pub const THIS_ENDS_THE_GM_PETITION_CONSULTATION: i16 = 387;
    pub const NOT_UNDER_PETITION_CONSULTATION: i16 = 388;
    pub const YOUR_PETITION_APPLICATION_HAS_BEEN_ACCEPTED_RECEIPT_NO_IS_S1: i16 = 389;
    pub const YOU_MAY_ONLY_SUBMIT_ONE_PETITION_ACTIVE_AT_A_TIME: i16 = 390;
    pub const RECEIPT_NO_S1_PETITION_CANCELLED: i16 = 391;
    pub const FAILED_TO_CANCEL_PETITION: i16 = 393;
    pub const STARTING_PETITION_CONSULTATION_WITH_C1: i16 = 394;
    pub const PETITION_CONSULTATION_WITH_C1_HAS_ENDED: i16 = 395;
    pub const PETITION_APPLICATION_ACCEPTED: i16 = 406;
    pub const YOUR_PETITION_IS_BEING_PROCESSED: i16 = 407;
    pub const THERE_ARE_S1_PETITIONS_CURRENTLY_ON_THE_WAITING_LIST: i16 = 601;
    pub const THE_PETITION_SERVICE_IS_CURRENTLY_UNAVAILABLE: i16 = 602;
    pub const THERE_ARE_NO_GMS_CURRENTLY_VISIBLE: i16 = 702;
    pub const YOU_HAVE_SUBMITTED_S1_PETITIONS_YOU_MAY_SUBMIT_S2_MORE_TODAY: i16 = 730;
    pub const WE_HAVE_RECEIVED_S1_PETITIONS_FROM_YOU_TODAY_MAXIMUM: i16 = 733;
    pub const THE_PETITION_WAS_CANCELED_YOU_MAY_SUBMIT_S1_MORE_TODAY: i16 = 736;
    pub const YOU_HAVE_NOT_SUBMITTED_A_PETITION: i16 = 738;
    pub const C1_HAS_BEEN_REPORTED_AS_AN_ILLEGAL_PROGRAM_USER_AND_CANNOT_JOIN_A_PARTY: i16 = 2482;
    pub const YOU_HAVE_BEEN_REPORTED_AS_AN_ILLEGAL_PROGRAM_USER_SO_PARTICIPATING_IN_A_PARTY_IS_NOT_ALLOWED:
        i16 = 2484;
    pub const YOU_ARE_NOT_IN_A_PARTY: i16 = 4201;
    pub const YOU_ARE_NOT_IN_A_CLAN: i16 = 4202;
    // Boats (G24.5)
    pub const YOU_DO_NOT_POSSESS_THE_CORRECT_TICKET: i16 = 402;
    // Olympiad (G25)
    pub const CHARACTER_C1_DOES_NOT_MEET_THE_OLYMPIAD_CONDITIONS: i16 = 1501;
    pub const THE_OLYMPIAD_GAMES_ARE_NOT_CURRENTLY_IN_PROGRESS: i16 = 1651;
    pub const PARTICIPATION_REQUESTS_ARE_NO_LONGER_BEING_ACCEPTED: i16 = 1803;
    pub const THE_MAXIMUM_MATCHES_YOU_CAN_PARTICIPATE_IN_1_WEEK_IS_30: i16 = 3224;
    pub const YOU_HAVE_BEEN_REGISTERED_FOR_THE_OLYMPIAD_WAITING_LIST_FOR_A_CLASS_BATTLE: i16 = 1503;
    pub const YOU_ARE_CURRENTLY_REGISTERED_FOR_A_1V1_CLASS_IRRELEVANT_MATCH: i16 = 1504;
    pub const YOU_HAVE_BEEN_REMOVED_FROM_THE_OLYMPIAD_WAITING_LIST: i16 = 1505;
    pub const YOU_ARE_NOT_CURRENTLY_REGISTERED_FOR_THE_OLYMPIAD: i16 = 1506;
    pub const C1_IS_ALREADY_REGISTERED_ON_THE_CLASS_MATCH_WAITING_LIST: i16 = 1689;
    pub const C1_IS_ALREADY_REGISTERED_ON_THE_WAITING_LIST_FOR_THE_ALL_CLASS_BATTLE: i16 = 1690;
    pub const CONGRATULATIONS_C1_YOU_WIN_THE_MATCH: i16 = 1497;
    pub const ROUND_S1_OF_THE_OLYMPIAD_GAMES_HAS_NOW_ENDED: i16 = 1640;
    pub const UNABLE_TO_PROCESS_UNTIL_INVENTORY_UNDER_80_PERCENT: i16 = 1118;
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
    /// "You do not have a servitor and therefore cannot use the
    /// automatic-use function."
    pub const YOU_DO_NOT_HAVE_A_SERVITOR_FOR_AUTO_USE: i16 = 1676;
    /// "You don't have enough soulshots needed for a servitor."
    pub const NOT_ENOUGH_SOULSHOTS_FOR_A_SERVITOR: i16 = 1701;
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
    /// "You earned $s1 PA Point(s)."
    pub const YOU_EARNED_S1_PA_POINT_S: i16 = 1707;
    /// "Double points! You earned $s1 PA Point(s)."
    pub const DOUBLE_POINTS_YOU_EARNED_S1_PA_POINT_S: i16 = 1708;
    /// "You have earned the maximum number of PA Points."
    pub const YOU_HAVE_EARNED_THE_MAXIMUM_NUMBER_OF_PA_POINTS: i16 = 2389;
    /// "Your skill was deactivated due to lack of MP." — a toggle's MP upkeep
    /// tick failing (`ManaDamOverTime`).
    pub const YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP: i16 = 140;
    /// `THAT_SKILL_HAS_BEEN_DE_ACTIVATED_AS_HP_WAS_FULLY_RECOVERED`
    /// (`@ClientString(id = 175)`) — Relax switching itself off at full HP.
    pub const THAT_SKILL_HAS_BEEN_DE_ACTIVATED_AS_HP_WAS_FULLY_RECOVERED: i16 = 175;
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
    /// Kept for the record only — deliberately **never sent** (it would fire on
    /// nearly every monster kill); see `game_loop::vitality::set_vitality_points`.
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
    Popup {
        target: i32,
        attacker: i32,
        damage: i32,
    },
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
            SmParam::Popup {
                target,
                attacker,
                damage,
            } => {
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

/// `ConfirmDlg` with a bare system message and no parameters — the shape the
/// `.offline` command uses (`new ConfirmDlg(SystemMessageId.…)`). The client
/// echoes `message_id` back in `DlgAnswer`, which is how the reply is routed.
pub fn confirm_dlg(message_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CONFIRM_DLG);
    w.write_i32(message_id);
    w.write_i32(0); // parameter count
    w.write_i32(0); // time
    w.write_i32(0); // requesterId
    w.into_bytes()
}

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
