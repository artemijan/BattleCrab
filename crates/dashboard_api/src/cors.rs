//! Which browser origins may call this API with credentials.
//!
//! The rule is "`battlecrab.com` and its subdomains, nothing else". That is a
//! suffix match, and a naive suffix match is a security hole: `ends_with(
//! "battlecrab.com")` also accepts `evilbattlecrab.com`, which an attacker can
//! register. Every comparison below is therefore explicit about label
//! boundaries, scheme, and ports, and the tests are written adversarially.
//!
//! Getting this wrong is not a cosmetic bug — an accepted origin can drive a
//! logged-in user's account, because CORS here is credentialed.

/// One entry from `AllowedOrigins`.
#[derive(Debug, Clone, PartialEq)]
pub enum OriginRule {
    /// A full origin, matched exactly (`http://localhost:3000`).
    Exact(String),
    /// A registrable domain: matches the domain itself and any subdomain,
    /// over HTTPS only (`battlecrab.com`).
    Domain(String),
}

#[derive(Debug, Clone, Default)]
pub struct OriginPolicy {
    rules: Vec<OriginRule>,
}

impl OriginPolicy {
    /// Parses a comma-separated list.
    ///
    /// An entry containing `://` is an exact origin; anything else is a domain
    /// (a leading `*.` is accepted and ignored, since `*.battlecrab.com` is the
    /// obvious way to write it).
    pub fn parse(raw: &str) -> Self {
        let rules = raw
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                let entry = entry.trim_end_matches('/').to_ascii_lowercase();
                if entry.contains("://") {
                    OriginRule::Exact(entry)
                } else {
                    OriginRule::Domain(entry.trim_start_matches("*.").to_string())
                }
            })
            .collect();
        Self { rules }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn rules(&self) -> &[OriginRule] {
        &self.rules
    }

    /// Whether an `Origin` header value is allowed.
    pub fn allows(&self, origin: &str) -> bool {
        let origin = origin.trim();
        // "null" is what a sandboxed iframe or a file:// page sends. Never trust it.
        if origin.is_empty() || origin.eq_ignore_ascii_case("null") {
            return false;
        }
        let origin = origin.to_ascii_lowercase();

        self.rules.iter().any(|rule| match rule {
            OriginRule::Exact(allowed) => *allowed == origin,
            OriginRule::Domain(domain) => matches_domain(&origin, domain),
        })
    }
}

/// `https://<domain>` or `https://<label>.<domain>`, with an optional port.
///
/// HTTPS only: allowing `http://battlecrab.com` would let anyone who can
/// intercept plaintext present themselves as a trusted origin.
fn matches_domain(origin: &str, domain: &str) -> bool {
    let Some(host_port) = origin.strip_prefix("https://") else {
        return false;
    };

    // An Origin is scheme + host + port and nothing else. Anything with a path,
    // query, fragment, or credentials is malformed — reject rather than parse.
    if host_port.is_empty()
        || host_port.contains('/')
        || host_port.contains('?')
        || host_port.contains('#')
        || host_port.contains('@')
    {
        return false;
    }

    // Strip a port, but only when it really is one: `:` followed by digits.
    // (An IPv6 literal would contain brackets; those never match a domain, and
    // the bracket check below rejects them outright.)
    if host_port.contains('[') || host_port.contains(']') {
        return false;
    }
    let host = match host_port.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => host,
        Some(_) => return false, // a colon that isn't a valid port
        None => host_port,
    };

    // Every DNS label must be non-empty. One check covers a leading dot
    // (".battlecrab.com"), a trailing dot ("battlecrab.com.", a distinct FQDN
    // browsers never send) and doubled dots ("..battlecrab.com") — the last of
    // which slipped past an earlier "prefix is longer than one char" version of
    // this function.
    if host.is_empty() || host.split('.').any(|label| label.is_empty()) {
        return false;
    }

    if host == domain {
        return true;
    }

    // Subdomain: must end with ".<domain>". The leading dot is what keeps
    // `evilbattlecrab.com` out — it ends with `battlecrab.com` but not with
    // `.battlecrab.com`.
    host.ends_with(&format!(".{domain}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> OriginPolicy {
        OriginPolicy::parse("battlecrab.com")
    }

    #[test]
    fn allows_the_apex_domain_and_subdomains() {
        let p = policy();
        assert!(p.allows("https://battlecrab.com"));
        assert!(p.allows("https://www.battlecrab.com"));
        assert!(p.allows("https://api.battlecrab.com"));
        assert!(p.allows("https://static.battlecrab.com"));
        assert!(p.allows("https://deep.nested.battlecrab.com"));
    }

    #[test]
    fn rejects_lookalike_domains() {
        let p = policy();
        // The whole point: a plain ends_with() would accept these.
        assert!(!p.allows("https://evilbattlecrab.com"));
        assert!(!p.allows("https://notbattlecrab.com"));
        assert!(!p.allows("https://xbattlecrab.com"));
    }

    #[test]
    fn rejects_the_domain_as_a_prefix_of_someone_elses() {
        let p = policy();
        assert!(!p.allows("https://battlecrab.com.evil.example"));
        assert!(!p.allows("https://battlecrab.company"));
    }

    #[test]
    fn rejects_plain_http_for_the_domain_rule() {
        let p = policy();
        assert!(!p.allows("http://battlecrab.com"));
        assert!(!p.allows("http://www.battlecrab.com"));
    }

    #[test]
    fn rejects_other_schemes_and_junk() {
        let p = policy();
        assert!(!p.allows("null"));
        assert!(!p.allows(""));
        assert!(!p.allows("   "));
        assert!(!p.allows("battlecrab.com"));
        assert!(!p.allows("file://battlecrab.com"));
        assert!(!p.allows("ftp://battlecrab.com"));
        assert!(!p.allows("https://"));
    }

    #[test]
    fn rejects_origins_carrying_more_than_scheme_host_port() {
        let p = policy();
        assert!(!p.allows("https://evil.example/https://battlecrab.com"));
        assert!(!p.allows("https://battlecrab.com/path"));
        assert!(!p.allows("https://battlecrab.com?x=1"));
        assert!(!p.allows("https://battlecrab.com#f"));
        assert!(!p.allows("https://user@battlecrab.com"));
        assert!(!p.allows("https://evil.example@battlecrab.com"));
    }

    #[test]
    fn rejects_empty_labels_and_trailing_dots() {
        let p = policy();
        assert!(!p.allows("https://.battlecrab.com"));
        assert!(!p.allows("https://battlecrab.com."));
        assert!(!p.allows("https://..battlecrab.com"));
    }

    #[test]
    fn allows_an_explicit_port_on_the_domain() {
        let p = policy();
        assert!(p.allows("https://battlecrab.com:8443"));
        assert!(p.allows("https://api.battlecrab.com:8443"));
        // A colon that is not a port is malformed.
        assert!(!p.allows("https://battlecrab.com:notaport"));
        assert!(!p.allows("https://battlecrab.com:"));
    }

    #[test]
    fn rejects_ipv6_literals() {
        let p = policy();
        assert!(!p.allows("https://[::1]"));
        assert!(!p.allows("https://[::1]:8443"));
    }

    #[test]
    fn matching_is_case_insensitive() {
        let p = policy();
        assert!(p.allows("HTTPS://API.BattleCrab.COM"));
    }

    #[test]
    fn exact_entries_match_only_themselves() {
        let p = OriginPolicy::parse("http://localhost:3000");
        assert!(p.allows("http://localhost:3000"));
        assert!(!p.allows("http://localhost:3001"));
        assert!(!p.allows("https://localhost:3000"));
        assert!(!p.allows("http://evil.example"));
    }

    #[test]
    fn parses_mixed_entries_and_wildcard_syntax() {
        let p = OriginPolicy::parse(" *.battlecrab.com , http://localhost:3000/ ,, ");
        assert_eq!(
            p.rules(),
            &[
                OriginRule::Domain("battlecrab.com".into()),
                OriginRule::Exact("http://localhost:3000".into()),
            ]
        );
        assert!(p.allows("https://api.battlecrab.com"));
        assert!(p.allows("http://localhost:3000"));
        assert!(!p.allows("https://evil.example"));
    }

    #[test]
    fn an_empty_policy_allows_nothing() {
        let p = OriginPolicy::parse("");
        assert!(p.is_empty());
        assert!(!p.allows("https://battlecrab.com"));
    }
}
