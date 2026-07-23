//! `hexid.txt` — the game server's identity on the login server
//! (`Config` HEXID_FILE block + `LoginServerThread` constructor).

use commons::config::PropertiesParser;
use commons::util::{generate_hex, hexid_from_string, hexid_to_string};

pub const HEXID_FILE: &str = "config/hexid.txt";

pub struct HexId {
    pub hex_id: Vec<u8>,
    pub server_id: i32,
    /// Where this was read from, so `save` writes back to the same file rather
    /// than to whatever the working directory happens to be.
    path: String,
}

impl HexId {
    /// Java: if `hexid.txt` has `HexID`+`ServerID`, use them and `SERVER_ID`;
    /// otherwise generate a fresh 16-byte hexid and use `RequestServerID`.
    pub fn load(request_id: i32) -> Self {
        Self::load_from("", request_id)
    }

    pub fn load_from(root: &str, request_id: i32) -> Self {
        let path = format!("{root}{HEXID_FILE}");
        if std::path::Path::new(&path).exists() {
            let p = PropertiesParser::load_rel(root, HEXID_FILE);
            if p.contains_key("ServerID") && p.contains_key("HexID") {
                if let Some(hex_id) = hexid_from_string(&p.get_string("HexID", "")) {
                    return Self {
                        hex_id,
                        server_id: p.get_int("ServerID", 1),
                        path,
                    };
                }
            }
        }
        Self {
            hex_id: generate_hex(16),
            server_id: request_id,
            path,
        }
    }

    /// Java `Config.saveHexid`: persist the id assigned by the login server so
    /// the next boot registers as the same server.
    pub fn save(&self, server_id: i32) {
        let content = format!(
            "#The HexId to Auth into LoginServer\nHexID={}\nServerID={}\n",
            hexid_to_string(&self.hex_id),
            server_id
        );
        if let Err(e) = std::fs::write(&self.path, content) {
            tracing::warn!("Failed to save {}: {e}", self.path);
        }
    }
}
