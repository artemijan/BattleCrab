//! Channel drains and session lifecycle: network connect/disconnect events,
//! the login-link and DB result channels, and restart/logout/kick handling.

use tracing::{debug, info, warn};

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
                on_packet(world, client_id, data);
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
    use crate::model::components::{Macros, PlayerVitals, Position, Quests, Shortcuts, SkillBook, Vitals};
    use crate::model::inventory::Inventory;

    let p = world.objects.get_component::<crate::model::Player>(&object_id)?;
    let pos = world.objects.get_component::<Position>(&object_id)?;
    let vitals = world.objects.get_component::<Vitals>(&object_id)?;
    let pvitals = world.objects.get_component::<PlayerVitals>(&object_id)?;
    let base = db::PlayerSnapshot::of(p, pos, vitals, pvitals);

    let items = world.objects.get_component::<Inventory>(&object_id).map(Inventory::to_rows).unwrap_or_default();
    let skills = world
        .objects
        .get_component::<SkillBook>(&object_id)
        .map(|s| s.0.iter().map(|(id, lvl)| (*id, *lvl)).collect())
        .unwrap_or_default();
    let shortcuts = world
        .objects
        .get_component::<Shortcuts>(&object_id)
        .map(|s| s.0.values().cloned().collect())
        .unwrap_or_default();
    let macros = world.objects.get_component::<Macros>(&object_id).map(|m| m.entries.clone()).unwrap_or_default();
    let quests = world.objects.get_component::<Quests>(&object_id).map(|q| q.0.clone()).unwrap_or_default();

    Some(db::PlayerSaveData { base, items, skills, shortcuts, macros, quests })
}

/// Flush a player who stays in the world — the periodic autosave and changes
/// that shouldn't wait for logout (class transfers).
pub(crate) fn store_player_now(world: &mut World, player_object_id: i32) {
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
    world.objects.for_each_mut::<&crate::model::Player>(|p| ids.push(p.object_id));
    let count = ids.len();
    for oid in ids {
        store_player_now(world, oid);
    }
    if count > 0 {
        info!("GameLoop: saved {count} online player(s) on shutdown.");
    }
}

/// Port of `clientpackets/RequestRestart.runImpl`: save + leave the world, drop
/// the session back to the character-selection lifecycle, and re-send the
/// character list. Olympiad/instance handling doesn't apply yet; `canLogout`
/// guards (attack stance, NO_RESTART zones, events) are TODO with combat (G9).
pub(crate) fn handle_request_restart(world: &mut World, client_id: u32) {
    let Some(ClientSession::InGame(_)) = world.clients.get(&client_id) else {
        return; // Java gates by IN_GAME
    };
    let Some(ClientSession::InGame(s)) = world.clients.remove(&client_id) else {
        unreachable!("checked above");
    };
    store_and_remove_player(world, s.player_object_id());
    info!("GameLoop: '{}' logged out to character selection.", s.account());

    // Java: setConnectionState(AUTHENTICATED) + RestartResponse.TRUE, then a
    // freshly restored CharSelectionInfo. The reload arrives through the normal
    // Authenticated → InLobby path (`on_characters_loaded`, send_list=true) and
    // is ordered after the StorePlayer above on the DB channel.
    let s = s.into_authenticated();
    s.send(server_packets::restart_response(true));
    let account = s.account().to_string();
    world.clients.insert(client_id, ClientSession::Authenticated(s));
    let _ = world.db.send(db::DbCommand::LoadCharacters { client_id, account });
}

/// Port of `clientpackets/Logout.runImpl`: save + leave the world, acknowledge
/// with `LeaveWorld`, and close. Valid from the lobby too (Java gates by
/// AUTHENTICATED + IN_GAME), where it just disconnects. `canLogout` guards are
/// TODO with combat (G9), same as `handle_request_restart`.
pub(crate) fn handle_logout(world: &mut World, client_id: u32) {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(_)) => {
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
                    base: db::PlayerSnapshot::of(&b.player, &b.position, &b.vitals, &b.player_vitals),
                    items: b.inventory.to_rows(),
                    skills: b.skills.0.iter().map(|(id, lvl)| (*id, *lvl)).collect(),
                    shortcuts: b.shortcuts.0.values().cloned().collect(),
                    macros: b.macros.entries.clone(),
                    quests: b.quests.0.clone(),
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
            DbEvent::ClansLoaded { clans } => {
                tracing::info!("GameLoop: loaded {} clans.", clans.len());
                world.clans = clans.into_iter().map(|c| (c.id, c)).collect();
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
