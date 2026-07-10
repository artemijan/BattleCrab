//! The game thread and its 100 ms tick loop (CONCURRENCY_MODEL §2.2).
//!
//! Runs on one dedicated OS thread that owns [`World`]. The base tick is 100 ms,
//! matching Java's `GameTimeTaskManager` and high-priority task-manager rate.
//! Steps: drain network events → drain login-link events → fire timers → run
//! tick systems (G4+) → flush. Packet dispatch and login handoff land here on
//! the game thread, keeping handler code sequential and 1:1 with Java `run()`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tracing::{debug, error, info, warn};

use crate::data::GameData;
use crate::db::{self, DbEvent, DbEventRx, NewCharacter};
use crate::loginlink::{CommandTx, LoginLinkCommand, LoginLinkEvent, LoginLinkEventRx};
use crate::network::client_packets::{
    self as cp, ex_opcodes as exop, opcodes as cop, AuthLogin, CharacterCreate,
};
use crate::network::{server_packets, NetEvent, NetEventRx};
use crate::session::{ClientSession, Session, SessionKey};
use crate::world::{WaitingClient, World};

/// Base tick period. Slower Java rates (1 s, 5 s…) become `world.tick % N == 0`
/// systems on top of this.
pub const TICK: Duration = Duration::from_millis(100);

/// A tick that runs longer than this is the failure mode of the single-thread
/// design, so it must be visible from day one (CONCURRENCY_MODEL §2.6 rule 4).
const TICK_OVERRUN_WARN: Duration = Duration::from_millis(50);

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
    pub max_characters_per_account: i32,
    pub delete_days: i32,
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
        max_characters_per_account,
        delete_days,
    } = ch;
    let mut world = World::new(
        link_tx,
        max_characters_per_account,
        delete_days,
        data,
        db_tx,
    );
    info!("GameLoop: started ({} ms tick).", TICK.as_millis());

    while !shutdown.is_requested() {
        let tick_start = Instant::now();

        // 1. Network events: connects, disconnects, and inbound packets.
        drain_network(&mut world, &net_rx);
        // 2. Service results: login-link + DB (path added G5+).
        drain_login_link(&mut world, &login_rx);
        drain_db(&mut world, &db_rx);

        // 3. One-shot timers due this tick.
        world.run_due_tasks();

        // 4. Fixed-rate tick systems (movement, AI, attack…) — added in G4+.
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
    // Final drain + save-all lands with the DB thread (G3).
}

/// Bounded, non-blocking drain of the network→game channel (step 1 of the tick).
fn drain_network(world: &mut World, net_rx: &NetEventRx) {
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

/// Dispatch one decrypted client packet body (opcode + payload) on the game
/// thread, gated by the client's session state (Java's per-`ConnectionState`
/// validity).
fn on_packet(world: &mut World, client_id: u32, data: Vec<u8>) {
    let Some(&opcode) = data.first() else { return };
    let body = &data[1..];
    match opcode {
        cop::AUTH_LOGIN => handle_auth_login(world, client_id, body),
        cop::NEW_CHARACTER => handle_new_character(world, client_id),
        cop::CHARACTER_CREATE => handle_character_create(world, client_id, body),
        cop::CHARACTER_DELETE => handle_character_delete(world, client_id, body),
        cop::CHARACTER_RESTORE => handle_character_restore(world, client_id, body),
        cop::CHARACTER_SELECT => handle_character_select(world, client_id, body),
        cop::ENTER_WORLD => handle_enter_world(world, client_id),
        // RequestSkillCoolTime (IN_GAME): resend the reuse timers.
        cop::REQUEST_SKILL_COOL_TIME => {
            if let Some(cs @ ClientSession::InGame(_)) = world.clients.get(&client_id) {
                cs.send(crate::network::enter_world::skill_cool_time());
            }
        }
        cop::EX_PACKET => on_ex_packet(world, client_id, body),
        _ => error!("GameLoop: client {client_id} sent opcode 0x{opcode:02x}, unhandled."),
    }
}

/// Dispatch an extended (`0xD0`) client packet by its 2-byte sub-opcode.
fn on_ex_packet(world: &mut World, client_id: u32, body: &[u8]) {
    let Some((sub, ex_body)) = cp::read_ex_opcode(body) else {
        return;
    };
    match sub {
        exop::REQUEST_CHARACTER_NAME_CREATABLE => {
            handle_request_character_name_creatable(world, client_id, ex_body)
        }
        // RequestKeyMapping (ENTERING + IN_GAME): STORE_UI_SETTINGS is on, so
        // reply with the (empty) stored UI key mapping.
        exop::REQUEST_KEY_MAPPING => {
            if let Some(cs) = world.clients.get(&client_id) {
                if matches!(cs, ClientSession::Entering(_) | ClientSession::InGame(_)) {
                    cs.send(server_packets::ex_ui_setting());
                }
            }
        }
        // RequestManorList (IN_GAME): the castles that offer a manor.
        exop::REQUEST_MANOR_LIST => {
            if let Some(cs @ ClientSession::InGame(_)) = world.clients.get(&client_id) {
                cs.send(server_packets::ex_send_manor_list());
            }
        }
        // RequestUserBanInfo (IN_GAME): Mobius has a null handler — the client
        // tolerates no reply, so consume it silently. TODO: ExUserBanInfo.
        exop::REQUEST_USER_BAN_INFO => {}
        exop::REQUEST_GOTO_LOBBY => {
            let maybe_session = world.clients.get(&client_id);
            if let Some(ClientSession::InLobby(session)) = maybe_session {
                let body = server_packets::char_selection_info(
                    &session.state.account,
                    session.play_ok1(),
                    &session.state.chars,
                    -1,
                    world.max_characters_per_account,
                    &world.data.experience,
                );
                session.send(body);
            }
        }
        _ => error!("GameLoop: client {client_id} sent ex-opcode 0x{sub:04x}, unhandled."),
    }
}

/// Port of `RequestCharacterNameCreatable.runImpl`: validate the name, then ask
/// the DB whether it already exists; the reply is `ExIsCharNameCreatable`.
fn handle_request_character_name_creatable(world: &mut World, client_id: u32, ex_body: &[u8]) {
    if !matches!(
        world.clients.get(&client_id),
        Some(ClientSession::InLobby(_))
    ) {
        return;
    }
    let Some(name) = cp::read_name_creatable(ex_body) else {
        return;
    };
    // INVALID_NAME=4 (Java `isAlphaNumeric` + template) is decided here; the
    // name-exists / length checks need the DB.
    let valid = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric());
    if !valid {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::ex_is_char_name_creatable(4));
        }
        return;
    }
    let _ = world
        .db
        .send(db::DbCommand::CheckNameCreatable { client_id, name });
}

/// Port of `NewCharacter.runImpl`: offer the creatable templates that exist.
fn handle_new_character(world: &mut World, client_id: u32) {
    if !matches!(
        world.clients.get(&client_id),
        Some(ClientSession::InLobby(_))
    ) {
        return;
    }
    let templates: Vec<_> = crate::data::player_template::CREATABLE_CLASSES
        .iter()
        .filter_map(|(class_id, race, _)| {
            world
                .data
                .player_templates
                .get(*class_id)
                .map(|t| (*class_id, *race, t))
        })
        .collect();
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::new_character_success(&templates));
    }
}

/// Port of `CharacterCreate.runImpl`: cheap validation on the game thread, then
/// hand the insert (name-uniqueness + count) to the DB thread.
fn handle_character_create(world: &mut World, client_id: u32, body: &[u8]) {
    if !matches!(
        world.clients.get(&client_id),
        Some(ClientSession::InLobby(_))
    ) {
        return;
    }
    let Some(pkt) = CharacterCreate::read(body) else {
        return;
    };
    use crate::network::server_packets::char_create_fail as fail;
    // Fail reasons: 16-chars=3, incorrect-name=4, creation-failed=0.
    // Java `Util.isAlphaNumeric` uses `Character.isLetterOrDigit` (Unicode).
    let name_ok = (1..=16).contains(&pkt.name.chars().count())
        && !pkt.name.is_empty()
        && pkt.name.chars().all(|c| c.is_alphanumeric());
    let send = |world: &World, body: Vec<u8>| {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(body);
        }
    };
    if pkt.name.chars().count() < 1 || pkt.name.chars().count() > 16 {
        return send(world, fail(3));
    }
    if !name_ok {
        return send(world, fail(4));
    }
    if !(0..=2).contains(&pkt.face)
        || pkt.hair_style < 0
        || (!pkt.is_female && pkt.hair_style > 4)
        || (pkt.is_female && pkt.hair_style > 6)
        || !(0..=3).contains(&pkt.hair_color)
    {
        return send(world, fail(0));
    }
    // Only base (creatable) classes; template must exist.
    let Some(race) = crate::data::player_template::creatable_race(pkt.class_id) else {
        return send(world, fail(0));
    };
    let Some(template) = world.data.player_templates.get(pkt.class_id) else {
        return send(world, fail(0));
    };
    let spawn = template
        .creation_points
        .get(0)
        .copied()
        .unwrap_or((0, 0, 0));
    let account = match world.clients.get(&client_id) {
        Some(ClientSession::InLobby(s)) => s.account().to_string(),
        _ => return,
    };
    // Created character starts at full HP/MP (Java: setCurrentHp(getMaxHp())).
    let max_hp = crate::model::calc_max_hp(&world.data, template, 1) as i32;
    let max_mp = crate::model::calc_max_mp(&world.data, template, 1) as i32;
    // Initial skills for the class (Java: getAvailableSkills at level 1).
    let skills = world.data.skill_trees.initial_skills(pkt.class_id);
    let data = NewCharacter {
        account,
        name: pkt.name,
        race: race.ordinal(),
        class_id: pkt.class_id,
        sex: pkt.is_female as i32,
        face: pkt.face,
        hair_style: pkt.hair_style,
        hair_color: pkt.hair_color,
        x: spawn.0,
        y: spawn.1,
        z: spawn.2,
        max_hp,
        max_mp,
        skills,
    };
    let _ = world
        .db
        .send(db::DbCommand::CreateCharacter { client_id, data });
}

/// Port of `CharacterDelete.runImpl`: mark the slot's character for deletion.
fn handle_character_delete(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(slot) = cp::read_char_slot(body) else {
        return;
    };
    let ClientSession::InLobby(s) = (match world.clients.get(&client_id) {
        Some(cs) => cs,
        None => return,
    }) else {
        return;
    };
    let Some(chr) = s.char_at(slot) else {
        s.send(server_packets::char_delete_fail(1)); // UNKNOWN
        return;
    };
    let (char_id, account) = (chr.object_id, s.account().to_string());
    s.send(server_packets::char_delete_success());
    if world.delete_days == 0 {
        let _ = world.db.send(db::DbCommand::DeleteCharacter { char_id });
        let _ = world
            .db
            .send(db::DbCommand::LoadCharacters { client_id, account });
    } else {
        let delete_time = commons::util::now_millis() + world.delete_days as i64 * 86_400_000;
        let _ = world.db.send(db::DbCommand::MarkDelete {
            client_id,
            account,
            char_id,
            delete_time,
        });
    }
}

/// Port of `CharacterRestore.runImpl`: clear the deletion timer.
fn handle_character_restore(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(slot) = cp::read_char_slot(body) else {
        return;
    };
    let ClientSession::InLobby(s) = (match world.clients.get(&client_id) {
        Some(cs) => cs,
        None => return,
    }) else {
        return;
    };
    let Some(chr) = s.char_at(slot) else { return };
    let (char_id, account) = (chr.object_id, s.account().to_string());
    let _ = world.db.send(db::DbCommand::RestoreCharacter {
        client_id,
        account,
        char_id,
    });
}

/// Port of `CharacterSelect.runImpl`: build the chosen character's `Player`,
/// move to the entering state, and send `CharSelected` (starts the loading
/// screen; the client then sends `EnterWorld`).
fn handle_character_select(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(slot) = cp::read_char_slot(body) else { return };
    let ClientSession::InLobby(s) = (match world.clients.get(&client_id) {
        Some(cs) => cs,
        None => return,
    }) else {
        return;
    };
    let Some(chr) = s.char_at(slot).cloned() else { return };
    let player = crate::model::Player::from_char(&world.data, &chr);
    let selected = server_packets::char_selected(&player, s.play_ok1(), 0);

    // Transition InLobby → Entering, holding the built Player.
    if let Some(ClientSession::InLobby(s)) = world.clients.remove(&client_id) {
        let s = s.into_entering(player);
        s.send(selected);
        info!("GameLoop: client {client_id} selected character '{}'.", s.player().name);
        world.clients.insert(client_id, ClientSession::Entering(s));
    }
}

/// Port of `EnterWorld.runImpl` (minimal): register the player in the world and
/// send `UserInfo` so the character appears with correct stats. The long tail of
/// enter-world packets (inventory, skills, shortcuts, quests, …) is deferred.
fn handle_enter_world(world: &mut World, client_id: u32) {
    let Some(ClientSession::Entering(s)) = world.clients.remove(&client_id) else {
        // Not in the entering state; ignore (Java gates by ENTERING).
        if let Some(cs) = world.clients.remove(&client_id) {
            world.clients.insert(client_id, cs);
        }
        return;
    };
    let (session, player) = s.into_ingame();
    let name = player.name.clone();
    let data = &world.data;
    use crate::network::enter_world as ew;
    use crate::network::user_info::user_info;

    // The enter-world packet burst (EnterWorld.runImpl). Lists that need systems
    // not yet built are empty (TODOs in `enter_world`); stats/position are real.
    // TODO(G6/G7): populate ItemList/SkillList/HennaInfo/shortcuts/quests once
    // inventory, skills, and quests exist. TODO(G9): clan/friend/mail packets.
    session.send(user_info(&player, data));
    session.send(ew::ex_vitality_effect_info(&player));
    session.send(server_packets::ex_ui_setting());
    // TODO: macros (SendMacroList) — empty for now.
    session.send(ew::ex_get_bookmark_info());
    session.send(ew::item_list());
    session.send(ew::ex_quest_item_list());
    session.send(ew::shortcut_init());
    session.send(ew::ex_basic_action_list(data));
    session.send(ew::henna_info());
    session.send(ew::skill_list());
    session.send(ew::acquire_skill_list());
    session.send(ew::etc_status_update());
    session.send(ew::ex_pledge_waiting_list_alarm());
    session.send(ew::ex_subjob_info(&player));
    session.send(ew::ex_user_info_inven_weight(&player));
    session.send(ew::ex_adena_inven_count());
    session.send(ew::ex_user_info_equip_slot(&player));
    session.send(ew::quest_list());
    session.send(ew::ex_rotation(&player));
    session.send(ew::friend_list());
    session.send(ew::skill_cool_time());

    // Register the player in the world and re-send UserInfo (Java does both).
    session.send(user_info(&player, data));
    session.send(ew::ex_set_compass_zone_code(0));
    session.send(ew::move_to_location(&player));
    for kind in 0..4 {
        session.send(ew::ex_auto_soul_shot(0, true, kind));
    }
    session.send(ew::abnormal_status_update());
    session.send(ew::system_message(ew::SM_WELCOME));

    world.players.insert(player.object_id, player);
    info!("GameLoop: '{name}' entered the world ({} online).", world.players.len());
    world.clients.insert(client_id, ClientSession::InGame(session));
}

/// Port of `clientpackets/AuthLogin.runImpl`: register the account on this game
/// server and ask the login server to validate the session key.
fn handle_auth_login(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = AuthLogin::read(body) else {
        return;
    };
    if pkt.login_name.is_empty() {
        world.clients.remove(&client_id); // closeNow
        return;
    }
    // Only valid once, from a still-connecting client (Java: accountName == null).
    if !matches!(
        world.clients.get(&client_id),
        Some(ClientSession::Connecting(_))
    ) {
        return;
    }
    let account = pkt.login_name;
    // addGameServerLogin: reject a duplicate login for the account.
    if world.login.accounts_in_gameserver.contains_key(&account) {
        world.clients.remove(&client_id); // close(null)
        return;
    }
    world
        .login
        .accounts_in_gameserver
        .insert(account.clone(), client_id);
    let key = SessionKey::new(pkt.login_key1, pkt.login_key2, pkt.play_key1, pkt.play_key2);
    world.login.waiting.insert(
        account.clone(),
        WaitingClient {
            client_id,
            session_key: key,
        },
    );
    let _ = world
        .login
        .link
        .send(LoginLinkCommand::PlayerAuthRequest { account, key });
}

/// Clean up a disconnected client and inform the login server.
fn on_disconnect(world: &mut World, client_id: u32) {
    // TODO(G3+): persist the player (store HP/MP/position/etc.) before removing.
    if let Some(ClientSession::InGame(s)) = world.clients.get(&client_id) {
        world.players.remove(&s.player_object_id());
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
fn drain_login_link(world: &mut World, login_rx: &LoginLinkEventRx) {
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
fn handle_player_auth_response(world: &mut World, account: String, authed: bool) {
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
fn drain_db(world: &mut World, db_rx: &DbEventRx) {
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
        }
    }
}

/// A character list came back from the DB. Always cache it on the session (for
/// slot → object-id mapping); send `CharSelectionInfo` only when `send_list`
/// (login/delete/restore) — after creation Java caches without re-sending.
/// Transitions `Authenticated` → `InLobby` on the first load.
fn on_characters_loaded(
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
fn handle_kick(world: &mut World, account: String) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::CharData;
    use commons::network::PacketWriter;

    fn test_world() -> (
        World,
        db::CmdTx,
        db::CmdRx,
        tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
    ) {
        let (link_tx, link_rx) = tokio::sync::mpsc::unbounded_channel();
        let (db_tx, db_rx) = tokio::sync::mpsc::unbounded_channel();
        let world = World::new(link_tx, 7, 3, GameData::for_test(), db_tx.clone());
        (world, db_tx, db_rx, link_rx)
    }

    fn connect(world: &mut World, id: u32) -> tokio::sync::mpsc::UnboundedReceiver<Vec<u8>> {
        let (out_tx, out_rx) = tokio::sync::mpsc::unbounded_channel();
        world.clients.insert(
            id,
            ClientSession::Connecting(Session::new(id, out_tx, "127.0.0.1:1".parse().unwrap())),
        );
        out_rx
    }

    fn auth_login_body(name: &str, key: SessionKey) -> Vec<u8> {
        // readImpl order: name, playKey2, playKey1, loginKey1, loginKey2.
        let mut w = PacketWriter::new();
        w.write_string(name);
        w.write_i32(key.play_ok2);
        w.write_i32(key.play_ok1);
        w.write_i32(key.login_ok1);
        w.write_i32(key.login_ok2);
        w.into_bytes()
    }

    fn dummy_char(object_id: i32, name: &str) -> CharData {
        CharData {
            object_id,
            name: name.into(),
            account_name: "bob".into(),
            level: 1,
            max_hp: 80,
            cur_hp: 80.0,
            max_mp: 30,
            cur_mp: 30.0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            sex: 0,
            x: 1,
            y: 2,
            z: 3,
            exp: 0,
            sp: 0,
            reputation: 0,
            pk_kills: 0,
            pvp_kills: 0,
            clan_id: 0,
            race: 0,
            class_id: 0,
            base_class_id: 0,
            delete_time: 0,
            last_access: 0,
            vitality_points: 0,
            access_level: 0,
            noble: false,
            char_slot: 0,
        }
    }

    fn human_fighter_template() -> crate::data::player_template::PlayerTemplate {
        let mut hp_table = vec![0.0; 90];
        let mut mp_table = vec![0.0; 90];
        hp_table[1] = 80.0;
        mp_table[1] = 30.0;
        crate::data::player_template::PlayerTemplate {
            class_id: 0,
            base_str: 40,
            base_dex: 30,
            base_con: 43,
            base_int: 21,
            base_wit: 11,
            base_men: 25,
            hp_table,
            mp_table,
            creation_points: vec![(-71338, 258271, -3104)],
            ..Default::default()
        }
    }

    fn character_create_body(name: &str, class_id: i32) -> Vec<u8> {
        // readImpl: name, race, isFemale, classId, 6 stat ints, hairStyle, hairColor, face.
        let mut w = commons::network::PacketWriter::new();
        w.write_string(name);
        w.write_i32(0); // race
        w.write_i32(0); // isFemale
        w.write_i32(class_id);
        for _ in 0..6 {
            w.write_i32(0);
        }
        w.write_i32(0); // hairStyle
        w.write_i32(0); // hairColor
        w.write_i32(0); // face
        w.into_bytes()
    }

    /// Reproduction: character creation must actually insert against the real
    /// characters schema and report success (the "can't create" report).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn character_create_inserts_into_real_schema() {
        // Copy of the real database so we exercise its exact schema.
        let src = concat!(env!("CARGO_MANIFEST_DIR"), "/../../interlude_classic.db");
        let dir = std::env::temp_dir().join(format!("l2r_create_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("c.db");
        std::fs::copy(src, &db_path).expect("copy real db");
        let url = format!("jdbc:sqlite:{}", db_path.display());

        let (db_tx, db_cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let (db_event_tx, db_event_rx) = std::sync::mpsc::channel();
        let db_handle = db::spawn(url, 1, 7, db_cmd_rx, db_event_tx);

        let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
        let data = GameData {
            experience: crate::data::ExperienceData::empty(),
            player_templates: crate::data::PlayerTemplateData::from_vec(vec![
                human_fighter_template(),
            ]),
            skill_trees: crate::data::SkillTreeData::empty(),
            stat_bonus: crate::data::StatBonus::empty(),
            action_data: crate::data::ActionData::empty(),
        };
        let mut world = World::new(link_tx, 7, 3, data, db_tx);

        let (out_tx, _out_rx) = tokio::sync::mpsc::unbounded_channel();
        let account = format!("acct{}", std::process::id());
        let s = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
            .into_authenticated(account.clone(), SessionKey::new(1, 2, 3, 4))
            .into_lobby(vec![]);
        world.clients.insert(1, ClientSession::InLobby(s));

        let name = format!("Tc{}", std::process::id() % 100000);
        handle_character_create(&mut world, 1, &character_create_body(&name, 0));

        // The DB thread must report a successful insert, then the reloaded list.
        match db_event_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
        {
            DbEvent::CharacterCreated { result, .. } => {
                assert_eq!(
                    result,
                    crate::db::CreateResult::Ok,
                    "character insert failed against real schema"
                );
            }
            _ => panic!("expected CharacterCreated"),
        }
        match db_event_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
        {
            DbEvent::CharactersLoaded { chars, .. } => {
                assert_eq!(chars.len(), 1);
                assert_eq!(chars[0].name, name);
                assert_eq!(chars[0].class_id, 0);
                assert_eq!(chars[0].x, -71338);
            }
            _ => panic!("expected CharactersLoaded"),
        }

        // Clean up the copied database.
        world.db.send(crate::db::DbCommand::Shutdown).ok();
        tokio::task::spawn_blocking(move || db_handle.join())
            .await
            .unwrap()
            .ok();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn auth_then_load_reaches_lobby_with_char_list() {
        let (mut world, _db_tx, mut db_rx, mut link_rx) = test_world();
        let mut out_rx = connect(&mut world, 1);

        // AuthLogin → PlayerAuthRequest.
        let key = SessionKey::new(11, 12, 21, 22);
        handle_auth_login(&mut world, 1, &auth_login_body("Bob", key));
        assert_eq!(world.login.accounts_in_gameserver.get("bob"), Some(&1));
        assert!(matches!(
            link_rx.try_recv().unwrap(),
            LoginLinkCommand::PlayerAuthRequest { .. }
        ));

        // PlayerAuthResponse(authed) → Authenticated + LOGIN_SUCCESS + LoadCharacters.
        handle_player_auth_response(&mut world, "bob".to_string(), true);
        assert!(matches!(
            world.clients.get(&1),
            Some(ClientSession::Authenticated(_))
        ));
        assert!(matches!(
            link_rx.try_recv().unwrap(),
            LoginLinkCommand::PlayerInGame { .. }
        ));
        assert_eq!(out_rx.try_recv().unwrap(), server_packets::login_success());
        assert!(matches!(
            db_rx.try_recv().unwrap(),
            db::DbCommand::LoadCharacters { client_id: 1, .. }
        ));

        // DB returns the list → InLobby + CharSelectionInfo (opcode 0x09).
        on_characters_loaded(
            &mut world,
            1,
            "bob".to_string(),
            vec![dummy_char(0x10000000, "Hero")],
            true,
        );
        assert!(matches!(
            world.clients.get(&1),
            Some(ClientSession::InLobby(_))
        ));
        let sel = out_rx.try_recv().unwrap();
        assert_eq!(sel[0], server_packets::opcodes::CHARACTER_SELECTION_INFO);
    }

    #[test]
    fn character_delete_marks_slot() {
        let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
        let mut out_rx = connect(&mut world, 1);
        // Fast-forward to InLobby with one character.
        let ClientSession::Connecting(s) = world.clients.remove(&1).unwrap() else {
            unreachable!()
        };
        let s = s
            .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
            .into_lobby(vec![dummy_char(555, "Hero")]);
        world.clients.insert(1, ClientSession::InLobby(s));

        let mut body = PacketWriter::new();
        body.write_i32(0); // slot 0
        handle_character_delete(&mut world, 1, &body.into_bytes());

        assert_eq!(
            out_rx.try_recv().unwrap(),
            server_packets::char_delete_success()
        );
        match db_rx.try_recv().unwrap() {
            db::DbCommand::MarkDelete {
                char_id,
                delete_time,
                ..
            } => {
                assert_eq!(char_id, 555);
                assert!(delete_time > commons::util::now_millis());
            }
            _ => panic!("expected MarkDelete"),
        }
    }

    #[test]
    fn wrong_session_key_closes_connection() {
        let (mut world, _db_tx, _db_rx, mut link_rx) = test_world();
        let mut out_rx = connect(&mut world, 1);

        handle_auth_login(
            &mut world,
            1,
            &auth_login_body("bob", SessionKey::new(1, 2, 3, 4)),
        );
        let _ = link_rx.try_recv(); // PlayerAuthRequest

        handle_player_auth_response(&mut world, "bob".to_string(), false);
        assert_eq!(out_rx.try_recv().unwrap(), server_packets::login_fail(0, 1));
        assert!(world.clients.get(&1).is_none());
        assert!(!world.login.accounts_in_gameserver.contains_key("bob"));
        assert!(matches!(
            link_rx.try_recv().unwrap(),
            LoginLinkCommand::PlayerLogout { .. }
        ));
    }

    #[test]
    fn duplicate_account_login_is_rejected() {
        let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
        world
            .login
            .accounts_in_gameserver
            .insert("bob".to_string(), 99); // already on
        connect(&mut world, 1);
        handle_auth_login(
            &mut world,
            1,
            &auth_login_body("bob", SessionKey::new(1, 2, 3, 4)),
        );
        assert!(world.clients.get(&1).is_none());
        assert_eq!(world.login.accounts_in_gameserver.get("bob"), Some(&99));
    }
}
