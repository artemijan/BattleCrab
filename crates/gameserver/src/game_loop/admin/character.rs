//! Character-editing commands — the `AdminEditChar` / `AdminExpSp` /
//! `AdminLevel` / `AdminEnchant` family that mutates a player's progression,
//! integer fields, appearance, and equipment enchant levels.

use crate::model::inventory::{Inventory, PaperdollSlot};
use crate::model::Player;
use crate::world::World;

use super::{current_target, send_message, target_player};

/// `AdminExpSp`'s `//add_exp_sp <exp> <sp>` — grant exp+sp to the targeted
/// player (or self), driving the level-up path.
pub(super) fn admin_add_exp_sp(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(exp), Some(sp)) = (
        args.first().and_then(|s| s.parse::<i64>().ok()),
        args.get(1).and_then(|s| s.parse::<i64>().ok()),
    ) else {
        send_message(world, client_id, "Usage: //add_exp_sp <exp> <sp>");
        return;
    };
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    super::death::add_exp_and_sp(world, target, exp, sp);
    if let Some(name) = world.objects.get_component::<Player>(&target).map(|p| p.name.clone()) {
        send_message(world, client_id, &format!("Added {exp} xp and {sp} sp to {name}."));
    }
}

/// `AdminExpSp`'s `//remove_exp_sp <exp> <sp>` — subtract exp+sp from the
/// targeted player (or self), deleveling as needed.
pub(super) fn admin_remove_exp_sp(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(exp), Some(sp)) = (
        args.first().and_then(|s| s.parse::<i64>().ok()),
        args.get(1).and_then(|s| s.parse::<i64>().ok()),
    ) else {
        send_message(world, client_id, "Usage: //remove_exp_sp exp sp");
        return;
    };
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    super::death::remove_exp_and_sp(world, target, exp, sp);
    if let Some(name) = world.objects.get_component::<Player>(&target).map(|p| p.name.clone()) {
        send_message(world, client_id, &format!("Removed {exp} xp and {sp} sp from {name}."));
    }
}

/// `AdminExpSp`'s `//add_exp_sp_to_character` — Java opens `expsp.htm`; we send
/// the same figures (level/exp/sp) as text for the current player target.
pub(super) fn admin_add_exp_sp_menu(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid))
    else {
        super::send_sm(world, client_id, crate::network::server_packets::sm_ids::INVALID_TARGET);
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&target) else { return };
    send_message(world, client_id, &format!("=== {} (level {}) ===", p.name, p.level));
    send_message(world, client_id, &format!("XP: {}  SP: {}", p.exp, p.sp));
    send_message(world, client_id, "Usage: //add_exp_sp <exp> <sp> | //remove_exp_sp <exp> <sp>");
}

/// `AdminLevel`'s `//add_level <n>` / `//set_level <n>` — add levels to, or set
/// the level of, the targeted player (or self). `set` chooses between the two.
pub(super) fn admin_change_level(world: &mut World, client_id: u32, object_id: i32, args: &[&str], set: bool) {
    let Some(value) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, if set { "Usage: //set_level <level>" } else { "Usage: //add_level <levels>" });
        return;
    };
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    let Some(current) = world.objects.get_component::<Player>(&target).map(|p| p.level) else { return };
    let max_level = world.data.experience.max_level as i32;
    let new_level = if set { value } else { current + value }.clamp(1, max_level);
    // Set exp to the level's threshold so the exp bar and future exp math stay
    // consistent (Java `PlayerStat.setLevel` → `setExp(getExpForLevel(level))`).
    let exp = world.data.experience.exp_for_level(new_level);
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.exp = exp;
    }
    super::death::set_level(world, target, new_level);
}

/// The integer `Player` fields `//editchar` can set directly.
#[derive(Clone, Copy)]
pub(super) enum IntField {
    Reputation,
    Fame,
    Pk,
    Pvp,
}

impl IntField {
    fn label(self) -> &'static str {
        match self {
            IntField::Reputation => "Reputation",
            IntField::Fame => "Fame",
            IntField::Pk => "PK count",
            IntField::Pvp => "PvP count",
        }
    }
}

/// `//setreputation`/`//setfame`/`//setpk`/`//setpvp <n>` — set an integer field
/// on the target player (or self).
pub(super) fn set_int_field(world: &mut World, client_id: u32, object_id: i32, field: IntField, args: &[&str]) {
    let Some(value) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, &format!("Usage: //set{} <value>", field.label().to_lowercase()));
        return;
    };
    set_field_value(world, client_id, object_id, field, value);
}

pub(super) fn set_field_value(world: &mut World, client_id: u32, object_id: i32, field: IntField, value: i32) {
    let target = target_player(world, object_id);
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        match field {
            IntField::Reputation => p.reputation = value,
            IntField::Fame => p.fame = value,
            IntField::Pk => p.pk_kills = value,
            IntField::Pvp => p.pvp_kills = value,
        }
    }
    super::party::broadcast_user_info(world, target);
    send_message(world, client_id, &format!("{} set to {value}.", field.label()));
}

/// Java `PlayerStat.MAX_VITALITY_POINTS` / `MIN_VITALITY_POINTS`.
const MAX_VITALITY_POINTS: i32 = 140_000;
const MIN_VITALITY_POINTS: i32 = 0;

/// `AdminVitality`'s `//set_vitality <n>` / `//full_vitality` / `//empty_vitality`
/// / `//get_vitality` — read or set the *targeted player's* vitality points
/// (Java requires a player target). Setting rebroadcasts UserInfo (the vitality
/// block).
pub(super) fn admin_vitality(world: &mut World, client_id: u32, object_id: i32, mode: &str, args: &[&str]) {
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid)) else {
        send_message(world, client_id, "Target not found or not a player");
        return;
    };
    match mode {
        "get" => {
            let v = world.objects.get_component::<Player>(&target).map_or(0, |p| p.vitality_points);
            send_message(world, client_id, &format!("Player vitality points: {v}"));
            return;
        }
        "set" => {
            let Some(value) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
                send_message(world, client_id, "Incorrect vitality");
                return;
            };
            set_vitality_points(world, target, value);
        }
        "full" => set_vitality_points(world, target, MAX_VITALITY_POINTS),
        "empty" => set_vitality_points(world, target, MIN_VITALITY_POINTS),
        _ => {}
    }
    super::party::broadcast_user_info(world, target);
}

fn set_vitality_points(world: &mut World, target: i32, value: i32) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.vitality_points = value.clamp(MIN_VITALITY_POINTS, MAX_VITALITY_POINTS);
    }
}

/// `AdminEditChar`'s `//setclass <id>` — change the target player's (or self's)
/// class. Reuses `set_level` at the current level to recompute vitals/stats and
/// grant the new class's skills, then rebroadcasts. The old class's skills are
/// not pruned yet (Java's full class-transfer cleanup is TODO).
pub(super) fn admin_setclass(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(class_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //setclass <classId>");
        return;
    };
    if world.data.player_templates.get(class_id).is_none() {
        send_message(world, client_id, &format!("Class id {class_id} does not exist."));
        return;
    }
    let target = target_player(world, object_id);
    let Some(level) = world.objects.get_component::<Player>(&target).map(|p| p.level) else { return };
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.class_id = class_id;
        p.base_class_id = class_id;
    }
    // Recompute HP/MP/stats + grant the new class's reachable skills, with the
    // status/UserInfo/SkillList refresh; then CharInfo to nearby (new class).
    super::death::set_level(world, target, level);
    super::party::broadcast_user_info(world, target);
    send_message(world, client_id, &format!("Class set to {class_id}."));
}

/// `//settitle <text>` — set the target player's title.
pub(super) fn admin_set_title(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let title = args.join(" ");
    let target = target_player(world, object_id);
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.title = title;
    }
    super::party::broadcast_user_info(world, target);
    send_message(world, client_id, "Title changed.");
}

/// `//setcolor`/`//settcolor <hex>` — set the target player's name/title color.
pub(super) fn admin_set_color(world: &mut World, client_id: u32, object_id: i32, args: &[&str], title: bool) {
    let Some(color) = args.first().and_then(|s| i32::from_str_radix(s.trim_start_matches("0x"), 16).ok()) else {
        send_message(world, client_id, "Usage: //setcolor <hex, e.g. FF0000>");
        return;
    };
    let target = target_player(world, object_id);
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        if title {
            p.title_color = color;
        } else {
            p.name_color = color;
        }
    }
    super::party::broadcast_user_info(world, target);
    send_message(world, client_id, "Color changed.");
}

/// `//setsex` — flip the target player's gender.
pub(super) fn admin_set_sex(world: &mut World, client_id: u32, object_id: i32) {
    let target = target_player(world, object_id);
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.is_female = !p.is_female;
    }
    super::party::broadcast_user_info(world, target);
    send_message(world, client_id, "Gender flipped.");
}

/// `AdminEnchant`'s per-slot `//set<slot> <value>` — set the enchant level of
/// the item equipped in `slot` on the targeted player (or self). Enchant *stat*
/// bonuses aren't applied yet (item stats are a later milestone); this sets the
/// stored level, refreshes the inventory, and rebroadcasts UserInfo (glow).
pub(super) fn admin_set_enchant(world: &mut World, client_id: u32, object_id: i32, slot: PaperdollSlot, args: &[&str]) {
    let Some(value) = args.first().and_then(|s| s.parse::<i32>().ok()).filter(|v| (0..=127).contains(v))
    else {
        send_message(world, client_id, "Usage: //set<slot> <0..127>");
        return;
    };
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    let changed = world
        .objects
        .get_component_mut::<Inventory>(&target)
        .and_then(|inv| inv.set_paperdoll_enchant(slot, value));
    let Some(item_oid) = changed else {
        send_message(world, client_id, "No item equipped in that slot.");
        return;
    };
    if let Some(cid) = super::helpers::client_for_player(world, target) {
        if let Some(inv) = world.objects.get_component::<Inventory>(&target) {
            let packet = crate::network::enter_world::inventory_update(inv, &world.data, &[item_oid]);
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(packet);
            }
        }
    }
    super::party::broadcast_user_info(world, target);
    send_message(world, client_id, &format!("Enchant set to +{value}."));
}
