//! Every shipped `.ini` must satisfy every key the server reads from it.
//!
//! `PropertiesParser` is **fail-soft**: a key the code asks for and the file
//! does not carry falls back to the *code* default and logs a warning. That is
//! the right runtime behaviour and a terrible failure mode to rely on — the
//! default is often the opposite of what an operator intends
//! (`AutoCreateAccounts` defaults to `true`), and the warning scrolls past at
//! boot among hundreds of other lines.
//!
//! Two such warnings had been printing at every boot for some time before
//! anyone read them:
//!
//! * `StrictDelevelSkillRemoval` — a port-invented deviation knob, defaulting
//!   to the *non*-retail branch, that was never written into `Character.ini`.
//! * `RandomOfBaiumSpawn` — the opposite case, a key correctly absent (Java
//!   reads no such key either) that the generic per-boss lookup asked for
//!   anyway. Fixed by `PropertiesParser::get_int_opt` rather than by inventing
//!   a key, so the warning keeps its meaning everywhere else.
//!
//! This test turns that class of drift from a log line into a build failure:
//! add a `get_*` call for a key the dist does not ship, and it fails here.
//!
//! **Not covered on purpose:** `HexId`, `IpConfig` and the login-link settings
//! are per-environment and legitimately absent from a fresh checkout.

use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

/// A minimal `Subscriber` that keeps WARN-and-worse messages.
///
/// Hand-rolled because `tracing-subscriber` is not a dependency of this crate
/// and pulling one in for a single assertion is a poor trade. Only the message
/// field is captured; spans are stubbed, since nothing here opens one.
#[derive(Default, Clone)]
struct WarnSink(Arc<Mutex<Vec<String>>>);

struct MessageOnly<'a>(&'a mut String);

impl Visit for MessageOnly<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            *self.0 = format!("{value:?}");
        }
    }
}

impl Subscriber for WarnSink {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::Id {
        tracing::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::Id, _: &tracing::Id) {}
    fn enter(&self, _: &tracing::Id) {}
    fn exit(&self, _: &tracing::Id) {}
    fn event(&self, event: &Event<'_>) {
        // `Level` orders inverted: TRACE is "greater" than WARN.
        if *event.metadata().level() > Level::WARN {
            return;
        }
        let mut message = String::new();
        event.record(&mut MessageOnly(&mut message));
        self.0.lock().unwrap().push(message);
    }
}

#[test]
fn the_shipped_configs_load_without_a_single_warning() {
    let sink = WarnSink::default();
    let captured = sink.0.clone();

    tracing::subscriber::with_default(sink, || {
        use gameserver::config::*;

        let _ = GeneralConfig::load_from(DIST);
        let _ = CharacterConfig::load_from(DIST);
        let _ = GrandBossConfig::load_from(DIST);
        let _ = ChatFilterConfig::load_from(DIST);
        let _ = AutoPlayConfig::load_from(DIST);
        let _ = AutoPotionsConfig::load_from(DIST);
        let _ = BotReportConfig::load_from(DIST);
        let _ = ChampionConfig::load_from(DIST);
        let _ = CommunityBoardConfig::load_from(DIST);
        let _ = DualboxConfig::load_from(DIST);
        let _ = FeatureConfig::load_from(DIST);
        let _ = FloodProtectorsConfig::load_from(DIST);
        let _ = GeoEngineConfig::load_from(DIST);
        let _ = AllowedRacesConfig::load_from(DIST);
        let _ = BankingConfig::load_from(DIST);
        let _ = BossAnnouncementsConfig::load_from(DIST);
        let _ = CustomMailConfig::load_from(DIST);
        let _ = CustomMiscConfig::load_from(DIST);
        let _ = CustomNpcConfig::load_from(DIST);
        let _ = PvpRewardConfig::load_from(DIST);
        let _ = PvpTitleColorConfig::load_from(DIST);
        let _ = RandomSpawnsConfig::load_from(DIST);
        let _ = NpcConfig::load_from(DIST);
        let _ = OfflineTradeConfig::load_from(DIST);
        let _ = PremiumConfig::load_from(DIST);
        let _ = SellBuffsConfig::load_from(DIST);
        let _ = RatesConfig::load_from(DIST);
        let _ = SecurityConfig::load_from(DIST);
        let _ = ServerConfig::load_from(DIST);
    });

    let warnings = captured.lock().unwrap().clone();
    assert!(
        warnings.is_empty(),
        "{} config warning(s) loading the shipped dist — either the .ini is \
         missing a key the code reads, or the code should stop reading it:\n  {}",
        warnings.len(),
        warnings.join("\n  ")
    );
}
