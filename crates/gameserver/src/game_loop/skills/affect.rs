//! Affect scopes and affect objects — how one cast's *primary* target expands
//! into the set of creatures the skill actually lands on.
//!
//! Java splits this across `Skill.forEachTargetAffected` →
//! `handlers/targethandlers/affectscope/*` (the sweep) →
//! `handlers/targethandlers/affectobject/*` (the friend/foe filter). Both sides
//! live here; the caller ([`super::cast::handle_skill_finish`]) just iterates
//! the returned list.
//!
//! **Ported scopes** — the four radius/group scopes that cover the dist's
//! non-single skills: `RANGE` (820 skills), `POINT_BLANK` (785), `PARTY`
//! (272), `PLEDGE` (44) — and the geometric family
//! (PLAN_G19_GEOMETRIC_SCOPES.md): `FAN`/`FAN_PB` (163+16, 5 learnable —
//! Sonic Buster, Force Burst, Wild Sweep, Wrath, Frost Wall), `SQUARE`/
//! `SQUARE_PB` (35+17) and `RING_RANGE` (18), which read the `<fanRange>`
//! tuple for their arc/rect/annulus geometry.
//! …plus the `DEAD_*` family (`DEAD_PLEDGE`/`DEAD_PARTY`/`DEAD_UNION`, the
//! mass-resurrect sweeps, 1 skill each).
//!
//! **Not ported, all falling back to single-target — and every one of them
//! verified to have no carrier a character on this dist can reach** (checked
//! against the skill trees and the whole datapack, the
//! [[l2r-abnormal-resist-dispel]] "rank by learnable usage" discipline):
//! `SUMMON_EXCEPT_MASTER` (22 skills, all id 11269+ — the Freya-era summoner
//! revamp, none learnable); `PARTY_PLEDGE` (5 — the Pa'agrio clan buffs 1534
//! -1563, in no skill tree); `RANGE_SORT_BY_HP` (4 — Chain Heal and later
//! -chronicle heals, likewise); `STATIC_OBJECT_SCOPE` (2 — Nornil's Power and
//! `Test - …` debug skills); and `WYVERN_SCOPE`/`BALAKAS_SCOPE` (boss
//! scripting). Note the first of these is **not** blocked on summons any more
//! — servitors and pets landed at G29; it is blocked on being off-chronicle.
//!
//! **Deviation worth knowing:** Java sweeps `World.forEachVisibleObjectInRange`,
//! which walks the region grid and so is bounded by *visibility*, not just the
//! numeric radius. This port sweeps the same 3×3 region block
//! (`npcs_visible_from` + the in-game player list) and then applies the radius,
//! which is the same set for every `affect_range` the dist actually uses (the
//! largest is 2000, comfortably inside a region block).

use crate::model::Player;
use crate::model::components::{Position, RegionCell, Vitals};
use crate::model::skill::{AffectObject, AffectScope, Skill, TargetType};
use crate::world::{World, regions_adjacent};

/// Resolve the full set of creatures a cast lands on.
///
/// Java `Skill.getTargetsAffected(creature, target)`. For the radius/group
/// scopes the primary target is always first and always included, so callers
/// that treat the first entry specially behave as they did before scopes
/// existed. The **geometric scopes are different**: their geometry applies to
/// the primary target too — a FAN cast at someone behind the caster misses
/// them, and RING_RANGE *never* hits its epicenter target (the donut hole) —
/// so the affected set can come back without the target, or empty.
pub(crate) fn targets_affected(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
) -> Vec<i32> {
    // The limit is rolled once per cast (Java calls `getAffectLimit()` once at
    // the top of each handler), so it must be drawn before the sweep.
    let limit = skill.affect_limit(|bound| world.roll(bound));

    match skill.affect_scope {
        // `Single.java` — and every scope we haven't ported yet, which Java
        // would refuse outright ("Target affect scope ... is not currently
        // handled") but which is far less disruptive to treat as single-target.
        AffectScope::Single | AffectScope::Other => vec![target_oid],
        AffectScope::Range => {
            sweep_radius(world, caster_oid, target_oid, skill, limit, Centre::Target)
        }
        // `PointBlank.java` forks on GROUND: the sweep centres on the stored
        // world position, and the caster sentinel is NOT in the result.
        AffectScope::PointBlank if skill.target_type == TargetType::Ground => {
            sweep_ground(world, caster_oid, skill, limit)
        }
        AffectScope::PointBlank => {
            sweep_radius(world, caster_oid, target_oid, skill, limit, Centre::Caster)
        }
        AffectScope::Party => {
            sweep_group(world, caster_oid, target_oid, skill, limit, Group::Party)
        }
        AffectScope::Pledge => {
            sweep_group(world, caster_oid, target_oid, skill, limit, Group::Clan)
        }
        AffectScope::Fan | AffectScope::FanPointBlank => {
            sweep_fan(world, caster_oid, target_oid, skill, limit)
        }
        AffectScope::Square | AffectScope::SquarePointBlank => {
            sweep_square(world, caster_oid, target_oid, skill, limit)
        }
        AffectScope::RingRange => sweep_ring(world, caster_oid, target_oid, skill, limit),
        AffectScope::DeadPledge => {
            sweep_dead_group(world, caster_oid, target_oid, skill, limit, Group::Clan)
        }
        AffectScope::DeadParty => {
            sweep_dead_group(world, caster_oid, target_oid, skill, limit, Group::Party)
        }
        AffectScope::DeadUnion => {
            sweep_dead_group(world, caster_oid, target_oid, skill, limit, Group::Alliance)
        }
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
    /// `DEAD_UNION`'s membership: the target's party, or any party sharing
    /// their command channel.
    Alliance,
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
    let Some(origin) = world
        .objects
        .get_component::<Position>(&centre_oid)
        .copied()
    else {
        return out;
    };
    // LOS is measured from the *target* in both Java handlers, even for
    // PointBlank (`canSeeTarget(target, c)`).
    let los_from = world
        .objects
        .get_component::<Position>(&target_oid)
        .copied();

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
        // Dist-local fix in `Range.java` (82a54bbc, "Fix minion buffs are
        // given to players"): a monster's *good* skill never sweeps players
        // in, so a mob's mass-buff can't land on bystanders. The primary
        // target is exempt, like the affect-object bypass Java gives it.
        if candidate != target_oid
            && !skill.is_bad()
            && is_monster(world, caster_oid)
            && world.objects.has_component::<Player>(&candidate)
        {
            continue;
        }
        let Some(pos) = world.objects.get_component::<Position>(&candidate).copied() else {
            continue;
        };
        if !within(&origin, &pos, range) {
            continue;
        }
        if !passes_affect_object(world, caster_oid, candidate, skill.affect_object) {
            continue;
        }
        if let Some(from) = los_from
            && !world
                .geo
                .can_see_target(from.x, from.y, from.z, pos.x, pos.y, pos.z)
        {
            continue;
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
    let Some(origin) = world
        .objects
        .get_component::<Position>(&target_oid)
        .copied()
    else {
        return out;
    };
    let range = skill.affect_range;

    let members: Vec<i32> = match group {
        // `Alliance` is only reachable from the DEAD_* sweep, which never
        // calls this helper.
        Group::Alliance => vec![target_oid],
        Group::Party => world
            .objects
            .get_component::<crate::model::components::PartyRef>(&target_oid)
            .and_then(|r| world.parties.get(&r.0))
            .map(|p| p.members.clone())
            // Java: an unpartied target is still "their own party of one".
            .unwrap_or_else(|| vec![target_oid]),
        Group::Clan => {
            let clan_id = world
                .objects
                .get_component::<Player>(&target_oid)
                .map(|p| p.clan_id)
                .unwrap_or(0);
            if clan_id <= 0 {
                vec![target_oid]
            } else {
                in_game_players(world)
                    .into_iter()
                    .filter(|&oid| {
                        world
                            .objects
                            .get_component::<Player>(&oid)
                            .is_some_and(|p| p.clan_id == clan_id)
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
        let Some(pos) = world.objects.get_component::<Position>(&member).copied() else {
            continue;
        };
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

/// `DeadPledge.java` / `DeadParty.java` / `DeadUnion.java` — the mass-resurrect
/// fan-out: the **corpses** of the target's group within `affect_range`.
///
/// Three things separate it from [`sweep_group`], and each one matters:
/// - a candidate qualifies **because** it is dead (`!p.isDead()` → drop);
/// - the **origin itself is filtered**, not assumed in. Mass Resurrection is
///   `targetType SELF`, so the origin is the living caster, who fails the
///   dead test and is correctly left out of their own resurrection;
/// - the affect limit therefore counts from 0, not from 1.
fn sweep_dead_group(
    world: &World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    limit: i32,
    group: Group,
) -> Vec<i32> {
    let Some(origin) = world
        .objects
        .get_component::<Position>(&target_oid)
        .copied()
    else {
        return Vec::new();
    };
    let range = skill.affect_range;

    let same_group = |oid: i32| -> bool {
        if oid == target_oid {
            return true;
        }
        match group {
            Group::Clan => {
                let clan_of = |o: i32| {
                    world
                        .objects
                        .get_component::<Player>(&o)
                        .map(|p| p.clan_id)
                        .unwrap_or(0)
                };
                let c = clan_of(target_oid);
                c != 0 && clan_of(oid) == c
            }
            Group::Party | Group::Alliance => {
                let party_of = |o: i32| {
                    world
                        .objects
                        .get_component::<crate::model::components::PartyRef>(&o)
                        .map(|r| r.0)
                };
                let (a, b) = (party_of(target_oid), party_of(oid));
                match (a, b) {
                    (Some(a), Some(b)) if a == b => true,
                    // A command channel widens DEAD_UNION past the single party
                    // (Java compares the two parties' `getCommandChannel()`).
                    (Some(a), Some(b)) if group == Group::Alliance => {
                        let channel_of = |pid: u32| {
                            world
                                .command_channels
                                .iter()
                                .find(|(_, cc)| cc.parties.contains(&pid))
                                .map(|(&id, _)| id)
                        };
                        matches!(
                            (channel_of(a), channel_of(b)),
                            (Some(x), Some(y)) if x == y
                        )
                    }
                    _ => false,
                }
            }
        }
    };

    let mut out = Vec::new();
    let mut affected = 0;
    for oid in in_game_players(world) {
        if limit > 0 && affected >= limit {
            break;
        }
        if !is_dead(world, oid) || !same_group(oid) {
            continue;
        }
        let Some(pos) = world.objects.get_component::<Position>(&oid).copied() else {
            continue;
        };
        // Java measures from the *origin playable* (the caster, for a SELF
        // cast) and lets the origin itself through without a range test.
        if oid != target_oid && range > 0 && !within(&origin, &pos, range) {
            continue;
        }
        if !passes_affect_object(world, caster_oid, oid, skill.affect_object) {
            continue;
        }
        out.push(oid);
        affected += 1;
    }
    out
}

/// `PointBlank.java`'s GROUND branch — everything within `affect_range` of
/// the **stored world position** (3D, `isInsideRadius3D(worldPosition, …)`),
/// with the usual point-blank filter on top.
///
/// The result never contains the caster: they are the ground cast's sentinel
/// "target", Java's world sweep skips its origin object, and no origin test
/// re-adds them — so a Volcano cannot burn its own caster even with an `ALL`
/// affect object. A caster with no stored position sweeps nothing (Java's
/// `worldPosition != null` gate; non-players never have one).
fn sweep_ground(world: &World, caster_oid: i32, skill: &Skill, limit: i32) -> Vec<i32> {
    let Some(gp) = world
        .objects
        .get_component::<crate::model::components::GroundSkillTarget>(&caster_oid)
        .copied()
    else {
        return Vec::new();
    };
    let centre = Position {
        x: gp.x,
        y: gp.y,
        z: gp.z,
        heading: 0,
    };
    // LOS runs `canSeeTarget(target, c)` and the target sentinel is the
    // caster, so it is measured from the caster's own position.
    let Some(caster_pos) = world
        .objects
        .get_component::<Position>(&caster_oid)
        .copied()
    else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let mut affected = 0;
    for candidate in candidates(world, caster_oid) {
        if candidate == caster_oid {
            continue; // Java's sweep skips its origin object.
        }
        if limit > 0 && affected >= limit {
            break;
        }
        if is_dead(world, candidate) && !corpse_skill(skill) {
            continue;
        }
        let Some(pos) = world.objects.get_component::<Position>(&candidate).copied() else {
            continue;
        };
        if !within(&centre, &pos, skill.affect_range) {
            continue;
        }
        if !passes_affect_object(world, caster_oid, candidate, skill.affect_object) {
            continue;
        }
        if !world.geo.can_see_target(
            caster_pos.x,
            caster_pos.y,
            caster_pos.z,
            pos.x,
            pos.y,
            pos.z,
        ) {
            continue;
        }
        out.push(candidate);
        affected += 1;
    }
    out
}

/// `Fan.java` / `FanPB.java` — an arc of `fan_range[3]` degrees around the
/// caster's heading (rotated by `fan_range[1]`), radius `fan_range[2]`.
///
/// The primary target gets no free pass through the *geometry*: a fan cast at
/// a target behind the caster misses it (Java runs the same filter over
/// everyone; only the affect-object check is bypassed for the target, FAN
/// only). Two quirks ported as written:
///
/// - **No wrap-around normalization** on the angle test. `angle_from` returns
///   [0, 360), so a caster whose heading maps to 350° does *not* hit a target
///   at bearing 10° (|10 − 350| = 340 > half-angle) even though it is 20°
///   away — the live server misses across the 0°/360° seam and so does this.
/// - `fanHalfAngle = fanAngle / 2` is **integer division** widened to double
///   (a 35° fan tests against 17.0, not 17.5).
///
/// FAN also runs the filter over the caster themselves ("including origin
/// itself") — self-bearing is `atan2(0,0) = 0°`, so that passes only when
/// `headingDeg + startDeg` lands inside the half-angle, and NOT_FRIEND drops
/// the caster anyway; ported literally rather than special-cased. LOS is
/// measured from the **caster** (unlike RANGE, which measures from the
/// target).
fn sweep_fan(
    world: &World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    limit: i32,
) -> Vec<i32> {
    let pb = skill.affect_scope == AffectScope::FanPointBlank;
    let Some(origin) = world
        .objects
        .get_component::<Position>(&caster_oid)
        .copied()
    else {
        return Vec::new();
    };
    let heading_deg = heading_to_degree(origin.heading);
    let start_deg = skill.fan_range[1] as f64;
    let radius = skill.fan_range[2];
    let half_angle = (skill.fan_range[3] / 2) as f64;

    let mut out = Vec::new();
    let mut affected = 0;
    // FAN tests the origin explicitly before the sweep; FAN_PB doesn't.
    let mut pool = if pb { Vec::new() } else { vec![caster_oid] };
    pool.extend(
        candidates(world, caster_oid)
            .into_iter()
            .filter(|&c| c != caster_oid),
    );
    for candidate in pool {
        if limit > 0 && affected >= limit {
            break;
        }
        // FAN_PB has no corpse-target exemption: the dead are always dropped.
        if is_dead(world, candidate) && (pb || !corpse_skill(skill)) {
            continue;
        }
        let Some(pos) = world.objects.get_component::<Position>(&candidate).copied() else {
            continue;
        };
        if candidate != caster_oid && !within(&origin, &pos, radius) {
            continue;
        }
        if (angle_from(&origin, &pos) - (heading_deg + start_deg)).abs() > half_angle {
            continue;
        }
        // FAN bypasses the friend/foe filter for the primary target; FAN_PB
        // checks everyone ("without taking target into account").
        if (pb || candidate != target_oid)
            && !passes_affect_object(world, caster_oid, candidate, skill.affect_object)
        {
            continue;
        }
        if !world
            .geo
            .can_see_target(origin.x, origin.y, origin.z, pos.x, pos.y, pos.z)
        {
            continue;
        }
        out.push(candidate);
        affected += 1;
    }
    out
}

/// `Square.java` / `SquarePB.java` — a `fan_range[2]` × `fan_range[3]`
/// rectangle extending from the caster along their heading (rotated by
/// `fan_range[1]`).
///
/// The rect test is Java's exact expression — rotate the candidate by the
/// negated heading around the caster, then compare against an axis-aligned
/// rect at `(x, y − width/2)` — including the integer division in `width / 2`
/// and the `(int)` truncation of the rotated coordinates. The strict `>`
/// against `rectX` means the caster's own corner position never passes, so
/// Java's origin self-test is dead code here; running the same filter
/// reproduces that for free. LOS from the caster, like FAN.
fn sweep_square(
    world: &World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    limit: i32,
) -> Vec<i32> {
    let pb = skill.affect_scope == AffectScope::SquarePointBlank;
    let Some(origin) = world
        .objects
        .get_component::<Position>(&caster_oid)
        .copied()
    else {
        return Vec::new();
    };
    let length = skill.fan_range[2];
    let width = skill.fan_range[3];
    // Java: `(int) Math.sqrt(len² + w²)` — the radius handed to the world
    // sweep, truncated.
    let radius = (((length * length) + (width * width)) as f64).sqrt() as i32;
    let rect_x = origin.x;
    let rect_y = origin.y - (width / 2);
    let heading = (skill.fan_range[1] as f64 + heading_to_degree(origin.heading)).to_radians();
    let (sin, cos) = (-heading).sin_cos();

    let mut out = Vec::new();
    let mut affected = 0;
    let mut pool = if pb { Vec::new() } else { vec![caster_oid] };
    pool.extend(
        candidates(world, caster_oid)
            .into_iter()
            .filter(|&c| c != caster_oid),
    );
    for candidate in pool {
        if limit > 0 && affected >= limit {
            break;
        }
        if is_dead(world, candidate) && (pb || !corpse_skill(skill)) {
            continue;
        }
        let Some(pos) = world.objects.get_component::<Position>(&candidate).copied() else {
            continue;
        };
        if candidate != caster_oid && !within(&origin, &pos, radius) {
            continue;
        }
        let xp = (pos.x - origin.x) as f64;
        let yp = (pos.y - origin.y) as f64;
        let xr = (origin.x as f64 + (xp * cos) - (yp * sin)) as i32;
        let yr = (origin.y as f64 + (xp * sin) + (yp * cos)) as i32;
        if !(xr > rect_x && xr < rect_x + length && yr > rect_y && yr < rect_y + width) {
            continue;
        }
        if (pb || candidate != target_oid)
            && !passes_affect_object(world, caster_oid, candidate, skill.affect_object)
        {
            continue;
        }
        if !world
            .geo
            .can_see_target(origin.x, origin.y, origin.z, pos.x, pos.y, pos.z)
        {
            continue;
        }
        out.push(candidate);
        affected += 1;
    }
    out
}

/// `RingRange.java` — an annulus around the **target**: inside `affect_range`
/// (3D, the world sweep's bound) but not inside `fan_range[2]` of the target
/// (2D, Java's `isInsideRadius2D`).
///
/// The epicenter target is never affected: the world sweep skips its own
/// origin object, and the 2D inner-radius test would drop it anyway — that is
/// the donut hole. No corpse exemption, no affect-object bypass for anyone,
/// and LOS is measured from the **target** (like RANGE).
fn sweep_ring(
    world: &World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    limit: i32,
) -> Vec<i32> {
    let Some(centre) = world
        .objects
        .get_component::<Position>(&target_oid)
        .copied()
    else {
        return Vec::new();
    };
    let range = skill.affect_range;
    let start_range = skill.fan_range[2];

    let mut out = Vec::new();
    let mut affected = 0;
    for candidate in candidates(world, target_oid) {
        if candidate == target_oid {
            continue; // Java's sweep skips its origin object.
        }
        if limit > 0 && affected >= limit {
            break;
        }
        if is_dead(world, candidate) {
            continue;
        }
        let Some(pos) = world.objects.get_component::<Position>(&candidate).copied() else {
            continue;
        };
        if !within(&centre, &pos, range) {
            continue;
        }
        // "Targets before the start range are unaffected."
        if within_2d(&centre, &pos, start_range) {
            continue;
        }
        if !passes_affect_object(world, caster_oid, candidate, skill.affect_object) {
            continue;
        }
        if !world
            .geo
            .can_see_target(centre.x, centre.y, centre.z, pos.x, pos.y, pos.z)
        {
            continue;
        }
        out.push(candidate);
        affected += 1;
    }
    out
}

/// Java `Util.convertHeadingToDegree` — client heading units (0..65536, 0 =
/// east, counter-clockwise) to degrees, with Java's exact divisor.
fn heading_to_degree(heading: i32) -> f64 {
    heading as f64 / 182.044444444
}

/// Java `Util.calculateAngleFrom` — the world-plane bearing from `a` to `b`
/// in degrees, normalized to [0, 360).
fn angle_from(a: &Position, b: &Position) -> f64 {
    let mut deg = ((b.y - a.y) as f64).atan2((b.x - a.x) as f64).to_degrees();
    if deg < 0.0 {
        deg += 360.0;
    }
    deg
}

/// 2D radius test (Java `isInsideRadius2D`).
fn within_2d(a: &Position, b: &Position, range: i32) -> bool {
    let (dx, dy) = ((a.x - b.x) as f64, (a.y - b.y) as f64);
    dx * dx + dy * dy <= (range as f64) * (range as f64)
}

/// Every creature (player or NPC) that could be swept up around `centre_oid` —
/// the port's stand-in for `World.forEachVisibleObjectInRange`'s candidate set.
fn candidates(world: &World, centre_oid: i32) -> Vec<i32> {
    let Some(region) = world
        .objects
        .get_component::<RegionCell>(&centre_oid)
        .map(|r| r.0)
    else {
        return Vec::new();
    };
    let mut out = world.npcs_visible_from(region);
    out.extend(in_game_players(world).into_iter().filter(|oid| {
        world
            .objects
            .get_component::<RegionCell>(oid)
            .is_some_and(|r| regions_adjacent(region, r.0))
    }));
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
    world
        .objects
        .get_component::<Vitals>(&oid)
        .map(|v| v.dead)
        .unwrap_or(true)
}

/// Java's dead-target exemption: only the corpse target types (`NPC_BODY`,
/// `PC_BODY`) keep dead creatures in the affected set. The `*_PB` geometric
/// scopes don't grant it — they drop the dead unconditionally.
fn corpse_skill(skill: &Skill) -> bool {
    matches!(skill.target_type, TargetType::NpcBody | TargetType::PcBody)
}

/// The same "is this a monster" test the targeting code uses: an NPC whose
/// template is auto-attackable.
fn is_monster(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&oid)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.is_auto_attackable())
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
pub(crate) fn passes_affect_object(
    world: &World,
    caster_oid: i32,
    candidate: i32,
    object: AffectObject,
) -> bool {
    match object {
        AffectObject::All | AffectObject::Other => true,
        AffectObject::NotFriend => {
            !is_friend(world, caster_oid, candidate)
                && !protected_by_peace(world, caster_oid, candidate)
        }
        AffectObject::Friend => is_friend(world, caster_oid, candidate),
        AffectObject::Clan => same_clan(world, caster_oid, candidate),
    }
}

/// The caster themselves, a party mate, or a clan mate. NPCs are never
/// friends — Java's check runs through `getActingPlayer()`, which is null for a
/// monster, so a mob always falls through to "not a friend".
fn is_friend(world: &World, caster_oid: i32, candidate: i32) -> bool {
    // Java's friend tests run on `getActingPlayer()` — an owned summon (a
    // symbol totem, a servitor) counts as its owner, so a Day of Doom seal
    // never curses the player who dropped it, or their party/clan.
    let caster_oid = crate::game_loop::pvp::acting_player(world, caster_oid);
    let candidate = crate::game_loop::pvp::acting_player(world, candidate);
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
    let pa = world
        .objects
        .get_component::<crate::model::components::PartyRef>(&a)
        .map(|r| r.0);
    let pb = world
        .objects
        .get_component::<crate::model::components::PartyRef>(&b)
        .map(|r| r.0);
    matches!((pa, pb), (Some(x), Some(y)) if x == y)
}

fn same_clan(world: &World, a: i32, b: i32) -> bool {
    let ca = world
        .objects
        .get_component::<Player>(&a)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    let cb = world
        .objects
        .get_component::<Player>(&b)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    ca > 0 && ca == cb
}

/// `NotFriend.checkAffectedObject`'s peace-zone leg: a player standing in a
/// peace zone can't be swept into a hostile AoE. Only applies player→player;
/// monsters in a peace zone are still valid targets (Java tests
/// `target.isInsidePeaceZone(player)`, which is player-scoped).
fn protected_by_peace(world: &World, caster_oid: i32, candidate: i32) -> bool {
    if !world.objects.has_component::<Player>(&candidate)
        || !world.objects.has_component::<Player>(&caster_oid)
    {
        return false;
    }
    world
        .objects
        .get_component::<crate::model::components::ZoneFlags>(&candidate)
        .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Peace))
}
