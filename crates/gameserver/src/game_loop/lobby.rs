//! Lobby / character-management handlers: the AuthLogin → EnterWorld stretch
//! (character list, name check, create/delete/restore/select).

use tracing::info;

use crate::db::{self, NewCharacter};
use crate::loginlink::LoginLinkCommand;
use crate::network::client_packets::{self as cp, AuthLogin, CharacterCreate};
use crate::network::server_packets;
use crate::session::{ClientSession, SessionKey};
use crate::world::{WaitingClient, World};

/// Port of `RequestCharacterNameCreatable.runImpl`: validate the name, then ask
/// the DB whether it already exists; the reply is `ExIsCharNameCreatable`.
pub(crate) fn handle_request_character_name_creatable(world: &mut World, client_id: u32, ex_body: &[u8]) {
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
pub(crate) fn handle_new_character(world: &mut World, client_id: u32) {
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
pub(crate) fn handle_character_create(world: &mut World, client_id: u32, body: &[u8]) {
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
pub(crate) fn resolve_initial_items(world: &World, class_id: i32) -> Vec<db::NewItem> {
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
pub(crate) fn handle_character_delete(world: &mut World, client_id: u32, body: &[u8]) {
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
pub(crate) fn handle_character_restore(world: &mut World, client_id: u32, body: &[u8]) {
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
pub(crate) fn handle_character_select(world: &mut World, client_id: u32, body: &[u8]) {
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
pub(crate) fn handle_enter_world(world: &mut World, client_id: u32) {
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

    let object_id = player.object_id;
    world.players.insert(object_id, player);
    info!("GameLoop: '{name}' entered the world ({} online).", world.players.len());
    world.clients.insert(client_id, ClientSession::InGame(session));
    // Java `spawnMe` → `World.addVisibleObject`: mutual CharInfo with every
    // player visible from the spawn region.
    super::visibility::on_enter_world(world, client_id, object_id);

    // Java `EnterWorld`: a character that logged out dead comes back dead —
    // re-open the death dialog.
    if world.players.get(&object_id).is_some_and(|p| p.dead) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(crate::network::server_packets::die(object_id, true));
        }
    }
}


/// Port of `clientpackets/AuthLogin.runImpl`: register the account on this game
/// server and ask the login server to validate the session key.
pub(crate) fn handle_auth_login(world: &mut World, client_id: u32, body: &[u8]) {
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

