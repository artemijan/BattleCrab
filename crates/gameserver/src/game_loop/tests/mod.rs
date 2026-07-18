//! Game-loop integration tests. Shared fixtures/helpers live here; the
//! `#[test]` cases are split into topical submodules that `use super::*`.

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
use crate::model::components::{AdminFlags, Buffs, Casting, ClientPos, CombatStats, Intent, LastFolkNpc, Movement, PlayerVitals, Position, Reuses, SkillBook, Speeds, TargetRef, Vitals};
use crate::model::Player;
use crate::network::client_packets::{self as cp, opcodes as cop};
use crate::network::server_packets;
use crate::session::{ClientSession, Session, SessionKey};
use commons::network::PacketWriter;
use crate::model::components::{Macros, Shortcuts};
use crate::model::shortcut::{Macro, MacroCmd, MacroType, Shortcut, ShortcutType};
use crate::model::components::{PartyRef, PendingRequest};
use crate::model::party::LootRule;
use crate::character::FriendInfo;
use crate::model::components::Friends;

mod admin_tests;
mod clans_tests;
mod combat_tests;
mod community_board_tests;
mod items_tests;
mod lobby_tests;
mod misc_tests;
mod movement_tests;
mod npc_tests;
mod quests_tests;
mod shortcuts_tests;
mod skills_tests;
mod social_tests;
mod teleport_cmds_tests;
mod visibility_tests;
mod zones_tests;

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
        rec_have: 0,
        rec_left: 20,
        clan_id: 0,
        clan_privs: 0,
        clan_create_expiry_time: 0,
        race: 0,
        class_id: 0,
        base_class_id: 0,
        delete_time: 0,
        last_access: 0,
        vitality_points: 0,
        pccafe_points: 0,
        prime_points: 0,
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
        pledge_skill_trees: crate::data::PledgeSkillTreeData::empty(),
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
        scheme_buffer: crate::data::SchemeBufferData::default(),
        categories: crate::data::CategoryData::empty(),
        cursed_weapons: crate::data::CursedWeaponData::empty(),
        siege_towers: std::collections::HashMap::new(),
        castle_restart_points: std::collections::HashMap::new(),
        teleporters: crate::data::TeleporterData::empty(),
        transforms: crate::data::TransformData::empty(),
        enchant: crate::data::EnchantData::empty(),
        variations: crate::data::VariationData::empty(),
        admin: crate::data::AdminData::empty(),
        combat_caps: crate::data::CombatCaps::default(),
        gm: crate::data::GmSettings::default(),
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
            DbEvent::IdBlock { .. }
            | DbEvent::ClansLoaded { .. }
            | DbEvent::PremiumLoaded { .. }
            | DbEvent::BufferSchemesLoaded { .. }
            | DbEvent::GrandBossesLoaded { .. }
            | DbEvent::CursedWeaponsLoaded { .. }
            | DbEvent::CastlesLoaded { .. }
            | DbEvent::SiegesLoaded { .. }
            | DbEvent::SiegeGuardsLoaded { .. } => continue,
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
        magic_level: 0,        effect_point: 0,
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
        activate_rate: -1,
        lvl_bonus_rate: 0,
        single_target: true,
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
    // Power Strike 3 — the canonical physical attack skill (`magic_type: 0`).
    data.skill_data.insert_for_test(Skill {
        id: 3,
        name: "Power Strike".into(),
        target_type: TargetType::EnemyOnly,
        magic_type: 0,
        magic_level: 0,        effect_point: -52,
        hit_time: 1000,
        reuse_delay: 3000,
        effects: vec![SkillEffect::PhysicalAttack {
            power: 30.0,
            p_atk_mod: 1.0,
            p_def_mod: 1.0,
            critical_chance: 10.0,
        }],
        ..base.clone()
    });
    // Decrease Speed 1160 — a single-target debuff: Speed -20% (PER) on an
    // enemy. `effect_point` negative → `is_bad`. The magic-level/activate/
    // lvl-bonus trio matches dist level 1 so the landing-rate roll and its
    // caster-facing chance line compute the real 90 (constrained) vs a low-level mob.
    data.skill_data.insert_for_test(Skill {
        id: 1160,
        name: "Decrease Speed".into(),
        target_type: TargetType::EnemyOnly,
        magic_level: 35,
        activate_rate: 80,
        lvl_bonus_rate: 30,
        effect_point: -331,
        hit_time: 1000,
        abnormal_time: 60,
        abnormal_type: "SPEED_DOWN".into(),
        effects: [Stat::RunSpeed, Stat::WalkSpeed, Stat::SwimRunSpeed, Stat::SwimWalkSpeed]
            .into_iter()
            .map(|stat| {
                SkillEffect::StatModifier(StatModifierEffect {
                    stat,
                    mode: StatModifierType::Per,
                    amount: -20.0,
                    armor_condition: 0,
                    weapon_condition: 0,
                })
            })
            .collect(),
        ..base.clone()
    });
    // Vampiric Touch 1147 — HpDrain: magic damage + 40% self-heal.
    data.skill_data.insert_for_test(Skill {
        id: 1147,
        name: "Vampiric Touch".into(),
        target_type: TargetType::Enemy,
        effect_point: -143,
        hit_time: 1000,
        effects: vec![SkillEffect::HpDrain { power: 18.0, percentage: 40.0 }],
        ..base.clone()
    });
    // Backstab 30 — a dagger blow requiring a flank (backstab: true).
    data.skill_data.insert_for_test(Skill {
        id: 30,
        name: "Backstab".into(),
        target_type: TargetType::Enemy,
        magic_type: 0,
        magic_level: 0,        effect_point: -305,
        hit_time: 1000,
        effects: vec![SkillEffect::Blow { power: 1107.0, chance_boost: 400.0, critical_chance: Some(5.0), backstab: true }],
        ..base.clone()
    });
    // Mortal Blow 16 — a FatalBlow (no flank requirement).
    data.skill_data.insert_for_test(Skill {
        id: 16,
        name: "Mortal Blow".into(),
        target_type: TargetType::Enemy,
        magic_type: 0,
        magic_level: 0,        effect_point: -52,
        hit_time: 1000,
        effects: vec![SkillEffect::Blow { power: 73.0, chance_boost: 200.0, critical_chance: Some(0.0), backstab: false }],
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

fn use_item_body(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(object_id);
    w.write_i32(0); // ctrl
    w.into_bytes()
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

fn bypass_body(command: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(command);
    w.into_bytes()
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

/// The next queued DB command, which must be a `StorePlayer`; returns its
/// full save payload.
fn expect_store_player(db_rx: &mut db::CmdRx) -> db::PlayerSaveData {
    match db_rx.try_recv() {
        Ok(db::DbCommand::StorePlayer { save }) => save,
        _ => panic!("expected a StorePlayer DB command"),
    }
}

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
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
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

/// The string of a `SystemMessage` whose first parameter is `Text` (the shape
/// `Player.sendMessage(String)` / `send_message` produces). `None` for other
/// packets or non-text messages. Layout: opcode, id(i16), count(u8),
/// type(u8=0 Text), then the UTF-16 string.
fn sysmsg_text(p: &[u8]) -> Option<String> {
    if p.first() != Some(&server_packets::opcodes::SYSTEM_MESSAGE) || p.len() < 5 || p[3] == 0 || p[4] != 0 {
        return None;
    }
    commons::network::PacketReader::new(&p[5..]).read_string()
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

fn acquire_skill_body(skill_id: i32, skill_level: i32, acquire_type: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(skill_id);
    w.write_i32(skill_level);
    w.write_i32(acquire_type);
    w.into_bytes()
}

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
            crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
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

/// A stand-in passive clan skill (e.g. Clan Body 370) for the clan-skill tests:
/// a passive with one flat +PAtk stat effect, so applying it both lands a
/// passive buff and (via the shared pipeline) moves a supported stat.
fn passive_clan_test_skill(id: i32) -> Skill {
    use crate::model::skill::{SkillEffect, StatModifierEffect};
    use crate::model::stats::{Stat, StatModifierType};
    Skill {
        id,
        level: 1,
        name: format!("Clan Skill {id}"),
        operate_type: OperateType::Passive,
        target_type: TargetType::Self_,
        magic_type: 2,
        magic_level: 0,        effect_point: 0,
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
        activate_rate: -1,
        lvl_bonus_rate: 0,
        single_target: true,
        effects: vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::PhysicalAttack,
            mode: StatModifierType::Diff,
            amount: 10.0,
            armor_condition: 0,
            weapon_condition: 0,
        })],
    }
}

/// A stand-in for `CommonSkill.CLAN_ADVENT` (skill 19009 lv.1). The real skill
/// lives in the dist `stats/skills` set, which the synthetic test `SkillData`
/// doesn't load — register this so the clan login/logout path has something to
/// apply. Permanent (`abnormal_time = -1`), one +5% PAtk stat modifier standing
/// in for the full six-effect aura.
fn clan_advent_test_skill() -> Skill {
    use crate::model::skill::{SkillEffect, StatModifierEffect};
    use crate::model::stats::{Stat, StatModifierType};
    Skill {
        id: 19009,
        level: 1,
        name: "Clan Advent".into(),
        operate_type: OperateType::Other,
        target_type: TargetType::Self_,
        magic_type: 2,
        magic_level: 0,        effect_point: 100,
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
        abnormal_time: -1,
        abnormal_level: 1,
        abnormal_type: "CLAN_ADVENT".into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        single_target: true,
        effects: vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::PhysicalAttack,
            mode: StatModifierType::Per,
            amount: 5.0,
            armor_condition: 0,
            weapon_condition: 0,
        })],
    }
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

fn decode_npc_html(pkt: &[u8]) -> Option<String> {
    if pkt[0] != server_packets::opcodes::NPC_HTML_MESSAGE {
        return None;
    }
    let mut r = commons::network::PacketReader::new(&pkt[1..]);
    r.read_i32()?;
    r.read_string()
}

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
        castle_id: 0,
    });
}

/// A synthetic siege-zone cuboid tied to `castle_id`.
fn insert_siege_zone(world: &mut World, castle_id: i32, x1: i32, x2: i32, y1: i32, y2: i32) {
    world.data.zone_data.insert(crate::data::zone_data::Zone {
        name: format!("test_siege_{castle_id}"),
        kind: crate::data::zone_data::ZoneKind::Siege,
        territory: crate::data::spawn_data::Territory {
            form: crate::data::spawn_data::ZoneForm::Cuboid { x1, x2, y1, y2 },
            min_z: -1000,
            max_z: 1000,
        },
        castle_id,
    });
}

fn compass_code(pkt: &[u8]) -> Option<i32> {
    (pkt[0] == server_packets::opcodes::EX
        && i16::from_le_bytes(pkt[1..3].try_into().unwrap()) == server_packets::opcodes::EX_SET_COMPASS_ZONE_CODE)
        .then(|| i32::from_le_bytes(pkt[3..7].try_into().unwrap()))
}

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
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
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
            crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
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

/// A `<set name="handler">` shot item template (soulshot/spiritshot).
fn shot_template(item_id: i32, grade: crate::data::item_data::CrystalType, handler: crate::data::item_data::ItemHandler, skill_id: i32) -> crate::data::item_data::ItemTemplate {
    crate::data::item_data::ItemTemplate {
        item_id,
        name: format!("shot{item_id}"),
        kind: crate::data::item_data::ItemKind::Etc,
        crystal_type: grade, crystal_count: 0,
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
        item_skills: vec![(skill_id, 1)], etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    }
}

/// A graded weapon template that consumes `ss`/`sps` shots per charge.
fn shot_weapon(world: &mut World, item_id: i32, grade: crate::data::item_data::CrystalType, ss: i32, sps: i32) {
    world.data.item_data.insert_for_test(crate::data::item_data::ItemTemplate {
        item_id,
        name: format!("weapon{item_id}"),
        kind: crate::data::item_data::ItemKind::Weapon,
        crystal_type: grade, crystal_count: 0,
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
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    });
    world.data.item_data.set_weapon_shots_for_test(item_id, ss, sps);
}

/// Equip a freshly granted item and return its object id.
fn grant_and_equip(world: &mut World, player_oid: i32, client_id: u32, item_id: i32) -> i32 {
    let oid = super::items::add_inventory_item(world, player_oid, item_id, 1).unwrap()[0];
    super::items::use_equipable_item(world, client_id, player_oid, oid);
    oid
}

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

fn dlg_answer_body(message_id: i32, answer: i32, requester_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(message_id);
    w.write_i32(answer);
    w.write_i32(requester_id);
    w.into_bytes()
}

/// `true` if any packet is a SystemMessage with the given id.
fn has_system_message(pkts: &[Vec<u8>], id: i16) -> bool {
    pkts.iter().any(|p| {
        p[0] == server_packets::opcodes::SYSTEM_MESSAGE
            && p.len() >= 3
            && i16::from_le_bytes([p[1], p[2]]) == id
    })
}

/// The effect count in an `ExUserInfoAbnormalVisualEffect` packet (0xFE:0x158),
/// or `None` if not present. Layout: opcode(1)+sub(2)+objId(4)+transform(4)+count(4).
fn ave_effect_count(pkts: &[Vec<u8>]) -> Option<i32> {
    pkts.iter()
        .find(|p| {
            p[0] == server_packets::opcodes::EX
                && p.len() >= 15
                && i16::from_le_bytes([p[1], p[2]])
                    == server_packets::opcodes::EX_USER_INFO_ABNORMAL_VISUAL_EFFECT
        })
        .map(|p| i32::from_le_bytes([p[11], p[12], p[13], p[14]]))
}

/// The `EtcStatusUpdate` mask byte (0xF9), or `None` if not present. The mask
/// is the packet's last byte; bit 0x01 = message-refusal / silence.
fn etc_status_mask(pkts: &[Vec<u8>]) -> Option<u8> {
    pkts.iter().find(|p| p[0] == 0xF9).map(|p| p[p.len() - 1])
}

/// Build a `//command` (SendBypassBuildCmd) packet from a full command line.
fn build_admin(command_line: &str) -> Vec<u8> {
    [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body(command_line)].concat()
}

fn contains_utf16(pkt: &[u8], needle: &str) -> bool {
    let n: Vec<u8> = needle.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    pkt.windows(n.len()).any(|w| w == n)
}

fn user_cmd_body(id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(id);
    w.into_bytes()
}

/// A Teleporter NPC (template 30001) with one NORMAL destination charging
/// 9400 adena, `MaxFreeTeleportLevel = 40` (this dist), and a player holding
/// `adena` at (0,0) who already clicked the gatekeeper.
fn teleporter_world(adena: i64) -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, ..) = test_world();
    world.cfg.character.max_free_teleport_level = 40;
    world.id_pool = 0x5000_0000..0x5000_0100; // item oids for the seeded adena
    // Adena template so `add_inventory_item`/`take_items` can stack it.
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
        crystal_type: crate::data::item_data::CrystalType::None, crystal_count: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    });
    world.data.teleporters.insert_for_test(
        30001,
        crate::data::teleporter_data::TeleportHolder {
            name: "NORMAL".into(),
            teleport_type: crate::data::teleporter_data::TeleportType::Normal,
            locations: vec![crate::data::teleporter_data::TeleportLocation {
                x: 1000,
                y: 2000,
                z: -30,
                name: None,
                npc_string_id: 1010004,
                fee_id: 57,
                fee_count: 9400,
            }],
        },
    );
    add_test_npc(&mut world, NPC_OID, 30001, "Teleporter", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    if adena > 0 {
        super::items::add_inventory_item(&mut world, 3001, 57, adena);
    }
    drain(&mut rx);
    (world, rx)
}
