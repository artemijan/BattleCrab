//! PvP flagging — the ported subset of `Player.updatePvPStatus` /
//! `updatePvPFlag` / `isAutoAttackable` plus the `PvpFlagTaskManager` sweep.
//!
//! Systems that don't exist yet (clans, parties, duels, olympiad, sieges,
//! faction, dark-side) are dropped from the ported checks, so the relations
//! reduce to karma (`Player.reputation`) + the runtime flag. PVP-zone (arena)
//! exemptions land with Phase 2, when those zones are loaded.

use crate::game_loop::guard::clan_of_or_zero;
use crate::model::Player;
use crate::model::components::{PvpState, ZoneFlags};
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::{World, regions_adjacent};

use super::helpers::{broadcast_including_self, client_for_player};
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::helpers::send_sm_to_player;
use crate::game_loop::helpers::send_to_client;

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
pub(crate) fn in_pvp_zone(world: &World, oid: i32) -> bool {
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
    super::siege::active_siege_castle_at(world, pos.x, pos.y, pos.z)
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
    let clan_id = clan_of_or_zero(world, oid);
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
/// mates are explicitly not** — killing one is a PK. A MUTUAL clan war makes
/// the kill lawful (the tail below); Java's `isOnDarkSide()` faction leg has
/// no Interlude counterpart — factions are a later-chronicle system with no
/// state on this dist.
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
    // Java skips the whole leg for academy members on either side
    // (`isAcademyMember`, pledge type -1).
    let clan_of = |oid: i32| {
        world
            .objects
            .get_component::<Player>(&oid)
            .filter(|p| p.pledge_type != -1)
            .map(|p| p.clan_id)
            .unwrap_or(0)
    };
    super::clans::mutual_war_between(world, clan_of(self_oid), clan_of(target_oid))
}

/// Java `Player.isAutoAttackable(attacker)` narrowed to the ported systems: a
/// target player can be freely attacked (no Ctrl needed) when it's a PK or
/// already flagged; monsters attack anyone; a target in a peace zone can't be
/// attacked. Party/clan/duel/oly/siege/faction and the PVP-zone (arena)
/// exemption are not modeled yet.
/// Both in a party, both parties in a command channel, and the *same* one —
/// Java's `getParty().getCommandChannel() == attacker.getParty().getCommandChannel()`
/// with all four null-guards. Two soloists are not friends; two parties with no
/// channel are not friends.
fn same_command_channel(world: &World, a_oid: i32, b_oid: i32) -> bool {
    let cc = |oid: i32| {
        super::command_channel::party_id_of(world, oid)
            .and_then(|pid| super::command_channel::cc_id_of_party(world, pid))
    };
    match (cc(a_oid), cc(b_oid)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

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
    // `AltCommandChannelFriends` — "Same Command Channel are friends". Java
    // puts this immediately after the peace-zone check, *ahead* of the arena
    // and siege arms, so two parties raiding together cannot hit each other
    // even inside a PvP zone.
    if world.cfg.character.alt_command_channel_friends
        && same_command_channel(world, attacker_oid, target_oid)
    {
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
        let (a_clan, t_clan) = (
            clan_of_or_zero(world, attacker_oid),
            clan_of_or_zero(world, target_oid),
        );
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
    let attacker_clan = clan_of_or_zero(world, attacker_oid);
    let target_clan = clan_of_or_zero(world, target_oid);
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
/// Java `PlayableAI`'s Blessing of Protection pair, shared by the attack and
/// bad-cast intentions: a chaotic character 10+ levels above a blessed newbie
/// cannot touch them, and the blessed newbie cannot touch a chaotic character
/// 10+ levels above either. Both ends resolve through [`acting_player`] (a
/// summon fights with its owner's karma and blessing), the blessing is the
/// `PK_PROTECT` abnormal Blessing of Protection (5182) lands, and a PVP zone
/// on the target suspends the whole thing — exactly Java's four conditions.
pub(crate) fn protection_blessing_blocks(world: &World, actor: i32, target: i32) -> bool {
    let a = acting_player(world, actor);
    let t = acting_player(world, target);
    let (Some(ap), Some(tp)) = (
        world.objects.get_component::<Player>(&a),
        world.objects.get_component::<Player>(&t),
    ) else {
        return false;
    };
    if world
        .objects
        .get_component::<ZoneFlags>(&t)
        .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Pvp))
    {
        return false;
    }
    let blessed = |oid: i32| {
        world
            .objects
            .get_component::<crate::model::components::Buffs>(&oid)
            .is_some_and(|b| b.0.iter().any(|x| x.abnormal_type == "PK_PROTECT"))
    };
    (blessed(t) && ap.level - tp.level >= 10 && ap.reputation < 0)
        || (blessed(a) && tp.level - ap.level >= 10 && tp.reputation < 0)
}

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
    let ticks = world.cfg.pvp.normal_ticks();
    set_flag_lasts(world, object_id, ticks);
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
        world.cfg.pvp.pvp_ticks()
    } else {
        world.cfg.pvp.normal_ticks()
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
    let base = super::player_info::relation_to(world, subject_oid, viewer_oid);
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

/// The two viewer-**independent** halves of a `RelationChanged` — reputation
/// and pvp flag. The relation bitmask itself is not here: it depends on who is
/// looking (`player_info::relation_to`), so hoisting it out of a per-viewer loop is
/// what made every onlooker see a player's party membership.
fn relation_parts(world: &World, oid: i32) -> (i32, u8) {
    let Some(p) = world.objects.get_component::<Player>(&oid) else {
        return (0, 0);
    };
    (p.reputation, flag_of(world, oid))
}

/// Java `Player.broadcastRelationChanged`, for the siege case: refresh how
/// `object_id` and every nearby player relate now, per-viewer (siege enemy/ally
/// is viewer-dependent). Sent on siege-zone enter/exit so the client shows — or
/// clears — the attackable state that neither `CharInfo` nor the pvp-flag path
/// carries. Without it, a combatant entering the zone never appears attackable.
pub(crate) fn broadcast_siege_relation(world: &World, object_id: i32) {
    let Some(my_region) = region_cell_of(world, object_id) else {
        return;
    };
    let my_client = client_for_player(world, object_id);
    let (my_rep, my_flag) = relation_parts(world, object_id);
    for cs in world.clients.values() {
        let ClientSession::InGame(s) = cs else {
            continue;
        };
        let viewer = s.player_object_id();
        if viewer == object_id {
            continue;
        }
        let Some(vr) = region_cell_of(world, viewer) else {
            continue;
        };
        if !regions_adjacent(my_region, vr) {
            continue;
        }
        // How `object_id` relates to (and is attackable by) this viewer.
        cs.send(server_packets::relation_changed(
            object_id,
            super::player_info::relation_to(world, object_id, viewer)
                | siege_relation_bits(world, object_id, viewer)
                | super::clans::war_relation_bits(world, object_id, viewer),
            is_player_auto_attackable(world, viewer, object_id),
            my_rep,
            my_flag,
        ));
        // The reverse, so `object_id`'s own client sees the viewer too.
        if let Some(mc) = my_client {
            let (v_rep, v_flag) = relation_parts(world, viewer);
            send_to_client(
                world,
                mc,
                server_packets::relation_changed(
                    viewer,
                    super::player_info::relation_to(world, viewer, object_id)
                        | siege_relation_bits(world, viewer, object_id)
                        | super::clans::war_relation_bits(world, viewer, object_id),
                    is_player_auto_attackable(world, object_id, viewer),
                    v_rep,
                    v_flag,
                ),
            );
        }
    }
}

/// Java `SiegeZone.onExit`'s `if (player.getPvpFlag() == 0) startPvPFlag()`:
/// leaving an active siege zone raises the flag on a currently-unflagged player
/// (for `PVP_NORMAL_TIME`), which the `PvpFlagTaskManager` then blinks out. An
/// already-flagged player keeps their existing (unrefreshed) timer.
pub(crate) fn start_pvp_flag_on_siege_exit(world: &mut World, object_id: i32) {
    if flag_of(world, object_id) == 0 {
        let ticks = world.cfg.pvp.normal_ticks();
        set_flag_lasts(world, object_id, ticks);
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
    //
    // Java's `broadcastRelationChanged` recomputes `getRelation(player)` **inside**
    // its visible-player loop, so this cannot be one packet shared by everyone:
    // the party and clan-mate bits differ per onlooker. It used to be, which told
    // every bystander that the flagged player was in a party.
    let auto_attackable = value > 0 || reputation < 0;
    let Some(region) = region_cell_of(world, object_id) else {
        return;
    };
    let viewers: Vec<i32> = world
        .players_visible_from(region)
        .filter(|&v| v != object_id)
        .collect();
    for viewer in viewers {
        let Some(client_id) = client_for_player(world, viewer) else {
            continue;
        };
        super::helpers::send_to_client(
            world,
            client_id,
            server_packets::relation_changed(
                object_id,
                super::player_info::relation_to(world, object_id, viewer),
                auto_attackable,
                reputation,
                value,
            ),
        );
    }
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

pub(crate) fn get_killer_rep_and_pk(world: &mut World, killer_oid: i32) -> Option<(i32, i32)> {
    let p = world.objects.get_component::<Player>(&killer_oid)?;
    Some((p.reputation, p.pk_kills))
}
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
    // Java `onPlayerKill`'s **first** branch, ahead of the olympiad, duel,
    // siege and PVP-zone bails: a cursed-weapon wielder scores the weapon and
    // `return`s, so the kill never awards pvp kills, never adds karma and
    // never counts as a PK. Placed here for that ordering — moving it below the
    // zone check would silently stop cursed kills scoring inside an arena.
    if super::cursed_weapon::on_player_kill(world, killer_oid, victim_oid) {
        return;
    }
    if in_pvp_zone(world, killer_oid) || in_pvp_zone(world, victim_oid) {
        return; // "Do nothing when in PVP zone."
    }

    let legitimate = check_if_pvp(world, killer_oid, victim_oid);

    let Some((killer_rep, killer_pk)) = get_killer_rep_and_pk(world, killer_oid) else {
        return;
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

    // `Player.setReputation` clamps to `Config.MAX_REPUTATION` — 0 here, which
    // is what stops reputation ever going positive on this dist.
    let (reputation_increase, max_reputation) = (
        world.cfg.pvp.reputation_increase,
        world.cfg.pvp.max_reputation,
    );
    if let Some(p) = world.objects.get_component_mut::<Player>(&killer_oid) {
        if legitimate {
            // Killing a PK within ±10 levels earns reputation back.
            if victim_rep < 0 && killer_rep >= 0 && level_diff < 11 && level_diff > -11 {
                p.reputation = (killer_rep + reputation_increase).min(max_reputation);
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

    // `updatePvpTitleAndColor(true)` — the `Custom/PvpTitleColor.ini` ladder,
    // applied right after the counter moves so a player crossing a threshold
    // is renamed on the spot.
    update_pvp_title_and_color(world, killer_oid, false);

    // `broadcastUserInfo(UserInfoType.SOCIAL)` — the name/title colour and the
    // karma flag other clients draw come from here.
    super::player_info::broadcast_user_info(world, killer_oid);
}

/// Java `Player.updatePvpTitleAndColor` — the five-rung ladder from
/// `Custom/PvpTitleColor.ini`. Java wraps each title in `®` and only ever
/// *raises* a player (there is no arm that clears the title back), so a
/// demotion is impossible and a player below the first rung keeps whatever
/// title they had.
///
/// `broadcast` mirrors Java's parameter: the kill path broadcasts through its
/// own `broadcastUserInfo` a line later, the enter-world path does not.
pub(crate) fn update_pvp_title_and_color(world: &mut World, player_oid: i32, broadcast: bool) {
    let Some(kills) = world
        .objects
        .get_component::<Player>(&player_oid)
        .map(|p| p.pvp_kills)
    else {
        return;
    };
    let Some(rank) = world.cfg.pvp_title_color.rank_for(kills) else {
        return;
    };
    let (title, color) = (format!("® {} ®", rank.title), rank.color);
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.title = title;
        p.title_color = color;
    }
    if broadcast {
        super::player_info::broadcast_user_info(world, player_oid);
    }
}

/// Java `Player.doDie`'s reward block: on a PvP or PK kill the killer is paid a
/// configured item. **A sibling of the reputation update, not part of it** —
/// `on_kill_update_pvp_reputation` returns early inside a PvP zone, so hanging
/// the reward off it would make `DisableRewardsInPvpZones` unreachable and the
/// config key meaningless (which is exactly what a sabotage run caught). The victim's **flag** picks the arm — a flagged victim is a
/// PvP kill, an unflagged one makes the killer a PK — and one shared guard
/// covers both (`DisableRewardsInInstances` / `DisableRewardsInPvpZones`, both
/// on here, tested against the **victim**).
pub(crate) fn pay_kill_reward(world: &mut World, killer_oid: i32, victim_oid: i32) {
    let cfg = world.cfg.pvp_reward.clone();
    if !cfg.reward_pvp && !cfg.reward_pk {
        return;
    }
    if cfg.disable_in_instances
        && world
            .objects
            .get_component::<crate::model::components::InstanceId>(&victim_oid)
            .is_some_and(|i| i.0 != 0)
    {
        return;
    }
    if cfg.disable_in_pvp_zones && in_pvp_zone(world, victim_oid) {
        return;
    }
    let victim_flagged = world
        .objects
        .get_component::<PvpState>(&victim_oid)
        .is_some_and(|f| f.flag != 0);
    let (enabled, item_id, amount, message) = if victim_flagged {
        (
            cfg.reward_pvp,
            cfg.pvp_item_id,
            cfg.pvp_item_amount,
            cfg.pvp_message,
        )
    } else {
        (
            cfg.reward_pk,
            cfg.pk_item_id,
            cfg.pk_item_amount,
            cfg.pk_message,
        )
    };
    if !enabled || amount <= 0 {
        return;
    }
    super::items::add_inventory_item(world, killer_oid, item_id, amount);
    if message {
        send_sm_to_player(
            world,
            killer_oid,
            crate::network::server_packets::sm_ids::YOU_HAVE_OBTAINED_S2_S1,
            &[
                crate::network::server_packets::SmParam::ItemName(item_id),
                crate::network::server_packets::SmParam::Long(amount),
            ],
        );
    }
}
