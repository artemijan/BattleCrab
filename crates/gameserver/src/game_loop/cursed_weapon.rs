//! Cursed weapons — the autonomous gameplay loop (G28, PLAN_G28_CURSED_WEAPONS.md).
//!
//! The activation engine (`activate` / `end_of_life`) lives in
//! [`super::admin::cursed_weapons`], where the `//cw_*` GM commands drove it
//! first; this module wires the parts that make a cursed weapon enter and leave
//! the world through *play*: a slain monster has a tiny chance to **drop** one
//! (`CursedWeaponsManager.checkDrop` → `CursedWeapon.checkDrop`), a player who
//! **picks it up** becomes cursed (reusing `activate`), and the weapon
//! **expires** when its life runs out — the `RemoveTask` deadline for both the
//! un-grabbed drop and the wielder — and a wielder who relogs comes back
//! **still cursed** ([`on_enter_world`]). (The kill-count level-up, the
//! "hungry" HP/time decay and drop-on-PK-death are a follow-up slice —
//! `TODO(G28)`.)

use crate::model::Player;
use crate::model::components::{Position, RegionCell, SkillBook};
use crate::model::inventory::Inventory;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

use super::admin::cursed_weapons::{activate, end_of_life, idx_by_item, now_millis};
use super::ground_items::{DropSource, despawn_ground_item, spawn_ground_item};

const TICKS_PER_SECOND: u64 = 10;
const MILLIS_PER_MINUTE: i64 = 60_000;
/// Java `CursedWeapon.dropRate` is out of 100000 (config comment "100000 for
/// 100%"), so a value of 50 is 0.05%.
const DROP_RATE_SCALE: i32 = 100_000;

/// `CursedWeaponsManager.checkDrop(attackable, player)` — a monster slain by a
/// player may drop a not-yet-in-world cursed weapon. No-op unless the killer is
/// a real, un-cursed player and the victim is an ordinary monster (Java
/// excludes `Defender`/`Guard`/`GrandBoss`/`FeedableBeast`/`FortCommander`).
pub(crate) fn on_monster_killed(world: &mut World, monster_oid: i32, killer_oid: i32) {
    if world.cursed_weapons.is_empty() {
        return;
    }
    // Every not-in-world weapon rolls; the first to hit drops (Java breaks).
    let candidates: Vec<usize> = (0..world.cursed_weapons.len())
        .filter(|&i| !world.cursed_weapons[i].is_active())
        .collect();
    if candidates.is_empty() {
        return;
    }

    let killer = super::pvp::acting_player(world, killer_oid);
    let eligible_killer = world
        .objects
        .get_component::<Player>(&killer)
        .is_some_and(|p| p.cursed_weapon_equipped_id == 0);
    if !eligible_killer {
        return;
    }
    // Ordinary monster only — `is_monster()` covers the Monster subtree
    // (including raids/feedable beasts), so subtract the excluded kinds.
    let ordinary = world
        .objects
        .get_component::<crate::model::npc::Npc>(&monster_oid)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.is_monster() && !t.is_raid() && t.type_name != "FeedableBeast");
    if !ordinary {
        return;
    }
    let Some(pos) = world
        .objects
        .get_component::<Position>(&monster_oid)
        .copied()
    else {
        return;
    };

    for idx in candidates {
        let drop_rate = world.cursed_weapons[idx].drop_rate;
        if world.roll(DROP_RATE_SCALE) < drop_rate {
            drop_weapon(world, idx, killer, pos.x, pos.y, pos.z);
            break;
        }
    }
}

/// `CursedWeapon.dropIt(attackable, player)` + the `checkDrop` tail: spawn the
/// weapon on the kill site (exempt from auto-destroy), red-sky + earthquake to
/// everyone, arm the full-`duration` life task, and announce the drop.
fn drop_weapon(world: &mut World, idx: usize, killer: i32, x: i32, y: i32, z: i32) {
    let (item_id, duration) = {
        let cw = &world.cursed_weapons[idx];
        (cw.item_id, cw.duration)
    };
    let oid = spawn_ground_item(world, item_id, 1, 0, x, y, z, 0, DropSource::CursedWeapon);

    // RedSky + Earthquake at the drop site (Java `dropIt`, fromMonster branch).
    broadcast_to_all(world, &server_packets::ex_red_sky(10));
    let quake = {
        let p = world
            .objects
            .get_component::<Position>(&killer)
            .copied()
            .unwrap_or(Position {
                x,
                y,
                z,
                heading: 0,
            });
        server_packets::earthquake(p.x, p.y, p.z, 14, 3)
    };
    broadcast_to_all(world, &quake);

    // Java's `checkDrop` arms the life task for the FULL duration (not
    // durationLost) — the ground weapon lives just as long as a wielded one.
    let deadline = now_millis() + (duration as i64) * MILLIS_PER_MINUTE;
    {
        let cw = &mut world.cursed_weapons[idx];
        cw.is_activated = false;
        cw.is_dropped = true;
        cw.dropped_item_oid = oid;
        cw.player_id = 0;
        cw.nb_kills = 0;
        cw.end_time = deadline;
    }
    // "$s2 was dropped in the $s1 region." — region SysString is a TODO(G28)
    // (MapRegion carries no sysstring id yet), so the region renders blank.
    let announce = server_packets::system_message_with(
        sm_ids::S2_WAS_DROPPED_IN_THE_S1_REGION,
        &[SmParam::SysString(0), SmParam::ItemName(item_id)],
    );
    broadcast_to_all(world, &announce);
    arm_expiry(world, idx);
}

/// Whether `item_id` is a cursed weapon currently lying on the ground — the
/// gate `pickup_ground_item` uses to route into [`try_pickup`].
pub(crate) fn is_dropped_cursed(world: &World, item_id: i32) -> bool {
    idx_by_item(world, item_id).is_some_and(|i| world.cursed_weapons[i].is_dropped)
}

/// `CursedWeaponsManager.activate(player, item)` for a picked-up drop: the
/// pickup animation, despawn, then either curse an un-cursed picker (the common
/// case) or silently consume the weapon if the picker already wields one.
pub(crate) fn try_pickup(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    item_oid: i32,
    region: (i32, i32),
    item_id: i32,
    pos: Position,
) {
    let Some(idx) = idx_by_item(world, item_id) else {
        return;
    };

    // Pickup animation to nearby, then remove the ground item.
    super::helpers::broadcast_near_region(
        world,
        region,
        &server_packets::get_item(player_oid, item_oid, pos.x, pos.y, pos.z),
    );
    despawn_ground_item(world, item_oid, region);
    {
        let cw = &mut world.cursed_weapons[idx];
        cw.is_dropped = false;
        cw.dropped_item_oid = 0;
    }

    let already_cursed = world
        .objects
        .get_component::<Player>(&player_oid)
        .is_some_and(|p| p.cursed_weapon_equipped_id != 0);
    if already_cursed {
        // Java erases the newly obtained weapon (and grants its existing one a
        // stage bonus). The kill-count level-up is a later slice, so here the
        // freshly grabbed weapon simply vanishes back to "not in world".
        // TODO(G28): grant the wielded weapon `stageKills - 1` + increaseKills.
        world.cursed_weapons[idx].reset();
        let _ = client_id;
        return;
    }

    // Curse the picker (equip + transform + skill + full heal + announce). The
    // drop already armed the life task at the full-duration deadline; `activate`
    // resets end_time to now + duration, so the picker restarts the clock.
    // TODO(G28): Java preserves the drop's end_time (total on-ground + wielded
    // life is one `duration`); the reset grants the ground-lying time back.
    activate(world, idx, player_oid);
    arm_expiry(world, idx);
}

// ---------------------------------------------------------------------------
// Login restore — `CursedWeaponsManager.checkPlayer` + `CursedWeapon.cursedOnLogin`
// ---------------------------------------------------------------------------

/// The curse survives a relog. Java splits this across two call sites:
/// `Player.restore` → `CursedWeaponsManager.checkPlayer` re-binds the weapon to
/// the freshly loaded character (`cursedWeaponEquippedId` + `giveSkill` + the
/// time-left notice), and `EnterWorld.runImpl` → `CursedWeapon.cursedOnLogin`
/// then re-applies the transform, re-grants the skill and announces the login.
/// Both halves land here, at Java's `EnterWorld` position — right after
/// `spawnMe`.
///
/// Without it a relog quietly *lifted* the curse: the character came back
/// holding an ordinary-looking sword, un-transformed and without the cursed
/// skill, and every `isCursedWeaponEquipped()` gate downstream (weapon swap,
/// party join, Olympiad, mounts, support magic) read `false`.
///
/// The `None` tail is `EnterWorld`'s "Remove demonic weapon if character is not
/// cursed weapon equipped" sweep — the safety net for a weapon whose life ran
/// out while its owner was offline, which leaves the item behind in their bag.
pub(crate) fn on_enter_world(world: &mut World, client_id: u32, object_id: i32) {
    let Some(idx) = world
        .cursed_weapons
        .iter()
        .position(|cw| cw.is_activated && cw.player_id == object_id)
    else {
        destroy_stray_cursed_items(world, client_id, object_id);
        return;
    };
    let item_id = world.cursed_weapons[idx].item_id;

    // `checkPlayer`: re-bind the weapon to this character. Everything that
    // gates on the curse reads this field, so it goes first.
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.cursed_weapon_equipped_id = item_id;
    }
    // `cursedOnLogin`: doTransform + giveSkill.
    do_transform(world, object_id, item_id);
    give_skill(world, idx, object_id);

    // "$s2's owner has logged into the $s1 region." to everyone. The region
    // SysString renders blank — the same TODO(G28) the drop/appear announces
    // carry (MapRegion has no sysstring id yet).
    let announce = server_packets::system_message_with(
        sm_ids::S2_S_OWNER_HAS_LOGGED_INTO_THE_S1_REGION,
        &[SmParam::SysString(0), SmParam::ItemName(item_id)],
    );
    broadcast_to_all(world, &announce);

    // "$s1 has $s2 minute(s) of usage time remaining." to the wielder alone.
    let minutes = (world.cursed_weapons[idx].time_left(now_millis()) / MILLIS_PER_MINUTE) as i32;
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::system_message_with(
            sm_ids::S1_HAS_S2_MINUTE_S_OF_USAGE_TIME_REMAINING,
            &[SmParam::ItemName(item_id), SmParam::Int(minutes)],
        ));
    }
}

/// `CursedWeapon.doTransform` — Zariche (8190) becomes transform 301, Akamanah
/// (8689) transform 302. Java stops an existing transform and re-transforms
/// 500 ms later (the client needs the two model swaps separated); the state
/// swap here is synchronous, so the revert runs inline and the apply's own
/// delayed visual refresh carries the new model.
pub(crate) fn do_transform(world: &mut World, target: i32, item_id: i32) {
    let transform_id = if item_id == 8689 { 302 } else { 301 };
    if world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.transform_id != 0)
    {
        super::admin::transforms::remove_transform(world, target);
    }
    super::admin::transforms::apply_transform(world, target, transform_id);
}

/// `CursedWeapon.giveSkill` — the weapon's own skill at Java's
/// `1 + kills/stageKills` (clamped to the skill's max level), then a refreshed
/// skill list. Java additionally adds Void Burst / Void Flow as *transform*
/// skills; on this dist the 301/302 transform templates already list both
/// (3630/3631), so [`do_transform`] grants them.
///
/// Written against `nb_kills` rather than `CursedWeapon::level()` on purpose:
/// `level()` returns 0 until `is_activated` is set, and `activate` grants the
/// skill before flipping that flag.
pub(crate) fn give_skill(world: &mut World, idx: usize, target: i32) {
    let (skill_id, level) = {
        let cw = &world.cursed_weapons[idx];
        (
            cw.skill_id,
            (1 + cw.nb_kills / cw.stage_kills.max(1)).min(cw.skill_max_level.max(1)),
        )
    };
    if world.data.skill_data.get(skill_id, level).is_some()
        && let Some(book) = world.objects.get_component_mut::<SkillBook>(&target)
    {
        book.0.insert(skill_id, level);
    }
    super::admin::refresh_skill_list(world, target);
}

/// `EnterWorld`'s "Remove demonic weapon if character is not cursed weapon
/// equipped": a Zariche/Akamanah sitting in the bag of someone the manager does
/// *not* consider cursed is a leftover (its life ended while they were offline)
/// and is destroyed on sight. Java names the two ids inline; iterating the
/// config is the same set on this dist and stays right if it ever changes.
fn destroy_stray_cursed_items(world: &mut World, client_id: u32, object_id: i32) {
    let item_ids: Vec<i32> = world.cursed_weapons.iter().map(|cw| cw.item_id).collect();
    let mut removed = false;
    for item_id in item_ids {
        let Some(item_oid) = world
            .objects
            .get_component::<Inventory>(&object_id)
            .and_then(|inv| inv.items().iter().find(|i| i.item_id == item_id))
            .map(|i| i.object_id)
        else {
            continue;
        };
        if let Some(inv) = world.objects.get_component_mut::<Inventory>(&object_id) {
            if inv.paperdoll_slot_of(item_oid).is_some() {
                inv.unequip_item(item_oid);
            }
            inv.remove_item(item_id, 1);
            removed = true;
        }
    }
    if !removed {
        return;
    }
    // Java's `destroyItem(…, sendMessage = true)` refreshes the client's bag;
    // the weight/adena footers ride along (see `helpers::send_inventory_update`).
    if let Some(inv) = world.objects.get_component::<Inventory>(&object_id) {
        let list = crate::network::enter_world::item_list(inv, &world.data, false);
        let adena = crate::network::enter_world::ex_adena_inven_count(inv);
        let weight =
            crate::network::enter_world::ex_user_info_inven_weight(object_id, inv, &world.data);
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(list);
            cs.send(adena);
            cs.send(weight);
        }
    }
    super::party::broadcast_user_info(world, object_id);
}

/// Arm the expiry timer at the weapon's current `end_time` (the wielder's
/// duration, or an un-grabbed drop's deadline). A later re-arm (a drop that's
/// then picked up) supersedes the earlier one via the `end_time` guard in
/// [`handle_expiry`].
pub(crate) fn arm_expiry(world: &mut World, idx: usize) {
    let (item_id, end_time) = {
        let cw = &world.cursed_weapons[idx];
        (cw.item_id, cw.end_time)
    };
    let delay_ticks = ((end_time - now_millis()).max(0) / 1000) as u64 * TICKS_PER_SECOND;
    world.scheduler.schedule(
        world.tick + delay_ticks,
        ScheduledTask::CursedWeaponExpiry { item_id },
    );
}

/// `CursedWeapon.RemoveTask.run`: the expiry timer fired — end-of-life the
/// weapon if its `end_time` really has passed. A stale timer (a drop that was
/// picked up and re-armed, or an already-gone weapon) no-ops. A dropped weapon
/// vanishes from the ground; an activated one is stripped from its wielder.
pub(crate) fn handle_expiry(world: &mut World, item_id: i32) {
    let Some(idx) = idx_by_item(world, item_id) else {
        return;
    };
    let (active, dropped, end_time, ground_oid) = {
        let cw = &world.cursed_weapons[idx];
        (
            cw.is_active(),
            cw.is_dropped,
            cw.end_time,
            cw.dropped_item_oid,
        )
    };
    if !active || now_millis() < end_time {
        return; // already gone, or a superseded (re-armed) timer
    }
    if dropped {
        // Despawn the un-grabbed ground item; `end_of_life` then announces +
        // clears the DB row + resets state (its non-activated branch).
        if let Some(region) = world
            .objects
            .get_component::<RegionCell>(&ground_oid)
            .map(|r| r.0)
        {
            despawn_ground_item(world, ground_oid, region);
        }
    }
    end_of_life(world, idx);
}

/// Broadcast `pkt` to every online player (Java `Broadcast.toAllOnlinePlayers`).
fn broadcast_to_all(world: &World, pkt: &[u8]) {
    for cs in world.clients.values() {
        if let ClientSession::InGame(_) = cs {
            cs.send(pkt.to_vec());
        }
    }
}

// ---------------------------------------------------------------------------
// The client's cursed-weapon window (`RequestCursedWeaponList` /
// `RequestCursedWeaponLocation`, ex 0x2A / 0x2B — row 10)
// ---------------------------------------------------------------------------

/// `RequestCursedWeaponList` → `ExCursedWeaponList`: every cursed-weapon item
/// id the server knows, live or not (Java sends `getCursedWeaponsIds()`).
pub(crate) fn handle_request_list(world: &World, client_id: u32) {
    let ids: Vec<i32> = world.cursed_weapons.iter().map(|cw| cw.item_id).collect();
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::server_packets::ex_cursed_weapon_list(&ids));
    }
}

/// `RequestCursedWeaponLocation` → `ExCursedWeaponLocation`: where each *live*
/// weapon is — the wielder's position when it is being carried, the ground
/// item's when it has been dropped. Java skips inactive ones and **sends
/// nothing at all** when none are live; kept.
pub(crate) fn handle_request_location(world: &World, client_id: u32) {
    let entries: Vec<(i32, i32, i32, i32, i32)> = world
        .cursed_weapons
        .iter()
        // Java's explicit `if (!cw.isActive()) continue`. Mirrored for clarity;
        // the position lookup below already excludes a retired weapon, whose
        // holder ids are cleared when it leaves the world.
        .filter(|cw| cw.is_active())
        .filter_map(|cw| {
            // Java `CursedWeapon.getWorldPosition()`: the player's position
            // while wielded, the dropped item's while on the ground.
            let holder = if cw.is_activated {
                cw.player_id
            } else {
                cw.dropped_item_oid
            };
            let pos = world
                .objects
                .get_component::<crate::model::components::Position>(&holder)?;
            Some((cw.item_id, i32::from(cw.is_activated), pos.x, pos.y, pos.z))
        })
        .collect();
    if entries.is_empty() {
        return;
    }
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::server_packets::ex_cursed_weapon_location(
            &entries,
        ));
    }
}
