//! `AdminEffects` — the broadcast-driven visual/environment commands
//! (`//social`, `//effect`, `//earthquake`, `//atmosphere`, `//play_sound`).
//!
//! The abnormal-visual-effect subset (`//invis`/`//para`/`//bighead`/…, teams,
//! `//settargetable`, `//playmovie`, `//event_trigger`, `//set_displayeffect`)
//! needs a per-creature AbnormalVisualEffect list / Team / targetable runtime
//! state this server does not model yet, so those stay deferred (still gated by
//! `AdminCommands.xml`, reaching the "not implemented" path).

use crate::model::components::Position;
use crate::model::Player;
use crate::network::server_packets::{self, sm_ids};
use crate::session::ClientSession;
use crate::world::World;

use super::{current_target, find_online_player, send_message, send_sm};

/// Whether `oid` is a `Creature` in Java terms — a player or an NPC (the only
/// creature kinds this server models; doors/static objects are not creatures).
fn is_creature(world: &World, oid: i32) -> bool {
    world.objects.has_component::<Player>(&oid)
        || world.objects.has_component::<crate::model::npc::Npc>(&oid)
}

/// Java `WorldObject.getName()` for GM feedback — player name, else the NPC
/// template name, else the object id.
fn object_name(world: &World, oid: i32) -> String {
    if let Some(p) = world.objects.get_component::<Player>(&oid) {
        return p.name.clone();
    }
    if let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&oid) {
        if let Some(t) = world.data.npc_data.get(npc.npc_id) {
            return t.name.clone();
        }
    }
    oid.to_string()
}

/// Port of `AdminEffects.performSocial` — broadcast a `SocialAction` on
/// `target`, gated by the same action-id ranges (NPCs 1..=20, players 2..=18 or
/// the level-up gesture). Returns whether the gesture was performed;
/// `NOTHING_HAPPENED` is sent to the GM on the out-of-range rejections exactly
/// as Java does inside this method.
fn perform_social(world: &World, action: i32, target: i32, gm_client_id: u32) -> bool {
    if !is_creature(world, target) {
        return false;
    }
    let is_npc = world.objects.has_component::<crate::model::npc::Npc>(&target);
    // (Java also rejects `Chest` NPCs outright; no Chest type exists here.)
    if is_npc && !(1..=20).contains(&action) {
        send_sm(world, gm_client_id, sm_ids::NOTHING_HAPPENED);
        return false;
    }
    if !is_npc
        && (action < 2 || (action > 18 && action != server_packets::SOCIAL_ACTION_LEVEL_UP))
    {
        send_sm(world, gm_client_id, sm_ids::NOTHING_HAPPENED);
        return false;
    }
    let packet = server_packets::social_action(target, action);
    super::helpers::broadcast_including_self(world, target, &packet);
    true
}

/// `AdminEffects`' `//social <id> [player_name|radius]` — play a social gesture
/// on the target/self, a named player, or every creature within a radius.
pub(super) fn admin_social(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    match args.len() {
        2 => {
            let Some(social) = args[0].parse::<i32>().ok() else { return };
            let who = args[1];
            if let Some(pid) = find_online_player(world, who) {
                if perform_social(world, social, pid, client_id) {
                    let name = object_name(world, pid);
                    send_message(world, client_id, &format!("{name} was affected by your request."));
                }
            } else if let Ok(radius) = who.parse::<i32>() {
                let Some(center) = world.objects.get_component::<Position>(&object_id).copied() else { return };
                for oid in creatures_in_range(world, &center, radius, object_id) {
                    perform_social(world, social, oid, client_id);
                }
                send_message(world, client_id, &format!("{radius} units radius affected by your request."));
            } else {
                send_message(world, client_id, "Incorrect parameter");
            }
        }
        1 => {
            let Some(social) = args[0].parse::<i32>().ok() else { return };
            let target = current_target(world, object_id).unwrap_or(object_id);
            if perform_social(world, social, target, client_id) {
                let name = object_name(world, target);
                send_message(world, client_id, &format!("{name} was affected by your request."));
            } else {
                send_sm(world, client_id, sm_ids::NOTHING_HAPPENED);
            }
        }
        _ => send_message(world, client_id, "Usage: //social <social_id> [player_name|radius]"),
    }
}

/// Every creature (player or NPC) within `radius` of `center`, excluding
/// `exclude` — Java `World.forEachVisibleObjectInRange(activeChar, …)`, which
/// omits the reference object itself.
fn creatures_in_range(world: &World, center: &Position, radius: i32, exclude: i32) -> Vec<i32> {
    let r = radius as f64;
    let mut out = Vec::new();
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let oid = s.player_object_id();
            if oid == exclude {
                continue;
            }
            if world.objects.get_component::<Position>(&oid).is_some_and(|p| center.distance_2d(p) <= r) {
                out.push(oid);
            }
        }
    }
    let region = crate::world::region_of(center.x, center.y);
    for oid in world.npcs_visible_from(region) {
        if world.objects.get_component::<Position>(&oid).is_some_and(|p| center.distance_2d(p) <= r) {
            out.push(oid);
        }
    }
    out
}

/// `AdminEffects`' `//effect` / `//npc_use_skill <skill> [level [hittime]]` —
/// broadcast a `MagicSkillUse` so the targeted creature (or the GM if none)
/// plays the skill's animation toward the GM. Purely cosmetic (no effects run).
pub(super) fn admin_effect(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //effect skill [level | level hittime]");
        return;
    };
    let level = args.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(1);
    let hit_time = args.get(2).and_then(|s| s.parse::<i32>().ok()).unwrap_or(1);
    // Java: obj = target, or self if none; must be a creature.
    let source = current_target(world, object_id).unwrap_or(object_id);
    if !is_creature(world, source) {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    }
    let (Some(src_pos), Some(gm_pos)) = (
        world.objects.get_component::<Position>(&source).copied(),
        world.objects.get_component::<Position>(&object_id).copied(),
    ) else {
        return;
    };
    let packet = server_packets::magic_skill_use_raw(
        (source, src_pos.x, src_pos.y, src_pos.z),
        (object_id, gm_pos.x, gm_pos.y, gm_pos.z),
        skill_id,
        level,
        hit_time,
    );
    super::helpers::broadcast_including_self(world, source, &packet);
    let name = object_name(world, source);
    send_message(world, client_id, &format!("{name} performs MSU {skill_id}/{level} by your request."));
}

/// `AdminEffects`' `//earthquake <intensity> <duration>` — a localised
/// screen-shake centred on the GM, broadcast to the surrounding regions.
pub(super) fn admin_earthquake(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(intensity), Some(duration)) = (
        args.first().and_then(|s| s.parse::<i32>().ok()),
        args.get(1).and_then(|s| s.parse::<i32>().ok()),
    ) else {
        send_message(world, client_id, "Usage: //earthquake <intensity> <duration>");
        return;
    };
    let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() else { return };
    let packet = server_packets::earthquake(pos.x, pos.y, pos.z, intensity, duration);
    super::helpers::broadcast_including_self(world, object_id, &packet);
}

/// `AdminEffects`' `//atmosphere <type> <state> <duration>` — port of
/// `adminAtmosphere`: only `sky day|night|red` is a real packet; the
/// `signsky` form is a no-op in Java too. Broadcast to *all* online players
/// (`Broadcast.toAllOnlinePlayers`), not just the surrounding regions.
pub(super) fn admin_atmosphere(world: &mut World, client_id: u32, args: &[&str]) {
    let usage = "Usage: //atmosphere <signsky dawn|dusk>|<sky day|night|red> <duration>";
    let (Some(&kind), Some(&state)) = (args.first(), args.get(1)) else {
        send_message(world, client_id, usage);
        return;
    };
    let duration = args.get(2).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
    let packet = if kind == "sky" {
        match state {
            "night" => Some(server_packets::sun_set()),
            "day" => Some(server_packets::sun_rise()),
            "red" => Some(server_packets::ex_red_sky(if duration != 0 { duration } else { 10 })),
            _ => None,
        }
    } else {
        None
    };
    let Some(packet) = packet else {
        send_message(world, client_id, usage);
        return;
    };
    for cs in world.clients.values() {
        if matches!(cs, ClientSession::InGame(_)) {
            cs.send(packet.clone());
        }
    }
}

/// `AdminEffects`' `//play_sound <name>` — play a client sound for the GM and
/// everyone who can see them.
pub(super) fn admin_play_sound(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(&sound) = args.first() else {
        send_message(world, client_id, "Usage: //play_sound <soundname>");
        return;
    };
    let packet = server_packets::play_sound(sound);
    super::helpers::broadcast_including_self(world, object_id, &packet);
    send_message(world, client_id, &format!("Playing {sound}."));
}
