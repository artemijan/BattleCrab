//! The game thread and its 100 ms tick loop (CONCURRENCY_MODEL §2.2).
//!
//! Runs on one dedicated OS thread that owns [`World`]. The base tick is 100 ms,
//! matching Java's `GameTimeTaskManager` and high-priority task-manager rate.
//! Steps: drain network events → drain login-link events → fire timers → run
//! tick systems (G4+) → flush. Packet dispatch and login handoff land here on
//! the game thread, keeping handler code sequential and 1:1 with Java `run()`.

mod abnormal;
pub(crate) mod admin;
pub(crate) mod antharas;
mod augment;
pub(crate) mod baium;
pub(crate) mod boats;
mod boss_respawn;
mod boss_threat;
mod bypass;
mod chat;
pub(crate) mod clan_hall_auction;
pub(crate) mod clan_hall_function;
mod clans;
mod combat;
mod common;
mod community_board;
mod core_boss;
mod crafting;
mod cubic;
pub(crate) mod cursed_weapon;
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
mod friends;
pub(crate) mod frintezza;
mod grand_boss;
mod ground_items;
mod helpers;
mod henna;
pub(crate) mod instances;
mod items;
mod lobby;
pub(crate) mod lottery;
pub(crate) mod manor;
pub(crate) mod minions;
pub(crate) mod multisell;
mod net;
pub(crate) mod npc_ai;
mod npc_cast;
mod npc_view;
pub(crate) mod olympiad;
mod orfen;
mod party;
mod passive_skills;
pub(crate) mod position;
mod private_store;
mod pvp;
mod queen_ant;
pub mod quests;
mod raid_curse;
mod ranged;
mod reco;
pub(crate) mod regen;
pub(crate) mod sailren;
mod servitor;
pub(crate) mod shop;
mod shortcuts;
mod siege;
mod skill_enchant;
pub(crate) mod skills;
pub(crate) mod subclass;
pub(crate) mod support_magic;
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
pub(crate) mod zones;

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
use regen::{run_npc_regen_tick, run_regen_tick, REGEN_TICK_PERIOD};
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
    /// Released once all boot data (incl. clans) is loaded, letting the
    /// login-link task begin connecting to the login server.
    pub login_ready_tx: tokio::sync::oneshot::Sender<()>,
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
        login_ready_tx,
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
    // Java `DailyTaskManager`: the daily 06:30 reset (recommendations only, so
    // far). Scheduled once here; the task reschedules itself every 24 h.
    reco::schedule_initial_daily_reset(&mut world);
    // Grand bosses spawn/respawn once their data lands — the `grandboss_data`
    // table arrives asynchronously as `DbEvent::GrandBossesLoaded`, so
    // `grand_boss::resolve_at_boot` (and `dr_chaos`) run from that handler, not
    // here where `world.grand_bosses` is still empty.
    boats::spawn_boats(&mut world);

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
        if world.tick.is_multiple_of(effect_zones::SWEEP_PERIOD) {
            effect_zones::effect_zone_tick(&mut world);
            effect_zones::damage_zone_tick(&mut world);
        }
        if world.tick.is_multiple_of(walkers::WALKER_PERIOD) {
            walkers::walker_tick(&mut world);
        }
        if world.tick.is_multiple_of(npc_ai::NPC_THINK_PERIOD) {
            // AttackableAI think (1 s) + the combat-stance sweep (15 s
            // timeouts, checked at the same 1 s cadence as Java).
            npc_ai::npc_ai_tick(&mut world);
            combat::stance_tick(&mut world);
            pvp::pvp_flag_tick(&mut world);
        }
        if world.tick.is_multiple_of(REGEN_TICK_PERIOD) {
            run_regen_tick(&mut world);
            run_npc_regen_tick(&mut world);
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
    // `DBSpawnManager.updateDb` — every living raid boss's current HP/MP, so a
    // restart mid-fight resumes at the HP the boss was left on.
    boss_respawn::save_all_bosses(&mut world);
    // `Olympiad.saveOlympiadStatus` — the period row + every noble's points.
    olympiad::save_all(&world);
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
            ScheduledTask::ServitorLifeTick { servitor_oid } => {
                servitor::handle_life_tick(world, servitor_oid);
            }
            ScheduledTask::GrandBossRespawn { boss_id } => {
                grand_boss::handle_grand_boss_respawn(world, boss_id);
            }
            ScheduledTask::QueenAntHeal { queen_oid } => {
                queen_ant::handle_heal_tick(world, queen_oid);
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
            } => {
                combat::handle_attack_hit(world, attacker, target, damage, miss, crit);
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
            ScheduledTask::DailyRecoReset => {
                reco::handle_daily_reco_reset(world);
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
            ScheduledTask::CursedWeaponExpiry { item_id } => {
                cursed_weapon::handle_expiry(world, item_id);
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
            ScheduledTask::TvtScoreBoard => {
                events::tvt::score_board(world);
            }
            ScheduledTask::TvtTeleportOut => {
                events::tvt::teleport_out(world);
            }
            ScheduledTask::LotteryStart => lottery::open_round(world),
            ScheduledTask::LotteryStopSelling => lottery::stop_selling(world),
            ScheduledTask::LotteryFinish => lottery::finish_lottery(world),
        }
    }
}
