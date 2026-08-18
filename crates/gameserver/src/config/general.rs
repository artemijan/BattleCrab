//! `General.ini` — port of the GM-startup / hero-aura keys of the
//! `GENERAL_CONFIG_FILE` block of `Config.java`. Only the keys the admin
//! login flow needs so far are loaded (grown per milestone, like the other
//! config sections).

use commons::config::PropertiesParser;

pub const GENERAL_CONFIG_FILE: &str = "config/General.ini";

/// The GM login-state settings applied in `EnterWorld.runImpl` plus the hero
/// aura toggle read by CharInfo/UserInfo (`Config.GM_*`).
#[derive(Debug, Clone, Default)]
pub struct GeneralConfig {
    /// `GMHeroAura`: give GMs the Hero glow on login (CharInfo/UserInfo hero
    /// byte = `isHero() || (isGM() && GMHeroAura)`).
    pub gm_hero_aura: bool,
    /// `GMStartupBuilderHide`: hide the GM on login (retail builder default).
    /// When set, Java **skips** the invul/invis/silence/diet block below.
    pub gm_startup_builder_hide: bool,
    /// `GMStartupInvulnerable`: auto-set invulnerable on login.
    pub gm_startup_invulnerable: bool,
    /// `GMStartupInvisible`: auto-set invisible on login (also applied at
    /// char-select, matching `CharacterSelect.java`).
    pub gm_startup_invisible: bool,
    /// `GMStartupSilence`: auto-enable silence (whisper block) on login.
    pub gm_startup_silence: bool,
    /// `GMStartupAutoList`: register the GM in the visible GM list on login
    /// (vs. hidden). Drives the `addGm` hidden flag.
    /// `AllowWear` (True here) — whether the try-on shop works at all.
    pub allow_wear: bool,
    /// `WearDelay` (5 s) — how long a previewed outfit stays on before the
    /// server tells the client to drop it.
    pub wear_delay: i32,
    /// `WearPrice` (10 adena) — charged **per previewed slot**, not per
    /// request.
    pub wear_price: i32,
    pub gm_startup_auto_list: bool,
    /// `GMStartupDietMode`: auto-enable diet mode (no weight overload) on login.
    pub gm_startup_diet_mode: bool,
    /// `GMGiveSpecialSkills` / `GMGiveSpecialAuraSkills`: grant the GM
    /// convenience kits at enter-world (`SkillTreeData.addSkills`). Read by
    /// `admin::flags`; the skills are session-only and filtered out of the
    /// persistence flush.
    pub gm_give_special_skills: bool,
    pub gm_give_special_aura_skills: bool,

    // --- Ground-item auto-destroy (`ItemsAutoDestroyTaskManager`) ---
    /// `AutoDestroyDroppedItemAfter` (seconds): how long a dropped ground item
    /// lies before auto-destroying. `0` = never (Java code default). Applies to
    /// NPC drops unconditionally and to player drops only when
    /// [`Self::destroy_dropped_player_item`] is set.
    pub autodestroy_item_after: u64,
    /// `AutoDestroyHerbTime` (seconds, **60** here): the same clock for herbs —
    /// items with `ex_immediate_effect`, which are consumed by walking over
    /// them. A tenth of the ordinary 600 s, because a battlefield would
    /// otherwise be carpeted in them. Gated **independently** of
    /// `AutoDestroyDroppedItemAfter`: Java schedules a herb whenever *this* is
    /// non-zero, even with the ordinary destroyer switched off.
    pub herb_auto_destroy_time: u64,
    /// `DestroyPlayerDroppedItem`: whether **player**-dropped items are subject
    /// to auto-destroy at all. Java default `false` (and the dist value) — so a
    /// player's drop persists until pickup/restart, unlike an NPC drop.
    pub destroy_dropped_player_item: bool,
    /// `DestroyEquipableItem`: extends [`Self::destroy_dropped_player_item`] to
    /// equipable player drops (weapons/armor). When false, only non-equipable
    /// player drops auto-destroy.
    pub destroy_equipable_player_item: bool,
    /// `ListOfProtectedItems`: item ids never auto-destroyed on the ground
    /// (dist ships `0`, a non-existent id ⇒ effectively empty).
    pub protected_items: Vec<i32>,
    /// `JailDisableTransaction` (dist `False`): whether a jailed character is
    /// barred from item transactions — `RequestDropItem` refuses while it is
    /// on. Java's own default is `false` too, so nothing changes unless an
    /// operator turns it on.
    pub jail_disable_transaction: bool,
    /// `JailDisableChat` (dist `True`): a jailed character without
    /// `PlayerCondOverride.CHAT_CONDITIONS` is refused chat.
    ///
    /// **Only the world-chat branch consumes this today.** Java gates two
    /// places on it: `ChatWorld.handleChat` (ported) and `Say2`'s own guard
    /// over WHISPER/SHOUT/TRADE/HERO_VOICE, which this port has never had —
    /// ported 2026-08-07 alongside the olympiad gate beside it, so both
    /// call sites now consume this key.
    pub jail_disable_chat: bool,

    /// `AllowWater` (dist `True`): whether swimming can drown you. Java gates
    /// only `Player.checkWaterState()` on it inside `revalidateZone` — the
    /// swim-speed switch in `WaterZone.onEnter` is unconditional, so turning
    /// this off makes water slow but harmless, not inert.
    pub allow_water: bool,

    /// `GMSkillRestriction` (dist **True**, Java default `false`) — whether a
    /// character holding `PlayerCondOverride.SKILL_CONDITIONS` is **still**
    /// bound by a skill's conditions.
    ///
    /// Java: `if (isFakePlayer() || (canOverrideCond(SKILL_CONDITIONS) &&
    /// !Config.GM_SKILL_RESTRICTION)) return true;` — so **True, the dist
    /// value, restricts**: the override stops exempting them. The port used to
    /// let every GM skip every skill condition unconditionally, behind a
    /// comment asserting this key was off.
    pub gm_skill_restriction: bool,
    /// `GMItemRestriction` (dist **True**, Java default `false`) — the same
    /// shape for `ItemTemplate.checkCondition` and
    /// `PlayerCondOverride.ITEM_CONDITIONS`.
    ///
    /// **No consumer here, and it is not a wiring oversight**: the port does
    /// not evaluate item conditions at all — `<cond>` is unparsed
    /// (`data::item_data`'s module header records it), and the Olympiad and
    /// event item restrictions are likewise absent. There is nothing for the
    /// override to bypass and therefore nothing for this key to re-restrict.
    /// Carried as a field anyway so the gate lands with the conditions
    /// whenever they are ported, rather than being rediscovered then.
    pub gm_item_restriction: bool,
    /// `GMTradeRestrictedItems` (dist **False**, Java default `false`) —
    /// whether an override-holder may drop, trade and store items the datapack
    /// marks untradeable or quest-bound. Four Java sites read it: the drop
    /// gate, the quest-item drop gate, `TradeStart`'s item list and
    /// `TradeList.addItem`.
    pub gm_trade_restricted_items: bool,
    /// `GMRestartFighting` (dist **True**, Java default `true`) — whether a GM
    /// may restart or log out while the attack stance is up
    /// (`Player.canLogout`). The one key in this family that *grants* rather
    /// than restricts.
    pub gm_restart_fighting: bool,
    /// `GMShowAnnouncerName` (dist **False**, Java default `false`) — append
    /// ` [GmName]` to a `//announce`. `//announce_screen` is deliberately
    /// exempt in Java, and so is the port.
    pub gm_announcer_name: bool,
    /// `GMDebugHtmlPaths` (dist **True**, Java default `true`) — send a GM the
    /// path of every HTML the server serves them (`HtmCache.getHtm`), as a
    /// plain chat line. A GM tool: it is how you find which file a dialog came
    /// from without grepping the datapack.
    pub gm_debug_html_paths: bool,
    /// `UseSuperHasteAsGMSpeed` (dist **False**, Java default `false`) —
    /// `//gmspeed <n>` forwards to `//superhaste <n>` instead of applying a
    /// run-speed multiplier. Java calls it a "rollback feature for old custom
    /// way, in order to make everyone happy".
    pub use_super_haste_as_gm_speed: bool,
    /// `DefaultAccessLevel` (dist **0**, Java default `0`) — the access level a
    /// character resolving to 0 is promoted to. Java applies it in
    /// `Player.setAccessLevel` behind a `> 0` guard, so **0 means "no
    /// promotion"** rather than "promote everyone to 0"; an operator setting it
    /// to a GM tier makes every character on the server a GM, which is why the
    /// guard is the whole feature.
    pub default_access_level: i32,
    /// `OnlyGMItemsFree` (dist **True**, Java default `true`) — a buy-list row
    /// priced at 0 is refused (and punished) for anyone who is not a GM. The
    /// port had this hard-coded to the dist's behaviour behind a comment
    /// naming the key; the field is what makes an operator's `False` mean
    /// something.
    pub only_gm_items_free: bool,
    /// `ServerGMOnly` (dist **False**, Java default `false`) — whether the
    /// server registers with the login server as GM-only at startup, and what
    /// the status returns to after a cancelled shutdown. `//server_gm_only` /
    /// `//server_all` flip it at runtime, in Java by assigning the config field
    /// itself.
    pub server_gm_only: bool,

    /// `SkillCheckEnable` (dist **True**, Java default `false`) — whether
    /// `restoreSkills` validates every row it reads out of `character_skills`
    /// against the skill trees. A row that fails is an illegal skill: it was
    /// never learnable by this class, so it arrived through a bug, a
    /// hand-edited database or an exploit.
    pub skill_check_enable: bool,
    /// `SkillCheckRemove` (dist **True**, Java default `false`) — whether a
    /// failing row is *removed* as well as reported. With it off the check is
    /// a pure audit.
    pub skill_check_remove: bool,
    /// `SkillCheckGM` (dist **False**, Java default `true`) — whether the check
    /// also applies to a character holding `PlayerCondOverride.SKILL_CONDITIONS`.
    /// Java's guard reads
    /// `(!canOverrideCond(SKILL_CONDITIONS) || Config.SKILL_CHECK_GM)`, so
    /// **False here exempts** such a character. Note the inversion: the key
    /// reads like "check GMs too", and that is exactly what it means — off, so
    /// they are skipped.
    pub skill_check_gm: bool,

    /// `EnableFallingDamage` (dist `True`) — whether a fall costs HP.
    /// Java reads it in exactly one place, `Formulas.calcFallDam`, which
    /// returns 0 when it is off; `Player.isFalling` still runs, so the
    /// position-validation suppression and the client re-grounding survive a
    /// server with damage disabled. Java's code default is `true`.
    pub enable_falling_damage: bool,

    /// `AllowManor`: whether the castle manor (seed sowing / crop harvest) runs.
    /// The dist ships `False`; the manor data + packets exist regardless so the
    /// feature works the moment an operator enables it.
    pub allow_manor: bool,
    /// `AltManorSaveAllActions`: persist every manor setup change immediately.
    /// `False` on this dist, in which case the setup is written by the periodic
    /// autosave every [`alt_manor_save_period_rate`] hours and again on
    /// shutdown — exactly Java's two branches in `CastleManorManager.load` and
    /// `Shutdown`.
    ///
    /// [`alt_manor_save_period_rate`]: General::alt_manor_save_period_rate
    pub alt_manor_save_all_actions: bool,
    /// `AltManorSavePeriodRate` (hours, dist 2) — the autosave interval used
    /// when `AltManorSaveAllActions` is off. Java schedules it at that rate
    /// both for the initial delay and the period.
    pub alt_manor_save_period_rate: i32,
    /// `AltManorRefreshTime` (hour, dist 20) — when the daily manor cycle rolls:
    /// the `APPROVED → MAINTENANCE` change (production rollover) fires at
    /// `refresh_time:refresh_min`.
    pub alt_manor_refresh_time: i32,
    /// `AltManorRefreshMin` (minute, dist 0).
    pub alt_manor_refresh_min: i32,
    /// `AltManorMaintenanceMin` (dist 6) — the maintenance window length; the
    /// `MAINTENANCE → MODIFIABLE` change fires at
    /// `refresh_time:(refresh_min + maintenance_min)`.
    pub alt_manor_maintenance_min: i32,
    /// `AltManorApproveTime` (hour, dist 4) — when the owner's edit window
    /// closes: the `MODIFIABLE → APPROVED` change fires at
    /// `approve_time:approve_min`.
    pub alt_manor_approve_time: i32,
    /// `AltManorApproveMin` (minute, dist 30).
    pub alt_manor_approve_min: i32,

    /// `AllowMail`: whether the mail/post system is available (G30). Dist and
    /// Java default both `True`.
    pub allow_mail: bool,
    /// `AllowAttachments`: whether mail may carry items and COD (G30). Java
    /// still delivers the *message* when this is off — it just strips the
    /// attachments and the payment request.
    pub allow_attachments: bool,

    /// `AllowLottery`: whether the weekly Lucky Lottery runs (G26.5). Dist ships
    /// `False`; the round engine + persistence exist regardless so it works the
    /// moment an operator enables it.
    pub allow_lottery: bool,
    /// `AltLotteryPrize` (dist 50000): the starting jackpot of a fresh round.
    pub alt_lottery_prize: i64,
    /// `AltLotteryTicketPrice` (dist 2000): adena charged per ticket.
    pub alt_lottery_ticket_price: i64,
    /// `AltLottery5NumberRate` (dist 0.6): share of the pot paid to 5-match
    /// (first-prize) winners.
    pub alt_lottery_5_number_rate: f64,
    /// `AltLottery4NumberRate` (dist 0.2): share paid to 4-match winners.
    pub alt_lottery_4_number_rate: f64,
    /// `AltLottery3NumberRate` (dist 0.2): share paid to 3-match winners.
    pub alt_lottery_3_number_rate: f64,
    /// `AltLottery2and1NumberPrize` (dist 200): flat adena for a 2-or-1 match.
    pub alt_lottery_2and1_number_prize: i64,

    /// `AllowRace`: whether the Monster Race Track runs (G26.5). Dist ships
    /// `False`; the race engine exists regardless so an operator can enable it.
    pub allow_race: bool,

    /// `AltItemAuctionEnabled`: whether the item-auction house runs (G30.5).
    /// Dist ships `True`, but `ItemAuctions.xml` is empty, so nothing auctions
    /// until an operator adds instances.
    pub alt_item_auction_enabled: bool,
    /// `AltItemAuctionExpiredAfter` (days, dist 14): how long a finished auction
    /// + its bids linger before cleanup / after which a bid can't be canceled.
    pub alt_item_auction_expired_after_days: i32,
    /// `AltItemAuctionTimeExtendsOnBid` (seconds, dist 0): the extra
    /// bid-driven ending extension past the built-in 5-/3-minute phases. `0`
    /// disables the config phases.
    pub alt_item_auction_time_extends_on_bid: i64,

    // --- World chat ---------------------------------------------------------
    // `handlers/chathandlers/ChatWorld.java`. Enabled on this dist. See the
    // chronicle caveat on [`crate::enums::ChatType::World`].
    /// `WorldChatEnabled` (dist `True`): the master gate. With it off
    /// `ChatWorld.handleChat` returns immediately — **silently**, with no
    /// notice to the speaker — and the daily point reset is skipped too.
    pub world_chat_enabled: bool,
    /// `WorldChatMinLevel` (Java default 95, dist **40**): below this the
    /// speaker is told `YOU_CAN_USE_WORLD_CHAT_FROM_LV_S1` and the line is
    /// dropped. Also zeroes the count `ExWorldChatCnt` reports.
    pub world_chat_min_level: i32,
    /// `WorldChatPointsPerDay` (dist 10): the daily quota, before the
    /// `Stat.WORLD_CHAT_POINTS` add/mul Java folds in. No skill or item on this
    /// dist grants that stat, so the config value is the whole quota here.
    pub world_chat_points_per_day: i32,
    /// `WorldChatInterval` (dist `20secs`) as **seconds**: the per-speaker
    /// reuse window. `0` disables the window entirely — Java guards both the
    /// check and the stamp with `getSeconds() > 0`.
    pub world_chat_interval_secs: i64,

    // --- Audit-record gates ------------------------------------------------
    // Which categories the never-dropped audit sink (`commons::audit`) records.
    // All ship `False`: these are operator decisions about retention and disk,
    // not features. The sink itself is always running — the gate only decides
    // whether a given category produces records.
    /// `LogChat`: record public and private chat (Java: `Say2`,
    /// `RequestSendFriendMsg`).
    pub log_chat: bool,
    /// `LogItems`: record item ownership and count changes.
    pub log_items: bool,
    /// `LogItemsSmallLog`: when [`Self::log_items`] is on, narrow it to adena
    /// and equippable items. Java treats this as an *override* rather than a
    /// filter — with it set, those items are recorded even though the broad
    /// branch is skipped.
    pub log_items_small_log: bool,
    /// `LogItemsIdsOnly`: narrow item records to [`Self::log_items_ids_list`],
    /// with the same override semantics as the small log.
    pub log_items_ids_only: bool,
    /// `LogItemsIdsList`: the ids [`Self::log_items_ids_only`] admits.
    pub log_items_ids_list: Vec<i32>,
    /// `LogItemEnchants`: record item enchant attempts and their outcome.
    pub log_item_enchants: bool,
    /// `LogSkillEnchants`: record skill enchant attempts and their outcome.
    pub log_skill_enchants: bool,
    /// `GMAudit`: record every GM command, its target and its arguments.
    pub gm_audit: bool,
    /// `DefaultPunish`: what happens to a player caught by a packet-validation
    /// guard (Java `Util.handleIllegalPlayerAction`). The dist ships `KICK`.
    pub default_punish: crate::model::punishment::IllegalActionPunishment,
    /// `DefaultPunishParam`: ban/jail duration in **seconds** for `KICKBAN` /
    /// `JAIL`. The dist ships `0`, which Java folds to one hundred years.
    pub default_punish_param: i64,
}

impl GeneralConfig {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(root, GENERAL_CONFIG_FILE))
    }

    /// Parse from an already-loaded `General.ini` (split out so tests can point
    /// at the real `dist/game` file without depending on the process cwd).
    fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            gm_hero_aura: p.get_bool("GMHeroAura", d.gm_hero_aura),
            gm_startup_builder_hide: p.get_bool("GMStartupBuilderHide", d.gm_startup_builder_hide),
            gm_startup_invulnerable: p.get_bool("GMStartupInvulnerable", d.gm_startup_invulnerable),
            gm_startup_invisible: p.get_bool("GMStartupInvisible", d.gm_startup_invisible),
            gm_startup_silence: p.get_bool("GMStartupSilence", d.gm_startup_silence),
            allow_wear: p.get_bool("AllowWear", true),
            wear_delay: p.get_int("WearDelay", 5),
            wear_price: p.get_int("WearPrice", 10),
            gm_startup_auto_list: p.get_bool("GMStartupAutoList", d.gm_startup_auto_list),
            gm_startup_diet_mode: p.get_bool("GMStartupDietMode", d.gm_startup_diet_mode),
            gm_give_special_skills: p.get_bool("GMGiveSpecialSkills", d.gm_give_special_skills),
            gm_give_special_aura_skills: p
                .get_bool("GMGiveSpecialAuraSkills", d.gm_give_special_aura_skills),
            autodestroy_item_after: p.get_int("AutoDestroyDroppedItemAfter", 0).max(0) as u64,
            herb_auto_destroy_time: p.get_int("AutoDestroyHerbTime", 60).max(0) as u64,
            destroy_dropped_player_item: p
                .get_bool("DestroyPlayerDroppedItem", d.destroy_dropped_player_item),
            destroy_equipable_player_item: p
                .get_bool("DestroyEquipableItem", d.destroy_equipable_player_item),
            // Java parses a comma-separated id list; the dist ships a lone `0`.
            protected_items: p
                .get_string("ListOfProtectedItems", "")
                .split(',')
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect(),
            jail_disable_transaction: p
                .get_bool("JailDisableTransaction", d.jail_disable_transaction),
            // Java's code default is `true`, not the derived `Default`'s false.
            jail_disable_chat: p.get_bool("JailDisableChat", true),
            // Java's code default is `true` (not `d.allow_water`, which the
            // derived `Default` would make `false` — the opposite meaning).
            allow_water: p.get_bool("AllowWater", true),
            // Java's code default is `true` — same trap as `allow_water`: the
            // derived `Default` would be `false`, which is the opposite.
            enable_falling_damage: p.get_bool("EnableFallingDamage", true),
            // Java's code defaults, which the dist inverts on all three:
            // `false`/`false`/`true` there, `True`/`True`/`False` here — so
            // this dist checks every character, removes what fails, and
            // exempts a `SKILL_CONDITIONS` override.
            // The GM-restriction family. Java's code defaults are all
            // `false`; this dist raises three of them.
            gm_skill_restriction: p.get_bool("GMSkillRestriction", false),
            gm_item_restriction: p.get_bool("GMItemRestriction", false),
            gm_trade_restricted_items: p.get_bool("GMTradeRestrictedItems", false),
            // …except this one, whose Java default is `true`.
            gm_restart_fighting: p.get_bool("GMRestartFighting", true),
            gm_announcer_name: p.get_bool("GMShowAnnouncerName", false),
            gm_debug_html_paths: p.get_bool("GMDebugHtmlPaths", true),
            use_super_haste_as_gm_speed: p.get_bool("UseSuperHasteAsGMSpeed", false),
            server_gm_only: p.get_bool("ServerGMOnly", false),
            only_gm_items_free: p.get_bool("OnlyGMItemsFree", true),
            default_access_level: p.get_int("DefaultAccessLevel", 0),
            skill_check_enable: p.get_bool("SkillCheckEnable", false),
            skill_check_remove: p.get_bool("SkillCheckRemove", false),
            skill_check_gm: p.get_bool("SkillCheckGM", true),
            allow_manor: p.get_bool("AllowManor", d.allow_manor),
            alt_manor_save_all_actions: p
                .get_bool("AltManorSaveAllActions", d.alt_manor_save_all_actions),
            alt_manor_save_period_rate: p.get_int("AltManorSavePeriodRate", 2),
            alt_manor_refresh_time: p.get_int("AltManorRefreshTime", 20),
            alt_manor_refresh_min: p.get_int("AltManorRefreshMin", 0),
            alt_manor_maintenance_min: p.get_int("AltManorMaintenanceMin", 6),
            alt_manor_approve_time: p.get_int("AltManorApproveTime", 4),
            alt_manor_approve_min: p.get_int("AltManorApproveMin", 30),
            allow_mail: p.get_bool("AllowMail", true),
            allow_attachments: p.get_bool("AllowAttachments", true),
            allow_lottery: p.get_bool("AllowLottery", d.allow_lottery),
            alt_lottery_prize: p.get_int("AltLotteryPrize", 50000) as i64,
            alt_lottery_ticket_price: p.get_int("AltLotteryTicketPrice", 2000) as i64,
            alt_lottery_5_number_rate: p.get_float("AltLottery5NumberRate", 0.6) as f64,
            alt_lottery_4_number_rate: p.get_float("AltLottery4NumberRate", 0.2) as f64,
            alt_lottery_3_number_rate: p.get_float("AltLottery3NumberRate", 0.2) as f64,
            alt_lottery_2and1_number_prize: p.get_int("AltLottery2and1NumberPrize", 200) as i64,
            allow_race: p.get_bool("AllowRace", d.allow_race),
            alt_item_auction_enabled: p
                .get_bool("AltItemAuctionEnabled", d.alt_item_auction_enabled),
            alt_item_auction_expired_after_days: p.get_int("AltItemAuctionExpiredAfter", 14),
            // Java parses seconds then converts to millis for the extend amount.
            alt_item_auction_time_extends_on_bid: p.get_int("AltItemAuctionTimeExtendsOnBid", 0)
                as i64
                * 1000,
            // Java's code defaults, not `d.*`: the derived `Default` would make
            // `world_chat_enabled` false, inverting the meaning (same trap as
            // `allow_water` above). Note Java defaults the min level to 95
            // while this dist ships 40 — the shipped value is the live one.
            world_chat_enabled: p.get_bool("WorldChatEnabled", true),
            world_chat_min_level: p.get_int("WorldChatMinLevel", 95),
            world_chat_points_per_day: p.get_int("WorldChatPointsPerDay", 10),
            world_chat_interval_secs: p.get_duration_secs("WorldChatInterval", 20),
            log_chat: p.get_bool("LogChat", d.log_chat),
            log_items: p.get_bool("LogItems", d.log_items),
            log_items_small_log: p.get_bool("LogItemsSmallLog", d.log_items_small_log),
            log_items_ids_only: p.get_bool("LogItemsIdsOnly", d.log_items_ids_only),
            log_items_ids_list: p
                .get_string("LogItemsIdsList", "")
                .split(',')
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect(),
            log_item_enchants: p.get_bool("LogItemEnchants", d.log_item_enchants),
            log_skill_enchants: p.get_bool("LogSkillEnchants", d.log_skill_enchants),
            gm_audit: p.get_bool("GMAudit", d.gm_audit),
            default_punish: crate::model::punishment::IllegalActionPunishment::find_by_name(
                &p.get_string("DefaultPunish", "KICK"),
            ),
            // Java: `0` means "one hundred years in seconds".
            default_punish_param: match p.get_int("DefaultPunishParam", 0) as i64 {
                0 => 3_155_695_200,
                v => v,
            },
        }
    }

    /// Java's `Item` gate, which is not a plain "is logging on" test: the small
    /// log and the id list are *overrides* that admit their own items even when
    /// the broad branch is off. Ported as one predicate so the four call sites
    /// cannot drift apart.
    pub fn should_log_item(&self, item_id: i32, equipable: bool) -> bool {
        const ADENA_ID: i32 = crate::data::item_data::ADENA_ID;
        (self.log_items && !self.log_items_small_log && !self.log_items_ids_only)
            || (self.log_items_small_log && (equipable || item_id == ADENA_ID))
            || (self.log_items_ids_only && self.log_items_ids_list.contains(&item_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real dist `General.ini` values (the GM block near the top of the
    /// file). Guards the key names against a config rename.
    #[test]
    fn loads_dist_gm_startup_values() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/config/General.ini"
        );
        let g = GeneralConfig::from_parser(&PropertiesParser::load(path));
        assert!(g.gm_hero_aura, "GMHeroAura=True");
        assert!(g.gm_startup_builder_hide, "GMStartupBuilderHide=True");
        assert!(g.gm_startup_invulnerable, "GMStartupInvulnerable=True");
        assert!(g.gm_startup_invisible, "GMStartupInvisible=True");
        assert!(g.gm_startup_silence, "GMStartupSilence=True");
        assert!(!g.gm_startup_auto_list, "GMStartupAutoList=False");
        assert!(!g.gm_startup_diet_mode, "GMStartupDietMode=False");
        assert!(!g.gm_give_special_skills, "GMGiveSpecialSkills=False");
        assert!(!g.allow_manor, "AllowManor=False on this dist");
        // Manor cutover times (guards the key names against a config rename).
        assert_eq!(g.alt_manor_refresh_time, 20, "AltManorRefreshTime=20");
        assert_eq!(g.alt_manor_refresh_min, 0, "AltManorRefreshMin=0");
        assert_eq!(g.alt_manor_maintenance_min, 6, "AltManorMaintenanceMin=6");
        assert_eq!(g.alt_manor_approve_time, 4, "AltManorApproveTime=4");
        assert_eq!(g.alt_manor_approve_min, 30, "AltManorApproveMin=30");
        // Lottery (G26.5): disabled on the dist, but the economics keys load.
        assert!(!g.allow_lottery, "AllowLottery=False on this dist");
        assert_eq!(g.alt_lottery_prize, 50000, "AltLotteryPrize=50000");
        assert_eq!(
            g.alt_lottery_ticket_price, 2000,
            "AltLotteryTicketPrice=2000"
        );
        // Parsed via get_float (f32) → f64, so compare with tolerance.
        assert!(
            (g.alt_lottery_5_number_rate - 0.6).abs() < 1e-6,
            "AltLottery5NumberRate=0.6"
        );
        assert_eq!(
            g.alt_lottery_2and1_number_prize, 200,
            "AltLottery2and1NumberPrize=200"
        );
        assert!(!g.allow_race, "AllowRace=False on this dist");
        assert!(
            g.alt_item_auction_enabled,
            "AltItemAuctionEnabled=True on this dist"
        );
    }

    /// The dist ground-item auto-destroy block: NPC drops decay after 600 s,
    /// but player drops are kept (`DestroyPlayerDroppedItem=False`).
    #[test]
    fn loads_dist_ground_item_autodestroy_values() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/config/General.ini"
        );
        let g = GeneralConfig::from_parser(&PropertiesParser::load(path));
        assert_eq!(
            g.autodestroy_item_after, 600,
            "AutoDestroyDroppedItemAfter=600"
        );
        assert!(
            !g.destroy_dropped_player_item,
            "DestroyPlayerDroppedItem=False"
        );
        assert!(
            !g.destroy_equipable_player_item,
            "DestroyEquipableItem=False"
        );
        assert_eq!(g.protected_items, vec![0], "ListOfProtectedItems=0");
    }
}
