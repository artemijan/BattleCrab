//! Auto play — port of `taskmanager/AutoPlayTaskManager` and the `.play`
//! voiced command, gated on `Custom/AutoPlay.ini` (`EnableAutoPlay = True`).
//!
//! **No packets are involved.** This build registers no Classic auto-hunt
//! opcode; the feature is a voiced command that opens an html panel, and the
//! loop below drives the ordinary target/attack/pickup paths on the player's
//! behalf. See `PLAN_G33_AUTO_PLAY.md`.

use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::maybe_position;
use crate::game_loop::helpers::nth_arg;
use crate::game_loop::helpers::send_to_client;
use crate::model::Player;
use crate::model::components::{AutoPlaySettings, GroundItem, Position};
use crate::world::World;

use super::helpers::client_for_player;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::target;

/// Java's pool runs every 300 ms; the loop ticks at 100 ms, so every 3 ticks.
pub(crate) const TICK_PERIOD: u64 = 3;
/// `AutoPlayTaskManager.IDLE_COUNT`'s threshold — passes doing nothing before
/// the player is nudged to a fresh spot.
const IDLE_LIMIT: u32 = 10;
/// Loot within this radius is picked up.
const PICKUP_RANGE: f64 = 200.0;
/// …and walked to from this far.
const PICKUP_REACH: f64 = 70.0;
/// The two target-scan radii (`isShortRange()`).
const SHORT_RANGE: f64 = 600.0;
const LONG_RANGE: f64 = 1400.0;
/// Java skips a creature more than this far above or below.
const MAX_Z_DIFF: i32 = 180;

/// `.play` / `.playskills` / `.playitems` / `.playpotion` — slice 1 answers the
/// main panel and its toggles; the three sub-pages land with auto-use.
pub(crate) fn handle_voiced(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    command: &str,
    args: &[&str],
) {
    if !world.cfg.auto_play.enabled {
        return;
    }
    // `AutoPlayPremium`: the whole panel is premium-only here.
    if world.cfg.auto_play.premium_only
        && !super::admin::premium::has_premium_status(world, player_oid)
    {
        return;
    }
    if command != "play" {
        // The three sub-panels are auto-use's.
        super::auto_use::handle_voiced(world, client_id, player_oid, command, args);
        return;
    }
    apply_toggle(world, player_oid, args);
    send_panel(world, client_id, player_oid);
}

/// One `.play <toggle>` press. Java's switch, verbatim.
fn apply_toggle(world: &mut World, player_oid: i32, args: &[&str]) {
    let Some(&verb) = args.first() else {
        return;
    };
    let Some(mut s) = settings(world, player_oid) else {
        return;
    };
    match verb {
        "attack" => s.auto_attack = !s.auto_attack,
        "loot" => s.pickup = !s.pickup,
        "respect" => s.respectful_hunting = !s.respectful_hunting,
        "range" => s.short_range = !s.short_range,
        "mode0" => s.next_target_mode = 0,
        "mode1" => s.next_target_mode = 1,
        "mode2" => s.next_target_mode = 2,
        "mode3" => s.next_target_mode = 3,
        "percent" => {
            if let Some(v) = nth_arg::<i32>(args, 1) {
                s.potion_percent = v.clamp(0, 100);
            }
        }
        "start" => s.active = true,
        "stop" => s.active = false,
        _ => return,
    }
    store(world, player_oid, s);
}

/// `data/html/mods/AutoPlay/Main.htm` with its checkbox / radio tokens filled.
fn send_panel(world: &World, client_id: u32, player_oid: i32) {
    let Some(s) = settings(world, player_oid) else {
        return;
    };
    let checkbox = |on: bool| {
        if on {
            "L2UI.CheckBox_checked"
        } else {
            "L2UI.CheckBox"
        }
    };
    let radio = |on: bool| {
        if on {
            "L2UI_CH3.radiobutton2"
        } else {
            "L2UI_CH3.radiobutton1"
        }
    };
    let mut html = crate::data::htm_cache::read_htm_for(
        world,
        player_oid,
        format!("{}data/html/mods/AutoPlay/Main.htm", world.data.root),
    )
    .unwrap_or_else(|| "<html><body>Auto Play</body></html>".to_string());
    html = html
        .replace("%attack%", checkbox(s.auto_attack))
        .replace("%loot%", checkbox(s.pickup))
        .replace("%respect%", checkbox(s.respectful_hunting))
        // Java inverts this one: the box is "long range".
        .replace("%range%", checkbox(!s.short_range))
        .replace("%mode0%", radio(s.next_target_mode == 0))
        .replace("%mode1%", radio(s.next_target_mode == 1))
        .replace("%mode2%", radio(s.next_target_mode == 2))
        .replace("%mode3%", radio(s.next_target_mode == 3))
        .replace("%percent%", &s.potion_percent.to_string());
    // Each sub-panel button exists only when its own config flag is on, which
    // is how Java hides a disabled half of the feature.
    for (token, on, cmd, label) in [
        (
            "%skill_button%",
            world.cfg.auto_play.skill,
            "playskills",
            "Select Skills",
        ),
        (
            "%item_button%",
            world.cfg.auto_play.item,
            "playitems",
            "Select Supply Items",
        ),
        (
            "%potion_button%",
            world.cfg.auto_play.potion,
            "playpotion",
            "Select Healing Potion",
        ),
    ] {
        let button = if on {
            format!(
                "<button action=\"bypass voice .{cmd}\" value=\"{label}\" \
                 width=240 height=31>"
            )
        } else {
            String::new()
        };
        html = html.replace(token, &button);
    }
    let status = if s.active {
        "<button action=\"bypass voice .play stop\" value=\"Stop\" width=240 height=31>"
    } else {
        "<button action=\"bypass voice .play start\" value=\"Start\" width=240 height=31>"
    };
    html = html.replace("%status_button%", status);
    send_to_client(
        world,
        client_id,
        crate::network::server_packets::npc_html_message(0, &html),
    );
}

/// `AutoPlayTaskManager.AutoPlay.run` — one pass over every active player.
pub(crate) fn tick(world: &mut World) {
    if !world.cfg.auto_play.enabled {
        return;
    }
    let active: Vec<i32> = world
        .in_game_player_oids()
        .filter(|oid| settings(world, *oid).is_some_and(|s| s.active))
        .collect();
    for player_oid in active {
        run_for_player(world, player_oid);
    }
}

fn run_for_player(world: &mut World, player_oid: i32) {
    // `isSitting() || isCastingNow() || getQueuedSkill() != null` — skip the
    // pass rather than stopping, so a cast finishes uninterrupted.
    let busy = world
        .objects
        .get_component::<Player>(&player_oid)
        .is_some_and(|p| p.sitting)
        || world
            .objects
            .has_component::<crate::model::components::Casting>(&player_oid);
    if busy {
        return;
    }
    let Some(s) = settings(world, player_oid) else {
        return;
    };

    // A live, still-valid target means "keep going" — the pass ends here.
    if let Some(target) = target::current(world, player_oid) {
        if target_still_valid(world, player_oid, target, s.next_target_mode) {
            keep_attacking(world, player_oid, target, &s);
            return;
        }
        clear_target(world, player_oid);
    }
    reset_idle(world, player_oid);

    if s.pickup && try_pickup(world, player_oid) {
        return; // Java `continue PLAY` — one action per pass.
    }
    if let Some(found) = find_target(world, player_oid, &s) {
        set_target(world, player_oid, found);
        // `isMageCaster` is a misnomer: it means auto-attack is **off**, so an
        // unticked box acquires a target and leaves the swinging to the skills.
        if s.auto_attack {
            attack(world, player_oid, found);
        }
    }
}

/// The already-attacking branch: keep the intention, or nudge a stuck player.
fn keep_attacking(world: &mut World, player_oid: i32, target: i32, s: &AutoPlaySettings) {
    if !s.auto_attack {
        return;
    }
    let attacking = world
        .objects
        .get_component::<crate::model::components::AttackState>(&player_oid)
        .is_some_and(|a| a.attack_end_tick > world.tick);
    let moving = world
        .objects
        .has_component::<crate::model::components::Movement>(&player_oid);
    if attacking || moving {
        reset_idle(world, player_oid);
        return;
    }
    // Idle: after `IDLE_LIMIT` passes doing nothing, step to a fresh spot so a
    // melee wedged on geometry unsticks itself (Java's `IDLE_COUNT` nudge).
    let idle = world
        .auto_play_idle
        .entry(player_oid)
        .and_modify(|c| *c += 1)
        .or_insert(1);
    if *idle > IDLE_LIMIT {
        world.auto_play_idle.remove(&player_oid);
        nudge(world, player_oid, target);
        return;
    }
    attack(world, player_oid, target);
}

/// Java's reposition: a point one collision-diameter beyond the target, so the
/// player walks *through* whatever it is stuck on.
fn nudge(world: &mut World, player_oid: i32, target: i32) {
    let (Some(p), Some(t)) = (
        maybe_position(world, player_oid),
        maybe_position(world, target),
    ) else {
        return;
    };
    let radius = world
        .objects
        .get_component::<crate::model::components::Collision>(&player_oid)
        .map_or(10.0, |c| c.radius);
    let angle = (t.y as f64 - p.y as f64).atan2(t.x as f64 - p.x as f64);
    let distance = radius * 4.0;
    let (x, y) = (
        t.x + (angle.cos() * distance) as i32,
        t.y + (angle.sin() * distance) as i32,
    );
    move_to(world, player_oid, p, (x, y, p.z));
}

/// Walk to a point through the ordinary move path (Java
/// `AI_INTENTION_MOVE_TO`), pathfinding left to the mover as for a click.
fn move_to(world: &mut World, player_oid: i32, from: Position, dest: (i32, i32, i32)) {
    if let Some(cid) = client_for_player(world, player_oid) {
        super::position::start_move(world, cid, player_oid, from, dest, None);
    }
}

/// Loot within `PICKUP_RANGE`: walk to it, then take it. Returns whether the
/// pass was spent.
fn try_pickup(world: &mut World, player_oid: i32) -> bool {
    let (Some(pos), Some(region)) = (
        maybe_position(world, player_oid),
        region_cell_of(world, player_oid),
    ) else {
        return false;
    };
    let candidates: Vec<(i32, Position, i32)> = world
        .ground_items_visible_from(region)
        .into_iter()
        .filter_map(|oid| {
            let g = world.objects.get_component::<GroundItem>(&oid)?;
            let p = world.objects.get_component::<Position>(&oid)?;
            Some((oid, *p, g.owner_id))
        })
        .collect();
    for (item_oid, ipos, owner_id) in candidates {
        let d = distance_2d(&pos, &ipos);
        if d > PICKUP_RANGE {
            continue;
        }
        // `isProtected() || ownerId == player` — someone else's drop is left.
        if owner_id != 0 && owner_id != player_oid {
            continue;
        }
        if d > PICKUP_REACH {
            move_to(world, player_oid, pos, (ipos.x, ipos.y, ipos.z));
            return true;
        }
        if let Some(cid) = client_for_player(world, player_oid) {
            super::ground_items::pickup_ground_item(world, cid, player_oid, item_oid);
        }
        return true;
    }
    false
}

/// The nearest reachable creature matching the mode, or the party leader's
/// target under `AssistLeader`.
fn find_target(world: &World, player_oid: i32, s: &AutoPlaySettings) -> Option<i32> {
    if world.cfg.auto_play.assist_leader
        && let Some(leader_target) = leader_target(world, player_oid)
    {
        return Some(leader_target);
    }
    let pos = maybe_position(world, player_oid)?;
    let region = region_cell_of(world, player_oid)?;
    // Characters mode ignores the short-range setting, as Java does.
    let range = if s.short_range && s.next_target_mode != 2 {
        SHORT_RANGE
    } else {
        LONG_RANGE
    };
    let mut best: Option<(i32, f64)> = None;
    let npcs = world.npcs_visible_from(region);
    for other in npcs {
        if !mode_allows(world, player_oid, other, s.next_target_mode) {
            continue;
        }
        let Some(opos) = maybe_position(world, other) else {
            continue;
        };
        if (pos.z - opos.z).abs() >= MAX_Z_DIFF {
            continue;
        }
        // `isRespectfulHunting`: leave a mob that is already fighting someone.
        if s.respectful_hunting && is_busy_with_someone_else(world, other, player_oid) {
            continue;
        }
        let d = distance_2d(&pos, &opos);
        if d > range {
            continue;
        }
        if !world
            .geo
            .can_see_target(pos.x, pos.y, pos.z, opos.x, opos.y, opos.z)
        {
            continue;
        }
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((other, d));
        }
    }
    best.map(|(oid, _)| oid)
}

/// `AssistLeader`: the leader's target if they have a valid one.
fn leader_target(world: &World, player_oid: i32) -> Option<i32> {
    let party_id = world
        .objects
        .get_component::<crate::model::components::PartyRef>(&player_oid)?
        .0;
    let leader = world.parties.get(&party_id)?.leader();
    if leader == player_oid {
        return None;
    }
    let target = target::current(world, leader)?;
    world
        .objects
        .has_component::<crate::model::npc::Npc>(&target)
        .then_some(target)
}

/// `isTargetModeValid`: 0 = anything attackable, 1 = monsters, 2 = playables,
/// 3 = NPCs. Slice 1 scans NPCs only, so mode 2 finds nothing yet.
fn mode_allows(world: &World, player_oid: i32, other: i32, mode: i32) -> bool {
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&other)
    else {
        return false;
    };
    if is_dead(world, other) {
        return false;
    }
    let Some(t) = world.data.npc_data.get(npc.npc_id) else {
        return false;
    };
    let attackable = t.is_monster() && t.attackable;
    let _ = player_oid;
    match mode {
        1 => attackable,
        2 => false, // playables — slice 1 does not scan players
        3 => !t.is_monster(),
        _ => attackable,
    }
}

/// Whether `other` is already engaged with somebody who is not this player.
fn is_busy_with_someone_else(world: &World, other: i32, player_oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::TargetRef>(&other)
        .and_then(|t| t.0)
        .is_some_and(|t| t != player_oid)
}

fn target_still_valid(world: &World, player_oid: i32, target: i32, mode: i32) -> bool {
    if is_dead(world, target) {
        return false;
    }
    mode_allows(world, player_oid, target, mode)
}

// --- small helpers ---------------------------------------------------------

fn distance_2d(a: &Position, b: &Position) -> f64 {
    let (dx, dy) = ((a.x - b.x) as f64, (a.y - b.y) as f64);
    (dx * dx + dy * dy).sqrt()
}

fn set_target(world: &mut World, player_oid: i32, target: i32) {
    if let Some(cid) = client_for_player(world, player_oid) {
        super::target::set_target(world, cid, player_oid, Some(target));
    }
}

fn clear_target(world: &mut World, player_oid: i32) {
    if let Some(cid) = client_for_player(world, player_oid) {
        super::target::set_target(world, cid, player_oid, None);
    }
}

fn attack(world: &mut World, player_oid: i32, target: i32) {
    if let Some(cid) = client_for_player(world, player_oid) {
        super::combat::start_attack_intent(world, cid, player_oid, target);
    }
}

fn reset_idle(world: &mut World, player_oid: i32) {
    world.auto_play_idle.remove(&player_oid);
}

pub(crate) fn settings(world: &World, player_oid: i32) -> Option<AutoPlaySettings> {
    world
        .objects
        .get_component::<AutoPlaySettings>(&player_oid)
        .copied()
        .or(Some(AutoPlaySettings::default()))
}

fn store(world: &mut World, player_oid: i32, s: AutoPlaySettings) {
    world.objects.add_components(&player_oid, s);
}

/// Drop a player from the loop's bookkeeping on logout.
pub(crate) fn remove(world: &mut World, player_oid: i32) {
    world.auto_play_idle.remove(&player_oid);
}
