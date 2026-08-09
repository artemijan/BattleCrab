//! Zone revalidation — the port of `Creature.revalidateZone(force)` +
//! `ZoneRegion.revalidateZones` + the `Player.revalidateZone` override's
//! compass-code push, driven by the `ZoneData` point queries instead of
//! per-zone character lists (each zone type's enter/exit effect is a diff
//! branch here; with three zone kinds and one consumer each, materializing
//! Java's `ZoneType._characterList` buys nothing).
//!
//! Call sites mirror Java: every movement tick (self-filtered by the
//! 100-unit `last_validate` distance), enter world / teleport / position
//! snaps with `force = true`.

use crate::game_loop::guard::position;
use crate::model::components::{Speeds, ZoneFlags};
use crate::network::server_packets::{self, compass_zone};
use crate::world::World;

use super::helpers::client_for_player;

/// Java `Creature.revalidateZone(force)` for a player. Recomputes the
/// membership mask at the current position and applies the enter/exit
/// effects of every bit that flipped.
pub(crate) fn revalidate_zone(world: &mut World, object_id: i32, force: bool) {
    let Some(pos) = position(world, object_id) else {
        return;
    };
    let Some(flags) = world
        .objects
        .get_component::<ZoneFlags>(&object_id)
        .copied()
    else {
        return;
    };

    // "This function is called too often from movement code."
    if !force {
        let (lx, ly, lz) = flags.last_validate;
        // f64: the fresh-player sentinel (`i32::MIN`) would overflow any
        // integer square.
        let (dx, dy, dz) = (
            pos.x as f64 - lx as f64,
            pos.y as f64 - ly as f64,
            pos.z as f64 - lz as f64,
        );
        if dx * dx + dy * dy + dz * dz < 100.0 * 100.0 {
            return;
        }
    }

    let new_mask = world.data.zone_data.mask_at(pos.x, pos.y, pos.z);

    // Compass indicator (`Player.revalidateZone`'s tail): peace icon vs
    // general, only pushed when the code changes. Siege/PvP/altered codes
    // wait for their zone types.
    let compass = if new_mask & crate::data::zone_data::ZoneKind::Peace.bit() != 0 {
        compass_zone::PEACE
    } else {
        compass_zone::GENERAL
    };
    let compass_changed = compass != flags.last_compass;

    let old_mask = flags.mask;
    if let Some(f) = world.objects.get_component_mut::<ZoneFlags>(&object_id) {
        f.mask = new_mask;
        f.last_validate = (pos.x, pos.y, pos.z);
        f.last_compass = compass;
    }

    if compass_changed
        && let Some(cs) =
            client_for_player(world, object_id).and_then(|cid| world.clients.get(&cid))
    {
        cs.send(server_packets::ex_set_compass_zone_code(compass));
    }

    // `SiegeZone.onEnter` → `startFameTask` for a registered participant. The
    // exit half is not here: the task checks `is_in_siege` when it fires and
    // stops re-arming itself, which also covers the cases that are not a zone
    // exit at all — the siege ending, or the clan unregistering under them.
    let siege_bit = crate::data::zone_data::ZoneKind::Siege.bit();
    if (old_mask ^ new_mask) & siege_bit != 0
        && new_mask & siege_bit != 0
        && crate::game_loop::pvp::is_in_siege(world, object_id)
    {
        crate::game_loop::siege::arm_fame_task(world, object_id);
    }

    // WaterZone.onEnter/onExit: flip the swim-speed branch and let everyone
    // (self included) re-read the speeds (Java `broadcastUserInfo`).
    let water_bit = crate::data::zone_data::ZoneKind::Water.bit();
    if (old_mask ^ new_mask) & water_bit != 0 {
        let swimming = new_mask & water_bit != 0;
        if let Some(speeds) = world.objects.get_component_mut::<Speeds>(&object_id) {
            speeds.swimming = swimming;
        }
        if swimming {
            // `onEnter`: a transform that can't swim is cancelled instead of
            // being rebroadcast — `stopTransformation` sends its own UserInfo,
            // which is why Java's `else` skips the broadcast on that branch.
            let cant_swim = world
                .objects
                .get_component::<crate::model::Player>(&object_id)
                .map(|p| p.transform_id)
                .filter(|&id| id != 0)
                .is_some_and(|id| world.data.transforms.get(id).is_some_and(|tf| !tf.can_swim));
            if cant_swim {
                super::admin::transforms::remove_transform(world, object_id);
            } else {
                super::party::broadcast_user_info(world, object_id);
            }
        } else {
            // `onExit`: Java skips the broadcast mid-teleport (the arrival
            // sends a full UserInfo anyway).
            if !world
                .objects
                .get_component::<crate::model::Player>(&object_id)
                .is_some_and(|p| p.teleporting)
            {
                super::party::broadcast_user_info(world, object_id);
            }
        }
    }

    // `Player.revalidateZone`'s tail: `if (Config.ALLOW_WATER) checkWaterState()`.
    // Note this is *not* folded into the transition branch above — Java
    // re-checks on every revalidate, so a player who enters the world already
    // submerged (or is teleported in) starts drowning without ever crossing an
    // edge.
    if world.cfg.general.allow_water {
        super::water::check_water_state(world, object_id);
    }

    // SwampZone.onEnter/onExit: refresh the cached move-speed multiplier and,
    // when it actually changed, recompute the speeds and rebroadcast UserInfo
    // (Java's `broadcastUserInfo()` on both edges). The mask bit answers the
    // common "not in any swamp" case without the per-zone walk.
    let swamp = if new_mask & crate::data::zone_data::ZoneKind::Swamp.bit() != 0 {
        super::effect_zones::swamp_multiplier_at(world, object_id)
    } else {
        1.0
    };
    let swamp_changed = world
        .objects
        .get_component::<Speeds>(&object_id)
        .is_some_and(|s| s.swamp_multiplier != swamp);
    if swamp_changed {
        if let Some(speeds) = world.objects.get_component_mut::<Speeds>(&object_id) {
            speeds.swamp_multiplier = swamp;
        }
        // Speeds only — the swamp multiplier is applied inside
        // `recalculate_stats`, so a plain recompute picks it up.
        if let Some((player, base, mods, inventory, mut speeds, mut combat)) =
            world.objects.get_many_mut::<(
                &crate::model::Player,
                &crate::model::components::BaseStats,
                &crate::model::components::StatModifiers,
                &crate::model::inventory::Inventory,
                &mut Speeds,
                &mut crate::model::components::CombatStats,
            )>(&object_id)
        {
            player.recalculate_stats(&world.data, base, mods, inventory, &mut speeds, &mut combat);
        }
        super::party::broadcast_user_info(world, object_id);
    }

    // The TvT event's `onEnterZone`/`onExitZone` for the two colosseum
    // headquarters (enemy kick + the inactivity clock). Edge-triggered: the
    // hook only runs when the named zone actually changes.
    let hq = world.data.zone_data.tvt_hq_zone_at(pos.x, pos.y, pos.z);
    if hq != flags.tvt_hq_zone {
        if let Some(f) = world.objects.get_component_mut::<ZoneFlags>(&object_id) {
            f.tvt_hq_zone = hq;
        }
        super::events::tvt::on_hq_zone_change(world, object_id, flags.tvt_hq_zone, hq);
    }

    // SiegeZone.onEnter/onExit — see `refresh_siege_zone_flag`.
    refresh_siege_zone_flag(world, object_id);

    // FishingZone.onEnter/onExit → `ExAutoFishAvailable` (G32): light the
    // client's auto-fish button when the player can fish here, dim it on exit.
    let fishing_avail = super::fishing::fishing_available(world, object_id);
    if fishing_avail != flags.fishing_available {
        if let Some(f) = world.objects.get_component_mut::<ZoneFlags>(&object_id) {
            f.fishing_available = fishing_avail;
        }
        if let Some(cs) =
            client_for_player(world, object_id).and_then(|cid| world.clients.get(&cid))
        {
            cs.send(server_packets::ex_auto_fish_available(fishing_avail));
        }
    }

    // Peace/NoRestart have no enter/exit side effects — membership itself is
    // the state their consumers check (`is_inside_peace_zone`; NO_RESTART has
    // no reader in this Mobius version beyond the login-inside teleport).

    // JailZone.onExit (G31): a jailed player who has wandered out of the prison
    // is teleported straight back. Geometry-queried (jail claims no mask bit).
    super::punishment::enforce_jail_keep_in(world, object_id);
}

/// `SiegeZone.onEnter/onExit` for one player: a siege zone is a combat zone only
/// while its castle's siege runs, which the membership mask can't express — so
/// the active-siege state is tracked separately. On a change: flip the flag, show
/// the combat-zone message, rebroadcast UserInfo (the in-siege **crown** bit,
/// Java `updatePlayerSiegeStateFlags` → `updateUserInfo`) and RelationChanged
/// (the attackable siege icon), and — on exit — flag the player (`startPvPFlag`),
/// which the PvP task then blinks out.
pub(crate) fn refresh_siege_zone_flag(world: &mut World, object_id: i32) {
    let now_active_siege = super::pvp::active_siege_castle(world, object_id).is_some();
    let was = world
        .objects
        .get_component::<ZoneFlags>(&object_id)
        .is_some_and(|f| f.in_active_siege);
    if now_active_siege == was {
        return;
    }
    if let Some(f) = world.objects.get_component_mut::<ZoneFlags>(&object_id) {
        f.in_active_siege = now_active_siege;
    }
    let msg = if now_active_siege {
        server_packets::sm_ids::YOU_HAVE_ENTERED_A_COMBAT_ZONE
    } else {
        server_packets::sm_ids::YOU_HAVE_LEFT_A_COMBAT_ZONE
    };
    if let Some(cs) = client_for_player(world, object_id).and_then(|cid| world.clients.get(&cid)) {
        cs.send(server_packets::system_message_with(msg, &[]));
    }
    if now_active_siege {
        dismount_for_siege(world, object_id);
    }
    // UserInfo — the in-siege crown bit (0x80) toggles with zone presence.
    super::party::broadcast_user_info(world, object_id);
    // RelationChanged — the attackable siege icon vs everyone nearby.
    super::pvp::broadcast_siege_relation(world, object_id);
    if !now_active_siege {
        super::pvp::start_pvp_flag_on_siege_exit(world, object_id);
    }
}

/// `SiegeZone.onEnter`'s two mount legs, both gated on
/// `AllowRideMountsDuringSiege` (**False** on this dist): a mounted player is
/// **dismounted**, and one wearing a `RIDING_MODE` transformation (the
/// horse/bike rides) is **untransformed**. Silent in Java — neither sends a
/// message.
///
/// The wyvern leg above them is gated on `AllowRideWyvernDuringSiege`, True
/// here, so it never fires.
pub(crate) fn dismount_for_siege(world: &mut World, object_id: i32) {
    if world.cfg.feature.allow_ride_mounts_during_siege {
        return;
    }
    let (mounted, transform_id) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .map_or((false, 0), |p| (p.is_mounted(), p.transform_id));
    if mounted {
        super::admin::mounts::dismount(world, object_id);
    }
    if transform_id != 0
        && world
            .data
            .transforms
            .get(transform_id)
            .is_some_and(|t| t.riding)
    {
        super::admin::transforms::remove_transform(world, object_id);
    }
}

/// Java `Castle.getZone().updateZoneStatusForCharactersInside()` — on siege
/// start/end, re-run the siege-zone check for every in-game player so those
/// standing in the castle's zone gain/lose the in-siege crown + attackable icon
/// the moment the siege flips (they never crossed a zone boundary themselves).
pub(crate) fn refresh_siege_zone_for_all(world: &mut World) {
    let players: Vec<i32> = world.in_game_player_oids().collect();
    for oid in players {
        refresh_siege_zone_flag(world, oid);
    }
}

/// Java `Creature.isInsidePeaceZone(attacker, target)` narrowed to the
/// states that exist: both sides must be players (playable), no karma/
/// GM-override branches (no reputation-based PvP or access levels yet).
/// True ⇒ hostile actions between them are refused.
pub(crate) fn is_inside_peace_zone(world: &World, attacker_oid: i32, target_oid: i32) -> bool {
    let attacker_player = world
        .objects
        .has_component::<crate::model::Player>(&attacker_oid);
    let target_player = world
        .objects
        .has_component::<crate::model::Player>(&target_oid);
    if !attacker_player || !target_player {
        return false;
    }
    let in_peace = |oid: i32| {
        world
            .objects
            .get_component::<ZoneFlags>(&oid)
            .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Peace))
    };
    in_peace(attacker_oid) || in_peace(target_oid)
}
