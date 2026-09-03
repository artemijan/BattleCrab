//! Bench-only surface for `benches/tick.rs` (`--features bench-api`).
//!
//! The per-tick systems are crate-private on purpose; the benchmarks need to
//! call them one at a time against a world populated from the real `dist`
//! data. This module re-exports exactly that — thin wrappers plus a world
//! builder mirroring the boot sequence in `game_loop::run` and the session
//! chain the test fixtures use. Feature-gated so nothing here exists in a
//! normal build; none of it is API.

use crate::game_loop::character::inventory;
use crate::game_loop::helpers::restore_hp_mp;
use crate::game_loop::space::position::{maybe_position, set_position};
use std::sync::Arc;

use crate::character::CharData;
use crate::data::GameData;
use crate::db;
use crate::game_loop::combat::pvp;
use crate::game_loop::net::broadcast;
use crate::loginlink::LoginLinkCommand;
use crate::model::Player;
use crate::model::components::{PlayerVitals, Position};
use crate::model::npc::Npc;
use crate::session::{ClientSession, Session, SessionKey};
use crate::world::World;

/// Datapack root of this repo's `dist` (trailing slash — the loaders append
/// `data/...` directly, same shape `main::resolve_datapack_root` produces).
pub const DIST_ROOT: &str = crate::data::DIST_GAME;

/// A `World` plus the channel ends the real services would hold. The
/// receivers must stay alive so `Session::send` / DB commands don't hit a
/// closed channel; [`BenchWorld::drain`] empties them between benches.
pub struct BenchWorld {
    pub world: World,
    client_rxs: Vec<tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>>,
    db_rx: db::CmdRx,
    link_rx: tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
}

/// Build a world from the real `dist` datapack + geodata and place the boot
/// spawns (the same static content `game_loop::run` starts with).
pub fn dist_world() -> BenchWorld {
    let (link_tx, link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, db_rx) = tokio::sync::mpsc::unbounded_channel();
    let data = GameData::load_from(DIST_ROOT);
    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);
    world.geo = Arc::new(crate::geo::GeoEngine::load(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/geodata"
    ))));
    crate::game_loop::npc::spawn_all(&mut world);
    crate::model::door::spawn_doors(&mut world);
    crate::model::static_object::spawn_static_objects(&mut world);
    BenchWorld {
        world,
        client_rxs: Vec::new(),
        db_rx,
        link_rx,
    }
}

impl BenchWorld {
    /// Put a fresh level-1 player in-game at (x, y, z) — the same
    /// `Session` state chain the login flow drives, so the client table,
    /// region index and ECS all see a real player.
    pub fn add_player(&mut self, client_id: u32, object_id: i32, x: i32, y: i32, z: i32) {
        let mut chr = bench_char(object_id, &format!("B{object_id}"));
        chr.x = x;
        chr.y = y;
        chr.z = z;
        let bundle = Player::from_char(&self.world.data, &chr);
        let (out_tx, out_rx) = tokio::sync::mpsc::unbounded_channel::<bytes::Bytes>();
        let out_tx = crate::network::OutboundTx::new(out_tx, false, 0);
        let s = Session::new(client_id, out_tx, "127.0.0.1:1".parse().unwrap())
            .into_authenticated("bench".into(), SessionKey::new(1, 2, 3, 4))
            .into_lobby(vec![])
            .into_entering(bundle);
        let (session, bundle) = s.into_ingame();
        bundle.spawn_into(&mut self.world);
        self.world
            .clients
            .insert(client_id, ClientSession::InGame(session));
        self.client_rxs.push(out_rx);
        // Top the pools up so regen's "already full" path is the one measured
        // unless a bench damages them on purpose.
        fill_vitals(&mut self.world, object_id);
    }

    /// Empty every held receiver (outbound packets, DB commands, login-link)
    /// so a long bench doesn't accumulate unbounded queues.
    pub fn drain(&mut self) {
        for rx in &mut self.client_rxs {
            while rx.try_recv().is_ok() {}
        }
        while self.db_rx.try_recv().is_ok() {}
        while self.link_rx.try_recv().is_ok() {}
    }
}

/// Set an object's HP/MP (and CP for players) to max.
pub fn fill_vitals(world: &mut World, object_id: i32) {
    restore_hp_mp(world, object_id);
    if let Some(pv) = world.objects.get_component_mut::<PlayerVitals>(&object_id) {
        pv.cur_cp = pv.max_cp as f64;
    }
}

/// World positions of up to `max` spawned monsters, taking every `stride`-th
/// NPC — spread-out anchors for placing bench players in hunting grounds.
pub fn monster_positions(world: &mut World, max: usize, stride: usize) -> Vec<(i32, i32, i32)> {
    let mut all: Vec<(i32, (i32, i32, i32))> = Vec::new();
    world
        .objects
        .for_each_mut::<(&Npc, &Position)>(|(n, p)| all.push((n.npc_id, (p.x, p.y, p.z))));
    let mut out = Vec::new();
    for (npc_id, pos) in all.into_iter().step_by(stride.max(1)) {
        if world
            .data
            .npc_data
            .get(npc_id)
            .is_some_and(|t| t.is_monster())
        {
            out.push(pos);
            if out.len() >= max {
                break;
            }
        }
    }
    out
}

/// Start a straight move toward `dest` through the real
/// `Creature.moveToLocation` tail (heading, speed-derived duration,
/// MoveToLocation broadcast).
pub fn begin_move(world: &mut World, client_id: u32, object_id: i32, dest: (i32, i32, i32)) {
    let Some(cur) = maybe_position(world, object_id) else {
        return;
    };
    super::space::position::start_move(world, client_id, object_id, cur, dest, None);
}

/// Move an object to (x, y, z) maintaining the region index + zone flags —
/// the invariant-preserving form of "write Position directly".
pub fn relocate(world: &mut World, object_id: i32, x: i32, y: i32, z: i32) {
    set_position(world, object_id, (x, y, z));
    super::space::visibility::update_region(world, object_id);
    super::space::zones::revalidate_zone(world, object_id, true);
}

// ---- the systems under measurement, in game-loop firing order ----

pub fn movement_tick(world: &mut World) {
    super::space::visibility::movement_tick(world);
}

pub fn npc_ai_tick(world: &mut World) {
    super::ai::npc_ai_tick(world);
}

pub fn stance_tick(world: &mut World) {
    super::combat::stance_tick(world);
}

pub fn pvp_flag_tick(world: &mut World) {
    pvp::pvp_flag_tick(world);
}

pub fn regen_tick(world: &mut World) {
    super::stats::regen::run_regen_tick(world);
}

pub fn npc_regen_tick(world: &mut World) {
    super::stats::regen::run_npc_regen_tick(world);
}

pub fn effect_zone_ticks(world: &mut World) {
    super::space::effect_zones::effect_zone_tick(world);
    super::space::effect_zones::damage_zone_tick(world);
}

pub fn weight_sweep(world: &mut World) {
    super::stats::weight::sweep(world);
}

pub fn drain_item_audit(world: &mut World) {
    inventory::drain_item_audit(world);
}

pub fn revalidate_zone(world: &mut World, object_id: i32) {
    super::space::zones::revalidate_zone(world, object_id, true);
}

pub fn broadcast_including_self(world: &World, object_id: i32, packet: &[u8]) {
    broadcast::broadcast_including_self(world, object_id, packet);
}

/// A `CharData` with every field a bench player needs and nothing else —
/// the same shape the test fixtures' `dummy_char` builds.
fn bench_char(object_id: i32, name: &str) -> CharData {
    CharData {
        object_id,
        name: name.into(),
        account_name: "bench".into(),
        level: 1,
        max_hp: 80,
        cur_hp: 80.0,
        max_mp: 30,
        cur_mp: 30.0,
        cur_cp: 0.0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        sex: 0,
        x: 1,
        y: 2,
        z: 3,
        exp: 0,
        sp: 0,
        lost_exp_on_death: 0,
        reputation: 0,
        pk_kills: 0,
        raidboss_points: 0,
        pvp_kills: 0,
        rec_have: 0,
        rec_left: 20,
        clan_id: 0,
        clan_privs: 0,
        clan_create_expiry_time: 0,
        clan_join_expiry_time: 0,
        create_date: "2026-01-15".to_string(),
        power_grade: 0,
        pledge_type: 0,
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
        subclasses: Vec::new(),
        lvl_joined_academy: 0,
        apprentice: 0,
        sponsor: 0,
        char_slot: 0,
        items: vec![],
        skills: vec![],
        skills_by_index: Default::default(),
        hennas_by_index: Default::default(),
        shortcuts_by_index: Default::default(),
        hennas: vec![],
        recipe_book: vec![],
        variables: vec![],
        pets: vec![],
        summons: vec![],
        shortcuts: vec![],
        macros: vec![],
        friends: vec![],
        quests: Default::default(),
        skill_reuses: vec![],
        skill_buffs: vec![],
    }
}
