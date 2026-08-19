//! Result rendering: `Quest.showResult` / `showHtmlFile` / `getHtm`.

use super::QuestScript;
use crate::game_loop::helpers::send_action_failed;
use crate::game_loop::helpers::send_to_client;
use crate::network::server_packets;
use crate::world::World;
use std::sync::Arc;
use tracing::warn;
/// `Quest.showResult`: `.htm`/`.html` → html file; inline `<html>` → plain
/// window; other non-empty strings are Java `sendMessage` (unported — none
/// of the shipped scripts return one; logged).
pub(super) fn show_result(
    world: &mut World,
    client_id: u32,
    npc_oid: i32,
    script: &Arc<dyn QuestScript>,
    res: Option<String>,
) {
    let Some(res) = res else { return };
    if res.is_empty() {
        return;
    }
    if res.ends_with(".htm") || res.ends_with(".html") {
        show_html_file(world, client_id, npc_oid, script, &res);
    } else if res.starts_with("<html>") {
        let player_name = player_name_of_client(world, client_id);
        let content = res
            .replace("%objectId%", &npc_oid.to_string())
            .replace("%playername%", &player_name)
            .replace("%questname%", script.name());
        send_to_client(
            world,
            client_id,
            server_packets::npc_html_message(npc_oid, &content),
        );
        send_action_failed(world, client_id);
    } else {
        warn!(
            "Quest {}: plain-message result [{res}] (sendMessage unported).",
            script.name()
        );
    }
}

/// `Quest.showHtmlFile`: quest-window packet (`ExNpcQuestHtmlMessage`) for
/// `.htm` results of real quests (`0 < id < 20000`, id ≠ 999), plain
/// `NpcHtmlMessage` otherwise. Missing files send nothing, like Java's
/// null-content branch.
pub(super) fn show_html_file(
    world: &mut World,
    client_id: u32,
    npc_oid: i32,
    script: &Arc<dyn QuestScript>,
    filename: &str,
) {
    let quest_window = !filename.ends_with(".html");
    let viewer = world.player_oid(client_id).unwrap_or(0);
    let Some(content) = load_quest_html(world, viewer, script, filename) else {
        warn!("Quest {}: missing html [{filename}].", script.name());
        return;
    };
    let player_name = player_name_of_client(world, client_id);
    let content = content
        .replace("%objectId%", &npc_oid.to_string())
        .replace("%playername%", &player_name)
        // The shared Saga htmls are quest-agnostic; their bypass buttons carry
        // `%questname%` so one html set serves all 31 Sagas.
        .replace("%questname%", script.name());
    let id = script.id();
    if quest_window && id > 0 && id < 20000 && id != 999 {
        send_to_client(
            world,
            client_id,
            server_packets::ex_npc_quest_html_message(npc_oid, &content, id),
        );
    } else {
        send_to_client(
            world,
            client_id,
            server_packets::npc_html_message(npc_oid, &content),
        );
    }
    send_action_failed(world, client_id);
}

/// `Quest.getHtm`: the script's own folder, then the
/// `data/scripts/quests/<Name>/` fallback.
pub(super) fn load_quest_html(
    world: &World,
    viewer_oid: i32,
    script: &Arc<dyn QuestScript>,
    filename: &str,
) -> Option<String> {
    let root = &world.data.root;
    crate::data::htm_cache::read_htm_for(
        world,
        viewer_oid,
        format!("{root}data/scripts/{}/{filename}", script.html_dir()),
    )
    .or_else(|| {
        crate::data::htm_cache::read_htm_for(
            world,
            viewer_oid,
            format!("{root}data/scripts/quests/{}/{filename}", script.name()),
        )
    })
}

/// `Quest.getNoQuestMsg` (`data/html/noquest.htm`, with Java's inline
/// default when the file is missing).
pub(super) fn no_quest_html(world: &World, viewer_oid: i32) -> String {
    crate::data::htm_cache::read_htm_for(
        world,
        viewer_oid,
        format!("{}data/html/noquest.htm", world.data.root),
    )
        .unwrap_or_else(|| "<html><body>You are either not on a quest that involves this NPC, or you don't meet this NPC's minimum quest requirements.</body></html>".to_string())
}

pub(super) fn send_no_quest_html(world: &mut World, client_id: u32, npc_oid: i32) {
    let viewer = world.player_oid(client_id).unwrap_or(0);
    let content = no_quest_html(world, viewer).replace("%objectId%", &npc_oid.to_string());
    send_to_client(
        world,
        client_id,
        server_packets::npc_html_message(npc_oid, &content),
    );
}

pub(super) fn player_name_of_client(world: &World, client_id: u32) -> String {
    if let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) {
        world
            .objects
            .get_component::<crate::model::Player>(&session.player_object_id())
            .map(|p| p.name.clone())
            .unwrap_or_default()
    } else {
        String::new()
    }
}
