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

// ---------------------------------------------------------------------------
// AdminEffects' G19 tail (PLAN: close the milestone's unblock list): teams,
// targetable, GM paralysis, big head, cinematics, event triggers, NPC display
// state. Java: handlers/admincommandhandlers/AdminEffects.java.
// ---------------------------------------------------------------------------

/// `//setteam <none|blue|red>` (current target) and `//setteam_close <team>
/// [radius=400]` (players around the GM). Player targets only — the port's
/// NpcInfo doesn't model the team block yet (TODO(G19)).
pub(super) fn admin_setteam(world: &mut World, client_id: u32, object_id: i32, args: &[&str], close: bool) {
    let Some(team) = args.first().and_then(|v| match v.to_lowercase().as_str() {
        "none" => Some(0u8),
        "blue" => Some(1),
        "red" => Some(2),
        _ => None,
    }) else {
        send_message(world, client_id, "Usage: //setteam <none|blue|red>");
        return;
    };
    let targets: Vec<i32> = if close {
        let radius = args.get(1).and_then(|r| r.parse::<i32>().ok()).unwrap_or(400) as f64;
        let Some(origin) = world.objects.get_component::<Position>(&object_id).copied() else { return };
        players_in_radius(world, &origin, radius)
    } else {
        vec![current_target(world, object_id).unwrap_or(object_id)]
    };
    let mut set = 0;
    for target in targets {
        if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
            p.team = team;
            set += 1;
            crate::game_loop::party::broadcast_user_info(world, target);
        }
    }
    send_message(world, client_id, &format!("Team set on {set} player(s)."));
}

/// `//clearteams` — every visible player back to NONE.
pub(super) fn admin_clearteams(world: &mut World, client_id: u32, object_id: i32) {
    let Some(origin) = world.objects.get_component::<Position>(&object_id).copied() else { return };
    // "Visible" ≈ the same broadcast radius the packet fan-out uses; a large
    // sweep is fine for a GM tool.
    let targets = players_in_radius(world, &origin, 10_000.0);
    for target in targets {
        if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
            if p.team != 0 {
                p.team = 0;
                crate::game_loop::party::broadcast_user_info(world, target);
            }
        }
    }
    send_message(world, client_id, "Teams cleared.");
}

fn players_in_radius(world: &World, origin: &Position, radius: f64) -> Vec<i32> {
    world
        .clients
        .values()
        .filter_map(|cs| match cs {
            ClientSession::InGame(s) => Some(s.player_object_id()),
            _ => None,
        })
        .filter(|oid| {
            world.objects.get_component::<Position>(oid).is_some_and(|p| {
                let (dx, dy) = ((p.x - origin.x) as f64, (p.y - origin.y) as f64);
                (dx * dx + dy * dy).sqrt() <= radius
            })
        })
        .collect()
}

/// `//settargetable` — toggle whether the GM can be selected (Java toggles
/// `activeChar` itself, not the target).
pub(super) fn admin_settargetable(world: &mut World, client_id: u32, object_id: i32) {
    let mut flags = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&object_id)
        .copied()
        .unwrap_or_default();
    flags.untargetable = !flags.untargetable;
    let off = flags.untargetable;
    world.objects.add_components(&object_id, flags);
    send_message(world, client_id, if off { "You are now untargetable." } else { "You are targetable again." });
}

/// `//para [type]` / `//unpara [type]` on the current target, and the `_all`
/// variants over nearby players. Type 1 draws PARALYZE, anything else
/// FLESH_STONE (Java's split); the block itself is `AdminFlags.paralyzed`.
pub(super) fn admin_para(world: &mut World, client_id: u32, object_id: i32, args: &[&str], on: bool, all: bool) {
    let ave_name = if args.first().copied().unwrap_or("1") == "1" { "PARALYZE" } else { "FLESH_STONE" };
    let ave = crate::model::skill::abnormal_visual_client_id(ave_name).expect("known AVE");
    let targets: Vec<i32> = if all {
        let Some(origin) = world.objects.get_component::<Position>(&object_id).copied() else { return };
        players_in_radius(world, &origin, 10_000.0)
    } else {
        vec![current_target(world, object_id).unwrap_or(object_id)]
    };
    for target in &targets {
        let mut flags = world
            .objects
            .get_component::<crate::model::components::AdminFlags>(target)
            .copied()
            .unwrap_or_default();
        flags.paralyzed = on;
        world.objects.add_components(target, flags);
        set_admin_visual(world, *target, ave, on);
        crate::game_loop::party::broadcast_user_info(world, *target);
    }
    send_message(
        world,
        client_id,
        &format!("{} {} target(s).", if on { "Paralyzed" } else { "Released" }, targets.len()),
    );
}

/// `//bighead` / `//shrinkhead` — the BIG_HEAD abnormal visual on the target.
pub(super) fn admin_bighead(world: &mut World, client_id: u32, object_id: i32, on: bool) {
    let ave = crate::model::skill::abnormal_visual_client_id("BIG_HEAD").expect("known AVE");
    let target = current_target(world, object_id).unwrap_or(object_id);
    set_admin_visual(world, target, ave, on);
    crate::game_loop::party::broadcast_user_info(world, target);
    send_message(world, client_id, if on { "Big head on." } else { "Big head off." });
}

/// Pin/unpin one GM abnormal visual (the `//ave_abnormal` storage).
fn set_admin_visual(world: &mut World, target: i32, ave: i16, on: bool) {
    use crate::model::components::AdminVisuals;
    match world.objects.get_component_mut::<AdminVisuals>(&target) {
        Some(v) => {
            if on {
                if !v.0.contains(&ave) {
                    v.0.push(ave);
                }
            } else {
                v.0.retain(|&x| x != ave);
            }
        }
        None if on => {
            world.objects.add_components(&target, AdminVisuals(vec![ave]));
        }
        None => {}
    }
}

/// `//playmovie <id>` — play a client cinematic for the GM. The MovieHolder
/// bookkeeping (movement freeze, escape handling) is TODO(G19); this is the
/// preview tool.
pub(super) fn admin_playmovie(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(id) = args.first().and_then(|v| v.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //playmovie <id>");
        return;
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::enter_world::ex_start_scene_player(id));
    }
}

/// `//event_trigger <id> [true|false]` — toggle a client emitter for everyone
/// nearby (Java fans out to visible players plus the GM).
pub(super) fn admin_event_trigger(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(id), enabled) = (
        args.first().and_then(|v| v.parse::<i32>().ok()),
        args.get(1).is_some_and(|v| v.eq_ignore_ascii_case("true")),
    ) else {
        send_message(world, client_id, "Usage: //event_trigger id [true | false]");
        return;
    };
    let pkt = crate::network::enter_world::event_trigger(id, enabled);
    super::helpers::broadcast_including_self(world, object_id, &pkt);
}

/// `//set_displayeffect <state>` — an NPC target's display-effect state
/// (`ExChangeNpcState`). Broadcast-only: the state isn't stored, so a fresh
/// observer won't see it (TODO(G19) — needs an NpcInfo field).
pub(super) fn admin_set_displayeffect(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(state) = args.first().and_then(|v| v.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //set_displayeffect <id>");
        return;
    };
    let Some(target) = current_target(world, object_id) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    if !world.objects.has_component::<crate::model::npc::Npc>(&target) {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    }
    let pkt = crate::network::enter_world::ex_change_npc_state(target, state);
    super::helpers::broadcast_including_self(world, object_id, &pkt);
}
