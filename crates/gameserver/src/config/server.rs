//! `Server.ini` — port of the `SERVER_CONFIG_FILE` block of `Config.java`.

use commons::config::PropertiesParser;

/// Matches Java's constant, and is **datapack-relative**: `Config::load_from`
/// joins it onto the datapack root.
///
/// Unlike the Java server (whose start scripts `cd` into `dist/game`), this one
/// does not change its working directory. Datapack-relative ini values —
/// `ScriptRoot`, `GeoDataPath`, `BackupPath` — are resolved against that root
/// as the config is parsed. The SQLite `URL` is deliberately not among them:
/// it resolves against the *executable's* directory (`commons::db`), because
/// the database is shared with the login server and so must be addressed
/// neither relative to this server's datapack nor to its working directory.
pub const SERVER_CONFIG_FILE: &str = "config/Server.ini";

pub struct ServerConfig {
    // Network binding.
    pub gameserver_hostname: String,
    pub port_game: u16,
    pub game_server_login_host: String,
    pub game_server_login_port: u16,
    pub packet_encryption: bool,
    pub request_id: i32,
    pub accept_alternate_id: bool,

    // Database.
    pub database_url: String,
    pub database_login: String,
    pub database_password: String,
    pub database_max_connections: u32,
    pub database_test_connections: bool,
    pub backup_database: bool,
    pub backup_path: String,
    pub backup_days: i32,

    // Data roots.
    pub datapack_root: String,
    pub script_root: String,

    // Player / world limits.
    pub pet_name_template: String,
    pub clan_name_template: String,
    pub char_name_template: String,
    pub max_characters_number_per_account: i32,
    pub maximum_online_users: i32,

    // Protocol.
    pub protocol_list: Vec<i32>,
    /// Bitmask from `getServerTypeId` (Classic = 0x400). `KeyPacket` sends the
    /// `isClassic` flag as `(server_list_type & 0x400) == 0x400`.
    pub server_list_type: i32,
    pub server_list_age: i32,
    pub server_list_bracket: bool,

    // Thread pools (kept for parity/logging; the Rust runtime sizes itself).
    pub scheduled_thread_pool_size: i32,
    pub instant_thread_pool_size: i32,
    pub threads_for_loading: bool,

    // Stability.
    pub deadlock_detector: bool,
    pub deadlock_check_interval: i32,
    pub restart_on_deadlock: bool,

    // Scheduled restart.
    pub server_restart_schedule_enabled: bool,
    pub server_restart_schedule_message: bool,
    pub server_restart_schedule_countdown: i32,
    pub server_restart_schedule: Vec<String>,
    pub server_restart_days: Vec<i32>,

    // Precautionary restart.
    pub precautionary_restart_enabled: bool,
    pub precautionary_restart_percentage: i32,
    pub precautionary_restart_delay: i32,
}

impl ServerConfig {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        let p = PropertiesParser::load_rel(root, SERVER_CONFIG_FILE);

        let protocol_list = split_ints(
            &p.get_string("AllowedProtocolRevisions", "603;606;607"),
            ';',
        );
        let server_restart_schedule = p
            .get_string("ServerRestartSchedule", "08:00")
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        let server_restart_days = split_ints(p.get_string("ServerRestartDays", "").trim(), ',');

        Self {
            gameserver_hostname: p.get_string("GameserverHostname", "0.0.0.0"),
            port_game: p.get_int("GameserverPort", 7777) as u16,
            game_server_login_port: p.get_int("LoginPort", 9014) as u16,
            game_server_login_host: p.get_string("LoginHost", "127.0.0.1"),
            packet_encryption: p.get_bool("PacketEncryption", false),
            request_id: p.get_int("RequestServerID", 0),
            accept_alternate_id: p.get_bool("AcceptAlternateID", true),

            database_url: p.get_string("URL", "jdbc:sqlite:./data/l2jmobius.db"),
            database_login: p.get_string("Login", "root"),
            database_password: p.get_string("Password", ""),
            database_max_connections: p.get_int("MaximumDatabaseConnections", 10).max(1) as u32,
            database_test_connections: p.get_bool("TestDatabaseConnections", false),
            backup_database: p.get_bool("BackupDatabase", false),
            backup_path: super::datapack_path(root, &p.get_string("BackupPath", "../backup/")),
            backup_days: p.get_int("BackupDays", 30),

            datapack_root: super::datapack_path(root, &p.get_string("DatapackRoot", ".")),
            script_root: super::datapack_path(root, &p.get_string("ScriptRoot", "./data/scripts")),

            pet_name_template: p.get_string("PetNameTemplate", ".*"),
            clan_name_template: p.get_string("ClanNameTemplate", ".*"),
            char_name_template: p.get_string("CnameTemplate", ".*"),
            max_characters_number_per_account: p.get_int("CharMaxNumber", 7),
            maximum_online_users: p.get_int("MaximumOnlineUsers", 100),

            protocol_list,
            server_list_type: parse_server_type(&p.get_string("ServerListType", "Free")),
            server_list_age: p.get_int("ServerListAge", 0),
            server_list_bracket: p.get_bool("ServerListBrackets", false),

            scheduled_thread_pool_size: p.get_int("ScheduledThreadPoolSize", -1),
            instant_thread_pool_size: p.get_int("InstantThreadPoolSize", -1),
            threads_for_loading: p.get_bool("ThreadsForLoading", false),

            deadlock_detector: p.get_bool("DeadLockDetector", true),
            deadlock_check_interval: p.get_int("DeadLockCheckInterval", 20),
            restart_on_deadlock: p.get_bool("RestartOnDeadlock", false),

            server_restart_schedule_enabled: p.get_bool("ServerRestartScheduleEnabled", false),
            server_restart_schedule_message: p.get_bool("ServerRestartScheduleMessage", false),
            server_restart_schedule_countdown: p.get_int("ServerRestartScheduleCountdown", 600),
            server_restart_schedule,
            server_restart_days,

            precautionary_restart_enabled: p.get_bool("PrecautionaryRestartEnabled", false),
            precautionary_restart_percentage: p.get_int("PrecautionaryRestartPercentage", 95),
            precautionary_restart_delay: p.get_int("PrecautionaryRestartDelay", 60) * 1000,
        }
    }
}

/// Split on `sep` and parse the trimmed pieces as ints, skipping non-numeric
/// entries (mirrors Java's per-token `NumberFormatException`/`isDigit` skip).
fn split_ints(s: &str, sep: char) -> Vec<i32> {
    s.split(sep)
        .filter_map(|tok| tok.trim().parse::<i32>().ok())
        .collect()
}

/// Port of `Config.getServerTypeId` — comma-separated type names → bitmask.
fn parse_server_type(types: &str) -> i32 {
    let mut server_type = 0;
    for c_type in types.split(',') {
        server_type |= match c_type.trim().to_lowercase().as_str() {
            "normal" => 0x01,
            "relax" => 0x02,
            "test" => 0x04,
            "broad" => 0x08,
            "restricted" => 0x10,
            "event" => 0x20,
            "free" => 0x40,
            "world" => 0x100,
            "new" => 0x200,
            "classic" => 0x400,
            _ => 0,
        };
    }
    server_type
}
