//! The DB event fan-out: route each boot-loaded table and query result to
//! its owning module.

use super::*;
/// One DB result: a boot-load table landing, or a mid-session read's
/// continuation (character list, name check, id block…).
pub(crate) fn handle_db_event(world: &mut World, event: DbEvent) {
    match event {
        DbEvent::CharactersLoaded {
            client_id,
            account,
            chars,
            send_list,
        } => {
            on_characters_loaded(world, client_id, account, chars, send_list);
        }
        DbEvent::CharacterCreated { client_id, result } => {
            use crate::db::CreateResult::*;
            let body = match result {
                Ok => server_packets::char_create_ok(),
                // NAME_ALREADY_EXISTS=2, TOO_MANY=1, CREATION_FAILED=0.
                NameExists => server_packets::char_create_fail(2),
                TooMany => server_packets::char_create_fail(1),
                Fail => server_packets::char_create_fail(0),
            };
            send_to_client(world, client_id, body);
        }
        DbEvent::CharCount {
            account,
            count,
            del_times,
        } => {
            let _ = world.login.link.send(LoginLinkCommand::ReplyCharacters {
                account,
                chars: count,
                del_times,
            });
        }
        DbEvent::NameCreatable { client_id, result } => {
            send_to_client(
                world,
                client_id,
                server_packets::ex_is_char_name_creatable(result),
            );
        }
        DbEvent::IdBlock { start, end } => {
            world.id_pool = start..end;
        }
        DbEvent::GlobalVariablesLoaded { entries } => {
            tracing::info!("GameLoop: loaded {} global variables.", entries.len());
            world.global_vars = entries.into_iter().collect();
            crate::game_loop::four_sepulchers::restore_entry_times(world);
            // Re-derive upgraded castle-door HP now that the ratios are known
            // (the doors spawned before this table landed) — Java's
            // `loadDoorUpgrade` at castle load.
            crate::game_loop::castle::apply_door_upgrades_at_boot(world);
        }
        DbEvent::PremiumLoaded { entries } => {
            tracing::info!("GameLoop: loaded {} premium accounts.", entries.len());
            world.premium = entries.into_iter().collect();
        }
        DbEvent::LotteryLoaded { row, draws } => {
            crate::game_loop::lottery::on_loaded(world, row, draws);
        }
        DbEvent::LotteryTicketsLoaded { round, rows } => {
            crate::game_loop::lottery::finish_complete(world, round, rows);
        }
        DbEvent::MdtLoaded { history, bets } => {
            crate::game_loop::monster_race::on_mdt_loaded(world, history, bets);
        }
        DbEvent::MailLoaded {
            messages,
            attachments,
            char_ids_by_name,
            block_lists,
        } => {
            crate::game_loop::mail::on_loaded(
                world,
                messages,
                attachments,
                char_ids_by_name,
                block_lists,
            );
        }
        DbEvent::ItemAuctionsLoaded {
            next_auction_id,
            auctions,
        } => {
            crate::game_loop::item_auction::on_loaded(world, next_auction_id, auctions);
        }
        DbEvent::PunishmentsLoaded {
            next_id,
            punishments,
        } => {
            crate::game_loop::punishment::on_loaded(world, next_id, punishments);
        }
        DbEvent::BotReportsLoaded { rows } => {
            let last_reset = crate::game_loop::bot_report::last_reset_millis(
                &world.cfg.bot_report,
                commons::util::now_millis(),
            );
            crate::game_loop::bot_report::on_loaded(world, rows, last_reset);
        }
        DbEvent::BufferSchemesLoaded { entries } => {
            // Java `SchemeBufferTable.load` drops any saved skill id no longer
            // in `_availableBuffs`; the buffer table lives here on the game
            // thread, so the filter runs at insert time (like grand bosses).
            for (object_id, scheme_name, skills) in entries {
                let skills: Vec<i32> = skills
                    .into_iter()
                    .filter(|id| world.data.scheme_buffer.contains(*id))
                    .collect();
                world
                    .buffer_schemes
                    .entry(object_id)
                    .or_default()
                    .push((scheme_name, skills));
            }
            tracing::info!(
                "GameLoop: loaded buffer schemes for {} characters.",
                world.buffer_schemes.len()
            );
        }
        DbEvent::FavoritesLoaded { entries } => {
            // `favId` is a table-wide AUTOINCREMENT PK; seed the game-thread
            // allocator past the highest loaded id so new favorites stay unique.
            let mut max_id = 0;
            for (player_id, fav_id, title, bypass, add_date) in entries {
                max_id = max_id.max(fav_id);
                world
                    .bbs_favorites
                    .entry(player_id)
                    .or_default()
                    .push(crate::world::Favorite {
                        fav_id,
                        title,
                        bypass,
                        add_date,
                    });
            }
            world.next_fav_id = max_id + 1;
            tracing::info!(
                "GameLoop: loaded favorites for {} characters.",
                world.bbs_favorites.len()
            );
        }
        DbEvent::NpcRespawnsLoaded { rows } => {
            // Settle the `dbSave` spawns the static pass deferred (Java's
            // `DBSpawnManager.load` + the `spawnNpc` hand-off).
            crate::game_loop::boss_respawn::resolve_boot(world, rows);
        }
        DbEvent::OfflineTradersLoaded { traders } => {
            // `GameServer.main`'s `OfflineTraderTable.restoreOfflineTraders()`.
            crate::game_loop::offline_trade::restore_offline_traders(world, traders);
        }
        DbEvent::GrandBossesLoaded { bosses } => {
            // Java skips rows whose NPC template is missing (`NpcData
            // .getTemplate(bossId) != null`); the datapack lives here on the
            // game thread, so the filter runs at insert time.
            world.grand_bosses = bosses
                .into_iter()
                .filter(|b| world.data.npc_data.get(b.boss_id).is_some())
                .map(|b| (b.boss_id, b))
                .collect();
            tracing::info!(
                "GameLoop: loaded {} grand bosses.",
                world.grand_bosses.len()
            );
            // Spawn the ones that are up, arm timers for the rest, and
            // immediately respawn any whose window elapsed while the server
            // was down. Must run *here*, once the data has landed — the
            // static world (`spawn_all`, geo) is already up before the loop.
            crate::game_loop::grand_boss::resolve_at_boot(world);
            crate::game_loop::dr_chaos::resolve_at_boot(world);
        }
        DbEvent::CursedWeaponsLoaded { rows } => {
            // Build from the XML config, compute each skill's max level, then
            // overlay the persisted wielder state (Java `restore` →
            // `reActivate`). The default table is empty, so both usually
            // start inactive.
            let mut weapons = world.data.cursed_weapons.weapons.clone();
            for cw in &mut weapons {
                cw.skill_max_level = (1..=100)
                    .take_while(|l| world.data.skill_data.get(cw.skill_id, *l).is_some())
                    .last()
                    .unwrap_or(1);
                if let Some(row) = rows.iter().find(|r| r.item_id == cw.item_id) {
                    cw.player_id = row.char_id;
                    cw.player_reputation = row.player_reputation;
                    cw.player_pk_kills = row.player_pk_kills;
                    cw.nb_kills = row.nb_kills;
                    cw.end_time = row.end_time;
                    cw.is_activated = true;
                }
            }
            tracing::info!("GameLoop: loaded {} cursed weapons.", weapons.len());
            world.cursed_weapons = weapons;
            // Java `restore()` → `reActivate()`: a weapon that survived a
            // restart gets its `RemoveTask` armed again. Without this the
            // restored curse is immortal — the wielder keeps it forever,
            // since only this timer ever calls `endOfLife`. One whose
            // deadline passed while the server was down fires immediately
            // (`arm_expiry` clamps the delay at 0, `handle_expiry`
            // re-checks `end_time`).
            for idx in 0..world.cursed_weapons.len() {
                if world.cursed_weapons[idx].is_activated {
                    crate::game_loop::cursed_weapon::arm_expiry(world, idx);
                }
            }
        }
        DbEvent::CastlesLoaded { castles } => {
            tracing::info!("GameLoop: loaded {} castles.", castles.len());
            world.castles = castles;
        }
        DbEvent::SiegesLoaded { rows } => {
            // One Siege per castle (Java creates a Siege for every castle),
            // then attach the registered clans from `siege_clans`.
            use crate::model::siege::{Siege, SiegeClanType};
            let mut sieges: std::collections::HashMap<i32, Siege> = world
                .castles
                .iter()
                .map(|c| (c.id, Siege::new(c.id)))
                .collect();
            for row in &rows {
                if let (Some(siege), Some(kind)) = (
                    sieges.get_mut(&row.castle_id),
                    SiegeClanType::from_db(row.kind),
                ) {
                    siege.add_clan(row.clan_id, kind);
                }
            }
            tracing::info!(
                "GameLoop: loaded sieges for {} castles ({} registered clans).",
                sieges.len(),
                rows.len()
            );
            world.sieges = sieges;
            // The per-castle Siege records now exist — arm the weekly
            // auto-start schedule (`SiegeSchedule.xml`).
            crate::game_loop::siege::schedule_all_at_boot(world);
        }
        DbEvent::ManorLoaded {
            production,
            procure,
        } => {
            // Group the rows by castle + period, dropping ids not in the
            // seed catalogue (Java's "Don't load unknown seeds/crops").
            use crate::model::manor::{CropProcure, ManorState, SeedProduction};
            let mut manor = ManorState::default();
            let mut prod: std::collections::HashMap<(i32, bool), Vec<SeedProduction>> =
                std::collections::HashMap::new();
            let mut proc: std::collections::HashMap<(i32, bool), Vec<CropProcure>> =
                std::collections::HashMap::new();
            let mut skipped = 0;
            for r in &production {
                if world.data.manor.seed_by_id(r.seed_id).is_none() {
                    skipped += 1;
                    continue;
                }
                prod.entry((r.castle_id, r.next_period))
                    .or_default()
                    .push(SeedProduction {
                        seed_id: r.seed_id,
                        amount: r.amount,
                        price: r.price,
                        start_amount: r.start_amount,
                    });
            }
            for r in &procure {
                if world.data.manor.seed_by_crop(r.crop_id).is_none() {
                    skipped += 1;
                    continue;
                }
                proc.entry((r.castle_id, r.next_period))
                    .or_default()
                    .push(CropProcure {
                        crop_id: r.crop_id,
                        amount: r.amount,
                        price: r.price,
                        start_amount: r.start_amount,
                        reward_type: r.reward_type,
                    });
            }
            for ((castle_id, next), list) in prod {
                manor.set_seed_production(castle_id, next, list);
            }
            for ((castle_id, next), list) in proc {
                manor.set_crop_procure(castle_id, next, list);
            }
            tracing::info!(
                "GameLoop: loaded manor state ({} production, {} procure rows, {skipped} unknown skipped).",
                production.len(),
                procure.len()
            );
            world.manor = manor;
            // Set the initial period mode from the wall clock and arm the
            // first daily mode change (Java `CastleManorManager` init).
            crate::game_loop::manor::schedule_manor_at_boot(world);
        }
        DbEvent::ClanHallsLoaded { rows } => {
            // Start from the static defs, then overlay persisted ownership.
            let mut halls = world.data.clan_halls.clone();
            for row in &rows {
                if let Some(hall) = halls.get_mut(&row.id) {
                    hall.owner_id = row.owner_id;
                    hall.paid_until = row.paid_until;
                }
            }
            let owned: Vec<i32> = halls
                .values()
                .filter(|h| h.owner_id != 0)
                .map(|h| h.id)
                .collect();
            tracing::info!(
                "GameLoop: loaded {} clan halls ({} owned).",
                halls.len(),
                owned.len()
            );
            world.clan_halls = halls;
            // Java `ClanHall.setOwner` on load arms each owned hall's lease
            // check; restore those timers here.
            for hall_id in owned {
                crate::game_loop::clans::hall_auction::arm_lease_check(world, hall_id);
            }
        }
        DbEvent::ClanHallBiddersLoaded { rows } => {
            use crate::model::clan_hall::ClanHallBid;
            for row in &rows {
                world.clan_hall_bids.entry(row.hall_id).or_default().insert(
                    row.clan_id,
                    ClanHallBid {
                        amount: row.bid,
                        bid_time: row.bid_time,
                    },
                );
            }
            tracing::info!("GameLoop: loaded {} clan-hall auction bids.", rows.len());
            // Arm the weekly auction close now that the bids exist.
            crate::game_loop::clans::hall_auction::schedule_weekly_close(world);
        }
        DbEvent::ResidenceFunctionsLoaded { rows } => {
            use crate::model::clan_hall::ActiveFunction;
            for row in &rows {
                world
                    .clan_hall_functions
                    .entry(row.residence_id)
                    .or_default()
                    .insert(
                        row.func_id,
                        ActiveFunction {
                            level: row.level,
                            expiration: row.expiration,
                        },
                    );
            }
            tracing::info!("GameLoop: loaded {} clan-hall functions.", rows.len());
            // Re-arm each function's expiry (Java `ResidenceFunction.init`).
            let funcs: Vec<(i32, i32)> = world
                .clan_hall_functions
                .iter()
                .flat_map(|(&hall, fs)| fs.keys().map(move |&f| (hall, f)))
                .collect();
            for (hall_id, func_id) in funcs {
                crate::game_loop::clans::hall_function::arm_function_expiry(
                    world, hall_id, func_id,
                );
            }
        }
        DbEvent::CustomMailLoaded { rows } => {
            crate::game_loop::custom_mail::apply_loaded(world, rows);
        }
        DbEvent::OlympiadLoaded {
            current_cycle,
            period,
            olympiad_end,
            validation_end,
            next_weekly_change,
            nobles,
            eom,
        } => {
            crate::game_loop::olympiad::apply_loaded(
                world,
                current_cycle,
                period,
                olympiad_end,
                validation_end,
                next_weekly_change,
                nobles,
                eom,
            );
            // `Olympiad.init` + `scheduleWeeklyChange`: arm the window and
            // weekly-refresh schedules now the persisted state is in place.
            crate::game_loop::olympiad::schedule_at_boot(world);
        }
        DbEvent::HeroesLoaded { heroes, diary } => {
            crate::game_loop::olympiad::apply_heroes_loaded(world, heroes, diary);
        }
        DbEvent::SiegeGuardsLoaded { guards } => {
            let mut by_castle: std::collections::HashMap<
                i32,
                Vec<crate::model::siege::SiegeSpawn>,
            > = std::collections::HashMap::new();
            for (castle_id, spawn) in guards {
                by_castle.entry(castle_id).or_default().push(spawn);
            }
            let total: usize = by_castle.values().map(|v| v.len()).sum();
            tracing::info!(
                "GameLoop: loaded {total} siege guards for {} castles.",
                by_castle.len()
            );
            world.siege_guards = by_castle;
        }
        DbEvent::ClansLoaded {
            clans,
            wars,
            crests,
            recruit_clans,
            recruit_waiting,
            recruit_applicants,
            notices,
        } => {
            world.clan_notices = notices
                .into_iter()
                .map(|(id, enabled, text)| (id, (enabled, text)))
                .collect();
            tracing::info!(
                "GameLoop: loaded {} clans, {} clan wars, {} crests, {} recruiting clans, \
                 {} waiting players, {} applications.",
                clans.len(),
                wars.len(),
                crests.len(),
                recruit_clans.len(),
                recruit_waiting.len(),
                recruit_applicants.iter().len()
            );
            world.clans = clans.into_iter().map(|c| (c.id, c)).collect();
            world.clan_wars = wars;
            world.next_crest_id = crests.iter().map(|c| c.id + 1).max().unwrap_or(1);
            world.crests = crests.into_iter().map(|c| (c.id, c)).collect();
            // `ClanEntryManager.load`: drop recruiting entries for clans
            // that no longer exist.
            world.recruit_clans = recruit_clans
                .into_iter()
                .filter(|r| world.clans.contains_key(&r.clan_id))
                .map(|r| (r.clan_id, r))
                .collect();
            world.recruit_waiting = recruit_waiting
                .into_iter()
                .map(|w| (w.player_id, w))
                .collect();
            for a in recruit_applicants {
                world
                    .recruit_applicants
                    .entry(a.clan_id)
                    .or_default()
                    .insert(a.player_id, a);
            }
            crate::game_loop::clans::rearm_clan_wars_at_boot(world);
            // Re-arm pending dissolutions (Java `ClanTable`'s constructor:
            // past-due stamps fire immediately).
            let pending: Vec<(i32, i64)> = world
                .clans
                .values()
                .filter(|c| c.dissolving_expiry_time > 0)
                .map(|c| (c.id, c.dissolving_expiry_time))
                .collect();
            for (clan_id, due) in pending {
                crate::game_loop::clans::schedule_clan_dissolve(world, clan_id, due);
            }
            // Clans are the last boot-load data (static datapack already
            // loaded synchronously at startup); release the login-link task
            // to connect now that the world is fully populated.
            if let Some(ready) = world.login.ready.take() {
                let _ = ready.send(());
            }
        }
    }
}
