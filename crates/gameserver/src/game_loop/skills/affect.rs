//! Affect scopes and affect objects — how one cast's *primary* target expands
//! into the set of creatures the skill actually lands on.
//!
//! Java splits this across `Skill.forEachTargetAffected` →
//! `handlers/targethandlers/affectscope/*` (the sweep) →
//! `handlers/targethandlers/affectobject/*` (the friend/foe filter). Both sides
//! live here; the caller ([`super::cast::handle_skill_finish`]) just iterates
//! the returned list.
//!
//! **Ported scopes** — the four that cover the dist's non-single skills:
//! `RANGE` (820 skills), `POINT_BLANK` (785), `PARTY` (272), `PLEDGE` (44).
//! **Not ported (`TODO(G19)`), all falling back to single-target:** the
//! geometric cone/rectangle scopes `FAN`/`FAN_PB` (179) and `SQUARE`/`SQUARE_PB`
//! (52) — they need the caster-heading arc/rect math; `RING_RANGE` (18, an
//! annulus with an inner radius); `RANGE_SORT_BY_HP` (4); `SUMMON_EXCEPT_MASTER`
//! (22) and `WYVERN_SCOPE`/`BALAKAS_SCOPE`, which need summons (G29) or boss
//! scripting (G23); the `DEAD_*` family (mass resurrect — needs the res flow);
//! `PARTY_PLEDGE` (5); and `STATIC_OBJECT_SCOPE`.
//!
//! **Deviation worth knowing:** Java sweeps `World.forEachVisibleObjectInRange`,
//! which walks the region grid and so is bounded by *visibility*, not just the
//! numeric radius. This port sweeps the same 3×3 region block
//! (`npcs_visible_from` + the in-game player list) and then applies the radius,
//! which is the same set for every `affect_range` the dist actually uses (the
//! largest is 2000, comfortably inside a region block).

use crate::model::components::{Position, RegionCell, Vitals};
use crate::model::skill::{AffectObject, AffectScope, Skill, TargetType};
use crate::model::Player;
use crate::world::{regions_adjacent, World};

/// Resolve the full set of creatures a cast lands on, primary target first.
///
/// Java `Skill.getTargetsAffected(creature, target)`. The order matters: the
/// primary target is always first (and always included — the scope filters
/// never drop it), so callers that treat the first entry specially — the
/// resist-message path, the "main target" damage — behave as they did before
/// scopes existed.
pub(crate) fn targets_affected(world: &mut World, caster_oid: i32, target_oid: i32, skill: &Skill) -> Vec<i32> {
    // The limit is rolled once per cast (Java calls `getAffectLimit()` once at
    // the top of each handler), so it must be drawn before the sweep.
    let limit = skill.affect_limit(|bound| world.roll(bound));

    match skill.affect_scope {
        // `Single.java` — and every scope we haven't ported yet, which Java
        // would refuse outright ("Target affect scope ... is not currently
        // handled") but which is far less disruptive to treat as single-target.
        AffectScope::Single | AffectScope::Other => vec![target_oid],
        AffectScope::Range => sweep_radius(world, caster_oid, target_oid, skill, limit, Centre::Target),
        AffectScope::PointBlank => sweep_radius(world, caster_oid, target_oid, skill, limit, Centre::Caster),
        AffectScope::Party => sweep_group(world, caster_oid, target_oid, skill, limit, Group::Party),
        AffectScope::Pledge => sweep_group(world, caster_oid, target_oid, skill, limit, Group::Clan),
    }
}

/// Which point a radius sweep is measured from — the difference between
/// `Range` (the target) and `PointBlank` (the caster).
#[derive(Clone, Copy, PartialEq)]
enum Centre {
    Target,
    Caster,
}

#[derive(Clone, Copy, PartialEq)]
enum Group {
    Party,
    Clan,
}

/// `Range.java` / `PointBlank.java` — every creature within `affect_range` of
/// the centre point, filtered and capped.
fn sweep_radius(
    world: &World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    limit: i32,
    centre: Centre,
) -> Vec<i32> {
    let mut out = vec![target_oid];
    let range = skill.affect_range;
    if range <= 0 {
        // Java would sweep a 0-radius circle and find nothing but the target.
        return out;
    }
    let centre_oid = match centre {
        Centre::Target => target_oid,
        Centre::Caster => caster_oid,
    };
    let Some(origin) = world.objects.get_component::<Position>(&centre_oid).copied() else { return out };
    // LOS is measured from the *target* in both Java handlers, even for
    // PointBlank (`canSeeTarget(target, c)`).
    let los_from = world.objects.get_component::<Position>(&target_oid).copied();

    // `affected` counts the primary target: Java's Range handler runs the
    // filter over the origin object first and increments there.
    let mut affected = 1;
    for candidate in candidates(world, centre_oid) {
        if candidate == target_oid {
            continue; // already in `out`
        }
        if limit > 0 && affected >= limit {
            break;
        }
        // "Range skills appear to not affect you unless you are the main
        // target" — the caster is swept up only when they *are* the target.
        if candidate == caster_oid && target_oid != caster_oid {
            continue;
        }
        if is_dead(world, candidate) && !corpse_skill(skill) {
            continue;
        }
        let Some(pos) = world.objects.get_component::<Position>(&candidate).copied() else { continue };
        if !within(&origin, &pos, range) {
            continue;
        }
        if !passes_affect_object(world, caster_oid, candidate, skill.affect_object) {
            continue;
        }
        if let Some(from) = los_from {
            if !world.geo.can_see_target(from.x, from.y, from.z, pos.x, pos.y, pos.z) {
                continue;
            }
        }
        out.push(candidate);
        affected += 1;
    }
    out
}

/// `Party.java` / `Pledge.java` — the target's group rather than a raw radius.
/// Both still respect `affect_range` (Java filters members by distance from the
/// *target*) and the limit.
fn sweep_group(
    world: &World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    limit: i32,
    group: Group,
) -> Vec<i32> {
    let mut out = vec![target_oid];
    let Some(origin) = world.objects.get_component::<Position>(&target_oid).copied() else { return out };
    let range = skill.affect_range;

    let members: Vec<i32> = match group {
        Group::Party => world
            .objects
            .get_component::<crate::model::components::PartyRef>(&target_oid)
            .and_then(|r| world.parties.get(&r.0))
            .map(|p| p.members.clone())
            // Java: an unpartied target is still "their own party of one".
            .unwrap_or_else(|| vec![target_oid]),
        Group::Clan => {
            let clan_id = world.objects.get_component::<Player>(&target_oid).map(|p| p.clan_id).unwrap_or(0);
            if clan_id <= 0 {
                vec![target_oid]
            } else {
                in_game_players(world)
                    .into_iter()
                    .filter(|&oid| {
                        world.objects.get_component::<Player>(&oid).is_some_and(|p| p.clan_id == clan_id)
                    })
                    .collect()
            }
        }
    };

    let mut affected = 1;
    for member in members {
        if member == target_oid {
            continue;
        }
        if limit > 0 && affected >= limit {
            break;
        }
        if is_dead(world, member) {
            continue; // Java: `p.isDead()` drops the member
        }
        let Some(pos) = world.objects.get_component::<Position>(&member).copied() else { continue };
        if range > 0 && !within(&origin, &pos, range) {
            continue;
        }
        if !passes_affect_object(world, caster_oid, member, skill.affect_object) {
            continue;
        }
        out.push(member);
        affected += 1;
    }
    out
}

/// Every creature (player or NPC) that could be swept up around `centre_oid` —
/// the port's stand-in for `World.forEachVisibleObjectInRange`'s candidate set.
fn candidates(world: &World, centre_oid: i32) -> Vec<i32> {
    let Some(region) = world.objects.get_component::<RegionCell>(&centre_oid).map(|r| r.0) else {
        return Vec::new();
    };
    let mut out = world.npcs_visible_from(region);
    out.extend(
        in_game_players(world)
            .into_iter()
            .filter(|oid| {
                world
                    .objects
                    .get_component::<RegionCell>(oid)
                    .is_some_and(|r| regions_adjacent(region, r.0))
            }),
    );
    out
}

fn in_game_players(world: &World) -> Vec<i32> {
    world
        .clients
        .values()
        .filter_map(|cs| match cs {
            crate::session::ClientSession::InGame(s) => Some(s.player_object_id()),
            _ => None,
        })
        .collect()
}

fn is_dead(world: &World, oid: i32) -> bool {
    world.objects.get_component::<Vitals>(&oid).map(|v| v.dead).unwrap_or(true)
}

/// Java's dead-target exemption: only the corpse target types keep dead
/// creatures in the affected set.
fn corpse_skill(skill: &Skill) -> bool {
    matches!(skill.target_type, TargetType::NpcBody)
}

/// 3D radius test (Java `isInsideRadius3D`).
fn within(a: &Position, b: &Position, range: i32) -> bool {
    let (dx, dy, dz) = ((a.x - b.x) as f64, (a.y - b.y) as f64, (a.z - b.z) as f64);
    dx * dx + dy * dy + dz * dz <= (range as f64) * (range as f64)
}

/// `handlers/targethandlers/affectobject/*` — the friend/foe filter.
///
/// Java's "friend" test walks command channel → party → event team → olympiad;
/// only the party and clan legs exist in this port, which is the whole of it
/// for a non-event, non-olympiad server. Peace-zone protection is ported since
/// it is the one that visibly matters: an AoE must not clip a player standing
/// in town.
pub(crate) fn passes_affect_object(world: &World, caster_oid: i32, candidate: i32, object: AffectObject) -> bool {
    match object {
        AffectObject::All | AffectObject::Other => true,
        AffectObject::NotFriend => !is_friend(world, caster_oid, candidate) && !protected_by_peace(world, caster_oid, candidate),
        AffectObject::Friend => is_friend(world, caster_oid, candidate),
        AffectObject::Clan => same_clan(world, caster_oid, candidate),
    }
}

/// The caster themselves, a party mate, or a clan mate. NPCs are never
/// friends — Java's check runs through `getActingPlayer()`, which is null for a
/// monster, so a mob always falls through to "not a friend".
fn is_friend(world: &World, caster_oid: i32, candidate: i32) -> bool {
    if caster_oid == candidate {
        return true;
    }
    let both_players = world.objects.has_component::<Player>(&caster_oid)
        && world.objects.has_component::<Player>(&candidate);
    if !both_players {
        return false;
    }
    if same_party(world, caster_oid, candidate) {
        return true;
    }
    same_clan(world, caster_oid, candidate)
}

fn same_party(world: &World, a: i32, b: i32) -> bool {
    let pa = world.objects.get_component::<crate::model::components::PartyRef>(&a).map(|r| r.0);
    let pb = world.objects.get_component::<crate::model::components::PartyRef>(&b).map(|r| r.0);
    matches!((pa, pb), (Some(x), Some(y)) if x == y)
}

fn same_clan(world: &World, a: i32, b: i32) -> bool {
    let ca = world.objects.get_component::<Player>(&a).map(|p| p.clan_id).unwrap_or(0);
    let cb = world.objects.get_component::<Player>(&b).map(|p| p.clan_id).unwrap_or(0);
    ca > 0 && ca == cb
}

/// `NotFriend.checkAffectedObject`'s peace-zone leg: a player standing in a
/// peace zone can't be swept into a hostile AoE. Only applies player→player;
/// monsters in a peace zone are still valid targets (Java tests
/// `target.isInsidePeaceZone(player)`, which is player-scoped).
fn protected_by_peace(world: &World, caster_oid: i32, candidate: i32) -> bool {
    if !world.objects.has_component::<Player>(&candidate) || !world.objects.has_component::<Player>(&caster_oid) {
        return false;
    }
    world
        .objects
        .get_component::<crate::model::components::ZoneFlags>(&candidate)
        .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Peace))
}
