//! Auto use — port of `taskmanager/AutoUseTaskManager` and the `.playskills` /
//! `.playitems` / `.playpotion` pages, the second half of auto play
//! (`PLAN_G33_AUTO_PLAY.md`).
//!
//! Four loops share one 300 ms pass, in Java's order: **supply items**, the
//! **healing potion**, **buffs**, then **attack skills**. Each is
//! independently config-gated (`EnableAutoItem` / `EnableAutoPotion` /
//! `EnableAutoSkill`), and the first three plus attack skills are additionally
//! **suppressed inside a peace zone** — buffs are not, so a player can pre-buff
//! in town and walk out ready.
//!
//! **A configured entry that the player no longer has is dropped from the
//! list**, not merely skipped: Java removes the id the moment the item is gone
//! or the skill unknown, so the panel self-cleans rather than accumulating
//! dead entries.

use crate::game_loop::helpers::hp_pair;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::send_to_client;
use crate::game_loop::items::item_skills;
use crate::model::Player;
use crate::model::components::{AutoPlaySettings, AutoUseSettings, SkillBook, ZoneFlags};
use crate::model::inventory::Inventory;
use crate::world::World;

use crate::game_loop::helpers::client_for_player;

// Java's pool is 300 ms; this loop shares the play loop's 3-tick cadence
// (`play::TICK_PERIOD`) rather than keeping a second constant in step.

/// `AutoUseTaskManager.AutoUse.run` — one pass over every active player.
pub(crate) fn tick(world: &mut World) {
    if !world.cfg.auto_play.enabled {
        return;
    }
    let active: Vec<i32> = world
        .in_game_player_oids()
        .filter(|oid| {
            crate::game_loop::automation::play::settings(world, *oid)
                .is_some_and(|s: AutoPlaySettings| s.active)
        })
        .collect();
    for player_oid in active {
        run_for_player(world, player_oid);
    }
}

fn run_for_player(world: &mut World, player_oid: i32) {
    // `isSitting() || hasBlockActions() || isControlBlocked() || isAlikeDead()
    // || isMounted() || (isTransformed() && isRiding())`.
    let blocked = world
        .objects
        .get_component::<Player>(&player_oid)
        .is_some_and(|p| p.sitting || p.is_mounted())
        || is_dead(world, player_oid)
        || crate::game_loop::abnormal::is_blocked_from_actions(world, player_oid)
        || crate::game_loop::abnormal::is_control_blocked(world, player_oid);
    if blocked {
        return;
    }
    let in_peace = world
        .objects
        .get_component::<ZoneFlags>(&player_oid)
        .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Peace));

    if world.cfg.auto_play.item && !in_peace {
        use_supply_items(world, player_oid);
    }
    if world.cfg.auto_play.potion && !in_peace {
        use_potion(world, player_oid);
    }
    if world.cfg.auto_play.skill {
        cast_buffs(world, player_oid);
        if !in_peace {
            cast_attack_skills(world, player_oid);
        }
    }
}

/// The supply-item loop: use each configured item whose skill the player is not
/// already under. An id the player no longer carries is dropped from the list.
fn use_supply_items(world: &mut World, player_oid: i32) {
    let ids = setting_list(world, player_oid, |s| &s.supply_items);
    for item_id in ids {
        let Some(item_object_id) =
            crate::game_loop::helpers::carried_item(world, player_oid, item_id)
        else {
            forget_item(world, player_oid, item_id);
            continue;
        };
        // `isAffectedBySkill(skill)` — don't re-apply a shot/buff already up.
        if item_skills(world, item_id)
            .iter()
            .any(|&(sid, _)| crate::game_loop::abnormal::has_buff(world, player_oid, sid))
        {
            continue;
        }
        crate::game_loop::items::use_item_by_object_id(world, player_oid, item_object_id);
    }
}

/// The healing potion: below `AutoPlaySettings.potion_percent` of max HP, drink
/// the one chosen on `.playpotion`. A potion that has run out clears the slot.
fn use_potion(world: &mut World, player_oid: i32) {
    let percent = crate::game_loop::automation::play::settings(world, player_oid)
        .map(|s| s.potion_percent)
        .unwrap_or(0);
    if percent <= 0 {
        return;
    }
    let Some((cur, max)) = hp_pair(world, player_oid) else {
        return;
    };
    if max <= 0.0 || (cur / max) * 100.0 >= percent as f64 {
        return;
    }
    let item_id = world
        .objects
        .get_component::<AutoUseSettings>(&player_oid)
        .map_or(0, |s| s.potion_item);
    if item_id <= 0 {
        return;
    }
    let Some(item_object_id) = crate::game_loop::helpers::carried_item(world, player_oid, item_id)
    else {
        // `setAutoPotionItem(0)` — the slot empties itself.
        let mut s = settings(world, player_oid);
        s.potion_item = 0;
        store(world, player_oid, s);
        return;
    };
    crate::game_loop::items::use_item_by_object_id(world, player_oid, item_object_id);
}

/// The buff loop — the one that also runs **in town**. A skill the player has
/// forgotten is dropped; one already up is skipped.
fn cast_buffs(world: &mut World, player_oid: i32) {
    let ids = setting_list(world, player_oid, |s| &s.buffs);
    for skill_id in ids {
        if known_level(world, player_oid, skill_id).is_none() {
            forget_skill(world, player_oid, skill_id, true);
            continue;
        }
        if crate::game_loop::abnormal::has_buff(world, player_oid, skill_id) {
            continue;
        }
        if busy_casting(world, player_oid) {
            return; // one cast at a time
        }
        cast(world, player_oid, skill_id);
        return;
    }
}

/// The attack-skill loop: needs a live hostile target, and casts one skill per
/// pass. Java also refuses a target inside a peace zone.
fn cast_attack_skills(world: &mut World, player_oid: i32) {
    let ids = setting_list(world, player_oid, |s| &s.skills);
    if ids.is_empty() {
        return;
    }
    let Some(target) = world
        .objects
        .get_component::<crate::model::components::TargetRef>(&player_oid)
        .and_then(|t| t.0)
    else {
        return;
    };
    if target == player_oid
        || is_dead(world, target)
        || world
            .objects
            .get_component::<ZoneFlags>(&target)
            .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Peace))
    {
        return;
    }
    for skill_id in ids {
        if known_level(world, player_oid, skill_id).is_none() {
            forget_skill(world, player_oid, skill_id, false);
            continue;
        }
        if busy_casting(world, player_oid) {
            return;
        }
        cast(world, player_oid, skill_id);
        return;
    }
}

// ---------------------------------------------------------------------------
// The three sub-pages
// ---------------------------------------------------------------------------

/// `.playskills` / `.playitems` / `.playpotion` — each lists what the player
/// could add and what they have added, with a toggle link per row. Java pages
/// these seven at a time; the port lists them whole, which is the same
/// information without the paging state.
pub(crate) fn handle_voiced(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    command: &str,
    args: &[&str],
) {
    // Java gates each page on its own config flag — a disabled one has no
    // button on the main panel and no handler behind it.
    let allowed = match command {
        "playskills" => world.cfg.auto_play.skill,
        "playitems" => world.cfg.auto_play.item,
        "playpotion" => world.cfg.auto_play.potion,
        _ => false,
    };
    if !allowed {
        return;
    }
    if let Some(&id) = args.first()
        && let Ok(id) = id.parse::<i32>()
    {
        toggle(world, player_oid, command, id);
    }
    send_page(world, client_id, player_oid, command);
}

/// Add or remove one entry (Java's per-row link).
fn toggle(world: &mut World, player_oid: i32, command: &str, id: i32) {
    let mut s = settings(world, player_oid);
    match command {
        "playskills" => {
            if world.cfg.auto_play.disabled_skills.contains(&id) {
                return;
            }
            // Java files a skill under buffs or attack skills by whether it
            // targets the caster — a self-target skill is a buff.
            let is_buff = world
                .data
                .skill_data
                .get(id, known_level(world, player_oid, id).unwrap_or(1))
                .is_some_and(|sk| sk.target_type == crate::model::skill::TargetType::Self_);
            let list = if is_buff { &mut s.buffs } else { &mut s.skills };
            if let Some(pos) = list.iter().position(|&x| x == id) {
                list.remove(pos);
            } else {
                list.push(id);
            }
        }
        "playitems" => {
            if world.cfg.auto_play.disabled_items.contains(&id) {
                return;
            }
            if let Some(pos) = s.supply_items.iter().position(|&x| x == id) {
                s.supply_items.remove(pos);
            } else {
                s.supply_items.push(id);
            }
        }
        _ => {
            // The potion is a single slot: picking the current one clears it.
            s.potion_item = if s.potion_item == id { 0 } else { id };
        }
    }
    store(world, player_oid, s);
}

fn send_page(world: &World, client_id: u32, player_oid: i32, command: &str) {
    let (file, rows) = match command {
        "playskills" => ("Skills.htm", skill_rows(world, player_oid)),
        "playitems" => ("Items.htm", item_rows(world, player_oid)),
        _ => ("Potion.htm", potion_rows(world, player_oid)),
    };
    let html = crate::data::htm_cache::read_htm_for(
        world,
        player_oid,
        format!("{}data/html/mods/AutoPlay/{file}", world.data.root),
    )
    .unwrap_or_else(|| "<html><body>%list%</body></html>".to_string())
    .replace("%list%", &rows);
    send_to_client(
        world,
        client_id,
        crate::network::server_packets::npc_html_message(0, &html),
    );
}

fn skill_rows(world: &World, player_oid: i32) -> String {
    let s = settings(world, player_oid);
    let mut ids: Vec<(i32, i32)> = world
        .objects
        .get_component::<SkillBook>(&player_oid)
        .map(|b| b.0.iter().map(|(id, lv)| (*id, *lv)).collect())
        .unwrap_or_default();
    ids.sort_unstable();
    let mut out = String::new();
    for (id, level) in ids {
        if world.cfg.auto_play.disabled_skills.contains(&id) {
            continue;
        }
        let on = s.buffs.contains(&id) || s.skills.contains(&id);
        let name = world
            .data
            .skill_data
            .get(id, level)
            .map(|sk| sk.name.clone())
            .unwrap_or_else(|| format!("Skill {id}"));
        out.push_str(&row(&name, on, "playskills", id));
    }
    out
}
fn inventory_item_ids(world: &World, player_oid: i32) -> Vec<i32> {
    world
        .objects
        .get_component::<Inventory>(&player_oid)
        .map(|inv| inv.items().iter().map(|i| i.item_id).collect())
        .unwrap_or_default()
}
fn item_rows(world: &World, player_oid: i32) -> String {
    let s = settings(world, player_oid);
    let mut ids: Vec<i32> = inventory_item_ids(world, player_oid);
    ids.sort_unstable();
    ids.dedup();
    let mut out = String::new();
    for id in ids {
        if world.cfg.auto_play.disabled_items.contains(&id) {
            continue;
        }
        // Only an item that *does* something on use can be auto-used.
        if item_skills(world, id).is_empty() {
            continue;
        }
        let name = item_name(world, id);
        out.push_str(&row(&name, s.supply_items.contains(&id), "playitems", id));
    }
    out
}

fn potion_rows(world: &World, player_oid: i32) -> String {
    let s = settings(world, player_oid);
    let mut ids: Vec<i32> = inventory_item_ids(world, player_oid);
    ids.sort_unstable();
    ids.dedup();
    let mut out = String::new();
    for id in ids {
        if item_skills(world, id).is_empty() {
            continue;
        }
        out.push_str(&row(
            &item_name(world, id),
            s.potion_item == id,
            "playpotion",
            id,
        ));
    }
    out
}

fn row(name: &str, on: bool, command: &str, id: i32) -> String {
    let box_img = if on {
        "L2UI.CheckBox_checked"
    } else {
        "L2UI.CheckBox"
    };
    format!(
        "<tr><td><img src=\"{box_img}\" width=16 height=16></td>\
         <td><a action=\"bypass voice .{command} {id}\">{name}</a></td></tr>"
    )
}

// --- helpers ---------------------------------------------------------------

pub(crate) fn settings(world: &World, player_oid: i32) -> AutoUseSettings {
    world
        .objects
        .get_component::<AutoUseSettings>(&player_oid)
        .cloned()
        .unwrap_or_default()
}

/// One configured list, cloned alone. The 300 ms loops need an owned id list
/// (their bodies mutate `world`), but `settings()` clones all three `Vec`s to
/// hand out one — 3–4 times per active player per pass.
fn setting_list(
    world: &World,
    player_oid: i32,
    pick: fn(&AutoUseSettings) -> &Vec<i32>,
) -> Vec<i32> {
    world
        .objects
        .get_component::<AutoUseSettings>(&player_oid)
        .map(|s| pick(s).clone())
        .unwrap_or_default()
}

fn store(world: &mut World, player_oid: i32, s: AutoUseSettings) {
    world.objects.add_components(&player_oid, s);
}

fn forget_item(world: &mut World, player_oid: i32, item_id: i32) {
    let mut s = settings(world, player_oid);
    s.supply_items.retain(|&x| x != item_id);
    store(world, player_oid, s);
}

fn forget_skill(world: &mut World, player_oid: i32, skill_id: i32, buff: bool) {
    let mut s = settings(world, player_oid);
    if buff {
        s.buffs.retain(|&x| x != skill_id);
    } else {
        s.skills.retain(|&x| x != skill_id);
    }
    store(world, player_oid, s);
}

fn item_name(world: &World, item_id: i32) -> String {
    world
        .data
        .item_data
        .get(item_id)
        .map(|t| t.name.clone())
        .unwrap_or_else(|| format!("Item {item_id}"))
}

fn known_level(world: &World, player_oid: i32, skill_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<SkillBook>(&player_oid)
        .and_then(|b| b.0.get(&skill_id).copied())
}

fn busy_casting(world: &World, player_oid: i32) -> bool {
    world
        .objects
        .has_component::<crate::model::components::Casting>(&player_oid)
}

fn cast(world: &mut World, player_oid: i32, skill_id: i32) {
    if let Some(cid) = client_for_player(world, player_oid) {
        crate::game_loop::skills::cast::use_magic(world, cid, player_oid, skill_id, false, false);
    }
}
