//! Loyalty/currency point commands — `AdminPcCafePoints` (`//pccafepoints`,
//! character-scoped `characters.pccafe_points`) and `AdminPrimePoints`
//! (`//primepoints`, account-scoped `account_gsdata` "PRIME_POINTS"). Both
//! operate on the current target player (or the GM when nothing/no player is
//! targeted) and re-render their HTML menu after every action, exactly like the
//! Java handlers.

use crate::game_loop::helpers::{format_amount, nth_arg};
use crate::model::Player;
use crate::network::server_packets;
use crate::world::World;

use super::{menu::show_admin_html_replace, send_message, target_player};
use crate::game_loop::helpers::player_name_or_empty;

/// `Config.PC_CAFE_MAX_POINTS` — the stored-value ceiling, read from
/// `config/Custom/PremiumSystem.ini` alongside the rest of the PC-café block.
///
/// It used to be inlined here as 200 000 with a comment crediting
/// `config/Custom/PcCafe.ini`. That file exists on this dist but **no Java
/// constant names it**, so nothing reads it — `CUSTOM_PREMIUM_SYSTEM_CONFIG_FILE`
/// is the only PC-café config path. Both files happen to say 200 000, so the
/// value was right and the provenance was not.
fn max_points(world: &World) -> i32 {
    world.cfg.premium.pc_cafe_max_points
}

/// `AdminPcCafePoints` — the `//pccafepoints [action] [value] [range]` command
/// and the `pccafe.htm` menu it renders. `action` ∈ `set`/`increase`/`decrease`
/// (target player or self) or `rewardOnline` (every online / in-range player).
/// With no action it just (re)opens the menu.
pub(super) fn admin_pccafepoints(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    if let Some(&action) = args.first() {
        let target = target_player(world, object_id);
        // Java: no value token after the action → `return false` (no menu, no
        // message); a present-but-non-numeric value → menu + "Invalid Value!".
        let Some(value_tok) = args.get(1) else { return };
        let Some(value) = value_tok.parse::<i32>().ok() else {
            show_pccafe_menu(world, client_id, object_id);
            send_message(world, client_id, "Invalid Value!");
            return;
        };
        let name = player_name_or_empty(world, target);
        let cur = points_of(world, target);
        match action {
            "set" => {
                if value > max_points(world) {
                    show_pccafe_menu(world, client_id, object_id);
                    send_message(
                        world,
                        client_id,
                        &format!("You cannot set more than {} PC points!", max_points(world)),
                    );
                    return;
                }
                let value = value.max(0);
                set_points(world, target, value);
                announce(
                    world,
                    client_id,
                    target,
                    &format!("Admin set your PC Cafe point(s) to {value}!"),
                    &format!("You set {value} PC Cafe point(s) to player {name}"),
                );
                send_pccafe_packet(world, target, value, value);
            }
            "increase" => {
                if cur == max_points(world) {
                    show_pccafe_menu(world, client_id, object_id);
                    send_message(
                        world,
                        client_id,
                        &format!("{name} already have max count of PC points!"),
                    );
                    return;
                }
                let new_count =
                    (cur as i64 + value as i64).clamp(0, max_points(world) as i64) as i32;
                set_points(world, target, new_count);
                announce(
                    world,
                    client_id,
                    target,
                    &format!("Admin increased your PC Cafe point(s) by {value}!"),
                    &format!("You increased PC Cafe point(s) of {name} by {value}"),
                );
                send_pccafe_packet(world, target, new_count, value);
            }
            "decrease" => {
                if cur == 0 {
                    show_pccafe_menu(world, client_id, object_id);
                    send_message(
                        world,
                        client_id,
                        &format!("{name} already have min count of PC points!"),
                    );
                    return;
                }
                let new_count = (cur - value).max(0);
                set_points(world, target, new_count);
                announce(
                    world,
                    client_id,
                    target,
                    &format!("Admin decreased your PC Cafe point(s) by {value}!"),
                    &format!("You decreased PC Cafe point(s) of {name} by {value}"),
                );
                send_pccafe_packet(world, target, points_of(world, target), -value);
            }
            "rewardOnline" => {
                // Java `rewardOnline`: the parsed `value` is the amount; the
                // optional next token is the range (default 0 = all online).
                let range = nth_arg::<i32>(args, 2).unwrap_or(0);
                let count = reward_online_pccafe(world, object_id, value, range);
                if range <= 0 {
                    send_message(
                        world,
                        client_id,
                        &format!(
                            "You increased PC Cafe point(s) of all online players ({count}) by {value}."
                        ),
                    );
                } else {
                    send_message(
                        world,
                        client_id,
                        &format!(
                            "You increased PC Cafe point(s) of all players ({count}) in range {range} by {value}."
                        ),
                    );
                }
            }
            _ => {}
        }
    }
    show_pccafe_menu(world, client_id, object_id);
}

/// Java `AdminPcCafePoints.increaseForAll` — raise every online player's (or
/// every in-range player's) PC-cafe points by `value`; returns how many.
fn reward_online_pccafe(world: &mut World, gm_oid: i32, value: i32, range: i32) -> i32 {
    let targets: Vec<i32> = if range <= 0 {
        world.in_game_player_oids().collect()
    } else {
        super::creatures_in_range(world, gm_oid, range, true, false)
    };
    let mut count = 0;
    for t in targets {
        let Some(cur) = world
            .objects
            .get_component::<Player>(&t)
            .map(|p| p.pccafe_points)
        else {
            continue;
        };
        let new_count = (cur as i64 + value as i64).clamp(0, max_points(world) as i64) as i32;
        set_points(world, t, new_count);
        send_player_message(
            world,
            t,
            &format!("Admin increased your PC Cafe point(s) by {value}!"),
        );
        send_pccafe_packet(world, t, new_count, value);
        count += 1;
    }
    count
}

/// `AdminPrimePoints` — `//primepoints [action] [value] [range]` and the
/// `primepoints.htm` menu. Prime (NCoin) points are account-scoped: Java stores
/// them in the `account_gsdata` "PRIME_POINTS" variable via
/// `AccountVariables.setPrimePoints`/`storeMe` (always `max(value, 0)`, capped
/// at `Integer.MAX_VALUE`). This port keeps a per-player mirror
/// (`Player.prime_points`, loaded at enter-world) and write-throughs each change
/// to the DB immediately. One account has one online character here, so the
/// per-player mirror is a documented multi-box simplification.
pub(super) fn admin_primepoints(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    if let Some(&action) = args.first() {
        let target = target_player(world, object_id);
        // Java: no value token → `return false` (no menu); non-numeric → menu +
        // "Invalid Value!".
        let Some(value_tok) = args.get(1) else { return };
        let Some(value) = value_tok.parse::<i32>().ok() else {
            show_primepoints_menu(world, client_id, object_id);
            send_message(world, client_id, "Invalid Value!");
            return;
        };
        let name = player_name_or_empty(world, target);
        let cur = prime_of(world, target);
        match action {
            "set" => {
                set_prime(world, target, value);
                let stored = value.max(0);
                announce(
                    world,
                    client_id,
                    target,
                    &format!("Admin set your Prime Point(s) to {stored}!"),
                    &format!("You set {stored} Prime Point(s) to player {name}"),
                );
            }
            "increase" => {
                if cur == i32::MAX {
                    show_primepoints_menu(world, client_id, object_id);
                    send_message(
                        world,
                        client_id,
                        &format!("{name} already have max count of Prime Points!"),
                    );
                    return;
                }
                let new_count = (cur as i64 + value as i64).clamp(0, i32::MAX as i64) as i32;
                set_prime(world, target, new_count);
                announce(
                    world,
                    client_id,
                    target,
                    &format!("Admin increase your Prime Point(s) by {value}!"),
                    &format!("You increased Prime Point(s) of {name} by {value}"),
                );
            }
            "decrease" => {
                if cur == 0 {
                    show_primepoints_menu(world, client_id, object_id);
                    send_message(
                        world,
                        client_id,
                        &format!("{name} already have min count of Prime Points!"),
                    );
                    return;
                }
                let new_count = (cur - value).max(0);
                set_prime(world, target, new_count);
                announce(
                    world,
                    client_id,
                    target,
                    &format!("Admin decreased your Prime Point(s) by {value}!"),
                    &format!("You decreased Prime Point(s) of {name} by {value}"),
                );
            }
            "rewardOnline" => {
                let range = nth_arg::<i32>(args, 2).unwrap_or(0);
                let count = reward_online_prime(world, object_id, value, range);
                if range <= 0 {
                    send_message(
                        world,
                        client_id,
                        &format!(
                            "You increased Prime Point(s) of all online players ({count}) by {value}."
                        ),
                    );
                } else {
                    send_message(
                        world,
                        client_id,
                        &format!(
                            "You increased Prime Point(s) of all players ({count}) in range {range} by {value}."
                        ),
                    );
                }
            }
            _ => {}
        }
    }
    show_primepoints_menu(world, client_id, object_id);
}

/// Java `AdminPrimePoints.increaseForAll` — raise every online (or in-range)
/// player's prime points by `value`; returns how many were affected.
fn reward_online_prime(world: &mut World, gm_oid: i32, value: i32, range: i32) -> i32 {
    let targets: Vec<i32> = if range <= 0 {
        world.in_game_player_oids().collect()
    } else {
        super::creatures_in_range(world, gm_oid, range, true, false)
    };
    let mut count = 0;
    for t in targets {
        let Some(cur) = world
            .objects
            .get_component::<Player>(&t)
            .map(|p| p.prime_points)
        else {
            continue;
        };
        let new_count = (cur as i64 + value as i64).clamp(0, i32::MAX as i64) as i32;
        set_prime(world, t, new_count);
        send_player_message(
            world,
            t,
            &format!("Admin increase your Prime Point(s) by {value}!"),
        );
        count += 1;
    }
    count
}

fn show_primepoints_menu(world: &World, client_id: u32, object_id: i32) {
    let target = target_player(world, object_id);
    let points = format_amount(prime_of(world, target) as i64);
    let name = player_name_or_empty(world, target);
    show_admin_html_replace(
        world,
        client_id,
        "primepoints.htm",
        &[("points", points), ("targetName", name)],
    );
}

fn prime_of(world: &World, target: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&target)
        .map_or(0, |p| p.prime_points)
}

/// Java `Player.setPrimePoints` — store `max(value, 0)` on the account var,
/// with immediate write-through to `account_gsdata` (Java `storeMe`).
fn set_prime(world: &mut World, target: i32, value: i32) {
    let value = value.max(0);
    let Some(account) = ({
        let p = world.objects.get_component_mut::<Player>(&target);
        p.map(|p| {
            p.prime_points = value;
            p.account.clone()
        })
    }) else {
        return;
    };
    let _ = world.db.send(crate::db::DbCommand::StoreAccountVar {
        account_name: account,
        var: "PRIME_POINTS".to_string(),
        value: value.to_string(),
    });
}

fn show_pccafe_menu(world: &World, client_id: u32, object_id: i32) {
    let target = target_player(world, object_id);
    let points = format_amount(points_of(world, target) as i64);
    let name = player_name_or_empty(world, target);
    show_admin_html_replace(
        world,
        client_id,
        "pccafe.htm",
        &[("points", points), ("targetName", name)],
    );
}

fn points_of(world: &World, target: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&target)
        .map_or(0, |p| p.pccafe_points)
}

/// Java `Player.setPcCafePoints` — store the value capped at the max.
fn set_points(world: &mut World, target: i32, value: i32) {
    let capped = value.min(max_points(world));
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.pccafe_points = capped;
    }
}

/// Java `target.sendMessage(...)` — a system message to the target player (the
/// GM themselves when self-targeted).
/// Tell the target what a GM just did to them, and confirm it back to the GM.
///
/// Every `set` / `increase` / `decrease` arm of both point commands sends this
/// pair; only the two message texts differ.
fn announce(world: &World, client_id: u32, target: i32, to_target: &str, to_gm: &str) {
    send_player_message(world, target, to_target);
    send_message(world, client_id, to_gm);
}

fn send_player_message(world: &World, target: i32, text: &str) {
    if let Some(cid) = super::helpers::client_for_player(world, target) {
        send_message(world, cid, text);
    }
}

/// Push an `ExPCCafePointInfo` to the target player so the client updates its
/// point display (`time = 1`, matching the Java admin handler).
fn send_pccafe_packet(world: &World, target: i32, points: i32, add: i32) {
    super::helpers::send_to_player(
        world,
        target,
        server_packets::ex_pccafe_point_info(points, add, 1),
    );
}
