//! PvP flagging — the ported subset of `Player.updatePvPStatus` /
//! `updatePvPFlag` / `isAutoAttackable` plus the `PvpFlagTaskManager` sweep.
//!
//! Systems that don't exist yet (clans, parties, duels, olympiad, sieges,
//! faction, dark-side) are dropped from the ported checks, so the relations
//! reduce to karma (`Player.reputation`) + the runtime flag. PVP-zone (arena)
//! exemptions land with Phase 2, when those zones are loaded.

use crate::model::components::{PvpState, ZoneFlags};
use crate::model::Player;
use crate::network::server_packets;
use crate::world::World;

use super::helpers::broadcast_including_self;

/// `Config.PVP_NORMAL_TIME` (PvPVsNormalTime, 120 s) in 100 ms ticks — how long
/// the flag lasts after a hostile action toward a *clean* target.
const PVP_NORMAL_TICKS: u64 = 1200;
/// `Config.PVP_PVP_TIME` (PvPVsPvPTime, 60 s) — the (shorter) flag when the
/// target is already a PK or flagged (`checkIfPvP`).
const PVP_PVP_TICKS: u64 = 600;
/// The flag blinks (value 2) over its final 20 s (`PvpFlagTaskManager`).
const PVP_BLINK_TICKS: u64 = 200;

/// Java `Player.reputation < 0` ⇒ the player is a PK (red name).
fn is_pk(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&oid)
        .is_some_and(|p| p.reputation < 0)
}

fn flag_of(world: &World, oid: i32) -> u8 {
    world
        .objects
        .get_component::<PvpState>(&oid)
        .map_or(0, |s| s.flag)
}

fn in_peace(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<ZoneFlags>(&oid)
        .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Peace))
}

/// In an `ArenaZone` (`ZoneId.PVP`): free-for-all, and hostile actions there
/// don't raise a flag.
fn in_pvp_zone(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<ZoneFlags>(&oid)
        .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Pvp))
}

/// Java `Playable.checkIfPvP(target)` reduced to the ported state: the target
/// is already "in PvP" (a PK or currently flagged), which shortens the
/// attacker's own flag to `PVP_PVP_TIME`. Party/clan-war/dark-side branches
/// aren't modeled.
fn check_if_pvp(world: &World, self_oid: i32, target_oid: i32) -> bool {
    self_oid != target_oid && (is_pk(world, target_oid) || flag_of(world, target_oid) > 0)
}

/// Java `Player.isAutoAttackable(attacker)` narrowed to the ported systems: a
/// target player can be freely attacked (no Ctrl needed) when it's a PK or
/// already flagged; monsters attack anyone; a target in a peace zone can't be
/// attacked. Party/clan/duel/oly/siege/faction and the PVP-zone (arena)
/// exemption are not modeled yet.
pub(crate) fn is_player_auto_attackable(world: &World, attacker_oid: i32, target_oid: i32) -> bool {
    if attacker_oid == target_oid {
        return false;
    }
    // Monster attacker → always auto-attackable.
    if super::combat::is_npc_oid(attacker_oid) {
        return true;
    }
    // A target standing in a peace zone is never auto-attackable.
    if in_peace(world, target_oid) {
        return false;
    }
    // Arena: both in a PVP zone → freely attackable (Java's `isInsideZone(PVP)`
    // pair check, minus the siege carve-out).
    if in_pvp_zone(world, attacker_oid) && in_pvp_zone(world, target_oid) {
        return true;
    }
    is_pk(world, target_oid) || flag_of(world, target_oid) > 0
}

/// Java `Player.updatePvPStatus()` (no target): self-flag from a "supporting"
/// action (buffing a monster or a flagged/PK player). No-op inside a PVP zone.
pub(crate) fn update_pvp_status(world: &mut World, object_id: i32) {
    // `if (isInsideZone(ZoneId.PVP)) return;` — no flag inside an arena.
    if in_pvp_zone(world, object_id) {
        return;
    }
    set_flag_lasts(world, object_id, PVP_NORMAL_TICKS);
}

/// Java `Player.updatePvPStatus(Creature target)`: flag the actor for a hostile
/// action toward a player `target`. Duration is `PVP_PVP_TIME` when the target
/// is already in PvP (`checkIfPvP`), else `PVP_NORMAL_TIME`.
pub(crate) fn update_pvp_status_target(world: &mut World, object_id: i32, target_oid: i32) {
    // The target must resolve to a player, and can't be the actor itself.
    if object_id == target_oid || !world.objects.has_component::<Player>(&target_oid) {
        return;
    }
    // `(!isInsideZone(PVP) || !target.isInsideZone(PVP)) && target.reputation >= 0`:
    // no flag when both stand in an arena, and no flag for attacking a PK
    // (a PK is freely attackable).
    if in_pvp_zone(world, object_id) && in_pvp_zone(world, target_oid) {
        return;
    }
    if is_pk(world, target_oid) {
        return;
    }
    let ticks = if check_if_pvp(world, object_id, target_oid) {
        PVP_PVP_TICKS
    } else {
        PVP_NORMAL_TICKS
    };
    set_flag_lasts(world, object_id, ticks);
}

/// Refresh the flag expiry to `now + ticks` and turn the flag on if it was off
/// (`setPvpFlagLasts` + `if (_pvpFlag == 0) startPvPFlag()`).
fn set_flag_lasts(world: &mut World, object_id: i32, ticks: u64) {
    let expires = world.tick + ticks;
    let Some(st) = world.objects.get_component_mut::<PvpState>(&object_id) else {
        return;
    };
    st.expires_tick = expires;
    if st.flag == 0 {
        update_pvp_flag(world, object_id, 1);
    }
}

/// Java `Player.updatePvPFlag(value)`: set the flag byte (if changed) and
/// broadcast the new state — `StatusUpdate(PVP_FLAG)` to self + nearby, and a
/// `RelationChanged` to nearby players so the name recolors. The per-viewer
/// `RelationCache` de-dup is skipped (flag changes are rare: on, 1→2, →0).
pub(crate) fn update_pvp_flag(world: &mut World, object_id: i32, value: u8) {
    {
        let Some(st) = world.objects.get_component_mut::<PvpState>(&object_id) else {
            return;
        };
        if st.flag == value {
            return;
        }
        st.flag = value;
    }
    let reputation = world
        .objects
        .get_component::<Player>(&object_id)
        .map_or(0, |p| p.reputation);
    // StatusUpdate(PVP_FLAG) → self + everyone nearby.
    broadcast_including_self(
        world,
        object_id,
        &server_packets::status_update(
            object_id,
            &[(server_packets::status_update_type::PVP_FLAG, value as i32)],
        ),
    );
    // RelationChanged → nearby players. `relation` is 0 (no party/clan), and
    // `auto_attackable` is the viewer-independent core (flagged or PK).
    let auto_attackable = value > 0 || reputation < 0;
    super::helpers::broadcast_to_others(
        world,
        object_id,
        &server_packets::relation_changed(object_id, 0, auto_attackable, reputation, value),
    );
}

/// Java `PvpFlagTaskManager.run` (1 s cadence): expire flags whose time ran
/// out, and blink (value 2) those in their final 20 s. Presence-filtered to
/// players carrying a live flag.
pub(crate) fn pvp_flag_tick(world: &mut World) {
    let now = world.tick;
    let mut ended: Vec<i32> = Vec::new();
    let mut blink: Vec<i32> = Vec::new();
    let mut solid: Vec<i32> = Vec::new();
    world
        .objects
        .for_each_mut::<(&Player, &PvpState)>(|(p, st)| {
            if st.flag == 0 {
                return;
            }
            if now >= st.expires_tick {
                ended.push(p.object_id);
            } else if now >= st.expires_tick.saturating_sub(PVP_BLINK_TICKS) {
                if st.flag != 2 {
                    blink.push(p.object_id);
                }
            } else if st.flag != 1 {
                solid.push(p.object_id);
            }
        });
    for oid in ended {
        // stopPvPFlag → updatePvPFlag(0). The expiry stays; flag == 0 is the
        // "not flagged" state.
        update_pvp_flag(world, oid, 0);
    }
    for oid in blink {
        update_pvp_flag(world, oid, 2);
    }
    for oid in solid {
        update_pvp_flag(world, oid, 1);
    }
}
