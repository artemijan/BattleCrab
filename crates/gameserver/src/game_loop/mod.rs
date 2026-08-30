//! The game thread and its 100 ms tick loop (CONCURRENCY_MODEL §2.2).
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
pub(crate) mod admin;
pub(crate) mod auto_play;
pub(crate) mod auto_potions;
pub(crate) mod auto_use;
mod basic_property;
/// Bench-only wrappers over the private tick systems (`benches/tick.rs`).
#[cfg(feature = "bench-api")]
pub mod bench_api;
pub(crate) mod boats;
// Boss submodules keep their historical `game_loop::<boss>` paths; callers
// (scripts, death, net, scheduler dispatch) address them through this re-export.
pub(crate) use npc::bosses::{
    antharas, baium, boss_respawn, common, core_boss, dr_chaos, frintezza, grand_boss, orfen,
    queen_ant, raid_curse, sailren, valakas,
};
pub(crate) mod birthday;
mod boot;
pub(crate) mod bot_report;
mod bypass;
pub(crate) mod castle;
mod chat;
pub(crate) mod clans;
pub(crate) mod combat;
pub(crate) mod command_channel;
mod community_board;
mod crafting;
mod cubic;
pub(crate) mod cursed_weapon;
pub(crate) mod custom_mail;
mod daily_tasks;
pub(crate) mod death;
mod dispatch;
mod effect_point;
pub(crate) mod effect_zones;
pub(crate) mod events;
pub(crate) mod falling;
pub(crate) mod fishing;
pub(crate) mod flood;
pub(crate) mod four_sepulchers;
mod friends;
pub(crate) mod game_time;
pub(crate) mod global_vars;
pub(crate) mod helpers;
mod henna;
pub(crate) mod instances;
pub(crate) mod items;
mod lobby;
pub(crate) mod lottery;
pub(crate) mod mail;
pub(crate) mod manor;
pub(crate) mod monster_race;
pub(crate) mod multisell;
mod net;
// The boot-time metric registration is the one thing `main` needs out of `net`;
// re-exported rather than opening the whole module up.
pub use net::register_metrics;
pub(crate) mod night_stats;
pub mod npc;
pub(crate) mod observation;
pub(crate) mod offline_trade;
pub(crate) mod olympiad;
mod options;
mod party;
mod party_room;
mod passive_skills;
pub(crate) mod pc_cafe;
pub(crate) mod pet_evolve;
pub(crate) mod petition;
pub(crate) mod player_actions;
pub(crate) mod player_info;
pub(crate) mod position;
mod private_store;
pub(crate) mod punishment;
pub mod quests;
mod ranged;
mod reco;
pub(crate) mod regen;
pub(crate) mod restart;
pub(crate) mod sell_buffs;
pub(crate) mod servitor;
mod settings;
pub(crate) mod shop;
mod shortcuts;
pub(crate) mod siege;
mod sit_stand;
pub(crate) mod skills;
pub(crate) mod spawn_protection;
pub(crate) mod spawn_scripts;
pub(crate) mod stat_ctx;
pub(crate) mod subclass;
pub(crate) mod support_magic;
pub(crate) mod tamed_beast;
mod target;
mod tasks;
#[cfg(test)]
mod tests;
pub(crate) mod time;
mod trade;
mod user_commands;
mod visibility;
mod vitality;
mod warehouse;
pub(crate) mod water;
pub(crate) mod weight;
pub(crate) mod zones;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tracing::{info, warn};

use crate::data::GameData;
use crate::db;
use crate::events::GameEventRx;
use crate::loginlink::CommandTx;
use crate::world::World;

use crate::game_loop::combat::pvp;
use crate::game_loop::items::ground_items;
use net::handle_game_event;
use npc::{ai, walkers};
use regen::{REGEN_TICK_PERIOD, run_npc_regen_tick, run_regen_tick};

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
/// design, so it must be visible from day one (CONCURRENCY_MODEL §2.6 rule 4).
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
        let event_work = pump_events_until(&mut world, &events_rx, deadline);
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
        timed!("movement", visibility::movement_tick(&mut world));
        // Player attack intents (chase + swing) every tick, like Java's
        // event-driven PlayerAI reacting as soon as it's ready to act.
        timed!("player_combat", combat::player_combat_tick(&mut world));
        if world.tick.is_multiple_of(effect_zones::SWEEP_PERIOD) {
            timed!("effect_zones", {
                effect_zones::effect_zone_tick(&mut world);
                effect_zones::damage_zone_tick(&mut world);
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
            timed!("weight", weight::sweep(&mut world));
        }
        if world.tick.is_multiple_of(auto_play::TICK_PERIOD) {
            timed!("auto_play", {
                auto_play::tick(&mut world);
                auto_use::tick(&mut world);
            });
        }
        if world.tick.is_multiple_of(auto_potions::TICK_PERIOD) {
            timed!("auto_potions", auto_potions::tick(&mut world));
        }
        if world
            .tick
            .is_multiple_of(custom_mail::poll_period_ticks(&world))
        {
            timed!("custom_mail", custom_mail::poll(&mut world));
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
        timed!("drowning", water::drown_tick(&mut world));
        // `_fallingDamageTask`'s 1.5 s one-shot (Java schedules a future per
        // falling player and cancels it on every further report; the port
        // sweeps the component instead). Every tick: each player's clock
        // starts when *they* stopped falling.
        timed!("falling", falling::falling_damage_tick(&mut world));
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
        timed!("item_audit", items::drain_item_audit(&mut world));
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
                "GameLoop: tick {} ran {} ms (budget {} ms; slowest: {slowest}).",
                world.tick,
                busy.as_millis(),
                TICK.as_millis()
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
/// Returns the time spent handling events (waiting excluded), for the
/// tick-overrun metric.
fn pump_events_until(world: &mut World, events_rx: &GameEventRx, deadline: Instant) -> Duration {
    let mut busy = Duration::ZERO;
    loop {
        // Everything already queued, without blocking.
        while let Ok(event) = events_rx.try_recv() {
            let start = Instant::now();
            handle_game_event(world, event);
            busy += start.elapsed();
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return busy;
        };
        if remaining.is_zero() {
            return busy;
        }
        match events_rx.recv_timeout(remaining) {
            Ok(event) => {
                let start = Instant::now();
                handle_game_event(world, event);
                busy += start.elapsed();
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => return busy,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // Every service sender is gone — teardown (or a test driving
                // the loop by hand). Keep the tick cadence instead of
                // busy-spinning on an empty, closed channel.
                std::thread::sleep(remaining);
                return busy;
            }
        }
    }
}
