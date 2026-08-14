//! Cloudflare Turnstile verification (the captcha on register, forgot-password
//! and throttled logins).
//!
//! The widget runs in the SPA; all this module does is present the token it
//! produced to Cloudflare's `siteverify` endpoint and turn the answer into a
//! pass/fail. Tokens are single-use on Cloudflare's side (`timeout-or-duplicate`),
//! so a verified token cannot be replayed.
//!
//! **Disabled by default.** With no secret configured every check passes, which
//! is what makes local development and the test suite work without touching the
//! network. Production is expected to configure it; `main` warns at boot when
//! it is not — the same contract as `mail::Mailer`.

use std::time::Duration;

use crate::config::DashboardConfig;

pub const SITEVERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

/// How long a siteverify round-trip may take before we fail the request.
/// Generous for a same-continent hop to Cloudflare, short enough that a
/// Cloudflare outage degrades into fast 403s instead of hung logins.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub enum CaptchaError {
    #[error("captcha token missing")]
    Missing,
    #[error("captcha verification failed")]
    Rejected,
    /// siteverify unreachable, non-2xx, or unparseable. Failing closed here is
    /// deliberate: an attacker must never gain anything by breaking our path
    /// to Cloudflare.
    #[error("captcha verification unavailable")]
    Unavailable,
}

#[derive(Clone)]
pub struct TurnstileVerifier {
    /// `None` when `$DASHBOARD_TURNSTILE_SECRET` is unset — every check passes.
    inner: Option<Inner>,
}

#[derive(Clone)]
struct Inner {
    secret: String,
    /// `SITEVERIFY_URL` in production; tests point it at a local fake.
    url: String,
    client: reqwest::Client,
}

/// The wire shape of a siteverify answer. Only what classification needs —
/// Cloudflare also returns `challenge_ts`/`hostname`, which we ignore.
#[derive(Debug, serde::Deserialize)]
struct SiteverifyResponse {
    success: bool,
    #[serde(rename = "error-codes", default)]
    error_codes: Vec<String>,
}

impl TurnstileVerifier {
    /// Builds a verifier, or a pass-everything no-op when no secret is set.
    pub fn from_config(config: &DashboardConfig) -> Self {
        if config.turnstile_secret.is_empty() {
            return Self::disabled();
        }
        Self::new(&config.turnstile_secret, SITEVERIFY_URL)
    }

    fn new(secret: &str, url: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(VERIFY_TIMEOUT)
            .build()
            .expect("reqwest client with static config must build");
        Self {
            inner: Some(Inner {
                secret: secret.to_string(),
                url: url.to_string(),
                client,
            }),
        }
    }

    fn disabled() -> Self {
        Self { inner: None }
    }

    /// A verifier pointed at a fake siteverify server. Test-only by contract;
    /// lives here because integration tests cannot reach private constructors.
    #[doc(hidden)]
    pub fn for_tests(secret: &str, url: &str) -> Self {
        Self::new(secret, url)
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    /// Ok(()) when the verifier is disabled or the token verifies.
    /// `remote_ip` is forwarded as `remoteip` so Cloudflare can cross-check
    /// the token against the visitor it was issued to.
    pub async fn verify(
        &self,
        token: Option<&str>,
        remote_ip: Option<&str>,
    ) -> Result<(), CaptchaError> {
        let Some(inner) = &self.inner else {
            return Ok(());
        };

        let token = match token {
            Some(t) if !t.trim().is_empty() => t,
            _ => return Err(CaptchaError::Missing),
        };

        let mut form = vec![("secret", inner.secret.as_str()), ("response", token)];
        if let Some(ip) = remote_ip {
            form.push(("remoteip", ip));
        }

        let response = inner
            .client
            .post(&inner.url)
            .form(&form)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("siteverify request failed: {e}");
                CaptchaError::Unavailable
            })?;

        if !response.status().is_success() {
            tracing::error!("siteverify answered HTTP {}", response.status());
            return Err(CaptchaError::Unavailable);
        }

        let body: SiteverifyResponse = response.json().await.map_err(|e| {
            tracing::error!("siteverify answer was not the expected JSON: {e}");
            CaptchaError::Unavailable
        })?;

        evaluate(body)
    }
}

/// Classifies a parsed siteverify answer. Pure so the mapping is testable
/// without a server.
fn evaluate(resp: SiteverifyResponse) -> Result<(), CaptchaError> {
    if resp.success {
        return Ok(());
    }

    // `invalid-input-response` / `timeout-or-duplicate` are the user's problem
    // (stale or mangled token — the SPA mints a fresh one and retries).
    // A secret-side code means *our* configuration is broken, and every
    // request is about to fail: that has to be loud.
    let ours = resp.error_codes.iter().any(|c| {
        matches!(
            c.as_str(),
            "invalid-input-secret" | "missing-input-secret" | "internal-error"
        )
    });
    if ours {
        tracing::error!(
            "siteverify rejected our request (server-side misconfiguration): {:?}",
            resp.error_codes
        );
    } else {
        tracing::debug!("siteverify rejected a token: {:?}", resp.error_codes);
    }
    Err(CaptchaError::Rejected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(success: bool, codes: &[&str]) -> SiteverifyResponse {
        SiteverifyResponse {
            success,
            error_codes: codes.iter().map(|c| c.to_string()).collect(),
        }
    }

    #[test]
    fn a_successful_answer_passes() {
        assert!(evaluate(resp(true, &[])).is_ok());
    }

    #[test]
    fn a_rejected_token_fails_regardless_of_which_side_erred() {
        assert!(matches!(
            evaluate(resp(false, &["invalid-input-response"])),
            Err(CaptchaError::Rejected)
        ));
        assert!(matches!(
            evaluate(resp(false, &["invalid-input-secret"])),
            Err(CaptchaError::Rejected)
        ));
        // No codes at all still fails: success:false is the verdict.
        assert!(matches!(
            evaluate(resp(false, &[])),
            Err(CaptchaError::Rejected)
        ));
    }

    #[test]
    fn parses_the_real_wire_shape() {
        // Taken from Cloudflare's documented response, extra fields included —
        // they must be ignored, and `error-codes` must map through the rename.
        let json = r#"{
            "success": false,
            "challenge_ts": "2022-02-28T15:14:30.096Z",
            "hostname": "example.com",
            "error-codes": ["timeout-or-duplicate"],
            "action": "login",
            "cdata": ""
        }"#;
        let parsed: SiteverifyResponse = serde_json::from_str(json).unwrap();
        assert!(!parsed.success);
        assert_eq!(parsed.error_codes, vec!["timeout-or-duplicate"]);
    }

    #[test]
    fn error_codes_may_be_absent() {
        let parsed: SiteverifyResponse = serde_json::from_str(r#"{"success": true}"#).unwrap();
        assert!(parsed.success);
        assert!(parsed.error_codes.is_empty());
    }

    #[tokio::test]
    async fn a_disabled_verifier_passes_with_and_without_a_token() {
        let v = TurnstileVerifier::disabled();
        assert!(!v.is_enabled());
        assert!(v.verify(None, None).await.is_ok());
        assert!(v.verify(Some("anything"), Some("1.2.3.4")).await.is_ok());
    }

    #[tokio::test]
    async fn an_enabled_verifier_rejects_a_missing_or_blank_token_without_network() {
        // Points at a URL that must never be contacted for the Missing path.
        let v = TurnstileVerifier::for_tests("secret", "http://127.0.0.1:9/siteverify");
        assert!(v.is_enabled());
        assert!(matches!(
            v.verify(None, None).await,
            Err(CaptchaError::Missing)
        ));
        assert!(matches!(
            v.verify(Some("   "), None).await,
            Err(CaptchaError::Missing)
        ));
    }

    #[tokio::test]
    async fn an_unreachable_siteverify_fails_closed() {
        // Port 9 (discard) refuses the connection — the request must come back
        // Unavailable, not hang and not pass.
        let v = TurnstileVerifier::for_tests("secret", "http://127.0.0.1:9/siteverify");
        assert!(matches!(
            v.verify(Some("token"), None).await,
            Err(CaptchaError::Unavailable)
        ));
    }
}
