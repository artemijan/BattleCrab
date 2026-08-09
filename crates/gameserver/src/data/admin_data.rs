//! Port of `data/xml/AdminData` + `model/AccessLevel` +
//! `model/AdminCommandAccessRight` — the GM access-level table and per-command
//! access rights. Loads `config/AccessLevels.xml` (9 levels: Banned −1 …
//! Master 100) and `config/AdminCommands.xml` (per-command required level +
//! optional confirm dialog).
//!
//! Java splits this across three classes and a singleton; here it is one
//! `AdminData` value owned by `GameData`. The `childAccess` privilege chain
//! (a high level "contains" the commands of the lower levels it descends
//! through) is walked on demand in [`AdminData::has_child_access`] rather than
//! lazily cached on each `AccessLevel` as Java does.

use std::collections::HashMap;

use crate::data::xml::attr_str;
use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

pub const ACCESS_LEVELS_FILE: &str = "config/AccessLevels.xml";
pub const ADMIN_COMMANDS_FILE: &str = "config/AdminCommands.xml";

/// The default color Java uses when `nameColor`/`titleColor` is absent
/// (`Integer.decode("0xFFFFFF")`).
const DEFAULT_COLOR: i32 = 0xFF_FFFF;

/// Port of `model/AccessLevel`. One GM (or user/banned) tier.
#[derive(Debug, Clone)]
pub struct AccessLevel {
    pub level: i32,
    pub name: String,
    /// BGR-packed name color (client order); Java stores the raw hex.
    pub name_color: i32,
    pub title_color: i32,
    /// `childAccess` — the next level down whose commands this level inherits
    /// (0 = none). Walked by [`AdminData::has_child_access`].
    pub child: i32,
    pub is_gm: bool,
    pub allow_peace_attack: bool,
    pub allow_fixed_res: bool,
    pub allow_transaction: bool,
    pub allow_alt_g: bool,
    pub give_damage: bool,
    pub take_aggro: bool,
    pub gain_exp: bool,
}

impl Default for AccessLevel {
    fn default() -> Self {
        Self::user_default()
    }
}

impl AccessLevel {
    /// Java `AccessLevel()` no-arg ctor — the level-0 "User" fallback used when
    /// a character's stored access level does not resolve to a defined tier.
    pub fn user_default() -> Self {
        Self {
            level: 0,
            name: "User".to_string(),
            name_color: DEFAULT_COLOR,
            title_color: DEFAULT_COLOR,
            child: 0,
            is_gm: false,
            allow_peace_attack: false,
            allow_fixed_res: false,
            allow_transaction: true,
            allow_alt_g: false,
            give_damage: true,
            take_aggro: true,
            gain_exp: true,
        }
    }
}

/// Port of `model/AdminCommandAccessRight`.
#[derive(Debug, Clone)]
pub struct AdminCommandAccessRight {
    pub command: String,
    /// Required access level (Java default 7 when the attribute is absent).
    pub access_level: i32,
    pub require_confirm: bool,
}

#[derive(Default)]
pub struct AdminData {
    access_levels: HashMap<i32, AccessLevel>,
    command_rights: HashMap<String, AdminCommandAccessRight>,
    highest_level: i32,
    /// The level-0 "User" fallback, handed out by [`AdminData::access_level`]
    /// when a lookup misses (mirrors Java handing back a fresh `AccessLevel()`).
    user_default: AccessLevel,
}

impl AdminData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut data = Self {
            user_default: AccessLevel::user_default(),
            ..Default::default()
        };
        data.parse_access_levels(file_path);
        data.parse_admin_commands(file_path);
        info!(
            "AdminData: Loaded {} access levels, {} access commands.",
            data.access_levels.len(),
            data.command_rights.len()
        );
        data
    }

    fn parse_access_levels(&mut self, file_path: &str) {
        let Ok(content) = std::fs::read_to_string(format!("{file_path}{ACCESS_LEVELS_FILE}"))
        else {
            return;
        };
        let mut reader = Reader::from_str(&content);
        while let Ok(event) = reader.read_event() {
            let e = match event {
                Event::Start(e) | Event::Empty(e) => e,
                Event::Eof => break,
                _ => continue,
            };
            if e.name().as_ref() != b"access" {
                continue;
            }
            let attr = |key: &[u8]| attr_str(&e, key);
            let get_i32 = |key: &[u8], default: i32| {
                attr(key)
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(default)
            };
            let get_bool = |key: &[u8], default: bool| {
                attr(key)
                    .map(|v| v.eq_ignore_ascii_case("true"))
                    .unwrap_or(default)
            };
            let get_color = |key: &[u8]| {
                attr(key)
                    .and_then(|v| i32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
                    .unwrap_or(DEFAULT_COLOR)
            };
            let Some(level) = attr(b"level").and_then(|v| v.parse::<i32>().ok()) else {
                continue;
            };
            let access = AccessLevel {
                level,
                name: attr(b"name").unwrap_or_default(),
                name_color: get_color(b"nameColor"),
                title_color: get_color(b"titleColor"),
                child: get_i32(b"childAccess", 0),
                is_gm: get_bool(b"isGM", false),
                allow_peace_attack: get_bool(b"allowPeaceAttack", false),
                allow_fixed_res: get_bool(b"allowFixedRes", false),
                allow_transaction: get_bool(b"allowTransaction", true),
                allow_alt_g: get_bool(b"allowAltg", false),
                give_damage: get_bool(b"giveDamage", true),
                take_aggro: get_bool(b"takeAggro", true),
                gain_exp: get_bool(b"gainExp", true),
            };
            if level > self.highest_level {
                self.highest_level = level;
            }
            self.access_levels.insert(level, access);
        }
    }

    fn parse_admin_commands(&mut self, file_path: &str) {
        let Ok(content) = std::fs::read_to_string(format!("{file_path}{ADMIN_COMMANDS_FILE}"))
        else {
            return;
        };
        let mut reader = Reader::from_str(&content);
        while let Ok(event) = reader.read_event() {
            let e = match event {
                Event::Start(e) | Event::Empty(e) => e,
                Event::Eof => break,
                _ => continue,
            };
            if e.name().as_ref() != b"admin" {
                continue;
            }
            let attr = |key: &[u8]| attr_str(&e, key);
            let Some(command) = attr(b"command") else {
                continue;
            };
            let access_level = attr(b"accessLevel")
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(7);
            let require_confirm = attr(b"confirmDlg")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            // Key on the lowercased command: L2J's handlers dispatch on
            // `actualCommand.toLowerCase()`, and some XML entries are camelCase
            // (e.g. `admin_deleteNpcByObjectId`, the `admin_chsiege_*` family)
            // while the bypass/`//` bar that triggers them may use any case.
            // Lookups (`has_command`/`has_access`/`require_confirm`) match the
            // already-lowercased incoming command word.
            self.command_rights.insert(
                command.to_ascii_lowercase(),
                AdminCommandAccessRight {
                    command,
                    access_level,
                    require_confirm,
                },
            );
        }
    }

    /// Java `AdminData.getAccessLevel` — negatives collapse to the Banned
    /// (`-1`) tier; a miss returns the shared level-0 "User" fallback (Java
    /// hands back a fresh `AccessLevel()`).
    pub fn access_level(&self, level: i32) -> &AccessLevel {
        let key = if level < 0 { -1 } else { level };
        self.access_levels.get(&key).unwrap_or(&self.user_default)
    }

    /// `true` if this level number resolves to a defined GM tier.
    pub fn is_gm(&self, level: i32) -> bool {
        self.access_level(level).is_gm
    }

    pub fn highest_level(&self) -> i32 {
        self.highest_level
    }

    /// `true` if the command has a defined access right (Java: a handler is
    /// registered). The "known command" set for dispatch.
    pub fn has_command(&self, command: &str) -> bool {
        self.command_rights.contains_key(command)
    }

    /// Java `AdminData.requireConfirm` — undefined commands do not confirm.
    pub fn require_confirm(&self, command: &str) -> bool {
        self.command_rights
            .get(command)
            .is_some_and(|r| r.require_confirm)
    }

    /// Java `AdminData.hasAccess`: an undefined command is granted only to the
    /// master level (Java auto-registers a right for it; we grant without the
    /// caching insert), otherwise checked against its right.
    pub fn has_access(&self, command: &str, char_level: i32) -> bool {
        match self.command_rights.get(command) {
            Some(right) => self.right_has_access(right, char_level),
            None => char_level > 0 && char_level == self.highest_level,
        }
    }

    /// Java `AdminCommandAccessRight.hasAccess`.
    fn right_has_access(&self, right: &AdminCommandAccessRight, char_level: i32) -> bool {
        // The required tier must itself be defined (Java: `getAccessLevel`
        // non-null); a right pointing at an unknown level denies everyone.
        let Some(required) = self.access_levels.get(&right.access_level) else {
            return false;
        };
        required.level == char_level || self.has_child_access(char_level, required.level)
    }

    /// Java `AccessLevel.hasChildAccess`: does `char_level`'s childAccess chain
    /// reach `target_level`? The chain is strictly descending in the real data;
    /// a depth cap guards against a malformed cyclic config.
    pub fn has_child_access(&self, char_level: i32, target_level: i32) -> bool {
        let mut current = self.access_levels.get(&char_level);
        for _ in 0..self.access_levels.len().max(1) {
            let Some(level) = current else { return false };
            if level.child <= 0 {
                return false;
            }
            match self.access_levels.get(&level.child) {
                Some(child) if child.level == target_level => return true,
                Some(child) => current = Some(child),
                None => return false,
            }
        }
        false
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            user_default: AccessLevel::user_default(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real() -> AdminData {
        AdminData::load_from(crate::data::DIST_GAME)
    }

    #[test]
    fn loads_real_dist_files() {
        let data = real();
        // AccessLevels.xml: Banned(-1), User(0), 10,20,30,40,50,60,70,100.
        assert_eq!(data.access_levels.len(), 10);
        assert_eq!(data.highest_level(), 100);
        // AdminCommands.xml command count (verified against the raw file).
        assert_eq!(data.command_rights.len(), 458);
    }

    #[test]
    fn access_level_lookup_and_negatives() {
        let data = real();
        assert_eq!(data.access_level(100).name, "Master");
        assert!(data.access_level(100).is_gm);
        assert!(!data.access_level(0).is_gm);
        // Negatives collapse to Banned(-1).
        assert_eq!(data.access_level(-5).level, -1);
        assert_eq!(data.access_level(-1).name, "Banned");
        // A miss hands back the level-0 User fallback.
        assert_eq!(data.access_level(999).level, 0);
    }

    #[test]
    fn master_passes_defined_and_undefined_commands() {
        let data = real();
        // Defined command at level 100.
        assert!(data.has_access("admin_heal", 100));
        // Undefined command: auto-granted to the highest (master) level only.
        assert!(data.has_access("admin_totally_made_up", 100));
        assert!(!data.has_access("admin_totally_made_up", 70));
        assert!(!data.has_access("admin_totally_made_up", 0));
    }

    #[test]
    fn non_gm_and_child_chain_gating() {
        let data = real();
        // admin_heal requires level 100; a plain user is denied.
        assert!(!data.has_access("admin_heal", 0));
        // Master(100) childAccess chain: 100→70→60→50→40→30→20→10→0. A right
        // required at level 70 is reachable from 100 via the chain.
        assert!(data.has_child_access(100, 70));
        assert!(data.has_child_access(100, 10));
        // The chain only descends: level 20 cannot reach master(100).
        assert!(!data.has_child_access(20, 100));
        // Exact-match still passes regardless of chain direction.
        assert!(data.has_access("admin_heal", 100));
    }

    #[test]
    fn require_confirm_reads_flag() {
        let data = real();
        // admin_givehero has confirmDlg="true"; admin_heal does not.
        assert!(data.require_confirm("admin_givehero"));
        assert!(!data.require_confirm("admin_heal"));
        assert!(!data.require_confirm("admin_undefined"));
    }

    #[test]
    fn camelcase_commands_resolve_case_insensitively() {
        let data = real();
        // AdminCommands.xml registers `admin_deleteNpcByObjectId` (camelCase,
        // confirmDlg="true"), but the scan list's Delete link and the dispatch
        // arm are lowercase. The table keys on the lowercased command so all
        // three agree — otherwise the command would be unreachable.
        assert!(data.has_command("admin_deletenpcbyobjectid"));
        assert!(data.has_access("admin_deletenpcbyobjectid", 100));
        assert!(data.require_confirm("admin_deletenpcbyobjectid"));
    }
}
