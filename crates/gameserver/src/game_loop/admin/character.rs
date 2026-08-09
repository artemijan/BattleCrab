//! Character-editing commands — the `AdminEditChar` / `AdminExpSp` /
//! `AdminLevel` / `AdminEnchant` family that mutates a player's progression,
//! integer fields, appearance, and equipment enchant levels.

use crate::game_loop::guard::{self, Guard, OrReject};
use crate::game_loop::helpers;
use crate::game_loop::helpers::nth_arg;
use crate::model::inventory::{Inventory, PaperdollSlot};
use crate::model::{MAX_VITALITY_POINTS, MIN_VITALITY_POINTS, Player};
use crate::network::server_packets::sm_ids;
use crate::world::World;

use super::{send_message, target_player};

/// `AdminExpSp`'s `//add_exp_sp <exp> <sp>` — grant exp+sp to the **targeted
/// player**, driving the level-up path. Faithful to Java `AdminExpSp`: a
/// player target is required (no self-fallback — target yourself to self-grant),
/// the target is told "Admin is adding you …", and the exp/sp menu is refreshed
/// afterwards (Java's trailing `addExpSp(activeChar)`, run for every invocation).
pub(super) fn admin_add_exp_sp(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let result = add_exp_sp(world, client_id, object_id, args);
    guard::finish(world, client_id, result);
}

fn add_exp_sp(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) -> Guard<()> {
    // Java `adminAddExpSp`: the target must be a player, else INVALID_TARGET.
    let target = guard::player_target(world, object_id).or_sm(sm_ids::INVALID_TARGET)?;
    // Exactly two numeric tokens, else the usage hint (Java: `countTokens() != 2`
    // or a parse failure returns false → "Usage" sysmessage).
    match (args.len(), nth_arg::<i64>(args, 0), nth_arg::<i64>(args, 1)) {
        // Java only applies + messages when at least one value is non-zero.
        (2, Some(exp), Some(sp)) if exp != 0 || sp != 0 => {
            let name = helpers::player_name_or_empty(world, target);
            if let Some(tcid) = crate::game_loop::helpers::client_for_player(world, target) {
                send_message(
                    world,
                    tcid,
                    &format!("Admin is adding you {exp} xp and {sp} sp."),
                );
            }
            // `AdminEditChar` uses the no-bonus overload.
            super::death::add_exp_and_sp(world, target, exp as f64, sp as f64, false);
            send_message(
                world,
                client_id,
                &format!("Added {exp} xp and {sp} sp to {name}."),
            );
        }
        (2, Some(_), Some(_)) => {} // both zero: Java no-ops the grant, still refreshes the menu.
        _ => send_message(world, client_id, "Usage: //add_exp_sp exp sp"),
    }
    admin_add_exp_sp_menu(world, client_id, object_id);
    Ok(())
}

/// `AdminExpSp`'s `//remove_exp_sp <exp> <sp>` — subtract exp+sp from the
/// **targeted player**, deleveling as needed. The mirror of [`admin_add_exp_sp`]
/// (player target required, target notified, menu refreshed).
pub(super) fn admin_remove_exp_sp(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let result = remove_exp_sp(world, client_id, object_id, args);
    guard::finish(world, client_id, result);
}

fn remove_exp_sp(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) -> Guard<()> {
    let target = guard::player_target(world, object_id).or_sm(sm_ids::INVALID_TARGET)?;
    match (args.len(), nth_arg::<i64>(args, 0), nth_arg::<i64>(args, 1)) {
        (2, Some(exp), Some(sp)) if exp != 0 || sp != 0 => {
            let name = helpers::player_name_or_empty(world, target);
            if let Some(tcid) = crate::game_loop::helpers::client_for_player(world, target) {
                send_message(
                    world,
                    tcid,
                    &format!("Admin is removing you {exp} xp and {sp} sp."),
                );
            }
            super::death::remove_exp_and_sp(world, target, exp, sp);
            send_message(
                world,
                client_id,
                &format!("Removed {exp} xp and {sp} sp from {name}."),
            );
        }
        (2, Some(_), Some(_)) => {}
        _ => send_message(world, client_id, "Usage: //remove_exp_sp exp sp"),
    }
    admin_add_exp_sp_menu(world, client_id, object_id);
    Ok(())
}

/// `AdminExpSp`'s `addExpSp` (the `//add_exp_sp_to_character` command and the
/// trailing menu refresh after every add/remove): open `expsp.htm` for the
/// targeted player through `NpcHtmlMessage(0, 1)` — the item id 1 keeps the
/// window up when its Add/Remove/Set-Level buttons fire. Java requires a player
/// target, else `INVALID_TARGET`.
pub(super) fn admin_add_exp_sp_menu(world: &mut World, client_id: u32, object_id: i32) {
    let result = add_exp_sp_menu(world, client_id, object_id);
    guard::finish(world, client_id, result);
}

fn add_exp_sp_menu(world: &mut World, client_id: u32, object_id: i32) -> Guard<()> {
    let target = guard::player_target(world, object_id).or_sm(sm_ids::INVALID_TARGET)?;
    let Some(p) = world.objects.get_component::<Player>(&target) else {
        return Ok(());
    };
    // Java fills `%class%` via `ClassListData` client-code; the port has no
    // client-code table, so use the numeric class id (as `//character_info` does).
    let r: Vec<(&str, String)> = vec![
        ("name", p.name.clone()),
        ("level", p.level.to_string()),
        ("xp", p.exp.to_string()),
        ("sp", p.sp.to_string()),
        ("class", p.class_id.to_string()),
    ];
    super::menu::show_admin_html_replace(world, client_id, "expsp.htm", &r);
    Ok(())
}

/// `AdminLevel`'s `//add_level <n>` / `//set_level <n>` — add levels to, or set
/// the level of, the targeted player (or self). `set` chooses between the two.
pub(super) fn admin_change_level(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
    set: bool,
) {
    let Some(value) = nth_arg::<i32>(args, 0) else {
        send_message(
            world,
            client_id,
            if set {
                "Usage: //set_level <level>"
            } else {
                "Usage: //add_level <levels>"
            },
        );
        return;
    };
    let target = guard::player_target(world, object_id).unwrap_or(object_id);
    let Some(current) = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.level)
    else {
        return;
    };
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
pub(super) fn set_int_field(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    field: IntField,
    args: &[&str],
) {
    let Some(value) = nth_arg::<i32>(args, 0) else {
        send_message(
            world,
            client_id,
            &format!("Usage: //set{} <value>", field.label().to_lowercase()),
        );
        return;
    };
    set_field_value(world, client_id, object_id, field, value);
}

pub(super) fn set_field_value(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    field: IntField,
    value: i32,
) {
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
    send_message(
        world,
        client_id,
        &format!("{} set to {value}.", field.label()),
    );
}

/// `AdminVitality`'s `//set_vitality <n>` / `//full_vitality` / `//empty_vitality`
/// / `//get_vitality` — read or set the *targeted player's* vitality points
/// (Java requires a player target). Java passes `quiet = true` to
/// `setVitalityPoints`, so the player gets the gauge update and the UserInfo
/// broadcast but none of the "your vitality has increased/decreased" lines.
pub(super) fn admin_vitality(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    mode: &str,
    args: &[&str],
) {
    let Some(target) = guard::player_target(world, object_id) else {
        send_message(world, client_id, "Target not found or not a player");
        return;
    };
    match mode {
        "get" => {
            let v = world
                .objects
                .get_component::<Player>(&target)
                .map_or(0, |p| p.vitality_points);
            send_message(world, client_id, &format!("Player vitality points: {v}"));
            return;
        }
        "set" => {
            let Some(value) = nth_arg::<i32>(args, 0) else {
                send_message(world, client_id, "Incorrect vitality");
                return;
            };
            crate::game_loop::vitality::set_vitality_points(world, target, value, true);
        }
        "full" => {
            crate::game_loop::vitality::set_vitality_points(
                world,
                target,
                MAX_VITALITY_POINTS,
                true,
            );
        }
        "empty" => {
            crate::game_loop::vitality::set_vitality_points(
                world,
                target,
                MIN_VITALITY_POINTS,
                true,
            );
        }
        _ => {}
    }
    super::party::broadcast_user_info(world, target);
}

/// `AdminEditChar`'s `//setclass <id>` — change the target player's (or self's)
/// class. Reuses `set_level` at the current level to recompute vitals/stats and
/// grant the new class's skills, then rebroadcasts. With no argument, opens the
/// `setclass/` class-picker menu (Java's catch branch).
///
/// (A marker here claimed Java pruned the old class's skills. It does not:
/// `setClassId` → `rewardSkills` only ever *adds*, and its one removal path,
/// `checkPlayerSkills`, downgrades by **level**, never by class. That path is
/// already run via `set_level`. What was genuinely missing — dropping hennas
/// the new class may not wear — now happens in `subclass::set_class_id`.)
pub(super) fn admin_setclass(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    // Java: no (parseable) argument throws StringIndexOutOfBoundsException and
    // opens the class-picker menu instead of printing a usage line.
    let Some(class_id) = nth_arg::<i32>(args, 0) else {
        super::menu::show_admin_html(world, client_id, "setclass/human_fighter.htm");
        return;
    };
    if world.data.player_templates.get(class_id).is_none() {
        send_message(
            world,
            client_id,
            &format!("Class id {class_id} does not exist."),
        );
        return;
    }
    let target = target_player(world, object_id);
    // Routed through the shared occupation-change mechanic. This used to set
    // `base_class_id` unconditionally, which — now that subclasses exist —
    // would rewrite the character's *base* class while standing on a subclass.
    if crate::game_loop::subclass::set_class_id(world, target, class_id) {
        send_message(world, client_id, &format!("Class set to {class_id}."));
    } else {
        send_message(
            world,
            client_id,
            &format!("Class id {class_id} does not exist."),
        );
    }
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
pub(super) fn admin_set_color(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
    title: bool,
) {
    let Some(color) = args
        .first()
        .and_then(|s| i32::from_str_radix(s.trim_start_matches("0x"), 16).ok())
    else {
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
pub(super) fn admin_set_enchant(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    slot: PaperdollSlot,
    args: &[&str],
) {
    let Some(value) = nth_arg::<i32>(args, 0).filter(|v| (0..=127).contains(v)) else {
        send_message(world, client_id, "Usage: //set<slot> <0..127>");
        return;
    };
    let target = guard::player_target(world, object_id).unwrap_or(object_id);
    let changed = world
        .objects
        .get_component_mut::<Inventory>(&target)
        .and_then(|inv| inv.set_paperdoll_enchant(slot, value));
    let Some(item_oid) = changed else {
        send_message(world, client_id, "No item equipped in that slot.");
        return;
    };
    if let Some(cid) = super::helpers::client_for_player(world, target)
        && let Some(packet) = world
            .objects
            .get_component::<Inventory>(&target)
            .map(|inv| crate::network::enter_world::inventory_update(inv, &world.data, &[item_oid]))
    {
        super::helpers::send_inventory_update(world, cid, target, packet);
    }
    super::party::broadcast_user_info(world, target);
    send_message(world, client_id, &format!("Enchant set to +{value}."));
}

/// `//setsubclass <classId>` — add a subclass to the target (Java's
/// `AdminEditChar` opens a picker; the id form is what the GM panel bypasses
/// use). With no argument, lists the target's current slots.
pub(super) fn admin_setsubclass(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    use crate::game_loop::subclass::{AddError, add_subclass};

    let target = target_player(world, object_id);
    let Some(class_id) = nth_arg::<i32>(args, 0) else {
        let listing = world
            .objects
            .get_component::<Player>(&target)
            .map(|p| {
                let mut s = format!(
                    "Base class {} (index 0){}",
                    p.base_class_id,
                    if p.class_index == 0 { " [active]" } else { "" }
                );
                for sub in &p.subclasses {
                    s.push_str(&format!(
                        "\nSubclass {} (index {}, level {}){}",
                        sub.class_id,
                        sub.class_index,
                        sub.level,
                        if p.class_index == sub.class_index {
                            " [active]"
                        } else {
                            ""
                        }
                    ));
                }
                s
            })
            .unwrap_or_default();
        send_message(world, client_id, &listing);
        return;
    };

    match add_subclass(world, target, class_id) {
        Ok(index) => send_message(
            world,
            client_id,
            &format!("Added subclass {class_id} in slot {index}."),
        ),
        Err(AddError::SlotsFull) => send_message(world, client_id, "No free subclass slots."),
        Err(AddError::AlreadyHave) => send_message(world, client_id, "That class is already held."),
        Err(AddError::UnknownClass) => send_message(
            world,
            client_id,
            &format!("Class id {class_id} does not exist."),
        ),
    }
}

/// `//changesubclass <index>` — switch the target's active class (0 = base).
pub(super) fn admin_changesubclass(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let Some(index) = nth_arg::<i32>(args, 0) else {
        send_message(
            world,
            client_id,
            "Usage: //changesubclass <index> (0 = base class)",
        );
        return;
    };
    let target = target_player(world, object_id);
    if crate::game_loop::subclass::set_active_class(world, target, index) {
        send_message(
            world,
            client_id,
            &format!("Active class index is now {index}."),
        );
    } else {
        send_message(
            world,
            client_id,
            "No such subclass slot (or it is already active).",
        );
    }
}
