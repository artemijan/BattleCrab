//! The `ScheduledTask` dispatch table: route every timer due this tick to its
//! owning module — the timer face of `dispatch.rs`'s packet routing.

use super::*;

/// Dispatch every `Scheduler`-due task for this tick. Split from
/// `World::drain_due_tasks` because task handlers need to send packets to
/// `world.clients` — the same reason packet dispatch lives here too.
pub(crate) fn apply_due_tasks(world: &mut World) {
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
                    player_info::broadcast_user_info(world, object_id);
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
            ScheduledTask::BroadcastCharInfo { object_id } => {
                player_info::broadcast_char_info_now(world, object_id);
            }
            ScheduledTask::SkillsReenable { object_id } => {
                world
                    .objects
                    .remove_component::<crate::model::components::SkillsDisabled>(&object_id);
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
            ScheduledTask::BaiumClearZone => baium::clear_zone(world),
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
            ScheduledTask::BuyListRestock { list_id, item_id } => {
                shop::handle_restock(world, list_id, item_id);
            }
            ScheduledTask::RemoveWornPreview { player_oid } => {
                shop::handle_remove_worn_preview(world, player_oid);
            }
            ScheduledTask::ClanHallAuctionEnd => {
                clans::hall_auction::handle_auction_end(world);
            }
            ScheduledTask::ClanHallLeaseCheck { hall_id } => {
                clans::hall_auction::handle_lease_check(world, hall_id);
            }
            ScheduledTask::ClanHallFunctionExpire { hall_id, func_id } => {
                clans::hall_function::handle_function_expiry(world, hall_id, func_id);
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
                death::despawn_npc_by_oid(world, npc_oid);
            }
            ScheduledTask::BoatArrive { boat_object_id } => {
                boats::handle_arrive(world, boat_object_id);
            }
            ScheduledTask::BoatDepart { boat_object_id } => {
                boats::depart(world, boat_object_id);
            }
            ScheduledTask::BoatDwellStage {
                boat_object_id,
                stage,
            } => {
                boats::run_dwell_stage(world, boat_object_id, stage);
            }
            ScheduledTask::BoatVoyageShout {
                boat_object_id,
                schedule,
                shout,
            } => {
                boats::handle_voyage_shout(world, boat_object_id, schedule, shout);
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
            ScheduledTask::InstanceEjectDead { player_object_id } => {
                instances::handle_eject_dead(world, player_object_id);
            }
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
            ScheduledTask::BabyPetHealTick { pet_oid } => {
                crate::scripts::baby_pets::handle_heal_tick(world, pet_oid);
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
                ai::on_npc_attack_ready(world, npc_oid);
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
            ScheduledTask::CraftPass { crafter_oid } => {
                crafting::handle_craft_pass(world, crafter_oid)
            }
            ScheduledTask::CraftFinish { crafter_oid } => {
                crafting::handle_craft_finish(world, crafter_oid)
            }
            ScheduledTask::CastleFunctionRenew {
                castle_id,
                func_type,
                charge_warehouse,
            } => castle::handle_function_renew(world, castle_id, func_type, charge_warehouse),
            ScheduledTask::CreatureSeeSweep => quests::handle_creature_see_sweep(world),
            ScheduledTask::IllegalActionPunish {
                object_id,
                message,
                punishment,
            } => punishment::on_illegal_action_punish(world, object_id, &message, punishment),
        }
    }
}
