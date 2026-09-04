//! `AdminMissingHtmls` — the datapack audit that finds talkable NPCs with no
//! dialog file of their own.
//!
//! A builder tool: walk the spawned NPCs, ask each whether clicking it would
//! produce a real page, and list the ids that would fall back to
//! `npcdefault.htm` (or, for the subclassed folk, find nothing at all).
//!
//! Three commands, all Java's: `//geomap_missing_htmls` scopes to the geodata
//! tile the GM is standing in, `//world_missing_htmls` sweeps everything, and
//! `//next_missing_html` teleports the GM to the first offender so they can
//! look at it.

use crate::game_loop::space::position::maybe_position;
use crate::world::World;

use super::send_message;

/// Java's exclusion list, by this port's `type_name`. Monsters and artefacts
/// are not talkable folk; broadcasting towers and fly-terrain objects are
/// scenery that happens to be an NPC.
fn is_excluded_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "Artefact" | "BroadcastingTower" | "FlyTerrainObject"
    )
}

/// Does clicking this NPC land on a real page?
///
/// Java asks two different questions depending on the subclass, and they are
/// the same question: plain `Folk`/`Npc` resolve through `data/html/default/`
/// and *fall back* to `npcdefault.htm`, so the test is whether the path came
/// back as that fallback; the subclassed folk (Merchant, Fisherman, Warehouse,
/// Guard) root in their own directory with **no** fallback, so the test is
/// whether the file exists. Either way: is there a page named after this npc
/// id in the directory its type routes to?
fn has_own_html(root: &str, type_name: &str, npc_id: i32) -> bool {
    let dir = match type_name {
        "Merchant" => "merchant",
        "Fisherman" => "fisherman",
        "Teleporter" => "teleporter",
        "Warehouse" => "warehouse",
        "Guard" => "guard",
        "PetManager" => "petmanager",
        t if t.starts_with("VillageMaster") => "villagemaster",
        _ => "default",
    };
    crate::data::htm_cache::read_htm(format!("{root}data/html/{dir}/{npc_id}.htm")).is_some()
}

/// Every spawned NPC that would open a page it does not have: `(npc_id, x, y,
/// z)` of the first instance of each id, sorted by id like Java's
/// `Collections.sort(results)`.
///
/// `bounds` is `(min_x, min_y, max_x, max_y)` when scoping to a geodata tile.
/// Java compares strictly (`>` / `<`), so an NPC exactly on the tile edge is
/// excluded; kept as written.
fn scan(world: &mut World, bounds: Option<(i32, i32, i32, i32)>) -> Vec<(i32, i32, i32, i32)> {
    use std::collections::BTreeMap;
    let mut found: BTreeMap<i32, (i32, i32, i32)> = BTreeMap::new();
    let mut npcs: Vec<i32> = Vec::new();
    world
        .objects
        .for_each_mut::<&crate::model::npc::Npc>(|n| npcs.push(n.object_id));

    for oid in npcs {
        let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&oid) else {
            continue;
        };
        let npc_id = npc.npc_id;
        if found.contains_key(&npc_id) {
            continue; // `!results.contains(obj.getId())`
        }
        let Some(t) = world.data.npc_data.get(npc_id) else {
            continue;
        };
        if t.is_monster() || is_excluded_type(&t.type_name) || !t.talkable {
            continue;
        }
        // `!npc.hasListener(ON_NPC_FIRST_TALK)` — a script owning the chat
        // window supplies the page itself, so a missing file is not a gap.
        if world.quests.first_talk_quest(npc_id).is_some() {
            continue;
        }
        let Some(pos) = world
            .objects
            .get_component::<crate::model::components::space::Position>(&oid)
        else {
            continue;
        };
        if let Some((min_x, min_y, max_x, max_y)) = bounds
            && !(pos.x > min_x && pos.x < max_x && pos.y > min_y && pos.y < max_y)
        {
            continue;
        }
        if !has_own_html(&world.data.root, &t.type_name, npc_id) {
            found.insert(npc_id, (pos.x, pos.y, pos.z));
        }
    }
    found
        .into_iter()
        .map(|(id, (x, y, z))| (id, x, y, z))
        .collect()
}

/// `//geomap_missing_htmls` — the geodata tile the GM is standing in.
pub(super) fn admin_geomap_missing_htmls(world: &mut World, client_id: u32, object_id: i32) {
    let Some(pos) = maybe_position(world, object_id) else {
        return;
    };
    let ((tx, ty), (min_x, min_y), (max_x, max_y)) = world.geo.geomap_tile(pos.x, pos.y);
    send_message(
        world,
        client_id,
        &format!("GeoMap: {tx}_{ty} ({min_x},{min_y} to {max_x},{max_y})"),
    );
    let results = scan(world, Some((min_x, min_y, max_x, max_y)));
    report(world, client_id, &results);
}

/// `//world_missing_htmls` — everything spawned.
pub(super) fn admin_world_missing_htmls(world: &mut World, client_id: u32) {
    send_message(world, client_id, "Missing htmls for the whole world.");
    let results = scan(world, None);
    report(world, client_id, &results);
}

/// `//next_missing_html` — teleport to the first offender.
pub(super) fn admin_next_missing_html(world: &mut World, client_id: u32, object_id: i32) {
    let results = scan(world, None);
    let Some(&(npc_id, x, y, z)) = results.first() else {
        return; // Java's loop simply finds nothing and returns
    };
    crate::game_loop::death::teleport_player(world, object_id, x, y, z);
    send_message(
        world,
        client_id,
        &format!("NPC {npc_id} does not have a default html."),
    );
}

fn report(world: &mut World, client_id: u32, results: &[(i32, i32, i32, i32)]) {
    for &(npc_id, ..) in results {
        send_message(
            world,
            client_id,
            &format!("NPC {npc_id} does not have a default html."),
        );
    }
    send_message(
        world,
        client_id,
        &format!("Found {} results.", results.len()),
    );
}

#[doc(hidden)]
#[cfg(test)]
pub(crate) fn scan_for_test(
    world: &mut World,
    bounds: Option<(i32, i32, i32, i32)>,
) -> Vec<(i32, i32, i32, i32)> {
    scan(world, bounds)
}
