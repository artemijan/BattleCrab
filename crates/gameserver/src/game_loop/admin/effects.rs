//! `AdminEffects` — the broadcast-driven visual/environment commands
//! (`//social`, `//effect`, `//earthquake`, `//atmosphere`, `//play_sound`).
//!
//! The abnormal-visual-effect subset (`//invis`/`//para`/`//bighead`/…, teams,
//! `//settargetable`, `//event_trigger`, `//set_displayeffect`)
//! needs a per-creature AbnormalVisualEffect list / Team / targetable runtime
//! state this server does not model yet, so those stay deferred (still gated by
//! `AdminCommands.xml`, reaching the "not implemented" path). `//playmovie`
//! carries the full `MovieHolder` bookkeeping (see [`admin_playmovie`]).

use crate::game_loop::admin::find_online_player;
use crate::game_loop::guard;
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::{is_creature, nth_arg, object_name};
use crate::game_loop::helpers::{send_message, send_sm_bare_to_client, send_to_client};
use crate::geo::distance::within_2d_xy;
use crate::model::Player;
use crate::model::components::Position;
use crate::network::server_packets::{self, sm_ids};
use crate::session::ClientSession;
use crate::world::World;

/// Port of `AdminEffects.performSocial` — broadcast a `SocialAction` on
/// `target`, gated by the same action-id ranges (NPCs 1..=20, players 2..=18 or
/// the level-up gesture). Returns whether the gesture was performed;
/// `NOTHING_HAPPENED` is sent to the GM on the out-of-range rejections exactly
/// as Java does inside this method.
fn perform_social(world: &World, action: i32, target: i32, gm_client_id: u32) -> bool {
    if !is_creature(world, target) {
        return false;
    }
    let is_npc = world
        .objects
        .has_component::<crate::model::npc::Npc>(&target);
    // (Java also rejects `Chest` NPCs outright; no Chest type exists here.)
    if is_npc && !(1..=20).contains(&action) {
        send_sm_bare_to_client(world, gm_client_id, sm_ids::NOTHING_HAPPENED);
        return false;
    }
    if !is_npc && (action < 2 || (action > 18 && action != server_packets::SOCIAL_ACTION_LEVEL_UP))
    {
        send_sm_bare_to_client(world, gm_client_id, sm_ids::NOTHING_HAPPENED);
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
            let Some(social) = args[0].parse::<i32>().ok() else {
                return;
            };
            let who = args[1];
            if let Some(pid) = find_online_player(world, who) {
                if perform_social(world, social, pid, client_id) {
                    let name = object_name(world, pid);
                    send_message(
                        world,
                        client_id,
                        &format!("{name} was affected by your request."),
                    );
                }
            } else if let Ok(radius) = who.parse::<i32>() {
                let Some(center) = maybe_position(world, object_id) else {
                    return;
                };
                for oid in creatures_in_range(world, &center, radius, object_id) {
                    perform_social(world, social, oid, client_id);
                }
                send_message(
                    world,
                    client_id,
                    &format!("{radius} units radius affected by your request."),
                );
            } else {
                send_message(world, client_id, "Incorrect parameter");
            }
        }
        1 => {
            let Some(social) = args[0].parse::<i32>().ok() else {
                return;
            };
            let target = guard::target(world, object_id).unwrap_or(object_id);
            if perform_social(world, social, target, client_id) {
                let name = object_name(world, target);
                send_message(
                    world,
                    client_id,
                    &format!("{name} was affected by your request."),
                );
            } else {
                send_sm_bare_to_client(world, client_id, sm_ids::NOTHING_HAPPENED);
            }
        }
        _ => send_message(
            world,
            client_id,
            "Usage: //social <social_id> [player_name|radius]",
        ),
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
            if world
                .objects
                .get_component::<Position>(&oid)
                .is_some_and(|p| center.distance_2d(p) <= r)
            {
                out.push(oid);
            }
        }
    }
    let region = crate::world::region_of(center.x, center.y);
    for oid in world.npcs_visible_from(region) {
        if world
            .objects
            .get_component::<Position>(&oid)
            .is_some_and(|p| center.distance_2d(p) <= r)
        {
            out.push(oid);
        }
    }
    out
}

/// `AdminEffects`' `//effect` / `//npc_use_skill <skill> [level [hittime]]` —
/// broadcast a `MagicSkillUse` so the targeted creature (or the GM if none)
/// plays the skill's animation toward the GM. Purely cosmetic (no effects run).
pub(super) fn admin_effect(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = nth_arg::<i32>(args, 0) else {
        send_message(
            world,
            client_id,
            "Usage: //effect skill [level | level hittime]",
        );
        return;
    };
    let level = nth_arg::<i32>(args, 1).unwrap_or(1);
    let hit_time = nth_arg::<i32>(args, 2).unwrap_or(1);
    // Java: obj = target, or self if none; must be a creature.
    let source = guard::target(world, object_id).unwrap_or(object_id);
    if !is_creature(world, source) {
        send_sm_bare_to_client(world, client_id, sm_ids::INVALID_TARGET);
        return;
    }
    let (Some(src_pos), Some(gm_pos)) = (
        maybe_position(world, source),
        maybe_position(world, object_id),
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
    send_message(
        world,
        client_id,
        &format!("{name} performs MSU {skill_id}/{level} by your request."),
    );
}

/// `AdminEffects`' `//earthquake <intensity> <duration>` — a localised
/// screen-shake centred on the GM, broadcast to the surrounding regions.
pub(super) fn admin_earthquake(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(intensity), Some(duration)) = (nth_arg::<i32>(args, 0), nth_arg::<i32>(args, 1))
    else {
        send_message(
            world,
            client_id,
            "Usage: //earthquake <intensity> <duration>",
        );
        return;
    };
    let Some(pos) = maybe_position(world, object_id) else {
        return;
    };
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
    let duration = nth_arg::<i32>(args, 2).unwrap_or(0);
    let packet = if kind == "sky" {
        match state {
            "night" => Some(server_packets::sun_set()),
            "day" => Some(server_packets::sun_rise()),
            "red" => Some(server_packets::ex_red_sky(if duration != 0 {
                duration
            } else {
                10
            })),
            _ => None,
        }
    } else {
        None
    };
    let Some(packet) = packet else {
        send_message(world, client_id, usage);
        return;
    };
    world.broadcast_to_all_online(&packet);
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
/// [radius=400]` (players around the GM). Java's single-target form takes any
/// `Creature`, so an **NPC** target gets the aura too (`NpcInfo`'s `TEAM`
/// block); the `_close` sweep is players-only, as it is in Java.
pub(super) fn admin_setteam(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
    close: bool,
) {
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
        let radius = nth_arg::<i32>(args, 1).unwrap_or(400) as f64;
        let Some(origin) = maybe_position(world, object_id) else {
            return;
        };
        players_in_radius(world, &origin, radius)
    } else {
        vec![guard::target(world, object_id).unwrap_or(object_id)]
    };
    let mut set = 0;
    for target in targets {
        if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
            p.team = team;
            set += 1;
            crate::game_loop::player_info::broadcast_user_info(world, target);
        } else if let Some(n) = world
            .objects
            .get_component_mut::<crate::model::npc::Npc>(&target)
        {
            n.team = team;
            set += 1;
            // The aura rides `NpcInfo`, so the whole packet is re-sent.
            super::death::introduce_npc(world, target);
        }
    }
    send_message(world, client_id, &format!("Team set on {set} target(s)."));
}

/// `//clearteams` — every visible player back to NONE.
pub(super) fn admin_clearteams(world: &mut World, client_id: u32, object_id: i32) {
    let Some(origin) = maybe_position(world, object_id) else {
        return;
    };
    // "Visible" ≈ the same broadcast radius the packet fan-out uses; a large
    // sweep is fine for a GM tool.
    let targets = players_in_radius(world, &origin, 10_000.0);
    for target in targets {
        if let Some(p) = world.objects.get_component_mut::<Player>(&target)
            && p.team != 0
        {
            p.team = 0;
            crate::game_loop::player_info::broadcast_user_info(world, target);
        }
    }
    send_message(world, client_id, "Teams cleared.");
}

fn players_in_radius(world: &World, origin: &Position, radius: f64) -> Vec<i32> {
    world
        .in_game_player_oids()
        .filter(|&oid| within_2d_xy(world, oid, origin.x, origin.y, radius))
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
    send_message(
        world,
        client_id,
        if off {
            "You are now untargetable."
        } else {
            "You are targetable again."
        },
    );
}

/// `//para [type]` / `//unpara [type]` on the current target, and the `_all`
/// variants over nearby players. Type 1 draws PARALYZE, anything else
/// FLESH_STONE (Java's split); the block itself is `AdminFlags.paralyzed`.
pub(super) fn admin_para(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
    on: bool,
    all: bool,
) {
    let ave_name = if args.first().copied().unwrap_or("1") == "1" {
        "PARALYZE"
    } else {
        "FLESH_STONE"
    };
    let ave = crate::model::skill::abnormal_visual_client_id(ave_name).expect("known AVE");
    let targets: Vec<i32> = if all {
        let Some(origin) = maybe_position(world, object_id) else {
            return;
        };
        players_in_radius(world, &origin, 10_000.0)
    } else {
        vec![guard::target(world, object_id).unwrap_or(object_id)]
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
        // Java's `startAbnormalVisualEffect`/`stopAbnormalVisualEffect` end in
        // `updateAbnormalVisualEffects()`, which sends the owner their own
        // `ExUserInfoAbnormalVisualEffect` on top of the `CharInfo` broadcast.
        // A `UserInfo` alone left the paralysis with no visual (GitHub #10).
        super::flags::push_admin_visuals(world, *target);
    }
    send_message(
        world,
        client_id,
        &format!(
            "{} {} target(s).",
            if on { "Paralyzed" } else { "Released" },
            targets.len()
        ),
    );
}

/// `//bighead` / `//shrinkhead` — the BIG_HEAD abnormal visual on the target.
pub(super) fn admin_bighead(world: &mut World, client_id: u32, object_id: i32, on: bool) {
    let ave = crate::model::skill::abnormal_visual_client_id("BIG_HEAD").expect("known AVE");
    let target = guard::target(world, object_id).unwrap_or(object_id);
    set_admin_visual(world, target, ave, on);
    super::flags::push_admin_visuals(world, target);
    send_message(
        world,
        client_id,
        if on { "Big head on." } else { "Big head off." },
    );
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
            world
                .objects
                .add_components(&target, AdminVisuals(vec![ave]));
        }
        None => {}
    }
}

/// Java's `Movie` enum as `(client_id, isEscapable)` rows — the ids
/// `Movie.findByClientId` accepts. An id not in this table is refused with
/// the usage line, matching Java's `AdminEffects` catch around the lookup.
const MOVIES: &[(i32, bool)] = &[
    (1, true),
    (2, true),
    (3, true),
    (4, true),
    (5, true),
    (6, true),
    (7, true),
    (8, true),
    (9, true),
    (10, true),
    (11, true),
    (12, true),
    (13, true),
    (14, true),
    (15, false),
    (16, true),
    (17, true),
    (18, false),
    (19, false),
    (20, false),
    (21, true),
    (22, true),
    (23, false),
    (24, true),
    (25, true),
    (26, false),
    (27, true),
    (28, false),
    (29, false),
    (30, false),
    (31, true),
    (32, false),
    (33, true),
    (34, true),
    (35, true),
    (36, false),
    (37, true),
    (38, true),
    (42, false),
    (43, false),
    (44, false),
    (45, false),
    (46, false),
    (47, false),
    (48, false),
    (49, false),
    (50, true),
    (51, true),
    (52, true),
    (53, false),
    (54, true),
    (55, true),
    (56, false),
    (57, false),
    (58, false),
    (59, false),
    (69, true),
    (70, true),
    (71, false),
    (72, true),
    (73, false),
    (74, false),
    (75, false),
    (76, false),
    (77, true),
    (78, true),
    (79, true),
    (80, true),
    (81, false),
    (99, true),
    (100, true),
    (101, true),
    (102, true),
    (103, true),
    (104, true),
    (105, true),
    (106, true),
    (107, false),
    (108, false),
    (109, false),
    (110, false),
    (111, false),
    (112, true),
    (113, true),
    (114, true),
    (115, true),
    (116, false),
    (117, false),
    (1000, true),
    (1001, true),
    (1002, true),
    (1003, true),
    (1004, true),
    (2001, false),
    (2002, false),
];

/// `//playmovie <id>` — play a client cinematic for the GM, with Java's
/// `Player.playMovie` bookkeeping: refused while one is already running,
/// aborts the swing (but **not** a cast — "Confirmed in retail"), stops
/// movement, and remembers the `MovieHolder` state so `EndScenePlayer` /
/// `RequestExEscapeScene` can end it. `ExStartScenePlayer` is skipped while
/// teleporting, as in Java.
pub(super) fn admin_playmovie(world: &mut World, client_id: u32, args: &[&str]) {
    use crate::model::components::InMovie;

    let movie =
        nth_arg::<i32>(args, 0).and_then(|id| MOVIES.iter().find(|&&(mid, _)| mid == id).copied());
    let Some((movie_id, escapable)) = movie else {
        send_message(world, client_id, "Usage: //playmovie <id>");
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    // `if (_movieHolder != null) return;`
    if world.objects.has_component::<InMovie>(&object_id) {
        return;
    }
    crate::game_loop::combat::abort_attack(world, object_id);
    crate::game_loop::position::handle_request_stop_move(world, client_id);
    world.objects.add_components(
        &object_id,
        InMovie {
            movie_id,
            escapable,
        },
    );
    let teleporting = world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.teleporting);
    if !teleporting {
        send_to_client(
            world,
            client_id,
            crate::network::enter_world::ex_start_scene_player(movie_id),
        );
    }
}

/// `EndScenePlayer` (ex 0x58) — the client's own notice that the cinematic
/// finished. Java: ignored unless the echoed id matches the running movie,
/// then `stopMovie()` — which also answers `ExStopScenePlayer`, harmless for
/// a scene the client already ended.
pub(crate) fn handle_end_scene_player(world: &mut World, client_id: u32, ex_body: &[u8]) {
    let mut r = commons::network::PacketReader::new(ex_body);
    let Some(movie_id) = r.read_i32() else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    let matches = world
        .objects
        .get_component::<crate::model::components::InMovie>(&object_id)
        .is_some_and(|m| movie_id != 0 && m.movie_id == movie_id);
    if matches {
        stop_movie(world, client_id, object_id);
    }
}

/// `RequestExEscapeScene` (ex 0x90) — the player pressed Esc. Java routes
/// this through `MovieHolder.playerEscapeVote`: refused for a non-escapable
/// movie; with a single viewer (the only case on this dist — `//playmovie`
/// plays to the GM alone) the vote passes at once and the movie stops.
pub(crate) fn handle_escape_scene(world: &mut World, client_id: u32) {
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    let escapable = world
        .objects
        .get_component::<crate::model::components::InMovie>(&object_id)
        .is_some_and(|m| m.escapable);
    if escapable {
        stop_movie(world, client_id, object_id);
    }
}

/// Java `Player.stopMovie` — send `ExStopScenePlayer` and clear the state.
fn stop_movie(world: &mut World, client_id: u32, object_id: i32) {
    let movie_id = world
        .objects
        .get_component::<crate::model::components::InMovie>(&object_id)
        .map(|m| m.movie_id);
    let Some(movie_id) = movie_id else {
        return;
    };
    send_to_client(
        world,
        client_id,
        crate::network::enter_world::ex_stop_scene_player(movie_id),
    );
    world
        .objects
        .remove_component::<crate::model::components::InMovie>(&object_id);
}

/// `//event_trigger <id> [true|false]` — toggle a client emitter for everyone
/// nearby (Java fans out to visible players plus the GM).
pub(super) fn admin_event_trigger(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let (Some(id), enabled) = (
        nth_arg::<i32>(args, 0),
        args.get(1).is_some_and(|v| v.eq_ignore_ascii_case("true")),
    ) else {
        send_message(world, client_id, "Usage: //event_trigger id [true | false]");
        return;
    };
    let pkt = crate::network::enter_world::event_trigger(id, enabled);
    super::helpers::broadcast_including_self(world, object_id, &pkt);
}

/// `//set_displayeffect <state>` — an NPC target's display-effect state. Java
/// stores it on the NPC (`setDisplayEffect`) *and* broadcasts
/// `ExChangeNpcState`, so the change reaches everyone watching now **and**
/// anyone who walks up later (`NpcInfo`'s `DISPLAY_EFFECT` block).
pub(super) fn admin_set_displayeffect(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let Some(state) = nth_arg::<i32>(args, 0) else {
        send_message(world, client_id, "Usage: //set_displayeffect <id>");
        return;
    };
    let Some(target) = guard::target(world, object_id) else {
        send_sm_bare_to_client(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    if !world
        .objects
        .has_component::<crate::model::npc::Npc>(&target)
    {
        send_sm_bare_to_client(world, client_id, sm_ids::INVALID_TARGET);
        return;
    }
    if let Some(n) = world
        .objects
        .get_component_mut::<crate::model::npc::Npc>(&target)
    {
        n.display_effect = state;
    }
    let pkt = crate::network::enter_world::ex_change_npc_state(target, state);
    super::helpers::broadcast_including_self(world, object_id, &pkt);
}
