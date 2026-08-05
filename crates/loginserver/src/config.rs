//! Login-server section of `Config.java`, reading the same
//! `config/LoginServer.ini` keys.

use commons::config::PropertiesParser;

pub struct LoginConfig {
    pub login_bind_address: String,
    pub port_login: u16,
    pub game_server_login_host: String,
    pub game_server_login_port: u16,

    pub database_url: String,
    pub database_max_connections: u32,

    pub login_try_before_ban: i32,
    pub login_block_after_ban: i32,
    pub accept_new_gameserver: bool,

    pub enable_flood_protection: bool,
    pub fast_connection_limit: i32,
    pub normal_connection_time: i32,
    pub fast_connection_time: i32,
    pub max_connection_per_ip: i32,

    pub enable_cmd_line_login: bool,
    pub only_cmd_line_login: bool,

    pub show_licence: bool,
    pub show_pi_agreement: bool,
    pub auto_create_accounts: bool,
    pub datapack_root: String,

    pub login_server_schedule_restart: bool,
    pub login_server_schedule_restart_time: i64,

    pub backup_database: bool,
    pub backup_path: String,

    /// `InternalStatusBindAddress` — where the internal status channel listens.
    /// **Loopback by default, and that is the security control**: the channel
    /// has no authentication because the kernel will not route off-host traffic
    /// to `127.0.0.1`. Widening this publishes account counts and server
    /// topology to anything that can reach the port.
    pub internal_status_bind_address: String,
    /// `InternalStatusPort` — `0` disables the channel entirely.
    pub internal_status_port: u16,
}

pub const LOGIN_CONFIG_FILE: &str = "dist/login/config/LoginServer.ini";
pub const BANNED_IP_FILE: &str = "dist/login/banned_ip.cfg";

/// The prefix the two paths above share: the login server addresses its files
/// from the *repository root*, not from inside `dist/login`, which is why the
/// systemd unit sets `WorkingDirectory` to the deployment root. Anything else
/// that has to sit beside `LoginServer.ini` — `Logging.ini`, the `log/`
/// directory — must be resolved against this, or it silently lands one or two
/// levels up from the rest of the datapack.
pub const LOGIN_ROOT: &str = "dist/login/";

impl LoginConfig {
    pub fn load() -> Self {
        Self::from_parser(&PropertiesParser::load(LOGIN_CONFIG_FILE))
    }

    /// Split out from [`load`](Self::load) so a caller (or a test) can supply
    /// the ini body directly; every key and default is unchanged.
    pub fn from_parser(p: &PropertiesParser) -> Self {
        Self {
            internal_status_bind_address: p.get_string("InternalStatusBindAddress", "127.0.0.1"),
            internal_status_port: p.get_int("InternalStatusPort", 7778) as u16,
            login_bind_address: p.get_string("LoginserverHostname", "0.0.0.0"),
            port_login: p.get_int("LoginserverPort", 2106) as u16,
            game_server_login_host: p.get_string("LoginHostname", "127.0.0.1"),
            game_server_login_port: p.get_int("LoginPort", 9014) as u16,

            database_url: p.get_string("URL", "jdbc:sqlite:./data/l2jmobius.db"),
            database_max_connections: p.get_int("MaximumDatabaseConnections", 5).max(1) as u32,

            login_try_before_ban: p.get_int("LoginTryBeforeBan", 5),
            login_block_after_ban: p.get_int("LoginBlockAfterBan", 900),
            accept_new_gameserver: p.get_bool("AcceptNewGameServer", true),

            enable_flood_protection: p.get_bool("EnableFloodProtection", true),
            fast_connection_limit: p.get_int("FastConnectionLimit", 15),
            normal_connection_time: p.get_int("NormalConnectionTime", 700),
            fast_connection_time: p.get_int("FastConnectionTime", 350),
            max_connection_per_ip: p.get_int("MaxConnectionPerIP", 50),

            enable_cmd_line_login: p.get_bool("EnableCmdLineLogin", false),
            only_cmd_line_login: p.get_bool("OnlyCmdLineLogin", false),

            show_licence: p.get_bool("ShowLicence", true),
            show_pi_agreement: p.get_bool("ShowPIAgreement", false),
            auto_create_accounts: p.get_bool("AutoCreateAccounts", true),
            datapack_root: p.get_string("DatapackRoot", "."),

            login_server_schedule_restart: p.get_bool("LoginRestartSchedule", false),
            login_server_schedule_restart_time: p.get_long("LoginRestartTime", 24),

            backup_database: p.get_bool("BackupDatabase", false),
            backup_path: p.get_string("BackupPath", "../backup/"),
        }
    }
}
