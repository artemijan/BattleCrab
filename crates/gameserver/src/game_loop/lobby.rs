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
pub(crate) fn handle_request_character_name_creatable(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
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
    // No equipped gear yet (initial items are added below, then equipped at
    // enter-world where `from_char` recomputes max with the paperdoll).
    let no_mods = crate::model::components::StatModifiers::default();
    let max_hp = crate::model::calc_max_hp(&world.data, template, 1, None, &no_mods) as i32;
    let max_mp = crate::model::calc_max_mp(&world.data, template, 1, None, &no_mods) as i32;
    // Initial skills for the class (Java: getAvailableSkills at level 1).
    let skills = world.data.skill_trees.initial_skills(pkt.class_id);
    let items = resolve_initial_items(world, pkt.class_id);
    let (shortcuts, macros) = resolve_initial_shortcuts(world, pkt.class_id, &skills);
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
        shortcuts,
        macros,
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

/// Port of `InitialShortcutData.registerAllShortcuts`' filtering half —
/// global + class `initialShortcuts.xml` entries, minus SKILL slots the new
/// character won't know and MACRO slots without an (enabled) preset; the
/// referenced presets ride along for `character_macroses`. ITEM entries keep
/// their item id — the DB thread resolves the created item's object id (and
/// drops entries whose item the class didn't receive), see
/// `db::create_character`.
pub(crate) fn resolve_initial_shortcuts(
    world: &World,
    class_id: i32,
    initial_skills: &[(i32, i32)],
) -> (Vec<db::NewShortcut>, Vec<crate::model::shortcut::Macro>) {
    use crate::model::shortcut::ShortcutType;
    let data = &world.data.initial_shortcuts;
    let mut shortcuts = Vec::new();
    let mut macros: Vec<crate::model::shortcut::Macro> = Vec::new();
    for sc in data.global().iter().chain(data.for_class(class_id)) {
        match sc.kind {
            ShortcutType::Skill if !initial_skills.iter().any(|&(id, _)| id == sc.id) => continue,
            ShortcutType::Macro => match data.macro_preset(sc.id) {
                Some(preset) => {
                    if !macros.iter().any(|m| m.id == preset.id) {
                        macros.push(preset.clone());
                    }
                }
                None => continue,
            },
            _ => {}
        }
        shortcuts.push(db::NewShortcut {
            slot: sc.slot,
            page: sc.page,
            kind: sc.kind,
            id: sc.id,
            level: sc.level,
        });
    }
    (shortcuts, macros)
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
    let Some(slot) = cp::read_char_slot(body) else {
        return;
    };
    let ClientSession::InLobby(s) = (match world.clients.get(&client_id) {
        Some(cs) => cs,
        None => return,
    }) else {
        return;
    };
    let Some(mut chr) = s.char_at(slot).cloned() else {
        return;
    };
    // `ShortCuts.restoreMe`'s ITEM verification: `from_char` drops shortcuts
    // whose item left the inventory. Memory-first — the dropped shortcuts simply
    // aren't in the bundle, so the next flush's reconcile removes their rows; no
    // per-select DB delete.
    // Java `restoreCharData` → `checkPlayerSkills`: filter the DB-loaded skill
    // list against the character's level *before* building the `Player`, so
    // `from_char` folds the corrected passives (Spellcraft casting speed, etc.)
    // and the enter-world `UserInfo` is right the first time — no post-spawn
    // recompute. Panel shortcuts for changed skills are synced to match.
    filter_skills_on_select(world, &mut chr);
    let mut bundle = crate::model::Player::from_char(&world.data, &chr);
    // Java `restoreEffects` (skill-reuse half): re-arm persisted cooldowns off
    // the current game tick before the bundle enters the world.
    bundle.restore_reuses(&chr, world.tick, commons::util::now_millis());
    let selected = server_packets::char_selected(&bundle.view(), s.play_ok1(), 0);

    // Transition InLobby → Entering, holding the built Player bundle.
    if let Some(ClientSession::InLobby(s)) = world.clients.remove(&client_id) {
        let s = s.into_entering(bundle);
        s.send(selected);
        info!(
            "GameLoop: client {client_id} selected character '{}'.",
            s.player().player.name
        );
        world.clients.insert(client_id, ClientSession::Entering(s));
    }
}

/// Run [`super::death::maybe_skill_remove_on_delevel`] over a just-selected
/// character's DB-loaded skill list and reconcile its panel shortcuts: matching
/// SKILL slots follow a downgrade or drop with a removed skill (transform skills
/// 3080–3259 are kept, per Java `removeSkill`). Both the skill list and the
/// shortcut edits are persisted; the caller then builds the `Player` from the
/// corrected `chr`.
fn filter_skills_on_select(world: &World, chr: &mut crate::character::CharData) {
    use crate::model::shortcut::ShortcutType;
    let mut skills: std::collections::HashMap<i32, i32> = chr.skills.iter().copied().collect();
    let changes = super::death::maybe_skill_remove_on_delevel(world, chr.object_id, chr.class_id, chr.level, &mut skills);
    if changes.is_empty() {
        return;
    }
    chr.skills = skills.into_iter().collect();
    for (skill_id, action) in changes {
        match action {
            Some(new_level) => {
                for sc in chr.shortcuts.iter_mut().filter(|sc| sc.kind == ShortcutType::Skill && sc.id == skill_id) {
                    sc.level = new_level;
                }
            }
            None if !(3080..=3259).contains(&skill_id) => {
                chr.shortcuts.retain(|sc| !(sc.kind == ShortcutType::Skill && sc.id == skill_id));
            }
            None => {}
        }
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
    let (session, mut bundle) = s.into_ingame();
    // Clan-leader flag comes from the live clan table (Java reads it off
    // the restored `Clan` object) — fix it before the first UserInfo.
    if let Some(clan) = world.clans.get(&bundle.player.clan_id) {
        bundle.player.clan_leader = clan.leader_id == bundle.player.object_id;
        bundle.player.pledge_class = clan.pledge_class_of(bundle.player.object_id);
    } else {
        bundle.player.clan_leader = false;
        bundle.player.pledge_class = 0;
    }

    // `Player.rewardSkills` on char-load: grant any reachable skills the book
    // is missing (autoGet always; with `AutoLearnSkills`, every reachable class
    // skill). Runs before the skill/shortcut burst below so both reflect the
    // grants. The player isn't in `world.objects` yet, so we apply to `bundle`.
    let learned = {
        let granted = super::death::reward_skill_grants(
            &world.data,
            &world.cfg.character,
            bundle.player.class_id,
            bundle.player.level,
            &bundle.skills.0,
            bundle.player.is_gm(&world.data),
        );
        for &(id, lvl) in &granted {
            bundle.skills.0.insert(id, lvl);
            // Memory-first: the grant and any matching shortcut level-bump land
            // in the in-memory bundle only; they persist on the next flush.
            for sc in bundle.shortcuts.0.values_mut() {
                if sc.kind == crate::model::shortcut::ShortcutType::Skill && sc.id == id {
                    sc.level = lvl;
                }
            }
        }
        granted
            .iter()
            .map(|&(id, _)| id)
            .collect::<std::collections::HashSet<_>>()
            .len()
    };

    // Delevel skill corrections already ran at character select
    // (`filter_skills_on_select`), so `bundle` carries the filtered skills and
    // already-correct passive stats — nothing to redo here.

    let view = bundle.view();
    let player = &bundle.player;
    let name = player.name.clone();
    let data = &world.data;
    use crate::network::enter_world as ew;
    use crate::network::user_info::user_info;

    // The enter-world packet burst (EnterWorld.runImpl). Inventory real as
    // of G5, skills G6, shortcuts/macros G9.6, friends G10, quest lists
    // G11; henna/mail still empty (TODOs in `enter_world`).
    session.send(user_info(&view, data, &world.cfg.character, super::party::calculate_relation(world, view.p)));
    session.send(ew::ex_vitality_effect_info(player));
    session.send(server_packets::ex_ui_setting());
    // `MacroList.sendAllMacros` — one packet per stored macro (or one empty
    // LIST packet), in Java's position before the bookmark/item lists.
    for pkt in server_packets::send_all_macros(&bundle.macros) {
        session.send(pkt);
    }
    session.send(ew::ex_get_bookmark_info());
    session.send(ew::item_list(&bundle.inventory, data, false));
    session.send(ew::ex_quest_item_list(&bundle.inventory, data));
    session.send(server_packets::shortcut_init(&bundle.shortcuts));
    session.send(ew::ex_basic_action_list(data));
    session.send(ew::henna_info());
    // Clan skills aren't applied yet (the clan login hook runs after the player
    // is registered and re-sends the merged list) → empty clan set here.
    session.send(ew::skill_list(&bundle.skills, &crate::model::components::ClanSkills::default(), data));
    session.send(ew::acquire_skill_list(player, &bundle.skills, data));
    // Initial burst carries 0/0; `refresh_expertise_penalty` (after the player
    // is registered below) recomputes and resends if any gear is over-grade.
    session.send(ew::etc_status_update(0, 0, false));
    session.send(ew::ex_pledge_waiting_list_alarm());
    session.send(ew::ex_subjob_info(player));
    session.send(ew::ex_user_info_inven_weight(
        player.object_id,
        &bundle.inventory,
        data,
    ));
    session.send(ew::ex_adena_inven_count(&bundle.inventory));
    session.send(ew::ex_storage_max_count(player.race, &world.cfg.character));
    session.send(ew::ex_user_info_equip_slot(
        player.object_id,
        &bundle.inventory,
    ));
    session.send(ew::quest_list(&bundle.quests, &world.quests));
    session.send(ew::ex_rotation(player.object_id, bundle.position.heading));
    // `L2FriendList` — the real roster (Java sends it at this spot).
    session.send(super::friends::l2_friend_list_packet(
        world,
        &bundle.friends,
    ));
    session.send(server_packets::skill_cool_time(&bundle.reuses, world.tick));
    // `EnterWorld`: the recommendation panel state.
    session.send(server_packets::ex_vote_system_info(player.rec_left, player.rec_have));

    // Register the player in the world and re-send UserInfo (Java does both).
    session.send(user_info(&view, data, &world.cfg.character, super::party::calculate_relation(world, view.p)));
    // No ExSetCompassZoneCode here: Java's EnterWorld never sends one — the
    // first revalidateZone below pushes the real code (0x08–0x0F). Sending an
    // out-of-range code (e.g. 0) leaves the client in an unknown zone state
    // where it refuses to open the world map.
    session.send(ew::move_to_location(player.object_id, &bundle.position));
    for kind in 0..4 {
        session.send(ew::ex_auto_soul_shot(0, true, kind));
    }
    session.send(ew::abnormal_status_update(
        &crate::model::components::Buffs::default(),
        world.tick,
    ));
    session.send(ew::system_message(ew::SM_WELCOME));
    // `giveAvailableSkills` notice (only the `AutoLearnSkills` path shows it).
    if world.cfg.character.auto_learn_skills && learned > 0 {
        session.send(server_packets::system_message_with(
            server_packets::sm_ids::S1_TEXT,
            &[server_packets::SmParam::Text(format!(
                "You have learned {learned} new skills."
            ))],
        ));
    }

    let object_id = player.object_id;
    bundle.spawn_into(&mut world.objects);
    info!(
        "GameLoop: '{name}' entered the world ({} online).",
        world.objects.count::<crate::model::Player>()
    );
    world
        .clients
        .insert(client_id, ClientSession::InGame(session));
    // Java `EnterWorld` calls `refreshExpertisePenalty` (via `restoreCharData`
    // → equip listeners): a character wearing over-grade gear logs in already
    // penalized. Runs now that the player is registered; resends
    // EtcStatusUpdate + UserInfo only when there's an actual penalty.
    super::expertise::refresh_expertise_penalty(world, object_id);
    // Java `restoreCharData`/`addSkill` also pumps armor-conditioned passives
    // (Spellcraft/Magician's Movement) at enter-world: a robe-wearing mystic
    // logs in with the casting/attack-speed bonus already folded in.
    super::passive_skills::refresh_conditioned_passives(world, object_id);
    // Java `EnterWorld` sends `HennaInfo` in the burst — the worn-dye panel
    // (the dyes' stat bonus is already in the UserInfo this burst carried).
    super::henna::send_henna_info(world, client_id, object_id);
    // Java `EnterWorld.runImpl`'s GM branch: apply the configured default GM
    // state (builder-hide / invul / invis / silence / diet) before the spawn
    // broadcast, so an invisible GM is never described to nearby players.
    if world.objects.get_component::<crate::model::Player>(&object_id).is_some_and(|p| p.is_gm(&world.data)) {
        super::admin::apply_gm_startup(world, client_id, object_id);
    }
    // Java `spawnMe` → `World.addVisibleObject`: mutual CharInfo with every
    // player visible from the spawn region.
    super::visibility::on_enter_world(world, client_id, object_id);
    // Schedule the first periodic autosave (Java `PlayerAutoSaveTaskManager.add`)
    // one interval out; `game_loop::autosave_tick` flushes and reschedules it.
    let due = world.tick + world.cfg.character.character_data_store_interval_ticks;
    world.player_autosave_due.insert(object_id, due);
    // Java `restore` → `startRecoGiveTask`: the per-player fixed-rate task that
    // hands out recommendations-to-give (10 after 2 h, then 1 hourly).
    super::reco::start_reco_give_task(world, object_id);
    // Java `EnterWorld` → `player.revalidateZone(true)` — initial zone set +
    // compass code at the spawn point.
    super::zones::revalidate_zone(world, object_id, true);
    // "Your friend just logged in" + FriendStatus(ONLINE) to online friends.
    super::friends::on_enter_world(world, object_id);
    // Pledge window to the member + online ping to the rest of the clan.
    super::clans::on_enter_world(world, client_id, object_id);

    // Java `EnterWorld`: a character that logged out dead comes back dead —
    // re-open the death dialog.
    if world
        .objects
        .get_component::<crate::model::components::Vitals>(&object_id)
        .is_some_and(|v| v.dead)
    {
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
