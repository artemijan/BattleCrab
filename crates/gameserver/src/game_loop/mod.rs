//! The game thread and its 100 ms tick loop (THREADING_MODEL §2).
//!
//! Runs on one dedicated OS thread that owns [`World`]. The base tick is 100 ms,
//! matching Java's `GameTimeTaskManager` and high-priority task-manager rate.
//! Each tick: handle service events (network, login-link, DB, path) **as they
//! arrive** while sleeping on the unified channel, then at the tick boundary
//! fire due timers and run the fixed-rate systems (G4+). Packet dispatch and
//! login handoff land here on the game thread, keeping handler code sequential
//! and 1:1 with Java `run()`.

// Lives under skills/ but keeps its historical game_loop::abnormal path.
pub(crate) use skills::abnormal;
pub(crate) mod activities;
pub(crate) mod admin;
pub(crate) mod automation;
/// Bench-only wrappers over the private tick systems (`benches/tick.rs`).
#[cfg(feature = "bench-api")]
pub mod bench_api;
// Boss submodules keep their historical `game_loop::<boss>` paths; callers
// (scripts, death, net, scheduler dispatch) address them through this re-export.
pub(crate) use npc::bosses::{
    antharas, baium, boss_respawn, common, core_boss, dr_chaos, frintezza, grand_boss, orfen,
    queen_ant, raid_curse, sailren, valakas,
};
mod boot;
pub(crate) mod character;
pub(crate) mod clans;
pub(crate) mod client;
pub(crate) mod combat;
pub(crate) mod commerce;
mod community_board;
pub(crate) mod events;
pub(crate) mod helpers;
pub(crate) mod items;
pub(crate) mod mail;
pub(crate) mod manor;
pub(crate) mod moderation;
pub(crate) mod net;
// The boot-time metric registration is the one thing `main` needs out of `net`;
// re-exported rather than opening the whole module up.
pub use net::register_metrics;
pub mod npc;
pub(crate) mod olympiad;
mod party;
pub mod quests;
pub(crate) mod servitor;
pub(crate) mod siege;
pub(crate) mod skills;
pub(crate) mod social;
pub(crate) mod space;
pub(crate) mod stats;
mod tasks;
#[cfg(test)]
mod tests;
pub(crate) mod time;
pub(crate) mod upkeep;

use crate::game_loop::character::inventory;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::data::GameData;
use crate::db;
use crate::events::{GameEvent, GameEventRx};
use crate::loginlink::CommandTx;
use crate::network::NetEvent;
use crate::world::World;
use combat::death;
use tracing::{info, warn};

use crate::game_loop::combat::pvp;
use crate::game_loop::items::ground_items;
use net::handle_game_event;
use npc::{ai, walkers};
use stats::regen::{REGEN_TICK_PERIOD, run_npc_regen_tick, run_regen_tick};

/// Base tick period. Slower Java rates (1 s, 5 s…) become `world.tick % N == 0`
/// systems on top of this.
pub const TICK: Duration = Duration::from_millis(100);

/// `Config.SAVE_DROPPED_ITEM_INTERVAL` in ticks, or `None` when the key is
/// `<= 0` — Java skips scheduling the task entirely in that case rather than
/// treating it as "every tick".
fn ground_item_store_period(world: &World) -> Option<u64> {
    let minutes = world.cfg.general.save_dropped_item_interval_minutes;
    (minutes > 0).then(|| minutes as u64 * 60 * 10)
}

/// A tick that runs longer than this is the failure mode of the single-thread
/// design, so it must be visible from day one (THREADING_MODEL §4 rule 4).
const TICK_OVERRUN_WARN: Duration = Duration::from_millis(50);

/// How often the staggered autosave sweep runs — every 1 s (10 ticks), the same
/// fixed-rate cadence as Java's `PlayerAutoSaveTaskManager`.
const AUTOSAVE_CHECK_PERIOD: u64 = 10;

/// The last tick's busy time in microseconds. Headroom is this against the
/// 100 000 µs budget — it turns "how close is the single-threaded design to
/// its ceiling" from a guess into a graphable series.
pub(crate) fn tick_busy_micros() -> &'static commons::metrics::Gauge {
    static G: std::sync::OnceLock<commons::metrics::Gauge> = std::sync::OnceLock::new();
    G.get_or_init(|| commons::metrics::gauge("tick_busy_micros"))
}

/// Signal shared with the async side (ctrl-c / scheduled restart) to stop the
/// loop after the current tick finishes.
#[derive(Clone, Default)]
pub struct Shutdown(Arc<AtomicBool>);

impl Shutdown {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_requested(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Everything the game thread needs to start.
pub struct GameThreadChannels {
    /// The unified service→game channel (`crate::events`): network, login-link,
    /// DB and path events all arrive here, and the loop sleeps on it.
    pub events_rx: GameEventRx,
    pub link_tx: CommandTx,
    /// Released once all boot data (incl. clans) is loaded, letting the
    /// login-link task begin connecting to the login server.
    pub login_ready_tx: tokio::sync::oneshot::Sender<()>,
    pub db_tx: db::CmdTx,
    pub data: GameData,
    pub geo: Arc<crate::geo::GeoEngine>,
    pub path_tx: crate::geo::worker::PathReqTx,
    pub path_finding: i32,
    /// `GeoEngine.ini`'s pathfinding tuning + geo-editor output dir, for the
    /// two admin commands that use them on the game thread (`//path_find`,
    /// `//geosave*`).
    pub path_cfg: crate::geo::path::PathConfig,
    pub geoedit_path: String,
    pub max_characters_per_account: i32,
    pub delete_days: i32,
    pub starting_adena: i64,
    pub cfg: crate::config::CombatConfig,
}

/// Spawn the game thread. Returns its join handle so `main` can wait for the
/// final tick (drain + save) before exiting.
pub fn spawn(shutdown: Shutdown, ch: GameThreadChannels) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("game-thread".to_string())
        .spawn(move || run(shutdown, ch))
        .expect("failed to spawn game thread")
}

fn run(shutdown: Shutdown, ch: GameThreadChannels) {
    let GameThreadChannels {
        events_rx,
        link_tx,
        login_ready_tx,
        db_tx,
        data,
        geo,
        path_tx,
        path_finding,
        path_cfg,
        geoedit_path,
        max_characters_per_account,
        delete_days,
        starting_adena,
        cfg,
    } = ch;
    let mut world = World::new(
        link_tx,
        max_characters_per_account,
        delete_days,
        starting_adena,
        data,
        db_tx,
    );
    world.geo = geo;
    world.shutdown_signal = Some(shutdown.clone());
    world.path = path_tx;
    world.path_finding = path_finding;
    world.path_cfg = path_cfg;
    world.geoedit_path = geoedit_path;
    world.cfg = cfg;
    // Java seeds `LoginServerThread._maxPlayer` from `MaximumOnlineUsers` when
    // the thread is built; `//server_login`'s page prints it back, and
    // `//server_max_player` overwrites it.
    world.login.max_players = world.cfg.server.maximum_online_users;
    // `Config.ALT_DEV_NO_QUESTS` — Java returns from
    // `ScriptEngineManager.executeScriptList()` before loading anything, so
    // despite the name it drops **every** script (AI and events included), not
    // only quests. The port's registry holds the same set, so emptying it is
    // the same switch.
    if world.cfg.general.alt_dev_no_quests {
        world.quests = std::sync::Arc::new(quests::QuestRegistry::new(Vec::new()));
        info!("ScriptEngine: AltDevNoQuests is set — no scripts registered.");
    } else if world.cfg.general.alt_dev_show_quests_load_in_logs
        || world.cfg.general.alt_dev_show_scripts_load_in_logs
    {
        // Java logs one line per registration, and the two keys are **not**
        // synonyms: `Quest(int questId)` calls `addQuest` when the id is
        // positive and `addScript` otherwise, and each has its own key and its
        // own wording. The port registers everything in one pass rather than
        // one call each, so the lines are emitted here — same split, same
        // wording, one place.
        for name in world.quests.names() {
            let is_quest = world.quests.quest_id(name).is_some_and(|id| id > 0);
            if is_quest {
                if world.cfg.general.alt_dev_show_quests_load_in_logs {
                    info!("Loaded quest {name}.");
                }
            } else if world.cfg.general.alt_dev_show_scripts_load_in_logs {
                info!("Loaded script {name}.");
            }
        }
    }
    // Held until `DbEvent::ClansLoaded` arrives; then the login-link task is
    // released to connect (Java: `LoginServerThread.start()` after `ClanTable`).
    world.login.ready = Some(login_ready_tx);

    boot::boot(&mut world);

    info!("GameLoop: started ({} ms tick).", TICK.as_millis());

    // The boundary the current tick's event phase runs to. Starting at "now"
    // makes tick 0 drain whatever the services queued during boot and run its
    // boundary work immediately (parity with the old drain-first order).
    let mut deadline = Instant::now();
    // Per-step timings for the current tick, reused across ticks. Filled
    // every tick (two clock reads per step is noise against a 100 ms budget)
    // so an overrun warning can name its culprit instead of just its size.
    let mut timings: Vec<(&'static str, Duration)> = Vec::with_capacity(24);
    // Times one step into `timings`. A macro rather than a closure because
    // each step body needs its own `&mut world`.
    macro_rules! timed {
        ($name:literal, $body:expr) => {{
            let start = Instant::now();
            $body;
            timings.push(($name, start.elapsed()));
        }};
    }

    while !shutdown.is_requested() {
        timings.clear();

        // 1. Events: connects, disconnects, inbound packets, and login-link /
        //    DB / path results — handled the moment they arrive. This *is* the
        //    tick sleep: between events the thread blocks on the channel until
        //    the deadline, so a packet no longer waits out the remainder of
        //    the 100 ms (the added-latency cost THREADING_MODEL §5 used to
        //    carry).
        let events = pump_events_until(&mut world, &events_rx, deadline);
        let event_work = events.busy;
        timings.push(("events", event_work));

        // The tick boundary: timers + fixed-rate systems.
        let boundary_start = Instant::now();

        // 2. One-shot timers due this tick.
        timed!("timers", tasks::apply_due_tasks(&mut world));

        // 3. Fixed-rate tick systems (movement, AI, attack…) — added in G4+.
        // Movement runs every tick (unlike the gated systems below) — it
        // needs to recompute the authoritative server-side position each
        // 100 ms, same as Java's `MovementTaskManager`. Region-switch
        // visibility events (CharInfo/DeleteObject) ride along.
        timed!("movement", space::visibility::movement_tick(&mut world));
        // Player attack intents (chase + swing) every tick, like Java's
        // event-driven PlayerAI reacting as soon as it's ready to act.
        timed!("player_combat", combat::player_combat_tick(&mut world));
        if world.tick.is_multiple_of(space::effect_zones::SWEEP_PERIOD) {
            timed!("effect_zones", {
                space::effect_zones::effect_zone_tick(&mut world);
                space::effect_zones::damage_zone_tick(&mut world);
            });
        }
        if world.tick.is_multiple_of(walkers::WALKER_PERIOD) {
            timed!("walkers", walkers::walker_tick(&mut world));
        }
        if world.tick.is_multiple_of(ai::NPC_THINK_PERIOD) {
            // AttackableAI think (1 s) + the combat-stance sweep (15 s
            // timeouts, checked at the same 1 s cadence as Java).
            timed!("npc_ai", ai::npc_ai_tick(&mut world));
            timed!("stance", combat::stance_tick(&mut world));
            timed!("pvp_flags", pvp::pvp_flag_tick(&mut world));
        }
        if world.tick.is_multiple_of(REGEN_TICK_PERIOD) {
            timed!("regen", {
                run_regen_tick(&mut world);
                run_npc_regen_tick(&mut world);
            });
            timed!("weight", stats::weight::sweep(&mut world));
        }
        if world.tick.is_multiple_of(automation::play::TICK_PERIOD) {
            timed!("auto_play", {
                automation::play::tick(&mut world);
                automation::use_items::tick(&mut world);
            });
        }
        if world.tick.is_multiple_of(automation::potions::TICK_PERIOD) {
            timed!("auto_potions", automation::potions::tick(&mut world));
        }
        if world
            .tick
            .is_multiple_of(mail::custom::poll_period_ticks(&world))
        {
            timed!("custom_mail", mail::custom::poll(&mut world));
        }
        if world.tick.is_multiple_of(AUTOSAVE_CHECK_PERIOD) {
            timed!("autosave", net::autosave_tick(&mut world));
        }
        if world.tick.is_multiple_of(death::TELEPORT_WATCHDOG_PERIOD) {
            timed!(
                "teleport_watchdog",
                death::teleport_watchdog_tick(&mut world)
            );
        }
        // `WaterTask`'s 1 s fixed-rate beat (Java schedules one future per
        // drowning player; the port sweeps the component instead). Every tick,
        // because each player's clock starts when *they* went under.
        timed!("drowning", space::water::drown_tick(&mut world));
        // `_fallingDamageTask`'s 1.5 s one-shot (Java schedules a future per
        // falling player and cancels it on every further report; the port
        // sweeps the component instead). Every tick: each player's clock
        // starts when *they* stopped falling.
        timed!("falling", space::falling::falling_damage_tick(&mut world));
        // `ItemsOnGroundManager`'s `scheduleAtFixedRate(this, interval, interval)`
        // — the periodic rewrite of `itemsonground`. Off entirely while
        // `SaveDroppedItem` is off, which is why the period is read here rather
        // than armed at boot.
        if world.cfg.general.save_dropped_item
            && let Some(period) = ground_item_store_period(&world)
            && world.tick.is_multiple_of(period)
        {
            timed!("ground_item_store", ground_items::store_all(&mut world));
        }
        // Item losses noted by the inventory removal methods become audit
        // records here, where the config gate and the owning player exist.
        // Every tick: a record that waits is a record that a crash loses.
        timed!("item_audit", inventory::drain_item_audit(&mut world));
        // 4. Flush outbound packets / DB commands — added in G3+.

        // The tick's *busy* time: event handling (waiting excluded) plus the
        // boundary work above. Overrun is the failure mode of the
        // single-thread design, so it must stay visible (rule 4) — and
        // attributable: the warning names the slowest steps, and the gauge
        // makes headroom (busy µs against the 100 000 µs budget) graphable.
        let busy = event_work + boundary_start.elapsed();
        tick_busy_micros().set(busy.as_micros() as u64);
        if busy > TICK_OVERRUN_WARN {
            timings.sort_by_key(|b| std::cmp::Reverse(b.1));
            let slowest = timings
                .iter()
                .take(3)
                .filter(|(_, d)| !d.is_zero())
                .map(|(name, d)| format!("{name} {:.1} ms", d.as_secs_f64() * 1000.0))
                .collect::<Vec<_>>()
                .join(", ");
            warn!(
                "GameLoop: tick {} ran {} ms (budget {} ms; slowest: {slowest}){}.",
                world.tick,
                busy.as_millis(),
                TICK.as_millis(),
                events.detail(),
            );
        }
        // Next boundary: one TICK after the previous one, but never in the
        // past — an overrun tick slides the phase (the old sleep-skipping
        // behaviour) rather than running back-to-back catch-up ticks.
        deadline = std::cmp::max(deadline + TICK, Instant::now());

        world.tick += 1;
    }

    info!("GameLoop: stopped after {} ticks.", world.tick);
    boot::shutdown_flush(&mut world);
}

/// What the event phase actually spent its time on.
///
/// The tick's overrun warning names its slowest *step*, which for every
/// boundary system is enough to act on: `regen 0.6 ms` is a system with a known
/// body. `events 376.4 ms` is not — the event phase is one step covering every
/// inbound packet plus every DB, login-link and pathfinding result that arrived
/// in the tick, so naming it says only "the work came from outside", which was
/// already known.
///
/// This is the missing half: per-kind totals, and the single slowest individual
/// event with the opcode that carried it. That distinguishes the two shapes an
/// overrun takes — one pathological handler, or a legitimate flood of cheap
/// ones — which are diagnosed in completely different places.
///
/// Cost: four `Instant` reads and some adds per event, on a path that already
/// takes two for `busy`. There is no histogram and nothing per-opcode is
/// accumulated, because a fixed-size struct is what makes this affordable
/// enough to leave on always — and an overrun that cannot be reproduced is one
/// that has to be diagnosed from the line that was already logged.
#[derive(Default)]
struct EventProfile {
    /// Handler time only; the channel wait is excluded, as in `busy`.
    busy: Duration,
    /// How many events were handled — the "flood of cheap ones" signal.
    handled: u32,
    /// Per-kind busy time and count, indexed by [`EventLabel::kind_index`].
    kinds: [(Duration, u32); 4],
    /// The slowest single event of the tick, and what it was.
    worst: Duration,
    worst_label: Option<EventLabel>,
}

/// Enough of an event to name it in a log line, captured *before* the handler
/// consumes it. Copy, and never holds the packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventLabel {
    /// A connect/disconnect/protocol-version event — no opcode to name.
    Net(&'static str),
    /// An inbound packet, by opcode. `ex` is the `0xD0` sub-opcode, which is
    /// where the interesting handlers live, so a bare `0xd0` would name the
    /// wrong thing.
    Packet {
        opcode: u8,
        ex: Option<u16>,
    },
    Login,
    Db,
    Path,
}

impl EventLabel {
    /// Read the label off an event without taking anything out of it.
    fn of(event: &GameEvent) -> Self {
        match event {
            GameEvent::Net(NetEvent::Received { data, .. }) => {
                let opcode = data.first().copied().unwrap_or(0);
                let ex = (opcode == crate::network::client_packets::opcodes::EX_PACKET)
                    .then(|| {
                        crate::network::client_packets::session::read_ex_opcode(&data[1..])
                            .map(|(sub, _)| sub)
                    })
                    .flatten();
                EventLabel::Packet { opcode, ex }
            }
            GameEvent::Net(NetEvent::Connected { .. }) => EventLabel::Net("connect"),
            GameEvent::Net(NetEvent::Disconnected { .. }) => EventLabel::Net("disconnect"),
            GameEvent::Net(NetEvent::ProtocolVersion { .. }) => EventLabel::Net("protocol"),
            GameEvent::Login(_) => EventLabel::Login,
            GameEvent::Db(_) => EventLabel::Db,
            GameEvent::Path(_) => EventLabel::Path,
        }
    }

    /// Which [`EventProfile::kinds`] slot this counts against. The four service
    /// channels, matching `handle_game_event`'s own four arms.
    fn kind_index(self) -> usize {
        match self {
            EventLabel::Net(_) | EventLabel::Packet { .. } => 0,
            EventLabel::Login => 1,
            EventLabel::Db => 2,
            EventLabel::Path => 3,
        }
    }
}

impl std::fmt::Display for EventLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventLabel::Net(what) => write!(f, "net/{what}"),
            EventLabel::Packet {
                opcode,
                ex: Some(sub),
            } => write!(f, "packet 0x{opcode:02x}:0x{sub:04x}"),
            EventLabel::Packet { opcode, ex: None } => write!(f, "packet 0x{opcode:02x}"),
            EventLabel::Login => write!(f, "login-link"),
            EventLabel::Db => write!(f, "db"),
            EventLabel::Path => write!(f, "path"),
        }
    }
}

/// The names of [`EventProfile::kinds`]' slots, in index order.
const EVENT_KINDS: [&str; 4] = ["net", "login-link", "db", "path"];

impl EventProfile {
    /// Time one event's handler into the profile.
    fn record(&mut self, label: EventLabel, elapsed: Duration) {
        self.busy += elapsed;
        self.handled += 1;
        let slot = &mut self.kinds[label.kind_index()];
        slot.0 += elapsed;
        slot.1 += 1;
        if elapsed > self.worst {
            self.worst = elapsed;
            self.worst_label = Some(label);
        }
    }

    /// The clause the overrun warning appends: how many events, which single
    /// one was worst, and the per-kind split. Empty when nothing was handled,
    /// so a boundary-only overrun does not carry a misleading `events` clause.
    fn detail(&self) -> String {
        let Some(worst) = self.worst_label else {
            return String::new();
        };
        // Worst-first, like the step list this clause hangs off — the reader is
        // scanning for where the time went, not looking a name up.
        let mut rows: Vec<(&str, Duration, u32)> = EVENT_KINDS
            .iter()
            .zip(&self.kinds)
            .filter(|(_, (d, _))| !d.is_zero())
            .map(|(name, (d, n))| (*name, *d, *n))
            .collect();
        rows.sort_by_key(|(name, d, _)| (std::cmp::Reverse(*d), *name));
        let kinds: Vec<String> = rows
            .iter()
            .map(|(name, d, n)| format!("{name} {n}×{:.1} ms", d.as_secs_f64() * 1000.0))
            .collect();
        format!(
            "; events: {} handled, slowest single {worst} {:.1} ms, by kind: {}",
            self.handled,
            self.worst.as_secs_f64() * 1000.0,
            kinds.join(", "),
        )
    }
}

/// Phase 1 of each tick: handle service events until `deadline`.
///
/// Blocks on the unified channel (`recv_timeout`) between events — this *is*
/// the tick sleep, so an event is handled the moment it arrives instead of
/// waiting out the remainder of the 100 ms. When the deadline has already
/// passed (boot, an overrun tick), everything queued is still drained: the
/// deadline bounds *waiting*, not handling, exactly like the old
/// drain-at-boundary calls. A client flooding faster than we can handle is
/// therefore bounded by the flood protector (dispatch punishes it), not here
/// — also as before.
///
/// Returns the phase's [`EventProfile`] — the time spent handling events
/// (waiting excluded) for the tick-overrun metric, plus the breakdown that
/// makes an overrun attributable.
fn pump_events_until(
    world: &mut World,
    events_rx: &GameEventRx,
    deadline: Instant,
) -> EventProfile {
    let mut profile = EventProfile::default();
    /// Handle one event, timed and labelled into the profile. A macro rather
    /// than a closure because the body needs `&mut world` at both call sites.
    macro_rules! handle {
        ($event:expr) => {{
            let event = $event;
            let label = EventLabel::of(&event);
            let start = Instant::now();
            handle_game_event(world, event);
            profile.record(label, start.elapsed());
        }};
    }
    loop {
        // Everything already queued, without blocking.
        while let Ok(event) = events_rx.try_recv() {
            handle!(event);
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return profile;
        };
        if remaining.is_zero() {
            return profile;
        }
        match events_rx.recv_timeout(remaining) {
            Ok(event) => handle!(event),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => return profile,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // Every service sender is gone — teardown (or a test driving
                // the loop by hand). Keep the tick cadence instead of
                // busy-spinning on an empty, closed channel.
                std::thread::sleep(remaining);
                return profile;
            }
        }
    }
}

#[cfg(test)]
mod event_profile_tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// **The point of the whole type.** The old warning said
    /// `slowest: events 376.4 ms` and stopped there; this asserts the clause
    /// that turns that into something to act on — one pathological handler,
    /// named by opcode, rather than a flood.
    #[test]
    fn the_detail_names_the_single_slowest_packet() {
        let mut p = EventProfile::default();
        p.record(
            EventLabel::Packet {
                opcode: 0x0f,
                ex: None,
            },
            ms(2),
        );
        p.record(
            EventLabel::Packet {
                opcode: 0xd0,
                ex: Some(0x005f),
            },
            ms(370),
        );
        p.record(EventLabel::Db, ms(4));

        let detail = p.detail();
        assert!(detail.contains("3 handled"), "{detail}");
        // The ex sub-opcode, not the bare 0xd0 envelope, is what identifies the
        // handler — naming `0xd0` would point at every extended packet at once.
        assert!(
            detail.contains("slowest single packet 0xd0:0x005f 370.0 ms"),
            "{detail}"
        );
        // …and the per-kind split separates "our own DB callback" from "a
        // packet", worst-first like the step list it hangs off.
        assert!(
            detail.ends_with("by kind: net 2×372.0 ms, db 1×4.0 ms"),
            "{detail}"
        );
        // A kind that contributed nothing is left out rather than logged as 0.
        assert!(!detail.contains("path"), "{detail}");
        assert!(!detail.contains("login-link"), "{detail}");
    }

    /// The other shape an overrun takes: nothing individually slow, just a lot
    /// of it. The count is what distinguishes the two, so it is always there.
    #[test]
    fn a_flood_of_cheap_events_is_visible_as_a_count() {
        let mut p = EventProfile::default();
        for _ in 0..500 {
            p.record(
                EventLabel::Packet {
                    opcode: 0x0f,
                    ex: None,
                },
                ms(1),
            );
        }
        let detail = p.detail();
        assert!(detail.contains("500 handled"), "{detail}");
        assert!(
            detail.contains("slowest single packet 0x0f 1.0 ms"),
            "{detail}"
        );
        assert_eq!(p.busy, ms(500));
    }

    /// A tick that overran on boundary work alone must not carry an `events`
    /// clause claiming otherwise — an empty phase says nothing, so it says
    /// nothing.
    #[test]
    fn an_empty_event_phase_adds_no_clause() {
        assert_eq!(EventProfile::default().detail(), "");
        assert_eq!(EventProfile::default().busy, Duration::ZERO);
    }

    /// Packets and the connect/disconnect events share the `net` slot because
    /// they share a service; the other three channels each get their own.
    #[test]
    fn every_label_counts_against_its_service_channel() {
        assert_eq!(
            EventLabel::Packet {
                opcode: 0,
                ex: None
            }
            .kind_index(),
            EventLabel::Net("connect").kind_index()
        );
        let indices = [
            EventLabel::Net("connect").kind_index(),
            EventLabel::Login.kind_index(),
            EventLabel::Db.kind_index(),
            EventLabel::Path.kind_index(),
        ];
        assert_eq!(indices, [0, 1, 2, 3], "one slot each, in EVENT_KINDS order");
        assert!(indices.iter().all(|i| *i < EVENT_KINDS.len()));
    }
}
