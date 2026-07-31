//! PvP flagging — the ported subset of `Player.updatePvPStatus` /
//! `updatePvPFlag` / `isAutoAttackable` plus the `PvpFlagTaskManager` sweep.
//!
//! Systems that don't exist yet (clans, parties, duels, olympiad, sieges,
//! faction, dark-side) are dropped from the ported checks, so the relations
//! reduce to karma (`Player.reputation`) + the runtime flag. PVP-zone (arena)
//! exemptions land with Phase 2, when those zones are loaded.

use crate::model::Player;
use crate::model::components::{PvpState, RegionCell, ZoneFlags};
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::{World, regions_adjacent};

use super::helpers::{broadcast_including_self, client_for_player};

/// `RelationChanged.RELATION_INSIEGE` (0x200) — the "in a siege" bit.
const RELATION_INSIEGE: i32 = 0x200;
/// `RelationChanged.RELATION_ENEMY` (0x1000) — the red, attackable siege icon.
const RELATION_ENEMY: i32 = 0x1000;
/// `RelationChanged.RELATION_ALLY` (0x800) — the blue same-side siege icon.
const RELATION_ALLY: i32 = 0x800;
/// `RelationChanged.RELATION_ATTACKER` (0x400) — set on a besieger's own crown.
const RELATION_ATTACKER: i32 = 0x400;
/// `Player.getSiegeState()`'s attacker value.
const ATTACKER_SIDE: u8 = 1;

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

/// The castle id of the active siege zone the creature stands in, if any. A
/// `SiegeZone` is only "active" (PvP) while its castle's siege runs, so — unlike
/// the static arena flag — this is a position + siege-state lookup.
pub(crate) fn active_siege_castle(world: &World, oid: i32) -> Option<i32> {
    let pos = world
        .objects
        .get_component::<crate::model::components::Position>(&oid)?;
    let castle_id = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z)?;
    world
        .sieges
        .get(&castle_id)
        .filter(|s| s.in_progress)
        .map(|_| castle_id)
}

/// Java `Player.isInSiege()` as the UserInfo relation reads it: the player is a
/// registered participant of an active siege *and* stands in that castle's siege
/// zone. This sets the in-siege bit (0x80) that puts the siege crown on their
/// character — Java `Siege.updatePlayerSiegeStateFlags` does `setInSiege(true)`
/// only for attacker/defender clan members inside the zone (`checkIfInZone`).
pub(crate) fn is_in_siege(world: &World, oid: i32) -> bool {
    let Some(castle_id) = active_siege_castle(world, oid) else {
        return false;
    };
    let clan_id = world
        .objects
        .get_component::<Player>(&oid)
        .map_or(0, |p| p.clan_id);
    clan_id != 0
        && world
            .sieges
            .get(&castle_id)
            .is_some_and(|s| s.is_registered(clan_id))
}

/// Both creatures stand in the *same* castle's active siege zone (Java
/// `SiegeZone` active → siege PvP).
fn in_active_siege_together(world: &World, a_oid: i32, b_oid: i32) -> bool {
    matches!(
        (active_siege_castle(world, a_oid), active_siege_castle(world, b_oid)),
        (Some(a), Some(b)) if a == b
    )
}

/// Java `Playable.checkIfPvP(target)`: is this a *legitimate* PvP engagement —
/// one that shortens the attacker's flag to `PVP_PVP_TIME`, and (on a kill)
/// costs them no karma?
///
/// True when the target is already "in PvP": a PK or currently flagged. **Party
/// mates are explicitly not** — killing one is a PK. Java's remaining legs need
/// systems this port lacks: clan wars (a mutual war makes kills lawful,
/// TODO(G18)) and the faction dark-side check.
pub(crate) fn check_if_pvp(world: &World, self_oid: i32, target_oid: i32) -> bool {
    if self_oid == target_oid {
        return false;
    }
    if is_pk(world, target_oid) || flag_of(world, target_oid) > 0 {
        return true;
    }
    // Party mates are explicitly not PvP (killing one is a PK) — checked
    // before the war leg in Java; the fallthrough below covers it since a
    // party mate in the same clan can't be at war with themselves.
    // The clan-war leg (Java `Playable.checkIfPvP`'s tail): a MUTUAL war
    // between the clans makes the kill lawful.
    let self_clan = world
        .objects
        .get_component::<Player>(&self_oid)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    let target_clan = world
        .objects
        .get_component::<Player>(&target_oid)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    super::clans::mutual_war_between(world, self_clan, target_clan)
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
    // pair check).
    if in_pvp_zone(world, attacker_oid) && in_pvp_zone(world, target_oid) {
        return true;
    }
    // Siege: both in the same castle's active siege zone → freely attackable,
    // **except** for same-side clans (Java `isAutoAttackable`'s siege block,
    // which reads the *clan's* registration rather than the player flag):
    //
    // - two DEFENDER clans never fight;
    // - two ATTACKER clans fight only once the castle has had its **first mid
    //   victory** — until someone engraves it, the besiegers are allies.
    //
    // Same clan falls through: a clanmate is not auto-attackable anyway.
    if let Some(castle_id) = both_in_same_active_siege(world, attacker_oid, target_oid) {
        let (a_clan, t_clan) = (clan_of(world, attacker_oid), clan_of(world, target_oid));
        if a_clan != 0
            && t_clan != 0
            && a_clan != t_clan
            && let Some(siege) = world.sieges.get(&castle_id)
        {
            if siege.is_defender(a_clan) && siege.is_defender(t_clan) {
                return false;
            }
            if siege.is_attacker(a_clan) && siege.is_attacker(t_clan) {
                return world
                    .castles
                    .iter()
                    .find(|c| c.id == castle_id)
                    .is_some_and(|c| c.first_mid_victory);
            }
        }
        return true;
    }
    // Mutual clan war → freely attackable (Java `isAutoAttackable`'s
    // `isAtWarWith` both-ways test; the shared war object makes MUTUAL
    // symmetric).
    let attacker_clan = world
        .objects
        .get_component::<Player>(&attacker_oid)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    let target_clan = world
        .objects
        .get_component::<Player>(&target_oid)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    if super::clans::mutual_war_between(world, attacker_clan, target_clan) {
        return true;
    }
    is_pk(world, target_oid) || flag_of(world, target_oid) > 0
}

/// Java `Creature.getActingPlayer()` — the player behind an actor.
///
/// A player is their own acting player; a **summon's** is its owner (Java
/// `Summon.getActingPlayer()` returns `_owner`). Everything else has none, and
/// is returned unchanged so caller guards still reject it.
///
/// The port had no equivalent, so every rule expressed in Java as "do X to
/// `getActingPlayer()`" silently skipped summons. PvP flagging was the case
/// with teeth: a player could attack through their pet and never go purple,
/// leaving the victim unable to retaliate without taking the karma.
pub(crate) fn acting_player(world: &World, object_id: i32) -> i32 {
    world
        .objects
        .get_component::<crate::model::components::ServitorOf>(&object_id)
        .map(|s| s.owner_object_id)
        // A symbol totem (`EffectPoint`) also acts as its summoner — Java's
        // `EffectPoint.getActingPlayer()` returns `_owner`.
        .or_else(|| {
            world
                .objects
                .get_component::<crate::model::components::SummonerRef>(&object_id)
                .map(|s| s.0)
        })
        .unwrap_or(object_id)
}

/// Java `Player.updatePvPStatus()` (no target): self-flag from a "supporting"
/// action (buffing a monster or a flagged/PK player). No-op inside a PVP zone.
pub(crate) fn update_pvp_status(world: &mut World, object_id: i32) {
    // `if (isInsideZone(ZoneId.PVP)) return;` — no flag inside an arena, nor in
    // an active siege zone (siege participants use siege relations, not flags).
    if in_pvp_zone(world, object_id) || active_siege_castle(world, object_id).is_some() {
        return;
    }
    set_flag_lasts(world, object_id, PVP_NORMAL_TICKS);
}

/// Java `Player.updatePvPStatus(Creature target)`: flag the actor for a hostile
/// action toward a player `target`. Duration is `PVP_PVP_TIME` when the target
/// is already in PvP (`checkIfPvP`), else `PVP_NORMAL_TIME`.
pub(crate) fn update_pvp_status_target(world: &mut World, object_id: i32, target_oid: i32) {
    // Java flags `getActingPlayer()`, and a `Summon`'s is its **owner** — so
    // setting your pet or servitor on another player flags *you*. Resolving
    // here rather than at each call site is what `getActingPlayer()` buys:
    // every flagging path gets the summon case for free.
    let object_id = acting_player(world, object_id);

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
    // Same carve-out for a shared active siege zone.
    if in_active_siege_together(world, object_id, target_oid) {
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

/// Siege relation bits for `subject` as `other` sees them — Java
/// `Player.getRelation(target)`'s `_siegeState != 0` block, where `this` is the
/// subject the packet describes.
///
/// **Argument order is load-bearing.** The bits describe the *subject*: INSIEGE
/// appears only if the subject is a registered participant, ATTACKER only if
/// the subject is a besieger. Only the ENEMY-vs-ALLY choice is symmetric. The
/// previous zone-derived implementation *was* symmetric, so the call sites
/// could pass either order; they can't now.
fn siege_relation_bits(world: &World, subject_oid: i32, other_oid: i32) -> i32 {
    let subject_state = siege_state_of(world, subject_oid);
    if subject_state == 0 {
        return 0;
    }
    let mut bits = RELATION_INSIEGE;
    if subject_state == siege_state_of(world, other_oid) {
        bits |= RELATION_ALLY;
    } else {
        bits |= RELATION_ENEMY;
    }
    if subject_state == ATTACKER_SIDE {
        bits |= RELATION_ATTACKER;
    }
    bits
}

/// Java `Player.getSiegeState()`.
fn siege_state_of(world: &World, object_id: i32) -> u8 {
    world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.siege_state)
        .unwrap_or(0)
}

/// The castle whose *active* siege zone both players stand in, if any.
fn both_in_same_active_siege(world: &World, a_oid: i32, b_oid: i32) -> Option<i32> {
    match (
        active_siege_castle(world, a_oid),
        active_siege_castle(world, b_oid),
    ) {
        (Some(a), Some(b)) if a == b => Some(a),
        _ => None,
    }
}

fn clan_of(world: &World, object_id: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.clan_id)
        .unwrap_or(0)
}

/// Java `Player.sendInfo`'s `RelationChanged` half: how `subject` relates to
/// `viewer` right now — the clan-leader crown (`RELATION_LEADER`), clan/party
/// bits, the siege enemy state — plus whether `subject` is attackable by
/// `viewer`. Emitted next to every `CharInfo` so a clan leader's crown shows the
/// moment they come into view, not only inside a siege zone (the crown rides
/// `RelationChanged`, since `CharInfo` carries no is-leader field).
pub(crate) fn sendinfo_relation_changed(
    world: &World,
    subject_oid: i32,
    viewer_oid: i32,
) -> Vec<u8> {
    let base = super::party::relation_changed_base(world, subject_oid);
    let siege = siege_relation_bits(world, subject_oid, viewer_oid);
    let war = super::clans::war_relation_bits(world, subject_oid, viewer_oid);
    let reputation = world
        .objects
        .get_component::<Player>(&subject_oid)
        .map_or(0, |p| p.reputation);
    server_packets::relation_changed(
        subject_oid,
        base | siege | war,
        is_player_auto_attackable(world, viewer_oid, subject_oid),
        reputation,
        flag_of(world, subject_oid),
    )
}

fn relation_parts(world: &World, oid: i32) -> (i32, i32, u8) {
    let Some(p) = world.objects.get_component::<Player>(&oid) else {
        return (0, 0, 0);
    };
    // RelationChanged uses `Player.getRelation`'s bitmask (leader = 0x80), not
    // `UserInfo.calculateRelation`'s (leader = 0x40) — the former is what carries
    // the on-head clan-leader crown.
    (
        super::party::relation_changed_base(world, oid),
        p.reputation,
        flag_of(world, oid),
    )
}

/// Java `Player.broadcastRelationChanged`, for the siege case: refresh how
/// `object_id` and every nearby player relate now, per-viewer (siege enemy/ally
/// is viewer-dependent). Sent on siege-zone enter/exit so the client shows — or
/// clears — the attackable state that neither `CharInfo` nor the pvp-flag path
/// carries. Without it, a combatant entering the zone never appears attackable.
pub(crate) fn broadcast_siege_relation(world: &World, object_id: i32) {
    let Some(my_region) = world
        .objects
        .get_component::<RegionCell>(&object_id)
        .map(|r| r.0)
    else {
        return;
    };
    let my_client = client_for_player(world, object_id).and_then(|c| world.clients.get(&c));
    let (my_relation, my_rep, my_flag) = relation_parts(world, object_id);
    for cs in world.clients.values() {
        let ClientSession::InGame(s) = cs else {
            continue;
        };
        let viewer = s.player_object_id();
        if viewer == object_id {
            continue;
        }
        let Some(vr) = world
            .objects
            .get_component::<RegionCell>(&viewer)
            .map(|r| r.0)
        else {
            continue;
        };
        if !regions_adjacent(my_region, vr) {
            continue;
        }
        // How `object_id` relates to (and is attackable by) this viewer.
        cs.send(server_packets::relation_changed(
            object_id,
            my_relation
                | siege_relation_bits(world, object_id, viewer)
                | super::clans::war_relation_bits(world, object_id, viewer),
            is_player_auto_attackable(world, viewer, object_id),
            my_rep,
            my_flag,
        ));
        // The reverse, so `object_id`'s own client sees the viewer too.
        if let Some(mc) = my_client {
            let (v_rel, v_rep, v_flag) = relation_parts(world, viewer);
            mc.send(server_packets::relation_changed(
                viewer,
                v_rel
                    | siege_relation_bits(world, viewer, object_id)
                    | super::clans::war_relation_bits(world, viewer, object_id),
                is_player_auto_attackable(world, object_id, viewer),
                v_rep,
                v_flag,
            ));
        }
    }
}

/// Java `SiegeZone.onExit`'s `if (player.getPvpFlag() == 0) startPvPFlag()`:
/// leaving an active siege zone raises the flag on a currently-unflagged player
/// (for `PVP_NORMAL_TIME`), which the `PvpFlagTaskManager` then blinks out. An
/// already-flagged player keeps their existing (unrefreshed) timer.
pub(crate) fn start_pvp_flag_on_siege_exit(world: &mut World, object_id: i32) {
    if flag_of(world, object_id) == 0 {
        set_flag_lasts(world, object_id, PVP_NORMAL_TICKS);
    }
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
    // RelationChanged → nearby players. Carry the player's real relation bits
    // (Java `broadcastRelationChanged` sends `getRelation`), not 0 — otherwise a
    // flag change would strip a clan leader's on-head crown. `auto_attackable` is
    // the viewer-independent core (flagged or PK).
    let auto_attackable = value > 0 || reputation < 0;
    let relation = super::party::relation_changed_base(world, object_id);
    super::helpers::broadcast_to_others(
        world,
        object_id,
        &server_packets::relation_changed(object_id, relation, auto_attackable, reputation, value),
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

// ---------------------------------------------------------------------------
// Kill consequences (`Player.onKillUpdatePvPReputation`)
// ---------------------------------------------------------------------------

/// Java `Formulas.calculateKarmaGain(pkCount, isSummon)` — how much reputation
/// a player loses for an unlawful kill. Summons aren't ported, so only the
/// player brackets are: a flat 43 200 above 180 kills, and the two rising
/// brackets below that. (Reputation is stored negative, so the caller
/// *subtracts* this.)
pub(crate) fn calculate_karma_gain(pk_count: i32) -> i32 {
    if pk_count < 99 {
        (((pk_count as f64 * 0.5) + 1.0) * 60.0 * 12.0) as i32
    } else if pk_count < 180 {
        (((pk_count as f64 * 0.125) + 37.75) * 60.0 * 12.0) as i32
    } else {
        43_200
    }
}

/// `Config.ReputationIncrease` — reputation granted for killing a PK. **0 on
/// this dist**, so the branch that uses it is inert here; ported for
/// faithfulness (and so an operator raising it gets the retail behaviour).
const REPUTATION_INCREASE: i32 = 0;

/// `Player.onKillUpdatePvPReputation` — the counters and karma a player kill
/// moves. Called from the victim's death path with their killer.
///
/// Three outcomes, in Java's order:
/// 1. a **legitimate PvP** kill (victim was flagged or a PK) → `pvp_kills++`,
///    plus reputation back for killing a PK within ±10 levels;
/// 2. otherwise, a killer with **positive reputation and no prior PKs** has it
///    reset to 0 — the "first offence" grace;
/// 3. otherwise a real **PK**: karma is added (reputation goes down) and
///    `pk_kills++`.
///
/// Nothing happens at all when either side is inside a PVP zone.
pub(crate) fn on_kill_update_pvp_reputation(world: &mut World, killer_oid: i32, victim_oid: i32) {
    if killer_oid == victim_oid || !world.objects.has_component::<Player>(&killer_oid) {
        return;
    }
    if in_pvp_zone(world, killer_oid) || in_pvp_zone(world, victim_oid) {
        return; // "Do nothing when in PVP zone."
    }

    let legitimate = check_if_pvp(world, killer_oid, victim_oid);
    let (killer_rep, killer_pk) = {
        let Some(p) = world.objects.get_component::<Player>(&killer_oid) else {
            return;
        };
        (p.reputation, p.pk_kills)
    };
    let victim_rep = world
        .objects
        .get_component::<Player>(&victim_oid)
        .map_or(0, |p| p.reputation);
    let level_diff = {
        let v = world
            .objects
            .get_component::<Player>(&victim_oid)
            .map_or(0, |p| p.level);
        let k = world
            .objects
            .get_component::<Player>(&killer_oid)
            .map_or(0, |p| p.level);
        v - k
    };

    if let Some(p) = world.objects.get_component_mut::<Player>(&killer_oid) {
        if legitimate {
            // Killing a PK within ±10 levels earns reputation back.
            if victim_rep < 0 && killer_rep >= 0 && level_diff < 11 && level_diff > -11 {
                p.reputation = killer_rep + REPUTATION_INCREASE;
            }
            p.pvp_kills += 1;
        } else if killer_rep > 0 && killer_pk == 0 {
            // First offence with positive reputation: reset rather than punish.
            p.reputation = 0;
            p.pk_kills += 1;
        } else {
            p.reputation = killer_rep - calculate_karma_gain(killer_pk);
            p.pk_kills += 1;
        }
    }

    // `broadcastUserInfo(UserInfoType.SOCIAL)` — the name/title colour and the
    // karma flag other clients draw come from here.
    super::party::broadcast_user_info(world, killer_oid);
}
