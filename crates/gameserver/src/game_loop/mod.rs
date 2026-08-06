//! The game thread and its 100 ms tick loop (CONCURRENCY_MODEL §2.2).
//!
//! Runs on one dedicated OS thread that owns [`World`]. The base tick is 100 ms,
//! matching Java's `GameTimeTaskManager` and high-priority task-manager rate.
//! Each tick: handle service events (network, login-link, DB, path) **as they
//! arrive** while sleeping on the unified channel, then at the tick boundary
//! fire due timers and run the fixed-rate systems (G4+). Packet dispatch and
//! login handoff land here on the game thread, keeping handler code sequential
//! and 1:1 with Java `run()`.

mod abnormal;
pub(crate) mod academy;
pub(crate) mod admin;
pub(crate) mod antharas;
pub(crate) mod area_npcs;
mod augment;
pub(crate) mod auto_play;
pub(crate) mod auto_potions;
pub(crate) mod auto_use;
pub(crate) mod baium;
mod basic_property;
/// Bench-only wrappers over the private tick systems (`benches/tick.rs`).
#[cfg(feature = "bench-api")]
pub mod bench_api;
pub(crate) mod boats;
mod boss_respawn;
mod boss_threat;
pub(crate) mod bot_report;
mod bypass;
pub(crate) mod castle;
mod chat;
pub(crate) mod clan_hall_auction;
pub(crate) mod clan_hall_function;
pub(crate) mod clans;
mod combat;
pub(crate) mod command_channel;
mod common;
mod community_board;
mod core_boss;
mod crafting;
mod cubic;
pub(crate) mod cursed_weapon;
pub(crate) mod custom_mail;
mod daily_tasks;
pub(crate) mod death;
mod dispatch;
pub(crate) mod doors;
pub(crate) mod dr_chaos;
pub(crate) mod duel;
mod effect_point;
pub(crate) mod effect_zones;
mod enchant;
pub(crate) mod events;
mod expertise;
pub(crate) mod fishing;
pub(crate) mod flood;
pub(crate) mod four_sepulchers;
mod friends;
pub(crate) mod frintezza;
pub(crate) mod game_time;
pub(crate) mod global_vars;
mod grand_boss;
pub(crate) mod ground_items;
pub(crate) mod helpers;
mod henna;
pub(crate) mod instances;
pub(crate) mod item_auction;
pub(crate) mod item_mana;
mod items;
mod lobby;
pub(crate) mod lottery;
pub(crate) mod mail;
pub(crate) mod manor;
pub(crate) mod minions;
pub(crate) mod monster_race;
pub(crate) mod multisell;
mod net;
// The boot-time metric registration is the one thing `main` needs out of `net`;
// re-exported rather than opening the whole module up.
pub use net::register_metrics;
pub(crate) mod night_stats;
pub(crate) mod npc_ai;
pub(crate) mod npc_cast;
mod npc_view;
pub(crate) mod offline_trade;
pub(crate) mod olympiad;
mod options;
mod orfen;
mod party;
mod party_room;
mod passive_skills;
pub(crate) mod pc_cafe;
pub(crate) mod pet_evolve;
pub(crate) mod petition;
pub(crate) mod position;
mod private_store;
pub(crate) mod punishment;
mod pvp;
mod queen_ant;
pub mod quests;
mod raid_curse;
mod ranged;
mod reco;
pub(crate) mod regen;
pub(crate) mod restart;
pub(crate) mod sailren;
pub(crate) mod sell_buffs;
mod servitor;
mod settings;
pub(crate) mod shop;
mod shortcuts;
pub(crate) mod siege;
mod sit_stand;
mod skill_enchant;
pub(crate) mod skills;
pub(crate) mod spawn_scripts;
pub(crate) mod subclass;
pub(crate) mod support_magic;
pub(crate) mod tamed_beast;
mod target;
pub(crate) mod teleporter;
#[cfg(test)]
mod tests;
mod trade;
mod user_commands;
pub(crate) mod valakas;
mod visibility;
mod vitality;
pub(crate) mod walkers;
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
use crate::scheduler::ScheduledTask;
use crate::world::World;

use net::handle_game_event;
use regen::{REGEN_TICK_PERIOD, run_npc_regen_tick, run_regen_tick};
use skills::cast::{handle_cast_end, handle_skill_finish, handle_skill_launch};
use skills::effects::handle_buff_expire;

/// Base tick period. Slower Java rates (1 s, 5 s…) become `world.tick % N == 0`
/// systems on top of this.
pub const TICK: Duration = Duration::from_millis(100);

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
    pub geo: std::sync::Arc<crate::geo::GeoEngine>,
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
    // Held until `DbEvent::ClansLoaded` arrives; then the login-link task is
    // released to connect (Java: `LoginServerThread.start()` after `ClanTable`).
    world.login.ready = Some(login_ready_tx);

    // Java `GameServer`: SpawnData.getInstance().init() — place the static
    // world content before accepting anyone in.
    crate::model::npc::spawn_all(&mut world);
    // DoorData's boot spawn (entities + BY_TIME cycles; the collision grid
    // was registered into the GeoEngine in main.rs, before it was shared).
    crate::model::door::spawn_doors(&mut world);
    doors::start_time_cycles(&mut world);
    crate::model::static_object::spawn_static_objects(&mut world);
    // Java `DailyTaskManager`: the daily 06:30 reset (recommends + vitality
    // refill). Scheduled once here; the task reschedules itself every 24 h.
    daily_tasks::schedule_initial_daily_reset(&mut world);
    bot_report::schedule_initial_points_reset(&mut world);
    restart::schedule_server_restart(&mut world);
    // Grand bosses spawn/respawn once their data lands — the `grandboss_data`
    // table arrives asynchronously as `DbEvent::GrandBossesLoaded`, so
    // `grand_boss::resolve_at_boot` (and `dr_chaos`) run from that handler, not
    // here where `world.grand_bosses` is still empty.
    boats::spawn_boats(&mut world);
    // Script-owned area NPCs (Toma is not in the spawn data — his script
    // places and relocates him).
    area_npcs::spawn_at_boot(&mut world);
    // Each event's `config.xml` cron schedule (Java's per-event `loadConfig`).
    events::schedule_at_boot(&mut world);
    // The Monster Race (like the Lottery) starts from its DB-load event
    // (`DbEvent::MdtLoaded` → `monster_race::on_mdt_loaded`), which seeds the
    // race number from the loaded history before beginning the cycle.

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
        timed!("timers", apply_due_tasks(&mut world));

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
        if world.tick.is_multiple_of(npc_ai::NPC_THINK_PERIOD) {
            // AttackableAI think (1 s) + the combat-stance sweep (15 s
            // timeouts, checked at the same 1 s cadence as Java).
            timed!("npc_ai", npc_ai::npc_ai_tick(&mut world));
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
            timed!("autosave", autosave_tick(&mut world));
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
            timings.sort_by(|a, b| b.1.cmp(&a.1));
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
    // Persist every still-online player so level/exp/position survive the
    // restart (Java `Shutdown` save-all). These `StorePlayer` commands queue
    // ahead of the `DbCommand::Shutdown` `main` sends only after this thread
    // joins, so the DB thread drains them first.
    net::save_all_players(&mut world);
    // `Shutdown` → `OfflineTraderTable.storeOffliners()` — only meaningful with
    // realtime storing off; with it on (this dist) the rows are already current
    // and Java skips the sweep entirely.
    offline_trade::store_offliners(&world);
    // `Shutdown` → `BotReportTable.saveReportedCharData()`.
    bot_report::save_reports(&world);
    // `DBSpawnManager.updateDb` — every living raid boss's current HP/MP, so a
    // restart mid-fight resumes at the HP the boss was left on.
    boss_respawn::save_all_bosses(&mut world);
    // `Olympiad.saveOlympiadStatus` — the period row + every noble's points.
    olympiad::save_all(&world);
    // `Shutdown` → `CursedWeaponsManager.saveData()`: every live weapon's row.
    // Only `activate`/`increaseKills` write it during play, so without this the
    // kill tally and the time already burned off a wielded weapon are lost on
    // restart — it would come back with its count and deadline as of pickup.
    cursed_weapon::save_all(&world);
    // `Shutdown`: "Save all manor data", guarded by `!ALT_MANOR_SAVE_ALL_ACTIONS`
    // exactly as Java guards it — with per-action saving on, the rows are
    // already current and the sweep is redundant.
    manor::save_all_on_shutdown(&world);
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

/// Staggered periodic player flush — the port of `PlayerAutoSaveTaskManager.run`
/// and the timer half of the memory-first model. Flushes **at most one** due
/// player per sweep (Java's `break; // Prevent SQL flood`) and reschedules it
/// one `CharacterDataStoreInterval` out. Because gameplay only mutates in-memory
/// components, this — together with the logout and shutdown flushes — is the
/// sole writer of character state, so no packet flood can become a DB flood.
fn autosave_tick(world: &mut World) {
    let interval = world.cfg.character.character_data_store_interval_ticks;
    // The single due player this sweep (lowest object id = deterministic).
    let due = world
        .player_autosave_due
        .iter()
        .filter(|&(_, &due)| world.tick >= due)
        .map(|(&oid, _)| oid)
        .min();
    if let Some(oid) = due {
        world.player_autosave_due.insert(oid, world.tick + interval);
        net::store_player_now(world, oid);
    }
}

/// Dispatch every `Scheduler`-due task for this tick. Split from
/// `World::drain_due_tasks` because task handlers need to send packets to
/// `world.clients` — the same reason packet dispatch lives here too.
fn apply_due_tasks(world: &mut World) {
    for task in world.drain_due_tasks() {
        match task {
            ScheduledTask::Noop { .. } => {}
            ScheduledTask::ServerShutdownTick => {
                admin::server_shutdown_tick(world);
            }
            ScheduledTask::ServerRestartSchedule => {
                restart::handle_server_restart_schedule(world);
            }
            ScheduledTask::DebugDoorTick { object_id } => {
                admin::debug_draw::door_tick(world, object_id);
            }
            ScheduledTask::DebugGeoTick { object_id } => {
                admin::debug_draw::geo_tick(world, object_id);
            }
            ScheduledTask::DebugMoveTick { object_id } => {
                admin::debug_draw::move_tick(world, object_id);
            }
            ScheduledTask::SkillLaunch {
                player_object_id,
                cast_seq,
            } => {
                handle_skill_launch(world, player_object_id, cast_seq);
            }
            ScheduledTask::SkillFinish {
                player_object_id,
                cast_seq,
            } => {
                handle_skill_finish(world, player_object_id, cast_seq);
            }
            ScheduledTask::CastEnd {
                player_object_id,
                cast_seq,
            } => {
                handle_cast_end(world, player_object_id, cast_seq);
            }
            ScheduledTask::ChannelingTick {
                player_object_id,
                cast_seq,
            } => {
                skills::cast::handle_channeling_tick(world, player_object_id, cast_seq);
            }
            ScheduledTask::EffectPointCast { npc_oid } => {
                effect_point::handle_effect_point_cast(world, npc_oid);
            }
            ScheduledTask::EffectPointDespawn { npc_oid } => {
                effect_point::handle_effect_point_despawn(world, npc_oid);
            }
            ScheduledTask::RefreshVisuals { object_id } => {
                abnormal::refresh_visuals(world, object_id);
            }
            ScheduledTask::BuffExpire {
                player_object_id,
                skill_id,
            } => {
                // A re-cast/refresh pushes a fresh instance with a later expiry
                // and its own `BuffExpire`; only fire when the *current* buff has
                // actually elapsed, so a stale task can't drop the refreshed buff.
                let now = world.tick;
                let elapsed = world
                    .objects
                    .get_component::<crate::model::components::Buffs>(&player_object_id)
                    .and_then(|b| b.0.iter().find(|b| b.skill_id == skill_id))
                    .is_some_and(|b| b.expires_at_tick <= now);
                if elapsed {
                    handle_buff_expire(world, player_object_id, skill_id);
                }
            }
            ScheduledTask::ItemManaTick {
                player_object_id,
                item_object_id,
            } => {
                item_mana::on_mana_tick(world, player_object_id, item_object_id);
            }
            ScheduledTask::ServitorLifeTick { servitor_oid } => {
                servitor::handle_life_tick(world, servitor_oid);
            }
            ScheduledTask::DismountWaterUserInfo { object_id } => {
                // `if (isInWater()) broadcastUserInfo()` — the drowning task is
                // Java's predicate here, so a rider who landed on the bank
                // (or `AllowWater = False`) gets nothing, as in Java.
                if water::is_drowning_task_active(world, object_id) {
                    party::broadcast_user_info(world, object_id);
                }
            }
            ScheduledTask::GrandBossRespawn { boss_id } => {
                grand_boss::handle_grand_boss_respawn(world, boss_id);
            }
            ScheduledTask::QueenAntHeal { queen_oid } => {
                queen_ant::handle_heal_tick(world, queen_oid);
            }
            ScheduledTask::TomaRelocate => area_npcs::relocate_toma(world),
            ScheduledTask::MammonRelocate { npc_id } => {
                area_npcs::relocate_mammon(world, npc_id);
            }
            ScheduledTask::SinEaterTalk { pet_oid } => {
                crate::scripts::sin_eater::handle_talk_beat(world, pet_oid);
            }
            ScheduledTask::GuardRandomWalk { npc_oid } => {
                area_npcs::handle_guard_random_walk(world, npc_oid);
            }
            ScheduledTask::CastleMassTeleport { npc_oid } => {
                area_npcs::handle_castle_mass_teleport(world, npc_oid);
            }
            ScheduledTask::DayNightCheck { was_night } => {
                area_npcs::handle_day_night_check(world, was_night);
            }
            ScheduledTask::EilhalderDespawnRetry => {
                area_npcs::handle_eilhalder_despawn_retry(world);
            }
            ScheduledTask::FogRefresh => area_npcs::handle_fog_refresh(world),
            ScheduledTask::TamedBeastDuration { beast_oid } => {
                tamed_beast::handle_duration(world, beast_oid);
            }
            ScheduledTask::TamedBeastFollow { beast_oid } => {
                tamed_beast::handle_follow(world, beast_oid);
            }
            ScheduledTask::TamedBeastBuffCheck { beast_oid } => {
                tamed_beast::handle_buff_check(world, beast_oid);
            }
            ScheduledTask::MadCowPolymorph {
                cow_oid,
                feeder_oid,
            } => {
                tamed_beast::handle_mad_cow_polymorph(world, cow_oid, feeder_oid);
            }
            ScheduledTask::SprigantTrap { npc_oid } => {
                crate::scripts::primeval_isle::handle_sprigant_trap(world, npc_oid);
            }
            ScheduledTask::TrexAttack {
                trex_oid,
                player_oid,
            } => {
                crate::scripts::primeval_isle::handle_trex_attack(world, trex_oid, player_oid);
            }
            ScheduledTask::FsMysteriousChest { sepulcher } => {
                four_sepulchers::handle_mysterious_chest(world, sepulcher);
            }
            ScheduledTask::FsWaveCheck { sepulcher } => {
                four_sepulchers::handle_wave_check(world, sepulcher);
            }
            ScheduledTask::FsOust { sepulcher } => four_sepulchers::handle_oust(world, sepulcher),
            ScheduledTask::FsVictimFlee { npc_oid } => {
                crate::scripts::four_sepulchers::handle_victim_flee(world, npc_oid);
            }
            ScheduledTask::FsRemovePetrify { npc_oid } => {
                crate::scripts::four_sepulchers::handle_remove_petrify(world, npc_oid);
            }
            ScheduledTask::QueenAntDistanceCheck { queen_oid } => {
                queen_ant::handle_distance_check(world, queen_oid);
            }
            ScheduledTask::OrfenDistanceCheck { orfen_oid } => {
                orfen::handle_distance_check(world, orfen_oid);
            }
            ScheduledTask::BaiumSelectTarget => baium::handle_select_target(world),
            ScheduledTask::BaiumCinematic { step } => baium::handle_cinematic_step(world, step),
            ScheduledTask::BaiumCheckAttack => baium::handle_check_attack(world),
            ScheduledTask::BaiumClearZone => baium::handle_clear_zone(world),
            ScheduledTask::SailrenBeginFight => sailren::begin_fight(world),
            ScheduledTask::SailrenSpawn => sailren::handle_spawn_sailren(world),
            ScheduledTask::SailrenAttackEnable { sailren_oid } => {
                sailren::handle_attack_enable(world, sailren_oid)
            }
            ScheduledTask::ValakasCinematic { valakas_oid, step } => {
                valakas::handle_cinematic_step(world, valakas_oid, step);
            }
            ScheduledTask::ValakasBeginning => {
                valakas::handle_beginning_timer(world);
            }
            ScheduledTask::ValakasDeathCinematic { valakas_oid, step } => {
                valakas::handle_death_cinematic_step(world, valakas_oid, step);
            }
            ScheduledTask::ValakasRemovePlayers => {
                valakas::handle_remove_players(world);
            }
            ScheduledTask::ValakasRegen { valakas_oid } => {
                valakas::handle_regen(world, valakas_oid);
            }
            ScheduledTask::ValakasSkillTask { valakas_oid } => {
                valakas::handle_skill_task(world, valakas_oid);
            }
            ScheduledTask::DrChaosParanoia { dr_chaos_oid } => {
                dr_chaos::handle_paranoia(world, dr_chaos_oid);
            }
            ScheduledTask::DrChaosTransform { dr_chaos_oid, step } => {
                dr_chaos::handle_transform(world, dr_chaos_oid, step);
            }
            ScheduledTask::DrChaosGolemDespawn { golem_oid } => {
                dr_chaos::handle_golem_despawn(world, golem_oid);
            }
            ScheduledTask::DrChaosReset => {
                dr_chaos::handle_reset(world);
            }
            ScheduledTask::AntharasSpawn => {
                antharas::handle_spawn_timer(world);
            }
            ScheduledTask::AntharasMinionWave { antharas_oid } => {
                antharas::handle_wave(world, antharas_oid);
            }
            ScheduledTask::AntharasCinematic { antharas_oid, step } => {
                antharas::handle_cinematic_step(world, antharas_oid, step);
            }
            ScheduledTask::AntharasSocial { antharas_oid } => {
                antharas::handle_social(world, antharas_oid);
            }
            ScheduledTask::AntharasClearZone => {
                antharas::handle_clear_zone(world);
            }
            ScheduledTask::ClanHallAuctionEnd => {
                clan_hall_auction::handle_auction_end(world);
            }
            ScheduledTask::ClanHallLeaseCheck { hall_id } => {
                clan_hall_auction::handle_lease_check(world, hall_id);
            }
            ScheduledTask::ClanHallFunctionExpire { hall_id, func_id } => {
                clan_hall_function::handle_function_expiry(world, hall_id, func_id);
            }
            ScheduledTask::AntharasSetRegen { antharas_oid } => {
                antharas::handle_set_regen(world, antharas_oid);
            }
            ScheduledTask::AntharasCheckAttack { antharas_oid } => {
                antharas::handle_check_attack(world, antharas_oid);
            }
            ScheduledTask::CoreMinionRespawn { npc_id } => {
                core_boss::handle_minion_respawn(world, npc_id);
            }
            ScheduledTask::CoreDespawnMinions => {
                core_boss::handle_despawn_minions(world);
            }
            ScheduledTask::DespawnNpc { npc_oid } => {
                if let Some(region) = world
                    .objects
                    .get_component::<crate::model::components::RegionCell>(&npc_oid)
                    .map(|r| r.0)
                {
                    death::despawn_npc(world, npc_oid, region);
                }
            }
            ScheduledTask::BoatArrive { boat_object_id } => {
                boats::handle_arrive(world, boat_object_id);
            }
            ScheduledTask::BoatDepart { boat_object_id } => {
                boats::handle_depart(world, boat_object_id);
            }
            ScheduledTask::BoatDwellStage {
                boat_object_id,
                stage,
            } => {
                boats::handle_dwell_stage(world, boat_object_id, stage);
            }
            ScheduledTask::BoatVoyageShout {
                boat_object_id,
                messages,
            } => {
                boats::handle_voyage_shout(world, boat_object_id, messages);
            }
            ScheduledTask::OlympiadCompStart => olympiad::handle_comp_start(world),
            ScheduledTask::OlympiadCompEnd => olympiad::handle_comp_end(world),
            ScheduledTask::OlympiadWeeklyChange => olympiad::handle_weekly_change(world),
            ScheduledTask::OlympiadGameManager => olympiad::handle_game_manager(world),
            ScheduledTask::OlympiadCountdown { arena, step } => {
                olympiad::handle_countdown(world, arena, step)
            }
            ScheduledTask::OlympiadMatchTick { arena } => olympiad::handle_match_tick(world, arena),
            ScheduledTask::OlympiadEnd => olympiad::handle_olympiad_end(world),
            ScheduledTask::OlympiadValidationEnd => olympiad::handle_validation_end(world),
            ScheduledTask::InstanceEmptyCheck { instance_id } => {
                instances::handle_empty_check(world, instance_id)
            }
            ScheduledTask::FrintezzaIntro { instance_id, step } => {
                frintezza::handle_intro_step(world, instance_id, step)
            }
            ScheduledTask::FrintezzaFight { instance_id, step } => {
                frintezza::handle_fight_step(world, instance_id, step)
            }
            ScheduledTask::FrintezzaSong { instance_id } => {
                frintezza::handle_song(world, instance_id)
            }
            ScheduledTask::FrintezzaDemons { instance_id } => {
                frintezza::handle_demon_spawn(world, instance_id)
            }
            ScheduledTask::FrintezzaFinish { instance_id, step } => {
                frintezza::handle_finish_step(world, instance_id, step)
            }
            ScheduledTask::ScarletSkill { instance_id } => {
                frintezza::handle_scarlet_skill(world, instance_id)
            }
            ScheduledTask::FishingReel {
                player_object_id,
                cast_seq,
            } => {
                fishing::handle_reel(world, player_object_id, cast_seq);
            }
            ScheduledTask::FishingCast {
                player_object_id,
                cast_seq,
            } => {
                fishing::handle_cast(world, player_object_id, cast_seq);
            }
            ScheduledTask::CubicAction {
                owner_oid,
                cubic_id,
            } => {
                cubic::handle_cubic_action(world, owner_oid, cubic_id);
            }
            ScheduledTask::MountFeedTick { player_oid } => {
                admin::mounts::handle_mount_feed_tick(world, player_oid);
            }
            ScheduledTask::PetFeedTick { pet_oid } => {
                servitor::handle_feed_tick(world, pet_oid);
            }
            ScheduledTask::DamOverTimeTick {
                caster,
                target,
                skill_id,
                skill_level,
            } => {
                skills::effects::handle_dam_over_time_tick(
                    world,
                    caster,
                    target,
                    skill_id,
                    skill_level,
                );
            }
            ScheduledTask::AttackHit {
                attacker,
                target,
                damage,
                miss,
                crit,
                swing_seq,
            } => {
                combat::handle_attack_hit(world, attacker, target, damage, miss, crit, swing_seq);
            }
            ScheduledTask::ResetCharges { player_oid, seq } => {
                skills::effects::reset_charges(world, player_oid, seq);
            }
            ScheduledTask::NpcSuicide { npc_oid } => {
                // Java `doDie(null)`. Killer 0 is inert on every reward and
                // aggro path (both gate on the killer being playable), so this
                // is the animation and the corpse and nothing else.
                death::npc_do_die(world, npc_oid, 0);
            }
            ScheduledTask::SiegeFame { player_oid } => {
                siege::handle_siege_fame(world, player_oid);
            }
            ScheduledTask::AttackFinish { object_id } => {
                helpers::run_queued_action(world, object_id);
            }
            ScheduledTask::NpcAttackReady { npc_oid } => {
                npc_ai::on_npc_attack_ready(world, npc_oid);
            }
            ScheduledTask::GroundItemDecay { item_object_id } => {
                ground_items::handle_ground_item_decay(world, item_object_id);
            }
            ScheduledTask::BossRespawn { spawn_ref } => {
                boss_respawn::handle_boss_respawn(world, spawn_ref);
            }
            ScheduledTask::MinionRespawn {
                master_object_id,
                minion_npc_id,
            } => {
                minions::handle_minion_respawn(world, master_object_id, minion_npc_id);
            }
            ScheduledTask::NpcDecay { npc_object_id } => {
                death::handle_npc_decay(world, npc_object_id);
            }
            ScheduledTask::NpcRespawn {
                spawn_idx,
                group_idx,
                npc_idx,
            } => {
                death::handle_npc_respawn(world, spawn_idx, group_idx, npc_idx);
            }
            ScheduledTask::RequestTimeout { object_id, seq } => {
                party::handle_request_timeout(world, object_id, seq);
            }
            ScheduledTask::ClanDissolve { clan_id } => {
                clans::handle_clan_dissolve_task(world, clan_id);
            }
            ScheduledTask::ClanWarTimeout { attacker, attacked } => {
                clans::handle_clan_war_timeout(world, attacker, attacked);
            }
            ScheduledTask::ClanWarDelete { clan1, clan2 } => {
                clans::delete_clan_wars(world, clan1, clan2);
            }
            ScheduledTask::PartyPositionBroadcast { party_id, seq } => {
                party::handle_position_broadcast(world, party_id, seq);
            }
            ScheduledTask::DuelCountdown { duel_id } => duel::handle_countdown(world, duel_id),
            ScheduledTask::DuelTick { duel_id } => duel::handle_tick(world, duel_id),
            ScheduledTask::PartyLootChangeTimeout { party_id, seq } => {
                party::handle_loot_change_timeout(world, party_id, seq);
            }
            ScheduledTask::QuestTimer {
                quest,
                name,
                player,
                npc,
                seq,
            } => {
                quests::handle_quest_timer(world, quest, &name, player, npc, seq);
            }
            ScheduledTask::DoorAutoClose {
                door_object_id,
                seq,
            } => {
                doors::handle_door_auto_close(world, door_object_id, seq);
            }
            ScheduledTask::DoorTimerToggle { door_object_id } => {
                doors::handle_door_timer_toggle(world, door_object_id);
            }
            ScheduledTask::RecoGive {
                player_object_id,
                seq,
            } => {
                reco::handle_reco_give(world, player_object_id, seq);
            }
            ScheduledTask::PcCafeReward {
                player_object_id,
                seq,
            } => {
                pc_cafe::handle_reward(world, player_object_id, seq);
            }
            ScheduledTask::DailyReset => {
                daily_tasks::handle_daily_reset(world);
            }
            ScheduledTask::BotReportPointsReset => {
                bot_report::handle_points_reset(world);
            }
            ScheduledTask::SiegeEnd { castle_id } => {
                siege::end_siege(world, castle_id);
            }
            ScheduledTask::SiegeStart { castle_id } => {
                siege::handle_scheduled_siege_start(world, castle_id);
            }
            ScheduledTask::ManorModeChange => {
                manor::advance_manor_mode(world);
            }
            ScheduledTask::ManorAutosave => {
                manor::handle_autosave(world);
            }
            ScheduledTask::InventoryEnable { object_id } => {
                // Java's task sets the flag false unconditionally, so a second
                // window opened inside the window is unblocked by the *first*
                // task rather than extending the block. Removing here keeps
                // that, where an expiry timestamp would not.
                world.inventory_blocked.remove(&object_id);
            }
            ScheduledTask::SitDownFinish { object_id } => {
                sit_stand::handle_sit_down_finish(world, object_id);
            }
            ScheduledTask::StandUpFinish { object_id } => {
                sit_stand::handle_stand_up_finish(world, object_id);
            }
            ScheduledTask::CursedWeaponExpiry { item_id } => {
                cursed_weapon::handle_expiry(world, item_id);
            }
            ScheduledTask::MailExpire { message_id } => {
                mail::handle_expiry(world, message_id);
            }
            ScheduledTask::TvtTeleportToArena => {
                events::tvt::teleport_to_arena(world);
            }
            ScheduledTask::TvtStartFight => {
                events::tvt::start_fight(world);
            }
            ScheduledTask::TvtEndFight => {
                events::tvt::end_fight(world);
            }
            ScheduledTask::TvtResurrect { player } => {
                events::tvt::resurrect_player(world, player);
            }
            ScheduledTask::EventSchedule { index, pattern } => {
                events::on_schedule_fired(world, index, pattern);
            }
            ScheduledTask::TvtInactivity {
                player,
                warning,
                seq,
            } => {
                events::tvt::inactivity_tick(world, player, warning, seq);
            }
            ScheduledTask::TvtCountdown { seconds, seq } => {
                events::tvt::countdown(world, seconds, seq);
            }
            ScheduledTask::TvtScoreBoard => {
                events::tvt::score_board(world);
            }
            ScheduledTask::TvtTeleportOut => {
                events::tvt::teleport_out(world);
            }
            ScheduledTask::LotteryStart => lottery::open_round(world),
            ScheduledTask::LotteryStopSelling => lottery::stop_selling(world),
            ScheduledTask::LotteryFinish => lottery::finish_begin(world),
            ScheduledTask::MonsterRaceTick => monster_race::tick(world),
            ScheduledTask::ItemAuctionState { auction_id } => {
                item_auction::run_state_task(world, auction_id)
            }
            ScheduledTask::PunishmentExpire { punishment_id } => {
                punishment::on_expire(world, punishment_id)
            }
        }
    }
}
