//! The game thread and its 100 ms tick loop (CONCURRENCY_MODEL §2.2).
//!
//! Runs on one dedicated OS thread that owns [`World`]. The base tick is 100 ms,
//! matching Java's `GameTimeTaskManager` and high-priority task-manager rate.
//! Steps: drain network events → drain login-link events → fire timers → run
//! tick systems (G4+) → flush. Packet dispatch and login handoff land here on
//! the game thread, keeping handler code sequential and 1:1 with Java `run()`.

mod bypass;
mod chat;
mod clans;
mod combat;
mod death;
mod dispatch;
mod doors;
mod expertise;
mod friends;
mod helpers;
mod items;
mod lobby;
mod net;
mod npc_ai;
mod party;
mod passive_skills;
mod position;
pub mod quests;
mod regen;
mod shop;
mod shortcuts;
mod skills;
mod target;
#[cfg(test)]
mod tests;
mod visibility;
mod zones;
mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tracing::{info, warn};

use crate::data::GameData;
use crate::db::{self, DbEventRx};
use crate::loginlink::{CommandTx, LoginLinkEventRx};
use crate::network::NetEventRx;
use crate::scheduler::ScheduledTask;
use crate::world::World;

use net::{drain_db, drain_login_link, drain_network, drain_path};
use regen::{run_regen_tick, REGEN_TICK_PERIOD};
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
    pub net_rx: NetEventRx,
    pub login_rx: LoginLinkEventRx,
    pub link_tx: CommandTx,
    pub db_rx: DbEventRx,
    pub db_tx: db::CmdTx,
    pub data: GameData,
    pub geo: std::sync::Arc<crate::geo::GeoEngine>,
    pub path_tx: crate::geo::worker::PathReqTx,
    pub path_rx: crate::geo::worker::PathEventRx,
    pub path_finding: i32,
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
        net_rx,
        login_rx,
        link_tx,
        db_rx,
        db_tx,
        data,
        geo,
        path_tx,
        path_rx,
        path_finding,
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
    world.path = path_tx;
    world.path_finding = path_finding;
    world.cfg = cfg;

    // Java `GameServer`: SpawnData.getInstance().init() — place the static
    // world content before accepting anyone in.
    crate::model::npc::spawn_all(&mut world);
    // DoorData's boot spawn (entities + BY_TIME cycles; the collision grid
    // was registered into the GeoEngine in main.rs, before it was shared).
    crate::model::door::spawn_doors(&mut world);
    doors::start_time_cycles(&mut world);
    crate::model::static_object::spawn_static_objects(&mut world);

    info!("GameLoop: started ({} ms tick).", TICK.as_millis());

    while !shutdown.is_requested() {
        let tick_start = Instant::now();

        // 1. Network events: connects, disconnects, and inbound packets.
        drain_network(&mut world, &net_rx);
        // 2. Service results: login-link + DB + path worker.
        drain_login_link(&mut world, &login_rx);
        drain_db(&mut world, &db_rx);
        drain_path(&mut world, &path_rx);

        // 3. One-shot timers due this tick.
        apply_due_tasks(&mut world);

        // 4. Fixed-rate tick systems (movement, AI, attack…) — added in G4+.
        // Movement runs every tick (unlike the gated systems below) — it
        // needs to recompute the authoritative server-side position each
        // 100 ms, same as Java's `MovementTaskManager`. Region-switch
        // visibility events (CharInfo/DeleteObject) ride along.
        visibility::movement_tick(&mut world);
        // Player attack intents (chase + swing) every tick, like Java's
        // event-driven PlayerAI reacting as soon as it's ready to act.
        combat::player_combat_tick(&mut world);
        if world.tick.is_multiple_of(npc_ai::NPC_THINK_PERIOD) {
            // AttackableAI think (1 s) + the combat-stance sweep (15 s
            // timeouts, checked at the same 1 s cadence as Java).
            npc_ai::npc_ai_tick(&mut world);
            combat::stance_tick(&mut world);
        }
        if world.tick.is_multiple_of(REGEN_TICK_PERIOD) {
            run_regen_tick(&mut world);
        }
        if world.tick.is_multiple_of(AUTOSAVE_CHECK_PERIOD) {
            autosave_tick(&mut world);
        }
        // 5. Flush outbound packets / DB commands — added in G3+.

        let elapsed = tick_start.elapsed();
        if elapsed > TICK_OVERRUN_WARN {
            warn!(
                "GameLoop: tick {} ran {} ms (budget {} ms).",
                world.tick,
                elapsed.as_millis(),
                TICK.as_millis()
            );
        }
        if let Some(remaining) = TICK.checked_sub(elapsed) {
            std::thread::sleep(remaining);
        }

        world.tick += 1;
    }

    info!("GameLoop: stopped after {} ticks.", world.tick);
    // Persist every still-online player so level/exp/position survive the
    // restart (Java `Shutdown` save-all). These `StorePlayer` commands queue
    // ahead of the `DbCommand::Shutdown` `main` sends only after this thread
    // joins, so the DB thread drains them first.
    net::save_all_players(&mut world);
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
            ScheduledTask::SkillLaunch { player_object_id, cast_seq } => {
                handle_skill_launch(world, player_object_id, cast_seq);
            }
            ScheduledTask::SkillFinish { player_object_id, cast_seq } => {
                handle_skill_finish(world, player_object_id, cast_seq);
            }
            ScheduledTask::CastEnd { player_object_id, cast_seq } => {
                handle_cast_end(world, player_object_id, cast_seq);
            }
            ScheduledTask::BuffExpire { player_object_id, skill_id } => {
                handle_buff_expire(world, player_object_id, skill_id);
            }
            ScheduledTask::AttackHit { attacker, target, damage, miss, crit } => {
                combat::handle_attack_hit(world, attacker, target, damage, miss, crit);
            }
            ScheduledTask::AttackFinish { object_id } => {
                helpers::run_queued_action(world, object_id);
            }
            ScheduledTask::NpcDecay { npc_object_id } => {
                death::handle_npc_decay(world, npc_object_id);
            }
            ScheduledTask::NpcRespawn { spawn_idx, group_idx, npc_idx } => {
                death::handle_npc_respawn(world, spawn_idx, group_idx, npc_idx);
            }
            ScheduledTask::RequestTimeout { object_id, seq } => {
                party::handle_request_timeout(world, object_id, seq);
            }
            ScheduledTask::PartyPositionBroadcast { party_id, seq } => {
                party::handle_position_broadcast(world, party_id, seq);
            }
            ScheduledTask::PartyLootChangeTimeout { party_id, seq } => {
                party::handle_loot_change_timeout(world, party_id, seq);
            }
            ScheduledTask::QuestTimer { quest, name, player, npc, seq } => {
                quests::handle_quest_timer(world, quest, &name, player, npc, seq);
            }
            ScheduledTask::DoorAutoClose { door_object_id, seq } => {
                doors::handle_door_auto_close(world, door_object_id, seq);
            }
            ScheduledTask::DoorTimerToggle { door_object_id } => {
                doors::handle_door_timer_toggle(world, door_object_id);
            }
        }
    }
}

