//! The game server's answer to the monitor channel's `clients` and `kick`
//! requests (`docs/MONITORING.md` §10): one record per session in
//! `world.clients`, built on the game thread, which owns them; and the
//! dashboard's "disconnect" button.
//!
//! The request arrives as a [`GameEvent::Monitor`](crate::events::GameEvent)
//! and is answered in the same drain, so the dashboard pays one event's latency
//! and the game thread pays nothing while nobody is looking.

use commons::monitor::clients::ClientRecord;
use serde_json::{Map, Value, json};

use crate::events::MonitorRequest;
use crate::game_loop::space::position::maybe_position;
use crate::model::Player;
use crate::model::components;
use crate::network::client_packets::session::HardwareInfo;
use crate::session::ClientSession;
use crate::world::World;

pub(crate) fn answer(world: &mut World, request: MonitorRequest) {
    // The channel may have given up waiting; nothing to do about that here.
    match request {
        MonitorRequest::Clients(reply) => {
            let _ = reply.send(client_records(world));
        }
        MonitorRequest::Kick {
            id,
            connected_ms,
            reply,
        } => {
            let _ = reply.send(kick(world, id, connected_ms));
        }
    }
}

/// The dashboard's disconnect: the same teardown as a flood-protector kick.
/// The dashboard records who asked for it in the GM audit log.
pub(crate) fn kick(world: &mut World, id: u64, connected_ms: u64) -> bool {
    let Ok(client_id) = u32::try_from(id) else {
        return false;
    };
    let Some(session) = world.clients.get(&client_id) else {
        return false;
    };
    if session.out().stats().connected_ms() != connected_ms {
        return false;
    }
    crate::game_loop::helpers::kick_client(world, client_id);
    true
}

pub(crate) fn client_records(world: &World) -> Vec<ClientRecord> {
    let mut records: Vec<ClientRecord> = world
        .clients
        .values()
        .map(|session| record(world, session))
        .collect();
    records.sort_by_key(|r| r.id);
    records
}

fn record(world: &World, session: &ClientSession) -> ClientRecord {
    let client_id = session.client_id();
    let addr = session.addr();
    let stats = session.out().stats();
    let mut details = Map::new();
    if let Some(version) = world.protocol_versions.get(&client_id) {
        details.insert("protocolVersion".into(), json!(version));
    }
    let hardware = world.hwids.get(&client_id);
    if let Some(hw) = hardware {
        details.insert("hardware".into(), hardware_details(hw));
    }
    // The same split as the `sessions_*` gauges (`refresh_session_gauges`).
    let (stage, character) = match session {
        ClientSession::Connecting(_) | ClientSession::Authenticated(_) => ("authenticating", None),
        ClientSession::InLobby(s) => {
            let chars: Vec<Value> = s
                .state
                .chars
                .iter()
                .map(|c| json!({ "name": c.name, "level": c.level, "classId": c.class_id }))
                .collect();
            details.insert("lobbyCharacters".into(), Value::Array(chars));
            ("lobby", None)
        }
        ClientSession::Entering(s) => ("entering", Some(s.state.player.player.name.clone())),
        ClientSession::InGame(s) => {
            let object_id = s.state.player_object_id;
            let name = world
                .objects
                .get_component::<Player>(&object_id)
                .map(|p| p.name.clone());
            if let Some(character) = character_details(world, object_id) {
                details.insert("character".into(), character);
            }
            ("in_game", name)
        }
    };
    ClientRecord {
        service: String::new(),
        id: u64::from(client_id),
        ip: addr.ip().to_string(),
        port: addr.port(),
        connected_ms: stats.connected_ms(),
        stage: stage.to_string(),
        account: session.account().map(str::to_string),
        character,
        hwid: hardware.map(|h| h.mac_address.clone()),
        traffic: stats.traffic(),
        details,
    }
}

/// What `RequestHardWareInfo` reported. The MAC address is the HWID itself
/// and is on the record already.
fn hardware_details(hw: &HardwareInfo) -> Value {
    json!({
        "cpu": hw.cpu_name,
        "cpuSpeedMhz": hw.cpu_speed,
        "cpuCores": hw.cpu_core_count,
        "gpu": hw.vga_name,
        "gpuDriver": hw.vga_driver_version,
        "windows": format!(
            "{}.{} build {} (platform {})",
            hw.windows_major_version,
            hw.windows_minor_version,
            hw.windows_build_number,
            hw.windows_platform_id
        ),
    })
}

/// The in-world character, roughly what `//charinfo` shows a GM: who it is,
/// where it stands and how it's doing.
fn character_details(world: &World, object_id: i32) -> Option<Value> {
    let p = world.objects.get_component::<Player>(&object_id)?;
    let clan = (p.clan_id != 0).then(|| {
        world
            .clans
            .get(&p.clan_id)
            .map_or_else(|| format!("#{}", p.clan_id), |c| c.name.clone())
    });
    let position = maybe_position(world, object_id);
    let vitals = world
        .objects
        .get_component::<components::stats::Vitals>(&object_id);
    let cp = world
        .objects
        .get_component::<components::stats::PlayerVitals>(&object_id);
    Some(json!({
        "objectId": object_id,
        "name": p.name,
        "title": p.title,
        "level": p.level,
        "classId": p.class_id,
        "baseClassId": p.base_class_id,
        "race": p.race,
        "clan": clan,
        "accessLevel": p.access_level,
        "hero": p.is_hero,
        "noble": p.is_noble,
        "reputation": p.reputation,
        "pvpKills": p.pvp_kills,
        "pkKills": p.pk_kills,
        "position": position.map(|pos| json!({ "x": pos.x, "y": pos.y, "z": pos.z })),
        "hp": vitals.map(|v| json!({ "cur": v.cur_hp as i64, "max": v.max_hp })),
        "mp": vitals.map(|v| json!({ "cur": v.cur_mp as i64, "max": v.max_mp })),
        "cp": cp.map(|v| json!({ "cur": v.cur_cp as i64, "max": v.max_cp })),
        "dead": vitals.is_some_and(|v| v.dead),
    }))
}
