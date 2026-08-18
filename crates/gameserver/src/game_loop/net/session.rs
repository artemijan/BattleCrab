//! The session lifecycle: network events (connect/receive/disconnect),
//! restart/logout, the login-link handshake, character-list load and kick.

use super::henna_rows;
use super::packets_handled;
use super::players_online;
use super::reuses_to_save;
use super::store_and_remove_player;
use crate::db;
use crate::game_loop::dispatch::on_packet;
use crate::game_loop::helpers::send_to_client;
use crate::loginlink::LoginLinkCommand;
use crate::loginlink::LoginLinkEvent;
use crate::network::NetEvent;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::session::Session;
use crate::world::World;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;
/// One network event: connect, inbound packet (dispatched under the
/// per-packet panic guard), or disconnect.
pub(crate) fn handle_net_event(world: &mut World, event: NetEvent) {
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
        // `_permit` is the connection's in-flight slot: holding it to the end
        // of this arm keeps the packet "in flight" until it is fully handled.
        NetEvent::Received {
            client_id,
            data,
            permit: _permit,
        } => {
            // Java `ExecuteThread`/`PacketHandler` catches Throwable around
            // each packet's run(), so one bad packet (an admin command with
            // missing args, a malformed bypass…) must not take the whole
            // game thread down. `World` is a single-thread structure with
            // no lock poisoning to worry about, but the handler may have
            // died mid-mutation, so the offending client's session state is
            // suspect: disconnect them (persist + clean removal) so they
            // come back clean while everyone else plays on.
            let opcode = data.first().copied();
            packets_handled().incr();
            // Correlation span: every log line emitted while handling this
            // packet inherits these fields, which turns "what happened to
            // this player" into one query over the JSON log instead of a
            // manual reconstruction from interleaved lines.
            //
            // Deliberately allocation-free. This is the game thread and the
            // span is built per packet, so the fields are `i32`s only — a
            // `char_name` here would mean a `String` clone per packet. The
            // name lives on the audit records, which carry it already, and
            // `oid` is the join key between the two.
            let span = tracing::info_span!(
                "packet",
                client_id,
                oid = world.player_oid(client_id),
                opcode = opcode
            );
            let _entered = span.enter();
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
        NetEvent::ProtocolVersion { client_id, version } => {
            world.protocol_versions.insert(client_id, version);
        }
        NetEvent::Disconnected { client_id } => {
            on_disconnect(world, client_id);
        }
    }
    players_online().set(world.clients.len() as u64);
}

/// Port of `Player.canLogout`: refuse a restart/logout while the player is
/// fighting. Java also blocks on a pending item request, a subclass-change lock,
/// and event registration — none of those systems are ported yet, so combat
/// stance (`AttackStanceTaskManager.hasAttackStanceTask`) is the only guard.
///
/// `GMRestartFighting` (**True** here, and Java's own default) is the exemption:
/// `!(isGM() && Config.GM_RESTART_FIGHTING)`. It reads the access level rather
/// than a condition override — Java does too at this site, which is why it is
/// the one member of the GM-restriction family that is not about
/// `PlayerCondOverride`.
fn can_logout(world: &World, object_id: i32) -> bool {
    if !crate::game_loop::combat::has_attack_stance(world, object_id) {
        return true;
    }
    world.cfg.general.gm_restart_fighting && crate::game_loop::helpers::is_gm(world, object_id)
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
    // Java: `if (!enteredOfflineMode(player)) { storeMe().deleteMe(); }` — a
    // player with a store open stays behind as an unattended shop. Java then
    // still writes RestartResponse/CharSelectionInfo to the now-closed client;
    // the port simply stops here, which is the same observable outcome.
    if crate::game_loop::offline_trade::enter_offline_mode(world, client_id) {
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
            // Java `Logout`: a player with a store open becomes an offline
            // shop instead of leaving the world.
            if crate::game_loop::offline_trade::enter_offline_mode(world, client_id) {
                return;
            }
            let Some(ClientSession::InGame(s)) = world.clients.remove(&client_id) else {
                unreachable!("checked above");
            };
            // TvT: drop a logging-out participant + forfeit if a team emptied
            // (Java's `onPlayerLogout` listener). No-op off-event.
            crate::game_loop::events::tvt::on_player_logout(world, s.player_object_id());
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
    // Flush any item losses this player noted before the session is torn down —
    // after removal the inventory is gone and the per-tick drain would never
    // see them. The last thing someone does before vanishing is exactly what an
    // audit gets asked about.
    crate::game_loop::items::drain_item_audit(world);

    // Java `GameClient.onDisconnection` → the `accounting` logger. Recorded
    // first, while the account and character are still reachable through the
    // session. Ungated on purpose: unlike chat and items, Java has no config
    // switch for accounting — who connected, and when, is always kept.
    {
        let (account, char_name) = match world.clients.get(&client_id) {
            Some(ClientSession::InGame(s)) => (
                Some(s.account().to_string()),
                world
                    .objects
                    .get_component::<crate::model::Player>(&s.player_object_id())
                    .map(|p| p.name.clone()),
            ),
            Some(ClientSession::Entering(s)) => (Some(s.account().to_string()), None),
            _ => (None, None),
        };
        commons::audit::record(
            commons::audit::Category::Accounting,
            serde_json::json!({
                "event": "disconnect",
                "account": account,
                "char_name": char_name,
                "client_id": client_id,
            }),
        );
    }

    // Unexpected disconnect while a character is loaded: persist it (Java
    // `GameClient.onDisconnection` → `Disconnection.storeMe().deleteMe()`).
    // In `Entering` the Player is still held by the session, not the world.
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => {
            let oid = s.player_object_id();
            // Java `GameClient.onDisconnection`: the account logout is sent
            // either way, but a player already in offline mode is *not*
            // deleted. The session is gone before the socket event in the
            // port's own offline path, so this only guards a redundant event.
            if !crate::game_loop::offline_trade::is_offline_trader(world, oid) {
                // TvT: same participant-drop / forfeit on an unexpected disconnect.
                crate::game_loop::events::tvt::on_player_logout(world, oid);
                store_and_remove_player(world, oid);
            }
        }
        Some(ClientSession::Entering(s)) => {
            // The Player is still held by the session, not the world store, so
            // build the full save straight from the loaded `PlayerData`. It must
            // carry every child collection: `store_player` reconciles them, so an
            // items/skills-empty save here would wipe the just-loaded character.
            let b = s.player();
            let _ = world.db.send(db::DbCommand::StorePlayer {
                save: Box::new(db::PlayerSaveData {
                    // Unconditional here, unlike the periodic save: this is the
                    // logout/disconnect store, which is Java's `storeMe()` path
                    // rather than `autoSave()`, and the comment above says why
                    // an items-empty save would be destructive.
                    store_items: true,
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
                }),
            });
        }
        _ => {}
    }
    world.clients.remove(&client_id);
    world.hwids.remove(&client_id); // Java `GameClient` hardware info dies with the connection (G31).
    world.protocol_versions.remove(&client_id); // same lifetime — it is the connection's, not the character's.
    let account = world
        .login
        .accounts_in_gameserver
        .iter()
        .find(|(_, id)| **id == client_id)
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

/// One login-link event: registration, auth response, kick, char-count ask.
pub(crate) fn handle_login_link_event(world: &mut World, event: LoginLinkEvent) {
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
        send_to_client(world, client_id, server_packets::login_fail(0, 1)); // SYSTEM_ERROR_LOGIN_LATER
        world.login.accounts_in_gameserver.remove(&account);
        world.clients.remove(&client_id); // disconnect after the queued packet
        let _ = world
            .login
            .link
            .send(LoginLinkCommand::PlayerLogout { account });
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
    // Top up the `CharInfoTable` equivalent (G30). This is also the create
    // path: the client reloads its character list right after a successful
    // creation, so a brand-new character becomes mailable here without the
    // create command having to round-trip its freshly assigned id back.
    for c in &chars {
        crate::game_loop::mail::on_character_created(world, &c.name, c.object_id);
    }
    // Java `CharSelectionInfo.loadCharacterSelectInfo`'s
    // `OFFLINE_DISCONNECT_SAME_ACCOUNT` branch: seeing the list for an account
    // evicts that account's unattended shops. Off on this dist.
    let ids: Vec<i32> = chars.iter().map(|c| c.object_id).collect();
    crate::game_loop::offline_trade::on_character_list(world, &ids);
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
            &world.cursed_weapons,
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
