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
use crate::model::formulas;
use crate::model::skill::{abnormal_type_client_id, ActiveBuff, OperateType, Skill, SkillEffect, TargetType};
use crate::model::stats::BaseStat;
use crate::model::Player;
use crate::scheduler::ScheduledTask;
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

/// `Formulas.getRegeneratePeriod`: 3000 ms for player characters (30 × the
/// 100 ms base tick), matching Java's `CreatureStatus.startHpMpRegeneration`.
const REGEN_TICK_PERIOD: u64 = 30;

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
    pub geo: crate::geo::GeoEngine,
    pub path_finding: i32,
    pub max_characters_per_account: i32,
    pub delete_days: i32,
    pub starting_adena: i64,
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
        path_finding,
        max_characters_per_account,
        delete_days,
        starting_adena,
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
    world.path_finding = path_finding;
    info!("GameLoop: started ({} ms tick).", TICK.as_millis());

    while !shutdown.is_requested() {
        let tick_start = Instant::now();

        // 1. Network events: connects, disconnects, and inbound packets.
        drain_network(&mut world, &net_rx);
        // 2. Service results: login-link + DB (path added G5+).
        drain_login_link(&mut world, &login_rx);
        drain_db(&mut world, &db_rx);

        // 3. One-shot timers due this tick.
        apply_due_tasks(&mut world);

        // 4. Fixed-rate tick systems (movement, AI, attack…) — added in G4+.
        // Movement runs every tick (unlike the gated systems below) — it
        // needs to recompute the authoritative server-side position each
        // 100 ms, same as Java's `MovementTaskManager`.
        crate::model::movement::tick(&mut world);
        if world.tick.is_multiple_of(REGEN_TICK_PERIOD) {
            run_regen_tick(&mut world);
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
            if let Some(cs @ ClientSession::InGame(session)) = world.clients.get(&client_id) {
                if let Some(player) = world.players.get(&session.player_object_id()) {
                    cs.send(server_packets::skill_cool_time(player, world.tick));
                }
            }
        }
        cop::USE_ITEM => handle_use_item(world, client_id, body),
        cop::REQUEST_UN_EQUIP_ITEM => handle_request_un_equip_item(world, client_id, body),
        cop::REQUEST_MAGIC_SKILL_USE => handle_request_magic_skill_use(world, client_id, body),
        cop::REQUEST_ACQUIRE_SKILL => handle_request_acquire_skill(world, client_id, body),
        cop::ACTION => handle_action(world, client_id, body),
        cop::REQUEST_TARGET_CANCELD => handle_request_target_canceld(world, client_id, body),
        cop::MOVE_BACKWARD_TO_LOCATION => handle_move_backward_to_location(world, client_id, body),
        cop::VALIDATE_POSITION => handle_validate_position(world, client_id, body),
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
    let items = resolve_initial_items(world, pkt.class_id);
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
        items,
    };
    let _ = world
        .db
        .send(db::DbCommand::CreateCharacter { client_id, data });
}

/// Port of `CharacterCreate.initNewChar`'s equipment loop: replay
/// `initialEquipment.xml` for `class_id` through a scratch `Inventory` (adding
/// starting adena too), so slot-conflict resolution matches
/// `Inventory::equip_item` by construction, then read the final state back out
/// as DB-ready rows.
fn resolve_initial_items(world: &World, class_id: i32) -> Vec<db::NewItem> {
    use crate::data::item_data;
    use crate::model::inventory::Inventory;

    let catalog = &world.data.item_data;
    let mut inv = Inventory::new();
    let mut next_temp_id = -1;
    let mut alloc = || {
        let id = next_temp_id;
        next_temp_id -= 1;
        id
    };

    for entry in world.data.initial_equipment.get(class_id) {
        let object_id = inv.add_item(catalog, alloc(), entry.item_id, entry.count);
        if entry.equipped {
            inv.equip_item(catalog, object_id);
        }
    }
    if world.starting_adena > 0 {
        inv.add_item(catalog, alloc(), item_data::ADENA_ID, world.starting_adena);
    }

    inv.items()
        .iter()
        .map(|item| db::NewItem {
            item_id: item.item_id,
            count: item.count,
            paperdoll_index: inv.paperdoll_slot_of(item.object_id),
        })
        .collect()
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

    // The enter-world packet burst (EnterWorld.runImpl). Inventory is real as of
    // G5; lists that need systems not yet built are still empty (TODOs in
    // `enter_world`): SkillList/HennaInfo/shortcuts/quests, clan/friend/mail.
    session.send(user_info(&player, data));
    session.send(ew::ex_vitality_effect_info(&player));
    session.send(server_packets::ex_ui_setting());
    // TODO: macros (SendMacroList) — empty for now.
    session.send(ew::ex_get_bookmark_info());
    session.send(ew::item_list(&player, data));
    session.send(ew::ex_quest_item_list());
    session.send(ew::shortcut_init());
    session.send(ew::ex_basic_action_list(data));
    session.send(ew::henna_info());
    session.send(ew::skill_list(&player, data));
    session.send(ew::acquire_skill_list(&player, data));
    session.send(ew::etc_status_update());
    session.send(ew::ex_pledge_waiting_list_alarm());
    session.send(ew::ex_subjob_info(&player));
    session.send(ew::ex_user_info_inven_weight(&player, data));
    session.send(ew::ex_adena_inven_count(&player));
    session.send(ew::ex_user_info_equip_slot(&player));
    session.send(ew::quest_list());
    session.send(ew::ex_rotation(&player));
    session.send(ew::friend_list());
    session.send(server_packets::skill_cool_time(&player, world.tick));

    // Register the player in the world and re-send UserInfo (Java does both).
    session.send(user_info(&player, data));
    session.send(ew::ex_set_compass_zone_code(0));
    session.send(ew::move_to_location(&player));
    for kind in 0..4 {
        session.send(ew::ex_auto_soul_shot(0, true, kind));
    }
    session.send(ew::abnormal_status_update(&player, world.tick));
    session.send(ew::system_message(ew::SM_WELCOME));

    world.players.insert(player.object_id, player);
    info!("GameLoop: '{name}' entered the world ({} online).", world.players.len());
    world.clients.insert(client_id, ClientSession::InGame(session));
}

/// Port of `clientpackets/UseItem.runImpl`, scoped to gear: right-clicking a
/// `Weapon`/`Armor` toggles equip/unequip (Java routes both through this same
/// packet). `EtcItem` "use" (potions, soulshots, …) is a later milestone — the
/// packet is consumed silently for those.
fn handle_use_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::UseItem::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    let catalog = &world.data.item_data;
    let Some(player) = world.players.get_mut(&object_id) else { return };

    let Some(item) = player.inventory.items().iter().find(|i| i.object_id == pkt.object_id) else { return };
    let Some(template) = catalog.get(item.item_id) else { return };
    if !template.is_equipable() {
        return; // EtcItem "use" (potions, shots, …): later milestone.
    }
    let body_part = template.body_part;

    let changed = if player.inventory.paperdoll_slot_of(pkt.object_id).is_some() {
        player.inventory.unequip_body_part(body_part)
    } else {
        player.inventory.equip_item(catalog, pkt.object_id)
    };
    finish_equip_change(world, client_id, object_id, &changed);
}

/// Port of `clientpackets/RequestUnEquipItem.runImpl` (combat/cursed-weapon
/// guards are skipped — there's no combat system yet).
fn handle_request_un_equip_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(body_part) = cp::read_char_slot(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    let Some(player) = world.players.get_mut(&object_id) else { return };
    let changed = player.inventory.unequip_slot(body_part);
    finish_equip_change(world, client_id, object_id, &changed);
}

/// Shared tail of the equip/unequip handlers: persist each changed slot
/// (`items.loc`/`loc_data`), then resend `InventoryUpdate` + `UserInfo` (Java:
/// `sendInventoryUpdate` + `broadcastUserInfo`).
fn finish_equip_change(world: &mut World, client_id: u32, object_id: i32, changed: &[i32]) {
    if changed.is_empty() {
        return;
    }
    let Some(player) = world.players.get(&object_id) else { return };
    for &oid in changed {
        let (loc, loc_data) = match player.inventory.paperdoll_slot_of(oid) {
            Some(slot) => ("PAPERDOLL", slot as i32),
            None => ("INVENTORY", 0),
        };
        let _ = world.db.send(db::DbCommand::UpdateItemLocation { object_id: oid, loc, loc_data });
    }
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::enter_world::inventory_update(player, &world.data, changed));
        cs.send(crate::network::user_info::user_info(player, &world.data));
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
        }
    }
}

/// The client id of the in-game session linked to a `Player`, or `None` if
/// they've disconnected since the task was scheduled (dead-id ⇒ no-op, per
/// the scheduler's contract).
fn client_for_player(world: &World, player_object_id: i32) -> Option<u32> {
    world.clients.iter().find_map(|(&cid, cs)| match cs {
        ClientSession::InGame(s) if s.player_object_id() == player_object_id => Some(cid),
        _ => None,
    })
}

/// Send `packet` to every in-game player except `exclude_object_id`. Java's
/// equivalent (`Creature.broadcastPacket`/`Broadcast.toKnownPlayers`) scopes
/// this to the known-list (visibility grid); there's no region grid yet (see
/// `docs/PROGRESS.md` G7's deferred-TODO note), so this is a flat "everyone
/// else connected" pass — correct for target/movement broadcast semantics,
/// just not filtered by distance/visibility yet.
fn broadcast_to_others(world: &World, exclude_object_id: i32, packet: &[u8]) {
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            if s.player_object_id() != exclude_object_id {
                cs.send(packet.to_vec());
            }
        }
    }
}

/// Round a millisecond duration up to whole 100 ms ticks.
fn ms_to_ticks(ms: i32) -> u64 {
    (ms.max(0) as u64).div_ceil(100)
}

/// Send a `SystemMessage` + `ActionFailed` to one client — the standard
/// "request rejected" reply shape all over `Player.useMagic` /
/// `SkillCaster.checkUseConditions`.
fn send_sm_and_action_failed(world: &World, client_id: u32, message_id: i16, params: &[server_packets::SmParam]) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::system_message_with(message_id, params));
        cs.send(server_packets::action_failed());
    }
}

/// Send `packet` to a player's own client (if still connected) and everyone
/// else — Java `Creature.broadcastPacket(packet)` with `includeSelf == true`.
fn broadcast_including_self(world: &World, object_id: i32, packet: &[u8]) {
    if let Some(client_id) = client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(packet.to_vec());
        }
    }
    broadcast_to_others(world, object_id, packet);
}

/// Port of `Skill.getTarget` + the `targethandlers/{Self,Target,Enemy,
/// EnemyOnly}.java` scripts as a static match, players-only (no NPCs, peace
/// zones, or party checks yet). `Err(sm_id)` is the system message the caller
/// sends alongside `ActionFailed` (Java: the handlers' `sendMessage` path) —
/// SM 109 for an invalid target, SM 181 when geodata blocks line of sight.
fn resolve_cast_target(world: &World, caster: &Player, skill: &Skill, ctrl: bool) -> Result<i32, i16> {
    use server_packets::sm_ids;

    let resolved = match skill.target_type {
        TargetType::Self_ => return Ok(caster.object_id),
        // `Target.java`: the selected target, friend or foe; self allowed
        // (and self skips the LOS check — "you can always target yourself").
        TargetType::Target => {
            let t = caster.target.ok_or(sm_ids::INVALID_TARGET)?;
            if t == caster.object_id {
                return Ok(t);
            }
            t
        }
        // `Enemy.java`/`EnemyOnly.java`: not self, and `isAutoAttackable ||
        // forceUse` — players carry no PvP flag/karma yet, so nothing is
        // auto-attackable and ctrl (force-use) is always required.
        TargetType::Enemy | TargetType::EnemyOnly => {
            let t = caster.target.ok_or(sm_ids::INVALID_TARGET)?;
            if t == caster.object_id || !ctrl {
                return Err(sm_ids::INVALID_TARGET);
            }
            t
        }
        TargetType::Other => return Err(sm_ids::INVALID_TARGET),
    };
    let target = world.players.get(&resolved).ok_or(sm_ids::INVALID_TARGET)?;
    // "Geodata check when character is within range" — every non-self
    // handler ends with `GeoEngine.canSeeTarget` → CANNOT_SEE_TARGET.
    if !world.geo.can_see_target(caster.x, caster.y, caster.z, target.x, target.y, target.z) {
        return Err(sm_ids::CANNOT_SEE_TARGET);
    }
    Ok(resolved)
}

/// `Util.checkIfInRange`: 2D (or 3D) distance vs `range` + both collision
/// radii.
fn in_range(a: &Player, b: &Player, range: i32, include_z: bool) -> bool {
    let (dx, dy, dz) = ((b.x - a.x) as f64, (b.y - a.y) as f64, (b.z - a.z) as f64);
    let d2 = dx * dx + dy * dy + if include_z { dz * dz } else { 0.0 };
    let reach = range as f64 + a.collision_radius + b.collision_radius;
    d2 <= reach * reach
}

/// Port of `clientpackets/RequestMagicSkillUse.runImpl` + `Player.useMagic`'s
/// guards + `SkillCaster.castSkill`/`checkUseConditions`. Narrowing: no
/// queued skills, no follow-into-range (an out-of-range cast just fails), no
/// mute/sit/fake-death states (none exist), toggles and non-single targeting
/// still silently ignored.
fn handle_request_magic_skill_use(world: &mut World, client_id: u32, body: &[u8]) {
    use server_packets::{sm_ids, SmParam};

    let Some(pkt) = cp::RequestMagicSkillUse::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();

    let Some(player) = world.players.get(&object_id) else { return };
    // Unknown skill → ActionFailed (RequestMagicSkillUse.runImpl).
    let Some(&skill_level) = player.skills.get(&pkt.magic_id) else {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    };
    let Some(skill) = world.data.skill_data.get(pkt.magic_id, skill_level).cloned() else { return };

    // Passive → ActionFailed (useMagic); toggles/unsupported targeting are
    // not castable yet and are consumed silently, same as before.
    if skill.operate_type == OperateType::Passive {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    if skill.operate_type != OperateType::Active || skill.target_type == TargetType::Other {
        return;
    }

    // Reuse gate (`Player.useMagic`'s `isSkillDisabled` branch): timestamp
    // reuses (> 3000 ms) get the remaining h/m/s breakdown, short ones SM 48.
    if let Some(&(until_tick, total_ms)) = player.reuses.get(&skill.id) {
        if until_tick > world.tick {
            let name_param = SmParam::SkillName { id: skill.id, level: skill.level };
            if total_ms > 3000 {
                let remaining_ms = (until_tick - world.tick) * 100;
                let hours = (remaining_ms / 3_600_000) as i32;
                let minutes = ((remaining_ms % 3_600_000) / 60_000) as i32;
                let seconds = ((remaining_ms / 1000) % 60) as i32;
                if hours > 0 {
                    send_sm_and_action_failed(
                        world,
                        client_id,
                        sm_ids::S2_HOURS_S3_MINUTES_S4_SECONDS_REMAINING_FOR_REUSE,
                        &[name_param, SmParam::Int(hours), SmParam::Int(minutes), SmParam::Int(seconds)],
                    );
                } else if minutes > 0 {
                    send_sm_and_action_failed(
                        world,
                        client_id,
                        sm_ids::S2_MINUTES_S3_SECONDS_REMAINING_FOR_REUSE,
                        &[name_param, SmParam::Int(minutes), SmParam::Int(seconds)],
                    );
                } else {
                    send_sm_and_action_failed(
                        world,
                        client_id,
                        sm_ids::S2_SECONDS_REMAINING_FOR_REUSE,
                        &[name_param, SmParam::Int(seconds)],
                    );
                }
            } else {
                send_sm_and_action_failed(world, client_id, sm_ids::S1_IS_NOT_AVAILABLE_REUSE, &[name_param]);
            }
            return;
        }
    }

    // Single NORMAL casting slot busy (`checkUseConditions`).
    if player.cast.is_some() {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }

    // MP/HP prechecks (`checkUseConditions`).
    if player.cur_mp < (skill.mp_initial_consume + skill.mp_consume) as f64 {
        send_sm_and_action_failed(world, client_id, sm_ids::NOT_ENOUGH_MP, &[]);
        return;
    }
    if player.cur_hp <= skill.hp_consume as f64 {
        send_sm_and_action_failed(world, client_id, sm_ids::NOT_ENOUGH_HP, &[]);
        return;
    }

    let target_oid = match resolve_cast_target(world, player, &skill, pkt.ctrl_pressed) {
        Ok(oid) => oid,
        Err(sm_id) => {
            send_sm_and_action_failed(world, client_id, sm_id, &[]);
            return;
        }
    };

    // Cast-range gate (`SkillCaster.castSkill`). Java returns null and lets
    // the AI walk into range; there's no follow-to-cast yet, so just unstick
    // the client (narrowing note).
    if skill.cast_range > 0 && target_oid != object_id {
        let target = &world.players[&target_oid];
        if !in_range(player, target, skill.cast_range, false) {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::action_failed());
            }
            return;
        }
    }

    start_casting(world, client_id, object_id, &skill, target_oid);
}

/// Port of `SkillCaster.startCasting` (phase 0). Narrowing: no skill mastery,
/// no `MAGIC_REUSE_RATE` stat (reuse = the skill's `reuseDelay`), no item/
/// fame/clan-rep consumes, no `stopEffectsOnAction`, no `MoveToPawn`
/// cosmetic (only `ExRotation` for target facing).
fn start_casting(world: &mut World, client_id: u32, object_id: i32, skill: &Skill, target_oid: i32) {
    use server_packets::{sm_ids, SmParam};

    let Some(player) = world.players.get(&object_id) else { return };
    let (hit_ms, cancel_ms, cool_ms) = formulas::calc_cast_times(player, &world.data, skill);
    let displayed_cast_time = hit_ms + cancel_ms;

    // Register the reuse (skipped when trivially short, like Java's `> 10`).
    if skill.reuse_delay > 10 {
        let until_tick = world.tick + ms_to_ticks(skill.reuse_delay);
        if let Some(player) = world.players.get_mut(&object_id) {
            player.reuses.insert(skill.id, (until_tick, skill.reuse_delay));
        }
    }

    // Stop movement (`clientStopMoving`) — the client freezes on its own; the
    // broadcast pins the position for everyone else.
    let was_moving = world.players.get(&object_id).is_some_and(|p| p.move_data.is_some());
    if was_moving {
        if let Some(player) = world.players.get_mut(&object_id) {
            player.move_data = None;
        }
        let p = &world.players[&object_id];
        broadcast_including_self(world, object_id, &server_packets::stop_move(object_id, p.x, p.y, p.z, p.heading));
    }

    // Face the target (Java: `setHeading` + broadcast `ExRotation`).
    if target_oid != object_id {
        let (dx, dy) = {
            let p = &world.players[&object_id];
            let t = &world.players[&target_oid];
            ((t.x - p.x) as f64, (t.y - p.y) as f64)
        };
        let heading = crate::model::movement::calculate_heading(dx, dy);
        if let Some(player) = world.players.get_mut(&object_id) {
            player.heading = heading;
        }
        let p = &world.players[&object_id];
        broadcast_including_self(world, object_id, &crate::network::enter_world::ex_rotation(p));
    }

    // Initial MP consume + StatusUpdate (re-checked here in Java too).
    let mut mp_update = None;
    if let Some(player) = world.players.get_mut(&object_id) {
        if skill.mp_initial_consume > 0 {
            if player.cur_mp < skill.mp_initial_consume as f64 {
                send_sm_and_action_failed(world, client_id, sm_ids::NOT_ENOUGH_MP, &[]);
                return;
            }
            player.cur_mp -= skill.mp_initial_consume as f64;
            mp_update = Some(player.cur_mp as i32);
        }
    }
    if let Some(mp) = mp_update {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::status_update(object_id, &[(server_packets::status_update_type::CUR_MP, mp)]));
        }
    }

    // Broadcast the cast start, then the caster-only YOU_USE_S1 + cast bar.
    {
        let caster = &world.players[&object_id];
        let target = &world.players[&target_oid];
        broadcast_including_self(
            world,
            object_id,
            &server_packets::magic_skill_use(caster, target, skill.id, skill.level, displayed_cast_time, skill.reuse_delay),
        );
    }
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::system_message_with(
            sm_ids::YOU_USE_S1,
            &[SmParam::SkillName { id: skill.id, level: skill.level }],
        ));
        cs.send(server_packets::setup_gauge(object_id, 0, displayed_cast_time));
    }

    let cast_seq = {
        let Some(player) = world.players.get_mut(&object_id) else { return };
        player.cast_seq += 1;
        player.cast = Some(crate::model::CastState {
            skill_id: skill.id,
            skill_level: skill.level,
            target_object_id: target_oid,
            seq: player.cast_seq,
            launched: false,
            cancel_ms,
            cool_ms,
        });
        player.cast_seq
    };
    world
        .scheduler
        .schedule(world.tick + ms_to_ticks(hit_ms), ScheduledTask::SkillLaunch { player_object_id: object_id, cast_seq });
}

/// A cast task's `CastState` if it's still the live one (seq matches);
/// stale/aborted tasks resolve to `None` and no-op.
fn live_cast(world: &World, player_object_id: i32, cast_seq: u64) -> Option<crate::model::CastState> {
    world
        .players
        .get(&player_object_id)?
        .cast
        .clone()
        .filter(|c| c.seq == cast_seq)
}

/// Port of `SkillCaster.launchSkill` (phase 1): re-check `effectRange`
/// (failure → SM 748 + a *quiet* stop, `stopCasting(false)` — Java only
/// sends `MagicSkillCanceled` on explicit aborts), broadcast
/// `MagicSkillLaunched`, mark the cast unabortable, schedule the finish.
fn handle_skill_launch(world: &mut World, player_object_id: i32, cast_seq: u64) {
    use server_packets::sm_ids;

    let Some(cast) = live_cast(world, player_object_id, cast_seq) else { return };
    let Some(skill) = world.data.skill_data.get(cast.skill_id, cast.skill_level).cloned() else { return };

    // Target gone (logged off) → quiet stop, like Java's dead-ref return.
    if !world.players.contains_key(&cast.target_object_id) {
        if let Some(player) = world.players.get_mut(&player_object_id) {
            player.cast = None;
        }
        return;
    }

    if skill.effect_range > 0 && cast.target_object_id != player_object_id {
        let caster = &world.players[&player_object_id];
        let target = &world.players[&cast.target_object_id];
        if !in_range(caster, target, skill.effect_range, true) {
            if let Some(client_id) = client_for_player(world, player_object_id) {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(server_packets::system_message_with(sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED, &[]));
                }
            }
            if let Some(player) = world.players.get_mut(&player_object_id) {
                player.cast = None;
            }
            return;
        }
    }

    broadcast_including_self(
        world,
        player_object_id,
        &server_packets::magic_skill_launched(player_object_id, skill.id, skill.level, &[cast.target_object_id]),
    );

    if let Some(player) = world.players.get_mut(&player_object_id) {
        if let Some(c) = player.cast.as_mut() {
            c.launched = true;
        }
    }
    world.scheduler.schedule(
        world.tick + ms_to_ticks(cast.cancel_ms),
        ScheduledTask::SkillFinish { player_object_id, cast_seq },
    );
}

/// Port of `SkillCaster.finishSkill` + `callSkill` (phase 2): re-check and
/// consume MP/HP (failure → SM + quiet stop, no cancel packet), apply the
/// skill's effects, then either free the cast slot or hold it for
/// `_coolTime`.
fn handle_skill_finish(world: &mut World, player_object_id: i32, cast_seq: u64) {
    use server_packets::sm_ids;

    let Some(cast) = live_cast(world, player_object_id, cast_seq) else { return };
    let Some(skill) = world.data.skill_data.get(cast.skill_id, cast.skill_level).cloned() else { return };
    let client_id = client_for_player(world, player_object_id);

    // MP/HP re-check at landing (no refund of the initial consume).
    let insufficient_mp = world.players[&player_object_id].cur_mp < skill.mp_consume as f64;
    let insufficient_hp = world.players[&player_object_id].cur_hp <= skill.hp_consume as f64;
    if insufficient_mp || insufficient_hp {
        if let Some(client_id) = client_id {
            let sm = if insufficient_mp { sm_ids::NOT_ENOUGH_MP } else { sm_ids::NOT_ENOUGH_HP };
            send_sm_and_action_failed(world, client_id, sm, &[]);
        }
        if let Some(player) = world.players.get_mut(&player_object_id) {
            player.cast = None;
        }
        return;
    }

    let mut updates = Vec::new();
    if let Some(player) = world.players.get_mut(&player_object_id) {
        if skill.mp_consume > 0 {
            player.cur_mp = (player.cur_mp - skill.mp_consume as f64).max(0.0);
            updates.push((server_packets::status_update_type::CUR_MP, player.cur_mp as i32));
        }
        if skill.hp_consume > 0 {
            player.cur_hp = (player.cur_hp - skill.hp_consume as f64).max(0.0);
            updates.push((server_packets::status_update_type::CUR_HP, player.cur_hp as i32));
        }
    }
    if !updates.is_empty() {
        if let Some(client_id) = client_id {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::status_update(player_object_id, &updates));
            }
        }
    }

    // `callSkill` → effect application, if the target is still around.
    if world.players.contains_key(&cast.target_object_id) {
        apply_skill_effects(world, player_object_id, cast.target_object_id, &skill);
    }

    // Hold the cast slot for the cool phase (`stopCasting(false)` after
    // `_coolTime`), freeing inline when there's nothing to wait out.
    let cool_ticks = ms_to_ticks(cast.cool_ms);
    if cool_ticks == 0 {
        if let Some(player) = world.players.get_mut(&player_object_id) {
            player.cast = None;
        }
    } else {
        world
            .scheduler
            .schedule(world.tick + cool_ticks, ScheduledTask::CastEnd { player_object_id, cast_seq });
    }
}

/// `SkillCaster.run`'s terminal `stopCasting(false)` — the cool phase ended.
fn handle_cast_end(world: &mut World, player_object_id: i32, cast_seq: u64) {
    if live_cast(world, player_object_id, cast_seq).is_none() {
        return;
    }
    if let Some(player) = world.players.get_mut(&player_object_id) {
        player.cast = None;
    }
}

/// Port of `Creature.abortCast` → `stopCasting(aborted == true)`: only casts
/// that haven't launched can be aborted; broadcast `MagicSkillCanceled` (self
/// included, to stop the animation) + `ActionFailed` to the caster. The
/// already-scheduled phase tasks go stale via the seq mismatch.
fn abort_cast(world: &mut World, object_id: i32) {
    let abortable = world.players.get(&object_id).is_some_and(|p| p.cast.as_ref().is_some_and(|c| !c.launched));
    if !abortable {
        return;
    }
    if let Some(player) = world.players.get_mut(&object_id) {
        player.cast = None;
    }
    broadcast_including_self(world, object_id, &server_packets::magic_skill_canceld(object_id));
    if let Some(client_id) = client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
    }
}

/// The `callSkill` → `activateSkill` → effect-handler chain for the effect
/// kinds ported so far. Continuous stat modifiers land as an `ActiveBuff` on
/// the target; `MagicalAttack`/`Heal` are instant.
fn apply_skill_effects(world: &mut World, caster_oid: i32, target_oid: i32, skill: &Skill) {
    use server_packets::{sm_ids, SmParam};

    // Magic crit is rolled once per cast (Java rolls in each instant effect's
    // `instant()`; one roll covers the single instant effect skills have).
    let m_crit_rate = world.players[&caster_oid].m_crit_hit as f64;
    let crit_roll = world.roll(1000);
    let mcrit = skill.magic_type == 1 && formulas::calc_magic_crit(m_crit_rate, skill.is_bad(), crit_roll);

    for effect in &skill.effects {
        match *effect {
            SkillEffect::MagicalAttack { power } => {
                let (m_atk, caster_name) = {
                    let c = &world.players[&caster_oid];
                    (c.m_atk as f64, c.name.clone())
                };
                let m_def = world.players[&target_oid].m_def as f64;
                let damage = formulas::calc_magic_dam(m_atk, m_def, power, mcrit);
                apply_magic_damage(world, caster_oid, target_oid, damage, mcrit, &caster_name);
            }
            SkillEffect::Heal { power } => {
                let m_atk = world.players[&caster_oid].m_atk as f64;
                let amount = formulas::calc_heal(power, m_atk, mcrit);
                let healed = {
                    let Some(target) = world.players.get_mut(&target_oid) else { continue };
                    // Overheal clamp (`Heal.java`).
                    let amount = amount.min((target.max_hp as f64 - target.cur_hp).max(0.0));
                    target.cur_hp += amount;
                    amount
                };
                let caster_name = world.players[&caster_oid].name.clone();
                if let Some(client_id) = client_for_player(world, target_oid) {
                    if let Some(cs) = world.clients.get(&client_id) {
                        if target_oid != caster_oid {
                            cs.send(server_packets::system_message_with(
                                sm_ids::S2_HP_HAS_BEEN_RESTORED_BY_C1,
                                &[SmParam::PlayerName(caster_name), SmParam::Int(healed as i32)],
                            ));
                        } else {
                            cs.send(server_packets::system_message_with(
                                sm_ids::S1_HP_HAS_BEEN_RESTORED,
                                &[SmParam::Int(healed as i32)],
                            ));
                        }
                        let cur_hp = world.players[&target_oid].cur_hp as i32;
                        cs.send(server_packets::status_update(
                            target_oid,
                            &[(server_packets::status_update_type::CUR_HP, cur_hp)],
                        ));
                    }
                }
            }
            SkillEffect::StatModifier(_) => {} // collected below
        }
    }

    // Continuous effects → one ActiveBuff on the target (`applyEffects`).
    let buff_effects = skill.stat_modifier_effects();
    if !buff_effects.is_empty() {
        let expires_at_tick = world.tick + (skill.abnormal_time.max(0) as u64) * 10;
        let buff = ActiveBuff {
            skill_id: skill.id,
            skill_level: skill.level,
            abnormal_type_client_id: abnormal_type_client_id(&skill.abnormal_type),
            expires_at_tick,
            effects: buff_effects,
        };
        if let Some(target) = world.players.get_mut(&target_oid) {
            target.apply_buff(&world.data, buff);
        }
        world
            .scheduler
            .schedule(expires_at_tick, ScheduledTask::BuffExpire { player_object_id: target_oid, skill_id: skill.id });
        let now = world.tick;
        if let Some(client_id) = client_for_player(world, target_oid) {
            if let Some(target) = world.players.get(&target_oid) {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(crate::network::enter_world::abnormal_status_update(target, now));
                }
            }
        }
    }
}

/// Port of `Creature.doAttack` → `PlayerStatus.reduceHp` for magic skill
/// damage between players: CP absorbs first, then HP — clamped at 1.0
/// because there's no death system yet (TODO(G9 death): `doDie`). Also rolls
/// Java's `Formulas.calcAtkBreak` cast-break against a pre-launch cast on
/// the victim (SM 27 + `MagicSkillCanceled`).
fn apply_magic_damage(world: &mut World, caster_oid: i32, target_oid: i32, damage: f64, mcrit: bool, caster_name: &str) {
    use server_packets::{sm_ids, SmParam};

    let (target_name, cp_after, hp_after) = {
        let Some(target) = world.players.get_mut(&target_oid) else { return };
        let cp_absorb = damage.min(target.cur_cp);
        target.cur_cp -= cp_absorb;
        target.cur_hp = (target.cur_hp - (damage - cp_absorb)).max(1.0);
        (target.name.clone(), target.cur_cp as i32, target.cur_hp as i32)
    };
    let dmg_int = damage as i32;

    if let Some(client_id) = client_for_player(world, caster_oid) {
        if let Some(cs) = world.clients.get(&client_id) {
            if mcrit {
                cs.send(server_packets::system_message_with(sm_ids::M_CRITICAL, &[]));
            }
            cs.send(server_packets::system_message_with(
                sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
                &[
                    SmParam::PlayerName(caster_name.to_string()),
                    SmParam::PlayerName(target_name.clone()),
                    SmParam::Int(dmg_int),
                ],
            ));
        }
    }
    if let Some(client_id) = client_for_player(world, target_oid) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                sm_ids::C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2,
                &[
                    SmParam::PlayerName(target_name.clone()),
                    SmParam::PlayerName(caster_name.to_string()),
                    SmParam::Int(dmg_int),
                ],
            ));
        }
    }

    // Both sides see the victim's new CP/HP (`broadcastStatusUpdate`).
    broadcast_including_self(
        world,
        target_oid,
        &server_packets::status_update(
            target_oid,
            &[
                (server_packets::status_update_type::CUR_CP, cp_after),
                (server_packets::status_update_type::CUR_HP, hp_after),
            ],
        ),
    );

    // Cast break (`Formulas.calcAtkBreak`, `AltGameCancelByHit = cast`).
    let breakable = world
        .players
        .get(&target_oid)
        .is_some_and(|p| p.cast.as_ref().is_some_and(|c| !c.launched));
    if breakable {
        let men_bonus = {
            let t = &world.players[&target_oid];
            world.data.stat_bonus.bonus(crate::model::stats::BaseStat::Men, t.men)
        };
        let break_roll = world.roll(100);
        if formulas::calc_atk_break(damage, men_bonus, break_roll) {
            abort_cast(world, target_oid);
            if let Some(client_id) = client_for_player(world, target_oid) {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(server_packets::system_message_with(sm_ids::YOUR_CASTING_HAS_BEEN_INTERRUPTED, &[]));
                }
            }
        }
    }
}

/// `BuffFinishTask`, fired when a buff's `abnormalTime` elapses
/// (`ScheduledTask::BuffExpire`). A buff already gone (re-cast/replaced) is a
/// no-op, matching the scheduler's dead-id contract.
fn handle_buff_expire(world: &mut World, player_object_id: i32, skill_id: i32) {
    let still_active = world
        .players
        .get(&player_object_id)
        .is_some_and(|p| p.buffs.iter().any(|b| b.skill_id == skill_id));
    if !still_active {
        return;
    }
    if let Some(player) = world.players.get_mut(&player_object_id) {
        player.remove_buff(&world.data, skill_id);
    }
    let now = world.tick;
    let Some(client_id) = client_for_player(world, player_object_id) else { return };
    if let Some(player) = world.players.get(&player_object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(crate::network::enter_world::abnormal_status_update(player, now));
        }
    }
}

/// Port of `clientpackets/RequestAcquireSkill.runImpl`, `AcquireSkillType::CLASS`
/// only (see the G6 plan's scope notes — every other type is silently
/// ignored, same as Java ignores an out-of-state/unsupported request).
fn handle_request_acquire_skill(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestAcquireSkill::read(body) else { return };
    if pkt.acquire_type != cp::RequestAcquireSkill::CLASS {
        return;
    }
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();

    let Some(player) = world.players.get(&object_id) else { return };
    let Some(learn) = world.data.skill_trees.skill_learn(player.class_id, pkt.skill_id, pkt.skill_level) else { return };
    if learn.get_level > player.level || learn.level_up_sp > player.sp {
        return; // TODO: SystemMessage (level/SP gate)
    }
    let (skill_id, skill_level, level_up_sp) = (learn.skill_id, learn.skill_level, learn.level_up_sp);

    if let Some(player) = world.players.get_mut(&object_id) {
        player.sp -= level_up_sp;
        player.skills.insert(skill_id, skill_level);
    }
    let _ = world.db.send(db::DbCommand::UpsertSkill { char_id: object_id, skill_id, skill_level });

    if let Some(player) = world.players.get(&object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::acquire_skill_done());
            cs.send(crate::network::enter_world::skill_list(player, &world.data));
            cs.send(crate::network::enter_world::acquire_skill_list(player, &world.data));
            cs.send(crate::network::user_info::user_info(player, &world.data));
        }
    }
}

/// Port of `clientpackets/Action.runImpl`, narrowed to the single-click
/// (`action_id == 0`) select-a-player case — the only targetable `WorldObject`
/// kind that exists yet (no NPCs/items until G8+). Clicking yourself goes
/// through the same path (Java routes self-clicks through `PlayerAction`
/// like any other player target). Shift-click (`action_id == 1`, examine
/// window) and the flood-protector/bot-penalty/trade/instance guards Java
/// has are all skipped as out of scope (no trade/instances/bot-detection in
/// the Rust port yet). Always terminates with `ActionFailed`, matching
/// `WorldObject.onAction`'s convention.
fn handle_action(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::Action::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();

    if world.players.contains_key(&pkt.object_id) {
        set_target(world, client_id, object_id, Some(pkt.object_id));
    }
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::action_failed());
    }
}

/// Port of `clientpackets/RequestTargetCanceld.runImpl`: Esc aborts an
/// in-flight cast (Java `abortAllSkillCasters`, regardless of the
/// `targetLost` flag), then clears the target if `targetLost`. The
/// locked-target/queued-skill/air-ship guards are features that don't exist
/// yet.
fn handle_request_target_canceld(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestTargetCanceld::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    abort_cast(world, object_id);
    if !pkt.target_lost {
        return;
    }
    set_target(world, client_id, object_id, None);
}

/// Port of `Player.setTarget`'s core, narrowed to Player targets (no
/// NPCs/vehicles/party checks yet — see the handlers above). Same-target
/// re-click is a no-op (Java only re-sends `ValidateLocation`, a cosmetic
/// target-position correction we skip).
fn set_target(world: &mut World, client_id: u32, object_id: i32, new_target: Option<i32>) {
    let Some(player) = world.players.get(&object_id) else { return };
    if player.target == new_target {
        return;
    }

    // Prevents /target exploiting: reject targets too far away in Z.
    let new_target = new_target.filter(|&t| {
        let Some(target_player) = world.players.get(&t) else { return false };
        (target_player.z - player.z).abs() <= 1000
    });
    if player.target == new_target {
        return;
    }

    let (px, py, pz) = (player.x, player.y, player.z);
    if let Some(t) = new_target {
        let Some(target_player) = world.players.get(&t) else { return };
        let (max_hp, cur_hp) = (target_player.max_hp, target_player.cur_hp as i32);
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::my_target_selected(t));
            cs.send(server_packets::status_update(
                t,
                &[
                    (server_packets::status_update_type::MAX_HP, max_hp),
                    (server_packets::status_update_type::CUR_HP, cur_hp),
                ],
            ));
        }
        broadcast_to_others(world, object_id, &server_packets::target_selected(object_id, t, px, py, pz));
    } else {
        // Java's clear path uses broadcastPacket(includeSelf=true): the
        // deselecting client must get TargetUnselected too, or its UI keeps
        // the target locked.
        let pkt = server_packets::target_unselected(object_id, px, py, pz);
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(pkt.clone());
        }
        broadcast_to_others(world, object_id, &pkt);
    }

    if let Some(player) = world.players.get_mut(&object_id) {
        player.target = new_target;
    }
}

/// Port of `clientpackets/MoveBackwardToLocation.runImpl` +
/// `Creature.moveToLocation`'s geodata movement checks: the requested
/// destination is clamped to the last walkable cell via
/// `GeoEngine.getValidLocation`. The pathfinding fallback (Java runs
/// `CellPathFinding` when the clamp shortens the path by > 30 units) is not
/// ported yet — where Java would walk around an obstacle, the player walks up
/// to it and stops. Door-crossing, teleport-mode switches, and queued-skill
/// clearing are all skipped as out of scope (no doors/admin-teleport/
/// queued-skills yet).
fn handle_move_backward_to_location(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::MoveBackwardToLocation::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    let Some(player) = world.players.get(&object_id) else { return };

    if pkt.target_x == pkt.origin_x && pkt.target_y == pkt.origin_y && pkt.target_z == pkt.origin_z {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::stop_move(object_id, player.x, player.y, player.z, player.heading));
            cs.send(server_packets::action_failed());
        }
        return;
    }

    // Java `PlayerAI.onIntentionMoveTo`: a move request during a cast is
    // rejected with ActionFailed (the cast is NOT aborted); the queued
    // next-intention move is not ported.
    if player.cast.is_some() {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }

    let mut target_x = pkt.target_x;
    let mut target_y = pkt.target_y;
    let target_z = pkt.target_z;
    let mut dx = (target_x - player.x) as f64;
    let mut dy = (target_y - player.y) as f64;
    if dx * dx + dy * dy > 98_010_000.0 {
        // 9900² — Java's max single-click move distance.
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    let mut distance = (dx * dx + dy * dy).sqrt();

    // GEODATA MOVEMENT CHECKS (`Creature.moveToLocation`). Java skips the
    // destination correction for far clicks (> 3000: "should be able to
    // click far away and move" — pathfinding would take over) and for
    // intentional falls ((curZ - z) > 300 with distance < 300).
    if world.path_finding > 0
        && distance <= 3000.0
        && !(player.z - target_z > 300 && distance < 300.0)
    {
        let (vx, vy, _vz) =
            world.geo.get_valid_location(player.x, player.y, player.z, target_x, target_y, target_z);
        // Players keep the client-requested z (Java: `if (!isPlayer()) z = destiny.getZ()`).
        target_x = vx;
        target_y = vy;
        dx = (target_x - player.x) as f64;
        dy = (target_y - player.y) as f64;
        distance = (dx * dx + dy * dy).sqrt();
    }

    // Java: `(distance < 1) && (Config.PATHFINDING > 0 || isPlayable())` —
    // a fully clamped-away (or degenerate) move is canceled.
    if distance < 1.0 {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }

    let (start_x, start_y, start_z) = (player.x, player.y, player.z);
    let heading = crate::model::movement::calculate_heading(dx, dy);
    let speed = (if player.running { player.run_spd } else { player.walk_spd } as f64) * player.move_multiplier;
    let total_ticks = if speed > 0.0 { ((10.0 * distance / speed).round() as u64).max(1) } else { 1 };
    let start_tick = world.tick;

    if let Some(player) = world.players.get_mut(&object_id) {
        player.heading = heading;
        player.move_data = Some(crate::model::movement::MoveData {
            start_x,
            start_y,
            start_z,
            dest_x: target_x,
            dest_y: target_y,
            dest_z: target_z,
            start_tick,
            total_ticks,
        });
    }

    // Broadcast once at move start, including the mover — the client does
    // not self-predict; it only starts walking once the server confirms with
    // `MoveToLocation` (Java: `Creature.moveToLocation` → `broadcastPacket`,
    // which `Player` overrides with `includeSelf == true`).
    let move_pkt =
        server_packets::move_to_location(object_id, target_x, target_y, target_z, start_x, start_y, start_z);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(move_pkt.clone());
    }
    broadcast_to_others(world, object_id, &move_pkt);
}

/// Port of `clientpackets/ValidatePosition.runImpl` — reconcile the client's
/// periodic position report with the server's authoritative position.
/// Narrowing: no vehicles, falling state, flying/water zones, observer mode,
/// or Blink, and the trailing door-exploit check is skipped (no doors) —
/// those branches simply can't trigger yet.
fn handle_validate_position(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::ValidatePosition::read(body) else { return };
    // Field-level split borrow: `player` (mut) + `geo`/`clients` (shared).
    let World { clients, players, geo, .. } = world;
    let Some(ClientSession::InGame(session)) = clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    let Some(player) = players.get_mut(&object_id) else { return };
    // Java: also bails while teleporting / in observer mode (states we lack).
    if player.cast.is_some() {
        return;
    }

    if pkt.x == 0 && pkt.y == 0 && player.x != 0 {
        return;
    }

    let dx = (pkt.x - player.x) as f64;
    let dy = (pkt.y - player.y) as f64;
    let dz = (pkt.z - player.z) as f64;
    let diff_sq = dx * dx + dy * dy;

    // "If too large, messes observation" — moderate drift only.
    let mut correction: Option<Vec<u8>> = None;
    if diff_sq < 360_000.0 && (diff_sq > 250_000.0 || dz.abs() > 200.0) {
        if dz.abs() > 200.0 && dz.abs() < 1500.0 && (pkt.z - player.client_z).abs() < 800 {
            // Plausible stairs/slope climb: trust the client's z.
            player.z = pkt.z;
        } else {
            // Push the server position back to the client (built pre-snap,
            // exactly where Java builds the packet).
            correction =
                Some(server_packets::validate_location(object_id, player.x, player.y, player.z, player.heading));
        }
    }

    // Out-of-sync check: a jump larger than one second of movement snaps the
    // server to the client position, geodata-correcting z when the server
    // was above the client (falling through a floor edge).
    let sdx = (pkt.x - player.x) as f64;
    let sdy = (pkt.y - player.y) as f64;
    let sdz = (pkt.z - player.z) as f64;
    let move_speed = (if player.running { player.run_spd } else { player.walk_spd } as f64) * player.move_multiplier;
    if (sdx * sdx + sdy * sdy + sdz * sdz).sqrt() > move_speed {
        let z = if player.z > pkt.z { geo.get_height(pkt.x, pkt.y, player.z) } else { pkt.z };
        player.x = pkt.x;
        player.y = pkt.y;
        player.z = z;
    }

    player.client_x = pkt.x;
    player.client_y = pkt.y;
    player.client_z = pkt.z;
    player.client_heading = pkt.heading;

    if let (Some(pkt_bytes), Some(cs)) = (correction, clients.get(&client_id)) {
        cs.send(pkt_bytes);
    }
}

/// `CreatureStatus.doRegeneration`, run every `REGEN_TICK_PERIOD` ticks for
/// every in-game player. Iterates connected clients (not `world.players`
/// directly) so each player's `StatusUpdate` reaches its own connection.
fn run_regen_tick(world: &mut World) {
    let targets: Vec<(u32, i32)> = world
        .clients
        .iter()
        .filter_map(|(&client_id, cs)| match cs {
            ClientSession::InGame(s) => Some((client_id, s.player_object_id())),
            _ => None,
        })
        .collect();
    for (client_id, object_id) in targets {
        let Some(player) = world.players.get_mut(&object_id) else { continue };
        let Some(updates) = regen_player(player, &world.data) else { continue };
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::status_update(object_id, &updates));
        }
    }
}

/// `Formulas.getRegeneratePeriod`'s standing-still multiplier (1.1×) — the
/// only movement state a player can be in until G7 adds sitting/moving.
/// TODO(G7): sitting (×1.5) / running (×0.7) once those states exist.
const STANDING_STILL_REGEN_MULTIPLIER: f64 = 1.1;

/// `RegenHPFinalizer`/`RegenMPFinalizer`/`RegenCPFinalizer`, config-multiplier
/// terms omitted (`HpRegenMultiplier`/… default to 1.0 — see the `MAX_*`
/// stat-cap TODO in `model/mod.rs`). Returns the `StatusUpdate` entries for
/// whichever of HP/MP/CP actually changed, or `None` if all are already full.
fn regen_player(p: &mut Player, data: &GameData) -> Option<Vec<(u8, i32)>> {
    if p.cur_hp >= p.max_hp as f64 && p.cur_mp >= p.max_mp as f64 && p.cur_cp >= p.max_cp as f64 {
        return None;
    }
    let t = data
        .player_templates
        .get(p.class_id)
        .or_else(|| data.player_templates.get(p.base_class_id))
        .cloned()
        .unwrap_or_default();
    let level_mod = (p.level as f64 + 89.0) / 100.0;
    let con_bonus = data.stat_bonus.bonus(BaseStat::Con, p.con);
    let men_bonus = data.stat_bonus.bonus(BaseStat::Men, p.men);

    let mut updates = Vec::new();
    if p.cur_hp < p.max_hp as f64 {
        let regen = t.base_hp_regen(p.level) * level_mod * con_bonus * STANDING_STILL_REGEN_MULTIPLIER;
        p.cur_hp = (p.cur_hp + regen).min(p.max_hp as f64);
        updates.push((server_packets::status_update_type::CUR_HP, p.cur_hp as i32));
    }
    if p.cur_mp < p.max_mp as f64 {
        let regen = t.base_mp_regen(p.level) * level_mod * men_bonus * STANDING_STILL_REGEN_MULTIPLIER;
        p.cur_mp = (p.cur_mp + regen).min(p.max_mp as f64);
        updates.push((server_packets::status_update_type::CUR_MP, p.cur_mp as i32));
    }
    if p.cur_cp < p.max_cp as f64 {
        let regen = t.base_cp_regen(p.level) * level_mod * con_bonus * STANDING_STILL_REGEN_MULTIPLIER;
        p.cur_cp = (p.cur_cp + regen).min(p.max_cp as f64);
        updates.push((server_packets::status_update_type::CUR_CP, p.cur_cp as i32));
    }
    if updates.is_empty() {
        None
    } else {
        Some(updates)
    }
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
        let world = World::new(link_tx, 7, 3, 0, GameData::for_test(), db_tx.clone());
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
            items: vec![],
            skills: vec![],
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
            item_data: crate::data::ItemData::empty(),
            initial_equipment: crate::data::InitialEquipmentData::empty(),
            skill_data: crate::data::SkillData::empty(),
        };
        let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

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
                    db::CreateResult::Ok,
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

    /// G6 cast-pipeline gate: learn a class skill (SP spend + level gate),
    /// cast it, watch the buff land (P.Def +8%) and the right packet sequence
    /// go out, then fast-forward the scheduler past `abnormalTime` and watch
    /// it expire and P.Def come back down. Runs entirely against a synthetic
    /// `World` (no sockets) driven by manually advancing `world.tick` — real
    /// time would mean actually waiting out the buff's 20 in-game seconds,
    /// which a unit test shouldn't do (PLAN_GAME_SERVER.md §8.5: tick systems
    /// are tested against synthetic `World` state, not real time).
    #[test]
    fn learn_and_cast_buff_skill_applies_and_expires() {
        use crate::model::skill::{Skill, SkillEffect, StatModifierEffect};
        use crate::model::stats::{Stat, StatModifierType};

        let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
        let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();

        let mut hp_table = vec![0.0; 90];
        let mut mp_table = vec![0.0; 90];
        let mut cp_table = vec![0.0; 90];
        hp_table[5] = 100.0;
        mp_table[5] = 50.0;
        cp_table[5] = 20.0;
        let template = crate::data::player_template::PlayerTemplate {
            class_id: 0,
            base_str: 40,
            base_dex: 30,
            base_con: 43,
            base_int: 21,
            base_wit: 11,
            base_men: 25,
            hp_table,
            mp_table,
            cp_table,
            base_p_def: 80, // naked P.Def, matches the real HumanFighter.xml sum
            ..Default::default()
        };

        let mut data = GameData::for_test();
        data.player_templates = crate::data::PlayerTemplateData::from_vec(vec![template]);
        data.skill_trees.insert_for_test(
            0,
            crate::data::skill_tree::SkillLearn {
                skill_id: 91,
                skill_level: 1,
                name: "Defense Aura".into(),
                get_level: 5,
                level_up_sp: 100,
                auto_get: false,
            },
        );
        data.skill_data.insert_for_test(Skill {
            id: 91,
            level: 1,
            name: "Defense Aura".into(),
            operate_type: OperateType::Active,
            target_type: TargetType::Self_,
            magic_type: 1,
            effect_point: 0,
            cast_range: 0,
            effect_range: 0,
            hit_time: 400,
            hit_cancel_time: 0.0,
            cool_time: 0,
            reuse_delay: 2000,
            mp_consume: 4,
            mp_initial_consume: 1,
            hp_consume: 0,
            abnormal_time: 20,
            abnormal_level: 1,
            abnormal_type: "PD_UP".into(),
            effects: vec![SkillEffect::StatModifier(StatModifierEffect {
                stat: Stat::PhysicalDefence,
                mode: StatModifierType::Per,
                amount: 8.0,
            })],
        });

        let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

        // A level-5 character with 200 SP, walked straight to `InGame` (same
        // `Session` transition chain `handle_enter_world` uses in production).
        let mut chr = dummy_char(2001, "Def");
        chr.level = 5;
        chr.sp = 200;
        chr.cur_mp = 50.0;
        let player = Player::from_char(&world.data, &chr);
        assert_eq!(player.p_def, 80, "naked P.Def before any buff");

        let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel();
        let s = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
            .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
            .into_lobby(vec![])
            .into_entering(player);
        let (session, player) = s.into_ingame();
        world.players.insert(player.object_id, player);
        world.clients.insert(1, ClientSession::InGame(session));

        // --- Learn: RequestAcquireSkill(id=91, level=1, type=CLASS). ---
        let mut w = PacketWriter::new();
        w.write_i32(91);
        w.write_i32(1);
        w.write_i32(cp::RequestAcquireSkill::CLASS);
        handle_request_acquire_skill(&mut world, 1, &w.into_bytes());

        assert_eq!(world.players[&2001].skills.get(&91), Some(&1));
        assert_eq!(world.players[&2001].sp, 100, "200 SP - levelUpSp(100)");
        assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::ACQUIRE_SKILL_DONE);
        assert_eq!(out_rx.try_recv().unwrap()[0], 0x5F); // SkillList
        let _ = out_rx.try_recv().unwrap(); // AcquireSkillList
        let _ = out_rx.try_recv().unwrap(); // UserInfo

        // --- Cast: RequestMagicSkillUse(91). ---
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));

        assert!(world.players[&2001].cast.is_some());
        assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE); // initial MP consume
        assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_USE);
        assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::SYSTEM_MESSAGE); // YOU_USE_S1
        assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::SETUP_GAUGE);
        assert_eq!(world.players[&2001].cur_mp, 49.0, "50 - mpInitialConsume(1)");

        // --- Launch: hit = max(400/factor(1.0) − cancel(500), 0) = 0 ms, so
        // the launch task is already due; the finish follows 500 ms later.
        apply_due_tasks(&mut world);
        assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);
        assert!(world.players[&2001].cast.as_ref().is_some_and(|c| c.launched));

        world.tick += 5;
        apply_due_tasks(&mut world);
        assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE); // final MP consume
        assert_eq!(out_rx.try_recv().unwrap()[0], 0x85); // AbnormalStatusUpdate

        {
            let p = &world.players[&2001];
            assert!(p.cast.is_none(), "coolTime 0 frees the cast slot inline");
            assert_eq!(p.cur_mp, 45.0, "49 - mpConsume(4)");
            assert_eq!(p.buffs.len(), 1);
            assert_eq!(p.p_def, 86, "80 * 1.08 (PhysicalDefence +8%), rounded");
        }

        // --- Advance past expiry (abnormalTime 20 s = 200 ticks) and drain again. ---
        world.tick += 200;
        apply_due_tasks(&mut world);

        let expired = out_rx.try_recv().unwrap();
        assert_eq!(expired[0], 0x85);
        assert_eq!(&expired[1..3], &[0, 0], "AbnormalStatusUpdate count = 0 once expired");

        let p = &world.players[&2001];
        assert!(p.buffs.is_empty());
        assert_eq!(p.p_def, 80, "P.Def restored after the buff expired");
    }

    fn magic_skill_use_body(magic_id: i32, ctrl: bool) -> Vec<u8> {
        let mut w = PacketWriter::new();
        w.write_i32(magic_id);
        w.write_i32(if ctrl { 1 } else { 0 });
        w.write_u8(0); // shiftPressed
        w.into_bytes()
    }

    /// The `SystemMessage` id of a packet (opcode 0x62 + LE i16 id).
    fn sm_id(pkt: &[u8]) -> i16 {
        assert_eq!(pkt[0], server_packets::opcodes::SYSTEM_MESSAGE, "not a SystemMessage: 0x{:02x}", pkt[0]);
        i16::from_le_bytes([pkt[1], pkt[2]])
    }

    /// A world with a mage-ish class-0 template (m.atk/m.def/cast speed set,
    /// level-5 HP/MP/CP tables) and three castable skills: a Wind-Strike-like
    /// nuke (1177, `EnemyOnly`, `MagicalAttack` power 12, 10 s reuse), a
    /// Battle-Heal-like heal (1015, `Target`, power 83), and a Might-like
    /// buff-on-other (1068, `Target`, P.Atk +8%).
    fn cast_test_world() -> (
        World,
        db::CmdRx,
        tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
    ) {
        use crate::model::skill::{Skill, SkillEffect, StatModifierEffect};
        use crate::model::stats::{Stat, StatModifierType};

        let (link_tx, link_rx) = tokio::sync::mpsc::unbounded_channel();
        let (db_tx, db_rx) = tokio::sync::mpsc::unbounded_channel();

        let mut hp_table = vec![0.0; 90];
        let mut mp_table = vec![0.0; 90];
        let mut cp_table = vec![0.0; 90];
        hp_table[5] = 100.0;
        mp_table[5] = 50.0;
        cp_table[5] = 100.0;
        let template = crate::data::player_template::PlayerTemplate {
            class_id: 0,
            base_str: 40,
            base_dex: 30,
            base_con: 43,
            base_int: 21,
            base_wit: 11,
            base_men: 25,
            base_p_atk: 100,
            base_m_atk: 100,
            base_m_def: 60,
            base_p_atk_spd: 300,
            base_m_atk_spd: 333,
            // base_m_crit_rate stays 0 → magic crits can never roll, keeping
            // damage/heal numbers deterministic.
            hp_table,
            mp_table,
            cp_table,
            ..Default::default()
        };
        let mut data = GameData::for_test();
        data.player_templates = crate::data::PlayerTemplateData::from_vec(vec![template]);

        let base = Skill {
            id: 0,
            level: 1,
            name: String::new(),
            operate_type: OperateType::Active,
            target_type: TargetType::Other,
            magic_type: 1,
            effect_point: 0,
            cast_range: 600,
            effect_range: 1100,
            hit_time: 4000,
            hit_cancel_time: 0.0,
            cool_time: 0,
            reuse_delay: 0,
            mp_consume: 7,
            mp_initial_consume: 2,
            hp_consume: 0,
            abnormal_time: 0,
            abnormal_level: 0,
            abnormal_type: "NONE".into(),
            effects: vec![],
        };
        data.skill_data.insert_for_test(Skill {
            id: 1177,
            name: "Wind Strike".into(),
            target_type: TargetType::EnemyOnly,
            effect_point: -92,
            reuse_delay: 10_000,
            effects: vec![SkillEffect::MagicalAttack { power: 12.0 }],
            ..base.clone()
        });
        data.skill_data.insert_for_test(Skill {
            id: 1015,
            name: "Battle Heal".into(),
            target_type: TargetType::Target,
            effect_point: 100,
            hit_time: 1000,
            effects: vec![SkillEffect::Heal { power: 83.0 }],
            ..base.clone()
        });
        data.skill_data.insert_for_test(Skill {
            id: 1068,
            name: "Might".into(),
            target_type: TargetType::Target,
            effect_point: 100,
            hit_time: 1000,
            abnormal_time: 20,
            abnormal_level: 1,
            abnormal_type: "PA_UP".into(),
            effects: vec![SkillEffect::StatModifier(StatModifierEffect {
                stat: Stat::PhysicalAttack,
                mode: StatModifierType::Per,
                amount: 8.0,
            })],
            ..base.clone()
        });
        // A slow self-buff (10 s cast) used as the interruptible victim cast.
        data.skill_data.insert_for_test(Skill {
            id: 91,
            name: "Slow Aura".into(),
            target_type: TargetType::Self_,
            cast_range: 0,
            effect_range: 0,
            hit_time: 10_000,
            abnormal_time: 20,
            abnormal_type: "PD_UP".into(),
            effects: vec![SkillEffect::StatModifier(StatModifierEffect {
                stat: Stat::PhysicalDefence,
                mode: StatModifierType::Per,
                amount: 8.0,
            })],
            ..base
        });

        (World::new(link_tx, 7, 3, 0, data, db_tx.clone()), db_rx, link_rx)
    }

    /// An `InGame` level-5 player knowing every `cast_test_world` skill, with
    /// full MP/CP.
    fn ingame_caster(
        world: &mut World,
        client_id: u32,
        object_id: i32,
        x: i32,
        y: i32,
    ) -> tokio::sync::mpsc::UnboundedReceiver<Vec<u8>> {
        let mut chr = dummy_char(object_id, &format!("P{object_id}"));
        chr.level = 5;
        chr.cur_mp = 50.0;
        chr.cur_hp = 100.0;
        chr.x = x;
        chr.y = y;
        chr.z = 0;
        chr.skills = vec![(1177, 1), (1015, 1), (1068, 1), (91, 1)];
        let player = Player::from_char(&world.data, &chr);
        let (out_tx, out_rx) = tokio::sync::mpsc::unbounded_channel();
        let s = Session::new(client_id, out_tx, "127.0.0.1:1".parse().unwrap())
            .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
            .into_lobby(vec![])
            .into_entering(player);
        let (session, player) = s.into_ingame();
        world.players.insert(player.object_id, player);
        world.clients.insert(client_id, ClientSession::InGame(session));
        world.players.get_mut(&object_id).unwrap().cur_cp = 100.0;
        out_rx
    }

    fn drain(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while let Ok(p) = rx.try_recv() {
            out.push(p);
        }
        out
    }

    /// Advance the world one tick at a time, firing due tasks each tick like
    /// the real loop — a task scheduled by another task (launch → finish)
    /// would never fire under a single big jump + one drain.
    fn advance_ticks(world: &mut World, n: u64) {
        for _ in 0..n {
            world.tick += 1;
            apply_due_tasks(world);
        }
    }

    /// The full happy path of an offensive cast on another player, phase by
    /// phase, plus the reuse gate on an immediate re-cast: exact
    /// Formulas.calcMagicDam damage, CP absorbed before HP, the SM
    /// 2261/2262 damage messages, and every broadcast reaching the target.
    #[test]
    fn cast_enemy_nuke_deals_damage_and_enforces_reuse() {
        let (mut world, ..) = cast_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);

        handle_action(&mut world, 1, &action_body(3002, 0));
        drain(&mut a_rx);
        drain(&mut b_rx);

        // Without ctrl an unflagged player is not a valid enemy target.
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
        assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::INVALID_TARGET);
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
        assert!(world.players[&3001].cast.is_none());

        // With ctrl: ExRotation (face target) + initial-MP StatusUpdate +
        // MagicSkillUse to everyone, YOU_USE_S1 + SetupGauge to the caster.
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::EX);
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE);
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_USE);
        assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::YOU_USE_S1);
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::SETUP_GAUGE);
        assert!(a_rx.try_recv().is_err());
        assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::EX);
        assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_USE);
        assert!(b_rx.try_recv().is_err());
        assert_eq!(world.players[&3001].cur_mp, 48.0, "50 - mpInitialConsume(2)");

        // Launch at hit = 4000/1.0 − 500 = 3500 ms = 35 ticks.
        world.tick += 35;
        apply_due_tasks(&mut world);
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);
        assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);

        // Finish 500 ms later: MP consume, damage, messages, status updates.
        world.tick += 5;
        apply_due_tasks(&mut world);

        let m_atk = world.players[&3001].m_atk as f64;
        let m_def = world.players[&3002].m_def as f64;
        let damage = formulas::calc_magic_dam(m_atk, m_def, 12.0, false);
        assert!(damage > 100.0, "sanity: the nuke must overflow B's CP ({damage})");
        {
            let b = &world.players[&3002];
            assert_eq!(b.cur_cp, 0.0, "CP absorbs first");
            assert!((b.cur_hp - (100.0 - (damage - 100.0))).abs() < 1e-9, "HP takes the rest");
        }
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE); // MP consume
        assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2);
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE); // B's CP/HP
        assert!(a_rx.try_recv().is_err());
        assert_eq!(sm_id(&b_rx.try_recv().unwrap()), server_packets::sm_ids::C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2);
        assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE);
        assert!(b_rx.try_recv().is_err());
        assert!(world.players[&3001].cast.is_none(), "coolTime 0 frees the slot");

        // Immediate re-cast: 10 s reuse still has 6 s left → SM 2303 + fail.
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
        assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::S2_SECONDS_REMAINING_FOR_REUSE);
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
        assert!(world.players[&3001].cast.is_none());
        assert!(b_rx.try_recv().is_err(), "rejected cast must not broadcast");
    }

    /// Out-of-cast-range requests are rejected before anything is announced.
    #[test]
    fn cast_out_of_range_rejected() {
        let (mut world, ..) = cast_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let mut b_rx = ingame_caster(&mut world, 2, 3002, 700, 0); // castRange 600
        handle_action(&mut world, 1, &action_body(3002, 0));
        drain(&mut a_rx);
        drain(&mut b_rx);

        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
        assert!(a_rx.try_recv().is_err());
        assert!(b_rx.try_recv().is_err());
        assert!(world.players[&3001].cast.is_none());
    }

    /// A nuke can never kill while there's no death system: HP floors at 1.
    #[test]
    fn nuke_never_kills_hp_clamped_at_1() {
        let (mut world, ..) = cast_test_world();
        let mut _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let mut _b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
        {
            let b = world.players.get_mut(&3002).unwrap();
            b.cur_cp = 0.0;
            b.cur_hp = 5.0;
        }
        handle_action(&mut world, 1, &action_body(3002, 0));
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
        advance_ticks(&mut world, 45);
        assert_eq!(world.players[&3002].cur_hp, 1.0);
    }

    /// Esc aborts a pre-launch cast: `MagicSkillCanceled` broadcast (self
    /// included) + `ActionFailed`, the stale phase tasks no-op, the reuse
    /// registered at cast start still stands (Java semantics), and once it
    /// runs out the caster can cast again.
    #[test]
    fn esc_aborts_cast_and_stale_tasks_noop() {
        let (mut world, ..) = cast_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
        handle_action(&mut world, 1, &action_body(3002, 0));
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
        drain(&mut a_rx);
        drain(&mut b_rx);
        let mp_after_start = world.players[&3001].cur_mp;

        // Esc (targetLost=false: abort only, keep the target).
        handle_request_target_canceld(&mut world, 1, &target_canceld_body(false));
        assert!(world.players[&3001].cast.is_none());
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_CANCELED);
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
        assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_CANCELED);

        // The scheduled launch is stale: nothing fires, nothing lands.
        world.tick += 40;
        apply_due_tasks(&mut world);
        assert!(a_rx.try_recv().is_err());
        assert!(b_rx.try_recv().is_err());
        assert_eq!(world.players[&3001].cur_mp, mp_after_start, "no finish consume after abort");
        assert_eq!(world.players[&3002].cur_hp, 100.0);

        // Reuse (registered at cast start) still blocks, then expires.
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
        assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::S2_SECONDS_REMAINING_FOR_REUSE);
        drain(&mut a_rx);
        world.tick += 60;
        apply_due_tasks(&mut world);
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
        assert!(world.players[&3001].cast.is_some(), "castable again after reuse expiry");
    }

    /// The launch-phase `effectRange` re-check: a target who got away between
    /// start and launch cancels the cast quietly (SM 748, no cancel packet —
    /// Java `stopCasting(false)`).
    #[test]
    fn effect_range_recheck_cancels_when_target_moves_away() {
        let (mut world, ..) = cast_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
        handle_action(&mut world, 1, &action_body(3002, 0));
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
        drain(&mut a_rx);
        drain(&mut b_rx);

        world.players.get_mut(&3002).unwrap().x = 5000; // > effectRange 1100

        world.tick += 40;
        apply_due_tasks(&mut world);
        assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED);
        assert!(a_rx.try_recv().is_err(), "no MagicSkillLaunched, no cancel packet");
        assert!(b_rx.try_recv().is_err());
        assert!(world.players[&3001].cast.is_none());
        assert_eq!(world.players[&3002].cur_hp, 100.0);
    }

    /// A heal on another player: Heal.java's `power + sqrt(2·mAtk)` amount,
    /// overheal-clamped, SM 1067 to the healed target.
    #[test]
    fn heal_on_other_restores_hp_with_formula() {
        let (mut world, ..) = cast_test_world();
        let mut _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
        world.players.get_mut(&3002).unwrap().cur_hp = 50.0;
        handle_action(&mut world, 1, &action_body(3002, 0));
        drain(&mut b_rx);

        // TARGET-type skills need no ctrl.
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
        assert!(world.players[&3001].cast.is_some());
        drain(&mut b_rx); // ExRotation + MagicSkillUse

        advance_ticks(&mut world, 10); // hit 500 ms + cancel 500 ms

        let heal = formulas::calc_heal(83.0, world.players[&3001].m_atk as f64, false);
        assert!(heal > 50.0, "sanity: heal ({heal}) overflows the missing 50 HP");
        assert_eq!(world.players[&3002].cur_hp, 100.0, "overheal clamped at max HP");
        assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);
        assert_eq!(sm_id(&b_rx.try_recv().unwrap()), server_packets::sm_ids::S2_HP_HAS_BEEN_RESTORED_BY_C1);
        assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE);
    }

    /// A buff cast on another player lands on the *target*: their stats pump,
    /// their client gets the AbnormalStatusUpdate, and the expiry restores.
    #[test]
    fn buff_on_other_player_lands_on_target() {
        let (mut world, ..) = cast_test_world();
        let mut _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
        handle_action(&mut world, 1, &action_body(3002, 0));
        drain(&mut b_rx);
        let base_p_atk = world.players[&3002].p_atk;

        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, false));
        advance_ticks(&mut world, 10);

        {
            let b = &world.players[&3002];
            assert_eq!(b.buffs.len(), 1);
            assert!(b.p_atk > base_p_atk, "P.Atk pumped by Might (+8%)");
        }
        let b_packets = drain(&mut b_rx);
        assert!(
            b_packets.iter().any(|p| p[0] == 0x85),
            "target's client gets the AbnormalStatusUpdate"
        );
        assert!(world.players[&3001].buffs.is_empty(), "nothing lands on the caster");

        advance_ticks(&mut world, 200);
        let b = &world.players[&3002];
        assert!(b.buffs.is_empty());
        assert_eq!(b.p_atk, base_p_atk, "restored after expiry");
    }

    /// Finish-phase MP shortfall stops the cast quietly: SM 24 +
    /// ActionFailed to the caster, but no `MagicSkillCanceled` (Java
    /// `stopCasting(false)`), and no effects land.
    #[test]
    fn finish_phase_mp_shortfall_aborts_quietly() {
        let (mut world, ..) = cast_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
        handle_action(&mut world, 1, &action_body(3002, 0));
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
        drain(&mut a_rx);
        drain(&mut b_rx);

        world.players.get_mut(&3001).unwrap().cur_mp = 0.0;

        advance_ticks(&mut world, 45);
        // Launch fires normally (range fine), then the finish fails on MP.
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);
        assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::NOT_ENOUGH_MP);
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
        assert!(a_rx.try_recv().is_err());
        assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);
        assert!(b_rx.try_recv().is_err(), "no cancel packet on a quiet stop");
        assert!(world.players[&3001].cast.is_none());
        assert_eq!(world.players[&3002].cur_hp, 100.0, "no damage landed");
    }

    /// `RequestSkillCoolTime` reports the remaining reuse of a just-cast
    /// skill.
    #[test]
    fn skill_cool_time_lists_remaining_reuse() {
        let (mut world, ..) = cast_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));
        drain(&mut a_rx);

        on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_COOL_TIME]);
        let pkt = a_rx.try_recv().unwrap();
        assert_eq!(pkt[0], server_packets::opcodes::SKILL_COOL_TIME);
        assert_eq!(i32::from_le_bytes(pkt[1..5].try_into().unwrap()), 0, "Slow Aura has no reuse delay");

        // A reuse with 6 s left is reported with its total and remainder.
        world.players.get_mut(&3001).unwrap().reuses.insert(1177, (world.tick + 60, 10_000));
        on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_COOL_TIME]);
        let pkt = a_rx.try_recv().unwrap();
        assert_eq!(pkt[0], server_packets::opcodes::SKILL_COOL_TIME);
        assert_eq!(i32::from_le_bytes(pkt[1..5].try_into().unwrap()), 1);
        assert_eq!(i32::from_le_bytes(pkt[5..9].try_into().unwrap()), 1177);
        assert_eq!(i32::from_le_bytes(pkt[9..13].try_into().unwrap()), 1, "known level");
        assert_eq!(i32::from_le_bytes(pkt[13..17].try_into().unwrap()), 10, "total seconds");
        assert_eq!(i32::from_le_bytes(pkt[17..21].try_into().unwrap()), 6, "remaining seconds");
    }

    /// Incoming magic damage can break a victim's pre-launch cast
    /// (`Formulas.calcAtkBreak`): `MagicSkillCanceled` broadcast + SM 27 to
    /// the victim, and their stale launch task no-ops.
    #[test]
    fn incoming_magic_damage_can_break_precast() {
        let (mut world, ..) = cast_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);

        // B starts a slow self-cast (hit = 9500 ms = 95 ticks).
        handle_request_magic_skill_use(&mut world, 2, &magic_skill_use_body(91, false));
        assert!(world.players[&3002].cast.is_some());

        // A nukes B; the nuke lands at 40 ticks, well before B's launch.
        handle_action(&mut world, 1, &action_body(3002, 0));
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
        drain(&mut a_rx);
        drain(&mut b_rx);

        // Force the rolls: crit d1000 (rate 0 → miss regardless), then the
        // atk-break d100 → 0 always breaks (rate ≥ 1).
        world.forced_rolls.extend([999, 0]);

        advance_ticks(&mut world, 45);

        assert!(world.players[&3002].cast.is_none(), "victim's cast broken");
        let b_packets = drain(&mut b_rx);
        assert!(b_packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_CANCELED));
        assert!(b_packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::YOUR_CASTING_HAS_BEEN_INTERRUPTED));
        let a_packets = drain(&mut a_rx);
        assert!(a_packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_CANCELED));

        // B's stale launch task fires and no-ops: no buff ever lands.
        advance_ticks(&mut world, 60);
        assert!(world.players[&3002].buffs.is_empty());
    }

    /// Puts a bare `Player` (built from `dummy_char`) straight into `InGame`,
    /// the same session-transition chain the other tests use, and returns its
    /// outbound packet receiver.
    fn ingame_player(
        world: &mut World,
        client_id: u32,
        object_id: i32,
        x: i32,
        y: i32,
        z: i32,
    ) -> tokio::sync::mpsc::UnboundedReceiver<Vec<u8>> {
        let mut chr = dummy_char(object_id, &format!("P{object_id}"));
        chr.x = x;
        chr.y = y;
        chr.z = z;
        let player = Player::from_char(&world.data, &chr);
        let (out_tx, out_rx) = tokio::sync::mpsc::unbounded_channel();
        let s = Session::new(client_id, out_tx, "127.0.0.1:1".parse().unwrap())
            .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
            .into_lobby(vec![])
            .into_entering(player);
        let (session, player) = s.into_ingame();
        world.players.insert(player.object_id, player);
        world.clients.insert(client_id, ClientSession::InGame(session));
        out_rx
    }

    fn action_body(object_id: i32, action_id: u8) -> Vec<u8> {
        let mut w = PacketWriter::new();
        w.write_i32(object_id);
        w.write_i32(0); // origin_x — unused
        w.write_i32(0); // origin_y — unused
        w.write_i32(0); // origin_z — unused
        w.write_u8(action_id);
        w.into_bytes()
    }

    fn target_canceld_body(target_lost: bool) -> Vec<u8> {
        let mut w = PacketWriter::new();
        w.write_i16(if target_lost { 1 } else { 0 });
        w.into_bytes()
    }

    fn move_body(target: (i32, i32, i32), origin: (i32, i32, i32), movement_mode: i32) -> Vec<u8> {
        let mut w = PacketWriter::new();
        w.write_i32(target.0);
        w.write_i32(target.1);
        w.write_i32(target.2);
        w.write_i32(origin.0);
        w.write_i32(origin.1);
        w.write_i32(origin.2);
        w.write_i32(movement_mode);
        w.into_bytes()
    }

    /// `Action` selects a player target: the selector gets `MyTargetSelected`
    /// + a `StatusUpdate` (target's HP) + the `ActionFailed` terminator; the
    /// target itself gets `TargetSelected` (never `MyTargetSelected`). A
    /// repeat click on the same target is a no-op (only `ActionFailed`).
    /// `RequestTargetCanceld{target_lost:true}` clears it and broadcasts
    /// `TargetUnselected` to everyone including the canceller (Java uses
    /// includeSelf=true there; without it the client keeps its target).
    #[test]
    fn action_selects_switches_and_cancels_target() {
        let (mut world, ..) = test_world();
        let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
        let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);

        handle_action(&mut world, 1, &action_body(3002, 0));

        assert_eq!(world.players[&3001].target, Some(3002));
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::MY_TARGET_SELECTED);
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE);
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
        assert!(a_rx.try_recv().is_err(), "no extra packets to the selector");

        assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::TARGET_SELECTED);
        assert!(b_rx.try_recv().is_err(), "target never gets MyTargetSelected");

        // Re-click the same target: no-op besides the ActionFailed terminator.
        handle_action(&mut world, 1, &action_body(3002, 0));
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
        assert!(a_rx.try_recv().is_err());
        assert!(b_rx.try_recv().is_err(), "no TargetSelected rebroadcast on re-click");

        // Cancel.
        handle_request_target_canceld(&mut world, 1, &target_canceld_body(true));
        assert_eq!(world.players[&3001].target, None);
        assert_eq!(
            a_rx.try_recv().unwrap()[0],
            server_packets::opcodes::TARGET_UNSELECTED,
            "canceller must receive TargetUnselected too"
        );
        assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::TARGET_UNSELECTED);

        // Self-click: same select path as any other player target (Java
        // routes self-clicks through `PlayerAction` too).
        handle_action(&mut world, 1, &action_body(3001, 0));
        assert_eq!(world.players[&3001].target, Some(3001));
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::MY_TARGET_SELECTED);
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE);
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
        assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::TARGET_SELECTED);
    }

    /// `MoveBackwardToLocation` starts a move: `move_data` is set, a
    /// `MoveToLocation` is sent to the mover (the client only starts walking
    /// on the server's confirmation) and broadcast to other players, and
    /// `movement::tick` interpolates the position over the precomputed tick
    /// count before snapping to the destination and clearing `move_data` on
    /// arrival.
    #[test]
    fn move_backward_to_location_interpolates_and_arrives() {
        let (mut world, ..) = test_world();
        let mut mover_rx = ingame_player(&mut world, 1, 4001, 0, 0, 0);
        let mut bystander_rx = ingame_player(&mut world, 2, 4002, 500, 500, 0);
        world.players.get_mut(&4001).unwrap().run_spd = 100;
        world.players.get_mut(&4001).unwrap().running = true;

        handle_move_backward_to_location(&mut world, 1, &move_body((1000, 0, 0), (0, 0, 0), 1));

        assert_eq!(mover_rx.try_recv().unwrap()[0], server_packets::opcodes::MOVE_TO_LOCATION);
        assert!(mover_rx.try_recv().is_err(), "exactly one packet to the mover");
        assert_eq!(bystander_rx.try_recv().unwrap()[0], server_packets::opcodes::MOVE_TO_LOCATION);

        let total_ticks = world.players[&4001].move_data.as_ref().unwrap().total_ticks;
        assert_eq!(total_ticks, 100, "distance 1000 / speed 100 * 10 ticks-per-sec");

        // Half way: linear interpolation.
        world.tick += total_ticks / 2;
        crate::model::movement::tick(&mut world);
        let p = &world.players[&4001];
        assert_eq!((p.x, p.y, p.z), (500, 0, 0));
        assert!(p.move_data.is_some());

        // Arrival: snapped exactly, move_data cleared, no StopMove needed.
        world.tick += total_ticks / 2;
        crate::model::movement::tick(&mut world);
        let p = &world.players[&4001];
        assert_eq!((p.x, p.y, p.z), (1000, 0, 0));
        assert!(p.move_data.is_none());
    }

    /// Java's `MoveBackwardToLocation` early-returns with `StopMove` +
    /// `ActionFailed` when the client's echoed origin equals its target
    /// (used by the client as an explicit "stop" signal) — no movement state
    /// is set.
    #[test]
    fn move_backward_to_location_same_origin_and_target_sends_stop_move() {
        let (mut world, ..) = test_world();
        let mut rx = ingame_player(&mut world, 1, 5001, 10, 20, 30);

        handle_move_backward_to_location(&mut world, 1, &move_body((100, 100, 100), (100, 100, 100), 1));

        assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::STOP_MOVE);
        assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
        assert!(world.players[&5001].move_data.is_none());
    }

    /// Region 20_18 covers world x,y ∈ [0, 32768): flat ground at z = 0 with
    /// a north-south wall at local cell x == 10 (world x 160..176) — 200
    /// units tall, not enterable, and the approach cells block their east
    /// exit (how real geodata encodes walls).
    fn install_wall_region(world: &mut World) {
        use crate::geo::{synthetic_region, NSWE_ALL, NSWE_EAST};
        world.geo.set_region(
            20,
            18,
            synthetic_region(|x, _y| {
                if x == 10 {
                    (200, 0)
                } else if x == 9 {
                    (0, NSWE_ALL & !NSWE_EAST)
                } else {
                    (0, NSWE_ALL)
                }
            }),
        );
    }

    fn validate_position_body(x: i32, y: i32, z: i32, heading: i32) -> Vec<u8> {
        let mut w = PacketWriter::new();
        w.write_i32(x);
        w.write_i32(y);
        w.write_i32(z);
        w.write_i32(heading);
        w.write_i32(0); // vehicle id
        w.into_bytes()
    }

    /// A click past a geodata wall is clamped to the last walkable cell
    /// (`GeoEngine.getValidLocation` in `Creature.moveToLocation`): the
    /// stored move and the broadcast `MoveToLocation` both carry the clamped
    /// destination, not the client's.
    #[test]
    fn move_destination_is_clamped_by_geodata() {
        let (mut world, ..) = test_world();
        install_wall_region(&mut world);
        let mut mover_rx = ingame_player(&mut world, 1, 4001, 8, 8, 0); // cell 0
        world.players.get_mut(&4001).unwrap().run_spd = 100;

        // Click to cell 20 (x = 328), on the far side of the wall at cell 10.
        handle_move_backward_to_location(&mut world, 1, &move_body((328, 8, 0), (8, 8, 0), 1));

        let md = world.players[&4001].move_data.clone().expect("move must start");
        assert_eq!((md.dest_x, md.dest_y), (152, 8), "clamped to cell 9, before the wall");
        let pkt = mover_rx.try_recv().unwrap();
        assert_eq!(pkt[0], server_packets::opcodes::MOVE_TO_LOCATION);
        let dest_x = i32::from_le_bytes(pkt[5..9].try_into().unwrap());
        assert_eq!(dest_x, 152, "MoveToLocation carries the clamped destination");
    }

    /// Standing right at the wall, a click into it clamps the whole path away
    /// (distance < 1) — Java cancels the movement with `ActionFailed`.
    #[test]
    fn move_into_wall_from_adjacent_cell_is_cancelled() {
        let (mut world, ..) = test_world();
        install_wall_region(&mut world);
        let mut mover_rx = ingame_player(&mut world, 1, 4001, 152, 8, 0); // cell 9
        world.players.get_mut(&4001).unwrap().run_spd = 100;

        handle_move_backward_to_location(&mut world, 1, &move_body((168, 8, 0), (152, 8, 0), 1));

        assert!(world.players[&4001].move_data.is_none(), "no movement into the wall");
        assert_eq!(mover_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
        assert!(mover_rx.try_recv().is_err());
    }

    /// The target-handler geodata check: a wall between caster and target
    /// fails the cast with SM 181 (`CANNOT_SEE_TARGET`); with the target on
    /// the caster's side the same cast starts normally.
    #[test]
    fn cast_blocked_by_wall_sends_cannot_see_target() {
        let (mut world, ..) = cast_test_world();
        install_wall_region(&mut world);
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 8, 8);
        let _b_rx = ingame_caster(&mut world, 2, 3002, 328, 8); // across the wall

        handle_action(&mut world, 1, &action_body(3002, 0));
        drain(&mut a_rx);

        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
        assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::CANNOT_SEE_TARGET);
        assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
        assert!(world.players[&3001].cast.is_none());

        // Same side of the wall: the cast starts.
        world.players.get_mut(&3002).unwrap().x = 72; // cell 4
        handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
        assert!(world.players[&3001].cast.is_some());
    }

    /// `ValidatePosition` reconciliation, one branch at a time: a plausible
    /// climb (|dz| 200..1500, near the last reported client z) adopts the
    /// client z; moderate 2D drift is answered with `ValidateLocation` and
    /// the server keeps its position; a desync beyond one second of movement
    /// snaps the server to the client, geodata-correcting z downwards.
    #[test]
    fn validate_position_reconciles_client_and_server() {
        let (mut world, ..) = test_world();
        install_wall_region(&mut world);
        let mut rx = ingame_player(&mut world, 1, 4001, 1000, 1000, 0);
        {
            let p = world.players.get_mut(&4001).unwrap();
            p.run_spd = 600;
            p.running = true;
        }

        // Climb: z 0 → 300 with matching client-z history — trusted, silent.
        handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, 300, 0));
        assert_eq!(world.players[&4001].z, 300);
        assert!(rx.try_recv().is_err(), "no correction for a trusted climb");

        // Drift: diffSq 270400 ∈ (250000, 360000), within move speed (600) —
        // server answers ValidateLocation and stays put.
        handle_validate_position(&mut world, 1, &validate_position_body(1520, 1000, 300, 0));
        assert_eq!(world.players[&4001].x, 1000, "server position kept on drift");
        let pkt = rx.try_recv().unwrap();
        assert_eq!(pkt[0], server_packets::opcodes::VALIDATE_LOCATION);
        assert!(rx.try_recv().is_err());

        // Desync: 2000 units in one report — snap to the client, with z
        // pulled onto the geodata ground (server was above the client).
        handle_validate_position(&mut world, 1, &validate_position_body(3000, 1000, 0, 0));
        let p = &world.players[&4001];
        assert_eq!((p.x, p.y, p.z), (3000, 1000, 0), "snapped, z on the geodata floor");
        assert_eq!((p.client_x, p.client_y, p.client_z), (3000, 1000, 0));
    }
}
