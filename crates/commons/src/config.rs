//! Port of `commons/util/PropertiesParser.java`.
//!
//! Reads Java-style `.properties`/`.ini` files and, like the Java version,
//! lets environment variables override file values. The env key is derived
//! from the file path: `config/LoginServer.ini` → `CONFIG_LOGINSERVER_<KEY>`.

use std::collections::HashMap;
use std::path::Path;

use tracing::warn;

pub struct PropertiesParser {
    file_name: String,
    env_path_prefix: String,
    properties: HashMap<String, String>,
}

impl PropertiesParser {
    /// Derives the environment-override prefix from a config path:
    /// `config/LoginServer.ini` → `CONFIG_LOGINSERVER`.
    fn env_prefix(path: &Path) -> String {
        path.to_string_lossy()
            .replace("./", "")
            .trim_end_matches(".ini")
            .replace('.', "")
            .replace('/', "_")
            .trim()
            .to_uppercase()
    }

    /// Loads `{root}{relative}` while deriving the env-override prefix from
    /// `relative` **alone**.
    ///
    /// Without this the variable name follows the datapack location: started
    /// inside `dist/game` the key is `CONFIG_SERVER_URL`, started from the repo
    /// root it becomes `DIST_GAME_CONFIG_SERVER_URL`. An override that works in
    /// one deployment would then silently do nothing in the other.
    pub fn load_rel(root: &str, relative: &str) -> Self {
        let mut parser = Self::load(format!("{root}{relative}"));
        parser.env_path_prefix = Self::env_prefix(Path::new(relative));
        parser
    }

    /// Builds a parser over an in-memory ini body, for tests and for callers
    /// that assemble config without a file on disk. `name` only ever shows up
    /// in the missing-key / invalid-value warnings.
    pub fn from_content(name: &str, content: &str) -> Self {
        let mut properties = HashMap::new();
        parse_properties(content, &mut properties);
        Self {
            file_name: name.to_string(),
            env_path_prefix: Self::env_prefix(Path::new(name)),
            properties,
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let env_path_prefix = Self::env_prefix(path);

        let mut properties = HashMap::new();
        match std::fs::read_to_string(path) {
            Ok(content) => parse_properties(&content, &mut properties),
            Err(e) => {
                let attempted = std::env::current_dir()
                    .map(|d| d.join(path).display().to_string())
                    .unwrap_or_else(|_| path.display().to_string());
                tracing::error!(
                    "[{file_name}] Could not load config file {attempted}: {e} — ALL keys will use defaults"
                );
            }
        }

        Self {
            file_name,
            env_path_prefix,
            properties,
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.properties.contains_key(key)
    }

    fn value(&self, key: &str) -> Option<String> {
        let env_key = format!("{}_{}", self.env_path_prefix, key.to_uppercase());
        std::env::var(env_key)
            .ok()
            .or_else(|| self.properties.get(key).cloned())
            .map(|v| v.trim().to_string())
    }

    pub fn get_string(&self, key: &str, default: &str) -> String {
        match self.value(key) {
            Some(v) => v,
            None => {
                self.warn_missing(key, default);
                default.to_string()
            }
        }
    }

    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        match self.value(key) {
            Some(v) if v.eq_ignore_ascii_case("true") => true,
            Some(v) if v.eq_ignore_ascii_case("false") => false,
            Some(v) => {
                self.warn_invalid(key, &v, default);
                default
            }
            None => {
                self.warn_missing(key, default);
                default
            }
        }
    }

    pub fn get_int(&self, key: &str, default: i32) -> i32 {
        self.get_parsed(key, default)
    }

    pub fn get_long(&self, key: &str, default: i64) -> i64 {
        self.get_parsed(key, default)
    }

    pub fn get_float(&self, key: &str, default: f32) -> f32 {
        self.get_parsed(key, default)
    }

    /// Like [`get_int`](Self::get_int), but an **absent** key yields `None`
    /// instead of a missing-property warning.
    ///
    /// For the handful of keys the reference server does not read either, so
    /// their absence from the shipped `.ini` is the expected state rather than
    /// config drift — `GrandBoss.ini`'s missing `RandomOfBaiumSpawn` is the
    /// motivating case. Reach for this only when the *reference* omits the key
    /// too; using it to quiet a key that genuinely should be present is how the
    /// missing-property warning stops being worth reading.
    ///
    /// A present-but-unparseable value still warns, since that is drift.
    pub fn get_int_opt(&self, key: &str) -> Option<i32> {
        let raw = self.value(key)?;
        match raw.parse() {
            Ok(parsed) => Some(parsed),
            Err(_) => {
                warn!(
                    "[{}] Invalid value specified for key: {key} specified value: {raw} — ignored",
                    self.file_name
                );
                None
            }
        }
    }

    fn get_parsed<T: std::str::FromStr + std::fmt::Display + Copy>(
        &self,
        key: &str,
        default: T,
    ) -> T {
        match self.value(key) {
            Some(v) => match v.parse() {
                Ok(parsed) => parsed,
                Err(_) => {
                    self.warn_invalid(key, &v, default);
                    default
                }
            },
            None => {
                self.warn_missing(key, default);
                default
            }
        }
    }

    fn warn_missing(&self, key: &str, default: impl std::fmt::Display) {
        warn!(
            "[{}] missing property for key: {key} using default value: {default}",
            self.file_name
        );
    }

    fn warn_invalid(&self, key: &str, value: &str, default: impl std::fmt::Display) {
        warn!(
            "[{}] Invalid value specified for key: {key} specified value: {value} using default value: {default}",
            self.file_name
        );
    }
}

/// Minimal `java.util.Properties` line format: `#`/`!` comments, `key = value`
/// (also `:` separator), trailing-backslash line continuation.
fn parse_properties(content: &str, out: &mut HashMap<String, String>) {
    let mut logical = String::new();
    for line in content.lines() {
        let trimmed_start = line.trim_start();
        if logical.is_empty()
            && (trimmed_start.is_empty()
                || trimmed_start.starts_with('#')
                || trimmed_start.starts_with('!'))
        {
            continue;
        }
        if let Some(stripped) = trimmed_start.strip_suffix('\\') {
            logical.push_str(stripped);
            continue;
        }
        logical.push_str(trimmed_start);

        if let Some(sep) = logical.find(['=', ':']) {
            let key = logical[..sep].trim().to_string();
            let value = logical[sep + 1..].trim().to_string();
            if !key.is_empty() {
                out.insert(key, value);
            }
        }
        logical.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_ini() {
        let mut map = HashMap::new();
        parse_properties(
            "# comment\nLoginserverPort = 2106\nEmpty=\nFlag = True\n",
            &mut map,
        );
        assert_eq!(map.get("LoginserverPort").unwrap(), "2106");
        assert_eq!(map.get("Empty").unwrap(), "");
        assert_eq!(map.get("Flag").unwrap(), "True");
    }
}
