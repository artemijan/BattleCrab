//! Skill & buff commands — `AdminSkill`'s `//add_skill`/`//remove_skill` and
//! `AdminBuffs`' `//buff`/`//getbuffs`/`//stopbuff`/`//stopallbuffs`.

use crate::model::components::{Buffs, SkillBook};
use crate::model::Player;
use crate::world::World;

use super::{current_target, send_message, target_player};

/// `AdminSkill`'s `//add_skill <id> [level]` — grant a skill to the targeted
/// player (or self) and refresh their skill list. Passive stat effects apply on
/// the next recompute/relog (the full immediate-passive path is TODO).
pub(super) fn admin_add_skill(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //add_skill <id> [level]");
        return;
    };
    let level = args.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(1).max(1);
    if world.data.skill_data.get(skill_id, level).is_none() {
        send_message(world, client_id, &format!("Skill {skill_id} level {level} does not exist."));
        return;
    }
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&target) {
        book.0.insert(skill_id, level);
    }
    refresh_skill_list(world, target);
    send_message(world, client_id, &format!("Added skill {skill_id} (level {level})."));
}

/// `AdminSkill`'s `//remove_skill <id>` — remove a skill from the targeted
/// player (or self).
pub(super) fn admin_remove_skill(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //remove_skill <id>");
        return;
    };
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&target) {
        book.0.remove(&skill_id);
    }
    refresh_skill_list(world, target);
    send_message(world, client_id, &format!("Removed skill {skill_id}."));
}

/// Resend a player's `SkillList` after a skill-book change.
fn refresh_skill_list(world: &World, target: i32) {
    let Some(cid) = super::helpers::client_for_player(world, target) else { return };
    let Some(book) = world.objects.get_component::<SkillBook>(&target) else {
        return;
    };
    let packet = crate::network::enter_world::skill_list(book, &world.data);
    if let Some(cs) = world.clients.get(&cid) {
        cs.send(packet);
    }
}

/// `AdminBuffs`'s `//buff <skillId> [level]` — apply a skill's effects to the
/// target (any creature) or self, exactly as a cast would (reuses the cast
/// effect pipeline, so buffs/heals broadcast correctly).
pub(super) fn admin_buff(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //buff <skillId> [level]");
        return;
    };
    let level = args.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(1).max(1);
    let Some(skill) = world.data.skill_data.get(skill_id, level).cloned() else {
        send_message(world, client_id, &format!("Skill {skill_id} level {level} does not exist."));
        return;
    };
    let target = current_target(world, object_id).unwrap_or(object_id);
    crate::game_loop::skills::effects::apply_skill_effects(world, object_id, target, &skill);
}

/// `AdminBuffs`'s `//getbuffs` — list the target player's active (non-passive)
/// buffs as text (Java shows an HTML window; documented simplification).
pub(super) fn admin_getbuffs(world: &mut World, client_id: u32, object_id: i32) {
    let target = target_player(world, object_id);
    let name = world.objects.get_component::<Player>(&target).map(|p| p.name.clone()).unwrap_or_default();
    let now = world.tick;
    let lines: Vec<String> = world
        .objects
        .get_component::<Buffs>(&target)
        .map(|b| {
            b.0.iter()
                .filter(|x| !x.passive)
                .map(|x| {
                    let secs = x.expires_at_tick.saturating_sub(now) / 10;
                    format!("  skill {} lvl {} — {secs}s left", x.skill_id, x.skill_level)
                })
                .collect()
        })
        .unwrap_or_default();
    send_message(world, client_id, &format!("=== Buffs on {name} ({} active) ===", lines.len()));
    for line in lines {
        send_message(world, client_id, &line);
    }
}

/// `AdminBuffs`'s `//stopbuff <skillId>` — remove a single buff from the target
/// (reuses the buff-expiry path: stat revert + rebroadcast).
pub(super) fn admin_stopbuff(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //stopbuff <skillId>");
        return;
    };
    let target = current_target(world, object_id).unwrap_or(object_id);
    crate::game_loop::skills::effects::handle_buff_expire(world, target, skill_id);
    send_message(world, client_id, &format!("Removed buff {skill_id}."));
}

/// `AdminBuffs`'s `//stopallbuffs` (confirmDlg) — remove every timed buff from
/// the target (passive grade-penalty pumps are kept). Each removal reverts its
/// stat contribution through the buff-expiry path.
pub(super) fn admin_stopallbuffs(world: &mut World, client_id: u32, object_id: i32) {
    let target = current_target(world, object_id).unwrap_or(object_id);
    let skill_ids: Vec<i32> = world
        .objects
        .get_component::<Buffs>(&target)
        .map(|b| b.0.iter().filter(|x| !x.passive).map(|x| x.skill_id).collect())
        .unwrap_or_default();
    let count = skill_ids.len();
    for skill_id in skill_ids {
        crate::game_loop::skills::effects::handle_buff_expire(world, target, skill_id);
    }
    send_message(world, client_id, &format!("Removed {count} buff(s)."));
}
