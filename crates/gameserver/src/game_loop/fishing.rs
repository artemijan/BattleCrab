//! Fishing engine (G32) — the new single-action system (Java
//! `model/fishing/Fishing`). Toggle auto-fish (`ExRequestAutoFish`): with a
//! fishing rod equipped and a bait hooked in the off-hand, the line casts, waits
//! out the bait's reel time, and reels in on the bait's win chance — a hit rolls
//! the bait's catch table for a fish (consuming one bait) and awards XP/SP; then
//! it auto-recasts after the wait window.
//!
//! The cast is gated on the 13 `FishingZone`s (`fishing.xml`) at both entry
//! points — the header claim that "no FishingZone is loaded yet" closed under
//! it. The bob still lands at a heading offset rather than a geo-validated
//! water point, which no zone can express; cosmetic only.

use crate::data::item_data::WeaponType;
use crate::data::zone_data::ZoneKind;
use crate::game_loop::guard::{in_zone, maybe_position};
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::region_cell_of;
use crate::model::Player;
use crate::model::components::FishingSession;
use crate::model::inventory::{Inventory, PaperdollSlot};
use crate::network::server_packets as sp;
use crate::scheduler::ScheduledTask;
use crate::world::World;

use super::helpers::send_to_player as send;
use super::helpers::{broadcast_near_region, client_for_player};

// FishingEndReason (Java enum ordinals).
const REASON_WIN: u8 = 0;
const REASON_LOSE: u8 = 1;
const REASON_STOP: u8 = 2;

/// `ExRequestAutoFish`: toggle the auto-fishing loop.
pub(crate) fn toggle_fishing(world: &mut World, player: i32) {
    let fishing = world
        .objects
        .get_component::<FishingSession>(&player)
        .map(|f| f.is_fishing)
        .unwrap_or(false);
    if fishing {
        stop_fishing(world, player, REASON_STOP);
    } else {
        start_fishing(world, player);
    }
}

fn start_fishing(world: &mut World, player: i32) {
    set_session(world, player, |f| f.is_fishing = true);
    cast_line(world, player);
}

/// Java `Player.getActiveWeaponInstance()` (RHand) item id, or 0.
fn equipped_rod(world: &World, player: i32) -> i32 {
    world
        .objects
        .get_component::<Inventory>(&player)
        .map(|inv| inv.paperdoll_item_id(PaperdollSlot::RHand))
        .unwrap_or(0)
}

/// The bait item hooked in the off-hand (LHand), or 0.
fn equipped_bait(world: &World, player: i32) -> i32 {
    world
        .objects
        .get_component::<Inventory>(&player)
        .map(|inv| inv.paperdoll_item_id(PaperdollSlot::LHand))
        .unwrap_or(0)
}

/// Whether the client's auto-fish button should be lit (Java `FishingZone`'s
/// `ExAutoFishAvailable` poll, condensed): the player stands in a FishingZone
/// and meets [`can_fish`] (rod, bait, level, not underwater). Fired on zone
/// transitions from [`revalidate_zone`](super::zones::revalidate_zone).
pub(crate) fn fishing_available(world: &World, player: i32) -> bool {
    in_zone(world, player, ZoneKind::Fishing) && can_fish(world, player)
}

/// Java `Fishing.canFish` (slice-1 subset): alive, a real fishing rod equipped,
/// a known bait hooked, and the player's level within the bait's range.
fn can_fish(world: &World, player: i32) -> bool {
    let dead = is_dead(world, player);
    if dead {
        return false;
    }
    let rod = equipped_rod(world, player);
    if world.data.item_data.weapon_type(rod) != WeaponType::FishingRod
        || world.data.fishing_data.rod(rod).is_none()
    {
        return false;
    }
    let bait = equipped_bait(world, player);
    let Some(bait_data) = world.data.fishing_data.bait(bait) else {
        return false;
    };
    // Premium-only bait needs a premium account.
    if bait_data.premium_only && !super::admin::premium::has_premium_status(world, player) {
        return false;
    }
    // `isInsideZone(ZoneId.WATER)` — no fishing while swimming.
    if let Some(pos) = maybe_position(world, player) {
        let in_water = world
            .data
            .zone_data
            .zones_at(pos.x, pos.y, pos.z)
            .any(|z| z.kind == ZoneKind::Water);
        if in_water {
            return false;
        }
    }
    let level = world
        .objects
        .get_component::<Player>(&player)
        .map(|p| p.level)
        .unwrap_or(0);
    level >= bait_data.min_player_level && level <= bait_data.max_player_level
}

fn cast_line(world: &mut World, player: i32) {
    if !can_fish(world, player) {
        stop_fishing(world, player, REASON_STOP);
        return;
    }
    let bait = equipped_bait(world, player);
    let rod = equipped_rod(world, player);
    // Reel/wait windows come from the bait; the rod shaves the reel time.
    let (time_min, wait_min, reduce) = {
        let bd = world.data.fishing_data.bait(bait).unwrap();
        let reduce = world
            .data
            .fishing_data
            .rod(rod)
            .map(|r| r.reduce_fishing_time)
            .unwrap_or(0);
        (bd.time_min, bd.wait_min, reduce)
    };
    let fishing_time = (time_min - reduce).max(1000);

    // Java `castLine`: the cast fails ("you can't fish here") unless the player
    // stands in a FishingZone *and* the bob lands on a WaterZone.
    let region = region_cell_of(world, player);
    let Some((bx, by, bz)) = calculate_bait_location(world, player) else {
        // Java's branch splits on `_isFishing`: a *fresh* cast in a bad spot is
        // told why, a re-cast mid-session only gets `ActionFailed` (its commented
        // -out "attempt cancelled" line stays commented out here too).
        let already_fishing = world
            .objects
            .get_component::<FishingSession>(&player)
            .is_some_and(|f| f.is_fishing);
        if !already_fishing {
            send(
                world,
                player,
                sp::system_message_with(sp::sm_ids::YOU_CAN_T_FISH_HERE, &[]),
            );
        }
        send(world, player, sp::action_failed());
        stop_fishing(world, player, REASON_STOP);
        return;
    };
    // Java: charge fishing shots for this cast if the player has them auto-on.
    if !is_charged_fish_shot(world, player) {
        super::items::recharge_shots(world, player, false, false, true);
    }
    let seq = world.next_request_seq();
    set_session(world, player, |f| {
        f.cast_seq = seq;
        f.bait_x = bx;
        f.bait_y = by;
        f.bait_z = bz;
    });
    let fire_at = world.tick + (fishing_time as u64).div_ceil(100);
    world.scheduler.schedule(
        fire_at,
        ScheduledTask::FishingReel {
            player_object_id: player,
            cast_seq: seq,
        },
    );
    let _ = wait_min; // used by the reel handler when scheduling the next cast

    let start = sp::ex_fishing_start(player, (bx, by, bz));
    if let Some(region) = region {
        broadcast_near_region(world, region, &start);
    }
    send(
        world,
        player,
        sp::ex_user_info_fishing(player, true, (bx, by, bz)),
    );
}

/// The `FishingReel` scheduled task: reel in (win/lose), then queue the next cast.
pub(crate) fn handle_reel(world: &mut World, player: i32, cast_seq: u64) {
    if !session_seq_matches(world, player, cast_seq) {
        return;
    }
    reel_in_with_reward(world, player);
    // Auto-recast after the bait's wait window (unless the reel stopped us).
    let still_fishing = world
        .objects
        .get_component::<FishingSession>(&player)
        .is_some_and(|f| f.is_fishing);
    if !still_fishing {
        return;
    }
    let wait = world
        .objects
        .get_component::<Inventory>(&player)
        .map(|inv| inv.paperdoll_item_id(PaperdollSlot::LHand))
        .and_then(|bait| world.data.fishing_data.bait(bait))
        .map(|b| b.wait_min)
        .unwrap_or(15000);
    let seq = current_seq(world, player);
    let fire_at = world.tick + (wait as u64).max(1).div_ceil(100);
    world.scheduler.schedule(
        fire_at,
        ScheduledTask::FishingCast {
            player_object_id: player,
            cast_seq: seq,
        },
    );
}

/// The `FishingCast` scheduled task: the wait elapsed, cast again.
pub(crate) fn handle_cast(world: &mut World, player: i32, cast_seq: u64) {
    if !session_seq_matches(world, player, cast_seq) {
        return;
    }
    cast_line(world, player);
}

fn reel_in_with_reward(world: &mut World, player: i32) {
    let bait = equipped_bait(world, player);
    let mut chance = match world.data.fishing_data.bait(bait) {
        Some(b) => b.chance,
        None => {
            reel_in(world, player, false, false);
            return;
        }
    };
    // Fishing shots double the win chance.
    if is_charged_fish_shot(world, player) {
        chance *= 2;
    }
    let win = world.roll(100) <= chance;
    reel_in(world, player, win, true);
}

fn is_charged_fish_shot(world: &World, player: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&player)
        .is_some_and(|p| p.is_charged_shot(crate::model::ShotType::FishSoulshots))
}

fn reel_in(world: &mut World, player: i32, win: bool, consume_bait: bool) {
    let bait = equipped_bait(world, player);
    let client_id = client_for_player(world, player);

    if consume_bait && !super::quests::take_items(world, client_id.unwrap_or(0), player, bait, 1) {
        // No bait — no reward.
        broadcast_end(world, player, REASON_LOSE);
        return;
    }

    let mut reason = if win { REASON_WIN } else { REASON_LOSE };
    if win {
        // Roll the bait's catch table.
        let roll = world.roll(100);
        let (fish, xp, sp_amt) = {
            let level = world
                .objects
                .get_component::<Player>(&player)
                .map(|p| p.level)
                .unwrap_or(1);
            let fd = &world.data.fishing_data;
            match fd.bait(bait).and_then(|b| b.pick_catch(roll)) {
                Some(c) => {
                    let lvl_mod = (level as f64).powf(2.2) * c.multiplier as f64;
                    let xp = (fd.xp_rate_min * lvl_mod) as i64;
                    let sp = (fd.sp_rate_min * lvl_mod) as i64;
                    (c.item_id, xp, sp)
                }
                None => (0, 0, 0),
            }
        };
        if fish != 0 {
            super::quests::give_item_with_earned_message(
                world,
                client_id.unwrap_or(0),
                player,
                fish,
                1,
            );
            if xp > 0 || sp_amt > 0 {
                super::death::add_exp_and_sp(world, player, xp as f64, sp_amt as f64, true);
            }
            // Java: a landed catch spends the charged fishing shot; re-charge for
            // the next cast if the player still has shots auto-on.
            if let Some(p) = world.objects.get_component_mut::<Player>(&player) {
                p.uncharge_shot(crate::model::ShotType::FishSoulshots);
            }
            super::items::recharge_shots(world, player, false, false, true);
        } else {
            reason = REASON_LOSE;
        }
    }
    broadcast_end(world, player, reason);
}

pub(crate) fn stop_fishing(world: &mut World, player: i32, reason: u8) {
    let was_fishing = world
        .objects
        .get_component::<FishingSession>(&player)
        .is_some_and(|f| f.is_fishing);
    // Bump the seq so any in-flight reel/cast task no-ops, and clear the flag.
    let seq = world.next_request_seq();
    set_session(world, player, |f| {
        f.is_fishing = false;
        f.cast_seq = seq;
    });
    if was_fishing {
        broadcast_end(world, player, reason);
        send(
            world,
            player,
            sp::ex_user_info_fishing(player, false, (0, 0, 0)),
        );
    }
}

// --- helpers ---

fn broadcast_end(world: &World, player: i32, reason: u8) {
    let pkt = sp::ex_fishing_end(player, reason);
    crate::game_loop::helpers::broadcast_from(world, player, &pkt);
}

/// Java `Fishing.calculateBaitLocation`: the bob lands `baitDistance` ahead of
/// the player along their heading. Fails (`None` → "you can't fish here")
/// unless the player is in a **FishingZone** and the bob's `(x, y)` is over a
/// **WaterZone** — whose upper Z (`getWaterZ`) is the bob's Z. Geo height checks
/// are elided (the port has no per-cell geo in most zones).
fn calculate_bait_location(world: &World, player: i32) -> Option<(i32, i32, i32)> {
    let p = maybe_position(world, player)?;
    // The player must stand in a fishing zone.
    let in_fishing_zone = world
        .data
        .zone_data
        .zones_at(p.x, p.y, p.z)
        .any(|z| z.kind == ZoneKind::Fishing);
    if !in_fishing_zone {
        return None;
    }
    let dist = world.data.fishing_data.bait_distance_min as f64;
    let angle = p.heading as f64 * (std::f64::consts::TAU / 65536.0);
    let bx = p.x + (dist * angle.cos()) as i32;
    let by = p.y + (dist * angle.sin()) as i32;
    // The bob must land over water; the water surface (zone's upper Z) is its Z.
    let water_z = world
        .data
        .zone_data
        .zones_at(bx, by, p.z)
        .find(|z| z.kind == ZoneKind::Water)
        .map(|z| z.territory.max_z)?;
    Some((bx, by, water_z))
}

fn set_session(world: &mut World, player: i32, f: impl FnOnce(&mut FishingSession)) {
    if let Some(s) = world.objects.get_component_mut::<FishingSession>(&player) {
        f(s);
    } else {
        let mut s = FishingSession::default();
        f(&mut s);
        world.objects.add_components(&player, s);
    }
}

fn current_seq(world: &World, player: i32) -> u64 {
    world
        .objects
        .get_component::<FishingSession>(&player)
        .map(|f| f.cast_seq)
        .unwrap_or(0)
}

fn session_seq_matches(world: &World, player: i32, seq: u64) -> bool {
    world
        .objects
        .get_component::<FishingSession>(&player)
        .is_some_and(|f| f.is_fishing && f.cast_seq == seq)
}
