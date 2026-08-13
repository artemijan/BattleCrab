//! Player-info views shared far beyond the party system: the UserInfo/CharInfo
//! relation bitmask, the broadcast helpers that push a changed player to
//! everyone who can see them, and the CharInfo state block. Extracted from
//! `party.rs`, whose 30+ external callers only ever wanted these.

use crate::game_loop::helpers::{broadcast_to_others, is_dead, send_to_player};
use crate::model::Player;
use crate::model::components::PartyRef;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;
/// Java `UserInfo.calculateRelation` — the party/clan/siege relation bitmask the
/// `UserInfo` RELATION block carries. Party membership comes off the
/// `PartyRef` component (absent → not in a party), clan off the `Player`.
/// Takes `&Player` so the clan bits are correct even before the object is
/// registered (the enter-world burst).
pub(crate) fn calculate_relation(world: &World, player: &Player) -> i32 {
    let mut relation = 0;
    if let Some(PartyRef(pid)) = world
        .objects
        .get_component::<PartyRef>(&player.object_id)
        .copied()
        && let Some(party) = world.parties.get(&pid)
    {
        relation |= 0x08; // party member
        if party.is_leader(player.object_id) {
            relation |= 0x10; // party leader
        }
    }
    if player.clan_id > 0 {
        relation |= 0x20; // clan member
        if player.clan_leader {
            relation |= 0x40; // clan leader
        }
    }
    if super::pvp::is_in_siege(world, player.object_id) {
        relation |= 0x80; // in siege — draws the siege crown (Java `isInSiege()`)
    }
    relation
}

/// Java `Player.getRelation(target)` — the bitmask the **`RelationChanged`**
/// packet carries, as `subject` appears *to* `viewer`.
///
/// A different layout from [`calculate_relation`] (which is `UserInfo`'s): here
/// clan member is `0x40` and the clan-leader bit (the one that draws the on-head
/// crown) is `0x80`. The siege enemy/ally and clan-war bits are folded in by the
/// caller; everything Java computes inside `getRelation` itself is here.
///
/// **It is viewer-dependent, and three of the bits are not obvious about it.**
/// `RELATION_HAS_PARTY` and the whole party-slot block only appear when
/// `party == target.getParty()` — the same party, not merely *a* party — so a
/// stranger never learns you are grouped. `RELATION_CLAN_MATE` compares the two
/// clans. `RELATION_ALLY_MEMBER` is the odd one out: it sits inside the
/// `clan != null` branch but asks only whether **the subject** has an ally, so it
/// is the same for every viewer.
pub(crate) fn relation_to(world: &World, subject_oid: i32, viewer_oid: i32) -> i32 {
    const RELATION_HAS_PARTY: i32 = 0x20;
    const RELATION_CLAN_MEMBER: i32 = 0x40;
    const RELATION_LEADER: i32 = 0x80;
    const RELATION_CLAN_MATE: i32 = 0x100;
    const RELATION_ALLY_MEMBER: i32 = 0x10000;

    let Some(p) = world.objects.get_component::<Player>(&subject_oid) else {
        return 0;
    };
    let mut relation = 0;
    if p.clan_id > 0 {
        relation |= RELATION_CLAN_MEMBER;
        if world
            .objects
            .get_component::<Player>(&viewer_oid)
            .is_some_and(|v| v.clan_id == p.clan_id)
        {
            relation |= RELATION_CLAN_MATE;
        }
        if p.ally_id != 0 {
            relation |= RELATION_ALLY_MEMBER;
        }
    }
    if p.clan_leader {
        relation |= RELATION_LEADER;
    }
    // Java: `(party != null) && (party == target.getParty())`.
    let party_of = |oid: i32| {
        world
            .objects
            .get_component::<PartyRef>(&oid)
            .map(|PartyRef(pid)| *pid)
    };
    if let Some(pid) = party_of(subject_oid)
        && party_of(viewer_oid) == Some(pid)
        && let Some(party) = world.parties.get(&pid)
    {
        relation |= RELATION_HAS_PARTY;
        if let Some(i) = party.members.iter().position(|&m| m == subject_oid) {
            relation |= party_slot_bits(i);
        }
    }
    relation
}

/// Java `getRelation`'s party-index `switch` — the client reads the member's
/// position in the party out of these low bits.
///
/// The encoding is not an index and not a bitfield: slot 0 is the leader flag
/// `0x10`, and slots 1..=8 count **down** from `0x8` to `0x1`, so member *i*
/// carries the value `9 - i`. Java spells all nine cases out longhand with the
/// values written as sums (`PARTY3 + PARTY2 + PARTY1` for 7), which hides the
/// arithmetic; the switch and this expression agree on every case, and slots
/// past 8 fall through to no bits at all in both (an Interlude party caps at 9).
pub(crate) fn party_slot_bits(index: usize) -> i32 {
    const RELATION_PARTYLEADER: i32 = 0x10;
    match index {
        0 => RELATION_PARTYLEADER,
        1..=8 => 9 - index as i32,
        _ => 0,
    }
}

/// `Player.broadcastUserInfo()` — fresh `UserInfo` to self, and Java's
/// **coalesced** `CharInfo` to everyone who can see them:
/// `broadcastCharInfo` never sends inline, it schedules
/// `_broadcastCharInfoTask` 50 ms out and folds every call made in that
/// window into one packet — which both spares onlookers a `CharInfo` per
/// update in a burst and lands the packet *after* whatever actor swap (a
/// `Ride`, a transform) preceded it.
pub(crate) fn broadcast_user_info(world: &mut World, object_id: i32) {
    let Some(v) = crate::model::PlayerView::of_world(world, object_id) else {
        return;
    };
    let relation = calculate_relation(world, v.p);
    send_to_player(
        world,
        object_id,
        crate::network::user_info::user_info(&v, &world.data, &world.cfg.character, relation),
    );
    // `if (_broadcastCharInfoTask == null) { schedule(50ms) }`.
    let pending = world
        .objects
        .get_component::<Player>(&object_id)
        .is_none_or(|p| p.char_info_pending);
    if pending {
        return;
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.char_info_pending = true;
    }
    world.scheduler.schedule(
        world.tick + crate::game_loop::helpers::ms_to_ticks(50),
        ScheduledTask::BroadcastCharInfo { object_id },
    );
}

/// The `_broadcastCharInfoTask` body: build the `CharInfo` **now** (state can
/// have moved since the calls that scheduled it) and send it to onlookers.
pub(crate) fn broadcast_char_info_now(world: &mut World, object_id: i32) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.char_info_pending = false;
    }
    let Some(v) = crate::model::PlayerView::of_world(world, object_id) else {
        return;
    };
    let cubics = world
        .objects
        .get_component::<super::cubic::Cubics>(&object_id)
        .map(|c| c.ids())
        .unwrap_or_default();
    // A hidden GM's CharInfo must not reach other players: Java's
    // `broadcastCharInfo` checks `isVisibleFor` per recipient; the port
    // suppresses wholesale, same as `visibility::send_char_info` (the
    // SEE_ALL_PLAYERS cond-override isn't modeled). Without this gate any
    // UserInfo-broadcasting action (transform, title, store, buff…) popped a
    // hidden GM back onto every nearby client.
    if world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&object_id)
        .is_some_and(|f| f.hidden)
    {
        return;
    }
    let char_info = server_packets::char_info(
        &v,
        &super::abnormal::visual_effects(world, object_id),
        &cubics,
        &char_info_state(world, object_id),
    );
    broadcast_to_others(world, object_id, &char_info);
}

/// Gather the manager-sourced `CharInfo` fields Java reads inside the packet
/// ctor (`CursedWeaponsManager`, `AttackStanceTaskManager`, the clan, death,
/// the fishing session) — see [`server_packets::CharInfoState`].
pub(crate) fn char_info_state(world: &World, object_id: i32) -> server_packets::CharInfoState {
    let p = world.objects.get_component::<Player>(&object_id);
    let clan = p
        .filter(|p| p.clan_id != 0)
        .and_then(|p| world.clans.get(&p.clan_id));
    server_packets::CharInfoState {
        in_combat: super::combat::has_attack_stance(world, object_id),
        // Java gates the byte on `!isInOlympiadMode()` so a downed Olympiad
        // fighter keeps standing until the match ends.
        alike_dead: !world.olympiad.is_in_competition(object_id) && is_dead(world, object_id),
        cursed_weapon_level: p
            .filter(|p| p.cursed_weapon_equipped_id != 0)
            .and_then(|p| {
                world
                    .cursed_weapons
                    .iter()
                    .find(|w| w.item_id == p.cursed_weapon_equipped_id)
            })
            .map_or(0, |w| w.level() as u8),
        clan_crest_large_id: clan.map_or(0, |c| c.crest_large_id),
        clan_reputation: clan.map_or(0, |c| c.reputation_score),
        fishing_bait: world
            .objects
            .get_component::<crate::model::components::FishingSession>(&object_id)
            .filter(|f| f.is_fishing)
            .map(|f| (f.bait_x, f.bait_y, f.bait_z)),
        armor_min_enchant: crate::game_loop::armor_sets::max_set_enchant(world, object_id)
            .clamp(0, u8::MAX as i32) as u8,
    }
}
