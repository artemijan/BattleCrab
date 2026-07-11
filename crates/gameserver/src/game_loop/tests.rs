use super::*;
use super::dispatch::*;
use super::lobby::*;
use super::net::*;
use super::position::*;
use super::skills::cast::*;
use super::skills::*;
use super::target::*;
use crate::character::CharData;
use crate::db::DbEvent;
use crate::loginlink::LoginLinkCommand;
use crate::model::formulas;
use crate::model::skill::{OperateType, Skill, TargetType};
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
    world.players.get_mut(&3001).unwrap().reuses.insert(
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
    let skills = &mut world.players.get_mut(&3001).unwrap().skills;
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
    let reuses = &world.players[&3001].reuses;
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

/// The next queued DB command, which must be a `StorePlayer`; returns its
/// snapshot.
fn expect_store_player(db_rx: &mut db::CmdRx) -> db::PlayerSnapshot {
    match db_rx.try_recv() {
        Ok(db::DbCommand::StorePlayer { snap }) => snap,
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
        let p = world.players.get_mut(&5001).unwrap();
        p.exp = 1234;
        p.x = 777;
    }

    handle_request_restart(&mut world, 1);

    // storeMe: the snapshot carries the live (not the loaded) state, and
    // is queued before the character-list reload.
    let snap = expect_store_player(&mut db_rx);
    assert_eq!((snap.object_id, snap.exp, snap.x), (5001, 1234, 777));
    match db_rx.try_recv() {
        Ok(db::DbCommand::LoadCharacters { client_id, account }) => {
            assert_eq!((client_id, account.as_str()), (1, "bob"));
        }
        _ => panic!("expected a LoadCharacters DB command after the store"),
    }

    // deleteMe + setConnectionState(AUTHENTICATED) + RestartResponse.TRUE.
    assert!(world.players.is_empty());
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
    assert!(world.players.contains_key(&5001), "player re-entered the world");
    assert!(matches!(world.clients.get(&1), Some(ClientSession::InGame(_))));
}

/// Logout: the player is stored + removed and the client gets `LeaveWorld`;
/// dropping the session is what closes the socket.
#[test]
fn logout_stores_player_and_sends_leave_world() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut out_rx = ingame_player(&mut world, 1, 5002, 100, 200, 0);

    handle_logout(&mut world, 1);

    assert_eq!(expect_store_player(&mut db_rx).object_id, 5002);
    assert!(world.players.is_empty());
    assert!(world.clients.is_empty(), "session dropped → socket closes");
    assert_eq!(out_rx.try_recv().unwrap()[0], server_packets::opcodes::LOG_OUT_OK);
}

/// An unexpected disconnect while in game persists the player too (Java
/// `GameClient.onDisconnection` → `Disconnection.storeMe().deleteMe()`).
#[test]
fn disconnect_stores_ingame_player() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let _out_rx = ingame_player(&mut world, 1, 5003, 100, 200, 0);

    on_disconnect(&mut world, 1);

    assert_eq!(expect_store_player(&mut db_rx).object_id, 5003);
    assert!(world.players.is_empty());
    assert!(world.clients.is_empty());
}
