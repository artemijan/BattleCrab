//! The small `config/Custom/*.ini` files — one struct per file, grouped here
//! because none of them is more than a handful of keys.
//!
//! Each is enabled on this dist and live in Java, which is what pulls it back
//! inside the ROADMAP scope gate ("the Mobius `config/Custom/*` features are
//! out of scope **except any the operator explicitly enables**"). See
//! `PLAN_G33_CUSTOM_INI_AUDIT.md` for the audit that found them.

use commons::config::PropertiesParser;

pub const ALLOWED_PLAYER_RACES_CONFIG_FILE: &str = "config/Custom/AllowedPlayerRaces.ini";
pub const BANKING_CONFIG_FILE: &str = "config/Custom/Banking.ini";
pub const BOSS_ANNOUNCEMENTS_CONFIG_FILE: &str = "config/Custom/BossAnnouncements.ini";
pub const ONLINE_INFO_CONFIG_FILE: &str = "config/Custom/OnlineInfo.ini";
pub const PRIVATE_STORE_RANGE_CONFIG_FILE: &str = "config/Custom/PrivateStoreRange.ini";
pub const WALKER_BOT_PROTECTION_CONFIG_FILE: &str = "config/Custom/WalkerBotProtection.ini";
pub const CUSTOM_MAIL_MANAGER_CONFIG_FILE: &str = "config/Custom/CustomMailManager.ini";

/// `Custom/CustomMailManager.ini` — the inbound `custom_mail` table poll.
#[derive(Debug, Clone)]
pub struct CustomMailConfig {
    /// `CustomMailManagerEnabled` (**True** here).
    pub enabled: bool,
    /// `DatabaseQueryDelay` (30) — seconds between polls. Java multiplies it by
    /// 1000 for its millisecond scheduler; kept in seconds here.
    pub query_delay_secs: i32,
}

impl Default for CustomMailConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            query_delay_secs: 30,
        }
    }
}

impl CustomMailConfig {
    pub fn load_from(root: &str) -> Self {
        let d = Self::default();
        let p = PropertiesParser::load_rel(root, CUSTOM_MAIL_MANAGER_CONFIG_FILE);
        Self {
            enabled: p.get_bool("CustomMailManagerEnabled", d.enabled),
            query_delay_secs: p.get_int("DatabaseQueryDelay", d.query_delay_secs),
        }
    }
}

/// `Custom/AllowedPlayerRaces.ini` — which races may be created. All five are
/// `True` on this dist, so the gate is a no-op today; it is ported because an
/// operator flipping one off is exactly the "explicitly enables" case, and a
/// missing gate would silently ignore them.
#[derive(Debug, Clone)]
pub struct AllowedRacesConfig {
    /// Indexed by Java's `Race` ordinal: 0 human, 1 elf, 2 dark elf, 3 orc,
    /// 4 dwarf. Java's own defaults are all `true`.
    allowed: [bool; 5],
}

impl Default for AllowedRacesConfig {
    fn default() -> Self {
        Self { allowed: [true; 5] }
    }
}

impl AllowedRacesConfig {
    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(
            root,
            ALLOWED_PLAYER_RACES_CONFIG_FILE,
        ))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        Self {
            allowed: [
                p.get_bool("AllowHuman", true),
                p.get_bool("AllowElf", true),
                p.get_bool("AllowDarkElf", true),
                p.get_bool("AllowOrc", true),
                p.get_bool("AllowDwarf", true),
            ],
        }
    }

    /// Test seam: build a config with an explicit allow-list.
    pub fn with_allowed_for_test(allowed: [bool; 5]) -> Self {
        Self { allowed }
    }

    /// Whether `race` (a Java `Race` ordinal) may be created. An unknown
    /// ordinal is allowed — Java's `switch` has no `default` arm that refuses,
    /// so a race it does not name falls through to creation.
    pub fn allows(&self, race: i32) -> bool {
        usize::try_from(race)
            .ok()
            .and_then(|i| self.allowed.get(i).copied())
            .unwrap_or(true)
    }
}

/// `Custom/Banking.ini` — the `.deposit` / `.withdraw` voiced commands that
/// swap adena for goldbars at a fixed rate.
#[derive(Debug, Clone)]
pub struct BankingConfig {
    /// `BankingEnabled` (**True** here; Java's default is false).
    pub enabled: bool,
    /// `BankingGoldbarCount` — goldbars one deposit yields (1 here).
    pub goldbars: i64,
    /// `BankingAdenaCount` — adena one deposit costs (1 000 000 000 here).
    pub adena: i64,
}

impl Default for BankingConfig {
    fn default() -> Self {
        // Java `Config`'s defaults for an absent file.
        Self {
            enabled: false,
            goldbars: 1,
            adena: 500_000_000,
        }
    }
}

impl BankingConfig {
    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(root, BANKING_CONFIG_FILE))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            enabled: p.get_bool("BankingEnabled", d.enabled),
            goldbars: p.get_long("BankingGoldbarCount", d.goldbars),
            adena: p.get_long("BankingAdenaCount", d.adena),
        }
    }
}

/// `Custom/BossAnnouncements.ini` — the server-wide "X has spawned!" lines.
/// Only the two *spawn* flags are on here; the defeat and instance flags ship
/// `false`, so those arms are parsed and never taken.
#[derive(Debug, Clone, Default)]
pub struct BossAnnouncementsConfig {
    pub raidboss_spawn: bool,
    pub raidboss_defeat: bool,
    pub raidboss_instance: bool,
    pub grandboss_spawn: bool,
    pub grandboss_defeat: bool,
    pub grandboss_instance: bool,
}

impl BossAnnouncementsConfig {
    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(
            root,
            BOSS_ANNOUNCEMENTS_CONFIG_FILE,
        ))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        Self {
            raidboss_spawn: p.get_bool("RaidBossSpawnAnnouncements", false),
            raidboss_defeat: p.get_bool("RaidBossDefeatAnnouncements", false),
            raidboss_instance: p.get_bool("RaidBossInstanceAnnouncements", false),
            grandboss_spawn: p.get_bool("GrandBossSpawnAnnouncements", false),
            grandboss_defeat: p.get_bool("GrandBossDefeatAnnouncements", false),
            grandboss_instance: p.get_bool("GrandBossInstanceAnnouncements", false),
        }
    }
}

/// `Custom/OnlineInfo.ini` + `Custom/PrivateStoreRange.ini` +
/// `Custom/WalkerBotProtection.ini` — one key each.
#[derive(Debug, Clone)]
pub struct CustomMiscConfig {
    /// `EnableOnlineCommand` — the `.online` player count.
    pub online_command: bool,
    /// `ShopMinRangeFromPlayer` (50) — how close another **seated** player may
    /// be when you open a private store. Java reads it through
    /// `Player.getMinShopDistance`, which returns it only while sitting, so it
    /// spaces shops out rather than blocking on any bystander.
    pub shop_min_range_from_player: i32,
    /// `ShopMinRangeFromNpc` (100) — the same distance from any NPC.
    pub shop_min_range_from_npc: i32,
    /// `L2WalkerProtection` — refuse a whisper whose text opens with an
    /// L2Walker bot-client command, and punish the sender.
    pub walker_protection: bool,
}

impl Default for CustomMiscConfig {
    fn default() -> Self {
        // Java's defaults for absent files.
        Self {
            online_command: false,
            shop_min_range_from_player: 50,
            shop_min_range_from_npc: 100,
            walker_protection: false,
        }
    }
}

impl CustomMiscConfig {
    pub fn load_from(root: &str) -> Self {
        let d = Self::default();
        let online = PropertiesParser::load_rel(root, ONLINE_INFO_CONFIG_FILE);
        let shop = PropertiesParser::load_rel(root, PRIVATE_STORE_RANGE_CONFIG_FILE);
        let walker = PropertiesParser::load_rel(root, WALKER_BOT_PROTECTION_CONFIG_FILE);
        Self {
            online_command: online.get_bool("EnableOnlineCommand", d.online_command),
            shop_min_range_from_player: shop
                .get_int("ShopMinRangeFromPlayer", d.shop_min_range_from_player),
            shop_min_range_from_npc: shop.get_int("ShopMinRangeFromNpc", d.shop_min_range_from_npc),
            walker_protection: walker.get_bool("L2WalkerProtection", d.walker_protection),
        }
    }
}

/// Java `Say2.WALKER_COMMAND_LIST` — the L2Walker bot client announces itself
/// by whispering these verbs, so a whisper *starting with* one is taken as
/// proof of an emulator rather than a player typing.
pub const WALKER_COMMAND_LIST: [&str; 8] = [
    "USESKILL", "USEITEM", "BUYITEM", "SELLITEM", "SAVEITEM", "LOADITEM", "MSG", "SET",
];

#[cfg(test)]
mod tests {
    use super::*;

    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

    /// The dist's values, which are the specification. `BankingEnabled` and
    /// `EnableOnlineCommand` both differ from Java's code default, so reading
    /// the file is what makes them true.
    #[test]
    fn dist_values_load() {
        let banking = BankingConfig::load_from(DIST);
        assert!(banking.enabled, "BankingEnabled = True");
        assert_eq!(banking.goldbars, 1);
        assert_eq!(banking.adena, 1_000_000_000);

        let misc = CustomMiscConfig::load_from(DIST);
        assert!(misc.online_command, "EnableOnlineCommand = True");
        assert!(misc.walker_protection, "L2WalkerProtection = True");
        assert_eq!(misc.shop_min_range_from_player, 50);
        assert_eq!(misc.shop_min_range_from_npc, 100);

        let boss = BossAnnouncementsConfig::load_from(DIST);
        assert!(
            boss.raidboss_spawn && boss.grandboss_spawn,
            "spawn lines on"
        );
        assert!(
            !boss.raidboss_defeat && !boss.grandboss_defeat,
            "defeat lines off — those arms are parsed and never taken"
        );

        let races = AllowedRacesConfig::load_from(DIST);
        assert!(
            (0..5).all(|r| races.allows(r)),
            "every race is allowed on this dist"
        );
    }

    /// An unknown race ordinal falls through to allowed — Java's `switch` has
    /// no refusing `default` arm.
    #[test]
    fn an_unknown_race_is_allowed() {
        let races = AllowedRacesConfig {
            allowed: [false; 5],
        };
        assert!(!races.allows(0));
        assert!(races.allows(9), "no arm for it in Java either");
        assert!(races.allows(-1));
    }
}
