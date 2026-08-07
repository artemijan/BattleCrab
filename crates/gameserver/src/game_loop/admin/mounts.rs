//! `AdminRide` mount commands — `//ride_strider`/`//ride_wolf`/`//ride_wyvern`
//! and `//unride`. Java's `//ride_horse`/`//ride_bike` are *transformations*
//! (not mounts) and stay on the deferred transform subsystem, as does
//! `AdminTransform`.
//!
//! A mount is durable state on the `Player` (`mount_type` + `mount_npc_id` +
//! `mount_level`) that the UserInfo/CharInfo builders serialize, so it renders
//! on every client that later sees the rider. Mounting swaps the rider's
//! speeds to the mount's `speed_on_ride` row (`recalculate_stats`) and its
//! collision cylinder to the mount NPC template's (Java
//! `Player.getCollisionRadius/Height` read the mount template while mounted);
//! a wyvern (`mount_type == 2`) additionally flies — `Player::is_flying`
//! feeds the movement pipeline's geodata exemptions and the packet fly
//! fields.

use crate::game_loop::guard;
use crate::model::Player;
use crate::model::components::{Collision, Position};
use crate::model::inventory::{Inventory, PaperdollSlot};
use crate::model::skill::OperateType;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::world::World;

use super::{send_message, send_sm};

/// The fixed npc ids `AdminRide` mounts (Java `petRideId`), with their
/// `MountType` ordinal (1 strider, 2 wyvern, 3 wolf).
pub(super) enum Mount {
    Strider,
    Wolf,
    Wyvern,
}

impl Mount {
    fn npc_id(&self) -> i32 {
        match self {
            Mount::Strider => 12526,
            Mount::Wolf => 16041,
            Mount::Wyvern => 12621,
        }
    }

    fn mount_type(&self) -> u8 {
        match self {
            Mount::Strider => 1,
            Mount::Wyvern => 2,
            Mount::Wolf => 3,
        }
    }
}

/// Java `AdminRide.getRideTarget` — the current target if it's a *different*
/// player, else the GM.
fn ride_target(world: &World, object_id: i32) -> i32 {
    guard::target(world, object_id)
        .filter(|&oid| oid != object_id && world.objects.has_component::<Player>(&oid))
        .unwrap_or(object_id)
}

/// `AdminRide`'s `//ride_strider|ride_wolf|ride_wyvern` — mount the ride target
/// on the fixed creature. Refused if already mounted or with a summon out.
pub(super) fn admin_ride(world: &mut World, client_id: u32, object_id: i32, mount: Mount) {
    let target = ride_target(world, object_id);
    if has_mount_or_summon(world, target) {
        send_message(world, client_id, "Target already have a summon.");
        return;
    }
    mount_player(world, target, mount.npc_id(), mount.mount_type());
}

/// Java `AdminRide`'s shared refusal gate — `player.isMounted() ||
/// player.hasSummon()` runs before *every* `//ride_*` branch, including the
/// transform-based horse/bike rides, so a strider rider can't stack a horse on
/// top. `hasSummon()` is either kind of summon: servitor or pet.
pub(super) fn has_mount_or_summon(world: &World, target: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(Player::is_mounted)
        || crate::game_loop::servitor::servitor_of(world, target).is_some()
        || crate::game_loop::servitor::pet_of(world, target).is_some()
}

/// Java `Player.mount(npcId, controlItemObjId, useFood)` +
/// `setMount(npcId, getLevel())`: disarm both hands and stop toggles, then
/// set the mount fields (mount level = the *rider's* level on this path),
/// swap collision and speeds to the mount's, and broadcast `Ride` +
/// UserInfo/CharInfo. Returns whether the mount happened (Java returns false
/// when the weapon can't be removed or the rider is transformed).
///
/// Java also starts the feed clock here ([`start_feed`]) — the gauge that
/// drains every 10 s and force-dismounts at zero.
pub(crate) fn mount_player(world: &mut World, target: i32, npc_id: i32, mount_type: u8) -> bool {
    // Java's first gate: `if (!ALLOW_MOUNTS_DURING_SIEGE && isInsideZone(SIEGE))
    // return false;` — silent, no message. **False** on this dist, so a rider
    // standing in a live siege zone simply cannot mount.
    if !world.cfg.feature.allow_ride_mounts_during_siege && in_active_siege(world, target) {
        return false;
    }
    // Java: `if (!disarmWeapons() || !disarmShield() || isTransformed())
    // return false;` — then `getEffectList().stopAllToggles()`. The disarm is
    // load-bearing for the client, not cosmetic: a mounted paperdoll that
    // still carries a weapon is a state retail never produces, and the client
    // renders it as a ghostly, non-animated mount.
    if world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.transform_id != 0)
    {
        return false;
    }
    if !disarm_hands(world, target) {
        return false;
    }
    stop_all_toggles(world, target);
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.mount_type = mount_type;
        p.mount_npc_id = npc_id;
        p.mount_level = p.level;
    }
    // Java `Player.getCollisionRadius/Height`: while mounted, the mount NPC
    // template's cylinder replaces the class template's.
    if let Some(t) = world.data.npc_data.get(npc_id) {
        let (radius, height) = (t.collision_radius, t.collision_height);
        world
            .objects
            .add_components(&target, Collision { radius, height });
    }
    // `startFeed(pet.getId())` runs *before* Java unsummons the pet, so a live
    // pet's own food carries onto the mount's gauge; with no pet out (the
    // admin rides, the wyvern manager) the bar starts full.
    let inherited = crate::game_loop::servitor::pet_of(world, target)
        .and_then(|pet| {
            world
                .objects
                .get_component::<crate::model::components::PetOf>(&pet)
        })
        .map(|p| p.fed);
    start_feed(world, target, inherited);
    super::transforms::recompute_speeds(world, target);
    broadcast_ride(world, target, true);
    super::party::broadcast_user_info(world, target);
    // The visual list has to follow *after* the client has rebuilt the actor
    // around the mount model, or it is dropped with the old one — Java's
    // `updateAbnormalVisualEffects` schedules it 50 ms out for the same reason.
    crate::game_loop::abnormal::schedule_visual_refresh(world, target);
    true
}

/// Java `Player.disarmWeapons()` + `disarmShield()` — unequip both hands
/// before the `Ride` goes out, with the same client traffic as a manual
/// unequip (InventoryUpdate/UserInfo/equip-slot via `finish_equip_change`)
/// plus Java's per-item system message. Returns false when the weapon can't
/// be removed: a cursed weapon refuses the whole mount (Java also refuses on
/// an equipped Combat Flag and on force-equip weapons — neither state exists
/// in the port yet). Java additionally calls `abortAttack()` here; the mount
/// paths can't currently be reached mid-swing, so that leg is skipped.
fn disarm_hands(world: &mut World, target: i32) -> bool {
    if world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.cursed_weapon_equipped_id != 0)
    {
        return false;
    }
    let client_id = super::helpers::client_for_player(world, target).unwrap_or(0);
    for slot in [PaperdollSlot::RHand, PaperdollSlot::LHand] {
        let Some((item_object_id, item_id, enchant)) = world
            .objects
            .get_component::<Inventory>(&target)
            .and_then(|inv| {
                let oid = inv.paperdoll_object_id(slot);
                (oid != 0).then(|| {
                    (
                        oid,
                        inv.paperdoll_item_id(slot),
                        inv.paperdoll_enchant_level(slot),
                    )
                })
            })
        else {
            continue;
        };
        let changed = world
            .objects
            .get_component_mut::<Inventory>(&target)
            .map(|inv| inv.unequip_item(item_object_id))
            .unwrap_or_default();
        crate::game_loop::items::finish_equip_change(world, client_id, target, &changed);
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(if enchant > 0 {
                server_packets::system_message_with(
                    sm_ids::THE_EQUIPMENT_S1_S2_HAS_BEEN_REMOVED,
                    &[SmParam::Int(enchant), SmParam::ItemName(item_id)],
                )
            } else {
                server_packets::system_message_with(
                    sm_ids::S1_HAS_BEEN_UNEQUIPPED,
                    &[SmParam::ItemName(item_id)],
                )
            });
        }
    }
    true
}

/// Java `EffectList.stopAllToggles()` — every live toggle drops on mount
/// (mounted players can't keep toggles up; `Player.useMagic` blocks
/// re-lighting them while mounted in Java).
fn stop_all_toggles(world: &mut World, target: i32) {
    // Level 1 (not the buff's own level) is what the original lookup used, and
    // `operate_type` does not vary by level.
    crate::game_loop::skills::effects::expire_buffs_where(world, target, |world, buff| {
        world
            .data
            .skill_data
            .get(buff.skill_id, 1)
            .is_some_and(|s| s.operate_type == OperateType::Toggle)
    });
}

/// Java `Player.dismount()` — refuse mid-air/over-water dismounts, then clear
/// the mount, restore the class collision/speeds, and broadcast. No-op if not
/// mounted. The `//unride*` commands route here through the transform module's
/// combined dismount-or-untransform path.
///
/// Java also clears the feed gauge and stops its task here; the port's task
/// self-cancels on the next tick once `is_mounted()` is false, so only the
/// gauge needs sending. The leftover feed is written back onto the ridden
/// pet's collar row first (`storePetFood` — memory-first: the `PlayerPets`
/// row carries it to the next save/summon). Java's `removeSkill(WYVERN_BREATH
/// 4289)` here is dead even in Java: its own `setMount` grants only the
/// noble's Strider Siege Assault, never 4289, so there is nothing to remove.
pub(crate) fn dismount(world: &mut World, target: i32) {
    if !world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(Player::is_mounted)
    {
        return;
    }
    let Some(pos) = world.objects.get_component::<Position>(&target).copied() else {
        return;
    };
    // Java: with no water 300 below, a dismount hanging in the sky (z > 10000
    // outside water) or more than 300 above the geodata floor is refused —
    // land the wyvern first.
    let water_below = world
        .data
        .zone_data
        .zones_at(pos.x, pos.y, pos.z - 300)
        .any(|z| z.kind == crate::data::zone_data::ZoneKind::Water);
    if !water_below {
        // Java's guard here is `isInWater()` — the *drowning task*, not the
        // zone flag (`Player.isInWater()` returns `_taskWater != null`). They
        // usually agree, but with `AllowWater = False` the task never starts,
        // and Java then refuses a high-altitude dismount even in open sea.
        let swimming = crate::game_loop::water::is_drowning_task_active(world, target);
        let client = super::helpers::client_for_player(world, target);
        if !swimming && pos.z > 10000 {
            if let Some(cid) = client {
                send_sm(
                    world,
                    cid,
                    sm_ids::YOU_ARE_NOT_ALLOWED_TO_DISMOUNT_IN_THIS_LOCATION,
                );
            }
            return;
        }
        if world.geo.get_height(pos.x, pos.y, pos.z) + 300 < pos.z {
            if let Some(cid) = client {
                send_sm(world, cid, sm_ids::YOU_CANNOT_DISMOUNT_FROM_THIS_ELEVATION);
            }
            return;
        }
    } else {
        // Dismounting *into* water: Java re-broadcasts `UserInfo` 1.5 s later
        // if the rider is by then actually swimming. The immediate broadcast at
        // the end of this function still carries the dismounted-but-dry speeds
        // — the fall into the water finishes after it, so without the delayed
        // resend the client keeps running speed while submerged.
        world.scheduler.schedule(
            world.tick + 15,
            crate::scheduler::ScheduledTask::DismountWaterUserInfo { object_id: target },
        );
    }
    // `storePetFood(_mountNpcId)` — the drained gauge goes back onto the
    // ridden pet's collar row before the mount fields clear (memory-first:
    // the `PlayerPets` row is what the save and the next summon read).
    let (collar, feed) = world
        .objects
        .get_component::<Player>(&target)
        .map_or((0, 0), |p| (p.mount_collar_object_id, p.mount_feed));
    if collar != 0
        && let Some(pets) = world
            .objects
            .get_component_mut::<crate::model::components::PlayerPets>(&target)
        && let Some(row) = pets.0.get_mut(&collar)
    {
        row.fed = feed;
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.mount_type = 0;
        p.mount_npc_id = 0;
        p.mount_level = 0;
        p.mount_feed = 0;
        p.mount_collar_object_id = 0;
    }
    // `stopFeed()` + `SetupGauge(3, 0, 0)`: blank the bar. Sent by hand rather
    // than through `set_current_feed`, which needs the (now cleared) mount to
    // resolve its level row.
    if let Some(cs) =
        super::helpers::client_for_player(world, target).and_then(|cid| world.clients.get(&cid))
    {
        cs.send(server_packets::setup_gauge_range(target, GAUGE_GREEN, 0, 0));
    }
    // Restore the class-template collision (same shape as untransform).
    let (class_id, base_class_id) = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| (p.class_id, p.base_class_id))
        .unwrap_or((0, 0));
    if let Some(t) = world
        .data
        .player_templates
        .get(class_id)
        .or_else(|| world.data.player_templates.get(base_class_id))
    {
        world.objects.add_components(
            &target,
            Collision {
                radius: t.collision_radius,
                height: t.collision_height,
            },
        );
    }
    super::transforms::recompute_speeds(world, target);
    broadcast_ride(world, target, false);
    super::party::broadcast_user_info(world, target);
    // Same on the way down, and this leg is a *fix*, not a port: Java's
    // `dismount()` sends `Ride` + `broadcastUserInfo()` and never refreshes the
    // visuals, so a GM who dismounts stays invisible with no STEALTH glow and
    // any other abnormal visual silently missing from their own view.
    crate::game_loop::abnormal::schedule_visual_refresh(world, target);
}

/// Broadcast the `Ride` packet (mount/dismount) to the rider and everyone
/// nearby.
fn broadcast_ride(world: &World, target: i32, mounted: bool) {
    let (Some(p), Some(pos)) = (
        world.objects.get_component::<Player>(&target),
        world.objects.get_component::<Position>(&target).copied(),
    ) else {
        return;
    };
    let packet = server_packets::ride(
        target,
        mounted,
        p.mount_type,
        p.mount_npc_id,
        pos.x,
        pos.y,
        pos.z,
    );
    super::helpers::broadcast_including_self(world, target, &packet);
}

/// Java `isInsideZone(ZoneId.SIEGE)` for a player — the zone flag the
/// `SiegeZone` sets while its castle's siege is running (`ZoneFlags`, kept
/// separately from the plain membership mask because a siege zone is only a
/// combat zone while active).
pub(crate) fn in_active_siege(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::ZoneFlags>(&object_id)
        .is_some_and(|f| f.in_active_siege)
}

// ---------------------------------------------------------------------------
// Mount feeding (Java `Player.startFeed`/`stopFeed` + `PetFeedTask`)
// ---------------------------------------------------------------------------

/// Java `ThreadPool.scheduleAtFixedRate(new PetFeedTask(this), 10000, 10000)`.
const FEED_TICK_SECS: u64 = 10;
/// Game-loop ticks per second.
const TICKS_PER_SECOND: u64 = 10;
/// `SetupGauge`'s green bar — the colour the feed gauge uses.
const GAUGE_GREEN: i32 = 3;

/// Java `Player.startFeed(npcId)`: fill the gauge, show it, and start the 10 s
/// clock. `inherited` is the food a **live pet** brings to the mount — Java
/// calls `startFeed` while `hasPet()` is still true (the pet is unsummoned one
/// line later), so mounting your own half-starved strider gives you a
/// half-empty bar; every other path (admin `//ride_*`, the wyvern manager,
/// the enter-world restore) has no pet and starts full.
pub(crate) fn start_feed(world: &mut World, target: i32, inherited: Option<i32>) {
    let Some(max) = max_feed(world, target) else {
        // Java: `getPetData(npcId) == null` → the task stops itself on the
        // first tick. Nothing to drain, so don't arm it at all.
        return;
    };
    let feed = inherited.unwrap_or(max).clamp(0, max);
    set_current_feed(world, target, feed);
    world.scheduler.schedule(
        world.tick + FEED_TICK_SECS * TICKS_PER_SECOND,
        crate::scheduler::ScheduledTask::MountFeedTick { player_oid: target },
    );
}

/// Java `Player.setCurrentFeed(num)` — clamp to the maximum and re-send the
/// gauge. (Java also re-broadcasts UserInfo when `isHungry()` flips; that
/// predicate is inert here — see [`is_hungry`].)
pub(crate) fn set_current_feed(world: &mut World, target: i32, feed: i32) {
    let Some(max) = max_feed(world, target) else {
        return;
    };
    let feed = feed.min(max);
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.mount_feed = feed;
    }
    send_feed_gauge(world, target, feed, max);
}

/// The gauge, in the milliseconds-of-riding-left the client draws: each unit of
/// feed buys one 10 s tick, so `feed * 10000 / consume` ms.
fn send_feed_gauge(world: &World, target: i32, feed: i32, max: i32) {
    let consume = feed_consume(world, target).max(1);
    let Some(cs) =
        super::helpers::client_for_player(world, target).and_then(|cid| world.clients.get(&cid))
    else {
        return;
    };
    // **Java bug, not reproduced.** All four feed call sites read
    // `new SetupGauge(3, cur, max)` — the *three*-argument constructor, whose
    // first parameter is the **object id**. Mobius added `objectId` to the
    // signature and never updated these lines, so retail-Mobius sends
    // `objectId = 3, colour = cur`: garbage the client cannot draw. Every other
    // `SetupGauge` call site passes `getObjectId()` first, which is what the
    // four-argument form here does.
    cs.send(server_packets::setup_gauge_range(
        target,
        GAUGE_GREEN,
        feed * 10000 / consume,
        max * 10000 / consume,
    ));
}

/// Java `PetFeedTask.run`: burn one tick's feed, or force-dismount when the bar
/// cannot cover it. Self-cancelling — a rider who has dismounted (or whose
/// mount has no pet data) simply stops re-arming, which is Java's `stopFeed`.
pub(crate) fn handle_mount_feed_tick(world: &mut World, target: i32) {
    if !world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(Player::is_mounted)
    {
        return;
    }
    if max_feed(world, target).is_none() {
        return;
    }
    let consume = feed_consume(world, target);
    let feed = world
        .objects
        .get_component::<Player>(&target)
        .map_or(0, |p| p.mount_feed);
    if feed > consume {
        set_current_feed(world, target, feed - consume);
    } else {
        // "You are out of feed. Mount status canceled."
        set_current_feed(world, target, 0);
        dismount(world, target);
        send_sm(
            world,
            super::helpers::client_for_player(world, target).unwrap_or(0),
            sm_ids::YOU_ARE_OUT_OF_FEED_MOUNT_STATUS_CANCELED,
        );
        return;
    }
    world.scheduler.schedule(
        world.tick + FEED_TICK_SECS * TICKS_PER_SECOND,
        crate::scheduler::ScheduledTask::MountFeedTick { player_oid: target },
    );
}

/// Java `Player.getFeedConsume()` — the battle rate while swinging, else the
/// normal one, from the mount's level row.
fn feed_consume(world: &World, target: i32) -> i32 {
    let attacking = world
        .objects
        .get_component::<crate::model::components::AttackState>(&target)
        .is_some_and(|a| a.attack_end_tick > world.tick);
    mount_level_row(world, target).map_or(1, |row| {
        if attacking {
            row.consume_meal_in_battle
        } else {
            row.consume_meal_in_normal
        }
    })
}

/// Java `Player.getMaxFeed()` — the level row's `max_meal`. `None` when the
/// ridden species has no pet data (Java's `getPetData(...) == null` guard).
fn max_feed(world: &World, target: i32) -> Option<i32> {
    mount_level_row(world, target).map(|row| row.max_meal)
}

/// The mount's `<level>` row, at the *mount's* level (Java `getPetLevelData`).
fn mount_level_row(world: &World, target: i32) -> Option<&crate::data::pet_data::PetLevel> {
    let p = world.objects.get_component::<Player>(&target)?;
    world
        .data
        .pet_data
        .get(p.mount_npc_id)
        .and_then(|pet| pet.level_row(p.mount_level))
}

/// Java `Player.isHungry()` — **inert, and faithfully so**: the predicate reads
/// `hasPet() && _canFeed && _curFeed < hungryLimit% * maxFeed`, but `mount()`
/// unsummons the pet immediately after starting the feed, so a rider never has
/// one. Both consumers — the `SpeedFinalizer` halving and the "a hungry strider
/// cannot be dismounted" refusal — are therefore dead code in this Java build.
/// Kept as a named function so the two call sites can point at the reason
/// rather than silently omitting a branch.
pub(crate) fn is_hungry(world: &World, target: i32) -> bool {
    if crate::game_loop::servitor::pet_of(world, target).is_none() {
        return false;
    }
    let Some(limit) = world
        .objects
        .get_component::<Player>(&target)
        .and_then(|p| world.data.pet_data.get(p.mount_npc_id))
        .map(|t| t.hungry_limit)
    else {
        return false;
    };
    let (Some(feed), Some(max)) = (
        world
            .objects
            .get_component::<Player>(&target)
            .map(|p| p.mount_feed),
        max_feed(world, target),
    ) else {
        return false;
    };
    (feed as f64) < (limit as f64 / 100.0) * max as f64
}
