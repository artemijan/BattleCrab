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
}

pub const LOGIN_CONFIG_FILE: &str = "dist/login/config/LoginServer.ini";
pub const BANNED_IP_FILE: &str = "dist/login/banned_ip.cfg";

impl LoginConfig {
    pub fn load() -> Self {
        let p = PropertiesParser::load(LOGIN_CONFIG_FILE);
        Self {
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
