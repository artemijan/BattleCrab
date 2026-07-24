//! Channel drains and session lifecycle: network connect/disconnect events,
//! the login-link and DB result channels, and restart/logout/kick handling.

use tracing::{debug, error, info, warn};

use crate::db::{self, DbEvent, DbEventRx};
use crate::geo::worker::PathEventRx;
use crate::loginlink::{LoginLinkCommand, LoginLinkEvent, LoginLinkEventRx};
use crate::network::{server_packets, NetEvent, NetEventRx};
use crate::session::{ClientSession, Session};
use crate::world::World;

use super::dispatch::on_packet;

/// Bounded, non-blocking drain of the network→game channel (step 1 of the tick).
pub(crate) fn drain_network(world: &mut World, net_rx: &NetEventRx) {
    while let Ok(event) = net_rx.try_recv() {
        match event {
            NetEvent::Connected {
                client_id,
                out,
                addr,
            } => {
                world.clients.insert(
                    client_id,
                    ClientSession::Connecting(Session::new(client_id, out, addr)),
                );
                debug!(
                    "GameLoop: client {client_id} connected from {addr} ({} online).",
                    world.clients.len()
                );
            }
            NetEvent::Received { client_id, data } => {
                // Java `ExecuteThread`/`PacketHandler` catches Throwable around
                // each packet's run(), so one bad packet (an admin command with
                // missing args, a malformed bypass…) must not take the whole
                // game thread down. `World` is a single-thread structure with
                // no lock poisoning to worry about, but the handler may have
                // died mid-mutation, so the offending client's session state is
                // suspect: disconnect them (persist + clean removal) so they
                // come back clean while everyone else plays on.
                let opcode = data.first().copied();
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    on_packet(world, client_id, data);
                }))
                .is_err()
                {
                    error!(
                        "GameLoop: panic while handling packet {:#04x?} from client {client_id}; disconnecting that client.",
                        opcode.unwrap_or(0)
                    );
                    // If the save path trips over the same corrupted state,
                    // fall back to dropping the raw session (closes the
                    // socket, skips the store).
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        on_disconnect(world, client_id);
                    }))
                    .is_err()
                    {
                        error!(
                            "GameLoop: panic in the disconnect path for client {client_id}; dropping the session unsaved."
                        );
                        world.clients.remove(&client_id);
                    }
                }
            }
            NetEvent::Disconnected { client_id } => {
                on_disconnect(world, client_id);
            }
        }
    }
}

/// Take the player out of the world and persist them — Java
/// `Disconnection.storeMe().deleteMe()`. Shared by restart, logout, and
/// unexpected disconnects. Scheduled tasks holding the dead object id no-op.
pub(crate) fn store_and_remove_player(world: &mut World, player_object_id: i32) {
    // deleteMe → leaveParty (DISCONNECTED semantics: leadership transfers)
    // + pending party/friend request cleanup on both sides.
    super::party::on_player_leave_world(world, player_object_id);
    // deleteMe → notifyFriends(MODE_OFFLINE).
    super::friends::on_leave_world(world, player_object_id);
    // A servitor does not outlive its owner's session. Java stores it in
    // `CharSummonTable` for `RestoreServitorOnReconnect`; persistence is a
    // later slice, so for now it simply goes away with them — which is at
    // least better than leaking an ownerless NPC into the world.
    super::servitor::on_owner_leave_world(world, player_object_id);
    // Cubics do not outlive their owner; nothing persists them.
    super::cubic::on_owner_leave_world(world, player_object_id);
    // deleteMe → clan.broadcastToOnlineMembers(PledgeShowMemberListUpdate offline).
    {
        let clan_id = world
            .objects
            .get_component::<crate::model::Player>(&player_object_id)
            .map(|p| p.clan_id)
            .unwrap_or(0);
        super::clans::on_leave_world(world, player_object_id, clan_id);
    }
    // deleteMe → World.removeVisibleObject: DeleteObject to everyone watching.
    super::visibility::on_leave_world(world, player_object_id);
    // Stop tracking the player for the periodic autosave; the logout flush below
    // is the final save.
    world.player_autosave_due.remove(&player_object_id);
    // Gather everything persistence needs before despawn — components drop
    // with the entity (PLAN_ECS_STAGE2 §7 risk 3).
    if let Some(save) = build_save_data(world, player_object_id) {
        world.objects.despawn(&player_object_id);
        let _ = world.db.send(db::DbCommand::StorePlayer { save });
    }
}

/// Gather a player's full persistable state into a [`db::PlayerSaveData`] for a
/// flush — the char row plus every in-memory child collection (inventory,
/// skills, shortcuts, macros, quests). `None` when the core components are
/// missing (not a live player); absent child collections default to empty. This
/// is the single gather point for all four flush triggers: the periodic
/// autosave, logout, class-transfer, and shutdown save-all. Because gameplay
/// only mutates these components (never the DB directly), one flush captures
/// everything the player did since the last one.
pub(crate) fn build_save_data(world: &World, object_id: i32) -> Option<db::PlayerSaveData> {
    use crate::model::components::{
        Macros, PlayerVitals, Position, Quests, Shortcuts, SkillBook, Vitals,
    };
    use crate::model::inventory::Inventory;

    let p = world
        .objects
        .get_component::<crate::model::Player>(&object_id)?;
    let pos = world.objects.get_component::<Position>(&object_id)?;
    let vitals = world.objects.get_component::<Vitals>(&object_id)?;
    let pvitals = world.objects.get_component::<PlayerVitals>(&object_id)?;
    let base = db::PlayerSnapshot::of(p, pos, vitals, pvitals);

    // The whole persisted item set = inventory + warehouse + freight (the save
    // deletes any `items` row not present, so every container must be included).
    let mut items = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map(Inventory::to_rows)
        .unwrap_or_default();
    if let Some(wh) = world
        .objects
        .get_component::<crate::model::inventory::Warehouse>(&object_id)
    {
        items.extend(wh.to_rows());
    }
    if let Some(fr) = world
        .objects
        .get_component::<crate::model::inventory::Freight>(&object_id)
    {
        items.extend(fr.to_rows());
    }
    // Pet-held items persist against the player (Java `PetInventory.getOwnerId`
    // returns the *owner's* id), so they join the same reconciled set.
    if let Some(pi) = world
        .objects
        .get_component::<crate::model::inventory::PetInventory>(&object_id)
    {
        items.extend(pi.to_rows());
    }
    let skill_enchants = world
        .objects
        .get_component::<crate::model::components::SkillEnchants>(&object_id)
        .map(|e| e.0.clone())
        .unwrap_or_default();
    let skills = world
        .objects
        .get_component::<SkillBook>(&object_id)
        .map(|s| {
            s.0.iter()
                .map(|(id, lvl)| (*id, *lvl, skill_enchants.get(id).copied().unwrap_or(0)))
                .collect()
        })
        .unwrap_or_default();
    let shortcuts = world
        .objects
        .get_component::<Shortcuts>(&object_id)
        .map(|s| s.0.values().cloned().collect())
        .unwrap_or_default();
    let macros = world
        .objects
        .get_component::<Macros>(&object_id)
        .map(|m| m.entries.clone())
        .unwrap_or_default();
    let quests = world
        .objects
        .get_component::<Quests>(&object_id)
        .map(|q| q.0.clone())
        .unwrap_or_default();

    let skill_reuses = reuses_to_save(
        world,
        world
            .objects
            .get_component::<crate::model::components::Reuses>(&object_id),
    );
    let skill_buffs = buffs_to_save(
        world,
        world
            .objects
            .get_component::<crate::model::components::Buffs>(&object_id),
    );

    let hennas = world
        .objects
        .get_component::<crate::model::components::HennaSlots>(&object_id)
        .map(henna_rows)
        .unwrap_or_default();

    // Registered recipes as (list_id, is_dwarven) — the component already keeps
    // the two books split, so the flag is known without a RecipeData lookup.
    let recipe_book = world
        .objects
        .get_component::<crate::model::components::RecipeBook>(&object_id)
        .map(|rb| {
            rb.dwarven
                .iter()
                .map(|&id| (id, true))
                .chain(rb.common.iter().map(|&id| (id, false)))
                .collect()
        })
        .unwrap_or_default();

    // A live servitor's row is captured the same way the pet's is: by the
    // caller, before the summon leaves the world.
    let summons = world
        .objects
        .get_component::<crate::model::components::PlayerSummons>(&object_id)
        .map(|s| s.0.clone())
        .unwrap_or_default();

    // `PlayerPets` is expected to already carry the live pet's state: callers
    // run `servitor::sync_pet_row` first (it needs `&mut World` for the store
    // sweep, which this read-only builder does not have).
    let pets = world
        .objects
        .get_component::<crate::model::components::PlayerPets>(&object_id)
        .map(|p| p.0.values().cloned().collect())
        .unwrap_or_default();

    // `PlayerVariables.storeMe` — the whole map, flushed with the character.
    let variables = world
        .objects
        .get_component::<crate::model::components::PlayerVariables>(&object_id)
        .map(|v| {
            v.0.iter()
                .map(|(k, val)| (k.clone(), val.clone()))
                .collect()
        })
        .unwrap_or_default();

    let (skills_by_index, hennas_by_index, shortcuts_by_index, class_index) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .map(|p| {
            (
                p.skills_by_index.clone(),
                p.hennas_by_index.clone(),
                p.shortcuts_by_index.clone(),
                p.class_index,
            )
        })
        .unwrap_or_default();

    Some(db::PlayerSaveData {
        base,
        pets,
        summons,
        items,
        skills,
        skills_by_index,
        hennas_by_index,
        shortcuts_by_index,
        class_index,
        hennas,
        recipe_book,
        variables,
        shortcuts,
        macros,
        quests,
        skill_reuses,
        skill_buffs,
    })
}

/// Worn henna dyes → `character_hennas` rows as `(slot 1-3, dye_id)`.
fn henna_rows(henna: &crate::model::components::HennaSlots) -> Vec<(i32, i32)> {
    henna
        .0
        .iter()
        .enumerate()
        .filter_map(|(i, dye)| dye.map(|id| (i as i32 + 1, id)))
        .collect()
}

/// Skill reuse cooldowns → `character_skills_save` rows (Java `storeEffect`,
/// reuse half), gated by `StoreSkillCooltime`. `until_tick` is server-uptime
/// relative, so persist an absolute wall-clock end time that survives a
/// relog/restart; only cooldowns with time still left are written. Empty (which
/// clears the DB rows on flush) when the config is off or there's no map.
fn reuses_to_save(
    world: &World,
    reuses: Option<&crate::model::components::Reuses>,
) -> Vec<db::SkillReuseRow> {
    let Some(reuses) = reuses.filter(|_| world.cfg.character.store_skill_cooltime) else {
        return Vec::new();
    };
    let now_tick = world.tick;
    let now_ms = commons::util::now_millis();
    reuses
        .0
        .iter()
        .filter_map(|(&reuse_key, sr)| {
            let remaining_ticks = sr.until_tick.saturating_sub(now_tick);
            (remaining_ticks > 0).then_some(db::SkillReuseRow {
                reuse_key,
                skill_level: sr.skill_level,
                reuse_delay: sr.total_ms,
                systime_ms: now_ms + remaining_ticks as i64 * 100,
            })
        })
        .collect()
}

/// Active buffs → `character_skills_save` rows (Java `storeEffect`, buff half),
/// gated by `StoreSkillCooltime` like the reuse half.
///
/// Stores the **remaining seconds**, not an end instant: a buff's countdown is
/// frozen while the character is offline (Java's `restoreEffects` hands this
/// value straight to `applyEffects` as a custom `abnormalTime`), unlike a
/// cooldown, which keeps decaying. Java's skip list is reproduced here:
///
/// * dances/songs, unless `AltStoreDances` — not kept in retail;
/// * toggles (Java `isToggle() && !isNecessaryToggle()`) — modelled here as
///   buffs with no expiry, which is also what a 0-`abnormalTime` skill looks
///   like; neither should come back on its own after a relog;
/// * `LIFE_FORCE_OTHERS` — Java refuses to persist heal-over-time herbs;
/// * one row per skill id, first occurrence wins (Java dedupes on
///   `getReuseHashCode()`).
///
/// Passive stand-in entries (the grade-penalty pumps) are skipped too: they
/// carry no real buff, and enter-world re-derives them via
/// `refresh_expertise_penalty` — persisting them would double-apply the pump.
///
/// TODO(G22): Java also skips `isDeleteAbnormalOnLeave()` skills; the flag
/// isn't parsed into `Skill` yet, so such a buff currently survives a relog it
/// shouldn't.
fn buffs_to_save(
    world: &World,
    buffs: Option<&crate::model::components::Buffs>,
) -> Vec<db::SkillBuffRow> {
    use crate::model::skill::BuffSlot;
    let Some(buffs) = buffs.filter(|_| world.cfg.character.store_skill_cooltime) else {
        return Vec::new();
    };
    let now_tick = world.tick;
    let mut seen = std::collections::HashSet::new();
    buffs
        .0
        .iter()
        .filter(|b| !b.passive)
        .filter(|b| b.slot != BuffSlot::Dance || world.cfg.character.alt_store_dances)
        .filter(|b| b.abnormal_type != "LIFE_FORCE_OTHERS")
        .filter_map(|b| {
            // `u64::MAX` is the no-expiry sentinel (toggle / 0-`abnormalTime`);
            // `saturating_sub` keeps it enormous, and the `> 0` seconds check
            // below can't reject it, so screen it out explicitly.
            if b.expires_at_tick == u64::MAX {
                return None;
            }
            let remaining_time_secs = (b.expires_at_tick.saturating_sub(now_tick) / 10) as i32;
            if remaining_time_secs <= 0 || !seen.insert(b.skill_id) {
                return None;
            }
            Some(db::SkillBuffRow {
                skill_id: b.skill_id,
                skill_level: b.skill_level,
                remaining_time_secs,
            })
        })
        .collect()
}

/// Flush a player who stays in the world — the periodic autosave and changes
/// that shouldn't wait for logout (class transfers).
pub(crate) fn store_player_now(world: &mut World, player_object_id: i32) {
    // Fold the live pet's state into `PlayerPets` before the snapshot, or the
    // autosave persists the row as it was at summon time and discards
    // everything the pet did this session.
    crate::game_loop::servitor::sync_pet_row(world, player_object_id);
    if let Some(save) = build_save_data(world, player_object_id) {
        let _ = world.db.send(db::DbCommand::StorePlayer { save });
    }
}

/// Server-shutdown save-all (Java `Shutdown` → `GameServer` disconnect-all →
/// `Disconnection.storeMe()` for every online player). In the memory-first model
/// all character state (level/exp/position/vitals, items, skills, shortcuts,
/// macros, quests) lives only in memory between the periodic autosave flushes,
/// so this final full flush is what keeps a restart from reverting everyone to
/// their last autosave/logout. Runs once after the game loop stops; the DB
/// thread drains these before it's told to shut down (`main` sends
/// `DbCommand::Shutdown` only after this thread joins).
pub(crate) fn save_all_players(world: &mut World) {
    let mut ids = Vec::new();
    world
        .objects
        .for_each_mut::<&crate::model::Player>(|p| ids.push(p.object_id));
    let count = ids.len();
    for oid in ids {
        store_player_now(world, oid);
    }
    if count > 0 {
        info!("GameLoop: saved {count} online player(s) on shutdown.");
    }
}

/// Port of `Player.canLogout`: refuse a restart/logout while the player is
/// fighting. Java also blocks on a pending item request, a subclass-change lock,
/// and event registration — none of those systems are ported yet, so combat
/// stance (`AttackStanceTaskManager.hasAttackStanceTask`) is the only guard.
fn can_logout(world: &World, object_id: i32) -> bool {
    !super::combat::has_attack_stance(world, object_id)
}

/// Port of `clientpackets/RequestRestart.runImpl`: save + leave the world, drop
/// the session back to the character-selection lifecycle, and re-send the
/// character list. Olympiad/instance handling doesn't apply yet.
pub(crate) fn handle_request_restart(world: &mut World, client_id: u32) {
    let Some(ClientSession::InGame(s)) = world.clients.get(&client_id) else {
        return; // Java gates by IN_GAME
    };
    // `!canLogout()` → RestartResponse.FALSE + ActionFailed, keep the player in.
    if !can_logout(world, s.player_object_id()) {
        s.send(server_packets::restart_response(false));
        s.send(server_packets::action_failed());
        return;
    }
    let Some(ClientSession::InGame(s)) = world.clients.remove(&client_id) else {
        unreachable!("checked above");
    };
    store_and_remove_player(world, s.player_object_id());
    info!(
        "GameLoop: '{}' logged out to character selection.",
        s.account()
    );

    // Java: setConnectionState(AUTHENTICATED) + RestartResponse.TRUE, then a
    // freshly restored CharSelectionInfo. The reload arrives through the normal
    // Authenticated → InLobby path (`on_characters_loaded`, send_list=true) and
    // is ordered after the StorePlayer above on the DB channel.
    let s = s.into_authenticated();
    s.send(server_packets::restart_response(true));
    let account = s.account().to_string();
    world
        .clients
        .insert(client_id, ClientSession::Authenticated(s));
    let _ = world
        .db
        .send(db::DbCommand::LoadCharacters { client_id, account });
}

/// Port of `clientpackets/Logout.runImpl`: save + leave the world, acknowledge
/// with `LeaveWorld`, and close. Valid from the lobby too (Java gates by
/// AUTHENTICATED + IN_GAME), where it just disconnects. In-game, `canLogout`
/// gates it the same way as `handle_request_restart`.
pub(crate) fn handle_logout(world: &mut World, client_id: u32) {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => {
            // `!canLogout()` → just ActionFailed, no LeaveWorld, stay in-game.
            if !can_logout(world, s.player_object_id()) {
                s.send(server_packets::action_failed());
                return;
            }
            let Some(ClientSession::InGame(s)) = world.clients.remove(&client_id) else {
                unreachable!("checked above");
            };
            store_and_remove_player(world, s.player_object_id());
            info!("GameLoop: '{}' logged out.", s.account());
            // Dropping the session closes the socket after the queued packet
            // is flushed; the resulting `Disconnected` event runs the login
            // notify in `on_disconnect`.
            s.send(server_packets::leave_world());
        }
        Some(_) => {
            // No player: Java `client.disconnect()`.
            world.clients.remove(&client_id);
        }
        None => {}
    }
}

/// Clean up a disconnected client and inform the login server.
pub(crate) fn on_disconnect(world: &mut World, client_id: u32) {
    // Unexpected disconnect while a character is loaded: persist it (Java
    // `GameClient.onDisconnection` → `Disconnection.storeMe().deleteMe()`).
    // In `Entering` the Player is still held by the session, not the world.
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => {
            store_and_remove_player(world, s.player_object_id());
        }
        Some(ClientSession::Entering(s)) => {
            // The Player is still held by the session, not the world store, so
            // build the full save straight from the loaded `PlayerData`. It must
            // carry every child collection: `store_player` reconciles them, so an
            // items/skills-empty save here would wipe the just-loaded character.
            let b = s.player();
            let _ = world.db.send(db::DbCommand::StorePlayer {
                save: db::PlayerSaveData {
                    base: db::PlayerSnapshot::of(
                        &b.player,
                        &b.position,
                        &b.vitals,
                        &b.player_vitals,
                    ),
                    // No summon can exist before entering the world, so the
                    // rows loaded at login are still current — but they must be
                    // written back, since `store_player` reconciles.
                    pets: b.pets.0.values().cloned().collect(),
                    summons: b.summons.0.clone(),
                    items: b
                        .inventory
                        .to_rows()
                        .into_iter()
                        .chain(b.warehouse.to_rows())
                        .chain(b.freight.to_rows())
                        .chain(b.pet_inventory.to_rows())
                        .collect(),
                    skills: b
                        .skills
                        .0
                        .iter()
                        .map(|(id, lvl)| {
                            (*id, *lvl, b.skill_enchants.0.get(id).copied().unwrap_or(0))
                        })
                        .collect(),
                    skills_by_index: Default::default(),
                    hennas_by_index: Default::default(),
                    shortcuts_by_index: Default::default(),
                    class_index: 0,
                    hennas: henna_rows(&b.henna),
                    recipe_book: b
                        .recipe_book
                        .dwarven
                        .iter()
                        .map(|&id| (id, true))
                        .chain(b.recipe_book.common.iter().map(|&id| (id, false)))
                        .collect(),
                    variables: b
                        .variables
                        .0
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                    shortcuts: b.shortcuts.0.values().cloned().collect(),
                    macros: b.macros.entries.clone(),
                    quests: b.quests.0.clone(),
                    skill_reuses: reuses_to_save(world, Some(&b.reuses)),
                    // This character never spawned, so its buffs were never
                    // applied — they're still the untouched rows the select path
                    // loaded. Write them straight back: running them through
                    // `buffs_to_save` (which reads the empty live `Buffs`
                    // component) would silently drop every buff of anyone who
                    // disconnects between char-select and enter-world.
                    skill_buffs: b.pending_buffs.clone(),
                },
            });
        }
        _ => {}
    }
    world.clients.remove(&client_id);
    let account = world
        .login
        .accounts_in_gameserver
        .iter()
        .find(|(_, &id)| id == client_id)
        .map(|(a, _)| a.clone());
    if let Some(account) = account {
        world.login.accounts_in_gameserver.remove(&account);
        world.login.waiting.remove(&account);
        let _ = world
            .login
            .link
            .send(LoginLinkCommand::PlayerLogout { account });
    }
    debug!(
        "GameLoop: client {client_id} disconnected ({} online).",
        world.clients.len()
    );
}

/// Bounded, non-blocking drain of the login-link→game channel (step 2).
pub(crate) fn drain_login_link(world: &mut World, login_rx: &LoginLinkEventRx) {
    while let Ok(event) = login_rx.try_recv() {
        match event {
            LoginLinkEvent::Registered {
                server_id,
                server_name,
            } => {
                info!("GameLoop: registered as Server {server_id}: {server_name}.");
                world.login.server_id = Some(server_id);
                world.login.server_name = Some(server_name);
            }
            LoginLinkEvent::PlayerAuthResponse { account, authed } => {
                handle_player_auth_response(world, account, authed);
            }
            LoginLinkEvent::KickPlayer { account } => handle_kick(world, account),
            LoginLinkEvent::RequestCharacters { account } => {
                // Ask the DB thread; reply on the CharCount event.
                let _ = world.db.send(db::DbCommand::CountCharacters { account });
            }
            LoginLinkEvent::Failed { reason } => {
                warn!("GameLoop: login-server registration failed (reason {reason}).");
            }
        }
    }
}

/// Port of the `PlayerAuthResponse` (0x03) branch of `LoginServerThread.run`.
pub(crate) fn handle_player_auth_response(world: &mut World, account: String, authed: bool) {
    let Some(waiting) = world.login.waiting.remove(&account) else {
        return;
    };
    let client_id = waiting.client_id;
    if authed {
        let _ = world.login.link.send(LoginLinkCommand::PlayerInGame {
            accounts: vec![account.clone()],
        });
        if let Some(ClientSession::Connecting(s)) = world.clients.remove(&client_id) {
            let s = s.into_authenticated(account.clone(), waiting.session_key);
            s.send(server_packets::login_success());
            info!(
                "GameLoop: client {} authenticated as '{}'.",
                s.client_id,
                s.account()
            );
            world
                .clients
                .insert(client_id, ClientSession::Authenticated(s));
            // Load the character list; CharSelectionInfo is sent on the result.
            let _ = world
                .db
                .send(db::DbCommand::LoadCharacters { client_id, account });
        }
    } else {
        warn!("GameLoop: session key incorrect, closing connection for account {account}.");
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::login_fail(0, 1)); // SYSTEM_ERROR_LOGIN_LATER
        }
        world.login.accounts_in_gameserver.remove(&account);
        world.clients.remove(&client_id); // disconnect after the queued packet
        let _ = world
            .login
            .link
            .send(LoginLinkCommand::PlayerLogout { account });
    }
}

/// Bounded, non-blocking drain of the DB→game channel (step 2).
/// Apply every path-worker reply that landed since the last tick.
pub(crate) fn drain_path(world: &mut World, path_rx: &PathEventRx) {
    while let Ok(event) = path_rx.try_recv() {
        super::position::handle_path_result(world, event);
    }
}

pub(crate) fn drain_db(world: &mut World, db_rx: &DbEventRx) {
    while let Ok(event) = db_rx.try_recv() {
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
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(body);
                }
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
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(server_packets::ex_is_char_name_creatable(result));
                }
            }
            DbEvent::IdBlock { start, end } => {
                world.id_pool = start..end;
            }
            DbEvent::PremiumLoaded { entries } => {
                tracing::info!("GameLoop: loaded {} premium accounts.", entries.len());
                world.premium = entries.into_iter().collect();
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
                    world.bbs_favorites.entry(player_id).or_default().push(
                        crate::world::Favorite {
                            fav_id,
                            title,
                            bypass,
                            add_date,
                        },
                    );
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
                super::boss_respawn::resolve_boot(world, rows);
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
                        // Java `reActivate()`; the decay/expiry task is deferred (G21).
                        cw.is_activated = true;
                    }
                }
                tracing::info!("GameLoop: loaded {} cursed weapons.", weapons.len());
                world.cursed_weapons = weapons;
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
            DbEvent::OlympiadLoaded {
                current_cycle,
                period,
                olympiad_end,
                validation_end,
                next_weekly_change,
                nobles,
            } => {
                crate::game_loop::olympiad::apply_loaded(
                    world,
                    current_cycle,
                    period,
                    olympiad_end,
                    validation_end,
                    next_weekly_change,
                    nobles,
                );
                // `Olympiad.init` + `scheduleWeeklyChange`: arm the window and
                // weekly-refresh schedules now the persisted state is in place.
                crate::game_loop::olympiad::schedule_at_boot(world);
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
            } => {
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
                super::clans::rearm_clan_wars_at_boot(world);
                // Re-arm pending dissolutions (Java `ClanTable`'s constructor:
                // past-due stamps fire immediately).
                let pending: Vec<(i32, i64)> = world
                    .clans
                    .values()
                    .filter(|c| c.dissolving_expiry_time > 0)
                    .map(|c| (c.id, c.dissolving_expiry_time))
                    .collect();
                for (clan_id, due) in pending {
                    super::clans::schedule_clan_dissolve(world, clan_id, due);
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
}

/// A character list came back from the DB. Always cache it on the session (for
/// slot → object-id mapping); send `CharSelectionInfo` only when `send_list`
/// (login/delete/restore) — after creation Java caches without re-sending.
/// Transitions `Authenticated` → `InLobby` on the first load.
pub(crate) fn on_characters_loaded(
    world: &mut World,
    client_id: u32,
    account: String,
    chars: Vec<crate::character::CharData>,
    send_list: bool,
) {
    let s = match world.clients.remove(&client_id) {
        Some(ClientSession::Authenticated(s)) => s.into_lobby(chars),
        Some(ClientSession::InLobby(mut s)) => {
            s.set_chars(chars);
            s
        }
        other => {
            // Client vanished mid-load; put back whatever was there.
            if let Some(cs) = other {
                world.clients.insert(client_id, cs);
            }
            return;
        }
    };
    if send_list {
        let body = server_packets::char_selection_info(
            &account,
            s.play_ok1(),
            &s.state.chars,
            -1,
            world.max_characters_per_account,
            &world.data.experience,
        );
        s.send(body);
    }
    world.clients.insert(client_id, ClientSession::InLobby(s));
}

/// Port of `doKickPlayer`: disconnect the account's client and notify login.
pub(crate) fn handle_kick(world: &mut World, account: String) {
    if let Some(&client_id) = world.login.accounts_in_gameserver.get(&account) {
        world.clients.remove(&client_id); // disconnect
    }
    world.login.accounts_in_gameserver.remove(&account);
    world.login.waiting.remove(&account);
    let _ = world
        .login
        .link
        .send(LoginLinkCommand::PlayerLogout { account });
}
