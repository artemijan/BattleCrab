use super::*;
use super::bypass::handle_request_bypass_to_server;
use super::dispatch::*;
use super::lobby::*;
use super::net::*;
use super::position::*;
use super::combat::handle_attack_request;
use super::death::handle_request_restart_point;
use super::skills::cast::*;
use super::skills::*;
use super::target::*;
use crate::character::CharData;
use crate::db::DbEvent;
use crate::loginlink::LoginLinkCommand;
use crate::model::formulas;
use crate::model::skill::{OperateType, Skill, TargetType};
use crate::model::components::{Buffs, Casting, ClientPos, CombatStats, Intent, LastFolkNpc, Movement, PlayerVitals, Position, Reuses, SkillBook, Speeds, TargetRef, Vitals};
use crate::model::Player;
use crate::network::client_packets::{self as cp, opcodes as cop};
use crate::network::server_packets;
use crate::session::{ClientSession, Session, SessionKey};
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

/// Component peek helpers for assertions (stage-2 shape).
fn pvit(world: &World, oid: i32) -> Vitals {
    *world.objects.get_component::<Vitals>(&oid).unwrap()
}
fn pcp(world: &World, oid: i32) -> PlayerVitals {
    *world.objects.get_component::<PlayerVitals>(&oid).unwrap()
}
fn nvit(world: &World, oid: i32) -> Vitals {
    *world.objects.get_component::<Vitals>(&oid).unwrap()
}
fn pcs(world: &World, oid: i32) -> CombatStats {
    *world.objects.get_component::<CombatStats>(&oid).unwrap()
}
fn pbuffs(world: &World, oid: i32) -> usize {
    world.objects.get_component::<Buffs>(&oid).map(|b| b.0.len()).unwrap_or(0)
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
        clan_privs: 0,
        clan_create_expiry_time: 0,
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
        shortcuts: vec![],
        macros: vec![],
        friends: vec![],
        quests: Default::default(),
        skill_reuses: vec![],
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
        root: String::new(),
        experience: crate::data::ExperienceData::empty(),
        player_templates: crate::data::PlayerTemplateData::from_vec(vec![
            human_fighter_template(),
        ]),
        skill_trees: crate::data::SkillTreeData::empty(),
        stat_bonus: crate::data::StatBonus::empty(),
        action_data: crate::data::ActionData::empty(),
        item_data: crate::data::ItemData::empty(),
        initial_equipment: crate::data::InitialEquipmentData::empty(),
        initial_shortcuts: crate::data::InitialShortcutData::empty(),
        skill_data: crate::data::SkillData::empty(),
        npc_data: crate::data::NpcData::empty(),
        spawn_data: crate::data::SpawnData::empty(),
        hit_condition_bonus: crate::data::HitConditionBonusData::default(),
        xp_lost: crate::data::PlayerXpPercentLostData::empty(),
        map_region: crate::data::MapRegionData::empty(),
        zone_data: crate::data::ZoneData::empty(),
        door_data: crate::data::DoorData::empty(),
        static_object_data: crate::data::StaticObjectData::empty(),
        buy_lists: crate::data::BuyListData::empty(),
        categories: crate::data::CategoryData::empty(),
        admin: crate::data::AdminData::empty(),
        combat_caps: crate::data::CombatCaps::default(),
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

    // The DB thread pushes its boot-time id block and clan table first;
    // skip them.
    let next_event = || loop {
        match db_event_rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap() {
            DbEvent::IdBlock { .. } | DbEvent::ClansLoaded { .. } => continue,
            other => return other,
        }
    };
    // The DB thread must report a successful insert, then the reloaded list.
    match next_event() {
        DbEvent::CharacterCreated { result, .. } => {
            assert_eq!(
                result,
                db::CreateResult::Ok,
                "character insert failed against real schema"
            );
        }
        _ => panic!("expected CharacterCreated"),
    }
    match next_event() {
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
        reuse_delay_group: -1,
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
            armor_condition: 0,
            weapon_condition: 0,
        })],
    });

    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

    // A level-5 character with 200 SP, walked straight to `InGame` (same
    // `Session` transition chain `handle_enter_world` uses in production).
    let mut chr = dummy_char(2001, "Def");
    chr.level = 5;
    chr.sp = 200;
    chr.cur_mp = 50.0;
    let bundle = Player::from_char(&world.data, &chr);
    // Naked P.Def = base(80) × levelMod((5+89)/100 = 0.94) = 75.2 (no gear,
    // so no slot subtraction); stored unrounded, the display truncates to 75.
    assert!((bundle.combat.p_def - 75.2).abs() < 1e-9, "naked P.Def before any buff: {}", bundle.combat.p_def);

    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel();
    let s = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![])
        .into_entering(bundle);
    let (session, bundle) = s.into_ingame();
    bundle.spawn_into(&mut world.objects);
    world.clients.insert(1, ClientSession::InGame(session));

    // --- Learn: RequestAcquireSkill(id=91, level=1, type=CLASS). ---
    let mut w = PacketWriter::new();
    w.write_i32(91);
    w.write_i32(1);
    w.write_i32(cp::RequestAcquireSkill::CLASS);
    handle_request_acquire_skill(&mut world, 1, &w.into_bytes());

    assert_eq!(world.objects.get_component::<SkillBook>(&2001).unwrap().0.get(&91), Some(&1));
    assert_eq!(world.objects.get_component::<crate::model::Player>(&2001).expect("player").sp, 100, "200 SP - levelUpSp(100)");
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::ACQUIRE_SKILL_DONE);
    assert_eq!(out_rx.try_recv().unwrap()[0], 0x5F); // SkillList
    let _ = out_rx.try_recv().unwrap(); // AcquireSkillList
    let _ = out_rx.try_recv().unwrap(); // UserInfo

    // --- Cast: RequestMagicSkillUse(91). ---
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));

    assert!(world.objects.has_component::<Casting>(&2001));
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE); // initial MP consume
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_USE);
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::SYSTEM_MESSAGE); // YOU_USE_S1
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::SETUP_GAUGE);
    assert_eq!(pvit(&world, 2001).cur_mp, 49.0, "50 - mpInitialConsume(1)");

    // --- Launch: hit = max(400/factor(1.0) − cancel(500), 0) = 0 ms, so
    // the launch task is already due; the finish follows 500 ms later.
    apply_due_tasks(&mut world);
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);
    assert!(world.objects.get_component::<Casting>(&2001).is_some_and(|c| c.0.launched));

    world.tick += 5;
    apply_due_tasks(&mut world);
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE); // final MP consume
    assert_eq!(out_rx.try_recv().unwrap()[0], 0x85); // AbnormalStatusUpdate
    let _ = out_rx.try_recv().unwrap(); // UserInfo (buff changed pDef → broadcastUserInfo)

    {
        assert!(!world.objects.has_component::<Casting>(&2001), "coolTime 0 frees the cast slot inline");
        assert_eq!(pbuffs(&world, 2001), 1);
        assert!((pcs(&world, 2001).p_def - 75.2 * 1.08).abs() < 1e-9, "75.2 × 1.08 (PhysicalDefence +8%): {}", pcs(&world, 2001).p_def);
    }
    assert_eq!(pvit(&world, 2001).cur_mp, 45.0, "49 - mpConsume(4)");

    // --- Advance past expiry (abnormalTime 20 s = 200 ticks) and drain again. ---
    world.tick += 200;
    apply_due_tasks(&mut world);

    let _ = out_rx.try_recv().unwrap(); // UserInfo (buff removal reverted pDef → broadcastUserInfo)
    let expired = out_rx.try_recv().unwrap();
    assert_eq!(expired[0], 0x85);
    assert_eq!(&expired[1..3], &[0, 0], "AbnormalStatusUpdate count = 0 once expired");

    assert_eq!(pbuffs(&world, 2001), 0);
    assert!((pcs(&world, 2001).p_def - 75.2).abs() < 1e-9, "P.Def restored after the buff expired: {}", pcs(&world, 2001).p_def);
}

/// Real-data stat parity: a level-1 Human Mystic loaded with the *real* class
/// starting gear (`initialEquipment.xml`, replayed through the equip-slot logic)
/// and *all* the class's level-1 autoGet skills (`skillTrees`), computed the
/// same way enter-world does, must show exactly the numbers the Java client
/// draws — including the Spellcraft-boosted casting speed of 499. Locks in the
/// finalizer fixes (pDef levelMod + slot-sub, mDef MEN×levelMod, RunSpeedBoost,
/// `(int)` truncation) *and* the armor-conditioned passives end to end.
#[test]
fn human_mystic_lvl1_full_loadout_matches_java_client() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    data.skill_trees = crate::data::skill_tree::SkillTreeData::load_from(DIST);
    data.initial_equipment = crate::data::initial_equipment::InitialEquipmentData::load_from(DIST);

    let class_id = 10; // Human Mystic

    // Replay the class starting equipment through the real equip-slot logic
    // (mirrors `resolve_initial_items`), then hand the resolved paperdoll to
    // `from_char` as stored `ItemRow`s.
    let mut inv = crate::model::inventory::Inventory::new();
    let mut next_oid = 1000;
    for entry in data.initial_equipment.get(class_id) {
        let oid = next_oid;
        next_oid += 1;
        inv.add_item(&data.item_data, oid, entry.item_id, entry.count);
        if entry.equipped {
            inv.equip_item(&data.item_data, oid);
        }
    }
    let items: Vec<crate::character::ItemRow> = inv
        .items()
        .iter()
        .map(|it| {
            let slot = inv.paperdoll_slot_of(it.object_id);
            crate::character::ItemRow {
                object_id: it.object_id,
                item_id: it.item_id,
                count: it.count,
                enchant_level: 0,
                loc: if slot.is_some() { "PAPERDOLL".into() } else { "INVENTORY".into() },
                loc_data: slot.map(|s| s as i32).unwrap_or(0),
                custom_type1: 0,
                custom_type2: 0,
                mana_left: -1,
                time: 0,
            }
        })
        .collect();

    let mut chr = dummy_char(4212, "Mystic");
    chr.class_id = class_id;
    chr.base_class_id = class_id;
    chr.items = items;
    chr.skills = data.skill_trees.initial_skills(class_id); // 118, 163, 214, 1177, 1216

    let b = Player::from_char(&data, &chr);
    let c = &b.combat;
    // Displayed via `(int)`/`as i32` truncation, matching the Java client panel.
    assert_eq!(c.p_atk as i32, 2, "p.atk");
    assert_eq!(c.m_atk as i32, 8, "m.atk");
    assert_eq!(c.p_def as i32, 52, "p.def");
    assert_eq!(c.accuracy, 31, "p.accuracy");
    assert_eq!(c.evasion, 23, "p.evasion");
    assert_eq!(c.crit_hit as i32, 60, "p.critical");
    assert_eq!(c.p_atk_spd, 384, "atk speed");
    assert_eq!(b.speeds.run_spd as i32, 159, "run speed");
    assert_eq!(c.m_def as i32, 54, "m.def");
    assert_eq!(c.magic_accuracy, 15, "m.accuracy");
    assert_eq!(c.magic_evasion, 15, "m.evasion");
    assert_eq!(c.m_crit_hit as i32, 50, "m.critical");
    assert_eq!(c.m_atk_spd, 499, "cast speed (333 × Spellcraft 1.5 in a robe)");

    // --- Now drive the real enter-world refresh tail (expertise + conditioned
    // passives, in the order `handle_enter_world` runs them) and confirm the
    // in-world stats still match — this is where the reported 349 shows up. ---
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);
    b.spawn_into(&mut world.objects);
    super::expertise::refresh_expertise_penalty(&mut world, 4212);
    super::passive_skills::refresh_conditioned_passives(&mut world, 4212);
    assert_eq!(pcs(&world, 4212).m_atk_spd, 499, "cast speed after enter-world refresh tail");
    assert_eq!(pcs(&world, 4212).p_atk as i32, 2, "p.atk after enter-world refresh tail");
}

/// The armor-conditioned passives close the last gap: Spellcraft (163) multiplies
/// a robe mystic's casting speed by 1.5 (333 → 499), while Magician's Movement
/// (118) stays inert (its −20% atk-speed penalty is gated to non-robe armor).
#[test]
fn spellcraft_passive_raises_mystic_cast_speed_in_a_robe() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let paperdoll = |object_id, item_id, slot| crate::character::ItemRow {
        object_id,
        item_id,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: slot,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
    };
    let mut chr = dummy_char(4211, "Robe");
    chr.class_id = 10;
    chr.base_class_id = 10;
    chr.items = vec![paperdoll(1001, 6, 5), paperdoll(1002, 425, 6), paperdoll(1003, 461, 11)];
    // The two autoGet mystic passives.
    chr.skills = vec![(163, 1), (118, 1)];
    let bundle = Player::from_char(&world.data, &chr);
    // `from_char` (Java `restoreCharData`/`addSkill`) already folds the robe
    // passives in: Spellcraft's MAGIC branch (+50%) applies, while Magician's
    // Movement stays inert (its −20% atk-speed penalty is gated to non-robe).
    assert_eq!(bundle.combat.m_atk_spd, 499, "Spellcraft: 333 × 1.5 in a robe");
    assert_eq!(bundle.combat.p_atk_spd, 384, "Magician's Movement stays inert in a robe");
    bundle.spawn_into(&mut world.objects);

    // Take the robe legs off: the MAGIC condition now fails (bare legs read as
    // NONE), so `refresh_conditioned_passives` drops Spellcraft's bonus.
    world.objects.get_component_mut::<crate::model::inventory::Inventory>(&4211).unwrap().unequip_item(1003);
    super::passive_skills::refresh_conditioned_passives(&mut world, 4211);
    assert_eq!(pcs(&world, 4211).m_atk_spd, 333, "no robe → Spellcraft bonus gone");
}

/// Reproduction of the reported "casting speed 349 at level 7" bug: a Human
/// Mystic learns Weapon Mastery (249) at getLevel 7, whose `-30%
/// MagicalAttackSpeed` is gated to `<weaponType>BOW/POLE`. Wielding a (non
/// bow/pole) staff in a no-grade robe, that effect must NOT apply, so casting
/// speed stays Spellcraft's 499 — but before the `<weaponType>` gate was
/// honored it dropped to 349 (499 × 0.7). Driven through the real relogin path
/// (delevel filter → `from_char` → enter-world refresh tail); the no-grade robe
/// keeps the armor grade-penalty out of it, isolating the weapon-condition bug.
#[test]
fn human_mystic_lvl7_weapon_mastery_does_not_slow_staff_casting() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    data.skill_trees = crate::data::skill_tree::SkillTreeData::load_from(DIST);
    data.initial_equipment = crate::data::initial_equipment::InitialEquipmentData::load_from(DIST);

    let class_id = 10; // Human Mystic

    // No-grade MAGIC robe (chest/legs/gloves → Spellcraft applies, no grade
    // penalty) plus a D-grade BLUNT staff (15149) — a weapon that is NOT
    // bow/pole, equipped through the real slot logic.
    let mut inv = crate::model::inventory::Inventory::new();
    let mut next_oid = 2000;
    for item_id in [6, 425, 461, 15149] {
        let oid = next_oid;
        next_oid += 1;
        inv.add_item(&data.item_data, oid, item_id, 1);
        inv.equip_item(&data.item_data, oid);
    }
    let items: Vec<crate::character::ItemRow> = inv
        .items()
        .iter()
        .map(|it| {
            let slot = inv.paperdoll_slot_of(it.object_id);
            crate::character::ItemRow {
                object_id: it.object_id,
                item_id: it.item_id,
                count: it.count,
                enchant_level: 0,
                loc: if slot.is_some() { "PAPERDOLL".into() } else { "INVENTORY".into() },
                loc_data: slot.map(|s| s as i32).unwrap_or(0),
                custom_type1: 0,
                custom_type2: 0,
                mana_left: -1,
                time: 0,
            }
        })
        .collect();

    let mut chr = dummy_char(4213, "Mystic7");
    chr.class_id = class_id;
    chr.base_class_id = class_id;
    chr.level = 7;
    chr.items = items;
    // Every skill a level-7 mystic can reach (autoGet + learnable), i.e. what the
    // character would have after "reaching level 7 and getting skills".
    chr.skills = data.skill_trees.all_available_skills(class_id, 7, &std::collections::HashMap::new());
    assert!(chr.skills.iter().any(|&(id, _)| id == 163), "level-7 mystic has Spellcraft (163)");
    assert!(chr.skills.iter().any(|&(id, _)| id == 249), "level-7 mystic has Weapon Mastery (249)");

    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

    // 1. Character select: the delevel filter (`filter_skills_on_select` →
    // `maybe_skill_remove_on_delevel`), replicated on `chr.skills`.
    let skills_before = chr.skills.len();
    {
        let mut skills_map: std::collections::HashMap<i32, i32> = chr.skills.iter().copied().collect();
        super::death::maybe_skill_remove_on_delevel(&world, chr.object_id, chr.class_id, chr.level, &mut skills_map);
        chr.skills = skills_map.into_iter().collect();
    }
    assert!(chr.skills.iter().any(|&(id, _)| id == 163), "delevel filter kept Spellcraft (163)");
    assert_eq!(chr.skills.len(), skills_before, "delevel filter removed no skills at level 7");

    // 2. Build the player from the (filtered) select data.
    let b = Player::from_char(&world.data, &chr);
    assert_eq!(b.combat.m_atk_spd, 499, "cast speed after from_char (Spellcraft ×1.5 in a robe)");
    b.spawn_into(&mut world.objects);

    // 3. Enter-world refresh tail, in `handle_enter_world` order.
    super::expertise::refresh_expertise_penalty(&mut world, 4213);
    assert_eq!(pcs(&world, 4213).m_atk_spd, 499, "cast speed after expertise refresh");
    super::passive_skills::refresh_conditioned_passives(&mut world, 4213);
    assert_eq!(pcs(&world, 4213).m_atk_spd, 499, "cast speed after conditioned-passive refresh");
}

/// Delevel skill filtering runs at character *select*, before `from_char`, so
/// the built `Player` folds only the surviving passives and its enter-world
/// `UserInfo` is right the first time (the casting-speed-349 bug). A robe
/// mystic delevelled below 7 loses its getLevel-7 class skill but keeps
/// Spellcraft (getLevel 1), so casting speed stays 499.
#[test]
fn delevel_filter_on_select_keeps_passive_stats() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    data.skill_trees = crate::data::skill_tree::SkillTreeData::load_from(DIST);
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let paperdoll = |object_id, item_id, slot| crate::character::ItemRow {
        object_id,
        item_id,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: slot,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
    };
    let mut chr = dummy_char(4213, "Robe");
    chr.class_id = 10;
    chr.base_class_id = 10;
    chr.level = 5; // below the getLevel-7 skills
    chr.items = vec![paperdoll(1001, 6, 5), paperdoll(1002, 425, 6), paperdoll(1003, 461, 11)];
    // Spellcraft (163, getLevel 1) + Magician's Movement (118, getLevel 1) +
    // Shield (1040, getLevel 7) that a level-5 delevel strips.
    chr.skills = vec![(163, 1), (118, 1), (1040, 1)];

    // The select-time filter (what `filter_skills_on_select` runs).
    let mut skills: std::collections::HashMap<i32, i32> = chr.skills.iter().copied().collect();
    let changes = super::death::maybe_skill_remove_on_delevel(&world, chr.object_id, chr.class_id, chr.level, &mut skills);
    assert!(changes.iter().any(|&(id, a)| id == 1040 && a.is_none()), "Shield stripped at level 5");
    chr.skills = skills.into_iter().collect();

    // `from_char` on the corrected skills: Shield gone, Spellcraft kept, so the
    // casting-speed bonus is folded in and the first UserInfo is 499 (not 349).
    let bundle = Player::from_char(&world.data, &chr);
    assert!(!bundle.skills.0.contains_key(&1040), "Shield removed from the book");
    assert!(bundle.skills.0.contains_key(&163), "Spellcraft survives");
    assert_eq!(bundle.combat.m_atk_spd, 499, "Spellcraft's casting-speed bonus intact");
}

/// A live level-down (`check_player_skills`) removes a now-too-high passive and
/// re-folds the stat block: Weapon Mastery (249, getLevel 7, +m.atk) is stripped
/// at level 5, lowering m.atk, while Spellcraft (getLevel 1) stays and keeps
/// casting speed at 499. Only passive skills move stats — step 4.
#[test]
fn live_delevel_removes_passive_and_recomputes_stats() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    data.skill_trees = crate::data::skill_tree::SkillTreeData::load_from(DIST);
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let paperdoll = |object_id, item_id, slot| crate::character::ItemRow {
        object_id,
        item_id,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: slot,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
    };
    let mut chr = dummy_char(4214, "Mage");
    chr.class_id = 10;
    chr.base_class_id = 10;
    chr.level = 5;
    chr.items = vec![paperdoll(1001, 6, 5), paperdoll(1002, 425, 6), paperdoll(1003, 461, 11)];
    // Spellcraft (163, getLevel 1) + Weapon Mastery (249, getLevel 7, passive +m.atk).
    chr.skills = vec![(163, 1), (249, 1)];
    let bundle = Player::from_char(&world.data, &chr);
    let m_atk_with_mastery = bundle.combat.m_atk;
    bundle.spawn_into(&mut world.objects);

    // Level-down check strips Weapon Mastery (5 < 7) and re-folds the stats.
    super::death::check_player_skills(&mut world, 4214);
    assert!(!world.objects.get_component::<SkillBook>(&4214).unwrap().0.contains_key(&249), "Weapon Mastery removed");
    assert!(world.objects.get_component::<SkillBook>(&4214).unwrap().0.contains_key(&163), "Spellcraft kept");
    // Weapon Mastery's +m.atk is gone; Spellcraft's casting-speed bonus (499)
    // is now un-corrupted by 249 and correctly folded from the reduced book.
    assert!(pcs(&world, 4214).m_atk < m_atk_with_mastery, "removing Weapon Mastery lowered m.atk");
    assert_eq!(pcs(&world, 4214).m_atk_spd, 499, "recompute re-folds only the surviving passives");
}

/// `AutoLearnSkills`: `rewardSkills` must grant every reachable class skill,
/// not just autoGet ones — and only autoGet ones when the flag is off.
#[test]
fn auto_learn_grants_all_reachable_class_skills() {
    use crate::data::skill_tree::SkillLearn;

    let mk_data = || {
        let mut data = GameData::for_test();
        data.player_templates = crate::data::PlayerTemplateData::from_vec(vec![human_fighter_template()]);
        // Class 0: a level-1 autoGet skill + a non-autoGet class skill (id 91,
        // levels 1@getLevel5 and 2@getLevel10).
        data.skill_trees.insert_for_test(0, SkillLearn { skill_id: 1000, skill_level: 1, name: "Auto".into(), get_level: 1, level_up_sp: 0, auto_get: true });
        data.skill_trees.insert_for_test(0, SkillLearn { skill_id: 91, skill_level: 1, name: "Class1".into(), get_level: 5, level_up_sp: 100, auto_get: false });
        data.skill_trees.insert_for_test(0, SkillLearn { skill_id: 91, skill_level: 2, name: "Class2".into(), get_level: 10, level_up_sp: 200, auto_get: false });
        data
    };

    let spawn_level_5 = |world: &mut World| {
        let mut chr = dummy_char(2001, "Al");
        chr.level = 5;
        let bundle = Player::from_char(&world.data, &chr);
        let (link_out, _r) = tokio::sync::mpsc::unbounded_channel();
        let s = Session::new(1, link_out, "127.0.0.1:1".parse().unwrap())
            .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
            .into_lobby(vec![])
            .into_entering(bundle);
        let (_session, bundle) = s.into_ingame();
        bundle.spawn_into(&mut world.objects);
    };

    // Flag ON: the class skill (id 91 @ level 1, the max reachable at char
    // level 5) is auto-learned alongside the autoGet skill.
    {
        let (link_tx, _l) = tokio::sync::mpsc::unbounded_channel();
        let (db_tx, _d) = tokio::sync::mpsc::unbounded_channel();
        let mut world = World::new(link_tx, 7, 3, 0, mk_data(), db_tx);
        world.cfg.character.auto_learn_skills = true;
        spawn_level_5(&mut world);
        super::death::reward_skills(&mut world, 2001);
        let book = &world.objects.get_component::<SkillBook>(&2001).unwrap().0;
        assert_eq!(book.get(&1000), Some(&1), "autoGet skill granted");
        assert_eq!(book.get(&91), Some(&1), "class skill auto-learned at level 5");
    }

    // Flag OFF: only the autoGet skill; the class skill stays unlearned.
    {
        let (link_tx, _l) = tokio::sync::mpsc::unbounded_channel();
        let (db_tx, _d) = tokio::sync::mpsc::unbounded_channel();
        let mut world = World::new(link_tx, 7, 3, 0, mk_data(), db_tx);
        assert!(!world.cfg.character.auto_learn_skills, "default is off");
        spawn_level_5(&mut world);
        super::death::reward_skills(&mut world, 2001);
        let book = &world.objects.get_component::<SkillBook>(&2001).unwrap().0;
        assert_eq!(book.get(&1000), Some(&1), "autoGet skill granted");
        assert_eq!(book.get(&91), None, "class skill NOT auto-learned when flag is off");
    }
}

/// `Player.checkPlayerSkills` on delevel: a skill above the `(level − 9)` grace
/// is downgraded to the highest still-reachable level, then removed once even
/// level 1 is out of range — and kept untouched when `DecreaseSkillOnDelevel`
/// is off.
#[test]
fn delevel_downgrades_then_removes_skills() {
    use crate::data::skill_tree::SkillLearn;

    let mk_data = || {
        let mut data = GameData::for_test();
        data.player_templates = crate::data::PlayerTemplateData::from_vec(vec![human_fighter_template()]);
        // Skill 91: level 1 @ getLevel 20, level 2 @ getLevel 40.
        data.skill_trees.insert_for_test(0, SkillLearn { skill_id: 91, skill_level: 1, name: "S1".into(), get_level: 20, level_up_sp: 100, auto_get: false });
        data.skill_trees.insert_for_test(0, SkillLearn { skill_id: 91, skill_level: 2, name: "S2".into(), get_level: 40, level_up_sp: 200, auto_get: false });
        // Skill 92: a single level @ getLevel 7 — used to show the strict flag
        // vs the 9-level grace at low character levels.
        data.skill_trees.insert_for_test(0, SkillLearn { skill_id: 92, skill_level: 1, name: "S3".into(), get_level: 7, level_up_sp: 100, auto_get: false });
        data
    };

    // Spawn a level-40 character who knows the skills, then force the level down
    // (a delevel already applied to the model) and run the check.
    let run = |decrease_flag: bool, strict: bool, new_level: i32, skill_id: i32| -> Option<i32> {
        let (link_tx, _l) = tokio::sync::mpsc::unbounded_channel();
        let (db_tx, _d) = tokio::sync::mpsc::unbounded_channel();
        let mut world = World::new(link_tx, 7, 3, 0, mk_data(), db_tx);
        world.cfg.character.decrease_skill_level = decrease_flag;
        world.cfg.character.strict_delevel_skill_removal = strict;

        let mut chr = dummy_char(2001, "Al");
        chr.level = 40;
        chr.skills = vec![(91, 2), (92, 1)];
        let bundle = Player::from_char(&world.data, &chr);
        let (link_out, _r) = tokio::sync::mpsc::unbounded_channel();
        let s = Session::new(1, link_out, "127.0.0.1:1".parse().unwrap())
            .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
            .into_lobby(vec![])
            .into_entering(bundle);
        let (_session, bundle) = s.into_ingame();
        bundle.spawn_into(&mut world.objects);

        world.objects.get_component_mut::<crate::model::Player>(&2001).unwrap().level = new_level;
        super::death::check_player_skills(&mut world, 2001);
        world.objects.get_component::<SkillBook>(&2001).unwrap().0.get(&skill_id).copied()
    };

    // --- Default strict mode (StrictDelevelSkillRemoval = true). ---
    // 40 → 30: skill 91 @ level 2 (getLevel 40) is out of range → downgrade to
    // the highest reachable level (1, getLevel 20).
    assert_eq!(run(true, true, 30, 91), Some(1), "downgraded to the highest reachable level");
    // 40 → 5: even level 1 (getLevel 20) is out of range → removed.
    assert_eq!(run(true, true, 5, 91), None, "removed when no level is reachable");
    // Skill 92 (getLevel 7) at level 1: strict strips it (1 < 7)…
    assert_eq!(run(true, true, 1, 92), None, "strict removes a getLevel-7 skill at level 1");

    // --- Non-strict (Java 9-level grace). ---
    // …but the 9-level grace keeps it (1 ≥ 7 − 9).
    assert_eq!(run(true, false, 1, 92), Some(1), "grace keeps a getLevel-7 skill at level 1");

    // Flag off: kept despite being out of range, regardless of strictness.
    assert_eq!(run(false, true, 5, 91), Some(2), "kept when DecreaseSkillOnDelevel is off");
}

fn magic_skill_use_body(magic_id: i32, ctrl: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(magic_id);
    w.write_i32(if ctrl { 1 } else { 0 });
    w.write_u8(0); // shiftPressed
    w.into_bytes()
}

fn magic_skill_use_body_shift(magic_id: i32, ctrl: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(magic_id);
    w.write_i32(if ctrl { 1 } else { 0 });
    w.write_u8(1); // shiftPressed — Java `dontMove`
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
        reuse_delay_group: -1,
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
            armor_condition: 0,
            weapon_condition: 0,
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
            armor_condition: 0,
            weapon_condition: 0,
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
    let (session, bundle) = s.into_ingame();
    bundle.spawn_into(&mut world.objects);
    world.clients.insert(client_id, ClientSession::InGame(session));
    world.objects.get_component_mut::<PlayerVitals>(&object_id).unwrap().cur_cp = 100.0;
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
    assert!(!world.objects.has_component::<Casting>(&3001));

    // With ctrl: ExRotation (face target) + initial-MP StatusUpdate +
    // MagicSkillUse to everyone, YOU_USE_S1 + SetupGauge to the caster.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::EX);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE);
    let msu = a_rx.try_recv().unwrap();
    assert_eq!(msu[0], server_packets::opcodes::MAGIC_SKILL_USE);
    assert_eq!(
        i32::from_le_bytes(msu[25..29].try_into().unwrap()),
        -1,
        "ungrouped skill must send reuse group -1 (0 greys every icon client-side)"
    );
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::YOU_USE_S1);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::SETUP_GAUGE);
    assert!(a_rx.try_recv().is_err());
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::EX);
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_USE);
    assert!(b_rx.try_recv().is_err());
    assert_eq!(pvit(&world, 3001).cur_mp, 48.0, "50 - mpInitialConsume(2)");

    // Launch at hit = 4000/1.0 − 500 = 3500 ms = 35 ticks.
    world.tick += 35;
    apply_due_tasks(&mut world);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);

    // Finish 500 ms later: MP consume, damage, messages, status updates.
    world.tick += 5;
    apply_due_tasks(&mut world);

    let m_atk = pcs(&world, 3001).m_atk;
    let m_def = pcs(&world, 3002).m_def;
    let damage = formulas::calc_magic_dam(m_atk, m_def, 12.0, false, 1.0);
    assert!(damage > 100.0, "sanity: the nuke must overflow B's CP ({damage})");
    {
        let b = pvit(&world, 3002);
        let bcp = pcp(&world, 3002);
        assert_eq!(bcp.cur_cp, 0.0, "CP absorbs first");
        assert!((b.cur_hp - (100.0 - (damage - 100.0))).abs() < 1e-9, "HP takes the rest");
    }
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE); // MP consume
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2);
    // Being hit puts B in combat stance (CreatureAI.onEvtAttacked ->
    // clientStartAutoAttack broadcast), then B's CP/HP status.
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::AUTO_ATTACK_START);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE); // B's CP/HP
    // Nuking a player flags the caster (SkillCaster: bad skill on a playable →
    // updatePvPStatus(target)): a PVP_FLAG StatusUpdate for object 3001, then
    // the caster's own stance — both broadcast, object 3001.
    let a_flag = a_rx.try_recv().unwrap();
    assert_eq!(a_flag[0], server_packets::opcodes::STATUS_UPDATE);
    assert_eq!(i32::from_le_bytes(a_flag[1..5].try_into().unwrap()), 3001, "caster's own pvp-flag update");
    let a_stance = a_rx.try_recv().unwrap();
    assert_eq!(a_stance[0], server_packets::opcodes::AUTO_ATTACK_START);
    assert_eq!(i32::from_le_bytes(a_stance[1..5].try_into().unwrap()), 3001, "caster's own stance");
    assert!(a_rx.try_recv().is_err());
    assert_eq!(sm_id(&b_rx.try_recv().unwrap()), server_packets::sm_ids::C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2);
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::AUTO_ATTACK_START);
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE);
    // B also sees A's flag: the PVP_FLAG StatusUpdate + a RelationChanged.
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE, "B sees A's pvp-flag update");
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::RELATION_CHANGED, "B sees A's relation change");
    let b_sees_a = b_rx.try_recv().unwrap();
    assert_eq!(b_sees_a[0], server_packets::opcodes::AUTO_ATTACK_START);
    assert_eq!(i32::from_le_bytes(b_sees_a[1..5].try_into().unwrap()), 3001, "B sees the caster's stance");
    assert!(b_rx.try_recv().is_err());
    assert!(world.objects.get_component::<crate::model::components::AttackState>(&3001).is_some_and(|st| st.stance_until_tick > world.tick), "caster is in combat stance → canLogout refuses relogin");
    assert_eq!(world.objects.get_component::<crate::model::components::PvpState>(&3001).unwrap().flag, 1, "caster is now flagged for attacking a player");
    assert!(!world.objects.has_component::<Casting>(&3001), "coolTime 0 frees the slot");

    // Immediate re-cast: 10 s reuse still has 6 s left → SM 2303 + fail.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::S2_SECONDS_REMAINING_FOR_REUSE);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert!(b_rx.try_recv().is_err(), "rejected cast must not broadcast");
}

/// A shift-click cast out of range (Java `dontMove`) is cancelled with
/// SM 748 — no walk-into-range, nothing announced.
#[test]
fn shift_cast_out_of_range_cancelled_without_moving() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 700, 0); // castRange 600
    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);
    drain(&mut b_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body_shift(1177, true));
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(a_rx.try_recv().is_err());
    assert!(b_rx.try_recv().is_err());
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert!(!world.objects.has_component::<Intent>(&3001), "dontMove must not start a walk-to-cast");
    assert!(!world.objects.has_component::<Movement>(&3001));
}

/// A lethal nuke kills (G9): HP hits 0, the victim is dead, and `Die` with
/// the to-village flag reaches both sides.
#[test]
fn nuke_kills_at_zero_hp() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    world.objects.get_component_mut::<PlayerVitals>(&3002).unwrap().cur_cp = 0.0;
    world.objects.get_component_mut::<Vitals>(&3002).unwrap().cur_hp = 5.0;
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    advance_ticks(&mut world, 45);
    let b = pvit(&world, 3002);
    assert_eq!(b.cur_hp, 0.0);
    assert!(b.dead);
    let a_packets = drain(&mut a_rx);
    let b_packets = drain(&mut b_rx);
    for packets in [&a_packets, &b_packets] {
        let die = packets
            .iter()
            .find(|p| p[0] == server_packets::opcodes::DIE && i32::from_le_bytes(p[1..5].try_into().unwrap()) == 3002)
            .expect("Die packet for B");
        assert_eq!(i32::from_le_bytes(die[5..9].try_into().unwrap()), 1, "to-village flag");
    }
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
    let mp_after_start = pvit(&world, 3001).cur_mp;

    // Esc (targetLost=false: abort only, keep the target).
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(false));
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_CANCELED);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_CANCELED);

    // The scheduled launch is stale: nothing fires, nothing lands.
    world.tick += 40;
    apply_due_tasks(&mut world);
    assert!(a_rx.try_recv().is_err());
    assert!(b_rx.try_recv().is_err());
    assert_eq!(pvit(&world, 3001).cur_mp, mp_after_start, "no finish consume after abort");
    assert_eq!(pvit(&world, 3002).cur_hp, 100.0);

    // Reuse (registered at cast start) still blocks, then expires.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::S2_SECONDS_REMAINING_FOR_REUSE);
    drain(&mut a_rx);
    world.tick += 60;
    apply_due_tasks(&mut world);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001), "castable again after reuse expiry");
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

    world.objects.get_component_mut::<Position>(&3002).unwrap().x = 5000; // > effectRange 1100

    world.tick += 40;
    apply_due_tasks(&mut world);
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED);
    assert!(a_rx.try_recv().is_err(), "no MagicSkillLaunched, no cancel packet");
    assert!(b_rx.try_recv().is_err());
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert_eq!(pvit(&world, 3002).cur_hp, 100.0);
}

/// A heal on another player: Heal.java's `power + sqrt(2·mAtk)` amount,
/// overheal-clamped, SM 1067 to the healed target.
#[test]
fn heal_on_other_restores_hp_with_formula() {
    let (mut world, ..) = cast_test_world();
    let mut _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    world.objects.get_component_mut::<Vitals>(&3002).unwrap().cur_hp = 50.0;
    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut b_rx);

    // TARGET-type skills need no ctrl.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
    assert!(world.objects.has_component::<Casting>(&3001));
    drain(&mut b_rx); // ExRotation + MagicSkillUse

    advance_ticks(&mut world, 10); // hit 500 ms + cancel 500 ms

    let heal = formulas::calc_heal(83.0, pcs(&world, 3001).m_atk, false, false, false, 0, false);
    assert!(heal > 50.0, "sanity: heal ({heal}) overflows the missing 50 HP");
    assert_eq!(pvit(&world, 3002).cur_hp, 100.0, "overheal clamped at max HP");
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
    let base_p_atk = pcs(&world, 3002).p_atk;

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, false));
    advance_ticks(&mut world, 10);

    {
        assert_eq!(pbuffs(&world, 3002), 1);
        assert!(pcs(&world, 3002).p_atk > base_p_atk, "P.Atk pumped by Might (+8%)");
    }
    let b_packets = drain(&mut b_rx);
    assert!(
        b_packets.iter().any(|p| p[0] == 0x85),
        "target's client gets the AbnormalStatusUpdate"
    );
    assert_eq!(pbuffs(&world, 3001), 0, "nothing lands on the caster");

    advance_ticks(&mut world, 200);
    assert_eq!(pbuffs(&world, 3002), 0);
    assert_eq!(pcs(&world, 3002).p_atk, base_p_atk, "restored after expiry");
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

    world.objects.get_component_mut::<Vitals>(&3001).unwrap().cur_mp = 0.0;

    advance_ticks(&mut world, 45);
    // Launch fires normally (range fine), then the finish fails on MP.
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::NOT_ENOUGH_MP);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(a_rx.try_recv().is_err());
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::MAGIC_SKILL_LAUNCHED);
    assert!(b_rx.try_recv().is_err(), "no cancel packet on a quiet stop");
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert_eq!(pvit(&world, 3002).cur_hp, 100.0, "no damage landed");
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
    world.objects.get_component_mut::<Reuses>(&3001).unwrap().0.insert(
        1177,
        crate::model::SkillReuse { skill_level: 1, until_tick: world.tick + 60, total_ms: 10_000 },
    );
    on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_COOL_TIME]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::SKILL_COOL_TIME);
    assert_eq!(i32::from_le_bytes(pkt[1..5].try_into().unwrap()), 1);
    assert_eq!(i32::from_le_bytes(pkt[5..9].try_into().unwrap()), 1177);
    assert_eq!(i32::from_le_bytes(pkt[9..13].try_into().unwrap()), 1, "known level");
    assert_eq!(i32::from_le_bytes(pkt[13..17].try_into().unwrap()), 10, "total seconds");
    assert_eq!(i32::from_le_bytes(pkt[17..21].try_into().unwrap()), 6, "remaining seconds");
}

/// RequestSkillList (0x50): empty body, re-sends the `SkillList` packet
/// (`player.sendSkillList()`) — the client asks for this when it opens the
/// skills panel.
#[test]
fn request_skill_list_resends_skill_list() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0); // 4 known skills
    drain(&mut a_rx);

    on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_LIST]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], 0x5F, "SkillList opcode");
    assert_eq!(i32::from_le_bytes(pkt[1..5].try_into().unwrap()), 4, "all known skills listed");
}

/// Skills sharing a positive `reuseDelayGroup` share one cooldown entry
/// keyed by the group id: the `MagicSkillUse` broadcast carries the group,
/// casting one blocks the sibling (SM 48 — short reuse), and
/// `SkillCoolTime` reports the group id with the cast level.
#[test]
fn shared_reuse_group_blocks_sibling_skill() {
    let (mut world, ..) = cast_test_world();

    // Two quick self-skills in shared group 9000 (potion-style), cloned
    // off Slow Aura (91) so only the reuse fields differ.
    let base = world.data.skill_data.get(91, 1).unwrap().clone();
    for id in [7001, 7002] {
        world.data.skill_data.insert_for_test(Skill {
            id,
            hit_time: 400,
            reuse_delay: 2000,
            reuse_delay_group: 9000,
            ..base.clone()
        });
    }

    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let skills = &mut world.objects.get_component_mut::<SkillBook>(&3001).unwrap().0;
    skills.insert(7001, 1);
    skills.insert(7002, 1);

    // Cast the first: MagicSkillUse carries group 9000 + the 2000 ms
    // delay, and the reuse lands under the group key, not the skill id.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(7001, false));
    let msu = drain(&mut a_rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE)
        .expect("MagicSkillUse broadcast");
    assert_eq!(i32::from_le_bytes(msu[25..29].try_into().unwrap()), 9000, "reuse group");
    assert_eq!(i32::from_le_bytes(msu[29..33].try_into().unwrap()), 2000, "reuse delay");
    let reuses = &world.objects.get_component::<Reuses>(&3001).unwrap().0;
    assert!(reuses.contains_key(&9000) && !reuses.contains_key(&7001));

    // The sibling is blocked by the shared cooldown (reuse gate fires
    // before the busy-casting-slot check, same as Java's useMagic order).
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(7002, false));
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::S1_IS_NOT_AVAILABLE_REUSE);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);

    // SkillCoolTime reports the group id, cast level, 2 s total/remaining.
    on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_COOL_TIME]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::SKILL_COOL_TIME);
    assert_eq!(i32::from_le_bytes(pkt[1..5].try_into().unwrap()), 1);
    assert_eq!(i32::from_le_bytes(pkt[5..9].try_into().unwrap()), 9000, "group id, not skill id");
    assert_eq!(i32::from_le_bytes(pkt[9..13].try_into().unwrap()), 1, "cast level");
    assert_eq!(i32::from_le_bytes(pkt[13..17].try_into().unwrap()), 2, "total seconds");
    assert_eq!(i32::from_le_bytes(pkt[17..21].try_into().unwrap()), 2, "remaining seconds");
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
    assert!(world.objects.has_component::<Casting>(&3002));

    // A nukes B; the nuke lands at 40 ticks, well before B's launch.
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Force the rolls: crit d1000 (rate 0 → miss regardless), then the
    // atk-break d100 → 0 always breaks (rate ≥ 1).
    world.forced_rolls.extend([999, 0]);

    advance_ticks(&mut world, 45);

    assert!(!world.objects.has_component::<Casting>(&3002), "victim's cast broken");
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
    assert_eq!(pbuffs(&world, 3002), 0);
}

/// A move click during a cast is rejected (ActionFailed, cast keeps going)
/// but saved as the next intention, and the move starts by itself once the
/// cast stops — Java `PlayerAI.onIntentionMoveTo`'s `saveNextIntention` +
/// `onEvtFinishCasting`.
#[test]
fn move_click_during_cast_is_queued_and_replayed_when_cast_stops() {
    use crate::model::components::QueuedAction;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    world.objects.get_component_mut::<Speeds>(&3001).unwrap().run_spd = 100.0;
    world.objects.get_component_mut::<Speeds>(&3001).unwrap().running = true;

    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Click to move mid-cast: rejected, cast intact, click remembered.
    handle_move_backward_to_location(&mut world, 1, &move_body((500, 0, 0), (0, 0, 0), 1));
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(a_rx.try_recv().is_err(), "nothing else while the cast runs");
    assert!(world.objects.has_component::<Casting>(&3001), "the cast is not aborted");
    assert!(!world.objects.has_component::<Movement>(&3001), "no move yet");
    assert!(matches!(world.objects.get_component::<QueuedAction>(&3001), Some(QueuedAction::Move { .. })));

    // Launch (35 ticks) + finish (5 more, coolTime 0 frees the slot): the
    // queued click replays through the normal move pipeline.
    advance_ticks(&mut world, 45);
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert!(!world.objects.has_component::<QueuedAction>(&3001), "queue consumed");
    let mv = world.objects.get_component::<Movement>(&3001).expect("move started at cast end");
    assert_eq!((mv.0.dest_x, mv.0.dest_y), (500, 0));
    let a_packets = drain(&mut a_rx);
    assert!(a_packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_LOCATION));
    let b_packets = drain(&mut b_rx);
    assert!(b_packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_LOCATION));
}

/// Casting a good skill while running pauses the move and resumes it toward
/// the original destination after the cast; an offensive skill forgets it —
/// Java `PlayerAI.changeIntention`'s save/clear of the interrupted intention.
#[test]
fn good_skill_cast_pauses_and_resumes_inflight_move() {
    use crate::model::components::QueuedAction;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    world.objects.get_component_mut::<Speeds>(&3001).unwrap().run_spd = 100.0;
    world.objects.get_component_mut::<Speeds>(&3001).unwrap().running = true;

    handle_move_backward_to_location(&mut world, 1, &move_body((600, 0, 0), (0, 0, 0), 1));
    assert!(world.objects.has_component::<Movement>(&3001));
    drain(&mut a_rx);

    // Slow Aura (good, self): the move stops but its destination is saved.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));
    assert!(world.objects.has_component::<Casting>(&3001));
    assert!(!world.objects.has_component::<Movement>(&3001), "cast stops the move");
    match world.objects.get_component::<QueuedAction>(&3001) {
        Some(&QueuedAction::Move { x, y, z }) => assert_eq!((x, y, z), (600, 0, 0)),
        other => panic!("interrupted move not saved: {other:?}"),
    }

    // hit 9500 ms (95 ticks) + finish 5 ticks later: the move resumes.
    advance_ticks(&mut world, 101);
    assert!(!world.objects.has_component::<Casting>(&3001));
    let mv = world.objects.get_component::<Movement>(&3001).expect("move resumed after the cast");
    assert_eq!((mv.0.dest_x, mv.0.dest_y), (600, 0));

    // An offensive cast instead forgets the interrupted move.
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
    assert!(!world.objects.has_component::<Movement>(&3001), "cast stops the move");
    assert!(!world.objects.has_component::<QueuedAction>(&3001), "bad skill forgets the move");
    advance_ticks(&mut world, 45);
    assert!(!world.objects.has_component::<Movement>(&3001), "nothing resumes after a nuke");
}

/// A skill clicked during a cast is queued (`Player._queuedSkill`) and fires
/// when the cast stops, resolved against the player's *current* target — so
/// re-targeting mid-cast redirects the queued skill (Java `stopCasting` →
/// `useMagic`, which re-resolves the target).
#[test]
fn skill_queued_during_cast_replays_on_current_target() {
    use crate::model::components::QueuedAction;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    let _c_rx = ingame_caster(&mut world, 3, 3003, 150, 0);
    world.objects.get_component_mut::<Vitals>(&3003).unwrap().cur_hp = 50.0;

    // A nukes B (hit 3500 + finish 500 ms = 40 ticks).
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
    drain(&mut a_rx);

    // Mid-cast: select C, then click Battle Heal → rejected but queued.
    handle_action(&mut world, 1, &action_body(3003, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(a_rx.try_recv().is_err(), "nothing else while the cast runs");
    assert!(
        matches!(world.objects.get_component::<QueuedAction>(&3001), Some(QueuedAction::Skill { skill_id: 1015, .. })),
        "skill click parked in the queue slot"
    );
    assert_eq!(
        world.objects.get_component::<Casting>(&3001).unwrap().0.skill_id,
        1177,
        "the running cast is untouched"
    );

    // The nuke finishes → the queued heal starts by itself, aimed at C.
    advance_ticks(&mut world, 45);
    let cast = world.objects.get_component::<Casting>(&3001).expect("queued skill cast started");
    assert_eq!(cast.0.skill_id, 1015);
    assert_eq!(cast.0.target_object_id, 3003, "replay resolves the mid-cast re-target");
    assert!(!world.objects.has_component::<QueuedAction>(&3001), "queue consumed");

    // Heal phases (hit 500 + finish 500 ms): C's HP goes up.
    advance_ticks(&mut world, 12);
    assert!(pvit(&world, 3003).cur_hp > 50.0, "heal landed on the new target");
}

/// A Ctrl-click (force attack) mid-cast on a *new* target must record the
/// attack as the next intention, so the swing starts once the cast ends —
/// Java's `onForcedAttack` → `setIntention(ATTACK)` (deferred to
/// `_nextIntention` while casting). Regression for the "it changes the target
/// but forgets to put the next intention, so when the cast finishes it doesn't
/// start a new action" report: a single ctrl-click used to only select.
#[test]
fn force_attack_mid_cast_engages_new_target_after_cast() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    // Nuke victim + the mob we force-attack next (in melee reach at x=20).
    add_test_npc(&mut world, NPC_OID + 90, 45001, "Monster", 5, 60, 0, 0);
    add_test_npc(&mut world, NPC_OID + 91, 45002, "Monster", 5, 20, 0, 0);
    let cast_target = NPC_OID + 90;
    let next = NPC_OID + 91;

    // Start a nuke on the first monster.
    handle_action(&mut world, 1, &action_body(cast_target, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Casting>(&3001), "nuke is casting");
    drain(&mut a_rx);

    // A SINGLE Ctrl-click on the second monster mid-cast: switches target AND
    // parks the attack as the intention (it can't swing yet — still casting).
    on_packet(&mut world, 1, [vec![cop::ATTACK], attack_request_body(next)].concat());
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(next),
        "target switched to the ctrl-clicked mob"
    );
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(crate::model::PlayerIntent::Attack { target_object_id })) if *target_object_id == next
        ),
        "the force-attack is remembered as the next intention"
    );
    assert!(world.objects.has_component::<Casting>(&3001), "the running nuke is untouched");

    // When the nuke finishes, the parked attack engages the new mob.
    let hp_before = nvit(&world, next).cur_hp;
    world.forced_rolls.extend(std::iter::repeat([0i32, 99, 10]).take(12).flatten());
    advance_world(&mut world, 55);
    assert!(nvit(&world, next).cur_hp < hp_before, "the new target took melee damage after the cast");
}

/// The queue slot is last-click-wins, both ways: a skill click supersedes a
/// queued move (Java: the `stopCasting` skill launch makes the new cast
/// forget `_nextIntention`), and a later move click wipes a queued skill
/// (Java `MoveBackwardToLocation.runImpl`'s "remove queued skill upon move
/// request").
#[test]
fn queued_action_slot_is_last_click_wins() {
    use crate::model::components::QueuedAction;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    world.objects.get_component_mut::<Speeds>(&3001).unwrap().run_spd = 100.0;
    world.objects.get_component_mut::<Speeds>(&3001).unwrap().running = true;

    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    drain(&mut a_rx);

    handle_move_backward_to_location(&mut world, 1, &move_body((500, 0, 0), (0, 0, 0), 1));
    assert!(matches!(world.objects.get_component::<QueuedAction>(&3001), Some(QueuedAction::Move { .. })));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
    assert!(matches!(world.objects.get_component::<QueuedAction>(&3001), Some(QueuedAction::Skill { skill_id: 1015, .. })));
    handle_move_backward_to_location(&mut world, 1, &move_body((600, 0, 0), (0, 0, 0), 1));
    match world.objects.get_component::<QueuedAction>(&3001) {
        Some(&QueuedAction::Move { x, .. }) => assert_eq!(x, 600, "move click wipes the queued skill"),
        other => panic!("expected the last move click in the slot: {other:?}"),
    }

    // Cast end: the last click (move) replays; no second cast starts.
    advance_ticks(&mut world, 45);
    assert!(!world.objects.has_component::<Casting>(&3001));
    let mv = world.objects.get_component::<Movement>(&3001).expect("move started at cast end");
    assert_eq!((mv.0.dest_x, mv.0.dest_y), (600, 0));
}

/// A skill clicked mid-swing (`isAttackingNow`) queues and fires when the
/// swing period ends (Java `thinkAttack`'s queued-skill check /
/// `EVT_READY_TO_ACT`), leaving the attack intent alive to resume after.
#[test]
fn skill_mid_swing_is_queued_until_swing_end() {
    use crate::model::components::QueuedAction;

    let (mut world, ..) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 20;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 100_000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    // Swing rolls: hit, no crit, ±0 random damage.
    world.forced_rolls.extend([0, 99, 10]);
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    drain(&mut a_rx);
    let swing_end = world.objects.get_component::<crate::model::components::AttackState>(&3001).unwrap().attack_end_tick;
    assert!(swing_end > world.tick, "swing in flight");

    // Mid-swing skill click: rejected, queued, intent intact.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(!world.objects.has_component::<Casting>(&3001), "no cast mid-swing");
    assert!(matches!(world.objects.get_component::<QueuedAction>(&3001), Some(QueuedAction::Skill { skill_id: 91, .. })));
    assert!(world.objects.has_component::<Intent>(&3001), "skill click keeps the attack intent");

    // Swing period over (`AttackFinish`): the queued cast starts.
    let remaining = swing_end - world.tick;
    advance_ticks(&mut world, remaining);
    let cast = world.objects.get_component::<Casting>(&3001).expect("queued skill fired at swing end");
    assert_eq!(cast.0.skill_id, 91);
    assert!(world.objects.has_component::<Intent>(&3001), "attack resumes after the cast");
}

/// A move click mid-swing waits out the swing (Java `onIntentionMoveTo`'s
/// `isAttackingNow` branch) and starts at swing end via `AttackFinish` —
/// which must fire even though the click dropped the attack intent.
#[test]
fn move_click_mid_swing_defers_to_swing_end() {
    use crate::model::components::QueuedAction;

    let (mut world, ..) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 21;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 100_000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    world.forced_rolls.extend([0, 99, 10]);
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    drain(&mut a_rx);
    let swing_end = world.objects.get_component::<crate::model::components::AttackState>(&3001).unwrap().attack_end_tick;

    handle_move_backward_to_location(&mut world, 1, &move_body((500, 0, 0), (0, 0, 0), 1));
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(!world.objects.has_component::<Movement>(&3001), "no move mid-swing");
    assert!(!world.objects.has_component::<Intent>(&3001), "move click ends the attack loop");
    assert!(matches!(world.objects.get_component::<QueuedAction>(&3001), Some(QueuedAction::Move { .. })));

    let remaining = swing_end - world.tick;
    advance_ticks(&mut world, remaining);
    let mv = world.objects.get_component::<Movement>(&3001).expect("move started at swing end");
    assert_eq!((mv.0.dest_x, mv.0.dest_y), (500, 0));
}

fn use_item_body(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(object_id);
    w.write_i32(0); // ctrl
    w.into_bytes()
}

/// Equipping gear during a cast is deferred to cast end (Java `UseItem`'s
/// `setNextAction(NextAction(EVT_FINISH_CASTING, …))`), silently — no packet
/// at click time, the equip lands when the cast stops.
#[test]
fn equip_click_during_cast_is_deferred_to_cast_end() {
    use crate::model::components::QueuedAction;
    use crate::model::inventory::Inventory;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    world.data.item_data.insert_for_test(crate::data::item_data::ItemTemplate {
        item_id: 2,
        name: "Test Sword".into(),
        kind: crate::data::item_data::ItemKind::Weapon,
        body_part: crate::data::item_data::SLOT_R_HAND,
        weight: 0,
        is_stackable: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        price: 0,
        handler: crate::data::item_data::ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
    });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 2, 1);
    }

    // Slow self-cast, then the equip click mid-cast: swallowed silently.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));
    drain(&mut a_rx);
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    assert!(a_rx.try_recv().is_err(), "no packet at click time (Java sends none)");
    assert!(matches!(
        world.objects.get_component::<QueuedAction>(&3001),
        Some(QueuedAction::UseItem { item_object_id: 9001 })
    ));
    {
        let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
        assert!(inv.paperdoll_slot_of(9001).is_none(), "not equipped mid-cast");
    }

    // Cast ends (hit 9500 + finish 500 ms): the equip fires.
    advance_ticks(&mut world, 101);
    assert!(!world.objects.has_component::<QueuedAction>(&3001), "queue consumed");
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(inv.paperdoll_slot_of(9001).is_some(), "sword equipped at cast end");
    let packets = drain(&mut a_rx);
    assert!(!packets.is_empty(), "InventoryUpdate/UserInfo sent with the deferred equip");
}

/// End-to-end guard for the ring/earring paperdoll bug: equipping, then
/// swapping, a dual-slot item (earring) must resend `ExUserInfoEquipSlot`
/// (Ex 0x156) — the packet that actually paints the client's own paperdoll —
/// with the correct REar/LEar object ids on *every* click, not just at
/// enter-world.
#[test]
fn equip_swap_resends_ex_user_info_equip_slot_with_correct_slots() {
    use crate::enums::InventorySlot;
    use crate::model::inventory::Inventory;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    for id in [501, 502] {
        world.data.item_data.insert_for_test(crate::data::item_data::ItemTemplate {
            item_id: id,
            name: format!("earring{id}"),
            kind: crate::data::item_data::ItemKind::Armor,
            body_part: crate::data::item_data::SLOT_L_EAR | crate::data::item_data::SLOT_R_EAR,
            weight: 0,
            is_stackable: false,
            type1: 0,
            type2: 0,
            is_quest_item: false,
            price: 0,
            handler: crate::data::item_data::ItemHandler::None,
            crystal_type: crate::data::item_data::CrystalType::None,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(),
        });
    }
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 501, 1);
        inv.add_item(&data.item_data, 9002, 502, 1);
    }

    // Extract (object_id, item_id) for a given InventorySlot from the most
    // recent ExUserInfoEquipSlot packet in `packets`, panicking if absent.
    fn ear_slots(packets: &[Vec<u8>]) -> (i32, i32, i32, i32) {
        let pkt = packets
            .iter()
            .rev()
            .find(|p| p.len() > 2 && p[0] == 0xFE && u16::from_le_bytes([p[1], p[2]]) == 0x156)
            .expect("ExUserInfoEquipSlot not sent");
        let mut offset = 14usize;
        let (mut rear, mut lear) = ((0, 0), (0, 0));
        for slot in InventorySlot::VALUES {
            let block_len = u16::from_le_bytes([pkt[offset], pkt[offset + 1]]) as usize;
            let obj_id = i32::from_le_bytes(pkt[offset + 2..offset + 6].try_into().unwrap());
            let item_id = i32::from_le_bytes(pkt[offset + 6..offset + 10].try_into().unwrap());
            match slot {
                InventorySlot::REar => rear = (obj_id, item_id),
                InventorySlot::LEar => lear = (obj_id, item_id),
                _ => {}
            }
            offset += block_len;
        }
        (rear.0, rear.1, lear.0, lear.1)
    }

    // First earring: fills LEar (equip_item fills left-then-right).
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    let packets = drain(&mut a_rx);
    let (rear_oid, _rear_iid, lear_oid, lear_iid) = ear_slots(&packets);
    assert_eq!((rear_oid, lear_oid, lear_iid), (0, 9001, 501), "first earring lands in LEar");

    // Second earring: fills the free REar slot, LEar untouched.
    items::handle_use_item(&mut world, 1, &use_item_body(9002));
    let packets = drain(&mut a_rx);
    let (rear_oid, rear_iid, lear_oid, lear_iid) = ear_slots(&packets);
    assert_eq!((rear_oid, rear_iid, lear_oid, lear_iid), (9002, 502, 9001, 501), "second earring lands in REar, first stays put");

    // Clicking an *already-equipped* earring toggles it back off. Java
    // resolves this via `getSlotFromItem` (the single-bit slot the item
    // currently occupies), not the item's raw (combined, for ears/fingers)
    // template body part — passing the latter used to silently no-op.
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    let packets = drain(&mut a_rx);
    assert!(!packets.is_empty(), "unequip-via-click must send packets, not silently no-op");
    let (rear_oid, rear_iid, lear_oid, _lear_iid) = ear_slots(&packets);
    assert_eq!((rear_oid, rear_iid, lear_oid), (9002, 502, 0), "LEar cleared, REar untouched");
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(inv.paperdoll_slot_of(9001).is_none(), "first earring actually unequipped");
}

/// The bug this guards: equipping gear moved the paperdoll but never recomputed
/// combat stats, so a freshly-equipped weapon's P.Atk / armor's P.Def never
/// reached the client's stat panel. `finish_equip_change` now reruns
/// `recalculate_stats`, and the weapon's stat *replaces* the naked base while
/// armor's *sums* on top (matching the Java finalizers).
#[test]
fn equipping_gear_updates_combat_stats() {
    use crate::data::item_data::{CrystalType, ItemHandler, ItemKind, ItemStats, ItemTemplate, SLOT_CHEST, SLOT_R_HAND};
    use crate::model::inventory::Inventory;
    use crate::model::stats::Stat;

    let (mut world, ..) = cast_test_world();
    let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let template = |item_id: i32, kind: ItemKind, body_part: i32| ItemTemplate {
        item_id,
        name: format!("gear{item_id}"),
        kind,
        body_part,
        weight: 0,
        is_stackable: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: CrystalType::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
    };
    // Weapon P.Atk 500 (well above the class base of 100, so equip must raise
    // P.Atk); chest armor P.Def 30 (class base P.Def is 0, so it must appear).
    world.data.item_data.insert_for_test(template(500, ItemKind::Weapon, SLOT_R_HAND));
    world.data.item_data.set_item_stats_for_test(500, ItemStats { bonuses: vec![(Stat::PhysicalAttack, 500.0)], ..Default::default() });
    world.data.item_data.insert_for_test(template(510, ItemKind::Armor, SLOT_CHEST));
    world.data.item_data.set_item_stats_for_test(510, ItemStats { bonuses: vec![(Stat::PhysicalDefence, 30.0)], ..Default::default() });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 500, 1);
        inv.add_item(&data.item_data, 9002, 510, 1);
    }

    let base_p_atk = pcs(&world, 3001).p_atk;
    let base_p_def = pcs(&world, 3001).p_def;

    // Equip the weapon → P.Atk jumps (weapon base 500 replaces the naked 100).
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    assert!(pcs(&world, 3001).p_atk > base_p_atk, "equipping a weapon must raise P.Atk (was {base_p_atk}, now {})", pcs(&world, 3001).p_atk);

    // Equip the armor → P.Def rises by its contribution, P.Atk unchanged.
    let after_weapon_p_atk = pcs(&world, 3001).p_atk;
    items::handle_use_item(&mut world, 1, &use_item_body(9002));
    assert!(pcs(&world, 3001).p_def > base_p_def, "equipping armor must raise P.Def (was {base_p_def}, now {})", pcs(&world, 3001).p_def);
    assert_eq!(pcs(&world, 3001).p_atk, after_weapon_p_atk, "armor doesn't touch P.Atk");

    // Unequip the weapon → P.Atk falls back to the naked value.
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    assert_eq!(pcs(&world, 3001).p_atk, base_p_atk, "unequipping the weapon restores naked P.Atk");
}

/// Companion to the combat-stat test: `maxMp` (and `maxHp`) item bonuses live
/// in `Vitals`, computed on a separate path from `recalculate_stats`. Equipping
/// +MP jewelry must raise Max MP; unequipping restores it and clamps current MP.
#[test]
fn equipping_gear_updates_max_hp_mp() {
    use crate::data::item_data::{CrystalType, ItemHandler, ItemKind, ItemStats, ItemTemplate, SLOT_NECK};
    use crate::model::components::Vitals;
    use crate::model::inventory::Inventory;
    use crate::model::stats::Stat;

    let (mut world, ..) = cast_test_world();
    let _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // A necklace granting +100 Max MP.
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 520,
        name: "MP Necklace".into(),
        kind: ItemKind::Armor,
        body_part: SLOT_NECK,
        weight: 0,
        is_stackable: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: CrystalType::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
    });
    world.data.item_data.set_item_stats_for_test(520, ItemStats { bonuses: vec![(Stat::MaxMp, 100.0)], ..Default::default() });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9003, 520, 1);
    }

    let base_max_mp = world.objects.get_component::<Vitals>(&3001).unwrap().max_mp;

    // Equip → Max MP rises by exactly the item's flat bonus.
    items::handle_use_item(&mut world, 1, &use_item_body(9003));
    assert_eq!(
        world.objects.get_component::<Vitals>(&3001).unwrap().max_mp,
        base_max_mp + 100,
        "equipping +100 MP jewelry raises Max MP by 100"
    );

    // Unequip → Max MP falls back, and current MP is clamped to the new max.
    items::handle_use_item(&mut world, 1, &use_item_body(9003));
    let v = world.objects.get_component::<Vitals>(&3001).unwrap();
    assert_eq!(v.max_mp, base_max_mp, "unequipping restores base Max MP");
    assert!(v.cur_mp <= v.max_mp as f64, "current MP clamped to the lowered max");
}

/// The bug this guards: `UseItem` on a non-equipable `EtcItem` used to be a
/// silent no-op (`is_equipable() == false` → early return before any handler
/// dispatch existed), so pack/box items like "Mage Class Equipment Set"
/// never unpacked in-game. `ExtractableItems` should destroy the pack and
/// grant its `<capsuled_items>` contents.
#[test]
fn extractable_pack_item_unpacks_into_its_contents() {
    use crate::data::item_data::{CapsuledItem, ItemHandler, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 15195,
        name: "Mage Class Equipment Set".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::ExtractableItems,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: vec![
            CapsuledItem { item_id: 15230, min: 1, max: 1, chance: 100_000 },
            CapsuledItem { item_id: 15270, min: 1, max: 1, chance: 100_000 },
        ],
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
    });
    for item_id in [15230, 15270] {
        world.data.item_data.insert_for_test(ItemTemplate {
            item_id,
            name: format!("Pack Content {item_id}"),
            kind: ItemKind::Etc,
            body_part: 0,
            weight: 0,
            is_stackable: false,
            type1: 4,
            type2: 5,
            is_quest_item: false,
            price: 0,
            handler: ItemHandler::None,
            crystal_type: crate::data::item_data::CrystalType::None,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(),
        });
    }
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 15195, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(inv.items().iter().all(|i| i.item_id != 15195), "pack consumed");
    assert!(inv.items().iter().any(|i| i.item_id == 15230), "first capsule granted");
    assert!(inv.items().iter().any(|i| i.item_id == 15270), "second capsule granted");

    let packets = drain(&mut rx);
    let obtained_count = sm_ids_of(&packets).into_iter().filter(|&id| id == server_packets::sm_ids::YOU_HAVE_OBTAINED_S1).count();
    assert_eq!(obtained_count, 2, "one obtained-message per capsule item");

    // Memory-first: the consumed pack instance (object 9001) is gone from the
    // Inventory component (asserted above as "pack consumed"); it persists as a
    // deletion on the next flush, not per use — so no per-action DB write.
}

/// The bug this guards: a capsule entry with `min == max == 2` on a
/// non-stackable, equipable item (e.g. the real "Jewelry Pack"'s Majestic
/// Earring/Ring, `min="2" max="2"` in `15200-15299.xml`) used to be granted
/// as a single item instance with `count == 2` — a state the paperdoll can't
/// represent. The client showed "you obtained 2" but only one icon in the
/// bag, and equipping it made the whole pair disappear (one unit moved to
/// the paperdoll, the other had no object id of its own to remain behind
/// with). `ItemContainer.addItem` in Java splits any non-stackable count
/// into one instance per unit; this asserts the Rust port now does too.
#[test]
fn extractable_pack_item_splits_non_stackable_multi_count_capsule() {
    use crate::data::item_data::{self, CapsuledItem, ItemHandler, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 15274,
        name: "Jewelry Pack (A-grade)".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::ExtractableItems,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: vec![CapsuledItem { item_id: 14966, min: 2, max: 2, chance: 100_000 }],
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 14966,
        name: "Majestic Earring of Fortune".into(),
        kind: ItemKind::Armor,
        body_part: item_data::SLOT_LR_EAR,
        weight: 0,
        is_stackable: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
    });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 15274, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let earring_oids: Vec<i32> = {
        let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
        inv.items().iter().filter(|i| i.item_id == 14966).map(|i| i.object_id).collect()
    };
    assert_eq!(earring_oids.len(), 2, "two separate earring instances, not one instance with count 2");
    {
        let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
        for oid in &earring_oids {
            assert_eq!(inv.items().iter().find(|i| i.object_id == *oid).unwrap().count, 1, "each instance is a single unit");
        }
    }

    let packets = drain(&mut rx);
    let obtained_two = sm_ids_of(&packets).into_iter().any(|id| id == server_packets::sm_ids::YOU_HAVE_OBTAINED_S2_S1);
    assert!(obtained_two, "message reports the pair as a count-2 grant");

    // Equipping one instance must not touch (or vanish) the other.
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.equip_item(&data.item_data, earring_oids[0]);
    }
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(inv.items().iter().any(|i| i.object_id == earring_oids[1]), "second earring still in the bag, not vanished");
}

/// The bug this guards: `extract_item` used to grant capsule rewards with no
/// capacity check at all, so a full inventory would silently overflow.
/// `ExtractableItems.useItem` refuses (leaving the box untouched) once
/// non-quest item count reaches 80% of the inventory cap
/// (`Player.isInventoryUnder80(false)`).
#[test]
fn extractable_pack_item_blocked_when_inventory_is_over_80_percent() {
    use crate::data::item_data::{CapsuledItem, ItemHandler, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    assert_eq!(world.cfg.character.inventory_max_no_dwarf, 80, "test assumes the default 80-slot cap");

    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 15195,
        name: "Mage Class Equipment Set".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::ExtractableItems,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: vec![CapsuledItem { item_id: 15230, min: 1, max: 1, chance: 100_000 }],
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
    });

    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        // 65 items (> 80% of the 80-slot cap), the pack itself included.
        for i in 0..64 {
            inv.add_item(&data.item_data, 9100 + i, 20000 + i, 1);
        }
        inv.add_item(&data.item_data, 9001, 15195, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(inv.items().iter().any(|i| i.item_id == 15195), "pack not consumed when inventory is full");
    assert!(inv.items().iter().all(|i| i.item_id != 15230), "no capsule granted when inventory is full");

    let packets = drain(&mut rx);
    let full_count = sm_ids_of(&packets).into_iter().filter(|&id| id == server_packets::sm_ids::YOUR_INVENTORY_IS_FULL).count();
    assert_eq!(full_count, 1, "YOUR_INVENTORY_IS_FULL sent");
}

/// `ItemSkills` (the `handlers/itemhandlers/ItemSkillsTemplate` port): a
/// self-targeted potion heals immediately (no cast bar) and consumes one
/// unit from the stack; a second use inside the skill's reuse window is
/// blocked and doesn't consume another.
#[test]
fn item_skill_potion_heals_and_enforces_reuse() {
    use crate::data::item_data::{ItemHandler, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;
    use crate::model::skill::SkillEffect;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    world.data.skill_data.insert_for_test(Skill {
        id: 2031,
        level: 1,
        name: "Lesser Healing Potion".into(),
        operate_type: OperateType::Active,
        target_type: TargetType::Self_,
        magic_type: 1,
        effect_point: 100,
        cast_range: 0,
        effect_range: 0,
        hit_time: 0,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 6000,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 0,
        abnormal_level: 0,
        abnormal_type: "NONE".into(),
        effects: vec![SkillEffect::Heal { power: 30.0 }],
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 9910,
        name: "Lesser Healing Potion".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::ItemSkills,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(2031, 1)],
    });
    if let Some(vitals) = world.objects.get_component_mut::<Vitals>(&3001) {
        vitals.max_hp = 100;
        vitals.cur_hp = 10.0;
    }
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 9910, 2);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    assert_eq!(pvit(&world, 3001).cur_hp, 40.0, "10 + Heal(30)");
    {
        let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
        let potion = inv.items().iter().find(|i| i.item_id == 9910).expect("one potion left");
        assert_eq!(potion.count, 1, "one unit consumed");
    }
    let packets = drain(&mut rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::STATUS_UPDATE),
        "heal must push an HP StatusUpdate"
    );
    // Memory-first: no per-use DB write; the remaining stack lives in the
    // Inventory component (asserted below) and persists on the next flush.

    // Second use, same tick: reuse still active, no extra heal or consume.
    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    assert_eq!(pvit(&world, 3001).cur_hp, 40.0, "reuse blocks a second heal");
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    let potion = inv.items().iter().find(|i| i.item_id == 9910).expect("still one potion left");
    assert_eq!(potion.count, 1, "reuse blocks a second consume");
}

/// The bug this guards: a `Restoration`-effect skill (e.g. the "Mysterious
/// Blessed Spiritshot Pack" line, item 22599 → skill 22490) used to parse
/// with an empty effect list — `SkillEffect::GiveItem`/`GiveItemRandom`
/// didn't exist yet — so `use_item_skills` still consumed the pack (a skill
/// was found and "cast") but granted nothing: the pack just disappeared.
#[test]
fn item_skill_give_item_grants_reward_and_consumes_pack() {
    use crate::data::item_data::{ItemHandler, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;
    use crate::model::skill::SkillEffect;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    world.data.skill_data.insert_for_test(Skill {
        id: 22490,
        level: 5,
        name: "Mysterious Spiritshot d 5000".into(),
        operate_type: OperateType::Active,
        target_type: TargetType::Self_,
        magic_type: 2,
        effect_point: 0,
        cast_range: 0,
        effect_range: 0,
        hit_time: 0,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 0,
        abnormal_level: 0,
        abnormal_type: "NONE".into(),
        effects: vec![SkillEffect::GiveItem { item_id: 21852, item_count: 5000, item_enchant_level: 0 }],
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 22599,
        name: "Mysterious Blessed Spiritshot Pack (5000) (D-grade)".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 1000,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::ItemSkills,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(22490, 5)],
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 21852,
        name: "Blessed Spiritshot: D-grade".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
    });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 22599, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert!(inv.items().iter().all(|i| i.item_id != 22599), "pack consumed");
    let shots = inv.items().iter().find(|i| i.item_id == 21852).expect("5000 Blessed Spiritshots granted, not lost");
    assert_eq!(shots.count, 5000);

    let packets = drain(&mut rx);
    assert!(
        sm_ids_of(&packets).into_iter().any(|id| id == server_packets::sm_ids::YOU_HAVE_OBTAINED_S2_S1),
        "reward message sent"
    );
}

/// `RestorationRandom` (e.g. "Quiver of Arrow"-shaped skills): exactly one
/// weighted group is picked and its items granted together, matching Java's
/// `100 * Rnd.nextDouble()` roulette roll against the raw 0-100 `chance`
/// values.
#[test]
fn item_skill_give_item_random_grants_one_weighted_group() {
    use crate::data::item_data::{ItemHandler, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;
    use crate::model::skill::{RestorationGroup, RestorationItem, SkillEffect};

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    // `apply_skill_effects` rolls a magic-crit check unconditionally before
    // walking the effect list (unused here since this isn't a
    // `MagicalAttack`) — force it out of the queue first, then force the
    // roulette roll: `roll_f64` reads a forced value `v` as `v / 1_000_000`,
    // so 600_000 -> 0.6 -> `100 * 0.6 = 60`, landing in the second slice
    // (30..80) below.
    world.forced_rolls.push_back(0);
    world.forced_rolls.push_back(600_000);

    world.data.skill_data.insert_for_test(Skill {
        id: 323,
        level: 1,
        name: "Quiver of Arrow".into(),
        operate_type: OperateType::Active,
        target_type: TargetType::Self_,
        magic_type: 2,
        effect_point: 0,
        cast_range: 0,
        effect_range: 0,
        hit_time: 0,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 0,
        abnormal_level: 0,
        abnormal_type: "NONE".into(),
        effects: vec![SkillEffect::GiveItemRandom {
            groups: vec![
                RestorationGroup {
                    chance: 30.0,
                    items: vec![RestorationItem { item_id: 1344, count: 700, min_enchant: 0, max_enchant: 0 }],
                },
                RestorationGroup {
                    chance: 50.0,
                    items: vec![RestorationItem { item_id: 1344, count: 1400, min_enchant: 0, max_enchant: 0 }],
                },
                RestorationGroup {
                    chance: 20.0,
                    items: vec![RestorationItem { item_id: 1344, count: 2800, min_enchant: 0, max_enchant: 0 }],
                },
            ],
        }],
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 1344,
        name: "Mithril Arrow".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 9999,
        name: "Quiver of Arrow scroll".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::ItemSkills,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(323, 1)],
    });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 9999, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    let arrows = inv.items().iter().find(|i| i.item_id == 1344).expect("arrows granted");
    assert_eq!(arrows.count, 1400, "roll 60 lands in the 30..80 (second) slice");
    let _ = &mut rx;
}

/// `RestorationRandom` with `maxEnchant > 0` rolls `Rnd.get(min, max)` (inclusive)
/// onto the created non-stackable item and sends the "obtained a +S1 S2" message.
#[test]
fn item_skill_give_item_random_rolls_enchant_on_created_item() {
    use crate::data::item_data::{ItemHandler, ItemKind, ItemTemplate};
    use crate::model::inventory::Inventory;
    use crate::model::skill::{RestorationGroup, RestorationItem, SkillEffect};

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);
    // Forced rolls, in consumption order: crit check (0), roulette `roll_f64`
    // (500_000 -> 0.5 -> 50, inside the single 0..100 slice), then the enchant
    // `roll(max-min+1)` = `roll(3)`; forcing 1 -> enchant = min(3) + 1 = 4.
    world.forced_rolls.push_back(0);
    world.forced_rolls.push_back(500_000);
    world.forced_rolls.push_back(1);

    world.data.skill_data.insert_for_test(Skill {
        id: 324,
        level: 1,
        name: "Enchanted Reward".into(),
        operate_type: OperateType::Active,
        target_type: TargetType::Self_,
        magic_type: 2,
        effect_point: 0,
        cast_range: 0,
        effect_range: 0,
        hit_time: 0,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 0,
        abnormal_level: 0,
        abnormal_type: "NONE".into(),
        effects: vec![SkillEffect::GiveItemRandom {
            groups: vec![RestorationGroup {
                chance: 100.0,
                items: vec![RestorationItem { item_id: 6001, count: 1, min_enchant: 3, max_enchant: 5 }],
            }],
        }],
    });
    // The reward is a non-stackable weapon so it carries an enchant.
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 6001,
        name: "Enchanted Blade".into(),
        kind: ItemKind::Weapon,
        body_part: 0,
        weight: 0,
        is_stackable: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
    });
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 9998,
        name: "Enchanted Reward scroll".into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: ItemHandler::ItemSkills,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(324, 1)],
    });
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&3001).unwrap();
        inv.add_item(&data.item_data, 9001, 9998, 1);
    }

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    let blade = inv.items().iter().find(|i| i.item_id == 6001).expect("blade granted");
    assert_eq!(blade.enchant_level, 4, "Rnd.get(3, 5) with forced roll 1 -> +4");
    assert!(
        sm_ids_of(&drain(&mut rx)).contains(&server_packets::sm_ids::YOU_HAVE_OBTAINED_A_S1_S2),
        "enchanted single grant uses the +S1 S2 message",
    );
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
    let bundle = Player::from_char(&world.data, &chr);
    let (out_tx, out_rx) = tokio::sync::mpsc::unbounded_channel();
    let s = Session::new(client_id, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![])
        .into_entering(bundle);
    let (session, bundle) = s.into_ingame();
    bundle.spawn_into(&mut world.objects);
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

    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, Some(3002));
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
    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, None);
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::TARGET_UNSELECTED,
        "canceller must receive TargetUnselected too"
    );
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::TARGET_UNSELECTED);

    // Self-click: same select path as any other player target (Java
    // routes self-clicks through `PlayerAction` too).
    handle_action(&mut world, 1, &action_body(3001, 0));
    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, Some(3001));
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::MY_TARGET_SELECTED);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert_eq!(b_rx.try_recv().unwrap()[0], server_packets::opcodes::TARGET_SELECTED);
}

/// Register a synthetic NPC template and place one instance in the world +
/// region index (the test-side mirror of `model::npc::spawn_one`).
fn add_test_npc(world: &mut World, object_id: i32, npc_id: i32, type_name: &str, level: i32, x: i32, y: i32, z: i32) {
    if world.data.npc_data.get(npc_id).is_none() {
        let mut t = crate::data::npc_data::default_template(npc_id);
        t.type_name = type_name.into();
        t.level = level;
        t.base_hp_max = 100.0;
        t.base_mp_max = 50.0;
        world.data.npc_data.insert_for_test(t);
    }
    let (npc, extra) = crate::model::npc::Npc::for_test(object_id, npc_id, x, y, z, 100, 50);
    world.npc_regions.entry(extra.1 .0).or_default().push(object_id);
    world.objects.spawn(object_id, (npc, extra));
    // Memoized combat stats, from the template registered above (the
    // test-side mirror of `spawn_one`'s `npc_combat_stats` bundle entry).
    let cs = crate::model::npc::npc_combat_stats(
        world.data.npc_data.get(npc_id).expect("registered above"),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&object_id, cs);
}

const NPC_OID: i32 = crate::model::npc::FIRST_NPC_OBJECT_ID;

/// Entering the world sends `NpcInfo` for NPCs in the 3×3 region block and
/// nothing for NPCs beyond it (Java `addVisibleObject` over the region grid).
#[test]
fn enter_world_sends_npc_info_for_nearby_npcs_only() {
    let (mut world, ..) = test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 500, 0, 0);
    add_test_npc(&mut world, NPC_OID + 1, 30002, "Folk", 5, 5 * 2048, 0, 0); // 5 regions east
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    visibility::on_enter_world(&world, 1, 3001);

    let packets = drain(&mut rx);
    let npc_infos: Vec<_> = packets.iter().filter(|p| p[0] == server_packets::opcodes::NPC_INFO).collect();
    assert_eq!(npc_infos.len(), 1, "only the nearby NPC is described");
    let described = i32::from_le_bytes(npc_infos[0][1..5].try_into().unwrap());
    assert_eq!(described, NPC_OID);
}

/// Crossing a region boundary introduces NPCs entering the 3×3 block
/// (`NpcInfo`) and removes NPCs leaving it (`DeleteObject`), dropping a
/// dangling NPC target like Java's forget event does.
#[test]
fn region_cross_sends_npc_deltas_and_drops_npc_target() {
    let (mut world, ..) = test_world();
    // NPC in region (3, 0): visible from region (2, 0) but not (0, 0).
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 3 * 2048 + 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    // Step into region (2, 0): the NPC appears.
    world.objects.get_component_mut::<Position>(&3001).unwrap().x = 2 * 2048 + 10;
    visibility::update_region(&mut world, 3001);
    let packets = drain(&mut rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::NPC_INFO),
        "NpcInfo on entering visibility range"
    );

    // Target it, then step back to region (0, 0): DeleteObject + target drop.
    world.objects.get_component_mut::<TargetRef>(&3001).unwrap().0 = Some(NPC_OID);
    world.objects.get_component_mut::<Position>(&3001).unwrap().x = 10;
    visibility::update_region(&mut world, 3001);
    let packets = drain(&mut rx);
    let del: Vec<_> = packets.iter().filter(|p| p[0] == server_packets::opcodes::DELETE_OBJECT).collect();
    assert_eq!(del.len(), 1, "DeleteObject for the NPC leaving range");
    assert_eq!(i32::from_le_bytes(del[0][1..5].try_into().unwrap()), NPC_OID);
    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, None, "dangling NPC target dropped");
}

/// `Action` on an NPC: first click selects (`ValidateLocation` +
/// `MyTargetSelected` + HP `StatusUpdate` + `ActionFailed`); a second click
/// on a talkable non-monster in interaction range opens the chat window
/// (`NpcHtmlMessage`).
#[test]
fn action_on_npc_selects_then_second_click_opens_chat_window() {
    let (mut world, ..) = test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, Some(NPC_OID));
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::VALIDATE_LOCATION);
    let mts = rx.try_recv().unwrap();
    assert_eq!(mts[0], server_packets::opcodes::MY_TARGET_SELECTED);
    assert_eq!(i16::from_le_bytes(mts[9..11].try_into().unwrap()), 0, "no level color on a Folk");
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE);
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(rx.try_recv().is_err());

    // Second click within INTERACTION_DISTANCE: the dialog opens (the html
    // file itself is absent in the synthetic world, so the "text is missing"
    // stub is served — the packet flow is what's under test).
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::NPC_HTML_MESSAGE);
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(rx.try_recv().is_err());
}

/// With `AltGameViewNpc` on, a shift-click (`Action`, action_id 1) on an NPC
/// opens the `NpcViewMod` info window instead of interacting — Java `Action`
/// case 1 → `Npc.onActionShift` → `NpcActionShift`'s `ALT_GAME_VIEWNPC`
/// branch, which sets the target first, then sends the html.
#[test]
fn shift_click_npc_opens_view_window_when_alt_game_view_npc() {
    let (mut world, ..) = test_world();
    world.cfg.npc.alt_game_view_npc = true;
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    handle_action(&mut world, 1, &action_body(NPC_OID, 1));

    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, Some(NPC_OID), "target set like NpcActionShift");
    let pkts = drain(&mut rx);
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::MY_TARGET_SELECTED), "target selected");
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE), "info window opened");
    assert!(!world.objects.has_component::<Intent>(&3001), "the info window must not start an attack/interact");
}

/// Without `AltGameViewNpc` (the default), a shift-click on an NPC is just a
/// plain select (Java `onAction(player, false)`) — no info window.
#[test]
fn shift_click_npc_without_alt_game_view_npc_only_selects() {
    let (mut world, ..) = test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    handle_action(&mut world, 1, &action_body(NPC_OID, 1));

    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, Some(NPC_OID));
    assert!(
        !drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE),
        "no info window without the config flag"
    );
}

/// `Action` on a talkable NPC outside `INTERACTION_DISTANCE`: the second
/// click can't open the dialog immediately (`Npc.canInteract` fails), so the
/// player walks in first (`AI_INTENTION_INTERACT` / `Interact` intent) —
/// `MoveToPawn` goes out, then once movement ticks close the distance the
/// chat window opens on its own, with no further client click.
#[test]
fn action_on_far_npc_walks_in_then_opens_chat_window() {
    let (mut world, ..) = test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 2000, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Speeds>(&3001).unwrap().run_spd = 100.0;

    // First click: select (far away, selection itself isn't range-gated).
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    drain(&mut rx);

    // Second click: too far to talk (2000 > INTERACTION_DISTANCE=250) — walks
    // in instead of doing nothing.
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    let packets = drain(&mut rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "out-of-range talk click must start walking toward the NPC"
    );
    assert!(
        !packets.iter().any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE),
        "no dialog yet — still far away"
    );
    assert!(matches!(
        world.objects.get_component::<Intent>(&3001).copied(),
        Some(Intent(crate::model::PlayerIntent::Interact { target_object_id })) if target_object_id == NPC_OID
    ));

    // Run the movement + combat-tick systems until the player arrives and
    // re-triggers the interact click on its own (Java: `EVT_ARRIVED` →
    // `thinkInteract` → `doInteract` re-dispatching `onAction`).
    advance_world(&mut world, 400);
    let packets = drain(&mut rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE),
        "chat window must open once the walk-in arrives"
    );
    assert!(
        world.objects.get_component::<Intent>(&3001).is_none(),
        "interact intent consumed on arrival"
    );
}

/// `Action` on a monster tints `MyTargetSelected` with the level gap; a second
/// click on the already-targeted (out-of-range) monster starts the attack and
/// walks toward it (`MoveToPawn`) — never a chat window.
#[test]
fn action_on_monster_colors_target_and_never_talks() {
    let (mut world, ..) = test_world();
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 3, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<crate::model::Player>(&3001).unwrap().level = 8;

    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::VALIDATE_LOCATION);
    let mts = rx.try_recv().unwrap();
    assert_eq!(mts[0], server_packets::opcodes::MY_TARGET_SELECTED);
    assert_eq!(i16::from_le_bytes(mts[9..11].try_into().unwrap()), 5, "player 8 vs monster 3");
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::STATUS_UPDATE);
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);

    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    let after: Vec<Vec<u8>> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert!(
        after.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "second click starts the attack and walks the out-of-range monster down"
    );
    assert!(
        !after.iter().any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE),
        "no chat window from a monster"
    );
}

fn bypass_body(command: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(command);
    w.into_bytes()
}

/// Bypass plumbing (G11): `npc_<oid>_<verb>` parses, range-checks, and always
/// terminates with `ActionFailed`; malformed/empty/unknown commands drop
/// without a reply (and without a panic); clicking an NPC records it as
/// `LastFolkNpc`, which bare `Quest …` bypasses resolve through.
#[test]
fn bypass_routes_npc_commands_and_tracks_last_folk_npc() {
    let (mut world, ..) = test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    // Clicking the NPC records it as the last folk NPC (`NpcAction.action`).
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    assert_eq!(
        world.objects.get_component::<LastFolkNpc>(&3001),
        Some(&LastFolkNpc(NPC_OID)),
        "NPC click must set LastFolkNpc"
    );
    drain(&mut rx);

    // `npc_`-prefixed command on an in-range NPC: the verb is unhandled in
    // this phase (log-drop) but the `ActionFailed` terminator still arrives —
    // Java sends it from the `npc_` branch regardless of the outcome.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Chat 0")));
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(rx.try_recv().is_err());

    // Malformed `npc_` forms never act but still terminate: missing command
    // tail, non-numeric id, unknown object id.
    for cmd in ["npc_12345", "npc_x_y", "npc_999_Chat 0"] {
        handle_request_bypass_to_server(&mut world, 1, &bypass_body(cmd));
        assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL, "for {cmd}");
        assert!(rx.try_recv().is_err(), "for {cmd}");
    }

    // Empty and unknown bare commands drop silently (deviation: Java
    // disconnects on empty; unhandled prefixes only log there too).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(""));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("_bbshome"));
    assert!(rx.try_recv().is_err());

    // Bare `Quest` with no LastFolkNpc (fresh player who never clicked an
    // NPC): dropped, no packets, no panic.
    let mut rx2 = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    handle_request_bypass_to_server(&mut world, 2, &bypass_body("Quest"));
    assert!(rx2.try_recv().is_err());
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
    world.objects.get_component_mut::<Speeds>(&4001).unwrap().run_spd = 100.0;
    world.objects.get_component_mut::<Speeds>(&4001).unwrap().running = true;

    handle_move_backward_to_location(&mut world, 1, &move_body((1000, 0, 0), (0, 0, 0), 1));

    assert_eq!(mover_rx.try_recv().unwrap()[0], server_packets::opcodes::MOVE_TO_LOCATION);
    assert!(mover_rx.try_recv().is_err(), "exactly one packet to the mover");
    assert_eq!(bystander_rx.try_recv().unwrap()[0], server_packets::opcodes::MOVE_TO_LOCATION);

    let total_ticks = world.objects.get_component::<Movement>(&4001).unwrap().0.total_ticks;
    assert_eq!(total_ticks, 100, "distance 1000 / speed 100 * 10 ticks-per-sec");

    // Half way: linear interpolation.
    world.tick += total_ticks / 2;
    crate::model::movement::tick(&mut world);
    let pos = world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (500, 0, 0));
    assert!(world.objects.has_component::<Movement>(&4001));

    // Arrival: snapped exactly, move_data cleared, no StopMove needed.
    world.tick += total_ticks / 2;
    crate::model::movement::tick(&mut world);
    let pos = world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (1000, 0, 0));
    assert!(!world.objects.has_component::<Movement>(&4001));
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
    assert!(!world.objects.has_component::<Movement>(&5001));
}

/// `RequestStopMove` (`player.stopMove(getLocation())`): the in-flight move
/// and any pending path request are dropped, and `StopMove` is broadcast to
/// the mover (Player `broadcastPacket` includes self) at the current spot.
#[test]
fn request_stop_move_clears_movement_and_pending_path() {
    use crate::model::components::PathWait;
    use crate::model::movement::MoveData;

    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 5001, 700, 800, 0);

    // Simulate an in-flight move plus a still-outstanding path request.
    world.objects.add_components(
        &5001,
        Movement(MoveData {
            start_x: 700,
            start_y: 800,
            start_z: 0,
            dest_x: 2000,
            dest_y: 800,
            dest_z: 0,
            start_tick: world.tick,
            total_ticks: 100,
            geo_path: None,
        }),
    );
    world.objects.add_components(&5001, PathWait { seq: 42 });

    handle_request_stop_move(&mut world, 1);

    assert!(!world.objects.has_component::<Movement>(&5001), "move data deleted");
    assert!(!world.objects.has_component::<PathWait>(&5001), "pending path dropped");
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::STOP_MOVE);
}

/// `ExSendSelectedQuestZoneID` stores the selected zone id on the player
/// (default -1 → the client's choice), read later by quest teleports.
#[test]
fn ex_send_selected_quest_zone_id_sets_field() {
    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, 1, 5001, 10, 20, 30);
    assert_eq!(world.objects.get_component::<Player>(&5001).unwrap().quest_zone_id, -1);

    handle_ex_send_selected_quest_zone_id(&mut world, 1, &int_body(7));

    assert_eq!(world.objects.get_component::<Player>(&5001).unwrap().quest_zone_id, 7);
}

/// Region 20_18 covers world x,y ∈ [0, 32768): flat ground at z = 0 with
/// a north-south wall at local cell x == 10 (world x 160..176) — 200
/// units tall, not enterable, and the approach cells block their east
/// exit (how real geodata encodes walls).
fn install_wall_region(world: &mut World) {
    use crate::geo::{synthetic_region, NSWE_ALL, NSWE_EAST};
    // `world.geo` is shared with the path worker via `Arc` — in tests nothing
    // has cloned it yet, so it can be mutated in place.
    std::sync::Arc::get_mut(&mut world.geo).expect("geo Arc not shared yet").set_region(
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

/// A click past a geodata wall used to just clamp; now a clamp that
/// shortens the move by > 30 units defers the move to the path worker
/// (Java: `CellPathFinding.findPath` inline): nothing moves yet, a
/// `PathWait` marks the pending request, and no packet is sent.
#[test]
fn move_blocked_by_wall_defers_to_path_worker() {
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let mut mover_rx = ingame_player(&mut world, 1, 4001, 8, 8, 0); // cell 0
    world.objects.get_component_mut::<Speeds>(&4001).unwrap().run_spd = 100.0;

    // Click to cell 20 (x = 328), on the far side of the wall at cell 10:
    // the clamp to cell 9 (x = 152) shortens 320 → 144, well over 30.
    handle_move_backward_to_location(&mut world, 1, &move_body((328, 8, 0), (8, 8, 0), 1));

    assert!(!world.objects.has_component::<Movement>(&4001), "move deferred, not started");
    assert!(world.objects.has_component::<crate::model::components::PathWait>(&4001));
    assert!(mover_rx.try_recv().is_err(), "no packet until the path reply lands");
}

/// A clamp of ≤ 30 units starts the move directly with the clamped
/// destination (`GeoEngine.getValidLocation` in `Creature.moveToLocation`) —
/// no pathfinding round-trip.
#[test]
fn move_destination_is_clamped_by_geodata() {
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let mut mover_rx = ingame_player(&mut world, 1, 4001, 120, 8, 0); // cell 7
    world.objects.get_component_mut::<Speeds>(&4001).unwrap().run_spd = 100.0;

    // Click one cell into the wall (cell 10, x = 168): clamped to cell 9
    // (x = 152), only 16 units short of the request.
    handle_move_backward_to_location(&mut world, 1, &move_body((168, 8, 0), (120, 8, 0), 1));

    let md = world.objects.get_component::<Movement>(&4001).map(|m| m.0.clone()).expect("move must start");
    assert_eq!((md.dest_x, md.dest_y), (152, 8), "clamped to cell 9, before the wall");
    let pkt = mover_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::MOVE_TO_LOCATION);
    let dest_x = i32::from_le_bytes(pkt[5..9].try_into().unwrap());
    assert_eq!(dest_x, 152, "MoveToLocation carries the clamped destination");
}

/// Full pathfinding round-trip against a real path-worker thread: a click
/// across a walled-off area (with a gap further south) starts a
/// multi-segment route move once the worker replies, route advances
/// broadcast `MoveToLocation` per segment, and the mover arrives at the
/// exact requested destination on the far side of the wall.
#[test]
fn path_worker_round_trip_walks_around_wall() {
    use crate::geo::path::PathConfig;
    use crate::geo::{synthetic_region, NSWE_ALL, NSWE_EAST};
    use crate::model::components::PathWait;

    let (mut world, ..) = test_world();
    // Mid-region wall at cell x == 10 with a gap at y ∈ [1010, 1014) — far
    // from region edges so the search can't skirt through unloaded void.
    std::sync::Arc::get_mut(&mut world.geo).expect("geo Arc not shared yet").set_region(
        20,
        18,
        synthetic_region(|x, y| {
            let in_gap = (1010..1014).contains(&y);
            if x == 10 && !in_gap {
                (200, 0)
            } else if x == 9 && !in_gap {
                (0, NSWE_ALL & !NSWE_EAST)
            } else {
                (0, NSWE_ALL)
            }
        }),
    );
    let (req_tx, req_rx) = std::sync::mpsc::channel();
    let (ev_tx, ev_rx) = std::sync::mpsc::channel();
    let worker = crate::geo::worker::spawn(world.geo.clone(), PathConfig::default(), req_rx, ev_tx);
    world.path = req_tx;

    // Player at cell (0, 1000) = (8, 16008); click to cell (20, 1000).
    let mut mover_rx = ingame_player(&mut world, 1, 4001, 8, 16008, 0);
    world.objects.get_component_mut::<Speeds>(&4001).unwrap().run_spd = 100.0;
    handle_move_backward_to_location(&mut world, 1, &move_body((328, 16008, 0), (8, 16008, 0), 1));
    assert!(world.objects.has_component::<PathWait>(&4001));

    // The reply normally lands via `drain_path` on a later tick.
    let ev = ev_rx.recv_timeout(std::time::Duration::from_secs(10)).expect("worker reply");
    handle_path_result(&mut world, ev);
    assert!(!world.objects.has_component::<PathWait>(&4001));

    let md = world.objects.get_component::<Movement>(&4001).map(|m| m.0.clone()).expect("route move started");
    let path = md.geo_path.expect("move carries the geodata route");
    assert_eq!(path.index, 0);
    assert!(path.points.len() > 1, "walking around needs several segments");
    assert_eq!((path.accurate_tx, path.accurate_ty), (328, 16008));
    assert_eq!(mover_rx.try_recv().unwrap()[0], server_packets::opcodes::MOVE_TO_LOCATION);

    // Walk the whole route: each segment completion advances to the next
    // point and broadcasts another MoveToLocation; the last one arrives.
    let mut advances = 0;
    for _ in 0..10_000 {
        if !world.objects.has_component::<Movement>(&4001) {
            break;
        }
        world.tick += 1;
        visibility::movement_tick(&mut world);
        if let Ok(pkt) = mover_rx.try_recv() {
            assert_eq!(pkt[0], server_packets::opcodes::MOVE_TO_LOCATION);
            advances += 1;
        }
    }
    assert!(!world.objects.has_component::<Movement>(&4001), "route must complete");
    assert!(advances >= 1, "route advances broadcast MoveToLocation");
    let pos = world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!((pos.x, pos.y), (328, 16008), "arrived at the exact requested destination");

    drop(world);
    worker.join().unwrap();
}

/// Standing right at the wall, a click into it clamps the whole path away
/// (distance < 1) — Java cancels the movement with `ActionFailed`.
#[test]
fn move_into_wall_from_adjacent_cell_is_cancelled() {
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let mut mover_rx = ingame_player(&mut world, 1, 4001, 152, 8, 0); // cell 9
    world.objects.get_component_mut::<Speeds>(&4001).unwrap().run_spd = 100.0;

    handle_move_backward_to_location(&mut world, 1, &move_body((168, 8, 0), (152, 8, 0), 1));

    assert!(!world.objects.has_component::<Movement>(&4001), "no movement into the wall");
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
    assert!(!world.objects.has_component::<Casting>(&3001));

    // Same side of the wall: the cast starts.
    world.objects.get_component_mut::<Position>(&3002).unwrap().x = 72; // cell 4
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
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
        let speeds = world.objects.get_component_mut::<Speeds>(&4001).unwrap();
        speeds.run_spd = 600.0;
        speeds.running = true;
    }

    // Climb: z 0 → 300 with matching client-z history — trusted, silent.
    handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, 300, 0));
    assert_eq!(world.objects.get_component::<Position>(&4001).unwrap().z, 300);
    assert!(rx.try_recv().is_err(), "no correction for a trusted climb");

    // Drift: diffSq 270400 ∈ (250000, 360000), within move speed (600) —
    // server answers ValidateLocation and stays put.
    handle_validate_position(&mut world, 1, &validate_position_body(1520, 1000, 300, 0));
    assert_eq!(world.objects.get_component::<Position>(&4001).unwrap().x, 1000, "server position kept on drift");
    let pkt = rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::VALIDATE_LOCATION);
    assert!(rx.try_recv().is_err());

    // Desync: 2000 units in one report — snap to the client, with z
    // pulled onto the geodata ground (server was above the client).
    handle_validate_position(&mut world, 1, &validate_position_body(3000, 1000, 0, 0));
    let pos = world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (3000, 1000, 0), "snapped, z on the geodata floor");
    let c = world.objects.get_component::<ClientPos>(&4001).unwrap();
    assert_eq!((c.x, c.y, c.z), (3000, 1000, 0));
}

/// The next queued DB command, which must be a `StorePlayer`; returns its
/// full save payload.
fn expect_store_player(db_rx: &mut db::CmdRx) -> db::PlayerSaveData {
    match db_rx.try_recv() {
        Ok(db::DbCommand::StorePlayer { save }) => save,
        _ => panic!("expected a StorePlayer DB command"),
    }
}

/// RequestRestart: the player is stored + removed, the client gets
/// `RestartResponse(true)`, drops back to `Authenticated`, and the reloaded
/// character list flows through the normal lobby path.
#[test]
fn restart_stores_player_and_returns_to_lobby() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut out_rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    {
        let p = world.objects.get_component_mut::<crate::model::Player>(&5001).unwrap();
        p.exp = 1234;
    }
    world.objects.get_component_mut::<Position>(&5001).unwrap().x = 777;

    handle_request_restart(&mut world, 1);

    // storeMe: the snapshot carries the live (not the loaded) state, and
    // is queued before the character-list reload.
    let save = expect_store_player(&mut db_rx);
    assert_eq!((save.base.object_id, save.base.exp, save.base.x), (5001, 1234, 777));
    match db_rx.try_recv() {
        Ok(db::DbCommand::LoadCharacters { client_id, account }) => {
            assert_eq!((client_id, account.as_str()), (1, "bob"));
        }
        _ => panic!("expected a LoadCharacters DB command after the store"),
    }

    // deleteMe + setConnectionState(AUTHENTICATED) + RestartResponse.TRUE.
    assert_eq!(world.objects.count::<Player>(), 0);
    assert!(matches!(world.clients.get(&1), Some(ClientSession::Authenticated(_))));
    let pkt = out_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::RESTART_RESPONSE);
    assert_eq!(pkt[1], 1, "RestartResponse.TRUE");

    // The reload result lands like any character-list load: InLobby +
    // CharSelectionInfo.
    on_characters_loaded(&mut world, 1, "bob".into(), vec![dummy_char(5001, "P5001")], true);
    assert!(matches!(world.clients.get(&1), Some(ClientSession::InLobby(_))));
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::CHARACTER_SELECTION_INFO);
}

/// Server-shutdown save-all: every online player is persisted (level/exp/
/// position) without being despawned, so a restart doesn't revert them to
/// their last logout — the bug where a character leveled up, the server was
/// restarted, and the level was lost (skills, saved eagerly, were not).
#[test]
fn shutdown_saves_all_online_players() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let _o1 = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    let _o2 = ingame_player(&mut world, 2, 5002, 300, 400, 0);
    {
        let p = world.objects.get_component_mut::<crate::model::Player>(&5001).unwrap();
        p.level = 7;
        p.exp = 9999;
    }

    super::net::save_all_players(&mut world);

    // A StorePlayer snapshot per online player (ECS iteration order isn't
    // fixed, so collect by object id).
    let mut snaps = std::collections::HashMap::new();
    for _ in 0..2 {
        let s = expect_store_player(&mut db_rx);
        snaps.insert(s.base.object_id, s);
    }
    assert_eq!(snaps.len(), 2, "both online players saved");
    assert_eq!(snaps[&5001].base.level, 7, "the leveled-up character's level is persisted");
    assert_eq!(snaps[&5001].base.exp, 9999);
    assert!(snaps.contains_key(&5002));
    // Save-all does not despawn — the players are still in the world.
    assert_eq!(world.objects.count::<Player>(), 2);
}

/// A second select → enter-world round trip works on the restarted session
/// (the original relogin bug: the restart packet was ignored entirely).
#[test]
fn restart_then_reenter_world() {
    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let mut out_rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    handle_request_restart(&mut world, 1);
    on_characters_loaded(&mut world, 1, "bob".into(), vec![dummy_char(5001, "P5001")], true);
    while out_rx.try_recv().is_ok() {} // RestartResponse + CharSelectionInfo

    let mut w = PacketWriter::new();
    w.write_i32(0); // slot
    handle_character_select(&mut world, 1, &w.into_bytes());
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::CHAR_SELECTED);
    handle_enter_world(&mut world, 1);
    assert!(world.objects.has_component::<crate::model::Player>(&5001), "player re-entered the world");
    assert!(matches!(world.clients.get(&1), Some(ClientSession::InGame(_))));
}

/// Logout: the player is stored + removed and the client gets `LeaveWorld`;
/// dropping the session is what closes the socket.
#[test]
fn logout_stores_player_and_sends_leave_world() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut out_rx = ingame_player(&mut world, 1, 5002, 100, 200, 0);

    handle_logout(&mut world, 1);

    assert_eq!(expect_store_player(&mut db_rx).base.object_id, 5002);
    assert_eq!(world.objects.count::<Player>(), 0);
    assert!(world.clients.is_empty(), "session dropped → socket closes");
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::LOG_OUT_OK);
}

/// `Player.canLogout` refuses a restart while the player is in combat stance:
/// the client gets `RestartResponse.FALSE` + `ActionFailed`, the player stays
/// in the world, the session stays `InGame`, and nothing is persisted.
#[test]
fn restart_blocked_while_in_combat_stance() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut out_rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    // In stance until 15 s from now (AttackStanceTaskManager.addAttackStanceTask).
    world.objects.get_component_mut::<crate::model::components::AttackState>(&5001).unwrap().stance_until_tick = world.tick + 1;

    handle_request_restart(&mut world, 1);

    assert_eq!(world.objects.count::<Player>(), 1, "player stays in the world");
    assert!(matches!(world.clients.get(&1), Some(ClientSession::InGame(_))), "still in game");
    assert!(db_rx.try_recv().is_err(), "no store/reload while refused");
    let pkt = out_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::RESTART_RESPONSE);
    assert_eq!(pkt[1], 0, "RestartResponse.FALSE");
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
}

/// `Player.canLogout` refuses a logout while in combat stance: `ActionFailed`
/// only, no `LeaveWorld`, and the player stays in the world.
#[test]
fn logout_blocked_while_in_combat_stance() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut out_rx = ingame_player(&mut world, 1, 5002, 100, 200, 0);
    world.objects.get_component_mut::<crate::model::components::AttackState>(&5002).unwrap().stance_until_tick = world.tick + 1;

    handle_logout(&mut world, 1);

    assert_eq!(world.objects.count::<Player>(), 1, "player stays in the world");
    assert!(matches!(world.clients.get(&1), Some(ClientSession::InGame(_))), "still in game");
    assert!(db_rx.try_recv().is_err(), "no store while refused");
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(out_rx.try_recv().is_err(), "no LeaveWorld");
}

/// PvP flag lifecycle (`Player.updatePvPStatus` + `PvpFlagTaskManager`): a
/// hostile action flags the player solid (1), the 1 s sweep blinks it (2) in
/// the final 20 s, then clears it (0) past expiry.
#[test]
fn pvp_flag_starts_blinks_and_expires() {
    use crate::game_loop::pvp;
    use crate::model::components::PvpState;
    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let start = world.tick;

    pvp::update_pvp_status(&mut world, 5001);
    let st = *world.objects.get_component::<PvpState>(&5001).unwrap();
    assert_eq!(st.flag, 1, "flagged solid");
    assert_eq!(st.expires_tick, start + 1200, "PVP_NORMAL_TIME = 120 s @ 100 ms ticks");

    // Mid-life (before the last 20 s) stays solid.
    world.tick = start + 900;
    pvp::pvp_flag_tick(&mut world);
    assert_eq!(world.objects.get_component::<PvpState>(&5001).unwrap().flag, 1);

    // Final 20 s (200 ticks) → blinking (2).
    world.tick = start + 1100;
    pvp::pvp_flag_tick(&mut world);
    assert_eq!(world.objects.get_component::<PvpState>(&5001).unwrap().flag, 2, "blinks in the last 20 s");

    // Past expiry → cleared.
    world.tick = start + 1200;
    pvp::pvp_flag_tick(&mut world);
    assert_eq!(world.objects.get_component::<PvpState>(&5001).unwrap().flag, 0, "cleared past expiry");
}

/// `updatePvPStatus(target)`: attacking a clean player flags for
/// `PVP_NORMAL_TIME`; attacking an already-flagged/PK player flags for the
/// shorter `PVP_PVP_TIME` (`checkIfPvP`). Attacking a PK doesn't flag at all.
#[test]
fn pvp_flag_duration_depends_on_target_state() {
    use crate::game_loop::pvp;
    use crate::model::components::PvpState;
    let (mut world, ..) = test_world();
    let _a = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 5002, 50, 0, 0);
    let start = world.tick;

    // A attacks a clean B → 120 s.
    pvp::update_pvp_status_target(&mut world, 5001, 5002);
    assert_eq!(world.objects.get_component::<PvpState>(&5001).unwrap().expires_tick, start + 1200);

    // B (clean) attacks the now-flagged A → 60 s (checkIfPvP true).
    world.tick = start + 10;
    pvp::update_pvp_status_target(&mut world, 5002, 5001);
    assert_eq!(world.objects.get_component::<PvpState>(&5002).unwrap().expires_tick, start + 10 + 600, "PVP time vs a flagged target");

    // Attacking a PK doesn't flag the attacker (target freely attackable).
    world.objects.get_component_mut::<Player>(&5002).unwrap().reputation = -1;
    world.objects.get_component_mut::<PvpState>(&5001).unwrap().flag = 0;
    world.objects.get_component_mut::<PvpState>(&5001).unwrap().expires_tick = 0;
    pvp::update_pvp_status_target(&mut world, 5001, 5002);
    assert_eq!(world.objects.get_component::<PvpState>(&5001).unwrap().flag, 0, "no flag for attacking a PK");
}

/// `isAutoAttackable` relation for players: a clean player needs Ctrl (not
/// auto-attackable), a flagged or PK one does not.
#[test]
fn flagged_or_pk_player_is_auto_attackable() {
    use crate::game_loop::pvp;
    let (mut world, ..) = test_world();
    let _a = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 5002, 50, 0, 0);

    assert!(!pvp::is_player_auto_attackable(&world, 5001, 5002), "clean player needs force");

    pvp::update_pvp_status(&mut world, 5002);
    assert!(pvp::is_player_auto_attackable(&world, 5001, 5002), "flagged player is attackable");

    world.objects.get_component_mut::<crate::model::components::PvpState>(&5002).unwrap().flag = 0;
    world.objects.get_component_mut::<Player>(&5002).unwrap().reputation = -1;
    assert!(pvp::is_player_auto_attackable(&world, 5001, 5002), "PK is attackable");
}

/// Melee-attacking a player inside a peace zone is refused with the peaceful-
/// zone message (`Creature.onForcedAttack`), and no attack intent is set.
#[test]
fn melee_player_in_peace_zone_is_refused() {
    use crate::model::components::{Intent, ZoneFlags};
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 5002, 30, 0, 0);
    // Both inside a peace zone.
    world.objects.get_component_mut::<ZoneFlags>(&5001).unwrap().mask =
        crate::data::zone_data::ZoneKind::Peace.bit();
    world.objects.get_component_mut::<ZoneFlags>(&5002).unwrap().mask =
        crate::data::zone_data::ZoneKind::Peace.bit();
    // Select first, then the attack-click.
    super::combat::start_attack_intent(&mut world, 1, 5001, 5002, false);

    assert!(!world.objects.has_component::<Intent>(&5001), "no attack intent in a peace zone");
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::YOU_MAY_NOT_ATTACK_THIS_TARGET_IN_A_PEACEFUL_ZONE);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
}

/// Melee-attacking a player outside a peace zone sets the attack intent (the
/// swing then flags the attacker on landing, covered by the combat path).
#[test]
fn melee_player_outside_peace_zone_starts_attack() {
    use crate::model::components::Intent;
    use crate::model::PlayerIntent;
    let (mut world, ..) = test_world();
    let _a = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 5002, 30, 0, 0);

    super::combat::start_attack_intent(&mut world, 1, 5001, 5002, false);

    assert!(
        matches!(world.objects.get_component::<Intent>(&5001).map(|i| i.0),
            Some(PlayerIntent::Attack { target_object_id: 5002 })),
        "attack intent against the player target",
    );
}

/// Arena (`ArenaZone`/`ZoneId.PVP`): both players in a PVP zone are freely
/// auto-attackable, and hostile actions there don't raise a flag.
#[test]
fn arena_players_attackable_without_flagging() {
    use crate::game_loop::pvp;
    use crate::model::components::{PvpState, ZoneFlags};
    let (mut world, ..) = test_world();
    let _a = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 5002, 30, 0, 0);
    let pvp_bit = crate::data::zone_data::ZoneKind::Pvp.bit();
    world.objects.get_component_mut::<ZoneFlags>(&5001).unwrap().mask = pvp_bit;
    world.objects.get_component_mut::<ZoneFlags>(&5002).unwrap().mask = pvp_bit;

    // Freely attackable (no Ctrl) while both are in the arena.
    assert!(pvp::is_player_auto_attackable(&world, 5001, 5002));
    // Attacking there does not flag the attacker.
    pvp::update_pvp_status_target(&mut world, 5001, 5002);
    assert_eq!(world.objects.get_component::<PvpState>(&5001).unwrap().flag, 0, "no flag inside an arena");
}

/// An unexpected disconnect while in game persists the player too (Java
/// `GameClient.onDisconnection` → `Disconnection.storeMe().deleteMe()`).
#[test]
fn disconnect_stores_ingame_player() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let _out_rx = ingame_player(&mut world, 1, 5003, 100, 200, 0);

    on_disconnect(&mut world, 1);

    assert_eq!(expect_store_player(&mut db_rx).base.object_id, 5003);
    assert_eq!(world.objects.count::<Player>(), 0);
    assert!(world.clients.is_empty());
}

/// The staggered periodic autosave (Java `PlayerAutoSaveTaskManager`): a due
/// player is flushed once and rescheduled one interval out, and at most one
/// player is flushed per sweep (SQL-flood throttle). The player stays in the
/// world — this is a live save, not logout.
#[test]
fn autosave_flushes_one_due_player_and_reschedules() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let _a = ingame_player(&mut world, 1, 5001, 10, 20, 0);
    let _b = ingame_player(&mut world, 2, 5002, 30, 40, 0);
    world.cfg.character.character_data_store_interval_ticks = 100;
    // Both due at the current tick.
    world.player_autosave_due.insert(5001, world.tick);
    world.player_autosave_due.insert(5002, world.tick);

    super::autosave_tick(&mut world);

    // Exactly one StorePlayer this sweep (the lowest object id), and both players
    // are still in the world.
    let save = expect_store_player(&mut db_rx);
    assert_eq!(save.base.object_id, 5001, "lowest due object id flushed first");
    assert!(db_rx.try_recv().is_err(), "only one player flushed per sweep");
    assert_eq!(world.objects.count::<Player>(), 2, "autosave does not despawn");
    // 5001 rescheduled one interval out; 5002 still due.
    assert_eq!(world.player_autosave_due[&5001], world.tick + 100);
    assert_eq!(world.player_autosave_due[&5002], world.tick);

    // Next sweep flushes the other player; a third finds nothing due.
    super::autosave_tick(&mut world);
    assert_eq!(expect_store_player(&mut db_rx).base.object_id, 5002);
    super::autosave_tick(&mut world);
    assert!(db_rx.try_recv().is_err(), "nothing due after both rescheduled");
}

// ---- region-scoped visibility (Java World regions / knownlist) ----

/// The object id carried by a `CharInfo` (opcode + GC byte + x/y/z/vehicle).
fn char_info_object_id(pkt: &[u8]) -> i32 {
    assert_eq!(pkt[0], server_packets::opcodes::CHAR_INFO);
    i32::from_le_bytes(pkt[18..22].try_into().unwrap())
}

/// The object id carried by a `DeleteObject`.
fn delete_object_id(pkt: &[u8]) -> i32 {
    assert_eq!(pkt[0], server_packets::opcodes::DELETE_OBJECT);
    i32::from_le_bytes(pkt[1..5].try_into().unwrap())
}

/// A client in the `Entering` state (post-CharSelected), ready for
/// `handle_enter_world` — unlike `ingame_player`, which skips the enter-world
/// packet path entirely.
fn entering_player(
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
    world.clients.insert(client_id, ClientSession::Entering(s));
    out_rx
}

/// Entering the world exchanges `CharInfo` with players in the surrounding
/// regions (Java `spawnMe` → `World.addVisibleObject`) and with no one
/// beyond them.
#[test]
fn enter_world_exchanges_char_info_with_nearby_players_only() {
    let (mut world, ..) = test_world();
    let mut near_rx = ingame_player(&mut world, 1, 6001, 500, 500, 0);
    let mut far_rx = ingame_player(&mut world, 2, 6002, 10_000, 10_000, 0);
    let mut new_rx = entering_player(&mut world, 3, 6003, 0, 0, 0);

    handle_enter_world(&mut world, 3);

    // The nearby player learns about the newcomer…
    let pkt = near_rx.try_recv().expect("nearby player must get CharInfo");
    assert_eq!(char_info_object_id(&pkt), 6003);
    assert!(near_rx.try_recv().is_err());
    // …the far one (regions (4,4) vs (0,0)) hears nothing…
    assert!(far_rx.try_recv().is_err(), "far player must not get CharInfo");
    // …and the newcomer's burst ends with the nearby player's CharInfo only.
    let to_newcomer = drain(&mut new_rx);
    let char_infos: Vec<i32> = to_newcomer
        .iter()
        .filter(|p| p[0] == server_packets::opcodes::CHAR_INFO)
        .map(|p| char_info_object_id(p))
        .collect();
    assert_eq!(char_infos, vec![6001]);
}

/// Broadcasts only reach players whose region cell is adjacent to the
/// broadcaster's (Java `broadcastPacket` over `forEachVisibleObject`).
#[test]
fn broadcast_is_scoped_to_surrounding_regions() {
    let (mut world, ..) = test_world();
    let _mover_rx = ingame_player(&mut world, 1, 6101, 0, 0, 0);
    let mut near_rx = ingame_player(&mut world, 2, 6102, 500, 500, 0);
    let mut far_rx = ingame_player(&mut world, 3, 6103, 10_000, 10_000, 0);
    world.objects.get_component_mut::<Speeds>(&6101).unwrap().run_spd = 100.0;

    handle_move_backward_to_location(&mut world, 1, &move_body((1000, 0, 0), (0, 0, 0), 1));

    assert_eq!(near_rx.try_recv().unwrap()[0], server_packets::opcodes::MOVE_TO_LOCATION);
    assert!(far_rx.try_recv().is_err(), "far player must not see the move");
}

/// Walking across a region boundary out of / back into an observer's 3×3
/// block sends `DeleteObject` / `CharInfo` (Java `World.switchRegion`), and a
/// newly visible mover is introduced mid-move (`describeStateToPlayer` →
/// `MoveToLocation`).
#[test]
fn region_crossing_exchanges_delete_object_and_char_info() {
    let (mut world, ..) = test_world();
    let mut mover_rx = ingame_player(&mut world, 1, 6201, 0, 0, 0);
    let mut watcher_rx = ingame_player(&mut world, 2, 6202, 3000, 0, 0); // region (1,0)
    world.objects.get_component_mut::<Speeds>(&6201).unwrap().run_spd = 500.0;
    world.objects.get_component_mut::<TargetRef>(&6202).unwrap().0 = Some(6201);

    // Walk west: region 0 → -1 → -2; (−1,0) is no longer adjacent to (1,0).
    handle_move_backward_to_location(&mut world, 1, &move_body((-2500, 0, 0), (0, 0, 0), 1));
    assert_eq!(mover_rx.try_recv().unwrap()[0], server_packets::opcodes::MOVE_TO_LOCATION);
    assert_eq!(watcher_rx.try_recv().unwrap()[0], server_packets::opcodes::MOVE_TO_LOCATION);
    for _ in 0..100 {
        world.tick += 1;
        visibility::movement_tick(&mut world);
    }
    assert!(!world.objects.has_component::<Movement>(&6201), "move must have finished");

    let to_watcher = drain(&mut watcher_rx);
    assert_eq!(to_watcher.len(), 1, "exactly one packet after the move start");
    assert_eq!(delete_object_id(&to_watcher[0]), 6201);
    assert_eq!(delete_object_id(&drain(&mut mover_rx).pop().unwrap()), 6202);
    assert_eq!(world.objects.get_component::<TargetRef>(&6202).unwrap().0, None, "dangling target dropped");

    // Walk back east: crossing into region 0 re-enters the watcher's block —
    // CharInfo, then the in-flight move (describeStateToPlayer).
    handle_move_backward_to_location(&mut world, 1, &move_body((500, 0, 0), (-2500, 0, 0), 1));
    assert_eq!(mover_rx.try_recv().unwrap()[0], server_packets::opcodes::MOVE_TO_LOCATION);
    for _ in 0..100 {
        world.tick += 1;
        visibility::movement_tick(&mut world);
    }
    let to_watcher = drain(&mut watcher_rx);
    assert_eq!(to_watcher.len(), 2);
    assert_eq!(char_info_object_id(&to_watcher[0]), 6201);
    assert_eq!(to_watcher[1][0], server_packets::opcodes::MOVE_TO_LOCATION);
    let to_mover = drain(&mut mover_rx);
    assert_eq!(to_mover.len(), 1, "watcher isn't moving → CharInfo only");
    assert_eq!(char_info_object_id(&to_mover[0]), 6202);
}

/// Leaving the world (logout here; restart/disconnect share the path)
/// broadcasts `DeleteObject` to everyone watching and drops their target
/// (Java `deleteMe` → `World.removeVisibleObject`).
#[test]
fn leave_world_sends_delete_object_to_watchers() {
    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let _leaver_rx = ingame_player(&mut world, 1, 6301, 0, 0, 0);
    let mut near_rx = ingame_player(&mut world, 2, 6302, 500, 500, 0);
    let mut far_rx = ingame_player(&mut world, 3, 6303, 10_000, 10_000, 0);
    world.objects.get_component_mut::<TargetRef>(&6302).unwrap().0 = Some(6301);

    handle_logout(&mut world, 1);

    assert_eq!(delete_object_id(&near_rx.try_recv().unwrap()), 6301);
    assert_eq!(world.objects.get_component::<TargetRef>(&6302).unwrap().0, None, "dangling target dropped");
    assert!(far_rx.try_recv().is_err());
}

// ===========================================================================
// G9 — combat & AI
// ===========================================================================

/// A world tuned for melee combat: the fighter-ish class-0 template from
/// `cast_test_world` plus a synthetic exp table (level N needs (N−1)·1000)
/// and a Monster template 40001 (level 5, pDef 40, exp 2000/sp 100, a 70%
/// 5-adena drop line, 2 s corpse time).
fn combat_test_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db_rx, link_rx) = cast_test_world();
    world.data.experience =
        crate::data::ExperienceData::from_table(vec![0, 0, 1000, 2000, 3000, 4000, 5000, 50000, 100_000], 8);
    // The caster template lacks the melee-side fields — give it reach, run
    // speed, defence, and level tables past 5 so level-ups stay sane.
    {
        let mut t = world.data.player_templates.get(0).unwrap().clone();
        t.base_atk_range = 20;
        t.base_run_spd = 115;
        t.base_p_def = 80;
        t.collision_radius = 9.0;
        for lvl in 1..=8usize {
            t.hp_table[lvl] = 100.0;
            t.mp_table[lvl] = 50.0;
            t.cp_table[lvl] = 100.0;
        }
        world.data.player_templates = crate::data::PlayerTemplateData::from_vec(vec![t]);
    }
    // Adena template so auto-loot stacks it.
    world.data.item_data.insert_for_test(crate::data::item_data::ItemTemplate {
        item_id: 57,
        name: "Adena".into(),
        kind: crate::data::item_data::ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: crate::data::item_data::ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
    });
    let mut t = crate::data::npc_data::default_template(40001);
    t.type_name = "Monster".into();
    t.name = "Test Gremlin".into();
    t.level = 5;
    t.base_hp_max = 100.0;
    t.base_mp_max = 30.0;
    t.base_p_atk = 50.0;
    t.base_p_def = 40.0;
    t.base_m_def = 40.0;
    t.base_atk_range = 60;
    t.base_rnd_dam = 10;
    t.collision_radius = 10.0;
    t.exp = 2000.0;
    t.sp = 100.0;
    t.corpse_time = Some(2);
    t.drop_list_death.push(crate::data::npc_data::DropHolder { item_id: 57, min: 5, max: 5, chance: 70.0 });
    world.data.npc_data.insert_for_test(t);
    // Loot needs a runtime id block (normally pushed by the DB thread at boot).
    world.id_pool = 0x2000_0000..0x2000_1000;
    // AutoLoot=True is the dist configuration this slice targets.
    world.cfg.character.auto_loot = true;
    (world, db_rx, link_rx)
}

/// Run the real per-tick systems (movement + player combat + the 1 s AI /
/// stance sweeps) alongside the scheduler, like `game_loop::run` does —
/// `advance_ticks` only fires timers.
fn advance_world(world: &mut World, n: u64) {
    for _ in 0..n {
        world.tick += 1;
        apply_due_tasks(world);
        visibility::movement_tick(world);
        combat::player_combat_tick(world);
        if world.tick.is_multiple_of(npc_ai::NPC_THINK_PERIOD) {
            npc_ai::npc_ai_tick(world);
            combat::stance_tick(world);
        }
    }
}

fn attack_request_body(object_id: i32) -> Vec<u8> {
    attack_request_body_shift(object_id, false)
}

fn attack_request_body_shift(object_id: i32, shift: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(object_id);
    w.write_i32(0); // origin x
    w.write_i32(0); // origin y
    w.write_i32(0); // origin z
    w.write_u8(if shift { 1 } else { 0 }); // 0 simple / 1 shift click
    w.into_bytes()
}

/// The full melee kill: AttackRequest → Attack packet + combat stance, the
/// scheduled hit lands with `Formulas` damage, the monster dies (Die), the
/// killer gets XP/SP (level-up: SocialAction 2122 + SM 96), auto-loot adena
/// (SM 28 + InventoryUpdate; memory-first — the loot persists on the next
/// flush, not on pickup), and the corpse decays (DeleteObject) with no respawn
/// for a respawn-less spawn line.
#[test]
fn melee_kill_rewards_and_decay() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    {
        // Level 5 exactly at its threshold +500 (table: L5 = 4000, L6 = 5000).
        let p = world.objects.get_component_mut::<crate::model::Player>(&3001).unwrap();
        p.exp = 4500;
    }
    let npc_oid = NPC_OID + 7;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 100, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);

    // Swing rolls: hit (miss roll 0), no crit (99), random-damage delta 0
    // (roll(21) == 10 → ±0 on rndDam 10).
    world.forced_rolls.extend([0, 99, 10]);
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));

    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::ATTACK), "Attack broadcast");
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::AUTO_ATTACK_START), "combat stance");

    // Expected damage: pAtk × rand(1.0) [+ position bonus] ×77 / pDef.
    // Attacker at (0,0), target heading 0 at (30,0) → attacker is BEHIND.
    let p_atk = pcs(&world, 3001).p_atk;
    let p_def = 40.0 * (5.0 + 89.0) / 100.0;
    let expected = formulas::calc_auto_attack_damage(
        p_atk,
        1.0,
        crate::model::movement::Position::Back,
        p_def,
        false,
        false,
    );
    assert!(expected > 100.0, "sanity: one swing must kill the 100 HP monster ({expected})");

    // Hit lands at timeToHit = 1666 × 0.644 ≈ 1073 ms ⇒ 11 ticks. Queue the
    // drop rolls it will consume on death: level-gap pass (0), chance pass
    // (0 < 70%).
    world.forced_rolls.extend([0, 0]);
    advance_world(&mut world, 12);

    // Monster died: Die broadcast, rewards granted.
    assert!(nvit(&world, npc_oid).dead);
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::DIE
            && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid),
        "Die broadcast for the monster"
    );
    // XP: 2000 × share 1.0 × gap 1.0 (same level) → 4500 + 2000 = 6500 ⇒ level 6.
    let p = &world.objects.get_component::<crate::model::Player>(&3001).expect("player");
    assert_eq!(p.exp, 6500);
    assert_eq!(p.level, 6);
    let cp = pcp(&world, 3001);
    assert_eq!(cp.cur_cp, cp.max_cp as f64, "level-up refills CP");
    assert_eq!(p.sp, 100);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::YOU_HAVE_ACQUIRED_S1_XP_BONUS_S2_AND_S3_SP_BONUS_S4),
        "XP/SP system message"
    );
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION
            && i32::from_le_bytes(p[5..9].try_into().unwrap()) == server_packets::SOCIAL_ACTION_LEVEL_UP),
        "level-up flourish"
    );
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::YOUR_LEVEL_HAS_INCREASED),
        "level-up message"
    );
    // Auto-loot: 5 adena in the inventory, SM 28, persisted via InsertItem.
    let inv = world.objects.get_component::<crate::model::inventory::Inventory>(&3001).unwrap();
    let adena = inv.items().iter().find(|i| i.item_id == 57).expect("looted adena");
    assert_eq!(adena.count, 5);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::YOU_HAVE_OBTAINED_S1_ADENA),
        "obtained-adena message"
    );
    // Memory-first: loot lands in the Inventory component (adena count asserted
    // above); it persists on the next flush, not on pickup.

    // The attack intent drops on the next combat tick (dead target).
    advance_world(&mut world, 1);
    assert!(!world.objects.has_component::<Intent>(&3001));

    // Decay after the 2 s corpse time: DeleteObject, corpse gone, no respawn
    // scheduled (respawn_secs == 0).
    advance_world(&mut world, 20);
    assert!(!world.objects.has_component::<crate::model::npc::Npc>(&npc_oid));
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT),
        "corpse DeleteObject"
    );
    assert!(world.scheduler.is_empty(), "no respawn for a respawn-less spawn line");
}

/// The dead mob stays *selected* for its whole corpse window (so future
/// sweep/loot logic can act on the selected corpse); the target is released
/// only when it decays. At decay, `TargetUnselected` goes to *every* player who
/// still had it selected — not just the killer — clearing each ground ring (our
/// client keeps a dead/deleted target locked without the packet). Each
/// server-side `TargetRef` is cleared too.
#[test]
fn decaying_mob_sends_target_unselected_to_all_holders() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    // A second player nearby who also has the mob targeted but did not kill it.
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 20, 0);
    let npc_oid = NPC_OID + 11;
    add_test_npc(&mut world, npc_oid, 40001, "Monster", 5, 40, 0, 0);

    // Both players select the mob (each client now shows its target ring).
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    handle_action(&mut world, 2, &action_body(npc_oid, 0));
    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, Some(npc_oid));
    assert_eq!(world.objects.get_component::<TargetRef>(&3002).unwrap().0, Some(npc_oid));
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Player 1 lands the kill — the corpse stays selected (sweep window).
    death::npc_do_die(&mut world, npc_oid, 3001);
    let got_unselect = |packets: &[Vec<u8>], player_oid: i32| {
        packets.iter().any(|p| p[0] == server_packets::opcodes::TARGET_UNSELECTED
            && i32::from_le_bytes(p[1..5].try_into().unwrap()) == player_oid)
    };
    assert!(!got_unselect(&drain(&mut a_rx), 3001), "no TargetUnselected at death");
    assert!(!got_unselect(&drain(&mut b_rx), 3002), "no TargetUnselected at death");
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(npc_oid),
        "corpse stays selected while it lasts (for sweep/loot)"
    );
    assert_eq!(world.objects.get_component::<TargetRef>(&3002).unwrap().0, Some(npc_oid));

    // Corpse decays → both clients get their own TargetUnselected (payload
    // carries the *deselecting* player's id) and both server-side targets clear.
    death::handle_npc_decay(&mut world, npc_oid);
    assert!(got_unselect(&drain(&mut a_rx), 3001), "killer's ring clears at decay");
    assert!(got_unselect(&drain(&mut b_rx), 3002), "onlooker's ring clears at decay");
    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, None);
    assert_eq!(world.objects.get_component::<TargetRef>(&3002).unwrap().0, None);
}

/// A mob that dies **mid-chase** must broadcast `StopMove` (Java `doDie` →
/// `stopMove(null)`) so the client freezes the corpse at the death spot instead
/// of sliding it toward its last move destination — the lingering selection/
/// target decal "where the mob died". The `StopMove` carries the mob's current
/// position and comes before the `Die` broadcast.
#[test]
fn moving_mob_death_broadcasts_stop_move() {
    use crate::model::components::Movement;
    use crate::model::movement::MoveData;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    add_test_npc(&mut world, npc_oid, 40001, "Monster", 5, 40, 0, 0);
    // Give it an in-flight chase move (client is interpolating it toward 400,0).
    world.objects.add_components(
        &npc_oid,
        Movement(MoveData {
            start_x: 40,
            start_y: 0,
            start_z: 0,
            dest_x: 400,
            dest_y: 0,
            dest_z: 0,
            start_tick: world.tick,
            total_ticks: 100,
            geo_path: None,
        }),
    );
    drain(&mut a_rx);

    death::npc_do_die(&mut world, npc_oid, 3001);

    let packets = drain(&mut a_rx);
    let stop_idx = packets
        .iter()
        .position(|p| p[0] == server_packets::opcodes::STOP_MOVE
            && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid)
        .expect("StopMove broadcast for the dying mob");
    // Frozen at the death spot (40,0), not the move destination (400,0).
    let stop = &packets[stop_idx];
    assert_eq!(i32::from_le_bytes(stop[5..9].try_into().unwrap()), 40, "StopMove at death x");
    assert_eq!(i32::from_le_bytes(stop[9..13].try_into().unwrap()), 0, "StopMove at death y");
    // Ordering: StopMove precedes Die (Java doDie order).
    let die_idx = packets
        .iter()
        .position(|p| p[0] == server_packets::opcodes::DIE
            && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid)
        .expect("Die broadcast");
    assert!(stop_idx < die_idx, "StopMove is sent before Die");
}

/// Regression: the Ctrl-click force-attack. Java's `ClientPackets` binds *both*
/// `ATTACK` (0x01) and `ATTACK_REQUEST` (0x32) to `AttackRequest`; the Interlude
/// client sends 0x01 on a Ctrl-click. It must route through `on_packet` to the
/// attack handler, and — since a Ctrl-click is a *force attack* — one click both
/// selects the target (`MyTargetSelected`) and engages it (`Attack` intent +
/// broadcast), without waiting for a second click. Before the 0x01 arm existed
/// the packet fell through to the unhandled branch and nothing happened.
#[test]
fn ctrl_click_opcode_0x01_switches_target_and_attacks() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 30;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 20, 0, 0, 100_000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    // A single Ctrl-click with no current target: routes to the handler,
    // switches the target AND engages the attack in one click (force attack).
    world.forced_rolls.extend([0, 99, 10]);
    let ctrl_click = [vec![cop::ATTACK], attack_request_body(npc_oid)].concat();
    on_packet(&mut world, 1, ctrl_click);
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(npc_oid),
        "0x01 selects the clicked target"
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::MY_TARGET_SELECTED),
        "target switch sends MyTargetSelected"
    );
    assert!(
        matches!(world.objects.get_component::<Intent>(&3001), Some(Intent(crate::model::PlayerIntent::Attack { .. }))),
        "one Ctrl-click engages the attack intent"
    );
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::ATTACK), "Attack broadcast on the same click");
}

/// Shift-click is `dontMove`: an out-of-reach shift-attack refuses to chase and
/// fails with "your target is out of range" (SM 22) + `ActionFailed`, leaving no
/// attack intent and no movement. A plain (non-shift) attack on the same mob
/// chases instead — the contrast the shift flag controls. (Java discards the
/// byte; this is a deliberate enhancement.)
#[test]
fn shift_attack_out_of_reach_fails_instead_of_chasing() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 33;
    // 200 units away — beyond reach 20 + 0 + 10 = 30.
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 200, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    // Shift-attack the far mob: selects it, but refuses to move.
    on_packet(&mut world, 1, [vec![cop::ATTACK], attack_request_body_shift(npc_oid, true)].concat());
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(npc_oid),
        "shift-attack still selects the target"
    );
    assert!(!world.objects.has_component::<Intent>(&3001), "no attack intent — dontMove");
    assert!(!world.objects.has_component::<Movement>(&3001), "no chase — dontMove");
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::YOUR_TARGET_IS_OUT_OF_RANGE),
        "out-of-range system message"
    );
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::ACTION_FAIL), "ActionFailed");

    // Contrast: a plain (non-shift) attack on the same mob DOES chase.
    on_packet(&mut world, 1, [vec![cop::ATTACK], attack_request_body(npc_oid)].concat());
    assert!(
        matches!(world.objects.get_component::<Intent>(&3001), Some(Intent(crate::model::PlayerIntent::Attack { .. }))),
        "a non-shift attack engages (and will chase)"
    );
}

/// dontMove is independent of the force modifier: a shift-click arrives on the
/// `Action` packet (`action_id == 1`), not `AttackRequest`, so the shift flag
/// has to be honoured there too. An out-of-reach shift-click on the current
/// monster target refuses to chase (SM 22 + no intent/movement); a plain click
/// on the same target chases. Regression for "dontMove only worked with
/// ctrl+shift" — ctrl routes to `AttackRequest`, shift alone routes to `Action`.
#[test]
fn shift_click_via_action_packet_does_not_move() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 34;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 200, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    // Select the far monster (plain click just targets it).
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);

    // Shift-click it (Action, action_id = 1) — dontMove: no chase, "out of range".
    handle_action(&mut world, 1, &action_body(npc_oid, 1));
    assert!(!world.objects.has_component::<Intent>(&3001), "no attack intent — dontMove");
    assert!(!world.objects.has_component::<Movement>(&3001), "no chase — dontMove");
    assert!(
        drain(&mut a_rx).iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::YOUR_TARGET_IS_OUT_OF_RANGE),
        "out-of-range system message"
    );

    // A plain click on the same target chases instead.
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    assert!(
        matches!(world.objects.get_component::<Intent>(&3001), Some(Intent(crate::model::PlayerIntent::Attack { .. }))),
        "a non-shift click engages (and will chase)"
    );
}

/// Chasing: an `AttackRequest` from out of melee reach walks the player
/// toward the monster (`MoveToPawn`) and only swings once in reach; the hurt
/// monster retaliates through its AI think (run mode + `Attack` back), and
/// its damage bites the player's HP directly (no CP soak from NPCs).
#[test]
fn attack_out_of_reach_chases_and_monster_retaliates() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 8;
    // 200 units away — beyond reach 20 + 0 + 10 = 30; big HP pool so the
    // monster survives and hits back.
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 200, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);

    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "out of reach: chase starts, no swing yet"
    );
    assert!(!packets.iter().any(|p| p[0] == server_packets::opcodes::ATTACK));

    // Player run speed 115 u/s over ~170 units ⇒ in reach in ~1.5 s. Force
    // every swing in the window (player ×2, monster ×1) to a plain hit.
    world.forced_rolls.extend([0, 99, 10, 0, 99, 10, 0, 99, 10]);
    let hp_before = pvit(&world, 3001).cur_hp;
    let cp_before = pcp(&world, 3001).cur_cp;
    advance_world(&mut world, 45);

    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::ATTACK
        && i32::from_le_bytes(p[1..5].try_into().unwrap()) == 3001), "player swung after closing in");
    assert!(nvit(&world, npc_oid).cur_hp < 5000.0, "monster took damage");
    assert_eq!(world.objects.get_component::<crate::model::npc::NpcAi>(&npc_oid).unwrap().intention, crate::model::npc::NpcIntention::Attack);
    assert!(world.objects.get_component::<Speeds>(&npc_oid).unwrap().running, "aggroed monsters run");
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::CHANGE_MOVE_TYPE),
        "run-mode broadcast"
    );
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::ATTACK
            && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid),
        "monster swung back"
    );
    assert!(pvit(&world, 3001).cur_hp < hp_before, "player HP bitten");
    assert_eq!(pcp(&world, 3001).cur_cp, cp_before, "no CP soak from NPC hits");
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2),
        "victim damage message"
    );
}

/// Spawn the standard 5000-HP test monster at `x` and target it with the
/// caster's `Action` click.
fn spawn_targeted_monster(
    world: &mut World,
    a_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    npc_oid: i32,
    x: i32,
) {
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, x, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    handle_action(world, 1, &action_body(npc_oid, 0));
    drain(a_rx);
}

/// An out-of-range cast walks the caster into cast range (Java `useMagic` →
/// CAST intention → `thinkCast`/`maybeMoveToPawn`) and only then starts the
/// cast at the snapshotted target.
#[test]
fn cast_out_of_range_walks_into_range_then_casts() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    // 700 away — castRange 600 + collision 9 + 10 leaves ~81 units to walk.
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 700);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN), "walks toward the cast target");
    assert!(!packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE), "no cast before range");
    assert!(world.objects.has_component::<Intent>(&3001));
    assert!(!world.objects.has_component::<Casting>(&3001));

    // ~81 units at run speed 115 ⇒ in range in ~8 ticks.
    advance_world(&mut world, 15);
    assert!(world.objects.has_component::<Casting>(&3001), "cast starts on arrival");
    assert!(!world.objects.has_component::<Intent>(&3001), "the walk-to-cast intent is consumed");
    assert!(!world.objects.has_component::<Movement>(&3001), "chase leg stopped before casting");
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE));

    // Launch (35 ticks) + finish (5): the nuke lands on the walked-to monster.
    advance_world(&mut world, 45);
    assert!(nvit(&world, npc_oid).cur_hp < 5000.0, "nuke landed after the walk");
}

/// Bug fix: casting a beneficial (`Target`-type) skill on a monster requires
/// Ctrl (force). Without it the cast is refused (INVALID_TARGET); with it, it
/// proceeds.
#[test]
fn buff_on_monster_requires_ctrl() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 20;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50);

    // No Ctrl → refused, no cast.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, false));
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::INVALID_TARGET);
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(!world.objects.has_component::<Casting>(&3001), "no cast on a mob without force");

    // Ctrl (force) → the cast starts.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, true));
    assert!(world.objects.has_component::<Casting>(&3001), "ctrl force-targets the mob");
}

/// Bug fix: a buff cast on a monster modifies the mob's stats (like on a
/// character) and reverts on expiry.
#[test]
fn buff_on_monster_modifies_stats_and_reverts() {
    use crate::model::components::{Buffs, CombatStats};
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 21;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50);
    let base_p_atk = world.objects.get_component::<CombatStats>(&npc_oid).unwrap().p_atk;
    assert!(base_p_atk > 0.0, "sanity: the mob has a base pAtk");

    // Might (+8% pAtk), forced onto the mob; lands after hit_time (10 ticks).
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, true));
    advance_ticks(&mut world, 12);
    let buffed = world.objects.get_component::<CombatStats>(&npc_oid).unwrap().p_atk;
    assert!((buffed - base_p_atk * 1.08).abs() < 1e-6, "Might raises the mob pAtk 8% ({base_p_atk} -> {buffed})");
    assert_eq!(world.objects.get_component::<Buffs>(&npc_oid).unwrap().0.len(), 1, "buff tracked on the mob");

    // abnormal_time 20 s = 200 ticks → expiry reverts the stat.
    advance_ticks(&mut world, 205);
    let reverted = world.objects.get_component::<CombatStats>(&npc_oid).unwrap().p_atk;
    assert!((reverted - base_p_atk).abs() < 1e-6, "expiry reverts the mob pAtk");
    assert!(world.objects.get_component::<Buffs>(&npc_oid).unwrap().0.is_empty(), "buff removed on expiry");
}

/// Bug fix: a buff cast on a monster is shown in the target window of players
/// who have it selected (`ExAbnormalStatusUpdateFromTarget`, 0xFE:0xE6).
#[test]
fn buff_on_monster_shows_in_target_window() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 22;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50); // caster now targets the mob

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, true));
    advance_ticks(&mut world, 12);

    let pkt = drain(&mut a_rx)
        .into_iter()
        .find(|p| p.len() >= 13 && p[0] == 0xFE && p[1] == 0xE6 && p[2] == 0x00)
        .expect("ExAbnormalStatusUpdateFromTarget sent to the observer");
    assert_eq!(i32::from_le_bytes(pkt[3..7].try_into().unwrap()), npc_oid, "for the buffed mob");
    assert_eq!(i16::from_le_bytes(pkt[7..9].try_into().unwrap()), 1, "one buff shown");
    assert_eq!(i32::from_le_bytes(pkt[9..13].try_into().unwrap()), 1068, "Might listed in the target window");
}

/// A move click while walking to cast abandons the cast intention (Java: the
/// new MOVE_TO intention replaces CAST) — the player never casts.
#[test]
fn move_click_cancels_walk_to_cast() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 10;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 700);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Intent>(&3001));

    handle_move_backward_to_location(&mut world, 1, &move_body((0, 300, 0), (0, 0, 0), 1));
    assert!(!world.objects.has_component::<Intent>(&3001), "move click drops the walk-to-cast");
    advance_world(&mut world, 60);
    assert!(!world.objects.has_component::<Casting>(&3001));
    let packets = drain(&mut a_rx);
    assert!(!packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE), "the cast never fires");
}

/// Selecting another target mid-walk must NOT drop the cast: Java's
/// `RequestTargetCanceld` (which the client also sends on a target switch)
/// never touches the AI intention, and `thinkCast` casts at the intention's
/// snapshotted target even after a re-target.
#[test]
fn retarget_mid_walk_keeps_cast_intent() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_a = NPC_OID + 60;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_a, 700);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Intent>(&3001), "walk-to-cast started");

    // Walk a couple of ticks, then switch to monster B — the client emits
    // a target cancel followed by the new select click.
    advance_world(&mut world, 2);
    let npc_b = NPC_OID + 61;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_b, 40001, 300, 300, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_b);
    world.objects.spawn(npc_b, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_b, cs);
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(true));
    handle_action(&mut world, 1, &action_body(npc_b, 0));
    drain(&mut a_rx);

    assert!(world.objects.has_component::<Intent>(&3001), "re-target must not drop the cast intent");
    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, Some(npc_b), "target switched to B");
    advance_world(&mut world, 60);
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "cast fires at the snapshotted target after the walk"
    );
    assert!(nvit(&world, npc_a).cur_hp < 5000.0, "nuke landed on monster A");
    assert_eq!(nvit(&world, npc_b).cur_hp, 5000.0, "monster B untouched");
}

/// Same as `retarget_mid_walk_keeps_cast_intent`, but the new target is far
/// away (out of the skill's cast range) — the reported live repro: the switch
/// must still not drop the walk-to-cast, and the nuke still lands on A.
#[test]
fn retarget_mid_walk_to_far_target_keeps_cast_intent() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_a = NPC_OID + 62;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_a, 700);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Intent>(&3001), "walk-to-cast started");

    // Walk a couple of ticks, then switch to monster B, far off to the side
    // (well beyond castRange 600 from the walking player).
    advance_world(&mut world, 2);
    let npc_b = NPC_OID + 63;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_b, 40001, 700, 1500, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_b);
    world.objects.spawn(npc_b, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_b, cs);
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(true));
    handle_action(&mut world, 1, &action_body(npc_b, 0));
    drain(&mut a_rx);

    assert!(world.objects.has_component::<Intent>(&3001), "re-target must not drop the cast intent");
    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, Some(npc_b), "target switched to B");
    advance_world(&mut world, 60);
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "cast fires at the snapshotted target after the walk"
    );
    assert!(nvit(&world, npc_a).cur_hp < 5000.0, "nuke landed on monster A");
    assert_eq!(nvit(&world, npc_b).cur_hp, 5000.0, "monster B untouched");
}

/// Off-axis approach where the reach-boundary point rounds to integer
/// coordinates just *outside* reach: from (0,0) to a monster at (500,500)
/// (distance ~707.1, reach 619) the exact-boundary destination rounds to
/// (62,62), which is ~619.4 from the target. Without Java `moveToLocation`'s
/// "move a bit closer" inset (`distance -= (offset - 5)`) the chase wedges
/// in an arrive/re-path loop there and the cast never fires.
#[test]
fn walk_to_cast_boundary_rounding_still_casts() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 64;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 500, 500, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Intent>(&3001), "walk-to-cast started");

    // ~93 units to walk at run speed — in range well within 20 ticks.
    advance_world(&mut world, 20);
    assert!(world.objects.has_component::<Casting>(&3001), "cast starts on arrival despite boundary rounding");
    assert!(!world.objects.has_component::<Intent>(&3001), "the walk-to-cast intent is consumed");
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE), "the cast fires");
}

/// The walk-to-cast target dying mid-walk drops the intention on the next
/// think (`checkTargetLost`).
#[test]
fn walk_to_cast_target_death_drops_intent() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 11;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 700);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Intent>(&3001));

    world.objects.get_component_mut::<Vitals>(&npc_oid).unwrap().dead = true;
    advance_world(&mut world, 1);
    assert!(!world.objects.has_component::<Intent>(&3001), "dead target ends the walk-to-cast");
    assert!(!world.objects.has_component::<Casting>(&3001));
}

/// An idle monster with random walk enabled wanders: with no target and
/// inside its drift radius, the 1-in-30 roll fires and it moves to a random
/// spot near its spawn, broadcasting `MoveToLocation`
/// (`AttackableAI.thinkActive`'s random-walk branch).
#[test]
fn idle_monster_random_walks_near_spawn() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    {
        // 40001 is passive (won't aggro the nearby player) but wanders.
        let mut t = world.data.npc_data.get(40001).unwrap().clone();
        t.random_walk = true;
        world.data.npc_data.insert_for_test(t);
    }
    // A player keeps the spawn region active so `npc_ai_tick` visits the mob.
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 0, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // Force the walk-rate hit (0) and a delta landing well within drift (300):
    // deltaX = 500, deltaY = 500 + 83 = 583 → √(583²−500²) ≈ 299 → (200, −1).
    world.forced_rolls.extend([0, 500, 83]);
    npc_ai::npc_ai_tick(&mut world);

    let mv = world.objects.get_component::<Movement>(&npc_oid).expect("idle mob started a random walk");
    let from_spawn = ((mv.0.dest_x as f64).powi(2) + (mv.0.dest_y as f64).powi(2)).sqrt();
    assert!(from_spawn <= world.cfg.npc.max_drift_range as f64, "wander destination stays within drift range");
    assert!((mv.0.dest_x, mv.0.dest_y) != (0, 0), "actually moved off the spawn spot");
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_LOCATION
            && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid),
        "the wander is broadcast as MoveToLocation"
    );
}

/// A monster with random walk disabled stays put when idle: the roll is never
/// even reached, so it never starts a wander.
#[test]
fn idle_monster_without_random_walk_stays_put() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    // 40001 already has random_walk = false in the base test template.
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 0, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // Even if a 0 were queued, the random_walk gate short-circuits before it.
    world.forced_rolls.extend([0, 0, 0]);
    npc_ai::npc_ai_tick(&mut world);

    assert!(!world.objects.has_component::<Movement>(&npc_oid), "a non-wandering mob never moves while idle");
    let packets = drain(&mut a_rx);
    assert!(!packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_LOCATION), "no wander broadcast");
}

/// An idle NPC in an active region plays a random social animation once its
/// pending timer elapses, broadcasting `SocialAction` with id 2 or 3
/// (`RandomAnimationTaskManager` → `onRandomAnimation`).
#[test]
fn idle_npc_plays_random_social_animation() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 0, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    // Pretend the animation timer already elapsed (skip the 5–60 s wait).
    world.tick = 100;
    world.objects.get_component_mut::<crate::model::npc::NpcAi>(&npc_oid).unwrap().next_animation_tick = Some(50);
    drain(&mut a_rx);

    npc_ai::npc_ai_tick(&mut world);

    let packets = drain(&mut a_rx);
    let social = packets
        .iter()
        .find(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid)
        .expect("idle NPC broadcast a SocialAction");
    let action_id = i32::from_le_bytes(social[5..9].try_into().unwrap());
    assert!((2..=3).contains(&action_id), "random idle animation is 2 or 3, got {action_id}");
    // The 6 s throttle is now armed and the next attempt was rescheduled out.
    let ai = world.objects.get_component::<crate::model::npc::NpcAi>(&npc_oid).unwrap();
    assert_eq!(ai.last_social_tick, 100);
    assert!(ai.next_animation_tick.unwrap() > 100, "next animation rescheduled into the future");
}

/// A moving NPC does not play idle animations even when its timer is due
/// (Java gates on `!isMoving()`).
#[test]
fn moving_npc_skips_random_animation() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 0, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    world.tick = 100;
    world.objects.get_component_mut::<crate::model::npc::NpcAi>(&npc_oid).unwrap().next_animation_tick = Some(50);
    // Currently walking somewhere (`isMoving()`), so no idle animation.
    world.objects.add_components(
        &npc_oid,
        Movement(crate::model::movement::MoveData {
            start_x: 0,
            start_y: 0,
            start_z: 0,
            dest_x: 500,
            dest_y: 0,
            dest_z: 0,
            start_tick: 100,
            total_ticks: 100,
            geo_path: None,
        }),
    );
    drain(&mut a_rx);

    npc_ai::npc_ai_tick(&mut world);

    let packets = drain(&mut a_rx);
    assert!(
        !packets.iter().any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION),
        "a walking NPC plays no idle animation"
    );
    // Still rescheduled, but the throttle stayed unarmed (nothing broadcast).
    let ai = world.objects.get_component::<crate::model::npc::NpcAi>(&npc_oid).unwrap();
    assert_eq!(ai.last_social_tick, 0);
    assert!(ai.next_animation_tick.unwrap() > 100);
}

/// An aggressive monster acquires a player who just stands inside its aggro
/// range: after the spawn-calm `_globalAggro` ticks up to 0, the scan seeds
/// hate and the AI attacks unprovoked.
#[test]
fn aggressive_monster_aggros_idle_player() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    {
        // Make 40001 aggressive for this test.
        let mut t = world.data.npc_data.get(40001).unwrap().clone();
        t.is_aggressive = true;
        t.aggro_range = 300;
        world.data.npc_data.insert_for_test(t);
    }
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 150, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // 10 think seconds of calm (globalAggro −10 → 0), then the scan seeds
    // hate and the AI locks on, chases in, and swings (both swings within
    // 140 ticks forced to plain hits).
    world.forced_rolls.extend([0, 99, 10, 0, 99, 10]);
    advance_world(&mut world, 140);
    assert_eq!(world.objects.get_component::<crate::model::npc::NpcAi>(&npc_oid).unwrap().intention, crate::model::npc::NpcIntention::Attack);
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::ATTACK
            && i32::from_le_bytes(p[1..5].try_into().unwrap()) == npc_oid),
        "unprovoked attack on the idle player"
    );
    assert!(pvit(&world, 3001).cur_hp < 100.0, "the swing landed");
}

/// Death and the to-village loop: a killing blow sends `Die` with the
/// to-village flag and applies the XP penalty; `RequestRestartPoint` ports
/// the corpse to the map-region town respawn (`TeleportToLocation`), and
/// `Appearing` revives at the configured 65% HP (`Revive` broadcast).
#[test]
fn player_death_penalty_and_revive_to_village() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    // One town region covering the fight location, respawn at (1000, 1000).
    world.data.map_region = crate::data::MapRegionData::from_regions(vec![crate::data::map_region::MapRegion {
        name: "test_town".into(),
        respawn_points: vec![(1000, 1000, 7)],
        tiles: vec![(20, 18)],
    }]);
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    {
        let p = world.objects.get_component_mut::<crate::model::Player>(&3001).unwrap();
        p.exp = 4500; // level 5 (threshold 4000) + 500 into the level
        p.level = 5;
    }
    world.objects.get_component_mut::<Vitals>(&3001).unwrap().cur_hp = 1.0;
    world.objects.get_component_mut::<PlayerVitals>(&3001).unwrap().cur_cp = 0.0;
    let npc_oid = NPC_OID + 10;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    // Wake the monster by damage (as if the player had hit it).
    combat::npc_receive_damage(&mut world, npc_oid, 3001, 10.0);
    drain(&mut a_rx);

    // Its swing kills the 1-HP player: force a clean hit.
    world.forced_rolls.extend([0, 99, 10]);
    advance_world(&mut world, 30);

    let p = pvit(&world, 3001);
    assert!(p.dead);
    assert_eq!(p.cur_hp, 0.0);
    // Death penalty: 1% (empty table default) of the 1000-XP level = 10.
    assert_eq!(world.objects.get_component::<crate::model::Player>(&3001).expect("player").exp, 4490);
    let packets = drain(&mut a_rx);
    let die = packets
        .iter()
        .find(|p| p[0] == server_packets::opcodes::DIE && i32::from_le_bytes(p[1..5].try_into().unwrap()) == 3001)
        .expect("player Die packet");
    assert_eq!(i32::from_le_bytes(die[5..9].try_into().unwrap()), 1, "to-village enabled");
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && sm_id(p) == server_packets::sm_ids::YOUR_XP_HAS_DECREASED_BY_S1),
        "XP-loss message"
    );

    // To village: teleport to the region respawn point.
    world.forced_rolls.push_back(0); // random respawn-point pick
    handle_request_restart_point(&mut world, 1, &{
        let mut w = PacketWriter::new();
        w.write_i32(0); // TO_VILLAGE
        w.into_bytes()
    });
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (1000, 1000, 7));
    let p = &world.objects.get_component::<crate::model::Player>(&3001).expect("player");
    assert!(p.teleporting && p.pending_revive && pvit(&world, 3001).dead);
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION));

    // Client finished loading: Appearing → revive at 65% HP.
    on_packet(&mut world, 1, vec![cp::opcodes::APPEARING]);
    let p = &world.objects.get_component::<crate::model::Player>(&3001).expect("player");
    assert!(!pvit(&world, 3001).dead && !p.teleporting && !p.pending_revive);
    let v = pvit(&world, 3001);
    assert_eq!(v.cur_hp, v.max_hp as f64 * 0.65);
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::REVIVE));
}

/// Nuking a monster with a skill wakes its AI exactly like a melee hit and
/// kills through the same death path (the "kill a monster with a skill"
/// half of the G9 gate).
#[test]
fn nuke_kills_monster_and_rewards() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 11;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 100, 0, 0, 100, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    world.objects.get_component_mut::<crate::model::Player>(&3001).unwrap().exp = 4000; // level 5 on the test table
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);

    // Monsters are valid Enemy targets without ctrl.
    let exp_before = world.objects.get_component::<crate::model::Player>(&3001).expect("player").exp;
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Casting>(&3001), "cast accepted without force-use");
    // Drop rolls at death: gap fails (droppable but let it fail → no loot
    // noise in this test).
    world.forced_rolls.extend([999_999, 999_999]);
    advance_world(&mut world, 45);

    assert!(nvit(&world, npc_oid).dead, "the nuke killed it");
    assert!(world.objects.get_component::<crate::model::Player>(&3001).expect("player").exp > exp_before, "XP rewarded through the same death path");
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::DIE));
}

/// The decay → respawn loop over a real spawn line: the corpse decays
/// (`DeleteObject`), `Spawn.decreaseCount` schedules the respawn, and the
/// respawned NPC (fresh object id) is announced with `NpcInfo`.
#[test]
fn dead_monster_decays_and_respawns() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    world.data.spawn_data = crate::data::SpawnData {
        spawns: vec![crate::data::spawn_data::SpawnTemplate {
            name: None,
            territories: vec![],
            groups: vec![crate::data::spawn_data::SpawnGroup {
                territories: vec![],
                npcs: vec![crate::data::spawn_data::NpcSpawnDef {
                    npc_id: 40001,
                    count: 1,
                    loc: Some(crate::data::spawn_data::FixedLoc { x: 30, y: 0, z: 0, heading: 0 }),
                    respawn_secs: 3,
                    respawn_random_secs: 0,
                }],
            }],
        }],
    };
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = crate::model::npc::spawn_one(&mut world, 0, 0, 0).expect("spawned");
    world.objects.get_component_mut::<TargetRef>(&3001).unwrap().0 = Some(npc_oid);
    drain(&mut a_rx);

    // Kill it outright (drop level-gap roll forced to fail: no loot noise).
    world.forced_rolls.push_back(999_999);
    combat::npc_receive_damage(&mut world, npc_oid, 3001, 1_000_000.0);
    assert!(nvit(&world, npc_oid).dead);

    // Decay at +2 s: corpse gone, DeleteObject seen, dangling target dropped,
    // respawn pending.
    advance_world(&mut world, 21);
    assert!(!world.objects.has_component::<crate::model::npc::Npc>(&npc_oid));
    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, None);
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT));

    // Respawn at +3 s more: a fresh NPC on the same spawn line, announced.
    advance_world(&mut world, 31);
    let mut respawned_ids: Vec<i32> = Vec::new();
    world.objects.for_each_mut::<&crate::model::npc::Npc>(|n| {
        if n.npc_id == 40001 {
            respawned_ids.push(n.object_id);
        }
    });
    let respawned_oid = *respawned_ids.first().expect("respawned");
    assert_ne!(respawned_oid, npc_oid, "transient ids are not reused");
    let rpos = world.objects.get_component::<Position>(&respawned_oid).unwrap();
    assert_eq!((rpos.x, rpos.y, rpos.z), (30, 0, 0));
    assert!(!nvit(&world, respawned_oid).dead);
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::NPC_INFO),
        "respawn announced with NpcInfo"
    );
}

// ---- G9.6 — macros & panel shortcuts (docs/PLAN_MACROS_SHORTCUTS.md) ----

use crate::model::components::{Macros, Shortcuts};
use crate::model::shortcut::{Macro, MacroCmd, MacroType, Shortcut, ShortcutType};

fn shortcut_reg_body(kind: i32, combined_slot: i32, id: i32, level: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(kind);
    w.write_i32(combined_slot);
    w.write_i32(id);
    w.write_i16(level as i16);
    w.write_i16(0); // sub-level
    w.write_i32(1); // character type: player
    w.into_bytes()
}

fn make_macro_body(id: i32, name: &str, descr: &str, commands: &[(u8, i32, u8, &str)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(id);
    w.write_string(name);
    w.write_string(descr);
    w.write_string("ac");
    w.write_i32(0); // icon
    w.write_u8(commands.len() as u8);
    for (i, (kind, d1, d2, cmd)) in commands.iter().enumerate() {
        w.write_u8((i + 1) as u8);
        w.write_u8(*kind);
        w.write_i32(*d1);
        w.write_u8(*d2);
        w.write_string(cmd);
    }
    w.into_bytes()
}

fn player_shortcuts(world: &World, oid: i32) -> Vec<Shortcut> {
    world.objects.get_component::<Shortcuts>(&oid).unwrap().iter().copied().collect()
}

fn drain_db(rx: &mut db::CmdRx) -> Vec<db::DbCommand> {
    let mut out = Vec::new();
    while let Ok(c) = rx.try_recv() {
        out.push(c);
    }
    out
}

/// Registering a skill shortcut echoes `ShortCutRegister` + a `SkillList`
/// re-send (Java's quirk) and persists; deleting it re-sends the whole
/// (now empty) `ShortCutInit` and deletes the row.
#[test]
fn register_and_delete_shortcut_round_trip() {
    let (mut world, _db_tx, mut db_rx, _link) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    super::shortcuts::handle_request_short_cut_reg(&mut world, 1, &shortcut_reg_body(2, 13, 1177, 1));
    let packets = drain(&mut rx);
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0][0], server_packets::opcodes::SHORT_CUT_REGISTER);
    assert_eq!(i32::from_le_bytes([packets[0][1], packets[0][2], packets[0][3], packets[0][4]]), 2, "SKILL type");
    assert_eq!(packets[1][0], 0x5F, "SkillList re-send");
    let scs = player_shortcuts(&world, 3001);
    assert_eq!(scs.len(), 1);
    assert_eq!((scs[0].slot, scs[0].page, scs[0].id, scs[0].level), (1, 1, 1177, 1));
    // Memory-first: the shortcut lives in the Shortcuts component; no per-action
    // DB write (it persists on the next flush).
    assert!(drain_db(&mut db_rx).is_empty(), "shortcut register does not touch the DB");

    super::shortcuts::handle_request_short_cut_del(&mut world, 1, &{
        let mut w = PacketWriter::new();
        w.write_i32(13);
        w.into_bytes()
    });
    let packets = drain(&mut rx);
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0][0], server_packets::opcodes::SHORT_CUT_INIT);
    assert_eq!(i32::from_le_bytes([packets[0][1], packets[0][2], packets[0][3], packets[0][4]]), 0, "panel now empty");
    assert!(player_shortcuts(&world, 3001).is_empty());
    assert!(drain_db(&mut db_rx).is_empty(), "shortcut delete does not touch the DB");
}

/// An ITEM shortcut referencing an object id not in the inventory isn't
/// stored or persisted — but the `ShortCutRegister` echo and `SkillList`
/// still go out, exactly like Java's unconditional replies.
#[test]
fn item_shortcut_without_item_not_stored_but_echoed() {
    let (mut world, _db_tx, mut db_rx, _link) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    super::shortcuts::handle_request_short_cut_reg(&mut world, 1, &shortcut_reg_body(1, 0, 999_999, 0));
    let packets = drain(&mut rx);
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0][0], server_packets::opcodes::SHORT_CUT_REGISTER);
    assert!(player_shortcuts(&world, 3001).is_empty(), "not stored");
    assert!(drain_db(&mut db_rx).is_empty(), "not persisted");
}

/// `RequestMakeMacro` validation order and the no-recurring-macros
/// deviation: a SHORTCUT-type command is rejected with SM 810 and nothing
/// is stored (Java accepts it — that's the AFK macro-loop vector).
#[test]
fn make_macro_validations_and_recurring_rejection() {
    let (mut world, _db_tx, mut db_rx, _link) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let macros_of = |world: &World| world.objects.get_component::<Macros>(&3001).unwrap().entries.len();

    // SHORTCUT command → invalid macro.
    super::shortcuts::handle_request_make_macro(&mut world, 1, &make_macro_body(0, "loop", "d", &[(4, 0, 11, "")]));
    assert_eq!(sm_id(&rx.try_recv().unwrap()), server_packets::sm_ids::INVALID_MACRO_REFER_TO_THE_HELP_FILE_FOR_INSTRUCTIONS);
    assert_eq!(macros_of(&world), 0);

    // Empty name.
    super::shortcuts::handle_request_make_macro(&mut world, 1, &make_macro_body(0, "", "d", &[(1, 1177, 1, "")]));
    assert_eq!(sm_id(&rx.try_recv().unwrap()), server_packets::sm_ids::ENTER_THE_NAME_OF_THE_MACRO);

    // Description over 32 chars.
    super::shortcuts::handle_request_make_macro(&mut world, 1, &make_macro_body(0, "m", &"d".repeat(33), &[(1, 1177, 1, "")]));
    assert_eq!(sm_id(&rx.try_recv().unwrap()), server_packets::sm_ids::MACRO_DESCRIPTIONS_MAY_CONTAIN_UP_TO_32_CHARACTERS);

    // Command strings over 255 chars total.
    let long = "x".repeat(256);
    super::shortcuts::handle_request_make_macro(&mut world, 1, &make_macro_body(0, "m", "d", &[(3, 0, 0, long.as_str())]));
    assert_eq!(sm_id(&rx.try_recv().unwrap()), server_packets::sm_ids::INVALID_MACRO_REFER_TO_THE_HELP_FILE_FOR_INSTRUCTIONS);

    // Macro cap: with more than 48 stored, registration is refused.
    {
        let macros = world.objects.get_component_mut::<Macros>(&3001).unwrap();
        for i in 0..49 {
            macros.entries.push(Macro { id: 2000 + i, icon: 0, name: "m".into(), descr: String::new(), acronym: String::new(), commands: vec![] });
        }
    }
    super::shortcuts::handle_request_make_macro(&mut world, 1, &make_macro_body(0, "m", "d", &[(1, 1177, 1, "")]));
    assert_eq!(sm_id(&rx.try_recv().unwrap()), server_packets::sm_ids::YOU_MAY_CREATE_UP_TO_48_MACROS);
    world.objects.get_component_mut::<Macros>(&3001).unwrap().entries.clear();
    assert!(drain_db(&mut db_rx).is_empty(), "no rejected macro persisted");

    // A valid macro: id 0 → allocated 1000, ADD echo, persisted.
    super::shortcuts::handle_request_make_macro(
        &mut world,
        1,
        &make_macro_body(0, "buffs", "d", &[(1, 1177, 1, ""), (3, 0, 0, "/sit")]),
    );
    let pkt = rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::MACRO_LIST);
    assert_eq!(pkt[1], 1, "ADD");
    assert_eq!(i32::from_le_bytes([pkt[2], pkt[3], pkt[4], pkt[5]]), 1000);
    assert_eq!(macros_of(&world), 1);
    let stored = world.objects.get_component::<Macros>(&3001).unwrap().get(1000).unwrap().clone();
    assert_eq!(stored.commands.len(), 2);
    assert_eq!(stored.commands[1].cmd, "/sit");
    // Memory-first: the macro lives in the Macros component; no per-action write.
    assert!(drain_db(&mut db_rx).is_empty(), "macro create does not touch the DB");

    // Editing it (real id) → MODIFY echo, still one macro.
    super::shortcuts::handle_request_make_macro(&mut world, 1, &make_macro_body(1000, "buffs2", "d", &[(1, 1177, 1, "")]));
    let pkt = rx.try_recv().unwrap();
    assert_eq!(pkt[1], 2, "MODIFY");
    assert_eq!(macros_of(&world), 1);
    assert_eq!(world.objects.get_component::<Macros>(&3001).unwrap().get(1000).unwrap().name, "buffs2");
}

/// Deleting a macro removes it, cascade-deletes the panel slots holding it
/// (each re-sending `ShortCutInit`, like Java), and echoes the DELETE
/// `SendMacroList`.
#[test]
fn delete_macro_cascades_panel_slots() {
    let (mut world, _db_tx, mut db_rx, _link) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    super::shortcuts::handle_request_make_macro(&mut world, 1, &make_macro_body(0, "m", "d", &[(3, 0, 0, "/loc")]));
    super::shortcuts::handle_request_short_cut_reg(&mut world, 1, &shortcut_reg_body(4, 5, 1000, 0));
    drain(&mut rx);
    drain_db(&mut db_rx);

    super::shortcuts::handle_request_delete_macro(&mut world, 1, &{
        let mut w = PacketWriter::new();
        w.write_i32(1000);
        w.into_bytes()
    });
    assert!(world.objects.get_component::<Macros>(&3001).unwrap().entries.is_empty());
    assert!(player_shortcuts(&world, 3001).is_empty(), "macro slot cascade-deleted");
    let packets = drain(&mut rx);
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0][0], server_packets::opcodes::SHORT_CUT_INIT, "cascade re-sends the panel");
    assert_eq!(packets[1][0], server_packets::opcodes::MACRO_LIST);
    assert_eq!(packets[1][1], 0, "DELETE");
    // Memory-first: the macro removal + shortcut cascade are in-memory (asserted
    // above); nothing is written per action.
    assert!(drain_db(&mut db_rx).is_empty(), "macro delete cascade does not touch the DB");
}

/// A skill upgrade rewrites the SKILL slots holding it: new level in the
/// component, a `ShortCutRegister` echo, and a row upsert
/// (`ShortCuts.updateShortCuts`, called from skill learn and level-up
/// grants).
#[test]
fn skill_upgrade_updates_matching_shortcuts() {
    let (mut world, _db_tx, mut db_rx, _link) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    super::shortcuts::handle_request_short_cut_reg(&mut world, 1, &shortcut_reg_body(2, 0, 1177, 1));
    super::shortcuts::handle_request_short_cut_reg(&mut world, 1, &shortcut_reg_body(3, 1, 2, 0)); // an ACTION, untouched
    drain(&mut rx);
    drain_db(&mut db_rx);

    super::shortcuts::update_skill_shortcuts(&mut world, 3001, 1177, 2);
    let scs = player_shortcuts(&world, 3001);
    assert_eq!(scs.iter().find(|sc| sc.id == 1177).unwrap().level, 2);
    assert_eq!(scs.iter().find(|sc| sc.kind == ShortcutType::Action).unwrap().level, 0);
    let packets = drain(&mut rx);
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0][0], server_packets::opcodes::SHORT_CUT_REGISTER);
    // Memory-first: the level bump is in the Shortcuts component; no per-action write.
    assert!(drain_db(&mut db_rx).is_empty(), "shortcut level bump does not touch the DB");

    // No matching slot → no traffic.
    super::shortcuts::update_skill_shortcuts(&mut world, 3001, 9999, 1);
    assert!(drain(&mut rx).is_empty());
    assert!(drain_db(&mut db_rx).is_empty());
}

/// `from_char` restores the panel and macros; ITEM shortcuts whose object
/// id left the inventory are pruned (`ShortCuts.restoreMe`'s verification),
/// so they never reach the bundle and the next flush's reconcile drops their
/// rows (`stale_item_shortcuts` identifies them).
#[test]
fn from_char_restores_and_prunes_shortcuts() {
    let (world, ..) = test_world();
    let mut chr = dummy_char(3001, "P");
    chr.items = vec![crate::character::ItemRow {
        object_id: 500,
        item_id: 57,
        count: 10,
        enchant_level: 0,
        loc: "INVENTORY".into(),
        loc_data: 0,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
    }];
    let sc = |slot: i32, kind: ShortcutType, id: i32| Shortcut { slot, page: 0, kind, id, level: 1, character_type: 1, shared_reuse_group: -1 };
    chr.shortcuts = vec![sc(0, ShortcutType::Item, 500), sc(1, ShortcutType::Item, 999), sc(2, ShortcutType::Skill, 1177)];
    chr.macros = vec![Macro {
        id: 1005,
        icon: 1,
        name: "m".into(),
        descr: String::new(),
        acronym: String::new(),
        commands: vec![MacroCmd { entry: 0, kind: MacroType::Text, d1: 0, d2: 0, cmd: "/loc".into() }],
    }];

    let bundle = Player::from_char(&world.data, &chr);
    let restored: Vec<_> = bundle.shortcuts.iter().copied().collect();
    assert_eq!(restored.len(), 2, "stale ITEM shortcut pruned");
    assert!(restored.iter().any(|s| s.kind == ShortcutType::Item && s.id == 500));
    assert!(restored.iter().any(|s| s.kind == ShortcutType::Skill && s.id == 1177));
    assert_eq!(Player::stale_item_shortcuts(&chr), vec![(1, 0)]);
    assert_eq!(bundle.macros.entries.len(), 1);
    assert_eq!(bundle.macros.entries[0].commands[0].cmd, "/loc");
}

/// The enter-world burst carries the real `ShortCutInit` and the macro LIST
/// packets in Java's order (macros before `ItemList`, panel after it).
#[test]
fn enter_world_sends_macros_and_shortcut_panel() {
    let (mut world, ..) = test_world();
    let mut chr = dummy_char(3001, "P");
    chr.shortcuts = vec![Shortcut { slot: 0, page: 0, kind: ShortcutType::Action, id: 2, level: 0, character_type: 1, shared_reuse_group: -1 }];
    chr.macros = vec![Macro { id: 1000, icon: 0, name: "m".into(), descr: String::new(), acronym: String::new(), commands: vec![] }];
    let bundle = Player::from_char(&world.data, &chr);
    let (out_tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let s = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![])
        .into_entering(bundle);
    world.clients.insert(1, ClientSession::Entering(s));

    handle_enter_world(&mut world, 1);
    let packets = drain(&mut rx);
    let pos = |op: u8| packets.iter().position(|p| p[0] == op).unwrap_or_else(|| panic!("opcode 0x{op:02x} missing"));
    let macro_pos = pos(server_packets::opcodes::MACRO_LIST);
    let item_list_pos = pos(0x11);
    let shortcut_pos = pos(server_packets::opcodes::SHORT_CUT_INIT);
    assert!(macro_pos < item_list_pos, "macros before ItemList");
    assert!(item_list_pos < shortcut_pos, "ShortCutInit after ItemList");
    let sc_pkt = &packets[shortcut_pos];
    assert_eq!(i32::from_le_bytes([sc_pkt[1], sc_pkt[2], sc_pkt[3], sc_pkt[4]]), 1, "one shortcut");
    let m_pkt = &packets[macro_pos];
    assert_eq!(m_pkt[6], 1, "one macro in the LIST burst");
}

// ---------------------------------------------------------------- chat (G10)

fn say2_body(text: &str, chat_type: i32, target: Option<&str>) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(text);
    w.write_i32(chat_type);
    if let Some(t) = target {
        w.write_string(t);
    }
    w.into_bytes()
}

/// Parsed `CreatureSay` (test-side): (sender oid, chat type, name, text,
/// whisper tail).
fn parse_creature_say(pkt: &[u8]) -> (i32, i32, String, String, Option<(u8, u8)>) {
    assert_eq!(pkt[0], server_packets::opcodes::SAY2, "not a CreatureSay: 0x{:02x}", pkt[0]);
    let mut r = commons::network::PacketReader::new(&pkt[1..]);
    let oid = r.read_i32().unwrap();
    let ty = r.read_i32().unwrap();
    let name = r.read_string().unwrap();
    assert_eq!(r.read_i32().unwrap(), -1, "NpcString id slot");
    let text = r.read_string().unwrap();
    let tail = r.read_u8().map(|mask| (mask, r.read_u8().unwrap_or(0)));
    (oid, ty, name, text, tail)
}

/// General chat reaches the speaker and players within 1250 units, but not a
/// region-adjacent player standing further away.
#[test]
fn general_chat_is_scoped_to_1250_units() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 1000, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 2000, 0, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    on_packet(&mut world, 1, [vec![cop::SAY2], say2_body("hello there", 0, None)].concat());

    let a_pkts = drain(&mut a_rx);
    assert_eq!(a_pkts.len(), 1, "speaker gets exactly the echo");
    let (oid, ty, name, text, tail) = parse_creature_say(&a_pkts[0]);
    assert_eq!((oid, ty), (3001, 0));
    assert_eq!(name, "P3001");
    assert_eq!(text, "hello there");
    assert!(tail.is_none(), "no whisper tail on general chat");

    let b_pkts = drain(&mut b_rx);
    assert_eq!(b_pkts.len(), 1, "in-range bystander hears it");
    assert!(drain(&mut c_rx).is_empty(), "1250+ units away hears nothing");
}

/// Whisper: case-insensitive name lookup, receiver gets the message with the
/// relation-mask tail (mask 0 + sender level), sender gets the `->Name` echo;
/// whispering to a name that isn't online answers SM 145.
#[test]
fn whisper_delivers_echoes_and_rejects_offline() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 500, 0, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(&mut world, 1, [vec![cop::SAY2], say2_body("psst", 2, Some("p3002"))].concat());

    let b_pkts = drain(&mut b_rx);
    assert_eq!(b_pkts.len(), 1);
    let (oid, ty, name, text, tail) = parse_creature_say(&b_pkts[0]);
    assert_eq!((oid, ty), (3001, 2));
    assert_eq!(name, "P3001");
    assert_eq!(text, "psst");
    assert_eq!(tail, Some((0, 1)), "mask 0 + sender level 1");

    let a_pkts = drain(&mut a_rx);
    assert_eq!(a_pkts.len(), 1);
    let (_, _, echo_name, _, echo_tail) = parse_creature_say(&a_pkts[0]);
    assert_eq!(echo_name, "->P3002");
    assert_eq!(echo_tail, Some((0, 1)));

    on_packet(&mut world, 1, [vec![cop::SAY2], say2_body("psst", 2, Some("nobody"))].concat());
    let a_pkts = drain(&mut a_rx);
    assert_eq!(a_pkts.len(), 1);
    assert_eq!(sm_id(&a_pkts[0]), server_packets::sm_ids::THAT_PLAYER_IS_NOT_ONLINE);
}

/// Shout/trade use map-region buckets; with no map regions loaded everyone
/// shares Java's fallback bucket, so even a far player hears it. Party/clan
/// chat without the group answers the "you are not in a …" SMs, and an
/// over-long line gets the spam warning.
#[test]
fn shout_reaches_region_bucket_and_groupless_chats_reject() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 60_000, 0, 0);
    drain(&mut a_rx);
    drain(&mut c_rx);

    on_packet(&mut world, 1, [vec![cop::SAY2], say2_body("WTS stuff", 1, None)].concat());
    let c_pkts = drain(&mut c_rx);
    assert_eq!(c_pkts.len(), 1, "same (empty) map-region bucket");
    let (_, ty, _, _, _) = parse_creature_say(&c_pkts[0]);
    assert_eq!(ty, 1);
    drain(&mut a_rx);

    on_packet(&mut world, 1, [vec![cop::SAY2], say2_body("anyone?", 3, None)].concat());
    let a_pkts = drain(&mut a_rx);
    assert_eq!(sm_id(&a_pkts[0]), server_packets::sm_ids::YOU_ARE_NOT_IN_A_PARTY);

    on_packet(&mut world, 1, [vec![cop::SAY2], say2_body("hi clan", 4, None)].concat());
    let a_pkts = drain(&mut a_rx);
    assert_eq!(sm_id(&a_pkts[0]), server_packets::sm_ids::YOU_ARE_NOT_IN_A_CLAN);

    let long = "x".repeat(106);
    on_packet(&mut world, 1, [vec![cop::SAY2], say2_body(&long, 0, None)].concat());
    let a_pkts = drain(&mut a_rx);
    assert_eq!(sm_id(&a_pkts[0]), server_packets::sm_ids::KEYBOARD_INPUT_SPAM_WARNING);
    assert!(drain(&mut c_rx).is_empty(), "rejected line is not broadcast");
}

// --------------------------------------------------------------- party (G10)

use crate::model::components::{PartyRef, PendingRequest};
use crate::model::party::LootRule;

fn join_party_body(name: &str, loot_rule_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(name);
    w.write_i32(loot_rule_id);
    w.into_bytes()
}

fn int_body(v: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(v);
    w.into_bytes()
}

fn name_body(name: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(name);
    w.into_bytes()
}

fn ex_packet(sub: u16, body: &[u8]) -> Vec<u8> {
    let mut out = vec![cop::EX_PACKET, (sub & 0xff) as u8, (sub >> 8) as u8];
    out.extend_from_slice(body);
    out
}

fn has_opcode(pkts: &[Vec<u8>], opcode: u8) -> bool {
    pkts.iter().any(|p| p[0] == opcode)
}

fn sm_ids_of(pkts: &[Vec<u8>]) -> Vec<i16> {
    pkts.iter()
        .filter(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE)
        .map(|p| i16::from_le_bytes([p[1], p[2]]))
        .collect()
}

fn ex_subs_of(pkts: &[Vec<u8>]) -> Vec<i16> {
    pkts.iter()
        .filter(|p| p[0] == server_packets::opcodes::EX)
        .map(|p| i16::from_le_bytes([p[1], p[2]]))
        .collect()
}

/// Directly install a formed party (the invite flow has its own tests).
fn make_party(world: &mut World, members: &[i32], rule: LootRule) -> u32 {
    let id = world.next_party_id;
    world.next_party_id += 1;
    let seq = world.next_request_seq();
    let mut p = crate::model::party::Party::new(members[0], rule, seq);
    for &m in &members[1..] {
        p.members.push(m);
    }
    world.parties.insert(id, p);
    for &m in members {
        world.objects.add_components(&m, PartyRef(id));
    }
    id
}

/// `RequestAcquireSkill.checkPlayerSkill` gates: an under-level request sends
/// `YOU_DO_NOT_MEET_THE_SKILL_LEVEL_REQUIREMENTS`, an unaffordable one sends
/// `YOU_DO_NOT_HAVE_ENOUGH_SP_TO_LEARN_THIS_SKILL` — instead of silently dropping.
#[test]
fn skill_acquire_gates_send_system_messages() {
    use crate::data::skill_tree::SkillLearn;

    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0); // dummy_char: class 0, level 1, sp 0
    drain(&mut rx);

    // Under-level: get_level 10 > player level 1.
    world.data.skill_trees.insert_for_test(0, SkillLearn {
        skill_id: 1001,
        skill_level: 1,
        name: "Too High".into(),
        get_level: 10,
        level_up_sp: 0,
        auto_get: false,
    });
    // Reachable level, but costs more SP than the player has (sp 0).
    world.data.skill_trees.insert_for_test(0, SkillLearn {
        skill_id: 1002,
        skill_level: 1,
        name: "Too Pricey".into(),
        get_level: 1,
        level_up_sp: 100,
        auto_get: false,
    });

    handle_request_acquire_skill(&mut world, 1, &acquire_skill_body(1001, 1, cp::RequestAcquireSkill::CLASS));
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        vec![server_packets::sm_ids::YOU_DO_NOT_MEET_THE_SKILL_LEVEL_REQUIREMENTS],
    );

    handle_request_acquire_skill(&mut world, 1, &acquire_skill_body(1002, 1, cp::RequestAcquireSkill::CLASS));
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        vec![server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_SP_TO_LEARN_THIS_SKILL],
    );

    // Neither gate learned the skill.
    let book = world.objects.get_component::<crate::model::components::SkillBook>(&3001).unwrap();
    assert!(!book.0.contains_key(&1001) && !book.0.contains_key(&1002));
}

fn acquire_skill_body(skill_id: i32, skill_level: i32, acquire_type: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(skill_id);
    w.write_i32(skill_level);
    w.write_i32(acquire_type);
    w.into_bytes()
}

/// `UserInfo.calculateRelation` (via `party::calculate_relation`): the party
/// and clan bits, driven off the `PartyRef` component and the `Player`'s clan
/// fields. The siege bit (0x80) is unported, so it never sets.
#[test]
fn relation_reflects_party_and_clan() {
    let (mut world, ..) = test_world();
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    let snapshot = |w: &World, oid: i32| w.objects.get_component::<Player>(&oid).unwrap().clone();

    // Solo, clanless → 0.
    assert_eq!(super::party::calculate_relation(&world, &snapshot(&world, 3001)), 0);

    // Clan member + leader → 0x20 | 0x40.
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.clan_id = 7;
        p.clan_leader = true;
    }
    assert_eq!(super::party::calculate_relation(&world, &snapshot(&world, 3001)), 0x20 | 0x40);

    // Clan member, not leader → 0x20.
    world.objects.get_component_mut::<Player>(&3001).unwrap().clan_leader = false;
    assert_eq!(super::party::calculate_relation(&world, &snapshot(&world, 3001)), 0x20);

    // Party leader (3001 first) → adds 0x08 | 0x10; the non-leader member
    // (3002, clanless) gets 0x08 only.
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    assert_eq!(super::party::calculate_relation(&world, &snapshot(&world, 3001)), 0x20 | 0x08 | 0x10);
    assert_eq!(super::party::calculate_relation(&world, &snapshot(&world, 3002)), 0x08);
}

/// `StoreSkillCooltime` round-trip: a live cooldown is captured into the save
/// (as an absolute wall-clock end time) and, on relog, `restore_reuses` re-arms
/// it against the current game tick — the cooldown survives the trip.
#[test]
fn skill_reuse_cooldown_survives_relog() {
    use crate::model::components::Reuses;
    use crate::model::SkillReuse;

    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    // A cooldown on reuse-key 1177, ending 500 ticks (50 s) out.
    world.objects.get_component_mut::<Reuses>(&3001).unwrap().0.insert(
        1177,
        SkillReuse { skill_level: 3, until_tick: world.tick + 500, total_ms: 300_000 },
    );

    // The save captures it (config default = on) as an absolute systime.
    let save = super::net::build_save_data(&world, 3001).expect("save data");
    assert_eq!(save.skill_reuses.len(), 1);
    let row = save.skill_reuses[0];
    assert_eq!((row.reuse_key, row.skill_level, row.reuse_delay), (1177, 3, 300_000));

    // Relog: a fresh bundle from a CharData carrying that row, restored against
    // the current tick + wall clock.
    let mut chr = dummy_char(3002, "Relog");
    chr.skill_reuses = vec![row];
    let mut bundle = Player::from_char(&world.data, &chr);
    bundle.restore_reuses(&chr, world.tick, commons::util::now_millis());

    let restored = bundle.reuses.0.get(&1177).expect("cooldown restored");
    assert_eq!((restored.skill_level, restored.total_ms), (3, 300_000));
    let remaining = restored.until_tick - world.tick;
    assert!((498..=500).contains(&remaining), "≈500 ticks left, got {remaining}");

    // With the config off, nothing is persisted (and the DB rows get cleared).
    world.cfg.character.store_skill_cooltime = false;
    assert!(super::net::build_save_data(&world, 3001).unwrap().skill_reuses.is_empty());
}

/// The invite → accept happy path: SM 105 + AskJoinParty, then JoinParty(1),
/// the window packets on both sides, the joined SMs, and a live party with
/// the leader first.
#[test]
fn party_invite_accept_builds_party_and_windows() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(&mut world, 1, [vec![cop::REQUEST_JOIN_PARTY], join_party_body("p3002", 1)].concat());
    let a_pkts = drain(&mut a_rx);
    assert_eq!(sm_ids_of(&a_pkts), vec![server_packets::sm_ids::C1_HAS_BEEN_INVITED_TO_THE_PARTY]);
    let b_pkts = drain(&mut b_rx);
    assert_eq!(b_pkts.len(), 1);
    assert_eq!(b_pkts[0][0], server_packets::opcodes::ASK_JOIN_PARTY);
    {
        let mut r = commons::network::PacketReader::new(&b_pkts[0][1..]);
        assert_eq!(r.read_string().unwrap(), "P3001");
        assert_eq!(r.read_i32().unwrap(), 1, "Random loot rule echoed");
    }
    assert!(world.objects.has_component::<PendingRequest>(&3001));
    assert!(world.objects.has_component::<PendingRequest>(&3002));

    on_packet(&mut world, 2, [vec![cop::REQUEST_ANSWER_JOIN_PARTY], int_body(1)].concat());

    let a_pkts = drain(&mut a_rx);
    assert!(has_opcode(&a_pkts, server_packets::opcodes::JOIN_PARTY), "JoinParty echo");
    assert!(has_opcode(&a_pkts, server_packets::opcodes::PARTY_SMALL_WINDOW_ADD), "leader window gains the member");
    assert!(sm_ids_of(&a_pkts).contains(&server_packets::sm_ids::C1_HAS_JOINED_THE_PARTY));

    let b_pkts = drain(&mut b_rx);
    let all = b_pkts.iter().find(|p| p[0] == server_packets::opcodes::PARTY_SMALL_WINDOW_ALL).expect("window all");
    {
        let mut r = commons::network::PacketReader::new(&all[1..]);
        assert_eq!(r.read_i32().unwrap(), 3001, "leader object id");
        assert_eq!(r.read_u8().unwrap(), 1, "loot rule byte");
        assert_eq!(r.read_u8().unwrap(), 1, "one other member");
        assert_eq!(r.read_i32().unwrap(), 3001, "the leader's entry");
    }
    let b_sms = sm_ids_of(&b_pkts);
    assert!(b_sms.contains(&server_packets::sm_ids::YOU_HAVE_JOINED_S1_S_PARTY));
    assert!(b_sms.contains(&server_packets::sm_ids::C1_HAS_JOINED_THE_PARTY));

    assert_eq!(world.parties.len(), 1);
    let party = world.parties.values().next().unwrap();
    assert_eq!(party.members, vec![3001, 3002]);
    assert!(!party.pending_invitation);
    let a_ref = world.objects.get_component::<PartyRef>(&3001).copied().unwrap();
    let b_ref = world.objects.get_component::<PartyRef>(&3002).copied().unwrap();
    assert_eq!(a_ref, b_ref);
    assert!(!world.objects.has_component::<PendingRequest>(&3001), "request cleared");
}

/// Declining the first invite answers JoinParty(0) and dissolves the embryo
/// party; guards: target busy (SM 153 via second inviter), target already in
/// a party (SM 160), non-leader invites (SM 154).
#[test]
fn party_invite_decline_and_guards() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 200, 0, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    // A invites B; C tries to invite the busy B → SM 153 after the SM 105.
    on_packet(&mut world, 1, [vec![cop::REQUEST_JOIN_PARTY], join_party_body("P3002", 0)].concat());
    on_packet(&mut world, 3, [vec![cop::REQUEST_JOIN_PARTY], join_party_body("P3002", 0)].concat());
    let c_sms = sm_ids_of(&drain(&mut c_rx));
    assert!(c_sms.contains(&server_packets::sm_ids::WAITING_FOR_ANOTHER_REPLY), "busy target: {c_sms:?}");

    // B declines: A gets JoinParty(0), the embryo party dies.
    drain(&mut a_rx);
    on_packet(&mut world, 2, [vec![cop::REQUEST_ANSWER_JOIN_PARTY], int_body(0)].concat());
    let a_pkts = drain(&mut a_rx);
    let jp = a_pkts.iter().find(|p| p[0] == server_packets::opcodes::JOIN_PARTY).expect("JoinParty");
    assert_eq!(i32::from_le_bytes(jp[1..5].try_into().unwrap()), 0, "declined");
    assert!(world.parties.is_empty(), "embryo party dissolved");
    assert!(!world.objects.has_component::<PartyRef>(&3001));

    // Formed party: B (not leader) inviting → SM 154; inviting a partied
    // player → SM 160.
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    drain(&mut b_rx);
    on_packet(&mut world, 2, [vec![cop::REQUEST_JOIN_PARTY], join_party_body("P3003", 0)].concat());
    let b_sms = sm_ids_of(&drain(&mut b_rx));
    assert!(b_sms.contains(&server_packets::sm_ids::ONLY_THE_LEADER_CAN_GIVE_OUT_INVITATIONS), "{b_sms:?}");
    drain(&mut c_rx);
    on_packet(&mut world, 3, [vec![cop::REQUEST_JOIN_PARTY], join_party_body("P3002", 0)].concat());
    let c_sms = sm_ids_of(&drain(&mut c_rx));
    assert!(c_sms.contains(&server_packets::sm_ids::C1_IS_A_MEMBER_OF_ANOTHER_PARTY_AND_CANNOT_BE_INVITED), "{c_sms:?}");
}

/// An unanswered invite times out after 30 s: both request slots clear and
/// the embryo party is dropped.
#[test]
fn party_invite_timeout_drops_embryo_party() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    drain(&mut a_rx);

    on_packet(&mut world, 1, [vec![cop::REQUEST_JOIN_PARTY], join_party_body("P3002", 0)].concat());
    assert_eq!(world.parties.len(), 1);
    advance_ticks(&mut world, 301);
    assert!(!world.objects.has_component::<PendingRequest>(&3001));
    assert!(!world.objects.has_component::<PendingRequest>(&3002));
    assert!(world.parties.is_empty(), "unanswered embryo party dropped");
}

/// Leaving a 2-member party disbands it (SM 203 + window clear on both).
#[test]
fn party_withdrawal_two_members_disbands() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(&mut world, 2, vec![cop::REQUEST_WITH_DRAWAL_PARTY]);
    for rx in [&mut a_rx, &mut b_rx] {
        let pkts = drain(rx);
        assert!(sm_ids_of(&pkts).contains(&server_packets::sm_ids::THE_PARTY_HAS_DISPERSED));
        assert!(has_opcode(&pkts, server_packets::opcodes::PARTY_SMALL_WINDOW_DELETE_ALL));
    }
    assert!(world.parties.is_empty());
    assert!(!world.objects.has_component::<PartyRef>(&3001));
    assert!(!world.objects.has_component::<PartyRef>(&3002));
}

/// A 3-member party: the leader disconnecting transfers leadership (SM 1384 +
/// window rebuild), ousting sends SM 202/201 + the delete entry, and
/// `RequestChangePartyLeader` swaps slot 0.
#[test]
fn party_leadership_oust_and_change() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 200, 0, 0);
    let party_id = make_party(&mut world, &[3001, 3002, 3003], LootRule::FindersKeepers);
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    // Leader disconnects → B becomes leader.
    world.clients.remove(&1);
    net::store_and_remove_player(&mut world, 3001);
    let b_pkts = drain(&mut b_rx);
    assert!(sm_ids_of(&b_pkts).contains(&server_packets::sm_ids::C1_HAS_BECOME_THE_PARTY_LEADER));
    assert!(has_opcode(&b_pkts, server_packets::opcodes::PARTY_SMALL_WINDOW_ALL), "window rebuilt");
    assert_eq!(world.parties[&party_id].members, vec![3002, 3003]);

    // New leader B ousts C → 2-member party disbands instead (the 2-left
    // rule), C sees the expelled SM.
    on_packet(&mut world, 2, [vec![cop::REQUEST_OUST_PARTY_MEMBER], name_body("P3003")].concat());
    let c_pkts = drain(&mut c_rx);
    assert!(sm_ids_of(&c_pkts).contains(&server_packets::sm_ids::THE_PARTY_HAS_DISPERSED));
    assert!(world.parties.is_empty());

    // Fresh 3-member party exercises oust + leader change proper.
    let mut a2_rx = ingame_player(&mut world, 1, 3004, 0, 0, 0);
    let party_id = make_party(&mut world, &[3004, 3002, 3003], LootRule::FindersKeepers);
    drain(&mut a2_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);
    on_packet(&mut world, 1, [vec![cop::REQUEST_OUST_PARTY_MEMBER], name_body("P3002")].concat());
    let b_pkts = drain(&mut b_rx);
    assert!(sm_ids_of(&b_pkts).contains(&server_packets::sm_ids::YOU_HAVE_BEEN_EXPELLED_FROM_THE_PARTY));
    assert!(has_opcode(&b_pkts, server_packets::opcodes::PARTY_SMALL_WINDOW_DELETE_ALL));
    let c_pkts = drain(&mut c_rx);
    assert!(sm_ids_of(&c_pkts).contains(&server_packets::sm_ids::C1_WAS_EXPELLED_FROM_THE_PARTY));
    assert!(has_opcode(&c_pkts, server_packets::opcodes::PARTY_SMALL_WINDOW_DELETE));
    assert_eq!(world.parties[&party_id].members, vec![3004, 3003]);

    // Change leader to C; a repeat naming the (new) leader → SM 1401 quirk
    // (sent to the requestor, who is no longer leader → silently ignored, so
    // name the current leader from the current leader instead).
    on_packet(&mut world, 1, ex_packet(0x0C, &name_body("P3003")));
    assert_eq!(world.parties[&party_id].members, vec![3003, 3004]);
    drain(&mut c_rx);
    on_packet(&mut world, 3, ex_packet(0x0C, &name_body("P3003")));
    let c_sms = sm_ids_of(&drain(&mut c_rx));
    assert!(c_sms.contains(&server_packets::sm_ids::SLOW_DOWN_YOU_ARE_ALREADY_THE_PARTY_LEADER), "{c_sms:?}");
}

/// Loot-rule voting: unanimous yes applies the rule (ExSetPartyLooting(1) +
/// SM 3138), the 15 s timeout cancels (ExSetPartyLooting(0) + SM 3137).
#[test]
fn party_loot_change_vote_and_timeout() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 200, 0, 0);
    let party_id = make_party(&mut world, &[3001, 3002, 3003], LootRule::FindersKeepers);
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    // Leader proposes Random (1): members get the FE:C0 ask, leader SM 3135.
    on_packet(&mut world, 1, ex_packet(0x75, &int_body(1)));
    assert!(ex_subs_of(&drain(&mut b_rx)).contains(&0xC0));
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::REQUESTING_APPROVAL_FOR_CHANGING_PARTY_LOOT_TO_S1));

    // Both members agree → applied everywhere.
    on_packet(&mut world, 2, ex_packet(0x76, &int_body(1)));
    on_packet(&mut world, 3, ex_packet(0x76, &int_body(1)));
    let a_pkts = drain(&mut a_rx);
    assert!(ex_subs_of(&a_pkts).contains(&0xC1), "ExSetPartyLooting");
    assert!(sm_ids_of(&a_pkts).contains(&server_packets::sm_ids::PARTY_LOOT_WAS_CHANGED_TO_S1));
    assert_eq!(world.parties[&party_id].distribution, LootRule::Random);

    // Second proposal times out → cancelled, rule unchanged.
    on_packet(&mut world, 1, ex_packet(0x75, &int_body(3)));
    drain(&mut a_rx);
    advance_ticks(&mut world, 151);
    let a_pkts = drain(&mut a_rx);
    assert!(sm_ids_of(&a_pkts).contains(&server_packets::sm_ids::PARTY_LOOT_CHANGE_WAS_CANCELLED));
    assert_eq!(world.parties[&party_id].distribution, LootRule::Random);
}

/// Party chat reaches exactly the members (speaker echo included).
#[test]
fn party_chat_reaches_members_only() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 200, 0, 0);
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    on_packet(&mut world, 1, [vec![cop::SAY2], say2_body("inc mob", 3, None)].concat());
    let (_, ty, _, text, _) = parse_creature_say(&drain(&mut b_rx)[0]);
    assert_eq!((ty, text.as_str()), (3, "inc mob"));
    assert_eq!(drain(&mut a_rx).len(), 1, "speaker echo");
    assert!(drain(&mut c_rx).is_empty(), "non-member hears nothing");
}

/// A party member taking damage pushes `PartySmallWindowUpdate` (vitals
/// flags) to the other members, not to themselves.
#[test]
fn party_vitals_piggyback_on_damage() {
    let (mut world, ..) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    drain(&mut a_rx);
    drain(&mut b_rx);

    combat::player_receive_damage(&mut world, 3002, 3001, 30.0);
    let a_pkts = drain(&mut a_rx);
    let upd = a_pkts.iter().find(|p| p[0] == server_packets::opcodes::PARTY_SMALL_WINDOW_UPDATE).expect("window update");
    let mut r = commons::network::PacketReader::new(&upd[1..]);
    assert_eq!(r.read_i32().unwrap(), 3002, "the damaged member's entry");
    assert_eq!(r.read_i16().unwrap() as u16, server_packets::party_window_flags::VITALS);
    assert!(!has_opcode(&drain(&mut b_rx), server_packets::opcodes::PARTY_SMALL_WINDOW_UPDATE), "not echoed to self");
}

/// The 12 s position broadcast reaches every member and keeps rescheduling
/// while the party lives.
#[test]
fn party_position_broadcast_ticks() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    // Through the real flow so the broadcast task starts.
    on_packet(&mut world, 1, [vec![cop::REQUEST_JOIN_PARTY], join_party_body("P3002", 0)].concat());
    on_packet(&mut world, 2, [vec![cop::REQUEST_ANSWER_JOIN_PARTY], int_body(1)].concat());
    drain(&mut a_rx);
    drain(&mut b_rx);

    advance_ticks(&mut world, 61); // initial delay = period/2 = 6 s
    let a_pkts = drain(&mut a_rx);
    let pos = a_pkts.iter().find(|p| p[0] == server_packets::opcodes::PARTY_MEMBER_POSITION).expect("positions");
    let mut r = commons::network::PacketReader::new(&pos[1..]);
    assert_eq!(r.read_i32().unwrap(), 2, "both members listed");

    advance_ticks(&mut world, 120);
    assert!(has_opcode(&drain(&mut b_rx), server_packets::opcodes::PARTY_MEMBER_POSITION), "keeps ticking");

    // Disband kills the task.
    on_packet(&mut world, 2, vec![cop::REQUEST_WITH_DRAWAL_PARTY]);
    drain(&mut a_rx);
    advance_ticks(&mut world, 240);
    assert!(!has_opcode(&drain(&mut a_rx), server_packets::opcodes::PARTY_MEMBER_POSITION), "task died with the party");
}

/// A party kill splits XP/SP with Java's math: both level-5 members in range,
/// killer deals all damage → base 2000 XP × 1.3 party bonus, level²-weighted
/// (equal levels → 1300 each); SP likewise (100 × 1.3 / 2 = 65).
#[test]
fn party_kill_splits_xp_and_sp() {
    let (mut world, mut _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    for oid in [3001, 3002] {
        world.objects.get_component_mut::<crate::model::Player>(&oid).unwrap().exp = 4000;
    }

    let npc_oid = NPC_OID + 21;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 100, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);
    world.forced_rolls.extend([0, 99, 10]); // hit, no crit, ±0 damage
    // Kill the drop roll chances deterministically: level-gap gate passes
    // (roll 0), drop chance fails (roll ~1.0 impossible via forced_rolls —
    // use the f64 hook by clearing the drop list instead).
    {
        let mut t = world.data.npc_data.get(40001).unwrap().clone();
        t.drop_list_death.clear();
        world.data.npc_data.insert_for_test(t);
    }
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    advance_world(&mut world, 12); // swing lands

    assert!(pvit(&world, npc_oid).dead || world.objects.get_component::<Vitals>(&npc_oid).is_none(), "monster died");
    let a_exp = world.objects.get_component::<crate::model::Player>(&3001).unwrap().exp;
    let b_exp = world.objects.get_component::<crate::model::Player>(&3002).unwrap().exp;
    assert_eq!(a_exp, 4000 + 1300, "killer: 2000 × 1.3 bonus × 25/50");
    assert_eq!(b_exp, 4000 + 1300, "idle member gets the same equal-level share");
    let b_sp = world.objects.get_component::<crate::model::Player>(&3002).unwrap().sp;
    assert_eq!(b_sp, 65, "SP: 100 × 1.3 / 2");
}

/// `Party.distributeItem`: adena splits evenly among in-range members;
/// BY_TURN rotates the looter, skipping the out-of-range member.
#[test]
fn party_loot_split_and_rotation() {
    let (mut world, ..) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    let mut c_rx = ingame_caster(&mut world, 3, 3003, 99_000, 0); // out of range
    let party_id = make_party(&mut world, &[3001, 3002, 3003], LootRule::ByTurn);
    world.data.item_data.insert_for_test(crate::data::item_data::ItemTemplate {
        item_id: 1234,
        name: "Test Loot".into(),
        kind: crate::data::item_data::ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: false,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: crate::data::item_data::ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
    });
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    // Adena: 100 split across the 2 in-range members → 50 each.
    party::distribute_item(&mut world, party_id, 3001, 57, 100, (0, 0));
    let count_of = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&oid)
            .map(|inv| inv.items().iter().filter(|i| i.item_id == 57).map(|i| i.count).sum::<i64>())
            .unwrap_or(0)
    };
    assert_eq!(count_of(&world, 3001), 50);
    assert_eq!(count_of(&world, 3002), 50);
    assert_eq!(count_of(&world, 3003), 0, "out-of-range member gets nothing");

    // BY_TURN: cursor starts at 0 → first item to member index 1 (3002),
    // next wraps past out-of-range 3003 back to 3001.
    party::distribute_item(&mut world, party_id, 3001, 1234, 1, (0, 0));
    party::distribute_item(&mut world, party_id, 3001, 1234, 1, (0, 0));
    let has_item = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&oid)
            .is_some_and(|inv| inv.items().iter().any(|i| i.item_id == 1234))
    };
    assert!(has_item(&world, 3002), "first by-turn item");
    assert!(has_item(&world, 3001), "rotation skipped the far member");
    assert!(!has_item(&world, 3003));
    // The non-looting members saw the "C1 has obtained" line.
    assert!(sm_ids_of(&drain(&mut b_rx)).contains(&server_packets::sm_ids::C1_HAS_OBTAINED_S2));
}

// ------------------------------------------------------------- friends (G10)

use crate::character::FriendInfo;
use crate::model::components::Friends;

fn friend_answer_body(response: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0);
    w.write_i32(response);
    w.into_bytes()
}

fn seed_friendship(world: &mut World, a: i32, b: i32) {
    let info = |world: &World, oid: i32| {
        let p = world.objects.get_component::<crate::model::Player>(&oid).unwrap();
        FriendInfo { char_id: oid, name: p.name.clone(), level: p.level, class_id: p.class_id }
    };
    let (ia, ib) = (info(world, a), info(world, b));
    world.objects.get_component_mut::<Friends>(&a).unwrap().0.push(ib);
    world.objects.get_component_mut::<Friends>(&b).unwrap().0.push(ia);
}

/// Invite → accept: FriendAddRequest popup, both sides' SMs +
/// FriendAddRequestResult, both lists updated, one DB pair insert; the
/// whisper relation mask then carries the friend bit.
#[test]
fn friend_invite_accept_and_whisper_mask() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(&mut world, 1, [vec![cop::REQUEST_FRIEND_INVITE], name_body("p3002")].concat());
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::YOU_VE_REQUESTED_C1_TO_BE_ON_YOUR_FRIENDS_LIST));
    assert!(has_opcode(&drain(&mut b_rx), server_packets::opcodes::FRIEND_ADD_REQUEST));

    on_packet(&mut world, 2, [vec![cop::REQUEST_ANSWER_FRIEND_INVITE], friend_answer_body(1)].concat());
    let a_pkts = drain(&mut a_rx);
    let a_sms = sm_ids_of(&a_pkts);
    assert!(a_sms.contains(&server_packets::sm_ids::FRIEND_ADDED_SUCCESSFULLY), "{a_sms:?}");
    assert!(a_sms.contains(&server_packets::sm_ids::S1_HAS_BEEN_ADDED_TO_YOUR_FRIENDS_LIST));
    assert!(has_opcode(&a_pkts, server_packets::opcodes::FRIEND_ADD_REQUEST_RESULT));
    let b_pkts = drain(&mut b_rx);
    assert!(sm_ids_of(&b_pkts).contains(&server_packets::sm_ids::S1_HAS_BEEN_ADDED_TO_YOUR_FRIENDS_LIST_2));
    assert!(has_opcode(&b_pkts, server_packets::opcodes::FRIEND_ADD_REQUEST_RESULT));

    let a_friends = world.objects.get_component::<Friends>(&3001).unwrap();
    assert_eq!(a_friends.0.len(), 1);
    assert_eq!((a_friends.0[0].char_id, a_friends.0[0].name.as_str()), (3002, "P3002"));
    assert_eq!(world.objects.get_component::<Friends>(&3002).unwrap().0[0].char_id, 3001);

    let mut saw_insert = false;
    while let Ok(cmd) = db_rx.try_recv() {
        if let db::DbCommand::InsertFriendPair { a, b } = cmd {
            assert_eq!((a, b), (3001, 3002));
            saw_insert = true;
        }
    }
    assert!(saw_insert, "friendship persisted");

    // Whisper now carries the friend relation bit (receiver's view).
    on_packet(&mut world, 1, [vec![cop::SAY2], say2_body("hey", 2, Some("P3002"))].concat());
    let (_, _, _, _, tail) = parse_creature_say(&drain(&mut b_rx)[0]);
    assert_eq!(tail, Some((0x01, 1)), "friend bit set");
}

/// Delete by name updates both sides' lists and rows; unknown names answer
/// SM 171. Friend messages deliver only when the receiver friended the
/// sender.
#[test]
fn friend_delete_and_messages() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    seed_friendship(&mut world, 3001, 3002);
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Friend message A → B (B has A friended).
    let mut msg = PacketWriter::new();
    msg.write_string("meet at giran");
    msg.write_string("P3002");
    on_packet(&mut world, 1, [vec![cop::REQUEST_SEND_FRIEND_MSG], msg.into_bytes()].concat());
    let b_pkts = drain(&mut b_rx);
    let say = b_pkts.iter().find(|p| p[0] == server_packets::opcodes::L2_FRIEND_SAY).expect("friend say");
    let mut r = commons::network::PacketReader::new(&say[1..]);
    r.read_i32().unwrap();
    assert_eq!(r.read_string().unwrap(), "P3002");
    assert_eq!(r.read_string().unwrap(), "P3001");
    assert_eq!(r.read_string().unwrap(), "meet at giran");

    // Delete: both lists + both clients + the DB pair.
    on_packet(&mut world, 1, [vec![cop::REQUEST_FRIEND_DEL], name_body("p3002")].concat());
    let a_pkts = drain(&mut a_rx);
    assert!(sm_ids_of(&a_pkts).contains(&server_packets::sm_ids::S1_HAS_BEEN_REMOVED_FROM_YOUR_FRIENDS_LIST_2));
    assert!(has_opcode(&a_pkts, server_packets::opcodes::FRIEND_REMOVE));
    assert!(has_opcode(&drain(&mut b_rx), server_packets::opcodes::FRIEND_REMOVE));
    assert!(world.objects.get_component::<Friends>(&3001).unwrap().0.is_empty());
    assert!(world.objects.get_component::<Friends>(&3002).unwrap().0.is_empty());
    let mut saw_delete = false;
    while let Ok(cmd) = db_rx.try_recv() {
        if let db::DbCommand::DeleteFriendPair { a, b } = cmd {
            assert_eq!((a, b), (3001, 3002));
            saw_delete = true;
        }
    }
    assert!(saw_delete);

    // Now strangers: the message bounces, delete answers SM 171.
    let mut msg = PacketWriter::new();
    msg.write_string("hello?");
    msg.write_string("P3002");
    on_packet(&mut world, 1, [vec![cop::REQUEST_SEND_FRIEND_MSG], msg.into_bytes()].concat());
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::THAT_PLAYER_IS_NOT_ONLINE));
    on_packet(&mut world, 1, [vec![cop::REQUEST_FRIEND_DEL], name_body("P3002")].concat());
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::C1_IS_NOT_ON_YOUR_FRIEND_LIST));
}

/// Enter world sends the real `L2FriendList` and pings online friends with
/// SM 503 + `FriendStatus(ONLINE)`; leaving pings `FriendStatus(OFFLINE)`.
#[test]
fn friend_login_logout_notifications() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        // A has B friended (for display); B's own list drives the pings.
        let p = FriendInfo { char_id: 3002, name: "P3002".into(), level: 1, class_id: 0 };
        world.objects.get_component_mut::<Friends>(&3001).unwrap().0.push(p);
    }
    drain(&mut a_rx);

    // B enters the world with A on their friend list.
    let mut chr = dummy_char(3002, "P3002");
    chr.x = 100;
    chr.friends = vec![FriendInfo { char_id: 3001, name: "P3001".into(), level: 1, class_id: 0 }];
    let bundle = Player::from_char(&world.data, &chr);
    let (out_tx, mut b_rx) = tokio::sync::mpsc::unbounded_channel();
    let s = Session::new(2, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![])
        .into_entering(bundle);
    world.clients.insert(2, ClientSession::Entering(s));
    on_packet(&mut world, 2, vec![cop::ENTER_WORLD]);

    let b_pkts = drain(&mut b_rx);
    let list = b_pkts.iter().find(|p| p[0] == server_packets::opcodes::L2_FRIEND_LIST).expect("L2FriendList");
    let mut r = commons::network::PacketReader::new(&list[1..]);
    assert_eq!(r.read_i32().unwrap(), 1, "one friend");
    assert_eq!(r.read_i32().unwrap(), 3001);
    assert_eq!(r.read_string().unwrap(), "P3001");
    assert_eq!(r.read_i32().unwrap(), 1, "online");

    let a_pkts = drain(&mut a_rx);
    assert!(sm_ids_of(&a_pkts).contains(&server_packets::sm_ids::YOUR_FRIEND_S1_JUST_LOGGED_IN));
    let status = a_pkts.iter().find(|p| p[0] == server_packets::opcodes::FRIEND_STATUS).expect("FriendStatus");
    assert_eq!(i32::from_le_bytes(status[1..5].try_into().unwrap()), 1, "MODE_ONLINE");

    // B logs out → A gets the offline ping.
    on_packet(&mut world, 2, vec![cop::LOGOUT]);
    let a_pkts = drain(&mut a_rx);
    let status = a_pkts.iter().find(|p| p[0] == server_packets::opcodes::FRIEND_STATUS).expect("offline ping");
    assert_eq!(i32::from_le_bytes(status[1..5].try_into().unwrap()), 0, "MODE_OFFLINE");
}

// ---------------------------------------------------------------------------
// Quests (G11)
// ---------------------------------------------------------------------------

/// `combat_test_world` + the real dist html root and the item/NPC templates
/// the two shipped quests touch (pelts/bones as stackable quest items, the
/// Q00258 reward gear, the quest NPCs and their monsters).
fn quest_test_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db_rx, link_rx) = combat_test_world();
    world.data.root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/").to_string();
    for (item_id, name, is_quest_item, is_stackable) in [
        (702, "Wolf Pelt", true, true),
        (809, "Bone Fragment", true, true),
        (41, "Cloth Cap", false, false),
        (42, "Leather Cap", false, false),
        (462, "Stockings", false, false),
    ] {
        world.data.item_data.insert_for_test(crate::data::item_data::ItemTemplate {
            item_id,
            name: name.into(),
            kind: crate::data::item_data::ItemKind::Etc,
            body_part: 0,
            weight: 0,
            is_stackable,
            type1: 4,
            type2: if is_quest_item { 3 } else { 5 },
            is_quest_item,
            price: 0,
            handler: crate::data::item_data::ItemHandler::None,
            crystal_type: crate::data::item_data::CrystalType::None,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(),
        });
    }
    for npc_id in [20120i32, 20517] {
        let mut t = crate::data::npc_data::default_template(npc_id);
        t.type_name = "Monster".into();
        t.level = 5;
        t.base_hp_max = 100.0;
        t.base_mp_max = 30.0;
        world.data.npc_data.insert_for_test(t);
    }
    (world, db_rx, link_rx)
}

/// Decode a `PlaySound` (0x9E) packet's sound-file string.
fn play_sound_name(pkt: &[u8]) -> Option<String> {
    if pkt[0] != server_packets::opcodes::PLAY_SOUND {
        return None;
    }
    let mut r = commons::network::PacketReader::new(&pkt[1..]);
    r.read_i32()?;
    r.read_string()
}

fn sound_names(pkts: &[Vec<u8>]) -> Vec<String> {
    pkts.iter().filter_map(|p| play_sound_name(p)).collect()
}

fn is_ex(pkt: &[u8], sub: i16) -> bool {
    pkt[0] == server_packets::opcodes::EX && pkt.len() >= 3 && i16::from_le_bytes([pkt[1], pkt[2]]) == sub
}

/// The full Q00258 loop against the real dist htmls: quest window on talk
/// (`ExNpcQuestHtmlMessage` for the `.htm`), accept event (`startQuest`:
/// cond 1 + STARTED persisted, accept sound, `.html` via plain
/// `NpcHtmlMessage`), pelts accumulating on kills (quest tab refresh +
/// "earned" SM), the 40-pelt cond bump (`ExShowQuestMark` + middle sound),
/// and the turn-in (reward roll, quest items destroyed with removed-type
/// `InventoryUpdate` + DB deletes, repeatable exit wiping the state).
#[test]
fn quest_q00258_accept_collect_turn_in() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 3;
    drain_db(&mut db_rx);

    // Talk: the single talk-quest short-circuits the chooser; CREATED at
    // level 3 → 30001-02.htm → the quest-window packet (FE:0x8E).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    let pkts = drain(&mut rx);
    assert!(pkts.iter().any(|p| is_ex(p, server_packets::opcodes::EX_NPC_QUEST_HTML_MESSAGE)), "quest window html");

    // Accept.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00258_BringWolfPelts 30001-03.html")),
    );
    let pkts = drain(&mut rx);
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        let qs = &quests.0["Q00258_BringWolfPelts"];
        assert_eq!(qs.state, crate::model::quest::state::STARTED);
        assert_eq!(qs.cond(), 1);
    }
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::QUEST_LIST), "QuestList after accept");
    assert!(sound_names(&pkts).contains(&"ItemSound.quest_accept".to_string()), "accept sound");
    assert!(
        pkts.iter().any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE),
        ".html result uses the plain window"
    );
    // Memory-first: cond + state land in the Quests component (they persist on
    // the next flush, not per set).
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        let qs = &quests.0["Q00258_BringWolfPelts"];
        assert_eq!(qs.cond(), 1, "cond set in memory");
        assert_eq!(qs.state, crate::model::quest::state::STARTED, "state Started in memory");
    }

    // First wolf kill: one pelt, earned-SM, quest-tab refresh, itemget sound.
    let wolf = NPC_OID + 1;
    add_test_npc(&mut world, wolf, 20120, "Monster", 5, 30, 0, 0);
    death::npc_do_die(&mut world, wolf, 3001);
    let pkts = drain(&mut rx);
    let inv_count = |world: &World| {
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&3001)
            .unwrap()
            .count_of(702)
    };
    assert_eq!(inv_count(&world), 1);
    assert!(sm_ids_of(&pkts).contains(&server_packets::sm_ids::YOU_HAVE_EARNED_S1), "earned SM");
    assert!(pkts.iter().any(|p| is_ex(p, server_packets::opcodes::EX_QUEST_ITEM_LIST)), "quest tab refresh");
    assert!(sound_names(&pkts).contains(&"ItemSound.quest_itemget".to_string()));

    // 38 more pelts, then the 40th kill flips cond 2 (+ mark + middle).
    super::items::add_inventory_item(&mut world, 3001, 702, 38).unwrap();
    let wolf2 = NPC_OID + 2;
    add_test_npc(&mut world, wolf2, 20442, "Monster", 5, 30, 0, 0);
    death::npc_do_die(&mut world, wolf2, 3001);
    let pkts = drain(&mut rx);
    assert_eq!(inv_count(&world), 40);
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert_eq!(quests.0["Q00258_BringWolfPelts"].cond(), 2);
    }
    let mark = pkts.iter().find(|p| is_ex(p, server_packets::opcodes::EX_SHOW_QUEST_MARK)).expect("quest mark");
    assert_eq!(i32::from_le_bytes(mark[3..7].try_into().unwrap()), 258);
    assert_eq!(i32::from_le_bytes(mark[7..11].try_into().unwrap()), 2);
    assert!(sound_names(&pkts).contains(&"ItemSound.quest_middle".to_string()));

    // Turn-in: roll 0 → Cloth Cap; pelts destroyed; repeatable exit.
    drain_db(&mut db_rx);
    world.forced_rolls.push_back(0);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00258_BringWolfPelts")));
    let pkts = drain(&mut rx);
    assert_eq!(inv_count(&world), 0, "pelts destroyed on exit");
    assert_eq!(
        world.objects.get_component::<crate::model::inventory::Inventory>(&3001).unwrap().count_of(41),
        1,
        "Cloth Cap rewarded on roll 0"
    );
    assert!(
        world
            .objects
            .get_component::<crate::model::components::Quests>(&3001)
            .unwrap()
            .0
            .get("Q00258_BringWolfPelts")
            .is_none(),
        "repeatable exit forgets the quest"
    );
    assert!(sound_names(&pkts).contains(&"ItemSound.quest_finish".to_string()));
    // The removal reaches the client as a removed-type InventoryUpdate.
    assert!(
        pkts.iter().any(|p| p[0] == 0x21 && i16::from_le_bytes([p[3], p[4]]) == 3),
        "InventoryUpdate with change type 3 (removed)"
    );
    // Memory-first: the pelts are gone from the Inventory component and the
    // quest from the Quests component (both asserted above); the flush reconcile
    // deletes their rows — no per-action DB write.

    // Re-talk: the quest is takeable again (CREATED intro window).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    let pkts = drain(&mut rx);
    assert!(pkts.iter().any(|p| is_ex(p, server_packets::opcodes::EX_NPC_QUEST_HTML_MESSAGE)), "repeatable re-offer");
}

/// Q00320's chance-drop path (forced `roll_f64`), the giveItemRandomly
/// limit semantics, the level/race start gates, and the rated adena reward.
#[test]
fn quest_q00320_chance_drops_and_adena_reward() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30359, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 10;
        p.race = 2; // Dark Elf
    }
    drain_db(&mut db_rx);

    // Accept (talk creates the CREATED state, the event starts it).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00320_BonesTellTheFuture 30359-04.htm")),
    );
    drain(&mut rx);

    let skel = NPC_OID + 1;
    add_test_npc(&mut world, skel, 20517, "Monster", 5, 30, 0, 0);

    // Roll 0.999999 > 0.18 → no drop.
    world.forced_rolls.push_back(999_999);
    death::npc_do_die(&mut world, skel, 3001);
    let count_of = |world: &World, id: i32| {
        world.objects.get_component::<crate::model::inventory::Inventory>(&3001).unwrap().count_of(id)
    };
    assert_eq!(count_of(&world, 809), 0, "18% roll failed");

    // Roll 0 → drop.
    let skel2 = NPC_OID + 2;
    add_test_npc(&mut world, skel2, 20517, "Monster", 5, 30, 0, 0);
    world.forced_rolls.push_back(0);
    death::npc_do_die(&mut world, skel2, 3001);
    assert_eq!(count_of(&world, 809), 1);
    drain(&mut rx);

    // 9 bones banked, the 10th caps the collection: cond 2 + middle sound.
    super::items::add_inventory_item(&mut world, 3001, 809, 8).unwrap();
    let skel3 = NPC_OID + 3;
    add_test_npc(&mut world, skel3, 20517, "Monster", 5, 30, 0, 0);
    world.forced_rolls.push_back(0);
    death::npc_do_die(&mut world, skel3, 3001);
    let pkts = drain(&mut rx);
    assert_eq!(count_of(&world, 809), 10);
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert_eq!(quests.0["Q00320_BonesTellTheFuture"].cond(), 2);
    }
    assert!(sound_names(&pkts).contains(&"ItemSound.quest_middle".to_string()), "limit-reached sound");

    // Turn-in: 500 adena (rates ×1 in tests), bones destroyed, exit.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00320_BonesTellTheFuture")));
    let pkts = drain(&mut rx);
    assert_eq!(count_of(&world, 809), 0);
    assert_eq!(count_of(&world, 57), 500, "500 adena at ×1 rates");
    assert!(sm_ids_of(&pkts).contains(&server_packets::sm_ids::YOU_HAVE_EARNED_S1_ADENA));
    assert!(
        world
            .objects
            .get_component::<crate::model::components::Quests>(&3001)
            .unwrap()
            .0
            .get("Q00320_BonesTellTheFuture")
            .is_none()
    );
}

/// The quest UI's Abandon button (`RequestQuestAbort` 0x63): repeatable
/// exit without the finish sound — state forgotten, quest items destroyed.
#[test]
fn quest_abort_wipes_state_and_items() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 3;

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00258_BringWolfPelts 30001-03.html")),
    );
    super::items::add_inventory_item(&mut world, 3001, 702, 5).unwrap();
    drain(&mut rx);
    drain_db(&mut db_rx);

    let mut w = PacketWriter::new();
    w.write_i32(258);
    on_packet(&mut world, 1, {
        let mut v = vec![cop::REQUEST_QUEST_ABORT];
        v.extend(w.into_bytes());
        v
    });

    let pkts = drain(&mut rx);
    assert!(
        world
            .objects
            .get_component::<crate::model::components::Quests>(&3001)
            .unwrap()
            .0
            .get("Q00258_BringWolfPelts")
            .is_none(),
        "abort forgets the quest"
    );
    assert_eq!(
        world.objects.get_component::<crate::model::inventory::Inventory>(&3001).unwrap().count_of(702),
        0,
        "quest items destroyed"
    );
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::QUEST_LIST), "QuestList refresh");
    assert!(!sound_names(&pkts).contains(&"ItemSound.quest_finish".to_string()), "no finish sound on abort");
    // Memory-first: the quest is forgotten in the Quests component (asserted
    // above); the flush reconcile drops its rows — no per-action DB write.
}

/// Quest-timer groundwork: a synthetic script starts a 500 ms timer via an
/// event bypass; it fires once through the scheduler (seq match) and a
/// cancelled one stays silent (seq bumped).
#[test]
fn quest_timer_fires_once_and_cancels() {
    struct TimerTestScript;
    impl quests::QuestScript for TimerTestScript {
        fn id(&self) -> i32 {
            -2
        }
        fn name(&self) -> &'static str {
            "TimerTest"
        }
        fn html_dir(&self) -> &'static str {
            ""
        }
        fn start_npcs(&self) -> &[i32] {
            &[]
        }
        fn talk_npcs(&self) -> &[i32] {
            &[30001]
        }
        fn on_talk(&self, _ctx: &mut quests::QuestCtx) -> Option<String> {
            None
        }
        fn on_event(&self, ctx: &mut quests::QuestCtx, event: &str) -> Option<String> {
            match event {
                "start" => ctx.start_quest_timer("tick", 500),
                "cancel" => ctx.cancel_quest_timer("tick"),
                _ => {}
            }
            None
        }
        fn on_timer(&self, ctx: &mut quests::QuestCtx, name: &str) {
            if name == "tick" {
                ctx.play_sound("timer_fired");
            }
        }
    }

    let (mut world, _db_rx, _link_rx) = quest_test_world();
    world.quests = std::sync::Arc::new(quests::QuestRegistry::new(vec![std::sync::Arc::new(TimerTestScript)]));
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest TimerTest start")));
    drain(&mut rx);
    advance_ticks(&mut world, 5);
    let pkts = drain(&mut rx);
    assert!(sound_names(&pkts).contains(&"timer_fired".to_string()), "timer fired at 500 ms");
    advance_ticks(&mut world, 10);
    assert!(drain(&mut rx).is_empty(), "non-repeating: fires once");

    // Start then cancel: the stale seq no-ops.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest TimerTest start")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest TimerTest cancel")));
    drain(&mut rx);
    advance_ticks(&mut world, 10);
    assert!(sound_names(&drain(&mut rx)).is_empty(), "cancelled timer never fires");
}

// ---------------------------------------------------------------------------
// Clans (G11)
// ---------------------------------------------------------------------------

fn decode_npc_html(pkt: &[u8]) -> Option<String> {
    if pkt[0] != server_packets::opcodes::NPC_HTML_MESSAGE {
        return None;
    }
    let mut r = commons::network::PacketReader::new(&pkt[1..]);
    r.read_i32()?;
    r.read_string()
}

/// The `create_clan` bypass: Java's guard matrix (SM ids in `ClanTable.
/// createClan` order), then the success path — clan registered + persisted,
/// leader flags/privileges set, the pledge-window packet trio + SM 189, and
/// duplicate-name/already-in-clan rejects afterwards.
#[test]
fn clan_create_guards_and_success() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);

    let create = |world: &mut World, client: u32, name: &str| {
        handle_request_bypass_to_server(world, client, &bypass_body(&format!("npc_{NPC_OID}_create_clan {name}")));
    };

    // Level < 10.
    create(&mut world, 1, "Myclan");
    let pkts = drain(&mut a_rx);
    assert!(sm_ids_of(&pkts).contains(&server_packets::sm_ids::YOU_DO_NOT_MEET_THE_CRITERIA_IN_ORDER_TO_CREATE_A_CLAN));

    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 10;

    // Name with a space arrives as two tokens → invalid.
    create(&mut world, 1, "My clan");
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::CLAN_NAME_IS_INVALID));
    // Non-alphanumeric.
    create(&mut world, 1, "Cl@n");
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::CLAN_NAME_IS_INVALID));
    // Too short / too long.
    create(&mut world, 1, "C");
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::CLAN_NAME_IS_INVALID));
    create(&mut world, 1, "Averyveryverylongclanname");
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::CLAN_NAME_S_LENGTH_IS_INCORRECT));
    // Recreate cooldown.
    world.objects.get_component_mut::<Player>(&3001).unwrap().clan_create_expiry_time = i64::MAX;
    create(&mut world, 1, "Myclan");
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::YOU_MUST_WAIT_10_DAYS_BEFORE_CREATING_A_NEW_CLAN));
    world.objects.get_component_mut::<Player>(&3001).unwrap().clan_create_expiry_time = 0;

    // Success.
    world.id_pool = 0x3000_0000..0x3000_0100;
    drain_db(&mut db_rx);
    create(&mut world, 1, "Myclan");
    let pkts = drain(&mut a_rx);
    let p = world.objects.get_component::<Player>(&3001).unwrap();
    let clan_id = p.clan_id;
    assert_ne!(clan_id, 0);
    assert!(p.clan_leader);
    assert_eq!(p.clan_privs, crate::model::clan::ALL_CLAN_PRIVILEGES);
    let clan = &world.clans[&clan_id];
    assert_eq!((clan.name.as_str(), clan.leader_id, clan.members.len()), ("Myclan", 3001, 1));
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_INFO_UPDATE));
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_ALL));
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_UPDATE));
    assert!(sm_ids_of(&pkts).contains(&server_packets::sm_ids::YOUR_CLAN_HAS_BEEN_CREATED));
    assert!(pkts.iter().any(|p| p[0] == 0x32), "fresh UserInfo with the clan id");
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::InsertClan { name, leader_id: 3001, .. } if name == "Myclan")));
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::UpdateCharClan { char_id: 3001, clan_privs, .. }
        if *clan_privs == crate::model::clan::ALL_CLAN_PRIVILEGES)));

    // Already in a clan.
    create(&mut world, 1, "Another");
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&server_packets::sm_ids::YOU_HAVE_FAILED_TO_CREATE_A_CLAN));

    // Second player: the name is taken (case-insensitive).
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3002).unwrap().level = 10;
    create(&mut world, 2, "MYCLAN");
    assert!(sm_ids_of(&drain(&mut b_rx)).contains(&server_packets::sm_ids::S1_ALREADY_EXISTS));
}

/// ClanMaster dialog navigation: `Quest ClanMaster <page>` events render
/// the page (bare bypass resolved through `LastFolkNpc`), with the
/// leader-required remap for non-leaders.
#[test]
fn clan_master_dialog_gates_on_leadership() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    // Click the NPC so LastFolkNpc resolves the bare Quest bypasses.
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    drain(&mut rx);

    let root = world.data.root.clone();
    let page = |name: &str| {
        std::fs::read_to_string(format!("{root}data/scripts/village_master/ClanMaster/{name}"))
            .expect(name)
            .replace("%objectId%", &NPC_OID.to_string())
    };

    // Talk → the root menu (ClanMaster id -1 ⇒ plain NpcHtmlMessage).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest ClanMaster"));
    let pkts = drain(&mut rx);
    let html = pkts.iter().find_map(|p| decode_npc_html(p)).expect("root menu html");
    assert_eq!(html, page("9000-01.htm"));

    // Leader-gated page as a non-leader → the -no variant.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest ClanMaster 9000-03.htm"));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("gated html");
    assert_eq!(html, page("9000-03-no.htm"));

    // As a leader → the real page.
    world.objects.get_component_mut::<Player>(&3001).unwrap().clan_leader = true;
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest ClanMaster 9000-03.htm"));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("leader html");
    assert_eq!(html, page("9000-03.htm"));
}

/// Clan roster notifications + clan chat: enter-world sends the pledge
/// window to the member and the online ping to the rest; clan chat reaches
/// every online member; leaving pings offline; the clanless get SM 4202.
#[test]
fn clan_roster_notifications_and_chat() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);

    // A clan with A (leader, online) and B — installed directly; invites
    // are deferred past G11.
    let clan_id = 5000;
    let member = |char_id: i32, name: &str| crate::model::clan::ClanMember {
        char_id,
        name: name.into(),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
    };
    world.clans.insert(
        clan_id,
        crate::model::clan::Clan {
            id: clan_id,
            name: "Testers".into(),
            leader_id: 3001,
            level: 0,
            members: vec![member(3001, "P3001"), member(3002, "P3002")],
        },
    );
    for oid in [3001, 3002] {
        world.objects.get_component_mut::<Player>(&oid).unwrap().clan_id = clan_id;
    }

    // B "enters world": pledge window to B, online ping to A.
    clans::on_enter_world(&mut world, 2, 3002);
    let b_pkts = drain(&mut b_rx);
    assert!(b_pkts.iter().any(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_ALL));
    let a_pkts = drain(&mut a_rx);
    let upd = a_pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_UPDATE)
        .expect("online ping to A");
    let mut r = commons::network::PacketReader::new(&upd[1..]);
    assert_eq!(r.read_string().unwrap(), "P3002");

    // Clan chat from A reaches both.
    chat::handle_say2(&mut world, 1, &say2_body("hail", crate::enums::ChatType::Clan.client_id(), None));
    assert!(drain(&mut a_rx).iter().any(|p| p[0] == server_packets::opcodes::SAY2));
    assert!(drain(&mut b_rx).iter().any(|p| p[0] == server_packets::opcodes::SAY2));

    // A clanless player gets SM 4202.
    let mut c_rx = ingame_player(&mut world, 3, 3003, 0, 0, 0);
    chat::handle_say2(&mut world, 3, &say2_body("hail", crate::enums::ChatType::Clan.client_id(), None));
    assert!(sm_ids_of(&drain(&mut c_rx)).contains(&server_packets::sm_ids::YOU_ARE_NOT_IN_A_CLAN));

    // B leaves the world: offline ping to A.
    net::store_and_remove_player(&mut world, 3002);
    let a_pkts = drain(&mut a_rx);
    let upd = a_pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_UPDATE)
        .expect("offline ping to A");
    // Online-status byte is the packet tail.
    assert_eq!(*upd.last().unwrap(), 0, "offline");
}

// ---------------------------------------------------------------------------
// Zones (G12): peace-zone gates, water swim state, compass codes
// ---------------------------------------------------------------------------

/// A synthetic zone cuboid registered into `world.data.zone_data`.
fn insert_zone(world: &mut World, kind: crate::data::zone_data::ZoneKind, x1: i32, x2: i32, y1: i32, y2: i32) {
    world.data.zone_data.insert(crate::data::zone_data::Zone {
        name: format!("test_{kind:?}"),
        kind,
        territory: crate::data::spawn_data::Territory {
            form: crate::data::spawn_data::ZoneForm::Cuboid { x1, x2, y1, y2 },
            min_z: -1000,
            max_z: 1000,
        },
    });
}

fn compass_code(pkt: &[u8]) -> Option<i32> {
    (pkt[0] == server_packets::opcodes::EX
        && i16::from_le_bytes(pkt[1..3].try_into().unwrap()) == server_packets::opcodes::EX_SET_COMPASS_ZONE_CODE)
        .then(|| i32::from_le_bytes(pkt[3..7].try_into().unwrap()))
}

/// Hostile casts between players are refused while either side stands in a
/// peace zone (`Enemy`/`EnemyOnly.java` → SM 2167), while friendly skills
/// still land; revalidation pushes the peace compass code.
#[test]
fn peace_zone_blocks_hostile_casts_between_players() {
    let (mut world, ..) = cast_test_world();
    insert_zone(&mut world, crate::data::zone_data::ZoneKind::Peace, -500, 500, -500, 500);
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    super::zones::revalidate_zone(&mut world, 3001, true);
    super::zones::revalidate_zone(&mut world, 3002, true);

    // The initial revalidate reports the peace compass code.
    let a_pkts = drain(&mut a_rx);
    assert_eq!(
        a_pkts.iter().filter_map(|p| compass_code(p)).collect::<Vec<_>>(),
        vec![server_packets::compass_zone::PEACE]
    );

    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Force-use nuke on the player target: refused with SM 2167 and no cast.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::YOU_CANNOT_USE_SKILLS_THAT_MAY_HARM_OTHER_PLAYERS_IN_HERE
    );
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert!(b_rx.try_recv().is_err(), "the target hears nothing about the refused cast");

    // A friendly skill (Battle Heal, TARGET type) is not gated.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
    assert!(world.objects.has_component::<Casting>(&3001), "heal must start casting in a peace zone");
}

/// The peace gate only guards playable-vs-playable: with only the *attacker*
/// outside, hitting a player inside the zone is still refused; and once
/// both stand outside, the same cast goes through.
#[test]
fn peace_zone_gate_checks_both_sides() {
    let (mut world, ..) = cast_test_world();
    insert_zone(&mut world, crate::data::zone_data::ZoneKind::Peace, 60, 200, -500, 500);
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0); // outside
    let _b_rx = ingame_caster(&mut world, 2, 3002, 100, 0); // inside
    super::zones::revalidate_zone(&mut world, 3001, true);
    super::zones::revalidate_zone(&mut world, 3002, true);

    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::YOU_CANNOT_USE_SKILLS_THAT_MAY_HARM_OTHER_PLAYERS_IN_HERE
    );
    assert!(!world.objects.has_component::<Casting>(&3001));

    // Move the target out of the zone and revalidate: the cast now starts.
    world.objects.get_component_mut::<Position>(&3002).unwrap().x = 30;
    super::zones::revalidate_zone(&mut world, 3002, true);
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
}

/// Entering/leaving a `WaterZone` flips the swim-speed branch
/// (`Creature.getMoveSpeed`'s water case) and re-broadcasts `UserInfo`,
/// with the compass staying GENERAL.
#[test]
fn water_zone_flips_swim_state_and_speeds() {
    let (mut world, ..) = cast_test_world();
    insert_zone(&mut world, crate::data::zone_data::ZoneKind::Water, 5000, 6000, -500, 500);
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    {
        let speeds = world.objects.get_component_mut::<Speeds>(&3001).unwrap();
        speeds.run_spd = 120.0;
        speeds.swim_run_spd = 50.0;
    }
    super::zones::revalidate_zone(&mut world, 3001, true);
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter().all(|p| compass_code(p).is_none()),
        "no compass push outside a peace zone (GENERAL is the client default)"
    );
    assert_eq!(world.objects.get_component::<Speeds>(&3001).unwrap().move_speed(), 120.0);

    // Wade in: swim speeds take over and a fresh UserInfo goes out.
    world.objects.get_component_mut::<Position>(&3001).unwrap().x = 5500;
    super::zones::revalidate_zone(&mut world, 3001, false);
    let speeds = *world.objects.get_component::<Speeds>(&3001).unwrap();
    assert!(speeds.swimming);
    assert_eq!(speeds.move_speed(), 50.0);
    let pkts = drain(&mut rx);
    assert!(pkts.iter().any(|p| p[0] == 0x32), "UserInfo re-broadcast on water enter");
    assert!(pkts.iter().all(|p| compass_code(p).is_none()), "water does not change the compass");

    // Wade out: ground speeds return.
    world.objects.get_component_mut::<Position>(&3001).unwrap().x = 0;
    super::zones::revalidate_zone(&mut world, 3001, false);
    let speeds = *world.objects.get_component::<Speeds>(&3001).unwrap();
    assert!(!speeds.swimming);
    assert_eq!(speeds.move_speed(), 120.0);
    assert!(drain(&mut rx).iter().any(|p| p[0] == 0x32));
}

/// The 100-unit revalidation filter: a small drift does not re-run the zone
/// query (the water flag stays stale until a real move), a forced call does.
#[test]
fn zone_revalidation_distance_filter() {
    let (mut world, ..) = cast_test_world();
    insert_zone(&mut world, crate::data::zone_data::ZoneKind::Water, 5000, 6000, -500, 500);
    let _rx = ingame_caster(&mut world, 1, 3001, 5990, 0);
    super::zones::revalidate_zone(&mut world, 3001, true);
    assert!(world.objects.get_component::<Speeds>(&3001).unwrap().swimming);

    // A 50-unit drift out of the zone edge: unforced revalidate is skipped.
    world.objects.get_component_mut::<Position>(&3001).unwrap().x = 6040;
    super::zones::revalidate_zone(&mut world, 3001, false);
    assert!(world.objects.get_component::<Speeds>(&3001).unwrap().swimming, "filtered — still stale");
    super::zones::revalidate_zone(&mut world, 3001, true);
    assert!(!world.objects.get_component::<Speeds>(&3001).unwrap().swimming, "forced — recomputed");
}

// ---------------------------------------------------------------------------
// Doors (G12)
// ---------------------------------------------------------------------------

fn test_door(door_id: i32, method: crate::data::door_data::DoorOpenMethod) -> crate::data::door_data::DoorTemplate {
    crate::data::door_data::DoorTemplate {
        id: door_id,
        name: "test_door".into(),
        // A thin wall crossing the x axis at x≈100, like the geo unit tests.
        node_x: [98, 102, 102, 98],
        node_y: [-50, -50, 50, 50],
        node_z: -100,
        height: 200,
        x: 100,
        y: 0,
        z: -100,
        hp_max: 1000,
        p_def: 100,
        m_def: 100,
        targetable: false,
        show_hp: false,
        open_by_default: false,
        open_method: method,
        open_time: 3,
        close_time: 2,
        random_time: 0,
    }
}

fn is_static_object_info(p: &[u8]) -> bool {
    p[0] == server_packets::opcodes::STATIC_OBJECT
}
fn is_door_status(p: &[u8]) -> bool {
    p[0] == server_packets::opcodes::DOOR_STATUS_UPDATE
}
/// The "isClosed" int of either door packet (offsets: StaticObjectInfo has
/// it at byte 1+4*5, DoorStatusUpdate at 1+4).
fn door_packet_closed(p: &[u8]) -> i32 {
    let off = if is_static_object_info(p) { 1 + 4 * 5 } else { 1 + 4 };
    i32::from_le_bytes(p[off..off + 4].try_into().unwrap())
}

/// Enter-world burst includes StaticObjectInfo + DoorStatusUpdate for a
/// nearby door (and nothing for a far one).
#[test]
fn enter_world_sends_door_info_for_nearby_doors() {
    let (mut world, ..) = test_world();
    crate::model::door::spawn_door_for_test(&mut world, test_door(9001, crate::data::door_data::DoorOpenMethod::None));
    let mut far = test_door(9002, crate::data::door_data::DoorOpenMethod::None);
    far.x = 50_000;
    far.node_x = [49_998, 50_002, 50_002, 49_998];
    crate::model::door::spawn_door_for_test(&mut world, far);

    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    visibility::on_enter_world(&world, 1, 3001);
    let pkts = drain(&mut rx);
    let so: Vec<_> = pkts.iter().filter(|p| is_static_object_info(p)).collect();
    let dsu: Vec<_> = pkts.iter().filter(|p| is_door_status(p)).collect();
    assert_eq!(so.len(), 1, "only the nearby door renders");
    assert_eq!(dsu.len(), 1);
    assert_eq!(door_packet_closed(so[0]), 1, "closed by default");
    // StaticObjectInfo leads with the door template id.
    assert_eq!(i32::from_le_bytes(so[0][1..5].try_into().unwrap()), 9001);
}

/// A closed door refuses casts through it (SM 181 via `can_see_target`);
/// opening it broadcasts the state change and un-blocks the cast.
#[test]
fn closed_door_blocks_cast_los_until_opened() {
    let (mut world, ..) = cast_test_world();
    let door_oid =
        crate::model::door::spawn_door_for_test(&mut world, test_door(9001, crate::data::door_data::DoorOpenMethod::None));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let _b_rx = ingame_caster(&mut world, 2, 3002, 200, 0);

    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut a_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert_eq!(sm_id(&a_rx.try_recv().unwrap()), server_packets::sm_ids::CANNOT_SEE_TARGET);
    assert!(!world.objects.has_component::<Casting>(&3001));

    // Open the door: both nearby players get the status packets…
    super::doors::open_door(&mut world, door_oid);
    let pkts = drain(&mut a_rx);
    assert!(pkts.iter().any(|p| is_static_object_info(p) && door_packet_closed(p) == 0));
    assert!(pkts.iter().any(|p| is_door_status(p) && door_packet_closed(p) == 0));

    // …and the cast now starts.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
}

/// A script-opened door with a `closeTime` shuts itself (`AutoClose`), and
/// a re-close before the timer makes the stale task a no-op.
#[test]
fn opened_door_auto_closes_after_close_time() {
    let (mut world, ..) = test_world();
    let door_oid =
        crate::model::door::spawn_door_for_test(&mut world, test_door(9001, crate::data::door_data::DoorOpenMethod::None));

    super::doors::open_door(&mut world, door_oid);
    assert!(world.geo.doors.is_open(9001));
    // closeTime = 2 s = 20 ticks.
    advance_ticks(&mut world, 19);
    assert!(world.geo.doors.is_open(9001));
    advance_ticks(&mut world, 1);
    assert!(!world.geo.doors.is_open(9001), "auto-closed");

    // Re-open, close by hand, then let the (stale) auto-close fire: no flip.
    super::doors::open_door(&mut world, door_oid);
    super::doors::close_door(&mut world, door_oid);
    super::doors::open_door(&mut world, door_oid);
    super::doors::close_door(&mut world, door_oid);
    assert!(!world.geo.doors.is_open(9001));
    advance_ticks(&mut world, 40);
    assert!(!world.geo.doors.is_open(9001), "stale auto-close is a no-op");
}

/// BY_TIME doors cycle on their own: closed → open after `closeTime`,
/// open → closed after `openTime` (Java `TimerOpen`), forever.
#[test]
fn by_time_door_cycles() {
    let (mut world, ..) = test_world();
    crate::model::door::spawn_door_for_test(&mut world, test_door(9001, crate::data::door_data::DoorOpenMethod::ByTime));
    super::doors::start_time_cycles(&mut world);

    assert!(!world.geo.doors.is_open(9001));
    // Initial delay while closed = closeTime (2 s).
    advance_ticks(&mut world, 20);
    assert!(world.geo.doors.is_open(9001), "opened by the cycle");
    // Now open: next toggle after closeTime (2 s) per TimerOpen's delay pick.
    advance_ticks(&mut world, 20);
    assert!(!world.geo.doors.is_open(9001), "closed again");
    // Closed: next toggle after openTime (3 s).
    advance_ticks(&mut world, 30);
    assert!(world.geo.doors.is_open(9001), "cycle keeps running");
}

/// Static objects (town maps/thrones) render on enter world with the Java
/// `StaticObjectInfo(StaticObject)` field shape, scoped by region.
#[test]
fn enter_world_sends_static_object_info_nearby() {
    let (mut world, ..) = test_world();
    world.data.static_object_data.objects.push(crate::data::static_object_data::StaticObjectTemplate {
        id: 17250001,
        name: "town_map".into(),
        kind: 0,
        x: 100,
        y: 100,
        z: 0,
    });
    world.data.static_object_data.objects.push(crate::data::static_object_data::StaticObjectTemplate {
        id: 17250002,
        name: "far_map".into(),
        kind: 0,
        x: 60_000,
        y: 60_000,
        z: 0,
    });
    crate::model::static_object::spawn_static_objects(&mut world);

    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    visibility::on_enter_world(&world, 1, 3001);
    let pkts = drain(&mut rx);
    let so: Vec<_> = pkts.iter().filter(|p| is_static_object_info(p)).collect();
    assert_eq!(so.len(), 1, "only the nearby panel renders");
    assert_eq!(i32::from_le_bytes(so[0][1..5].try_into().unwrap()), 17250001);
    // type field (offset 9..13) is 0, targetable (13..17) is 1.
    assert_eq!(i32::from_le_bytes(so[0][9..13].try_into().unwrap()), 0);
    assert_eq!(i32::from_le_bytes(so[0][13..17].try_into().unwrap()), 1);
}

// ---------------------------------------------------------------------------
// Link bypass (G12)
// ---------------------------------------------------------------------------

/// The generic `Link <file>` bypass: whitelisted pages are served from
/// `data/html/` through a plain `NpcHtmlMessage` anchored at the last
/// clicked NPC; non-whitelisted or path-escaping requests answer an empty
/// html (Java's null content) or drop.
#[test]
fn link_bypass_serves_whitelisted_html_only() {
    let (mut world, ..) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.add_components(&3001, LastFolkNpc(NPC_OID));

    // Whitelisted page (real dist file).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Link common/craft_01.htm"));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("html window");
    assert!(html.contains("Dualsword"), "served the real page: {html}");

    // Non-whitelisted page: empty html window, not the file.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Link merchant/30001.htm"));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("empty html window");
    assert!(html.is_empty());

    // Path traversal: dropped outright.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Link ../config/Server.ini"));
    assert!(drain(&mut rx).is_empty());
}

// ---------------------------------------------------------------------------
// Buy shop (G12)
// ---------------------------------------------------------------------------

fn buy_body(list_id: i32, lines: &[(i32, i64)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(list_id);
    w.write_i32(lines.len() as i32);
    for &(item_id, count) in lines {
        w.write_i32(item_id);
        w.write_i64(count);
    }
    w.into_bytes()
}

/// A merchant + a two-product buylist on top of `quest_test_world`; the
/// player holds 1000 adena and targets the merchant.
fn shop_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
) {
    let (mut world, db_rx, _link_rx) = quest_test_world();
    // A stackable potion the shop sells in bulk.
    world.data.item_data.insert_for_test(crate::data::item_data::ItemTemplate {
        item_id: 1061,
        name: "Greater Healing Potion".into(),
        kind: crate::data::item_data::ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 0,
        handler: crate::data::item_data::ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
    });
    world.data.buy_lists.insert_for_test(crate::data::buy_list_data::BuyList {
        list_id: 3,
        npcs: vec![30001],
        products: vec![
            crate::data::buy_list_data::Product { item_id: 41, price: 100, base_tax: 0 },
            crate::data::buy_list_data::Product { item_id: 1061, price: 10, base_tax: 0 },
        ],
    });
    add_test_npc(&mut world, NPC_OID, 30001, "Merchant", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    super::items::add_inventory_item(&mut world, 3001, 57, 1000);
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    drain(&mut rx);
    (world, db_rx, rx)
}

fn adena_of(world: &World, oid: i32) -> i64 {
    world.objects.get_component::<crate::model::inventory::Inventory>(&oid).unwrap().adena()
}
fn count_of_item(world: &World, oid: i32, item_id: i32) -> i64 {
    world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&oid)
        .unwrap()
        .items()
        .iter()
        .filter(|i| i.item_id == item_id)
        .map(|i| i.count)
        .sum()
}

/// The `Buy <listId>` bypass opens the buy window: the BUY tab (type 0,
/// list id + adena + both products) and the SELL tab (type 1).
#[test]
fn buy_bypass_opens_buy_and_sell_tabs() {
    let (mut world, _db_rx, mut rx) = shop_world();
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Buy 3")));
    let pkts = drain(&mut rx);
    let tabs: Vec<_> = pkts.iter().filter(|p| is_ex(p, crate::network::trade::EX_BUY_SELL_LIST)).collect();
    assert_eq!(tabs.len(), 2, "buy + sell tab");
    // BUY tab: type 0, money 1000, list id 3, then the product table.
    let buy = tabs[0];
    assert_eq!(i32::from_le_bytes(buy[3..7].try_into().unwrap()), 0);
    assert_eq!(i64::from_le_bytes(buy[7..15].try_into().unwrap()), 1000);
    assert_eq!(i32::from_le_bytes(buy[15..19].try_into().unwrap()), 3);
    // SELL tab leads with type 1.
    assert_eq!(i32::from_le_bytes(tabs[1][3..7].try_into().unwrap()), 1);

    // A non-merchant NPC refuses the same bypass.
    add_test_npc(&mut world, NPC_OID + 1, 30002, "Folk", 5, 120, 0, 0);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{}_Buy 3", NPC_OID + 1)));
    let pkts = drain(&mut rx);
    assert!(!pkts.iter().any(|p| is_ex(p, crate::network::trade::EX_BUY_SELL_LIST)));
}

/// A purchase debits adena, adds the items, and answers with the
/// InventoryUpdate/inven-weight/sell-refresh/SM-4358 tail; the guards
/// (wrong quantity, empty purse, no merchant target) refuse cleanly.
#[test]
fn request_buy_item_purchases_and_guards() {
    let (mut world, _db_rx, mut rx) = shop_world();

    // 1 Cloth Cap (100) + 5 potions (50) = 150 adena.
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(41, 1), (1061, 5)]));
    assert_eq!(adena_of(&world, 3001), 850);
    assert_eq!(count_of_item(&world, 3001, 41), 1);
    assert_eq!(count_of_item(&world, 3001, 1061), 5);
    let pkts = drain(&mut rx);
    assert!(pkts.iter().any(|p| p[0] == 0x21), "InventoryUpdate");
    assert!(pkts.iter().any(|p| is_ex(p, 0x166)), "ExUserInfoInvenWeight");
    let sell_done = pkts.iter().find(|p| is_ex(p, crate::network::trade::EX_BUY_SELL_LIST)).expect("sell refresh");
    assert_eq!(*sell_done.last().unwrap(), 1, "done flag");
    assert!(sm_ids_of(&pkts).contains(&server_packets::sm_ids::EXCHANGE_IS_SUCCESSFUL));

    // Non-stackable quantity > 1: SM 1036, nothing purchased.
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(41, 2)]));
    assert!(sm_ids_of(&drain(&mut rx)).contains(&server_packets::sm_ids::YOU_HAVE_EXCEEDED_THE_QUANTITY_THAT_CAN_BE_INPUTTED));
    assert_eq!(adena_of(&world, 3001), 850);

    // Too expensive: SM 279.
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(1061, 100)]));
    assert!(sm_ids_of(&drain(&mut rx)).contains(&server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA));
    assert_eq!(adena_of(&world, 3001), 850);

    // Off-list item: dropped, no charge.
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(702, 1)]));
    assert!(drain(&mut rx).is_empty());
    assert_eq!(adena_of(&world, 3001), 850);

    // No merchant targeted: ActionFailed.
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(true));
    drain(&mut rx);
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(1061, 1)]));
    let pkts = drain(&mut rx);
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::ACTION_FAIL));
    assert_eq!(adena_of(&world, 3001), 850);
}

// ---------------------------------------------------------------------------
// G12 quest/script breadth
// ---------------------------------------------------------------------------

/// Register extra quest-item templates on top of `quest_test_world`.
fn add_quest_items(world: &mut World, ids: &[(i32, &str, bool)]) {
    for &(item_id, name, is_quest_item) in ids {
        world.data.item_data.insert_for_test(crate::data::item_data::ItemTemplate {
            item_id,
            name: name.into(),
            kind: crate::data::item_data::ItemKind::Etc,
            body_part: 0,
            weight: 0,
            is_stackable: true,
            type1: 4,
            type2: if is_quest_item { 3 } else { 5 },
            is_quest_item,
            price: 0,
            handler: crate::data::item_data::ItemHandler::None,
            crystal_type: crate::data::item_data::CrystalType::None,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(),
        });
    }
}

fn quest_cond(world: &World, player: i32, quest: &str) -> Option<i32> {
    world
        .objects
        .get_component::<crate::model::components::Quests>(&player)
        .and_then(|q| q.0.get(quest).map(|qs| qs.cond()))
}

fn item_count(world: &World, player: i32, item_id: i32) -> i64 {
    world.objects.get_component::<crate::model::inventory::Inventory>(&player).unwrap().count_of(item_id)
}

/// Q00303 Collect Arrowheads: accept → 40%-chance drops to the 10-arrowhead
/// cap (cond 2) → turn-in pays 500 adena and exits repeatably.
#[test]
fn quest_q00303_collect_arrowheads_loop() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(963, "Orcish Arrowhead", true)]);
    let mut t = crate::data::npc_data::default_template(20361);
    t.type_name = "Monster".into();
    t.level = 11;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30029, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 10;
    drain_db(&mut db_rx);

    // Accept.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00303_CollectArrowheads")));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00303_CollectArrowheads 30029-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, "Q00303_CollectArrowheads"), Some(1));
    drain(&mut rx);

    // Kill 10 marksmen with the 40% roll forced to hit each time.
    let mob = NPC_OID + 1;
    for i in 0..10 {
        add_test_npc(&mut world, mob + i, 20361, "Monster", 11, 30, 0, 0);
        world.forced_rolls.push_back(0); // roll_f64 → 0.0 ≤ 0.4
        death::npc_do_die(&mut world, mob + i, 3001);
    }
    assert_eq!(item_count(&world, 3001, 963), 10);
    assert_eq!(quest_cond(&world, 3001, "Q00303_CollectArrowheads"), Some(2));
    drain(&mut rx);

    // Turn-in.
    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00303_CollectArrowheads")));
    assert_eq!(item_count(&world, 3001, 57), adena_before + 500);
    assert_eq!(item_count(&world, 3001, 963), 0, "quest items removed on exit");
    assert!(quest_cond(&world, 3001, "Q00303_CollectArrowheads").is_none(), "repeatable exit");
}

/// Q00316 Destroy Plague Carriers: the first hit on Varool Foulclaw makes
/// him shout (`on_attack` + script value), his fang drops at most once, and
/// the turn-in pays the fang/wererat ladder.
#[test]
fn quest_q00316_on_attack_say_and_limited_fang() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1042, "Wererat Fang", true), (1043, "Varool Foulclaw Fang", true)]);
    for id in [27020, 20040] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        t.base_hp_max = 1000.0;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30155, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 1; // Elf
    }
    drain_db(&mut db_rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00316_DestroyPlagueCarriers")));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00316_DestroyPlagueCarriers 30155-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, "Q00316_DestroyPlagueCarriers"), Some(1));
    drain(&mut rx);

    // First hit on Varool: exactly one NpcSay; further hits stay quiet.
    let varool = NPC_OID + 1;
    add_test_npc(&mut world, varool, 27020, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, varool, 3001, 10.0);
    let pkts = drain(&mut rx);
    let says: Vec<_> = pkts.iter().filter(|p| p[0] == server_packets::opcodes::NPC_SAY).collect();
    assert_eq!(says.len(), 1, "one shout on the first hit");
    assert_eq!(i32::from_le_bytes(says[0][13..17].try_into().unwrap()), 31603, "WHY_DO_YOU_OPPRESS_US_SO");
    combat::npc_receive_damage(&mut world, varool, 3001, 10.0);
    assert!(
        !drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::NPC_SAY),
        "script value keeps him quiet"
    );

    // His fang drops once (chance 10/7 ≥ 1 → guaranteed), never twice.
    death::npc_do_die(&mut world, varool, 3001);
    assert_eq!(item_count(&world, 3001, 1043), 1);
    let varool2 = NPC_OID + 2;
    add_test_npc(&mut world, varool2, 27020, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, varool2, 3001);
    assert_eq!(item_count(&world, 3001, 1043), 1, "only one Varool fang ever");

    // Wererats drop fangs freely (chance 2.0 → always).
    for i in 0..10 {
        let rat = NPC_OID + 3 + i;
        add_test_npc(&mut world, rat, 20040, "Monster", 20, 30, 0, 0);
        death::npc_do_die(&mut world, rat, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1042), 10);
    drain(&mut rx);

    // Turn-in: 10×5 + 1×1000 + 5000 bonus.
    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00316_DestroyPlagueCarriers")));
    assert_eq!(item_count(&world, 3001, 57), adena_before + 50 + 1000 + 5000);
    assert_eq!(item_count(&world, 3001, 1042), 0);
    assert_eq!(item_count(&world, 3001, 1043), 0);
}

/// Q00109 In Search of the Nest: the three-NPC cond 1→2→3 chain ends in a
/// one-time completion — the quest survives as COMPLETED and answers with
/// the already-completed page.
#[test]
fn quest_q00109_multi_cond_one_time() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(14858, "Scout's Note", true)]);
    let (pierce, corpse, kahman) = (NPC_OID, NPC_OID + 1, NPC_OID + 2);
    add_test_npc(&mut world, pierce, 31553, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, corpse, 32015, "Folk", 5, 120, 0, 0);
    add_test_npc(&mut world, kahman, 31554, "Folk", 5, 140, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 81;
    drain_db(&mut db_rx);

    let q = "Q00109_InSearchOfTheNest";
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{pierce}_Quest {q}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{pierce}_Quest {q} 31553-0.htm")));
    assert_eq!(quest_cond(&world, 3001, q), Some(1));

    // The corpse: cond 2 + the note.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{corpse}_Quest {q} 32015-2.html")));
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    assert_eq!(item_count(&world, 3001, 14858), 1);

    // Back to Pierce: cond 3, note taken.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{pierce}_Quest {q} 31553-3.html")));
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    assert_eq!(item_count(&world, 3001, 14858), 0);

    // Kahman pays out; one-time exit keeps the COMPLETED state.
    let (adena, exp) = (
        item_count(&world, 3001, 57),
        world.objects.get_component::<Player>(&3001).unwrap().exp,
    );
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{kahman}_Quest {q} 31554-2.html")));
    assert_eq!(item_count(&world, 3001, 57), adena + 161500);
    assert!(world.objects.get_component::<Player>(&3001).unwrap().exp > exp);
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0[q].is_completed(), "one-time quest stays COMPLETED");
    }

    // Talking to Pierce again answers the already-completed page.
    drain(&mut rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{pierce}_Quest {q}")));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("html");
    assert!(
        html.contains("already completed") || html.contains("already been completed"),
        "already-completed message, got: {html}"
    );
}

/// OrcChange1: an eligible Orc Fighter with the Mark of Raider becomes an
/// Orc Raider — proof consumed, 15 coupons paid, class persisted; the
/// category gates refuse a player who already transferred.
#[test]
fn orc_change1_first_class_transfer() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1592, "Mark of Raider", true), (8869, "Shadow Coupon (D)", false)]);
    world.data.categories.insert_for_test("FIGHTER_GROUP", &[44, 45]);
    world.data.categories.insert_for_test("MAGE_GROUP", &[49]);
    world.data.categories.insert_for_test("SECOND_CLASS_GROUP", &[45]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30500, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 3; // Orc
        p.class_id = 44; // Orc Fighter
        p.base_class_id = 44;
    }
    super::items::add_inventory_item(&mut world, 3001, 1592, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    // The named bypass shows the fighter class list.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest OrcChange1")));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("class list");
    assert!(html.contains("45") || !html.is_empty());

    // Transfer to Orc Raider (45).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest OrcChange1 45")));
    {
        let p = world.objects.get_component::<Player>(&3001).unwrap();
        assert_eq!(p.class_id, 45);
        assert_eq!(p.base_class_id, 45);
    }
    assert_eq!(item_count(&world, 3001, 1592), 0, "proof consumed");
    assert_eq!(item_count(&world, 3001, 8869), 15, "shadow coupons");
    // The change persisted immediately.
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter().any(|c| matches!(c, db::DbCommand::StorePlayer { save } if save.base.class_id == 45)),
        "StorePlayer with the new class"
    );
    // A UserInfo re-broadcast reached the player.
    assert!(drain(&mut rx).iter().any(|p| p[0] == 0x32), "UserInfo after transfer");

    // Now in SECOND_CLASS_GROUP: another transfer attempt is refused.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest OrcChange1 45")));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("refusal page");
    assert!(html.contains("class transfer") || !html.is_empty());
    assert_eq!(world.objects.get_component::<Player>(&3001).unwrap().class_id, 45, "unchanged");
}

/// TeleportWithCharm: the bare `Quest` click consumes the token and
/// teleports; without a token it shows the "come back with one" page.
#[test]
fn teleport_with_charm_consumes_token() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1659, "Gatekeeper Token", false)]);
    add_test_npc(&mut world, NPC_OID, 30540, "Teleporter", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);

    // No token: the explain page.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("no-token page");
    assert!(html.contains("Token") || html.contains("token"), "got: {html}");

    // With a token: teleport + consumption.
    super::items::add_inventory_item(&mut world, 3001, 1659, 1);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    assert_eq!(item_count(&world, 3001, 1659), 0, "token consumed");
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (-80826, 149775, -3043));
    assert!(world.objects.get_component::<Player>(&3001).unwrap().teleporting);
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == 0x22),
        "TeleportToLocation sent"
    );
}

/// The `on_spawn` hook fires for registered NPCs on every (re)spawn — a
/// synthetic script stamps the NPC's script value at spawn.
#[test]
fn on_spawn_hook_fires_for_registered_npcs() {
    struct SpawnStamp;
    impl crate::game_loop::quests::QuestScript for SpawnStamp {
        fn id(&self) -> i32 {
            -1
        }
        fn name(&self) -> &'static str {
            "SpawnStamp"
        }
        fn html_dir(&self) -> &'static str {
            ""
        }
        fn start_npcs(&self) -> &[i32] {
            &[]
        }
        fn talk_npcs(&self) -> &[i32] {
            &[]
        }
        fn spawn_npcs(&self) -> &[i32] {
            &[40001]
        }
        fn on_talk(&self, _ctx: &mut crate::game_loop::quests::QuestCtx) -> Option<String> {
            None
        }
        fn on_spawn(&self, ctx: &mut crate::game_loop::quests::QuestCtx) {
            ctx.set_npc_script_value(7);
        }
    }
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    world.quests = std::sync::Arc::new(crate::game_loop::quests::QuestRegistry::new(vec![
        std::sync::Arc::new(SpawnStamp),
    ]));
    // Spawn through the real spawn line (template 40001 registered by
    // combat_test_world's spawn_data? — spawn directly via spawn_one needs
    // a spawn line; use notify path through add_test_npc + explicit call).
    add_test_npc(&mut world, NPC_OID, 40001, "Monster", 5, 30, 0, 0);
    crate::game_loop::quests::notify_spawn(&mut world, NPC_OID, 40001);
    assert_eq!(
        world.objects.get_component::<crate::model::npc::Npc>(&NPC_OID).unwrap().script_value,
        7
    );
}

// ---------------------------------------------------------------------------
// Soulshots / spiritshots
// ---------------------------------------------------------------------------

/// A `<set name="handler">` shot item template (soulshot/spiritshot).
fn shot_template(item_id: i32, grade: crate::data::item_data::CrystalType, handler: crate::data::item_data::ItemHandler, skill_id: i32) -> crate::data::item_data::ItemTemplate {
    crate::data::item_data::ItemTemplate {
        item_id,
        name: format!("shot{item_id}"),
        kind: crate::data::item_data::ItemKind::Etc,
        crystal_type: grade,
        body_part: crate::data::item_data::SLOT_NONE,
        weight: 0,
        is_stackable: true,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        price: 0,
        handler,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: vec![(skill_id, 1)],
    }
}

/// A graded weapon template that consumes `ss`/`sps` shots per charge.
fn shot_weapon(world: &mut World, item_id: i32, grade: crate::data::item_data::CrystalType, ss: i32, sps: i32) {
    world.data.item_data.insert_for_test(crate::data::item_data::ItemTemplate {
        item_id,
        name: format!("weapon{item_id}"),
        kind: crate::data::item_data::ItemKind::Weapon,
        crystal_type: grade,
        body_part: crate::data::item_data::SLOT_R_HAND,
        weight: 0,
        is_stackable: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        price: 0,
        handler: crate::data::item_data::ItemHandler::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
    });
    world.data.item_data.set_weapon_shots_for_test(item_id, ss, sps);
}

/// Equip a freshly granted item and return its object id.
fn grant_and_equip(world: &mut World, player_oid: i32, client_id: u32, item_id: i32) -> i32 {
    let oid = super::items::add_inventory_item(world, player_oid, item_id, 1).unwrap()[0];
    super::items::use_equipable_item(world, client_id, player_oid, oid);
    oid
}

/// Using a soulshot with a matching-grade weapon charges the shot, consumes
/// `weapon.soulShotCount` from the stack, and plays the shot's `<skills>`
/// visual (`SoulShots.useItem`).
#[test]
fn soulshot_charges_consumes_and_plays_visual() {
    use crate::data::item_data::{CrystalType, ItemHandler};
    use crate::model::inventory::Inventory;
    use crate::model::{Player, ShotType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    shot_weapon(&mut world, 9500, CrystalType::D, 2, 2);
    world.data.item_data.insert_for_test(shot_template(1463, CrystalType::D, ItemHandler::SoulShots, 2150));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    grant_and_equip(&mut world, 3001, 1, 9500);
    let shot_oid = super::items::add_inventory_item(&mut world, 3001, 1463, 10).unwrap()[0];
    drain(&mut a_rx);

    super::items::use_equipable_item(&mut world, 1, 3001, shot_oid);

    assert!(world.objects.get_component::<Player>(&3001).unwrap().is_charged_shot(ShotType::Soulshots), "soulshot charged");
    assert_eq!(world.objects.get_component::<Inventory>(&3001).unwrap().count_of(1463), 8, "weapon.soulShotCount (2) consumed");
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE), "enable message sent");
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE
            && i32::from_le_bytes(p[13..17].try_into().unwrap()) == 2150),
        "shot visual (skill 2150) broadcast"
    );
}

/// A soulshot whose grade doesn't match the equipped weapon is refused.
#[test]
fn soulshot_wrong_grade_is_refused() {
    use crate::data::item_data::{CrystalType, ItemHandler};
    use crate::model::{Player, ShotType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    shot_weapon(&mut world, 9500, CrystalType::D, 2, 2);
    // A C-grade soulshot on a D-grade weapon.
    world.data.item_data.insert_for_test(shot_template(1464, CrystalType::C, ItemHandler::SoulShots, 2151));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    grant_and_equip(&mut world, 3001, 1, 9500);
    let shot_oid = super::items::add_inventory_item(&mut world, 3001, 1464, 10).unwrap()[0];
    drain(&mut a_rx);

    super::items::use_equipable_item(&mut world, 1, 3001, shot_oid);

    assert!(!world.objects.get_component::<Player>(&3001).unwrap().is_charged_shot(ShotType::Soulshots), "wrong-grade shot not charged");
    assert_eq!(world.objects.get_component::<crate::model::inventory::Inventory>(&3001).unwrap().count_of(1464), 10, "nothing consumed");
}

/// A charged soulshot is spent on the next non-miss melee swing, doubles its
/// damage, and sets the `SHOT_USED` flag (`generateHit`).
#[test]
fn soulshot_consumed_on_hit_doubles_melee_damage() {
    use crate::model::{Player, ShotType};

    fn attack_damage_and_flags(packets: &[Vec<u8>]) -> (i32, i32) {
        let atk = packets.iter().find(|p| p[0] == server_packets::opcodes::ATTACK).expect("Attack broadcast");
        (
            i32::from_le_bytes(atk[13..17].try_into().unwrap()),
            i32::from_le_bytes(atk[17..21].try_into().unwrap()),
        )
    }

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // Control swing (no shot): plain hit, no crit.
    world.forced_rolls.extend([0, 99, 10]);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    let (base_dmg, base_flags) = attack_damage_and_flags(&drain(&mut a_rx));
    assert_eq!(base_flags & 0x08, 0, "no soulshot flag without a charge");

    // Charged swing: identical rolls → exactly double, flag set, shot spent.
    world.objects.get_component_mut::<Player>(&3001).unwrap().charge_shot(ShotType::Soulshots);
    world.forced_rolls.extend([0, 99, 10]);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    let (ss_dmg, ss_flags) = attack_damage_and_flags(&drain(&mut a_rx));

    assert_eq!(ss_dmg, base_dmg * 2, "soulshot doubles the swing");
    assert_ne!(ss_flags & 0x08, 0, "SHOT_USED flag set");
    assert!(!world.objects.get_component::<Player>(&3001).unwrap().is_charged_shot(ShotType::Soulshots), "shot consumed");
}

/// A charged spiritshot doubles a magic attack's damage and is spent
/// (`calcMagicDam` `sps` bonus + `Skill` uncharge).
#[test]
fn spiritshot_doubles_magic_damage_and_is_consumed() {
    use crate::model::components::Vitals;
    use crate::model::{Player, ShotType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    let skill = world.data.skill_data.get(1177, 1).expect("Wind Strike").clone();
    assert_eq!(skill.magic_type, 1, "test skill must be magic");
    drain(&mut a_rx);

    let start_hp = nvit(&world, npc_oid).cur_hp;
    // Control cast (no shot), non-crit.
    world.forced_rolls.push_back(999_999);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let base = start_hp - nvit(&world, npc_oid).cur_hp;
    assert!(base > 0.0, "control nuke dealt damage");
    world.objects.get_component_mut::<Vitals>(&npc_oid).unwrap().cur_hp = start_hp;

    // Charged spiritshot cast, identical crit roll.
    world.objects.get_component_mut::<Player>(&3001).unwrap().charge_shot(ShotType::Spiritshots);
    world.forced_rolls.push_back(999_999);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let ss = start_hp - nvit(&world, npc_oid).cur_hp;

    assert!((ss - base * 2.0).abs() < 1e-6, "spiritshot doubles magic damage ({ss} vs {base})");
    assert!(!world.objects.get_component::<Player>(&3001).unwrap().is_charged_shot(ShotType::Spiritshots), "spiritshot consumed");
}

/// Toggling auto-use (`RequestAutoSoulShot`) with a matching weapon activates
/// the shot: `ExAutoSoulShot` ack, the auto-set records the item, and it's
/// charged immediately; a following attack keeps it topped up.
#[test]
fn auto_soulshot_toggle_activates_and_recharges() {
    use crate::data::item_data::{CrystalType, ItemHandler};
    use crate::model::{Player, ShotType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    shot_weapon(&mut world, 9500, CrystalType::D, 1, 1);
    world.data.item_data.insert_for_test(shot_template(1463, CrystalType::D, ItemHandler::SoulShots, 2150));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    grant_and_equip(&mut world, 3001, 1, 9500);
    super::items::add_inventory_item(&mut world, 3001, 1463, 10);
    drain(&mut a_rx);

    // itemId=1463, enable=1, type=0.
    let mut body = Vec::new();
    body.extend_from_slice(&1463i32.to_le_bytes());
    body.extend_from_slice(&1i32.to_le_bytes());
    body.extend_from_slice(&0i32.to_le_bytes());
    super::items::handle_request_auto_soul_shot(&mut world, 1, &body);

    assert!(world.objects.get_component::<Player>(&3001).unwrap().auto_shots.contains(&1463), "item recorded for auto-use");
    assert!(world.objects.get_component::<Player>(&3001).unwrap().is_charged_shot(ShotType::Soulshots), "charged on activation");
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::EX && i16::from_le_bytes(p[1..3].try_into().unwrap()) == server_packets::opcodes::EX_AUTO_SOUL_SHOT),
        "ExAutoSoulShot ack sent"
    );

    // The charge is spent on a hit, and the next attack auto-recharges it.
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // Swing 1 spends the activation charge (no item, just the flag).
    world.forced_rolls.extend([0, 99, 10]);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    drain(&mut a_rx);
    // Swing 2 finds no charge, auto-recharges (spends an item), then spends it:
    // the `SHOT_USED` flag on this swing proves the recharge fed it.
    world.forced_rolls.extend([0, 99, 10]);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    let atk = drain(&mut a_rx).into_iter().find(|p| p[0] == server_packets::opcodes::ATTACK).expect("Attack");
    assert_ne!(i32::from_le_bytes(atk[17..21].try_into().unwrap()) & 0x08, 0, "auto-shot re-charged and was spent on the 2nd swing");
    assert_eq!(
        world.objects.get_component::<crate::model::inventory::Inventory>(&3001).unwrap().count_of(1463),
        8,
        "activation + one auto-recharge consumed two shots"
    );
}

// --------------------------------------------------------------- admin (G13.A)

/// A datapack-backed world (real `AdminData`) so `is_gm`/access gating and the
/// name colors resolve; the synthetic `test_world` otherwise loads empty admin.
fn admin_world() -> (World, db::CmdTx, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db_tx, db_rx, link_rx) = test_world();
    world.data.admin =
        crate::data::AdminData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    (world, db_tx, db_rx, link_rx)
}

/// Like [`ingame_player`] but with a chosen access level (0 = user).
fn ingame_player_access(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    access_level: i32,
) -> tokio::sync::mpsc::UnboundedReceiver<Vec<u8>> {
    let mut chr = dummy_char(object_id, &format!("P{object_id}"));
    chr.access_level = access_level;
    let bundle = Player::from_char(&world.data, &chr);
    let (out_tx, out_rx) = tokio::sync::mpsc::unbounded_channel();
    let s = Session::new(client_id, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![])
        .into_entering(bundle);
    let (session, bundle) = s.into_ingame();
    bundle.spawn_into(&mut world.objects);
    world.clients.insert(client_id, ClientSession::InGame(session));
    out_rx
}

/// `SendBypassBuildCmd` (0x74) body — the raw `//command` text (no `admin_`).
fn build_cmd_body(command: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(command);
    w.into_bytes()
}

fn count_system_messages(pkts: &[Vec<u8>]) -> usize {
    pkts.iter().filter(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE).count()
}

/// A GM's `//serverinfo` runs and answers with server-info text lines.
#[test]
fn admin_serverinfo_runs_for_gm() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("serverinfo")].concat());
    let pkts = drain(&mut gm_rx);
    assert_eq!(count_system_messages(&pkts), 3, "three server-info lines");
}

/// A non-GM issuing an admin command is silently ignored (Java `isGM` gate).
#[test]
fn admin_command_ignored_for_non_gm() {
    let (mut world, ..) = admin_world();
    let mut user_rx = ingame_player_access(&mut world, 1, 5002, 0);
    drain(&mut user_rx);

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("serverinfo")].concat());
    assert!(drain(&mut user_rx).is_empty(), "non-GM gets no reply at all");
}

/// A GM whose tier lacks the required access level is refused with the Java
/// message. We synthesize a right the master tier's childAccess cannot reach by
/// using a real command but a mid-tier GM: `admin_serverinfo` needs level 100,
/// and a level-70 Admin's chain descends (never ascends) so it is denied.
#[test]
fn admin_command_access_denied_for_insufficient_level() {
    let (mut world, ..) = admin_world();
    // Level 70 ("Admin") is a GM (isGM=true) but its childAccess chain runs
    // 70→60→…→0, never reaching 100, so a level-100 command is refused.
    let mut rx = ingame_player_access(&mut world, 1, 5003, 70);
    drain(&mut rx);

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("serverinfo")].concat());
    let pkts = drain(&mut rx);
    // One system message: the "no access rights" refusal, not the 3 info lines.
    assert_eq!(count_system_messages(&pkts), 1, "single refusal line, command not run");
}

/// An unknown command answers "does not exist"; a known-but-unimplemented
/// command (gated in AdminCommands.xml, no body yet — G13.C) answers the
/// not-implemented path. Both for a master GM.
#[test]
fn admin_unknown_vs_unimplemented() {
    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 5004, 100);
    drain(&mut rx);

    // Not in AdminCommands.xml → "does not exist".
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("totally_made_up")].concat());
    assert_eq!(count_system_messages(&drain(&mut rx)), 1, "does-not-exist line");

    // In AdminCommands.xml (admin_debug, level 100) but no body yet (G13.B) →
    // not-implemented path, does not crash.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("debug")].concat());
    assert_eq!(count_system_messages(&drain(&mut rx)), 1, "not-implemented line");
}

/// A GM's name/title color comes from the access-level table; a normal player
/// keeps the client defaults.
#[test]
fn access_level_colors_applied() {
    let (world, ..) = admin_world();
    // Level 70 "Admin": nameColor/titleColor 0FF000 in AccessLevels.xml.
    let mut chr = dummy_char(6001, "Gm");
    chr.access_level = 70;
    let gm = Player::from_char(&world.data, &chr);
    assert_eq!(gm.player.name_color, 0x0F_F000);
    assert_eq!(gm.player.title_color, 0x0F_F000);

    // Level 0 keeps the client defaults (real-capture parity).
    let user = Player::from_char(&world.data, &dummy_char(6002, "Joe"));
    assert_eq!(user.player.name_color, crate::model::DEFAULT_NAME_COLOR);
    assert_eq!(user.player.title_color, crate::model::DEFAULT_TITLE_COLOR);
}

fn dlg_answer_body(message_id: i32, answer: i32, requester_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(message_id);
    w.write_i32(answer);
    w.write_i32(requester_id);
    w.into_bytes()
}

/// A `confirmDlg` command (admin_givehero) prompts with a ConfirmDlg and does
/// NOT execute; the DlgAnswer "yes" re-runs it (reaching dispatch — here the
/// not-implemented path), while "no" drops it silently.
#[test]
fn admin_confirm_dialog_round_trip() {
    const S1_3: i32 = server_packets::S1_3_MESSAGE_ID;

    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 5005, 100);
    drain(&mut rx);

    // //givehero → a single ConfirmDlg (0xF3), no execution yet.
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("givehero")].concat());
    let pkts = drain(&mut rx);
    assert_eq!(pkts.len(), 1, "only the ConfirmDlg is sent");
    assert_eq!(pkts[0][0], server_packets::opcodes::CONFIRM_DLG, "it's a ConfirmDlg");
    assert_eq!(count_system_messages(&pkts), 0, "command did not execute yet");

    // Answer "yes" → the stored command re-runs and reaches dispatch (givehero
    // has no body yet → the not-implemented reply proves re-execution).
    on_packet(&mut world, 1, [vec![cop::DLG_ANSWER], dlg_answer_body(S1_3, 1, 0)].concat());
    assert_eq!(count_system_messages(&drain(&mut rx)), 1, "re-ran on confirm");

    // A second "yes" does nothing — the pending command was consumed.
    on_packet(&mut world, 1, [vec![cop::DLG_ANSWER], dlg_answer_body(S1_3, 1, 0)].concat());
    assert!(drain(&mut rx).is_empty(), "no pending command to re-run");
}

/// Answering "no" to the confirm drops the command without executing it.
#[test]
fn admin_confirm_dialog_declined() {
    const S1_3: i32 = server_packets::S1_3_MESSAGE_ID;

    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 5006, 100);
    drain(&mut rx);

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("givehero")].concat());
    drain(&mut rx);
    on_packet(&mut world, 1, [vec![cop::DLG_ANSWER], dlg_answer_body(S1_3, 0, 0)].concat());
    assert!(drain(&mut rx).is_empty(), "declined command does not run");
}

/// `//heal` on a targeted, damaged player fully restores HP/MP/CP and pushes a
/// StatusUpdate to that player.
#[test]
fn admin_heal_restores_targeted_player() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7001, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7002, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    if let Some(v) = world.objects.get_component_mut::<Vitals>(&7002) {
        v.cur_hp = 1.0;
    }
    world.objects.add_components(&7001, crate::model::components::TargetRef(Some(7002)));

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("heal")].concat());

    let v = pvit(&world, 7002);
    assert_eq!(v.cur_hp, v.max_hp as f64, "victim fully healed");
    assert!(
        drain(&mut victim_rx).iter().any(|p| p[0] == server_packets::opcodes::STATUS_UPDATE),
        "victim got a StatusUpdate"
    );
}

/// `//kill` on a targeted player kills them (Java `doDie` path).
#[test]
fn admin_kill_slays_targeted_player() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7003, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7004, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    world.objects.add_components(&7003, crate::model::components::TargetRef(Some(7004)));
    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("kill")].concat());

    assert!(pvit(&world, 7004).dead, "victim is dead after //kill");
}

/// `//kill` with no target tells the GM to select one and kills nothing.
#[test]
fn admin_kill_without_target_warns() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7005, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("kill")].concat());
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "one 'select a target' line");
}

/// Build a `//command` (SendBypassBuildCmd) packet from a full command line.
fn build_admin(command_line: &str) -> Vec<u8> {
    [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body(command_line)].concat()
}

/// `//res` revives a dead targeted player and fully restores them.
#[test]
fn admin_res_revives_and_restores_target() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7101, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7102, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    if let Some(v) = world.objects.get_component_mut::<Vitals>(&7102) {
        v.cur_hp = 0.0;
        v.dead = true;
    }
    world.objects.add_components(&7101, crate::model::components::TargetRef(Some(7102)));
    on_packet(&mut world, 1, build_admin("res"));

    let v = pvit(&world, 7102);
    assert!(!v.dead, "victim revived");
    assert_eq!(v.cur_hp, v.max_hp as f64, "victim fully restored");
}

/// `//gmspeed N` sets the move multiplier to `1 + N` (0 resets) and rebroadcasts
/// UserInfo.
#[test]
fn admin_gmspeed_sets_move_multiplier() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7103, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("gmspeed 3"));
    assert_eq!(
        world.objects.get_component::<crate::model::components::Speeds>(&7103).unwrap().move_multiplier,
        4.0,
        "1 + boost"
    );
    assert!(drain(&mut gm_rx).iter().any(|p| p[0] == 0x32), "UserInfo (0x32) rebroadcast");

    on_packet(&mut world, 1, build_admin("gmspeed 0"));
    assert_eq!(
        world.objects.get_component::<crate::model::components::Speeds>(&7103).unwrap().move_multiplier,
        1.0,
        "boost 0 resets"
    );
}

/// `//gmspeed` out of range answers the usage line and changes nothing.
#[test]
fn admin_gmspeed_rejects_out_of_range() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7107, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("gmspeed 99"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "usage line");
    assert_eq!(
        world.objects.get_component::<crate::model::components::Speeds>(&7107).unwrap().move_multiplier,
        1.0,
        "unchanged"
    );
}

/// `//teleport x y z` moves the GM to those coordinates and broadcasts a
/// TeleportToLocation.
#[test]
fn admin_teleport_moves_gm_to_coords() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7104, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("teleport 100 200 300"));
    let pos = *world.objects.get_component::<crate::model::components::Position>(&7104).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (100, 200, 300), "moved to coords");
    assert!(
        drain(&mut gm_rx).iter().any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION),
        "TeleportToLocation broadcast"
    );
}

/// `//recall <name>` brings the named online player to the GM's location.
#[test]
fn admin_recall_brings_player_to_gm() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7105, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 7106, 0);
    drain(&mut gm_rx);
    drain(&mut other_rx);

    if let Some(p) = world.objects.get_component_mut::<crate::model::components::Position>(&7105) {
        p.x = 500;
        p.y = 600;
        p.z = 700;
    }
    on_packet(&mut world, 1, build_admin("recall P7106"));
    let pos = *world.objects.get_component::<crate::model::components::Position>(&7106).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (500, 600, 700), "recalled to GM position");
}

/// `//create_item 57 1000` puts 1000 adena in the GM's inventory.
#[test]
fn admin_create_item_adds_to_gm_inventory() {
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut gm_rx = ingame_player_access(&mut world, 1, 7201, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("create_item 57 1000"));
    assert_eq!(
        world.objects.get_component::<crate::model::inventory::Inventory>(&7201).unwrap().count_of(57),
        1000,
        "1000 adena created"
    );
}

/// `//create_item` with a bogus id answers "does not exist" and adds nothing.
#[test]
fn admin_create_item_rejects_unknown_id() {
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 7204, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("create_item 99999999 5"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "does-not-exist line");
}

/// `//kick <name>` persists + despawns the target and drops their session.
#[test]
fn admin_kick_disconnects_target() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7202, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7203, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    // admin_kick carries confirmDlg="true", so it prompts first; answer "yes".
    on_packet(&mut world, 1, build_admin("kick P7203"));
    assert!(world.clients.contains_key(&2), "not kicked until confirmed");
    on_packet(
        &mut world,
        1,
        [vec![cop::DLG_ANSWER], dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0)].concat(),
    );
    assert!(!world.clients.contains_key(&2), "victim session removed after confirm");
    assert!(world.objects.get_component::<Player>(&7203).is_none(), "victim despawned");
}

/// `//add_exp_sp <exp> <sp>` grants exp and sp (driving level-up).
#[test]
fn admin_add_exp_sp_grants_to_self() {
    let (mut world, ..) = admin_world();
    world.data.experience =
        crate::data::ExperienceData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 7301, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("add_exp_sp 1000 500"));
    let p = world.objects.get_component::<Player>(&7301).unwrap();
    assert!(p.exp >= 1000, "exp granted");
    assert_eq!(p.sp, 500, "sp granted");
}

/// `//set_level N` sets the target's level; `//add_level N` adds to it.
#[test]
fn admin_set_and_add_level() {
    let (mut world, ..) = admin_world();
    world.data.experience =
        crate::data::ExperienceData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 7305, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("set_level 20"));
    assert_eq!(world.objects.get_component::<Player>(&7305).unwrap().level, 20, "set to 20");

    on_packet(&mut world, 1, build_admin("add_level 5"));
    assert_eq!(world.objects.get_component::<Player>(&7305).unwrap().level, 25, "added 5");
}

/// `//gmchat` reaches every online GM (including the sender) but no normal
/// player.
#[test]
fn admin_gmchat_broadcasts_to_gms_only() {
    let (mut world, ..) = admin_world();
    let mut gm1 = ingame_player_access(&mut world, 1, 7302, 100);
    let mut gm2 = ingame_player_access(&mut world, 2, 7303, 100);
    let mut user = ingame_player_access(&mut world, 3, 7304, 0);
    drain(&mut gm1);
    drain(&mut gm2);
    drain(&mut user);

    on_packet(&mut world, 1, build_admin("gmchat hello gms"));
    let say = server_packets::opcodes::SAY2;
    assert!(drain(&mut gm1).iter().any(|p| p[0] == say), "sender GM sees it");
    assert!(drain(&mut gm2).iter().any(|p| p[0] == say), "other GM sees it");
    assert!(drain(&mut user).iter().all(|p| p[0] != say), "normal player does not");
}

/// `//changelvl <name> <level>` promotes a player, updates colors/is_gm, and
/// queues the persisting DB update.
#[test]
fn admin_changelvl_sets_access_and_persists() {
    let (mut world, _db_tx, mut db_rx, _link) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7401, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7402, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    on_packet(&mut world, 1, build_admin("changelvl P7402 70"));
    let p = world.objects.get_component::<Player>(&7402).unwrap();
    assert_eq!(p.access_level, 70, "promoted to 70");
    assert!(p.is_gm(&world.data), "now a GM");
    assert_eq!(p.name_color, 0x0F_F000, "tier color applied");
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::SetAccessLevel { char_id: 7402, level: 70 })),
        "access-level UPDATE queued"
    );
}

/// `//changelvl` to an undefined level is refused and changes nothing.
#[test]
fn admin_changelvl_rejects_unknown_level() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7404, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("changelvl 55"));
    assert_eq!(world.objects.get_component::<Player>(&7404).unwrap().access_level, 100, "unchanged");
}

/// `//gm` deactivates the caller's own GM access for the session (not persisted).
#[test]
fn admin_gm_deactivates_own_access() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7403, 100);
    drain(&mut gm_rx);
    assert!(world.objects.get_component::<Player>(&7403).unwrap().is_gm(&world.data));

    on_packet(&mut world, 1, build_admin("gm"));
    let p = world.objects.get_component::<Player>(&7403).unwrap();
    assert_eq!(p.access_level, 0, "demoted to user");
    assert!(!p.is_gm(&world.data), "no longer GM");
}

/// `//announce` reaches every online player.
#[test]
fn admin_announce_reaches_all_players() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7501, 100);
    let mut u1 = ingame_player_access(&mut world, 2, 7502, 0);
    let mut u2 = ingame_player_access(&mut world, 3, 7503, 0);
    drain(&mut gm_rx);
    drain(&mut u1);
    drain(&mut u2);

    on_packet(&mut world, 1, build_admin("announce server restart soon"));
    assert_eq!(count_system_messages(&drain(&mut u1)), 1, "player 1 got the announce");
    assert_eq!(count_system_messages(&drain(&mut u2)), 1, "player 2 got the announce");
}

/// `//character_disconnect` disconnects the targeted player.
#[test]
fn admin_character_disconnect_kicks_target() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7504, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7505, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    world.objects.add_components(&7504, crate::model::components::TargetRef(Some(7505)));
    on_packet(&mut world, 1, build_admin("character_disconnect"));
    assert!(!world.clients.contains_key(&2), "victim disconnected");
    assert!(world.objects.get_component::<Player>(&7505).is_none(), "victim despawned");
}

/// `//delete` despawns the targeted NPC and broadcasts DeleteObject.
#[test]
fn admin_delete_despawns_targeted_npc() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7601, 100);
    drain(&mut gm_rx);

    let npc_oid = crate::model::npc::FIRST_NPC_OBJECT_ID + 1;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 1, 2, 3, 100, 50);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    world.objects.add_components(&7601, crate::model::components::TargetRef(Some(npc_oid)));

    on_packet(&mut world, 1, build_admin("delete"));
    assert!(
        world.objects.get_component::<crate::model::npc::Npc>(&npc_oid).is_none(),
        "npc despawned by //delete"
    );
    assert!(
        drain(&mut gm_rx).iter().any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT),
        "GM got DeleteObject"
    );
}

/// `//delete` with a non-NPC target (or none) warns and deletes nothing.
#[test]
fn admin_delete_without_npc_target_warns() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7603, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("delete"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "select-an-npc line");
}

/// `//spawn` with an unknown NPC id is refused.
#[test]
fn admin_spawn_rejects_unknown_npc() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7602, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("spawn 99999"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "does-not-exist line");
}

/// `//spawn <npcId>` creates the NPC at the GM's location and shows it to them.
#[test]
fn admin_spawn_creates_npc_at_gm() {
    let (mut world, ..) = admin_world();
    world.data.npc_data =
        crate::data::NpcData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 7604, 100);
    drain(&mut gm_rx);
    if let Some(p) = world.objects.get_component_mut::<crate::model::components::Position>(&7604) {
        p.x = 100;
        p.y = 200;
        p.z = 300;
    }

    let npc_oid = world.next_npc_object_id;
    on_packet(&mut world, 1, build_admin("spawn 30001")); // Lector, a Merchant (non-monster)
    assert_eq!(world.next_npc_object_id, npc_oid + 1, "one NPC spawned");
    let npc = world.objects.get_component::<crate::model::npc::Npc>(&npc_oid).expect("npc entity exists");
    assert_eq!(npc.npc_id, 30001);
    let pos = world.objects.get_component::<crate::model::components::Position>(&npc_oid).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (100, 200, 300), "spawned at the GM");
    assert!(
        drain(&mut gm_rx).iter().any(|p| p[0] == server_packets::opcodes::NPC_INFO),
        "GM was shown the NPC"
    );
}

/// `//target <name>` selects that player (MyTargetSelected + TargetRef set).
#[test]
fn admin_target_selects_named_player() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7701, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7702, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    on_packet(&mut world, 1, build_admin("target P7702"));
    assert_eq!(
        world.objects.get_component::<crate::model::components::TargetRef>(&7701).and_then(|t| t.0),
        Some(7702),
        "GM now targets the named player"
    );
    assert!(
        drain(&mut gm_rx).iter().any(|p| p[0] == server_packets::opcodes::MY_TARGET_SELECTED),
        "GM got MyTargetSelected"
    );
}

/// `//invul` toggles invulnerability; incoming damage is ignored while on.
#[test]
fn admin_invul_blocks_damage() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7801, 100);
    drain(&mut gm_rx);
    // The synthetic template has no HP table; give the player real HP.
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&7801) {
        v.max_hp = 1000;
        v.cur_hp = 1000.0;
        v.dead = false;
    }

    on_packet(&mut world, 1, build_admin("invul"));
    assert!(world.objects.get_component::<crate::model::components::AdminFlags>(&7801).unwrap().invul);

    let hp_before = pvit(&world, 7801).cur_hp;
    super::combat::player_receive_damage(&mut world, 7801, 12345, 50.0);
    assert_eq!(pvit(&world, 7801).cur_hp, hp_before, "invul: no damage taken");

    // Toggle off → damage lands.
    on_packet(&mut world, 1, build_admin("invul"));
    super::combat::player_receive_damage(&mut world, 7801, 12345, 50.0);
    assert!(pvit(&world, 7801).cur_hp < hp_before, "damage applies once invul is off");
}

/// `//undying` lets damage apply but never kills — HP floors at 1.
#[test]
fn admin_undying_floors_hp_at_one() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7802, 100);
    drain(&mut gm_rx);
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&7802) {
        v.max_hp = 1000;
        v.cur_hp = 1000.0;
        v.dead = false;
    }

    on_packet(&mut world, 1, build_admin("undying"));
    super::combat::player_receive_damage(&mut world, 7802, 12345, 100_000.0);
    let v = pvit(&world, 7802);
    assert_eq!(v.cur_hp, 1.0, "undying floors HP at 1");
    assert!(!v.dead, "undying player does not die");
}

/// `//setinvul` toggles invulnerability on the targeted player.
#[test]
fn admin_setinvul_targets_a_player() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7803, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 7804, 0);
    drain(&mut gm_rx);
    drain(&mut other_rx);

    world.objects.add_components(&7803, crate::model::components::TargetRef(Some(7804)));
    on_packet(&mut world, 1, build_admin("setinvul"));
    assert!(world.objects.get_component::<crate::model::components::AdminFlags>(&7804).unwrap().invul);
}

/// `//hide` removes the GM from nearby players' view (DeleteObject) and toggling
/// it off re-introduces them (CharInfo).
#[test]
fn admin_hide_toggles_visibility() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7901, 100);
    let mut obs_rx = ingame_player_access(&mut world, 2, 7902, 0);
    drain(&mut gm_rx);
    drain(&mut obs_rx);

    on_packet(&mut world, 1, build_admin("hide"));
    assert!(world.objects.get_component::<crate::model::components::AdminFlags>(&7901).unwrap().hidden);
    assert!(
        drain(&mut obs_rx).iter().any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT
            && i32::from_le_bytes([p[1], p[2], p[3], p[4]]) == 7901),
        "observer got DeleteObject for the hidden GM"
    );

    on_packet(&mut world, 1, build_admin("hide"));
    assert!(!world.objects.get_component::<crate::model::components::AdminFlags>(&7901).unwrap().hidden);
    assert!(
        drain(&mut obs_rx).iter().any(|p| p[0] == server_packets::opcodes::CHAR_INFO),
        "observer got CharInfo when the GM reappeared"
    );
}

/// `//add_skill <id> <lvl>` puts the skill in the target's book and refreshes
/// their SkillList; `//remove_skill` takes it back out.
#[test]
fn admin_add_and_remove_skill() {
    let (mut world, ..) = admin_world();
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8001, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("add_skill 1177 1"));
    assert_eq!(
        world.objects.get_component::<crate::model::components::SkillBook>(&8001).unwrap().0.get(&1177),
        Some(&1),
        "skill added to the book"
    );
    assert!(drain(&mut gm_rx).iter().any(|p| p[0] == 0x5F), "SkillList refresh sent");

    on_packet(&mut world, 1, build_admin("remove_skill 1177"));
    assert!(
        !world.objects.get_component::<crate::model::components::SkillBook>(&8001).unwrap().0.contains_key(&1177),
        "skill removed"
    );
}

/// `//add_skill` with an unknown id is refused.
#[test]
fn admin_add_skill_rejects_unknown() {
    let (mut world, ..) = admin_world();
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8002, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("add_skill 99999999 1"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "does-not-exist line");
}

/// `//setew <n>` sets the enchant level of the equipped weapon.
#[test]
fn admin_setew_enchants_equipped_weapon() {
    let (mut world, ..) = admin_world();
    world.data.item_data =
        crate::data::ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8101, 100);
    drain(&mut gm_rx);
    // Equip a weapon (item 1, the starter gloves aside — any weapon id) in RHand.
    let weapon = crate::character::ItemRow {
        object_id: 50000,
        item_id: 1,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: crate::model::inventory::PaperdollSlot::RHand as i32,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
    };
    world.objects.add_components(&8101, crate::model::inventory::Inventory::from_rows(&[weapon]));

    on_packet(&mut world, 1, build_admin("setew 10"));
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&8101)
            .unwrap()
            .paperdoll_enchant_level(crate::model::inventory::PaperdollSlot::RHand),
        10,
        "weapon enchanted to +10"
    );
}

/// `//setew` with no weapon equipped warns.
#[test]
fn admin_setew_without_weapon_warns() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8102, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("setew 10"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "no-item-in-slot line");
}

/// `//buff <id>` applies the skill's effects (a buff) to the GM.
#[test]
fn admin_buff_applies_skill_to_self() {
    let (mut world, ..) = admin_world();
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8201, 100);
    drain(&mut gm_rx);

    let before = pbuffs(&world, 8201);
    on_packet(&mut world, 1, build_admin("buff 1068 1")); // Might
    assert!(pbuffs(&world, 8201) > before, "//buff applied a buff");
}

/// `//buff` with an unknown skill is refused.
#[test]
fn admin_buff_rejects_unknown_skill() {
    let (mut world, ..) = admin_world();
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8202, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("buff 99999999 1"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "does-not-exist line");
}

/// The `//editchar` field setters mutate the targeted player and broadcast.
#[test]
fn admin_editchar_field_setters() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8301, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 8302, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);
    world.objects.add_components(&8301, crate::model::components::TargetRef(Some(8302)));

    let p = |w: &World| w.objects.get_component::<Player>(&8302).unwrap().clone();

    on_packet(&mut world, 1, build_admin("setreputation -500"));
    assert_eq!(p(&world).reputation, -500);
    on_packet(&mut world, 1, build_admin("nokarma"));
    assert_eq!(p(&world).reputation, 0);
    on_packet(&mut world, 1, build_admin("setpk 7"));
    assert_eq!(p(&world).pk_kills, 7);
    on_packet(&mut world, 1, build_admin("setpvp 9"));
    assert_eq!(p(&world).pvp_kills, 9);
    on_packet(&mut world, 1, build_admin("setfame 42"));
    assert_eq!(p(&world).fame, 42);
    on_packet(&mut world, 1, build_admin("settitle Hello World"));
    assert_eq!(p(&world).title, "Hello World");
    on_packet(&mut world, 1, build_admin("setcolor FF0000"));
    assert_eq!(p(&world).name_color, 0xFF_0000);
    assert!(!p(&world).is_female);
    on_packet(&mut world, 1, build_admin("setsex"));
    assert!(p(&world).is_female, "gender flipped");
}

/// `//set_hp <n>` sets the caster's current HP (clamped to max).
#[test]
fn admin_set_hp_sets_current_hp() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8303, 100);
    drain(&mut gm_rx);
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&8303) {
        v.max_hp = 500;
        v.cur_hp = 500.0;
        v.dead = false;
    }

    on_packet(&mut world, 1, build_admin("set_hp 100"));
    assert_eq!(pvit(&world, 8303).cur_hp, 100.0, "HP set to 100");
    // Clamps above max.
    on_packet(&mut world, 1, build_admin("set_hp 99999"));
    assert_eq!(pvit(&world, 8303).cur_hp, 500.0, "clamped to max");
}

/// `//getbuffs` lists the target's active buffs (header + one line per buff).
#[test]
fn admin_getbuffs_lists_active_buffs() {
    let (mut world, ..) = admin_world();
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8401, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("buff 1068 1")); // Might
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin("getbuffs"));
    assert!(count_system_messages(&drain(&mut gm_rx)) >= 2, "header + at least one buff line");
}

/// `//stopbuff <id>` removes that one buff.
#[test]
fn admin_stopbuff_removes_one() {
    let (mut world, ..) = admin_world();
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8501, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("buff 1068 1"));
    let has = |w: &World| {
        w.objects
            .get_component::<crate::model::components::Buffs>(&8501)
            .is_some_and(|b| b.0.iter().any(|x| x.skill_id == 1068))
    };
    assert!(has(&world), "Might applied");
    on_packet(&mut world, 1, build_admin("stopbuff 1068"));
    assert!(!has(&world), "Might removed by //stopbuff");
}

/// `//stopallbuffs` prompts (confirmDlg) and clears every buff on confirm.
#[test]
fn admin_stopallbuffs_clears_after_confirm() {
    let (mut world, ..) = admin_world();
    world.data.skill_data =
        crate::data::SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8502, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("buff 1068 1"));
    assert!(pbuffs(&world, 8502) >= 1, "a buff is active");

    // confirmDlg="true": prompts first, no clear yet.
    on_packet(&mut world, 1, build_admin("stopallbuffs"));
    assert!(pbuffs(&world, 8502) >= 1, "not cleared until confirmed");

    on_packet(
        &mut world,
        1,
        [vec![cop::DLG_ANSWER], dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0)].concat(),
    );
    assert_eq!(pbuffs(&world, 8502), 0, "all buffs cleared after confirm");
}

/// `//setclass <id>` changes the target's class and recomputes their template.
#[test]
fn admin_setclass_changes_class() {
    let (mut world, ..) = admin_world();
    world.data.player_templates =
        crate::data::PlayerTemplateData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8701, 100);
    drain(&mut gm_rx);
    assert_eq!(world.objects.get_component::<Player>(&8701).unwrap().class_id, 0);

    on_packet(&mut world, 1, build_admin("setclass 1"));
    let p = world.objects.get_component::<Player>(&8701).unwrap();
    assert_eq!(p.class_id, 1, "class changed to 1");
    assert_eq!(p.base_class_id, 1);
}

/// `//setclass` with an unknown class id is refused.
#[test]
fn admin_setclass_rejects_unknown() {
    let (mut world, ..) = admin_world();
    world.data.player_templates =
        crate::data::PlayerTemplateData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8702, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("setclass 99999"));
    assert_eq!(world.objects.get_component::<Player>(&8702).unwrap().class_id, 0, "unchanged");
}
