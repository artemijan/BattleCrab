//! `config/FloodProtector.ini` — port of `Config.loadFloodProtectorConfigs` and
//! `gameserver/util/FloodProtectorConfig.java`.
//!
//! Fifteen independent rate limiters, each "one request per N game ticks"
//! (1 tick = 100 ms, the game loop's own `TICK`). Exceeding the interval drops
//! the request; exceeding `PunishmentLimit` *within* an interval additionally
//! kicks / bans the account / jails the character.
//!
//! **Four of the fifteen have no call site in Java** — `RollDice`,
//! `ItemPetSummon`, `HeroVoice` and `GlobalChat` are configured here and read
//! into `Config`, but nothing ever calls `canRollDice()` / `canUseHeroVoice()` /
//! `canUseGlobalChat()` / `canUsePetSummonItem()`. They are parsed anyway (the
//! keys are cheap, and a silently-missing key falling back to a code default is
//! the failure mode that has bitten this deploy before), and they stay
//! unconsumed here too: notably **this build rate-limits no chat channel at
//! all**, despite shipping `FloodProtectorGlobalChatInterval = 5`.

use commons::config::PropertiesParser;

pub const FLOOD_PROTECTOR_CONFIG_FILE: &str = "config/FloodProtector.ini";

/// The fifteen protector slots. The discriminant doubles as the index into
/// [`FloodProtectorsConfig::by_action`] and into the per-client runtime state,
/// so the two arrays cannot drift out of step with this list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum FloodAction {
    UseItem = 0,
    RollDice,
    ItemPetSummon,
    HeroVoice,
    GlobalChat,
    Subclass,
    DropItem,
    ServerBypass,
    MultiSell,
    Transaction,
    Manufacture,
    SendMail,
    CharacterSelect,
    ItemAuction,
    PlayerAction,
}

/// Every slot, in discriminant order — the loader and the runtime state both
/// walk this rather than repeating the list.
pub const ALL_FLOOD_ACTIONS: [FloodAction; FloodAction::COUNT] = [
    FloodAction::UseItem,
    FloodAction::RollDice,
    FloodAction::ItemPetSummon,
    FloodAction::HeroVoice,
    FloodAction::GlobalChat,
    FloodAction::Subclass,
    FloodAction::DropItem,
    FloodAction::ServerBypass,
    FloodAction::MultiSell,
    FloodAction::Transaction,
    FloodAction::Manufacture,
    FloodAction::SendMail,
    FloodAction::CharacterSelect,
    FloodAction::ItemAuction,
    FloodAction::PlayerAction,
];

impl FloodAction {
    pub const COUNT: usize = 15;

    pub fn index(self) -> usize {
        self as usize
    }

    /// The ini key stem: `FloodProtector<stem>Interval` and friends.
    pub fn key_stem(self) -> &'static str {
        match self {
            FloodAction::UseItem => "UseItem",
            FloodAction::RollDice => "RollDice",
            FloodAction::ItemPetSummon => "ItemPetSummon",
            FloodAction::HeroVoice => "HeroVoice",
            FloodAction::GlobalChat => "GlobalChat",
            FloodAction::Subclass => "Subclass",
            FloodAction::DropItem => "DropItem",
            FloodAction::ServerBypass => "ServerBypass",
            FloodAction::MultiSell => "MultiSell",
            FloodAction::Transaction => "Transaction",
            FloodAction::Manufacture => "Manufacture",
            FloodAction::SendMail => "SendMail",
            FloodAction::CharacterSelect => "CharacterSelect",
            FloodAction::ItemAuction => "ItemAuction",
            FloodAction::PlayerAction => "PlayerAction",
        }
    }

    /// Java `FloodProtectorConfig.FLOOD_PROTECTOR_TYPE` — the name that leads
    /// every log line, kept verbatim so operator-facing output matches.
    pub fn type_name(self) -> &'static str {
        match self {
            FloodAction::UseItem => "UseItemFloodProtector",
            FloodAction::RollDice => "RollDiceFloodProtector",
            FloodAction::ItemPetSummon => "ItemPetSummonFloodProtector",
            FloodAction::HeroVoice => "HeroVoiceFloodProtector",
            FloodAction::GlobalChat => "GlobalChatFloodProtector",
            FloodAction::Subclass => "SubclassFloodProtector",
            FloodAction::DropItem => "DropItemFloodProtector",
            FloodAction::ServerBypass => "ServerBypassFloodProtector",
            FloodAction::MultiSell => "MultiSellFloodProtector",
            FloodAction::Transaction => "TransactionFloodProtector",
            FloodAction::Manufacture => "ManufactureFloodProtector",
            FloodAction::SendMail => "SendMailFloodProtector",
            FloodAction::CharacterSelect => "CharacterSelectFloodProtector",
            FloodAction::ItemAuction => "ItemAuctionFloodProtector",
            FloodAction::PlayerAction => "PlayerActionFloodProtector",
        }
    }

    /// Java's **code** default interval, used when the key is absent. These are
    /// not always what the shipped ini says (`ServerBypass` defaults to 1 here
    /// but ships 5, `PlayerAction` defaults to 3 but ships 1) — a missing key
    /// must land on the Java default, not on the dist value.
    fn default_interval(self) -> i32 {
        match self {
            FloodAction::UseItem => 4,
            FloodAction::RollDice => 42,
            FloodAction::ItemPetSummon => 16,
            FloodAction::HeroVoice => 100,
            FloodAction::GlobalChat => 5,
            FloodAction::Subclass => 20,
            FloodAction::DropItem => 10,
            FloodAction::ServerBypass => 1,
            FloodAction::MultiSell => 1,
            FloodAction::Transaction => 10,
            FloodAction::Manufacture => 3,
            FloodAction::SendMail => 100,
            FloodAction::CharacterSelect => 30,
            FloodAction::ItemAuction => 9,
            FloodAction::PlayerAction => 3,
        }
    }
}

/// `FloodProtector<X>PunishmentType` — Java compares the raw string against
/// `"kick"`, `"ban"` and `"jail"`, so anything else (including the shipped
/// `none`) applies no punishment at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FloodPunishment {
    #[default]
    None,
    /// Close the connection (Java `Disconnection.of(client).defaultSequence`).
    Kick,
    /// `PunishmentAffect.ACCOUNT` + `PunishmentType.BAN`.
    Ban,
    /// `PunishmentAffect.CHARACTER` + `PunishmentType.JAIL`.
    Jail,
}

impl FloodPunishment {
    fn parse(raw: &str) -> Self {
        match raw {
            "kick" => FloodPunishment::Kick,
            "ban" => FloodPunishment::Ban,
            "jail" => FloodPunishment::Jail,
            "none" | "" => FloodPunishment::None,
            other => {
                // Java falls through its if/else chain silently; the warning is
                // ours, so a typo'd punishment type is not mistaken for "off".
                tracing::warn!(
                    "FloodProtector: unknown PunishmentType '{other}' — no punishment applied."
                );
                FloodPunishment::None
            }
        }
    }
}

/// One protector slot's settings (Java `FloodProtectorConfig`).
#[derive(Debug, Clone, Copy, Default)]
pub struct FloodProtectorConfig {
    /// Game ticks (100 ms) in which only one request is allowed. **`0` disables
    /// the protector** — `UseItem` ships `0` "to match retail".
    pub interval: i32,
    pub log_flooding: bool,
    /// Requests inside one interval before the punishment fires. `0` = never
    /// punish, which is what every slot ships on this dist.
    pub punishment_limit: i32,
    pub punishment_type: FloodPunishment,
    /// Absolute duration in **milliseconds** (Java multiplies the ini's minutes
    /// by 60000). `0` = forever.
    pub punishment_time_millis: i64,
}

impl FloodProtectorConfig {
    /// A protector with `interval <= 0` is off: Java's `curTick < _nextGameTick`
    /// can never hold once the interval stops advancing the next tick past the
    /// current one, so every request passes.
    pub fn is_enabled(&self) -> bool {
        self.interval > 0
    }
}

/// All fifteen slots, indexed by [`FloodAction::index`].
#[derive(Debug, Clone)]
pub struct FloodProtectorsConfig {
    by_action: [FloodProtectorConfig; FloodAction::COUNT],
}

impl Default for FloodProtectorsConfig {
    /// Java's code defaults (what an absent `FloodProtector.ini` would give):
    /// every interval at its built-in value, no logging, no punishment.
    fn default() -> Self {
        let mut by_action = [FloodProtectorConfig::default(); FloodAction::COUNT];
        for action in ALL_FLOOD_ACTIONS {
            by_action[action.index()] = FloodProtectorConfig {
                interval: action.default_interval(),
                ..Default::default()
            };
        }
        Self { by_action }
    }
}

impl FloodProtectorsConfig {
    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(
            root,
            FLOOD_PROTECTOR_CONFIG_FILE,
        ))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        let mut by_action = [FloodProtectorConfig::default(); FloodAction::COUNT];
        for action in ALL_FLOOD_ACTIONS {
            let stem = action.key_stem();
            by_action[action.index()] = FloodProtectorConfig {
                interval: p.get_int(
                    &format!("FloodProtector{stem}Interval"),
                    action.default_interval(),
                ),
                log_flooding: p.get_bool(&format!("FloodProtector{stem}LogFlooding"), false),
                punishment_limit: p.get_int(&format!("FloodProtector{stem}PunishmentLimit"), 0),
                punishment_type: FloodPunishment::parse(
                    p.get_string(&format!("FloodProtector{stem}PunishmentType"), "none")
                        .trim(),
                ),
                // Java: `getInt(...) * 60000L` — the ini is in minutes.
                punishment_time_millis: p.get_int(&format!("FloodProtector{stem}PunishmentTime"), 0)
                    as i64
                    * 60_000,
            };
        }
        Self { by_action }
    }

    pub fn get(&self, action: FloodAction) -> &FloodProtectorConfig {
        &self.by_action[action.index()]
    }

    /// Every slot off — an ini of all-zero intervals, which is a configuration
    /// Java accepts and the dist already uses for `UseItem`.
    ///
    /// The game-loop test fixtures start here **on purpose**: those tests drive
    /// whole interactions (deposit → withdraw → assert) from a single game
    /// tick, which the shipped 1-second `Transaction` window would refuse, and
    /// silently throttled fixtures are how a real assertion gets weakened to
    /// make a test pass again. The protector's own behaviour — including a
    /// legitimate sequence being throttled under the *dist* config — is covered
    /// by `game_loop::tests::flood_tests`, which turns it back on explicitly.
    pub fn disabled() -> Self {
        Self {
            by_action: [FloodProtectorConfig::default(); FloodAction::COUNT],
        }
    }

    /// Turn **one** slot off (interval 0), leaving the other fourteen as they
    /// are — the middle ground between the shipped config and [`disabled`].
    ///
    /// For a test that exercises a real flow through a protector whose only
    /// effect on it is a wait: the e2e client re-selects a character within
    /// milliseconds, which no human does and the 3-second `CharacterSelect`
    /// window would swallow. Reach for this rather than [`disabled`] so the
    /// remaining protectors still gate the flow being tested.
    ///
    /// [`disabled`]: Self::disabled
    pub fn disable(&mut self, action: FloodAction) {
        self.by_action[action.index()].interval = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser(body: &str) -> PropertiesParser {
        PropertiesParser::from_content("FloodProtector.ini", body)
    }

    /// The shipped dist values, read through the real key names.
    #[test]
    fn reads_the_dist_shape() {
        let p = parser(
            "FloodProtectorUseItemInterval = 0\n\
             FloodProtectorTransactionInterval = 10\n\
             FloodProtectorTransactionLogFlooding = True\n\
             FloodProtectorTransactionPunishmentLimit = 5\n\
             FloodProtectorTransactionPunishmentType = jail\n\
             FloodProtectorTransactionPunishmentTime = 3\n",
        );
        let cfg = FloodProtectorsConfig::from_parser(&p);

        let tx = cfg.get(FloodAction::Transaction);
        assert_eq!(tx.interval, 10);
        assert!(tx.log_flooding);
        assert_eq!(tx.punishment_limit, 5);
        assert_eq!(tx.punishment_type, FloodPunishment::Jail);
        assert_eq!(
            tx.punishment_time_millis, 180_000,
            "the ini is in minutes; Java stores millis"
        );

        assert!(
            !cfg.get(FloodAction::UseItem).is_enabled(),
            "interval 0 disables the protector — the dist ships UseItem off to match retail"
        );
    }

    /// A key that is absent must land on Java's **code** default, not on
    /// whatever the dist happens to ship: `ServerBypass` defaults to 1 tick in
    /// `Config.java` while the shipped ini says 5.
    #[test]
    fn missing_keys_fall_back_to_the_java_code_defaults() {
        let cfg = FloodProtectorsConfig::from_parser(&parser(""));
        assert_eq!(cfg.get(FloodAction::ServerBypass).interval, 1);
        assert_eq!(cfg.get(FloodAction::PlayerAction).interval, 3);
        assert_eq!(cfg.get(FloodAction::RollDice).interval, 42);
        assert_eq!(cfg.get(FloodAction::UseItem).interval, 4);
        assert_eq!(
            cfg.get(FloodAction::Transaction).punishment_type,
            FloodPunishment::None
        );
    }

    /// Java's punishment dispatch is three string equality tests, so an
    /// unrecognised value is silently "no punishment" rather than an error.
    #[test]
    fn an_unknown_punishment_type_disables_the_punishment() {
        let cfg = FloodProtectorsConfig::from_parser(&parser(
            "FloodProtectorDropItemPunishmentType = explode\n",
        ));
        assert_eq!(
            cfg.get(FloodAction::DropItem).punishment_type,
            FloodPunishment::None
        );
    }

    /// The index/table pairing is load-bearing: every slot must round-trip
    /// through its own discriminant, or two protectors share one counter.
    #[test]
    fn every_action_indexes_its_own_slot() {
        for (i, action) in ALL_FLOOD_ACTIONS.iter().enumerate() {
            assert_eq!(action.index(), i, "{} is misplaced", action.type_name());
        }
        assert_eq!(ALL_FLOOD_ACTIONS.len(), FloodAction::COUNT);
    }
}
