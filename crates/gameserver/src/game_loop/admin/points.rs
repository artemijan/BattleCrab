//! Loyalty/currency point commands — `AdminPcCafePoints` (`//pccafepoints`).
//! `AdminPrimePoints` (`//primepoints`) joins here once the account-variable
//! store lands (Stage 3). Both operate on the current target player (or the GM
//! when nothing/no player is targeted) and re-render their HTML menu after every
//! action, exactly like the Java handlers.

use crate::model::Player;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

use super::{menu::show_admin_html_replace, send_message, target_player};

/// `Config.PC_CAFE_MAX_POINTS` — the stored-value ceiling. Sourced from this
/// dist's `config/Custom/PcCafe.ini` (`MaxPcCafePoints = 200000`, the Java
/// default); a dedicated PcCafe config loader is not ported, so the authoritative
/// dist value is inlined here (matching the "dist data is the spec" rule).
const PC_CAFE_MAX_POINTS: i32 = 200_000;

/// `AdminPcCafePoints` — the `//pccafepoints [action] [value] [range]` command
/// and the `pccafe.htm` menu it renders. `action` ∈ `set`/`increase`/`decrease`
/// (target player or self) or `rewardOnline` (every online / in-range player).
/// With no action it just (re)opens the menu.
pub(super) fn admin_pccafepoints(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    if let Some(&action) = args.first() {
        if action == "rewardOnline" {
            reward_online_cmd(world, client_id, object_id, args);
            show_pccafe_menu(world, client_id, object_id);
            return;
        }

        let target = target_player(world, object_id);
        // Java: a missing/invalid value re-shows the menu with "Invalid Value!".
        let Some(value) = args.get(1).and_then(|s| s.parse::<i32>().ok()) else {
            show_pccafe_menu(world, client_id, object_id);
            send_message(world, client_id, "Invalid Value!");
            return;
        };
        let name = name_of(world, target);
        let cur = points_of(world, target);
        match action {
            "set" => {
                if value > PC_CAFE_MAX_POINTS {
                    show_pccafe_menu(world, client_id, object_id);
                    send_message(world, client_id, &format!("You cannot set more than {PC_CAFE_MAX_POINTS} PC points!"));
                    return;
                }
                let value = value.max(0);
                set_points(world, target, value);
                send_player_message(world, target, &format!("Admin set your PC Cafe point(s) to {value}!"));
                send_message(world, client_id, &format!("You set {value} PC Cafe point(s) to player {name}"));
                send_pccafe_packet(world, target, value, value);
            }
            "increase" => {
                if cur == PC_CAFE_MAX_POINTS {
                    show_pccafe_menu(world, client_id, object_id);
                    send_message(world, client_id, &format!("{name} already have max count of PC points!"));
                    return;
                }
                let new_count = (cur as i64 + value as i64).clamp(0, PC_CAFE_MAX_POINTS as i64) as i32;
                set_points(world, target, new_count);
                send_player_message(world, target, &format!("Admin increased your PC Cafe point(s) by {value}!"));
                send_message(world, client_id, &format!("You increased PC Cafe point(s) of {name} by {value}"));
                send_pccafe_packet(world, target, new_count, value);
            }
            "decrease" => {
                if cur == 0 {
                    show_pccafe_menu(world, client_id, object_id);
                    send_message(world, client_id, &format!("{name} already have min count of PC points!"));
                    return;
                }
                let new_count = (cur - value).max(0);
                set_points(world, target, new_count);
                send_player_message(world, target, &format!("Admin decreased your PC Cafe point(s) by {value}!"));
                send_message(world, client_id, &format!("You decreased PC Cafe point(s) of {name} by {value}"));
                send_pccafe_packet(world, target, points_of(world, target), -value);
            }
            _ => {}
        }
    }
    show_pccafe_menu(world, client_id, object_id);
}

/// `//pccafepoints rewardOnline <value> [range]` — increase every online player
/// (range ≤ 0) or every player within `range` of the GM.
fn reward_online_cmd(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(value) = args.get(1).and_then(|s| s.parse::<i32>().ok()) else {
        show_pccafe_menu(world, client_id, object_id);
        send_message(world, client_id, "Invalid Value!");
        return;
    };
    let range = args.get(2).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
    let targets: Vec<i32> = if range <= 0 {
        world
            .clients
            .values()
            .filter_map(|cs| match cs {
                ClientSession::InGame(s) => Some(s.player_object_id()),
                _ => None,
            })
            .collect()
    } else {
        super::creatures_in_range(world, object_id, range, true, false)
    };
    let mut count = 0;
    for t in targets {
        let Some(cur) = world.objects.get_component::<Player>(&t).map(|p| p.pccafe_points) else { continue };
        let new_count = (cur as i64 + value as i64).clamp(0, PC_CAFE_MAX_POINTS as i64) as i32;
        set_points(world, t, new_count);
        send_player_message(world, t, &format!("Admin increased your PC Cafe point(s) by {value}!"));
        send_pccafe_packet(world, t, new_count, value);
        count += 1;
    }
    if range <= 0 {
        send_message(world, client_id, &format!("You increased PC Cafe point(s) of all online players ({count}) by {value}."));
    } else {
        send_message(world, client_id, &format!("You increased PC Cafe point(s) of all players ({count}) in range {range} by {value}."));
    }
}

fn show_pccafe_menu(world: &World, client_id: u32, object_id: i32) {
    let target = target_player(world, object_id);
    let points = format_adena(points_of(world, target));
    let name = name_of(world, target);
    show_admin_html_replace(world, client_id, "pccafe.htm", &[("points", points), ("targetName", name)]);
}

fn points_of(world: &World, target: i32) -> i32 {
    world.objects.get_component::<Player>(&target).map_or(0, |p| p.pccafe_points)
}

/// Java `Player.setPcCafePoints` — store the value capped at the max.
fn set_points(world: &mut World, target: i32, value: i32) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.pccafe_points = value.min(PC_CAFE_MAX_POINTS);
    }
}

fn name_of(world: &World, target: i32) -> String {
    world.objects.get_component::<Player>(&target).map(|p| p.name.clone()).unwrap_or_default()
}

/// Java `target.sendMessage(...)` — a system message to the target player (the
/// GM themselves when self-targeted).
fn send_player_message(world: &World, target: i32, text: &str) {
    if let Some(cid) = super::helpers::client_for_player(world, target) {
        send_message(world, cid, text);
    }
}

/// Push an `ExPCCafePointInfo` to the target player so the client updates its
/// point display (`time = 1`, matching the Java admin handler).
fn send_pccafe_packet(world: &World, target: i32, points: i32, add: i32) {
    if let Some(cid) = super::helpers::client_for_player(world, target) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(server_packets::ex_pccafe_point_info(points, add, 1));
        }
    }
}

/// `Util.formatAdena` — group digits into thousands with commas (`200000` →
/// `"200,000"`). Points are never negative, but a sign is handled for safety.
pub(super) fn format_adena(value: i32) -> String {
    let neg = value < 0;
    let digits = value.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if neg {
        format!("-{out}")
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::format_adena;

    #[test]
    fn formats_thousands() {
        assert_eq!(format_adena(0), "0");
        assert_eq!(format_adena(999), "999");
        assert_eq!(format_adena(1_000), "1,000");
        assert_eq!(format_adena(200_000), "200,000");
        assert_eq!(format_adena(1_234_567), "1,234,567");
        assert_eq!(format_adena(-4_200), "-4,200");
    }
}
